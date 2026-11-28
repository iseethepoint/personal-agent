//! Системный текст главного разговора — сборка из блоков ядра, как
//! `prompt_assembly` muse (`hatch-agent/src/session/impl_session/prompt_assembly/`,
//! `source-files.txt:1365-1394`).
//!
//! Порядок и стыки — как в копии, которую muse пишет при старте
//! (`prompt_assembly/system_prompt_copy.rs`, `source-files.txt:1386`;
//! снимок — `rootfs-full/home/hatch/workspace/system/system_prompt.md`,
//! стандартная обвязка). Тексты верхних блоков лежат в ядре в том же
//! порядке (`templates.txt:704-836`). Перед каждым блоком — свой стык (сколько
//! переводов строки между ним и предыдущим), текст блока — без переводов
//! строки по краям.
//!
//! Условия — по набору инструментов агента, как варианты muse
//! (`@without_todo_list` — когда нет `todo.write`): блок о том, чего в
//! наборе нет, не уходит. Блоки частей, которых у Hamlet ещё нет, не
//! уходят вовсе; их список и строки плана — в `prompts/SOURCES.md`
//! (раздел П5-1, часть 2).

use agent_loop::{personal, Tools};

macro_rules! p {
    ($path:literal) => {
        include_str!(concat!("../../../prompts/", $path))
    };
}

/// Копия системного текста в рабочей папке — путь muse
/// (`blocks/chat/system_prompt_copy_file.md`).
pub const COPY_PATH: &str = "workspace/system/system_prompt.md";

/// Имя инструмента глубокого исследования у Hamlet (`roles.rs`).
const DEEP_RESEARCH_TOOL: &str = "browser_deep_research";

/// Подставить переменные; `trim` — значение без переводов строки в конце.
fn fill(text: &str, vars: &[(&str, &str)], trim: bool) -> String {
    let mut out = text.to_string();
    for (name, value) in vars {
        let v = if trim { value.trim_end_matches('\n') } else { value };
        out = out.replace(&format!("{{{name}}}"), v);
    }
    out
}

struct Out(String);

impl Out {
    /// Блок со своим стыком.
    fn push(&mut self, gap: &str, text: &str) {
        let t = text.trim_matches('\n');
        if t.is_empty() {
            return;
        }
        if !self.0.is_empty() {
            self.0.push_str(gap);
        }
        self.0.push_str(t);
    }
}

