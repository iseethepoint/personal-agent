//! П2б: поставщик `anthropic` на поддельном сервере (127.0.0.1, записанные ответы).

mod common;

use agent_model::anthropic::{request_body, AnthropicProvider, ANTHROPIC_VERSION, MAX_REQUEST_BYTES};
use agent_model::{
    AnthropicFacts, Effort, Message, OutputEffort, Part, Provider, ProviderErrorKind, Purpose,
    Registry, Request, Role, Settings, StopReason, ToolChoice, ToolResultPart, ToolSpec,
};
use common::{fixture, FakeServer};
use serde_json::json;

fn full_facts() -> AnthropicFacts {
    AnthropicFacts {
        adaptive_thinking: true,
        mid_conversation_system_messages: true,
        output_effort: OutputEffort::Supported,
        runtime_output_ceiling: Some(64000),
        prompt_cache: true,
        ..Default::default()
    }
}

fn bare_facts() -> AnthropicFacts {
    AnthropicFacts {
        runtime_output_ceiling: Some(4096),
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

fn provider(server: &FakeServer, facts: AnthropicFacts) -> AnthropicProvider {
    AnthropicProvider::new("claude-opus-5-5", facts, Some(server.base_url.clone()), "test-key").unwrap()
}

#[test]
fn text_answer_and_request_shape() {
    let server = FakeServer::sse("anthropic/text.sse");
    let r = provider(&server, full_facts()).complete(&request()).unwrap();
    assert_eq!(r.text(), "Hello, world");
    assert_eq!(r.stop_reason, StopReason::EndTurn);
    assert_eq!(r.usage.input_tokens, 25);
    assert_eq!(r.usage.output_tokens, 12);
    assert_eq!(r.usage.cache_read_tokens, 20);
    assert_eq!(r.usage.cache_write_tokens, 5);
    assert!(r.usage.reported);

    let c = server.captured();
    assert_eq!(c.request_line, "POST /v1/messages HTTP/1.1");
    assert_eq!(c.header("anthropic-version"), Some(ANTHROPIC_VERSION));
    assert_eq!(c.header("x-api-key"), Some("test-key"));
    assert_eq!(c.header("anthropic-beta"), None);
    let b = &c.body;
    assert_eq!(b["model"], "claude-opus-5-5");
    assert_eq!(b["stream"], true);
    // Предел ответа не больше потолка весов.
    assert_eq!(b["max_tokens"], 64000);
    assert_eq!(b["messages"][0]["role"], "user");
    assert_eq!(b["messages"][0]["content"][0]["text"], "Привет");
}

#[test]
fn two_tool_calls_in_order() {
    let server = FakeServer::sse("anthropic/two_tools.sse");
    let r = provider(&server, full_facts()).complete(&request()).unwrap();
    assert_eq!(r.stop_reason, StopReason::ToolUse);
    let calls: Vec<_> = r.tool_calls().collect();
    assert_eq!(calls.len(), 2);
    match (calls[0], calls[1]) {
        (
            Part::ToolCall { id: a, name: na, arguments: aa },
            Part::ToolCall { id: b, arguments: ab, .. },
        ) => {
            assert_eq!((a.as_str(), na.as_str()), ("toolu_01", "muse.read"));
            assert_eq!(aa, &json!({"path": "~/a.txt"}));
            assert_eq!(b, "toolu_02");
            assert_eq!(ab, &json!({"path": "~/b.txt"}));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(r.content[0], Part::Text { .. }));
    server.captured();
}

#[test]
fn thinking_round_trip() {
    let server = FakeServer::sse("anthropic/thinking.sse");
    let r = provider(&server, full_facts()).complete(&request()).unwrap();
    server.captured();
    assert_eq!(
        r.content[0],
        Part::Thinking {
            text: "Сначала посчитаю.".into(),
            signature: Some("EqQBCgIYAhIM".into())
        }
    );
    assert_eq!(r.content[1], Part::RedactedThinking { data: "EmwKAhgBEgy3va".into() });
    assert_eq!(r.text(), "42");

    // Обратно в историю — как есть.
    let mut req = request();
    req.messages.push(Message {
        role: Role::Assistant,
        content: r.content.clone(),
    });
    req.messages.push(user("Дальше"));
    let body = request_body("claude-opus-5-5", &req, &full_facts()).unwrap();
    let a = &body["messages"][1]["content"];
    assert_eq!(a[0], json!({"type": "thinking", "thinking": "Сначала посчитаю.", "signature": "EqQBCgIYAhIM"}));
    assert_eq!(a[1], json!({"type": "redacted_thinking", "data": "EmwKAhgBEgy3va"}));
    assert_eq!(a[2], json!({"type": "text", "text": "42"}));
}

fn http_error(status: u16, file: &str, headers: &[(&str, &str)]) -> agent_model::ProviderError {
    let server = FakeServer::start(status, "application/json", headers, fixture(file));
    let e = provider(&server, full_facts()).complete(&request()).unwrap_err();
    server.captured();
    e
}

#[test]
fn http_errors_retryable_or_not() {
    let e = http_error(429, "anthropic/error_429.json", &[("retry-after", "2")]);
    assert_eq!(e.kind, ProviderErrorKind::Http { status: 429 });
    assert!(e.retryable);
    assert_eq!(e.retry_after_ms, Some(2000));
    assert!(e.message.contains("rate_limit_error"));

    let e = http_error(500, "anthropic/error_500.json", &[]);
    assert!(e.retryable);
    assert!(e.message.contains("api_error"));

    let e = http_error(400, "anthropic/error_400.json", &[]);
    assert_eq!(e.kind, ProviderErrorKind::Http { status: 400 });
    assert!(!e.retryable);
    assert!(e.message.contains("invalid_request_error"));
}

#[test]
fn cut_stream_is_retryable() {
    let server = FakeServer::sse("anthropic/cut.sse");
    let e = provider(&server, full_facts()).complete(&request()).unwrap_err();
    server.captured();
    assert_eq!(e.kind, ProviderErrorKind::Stream);
    assert!(e.retryable);

    let server = FakeServer::sse("anthropic/error_event.sse");
    let e = provider(&server, full_facts()).complete(&request()).unwrap_err();
    server.captured();
    assert_eq!(e.kind, ProviderErrorKind::Stream);
    assert!(e.retryable);
    assert!(e.message.contains("overloaded_error"));
}

#[test]
fn flags_on_fields_present() {
    let mut facts = full_facts();
    facts.beta = Some("context-1m-2025-08-07".into());
    let server = FakeServer::sse("anthropic/text.sse");
    provider(&server, facts).complete(&request()).unwrap();
    let c = server.captured();
    assert_eq!(c.header("anthropic-beta"), Some("context-1m-2025-08-07"));
    let b = &c.body;
    assert_eq!(b["thinking"], json!({"type": "adaptive", "display": "summarized"}));
    assert_eq!(b["output_config"], json!({"effort": "high"}));
    assert_eq!(b["system"][0]["cache_control"], json!({"type": "ephemeral"}));
    assert_eq!(b["tools"][0]["cache_control"], json!({"type": "ephemeral"}));
}

#[test]
fn flags_off_fields_absent() {
    let server = FakeServer::sse("anthropic/text.sse");
    provider(&server, bare_facts()).complete(&request()).unwrap();
    let b = server.captured().body;
    let o = b.as_object().unwrap();
    assert!(!o.contains_key("thinking"));
    assert!(!o.contains_key("output_config"));
    assert!(b["system"][0].get("cache_control").is_none());
    assert!(b["tools"][0].get("cache_control").is_none());
    assert_eq!(b["max_tokens"], 4096);
}

#[test]
fn effort_xhigh_as_max() {
    let mut facts = full_facts();
    facts.output_effort = OutputEffort::XhighAsMax;
    let mut req = request();
    req.settings.effort = Some(Effort::Xhigh);
    let b = request_body("m", &req, &facts).unwrap();
    assert_eq!(b["output_config"]["effort"], "max");
}

#[test]
fn developer_messages_by_flag() {
    let mut req = request();
    req.messages.push(Message {
        role: Role::Developer,
        content: vec![Part::Text { text: "[время 10:00]".into() }],
    });
    let on = request_body("m", &req, &full_facts()).unwrap();
    assert_eq!(on["messages"][1], json!({"role": "system", "content": "[время 10:00]"}));

    let off = request_body("m", &req, &bare_facts()).unwrap();
    let msgs = off["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1, "заметка склеена с сообщением пользователя");
    assert_eq!(
        msgs[0]["content"][1]["text"],
        "<system-reminder>\n[время 10:00]\n</system-reminder>"
    );
}

#[test]
fn tool_results_and_choice() {
    let mut req = request();
    req.messages.push(Message {
        role: Role::Assistant,
        content: vec![Part::ToolCall {
            id: "toolu_01".into(),
            name: "muse.read".into(),
            arguments: json!({"path": "~/a.txt"}),
        }],
    });
    req.messages.push(Message {
        role: Role::User,
        content: vec![Part::ToolResult {
            call_id: "toolu_01".into(),
            content: vec![ToolResultPart::Text { text: "нет файла".into() }],
            is_error: true,
        }],
    });
    req.settings.tool_choice = ToolChoice::Tool { name: "muse.read".into() };
    let b = request_body("m", &req, &bare_facts()).unwrap();
    assert_eq!(b["messages"][1]["content"][0]["type"], "tool_use");
    let r = &b["messages"][2]["content"][0];
    assert_eq!(r["tool_use_id"], "toolu_01");
    assert_eq!(r["is_error"], true);
    assert_eq!(b["tool_choice"], json!({"type": "tool", "name": "muse.read"}));
}

#[test]
fn unsupported_flags_and_limits_are_errors() {
    let facts = AnthropicFacts {
        compaction: true,
        ..full_facts()
    };
    let e = AnthropicProvider::new("m", facts, None, "k").err().unwrap();
    assert_eq!(e.kind, ProviderErrorKind::Config);
    assert!(!e.retryable);

    let mut req = request();
    req.settings.max_output_tokens = None;
    let no_ceiling = AnthropicFacts::default();
    assert!(request_body("m", &req, &no_ceiling).is_err());

    let mut req = request();
    req.messages = vec![user(&"я".repeat(MAX_REQUEST_BYTES / 2 + 1))];
    assert_eq!(
        request_body("m", &req, &full_facts()).unwrap_err().kind,
        ProviderErrorKind::Config
    );
}

#[test]
fn from_registry_needs_key_env() {
    let reg = Registry::load(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/models.toml"),
    )
    .unwrap();
    let main = reg.resolve(Purpose::Main, None).unwrap().unwrap();
    // Ключа в тестах нет — ошибка настройки, без сети.
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        let e = AnthropicProvider::from_resolved(&main).err().unwrap();
        assert_eq!(e.kind, ProviderErrorKind::Config);
    }
    let local = reg.resolve(Purpose::Main, Some("local/qwen3-8b")).unwrap().unwrap();
    assert!(AnthropicProvider::from_resolved(&local).is_err());
}
