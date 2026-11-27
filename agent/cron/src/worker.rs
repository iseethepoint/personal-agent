//! Работник расписания: один прогон задания — фоновый ход на модели
//! `CronWorker`, в две фазы, как у muse (`hatch-cron/prompts/scheduler/*`):
//!
//! 1. исполнение — `worker_instruction.md` + блок задания + `[BODY]` +
//!    `worker_phase_fence.md`; ход идёт до ответа без вызовов или до срока
//!    `timeout_secs`; по сроку — заметка `[CRON EXECUTION TIMEOUT]`
//!    (`strings-all.txt:28884-28887`);
//! 2. итог — `cron_resolution_contract.md` и ровно один завершающий
//!    инструмент: `hamlet.nothing_to_report` (тихо) или
//!    `hamlet.notify_main_agent` (итог главному агенту). Проверки итога —
//!    `cron_unresolved_execution.md`, `cron_incomplete_execution.md`.
//!
//! Склейки сообщения работнику в разборе нет — есть только метки
//! `[SCHEDULED TASK CONTEXT]`, `<scheduled_job id="…">`, `[BODY]`
//! (`strings-all.txt:84914-84917`); порядок блоков — свой.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools, Turn};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};

const WORKER_INSTRUCTION: &str = include_str!("../../../prompts/cron/worker_instruction.md");
const PHASE_FENCE: &str = include_str!("../../../prompts/cron/worker_phase_fence.md");
const CHANGE_RULES_USER: &str = include_str!("../../../prompts/cron/worker_schedule_change_rules_user.md");
const CHANGE_RULES_SYSTEM: &str = include_str!("../../../prompts/cron/worker_schedule_change_rules_system.md");
const RESOLUTION_CONTRACT: &str = include_str!("../../../prompts/cron/cron_resolution_contract.md");
const UNRESOLVED: &str = include_str!("../../../prompts/cron/cron_unresolved_execution.md");
const INCOMPLETE: &str = include_str!("../../../prompts/cron/cron_incomplete_execution.md");

pub const NOTHING_TO_REPORT: &str = "hamlet.nothing_to_report";
pub const NOTHING_TO_DO: &str = "hamlet.nothing_to_do";

/// Описание `nothing_to_do` — `P/hatch-agent/prompts/tools/nothing_to_do_without_reaction_description.md`.
const NOTHING_TO_DO_DESCRIPTION: &str = include_str!("../../../prompts/hooks/nothing_to_do_description.md");
pub const NOTIFY_MAIN_AGENT: &str = "hamlet.notify_main_agent";

/// Описание `nothing_to_report` — `strings-all.txt:85281-85282`; потерянное
/// в разборе тире восстановлено.
const NOTHING_TO_REPORT_DESCRIPTION: &str = "Signal that this cron worker run produced nothing worth surfacing. Terminal: the run ends immediately, nothing is sent to the main agent, and any further output is dropped. This is the only way to signal a quiet completion \u{2014} do not write NO_REPLY or similar text instead. This tool takes no input arguments.";

/// `hatch-agent/prompts/tools/notify_main_agent_description.md`.
const NOTIFY_DESCRIPTION: &str = "Finish this background worker by handing its final result to its receiving\nagent without rewriting it. The message is stored as a hidden handoff in the\nconversation history and delivered through the receiving agent. Complete any\nvalidation required by this worker's system prompt before calling; do not do\nunrelated work or imitate an unavailable capability.";

/// `notify_main_agent_message_description.md`.
const NOTIFY_MESSAGE_DESCRIPTION: &str =
    "Exact result text to preserve in the hidden handoff and deliver through the\nreceiving agent.";

/// Виды блокировки — перечисление ядра (`strings-all.txt:84222`).
const BLOCKED_REASONS: [&str; 5] =
    ["connector_not_connected", "reauth_required", "account_action_required", "site_challenge", "device_offline"];

