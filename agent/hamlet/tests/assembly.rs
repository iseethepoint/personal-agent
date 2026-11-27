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
    let opts = Options { clock: Arc::new(clock) };
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
        let services = Services::start(&h);
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
        assert!(h.todo_continuation().await.is_none());
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

/// П4л-8: сверку памяти будит наблюдатель файлов — новый файл памяти
/// находится поиском без хода часов и без ровного такта.
#[tokio::test]
async fn memory_file_change_wakes_reindex() {
    let (server, _p) = model(Wire::OpenaiCompat, vec![], false);
    let dir = agent_dir("watch", Wire::OpenaiCompat, &server);
    let h = build(&dir, FakeClock::at(t0())).await;
    let services = Services::start(&h);
    let search = || async {
        let out = h.tools.get("hamlet.memory_search").unwrap().call(json!({"queries": ["zanzibar"]})).await.unwrap();
        let agent_model::ToolResultPart::Text { text } = &out[0] else { panic!() };
        text.clone()
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!search().await.contains("memory/trip.md"));
    std::fs::create_dir_all(dir.join("home/memory")).unwrap();
    std::fs::write(dir.join("home/memory/trip.md"), "# Trip\n\nThe owner plans a trip to Zanzibar in March.\n").unwrap();
    let mut found = String::new();
    for _ in 0..100 {
        found = search().await;
        if found.contains("memory/trip.md") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(found.contains("memory/trip.md"), "{found}");
    services.stop(&h).await;
}

/// П3е-1: перенаправить папку агента на другой поддельный сервер модели
/// (та же база, тот же дом).
fn repoint(dir: &Path, old: &Server, new: &Server) {
    let p = dir.join("config/models.toml");
    let s = std::fs::read_to_string(&p).unwrap().replace(&old.base_url, &new.base_url);
    std::fs::write(p, s).unwrap();
}

/// Начало запроса: системный текст и описания инструментов (оба провода).
fn request_start(wire: Wire, req: &serde_json::Value) -> String {
    match wire {
        Wire::Anthropic => format!("{}{}", req["system"], req["tools"]),
        Wire::OpenaiCompat => format!("{}{}", req["messages"][0], req["tools"]),
    }
}

async fn one_run(h: &Arc<Hamlet>, line: Option<&str>) -> String {
    let (tx, rx) = mpsc::channel(4);
    let (otx, mut orx) = mpsc::unbounded_channel();
    let runner = {
        let h = h.clone();
        tokio::spawn(async move { h.run(rx, otx).await })
    };
    if let Some(l) = line {
        tx.send(l.into()).await.unwrap();
    }
    let out = tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap();
    runner.abort();
    out
}

/// П3е-1: история главного агента пишется в базу по мере хода; после
/// перезапуска разговор продолжается с тем же началом подсказки байт в байт
/// (даже если файл дома изменился), а ход, прерванный посреди, —
/// продолжается сам.
#[tokio::test]
async fn conversation_survives_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        // Первый запуск: ход; второй ход прерывается после результата
        // инструмента (сервер модели больше не отвечает).
        let (s1, _p) = model(wire, vec![Step::Text("One."), Step::Calls(vec![("r1", "hamlet.read", json!({"path": "MEMORY.md"}))])], true);
        let dir = agent_dir("restart", wire, &s1);
        let h1 = build(&dir, FakeClock::at(t0())).await;
        assert_eq!(one_run(&h1, Some("first")).await, "One.", "{wire:?}");
        let broken = one_run(&h1, Some("read memory")).await;
        assert!(broken.starts_with("[ошибка хода]"), "{wire:?}: {broken}");
        let first_start = request_start(wire, &s1.requests()[0]);
        h1.browsers.shutdown().await;
        drop(h1);

        // Файл дома меняется, пока программа стоит: начало закреплено.
        std::fs::write(dir.join("home/MEMORY.md"), "# Memory\n\nChanged while stopped.\n").unwrap();
        let (s2, _p) = model(wire, vec![Step::Text("Coffee without sugar.")], true);
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        // Ход продолжается без новой строки: история кончалась результатом.
        assert_eq!(one_run(&h2, None).await, "Coffee without sugar.", "{wire:?}");
        let req = &s2.requests()[0];
        assert_eq!(request_start(wire, req), first_start, "{wire:?}: начало подсказки байт в байт");
        let body = req.to_string();
        assert!(body.contains("first") && body.contains("One.") && body.contains("read memory"), "{wire:?}: история из базы");
        assert!(body.contains("The owner drinks coffee without sugar."), "{wire:?}: результат инструмента из базы");
        assert!(!body.contains("Changed while stopped."), "{wire:?}");
        // В базе: first, заметка изменчивого, One., read memory, вызов,
        // результат, ответ после перезапуска.
        let saved = h2.history.load().await.unwrap().unwrap();
        assert_eq!(saved.history.len(), 7, "{wire:?}: {:?}", saved.history);
        assert!(saved.last_volatile.is_some_and(|v| v.contains("2027-01-15")), "{wire:?}");
        h2.browsers.shutdown().await;
    }
}

