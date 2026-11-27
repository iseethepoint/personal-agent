//! Тесты П3з-1: роли — определения ядра разбираются, имена реестра ядра
//! переводятся в имена Hamlet, набор роли отсекает чужие инструменты.

use agent_loop::roles::{hamlet_name, Role};
use agent_loop::{Tool, ToolFuture, Tools};
use agent_model::{ToolResultPart, ToolSpec};
use serde_json::{json, Value};

struct Named(&'static str);

impl Tool for Named {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.0.into(), description: String::new(), input_schema: json!({"type": "object"}) }
    }
    fn call(&self, _: Value) -> ToolFuture<'_> {
        Box::pin(async { Ok(vec![ToolResultPart::Text { text: String::new() }]) })
    }
}

#[test]
fn every_role_parses() {
    for id in ["browser_task", "foreground_root", "owner_subagent_depth1", "workflow_subagent_depth1", "scheduler_cron_execute", "scheduler_cron_resolve", "execute_resolve_worker", "verifier", "memory_flush"] {
        let r = Role::get(id);
        assert_eq!(r.id, id);
        assert!(!r.tools.is_empty(), "{id}");
    }
    assert_eq!(Role::get("browser_task").model, "session");
    assert_eq!(Role::get("memory_flush").model, "memory_flush");
}

#[test]
fn core_names_become_hamlet_names() {
    assert_eq!(hamlet_name("read"), "hamlet.read");
    assert_eq!(hamlet_name("notify_main_agent"), "hamlet.notify_main_agent");
    assert_eq!(hamlet_name("browser_search"), "browser.search");
    assert_eq!(hamlet_name("browser_deep_research"), "browser_deep_research");
    assert_eq!(hamlet_name("cron.add"), "cron.add");
}

/// Исполнитель окна: роль `browser_task` оставляет `browser.*` окна и
/// отсекает остальное. Без отбора — красный.
#[test]
fn browser_task_role_keeps_window_tools_only() {
    let mut t = Tools::new();
    for n in ["browser.act", "browser.observe", "browser.report", "hamlet.exec", "browser.windows"] {
        t.add(Named(n));
    }
    let names: Vec<String> = Role::get("browser_task").apply(t).specs().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["browser.act", "browser.observe", "browser.report"]);
}
