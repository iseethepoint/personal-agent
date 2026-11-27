//! `agent-history` — история агентов в Postgres, как у muse
//! (`session/impl_session/history_persistence/*`, `hatch-db-conversations`
//! › `agent_state`): каждое сообщение хода — строка `agent.context_items`
//! по `(agent_id, seq)`, запись по мере хода; граница текущей подсказки —
//! `agent.agents.prompt_floor_seq` (сжатие пишет новую историю после
//! старой и сдвигает границу); закреплённое — `agent.volatile_context_pins`.
//! Запрос записи — форма ядра (`INSERT INTO context_items (agent_id, seq,
//! item_kind, text_content, role, call_id, tool_name, success,
//! message_source, created_at, item_json, …) … ON CONFLICT (agent_id, seq)
//! DO UPDATE`, около `0x6cb97b`); граница — `SELECT COALESCE(MAX(seq), -1)
//! FROM context_items WHERE agent_id = $1`.
//!
//! Строка — одна часть сообщения, части одного сообщения связывает
//! `message_id` (как у ядра: `item_kind` — `message`, `message_text`,
//! `text_segments`, `tool_call`, `tool_output`; `item_json` с `parts` или
//! одной частью, `jsonb_array_elements(item -> 'parts')` около `0x16f8c7b`).
//! Мышление — отдельные строки: каталог ядра скрывает у `context_items`
//! «rows holding model reasoning (thinking and redacted-thinking items)»
//! видом `inspection.context_items`.
//!
//! Своё (в разборе не видно или у muse иначе устроено):
//! - вид части: текст — `message_text`, мышление, скрытое мышление и
//!   картинка — `message`, вызов — `tool_call`, результат — `tool_output`;
//!   `item_json` — наша часть как есть (мышление — с подписью);
//!   `text_segments` (длинный текст в `agent.context_text_segments`) не
//!   используется — порога в разборе нет;
//! - сообщение без частей — одна строка `message` с `item_json = null`;
//! - `message_id` — `<agent_id>:<seq первой части>`;
//! - заметка изменчивого помечается `message_source = 'volatile'`, по ней
//!   после перезапуска восстанавливается последняя заметка;
//! - начало подсказки (системный текст и описания инструментов)
//!   закрепляется пином `prompt_prefix` и после перезапуска берётся из него
//!   байт в байт.

use std::sync::Arc;

use agent_loop::{ChildRecord, Mailbox, Prefix, Record, SavedSubagent, Status, SubagentStore, WorkerStore};
use agent_model::{Message, Part, Role, ToolSpec};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

const MIGRATIONS: [&str; 7] = [
    include_str!("../migrations/0001_history.sql"),
    include_str!("../migrations/0002_mailbox.sql"),
    include_str!("../migrations/0003_todo.sql"),
    include_str!("../migrations/0004_compactions.sql"),
    include_str!("../migrations/0005_restart.sql"),
    include_str!("../migrations/0006_subagents.sql"),
    include_str!("../migrations/0007_recovery_owners.sql"),
];
/// Режимы точки перезапуска (значения ядра).
const BEFORE_INFERENCE: &str = "before_inference";
const IDLE: &str = "idle";
/// Причина сжатия в `agent_compactions.trigger`. Своё: значения в разборе не
/// видно; рядом — «context-pressure compaction failed…» ядра.
const COMPACTION_TRIGGER: &str = "context_pressure";
const PREFIX_PIN: &str = "prompt_prefix";
const VOLATILE: &str = "volatile";

pub type Now = Arc<dyn Fn() -> i64 + Send + Sync>;

fn db(e: sqlx::Error) -> String {
    format!("history: {e}")
}

/// Подключение и миграция.
#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub async fn connect(url: &str) -> Result<Store, String> {
        let pool = PgPoolOptions::new().max_connections(4).connect(url).await.map_err(db)?;
        for m in MIGRATIONS {
            sqlx::raw_sql(m).execute(&pool).await.map_err(db)?;
        }
        Ok(Store { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Serialize, Deserialize)]
struct PrefixJson {
    system: String,
    tools: Vec<ToolSpec>,
}

/// Восстановленный разговор.
pub struct Saved {
    pub prefix: Prefix,
    pub history: Vec<Message>,
    pub last_volatile: Option<String>,
}

struct State {
    /// `seq` первой строки текущей подсказки.
    floor: i64,
    /// Следующий свободный `seq`.
    next: i64,
    /// Сколько сообщений текущей подсказки уже записано.
    written: usize,
    /// `seq` первой части каждого записанного сообщения текущей подсказки.
    starts: Vec<i64>,
}

/// История одного агента.
pub struct AgentHistory {
    pool: PgPool,
    agent_id: String,
    model: String,
    now: Now,
    state: tokio::sync::Mutex<State>,
}