/// Адрес базы агента из его `agent.toml`.
fn db_url(dir: &Path) -> String {
    let toml = std::fs::read_to_string(dir.join("agent.toml")).unwrap();
    let var = toml.split("url_env = \"").nth(1).unwrap().split('"').next().unwrap();
    std::env::var(var).unwrap()
}

/// П3е-5: перезапуск посреди исполнения инструмента. Ход оставил точку
/// `before_inference` и взятую весть; после перезапуска вызов без результата
/// получает текст ядра «service restarted», ход продолжается под тем же
/// именем и закрывает свою весть, точка — `idle`. Весть, взятая ходом,
/// которого нет в точке, — `aborted`.
#[tokio::test]
async fn interrupted_tool_call_resumes_after_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (s1, _p) = model(wire, vec![Step::Calls(vec![("e1", "hamlet.exec", json!({"command": "sleep 30"}))])], true);
        let dir = agent_dir("interrupted", wire, &s1);
        let h1 = build(&dir, FakeClock::at(t0())).await;
        let (tx, rx) = mpsc::channel(4);
        let (otx, _orx) = mpsc::unbounded_channel();
        let runner = {
            let h = h1.clone();
            tokio::spawn(async move { h.run(rx, otx).await })
        };
        tx.send("start the work".into()).await.unwrap();
        // Модель ответила вызовом, команда идёт — программа обрывается.
        while s1.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        runner.abort();
        let _ = runner.await;
        h1.browsers.shutdown().await;
        drop(h1);

        let pool = sqlx::PgPool::connect(&db_url(&dir)).await.unwrap();
        let (mode, payload, class, owner): (String, Option<String>, Option<String>, Option<String>) =
            sqlx::query_as("select mode, payload_json, recovery_class, recovery_owner_id from agent.runtime_restart_checkpoints where agent_id = 'root'").fetch_one(&pool).await.unwrap();
        assert_eq!((mode.as_str(), class.as_deref(), owner.as_deref()), ("before_inference", Some("foreground_root"), Some("root")), "{wire:?}");
        let turn_id = serde_json::from_str::<serde_json::Value>(&payload.unwrap()).unwrap()["message_id"].as_str().unwrap().to_string();
        let attached: Vec<(String, String)> = sqlx::query_as("select submission_id, stream_owner_message_id from agent.message_mailbox where agent_id = 'root' and state = 'attached'").fetch_all(&pool).await.unwrap();
        assert_eq!(attached.len(), 1, "{wire:?}");
        assert_eq!(attached[0].1, turn_id, "{wire:?}");
        // Весть, взятая ходом, которого нет в точке.
        sqlx::query("insert into agent.message_mailbox (submission_id, accepted_ordinal, agent_id, conversation_epoch, submission_kind, routing_scope, state, stream_owner_message_id, submission_json) values ('root:mailbox:orphan', 1000, 'root', 0, 'notice', 'root', 'attached', 'root:turn:gone', '{}')")
            .execute(&pool)
            .await
            .unwrap();

        let (s2, _p) = model(wire, vec![Step::Text("Checked.")], true);
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        assert_eq!(one_run(&h2, None).await, "Checked.", "{wire:?}");
        let body = s2.requests()[0].to_string();
        assert!(body.contains("start the work"), "{wire:?}");
        assert!(body.contains(&serde_json::to_string(agent_loop::INTERRUPTED_TOOL_CALL).unwrap()[1..60]), "{wire:?}: результат оборванного вызова: {body}");
        let state = |id: String| {
            let pool = pool.clone();
            async move { sqlx::query_scalar::<_, String>("select state from agent.message_mailbox where submission_id = $1").bind(id).fetch_one(&pool).await.unwrap() }
        };
        assert_eq!(state(attached[0].0.clone()).await, "completed", "{wire:?}: весть хода закрыта им после перезапуска");
        assert_eq!(state("root:mailbox:orphan".into()).await, "aborted", "{wire:?}");
        let (mode, payload): (String, Option<String>) = sqlx::query_as("select mode, payload_json from agent.runtime_restart_checkpoints where agent_id = 'root'").fetch_one(&pool).await.unwrap();
        assert_eq!((mode.as_str(), payload), ("idle", None), "{wire:?}");
        // В истории: строка, заметка, вызов, результат перезапуска, ответ.
        let saved = h2.history.load().await.unwrap().unwrap();
        assert_eq!(saved.history.len(), 5, "{wire:?}: {:?}", saved.history);
        h2.browsers.shutdown().await;
    }
}

