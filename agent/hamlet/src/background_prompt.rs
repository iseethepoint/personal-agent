//! Системные тексты фоновых ролей по профилям ядра (П5-2, часть 3; разбор —
//! `prompts/SOURCES.md`): работник наблюдателя — профиль 10 (`0x290bc030`,
//! срез 0 `0x99065a`: `7f 7d 3d 7e 7c 335 33b`), проверяющий — профиль 19
//! (`0x290bc228`, `0x49a180`: `326 3d 435 33b`), сброс памяти — профиль 30
//! (`0x290bc490`, `0x49a850`: `9c 3d 735 73b`). Сборка — как у подагента:
//! записи подряд, переменные — по сборщику `0xcdbbdf0`. Раздел `shared/runtime`
//! (`33b`, `73b`) — П8. Окно времени — `crate::temporal`, на момент прогона.

use agent_loop::personal;

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

/// Стек раздела `0x35` (`0xcd52e60`): записи подряд, без краёв, `"\n{}\n\n"`.
fn stack(parts: &[&str]) -> String {
    format!("\n{}\n\n", parts.concat().trim())
}

/// Раздел безопасности фоновых работ (`335`/`435`): правила и стек.
/// `shared/discretion_connections` — `agent-to-agent-only`, П19.
fn detached_safety(r: &mut impl FnMut(&'static str, &'static str, String), section: &'static str, defense: String) {
    r(section, "shared/safety_core_detached", p!("shared/safety_core_detached.md").into());
    r(section, "shared/safety_self_modification_detached", p!("shared/safety_self_modification_detached.md").into());
    r(section, "shared/safety_sensitive_attributes", p!("shared/safety_sensitive_attributes.md").into());
    r(section, "shared/safety_delegated_authority_task", p!("shared/safety_delegated_authority_task.md").into());
    r(section, "shared/safety_credentials_detached_worker", p!("shared/safety_credentials_detached_worker.md").into());
    r(section, "shared/discretion", p!("shared/discretion.md").into());
    r(section, "shared/discretion_user_choice", p!("shared/discretion_user_choice.md").into());
    r(section, "shared/discretion_alignment_task", p!("shared/discretion_alignment_task.md").into());
    r(section, "shared/discretion_trust", p!("shared/discretion_trust.md").into());
    r(section, "shared/safety_security_policy", fill(p!("shared/safety_security_policy.md"), &[("prompt_injection_defense", &defense)]));
}

fn window_record(r: &mut impl FnMut(&'static str, &'static str, String), now_ms: i64, timezone: Option<&str>, exec: bool) {
    r(
        "3d",
        "shared/temporal_awareness_window_only",
        fill(p!("shared/temporal_awareness_window_only.md"), &[("temporal_awareness_window", &crate::temporal::window(now_ms, timezone, exec))]),
    );
}

/// Работник наблюдателя — профиль 10. Стек — `0x35` вариант 3
/// (`0x29054da8`, 12 записей). `exec` — в наборе работника
/// (`execute_resolve_worker`).
pub fn event_hook_records(now_ms: i64, timezone: Option<&str>, exec: bool) -> Vec<Record> {
    let mut out = Vec::new();
    let mut r = |section: &'static str, block: &'static str, text: String| out.push(Record { section, block, text });
    r("7f", "shared/who_built_you", p!("shared/who_built_you.md").into());
    r("7f", "event_hook/who_you_are", p!("hooks/who_you_are.md").into());
    r("7d", "event_hook/how_this_run_works", p!("hooks/how_this_run_works.md").into());
    window_record(&mut r, now_ms, timezone, exec);
    r("7e", "event_hook/tools", p!("hooks/tools.md").into());
    r("7c", "event_hook/filesystem", p!("hooks/filesystem.md").into());
    let defense = stack(&[
        p!("shared/security_authority.md"),
        p!("shared/security_content_markers.md"),
        p!("shared/security_authority_detached_worker.md"),
        p!("shared/security_task_alignment.md"),
        p!("shared/security_secrets.md"),
        p!("shared/security_file_uploads.md"),
        p!("shared/security_gates.md"),
        p!("shared/security_identifiers.md"),
        p!("shared/security_social_engineering.md"),
        p!("agent/security_sensitive_actions.md"),
        p!("agent/security_standing_risk.md"),
        p!("shared/security_precall_checklist.md"),
    ]);
    detached_safety(&mut r, "335", defense);
    out
}

