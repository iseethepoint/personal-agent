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
const GOAL_CONTEXT: &str = include_str!("../../../prompts/cron/goal_context.md");
const GOAL_RECORD_USER_GOAL: &str = include_str!("../../../prompts/cron/goal_record_user_goal.md");
const GOAL_RECORD_TRACKING: &str = include_str!("../../../prompts/cron/goal_record_tracking.md");
const GOAL_GUIDE_MAX_BYTES: usize = 64_000;

pub const HEARTBEAT_ID: &str = "heartbeat";

/// Шаблон `HEARTBEAT.md` — тело шаблона ядра (`strings-all.txt:84903-84908`)
/// и первый вариант комментария (`:85426`).
pub const HEARTBEAT_TEMPLATE: &str = "# HEARTBEAT.md\n\n# Keep this file empty (or with only comments) to skip heartbeat API calls.\n\n_Recurring checks for your heartbeat worker. Add one per line, and the next tick picks them up._\n";

/// Предел `HEARTBEAT.md` в сообщении — 100 000 знаков (`strings-all.txt:84929`).
const HEARTBEAT_MAX_CHARS: usize = 100_000;

/// Есть ли в `HEARTBEAT.md` инструкции: строки, кроме пустых, комментариев
/// `#` и строки-подсказки шаблона. Что именно считается инструкцией, в
/// разборе не видно — своё.
pub fn has_instructions(text: &str) -> bool {
    text.lines().map(str::trim).any(|l| !l.is_empty() && !l.starts_with('#') && !(l.starts_with('_') && l.ends_with('_')) && !l.starts_with("<!--"))
}

fn truncate_heartbeat(text: &str) -> String {
    if text.chars().count() <= HEARTBEAT_MAX_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(HEARTBEAT_MAX_CHARS).collect();
    format!("HEARTBEAT.md exceeds 100000 characters. This message includes only the first 100000 characters; read ~/HEARTBEAT.md for the rest of the contents.\n\n{head}")
}

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
    /// Имена переменных с ключами: скрипты наблюдателей их не получают.
    pub secret_envs: Vec<String>,
    /// Сроки следующих опросов наблюдателей.
    pub(crate) next_poll: crate::hooks::PollSchedule,
    /// Срабатывания наблюдателей, ждущие работника.
    pub(crate) wakes: Mutex<Vec<crate::hooks::Wake>>,
    /// Цели — для `owner: goal:<slug>` и контекста работника.
    pub goals: Option<agent_goals::Store>,
    /// Будильник эскалаций: сроки до этой отметки уже разобраны.
    pub(crate) escalations_through: Mutex<i64>,
}

impl Core {
    /// Затравка времени машины для `timing=window|anytime` — путь дома
    /// агента (у muse — ВМ; своё).
    pub fn machine(&self) -> String {
        self.home.display().to_string()
    }

    pub fn new(store: Store, clock: Arc<dyn Clock>, home: impl Into<PathBuf>, user_tz: Option<String>) -> Arc<Core> {
        Self::build(store, clock, home, user_tz, Vec::new(), None)
    }

    /// `secret_envs` — как у `hamlet.exec` (`agent_tools::secret_envs`).
    pub fn with_secrets(store: Store, clock: Arc<dyn Clock>, home: impl Into<PathBuf>, user_tz: Option<String>, secret_envs: Vec<String>) -> Arc<Core> {
        Self::build(store, clock, home, user_tz, secret_envs, None)
    }

    pub fn build(
        store: Store,
        clock: Arc<dyn Clock>,
        home: impl Into<PathBuf>,
        user_tz: Option<String>,
        secret_envs: Vec<String>,
        goals: Option<agent_goals::Store>,
    ) -> Arc<Core> {
        let started = clock.now_ms();
        Arc::new(Core {
            store,
            clock,
            home: home.into(),
            user_tz,
            running: Mutex::new(HashMap::new()),
            secret_envs,
            next_poll: Default::default(),
            wakes: Mutex::new(Vec::new()),
            goals,
            escalations_through: Mutex::new(started),
        })
    }

