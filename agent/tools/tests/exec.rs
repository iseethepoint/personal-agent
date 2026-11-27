//! Тесты П4б: исполнение команд. Только безобидные команды (echo, sleep,
//! cat), рабочая папка — временная. Инструменты — напрямую, плюс один ход
//! через цикл на обоих поставщиках.

#[path = "../../loop/tests/common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use agent_loop::{Inbox, Tools, Turn};
use agent_model::{Message, Part, Registry, Role, ToolResultPart};
use agent_tools::{ExecTools, Home};
use common::{model, temp_dir, Step, Wire};
use serde_json::{json, Value};

fn setup(name: &str, secrets: Vec<String>) -> (std::path::PathBuf, Tools, Inbox) {
    let home = temp_dir(&format!("{name}-home"));
    let inbox = Inbox::new();
    let mut tools = Tools::new();
    ExecTools::new(Home::new(&home), secrets, Some(inbox.clone())).add_to(&mut tools);
    (home, tools, inbox)
}

async fn call(tools: &Tools, name: &str, args: Value) -> Result<Value, String> {
    match tools.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::test]
async fn short_command() {
    let (home, tools, inbox) = setup("short", vec![]);
    let r = call(&tools, "muse.exec", json!({"command": "echo привет; pwd"})).await.unwrap();
    assert_eq!(r["status"], "completed");
    assert_eq!(r["exit_code"], 0);
    assert_eq!(r["backgrounded"], false);
    let real = home.canonicalize().unwrap();
    assert_eq!(r["output"], format!("привет\n{}\n", real.display()));

    let r = call(&tools, "muse.exec", json!({"command": "echo нет >&2; exit 3"})).await.unwrap();
    assert_eq!((r["status"].as_str(), r["exit_code"].as_i64()), (Some("failed"), Some(3)));
    assert_eq!(r["output"], "нет\n");

    let r = call(&tools, "muse.exec", json!({"command": "cat", "stdin": "строка без перевода"})).await.unwrap();
    assert_eq!(r["output"], "строка без перевода");

    let r = call(&tools, "muse.exec", json!({"command": "echo $GREETING", "env": {"GREETING": "здравствуй"}})).await.unwrap();
    assert_eq!(r["output"], "здравствуй\n");
    // Короткая команда — в ящик ничего не приходит.
    assert!(inbox.drain().is_empty());
}

#[tokio::test]
async fn long_command_goes_to_background() {
    let (_home, tools, inbox) = setup("long", vec![]);
    let started = Instant::now();
    let r = call(&tools, "muse.exec", json!({"command": "echo начало; sleep 30", "yield_ms": 300})).await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!((r["status"].as_str(), r["backgrounded"].as_bool()), (Some("running"), Some(true)));
    assert_eq!(r["output"], "начало\n");
    let id = r["id"].as_str().unwrap().to_string();

    let list = call(&tools, "process.list", json!({})).await.unwrap();
    assert_eq!(list[0]["id"], id.as_str());
    assert_eq!(list[0]["status"], "running");

    // poll: нового вывода нет — ждёт timeout и отдаёт состояние.
    let started = Instant::now();
    let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 200})).await.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(180));
    assert_eq!((p["status"].as_str(), p["stdout"].as_str()), (Some("running"), Some("")));

    let log = call(&tools, "process.log", json!({"session_id": id})).await.unwrap();
    assert_eq!(log["lines"], json!(["начало"]));
    assert_eq!((log["totalLines"].as_u64(), log["nextOffset"].as_u64()), (Some(1), Some(1)));

    // clear до остановки — отказ; kill — вся группа процессов.
    assert!(call(&tools, "process.clear", json!({"session_id": id})).await.unwrap_err().contains("still running"));
    let k = call(&tools, "process.kill", json!({"session_id": id})).await.unwrap();
    assert_eq!(k["status"], "failed");
    assert_eq!(k["signal"], 9);
    assert!(call(&tools, "process.log", json!({"session_id": id})).await.is_ok(), "после kill вывод читается");
    call(&tools, "process.clear", json!({"session_id": id})).await.unwrap();
    assert_eq!(call(&tools, "process.list", json!({})).await.unwrap(), json!([]));
    assert!(call(&tools, "process.poll", json!({"session_id": id})).await.unwrap_err().contains("unknown process session"));
    let _ = inbox.drain();
}

