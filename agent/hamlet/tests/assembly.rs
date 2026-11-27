//! Тесты П4м: собранный агент. Модели — поддельные серверы на обоих
//! проводах (записанные ответы), Postgres — временный кластер, часы —
//! поддельные; живых вызовов нет.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;
#[path = "../../research/tests/firecrawl/mod.rs"]
mod firecrawl;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_cron::FakeClock;
use agent_hamlet::daemon::Services;
use agent_hamlet::{AgentDir, Hamlet, Options};
use chrono::TimeZone;
use common::{model, temp_dir, Server, Step, Wire};
use serde_json::json;
use tokio::sync::mpsc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Запретное имя (П10, как `agent/tools/tests/names.rs`): `muse` в любом
/// регистре или отдельное слово `Meta`.
fn forbidden(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if let Some(i) = lower.find("muse") {
        return Some(text[text.floor_char_boundary(i.saturating_sub(40))..text.ceil_char_boundary((i + 40).min(text.len()))].to_string());
    }
    let b = text.as_bytes();
    let mut from = 0;
    while let Some(j) = text[from..].find("Meta") {
        let i = from + j;
        let before = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        let after = b.get(i + 4).is_none_or(|c| !c.is_ascii_alphanumeric());
        if before && after {
            return Some(text[text.floor_char_boundary(i.saturating_sub(40))..text.ceil_char_boundary((i + 40).min(text.len()))].to_string());
        }
        from = i + 4;
    }
    None
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn t0() -> i64 {
    chrono::Utc.with_ymd_and_hms(2027, 1, 15, 8, 0, 0).unwrap().timestamp_millis()
}

/// Временная папка агента: реестр на поддельный сервер, база — свой адрес
/// в своей переменной.
fn agent_dir(name: &str, wire: Wire, server: &Server) -> PathBuf {
    agent_dir_with(name, wire, server, "http://127.0.0.1:9/v2")
}

fn agent_dir_with(name: &str, wire: Wire, server: &Server, search_base: &str) -> PathBuf {
    let dir = temp_dir(&format!("hamlet-{name}"));
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let db_var = format!("HAMLET_TEST_DB_{}_{n}", std::process::id());
    let key_var = format!("HAMLET_TEST_KEY_{}_{n}", std::process::id());
    // SAFETY: тест сам задаёт свои переменные с уникальными именами.
    unsafe {
        std::env::set_var(&db_var, pg::database());
        std::env::set_var(&key_var, "k");
    }
    let models = match wire {
        Wire::Anthropic => format!(
            "[defaults]\nmodel = \"fake/main\"\nmaintenance = \"fake/main\"\n\n[weights.w]\ncontext_window = 1000000\ncompaction_trigger = 900000\nanthropic.forced_tool_choice = true\nanthropic.runtime_output_ceiling = 64000\n\n[wires]\n\"claude-opus-5-5\" = {{ weights = \"w\" }}\n\n[[models]]\nid = \"fake/main\"\nprovider = \"anthropic\"\nmodel = \"claude-opus-5-5\"\nbase_url = \"{}\"\napi_key_env = \"{key_var}\"\n",
            server.base_url
        ),
        Wire::OpenaiCompat => format!(
            "[defaults]\nmodel = \"fake/main\"\nmaintenance = \"fake/main\"\n\n[weights.w]\ncontext_window = 1000000\ncompaction_trigger = 900000\nopenai_compat.reasoning = true\nopenai_compat.vision = true\n\n[wires]\n\"qwen3:8b\" = {{ weights = \"w\", local_inference = true }}\n\n[[models]]\nid = \"fake/main\"\nprovider = \"openai_compat\"\nmodel = \"qwen3:8b\"\nbase_url = \"{}/v1\"\n",
            server.base_url
        ),
    };
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(dir.join("config/models.toml"), models).unwrap();
    let bl = root().join("config/blocklist");
    std::fs::write(
        dir.join("config/web.toml"),
        format!(
            "[search]\nprovider = \"firecrawl\"\nbase_url = \"{search_base}\"\napi_key_env = \"{key_var}\"\n\n[blocklist]\ndomains = \"{}\"\nregex = \"{}\"\n",
            bl.join("mase_all_domains.fst").display(),
            bl.join("mase_blocklist_regex.json").display()
        ),
    )
    .unwrap();
    std::fs::write(dir.join("agent.toml"), format!("[agent]\nlocale = \"en-US\"\n\n[database]\nurl_env = \"{db_var}\"\n")).unwrap();
    std::fs::write(dir.join("system.md"), "You are Hamlet, a test agent.").unwrap();
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::write(dir.join("home/MEMORY.md"), "# Memory\n\nThe owner drinks coffee without sugar.\n").unwrap();
    dir
}

async fn build(dir: &Path, clock: FakeClock) -> Arc<Hamlet> {
    let opts = Options { clock: Arc::new(clock), tick: Duration::from_millis(50) };
    Arc::new(Hamlet::build(AgentDir::load(dir).unwrap(), &opts).await.unwrap())
}

/// Полный список инструментов главного агента — красный, если что-то не
/// подключено.
const MAIN_TOOLS: [&str; 90] = [
    "browser.browse", "browser.find", "browser.lookup_citation_url", "browser.open", "browser.search", "browser.windows", "browser_deep_research",
    "cron.add", "cron.list", "cron.remove", "cron.run", "cron.runs", "cron.status", "cron.stop", "cron.update", "cron.view",
    "forget.confirm", "forget.plan",
    "hamlet.connector_search", "hamlet.db", "hamlet.edit", "hamlet.exec", "hamlet.memory_explain", "hamlet.memory_get", "hamlet.memory_search", "hamlet.read", "hamlet.skill_search", "hamlet.write",
    "hooks.add", "hooks.disable", "hooks.dry_run", "hooks.enable", "hooks.list", "hooks.logs", "hooks.remove", "hooks.run", "hooks.update", "hooks.view",
    "process.clear", "process.kill", "process.list", "process.log", "process.poll", "process.send_keys", "process.write",
    "subagent.close", "subagent.list", "subagent.resume", "subagent.send", "subagent.spawn",
    "todo.write",
    "tool_search.load_tool_namespace",
    "tracking.create", "tracking.create_entry", "tracking.delete", "tracking.delete_entry", "tracking.get", "tracking.list", "tracking.search", "tracking.set_status", "tracking.update", "tracking.update_entry",
    "trash.list", "trash.put", "trash.restore",
    "user_goal.create", "user_goal.create_entry", "user_goal.delete", "user_goal.delete_briefing", "user_goal.delete_entry", "user_goal.get", "user_goal.list", "user_goal.search", "user_goal.set_status", "user_goal.update", "user_goal.update_entry",
    "workflow.create", "workflow.delete", "workflow.launch", "workflow.launch_async", "workflow.list", "workflow.list_runs", "workflow.pause",
    "workflow.resume", "workflow.save_run", "workflow.stop", "workflow.stop_agent", "workflow.update", "workflow.view", "workflow.view_run",
];

#[tokio::test]
async fn main_agent_registry_has_every_tool() {
    let (server, _p) = model(Wire::OpenaiCompat, vec![], false);
    let dir = agent_dir("tools", Wire::OpenaiCompat, &server);
    let h = build(&dir, FakeClock::at(t0())).await;
    let names = h.tool_names();
    assert_eq!(names, MAIN_TOOLS, "реестр главного агента");
    // П10: в собранном реестре и начале подсказки нет имён muse и Meta.
    for s in h.tools.specs() {
        for text in [&s.name, &s.description, &s.input_schema.to_string()] {
            assert!(forbidden(text).is_none(), "{}: …{}…", s.name, forbidden(text).unwrap());
        }
    }
    assert!(forbidden(&h.prefix.system).is_none(), "начало подсказки");
    // Отложенные пространства: их функции не уходят модели сразу, в начале
    // подсказки — список пространств с маркером; полное описание — при загрузке.
    let upfront: Vec<&str> = h.prefix.tools.iter().map(|s| s.name.as_str()).collect();
    for ns in ["cron.", "hooks.", "user_goal.", "tracking.", "workflow."] {
        assert!(!upfront.iter().any(|n| n.starts_with(ns)), "{ns} отложено");
        let line = format!("- `{}` [deferred: use tool_search to discover functions]: ", ns.trim_end_matches('.'));
        assert!(h.prefix.system.contains(&line), "{line}");
    }
    assert!(upfront.contains(&"tool_search.load_tool_namespace") && upfront.contains(&"hamlet.exec") && upfront.contains(&"subagent.spawn"));
    let cron_add = h.tools.specs().into_iter().find(|s| s.name == "cron.add").unwrap();
    assert!(!cron_add.description.contains("## "), "описание пространства не прикреплено к cron.add");
    let loaded = h.tools.load(&json!({"paths": ["cron"]}), &h.tools.loaded(&[]));
    assert!(loaded.starts_with("Loaded 9 function(s). Full schemas:\n\n## cron\n"), "{}", &loaded[..200]);
    // Описание браузера и начало подсказки — от папки агента.
    assert!(h.prefix.system.starts_with("You are Hamlet, a test agent."));
    assert!(h.prefix.system.contains("<!-- INJECTED FILE: MEMORY.md -->") && h.prefix.system.contains("coffee without sugar"));
    h.browsers.shutdown().await;
}

/// Сквозной разговор: запись файла, поиск в памяти, заведение расписания;
/// расписание срабатывает по поддельным часам, весть в ящике даёт новый
/// ход.
#[tokio::test]
async fn conversation_writes_searches_schedules_and_wakes_on_handoff() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let job = json!({"id": "coffee", "enabled": true, "mode": "task", "title": "Coffee", "body": "Remind the owner about coffee.",
            "schedule": {"kind": "runonce", "at": "2027-01-15T08:05", "timezone": "UTC", "timing": "exact"}});
        let (server, _p) = model(
            wire,
            vec![
                Step::Calls(vec![("w1", "hamlet.write", json!({"path": "workspace/notes.md", "content": "Buy coffee beans.\n"}))]),
                Step::Calls(vec![("m1", "hamlet.memory_search", json!({"queries": ["coffee"]}))]),
                // Пространство `cron` отложено — сначала загрузка.
                Step::Calls(vec![("l1", "tool_search.load_tool_namespace", json!({"paths": ["cron"]}))]),
                Step::Calls(vec![("c1", "cron.add", job)]),
                Step::Text("Done."),
                // Работник расписания: исполнение и итог.
                Step::Text("Reminder: coffee time."),
                Step::Calls(vec![("n1", "hamlet.notify_main_agent", json!({"outcome": "succeeded", "message": "Time for coffee."}))]),
                // Новый ход главного агента по вести.
                Step::Text("Reminder: it is time for coffee."),
            ],
            true,
        );
        let dir = agent_dir("e2e", wire, &server);
        let clock = FakeClock::at(t0());
        let h = build(&dir, clock.clone()).await;
        let services = Services::start(&h, Duration::from_millis(50));
        // Первый такт сверяет память с файлами.
        tokio::time::sleep(Duration::from_millis(400)).await;

        let (tx, rx) = mpsc::channel(4);
        let (otx, mut orx) = mpsc::unbounded_channel();
        let runner = {
            let h = h.clone();
            tokio::spawn(async move { h.run(rx, otx).await })
        };
        tx.send("Note to buy coffee beans, check what you remember about coffee, and remind me at 08:05.".into()).await.unwrap();
        let first = tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap();
        assert_eq!(first, "Done.", "{wire:?}");
        assert_eq!(std::fs::read_to_string(dir.join("home/workspace/notes.md")).unwrap(), "Buy coffee beans.\n");
        let reqs = server.requests();
        assert!(reqs[0].to_string().contains("You are Hamlet, a test agent.") && reqs[0].to_string().contains("Current date and time: 2027-01-15T08:00:00Z"), "{wire:?}: начало и изменчивое");
        assert!(reqs[2].to_string().contains("coffee without sugar"), "{wire:?}: поиск в памяти нашёл MEMORY.md");
        assert!(reqs[3].to_string().contains("Loaded 9 function(s). Full schemas:"), "{wire:?}: пространство cron загружено");
        assert!(reqs[4].to_string().contains("next_run_in"), "{wire:?}: расписание заведено");

        // Срок наступил — такт служб запускает работника, весть даёт новый ход.
        clock.advance_secs(6 * 60);
        let second = tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap();
        assert_eq!(second, "Reminder: it is time for coffee.", "{wire:?}");
        let reqs = server.requests();
        let last = reqs.last().unwrap().to_string();
        assert!(last.contains("[SCHEDULED TASK HANDOFF]") && last.contains("Time for coffee."), "{wire:?}: ход по вести");
        assert!(last.contains("Note to buy coffee beans"), "{wire:?}: тот же разговор");

        drop(tx);
        runner.await.unwrap();
        services.stop(&h).await;
    }
}