    /// Блок `[GOAL CONTEXT]` для задания цели — `goal_context.md` muse с
    /// указаниями по виду цели (`goal_record_user_goal.md` /
    /// `goal_record_tracking.md`) и `GOAL.md` (до 64 000 байт, текст ядра
    /// `strings-all.txt:84943`).
    pub async fn goal_context(&self, d: &crate::store::Definition) -> Option<String> {
        let slug = d.goal_slug()?;
        let goal = self.goals.as_ref()?.goal_by_slug(slug).await.ok()??;
        let record = if goal.source == agent_goals::TRACKING { GOAL_RECORD_TRACKING } else { GOAL_RECORD_USER_GOAL };
        let raw = tokio::fs::read(self.home.join(format!("workspace/goals/{slug}/GOAL.md"))).await.unwrap_or_default();
        let guide = if raw.len() > GOAL_GUIDE_MAX_BYTES {
            let mut cut = GOAL_GUIDE_MAX_BYTES;
            while cut > 0 && (raw[cut] & 0xC0) == 0x80 {
                cut -= 1;
            }
            format!(
                "GOAL.md exceeds 64000 bytes. This scheduled worker context includes only the first 64000 bytes; read the goal guide from the goal workspace if more detail is required.\n\n{}",
                String::from_utf8_lossy(&raw[..cut])
            )
        } else {
            String::from_utf8_lossy(&raw).to_string()
        };
        Some(
            agent_loop::personal(GOAL_CONTEXT)
                .replace("{goal_slug}", slug)
                .replace("{goal_record_instructions}", agent_loop::personal(record).trim_end())
                .replace("{goal_guide}", guide.trim_end()),
        )
    }

    /// Снять задание: отменить его прогоны, удалить из базы и копию.
    pub async fn remove_job(&self, id: &str, reason: &str) -> Result<(), String> {
        let now = self.now();
        for r in self.store.active_runs(id).await? {
            if let Some(h) = self.running.lock().unwrap().remove(&r.run_id) {
                h.abort();
            }
            self.store.finish_run(&r.run_id, "cancelled", None, Some(reason), now).await?;
        }
        let job = self.store.job(id).await?;
        self.store.delete_job(id).await?;
        if let Some(p) = job.and_then(|j| j.source_path) {
            let _ = tokio::fs::remove_file(self.home.join(p)).await;
        }
        self.store.record_mutation(id, "remove", now).await
    }
}

/// Задания цели удаляются вместе с ней (`cron_cleanup.rs` muse).
impl agent_goals::OwnedCleanup for Core {
    fn remove_owned<'a>(&'a self, owner: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send + 'a>> {
        Box::pin(async move {
            let mut removed = Vec::new();
            for j in self.store.jobs(true).await? {
                if j.definition.owner.as_deref() == Some(owner) {
                    self.remove_job(&j.job_id, "owner removed").await?;
                    removed.push(j.job_id);
                }
            }
            Ok(removed)
        })
    }
}

impl Core {

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

/// Модели работника расписаний через `Registry::resolve`: исполнение —
/// `CronWorker`, итог — `CronResolve`.
pub struct Plan<'r> {
    pub worker: agent_model::Resolved<'r>,
    pub resolve: agent_model::Resolved<'r>,
}

impl<'r> Plan<'r> {
    pub fn from_registry(reg: &'r agent_model::Registry, selection: Option<&str>) -> Result<Self, agent_model::RegistryError> {
        let get = |p: agent_model::Purpose| -> Result<agent_model::Resolved<'r>, agent_model::RegistryError> {
            Ok(reg.resolve(p, selection)?.expect("resolve отдаёт None только для Embeddings"))
        };
        Ok(Plan { worker: get(agent_model::Purpose::CronWorker)?, resolve: get(agent_model::Purpose::CronResolve)? })
    }
}

