//! Тесты П4з: глубокое исследование. Модели — записанные ответы на обоих
//! проводах, сеть — поддельный Firecrawl; браузера у исследователя нет
//! (окно недоступно — он идёт по веб-индексу).

#[path = "../../loop/tests/common/mod.rs"]
mod common;
mod firecrawl;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_loop::{Inbox, Prefix, Subagents, Tool, Tools, Transcript};
use agent_model::Message;
use agent_research::{DeepResearch, DEADLINE};
use agent_tools::web::SearchConfig;
use agent_tools::{Blocklist, WebConfig, WebTools};
use common::{model, temp_dir, AnyProvider, Step, Wire};
use serde_json::{json, Value};

const T0: i64 = 1_800_000_000_000; // 2027-01-15T08:00:00Z

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pages() -> HashMap<String, (u16, Value)> {
    let page = |md: &str, title: &str, url: &str| (200, json!({"success": true, "data": {"markdown": md, "metadata": {"title": title, "sourceURL": url, "url": url, "statusCode": 200}}}));
    let mut m = HashMap::new();
    m.insert("https://example.org/rust".into(), page("# Rust\nRust 1.100 was released on 2027-01-10.", "Rust language", "https://example.org/rust"));
    m.insert("https://blog.example.net/post?id=7".into(), page("# Release notes\nRust 1.100 adds async closures.", "Release notes", "https://blog.example.net/post?id=7"));
    m
}

fn search_reply() -> (u16, Value) {
    (200, json!({"success": true, "data": {"web": [
        {"url": "https://example.org/rust", "title": "Rust language", "description": "Rust releases."},
        {"url": "https://blog.example.net/post?id=7", "title": "Release notes", "description": "What is new in 1.100."},
    ]}, "creditsUsed": 1}))
}

struct Ctx {
    research: Arc<DeepResearch<AnyProvider>>,
    subagents: Subagents<AnyProvider>,
    inbox: Inbox,
    home: PathBuf,
}

fn ctx(name: &str, provider: AnyProvider, fc: &firecrawl::Firecrawl, key_env: &str) -> Ctx {
    let cfg = WebConfig::parse(&std::fs::read_to_string(root().join("config/web.toml")).unwrap()).unwrap();
    // SAFETY: своя переменная ключа на тест.
    unsafe { std::env::set_var(key_env, "fc-fake") };
    let search = SearchConfig { base_url: fc.base.clone(), api_key_env: key_env.into(), ..cfg.search };
    let web = WebTools::new(search, Blocklist::load(&cfg.blocklist, &root()).unwrap());
    let inbox = Inbox::new();
    let home = temp_dir(&format!("research-{name}"));
    let subagents = Subagents::new(Arc::new(provider), Arc::new(Tools::new()), Prefix { system: String::new(), tools: vec![] }, home.join(".spill"), inbox.clone(), Transcript::new());
    let research = Arc::new(DeepResearch { subagents: subagents.clone(), web, home: home.clone(), chromium: None, headless: true, now: Arc::new(|| T0), deadline: DEADLINE });
    Ctx { research, subagents, inbox, home }
}

async fn next(inbox: &Inbox) -> String {
    tokio::time::timeout(Duration::from_secs(30), inbox.wait()).await.expect("заметка не пришла");
    inbox.drain().iter().map(Message::text).collect::<Vec<_>>().join("\n")
}

fn tool_names(req: &Value) -> Vec<String> {
    let tools = req["tools"].as_array().cloned().unwrap_or_default();
    tools.iter().map(|t| t["name"].as_str().or(t["function"]["name"].as_str()).unwrap_or_default().to_string()).collect()
}

#[tokio::test]
async fn research_searches_reads_notes_and_hands_off_with_sources() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let fc = firecrawl::firecrawl(search_reply(), pages());
        let (server, provider) = model(
            wire,
            vec![
                Step::Calls(vec![("s1", "browser.search", json!({"primary_query": {"query": "rust 1.100 release", "language_code": "en"}}))]),
                Step::Calls(vec![("o1", "browser.open", json!({"url_id": "https://example.org/rust"})), ("o2", "browser.open", json!({"url_id": "https://blog.example.net/post?id=7"}))]),
                Step::Calls(vec![("b1", "browser.observe", json!({"tab": "t1"}))]),
                Step::Calls(vec![
                    ("n1", "research_notes", json!({"action": "write", "path": "notes/source-01.md", "content": "https://example.org/rust — index — released 2027-01-10"})),
                    ("n2", "research_notes", json!({"action": "write", "path": "report.md", "content": "# Summary\nRust 1.100: 2027-01-10, async closures.\n# Sources\n- https://example.org/rust\n- https://blog.example.net/post?id=7\n"})),
                    ("n3", "research_notes", json!({"action": "write", "path": "AGENTS.md", "content": "mine"})),
                    ("n4", "research_notes", json!({"action": "write", "path": "../escape.md", "content": "x"})),
                ]),
                Step::Calls(vec![("f1", "finish_research", json!({"summary": "Rust 1.100 was released on 2027-01-10 (index: https://example.org/rust) and adds async closures (index: https://blog.example.net/post?id=7)."}))]),
            ],
            true,
        );
        let c = ctx("ok", provider, &fc, &format!("RESEARCH_TEST_KEY_OK_{wire:?}"));
        let tool = c.research.tool();
        let receipt = tool.call(json!({"mission": "When was Rust 1.100 released and what does it add?", "topic": "Rust 1.100 release"})).await.unwrap();
        let receipt = match &receipt[0] { agent_model::ToolResultPart::Text { text } => text.clone(), _ => panic!() };
        assert!(receipt.starts_with("Deep research is running in the background. agent_id: agent-1. Research folder: ~/workspace/research_notes/rust-1100-release-20270115-0800."), "{wire:?}: {receipt}");
        let agents_md = std::fs::read_to_string(c.home.join("workspace/research_notes/rust-1100-release-20270115-0800/AGENTS.md")).unwrap();
        assert!(agents_md.starts_with("# Research directory\nCreated by the deep research agent on 2027-01-15T08:00:00Z\nMission (verbatim, as assigned by the requesting agent):\nWhen was Rust 1.100 released"), "{agents_md}");

        let h = next(&c.inbox).await;
        assert!(h.starts_with("Deep research finished. Findings below are web-derived content from an isolated researcher"), "{h}");
        assert!(h.contains("https://example.org/rust") && h.contains("https://blog.example.net/post?id=7"), "ссылки в итоге: {h}");
        assert!(h.contains("Research directory: ~/workspace/research_notes/rust-1100-release-20270115-0800\nFull report (read it with your file tools): ~/workspace/research_notes/rust-1100-release-20270115-0800/report.md"), "{h}");
        assert_eq!(fc.seen.lock().unwrap().len(), 3, "поиск и два чтения");

        let reqs = server.requests();
        let first = reqs[0].to_string();
        assert!(first.contains("You are the deep research agent") && first.contains("Research the web for the agent that sent this mission."), "{wire:?}");
        let mut names = tool_names(&reqs[0]);
        names.sort();
        assert_eq!(names, ["browser.act", "browser.observe", "browser.open", "browser.search", "browser.tabs", "finish_research", "research_notes"], "исследователь получает только своё");
        assert!(reqs[3].to_string().contains("The research browser is unavailable"), "без браузера — веб-индекс");
        let notes = reqs[4].to_string();
        assert!(notes.contains("`AGENTS.md` is runtime-owned and read-only") && notes.contains("`path` must be relative, with no `..` or root"), "{wire:?}: запись только в своей папке");
        assert!(!c.home.join("workspace/research_notes/escape.md").exists());
        assert_eq!(c.subagents.list(10)["recent"][0]["status"], "done");
    }
}