/// П3е-5: подагенты в базе. Подагент, оборванный перезапуском посреди
/// команды (свой рантайм гасится целиком), после старта продолжает ход:
/// вызов без результата закрыт текстом ядра, итог — родителю, в базе —
/// `completed`. Незаконченный без точки `before_inference` — `errored`, итог
/// родителю с текстом ядра. Номера новых подагентов не повторяют старые.
#[tokio::test]
async fn subagents_survive_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (s1, _p) = model(wire, vec![Step::Calls(vec![("x1", "hamlet.exec", json!({"command": "sleep 30"}))])], true);
        let s1 = Arc::new(s1);
        let dir = agent_dir("subagents", wire, &s1);
        let (d, server) = (dir.clone(), s1.clone());
        let id = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let id = rt.block_on(async {
                let h1 = build(&d, FakeClock::at(t0())).await;
                let id = h1.subagents.spawn("count the files in the workspace");
                while server.requests().is_empty() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
                id
            });
            // Перезапуск: все задачи рантайма гибнут посреди команды.
            drop(rt);
            id
        })
        .join()
        .unwrap();
        assert_eq!(id, "agent-1");

        let pool = sqlx::PgPool::connect(&db_url(&dir)).await.unwrap();
        let (kind, parent): (String, Option<String>) = sqlx::query_as("select kind, parent_agent_id from agent.agents where id = $1").bind(&id).fetch_one(&pool).await.unwrap();
        assert_eq!((kind.as_str(), parent.as_deref()), ("subagent", Some("root")), "{wire:?}");
        let mode: String = sqlx::query_scalar("select mode from agent.runtime_restart_checkpoints where agent_id = $1").bind(&id).fetch_one(&pool).await.unwrap();
        assert_eq!(mode, "before_inference", "{wire:?}");
        // Незаконченный подагент без точки — его ход продолжить нельзя.
        sqlx::query("insert into agent.agents (agent_id, id, kind, parent_id, parent_agent_id, depth, model, status) values ('agent-7', 'agent-7', 'subagent', 'root', 'root', 1, 'm', 'running')").execute(&pool).await.unwrap();
        sqlx::query("insert into agent.subagent_spawns (parent_agent_id, child_agent_id, child_depth, status, prompt) values ('root', 'agent-7', 1, 'running', 'old work')").execute(&pool).await.unwrap();

        let (s2, _p) = model(wire, vec![Step::Text("There are 3 files.")], true);
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        let mut notes = Vec::new();
        for _ in 0..300 {
            notes.extend(h2.inbox.drain().into_iter().map(|m| m.text()));
            if notes.len() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let body = s2.requests()[0].to_string();
        assert!(body.contains("count the files in the workspace"), "{wire:?}: история подагента из базы");
        assert!(body.contains(&serde_json::to_string(agent_loop::INTERRUPTED_TOOL_CALL).unwrap()[1..60]), "{wire:?}: {body}");
        assert!(notes.iter().any(|n| n.contains("agent_id: agent-1, status: done") && n.contains("There are 3 files.")), "{wire:?}: {notes:?}");
        assert!(notes.iter().any(|n| n.contains("agent_id: agent-7, status: failed") && n.contains(agent_loop::WORK_NOT_RESUMABLE)), "{wire:?}: {notes:?}");
        let spawn = |id: &'static str| {
            let pool = pool.clone();
            async move { sqlx::query_as::<_, (Option<String>, Option<i64>)>("select status, completed_at from agent.subagent_spawns where child_agent_id = $1").bind(id).fetch_one(&pool).await.unwrap() }
        };
        let (st, done) = spawn("agent-1").await;
        assert_eq!(st.as_deref(), Some("completed"), "{wire:?}");
        assert!(done.is_some());
        assert_eq!(spawn("agent-7").await.0.as_deref(), Some("errored"), "{wire:?}");
        let list = h2.subagents.list(20).to_string();
        assert!(list.contains("agent-1") && list.contains("agent-7"), "{wire:?}: {list}");
        h2.browsers.shutdown().await;
        // Новый номер — после старых.
        let h3 = build(&dir, FakeClock::at(t0())).await;
        assert_eq!(h3.subagents.spawn("more work"), "agent-8", "{wire:?}");
        h3.browsers.shutdown().await;
    }
}