/// Служба: такт и прогоны на модели `CronWorker`.
pub struct Scheduler<P> {
    core: Arc<Core>,
    provider: Arc<P>,
    /// Модель фазы итога работника расписаний — назначение muse
    /// `CronResolve` (`strings-all.txt:56281`).
    resolver: Arc<P>,
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
        Scheduler { core, resolver: provider.clone(), provider, system: system.into(), tools, spill_dir: spill_dir.into(), inbox, background: Vec::new() }
    }

    /// Модель фазы итога (`Plan::resolve`); без вызова — та же, что у
    /// исполнения.
    pub fn resolver(&mut self, p: Arc<P>) -> &mut Self {
        self.resolver = p;
        self
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
        crate::hooks::poll(&self.core).await?;
        let wakes: Vec<_> = self.core.wakes.lock().unwrap().drain(..).collect();
        for w in wakes {
            self.wake(w);
        }
        let now = self.core.now();
        for job in self.core.store.due(now).await? {
            self.admit(&job, now).await?;
        }
        // В очередь прогон попадает уже созревшим (срок, `cron.run`, повтор).
        for run in self.core.store.queued().await? {
            self.start(run.run_id, run.job_id, run.attempt).await?;
        }
        self.escalations(now).await?;
        self.deliver().await
    }

    /// Будильник эскалаций целей: созревшая эскалация — весть главному
    /// агенту. У muse её получает проактивный вестник по выборке
    /// `goals.upcoming_escalations` (sql.txt:4314, «escalation due»,
    /// действия «Muse should let the user know» / «Muse should ask the user
    /// what to do», `strings-all.txt`); вестника у нас нет — весть идёт в
    /// ящик главного агента, как итог работника. Своё: вид вести и отметка
    /// разобранного срока в памяти службы (после перезапуска срок,
    /// созревший во время простоя, не будит).
    async fn escalations(&self, now: i64) -> Result<(), String> {
        let Some(goals) = &self.core.goals else { return Ok(()) };
        let after = *self.core.escalations_through.lock().unwrap();
        if now <= after {
            return Ok(());
        }
        for g in goals.due_escalations(after, now).await? {
            match escalation_note(&g) {
                Some(m) => self.inbox.push(m),
                None => eprintln!("goal {}: goal has escalation_due_at without an escalation payload", g.goal_id),
            }
        }
        *self.core.escalations_through.lock().unwrap() = now;
        Ok(())
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
        if let Err(e) = self.ensure_heartbeat().await {
            eprintln!("scheduler heartbeat: {e}");
        }
        loop {
            if let Err(e) = self.tick().await {
                eprintln!("scheduler tick: {e}");
            }
            tokio::time::sleep(TICK).await;
        }
    }

    /// Работник срабатывания наблюдателя: итог — сразу в ящик.
    fn wake(&self, w: crate::hooks::Wake) {
        let run_id = format!("hook:{}:{}", w.def.id, w.inv.invocation_id);
        let provider = self.provider.clone();
        let tools = self.tools.clone();
        let system = crate::hooks::worker_system(&self.system);
        let spill = self.spill_dir.clone();
        let inbox = self.inbox.clone();
        let id = run_id.clone();
        let handle = tokio::spawn(async move {
            let first = crate::hooks::execute_message(&w, &id);
            let kind = worker::Kind::Hook { resolve: crate::hooks::resolve_message(&w, &id) };
            let out = worker::run(&*provider, &*provider, &system, &tools, &spill, first, None, kind, |_, _| {}).await;
            match out.signal {
                Some(Signal::Notify { message, .. }) => inbox.push(Message {
                    role: Role::Developer,
                    content: vec![Part::Text { text: crate::hooks::result_text(&message) }],
                }),
                Some(Signal::Quiet) => {}
                None => eprintln!("hook run {id}: no terminal decision ({})", out.error.unwrap_or_default()),
            }
        });
        self.core.running.lock().unwrap().insert(run_id, handle);
    }

    /// Пульс: системное задание `heartbeat` раз в 30 минут и шаблон
    /// `~/HEARTBEAT.md`, если их нет — `seed_default_heartbeat_schedule_if_missing`
    /// muse (`strings-all.txt`, «30mRun the heartbeat checklist from
    /// HEARTBEAT.md»); фронтматтер — как у `heartbeat__interval@30m.md` muse.
    pub async fn ensure_heartbeat(&self) -> Result<(), String> {
        let file = self.core.home.join("HEARTBEAT.md");
        if !file.exists() {
            tokio::fs::write(&file, HEARTBEAT_TEMPLATE).await.map_err(|e| format!("HEARTBEAT.md: {e}"))?;
        }
        if self.core.store.job(HEARTBEAT_ID).await?.is_some() {
            return Ok(());
        }
        let now = self.core.now();
        let d = crate::store::Definition {
            id: HEARTBEAT_ID.into(),
            title: None,
            enabled: true,
            mode: "heartbeat".into(),
            owner: None,
            delivery: vec![],
            timeout_secs: None,
            retry: Default::default(),
            schedule: crate::schedule::Schedule::from_input(
                &json!({"kind": "interval", "every": "30m", "at": "2026-01-01T00:00:00", "timezone": "UTC"}),
                None,
                None,
                &self.core.machine(),
                now,
            )?,
            metadata: crate::store::Metadata { created_by: "system".into(), notification_requested: None, created_at_ms: now },
            body: "Run the heartbeat checklist from HEARTBEAT.md.".into(),
        };
        let next = d.schedule.next_after(now, now, None)?.unwrap_or(0);
        let text = d.render();
        self.core.store.save(&d, &crate::tools::fingerprint(&text), next, now).await?;
        let path = self.core.home.join(d.source_path());
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
        }
        tokio::fs::write(path, text).await.map_err(|e| e.to_string())
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
        // Пульс без инструкций модель не зовёт — «skipping heartbeat
        // inference because HEARTBEAT.md has no instructions» (`strings-all.txt:84815`).
        let goal = self.core.goal_context(&job.definition).await;
        let heartbeat = if job.definition.mode == "heartbeat" {
            let text = tokio::fs::read_to_string(self.core.home.join("HEARTBEAT.md")).await.unwrap_or_default();
            if !has_instructions(&text) {
                self.core.store.finish_run(&run_id, "succeeded", Some("heartbeat_no_instructions"), None, now).await?;
                return Ok(());
            }
            Some(truncate_heartbeat(&text))
        } else {
            None
        };
        let core = self.core.clone();
        let provider = self.provider.clone();
        let resolver = self.resolver.clone();
        let tools = self.tools.clone();
        let system = self.system.clone();
        let spill = self.spill_dir.clone();
        let id = run_id.clone();
        let handle = tokio::spawn(async move {
            let d = &job.definition;
            let first = worker::worker_message(&d.id, &d.mode, d.title.as_deref(), &d.body, heartbeat.as_deref(), goal.as_deref());
            let store = core.store.clone();
            let phase_run = id.clone();
            let on_phase = move |p: &'static str, timed_out: bool| {
                let store = store.clone();
                let run = phase_run.clone();
                tokio::spawn(async move {
                    let _ = store.set_phase(&run, p, timed_out).await;
                });
            };
            let kind = worker::Kind::Cron { mode: d.mode.clone() };
            let out = worker::run(&*provider, &*resolver, &system, &tools, &spill, first, Some(timeout), kind, on_phase).await;
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
            "job_id": d.id, "run_id": run_id, "title": d.title, "body": d.body, "mode": d.mode,
            "status": status, "message": text,
        });
        store.outbox_put(&d.id, run_id, &payload.to_string(), now).await?;
    }
    Ok(())
}