/// Заметка по сроку исполнения — `strings-all.txt:28884-28887`.
fn timeout_notice(secs: i64) -> String {
    format!(
        "[CRON EXECUTION TIMEOUT]\nstatus: execution_timed_out\nexecution_timeout_secs: {secs}\nguidance: The execution phase hit its timeout. You are now in resolution only. Do not continue execution, start new work, or initiate new external side effects. Use the transcript and any resolved subagent progress to report committed outcomes, partial progress, uncertainty, and anything still incomplete."
    )
}

/// Заметка после сбоя потока модели — `strings-all.txt:20455-20458`.
fn stream_error_notice(failure: &str) -> String {
    format!(
        "[CRON EXECUTION MODEL STREAM ERROR]\nstatus: execution_provider_stream_decode_error\nfailure: {failure}\nguidance: The execution phase ended after a provider stream decode/no-final error. Do not repeat external side effects just to get a final assistant message. Use the transcript, committed tool results, and any resolved subagent progress to classify the run. Call nothing_to_report only if the scheduled work was fully completed and silence is appropriate; otherwise call notify_main_agent with the committed outcome, uncertainty, or partial failure."
    )
}

/// Первое сообщение работнику. У пульса (`mode = heartbeat`) — правила
/// `worker_schedule_change_rules_system.md` и блок `[HEARTBEAT]` с
/// `~/HEARTBEAT.md` (`heartbeat`).
/// У задания цели — блок `[GOAL CONTEXT]` (`goal`, см. `Core::goal_context`).
pub fn worker_message(job_id: &str, mode: &str, title: Option<&str>, body: &str, heartbeat: Option<&str>, goal: Option<&str>) -> String {
    let rules = if mode == "heartbeat" { CHANGE_RULES_SYSTEM } else { CHANGE_RULES_USER };
    let instruction = personal(WORKER_INSTRUCTION)
        .replace("{job_id}", job_id)
        .replace("{mode}", mode)
        .replace("{schedule_change_rules}", personal(rules).trim_end());
    let title = title.map(|t| format!("\ntitle: {t}")).unwrap_or_default();
    let mut heartbeat = heartbeat.map(|h| format!("\n\n[HEARTBEAT]\n{}", h.trim_end())).unwrap_or_default();
    if let Some(g) = goal {
        heartbeat = format!("\n\n{}{heartbeat}", g.trim_end());
    }
    format!(
        "{}\n\n[SCHEDULED TASK CONTEXT]\n<scheduled_job id=\"{job_id}\">{title}\n</scheduled_job>{heartbeat}\n\n[BODY]\n{}\n\n{}",
        instruction.trim_end(),
        body.trim_end(),
        personal(PHASE_FENCE).trim_end()
    )
}

/// Сигнал работника.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    Quiet,
    Notify { outcome: String, message: String, blocked_reason: Option<String>, blocked_dependency: Option<String> },
}

/// Что было при исполнении — по нему проверяется итог.
#[derive(Debug, Clone, Default)]
struct Evidence {
    failed_calls: Vec<String>,
    unanswered_calls: Vec<String>,
    ending: String,
    complete: bool,
}

impl Evidence {
    fn render(&self) -> String {
        let mut lines = vec![format!("execution: {}", self.ending)];
        for c in &self.failed_calls {
            lines.push(format!("failed call: {c}"));
        }
        for c in &self.unanswered_calls {
            lines.push(format!("call without result: {c}"));
        }
        lines.join("\n")
    }
}

