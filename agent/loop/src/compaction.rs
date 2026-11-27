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
    StopReason, ToolChoice,
};

use crate::prompt::Conversation;
use crate::tool::Tools;
use crate::turn::{Turn, TurnError};

/// Тексты muse, перенесены как есть (`prompts/compaction/`).
pub const SYSTEM_MD: &str = include_str!("../../../prompts/compaction/system.md");
pub const SUMMARIZATION_MD: &str = include_str!("../../../prompts/compaction/summarization.md");
pub const SUMMARY_RULES_MD: &str = include_str!("../../../prompts/compaction/summary_rules.md");
pub const MEMORY_FLUSH_PROMPT_MD: &str = include_str!("../../../prompts/compaction/memory_flush_prompt.md");
pub const OUTPUT_CAP_FEEDBACK_MD: &str = include_str!("../../../prompts/compaction/output_cap_feedback.md");
/// Заметка muse в запросе сводки: инструментов нет (`compact/no_tools.md`).
pub const NO_TOOLS_MD: &str = include_str!("../../../prompts/compaction/no_tools.md");

/// Старая часть разговора для сводки: заметка `no_tools.md` и просьба.
fn summary_input(old: &[Message], prompt: String) -> Vec<Message> {
    let mut messages = old.to_vec();
    messages.push(Message { role: Role::Developer, content: vec![Part::Text { text: personal(NO_TOOLS_MD).trim_end().to_string() }] });
    messages.push(Message { role: Role::User, content: vec![Part::Text { text: prompt }] });
    messages
}

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

/// Просьба о сжатии по частям: `segmented_summarization.md` с указанием о
/// состоянии (и `segmented_state_over_cap.md`, если прошлое состояние длиннее
/// цели), целевыми длинами и `summary_rules.md`.
pub fn segmented_prompt(limits: &crate::segmented::Limits, previous_state: Option<&str>) -> String {
    use crate::segmented::*;
    let mut state = personal(SEGMENTED_STATE_MD).trim_end().to_string();
    if previous_state.is_some_and(|s| tokens(s) > limits.state_target) {
        state.push(' ');
        state.push_str(personal(SEGMENTED_STATE_OVER_CAP_MD).trim_end());
    }
    personal(SEGMENTED_SUMMARIZATION_MD)
        .replace("{state_instruction}", &state)
        .replace("{segment_target_tokens}", &limits.segment_target.to_string())
        .replace("{state_target_tokens}", &limits.state_target.to_string())
        .replace("{summary_rules}", personal(SUMMARY_RULES_MD).trim_end())
}