/// П3е-5: прогон расписания, оборванный перезапуском посреди команды
/// работника, продолжается с записанного: вызов без результата закрыт
/// текстом ядра, за ним — заметка ядра «the run continues from what it
/// already recorded», итог доходит главному агенту; владелец
/// восстановления закрыт итогом прогона.
#[tokio::test]
async fn cron_run_continues_after_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (s1, _p) = model(wire, vec![Step::Calls(vec![("x1", "hamlet.exec", json!({"command": "sleep 30"}))])], true);
        let s1 = Arc::new(s1);
        let dir = agent_dir("cron-restart", wire, &s1);
        let (d, server) = (dir.clone(), s1.clone());
        let run_id = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let run_id = rt.block_on(async {
                let h1 = build(&d, FakeClock::at(t0())).await;
                let job = json!({"id": "count", "enabled": true, "mode": "task", "title": "Count", "body": "Count the files in the workspace.",
                    "schedule": {"kind": "runonce", "at": "2027-02-01T08:00", "timezone": "UTC", "timing": "exact"}});
                h1.tools.get("cron.add").unwrap().call(job).await.unwrap();
                let queued = h1.tools.get("cron.run").unwrap().call(json!({"id": "count"})).await.unwrap();
                let run_id = serde_json::from_str::<serde_json::Value>(&match &queued[0] {
                    agent_model::ToolResultPart::Text { text } => text.clone(),
                    p => panic!("{p:?}"),
                })
                .unwrap()["run_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let _services = Services::start(&h1);
                while server.requests().is_empty() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
                run_id
            });
            drop(rt);
            run_id
        })
        .join()
        .unwrap();

        let pool = sqlx::PgPool::connect(&db_url(&dir)).await.unwrap();
        let (class, owner): (Option<String>, Option<String>) =
            sqlx::query_as("select recovery_class, recovery_owner_id from agent.runtime_restart_checkpoints where agent_id = $1").bind(format!("cron-worker:{run_id}")).fetch_one(&pool).await.unwrap();
        assert_eq!((class.as_deref(), owner.as_deref()), (Some("scheduler_cron_execute"), Some(run_id.as_str())), "{wire:?}");

        let (s2, _p) = model(
            wire,
            vec![
                Step::Text("There are 3 files."),
                Step::Calls(vec![("n1", "hamlet.notify_main_agent", json!({"outcome": "succeeded", "message": "The workspace has 3 files."}))]),
            ],
            true,
        );
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        let services = Services::start(&h2);
        let mut notes = Vec::new();
        for _ in 0..300 {
            notes.extend(h2.inbox.drain().into_iter().map(|m| m.text()));
            if !notes.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(notes.iter().any(|n| n.contains("The workspace has 3 files.")), "{wire:?}: {notes:?}");
        let body = s2.requests()[0].to_string();
        assert!(body.contains("Count the files in the workspace."), "{wire:?}: история работника из базы");
        assert!(body.contains(&serde_json::to_string(agent_loop::INTERRUPTED_TOOL_CALL).unwrap()[1..60]), "{wire:?}: {body}");
        assert!(body.contains(&agent_cron::worker::CONTINUES_AFTER_RESTART[..60]), "{wire:?}: {body}");
        let status: String = sqlx::query_scalar("select status::text from scheduler.job_runs where run_id = $1").bind(&run_id).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "succeeded", "{wire:?}");
        let terminal: Option<String> = sqlx::query_scalar("select terminal_status from agent.recovery_owners where recovery_class = 'scheduler_cron_execute' and owner_id = $1").bind(&run_id).fetch_one(&pool).await.unwrap();
        assert_eq!(terminal.as_deref(), Some("succeeded"), "{wire:?}");
        services.stop(&h2).await;
    }
}

