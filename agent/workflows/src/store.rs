//! Хранение хода работ — запросы по форме ядра muse
//! (`strings-all.txt:196000-196540`): запуски, фазы, вызовы `agent()`.

use serde_json::{json, Value};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;

const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_workflows.sql")];

fn db(e: sqlx::Error) -> String {
    format!("workflow store: {e}")
}

/// Конечные состояния запуска (`… in ('stopped', 'completed', 'failed',
/// 'syntax_error')`).
pub const TERMINAL: [&str; 4] = ["stopped", "completed", "failed", "syntax_error"];

#[derive(Debug, Clone)]
pub struct Run {
    pub run_id: String,
    pub launch_mode: String,
    pub resume_from_run_id: Option<String>,
    pub workflow_name: String,
    pub description: String,
    pub phases: Value,
    pub args: Value,
    pub workspace_root: String,
    pub script_path: String,
    pub script_sha256: String,
    pub recovery_count: i32,
    pub last_recovery_reason: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub final_result: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub handoff_generation: i32,
    pub handoff_delivered: bool,
}

#[derive(Debug, Clone)]
pub struct Phase {
    pub phase_run_id: String,
    pub name: String,
    pub ordinal: i32,
    pub status: String,
    pub agent_total: i32,
    pub agent_completed: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Call {
    pub call_id: String,
    pub phase_run_id: Option<String>,
    pub replay_key: String,
    pub cache_key: String,
    pub call_ordinal: i32,
    pub prompt: String,
    pub options: Value,
    pub child_agent_id: Option<String>,
    pub status: String,
    pub cached_from_call_id: Option<String>,
    pub final_response: Option<String>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
}

fn ts(r: &PgRow, c: &str) -> Option<String> {
    r.get::<Option<chrono::DateTime<chrono::Utc>>, _>(c).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn run(r: &PgRow) -> Run {
    Run {
        run_id: r.get("run_id"),
        launch_mode: r.get("launch_mode"),
        resume_from_run_id: r.get("resume_from_run_id"),
        workflow_name: r.get("workflow_name"),
        description: r.get("description"),
        phases: r.get("phases_json"),
        args: r.get("args_json"),
        workspace_root: r.get("workspace_root"),
        script_path: r.get("script_path"),
        script_sha256: r.get("script_sha256"),
        recovery_count: r.get("recovery_count"),
        last_recovery_reason: r.get("last_recovery_reason"),
        status: r.get("status"),
        error: r.get("error"),
        final_result: r.get("final_result"),
        created_at: ts(r, "created_at").unwrap_or_default(),
        started_at: ts(r, "started_at"),
        updated_at: ts(r, "updated_at").unwrap_or_default(),
        completed_at: ts(r, "completed_at"),
        handoff_generation: r.get("terminal_handoff_generation"),
        handoff_delivered: r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("terminal_handoff_delivered_at").is_some(),
    }
}

fn phase(r: &PgRow) -> Phase {
    Phase {
        phase_run_id: r.get("phase_run_id"),
        name: r.get("phase_name"),
        ordinal: r.get("ordinal"),
        status: r.get("status"),
        agent_total: r.get("agent_total"),
        agent_completed: r.get("agent_completed"),
        error: r.get("error"),
    }
}

fn call(r: &PgRow) -> Call {
    Call {
        call_id: r.get("call_id"),
        phase_run_id: r.get("phase_run_id"),
        replay_key: r.get("replay_key"),
        cache_key: r.get("cache_key"),
        call_ordinal: r.get("call_ordinal"),
        prompt: r.get("prompt"),
        options: r.get("options_json"),
        child_agent_id: r.get("child_agent_id"),
        status: r.get("status"),
        cached_from_call_id: r.get("cached_from_call_id"),
        final_response: r.get("final_response"),
        duration_ms: r.get("duration_ms"),
        error: r.get("error"),
    }
}

/// Новый запуск.
pub struct NewRun<'a> {
    pub run_id: &'a str,
    pub launch_mode: &'a str,
    pub resume_from_run_id: Option<&'a str>,
    pub workflow_name: &'a str,
    pub description: &'a str,
    pub phases: &'a Value,
    pub args: &'a Value,
    pub workspace_root: &'a str,
    pub script_path: &'a str,
    pub script_sha256: &'a str,
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub async fn connect(url: &str) -> Result<Self, String> {
        let pool = PgPoolOptions::new().max_connections(4).connect(url).await.map_err(db)?;
        for m in MIGRATIONS {
            sqlx::raw_sql(m).execute(&pool).await.map_err(db)?;
        }
        Ok(Store { pool })
    }

    /// Запуск и его начальные фазы (`admit_run_with_initial_phases`).
    pub async fn admit(&self, n: NewRun<'_>, phase_names: &[String]) -> Result<(), String> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query(
            "insert into runtime.workflow_runs (run_id, launch_mode, resume_from_run_id, workflow_name, description, phases_json, args_json, workspace_root, script_path, script_sha256, status) \
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'queued')",
        )
        .bind(n.run_id)
        .bind(n.launch_mode)
        .bind(n.resume_from_run_id)
        .bind(n.workflow_name)
        .bind(n.description)
        .bind(n.phases)
        .bind(n.args)
        .bind(n.workspace_root)
        .bind(n.script_path)
        .bind(n.script_sha256)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        for (i, p) in phase_names.iter().enumerate() {
            sqlx::query("insert into runtime.workflow_phase_runs (phase_run_id, run_id, phase_name, ordinal, status) values ($1, $2, $3, $4, 'pending')")
                .bind(format!("{}:phase:{i}", n.run_id))
                .bind(n.run_id)
                .bind(p)
                .bind(i as i32)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        }
        tx.commit().await.map_err(db)
    }

