//! Тесты хода П3а. Каждый тест гоняется дважды: через поддельный сервер
//! anthropic и через поддельный openai_compat (`both!`).

mod common;

use std::time::{Duration, Instant};

use agent_loop::{Turn, TurnError, OUTPUT_TOKEN_CAP_NOTICE, SPILL_THRESHOLD_BYTES, SUBMIT_RESULT, THINKING_ONLY_NOTICE};
use agent_model::{Message, Part, Role, ToolResultPart};
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

fn user(t: &str) -> Vec<Message> {
    vec![Message { role: Role::User, content: vec![Part::Text { text: t.into() }] }]
}

/// Результаты вызовов из сообщения: (call_id, текст, is_error).
fn results(m: &Message) -> Vec<(String, String, bool)> {
    assert_eq!(m.role, Role::User);
    m.content
        .iter()
        .map(|p| match p {
            Part::ToolResult { call_id, content, is_error } => {
                let text = content
                    .iter()
                    .map(|c| match c {
                        ToolResultPart::Text { text } => text.clone(),
                        ToolResultPart::Image(_) => String::new(),
                    })
                    .collect();
                (call_id.clone(), text, *is_error)
            }
            other => panic!("не результат: {other:?}"),
        })
        .collect()
}

async fn no_calls(wire: Wire) {
    let (server, p) = model(wire, vec![Step::Text("Привет, владелец")], false);
    let t = tools();
    let turn = Turn::new(&p, &t, "Ты — агент.", temp_dir("no_calls"));
    let mut h = user("привет");
    let mut pieces = Vec::new();
    let resp = turn.run(&mut h, &mut |s: &str| pieces.push(s.to_string())).await.unwrap();
    assert_eq!(resp.text(), "Привет, владелец");
    assert_eq!(pieces.concat(), "Привет, владелец");
    assert_eq!(h.len(), 2);
    assert_eq!(h[1].role, Role::Assistant);
    assert_eq!(server.requests().len(), 1);
    // Описания инструментов ушли модели из реестра.
    let body = server.requests()[0].to_string();
    for name in ["echo", "slow", "fail", "big"] {
        assert!(body.contains(&format!("\"{name}\"")), "{name}");
    }
}
both!(no_calls);

async fn parallel_batch(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![("c1", "slow", json!({"ms": 400})), ("c2", "slow", json!({"ms": 300}))]),
        Step::Text("готово"),
    ];
    let (server, p) = model(wire, steps, false);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("parallel"));
    let mut h = user("подожди");
    let started = Instant::now();
    let resp = turn.run(&mut h, &mut |_: &str| {}).await.unwrap();
    let took = started.elapsed();
    assert!(took < Duration::from_millis(650), "пачка не параллельна: {took:?}");
    assert_eq!(resp.text(), "готово");
    assert_eq!(h.len(), 4);
    // Результаты — одним сообщением, в порядке вызовов, а не окончания.
    assert_eq!(
        results(&h[2]),
        vec![("c1".into(), "slept 400".into(), false), ("c2".into(), "slept 300".into(), false)]
    );
    assert_eq!(server.requests().len(), 2);
    assert!(server.requests()[1].to_string().contains("slept 400"));
}
both!(parallel_batch);

async fn unknown_tool_and_bad_arguments(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![
            ("c1", "nope", json!({})),
            ("c2", "echo", json!({"wrong": 1})),
            ("c3", "fail", json!({})),
            ("c4", "echo", json!({"text": "эхо"})),
        ]),
        Step::Text("понял"),
    ];
    let (_server, p) = model(wire, steps, false);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("unknown"));
    let mut h = user("x");
    turn.run(&mut h, &mut |_: &str| {}).await.unwrap();
    let r = results(&h[2]);
    assert!(r[0].2 && r[0].1.starts_with("Unknown tool `nope`: no exact match"), "{:?}", r[0]);
    assert!(r[1].2 && r[1].1 == "Invalid arguments for echo: `text` (string) is required", "{:?}", r[1]);
    assert!(r[2].2 && r[2].1 == "disk is on fire", "{:?}", r[2]);
    assert_eq!(r[3], ("c4".into(), "эхо".into(), false));
}
both!(unknown_tool_and_bad_arguments);