fn text_of(m: &Message) -> String {
    m.content
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl AgentHistory {
    /// Строка агента заводится, если её нет; граница и число записанных —
    /// из базы.
    pub async fn open(store: &Store, agent_id: &str, kind: &str, model: &str, now: Now) -> Result<AgentHistory, String> {
        let secs = now() / 1000;
        sqlx::query("insert into agent.agents (agent_id, id, kind, model, status, created_at, updated_at) values ($1, $1, $2, $3, 'idle', $4, $4) on conflict (agent_id) do nothing")
            .bind(agent_id)
            .bind(kind)
            .bind(model)
            .bind(secs)
            .execute(&store.pool)
            .await
            .map_err(db)?;
        let floor: Option<i64> = sqlx::query_scalar("select prompt_floor_seq from agent.agents where agent_id = $1").bind(agent_id).fetch_one(&store.pool).await.map_err(db)?;
        let floor = floor.unwrap_or(0);
        let starts: Vec<i64> = sqlx::query_scalar("select min(seq) from agent.context_items where agent_id = $1 and seq >= $2 group by message_id order by 1")
            .bind(agent_id)
            .bind(floor)
            .fetch_all(&store.pool)
            .await
            .map_err(db)?;
        let max: Option<i64> = sqlx::query_scalar("select max(seq) from agent.context_items where agent_id = $1 and seq >= $2")
            .bind(agent_id)
            .bind(floor)
            .fetch_one(&store.pool)
            .await
            .map_err(db)?;
        let next = max.map_or(floor, |m| m + 1);
        let state = State { floor, next, written: starts.len(), starts };
        Ok(AgentHistory { pool: store.pool.clone(), agent_id: agent_id.into(), model: model.into(), now, state: tokio::sync::Mutex::new(state) })
    }

    /// Разговор из базы: закреплённое начало, история с границы, последняя
    /// заметка изменчивого. `None` — разговора ещё не было.
    pub async fn load(&self) -> Result<Option<Saved>, String> {
        let pin: Option<String> = sqlx::query_scalar("select variant_json from agent.volatile_context_pins where agent_id = $1 and variant_hash = $2")
            .bind(&self.agent_id)
            .bind(PREFIX_PIN)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        let Some(pin) = pin else { return Ok(None) };
        let p: PrefixJson = serde_json::from_str(&pin).map_err(|e| format!("history: prompt_prefix: {e}"))?;
        let floor = self.state.lock().await.floor;
        let rows = sqlx::query("select message_id, role::text as role, item_json, message_source from agent.context_items where agent_id = $1 and seq >= $2 order by seq")
            .bind(&self.agent_id)
            .bind(floor)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        let mut history: Vec<Message> = Vec::new();
        let mut last_volatile = None;
        let mut current: Option<String> = None;
        for r in rows {
            let id: Option<String> = r.get("message_id");
            if current.is_none() || id != current {
                let role = match r.get::<Option<String>, _>("role").as_deref() {
                    Some("assistant") => Role::Assistant,
                    Some("developer") => Role::Developer,
                    _ => Role::User,
                };
                history.push(Message { role, content: Vec::new() });
                current = id;
            }
            let json: String = r.get("item_json");
            let part: Option<Part> = serde_json::from_str(&json).map_err(|e| format!("history: item_json: {e}"))?;
            let m = history.last_mut().unwrap();
            m.content.extend(part);
            if r.get::<Option<String>, _>("message_source").as_deref() == Some(VOLATILE) {
                last_volatile = Some(text_of(m));
            }
        }
        Ok(Some(Saved { prefix: Prefix { system: p.system, tools: p.tools }, history, last_volatile }))
    }

    /// Закрепить начало подсказки.
    pub async fn pin_prefix(&self, prefix: &Prefix) -> Result<(), String> {
        let json = serde_json::to_string(&PrefixJson { system: prefix.system.clone(), tools: prefix.tools.clone() }).map_err(|e| e.to_string())?;
        let floor = self.state.lock().await.floor;
        sqlx::query(
            "insert into agent.volatile_context_pins (agent_id, variant_hash, variant_json, prompt_floor_seq, checkpoint_seq) values ($1, $2, $3, $4, -1) \
             on conflict (agent_id, variant_hash) do update set variant_json = excluded.variant_json, prompt_floor_seq = excluded.prompt_floor_seq, created_at = now()",
        )
        .bind(&self.agent_id)
        .bind(PREFIX_PIN)
        .bind(json)
        .bind(floor)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Дописать то, что ещё не записано. `volatile` — текущая заметка
    /// изменчивого: такое сообщение помечается.
    pub async fn sync(&self, history: &[Message], volatile: Option<&str>) -> Result<(), String> {
        let mut st = self.state.lock().await;
        while st.written < history.len() {
            let m = &history[st.written];
            let start = st.next;
            st.next = self.insert(start, m, volatile).await?;
            st.starts.push(start);
            st.written += 1;
        }
        Ok(())
    }

    /// История заменена (сжатие): новая пишется после старой, граница
    /// подсказки сдвигается на её начало, сжатие — строка
    /// `agent.agent_compactions` (`checkpoint_seq` — новая граница,
    /// `first_kept_seq` — `seq` первого оставленного сообщения старой
    /// истории, `details_json` — история, которой заменён разговор).
    pub async fn replace(&self, history: &[Message], volatile: Option<&str>, compacted: &agent_loop::Compacted) -> Result<(), String> {
        {
            let mut st = self.state.lock().await;
            let max: i64 = sqlx::query_scalar("select coalesce(max(seq), -1) from agent.context_items where agent_id = $1")
                .bind(&self.agent_id)
                .fetch_one(&self.pool)
                .await
                .map_err(db)?;
            let first_kept = st.starts.get(compacted.first_kept).copied();
            st.floor = max + 1;
            st.next = st.floor;
            st.written = 0;
            st.starts.clear();
            let secs = (self.now)() / 1000;
            let details = serde_json::to_string(&serde_json::json!({"replacement_history": history})).map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO agent.agent_compactions (agent_id, summary, first_kept_seq, checkpoint_seq, tokens_before, tokens_after, details_json, has_replacement_history, trigger, will_retry, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, false, $9)",
            )
            .bind(&self.agent_id)
            .bind(&compacted.summary)
            .bind(first_kept)
            .bind(st.floor)
            .bind(compacted.tokens_before as i64)
            .bind(compacted.tokens_after as i64)
            .bind(details)
            .bind(COMPACTION_TRIGGER)
            .bind(secs)
            .execute(&self.pool)
            .await
            .map_err(db)?;
            sqlx::query("update agent.agents set prompt_floor_seq = $2, updated_at = $3 where agent_id = $1")
                .bind(&self.agent_id)
                .bind(st.floor)
                .bind(secs)
                .execute(&self.pool)
                .await
                .map_err(db)?;
        }
        self.sync(history, volatile).await
    }

    /// Счёт поставщика за последний записанный ответ модели —
    /// `INSERT INTO agent_message_token_usage … ON CONFLICT(agent_id,
    /// message_id) DO UPDATE` (ядро, около `0x1700cb4`). Своё: `input_tokens`
    /// — вход вместе с чтением и записью кэша.
    pub async fn record_usage(&self, usage: &agent_model::Usage) -> Result<(), String> {
        let st = self.state.lock().await;
        let Some(start) = st.starts.last() else { return Ok(()) };
        sqlx::query(
            "INSERT INTO agent.agent_message_token_usage (agent_id, message_id, input_tokens, output_tokens, created_at) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT(agent_id, message_id) DO UPDATE SET input_tokens = excluded.input_tokens, output_tokens = excluded.output_tokens",
        )
        .bind(&self.agent_id)
        .bind(format!("{}:{start}", self.agent_id))
        .bind((usage.input_tokens + usage.cache_read_tokens + usage.cache_write_tokens) as i64)
        .bind(usage.output_tokens as i64)
        .bind((self.now)() / 1000)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Записать сообщение по частям с `seq`; вернуть следующий свободный.
    async fn insert(&self, seq: i64, m: &Message, volatile: Option<&str>) -> Result<i64, String> {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Developer => "developer",
        };
        let message_id = format!("{}:{seq}", self.agent_id);
        let source = (m.role == Role::Developer && volatile.is_some_and(|v| v == text_of(m))).then_some(VOLATILE);
        let empty: [Option<&Part>; 1] = [None];
        let parts: Vec<Option<&Part>> = if m.content.is_empty() { empty.to_vec() } else { m.content.iter().map(Some).collect() };
        let mut next = seq;
        for part in parts {
            let (kind, text, call_id, tool_name, success): (&str, Option<&str>, Option<&str>, Option<&str>, Option<bool>) = match part {
                Some(Part::Text { text }) => ("message_text", Some(text), None, None, None),
                Some(Part::ToolCall { id, name, .. }) => ("tool_call", None, Some(id), Some(name), None),
                Some(Part::ToolResult { call_id, is_error, .. }) => ("tool_output", None, Some(call_id), None, Some(!*is_error)),
                Some(Part::Thinking { text, .. }) => ("message", Some(text), None, None, None),
                Some(Part::RedactedThinking { .. }) | Some(Part::Image(_)) | None => ("message", None, None, None, None),
            };
            let json = serde_json::to_string(&part).map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO agent.context_items (agent_id, seq, item_kind, message_id, text_content, role, call_id, tool_name, success, message_source, created_at, item_json) \
                 VALUES ($1, $2, $3::agent.context_item_kind, $4, $5, $6::runtime.message_role, $7, $8, $9, $10, to_timestamp($11::double precision), $12) \
                 ON CONFLICT (agent_id, seq) DO UPDATE SET item_kind = excluded.item_kind, message_id = excluded.message_id, text_content = excluded.text_content, role = excluded.role, \
                 call_id = excluded.call_id, tool_name = excluded.tool_name, success = excluded.success, \
                 message_source = COALESCE(excluded.message_source, context_items.message_source), created_at = excluded.created_at, item_json = excluded.item_json",
            )
            .bind(&self.agent_id)
            .bind(next)
            .bind(kind)
            .bind(&message_id)
            .bind(text)
            .bind(role)
            .bind(call_id)
            .bind(tool_name)
            .bind(success)
            .bind(source)
            .bind((self.now)() as f64 / 1000.0)
            .bind(json)
            .execute(&self.pool)
            .await
            .map_err(db)?;
            next += 1;
        }
        Ok(next)
    }
}