/// Программа `hamlet --agent <папка>`: строка в stdin — ход, ответ — в
/// stdout; закрытый вход завершает программу вместе со службами.
#[tokio::test]
async fn hamlet_binary_answers_stdin_line() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (server, _p) = model(wire, vec![Step::Text("Hello from Hamlet.")], true);
        let dir = agent_dir("bin", wire, &server);
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_hamlet"))
            .args(["--agent", dir.to_str().unwrap()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(b"hello\n").unwrap();
        }
        let out = tokio::task::spawn_blocking(move || child.wait_with_output().unwrap()).await.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{wire:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(stdout.trim(), "Hello from Hamlet.", "{wire:?}");
        assert!(server.requests()[0].to_string().contains("hello"));
    }
}

/// Пример папки агента в репозитории разбирается, пути ведут к файлам, ключей
/// в ней нет.
#[test]
fn example_agent_dir_parses() {
    let d = AgentDir::load(&root().join("agents/hamlet")).unwrap();
    for p in [&d.toml.paths.models, &d.toml.paths.web, &d.toml.paths.system] {
        assert!(d.path(p).exists(), "{}", d.path(p).display());
    }
    agent_model::Registry::load(&d.path(&d.toml.paths.models)).unwrap();
    assert_eq!(d.toml.database.url_env, "HAMLET_DATABASE_URL");
    let text = std::fs::read_to_string(root().join("agents/hamlet/agent.toml")).unwrap();
    assert!(!text.contains("sk-") && !text.contains("postgres://"), "в папке только имена переменных");
}

