//! Тесты П4л-4: корзина `trash.*`. Инструменты — напрямую с поддельными
//! часами, плюс один ход через цикл на обоих поставщиках.

#[path = "../../loop/tests/common/mod.rs"]
mod common;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_loop::{Tools, Turn};
use agent_model::{Message, Part, Role, ToolResultPart};
use agent_tools::{Home, Trash};
use chrono::{DateTime, Duration, TimeZone, Utc};
use common::{model, temp_dir, Step, Wire};
use serde_json::{json, Value};

type Clock = Arc<Mutex<DateTime<Utc>>>;

fn setup(name: &str) -> (PathBuf, Tools, Clock, Trash) {
    let home = temp_dir(&format!("{name}-trash"));
    let clock: Clock = Arc::new(Mutex::new(Utc.with_ymd_and_hms(2027, 1, 15, 8, 0, 0).unwrap()));
    let c = clock.clone();
    let mut tools = Tools::new();
    let trash = Trash::with_clock(Home::new(&home), Arc::new(move || *c.lock().unwrap()));
    trash.add_to(&mut tools);
    (home, tools, clock, trash)
}

async fn call(tools: &Tools, name: &str, args: Value) -> Result<String, String> {
    match tools.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(text.clone()),
            _ => Ok(String::new()),
        },
        Err(e) => Err(e.to_string()),
    }
}

fn field(out: &str, key: &str) -> String {
    let p = format!("- `{key}`: ");
    out.lines().find_map(|l| l.strip_prefix(&p)).unwrap_or_else(|| panic!("{key} in {out}")).to_string()
}

#[tokio::test]
async fn put_list_restore_and_expiry() {
    let (home, tools, clock, trash) = setup("flow");
    assert_eq!(call(&tools, "trash.list", json!({})).await.unwrap(), "No Hamlet trash entries found.");
    std::fs::create_dir_all(home.join("workspace/old")).unwrap();
    std::fs::write(home.join("workspace/old/a.txt"), "a").unwrap();

    let put = call(&tools, "trash.put", json!({"path": "workspace/old"})).await.unwrap();
    let id = field(&put, "trash_id");
    assert!(id.starts_with("20270115-080000-") && id.len() == "20270115-080000-".len() + 32, "{id}");
    assert_eq!(field(&put, "expiry_date"), "2027-02-14T08:00:00Z");
    assert!(put.ends_with("Trashed files and directories are inspectable at `~/.local/share/Trash/hamlet/<trash_id>/files/`."), "{put}");
    assert!(!home.join("workspace/old").exists());
    assert!(home.join(format!(".local/share/Trash/hamlet/{id}/files/old/a.txt")).exists());
    let info: Value = serde_json::from_str(&std::fs::read_to_string(home.join(format!(".local/share/Trash/hamlet/{id}/.trashinfo"))).unwrap()).unwrap();
    assert_eq!(info.as_object().unwrap().len(), 10);
    assert_eq!(info["status"], "active");

    let list = call(&tools, "trash.list", json!({})).await.unwrap();
    assert!(list.starts_with("| trash_id | status | expiry_date | original_path | top_level_entries |\n|---|---|---|---|---|\n"), "{list}");
    assert!(list.contains(&format!("| `{id}` | active | 2027-02-14T08:00:00Z | `")), "{list}");

    // Место занято — восстановление не перезаписывает.
    std::fs::create_dir_all(home.join("workspace/old")).unwrap();
    let e = call(&tools, "trash.restore", json!({"trash_id": id})).await.unwrap_err();
    assert!(e.starts_with("restore conflict: '") && e.ends_with("already exists. Inspect that path, choose a new name or directory, and call restore again with custom_path."), "{e}");
    let r = call(&tools, "trash.restore", json!({"trash_id": id, "custom_path": "workspace/restored/old"})).await.unwrap();
    assert!(field(&r, "restored_path").ends_with("workspace/restored/old"));
    assert!(r.ends_with("The trash metadata remains at `~/.local/share/Trash/hamlet/<trash_id>/.trashinfo` as an audit record."));
    assert_eq!(std::fs::read_to_string(home.join("workspace/restored/old/a.txt")).unwrap(), "a");
    let e = call(&tools, "trash.restore", json!({"trash_id": id})).await.unwrap_err();
    assert_eq!(e, format!("trash_id '{id}' is not restorable because its status is 'restored'"));

    // Через 30 дней запись истекает: содержимое удалено, восстановить нельзя.
    std::fs::write(home.join("workspace/b.txt"), "b").unwrap();
    let id2 = field(&call(&tools, "trash.put", json!({"path": "~/workspace/b.txt"})).await.unwrap(), "trash_id");
    *clock.lock().unwrap() += Duration::days(30);
    // Обращение к корзине само не убирает: это работа уборщика (такт службы).
    assert!(call(&tools, "trash.list", json!({})).await.unwrap().contains(&format!("| `{id2}` | active |")));
    assert_eq!(trash.reap().unwrap(), 1);
    assert_eq!(trash.reap().unwrap(), 0);
    let list = call(&tools, "trash.list", json!({})).await.unwrap();
    assert!(list.contains(&format!("| `{id2}` | expired |")), "{list}");
    assert!(!home.join(format!(".local/share/Trash/hamlet/{id2}/files")).exists());
    assert!(call(&tools, "trash.restore", json!({"trash_id": id2})).await.unwrap_err().contains("status is 'expired'"));

    // Ошибки ядра.
    assert!(call(&tools, "trash.restore", json!({"trash_id": "../x"})).await.unwrap_err().starts_with("invalid trash_id: ../x"));
    let e = call(&tools, "trash.restore", json!({"trash_id": "20270101-000000-ab"})).await.unwrap_err();
    assert!(e.contains("Use the trash.list tool to inspect trash ids."), "{e}");
    assert_eq!(call(&tools, "trash.put", json!({"path": "~/.local/share/Trash/hamlet"})).await.unwrap_err(), "refusing to trash the Hamlet trash directory itself");
    assert_eq!(call(&tools, "trash.put", json!({"path": "  "})).await.unwrap_err(), "path is required for trash action `put`");
    assert!(call(&tools, "trash.put", json!({"path": "nope.txt"})).await.unwrap_err().starts_with("failed to inspect trash entry"));
    assert!(tools.get("trash.put").unwrap().spec().description.starts_with("Move one file or directory into Hamlet's recoverable trash"));
}

async fn one_turn(wire: Wire) {
    let (home, tools, _clock, _trash) = setup(&format!("turn-{wire:?}"));
    std::fs::write(home.join("draft.md"), "x").unwrap();
    let (_server, p) = model(
        wire,
        vec![Step::Calls(vec![("t1", "trash.put", json!({"path": "draft.md"}))]), Step::Calls(vec![("t2", "trash.list", json!({}))]), Step::Text("Убрал в корзину.")],
        false,
    );
    let turn = Turn::new(&p, &tools, "s", home.join(".spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "убери черновик".into() }] }];
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Убрал в корзину.");
    assert!(!home.join("draft.md").exists());
    let listed = &h[4].content[0];
    assert!(matches!(listed, Part::ToolResult { is_error: false, content, .. }
        if matches!(&content[0], ToolResultPart::Text { text } if text.contains("| active |") && text.contains("| draft.md |"))), "{listed:?}");
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}
