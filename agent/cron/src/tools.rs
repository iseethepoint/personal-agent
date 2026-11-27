//! Инструменты `cron.*` — форма muse (`strings-all.txt:85138-85167`,
//! описания полей `CronArgs`, ответы и ошибки — тексты ядра;
//! `prompts/cron/tools.json`, правки — в `prompts/SOURCES.md`).
//!
//! Описание пространства `cron` (`prompts/cron/cron_namespace.md`) у muse
//! отдаёт поиск инструментов (`tool_search`); его у нас нет — описание
//! прикреплено к `cron.add`. Своё.
//!
//! Не делаются (их служб у Hamlet нет): `delivery`, `connection_id`,
//! `execution`, `predicted_connector_permissions`, `confirmation`,
//! `cron.add_artifact_action`; у `owner` — только `goal:<slug>` (`space:` —
//! владельцы-артефакты, их у нас нет).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

use agent_loop::{personal, Tool, ToolError, ToolFuture};
use agent_model::{ToolResultPart, ToolSpec};
use serde_json::{json, Map, Value};

use crate::schedule::{local_hour, local_label, wait_label, Schedule};
use crate::service::{Core, MAX_TIMEOUT_SECS};
use crate::store::{Definition, Job, Metadata, Retry, Run};

const NAMESPACE: &str = include_str!("../../../prompts/cron/cron_namespace.md");
const TEXTS_JSON: &str = include_str!("../../../prompts/cron/tools.json");

static TEXTS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(TEXTS_JSON).expect("prompts/cron/tools.json"));

fn text(group: &str, key: &str) -> String {
    personal(TEXTS[group][key].as_str().unwrap_or_default())
}

fn field(key: &str) -> String {
    text("fields", key)
}

/// Остановка по просьбе владельца — текст главному агенту (ядро, рядом с
/// `stopped by user request`).
pub const STOPPED_BY_USER: &str = "The user asked to stop this scheduled run, and it was stopped before it could finish. Confirm briefly that the run was stopped as requested. Steps it finished before the stop stand; verify committed external effects before assuming anything else happened. The schedule itself is unchanged and will still fire at its next occurrence unless the user disables or removes it.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Status,
    List,
    View,
    Add,
    Update,
    Remove,
    Run,
    Runs,
    Stop,
}

impl Op {
    pub const ALL: [Op; 9] = [Op::Status, Op::List, Op::View, Op::Add, Op::Update, Op::Remove, Op::Run, Op::Runs, Op::Stop];

    fn name(self) -> &'static str {
        match self {
            Op::Status => "cron.status",
            Op::List => "cron.list",
            Op::View => "cron.view",
            Op::Add => "cron.add",
            Op::Update => "cron.update",
            Op::Remove => "cron.remove",
            Op::Run => "cron.run",
            Op::Runs => "cron.runs",
            Op::Stop => "cron.stop",
        }
    }
}

pub struct CronTool {
    op: Op,
    core: Arc<Core>,
}

/// Инструменты `cron.*` над общим ядром службы.
pub fn tools(core: &Arc<Core>) -> Vec<CronTool> {
    Op::ALL.into_iter().map(|op| CronTool { op, core: core.clone() }).collect()
}

fn s(desc: String) -> Value {
    json!({"type": "string", "description": desc})
}

fn schedule_schema() -> Value {
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    json!({
        "type": "object",
        "description": field("schedule"),
        "properties": {
            "kind": {"type": "string", "enum": ["runonce", "interval", "daily", "weekly", "monthly", "yearly"], "description": field("schedule.kind")},
            "timezone": s(field("schedule.timezone")),
            "at": {"type": "string", "pattern": field("schedule.at.pattern"), "description": field("schedule.at")},
            "every": s(field("schedule.every")),
            "time": {"type": "string", "pattern": field("schedule.time.pattern"), "description": field("schedule.time")},
            "flexible_time": {"type": "boolean", "description": field("schedule.flexible_time")},
            "dow": {"type": "array", "items": {"type": "string", "enum": days}, "description": field("schedule.dow")},
            "dom": {"type": "array", "items": {"type": "integer", "minimum": 1, "maximum": 31}, "description": field("schedule.dom")},
            "month": {"type": "array", "items": {"type": "integer", "minimum": 1, "maximum": 12}, "description": field("schedule.month")},
        },
        "additionalProperties": false,
    })
}

