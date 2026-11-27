//! Инструменты `workflow.*` — полный список ядра (профиль
//! `foreground_root`, `strings-all.txt:70229-70242`), описания и поля —
//! тексты ядра (`strings-all.txt:332317-332347`, `prompts/workflows/
//! tools.json`, правки — в `prompts/SOURCES.md`). Поля — как у muse,
//! camelCase; варианты snake_case (`script_path`, `resume_from_run_id`)
//! тоже принимаются (`strings-all.txt:75970`).
//!
//! Описание пространства и договор написания сценария
//! (`prompts/workflows/workflow_namespace.md`) у muse приходят при загрузке
//! пространства; поиска инструментов у нас нет — договор прикреплён к
//! инструментам, чьи поля на него ссылаются («authored per the contract in
//! this tool's description»): `create`, `update`, `launch`, `launch_async`.
//! Своё: у поля `limit` инструмента `list_runs` описания в разборе нет.

use std::sync::{Arc, LazyLock};

use agent_loop::{personal, Tool, ToolError, ToolFuture};
use agent_model::{Provider, ToolResultPart, ToolSpec};
use serde_json::{json, Map, Value};

use crate::{saved, Workflows};

const NAMESPACE: &str = include_str!("../../../prompts/workflows/workflow_namespace.md");
const TEXTS_JSON: &str = include_str!("../../../prompts/workflows/tools.json");

static TEXTS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(TEXTS_JSON).expect("prompts/workflows/tools.json"));

fn text(group: &str, key: &str) -> String {
    personal(TEXTS[group][key].as_str().unwrap_or_default())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Create,
    Delete,
    Launch,
    LaunchAsync,
    List,
    ListRuns,
    Pause,
    Resume,
    SaveRun,
    Stop,
    StopAgent,
    Update,
    View,
    ViewRun,
}

impl Op {
    pub const ALL: [Op; 14] = [Op::Create, Op::Delete, Op::Launch, Op::LaunchAsync, Op::List, Op::ListRuns, Op::Pause, Op::Resume, Op::SaveRun, Op::Stop, Op::StopAgent, Op::Update, Op::View, Op::ViewRun];

    pub fn name(self) -> &'static str {
        match self {
            Op::Create => "workflow.create",
            Op::Delete => "workflow.delete",
            Op::Launch => "workflow.launch",
            Op::LaunchAsync => "workflow.launch_async",
            Op::List => "workflow.list",
            Op::ListRuns => "workflow.list_runs",
            Op::Pause => "workflow.pause",
            Op::Resume => "workflow.resume",
            Op::SaveRun => "workflow.save_run",
            Op::Stop => "workflow.stop",
            Op::StopAgent => "workflow.stop_agent",
            Op::Update => "workflow.update",
            Op::View => "workflow.view",
            Op::ViewRun => "workflow.view_run",
        }
    }
}

pub struct WorkflowTool<P> {
    op: Op,
    w: Arc<Workflows<P>>,
}

impl<P: Provider + Send + Sync + 'static> Workflows<P> {
    /// Инструменты `workflow.*` над службой.
    pub fn tools(self: &Arc<Self>) -> Vec<WorkflowTool<P>> {
        Op::ALL.into_iter().map(|op| WorkflowTool { op, w: self.clone() }).collect()
    }
}

fn s(desc: String) -> Value {
    json!({"type": "string", "description": desc})
}

fn bad(s: impl Into<String>) -> ToolError {
    ToolError::InvalidArguments(s.into())
}

fn need(args: &Value, k: &str) -> Result<String, ToolError> {
    args.get(k).and_then(Value::as_str).map(String::from).ok_or_else(|| bad(format!("`{k}` (string) is required")))
}

fn run_id(args: &Value) -> Result<String, ToolError> {
    args.get("runId").or_else(|| args.get("run_id")).and_then(Value::as_str).map(String::from).ok_or_else(|| bad("`runId` (string) is required"))
}

/// `limit`: по умолчанию 100, не больше 200.
fn limit(args: &Value) -> usize {
    args.get("limit").and_then(Value::as_u64).map(|n| n.clamp(1, 200) as usize).unwrap_or(100)
}

