//! Каталог `hamlet.db` — форма каталога muse (`struct SchemaCatalog`,
//! `CatalogTable`, `QueryCapabilities`; вшитый JSON около `0x18a9a6a`,
//! `format_version` 3): таблицы со столбцами и ограничениями, разрешённые
//! функции и приведения. Как у muse, каталог вшит в движок
//! (`agent/db/catalog.json`), а тест сверяет его с базой после миграций.
//!
//! `redaction` — как у muse: таблица читается через вид, который скрывает
//! строки или столбцы (у `agent.context_items` — строки мышления, вид
//! `inspection.context_items`, текст пометки — ядра).
//!
//! Своё: в каталоге — наши таблицы, которые есть и в каталоге muse
//! (`READABLE`); нет `migrations_sha256` (сверку держит тест).

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

pub const FORMAT_VERSION: u32 = 3;
pub const SCOPE: &str = "Hamlet application base tables readable by hamlet.db";

/// Наши таблицы из каталога muse (`runtime.event_hook_admissions` в нём нет).
pub const READABLE: [&str; 26] = [
    "agent.agent_compactions",
    "agent.agent_message_token_usage",
    "agent.agents",
    "agent.context_items",
    "agent.message_mailbox",
    "agent.volatile_context_pins",
    "goals.briefings",
    "goals.goals",
    "goals.updates",
    "memory.claims",
    "memory.embedding_models",
    "memory.embeddings",
    "memory.entries",
    "memory.entry_attributes",
    "memory.metadata",
    "runtime.agent_todo_snapshots",
    "runtime.search_documents",
    "runtime.workflow_agent_calls",
    "runtime.workflow_phase_runs",
    "runtime.workflow_runs",
    "scheduler.cron_mutations",
    "scheduler.delivery_outbox",
    "scheduler.job_definitions",
    "scheduler.job_runs",
    "scheduler.jobs",
    "scheduler.terminal_signals",
];

/// `query_capabilities.functions` каталога muse.
pub const FUNCTIONS: [&str; 85] = [
    "abs", "age", "array_length", "array_position", "array_to_string", "avg", "bit_and", "bit_or", "bool_and", "bool_or", "btrim", "cardinality", "ceil", "ceiling",
    "char_length", "coalesce", "concat", "concat_ws", "count", "date_bin", "date_part", "date_trunc", "dense_rank", "encode", "every", "extract", "first_value", "floor",
    "greatest", "json_array_length", "json_build_array", "json_build_object", "json_extract_path", "json_extract_path_text", "json_object_keys", "json_typeof",
    "jsonb_array_length", "jsonb_build_array", "jsonb_build_object", "jsonb_extract_path", "jsonb_extract_path_text", "jsonb_object_keys", "jsonb_typeof", "lag",
    "last_value", "lead", "least", "left", "length", "lower", "ltrim", "make_interval", "max", "md5", "min", "mod", "now", "nth_value", "ntile", "nullif",
    "octet_length", "position", "power", "rank", "regexp_match", "regexp_replace", "replace", "reverse", "right", "round", "row_number", "rtrim", "split_part", "sqrt",
    "strpos", "substr", "substring", "sum", "time_bucket", "timezone", "to_char", "to_json", "to_jsonb", "trim", "upper",
];

/// `query_capabilities.cast_types` каталога muse.
pub const CAST_TYPES: [&str; 32] = [
    "bigint", "bool", "boolean", "bytea", "char", "character", "character varying", "date", "decimal", "double precision", "float4", "float8", "int", "int2", "int4",
    "int8", "integer", "interval", "json", "jsonb", "numeric", "real", "smallint", "text", "time", "timestamp", "timestamp with time zone", "timestamp without time zone",
    "timestamptz", "timetz", "uuid", "varchar",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub format_version: u32,
    pub query_capabilities: QueryCapabilities,
    pub scope: String,
    pub tables: Vec<Table>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryCapabilities {
    pub cast_types: Vec<String>,
    pub functions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<Column>,
    pub comment: Option<String>,
    pub constraints: Vec<Constraint>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction: Option<Redaction>,
    pub schema: String,
}

/// Скрытое у таблицы: запрос к ней идёт через `view`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Redaction {
    pub note: String,
    pub view: String,
    pub withheld_columns: Vec<String>,
}

