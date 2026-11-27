//! Служба расписаний: такт — фоновая работа, созревшие задания в очередь,
//! прогоны из очереди, итоги главному агенту. Форма — `scheduler loop` muse
//! (`strings-all.txt:223125-224100`: admission, reconcile, delivery outbox).
//!
//! Своё: у muse такт будят наблюдатель файлов и срок ближайшего прогона;
//! здесь — ровный такт `TICK` (сверка памяти идёт на нём же, потому что
//! наблюдателя файлов у нас нет).

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_loop::{Inbox, Tools};
use agent_model::{Message, Part, Provider, Role};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::clock::Clock;
use crate::store::{Job, Store};
use crate::worker::{self, Signal};

const HANDOFF_INTRO: &str = include_str!("../../../prompts/cron/main_agent_handoff_intro.md");

/// Ровный такт службы. Своё (см. заголовок).
pub const TICK: Duration = Duration::from_secs(15);

/// Срок исполнения по умолчанию: у повторяющихся — 3600 с, у разовых —
/// 86400 с; потолок — 86400 с (описание `timeout_secs` muse).
pub const RECURRING_TIMEOUT_SECS: i64 = 3600;
pub const MAX_TIMEOUT_SECS: i64 = 86400;

pub type Background = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

/// Общее для службы и инструментов `cron.*`.
pub struct Core {
    pub store: Store,
    pub clock: Arc<dyn Clock>,
    /// Дом агента: копии определений лежат в `workspace/cron.d/`.
    pub home: PathBuf,
    /// Текущий пояс владельца, если известен.
    pub user_tz: Option<String>,
    pub(crate) running: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl Core {
    pub fn new(store: Store, clock: Arc<dyn Clock>, home: impl Into<PathBuf>, user_tz: Option<String>) -> Arc<Core> {
        Arc::new(Core { store, clock, home: home.into(), user_tz, running: Mutex::new(HashMap::new()) })
    }

    pub fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Бюджет прогона (описание `timeout_secs` muse): повторяющееся —
    /// `min(86400, max(запрошенное или 3600, промежуток между сроками))`,
    /// разовое — запрошенное или 86400.
    pub fn effective_timeout(&self, job: &crate::store::Definition, from_ms: i64) -> (i64, Option<i64>) {
        if job.schedule.kind == "runonce" {
            return (job.timeout_secs.unwrap_or(MAX_TIMEOUT_SECS), None);
        }
        let gap = job.schedule.gap_secs(from_ms, job.metadata.created_at_ms, self.user_tz.as_deref());
        let t = job.timeout_secs.unwrap_or(RECURRING_TIMEOUT_SECS).max(gap.unwrap_or(0)).min(MAX_TIMEOUT_SECS);
        (t, gap)
    }
}

/// Служба: такт и прогоны на модели `CronWorker`.
pub struct Scheduler<P> {
    core: Arc<Core>,
    provider: Arc<P>,
    /// Системный текст и инструменты работника.
    system: String,
    tools: Arc<Tools>,
    spill_dir: PathBuf,
    inbox: Inbox,
    background: Vec<Background>,
}

fn new_run_id(job_id: &str, scheduled_for: i64, attempt: i32) -> String {
    format!("{job_id}:{scheduled_for}:{attempt}")
}

impl<P: Provider + Send + Sync + 'static> Scheduler<P> {
    pub fn new(core: Arc<Core>, provider: Arc<P>, system: impl Into<String>, tools: Arc<Tools>, spill_dir: impl Into<PathBuf>, inbox: Inbox) -> Self {
        Scheduler { core, provider, system: system.into(), tools, spill_dir: spill_dir.into(), inbox, background: Vec::new() }
    }

    /// Фоновая работа на каждом такте (сверка памяти).
    pub fn background(&mut self, f: Background) -> &mut Self {
        self.background.push(f);
        self
    }

    pub fn core(&self) -> &Arc<Core> {
        &self.core
    }

    /// Один такт.
    pub async fn tick(&self) -> Result<(), String> {
        for b in &self.background {
            b().await?;
        }
        let now = self.core.now();
        for job in self.core.store.due(now).await? {
            self.admit(&job, now).await?;
        }
        // В очередь прогон попадает уже созревшим (срок, `cron.run`, повтор).
        for run in self.core.store.queued().await? {
            self.start(run.run_id, run.job_id, run.attempt).await?;
        }
        self.deliver().await
    }

