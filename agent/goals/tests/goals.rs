//! Тесты П4ж: цели. Postgres — временный кластер на 127.0.0.1, модели —
//! записанные ответы на обоих проводах; живых вызовов нет.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::path::PathBuf;
use std::sync::Arc;

use agent_goals::{tools, Goals, Models, Plan, Store};
use agent_loop::{Tool, Tools, Turn};
use agent_model::{Message, Part, Role, ToolResultPart};
use common::{model, temp_dir, AnyProvider, Server, Step, Wire};
use serde_json::{json, Value};

const WIRES: [Wire; 2] = [Wire::Anthropic, Wire::OpenaiCompat];

async fn goals(name: &str, provider: Option<AnyProvider>) -> (Arc<Goals<AnyProvider>>, PathBuf) {
    let home = temp_dir(&format!("goals-{name}"));
    let store = Store::connect(&pg::database(), Arc::new(|| 1_800_000_000_000)).await.unwrap();
    let models = provider.map(|p| {
        let p = Arc::new(p);
        Models { activity_state: p.clone(), subgoal_rollup: p.clone(), lifecycle: p.clone(), attention: p }
    });
    (Goals::new(store, home.clone(), models, None), home)
}

/// Цели с отдельной моделью на каждое назначение.
async fn goals_with(name: &str, models: Models<AnyProvider>) -> Arc<Goals<AnyProvider>> {
    let store = Store::connect(&pg::database(), Arc::new(|| 1_800_000_000_000)).await.unwrap();
    Goals::new(store, temp_dir(&format!("goals-{name}")), Some(models), None)
}

fn idle(wire: Wire) -> (Server, Arc<AnyProvider>) {
    let (s, p) = model(wire, vec![Step::Text("—")], true);
    (s, Arc::new(p))
}

