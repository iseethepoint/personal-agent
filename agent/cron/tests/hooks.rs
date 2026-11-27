//! Тесты П4е-2: наблюдатели и пульс. Часы поддельные, Postgres —
//! временный кластер на 127.0.0.1, модели — записанные ответы на обоих
//! проводах; скрипты — настоящий bash с jq.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_cron::{hook_tools, tools, Core, FakeClock, Scheduler, Store};
use agent_loop::{Inbox, Tool, Tools, Turn};
use agent_model::{Message, Part, Role, ToolResultPart};
use chrono::TimeZone;
use common::{model, temp_dir, AnyProvider, Server, Step, Wire};
use serde_json::{json, Value};

const WIRES: [Wire; 2] = [Wire::Anthropic, Wire::OpenaiCompat];
const SECRET: &str = "AGENT_CRON_TEST_SECRET_KEY";

fn t0() -> i64 {
    chrono::Utc.with_ymd_and_hms(2027, 1, 15, 8, 0, 0).unwrap().timestamp_millis()
}

struct Ctx {
    home: PathBuf,
    clock: FakeClock,
    core: Arc<Core>,
    sched: Scheduler<AnyProvider>,
    inbox: Inbox,
    server: Server,
}

async fn ctx(name: &str, wire: Wire, worker: Vec<Step>) -> Ctx {
    let home = temp_dir(&format!("hooks-{name}"));
    let clock = FakeClock::at(t0());
    let store = Store::connect(&pg::database()).await.unwrap();
    let core = Core::with_secrets(store, Arc::new(clock.clone()), &home, None, vec![SECRET.into()]);
    let (server, provider) = model(wire, worker, true);
    let mut wt = common::tools();
    for t in tools(&core) {
        wt.add(t);
    }
    let inbox = Inbox::new();
    let sched = Scheduler::new(core.clone(), Arc::new(provider), "worker system", Arc::new(wt), home.join("spill"), inbox.clone());
    Ctx { home, clock, core, sched, inbox, server }
}

fn script(home: &Path, name: &str, body: &str) {
    let dir = home.join("hooks/scripts");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), format!("#!/usr/bin/env bash\nset -euo pipefail\nsource \"$HAMLET_HOOK_RUNTIME\"\n{body}\n")).unwrap();
}

/// Будит, когда есть файл `~/flag`.
const FLAG: &str = r#"if [[ -f "$HOME_DIR/flag" ]]; then
  log "flag seen" '{"count":1}'
  wake "flag appeared" "$(jq -cn --arg t "$(cat "$HOME_DIR/flag")" '{text:$t}')"
fi
log "no flag" '{"count":0}'
silent "no flag""#;

fn flag_script(home: &Path) {
    script(home, "flag.sh", &format!("HOME_DIR={}\n{FLAG}", home.display()));
}

impl Ctx {
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        let all: Vec<Box<dyn Tool>> = hook_tools(&self.core)
            .into_iter()
            .map(|t| Box::new(t) as Box<dyn Tool>)
            .chain(tools(&self.core).into_iter().map(|t| Box::new(t) as Box<dyn Tool>))
            .collect();
        let t = all.into_iter().find(|t| t.spec().name == name).unwrap();
        match t.call(args).await {
            Ok(parts) => match &parts[0] {
                ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
                _ => panic!(),
            },
            Err(e) => Err(e.to_string()),
        }
    }

    async fn logs(&self, id: &str) -> Vec<Value> {
        self.call("hooks.logs", json!({"id": id, "limit": 100})).await.unwrap()["entries"].as_array().unwrap().clone()
    }

    fn handoffs(&self) -> Vec<String> {
        self.inbox
            .drain()
            .iter()
            .map(|m| m.content.iter().filter_map(|p| if let Part::Text { text } = p { Some(text.clone()) } else { None }).collect())
            .collect()
    }

    async fn tick(&self) {
        self.sched.tick().await.unwrap();
        self.sched.settle().await.unwrap();
    }
}

