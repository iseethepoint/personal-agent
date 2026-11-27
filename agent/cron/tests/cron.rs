//! Тесты П4е-1: расписания. Часы поддельные, Postgres — временный кластер
//! на 127.0.0.1, модели — записанные ответы на обоих проводах; живых
//! вызовов нет.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::path::PathBuf;
use std::sync::Arc;

use agent_cron::{tools, Core, FakeClock, Scheduler, Store};
use agent_loop::{Inbox, Tool, Tools, Turn};
use agent_model::{Message, Part, ToolResultPart};
use chrono::TimeZone;
use common::{model, temp_dir, AnyProvider, Server, Step, Wire};
use serde_json::{json, Value};

const WIRES: [Wire; 2] = [Wire::Anthropic, Wire::OpenaiCompat];

/// Пятница, 15 января 2027, 08:00 UTC.
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

async fn ctx(name: &str, wire: Wire, worker: Vec<Step>, user_tz: Option<&str>) -> Ctx {
    let home = temp_dir(&format!("cron-{name}"));
    let clock = FakeClock::at(t0());
    let store = Store::connect(&pg::database()).await.unwrap();
    let core = Core::new(store, Arc::new(clock.clone()), &home, user_tz.map(String::from));
    let (server, provider) = model(wire, worker, true);
    let mut wt = common::tools();
    for t in tools(&core) {
        wt.add(t);
    }
    let inbox = Inbox::new();
    let sched = Scheduler::new(core.clone(), Arc::new(provider), "worker system", Arc::new(wt), home.join("spill"), inbox.clone());
    Ctx { home, clock, core, sched, inbox, server }
}

impl Ctx {
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        let t = tools(&self.core).into_iter().find(|t| t.spec().name == name).unwrap();
        match t.call(args).await {
            Ok(parts) => match &parts[0] {
                ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
                _ => panic!(),
            },
            Err(e) => Err(e.to_string()),
        }
    }

    async fn runs(&self, id: &str) -> Vec<Value> {
        self.call("cron.runs", json!({"id": id})).await.unwrap()["runs"].as_array().unwrap().clone()
    }

    fn handoffs(&self) -> Vec<String> {
        self.inbox.drain().iter().map(text_of).collect()
    }
}

fn text_of(m: &Message) -> String {
    m.content
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn tool_names(req: &Value) -> Vec<String> {
    req["tools"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| t["name"].as_str().or_else(|| t["function"]["name"].as_str()).unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn water(schedule: Value) -> Value {
    json!({"id": "water", "enabled": true, "mode": "task", "title": "Water", "body": "Remind the owner to drink water.", "schedule": schedule})
}

fn notify(id: &'static str, outcome: &str, message: &str) -> Step {
    Step::Calls(vec![(id, "hamlet.notify_main_agent", json!({"outcome": outcome, "message": message}))])
}

fn quiet(id: &'static str) -> Step {
    Step::Calls(vec![(id, "hamlet.nothing_to_report", json!({}))])
}

#[tokio::test]
async fn cron_add_through_turn_saves_copy_and_reports_next_run() {
    for wire in WIRES {
        let c = ctx("add-turn", wire, vec![], Some("Europe/Moscow")).await;
        let (main, provider) = model(
            wire,
            vec![Step::Calls(vec![("c1", "cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"})))]), Step::Text("Готово")],
            true,
        );
        let mut t = Tools::new();
        for tool in tools(&c.core) {
            t.add(tool);
        }
        let turn = Turn::new(&provider, &t, "main system", c.home.join("spill"));
        let mut history = vec![Message { role: agent_model::Role::User, content: vec![Part::Text { text: "напоминай пить воду каждый час".into() }] }];
        turn.run(&mut history, &mut |_| {}).await.unwrap();
        let result = history
            .iter()
            .flat_map(|m| &m.content)
            .find_map(|p| match p {
                Part::ToolResult { content, is_error: false, .. } => match &content[0] {
                    ToolResultPart::Text { text } => Some(serde_json::from_str::<Value>(text).unwrap()),
                    _ => None,
                },
                _ => None,
            })
            .expect("результат cron.add");
        assert_eq!(result["next_run_in"], "1h 0m", "{wire:?}: {result}");
        assert_eq!(result["timeout_secs"], 3600);
        assert!(result["timeout_note"].as_str().unwrap().contains("The next schedule gap is 3600s"), "{result}");
        let copy = std::fs::read_to_string(c.home.join("workspace/cron.d/hourly/water__interval@1h.md")).unwrap();
        assert!(copy.starts_with("---\nid: water\n"), "{copy}");
        assert!(copy.ends_with("Remind the owner to drink water.\n"), "{copy}");
        // Описание пространства `cron` — не в `cron.add`, а при загрузке
        // пространства (`agent_cron::cron_namespace`).
        let req = &main.requests()[0];
        assert!(tool_names(req).contains(&"cron.add".to_string()));
        assert!(!req.to_string().contains("Manage saved scheduled jobs through the cron tools."));
        assert!(agent_cron::cron_namespace().contains("Manage saved scheduled jobs through the cron tools."));
    }
}

