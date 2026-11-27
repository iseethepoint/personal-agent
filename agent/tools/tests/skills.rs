//! Тесты П4в: навыки, коннекторы, список дел. Свои маленькие навыки во
//! временной папке; инструменты — напрямую, плюс один ход через цикл на
//! обоих поставщиках.

#[path = "../../loop/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use agent_loop::{Prefix, Tools, Turn};
use agent_model::{Message, Part, Role, ToolResultPart};
use agent_tools::{add_search, Catalog, Home, Todo};
use common::{model, temp_dir, Step, Wire};
use serde_json::{json, Value};

fn skill(root: &Path, dir: &str, name: &str, description: &str, include: bool, body: &str) -> PathBuf {
    let d = root.join(dir);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("skill.yaml"),
        format!("name: {name}\ntitle: \"{name}\"\ndescription: \"{description}\"\ncategory: \"Test\"\ninclude_in_prompt: {include}\n"),
    )
    .unwrap();
    std::fs::write(d.join("SKILL.md"), body).unwrap();
    d
}

fn connector(root: &Path, id: &str, description: &str, aliases: &str) {
    let d = root.join(id);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("connector.yaml"),
        format!("id: {id}\ntitle: {id}\ndescription: {description}\naliases: [{aliases}]\ninclude_in_prompt: false\n"),
    )
    .unwrap();
    std::fs::write(d.join("CONNECTOR.md"), format!("# {id}\n")).unwrap();
}

struct Fixture {
    home: PathBuf,
    system_skills: PathBuf,
    skills: Catalog,
    tools: Tools,
}

fn setup(name: &str) -> Fixture {
    let home = temp_dir(&format!("{name}-home"));
    let system_skills = temp_dir(&format!("{name}-sys-skills"));
    let system_connectors = temp_dir(&format!("{name}-sys-conn"));
    skill(&system_skills, "flight-booking", "flight_booking", "Book flights and compare airfares.", true, "Search airlines first.\n");
    skill(&system_skills, "trip-planning", "trip_planning", "Plan a trip with an itinerary.", false, "Build the day plan.\n");
    skill(&system_skills, "artifacts/testing", "artifact_testing", "Test web artifacts in a browser.", false, "Open the page.\n");
    skill(&home.join("workspace/custom-skills"), "groceries", "grocery_shopping", "Order groceries for the week.", false, "Use the shopping list.\n");
    connector(&system_connectors, "gmail", "Read and send email.", "google mail, email");
    connector(&system_connectors, "opentable", "Book a restaurant table.", "restaurant");
    connector(&home.join("workspace/custom-connectors"), "homeassistant", "Control smart home devices.", "hass");
    let h = Home::new(&home);
    let skills = Catalog::skills(&system_skills, &h);
    let connectors = Catalog::connectors(&system_connectors, &h);
    let mut tools = Tools::new();
    add_search(&mut tools, &skills, &connectors);
    tools.add(Todo::new());
    Fixture { home, system_skills, skills, tools }
}

async fn call(tools: &Tools, name: &str, args: Value) -> Result<Value, String> {
    match tools.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(serde_json::from_str(text).unwrap()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

fn names(v: &Value, q: usize) -> Vec<String> {
    v["results"][q]["hits"].as_array().unwrap().iter().map(|h| h["name"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn search_finds_system_and_custom() {
    let f = setup("find");
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["book a flight", "groceries for the week"]})).await.unwrap();
    assert_eq!(names(&r, 0)[0], "flight_booking");
    assert_eq!(names(&r, 1), vec!["grocery_shopping"]);
    let hit = &r["results"][1]["hits"][0];
    assert!(hit["path"].as_str().unwrap().ends_with("workspace/custom-skills/groceries/SKILL.md"), "{hit}");
    assert!(r["note"].as_str().unwrap().starts_with("Read useful skills and their listed local amendments."));
    // Вложенный навык тоже в каталоге.
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["artifacts browser"]})).await.unwrap();
    assert_eq!(names(&r, 0)[0], "artifact_testing");
    // Промах — заметка muse.
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["quantum chromodynamics"]})).await.unwrap();
    assert!(names(&r, 0).is_empty());
    assert!(r["note"].as_str().unwrap().starts_with("No matching skill was found."));
    // regex — по имени, описанию и тексту SKILL.md.
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["AIRLINES"], "mode": "regex"})).await.unwrap();
    assert_eq!(names(&r, 0), vec!["flight_booking"]);
    // Ошибки — тексты muse.
    assert!(call(&f.tools, "muse.skill_search", json!({"queries": []})).await.unwrap_err().contains("queries cannot be empty"));
    assert!(call(&f.tools, "muse.skill_search", json!({"queries": ["x"], "mode": "fuzzy"}))
        .await
        .unwrap_err()
        .contains("unknown mode 'fuzzy'; expected 'bm25' or 'regex'"));
    assert!(call(&f.tools, "muse.skill_search", json!({"queries": ["x"], "maxResults": 0})).await.is_err());
}

