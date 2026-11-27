//! Роли — определения агентов muse (`strings-all.txt:69124-74375`): у
//! каждой роли `id`, `name`, `model`, `tool_loading` и список `tools`.
//! Файлы `prompts/roles/<id>.toml` — определения ядра как есть (П10: имена
//! флагов `muse_*` → `hamlet_*`). Набор
//! инструментов агента — общий набор, отобранный списком его роли;
//! инструментов роли, которых у Hamlet нет, в наборе нет.
//!
//! Имена в списке — имена реестра muse; у Hamlet: имя с точкой — как есть,
//! `browser_<x>` — `browser.<x>` (кроме `browser_deep_research`), прочие —
//! `hamlet.<x>` (`read` → `hamlet.read`, `notify_main_agent` →
//! `hamlet.notify_main_agent`).

use crate::tool::Tools;
use serde::Deserialize;

/// Определения ядра.
const ROLES: [(&str, &str); 9] = [
    ("browser_task", include_str!("../../../prompts/roles/browser_task.toml")),
    ("foreground_root", include_str!("../../../prompts/roles/foreground_root.toml")),
    ("owner_subagent_depth1", include_str!("../../../prompts/roles/owner_subagent_depth1.toml")),
    ("workflow_subagent_depth1", include_str!("../../../prompts/roles/workflow_subagent_depth1.toml")),
    ("scheduler_cron_execute", include_str!("../../../prompts/roles/scheduler_cron_execute.toml")),
    ("scheduler_cron_resolve", include_str!("../../../prompts/roles/scheduler_cron_resolve.toml")),
    ("execute_resolve_worker", include_str!("../../../prompts/roles/execute_resolve_worker.toml")),
    ("verifier", include_str!("../../../prompts/roles/verifier.toml")),
    ("memory_flush", include_str!("../../../prompts/roles/memory_flush.toml")),
];

/// Роль агента.
#[derive(Debug, Clone, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub model: String,
    pub tool_loading: String,
    pub tools: Vec<String>,
}

/// Имя инструмента Hamlet по имени реестра ядра.
pub fn hamlet_name(core: &str) -> String {
    if core.contains('.') || core == "browser_deep_research" {
        core.to_string()
    } else if let Some(rest) = core.strip_prefix("browser_") {
        format!("browser.{rest}")
    } else {
        format!("hamlet.{core}")
    }
}

impl Role {
    /// Роль по `id`.
    pub fn get(id: &str) -> Role {
        let (_, text) = ROLES.iter().find(|(r, _)| *r == id).unwrap_or_else(|| panic!("нет роли {id}"));
        toml::from_str(text).unwrap_or_else(|e| panic!("роль {id}: {e}"))
    }

    /// Имена инструментов роли у Hamlet.
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| hamlet_name(t)).collect()
    }

    pub fn allows(&self, name: &str) -> bool {
        self.tools.iter().any(|t| hamlet_name(t) == name)
    }

    /// Набор роли: из `tools` — только инструменты её списка.
    pub fn apply(&self, mut tools: Tools) -> Tools {
        tools.retain(|n| self.allows(n));
        tools
    }
}
