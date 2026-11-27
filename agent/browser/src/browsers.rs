//! Окна главного агента — `browser.browse` и `browser.windows` muse
//! (`tool-schemas.txt:2-23`, описания `strings-all.txt` около смещений
//! `75b11b`–`75c0a0` и `75dd3a`–`75ea7c`; тексты — `prompts/browser/main/`).
//!
//! Окно — задача браузера с `task_id` вида `browser-task:<uuid>` (L55758) и
//! своими вкладками; окна живут между командами главного агента. Работу
//! ведёт исполнитель (`planner.rs`) в фоне: приём — квитанция и текст
//! «The browser has accepted the request…» (L78793), итог — заметка
//! «## Browser Task Handoff» в ящик главного агента (L225912-225945),
//! вопрос исполнителя — заметка «A browser task is waiting for your
//! answer…» с `question_id` вида `question_<16 hex>` (L58890); ответ
//! главного агента — `browser.browse` с `answer_to`, обновление без него
//! уходит исполнителю между шагами, а простаивающее окно начинает с ним
//! новую работу («Continue this task from the history above…»). Квитанции —
//! строки ядра: «A new window (task_id …) accepted the request», «The
//! window (task_id …) accepted the request as new work», «Sent to the
//! window (task_id …) as an update», «Answered … in the window (task_id …»
//! (L10660, L15254, L9554). Простаивающее окно закрывается примерно через
//! два часа («An idle window closes after about two hours»).
//!
//! Своё: вид списка окон и метаданных итога, вопрос — блок полей после
//! вводного текста, закрытие простаивающих окон — при обращении к окнам
//! (своего таймера нет), один Chromium на профиль, профиль — папка
//! `<root>/profiles/<id>`. Не взяты в П4к-1б: сводка `<browser_status>` в
//! каждом ходе, `browser.profiles` (создание профилей), окна расписаний
//! (`owner`), вход, коды, файлы, кошелёк, выбор браузера устройства.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use agent_loop::{personal, Inbox, Tool, ToolError, ToolFuture};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use rand::Rng;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::planner::{assignment_message, update_message, Asker, Planner, Question, Report};
use crate::window::{Browser, Window};

const BROWSE_MD: &str = include_str!("../../../prompts/browser/main/browse.md");
const BROWSE_JSON: &str = include_str!("../../../prompts/browser/main/browse.json");
const WINDOWS_MD: &str = include_str!("../../../prompts/browser/main/windows.md");
const WINDOWS_JSON: &str = include_str!("../../../prompts/browser/main/windows.json");
const ACCEPTED_MD: &str = include_str!("../../../prompts/browser/main/accepted.md");
const HANDOFF_MD: &str = include_str!("../../../prompts/browser/main/handoff.md");
const QUESTION_MD: &str = include_str!("../../../prompts/browser/main/question.md");
const OUTCOME_COMPLETED_MD: &str = include_str!("../../../prompts/browser/main/outcome_completed.md");
const OUTCOME_NEEDS_INPUT_MD: &str = include_str!("../../../prompts/browser/main/outcome_needs_input.md");
const OUTCOME_BLOCKED_MD: &str = include_str!("../../../prompts/browser/main/outcome_blocked.md");

pub const BROWSE: &str = "browser.browse";
pub const WINDOWS: &str = "browser.windows";
pub const DEFAULT_PROFILE: &str = "default";

/// «An idle window closes after about two hours.»
pub const IDLE_CLOSE: Duration = Duration::from_secs(2 * 60 * 60);

struct Waiting {
    id: String,
    tx: oneshot::Sender<String>,
}

struct Task {
    id: String,
    profile: String,
    assignment: String,
    window: Arc<Window>,
    /// Обновления для идущей работы.
    updates: Inbox,
    history: tokio::sync::Mutex<Vec<Message>>,
    /// Идёт ли работа исполнителя; под этим же замком решается, куда
    /// попадёт обновление, чтобы оно не потерялось на стыке.
    running: Mutex<bool>,
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    questions: Mutex<Vec<Waiting>>,
    last_active: Mutex<Instant>,
}

/// Окна главного агента.
pub struct Browsers<P> {
    pub bin: PathBuf,
    /// Папка браузера: профили — `profiles/<id>`.
    pub root: PathBuf,
    pub headless: bool,
    /// Модель исполнителя — `planner::model` (`BrowserAgent`).
    pub provider: Arc<P>,
    pub locale: String,
    pub spill_dir: PathBuf,
    /// Ящик главного агента: итоги и вопросы окон.
    pub inbox: Inbox,
    profiles: tokio::sync::Mutex<HashMap<String, Arc<Browser>>>,
    tasks: Mutex<Vec<Arc<Task>>>,
}

