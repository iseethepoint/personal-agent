//! Тесты П4а: файлы. Инструменты — напрямую, плюс один ход через цикл на
//! обоих поставщиках.

#[path = "../../loop/tests/common/mod.rs"]
mod common;

use std::path::PathBuf;

use agent_loop::{spill, Tools, Turn, SPILL_THRESHOLD_BYTES};
use agent_model::{Message, Part, Role, ToolResultPart};
use agent_tools::{FileTools, Home};
use common::{model, temp_dir, Step, Wire};
use serde_json::{json, Value};

fn setup(name: &str) -> (PathBuf, PathBuf, Tools) {
    let home = temp_dir(&format!("{name}-home"));
    let tmp = temp_dir(&format!("{name}-tmp"));
    let mut tools = Tools::new();
    FileTools::new(Home::new(&home)).add_to(&mut tools);
    (home, tmp, tools)
}

async fn call(tools: &Tools, name: &str, args: Value) -> Result<String, String> {
    match tools.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(text.clone()),
            ToolResultPart::Image(i) => Ok(format!("image {}", i.media_type)),
        },
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::test]
async fn read_offset_continues_spill_file() {
    let (home, _tmp, tools) = setup("spill");
    // Вывод в 300 000 байт: строки с номерами, чтобы видеть стык.
    let full: String = (1..).map(|i| format!("line {i:06} {}\n", "x".repeat(80))).take_while({
        let mut n = 0;
        move |l: &String| {
            n += l.len();
            n <= 300_000
        }
    }).collect();
    let bounded = spill::bound("call_1", vec![ToolResultPart::Text { text: full.clone() }], &home.join(".spill"))
        .await
        .unwrap();
    let ToolResultPart::Text { text: shown } = &bounded[0] else { panic!() };
    let (head, notice) = shown.rsplit_once('\n').unwrap();
    // Путь и offset — ровно из сообщения spill.
    let path = notice.split("path=\"").nth(1).unwrap().split('"').next().unwrap();
    let offset: u64 = notice.split("offset=").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();

    // Читаем страницами, пока muse.read говорит, с какой строки продолжить.
    let mut got = vec![head.to_string()];
    let mut next = Some(offset);
    while let Some(off) = next {
        let page = call(&tools, "hamlet.read", json!({"path": path, "offset": off})).await.unwrap();
        match page.rsplit_once("\n\n[") {
            Some((body, note)) if note.contains("Use offset=") => {
                got.push(body.to_string());
                next = Some(note.split("offset=").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap());
            }
            _ => {
                got.push(page);
                next = None;
            }
        }
    }
    assert!(got.len() >= 3, "должно быть больше одной страницы: {}", got.len());
    // Стык без пропуска и повтора: склейка = весь вывод.
    assert_eq!(format!("{}\n", got.join("\n")), full);
    assert!(got[1].starts_with(&format!("line {offset:06} ")));
    assert!(got[1].len() <= SPILL_THRESHOLD_BYTES);
}

