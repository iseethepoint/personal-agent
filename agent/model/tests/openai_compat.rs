//! П2в: поставщик `openai_compat` на поддельном сервере (127.0.0.1, записанные ответы).

mod common;

use std::time::Duration;

use agent_model::openai_compat::{request_body, OpenaiCompatProvider, TOOL_ERROR_PREFIX};
use agent_model::{
    Effort, Image, ImageFormat, Message, OpenaiCompatFacts, Part, Provider, ProviderErrorKind,
    Purpose, Registry, Request, Role, Settings, StopReason, ToolChoice, ToolResultPart, ToolSpec,
};
use common::{fixture, FakeServer};
use serde_json::json;

fn full_facts() -> OpenaiCompatFacts {
    OpenaiCompatFacts {
        vision: true,
        image_formats: vec![ImageFormat::Png, ImageFormat::Jpeg],
        reasoning: true,
        parallel_tool_calls: true,
        developer_role: true,
        runtime_output_ceiling: Some(8192),
        ..Default::default()
    }
}

fn user(text: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![Part::Text { text: text.into() }],
    }
}

fn request() -> Request {
    Request {
        system: "Ты — личный агент.".into(),
        messages: vec![user("Привет")],
        tools: vec![ToolSpec {
            name: "muse.read".into(),
            description: "Прочитать файл".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        }],
        settings: Settings {
            max_output_tokens: Some(100_000),
            effort: Some(Effort::High),
            tool_choice: ToolChoice::Auto,
        },
    }
}

fn provider(server: &FakeServer, facts: OpenaiCompatFacts, key: Option<&str>) -> OpenaiCompatProvider {
    OpenaiCompatProvider::new("qwen3:8b", facts, format!("{}/v1", server.base_url), key.map(str::to_string)).unwrap()
}

