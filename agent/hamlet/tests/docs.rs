//! П5-3: справочник о себе (`agents/hamlet/home/docs/`). Каждая ссылка
//! `~/docs/…` из собранных текстов и из самих документов ведёт на файл,
//! который есть; список `~/docs/` главного текста — ровно перенесённые файлы.

use agent_hamlet::{background_prompt, main_prompt, scheduler_prompt, subagent_prompt};
use agent_loop::{personal, Tool, ToolFuture, Tools};
use agent_model::ToolSpec;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

struct Stub(&'static str);
impl Tool for Stub {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.0.into(), description: String::new(), input_schema: json!({"type": "object"}) }
    }
    fn call(&self, _: Value) -> ToolFuture<'_> {
        Box::pin(async { Ok(vec![]) })
    }
}

/// Все имена, от которых зависят ветки сборок.
const ALL: [&str; 12] = [
    "todo.write",
    "hamlet.memory_search",
    "hamlet.memory_get",
    "chat.read_messages",
    "browser_deep_research",
    "subagent.spawn",
    "cron.add",
    "hooks.add",
    "exec.run",
    "browser.browse",
    "browser.search",
    "files.read",
];

fn tools(names: &[&'static str]) -> Tools {
    let mut t = Tools::new();
    for n in names {
        t.add(Stub(n));
    }
    t.defer("cron", "cron namespace");
    t
}

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../agents/hamlet/home/docs")
}

/// Собранные тексты всех ролей — с полным набором и без инструментов.
fn assembled() -> Vec<(String, String)> {
    let mut out = vec![];
    for (tag, names) in [("full", &ALL[..]), ("empty", &[][..])] {
        let t = tools(names);
        out.push((format!("main/{tag}"), main_prompt::assemble(&t)));
        out.push((format!("turn/{tag}"), main_prompt::turn_texts(&t, main_prompt::MAIN_ROLE).join("\n\n")));
        for (inherits, spawn) in [(true, true), (false, false)] {
            out.push((format!("subagent/{tag}/{inherits}"), subagent_prompt::assemble(&t, inherits, spawn)));
        }
        out.push((format!("worker/{tag}"), scheduler_prompt::worker(&t)));
    }
    out.push(("resolution".into(), scheduler_prompt::resolution(0, None)));
    for exec in [true, false] {
        out.push((format!("event_hook/{exec}"), background_prompt::event_hook(0, None, exec)));
        out.push((format!("verification/{exec}"), background_prompt::verification(0, None, exec)));
        out.push((format!("memory_flush/{exec}"), background_prompt::memory_flush(0, None, exec)));
    }
    out.push(("verification_turn".into(), background_prompt::verification_turn_texts().join("\n\n")));
    out.push(("browser".into(), agent_browser::planner::system()));
    out
}

/// Ссылки `~/docs/<путь>` в тексте; точка в конце предложения не входит.
fn links(text: &str) -> Vec<String> {
    let mut out = vec![];
    for (i, _) in text.match_indices("~/docs/") {
        let rest = &text[i + "~/docs/".len()..];
        let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || "_-./".contains(c))).unwrap_or(rest.len());
        out.push(rest[..end].trim_end_matches('.').to_string());
    }
    out
}

fn assert_exists(from: &str, link: &str) {
    let p = docs_dir().join(link);
    assert!(p.exists(), "{from}: ~/docs/{link} — такого файла нет");
}

/// Файлы справочника (пути от `docs/`).
fn shipped() -> BTreeSet<String> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.insert(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&docs_dir(), &docs_dir(), &mut out);
    out
}

#[test]
fn links_from_assembled_texts_exist() {
    let mut seen = 0;
    for (name, text) in assembled() {
        for link in links(&text) {
            assert_exists(&name, &link);
            seen += 1;
        }
    }
    assert!(seen > 0, "ссылок не нашлось — проверка пустая");
}

#[test]
fn links_inside_docs_exist() {
    for f in shipped() {
        let text = std::fs::read_to_string(docs_dir().join(&f)).unwrap();
        for link in links(&text) {
            assert_exists(&f, &link);
        }
    }
}

/// Список `~/docs/` главного текста (`user_chat/docs.md`) — ровно
/// перенесённые файлы.
#[test]
fn docs_list_is_shipped_files() {
    let text = main_prompt::assemble(&tools(&ALL));
    let start = text.find("- `~/docs/`\n").expect("списка нет");
    let listed: BTreeSet<String> = text[start..]
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("  `"))
        .map(|l| l.trim_start().trim_start_matches('`').split('`').next().unwrap().to_string())
        .collect();
    assert_eq!(listed, shipped());
}

/// Справочник — личный вариант, без меток, без muse и Meta (П10).
#[test]
fn docs_are_personal_and_clean() {
    for f in shipped() {
        let text = std::fs::read_to_string(docs_dir().join(&f)).unwrap();
        assert_eq!(personal(&text), text, "{f}: метки");
        assert!(!text.to_lowercase().contains("muse"), "{f}: muse");
        assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == "Meta"), "{f}: Meta");
    }
}
