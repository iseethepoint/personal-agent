//! Перевод входа прослойки в тело запроса Messages API — по форме muse
//! `model/anthropic/convert_prompt.rs`, `convert_tool_spec.rs`
//! (`source-files.txt:5265-5266`). Умения шлются только по флагам весов.

use serde_json::{json, Map, Value};

use crate::registry::{AnthropicFacts, OutputEffort};
use crate::types::{Effort, Message, Part, ProviderError, ProviderErrorKind, Request, Role, ToolChoice, ToolResultPart};

/// Предел тела запроса Messages API — 32 МБ (документация API, ошибка 413);
/// у muse проверка до отправки — `enforce_request_size_budget.rs` (`source-files.txt:5267`).
pub const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;

fn config(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Config, msg)
}

/// Умения, которые прослойка пока не умеет. Включены в весах — ошибка,
/// а не тихое игнорирование.
pub fn check_supported(facts: &AnthropicFacts) -> Result<(), ProviderError> {
    if facts.compaction {
        return Err(config("anthropic.compaction включено, но прослойка его не поддерживает"));
    }
    if facts.tool_search {
        return Err(config("anthropic.tool_search включено, но прослойка его не поддерживает"));
    }
    Ok(())
}

fn effort_name(effort: Effort, mode: OutputEffort) -> Option<&'static str> {
    let name = match (effort, mode) {
        (_, OutputEffort::Unsupported) => return None,
        (Effort::Low, _) => "low",
        (Effort::Medium, _) => "medium",
        (Effort::High, _) => "high",
        // `xhigh_as_max` у muse (`strings-all.txt:235847`): у весов нет xhigh.
        (Effort::Xhigh, OutputEffort::XhighAsMax) => "max",
        (Effort::Xhigh, _) => "xhigh",
        (Effort::Max, _) => "max",
    };
    Some(name)
}

fn image_block(media_type: &str, data: &str) -> Value {
    json!({"type": "image", "source": {"type": "base64", "media_type": media_type, "data": data}})
}

fn part_to_block(p: &Part) -> Value {
    match p {
        Part::Text { text } => json!({"type": "text", "text": text}),
        Part::Image(i) => image_block(&i.media_type, &i.data_base64),
        Part::ToolCall {
            id,
            name,
            arguments,
        } => json!({"type": "tool_use", "id": id, "name": name, "input": arguments}),
        Part::ToolResult {
            call_id,
            content,
            is_error,
        } => {
            let content: Vec<Value> = content
                .iter()
                .map(|c| match c {
                    ToolResultPart::Text { text } => json!({"type": "text", "text": text}),
                    ToolResultPart::Image(i) => image_block(&i.media_type, &i.data_base64),
                })
                .collect();
            let mut b = json!({"type": "tool_result", "tool_use_id": call_id, "content": content});
            if *is_error {
                b["is_error"] = json!(true);
            }
            b
        }
        Part::Thinking { text, signature } => {
            let mut b = json!({"type": "thinking", "thinking": text});
            if let Some(s) = signature {
                b["signature"] = json!(s);
            }
            b
        }
        Part::RedactedThinking { data } => json!({"type": "redacted_thinking", "data": data}),
    }
}

/// Заметка движка без поддержки системных сообщений посреди разговора:
/// текст в обёртке `<system-reminder>` внутри сообщения пользователя —
/// так muse передаёт уведомления (`strings-all.txt:59333-59349`).
fn developer_as_user_text(m: &Message) -> Value {
    let text: String = m
        .content
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    json!({"type": "text", "text": format!("<system-reminder>\n{text}\n</system-reminder>")})
}

fn messages(req: &Request, facts: &AnthropicFacts) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for m in &req.messages {
        let (role, blocks): (&str, Vec<Value>) = match m.role {
            Role::User => ("user", m.content.iter().map(part_to_block).collect()),
            Role::Assistant => ("assistant", m.content.iter().map(part_to_block).collect()),
            Role::Developer if facts.mid_conversation_system_messages => {
                let text: String = m
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        Part::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                out.push(json!({"role": "system", "content": text}));
                continue;
            }
            Role::Developer => ("user", vec![developer_as_user_text(m)]),
        };
        // Подряд идущие сообщения одной роли склеиваются в одно.
        if let Some(last) = out.last_mut() {
            if last["role"] == role {
                if let Some(arr) = last["content"].as_array_mut() {
                    arr.extend(blocks);
                    continue;
                }
            }
        }
        out.push(json!({"role": role, "content": blocks}));
    }
    out
}

/// Тело запроса. `stream: true` всегда: ответ читается потоком SSE.
pub fn request_body(model: &str, req: &Request, facts: &AnthropicFacts) -> Result<Value, ProviderError> {
    check_supported(facts)?;
    let ceiling = facts.runtime_output_ceiling;
    let max_tokens = match (req.settings.max_output_tokens, ceiling) {
        (Some(want), Some(c)) => want.min(c),
        (Some(want), None) => want,
        (None, Some(c)) => c,
        (None, None) => {
            return Err(config(
                "нет предела ответа: ни settings.max_output_tokens, ни anthropic.runtime_output_ceiling",
            ))
        }
    };
    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("max_tokens".into(), json!(max_tokens));
    body.insert("stream".into(), json!(true));
    if !req.system.is_empty() {
        let mut block = json!({"type": "text", "text": req.system});
        if facts.prompt_cache {
            block["cache_control"] = json!({"type": "ephemeral"});
        }
        body.insert("system".into(), json!([block]));
    }
    body.insert("messages".into(), Value::Array(messages(req, facts)));
    if !req.tools.is_empty() {
        let n = req.tools.len();
        let tools: Vec<Value> = req
            .tools
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut v = json!({"name": t.name, "description": t.description, "input_schema": t.input_schema});
                if facts.prompt_cache && i + 1 == n {
                    v["cache_control"] = json!({"type": "ephemeral"});
                }
                v
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
    }
    match &req.settings.tool_choice {
        ToolChoice::Auto => {}
        ToolChoice::None => {
            body.insert("tool_choice".into(), json!({"type": "none"}));
        }
        ToolChoice::Tool { name } => {
            body.insert("tool_choice".into(), json!({"type": "tool", "name": name}));
        }
    }
    if facts.adaptive_thinking {
        let display = if facts.omit_thinking_display { "omitted" } else { "summarized" };
        body.insert("thinking".into(), json!({"type": "adaptive", "display": display}));
    }
    if let Some(name) = req.settings.effort.and_then(|e| effort_name(e, facts.output_effort)) {
        body.insert("output_config".into(), json!({"effort": name}));
    }
    let body = Value::Object(body);
    let size = serde_json::to_vec(&body).map(|v| v.len()).unwrap_or(0);
    if size > MAX_REQUEST_BYTES {
        return Err(config(format!(
            "тело запроса {size} байт больше предела {MAX_REQUEST_BYTES}"
        )));
    }
    Ok(body)
}