async fn tool_timeout(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![("c1", "slow", json!({"ms": 10_000})), ("c2", "echo", json!({"text": "ok"}))]),
        Step::Text("ясно"),
    ];
    let (_server, p) = model(wire, steps, false);
    let t = tools();
    let mut turn = Turn::new(&p, &t, "s", temp_dir("timeout"));
    assert_eq!(turn.tool_timeout, Duration::from_secs(180));
    turn.tool_timeout = Duration::from_millis(200);
    let mut h = user("x");
    let started = Instant::now();
    turn.run(&mut h, &mut |_: &str| {}).await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    let r = results(&h[2]);
    assert!(r[0].2 && r[0].1.contains("did not finish within 200ms"), "{:?}", r[0]);
    assert_eq!(r[1], ("c2".into(), "ok".into(), false));
}
both!(tool_timeout);

async fn big_output(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![
            ("c1", "big", json!({"bytes": 200_000})),
            ("c2", "big", json!({"bytes": SPILL_THRESHOLD_BYTES})),
        ]),
        Step::Text("прочёл"),
    ];
    let (_server, p) = model(wire, steps, false);
    let t = tools();
    let dir = temp_dir("big");
    let turn = Turn::new(&p, &t, "s", &dir);
    let mut h = user("x");
    turn.run(&mut h, &mut |_: &str| {}).await.unwrap();
    let r = results(&h[2]);
    let path = dir.join("c1.txt");
    assert_eq!(std::fs::read_to_string(&path).unwrap().len(), 200_000);
    let (head, notice) = r[0].1.rsplit_once('\n').unwrap();
    assert!(!r[0].2);
    assert!(head.len() <= SPILL_THRESHOLD_BYTES && head.len() > SPILL_THRESHOLD_BYTES - 100);
    assert!(head.starts_with("xxxx"));
    let lines = head.lines().count();
    assert_eq!(
        notice,
        format!(
            "[Tool output exceeded 100KB (200000 bytes) and was saved to {p}. Use read with path=\"{p}\" offset={o} to continue.]",
            p = path.display(),
            o = lines + 1
        )
    );
    // Ровно на пороге — не в файл.
    assert_eq!(r[1].1.len(), SPILL_THRESHOLD_BYTES);
    assert!(!dir.join("c2.txt").exists());
}
both!(big_output);

async fn retry_before_visible_output(wire: Wire) {
    // Ошибка поставщика «можно повторить» → повтор.
    let (server, p) = model(wire, vec![Step::Status(500), Step::Text("ok")], false);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("retry"));
    let mut h = user("x");
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(h.len(), 2);

    // Сбой разбора потока → повтор.
    let (server, p) = model(wire, vec![Step::Garbage, Step::Text("ok")], false);
    let turn = Turn::new(&p, &t, "s", temp_dir("retry"));
    let mut h = user("x");
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    assert_eq!(server.requests().len(), 2);

    // Неверное имя инструмента → повтор, неверный вызов в историю не идёт.
    let steps = vec![Step::Calls(vec![("c1", "bad name!", json!({}))]), Step::Text("ok")];
    let (server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &t, "s", temp_dir("retry"));
    let mut h = user("x");
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(h.len(), 2);

    // Упор в предел ответа посреди вызова → вставка «Internal retry notice» и повтор.
    let steps = vec![Step::MaxTokensInCall("c1", "echo"), Step::Text("ok")];
    let (server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &t, "s", temp_dir("retry"));
    let mut h = user("x");
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    let reqs = server.requests();
    assert_eq!(reqs.len(), 2);
    assert!(!reqs[0].to_string().contains("reached the output limit"));
    assert!(reqs[1].to_string().contains("reached the output limit before it finished"));
    assert_eq!(h[1].role, Role::Developer);
    assert_eq!(h[1].text(), OUTPUT_TOKEN_CAP_NOTICE);

    // Попытки кончились → ошибка.
    let (server, p) = model(wire, vec![Step::Status(500), Step::Status(503), Step::Text("ok")], false);
    let turn = Turn::new(&p, &t, "s", temp_dir("retry"));
    let mut h = user("x");
    assert!(matches!(turn.run(&mut h, &mut |_: &str| {}).await, Err(TurnError::Model(_))));
    assert_eq!(server.requests().len(), 2);

    // Ошибка, которую повторять нельзя (400), — без повтора.
    let (server, p) = model(wire, vec![Step::Status(400), Step::Text("ok")], false);
    let turn = Turn::new(&p, &t, "s", temp_dir("retry"));
    let mut h = user("x");
    assert!(matches!(turn.run(&mut h, &mut |_: &str| {}).await, Err(TurnError::Model(_))));
    assert_eq!(server.requests().len(), 1);
}
both!(retry_before_visible_output);