/// Точка перезапуска агента в `agent.runtime_restart_checkpoints` —
/// запросы ядра: запись `INSERT … SELECT … WHERE EXISTS (SELECT 1 FROM agents
/// WHERE id = $1 AND status NOT IN ('shutdown', 'not_found')) ON CONFLICT
/// (agent_id) DO UPDATE` (около `0x1711918`), чтение `SELECT payload_json …
/// WHERE agent_id = $1 AND mode = $2` (около `0x1710f52`). Ход пишет
/// `before_inference` с payload, между ходами — `idle` без payload; у
/// главного агента `recovery_class = 'foreground_root'`, `recovery_owner_id`
/// — его id, `execution_config_json` не пуст (запрос ядра около
/// `0x1729b52`). Своё: у подагента `recovery_class` пуст (значения в разборе
/// не видно).
///
/// Своё (в разборе не видно): payload — `{"message_id": <имя хода>}`, вести
/// хода связаны с ним через `stream_owner_message_id`;
/// `execution_config_json` — `{"model": …}`.
impl AgentHistory {
    /// `Some(ход)` — ход идёт (`before_inference`), `None` — между ходами
    /// (`idle`).
    pub async fn checkpoint(&self, recovery_class: Option<&str>, owner: Option<&str>, message_id: Option<&str>) -> Result<(), String> {
        let (mode, payload) = match message_id {
            Some(id) => (BEFORE_INFERENCE, Some(serde_json::json!({ "message_id": id }).to_string())),
            None => (IDLE, None),
        };
        let config = serde_json::json!({ "model": self.model }).to_string();
        sqlx::query(
            "insert into agent.runtime_restart_checkpoints (agent_id, mode, recovery_class, recovery_owner_id, execution_config_json, payload_json, created_at, updated_at) \
             select $1, $2, $3, coalesce($7, $1), $4, $5, $6, $6 where exists (select 1 from agent.agents where id = $1 and status not in ('shutdown', 'not_found')) \
             on conflict (agent_id) do update set mode = excluded.mode, recovery_class = excluded.recovery_class, recovery_owner_id = excluded.recovery_owner_id, \
             execution_config_json = excluded.execution_config_json, payload_json = excluded.payload_json, updated_at = excluded.updated_at",
        )
        .bind(&self.agent_id)
        .bind(mode)
        .bind(recovery_class)
        .bind(config)
        .bind(payload)
        .bind((self.now)())
        .bind(owner)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Ход, оборванный перезапуском: точка `before_inference`.
    pub async fn interrupted(&self) -> Result<Option<agent_loop::Checkpoint>, String> {
        let row: Option<(Option<String>, Option<String>)> =
            sqlx::query_as("select payload_json, recovery_class from agent.runtime_restart_checkpoints where agent_id = $1 and mode = $2")
                .bind(&self.agent_id)
                .bind(BEFORE_INFERENCE)
                .fetch_optional(&self.pool)
                .await
                .map_err(db)?;
        let Some((Some(json), class)) = row else { return Ok(None) };
        let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("history: payload_json: {e}"))?;
        Ok(v["message_id"].as_str().map(|m| agent_loop::Checkpoint { class, message_id: m.to_string() }))
    }

