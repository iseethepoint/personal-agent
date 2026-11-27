//! Сжатие разговора и сброс памяти перед ним — по форме muse
//! `hatch-compaction` (`source-files.txt:2145-2150`), `memory_flush/*`
//! (`:1296-1302`), `compaction_model.rs` (`:1310`).
//!
//! Когда оценка длины разговора дошла до `compaction_trigger` весов главной
//! модели:
//! 1. ход сброса памяти — модель задачи `MemoryFlush` с инструментами
//!    записывает важное в файлы; сам ход в историю не идёт;
//! 2. сводка старой части — модель задачи `Compaction`, тексты muse
//!    `compact/system.md`, `compact/summarization.md`;
//! 3. история = сводка + последний ход владельца целиком: его вызовы с
//!    результатами и блоки мышления не трогаются. Неизменное начало (П3б)
//!    сжатие не меняет.
//!
//! Периодический сброс — как у muse (`memory_flush/periodic_coverage.rs`,
//! `maintenance/handlers/periodic_memory_flush.rs`): когда в разговоре со
//! времени прошлого сброса набралось `JARVIS_MEMORY_FLUSH_PERIODIC_
//! THRESHOLD_TOKENS` токенов (по умолчанию 50 000, `0xc350` в ядре около
//! `0xac1408c`), тот же ход сброса идёт между ходами («periodic memory
//! flush started / completed / failed»); сброс перед сжатием пропускается,
//! если всё уже сброшено (`periodic_flush_covered`). Перед просьбой о сбросе
//! — сегодняшний журнал памяти («Today's memory log at ~/… already ends with
//! the entries below…», строка ядра около `0x6bf7fc`), если файл есть.

use std::path::PathBuf;

use agent_model::{
    Message, Part, Provider, Purpose, Registry, RegistryError, Request, Resolved, Role, Settings,
    ToolChoice,
};

use crate::prompt::Conversation;
use crate::tool::Tools;
use crate::turn::{Turn, TurnError};

/// Тексты muse, перенесены как есть (`prompts/compaction/`).
pub const SYSTEM_MD: &str = include_str!("../../../prompts/compaction/system.md");
pub const SUMMARIZATION_MD: &str = include_str!("../../../prompts/compaction/summarization.md");
pub const SUMMARY_RULES_MD: &str = include_str!("../../../prompts/compaction/summary_rules.md");
pub const MEMORY_FLUSH_PROMPT_MD: &str = include_str!("../../../prompts/compaction/memory_flush_prompt.md");

/// Порог периодического сброса памяти в токенах (по умолчанию ядра).
pub const PERIODIC_THRESHOLD_TOKENS: u64 = 50_000;

/// Сегодняшний журнал памяти перед просьбой о сбросе.
pub fn today_log_note(path: &str, content: &str) -> String {
    format!("Today's memory log at ~/{path} already ends with the entries below. Do not re-append anything already recorded there; append only new entries and corrections.\n\n{content}")
}

/// Заголовок сводки в истории — строка ядра muse (0xb3a958).
pub const SUMMARY_HEADER: &str = "Context checkpoint compaction summary:\n";

const PERSONAL_START: &str = "<!-- personal-only:start -->";
const PERSONAL_END: &str = "<!-- personal-only:end -->";
const GROUP_START: &str = "<!-- group-only:start -->";
const GROUP_END: &str = "<!-- group-only:end -->";

/// Личный вариант текста muse: куски `personal-only` остаются без
/// меток, куски `group-only` убираются. Строка, на которой были
/// только метки, убирается целиком.
pub fn personal(text: &str) -> String {
    const MARK: char = '\u{0}';
    let mut marked = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(GROUP_START) {
        marked.push_str(&rest[..i]);
        marked.push(MARK);
        let after = &rest[i + GROUP_START.len()..];
        rest = match after.find(GROUP_END) {
            Some(j) => &after[j + GROUP_END.len()..],
            None => "",
        };
    }
    marked.push_str(rest);
    let marked = marked.replace(PERSONAL_START, "\u{0}").replace(PERSONAL_END, "\u{0}");
    let mut out = String::new();
    for line in marked.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        if !body.is_empty() && body.chars().all(|c| c == MARK) {
            continue;
        }
        out.push_str(&line.replace(MARK, ""));
    }
    out
}

/// Просьба о сводке: `summarization.md` с подставленным `summary_rules.md`.
pub fn summarization_prompt() -> String {
    personal(SUMMARIZATION_MD).replace("{summary_rules}", personal(SUMMARY_RULES_MD).trim_end())
}

/// Порог и модели сжатия для выбранной главной модели.
pub struct Plan<'r> {
    /// `compaction_trigger` весов главной модели.
    pub trigger: u64,
    pub flush: Resolved<'r>,
    pub compaction: Resolved<'r>,
}

