//! Сборка подсказки — по форме `prompt_assembly/*` muse
//! (`source-files.txt:1365-1394`).
//!
//! - Неизменное начало (`prompt_prefix_pin`): системный текст, файлы домашней
//!   папки, описания инструментов. Собирается один раз за разговор и дальше
//!   не меняется — между ходами начало запроса совпадает байт в байт, и кэш
//!   подсказки у поставщика не сбивается.
//! - Изменчивое (`volatile_context_pin`): время, откуда пришло сообщение —
//!   заметкой `Role::Developer` в хвосте разговора. Новая заметка — только
//!   когда изменчивое изменилось (у muse тела заметок хранятся по хэшу,
//!   `INSERT … ON CONFLICT (body_hash) DO NOTHING`, `strings-all.txt:68724`).
//! - Оценка длины (`context_estimate.rs`): по счёту токенов поставщика, а
//!   где его нет — по словарю `o200k_base`, как `o200k_count.rs` muse
//!   (`source-files.txt:5307`; словарь вшит в ядро, `strings-all.txt:237424`).
//!
//! Тексты muse (системный текст, текст заметки) — не здесь, это П5.

use std::path::Path;

use agent_model::{Message, Part, Role, ToolResultPart, ToolSpec, Usage};

use crate::tool::Tools;

/// Файлы домашней папки в начале подсказки, в порядке списка muse
/// (`blocks/chat/runtime_files_list.md`, `strings-all.txt:59646-59655`).
pub const HOME_FILES: [&str; 5] = ["AGENTS.md", "SOUL.md", "IDENTITY.md", "USER.md", "MEMORY.md"];

/// Неизменное начало подсказки разговора.
#[derive(Debug, Clone, PartialEq)]
pub struct Prefix {
    pub system: String,
    pub tools: Vec<ToolSpec>,
}

impl Prefix {
    /// Собрать начало: системный текст, за ним файлы домашней папки в
    /// обёртке muse `<!-- INJECTED FILE: … -->` (ядро 0xa70d20), и
    /// описания инструментов из реестра. Нечитаемый или отсутствующий файл
    /// читается как пустой — как у muse («home file unreadable; read as
    /// empty», `strings-all.txt:76316`).
    /// Если есть отложенные пространства — после системного текста блок
    /// «Loading Tools» (`tool_search.rs`), а модели сразу уходят только
    /// неотложенные описания.
    pub fn assemble(system: &str, home: &Path, tools: &Tools) -> Prefix {
        let mut text = system.to_string();
        if let Some(block) = tools.loading_block() {
            text = if text.is_empty() { block } else { format!("{}\n\n{block}", text.trim_end()) };
        }
        for name in HOME_FILES {
            let body = std::fs::read_to_string(home.join(name)).unwrap_or_default();
            text.push_str(&format!(
                "\n\n<!-- INJECTED FILE: {name} -->\n\n{body}\n\n<!-- END INJECTED FILE: {name} -->"
            ));
        }
        Prefix { system: text, tools: tools.upfront_specs() }
    }
}

/// Разговор: неизменное начало, история, последняя заметка изменчивого и
/// опора оценки длины.
#[derive(Debug, Clone)]
pub struct Conversation {
    pub prefix: Prefix,
    pub history: Vec<Message>,
    last_volatile: Option<String>,
    /// (длина истории, токенов по счёту поставщика) после последнего ответа.
    anchor: Option<(usize, u64)>,
}

impl Conversation {
    pub fn new(prefix: Prefix) -> Self {
        Conversation { prefix, history: Vec::new(), last_volatile: None, anchor: None }
    }

    /// Разговор, восстановленный после перезапуска: закреплённое начало,
    /// история и последняя заметка изменчивого.
    pub fn restore(prefix: Prefix, history: Vec<Message>, last_volatile: Option<String>) -> Self {
        Conversation { prefix, history, last_volatile, anchor: None }
    }

    /// Последняя заметка изменчивого, ушедшая в историю.
    pub fn last_volatile(&self) -> Option<&str> {
        self.last_volatile.as_deref()
    }

    /// Сообщение владельца и, если изменилось, заметка изменчивого за ним —
    /// в хвост разговора.
    pub fn push_user(&mut self, content: Vec<Part>, volatile: &str) {
        self.history.push(Message { role: Role::User, content });
        if self.last_volatile.as_deref() != Some(volatile) {
            self.history.push(Message {
                role: Role::Developer,
                content: vec![Part::Text { text: volatile.to_string() }],
            });
            self.last_volatile = Some(volatile.to_string());
        }
    }

    /// Запомнить счёт поставщика после ответа, который последним лёг в
    /// историю. Без счёта (`reported = false`) опора не ставится.
    pub fn record_usage(&mut self, usage: &Usage) {
        if usage.reported {
            let tokens = usage.input_tokens
                + usage.cache_read_tokens
                + usage.cache_write_tokens
                + usage.output_tokens;
            self.anchor = Some((self.history.len(), tokens));
        }
    }

    /// Заменить историю сжатой. Опора счёта поставщика снимается: до
    /// следующего ответа оценка — по o200k. Начало не меняется.
    pub fn replace_history(&mut self, history: Vec<Message>) {
        self.history = history;
        self.anchor = None;
    }

    /// Оценка длины разговора в токенах: счёт поставщика на последнем
    /// ответе плюс o200k того, что легло после; без счёта — o200k всего
    /// (начало, описания инструментов, история).
    pub fn estimate_tokens(&self) -> u64 {
        match self.anchor {
            Some((len, tokens)) if len <= self.history.len() => {
                tokens + self.history[len..].iter().map(message_tokens).sum::<u64>()
            }
            _ => {
                let tools: u64 = self
                    .prefix
                    .tools
                    .iter()
                    .map(|t| o200k_count(&serde_json::to_string(t).unwrap_or_default()))
                    .sum();
                o200k_count(&self.prefix.system) + tools + self.history.iter().map(message_tokens).sum::<u64>()
            }
        }
    }
}

/// Токенов по словарю `o200k_base`.
pub fn o200k_count(text: &str) -> u64 {
    tiktoken_rs::o200k_base_singleton().encode_ordinary(text).len() as u64
}

/// Токенов в сообщении: тексты, мышление, вызовы (имя и аргументы),
/// результаты. Картинки не считаются.
fn message_tokens(m: &Message) -> u64 {
    m.content
        .iter()
        .map(|p| match p {
            Part::Text { text } | Part::Thinking { text, .. } => o200k_count(text),
            Part::RedactedThinking { data } => o200k_count(data),
            Part::ToolCall { name, arguments, .. } => o200k_count(name) + o200k_count(&arguments.to_string()),
            Part::ToolResult { content, .. } => content
                .iter()
                .map(|c| match c {
                    ToolResultPart::Text { text } => o200k_count(text),
                    ToolResultPart::Image(_) => 0,
                })
                .sum(),
            Part::Image(_) => 0,
        })
        .sum()
}
