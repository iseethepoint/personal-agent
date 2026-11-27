//! Файлы памяти и база — форма muse `hatch-memory/src/ingest/markdown_reader.rs`
//! и `maintenance/reindex.rs` (`source-files.txt:5467`, `:5471`).
//!
//! Источник правды — файлы: `~/MEMORY.md` и `~/memory/**/*.md` (описание
//! `muse.memory_search`, `strings-all.txt:332211-332213`). База — их индекс:
//! записи `memory.entries` с адресом `memory://md/<путь>#L<a>-L<b>` (форма
//! адресов muse `memory://md/MEMORY.md#…`, `strings-all.txt:37733`, `:40078`),
//! ссылкой `citation_path` и строками `line_start`/`line_end` — по ним
//! работают ссылки `MEMORY.md#L7` и `memory/2026-08-10.md#L12-L15`.
//!
//! Своё (в ядре не видно, как режет muse): запись — абзац, заголовок
//! начинает новую запись; `chunk_id` — отпечаток текста.

use std::path::{Path, PathBuf};

use crate::store::{NewEntry, Store};
use crate::Embedder;

/// Отпечаток файла в `memory.metadata` — ключ `file:<путь>`.
const FILE_KEY: &str = "file:";

/// Сколько записей векторизуется за один запрос к модели векторов. Своё.
const EMBED_BATCH: i64 = 64;

/// Вид источника — значения muse (`strings-all.txt:247272`: `daily`,
/// `memory_md`, `entity`, `person`, `group`). Какой каталог какому виду —
/// вывод по путям `memory/people/`, `memory/groups/` (`strings-all.txt:17363`).
pub fn source_type(rel: &str) -> &'static str {
    let rel = side_chat_inner(rel).unwrap_or(rel);
    if rel == "MEMORY.md" {
        "memory_md"
    } else if daily_date(rel).is_some() {
        "daily"
    } else if rel.starts_with("memory/people/") {
        "person"
    } else if rel.starts_with("memory/groups/") {
        "group"
    } else {
        "entity"
    }
}

/// Путь внутри памяти побочного чата: `side-chats/<id>/X` → `X`.
fn side_chat_inner(rel: &str) -> Option<&str> {
    rel.strip_prefix("side-chats/")?.split_once('/').map(|(_, r)| r)
}

/// Дата дневника `memory/YYYY-MM-DD.md` (и в памяти побочного чата).
pub fn daily_date(rel: &str) -> Option<&str> {
    let rel = side_chat_inner(rel).unwrap_or(rel);
    let name = rel.strip_prefix("memory/")?.strip_suffix(".md")?;
    let b = name.as_bytes();
    let ok = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() });
    ok.then_some(name)
}

/// FNV-1a, 64 бита: устойчивый отпечаток без лишних зависимостей.
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Кусок файла.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub line_start: i64,
    pub line_end: i64,
    pub text: String,
    /// Id утверждения из строки `claim: <id>` (у muse «a dated log's
    /// `claim:` line», `strings-all.txt:332220`).
    pub claim_id: Option<String>,
}

/// Порезать файл на абзацы; строки нумеруются с 1.
pub fn chunks(text: &str) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut cur: Vec<(i64, &str)> = Vec::new();
    let flush = |cur: &mut Vec<(i64, &str)>, out: &mut Vec<Chunk>| {
        if cur.is_empty() {
            return;
        }
        let claim_id = cur.iter().find_map(|(_, l)| {
            let t = l.trim().trim_start_matches(['-', '*']).trim();
            t.strip_prefix("claim:").map(|id| id.trim().to_string()).filter(|id| !id.is_empty())
        });
        out.push(Chunk {
            line_start: cur[0].0,
            line_end: cur[cur.len() - 1].0,
            text: cur.iter().map(|(_, l)| *l).collect::<Vec<_>>().join("\n"),
            claim_id,
        });
        cur.clear();
    };
    for (i, line) in text.lines().enumerate() {
        let n = i as i64 + 1;
        if line.trim().is_empty() {
            flush(&mut cur, &mut out);
        } else {
            if line.trim_start().starts_with('#') {
                flush(&mut cur, &mut out);
            }
            cur.push((n, line));
        }
    }
    flush(&mut cur, &mut out);
    out
}

pub fn uri(rel: &str, c: &Chunk) -> String {
    format!("memory://md/{rel}#L{}-L{}", c.line_start, c.line_end)
}

/// Файлы памяти, путь от дома, по порядку: `MEMORY.md`, `memory/**/*.md` и
/// то же в памяти побочных чатов `side-chats/<id>/` (описание muse
/// `memory_search`; настройки побочного чата не входят).
pub fn memory_files(home: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect(home, "", &mut out);
    let mut chats: Vec<_> = std::fs::read_dir(home.join("side-chats")).map(|r| r.flatten().collect()).unwrap_or_default();
    chats.sort_by_key(|e| e.file_name());
    for c in chats {
        let name = c.file_name().to_string_lossy().to_string();
        if c.path().is_dir() && !name.starts_with('.') {
            collect(&c.path(), &format!("side-chats/{name}/"), &mut out);
        }
    }
    out
}

