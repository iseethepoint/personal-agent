//! Хранилище расписаний в Postgres — форма muse `hatch-db-scheduler`
//! (`source-files.txt:4880-…`). Запросы — по текстам ядра
//! (`strings-all.txt:223125-224100`): `insert into scheduler.jobs … on
//! conflict (job_id) do update`, `insert into scheduler.job_runs … select
//! greatest($3, coalesce(max(scheduled_for_utc) + 1, $3))`, `insert into
//! scheduler.delivery_outbox … 'pending'`, `insert into
//! scheduler.terminal_signals … on conflict (run_id) do nothing`.
//!
//! Сохранённое определение задания — `job_definitions.task_text`: файл
//! определения целиком (фронтматтер + тело), тот же, что копией лежит в
//! `~/workspace/cron.d/` («read-only copies of saved job definitions»,
//! описание `cron` muse). Своё: у muse полный файл держит хост-хранилище
//! заданий, а `task_text` — вероятно, только тело.

use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions};
use serde_json::Value;
use sqlx::Row;

use crate::schedule::Schedule;

const MIGRATIONS: [&str; 3] = [
    include_str!("../migrations/0001_scheduler.sql"),
    include_str!("../migrations/0002_runtime_hooks.sql"),
    include_str!("../migrations/0003_execute_resolve.sql"),
];

pub type Result<T> = std::result::Result<T, String>;

fn db(e: sqlx::Error) -> String {
    format!("scheduler database: {e}")
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Retry {
    #[serde(default)]
    pub on_failure: bool,
    #[serde(default)]
    pub max_retries: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub created_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_requested: Option<bool>,
    pub created_at_ms: i64,
}

/// Определение задания — фронтматтер файла muse (`id`, `enabled`, `mode`,
/// `schedule`, `retry`, `delivery`, …; «required canonical frontmatter keys
/// include id/enabled/mode/schedule/retry/delivery»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub enabled: bool,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub delivery: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<i64>,
    pub retry: Retry,
    pub schedule: Schedule,
    pub metadata: Metadata,
    #[serde(skip)]
    pub body: String,
}

impl Definition {
    /// Файл определения: `---`, YAML, `---`, тело.
    pub fn render(&self) -> String {
        let yaml = serde_yaml::to_string(self).unwrap_or_default();
        format!("---\n{yaml}---\n{}\n", self.body.trim_end())
    }

    pub fn parse(text: &str) -> Result<Definition> {
        let rest = text.strip_prefix("---\n").ok_or("invalid scheduler definition: missing frontmatter")?;
        let (front, body) = rest.split_once("\n---\n").ok_or("invalid scheduler definition: unterminated frontmatter")?;
        let mut d: Definition =
            serde_yaml::from_str(front).map_err(|e| format!("invalid scheduler frontmatter: expected YAML mapping ({e})"))?;
        d.body = body.trim_end().to_string();
        Ok(d)
    }

    /// Путь копии от дома: `workspace/cron.d/<вид>/<id>__<ключ>.md`; у
    /// задания цели — `workspace/goals/<slug>/crons/<id>__<ключ>.md`
    /// (описание `cron` muse, `goal_context.md`).
    pub fn source_path(&self) -> String {
        match self.goal_slug() {
            Some(slug) => format!("workspace/goals/{slug}/crons/{}__{}.md", self.id, self.schedule.key()),
            None => format!("workspace/cron.d/{}/{}__{}.md", self.schedule.folder(), self.id, self.schedule.key()),
        }
    }