/// Системный текст главного разговора для набора `tools`.
pub fn assemble(tools: &Tools) -> String {
    let has = |name: &str| tools.get(name).is_some();
    let mut o = Out(String::new());

    // Кто ты.
    o.push("", &fill(p!("user_chat/who_you_are.md"), &[("thoughtful_follow_through_principle", p!("user_chat/proactivity_follow_through.md"))], false));
    o.push("\n", p!("shared/who_built_you.md"));
    o.push("\n\n", p!("user_chat/who_you_work_for.md"));
    o.push("\n\n", p!("user_chat/who_you_work_for_household.md"));
    o.push("\n\n", p!("user_chat/who_you_work_for_principal.md"));
    o.push("\n\n", p!("user_chat/discretion_and_alignment.md"));
    o.push("\n\n", p!("user_chat/how_you_work.md"));
    o.push("", p!("user_chat/how_you_work_limitation.md"));
    // Справочник о себе (П5-3): место — по копии системного текста.
    o.push("\n\n", p!("user_chat/how_you_work_product_questions.md"));

    // Личное и память.
    o.push("\n", &fill(p!("chat/personalization.md"), &[("alignment_status_pointer", "")], false));
    if has("hamlet.memory_search") {
        o.push("\n\n", &fill(p!("user_chat/memory_recall.md"), &[("user_chat_memory_section", p!("user_chat/memory.md"))], false));
    }

    // Время.
    o.push("\n\n", p!("chat/contextual_awareness.md"));
    o.push("\n\n", p!("user_chat/date_time_awareness.md"));
    o.push("\n\n", p!("chat/timezones_save.md"));
    o.push("\n\n", p!("chat/date_validation_opener.md"));
    o.push(
        "\n\n",
        &fill(p!("chat/date_validation_rest.md"), &[("nearby_dates_reference", ""), ("date_batch_check_guidance", p!("chat/date_batch_check_available.md"))], false),
    );

    // Длинный разговор и текущая работа.
    o.push("\n\n", p!("chat/managing_context_opener.md"));
    o.push("", p!("shared/managing_context_body.md"));
    o.push("\n\n", p!("chat/managing_context_retention.md"));
    if has("chat.read_messages") {
        o.push("\n", p!("chat/managing_context_chat_history.md"));
    }
    if has("todo.write") {
        o.push("\n\n", p!("chat/keeping_track_of_active_work.md"));
    } else {
        o.push("\n\n", p!("chat/keeping_track_of_active_work@without_todo_list.md"));
    }

    // Как писать.
    o.push("\n\n", p!("chat/writing_style.md"));
    o.push("\n", p!("chat/writing_style_follow_ups.md"));
    o.push("\n\n", p!("chat/writing_style_brevity.md"));
    o.push("\n", p!("chat/response_formatting.md"));
    o.push("\n", p!("chat/response_formatting_markdown.md"));
    o.push("", p!("chat/response_formatting_message_breaks.md"));
    o.push("\n", p!("chat/response_formatting_links.md"));
    o.push("", p!("chat/response_formatting_link_labels.md"));
    o.push("\n", p!("chat/response_formatting_citations.md"));

    // Среда: пункты — строками подряд.
    o.push("\n\n\n", p!("user_chat/environment.md"));
    let env = [
        p!("shared/environment/computer.md"),
        p!("shared/environment/browser.md"),
        p!("shared/environment/runtime.md"),
        p!("shared/environment/user_goals.md"),
        p!("shared/environment/tracking.md"),
    ];
    o.push("\n\n", &env.iter().map(|t| t.trim_matches('\n')).collect::<Vec<_>>().join("\n"));

    // Инструменты.
    o.push("\n\n", p!("user_chat/tools.md"));
    o.push("", p!("user_chat/tools_estimates.md"));
    o.push("\n", p!("user_chat/tools_confirmation.md"));
    o.push("\n", p!("user_chat/tools_outward_discretion.md"));
    if has(DEEP_RESEARCH_TOOL) {
        // Своё: правило глубокого исследования у muse — переменная блока
        // `tools_approvals`, которого у Hamlet нет; здесь — отдельным пунктом
        // на его месте.
        o.push("\n", &fill(p!("chat/tool_rules_deep_research.md"), &[("deep_research_tool_name", DEEP_RESEARCH_TOOL)], true));
    }
    if let Some(block) = tools.loading_block() {
        o.push("\n\n", &block);
    }

    // Подагенты.
    if has("subagent.spawn") {
        o.push(
            "\n",
            &fill(
                p!("session/subagents_doctrine.md"),
                &[
                    ("responsive_clause", p!("subagents/chat/responsive_clause.md")),
                    ("overload_consequence", p!("subagents/chat/overload_consequence.md")),
                    ("transcript_fact", p!("subagents/inherited_context/transcript_fact.md")),
                    ("task_content_bullet", p!("subagents/inherited_context/task_content_bullet.md")),
                    ("fanout_bullet", p!("subagents/inherited_context/fanout_bullet.md")),
                    ("extra_delegation_bullet", ""),
                    ("after_spawn_clause", p!("subagents/chat/after_spawn_clause.md")),
                    ("synthesis_sentence", ""),
                ],
                true,
            ),
        );
        o.push("\n\n", &fill(p!("session/subagents_status.md"), &[("asker", "the user"), ("failure_action", "tell the user")], false));
    }

    // Расписания.
    if has("cron.add") {
        o.push("\n\n", p!("user_chat/recurring_work.md"));
        if has("hooks.add") {
            o.push("\n", p!("user_chat/recurring_work_hooks.md"));
        }
        o.push("\n", &fill(p!("user_chat/recurring_work_operations.md"), &[("cron_created_acknowledgement", p!("chat/cron_created_acknowledgement.md"))], true));
        o.push("", p!("user_chat/recurring_work_failures.md"));
        o.push("\n\n", p!("user_chat/recurring_work_results.md"));
        o.push("\n\n", p!("user_chat/recurring_work_verification.md"));
    }

    // Файлы.
    o.push("\n\n\n", p!("chat/workspace_files.md"));
    o.push("\n", p!("chat/runtime_files_list.md"));
    o.push("\n\n", &fill(p!("chat/filesystem.md"), &[("system_prompt_copy_file", p!("chat/system_prompt_copy_file.md"))], false));
    o.push("\n", p!("chat/filesystem_memory.md"));
    o.push("\n\n", p!("chat/filesystem_workspace.md"));
    o.push("\n\n", p!("user_chat/docs.md"));

    // Безопасность.
    o.push("\n", p!("user_chat/security.md"));
    o.push("\n\n", p!("user_chat/security_secrets.md"));
    o.push("\n\n", p!("user_chat/security_service_limits.md"));
    o.push("\n\n", p!("user_chat/security_anti_phishing.md"));
    o.push("\n\n", p!("user_chat/security_anti_phishing_tail.md"));
    o.push("\n\n\n", p!("user_chat/safety_core.md"));
    o.push("\n", p!("chat/safety_oversight.md"));
    o.push("\n", p!("shared/safety_hard_stops.md"));
    o.push("\n", p!("shared/safety_self_modification.md"));
    o.push("\n", p!("shared/safety_sensitive_attributes.md"));
    o.push("\n\n", p!("user_chat/commentary_visibility.md"));

    personal(&o.0)
}

