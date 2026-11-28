//! П5-2, часть 5б: developer-текст хода главного разговора
//! (`agent/hamlet/src/main_prompt.rs`, `turn_texts`; разбор —
//! `prompts/SOURCES.md`, «часть 5б»).

use agent_hamlet::main_prompt::{turn_texts, MAIN_ROLE};
use agent_loop::{personal, Tool, ToolFuture, Tools};
use agent_model::ToolSpec;
use serde_json::{json, Value};

struct Stub(&'static str);
impl Tool for Stub {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.0.into(), description: String::new(), input_schema: json!({"type": "object"}) }
    }
    fn call(&self, _: Value) -> ToolFuture<'_> {
        Box::pin(async { Ok(vec![]) })
    }
}

fn tools(names: &[&'static str]) -> Tools {
    let mut t = Tools::new();
    for n in names {
        t.add(Stub(n));
    }
    t
}

const ALL: [&str; 2] = ["todo.write", "hamlet.memory_search"];

fn text(names: &[&'static str]) -> String {
    let t = turn_texts(&tools(names), MAIN_ROLE);
    assert_eq!(t.len(), 1, "один текст: turn_directives пусто, составной = developer-текст");
    t.into_iter().next().unwrap()
}

fn prompt(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../prompts");
    std::fs::read_to_string(root.join(path)).unwrap()
}

/// Первая непустая строка блока после личного варианта меток.
fn first_line(path: &str) -> String {
    personal(&prompt(path)).lines().find(|l| !l.trim().is_empty()).unwrap().to_string()
}

/// Блоки в порядке среза 1 главного (`0x9904fe`): разделы `c d e 10 11 15 16
/// 17 18 19 1b 1d 1e 1f 22`, внутри — порядок карты раздела.
const ORDER: [&str; 29] = [
    "user_chat/context_and_active_work.md",
    "user_chat/taking_ownership.md",
    "user_chat/memory_search_baseline.md",
    "user_chat/retaining_memories.md",
    "user_chat/memory_correction_baseline.md",
    "user_chat/memory_scope_and_custody.md",
    "user_chat/active_work.md",
    "user_chat/active_work_blockers.md",
    "user_chat/active_work_updates.md",
    "user_chat/active_work_finishing.md",
    "user_chat/due_diligence.md",
    "user_chat/due_diligence_follow_ups.md",
    "user_chat/due_diligence_live_information.md",
    "user_chat/identifiers_and_dates.md",
    "user_chat/security_operations.md",
    "user_chat/security_operations_anti_phishing.md",
    "user_chat/security_operations_disclosure.md",
    "user_chat/security_operations_sensitive_values.md",
    "user_chat/working_with_tools.md",
    "user_chat/service_limits.md",
    "user_chat/browser_and_sources.md",
    "user_chat/browser_and_sources_handoffs.md",
    "user_chat/captchas.md",
    "user_chat/recurring_work_recovery.md",
    "user_chat/communication_and_delivery.md",
    "user_chat/conversation.md",
    "user_chat/background_results.md",
    "user_chat/background_results_silence.md",
    "user_chat/background_results_delivery.md",
];

/// Порядок — по собранному тексту: первая строка каждого блока стоит после
/// первой строки предыдущего.
#[test]
fn sections_follow_profile_order() {
    let t = text(&ALL);
    let mut at = 0;
    for path in ORDER {
        let line = first_line(path);
        let i = t[at..].find(&line).unwrap_or_else(|| panic!("{path}: «{line}» нет или не по порядку")) + at;
        at = i + line.len();
    }
    // Вставка `{offer_options_guidance}` — в конце `taking_ownership`, до
    // памяти.
    let offer = first_line("user_chat/offer_options.md");
    let (o, own, mem) = (t.find(&offer).unwrap(), t.find(&first_line("user_chat/taking_ownership.md")).unwrap(), t.find(&first_line("user_chat/memory_search_baseline.md")).unwrap());
    assert!(own < o && o < mem);
}

/// Блоки целиком о несделанных частях не уходят (строки плана — в SOURCES).
#[test]
fn blocks_of_missing_parts_are_absent() {
    let t = text(&ALL);
    for path in [
        "user_chat/shared_spaces.md",
        "user_chat/browser_and_sources_places.md",
        "user_chat/secure_vault.md",
        "user_chat/secure_vault_entry_card.md",
        "user_chat/secure_vault_credentials.md",
        "user_chat/secure_vault_code_from_user.md",
        "user_chat/secure_vault_resend.md",
        "user_chat/purchases.md",
        "user_chat/artifacts.md",
        "user_chat/proactivity_guidance.md",
        "user_chat/peer_collaboration.md",
        "user_chat/current_conversation.md",
    ] {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../prompts").join(path);
        if !p.exists() {
            continue;
        }
        let line = first_line(path);
        assert!(!t.contains(&line), "{path} не должен уходить");
    }
    // Упоминание Secure Vault внутри общего правила о раскрытии
    // (`security_operations_disclosure`) остаётся — правило П5-1.
    assert!(!t.contains("place_discovery") && !t.contains("PROACTIVE_PREFERENCES"));
}

/// Без переменных, меток и имени muse; края без переводов строки; стык
/// разделов и блоков — пустая строка.
#[test]
fn no_variables_marks_or_muse() {
    let t = text(&ALL);
    assert!(!t.contains('{') && !t.contains('}'), "переменная осталась");
    assert!(!t.contains("<!--"), "метка осталась");
    let lower = t.to_lowercase();
    assert!(!lower.contains("muse"), "muse в тексте");
    assert!(!t.split(|c: char| !c.is_alphanumeric()).any(|w| w == "Meta"), "Meta в тексте");
    assert_eq!(t, t.trim_matches('\n'));
    assert!(t.contains("# Context and Active Work\n\n## Taking Ownership"), "стык разделов");
}

/// Условия веток: память — при `memory_search`/`memory_get`; текущая работа —
/// при `todo.write`; `communication_and_delivery` — только у роли
/// `foreground_root`; варианты `@without_todo_list` — по набору.
#[test]
fn branches_follow_tools_and_role() {
    let mem = first_line("user_chat/memory_search_baseline.md");
    let active = first_line("user_chat/active_work.md");
    let comm = first_line("user_chat/communication_and_delivery.md");
    assert!(!text(&["todo.write"]).contains(&mem));
    assert!(text(&["todo.write", "hamlet.memory_get"]).contains(&mem));
    let no_todo = text(&["hamlet.memory_search"]);
    assert!(!no_todo.contains(&active));
    assert!(no_todo.contains(&first_line("user_chat/retaining_memories@without_todo_list.md")));
    assert!(!no_todo.contains(&first_line("user_chat/retaining_memories.md")) || first_line("user_chat/retaining_memories.md") == first_line("user_chat/retaining_memories@without_todo_list.md"));
    let other = turn_texts(&tools(&ALL), "owner_subagent_depth1").remove(0);
    assert!(!other.contains(&comm) && text(&ALL).contains(&comm));
}
