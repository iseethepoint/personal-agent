//! `{temporal_awareness_window}` — `0xcdc2380` → `0xcdbf080`, шаблон
//! `0xa147a9` (`prompts/shared/temporal_awareness_window.md`; разбор —
//! `prompts/SOURCES.md`, П5-2 части 2–3).
//!
//! - строка года: «The year is {y}, not {y-1}.» (`0x2e6831`) + `"\n\n"`
//!   (`0x414d26`); дата — в поясе, без пояса — `UTC` (`0x26a61690` →
//!   `0x26a616f2`);
//! - правило поясов — `shared/temporal_guidance_detached` без краёв;
//! - с `exec` (`0xcdc2479`): соседних дат нет, `0xa14246` и `0xa14448`;
//! - без `exec` и без соседних дат в метке времени (поле `+0x384` = 0):
//!   соседние даты (`0x26a60870`) — «Nearby dates (in the user's timezone): »
//!   и 7 дней от −3 до +3 через « | » (`0x26a64a30`: «{день} {%Y-%m-%d}»,
//!   день — `Mon`…`Sun` из `0x298df0c8`, пометки `(yesterday)` `0x32d45a`,
//!   `(today)` `0x3f7d82`, `(tomorrow)` `0x3b06b2`) + `"\n\n"`; `0xa143ab`
//!   и `0xa1470a`.

use agent_loop::personal;

macro_rules! p {
    ($path:literal) => {
        include_str!(concat!("../../../prompts/", $path))
    };
}

fn fill(text: &str, vars: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (name, value) in vars {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// Соседние даты: 7 дней от −3 до +3 в поясе.
fn nearby_dates(now: chrono::DateTime<chrono_tz::Tz>) -> String {
    let items: Vec<String> = (-3i64..=3)
        .map(|k| {
            let day = now + chrono::Duration::days(k);
            let base = day.format("%a %Y-%m-%d").to_string();
            match k {
                -1 => format!("{base} (yesterday)"),
                0 => format!("{base} (today)"),
                1 => format!("{base} (tomorrow)"),
                _ => base,
            }
        })
        .collect();
    format!("{}\n\n", p!("shared/temporal_nearby_dates.md").replace("{dates}", &items.join(" | ")))
}

/// Окно времени на момент `now_ms`; `exec` — есть ли `exec` в наборе роли.
pub fn window(now_ms: i64, timezone: Option<&str>, exec: bool) -> String {
    use chrono::{Datelike, TimeZone};
    let tz: chrono_tz::Tz = timezone.and_then(|t| t.parse().ok()).unwrap_or(chrono_tz::UTC);
    let now = tz.timestamp_millis_opt(now_ms).single();
    let anchor = now.map(|t| format!("The year is {}, not {}.\n\n", t.year(), t.year() - 1)).unwrap_or_default();
    let guidance = personal(p!("shared/temporal_guidance_detached.md"));
    let (nearby, clause, rule) = if exec {
        (String::new(), p!("shared/temporal_weekday_clause_exec.md"), p!("shared/temporal_weekday_rule_exec.md"))
    } else {
        (now.map(nearby_dates).unwrap_or_default(), p!("shared/temporal_weekday_clause_listed.md"), p!("shared/temporal_weekday_rule_listed.md"))
    };
    fill(
        p!("shared/temporal_awareness_window.md"),
        &[("year_anchor", &anchor), ("temporal_guidance", guidance.trim()), ("nearby_dates", &nearby), ("weekday_clause", clause), ("weekday_rule", rule)],
    )
}