    /// Вести, взятые ходом, которого больше нет (не `owner`), — `aborted`
    /// (форма ядра `UPDATE agent.message_mailbox SET state = 'aborted',
    /// terminalized_at = now() WHERE agent_id = $1 AND state = 'attached'
    /// …`). Своё: отбор — `stream_owner_message_id` не равен ходу из точки.
    pub async fn abort_orphaned(&self, owner: Option<&str>) -> Result<u64, String> {
        let r = sqlx::query(
            "update agent.message_mailbox set state = 'aborted', checkpoint_custody = coalesce(checkpoint_custody, 'executing'), terminalized_at = now() \
             where agent_id = $1 and state = 'attached' and stream_owner_message_id is distinct from $2",
        )
        .bind(&self.agent_id)
        .bind(owner)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(r.rows_affected())
    }
}

impl Record for AgentHistory {
    fn record<'a>(&'a self, history: &'a [Message]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(self.sync(history, None))
    }

    fn usage<'a>(&'a self, usage: &'a agent_model::Usage) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(self.record_usage(usage))
    }
}

/// Ящик агента в `agent.message_mailbox` — запросы по форме ядра:
/// `INSERT INTO agent.message_mailbox (submission_id, agent_id,
/// conversation_epoch, submission_kind, routing_scope, state, …)`,
/// `UPDATE … SET state = 'attached', checkpoint_custody = 'executing',
/// stream_owner_message_id = $3, attached_at = now() WHERE … state =
/// 'accepted'`, `UPDATE … SET state = CASE WHEN $4 THEN 'completed' ELSE
/// 'aborted' END, terminalized_at = now() WHERE … state = 'attached'`,
/// `SELECT accepted_ordinal, submission_json … WHERE agent_id = $1 AND state
/// = 'accepted' ORDER BY accepted_ordinal`.
///
/// Своё (в разборе не видно): `submission_id` — `<agent_id>:mailbox:<ordinal>`,
/// `accepted_ordinal` — следующий по агенту, `conversation_epoch` — 0,
/// `routing_scope` — `root`, `submission_json` — сообщение. Вид подачи:
/// строка владельца — `user` (как в запросах ядра), весть службы — `notice`
/// (своё: у ядра внутренние подачи — варианты `SavedSubmissionKind`, их
/// значений в столбце в разборе не видно).
pub struct AgentMailbox {
    pool: PgPool,
    agent_id: String,
}

