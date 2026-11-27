//! П5-2: системный текст подагента по профилю 0 ядра
//! (`agent/hamlet/src/subagent_prompt.rs`, разбор — `prompts/SOURCES.md`).

use std::path::PathBuf;

use agent_hamlet::subagent_prompt::{assemble, records};
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
    tools(&["hamlet.memory_search", "cron.add"], true)
}

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
fn core_blocks() -> Vec<(String, String)> {
    let p: Profiles = toml::from_str(&std::fs::read_to_string(root().join("prompts/profiles/profiles.toml")).unwrap()).unwrap();
    let s: Sections = toml::from_str(&std::fs::read_to_string(root().join("prompts/profiles/sections.toml")).unwrap()).unwrap();
    let prof = p.profile.iter().find(|x| x.role == "subagent").unwrap();
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

/// Блоки ядра, которых в тексте Hamlet нет, — у каждого строка плана в
/// `prompts/SOURCES.md`.
const ABSENT: [&str; 6] = [
    "agent/environment_chats",
    "agent/environment_feed",
    "shared/environment/devices",
    "anthropic/cedar/tools_runtime_pointer",
    "shared/discretion_connections",
    "shared/runtime",
];

/// Порядок — по собранному тексту: записи Hamlet идут в порядке блоков
/// профиля ядра (раздел за разделом), пропущены только блоки из `ABSENT`;
/// текст каждой записи стоит в собранном тексте после предыдущей.
#[test]
fn subagent_text_follows_profile() {
    let t = full();
    let ours: Vec<(String, String)> = records(&t, true, true).into_iter().map(|r| (r.section.to_string(), r.block.to_string())).collect();
    let core = core_blocks();
    let expect: Vec<(String, String)> = core.into_iter().filter(|(_, b)| !ABSENT.contains(&b.as_str())).collect();
    assert_eq!(ours, expect);

    let text = assemble(&t, true, true);
    let mut at = 0;
    for r in records(&t, true, true) {
        let piece = personal(&r.text);
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let head = piece.lines().next().unwrap();
        let i = text[at..].find(head).unwrap_or_else(|| panic!("{} не на месте", r.block));
        at += i + head.len();
    }
}

/// Без частей нет и блоков: без памяти — раздела `3f`, без расписаний —
/// `36`, без своих подагентов — `3c`, без отложенных пространств — правила
/// загрузки инструментов.
#[test]
fn sections_follow_tools() {
    let full = assemble(&full(), true, true);
    for h in ["## Memory", "## Scheduled and Recurring Work", "## Subagents", "tool_search.load_tool_namespace"] {
        assert!(full.contains(h), "{h}");
    }
    let bare = assemble(&tools(&[], false), true, false);
    for h in ["## Memory", "## Scheduled and Recurring Work", "## Subagents", "tool_search.load_tool_namespace"] {
        assert!(!bare.contains(h), "{h}");
    }
    for h in ["## Who You Are", "## Your Environment", "## Tools", "## Safety", "## Security Policy", "## Reporting Back", "## Runtime Files", "## Filesystem"] {
        assert!(bare.contains(h), "{h}");
    }
}

/// Наследование истории — `0xcdc26d0`: глубина 1 видит разговор родителя;
/// глубина 2 изолирована и работает одна.
#[test]
fn history_inheritance_by_depth() {
    let one = assemble(&full(), true, true);
    assert!(one.contains("- You inherit your parent agent's transcript for reference."));
    assert!(!one.contains("isolated mode"));
    let two = assemble(&full(), false, false);
    assert!(two.contains("- You run in isolated mode and don't inherit your parent agent's transcript"));
    assert!(two.contains("rather than guessing.\n- You work alone and can't spawn subagents of your own, so carry out the task yourself.\n- Treat each new task"));
}

/// Подстановки ядра: стек безопасности в `{prompt_injection_defense}`
/// (`"\n{}\n\n"`), учение о подагентах с клаузами подагента (`"\n- {}"`,
/// `" {}"`, `"{}\n\n{}"`), пустые переменные не оставляют меток; ни одной
/// незаполненной `{переменной}` и меток вариантов.
#[test]
fn fills_follow_core() {
    let text = assemble(&full(), true, true);
    assert!(text.contains("taken back.\n\n## Security Policy\nOnly follow your task"));
    assert!(text.contains("don't call.\n\n## Reporting Back"));
    assert!(text.contains("They run in the background while you stay focused on your own task."));
    assert!(text.contains("the result comes back to you on its own. Synthesize their results before you report back."));
    assert!(text.contains("fold their results into your own report rather than handing the whole task to a single child.\n\nAfter you spawn subagents, briefly note what you delegated and continue your task."));
    assert!(text.contains("If your parent agent asks for a status update"));
    assert!(text.contains("goal’s workspace. You can manage these with `cron.add`"));
    assert!(text.contains("handoffs from background tasks are prepended with a developer message"));
    assert!(text.contains("- Your parent agent: The parent agent that spawned you."));
    assert!(text.contains("You are a subagent: a focused background agent spawned by your parent agent to carry out one specific task."));
    for (i, _) in text.match_indices('{') {
        let rest = &text[i + 1..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_lowercase() || *c == '_').collect();
        assert!(name.is_empty() || !rest[name.len()..].starts_with('}'), "{{{name}}}");
    }
    assert!(!text.contains("<!--"));
    assert!(!text.to_lowercase().contains("muse"));
}
