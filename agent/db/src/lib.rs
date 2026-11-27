//! `hamlet.db` — запросы только на чтение к своей базе, как `muse.db`
//! (`hatch-agent/src/tools/db.rs`, `struct MuseDbQueryArgs { sql }`).
//!
//! Исполнение — как в ядре (около `0x1f444281`…`0x1f445717`):
//! - проверка и нормализация запроса (`validate.rs`);
//! - транзакция `set transaction read only`, `set local search_path =
//!   pg_catalog`, `set local statement_timeout = '3000ms'` (`0x4a3318`),
//!   `set local lock_timeout = '250ms'` (`0x4a3970`);
//! - имена столбцов результата — без повторов;
//! - каждая строка — `to_jsonb(...)::text`, строка длиннее 65536 байт
//!   (`0x4a1f28`) заменяется пометкой `_hatch_db_row_omitted`, не больше 201
//!   строки (`0x498088`);
//! - весь вызов — не дольше 5000 мс (`0x49f2e0`).
//!
//! Своё (в разборе не видно):
//! - ответ модели — строки JSON по одной на строку; из 201 строки
//!   показываются 200 и строка о том, что есть ещё; пустой ответ — `0 rows`;
//! - путь к справочнику в описании — настоящий путь навыка `hamlet_db` в
//!   папке системных навыков агента.

pub mod catalog;
pub mod validate;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use agent_loop::{Tool, ToolError, ToolFuture};
use agent_model::{ToolResultPart, ToolSpec};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Executor, PgPool};

pub use catalog::Catalog;

pub const NAME: &str = "hamlet.db";
/// Предел на строку результата в байтах JSON.
pub const ROW_BYTES: usize = 65536;
/// Сколько строк забирается (200 показываются, 201-я — признак «есть ещё»).
pub const ROW_LIMIT: usize = 201;
pub const TOTAL_LIMIT: Duration = Duration::from_millis(5000);
const SETUP: [&str; 4] = [
    "set transaction read only",
    "set local search_path = pg_catalog",
    "set local statement_timeout = '3000ms'",
    "set local lock_timeout = '250ms'",
];

const TOOLS_JSON: &str = include_str!("../../../prompts/db/tools.json");

fn texts() -> Value {
    serde_json::from_str(TOOLS_JSON).expect("prompts/db/tools.json")
}

pub fn total_limit_exceeded() -> String {
    format!("hamlet.db exceeded its {}ms total time limit", TOTAL_LIMIT.as_millis())
}

pub fn duplicate_columns(names: &[String]) -> String {
    format!("hamlet.db query has duplicate output columns: {}; alias every selected column to a unique name", names.join(", "))
}

/// Инструмент `hamlet.db`.
#[derive(Clone)]
pub struct Db {
    pool: PgPool,
    catalog: Arc<Catalog>,
    description: String,
}

impl Db {
    /// `skills` — папка системных навыков: справочник лежит в
    /// `hamlet_db/references/schema.md`.
    pub async fn connect(url: &str, skills: &Path) -> Result<Db, String> {
        let pool = PgPoolOptions::new().max_connections(4).connect(url).await.map_err(|e| e.to_string())?;
        Ok(Db::new(pool, skills))
    }

    pub fn new(pool: PgPool, skills: &Path) -> Db {
        let schema = skills.join("hamlet_db/references/schema.md");
        let description = texts()["tool"]["description"].as_str().unwrap().replace("{schema_path}", &schema.display().to_string());
        Db { pool, catalog: Arc::new(Catalog::embedded()), description }
    }

    /// Другой каталог вместо вшитого (тест исполнения на чтение подаёт
    /// каталог с функцией, которой во вшитом нет).
    pub fn with_catalog(mut self, catalog: Catalog) -> Db {
        self.catalog = Arc::new(catalog);
        self
    }

    /// Проверить и исполнить запрос; ответ — текст для модели.
    pub async fn query(&self, sql: &str) -> Result<String, String> {
        let normalized = validate::validate(sql, &self.catalog)?;
        let rows = tokio::time::timeout(TOTAL_LIMIT, self.run(&normalized)).await.map_err(|_| total_limit_exceeded())??;
        Ok(render(rows))
    }

    async fn run(&self, normalized: &str) -> Result<Vec<String>, String> {
        let failed = |e: sqlx::Error| format!("hamlet.db failed: {e}");
        let mut tx = self.pool.begin().await.map_err(failed)?;
        for s in SETUP {
            sqlx::query(s).execute(&mut *tx).await.map_err(failed)?;
        }
        let stmt = (&mut *tx).prepare(normalized).await.map_err(failed)?;
        let mut seen = std::collections::HashSet::new();
        let mut dups: Vec<String> = Vec::new();
        for c in sqlx::Statement::columns(&stmt) {
            let n = sqlx::Column::name(c).to_string();
            if !seen.insert(n.clone()) && !dups.contains(&n) {
                dups.push(n);
            }
        }
        if !dups.is_empty() {
            return Err(duplicate_columns(&dups));
        }
        let wrapped = format!(
            "select case when octet_length(to_jsonb(hatch_db_row)::text) <= {ROW_BYTES} then to_jsonb(hatch_db_row)::text else jsonb_build_object('_hatch_db_row_omitted', true, 'serialized_bytes', octet_length(to_jsonb(hatch_db_row)::text))::text end as row_json from ({normalized}) as hatch_db_row limit {ROW_LIMIT}"
        );
        let rows: Vec<String> = sqlx::query_scalar(&wrapped).fetch_all(&mut *tx).await.map_err(failed)?;
        tx.rollback().await.map_err(failed)?;
        Ok(rows)
    }
}

fn render(mut rows: Vec<String>) -> String {
    let t = texts();
    if rows.is_empty() {
        return t["texts"]["no_rows"].as_str().unwrap().to_string();
    }
    let more = rows.len() >= ROW_LIMIT;
    rows.truncate(ROW_LIMIT - 1);
    let mut out = rows.join("\n");
    if more {
        out.push('\n');
        out.push_str(&t["texts"]["truncated"].as_str().unwrap().replace("{n}", &(ROW_LIMIT - 1).to_string()));
    }
    out
}

impl Tool for Db {
    fn spec(&self) -> ToolSpec {
        let t = texts();
        ToolSpec {
            name: NAME.into(),
            description: self.description.clone(),
            input_schema: json!({
                "type": "object",
                "properties": {"sql": {"type": "string", "description": t["tool"]["sql"]}},
                "required": ["sql"],
                "additionalProperties": false
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let Some(sql) = args.get("sql").and_then(Value::as_str) else {
                return Err(ToolError::InvalidArguments("missing field `sql`".into()));
            };
            let text = self.query(sql).await.map_err(ToolError::Failed)?;
            Ok(vec![ToolResultPart::Text { text }])
        })
    }
}