fn tool_names(req: &Value) -> Vec<String> {
    req["tools"]
        .as_array()
        .map(|a| a.iter().map(|t| t["name"].as_str().or_else(|| t["function"]["name"].as_str()).unwrap_or_default().to_string()).collect())
        .unwrap_or_default()
}

fn hook(id: &str) -> Value {
    json!({"id": id, "script_path": "~/hooks/scripts/flag.sh", "prompt": "Check the flag and tell the owner what it says."})
}

#[tokio::test]
async fn hook_wakes_worker_only_when_script_says_wake() {
    for wire in WIRES {
        let worker = vec![Step::Text("The flag says: deploy finished."), Step::Calls(vec![("n1", "hamlet.notify_main_agent", json!({"message": "Деплой закончился."}))])];
        let c = ctx("wake", wire, worker).await;
        flag_script(&c.home);
        let r = c.call("hooks.add", hook("flag")).await.unwrap();
        assert_eq!(r["enabled"], false, "{r}");
        assert!(r["message"].as_str().unwrap().starts_with("The hook was created disabled."));
        c.call("hooks.enable", json!({"id": "flag"})).await.unwrap();

        c.tick().await;
        assert!(c.server.requests().is_empty(), "{wire:?}: скрипт сказал silent — работника нет");
        assert_eq!(c.logs("flag").await.last().unwrap()["outcome"], "silent");

        // До срока опроса скрипт не запускается.
        c.clock.advance_secs(30);
        std::fs::write(c.home.join("flag"), "deploy finished").unwrap();
        c.tick().await;
        assert_eq!(c.logs("flag").await.len(), 1);
        assert!(c.server.requests().is_empty());

        c.clock.advance_secs(30);
        c.tick().await;
        let logs = c.logs("flag").await;
        assert_eq!(logs.last().unwrap()["outcome"], "wake", "{logs:?}");
        assert_eq!(logs.last().unwrap()["script_logs"][0]["message"], "flag seen");
        let reqs = c.server.requests();
        assert_eq!(reqs.len(), 2, "{wire:?}");
        let first = reqs[0].to_string();
        assert!(first.contains("[EVENT HOOK \u{2014} EXECUTE]") && first.contains("source_ref: hook:flag"), "{first}");
        assert!(first.contains("\\\"wake_reason\\\": \\\"flag appeared\\\"") && first.contains("<hook_instruction>\\nCheck the flag"), "{first}");
        assert!(first.contains("You are an event hook worker"), "системный текст работника наблюдателя");
        assert!(reqs[1].to_string().contains("[EVENT HOOK \u{2014} RESOLVE]"));
        assert_eq!(tool_names(&reqs[1]), ["hamlet.nothing_to_do", "hamlet.notify_main_agent"]);
        let h = c.handoffs();
        assert_eq!(h.len(), 1);
        assert!(h[0].starts_with("[EVENT HOOK RESULT]This result came from an event hook the user asked for.") && h[0].ends_with("Деплой закончился."), "{}", h[0]);
    }
}

#[tokio::test]
async fn nothing_to_do_keeps_hook_quiet() {
    for wire in WIRES {
        let c = ctx("quiet", wire, vec![Step::Text("Stale."), Step::Calls(vec![("q1", "hamlet.nothing_to_do", json!({}))])]).await;
        flag_script(&c.home);
        std::fs::write(c.home.join("flag"), "old").unwrap();
        c.call("hooks.add", hook("flag")).await.unwrap();
        c.call("hooks.run", json!({"id": "flag"})).await.unwrap();
        c.tick().await;
        assert_eq!(c.server.requests().len(), 2, "{wire:?}");
        assert!(c.handoffs().is_empty());
    }
}