#[tokio::test]
async fn interval_fires_on_fake_clock_and_hands_off_to_main_agent() {
    for wire in WIRES {
        let c = ctx("interval", wire, vec![Step::Text("Reminder: drink water."), notify("n1", "succeeded", "Пора выпить воды.")], None).await;
        c.call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await.unwrap();
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        assert!(c.runs("water").await.is_empty(), "до срока прогонов нет");

        c.clock.advance_secs(3600);
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();

        let reqs = c.server.requests();
        assert_eq!(reqs.len(), 2, "{wire:?}: исполнение и итог");
        let first = reqs[0].to_string();
        assert!(first.contains("[WORKER TASK]") && first.contains("job_id: water"), "{first}");
        assert!(first.contains("[SCHEDULED TASK CONTEXT]") && first.contains("<scheduled_job id=\\\"water\\\">"), "{first}");
        assert!(first.contains("[BODY]\\nRemind the owner to drink water.") && first.contains("[EXECUTION PHASE FENCE]"));
        assert!(reqs[1].to_string().contains("Call exactly one final tool"));
        assert_eq!(tool_names(&reqs[1]), ["hamlet.nothing_to_report", "hamlet.notify_main_agent"]);

        let h = c.handoffs();
        assert_eq!(h.len(), 1, "{h:?}");
        assert!(h[0].starts_with("[SCHEDULED TASK HANDOFF]\nThis hidden scheduled-work handoff"), "{}", h[0]);
        assert!(h[0].contains("<job_result>\nПора выпить воды.\n</job_result>"), "{}", h[0]);
        let runs = c.runs("water").await;
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0]["status"], "succeeded");
        assert_eq!(runs[0]["result_summary"], "Пора выпить воды.");
        let timed_out: bool = sqlx::query_scalar("select worker_execution_timed_out from scheduler.job_runs").fetch_one(c.core.store.pool()).await.unwrap();
        assert!(!timed_out);
        let st = c.call("cron.status", json!({"id": "water"})).await.unwrap();
        assert_eq!(st["next_run_in"], "1h 0m", "{st}");
        assert_eq!(st["last_status"], "succeeded");
    }
}

#[tokio::test]
async fn nothing_to_report_is_silent_and_runonce_does_not_repeat() {
    for wire in WIRES {
        let c = ctx("quiet", wire, vec![Step::Text("Nothing new."), quiet("q1")], None).await;
        c.call("cron.add", water(json!({"kind": "runonce", "at": "2027-01-15T08:01", "timezone": "UTC", "timing": "exact"}))).await.unwrap();
        c.clock.advance_secs(60);
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        assert_eq!(c.server.requests().len(), 2, "{wire:?}: после завершающего вызова модель не зовётся");
        assert!(c.handoffs().is_empty());
        let runs = c.runs("water").await;
        assert_eq!(runs[0]["status"], "succeeded");
        let signal: String = sqlx::query_scalar("select signal_type from scheduler.terminal_signals")
            .fetch_one(c.core.store.pool())
            .await
            .unwrap();
        assert_eq!(signal, "nothing_to_report");
        let st = c.call("cron.status", json!({"id": "water"})).await.unwrap();
        assert!(st["next_run_local"].is_null(), "{st}");
        c.clock.advance_secs(86400);
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        assert_eq!(c.runs("water").await.len(), 1);
    }
}

