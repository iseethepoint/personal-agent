//! Тесты П4л-6: `hamlet.db` — проверка запроса, исполнение на чтение,
//! вшитый каталог против базы после миграций, справочники навыка
//! `hamlet_db`, и ход через цикл на обоих поставщиках.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
#[path = "../../memory/tests/pg/mod.rs"]
mod pg;

use std::path::PathBuf;
use std::sync::Arc;

use agent_db::validate::{self, validate};
use agent_db::{Catalog, Db};
use agent_loop::{Tools, Turn};
use agent_model::{Message, Part, Role, ToolResultPart};
use common::{model, temp_dir, Step, Wire};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// База со всеми нашими миграциями.
async fn database() -> (String, PgPool) {
    let url = pg::database();
    agent_memory::Store::connect(&url).await.unwrap();
    agent_cron::Store::connect(&url).await.unwrap();
    agent_goals::Store::connect(&url, Arc::new(|| 0)).await.unwrap();
    agent_workflows::store::Store::connect(&url).await.unwrap();
    agent_history::Store::connect(&url).await.unwrap();
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await.unwrap();
    (url, pool)
}

/// Каталог, вшитый в движок, и справочники навыка совпадают с базой.
/// `HAMLET_DB_WRITE_CATALOG=1` переписывает их по базе.
#[tokio::test]
async fn catalog_matches_database() {
    let (_url, pool) = database().await;
    let live = Catalog::from_database(&pool).await.unwrap();
    let files = [
        (root().join("agent/db/catalog.json"), serde_json::to_string_pretty(&live).unwrap() + "\n"),
        (root().join("skills/hamlet_db/references/schema.md"), live.schema_md()),
        (root().join("skills/hamlet_db/references/querying.md"), live.querying_md()),
    ];
    if std::env::var_os("HAMLET_DB_WRITE_CATALOG").is_some() {
        for (p, s) in &files {
            std::fs::write(p, s).unwrap();
        }
    }
    assert_eq!(Catalog::embedded(), live, "agent/db/catalog.json отстал от миграций");
    for (p, s) in &files {
        assert_eq!(&std::fs::read_to_string(p).unwrap(), s, "{}", p.display());
    }
    assert_eq!(live.tables.len(), 26);
    let entries = live.tables.iter().find(|t| t.schema == "memory" && t.name == "entries").unwrap();
    assert_eq!(entries.columns[0].default.as_deref(), Some("nextval('memory.entries_memory_entry_id_seq'::regclass)"));
    assert!(live.schema_md().contains("\n## `scheduler.jobs`\n"));
    assert!(!live.has_table("runtime", "event_hook_admissions"));
    assert_eq!(live.view_for("agent", "context_items"), Some("inspection.context_items"));
    assert!(live.schema_md().contains("Rows holding model reasoning (thinking and redacted-thinking items) are withheld."));
}

#[test]
fn validator_rejects_like_the_core() {
    let c = Catalog::embedded();
    let err = |sql: &str| validate(sql, &c).unwrap_err();
    assert_eq!(err("  \n"), validate::EMPTY);
    assert_eq!(err(&format!("select 1 as x -- {}", "a".repeat(16384))), "SQL statement exceeds the 16384-byte limit");
    assert!(err("select 1 +").starts_with("invalid PostgreSQL SELECT: "));
    assert_eq!(err("delete from memory.metadata"), validate::DATA_CHANGING);
    assert_eq!(err("with hatch_cte_x as (delete from memory.metadata returning key) select key from hatch_cte_x"), validate::DATA_CHANGING);
    assert_eq!(err("select 1 as a; select 2 as b"), validate::ONE_SELECT);
    assert_eq!(err("show search_path"), validate::ONE_SELECT);
    assert_eq!(err("select key from memory.metadata for update"), validate::CLAUSES);
    assert_eq!(err("with recursive hatch_cte_r as (select 1 as n) select n from hatch_cte_r"), validate::RECURSIVE);
    assert_eq!(err("with r as (select 1 as n) select n from r"), validate::CTE_PREFIX);
    assert_eq!(err("select key into t from memory.metadata"), validate::SELECT_INTO);
    assert_eq!(err("select x from memory.metadata m, lateral (select m.key as x) s"), validate::TABLE_MODIFIERS);
    assert_eq!(err("select key from memory.metadata tablesample system (10)"), validate::TABLE_MODIFIERS);
    assert_eq!(err("select k from jsonb_object_keys('{}') as k"), validate::TABLE_MODIFIERS);
    assert_eq!(err("select usename from pg_catalog.pg_user"), "relation `pg_catalog.pg_user` is outside the `hamlet.db` table catalog");
    assert_eq!(err("select key from metadata"), "relation `metadata` must be a declared CTE or a schema-qualified table from the `hamlet.db` schema guide");
    assert_eq!(err("select key from runtime.event_hook_admissions"), "relation `runtime.event_hook_admissions` is outside the `hamlet.db` table catalog");
    assert_eq!(err("select pg_sleep(4) as s"), "function `pg_sleep` is outside the `hamlet.db` safe function set");
    assert_eq!(err("select pg_catalog.lower(key) as k from memory.metadata"), "function `pg_catalog.lower` is outside the `hamlet.db` safe function set");
    assert_eq!(err("select key::regclass as k from memory.metadata"), "cast type `regclass` is outside the `hamlet.db` safe type set");
    assert_eq!(err("select key collate \"C\" as k from memory.metadata"), validate::COLLATION);
    // Таблица с redaction читается через свой вид.
    let v = validate("select seq, item_json from agent.context_items where agent_id = 'root'", &c).unwrap();
    assert!(v.contains("FROM inspection.context_items") && !v.contains("agent.context_items"), "{v}");
    // Разрешённое: CTE с приставкой, функции и приведения из списка.
    let ok = validate(
        "with hatch_cte_m as (select key, value from memory.metadata) select lower(key) as k, count(*)::int8 as n, cast(now() as timestamp with time zone) as t, value::varchar(10) as v from hatch_cte_m group by key, value",
        &c,
    )
    .unwrap();
    assert!(ok.starts_with("WITH hatch_cte_m AS"), "{ok}");
}