/// Скрытая заметка главному агенту: `[SCHEDULED TASK HANDOFF]`
/// (`strings-all.txt:85472`), вступление `main_agent_handoff_intro.md`,
/// задание в `task_context`, отчёт в `<job_result>`. Склейка — своя.
/// Весть о созревшей эскалации цели.
pub fn escalation_note(g: &agent_goals::Goal) -> Option<Message> {
    let e = g.escalation.as_ref()?;
    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let act = if s("action") == "ask" { "You should ask the user what to do" } else { "You should let the user know" };
    let text = format!(
        "[GOAL ESCALATION DUE]\nThe escalation stored on this goal has come due. {act}.\n\ngoal_id: {}\ngoal: {}\ncondition: {}\ndue_at: {}\naction: {}",
        g.goal_id,
        g.title,
        s("condition"),
        s("due_at"),
        s("action"),
    );
    Some(Message { role: Role::Developer, content: vec![Part::Text { text }] })
}

pub fn handoff(v: &Value) -> Message {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
    let title = if s("title").is_empty() { String::new() } else { format!("title: {}\n", s("title")) };
    let text = format!(
        "{}\n{}\n\n<task_context>\njob_id: {}\n{title}run_id: {}\nstatus: {}\n\n{}\n</task_context>\n\n<job_result>\n{}\n</job_result>",
        // Метки ядра: `[SCHEDULED TASK HANDOFF]`, `[HEARTBEAT HANDOFF]` (`strings-all.txt:85472`).
        if s("mode") == "heartbeat" { "[HEARTBEAT HANDOFF]" } else { "[SCHEDULED TASK HANDOFF]" },
        agent_loop::personal(HANDOFF_INTRO).trim_end(),
        s("job_id"),
        s("run_id"),
        s("status"),
        s("body"),
        s("message"),
    );
    Message { role: Role::Developer, content: vec![Part::Text { text }] }
}
