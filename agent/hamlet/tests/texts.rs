//! П5-1: тексты главного разговора — блоки ядра `blocks/user_chat/*.md`
//! (101 имя: 88 основных, 9 вариантов с `@` и 4 `environment/*`) и 18
//! `P/hatch-agent/prompts/blocks/chat/*.md`; шаблоны домашних файлов —
//! тела шаблонов ядра (`prompts/SOURCES.md`, раздел П5-1).

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const USER_CHAT: [&str; 101] = [
    "active_work.md",
    "active_work@without_todo_list.md",
    "active_work_blockers.md",
    "active_work_blockers@without_todo_list.md",
    "active_work_finishing.md",
    "active_work_finishing@without_todo_list.md",
    "active_work_updates.md",
    "active_work_updates@without_todo_list.md",
    "artifacts.md",
    "background_results.md",
    "background_results_delivery.md",
    "background_results_silence.md",
    "browser_and_sources.md",
    "browser_and_sources_handoffs.md",
    "browser_and_sources_places.md",
    "captchas.md",
    "commentary_visibility.md",
    "communication_and_delivery.md",
    "context_and_active_work.md",
    "conversation.md",
    "conversation@texting_style.md",
    "date_time_awareness.md",
    "discretion_and_alignment.md",
    "docs.md",
    "due_diligence.md",
    "due_diligence_follow_ups.md",
    "due_diligence_live_information.md",
    "environment.md",
    "environment/connected_accounts.md",
    "environment/deployments.md",
    "environment/secure_vault.md",
    "environment/wallet.md",
    "goals.md",
    "handling_background_handoffs.md",
    "handoff_orientation.md",
    "how_you_work.md",
    "how_you_work_limitation.md",
    "how_you_work_product_questions.md",
    "identifiers_and_dates.md",
    "loading_tools.md",
    "memory.md",
    "memory_baseline.md",
    "memory_correction_baseline.md",
    "memory_recall.md",
    "memory_scope_and_custody.md",
    "memory_search.md",
    "memory_search_baseline.md",
    "offer_options.md",
    "peer_collaboration.md",
    "proactive_work.md",
    "proactivity.md",
    "proactivity_follow_through.md",
    "proactivity_guidance.md",
    "proactivity_shared_preferences.md",
    "purchases.md",
    "recurring_work.md",
    "recurring_work_failures.md",
    "recurring_work_hooks.md",
    "recurring_work_operations.md",
    "recurring_work_recovery.md",
    "recurring_work_results.md",
    "recurring_work_verification.md",
    "retaining_memories.md",
    "retaining_memories@without_todo_list.md",
    "safety_core.md",
    "secure_vault.md",
    "secure_vault_code_from_user.md",
    "secure_vault_credentials.md",
    "secure_vault_entry_card.md",
    "secure_vault_resend.md",
    "security.md",
    "security_anti_phishing.md",
    "security_anti_phishing_tail.md",
    "security_operations.md",
    "security_operations_anti_phishing.md",
    "security_operations_disclosure.md",
    "security_operations_sensitive_values.md",
    "security_secrets.md",
    "security_service_limits.md",
    "service_limits.md",
    "shared_spaces.md",
    "taking_ownership.md",
    "tools.md",
    "tools_approvals.md",
    "tools_confirmation.md",
    "tools_estimates.md",
    "tools_outward_discretion.md",
    "turn_commentary_instructions.md",
    "turn_diligence_reminder.md",
    "turn_diligence_reminder@texting_style.md",
    "turn_diligence_reminder@texting_style_without_todo_list.md",
    "turn_diligence_reminder@without_todo_list.md",
    "turn_memory_search_reminder.md",
    "turn_proactivity_reminder.md",
    "turn_public_commentary_instructions.md",
    "turn_recollection_reminder.md",
    "who_you_are.md",
    "who_you_work_for.md",
    "who_you_work_for_household.md",
    "who_you_work_for_principal.md",
    "working_with_tools.md",
];

const CHAT: [&str; 18] = [
    "date_grounding_developer_message.md",
    "date_time_awareness_head.md",
    "grounding.md",
    "helpfulness_reminder.md",
    "how_you_work.md",
    "identifier_standing_rule.md",
    "identifier_validation.md",
    "memory_recall.md",
    "offer_options.md",
    "ownership_principle.md",
    "plain_language_head.md",
    "prompt_injection_defense.md",
    "security_authority.md",
    "security_sensitive_actions.md",
    "security_social_engineering.md",
    "time_tag_after_input.md",
    "who_you_are.md",
    "who_you_work_for.md",
];

#[test]
fn every_core_block_is_present() {
    for (dir, names) in [("prompts/user_chat", &USER_CHAT[..]), ("prompts/chat", &CHAT[..])] {
        for n in names {
            let p = root().join(dir).join(n);
            let t = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(!t.trim().is_empty(), "{} пуст", p.display());
        }
        let on_disk = walk(&root().join(dir));
        assert_eq!(on_disk.len(), names.len(), "{dir}: лишние файлы");
    }
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// Варианты `@…` — при своём основном блоке.
#[test]
fn variants_have_their_base() {
    for n in USER_CHAT.iter().filter(|n| n.contains('@')) {
        let base = format!("{}.md", n.split('@').next().unwrap());
        assert!(USER_CHAT.contains(&base.as_str()), "{n} без {base}");
    }
}

/// Шаблоны домашних файлов — тела шаблонов ядра (без шапки YAML).
#[test]
fn home_templates_are_core_templates() {
    let home = root().join("agents/hamlet/home");
    let read = |n: &str| std::fs::read_to_string(home.join(n)).unwrap();
    assert!(read("AGENTS.md").starts_with("# AGENTS.md\n\nYour operating manual for this workspace, written by you."));
    assert!(read("SOUL.md").starts_with("# SOUL.md\n\n_You're not a chatbot. You're becoming someone._"));
    assert!(read("IDENTITY.md").starts_with("# IDENTITY.md\n\n_Fill this in as you figure out who you are._"));
    assert!(read("USER.md").starts_with("# USER.md\n\n_What you know about the person you're helping."));
    assert_eq!(read("MEMORY.md"), "# MEMORY.md\n\n<!-- Your curated long-term memory: durable facts, preferences, and commitments. Keep it tight: promote what lasts here, and leave raw day-to-day detail in your daily notes. -->\n\n## Facts\n\n## Preferences\n\n## Commitments\n");
    for n in ["AGENTS.md", "SOUL.md", "IDENTITY.md", "USER.md", "MEMORY.md"] {
        assert!(!read(n).starts_with("---"), "{n}: шапка YAML не снята");
    }
}