fn collect(history: &[Message]) -> (Vec<String>, Vec<String>) {
    let mut names = std::collections::HashMap::new();
    let mut answered = std::collections::HashSet::new();
    let mut failed = Vec::new();
    for m in history {
        for p in &m.content {
            match p {
                Part::ToolCall { id, name, .. } => {
                    names.insert(id.clone(), name.clone());
                }
                Part::ToolResult { call_id, content, is_error } => {
                    answered.insert(call_id.clone());
                    if *is_error {
                        let first = content
                            .iter()
                            .find_map(|c| match c {
                                ToolResultPart::Text { text } => text.lines().next().map(str::to_string),
                                _ => None,
                            })
                            .unwrap_or_default();
                        let name = names.get(call_id).cloned().unwrap_or_default();
                        failed.push(format!("`{name}`: {first}"));
                    }
                }
                _ => {}
            }
        }
    }
    let mut unanswered: Vec<String> =
        names.iter().filter(|(id, _)| !answered.contains(*id)).map(|(_, n)| format!("`{n}`")).collect();
    unanswered.sort();
    (failed, unanswered)
}

#[derive(Clone)]
struct Resolution {
    evidence: Arc<Evidence>,
    signal: Arc<Mutex<Option<Signal>>>,
    /// Итог работника наблюдателя: `nothing_to_do` и `notify_main_agent`
    /// с одним `message`, без проверок свидетельств (они — у расписаний).
    hook: bool,
}

struct NothingToReport(Resolution);
struct NothingToDo(Resolution);
struct NotifyMainAgent(Resolution);

