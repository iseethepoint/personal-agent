//! Хранилище памяти в Postgres — форма muse `hatch-memory/src/store/repository.rs`
//! (`source-files.txt:5476`). Тексты запросов — из ядра muse
//! (`strings-all.txt:200351-200560`, участок 0x17f94dc–0x17fc944), с
//! подставленными на место пропусков параметрами. Своё — названо у функции.

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

/// Длина вектора — `vector(384)` у muse (`strings-all.txt:213514-213517`).
pub const DIMENSIONS: usize = 384;
/// Имя модели векторов по умолчанию у muse — `hatch-memory-default`, мера
/// `cosine` (`strings-all.txt:200516`). Своё нейтральное имя.
pub const DEFAULT_EMBEDDING_MODEL: &str = "memory-default";
pub const DISTANCE: &str = "cosine";

const MIGRATION: &str = include_str!("../migrations/0001_memory.sql");

pub type Result<T> = std::result::Result<T, String>;

fn db(e: sqlx::Error) -> String {
    format!("memory database: {e}")
}

/// Запись для `memory.entries`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEntry {
    pub memory_uri: String,
    pub chunk_id: String,
    pub source_type: String,
    pub status: String,
    pub privacy_class: String,
    pub body_text: String,
    pub confidence: f64,
    pub reason_text: Option<String>,
    pub citation_path: String,
    pub line_start: i64,
    pub line_end: i64,
    /// `None` — сейчас (`coalesce($12, now())`).
    pub created_at: Option<String>,
    pub created_at_unix: i64,
}

/// Запись памяти, как её отдаёт `entries.get` muse.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub memory_entry_id: i64,
    pub memory_uri: String,
    pub chunk_id: String,
    pub text: String,
    pub source_type: String,
    pub status: String,
    pub privacy_class: String,
    pub confidence: f64,
    pub citation_path: String,
    pub line_start: i64,
    pub line_end: i64,
    pub created_at: String,
    pub metadata: serde_json::Value,
}

/// Утверждение (`memory.claims`).
#[derive(Debug, Clone, PartialEq)]
pub struct NewClaim {
    pub claim_id: String,
    pub run_id: String,
    pub kind: String,
    pub salience: String,
    pub claim_text: String,
    pub quote: Option<String>,
    pub speaker: String,
    pub evidence_handles: Vec<String>,
    pub confidence: f64,
    /// Время, когда узнали; `None` — сейчас.
    pub first_seen: Option<String>,
    pub valid_until: Option<String>,
    pub source_path: String,
    pub source_line: i64,
    /// Старое утверждение, которое это заменяет.
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub claim_id: String,
    pub kind: String,
    pub salience: String,
    pub claim_text: String,
    pub quote: Option<String>,
    pub speaker: String,
    pub evidence_handles: serde_json::Value,
    pub supersedes_claim_id: Option<String>,
    pub status: String,
    pub confidence: f64,
    pub first_seen: String,
    pub reinforced_at: String,
    pub valid_until: Option<String>,
    pub source_path: String,
    pub source_line: i64,
}

const CLAIM_COLUMNS: &str = "claim_id, kind, salience, claim_text, quote, speaker, evidence_handles, supersedes_claim_id, status, confidence, first_seen::text as first_seen, reinforced_at::text as reinforced_at, valid_until::text as valid_until, source_path, source_line";

fn claim(r: &sqlx::postgres::PgRow) -> Claim {
    Claim {
        claim_id: r.get("claim_id"),
        kind: r.get("kind"),
        salience: r.get("salience"),
        claim_text: r.get("claim_text"),
        quote: r.get("quote"),
        speaker: r.get("speaker"),
        evidence_handles: r.get("evidence_handles"),
        supersedes_claim_id: r.get("supersedes_claim_id"),
        status: r.get("status"),
        confidence: r.get("confidence"),
        first_seen: r.get("first_seen"),
        reinforced_at: r.get("reinforced_at"),
        valid_until: r.get("valid_until"),
        source_path: r.get("source_path"),
        source_line: r.get("source_line"),
    }
}

/// Вектор в текстовой форме pgvector: `[a,b,…]`.
pub fn vector_text(v: &[f32]) -> String {
    let parts: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", parts.join(","))
}

/// Текст утверждения для сравнения: как `rtrim(lower(regexp_replace(claim_text,
/// '\s+', ' ', 'g')), '.!?')` у muse.
pub fn normalized(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    joined.trim_end_matches(['.', '!', '?']).to_string()
}

