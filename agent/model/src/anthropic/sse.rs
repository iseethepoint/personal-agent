//! Разбор потока SSE Messages API — по форме muse `parse_sse_stream.rs`,
//! `sse_parser_state_machine.rs` (`source-files.txt:5269,5273`).
//! Поток собирается в один `Response`; блоки идут в порядке индексов.

use std::io::BufRead;

use serde_json::Value;

use crate::types::{Part, ProviderError, ProviderErrorKind, Response, StopReason, Usage};

/// Собираемый блок содержимого.
enum Block {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        json: String,
    },
    Thinking {
        text: String,
        signature: Option<String>,
    },
    Redacted(String),
}

#[derive(Default)]
struct State {
    blocks: Vec<Option<Block>>,
    usage: Usage,
    stop_reason: Option<StopReason>,
    stopped: bool,
}

fn stream_err(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Stream, msg)
}

fn decode_err(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Decode, msg)
}

fn u(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

fn apply_usage(usage: &mut Usage, v: &Value) {
    if let Some(n) = u(v, "input_tokens") {
        usage.input_tokens = n;
    }
    if let Some(n) = u(v, "output_tokens") {
        usage.output_tokens = n;
    }
    if let Some(n) = u(v, "cache_read_input_tokens") {
        usage.cache_read_tokens = n;
    }
    if let Some(n) = u(v, "cache_creation_input_tokens") {
        usage.cache_write_tokens = n;
    }
    usage.reported = true;
}

fn stop_reason(s: &str) -> StopReason {
    match s {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "refusal" => StopReason::Refusal,
        other => StopReason::Other(other.to_string()),
    }
}

/// Ошибка внутри потока (`event: error`): повтор по типу ошибки.
fn stream_error_event(v: &Value) -> ProviderError {
    let kind = v["error"]["type"].as_str().unwrap_or("");
    let message = v["error"]["message"].as_str().unwrap_or("").to_string();
    let retryable = matches!(kind, "overloaded_error" | "api_error" | "rate_limit_error");
    ProviderError {
        kind: ProviderErrorKind::Stream,
        message: format!("{kind}: {message}"),
        retryable,
        retry_after_ms: None,
    }
}

impl State {
    fn slot(&mut self, index: usize) -> &mut Option<Block> {
        if self.blocks.len() <= index {
            self.blocks.resize_with(index + 1, || None);
        }
        &mut self.blocks[index]
    }

    fn event(&mut self, data: &str) -> Result<(), ProviderError> {
        let v: Value = serde_json::from_str(data).map_err(|e| decode_err(format!("SSE data: {e}")))?;
        let index = u(&v, "index").unwrap_or(0) as usize;
        match v["type"].as_str().unwrap_or("") {
            "message_start" => apply_usage(&mut self.usage, &v["message"]["usage"]),
            "content_block_start" => {
                let b = &v["content_block"];
                let block = match b["type"].as_str().unwrap_or("") {
                    "text" => Block::Text(b["text"].as_str().unwrap_or("").to_string()),
                    "tool_use" => Block::ToolCall {
                        id: b["id"].as_str().unwrap_or("").to_string(),
                        name: b["name"].as_str().unwrap_or("").to_string(),
                        json: String::new(),
                    },
                    "thinking" => Block::Thinking {
                        text: b["thinking"].as_str().unwrap_or("").to_string(),
                        signature: b["signature"].as_str().map(str::to_string),
                    },
                    "redacted_thinking" => Block::Redacted(b["data"].as_str().unwrap_or("").to_string()),
                    other => return Err(decode_err(format!("незнакомый блок `{other}`"))),
                };
                *self.slot(index) = Some(block);
            }
            "content_block_delta" => {
                let d = &v["delta"];
                let Some(block) = self.slot(index).as_mut() else {
                    return Err(decode_err(format!("дельта для блока {index} без начала")));
                };
                match (d["type"].as_str().unwrap_or(""), block) {
                    ("text_delta", Block::Text(t)) => t.push_str(d["text"].as_str().unwrap_or("")),
                    ("input_json_delta", Block::ToolCall { json, .. }) => {
                        json.push_str(d["partial_json"].as_str().unwrap_or(""))
                    }
                    ("thinking_delta", Block::Thinking { text, .. }) => {
                        text.push_str(d["thinking"].as_str().unwrap_or(""))
                    }
                    ("signature_delta", Block::Thinking { signature, .. }) => {
                        signature.get_or_insert_with(String::new).push_str(d["signature"].as_str().unwrap_or(""))
                    }
                    (t, _) => return Err(decode_err(format!("дельта `{t}` не подходит блоку {index}"))),
                }
            }
            "content_block_stop" | "ping" => {}
            "message_delta" => {
                if let Some(s) = v["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(stop_reason(s));
                }
                if v.get("usage").is_some() {
                    apply_usage(&mut self.usage, &v["usage"]);
                }
            }
            "message_stop" => self.stopped = true,
            "error" => return Err(stream_error_event(&v)),
            other => return Err(decode_err(format!("незнакомое событие `{other}`"))),
        }
        Ok(())
    }

    fn finish(self) -> Result<Response, ProviderError> {
        if !self.stopped {
            return Err(stream_err("поток оборвался до message_stop"));
        }
        let mut content = Vec::new();
        for b in self.blocks.into_iter().flatten() {
            content.push(match b {
                Block::Text(text) => Part::Text { text },
                Block::ToolCall { id, name, json } => {
                    let arguments = if json.trim().is_empty() {
                        Value::Object(Default::default())
                    } else {
                        serde_json::from_str(&json)
                            .map_err(|e| decode_err(format!("аргументы `{name}`: {e}")))?
                    };
                    Part::ToolCall { id, name, arguments }
                }
                Block::Thinking { text, signature } => Part::Thinking { text, signature },
                Block::Redacted(data) => Part::RedactedThinking { data },
            });
        }
        let stop_reason = self
            .stop_reason
            .ok_or_else(|| decode_err("в потоке нет stop_reason"))?;
        Ok(Response {
            content,
            stop_reason,
            usage: self.usage,
        })
    }
}

/// Читает поток до конца. Ошибка чтения (обрыв связи) — ошибка потока,
/// её можно повторить.
pub fn read_stream(reader: impl BufRead) -> Result<Response, ProviderError> {
    let mut state = State::default();
    let mut data = String::new();
    for line in reader.lines() {
        let line = line.map_err(|e| stream_err(format!("чтение потока: {e}")))?;
        if line.is_empty() {
            if !data.is_empty() {
                state.event(&data)?;
                data.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // Строки `event:` и комментарии не нужны: тип есть в самом JSON.
    }
    if !data.is_empty() {
        state.event(&data)?;
    }
    state.finish()
}
