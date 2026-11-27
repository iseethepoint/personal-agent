//! Перевод входа прослойки в тело Chat Completions — по форме muse
//! `model/kimi_chat_completions/prompt.rs` (`source-files.txt:5302-5303`):
//! `model`, `stream`, `tools`, `tool_choice`, `parallel_tool_calls`
//! (`strings-all.txt:237328`). Умения — только по флагам `openai_compat.*`.

use serde_json::{json, Map, Value};

use crate::http::config;
use crate::registry::{ImageFormat, OpenaiCompatFacts};
use crate::types::{system_reminder, Effort, Image, Part, ProviderError, Request, Role, ToolChoice, ToolResultPart};

/// Умения, которых в Chat Completions нет. Включены в весах — ошибка.
pub fn check_supported(facts: &OpenaiCompatFacts) -> Result<(), ProviderError> {
    if facts.tool_output_error_status {
        return Err(config(
            "openai_compat.tool_output_error_status включено, но в Chat Completions нет признака ошибки результата",
        ));
    }
    Ok(())
}

/// Ошибка инструмента — текстом результата: признака ошибки в Chat
/// Completions нет (опись, раздел 3).
pub const TOOL_ERROR_PREFIX: &str = "[ошибка инструмента]\n";

fn format_of(media_type: &str) -> Option<ImageFormat> {
    match media_type {
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" | "image/jpg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        "image/webp" => Some(ImageFormat::Webp),
        _ => None,
    }
}

fn image_part(i: &Image, facts: &OpenaiCompatFacts) -> Result<Value, ProviderError> {
    if !facts.vision {
        return Err(config("картинка на входе, но у весов выключено openai_compat.vision"));
    }
    if !facts.image_formats.is_empty() {
        let ok = format_of(&i.media_type).is_some_and(|f| facts.image_formats.contains(&f));
        if !ok {
            return Err(config(format!(
                "формат картинки `{}` не входит в openai_compat.image_formats",
                i.media_type
            )));
        }
    }
    Ok(json!({"type": "image_url", "image_url": {"url": format!("data:{};base64,{}", i.media_type, i.data_base64)}}))
}

/// Содержимое сообщения: одна строка, если там только текст (так понимают
/// все серверы), иначе массив частей.
fn content(parts: Vec<Value>) -> Value {
    if parts.iter().all(|p| p["type"] == "text") {
        let text: Vec<&str> = parts.iter().filter_map(|p| p["text"].as_str()).collect();
        Value::String(text.join("\n"))
    } else {
        Value::Array(parts)
    }
}

/// Текст результата для роли `tool` и картинки результата. Роль `tool` в
/// Chat Completions принимает только текст, поэтому картинки уходят
/// следом сообщением `user` после всех результатов шага (у muse для Chat
/// Completions есть свой модуль медиа — `kimi_chat_completions/prompt/media.rs`,
/// `source-files.txt:5303`; вид пометки — своё). Без `vision` картинка —
/// ошибка, как и в сообщении пользователя.
fn tool_result(call_id: &str, content: &[ToolResultPart], is_error: bool, facts: &OpenaiCompatFacts) -> Result<(String, Vec<Value>), ProviderError> {
    let mut text = String::new();
    let mut images = Vec::new();
    if is_error {
        text.push_str(TOOL_ERROR_PREFIX);
    }
    for c in content {
        match c {
            ToolResultPart::Text { text: t } => text.push_str(t),
            ToolResultPart::Image(i) => {
                if images.is_empty() {
                    images.push(json!({"type": "text", "text": format!("[картинки результата инструмента {call_id}]")}));
                }
                images.push(image_part(i, facts)?);
            }
        }
    }
    Ok((text, images))
}

/// Подряд идущие сообщения user склеиваются (заметка движка без
/// `developer_role` уходит текстом в сообщение пользователя).
fn push_user(out: &mut Vec<Value>, parts: Vec<Value>) {
    if let Some(last) = out.last_mut() {
        if last["role"] == "user" {
            let mut prev = match last["content"].take() {
                Value::String(s) => vec![json!({"type": "text", "text": s})],
                Value::Array(a) => a,
                _ => Vec::new(),
            };
            prev.extend(parts);
            last["content"] = content(prev);
            return;
        }
    }
    out.push(json!({"role": "user", "content": content(parts)}));
}

