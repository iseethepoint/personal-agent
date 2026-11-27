# Откуда взяты тексты `prompts/`

Рабочий документ: модель его не видит. Для каждого файла указаны исходник в `iseethepoint/muse-live` (HEAD `8ab06ae59a`) и правки. Путь `P/` означает `rootfs-full/opt/hatch-private/prompts/hatch-engine/crates/`.

Общая правка всех файлов (П3д): метки исходника `<!-- personal-muse-only:… -->` и `<!-- group-muse-only:… -->` переименованы в `<!-- personal-only:… -->` и `<!-- group-only:… -->`. Модели уходит личный вариант текста, метки срезает `agent_loop::personal`. При переносе нового текста метки переименовываются так же.

| Файл | Исходник | Правки, кроме меток |
|---|---|---|
| `compaction/memory_flush_prompt.md` | `P/hatch-agent/prompts/session/memory_flush_prompt.md` | `muse.write` → `hamlet.write` (П10) |
| `compaction/summarization.md` | `P/hatch-compaction/prompts/compact/summarization.md` | нет |
| `compaction/summary_rules.md` | `P/hatch-compaction/prompts/compact/summary_rules.md` | нет |
| `compaction/system.md` | `P/hatch-compaction/prompts/compact/system.md` | нет |
| `subagent/active_turn_subagent_history_developer_message.md` | `P/hatch-agent/prompts/session/active_turn_subagent_history_developer_message.md` | нет (П3д) |
| `subagent/subagent_context_parent_background.md` | `P/hatch-agent/prompts/session/subagent_context_parent_background.md` | нет (П3д) |
| `subagent/subagent_context_persistent_session.md` | `P/hatch-agent/prompts/session/subagent_context_persistent_session.md` | нет |
| `subagent/subagent_context_standard.md` | `P/hatch-agent/prompts/session/subagent_context_standard.md` | нет |
| `subagent/subagent_context_user_message.md` | `P/hatch-agent/prompts/session/subagent_context_user_message.md` | нет |
| `subagent/subagent_history_inheritance_inherited.md` | `P/hatch-agent/prompts/session/subagent_history_inheritance_inherited.md` | убран второй пункт «Any subagents you spawn deeper than 1 start in isolated mode…»: у нас подагенты глубже не спавнят (`MAX_SPAWN_DEPTH` = 1) (П3д) |
| `cron/worker_instruction.md` | `P/hatch-cron/prompts/scheduler/worker_instruction.md` | П4е-1: в абзаце о фазе исполнения предложения об артефактах и `chat.send_message`/`chat.read_messages` заменены одним «The run's result goes through resolution, not through a chat message.»; убран абзац о `device.read_history` (SMS, iMessage) — таких инструментов у Hamlet нет |
| `cron/worker_phase_fence.md` | `P/hatch-cron/prompts/scheduler/worker_phase_fence.md` | П4е-1: убраны предложения об артефактах и `chat.send_message`; «a widget or a public reply» → «a public reply» (виджетов нет) |
| `cron/worker_schedule_change_rules_user.md` | `P/hatch-cron/prompts/scheduler/worker_schedule_change_rules_user.md` | нет (П4е-1) |
| `cron/cron_resolution_contract.md` | `P/hatch-cron/prompts/scheduler/cron_resolution_contract.md` | П4е-1: `muse.nothing_to_report`, `muse.notify_main_agent` → `hamlet.*`; «main agent's widget or public reply» → «main agent's public reply» |
| `cron/cron_unresolved_execution.md` | `P/hatch-cron/prompts/tools/cron_unresolved_execution.md` | `muse.*` → `hamlet.*` (П4е-1) |
| `cron/cron_incomplete_execution.md` | `P/hatch-cron/prompts/tools/cron_incomplete_execution.md` | `muse.*` → `hamlet.*` (П4е-1) |
| `cron/main_agent_handoff_intro.md` | ядро: `core-v2-analysis/strings-all.txt:84914-84917` (`cron/scheduler/main_agent_handoff_intro.md`, файла на диске нет) | нет (П4е-1) |
| `cron/cron_namespace.md` | ядро: описание пространства `cron`, вариант A, `strings-all.txt:85084-85109` | П4е-1: убраны первые два абзаца (владелец задания `owner`, цели и их папки — придут с целями, П4ж); в абзаце о копиях убраны `workspace/goals/<goal-slug>/crons/` и `workspace/system-cron.d/`, «Writing a note, saving a preference to memory, or editing a goal description» → «Writing a note or saving a preference to memory»; убрано предложение о системных заданиях; убран хвост «a one-shot reminder's tracked item closes…»; `action=list` → `cron.list`; убраны предложения об ожидающих согласованиях («An older occurrence safely parked on an unanswered approval…»), о записанном восстановлении и блокировках («Check its recorded recovery and blockers…»), абзац о `delivery_chat_id` и `vm_authentication` — этих служб у Hamlet нет |
| `cron/tools.json` | ядро: описания инструментов `cron.*` (`strings-all.txt:85138-85167`), полей `CronArgs` (`:85141-85184`), ответов и проверок (`:84897-84898`, `:84932`, `:85201`) | П4е-1: собрано из выписок ядра; `cron.stop` — потерянные в разборе тире восстановлены, «its worker, subagents, and browser windows included» → «its worker and subagents included», «notifies the job's chat that the run was stopped» → «tells the main agent that the run was stopped»; `body` — убраны предложения про `execution.kind`, чужие чаты и «Trusted»; `schedule` — «For execution.kind=agent, a condition…» → «A condition…», убрано предложение о `space_action`; `schedule.timezone` — из личного варианта убраны `@user.home`/`@user.work` и `user/timezones.yaml` (у нас только `@user.current`) и хвост «prefer the symbolic ref for home/work…». Поля `delivery`, `connection_id`, `execution`, `predicted_connector_permissions`, `owner`, `timing`, `window` не взяты |

Удалён (П3д): `subagent/subagent_history_inheritance_isolated.md`. У muse изолированы только подагенты глубже первого, у нас таких нет.

Проверка «нет»: исходник после переименования меток совпадает с файлом байт в байт, `cmp` (27.11.2026).

Тексты, встроенные в код (описания инструментов, задания подагентам, заметки), записаны в комментарии рядом с константой: строка ядра и правки. Тест `agent/tools/tests/names.rs` проверяет, что в текстах, которые уходят модели, нет имён `muse`, `Muse` и `Meta`.
