//! Подагенты — по форме muse `subagents_tool/*` (`source-files.txt:1757-1762`),
//! `subagent_execution.rs`, `subagent_completion_stream.rs` (`:1352-1353`).
//!
//! Подагент — тот же цикл хода со своей историей и своим набором
//! инструментов. Работа асинхронная, как велят описания инструментов muse:
//! `subagent.spawn` сразу отдаёт id, итог приходит родителю сам — в его
//! ящик (`Inbox`), а оттуда заметкой в историю перед следующим вызовом
//! модели. Сессия подагента живёт и после итога: `subagent.send` даёт ей
//! новую работу. Между перезапусками ничего не хранится (П3, карточка).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};

use crate::compaction::personal;
use crate::prompt::{Conversation, Prefix};
use crate::tool::{Tool, ToolError, ToolFuture, Tools};
use crate::turn::{Turn, TurnError};

/// Срок команды подагенту — 90 с (`JARVIS_SUBAGENT_CONTROL_OP_TIMEOUT_MS`;
/// опись, таблица 4.2).
pub const CONTROL_TIMEOUT: Duration = Duration::from_secs(90);

/// Глубина: подагенты своих подагентов не заводят.
pub const MAX_SPAWN_DEPTH: u32 = 1;

/// Тексты muse, перенесены как есть (`prompts/subagent/`).
pub const USER_MESSAGE_MD: &str = include_str!("../../../prompts/subagent/subagent_context_user_message.md");
pub const STANDARD_MD: &str = include_str!("../../../prompts/subagent/subagent_context_standard.md");
pub const PERSISTENT_SESSION_MD: &str =
    include_str!("../../../prompts/subagent/subagent_context_persistent_session.md");
pub const ISOLATED_MD: &str = include_str!("../../../prompts/subagent/subagent_history_inheritance_isolated.md");