/// П3е-5: прогоны наблюдателей в `runtime.execute_resolve_runs`. Прогон,
/// оборванный перезапуском посреди команды работника, продолжается с
/// записанного (владелец `execute_resolve_worker`), итог доходит главному
/// агенту; итог `notify_pending`, не доставленный до остановки, доставляется
/// снова; взятый работником без точки — `failed` со стадией
/// `invalid_restart_checkpoint`.
#[tokio::test]
async fn hook_run_continues_after_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (s1, _p) = model(wire, vec![Step::Calls(vec![("x1", "hamlet.exec", json!({"command": "sleep 30"}))])], true);
        let s1 = Arc::new(s1);
        let dir = agent_dir("hook-restart", wire, &s1);
        let scripts = dir.join("home/hooks/scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(scripts.join("flag.sh"), "#!/usr/bin/env bash\nset -euo pipefail\nsource \"$HAMLET_HOOK_RUNTIME\"\nwake \"flag appeared\" '{\"text\":\"deploy finished\"}'\n").unwrap();
        let (d, server) = (dir.clone(), s1.clone());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let h1 = build(&d, FakeClock::at(t0())).await;
                h1.tools.get("hooks.add").unwrap().call(json!({"id": "flag", "script_path": "~/hooks/scripts/flag.sh", "prompt": "Tell the owner what the flag says."})).await.unwrap();
                h1.tools.get("hooks.run").unwrap().call(json!({"id": "flag"})).await.unwrap();
                let _services = Services::start(&h1);
                while server.requests().is_empty() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            });
            drop(rt);
        })
        .join()
        .unwrap();

        let pool = sqlx::PgPool::connect(&db_url(&dir)).await.unwrap();
        let (run_id, status, gen): (String, String, i64) = sqlx::query_as("select run_id, status, worker_generation from runtime.execute_resolve_runs where worker_kind = 'event_hook'").fetch_one(&pool).await.unwrap();
        assert_eq!((status.as_str(), gen), ("pending", 1), "{wire:?}");
        // Итог, не доставленный до остановки, и прогон, взятый работником без точки.
        sqlx::query("insert into runtime.execute_resolve_runs (run_id, worker_kind, source_ref, lane_key, status, terminal_decision, terminal_message, worker_agent_id) values ('hook:old:1', 'event_hook', 'old-1', 'old', 'notify_pending', 'notify_main_agent', 'The backup finished.', 'hook-worker:gone')").execute(&pool).await.unwrap();
        sqlx::query("insert into runtime.execute_resolve_runs (run_id, worker_kind, source_ref, lane_key, status, worker_agent_id, worker_generation) values ('hook:old:2', 'event_hook', 'old-2', 'old', 'pending', 'hook-worker:gone2', 1)").execute(&pool).await.unwrap();

        let (s2, _p) = model(
            wire,
            vec![Step::Text("The flag says deploy finished."), Step::Calls(vec![("n1", "hamlet.notify_main_agent", json!({"message": "The deploy finished."}))])],
            true,
        );
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        let services = Services::start(&h2);
        let mut notes = Vec::new();
        for _ in 0..300 {
            notes.extend(h2.inbox.drain().into_iter().map(|m| m.text()));
            if notes.len() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(notes.iter().any(|n| n.starts_with("[EVENT HOOK RESULT]") && n.ends_with("The deploy finished.")), "{wire:?}: {notes:?}");
        assert!(notes.iter().any(|n| n.ends_with("The backup finished.")), "{wire:?}: {notes:?}");
        let body = s2.requests()[0].to_string();
        assert!(body.contains("[EVENT HOOK \u{2014} EXECUTE]"), "{wire:?}: история работника из базы");
        assert!(body.contains(&serde_json::to_string(agent_loop::INTERRUPTED_TOOL_CALL).unwrap()[1..60]), "{wire:?}: {body}");
        let state = |id: String| {
            let pool = pool.clone();
            async move { sqlx::query_as::<_, (String, Option<String>)>("select status, last_error_stage from runtime.execute_resolve_runs where run_id = $1").bind(id).fetch_one(&pool).await.unwrap() }
        };
        assert_eq!(state(run_id.clone()).await.0, "completed_notified", "{wire:?}");
        assert_eq!(state("hook:old:1".into()).await.0, "completed_notified", "{wire:?}");
        assert_eq!(state("hook:old:2".into()).await, ("failed".to_string(), Some("invalid_restart_checkpoint".to_string())), "{wire:?}");
        let terminal: Option<String> = sqlx::query_scalar("select terminal_status from agent.recovery_owners where recovery_class = 'execute_resolve_worker' and owner_id = $1").bind(&run_id).fetch_one(&pool).await.unwrap();
        assert_eq!(terminal.as_deref(), Some("completed_notified"), "{wire:?}");
        services.stop(&h2).await;
    }
}

