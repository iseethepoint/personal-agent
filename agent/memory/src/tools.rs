//! Инструменты памяти — форма muse `hatch-agent/src/tools/memory.rs`,
//! `memory/file_reader.rs`, `memory_explain.rs` (`source-files.txt:1631-1635`).
//!
//! Описания — строки ядра muse, полные варианты, личный вариант:
//! `memory_get` — 0x2ccf3b9–0x2ccf7bf, `memory_search` — 0x2ccf8c8–0x2ccfe41,
//! `memory_explain` — 0x2ccff61–0x2cd01bc (`strings-all.txt:332201-332226`);
//! заметка к выдаче — 0xa2f8e1 (`strings-all.txt:77980`); тексты explain —
//! 0x8f5dc3 (`strings-all.txt:74472`).
//! Отличия от muse: имена `hamlet.memory_*` вместо `muse.memory_*` (слово
//! владельца, план П10).
//!
//! «Past conversations» (П3е-6) — раздел выдачи с совпадениями по словам из
//! прошлых разговоров главного чата, как у muse: описание и параметры —
//! вариант ядра с этим разделом (около `0x8fa116`, `0x8facc2`); поиск —
//! форма ядра по `runtime.search_documents` (`websearch_to_tsquery('simple',
//! $1)` или префиксный `to_tsquery('simple', $2)`, новые сверху,
//! `ts_headline(… 'StartSel="", StopSel="", MaxWords=24, MinWords=8')`,
//! около `0x175c52f`, `0x35da36`); сбой одной половины — тексты ядра
//! (около `0x6da6ff`). Читать совпадение — `chat.read_messages`.
//!
//! Сверка базы с файлами идёт в фоне (`maintain`, её зовёт служба расписаний),
//! как у muse: выдача может отставать от записи файлов («Search results can
//! lag file writes»).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools};
use agent_model::{ToolResultPart, ToolSpec};
use serde_json::{json, Value};

use crate::files::reindex;
use crate::forget::apply_pending;
use crate::store::{Claim, Entry, Store};
use crate::Embedder;

const GET_DESCRIPTION: &str = "Read a snippet from `~/MEMORY.md`, `~/memory/**/*.md`,\n`~/side-chats/<chat-id>/MEMORY.md`, or `~/side-chats/<chat-id>/memory/**/*.md`.\nUse optional from/lines after hamlet.memory_search to read only the needed lines.\nPass a search citation such as `side-chats/<chat-id>/MEMORY.md#L7` as the path\nexactly as cited.";
const GET_PATH: &str = "MEMORY.md or memory/<date>.md bare, ~/-prefixed, or absolute under home. A memory_search citation like MEMORY.md#L7 or memory/2026-08-10.md#L12-L15 works as-is; its line suffix sets the read window.";
const GET_FROM: &str = "1-based first line to return; overrides a citation's window.";
const GET_LINES: &str = "Number of lines to return from `from`.";

