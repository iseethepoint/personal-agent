//! `muse.exec` и `process.list/poll/log/write/send_keys/kill/clear` — по форме
//! muse `subprocesses/*` (`source-files.txt:1764-1796`: `exec_tool.rs`,
//! `process_tool.rs`, `executor/local*`, `redaction.rs`).
//!
//! Команда идёт в оболочке, рабочая папка — домашняя. Если через `yield_ms`
//! (по умолчанию 10 000 мс) она ещё работает, она уходит в фон: модель
//! получает id, а итог потом приходит сам — заметкой в ящик (`Inbox`).
//! С `background: true` — в фон сразу. Сессии видны через `process.*`.
//! Значения переменных с ключами моделей (`api_key_env` реестра) в выводе
//! заменяются. Своих запретов на команды нет: граница — права ОС.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_loop::{personal, Inbox, Tool, ToolError, ToolFuture, Tools};
use agent_model::{Message, Part, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Notify;

use crate::paths::Home;

// Описания — строки ядра muse, полные варианты, личный вариант:
// `muse.exec` — 0x904b8e–0x905244, `process.*` — 0x2cd0164–0x2cd1b55.
// Правки (П10): имя `muse.exec` → `hamlet.exec`.
const EXEC_DESCRIPTION: &str = "Execute a shell command. If the command is still running after `yield_ms`, it backgrounds and the runtime delivers the terminal result back into your context automatically. Use exec only for operations that have no purpose-built tool.\nNever use exec to imitate, bypass, or probe a tool that is unavailable or has rejected a call.";
const EXEC_COMMAND: &str = "Shell command to execute.\n\nFor a command that reads literal text or code from stdin, put that input in\nthe separate `stdin` field instead of interpolating it through a heredoc,\nshell variable, or `echo`/`printf` pipeline. Normal shell pipelines and\nredirections still work. Follow each CLI's documented input interface; this\nfield does not replace required command-line arguments.";
const EXEC_STDIN: &str = "Optional complete literal UTF-8 input. The runtime writes this text\nunchanged to the command's stdin, then closes the input stream. No shell\nexpansion or extra newline is applied. An empty string sends immediate EOF;\nomit this field to send input incrementally through process.write, setting\neof=true on the final write. The input shares the normal request-size limits\nwith the other tool arguments.\n\nFor a pipeline, this supplies the first command's input; subsequent commands\nread the preceding command's output. Explicit input redirections override\nthe supplied stream normally. Put the actual input here, not a shell variable\nor placeholder. Do not use process.write to append to supplied stdin.";
const EXEC_WORKDIR: &str = "Working directory. Defaults to your working directory.";
const EXEC_ENV: &str = "Environment variables to set.";
const EXEC_BACKGROUND: &str = "Run in background immediately. The runtime will deliver the terminal result automatically when it completes.";
const EXEC_YIELD: &str = "Milliseconds to wait before backgrounding (default 10000).";
const EXEC_PTY: &str = "Run in a pseudo-terminal.";

const LIST_DESCRIPTION: &str = "List every background process session the current agent has started through exec, running or finished. Takes no parameters. Each entry reports the session id, command line, pid, working directory, status (running, completed, or failed), exit code and signal, failure kind, whether the command was backgrounded, elapsed milliseconds, total output characters, whether output was truncated, and whether the session runs in a PTY. Sessions are scoped to the calling agent; other agents' commands do not appear. Backgrounded exec commands deliver their terminal result into your context automatically, so do not use this tool to wait for completion. For one session's output use process.log or process.poll.";
const POLL_DESCRIPTION: &str = "Wait up to timeout milliseconds for a background process session started by exec to produce new output or exit, then return its state. The reply carries the session status, exit code and signal, failure kind, the stdout and stderr produced since the last poll (deltas are consumed and returned once), elapsed milliseconds, total output characters, whether output was truncated, and a tail excerpt of recent output. timeout defaults to 1000 and is clamped to 0-120000; 0 returns the current state immediately. Backgrounded exec commands deliver their terminal result into your context automatically, so never poll just to wait for completion; if a finished session's completion message is already queued for automatic delivery, this call returns an error instead of double-reporting it. For the full accumulated log use process.log; to stop the process use process.kill.";
const POLL_TIMEOUT: &str = "How long to wait in milliseconds for new output or exit before returning. Default 1000, clamped to 0-120000; 0 returns immediately.";
const LOG_DESCRIPTION: &str = "Read a paged window of the accumulated output of a background process session started by exec. Output is split into lines: offset is the zero-based line to start from (default 0) and limit is how many lines to return (default 200, maximum 1000). The reply carries the requested lines plus nextOffset and totalLines for paging, the session status, exit code and signal, failure kind, whether output was truncated, and a tail excerpt. Reading is non-destructive and repeatable: unlike process.poll it consumes nothing, so you can page through history freely. Use process.poll instead when you want to wait briefly for fresh output.";
const SESSION_ID: &str = "The session id exec returned for this tracked command.";
const LOG_OFFSET: &str = "Zero-based line offset into the session's accumulated output. Defaults to 0.";
const LOG_LIMIT: &str = "Maximum number of lines to return. Default 200, clamped to 1-1000.";
const KILL_DESCRIPTION: &str = "Terminate a background process session started by exec. The kill tears down the whole process tree, including descendants that detached from the original process group, then reports the session's final status, exit code and signal, and failure kind. Killing does not remove the session: its output stays readable with process.log until you drop it with process.clear. Fails if the session id is unknown. Backgrounded commands that finish on their own report back automatically; kill is for stopping work you no longer want.";
const CLEAR_DESCRIPTION: &str = "Remove a finished background process session from tracking and discard its buffered output. Only sessions that already exited can be cleared: the call fails if the process is still running (stop it first with process.kill) or the session id is unknown. After clearing, the session no longer appears in process.list and its logs are gone. Clearing is optional housekeeping; finished sessions otherwise stay listed with their output available to process.log.";
const WRITE_DESCRIPTION: &str = "Write raw text to the stdin of a background process session started by exec. data is sent exactly as given, so include a trailing newline to submit a line to line-oriented programs. Set eof true to close the session's stdin after the write, which lets programs that read until end-of-input proceed. The reply reports the session id, bytes written, and whether stdin was closed. Fails if the session is unknown or its stdin is no longer writable. For named terminal keys such as enter, arrows, or control chords in a PTY session, use process.send_keys; to read what the program printed back, follow with process.poll.";
const WRITE_DATA: &str = "Raw text to write to the session's stdin, sent exactly as given. Include a trailing newline to submit a line.";
const WRITE_EOF: &str = "Close the session's stdin after this write. Defaults to false.";
const KEYS_DESCRIPTION: &str = "Send named key presses to a background process session started by exec, encoded as terminal control sequences and written to the session's input. Intended for PTY sessions running interactive programs. keys is an ordered list of case-insensitive tokens: enter or return, tab, escape or esc, space, backspace, up, down, left, right, home, end, pageup, pagedown, insert, delete, f1 through f12, and control chords written c-<char> (for example c-c, c-d, c-z). An unsupported token fails the whole call and nothing is sent. Set eof true to close the session's input after the keys. The reply reports the session id, bytes written, and whether input was closed. For plain text use process.write; to see the program's response, follow with process.poll.";
const KEYS_KEYS: &str = "Ordered key tokens to send, for example [\"c-c\"] or [\"escape\", \"enter\"]. The tool description lists every supported token.";
const KEYS_EOF: &str = "Close the session's input after sending the keys. Defaults to false.";

/// `yield_ms` по умолчанию — описание muse.
pub const DEFAULT_YIELD_MS: u64 = 10_000;
/// Замена значения ключа в выводе. В ядре рядом с `stdin`/`env` видна строка
/// `<redacted>` (0x869117); точной связи с `redaction.rs` по разбору нет.
pub const REDACTED: &str = "<redacted>";
/// Сколько ждать закрытия вывода после выхода оболочки — «did not close
/// within two seconds» (строка ядра около 0x9049a0).
const OUTPUT_CLOSE_WAIT: Duration = Duration::from_secs(2);
const TAIL_CHARS: usize = 2000;
/// Предел накопленного вывода сессии. Поле `truncated` в описаниях muse
/// говорит, что предел есть, но числа в ядре не найдено — взят порог spill
/// из П3а, 100 КБ. Держится хвост; начало отбрасывается.
pub const OUTPUT_CAP_BYTES: usize = agent_loop::SPILL_THRESHOLD_BYTES;

type Writer = Box<dyn tokio::io::AsyncWrite + Send + Unpin>;
/// Поток вывода и признак stderr.
type Output = (Box<dyn tokio::io::AsyncRead + Send + Unpin>, bool);

/// Дописать кусок и удержать хвост в пределе. `true` — начало отброшено.
fn push_capped(buf: &mut String, piece: &str) -> bool {
    buf.push_str(piece);
    if buf.len() <= OUTPUT_CAP_BYTES {
        return false;
    }
    let mut cut = buf.len() - OUTPUT_CAP_BYTES;
    while !buf.is_char_boundary(cut) {
        cut += 1;
    }
    buf.drain(..cut);
    true
}

#[derive(Default)]
struct State {
    log: String,
    stdout_delta: String,
    stderr_delta: String,
    exit_code: Option<i32>,
    signal: Option<i32>,
    done: bool,
    backgrounded: bool,
    finished: Option<Instant>,
    truncated: bool,
}

struct Session {
    id: String,
    command: String,
    pid: Option<u32>,
    cwd: String,
    started: Instant,
    state: Mutex<State>,
    stdin: tokio::sync::Mutex<Option<Writer>>,
    pty: bool,
    changed: Notify,
}

impl Session {
    fn status(st: &State) -> &'static str {
        match (st.done, st.exit_code) {
            (false, _) => "running",
            (true, Some(0)) => "completed",
            (true, _) => "failed",
        }
    }
}

