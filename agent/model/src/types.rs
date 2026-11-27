//! Один вход и один выход прослойки — одинаковые для всех поставщиков
//! (опись, раздел 4.1). Поставщик переводит их в свой протокол.

use serde::{Deserialize, Serialize};

/// Роль сообщения. `Developer` — заметки движка посреди разговора: метки
/// времени, напоминания, передачи от фоновых работ (у muse «developer
/// messages», `strings-all.txt:78528`). Как их передать — решает поставщик
/// по умениям весов (`mid_conversation_system_messages` / `developer_role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    Developer,
}

/// Картинка, base64 без префикса `data:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub media_type: String,
    pub data_base64: String,
}

/// Часть результата инструмента.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultPart {
    Text { text: String },
    Image(Image),
}

/// Часть сообщения.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text {
        text: String,
    },
    Image(Image),
    /// Вызов инструмента моделью; `arguments` — JSON-объект.
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    /// Результат вызова; `is_error` — признак ошибки инструмента.
    ToolResult {
        call_id: String,
        content: Vec<ToolResultPart>,
        is_error: bool,
    },
    /// Мышление модели. Возвращается в историю как есть, вместе с подписью.
    Thinking {
        text: String,
        signature: Option<String>,
    },
    /// Скрытое мышление: непрозрачные данные, тоже возвращаются как есть.
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Part>,
}

/// Описание инструмента: одно на всех поставщиков.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema параметров.
    pub input_schema: serde_json::Value,
}

/// Сила ответа (`output_effort` у muse).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

/// Выбор инструмента.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Модель решает сама.
    #[default]
    Auto,
    /// Без инструментов.
    None,
    /// Обязательный вызов этого инструмента — так у muse делается ответ
    /// в заданной форме (`output_contract.rs`, `strings-all.txt:235518`).
    Tool { name: String },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Предел ответа в токенах; не больше потолка весов.
    pub max_output_tokens: Option<u32>,
    /// Сила ответа; отправляется, только если умение есть у весов.
    pub effort: Option<Effort>,
    pub tool_choice: ToolChoice,
}

/// Вход прослойки.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Request {
    /// Неизменное начало подсказки (системный текст).
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub settings: Settings,
}

/// Причина остановки ответа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
    Other(String),
}

/// Счёт токенов. `reported = false` — поставщик `usage` не отдал: числа
/// нулевые, и это видно.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reported: bool,
}

/// Выход прослойки. В `content` — только `Text`, `ToolCall`, `Thinking`,
/// `RedactedThinking`, в том порядке, в каком их дала модель.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub content: Vec<Part>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

impl Response {
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &Part> {
        self.content
            .iter()
            .filter(|p| matches!(p, Part::ToolCall { .. }))
    }
}

/// Вид ошибки поставщика (форма `provider_error.rs`, опись, раздел 4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorKind {
    /// Ответ HTTP с кодом ошибки.
    Http { status: u16 },
    /// Обрыв или порча потока.
    Stream,
    /// Нет связи с сервером.
    Transport,
    /// Ответ не разбирается.
    Decode,
    /// Запрос нельзя собрать: умения нет у модели, нет ключа и т.п.
    Config,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub message: String,
    /// Можно повторить тот же запрос.
    pub retryable: bool,
    /// Подсказка сервера, через сколько повторить.
    pub retry_after_ms: Option<u64>,
}

impl ProviderError {
    /// Ошибка по коду HTTP: повторяются 429 и 5xx — так велят документации
    /// Messages API и Chat Completions.
    pub fn http(status: u16, message: impl Into<String>, retry_after_ms: Option<u64>) -> Self {
        let retryable = status == 429 || status >= 500;
        ProviderError {
            kind: ProviderErrorKind::Http { status },
            message: message.into(),
            retryable,
            retry_after_ms,
        }
    }

    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        let retryable = matches!(kind, ProviderErrorKind::Stream | ProviderErrorKind::Transport);
        ProviderError {
            kind,
            message: message.into(),
            retryable,
            retry_after_ms: None,
        }
    }
}