fn definition_props(p: &mut Map<String, Value>) {
    p.insert("title".into(), s(field("title")));
    p.insert("enabled".into(), json!({"type": "boolean", "description": field("enabled")}));
    p.insert("mode".into(), json!({"type": "string", "enum": ["task"], "description": field("mode")}));
    p.insert("schedule".into(), schedule_schema());
    p.insert("body".into(), s(field("body")));
    p.insert("prompt".into(), s(field("prompt")));
    p.insert("timeout_secs".into(), json!({"type": "integer", "minimum": 1, "maximum": MAX_TIMEOUT_SECS, "description": field("timeout_secs")}));
    p.insert(
        "retry".into(),
        json!({
            "type": "object",
            "description": field("retry"),
            "properties": {
                "on_failure": {"type": "boolean", "description": field("retry.on_failure")},
                "max_retries": {"type": "integer", "minimum": 0, "description": field("retry.max_retries")},
            },
            "additionalProperties": false,
        }),
    );
    p.insert("notification_requested".into(), json!({"type": "boolean", "description": field("notification_requested")}));
    p.insert("owner".into(), s(field("owner")));
}

impl CronTool {
    fn schema(&self) -> Value {
        let mut p = Map::new();
        let mut required = vec![];
        let id = s(field("id"));
        match self.op {
            Op::Status | Op::Runs => {
                p.insert("id".into(), id);
                if self.op == Op::Runs {
                    p.insert("limit".into(), json!({"type": "integer", "minimum": 1, "description": field("limit")}));
                }
            }
            Op::List => {
                p.insert("include_disabled".into(), json!({"type": "boolean", "description": field("include_disabled")}));
            }
            Op::View | Op::Remove => {
                p.insert("id".into(), id);
                required.push("id");
            }
            Op::Run => {
                p.insert("id".into(), id);
                p.insert("reason".into(), s(field("reason")));
                required.push("id");
            }
            Op::Stop => {
                p.insert("id".into(), id);
                p.insert("run".into(), s(field("run")));
                required.push("id");
            }
            Op::Add => {
                p.insert("id".into(), id);
                definition_props(&mut p);
                required.extend(["id", "enabled", "mode", "schedule"]);
            }
            Op::Update => {
                p.insert("id".into(), id);
                definition_props(&mut p);
                required.push("id");
            }
        }
        json!({"type": "object", "properties": p, "required": required, "additionalProperties": false})
    }
}

impl Tool for CronTool {
    fn spec(&self) -> ToolSpec {
        let mut description = text("tools", self.op.name());
        if self.op == Op::Add {
            description = format!("{description}\n\n{}", personal(NAMESPACE).trim_end());
        }
        ToolSpec { name: self.op.name().into(), description, input_schema: self.schema() }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let out = match self.op {
                Op::Status => self.status(&args).await,
                Op::List => self.list(&args).await,
                Op::View => self.view(&args).await,
                Op::Add => self.add(&args).await,
                Op::Update => self.update(&args).await,
                Op::Remove => self.remove(&args).await,
                Op::Run => self.run(&args).await,
                Op::Runs => self.runs(&args).await,
                Op::Stop => self.stop(&args).await,
            }?;
            let text = serde_json::to_string_pretty(&out).unwrap_or_default();
            Ok(vec![ToolResultPart::Text { text }])
        })
    }
}

fn bad(s: impl Into<String>) -> ToolError {
    ToolError::InvalidArguments(s.into())
}

fn failed(s: impl Into<String>) -> ToolError {
    ToolError::Failed(s.into())
}

fn str_arg<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k).and_then(Value::as_str)
}