/// Описания инструментов — строки ядра muse (0x2cdb1b3–0x2cdd367), полные
/// варианты: первая строка и абзацы через пустую строку.
const LIST_DESCRIPTION: &str = "List your child agents and the agents of <!-- personal-muse-only:start -->the user's<!-- personal-muse-only:end --><!-- group-muse-only:start -->this Muse's<!-- group-muse-only:end --> chats and scheduled jobs, grouped into active and recent, with a normalized status (running, done, interrupted, failed), task and final response previews, timing, and whether a finished result was already seen.\n\nRunning entries include how long the agent has been running and how long since its last activity. A quiet agent is not proof it is stuck; it may be inside one long tool call. Entries carry the agent ids the other subagent tools need, plus the browser task id for browser task owners and the artifact slug for artifact builders. An entry a chat or a scheduled job owns carries `owner`, saying which, with the schedule and run ids for scheduled work, so a scheduled job's agents are never mistaken for missing. You can view agents owned by another chat or a scheduled job, but the subagent tools cannot steer or stop them: a scheduled job's in-flight run stops with the cron stop tool and the schedule id shown in `owner`, and another chat's agents cannot be closed from here at all.\n\nCheck status only to act on it: <!-- personal-muse-only:start -->the user<!-- personal-muse-only:end --><!-- group-muse-only:start -->a member<!-- group-muse-only:end --> asks where things stand, you need an agent id, or you suspect slow or failed work. Finished results are delivered to you automatically; mentioning running work unprompted needs no fresh check, and calling this in a polling loop wastes turns.";
const LIST_SCOPE: &str = "'direct' (default): your immediate children. 'descendants': the full subtree of agents under you.";
const LIST_LIMIT: &str = "Max entries to return. Default 20, max 200.";
const SEND_DESCRIPTION: &str = "Deliver follow-up input to one of your running child agents by id.\n\nDelivery is ASYNC: a success response means the message was submitted, not applied. The instruction stays pending until the agent's handoff or completion result reports it, so do not describe it as done, do not poll for it, and do not fabricate the outcome. Handle anything else the turn needs, then end your response.\n\nProvide the input in `message`, or in `items` for images or files; `items` takes precedence when both are set. Set `interrupt` true to cut off the agent's current work first, so the new input starts a fresh turn instead of queueing behind the old one.\n\nBrowser task owners cannot be steered by agent id; their parent agent continues the task through its browser tools instead. To start ordinary delegated work when no suitable agent exists, use `subagent.spawn`.";
const TARGET_ID: &str = "Target agent id (from subagent.spawn or subagent.list output).";
const SEND_MESSAGE: &str = "Follow-up text for the agent. If items is also provided, items takes precedence.";
const SEND_SCOPE: &str = "Where the target may live: 'direct' (default) covers your immediate children, 'descendants' the full subtree under you.";
const SEND_INTERRUPT: &str = "If true, interrupt the agent's current work before delivering this input.";
const CLOSE_DESCRIPTION: &str = "Stop one of your child agents by id and end its work.\n\nUse this when the delegated work is no longer needed, the agent is stuck, or it should not keep going. To redirect an agent instead of stopping it, use `subagent.send`; to find the right id and check state first, use `subagent.list`.\n\nClosing a browser task owner stops the whole browser task, and the response includes its `browser_task_id`. If the owner's task identity cannot be recovered, the close fails and nothing is stopped.";
const SPAWN_DESCRIPTION: &str = "Delegate a task to a new child agent that works on it in the background while you continue.\n\nDo not spawn a generic child for requested website navigation, purchases, or other live-browser work: logged-in sites, clicking, forms, or multi-step interactions. That work belongs to the browser-task delegation route, not another generic child. Generic subagents may search the public web and fetch page text from links supplied by <!-- personal-muse-only:start -->the user<!-- personal-muse-only:end --><!-- group-muse-only:start -->members<!-- group-muse-only:end --> or returned by tools; requests to visit or interact with websites are returned to the parent for appropriate delegation.\n\nThis call is ASYNC: the response only confirms the spawn and returns the new agent id. Results arrive later through the runtime, so never treat the response as completion and never produce results for delegated work yourself. After spawning, acknowledge what you delegated and end your response; do not poll, sleep, or promise a delivery time. Spawn all independent agents at once instead of one at a time.\n\nProvide the task in `message`, or in `items` when it needs images or files; `items` takes precedence when both are set. Manage the agent afterwards by its id: `subagent.list` shows status, `subagent.send` delivers follow-up input, `subagent.resume` restarts an interrupted one, and `subagent.close` stops one.\n\nSome work has its own dedicated parent-agent capability and cannot be spawned here, including browser tasks, artifacts (a page, app, document, deck, or spreadsheet), and accepted Idea builds. Artifacts do not need a subagent: the artifact tools already run the build in the background and report back. If <!-- personal-muse-only:start -->the user<!-- personal-muse-only:end --><!-- group-muse-only:start -->a member<!-- group-muse-only:end --> asks for a subagent, tell the child to use the artifact tools instead of creating a file itself. A rejected spawn returns a structured error with a stable reason code; fix the cause it names instead of retrying the same call.";
const SPAWN_MESSAGE: &str = "Task instruction for the child. State the task, the outcome you want, any non-obvious constraint, and every fact it needs. If items is also provided, items takes precedence.";
const RESUME_DESCRIPTION: &str = "Resume one of your interrupted child agents by id so it continues its existing task from where it stopped.\n\nOnly a paused or interrupted agent can be resumed. An agent that already finished or shut down is not resumed; the call returns an error saying so, because resuming a terminal agent re-runs nothing. Spawn a new agent with `subagent.spawn` to continue that work. Resume restarts the agent on its own task; to give it new direction once it is running again, use `subagent.send`.\n\nTwo kinds of agents refuse generic resume: browser task owners continue through their parent agent's dedicated browser-task control, and a web artifact builder whose build is still in flight cannot be resumed; wait for it to finish or send it guidance with `subagent.send`.";

/// Состояние подагента — нормализованные статусы muse (running, done,
/// interrupted, failed) и закрытый.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Done,
    /// Ход прервался временной ошибкой модели; можно `subagent.resume`.
    Interrupted,
    Failed,
    Closed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Done => "done",
            Status::Interrupted => "interrupted",
            Status::Failed => "failed",
            Status::Closed => "closed",
        }
    }
}

/// Ящик родителя: итоги подагентов ждут здесь следующего вызова модели.
#[derive(Clone, Default)]
pub struct Inbox {
    queue: Arc<Mutex<VecDeque<Message>>>,
    notify: Arc<Notify>,
}