#[tokio::test]
async fn hooks_through_turn_add_and_dry_run() {
    for wire in WIRES {
        let c = ctx("turn", wire, vec![]).await;
        flag_script(&c.home);
        let (main, provider) = model(
            wire,
            vec![
                Step::Calls(vec![("c1", "hooks.add", hook("flag"))]),
                Step::Calls(vec![("c2", "hooks.dry_run", json!({"id": "flag"}))]),
                Step::Text("Проверил."),
            ],
            true,
        );
        let mut t = Tools::new();
        for tool in hook_tools(&c.core) {
            t.add(tool);
        }
        let turn = Turn::new(&provider, &t, "main", c.home.join("spill"));
        let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "следи за флагом".into() }] }];
        turn.run(&mut h, &mut |_| {}).await.unwrap();
        let results: Vec<Value> = h
            .iter()
            .flat_map(|m| &m.content)
            .filter_map(|p| match p {
                Part::ToolResult { content, is_error: false, .. } => match &content[0] {
                    ToolResultPart::Text { text } => serde_json::from_str(text).ok(),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 2, "{wire:?}: {h:?}");
        let dry = &results[1];
        assert_eq!(dry["decision"], "silent", "{dry}");
        assert_eq!(dry["would_wake"], false);
        // Все наблюдённые значения нулевые, срабатываний ещё не было.
        assert!(dry["observation_note"].as_str().unwrap().starts_with("Every value this dry run observed is zero or empty"));
        assert!(dry["wake_note"].as_str().unwrap().starts_with("This hook has never produced a wake"));
        assert!(main.requests()[0].to_string().contains("Create and manage event hooks"));
    }
}

