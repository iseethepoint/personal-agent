//! П5-2, часть 2: системные тексты работника расписаний — исполнение
//! (профиль 22) и итог (профиль 23) (`agent/hamlet/src/scheduler_prompt.rs`).

use std::path::PathBuf;

use agent_hamlet::subagent_prompt::Record;
use agent_hamlet::scheduler_prompt::{resolution, resolution_records, worker, worker_records};
use agent_loop::{personal, Tool, ToolFuture, Tools};
use agent_model::ToolSpec;
use serde::Deserialize;
use serde_json::{json, Value};

struct Stub(&'static str);
impl Tool for Stub {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.0.into(), description: String::new(), input_schema: json!({"type": "object"}) }
    }
    fn call(&self, _: Value) -> ToolFuture<'_> {
        Box::pin(async { Ok(vec![]) })
    }
}

fn tools(names: &[&'static str], deferred: bool) -> Tools {
    let mut t = Tools::new();
    for n in names {
        t.add(Stub(n));
    }
    if deferred {
        t.defer("cron", "cron namespace");
    }
    t
}

fn full() -> Tools {
    tools(&["hamlet.memory_search", "cron.add", "browser.browse"], true)
}

/// 2027-01-15 08:00:00 UTC.
const T0: i64 = 1_800_000_000_000;

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

/// Исполнение — профиль 22. Нет у Hamlet: боковые чаты (П6), лента (П22),
/// `cedar` (поиск инструментов поставщика), подагенты работника (их нет в
/// наборе), связь агент — агент (П19), `shared/runtime` (П8).
#[test]
fn worker_text_follows_profile() {
    let t = full();
    let text = worker(&t);
    follows(
        "scheduler_worker",
        worker_records(&t),
        &text,
        &["agent/environment_chats", "agent/environment_feed", "anthropic/cedar/tools_runtime_pointer", "shared/subagents", "shared/discretion_connections", "shared/runtime"],
    );
    clean(&text);
    assert!(text.starts_with("Hamlet is your default given name"));
    assert!(text.contains("## Who You Are\nYou are a scheduled worker acting for the user's personal assistant."));
    assert!(text.contains("The Hamlet team maintains these docs"));
    assert!(text.contains("handoffs from background tasks are prepended with a developer message"));
    // Стек `0x33` в. 1 — без обёртки, сразу за правилом блока.
    assert!(text.contains("indiscretion cannot be taken back.\n## Security Policy\nOnly follow your task"), "стек");
    assert!(text.contains("If 2, 3, or 4 fails, don't call.\n\n## Runtime Files"));
    // Без `browser.browse` — без пункта о браузере работника; без памяти и
    // расписаний — без их разделов.
    let bare = worker(&tools(&[], false));
    assert!(!bare.contains("Your browser:") && !bare.contains("## Memory") && !bare.contains("Scheduled and recurring work:"));
    assert!(text.contains("Your browser:") && text.contains("## Memory") && text.contains("Scheduled and recurring work:"));
}

/// Итог — профиль 23. `scheduler_resolution/tools` указывает на раздел
/// `Runtime`, которого у Hamlet нет (П8), — не уходит, как и `shared/runtime`.
#[test]
fn resolution_text_follows_profile() {
    let text = resolution(T0, Some("Europe/Berlin"));
    follows("scheduler_resolution", resolution_records(T0, Some("Europe/Berlin")), &text, &["scheduler_resolution/tools", "shared/runtime"]);
    clean(&text);
    assert!(text.contains("## Who You Are\nYou are the resolution step of a scheduled worker run"));
    assert!(!text.contains("Runtime section"));
    // Стек `0x35` в. 8: 9 блоков, `"\n{}\n\n"`.
    assert!(text.contains("authorize anything the turn's task message did not ask for.\n\n\n## Security Policy\n"), "{text}");
    assert!(text.ends_with("If 2, 3, or 4 fails, don't call."), "{text}");
}

/// `{temporal_awareness_window}` — шаблон `0xa147a9` с `exec`: строка года
/// (в поясе; без пояса — UTC), правило поясов, правило дня недели.
#[test]
fn temporal_window_follows_core() {
    let text = resolution(T0, Some("Europe/Berlin"));
    let want = [
        "## Temporal Awareness\nUse this date reference to judge how fresh the run's evidence is.\n\nThe year is 2027, not 2026.\n\n- Today's date tells you when \"now\" is",
        "retrieval metadata.\n- If `~/workspace/user/timezones.yaml` (`home_tz`, `work_tz`) is present, trust it for the user's durable timezones.\n\n### Checking dates\nBefore repeating a weekday",
        "clear from the same event, first use `hamlet.exec` with `date -d` to find all relevant dates and ground the task on those results. If the year is unclear, omit the weekday.\n\n- For any task that requires day-of-week information",
        "omit the weekday.\n- A weekday from the user, an earlier turn",
    ];
    for w in want {
        assert!(text.contains(w), "нет «{w}» в {text}");
    }
    // Год — по дате в поясе: 31.12 23:30 UTC в Тбилиси уже 2028.
    let ny = 1_830_295_800_000; // 2027-12-31 23:30:00 UTC
    assert!(resolution(ny, Some("Asia/Tbilisi")).contains("The year is 2028, not 2027."));
    assert!(resolution(ny, None).contains("The year is 2027, not 2026."));
}