fn required_id(args: &Value) -> Result<String, ToolError> {
    let id = str_arg(args, "id").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad("cron job id is required"))?;
    // Id идёт в имя файла копии. Текст ошибки — свой.
    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(bad("cron job id must be kebab-case: lowercase ASCII letters, digits, and hyphens"));
    }
    Ok(id.to_string())
}

/// Отпечаток определения для `jobs.source_hash`.
pub(crate) fn fingerprint(text: &str) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn run_meaning(r: &Run) -> &'static str {
    // Тексты статусов — ядро («Waiting to start.», «Finished. …»).
    match r.status.as_str() {
        "queued" => "Waiting to start.",
        "running" => "A run is in progress now.",
        "succeeded" => "Finished. A finished run does not by itself mean anything reached the user.",
        "cancelled" => "Cancelled before it finished.",
        _ if r.error_text.is_none() => "Failed without a recorded reason.",
        _ => "",
    }
}

fn run_json(r: &Run) -> Value {
    json!({
        "run_id": r.run_id,
        "id": r.job_id,
        "status": r.status,
        "meaning": run_meaning(r),
        "scheduled_for_utc": r.scheduled_for_utc,
        "trigger_reason": r.trigger_reason,
        "started_at_utc": r.started_at_utc,
        "finished_at_utc": r.finished_at_utc,
        "attempt": r.attempt,
        "result_summary": r.result_summary,
        "error_text": r.error_text,
    })
}

impl CronTool {
    fn user_tz(&self) -> Option<&str> {
        self.core.user_tz.as_deref()
    }

    fn next_fields(&self, d: &Definition, next: i64, now: i64) -> Value {
        if next <= 0 {
            return json!({"next_run_local": null, "next_run_in": null});
        }
        json!({
            "next_run_local": local_label(next, &d.schedule.timezone, self.user_tz()),
            "next_run_in": wait_label(next - now),
        })
    }

    fn job_json(&self, j: &Job, now: i64) -> Value {
        let d = &j.definition;
        let mut v = json!({
            "id": j.job_id,
            "title": d.title,
            "enabled": j.enabled,
            "mode": d.mode,
            "schedule": d.schedule,
            "source_path": j.source_path,
            "owner": d.owner,
            "system": d.mode == "heartbeat",
            "last_run_at_utc": j.last_run_at_utc,
            "last_status": j.last_status,
        });
        merge(&mut v, self.next_fields(d, j.next_run_at_utc, now));
        v
    }

    async fn job(&self, id: &str) -> Result<Job, ToolError> {
        self.core.store.job(id).await.map_err(failed)?.ok_or_else(|| failed("schedule does not exist"))
    }

    async fn status(&self, args: &Value) -> Result<Value, ToolError> {
        let now = self.core.now();
        match str_arg(args, "id") {
            None => {
                let (enabled, queued, running) = self.core.store.counts().await.map_err(failed)?;
                Ok(json!({"enabled_jobs": enabled, "queued_runs": queued, "running_runs": running}))
            }
            Some(id) => {
                let j = self.job(id).await?;
                let active = self.core.store.active_runs(id).await.map_err(failed)?;
                let mut v = self.job_json(&j, now);
                v["queued_runs"] = active.iter().filter(|r| r.status == "queued").map(run_json).collect();
                v["running_runs"] = active.iter().filter(|r| r.status == "running").map(run_json).collect();
                if !j.enabled {
                    v["state"] = "Disabled; it will not run until it is enabled again.".into();
                } else if active.iter().any(|r| r.status == "running") {
                    v["state"] = "A run is in progress now.".into();
                }
                Ok(v)
            }
        }
    }

    async fn list(&self, args: &Value) -> Result<Value, ToolError> {
        let now = self.core.now();
        let all = args.get("include_disabled").and_then(Value::as_bool).unwrap_or(false);
        let jobs = self.core.store.jobs(all).await.map_err(failed)?;
        Ok(json!({"jobs": jobs.iter().map(|j| self.job_json(j, now)).collect::<Vec<_>>()}))
    }

