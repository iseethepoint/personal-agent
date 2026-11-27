//! Тексты хода (П5-2, часть 5а; `prompts/SOURCES.md`): за сообщением
//! владельца — `turn_directives`, затем составной текст, каждый отдельной
//! заметкой. Форма у поставщиков — как у встроенного системного элемента
//! muse подвида 1. Тесты гоняются на обоих поддельных поставщиках.

mod common;

use agent_loop::{Compactor, Conversation, Prefix, Turn, SUMMARY_HEADER};
use agent_model::anthropic::AnthropicProvider;
use agent_model::openai_compat::OpenaiCompatProvider;
use agent_model::{AnthropicFacts, AnyProvider, Message, OpenaiCompatFacts, Part, Role};
use common::{model, temp_dir, tools, Server, Step, Wire};
use serde_json::{json, Value};

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

const DIRECTIVES: &str = "ТЕКСТ-1: указания хода";
const COMPOSITE: &str = "ТЕКСТ-2: составной текст";

fn text(t: &str) -> Vec<Part> {
    vec![Part::Text { text: t.into() }]
}

fn texts() -> Vec<String> {
    vec![DIRECTIVES.into(), COMPOSITE.into()]
}

/// Поставщик с умением встроенных системных сообщений (`anthropic.
/// mid_conversation_system_messages` / `developer_role`) или без него.
fn provider(wire: Wire, steps: Vec<Step>, inline: bool) -> (Server, AnyProvider) {
    let server = Server::start(wire, steps);
    let p = match wire {
        Wire::Anthropic => {
            let facts = AnthropicFacts { mid_conversation_system_messages: inline, runtime_output_ceiling: Some(64000), ..Default::default() };
            AnyProvider::Anthropic(AnthropicProvider::new("claude-opus-5-5", facts, Some(server.base_url.clone()), "k").unwrap())
        }
        Wire::OpenaiCompat => {
            let facts = OpenaiCompatFacts { developer_role: inline, ..Default::default() };
            AnyProvider::OpenaiCompat(OpenaiCompatProvider::new("qwen3:8b", facts, format!("{}/v1", server.base_url), None).unwrap())
        }
    };
    (server, p)
}

fn messages(body: &Value) -> Vec<Value> {
    body["messages"].as_array().unwrap().clone()
}

/// Умение есть: каждый текст — своим встроенным сообщением, по порядку,
/// сразу за сообщением владельца. Умения нет — оба текста в одной обёртке
/// `<system-reminder>` внутри сообщения владельца, тоже по порядку.
async fn inline_on_and_off(wire: Wire) {
    let t = tools();
    for inline in [true, false] {
        let (server, p) = provider(wire, vec![Step::Text("ок")], inline);
        let mut conv = Conversation::new(Prefix { system: "s".into(), tools: vec![] });
        conv.push_user(text("привет"), "Время: 10:00");
        conv.push_turn_texts(&texts());
        Turn::from_prefix(&p, &t, &conv.prefix, temp_dir("spill")).run(&mut conv.history, &mut |_: &str| {}).await.unwrap();
        let m = messages(&server.requests()[0]);
        let i = m.iter().position(|x| x.to_string().contains("привет")).unwrap();
        match (wire, inline) {
            (Wire::Anthropic, true) => {
                assert_eq!(m[i + 1], json!({"role": "system", "content": [{"type": "text", "text": DIRECTIVES}]}));
                assert_eq!(m[i + 2], json!({"role": "system", "content": [{"type": "text", "text": COMPOSITE}]}));
            }
            (Wire::OpenaiCompat, true) => {
                assert_eq!(m[i + 1], json!({"role": "developer", "content": DIRECTIVES}));
                assert_eq!(m[i + 2], json!({"role": "developer", "content": COMPOSITE}));
            }
            (_, false) => {
                assert!(m.iter().all(|x| x["role"] != "system" || x == &m[0] && wire == Wire::OpenaiCompat), "{m:?}");
                let user = m[i].to_string();
                let (a, b) = (user.find(DIRECTIVES).expect("1"), user.find(COMPOSITE).expect("2"));
                assert!(user.find("привет").unwrap() < a && a < b, "{user}");
                assert!(user.contains("<system-reminder>"));
            }
        }
    }
}
both!(inline_on_and_off);

