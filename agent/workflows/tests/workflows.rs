//! Тесты П4и: сохранённые многошаговые работы. Модели шагов — записанные
//! ответы на обоих проводах, база — временный Postgres.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use agent_loop::{Inbox, Tool, Tools};
use agent_model::{Message, Part};
use agent_workflows::store::Store;
use agent_workflows::Workflows;
use common::{model, temp_dir, AnyProvider, Server, Step, Wire};
use serde_json::{json, Value};

const TRIP: &str = r#"export const meta = { name: "trip", description: "Plan a trip with the user's city", phases: ["gather", "plan"] };
await phase("gather");
const dates = await agent("Find the user's free dates in the calendar notes.", { key: "dates", label: "Dates" });
const city = await agent("Ask the user which city the trip should go to.", { key: "city", label: "City" });
await phase("plan");
const plan = await agent(`Plan a trip to ${city} for ${args.travelers} travelers.`, {
  key: "plan",
  label: "Plan",
  schema: { type: "object", required: ["days"], properties: { days: { type: "integer" } }, additionalProperties: false },
});
return `Trip to ${city} for ${args.travelers}: ${plan.days} days, ${dates}`;
"#;

struct Env {
    w: Arc<Workflows<AnyProvider>>,
    tools: HashMap<String, Box<dyn Tool>>,
    inbox: Inbox,
    server: Server,
}

async fn env(name: &str, wire: Wire, steps: Vec<Step>) -> Env {
    let (server, provider) = model(wire, steps, false);
    let dir = temp_dir(&format!("workflows-{name}"));
    let ws = dir.join("home/workspace");
    std::fs::create_dir_all(&ws).unwrap();
    let store = Store::connect(&pg::database()).await.unwrap();
    let inbox = Inbox::new();
    let w = Workflows::new(store, Arc::new(provider), Arc::new(|_: &Path| Tools::new()), String::new(), dir.join("home"), ws, dir.join("spill"), inbox.clone());
    let tools = w.tools().into_iter().map(|t| (t.spec().name, Box::new(t) as Box<dyn Tool>)).collect();
    Env { w, tools, inbox, server }
}

impl Env {
    async fn call(&self, name: &str, args: Value) -> Result<String, String> {
        let parts = self.tools[name].call(args).await.map_err(|e| e.to_string())?;
        Ok(parts
            .into_iter()
            .map(|p| match p {
                agent_model::ToolResultPart::Text { text } => text,
                _ => String::new(),
            })
            .collect())
    }

    async fn note(&self) -> String {
        tokio::time::timeout(std::time::Duration::from_secs(20), self.inbox.wait()).await.expect("handoff in inbox");
        let notes: Vec<Message> = self.inbox.drain();
        assert_eq!(notes.len(), 1, "{notes:?}");
        notes
            .into_iter()
            .flat_map(|m| m.content)
            .filter_map(|p| match p {
                Part::Text { text } => Some(text),
                _ => None,
            })
            .collect()
    }
}

/// JSON после строки-квитанции.
fn body(out: &str) -> Value {
    let i = out.find('{').unwrap_or_else(|| panic!("no json: {out}"));
    serde_json::from_str(&out[i..]).unwrap()
}

fn request_text(r: &Value) -> String {
    r.to_string()
}

