//! Отложенные пространства инструментов и `tool_search.load_tool_namespace`
//! — форма muse (профиль `foreground_root`: `tool_loading = "deferred"`,
//! `strings-all.txt:70040`; блок `blocks/user_chat/loading_tools.md`,
//! `:59570-59576`; описание и поле `paths` инструмента загрузки, ответы —
//! строки ядра около `0xa10827`, `0x6daf7e`, `0x99117a`, `0x990e85`,
//! `0x96daa7`; маркеры — `0x9d9b51`; краткие описания пространств —
//! `0x2cc9b77`–`0x2ccbae2`; тексты — `prompts/tool_search/`).
//!
//! Функции отложенного пространства не уходят модели, пока пространство не
//! загружено. Само пространство уходит в списке инструментов запроса
//! заглушкой: краткое описание с маркером в начале, без пробела (форма ядра:
//! `strings-all.txt:332137` «[deferred: use tool_search to discover
//! functions]Drive the user's live app UI…»; маркеры — `templates.txt:2253`).
//! В системном тексте — только правила, блок «Loading Tools»
//! (`blocks/user_chat/loading_tools.md`, `0x7a9c33`, часть
//! `client-tool-loading-only`). Пространство загружено, пока схемы из его
//! загрузки лежат в контексте («a namespace counts as loaded while the full
//! schemas from its load are still in your context») — поэтому загруженное
//! вычисляется по истории разговора: после сжатия оно снова отложено. Вызов
//! функции, которая ещё не загружена, не исполняется: ответ — её схема и
//! «NOW LOADED» (`0x99117a`).
//!
//! Своё (в разборе нет):
//! - какие пространства отложены — решает сборка (`Tools::defer`);
//! - заглушка — инструмент с именем пространства и пустой схемой параметров
//!   (у поставщиков с плоским списком функций иначе её не передать), в конце
//!   списка; вызов заглушки — как любого неизвестного инструмента;
//! - вид схем в ответе: описание пространства, затем по функции —
//!   заголовок, описание и JSON Schema;
//! - загруженные функции добавляются и в список инструментов запроса (в
//!   конец), чтобы любой поставщик принял их вызов;
//! - склейка ответов загрузки из строк ядра (какой текст в каком случае).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use agent_model::{Message, Part, Role, ToolSpec};
use serde_json::{json, Value};

use crate::compaction::personal;
use crate::tool::Tools;

pub const LOAD_TOOL: &str = "tool_search.load_tool_namespace";

const TEXTS_JSON: &str = include_str!("../../../prompts/tool_search/tools.json");
const LOADING_TOOLS: &str = include_str!("../../../prompts/user_chat/loading_tools.md");

static TEXTS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(TEXTS_JSON).expect("prompts/tool_search/tools.json"));

fn text(group: &str, key: &str) -> String {
    personal(TEXTS[group][key].as_str().unwrap_or_default())
}

/// Отложенное пространство: имя, краткое описание (строка списка), полное
/// описание (уходит при загрузке).
#[derive(Debug, Clone)]
pub struct Namespace {
    pub name: String,
    pub summary: String,
    pub description: String,
}

pub(crate) fn load_spec() -> ToolSpec {
    ToolSpec {
        name: LOAD_TOOL.into(),
        description: text("tool", "description"),
        input_schema: json!({
            "type": "object",
            "properties": {"paths": {"type": "array", "items": {"type": "string"}, "description": text("tool", "paths")}},
            "required": ["paths"],
            "additionalProperties": false,
        }),
    }
}

/// Что загружено в разговоре: пространства и отдельные функции.
#[derive(Debug, Default, Clone)]
pub struct Loaded {
    namespaces: HashSet<String>,
    functions: HashSet<String>,
}

impl Tools {
    /// Отложить пространство `name` (функции `name.*`); краткое описание —
    /// из ядра (`prompts/tool_search/tools.json`), полное — `description`.
    pub fn defer(&mut self, name: &str, description: &str) -> &mut Self {
        self.deferred.retain(|n| n.name != name);
        self.deferred.push(Namespace { name: name.into(), summary: text("summaries", name), description: description.into() });
        self
    }

    pub fn has_deferred(&self) -> bool {
        !self.deferred.is_empty()
    }

    pub(crate) fn namespace_of(&self, tool: &str) -> Option<&Namespace> {
        let (ns, _) = tool.split_once('.')?;
        self.deferred.iter().find(|n| n.name == ns)
    }

    /// Блок системного текста «Loading Tools» — правила, без списка
    /// пространств; метки `client-tool-loading-only` снимаются (Hamlet
    /// грузит инструменты сам), строки, где были только метки, уходят.
    pub fn loading_block(&self) -> Option<String> {
        self.client_loading(LOADING_TOOLS)
    }