async fn insert(pool: &PgPool, key: &str, value: &str) {
    sqlx::query("insert into memory.metadata (key, value) values ($1, $2)").bind(key).bind(value).execute(pool).await.unwrap();
}

#[tokio::test]
async fn query_runs_read_only_with_core_limits() {
    let (_url, pool) = database().await;
    let db = Db::new(pool.clone(), &root().join("skills"));
    assert_eq!(db.query("select key as k from memory.metadata").await.unwrap(), "0 rows");
    insert(&pool, "big", &"x".repeat(70_000)).await;
    insert(&pool, "small", "v").await;
    let out = db.query("select key, value from memory.metadata order by key").await.unwrap();
    let lines: Vec<&str> = out.lines().collect();
    let omitted: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(omitted["_hatch_db_row_omitted"], true);
    assert!(omitted["serialized_bytes"].as_i64().unwrap() > 65536);
    assert_eq!(serde_json::from_str::<serde_json::Value>(lines[1]).unwrap(), json!({"key": "small", "value": "v"}));
    // Повтор имён столбцов — отказ.
    assert_eq!(
        db.query("select a.key, b.key from memory.metadata a join memory.metadata b on a.key = b.key").await.unwrap_err(),
        "hamlet.db query has duplicate output columns: key; alias every selected column to a unique name"
    );
    // Больше 200 строк: 200 и строка «есть ещё».
    for i in 0..250 {
        insert(&pool, &format!("k{i:03}"), "v").await;
    }
    let out = db.query("select key from memory.metadata where key like 'k%' order by key").await.unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 201);
    assert_eq!(lines[199], r#"{"key": "k199"}"#);
    assert_eq!(lines[200], "200 rows shown; more rows matched. Narrow the query to see the rest.");
    // Ошибка базы — текстом ядра.
    assert!(db.query("select 1 / (count(*) - count(*)) as x from memory.metadata").await.unwrap_err().starts_with("hamlet.db failed: "));
}

/// Транзакция только на чтение: запись через функцию в SELECT (проверку
/// она прошла бы, будь функция в каталоге) падает в базе, значение не
/// сдвигается. Без `set transaction read only` — красный.
#[tokio::test]
async fn transaction_is_read_only() {
    let (_url, pool) = database().await;
    let mut c = Catalog::embedded();
    c.query_capabilities.functions.push("nextval".into());
    let db = Db::new(pool.clone(), &root().join("skills")).with_catalog(c);
    let e = db.query("select nextval('memory.entries_memory_entry_id_seq') as n").await.unwrap_err();
    assert!(e.starts_with("hamlet.db failed: ") && e.contains("cannot execute nextval() in a read-only transaction"), "{e}");
    let called: bool = sqlx::query_scalar("select is_called from memory.entries_memory_entry_id_seq").fetch_one(&pool).await.unwrap();
    assert!(!called, "последовательность сдвинулась");
}

async fn one_turn(wire: Wire) {
    let (_url, pool) = database().await;
    insert(&pool, "owner_city", "Tbilisi").await;
    let mut tools = Tools::new();
    tools.add(Db::new(pool, &root().join("skills")));
    let spec = tools.get("hamlet.db").unwrap().spec();
    assert!(spec.description.contains("skills/hamlet_db/references/schema.md`."), "{}", spec.description);
    assert!(spec.description.starts_with("Run bounded, read-only SQL SELECT queries against database-backed Hamlet records."));
    let (_server, p) = model(
        wire,
        vec![
            Step::Calls(vec![("d1", "hamlet.db", json!({"sql": "update memory.metadata set value = 'x'"}))]),
            Step::Calls(vec![("d2", "hamlet.db", json!({"sql": "select key, value from memory.metadata"}))]),
            Step::Text("Город — Tbilisi."),
        ],
        false,
    );
    let dir = temp_dir(&format!("db-turn-{wire:?}"));
    let turn = Turn::new(&p, &tools, "s", dir.join("spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "какой город записан?".into() }] }];
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Город — Tbilisi.");
    let text = |m: &Message| match &m.content[0] {
        Part::ToolResult { content, is_error, .. } => (match &content[0] {
            ToolResultPart::Text { text } => text.clone(),
            _ => String::new(),
        }, *is_error),
        p => panic!("{p:?}"),
    };
    assert_eq!(text(&h[2]), (validate::DATA_CHANGING.to_string(), true));
    assert_eq!(text(&h[4]), (r#"{"key": "owner_city", "value": "Tbilisi"}"#.to_string(), false));
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}
