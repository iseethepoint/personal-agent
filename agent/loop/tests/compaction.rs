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
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1_000_000, spill_dir: temp_dir("spill"), today: None, flush_covered: false, segmented: false, window: None, flush_system: None };
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
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: false, segmented: false, window: None, flush_system: None };
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
    // Заметка muse: инструментов нет — перед просьбой.
    let no_tools = sum_text.find("The available tool set has been modified.").expect("no_tools");
    assert!(sum_text.contains("No tools available.") && no_tools < sum_text.find("Create a context checkpoint summary").unwrap());
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
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: true, segmented: false, window: None, flush_system: None };
    assert!(c.compact_if_needed(&mut conv).await.unwrap().is_some());
    assert!(main.requests().is_empty(), "{wire:?}: сброса нет");
    assert_eq!(small.requests().len(), 1);

    let (main, pm) = model(wire, vec![Step::Text("записал")], false);
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: Some(("memory/2027-01-15.md".into(), "- Пьёт чай.\n".into())), flush_covered: false, segmented: false, window: None, flush_system: None };
    c.flush(&conv).await.unwrap();
    let body = main.requests()[0]["messages"].to_string();
    let note = body.find("Today's memory log at ~/memory/2027-01-15.md already ends with the entries below.").expect("заметка о журнале");
    assert!(body.contains("- Пьёт чай.") && body.find("Pre-compaction memory flush.").unwrap() > note, "{body}");
}
both!(covered_flush_is_skipped);

/// П3ж-5: пределы сжатия по частям — от окна модели, без окна — числа ядра.
#[test]
fn segmented_limits_follow_the_core() {
    use agent_loop::segmented::Limits;
    let l = |w| Limits::for_window(w);
    assert_eq!(l(None), Limits { block: 27_200, state_target: 10_880, segment_target: 2_720, state_limit: 21_760, segment_limit: 5_440 });
    assert_eq!(l(Some(1_000_000)), Limits { block: 100_000, state_target: 40_000, segment_target: 3_000, state_limit: 80_000, segment_limit: 6_000 });
    assert_eq!(agent_loop::segmented::tokens("1234567"), 2);
}

/// Старые части сверх бюджета удаляются, новейшая остаётся, на месте
/// удалённых — пометка; повторное вырезание пометку не множит.
#[test]
fn oldest_segments_give_way_to_the_marker() {
    use agent_loop::segmented::{Summary, OLDER_REMOVED};
    let mut s = Summary { segments: vec!["a".repeat(700), "b".repeat(700), "c".repeat(700)], state: Some("st".into()) };
    assert_eq!(s.trim(1_000_000), 0);
    assert_eq!(s.trim(300), 2);
    assert_eq!(s.segments, [OLDER_REMOVED.to_string(), "c".repeat(700)]);
    assert_eq!(s.trim(1), 0, "новейшая остаётся");
    let back = Summary::parse(&s.render());
    assert_eq!(back, s);
    assert_eq!(Summary::parse("просто текст").segments, ["просто текст"], "не разбирается — одна часть");
}

fn summarized(prior: &str) -> Conversation {
    let t = tools();
    let mut conv = conversation(&t);
    conv.history.insert(0, Message { role: Role::User, content: text(&format!("{SUMMARY_HEADER}{prior}")) });
    conv
}

