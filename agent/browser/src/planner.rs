//! Исполнитель окна — «browser planner» muse: отдельный агент на модели
//! задачи `BrowserAgent` со своим системным текстом — профиль 1 ядра
//! (`prompts/browser/system.md`, `pages.md`) — и инструментами `browser.observe`,
//! `browser.act`, `browser.tabs`, `browser.request_input`, `browser.report`
//! (`prompts/browser/tools/*.json|md`). Работа кончается принятым
//! `browser.report`; ответ без него возвращается к модели с текстом
//! `completion_gate/report_required.md` («a reply alone does not close the
//! task»).
//!
//! Первое сообщение и продолжение — сборка задания `0x8b1f310`: строка
//! «Carry out this assignment end to end, then report the result.» или
//! «Continue this task from the history above with Muse's new instruction
//! below, then report the result.» (`0x539613`, `0x539652`; Muse → Hamlet),
//! задание, «\n\nLanguage:\n» и `presentation_language.md` с подставленным
//! `{locale}` (`0x8b1f920`). Своё: вид ответа на `browser.request_input` и
//! подтверждение отчёта.
//!
//! Не взяты в П4к-1а: вход на сайты, покупки, проверки ботов, перерывы,
//! специалисты, файлы, программы (`browser.run`), `look`-цели с моделью
//! наведения, звук страницы — их тексты сокращены (`prompts/SOURCES.md`).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_loop::{personal, Inbox, Tool, ToolError, ToolFuture, Tools, Turn};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::Value;

use crate::window::Window;

const SYSTEM_MD: &str = include_str!("../../../prompts/browser/system.md");
const PAGES_MD: &str = include_str!("../../../prompts/browser/pages.md");
const LANGUAGE_MD: &str = include_str!("../../../prompts/browser/presentation_language.md");
const GATE_MD: &str = include_str!("../../../prompts/browser/completion_gate/report_required.md");

macro_rules! tool_text {
    ($name:literal) => {
        (
            include_str!(concat!("../../../prompts/browser/tools/", $name, ".md")),
            include_str!(concat!("../../../prompts/browser/tools/", $name, ".json")),
        )
    };
}

pub const OBSERVE: &str = "browser.observe";
pub const ACT: &str = "browser.act";
pub const TABS: &str = "browser.tabs";
pub const REQUEST_INPUT: &str = "browser.request_input";
pub const REPORT: &str = "browser.report";

/// Вопрос исполнителя главному агенту.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub question: String,
    pub context: Option<String>,
    pub reason: Option<String>,
}

/// Кто отвечает на вопросы исполнителя (главный агент — П4к-1б).
pub trait Asker: Send + Sync {
    fn ask<'a>(&'a self, q: Question) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;
}

/// Принятый отчёт окна.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub status: String,
    pub summary: String,
    pub reason: Option<String>,
    pub products: Vec<Value>,
}

/// `presentation_language.md` (`0x8b1f920`): текст файла без краёв,
/// `{locale}` — язык, на котором исполнитель пишет людям.
fn language(locale: &str) -> String {
    personal(LANGUAGE_MD).trim().replace("{locale}", locale)
}

/// Первое сообщение окна (`0x8b1f310`, флаг продолжения снят).
pub fn assignment_message(task: &str, locale: &str) -> String {
    format!("Carry out this assignment end to end, then report the result.\n\n{task}\n\nLanguage:\n{}", language(locale))
}

/// Продолжение (`0x8b1f310`, флаг продолжения поднят).
pub fn update_message(text: &str, locale: &str) -> String {
    format!("Continue this task from the history above with Hamlet's new instruction below, then report the result.\n\n{text}\n\nLanguage:\n{}", language(locale))
}

/// Системный текст исполнителя — профиль 1 (`0x290bbe38`), срез 0 `64 23b`:
/// раздел `0x64` вариант 0 — записи `browser/system.md`, `browser/pages.md`
/// (таблица `0x2906bd30`) подряд, без разделителей; затем личный вариант
/// меток и края без переводов строки (как у других профилей). `23b` — П8.
/// Срез 1 (`0x164`, `instructions/*`) у профиля по виду не выводится
/// (`prompts/SOURCES.md`, П5-2 часть 4).
pub fn system() -> String {
    personal(&format!("{SYSTEM_MD}{PAGES_MD}")).trim_matches('\n').to_string()
}

fn spec(name: &str, (md, json): (&str, &str)) -> ToolSpec {
    ToolSpec { name: name.into(), description: personal(md).trim_end().into(), input_schema: serde_json::from_str(json).expect("prompts/browser/tools/*.json") }
}

