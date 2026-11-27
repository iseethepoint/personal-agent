//! Перевод входа прослойки в тело запроса Messages API — по форме muse
//! `model/anthropic/convert_prompt.rs`, `convert_tool_spec.rs`
//! (`source-files.txt:5265-5266`). Умения шлются только по флагам весов.

use serde_json::{json, Map, Value};

use crate::registry::{AnthropicFacts, OutputEffort};
use crate::http::config;
use crate::types::{system_reminder, Effort, Part, ProviderError, Request, Role, ToolChoice, ToolResultPart};

/// Предел тела запроса Messages API — 32 МБ (документация API, ошибка 413);
/// у muse проверка до отправки — `enforce_request_size_budget.rs` (`source-files.txt:5267`).
pub const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;


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

/// Сообщение до склейки ролей: обычное или заметки движка подряд.
enum Item {
    Normal(&'static str, Vec<Value>),
    Developer(Vec<String>),
}

fn push_merged(out: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    // Подряд идущие сообщения одной роли склеиваются в одно.
    if let Some(last) = out.last_mut() {
        if last["role"] == role {
            if let Some(arr) = last["content"].as_array_mut() {
                arr.extend(blocks);
                return;
            }
        }
    }
    out.push(json!({"role": role, "content": blocks}));
}

/// Сообщения запроса. Заметки движка (`Role::Developer`) — встроенный
/// системный элемент muse подвида 1: при `mid_conversation_system_messages`
/// каждая заметка — отдельное сообщение `{"role":"system","content":
/// [{"type":"text","text":…}]}` на своём месте (`convert_prompt.rs`,
/// `0x20cd9f72`: строка `system` — `0x20cda00a`, массив `messages` —
/// `0x20cda16f`). Место должно быть допустимым: заметки подряд — сразу после
/// сообщения пользователя и перед ответом модели или в конце; так
/// проверяет и muse («system section must follow a user message», «system
/// section must precede an assistant message or end the array»,
/// `strings-all.txt:235512`). Иначе, и у весов без умения, — заметки через
/// перевод строки одним текстом в `<system-reminder>` внутри сообщения
/// пользователя (своё: ветку muse для таких весов не нашли в П1б; форма
/// обёртки — уведомления muse, `strings-all.txt:59333-59349`).
fn messages(req: &Request, facts: &AnthropicFacts) -> Vec<Value> {
    let mut items: Vec<Item> = Vec::new();
    for m in &req.messages {
        match m.role {
            Role::User => items.push(Item::Normal("user", m.content.iter().map(part_to_block).collect())),
            Role::Assistant => {
                items.push(Item::Normal("assistant", m.content.iter().map(part_to_block).collect()))
            }
            Role::Developer => {
                let text = m.text();
                if let Some(Item::Developer(run)) = items.last_mut() {
                    run.push(text);
                } else {
                    items.push(Item::Developer(vec![text]));
                }
            }
        }
    }
    let mut out: Vec<Value> = Vec::new();
    for i in 0..items.len() {
        match &items[i] {
            Item::Normal(role, blocks) => push_merged(&mut out, role, blocks.clone()),
            Item::Developer(run) => {
                let after_user = out.last().is_some_and(|l| l["role"] == "user");
                let before_ok = match items.get(i + 1) {
                    None => true,
                    Some(Item::Normal(role, _)) => *role == "assistant",
                    Some(Item::Developer(_)) => false,
                };
                if facts.mid_conversation_system_messages && after_user && before_ok {
                    for text in run {
                        out.push(json!({"role": "system", "content": [{"type": "text", "text": text}]}));
                    }
                } else {
                    let block = json!({"type": "text", "text": system_reminder(&run.join("\n"))});
                    push_merged(&mut out, "user", vec![block]);
                }
            }
        }
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
            if !facts.forced_tool_choice {
                // Ответ в заданной форме для таких моделей — в П3 по форме muse
                // `output_contract.rs`: `tool_choice` auto и текст «Complete the task
                // by calling `…` exactly once» (`strings-all.txt:235518`).
                return Err(config(format!(
                    "принудительный вызов `{name}` выключен у весов (anthropic.forced_tool_choice = false): \
                     сервер ответил бы 400; нужен tool_choice auto и указание в подсказке"
                )));
            }
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
