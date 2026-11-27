//! Тесты П3е-1: запись истории по мере хода, граница подсказки после
//! сжатия, восстановление после повторного открытия.

#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::sync::Arc;

use agent_history::{AgentHistory, Store};
use agent_loop::{Prefix, Record};
use agent_model::{Message, Part, Role, ToolResultPart};
use serde_json::json;

fn msg(role: Role, text: &str) -> Message {
    Message { role, content: vec![Part::Text { text: text.into() }] }
}

#[tokio::test]
async fn history_is_written_as_it_grows_and_compaction_moves_the_floor() {
    let url = pg::database();
    let store = Store::connect(&url).await.unwrap();
    let now = Arc::new(|| 1_800_000_000_000i64);
    let h = AgentHistory::open(&store, "root", "root", "fake/main", now.clone()).await.unwrap();
    assert!(h.load().await.unwrap().is_none(), "без закреплённого начала разговора нет");
    let prefix = Prefix { system: "SYSTEM".into(), tools: vec![] };
    h.pin_prefix(&prefix).await.unwrap();

    let mut history = vec![msg(Role::User, "hi"), msg(Role::Developer, "Current date: today")];
    h.sync(&history, Some("Current date: today")).await.unwrap();
    history.push(Message {
        role: Role::Assistant,
        content: vec![
            Part::Thinking { text: "think".into(), signature: Some("sig".into()) },
            Part::RedactedThinking { data: "opaque".into() },
            Part::Text { text: "calling".into() },
            Part::ToolCall { id: "c1".into(), name: "echo".into(), arguments: json!({"x": 1}) },
        ],
    });
    h.record(&history).await.unwrap();
    history.push(Message { role: Role::User, content: vec![Part::ToolResult { call_id: "c1".into(), content: vec![ToolResultPart::Text { text: "1".into() }], is_error: false }] });
    h.record(&history).await.unwrap();
    // Повторная запись того же не дублирует.
    h.record(&history).await.unwrap();
    // Одна часть — одна строка; части сообщения связаны message_id.
    type Row = (i64, String, String, Option<String>, Option<String>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as("select seq, item_kind::text, message_id, role::text, call_id, message_source from agent.context_items where agent_id = 'root' order by seq")
        .fetch_all(store.pool())
        .await
        .unwrap();
    let a = |s: &str| Some(s.to_string());
    assert_eq!(
        rows,
        [
            (0, "message_text".into(), "root:0".into(), a("user"), None, None),
            (1, "message_text".into(), "root:1".into(), a("developer"), None, a("volatile")),
            (2, "message".into(), "root:2".into(), a("assistant"), None, None),
            (3, "message".into(), "root:2".into(), a("assistant"), None, None),
            (4, "message_text".into(), "root:2".into(), a("assistant"), None, None),
            (5, "tool_call".into(), "root:2".into(), a("assistant"), a("c1"), None),
            (6, "tool_output".into(), "root:6".into(), a("user"), a("c1"), None),
        ]
    );
    // Мышление скрыто видом inspection.context_items (redaction каталога muse).
    let visible: i64 = sqlx::query_scalar("select count(*) from inspection.context_items where agent_id = 'root'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(visible, 5);

    // Повторное открытие: всё на месте, последняя заметка изменчивого — та же.
    let again = AgentHistory::open(&store, "root", "root", "fake/main", now.clone()).await.unwrap();
    let saved = again.load().await.unwrap().unwrap();
    assert_eq!(saved.prefix, prefix);
    assert_eq!(saved.history, history);
    assert_eq!(saved.last_volatile.as_deref(), Some("Current date: today"));

    // Сжатие: новая история после старой, граница — на её начало.
    let compacted = vec![msg(Role::User, "summary"), msg(Role::User, "hi again")];
    again.replace(&compacted, None).await.unwrap();
    let floor: Option<i64> = sqlx::query_scalar("select prompt_floor_seq from agent.agents where agent_id = 'root'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(floor, Some(7));
    let reopened = AgentHistory::open(&store, "root", "root", "fake/main", now).await.unwrap();
    let saved = reopened.load().await.unwrap().unwrap();
    assert_eq!(saved.history, compacted);
    assert_eq!(saved.last_volatile, None);
    let total: i64 = sqlx::query_scalar("select count(*) from agent.context_items where agent_id = 'root'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(total, 9, "старая история остаётся в базе");
}

/// П3е-2: состояния ящика — accepted → attached → completed/aborted;
/// непринятые после перезапуска возвращаются в очередь.
#[tokio::test]
async fn mailbox_states_follow_the_core() {
    use agent_history::AgentMailbox;
    use agent_loop::Inbox;
    let store = Store::connect(&pg::database()).await.unwrap();
    AgentHistory::open(&store, "root", "root", "fake/main", Arc::new(|| 0)).await.unwrap();
    let inbox = Inbox::persistent(Arc::new(AgentMailbox::new(&store, "root"))).await.unwrap();
    inbox.push(msg(Role::Developer, "a")).await;
    inbox.push_user(msg(Role::User, "b")).await;
    let kinds: Vec<String> = sqlx::query_scalar("select submission_kind from agent.message_mailbox order by accepted_ordinal").fetch_all(store.pool()).await.unwrap();
    assert_eq!(kinds, ["notice", "user"]);
    let state = || async {
        let rows: Vec<(String, i64, String, Option<String>)> =
            sqlx::query_as("select submission_id, accepted_ordinal, state, stream_owner_message_id from agent.message_mailbox order by accepted_ordinal").fetch_all(store.pool()).await.unwrap();
        rows
    };
    assert_eq!(state().await, [("root:mailbox:1".into(), 1, "accepted".into(), None), ("root:mailbox:2".into(), 2, "accepted".into(), None)]);
    // Перезапуск: непринятые — снова в очереди, по порядку.
    let again = Inbox::persistent(Arc::new(AgentMailbox::new(&store, "root"))).await.unwrap();
    let (ids, msgs) = again.take();
    assert_eq!(ids, ["root:mailbox:1", "root:mailbox:2"]);
    assert_eq!(msgs, [msg(Role::Developer, "a"), msg(Role::User, "b")]);
    again.attach(&ids[..1], "t1").await.unwrap();
    again.attach(&ids[1..], "t2").await.unwrap();
    again.complete("t1", true).await.unwrap();
    again.complete("t2", false).await.unwrap();
    let rows = state().await;
    assert_eq!((rows[0].2.as_str(), rows[0].3.as_deref()), ("completed", Some("t1")));
    assert_eq!((rows[1].2.as_str(), rows[1].3.as_deref()), ("aborted", Some("t2")));
    let third = Inbox::persistent(Arc::new(AgentMailbox::new(&store, "root"))).await.unwrap();
    assert!(third.drain().is_empty());
}

/// П3е-3: список дел и сторож продолжения в базе переживают перезапуск:
/// после двух напоминаний по той же работе — тишина и в новом процессе;
/// новая незакрытая работа — новое поколение сторожа.
#[tokio::test]
async fn todo_and_continuation_guard_survive_restart() {
    use agent_history::AgentTodo;
    use agent_loop::Tool;
    use agent_tools::{Continuation, Todo};
    let store = Store::connect(&pg::database()).await.unwrap();
    let open = || async { Todo::persistent(Arc::new(AgentTodo::new(&store, "root"))).await.unwrap() };
    let todo = open().await;
    todo.call(json!({"todos": [{"text": "a", "status": "in_progress"}, {"text": "b", "status": "pending"}]})).await.unwrap();
    let first = Continuation::new().next(&todo).await.unwrap();
    assert!(!first.contains("has not changed since the last reminder"));
    // Перезапуск: список тот же, сторож помнит одно напоминание.
    let todo = open().await;
    assert_eq!(todo.current().len(), 2);
    let second = Continuation::new().next(&todo).await.unwrap();
    assert!(second.contains("The unfinished work in this list has not changed since the last reminder."), "{second}");
    let todo = open().await;
    assert!(Continuation::new().next(&todo).await.is_none(), "два напоминания — тишина и после перезапуска");
    // Новая незакрытая работа — новое поколение, снова продолжение.
    todo.call(json!({"todos": [{"text": "a", "status": "completed"}, {"text": "b", "status": "in_progress"}]})).await.unwrap();
    let todo = open().await;
    assert!(Continuation::new().next(&todo).await.is_some());
    let row: (i64, i64, i16) = sqlx::query_as("select revision, continuation_generation, continuation_reminders from runtime.agent_todo_snapshots where conversation_id = 'root'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(row, (2, 2, 1));
}
