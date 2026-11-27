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
//! описания инструментов). Своё: событие монитора — исполненная пачка
//! вызовов подагента, и ход подагента ждёт решения монитора (у muse монитор
//! — отдельная служба); вид блоков «Trigger» и «Subtree Snapshot» и заметки
//! родителю (поля `coordinator_agent_*`, `worker_agent_*`,
//! `coordinator_children` названы в системном тексте muse; заголовок — как в
//! `handoff_child.md`). Глубина подагентов у нас одна: координатор и
//! работник — один и тот же подагент, детей у него нет.

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
const PROMPT_USER_MD: &str = include_str!("../../../prompts/subagent_monitor/prompt_user.md");
const NOTHING_TO_DO_MD: &str = include_str!("../../../prompts/subagent_monitor/nothing_to_do_tool.md");
const NOTIFY_MD: &str = include_str!("../../../prompts/subagent_monitor/notify_main_agent_tool.md");

pub const NOTIFY: &str = "subagent_monitor.notify_main_agent";
pub const NOTHING_TO_DO: &str = "subagent_monitor.nothing_to_do";
pub const MONITORING_FILE: &str = "SUBAGENTS_MONITORING.md";

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

    /// Решение по одному событию: `true` — родителю ушла заметка.
    pub async fn decide(&self, agent_id: &str, task: &str, calls: &[StepCall], inbox: &Inbox) -> bool {
        let event = self.events.fetch_add(1, Ordering::SeqCst) + 1;
        let system = personal(SYSTEM_MD)
            .replace("{subagents_monitoring_md}", self.contract().await.trim_end())
            .replace("{coordinator_task_context_section}", personal(COORDINATOR_CONTEXT_MD).replace("{coordinator_task_prompt}", task).trim_end())
            .replace("{nested_subagent_task_context_section}", "");
        let agent = format!("agent_id: {agent_id}\ndepth: 1\nstatus: running");
        let trig = trigger(calls);
        let user = personal(PROMPT_USER_MD)
            .replace("{{MONITORING_EVENT}}", &format!("monitor_event_id: {event}"))
            .replace("{{COORDINATOR}}", &agent)
            .replace("{{WORKER}}", &agent)
            .replace("{{COORDINATOR_CHILDREN}}", "(none)")
            .replace("{{TRIGGER}}", &trig)
            .replace("{{SUBTREE_SNAPSHOT}}", &format!("{agent_id} (depth 1): running\nchildren: none"));
        let picked = Arc::new(Mutex::new(None));
        let mut tools = Tools::new();
        tools.add(Decide { name: NOTIFY, description: personal(NOTIFY_MD).trim_end().into(), picked: picked.clone() });
        tools.add(Decide { name: NOTHING_TO_DO, description: personal(NOTHING_TO_DO_MD).trim_end().into(), picked: picked.clone() });
        let mut turn = Turn::new(&*self.provider, &tools, system, std::env::temp_dir());
        turn.terminal = vec![NOTIFY.into(), NOTHING_TO_DO.into()];
        let mut history = vec![Message { role: Role::User, content: vec![Part::Text { text: user }] }];
        if let Err(e) = turn.run(&mut history, &mut |_| {}).await {
            eprintln!("subagent monitor {agent_id}: {e}");
            return false;
        }
        let notify = *picked.lock().unwrap() == Some(NOTIFY);
        if notify {
            inbox.push(Message {
                role: Role::Developer,
                content: vec![Part::Text {
                    text: format!(
                        "[Automatic progress note from a running subagent task, not a user message]\nmonitor_event_id: {event}\ncoordinator_agent_id: {agent_id}\ncoordinator_agent_status: running\nworker_agent_id: {agent_id}\nworker_agent_status: running\ncoordinator_children:\n(none)\ntrigger:\n{trig}"
                    ),
                }],
            });
        }
        notify
    }

    /// Крючок шага для хода подагента.
    pub fn hook(self: &Arc<Self>, agent_id: String, task: String, inbox: Inbox) -> StepHook {
        let me = self.clone();
        Arc::new(move |calls: Vec<StepCall>| {
            let me = me.clone();
            let (id, task, inbox) = (agent_id.clone(), task.clone(), inbox.clone());
            Box::pin(async move {
                me.decide(&id, &task, &calls, &inbox).await;
            })
        })
    }
}