/// Сжатие по частям: прошлые части остаются как есть, модель пишет новую
/// часть и состояние; ответ не в двух разделах — замечание с целевыми
/// длинами, второй ответ ложится в сводку.
async fn segmented_keeps_old_segments(wire: Wire) {
    let t = tools();
    let (_main, pm) = model(wire, vec![], false);
    let (small, ps) = model(
        wire,
        vec![Step::Text("посчитали файлы"), Step::Text("## New segment\n- Посчитали файлы: 1.\n\n## Current state\n- Nothing open.")],
        false,
    );
    let prior = "<segment>\nЧАСТЬ-А\n</segment>\n\n<segment>\nЧАСТЬ-Б\n</segment>\n\n<current_state>\n- Старое состояние.\n</current_state>";
    let mut conv = summarized(prior);
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: true, segmented: true, window: Some(272_000), flush_system: None };
    let done = c.compact_if_needed(&mut conv).await.unwrap().unwrap();
    let reqs = small.requests();
    assert_eq!(reqs.len(), 2, "{wire:?}: ответ и ответ после замечания");
    let first = reqs[0]["messages"].to_string();
    assert!(first.contains("ЧАСТЬ-А") && first.contains("Rewrite the current state fresh and complete"), "{wire:?}");
    assert!(first.contains("No tools available."), "{wire:?}");
    assert!(first.contains("Keep this segment under about 2720 tokens") && first.contains("under about 10880 tokens"), "{wire:?}");
    assert!(!first.contains("The previous current state is longer than this limit"), "{wire:?}");
    let second = reqs[1]["messages"].to_string();
    assert!(second.contains("Your previous reply was too long or not in the two-section form") && second.contains("the new segment under about 2720 tokens"), "{wire:?}");
    let want = "<segment>\nЧАСТЬ-А\n</segment>\n\n<segment>\nЧАСТЬ-Б\n</segment>\n\n<segment>\n- Посчитали файлы: 1.\n</segment>\n\n<current_state>\n- Nothing open.\n</current_state>";
    assert_eq!(done.summary, want);
    assert_eq!(conv.history[0].text(), format!("{SUMMARY_HEADER}{want}"));
}
both!(segmented_keeps_old_segments);

/// Маленькое окно: прошлое состояние длиннее цели — указание
/// `segmented_state_over_cap`; ответ длиннее жёстких пределов и после
/// замечания — обрезка; старые части сверх бюджета — пометка.
async fn segmented_reply_is_cut_to_fit(wire: Wire) {
    use agent_loop::segmented::{tokens, Limits, OLDER_REMOVED};
    let t = tools();
    let (_main, pm) = model(wire, vec![], false);
    let long_seg = "сегмент ".repeat(40);
    let long_state = "состояние ".repeat(40);
    let reply: &'static str = Box::leak(format!("## New segment\n{long_seg}\n## Current state\n{long_state}").into_boxed_str());
    let (small, ps) = model(wire, vec![Step::Text(reply), Step::Text(reply)], false);
    let prior = format!("<segment>\n{}\n</segment>\n\n<current_state>\n{}\n</current_state>", "старое ".repeat(40), "s".repeat(200));
    let mut conv = summarized(&prior);
    let window = 500;
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: true, segmented: true, window: Some(window), flush_system: None };
    let done = c.compact_if_needed(&mut conv).await.unwrap().unwrap();
    assert!(small.requests()[0]["messages"].to_string().contains("The previous current state is longer than this limit"), "{wire:?}");
    let s = agent_loop::segmented::Summary::parse(&done.summary);
    let l = Limits::for_window(Some(window));
    assert_eq!(s.segments.len(), 2, "{wire:?}: {s:?}");
    assert_eq!(s.segments[0], OLDER_REMOVED);
    assert!(tokens(&s.segments[1]) <= l.segment_limit && tokens(s.state.as_deref().unwrap()) <= l.state_limit, "{wire:?}: {s:?}");
}
both!(segmented_reply_is_cut_to_fit);

/// Обычное сжатие упёрлось в предел ответа — одно замечание
/// `output_cap_feedback.md`, сводка — из второго ответа.
async fn output_cap_feedback_retries_summary(wire: Wire) {
    let t = tools();
    let (_main, pm) = model(wire, vec![], false);
    let (small, ps) = model(wire, vec![Step::MaxTokensInCall("x1", "echo"), Step::Text("СВОДКА короче")], false);
    let mut conv = conversation(&t);
    let c = Compactor { flush: &pm, compaction: &ps, tools: &t, trigger: 1, spill_dir: temp_dir("spill"), today: None, flush_covered: true, segmented: false, window: None, flush_system: None };
    let done = c.compact_if_needed(&mut conv).await.unwrap().unwrap();
    assert_eq!(done.summary, "СВОДКА короче", "{wire:?}");
    let second = small.requests()[1]["messages"].to_string();
    assert!(second.contains("Your previous summary exceeded the output limit."), "{wire:?}");
}
both!(output_cap_feedback_retries_summary);
