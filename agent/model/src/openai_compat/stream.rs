//! Разбор потока Chat Completions — по форме muse
//! `model/kimi_chat_completions/stream.rs` (`source-files.txt:5304`):
//! куски `choices[0].delta` до `data: [DONE]`; без `[DONE]` — обрыв
//! («Chat Completions stream ended before [DONE]», `strings-all.txt:237328`).

use serde_json::Value;

use crate::http::{decode_err, stream_err};
use crate::types::{Part, ProviderError, ProviderErrorKind, Response, StopReason, Usage};

#[derive(Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
pub(crate) struct State {
    /// Брать ли `reasoning_content` (умение `openai_compat.reasoning`).
    reasoning: bool,
    thinking: String,
    text: String,
    calls: Vec<Call>,
    finish: Option<String>,
    usage: Usage,
    done: bool,
}

fn u(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

impl State {
    pub(crate) fn new(reasoning: bool) -> Self {
        State {
            reasoning,
            ..Default::default()
        }
    }

    pub(crate) fn event(&mut self, data: &str) -> Result<(), ProviderError> {
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let v: Value =
            serde_json::from_str(data).map_err(|e| decode_err(format!("SSE data: {e}")))?;
        if let Some(e) = v.get("error") {
            let text = crate::http::error_text(&v.to_string());
            let kind = e["type"].as_str().unwrap_or("");
            return Err(ProviderError {
                kind: ProviderErrorKind::Stream,
                message: text,
                retryable: !kind.contains("invalid"),
                retry_after_ms: None,
            });
        }
        if let Some(usage) = v.get("usage").filter(|x| x.is_object()) {
            self.usage = Usage {
                input_tokens: u(usage, "prompt_tokens"),
                output_tokens: u(usage, "completion_tokens"),
                cache_read_tokens: u(&usage["prompt_tokens_details"], "cached_tokens"),
                cache_write_tokens: 0,
                reported: true,
            };
        }
        let Some(choice) = v["choices"].get(0) else {
            return Ok(());
        };
        let d = &choice["delta"];
        if let Some(t) = d["content"].as_str() {
            self.text.push_str(t);
        }
        if self.reasoning {
            if let Some(t) = d["reasoning_content"].as_str() {
                self.thinking.push_str(t);
            }
        }
        if let Some(calls) = d["tool_calls"].as_array() {
            for c in calls {
                let i = c["index"].as_u64().unwrap_or(0) as usize;
                if self.calls.len() <= i {
                    self.calls.resize_with(i + 1, Call::default);
                }
                let call = &mut self.calls[i];
                if let Some(id) = c["id"].as_str() {
                    call.id = id.to_string();
                }
                if let Some(n) = c["function"]["name"].as_str() {
                    call.name.push_str(n);
                }
                if let Some(a) = c["function"]["arguments"].as_str() {
                    call.arguments.push_str(a);
                }
            }
        }
        if let Some(f) = choice["finish_reason"].as_str() {
            self.finish = Some(f.to_string());
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<Response, ProviderError> {
        if !self.done {
            return Err(stream_err("Chat Completions stream ended before [DONE]"));
        }
        let mut content = Vec::new();
        if !self.thinking.is_empty() {
            content.push(Part::Thinking {
                text: self.thinking,
                signature: None,
            });
        }
        if !self.text.is_empty() {
            content.push(Part::Text { text: self.text });
        }
        for c in self.calls {
            let arguments = if c.arguments.trim().is_empty() {
                Value::Object(Default::default())
            } else {
                serde_json::from_str(&c.arguments)
                    .map_err(|e| decode_err(format!("аргументы `{}`: {e}", c.name)))?
            };
            content.push(Part::ToolCall {
                id: c.id,
                name: c.name,
                arguments,
            });
        }
        let has_calls = content.iter().any(|p| matches!(p, Part::ToolCall { .. }));
        let stop_reason = match self.finish.as_deref() {
            // Часть локальных серверов ставит `stop` и при вызовах инструментов.
            Some("stop") if has_calls => StopReason::ToolUse,
            Some("stop") => StopReason::EndTurn,
            Some("tool_calls") => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
            Some("content_filter") => StopReason::Refusal,
            Some(other) => StopReason::Other(other.to_string()),
            None => return Err(decode_err("в потоке нет finish_reason")),
        };
        if stop_reason == StopReason::ToolUse && !has_calls {
            // Та же проверка у muse (`strings-all.txt:237351`).
            return Err(decode_err("Chat Completions reported tool_calls without a tool call"));
        }
        Ok(Response {
            content,
            stop_reason,
            usage: self.usage,
        })
    }
}
