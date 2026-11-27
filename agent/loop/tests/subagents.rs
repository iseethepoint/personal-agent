//! Тесты П3г: подагенты. Каждый тест гоняется дважды — через поддельный
//! сервер anthropic и через поддельный openai_compat.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_loop::{first_message, Inbox, Prefix, Status, Subagents, Tools, Turn, CONTROL_TIMEOUT};
use agent_model::{Message, Part, Role, ToolResultPart};
use common::{model, temp_dir, tools, AnyProvider, Server, Step, Wire};
use serde_json::{json, Value};

macro_rules! both {
    ($name:ident) => {
        mod $name {
            use super::*;
            #[tokio::test(flavor = "multi_thread")]
            async fn anthropic() {
                super::$name(Wire::Anthropic).await
            }
            #[tokio::test(flavor = "multi_thread")]
            async fn openai_compat() {
                super::$name(Wire::OpenaiCompat).await
            }
        }
    };
}

/// Подагенты на своём сервере; инструменты подагента — тестовые.
fn children(wire: Wire, steps: Vec<Step>) -> (Server, Subagents<AnyProvider>) {
    let (server, p) = model(wire, steps, false);
    let child_tools = tools();
    let prefix = Prefix { system: "Ты — подагент.".into(), tools: child_tools.specs() };
    let s = Subagents::new(Arc::new(p), Arc::new(child_tools), prefix, temp_dir("child"), Inbox::new());
    (server, s)
}

/// Вызвать инструмент `subagent.*` так, как его вызвал бы цикл.
async fn call(s: &Subagents<AnyProvider>, name: &str, args: Value) -> Result<Value, String> {
    let mut t = Tools::new();
    for tool in s.tools() {
        t.add(tool);
    }
    match t.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

async fn wait(inbox: &Inbox) -> Vec<Message> {
    tokio::time::timeout(Duration::from_secs(5), inbox.wait()).await.expect("итог не пришёл");
    inbox.drain()
}

async fn spawn_and_handoff(wire: Wire) {
    let (child_server, subs) = children(wire, vec![Step::Text("Ответ: 42")]);
    let mut parent_tools = tools();
    for tool in subs.tools() {
        parent_tools.add(tool);
    }
    let parent_steps = vec![
        Step::Calls(vec![("p1", "subagent.spawn", json!({"message": "посчитай ответ"}))]),
        Step::Text("Поручил подагенту."),
        Step::Text("Подагент ответил: 42"),
    ];
    let (parent_server, pp) = model(wire, parent_steps, false);
    let mut turn = Turn::new(&pp, &parent_tools, "Ты — родитель.", temp_dir("parent"));
    turn.inbox = Some(subs.inbox().clone());
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "узнай ответ".into() }] }];

    // Ход родителя: spawn сразу отдаёт id, итог ещё не пришёл.
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Поручил подагенту.");
    let spawned = match &h[2].content[0] {
        Part::ToolResult { content, is_error: false, .. } => match &content[0] {
            ToolResultPart::Text { text } => serde_json::from_str::<Value>(text).unwrap(),
            _ => panic!(),
        },
        other => panic!("{other:?}"),
    };
    let id = spawned["agent_id"].as_str().unwrap().to_string();
    assert_eq!(spawned["status"], "running");

    // Итог приходит сам; следующий ход родителя видит его в истории.
    tokio::time::timeout(Duration::from_secs(5), subs.inbox().wait()).await.unwrap();
    assert_eq!(subs.status(&id), Some(Status::Done));
    h.push(Message { role: Role::User, content: vec![Part::Text { text: "ну что?".into() }] });
    turn.run(&mut h, &mut |_: &str| {}).await.unwrap();
    let last = parent_server.requests()[2]["messages"].to_string();
    assert!(last.contains(&format!("[Subagent Result] agent_id: {id}, status: done")), "{last}");
    assert!(last.contains("Ответ: 42"));

    // Подагент работал со своей историей: текст muse и задача.
    let child = child_server.requests()[0].to_string();
    assert!(child.contains("[Subagent Context] You are running as a subagent (depth 1/1)"));
    assert!(child.contains("[Subagent Task]: посчитай ответ"));
    assert!(child.contains("You are running in isolated mode."));
    assert!(!child.contains("{{") && !child.contains("<!--") && !child.contains("узнай ответ"));
    // Описания инструментов родителя — тексты muse.
    let tools_json = parent_server.requests()[0]["tools"].to_string();
    assert!(tools_json.contains("Delegate a task to a new child agent that works on it in the background while you continue."));
    for name in ["subagent.spawn", "subagent.send", "subagent.list", "subagent.close", "subagent.resume"] {
        assert!(tools_json.contains(&format!("\"{name}\"")), "{name}");
    }
}
both!(spawn_and_handoff);

