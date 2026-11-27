//! Тесты П3в: сжатие и сброс памяти. Тесты с моделью гоняются дважды —
//! через поддельный сервер anthropic и через поддельный openai_compat.

mod common;

use agent_loop::compaction::{MEMORY_FLUSH_PROMPT_MD, SYSTEM_MD};
use agent_loop::{personal, summarization_prompt, Compactor, Conversation, Plan, Prefix, Turn, SUMMARY_HEADER};
use agent_model::{Message, Part, Registry, Role, ToolResultPart};
use common::{model, temp_dir, tools, Step, Wire};
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

fn text(t: &str) -> Vec<Part> {
    vec![Part::Text { text: t.into() }]
}

fn call(id: &str, t: &str) -> Part {
    Part::ToolCall { id: id.into(), name: "echo".into(), arguments: json!({"text": t}) }
}

fn result(id: &str, t: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![Part::ToolResult {
            call_id: id.into(),
            content: vec![ToolResultPart::Text { text: t.into() }],
            is_error: false,
        }],
    }
}

/// Разговор из двух ходов; второй — посреди: мышление и вызов с результатом.
fn conversation(t: &agent_loop::Tools) -> Conversation {
    let mut conv = Conversation::new(Prefix { system: "Системный текст.".into(), tools: t.specs() });
    conv.push_user(text("посчитай файлы"), "Время: 10:00");
    conv.history.push(Message { role: Role::Assistant, content: vec![Part::Text { text: "Смотрю".into() }, call("c1", "a")] });
    conv.history.push(result("c1", "a"));
    conv.history.push(Message { role: Role::Assistant, content: text("Готово: 1 файл") });
    conv.push_user(text("а теперь сожми"), "Время: 10:05");
    conv.history.push(Message {
        role: Role::Assistant,
        content: vec![Part::Thinking { text: "думаю".into(), signature: Some("sig-1".into()) }, call("c2", "b")],
    });
    conv.history.push(result("c2", "b"));
    conv
}

/// Системный текст запроса.
fn system_of(wire: Wire, body: &Value) -> String {
    match wire {
        Wire::Anthropic => match &body["system"] {
            Value::String(s) => s.clone(),
            v => v.as_array().unwrap().iter().map(|b| b["text"].as_str().unwrap_or("")).collect(),
        },
        Wire::OpenaiCompat => body["messages"][0]["content"].as_str().unwrap().to_string(),
    }
}

/// Проверка поставщика: у каждого вызова есть результат после него, у
/// каждого результата — вызов до него.
fn pairs_ok(wire: Wire, body: &Value) -> bool {
    let mut calls = Vec::new();
    let mut results = Vec::new();
    for (i, m) in body["messages"].as_array().unwrap().iter().enumerate() {
        match wire {
            Wire::Anthropic => {
                for b in m["content"].as_array().into_iter().flatten() {
                    match b["type"].as_str() {
                        Some("tool_use") => calls.push((i, b["id"].as_str().unwrap().to_string())),
                        Some("tool_result") => results.push((i, b["tool_use_id"].as_str().unwrap().to_string())),
                        _ => {}
                    }
                }
            }
            Wire::OpenaiCompat => {
                for c in m["tool_calls"].as_array().into_iter().flatten() {
                    calls.push((i, c["id"].as_str().unwrap().to_string()));
                }
                if m["role"] == "tool" {
                    results.push((i, m["tool_call_id"].as_str().unwrap().to_string()));
                }
            }
        }
    }
    calls.iter().all(|(i, id)| results.iter().any(|(j, r)| r == id && j > i))
        && results.iter().all(|(j, r)| calls.iter().any(|(i, id)| id == r && i < j))
}