/// Глубокое исследование в собранном агенте: исследователь ищет в
/// поддельном Firecrawl, итог со ссылкой приходит в ящик и даёт новый ход
/// главного агента.
#[tokio::test]
async fn research_handoff_wakes_main_agent() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let fc = firecrawl::firecrawl(
            (200, json!({"success": true, "data": {"web": [{"url": "https://example.org/rust", "title": "Rust", "description": "Rust 1.100 released 2027-01-10."}]}})),
            std::collections::HashMap::new(),
        );
        let (server, _p) = model(
            wire,
            vec![
                Step::Calls(vec![("s1", "browser.search", json!({"primary_query": {"query": "rust 1.100", "language_code": "en"}}))]),
                Step::Calls(vec![("f1", "finish_research", json!({"summary": "Rust 1.100 was released on 2027-01-10 (index: https://example.org/rust)."}))]),
                Step::Text("Research is in: Rust 1.100 came out on 2027-01-10."),
            ],
            true,
        );
        let dir = agent_dir_with("research", wire, &server, &fc.base);
        let h = build(&dir, FakeClock::at(t0())).await;
        let (tx, rx) = mpsc::channel(4);
        let (otx, mut orx) = mpsc::unbounded_channel();
        let runner = {
            let h = h.clone();
            tokio::spawn(async move { h.run(rx, otx).await })
        };
        let tool = h.tools.get("browser_deep_research").expect("инструмент в реестре");
        tool.call(json!({"mission": "When was Rust 1.100 released?", "topic": "rust release"})).await.unwrap();
        let answer = tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap();
        assert_eq!(answer, "Research is in: Rust 1.100 came out on 2027-01-10.", "{wire:?}");
        let last = server.requests().last().unwrap().to_string();
        assert!(last.contains("Deep research finished.") && last.contains("https://example.org/rust") && last.contains("research_notes/rust-release-20270115-0800"), "{wire:?}: ход по итогу исследования");
        drop(tx);
        runner.await.unwrap();
        h.browsers.shutdown().await;
    }
}