#[tokio::test]
async fn nothing_to_report_is_rejected_after_a_failed_call() {
    for wire in WIRES {
        let worker = vec![
            Step::Calls(vec![("f1", "fail", json!({}))]),
            Step::Text("Tried and failed."),
            quiet("q1"),
            notify("n1", "failed", "Не вышло: disk is on fire."),
        ];
        let c = ctx("rejected", wire, worker, None).await;
        c.call("cron.add", water(json!({"kind": "runonce", "at": "2027-01-15T08:00", "timezone": "UTC", "timing": "exact"}))).await.unwrap();
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        let reqs = c.server.requests();
        assert_eq!(reqs.len(), 4, "{wire:?}");
        let last = reqs[3].to_string();
        assert!(last.contains("was rejected because the execution record includes a failed call"), "{last}");
        assert!(last.contains("failed call: `fail`: disk is on fire"), "{last}");
        let runs = c.runs("water").await;
        assert_eq!(runs[0]["status"], "failed");
        let h = c.handoffs();
        assert_eq!(h.len(), 1);
        assert!(h[0].contains("Не вышло: disk is on fire."));
    }
}

#[tokio::test]
async fn execution_timeout_goes_to_resolution_with_notice() {
    for wire in WIRES {
        let worker = vec![Step::Calls(vec![("s1", "slow", json!({"ms": 5000}))]), notify("n1", "failed", "Не успел.")];
        let c = ctx("timeout", wire, worker, None).await;
        let mut job = water(json!({"kind": "runonce", "at": "2027-01-15T08:00", "timezone": "UTC", "timing": "exact"}));
        job["timeout_secs"] = json!(1);
        c.call("cron.add", job).await.unwrap();
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        let reqs = c.server.requests();
        assert_eq!(reqs.len(), 2, "{wire:?}");
        let second = reqs[1].to_string();
        assert!(second.contains("[CRON EXECUTION TIMEOUT]\\nstatus: execution_timed_out\\nexecution_timeout_secs: 1"), "{second}");
        assert_eq!(c.runs("water").await[0]["status"], "failed");
        assert!(c.handoffs()[0].contains("Не успел."));
        let timed_out: bool = sqlx::query_scalar("select worker_execution_timed_out from scheduler.job_runs").fetch_one(c.core.store.pool()).await.unwrap();
        assert!(timed_out, "{wire:?}");
    }
}

#[tokio::test]
async fn run_without_terminal_call_fails_and_tells_main_agent() {
    for wire in WIRES {
        let c = ctx("unresolved", wire, vec![Step::Text("Done."), Step::Text("All good, nothing to add.")], None).await;
        c.call("cron.add", water(json!({"kind": "runonce", "at": "2027-01-15T08:00", "timezone": "UTC", "timing": "exact"}))).await.unwrap();
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        let runs = c.runs("water").await;
        assert_eq!(runs[0]["status"], "failed", "{wire:?}");
        let h = c.handoffs();
        assert!(h[0].contains("The cron run failed before it could finish.") && h[0].contains("never filed its report"), "{}", h[0]);
    }
}

#[tokio::test]
async fn runtime_failure_is_retried_by_policy() {
    for wire in WIRES {
        let worker = vec![Step::Text("Done."), Step::Text("no tool"), Step::Text("Done again."), quiet("q1")];
        let c = ctx("retry", wire, worker, None).await;
        let mut job = water(json!({"kind": "runonce", "at": "2027-01-15T08:00", "timezone": "UTC", "timing": "exact"}));
        job["retry"] = json!({"on_failure": true, "max_retries": 1});
        c.call("cron.add", job).await.unwrap();
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        assert!(c.handoffs().is_empty(), "{wire:?}: сбой с повтором главному агенту не идёт");
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        let runs = c.runs("water").await;
        let statuses: Vec<_> = runs.iter().map(|r| (r["attempt"].as_i64().unwrap(), r["status"].as_str().unwrap().to_string())).collect();
        assert_eq!(statuses, [(2, "succeeded".to_string()), (1, "failed".to_string())]);
        assert!(c.handoffs().is_empty());
    }
}