impl AgentMailbox {
    pub fn new(store: &Store, agent_id: &str) -> AgentMailbox {
        AgentMailbox { pool: store.pool.clone(), agent_id: agent_id.into() }
    }
}

type Fut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, String>> + Send + 'a>>;

impl Mailbox for AgentMailbox {
    fn accept<'a>(&'a self, m: &'a Message, kind: &'a str) -> Fut<'a, String> {
        Box::pin(async move {
            let json = serde_json::to_string(m).map_err(|e| e.to_string())?;
            sqlx::query_scalar(
                "with next as (select coalesce(max(accepted_ordinal), 0) + 1 as ord from agent.message_mailbox where agent_id = $1 and conversation_epoch = 0) \
                 insert into agent.message_mailbox (submission_id, accepted_ordinal, agent_id, conversation_epoch, submission_kind, routing_scope, state, submission_json) \
                 select $1 || ':mailbox:' || ord, ord, $1, 0, $3, 'root', 'accepted', $2 from next returning submission_id",
            )
            .bind(&self.agent_id)
            .bind(json)
            .bind(kind)
            .fetch_one(&self.pool)
            .await
            .map_err(db)
        })
    }

    fn attach<'a>(&'a self, ids: &'a [String], owner: &'a str) -> Fut<'a, ()> {
        Box::pin(async move {
            sqlx::query(
                "update agent.message_mailbox set state = 'attached', checkpoint_custody = 'executing', stream_owner_message_id = $3, attached_at = now() \
                 where submission_id = any($1) and agent_id = $2 and state = 'accepted' and stream_owner_message_id is null",
            )
            .bind(ids)
            .bind(&self.agent_id)
            .bind(owner)
            .execute(&self.pool)
            .await
            .map_err(db)?;
            Ok(())
        })
    }

    fn complete<'a>(&'a self, owner: &'a str, ok: bool) -> Fut<'a, ()> {
        Box::pin(async move {
            sqlx::query(
                "update agent.message_mailbox set state = case when $3 then 'completed' else 'aborted' end, checkpoint_custody = coalesce(checkpoint_custody, 'executing'), terminalized_at = now() \
                 where agent_id = $1 and stream_owner_message_id = $2 and state = 'attached'",
            )
            .bind(&self.agent_id)
            .bind(owner)
            .bind(ok)
            .execute(&self.pool)
            .await
            .map_err(db)?;
            Ok(())
        })
    }

    fn pending<'a>(&'a self) -> Fut<'a, Vec<(String, Message)>> {
        Box::pin(async move {
            let rows: Vec<(String, String)> =
                sqlx::query_as("select submission_id, submission_json from agent.message_mailbox where agent_id = $1 and state = 'accepted' order by accepted_ordinal")
                    .bind(&self.agent_id)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(db)?;
            rows.into_iter()
                .map(|(id, json)| serde_json::from_str(&json).map(|m| (id, m)).map_err(|e| format!("history: submission_json: {e}")))
                .collect()
        })
    }
}

/// Список дел агента в `runtime.agent_todo_snapshots` — запросы ядра
/// (около `0x16f9853`): чтение снимка со сторожем, замена с
/// `revision + 1` и новым поколением при новом ключе, запись напоминания
/// условным `UPDATE`.
///
/// Своё: `conversation_id` и `agent_identity` — id агента (`root`);
/// `items_json` — `[{"text", "status"}]`.
pub struct AgentTodo {
    pool: PgPool,
    agent_id: String,
}

impl AgentTodo {
    pub fn new(store: &Store, agent_id: &str) -> AgentTodo {
        AgentTodo { pool: store.pool.clone(), agent_id: agent_id.into() }
    }
}

#[derive(Serialize, Deserialize)]
struct ItemJson {
    text: String,
    status: String,
}

