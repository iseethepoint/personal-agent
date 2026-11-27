//! Сжатие по частям — как у muse (`hatch-compaction/src/segmented.rs`,
//! флаг `hatch_segmented_compaction`). Сводка — блоки `<segment>` и блок
//! `<current_state>`; новое сжатие пишет только новую часть и состояние
//! заново (`segmented_summarization.md`, `segmented_state.md`), старые части
//! остаются как есть, самые старые удаляются, когда части не помещаются в
//! бюджет, — на их месте пометка «Older summaries were removed here…».
//!
//! Пределы — от окна модели `W` (ядро около `0xccdbe0e`, функция
//! подготовки `0xccdb910`; без окна — значения для W = 272 000):
//! - бюджет хранимых частей — W/10;
//! - `state_target_tokens` — W/25, жёсткий предел состояния — 2W/25;
//! - `segment_target_tokens` — min(W/50, 6000)/2, жёсткий предел части —
//!   min(W/50, 6000).
//!
//! Токены считаются как байты × 2/7 с округлением вверх (`* 2 / 7` в
//! ядре). Прошлое состояние длиннее цели — к указанию о состоянии
//! добавляется `segmented_state_over_cap.md` (`0xf1f67e0`). Ответ без
//! состояния или длиннее жёстких пределов (`segmented_block_limit`,
//! `0x974df76`) — замечание `segmented_output_cap_feedback.md`
//! (`compaction_segmented_reply_feedback`); ответ всё ещё длиннее — обрезка
//! («segmented compaction reply over its hard limits; cut to fit»,
//! `0xcdadad0`). Сводка, которая не разбирается, — одна часть («segmented
//! compaction checkpoint does not parse; keeping it as one segment»).

pub const SEGMENTED_SUMMARIZATION_MD: &str = include_str!("../../../prompts/compaction/segmented_summarization.md");
pub const SEGMENTED_STATE_MD: &str = include_str!("../../../prompts/compaction/segmented_state.md");
pub const SEGMENTED_STATE_OVER_CAP_MD: &str = include_str!("../../../prompts/compaction/segmented_state_over_cap.md");
pub const SEGMENTED_OUTPUT_CAP_FEEDBACK_MD: &str = include_str!("../../../prompts/compaction/segmented_output_cap_feedback.md");

/// Пометка на месте удалённых частей — строка ядра (`0xb3a3ca`).
pub const OLDER_REMOVED: &str = "Older summaries were removed here to save space. Recover what happened in this span from saved memories, including the daily notes under `~/memory/`, or from conversation history.";

/// Окно по умолчанию, из которого выходят значения ядра без окна
/// (27 200 / 10 880 / 2 720 / 21 760 / 5 440).
const DEFAULT_WINDOW: u64 = 272_000;

/// Пределы сжатия по частям.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub block: u64,
    pub state_target: u64,
    pub segment_target: u64,
    pub state_limit: u64,
    pub segment_limit: u64,
}

impl Limits {
    pub fn for_window(window: Option<u64>) -> Limits {
        let w = window.filter(|&w| w > 0).unwrap_or(DEFAULT_WINDOW);
        let segment_limit = (w / 50).min(6000);
        Limits { block: w / 10, state_target: w / 10 * 2 / 5, segment_target: segment_limit / 2, state_limit: w / 10 * 4 / 5, segment_limit }
    }
}

/// Токены по правилу ядра: байты × 2/7, вверх.
pub fn tokens(text: &str) -> u64 {
    (text.len() as u64 * 2).div_ceil(7)
}

/// Сводка: части по порядку и текущее состояние.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    pub segments: Vec<String>,
    pub state: Option<String>,
}

impl Summary {
    /// Разобрать текст сводки; не разбирается — одна часть.
    pub fn parse(text: &str) -> Summary {
        let mut rest = text.trim();
        let mut s = Summary::default();
        while let Some(body) = rest.strip_prefix("<segment>\n") {
            let Some(end) = body.find("\n</segment>") else { return Summary::one(text) };
            s.segments.push(body[..end].to_string());
            rest = body[end + "\n</segment>".len()..].trim_start();
        }
        if let Some(body) = rest.strip_prefix("<current_state>\n") {
            let Some(end) = body.find("\n</current_state>") else { return Summary::one(text) };
            s.state = Some(body[..end].to_string());
            rest = body[end + "\n</current_state>".len()..].trim_start();
        }
        if !rest.is_empty() || (s.segments.is_empty() && s.state.is_none()) {
            if !text.trim().is_empty() {
                eprintln!("segmented compaction checkpoint does not parse; keeping it as one segment");
            }
            return Summary::one(text);
        }
        s
    }

    fn one(text: &str) -> Summary {
        let t = text.trim();
        Summary { segments: if t.is_empty() { vec![] } else { vec![t.to_string()] }, state: None }
    }

    fn render_segments(&self) -> String {
        self.segments.iter().map(|t| format!("<segment>\n{t}\n</segment>\n\n")).collect()
    }

    /// Текст сводки.
    pub fn render(&self) -> String {
        let mut out = self.render_segments();
        if let Some(st) = &self.state {
            out.push_str(&format!("<current_state>\n{st}\n</current_state>"));
        }
        out.trim_end().to_string()
    }

    /// Удалять самые старые части, пока части не помещаются в `budget`;
    /// новейшая остаётся; на месте удалённых — пометка. Вернуть число
    /// удалённых.
    pub fn trim(&mut self, budget: u64) -> usize {
        let mut removed = 0;
        loop {
            let marker = self.segments.first().is_some_and(|s| s == OLDER_REMOVED) as usize;
            if self.segments.len() <= marker + 1 || tokens(&self.render_segments()) <= budget {
                return removed;
            }
            self.segments.remove(marker);
            if marker == 0 {
                self.segments.insert(0, OLDER_REMOVED.to_string());
            }
            removed += 1;
        }
    }
}

/// Ответ модели: новая часть и состояние (разделы `## New segment` /
/// `## Current state`, или блоки `<new_segment>` / `<current_state>`).
pub fn parse_reply(text: &str) -> (Option<String>, Option<String>) {
    let tag = |open: &str, close: &str| -> Option<String> {
        let start = text.find(open)? + open.len();
        let end = text[start..].find(close)? + start;
        Some(text[start..end].trim().to_string())
    };
    if let (Some(seg), state) = (tag("<new_segment>", "</new_segment>"), tag("<current_state>", "</current_state>")) {
        return (Some(seg), state);
    }
    let seg_at = text.find("## New segment");
    let state_at = text.find("## Current state");
    let segment = seg_at.map(|i| {
        let body = &text[i + "## New segment".len()..];
        let end = state_at.filter(|&j| j > i).map_or(body.len(), |j| j - i - "## New segment".len());
        body[..end].trim().to_string()
    });
    let state = state_at.map(|i| text[i + "## Current state".len()..].trim().to_string());
    (segment, state)
}

/// Обрезать текст до `limit` токенов (по правилу ядра), по границе символа.
pub fn cut_to(text: &str, limit: u64) -> String {
    if tokens(text) <= limit {
        return text.to_string();
    }
    let mut end = (limit * 7 / 2) as usize;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Ответ превышает жёсткие пределы.
pub fn over_limits(segment: &str, state: &str, l: &Limits) -> bool {
    tokens(segment) > l.segment_limit || tokens(state) > l.state_limit
}