impl Inbox {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&self, m: Message) {
        self.queue.lock().unwrap().push_back(m);
        self.notify.notify_one();
    }

    /// Забрать всё дошедшее.
    pub fn drain(&self) -> Vec<Message> {
        self.queue.lock().unwrap().drain(..).collect()
    }

    /// Дождаться, пока что-нибудь дойдёт (для начала нового хода родителя,
    /// когда его ход уже кончился, а подагенты ещё работали).
    pub async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            if !self.queue.lock().unwrap().is_empty() {
                return;
            }
            notified.await;
        }
    }
}

enum Input {
    Message(String),
    Resume,
}

struct Agent {
    id: String,
    task: String,
    status: Status,
    result: Option<String>,
    tx: mpsc::UnboundedSender<Input>,
    interrupt: Arc<Notify>,
    handle: tokio::task::JoinHandle<()>,
}

struct Inner<P> {
    provider: Arc<P>,
    tools: Arc<Tools>,
    prefix: Prefix,
    spill_dir: std::path::PathBuf,
    inbox: Inbox,
    agents: Mutex<Vec<Agent>>,
}

/// Подагенты одного родителя. Инструменты `subagent.*` — из `tools()`.
pub struct Subagents<P> {
    inner: Arc<Inner<P>>,
}

impl<P> Clone for Subagents<P> {
    fn clone(&self) -> Self {
        Subagents { inner: self.inner.clone() }
    }
}

/// Первое сообщение подагента — `subagent_context_user_message.md` muse,
/// личный вариант. Подагент изолирован: истории родителя не наследует, и
/// глубже не спавнит (`MAX_SPAWN_DEPTH` = 1), поэтому блоки глубины пусты.
pub fn first_message(task: &str) -> String {
    let standard = personal(STANDARD_MD)
        .replace("{{DEPTH_ONE_COORDINATOR_BLOCK}}\n", "")
        .replace("{{DEPTH_ONE_SPAWN_INHERITANCE_BLOCK}}\n", "");
    personal(USER_MESSAGE_MD)
        .replace("{{CHILD_DEPTH}}", "1")
        .replace("{{MAX_SPAWN_DEPTH}}", &MAX_SPAWN_DEPTH.to_string())
        .replace("{{PARENT_CONTEXT_BLOCK}}", personal(ISOLATED_MD).trim_end())
        .replace("{{STANDARD_SUBAGENT_BLOCK}}", &standard)
        .replace("{{PERSISTENT_SESSION_BLOCK}}", &personal(PERSISTENT_SESSION_MD))
        .replace("{{SESSION_LINES}}\n", "")
        .replace("{{METADATA_BLOCK}}\n", "")
        .replace("{{TASK_INPUT_START}}", task)
}

/// Итог подагента для родителя. Формы заметки muse в разборе нет — своя.
fn handoff(id: &str, status: Status, text: &str) -> Message {
    Message {
        role: Role::Developer,
        content: vec![Part::Text {
            text: format!("[Subagent Result] agent_id: {id}, status: {}\n\n{text}", status.as_str()),
        }],
    }
}

/// После прерванного хода у вызовов без результата появляется результат с
/// ошибкой — история остаётся годной для поставщика.
fn close_dangling_calls(history: &mut Vec<Message>) {
    let Some(last) = history.last() else { return };
    if last.role != Role::Assistant {
        return;
    }
    let results: Vec<Part> = last
        .content
        .iter()
        .filter_map(|p| match p {
            Part::ToolCall { id, .. } => Some(Part::ToolResult {
                call_id: id.clone(),
                content: vec![ToolResultPart::Text { text: "interrupted before the tool finished".into() }],
                is_error: true,
            }),
            _ => None,
        })
        .collect();
    if !results.is_empty() {
        history.push(Message { role: Role::User, content: results });
    }
}