/// Порог и модели сжатия для выбранной главной модели.
pub struct Plan<'r> {
    /// `compaction_trigger` весов главной модели.
    pub trigger: u64,
    /// Окно главной модели — от него пределы сжатия по частям.
    pub window: u64,
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
            window: get(Purpose::Main)?.weights.context_window,
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
    /// Сжатие по частям (`hatch_segmented_compaction`) и окно модели для
    /// его пределов.
    pub segmented: bool,
    pub window: Option<u64>,
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
        let summary = if self.segmented { self.segmented_summary(conv, cut).await? } else { self.plain_summary(conv, cut).await? };
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
    fn request(&self, conv: &Conversation, messages: Vec<Message>) -> Request {
        Request {
            system: personal(SYSTEM_MD),
            messages,
            // Описания нужны: в старой части есть вызовы инструментов.
            tools: conv.prefix.tools.clone(),
            settings: Settings { tool_choice: ToolChoice::None, ..Default::default() },
        }
    }

    /// Сводка одним куском; упор в предел ответа — одно замечание
    /// `output_cap_feedback.md` (`compaction_output_token_cap_feedback`).
    async fn plain_summary(&self, conv: &Conversation, cut: usize) -> Result<String, TurnError> {
        let mut messages = summary_input(&conv.history[..cut], summarization_prompt());
        let resp = self.compaction.complete(&self.request(conv, messages.clone())).await.map_err(TurnError::Model)?;
        if resp.stop_reason != StopReason::MaxTokens {
            return Ok(resp.text());
        }
        messages.push(Message { role: Role::Assistant, content: vec![Part::Text { text: resp.text() }] });
        messages.push(Message { role: Role::User, content: vec![Part::Text { text: personal(OUTPUT_CAP_FEEDBACK_MD).trim_end().to_string() }] });
        Ok(self.compaction.complete(&self.request(conv, messages)).await.map_err(TurnError::Model)?.text())
    }

    /// Сводка по частям (`segmented.rs`): прошлая сводка остаётся в начале
    /// истории, модель пишет новую часть и состояние; ответ без состояния,
    /// длиннее жёстких пределов или упёршийся в предел ответа — одно
    /// замечание `segmented_output_cap_feedback.md`, потом обрезка; старые
    /// части сверх бюджета удаляются.
    async fn segmented_summary(&self, conv: &Conversation, cut: usize) -> Result<String, TurnError> {
        use crate::segmented::*;
        let limits = Limits::for_window(self.window);
        let old = conv.history.first().map(|m| m.text()).and_then(|t| t.strip_prefix(SUMMARY_HEADER).map(Summary::parse)).unwrap_or_default();
        let mut messages = summary_input(&conv.history[..cut], segmented_prompt(&limits, old.state.as_deref()));
        let resp = self.compaction.complete(&self.request(conv, messages.clone())).await.map_err(TurnError::Model)?;
        let (mut segment, mut state) = parse_reply(&resp.text());
        let bad = |seg: &Option<String>, st: &Option<String>| seg.is_none() || st.is_none() || over_limits(seg.as_deref().unwrap_or(""), st.as_deref().unwrap_or(""), &limits);
        if resp.stop_reason == StopReason::MaxTokens || bad(&segment, &state) {
            eprintln!("compaction_segmented_reply_feedback");
            messages.push(Message { role: Role::Assistant, content: vec![Part::Text { text: resp.text() }] });
            let feedback = personal(SEGMENTED_OUTPUT_CAP_FEEDBACK_MD)
                .replace("{segment_target_tokens}", &limits.segment_target.to_string())
                .replace("{state_target_tokens}", &limits.state_target.to_string());
            messages.push(Message { role: Role::User, content: vec![Part::Text { text: feedback.trim_end().to_string() }] });
            let again = self.compaction.complete(&self.request(conv, messages)).await.map_err(TurnError::Model)?;
            (segment, state) = parse_reply(&again.text());
        }
        let Some(segment) = segment.filter(|s| !s.is_empty()) else { return Ok(String::new()) };
        // Состояния нет — остаётся прошлое (`segmented_state_missing`).
        let state = state.or(old.state.clone());
        let (seg_cut, state_cut) = (cut_to(&segment, limits.segment_limit), state.as_deref().map(|s| cut_to(s, limits.state_limit)));
        if seg_cut != segment || state_cut.as_deref() != state.as_deref() {
            eprintln!("segmented compaction reply over its hard limits; cut to fit");
        }
        let mut next = Summary { segments: old.segments, state: state_cut };
        next.segments.push(seg_cut);
        next.trim(limits.block);
        Ok(next.render())
    }

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
        // Системный текст — главного агента, инструменты — набор хода сброса
        // (роль `memory_flush` у Hamlet).
        Turn::new(self.flush, self.tools, conv.prefix.system.clone(), self.spill_dir.clone())
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
/// результатов инструментов вместе с заметками `Developer` прямо перед ним
/// (метка времени входа стоит перед сообщением, `Conversation::push_user`).
/// С него и до конца — ни один вызов не отрывается от результата, мышление
/// хода остаётся как есть.
fn last_owner_turn(history: &[Message]) -> Option<usize> {
    let mut i = history.iter().rposition(|m| {
        m.role == Role::User && !m.content.iter().any(|p| matches!(p, Part::ToolResult { .. }))
    })?;
    while i > 0 && history[i - 1].role == Role::Developer {
        i -= 1;
    }
    Some(i)
}
