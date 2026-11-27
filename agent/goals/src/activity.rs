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
//! - «Goal Attention» (`:72033-72039`): внимание дела после события —
//!   `P/hatch-server/prompts/goals/attention.md`, инструмент `set_attention`
//!   (описание и поля — ядро: attention, evidence_dates, next_due_at,
//!   escalation {condition, due_at, action}, clear_escalation); входы —
//!   current_state, current_attention, triggering_activity (`:150805`),
//!   время — с днём недели (`%A, %Y-%m-%dT%H:%M:%SZ`);
//! - «Tracking Lifecycle Copy» (`:74321-74325`): состояние и заголовок
//!   записи при завершении или возобновлении дела —
//!   `tracking_lifecycle_{completion,reopen}_system.md`,
//!   `tracking_lifecycle_copy_input.md` (`{{evidence_json}}`), инструмент
//!   `set_tracking_goal_lifecycle_copy`; поля улик — имена ядра
//!   (`status_before_transition`, `lifecycle_reason`, `current_state_hint`).
//!
//! Без модели и при её сбое muse сохраняет прежнее состояние и переносит запись
//! на родителя как есть («goal activity pulse/attention inference failed;
//! preserving existing state», «subgoal activity rollup inference failed;
//! using deterministic copy», `strings-all.txt:55730`) — так же и здесь;
//! внимание остаётся прежним («goal attention inference failed; preserving
//! existing attention»), текст смены статуса — запасной («tracking
//! lifecycle copy inference failed; using deterministic fallback»).
//! Поля инструментов — имена ядра (`strings-all.txt:226003`); их описаний и
//! вида входного сообщения в разборе нет — свои.

use std::sync::{Arc, Mutex};

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools, Turn};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};

use crate::store::{Entry, Escalation, Goal, ATTENTION, ESCALATION_ACTIONS};

const PULSE_MD: &str = include_str!("../../../prompts/goals/activity_current_state.md");
const ROLLUP_MD: &str = include_str!("../../../prompts/goals/subgoal_activity_rollup.md");
const ATTENTION_MD: &str = include_str!("../../../prompts/goals/attention.md");
const COMPLETION_MD: &str = include_str!("../../../prompts/goals/tracking_lifecycle_completion_system.md");
const REOPEN_MD: &str = include_str!("../../../prompts/goals/tracking_lifecycle_reopen_system.md");
const LIFECYCLE_INPUT_MD: &str = include_str!("../../../prompts/goals/tracking_lifecycle_copy_input.md");

pub const REPLACE_CURRENT_STATE: &str = "hamlet.replace_current_state";
pub const NO_NEW_STATE: &str = "hamlet.no_new_state";
pub const LOG_PARENT_ACTIVITY: &str = "hamlet.log_parent_activity";
pub const SET_ATTENTION: &str = "hamlet.set_attention";
pub const SET_LIFECYCLE_COPY: &str = "hamlet.set_tracking_goal_lifecycle_copy";

/// Решение о внимании дела.
#[derive(Debug, Clone, PartialEq)]
pub struct Attention {
    pub attention: String,
    pub next_due_at: Option<String>,
    pub escalation: Option<Escalation>,
    pub clear_escalation: bool,
}

/// Текст смены статуса дела.
#[derive(Debug, Clone, PartialEq)]
pub struct Lifecycle {
    pub current_state: String,
    pub activity_title: String,
}

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

fn parse_attention(v: &Value) -> Result<Attention, String> {
    let attention = text(v, "attention")?;
    if !ATTENTION.contains(&attention.as_str()) {
        return Err(format!("invalid goal attention `{attention}`"));
    }
    let clear_escalation = v.get("clear_escalation").and_then(Value::as_bool).unwrap_or(false);
    let escalation = match v.get("escalation") {
        None | Some(Value::Null) => None,
        Some(e) => Some(Escalation::parse(e, "invalid stored goal escalation: condition and due_at must be non-empty", "goal escalation due_at is not a valid RFC 3339 timestamp")?),
    };
    if clear_escalation && escalation.is_some() {
        return Err("clear_escalation cannot be combined with a new escalation".into());
    }
    Ok(Attention { attention, next_due_at: v.get("next_due_at").and_then(Value::as_str).map(String::from), escalation, clear_escalation })
}

fn parse_lifecycle(v: &Value) -> Result<Lifecycle, String> {
    Ok(Lifecycle { current_state: text(v, "current_state")?, activity_title: text(v, "activity_title")? })
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
           "goal_kind": g.goal_kind, "status": g.status, "attention": g.attention_kind, "next_due_at": g.next_due_at,
           "escalation": g.escalation})
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