impl Tool for NothingToDo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NOTHING_TO_DO.into(),
            description: personal(NOTHING_TO_DO_DESCRIPTION).trim_end().into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        }
    }

    fn call(&self, _args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            *self.0.signal.lock().unwrap() = Some(Signal::Quiet);
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

impl Tool for NothingToReport {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NOTHING_TO_REPORT.into(),
            description: NOTHING_TO_REPORT_DESCRIPTION.into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        }
    }

    fn call(&self, _args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let e = &self.0.evidence;
            if !e.complete || !e.failed_calls.is_empty() || !e.unanswered_calls.is_empty() {
                return Err(ToolError::Failed(personal(UNRESOLVED).replace("{execution_evidence}", &e.render())));
            }
            *self.0.signal.lock().unwrap() = Some(Signal::Quiet);
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

impl Tool for NotifyMainAgent {
    fn spec(&self) -> ToolSpec {
        if self.0.hook {
            return ToolSpec {
                name: NOTIFY_MAIN_AGENT.into(),
                description: NOTIFY_DESCRIPTION.into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"message": {"type": "string", "description": NOTIFY_MESSAGE_DESCRIPTION}},
                    "required": ["message"],
                    "additionalProperties": false,
                }),
            };
        }
        // Описаний `outcome` и `blocked_*` в разборе нет — свои; значения и
        // правила — из проверок ядра (`strings-all.txt:84932`).
        ToolSpec {
            name: NOTIFY_MAIN_AGENT.into(),
            description: NOTIFY_DESCRIPTION.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": NOTIFY_MESSAGE_DESCRIPTION},
                    "outcome": {"type": "string", "enum": ["succeeded", "failed", "blocked"]},
                    "blocked_reason": {"type": "string", "enum": BLOCKED_REASONS},
                    "blocked_dependency": {"type": "string"},
                },
                "required": ["message", "outcome"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
            let message = s("message").unwrap_or_default();
            if message.trim().is_empty() {
                return Err(ToolError::InvalidArguments("message cannot be empty".into()));
            }
            if self.0.hook {
                *self.0.signal.lock().unwrap() =
                    Some(Signal::Notify { outcome: "succeeded".into(), message, blocked_reason: None, blocked_dependency: None });
                return Ok(vec![ToolResultPart::Text { text: "ok".into() }]);
            }
            let outcome = s("outcome").unwrap_or_default();
            if !["succeeded", "failed", "blocked"].contains(&outcome.as_str()) {
                return Err(ToolError::InvalidArguments("outcome must be one of succeeded|failed|blocked".into()));
            }
            let reason = s("blocked_reason");
            let dependency = s("blocked_dependency");
            if outcome != "blocked" && (reason.is_some() || dependency.is_some()) {
                return Err(ToolError::InvalidArguments("blocked_reason and blocked_dependency require outcome=blocked".into()));
            }
            if outcome == "blocked" {
                let dep_ok = dependency.as_deref().is_some_and(|d| (1..=160).contains(&d.len()) && !d.contains(char::is_whitespace));
                if !dep_ok {
                    return Err(ToolError::InvalidArguments("blocked_dependency must be a stable 1-160 character identifier".into()));
                }
                if !reason.as_deref().is_some_and(|r| BLOCKED_REASONS.contains(&r)) {
                    return Err(ToolError::InvalidArguments(format!("blocked_reason must be one of {}", BLOCKED_REASONS.join("|"))));
                }
            }
            let e = &self.0.evidence;
            if outcome == "succeeded" && (!e.complete || !e.unanswered_calls.is_empty()) {
                return Err(ToolError::Failed(personal(INCOMPLETE).replace("{execution_evidence}", &e.render())));
            }
            *self.0.signal.lock().unwrap() =
                Some(Signal::Notify { outcome, message, blocked_reason: reason, blocked_dependency: dependency });
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

/// Описания завершающих инструментов (для проверок того, что уходит модели).
pub fn resolution_specs() -> Vec<ToolSpec> {
    let r = Resolution { evidence: Arc::new(Evidence::default()), signal: Arc::new(Mutex::new(None)), hook: false };
    let h = Resolution { hook: true, ..r.clone() };
    vec![NothingToReport(r.clone()).spec(), NotifyMainAgent(r).spec(), NothingToDo(h.clone()).spec(), NotifyMainAgent(h).spec()]
}

/// Чей работник: расписания (`mode` — `task` или `heartbeat`),
/// наблюдателя (`resolve` — сообщение фазы итога) или проверяющего ответ
/// (`resolve`; `terminal` — завершающие инструменты фазы исполнения, `stop`
/// — исполнение кончилось таким инструментом: фаза итога не идёт, итог —
/// исходный черновик).
pub enum Kind {
    /// `resolve` — инструменты исполнения, которые есть и в фазе итога
    /// (роль `scheduler_cron_resolve`: `exec`); `system` — системный текст
    /// фазы итога (профиль `scheduler_resolution`), без него — текст исполнения.
    Cron { mode: String, resolve: Vec<String>, system: Option<String> },
    Hook { resolve: String },
    Verify { resolve: String, terminal: Vec<String>, stop: Arc<dyn Fn() -> bool + Send + Sync> },
}

/// Итог прогона.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub signal: Option<Signal>,
    pub timed_out: bool,
    /// Ошибка модели, если ход не дошёл до итога.
    pub error: Option<String>,
    /// История работника (ход для проверяющего отчёта).
    pub history: Vec<Message>,
}

/// Классы владельцев восстановления фаз работника расписаний — значения
/// ядра (`scheduler_cron_execute`, `scheduler_cron_resolve`).
pub const EXECUTE_CLASS: &str = "scheduler_cron_execute";
pub const RESOLVE_CLASS: &str = "scheduler_cron_resolve";
/// Класс владельца восстановления работника наблюдателя — значение ядра.
pub const HOOK_CLASS: &str = "execute_resolve_worker";

/// Прогон после перезапуска продолжается с записанного — текст ядра
/// («continue scheduler recovery owner», около `0x2a4cf45`).
pub const CONTINUES_AFTER_RESTART: &str = include_str!("../../../prompts/restart/run_continues.md");

/// С чего начать прогон.
pub enum Begin {
    /// Новый прогон с первым сообщением.
    Fresh(String),
    /// Новый прогон с готовыми первыми сообщениями (метка времени и
    /// сообщение работнику).
    Messages(Vec<Message>),
    /// Продолжение после перезапуска: записанная история и фаза, в которой
    /// прогон оборвался; `timed_out` — срок исполнения уже вышел.
    Resume { history: Vec<Message>, resolve: bool, timed_out: bool },
}