impl agent_tools::TodoStore for AgentTodo {
    fn load<'a>(&'a self) -> Fut<'a, Option<agent_tools::Snapshot>> {
        Box::pin(async move {
            let row: Option<(String, Option<String>, i64, i16)> = sqlx::query_as(
                "SELECT items_json, continuation_key, continuation_generation, continuation_reminders FROM runtime.agent_todo_snapshots WHERE conversation_id = $1 AND agent_identity = $2",
            )
            .bind(&self.agent_id)
            .bind(&self.agent_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
            let Some((items, key, generation, reminders)) = row else { return Ok(None) };
            let items: Vec<ItemJson> = serde_json::from_str(&items).map_err(|e| format!("history: items_json: {e}"))?;
            Ok(Some(agent_tools::Snapshot {
                items: items.into_iter().map(|i| agent_tools::TodoItem { text: i.text, status: i.status }).collect(),
                continuation_key: key,
                continuation_generation: generation,
                continuation_reminders: reminders,
            }))
        })
    }

    fn replace<'a>(&'a self, items: &'a [agent_tools::TodoItem], continuation_key: Option<&'a str>) -> Fut<'a, ()> {
        Box::pin(async move {
            let json = serde_json::to_string(&items.iter().map(|i| ItemJson { text: i.text.clone(), status: i.status.clone() }).collect::<Vec<_>>()).map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO runtime.agent_todo_snapshots (conversation_id, agent_identity, items_json, revision, updated_at_ms, continuation_key, continuation_generation, continuation_reminders) \
                 VALUES ($1, $2, $3, 1, (extract(epoch from now()) * 1000)::bigint, $4, 1, 0) \
                 ON CONFLICT (conversation_id, agent_identity) DO UPDATE SET items_json = excluded.items_json, revision = agent_todo_snapshots.revision + 1, \
                 updated_at_ms = excluded.updated_at_ms, continuation_key = excluded.continuation_key, \
                 continuation_generation = CASE WHEN agent_todo_snapshots.continuation_key = excluded.continuation_key THEN agent_todo_snapshots.continuation_generation ELSE agent_todo_snapshots.continuation_generation + 1 END, \
                 continuation_reminders = CASE WHEN agent_todo_snapshots.continuation_key = excluded.continuation_key THEN agent_todo_snapshots.continuation_reminders ELSE 0 END",
            )
            .bind(&self.agent_id)
            .bind(&self.agent_id)
            .bind(json)
            .bind(continuation_key)
            .execute(&self.pool)
            .await
            .map_err(db)?;
            Ok(())
        })
    }

    fn record_reminder<'a>(&'a self, key: &'a str, generation: i64, reminders: i16) -> Fut<'a, bool> {
        Box::pin(async move {
            let n = sqlx::query(
                "UPDATE runtime.agent_todo_snapshots SET continuation_reminders = $5 WHERE conversation_id = $1 AND agent_identity = $2 \
                 AND continuation_key = $3 AND continuation_generation = $4 AND continuation_reminders < $5",
            )
            .bind(&self.agent_id)
            .bind(&self.agent_id)
            .bind(key)
            .bind(generation)
            .bind(reminders)
            .execute(&self.pool)
            .await
            .map_err(db)?
            .rows_affected();
            Ok(n == 1)
        })
    }
}

impl ChildRecord for AgentHistory {
    fn checkpoint<'a>(&'a self, class: Option<&'a str>, owner: Option<&'a str>, turn: Option<&'a str>) -> Fut<'a, ()> {
        Box::pin(AgentHistory::checkpoint(self, class, owner, turn))
    }

    fn interrupted<'a>(&'a self) -> Fut<'a, Option<agent_loop::Checkpoint>> {
        Box::pin(AgentHistory::interrupted(self))
    }

    fn load<'a>(&'a self) -> Fut<'a, (Prefix, Vec<Message>)> {
        Box::pin(async move {
            let saved = AgentHistory::load(self).await?.ok_or_else(|| format!("history: no prompt_prefix pin for {}", self.agent_id))?;
            Ok((saved.prefix, saved.history))
        })
    }
}

/// Значение статуса в `agent.agents.status` и `subagent_spawns.status` —
/// значения ядра (`pending_init`, `running`, `interrupted`, `completed`,
/// `errored`, `shutdown`).
fn status_str(s: Status) -> &'static str {
    match s {
        Status::Running => "running",
        Status::Done => "completed",
        Status::Interrupted => "interrupted",
        Status::Failed => "errored",
        Status::Closed => "shutdown",
    }
}

fn status_of(s: Option<&str>) -> Status {
    match s {
        Some("completed") => Status::Done,
        Some("interrupted") => Status::Interrupted,
        Some("errored") => Status::Failed,
        Some("shutdown") | Some("not_found") => Status::Closed,
        _ => Status::Running,
    }
}

/// Подагенты одного родителя в `agent.agents` (`kind = 'subagent'`) и
/// `agent.subagent_spawns` — запросы по форме ядра: строка агента `INSERT
/// INTO agents (agent_id, id, session_id, kind, parent_id, parent_agent_id,
/// root_session_id, depth, created_at, updated_at, model, status,
/// last_assistant_message) … ON CONFLICT (agent_id) DO NOTHING` (около
/// `0x1707f0c`), заведение `INSERT INTO subagent_spawns (…, child_depth,
/// created_at, status, prompt, …, agent_type, metadata_json, …)` (около
/// `0x6abfd4`), новая работа — `UPDATE subagent_spawns SET status =
/// 'running', deferred_terminal_status = NULL, final_response = NULL,
/// completed_at = NULL, seen_at = NULL` и `UPDATE agents SET … status =
/// 'running', last_assistant_message = NULL WHERE … status NOT IN
/// ('shutdown', 'not_found')` (около `0x16a2601`), конец — `UPDATE
/// subagent_spawns SET completed_at = COALESCE(completed_at, $1), status =
/// $2, deferred_terminal_status = NULL, final_response =
/// COALESCE(final_response, $3)` (около `0x8b76dd`) и `UPDATE agents SET
/// updated_at, status, last_assistant_message` (около `0x171a560`).
///
/// Своё (в разборе не видно): `metadata_json` — `{"finish_hook": true}`, если
/// итог проходит через крючок родителя; `seen_at` не ведётся (итог сразу
/// уходит в ящик родителя); глубина подагента — 1.
pub struct AgentSubagents {
    store: Store,
    parent: String,
    model: String,
    now: Now,
}