#[tokio::test]
async fn add_validates_like_muse() {
    let c = ctx("validate", Wire::Anthropic, vec![], None).await;
    let e = c.call("cron.add", water(json!({"kind": "daily", "time": "09:00", "timing": "exact"}))).await.unwrap_err();
    assert!(e.contains("schedule.timezone required (the user's current timezone is not known yet)"), "{e}");
    let e = c.call("cron.add", water(json!({"kind": "runonce", "at": "2027-01-15T09:00", "timezone": "@user.current", "timing": "exact"}))).await.unwrap_err();
    assert!(e.contains("symbolic timezone (@user.*) is only valid for recurring schedules"), "{e}");
    let mut job = water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}));
    job["body"] = json!("Check mail, then call notify_main_agent.");
    let e = c.call("cron.add", job).await.unwrap_err();
    assert!(e.contains("do not name or call the terminal tools"), "{e}");
    let mut job = water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}));
    job["mode"] = json!("heartbeat");
    assert!(c.call("cron.add", job).await.unwrap_err().contains("mode must be `task`"));
    let e = c.call("cron.add", json!({"id": "x", "enabled": true})).await.unwrap_err();
    assert!(e.contains("canonical cron args (id, enabled, mode, schedule) required"), "{e}");
    c.call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await.unwrap();
    let e = c.call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await.unwrap_err();
    assert!(e.contains("schedule already exists"), "{e}");
    let e = c.call("cron.view", json!({"id": "missing"})).await.unwrap_err();
    assert!(e.contains("schedule does not exist"), "{e}");
}