    /// Текст о загрузке инструментов (`client-tool-loading-only`) с метками
    /// отложенного: есть отложенные пространства — метки сняты, строки только
    /// с метками ушли; нет — `None`.
    pub fn client_loading(&self, raw: &str) -> Option<String> {
        if self.deferred.is_empty() {
            return None;
        }
        let text = raw
            .replace("{deferred_stub_marker}", &text("markers", "stub"))
            .replace("{deferred_function_marker}", &text("markers", "function"));
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("<!-- client-tool-loading-only:")).collect();
        Some(personal(lines.join("\n").trim_end()))
    }

    /// Заглушки отложенных пространств для списка инструментов запроса.
    pub(crate) fn stub_specs(&self) -> Vec<ToolSpec> {
        let stub = text("markers", "stub");
        self.deferred
            .iter()
            .map(|n| ToolSpec {
                name: n.name.clone(),
                description: format!("{stub}{}", n.summary.trim()),
                input_schema: json!({"type": "object", "properties": {}}),
            })
            .collect()
    }

    /// Загруженное по истории разговора.
    pub fn loaded(&self, history: &[Message]) -> Loaded {
        let mut out = Loaded::default();
        if self.deferred.is_empty() {
            return out;
        }
        let answered: HashSet<&str> = history
            .iter()
            .filter(|m| m.role == Role::User)
            .flat_map(|m| m.content.iter())
            .filter_map(|p| match p {
                Part::ToolResult { call_id, .. } => Some(call_id.as_str()),
                _ => None,
            })
            .collect();
        for p in history.iter().filter(|m| m.role == Role::Assistant).flat_map(|m| m.content.iter()) {
            let Part::ToolCall { id, name, arguments } = p else { continue };
            if !answered.contains(id.as_str()) {
                continue;
            }
            if name == LOAD_TOOL {
                for path in paths(arguments) {
                    if self.deferred.iter().any(|n| n.name == path) {
                        out.namespaces.insert(path);
                    } else if self.namespace_of(&path).is_some() && self.get(&path).is_some() {
                        out.functions.insert(path);
                    }
                }
            } else if self.namespace_of(name).is_some() {
                out.functions.insert(name.clone());
            }
        }
        out
    }

    fn is_loaded(&self, l: &Loaded, tool: &str) -> bool {
        match self.namespace_of(tool) {
            None => true,
            Some(ns) => l.namespaces.contains(&ns.name) || l.functions.contains(tool),
        }
    }

    /// Описания загруженных функций — к списку инструментов запроса.
    pub fn loaded_specs(&self, l: &Loaded) -> Vec<ToolSpec> {
        self.specs().into_iter().filter(|s| self.namespace_of(&s.name).is_some() && self.is_loaded(l, &s.name)).collect()
    }

    /// Не загруженная ещё функция отложенного пространства.
    pub fn needs_load(&self, l: &Loaded, tool: &str) -> bool {
        self.get(tool).is_some() && !self.is_loaded(l, tool)
    }

    fn schema_block(spec: &ToolSpec) -> String {
        format!("### {}\n{}\n```json\n{}\n```", spec.name, spec.description.trim_end(), serde_json::to_string_pretty(&spec.input_schema).unwrap_or_default())
    }

    /// Ответ на вызов ещё не загруженной функции: её схема.
    pub fn autoload_text(&self, tool: &str) -> String {
        let spec = self.get(tool).map(|t| t.spec());
        let head = text("texts", "autoload").replace("{name}", tool);
        match spec {
            Some(s) => format!("{head}\n\n{}", Self::schema_block(&s)),
            None => head,
        }
    }

    /// `tool_search.load_tool_namespace`: схемы запрошенных пространств и
    /// функций.
    pub fn load(&self, args: &Value, l: &Loaded) -> String {
        let mut unknown = false;
        let mut fresh: Vec<ToolSpec> = vec![];
        let mut fresh_ns: Vec<&Namespace> = vec![];
        let mut had_loaded = false;
        let mut already: Vec<String> = vec![];
        let specs = self.specs();
        let by_name: HashMap<&str, &ToolSpec> = specs.iter().map(|s| (s.name.as_str(), s)).collect();
        for path in paths(args) {
            let members: Vec<&ToolSpec> = if let Some(ns) = self.deferred.iter().find(|n| n.name == path) {
                let m: Vec<&ToolSpec> = specs.iter().filter(|s| self.namespace_of(&s.name).is_some_and(|x| x.name == ns.name)).collect();
                if m.iter().all(|s| self.is_loaded(l, &s.name)) {
                    already.push(ns.name.clone());
                } else if !fresh_ns.iter().any(|x| x.name == ns.name) {
                    fresh_ns.push(ns);
                }
                m
            } else if let (Some(_), Some(s)) = (self.namespace_of(&path), by_name.get(path.as_str())) {
                vec![*s]
            } else {
                unknown = true;
                vec![]
            };
            for s in members {
                if self.is_loaded(l, &s.name) {
                    had_loaded = true;
                } else if !fresh.iter().any(|f| f.name == s.name) {
                    fresh.push(s.clone());
                }
            }
        }
        let mut out = if !fresh.is_empty() {
            let head = if had_loaded { text("texts", "loaded_missing") } else { text("texts", "loaded") }.replace("{n}", &fresh.len().to_string());
            let mut blocks = vec![head];
            for ns in &fresh_ns {
                blocks.push(format!("## {}\n{}", ns.name, personal(&ns.description).trim_end()));
            }
            blocks.extend(fresh.iter().map(Self::schema_block));
            blocks.join("\n\n")
        } else if self.deferred.iter().all(|n| l.namespaces.contains(&n.name)) {
            text("texts", "every_loaded")
        } else if !already.is_empty() {
            text("texts", "already_loaded").replace("{names}", &already.join(", "))
        } else if had_loaded {
            text("texts", "all_available")
        } else {
            String::new()
        };
        if unknown {
            let note = format!("{}\n{} {}", text("texts", "unavailable"), text("texts", "hint_loaded"), text("texts", "hint_connectors"));
            out = if out.is_empty() { note } else { format!("{out}\n{note}") };
        }
        out
    }
}

fn paths(args: &Value) -> Vec<String> {
    match args.get("paths") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        Some(Value::String(s)) => vec![s.trim().to_string()],
        _ => vec![],
    }
}
