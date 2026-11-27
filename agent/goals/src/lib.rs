//! `agent-goals` — цели по форме muse `hatch-goals` и `goals_tool`:
//! таблицы `goals.*` в Postgres, папка цели `~/workspace/goals/<slug>/`,
//! инструменты `user_goal.*` и `tracking.*`.

pub mod activity;
pub mod store;
pub mod tools;
pub mod workspace;

use std::future::Future;
use std::pin::Pin;

pub use store::{Entry, Goal, NewGoal, Store, TRACKING, USER_GOAL};
pub use tools::{tools, GoalTool, Goals, Models, Ns, Plan};

/// Удаление расписаний, принадлежащих цели (`cron_cleanup.rs` muse:
/// «Deleting removes its subgoals, history, briefings, and the scheduled
/// jobs that belong to it»). Возвращает id удалённых заданий.
pub trait OwnedCleanup: Send + Sync {
    fn remove_owned<'a>(&'a self, owner: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send + 'a>>;
}
