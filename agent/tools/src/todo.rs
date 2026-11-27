//! `todo.write` — список дел внутри хода (`todo_tool.rs`,
//! `source-files.txt:1797-1798`). Описание и тексты ошибок — строки ядра
//! muse (0xa3f121–0xa3f9a0, 0x2cd1b55), личный вариант.

use std::sync::{Arc, Mutex};

use agent_loop::{personal, Tool, ToolError, ToolFuture};
use agent_model::{ToolResultPart, ToolSpec};
use serde_json::{json, Value};

const DESCRIPTION: &str = "Records your working todo list for the current task, replacing the previous list. Use it for a task with three or more distinct steps, or when several requests need to stay in view. Skip it for a quick answer or a single action.\n\nAlways send the full list, with at most 25 items, and preserve item text when updating its status. Keep one item `in_progress` while you can work and leave work you have not started `pending`. Continue actionable work until it is complete or genuinely blocked. Keep only unfinished and blocked work. Before ending your turn, remove completed and cancelled items, including work you just finished and verified. You can report those results after updating the list. Send an empty list when nothing remains.\n\nMark an item `blocked` when progress depends on new input or a result. While you are waiting for a running agent or background result, mark the affected items `blocked`. Resume them when the result arrives. Mark dependent work `blocked` too if it cannot advance on its own. Before ending your turn, explain each new blocker, what would unblock it and how <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->a member<!-- group-only:end --> can help if needed. Continue independent work. Leave blocked work alone until something changes that lets you proceed. Do not use `blocked` for work you can still advance.";
const STATUS: &str = "Use `pending` for work not started, `in_progress` for the one active item, `blocked` when progress depends on new input or a result, `completed` when the outcome is established, and `cancelled` when it no longer applies.";
const TODOS: &str = "The complete todo list; this call replaces the previous list.";
const TEXT: &str = "One concrete step, phrased as an action.";

pub const MAX_ITEMS: usize = 25;
const STATUSES: [&str; 5] = ["pending", "in_progress", "blocked", "completed", "cancelled"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub text: String,
    pub status: String,
}

/// Список дел разговора.
#[derive(Clone, Default)]
pub struct Todo {
    list: Arc<Mutex<Vec<TodoItem>>>,
}

impl Todo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Vec<TodoItem> {
        self.list.lock().unwrap().clone()
    }
}

impl Tool for Todo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todo.write".into(),
            description: personal(DESCRIPTION),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "maxItems": MAX_ITEMS,
                        "description": TODOS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": {"type": "string", "description": TEXT},
                                "status": {"type": "string", "enum": STATUSES, "description": STATUS},
                            },
                            "required": ["text", "status"],
                        },
                    },
                },
                "required": ["todos"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let raw = args
                .get("todos")
                .and_then(Value::as_array)
                .ok_or_else(|| ToolError::InvalidArguments("`todos` (array) is required".into()))?;
            if raw.len() > MAX_ITEMS {
                return Err(ToolError::InvalidArguments(format!("at most {MAX_ITEMS} items")));
            }
            let mut items = Vec::new();
            for r in raw {
                let text = r.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();
                if text.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "every todo item needs non-empty text; write the step it stands for".into(),
                    ));
                }
                let status = r.get("status").and_then(Value::as_str).unwrap_or("");
                if !STATUSES.contains(&status) {
                    return Err(ToolError::InvalidArguments(format!(
                        "status must be one of pending, in_progress, blocked, completed, cancelled, not '{status}'"
                    )));
                }
                items.push(TodoItem { text, status: status.into() });
            }
            let active = items.iter().filter(|i| i.status == "in_progress").count();
            let pending = items.iter().any(|i| i.status == "pending");
            if active > 1 {
                return Err(ToolError::InvalidArguments(
                    "keep exactly one item in_progress: mark only the step you are working on right now in_progress, leave the rest pending, and send the full list again".into(),
                ));
            }
            if active == 0 && pending {
                return Err(ToolError::InvalidArguments(
                    "no item is in_progress; mark the step you are working on now when work continues".into(),
                ));
            }
            let n = items.len();
            *self.list.lock().unwrap() = items;
            Ok(vec![ToolResultPart::Text { text: json!({"items": n, "in_progress": active}).to_string() }])
        })
    }
}