#[tokio::test]
async fn custom_amends_system_skill() {
    let f = setup("amend");
    let own = skill(&f.home.join("workspace/custom-skills"), "trip-planning", "trip_planning", "Plan a trip my way, with trains.", false, "Prefer trains.\n");
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["plan a trip"]})).await.unwrap();
    let hits = r["results"][0]["hits"].as_array().unwrap();
    assert_eq!(hits.iter().filter(|h| h["name"] == "trip_planning").count(), 1, "один навык, не два");
    let h = hits.iter().find(|h| h["name"] == "trip_planning").unwrap();
    assert_eq!(h["description"], "Plan a trip my way, with trains.");
    assert_eq!(h["path"], f.system_skills.join("trip-planning/SKILL.md").display().to_string());
    assert_eq!(h["amendments"], json!([own.join("SKILL.md").display().to_string()]));
    // Системный каталог не тронут.
    assert!(std::fs::read_to_string(f.system_skills.join("trip-planning/skill.yaml")).unwrap().contains("itinerary"));
}

#[tokio::test]
async fn include_in_prompt_goes_to_prefix() {
    let f = setup("prompt");
    let listing = f.skills.prompt_listing();
    assert!(listing.contains("`flight_booking`: Book flights and compare airfares."), "{listing}");
    assert!(!listing.contains("trip_planning") && !listing.contains("grocery"));
    let prefix = Prefix::assemble(&format!("Системный текст.\n\n{listing}"), &f.home, &f.tools);
    assert!(prefix.system.contains("`flight_booking`"));
    // Начало собрано один раз: правка каталога его не меняет.
    std::fs::write(f.system_skills.join("trip-planning/skill.yaml"), "name: trip_planning\ndescription: x\ninclude_in_prompt: true\n").unwrap();
    assert!(!prefix.system.contains("trip_planning"));
    assert!(f.skills.prompt_listing().contains("trip_planning"), "новое начало увидит правку");
}

#[tokio::test]
async fn edited_skill_is_reread() {
    let f = setup("edit");
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["sailing"]})).await.unwrap();
    assert!(names(&r, 0).is_empty());
    std::fs::write(
        f.system_skills.join("trip-planning/skill.yaml"),
        "name: trip_planning\ndescription: \"Plan a sailing trip.\"\ninclude_in_prompt: false\n",
    )
    .unwrap();
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["sailing"]})).await.unwrap();
    assert_eq!(names(&r, 0), vec!["trip_planning"]);
    // Правка только SKILL.md тоже видна (regex по тексту).
    std::fs::write(f.system_skills.join("trip-planning/SKILL.md"), "Check the tides.\n").unwrap();
    let r = call(&f.tools, "muse.skill_search", json!({"queries": ["tides"], "mode": "regex"})).await.unwrap();
    assert_eq!(names(&r, 0), vec!["trip_planning"]);
}

#[tokio::test]
async fn connector_search_by_name_alias_description() {
    let f = setup("conn");
    let r = call(&f.tools, "muse.connector_search", json!({"queries": ["google mail", "restaurant", "smart home"]})).await.unwrap();
    assert_eq!(names(&r, 0)[0], "gmail");
    assert_eq!(names(&r, 1)[0], "opentable");
    assert_eq!(names(&r, 2)[0], "homeassistant");
    assert!(r["results"][0]["hits"][0]["path"].as_str().unwrap().ends_with("gmail/CONNECTOR.md"));
    assert!(r["note"].as_str().unwrap().starts_with("Read the matching connector docs"));
    assert!(!r["note"].as_str().unwrap().contains("<!--"));
}

#[tokio::test]
async fn todo_write() {
    let f = setup("todo");
    let ok = json!({"todos": [
        {"text": "Найти рейс", "status": "in_progress"},
        {"text": "Забронировать отель", "status": "pending"},
        {"text": "Ждать ответа владельца", "status": "blocked"},
    ]});
    assert_eq!(call(&f.tools, "todo.write", ok).await.unwrap(), json!({"items": 3, "in_progress": 1}));
    let two = json!({"todos": [{"text": "a", "status": "in_progress"}, {"text": "b", "status": "in_progress"}]});
    assert!(call(&f.tools, "todo.write", two).await.unwrap_err().contains("keep exactly one item in_progress"));
    let none = json!({"todos": [{"text": "a", "status": "pending"}]});
    assert!(call(&f.tools, "todo.write", none).await.unwrap_err().contains("no item is in_progress"));
    let empty = json!({"todos": [{"text": " ", "status": "in_progress"}]});
    assert!(call(&f.tools, "todo.write", empty).await.unwrap_err().contains("every todo item needs non-empty text"));
    let many: Vec<Value> = (0..26).map(|i| json!({"text": format!("шаг {i}"), "status": "blocked"})).collect();
    assert!(call(&f.tools, "todo.write", json!({"todos": many})).await.is_err());
    assert_eq!(call(&f.tools, "todo.write", json!({"todos": []})).await.unwrap()["items"], 0);
}

async fn one_turn(wire: Wire) {
    let f = setup("turn");
    let steps = vec![
        Step::Calls(vec![
            ("c1", "muse.skill_search", json!({"queries": ["book a flight"]})),
            ("c2", "todo.write", json!({"todos": [{"text": "Прочитать навык", "status": "in_progress"}]})),
        ]),
        Step::Text("Нашёл навык."),
    ];
    let (server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &f.tools, "s", temp_dir("turn-spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "купи билет".into() }] }];
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Нашёл навык.");
    let body = server.requests()[1]["messages"].to_string();
    assert!(body.contains("flight_booking"), "{body}");
    let tools_json = server.requests()[0]["tools"].to_string();
    for n in ["muse.skill_search", "muse.connector_search", "todo.write"] {
        assert!(tools_json.contains(&format!("\"{n}\"")), "{n}");
    }
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}