const SEARCH_DESCRIPTION: &str = "Search <!-- personal-only:start -->personal<!-- personal-only:end --><!-- group-only:start -->the group's memory<!-- group-only:end --> and side-chat memory: `~/MEMORY.md`, `~/memory/**/*.md`,\n`~/side-chats/<chat-id>/MEMORY.md`, and `~/side-chats/<chat-id>/memory/**/*.md`.\nResults include snippets with paths and line numbers. Side-chat configuration is excluded.\nApply the <!-- personal-only:start -->user's<!-- personal-only:end --><!-- group-only:start -->group's and members'<!-- group-only:end --> preferences and instructions within their original scope. A past assistant decision to stop or limit retries does not create a <!-- personal-only:start -->user-imposed retry limit<!-- personal-only:end --><!-- group-only:start -->retry limit a member set<!-- group-only:end -->. Preserve reported service restrictions that still apply, such as rate limits or account locks. When a summary leaves the source or scope of a restriction unclear and it affects your next action, read its cited source.\nPass 2-3 close recall phrasings as a `queries` array. Keep `queries[0]` closest to the <!-- personal-only:start -->user's<!-- personal-only:end --><!-- group-only:start -->member's<!-- group-only:end --> wording.";
/// Описание с разделом «Past conversations» — строка ядра (личный и
/// групповой варианты), `muse.memory_get` → `hamlet.memory_get`.
const SEARCH_WITH_CONVERSATIONS: &str = "Search saved memory and past conversations for context relevant to the <!-- personal-only:start -->user's<!-- personal-only:end --><!-- group-only:start -->member's<!-- group-only:end --> message or the work at hand.\n\nApply the <!-- personal-only:start -->user's<!-- personal-only:end --><!-- group-only:start -->group's and members'<!-- group-only:end --> preferences and instructions within their original scope. A past assistant decision to stop or limit retries does not create a <!-- personal-only:start -->user-imposed retry limit<!-- personal-only:end --><!-- group-only:start -->retry limit a member set<!-- group-only:end -->. Preserve reported service restrictions that still apply, such as rate limits or account locks. When a summary leaves the source or scope of a restriction unclear and it affects your next action, read its cited source.\n\nResults have separate sections. Memories searches `~/MEMORY.md`, `~/memory/**/*.md`, and side-chat memory under `~/side-chats/<chat-id>/`. Audio notes appear separately when matched. Open their path and line references with `hamlet.memory_get`.\n\nPast conversations returns matches from the <!-- personal-only:start -->main chat and personal<!-- personal-only:end --><!-- group-only:start -->group's thread and your<!-- group-only:end --> side chats, with a `chat_id`, `message_id`, timestamp and text excerpt. Read a match and surrounding messages with `chat.read_messages` using its `message_id`. A past statement records what was said then, not whether it remains true now.\n\nPass one to three queries. Keep the first close to the <!-- personal-only:start -->user's<!-- personal-only:end --><!-- group-only:start -->member's<!-- group-only:end --> wording and use short, specific names or terms in another query to find past conversations. `maxResults` limits each source separately. `minScore` applies only to saved memories.";
const SEARCH_MAX_WITH_CONVERSATIONS: &str = "Maximum matches per source: saved memories and past conversations. Defaults to 6; at most 200 memories and 100 conversation matches.";
const SEARCH_MIN_WITH_CONVERSATIONS: &str = "Minimum relevance score for saved memories. Does not filter keyword matches from past conversations.";
const CONVERSATIONS_FAILED: &str = "Past conversation search failed; memory lookup completed.";
const MEMORY_FAILED: &str = "Memory lookup failed; past conversation search completed.";
const BOTH_FAILED: &str = "Memory and past conversation search failed.";
/// «at most … 100 conversation matches».
const MAX_CONVERSATIONS: usize = 100;

const SEARCH_QUERIES: &str = "One to three memory search queries.";
const SEARCH_MAX: &str = "Number of hits to return.";
const SEARCH_MIN: &str = "Optional final-score floor between 0 and 1.";
const SEARCH_NOTE: &str = "Read more with `hamlet.memory_get`, using the result's file `path` and starting line `from`. Set `lines` to include the surrounding context you need.";

const EXPLAIN_DESCRIPTION: &str = "Show where one memory came from and what replaced what. Pass a claim id (from\nhamlet.memory_search result metadata or a dated log's `claim:` line), a\n`memory://` uri, or a citation like `MEMORY.md#L7`. Returns the claim, its\nkind and salience, who said it and the quote, when it was first learned and\nlast reinforced, its current confidence, the conversation messages it rests\non with their dates, the earlier claims it replaced and the later claims that\nreplaced it, and the indexed line when one exists. Read only.";
const EXPLAIN_ID: &str = "A claim id, a memory:// uri, or a citation like MEMORY.md#L7 or memory/2026-08-10.md#L12.";
const EARLIER: &str = "Replaced these earlier claims";
const LATER: &str = "Replaced by these later claims";
const NO_CLAIM: &str = "No verified claim stands behind this memory; it was indexed from the file line below without a claim record.";

/// По умолчанию — 6 совпадений, не больше 200 (описание `maxResults` у
/// muse: «Defaults to 6; at most 200 memories», `strings-all.txt:74511`).
const DEFAULT_RESULTS: usize = 6;
const MAX_RESULTS: usize = 200;
const MAX_QUERIES: usize = 3;
/// Окно `memory_get` без ссылки и без `lines`. Своё: у muse не видно.
const DEFAULT_LINES: usize = 40;

