//! Карантин куска подсказки — как у muse (`message_execution ›
//! prompt_quarantine.rs`, `turn_execution › anthropic_prompt_poison.rs`,
//! `strings-all.txt:54780-54782`): ошибка поставщика `invalid_request` с
//! признаком битой картинки (список ядра около `0x22478f2`: «could not
//! process image», «invalid base64 image» и др.) — «prompt poison».
//! Кусок, на котором модель падает, удаляется из истории и из
//! `agent.context_items` (`DELETE FROM agent.context_items WHERE agent_id =
//! $1 AND seq = ANY($2)`, `removed_context_seq_count`); вызов инструмента —
//! вместе с результатом (`tool_call_count`, «quarantined tool-call chat
//! events»). Кусок из прошлого — карантин и повтор вызова модели («quarantine
//! historical poison»); кусок текущего шага (`current_iteration_input_
//! quarantined`) — карантин и ошибка хода «request failed because part of
//! the input context is unsupported; the failing input was excluded from
//! future context to keep the session recoverable (…)». Кандидата нет
//! («prompt poison recovery found no quarantine candidate») — ошибка как
//! есть.
//!
//! Своё (в разборе не видно): место берётся из `messages.N` / `messages[N]`
//! ошибки, если там картинка, иначе — последнее сообщение с картинкой;
//! вызов уходит вместе с сообщением ассистента целиком; текущий шаг —
//! сообщения после последнего ответа модели; в скобках — текст ошибки
//! поставщика; правило одно для обоих поставщиков.

use agent_model::{Message, Part, ProviderError, ProviderErrorKind, Role, ToolResultPart};

/// Признаки битой картинки в ошибке поставщика — строки ядра.
pub const POISON_MARKERS: [&str; 10] = [
    "turbojpeg decompress failed",
    "premature end of jpeg file",
    "could not decode image",
    "could not process image",
    "unable to decode image",
    "failed to decode image",
    "image decode failed",
    "invalid image data",
    "invalid base64 image",
    "unsupported gif variant",
];

pub const CURRENT_INPUT_QUARANTINED: &str =
    "request failed because part of the input context is unsupported; the failing input was excluded from future context to keep the session recoverable";

/// Ошибка — «prompt poison»: отказ в запросе (400) с признаком битой картинки.
pub fn is_poison(e: &ProviderError) -> bool {
    let lower = e.message.to_lowercase();
    matches!(e.kind, ProviderErrorKind::Http { status: 400 }) && POISON_MARKERS.iter().any(|m| lower.contains(m))
}

/// Номер сообщения из `messages.N` / `messages[N]`.
fn hinted(message: &str) -> Option<usize> {
    for pat in ["messages.", "messages["] {
        if let Some(i) = message.find(pat) {
            let digits: String = message[i + pat.len()..].chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse() {
                return Some(n);
            }
        }
    }
    None
}

fn has_image(m: &Message) -> bool {
    m.content.iter().any(|p| match p {
        Part::Image(_) => true,
        Part::ToolResult { content, .. } => content.iter().any(|c| matches!(c, ToolResultPart::Image(_))),
        _ => false,
    })
}

/// Что уйдёт в карантин: номера сообщений по возрастанию. Сообщение с
/// результатами — вместе с сообщением ассистента, где их вызовы.
pub fn candidate(history: &[Message], e: &ProviderError) -> Option<Vec<usize>> {
    let at = match hinted(&e.message) {
        Some(i) if history.get(i).is_some_and(has_image) => i,
        _ => history.iter().rposition(has_image)?,
    };
    let mut out = vec![at];
    let ids: Vec<&str> = history[at]
        .content
        .iter()
        .filter_map(|p| if let Part::ToolResult { call_id, .. } = p { Some(call_id.as_str()) } else { None })
        .collect();
    if !ids.is_empty() {
        if let Some(c) = history[..at].iter().rposition(|m| m.content.iter().any(|p| matches!(p, Part::ToolCall { id, .. } if ids.contains(&id.as_str())))) {
            out.insert(0, c);
        }
    }
    Some(out)
}

/// Кусок — из текущего шага: сообщение с картинкой стоит после последнего
/// ответа модели.
pub fn is_current(history: &[Message], removed: &[usize]) -> bool {
    let max = removed.iter().copied().max().unwrap_or(0);
    history.iter().rposition(|m| m.role == Role::Assistant).is_none_or(|a| max > a)
}

/// Ошибка хода после карантина текущего шага.
pub fn current_error(e: &ProviderError) -> ProviderError {
    ProviderError { kind: e.kind.clone(), message: format!("{CURRENT_INPUT_QUARANTINED} ({})", e.message), retryable: false, retry_after_ms: None }
}