    /// Slug цели-владельца (`owner: goal:<slug>`).
    pub fn goal_slug(&self) -> Option<&str> {
        self.owner.as_deref().and_then(|o| o.strip_prefix("goal:"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub job_id: String,
    pub enabled: bool,
    pub schedule_kind: String,
    pub timezone: String,
    pub next_run_at_utc: i64,
    pub last_run_at_utc: Option<i64>,
    pub last_status: Option<String>,
    pub source_path: Option<String>,
    pub definition: Definition,
    pub definition_id: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub run_id: String,
    pub job_id: String,
    pub scheduled_for_utc: i64,
    pub trigger_reason: Option<String>,
    pub status: String,
    pub started_at_utc: Option<i64>,
    pub finished_at_utc: Option<i64>,
    pub result_summary: Option<String>,
    pub error_text: Option<String>,
    pub attempt: i32,
    /// Срок фазы исполнения уже вышел (`worker_execution_timed_out`).
    pub timed_out: bool,
}

fn run(r: &sqlx::postgres::PgRow) -> Run {
    Run {
        run_id: r.get("run_id"),
        job_id: r.get("job_id"),
        scheduled_for_utc: r.get("scheduled_for_utc"),
        trigger_reason: r.get("trigger_reason"),
        status: r.get("status"),
        started_at_utc: r.get("started_at_utc"),
        finished_at_utc: r.get("finished_at_utc"),
        result_summary: r.get("result_summary"),
        error_text: r.get("error_text"),
        attempt: r.get("attempt"),
        timed_out: r.get("worker_execution_timed_out"),
    }
}

const RUN_COLUMNS: &str = "run_id, job_id, scheduled_for_utc, trigger_reason, status::text as status, started_at_utc, finished_at_utc, result_summary, error_text, attempt, worker_execution_timed_out";

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(4).connect(url).await.map_err(db)?;
        for m in MIGRATIONS {
            sqlx::raw_sql(m).execute(&pool).await.map_err(db)?;
        }
        Ok(Store { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Сохранить определение и проекцию задания: новая версия
    /// `job_definitions` (старая — `retired_at`), строка `jobs`.
    pub async fn save(&self, d: &Definition, source_hash: &str, next_run_at_utc: i64, now_ms: i64) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("update scheduler.job_definitions set retired_at = now() where job_id = $1 and retired_at is null")
            .bind(&d.id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query(
            "insert into scheduler.job_definitions (job_id, version, task_text)
             select $1, coalesce(max(version), 0) + 1, $2 from scheduler.job_definitions where job_id = $1",
        )
        .bind(&d.id)
        .bind(d.render())
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query(
            "insert into scheduler.jobs
         (job_id, is_heartbeat, source_path, source_hash,
          schedule_kind, schedule_expr, timezone, enabled,
          retry_on_failure, max_retries,
          delivery_targets_json,
          next_run_at, next_run_at_utc, updated_at_utc, updated_at)
         values ($1, $12, $2, $3, $4, $5, $6, $7, $8, $9, '[]',
                 case when $10 > 0 then to_timestamp($10::double precision / 1000) end, $10, $11,
                 to_timestamp($11::double precision / 1000))
         on conflict (job_id) do update set
           is_heartbeat = excluded.is_heartbeat,
           source_path = excluded.source_path,
           source_hash = excluded.source_hash,
           schedule_kind = excluded.schedule_kind,
           schedule_expr = excluded.schedule_expr,
           timezone = excluded.timezone,
           enabled = excluded.enabled,
           retry_on_failure = excluded.retry_on_failure,
           max_retries = excluded.max_retries,
           next_run_at = excluded.next_run_at,
           next_run_at_utc = excluded.next_run_at_utc,
           updated_at_utc = excluded.updated_at_utc,
           updated_at = excluded.updated_at",
        )
        .bind(&d.id)
        .bind(d.source_path())
        .bind(source_hash)
        .bind(&d.schedule.kind)
        .bind(serde_json::to_string(&d.schedule).unwrap_or_default())
        .bind(&d.schedule.timezone)
        .bind(d.enabled)
        .bind(d.retry.on_failure)
        .bind(d.retry.max_retries)
        .bind(next_run_at_utc)
        .bind(now_ms)
        .bind(d.mode == "heartbeat")
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        tx.commit().await.map_err(db)
    }

    /// Запись в журнал изменений — `INSERT INTO scheduler.cron_mutations`
    /// (`sql.txt:4591`). Id сообщения, запроса и вызова инструменту не видны —
    /// пустые строки.
    pub async fn record_mutation(&self, job_id: &str, action: &str, now_ms: i64) -> Result<()> {
        sqlx::query(
            "insert into scheduler.cron_mutations (job_id, action, occurred_at_ms, carrier_message_id, request_id, tool_call_id, input_message_ids)
             values ($1, $2, $3, '', '', '', '{}')",
        )
        .bind(job_id)
        .bind(action)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    fn job_row(r: &sqlx::postgres::PgRow) -> Result<Job> {
        Ok(Job {
            job_id: r.get("job_id"),
            enabled: r.get("enabled"),
            schedule_kind: r.get("schedule_kind"),
            timezone: r.get("timezone"),
            next_run_at_utc: r.get("next_run_at_utc"),
            last_run_at_utc: r.get("last_run_at_utc"),
            last_status: r.get("last_status"),
            source_path: r.get("source_path"),
            definition: Definition::parse(r.get::<String, _>("task_text").as_str())?,
            definition_id: r.get("job_definition_id"),
        })
    }

    const JOB_SELECT: &'static str = "select j.job_id, j.enabled, j.schedule_kind, j.timezone, j.next_run_at_utc, j.last_run_at_utc, j.last_status, j.source_path, d.task_text, d.job_definition_id
         from scheduler.jobs j join scheduler.job_definitions d on d.job_id = j.job_id and d.retired_at is null";

    pub async fn job(&self, id: &str) -> Result<Option<Job>> {
        let row = sqlx::query(&format!("{} where j.job_id = $1", Self::JOB_SELECT))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        row.as_ref().map(Self::job_row).transpose()
    }

    pub async fn jobs(&self, include_disabled: bool) -> Result<Vec<Job>> {
        let rows = sqlx::query(&format!("{} where j.enabled or $1 order by j.job_id", Self::JOB_SELECT))
            .bind(include_disabled)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        rows.iter().map(Self::job_row).collect()
    }

    /// Созревшие задания: включены и срок наступил.
    /// Ближайший срок включённого задания.
    pub async fn next_due(&self) -> Result<Option<i64>> {
        sqlx::query_scalar("select min(next_run_at_utc) from scheduler.jobs where enabled and next_run_at_utc > 0").fetch_one(&self.pool).await.map_err(db)
    }

    pub async fn due(&self, now_ms: i64) -> Result<Vec<Job>> {
        let rows = sqlx::query(&format!(
            "{} where j.enabled and j.next_run_at_utc > 0 and j.next_run_at_utc <= $1 order by j.next_run_at_utc, j.job_id",
            Self::JOB_SELECT
        ))
        .bind(now_ms)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        rows.iter().map(Self::job_row).collect()
    }

    pub async fn set_next_run(&self, job_id: &str, next: i64, now_ms: i64) -> Result<()> {
        sqlx::query(
            "update scheduler.jobs set next_run_at_utc = $2,
                 next_run_at = case when $2 > 0 then to_timestamp($2::double precision / 1000) end,
                 updated_at_utc = $3, updated_at = to_timestamp($3::double precision / 1000)
             where job_id = $1",
        )
        .bind(job_id)
        .bind(next)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    pub async fn delete_job(&self, id: &str) -> Result<bool> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let n = sqlx::query("delete from scheduler.jobs where job_id = $1").bind(id).execute(&mut *tx).await.map_err(db)?.rows_affected();
        sqlx::query("update scheduler.job_definitions set retired_at = now() where job_id = $1 and retired_at is null")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(n > 0)
    }

    // ---- прогоны ----

    /// Поставить прогон в очередь — запрос muse: срок не раньше последнего
    /// прогона этого задания (`greatest($3, coalesce(max(scheduled_for_utc) + 1, $3))`).
    pub async fn enqueue(&self, run_id: &str, job: &Job, scheduled_for_utc: i64, trigger: &str, attempt: i32) -> Result<()> {
        sqlx::query(
            "insert into scheduler.job_runs
                         (run_id, job_id, job_definition_id, scheduled_for, scheduled_for_utc, trigger_reason, status, attempt)
                         select $1, $2, $5, to_timestamp(slot::double precision / 1000), slot, $4, 'queued', $6
                         from (select greatest($3, coalesce(max(scheduled_for_utc) + 1, $3)) as slot
                               from scheduler.job_runs where job_id = $2) next",
        )
        .bind(run_id)
        .bind(&job.job_id)
        .bind(scheduled_for_utc)
        .bind(trigger)
        .bind(job.definition_id)
        .bind(attempt)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    pub async fn queued(&self) -> Result<Vec<Run>> {
        let rows = sqlx::query(&format!("select {RUN_COLUMNS} from scheduler.job_runs where status = 'queued' order by scheduled_for_utc, run_id"))
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(run).collect())
    }

    pub async fn running(&self) -> Result<Vec<Run>> {
        let rows = sqlx::query(&format!("select {RUN_COLUMNS} from scheduler.job_runs where status = 'running' order by scheduled_for_utc, run_id"))
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(run).collect())
    }

    /// queued → running. `false` — прогон уже не в очереди.
    pub async fn start_run(&self, run_id: &str, now_ms: i64) -> Result<bool> {
        let n = sqlx::query(
            "update scheduler.job_runs set status = 'running', started_at_utc = $2,
                 started_at = to_timestamp($2::double precision / 1000), worker_phase = 'execute'
             where run_id = $1 and status = 'queued'",
        )
        .bind(run_id)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(db)?
        .rows_affected();
        Ok(n == 1)
    }

    /// Работник прогона (`worker_agent_id`).
    pub async fn set_worker(&self, run_id: &str, agent_id: &str) -> Result<()> {
        sqlx::query("update scheduler.job_runs set worker_agent_id = $2 where run_id = $1").bind(run_id).bind(agent_id).execute(&self.pool).await.map_err(db)?;
        Ok(())
    }

    pub async fn set_phase(&self, run_id: &str, phase: &str, timed_out: bool) -> Result<()> {
        sqlx::query("update scheduler.job_runs set worker_phase = $2, worker_execution_timed_out = worker_execution_timed_out or $3 where run_id = $1")
            .bind(run_id)
            .bind(phase)
            .bind(timed_out)
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    /// Закончить прогон и записать итог в задание. Только из queued/running:
    /// у уже закрытого прогона итог не переписывается.
    pub async fn finish_run(&self, run_id: &str, status: &str, summary: Option<&str>, error: Option<&str>, now_ms: i64) -> Result<bool> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let job: Option<String> = sqlx::query_scalar(
            "update scheduler.job_runs set status = $2::scheduler.run_status, result_summary = $3, error_text = $4,
                 finished_at_utc = $5, finished_at = to_timestamp($5::double precision / 1000), worker_phase = null
             where run_id = $1 and status in ('queued', 'running')
             returning job_id",
        )
        .bind(run_id)
        .bind(status)
        .bind(summary)
        .bind(error)
        .bind(now_ms)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        if let Some(job) = &job {
            sqlx::query(
                "update scheduler.jobs set last_run_at_utc = $2, last_status = $3,
                     last_success_at_utc = case when $3 = 'succeeded' then $2 else last_success_at_utc end,
                     consecutive_failures = case when $3 = 'failed' then consecutive_failures + 1 else 0 end
                 where job_id = $1",
            )
            .bind(job)
            .bind(now_ms)
            .bind(status)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        tx.commit().await.map_err(db)?;
        Ok(job.is_some())
    }

    pub async fn run(&self, run_id: &str) -> Result<Option<Run>> {
        let row = sqlx::query(&format!("select {RUN_COLUMNS} from scheduler.job_runs where run_id = $1"))
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(row.as_ref().map(run))
    }

    pub async fn runs(&self, job_id: Option<&str>, limit: i64) -> Result<Vec<Run>> {
        let rows = sqlx::query(&format!(
            "select {RUN_COLUMNS} from scheduler.job_runs where ($1::text is null or job_id = $1) order by scheduled_for_utc desc, run_id desc limit $2"
        ))
        .bind(job_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        Ok(rows.iter().map(run).collect())
    }

    pub async fn active_runs(&self, job_id: &str) -> Result<Vec<Run>> {
        let rows = sqlx::query(&format!(
            "select {RUN_COLUMNS} from scheduler.job_runs where job_id = $1 and status in ('queued', 'running') order by scheduled_for_utc"
        ))
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        Ok(rows.iter().map(run).collect())
    }

    /// Счёт для `cron.status` — запрос muse: включённые задания, прогоны в
    /// очереди и идущие.
    pub async fn counts(&self) -> Result<(i64, i64, i64)> {
        let r = sqlx::query(
            "select
               (select count(*)::bigint from scheduler.jobs where enabled = true) as enabled_jobs,
               (select count(*)::bigint from scheduler.job_runs where status = 'queued') as queued_runs,
               (select count(*)::bigint from scheduler.job_runs where status = 'running') as running_runs",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db)?;
        Ok((r.get(0), r.get(1), r.get(2)))
    }

    // ---- итог работника и доставка ----

    /// Завершающий сигнал работника — запрос muse `insert into
    /// scheduler.terminal_signals … on conflict (run_id) do nothing`.
    pub async fn signal(
        &self,
        run_id: &str,
        signal_type: &str,
        message: Option<&str>,
        blocked: (Option<&str>, Option<&str>),
        now_ms: i64,
    ) -> Result<()> {
        sqlx::query(
            "insert into scheduler.terminal_signals (run_id, signal_type, message, created_at_utc, blocked_reason, blocked_dependency)
             values ($1, $2, $3, $4, $5, $6) on conflict (run_id) do nothing",
        )
        .bind(run_id)
        .bind(signal_type)
        .bind(message)
        .bind(now_ms)
        .bind(blocked.0)
        .bind(blocked.1)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Итог для главного агента — запрос muse `insert into
    /// scheduler.delivery_outbox … 'pending' … on conflict (delivery_key) do nothing`.
    pub async fn outbox_put(&self, job_id: &str, run_id: &str, payload: &str, now_ms: i64) -> Result<()> {
        sqlx::query(
            "insert into scheduler.delivery_outbox
                           (delivery_key, job_id, run_id, payload_json, state,
                            attempt_count, next_attempt_at_utc, created_at_utc,
                            updated_at_utc)
                         values ($1, $2, $3, $4, 'pending', 0, $5, $5, $5)
                         on conflict (delivery_key) do nothing",
        )
        .bind(format!("main:{run_id}"))
        .bind(job_id)
        .bind(run_id)
        .bind(payload)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Неотданные итоги, срок которых наступил.
    pub async fn outbox_pending(&self, now_ms: i64) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "select delivery_key, payload_json from scheduler.delivery_outbox
             where state = 'pending' and next_attempt_at_utc <= $1 order by next_attempt_at_utc, delivery_key",
        )
        .bind(now_ms)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
    }

    pub async fn outbox_delivered(&self, key: &str, now_ms: i64) -> Result<()> {
        sqlx::query(
            "update scheduler.delivery_outbox set state = 'delivered', delivered_at_utc = $2, updated_at_utc = $2,
                 attempt_count = attempt_count + 1 where delivery_key = $1 and state = 'pending'",
        )
        .bind(key)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }
}

/// Прогон наблюдателя (`runtime.execute_resolve_runs`).
#[derive(Debug, Clone)]
pub struct HookRun {
    pub run_id: String,
    pub status: String,
    pub terminal_message: Option<String>,
    pub worker_agent_id: Option<String>,
    pub metadata: Value,
}

/// Прогоны наблюдателей — запросы ядра (около `0x18c4fcf`…`0x18c6d27`):
/// заведение `insert … values ($1, $2, $3, $4, 'pending', $5) on conflict
/// (worker_kind, source_ref) do update …`, взятие работником `set
/// worker_generation = worker_generation + 1, worker_phase = 'execute',
/// worker_agent_id = $2, worker_started_at = now(), execute_message_id = $3,
/// resolve_message_id = $4 … where … status = 'pending' and worker_agent_id
/// is null`, фаза `set worker_phase = $5`, решение `set terminal_decision =
/// $2, terminal_message = $3, worker_phase = 'resolve'`, `notify_pending` →
/// `completed_notified`, `completed_noop`, `failed` с `last_error_stage`.
impl Store {
    pub async fn hook_run_insert(&self, run_id: &str, source_ref: &str, lane_key: &str, metadata: &Value) -> Result<()> {
        self.run_insert("event_hook", run_id, source_ref, lane_key, metadata).await
    }

    /// Новый прогон `worker_kind` (`event_hook`, `verification`).
    pub async fn run_insert(&self, worker_kind: &str, run_id: &str, source_ref: &str, lane_key: &str, metadata: &Value) -> Result<()> {
        sqlx::query(
            "insert into runtime.execute_resolve_runs as existing (run_id, worker_kind, source_ref, lane_key, status, metadata_json) values ($1, $5, $2, $3, 'pending', $4) \
             on conflict (worker_kind, source_ref) do nothing",
        )
        .bind(run_id)
        .bind(source_ref)
        .bind(lane_key)
        .bind(metadata)
        .bind(worker_kind)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Взять прогон работником; `false` — уже взят или закрыт.
    pub async fn hook_run_claim(&self, run_id: &str, agent_id: &str, execute_message_id: &str, resolve_message_id: &str) -> Result<bool> {
        let n = sqlx::query(
            "update runtime.execute_resolve_runs set worker_generation = worker_generation + 1, worker_phase = 'execute', worker_agent_id = $2, worker_started_at = now(), \
             execute_message_id = $3, resolve_message_id = $4, updated_at = now() where run_id = $1 and status = 'pending' and worker_agent_id is null",
        )
        .bind(run_id)
        .bind(agent_id)
        .bind(execute_message_id)
        .bind(resolve_message_id)
        .execute(&self.pool)
        .await
        .map_err(db)?
        .rows_affected();
        Ok(n == 1)
    }

    pub async fn hook_run_phase(&self, run_id: &str, phase: &str) -> Result<()> {
        sqlx::query("update runtime.execute_resolve_runs set worker_phase = $2, updated_at = now() where run_id = $1 and status = 'pending'").bind(run_id).bind(phase).execute(&self.pool).await.map_err(db)?;
        Ok(())
    }

    /// Решение работника: `notify_main_agent` → `notify_pending`,
    /// `nothing_to_do` → `completed_noop`.
    pub async fn hook_run_decide(&self, run_id: &str, decision: &str, message: Option<&str>) -> Result<()> {
        let status = if decision == "notify_main_agent" { "notify_pending" } else { "completed_noop" };
        sqlx::query(
            "update runtime.execute_resolve_runs set terminal_decision = $2, terminal_message = $3, worker_phase = 'resolve', status = $4, \
             handoff_message_id = case when $4 = 'notify_pending' then run_id || ':handoff' else handoff_message_id end, updated_at = now() where run_id = $1 and status = 'pending'",
        )
        .bind(run_id)
        .bind(decision)
        .bind(message)
        .bind(status)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    pub async fn hook_run_notified(&self, run_id: &str) -> Result<()> {
        sqlx::query("update runtime.execute_resolve_runs set status = 'completed_notified', last_error_stage = null, last_error_message = null, updated_at = now() where run_id = $1 and status = 'notify_pending'")
            .bind(run_id)
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    pub async fn hook_run_fail(&self, run_id: &str, stage: &str, message: &str) -> Result<()> {
        sqlx::query("update runtime.execute_resolve_runs set status = 'failed', last_error_stage = $2, last_error_message = $3, updated_at = now() where run_id = $1 and status in ('pending', 'notify_pending')")
            .bind(run_id)
            .bind(stage)
            .bind(message)
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    /// Незакрытые прогоны наблюдателей (`pending`, `notify_pending`).
    pub async fn hook_runs_open(&self) -> Result<Vec<HookRun>> {
        let rows = sqlx::query("select run_id, status, terminal_message, worker_agent_id, metadata_json from runtime.execute_resolve_runs where worker_kind = 'event_hook' and status in ('pending', 'notify_pending') order by created_at, run_id")
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows
            .iter()
            .map(|r| HookRun {
                run_id: r.get("run_id"),
                status: r.get("status"),
                terminal_message: r.get("terminal_message"),
                worker_agent_id: r.get("worker_agent_id"),
                metadata: r.get("metadata_json"),
            })
            .collect())
    }
}