#[derive(Clone)]
pub struct MemoryTools {
    home: PathBuf,
    store: Store,
    embedder: Option<Arc<dyn Embedder>>,
    /// Агент главного чата, по разговорам которого ищет «Past
    /// conversations»; `None` — раздела нет.
    conversations: Option<String>,
}

impl MemoryTools {
    pub fn new(home: impl Into<PathBuf>, store: Store, embedder: Option<Arc<dyn Embedder>>) -> Self {
        MemoryTools { home: home.into(), store, embedder, conversations: None }
    }

    /// Искать и по прошлым разговорам агента `agent_id` (главный чат).
    pub fn with_conversations(mut self, agent_id: &str) -> Self {
        self.conversations = Some(agent_id.into());
        self
    }

    pub fn add_to(&self, tools: &mut Tools) {
        tools.add(Search(self.clone()));
        tools.add(Get(self.clone()));
        tools.add(Explain(self.clone()));
    }

    /// Фоновая работа памяти: применить забывание, если `pending.json`
    /// остался, и сверить базу с файлами. У muse сверку запускает слежение
    /// за файлами (`notify::inotify`, `strings-all.txt:344927`; «file watcher
    /// backend error», `:17841`; `maintenance_reindex trigger=`, `:19814`) и
    /// запуск службы; у нас — такт службы расписаний (`agent-cron`).
    pub async fn maintain(&self) -> Result<(), String> {
        let emb = self.embedder.as_deref();
        apply_pending(&self.home, &self.store, emb).await?;
        reindex(&self.home, &self.store, emb).await?;
        Ok(())
    }
}

fn prop(t: &str, d: &str) -> Value {
    json!({"type": t, "description": d})
}

fn text(v: String) -> Vec<ToolResultPart> {
    vec![ToolResultPart::Text { text: v }]
}

/// Ссылка `path#L7` или `path#L12-L15` → (путь, окно).
fn split_citation(raw: &str) -> (&str, Option<(usize, usize)>) {
    let Some((path, anchor)) = raw.rsplit_once("#L") else { return (raw, None) };
    let (a, b) = match anchor.split_once("-L") {
        Some((a, b)) => (a, b),
        None => (anchor, anchor),
    };
    match (a.parse::<usize>(), b.parse::<usize>()) {
        (Ok(a), Ok(b)) if a >= 1 && b >= a => (path, Some((a, b))),
        _ => (raw, None),
    }
}

/// Путь модели → путь файла памяти от дома, если это файл памяти.
fn memory_rel(home: &Path, raw: &str) -> Option<String> {
    let raw = raw.trim();
    let rel = if let Some(r) = raw.strip_prefix("~/") {
        r.to_string()
    } else if Path::new(raw).is_absolute() {
        Path::new(raw).strip_prefix(home).ok()?.to_string_lossy().to_string()
    } else {
        raw.to_string()
    };
    if rel.split('/').any(|c| c == ".." || c.is_empty()) {
        return None;
    }
    let inner = match rel.strip_prefix("side-chats/") {
        Some(rest) => rest.split_once('/')?.1,
        None => rel.as_str(),
    };
    let ok = inner == "MEMORY.md" || (inner.starts_with("memory/") && inner.ends_with(".md"));
    ok.then_some(rel)
}

fn citation(e: &Entry) -> String {
    if e.line_start == e.line_end {
        format!("{}#L{}", e.citation_path, e.line_start)
    } else {
        format!("{}#L{}-L{}", e.citation_path, e.line_start, e.line_end)
    }
}

// ---- hamlet.memory_search ----

struct Search(MemoryTools);