#[tokio::test]
async fn background_result_arrives_by_itself() {
    let (_home, tools, inbox) = setup("deliver", vec![]);
    let r = call(&tools, "muse.exec", json!({"command": "sleep 0.3; echo готово", "background": true})).await.unwrap();
    assert_eq!(r["status"], "running");
    let id = r["id"].as_str().unwrap().to_string();
    tokio::time::timeout(Duration::from_secs(5), inbox.wait()).await.unwrap();
    let got = inbox.drain();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].role, Role::Developer);
    let text = got[0].text();
    assert!(text.starts_with(&format!("[Background exec finished] id: {id}, status: completed, exit_code: 0")), "{text}");
    assert!(text.contains("готово"));
}

#[tokio::test]
async fn write_and_keys_to_running_session() {
    let (_home, tools, _inbox) = setup("stdin", vec![]);
    let r = call(&tools, "muse.exec", json!({"command": "cat", "background": true})).await.unwrap();
    let id = r["id"].as_str().unwrap().to_string();
    let w = call(&tools, "process.write", json!({"session_id": id, "data": "эхо\n"})).await.unwrap();
    assert_eq!(w["bytes_written"], "эхо\n".len());
    let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 2000})).await.unwrap();
    assert_eq!(p["stdout"], "эхо\n");
    // Повторный poll — дельта уже отдана.
    let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 0})).await.unwrap();
    assert_eq!(p["stdout"], "");

    let k = call(&tools, "process.send_keys", json!({"session_id": id, "keys": ["tab", "c-c", "up"]})).await.unwrap();
    assert_eq!(k["bytes_written"], 1 + 1 + 3);
    let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 2000})).await.unwrap();
    assert_eq!(p["stdout"], "\t\u{3}\u{1b}[A");
    assert!(call(&tools, "process.send_keys", json!({"session_id": id, "keys": ["meta-x"]})).await.unwrap_err().contains("unsupported key token"));

    // eof закрывает stdin — cat заканчивает сам.
    call(&tools, "process.write", json!({"session_id": id, "data": "", "eof": true})).await.unwrap();
    let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 3000})).await.unwrap();
    assert_eq!(p["status"], "completed");
}

#[tokio::test]
async fn secrets_are_redacted() {
    // Имена ключей — из реестра моделей.
    let reg = Registry::parse(
        r#"
[defaults]
model = "anthropic/main"
maintenance = "anthropic/main"
[weights.big]
context_window = 200000
compaction_trigger = 150000
[wires]
"big-1" = { weights = "big" }
[[models]]
id = "anthropic/main"
provider = "anthropic"
model = "big-1"
api_key_env = "AGENT_TOOLS_TEST_SECRET_KEY"
"#,
    )
    .unwrap();
    assert_eq!(reg.api_key_envs(), vec!["AGENT_TOOLS_TEST_SECRET_KEY".to_string()]);
    // Безобидное поддельное значение, только для этого теста.
    std::env::set_var("AGENT_TOOLS_TEST_SECRET_KEY", "sk-fake-0123456789");
    let (_home, tools, _inbox) = setup("secret", reg.api_key_envs());
    let r = call(
        &tools,
        "muse.exec",
        json!({"command": "echo $AGENT_TOOLS_TEST_SECRET_KEY; echo ключ=sk-fake-0123456789 >&2"}),
    )
    .await
    .unwrap();
    let out = r["output"].as_str().unwrap();
    assert!(!out.contains("sk-fake-0123456789"), "{out}");
    assert_eq!(out, "<redacted>\nключ=<redacted>\n");

    let r = call(&tools, "muse.exec", json!({"command": "echo $AGENT_TOOLS_TEST_SECRET_KEY; sleep 30", "yield_ms": 200}))
        .await
        .unwrap();
    let id = r["id"].as_str().unwrap();
    for (tool, args) in [
        ("process.log", json!({"session_id": id})),
        ("process.poll", json!({"session_id": id, "timeout": 0})),
        ("process.list", json!({})),
    ] {
        let v = call(&tools, tool, args).await.unwrap().to_string();
        assert!(!v.contains("sk-fake-0123456789"), "{tool}: {v}");
    }
    call(&tools, "process.kill", json!({"session_id": id})).await.unwrap();
}

#[tokio::test]
async fn yield_default_and_tool_specs() {
    assert_eq!(agent_tools::exec::DEFAULT_YIELD_MS, 10_000);
    let (_home, tools, _inbox) = setup("specs", vec![]);
    let specs = tools.specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        ["muse.exec", "process.list", "process.poll", "process.log", "process.write", "process.send_keys", "process.kill", "process.clear"]
    );
    assert!(specs[0].description.starts_with("Execute a shell command. If the command is still running after `yield_ms`, it backgrounds"));
    assert_eq!(
        specs[0].input_schema["properties"]["yield_ms"]["description"],
        "Milliseconds to wait before backgrounding (default 10000)."
    );
}