async fn blocked_step_waits_for_answer_then_resume_finishes(wire: Wire) {
    let e = env(
        "blocked",
        wire,
        vec![
            Step::Text(r#"{"status":"ok","result":"May 3-6"}"#),
            Step::Text(r#"{"status":"blocked","result":{"blocked_reason":"needs_user_input","message":"Which city should the trip go to?"}}"#),
            Step::Text(r#"{"status":"ok","result":"Lisbon"}"#),
            Step::Text(r#"{"status":"ok","result":{"days":3,"note":"extra fields are fine"}}"#),
        ],
    )
    .await;
    let created = e.call("workflow.create", json!({"name": "trip", "script": TRIP})).await.unwrap();
    assert_eq!(body(&created)["phases"], json!(["gather", "plan"]));
    assert!(e.call("workflow.create", json!({"name": "trip", "script": TRIP})).await.unwrap_err().contains("saved workflow `trip` already exists"));
    let listed = e.call("workflow.list", json!({})).await.unwrap();
    assert_eq!(body(&listed)["workflows"][0]["workflowName"], "trip");

    let launched = e.call("workflow.launch_async", json!({"name": "trip", "args": {"travelers": 2}})).await.unwrap();
    assert!(launched.starts_with("Workflow started in background."), "{launched}");
    let run_id = body(&launched)["runId"].as_str().unwrap().to_string();

    // Шаг ждёт ответа: запуск на паузе, главному агенту — весть.
    let note = e.note().await;
    assert!(note.contains(&format!("run_id: {run_id}")), "{note}");
    assert!(note.contains("Workflow `trip` blocked: Which city should the trip go to?"), "{note}");
    let view = body(&e.call("workflow.view_run", json!({"runId": run_id})).await.unwrap());
    assert_eq!(view["status"], "paused");
    assert_eq!(view["agentCalls"].as_array().unwrap().len(), 2);
    assert_eq!(view["agentCalls"][0]["status"], "completed");
    assert_eq!(view["agentCalls"][1]["status"], "blocked");
    assert_eq!(view["agentCalls"][1]["label"], "City");
    assert_eq!(view["phases"][0]["status"], "running");
    assert_eq!(view["phases"][1]["status"], "pending");

    // Ответ получен — продолжить тот же запуск.
    let resumed = e.call("workflow.resume", json!({"runId": run_id})).await.unwrap();
    assert!(resumed.starts_with("Applied workflow control `resume`."), "{resumed}");
    let note = e.note().await;
    assert!(note.contains("Workflow `trip` completed."), "{note}");
    assert!(note.ends_with("Trip to Lisbon for 2: 3 days, May 3-6"), "{note}");

    let view = body(&e.call("workflow.view_run", json!({"runId": run_id})).await.unwrap());
    assert_eq!(view["status"], "completed");
    assert_eq!(view["finalResult"], "Trip to Lisbon for 2: 3 days, May 3-6");
    assert_eq!(view["phases"][0]["status"], "completed");
    assert_eq!(view["phases"][1]["status"], "completed");
    assert_eq!(view["phases"][1]["agentCompleted"], 1);
    let calls = view["agentCalls"].as_array().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1]["replayKey"], "explicit:city");
    assert_eq!(calls[1]["status"], "completed");
    assert_eq!(calls[1]["result"], "Lisbon");
    assert_eq!(calls[2]["result"]["days"], 3);

    // Готовый шаг при повторе сценария взят из базы: модель позвана
    // четыре раза, не пять. Шаг получает договор вывода, задачу и общую
    // папку; со схемой — схему результата.
    let reqs = e.server.requests();
    assert_eq!(reqs.len(), 4);
    let first = request_text(&reqs[1]);
    assert!(first.contains("## Workflow Agent Output Contract"), "{first}");
    assert!(first.contains("Ask the user which city the trip should go to."));
    assert!(first.contains("must be a plain string message"));
    assert!(first.contains(".jarvis/workflow-runs/workflow-run-"));
    assert!(!first.contains("workflow.launch"), "у шага нет инструментов работ");
    let third = request_text(&reqs[3]);
    assert!(third.contains("Plan a trip to Lisbon for 2 travelers."), "{third}");
    assert!(third.contains("must validate against the JSON Schema below"));
    assert!(third.contains("\\\"days\\\""));

    let runs = body(&e.call("workflow.list_runs", json!({})).await.unwrap());
    assert_eq!(runs["runs"][0]["runId"], run_id.as_str());
    assert!(e.call("workflow.resume", json!({"runId": run_id})).await.unwrap_err().contains("invalid workflow control: cannot resume workflow run"));
}

#[tokio::test]
async fn blocked_step_waits_for_answer_then_resume_finishes_anthropic() {
    blocked_step_waits_for_answer_then_resume_finishes(Wire::Anthropic).await;
}

#[tokio::test]
async fn blocked_step_waits_for_answer_then_resume_finishes_openai_compat() {
    blocked_step_waits_for_answer_then_resume_finishes(Wire::OpenaiCompat).await;
}

const PAIR: &str = r#"export const meta = { name: "pair", description: "Two steps", phases: ["work"] };
await phase("work");
const a = await agent("Step one.", { key: "one" });
const b = await agent("Step two.", { key: "two" });
return `${a} + ${b}`;
"#;

async fn failed_sync_run_retries_from_cached_steps(wire: Wire) {
    let e = env(
        "retry",
        wire,
        vec![
            Step::Text(r#"{"status":"ok","result":"A"}"#),
            Step::Text(r#"{"status":"error","result":{"message":"service unavailable"}}"#),
            Step::Text(r#"{"status":"ok","result":"B"}"#),
        ],
    )
    .await;
    let failed = e.call("workflow.launch", json!({"script": PAIR})).await.unwrap();
    assert!(failed.starts_with("Workflow `pair` failed."), "{failed}");
    let failed = body(&failed);
    assert_eq!(failed["run"]["error"], "service unavailable");
    let src = failed["run"]["runId"].as_str().unwrap().to_string();

    let done = e.call("workflow.launch", json!({"script": PAIR, "resumeFromRunId": src})).await.unwrap();
    assert!(done.starts_with("Workflow `pair` completed."), "{done}");
    assert_eq!(body(&done)["finalResult"], "A + B");
    let run_id = body(&done)["run"]["runId"].as_str().unwrap().to_string();
    let view = body(&e.call("workflow.view_run", json!({"runId": run_id})).await.unwrap());
    assert_eq!(view["agentCalls"][0]["status"], "cached");
    assert!(view["agentCalls"][0]["cachedFromCallId"].is_string());
    assert_eq!(view["agentCalls"][1]["status"], "completed");
    // Первый шаг взят из прошлого запуска — модель позвана трижды, не четырежды.
    assert_eq!(e.server.requests().len(), 3);
    // Синхронный запуск отвечает сам — в ящике пусто.
    assert!(e.inbox.drain().is_empty());

    let saved = e.call("workflow.save_run", json!({"runId": run_id, "name": "pair-copy"})).await.unwrap();
    assert_eq!(body(&saved)["workflowName"], "pair");
    assert!(e.call("workflow.view", json!({"name": "pair-copy"})).await.unwrap().contains("Step two."));
    let other = e.call("workflow.launch", json!({"name": "trip", "resumeFromRunId": run_id})).await.unwrap_err();
    assert!(other.contains("workflow script `trip` was not found"), "{other}");
}

#[tokio::test]
async fn failed_sync_run_retries_from_cached_steps_anthropic() {
    failed_sync_run_retries_from_cached_steps(Wire::Anthropic).await;
}

#[tokio::test]
async fn failed_sync_run_retries_from_cached_steps_openai_compat() {
    failed_sync_run_retries_from_cached_steps(Wire::OpenaiCompat).await;
}

async fn saved_workflow_management(wire: Wire) {
    let e = env("saved", wire, vec![]).await;
    assert_eq!(e.call("workflow.list", json!({})).await.unwrap(), "No saved workflows found.");
    assert!(e.call("workflow.update", json!({"name": "trip", "script": TRIP})).await.unwrap_err().contains("workflow script `trip` was not found"));
    assert!(e.call("workflow.create", json!({"name": "bad name", "script": TRIP})).await.unwrap_err().contains("expected only ASCII letters"));
    assert!(e.call("workflow.create", json!({"name": "x", "script": "return 1;"})).await.unwrap_err().contains("invalid workflow metadata: missing `export const meta`"));
    e.call("workflow.create", json!({"name": "trip", "script": TRIP})).await.unwrap();
    let edited = TRIP.replace("Plan a trip with the user's city", "Plan a trip");
    let updated = body(&e.call("workflow.update", json!({"name": "trip", "script": edited})).await.unwrap());
    assert_eq!(updated["description"], "Plan a trip");
    let viewed = body(&e.call("workflow.view", json!({"name": "trip"})).await.unwrap());
    assert!(viewed["scriptContent"].as_str().unwrap().contains("Plan a trip\""));
    assert!(viewed["scriptPath"].as_str().unwrap().ends_with(".jarvis/workflows/trip.js"));
    let deleted = e.call("workflow.delete", json!({"name": "trip"})).await.unwrap();
    assert!(deleted.starts_with("Deleted saved workflow `trip`."));
    assert!(body(&deleted)["scriptContent"].is_string());
    assert!(e.call("workflow.view", json!({"name": "trip"})).await.unwrap_err().contains("workflow script `trip` was not found"));
    assert!(e.call("workflow.launch", json!({})).await.unwrap_err().contains("workflow input requires one of scriptPath, script, or name"));
    assert!(e.call("workflow.launch", json!({"scriptPath": "../../etc/x.js"})).await.unwrap_err().contains("must stay inside workspace root"));
    let syntax = e.call("workflow.launch", json!({"script": "export const meta = { name: \"s\", description: \"d\", phases: [\"a\"] };\nreturn (;"})).await.unwrap();
    assert!(syntax.starts_with("Workflow `s` failed."), "{syntax}");
    assert_eq!(body(&syntax)["run"]["status"], "syntax_error");
    let mut names: Vec<String> = e.tools.keys().cloned().collect();
    names.sort();
    assert_eq!(
        names,
        [
            "workflow.create", "workflow.delete", "workflow.launch", "workflow.launch_async", "workflow.list", "workflow.list_runs", "workflow.pause",
            "workflow.resume", "workflow.save_run", "workflow.stop", "workflow.stop_agent", "workflow.update", "workflow.view", "workflow.view_run",
        ]
    );
    let _ = &e.w;
}

#[tokio::test]
async fn saved_workflow_management_anthropic() {
    saved_workflow_management(Wire::Anthropic).await;
}

#[tokio::test]
async fn saved_workflow_management_openai_compat() {
    saved_workflow_management(Wire::OpenaiCompat).await;
}
