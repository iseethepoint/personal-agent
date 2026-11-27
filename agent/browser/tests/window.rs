//! Тесты П4к-1а: окно на живом Chromium (headless) и поддельной странице.

mod site;

use agent_browser::window::Window;
use agent_model::ToolResultPart;
use serde_json::json;

fn text(parts: &[ToolResultPart]) -> String {
    parts.iter().filter_map(|p| match p { ToolResultPart::Text { text } => Some(text.as_str()), _ => None }).collect::<Vec<_>>().join("\n")
}

fn ref_of(view: &str, needle: &str) -> String {
    let line = view.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("нет «{needle}» в виде:\n{view}"));
    let at = line.find('@').unwrap_or_else(|| panic!("нет ссылки в строке {line}"));
    line[at + 1..].split_whitespace().next().unwrap().to_string()
}

#[tokio::test]
async fn window_fills_form_follows_links_and_tabs() {
    let Some(bin) = site::chromium() else { return };
    let base = site::serve().await;
    let dir = site::profile("window");
    let w = Window::open(&bin, &dir.0, true).await.unwrap();

    let parts = w.act("t1", &[json!({"kind": "navigate", "url": format!("{base}/")})]).await.unwrap();
    let t = text(&parts);
    assert!(t.contains("Action 1 (navigate): applied; the page navigated."), "{t}");
    assert!(t.contains("## Tab t1, view v") && t.contains("Title: Fake shop"), "{t}");
    assert!(parts.iter().any(|p| matches!(p, ToolResultPart::Image(_))), "снимок при виде");
    let name = ref_of(&t, "textbox \"Your name\"");
    let color = ref_of(&t, "combobox \"Color\"");
    let agree = ref_of(&t, "checkbox \"I agree to the terms\"");
    let submit = ref_of(&t, "button \"Place order\"");

    let parts = w
        .act("t1", &[
            json!({"kind": "fill", "ref": format!("@{name}"), "text": "Ann Lee"}),
            json!({"kind": "select", "ref": color, "value": "blue"}),
            json!({"kind": "set_checked", "ref": agree, "checked": true}),
            json!({"kind": "click", "ref": submit}),
        ])
        .await
        .unwrap();
    let t = text(&parts);
    assert!(t.contains("Action 4 (click @") && t.contains("the page navigated"), "{t}");
    assert!(t.contains("Thanks Ann Lee: blue mug, terms yes. Order number A-1042."), "{t}");

    // Старая ссылка — уже не в последнем виде.
    let parts = w.act("t1", &[json!({"kind": "click", "ref": submit})]).await.unwrap();
    assert!(text(&parts).contains("no longer names a control by its reference in the latest view"), "{}", text(&parts));

    w.act("t1", &[json!({"kind": "back"})]).await.unwrap();
    let v = w.observe("t1", "auto", None, None).await.unwrap();
    let help = ref_of(&v.text, "link \"Open help\"");
    let parts = w.act("t1", &[json!({"kind": "click", "ref": help})]).await.unwrap();
    let t = text(&parts);
    assert!(t.contains("it opened tab t2") && t.contains("## Tab t2, view"), "{t}");
    let list = text(&w.tabs("list", None, None, None).await.unwrap());
    assert!(list.contains("- t1") && list.contains("- t2") && list.contains("Title: Help"), "{list}");

    let v = w.observe("t1", "text", Some("price"), None).await.unwrap();
    assert!(!v.text.contains("Ceramic"), "на t1 нет описания");
    let details = ref_of(&w.observe("t1", "auto", None, None).await.unwrap().text, "link \"Mug details\"");
    let parts = w.tabs("open", None, Some("t1"), Some(&details)).await.unwrap();
    assert!(text(&parts).contains("Opened tab t3. It loaded the requested URL.") && text(&parts).contains("Ceramic mug, 350 ml, price 12 EUR."), "{}", text(&parts));
    let v = w.observe("t3", "text", Some("price"), None).await.unwrap();
    assert!(v.text.ends_with("Ceramic mug, 350 ml, price 12 EUR."), "{}", v.text);

    let parts = w.tabs("close", None, Some("t2"), None).await.unwrap();
    assert!(!text(&parts).contains("- t2"), "{}", text(&parts));
    let parts = w.act("t1", &[json!({"kind": "wait", "text": "never appears", "condition": "visible", "timeout_ms": 300}), json!({"kind": "reload"})]).await.unwrap();
    let t = text(&parts);
    assert!(t.contains("timed out: condition_not_met") && t.contains("Action 2 (reload): not started."), "{t}");

    // Страница ушла — следующие шаги со ссылками прежнего вида не идут.
    w.act("t1", &[json!({"kind": "navigate", "url": format!("{base}/")})]).await.unwrap();
    let v = w.observe("t1", "auto", None, None).await.unwrap();
    let (details, help) = (ref_of(&v.text, "link \"Mug details\""), ref_of(&v.text, "link \"Open help\""));
    let parts = w.act("t1", &[json!({"kind": "click", "ref": details}), json!({"kind": "click", "ref": help})]).await.unwrap();
    let t = text(&parts);
    assert!(t.contains("Action 2 (click @") && t.contains("): not started.") && t.contains("Title: Mug details"), "{t}");

    w.close().await;
}