#[tokio::test]
async fn text_answer_and_request_shape() {
    let server = FakeServer::sse("openai_compat/text.sse");
    let r = provider(&server, full_facts(), None).complete(&request()).await.unwrap();
    assert_eq!(r.text(), "Hello, world");
    assert_eq!(r.stop_reason, StopReason::EndTurn);
    assert_eq!(r.usage.input_tokens, 25);
    assert_eq!(r.usage.output_tokens, 12);
    assert_eq!(r.usage.cache_read_tokens, 20);
    assert!(r.usage.reported);

    let c = server.captured();
    assert_eq!(c.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(c.header("authorization"), None, "локальному серверу ключ не шлётся");
    let b = &c.body;
    assert_eq!(b["model"], "qwen3:8b");
    assert_eq!(b["stream"], true);
    assert_eq!(b["stream_options"], json!({"include_usage": true}));
    assert_eq!(b["max_tokens"], 8192);
    assert_eq!(b["messages"][0], json!({"role": "system", "content": "Ты — личный агент."}));
    assert_eq!(b["messages"][1], json!({"role": "user", "content": "Привет"}));
    assert_eq!(b["tools"][0]["type"], "function");
    assert_eq!(b["tools"][0]["function"]["name"], "muse.read");
    assert_eq!(b["tools"][0]["function"]["parameters"]["required"], json!(["path"]));
}

#[tokio::test]
async fn api_key_as_bearer() {
    let server = FakeServer::sse("openai_compat/text.sse");
    provider(&server, full_facts(), Some("sk-test")).complete(&request()).await.unwrap();
    assert_eq!(server.captured().header("authorization"), Some("Bearer sk-test"));
}

#[tokio::test]
async fn two_tool_calls_in_order() {
    let server = FakeServer::sse("openai_compat/two_tools.sse");
    let r = provider(&server, full_facts(), None).complete(&request()).await.unwrap();
    server.captured();
    assert_eq!(r.stop_reason, StopReason::ToolUse);
    let calls: Vec<_> = r.tool_calls().cloned().collect();
    assert_eq!(
        calls,
        vec![
            Part::ToolCall { id: "toolu_01".into(), name: "muse.read".into(), arguments: json!({"path": "~/a.txt"}) },
            Part::ToolCall { id: "toolu_02".into(), name: "muse.read".into(), arguments: json!({"path": "~/b.txt"}) },
        ]
    );
}

#[tokio::test]
async fn thinking_by_flag() {
    let server = FakeServer::sse("openai_compat/thinking.sse");
    let r = provider(&server, full_facts(), None).complete(&request()).await.unwrap();
    let b = server.captured().body;
    assert_eq!(b["reasoning_effort"], "high");
    assert_eq!(b["parallel_tool_calls"], true);
    assert_eq!(r.content[0], Part::Thinking { text: "Сначала посчитаю.".into(), signature: None });
    assert_eq!(r.text(), "42");

    // Флаги выключены: полей нет, reasoning_content не берётся.
    let server = FakeServer::sse("openai_compat/thinking.sse");
    let r = provider(&server, OpenaiCompatFacts::default(), None).complete(&request()).await.unwrap();
    let b = server.captured().body;
    let o = b.as_object().unwrap();
    assert!(!o.contains_key("reasoning_effort"));
    assert!(!o.contains_key("parallel_tool_calls"));
    // Потолка у весов нет — предел ответа из запроса как есть.
    assert_eq!(b["max_tokens"], 100_000);
    assert_eq!(r.content, vec![Part::Text { text: "42".into() }]);
}

#[test]
fn thinking_not_sent_back() {
    let mut req = request();
    req.messages.push(Message {
        role: Role::Assistant,
        content: vec![
            Part::Thinking { text: "думаю".into(), signature: None },
            Part::Text { text: "ответ".into() },
        ],
    });
    let b = request_body("m", &req, &full_facts()).unwrap();
    assert_eq!(b["messages"][2], json!({"role": "assistant", "content": "ответ"}));
}

async fn http_error(status: u16, file: &str, headers: &[(&str, &str)]) -> agent_model::ProviderError {
    let server = FakeServer::start(status, "application/json", headers, fixture(file));
    let e = provider(&server, full_facts(), None).complete(&request()).await.unwrap_err();
    server.captured();
    e
}

#[tokio::test]
async fn http_errors_retryable_or_not() {
    let e = http_error(429, "openai_compat/error_429.json", &[("retry-after", "3")]).await;
    assert_eq!(e.kind, ProviderErrorKind::Http { status: 429 });
    assert!(e.retryable);
    assert_eq!(e.retry_after_ms, Some(3000));
    assert!(e.message.contains("Rate limit"));

    let e = http_error(500, "openai_compat/error_500.json", &[]).await;
    assert!(e.retryable);

    let e = http_error(400, "openai_compat/error_400.json", &[]).await;
    assert!(!e.retryable);
    assert!(e.message.contains("invalid_request_error"));
}

#[tokio::test]
async fn cut_stream_is_retryable() {
    let server = FakeServer::sse("openai_compat/cut.sse");
    let e = provider(&server, full_facts(), None).complete(&request()).await.unwrap_err();
    server.captured();
    assert_eq!(e.kind, ProviderErrorKind::Stream);
    assert!(e.retryable);
    assert!(e.message.contains("[DONE]"));
}

#[tokio::test]
async fn idle_stream_times_out() {
    let server = FakeServer::start_paused(
        200,
        "text/event-stream",
        &[],
        fixture("openai_compat/text.sse"),
        Some(Duration::from_millis(1500)),
    );
    let p = provider(&server, full_facts(), None).with_timeouts(Duration::from_secs(5), Duration::from_millis(200));
    let e = p.complete(&request()).await.unwrap_err();
    server.captured();
    assert_eq!(e.kind, ProviderErrorKind::Stream);
    assert!(e.retryable);
}

#[tokio::test]
async fn no_usage_is_marked() {
    let server = FakeServer::sse("openai_compat/no_usage.sse");
    let r = provider(&server, full_facts(), None).complete(&request()).await.unwrap();
    server.captured();
    assert_eq!(r.text(), "Привет");
    assert!(!r.usage.reported);
    assert_eq!(r.usage.input_tokens + r.usage.output_tokens, 0);
}

#[tokio::test]
async fn server_without_tools_is_config_error() {
    let server = FakeServer::start(400, "application/json", &[], fixture("openai_compat/no_tools_400.json"));
    let e = provider(&server, full_facts(), None).check_tools().await.unwrap_err();
    server.captured();
    assert_eq!(e.kind, ProviderErrorKind::Config);
    assert!(!e.retryable);
    assert!(e.message.contains("не поддерживает инструменты"));
}

#[tokio::test]
async fn embeddings_in_input_order() {
    let server = FakeServer::start(200, "application/json", &[], fixture("openai_compat/embeddings.json"));
    let p = OpenaiCompatProvider::new("all-minilm", OpenaiCompatFacts::default(), format!("{}/v1", server.base_url), None).unwrap();
    let v = p.embed(&["первое".into(), "второе".into()]).await.unwrap();
    let c = server.captured();
    assert_eq!(c.request_line, "POST /v1/embeddings HTTP/1.1");
    assert_eq!(c.body, json!({"model": "all-minilm", "input": ["первое", "второе"]}));
    assert_eq!(v, vec![vec![0.1, 0.2, 0.25], vec![0.3, 0.4, 0.5]]);
}

#[test]
fn tool_results_images_developer() {
    let png = Image { media_type: "image/png".into(), data_base64: "iVBO".into() };
    let mut req = request();
    req.messages = vec![
        Message { role: Role::User, content: vec![Part::Text { text: "смотри".into() }, Part::Image(png.clone())] },
        Message {
            role: Role::Assistant,
            content: vec![Part::ToolCall { id: "c1".into(), name: "muse.read".into(), arguments: json!({"path": "~/a"}) }],
        },
        Message {
            role: Role::User,
            content: vec![Part::ToolResult {
                call_id: "c1".into(),
                content: vec![ToolResultPart::Text { text: "нет файла".into() }],
                is_error: true,
            }],
        },
        Message { role: Role::Developer, content: vec![Part::Text { text: "[время 10:00]".into() }] },
    ];
    req.settings.tool_choice = ToolChoice::Tool { name: "muse.read".into() };
    let b = request_body("m", &req, &full_facts()).unwrap();
    let m = &b["messages"];
    assert_eq!(m[1]["content"][1], json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBO"}}));
    assert_eq!(m[2]["tool_calls"][0]["function"], json!({"name": "muse.read", "arguments": "{\"path\":\"~/a\"}"}));
    assert_eq!(m[3], json!({"role": "tool", "tool_call_id": "c1", "content": format!("{TOOL_ERROR_PREFIX}нет файла")}));
    assert_eq!(m[4], json!({"role": "system", "content": "[время 10:00]"}));
    assert_eq!(b["tool_choice"], json!({"type": "function", "function": {"name": "muse.read"}}));

    // Без developer_role — обёртка в сообщении пользователя.
    let facts = OpenaiCompatFacts { developer_role: false, ..full_facts() };
    let b = request_body("m", &req, &facts).unwrap();
    assert_eq!(b["messages"][4], json!({"role": "user", "content": "<system-reminder>\n[время 10:00]\n</system-reminder>"}));

    // Без зрения картинка — ошибка, а не тихий пропуск.
    let facts = OpenaiCompatFacts { vision: false, ..full_facts() };
    assert_eq!(request_body("m", &req, &facts).unwrap_err().kind, ProviderErrorKind::Config);
    // Формат не из списка — ошибка.
    let facts = OpenaiCompatFacts { image_formats: vec![ImageFormat::Jpeg], ..full_facts() };
    assert_eq!(request_body("m", &req, &facts).unwrap_err().kind, ProviderErrorKind::Config);
}

#[test]
fn unsupported_flag_and_registry() {
    let facts = OpenaiCompatFacts { tool_output_error_status: true, ..Default::default() };
    assert_eq!(
        OpenaiCompatProvider::new("m", facts, "http://127.0.0.1:1/v1", None).err().unwrap().kind,
        ProviderErrorKind::Config
    );
    let reg = Registry::load(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/models.toml")).unwrap();
    let local = reg.resolve(Purpose::Main, Some("local/qwen3-8b")).unwrap().unwrap();
    // Локальной модели ключ не нужен.
    assert!(OpenaiCompatProvider::from_resolved(&local).is_ok());
    let main = reg.resolve(Purpose::Main, None).unwrap().unwrap();
    assert!(OpenaiCompatProvider::from_resolved(&main).is_err());
}
