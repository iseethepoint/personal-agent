//! Инструменты целей — `hatch-agent/src/tools/goals_tool*` muse
//! (`source-files.txt:1594-1602`): два пространства одного модуля,
//! `user_goal.*` (цели владельца, source `user_goal`) и `tracking.*` (дела,
//! которые ведёт агент, source `assistant_tracking`); имена —
//! `strings-all.txt:69508-69530`. Описания, поля, ответы и ошибки — тексты
//! ядра (`prompts/goals/tools.json`, правки — `prompts/SOURCES.md`).
//! Описание пространства (`namespace`) прикреплено к `*.create`: поиска
//! инструментов у нас пока нет (как у `cron.add`).
//!
//! Не взяты (их служб у Hamlet нет): общие цели (`host_muse`,
//! `expected_revision`, задачи), `watch` / `requested` / `connection_id` /
//! `follow_up_at` у дел (коннекторы, П6), `escalation` (нужен будильник
//! внимания), подсказки и связи цели (`suggestions`, `associations` —
//! в ответе `get` пустые).

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use agent_loop::{personal, Tool, ToolError, ToolFuture};
use agent_model::{Provider, ToolResultPart, ToolSpec};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde_json::{json, Map, Value};

use crate::activity;
use crate::store::{rfc3339, GoalPatch, NewGoal, Store, CATEGORIES, GOAL_KINDS, TRACKING, USER_GOAL};
use crate::workspace;
use crate::OwnedCleanup;

const TEXTS_JSON: &str = include_str!("../../../prompts/goals/tools.json");
static TEXTS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(TEXTS_JSON).expect("prompts/goals/tools.json"));

fn lookup(path: &[&str]) -> String {
    let mut v = &*TEXTS;
    for k in path {
        v = &v[*k];
    }
    personal(v.as_str().unwrap_or_default())
}

fn msg(key: &str) -> String {
    lookup(&["texts", key])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ns {
    UserGoal,
    Tracking,
}

impl Ns {
    pub fn prefix(self) -> &'static str {
        match self {
            Ns::UserGoal => "user_goal",
            Ns::Tracking => "tracking",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Ns::UserGoal => USER_GOAL,
            Ns::Tracking => TRACKING,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Create,
    CreateEntry,
    Delete,
    DeleteBriefing,
    DeleteEntry,
    Get,
    List,
    Search,
    SetStatus,
    Update,
    UpdateEntry,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::Create => "create",
            Op::CreateEntry => "create_entry",
            Op::Delete => "delete",
            Op::DeleteBriefing => "delete_briefing",
            Op::DeleteEntry => "delete_entry",
            Op::Get => "get",
            Op::List => "list",
            Op::Search => "search",
            Op::SetStatus => "set_status",
            Op::Update => "update",
            Op::UpdateEntry => "update_entry",
        }
    }

    fn all(ns: Ns) -> Vec<Op> {
        let mut v = vec![Op::Create, Op::CreateEntry, Op::Delete, Op::DeleteEntry, Op::Get, Op::List, Op::Search, Op::SetStatus, Op::Update, Op::UpdateEntry];
        if ns == Ns::UserGoal {
            v.insert(3, Op::DeleteBriefing);
        }
        v
    }
}

/// Общее для всех инструментов целей.
pub struct Goals<P> {
    pub store: Store,
    pub home: PathBuf,
    /// Модель для решений после записи; без неё — поведение muse при сбое
    /// вывода (состояние не меняется, запись подцели копируется как есть).
    pub provider: Option<Arc<P>>,
    /// Расписания, принадлежащие цели, — удаляются вместе с ней.
    pub cleanup: Option<Arc<dyn OwnedCleanup>>,
}

pub struct GoalTool<P> {
    ns: Ns,
    op: Op,
    goals: Arc<Goals<P>>,
}