    /// Дождаться идущих прогонов и отдать их итоги (для тестов и остановки).
    pub async fn settle(&self) -> Result<(), String> {
        loop {
            let handles: Vec<JoinHandle<()>> = self.core.running.lock().unwrap().drain().map(|(_, h)| h).collect();
            if handles.is_empty() {
                break;
            }
            for h in handles {
                let _ = h.await;
            }
        }
        self.deliver().await
    }

    /// Прогоны, оставшиеся «идущими» после остановки службы, закрываются
    /// как неудачные — `recover_orphaned_running_runs` muse; текст для
    /// главного агента — ядро («Stopped because the agent restarted mid-run…»).
    pub async fn recover(&self) -> Result<(), String> {
        let now = self.core.now();
        for r in self.core.store.running().await? {
            let text = "Stopped because the agent restarted mid-run and this run could not be resumed safely. Steps it finished stand; check their effects before running it again.";
            if self.core.store.finish_run(&r.run_id, "failed", None, Some(text), now).await? {
                let payload = json!({"job_id": r.job_id, "run_id": r.run_id, "status": "failed", "message": text});
                self.core.store.outbox_put(&r.job_id, &r.run_id, &payload.to_string(), now).await?;
            }
        }
        Ok(())
    }

    /// Ровный такт до конца процесса.
    pub async fn run_forever(&self) {
        if let Err(e) = self.recover().await {
            eprintln!("scheduler recovery: {e}");
        }
        loop {
            if let Err(e) = self.tick().await {
                eprintln!("scheduler tick: {e}");
            }
            tokio::time::sleep(TICK).await;
        }
    }

    /// Созревшее задание: прогон в очередь, следующий срок. Повторяющееся
    /// держит только новейший прогон из очереди («Recurring tasks keep the
    /// newest queued occurrence by default», описание `cron` muse).
    async fn admit(&self, job: &Job, now: i64) -> Result<(), String> {
        let d = &job.definition;
        let recurring = d.schedule.kind != "runonce";
        if recurring {
            for r in self.core.store.active_runs(&job.job_id).await? {
                if r.status == "queued" {
                    self.core.store.finish_run(&r.run_id, "cancelled", None, Some("superseded by a newer occurrence"), now).await?;
                }
            }
        }
        let scheduled = job.next_run_at_utc;
        self.core.store.enqueue(&new_run_id(&job.job_id, scheduled, 1), job, scheduled, "schedule", 1).await?;
        let next = if recurring {
            d.schedule.next_after(now, d.metadata.created_at_ms, self.core.user_tz.as_deref())?.unwrap_or(0)
        } else {
            0
        };
        self.core.store.set_next_run(&job.job_id, next, now).await
    }

    async fn start(&self, run_id: String, job_id: String, attempt: i32) -> Result<(), String> {
        let now = self.core.now();
        let Some(job) = self.core.store.job(&job_id).await? else {
            self.core.store.finish_run(&run_id, "cancelled", None, Some("schedule does not exist"), now).await?;
            return Ok(());
        };
        if !self.core.store.start_run(&run_id, now).await? {
            return Ok(());
        }
        let (timeout, _) = self.core.effective_timeout(&job.definition, now);
        let core = self.core.clone();
        let provider = self.provider.clone();
        let tools = self.tools.clone();
        let system = self.system.clone();
        let spill = self.spill_dir.clone();
        let id = run_id.clone();
        let handle = tokio::spawn(async move {
            let d = &job.definition;
            let first = worker::worker_message(&d.id, &d.mode, d.title.as_deref(), &d.body);
            let store = core.store.clone();
            let phase_run = id.clone();
            let on_phase = move |p: &'static str, timed_out: bool| {
                let store = store.clone();
                let run = phase_run.clone();
                tokio::spawn(async move {
                    let _ = store.set_phase(&run, p, timed_out).await;
                });
            };
            let out = worker::run(&*provider, &system, &tools, &spill, first, timeout, on_phase).await;
            if let Err(e) = settle_run(&core, &job, &id, attempt, timeout, out).await {
                eprintln!("scheduler run {id}: {e}");
            }
        });
        self.core.running.lock().unwrap().insert(run_id, handle);
        Ok(())
    }

    /// Итоги из `delivery_outbox` — в ящик главного агента.
    async fn deliver(&self) -> Result<(), String> {
        let now = self.core.now();
        for (key, payload) in self.core.store.outbox_pending(now).await? {
            let v: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
            self.inbox.push(handoff(&v));
            self.core.store.outbox_delivered(&key, now).await?;
        }
        Ok(())
    }
}