async fn below_trigger(wire: Wire) {
    let t = tools();
    let (main, pm) = model(wire, vec![], false);
    let (small, ps) = model(wire, vec![], false);
    let mut conv = conversation(&t);
    let before = conv.history.clone();
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1_000_000, spill_dir: temp_dir("spill"), today: None, flush_covered: false };
    assert!(c.compact_if_needed(&mut conv).await.unwrap().is_none());
    assert_eq!(conv.history, before);

    // Весь разговор — один ход: сжимать нечего, даже выше порога.
    let mut one = Conversation::new(conv.prefix.clone());
    one.push_user(text("привет"), "Время: 10:00");
    let c = Compactor { trigger: 1, ..c };
    assert!(c.compact_if_needed(&mut one).await.unwrap().is_none());
    assert!(main.requests().is_empty() && small.requests().is_empty());
}
both!(below_trigger);

async fn flush_then_summary(wire: Wire) {
    let t = tools();
    let main_steps = vec![
        // Ход сброса памяти: запись инструментом, потом ответ.
        Step::Calls(vec![("f1", "echo", json!({"text": "запомнить"}))]),
        Step::Text("записал"),
        // Ход после сжатия.
        Step::Text("продолжаю"),
    ];
    let (main, pm) = model(wire, main_steps, false);
    let (small, ps) = model(wire, vec![Step::Text("СВОДКА: посчитали файлы")], false);
    let mut conv = conversation(&t);
    let before = conv.history.clone();
    let prefix = conv.prefix.clone();
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: false };
    let done = c.compact_if_needed(&mut conv).await.unwrap().unwrap();
    assert!(done.first_kept > 0 && done.tokens_after < done.tokens_before, "{done:?}");

    // Сначала сброс памяти (два запроса главной), потом сводка.
    let (mo, so) = (main.order(), small.order());
    assert_eq!((mo.len(), so.len()), (2, 1));
    assert!(mo[1] < so[0], "{mo:?} {so:?}");
    let flush = &main.requests()[0];
    let flush_text = flush["messages"].to_string();
    assert!(flush_text.contains("Pre-compaction memory flush."));
    assert!(flush_text.contains("посчитай файлы") && flush_text.contains("а теперь сожми"));
    assert!(flush["tools"].to_string().contains("\"echo\""));

    // Сводка — на модели обслуживания, текстами muse, только старая часть.
    let sum = &small.requests()[0];
    assert_eq!(system_of(wire, sum), personal(SYSTEM_MD));
    let sum_text = sum["messages"].to_string();
    assert!(sum_text.contains("Create a context checkpoint summary for yourself"));
    assert!(sum_text.contains("Готово: 1 файл"));
    assert!(!sum_text.contains("а теперь сожми"));
    match wire {
        Wire::Anthropic => assert_eq!(sum["tool_choice"], json!({"type": "none"})),
        Wire::OpenaiCompat => assert_eq!(sum["tool_choice"], json!("none")),
    }
    assert!(pairs_ok(wire, sum));

    // История: сводка + последний ход целиком; сброс памяти в неё не попал.
    assert_eq!(conv.history[0].text(), format!("{SUMMARY_HEADER}СВОДКА: посчитали файлы"));
    assert_eq!(&conv.history[1..], &before[5..]);
    assert!(!format!("{:?}", conv.history).contains("Pre-compaction"));
    // Начало разговора сжатие не меняет.
    assert_eq!(conv.prefix, prefix);

    // Следующий запрос проходит проверку поставщика: пары вызов-результат;
    // мышление последнего хода — с подписью, как было.
    let turn = Turn::from_prefix(&pm, &t, &conv.prefix, temp_dir("spill"));
    turn.run(&mut conv.history, &mut |_: &str| {}).await.unwrap();
    let next = &main.requests()[2];
    assert!(pairs_ok(wire, next), "{next}");
    assert!(next["messages"].to_string().contains("c2"));
    if wire == Wire::Anthropic {
        assert!(next["messages"].to_string().contains("sig-1"));
    }
    assert_eq!(system_of(wire, next), system_of(wire, flush));
    assert_eq!(next["tools"], flush["tools"]);
}
both!(flush_then_summary);

