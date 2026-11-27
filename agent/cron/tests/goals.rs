//! Тесты П4ж: расписание, принадлежащее цели. Часы поддельные, Postgres —
//! временный кластер на 127.0.0.1, модели — записанные ответы на обоих
//! проводах.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::sync::Arc;

use agent_cron::{tools, Core, FakeClock, Scheduler, Store};
use agent_goals::{Goals, OwnedCleanup};
use agent_loop::{Inbox, Tool};
use agent_model::ToolResultPart;
use chrono::TimeZone;
use common::{model, temp_dir, AnyProvider, Step, Wire};
use serde_json::{json, Value};

async fn call(t: Vec<Box<dyn Tool>>, name: &str, args: Value) -> Result<Value, String> {
    let t = t.into_iter().find(|t| t.spec().name == name).unwrap();
    match t.call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::test]
async fn goal_owned_schedule_gets_goal_context_and_dies_with_the_goal() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let home = temp_dir("cron-goal");
        let t0 = chrono::Utc.with_ymd_and_hms(2027, 1, 15, 8, 0, 0).unwrap().timestamp_millis();
        let clock = FakeClock::at(t0);
        let c2 = clock.clone();
        let gstore = agent_goals::Store::connect(&pg::database(), Arc::new(move || agent_cron::Clock::now_ms(&c2))).await.unwrap();
        let core = Core::build(Store::connect(&pg::database()).await.unwrap(), Arc::new(clock.clone()), &home, None, vec![], Some(gstore.clone()));
        let goals: Arc<Goals<AnyProvider>> = Goals::new(gstore, home.clone(), None, Some(core.clone() as Arc<dyn OwnedCleanup>));
        let gt = || agent_goals::tools(&goals).into_iter().map(|t| Box::new(t) as Box<dyn Tool>).collect::<Vec<_>>();
        let ct = || tools(&core).into_iter().map(|t| Box::new(t) as Box<dyn Tool>).collect::<Vec<_>>();

        let goal = call(gt(), "user_goal.create", json!({"title": "Marathon", "description": "Run a marathon."})).await.unwrap();
        let job = |owner: &str| json!({"id": "long-run-check", "enabled": true, "mode": "task", "body": "Check this week's long run.", "owner": owner,
            "schedule": {"kind": "runonce", "at": "2027-01-15T08:00", "timezone": "UTC"}});
        let e = call(ct(), "cron.add", job("space:site")).await.unwrap_err();
        assert!(e.contains("owner must be a supported namespaced value"), "{e}");
        let e = call(ct(), "cron.add", job("goal:Marathon")).await.unwrap_err();
        assert!(e.contains("must use the stored goal slug"), "{e}");
        let r = call(ct(), "cron.add", job("goal:marathon")).await.unwrap();
        assert_eq!(r["source_path"], "workspace/goals/marathon/crons/long-run-check__runonce@2027-01-15T08:00:00.md", "{r}");
        assert!(home.join("workspace/goals/marathon/crons/long-run-check__runonce@2027-01-15T08:00:00.md").exists());
        assert_eq!(call(ct(), "cron.list", json!({})).await.unwrap()["jobs"][0]["owner"], "goal:marathon");

        let (server, provider) = model(wire, vec![Step::Text("Long run looks good."), Step::Calls(vec![("q", "hamlet.nothing_to_report", json!({}))])], true);
        let sched = Scheduler::new(core.clone(), Arc::new(provider), "worker", Arc::new(agent_loop::Tools::new()), home.join("spill"), Inbox::new());
        sched.tick().await.unwrap();
        sched.settle().await.unwrap();
        let first = server.requests()[0].to_string();
        assert!(first.contains("[GOAL CONTEXT]\\nThis scheduled task is owned by `goal:marathon`."), "{wire:?}: {first}");
        assert!(first.contains("record it on the owned goal with `user_goal.create_entry`"), "указания для цели владельца");
        assert!(first.contains("[GOAL GUIDE]\\n# Marathon") && first.contains("Run a marathon."), "GOAL.md в контексте");

        // Цель удалена — её расписание тоже.
        let d = call(gt(), "user_goal.delete", json!({"goal_id": goal["goal_id"]})).await.unwrap();
        assert_eq!(d["deleted_schedules"], json!(["long-run-check"]), "{d}");
        assert!(call(ct(), "cron.list", json!({"include_disabled": true})).await.unwrap()["jobs"].as_array().unwrap().is_empty());
        assert!(!home.join("workspace/goals/marathon/crons").exists());
    }
}

/// Будильник эскалаций: созревшая эскалация активной цели — весть
/// главному агенту, один раз; завершённая цель не будит.
#[tokio::test]
async fn due_escalation_wakes_main_agent_once() {
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let home = temp_dir("cron-escalation");
        let t0 = chrono::Utc.with_ymd_and_hms(2027, 1, 15, 8, 0, 0).unwrap().timestamp_millis();
        let clock = FakeClock::at(t0);
        let c2 = clock.clone();
        let gstore = agent_goals::Store::connect(&pg::database(), Arc::new(move || agent_cron::Clock::now_ms(&c2))).await.unwrap();
        let core = Core::build(Store::connect(&pg::database()).await.unwrap(), Arc::new(clock.clone()), &home, None, vec![], Some(gstore.clone()));
        let goals: Arc<Goals<AnyProvider>> = Goals::new(gstore, home.clone(), None, None);
        let gt = || agent_goals::tools(&goals).into_iter().map(|t| Box::new(t) as Box<dyn Tool>).collect::<Vec<_>>();
        let a = call(gt(), "tracking.create", json!({"title": "Landlord deposit"})).await.unwrap();
        call(gt(), "tracking.update", json!({"goal_id": a["goal_id"], "escalation": {"condition": "No reply from the landlord", "due_at": "2027-01-15T09:00:00Z", "action": "ask"}})).await.unwrap();
        let b = call(gt(), "tracking.create", json!({"title": "Refund"})).await.unwrap();
        call(gt(), "tracking.update", json!({"goal_id": b["goal_id"], "escalation": {"condition": "No refund", "due_at": "2027-01-15T08:30:00Z", "action": "notify"}})).await.unwrap();
        call(gt(), "tracking.set_status", json!({"goal_id": b["goal_id"], "status": "completed", "reason": "Refund arrived."})).await.unwrap();

        let (_server, provider) = model(wire, vec![Step::Text("—")], true);
        let inbox = Inbox::new();
        let sched = Scheduler::new(core.clone(), Arc::new(provider), "worker", Arc::new(agent_loop::Tools::new()), home.join("spill"), inbox.clone());
        sched.tick().await.unwrap();
        assert!(inbox.drain().is_empty(), "{wire:?}: срок не наступил");
        clock.set(t0 + 61 * 60 * 1000);
        sched.tick().await.unwrap();
        let notes: Vec<String> = inbox.drain().iter().flat_map(|m| m.content.iter()).filter_map(|p| match p { agent_model::Part::Text { text } => Some(text.clone()), _ => None }).collect();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("[GOAL ESCALATION DUE]") && notes[0].contains("No reply from the landlord") && notes[0].contains("ask the user what to do"), "{}", notes[0]);
        assert!(notes[0].contains(a["goal_id"].as_str().unwrap()));
        sched.tick().await.unwrap();
        assert!(inbox.drain().is_empty(), "одна весть на срок");
    }
}
