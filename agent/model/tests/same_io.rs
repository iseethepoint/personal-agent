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
