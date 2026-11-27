//! Тест П10: в том, что уходит модели, нет имён muse и Meta. Проверяется
//! всё сразу:
//! - описания всех инструментов реестра (вместе с параметрами);
//! - все тексты `prompts/` в личном варианте;
//! - все строки в коде `agent/*/src` (без комментариев): из них собираются
//!   заметки, ошибки и ответы инструментов.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_loop::{first_message, personal, summarization_prompt, Inbox, Prefix, Subagents, Tools, Transcript};
use agent_memory::{ForgetTools, MemoryTools, Store};
use agent_tools::web::SearchConfig;
use agent_tools::{add_search, Blocklist, Catalog, ExecTools, FileTools, Home, Todo, WebConfig, WebTools};
use common::{model, temp_dir, Wire};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Запретное имя в тексте: `muse` в любом регистре или слово `Meta`.
fn forbidden(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if let Some(i) = lower.find("muse") {
        return Some(snippet(text, i));
    }
    let b = text.as_bytes();
    let mut from = 0;
    while let Some(j) = text[from..].find("Meta") {
        let i = from + j;
        let before = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        let after = b.get(i + 4).is_none_or(|c| !c.is_ascii_alphanumeric());
        if before && after {
            return Some(snippet(text, i));
        }
        from = i + 4;
    }
    None
}

fn snippet(text: &str, i: usize) -> String {
    let start = text.floor_char_boundary(i.saturating_sub(60));
    let end = text.ceil_char_boundary((i + 60).min(text.len()));
    text[start..end].replace('\n', "\\n")
}

fn assert_clean(place: &str, text: &str, bad: &mut Vec<String>) {
    if let Some(s) = forbidden(text) {
        bad.push(format!("{place}: …{s}…"));
    }
}

/// Весь реестр инструментов агента.
async fn all_tools() -> Tools {
    let home_dir = temp_dir("names-home");
    let home = Home::new(&home_dir);
    let mut tools = Tools::new();
    FileTools::new(home.clone()).add_to(&mut tools);
    ExecTools::new(home.clone(), vec![], Some(Inbox::new())).add_to(&mut tools);
    tools.add(Todo::new());
    let skills = Catalog::skills(temp_dir("names-skills"), &home);
    let connectors = Catalog::connectors(temp_dir("names-connectors"), &home);
    add_search(&mut tools, &skills, &connectors);
    let cfg = WebConfig::parse(&std::fs::read_to_string(root().join("config/web.toml")).unwrap()).unwrap();
    let search = SearchConfig { base_url: "http://127.0.0.1:9/v2".into(), ..cfg.search };
    WebTools::new(search, Blocklist::load(&cfg.blocklist, &root()).unwrap()).add_to(&mut tools);
    let store = Store::connect(&pg::database()).await.unwrap();
    MemoryTools::new(&home_dir, store.clone(), None).add_to(&mut tools);
    let (_server, provider) = model(Wire::OpenaiCompat, vec![], false);
    let subagents = Subagents::new(
        Arc::new(provider),
        Arc::new(Tools::new()),
        Prefix { system: String::new(), tools: vec![] },
        temp_dir("names-spill"),
        Inbox::new(),
        Transcript::new(),
    );
    ForgetTools::new(subagents.clone(), &home_dir, store, None, home_dir.join("skills/forget/SKILL.md")).add_to(&mut tools);
    for t in subagents.tools() {
        tools.add(t);
    }
    let cron = agent_cron::Core::new(agent_cron::Store::connect(&pg::database()).await.unwrap(), Arc::new(agent_cron::SystemClock), &home_dir, None);
    for t in agent_cron::tools(&cron) {
        tools.add(t);
    }
    for t in agent_cron::hook_tools(&cron) {
        tools.add(t);
    }
    let goals_store = agent_goals::Store::connect(&pg::database(), Arc::new(|| 0)).await.unwrap();
    let goals: Arc<agent_goals::Goals<common::AnyProvider>> = Arc::new(agent_goals::Goals { store: goals_store, home: home_dir.clone(), models: None, cleanup: None });
    for t in agent_goals::tools(&goals) {
        tools.add(t);
    }
    tools
}