fn str_arg<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Окно для инструментов страницы: уже открытое (окно задачи браузера)
/// или открываемое при первом вызове (окно исследования — «It opens on your
/// first browser call», `blocks/deep_research/your_environment.md`).
#[derive(Clone)]
pub enum WindowRef {
    Ready(Arc<Window>),
    Lazy(Arc<LazyWindow>),
}

/// Окно, которое запускает свой Chromium с временным профилем при первом
/// вызове; не запустилось — вызовы говорят, что браузер недоступен.
pub struct LazyWindow {
    pub bin: PathBuf,
    pub profile_dir: PathBuf,
    pub headless: bool,
    cell: tokio::sync::OnceCell<Result<Arc<Window>, String>>,
}

impl LazyWindow {
    pub fn new(bin: PathBuf, profile_dir: PathBuf, headless: bool) -> Arc<Self> {
        Arc::new(LazyWindow { bin, profile_dir, headless, cell: tokio::sync::OnceCell::new() })
    }

    pub async fn get(&self) -> Result<Arc<Window>, String> {
        self.cell
            .get_or_init(|| async { Window::open(&self.bin, &self.profile_dir, self.headless).await.map_err(|e| format!("The research browser is unavailable ({e}); continue from the web index.")) })
            .await
            .clone()
    }

    /// Закрыть окно, если оно открывалось, и удалить профиль.
    pub async fn close(&self) {
        if let Some(Ok(w)) = self.cell.get() {
            w.close().await;
        }
        let _ = tokio::fs::remove_dir_all(&self.profile_dir).await;
    }
}

impl WindowRef {
    async fn get(&self) -> Result<Arc<Window>, ToolError> {
        match self {
            WindowRef::Ready(w) => Ok(w.clone()),
            WindowRef::Lazy(l) => l.get().await.map_err(ToolError::Failed),
        }
    }
}

pub struct Observe(pub WindowRef);
pub struct Act(pub WindowRef);
pub struct Tabs(pub WindowRef);
struct RequestInput(Arc<dyn Asker>);
struct ReportTool(Arc<Mutex<Option<Report>>>);

impl Tool for Observe {
    fn spec(&self) -> ToolSpec {
        spec(OBSERVE, tool_text!("observe"))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let tab = str_arg(&args, "tab").ok_or_else(|| ToolError::InvalidArguments("tab is required".into()))?;
            let view = str_arg(&args, "view").unwrap_or("auto");
            let fields: Vec<String> = args.get("fields").and_then(Value::as_array).into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
            if view == "element" && args.get("query").is_some() {
                return Err(ToolError::Failed("query works with the auto and text views, not the element view".into()));
            }
            if view != "element" && (args.get("ref").is_some() || !fields.is_empty()) {
                return Err(ToolError::Failed("target and fields need view \"element\"".into()));
            }
            let element = match (view, str_arg(&args, "ref")) {
                ("element", Some(r)) => Some((r, fields.as_slice())),
                ("element", None) => return Err(ToolError::Failed("the element view needs a target from the latest view".into())),
                _ => None,
            };
            let v = self.0.get().await?.observe(tab, view, str_arg(&args, "query"), element).await.map_err(ToolError::Failed)?;
            Ok(v.parts())
        })
    }
}

impl Tool for Act {
    fn spec(&self) -> ToolSpec {
        spec(ACT, tool_text!("act"))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let tab = str_arg(&args, "tab").ok_or_else(|| ToolError::InvalidArguments("tab is required".into()))?;
            let actions = args.get("actions").and_then(Value::as_array).cloned().unwrap_or_default();
            if actions.is_empty() {
                return Err(ToolError::InvalidArguments("actions is required".into()));
            }
            self.0.get().await?.act(tab, &actions).await.map_err(ToolError::Failed)
        })
    }
}

impl Tool for Tabs {
    fn spec(&self) -> ToolSpec {
        spec(TABS, tool_text!("tabs"))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let kind = str_arg(&args, "kind").ok_or_else(|| ToolError::InvalidArguments("kind is required".into()))?;
            self.0.get().await?.tabs(kind, str_arg(&args, "url"), str_arg(&args, "tab"), str_arg(&args, "ref")).await.map_err(ToolError::Failed)
        })
    }
}