/// Все инструменты `user_goal.*` и `tracking.*`.
pub fn tools<P: Provider + Send + Sync + 'static>(goals: &Arc<Goals<P>>) -> Vec<GoalTool<P>> {
    [Ns::UserGoal, Ns::Tracking]
        .into_iter()
        .flat_map(|ns| Op::all(ns).into_iter().map(move |op| (ns, op)))
        .map(|(ns, op)| GoalTool { ns, op, goals: goals.clone() })
        .collect()
}

impl<P> GoalTool<P> {
    fn field(&self, op: &str, f: &str) -> String {
        // Описание поля есть не у каждого инструмента — берётся то же поле
        // соседнего инструмента пространства.
        let ns = self.ns.prefix();
        [format!("{op}.{f}"), f.to_string(), format!("create.{f}"), format!("get.{f}"), format!("create_entry.{f}")]
            .iter()
            .map(|k| lookup(&[ns, "fields", k]))
            .find(|t| !t.is_empty())
            .unwrap_or_default()
    }

    fn s(&self, op: &str, f: &str) -> Value {
        json!({"type": "string", "description": self.field(op, f)})
    }

    fn schema(&self) -> Value {
        let mut p = Map::new();
        let mut req: Vec<&str> = vec![];
        let op = self.op.name();
        let user = self.ns == Ns::UserGoal;
        let goal_fields = |p: &mut Map<String, Value>, op: &str| {
            p.insert("title".into(), self.s(op, "title"));
            p.insert("current_state".into(), self.s(op, "current_state"));
            p.insert("description".into(), self.s(op, "description"));
            p.insert("emoji".into(), self.s("create", "emoji"));
            p.insert("parent_goal_id".into(), self.s(op, "parent_goal_id"));
            if user {
                p.insert("category".into(), json!({"type": "string", "enum": CATEGORIES, "description": self.field(op, "category")}));
            }
            p.insert("goal_kind".into(), json!({"type": "string", "enum": GOAL_KINDS, "description": self.field("set_status", "goal_kind")}));
            p.insert("value_alignment".into(), self.s("create", "value_alignment"));
        };
        match self.op {
            Op::Create => {
                goal_fields(&mut p, op);
                req.push("title");
            }
            Op::Update => {
                p.insert("goal_id".into(), self.s(op, "goal_id"));
                goal_fields(&mut p, op);
                if user {
                    p.insert("pushable".into(), json!({"type": "boolean", "description": self.field(op, "pushable")}));
                }
                req.push("goal_id");
            }
            Op::Get | Op::Delete => {
                p.insert("goal_id".into(), self.s(op, "goal_id"));
                req.push("goal_id");
            }
            Op::List => {
                p.insert("offset".into(), json!({"type": "integer", "minimum": 0, "description": self.field(op, "offset")}));
                p.insert("limit".into(), json!({"type": "integer", "minimum": 1, "description": self.field(op, "limit")}));
            }
            Op::Search => {
                p.insert("query".into(), self.s(op, "query"));
                p.insert("limit".into(), json!({"type": "integer", "minimum": 1, "maximum": 100, "description": self.field(op, "limit")}));
                req.push("query");
            }
            Op::SetStatus => {
                p.insert("goal_id".into(), self.s(op, "goal_id"));
                p.insert("status".into(), json!({"type": "string", "enum": ["active", "completed"], "description": self.field(op, "status")}));
                p.insert("reason".into(), self.s(op, "reason"));
                req.extend(["goal_id", "status", "reason"]);
            }
            Op::CreateEntry | Op::UpdateEntry => {
                if self.op == Op::CreateEntry {
                    p.insert("goal_id".into(), self.s(op, "goal_id"));
                    req.push("goal_id");
                } else {
                    p.insert("update_id".into(), self.s("delete_entry", "update_id"));
                    req.push("update_id");
                }
                for f in ["title", "current_state", "description", "effective_at"] {
                    p.insert(f.into(), self.s("create_entry", f));
                }
            }
            Op::DeleteEntry => {
                p.insert("update_id".into(), self.s(op, "update_id"));
                req.push("update_id");
            }
            Op::DeleteBriefing => {
                p.insert("briefing_id".into(), self.s(op, "briefing_id"));
                req.push("briefing_id");
            }
        }
        json!({"type": "object", "properties": p, "required": req, "additionalProperties": false})
    }
}

