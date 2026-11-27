//! Инструмент и реестр инструментов. Описание инструмента — `ToolSpec` из
//! прослойки модели, одно на всех поставщиков; исполнение — `call(args)`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use agent_model::{ToolResultPart, ToolSpec};
use serde_json::Value;

/// Ошибка инструмента. Модели уходит результатом с `is_error`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// Аргументы не подходят инструменту. Модели уходит текстом muse
    /// «Invalid arguments for {name}: …» (`strings-all.txt:11963`).
    #[error("{0}")]
    InvalidArguments(String),
    /// Инструмент не справился.
    #[error("{0}")]
    Failed(String),
}

pub type ToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<ToolResultPart>, ToolError>> + Send + 'a>>;

/// Инструмент: описание и исполнение.
pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn call(&self, args: Value) -> ToolFuture<'_>;
}

/// Реестр: имя → инструмент. Описания модели уходят в порядке добавления.
/// Функции отложенных пространств (`defer`) уходят модели только после
/// загрузки (`tool_search.rs`).
#[derive(Default, Clone)]
pub struct Tools {
    list: Vec<(String, Arc<dyn Tool>)>,
    pub(crate) deferred: Vec<crate::tool_search::Namespace>,
}

impl Tools {
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавить инструмент; инструмент с тем же именем заменяется.
    pub fn add(&mut self, tool: impl Tool + 'static) -> &mut Self {
        let name = tool.spec().name;
        let tool: Arc<dyn Tool> = Arc::new(tool);
        match self.list.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = tool,
            None => self.list.push((name, tool)),
        }
        self
    }

    /// Оставить только инструменты, для которых `keep` отвечает «да»;
    /// отложенное пространство без оставшихся функций уходит.
    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) -> &mut Self {
        self.list.retain(|(n, _)| keep(n));
        let names: Vec<String> = self.list.iter().map(|(n, _)| n.clone()).collect();
        self.deferred.retain(|ns| names.iter().any(|n| n.strip_prefix(ns.name.as_str()).is_some_and(|r| r.starts_with('.'))));
        self
    }

    /// Взять инструмент из другого реестра под его именем.
    pub fn add_shared(&mut self, name: &str, tool: Arc<dyn Tool>) -> &mut Self {
        match self.list.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = tool,
            None => self.list.push((name.to_string(), tool)),
        }
        self
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.list.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    /// Все описания, вместе с `tool_search.load_tool_namespace`, если есть
    /// отложенные пространства.
    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut v: Vec<ToolSpec> = self.list.iter().map(|(_, t)| t.spec()).collect();
        if !self.deferred.is_empty() {
            v.push(crate::tool_search::load_spec());
        }
        v
    }

    /// Описания, которые уходят модели сразу: без функций отложенных
    /// пространств, с заглушками самих пространств в конце (`tool_search.rs`).
    pub fn upfront_specs(&self) -> Vec<ToolSpec> {
        let mut out: Vec<ToolSpec> = self.specs().into_iter().filter(|s| self.namespace_of(&s.name).is_none()).collect();
        out.extend(self.stub_specs());
        out
    }
}