#[tokio::test]
async fn recurring_job_follows_user_timezone_and_timing() {
    let c = ctx("tz", Wire::Anthropic, vec![], Some("Europe/Moscow")).await;
    // `timing=window`: одно время внутри окна от `time`, своё для машины.
    let r = c.call("cron.add", water(json!({"kind": "daily", "time": "09:00", "timing": "window", "window": "2h"}))).await.unwrap();
    assert_eq!(r["schedule"]["timezone"], "@user.current", "{r}");
    assert_eq!(r["timezone"], "@user.current (Europe/Moscow)");
    assert_eq!((r["schedule"]["timing"].as_str(), r["schedule"]["window"].as_str()), (Some("window"), Some("2h")));
    let time = r["schedule"]["time"].as_str().unwrap().to_string();
    assert!(("09:00".."11:00").contains(&time.as_str()), "{time}");
    // 08:00 UTC = 11:00 в Москве: ближайшее — завтра, в субботу.
    assert!(r["next_run_local"].as_str().unwrap().starts_with("Sat 2027-01-16 "), "{r}");
    assert!(c.home.join(format!("workspace/cron.d/daily/water__daily@{}.md", time.replace(':', ""))).exists());
    // Правка без времени сохраняет выбранное; другое время — только с `timing`.
    let r = c.call("cron.update", json!({"id": "water", "title": "Вода"})).await.unwrap();
    assert_eq!(r["schedule"]["time"], time.as_str());
    let e = c.call("cron.update", json!({"id": "water", "schedule": {"time": "10:00"}})).await.unwrap_err();
    assert!(e.contains("the runtime picked this schedule's time, so changing schedule.time or schedule.at needs schedule.timing"), "{e}");
    let e = c.call("cron.update", json!({"id": "water", "schedule": {"timing": "exact"}})).await.unwrap_err();
    assert!(e.contains("schedule.timing needs schedule.time (the window start) when it changes a schedule's timing"), "{e}");
    let r = c.call("cron.update", json!({"id": "water", "schedule": {"timing": "exact", "time": "10:00"}})).await.unwrap();
    assert_eq!((r["schedule"]["time"].as_str(), r["schedule"].get("window")), (Some("10:00"), None), "{r}");

    // `anytime`: дневное время машины, одно на все задания.
    let any = |id: &str, sched: serde_json::Value| {
        let mut j = water(sched);
        j["id"] = json!(id);
        j
    };
    let a = c.call("cron.add", any("a1", json!({"kind": "daily", "timing": "anytime"}))).await.unwrap();
    let b = c.call("cron.add", any("a2", json!({"kind": "weekly", "dow": ["Mon"], "timing": "anytime"}))).await.unwrap();
    let daily = a["schedule"]["time"].as_str().unwrap().to_string();
    assert_eq!(b["schedule"]["time"], daily.as_str());
    // Разовое `anytime`: `at` задаёт дату; попав в прошлое — на день позже.
    let once = c.call("cron.add", any("a3", json!({"kind": "runonce", "at": "2027-01-15T00:00", "timezone": "UTC", "timing": "anytime"}))).await.unwrap();
    let at = once["schedule"]["at"].as_str().unwrap();
    let day = if daily.as_str() > "08:00" { "2027-01-15" } else { "2027-01-16" };
    assert_eq!(at, format!("{day}T{daily}:00"), "{once}");

    // Ошибки ядра.
    let e = c.call("cron.add", any("e1", json!({"kind": "daily", "time": "09:00"}))).await.unwrap_err();
    assert!(e.contains("schedule.timing required for kind=daily: exact, window, or anytime"), "{e}");
    let e = c.call("cron.add", any("e2", json!({"kind": "daily", "time": "09:00", "flexible_time": true}))).await.unwrap_err();
    assert!(e.contains("schedule.flexible_time is not available; use schedule.timing"), "{e}");
    let e = c.call("cron.add", any("e3", json!({"kind": "daily", "time": "09:00", "timing": "soon"}))).await.unwrap_err();
    assert!(e.contains("schedule.timing must be one of exact|window|anytime"), "{e}");
    let e = c.call("cron.add", any("e4", json!({"kind": "daily", "time": "09:00", "timing": "exact", "window": "1h"}))).await.unwrap_err();
    assert!(e.contains("schedule.window was given without schedule.timing=window"), "{e}");
    let e = c.call("cron.add", any("e5", json!({"kind": "daily", "time": "09:00", "timing": "window", "window": "5m"}))).await.unwrap_err();
    assert!(e.contains("schedule.window must be between 15m and 12h"), "{e}");
    let e = c.call("cron.add", any("e6", json!({"kind": "daily", "time": "09:00", "timing": "window", "window": "1d"}))).await.unwrap_err();
    assert!(e.contains("schedule.window must be minutes or hours, as Nm or Nh"), "{e}");
    let e = c.call("cron.add", any("e7", json!({"kind": "daily", "time": "09:00", "timing": "anytime"}))).await.unwrap_err();
    assert!(e.contains("schedule.time does not apply with schedule.timing=anytime"), "{e}");
    let e = c.call("cron.add", any("e8", json!({"kind": "daily", "timing": "exact"}))).await.unwrap_err();
    assert!(e.contains("schedule.time is required for this schedule kind"), "{e}");
    // Окно по умолчанию — 1h.
    let w = c.call("cron.add", any("w1", json!({"kind": "daily", "time": "09:00", "timing": "window"}))).await.unwrap();
    assert_eq!(w["schedule"]["window"], "1h");
    assert!(("09:00".."10:00").contains(&w["schedule"]["time"].as_str().unwrap()), "{w}");
}

#[tokio::test]
async fn update_keeps_body_and_moves_copy() {
    let c = ctx("update", Wire::Anthropic, vec![], None).await;
    c.call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await.unwrap();
    let r = c.call("cron.update", json!({"id": "water", "schedule": {"every": "30m"}})).await.unwrap();
    assert_eq!(r["next_run_in"], "30m", "{r}");
    assert!(!c.home.join("workspace/cron.d/hourly/water__interval@1h.md").exists());
    let copy = std::fs::read_to_string(c.home.join("workspace/cron.d/minutely/water__interval@30m.md")).unwrap();
    assert!(copy.contains("Remind the owner to drink water."));
    let v = c.call("cron.view", json!({"id": "water"})).await.unwrap();
    assert_eq!(v["body"], "Remind the owner to drink water.");
    assert_eq!(v["title"], "Water");
    let r = c.call("cron.update", json!({"id": "water", "title": ""})).await.unwrap();
    assert!(r["title"].is_null(), "{r}");
    let mutations: Vec<String> = sqlx::query_scalar("select action from scheduler.cron_mutations order by mutation_seq")
        .fetch_all(c.core.store.pool())
        .await
        .unwrap();
    assert_eq!(mutations, ["add", "update", "update"]);
}