impl Tool for Search {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "hamlet.memory_search".into(),
            description: personal(if self.0.conversations.is_some() { SEARCH_WITH_CONVERSATIONS } else { SEARCH_DESCRIPTION }),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queries": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": MAX_QUERIES, "description": SEARCH_QUERIES},
                    "maxResults": prop("integer", if self.0.conversations.is_some() { SEARCH_MAX_WITH_CONVERSATIONS } else { SEARCH_MAX }),
                    "minScore": prop("number", if self.0.conversations.is_some() { SEARCH_MIN_WITH_CONVERSATIONS } else { SEARCH_MIN }),
                },
                "required": ["queries"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let m = &self.0;
            let queries: Vec<String> = args
                .get("queries")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::trim).filter(|q| !q.is_empty()).map(String::from).collect())
                .unwrap_or_default();
            if queries.is_empty() || queries.len() > MAX_QUERIES {
                return Err(ToolError::InvalidArguments("`queries` needs one to three non-empty memory search queries".into()));
            }
            let max = args.get("maxResults").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(DEFAULT_RESULTS).clamp(1, MAX_RESULTS);
            let min = args.get("minScore").and_then(Value::as_f64).unwrap_or(0.0);
            if !(0.0..=1.0).contains(&min) {
                return Err(ToolError::InvalidArguments("`minScore` must be between 0 and 1".into()));
            }
            let memories = search_memories(m, &queries, max, min).await;
            let Some(agent) = &m.conversations else {
                return Ok(text(json!({"memories": memories.map_err(ToolError::Failed)?, "note": SEARCH_NOTE}).to_string()));
            };
            let past = past_conversations(m, agent, &queries, max.min(MAX_CONVERSATIONS)).await;
            let out = match (memories, past) {
                (Ok(mem), Ok(p)) => json!({"memories": mem, "past_conversations": p, "note": SEARCH_NOTE}),
                (Ok(mem), Err(_)) => json!({"memories": mem, "status": "partial", "warning": CONVERSATIONS_FAILED, "note": SEARCH_NOTE}),
                (Err(_), Ok(p)) => json!({"past_conversations": p, "status": "partial", "warning": MEMORY_FAILED, "note": SEARCH_NOTE}),
                (Err(_), Err(_)) => return Err(ToolError::Failed(BOTH_FAILED.into())),
            };
            Ok(text(out.to_string()))
        })
    }
}

/// Сохранённые воспоминания: лучшие по оценке не ниже `min`.
async fn search_memories(m: &MemoryTools, queries: &[String], max: usize, min: f64) -> Result<Vec<Value>, String> {
    let scores = score(m, queries, max).await?;
    let mut hits: Vec<(String, f64)> = scores.into_iter().filter(|(_, s)| *s >= min).collect();
    hits.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    hits.truncate(max);
    let uris: Vec<String> = hits.iter().map(|(u, _)| u.clone()).collect();
    let entries = m.store.entries_by_uris(&uris).await?;
    let memories: Vec<Value> = hits
        .iter()
        .filter_map(|(u, s)| {
            let e = entries.iter().find(|e| &e.memory_uri == u)?;
            let mut hit = json!({
                "path": e.citation_path,
                "from": e.line_start,
                "lines": e.line_end - e.line_start + 1,
                "citation": citation(e),
                "score": (s * 1000.0).round() / 1000.0,
                "source_type": e.source_type,
                "snippet": e.text,
            });
            if let Some(c) = e.metadata.get("claim_id") {
                hit["metadata"] = json!({"claim_id": c});
            }
            Some(hit)
        })
        .collect();
    Ok(memories)
}

/// Префиксный запрос: каждое слово — и как начало слова (`слово:*`). Своё:
/// как ядро строит второй запрос, не видно; берутся буквы и цифры.
fn prefix_query(q: &str) -> String {
    q.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("{}:*", w.to_lowercase()))
        .collect::<Vec<_>>()
        .join(" & ")
}

