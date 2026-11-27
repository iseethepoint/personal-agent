//! Системный текст подагента — профиль 0 ядра (`0x290bbe00`, срез 0
//! `0x990584`: разделы `43 45 12e 132 39 3c 36 3f 35 42 44 2f 13b`),
//! сборщик `0xcdbbdf0` (таблица заполнения — `prompts/SOURCES.md`, П5-2).
//!
//! Как у ядра: раздел — список записей (`0xa9b9dc0`), запись — текст блока
//! с подставленными переменными (`0x29aacef8`), текст — записи подряд, без
//! разделителей (`0xbff27e0`); стыки — переводы строк самих блоков.
//! Записи без имени — вставки ядра (`"\n"` в разделе `2f`).
//!
//! Блоки о частях, которых у Hamlet нет, не уходят; их список и строки
//! плана — в `prompts/SOURCES.md`. Раздел `13b` (`shared/runtime`,
//! `{runtime_facts}`) — сведения о клетке исполнения, П8.

use agent_loop::{personal, Tools};

macro_rules! p {
    ($path:literal) => {
        include_str!(concat!("../../../prompts/", $path))
    };
}

/// Запись: раздел профиля, имя блока ядра, текст.
pub struct Record {
    pub section: &'static str,
    pub block: &'static str,
    pub text: String,
}