/// Сценарий из одного шага.
const ONE_STEP: &str = r#"export const meta = { name: "count", description: "One step", phases: ["work"] };
await phase("work");
return await agent("Count the files.", { key: "one" });
"#;

/// Запустить workflow в отдельном рантайме и погасить его, пока шаг
/// исполняет команду; вернуть id запуска.
fn workflow_until_killed(dir: &Path, server: Arc<Server>) -> String {
    let d = dir.to_path_buf();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let run_id = rt.block_on(async {
            let h1 = build(&d, FakeClock::at(t0())).await;
            let out = h1.tools.get("workflow.launch_async").unwrap().call(json!({"script": ONE_STEP})).await.unwrap();
            let text = match &out[0] {
                agent_model::ToolResultPart::Text { text } => text.clone(),
                p => panic!("{p:?}"),
            };
            while server.requests().is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            let at = text.find("workflow-run-").unwrap_or_else(|| panic!("{text}"));
            text[at..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>()
        });
        drop(rt);
        run_id
    })
    .join()
    .unwrap()
}

/// П3е-5: вызов шага workflow, оборванный перезапуском посреди команды,
/// продолжается при повторе сценария (владелец `workflow_subagent`, точка
/// подагента шага): вызов без результата закрыт текстом ядра, запуск
/// кончается ответом шага. Без владельца — вызов `failed` текстом ядра
/// `restart_after_output_unrecoverable`, и запуск получает эту ошибку.
#[tokio::test]
async fn workflow_call_continues_after_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        for keep_owner in [true, false] {
            let (s1, _p) = model(wire, vec![Step::Calls(vec![("x1", "hamlet.exec", json!({"command": "sleep 30"}))])], true);
            let s1 = Arc::new(s1);
            let dir = agent_dir("workflow-restart", wire, &s1);
            let run_id = workflow_until_killed(&dir, s1.clone());
            let pool = sqlx::PgPool::connect(&db_url(&dir)).await.unwrap();
            let (call_id, status): (String, String) = sqlx::query_as("select call_id, status from runtime.workflow_agent_calls where run_id = $1").bind(&run_id).fetch_one(&pool).await.unwrap();
            assert_eq!(status, "running", "{wire:?}");
            if !keep_owner {
                sqlx::query("delete from agent.recovery_owners where owner_id = $1").bind(&call_id).execute(&pool).await.unwrap();
            }

            let (s2, _p) = model(wire, vec![Step::Text(r#"{"status":"ok","result":"3 files"}"#)], true);
            repoint(&dir, &s1, &s2);
            let h2 = build(&dir, FakeClock::at(t0())).await;
            let services = Services::start(&h2);
            let run = tokio::time::timeout(Duration::from_secs(30), h2.workflows.wait(&run_id)).await.unwrap().unwrap();
            let (call_status, error): (String, Option<String>) = sqlx::query_as("select status, error from runtime.workflow_agent_calls where call_id = $1").bind(&call_id).fetch_one(&pool).await.unwrap();
            if keep_owner {
                assert_eq!(run.status, "completed", "{wire:?}: {run:?}");
                assert_eq!(call_status, "completed", "{wire:?}");
                let body = s2.requests()[0].to_string();
                assert!(body.contains("Count the files."), "{wire:?}: история шага из базы");
                assert!(body.contains(&serde_json::to_string(agent_loop::INTERRUPTED_TOOL_CALL).unwrap()[1..60]), "{wire:?}: {body}");
                let terminal: Option<String> = sqlx::query_scalar("select terminal_status from agent.recovery_owners where recovery_class = 'workflow_subagent' and owner_id = $1").bind(&call_id).fetch_one(&pool).await.unwrap();
                assert_eq!(terminal.as_deref(), Some("done"), "{wire:?}");
            } else {
                assert_eq!(run.status, "failed", "{wire:?}: {run:?}");
                assert_eq!((call_status.as_str(), error.as_deref()), ("failed", Some(agent_loop::WORK_NOT_RESUMABLE)), "{wire:?}");
                assert!(s2.requests().is_empty(), "{wire:?}: шаг не повторялся");
            }
            services.stop(&h2).await;
        }
    }
}

/// П3е-2: ящик в базе — весть, пришедшая, когда хода нет, и не взятая до
/// остановки, после перезапуска доходит новым ходом и закрывается.
#[tokio::test]
async fn mailbox_survives_restart() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (s1, _p) = model(wire, vec![], true);
        let dir = agent_dir("mailbox", wire, &s1);
        let h1 = build(&dir, FakeClock::at(t0())).await;
        let note = |t: &str| agent_model::Message { role: agent_model::Role::Developer, content: vec![agent_model::Part::Text { text: t.into() }] };
        h1.inbox.push(note("[Subagent Result] the report is ready.")).await;
        h1.browsers.shutdown().await;
        drop(h1);

        let (s2, _p) = model(wire, vec![Step::Text("The report is ready.")], true);
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        assert_eq!(one_run(&h2, None).await, "The report is ready.", "{wire:?}");
        assert!(s2.requests()[0].to_string().contains("[Subagent Result] the report is ready."), "{wire:?}: весть дошла");
        // Весть взята ходом и закрыта; в истории она есть.
        assert!(h2.inbox.drain().is_empty());
        let saved = h2.history.load().await.unwrap().unwrap();
        assert!(saved.history.iter().any(|m| m.text().contains("the report is ready")), "{wire:?}");
        let h3 = build(&dir, FakeClock::at(t0())).await;
        assert!(h3.inbox.drain().is_empty(), "{wire:?}: закрытая весть не возвращается");
        h2.browsers.shutdown().await;
        h3.browsers.shutdown().await;
    }
}