pub fn event_hook(now_ms: i64, timezone: Option<&str>, exec: bool) -> String {
    join(event_hook_records(now_ms, timezone, exec))
}

/// Проверяющий — профиль 19. Задание — `background/verification`
/// (раздел `0x26` вариант 3, `0xd32dc40`); стек — `0x35` вариант 4
/// (`0x29055108`, 12 записей).
pub fn verification_records(now_ms: i64, timezone: Option<&str>, exec: bool) -> Vec<Record> {
    let mut out = Vec::new();
    let mut r = |section: &'static str, block: &'static str, text: String| out.push(Record { section, block, text });
    r("326", "background/verification", p!("background/verification.md").into());
    window_record(&mut r, now_ms, timezone, exec);
    let defense = stack(&[
        p!("shared/security_authority.md"),
        p!("shared/security_content_markers.md"),
        p!("background/security_authority.md"),
        p!("shared/security_task_alignment.md"),
        p!("shared/security_secrets.md"),
        p!("shared/security_file_uploads.md"),
        p!("shared/security_gates.md"),
        p!("shared/security_identifiers.md"),
        p!("shared/security_social_engineering.md"),
        p!("agent/security_sensitive_actions.md"),
        p!("agent/security_standing_risk.md"),
        p!("shared/security_precall_checklist.md"),
    ]);
    detached_safety(&mut r, "435", defense);
    out
}

pub fn verification(now_ms: i64, timezone: Option<&str>, exec: bool) -> String {
    join(verification_records(now_ms, timezone, exec))
}

/// Тексты хода проверяющего (П5-2, часть 5в; `prompts/SOURCES.md`):
/// developer-текст по срезу 1 профиля 19 (`0x9906bc`: `426 b8 ba bc bd bf
/// c1`) в ветках `0xcdbf740` — раздел `0x26` вариант 4 без условия
/// (`0xcdbfffc`, `background/standing_context.md`); `b8` — контекст проекта
/// (`turn.0x33e8`, «root_project»), не опознан; флаговые — пропуск.
/// `date_grounding` нет (`0xc8` в срезе нет), `0x3b` — вариант 3, не
/// выводится; составной текст — без `0x64`/`0xcc`/`0xcd`/`0xce`.
pub fn verification_turn_texts() -> Vec<String> {
    vec![personal(p!("background/standing_context.md")).trim_matches('\n').to_string()]
}

/// Сброс памяти — профиль 30. Стек — `0x35` вариант 7 (`0x29055c00`,
/// 9 записей).
pub fn memory_flush_records(now_ms: i64, timezone: Option<&str>, exec: bool) -> Vec<Record> {
    let mut out = Vec::new();
    let mut r = |section: &'static str, block: &'static str, text: String| out.push(Record { section, block, text });
    r("9c", "shared/who_built_you", p!("shared/who_built_you.md").into());
    r("9c", "memory_flush/who_you_are", p!("memory_flush/who_you_are.md").into());
    window_record(&mut r, now_ms, timezone, exec);
    r("735", "memory_flush/safety", p!("memory_flush/safety.md").into());
    let defense = stack(&[
        p!("shared/security_authority.md"),
        p!("shared/security_content_markers.md"),
        p!("memory_flush/security_authority.md"),
        p!("shared/security_task_alignment.md"),
        p!("shared/security_secrets.md"),
        p!("shared/security_file_uploads.md"),
        p!("shared/security_gates.md"),
        p!("shared/security_identifiers.md"),
        p!("shared/security_precall_checklist.md"),
    ]);
    r("735", "shared/safety_security_policy", fill(p!("shared/safety_security_policy.md"), &[("prompt_injection_defense", &defense)]));
    out
}

pub fn memory_flush(now_ms: i64, timezone: Option<&str>, exec: bool) -> String {
    join(memory_flush_records(now_ms, timezone, exec))
}