/// Внимание дела после события. `None` — оставить прежнее (сбой модели).
/// `trigger` — событие: запись ленты или создание дела.
pub async fn attention<P: Provider>(provider: &P, goal: &Goal, trigger: Value, recent: &[Entry], observed_at: &str) -> Option<Attention> {
    let slot: Slot<Attention> = Arc::new(Mutex::new(None));
    let mut tools = Tools::new();
    tools.add(Decide {
        spec: ToolSpec {
            name: SET_ATTENTION.into(),
            description: "Set a tracking goal's effective attention state after reviewing the supplied evidence.".into(),
            input_schema: json!({
                "type": "object",
                "description": "Set the tracking goal's effective attention after reviewing all supplied context.",
                "properties": {
                    "attention": {"type": "string", "enum": ATTENTION},
                    "evidence_dates": {"type": "array", "items": {"type": "string"},
                        "description": "Optional. First list every concrete date or deadline the evidence states, each as 'ISO date - short label', resolving relative phrases against the supplied observed_at."},
                    "next_due_at": {"type": "string",
                        "description": "Optional. From evidence_dates, the nearest date the user is actually due to meet, as an ISO 8601 date or datetime. A goal's own scheduled occurrence is not a user obligation; never report it here. Omit when the evidence names no dated obligation; never invent a date."},
                    "escalation": {
                        "type": ["object", "null"],
                        "description": "Optional. The goal's one escalation: set only from a conditional the user stated in their own words (\"if no reply by Thursday, remind me\") or a hard deadline with a stated consequence. Restate it on every run while it still holds; never infer a soft deadline or a goal's own schedule. Null leaves the stored escalation unchanged.",
                        "properties": {
                            "condition": {"type": "string", "description": "The user's condition in their own framing."},
                            "due_at": {"type": "string", "description": "When the condition becomes due, RFC 3339, resolved against observed_at."},
                            "action": {"type": "string", "enum": ESCALATION_ACTIONS,
                                "description": "notify: tell the user the deadline passed and its consequence. ask: the user must decide something when it comes due."},
                        },
                        "required": ["condition", "due_at", "action"],
                        "additionalProperties": false,
                    },
                    "clear_escalation": {"type": "boolean",
                        "description": "Optional. True when the stored escalation's condition has been satisfied or the user withdrew it; the escalation is removed. Never combine with a new escalation."},
                },
                "required": ["attention"],
                "additionalProperties": false,
            }),
        },
        slot: slot.clone(),
        parse: parse_attention,
    });
    let user = serde_json::to_string_pretty(&json!({
        "observed_at": observed_at,
        "goal": goal_json(goal),
        "current_state": goal.current_state,
        "current_attention": goal.attention_kind,
        "triggering_activity": trigger,
        "recent_activity": recent.iter().take(10).map(entry_json).collect::<Vec<_>>(),
    }))
    .unwrap_or_default();
    if let Err(e) = ask(provider, &personal(ATTENTION_MD), user, tools, vec![SET_ATTENTION.into()]).await {
        eprintln!("goal attention inference failed; preserving existing attention: {e}");
    }
    let got = slot.lock().unwrap().clone();
    got
}

/// Состояние и заголовок записи при смене статуса дела. `None` — сбой
/// модели (берётся запасной текст, `lifecycle_fallback`).
pub async fn lifecycle<P: Provider>(provider: &P, goal: &Goal, completed: bool, reason: &str, recent: &[Entry]) -> Option<Lifecycle> {
    let slot: Slot<Lifecycle> = Arc::new(Mutex::new(None));
    let mut tools = Tools::new();
    tools.add(Decide {
        spec: ToolSpec {
            name: SET_LIFECYCLE_COPY.into(),
            description: "Set the user-facing state and activity title for this tracking lifecycle transition.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "current_state": {"type": "string", "description": "One short sentence with the durable state."},
                    "activity_title": {"type": "string", "description": "A concise event label."},
                },
                "required": ["current_state", "activity_title"],
                "additionalProperties": false,
            }),
        },
        slot: slot.clone(),
        parse: parse_lifecycle,
    });
    let evidence = json!({
        "transition": if completed { "completion" } else { "reopen" },
        "status_before_transition": goal.status,
        "lifecycle_reason": reason,
        "goal": goal_json(goal),
        "recent_activity": recent.iter().take(10).map(entry_json).collect::<Vec<_>>(),
        "current_state_hint": Value::Null,
    });
    let user = personal(LIFECYCLE_INPUT_MD).replace("{{evidence_json}}", &serde_json::to_string_pretty(&evidence).unwrap_or_default());
    let system = personal(if completed { COMPLETION_MD } else { REOPEN_MD });
    if let Err(e) = ask(provider, &system, user, tools, vec![SET_LIFECYCLE_COPY.into()]).await {
        eprintln!("tracking lifecycle copy inference failed; using deterministic fallback: {e}");
    }
    let got = slot.lock().unwrap().clone();
    got
}

/// Запасной заголовок записи смены статуса — заголовки ядра
/// ('Tracking completed', 'Tracking resumed', 'Goal completed',
/// 'Goal reactivated', выборка последней записи жизненного цикла в
/// sql/strings `17c2aab`).
pub fn lifecycle_fallback(tracking: bool, completed: bool) -> &'static str {
    match (tracking, completed) {
        (true, true) => "Tracking completed",
        (true, false) => "Tracking resumed",
        (false, true) => "Goal completed",
        (false, false) => "Goal reactivated",
    }
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
