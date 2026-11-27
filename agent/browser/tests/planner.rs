//! Тесты П4к-1а: исполнитель через цикл на обоих поставщиках проходит
//! задачу на поддельной странице и отчитывается; вопрос главному агенту;
//! ответ без отчёта возвращается к модели.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
mod site;

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_browser::planner::{assignment_message, Asker, Planner, Question};
use agent_browser::window::Window;
use common::{model, temp_dir, Step, Wire};
use serde_json::json;

struct Owner(Mutex<Vec<Question>>);

impl Asker for Owner {
    fn ask<'a>(&'a self, q: Question) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        self.0.lock().unwrap().push(q);
        Box::pin(async { Ok("Use the name Ann Lee and a blue mug.".to_string()) })
    }
}

#[tokio::test]
async fn planner_completes_order_through_loop() {
    let Some(bin) = site::chromium() else { return };
    for wire in [Wire::Anthropic, Wire::OpenaiCompat] {
        let base = site::serve().await;
        let dir = site::profile("planner");
        let window = Window::open(&bin, &dir.0, true).await.unwrap();
        // Ссылки нового окна идут по порядку: e1 — имя, e2 — цвет,
        // e3/e4 — варианты, e5 — согласие, e6 — кнопка.
        let (server, provider) = model(
            wire,
            vec![
                Step::Calls(vec![("a1", "browser.act", json!({"tab": "t1", "intent": "Opening the shop", "actions": [{"kind": "navigate", "url": format!("{base}/")}]}))]),
                Step::Calls(vec![("q1", "browser.request_input", json!({"question": "Which name and color?", "reason": "missing_info"}))]),
                Step::Calls(vec![(
                    "a2",
                    "browser.act",
                    json!({"tab": "t1", "intent": "Placing the order", "actions": [
                        {"kind": "fill", "ref": "e1", "text": "Ann Lee"},
                        {"kind": "select", "ref": "e2", "value": "blue"},
                        {"kind": "set_checked", "ref": "e5", "checked": true},
                        {"kind": "click", "ref": "e6"}]}),
                )]),
                Step::Text("The order is placed."),
                Step::Calls(vec![("r1", "browser.report", json!({"status": "completed", "summary": "Order A-1042 placed for a blue mug."}))]),
            ],
            true,
        );
        let owner = Arc::new(Owner(Mutex::new(vec![])));
        let planner = Planner { provider: Arc::new(provider), window: window.clone(), asker: owner.clone(), locale: "en-US".into(), spill_dir: temp_dir("browser-spill"), updates: None };
        let mut history = vec![];
        let report = planner.run(&mut history, assignment_message(&format!("Order a blue mug at {base}/ for Ann Lee."))).await.unwrap();
        assert_eq!(report.status, "completed", "{wire:?}");
        assert_eq!(report.summary, "Order A-1042 placed for a blue mug.");
        assert_eq!(owner.0.lock().unwrap()[0].question, "Which name and color?");

        let reqs = server.requests();
        let first = reqs[0].to_string();
        assert!(first.contains("browser planner") && first.contains("Carry out this assignment end to end"), "{wire:?}");
        assert!(first.contains("browser.observe") && first.contains("browser.report") && !first.contains("browser.login"), "инструменты исполнителя");
        assert!(reqs[2].to_string().contains("Use the name Ann Lee and a blue mug."), "ответ главного агента");
        let after_order = reqs[3].to_string();
        assert!(after_order.contains("Order number A-1042") && after_order.contains("the page navigated"), "{wire:?}: вид после заказа");
        assert!(after_order.contains("image/jpeg") || after_order.contains("data:image/jpeg"), "снимок уходит модели");
        assert!(reqs[4].to_string().contains("Finish with `browser.report`"), "ответ без отчёта — ворота завершения");

        window.close().await;
    }
}

/// Модель исполнителя — через `Registry::resolve` по назначению
/// `BrowserAgent`: переопределение `setup.overrides` держит браузер на своей
/// модели и при выборе другой.
#[test]
fn planner_model_comes_from_browser_agent_purpose() {
    let base = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../model/tests/fixtures/registry_base.toml")).unwrap();
    let reg = agent_model::Registry::parse(&base).unwrap();
    assert_eq!(agent_browser::planner::model(&reg, Some("local/chat")).unwrap().option.id, "anthropic/main");
    let other = base.replacen("{ purpose = \"BrowserAgent\", model = \"anthropic/main\" }", "{ purpose = \"BrowserAgent\", model = \"anthropic/small\" }", 1);
    let reg = agent_model::Registry::parse(&other).unwrap();
    assert_eq!(agent_browser::planner::model(&reg, Some("local/chat")).unwrap().option.id, "anthropic/small");
}