const ENTRY_SELECT: &str = "select e.memory_entry_id, e.memory_uri, e.chunk_id, e.body_text as text, e.source_type, e.status, e.privacy_class, e.confidence, coalesce(e.citation_path, '') as citation_path, e.line_start, e.line_end, e.created_at::text as created_at, coalesce((select jsonb_object_agg(a.attribute_name, a.scalar_value order by a.attribute_name) from memory.entry_attributes a where a.memory_entry_id = e.memory_entry_id), '{}'::jsonb) as metadata_json from memory.entries e";

fn entry(r: &sqlx::postgres::PgRow) -> Entry {
    Entry {
        memory_entry_id: r.get("memory_entry_id"),
        memory_uri: r.get("memory_uri"),
        chunk_id: r.get("chunk_id"),
        text: r.get("text"),
        source_type: r.get("source_type"),
        status: r.get("status"),
        privacy_class: r.get("privacy_class"),
        confidence: r.get("confidence"),
        citation_path: r.get("citation_path"),
        line_start: r.get("line_start"),
        line_end: r.get("line_end"),
        created_at: r.get("created_at"),
        metadata: r.get("metadata_json"),
    }
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    /// Подключиться и привести схему к миграции (она идемпотентна).
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(4).connect(url).await.map_err(db)?;
        sqlx::raw_sql(MIGRATION).execute(&pool).await.map_err(db)?;
        Ok(Store { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ---- записи ----

    /// Записать запись и её документ поиска по словам — запросы muse
    /// `insert into memory.entries … on conflict (memory_uri) do update` и
    /// `insert into runtime.search_documents … to_tsvector('english', $3)`.
    pub async fn upsert_entry(&self, e: &NewEntry) -> Result<i64> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let id: i64 = sqlx::query_scalar(
            "insert into memory.entries
         (memory_uri, chunk_id, source_type, status, privacy_class, body_text,
          confidence, reason_text, citation_path, line_start, line_end,
          created_at, created_at_unix)
         values ($1, $2, $3, $4, $5, $6, $7, $8, nullif($9, ''), $10, $11,
                 coalesce($12::timestamptz, now()), $13)
         on conflict (memory_uri) do update set
           chunk_id = excluded.chunk_id,
           source_type = excluded.source_type,
           status = excluded.status,
           privacy_class = excluded.privacy_class,
           body_text = excluded.body_text,
           confidence = excluded.confidence,
           reason_text = excluded.reason_text,
           citation_path = excluded.citation_path,
           line_start = excluded.line_start,
           line_end = excluded.line_end,
           created_at = least(memory.entries.created_at, excluded.created_at),
           created_at_unix = least(memory.entries.created_at_unix, excluded.created_at_unix)
         returning memory_entry_id",
        )
        .bind(&e.memory_uri)
        .bind(&e.chunk_id)
        .bind(&e.source_type)
        .bind(&e.status)
        .bind(&e.privacy_class)
        .bind(&e.body_text)
        .bind(e.confidence)
        .bind(&e.reason_text)
        .bind(&e.citation_path)
        .bind(e.line_start)
        .bind(e.line_end)
        .bind(&e.created_at)
        .bind(e.created_at_unix)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        let search_text = &e.body_text;
        sqlx::query(
            "insert into runtime.search_documents
         (owner_table, owner_key, search_text, search_vector)
         values ('memory.entries', $1, $2, to_tsvector('english', $3))
         on conflict (owner_table, owner_key) do update set
           search_text = excluded.search_text,
           search_vector = excluded.search_vector,
           updated_at = now()",
        )
        .bind(&e.memory_uri)
        .bind(search_text)
        .bind(search_text)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        // Содержимое могло смениться — старые векторы недействительны,
        // их дозаполнит следующая сверка.
        sqlx::query("delete from memory.embeddings where memory_entry_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(id)
    }

    /// Признак записи (`memory.entry_attributes`), например `claim_id`.
    pub async fn set_attribute(&self, entry_id: i64, name: &str, value: &str) -> Result<()> {
        sqlx::query(
            "insert into memory.entry_attributes (memory_entry_id, attribute_name, scalar_value)
             values ($1, $2, $3)
             on conflict (memory_entry_id, attribute_name) do update set scalar_value = excluded.scalar_value",
        )
        .bind(entry_id)
        .bind(name)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    pub async fn delete_attribute(&self, entry_id: i64, name: &str) -> Result<()> {
        sqlx::query("delete from memory.entry_attributes where memory_entry_id = $1 and attribute_name = $2")
            .bind(entry_id)
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    /// Удалить записи — запросы muse `delete from runtime.search_documents
    /// where owner_table = 'memory.entries' and owner_key in (…)` и
    /// `delete from memory.entries where memory_uri in (…)`; векторы и
    /// признаки — до записей (у muse на них внешние ключи).
    pub async fn delete_entries(&self, uris: &[String]) -> Result<()> {
        if uris.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("delete from runtime.search_documents where owner_table = 'memory.entries' and owner_key = any($1)")
            .bind(uris)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        for table in ["memory.embeddings", "memory.entry_attributes"] {
            sqlx::query(&format!(
                "delete from {table} where memory_entry_id in (select memory_entry_id from memory.entries where memory_uri = any($1))"
            ))
            .bind(uris)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        sqlx::query("delete from memory.entries where memory_uri = any($1)")
            .bind(uris)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(())
    }

    /// Адреса записей одного файла. Своё: у muse выборка по файлу не видна,
    /// есть только `entries.page` по адресам.
    pub async fn uris_for_path(&self, citation_path: &str) -> Result<Vec<String>> {
        sqlx::query_scalar("select memory_uri from memory.entries where citation_path = $1 order by memory_uri")
            .bind(citation_path)
            .fetch_all(&self.pool)
            .await
            .map_err(db)
    }

    /// Запись по адресу или `chunk_id` — запрос muse `entries.get`.
    pub async fn get_entry(&self, uri: &str, chunk_id: &str) -> Result<Option<Entry>> {
        let q = format!(
            "{ENTRY_SELECT} where e.memory_uri = $1 or e.chunk_id = $2 order by (e.memory_uri = $1) desc, e.created_at desc, e.memory_entry_id desc limit 1"
        );
        let row = sqlx::query(&q).bind(uri).bind(chunk_id).fetch_optional(&self.pool).await.map_err(db)?;
        Ok(row.as_ref().map(entry))
    }

    /// Записи по адресам — muse `entries.fetch_by_uris`.
    pub async fn entries_by_uris(&self, uris: &[String]) -> Result<Vec<Entry>> {
        let q = format!("{ENTRY_SELECT} where e.memory_uri = any($1)");
        let rows = sqlx::query(&q).bind(uris).fetch_all(&self.pool).await.map_err(db)?;
        Ok(rows.iter().map(entry).collect())
    }

    /// Записи, чьи строки покрывают строку `line` файла (для ссылок вида `MEMORY.md#L7`).
    pub async fn entries_at_line(&self, citation_path: &str, line: i64) -> Result<Vec<Entry>> {
        let q = format!("{ENTRY_SELECT} where e.citation_path = $1 and e.line_start <= $2 and e.line_end >= $2");
        let rows = sqlx::query(&q).bind(citation_path).bind(line).fetch_all(&self.pool).await.map_err(db)?;
        Ok(rows.iter().map(entry).collect())
    }

    // ---- поиск ----

    /// Поиск по словам — запрос muse `entries.search_sparse_rank`
    /// (`websearch_to_tsquery`, `ts_rank_cd`). Пропуск фильтра в ядре
    /// заполнен своим: только активные записи.
    pub async fn search_sparse(&self, query: &str, limit: i64) -> Result<Vec<(String, f64)>> {
        let rows = sqlx::query(
            "with q as (select websearch_to_tsquery('english', $1) as query)
                     select e.memory_uri,
                            ts_rank_cd(sd.search_vector, q.query)::double precision as rank_score
                     from memory.entries e
                     cross join q
                     join runtime.search_documents sd
                       on sd.owner_table = 'memory.entries'
                      and sd.owner_key = e.memory_uri
                     where sd.search_vector @@ q.query
                       and e.status = 'active'
                     order by rank_score desc, e.created_at desc limit $2",
        )
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        Ok(rows.iter().map(|r| (r.get("memory_uri"), r.get("rank_score"))).collect())
    }

    /// Поиск по векторам — запрос muse `entries.search_dense` (cosine,
    /// `hnsw.iterative_scan = 'strict_order'`).
    pub async fn search_dense(&self, vector: &[f32], model_id: i64, limit: i64) -> Result<Vec<(String, f64)>> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("set local hnsw.iterative_scan = 'strict_order'").execute(&mut *tx).await.map_err(db)?;
        let rows = sqlx::query(
            "select e.memory_uri,
                            greatest(
                                0.0::double precision,
                                1.0::double precision - (emb.embedding <=> $1::vector)
                            ) as similarity
                     from memory.embeddings emb
                     join memory.entries e on e.memory_entry_id = emb.memory_entry_id
                     where emb.embedding_model_id = $2 and e.status = 'active'
                     order by emb.embedding <=> $1::vector limit $3",
        )
        .bind(vector_text(vector))
        .bind(model_id)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(rows.iter().map(|r| (r.get("memory_uri"), r.get("similarity"))).collect())
    }

    // ---- векторы ----

    /// Модель векторов — запрос muse `insert into memory.embedding_models … returning`.
    pub async fn embedding_model(&self, name: &str) -> Result<i64> {
        sqlx::query_scalar(
            "insert into memory.embedding_models (model_name, dimensions, distance_metric)
         values ($1, $2, $3)
         on conflict (model_name, dimensions, distance_metric) do update set
           model_name = memory.embedding_models.model_name
         returning embedding_model_id",
        )
        .bind(name)
        .bind(DIMENSIONS as i32)
        .bind(DISTANCE)
        .fetch_one(&self.pool)
        .await
        .map_err(db)
    }

    /// Записать вектор — запрос muse `insert into memory.embeddings … $3::vector`.
    pub async fn put_embedding(&self, entry_id: i64, model_id: i64, v: &[f32]) -> Result<()> {
        if v.len() != DIMENSIONS {
            return Err(format!("embedding has {} dimensions; memory stores vector({DIMENSIONS})", v.len()));
        }
        sqlx::query(
            "insert into memory.embeddings (memory_entry_id, embedding_model_id, embedding)
         values ($1, $2, $3::vector)
         on conflict (memory_entry_id, embedding_model_id) do update set
           embedding = excluded.embedding,
           created_at = now()",
        )
        .bind(entry_id)
        .bind(model_id)
        .bind(vector_text(v))
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Активные записи без вектора этой модели — их дозаполняет сверка
    /// (форма muse: векторы пишутся отдельно от записи, `select exists(select 1
    /// from memory.embeddings where memory_entry_id = $1)`).
    pub async fn entries_without_embedding(&self, model_id: i64, limit: i64) -> Result<Vec<(i64, String)>> {
        let rows = sqlx::query(
            "select e.memory_entry_id, e.body_text from memory.entries e
             where e.status = 'active' and not exists (
               select 1 from memory.embeddings m where m.memory_entry_id = e.memory_entry_id and m.embedding_model_id = $1)
             order by e.memory_entry_id limit $2",
        )
        .bind(model_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
    }

    // ---- служебные метки ----

    /// Запрос muse `metadata.get`.
    pub async fn metadata(&self, key: &str) -> Result<Option<String>> {
        sqlx::query_scalar("select value from memory.metadata where key = $1 limit 1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)
    }

    /// Запрос muse `metadata.set`.
    pub async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "insert into memory.metadata (key, value, updated_at)
                     values ($1, $2, now())
                     on conflict (key) do update set value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    /// Запрос muse `metadata.delete`.
    pub async fn delete_metadata(&self, key: &str) -> Result<()> {
        sqlx::query("delete from memory.metadata where key = $1").bind(key).execute(&self.pool).await.map_err(db)?;
        Ok(())
    }

    /// Метки с приставкой. Своё: у muse выборка по приставке не видна.
    pub async fn metadata_with_prefix(&self, prefix: &str) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query("select key, value from memory.metadata where starts_with(key, $1) order by key")
            .bind(prefix)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
    }

    // ---- утверждения ----

    /// Записать утверждение. Как у muse: сначала ищется то же утверждение
    /// (тот же id или тот же текст среди активных) — оно подкрепляется
    /// (`reinforced_at`); иначе новое (`on conflict (claim_id) do nothing`).
    /// С `supersedes` старое помечается `superseded`, а новое запоминает,
    /// что заменило. Возвращает id записанного или найденного утверждения.
    pub async fn add_claim(&self, c: &NewClaim) -> Result<String> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let found: Option<String> = sqlx::query_scalar(
            "select claim_id from memory.claims
                     where claim_id = $1
                        or (status = 'active' and rtrim(lower(regexp_replace(claim_text, '\\s+', ' ', 'g')), '.!?') = $2)
                     order by (claim_id = $1) desc, first_seen asc
                     limit 1",
        )
        .bind(&c.claim_id)
        .bind(normalized(&c.claim_text))
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let id = match found {
            Some(id) => {
                sqlx::query(
                    "update memory.claims
                         set reinforced_at = greatest(reinforced_at, coalesce($2::timestamptz, now())),
                             updated_at = now()
                         where claim_id = $1",
                )
                .bind(&id)
                .bind(&c.first_seen)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
                id
            }
            None => {
                sqlx::query(
                    "insert into memory.claims
                           (claim_id, run_id, kind, salience, claim_text, quote, speaker,
                            evidence_handles, status, confidence, first_seen, reinforced_at,
                            valid_until, source_path, source_line)
                         values ($1, $2, $3, $4, $5, $6, $7, $8, 'active', $9,
                                 coalesce($10::timestamptz, now()), coalesce($10::timestamptz, now()),
                                 $11::timestamptz, $12, $13)
                         on conflict (claim_id) do nothing",
                )
                .bind(&c.claim_id)
                .bind(&c.run_id)
                .bind(&c.kind)
                .bind(&c.salience)
                .bind(&c.claim_text)
                .bind(&c.quote)
                .bind(&c.speaker)
                .bind(serde_json::json!(c.evidence_handles))
                .bind(c.confidence)
                .bind(&c.first_seen)
                .bind(&c.valid_until)
                .bind(&c.source_path)
                .bind(c.source_line)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
                c.claim_id.clone()
            }
        };
        if let Some(old) = &c.supersedes {
            if old != &id {
                sqlx::query(
                    "update memory.claims
                     set status = 'superseded', updated_at = now()
                     where claim_id = $1 and status = 'active'",
                )
                .bind(old)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
                sqlx::query(
                    "update memory.claims
                     set supersedes_claim_id = coalesce(supersedes_claim_id, $2), updated_at = now()
                     where claim_id = $1",
                )
                .bind(&id)
                .bind(old)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            }
        }
        tx.commit().await.map_err(db)?;
        Ok(id)
    }

    pub async fn claim(&self, id: &str) -> Result<Option<Claim>> {
        let row = sqlx::query(&format!("select {CLAIM_COLUMNS} from memory.claims where claim_id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(row.as_ref().map(claim))
    }

    /// Что утверждение заменило и чем заменено — запрос muse
    /// `claims.with_replacements`: `where supersedes_claim_id = $1 or claim_id =
    /// (select supersedes_claim_id from memory.claims where claim_id = $1)`.
    /// Возвращает (более ранние, более поздние).
    pub async fn replacements(&self, id: &str) -> Result<(Vec<Claim>, Vec<Claim>)> {
        let rows = sqlx::query(&format!(
            "select {CLAIM_COLUMNS} from memory.claims
                     where supersedes_claim_id = $1
                        or claim_id = (select supersedes_claim_id from memory.claims where claim_id = $1)
                     order by first_seen, claim_id"
        ))
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        let all: Vec<Claim> = rows.iter().map(claim).collect();
        let (later, earlier): (Vec<Claim>, Vec<Claim>) =
            all.into_iter().partition(|c| c.supersedes_claim_id.as_deref() == Some(id));
        Ok((earlier, later))
    }

    /// Отозвать утверждения — запрос muse `update memory.claims set status = $2
    /// … where claim_id = any($1) and status <> $2` со статусом `retracted`.
    pub async fn set_claims_status(&self, ids: &[String], status: &str) -> Result<u64> {
        let r = sqlx::query(
            "update memory.claims
                     set status = $2, updated_at = now()
                     where claim_id = any($1) and status <> $2",
        )
        .bind(ids)
        .bind(status)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(r.rows_affected())
    }

    /// Утверждения, выросшие из этих: те, что их заменили, и далее по цепочке.
    /// Своё: у muse «everything derived from them» (`strings-all.txt:55423`), запрос не виден.
    pub async fn derived_claims(&self, ids: &[String]) -> Result<Vec<String>> {
        sqlx::query_scalar(
            "with recursive d(claim_id) as (
               select claim_id from memory.claims where claim_id = any($1)
               union
               select c.claim_id from memory.claims c join d on c.supersedes_claim_id = d.claim_id)
             select claim_id from d order by claim_id",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(db)
    }

    /// Записи, которые несут утверждение (`attribute_name = 'claim_id'`).
    pub async fn entries_with_claims(&self, ids: &[String]) -> Result<Vec<String>> {
        sqlx::query_scalar(
            "select e.memory_uri from memory.entries e
             join memory.entry_attributes a on a.memory_entry_id = e.memory_entry_id
             where a.attribute_name = 'claim_id' and a.scalar_value = any($1)
             order by e.memory_uri",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(db)
    }

    /// Истёкшие утверждения — запрос muse `set status = 'expired' … where
    /// valid_until is not null and status = 'active' and valid_until <= $1`.
    pub async fn expire_claims(&self, now: &str) -> Result<Vec<String>> {
        sqlx::query_scalar(
            "update memory.claims
                     set status = 'expired', updated_at = now()
                     where valid_until is not null and status = 'active'
                       and valid_until <= $1::timestamptz
                     returning claim_id",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(db)
    }
}
