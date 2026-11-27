//! Решения модели после новой записи — `goals_tool/activity_state.rs` и
//! `goals_tool/subgoal_rollup.rs` muse:
//!
//! - «Goal Activity Pulse» (`strings-all.txt:72031-72032`): заменить ли
//!   `current_state` цели и какое теперь внимание — текст
//!   `P/hatch-agent/prompts/goals/activity_current_state.md`, инструменты
//!   `replace_current_state` / `no_new_state`;
//! - «Subgoal Activity Rollup» (`:74318`): запись подцели в словах
//!   родителя — `subgoal_activity_rollup.md`, инструмент
//!   `log_parent_activity`.
//!
//! Без модели и при её сбое muse сохраняет прежнее состояние и переносит запись
//! на родителя как есть («goal activity pulse/attention inference failed;
//! preserving existing state», «subgoal activity rollup inference failed;
//! using deterministic copy», `strings-all.txt:55730`) — так же и здесь.
//! Поля инструментов — имена ядра (`strings-all.txt:226003`); их описаний и
//! вида входного сообщения в разборе нет — свои.

use std::sync::{Arc, Mutex};

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools, Turn};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};

use crate::store::{Entry, Goal, ATTENTION};

const PULSE_MD: &str = include_str!("../../../prompts/goals/activity_current_state.md");
const ROLLUP_MD: &str = include_str!("../../../prompts/goals/subgoal_activity_rollup.md");

pub const REPLACE_CURRENT_STATE: &str = "hamlet.replace_current_state";
pub const NO_NEW_STATE: &str = "hamlet.no_new_state";
pub const LOG_PARENT_ACTIVITY: &str = "hamlet.log_parent_activity";

/// Решение о строке состояния.
#[derive(Debug, Clone, PartialEq)]
pub struct Pulse {
    pub current_state: String,
    pub attention: String,
    pub next_due_at: Option<String>,
}

/// Запись подцели в словах родителя.
#[derive(Debug, Clone, PartialEq)]
pub struct Rollup {
    pub title: String,
    pub current_state: String,
    pub description: String,
    pub parent_current_state: Option<String>,
}

type Slot<T> = Arc<Mutex<Option<T>>>;

struct Decide<T> {
    spec: ToolSpec,
    slot: Slot<T>,
    parse: fn(&Value) -> Result<T, String>,
}

impl<T: Send + 'static> Tool for Decide<T> {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let v = (self.parse)(&args).map_err(ToolError::InvalidArguments)?;
            *self.slot.lock().unwrap() = Some(v);
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

fn text(v: &Value, k: &str) -> Result<String, String> {
    v.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from).ok_or_else(|| format!("{k} is required"))
}

fn parse_pulse(v: &Value) -> Result<Pulse, String> {
    let attention = text(v, "attention")?;
    if !ATTENTION.contains(&attention.as_str()) {
        return Err(format!("attention must be one of {}", ATTENTION.join("|")));
    }
    Ok(Pulse { current_state: text(v, "current_state")?, attention, next_due_at: v.get("next_due_at").and_then(Value::as_str).map(String::from) })
}

fn parse_unit(_: &Value) -> Result<(), String> {
    Ok(())
}

fn parse_rollup(v: &Value) -> Result<Rollup, String> {
    Ok(Rollup {
        title: text(v, "title")?,
        current_state: text(v, "current_state")?,
        description: text(v, "description")?,
        parent_current_state: v.get("parent_current_state").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from),
    })
}

fn goal_json(g: &Goal) -> Value {
    json!({"goal_id": g.goal_id, "title": g.title, "current_state": g.current_state, "description": g.description,
           "goal_kind": g.goal_kind, "attention": g.attention_kind, "next_due_at": g.next_due_at})
}

fn entry_json(e: &Entry) -> Value {
    json!({"title": e.title, "current_state": e.current_state, "description": e.description, "effective_at": e.effective_at})
}

async fn ask<P: Provider>(provider: &P, system: &str, user: String, tools: Tools, terminal: Vec<String>) -> Result<(), String> {
    let mut history = vec![Message { role: Role::User, content: vec![Part::Text { text: user }] }];
    let spill = std::env::temp_dir();
    let mut turn = Turn::new(provider, &tools, system, spill);
    turn.terminal = terminal;
    turn.run(&mut history, &mut |_| {}).await.map(|_| ()).map_err(|e| e.to_string())
}