/// Записать итог прогона: сигнал, статус, итог для главного агента,
/// повтор по `retry`.
async fn settle_run(core: &Core, job: &Job, run_id: &str, attempt: i32, timeout: i64, out: worker::Outcome) -> Result<(), String> {
    let now = core.now();
    let store = &core.store;
    let d = &job.definition;
    let (status, summary, error, report) = match &out.signal {
        Some(Signal::Quiet) => {
            store.signal(run_id, "nothing_to_report", None, (None, None), now).await?;
            ("succeeded", None, None, None)
        }
        Some(Signal::Notify { outcome, message, blocked_reason, blocked_dependency }) => {
            store.signal(run_id, "notify_main_agent", Some(message), (blocked_reason.as_deref(), blocked_dependency.as_deref()), now).await?;
            let status = if outcome == "succeeded" { "succeeded" } else { "failed" };
            let error = match outcome.as_str() {
                "blocked" => Some(format!("blocked: {} ({})", blocked_reason.clone().unwrap_or_default(), blocked_dependency.clone().unwrap_or_default())),
                "failed" => Some("The worker reported that the work is not finished; its report is in the result summary.".to_string()),
                _ => None,
            };
            (status, Some(message.clone()), error, Some(message.clone()))
        }
        None => {
            // Тексты для главного агента — `strings-all.txt:18464-18465`
            // (срок) и «The cron run failed before it could finish. Failure: …».
            let text = if out.timed_out {
                format!("The cron run timed out after {timeout}s before it could finish. Treat this scheduled task as incomplete. Report the observed timeout and any committed work. Do not infer that the schedule never started or recreate the schedule; changing its identity revokes its permissions. Verify external effects before considering a retry.")
            } else {
                let failure = out.error.clone().unwrap_or_else(|| "The check finished its work but never filed its report, so nothing reached the user. Its work may be done; check its effects before running it again.".into());
                format!("The cron run failed before it could finish.\nFailure: {failure}\nTreat this scheduled task as incomplete. Explain this recorded failure and any committed work; do not infer that execution never started. Keep the same schedule identity, and verify external effects before retrying.")
            };
            ("failed", None, Some(text.clone()), Some(text))
        }
    };
    if !store.finish_run(run_id, status, summary.as_deref(), error.as_deref(), now).await? {
        // Прогон уже закрыт (`cron.stop`).
        return Ok(());
    }
    // Повтор — только сбои среды и сроки, не итог работника («Worker-reported
    // incomplete results … are never retried automatically»).
    let runtime_failure = out.signal.is_none();
    if runtime_failure && d.retry.on_failure && attempt <= d.retry.max_retries {
        if let Some(r) = store.run(run_id).await? {
            store.enqueue(&new_run_id(&d.id, r.scheduled_for_utc, attempt + 1), job, r.scheduled_for_utc, "retry", attempt + 1).await?;
            return Ok(());
        }
    }
    if let Some(text) = report {
        let payload = json!({
            "job_id": d.id, "run_id": run_id, "title": d.title, "body": d.body,
            "status": status, "message": text,
        });
        store.outbox_put(&d.id, run_id, &payload.to_string(), now).await?;
    }
    Ok(())
}


/// Скрытая заметка главному агенту: `[SCHEDULED TASK HANDOFF]`
/// (`strings-all.txt:85472`), вступление `main_agent_handoff_intro.md`,
/// задание в `task_context`, отчёт в `<job_result>`. Склейка — своя.
pub fn handoff(v: &Value) -> Message {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
    let title = if s("title").is_empty() { String::new() } else { format!("title: {}\n", s("title")) };
    let text = format!(
        "[SCHEDULED TASK HANDOFF]\n{}\n\n<task_context>\njob_id: {}\n{title}run_id: {}\nstatus: {}\n\n{}\n</task_context>\n\n<job_result>\n{}\n</job_result>",
        agent_loop::personal(HANDOFF_INTRO).trim_end(),
        s("job_id"),
        s("run_id"),
        s("status"),
        s("body"),
        s("message"),
    );
    Message { role: Role::Developer, content: vec![Part::Text { text }] }
}