/// Системный текст главного разговора: сборка и дополнения владельца из
/// `system.md` папки агента (своё: у muse такого файла нет; пустой — ничего
/// не добавляет).
pub fn main_system(tools: &Tools, owner: &str) -> String {
    with_owner(assemble(tools), owner)
}

/// Собранный текст и дополнения владельца (`system.md`) за ним — у главного
/// разговора и подагентов.
pub fn with_owner(base: String, owner: &str) -> String {
    if owner.trim().is_empty() {
        base
    } else {
        format!("{base}\n\n{}", owner.trim_matches('\n'))
    }
}

/// Имя роли главного разговора — то, с чем ядро сравнивает роль хода
/// (`ctx.0x360`, `0xcdc0cec`: `foreground_root`); у Hamlet роль главного
/// так и зовётся (`roles.rs`).
pub const MAIN_ROLE: &str = "foreground_root";

/// Раздел developer-текста: блоки по порядку, каждый без переводов строки по
/// краям, пустые выпадают, стык — пустая строка (`0xcd55160` → `0xbff2900`:
/// рендер, `trim`, «\n\n» `0x9e0a89`).
fn section(blocks: &[&str]) -> String {
    blocks.iter().map(|b| b.trim_matches('\n')).filter(|b| !b.is_empty()).collect::<Vec<_>>().join("\n\n")
}