    async fn view(&self, args: &Value) -> Result<Value, ToolError> {
        let j = self.job(&required_id(args)?).await?;
        let mut v = self.job_json(&j, self.core.now());
        v["body"] = j.definition.body.clone().into();
        v["timeout_secs"] = j.definition.timeout_secs.into();
        v["retry"] = json!(j.definition.retry);
        v["definition"] = j.definition.render().into();
        Ok(v)
    }

    /// Поля определения из аргументов поверх `prev`.
    fn apply(&self, args: &Value, d: &mut Definition, adding: bool) -> Result<(), ToolError> {
        if let Some(m) = str_arg(args, "mode") {
            if m != "task" {
                return Err(bad("mode must be `task` for add/update; heartbeat mode is runtime-managed"));
            }
        }
        if let Some(t) = str_arg(args, "title") {
            if t.contains('\n') {
                return Err(bad("title must be a single line"));
            }
            if t.chars().count() > 120 {
                return Err(bad("title must be at most 120 characters"));
            }
            d.title = (!t.is_empty()).then(|| t.to_string());
        }
        if let Some(e) = args.get("enabled").and_then(Value::as_bool) {
            d.enabled = e;
        }
        if let Some(b) = str_arg(args, "body").or_else(|| str_arg(args, "prompt")) {
            if ["nothing_to_do", "nothing_to_report", "notify_main_agent"].iter().any(|n| b.contains(n)) {
                return Err(bad("schedule bodies describe execute-phase work and result intent; do not name or call the terminal tools `nothing_to_do`, `nothing_to_report`, or `notify_main_agent`"));
            }
            d.body = b.trim_end().to_string();
        }
        if let Some(t) = args.get("timeout_secs") {
            let t = t.as_i64().filter(|t| (1..=MAX_TIMEOUT_SECS).contains(t)).ok_or_else(|| bad("timeout_secs must be from 1 to 86400 seconds"))?;
            d.timeout_secs = Some(t);
        }
        if let Some(r) = args.get("retry") {
            let r = r.as_object().ok_or_else(|| bad("retry must be a JSON object"))?;
            if let Some(v) = r.get("on_failure").and_then(Value::as_bool) {
                d.retry.on_failure = v;
            }
            if let Some(v) = r.get("max_retries").and_then(Value::as_i64) {
                d.retry.max_retries = v.max(0) as i32;
            }
        }
        if let Some(o) = str_arg(args, "owner") {
            d.owner = (!o.trim().is_empty()).then(|| o.trim().to_string());
        }
        if let Some(n) = args.get("notification_requested").and_then(Value::as_bool) {
            d.metadata.notification_requested = Some(n);
        }
        if let Some(v) = args.get("schedule") {
            d.schedule = Schedule::from_input(v, (!adding).then_some(&d.schedule), self.user_tz(), &d.id).map_err(bad)?;
        }
        Ok(())
    }

    /// Проверка владельца — тексты ядра (`strings-all.txt`, «Ошибки имени
    /// файла и формата расписания»); из текста о неизвестном виде убран
    /// `space:<space-slug>`.
    async fn check_owner(&self, d: &Definition) -> Result<(), ToolError> {
        let Some(owner) = &d.owner else { return Ok(()) };
        let Some(slug) = owner.strip_prefix("goal:") else {
            return Err(bad("owner must be a supported namespaced value; use `goal:<goal-slug>` for goal-owned crons"));
        };
        let well_formed = !slug.is_empty() && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let exists = match (&self.core.goals, well_formed) {
            (Some(g), true) => g.goal_by_slug(slug).await.map_err(failed)?.is_some(),
            _ => false,
        };
        if !exists {
            return Err(bad("owner `goal:<goal-slug>` must use the stored goal slug containing only lowercase ASCII letters, digits, and hyphens; pass the goal slug, not the title"));
        }
        Ok(())
    }

