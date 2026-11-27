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
    history.push(Message { role: Role::Assistant, content: vec![Part::ToolCall { id: "c1".into(), name: "echo".into(), arguments: json!({"x": 1}) }] });
    h.record(&history).await.unwrap();
    history.push(Message { role: Role::User, content: vec![Part::ToolResult { call_id: "c1".into(), content: vec![ToolResultPart::Text { text: "1".into() }], is_error: false }] });
    h.record(&history).await.unwrap();
    // Повторная запись того же не дублирует.
    h.record(&history).await.unwrap();
    let kinds: Vec<(i64, String, Option<String>, Option<String>)> =
        sqlx::query_as("select seq, item_kind::text, call_id, message_source from agent.context_items where agent_id = 'root' order by seq").fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        kinds,
        [
            (0, "message_text".into(), None, None),
            (1, "message_text".into(), None, Some("volatile".into())),
            (2, "tool_call".into(), Some("c1".into()), None),
            (3, "tool_output".into(), Some("c1".into()), None),
        ]
    );

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
    assert_eq!(floor, Some(4));
    let reopened = AgentHistory::open(&store, "root", "root", "fake/main", now).await.unwrap();
    let saved = reopened.load().await.unwrap().unwrap();
    assert_eq!(saved.history, compacted);
    assert_eq!(saved.last_volatile, None);
    let total: i64 = sqlx::query_scalar("select count(*) from agent.context_items where agent_id = 'root'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(total, 6, "старая история остаётся в базе");
}
