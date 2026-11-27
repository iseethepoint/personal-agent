//! Монитор подагентов — назначение muse `SubagentMonitor`
//! (`strings-all.txt:56281`), профиль «subagent_monitor» с инструментами
//! `subagent_monitor.notify_main_agent` / `subagent_monitor.nothing_to_do`
//! (`:74305-74318`). Итог подагента рантайм отдаёт родителю сам, без
//! монитора («Runtime handles terminal completion delivery deterministically
//! before this monitor is asked anything», `subagents_monitoring.md`);
//! монитор решает только о промежуточных событиях — по правилам файла
//! `~/SUBAGENTS_MONITORING.md` (владелец может его править; нет файла —
//! кладётся текст muse по умолчанию) и правилу безопасности системного
//! текста.
//!
//! Тексты — `prompts/subagent_monitor/` (системный блок
//! `blocks/activity_monitor/subagent_monitor.md`, `prompt_user.md`,
//! описания инструментов). Монитор — своя задача tokio с очередью событий,
//! как отдельная служба muse: ход подагента его не ждёт. Событие — снимок
//! исполненной пачки вызовов; события, дошедшие до монитора после итога
//! подагента, уже не рассматриваются (итог доставлен, «terminal completion
//! delivery … before this monitor is asked anything»).
//!
//! Координатор — подагент корня (глубина 1), вершина наблюдаемого поддерева;
//! работник — агент, чей шаг вызвал событие: сам координатор или его
//! подагент (глубина 2). Снимок поддерева берётся в миг решения. Заметка —
//! «receiving parent agent» работника (системный текст muse): работник —
//! координатор, — корню, с полями `coordinator_agent_*`, `worker_agent_*`,
//! `coordinator_children`; работник глубже — координатору, по
//! `handoff_child.md` (`child_agent_*`, `child_subagents`) с указанием
//! `parent_follow_up_silence_guidance.md`, а задача работника — в системном
//! тексте монитора (`subagent_monitor_nested_subagent_task_context.md`).
//! Своё: вид блоков «Coordinator», «Worker», «Coordinator Children»,
//! «Trigger», «Subtree Snapshot» и заметки корню (поля названы в системном
//! тексте muse; заголовок — как в `handoff_child.md`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};

use crate::compaction::personal;
use crate::subagents::Inbox;
use crate::tool::{Tool, ToolFuture, Tools};
use crate::turn::{StepCall, StepHook, Turn};

const SYSTEM_MD: &str = include_str!("../../../prompts/subagent_monitor/system.md");
pub const DEFAULT_MONITORING_MD: &str = include_str!("../../../prompts/subagent_monitor/subagents_monitoring.md");
const EMPTY_MONITORING_MD: &str = include_str!("../../../prompts/subagent_monitor/empty_subagents_monitoring.md");
const COORDINATOR_CONTEXT_MD: &str = include_str!("../../../prompts/subagent_monitor/subagent_monitor_coordinator_task_context.md");
const NESTED_CONTEXT_MD: &str = include_str!("../../../prompts/subagent_monitor/subagent_monitor_nested_subagent_task_context.md");
const HANDOFF_CHILD_MD: &str = include_str!("../../../prompts/subagent_monitor/handoff_child.md");
const SILENCE_GUIDANCE_MD: &str = include_str!("../../../prompts/subagent_monitor/parent_follow_up_silence_guidance.md");
const PROMPT_USER_MD: &str = include_str!("../../../prompts/subagent_monitor/prompt_user.md");
const NOTHING_TO_DO_MD: &str = include_str!("../../../prompts/subagent_monitor/nothing_to_do_tool.md");
const NOTIFY_MD: &str = include_str!("../../../prompts/subagent_monitor/notify_main_agent_tool.md");

pub const NOTIFY: &str = "subagent_monitor.notify_main_agent";
pub const NOTHING_TO_DO: &str = "subagent_monitor.nothing_to_do";
pub const MONITORING_FILE: &str = "SUBAGENTS_MONITORING.md";

/// Агент поддерева в снимке.
#[derive(Debug, Clone)]
pub struct View {
    pub id: String,
    pub depth: u32,
    pub status: String,
    pub task: String,
}