#[tokio::test]
async fn manual_run_stop_and_remove() {
    let c = ctx("stop", Wire::Anthropic, vec![], None).await;
    c.call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await.unwrap();
    let r = c.call("cron.run", json!({"id": "water", "reason": "test"})).await.unwrap();
    assert_eq!(r["status"], "queued");
    let st = c.call("cron.status", json!({})).await.unwrap();
    assert_eq!(st, json!({"enabled_jobs": 1, "queued_runs": 1, "running_runs": 0}));
    let s = c.call("cron.stop", json!({"id": "water"})).await.unwrap();
    assert!(s["message"].as_str().unwrap().starts_with("No run was executing"), "{s}");
    let runs = c.runs("water").await;
    assert_eq!(runs[0]["status"], "cancelled");
    assert_eq!(runs[0]["trigger_reason"], "manual: test");
    let s = c.call("cron.stop", json!({"id": "water", "run": runs[0]["run_id"]})).await.unwrap();
    assert!(s["message"].as_str().unwrap().starts_with("This run already has a recorded result"), "{s}");
    c.call("cron.remove", json!({"id": "water"})).await.unwrap();
    assert!(!c.home.join("workspace/cron.d/hourly/water__interval@1h.md").exists());
    assert!(c.call("cron.list", json!({"include_disabled": true})).await.unwrap()["jobs"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn disabled_job_is_not_queued() {
    let c = ctx("disabled", Wire::Anthropic, vec![], None).await;
    let mut job = water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}));
    job["enabled"] = json!(false);
    c.call("cron.add", job).await.unwrap();
    c.clock.advance_secs(7200);
    c.sched.tick().await.unwrap();
    assert!(c.runs("water").await.is_empty());
    assert!(c.call("cron.list", json!({})).await.unwrap()["jobs"].as_array().unwrap().is_empty());
    let st = c.call("cron.status", json!({"id": "water"})).await.unwrap();
    assert_eq!(st["state"], "Disabled; it will not run until it is enabled again.");
}

#[tokio::test]
async fn orphaned_running_run_is_closed_on_recovery() {
    let c = ctx("recover", Wire::Anthropic, vec![], None).await;
    c.call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await.unwrap();
    let r = c.call("cron.run", json!({"id": "water"})).await.unwrap();
    let run_id = r["run_id"].as_str().unwrap();
    assert!(c.core.store.start_run(run_id, t0()).await.unwrap());
    c.sched.recover().await.unwrap();
    c.sched.tick().await.unwrap();
    assert_eq!(c.runs("water").await[0]["status"], "failed");
    assert!(c.handoffs()[0].contains("Stopped because the agent restarted mid-run"));
}

/// Долг П4д: сверка памяти с файлами идёт на такте службы.
#[tokio::test]
async fn memory_maintenance_runs_on_scheduler_tick() {
    for wire in WIRES {
        let mut c = ctx("memory", wire, vec![], None).await;
        std::fs::write(c.home.join("MEMORY.md"), "# Memory\n\nThe cat is called Pushok.\n").unwrap();
        let mstore = agent_memory::Store::connect(&pg::database()).await.unwrap();
        let m = agent_memory::MemoryTools::new(&c.home, mstore, None);
        let mut t = Tools::new();
        m.add_to(&mut t);
        let search = || async {
            let parts = t.get("hamlet.memory_search").unwrap().call(json!({"queries": ["Pushok"]})).await.unwrap();
            match &parts[0] {
                ToolResultPart::Text { text } => text.clone(),
                _ => panic!(),
            }
        };
        assert!(!search().await.contains("MEMORY.md"), "{wire:?}: до такта база не сверена");
        let mm = m.clone();
        c.sched.background(Arc::new(move || {
            let mm = mm.clone();
            Box::pin(async move { mm.maintain().await.map(|_| None) })
        }));
        c.sched.tick().await.unwrap();
        assert!(search().await.contains("MEMORY.md"), "{wire:?}");
    }
}

