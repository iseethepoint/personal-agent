//! Системные тексты работника расписаний: фаза исполнения — профиль 22
//! ядра (`0x290bc2d0`, срез 0 `0x99083a`: `96 95 97 12e 41 339 3c 40 235 133
//! 44 2f 43b`), фаза итога — профиль 23 (`0x290bc308`, срез 0 `0x990854`:
//! `9b 99 98 9a 835 63b`). Сборка — как у подагента (`subagent_prompt`):
//! записи подряд, переменные — по сборщику `0xcdbbdf0` (таблица —
//! `prompts/SOURCES.md`, П5-2 часть 2). Разделы `43b`/`63b` (`shared/runtime`) —
//! П8.

use agent_loop::{personal, Tools};

use crate::subagent_prompt::Record;

macro_rules! p {
    ($path:literal) => {
        include_str!(concat!("../../../prompts/", $path))
    };
}

fn fill(text: &str, vars: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (name, value) in vars {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

fn join(records: Vec<Record>) -> String {
    let text: String = records.into_iter().map(|r| r.text).collect();
    personal(&text).trim_matches('\n').to_string()
}

/// `{prompt_injection_defense}` работника — раздел `0x33` вариант 1
/// (`0xcd52e60`: таблица `0x290ebc58`[1] = `0x29054388`, 12 записей): блоки
/// подряд, без пробелов по краям, без обёртки (она — только у `0x35`).
fn worker_security_stack() -> String {
    [
        p!("shared/security_authority.md"),
        p!("shared/security_content_markers.md"),
        p!("chat/security_authority.md"),
        p!("shared/security_task_alignment.md"),
        p!("shared/security_secrets.md"),
        p!("shared/security_file_uploads.md"),
        p!("shared/security_gates_credential_handling.md"),
        p!("shared/security_identifiers.md"),
        p!("agent/security_social_engineering_worker.md"),
        p!("agent/security_sensitive_actions_worker.md"),
        p!("agent/security_standing_risk_worker.md"),
        p!("shared/security_precall_checklist_credential_handling.md"),
    ]
    .concat()
    .trim()
    .to_string()
}

/// Записи фазы исполнения по порядку профиля 22.
pub fn worker_records(tools: &Tools) -> Vec<Record> {
    let has = |name: &str| tools.get(name).is_some();
    let mut out = Vec::new();
    let mut r = |section: &'static str, block: &'static str, text: String| out.push(Record { section, block, text });

    // 96 — кто ты.
    r("96", "shared/who_built_you", p!("shared/who_built_you.md").into());
    r("96", "scheduler_worker/who_you_are", p!("scheduler_worker/who_you_are.md").into());
    // 95 — как кончается прогон.
    r("95", "scheduler_worker/how_this_run_ends", p!("scheduler_worker/how_this_run_ends.md").into());

    // 97 — среда. Нет у Hamlet: `agent/environment_chats` (П6),
    // `agent/environment_feed` (П22); `{artifacts_environment_entry}` пуст
    // (П17). Браузер работника — `browser.browse` и окна; у работника
    // Hamlet их нет (П4к-3) — пункт только при `browser.browse`.
    r("97", "agent/your_environment_intro", p!("agent/your_environment_intro.md").into());
    r("97", "shared/environment/computer", p!("shared/environment/computer.md").into());
    if has("browser.browse") {
        r("97", "agent/environment_browser_worker", p!("agent/environment_browser_worker.md").into());
    }
    r("97", "agent/environment_runtime_worker", p!("agent/environment_runtime_worker.md").into());
    r("97", "agent/environment_artifacts_slot", fill(p!("agent/environment_artifacts_slot.md"), &[("artifacts_environment_entry", "")]));
    r("97", "shared/environment/user_goals", p!("shared/environment/user_goals.md").into());
    r("97", "shared/environment/tracking", p!("shared/environment/tracking.md").into());
    if has("cron.add") {
        r("97", "agent/environment_scheduled_work_worker", p!("agent/environment_scheduled_work_worker.md").into());
    }
    r("97", "", "\n".into());

    // 12e — время: как у подагента; `exec` у работника есть —
    // `{nearby_dates_reference}` пуст (`0xcd52990`), проверка дат —
    // `date_batch_check_available`.
    r("12e", "chat/contextual_awareness", p!("chat/contextual_awareness.md").into());
    r("12e", "chat/date_time_awareness_head", fill(p!("chat/date_time_awareness_head.md"), &[("time_tag_placement", p!("chat/time_tag_before_input.md").trim())]));
    r("12e", "agent/timezones_trust", p!("agent/timezones_trust.md").into());
    r("12e", "agent/date_validation_opener", p!("agent/date_validation_opener.md").into());
    r(
        "12e",
        "chat/date_validation_rest",
        fill(p!("chat/date_validation_rest.md"), &[("nearby_dates_reference", ""), ("date_batch_check_guidance", p!("chat/date_batch_check_available.md"))]),
    );
    r("12e", "agent/identifier_validation", p!("agent/identifier_validation.md").into());

    // 41 — длинный прогон.
    r("41", "agent/managing_context_opener_worker", p!("agent/managing_context_opener_worker.md").into());
    r("41", "shared/managing_context_body", p!("shared/managing_context_body.md").into());
    r("41", "agent/managing_context_retention_worker", p!("agent/managing_context_retention_worker.md").into());

    // 339 — инструменты.
    r("339", "shared/tools_intro", p!("shared/tools_intro.md").into());
    r("339", "shared/tool_rules_reporting_truth", p!("shared/tool_rules_reporting_truth.md").into());
    r("339", "shared/tool_rules_no_time_guesses", p!("shared/tool_rules_no_time_guesses.md").into());
    r("339", "shared/tool_rules_detached_approvals", p!("shared/tool_rules_detached_approvals.md").into());
    r("339", "agent/tool_rules_worker", p!("agent/tool_rules_worker.md").into());
    if let Some(t) = tools.client_loading(p!("shared/tools_runtime_pointer.md")) {
        r("339", "shared/tools_runtime_pointer", format!("{t}\n"));
    }

    // 3c — `{subagents_section}`: `0xcdc2560` — без `subagent.spawn` пусто;
    // в наборе работника Hamlet подагентов нет, раздел не уходит (форма корня
    // с подагентами — `prompts/SOURCES.md`).

    // 40 — память.
    if has("hamlet.memory_search") {
        r("40", "agent/memory", p!("agent/memory.md").into());
        r("40", "agent/memory_recall_opening", p!("agent/memory_recall_opening.md").into());
        r("40", "shared/memory_recall_doctrine", p!("shared/memory_recall_doctrine.md").into());
        r("40", "agent/person_records", p!("agent/person_records.md").into());
        r("40", "agent/memory_read_only_worker", p!("agent/memory_read_only_worker.md").into());
    }

    // 235 — безопасность. `shared/discretion_connections` — `agent-to-agent-only`, П19.
    r("235", "shared/safety_core_conversational", p!("shared/safety_core_conversational.md").into());
    r("235", "agent/safety_oversight_detached", p!("agent/safety_oversight_detached.md").into());
    r("235", "shared/safety_self_modification", p!("shared/safety_self_modification.md").into());
    r("235", "shared/safety_sensitive_attributes", p!("shared/safety_sensitive_attributes.md").into());
    r("235", "agent/safety_credentials_worker", p!("agent/safety_credentials_worker.md").into());
    r("235", "shared/safety_credentials_transient_limits", p!("shared/safety_credentials_transient_limits.md").into());
    r("235", "shared/discretion", p!("shared/discretion.md").into());
    r("235", "shared/discretion_user_choice", p!("shared/discretion_user_choice.md").into());
    r("235", "shared/discretion_alignment_task", p!("shared/discretion_alignment_task.md").into());
    r("235", "shared/discretion_trust", p!("shared/discretion_trust.md").into());

    // 133 — стек безопасности.
    r("133", "chat/prompt_injection_defense", fill(p!("chat/prompt_injection_defense.md"), &[("prompt_injection_defense", &worker_security_stack())]));

    // 44 — файлы дома.
    r("44", "agent/workspace_files", p!("agent/workspace_files.md").into());
    r("44", "agent/runtime_files_list", p!("agent/runtime_files_list.md").into());

    // 2f — папки.
    r("2f", "chat/filesystem", fill(p!("chat/filesystem.md"), &[("system_prompt_copy_file", "")]));
    r("2f", "chat/filesystem_memory", p!("chat/filesystem_memory.md").into());
    r("2f", "", "\n".into());
    r("2f", "chat/filesystem_workspace", p!("chat/filesystem_workspace.md").into());
    r("2f", "", "\n".into());
    out
}

/// Системный текст фазы исполнения.
pub fn worker(tools: &Tools) -> String {
    join(worker_records(tools))
}

/// `{temporal_awareness_window}` — `0xcdc2380` → `0xcdbf080`, шаблон
/// `0xa147a9`: строка года (`0x2e6831` + `"\n\n"` `0x414d26`; дата — в поясе,
/// без пояса — `UTC`, `0x26a616f2`), правило поясов
/// `shared/temporal_guidance_detached` (без краёв), остальное — по `exec`:
/// с ним — `0xa14246` и `0xa14448`, соседних дат нет.
fn temporal_awareness_window(now_ms: i64, timezone: Option<&str>) -> String {
    use chrono::{Datelike, TimeZone};
    let tz: chrono_tz::Tz = timezone.and_then(|t| t.parse().ok()).unwrap_or(chrono_tz::UTC);
    let year = tz.timestamp_millis_opt(now_ms).single().map(|t| t.year());
    let anchor = year.map(|y| format!("The year is {y}, not {}.\n\n", y - 1)).unwrap_or_default();
    let guidance = personal(p!("shared/temporal_guidance_detached.md"));
    fill(
        p!("shared/temporal_awareness_window.md"),
        &[
            ("year_anchor", &anchor),
            ("temporal_guidance", guidance.trim()),
            ("nearby_dates", ""),
            ("weekday_clause", p!("shared/temporal_weekday_clause_exec.md")),
            ("weekday_rule", p!("shared/temporal_weekday_rule_exec.md")),
        ],
    )
}

/// `{prompt_injection_defense}` фазы итога — раздел `0x35` вариант 8
/// (`0x29055e88`, 9 записей), формат `"\n{}\n\n"` (`0x393cc5`).
fn resolution_security_stack() -> String {
    let stack = [
        p!("shared/security_authority.md"),
        p!("shared/security_content_markers.md"),
        p!("scheduler_resolution/security_authority.md"),
        p!("shared/security_task_alignment.md"),
        p!("shared/security_secrets.md"),
        p!("shared/security_file_uploads.md"),
        p!("shared/security_gates.md"),
        p!("shared/security_identifiers.md"),
        p!("shared/security_precall_checklist.md"),
    ]
    .concat();
    format!("\n{}\n\n", stack.trim())
}

/// Записи фазы итога по порядку профиля 23. Окно времени — с `exec`: он в
/// роли `scheduler_cron_resolve`. `scheduler_resolution/tools` («The full
/// tool list … is in the Runtime section below») не уходит: раздела
/// `shared/runtime` у Hamlet нет (П8).
pub fn resolution_records(now_ms: i64, timezone: Option<&str>) -> Vec<Record> {
    let mut out = Vec::new();
    let mut r = |section: &'static str, block: &'static str, text: String| out.push(Record { section, block, text });
    r("9b", "shared/who_built_you", p!("shared/who_built_you.md").into());
    r("9b", "scheduler_resolution/who_you_are", p!("scheduler_resolution/who_you_are.md").into());
    r("99", "scheduler_resolution/tool_call_style", p!("scheduler_resolution/tool_call_style.md").into());
    r(
        "98",
        "scheduler_resolution/temporal_awareness",
        fill(p!("scheduler_resolution/temporal_awareness.md"), &[("temporal_awareness_window", &temporal_awareness_window(now_ms, timezone))]),
    );
    r("835", "scheduler_resolution/safety", p!("scheduler_resolution/safety.md").into());
    r("835", "shared/safety_security_policy", fill(p!("shared/safety_security_policy.md"), &[("prompt_injection_defense", &resolution_security_stack())]));
    out
}

/// Системный текст фазы итога на момент `now_ms`.
pub fn resolution(now_ms: i64, timezone: Option<&str>) -> String {
    join(resolution_records(now_ms, timezone))
}