impl<P> WorkflowTool<P> {
    fn schema(&self) -> Value {
        let mut p = Map::new();
        let mut required: Vec<&str> = vec![];
        let run = || s(text("fields", "runId"));
        match self.op {
            Op::List => {
                p.insert("limit".into(), json!({"type": "integer", "minimum": 1, "description": text("fields", "list.limit")}));
            }
            Op::ListRuns => {
                p.insert("limit".into(), json!({"type": "integer", "minimum": 1}));
            }
            Op::View | Op::Delete => {
                let key = if self.op == Op::View { "view.name" } else { "delete.name" };
                p.insert("name".into(), s(text("fields", key)));
                required.push("name");
            }
            Op::Create | Op::Update => {
                let pre = if self.op == Op::Create { "create" } else { "update" };
                p.insert("name".into(), s(text("fields", &format!("{pre}.name"))));
                p.insert("script".into(), s(text("fields", &format!("{pre}.script"))));
                required.extend(["name", "script"]);
            }
            Op::Launch | Op::LaunchAsync => {
                p.insert("scriptPath".into(), s(text("fields", "launch.scriptPath")));
                p.insert("script".into(), s(text("fields", "launch.script")));
                p.insert("name".into(), s(text("fields", "launch.name")));
                p.insert("args".into(), json!({"description": text("fields", "launch.args")}));
                p.insert("resumeFromRunId".into(), s(text("fields", "launch.resumeFromRunId")));
            }
            Op::Pause | Op::Resume | Op::Stop => {
                let key = match self.op {
                    Op::Pause => "pause.runId",
                    Op::Resume => "resume.runId",
                    _ => "stop.runId",
                };
                p.insert("runId".into(), s(text("fields", key)));
                required.push("runId");
            }
            Op::ViewRun => {
                p.insert("runId".into(), run());
                required.push("runId");
            }
            Op::SaveRun => {
                p.insert("runId".into(), run());
                p.insert("name".into(), s(text("fields", "save_run.name")));
                required.push("runId");
            }
            Op::StopAgent => {
                p.insert("runId".into(), run());
                p.insert("callId".into(), s(text("fields", "stop_agent.callId")));
                required.extend(["runId", "callId"]);
            }
        }
        json!({"type": "object", "properties": p, "required": required, "additionalProperties": false})
    }
}

fn out(head: &str, v: &Value) -> Vec<ToolResultPart> {
    let body = serde_json::to_string_pretty(v).unwrap_or_default();
    let text = if head.is_empty() { body } else { format!("{head}\n\n{body}") };
    vec![ToolResultPart::Text { text }]
}

impl<P: Provider + Send + Sync + 'static> Tool for WorkflowTool<P> {
    fn spec(&self) -> ToolSpec {
        let mut description = text("tools", self.op.name());
        if matches!(self.op, Op::Create | Op::Update | Op::Launch | Op::LaunchAsync) {
            description = format!("{description}\n\n{}", personal(NAMESPACE).trim_end());
        }
        ToolSpec { name: self.op.name().into(), description, input_schema: self.schema() }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let ws = self.w.workspace();
            let fail = ToolError::Failed;
            let (head, v): (String, Value) = match self.op {
                Op::List => {
                    let list = saved::list(ws, limit(&args));
                    if list.is_empty() {
                        return Ok(vec![ToolResultPart::Text { text: "No saved workflows found.".into() }]);
                    }
                    (String::new(), json!({"workflows": list}))
                }
                Op::View => (String::new(), saved::describe(ws, &need(&args, "name")?, true).map_err(fail)?),
                Op::Create => {
                    let name = need(&args, "name")?;
                    (String::new(), saved::write(ws, &name, &need(&args, "script")?, Some(false)).map_err(fail)?)
                }
                Op::Update => {
                    let name = need(&args, "name")?;
                    (String::new(), saved::write(ws, &name, &need(&args, "script")?, Some(true)).map_err(fail)?)
                }
                Op::Delete => {
                    let name = need(&args, "name")?;
                    let v = saved::delete(ws, &name).map_err(fail)?;
                    (format!("Deleted saved workflow `{name}`."), v)
                }
                Op::Launch => self.w.launch(&args, "sync").await.map_err(fail)?,
                Op::LaunchAsync => self.w.launch(&args, "async").await.map_err(fail)?,
                Op::ListRuns => (String::new(), json!({"runs": self.w.list_runs(limit(&args) as i64).await.map_err(fail)?})),
                Op::ViewRun => (String::new(), self.w.view_run(&run_id(&args)?).await.map_err(fail)?),
                Op::Pause => self.w.pause(&run_id(&args)?).await.map_err(fail)?,
                Op::Resume => self.w.resume(&run_id(&args)?).await.map_err(fail)?,
                Op::Stop => self.w.stop(&run_id(&args)?).await.map_err(fail)?,
                Op::StopAgent => {
                    let call = args.get("callId").or_else(|| args.get("call_id")).and_then(Value::as_str).ok_or_else(|| bad("workflow control action `stop_agent` requires call_id"))?;
                    self.w.stop_agent(&run_id(&args)?, call).await.map_err(fail)?
                }
                Op::SaveRun => self.w.save_run(&run_id(&args)?, args.get("name").and_then(Value::as_str)).await.map_err(fail)?,
            };
            Ok(out(&head, &v))
        })
    }
}