struct Inner {
    home: Home,
    secrets: Vec<String>,
    inbox: Option<Inbox>,
    /// Папка, где команда начинается без `workdir`; нет — дом.
    start: Option<PathBuf>,
    sessions: Mutex<Vec<Arc<Session>>>,
    next: Mutex<u64>,
}

/// Исполнение команд одного агента.
#[derive(Clone)]
pub struct ExecTools {
    inner: Arc<Inner>,
}

impl ExecTools {
    /// `secret_envs` — имена переменных с ключами (`secret_envs`: реестр
    /// моделей и настройка поиска);
    /// `inbox` — куда приходят итоги фоновых команд.
    pub fn new(home: Home, secret_envs: Vec<String>, inbox: Option<Inbox>) -> Self {
        Self::starting_in(home, secret_envs, inbox, None)
    }

    /// Команды без `workdir` начинаются в `start` — у шага сохранённой
    /// работы это общая папка запуска («Exec commands start in this
    /// directory unless you specify another working directory»,
    /// `prompts/workflows/execution_files.md`).
    pub fn starting_in(home: Home, secret_envs: Vec<String>, inbox: Option<Inbox>, start: Option<PathBuf>) -> Self {
        ExecTools {
            inner: Arc::new(Inner { home, secrets: secret_envs, inbox, start, sessions: Mutex::new(Vec::new()), next: Mutex::new(0) }),
        }
    }