#[tokio::test]
async fn dry_run_wake_admits_no_worker_and_skips_state() {
    let c = ctx("dry", Wire::Anthropic, vec![]).await;
    script(&c.home, "state.sh", r#"hook_state_set '{"seen":true}'
wake "found" '{"id":7}'"#);
    c.call("hooks.add", json!({"id": "state", "script_path": "~/hooks/scripts/state.sh", "prompt": "Handle it."})).await.unwrap();
    let r = c.call("hooks.dry_run", json!({"id": "state"})).await.unwrap();
    assert_eq!(r["would_wake"], true, "{r}");
    assert_eq!(r["payload"], json!({"id": 7}));
    assert!(r["state_note"].as_str().unwrap().starts_with("This dry run reached a wake decision but admitted no wake"));
    assert!(!c.home.join("hooks/state/state.json").exists(), "hook_state_set на пробном прогоне не пишет");
    c.tick().await;
    assert!(c.server.requests().is_empty());
    // Живой прогон пишет состояние и будит.
    let r = c.call("hooks.run", json!({"id": "state"})).await.unwrap();
    assert_eq!(r["decision"], "wake");
    assert_eq!(std::fs::read_to_string(c.home.join("hooks/state/state.json")).unwrap().trim(), r#"{"seen":true}"#);
}

#[tokio::test]
async fn edited_script_is_not_polled_until_updated() {
    let c = ctx("edited", Wire::Anthropic, vec![]).await;
    flag_script(&c.home);
    c.call("hooks.add", hook("flag")).await.unwrap();
    c.call("hooks.enable", json!({"id": "flag"})).await.unwrap();
    script(&c.home, "flag.sh", "wake \"edited by hand\"");
    c.tick().await;
    let logs = c.logs("flag").await;
    assert_eq!(logs.last().unwrap()["outcome"], "definition_changed", "{logs:?}");
    assert!(c.server.requests().is_empty());
    c.call("hooks.update", json!({"id": "flag"})).await.unwrap();
    c.clock.advance_secs(60);
    c.sched.tick().await.unwrap();
    assert_eq!(c.logs("flag").await.last().unwrap()["outcome"], "wake");
}

#[tokio::test]
async fn disable_after_run_turns_the_hook_off() {
    let c = ctx("disable", Wire::Anthropic, vec![]).await;
    script(&c.home, "once.sh", "disable_after_run\nsilent \"done for good\"");
    c.call("hooks.add", json!({"id": "once", "script_path": "~/hooks/scripts/once.sh", "prompt": "x"})).await.unwrap();
    c.call("hooks.enable", json!({"id": "once"})).await.unwrap();
    c.tick().await;
    let v = c.call("hooks.view", json!({"id": "once"})).await.unwrap();
    assert_eq!(v["enabled"], false, "{v}");
    let last = c.logs("once").await.pop().unwrap();
    assert_eq!(last["disable_requested"], true);
    assert_eq!(last["disable_outcome"], "disabled");
    assert!(c.call("hooks.list", json!({})).await.unwrap()["hooks"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn script_runs_without_secret_env() {
    // SAFETY: переменную читает только скрипт этого теста.
    unsafe { std::env::set_var(SECRET, "sk-test-value") };
    let c = ctx("secret", Wire::Anthropic, vec![]).await;
    script(&c.home, "env.sh", &format!("silent \"checked\" \"$(jq -cn --arg v \"${{{SECRET}:-}}\" '{{secret:$v, home:env.HOME}}')\""));
    c.call("hooks.add", json!({"id": "env", "script_path": "~/hooks/scripts/env.sh", "prompt": "x"})).await.unwrap();
    let r = c.call("hooks.dry_run", json!({"id": "env"})).await.unwrap();
    assert_eq!(r["payload"]["secret"], "", "{r}");
    assert!(!r["payload"]["home"].as_str().unwrap().is_empty(), "обычное окружение остаётся");
}

#[tokio::test]
async fn hook_validation_and_script_errors() {
    let c = ctx("validate", Wire::Anthropic, vec![]).await;
    flag_script(&c.home);
    let bad = |id: &str| json!({"id": id, "script_path": "~/hooks/scripts/flag.sh", "prompt": "x"});
    assert!(c.call("hooks.add", bad("Bad")).await.unwrap_err().contains("must start with a lowercase ASCII letter or digit"));
    assert!(c.call("hooks.add", bad("a.b")).await.unwrap_err().contains("may contain only lowercase ASCII letters"));
    let e = c.call("hooks.add", json!({"id": "x", "script_path": "~/hooks/scripts/missing.sh", "prompt": "x"})).await.unwrap_err();
    assert!(e.contains("hook script is not a regular file"), "{e}");
    std::fs::write(c.home.join("outside.sh"), "").unwrap();
    assert!(c.call("hooks.add", json!({"id": "x", "script_path": "~/outside.sh", "prompt": "x"})).await.unwrap_err().contains("Invalid hook script path"));
    assert!(c.call("hooks.add", json!({"id": "x", "script_path": "~/hooks/scripts/flag.py", "prompt": "x"})).await.unwrap_err().contains(".sh extension"));
    assert!(c.call("hooks.add", json!({"id": "x", "script_path": "~/hooks/scripts/flag.sh", "prompt": " "})).await.unwrap_err().contains("prompt cannot be empty"));
    c.call("hooks.add", bad("x")).await.unwrap();
    assert!(c.call("hooks.add", bad("x")).await.unwrap_err().contains("hook already exists: x"));
    // Пределы ядра: poll 5..604800, срок скрипта 1..600, prompt до 65 536 байт.
    let with = |k: &str, v: Value| {
        let mut h = bad("y");
        h[k] = v;
        h
    };
    for (k, v, err) in [
        ("poll_interval_secs", json!(4), "poll_interval_secs must be between 5 and 604800"),
        ("poll_interval_secs", json!(604_801), "poll_interval_secs must be between 5 and 604800"),
        ("script_timeout_secs", json!(601), "script_timeout_secs must be between 1 and 600"),
        ("prompt", json!("p".repeat(65_537)), "prompt exceeds the 65536-byte limit"),
    ] {
        assert_eq!(c.call("hooks.add", with(k, v)).await.unwrap_err(), err);
    }
    c.call("hooks.add", with("prompt", json!("p".repeat(65_536)))).await.unwrap();
    assert_eq!(c.call("hooks.update", json!({"id": "y", "poll_interval_secs": 3})).await.unwrap_err(), "poll_interval_secs must be between 5 and 604800");
    let r = c.call("hooks.update", json!({"id": "y", "poll_interval_secs": 5, "script_timeout_secs": 600})).await.unwrap();
    assert_eq!(r["poll_interval_secs"], 5, "{r}");
    assert!(c.call("hooks.view", json!({"id": "nope"})).await.unwrap_err().contains("hook does not exist: nope"));

    for (name, body, err) in [
        ("none.sh", "echo hi", "script exited without calling silent() or wake()"),
        ("fail.sh", "exit 3", "hook script exited unsuccessfully"),
        ("two.sh", "printf 'HAMLET_HOOK_RESULT:{\"decision\":\"silent\"}\\n'; silent \"again\"", "script emitted more than one hook decision"),
    ] {
        script(&c.home, name, body);
        let id = name.trim_end_matches(".sh");
        c.call("hooks.add", json!({"id": id, "script_path": format!("~/hooks/scripts/{name}"), "prompt": "x"})).await.unwrap();
        let r = c.call("hooks.dry_run", json!({"id": id})).await.unwrap();
        assert_eq!(r["error"], err, "{r}");
    }
    c.call("hooks.remove", json!({"id": "x"})).await.unwrap();
    assert!(!c.home.join("hooks/definitions/x.json").exists());
}

#[tokio::test]
async fn heartbeat_runs_checklist_and_skips_when_empty() {
    for wire in WIRES {
        let worker = vec![Step::Text("Checked the inbox."), Step::Calls(vec![("n1", "hamlet.notify_main_agent", json!({"outcome": "succeeded", "message": "Новое письмо от банка."}))])];
        let c = ctx("heartbeat", wire, worker).await;
        c.sched.ensure_heartbeat().await.unwrap();
        c.sched.ensure_heartbeat().await.unwrap();
        let tmpl = std::fs::read_to_string(c.home.join("HEARTBEAT.md")).unwrap();
        assert!(tmpl.starts_with("# HEARTBEAT.md\n"));
        assert!(c.home.join("workspace/cron.d/minutely/heartbeat__interval@30m.md").exists());
        let st = c.call("cron.status", json!({"id": "heartbeat"})).await.unwrap();
        assert_eq!(st["next_run_local"], "Fri 2027-01-15 08:30", "{st}");
        assert_eq!(st["system"], true);
        let e = c.call("cron.update", json!({"id": "heartbeat", "enabled": false})).await.unwrap_err();
        assert!(e.contains("heartbeat is the assistant's heartbeat and cannot be paused or resumed"), "{e}");
        assert!(c.call("cron.remove", json!({"id": "heartbeat"})).await.is_err());

        // Шаблон без инструкций — модель не зовётся.
        c.clock.advance_secs(30 * 60);
        c.tick().await;
        assert!(c.server.requests().is_empty(), "{wire:?}");
        let runs = c.call("cron.runs", json!({"id": "heartbeat"})).await.unwrap();
        assert_eq!(runs["runs"][0]["result_summary"], "heartbeat_no_instructions");

        std::fs::write(c.home.join("HEARTBEAT.md"), format!("{tmpl}- Check the inbox for bank letters.\n")).unwrap();
        c.clock.advance_secs(30 * 60);
        c.tick().await;
        let reqs = c.server.requests();
        assert_eq!(reqs.len(), 2, "{wire:?}");
        let first = reqs[0].to_string();
        assert!(first.contains("mode: heartbeat") && first.contains("[HEARTBEAT]\\n# HEARTBEAT.md"), "{first}");
        assert!(first.contains("- Check the inbox for bank letters.") && first.contains("[BODY]\\nRun the heartbeat checklist from HEARTBEAT.md."));
        assert!(first.contains("Treat cadence text as a `cron.update` directive only when this task's body explicitly defines cadence bands"), "правила системного задания");
        let h = c.handoffs();
        assert!(h[0].starts_with("[HEARTBEAT HANDOFF]\n") && h[0].contains("Новое письмо от банка."), "{}", h[0]);
    }
}

#[test]
fn heartbeat_instructions_ignore_comments_and_template() {
    assert!(!agent_cron::has_instructions(agent_cron::HEARTBEAT_TEMPLATE));
    assert!(!agent_cron::has_instructions("\n# only a comment\n\n"));
    assert!(agent_cron::has_instructions("# HEARTBEAT.md\n- Look at the weather.\n"));
}