/// Продолжение по списку дел: ход кончился с незакрытыми пунктами — среда
/// сама даёт новый ход с запиской; пустой список — продолжения нет.
#[tokio::test]
async fn unfinished_todo_continues_the_turn() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (server, _p) = model(
            wire,
            vec![
                Step::Calls(vec![("t1", "todo.write", json!({"todos": [{"text": "Draft the plan", "status": "in_progress"}, {"text": "Send the plan", "status": "pending"}]}))]),
                Step::Text("Started."),
                Step::Calls(vec![("t2", "todo.write", json!({"todos": []}))]),
                Step::Text("Plan drafted and sent."),
            ],
            true,
        );
        let dir = agent_dir("todo", wire, &server);
        let h = build(&dir, FakeClock::at(t0())).await;
        let (tx, rx) = mpsc::channel(4);
        let (otx, mut orx) = mpsc::unbounded_channel();
        let runner = {
            let h = h.clone();
            tokio::spawn(async move { h.run(rx, otx).await })
        };
        tx.send("Plan the week and send it to me.".into()).await.unwrap();
        let first = tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap();
        assert_eq!(first, "Started.", "{wire:?}");
        let second = tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap();
        assert_eq!(second, "Plan drafted and sent.", "{wire:?}");
        let reqs = server.requests();
        assert_eq!(reqs.len(), 4, "{wire:?}");
        let handoff = reqs[2].to_string();
        assert!(handoff.contains("This runtime handoff follows an assistant turn with work still `pending` or `in_progress`."), "{wire:?}");
        assert!(handoff.contains("Send the plan") && !handoff.contains("has not changed since the last reminder"), "{wire:?}");
        // Список пуст — нового хода нет.
        assert!(tokio::time::timeout(Duration::from_millis(300), orx.recv()).await.is_err(), "{wire:?}");
        assert!(h.todo_continuation().is_none());
        drop(tx);
        runner.await.unwrap();
        h.browsers.shutdown().await;
    }
}