/// Запись работника: история, точки перезапуска фаз (`recovery_class` —
/// класс фазы, `recovery_owner_id` — прогон).
#[derive(Clone)]
pub struct Recording {
    pub record: Arc<dyn agent_loop::ChildRecord>,
    pub run_id: String,
    /// `recovery_class` точек фаз исполнения и итога.
    pub classes: (&'static str, &'static str),
}

impl Recording {
    /// Работник прогона расписания.
    pub fn cron(record: Arc<dyn agent_loop::ChildRecord>, run_id: String) -> Recording {
        Recording { record, run_id, classes: (EXECUTE_CLASS, RESOLVE_CLASS) }
    }

    /// Работник прогона наблюдателя: класс один на обе фазы.
    pub fn hook(record: Arc<dyn agent_loop::ChildRecord>, run_id: String) -> Recording {
        Recording { record, run_id, classes: (HOOK_CLASS, HOOK_CLASS) }
    }
}

/// Фаза итога: имя хода в точке кончается на `:resolve`.
pub fn resolve_phase(cp: &agent_loop::Checkpoint) -> bool {
    cp.message_id.ends_with(":resolve")
}

impl Recording {
    async fn checkpoint(&self, class: Option<&str>, turn: Option<&str>) {
        if let Err(e) = self.record.checkpoint(class, Some(&self.run_id), turn).await {
            eprintln!("worker checkpoint {}: {e}", self.run_id);
        }
    }
}