#[test]
fn personal_variant_of_source_texts() {
    let s = summarization_prompt();
    assert!(!s.contains("<!--") && !s.contains("{summary_rules}") && !s.contains("member"), "{s}");
    assert!(s.contains("- Current progress and key decisions made\n- Important context, constraints, or user preferences\n- What remains to be done"));
    assert!(s.contains("promises made to the user, and any state changes"));
    assert!(s.contains("Leave out claims about which deferred tool namespaces"));
    assert!(s.contains("in the user's own words where possible"));
    let sys = personal(SYSTEM_MD);
    assert!(sys.ends_with("The user's own words are not that: their instructions, constraints, and preferences are exactly what the summary preserves."));
    let flush = personal(MEMORY_FLUSH_PROMPT_MD);
    assert!(flush.contains("Credit the user only with what they actually said"));
    assert!(!flush.contains("<!--"));
}

const REGISTRY: &str = r#"
[defaults]
model = "anthropic/main"
maintenance = "anthropic/small"

[weights.big]
context_window = 200000
compaction_trigger = 150000

[weights.small]
context_window = 200000
compaction_trigger = 100000

[weights.qwen]
context_window = 32768
compaction_trigger = 24576

[wires]
"big-1" = { weights = "big" }
"small-1" = { weights = "small" }
"qwen3:8b" = { weights = "qwen", local_inference = true }

[[models]]
id = "anthropic/main"
provider = "anthropic"
model = "big-1"
api_key_env = "ANTHROPIC_API_KEY"

[[models]]
id = "anthropic/small"
provider = "anthropic"
model = "small-1"
api_key_env = "ANTHROPIC_API_KEY"

[[models]]
id = "local/chat"
provider = "openai_compat"
model = "qwen3:8b"
base_url = "http://127.0.0.1:11434/v1"
setup.compaction_memory_flush_use_main = true
"#;

#[test]
fn plan_from_registry() {
    let reg = Registry::parse(REGISTRY).unwrap();
    // Порог — главной модели; сжатие и сброс — на модели обслуживания.
    let p = Plan::from_registry(&reg, None).unwrap();
    assert_eq!(p.trigger, 150000);
    assert_eq!((p.flush.option.id.as_str(), p.compaction.option.id.as_str()), ("anthropic/small", "anthropic/small"));
    // compaction_memory_flush_use_main — на самой главной.
    let p = Plan::from_registry(&reg, Some("local/chat")).unwrap();
    assert_eq!(p.trigger, 24576);
    assert_eq!((p.flush.option.id.as_str(), p.compaction.option.id.as_str()), ("local/chat", "local/chat"));
}

/// П3ж-4: сброс уже покрыт периодическим (`periodic_flush_covered`) —
/// сжатие идёт без хода сброса; сегодняшний журнал — заметкой перед
/// просьбой о сбросе.
async fn covered_flush_is_skipped(wire: Wire) {
    let t = tools();
    let (main, pm) = model(wire, vec![], false);
    let (small, ps) = model(wire, vec![Step::Text("СВОДКА")], false);
    let mut conv = conversation(&t);
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: true };
    assert!(c.compact_if_needed(&mut conv).await.unwrap().is_some());
    assert!(main.requests().is_empty(), "{wire:?}: сброса нет");
    assert_eq!(small.requests().len(), 1);

    let (main, pm) = model(wire, vec![Step::Text("записал")], false);
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: Some(("memory/2027-01-15.md".into(), "- Пьёт чай.\n".into())), flush_covered: false };
    c.flush(&conv).await.unwrap();
    let body = main.requests()[0]["messages"].to_string();
    let note = body.find("Today's memory log at ~/memory/2027-01-15.md already ends with the entries below.").expect("заметка о журнале");
    assert!(body.contains("- Пьёт чай.") && body.find("Pre-compaction memory flush.").unwrap() > note, "{body}");
}
both!(covered_flush_is_skipped);
