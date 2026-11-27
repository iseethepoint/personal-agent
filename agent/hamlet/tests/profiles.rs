//! П5-2, часть 1: профили подсказок ядра и карта разделов
//! (`prompts/profiles/`, разбор — `prompts/SOURCES.md`, П5-2).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Deserialize)]
struct Profiles {
    profile: Vec<Profile>,
}

#[derive(Deserialize)]
struct Profile {
    number: toml::Value,
    at: String,
    role: String,
    hamlet: String,
    #[serde(default)]
    slice: Vec<Slice>,
}

#[derive(Deserialize)]
struct Slice {
    at: String,
    sections: Vec<String>,
}

#[derive(Deserialize)]
struct Sections {
    section: Vec<Section>,
}

#[derive(Deserialize)]
struct Section {
    id: String,
    via: String,
    #[serde(default)]
    block: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    block: Option<String>,
}

fn load() -> (Vec<Profile>, HashMap<String, Section>) {
    let p: Profiles = toml::from_str(&std::fs::read_to_string(root().join("prompts/profiles/profiles.toml")).unwrap()).unwrap();
    let s: Sections = toml::from_str(&std::fs::read_to_string(root().join("prompts/profiles/sections.toml")).unwrap()).unwrap();
    (p.profile, s.section.into_iter().map(|x| (x.id.clone(), x)).collect())
}

fn order(p: &Profile) -> Vec<&str> {
    p.slice.iter().flat_map(|s| s.sections.iter().map(String::as_str)).collect()
}

fn by_role<'a>(ps: &'a [Profile], role: &str) -> &'a Profile {
    ps.iter().find(|p| p.role == role).unwrap_or_else(|| panic!("нет профиля {role}"))
}

/// Вся таблица 0x290bbdd0–0x290bcd18: главный и 69 пронумерованных профилей;
/// у каждого — роль (или «unidentified»), место у Hamlet или строка плана;
/// каждый номер раздела — в карте.
#[test]
fn every_profile_and_section_is_mapped() {
    let (ps, secs) = load();
    assert_eq!(ps.len(), 70);
    for (k, p) in ps.iter().skip(1).enumerate() {
        assert_eq!(p.number.as_integer(), Some(k as i64), "{}", p.at);
    }
    for p in &ps {
        assert!(!p.hamlet.is_empty(), "{}: где у Hamlet", p.at);
        for s in &p.slice {
            assert!(s.at.starts_with("0x"));
        }
        for id in order(p) {
            let s = secs.get(id).unwrap_or_else(|| panic!("{}: раздел {id} не в карте", p.at));
            assert!(["dispatcher", "flag", "browser_table", "unresolved"].contains(&s.via.as_str()));
        }
    }
    let unresolved: Vec<&str> = secs.values().filter(|s| s.via == "unresolved").map(|s| s.id.as_str()).collect();
    let mut u = unresolved.clone();
    u.sort_by_key(|x| u32::from_str_radix(x, 16).unwrap());
    assert_eq!(u, ["75"]);
}

// Порядок ролей Hamlet проверяется по собранным текстам: подагент —
// `tests/subagent_prompt.rs`, работник расписаний и итог —
// `tests/scheduler_prompt.rs`, наблюдатель, проверяющий, сброс памяти —
// `tests/background_prompt.rs`, исполнитель браузера и глубокое
// исследование — ниже.

/// Блоки глубокого исследования у Hamlet — в порядке профиля 6 ядра (блоки
/// `deep_research/*` его разделов); `security_authority` — из стека
/// безопасности, его место — за сборщиком, пока в конце.
#[test]
fn deep_research_blocks_follow_profile() {
    let (ps, secs) = load();
    let core: Vec<String> = order(by_role(&ps, "deep_research"))
        .into_iter()
        .flat_map(|id| secs[id].block.iter().filter_map(|b| b.block.clone()))
        .filter(|n| n.starts_with("blocks/deep_research/"))
        .map(|n| n.trim_start_matches("blocks/").to_string())
        .collect();
    let ours: Vec<&str> = agent_research::BLOCKS.iter().map(|(n, _)| *n).collect();
    assert_eq!(core, ours[..ours.len() - 1]);
    assert_eq!(ours.last(), Some(&"deep_research/security_authority.md"));
}

/// Карта раздела — список блоков ядра по порядку: проверка на известных.
#[test]
fn section_map_matches_dispatcher() {
    let (_, secs) = load();
    let names = |id: &str| -> Vec<String> { secs[id].block.iter().map(|b| b.block.clone().unwrap_or_default()).collect() };
    assert_eq!(names("43"), ["blocks/shared/who_built_you.md", "blocks/agent/who_you_are.md"]);
    assert_eq!(
        names("2f"),
        ["blocks/chat/filesystem.md", "blocks/chat/filesystem_memory.md", "", "blocks/chat/filesystem_workspace.md", ""]
    );
    assert_eq!(names("13b"), ["blocks/shared/runtime.md"]);
    for id in ["b7", "b8", "c1", "c2"] {
        assert_eq!(secs[id].via, "flag", "{id}");
    }
}

/// Исполнитель браузера — профиль 1 (`0x290bbe38`, срез 0 `0x4a628c`:
/// `64 23b`): раздел `0x64` вариант 0 — `browser/system.md`, затем
/// `browser/pages.md` (таблица `0x2906bd30`); `23b` — П8. Порядок — по
/// собранному тексту исполнителя.
#[test]
fn browser_planner_follows_profile() {
    let (ps, secs) = load();
    let p = by_role(&ps, "browser_planner");
    assert_eq!(p.slice[0].sections, ["64", "23b"]);
    let blocks: Vec<String> = secs["64"].block.iter().filter_map(|b| b.block.clone()).collect();
    assert_eq!(blocks, ["browser/system.md", "browser/pages.md"]);
    let text = agent_browser::planner::system();
    let first = |path: &str| -> String {
        let t = agent_loop::personal(&std::fs::read_to_string(root().join("prompts").join(path)).unwrap());
        t.trim().lines().next().unwrap().to_string()
    };
    let (sys, pages) = (first("browser/system.md"), first("browser/pages.md"));
    assert!(text.starts_with(&sys), "{text}");
    assert!(text.find(&sys).unwrap() < text.find(&pages).unwrap());
}