fn new_task_id() -> String {
    let b: [u8; 16] = rand::thread_rng().gen();
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("browser-task:{}-{}-4{}-{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[16..20], &h[20..32])
}

fn new_question_id() -> String {
    let b: [u8; 8] = rand::thread_rng().gen();
    format!("question_{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

fn note(text: String) -> Message {
    Message { role: Role::Developer, content: vec![Part::Text { text }] }
}

fn rfc3339_now() -> String {
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (d, s) = ((secs / 86_400) as i64, secs % 86_400);
    // Гражданская дата из дней от 1970-01-01 (алгоритм Хиннанта).
    let z = d + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
}

/// Заметка-итог окна для главного агента.
pub fn handoff(task_id: &str, r: &Report, ran: Duration) -> Message {
    let guidance = match r.status.as_str() {
        "completed" => OUTCOME_COMPLETED_MD,
        "needs_input" => OUTCOME_NEEDS_INPUT_MD,
        _ => OUTCOME_BLOCKED_MD,
    };
    let mut meta = vec![format!("browser_task_id: {task_id}"), format!("status: {}", r.status)];
    if let Some(reason) = &r.reason {
        meta.push(format!("reason: {reason}"));
    }
    meta.push(format!("ran: {}s (harness-measured)", ran.as_secs()));
    meta.push(format!("completed_at: {}", rfc3339_now()));
    let mut report = r.summary.clone();
    if !r.products.is_empty() {
        report.push_str(&format!("\n\nproducts: {}", Value::Array(r.products.clone())));
    }
    let text = personal(HANDOFF_MD)
        .replace("{{BROWSER_TASK_METADATA}}", &meta.join("\n"))
        .replace("{{BROWSER_TASK_OUTCOME_GUIDANCE}}", personal(guidance).trim_end())
        .replace("{{BROWSER_TASK_REPORT}}", &report);
    note(text.trim_end().to_string())
}

struct TaskAsker {
    task: Arc<Task>,
    inbox: Inbox,
}

impl Asker for TaskAsker {
    fn ask<'a>(&'a self, q: Question) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let id = new_question_id();
            let (tx, rx) = oneshot::channel();
            self.task.questions.lock().unwrap().push(Waiting { id: id.clone(), tx });
            let mut block = vec![format!("task_id: {}", self.task.id), format!("question_id: {id}")];
            if let Some(r) = &q.reason {
                block.push(format!("reason: {r}"));
            }
            block.push(format!("question: {}", q.question));
            if let Some(c) = &q.context {
                block.push(format!("context: {c}"));
            }
            block.push(format!("original assignment:\n{}", self.task.assignment));
            self.inbox.push(note(personal(QUESTION_MD).replace("{question}", &block.join("\n")).trim_end().to_string())).await;
            rx.await.map_err(|_| "This browser has closed and cannot accept more work.".to_string())
        })
    }
}

fn str_arg<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