/// П3е-2: строка владельца — подача в ящик (`user`): пришедшая во время хода
/// берётся ходом между шагами, пришедшая во время остановки — после
/// перезапуска; всё по порядку.
#[tokio::test]
async fn owner_lines_go_through_the_mailbox_in_order() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let (s1, _p) = model(wire, vec![Step::Calls(vec![("e1", "hamlet.exec", json!({"command": "sleep 1"}))]), Step::Text("A.")], true);
        let dir = agent_dir("lines", wire, &s1);
        let h1 = build(&dir, FakeClock::at(t0())).await;
        let (tx, rx) = mpsc::channel(4);
        let (otx, mut orx) = mpsc::unbounded_channel();
        let runner = {
            let h = h1.clone();
            tokio::spawn(async move { h.run(rx, otx).await })
        };
        tx.send("first".into()).await.unwrap();
        // Пока идёт команда, приходит вторая строка.
        tokio::time::sleep(Duration::from_millis(300)).await;
        tx.send("second".into()).await.unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(30), orx.recv()).await.unwrap().unwrap(), "A.", "{wire:?}");
        let mid = s1.requests()[1].to_string();
        assert!(mid.contains("second"), "{wire:?}: строка посреди хода взята между шагами");
        runner.abort();
        // Программа стоит — третья строка ждёт в базе.
        h1.inbox.push_user(agent_model::Message { role: agent_model::Role::User, content: vec![agent_model::Part::Text { text: "third".into() }] }).await;
        h1.browsers.shutdown().await;
        drop(h1);

        let (s2, _p) = model(wire, vec![Step::Text("B.")], true);
        repoint(&dir, &s1, &s2);
        let h2 = build(&dir, FakeClock::at(t0())).await;
        assert_eq!(one_run(&h2, None).await, "B.", "{wire:?}");
        let body = s2.requests()[0].to_string();
        let at = |w: &str| body.find(w).unwrap_or_else(|| panic!("{wire:?}: нет {w}"));
        assert!(at("first") < at("second") && at("second") < at("third"), "{wire:?}: по порядку");
        h2.browsers.shutdown().await;
    }
}
