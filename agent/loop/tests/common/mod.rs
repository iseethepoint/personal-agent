//! Поддельный сервер модели для тестов цикла: только 127.0.0.1, череда
//! записанных ответов — по одному на запрос. Одна и та же череда пишется
//! на проводе anthropic (Messages API) или openai_compat (Chat Completions):
//! каждый тест цикла гоняется через оба поставщика. Живых вызовов нет.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Общий номер запроса по всем серверам теста — порядок между серверами.
static SEQ: AtomicUsize = AtomicUsize::new(0);
use std::time::Duration;

use agent_loop::{Tool, ToolError, ToolFuture};
use agent_model::anthropic::AnthropicProvider;
use agent_model::openai_compat::OpenaiCompatProvider;
use agent_model::{
    AnthropicFacts, OnText, OpenaiCompatFacts, Provider, ProviderError, Request, Response,
    ToolResultPart, ToolSpec,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    Anthropic,
    OpenaiCompat,
}

/// Один записанный ответ модели.
#[derive(Debug, Clone)]
pub enum Step {
    /// Текст без вызовов.
    Text(&'static str),
    /// Пачка вызовов: (id, имя, аргументы).
    Calls(Vec<(&'static str, &'static str, Value)>),
    /// Упор в предел ответа посреди вызова, текста нет.
    MaxTokensInCall(&'static str, &'static str),
    /// Код ошибки HTTP.
    Status(u16),
    /// Код ошибки HTTP с заголовком `retry-after` (секунды).
    RetryAfter(u16, &'static str),
    /// Только мышление: ни текста, ни вызовов.
    ThinkingOnly(&'static str),
    /// Поток оборвался после видимого текста.
    CutAfterText(&'static str),
    /// Данные потока не разбираются, текста ещё нет.
    Garbage,
}

fn sse(events: &[Value]) -> String {
    events.iter().map(|e| format!("data: {e}\n\n")).collect()
}

/// Текст отдаётся двумя кусками — как поток.
fn halves(t: &str) -> (String, String) {
    let mut mid = t.len() / 2;
    while !t.is_char_boundary(mid) {
        mid += 1;
    }
    (t[..mid].to_string(), t[mid..].to_string())
}

fn anthropic_body(step: &Step) -> (u16, &'static str, String) {
    let start = json!({"type": "message_start", "message": {"id": "msg", "type": "message", "role": "assistant",
        "content": [], "model": "m", "stop_reason": null, "usage": {"input_tokens": 10, "output_tokens": 1}}});
    let text_block = |i: usize, t: &str| {
        let (a, b) = halves(t);
        vec![
            json!({"type": "content_block_start", "index": i, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": a}}),
            json!({"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": b}}),
            json!({"type": "content_block_stop", "index": i}),
        ]
    };
    let call_block = |i: usize, id: &str, name: &str, args: &str| {
        vec![
            json!({"type": "content_block_start", "index": i, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}}),
            json!({"type": "content_block_delta", "index": i, "delta": {"type": "input_json_delta", "partial_json": args}}),
            json!({"type": "content_block_stop", "index": i}),
        ]
    };
    let stop = |reason: &str| {
        vec![
            json!({"type": "message_delta", "delta": {"stop_reason": reason, "stop_sequence": null}, "usage": {"output_tokens": 5}}),
            json!({"type": "message_stop"}),
        ]
    };
    let mut ev = vec![start];
    match step {
        Step::Text(t) => {
            ev.extend(text_block(0, t));
            ev.extend(stop("end_turn"));
        }
        Step::Calls(calls) => {
            for (i, (id, name, args)) in calls.iter().enumerate() {
                ev.extend(call_block(i, id, name, &args.to_string()));
            }
            ev.extend(stop("tool_use"));
        }
        Step::MaxTokensInCall(id, name) => {
            ev.extend(call_block(0, id, name, "{\"text\": \"aaaa"));
            ev.extend(stop("max_tokens"));
        }
        Step::Status(code) | Step::RetryAfter(code, _) => {
            let body = json!({"type": "error", "error": {"type": "api_error", "message": "boom"}});
            return (*code, "application/json", body.to_string());
        }
        Step::ThinkingOnly(t) => {
            ev.push(json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}));
            ev.push(json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": t}}));
            ev.push(json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig"}}));
            ev.push(json!({"type": "content_block_stop", "index": 0}));
            ev.extend(stop("end_turn"));
        }
        Step::CutAfterText(t) => {
            ev.extend(text_block(0, t));
            ev.pop();
        }
        Step::Garbage => return (200, "text/event-stream", "data: {не json\n\n".into()),
    }
    (200, "text/event-stream", sse(&ev))
}

fn openai_body(step: &Step) -> (u16, &'static str, String) {
    let chunk = |delta: Value, finish: Value| {
        json!({"id": "c", "object": "chat.completion.chunk", "created": 1, "model": "m",
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
    };
    let text = |t: &str| {
        let (a, b) = halves(t);
        vec![chunk(json!({"role": "assistant", "content": a}), Value::Null), chunk(json!({"content": b}), Value::Null)]
    };
    let call = |i: usize, id: &str, name: &str, args: &str| {
        chunk(
            json!({"tool_calls": [{"index": i, "id": id, "type": "function", "function": {"name": name, "arguments": args}}]}),
            Value::Null,
        )
    };
    let end = |finish: &str| {
        vec![
            chunk(json!({}), json!(finish)),
            json!({"id": "c", "object": "chat.completion.chunk", "created": 1, "model": "m", "choices": [],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}),
        ]
    };
    let mut ev = Vec::new();
    match step {
        Step::Text(t) => {
            ev.extend(text(t));
            ev.extend(end("stop"));
        }
        Step::Calls(calls) => {
            for (i, (id, name, args)) in calls.iter().enumerate() {
                ev.push(call(i, id, name, &args.to_string()));
            }
            ev.extend(end("tool_calls"));
        }
        Step::MaxTokensInCall(id, name) => {
            ev.push(call(0, id, name, "{\"text\": \"aaaa"));
            ev.extend(end("length"));
        }
        Step::Status(code) | Step::RetryAfter(code, _) => {
            let body = json!({"error": {"message": "boom", "type": "server_error"}});
            return (*code, "application/json", body.to_string());
        }
        Step::ThinkingOnly(t) => {
            ev.push(chunk(json!({"role": "assistant", "reasoning_content": t}), Value::Null));
            ev.extend(end("stop"));
        }
        Step::CutAfterText(t) => {
            ev.extend(text(t));
            return (200, "text/event-stream", sse(&ev));
        }
        Step::Garbage => return (200, "text/event-stream", "data: {не json\n\n".into()),
    }
    let mut body = sse(&ev);
    body.push_str("data: [DONE]\n\n");
    (200, "text/event-stream", body)
}

/// Сервер с чередой ответов. Запросы, которые он получил, — в `requests()`.
pub struct Server {
    pub base_url: String,
    requests: Arc<Mutex<Vec<(usize, Value)>>>,
}

impl Server {
    pub fn start(wire: Wire, steps: Vec<Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        std::thread::spawn(move || {
            for step in steps {
                let Ok((mut stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    let h = h.trim_end();
                    if h.is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("content-length") {
                            len = v.trim().parse().unwrap();
                        }
                    }
                }
                let mut raw = vec![0u8; len];
                reader.read_exact(&mut raw).unwrap();
                seen.lock().unwrap().push((SEQ.fetch_add(1, Ordering::SeqCst), serde_json::from_slice(&raw).unwrap_or(Value::Null)));
                let (status, ctype, body) = match wire {
                    Wire::Anthropic => anthropic_body(&step),
                    Wire::OpenaiCompat => openai_body(&step),
                };
                let extra = match &step {
                    Step::RetryAfter(_, secs) => format!("retry-after: {secs}\r\n"),
                    _ => String::new(),
                };
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\n{extra}connection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
                let _ = stream.flush();
            }
        });
        Server { base_url, requests }
    }

    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().iter().map(|(_, v)| v.clone()).collect()
    }

    /// Общие номера полученных запросов.
    pub fn order(&self) -> Vec<usize> {
        self.requests.lock().unwrap().iter().map(|(n, _)| *n).collect()
    }
}

/// Один из двух поставщиков — для общего набора тестов.
pub enum AnyProvider {
    Anthropic(AnthropicProvider),
    OpenaiCompat(OpenaiCompatProvider),
}

impl Provider for AnyProvider {
    async fn stream(&self, req: &Request, on_text: &mut OnText<'_>) -> Result<Response, ProviderError> {
        match self {
            AnyProvider::Anthropic(p) => p.stream(req, on_text).await,
            AnyProvider::OpenaiCompat(p) => p.stream(req, on_text).await,
        }
    }
}

/// Сервер с чередой и поставщик к нему. `forced` — умение весов
/// `anthropic.forced_tool_choice` (у Chat Completions оно есть всегда).
pub fn model(wire: Wire, steps: Vec<Step>, forced: bool) -> (Server, AnyProvider) {
    let server = Server::start(wire, steps);
    let provider = match wire {
        Wire::Anthropic => {
            let facts = AnthropicFacts { forced_tool_choice: forced, runtime_output_ceiling: Some(64000), ..Default::default() };
            AnyProvider::Anthropic(
                AnthropicProvider::new("claude-opus-5-5", facts, Some(server.base_url.clone()), "k").unwrap(),
            )
        }
        Wire::OpenaiCompat => AnyProvider::OpenaiCompat(
            OpenaiCompatProvider::new("qwen3:8b", OpenaiCompatFacts { reasoning: true, ..Default::default() }, format!("{}/v1", server.base_url), None)
                .unwrap(),
        ),
    };
    (server, provider)
}

/// Своя пустая папка для большого вывода.
pub fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("agent-loop-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn spec(name: &str, props: Value) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("тестовый инструмент {name}"),
        input_schema: json!({"type": "object", "properties": props}),
    }
}

fn text(t: impl Into<String>) -> Vec<ToolResultPart> {
    vec![ToolResultPart::Text { text: t.into() }]
}

/// Эхо: возвращает `text`.
pub struct Echo;
impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        spec("echo", json!({"text": {"type": "string"}}))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            match args.get("text").and_then(Value::as_str) {
                Some(t) => Ok(text(t)),
                None => Err(ToolError::InvalidArguments("`text` (string) is required".into())),
            }
        })
    }
}