/// Файл памяти ли `rel` (путь от дома) — те же правила, что у
/// `memory_files`: `MEMORY.md` и `memory/**/*.md` дома и боковых чатов.
pub fn is_memory_path(rel: &Path) -> bool {
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    let local = match parts.as_slice() {
        [first, _chat, rest @ ..] if first == "side-chats" => rest,
        all => all,
    };
    if local.iter().any(|p| p.starts_with('.')) {
        return false;
    }
    match local {
        [one] => one == "MEMORY.md" || one == "memory",
        [dir, .., last] => dir == "memory" && last.ends_with(".md"),
        [] => false,
    }
}

/// `MEMORY.md` и `memory/**/*.md` одной папки; `prefix` — её путь от дома.
fn collect(root: &Path, prefix: &str, out: &mut Vec<String>) {
    if root.join("MEMORY.md").is_file() {
        out.push(format!("{prefix}MEMORY.md"));
    }
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut items: Vec<_> = rd.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            let r = format!("{rel}/{name}");
            if path.is_dir() {
                walk(&path, &r, out);
            } else if name.ends_with(".md") {
                out.push(r);
            }
        }
    }
    walk(&root.join("memory"), &format!("{prefix}memory"), out);
}

/// Итог сверки.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Reindex {
    /// Файлы, чьи записи переписаны.
    pub files_indexed: Vec<String>,
    /// Файлы, которых больше нет: их записи удалены.
    pub files_removed: Vec<String>,
    pub embedded: usize,
    /// Модель векторов не ответила — поиск идёт по словам, векторы
    /// дозаполнятся при следующей сверке.
    pub embed_error: Option<String>,
}

/// Сверить базу с файлами: изменившийся файл переписывается целиком,
/// исчезнувший — удаляется; затем дозаполняются векторы.
pub async fn reindex(home: &Path, store: &Store, embedder: Option<&dyn Embedder>) -> Result<Reindex, String> {
    let mut report = Reindex::default();
    let files = memory_files(home);
    for rel in &files {
        let path: PathBuf = home.join(rel);
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let print = fingerprint(&bytes);
        let key = format!("{FILE_KEY}{rel}");
        if store.metadata(&key).await?.as_deref() == Some(print.as_str()) {
            continue;
        }
        index_file(store, rel, &String::from_utf8_lossy(&bytes)).await?;
        store.set_metadata(&key, &print).await?;
        report.files_indexed.push(rel.clone());
    }
    for (key, _) in store.metadata_with_prefix(FILE_KEY).await? {
        let rel = &key[FILE_KEY.len()..];
        if !files.iter().any(|f| f == rel) {
            let gone = store.uris_for_path(rel).await?;
            store.delete_entries(&gone).await?;
            store.delete_metadata(&key).await?;
            report.files_removed.push(rel.to_string());
        }
    }
    if let Some(emb) = embedder {
        let model = store.embedding_model(&emb.model_name()).await?;
        loop {
            let todo = store.entries_without_embedding(model, EMBED_BATCH).await?;
            if todo.is_empty() {
                break;
            }
            let texts: Vec<String> = todo.iter().map(|(_, t)| t.clone()).collect();
            match emb.embed(&texts).await {
                Ok(vs) => {
                    for ((id, _), v) in todo.iter().zip(vs) {
                        store.put_embedding(*id, model, &v).await?;
                        report.embedded += 1;
                    }
                }
                Err(e) => {
                    report.embed_error = Some(e);
                    break;
                }
            }
        }
    }
    Ok(report)
}

/// Переписать записи одного файла.
async fn index_file(store: &Store, rel: &str, text: &str) -> Result<(), String> {
    let kind = source_type(rel);
    let created_at = daily_date(rel).map(|d| format!("{d} 00:00:00+00"));
    let mut keep = Vec::new();
    for c in chunks(text) {
        let u = uri(rel, &c);
        let id = store
            .upsert_entry(&NewEntry {
                memory_uri: u.clone(),
                chunk_id: fingerprint(c.text.as_bytes()),
                source_type: kind.into(),
                status: "active".into(),
                privacy_class: "default".into(),
                body_text: c.text.clone(),
                confidence: 1.0,
                reason_text: None,
                citation_path: rel.into(),
                line_start: c.line_start,
                line_end: c.line_end,
                created_at: created_at.clone(),
                created_at_unix: 0,
            })
            .await?;
        match &c.claim_id {
            Some(claim) => store.set_attribute(id, "claim_id", claim).await?,
            None => store.delete_attribute(id, "claim_id").await?,
        }
        keep.push(u);
    }
    let stale: Vec<String> = store.uris_for_path(rel).await?.into_iter().filter(|u| !keep.contains(u)).collect();
    store.delete_entries(&stale).await
}