/// Заменить ли строку состояния. `None` — оставить как есть (решение
/// `no_new_state` или сбой модели).
pub async fn pulse<P: Provider>(provider: &P, goal: &Goal, entry: &Entry, recent: &[Entry], observed_at: &str) -> Option<Pulse> {
    let slot: Slot<Pulse> = Arc::new(Mutex::new(None));
    let noop: Slot<()> = Arc::new(Mutex::new(None));
    let mut tools = Tools::new();
    tools.add(Decide {
        spec: ToolSpec {
            name: REPLACE_CURRENT_STATE.into(),
            description: "Replace the goal's pulse and set its attention after this activity.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "current_state": {"type": "string", "description": "The new one-sentence pulse."},
                    "attention": {"type": "string", "enum": ATTENTION},
                    "evidence_dates": {"type": "array", "items": {"type": "string"}, "description": "Due dates or deadlines the evidence names, resolved to dates."},
                    "next_due_at": {"type": "string", "description": "Nearest date the owner is due to meet, RFC 3339."},
                },
                "required": ["current_state", "attention"],
                "additionalProperties": false,
            }),
        },
        slot: slot.clone(),
        parse: parse_pulse,
    });
    tools.add(Decide {
        spec: ToolSpec {
            name: NO_NEW_STATE.into(),
            description: "Keep the existing pulse and attention.".into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        slot: noop,
        parse: parse_unit,
    });
    let user = serde_json::to_string_pretty(&json!({
        "observed_at": observed_at,
        "goal": goal_json(goal),
        "new_activity": entry_json(entry),
        "recent_activity": recent.iter().take(10).map(entry_json).collect::<Vec<_>>(),
    }))
    .unwrap_or_default();
    let system = personal(PULSE_MD);
    if let Err(e) = ask(provider, &system, user, tools, vec![REPLACE_CURRENT_STATE.into(), NO_NEW_STATE.into()]).await {
        eprintln!("goal activity pulse/attention inference failed; preserving existing state: {e}");
    }
    let got = slot.lock().unwrap().clone();
    got
}

/// Запись подцели в словах родителя. При сбое модели — запись как есть.
pub async fn rollup<P: Provider>(provider: &P, parent: &Goal, child: &Goal, entry: &Entry, parent_recent: &[Entry]) -> Rollup {
    let slot: Slot<Rollup> = Arc::new(Mutex::new(None));
    let mut tools = Tools::new();
    tools.add(Decide {
        spec: ToolSpec {
            name: LOG_PARENT_ACTIVITY.into(),
            description: "Log the parent goal's version of the sub-goal activity.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "current_state": {"type": "string"},
                    "description": {"type": "string"},
                    "parent_current_state": {"type": "string"},
                },
                "required": ["title", "current_state", "description"],
                "additionalProperties": false,
            }),
        },
        slot: slot.clone(),
        parse: parse_rollup,
    });
    let user = serde_json::to_string_pretty(&json!({
        "parent_goal": goal_json(parent),
        "sub_goal": goal_json(child),
        "sub_goal_activity": entry_json(entry),
        "recent_parent_activity": parent_recent.iter().take(10).map(entry_json).collect::<Vec<_>>(),
    }))
    .unwrap_or_default();
    if let Err(e) = ask(provider, &personal(ROLLUP_MD), user, tools, vec![LOG_PARENT_ACTIVITY.into()]).await {
        eprintln!("subgoal activity rollup inference failed; using deterministic copy: {e}");
    }
    let got = slot.lock().unwrap().clone();
    got.unwrap_or_else(|| copy(entry))
}

/// Перенос как есть — «using deterministic copy».
pub fn copy(entry: &Entry) -> Rollup {
    Rollup {
        title: entry.title.clone().unwrap_or_default(),
        current_state: entry.current_state.clone().unwrap_or_default(),
        description: entry.description.clone().unwrap_or_default(),
        parent_current_state: None,
    }
}
