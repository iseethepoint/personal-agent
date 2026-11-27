//! Забывание — часть среды. Форма muse: агент, выполнив план забывания,
//! пишет `~/workspace/memory/forget/pending.json` как
//! `{"claims":[…],"citations":[…]}` — id утверждений и места `MEMORY.md#L<n>`
//! или `memory/<date>.md#L<n>`, без самого текста (`strings-all.txt:55423`,
//! навык `forget`). Среда отзывает эти утверждения «и всё, что из них
//! выросло», затем обновляет поиск по памяти (`hatch-server/src/daemon/runtime/
//! forget_reindex.rs`, `strings-all.txt:39400`).
//!
//! Инструменты `forget.plan` и `forget.confirm` — форма muse
//! (`strings-all.txt:78661`, задания подагентам — `:55418-55423`): оба
//! запускают обычного подагента, наследующего разговор, и не берут
//! аргументов — предмет забывания он находит в разговоре сам. План только
//! читает; исполнитель делает уборку, пишет `pending.json`, а среда по его
//! концу отзывает утверждения и обновляет поиск, и лишь потом итог уходит
//! родителю («Muse finishes the claim retraction and the refresh before it
//! relays the result», навык `forget`).
//!
//! Правки текстов (П10): «Muse» → «Hamlet»; путь навыка
//! `/opt/muse/skills/forget/SKILL.md` → путь навыка `forget` из настройки
//! агента; «never query Hatch-owned databases» → «never query the agent's
//! own databases»; из задания исполнителю убрано предложение о профилях
//! покупок и путешествий (`~/memory/shopping/…`, `~/memory/travel/…`) —
//! этих служб у Hamlet нет. Описание каждого инструмента — две строки ядра
//! через пустую строку.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent_loop::{AfterDone, Status, Subagents, Tool, ToolError, ToolFuture, Tools};
use agent_model::{Provider, ToolResultPart, ToolSpec};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::files::{reindex, Reindex};
use crate::store::Store;
use crate::Embedder;

const PLAN_DESCRIPTION: &str = "Start an ordinary untyped read-only planning subagent to find active copies, derivatives, and future producers of information the user explicitly asked Hamlet to forget. The current conversation supplies the subject, so this tool takes no sensitive text. Wait for its plan, show the user the scope and limits, and ask for confirmation before using forget.confirm.\n\nUse only for an explicit request to remove information from Hamlet's memory or stop Hamlet from remembering it. Do not use when 'forget it' merely means cancel the current task.";
const CONFIRM_DESCRIPTION: &str = "After the user approves the most recent forget plan, start a fresh ordinary untyped subagent to re-read that plan and confirmation from the inherited conversation, revalidate it, execute it, and verify the result. This tool takes no plan or agent id. Do not call it for silence, ambiguity, partial approval, or a changed scope.\n\nUse only after a later user message clearly approves the most recent forget plan. A changed or ambiguous scope needs a new plan.";
const CONFIRM_REFUSAL: &str = "forget.confirm requires a completed forget plan that was already shown to the user";

fn plan_task(skill: &Path) -> String {
    format!(
        "Read `{}` and its linked `references/artifact-inventory.md` completely. Derive the subject from the inherited conversation; do not ask the parent to repeat it or copy it into a memory, todo, filename, or report.\nThis turn is planning only. Stay read-only while tracing authoritative copies, derived projections, scheduled or active producers, model-context copies, publications, and retention limits. Prefer product tools and typed APIs; never query the agent's own databases with ad hoc SQL.\nReturn a concise cleanup plan: what you found, what should change, how you would verify it, and what Hamlet cannot erase. Keep the summary discreet and do not mutate anything.",
        skill.display()
    )
}

fn confirm_task(skill: &Path) -> String {
    format!(
        "Review the inherited parent conversation for the most recent forget plan and a later clear user approval of that plan. Do not assume the original forget request was approval.\nRead `{}` and its artifact inventory. If the plan or later approval is missing or ambiguous, or if the user changed the scope, do not mutate anything; report the blocker so the parent can make a new plan.\nOtherwise revalidate the plan against current state, stop approved producers before removing their outputs, use owning product APIs, preserve unrelated content, refresh affected derived surfaces, and verify from fresh state. Perform the cleanup directly rather than delegating it to another subagent; this executor's terminal boundary owns the memory-search refresh. Do not broaden the plan, contact outside parties, create new schedules or artifacts, write the forgotten information to a todo, memory, log, or report, or describe partial cleanup as full forgetting.\nBefore you finish, stage the memory claims you removed so the runtime can retract them and everything derived from them: write `~/workspace/memory/forget/pending.json` as `{{\"claims\":[\"<claim id>\", ...],\"citations\":[\"<path>#L<line>\", ...]}}`. Claim ids come from `memory_explain` and from `memory_search` result metadata; citations are the exact `MEMORY.md#L<line>` or `memory/<date>.md#L<line>` locators of the lines you removed or rewrote. List ids and locators only, never the forgotten text. Write the file even when both lists are empty.",
        skill.display()
    )
}

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