async fn no_retry_after_visible_output(wire: Wire) {
    let (server, p) = model(wire, vec![Step::CutAfterText("Сейчас посмотрю"), Step::Text("ok")], false);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("visible"));
    let mut h = user("x");
    let mut seen = String::new();
    let err = turn.run(&mut h, &mut |s: &str| seen.push_str(s)).await.unwrap_err();
    assert!(matches!(err, TurnError::Model(_)), "{err:?}");
    assert_eq!(server.requests().len(), 1, "после видимого текста повтора нет");
    assert_eq!(seen, "Сейчас посмотрю");
    // Увиденное владельцем остаётся в истории.
    assert_eq!(h.len(), 2);
    assert_eq!((h[1].role, h[1].text()), (Role::Assistant, "Сейчас посмотрю".to_string()));
}
both!(no_retry_after_visible_output);

fn schema() -> Value {
    json!({"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"]})
}

async fn contract(wire: Wire, forced: bool) {
    let steps = vec![Step::Calls(vec![("c1", SUBMIT_RESULT, json!({"result": {"n": 5}}))])];
    let (server, p) = model(wire, steps, forced);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("contract"));
    let mut h = user("посчитай");
    let v = turn.run_contract(&mut h, &mut |_: &str| {}, &schema(), forced).await.unwrap();
    assert_eq!(v, json!({"n": 5}));
    let body = &server.requests()[0];
    let text = body.to_string();
    assert!(text.contains("\"submit_result\""));
    let choice = &body["tool_choice"];
    let instruction = "Complete the task by calling `submit_result` exactly once.";
    if forced {
        match wire {
            Wire::Anthropic => assert_eq!(choice, &json!({"type": "tool", "name": "submit_result"})),
            Wire::OpenaiCompat => {
                assert_eq!(choice, &json!({"type": "function", "function": {"name": "submit_result"}}))
            }
        }
        assert!(!text.contains(instruction));
    } else {
        assert!(choice.is_null() || choice == &json!("auto") || choice == &json!({"type": "auto"}), "{choice}");
        assert!(text.contains(instruction), "{text}");
    }
    // История закончена: у вызова есть результат.
    assert_eq!(results(h.last().unwrap()), vec![("c1".into(), String::new(), false)]);

    // Вызов не один → ошибка формы.
    let steps = vec![Step::Calls(vec![
        ("c1", SUBMIT_RESULT, json!({"result": {"n": 1}})),
        ("c2", SUBMIT_RESULT, json!({"result": {"n": 2}})),
    ])];
    let (_server, p) = model(wire, steps, forced);
    let turn = Turn::new(&p, &t, "s", temp_dir("contract"));
    let mut h = user("посчитай");
    let err = turn.run_contract(&mut h, &mut |_: &str| {}, &schema(), forced).await.unwrap_err();
    assert!(matches!(err, TurnError::OutputContract(_)), "{err:?}");

    // Текст вместо вызова → ошибка формы.
    let (_server, p) = model(wire, vec![Step::Text("{\"n\": 5}")], forced);
    let turn = Turn::new(&p, &t, "s", temp_dir("contract"));
    let mut h = user("посчитай");
    let err = turn.run_contract(&mut h, &mut |_: &str| {}, &schema(), forced).await.unwrap_err();
    assert!(matches!(err, TurnError::OutputContract(_)), "{err:?}");
}

async fn contract_forced(wire: Wire) {
    contract(wire, true).await
}
both!(contract_forced);

async fn contract_not_forced(wire: Wire) {
    contract(wire, false).await
}
both!(contract_not_forced);