impl Tool for RequestInput {
    fn spec(&self) -> ToolSpec {
        spec(REQUEST_INPUT, tool_text!("request_input"))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let question = str_arg(&args, "question").ok_or_else(|| ToolError::InvalidArguments("question is required".into()))?.to_string();
            let q = Question { question, context: str_arg(&args, "context").map(String::from), reason: str_arg(&args, "reason").map(String::from) };
            let answer = self.0.ask(q).await.map_err(ToolError::Failed)?;
            Ok(vec![ToolResultPart::Text { text: format!("The main agent answered:\n{answer}") }])
        })
    }
}

impl Tool for ReportTool {
    fn spec(&self) -> ToolSpec {
        spec(REPORT, tool_text!("report"))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let status = str_arg(&args, "status").unwrap_or_default().to_string();
            if !["completed", "needs_input", "blocked"].contains(&status.as_str()) {
                return Err(ToolError::InvalidArguments("status must be completed, needs_input or blocked".into()));
            }
            let summary = str_arg(&args, "summary").ok_or_else(|| ToolError::InvalidArguments("summary is required".into()))?.to_string();
            let reason = str_arg(&args, "reason").map(String::from);
            if status == "needs_input" && reason.is_none() {
                return Err(ToolError::InvalidArguments("reason is required for needs_input".into()));
            }
            if status == "completed" && reason.is_some() {
                return Err(ToolError::InvalidArguments("omit reason for completed".into()));
            }
            let products = args.get("products").and_then(Value::as_array).cloned().unwrap_or_default();
            *self.0.lock().unwrap() = Some(Report { status, summary, reason, products });
            // Строка ядра (L49713).
            Ok(vec![ToolResultPart::Text { text: "Hamlet receives it now.".into() }])
        })
    }
}

/// Модель исполнителя — назначение реестра `BrowserAgent` (П2а).
pub fn model<'r>(reg: &'r agent_model::Registry, selection: Option<&str>) -> Result<agent_model::Resolved<'r>, agent_model::RegistryError> {
    Ok(reg.resolve(agent_model::Purpose::BrowserAgent, selection)?.expect("resolve отдаёт None только для Embeddings"))
}

/// Исполнитель одного окна.
pub struct Planner<P> {
    pub provider: Arc<P>,
    pub window: Arc<Window>,
    pub asker: Arc<dyn Asker>,
    pub spill_dir: PathBuf,
    /// Обновления главного агента, пришедшие во время работы: ход забирает
    /// их между шагами.
    pub updates: Option<Inbox>,
    /// Запись истории окна и точек перезапуска (владелец `browser_task`).
    pub record: Option<agent_loop::KindRecord>,
}

impl<P: Provider + Send + Sync + 'static> Planner<P> {
    /// Довести работу до принятого `browser.report`. `history` — история
    /// окна; `message` — задание или продолжение; `None` — продолжение
    /// записанной работы после перезапуска.
    pub async fn run(&self, history: &mut Vec<Message>, message: Option<String>) -> Result<Report, String> {
        let slot = Arc::new(Mutex::new(None));
        let mut tools = Tools::new();
        tools.add(Observe(WindowRef::Ready(self.window.clone())));
        tools.add(Act(WindowRef::Ready(self.window.clone())));
        tools.add(Tabs(WindowRef::Ready(self.window.clone())));
        tools.add(RequestInput(self.asker.clone()));
        tools.add(ReportTool(slot.clone()));
        // Набор исполнителя окна — роль `browser_task`.
        let tools = agent_loop::roles::Role::get("browser_task").apply(tools);
        let mut turn = Turn::new(&*self.provider, &tools, system(), self.spill_dir.clone());
        turn.terminal = vec![REPORT.into()];
        turn.inbox = self.updates.clone();
        turn.record = self.record.as_ref().map(|r| r.record.clone() as Arc<dyn agent_loop::Record>);
        if let Some(text) = message {
            history.push(Message { role: Role::User, content: vec![Part::Text { text }] });
        }
        loop {
            if let Some(r) = &self.record {
                // Имя хода. Своё: `<task_id>:turn:<длина истории>`.
                if let Err(e) = r.record.checkpoint(Some(r.class), Some(&r.owner), Some(&format!("{}:turn:{}", r.owner, history.len()))).await {
                    eprintln!("browser checkpoint: {e}");
                }
            }
            turn.run(history, &mut |_| {}).await.map_err(|e| e.to_string())?;
            if let Some(r) = slot.lock().unwrap().take() {
                return Ok(r);
            }
            history.push(Message { role: Role::User, content: vec![Part::Text { text: personal(GATE_MD).trim_end().into() }] });
        }
    }
}