fn messages(req: &Request, facts: &OpenaiCompatFacts) -> Result<Vec<Value>, ProviderError> {
    let mut out = Vec::new();
    if !req.system.is_empty() {
        out.push(json!({"role": "system", "content": req.system}));
    }
    for m in &req.messages {
        match m.role {
            Role::User => {
                let mut parts = Vec::new();
                let mut result_images = Vec::new();
                for p in &m.content {
                    match p {
                        Part::Text { text } => parts.push(json!({"type": "text", "text": text})),
                        Part::Image(i) => parts.push(image_part(i, facts)?),
                        Part::ToolResult {
                            call_id,
                            content,
                            is_error,
                        } => {
                            if !parts.is_empty() {
                                push_user(&mut out, std::mem::take(&mut parts));
                            }
                            let (text, images) = tool_result(call_id, content, *is_error, facts)?;
                            out.push(json!({
                                "role": "tool",
                                "tool_call_id": call_id,
                                "content": text,
                            }));
                            result_images.extend(images);
                        }
                        // Мышление и вызовы в сообщении пользователя не бывают.
                        _ => {}
                    }
                }
                result_images.extend(parts);
                if !result_images.is_empty() {
                    push_user(&mut out, result_images);
                }
            }
            Role::Assistant => {
                let mut text = String::new();
                let mut calls = Vec::new();
                for p in &m.content {
                    match p {
                        Part::Text { text: t } => text.push_str(t),
                        Part::ToolCall {
                            id,
                            name,
                            arguments,
                        } => calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {"name": name, "arguments": arguments.to_string()},
                        })),
                        // Мышление в Chat Completions назад не передаётся: поля для
                        // него во входе нет (`reasoning_content` — только в ответе).
                        _ => {}
                    }
                }
                let mut msg = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { Value::String(text) }});
                if !calls.is_empty() {
                    msg["tool_calls"] = Value::Array(calls);
                }
                out.push(msg);
            }
            Role::Developer => {
                let text = m.text();
                // Роль встроенного системного элемента подвида 1 у muse в
                // Chat Completions — `developer` (`kimi_chat_completions/
                // prompt.rs`, таблица ролей `0x297732c0` по байту подвида,
                // `0x20ad26da`). Без умения — `<system-reminder>` в сообщении
                // пользователя (своё).
                if facts.developer_role {
                    out.push(json!({"role": "developer", "content": text}));
                } else {
                    push_user(&mut out, vec![json!({"type": "text", "text": system_reminder(&text)})]);
                }
            }
        }
    }
    Ok(out)
}

fn effort_name(e: Effort) -> &'static str {
    match e {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
        Effort::Xhigh => "xhigh",
        Effort::Max => "max",
    }
}

/// Тело запроса Chat Completions; поток всегда, со счётом токенов в конце.
pub fn request_body(model: &str, req: &Request, facts: &OpenaiCompatFacts) -> Result<Value, ProviderError> {
    check_supported(facts)?;
    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("stream".into(), json!(true));
    body.insert("stream_options".into(), json!({"include_usage": true}));
    body.insert("messages".into(), Value::Array(messages(req, facts)?));
    let max = match (req.settings.max_output_tokens, facts.runtime_output_ceiling) {
        (Some(w), Some(c)) => Some(w.min(c)),
        (w, c) => w.or(c),
    };
    if let Some(n) = max {
        let field = if facts.max_completion_tokens { "max_completion_tokens" } else { "max_tokens" };
        body.insert(field.into(), json!(n));
    }
    if !req.tools.is_empty() {
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.input_schema}}))
            .collect();
        body.insert("tools".into(), Value::Array(tools));
        if facts.parallel_tool_calls {
            body.insert("parallel_tool_calls".into(), json!(true));
        }
    }
    match &req.settings.tool_choice {
        ToolChoice::Auto => {}
        ToolChoice::None => {
            body.insert("tool_choice".into(), json!("none"));
        }
        ToolChoice::Tool { name } => {
            body.insert(
                "tool_choice".into(),
                json!({"type": "function", "function": {"name": name}}),
            );
        }
    }
    if facts.reasoning {
        if let Some(e) = req.settings.effort {
            body.insert("reasoning_effort".into(), json!(effort_name(e)));
        }
    }
    Ok(Value::Object(body))
}