async fn call(g: &Arc<Goals<AnyProvider>>, name: &str, args: Value) -> Result<Value, String> {
    let t = tools(g).into_iter().find(|t| t.spec().name == name).unwrap_or_else(|| panic!("нет {name}"));
    match t.call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

fn results(h: &[Message]) -> Vec<Value> {
    h.iter()
        .flat_map(|m| &m.content)
        .filter_map(|p| match p {
            Part::ToolResult { content, is_error: false, .. } => match &content[0] {
                ToolResultPart::Text { text } => serde_json::from_str(text).ok(),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

async fn turn(g: &Arc<Goals<AnyProvider>>, wire: Wire, steps: Vec<Step>, home: &std::path::Path) -> (Server, Vec<Message>) {
    let (server, provider) = model(wire, steps, true);
    let mut t = Tools::new();
    for tool in tools(g) {
        t.add(tool);
    }
    let turn = Turn::new(&provider, &t, "main", home.join("spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "цели".into() }] }];
    turn.run(&mut h, &mut |_| {}).await.unwrap();
    (server, h)
}

#[tokio::test]
async fn goal_with_subgoal_and_entries_through_turn() {
    for wire in WIRES {
        // Модель для решений после записи: pulse, затем перенос на родителя.
        let (_decide_server, decide) = model(
            wire,
            vec![
                Step::Calls(vec![("d1", "hamlet.replace_current_state", json!({"current_state": "First 20 km run done.", "attention": "new_information"}))]),
                Step::Calls(vec![(
                    "d2",
                    "hamlet.log_parent_activity",
                    json!({"title": "Long-run base growing", "current_state": "20 km long run logged.", "description": "The long-runs sub-goal logged a first 20 km run.", "parent_current_state": "Base building is on track."}),
                )]),
            ],
            true,
        );
        let (g, home) = goals("tree", Some(decide)).await;
        let parent = call(&g, "user_goal.create", json!({"title": "Marathon under four hours", "description": "Run a marathon in under four hours.", "category": "health", "goal_kind": "outcome"})).await.unwrap();
        let pid = parent["goal_id"].as_str().unwrap().to_string();
        assert!(pid.starts_with("goal_") && pid.len() == 17, "{pid}");
        assert_eq!(parent["slug"], "marathon-under-four-hours");

        let (server, h) = turn(&g, wire, vec![Step::Calls(vec![("c1", "user_goal.create", json!({"title": "Long runs", "parent_goal_id": pid}))]), Step::Text("Готово")], &home).await;
        let child = &results(&h)[0];
        let cid = child["goal_id"].as_str().unwrap().to_string();
        assert_eq!(child["parent_goal_id"], pid.as_str(), "{wire:?}");
        // Описание пространства — при загрузке пространства, не в `create`.
        assert!(!server.requests()[0].to_string().contains("Durable goals the user is working toward."));
        assert!(agent_goals::namespace_description(agent_goals::Ns::UserGoal).contains("Durable goals the user is working toward."));

        let (_s, h) = turn(
            &g,
            wire,
            vec![Step::Calls(vec![("e1", "user_goal.create_entry", json!({"goal_id": cid, "title": "First 20 km", "current_state": "Ran 20 km.", "description": "Ran 20 km at easy pace.", "effective_at": "2027-01-14"}))]), Step::Text("Записал")],
            &home,
        )
        .await;
        let r = &results(&h)[0];
        assert_eq!(r["current_state"], "First 20 km run done.", "{wire:?}: {r}");
        assert_eq!(r["attention_kind"], "new_information");
        assert_eq!(r["parent_entry"]["author_source"], "subgoal_rollup");

        let p = call(&g, "user_goal.get", json!({"goal_id": pid})).await.unwrap();
        assert_eq!(p["current_state"], "Base building is on track.");
        assert_eq!(p["updates"][0]["title"], "Long-run base growing");
        assert_eq!(p["children"][0]["goal_id"], cid.as_str());
        let c = call(&g, "user_goal.get", json!({"goal_id": cid})).await.unwrap();
        assert_eq!(c["updates"][0]["effective_at"], "2027-01-14T00:00:00.000Z");

        let guide = std::fs::read_to_string(home.join("workspace/goals/marathon-under-four-hours/GOAL.md")).unwrap();
        assert!(guide.starts_with("# Marathon under four hours\n") && guide.contains(&format!("Goal ID: {pid}")) && guide.contains("Run a marathon in under four hours."), "{guide}");
        assert!(home.join("workspace/goals/long-runs/files").is_dir() && home.join("workspace/goals/long-runs/hidden_files").is_dir());
    }
}

#[tokio::test]
async fn without_model_entry_keeps_state_and_copies_to_parent() {
    let (g, _home) = goals("nomodel", None).await;
    let p = call(&g, "user_goal.create", json!({"title": "Learn Spanish", "current_state": "Just started."})).await.unwrap();
    let c = call(&g, "user_goal.create", json!({"title": "Daily vocabulary", "parent_goal_id": p["goal_id"]})).await.unwrap();
    let r = call(&g, "user_goal.create_entry", json!({"goal_id": c["goal_id"], "title": "100 words", "current_state": "Learned 100 words."})).await.unwrap();
    assert!(r["current_state"].is_null(), "{r}");
    let pg = call(&g, "user_goal.get", json!({"goal_id": p["goal_id"]})).await.unwrap();
    assert_eq!(pg["updates"][0]["title"], "100 words", "без модели запись подцели переносится как есть");
    assert_eq!(pg["updates"][0]["author_source"], "subgoal_rollup");
    assert_eq!(pg["current_state"], "Just started.");
}

#[tokio::test]
async fn hierarchy_namespaces_and_errors() {
    let (g, _home) = goals("errors", None).await;
    let top = call(&g, "user_goal.create", json!({"title": "Career change"})).await.unwrap();
    let sub = call(&g, "user_goal.create", json!({"title": "Portfolio", "parent_goal_id": top["goal_id"]})).await.unwrap();
    let e = call(&g, "user_goal.create", json!({"title": "Deeper", "parent_goal_id": sub["goal_id"]})).await.unwrap_err();
    assert!(e.contains("top-level"), "{e}");
    let other = call(&g, "user_goal.create", json!({"title": "Other"})).await.unwrap();
    let e = call(&g, "user_goal.update", json!({"goal_id": top["goal_id"], "parent_goal_id": other["goal_id"]})).await.unwrap_err();
    assert!(e.contains("already has child goals and cannot be assigned a parent"), "{e}");
    let e = call(&g, "user_goal.update", json!({"goal_id": other["goal_id"], "parent_goal_id": other["goal_id"]})).await.unwrap_err();
    assert_eq!(e, "goal cannot be its own parent");
    let e = call(&g, "user_goal.update", json!({"goal_id": other["goal_id"], "status": "completed"})).await.unwrap_err();
    assert!(e.contains("status is not editable here"), "{e}");

    // Дело под целью владельца — нельзя, цель уже представляет эту работу.
    let e = call(&g, "tracking.create", json!({"title": "Check-ins", "parent_goal_id": top["goal_id"]})).await.unwrap_err();
    assert!(e.contains("is a user goal and already represents this responsibility"), "{e}");
    let e = call(&g, "tracking.create", json!({"title": "Pizza delivery", "category": "money"})).await.unwrap_err();
    assert_eq!(e, "category applies to user goals only; tracked work items do not carry one");
    let trip = call(&g, "tracking.create", json!({"title": "Trip to Rome"})).await.unwrap();
    assert_eq!(trip["source"], "assistant_tracking");
    let e = call(&g, "tracking.update", json!({"goal_id": top["goal_id"], "title": "x"})).await.unwrap_err();
    assert_eq!(e, "Private tracking tools can only change tracking records");
    let e = call(&g, "user_goal.get", json!({"goal_id": "goal_000000000000"})).await.unwrap_err();
    assert!(e.starts_with("goal_id goal_000000000000 does not exist."), "{e}");
    assert!(call(&g, "user_goal.list", json!({})).await.unwrap()["user_goals"].as_array().unwrap().len() == 3);
    assert_eq!(call(&g, "tracking.list", json!({})).await.unwrap()["trackings"].as_array().unwrap().len(), 1);

    // Поиск.
    let e = call(&g, "user_goal.search", json!({"query": "a"})).await.unwrap_err();
    assert!(e.contains("use user_goal.list to browse"), "{e}");
    let s = call(&g, "user_goal.search", json!({"query": "career portfolio"})).await.unwrap();
    assert_eq!(s["match_count"], 2, "{s}");

    // Жизненный цикл.
    let r = call(&g, "user_goal.set_status", json!({"goal_id": other["goal_id"], "status": "completed", "reason": "Done."})).await.unwrap();
    assert_eq!(r["changed"], true);
    assert_eq!(r["entry"]["description"], "Done.");
    assert_eq!(r["entry"]["title"], "Goal completed", "заголовок ядра");
    let r = call(&g, "user_goal.set_status", json!({"goal_id": other["goal_id"], "status": "completed", "reason": "Again."})).await.unwrap();
    assert_eq!(r["changed"], false);
    let e = call(&g, "user_goal.set_status", json!({"goal_id": other["goal_id"], "status": "paused", "reason": "x"})).await.unwrap_err();
    assert_eq!(e, "must be one of `active` or `completed`");

    // Записи.
    let en = call(&g, "user_goal.create_entry", json!({"goal_id": top["goal_id"], "title": "Interview"})).await.unwrap();
    let uid = en["entry"]["update_id"].as_str().unwrap().to_string();
    assert!(call(&g, "user_goal.update_entry", json!({"update_id": uid})).await.unwrap_err().contains("at least one field must be provided for edit"));
    let ed = call(&g, "user_goal.update_entry", json!({"update_id": uid, "title": "Second interview"})).await.unwrap();
    assert_eq!(ed["title"], "Second interview");
    call(&g, "user_goal.delete_entry", json!({"update_id": uid})).await.unwrap();
    assert!(call(&g, "user_goal.delete_entry", json!({"update_id": uid})).await.unwrap_err().starts_with("goal update not found: "));
}

#[tokio::test]
async fn delete_removes_subgoals_and_keeps_requested_documents() {
    let (g, home) = goals("delete", None).await;
    let top = call(&g, "user_goal.create", json!({"title": "Garden"})).await.unwrap();
    let sub = call(&g, "user_goal.create", json!({"title": "Tomatoes", "parent_goal_id": top["goal_id"]})).await.unwrap();
    call(&g, "user_goal.create_entry", json!({"goal_id": sub["goal_id"], "title": "Planted"})).await.unwrap();
    std::fs::write(home.join("workspace/goals/garden/files/plan.md"), "plan").unwrap();
    std::fs::write(home.join("workspace/goals/garden/hidden_files/log.txt"), "log").unwrap();
    let b = g.store.add_briefing(top["goal_id"].as_str().unwrap(), "workspace/goals/garden/briefs/b1.html").await.unwrap();
    let r = call(&g, "user_goal.delete", json!({"goal_id": top["goal_id"]})).await.unwrap();
    assert_eq!(r["deleted_goal_ids"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(r["preserved_files_paths"], json!(["~/workspace/goals/garden/files"]));
    assert!(home.join("workspace/goals/garden/files/plan.md").exists());
    assert!(!home.join("workspace/goals/garden/hidden_files").exists() && !home.join("workspace/goals/garden/GOAL.md").exists());
    assert!(!home.join("workspace/goals/tomatoes").exists());
    assert!(g.store.goal(sub["goal_id"].as_str().unwrap()).await.unwrap().is_none());
    assert!(g.store.briefings(&b.goal_id).await.unwrap().is_empty());
}

/// Модели решений — через `Registry::resolve` по назначениям
/// `GoalActivityState`, `GoalSubgoalRollup`, `TrackingGoalLifecycle` и
/// `GoalAttention`, с переопределением выбранной модели.
#[test]
fn decision_models_come_from_registry_purposes() {
    let base = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../model/tests/fixtures/registry_base.toml")).unwrap();
    let from = "  { purpose = \"BrowserAgent\", model = \"anthropic/main\" },";
    assert!(base.contains(from));
    let text = base.replacen(from, &format!("{from}\n  {{ purpose = \"GoalActivityState\", model = \"anthropic/small\" }},\n  {{ purpose = \"GoalAttention\", model = \"anthropic/small\" }},"), 1);
    let reg = agent_model::Registry::parse(&text).unwrap();
    let ids = |sel| {
        let plan = Plan::from_registry(&reg, sel).unwrap();
        let m = Models::from_plan(&plan, |r| Ok::<_, ()>(r.option.id.clone())).unwrap();
        [(*m.activity_state).clone(), (*m.subgoal_rollup).clone(), (*m.lifecycle).clone(), (*m.attention).clone()]
    };
    assert_eq!(ids(None), ["anthropic/main", "anthropic/main", "anthropic/main", "anthropic/main"].map(String::from));
    assert_eq!(ids(Some("local/chat")), ["anthropic/small", "local/chat", "local/chat", "anthropic/small"].map(String::from));
}

/// Смена статуса дела: состояние и заголовок записи пишет модель
/// `TrackingGoalLifecycle` по своему системному тексту; модель не ответила
/// инструментом — заголовки ядра, состояние прежнее.
#[tokio::test]
async fn tracking_lifecycle_copy_comes_from_model() {
    for wire in WIRES {
        let (life_server, life) = model(
            wire,
            vec![
                Step::Calls(vec![("l1", "hamlet.set_tracking_goal_lifecycle_copy", json!({"current_state": "The visa was approved on January 14.", "activity_title": "Visa approved"}))]),
                Step::Calls(vec![("l2", "hamlet.set_tracking_goal_lifecycle_copy", json!({"current_state": "Waiting for the embassy to return the passport.", "activity_title": "Tracking resumed for passport"}))]),
                Step::Text("no tool"),
            ],
            true,
        );
        let (_a, attention) = idle(wire);
        let (_s, other) = idle(wire);
        let g = goals_with("lifecycle", Models { activity_state: other.clone(), subgoal_rollup: other, lifecycle: Arc::new(life), attention }).await;
        let t = call(&g, "tracking.create", json!({"title": "Visa application", "current_state": "Submitted."})).await.unwrap();
        let id = t["goal_id"].as_str().unwrap().to_string();
        g.settle().await;

        let r = call(&g, "tracking.set_status", json!({"goal_id": id, "status": "completed", "reason": "Embassy approved the visa."})).await.unwrap();
        assert_eq!(r["entry"]["title"], "Visa approved", "{wire:?}: {r}");
        assert_eq!(r["entry"]["current_state"], "The visa was approved on January 14.");
        assert_eq!(r["entry"]["description"], "Embassy approved the visa.");
        let got = call(&g, "tracking.get", json!({"goal_id": id})).await.unwrap();
        assert_eq!(got["current_state"], "The visa was approved on January 14.", "{got}");
        let req = life_server.requests()[0].to_string();
        assert!(req.contains("assistant-tracking goal is\\ncompleted") || req.contains("assistant-tracking goal is\ncompleted"), "{req}");
        assert!(req.contains("lifecycle_reason") && req.contains("Embassy approved the visa.") && req.contains("status_before_transition"), "{req}");

        let r = call(&g, "tracking.set_status", json!({"goal_id": id, "status": "active", "reason": "Passport still at the embassy."})).await.unwrap();
        assert_eq!(r["entry"]["title"], "Tracking resumed for passport", "{r}");
        assert!(life_server.requests()[1].to_string().contains("reopened"), "текст возобновления");

        // Модель не вызвала инструмент — запасной заголовок ядра.
        let r = call(&g, "tracking.set_status", json!({"goal_id": id, "status": "completed", "reason": "Passport returned."})).await.unwrap();
        assert_eq!(r["entry"]["title"], "Tracking completed", "{r}");
        assert!(r["entry"]["current_state"].is_null());
    }
}

/// Внимание дела пересчитывается в фоне после создания и записи моделью
/// `GoalAttention`: внимание, срок, эскалация и её снятие; сбой модели —
/// внимание прежнее.
#[tokio::test]
async fn attention_refresh_sets_attention_due_and_escalation() {
    for wire in WIRES {
        let esc = json!({"condition": "No reply from the landlord", "due_at": "2027-01-20T09:00:00Z", "action": "notify"});
        let (att_server, attention) = model(
            wire,
            vec![
                Step::Calls(vec![("a1", "hamlet.set_attention", json!({"attention": "new_information"}))]),
                Step::Calls(vec![("a2", "hamlet.set_attention", json!({"attention": "user_action_required", "evidence_dates": ["2027-01-20 - landlord reply"], "next_due_at": "2027-01-20", "escalation": esc}))]),
                Step::Calls(vec![("a3", "hamlet.set_attention", json!({"attention": "does_not_need_attention", "clear_escalation": true}))]),
                Step::Text("no tool"),
            ],
            true,
        );
        let (_p, pulse) = model(wire, vec![Step::Calls(vec![("n", "hamlet.no_new_state", json!({}))]); 3], true);
        let pulse = Arc::new(pulse);
        let (_s, other) = idle(wire);
        let g = goals_with("attention", Models { activity_state: pulse, subgoal_rollup: other.clone(), lifecycle: other, attention: Arc::new(attention) }).await;
        let t = call(&g, "tracking.create", json!({"title": "Flat deposit"})).await.unwrap();
        let id = t["goal_id"].as_str().unwrap().to_string();
        g.settle().await;
        let goal = g.store.goal(&id).await.unwrap().unwrap();
        assert_eq!(goal.attention_kind, "new_information", "{wire:?}: создание — новость");
        let first = att_server.requests()[0].to_string();
        assert!(first.contains("goal_created") && first.contains("current_attention") && first.contains("Friday, 2027-01-15T08:00:00Z"), "{first}");
        assert!(first.contains("hamlet.set_attention") && first.contains("clear_escalation"), "{first}");

        call(&g, "tracking.create_entry", json!({"goal_id": id, "title": "Asked for deposit", "description": "If the landlord does not reply by Wednesday, tell me."})).await.unwrap();
        g.settle().await;
        let goal = g.store.goal(&id).await.unwrap().unwrap();
        assert_eq!(goal.attention_kind, "user_action_required");
        assert_eq!(goal.next_due_at.as_deref(), Some("2027-01-20T00:00:00.000Z"));
        assert_eq!(goal.escalation, Some(esc.clone()));
        assert!(att_server.requests()[1].to_string().contains("Asked for deposit"), "событие — запись ленты");

        call(&g, "tracking.create_entry", json!({"goal_id": id, "title": "Landlord replied"})).await.unwrap();
        g.settle().await;
        let goal = g.store.goal(&id).await.unwrap().unwrap();
        assert_eq!(goal.attention_kind, "does_not_need_attention");
        assert!(goal.escalation.is_none(), "clear_escalation снимает эскалацию");

        call(&g, "tracking.create_entry", json!({"goal_id": id, "title": "Deposit back"})).await.unwrap();
        g.settle().await;
        assert_eq!(g.store.goal(&id).await.unwrap().unwrap().attention_kind, "does_not_need_attention", "сбой — внимание прежнее");
    }
}

/// `escalation` у `update`: проверка, запись, снятие `null`, снятие при
/// завершении.
#[tokio::test]
async fn update_escalation_and_clear_on_completion() {
    let (g, _home) = goals("escalation", None).await;
    let t = call(&g, "user_goal.create", json!({"title": "Apartment hunt"})).await.unwrap();
    let id = t["goal_id"].as_str().unwrap().to_string();
    let e = call(&g, "user_goal.update", json!({"goal_id": id, "escalation": {"condition": "No offer", "due_at": "Friday", "action": "ask"}})).await.unwrap_err();
    assert_eq!(e, "goal escalation due_at is not a valid RFC 3339 timestamp");
    let e = call(&g, "user_goal.update", json!({"goal_id": id, "escalation": {"condition": "", "due_at": "2027-02-01T00:00:00Z", "action": "ask"}})).await.unwrap_err();
    assert_eq!(e, "invalid stored goal escalation: condition and due_at must be non-empty");
    let esc = json!({"condition": "No offer by February", "due_at": "2027-02-01T00:00:00Z", "action": "ask"});
    let r = call(&g, "user_goal.update", json!({"goal_id": id, "escalation": esc})).await.unwrap();
    assert_eq!(r["escalation"], esc);
    let r = call(&g, "user_goal.update", json!({"goal_id": id, "title": "Apartment search"})).await.unwrap();
    assert_eq!(r["escalation"], esc, "нет поля — не трогать");
    let r = call(&g, "user_goal.update", json!({"goal_id": id, "escalation": null})).await.unwrap();
    assert!(r["escalation"].is_null());
    call(&g, "user_goal.update", json!({"goal_id": id, "escalation": esc})).await.unwrap();
    assert_eq!(g.store.due_escalations(0, 2_000_000_000_000).await.unwrap().len(), 1);
    call(&g, "user_goal.set_status", json!({"goal_id": id, "status": "completed", "reason": "Signed a lease."})).await.unwrap();
    assert!(g.store.goal(&id).await.unwrap().unwrap().escalation.is_none(), "завершение снимает эскалацию");
    assert!(g.store.due_escalations(0, 2_000_000_000_000).await.unwrap().is_empty());
}
