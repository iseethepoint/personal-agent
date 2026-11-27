//! П5-2, часть 3: системные тексты фоновых ролей — работник наблюдателя
//! (профиль 10), проверяющий (19), сброс памяти (30)
//! (`agent/hamlet/src/background_prompt.rs`, `temporal.rs`).

use std::path::PathBuf;

use agent_hamlet::background_prompt::{event_hook, event_hook_records, memory_flush, memory_flush_records, verification, verification_records};
use agent_hamlet::subagent_prompt::Record;
use agent_loop::personal;
use serde::Deserialize;

/// 2027-01-15 08:00:00 UTC — пятница.
const T0: i64 = 1_800_000_000_000;

const ABSENT: [&str; 2] = ["shared/discretion_connections", "shared/runtime"];

#[derive(Deserialize)]
struct Profiles {
    profile: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    role: String,
    #[serde(default)]
    slice: Vec<Slice>,
}
#[derive(Deserialize)]
struct Slice {
    sections: Vec<String>,
}
#[derive(Deserialize)]
struct Sections {
    section: Vec<Section>,
}
#[derive(Deserialize)]
struct Section {
    id: String,
    #[serde(default)]
    block: Vec<Block>,
}
#[derive(Deserialize)]
struct Block {
    block: Option<String>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Блоки системного текста подагента в ядре: срез 0 профиля 0 (разделы
/// по порядку), у каждого раздела — его блоки (`prompts/profiles/`);
/// пустое имя — вставка ядра.
fn core_blocks(role: &str) -> Vec<(String, String)> {
    let p: Profiles = toml::from_str(&std::fs::read_to_string(root().join("prompts/profiles/profiles.toml")).unwrap()).unwrap();
    let s: Sections = toml::from_str(&std::fs::read_to_string(root().join("prompts/profiles/sections.toml")).unwrap()).unwrap();
    let prof = p.profile.iter().find(|x| x.role == role).unwrap();
    let mut out = Vec::new();
    for id in &prof.slice[0].sections {
        let sec = s.section.iter().find(|x| &x.id == id).unwrap();
        for b in &sec.block {
            let name = b.block.clone().unwrap_or_default();
            out.push((id.clone(), name.trim_start_matches("blocks/").trim_end_matches(".md").to_string()));
        }
    }
    out
}


/// Записи идут в порядке блоков профиля (без `absent`), текст каждой — в
/// собранном тексте после предыдущей.
fn follows(role: &str, records: Vec<Record>, text: &str, absent: &[&str]) {
    let ours: Vec<(String, String)> = records.iter().map(|r| (r.section.to_string(), r.block.to_string())).collect();
    let expect: Vec<(String, String)> = core_blocks(role).into_iter().filter(|(_, b)| !absent.contains(&b.as_str())).collect();
    assert_eq!(ours, expect, "{role}");
    let mut at = 0;
    for r in records {
        let piece = personal(&r.text);
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let head = piece.lines().next().unwrap();
        let i = text[at..].find(head).unwrap_or_else(|| panic!("{role}: {} не на месте", r.block));
        at += i + head.len();
    }
}

fn clean(text: &str) {
    for (i, _) in text.match_indices('{') {
        let rest = &text[i + 1..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_lowercase() || *c == '_').collect();
        assert!(name.is_empty() || !rest[name.len()..].starts_with('}'), "{{{name}}}");
    }
    assert!(!text.contains("<!--"));
    assert!(!text.to_lowercase().contains("muse") && !text.contains("Meta "));
}


/// Работник наблюдателя — профиль 10; стек `0x35` в. 3.
#[test]
fn event_hook_follows_profile() {
    let text = event_hook(T0, Some("UTC"), true);
    follows("event_hook", event_hook_records(T0, Some("UTC"), true), &text, &ABSENT);
    clean(&text);
    assert!(text.starts_with("Hamlet is your default given name"));
    assert!(text.contains("## Who You Are\nYou are an event hook worker"));
    assert!(text.contains("## Temporal Awareness\nThe year is 2027, not 2026.\n\n- Today's date"));
    assert!(text.contains("first use `hamlet.exec` with `date -d`"));
    assert!(text.contains("indiscretion cannot be taken back.\n\n## Security Policy\n"), "{text}");
}

/// Проверяющий — профиль 19; без `exec` — соседние даты (7 дней) и правило
/// «listed above».
#[test]
fn verification_follows_profile() {
    let text = verification(T0, Some("Europe/Berlin"), false);
    follows("verification", verification_records(T0, Some("Europe/Berlin"), false), &text, &ABSENT);
    clean(&text);
    assert!(text.starts_with("## Your Task\n\nYou independently verify"));
    assert!(text.contains(
        "### Checking dates\nNearby dates (in the user's timezone): Tue 2027-01-12 | Wed 2027-01-13 | Thu 2027-01-14 (yesterday) | Fri 2027-01-15 (today) | Sat 2027-01-16 (tomorrow) | Sun 2027-01-17 | Mon 2027-01-18\n\nBefore repeating a weekday"
    ), "{text}");
    assert!(text.contains("same event, use the nearby dates listed above when they cover the date."));
    assert!(text.contains("- Do not calculate a weekday from memory. Use the nearby dates listed above or a verified `date` command result already in context."));
    assert!(!text.contains("hamlet.exec"));
    // Даты — в поясе: в Тбилиси 15.01 20:30 UTC — уже 16-е.
    let late = verification(T0 + 12 * 3_600_000 + 1_800_000, Some("Asia/Tbilisi"), false);
    assert!(late.contains("Sat 2027-01-16 (today)"), "{late}");
}

/// Сброс памяти — профиль 30; стек `0x35` в. 7 (9 записей).
#[test]
fn memory_flush_follows_profile() {
    let text = memory_flush(T0, None, true);
    follows("memory_flush", memory_flush_records(T0, None, true), &text, &ABSENT);
    clean(&text);
    assert!(text.contains("running a memory flush before older context is condensed"));
    assert!(text.contains("The year is 2027, not 2026."));
    assert!(text.ends_with("If 2, 3, or 4 fails, don't call."), "{text}");
}