/// Итог уборки для родителя. Своя форма: у muse текста заметки в ядре не видно.
fn refresh_note(r: Result<Option<Forgotten>, String>) -> String {
    match r {
        Ok(Some(f)) => format!(
            "[Memory refresh] Retracted {} claims and removed {} memory search entries; memory search was refreshed.",
            f.retracted.len(),
            f.entries_removed.len()
        ),
        Ok(None) => format!("[Memory refresh] `~/{PENDING}` was not written, so no claims were retracted. Report the cleanup as incomplete."),
        Err(e) => format!("[Memory refresh failed] {e}. Report the cleanup as incomplete."),
    }
}

/// `forget.plan` и `forget.confirm` поверх подагентов родителя.
pub struct ForgetTools<P> {
    subagents: Subagents<P>,
    home: PathBuf,
    store: Store,
    embedder: Option<Arc<dyn Embedder>>,
    /// `SKILL.md` навыка `forget`.
    skill: PathBuf,
    /// Подагент последнего плана.
    plan: Arc<Mutex<Option<String>>>,
}

impl<P> Clone for ForgetTools<P> {
    fn clone(&self) -> Self {
        ForgetTools {
            subagents: self.subagents.clone(),
            home: self.home.clone(),
            store: self.store.clone(),
            embedder: self.embedder.clone(),
            skill: self.skill.clone(),
            plan: self.plan.clone(),
        }
    }
}

impl<P: Provider + Send + Sync + 'static> ForgetTools<P> {
    pub fn new(
        subagents: Subagents<P>,
        home: impl Into<PathBuf>,
        store: Store,
        embedder: Option<Arc<dyn Embedder>>,
        skill: impl Into<PathBuf>,
    ) -> Self {
        ForgetTools { subagents, home: home.into(), store, embedder, skill: skill.into(), plan: Arc::new(Mutex::new(None)) }
    }

    pub fn add_to(&self, tools: &mut Tools) {
        tools.add(ForgetTool { confirm: false, f: self.clone() });
        tools.add(ForgetTool { confirm: true, f: self.clone() });
    }
}

struct ForgetTool<P> {
    confirm: bool,
    f: ForgetTools<P>,
}

impl<P: Provider + Send + Sync + 'static> Tool for ForgetTool<P> {
    fn spec(&self) -> ToolSpec {
        let (name, description) = if self.confirm { ("forget.confirm", CONFIRM_DESCRIPTION) } else { ("forget.plan", PLAN_DESCRIPTION) };
        ToolSpec { name: name.into(), description: description.into(), input_schema: json!({"type": "object", "properties": {}}) }
    }

    fn call(&self, _args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let f = &self.f;
            let id = if !self.confirm {
                let id = f.subagents.spawn(&plan_task(&f.skill));
                *f.plan.lock().unwrap() = Some(id.clone());
                id
            } else {
                let planned = f.plan.lock().unwrap().clone();
                if planned.and_then(|p| f.subagents.status(&p)) != Some(Status::Done) {
                    return Err(ToolError::Failed(CONFIRM_REFUSAL.into()));
                }
                let (home, store, emb) = (f.home.clone(), f.store.clone(), f.embedder.clone());
                let after: AfterDone = Arc::new(move |_status, text| {
                    let (home, store, emb) = (home.clone(), store.clone(), emb.clone());
                    Box::pin(async move {
                        let note = refresh_note(apply_pending(&home, &store, emb.as_deref()).await);
                        format!("{text}\n\n{note}")
                    })
                });
                f.subagents.spawn_then(&confirm_task(&f.skill), Some(after))
            };
            Ok(vec![ToolResultPart::Text { text: json!({"agent_id": id, "status": "running"}).to_string() }])
        })
    }
}