impl<'r> Plan<'r> {
    /// Модели — через `Registry::resolve(MemoryFlush / Compaction)`: это
    /// модель обслуживания, а при `setup.compaction_memory_flush_use_main` —
    /// сама главная.
    pub fn from_registry(reg: &'r Registry, selection: Option<&str>) -> Result<Self, RegistryError> {
        let get = |p: Purpose| -> Result<Resolved<'r>, RegistryError> {
            Ok(reg.resolve(p, selection)?.expect("resolve отдаёт None только для Embeddings"))
        };
        Ok(Plan {
            trigger: get(Purpose::Main)?.weights.compaction_trigger,
            flush: get(Purpose::MemoryFlush)?,
            compaction: get(Purpose::Compaction)?,
        })
    }
}

/// Сжатие разговора.
pub struct Compactor<'a, F, C> {
    pub flush: &'a F,
    pub compaction: &'a C,
    /// Инструменты хода сброса памяти.
    pub tools: &'a Tools,
    pub trigger: u64,
    pub spill_dir: PathBuf,
    /// Сегодняшний журнал памяти: путь от дома и содержимое.
    pub today: Option<(String, String)>,
    /// Всё уже сброшено периодическим сбросом (`periodic_flush_covered`).
    pub flush_covered: bool,
}

impl<'a, F: Provider, C: Provider> Compactor<'a, F, C> {
    /// Сжать, если оценка длины дошла до порога. `Some` — сжато.
    pub async fn compact_if_needed(&self, conv: &mut Conversation) -> Result<Option<Compacted>, TurnError> {
        let tokens_before = conv.estimate_tokens();
        if tokens_before < self.trigger {
            return Ok(None);
        }
        let Some(cut) = last_owner_turn(&conv.history).filter(|&c| c > 0) else {
            // Весь разговор — один ход: старой части нет.
            return Ok(None);
        };

        // 1. Ход сброса памяти — на копии истории, в разговор не идёт.
        if !self.flush_covered {
            self.flush(conv).await?;
        }

        // 2. Сводка старой части.
        let mut messages = conv.history[..cut].to_vec();
        messages.push(Message { role: Role::User, content: vec![Part::Text { text: summarization_prompt() }] });
        let req = Request {
            system: personal(SYSTEM_MD),
            messages,
            // Описания нужны: в старой части есть вызовы инструментов.
            tools: conv.prefix.tools.clone(),
            settings: Settings { tool_choice: ToolChoice::None, ..Default::default() },
        };
        let summary = self.compaction.complete(&req).await.map_err(TurnError::Model)?.text();
        if summary.trim().is_empty() {
            // Строка muse, `strings-all.txt:188356`.
            return Err(TurnError::Compaction("compaction summary cannot be empty".into()));
        }

        // 3. Сводка + последний ход владельца целиком.
        let mut history = vec![Message {
            role: Role::User,
            content: vec![Part::Text { text: format!("{SUMMARY_HEADER}{summary}") }],
        }];
        history.extend_from_slice(&conv.history[cut..]);
        conv.replace_history(history);
        Ok(Some(Compacted { summary, first_kept: cut, tokens_before, tokens_after: conv.estimate_tokens() }))
    }
}

impl<'a, F: Provider, C: Provider> Compactor<'a, F, C> {
    /// Ход сброса памяти на копии истории; в разговор не идёт.
    pub async fn flush(&self, conv: &Conversation) -> Result<(), TurnError> {
        let mut scratch = conv.history.clone();
        if let Some((path, content)) = &self.today {
            scratch.push(Message { role: Role::Developer, content: vec![Part::Text { text: today_log_note(path, content) }] });
        }
        scratch.push(Message {
            role: Role::Developer,
            content: vec![Part::Text { text: personal(MEMORY_FLUSH_PROMPT_MD) }],
        });
        Turn::from_prefix(self.flush, self.tools, &conv.prefix, self.spill_dir.clone())
            .run(&mut scratch, &mut |_: &str| {})
            .await?;
        Ok(())
    }
}

/// Что сделало сжатие (строка `agent.agent_compactions` muse).
#[derive(Debug, Clone, PartialEq)]
pub struct Compacted {
    pub summary: String,
    /// Индекс в старой истории первого оставленного сообщения.
    pub first_kept: usize,
    pub tokens_before: u64,
    pub tokens_after: u64,
}

/// Начало последнего хода владельца: последнее сообщение `User` без
/// результатов инструментов. С него и до конца — ни один вызов не
/// отрывается от результата, мышление хода остаётся как есть.
fn last_owner_turn(history: &[Message]) -> Option<usize> {
    history.iter().rposition(|m| {
        m.role == Role::User && !m.content.iter().any(|p| matches!(p, Part::ToolResult { .. }))
    })
}