/// Тексты хода главного разговора (П5-2, часть 5б; `prompts/SOURCES.md`).
///
/// `turn_directives` у Hamlet пусто: его заметки присылает отправитель хода,
/// источников нет. Составной текст главного — только developer-текст
/// (`0xcdb9b00`): разделов `0xcf`, `0x64`, `0xcc`, `0xcd`, `0xce` в срезе 1
/// главного нет, `date_grounding` — по разделу `0xc8`, его тоже нет, раздел
/// `0x3b` (`shared/runtime`) — П8.
///
/// Developer-текст — срез 1 главного профиля (`0x9904fe`) по веткам
/// `0xcdbf740`; разделы склеиваются пустой строкой (`0xcdc14ce`). Варианты
/// блоков — по умолчанию (байты хода `0x8ea2`/`0x8eca`/`0x8ee5`/`0x8eb9` = 0,
/// как снимок стандартной обвязки в П5-1); `@without_todo_list` — по набору
/// инструментов, как в системном тексте. Имена инструментов ядро сравнивает
/// без пространства `muse.`, у Hamlet — без `hamlet.`.
pub fn turn_texts(tools: &Tools, role: &str) -> Vec<String> {
    let has = |name: &str| tools.get(name).is_some();
    let todo = has("todo.write");
    let v = |base: &'static str, without: &'static str| if todo { base } else { without };
    let mut out: Vec<String> = Vec::new();

    // `c`, `d` — ветка по умолчанию; `{offer_options_guidance}` (`0xcd53cb0`)
    // при `ctx.0x37a` = 0 — `user_chat/offer_options.md`.
    out.push(section(&[p!("user_chat/context_and_active_work.md")]));
    out.push(section(&[&fill(p!("user_chat/taking_ownership.md"), &[("offer_options_guidance", p!("user_chat/offer_options.md"))], false)]));
    // `e` — при инструменте `memory_search` или `memory_get` (`0xcdc0448`,
    // `0xcdc0e9b`).
    if has("hamlet.memory_search") || has("hamlet.memory_get") {
        out.push(section(&[
            p!("user_chat/memory_search_baseline.md"),
            v(p!("user_chat/retaining_memories.md"), p!("user_chat/retaining_memories@without_todo_list.md")),
            p!("user_chat/memory_correction_baseline.md"),
            p!("user_chat/memory_scope_and_custody.md"),
        ]));
    }
    // `10` — без условия, если флаг `0x3f` не стоит, иначе при `todo.write`
    // (`0xcdc0b43`); флаг не опознан, поэтому — при `todo.write`.
    if todo {
        out.push(section(&[
            p!("user_chat/active_work.md"),
            p!("user_chat/active_work_blockers.md"),
            p!("user_chat/active_work_updates.md"),
            p!("user_chat/active_work_finishing.md"),
        ]));
    }
    // `11` — вариант 0 (вариант 2 — по глобальному `0x29ca27c8`, `0xcdc0c7a`).
    out.push(section(&[
        p!("user_chat/due_diligence.md"),
        p!("user_chat/due_diligence_follow_ups.md"),
        p!("user_chat/due_diligence_live_information.md"),
    ]));
    // `12` — вшитый текст `0xa14c1f` при `ctx.0x37a` (не опознан) — SOURCES.
    out.push(section(&[p!("user_chat/identifiers_and_dates.md")]));
    out.push(section(&[
        p!("user_chat/security_operations.md"),
        p!("user_chat/security_operations_anti_phishing.md"),
        p!("user_chat/security_operations_disclosure.md"),
        p!("user_chat/security_operations_sensitive_values.md"),
    ]));
    // `17`: `shared_spaces` — П17.
    out.push(section(&[p!("user_chat/working_with_tools.md")]));
    out.push(section(&[p!("user_chat/service_limits.md")]));
    // `19`: `browser_and_sources_places` — П20.
    out.push(section(&[p!("user_chat/browser_and_sources.md"), p!("user_chat/browser_and_sources_handoffs.md")]));
    // `1a` (`secure_vault*`) — П18; `1c` (`purchases`) — П20.
    out.push(section(&[p!("user_chat/captchas.md")]));
    out.push(section(&[p!("user_chat/recurring_work_recovery.md")]));
    // `1e` — роль `foreground_root`, вида сессии нет, `cfg.0x72e0` = 0
    // (`0xcdc0cd3`).
    if role == MAIN_ROLE {
        out.push(section(&[p!("user_chat/communication_and_delivery.md")]));
    }
    out.push(section(&[p!("user_chat/conversation.md")]));
    // `2d`, `20` — при `ctx.0x378` (П19); `21` (`artifacts`) — П17.
    // `22`: `proactivity_guidance` — П13.
    out.push(section(&[
        p!("user_chat/background_results.md"),
        p!("user_chat/background_results_silence.md"),
        p!("user_chat/background_results_delivery.md"),
    ]));

    let text = out.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n");
    let text = personal(&text).trim_matches('\n').to_string();
    if text.is_empty() {
        Vec::new()
    } else {
        vec![text]
    }
}