    pub fn add_to(&self, tools: &mut Tools) {
        for op in [Op::Exec, Op::List, Op::Poll, Op::Log, Op::Write, Op::SendKeys, Op::Kill, Op::Clear] {
            tools.add(ExecTool { op, ex: self.clone() });
        }
    }

    /// Значения ключей → `<redacted>`.
    fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for name in &self.inner.secrets {
            if let Ok(v) = std::env::var(name) {
                if !v.is_empty() {
                    out = out.replace(&v, REDACTED);
                }
            }
        }
        out
    }

    fn get(&self, id: &str) -> Result<Arc<Session>, ToolError> {
        self.inner
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| ToolError::Failed(format!("unknown process session `{id}`")))
    }

    fn summary(&self, s: &Session, st: &State) -> Value {
        json!({
            "id": s.id,
            "command": self.redact(&s.command),
            "pid": s.pid,
            "cwd": s.cwd,
            "status": Session::status(st),
            "exit_code": st.exit_code,
            "signal": st.signal,
            "backgrounded": st.backgrounded,
            "elapsed_ms": st.finished.unwrap_or_else(Instant::now).duration_since(s.started).as_millis() as u64,
            "output_chars": st.log.chars().count(),
            "truncated": st.truncated,
            "pty": s.pty,
        })
    }

    fn tail(&self, log: &str) -> String {
        let n = log.chars().count();
        self.redact(&log.chars().skip(n.saturating_sub(TAIL_CHARS)).collect::<String>())
    }

    async fn exec(&self, args: Value) -> Result<Value, ToolError> {
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("`command` (string) is required".into()))?
            .to_string();
        let pty = args.get("pty").and_then(Value::as_bool).unwrap_or(false);
        let cwd = match args.get("workdir").and_then(Value::as_str) {
            Some(w) => self.inner.home.resolve(w).map_err(ToolError::InvalidArguments)?,
            None => self.inner.start.clone().unwrap_or_else(|| self.inner.home.dir.clone()),
        };
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&command).current_dir(&cwd);
        if let Some(env) = args.get("env").and_then(Value::as_object) {
            for (k, v) in env {
                cmd.env(k, v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()));
            }
        }
        let start = |e: std::io::Error| ToolError::Failed(format!("failed to start command: {e}"));
        let (mut child, mut stdin, outputs): (_, Option<Writer>, Vec<Output>) = if pty {
            let (master, slave) = open_pty().map_err(start)?;
            cmd.stdin(slave.try_clone().map_err(start)?)
                .stdout(slave.try_clone().map_err(start)?)
                .stderr(slave);
            // Свой сеанс с управляющим терминалом — как у программы в терминале.
            unsafe {
                cmd.pre_exec(|| {
                    if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = cmd.spawn().map_err(start)?;
            drop(cmd); // родитель не держит сторону подчинённого терминала
            let reader = tokio::fs::File::from_std(std::fs::File::from(master.try_clone().map_err(start)?));
            let writer = tokio::fs::File::from_std(std::fs::File::from(master));
            (child, Some(Box::new(writer) as Writer), vec![(Box::new(reader) as _, false)])
        } else {
            cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
            #[cfg(unix)]
            cmd.process_group(0);
            let mut child = cmd.spawn().map_err(start)?;
            let stdin = child.stdin.take().map(|w| Box::new(w) as Writer);
            let mut outs: Vec<Output> = Vec::new();
            if let Some(r) = child.stdout.take() {
                outs.push((Box::new(r), false));
            }
            if let Some(r) = child.stderr.take() {
                outs.push((Box::new(r), true));
            }
            (child, stdin, outs)
        };
        if let Some(input) = args.get("stdin").and_then(Value::as_str) {
            if let Some(mut w) = stdin.take() {
                let _ = w.write_all(input.as_bytes()).await;
                if pty {
                    // У терминала конец ввода — знак EOT (Ctrl-D), а не закрытие.
                    let _ = w.write_all(&[0x04]).await;
                    let _ = w.flush().await;
                    stdin = Some(w);
                }
            }
        }
        let id = {
            let mut n = self.inner.next.lock().unwrap();
            *n += 1;
            format!("proc_{n}")
        };
        let session = Arc::new(Session {
            id: id.clone(),
            command,
            pid: child.id(),
            cwd: cwd.display().to_string(),
            started: Instant::now(),
            state: Mutex::new(State::default()),
            stdin: tokio::sync::Mutex::new(stdin),
            pty,
            changed: Notify::new(),
        });
        self.inner.sessions.lock().unwrap().push(session.clone());

        let readers: Vec<_> = outputs.into_iter().map(|(r, err)| self.reader(session.clone(), r, err)).collect();
        let me = self.clone();
        let s = session.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            let _ = tokio::time::timeout(OUTPUT_CLOSE_WAIT, async {
                for r in readers {
                    let _ = r.await;
                }
            })
            .await;
            let deliver = {
                let mut st = s.state.lock().unwrap();
                if let Ok(status) = status {
                    st.exit_code = status.code();
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        st.signal = status.signal();
                    }
                }
                st.done = true;
                st.finished = Some(Instant::now());
                st.backgrounded.then(|| me.completion(&s, &st))
            };
            s.changed.notify_waiters();
            if let (Some(m), Some(inbox)) = (deliver, &me.inner.inbox) {
                inbox.push(m);
            }
        });

        let background = args.get("background").and_then(Value::as_bool).unwrap_or(false);
        let yield_ms = if background { 0 } else { args.get("yield_ms").and_then(Value::as_u64).unwrap_or(DEFAULT_YIELD_MS) };
        let deadline = tokio::time::Instant::now() + Duration::from_millis(yield_ms);
        loop {
            let notified = session.changed.notified();
            {
                let mut st = session.state.lock().unwrap();
                if st.done {
                    let mut out = self.summary(&session, &st);
                    out["output"] = self.redact(&st.log).into();
                    st.stdout_delta.clear();
                    st.stderr_delta.clear();
                    return Ok(out);
                }
                if tokio::time::Instant::now() >= deadline {
                    st.backgrounded = true;
                    let mut out = self.summary(&session, &st);
                    out["output"] = self.redact(&st.log).into();
                    // Отданное здесь — уже доставлено: poll его не повторит.
                    st.stdout_delta.clear();
                    st.stderr_delta.clear();
                    return Ok(out);
                }
            }
            let _ = tokio::time::timeout_at(deadline, notified).await;
        }
    }

    /// Заметка о конце фоновой команды. Формы muse в разборе нет — своя.
    fn completion(&self, s: &Session, st: &State) -> Message {
        let head = format!(
            "[Background exec finished] id: {}, status: {}, exit_code: {}",
            s.id,
            Session::status(st),
            st.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "none".into())
        );
        Message { role: Role::Developer, content: vec![Part::Text { text: format!("{head}\n\n{}", self.tail(&st.log)) }] }
    }

    fn reader(
        &self,
        s: Arc<Session>,
        mut r: impl tokio::io::AsyncRead + Unpin + Send + 'static,
        stderr: bool,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let mut pending = Vec::new();
            loop {
                let n = match r.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                pending.extend_from_slice(&buf[..n]);
                // Только целые знаки UTF-8; хвост ждёт следующего куска.
                let valid = match std::str::from_utf8(&pending) {
                    Ok(_) => pending.len(),
                    Err(e) => e.valid_up_to(),
                };
                let piece = String::from_utf8_lossy(&pending[..valid]).to_string();
                pending.drain(..valid);
                {
                    let mut st = s.state.lock().unwrap();
                    let cut = push_capped(&mut st.log, &piece);
                    st.truncated |= cut;
                    if stderr { push_capped(&mut st.stderr_delta, &piece); } else { push_capped(&mut st.stdout_delta, &piece); }
                }
                s.changed.notify_waiters();
            }
            if !pending.is_empty() {
                let piece = String::from_utf8_lossy(&pending).to_string();
                let mut st = s.state.lock().unwrap();
                let cut = push_capped(&mut st.log, &piece);
                st.truncated |= cut;
                if stderr { push_capped(&mut st.stderr_delta, &piece); } else { push_capped(&mut st.stdout_delta, &piece); }
            }
        })
    }

    async fn poll(&self, s: Arc<Session>, timeout_ms: u64) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let notified = s.changed.notified();
            {
                let mut st = s.state.lock().unwrap();
                let fresh = !st.stdout_delta.is_empty() || !st.stderr_delta.is_empty();
                if fresh || st.done || tokio::time::Instant::now() >= deadline {
                    let mut out = self.summary(&s, &st);
                    out["stdout"] = self.redact(&std::mem::take(&mut st.stdout_delta)).into();
                    out["stderr"] = self.redact(&std::mem::take(&mut st.stderr_delta)).into();
                    out["tail"] = self.tail(&st.log).into();
                    return out;
                }
            }
            let _ = tokio::time::timeout_at(deadline, notified).await;
        }
    }

    async fn kill(&self, s: Arc<Session>) -> Result<Value, ToolError> {
        if !s.state.lock().unwrap().done {
            if let Some(pid) = s.pid {
                // Вся группа процессов: оболочка и её потомки.
                #[cfg(unix)]
                unsafe {
                    libc::killpg(pid as libc::pid_t, libc::SIGKILL);
                }
            }
            loop {
                let notified = s.changed.notified();
                if s.state.lock().unwrap().done {
                    break;
                }
                let _ = tokio::time::timeout(Duration::from_secs(5), notified).await;
            }
        }
        let st = s.state.lock().unwrap();
        Ok(self.summary(&s, &st))
    }
}