/// Медленный: ждёт `ms` миллисекунд.
pub struct Slow;
impl Tool for Slow {
    fn spec(&self) -> ToolSpec {
        spec("slow", json!({"ms": {"type": "integer"}}))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let ms = args["ms"].as_u64().unwrap_or(0);
            tokio::time::sleep(Duration::from_millis(ms)).await;
            Ok(text(format!("slept {ms}")))
        })
    }
}

/// Падающий.
pub struct Fail;
impl Tool for Fail {
    fn spec(&self) -> ToolSpec {
        spec("fail", json!({}))
    }
    fn call(&self, _args: Value) -> ToolFuture<'_> {
        Box::pin(async { Err(ToolError::Failed("disk is on fire".into())) })
    }
}

/// С большим выводом: `bytes` байт, строками по 100 байт.
pub struct Big;
impl Tool for Big {
    fn spec(&self) -> ToolSpec {
        spec("big", json!({"bytes": {"type": "integer"}}))
    }
    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let n = args["bytes"].as_u64().unwrap_or(0) as usize;
            let line = format!("{}\n", "x".repeat(99));
            let mut out = line.repeat(n / 100);
            out.push_str(&"y".repeat(n % 100));
            Ok(text(out))
        })
    }
}

pub fn tools() -> agent_loop::Tools {
    let mut t = agent_loop::Tools::new();
    t.add(Echo).add(Slow).add(Fail).add(Big);
    t
}