/// П4ж-2: фаза итога — на своей модели (`CronResolve`).
#[tokio::test]
async fn resolution_phase_runs_on_resolve_model() {
    for wire in WIRES {
        let mut c = ctx("resolve", wire, vec![Step::Text("Checked.")], None).await;
        let (resolve_server, rp) = model(wire, vec![quiet("q1")], true);
        c.sched.resolver(Arc::new(rp));
        c.call("cron.add", water(json!({"kind": "runonce", "at": "2027-01-15T08:00", "timezone": "UTC", "timing": "exact"}))).await.unwrap();
        c.sched.tick().await.unwrap();
        c.sched.settle().await.unwrap();
        assert_eq!(c.server.requests().len(), 1, "{wire:?}: исполнение — на модели работника");
        let r = resolve_server.requests();
        assert_eq!(r.len(), 1, "итог — на модели CronResolve");
        assert!(r[0].to_string().contains("Call exactly one final tool"));
        assert_eq!(c.runs("water").await[0]["status"], "succeeded");
    }
}

#[test]
fn plan_picks_worker_and_resolve_models() {
    let base = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../model/tests/fixtures/registry_base.toml")).unwrap();
    let from = "  { purpose = \"BrowserAgent\", model = \"anthropic/main\" },";
    let text = base.replacen(from, &format!("{from}\n  {{ purpose = \"CronResolve\", model = \"anthropic/small\" }},"), 1);
    let reg = agent_model::Registry::parse(&text).unwrap();
    let p = agent_cron::Plan::from_registry(&reg, Some("local/chat")).unwrap();
    // CronWorker — в default_model_purposes у local/chat; CronResolve — переопределён.
    assert_eq!(p.worker.option.id, "anthropic/main");
    assert_eq!(p.resolve.option.id, "anthropic/small");
    let p = agent_cron::Plan::from_registry(&reg, None).unwrap();
    assert_eq!(p.resolve.option.id, "anthropic/main");
}