fn key_bytes(token: &str) -> Option<Vec<u8>> {
    let t = token.to_ascii_lowercase();
    let seq: &[u8] = match t.as_str() {
        "enter" | "return" => b"\r",
        "tab" => b"\t",
        "escape" | "esc" => b"\x1b",
        "space" => b" ",
        "backspace" => b"\x7f",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" => b"\x1b[5~",
        "pagedown" => b"\x1b[6~",
        "insert" => b"\x1b[2~",
        "delete" => b"\x1b[3~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => {
            let c = t.strip_prefix("c-")?;
            let mut chars = c.chars();
            let ch = chars.next()?;
            if chars.next().is_some() || !ch.is_ascii() {
                return None;
            }
            return Some(vec![(ch as u8) & 0x1f]);
        }
    };
    Some(seq.to_vec())
}

#[derive(Clone, Copy)]
enum Op {
    Exec,
    List,
    Poll,
    Log,
    Write,
    SendKeys,
    Kill,
    Clear,
}

struct ExecTool {
    op: Op,
    ex: ExecTools,
}

fn p(t: &str, d: &str) -> Value {
    json!({"type": t, "description": personal(d)})
}

fn sid() -> Value {
    p("string", SESSION_ID)
}

impl Tool for ExecTool {
    fn spec(&self) -> ToolSpec {
        let (name, d, props, req) = match self.op {
            Op::Exec => (
                "hamlet.exec",
                EXEC_DESCRIPTION,
                json!({
                    "command": p("string", EXEC_COMMAND),
                    "stdin": p("string", EXEC_STDIN),
                    "workdir": p("string", EXEC_WORKDIR),
                    "env": {"type": "object", "additionalProperties": {"type": "string"}, "description": EXEC_ENV},
                    "background": p("boolean", EXEC_BACKGROUND),
                    "yield_ms": p("integer", EXEC_YIELD),
                    "pty": p("boolean", EXEC_PTY),
                }),
                json!(["command"]),
            ),
            Op::List => ("process.list", LIST_DESCRIPTION, json!({}), json!([])),
            Op::Poll => ("process.poll", POLL_DESCRIPTION, json!({"session_id": sid(), "timeout": p("integer", POLL_TIMEOUT)}), json!(["session_id"])),
            Op::Log => (
                "process.log",
                LOG_DESCRIPTION,
                json!({"session_id": sid(), "offset": p("integer", LOG_OFFSET), "limit": p("integer", LOG_LIMIT)}),
                json!(["session_id"]),
            ),
            Op::Write => (
                "process.write",
                WRITE_DESCRIPTION,
                json!({"session_id": sid(), "data": p("string", WRITE_DATA), "eof": p("boolean", WRITE_EOF)}),
                json!(["session_id", "data"]),
            ),
            Op::SendKeys => (
                "process.send_keys",
                KEYS_DESCRIPTION,
                json!({"session_id": sid(), "keys": {"type": "array", "items": {"type": "string"}, "description": KEYS_KEYS}, "eof": p("boolean", KEYS_EOF)}),
                json!(["session_id", "keys"]),
            ),
            Op::Kill => ("process.kill", KILL_DESCRIPTION, json!({"session_id": sid()}), json!(["session_id"])),
            Op::Clear => ("process.clear", CLEAR_DESCRIPTION, json!({"session_id": sid()}), json!(["session_id"])),
        };
        ToolSpec { name: name.into(), description: personal(d), input_schema: json!({"type": "object", "properties": props, "required": req}) }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let ex = &self.ex;
            let session = || {
                let id = args
                    .get("session_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ToolError::InvalidArguments("`session_id` (string) is required".into()))?;
                ex.get(id)
            };
            let out = match self.op {
                Op::Exec => ex.exec(args.clone()).await?,
                Op::List => {
                    let sessions = ex.inner.sessions.lock().unwrap().clone();
                    Value::Array(sessions.iter().map(|s| ex.summary(s, &s.state.lock().unwrap())).collect())
                }
                Op::Poll => {
                    let t = args.get("timeout").and_then(Value::as_u64).unwrap_or(1000).min(120_000);
                    ex.poll(session()?, t).await
                }
                Op::Log => {
                    let s = session()?;
                    let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(200).clamp(1, 1000) as usize;
                    let st = s.state.lock().unwrap();
                    let log = ex.redact(&st.log);
                    let lines: Vec<&str> = log.lines().collect();
                    let window: Vec<&str> = lines.iter().skip(offset).take(limit).copied().collect();
                    let mut out = ex.summary(&s, &st);
                    out["lines"] = json!(window);
                    out["nextOffset"] = json!((offset + window.len()).min(lines.len()));
                    out["totalLines"] = json!(lines.len());
                    out["tail"] = ex.tail(&st.log).into();
                    out
                }
                Op::Write | Op::SendKeys => {
                    let s = session()?;
                    let bytes = if let Op::Write = self.op {
                        args.get("data")
                            .and_then(Value::as_str)
                            .ok_or_else(|| ToolError::InvalidArguments("`data` (string) is required".into()))?
                            .as_bytes()
                            .to_vec()
                    } else {
                        let keys = args
                            .get("keys")
                            .and_then(Value::as_array)
                            .ok_or_else(|| ToolError::InvalidArguments("`keys` (array) is required".into()))?;
                        let mut b = Vec::new();
                        for k in keys {
                            let t = k.as_str().unwrap_or("");
                            b.extend(key_bytes(t).ok_or_else(|| ToolError::InvalidArguments(format!("unsupported key token `{t}`")))?);
                        }
                        b
                    };
                    let eof = args.get("eof").and_then(Value::as_bool).unwrap_or(false);
                    let mut stdin = s.stdin.lock().await;
                    let w = stdin.as_mut().ok_or_else(|| ToolError::Failed(format!("stdin of `{}` is no longer writable", s.id)))?;
                    w.write_all(&bytes).await.map_err(|e| ToolError::Failed(format!("write to `{}` failed: {e}", s.id)))?;
                    if eof && s.pty {
                        // У терминала конец ввода — знак EOT (Ctrl-D).
                        w.write_all(&[0x04]).await.map_err(|e| ToolError::Failed(format!("write to `{}` failed: {e}", s.id)))?;
                    }
                    let _ = w.flush().await;
                    if eof && !s.pty {
                        *stdin = None;
                    }
                    json!({"session_id": s.id, "bytes_written": bytes.len(), "stdin_closed": eof})
                }
                Op::Kill => ex.kill(session()?).await?,
                Op::Clear => {
                    let s = session()?;
                    if !s.state.lock().unwrap().done {
                        return Err(ToolError::Failed(format!("session `{}` is still running; stop it first with process.kill", s.id)));
                    }
                    ex.inner.sessions.lock().unwrap().retain(|x| x.id != s.id);
                    json!({"session_id": s.id, "cleared": true})
                }
            };
            Ok(vec![ToolResultPart::Text { text: out.to_string() }])
        })
    }
}

/// Пара псевдотерминала: (ведущая сторона, подчинённая).
fn open_pty() -> std::io::Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    use std::os::fd::FromRawFd;
    let (mut master, mut slave) = (0, 0);
    let rc = unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    unsafe { Ok((std::os::fd::OwnedFd::from_raw_fd(master), std::os::fd::OwnedFd::from_raw_fd(slave))) }
}
