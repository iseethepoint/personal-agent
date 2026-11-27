//! Хранилище целей в Postgres — форма muse `hatch-goals/src/store/*`
//! (`source-files.txt:5205-5227`), таблицы `goals.goals`, `goals.updates`,
//! `goals.briefings` по схеме базы muse.
//!
//! `current_state` инструментов хранится в колонке `summary` (у записи —
//! тоже `summary`): в ядре рядом стоят `title`, `summary` как изменяемые
//! поля записи (`strings-all.txt:54232`). Вывод.
//! Формат id цели — `goal_<12 lowercase hex chars>`, брифинга —
//! `goal_briefing_<12 …>` (`strings-all.txt:227341`); id записи в разборе не
//! найден — своё: `goal_update_<12 …>`.

use std::sync::Arc;

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;

const MIGRATION: &str = include_str!("../migrations/0001_goals.sql");

pub type Result<T> = std::result::Result<T, String>;

fn db(e: sqlx::Error) -> String {
    format!("goals database: {e}")
}

pub type Now = Arc<dyn Fn() -> i64 + Send + Sync>;

pub const USER_GOAL: &str = "user_goal";
pub const TRACKING: &str = "assistant_tracking";

/// Жизненные области — `strings-all.txt:227341`.
pub const CATEGORIES: [&str; 7] = ["health", "relationships", "money", "career", "interests", "productivity", "something_else"];
/// Виды цели — описание `set_status.goal_kind` (`strings-all.txt:58198`).
pub const GOAL_KINDS: [&str; 8] = ["outcome", "behavior", "learning", "habit", "maintenance", "recovery", "identity", "exploratory"];
pub const ATTENTION: [&str; 3] = ["user_action_required", "new_information", "does_not_need_attention"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Goal {
    pub goal_id: String,
    pub slug: String,
    pub title: String,
    pub current_state: Option<String>,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub status: String,
    pub source: String,
    pub parent_goal_id: Option<String>,
    pub category: Option<String>,
    pub goal_kind: Option<String>,
    pub value_alignment: Option<String>,
    pub pushable: bool,
    pub attention_kind: String,
    pub next_due_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub last_activity_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub update_id: String,
    pub goal_id: String,
    pub title: Option<String>,
    pub current_state: Option<String>,
    pub description: Option<String>,
    pub effective_at: Option<String>,
    pub created_at: String,
    pub author_source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Briefing {
    pub briefing_id: String,
    pub goal_id: String,
    pub path: String,
    pub created_at: String,
}

/// Новая цель.
#[derive(Debug, Clone, Default)]
pub struct NewGoal {
    pub title: String,
    pub current_state: Option<String>,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub parent_goal_id: Option<String>,
    pub category: Option<String>,
    pub goal_kind: Option<String>,
    pub value_alignment: Option<String>,
    pub source: String,
}

/// Изменение полей цели: `None` — не трогать; `Some(None)` — очистить.
#[derive(Debug, Clone, Default)]
pub struct GoalPatch {
    pub title: Option<String>,
    pub current_state: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub emoji: Option<Option<String>>,
    pub parent_goal_id: Option<Option<String>>,
    pub category: Option<Option<String>>,
    pub goal_kind: Option<Option<String>>,
    pub value_alignment: Option<Option<String>>,
    pub pushable: Option<bool>,
    pub attention_kind: Option<String>,
    pub next_due_at: Option<Option<i64>>,
}

pub fn rfc3339(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn ts(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}

pub fn new_id(prefix: &str) -> String {
    use rand::Rng;
    let mut r = rand::thread_rng();
    let hex: String = (0..12).map(|_| format!("{:x}", r.gen_range(0..16u8))).collect();
    format!("{prefix}{hex}")
}

/// Slug из названия: латиница и цифры, остальное — дефис. Правило muse
/// (`normalization.rs`) в разборе не видно — своё.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(60).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "goal".into()
    } else {
        out
    }
}

const GOAL_COLUMNS: &str = "goal_id, slug, title, summary, description, emoji, status, source, parent_goal_id, category, goal_kind, value_alignment, pushable, attention_kind, next_due_at, created_at, updated_at, completed_at, last_activity_at";

fn goal(r: &PgRow) -> Goal {
    Goal {
        goal_id: r.get("goal_id"),
        slug: r.get::<Option<String>, _>("slug").unwrap_or_default(),
        title: r.get("title"),
        current_state: r.get("summary"),
        description: r.get("description"),
        emoji: r.get("emoji"),
        status: r.get("status"),
        source: r.get("source"),
        parent_goal_id: r.get("parent_goal_id"),
        category: r.get("category"),
        goal_kind: r.get("goal_kind"),
        value_alignment: r.get("value_alignment"),
        pushable: r.get("pushable"),
        attention_kind: r.get("attention_kind"),
        next_due_at: r
            .get::<Option<DateTime<Utc>>, _>("next_due_at")
            .map(|t| t.to_rfc3339_opts(SecondsFormat::Millis, true)),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        completed_at: r.get("completed_at"),
        last_activity_at: r.get("last_activity_at"),
    }
}

const ENTRY_COLUMNS: &str = "update_id, goal_id, title, summary, description, effective_at, created_at, author_source";

fn entry(r: &PgRow) -> Entry {
    Entry {
        update_id: r.get("update_id"),
        goal_id: r.get("goal_id"),
        title: r.get("title"),
        current_state: r.get("summary"),
        description: r.get("description"),
        effective_at: r.get("effective_at"),
        created_at: r.get("created_at"),
        author_source: r.get("author_source"),
    }
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
    now: Now,
}

impl Store {
    pub async fn connect(url: &str, now: Now) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(4).connect(url).await.map_err(db)?;
        sqlx::raw_sql(MIGRATION).execute(&pool).await.map_err(db)?;
        Ok(Store { pool, now })
    }

    pub fn now_ms(&self) -> i64 {
        (self.now)()
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn goal(&self, id: &str) -> Result<Option<Goal>> {
        let r = sqlx::query(&format!("select {GOAL_COLUMNS} from goals.goals where goal_id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(r.as_ref().map(goal))
    }

    pub async fn goal_by_slug(&self, slug: &str) -> Result<Option<Goal>> {
        let r = sqlx::query(&format!("select {GOAL_COLUMNS} from goals.goals where slug = $1"))
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(r.as_ref().map(goal))
    }

    pub async fn children(&self, id: &str) -> Result<Vec<Goal>> {
        let rows = sqlx::query(&format!("select {GOAL_COLUMNS} from goals.goals where parent_goal_id = $1 order by sort_order asc, goal_id asc"))
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(goal).collect())
    }

    /// Страница целей одного вида. Порядок — запросы muse: у целей
    /// владельца `sort_order asc, goal_id asc`; у дел — по вниманию, сроку и
    /// свежести (`strings-all.txt:249386`).
    pub async fn list(&self, source: &str, offset: i64, limit: i64) -> Result<(Vec<Goal>, i64)> {
        let order = if source == TRACKING {
            "case attention_kind when 'user_action_required' then 0 when 'action_required' then 0 when 'new_information' then 1 else 2 end asc, case when next_due_at is not null and next_due_at <= now() + interval '30 days' then next_due_at else timestamptz '9999-12-31' end asc, coalesce(last_activity_at_ts, updated_at_ts, updated_at::timestamptz) desc, sort_order asc, goal_id asc"
        } else {
            "sort_order asc, goal_id asc"
        };
        let rows = sqlx::query(&format!("select {GOAL_COLUMNS} from goals.goals where source = $1 order by {order} offset $2 limit $3"))
            .bind(source)
            .bind(offset)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        let total: i64 = sqlx::query_scalar("select count(*)::bigint from goals.goals where source = $1")
            .bind(source)
            .fetch_one(&self.pool)
            .await
            .map_err(db)?;
        Ok((rows.iter().map(goal).collect(), total))
    }

    pub async fn all(&self, source: &str) -> Result<Vec<Goal>> {
        let rows = sqlx::query(&format!("select {GOAL_COLUMNS} from goals.goals where source = $1"))
            .bind(source)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(goal).collect())
    }

    async fn free_slug(&self, base: &str) -> Result<String> {
        let mut slug = base.to_string();
        let mut n = 2;
        while self.goal_by_slug(&slug).await?.is_some() {
            slug = format!("{base}-{n}");
            n += 1;
        }
        Ok(slug)
    }

    pub async fn create(&self, g: &NewGoal) -> Result<Goal> {
        let now = self.now_ms();
        let id = new_id("goal_");
        let slug = self.free_slug(&slugify(&g.title)).await?;
        let sort: f64 = sqlx::query_scalar("select coalesce(max(sort_order), 0) + 1 from goals.goals where source = $1")
            .bind(&g.source)
            .fetch_one(&self.pool)
            .await
            .map_err(db)?;
        sqlx::query(
            "insert into goals.goals (goal_id, slug, title, summary, description, emoji, status, created_at, updated_at,
                 parent_goal_id, sort_order, created_at_ts, updated_at_ts, category, goal_kind, value_alignment, source)
             values ($1, $2, $3, $4, $5, $6, 'active', $7, $7, $8, $9, $10, $10, $11, $12, $13, $14)",
        )
        .bind(&id)
        .bind(&slug)
        .bind(&g.title)
        .bind(&g.current_state)
        .bind(&g.description)
        .bind(&g.emoji)
        .bind(rfc3339(now))
        .bind(&g.parent_goal_id)
        .bind(sort)
        .bind(ts(now))
        .bind(&g.category)
        .bind(&g.goal_kind)
        .bind(&g.value_alignment)
        .bind(&g.source)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(self.goal(&id).await?.expect("just inserted"))
    }

    pub async fn update(&self, id: &str, p: &GoalPatch) -> Result<Goal> {
        let now = self.now_ms();
        let cur = self.goal(id).await?.ok_or_else(|| format!("goal not found: {id}"))?;
        let pick = |new: &Option<Option<String>>, old: &Option<String>| new.clone().unwrap_or_else(|| old.clone());
        let next_due: Option<DateTime<Utc>> = match p.next_due_at {
            Some(v) => v.map(ts),
            None => cur.next_due_at.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)),
        };
        let attention_changed = p.attention_kind.as_ref().is_some_and(|a| *a != cur.attention_kind);
        sqlx::query(
            "update goals.goals set title = $2, summary = $3, description = $4, emoji = $5, parent_goal_id = $6,
                 category = $7, goal_kind = $8, value_alignment = $9, pushable = $10, attention_kind = $11,
                 attention_updated_at = case when $12 then $14 else attention_updated_at end,
                 next_due_at = $13, updated_at = $15, updated_at_ts = $14
             where goal_id = $1",
        )
        .bind(id)
        .bind(p.title.clone().unwrap_or(cur.title.clone()))
        .bind(pick(&p.current_state, &cur.current_state))
        .bind(pick(&p.description, &cur.description))
        .bind(pick(&p.emoji, &cur.emoji))
        .bind(pick(&p.parent_goal_id, &cur.parent_goal_id))
        .bind(pick(&p.category, &cur.category))
        .bind(pick(&p.goal_kind, &cur.goal_kind))
        .bind(pick(&p.value_alignment, &cur.value_alignment))
        .bind(p.pushable.unwrap_or(cur.pushable))
        .bind(p.attention_kind.clone().unwrap_or(cur.attention_kind.clone()))
        .bind(attention_changed)
        .bind(next_due)
        .bind(ts(now))
        .bind(rfc3339(now))
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(self.goal(id).await?.expect("exists"))
    }

    /// Сменить жизненный цикл. `false` — менять было нечего.
    pub async fn set_status(&self, id: &str, status: &str) -> Result<bool> {
        let now = self.now_ms();
        let n = sqlx::query(
            "update goals.goals set status = $2,
                 completed_at = case when $2 = 'completed' then $3 else null end,
                 completed_at_ts = case when $2 = 'completed' then $4 else null end,
                 updated_at = $3, updated_at_ts = $4
             where goal_id = $1 and status <> $2",
        )
        .bind(id)
        .bind(status)
        .bind(rfc3339(now))
        .bind(ts(now))
        .execute(&self.pool)
        .await
        .map_err(db)?
        .rows_affected();
        Ok(n == 1)
    }

    /// Удалить цель с подцелями, записями и брифингами. Возвращает
    /// удалённые цели (для папок и расписаний).
    pub async fn delete(&self, id: &str) -> Result<Vec<Goal>> {
        let mut gone = self.children(id).await?;
        if let Some(g) = self.goal(id).await? {
            gone.push(g);
        }
        let ids: Vec<String> = gone.iter().map(|g| g.goal_id.clone()).collect();
        let mut tx = self.pool.begin().await.map_err(db)?;
        for t in ["goals.updates", "goals.briefings"] {
            sqlx::query(&format!("delete from {t} where goal_id = any($1)")).bind(&ids).execute(&mut *tx).await.map_err(db)?;
        }
        sqlx::query("delete from goals.goals where parent_goal_id = any($1)").bind(&ids).execute(&mut *tx).await.map_err(db)?;
        sqlx::query("delete from goals.goals where goal_id = any($1)").bind(&ids).execute(&mut *tx).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(gone)
    }

    // ---- записи ----

    pub async fn add_entry(&self, goal_id: &str, title: Option<&str>, state: Option<&str>, description: Option<&str>, effective_ms: Option<i64>, author: &str) -> Result<Entry> {
        let now = self.now_ms();
        let id = new_id("goal_update_");
        let eff = effective_ms.unwrap_or(now);
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query(
            "insert into goals.updates (update_id, goal_id, title, summary, description, effective_at, created_at, updated_at, effective_at_ts, updated_at_ts, author_source)
             values ($1, $2, $3, $4, $5, $6, $7, $7, $8, $9, $10)",
        )
        .bind(&id)
        .bind(goal_id)
        .bind(title)
        .bind(state)
        .bind(description)
        .bind(rfc3339(eff))
        .bind(rfc3339(now))
        .bind(ts(eff))
        .bind(ts(now))
        .bind(author)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query("update goals.goals set last_activity_at = $2, last_activity_at_ts = $3 where goal_id = $1")
            .bind(goal_id)
            .bind(rfc3339(now))
            .bind(ts(now))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)?;
        self.entry(&id).await?.ok_or_else(|| "goal update vanished".into())
    }

    pub async fn entry(&self, id: &str) -> Result<Option<Entry>> {
        let r = sqlx::query(&format!("select {ENTRY_COLUMNS} from goals.updates where update_id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(r.as_ref().map(entry))
    }

    pub async fn entries(&self, goal_id: &str) -> Result<Vec<Entry>> {
        let rows = sqlx::query(&format!(
            "select {ENTRY_COLUMNS} from goals.updates where goal_id = $1 order by effective_at_ts desc nulls last, created_at desc, update_id desc"
        ))
        .bind(goal_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        Ok(rows.iter().map(entry).collect())
    }

    pub async fn edit_entry(&self, id: &str, title: Option<&str>, state: Option<&str>, description: Option<&str>, effective_ms: Option<i64>) -> Result<Entry> {
        let now = self.now_ms();
        sqlx::query(
            "update goals.updates set title = coalesce($2, title), summary = coalesce($3, summary), description = coalesce($4, description),
                 effective_at = coalesce($5, effective_at), effective_at_ts = coalesce($6, effective_at_ts),
                 updated_at = $7, updated_at_ts = $8
             where update_id = $1",
        )
        .bind(id)
        .bind(title)
        .bind(state)
        .bind(description)
        .bind(effective_ms.map(rfc3339))
        .bind(effective_ms.map(ts))
        .bind(rfc3339(now))
        .bind(ts(now))
        .execute(&self.pool)
        .await
        .map_err(db)?;
        self.entry(id).await?.ok_or_else(|| format!("goal update not found: {id}"))
    }

    pub async fn delete_entry(&self, id: &str) -> Result<Option<Entry>> {
        let e = self.entry(id).await?;
        sqlx::query("delete from goals.updates where update_id = $1").bind(id).execute(&self.pool).await.map_err(db)?;
        Ok(e)
    }

    // ---- брифинги ----

    pub async fn add_briefing(&self, goal_id: &str, path: &str) -> Result<Briefing> {
        let now = self.now_ms();
        let id = new_id("goal_briefing_");
        sqlx::query(
            "insert into goals.briefings (briefing_id, goal_id, path, created_at, updated_at, created_at_ts, updated_at_ts)
             values ($1, $2, $3, $4, $4, $5, $5)",
        )
        .bind(&id)
        .bind(goal_id)
        .bind(path)
        .bind(rfc3339(now))
        .bind(ts(now))
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(Briefing { briefing_id: id, goal_id: goal_id.into(), path: path.into(), created_at: rfc3339(now) })
    }

    pub async fn briefings(&self, goal_id: &str) -> Result<Vec<Briefing>> {
        let rows = sqlx::query("select briefing_id, goal_id, path, created_at from goals.briefings where goal_id = $1 order by created_at desc")
            .bind(goal_id)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?;
        Ok(rows.iter().map(|r| Briefing { briefing_id: r.get(0), goal_id: r.get(1), path: r.get(2), created_at: r.get(3) }).collect())
    }

    pub async fn delete_briefing(&self, id: &str) -> Result<Option<Briefing>> {
        let row = sqlx::query("delete from goals.briefings where briefing_id = $1 returning briefing_id, goal_id, path, created_at")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        Ok(row.map(|r| Briefing { briefing_id: r.get(0), goal_id: r.get(1), path: r.get(2), created_at: r.get(3) }))
    }
}