/// Тексты лежат в истории хода: после перезапуска (история из записи) запрос
/// несёт их те же и на том же месте, нового расчёта нет.
async fn same_after_restart(wire: Wire) {
    let t = tools();
    let (server, p) = provider(wire, vec![Step::Text("ок"), Step::Text("снова")], true);
    let mut conv = Conversation::new(Prefix { system: "s".into(), tools: vec![] });
    conv.push_user(text("привет"), "Время: 10:00");
    conv.push_turn_texts(&texts());
    Turn::from_prefix(&p, &t, &conv.prefix, temp_dir("spill")).run(&mut conv.history, &mut |_: &str| {}).await.unwrap();

    let saved = serde_json::to_string(&conv.history).unwrap();
    let history: Vec<Message> = serde_json::from_str(&saved).unwrap();
    let mut back = Conversation::restore(conv.prefix.clone(), history, conv.last_volatile().map(str::to_string));
    back.push_user(text("ещё"), "Время: 10:00");
    Turn::from_prefix(&p, &t, &back.prefix, temp_dir("spill")).run(&mut back.history, &mut |_: &str| {}).await.unwrap();

    let reqs = server.requests();
    let (m0, m1) = (messages(&reqs[0]), messages(&reqs[1]));
    assert_eq!(&m1[..m0.len()], &m0[..], "история до перезапуска — байт в байт");
    assert_eq!(reqs[1].to_string().matches(COMPOSITE).count(), 1, "новый ход без своих текстов — без них");
}
both!(same_after_restart);

/// Сжатие: старый ход уходит в сводку вместе со своими текстами; у
/// оставшегося хода они на месте.
async fn compaction_drops_old_turn_texts(wire: Wire) {
    let t = tools();
    let (_main, pm) = model(wire, vec![], false);
    let (_small, ps) = model(wire, vec![Step::Text("СВОДКА")], false);
    let mut conv = Conversation::new(Prefix { system: "s".into(), tools: t.specs() });
    conv.push_user(text("первый"), "Время: 10:00");
    conv.push_turn_texts(&["СТАРЫЙ-1".into(), "СТАРЫЙ-2".into()]);
    conv.history.push(Message { role: Role::Assistant, content: text("ответ один") });
    conv.push_user(text("второй"), "Время: 10:05");
    conv.push_turn_texts(&texts());
    conv.history.push(Message { role: Role::Assistant, content: text("ответ два") });
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: true, segmented: false, window: None, flush_system: None };
    c.compact_if_needed(&mut conv).await.unwrap().expect("сжато");
    let all = format!("{:?}", conv.history);
    assert_eq!(conv.history[0].text(), format!("{SUMMARY_HEADER}СВОДКА"));
    assert!(!all.contains("СТАРЫЙ-1") && !all.contains("СТАРЫЙ-2"), "{all}");
    let i = conv.history.iter().position(|m| m.text() == "второй").unwrap();
    assert_eq!(conv.history[i + 1], Message { role: Role::Developer, content: text(DIRECTIVES) });
    assert_eq!(conv.history[i + 2], Message { role: Role::Developer, content: text(COMPOSITE) });
}
both!(compaction_drops_old_turn_texts);

#[test]
fn empty_texts_are_skipped() {
    let mut conv = Conversation::new(Prefix { system: "s".into(), tools: vec![] });
    conv.push_user(text("a"), "Время: 10:00");
    conv.push_turn_texts(&[String::new(), COMPOSITE.into()]);
    let roles: Vec<Role> = conv.history.iter().map(|m| m.role).collect();
    assert_eq!(roles, vec![Role::Developer, Role::User, Role::Developer]);
    assert_eq!(conv.history[2].text(), COMPOSITE);
}