async fn retry_waits_for_retry_after(wire: Wire) {
    // Сервер велел ждать 0.4 с — повтор не раньше.
    let (server, p) = model(wire, vec![Step::RetryAfter(429, "0.4"), Step::Text("ok")], false);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("retry_after"));
    let mut h = user("x");
    let started = Instant::now();
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    assert!(started.elapsed() >= Duration::from_millis(400), "{:?}", started.elapsed());
    assert_eq!(server.requests().len(), 2);

    // Без retry-after — повтор сразу.
    let (server, p) = model(wire, vec![Step::Status(429), Step::Text("ok")], false);
    let turn = Turn::new(&p, &t, "s", temp_dir("retry_after"));
    let mut h = user("x");
    let started = Instant::now();
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    assert!(started.elapsed() < Duration::from_millis(300), "{:?}", started.elapsed());
    assert_eq!(server.requests().len(), 2);
}
both!(retry_waits_for_retry_after);

async fn retry_after_thinking_only(wire: Wire) {
    let (server, p) = model(wire, vec![Step::ThinkingOnly("думаю, но молчу"), Step::Text("ok")], false);
    let t = tools();
    let turn = Turn::new(&p, &t, "s", temp_dir("thinking_only"));
    let mut h = user("x");
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "ok");
    let reqs = server.requests();
    assert_eq!(reqs.len(), 2);
    assert!(!reqs[0].to_string().contains("ended after reasoning only"));
    assert!(reqs[1].to_string().contains("ended after reasoning only"));
    // Ответ из одного мышления в историю не идёт; идёт вставка.
    assert_eq!(h.len(), 3);
    assert_eq!((h[1].role, h[1].text()), (Role::Developer, THINKING_ONLY_NOTICE.to_string()));
}
both!(retry_after_thinking_only);

/// Завершающий инструмент: успешный вызов кончает ход без нового вызова
/// модели; неудачный — ход идёт дальше (`Turn::terminal`, П4е-1).
async fn terminal_tool_ends_the_turn(wire: Wire) {
    let steps = vec![
        Step::Calls(vec![("c1", "fail", json!({}))]),
        Step::Calls(vec![("c2", "echo", json!({"text": "done"}))]),
        Step::Text("не должен понадобиться"),
    ];
    let (server, p) = model(wire, steps, false);
    let t = tools();
    let mut turn = Turn::new(&p, &t, "Ты — агент.", temp_dir("terminal"));
    turn.terminal = vec!["fail".into(), "echo".into()];
    let mut h = user("закончи");
    turn.run(&mut h, &mut |_| {}).await.unwrap();
    assert_eq!(server.requests().len(), 2, "после успешного завершающего вызова модель не зовётся");
    assert!(results(&h[2])[0].2);
    assert_eq!(results(h.last().unwrap()), [("c2".to_string(), "done".to_string(), false)]);
}
both!(terminal_tool_ends_the_turn);

/// П3е-4: ход отдаёт счёт поставщика после каждого записанного ответа
/// модели (`agent_message_token_usage`) — на обоих проводах.
struct Usages(std::sync::Mutex<Vec<(usize, u64)>>, std::sync::Mutex<usize>);

impl agent_loop::Record for Usages {
    fn record<'a>(&'a self, history: &'a [Message]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        *self.1.lock().unwrap() = history.len();
        Box::pin(async { Ok(()) })
    }
    fn usage<'a>(&'a self, usage: &'a agent_model::Usage) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        let len = *self.1.lock().unwrap();
        self.0.lock().unwrap().push((len, usage.output_tokens));
        Box::pin(async { Ok(()) })
    }
}

async fn usage_follows_each_answer(wire: Wire) {
    let (_server, provider) = model(wire, vec![Step::Calls(vec![("c1", "echo", json!({"text": "x"}))]), Step::Text("Done.")], false);
    let tools = tools();
    let rec = std::sync::Arc::new(Usages(Default::default(), Default::default()));
    let mut turn = Turn::new(&provider, &tools, "s", temp_dir("usage").join("spill"));
    turn.record = Some(rec.clone());
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "go".into() }] }];
    turn.run(&mut h, &mut |_| {}).await.unwrap();
    // После ответа с вызовом (история из 2) и после итогового (из 4).
    assert_eq!(rec.0.lock().unwrap().as_slice(), [(2, 5), (4, 5)], "{wire:?}");
}

#[tokio::test]
async fn usage_follows_each_answer_anthropic() {
    usage_follows_each_answer(Wire::Anthropic).await
}

#[tokio::test]
async fn usage_follows_each_answer_openai_compat() {
    usage_follows_each_answer(Wire::OpenaiCompat).await
}