    pub async fn run(&self, run_id: &str) -> Result<Option<Run>, String> {
        let r = sqlx::query("select * from runtime.workflow_runs where run_id = $1").bind(run_id).fetch_optional(&self.pool).await.map_err(db)?;
        Ok(r.as_ref().map(run))
    }

    pub async fn runs(&self, limit: i64) -> Result<Vec<Run>, String> {
        let rows = sqlx::query("select * from runtime.workflow_runs order by updated_at desc, run_id asc limit $1").bind(limit).fetch_all(&self.pool).await.map_err(db)?;
        Ok(rows.iter().map(run).collect())
    }

    /// Запуски, прерванные остановкой службы (`status in ('queued',
    /// 'running', 'stopping')`).
    pub async fn unfinished(&self) -> Result<Vec<Run>, String> {
        let rows = sqlx::query("select * from runtime.workflow_runs where status in ('queued', 'running', 'stopping') order by updated_at asc, run_id asc")
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(run).collect())
    }

    /// queued → running (`… where run_id = $1 and status = 'queued'`).
    pub async fn start(&self, run_id: &str) -> Result<bool, String> {
        let r = sqlx::query(
            "update runtime.workflow_runs set status = 'running', error = null, final_result = null, started_at = coalesce(started_at, now()), completed_at = null, updated_at = now() \
             where run_id = $1 and status = 'queued'",
        )
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(r.rows_affected() == 1)
    }

    /// Смена состояния при условии прежнего (`and ($2::text is null or
    /// status = $2)`).
    pub async fn set_status(&self, run_id: &str, from: Option<&str>, to: &str, error: Option<&str>, final_result: Option<&str>) -> Result<Option<Run>, String> {
        let r = sqlx::query(
            "update runtime.workflow_runs set status = $3, error = $4, final_result = $5, \
             started_at = case when $3 in ('running', 'stopping', 'stopped', 'completed', 'failed') then coalesce(started_at, now()) else started_at end, \
             completed_at = case when $3 in ('stopped', 'completed', 'failed', 'syntax_error') then coalesce(completed_at, now()) else completed_at end, \
             updated_at = now() where run_id = $1 and ($2::text is null or status = $2) returning *",
        )
        .bind(run_id)
        .bind(from)
        .bind(to)
        .bind(error)
        .bind(final_result)
        .fetch_optional(&self.pool)
        .await
        .map_err(db)?;
        Ok(r.as_ref().map(run))
    }

    /// paused → queued; у фонового запуска — новое поколение передачи.
    pub async fn resume(&self, run_id: &str) -> Result<Option<Run>, String> {
        let r = sqlx::query(
            "update runtime.workflow_runs set status = 'queued', error = null, final_result = null, \
             terminal_handoff_message_id = case when launch_mode = 'async' then null else terminal_handoff_message_id end, \
             terminal_handoff_generation = terminal_handoff_generation + case when launch_mode = 'async' then 1 else 0 end, \
             terminal_handoff_delivered_at = case when launch_mode = 'async' then null else terminal_handoff_delivered_at end, \
             terminal_handoff_disposition = case when launch_mode = 'async' then null else terminal_handoff_disposition end, \
             updated_at = now() where run_id = $1 and status = 'paused' returning *",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db)?;
        Ok(r.as_ref().map(run))
    }

    /// running → queued для повтора после рестарта.
    pub async fn requeue(&self, run_id: &str, reason: &str) -> Result<(), String> {
        sqlx::query(
            "update runtime.workflow_runs set status = 'queued', recovery_count = recovery_count + 1, last_recovery_reason = $2, last_recovered_at = now(), updated_at = now() \
             where run_id = $1 and status in ('queued', 'running')",
        )
        .bind(run_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Отметить передачу итога доставленной; `None` — уже доставлена или
    /// передавать нечего (форма `terminal_handoff_*`).
    pub async fn deliver(&self, run_id: &str) -> Result<Option<Run>, String> {
        let r = sqlx::query(
            "update runtime.workflow_runs set terminal_handoff_message_id = coalesce(terminal_handoff_message_id, \
             'workflow-terminal:' || run_id || case when terminal_handoff_generation > 0 then ':resume:' || terminal_handoff_generation::text else '' end), \
             terminal_handoff_delivered_at = coalesce(terminal_handoff_delivered_at, now()), terminal_handoff_disposition = 'delivered' \
             where run_id = $1 and launch_mode = 'async' and terminal_handoff_delivered_at is null and terminal_handoff_disposition is null \
             and (status in ('completed', 'failed', 'syntax_error', 'stopped') or (status = 'paused' and final_result like '%\"status\":\"blocked\"%')) returning *",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db)?;
        Ok(r.as_ref().map(run))
    }

    pub async fn phases(&self, run_id: &str) -> Result<Vec<Phase>, String> {
        let rows = sqlx::query("select * from runtime.workflow_phase_runs where run_id = $1 order by ordinal asc").bind(run_id).fetch_all(&self.pool).await.map_err(db)?;
        Ok(rows.iter().map(phase).collect())
    }

    /// Событие фазы (`upsert_phase_run`).
    pub async fn phase_event(&self, run_id: &str, name: &str, status: &str, error: Option<&str>) -> Result<(), String> {
        sqlx::query(
            "update runtime.workflow_phase_runs set status = $3, error = $4, \
             started_at = case when $3 = 'running' then coalesce(started_at, now()) else started_at end, \
             completed_at = case when $3 in ('completed', 'failed') then now() else completed_at end, updated_at = now() \
             where run_id = $1 and phase_name = $2",
        )
        .bind(run_id)
        .bind(name)
        .bind(status)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Пересчёт вызовов фазы.
    async fn recount(&self, phase_run_id: Option<&str>) -> Result<(), String> {
        let Some(p) = phase_run_id else { return Ok(()) };
        sqlx::query(
            "update runtime.workflow_phase_runs set agent_total = (select count(*) from runtime.workflow_agent_calls where phase_run_id = $1), \
             agent_completed = (select count(*) from runtime.workflow_agent_calls where phase_run_id = $1 and status in ('completed', 'cached')), updated_at = now() \
             where phase_run_id = $1",
        )
        .bind(p)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    pub async fn calls(&self, run_id: &str) -> Result<Vec<Call>, String> {
        let rows = sqlx::query("select * from runtime.workflow_agent_calls where run_id = $1 order by call_ordinal asc, call_id asc").bind(run_id).fetch_all(&self.pool).await.map_err(db)?;
        Ok(rows.iter().map(call).collect())
    }

    pub async fn call(&self, run_id: &str, replay_key: &str) -> Result<Option<Call>, String> {
        let r = sqlx::query("select * from runtime.workflow_agent_calls where run_id = $1 and replay_key = $2").bind(run_id).bind(replay_key).fetch_optional(&self.pool).await.map_err(db)?;
        Ok(r.as_ref().map(call))
    }

    /// Готовый вызов источника для `resumeFromRunId` — по `cache_key`.
    pub async fn cached(&self, run_id: &str, cache_key: &str) -> Result<Option<Call>, String> {
        let r = sqlx::query("select * from runtime.workflow_agent_calls where run_id = $1 and cache_key = $2 and status in ('completed', 'cached') order by call_ordinal asc, call_id asc limit 1")
            .bind(run_id)
            .bind(cache_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(r.as_ref().map(call))
    }

    /// Новый вызов (`on conflict (run_id, replay_key) do nothing`).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_call(&self, call_id: &str, run_id: &str, phase_run_id: Option<&str>, replay_key: &str, cache_key: &str, prompt: &str, options: &Value, status: &str, cached_from: Option<&Call>) -> Result<Call, String> {
        sqlx::query(
            "insert into runtime.workflow_agent_calls (call_id, run_id, phase_run_id, replay_key, cache_key, call_ordinal, prompt, options_json, status, cached_from_call_id, final_response, completed_at) \
             values ($1, $2, $3, $4, $5, (select coalesce(max(call_ordinal) + 1, 0) from runtime.workflow_agent_calls where run_id = $2), $6, $7, $8, $9, $10, case when $8 = 'cached' then now() end) \
             on conflict (run_id, replay_key) do nothing",
        )
        .bind(call_id)
        .bind(run_id)
        .bind(phase_run_id)
        .bind(replay_key)
        .bind(cache_key)
        .bind(prompt)
        .bind(options)
        .bind(status)
        .bind(cached_from.map(|c| c.call_id.as_str()))
        .bind(cached_from.and_then(|c| c.final_response.as_deref()))
        .execute(&self.pool)
        .await
        .map_err(db)?;
        self.recount(phase_run_id).await?;
        self.call(run_id, replay_key).await?.ok_or_else(|| "workflow child owner disappeared during transition".into())
    }

    /// Повтор вызова: blocked / прерванный → pending (`requeue blocked
    /// workflow agent call`).
    pub async fn requeue_call(&self, call_id: &str) -> Result<(), String> {
        sqlx::query(
            "update runtime.workflow_agent_calls set status = 'pending', child_agent_id = null, cached_from_call_id = null, final_response = null, \
             input_tokens = 0, output_tokens = 0, tool_call_count = 0, duration_ms = null, error = null, created_at = now(), started_at = null, completed_at = null, updated_at = now() \
             where call_id = $1",
        )
        .bind(call_id)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// pending → running с подагентом.
    pub async fn call_running(&self, call_id: &str, child: &str) -> Result<(), String> {
        sqlx::query("update runtime.workflow_agent_calls set status = 'running', child_agent_id = $2, started_at = coalesce(started_at, now()), updated_at = now() where call_id = $1")
            .bind(call_id)
            .bind(child)
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    /// Итог вызова.
    pub async fn finish_call(&self, c: &Call, status: &str, final_response: Option<&str>, duration_ms: i64, error: Option<&str>) -> Result<(), String> {
        sqlx::query(
            "update runtime.workflow_agent_calls set status = $2, final_response = $3, duration_ms = $4, error = $5, started_at = coalesce(started_at, now()), \
             completed_at = case when $2 in ('completed', 'failed', 'stopped', 'cached') then coalesce(completed_at, now()) else completed_at end, updated_at = now() \
             where call_id = $1",
        )
        .bind(&c.call_id)
        .bind(status)
        .bind(final_response)
        .bind(duration_ms)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        self.recount(c.phase_run_id.as_deref()).await
    }

    /// Незаконченные вызовы запуска — в `status` (пауза: pending, стоп:
    /// stopped).
    pub async fn settle_open_calls(&self, run_id: &str, status: &str, error: Option<&str>) -> Result<(), String> {
        sqlx::query(
            "update runtime.workflow_agent_calls set status = $2, error = $3, child_agent_id = case when $2 = 'pending' then null else child_agent_id end, updated_at = now(), \
             completed_at = case when $2 = 'stopped' then coalesce(completed_at, now()) else completed_at end \
             where run_id = $1 and status in ('pending', 'running')",
        )
        .bind(run_id)
        .bind(status)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }
}

impl Run {
    pub fn is_terminal(&self) -> bool {
        TERMINAL.contains(&self.status.as_str())
    }

    /// Пауза из-за `blocked` (`final_result like '%"status":"blocked"%'`).
    pub fn blocked(&self) -> Option<Value> {
        let v: Value = serde_json::from_str(self.final_result.as_deref()?).ok()?;
        (self.status == "paused" && v["status"] == "blocked").then_some(v)
    }

    /// Итог для `finalResult`: JSON итога сценария, если разбирается.
    pub fn final_value(&self) -> Value {
        match self.final_result.as_deref() {
            Some(s) => serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.into())),
            None => Value::Null,
        }
    }

    pub fn json(&self) -> Value {
        json!({
            "runId": self.run_id,
            "workflowName": self.workflow_name,
            "description": self.description,
            "status": self.status,
            "launchMode": self.launch_mode,
            "resumeFromRunId": self.resume_from_run_id,
            "scriptPath": self.script_path,
            "scriptSha256": self.script_sha256,
            "createdAt": self.created_at,
            "startedAt": self.started_at,
            "updatedAt": self.updated_at,
            "completedAt": self.completed_at,
            "error": self.error,
            "finalResult": self.final_value(),
            "recoveryCount": self.recovery_count,
            "lastRecoveryReason": self.last_recovery_reason,
        })
    }
}