impl<P: Provider + Send + Sync + 'static> Browsers<P> {
    pub fn new(bin: PathBuf, root: PathBuf, headless: bool, provider: Arc<P>, locale: String, spill_dir: PathBuf, inbox: Inbox) -> Arc<Self> {
        Arc::new(Browsers { bin, root, headless, provider, locale, spill_dir, inbox, profiles: Default::default(), tasks: Default::default() })
    }

    /// Инструменты главного агента.
    pub fn tools(self: &Arc<Self>) -> (BrowseTool<P>, WindowsTool<P>) {
        (BrowseTool(self.clone()), WindowsTool(self.clone()))
    }

    fn profiles_on_disk(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.root.join("profiles"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != DEFAULT_PROFILE)
            .collect();
        v.sort();
        v.insert(0, DEFAULT_PROFILE.into());
        v
    }

    async fn browser(&self, profile: &str) -> Result<Arc<Browser>, String> {
        if profile != DEFAULT_PROFILE && !self.profiles_on_disk().iter().any(|p| p == profile) {
            return Err(format!("{profile} is not a browser profile. Use a profile ID from browser.windows list."));
        }
        let mut map = self.profiles.lock().await;
        if let Some(b) = map.get(profile) {
            return Ok(b.clone());
        }
        let b = Browser::launch(&self.bin, &self.root.join("profiles").join(profile), self.headless).await?;
        map.insert(profile.into(), b.clone());
        Ok(b)
    }

    fn find(&self, id: &str) -> Result<Arc<Task>, String> {
        self.tasks.lock().unwrap().iter().find(|t| t.id == id).cloned().ok_or_else(|| format!("No window has task_id {id}. Check browser.windows list."))
    }

    /// Закрыть окна, простаивающие дольше `IDLE_CLOSE`.
    async fn close_idle(&self) {
        let idle: Vec<Arc<Task>> = self.tasks.lock().unwrap().iter().filter(|t| !*t.running.lock().unwrap() && t.last_active.lock().unwrap().elapsed() > IDLE_CLOSE).cloned().collect();
        for t in idle {
            self.close_task(&t).await;
        }
    }

    async fn close_task(&self, t: &Arc<Task>) {
        if let Some(h) = t.handle.lock().unwrap().take() {
            h.abort();
        }
        t.questions.lock().unwrap().clear();
        t.window.close().await;
        self.tasks.lock().unwrap().retain(|x| x.id != t.id);
    }

    async fn open_window(&self, profile: &str, url: Option<&str>, assignment: &str) -> Result<Arc<Task>, String> {
        let browser = self.browser(profile).await?;
        let window = Window::new_in(browser).await?;
        if let Some(u) = url {
            window.act("t1", &[json!({"kind": "navigate", "url": u})]).await?;
        }
        let t = Arc::new(Task {
            id: new_task_id(),
            profile: profile.into(),
            assignment: assignment.into(),
            window,
            updates: Inbox::new(),
            history: Default::default(),
            running: Mutex::new(false),
            handle: Mutex::new(None),
            questions: Mutex::new(vec![]),
            last_active: Mutex::new(Instant::now()),
        });
        self.tasks.lock().unwrap().push(t.clone());
        Ok(t)
    }

    /// Запустить работу исполнителя; `running` уже стоит.
    fn start(self: &Arc<Self>, t: Arc<Task>, first: String) {
        let me = self.clone();
        let task = t.clone();
        let h = tokio::spawn(async move {
            let mut message = first;
            loop {
                let planner = Planner {
                    provider: me.provider.clone(),
                    window: task.window.clone(),
                    asker: Arc::new(TaskAsker { task: task.clone(), inbox: me.inbox.clone() }),
                    locale: me.locale.clone(),
                    spill_dir: me.spill_dir.clone(),
                    updates: Some(task.updates.clone()),
                };
                let started = Instant::now();
                let result = {
                    let mut history = task.history.lock().await;
                    planner.run(&mut history, message).await
                };
                let report = result.unwrap_or_else(|e| Report { status: "blocked".into(), summary: format!("The browser failed: {e}"), reason: Some("browser_failure".into()), products: vec![] });
                me.inbox.push(handoff(&task.id, &report, started.elapsed())).await;
                *task.last_active.lock().unwrap() = Instant::now();
                // Обновления, пришедшие на стыке, — следующая работа.
                let mut running = task.running.lock().unwrap();
                let rest = task.updates.drain();
                if rest.is_empty() {
                    *running = false;
                    break;
                }
                message = rest.iter().map(Message::text).collect::<Vec<_>>().join("\n\n");
            }
        });
        *t.handle.lock().unwrap() = Some(h);
    }

    pub async fn browse(self: &Arc<Self>, args: &Value) -> Result<String, String> {
        self.close_idle().await;
        let task_id = str_arg(args, "task_id").ok_or("task_id is required")?;
        let text = str_arg(args, "task").ok_or("task is required")?;
        let accepted = personal(ACCEPTED_MD).trim_end().to_string();
        if task_id == "new" {
            if args.get("answer_to").is_some() || args.get("tab").is_some() {
                return Err("answer_to and tab need an existing window's task_id".into());
            }
            let profile = str_arg(args, "profile").unwrap_or(DEFAULT_PROFILE);
            let t = self.open_window(profile, str_arg(args, "url"), text).await?;
            *t.running.lock().unwrap() = true;
            self.start(t.clone(), assignment_message(text));
            return Ok(format!("A new window (task_id {}) accepted the request.\n\n{accepted}", t.id));
        }
        let t = self.find(task_id)?;
        *t.last_active.lock().unwrap() = Instant::now();
        if let Some(q) = str_arg(args, "answer_to") {
            let w = {
                let mut qs = t.questions.lock().unwrap();
                let i = qs.iter().position(|w| w.id == q).ok_or_else(|| format!("{q} is not a question waiting in the window (task_id {task_id})."))?;
                qs.remove(i)
            };
            let _ = w.tx.send(text.to_string());
            return Ok(format!("Answered {q} in the window (task_id {task_id})."));
        }
        let mut body = text.to_string();
        if let Some(tab) = str_arg(args, "tab") {
            body = format!("Work in tab {tab}.\n\n{body}");
        }
        // Идёт работа — весть-обновление; нет — окно начинает новую работу.
        let was_running = {
            let mut running = t.running.lock().unwrap();
            std::mem::replace(&mut *running, true)
        };
        if was_running {
            t.updates.push(Message { role: Role::User, content: vec![Part::Text { text: update_message(&body) }] }).await;
            let waiting: Vec<String> = t.questions.lock().unwrap().iter().map(|w| w.id.clone()).collect();
            let mut out = format!("Sent to the window (task_id {task_id}) as an update.");
            if !waiting.is_empty() {
                out.push_str(&format!(
                    "\n\nWaiting questions: {}\nThe questions listed in the receipt still need answers. Answer one with answer_to set to its question_id. Sending ordinary instructions does not answer a question.",
                    waiting.join(", ")
                ));
            }
            return Ok(format!("{out}\n\n{accepted}"));
        }
        self.start(t.clone(), update_message(&body));
        Ok(format!("The window (task_id {task_id}) accepted the request as new work.\n\n{accepted}"))
    }

    pub async fn windows(self: &Arc<Self>, args: &Value) -> Result<Vec<ToolResultPart>, String> {
        self.close_idle().await;
        let text = |t: String| vec![ToolResultPart::Text { text: t }];
        match str_arg(args, "action").unwrap_or_default() {
            "list" => {
                let tasks: Vec<Arc<Task>> = self.tasks.lock().unwrap().clone();
                let mut out = vec!["## Browser windows".to_string(), format!("Profiles: {}", self.profiles_on_disk().join(", "))];
                if tasks.is_empty() {
                    out.push("No windows are open.".into());
                }
                for t in tasks {
                    let status = if !t.questions.lock().unwrap().is_empty() {
                        "waiting_on_root"
                    } else if *t.running.lock().unwrap() {
                        "running"
                    } else {
                        "idle"
                    };
                    out.push(format!("\n### Window {}\nstatus: {status}\nprofile: {}", t.id, t.profile));
                    let tabs = t.window.tab_lines().await;
                    if tabs.is_empty() {
                        out.push("It has no open tabs.".into());
                    }
                    out.extend(tabs);
                }
                Ok(text(out.join("\n")))
            }
            "open" => match str_arg(args, "task_id") {
                None => {
                    let t = self.open_window(str_arg(args, "profile").unwrap_or(DEFAULT_PROFILE), str_arg(args, "url"), "").await?;
                    Ok(text(format!("Opened a new window (task_id {}). It starts no browser planner; send work with browser.browse and this task_id.\n{}", t.id, t.window.tab_lines().await.join("\n"))))
                }
                Some(id) => {
                    let t = self.find(id)?;
                    t.window.tabs("open", str_arg(args, "url"), None, None).await
                }
            },
            "close" => {
                let id = str_arg(args, "task_id").ok_or("close needs task_id")?;
                let t = self.find(id)?;
                match str_arg(args, "tab") {
                    Some(tab) => t.window.tabs("close", None, Some(tab), None).await,
                    None => {
                        self.close_task(&t).await;
                        Ok(text(format!("Closed the window (task_id {id}) and stopped its browser task.")))
                    }
                }
            }
            other => Err(format!("unknown action `{other}`")),
        }
    }

    /// Закрыть все окна и браузеры (остановка агента, тесты).
    pub async fn shutdown(&self) {
        let tasks: Vec<Arc<Task>> = self.tasks.lock().unwrap().clone();
        for t in tasks {
            self.close_task(&t).await;
        }
        for (_, b) in self.profiles.lock().await.drain() {
            b.close().await;
        }
    }
}

pub struct BrowseTool<P>(Arc<Browsers<P>>);
pub struct WindowsTool<P>(Arc<Browsers<P>>);

impl<P: Provider + Send + Sync + 'static> Tool for BrowseTool<P> {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: BROWSE.into(), description: personal(BROWSE_MD).trim_end().into(), input_schema: serde_json::from_str(&personal(BROWSE_JSON)).expect("browse.json") }
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let out = self.0.browse(&args).await.map_err(ToolError::Failed)?;
            Ok(vec![ToolResultPart::Text { text: out }])
        })
    }
}

impl<P: Provider + Send + Sync + 'static> Tool for WindowsTool<P> {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: WINDOWS.into(), description: personal(WINDOWS_MD).trim_end().into(), input_schema: serde_json::from_str(WINDOWS_JSON).expect("windows.json") }
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move { self.0.windows(&args).await.map_err(ToolError::Failed) })
    }
}