#[tokio::test]
async fn tool_descriptions_have_no_muse_or_meta() {
    let tools = all_tools().await;
    let mut specs = tools.specs();
    specs.extend(agent_cron::resolution_specs());
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    for want in ["hamlet.read", "hamlet.write", "hamlet.edit", "hamlet.exec", "hamlet.skill_search", "hamlet.connector_search", "hamlet.memory_search", "subagent.spawn", "browser.search", "todo.write", "forget.plan", "forget.confirm", "cron.add", "cron.stop", "hooks.add", "hooks.dry_run", "hamlet.notify_main_agent", "hamlet.nothing_to_report", "hamlet.nothing_to_do", "user_goal.create", "user_goal.delete_briefing", "tracking.create", "tracking.set_status"] {
        assert!(names.contains(&want), "нет {want} в {names:?}");
    }
    let mut bad = Vec::new();
    for s in &specs {
        assert_clean(&s.name, &s.name, &mut bad);
        assert_clean(&s.name, &s.description, &mut bad);
        assert_clean(&s.name, &s.input_schema.to_string(), &mut bad);
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn prompts_have_no_muse_or_meta() {
    let mut bad = Vec::new();
    let mut seen = 0;
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                walk(&e.path(), out);
            } else {
                out.push(e.path());
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("prompts"), &mut files);
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        // SOURCES.md — рабочий документ: откуда взят каждый файл; модель его не видит.
        if name == "SOURCES.md" {
            continue;
        }
        seen += 1;
        assert_clean(&f.display().to_string(), &personal(&std::fs::read_to_string(&f).unwrap()), &mut bad);
    }
    assert!(seen >= 8, "тексты prompts/ не найдены");
    assert_clean("summarization_prompt", &summarization_prompt(), &mut bad);
    assert_clean("first_message", &first_message("задача"), &mut bad);
    assert_clean("worker_message", &agent_cron::worker_message("job", "task", Some("t"), "тело", None, None), &mut bad);
    assert_clean("heartbeat worker_message", &agent_cron::worker_message("heartbeat", "heartbeat", None, "тело", Some(agent_cron::HEARTBEAT_TEMPLATE), None), &mut bad);
    assert_clean("hook worker system", &agent_cron::hooks::worker_system(""), &mut bad);
    assert_clean("hook result", &agent_cron::hooks::result_text("m"), &mut bad);
    assert_clean("handoff", &format!("{:?}", agent_cron::handoff(&serde_json::json!({"job_id": "job", "message": "m"}))), &mut bad);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Строковые литералы файла, без комментариев.
fn literals(src: &str) -> Vec<(usize, String)> {
    let b = src.as_bytes();
    let (mut i, mut out) = (0, Vec::new());
    let line = |i: usize| src[..i].matches('\n').count() + 1;
    while i < b.len() {
        if src[i..].starts_with("//") {
            i = src[i..].find('\n').map_or(b.len(), |j| i + j);
        } else if src[i..].starts_with("/*") {
            i = src[i + 2..].find("*/").map_or(b.len(), |j| i + 2 + j + 2);
        } else if b[i] == b'r' && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) && {
            let h = src[i + 1..].bytes().take_while(|c| *c == b'#').count();
            b.get(i + 1 + h) == Some(&b'"')
        } {
            let h = src[i + 1..].bytes().take_while(|c| *c == b'#').count();
            let close = format!("\"{}", "#".repeat(h));
            let body = i + 2 + h;
            let end = src[body..].find(&close).map_or(b.len(), |j| body + j);
            out.push((line(i), src[body..end].to_string()));
            i = end + close.len();
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            out.push((line(i), src[i + 1..j.min(b.len())].to_string()));
            i = j + 1;
        } else if b[i] == b'\'' && src[i..].len() >= 3 && (b[i + 2] == b'\'' || (b[i + 1] == b'\\' && b.get(i + 3) == Some(&b'\''))) {
            i += if b[i + 1] == b'\\' { 4 } else { 3 };
        } else {
            i += 1;
        }
    }
    out
}

#[test]
fn code_strings_have_no_muse_or_meta() {
    let mut bad = Vec::new();
    let mut files = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    for c in std::fs::read_dir(root().join("agent")).unwrap().flatten() {
        if c.path().join("src").is_dir() {
            walk(&c.path().join("src"), &mut files);
        }
    }
    assert!(files.len() > 20, "исходники не найдены");
    for f in &files {
        for (n, lit) in literals(&std::fs::read_to_string(f).unwrap()) {
            assert_clean(&format!("{}:{n}", f.strip_prefix(root()).unwrap_or(f).display()), &lit, &mut bad);
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn forbidden_names_are_caught() {
    assert!(forbidden("use `muse.read` first").is_some());
    assert!(forbidden("Muse cannot erase").is_some());
    assert!(forbidden("Meta AI chats").is_some());
    assert!(forbidden("metadata and Metadata are fine").is_none());
    assert!(literals("let a = \"x\"; // \"muse\"\nlet b = r#\"y \"z\"\"#;").iter().map(|(_, s)| s.as_str()).eq(["x", "y \"z\""]));
}