/// Уборщик корзины — на такте службы: через 30 дней по часам службы запись
/// истекает.
#[tokio::test]
async fn trash_reaper_runs_on_service_tick() {
    let (server, _p) = model(Wire::OpenaiCompat, vec![], false);
    let dir = agent_dir("reaper", Wire::OpenaiCompat, &server);
    let clock = FakeClock::at(t0());
    let h = build(&dir, clock.clone()).await;
    std::fs::write(dir.join("home/old.txt"), "x").unwrap();
    let put = h.tools.get("trash.put").unwrap().call(json!({"path": "old.txt"})).await.unwrap();
    let agent_model::ToolResultPart::Text { text } = &put[0] else { panic!() };
    let id = text.lines().find_map(|l| l.strip_prefix("- `trash_id`: ")).unwrap().to_string();
    assert!(id.starts_with("20270115-080000-"), "корзина на часах службы: {id}");
    let info = |d: &std::path::Path| -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(d.join(format!("home/.local/share/Trash/hamlet/{id}/.trashinfo"))).unwrap()).unwrap() };
    h.scheduler.tick().await.unwrap();
    assert_eq!(info(&dir)["status"], "active");
    clock.advance_secs(30 * 86_400);
    h.scheduler.tick().await.unwrap();
    assert_eq!(info(&dir)["status"], "expired");
    assert!(!dir.join(format!("home/.local/share/Trash/hamlet/{id}/files")).exists());
    h.browsers.shutdown().await;
}