async fn two_in_parallel(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![("c1", "slow", json!({"ms": 400}))]),
        Step::Calls(vec![("c1", "slow", json!({"ms": 400}))]),
        Step::Text("итог"),
        Step::Text("итог"),
    ];
    let (_server, subs) = children(wire, steps);
    let started = Instant::now();
    let a = call(&subs, "subagent.spawn", json!({"message": "раз"})).await.unwrap();
    let b = call(&subs, "subagent.spawn", json!({"message": "два"})).await.unwrap();
    let mut got = Vec::new();
    while got.len() < 2 {
        got.extend(wait(subs.inbox()).await);
    }
    assert!(started.elapsed() < Duration::from_millis(750), "не параллельно: {:?}", started.elapsed());
    let list = call(&subs, "subagent.list", json!({})).await.unwrap();
    assert_eq!(list["active"], json!([]));
    let ids: Vec<&str> = list["recent"].as_array().unwrap().iter().map(|e| e["agent_id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec![a["agent_id"].as_str().unwrap(), b["agent_id"].as_str().unwrap()]);
    assert!(list["recent"].as_array().unwrap().iter().all(|e| e["status"] == "done"));
}
both!(two_in_parallel);

async fn send_to_running(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![("c1", "slow", json!({"ms": 300}))]),
        Step::Text("первое"),
        Step::Text("с учётом уточнения"),
    ];
    let (server, subs) = children(wire, steps);
    let a = call(&subs, "subagent.spawn", json!({"message": "работа"})).await.unwrap();
    let id = a["agent_id"].as_str().unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(subs.status(id), Some(Status::Running));
    let r = call(&subs, "subagent.send", json!({"agent_id": id, "message": "уточнение: короче"})).await.unwrap();
    assert_eq!(r["submitted"], true);
    // Итог — один, после уточнения.
    let got = wait(subs.inbox()).await;
    assert_eq!(got.len(), 1);
    assert!(got[0].text().contains("с учётом уточнения"));
    let reqs = server.requests();
    assert_eq!(reqs.len(), 3);
    assert!(reqs[2]["messages"].to_string().contains("уточнение: короче"));
    // Неизвестный id — ошибка.
    assert!(call(&subs, "subagent.send", json!({"agent_id": "nope", "message": "x"})).await.unwrap_err().contains("nope"));
}
both!(send_to_running);

async fn close_stops(wire: Wire) {
    let (_server, subs) = children(wire, vec![Step::Calls(vec![("c1", "slow", json!({"ms": 10_000}))])]);
    assert_eq!(CONTROL_TIMEOUT, Duration::from_secs(90));
    let a = call(&subs, "subagent.spawn", json!({"message": "долго"})).await.unwrap();
    let id = a["agent_id"].as_str().unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let started = Instant::now();
    let r = call(&subs, "subagent.close", json!({"agent_id": id})).await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(r["status"], "closed");
    assert_eq!(subs.status(id), Some(Status::Closed));
    // Итога нет, новая работа не принимается.
    assert!(tokio::time::timeout(Duration::from_millis(300), subs.inbox().wait()).await.is_err());
    assert!(call(&subs, "subagent.send", json!({"agent_id": id, "message": "x"})).await.is_err());
    let list = call(&subs, "subagent.list", json!({})).await.unwrap();
    assert_eq!(list["recent"][0]["status"], "closed");
}
both!(close_stops);

async fn resume_interrupted(wire: Wire) {
    // Временная ошибка модели после повтора — подагент прерван.
    let steps = vec![Step::Status(500), Step::Status(503), Step::Text("продолжил и закончил")];
    let (_server, subs) = children(wire, steps);
    let a = call(&subs, "subagent.spawn", json!({"message": "работа"})).await.unwrap();
    let id = a["agent_id"].as_str().unwrap();
    let got = wait(subs.inbox()).await;
    assert!(got[0].text().contains("status: interrupted"));
    assert_eq!(subs.status(id), Some(Status::Interrupted));
    call(&subs, "subagent.resume", json!({"agent_id": id})).await.unwrap();
    let got = wait(subs.inbox()).await;
    assert!(got[0].text().contains("status: done") && got[0].text().contains("продолжил и закончил"));
    // Законченный не возобновляется.
    let err = call(&subs, "subagent.resume", json!({"agent_id": id})).await.unwrap_err();
    assert!(err.contains("only a paused or interrupted agent can be resumed"), "{err}");
}
both!(resume_interrupted);

#[test]
fn first_message_is_source_text() {
    let m = first_message("задача");
    assert!(m.starts_with("[Subagent Context] You are running as a subagent (depth 1/1);"));
    assert!(m.ends_with("[Subagent Task]: задача\n"));
    assert!(m.contains("This subagent session is persistent and remains available for follow-up messages."));
    assert!(m.contains("Browser work: Generic subagents cannot operate a live browser"));
    assert!(!m.contains("{{") && !m.contains("<!--") && !m.contains("a member"));
}

async fn send_with_interrupt(wire: Wire) {
    let steps = vec![Step::Calls(vec![("c1", "slow", json!({"ms": 10_000}))]), Step::Text("сделал иначе")];
    let (server, subs) = children(wire, steps);
    let a = call(&subs, "subagent.spawn", json!({"message": "долго"})).await.unwrap();
    let id = a["agent_id"].as_str().unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let started = Instant::now();
    call(&subs, "subagent.send", json!({"agent_id": id, "message": "брось, сделай иначе", "interrupt": true}))
        .await
        .unwrap();
    let got = wait(subs.inbox()).await;
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(got[0].text().contains("сделал иначе"));
    // Оборванный вызов получил результат: история годна для поставщика.
    let next = server.requests()[1]["messages"].to_string();
    assert!(next.contains("interrupted before the tool finished") && next.contains("брось, сделай иначе"), "{next}");
}
both!(send_with_interrupt);
