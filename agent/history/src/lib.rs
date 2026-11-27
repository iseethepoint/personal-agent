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
//! Своё (в разборе не видно или у muse иначе устроено):
//! - строка — одно наше сообщение целиком (`item_json`), а не часть;
//!   `item_kind`: сообщение с вызовами — `tool_call`, с результатами —
//!   `tool_output`, иначе `message_text`; `call_id`/`tool_name` — когда вызов
//!   или результат один;
//! - заметка изменчивого помечается `message_source = 'volatile'`, по ней
//!   после перезапуска восстанавливается последняя заметка;
//! - начало подсказки (системный текст и описания инструментов)
//!   закрепляется пином `prompt_prefix` и после перезапуска берётся из него
//!   байт в байт.

use std::sync::Arc;

use agent_loop::{Prefix, Record};
use agent_model::{Message, Part, Role, ToolSpec};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

const MIGRATION: &str = include_str!("../migrations/0001_history.sql");
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
        sqlx::raw_sql(MIGRATION).execute(&pool).await.map_err(db)?;
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
    /// Сколько сообщений текущей подсказки уже записано.
    written: usize,
}

/// История одного агента.
pub struct AgentHistory {
    pool: PgPool,
    agent_id: String,
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
        let written: i64 = sqlx::query_scalar("select count(*) from agent.context_items where agent_id = $1 and seq >= $2")
            .bind(agent_id)
            .bind(floor)
            .fetch_one(&store.pool)
            .await
            .map_err(db)?;
        Ok(AgentHistory { pool: store.pool.clone(), agent_id: agent_id.into(), now, state: tokio::sync::Mutex::new(State { floor, written: written as usize }) })
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
        let rows = sqlx::query("select item_json, message_source from agent.context_items where agent_id = $1 and seq >= $2 order by seq")
            .bind(&self.agent_id)
            .bind(floor)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        let mut history = Vec::new();
        let mut last_volatile = None;
        for r in rows {
            let json: String = r.get("item_json");
            let m: Message = serde_json::from_str(&json).map_err(|e| format!("history: item_json: {e}"))?;
            if r.get::<Option<String>, _>("message_source").as_deref() == Some(VOLATILE) {
                last_volatile = Some(text_of(&m));
            }
            history.push(m);
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
            let seq = st.floor + st.written as i64;
            self.insert(seq, m, volatile).await?;
            st.written += 1;
        }
        Ok(())
    }

    /// История заменена (сжатие): новая пишется после старой, граница
    /// подсказки сдвигается на её начало.
    pub async fn replace(&self, history: &[Message], volatile: Option<&str>) -> Result<(), String> {
        {
            let mut st = self.state.lock().await;
            let max: i64 = sqlx::query_scalar("select coalesce(max(seq), -1) from agent.context_items where agent_id = $1")
                .bind(&self.agent_id)
                .fetch_one(&self.pool)
                .await
                .map_err(db)?;
            st.floor = max + 1;
            st.written = 0;
            sqlx::query("update agent.agents set prompt_floor_seq = $2, updated_at = $3 where agent_id = $1")
                .bind(&self.agent_id)
                .bind(st.floor)
                .bind((self.now)() / 1000)
                .execute(&self.pool)
                .await
                .map_err(db)?;
        }
        self.sync(history, volatile).await
    }

    async fn insert(&self, seq: i64, m: &Message, volatile: Option<&str>) -> Result<(), String> {
        let calls: Vec<(&str, &str)> = m
            .content
            .iter()
            .filter_map(|p| match p {
                Part::ToolCall { id, name, .. } => Some((id.as_str(), name.as_str())),
                _ => None,
            })
            .collect();
        let results: Vec<(&str, bool)> = m
            .content
            .iter()
            .filter_map(|p| match p {
                Part::ToolResult { call_id, is_error, .. } => Some((call_id.as_str(), !*is_error)),
                _ => None,
            })
            .collect();
        let (kind, call_id, tool_name, success) = if !calls.is_empty() {
            ("tool_call", (calls.len() == 1).then(|| calls[0].0), (calls.len() == 1).then(|| calls[0].1), None)
        } else if !results.is_empty() {
            ("tool_output", (results.len() == 1).then(|| results[0].0), None, (results.len() == 1).then(|| results[0].1))
        } else {
            ("message_text", None, None, None)
        };
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Developer => "developer",
        };
        let text = text_of(m);
        let source = (m.role == Role::Developer && volatile.is_some_and(|v| v == text)).then_some(VOLATILE);
        let json = serde_json::to_string(m).map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO agent.context_items (agent_id, seq, item_kind, text_content, role, call_id, tool_name, success, message_source, created_at, item_json) \
             VALUES ($1, $2, $3::agent.context_item_kind, $4, $5::runtime.message_role, $6, $7, $8, $9, to_timestamp($10::double precision), $11) \
             ON CONFLICT (agent_id, seq) DO UPDATE SET item_kind = excluded.item_kind, text_content = excluded.text_content, role = excluded.role, \
             call_id = excluded.call_id, tool_name = excluded.tool_name, success = excluded.success, \
             message_source = COALESCE(excluded.message_source, context_items.message_source), created_at = excluded.created_at, item_json = excluded.item_json",
        )
        .bind(&self.agent_id)
        .bind(seq)
        .bind(kind)
        .bind(&text)
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
        Ok(())
    }
}

impl Record for AgentHistory {
    fn record<'a>(&'a self, history: &'a [Message]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(self.sync(history, None))
    }
}
