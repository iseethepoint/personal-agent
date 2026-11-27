//! Тесты П4л-3: отложенные пространства и `tool_search.load_tool_namespace`
//! на обоих проводах.

mod common;

use agent_loop::{Prefix, Tool, ToolFuture, Tools, Turn, LOAD_TOOL};
use agent_model::{Message, Part, Role, ToolResultPart, ToolSpec};
use common::{model, temp_dir, Echo, Step, Wire};
use serde_json::{json, Value};

/// Инструмент с именем пространства: отвечает своим именем.
struct Named(&'static str);

impl Tool for Named {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.0.into(), description: format!("Описание {}.", self.0), input_schema: json!({"type": "object", "properties": {"x": {"type": "string"}}}) }
    }
    fn call(&self, _args: Value) -> ToolFuture<'_> {
        let n = self.0;
        Box::pin(async move { Ok(vec![ToolResultPart::Text { text: format!("ran {n}") }]) })
    }
}

fn deferred_tools() -> Tools {
    let mut t = Tools::new();
    t.add(Echo).add(Named("workflow.launch")).add(Named("workflow.view"));
    t.defer("workflow", "FULL WORKFLOW NAMESPACE TEXT");
    t
}

/// Имена инструментов запроса (оба провода).
fn tool_names(req: &Value) -> Vec<String> {
    req["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.get("name").or_else(|| t["function"].get("name")).and_then(Value::as_str).unwrap().to_string())
        .collect()
}

fn results(history: &[Message]) -> Vec<(String, bool)> {
    history
        .iter()
        .filter(|m| m.role == Role::User)
        .flat_map(|m| m.content.iter())
        .filter_map(|p| match p {
            Part::ToolResult { content, is_error, .. } => Some((
                content.iter().filter_map(|c| if let ToolResultPart::Text { text } = c { Some(text.clone()) } else { None }).collect(),
                *is_error,
            )),
            _ => None,
        })
        .collect()
}

async fn deferred_namespace_loads_on_demand(wire: Wire) {
    let (server, provider) = model(
        wire,
        vec![
            Step::Calls(vec![("c1", "workflow.view", json!({"x": "a"}))]),
            Step::Calls(vec![("c2", LOAD_TOOL, json!({"paths": ["workflow", "nope"]}))]),
            Step::Calls(vec![("c3", "workflow.launch", json!({"x": "b"}))]),
            Step::Calls(vec![("c4", LOAD_TOOL, json!({"paths": ["workflow"]}))]),
            Step::Text("Готово."),
        ],
        false,
    );
    let tools = deferred_tools();
    let dir = temp_dir("tool-search");
    let prefix = Prefix::assemble("SYSTEM", &dir, &tools);
    // Сразу модели уходят неотложенные функции, инструмент загрузки и
    // заглушка пространства с маркером в начале описания (форма ядра,
    // `strings-all.txt:332137`); в системном тексте — только правила.
    let upfront: Vec<String> = prefix.tools.iter().map(|s| s.name.clone()).collect();
    assert_eq!(upfront, ["echo", LOAD_TOOL, "workflow"]);
    let stub = &prefix.tools[2];
    assert!(stub.description.starts_with("[deferred: use tool_search to discover functions]Create, update, delete, launch, inspect, and control VM-local JavaScript workflows"), "{}", stub.description);
    assert!(prefix.system.starts_with(&format!("SYSTEM\n\n{LOADING_TOOLS_SNAPSHOT}\n\n<!-- INJECTED FILE: AGENTS.md -->")), "{}", prefix.system);
    assert!(!prefix.system.contains("`workflow`"));
    assert!(!prefix.system.contains("{deferred_"));

    let mut history = vec![Message { role: Role::User, content: vec![Part::Text { text: "запусти работу".into() }] }];
    let turn = Turn::from_prefix(&provider, &tools, &prefix, dir.join("spill"));
    turn.run(&mut history, &mut |_| {}).await.unwrap();

    let r = results(&history);
    // Вызов незагруженной функции не исполняется: её схема и «NOW LOADED».
    assert!(r[0].1);
    assert!(r[0].0.starts_with("tool workflow.view is deferred and was not yet loaded. Its full schema follows and it is NOW LOADED: retry the call with arguments matching this schema.\n\n### workflow.view\nОписание workflow.view."), "{}", r[0].0);
    assert!(!r[0].0.contains("ran "));
    // Загрузка: недостающая функция и описание пространства; неизвестное — пометка.
    assert!(!r[1].1);
    assert!(r[1].0.starts_with("Loaded missing schemas from 1 function(s). Full schemas:\n\n## workflow\nFULL WORKFLOW NAMESPACE TEXT\n\n### workflow.launch\n"), "{}", r[1].0);
    assert!(r[1].0.contains("\n(Some requested namespaces or tools were unavailable.)\nCall already-loaded functions directly:"), "{}", r[1].0);
    // После загрузки функция исполняется.
    assert_eq!(r[2], ("ran workflow.launch".to_string(), false));
    // Повторная загрузка — уже загружено.
    assert_eq!(r[3].0, "Every deferred namespace is already loaded; call the loaded functions directly.");

    let reqs = server.requests();
    assert_eq!(tool_names(&reqs[0]), ["echo", LOAD_TOOL, "workflow"]);
    assert_eq!(tool_names(&reqs[1]), ["echo", LOAD_TOOL, "workflow", "workflow.view"]);
    assert_eq!(tool_names(&reqs[2]), ["echo", LOAD_TOOL, "workflow", "workflow.launch", "workflow.view"]);
    assert!(reqs[0].to_string().contains("### Loading Tools"));
}

/// Блок «Loading Tools» — побайтно как в копии системного текста muse
/// (`rootfs-full/home/hatch/workspace/system/system_prompt.md`, строки
/// 278–283).
const LOADING_TOOLS_SNAPSHOT: &str = "### Loading Tools\n\nTools are organized into namespaces and functions. Some namespaces are not loaded fully and need to be loaded. Call `tool_search.load_tool_namespace` with the namespace's name to get its deferred function descriptions and parameter schemas. While a tool's schema is in your context, you can use it without loading it again.\n\n- Namespaces marked \"[deferred: use tool_search to discover functions]\" have deferred functions. A namespace whose functions are all deferred hides their entries until loaded.\n- Namespaces marked \"[deferred: use tool_search to load full schema]\" have no parameter schema for the functions until loaded.\n- Namespaces with fully specified functions and schemas are not deferred and can be called directly.";

#[test]
fn loading_block_matches_muse_snapshot() {
    assert_eq!(deferred_tools().loading_block().unwrap(), LOADING_TOOLS_SNAPSHOT);
}

#[tokio::test]
async fn deferred_namespace_loads_on_demand_anthropic() {
    deferred_namespace_loads_on_demand(Wire::Anthropic).await;
}

#[tokio::test]
async fn deferred_namespace_loads_on_demand_openai_compat() {
    deferred_namespace_loads_on_demand(Wire::OpenaiCompat).await;
}

#[test]
fn load_answers_and_loaded_state_follow_history() {
    let tools = deferred_tools();
    let call = |id: &str, name: &str, args: Value| Message { role: Role::Assistant, content: vec![Part::ToolCall { id: id.into(), name: name.into(), arguments: args }] };
    let result = |id: &str| Message { role: Role::User, content: vec![Part::ToolResult { call_id: id.into(), content: vec![], is_error: false }] };
    // Ничего не загружено.
    let none = tools.loaded(&[]);
    assert!(tools.needs_load(&none, "workflow.launch"));
    assert!(!tools.needs_load(&none, "echo"));
    let fresh = tools.load(&json!({"paths": ["workflow"]}), &none);
    assert!(fresh.starts_with("Loaded 2 function(s). Full schemas:"), "{fresh}");
    // Загрузка без ответа в истории не считается; с ответом — пространство загружено.
    let h = vec![call("a", LOAD_TOOL, json!({"paths": ["workflow"]}))];
    assert!(tools.needs_load(&tools.loaded(&h), "workflow.view"));
    let h = vec![call("a", LOAD_TOOL, json!({"paths": ["workflow"]})), result("a")];
    let l = tools.loaded(&h);
    assert!(!tools.needs_load(&l, "workflow.view"));
    assert_eq!(tools.loaded_specs(&l).len(), 2);
    // Только неизвестное.
    let unknown = tools.load(&json!({"paths": ["skills"]}), &none);
    assert!(unknown.starts_with("(Some requested namespaces or tools were unavailable.)"), "{unknown}");
    assert!(unknown.contains("hamlet.connector_search"));
    // Загрузка одной функции.
    let one = tools.load(&json!({"paths": ["workflow.view"]}), &none);
    assert!(one.starts_with("Loaded 1 function(s). Full schemas:\n\n### workflow.view"), "{one}");
    // Без отложенных пространств инструмента загрузки нет.
    let mut plain = Tools::new();
    plain.add(Echo);
    assert_eq!(plain.specs().len(), 1);
    assert_eq!(plain.upfront_specs().len(), 1);
    assert!(plain.loading_block().is_none());
}