impl AgentSubagents {
    pub fn new(store: &Store, parent: &str, model: &str, now: Now) -> AgentSubagents {
        AgentSubagents { store: store.clone(), parent: parent.into(), model: model.into(), now }
    }

    async fn open(&self, id: &str) -> Result<Arc<dyn ChildRecord>, String> {
        Ok(Arc::new(AgentHistory::open(&self.store, id, "subagent", &self.model, self.now.clone()).await?))
    }
}

impl SubagentStore for AgentSubagents {
    fn saved<'a>(&'a self) -> Fut<'a, Vec<SavedSubagent>> {
        Box::pin(async move {
            let rows = sqlx::query("select child_agent_id, prompt, status, final_response, completed_at, metadata_json from agent.subagent_spawns where parent_agent_id = $1 order by spawn_id")
                .bind(&self.parent)
                .fetch_all(&self.store.pool)
                .await
                .map_err(db)?;
            Ok(rows
                .into_iter()
                .map(|r| {
                    let meta: Option<String> = r.get("metadata_json");
                    SavedSubagent {
                        id: r.get("child_agent_id"),
                        task: r.get::<Option<String>, _>("prompt").unwrap_or_default(),
                        status: status_of(r.get::<Option<String>, _>("status").as_deref()),
                        result: r.get("final_response"),
                        completed: r.get::<Option<i64>, _>("completed_at").is_some(),
                        finish_hook: meta.and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok()).is_some_and(|v| v["finish_hook"] == true),
                    }
                })
                .collect())
        })
    }

    fn spawned<'a>(&'a self, id: &'a str, task: &'a str, prefix: &'a Prefix, finish_hook: bool) -> Fut<'a, Arc<dyn ChildRecord>> {
        Box::pin(async move {
            let now = (self.now)() / 1000;
            sqlx::query(
                "insert into agent.agents (agent_id, id, session_id, kind, parent_id, parent_agent_id, root_session_id, depth, created_at, updated_at, model, status, last_assistant_message) \
                 values ($1, $1, null, 'subagent', $2, $2, null, 1, $3, $3, $4, 'running', null) on conflict (agent_id) do nothing",
            )
            .bind(id)
            .bind(&self.parent)
            .bind(now)
            .bind(&self.model)
            .execute(&self.store.pool)
            .await
            .map_err(db)?;
            let meta = finish_hook.then(|| serde_json::json!({ "finish_hook": true }).to_string());
            sqlx::query(
                "insert into agent.subagent_spawns (parent_agent_id, child_agent_id, child_depth, created_at, status, prompt, metadata_json) \
                 select $1, $2, coalesce((select depth from agent.agents where id = $2), 0), $3, 'running', $4, $5",
            )
            .bind(&self.parent)
            .bind(id)
            .bind(now)
            .bind(task)
            .bind(meta)
            .execute(&self.store.pool)
            .await
            .map_err(db)?;
            let h = AgentHistory::open(&self.store, id, "subagent", &self.model, self.now.clone()).await?;
            h.pin_prefix(prefix).await?;
            Ok(Arc::new(h) as Arc<dyn ChildRecord>)
        })
    }

    fn child<'a>(&'a self, id: &'a str) -> Fut<'a, Arc<dyn ChildRecord>> {
        Box::pin(self.open(id))
    }

    fn status<'a>(&'a self, id: &'a str, status: Status, result: Option<&'a str>) -> Fut<'a, ()> {
        Box::pin(async move {
            let now = (self.now)() / 1000;
            let pool = &self.store.pool;
            if status == Status::Running {
                sqlx::query("update agent.subagent_spawns set status = 'running', deferred_terminal_status = null, final_response = null, completed_at = null, seen_at = null where child_agent_id = $1")
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(db)?;
                sqlx::query("update agent.agents set updated_at = $1, status = 'running', last_assistant_message = null where id = $2 and status not in ('shutdown', 'not_found')")
                    .bind(now)
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(db)?;
                return Ok(());
            }
            let st = status_str(status);
            sqlx::query(
                "update agent.subagent_spawns set completed_at = coalesce(completed_at, $1), status = $2, deferred_terminal_status = null, final_response = coalesce(final_response, $3) where child_agent_id = $4",
            )
            .bind(now)
            .bind(st)
            .bind(result)
            .bind(id)
            .execute(pool)
            .await
            .map_err(db)?;
            sqlx::query("update agent.agents set updated_at = $1, status = $2, last_assistant_message = coalesce($3, last_assistant_message) where id = $4")
                .bind(now)
                .bind(st)
                .bind(result)
                .bind(id)
                .execute(pool)
                .await
                .map_err(db)?;
            Ok(())
        })
    }
}