/// «Past conversations»: реплики пользователя и ассистента агента `agent`,
/// совпавшие по словам с любым запросом, новые сверху. Своё: поля
/// совпадения — `chat_id` (`main`), `message_id`, `timestamp` (RFC 3339),
/// `speaker` (`user` / `assistant`), `excerpt`; совпадения запросов
/// сливаются по `message_id`.
async fn past_conversations(m: &MemoryTools, agent: &str, queries: &[String], max: usize) -> Result<Vec<Value>, String> {
    let mut rows: Vec<(i64, Value)> = vec![];
    for q in queries {
        let found: Vec<(i64, String, String, String, String)> = sqlx::query_as(
            "select ci.seq, ci.message_id, ci.role::text, to_char(ci.created_at at time zone 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), \
             ts_headline('simple', coalesce(sd.search_text, ''), websearch_to_tsquery('simple', $1), 'StartSel=\"\", StopSel=\"\", MaxWords=24, MinWords=8') \
             from runtime.search_documents sd join agent.context_items ci on sd.owner_table = 'agent.context_items' and sd.owner_key = ci.agent_id || ':' || ci.seq \
             where ci.agent_id = $3 and (sd.search_vector @@ websearch_to_tsquery('simple', $1) or ($2 <> '' and sd.search_vector @@ to_tsquery('simple', $2))) \
             order by ci.seq desc limit $4",
        )
        .bind(q)
        .bind(prefix_query(q))
        .bind(agent)
        .bind(max as i64)
        .fetch_all(m.store.pool())
        .await
        .map_err(|e| e.to_string())?;
        for (seq, message_id, speaker, ts, excerpt) in found {
            if !rows.iter().any(|(_, v)| v["message_id"] == message_id.as_str()) {
                rows.push((seq, json!({"chat_id": "main", "message_id": message_id, "timestamp": ts, "speaker": speaker, "excerpt": excerpt})));
            }
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    rows.truncate(max);
    Ok(rows.into_iter().map(|(_, v)| v).collect())
}

/// Оценка от 0 до 1 на запись: лучшая по всем запросам. По словам — ранг
/// `ts_rank_cd`, делённый на лучший ранг запроса; по векторам — сходство
/// cosine. Своё: как muse сводит два поиска, в ядре не видно.
async fn score(m: &MemoryTools, queries: &[String], max: usize) -> Result<Vec<(String, f64)>, String> {
    let pool = (max * 4) as i64;
    let mut best: Vec<(String, f64)> = Vec::new();
    let mut put = |u: String, s: f64| match best.iter_mut().find(|(b, _)| *b == u) {
        Some(slot) => slot.1 = slot.1.max(s),
        None => best.push((u, s)),
    };
    for q in queries {
        let sparse = m.store.search_sparse(q, pool).await?;
        let top = sparse.first().map(|(_, r)| *r).unwrap_or(0.0);
        for (u, r) in sparse {
            put(u, if top > 0.0 { r / top } else { 0.0 });
        }
    }
    if let Some(emb) = &m.embedder {
        // Модель не ответила — остаётся поиск по словам.
        if let Ok(vs) = emb.embed(queries).await {
            let model = m.store.embedding_model(&emb.model_name()).await?;
            for v in vs {
                for (u, s) in m.store.search_dense(&v, model, pool).await? {
                    put(u, s);
                }
            }
        }
    }
    Ok(best)
}

// ---- hamlet.memory_get ----

struct Get(MemoryTools);

impl Tool for Get {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "hamlet.memory_get".into(),
            description: personal(GET_DESCRIPTION),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": prop("string", GET_PATH),
                    "from": prop("integer", GET_FROM),
                    "lines": prop("integer", GET_LINES),
                },
                "required": ["path"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let home = &self.0.home;
            let raw = args.get("path").and_then(Value::as_str).unwrap_or("");
            let (path, window) = split_citation(raw);
            let rel = memory_rel(home, path).ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "`{raw}` is not a memory file: use ~/MEMORY.md, ~/memory/**/*.md, or side-chat memory under ~/side-chats/<chat-id>/"
                ))
            })?;
            let body = std::fs::read_to_string(home.join(&rel)).map_err(|e| ToolError::Failed(format!("{rel}: {e}")))?;
            let all: Vec<&str> = body.lines().collect();
            let from = args.get("from").and_then(Value::as_u64).map(|n| n as usize).or(window.map(|w| w.0)).unwrap_or(1).max(1);
            let lines = args
                .get("lines")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .or(window.map(|(a, b)| b - a + 1))
                .unwrap_or(DEFAULT_LINES)
                .max(1);
            let start = (from - 1).min(all.len());
            let end = (start + lines).min(all.len());
            Ok(text(
                json!({"path": rel, "from": from, "lines": end - start, "total_lines": all.len(), "text": all[start..end].join("\n")})
                    .to_string(),
            ))
        })
    }
}