impl<P: Provider + Send + Sync + 'static> Subagents<P> {
    /// `prefix` и `tools` — неизменное начало и набор инструментов подагента.
    pub fn new(provider: Arc<P>, tools: Arc<Tools>, prefix: Prefix, spill_dir: impl Into<std::path::PathBuf>, inbox: Inbox) -> Self {
        Subagents {
            inner: Arc::new(Inner { provider, tools, prefix, spill_dir: spill_dir.into(), inbox, agents: Mutex::new(Vec::new()) }),
        }
    }

    pub fn inbox(&self) -> &Inbox {
        &self.inner.inbox
    }

    /// Инструменты `subagent.spawn/send/list/close/resume` для родителя.
    pub fn tools(&self) -> Vec<SubagentTool<P>> {
        [Op::Spawn, Op::Send, Op::List, Op::Close, Op::Resume]
            .into_iter()
            .map(|op| SubagentTool { op, agents: self.clone() })
            .collect()
    }

    pub fn status(&self, id: &str) -> Option<Status> {
        self.inner.agents.lock().unwrap().iter().find(|a| a.id == id).map(|a| a.status)
    }

    fn set(inner: &Inner<P>, id: &str, status: Status, result: Option<String>) {
        if let Some(a) = inner.agents.lock().unwrap().iter_mut().find(|a| a.id == id) {
            if a.status == Status::Closed {
                return;
            }
            a.status = status;
            if result.is_some() {
                a.result = result;
            }
        }
    }

    pub fn spawn(&self, task: &str) -> String {
        let mut agents = self.inner.agents.lock().unwrap();
        let id = format!("agent-{}", agents.len() + 1);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let interrupt = Arc::new(Notify::new());
        let _ = tx.send(Input::Message(first_message(task)));
        let inner = self.inner.clone();
        let stop = interrupt.clone();
        let agent_id = id.clone();
        let handle = tokio::spawn(async move {
            let mut conv = Conversation::new(inner.prefix.clone());
            while let Some(input) = rx.recv().await {
                if let Input::Message(text) = input {
                    conv.history.push(Message { role: Role::User, content: vec![Part::Text { text }] });
                }
                Self::set(&inner, &agent_id, Status::Running, None);
                let turn = Turn::from_prefix(&*inner.provider, &inner.tools, &inner.prefix, inner.spill_dir.clone());
                let mut quiet = |_: &str| {};
                let outcome = tokio::select! {
                    r = turn.run(&mut conv.history, &mut quiet) => Some(r),
                    _ = stop.notified() => None,
                };
                let Some(outcome) = outcome else {
                    // Прервано `send` с `interrupt`: новая работа уже в очереди.
                    close_dangling_calls(&mut conv.history);
                    continue;
                };
                if !rx.is_empty() {
                    // Пришла новая работа — итог отдаётся после неё.
                    continue;
                }
                let (status, text) = match outcome {
                    Ok(resp) => (Status::Done, resp.text()),
                    Err(TurnError::Model(e)) if e.retryable => (Status::Interrupted, e.to_string()),
                    Err(e) => (Status::Failed, e.to_string()),
                };
                Self::set(&inner, &agent_id, status, Some(text.clone()));
                inner.inbox.push(handoff(&agent_id, status, &text));
            }
        });
        agents.push(Agent { id: id.clone(), task: task.to_string(), status: Status::Running, result: None, tx, interrupt, handle });
        id
    }

    pub fn send(&self, id: &str, message: &str, interrupt: bool) -> Result<(), String> {
        let agents = self.inner.agents.lock().unwrap();
        let a = agents.iter().find(|a| a.id == id).ok_or_else(|| unknown(id))?;
        if a.status == Status::Closed {
            return Err(format!("agent {id} is closed"));
        }
        a.tx.send(Input::Message(message.to_string())).map_err(|_| format!("agent {id} is gone"))?;
        if interrupt {
            a.interrupt.notify_one();
        }
        Ok(())
    }

    pub fn resume(&self, id: &str) -> Result<(), String> {
        let agents = self.inner.agents.lock().unwrap();
        let a = agents.iter().find(|a| a.id == id).ok_or_else(|| unknown(id))?;
        if a.status != Status::Interrupted {
            return Err(format!(
                "agent {id} is {}: only a paused or interrupted agent can be resumed; spawn a new agent with `subagent.spawn` to continue that work",
                a.status.as_str()
            ));
        }
        a.tx.send(Input::Resume).map_err(|_| format!("agent {id} is gone"))
    }

    /// Остановить подагента и дождаться конца его работы — не дольше срока
    /// команды.
    pub async fn close(&self, id: &str) -> Result<(), String> {
        let handle = {
            let mut agents = self.inner.agents.lock().unwrap();
            let a = agents.iter_mut().find(|a| a.id == id).ok_or_else(|| unknown(id))?;
            a.status = Status::Closed;
            a.handle.abort();
            std::mem::replace(&mut a.handle, tokio::spawn(async {}))
        };
        match tokio::time::timeout(CONTROL_TIMEOUT, handle).await {
            Ok(_) => Ok(()),
            Err(_) => Err(format!("closing agent {id} did not finish within {CONTROL_TIMEOUT:?}")),
        }
    }

    pub fn list(&self, limit: usize) -> Value {
        let agents = self.inner.agents.lock().unwrap();
        let entry = |a: &Agent| {
            json!({
                "agent_id": a.id,
                "status": a.status.as_str(),
                "task": preview(&a.task),
                "result": a.result.as_deref().map(preview),
            })
        };
        let (active, recent): (Vec<&Agent>, Vec<&Agent>) = agents.iter().partition(|a| a.status == Status::Running);
        json!({
            "active": active.into_iter().take(limit).map(entry).collect::<Vec<_>>(),
            "recent": recent.into_iter().take(limit).map(entry).collect::<Vec<_>>(),
        })
    }
}