#[tokio::test]
async fn read_limit_and_notes() {
    let (home, _tmp, tools) = setup("limit");
    std::fs::write(home.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    let r = call(&tools, "hamlet.read", json!({"path": "a.txt", "offset": 2, "limit": 2})).await.unwrap();
    assert_eq!(r, "2\n3\n\n[2 more lines in file. Use offset=4 to continue.]");
    assert_eq!(call(&tools, "hamlet.read", json!({"path": "~/a.txt", "offset": 4})).await.unwrap(), "4\n5");
    // Папка — список на два уровня, без скрытых.
    std::fs::create_dir_all(home.join("workspace/sub")).unwrap();
    std::fs::write(home.join(".hidden"), "").unwrap();
    assert_eq!(call(&tools, "hamlet.read", json!({"path": "~"})).await.unwrap(), "a.txt\nworkspace/\n  sub/");
}

#[tokio::test]
async fn write_and_edit() {
    let (home, _tmp, tools) = setup("write");
    call(&tools, "hamlet.write", json!({"path": "workspace/r.md", "content": "один\n"})).await.unwrap();
    call(&tools, "hamlet.write", json!({"path": "workspace/r.md", "content": "два\n", "mode": "append"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(home.join("workspace/r.md")).unwrap(), "один\nдва\n");

    call(&tools, "hamlet.edit", json!({"path": "workspace/r.md", "old_text": "два", "new_text": "три"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(home.join("workspace/r.md")).unwrap(), "один\nтри\n");

    // Перезапись чужого непрочитанного файла — отказ; после чтения — можно.
    std::fs::write(home.join("old.txt"), "старое").unwrap();
    let err = call(&tools, "hamlet.write", json!({"path": "old.txt", "content": "новое"})).await.unwrap_err();
    assert!(err.contains("was not read in this conversation"), "{err}");
    call(&tools, "hamlet.read", json!({"path": "old.txt"})).await.unwrap();
    call(&tools, "hamlet.write", json!({"path": "old.txt", "content": "новое"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(home.join("old.txt")).unwrap(), "новое");
}

#[tokio::test]
async fn edit_without_match_and_with_two() {
    let (home, _tmp, tools) = setup("edit");
    std::fs::write(home.join("e.txt"), "a = 1\nb = 2\na = 1\n").unwrap();
    let err = call(&tools, "hamlet.edit", json!({"path": "e.txt", "old_text": "c = 3", "new_text": "x"})).await.unwrap_err();
    assert!(err.ends_with("e.txt': old_text was not present") && err.starts_with("text not found in '"), "{err}");
    assert_eq!(std::fs::read_to_string(home.join("e.txt")).unwrap(), "a = 1\nb = 2\na = 1\n");

    // Два совпадения: как у muse — заменено первое, остальные названы.
    let out = call(&tools, "hamlet.edit", json!({"path": "e.txt", "old_text": "a = 1", "new_text": "a = 9"})).await.unwrap();
    assert!(out.contains("note: old_text matched 2 locations (lines 1, 3); the first was replaced."), "{out}");
    assert_eq!(std::fs::read_to_string(home.join("e.txt")).unwrap(), "a = 9\nb = 2\na = 1\n");

    let err = call(&tools, "hamlet.edit", json!({"path": "e.txt", "old_text": "", "new_text": "x"})).await.unwrap_err();
    assert!(err.contains("must not be empty"));
}

#[tokio::test]
async fn absolute_and_parent_paths() {
    // Как у muse: абсолютный путь — любой, `..` считается от `~`.
    let (home, tmp, tools) = setup("abs");
    let outside = tmp.join("outside.txt");
    call(&tools, "hamlet.write", json!({"path": outside.to_str().unwrap(), "content": "снаружи"})).await.unwrap();
    assert_eq!(call(&tools, "hamlet.read", json!({"path": outside.to_str().unwrap()})).await.unwrap(), "снаружи");
    call(&tools, "hamlet.edit", json!({"path": outside.to_str().unwrap(), "old_text": "снаружи", "new_text": "правка"}))
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "правка");

    let sibling = home.parent().unwrap().join(format!("{}-sibling.txt", home.file_name().unwrap().to_str().unwrap()));
    let rel = format!("../{}", sibling.file_name().unwrap().to_str().unwrap());
    call(&tools, "hamlet.write", json!({"path": rel, "content": "рядом"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "рядом");
    assert_eq!(call(&tools, "hamlet.read", json!({"path": format!("~/{rel}")})).await.unwrap(), "рядом");
    // Сторож перезаписи узнаёт файл при любом написании пути.
    call(&tools, "hamlet.write", json!({"path": sibling.to_str().unwrap(), "content": "снова"})).await.unwrap();
    std::fs::remove_file(&sibling).unwrap();
}

#[tokio::test]
async fn read_output_capped() {
    let (home, _tmp, tools) = setup("capped");
    let body: String = (0..3000).map(|i| format!("{i:05} {}\n", "y".repeat(60))).collect();
    std::fs::write(home.join("big.txt"), &body).unwrap();
    let r = call(&tools, "hamlet.read", json!({"path": "big.txt"})).await.unwrap();
    let (_, note) = r.rsplit_once("\n\n").unwrap();
    assert!(note.starts_with("[Read output capped at 100KB for this call. Use offset="), "{note}");
    let r = call(&tools, "hamlet.read", json!({"path": "big.txt", "limit": 2900})).await.unwrap();
    let (_, note) = r.rsplit_once("\n\n").unwrap();
    assert!(note.starts_with("[Showing lines 1-") && note.contains("of 3000 (100KB limit). Use offset="), "{note}");
}

#[tokio::test]
async fn edit_reports_closest_match() {
    let (home, _tmp, tools) = setup("closest");
    std::fs::write(home.join("c.rs"), "fn main() {\n    let total = 10;\n    println!(\"{total}\");\n}\n").unwrap();
    let err = call(
        &tools,
        "hamlet.edit",
        json!({"path": "c.rs", "old_text": "    let totl = 10;\n    println!(\"{total}\");", "new_text": "x"}),
    )
    .await
    .unwrap_err();
    assert!(err.contains("old_text was not present\n\nclosest match: similarity 0.98 (threshold 0.80), chars 12..56, lines 2-3"), "{err}");
    assert!(err.contains("\nsnippet:\n    let total = 10;\n    println!(\"{total}\");\nold_text vs closest match:\n--- old_text\n+++ closest match\n@@\n"), "{err}");
    assert!(err.contains("-    let totl = 10;\n+    let total = 10;\n     println!"), "{err}");
    // Совсем непохожее — без ближайшего.
    let err = call(&tools, "hamlet.edit", json!({"path": "c.rs", "old_text": "struct Foo;", "new_text": "x"})).await.unwrap_err();
    assert!(!err.contains("closest match"), "{err}");
}

#[tokio::test]
async fn descriptions_are_source_texts() {
    let (_h, _t, tools) = setup("specs");
    let specs = tools.specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["hamlet.read", "hamlet.write", "hamlet.edit"]);
    assert!(specs[0].description.starts_with("Read file contents or list directory contents."));
    assert!(specs[1].description.ends_with("fails unless the file was successfully read earlier in the conversation."));
    assert!(specs[2].description.starts_with("Edit a file by replacing exact text. `old_text` must match exactly"));
    assert_eq!(
        specs[0].input_schema["properties"]["offset"]["description"],
        "Optional 1-indexed starting line for text files. Defaults to 1."
    );
}

async fn one_turn(wire: Wire) {
    let (home, _tmp, tools) = setup("turn");
    let steps = vec![
        Step::Calls(vec![("c1", "hamlet.write", json!({"path": "~/workspace/note.md", "content": "заметка\n"}))]),
        Step::Calls(vec![("c2", "hamlet.read", json!({"path": "~/workspace/note.md"}))]),
        Step::Text("Записал и прочитал."),
    ];
    let (_server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &tools, "s", home.join(".spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "запиши заметку".into() }] }];
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Записал и прочитал.");
    assert_eq!(std::fs::read_to_string(home.join("workspace/note.md")).unwrap(), "заметка\n");
    let read = &h[4].content[0];
    assert!(matches!(read, Part::ToolResult { is_error: false, content, .. }
        if content == &vec![ToolResultPart::Text { text: "заметка".into() }]), "{read:?}");
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}
