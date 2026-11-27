//! Тексты исполнителя окна по форме ядра (П5-2, часть 4;
//! `prompts/SOURCES.md`): системный текст — профиль 1 (`system.md` +
//! `pages.md`), задание и продолжение — сборка `0x8b1f310` с языком
//! `presentation_language.md` (`0x8b1f920`).

use agent_browser::planner::{assignment_message, system, update_message};
use agent_loop::personal;

fn prompt(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../prompts");
    std::fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn system_is_profile_one() {
    let expected = personal(&format!("{}{}", prompt("browser/system.md"), prompt("browser/pages.md"))).trim_matches('\n').to_string();
    let text = system();
    assert_eq!(text, expected);
    assert!(!text.contains("## Your Assignment"), "instructions/* — срез 1, не выводится");
    assert!(!text.contains("{locale}") && !text.contains("Write everything you author"), "язык — в задании");
}

#[test]
fn assignment_and_update_follow_core() {
    let language = "Write everything you author for people in `de-DE`, including action intents and report summaries, even when the assignment, an earlier run or the page uses another language. Keep names, identifiers, URLs, domains, file names and quoted text exactly as they are.";
    assert_eq!(
        assignment_message("Find the opening hours.", "de-DE"),
        format!("Carry out this assignment end to end, then report the result.\n\nFind the opening hours.\n\nLanguage:\n{language}")
    );
    assert_eq!(
        update_message("Also check Sunday.", "de-DE"),
        format!("Continue this task from the history above with Hamlet's new instruction below, then report the result.\n\nAlso check Sunday.\n\nLanguage:\n{language}")
    );
}
