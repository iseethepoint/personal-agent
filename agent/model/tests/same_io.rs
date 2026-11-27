//! Общий тест П2б и П2в: одинаковый вход даёт одинаковый выход у обоих
//! поставщиков. Записанные ответы обоих протоколов описывают один и тот же
//! ответ модели.

mod common;

use agent_model::anthropic::AnthropicProvider;
use agent_model::openai_compat::OpenaiCompatProvider;
use agent_model::{
    AnthropicFacts, Message, OpenaiCompatFacts, Part, Provider, Request, Response, Role, Settings,
    ToolSpec,
};
use common::FakeServer;
use serde_json::json;

fn request() -> Request {
    Request {
        system: "Ты — личный агент.".into(),
        messages: vec![Message {
            role: Role::User,
            content: vec![Part::Text { text: "Прочитай a.txt и b.txt".into() }],
        }],
        tools: vec![ToolSpec {
            name: "muse.read".into(),
            description: "Прочитать файл".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }],
        settings: Settings {
            max_output_tokens: Some(1000),
            ..Default::default()
        },
    }
}

async fn both(anthropic_fixture: &str, openai_fixture: &str) -> (Response, Response) {
    let req = request();
    let a = FakeServer::sse(anthropic_fixture);
    let ra = AnthropicProvider::new("claude-opus-5-5", AnthropicFacts::default(), Some(a.base_url.clone()), "k")
        .unwrap()
        .complete(&req)
        .await
        .unwrap();
    a.captured();
    let o = FakeServer::sse(openai_fixture);
    let ro = OpenaiCompatProvider::new("qwen3:8b", OpenaiCompatFacts::default(), format!("{}/v1", o.base_url), None)
        .unwrap()
        .complete(&req)
        .await
        .unwrap();
    o.captured();
    (ra, ro)
}

#[tokio::test]
async fn same_tool_calls_same_response() {
    let (a, o) = both("anthropic/two_tools.sse", "openai_compat/two_tools.sse").await;
    assert_eq!(a, o);
}

#[tokio::test]
async fn same_text_same_content() {
    let (a, o) = both("anthropic/text.sse", "openai_compat/text.sse").await;
    assert_eq!(a.content, o.content);
    assert_eq!(a.stop_reason, o.stop_reason);
    assert_eq!(
        (a.usage.input_tokens, a.usage.output_tokens, a.usage.cache_read_tokens),
        (o.usage.input_tokens, o.usage.output_tokens, o.usage.cache_read_tokens)
    );
}

/// Куски видимого текста приходят в `on_text` по мере потока, и вместе дают
/// текст ответа — у обоих поставщиков.
#[tokio::test]
async fn stream_hands_out_visible_text() {
    let req = request();
    let a = FakeServer::sse("anthropic/text.sse");
    let mut pa = Vec::new();
    let ra = AnthropicProvider::new("claude-opus-5-5", AnthropicFacts::default(), Some(a.base_url.clone()), "k")
        .unwrap()
        .stream(&req, &mut |t: &str| pa.push(t.to_string()))
        .await
        .unwrap();
    let o = FakeServer::sse("openai_compat/text.sse");
    let mut po = Vec::new();
    let ro = OpenaiCompatProvider::new("qwen3:8b", OpenaiCompatFacts::default(), format!("{}/v1", o.base_url), None)
        .unwrap()
        .stream(&req, &mut |t: &str| po.push(t.to_string()))
        .await
        .unwrap();
    assert!(pa.len() > 1 && po.len() > 1, "{pa:?} {po:?}");
    assert_eq!(pa.concat(), ra.text());
    assert_eq!(po.concat(), ro.text());
}

/// Вызов, оборванный пределом ответа, не разбирается и отбрасывается: ответ
/// приходит с `MaxTokens`, а не ошибкой разбора — у обоих поставщиков.
#[tokio::test]
async fn call_cut_by_output_limit_is_dropped() {
    let req = request();
    let a_body = [
        r#"{"type":"message_start","message":{"id":"m","type":"message","role":"assistant","content":[],"model":"m","stop_reason":null,"usage":{"input_tokens":10,"output_tokens":1}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":"muse.read","input":{}}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\": \"~/a"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens","stop_sequence":null},"usage":{"output_tokens":5}}"#,
        r#"{"type":"message_stop"}"#,
    ]
    .iter()
    .map(|e| format!("data: {e}\n\n"))
    .collect::<String>();
    let o_body = [
        r#"{"id":"c","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"t1","type":"function","function":{"name":"muse.read","arguments":"{\"path\": \"~/a"}}]},"finish_reason":null}]}"#,
        r#"{"id":"c","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"length"}]}"#,
        "[DONE]",
    ]
    .iter()
    .map(|e| format!("data: {e}\n\n"))
    .collect::<String>();
    let a = FakeServer::start(200, "text/event-stream", &[], a_body);
    let ra = AnthropicProvider::new("claude-opus-5-5", AnthropicFacts::default(), Some(a.base_url.clone()), "k")
        .unwrap()
        .complete(&req)
        .await
        .unwrap();
    let o = FakeServer::start(200, "text/event-stream", &[], o_body);
    let ro = OpenaiCompatProvider::new("qwen3:8b", OpenaiCompatFacts::default(), format!("{}/v1", o.base_url), None)
        .unwrap()
        .complete(&req)
        .await
        .unwrap();
    for r in [ra, ro] {
        assert_eq!(r.stop_reason, agent_model::StopReason::MaxTokens);
        assert!(r.content.is_empty(), "{:?}", r.content);
    }
}