fn fill(text: &str, vars: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (name, value) in vars {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// `{history_inheritance_section}` — `0xcdc26d0`: наследует — `0xa15cd4`;
/// изолирован (глубина > 0) — `0xa15a18`, без своих подагентов — с
/// `0xa15c7c` («You work alone…»).
fn history_inheritance(inherits: bool, can_spawn: bool) -> String {
    if inherits {
        return p!("agent/history_inheritance_inherited.md").to_string();
    }
    let mut s = p!("agent/history_inheritance_isolated.md").to_string();
    if !can_spawn {
        s.push_str(p!("agent/history_inheritance_alone.md"));
    }
    s
}

/// `{subagents_section}` — `0xcd545c0`: учение о подагентах (`0xcd53100`,
/// клаузы `blocks/subagents/subagent/*`, таблица `0x290e09d0`) и
/// `session/subagents_status.md`, формат `"{}\n\n{}"` (`0x385945`), статус —
/// без пробелов по краям (`0x29aa2030`). `extra_delegation_bullet` — пункт
/// «`\n- {}`» (`0x33beeb`) о том, что подагенты не делают артефактов, —
/// пустой: артефактов у Hamlet нет (П17). `synthesis_sentence` — «` {}`»
/// (`0x3639e8`).
fn subagents_section() -> String {
    let doctrine = fill(
        p!("session/subagents_doctrine.md"),
        &[
            ("responsive_clause", p!("subagents/subagent/responsive_clause.md").trim()),
            ("overload_consequence", p!("subagents/subagent/overload_consequence.md").trim()),
            ("transcript_fact", p!("subagents/subagent/transcript_fact.md").trim()),
            ("task_content_bullet", p!("subagents/subagent/task_content_bullet.md").trim()),
            ("fanout_bullet", p!("subagents/subagent/fanout_bullet.md").trim()),
            ("extra_delegation_bullet", ""),
            ("after_spawn_clause", p!("subagents/subagent/after_spawn_clause.md").trim()),
            ("synthesis_sentence", &format!(" {}", p!("subagents/subagent/synthesis_sentence.md").trim())),
        ],
    );
    let status = fill(
        p!("session/subagents_status.md"),
        &[("asker", p!("subagents/subagent/asker.md").trim()), ("failure_action", p!("subagents/subagent/failure_action.md").trim())],
    );
    format!("{}\n\n{}", doctrine.trim(), status.trim())
}

/// `{prompt_injection_defense}` — стек безопасности подагента (`0xcd52e60`,
/// раздел `35` вариант 0 → таблица `0x290546e8`): блоки подряд, без
/// пробелов по краям, в формате `"\n{}\n\n"` (`0x393cc5`).
fn security_stack() -> String {
    let stack = [
        p!("shared/security_authority.md"),
        p!("shared/security_content_markers.md"),
        p!("agent/security_authority.md"),
        p!("shared/security_task_alignment.md"),
        p!("shared/security_secrets.md"),
        p!("shared/security_file_uploads.md"),
        p!("shared/security_gates_credential_handling.md"),
        p!("shared/security_identifiers.md"),
        p!("shared/security_social_engineering.md"),
        p!("agent/security_sensitive_actions.md"),
        p!("agent/security_standing_risk.md"),
        p!("shared/security_precall_checklist_credential_handling.md"),
    ]
    .concat();
    format!("\n{}\n\n", stack.trim())
}

/// Записи системного текста подагента по порядку профиля. `inherits` —
/// подагент видит разговор родителя (у Hamlet — глубина 1); `can_spawn` —
/// у него есть `subagent.*` (они добавляются к началу подсказки отдельно,
/// `spawning_prefix`, в наборе `tools` их нет).
pub fn records(tools: &Tools, inherits: bool, can_spawn: bool) -> Vec<Record> {
    let has = |name: &str| tools.get(name).is_some();
    let mut out = Vec::new();
    let mut r = |section: &'static str, block: &'static str, text: String| out.push(Record { section, block, text });

    // 43 — кто ты.
    r("43", "shared/who_built_you", p!("shared/who_built_you.md").into());
    let history = history_inheritance(inherits, can_spawn);
    r(
        "43",
        "agent/who_you_are",
        fill(p!("agent/who_you_are.md"), &[("subagent_identity_intro", p!("agent/subagent_identity_intro.md")), ("history_inheritance_section", &history)]),
    );

    // 45 — среда. Нет у Hamlet: `agent/environment_chats` (боковые чаты,
    // П6), `agent/environment_feed` (лента, П22), `shared/environment/devices`
    // (устройства, П21); `{artifacts_environment_entry}` пуст (П17).
    r("45", "agent/your_environment_intro", p!("agent/your_environment_intro.md").into());
    r("45", "shared/environment/computer", p!("shared/environment/computer.md").into());
    r("45", "agent/environment_browser", p!("agent/environment_browser.md").into());
    r("45", "shared/environment/runtime", p!("shared/environment/runtime.md").into());
    r("45", "agent/environment_artifacts_slot", fill(p!("agent/environment_artifacts_slot.md"), &[("artifacts_environment_entry", "")]));
    r("45", "shared/environment/user_goals", p!("shared/environment/user_goals.md").into());
    r("45", "shared/environment/tracking", p!("shared/environment/tracking.md").into());
    r("45", "agent/your_requester", fill(p!("agent/your_requester.md"), &[("requester_description", p!("agent/requester_description.md"))]));

    // 12e — время. Метка времени у Hamlet идёт за сообщением
    // (`Conversation::push_user`) — `time_tag_after_input`; соседних дат в
    // заметке нет — `{nearby_dates_reference}` пуст, как у главного.
    r("12e", "chat/contextual_awareness", p!("chat/contextual_awareness.md").into());
    r("12e", "chat/date_time_awareness_head", fill(p!("chat/date_time_awareness_head.md"), &[("time_tag_placement", p!("chat/time_tag_after_input.md").trim())]));
    r("12e", "agent/timezones_trust", p!("agent/timezones_trust.md").into());
    r("12e", "agent/date_validation_opener", p!("agent/date_validation_opener.md").into());
    r(
        "12e",
        "chat/date_validation_rest",
        fill(p!("chat/date_validation_rest.md"), &[("nearby_dates_reference", ""), ("date_batch_check_guidance", p!("chat/date_batch_check_available.md"))]),
    );
    r("12e", "agent/identifier_validation", p!("agent/identifier_validation.md").into());

    // 132 — длинный разговор.
    r("132", "agent/managing_context_opener", p!("agent/managing_context_opener.md").into());
    r("132", "shared/managing_context_body", p!("shared/managing_context_body.md").into());
    r("132", "agent/managing_context_retention", p!("agent/managing_context_retention.md").into());

    // 39 — инструменты. `anthropic/cedar/tools_runtime_pointer` — только при
    // поиске инструментов на стороне поставщика; у Hamlet загрузка своя.
    r("39", "shared/tools_intro", p!("shared/tools_intro.md").into());
    r("39", "shared/tool_rules_reporting_truth", p!("shared/tool_rules_reporting_truth.md").into());
    r("39", "shared/tool_rules_no_time_guesses", p!("shared/tool_rules_no_time_guesses.md").into());
    r("39", "agent/tool_rules", p!("agent/tool_rules.md").into());
    if let Some(t) = tools.client_loading(p!("shared/tools_runtime_pointer.md")) {
        r("39", "shared/tools_runtime_pointer", format!("{t}\n"));
    }

    // 3c — свои подагенты (`0xcdc2560`: пусто без `subagent.spawn`).
    if can_spawn {
        r("3c", "shared/subagents", fill(p!("shared/subagents.md"), &[("subagents_section", &subagents_section())]));
    }

    // 36 — расписания; `{cron_created_acknowledgement}` у подагента пуст
    // (`0xcd54910`: глубина > 0).
    if has("cron.add") {
        r("36", "shared/scheduled_work_mechanics", fill(p!("shared/scheduled_work_mechanics.md"), &[("cron_created_acknowledgement", "")]));
    }

    // 3f — память.
    if has("hamlet.memory_search") {
        r("3f", "agent/memory", p!("agent/memory.md").into());
        r("3f", "agent/memory_recall_opening", p!("agent/memory_recall_opening.md").into());
        r("3f", "shared/memory_recall_doctrine", p!("shared/memory_recall_doctrine.md").into());
        r("3f", "agent/person_records", p!("agent/person_records.md").into());
        r("3f", "agent/memory_read_only", p!("agent/memory_read_only.md").into());
    }

    // 35 — безопасность. `shared/discretion_connections` — только при связи
    // агент — агент (`agent-to-agent-only`), П19.
    r("35", "shared/safety_core_conversational", p!("shared/safety_core_conversational.md").into());
    r("35", "agent/safety_oversight", p!("agent/safety_oversight.md").into());
    r("35", "shared/safety_self_modification", p!("shared/safety_self_modification.md").into());
    r("35", "shared/safety_sensitive_attributes", p!("shared/safety_sensitive_attributes.md").into());
    r("35", "shared/safety_delegated_authority_requester", p!("shared/safety_delegated_authority_requester.md").into());
    r("35", "agent/safety_credentials", p!("agent/safety_credentials.md").into());
    r("35", "shared/safety_credentials_transient_limits", p!("shared/safety_credentials_transient_limits.md").into());
    r("35", "shared/discretion", p!("shared/discretion.md").into());
    r("35", "shared/discretion_user_choice", p!("shared/discretion_user_choice.md").into());
    r("35", "shared/discretion_alignment_task", p!("shared/discretion_alignment_task.md").into());
    r("35", "shared/discretion_trust", p!("shared/discretion_trust.md").into());
    r("35", "shared/safety_security_policy", fill(p!("shared/safety_security_policy.md"), &[("prompt_injection_defense", &security_stack())]));

    // 42 — итог.
    r("42", "agent/reporting_back", p!("agent/reporting_back.md").into());

    // 44 — файлы дома (сами файлы — `Prefix::assemble`).
    r("44", "agent/workspace_files", p!("agent/workspace_files.md").into());
    r("44", "agent/runtime_files_list", p!("agent/runtime_files_list.md").into());

    // 2f — папки; `{system_prompt_copy_file}` у агентов пуст (`0xcdbbdf0`).
    r("2f", "chat/filesystem", fill(p!("chat/filesystem.md"), &[("system_prompt_copy_file", "")]));
    r("2f", "chat/filesystem_memory", p!("chat/filesystem_memory.md").into());
    r("2f", "", "\n".into());
    r("2f", "chat/filesystem_workspace", p!("chat/filesystem_workspace.md").into());
    r("2f", "", "\n".into());

    out
}

/// Системный текст подагента: записи подряд, личный вариант меток. Края —
/// без переводов строки (своё, как у главного разговора).
pub fn assemble(tools: &Tools, inherits: bool, can_spawn: bool) -> String {
    let text: String = records(tools, inherits, can_spawn).into_iter().map(|r| r.text).collect();
    personal(&text).trim_matches('\n').to_string()
}
