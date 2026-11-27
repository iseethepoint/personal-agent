//! Тесты П3б: сборка подсказки. Тесты с моделью гоняются дважды — через
//! поддельный сервер anthropic и через поддельный openai_compat.

mod common;

use agent_loop::{o200k_count, Conversation, Prefix, Turn};
use agent_model::{Message, Part, Role, Usage};
use common::{model, temp_dir, tools, Step, Wire};
use serde_json::Value;

macro_rules! both {
    ($name:ident) => {
        mod $name {
            use super::*;
            #[tokio::test]
            async fn anthropic() {
                super::$name(Wire::Anthropic).await
            }
            #[tokio::test]
            async fn openai_compat() {
                super::$name(Wire::OpenaiCompat).await
            }
        }
    };
}

fn text(t: &str) -> Vec<Part> {
    vec![Part::Text { text: t.into() }]
}

/// Неизменное начало запроса: системный текст и описания инструментов.
fn prefix_of(wire: Wire, body: &Value) -> String {
    match wire {
        Wire::Anthropic => format!("{}{}", body["system"], body["tools"]),
        Wire::OpenaiCompat => format!("{}{}", body["messages"][0], body["tools"]),
    }
}

async fn prefix_is_pinned(wire: Wire) {
    let home = temp_dir("home");
    std::fs::write(home.join("SOUL.md"), "Я — тестовый агент.").unwrap();
    std::fs::write(home.join("USER.md"), "Владелец — Тест.").unwrap();
    let t = tools();
    let mut conv = Conversation::new(Prefix::assemble("Системный текст.", &home, &t));
    let (server, p) = model(wire, vec![Step::Text("Привет"), Step::Text("Снова привет")], false);

    conv.push_user(text("привет"), "Время: 10:00");
    let turn = Turn::from_prefix(&p, &t, &conv.prefix, temp_dir("spill"));
    turn.run(&mut conv.history, &mut |_: &str| {}).await.unwrap();

    // Файлы домашней папки меняются посреди разговора — начало уже собрано.
    std::fs::write(home.join("SOUL.md"), "Другой характер.").unwrap();
    conv.push_user(text("ещё"), "Время: 10:05");
    let turn = Turn::from_prefix(&p, &t, &conv.prefix, temp_dir("spill"));
    turn.run(&mut conv.history, &mut |_: &str| {}).await.unwrap();

    let reqs = server.requests();
    assert_eq!(reqs.len(), 2);
    // Начало байт в байт одно и то же.
    assert_eq!(prefix_of(wire, &reqs[0]), prefix_of(wire, &reqs[1]));
    let start = prefix_of(wire, &reqs[0]);
    assert!(start.contains("Системный текст."));
    assert!(start.contains(r"<!-- INJECTED FILE: SOUL.md -->\n\nЯ — тестовый агент.\n\n<!-- END INJECTED FILE: SOUL.md -->"), "{start}");
    assert!(start.contains("<!-- INJECTED FILE: AGENTS.md -->"));
    assert!(!start.contains("Другой характер."));
    // Время — не в начале, а в хвосте.
    assert!(!start.contains("Время"));
    let last = reqs[1]["messages"].as_array().unwrap().last().unwrap().to_string();
    assert!(last.contains("Время: 10:05"), "{last}");
    // История только дописывается: сообщения первого запроса — начало второго.
    let m0 = reqs[0]["messages"].as_array().unwrap();
    let m1 = reqs[1]["messages"].as_array().unwrap();
    assert_eq!(&m1[..m0.len()], &m0[..]);
}
both!(prefix_is_pinned);

#[test]
fn volatile_note_only_when_changed() {
    let mut conv = Conversation::new(Prefix { system: "s".into(), tools: vec![] });
    conv.push_user(text("a"), "Время: 10:00");
    conv.push_user(text("b"), "Время: 10:00");
    conv.push_user(text("c"), "Время: 10:01");
    let roles: Vec<Role> = conv.history.iter().map(|m| m.role).collect();
    // Заметка — перед сообщением (метка времени входа, `time_tag_before_input`).
    assert_eq!(roles, vec![Role::Developer, Role::User, Role::User, Role::Developer, Role::User]);
    assert_eq!(conv.history[3].text(), "Время: 10:01");
    assert_eq!(conv.history[4].text(), "c");
}

async fn estimate_by_provider_usage(wire: Wire) {
    let t = tools();
    let mut conv = Conversation::new(Prefix { system: "s".into(), tools: t.specs() });
    let (_server, p) = model(wire, vec![Step::Text("Привет")], false);
    conv.push_user(text("привет"), "Время: 10:00");
    let turn = Turn::from_prefix(&p, &t, &conv.prefix, temp_dir("spill"));
    let resp = turn.run(&mut conv.history, &mut |_: &str| {}).await.unwrap();
    assert!(resp.usage.reported);
    conv.record_usage(&resp.usage);
    // Записанный ответ: 10 на входе, 5 на выходе.
    assert_eq!(conv.estimate_tokens(), 15);
    // Дописанное после ответа — по o200k.
    conv.push_user(text("ещё вопрос"), "Время: 10:05");
    assert_eq!(conv.estimate_tokens(), 15 + o200k_count("ещё вопрос") + o200k_count("Время: 10:05"));
}
both!(estimate_by_provider_usage);

#[test]
fn estimate_without_usage_is_o200k() {
    assert_eq!(o200k_count("hello world"), 2);
    let t = tools();
    let mut conv = Conversation::new(Prefix { system: "Системный текст.".into(), tools: t.specs() });
    conv.push_user(text("привет"), "Время: 10:00");
    conv.history.push(Message { role: Role::Assistant, content: text("Здравствуй") });
    conv.record_usage(&Usage::default());
    let tools_tokens: u64 =
        t.specs().iter().map(|s| o200k_count(&serde_json::to_string(s).unwrap())).sum();
    let expected = o200k_count("Системный текст.")
        + tools_tokens
        + o200k_count("привет")
        + o200k_count("Время: 10:00")
        + o200k_count("Здравствуй");
    assert_eq!(conv.estimate_tokens(), expected);
    assert!(expected > 0);
}