/// Владельцы восстановления в `agent.recovery_owners` — запросы ядра
/// (около `0x1717efb`): заведение `insert … values ($1, $2, 1, $3, $4,
/// now(), null, null) on conflict (recovery_class, owner_id) do nothing`,
/// закрытие `update … set terminal_at = now(), terminal_status = $5 where …
/// and terminal_at is null`.
///
/// Своё (в разборе не видно): строка работника в `agent.agents` — `kind =
/// 'subagent'`, `agent_type` — класс владельца, без родителя, сведения
/// службы — `spawn_metadata_json`;
/// `message_id` владельца — id работника.
pub struct AgentWorkers {
    store: Store,
    model: String,
    now: Now,
}

impl AgentWorkers {
    pub fn new(store: &Store, model: &str, now: Now) -> AgentWorkers {
        AgentWorkers { store: store.clone(), model: model.into(), now }
    }
}

impl WorkerStore for AgentWorkers {
    fn begin<'a>(&'a self, class: &'a str, owner_id: &'a str, agent_id: &'a str, prefix: &'a Prefix, metadata: Option<&'a str>) -> Fut<'a, Arc<dyn ChildRecord>> {
        Box::pin(async move {
            let now = (self.now)() / 1000;
            sqlx::query(
                "insert into agent.agents (agent_id, id, session_id, kind, parent_id, parent_agent_id, root_session_id, depth, created_at, updated_at, model, status, last_assistant_message, agent_type, spawn_metadata_json) \
                 values ($1, $1, null, 'subagent', null, null, null, 0, $2, $2, $3, 'running', null, $4, $5) on conflict (agent_id) do nothing",
            )
            .bind(agent_id)
            .bind(now)
            .bind(&self.model)
            .bind(class)
            .bind(metadata)
            .execute(&self.store.pool)
            .await
            .map_err(db)?;
            sqlx::query(
                "insert into agent.recovery_owners (recovery_class, owner_id, generation, agent_id, message_id, active_at, terminal_at, terminal_status) \
                 values ($1, $2, 1, $3, $3, now(), null, null) on conflict (recovery_class, owner_id) do nothing",
            )
            .bind(class)
            .bind(owner_id)
            .bind(agent_id)
            .execute(&self.store.pool)
            .await
            .map_err(db)?;
            let h = AgentHistory::open(&self.store, agent_id, "subagent", &self.model, self.now.clone()).await?;
            h.pin_prefix(prefix).await?;
            Ok(Arc::new(h) as Arc<dyn ChildRecord>)
        })
    }

    fn active<'a>(&'a self, class: &'a str, owner_id: &'a str) -> Fut<'a, Option<Arc<dyn ChildRecord>>> {
        Box::pin(async move {
            let agent: Option<String> = sqlx::query_scalar("select agent_id from agent.recovery_owners where recovery_class = $1 and owner_id = $2 and terminal_at is null")
                .bind(class)
                .bind(owner_id)
                .fetch_optional(&self.store.pool)
                .await
                .map_err(db)?;
            let Some(agent) = agent else { return Ok(None) };
            let h = AgentHistory::open(&self.store, &agent, "subagent", &self.model, self.now.clone()).await?;
            Ok(Some(Arc::new(h) as Arc<dyn ChildRecord>))
        })
    }

    fn list_active<'a>(&'a self, class: &'a str) -> Fut<'a, Vec<agent_loop::ActiveOwner>> {
        Box::pin(async move {
            let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
                "select o.owner_id, o.agent_id, a.spawn_metadata_json from agent.recovery_owners o join agent.agents a on a.agent_id = o.agent_id \
                 where o.recovery_class = $1 and o.terminal_at is null order by o.active_at, o.owner_id",
            )
            .bind(class)
            .fetch_all(&self.store.pool)
            .await
            .map_err(db)?;
            let mut out = vec![];
            for (owner, agent, meta) in rows {
                let h = AgentHistory::open(&self.store, &agent, "subagent", &self.model, self.now.clone()).await?;
                out.push((owner, Arc::new(h) as Arc<dyn ChildRecord>, meta));
            }
            Ok(out)
        })
    }

    fn terminal<'a>(&'a self, class: &'a str, owner_id: &'a str, status: &'a str) -> Fut<'a, ()> {
        Box::pin(async move {
            let agent: Option<String> = sqlx::query_scalar(
                "update agent.recovery_owners set terminal_at = now(), terminal_status = $3 where recovery_class = $1 and owner_id = $2 and terminal_at is null returning agent_id",
            )
            .bind(class)
            .bind(owner_id)
            .bind(status)
            .fetch_optional(&self.store.pool)
            .await
            .map_err(db)?;
            if let Some(agent) = agent {
                sqlx::query("update agent.agents set updated_at = $1, status = 'completed' where id = $2").bind((self.now)() / 1000).bind(&agent).execute(&self.store.pool).await.map_err(db)?;
            }
            Ok(())
        })
    }
}