/// Прогнать задание: фаза исполнения — на `provider`, фаза итога — на
/// `resolver` (у расписаний — модель `CronResolve`); `prefix` — начало
/// подсказки работника (системный текст и инструменты исполнения);
/// `timeout_secs` — срок фазы исполнения (у наблюдателя срока нет);
/// `on_phase` узнаёт о переходе к итогу и о том, был ли срок.
#[allow(clippy::too_many_arguments)]
pub async fn run<P: Provider>(
    provider: &P,
    resolver: &P,
    prefix: &agent_loop::Prefix,
    tools: &Tools,
    spill_dir: &std::path::Path,
    begin: Begin,
    timeout_secs: Option<i64>,
    kind: Kind,
    on_phase: impl Fn(&'static str, bool),
    recording: Option<Recording>,
) -> Outcome {
    let mut quiet = |_: &str| {};
    let record = recording.as_ref().map(|r| r.record.clone() as Arc<dyn agent_loop::Record>);
    let turn_name = |phase: &str| recording.as_ref().map(|r| format!("{}:{phase}", r.run_id));
    let (mut history, resolve_only, was_timed_out) = match begin {
        Begin::Fresh(first) => (vec![Message { role: Role::User, content: vec![Part::Text { text: first }] }], false, false),
        Begin::Messages(m) => (m, false, false),
        Begin::Resume { history, resolve, timed_out } => (history, resolve, timed_out),
    };
    let mut timed_out = was_timed_out;
    let mut evidence = Evidence::default();
    if !resolve_only {
        if let Some(r) = &recording {
            r.checkpoint(Some(r.classes.0), turn_name("execute").as_deref()).await;
        }
        let mut turn = Turn::from_prefix(provider, tools, prefix, spill_dir);
        turn.record = record.clone();
        if let Kind::Verify { terminal, .. } = &kind {
            turn.terminal = terminal.clone();
        }
        let executed = match timeout_secs {
            Some(t) => tokio::time::timeout(Duration::from_secs(t.max(1) as u64), turn.run(&mut history, &mut quiet)).await,
            None => Ok(turn.run(&mut history, &mut quiet).await),
        };
        let timeout_secs = timeout_secs.unwrap_or_default();
        let mut notice = None;
        match executed {
            Ok(Ok(_)) => {
                evidence.complete = true;
                evidence.ending = "completed".into();
            }
            Ok(Err(e)) => {
                evidence.ending = format!("ended with error: {e}");
                notice = Some(stream_error_notice(&e.to_string()));
            }
            Err(_) => {
                timed_out = true;
                evidence.ending = format!("timed out after {timeout_secs}s");
                notice = Some(timeout_notice(timeout_secs));
            }
        }
        close_dangling(&mut history);
        if let Kind::Verify { stop, .. } = &kind {
            if stop() {
                if let Some(r) = &recording {
                    r.checkpoint(None, None).await;
                }
                return Outcome { signal: Some(Signal::Quiet), timed_out, error: None, history };
            }
        }
        let (failed, unanswered) = collect(&history);
        evidence.failed_calls = failed;
        evidence.unanswered_calls = unanswered;

        on_phase("resolve", timed_out);
        let mut text = String::new();
        if let Some(n) = notice {
            text.push_str(&n);
            text.push_str("\n\n");
        }
        match &kind {
            Kind::Cron { mode, .. } => text.push_str(personal(RESOLUTION_CONTRACT).replace("{mode}", mode).trim_end()),
            Kind::Hook { resolve } | Kind::Verify { resolve, .. } => text.push_str(resolve.trim_end()),
        }
        history.push(Message { role: Role::User, content: vec![Part::Text { text }] });
        // Договор итога записан до точки фазы итога: продолжение итога после
        // перезапуска всегда видит его в истории.
        if let Some(r) = &recording {
            if let Err(e) = r.record.record(&history).await {
                eprintln!("worker history {}: {e}", r.run_id);
            }
        }
    } else {
        // Итог после перезапуска: свидетельства — по записанной истории.
        let (failed, unanswered) = collect(&history);
        evidence.failed_calls = failed;
        evidence.unanswered_calls = unanswered;
        evidence.complete = !timed_out;
        evidence.ending = if timed_out { "timed out".into() } else { "completed".into() };
    }
    let hook = matches!(kind, Kind::Hook { .. } | Kind::Verify { .. });

    let resolution = Resolution { evidence: Arc::new(evidence), signal: Arc::new(Mutex::new(None)), hook };
    let mut terminal = Tools::new();
    if let Kind::Cron { resolve, .. } = &kind {
        for name in resolve {
            if let Some(t) = tools.get(name) {
                terminal.add_shared(name, t.clone());
            }
        }
    }
    let quiet_name = if hook {
        terminal.add(NothingToDo(resolution.clone()));
        NOTHING_TO_DO
    } else {
        terminal.add(NothingToReport(resolution.clone()));
        NOTHING_TO_REPORT
    };
    terminal.add(NotifyMainAgent(resolution.clone()));
    if let Some(r) = &recording {
        r.checkpoint(Some(r.classes.1), turn_name("resolve").as_deref()).await;
    }
    let system = match &kind {
        Kind::Cron { system: Some(s), .. } => s.clone(),
        _ => prefix.system.clone(),
    };
    let mut turn = Turn::new(resolver, &terminal, system, spill_dir);
    turn.terminal = vec![quiet_name.into(), NOTIFY_MAIN_AGENT.into()];
    turn.record = record;
    let error = turn.run(&mut history, &mut quiet).await.err().map(|e| e.to_string());
    if let Some(r) = &recording {
        r.checkpoint(None, None).await;
    }
    let signal = resolution.signal.lock().unwrap().clone();
    Outcome { signal, timed_out, error, history }
}

/// После прерванного по сроку хода у вызовов без результата появляется
/// результат с ошибкой — история остаётся годной для поставщика.
fn close_dangling(history: &mut Vec<Message>) {
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
                content: vec![ToolResultPart::Text { text: "interrupted by the execution timeout before the tool finished".into() }],
                is_error: true,
            }),
            _ => None,
        })
        .collect();
    if !results.is_empty() {
        history.push(Message { role: Role::User, content: results });
    }
}