async fn one_turn(wire: Wire) {
    let (_home, tools, _inbox) = setup("turn", vec![]);
    let steps = vec![
        Step::Calls(vec![("c1", "muse.exec", json!({"command": "echo из оболочки"}))]),
        Step::Text("Выполнил."),
    ];
    let (_server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &tools, "s", temp_dir("turn-spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "выполни".into() }] }];
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Выполнил.");
    let Part::ToolResult { content, is_error: false, .. } = &h[2].content[0] else { panic!("{:?}", h[2]) };
    let ToolResultPart::Text { text } = &content[0] else { panic!() };
    let v: Value = serde_json::from_str(text).unwrap();
    assert_eq!((v["status"].as_str(), v["output"].as_str()), (Some("completed"), Some("из оболочки\n")));
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}

#[tokio::test]
async fn pty_gives_a_terminal() {
    let (_home, tools, _inbox) = setup("pty", vec![]);
    // Без pty терминала нет.
    let r = call(&tools, "muse.exec", json!({"command": "tty"})).await.unwrap();
    assert_eq!(r["exit_code"], 1, "{r}");
    assert_eq!(r["pty"], false);
    assert!(r["output"].as_str().unwrap().contains("not a tty"), "{r}");
    // Под pty — есть.
    let r = call(&tools, "muse.exec", json!({"command": "tty", "pty": true})).await.unwrap();
    assert_eq!((r["exit_code"].as_i64(), r["pty"].as_bool()), (Some(0), Some(true)), "{r}");
    assert!(r["output"].as_str().unwrap().trim().starts_with("/dev/"), "{r}");
}

#[tokio::test]
async fn send_keys_reach_pty() {
    let (_home, tools, _inbox) = setup("pty-keys", vec![]);
    // read ждёт строку с терминала; Ctrl-C прерывает его, как в терминале.
    let r = call(&tools, "muse.exec", json!({"command": "read line; echo \"got:$line\"", "pty": true, "background": true}))
        .await
        .unwrap();
    let id = r["id"].as_str().unwrap().to_string();
    call(&tools, "process.write", json!({"session_id": id, "data": "привет"})).await.unwrap();
    call(&tools, "process.send_keys", json!({"session_id": id, "keys": ["enter"]})).await.unwrap();
    let mut out = String::new();
    for _ in 0..30 {
        let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 200})).await.unwrap();
        out.push_str(p["stdout"].as_str().unwrap());
        if p["status"] != "running" {
            break;
        }
    }
    assert!(out.contains("got:привет"), "{out:?}");

    let r = call(&tools, "muse.exec", json!({"command": "sleep 30", "pty": true, "background": true})).await.unwrap();
    let id = r["id"].as_str().unwrap().to_string();
    tokio::time::sleep(Duration::from_millis(200)).await;
    call(&tools, "process.send_keys", json!({"session_id": id, "keys": ["c-c"]})).await.unwrap();
    let p = call(&tools, "process.poll", json!({"session_id": id, "timeout": 3000})).await.unwrap();
    let p = if p["status"] == "running" { call(&tools, "process.poll", json!({"session_id": id, "timeout": 3000})).await.unwrap() } else { p };
    assert_ne!(p["status"], "running", "Ctrl-C через pty должен остановить sleep: {p}");
}

#[tokio::test]
async fn output_buffer_keeps_tail() {
    let cap = agent_tools::exec::OUTPUT_CAP_BYTES;
    assert_eq!(cap, 100 * 1024);
    let (_home, tools, _inbox) = setup("cap", vec![]);
    // 3000 строк по 100 байт = 300 000 байт > предела.
    let r = call(&tools, "muse.exec", json!({"command": "for i in $(seq 1 3000); do printf '%05d %093d\\n' $i 0; done"}))
        .await
        .unwrap();
    assert_eq!(r["truncated"], true);
    let out = r["output"].as_str().unwrap();
    assert!(out.len() <= cap);
    assert!(out.ends_with(&format!("{:05} {:093}\n", 3000, 0)), "хвост на месте");
    assert!(!out.contains(&format!("{:05} {:093}\n", 1, 0)), "начало отброшено");

    let r = call(&tools, "muse.exec", json!({"command": "echo коротко"})).await.unwrap();
    assert_eq!(r["truncated"], false);
}