impl<P: Provider + Send + Sync + 'static> Tool for GoalTool<P> {
    fn spec(&self) -> ToolSpec {
        let ns = self.ns.prefix();
        let mut description = lookup(&[ns, "tools", self.op.name(), "description"]);
        if self.op == Op::Create {
            description = format!("{}\n\n{}", description.trim_end(), lookup(&[ns, "namespace"]).trim_end());
        }
        ToolSpec { name: format!("{ns}.{}", self.op.name()), description, input_schema: self.schema() }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let out = self.dispatch(&args).await.map_err(ToolError::Failed)?;
            Ok(vec![ToolResultPart::Text { text: serde_json::to_string_pretty(&out).unwrap_or_default() }])
        })
    }
}

fn str_arg<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k).and_then(Value::as_str)
}

fn opt_text(args: &Value, k: &str) -> Option<String> {
    str_arg(args, k).map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

/// `effective_at`: RFC 3339 или `YYYY-MM-DD` (описание поля muse).
fn parse_effective(raw: &str) -> Result<i64, String> {
    if let Ok(t) = DateTime::parse_from_rfc3339(raw) {
        return Ok(t.timestamp_millis());
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map(|d| Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()).timestamp_millis())
        .map_err(|_| "effective_at must be an RFC 3339 timestamp or YYYY-MM-DD".into())
}

impl<P: Provider + Send + Sync + 'static> GoalTool<P> {
    fn store(&self) -> &Store {
        &self.goals.store
    }

    fn not_exists(&self, id: &str) -> String {
        msg("goal_not_exists").replace("{goal_id}", &format!("goal_id {id}"))
    }

    /// Цель этого пространства по id.
    async fn own(&self, id: &str) -> Result<crate::store::Goal, String> {
        if !id.starts_with("goal_") {
            return Err(msg("goal_id_prefix"));
        }
        let g = self.store().goal(id).await?.ok_or_else(|| self.not_exists(id))?;
        if g.source != self.ns.source() {
            return Err(match self.ns {
                Ns::Tracking => msg("tracking_can_only_change"),
                Ns::UserGoal => self.not_exists(id),
            });
        }
        Ok(g)
    }

    /// Проверка родителя: существует, верхнего уровня, того же вида, не
    /// сама цель; у цели с подцелями родителя быть не может.
    async fn check_parent(&self, parent: &str, child: Option<&str>) -> Result<(), String> {
        if child == Some(parent) {
            return Err(msg("own_parent"));
        }
        let p = self.store().goal(parent).await?.ok_or_else(|| self.not_exists(parent))?;
        if p.source != self.ns.source() {
            return Err(match self.ns {
                Ns::Tracking if p.source == USER_GOAL => msg("user_goal_exists_for_tracking").replace("{goal}", &p.title),
                Ns::Tracking => msg("tracking_parent"),
                Ns::UserGoal => msg("source_mismatch"),
            });
        }
        if p.parent_goal_id.is_some() {
            // У muse — вариант ошибки `GoalParentMustBeTopLevel` без текста
            // в разборе; текст свой.
            return Err("the parent goal must be a top-level goal; goals support only one layer of child goals".into());
        }
        if let Some(c) = child {
            if !self.store().children(c).await?.is_empty() {
                return Err(msg("parent_has_children").replace("{goal}", c));
            }
        }
        Ok(())
    }

    fn category(&self, args: &Value) -> Result<Option<Option<String>>, String> {
        let Some(c) = str_arg(args, "category") else { return Ok(None) };
        if self.ns == Ns::Tracking {
            return Err(msg("category_user_only"));
        }
        // Вне перечня — «coerced to uncategorized at storage» (`strings-all.txt:227341`).
        Ok(Some(CATEGORIES.contains(&c).then(|| c.to_string())))
    }

    fn goal_kind(args: &Value) -> Result<Option<String>, String> {
        match str_arg(args, "goal_kind") {
            None => Ok(None),
            Some(k) if GOAL_KINDS.contains(&k) => Ok(Some(k.into())),
            // Текста ошибки в разборе нет — свой.
            Some(_) => Err(format!("goal_kind must be one of {}", GOAL_KINDS.join("|"))),
        }
    }

    async fn dispatch(&self, args: &Value) -> Result<Value, String> {
        match self.op {
            Op::Create => self.create(args).await,
            Op::Update => self.update(args).await,
            Op::Get => self.get(args).await,
            Op::List => self.list(args).await,
            Op::Search => self.search(args).await,
            Op::SetStatus => self.set_status(args).await,
            Op::Delete => self.delete(args).await,
            Op::CreateEntry => self.create_entry(args).await,
            Op::UpdateEntry => self.update_entry(args).await,
            Op::DeleteEntry => self.delete_entry(args).await,
            Op::DeleteBriefing => self.delete_briefing(args).await,
        }
    }

    async fn create(&self, args: &Value) -> Result<Value, String> {
        let title = opt_text(args, "title").ok_or("title is required")?;
        let parent = opt_text(args, "parent_goal_id");
        if let Some(p) = &parent {
            self.check_parent(p, None).await?;
        }
        let g = self
            .store()
            .create(&NewGoal {
                title,
                current_state: opt_text(args, "current_state"),
                description: opt_text(args, "description"),
                emoji: opt_text(args, "emoji"),
                parent_goal_id: parent,
                category: self.category(args)?.flatten(),
                goal_kind: Self::goal_kind(args)?,
                value_alignment: opt_text(args, "value_alignment"),
                source: self.ns.source().into(),
            })
            .await?;
        workspace::ensure(&self.goals.home, &g).await?;
        let mut v = json!(g);
        v["workspace"] = format!("workspace/goals/{}/", g.slug).into();
        Ok(v)
    }

    async fn update(&self, args: &Value) -> Result<Value, String> {
        if args.get("status").is_some() {
            return Err(msg("status_not_editable"));
        }
        let id = str_arg(args, "goal_id").unwrap_or_default();
        self.own(id).await?;
        let mut p = GoalPatch { title: opt_text(args, "title"), ..Default::default() };
        let text = |k: &str| str_arg(args, k).map(|s| Some(s.trim().to_string()).filter(|s| !s.is_empty()));
        p.current_state = text("current_state");
        p.description = text("description");
        p.emoji = text("emoji");
        p.value_alignment = text("value_alignment");
        p.category = self.category(args)?;
        p.goal_kind = Self::goal_kind(args)?.map(Some);
        if let Some(parent) = str_arg(args, "parent_goal_id") {
            if parent.is_empty() {
                p.parent_goal_id = Some(None);
            } else {
                self.check_parent(parent, Some(id)).await?;
                p.parent_goal_id = Some(Some(parent.into()));
            }
        }
        if self.ns == Ns::UserGoal {
            p.pushable = args.get("pushable").and_then(Value::as_bool);
        }
        let g = self.store().update(id, &p).await?;
        Ok(json!(g))
    }

    async fn get(&self, args: &Value) -> Result<Value, String> {
        let g = self.own(str_arg(args, "goal_id").unwrap_or_default()).await?;
        let mut v = json!(g);
        v["updates"] = json!(self.store().entries(&g.goal_id).await?);
        v["briefings"] = json!(self.store().briefings(&g.goal_id).await?);
        v["children"] = json!(self.store().children(&g.goal_id).await?.iter().map(|c| json!({"goal_id": c.goal_id, "title": c.title, "status": c.status})).collect::<Vec<_>>());
        v["suggestions"] = json!([]);
        v["associations"] = json!([]);
        v["workspace"] = format!("workspace/goals/{}/", g.slug).into();
        Ok(v)
    }

    async fn list(&self, args: &Value) -> Result<Value, String> {
        let offset = args.get("offset").and_then(Value::as_i64).unwrap_or(0).max(0);
        let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(100).max(1);
        let (goals, total) = self.store().list(self.ns.source(), offset, limit).await?;
        let next = offset + goals.len() as i64;
        let mut v = json!({self.ns.prefix().to_string() + "s": goals, "returned_count": goals.len(), "total": total});
        if next < total {
            v["next_offset"] = next.into();
            if self.ns == Ns::Tracking {
                v["note"] = msg("more_items").replace("{offset}", &next.to_string()).into();
            }
        }
        Ok(v)
    }

    async fn search(&self, args: &Value) -> Result<Value, String> {
        let query = str_arg(args, "query").unwrap_or_default().to_lowercase();
        let terms: Vec<&str> = query.split_whitespace().filter(|t| t.chars().count() >= 2).collect();
        if terms.is_empty() {
            return Err(msg("query_too_short").replace("{ns}", self.ns.prefix()));
        }
        let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(10).clamp(1, 100) as usize;
        let mut hits: Vec<(usize, crate::store::Goal)> = self
            .store()
            .all(self.ns.source())
            .await?
            .into_iter()
            .filter_map(|g| {
                let hay = format!("{} {} {} {}", g.goal_id, g.title, g.current_state.as_deref().unwrap_or(""), g.description.as_deref().unwrap_or("")).to_lowercase();
                let n = terms.iter().filter(|t| hay.contains(**t)).count();
                (n > 0).then_some((n, g))
            })
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.updated_at.cmp(&a.1.updated_at)));
        let match_count = hits.len();
        let results: Vec<Value> = hits
            .into_iter()
            .take(limit)
            .map(|(_, g)| json!({"goal_id": g.goal_id, "title": g.title, "current_state": g.current_state, "status": g.status, "category": g.category, "created_at": g.created_at, "updated_at": g.updated_at}))
            .collect();
        Ok(json!({"results": results, "match_count": match_count, "returned_count": results.len()}))
    }

    async fn set_status(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "goal_id").unwrap_or_default();
        let g = self.own(id).await?;
        let status = str_arg(args, "status").unwrap_or_default();
        if status != "active" && status != "completed" {
            return Err(msg("status_values"));
        }
        let reason = opt_text(args, "reason").ok_or("reason is required")?;
        let changed = self.store().set_status(id, status).await?;
        // Причина ложится последней записью на ленту («That status transition
        // automatically records the reason as the final timeline entry»,
        // `goal_record_generic.md`). Заголовок записи — свой.
        let entry = if changed {
            let title = if status == "completed" { "Completed" } else { "Reopened" };
            Some(self.store().add_entry(id, Some(title), None, Some(&reason), None, "assistant").await?)
        } else {
            None
        };
        Ok(json!({"goal_id": g.goal_id, "status": status, "changed": changed, "entry": entry}))
    }

    async fn delete(&self, args: &Value) -> Result<Value, String> {
        let g = self.own(str_arg(args, "goal_id").unwrap_or_default()).await?;
        let gone = self.store().delete(&g.goal_id).await?;
        let mut kept = Vec::new();
        let mut deleted = Vec::new();
        let mut schedules = Vec::new();
        for x in &gone {
            if let Some(c) = &self.goals.cleanup {
                schedules.extend(c.remove_owned(&format!("goal:{}", x.slug)).await?);
            }
            let (k, d) = workspace::remove(&self.goals.home, &x.slug).await?;
            kept.extend(k);
            deleted.extend(d);
        }
        Ok(json!({
            "deleted_goal_ids": gone.iter().map(|x| x.goal_id.clone()).collect::<Vec<_>>(),
            "deleted_schedules": schedules,
            "preserved_files_paths": kept,
            "deleted_paths": deleted,
        }))
    }

    async fn create_entry(&self, args: &Value) -> Result<Value, String> {
        let g = self.own(str_arg(args, "goal_id").unwrap_or_default()).await?;
        let eff = str_arg(args, "effective_at").map(parse_effective).transpose()?;
        let (title, state, desc) = (opt_text(args, "title"), opt_text(args, "current_state"), opt_text(args, "description"));
        if title.is_none() && state.is_none() && desc.is_none() {
            return Err(msg("edit_needs_field"));
        }
        let author = if self.ns == Ns::Tracking { "assistant" } else { "user" };
        let e = self.store().add_entry(&g.goal_id, title.as_deref(), state.as_deref(), desc.as_deref(), eff, author).await?;
        let mut out = json!({"goal_id": g.goal_id, "entry": e});
        let provider = self.goals.provider.as_deref();
        let recent = self.store().entries(&g.goal_id).await?;
        let observed = rfc3339(self.store().now_ms());
        let mut updated = g.clone();
        let decided = match provider {
            Some(p) => activity::pulse(p, &g, &e, &recent[1.min(recent.len())..], &observed).await,
            None => None,
        };
        if let Some(p) = decided {
            let due = p.next_due_at.as_deref().and_then(|d| parse_effective(d).ok());
            updated = self
                .store()
                .update(&g.goal_id, &GoalPatch { current_state: Some(Some(p.current_state)), attention_kind: Some(p.attention), next_due_at: Some(due), ..Default::default() })
                .await?;
        }
        out["current_state"] = json!(updated.current_state);
        out["attention_kind"] = json!(updated.attention_kind);
        // Запись подцели переносится на родителя («An entry on a nested goal
        // is also rolled up onto its parent goal's timeline automatically»).
        if let Some(pid) = &g.parent_goal_id {
            if let Some(parent) = self.store().goal(pid).await? {
                let parent_recent = self.store().entries(pid).await?;
                let r = match provider {
                    Some(p) => activity::rollup(p, &parent, &updated, &e, &parent_recent).await,
                    None => activity::copy(&e),
                };
                let pe = self.store().add_entry(pid, Some(&r.title), Some(&r.current_state), Some(&r.description), eff, "subgoal_rollup").await?;
                if let Some(s) = r.parent_current_state {
                    self.store().update(pid, &GoalPatch { current_state: Some(Some(s)), ..Default::default() }).await?;
                }
                out["parent_entry"] = json!(pe);
            }
        }
        Ok(out)
    }

    async fn update_entry(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "update_id").unwrap_or_default();
        let e = self.store().entry(id).await?.ok_or_else(|| format!("{}{id}", msg("update_not_found")))?;
        self.own(&e.goal_id).await?;
        let (title, state, desc) = (opt_text(args, "title"), opt_text(args, "current_state"), opt_text(args, "description"));
        let eff = str_arg(args, "effective_at").map(parse_effective).transpose()?;
        if title.is_none() && state.is_none() && desc.is_none() && eff.is_none() {
            return Err(msg("edit_needs_field"));
        }
        let e = self.store().edit_entry(id, title.as_deref(), state.as_deref(), desc.as_deref(), eff).await?;
        Ok(json!(e))
    }

    async fn delete_entry(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "update_id").unwrap_or_default();
        let e = self.store().entry(id).await?.ok_or_else(|| format!("{}{id}", msg("update_not_found")))?;
        self.own(&e.goal_id).await?;
        self.store().delete_entry(id).await?;
        Ok(json!({"update_id": e.update_id, "goal_id": e.goal_id, "title": e.title}))
    }

    async fn delete_briefing(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "briefing_id").unwrap_or_default();
        let b = self.store().delete_briefing(id).await?.ok_or_else(|| format!("{}{id}", msg("briefing_not_found")))?;
        let file = self.goals.home.join(b.path.trim_start_matches("~/"));
        let _ = tokio::fs::remove_file(file).await;
        Ok(json!({"briefing_id": b.briefing_id, "goal_id": b.goal_id, "deleted": true}))
    }
}
