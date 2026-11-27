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
            assert!(["dispatcher", "other_harness", "unresolved"].contains(&s.via.as_str()));
        }
    }
    let unresolved: Vec<&str> = secs.values().filter(|s| s.via == "unresolved").map(|s| s.id.as_str()).collect();
    let mut u = unresolved.clone();
    u.sort_by_key(|x| u32::from_str_radix(x, 16).unwrap());
    assert_eq!(u, ["64", "75", "164", "264", "364", "464", "564", "664", "764", "864", "964"]);
}

/// Порядок разделов ролей, которые у Hamlet есть, но чья сборка ещё не по
/// профилю (части 2–3 П5-2), — массивы профилей ядра (адрес среза — в
/// комментарии). Подагент — по собранному тексту (`tests/subagent_prompt.rs`),
/// работник расписаний и итог — `tests/scheduler_prompt.rs`,
/// глубокое исследование — `deep_research_blocks_follow_profile`.
#[test]
fn hamlet_roles_follow_core_order() {
    let (ps, _) = load();
    let cases: [(&str, &[&str]); 5] = [
        // 0x290bbf50: 0x9905ee
        ("deep_research", &["66", "67", "68", "12e", "69", "6a", "635", "6b", "6c", "13b"]),
        // 0x290bc030: 0x99065a, 0x4b0fa0, 0x9905ba
        ("event_hook", &["7f", "7d", "3d", "7e", "7c", "335", "33b", "cd", "ce", "cc", "b7", "c9", "ca", "b8", "b9", "ba", "bb", "bc", "bd", "bf", "c2", "c1", "92"]),
        // 0x290bc180: 0x4a3198, 0x9906ca, 0x9905ba
        ("execute_resolve", &["126", "3d", "435", "33b", "426", "b7", "b8", "b9", "ba", "bb", "bc", "bd", "bf", "c1"]),
        // 0x290bc228: 0x49a180, 0x9906bc
        ("verification", &["326", "3d", "435", "33b", "426", "b8", "ba", "bc", "bd", "bf", "c1"]),
        // 0x290bc490: 0x49a850, 0x99059e, 0x9905ba
        ("memory_flush", &["9c", "3d", "735", "73b", "cc", "b7", "b8", "b9", "ba", "bb", "bc", "bd", "bf", "c2", "c1", "92", "8c", "91"]),
    ];
    for (role, want) in cases {
        let got: Vec<&str> = order(by_role(&ps, role)).into_iter().filter(|id| !id.ends_with("d0")).collect();
        assert_eq!(got, want, "{role}");
    }
}

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
        assert_eq!(secs[id].via, "other_harness", "{id}");
    }
}
