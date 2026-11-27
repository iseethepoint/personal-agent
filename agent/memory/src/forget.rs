//! Забывание — часть среды. Форма muse: агент, выполнив план забывания,
//! пишет `~/workspace/memory/forget/pending.json` как
//! `{"claims":[…],"citations":[…]}` — id утверждений и места `MEMORY.md#L<n>`
//! или `memory/<date>.md#L<n>`, без самого текста (`strings-all.txt:55423`,
//! навык `forget`). Среда отзывает эти утверждения «и всё, что из них
//! выросло», затем обновляет поиск по памяти (`hatch-server/src/daemon/runtime/
//! forget_reindex.rs`, `strings-all.txt:39400`).
//!
//! Инструменты `forget.plan` и `forget.confirm` у muse запускают подагента,
//! который наследует разговор (`strings-all.txt:78661`, `:55418-55420`); в
//! нашем цикле подагенты пока только изолированные — эти инструменты здесь
//! не делаются.

use std::path::Path;

use serde::Deserialize;

use crate::files::{reindex, Reindex};
use crate::store::Store;
use crate::Embedder;

pub const PENDING: &str = "workspace/memory/forget/pending.json";

#[derive(Debug, Deserialize)]
struct Pending {
    claims: Vec<String>,
    citations: Vec<String>,
}

/// Итог забывания.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Forgotten {
    /// Отозванные утверждения, вместе с выросшими из них.
    pub retracted: Vec<String>,
    /// Записи поиска, которые несли эти утверждения и сняты.
    pub entries_removed: Vec<String>,
    pub citations: Vec<String>,
    pub reindex: Reindex,
}

/// Применить `pending.json`, если он есть: отозвать утверждения, снять
/// записи с ними, пересобрать поиск по файлам. Файл после этого удаляется.
/// `Ok(None)` — забывать нечего.
pub async fn apply_pending(home: &Path, store: &Store, embedder: Option<&dyn Embedder>) -> Result<Option<Forgotten>, String> {
    let path = home.join(PENDING);
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{PENDING}: {e}")),
    };
    let p: Pending = serde_json::from_str(&raw)
        .map_err(|e| format!("{PENDING} must be {{\"claims\":[...],\"citations\":[...]}}: {e}"))?;
    let retracted = store.derived_claims(&p.claims).await?;
    store.set_claims_status(&retracted, "retracted").await?;
    let entries_removed = store.entries_with_claims(&retracted).await?;
    store.delete_entries(&entries_removed).await?;
    // Записи по местам из citations перепишет сверка: файлы уже поправлены.
    let reindex = reindex(home, store, embedder).await?;
    std::fs::remove_file(&path).map_err(|e| format!("{PENDING}: {e}"))?;
    Ok(Some(Forgotten { retracted, entries_removed, citations: p.citations, reindex }))
}