    /// Сохранить: база — правда, файл в `cron.d` — копия.
    async fn save(&self, d: &Definition, next: i64, old_path: Option<&str>, action: &str) -> Result<Value, ToolError> {
        let now = self.core.now();
        let rendered = d.render();
        self.core.store.save(d, &fingerprint(&rendered), next, now).await.map_err(failed)?;
        let path = d.source_path();
        if let Some(old) = old_path.filter(|o| *o != path) {
            let _ = tokio::fs::remove_file(self.core.home.join(old)).await;
        }
        let full = self.core.home.join(&path);
        if let Some(dir) = full.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(|e| failed(format!("cron.d: {e}")))?;
        }
        tokio::fs::write(&full, &rendered).await.map_err(|e| failed(format!("cron.d: {e}")))?;
        self.core.store.record_mutation(&d.id, action, now).await.map_err(failed)?;

        let (timeout, gap) = self.core.effective_timeout(d, now);
        let timeout_note = match gap {
            None if d.schedule.kind == "runonce" => format!("Runtime uses {timeout}s for this one-off execution. One-offs default to 86400s and honor an explicit timeout_secs. Each occurrence keeps the budget assigned when it starts."),
            gap => format!(
                "Runtime uses {timeout}s. Recurring execution allows max(requested/default timeout, time between scheduled runs), capped at 86400s. The next schedule gap is {}s; later calendar gaps may differ. Each occurrence keeps the budget assigned when it starts. Existing concurrency rules still apply.",
                gap.unwrap_or(0)
            ),
        };
        let mut v = json!({
            "id": d.id,
            "title": d.title,
            "enabled": d.enabled,
            "schedule": d.schedule,
            "timezone": if d.schedule.timezone == crate::schedule::USER_CURRENT { format!("{} ({})", d.schedule.timezone, self.user_tz().unwrap_or("UTC")) } else { d.schedule.timezone.clone() },
            "source_path": path,
            "requested_timeout_secs": d.timeout_secs,
            "timeout_secs": timeout,
            "timeout_note": timeout_note,
        });
        merge(&mut v, self.next_fields(d, next, now));
        // «Small hours» — с полуночи до пяти утра владельца; границы своё.
        if d.schedule.kind == "runonce" && next - now > 20 * 3600 * 1000 && local_hour(now, self.user_tz()).is_some_and(|h| h < 5) {
            v["warning"] = text("texts", "small_hours").into();
        }
        Ok(v)
    }

    fn first_run(&self, d: &Definition, now: i64) -> Result<i64, ToolError> {
        if d.schedule.kind == "runonce" {
            // Прошедший `at` даёт немедленный догоняющий прогон.
            let at = d.schedule.next_after(i64::MIN / 2, now, self.user_tz()).map_err(bad)?;
            return Ok(at.unwrap_or(now));
        }
        Ok(d.schedule.next_after(now, d.metadata.created_at_ms, self.user_tz()).map_err(bad)?.unwrap_or(0))
    }

    async fn add(&self, args: &Value) -> Result<Value, ToolError> {
        let id = required_id(args)?;
        if args.get("enabled").is_none() || args.get("mode").is_none() || args.get("schedule").is_none() {
            return Err(bad("canonical cron args (id, enabled, mode, schedule) required"));
        }
        if self.core.store.job(&id).await.map_err(failed)?.is_some() {
            return Err(failed("schedule already exists"));
        }
        let now = self.core.now();
        let mut d = Definition {
            id: id.clone(),
            title: None,
            enabled: true,
            mode: "task".into(),
            owner: None,
            delivery: vec![],
            timeout_secs: None,
            retry: Retry::default(),
            schedule: Schedule::from_input(&args["schedule"], None, self.user_tz(), &id).map_err(bad)?,
            metadata: Metadata { created_by: "chat".into(), notification_requested: None, created_at_ms: now },
            body: String::new(),
        };
        self.apply(args, &mut d, true)?;
        self.check_owner(&d).await?;
        let next = self.first_run(&d, now)?;
        self.save(&d, next, None, "add").await
    }

    async fn update(&self, args: &Value) -> Result<Value, ToolError> {
        let id = required_id(args)?;
        let job = self.job(&id).await?;
        system_guard(&job)?;
        let mut d = job.definition.clone();
        self.apply(args, &mut d, false)?;
        self.check_owner(&d).await?;
        let now = self.core.now();
        let rescheduled = d.schedule != job.definition.schedule || (d.enabled && !job.enabled);
        let next = if rescheduled { self.first_run(&d, now)? } else { job.next_run_at_utc };
        self.save(&d, next, job.source_path.as_deref(), "update").await
    }

    async fn remove(&self, args: &Value) -> Result<Value, ToolError> {
        let id = required_id(args)?;
        let job = self.job(&id).await?;
        system_guard(&job)?;
        self.core.remove_job(&id, "schedule removed").await.map_err(failed)?;
        Ok(json!({"id": id, "removed": true}))
    }

    async fn run(&self, args: &Value) -> Result<Value, ToolError> {
        let id = required_id(args)?;
        let job = self.job(&id).await?;
        let now = self.core.now();
        let reason = str_arg(args, "reason").unwrap_or("");
        let run_id = format!("{id}:manual:{now}");
        let trigger = if reason.is_empty() { "manual".to_string() } else { format!("manual: {reason}") };
        self.core.store.enqueue(&run_id, &job, now, &trigger, 1).await.map_err(failed)?;
        Ok(json!({"id": id, "run_id": run_id, "status": "queued"}))
    }

    async fn runs(&self, args: &Value) -> Result<Value, ToolError> {
        // Своё: без `limit` — 20 строк.
        let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(20).max(1);
        let runs = self.core.store.runs(str_arg(args, "id"), limit).await.map_err(failed)?;
        Ok(json!({"runs": runs.iter().map(run_json).collect::<Vec<_>>(), "note": text("texts", "runs_note")}))
    }

    async fn stop(&self, args: &Value) -> Result<Value, ToolError> {
        let id = required_id(args)?;
        self.job(&id).await?;
        let now = self.core.now();
        let store = &self.core.store;
        let target = match str_arg(args, "run") {
            Some(run) => {
                let r = store.run(run).await.map_err(failed)?.filter(|r| r.job_id == id).ok_or_else(|| failed("run does not exist for this schedule"))?;
                if !["queued", "running"].contains(&r.status.as_str()) {
                    return Ok(json!({"message": "This run already has a recorded result, so this call changed nothing."}));
                }
                vec![r]
            }
            None => {
                let active = store.active_runs(&id).await.map_err(failed)?;
                let running: Vec<Run> = active.iter().filter(|r| r.status == "running").cloned().collect();
                if running.is_empty() { active } else { running }
            }
        };
        if target.is_empty() || target.iter().all(|r| r.status == "queued") {
            for r in &target {
                store.finish_run(&r.run_id, "cancelled", None, Some("stopped by user request"), now).await.map_err(failed)?;
            }
            return Ok(json!({"message": "No run was executing; the schedule's queued occurrence(s) were cancelled silently. The schedule itself is unchanged."}));
        }
        for r in &target {
            if let Some(h) = self.core.running.lock().unwrap().remove(&r.run_id) {
                h.abort();
            }
            if store.finish_run(&r.run_id, "cancelled", None, Some("stopped by user request"), now).await.map_err(failed)? && r.status == "running" {
                let payload = json!({"job_id": id, "run_id": r.run_id, "status": "cancelled", "message": STOPPED_BY_USER});
                store.outbox_put(&id, &r.run_id, &payload.to_string(), now).await.map_err(failed)?;
            }
        }
        Ok(json!({"message": "The in-flight run was stopped and its worker and subagents were cancelled. The stop notice for the job's chat is saved and will be delivered shortly."}))
    }
}

/// Системное задание (пульс) этими инструментами не меняется — текст ядра
/// «… is the assistant's heartbeat and cannot be paused or resumed»
/// (`strings-all.txt:38953`).
fn system_guard(job: &Job) -> Result<(), ToolError> {
    if job.definition.mode == "heartbeat" {
        return Err(failed(format!("{} is the assistant's heartbeat and cannot be paused or resumed", job.job_id)));
    }
    Ok(())
}

fn merge(a: &mut Value, b: Value) {
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
}