impl View {
    fn block(&self) -> String {
        format!("agent_id: {}\ndepth: {}\nstatus: {}", self.id, self.depth, self.status)
    }

    fn line(&self) -> String {
        format!("- {} (depth {}): {}", self.id, self.depth, self.status)
    }
}

/// Снимок поддерева на миг решения.
#[derive(Debug, Clone)]
pub struct Event {
    pub coordinator: View,
    pub worker: View,
    /// Место работника среди детей его родителя: (номер, сколько).
    pub ordinal: (usize, usize),
    pub coordinator_children: Vec<View>,
    pub worker_children: Vec<View>,
}

impl Event {
    /// Работник — сам координатор: заметка — корню.
    fn to_root(&self) -> bool {
        self.worker.id == self.coordinator.id
    }
}

/// Снимок поддерева по требованию; `None` — агента уже нет.
pub type Snapshot = Arc<dyn Fn() -> Option<Event> + Send + Sync>;

fn lines(v: &[View]) -> String {
    if v.is_empty() {
        "(none)".into()
    } else {
        v.iter().map(View::line).collect::<Vec<_>>().join("\n")
    }
}

/// Монитор: модель `SubagentMonitor` и дом, где лежит
/// `SUBAGENTS_MONITORING.md`.
pub struct Monitor<P> {
    pub provider: Arc<P>,
    pub home: PathBuf,
    events: AtomicU64,
}

struct Decide {
    name: &'static str,
    description: String,
    picked: Arc<Mutex<Option<&'static str>>>,
}