/// П4л-8: ровного такта нет — служба спит до ближайшего срока (часы
/// службы), а правка расписания будит её сразу.
async fn service_sleeps_until_due_and_wakes_on_change(wire: Wire) {
    let steps = vec![Step::Text("Reminder."), notify("n1", "succeeded", "Drink water."), Step::Text("Again."), notify("n2", "succeeded", "Drink water now.")];
    let Ctx { clock, core, sched, inbox, server: _server, .. } = ctx(&format!("wake-{wire:?}"), wire, steps, None).await;
    let call = |name: &str, args: Value| {
        let t = tools(&core).into_iter().find(|t| t.spec().name == name).unwrap();
        async move { t.call(args).await.unwrap() }
    };
    call("cron.add", water(json!({"kind": "interval", "every": "1h", "timezone": "UTC"}))).await;
    let sched = Arc::new(sched);
    let s = sched.clone();
    let service = tokio::spawn(async move { s.run_forever().await });
    let wait_inbox = |what: &'static str| {
        let inbox = inbox.clone();
        async move {
            for _ in 0..200 {
                let got: Vec<String> = inbox.drain().iter().map(text_of).collect();
                if let Some(t) = got.into_iter().find(|t| t.contains(what)) {
                    return t;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            panic!("нет вести «{what}»");
        }
    };
    // До срока прогона нет.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // Ближайший срок — пульс (30m), который служба заводит при старте.
    assert_eq!(core.store.next_due().await.unwrap(), Some(t0() + 1_800_000));
    assert!(core.store.queued().await.unwrap().is_empty() && core.store.running().await.unwrap().is_empty());
    // Часы дошли до срока — служба просыпается сама.
    clock.advance_secs(3600);
    assert!(wait_inbox("Drink water.").await.contains("[SCHEDULED TASK HANDOFF]"));
    // `cron.run` будит службу без хода часов.
    call("cron.run", json!({"id": "water", "reason": "now"})).await;
    wait_inbox("Drink water now.").await;
    service.abort();
}

#[tokio::test]
async fn service_sleeps_until_due_and_wakes_on_change_anthropic() {
    service_sleeps_until_due_and_wakes_on_change(Wire::Anthropic).await
}

#[tokio::test]
async fn service_sleeps_until_due_and_wakes_on_change_openai_compat() {
    service_sleeps_until_due_and_wakes_on_change(Wire::OpenaiCompat).await
}

/// П3ж-2: успешный отчёт расписания проверяется перед итогом. Покупка —
/// исправленный отчёт уходит главному агенту и в итог прогона; сбой
/// проверки — отчёт с пометкой `unverified_handoff`; `none` — отчёт как
/// есть, проверяющий не зовётся. Без проверки в `launch` — красный.
#[tokio::test]
async fn cron_report_is_verified_before_handoff() {
    use agent_cron::verification::{CronVerification, Verifier};
    for wire in WIRES {
        let cases: [(&str, Vec<Step>, &str); 3] = [
            (
                "purchases",
                vec![
                    Step::Calls(vec![("v1", "hamlet.submit_cron_verification", json!({"category": "purchases"}))]),
                    Step::Text("Цена на сайте продавца — 129 €, скидки нет."),
                    notify("v2", "succeeded", "Наушники стоят 129 €, скидка закончилась."),
                ],
                "<job_result>\nНаушники стоят 129 €, скидка закончилась.\n</job_result>",
            ),
            (
                "unfinished",
                vec![Step::Calls(vec![("v1", "hamlet.submit_cron_verification", json!({"category": "purchases"}))]), Step::Status(400), Step::Status(400)],
                "<job_result>\nНаушники подешевели до 99 €.\n\nVerification did not finish. Present consequential factual claims in this\nreport as unverified.\n</job_result>",
            ),
            (
                "none",
                vec![Step::Calls(vec![("v1", "hamlet.submit_cron_verification", json!({"category": "none"}))])],
                "<job_result>\nНаушники подешевели до 99 €.\n</job_result>",
            ),
        ];
        for (name, steps, expected) in cases {
            let mut c = ctx(&format!("verify-{name}"), wire, vec![Step::Text("Смотрю цену."), notify("n1", "succeeded", "Наушники подешевели до 99 €.")], None).await;
            let (vserver, vp) = model(wire, steps, true);
            let vp = Arc::new(vp);
            let clock = c.clock.clone();
            c.sched.verification(CronVerification {
                classifier: vp.clone(),
                verifier: Verifier {
                    store: c.core.store.clone(),
                    provider: vp,
                    tools: Arc::new(Tools::new),
                    system: Arc::new(|| "verifier system".to_string()),
                    spill_dir: c.home.join("spill"),
                    now: Arc::new(move || agent_cron::Clock::now_ms(&clock)),
                },
            });
            let job = json!({"id": "price", "enabled": true, "mode": "task", "title": "Price", "body": "Check the headphone price.", "schedule": {"kind": "interval", "every": "1h", "timezone": "UTC"}});
            c.call("cron.add", job).await.unwrap();
            c.clock.advance_secs(3600);
            c.sched.tick().await.unwrap();
            c.sched.settle().await.unwrap();
            c.sched.tick().await.unwrap();

            let h = c.handoffs();
            assert_eq!(h.len(), 1, "{wire:?} {name}: {h:?}");
            assert!(h[0].contains(expected), "{wire:?} {name}: {}", h[0]);
            let reqs = vserver.requests();
            let first = reqs[0].to_string();
            assert!(first.contains("Classify the cron report by calling hamlet.submit_cron_verification once."), "{first}");
            assert!(first.contains("<task>\\nCheck the headphone price.\\n</task>") && first.contains("<report>\\nНаушники подешевели до 99 €.\\n</report>"), "{first}");
            assert_eq!(tool_names(&reqs[0]), ["hamlet.submit_cron_verification"]);
            match name {
                "purchases" => {
                    assert_eq!(reqs.len(), 3);
                    let exec = reqs[1].to_string();
                    assert!(exec.contains("<verification-draft>\\nНаушники подешевели до 99 €.") && exec.contains("Verify claims that affect the purchase decision"), "{exec}");
                    assert_eq!(c.runs("price").await[0]["result_summary"], "Наушники стоят 129 €, скидка закончилась.");
                }
                "unfinished" => assert_eq!(reqs.len(), 3),
                _ => assert_eq!(reqs.len(), 1),
            }
        }
    }
}
