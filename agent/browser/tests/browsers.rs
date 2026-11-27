//! Тесты П4к-1б: окна главного агента — `browser.browse` и
//! `browser.windows` на живом Chromium и поддельной странице, исполнитель —
//! на обоих поставщиках.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
mod site;

use std::time::Duration;

use agent_browser::browsers::Browsers;
use agent_loop::{Inbox, Tool};
use agent_model::{Message, ToolResultPart};
use common::{model, temp_dir, Step, Wire};
use serde_json::json;

async fn next(inbox: &Inbox) -> String {
    tokio::time::timeout(Duration::from_secs(30), inbox.wait()).await.expect("заметка не пришла");
    inbox.drain().iter().map(Message::text).collect::<Vec<_>>().join("\n")
}

async fn call(t: &dyn Tool, args: serde_json::Value) -> Result<String, String> {
    match t.call(args).await {
        Ok(parts) => Ok(parts.iter().filter_map(|p| match p { ToolResultPart::Text { text } => Some(text.clone()), _ => None }).collect::<Vec<_>>().join("\n")),
        Err(e) => Err(e.to_string()),
    }
}

fn find(text: &str, prefix: &str, len: usize) -> String {
    let i = text.find(prefix).unwrap_or_else(|| panic!("нет {prefix} в {text}"));
    text[i..i + prefix.len() + len].to_string()
}

#[tokio::test]
async fn browse_question_answer_handoff_continue_and_close() {
    let Some(bin) = site::chromium() else { return };
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let base = site::serve().await;
        let root = site::profile("browsers");
        let (server, provider) = model(
            wire,
            vec![
                Step::Calls(vec![("o1", "browser.observe", json!({"tab": "t1"}))]),
                Step::Calls(vec![("q1", "browser.request_input", json!({"question": "Which name and color?", "reason": "missing_info"}))]),
                Step::Calls(vec![(
                    "a1",
                    "browser.act",
                    json!({"tab": "t1", "intent": "Placing the order", "actions": [
                        {"kind": "fill", "ref": "e1", "text": "Ann Lee"},
                        {"kind": "select", "ref": "e2", "value": "blue"},
                        {"kind": "set_checked", "ref": "e5", "checked": true},
                        {"kind": "click", "ref": "e6"}]}),
                )]),
                Step::Calls(vec![("r1", "browser.report", json!({"status": "completed", "summary": "Order A-1042 placed for a blue mug."}))]),
                Step::Calls(vec![("a2", "browser.act", json!({"tab": "t1", "intent": "Opening details", "actions": [{"kind": "navigate", "url": format!("{base}/details")}]}))]),
                Step::Calls(vec![("r2", "browser.report", json!({"status": "completed", "summary": "The mug costs 12 EUR."}))]),
            ],
            true,
        );
        let inbox = Inbox::new();
        let b = Browsers::new(bin.clone(), root.0.clone(), true, std::sync::Arc::new(provider), "en-US".into(), temp_dir("browsers-spill"), inbox.clone());
        let (browse, windows) = b.tools();

        let r = call(&browse, json!({"task_id": "new", "task": "Order a blue mug for Ann Lee.", "url": format!("{base}/")})).await.unwrap();
        assert!(r.contains("A new window (task_id browser-task:") && r.contains("The browser has accepted the request."), "{wire:?}: {r}");
        let task_id = find(&r, "browser-task:", 36);

        let q = next(&inbox).await;
        assert!(q.contains("A browser task is waiting for your answer.") && q.contains("question: Which name and color?") && q.contains(&task_id), "{q}");
        let qid = find(&q, "question_id: question_", 16)["question_id: ".len()..].to_string();
        let list = call(&windows, json!({"action": "list"})).await.unwrap();
        assert!(list.contains(&format!("### Window {task_id}\nstatus: waiting_on_root")) && list.contains("Title: Fake shop"), "{list}");

        let r = call(&browse, json!({"task_id": task_id, "task": "Ann Lee, blue.", "answer_to": qid})).await.unwrap();
        assert_eq!(r, format!("Answered {qid} in the window (task_id {task_id})."));
        let h = next(&inbox).await;
        assert!(h.starts_with("## Browser Task Handoff") && h.contains(&format!("browser_task_id: {task_id}\nstatus: completed")), "{h}");
        assert!(h.contains("<browser_task_report>\nOrder A-1042 placed for a blue mug.\n</browser_task_report>") && h.contains("Compare the report with"), "{h}");

        // Окно живёт: продолжение той же задачи с историей.
        let r = call(&browse, json!({"task_id": task_id, "task": "Check the mug price on the details page."})).await.unwrap();
        assert!(r.starts_with(&format!("The window (task_id {task_id}) accepted the request as new work.")), "{r}");
        let h = next(&inbox).await;
        assert!(h.contains("The mug costs 12 EUR."), "{h}");
        let reqs = server.requests();
        let cont = reqs[4].to_string();
        assert!(cont.contains("Continue this task from the history above") && cont.contains("Order number A-1042"), "{wire:?}: продолжение с историей");
        assert!(reqs[2].to_string().contains("Ann Lee, blue."), "ответ дошёл до шага, который спросил");

        let list = call(&windows, json!({"action": "list"})).await.unwrap();
        assert!(list.contains("status: idle") && list.contains(&format!("URL: {base}/details")), "{list}");
        let r = call(&windows, json!({"action": "close", "task_id": task_id})).await.unwrap();
        assert!(r.starts_with("Closed the window"), "{r}");
        assert!(call(&windows, json!({"action": "list"})).await.unwrap().contains("No windows are open."));
        let e = call(&browse, json!({"task_id": task_id, "task": "more"})).await.unwrap_err();
        assert!(e.contains("No window has task_id"), "{e}");
        b.shutdown().await;
    }
}

/// Обновление, пока работа идёт, попадает исполнителю между шагами; окно
/// без работы открывается `browser.windows open`.
#[tokio::test]
async fn update_while_running_reaches_planner() {
    let Some(bin) = site::chromium() else { return };
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let base = site::serve().await;
        let root = site::profile("browsers-update");
        let (server, provider) = model(
            wire,
            vec![
                Step::Calls(vec![("w1", "browser.act", json!({"tab": "t1", "intent": "Waiting", "actions": [{"kind": "wait", "text": "never", "condition": "visible", "timeout_ms": 1500}]}))]),
                Step::Calls(vec![("r1", "browser.report", json!({"status": "completed", "summary": "Done with the update."}))]),
            ],
            true,
        );
        let inbox = Inbox::new();
        let b = Browsers::new(bin.clone(), root.0.clone(), true, std::sync::Arc::new(provider), "en-US".into(), temp_dir("browsers-spill"), inbox.clone());
        let (browse, windows) = b.tools();
        let r = call(&windows, json!({"action": "open", "url": format!("{base}/help")})).await.unwrap();
        assert!(r.contains("starts no browser planner") && r.contains("Title: Help"), "{r}");
        let task_id = find(&r, "browser-task:", 36);
        call(&browse, json!({"task_id": task_id, "task": "Read the help page."})).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        let r = call(&browse, json!({"task_id": task_id, "task": "Also note the shipping time."})).await.unwrap();
        assert!(r.starts_with(&format!("Sent to the window (task_id {task_id}) as an update.")), "{wire:?}: {r}");
        let h = next(&inbox).await;
        assert!(h.contains("Done with the update."), "{h}");
        assert!(server.requests()[1].to_string().contains("Also note the shipping time."), "обновление — исполнителю между шагами");
        b.shutdown().await;
    }
}