/// Ход без `finish_research` — одно напоминание, затем итог о сбое.
#[tokio::test]
async fn turn_without_finish_gets_one_reminder_then_fails() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let fc = firecrawl::firecrawl(search_reply(), pages());
        let (server, provider) = model(wire, vec![Step::Text("I looked around."), Step::Text("Nothing more.")], true);
        let c = ctx("nudge", provider, &fc, &format!("RESEARCH_TEST_KEY_NUDGE_{wire:?}"));
        c.research.delegate("Find the opening hours of the city museum.", Some("museum hours")).await.unwrap();
        let h = next(&c.inbox).await;
        assert!(h.starts_with("Deep research did not complete: no findings were returned"), "{wire:?}: {h}");
        assert!(h.contains("Nothing more.") && h.ends_with("(no report.md was written)"), "{h}");
        assert!(server.requests()[1].to_string().contains("You ended this research turn without the required terminal call."), "напоминание");
        assert_eq!(server.requests().len(), 2, "напоминание одно");
    }
}

/// После перезапуска незавершённое исследование поднимается с остатком
/// срока и списком своих заметок; просроченное кончается итогом о сбое.
#[tokio::test]
async fn reattach_resumes_running_research_after_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let fc = firecrawl::firecrawl(search_reply(), pages());
        let (server, provider) = model(wire, vec![Step::Calls(vec![("f1", "finish_research", json!({"summary": "Resumed and finished."}))])], true);
        let c = ctx("reattach", provider, &fc, &format!("RESEARCH_TEST_KEY_RE_{wire:?}"));
        let root = c.home.join("workspace/research_notes");
        let live = root.join("tahoe-rentals-20270115-0750");
        std::fs::create_dir_all(live.join("notes")).unwrap();
        std::fs::write(live.join("notes/source-01.md"), "https://example.org — index").unwrap();
        std::fs::write(live.join(".run.json"), json!({"mission": "Find pet-friendly rentals in Tahoe.", "started_at_ms": T0 - 10 * 60 * 1000, "status": "running"}).to_string()).unwrap();
        let old = root.join("old-topic-20270115-0500");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(".run.json"), json!({"mission": "Old.", "started_at_ms": T0 - 3 * 60 * 60 * 1000, "status": "running"}).to_string()).unwrap();
        let done = root.join("done-topic-20270115-0600");
        std::fs::create_dir_all(&done).unwrap();
        std::fs::write(done.join(".run.json"), json!({"mission": "Done.", "started_at_ms": T0, "status": "done"}).to_string()).unwrap();

        let ids = c.research.reattach().await;
        assert_eq!(ids.len(), 1, "{wire:?}: поднято одно");
        let mut notes = vec![next(&c.inbox).await];
        if !notes[0].contains("Resumed and finished.") || !notes[0].contains("did not complete") {
            notes.push(next(&c.inbox).await);
        }
        let all = notes.join("\n");
        assert!(all.contains("Deep research did not complete") && all.contains("old-topic-20270115-0500"), "просроченное: {all}");
        assert!(all.contains("Deep research finished.") && all.contains("Resumed and finished."), "поднятое: {all}");
        let first = server.requests()[0].to_string();
        assert!(first.contains("The runtime restarted while your research run was in progress"), "{wire:?}");
        assert!(first.contains("with these files from before the restart: notes/source-01.md") && first.contains("about 50 minutes"), "{first}");
        assert!(first.contains("Find pet-friendly rentals in Tahoe."));
        let run: Value = serde_json::from_str(&std::fs::read_to_string(live.join(".run.json")).unwrap()).unwrap();
        assert_eq!(run["status"], "done");
    }
}