/// `redaction` каталога muse у наших таблиц; «muse.db» → «hamlet.db».
fn redaction(full: &str) -> Option<Redaction> {
    match full {
        "agent.context_items" => Some(Redaction {
            note: "Rows holding model reasoning (thinking and redacted-thinking items) are withheld. Stored commentary can be queried through `hamlet.db` for diagnosis. Commentary is hidden from the user's chat. A commentary record does not establish that the user received its contents.".into(),
            view: "inspection.context_items".into(),
            withheld_columns: vec![],
        }),
        "agent.agent_compactions" => Some(Redaction {
            note: "The compaction detail payload is withheld because it carries the model reasoning retained across the checkpoint.".into(),
            view: "inspection.agent_compactions".into(),
            withheld_columns: vec!["details_json".into()],
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub comment: Option<String>,
    pub default: Option<String>,
    pub name: String,
    pub nullable: bool,
    pub position: i32,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub column: String,
    pub name: String,
    pub position: i32,
    pub references: Option<Reference>,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub column: String,
    pub schema: String,
    pub table: String,
}

/// Вшитый каталог.
pub const EMBEDDED: &str = include_str!("../catalog.json");

impl Catalog {
    pub fn embedded() -> Catalog {
        serde_json::from_str(EMBEDDED).expect("hamlet.db schema catalog must be valid")
    }

    pub fn has_table(&self, schema: &str, table: &str) -> bool {
        self.tables.iter().any(|t| t.schema == schema && t.name == table)
    }

    /// Вид, через который читается таблица, если у неё есть `redaction`.
    pub fn view_for(&self, schema: &str, table: &str) -> Option<&str> {
        self.tables.iter().find(|t| t.schema == schema && t.name == table).and_then(|t| t.redaction.as_ref()).map(|r| r.view.as_str())
    }

    /// Каталог по живой базе: таблицы `READABLE`, столбцы и ограничения
    /// из `pg_catalog`.
    pub async fn from_database(pool: &PgPool) -> Result<Catalog, sqlx::Error> {
        let mut tables = Vec::new();
        for full in READABLE {
            let (schema, name) = full.split_once('.').unwrap();
            let comment: Option<String> = sqlx::query_scalar(
                "select obj_description(c.oid, 'pg_class') from pg_class c join pg_namespace n on n.oid = c.relnamespace where n.nspname = $1 and c.relname = $2",
            )
            .bind(schema)
            .bind(name)
            .fetch_one(pool)
            .await?;
            let columns = sqlx::query(
                "select a.attname::text as name, format_type(a.atttypid, a.atttypmod) as ty, not a.attnotnull as nullable, a.attnum::int4 as position, \
                 pg_get_expr(d.adbin, d.adrelid) as dflt, col_description(c.oid, a.attnum) as comment \
                 from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace \
                 left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum \
                 where n.nspname = $1 and c.relname = $2 and a.attnum > 0 and not a.attisdropped order by a.attnum",
            )
            .bind(schema)
            .bind(name)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| Column { comment: r.get("comment"), default: r.get("dflt"), name: r.get("name"), nullable: r.get("nullable"), position: r.get("position"), ty: r.get("ty") })
            .collect();
            let constraints = sqlx::query(
                "select con.conname::text as name, case con.contype when 'p' then 'PRIMARY KEY' when 'u' then 'UNIQUE' else 'FOREIGN KEY' end as ty, \
                 k.pos::int4 as position, a.attname::text as col, rn.nspname::text as ref_schema, rc.relname::text as ref_table, ra.attname::text as ref_col \
                 from pg_constraint con join pg_class c on c.oid = con.conrelid join pg_namespace n on n.oid = c.relnamespace \
                 cross join lateral unnest(con.conkey) with ordinality as k(attnum, pos) \
                 join pg_attribute a on a.attrelid = c.oid and a.attnum = k.attnum \
                 left join pg_class rc on rc.oid = con.confrelid left join pg_namespace rn on rn.oid = rc.relnamespace \
                 left join pg_attribute ra on ra.attrelid = con.confrelid and ra.attnum = con.confkey[k.pos] \
                 where n.nspname = $1 and c.relname = $2 and con.contype in ('p', 'u', 'f') order by con.conname, k.pos",
            )
            .bind(schema)
            .bind(name)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| {
                let ref_table: Option<String> = r.get("ref_table");
                Constraint {
                    column: r.get("col"),
                    name: r.get("name"),
                    position: r.get("position"),
                    references: ref_table.map(|table| Reference { column: r.get("ref_col"), schema: r.get("ref_schema"), table }),
                    ty: r.get("ty"),
                }
            })
            .collect();
            tables.push(Table { columns, comment, constraints, name: name.into(), redaction: redaction(full), schema: schema.into() });
        }
        Ok(Catalog {
            format_version: FORMAT_VERSION,
            query_capabilities: QueryCapabilities { cast_types: CAST_TYPES.iter().map(|s| s.to_string()).collect(), functions: FUNCTIONS.iter().map(|s| s.to_string()).collect() },
            scope: SCOPE.into(),
            tables,
        })
    }

    /// Справочник для навыка (`references/schema.md`). Своё: у muse текст
    /// навыка `muse_db` в снимке не сохранился — справочник строится из
    /// каталога.
    pub fn schema_md(&self) -> String {
        let mut s = String::from("# Hamlet database schema\n\nTables readable by `hamlet.db`. Always use the schema-qualified name.\n");
        for t in &self.tables {
            s.push_str(&format!("\n## `{}.{}`\n\n", t.schema, t.name));
            if let Some(c) = &t.comment {
                s.push_str(&format!("{c}\n\n"));
            }
            if let Some(r) = &t.redaction {
                s.push_str(&format!("{}\n\n", r.note));
            }
            s.push_str("| column | type | nullable | default |\n|---|---|---|---|\n");
            for c in &t.columns {
                let d = c.default.as_deref().map(|d| format!("`{}`", d.replace('|', "\\|"))).unwrap_or_default();
                s.push_str(&format!("| `{}` | {} | {} | {} |\n", c.name, c.ty, if c.nullable { "yes" } else { "no" }, d));
            }
            if !t.constraints.is_empty() {
                s.push('\n');
                for k in &t.constraints {
                    match &k.references {
                        Some(r) => s.push_str(&format!("- {} `{}` → `{}.{}.{}`\n", k.ty, k.column, r.schema, r.table, r.column)),
                        None => s.push_str(&format!("- {} `{}` (`{}`)\n", k.ty, k.column, k.name)),
                    }
                }
            }
        }
        s
    }

    /// Справочник разрешённых функций и приведений (`references/querying.md`).
    pub fn querying_md(&self) -> String {
        let q = &self.query_capabilities;
        format!(
            "# Querying with hamlet.db\n\n\
             - One PostgreSQL `SELECT` per call; tables are schema-qualified (`memory.entries`), as listed in `schema.md`.\n\
             - CTE names start with `hatch_cte_`; recursive CTEs, `SELECT INTO`, locking clauses, table functions, `LATERAL` and sampling are rejected.\n\
             - Give every selected column a unique name; duplicate output names are rejected.\n\
             - At most 200 rows come back; a row larger than 65536 bytes of JSON is replaced by `{{\"_hatch_db_row_omitted\": true, \"serialized_bytes\": N}}` — select fewer or shorter columns.\n\
             - The query runs read-only with a 3000 ms statement timeout and a 5000 ms total limit.\n\n\
             ## Functions\n\nOnly these built-in functions are accepted:\n\n{}\n\n## Casts\n\nOnly these cast types are accepted (with `::type` or `CAST(… AS type)`):\n\n{}\n",
            q.functions.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", "),
            q.cast_types.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", "),
        )
    }
}