impl Tool for Decide {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.into(),
            description: self.description.clone(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        }
    }

    fn call(&self, _args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            *self.picked.lock().unwrap() = Some(self.name);
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

fn trigger(calls: &[StepCall]) -> String {
    calls
        .iter()
        .map(|c| {
            let mut args = c.arguments.to_string();
            if args.chars().count() > 300 {
                args = args.chars().take(300).collect::<String>() + "…";
            }
            let outcome = if c.is_error { "failed" } else { "completed" };
            format!("- tool `{}` {outcome}; arguments: {args}; result: {}", c.name, c.result)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl<P: Provider + Send + Sync + 'static> Monitor<P> {
    pub fn new(provider: Arc<P>, home: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Monitor { provider, home: home.into(), events: AtomicU64::new(0) })
    }

    /// `SUBAGENTS_MONITORING.md` владельца; нет — кладётся текст muse.
    async fn contract(&self) -> String {
        let path = self.home.join(MONITORING_FILE);
        match tokio::fs::read_to_string(&path).await {
            Ok(t) if t.trim().is_empty() => personal(EMPTY_MONITORING_MD).trim_end().to_string(),
            Ok(t) => t,
            Err(_) => {
                let _ = tokio::fs::write(&path, DEFAULT_MONITORING_MD).await;
                personal(DEFAULT_MONITORING_MD)
            }
        }
    }

    /// Решение по одному событию: `true` — получателю ушла заметка.
    /// `inbox` — ящик получателя (корня или координатора). Решение,
    /// принятое уже после итога работника (`running` — нет), заметки не
    /// шлёт: итог доставлен, промежуточное устарело.
    pub async fn decide(&self, ev: &Event, calls: &[StepCall], inbox: &Inbox, running: &(dyn Fn() -> bool + Send + Sync)) -> bool {
        let event = self.events.fetch_add(1, Ordering::SeqCst) + 1;
        let nested = if ev.to_root() {
            String::new()
        } else {
            personal(NESTED_CONTEXT_MD).replace("{nested_subagent_task_prompt}", &ev.worker.task).trim_end().to_string()
        };
        let system = personal(SYSTEM_MD)
            .replace("{subagents_monitoring_md}", self.contract().await.trim_end())
            .replace("{coordinator_task_context_section}", personal(COORDINATOR_CONTEXT_MD).replace("{coordinator_task_prompt}", &ev.coordinator.task).trim_end())
            .replace("{nested_subagent_task_context_section}", &nested);
        let trig = trigger(calls);
        let mut snapshot = format!("{} (depth {}): {}", ev.coordinator.id, ev.coordinator.depth, ev.coordinator.status);
        for c in &ev.coordinator_children {
            snapshot.push_str(&format!("\n  {} (depth {}): {}", c.id, c.depth, c.status));
        }
        let user = personal(PROMPT_USER_MD)
            .replace("{{MONITORING_EVENT}}", &format!("monitor_event_id: {event}"))
            .replace("{{COORDINATOR}}", &ev.coordinator.block())
            .replace("{{WORKER}}", &format!("{}\nordinal: {}/{}", ev.worker.block(), ev.ordinal.0, ev.ordinal.1))
            .replace("{{COORDINATOR_CHILDREN}}", &lines(&ev.coordinator_children))
            .replace("{{TRIGGER}}", &trig)
            .replace("{{SUBTREE_SNAPSHOT}}", &snapshot);
        let picked = Arc::new(Mutex::new(None));
        let mut tools = Tools::new();
        tools.add(Decide { name: NOTIFY, description: personal(NOTIFY_MD).trim_end().into(), picked: picked.clone() });
        tools.add(Decide { name: NOTHING_TO_DO, description: personal(NOTHING_TO_DO_MD).trim_end().into(), picked: picked.clone() });
        let mut turn = Turn::new(&*self.provider, &tools, system, std::env::temp_dir());
        turn.terminal = vec![NOTIFY.into(), NOTHING_TO_DO.into()];
        let mut history = vec![Message { role: Role::User, content: vec![Part::Text { text: user }] }];
        if let Err(e) = turn.run(&mut history, &mut |_| {}).await {
            eprintln!("subagent monitor {}: {e}", ev.worker.id);
            return false;
        }
        let notify = *picked.lock().unwrap() == Some(NOTIFY) && running();
        if notify {
            let text = if ev.to_root() {
                format!(
                    "[Automatic progress note from a running subagent task, not a user message]\nmonitor_event_id: {event}\ncoordinator_agent_id: {}\ncoordinator_agent_status: {}\nworker_agent_id: {}\nworker_agent_status: {}\ncoordinator_children:\n{}\ntrigger:\n{trig}",
                    ev.coordinator.id,
                    ev.coordinator.status,
                    ev.worker.id,
                    ev.worker.status,
                    lines(&ev.coordinator_children)
                )
            } else {
                personal(HANDOFF_CHILD_MD)
                    .replace("{{EVENT_ID}}", &event.to_string())
                    .replace("{{CHILD_AGENT_ID}}", &ev.worker.id)
                    .replace("{{CHILD_AGENT_DEPTH}}", &ev.worker.depth.to_string())
                    .replace("{{CHILD_AGENT_ORDINAL}}", &ev.ordinal.0.to_string())
                    .replace("{{CHILD_AGENT_COUNT}}", &ev.ordinal.1.to_string())
                    .replace("{{CHILD_AGENT_STATUS}}", &ev.worker.status)
                    .replace("{{CHILD_SUBAGENTS}}", &lines(&ev.worker_children))
                    .replace("{{SUBAGENT_HANDOFF_GUIDANCE}}", personal(SILENCE_GUIDANCE_MD).trim_end())
                    .replace("{{EVENT_DETAILS}}", &trig)
                    .trim_end()
                    .to_string()
            };
            inbox.push(Message { role: Role::Developer, content: vec![Part::Text { text }] }).await;
        }
        notify
    }

    /// Очередь и задача монитора одного подагента; крючок шага только
    /// кладёт событие в очередь. `running` — идёт ли ещё работа подагента.
    pub fn watch(self: &Arc<Self>, snapshot: Snapshot, inbox: Inbox, running: Arc<dyn Fn() -> bool + Send + Sync>) -> StepHook {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<StepCall>>();
        let me = self.clone();
        tokio::spawn(async move {
            while let Some(calls) = rx.recv().await {
                if !running() {
                    continue;
                }
                let Some(ev) = snapshot() else { continue };
                me.decide(&ev, &calls, &inbox, &*running).await;
            }
        });
        Arc::new(move |calls: Vec<StepCall>| {
            let _ = tx.send(calls);
        })
    }
}