fn unknown(id: &str) -> String {
    format!("no child agent with id `{id}`")
}

fn preview(s: &str) -> String {
    const MAX: usize = 200;
    match s.char_indices().nth(MAX) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

#[derive(Clone, Copy)]
enum Op {
    Spawn,
    Send,
    List,
    Close,
    Resume,
}

/// Инструмент `subagent.*`.
pub struct SubagentTool<P> {
    op: Op,
    agents: Subagents<P>,
}

fn prop(d: &str, t: &str) -> Value {
    json!({"type": t, "description": d})
}

impl<P: Provider + Send + Sync + 'static> Tool for SubagentTool<P> {
    fn spec(&self) -> ToolSpec {
        let (name, description, props, required) = match self.op {
            Op::Spawn => ("subagent.spawn", SPAWN_DESCRIPTION, json!({"message": prop(SPAWN_MESSAGE, "string")}), json!(["message"])),
            Op::Send => (
                "subagent.send",
                SEND_DESCRIPTION,
                json!({
                    "agent_id": prop(TARGET_ID, "string"),
                    "message": prop(SEND_MESSAGE, "string"),
                    "scope": {"type": "string", "enum": ["direct", "descendants"], "description": SEND_SCOPE},
                    "interrupt": prop(SEND_INTERRUPT, "boolean"),
                }),
                json!(["agent_id", "message"]),
            ),
            Op::List => (
                "subagent.list",
                LIST_DESCRIPTION,
                json!({
                    "scope": {"type": "string", "enum": ["direct", "descendants"], "description": LIST_SCOPE},
                    "limit": prop(LIST_LIMIT, "integer"),
                }),
                json!([]),
            ),
            Op::Close => ("subagent.close", CLOSE_DESCRIPTION, json!({"agent_id": prop(TARGET_ID, "string")}), json!(["agent_id"])),
            Op::Resume => ("subagent.resume", RESUME_DESCRIPTION, json!({"agent_id": prop(TARGET_ID, "string")}), json!(["agent_id"])),
        };
        let mut props = props;
        for p in props.as_object_mut().unwrap().values_mut() {
            if let Some(d) = p.get("description").and_then(Value::as_str) {
                p["description"] = personal(d).into();
            }
        }
        ToolSpec {
            name: name.into(),
            description: personal(description),
            input_schema: json!({"type": "object", "properties": props, "required": required}),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let s = |k: &str| args.get(k).and_then(Value::as_str);
            let need = |k: &str| s(k).ok_or_else(|| ToolError::InvalidArguments(format!("`{k}` (string) is required")));
            let text = |v: Value| Ok(vec![ToolResultPart::Text { text: v.to_string() }]);
            match self.op {
                Op::Spawn => {
                    let id = self.agents.spawn(need("message")?);
                    text(json!({"agent_id": id, "status": "running"}))
                }
                Op::Send => {
                    let interrupt = args.get("interrupt").and_then(Value::as_bool).unwrap_or(false);
                    let id = need("agent_id")?;
                    self.agents.send(id, need("message")?, interrupt).map_err(ToolError::Failed)?;
                    text(json!({"agent_id": id, "submitted": true}))
                }
                Op::List => {
                    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20).clamp(1, 200) as usize;
                    text(self.agents.list(limit))
                }
                Op::Close => {
                    let id = need("agent_id")?;
                    self.agents.close(id).await.map_err(ToolError::Failed)?;
                    text(json!({"agent_id": id, "status": "closed"}))
                }
                Op::Resume => {
                    let id = need("agent_id")?;
                    self.agents.resume(id).map_err(ToolError::Failed)?;
                    text(json!({"agent_id": id, "status": "running"}))
                }
            }
        })
    }
}