// ---- hamlet.memory_explain ----

struct Explain(MemoryTools);

fn claim_block(c: &Claim) -> String {
    let mut s = format!(
        "claim {}\n- text: {}\n- kind: {}\n- salience: {}\n- speaker: {}\n",
        c.claim_id, c.claim_text, c.kind, c.salience, c.speaker
    );
    if let Some(q) = &c.quote {
        s.push_str(&format!("- quote: {q}\n"));
    }
    s.push_str(&format!(
        "- status: {}\n- confidence: {}\n- first learned: {}\n- last reinforced: {}\n",
        c.status, c.confidence, c.first_seen, c.reinforced_at
    ));
    if let Some(v) = &c.valid_until {
        s.push_str(&format!("- valid until: {v}\n"));
    }
    s.push_str(&format!("- source: {}#L{}\n- evidence: {}\n", c.source_path, c.source_line, c.evidence_handles));
    s
}

fn brief(c: &Claim) -> String {
    format!("- {} ({}): {}\n", c.claim_id, c.status, c.claim_text)
}

impl Tool for Explain {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "hamlet.memory_explain".into(),
            description: personal(EXPLAIN_DESCRIPTION),
            input_schema: json!({
                "type": "object",
                "properties": {"id": prop("string", EXPLAIN_ID)},
                "required": ["id"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let m = &self.0;
            let id = args.get("id").and_then(Value::as_str).unwrap_or("").trim().to_string();
            if id.is_empty() {
                return Err(ToolError::InvalidArguments("`id` needs a claim id, a memory:// uri, or a citation like MEMORY.md#L7".into()));
            }
            let f = |e: String| ToolError::Failed(e);
            // Запись: по адресу, по ссылке или нет (тогда id — утверждение).
            let entry = if id.starts_with("memory://") {
                m.store.get_entry(&id, &id).await.map_err(f)?
            } else if let (path, Some((line, _))) = split_citation(&id) {
                let rel = memory_rel(&m.home, path)
                    .ok_or_else(|| ToolError::InvalidArguments(format!("`{id}` is not a memory citation")))?;
                m.store.entries_at_line(&rel, line as i64).await.map_err(f)?.into_iter().next()
            } else {
                None
            };
            let claim_id = match &entry {
                Some(e) => e.metadata.get("claim_id").and_then(Value::as_str).map(String::from),
                None => Some(id.clone()),
            };
            let claim = match &claim_id {
                Some(c) => m.store.claim(c).await.map_err(f)?,
                None => None,
            };
            let mut out = String::new();
            match &claim {
                Some(c) => {
                    out.push_str(&claim_block(c));
                    let (earlier, later) = m.store.replacements(&c.claim_id).await.map_err(f)?;
                    if !earlier.is_empty() {
                        out.push_str(&format!("{EARLIER}:\n"));
                        earlier.iter().for_each(|c| out.push_str(&brief(c)));
                    }
                    if !later.is_empty() {
                        out.push_str(&format!("{LATER}:\n"));
                        later.iter().for_each(|c| out.push_str(&brief(c)));
                    }
                }
                None if entry.is_some() => out.push_str(&format!("{NO_CLAIM}\n")),
                None => return Err(ToolError::Failed(format!("no memory or claim matches `{id}`"))),
            }
            let entry = match (entry, &claim) {
                (Some(e), _) => Some(e),
                (None, Some(c)) => {
                    let uris = m.store.entries_with_claims(std::slice::from_ref(&c.claim_id)).await.map_err(f)?;
                    m.store.entries_by_uris(&uris).await.map_err(f)?.into_iter().next()
                }
                _ => None,
            };
            if let Some(e) = entry {
                out.push_str(&format!("indexed line {}:\n{}\n", citation(&e), e.text));
            }
            Ok(text(out.trim_end().to_string()))
        })
    }
}
