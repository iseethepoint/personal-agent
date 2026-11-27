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
| `cron/cron_namespace.md` | ядро: описание пространства `cron`, вариант A, `strings-all.txt:85084-85109` | П4е-1: в абзаце о копиях убран `workspace/system-cron.d/` (первые два абзаца, путь `workspace/goals/<goal-slug>/crons/` и «editing a goal description» убирались в П4е-1 и возвращены в П4ж); предложение о системных заданиях убрано в П4е-1 и возвращено в П4е-2 (пульс); убран хвост «a one-shot reminder's tracked item closes…»; `action=list` → `cron.list`; убраны предложения об ожидающих согласованиях («An older occurrence safely parked on an unanswered approval…»), о записанном восстановлении и блокировках («Check its recorded recovery and blockers…»), абзац о `delivery_chat_id` и `vm_authentication` — этих служб у Hamlet нет |
| `cron/tools.json` | ядро: описания инструментов `cron.*` (`strings-all.txt:85138-85167`), полей `CronArgs` (`:85141-85184`), ответов и проверок (`:84897-84898`, `:84932`, `:85201`) | П4е-1: собрано из выписок ядра; `cron.stop` — потерянные в разборе тире восстановлены, «its worker, subagents, and browser windows included» → «its worker and subagents included», «notifies the job's chat that the run was stopped» → «tells the main agent that the run was stopped»; `body` — убраны предложения про `execution.kind`, чужие чаты и «Trusted»; `schedule` — «For execution.kind=agent, a condition…» → «A condition…», убрано предложение о `space_action`; `schedule.timezone` — из личного варианта убраны `@user.home`/`@user.work` и `user/timezones.yaml` (у нас только `@user.current`) и хвост «prefer the symbolic ref for home/work…». Поля `delivery`, `connection_id`, `execution`, `predicted_connector_permissions`, `owner`, `timing`, `window` не взяты |
| `cron/worker_schedule_change_rules_system.md` | `P/hatch-cron/prompts/scheduler/worker_schedule_change_rules_system.md` | нет (П4е-2) |
| `hooks/who_you_are.md` | `P/hatch-agent/prompts/blocks/event_hook/who_you_are.md` | нет (П4е-2) |
| `hooks/how_this_run_works.md` | `P/hatch-agent/prompts/blocks/event_hook/how_this_run_works.md` | `muse.notify_main_agent`, `muse.nothing_to_do` → `hamlet.*` (П4е-2) |
| `hooks/tools.md` | `P/hatch-agent/prompts/blocks/event_hook/tools.md` | нет (П4е-2) |
| `hooks/nothing_to_do_description.md` | `P/hatch-agent/prompts/tools/nothing_to_do_without_reaction_description.md` | нет (П4е-2) |
| `hooks/tools.json` | ядро: описание пространства `hooks`, инструментов `hooks.*` и полей `HooksArgs` (`strings-all.txt:85287`), ответы `hooks.add`/`dry_run` (`:84899`), задание работника (`:178084-178094`) | П4е-2: собрано из выписок ядра; `HATCH_HOOK_*` → `HAMLET_HOOK_*`; в описании пространства убраны «and proxy setup» (прокси у Hamlet нет), путь `workspace/goals/<goal-slug>/files/` (цели — П4ж) и предложение о `delivery` (один разговор — главный, П6); `muse.notify_main_agent`, `muse.nothing_to_do` → `hamlet.*`. Поля `delivery`, `connection_id` не взяты; описание `script_timeout_secs` в разборе не найдено — своё, в коде |

Библиотека скриптов наблюдателей `agent/cron/runtime/hamlet_hook_runtime.sh` — `rootfs-full/home/hatch/hooks/runtime/hatch_hook_runtime.sh`; правки: `HATCH_HOOK_*` → `HAMLET_HOOK_*`, `_hatch_hook_*` → `_hamlet_hook_*`, метки вывода `HATCH_HOOK_RESULT:` / `HATCH_HOOK_LOG:` → `HAMLET_HOOK_RESULT:` / `HAMLET_HOOK_LOG:` (П4е-2).
| `cron/goal_context.md` | `P/hatch-cron/prompts/scheduler/goal_context.md` | П4ж: убрано «Published web artifacts remain in the web artifact system.» (артефактов у Hamlet нет) |
| `cron/goal_record_user_goal.md` | `P/hatch-cron/prompts/scheduler/goal_record_user_goal.md` | нет (П4ж) |
| `cron/goal_record_tracking.md` | `P/hatch-cron/prompts/scheduler/goal_record_tracking.md` | нет (П4ж) |
| `hooks/filesystem.md` | `P/hatch-agent/prompts/blocks/event_hook/filesystem.md` | П4ж: `muse.read/write/edit` → `hamlet.*`; убрано «(`/home/hatch`)» — дом у Hamlet другой |
| `goals/activity_current_state.md` | `P/hatch-agent/prompts/goals/activity_current_state.md` | `muse.replace_current_state`, `muse.no_new_state` → `hamlet.*` (П4ж) |
| `goals/subgoal_activity_rollup.md` | `P/hatch-agent/prompts/goals/subgoal_activity_rollup.md` | `muse.log_parent_activity` → `hamlet.*` (П4ж) |
| `goals/attention.md` | `P/hatch-server/prompts/goals/attention.md` | `muse.set_attention` → `hamlet.*`; метки `personal-muse-only`/`group-muse-only` → `personal-only`/`group-only` (П4ж-2б). Описание и поля инструмента `set_attention` — строки ядра, в коде (`agent/goals/src/activity.rs`) |
| `goals/tracking_lifecycle_completion_system.md` | `P/hatch-server/prompts/goals/tracking_lifecycle_completion_system.md` | `muse.set_tracking_goal_lifecycle_copy` → `hamlet.*` (П4ж-2б) |
| `goals/tracking_lifecycle_reopen_system.md` | `P/hatch-server/prompts/goals/tracking_lifecycle_reopen_system.md` | `muse.*` → `hamlet.*`, метки `*-muse-only` → `*-only` (П4ж-2б) |
| `goals/tracking_lifecycle_copy_input.md` | `P/hatch-server/prompts/goals/tracking_lifecycle_copy_input.md` | без правок (П4ж-2б) |
| `goals/tools.json` | ядро: пространства `user_goal` (`strings-all.txt:57858-57917`) и `tracking` (`:57918-57968`), инструменты и поля (`:58190-58222`, `:332241-332264`), ответы и ошибки (`:15487`, `:21510`, `:22645-22647`, `:28649-28653`, `:32387`, `:54216`, `:54232`, `:54792`, `:55513`, `:55730-55736`, `:150770`, `:198443-198502`, `:227341`); байты — из ядра по смещениям | П4ж: собрано скриптом из ядра; убрана скобка про `muse.create_options` (в описании `user_goal.create` и в пространстве `user_goal`); `/opt/muse/skills/` → `~/skills/`; `host_muse` → `host_hamlet`, «Muse ID» → «Hamlet ID» (общие цели — в `extra`, в инструменты не идут); абзац о Letters/`pushable` вынесен из пространства `user_goal` в `extra.namespace.letters`; `goal_not_exists` — без литерала «goal_id », его добавляет код. В `extra` лежат и не взятые поля: общие цели, `watch`/`requested`/`connection_id`/`follow_up_at` |

`cron/cron_namespace.md` в П4ж: возвращены первые два абзаца (владелец `owner`, цели), пути `workspace/goals/<goal-slug>/crons/` и «editing a goal description». В `cron/tools.json` добавлено поле `owner` (ядро, «#### owner»): убраны `space:<space-slug>` и «or web artifact» — владельцев-артефактов у Hamlet нет.
| `subagent_monitor/system.md` | `P/hatch-agent/prompts/blocks/activity_monitor/subagent_monitor.md` | нет (П4ж-2) |
| `subagent_monitor/subagents_monitoring.md` | `P/hatch-activity/prompts/activity_monitor/subagents_monitoring.md` — текст по умолчанию для `~/SUBAGENTS_MONITORING.md` | нет (П4ж-2) |
| `subagent_monitor/empty_subagents_monitoring.md` | `P/hatch-agent/prompts/activity_monitor/empty_subagents_monitoring.md` | нет (П4ж-2) |
| `subagent_monitor/subagent_monitor_coordinator_task_context.md` | `P/hatch-agent/prompts/activity_monitor/subagent_monitor_coordinator_task_context.md` | нет (П4ж-2) |
| `subagent_monitor/prompt_user.md` | `P/hatch-agent/prompts/subagent_monitor/prompt_user.md` | нет (П4ж-2) |
| `subagent_monitor/nothing_to_do_tool.md` | `P/hatch-agent/prompts/subagent_monitor/nothing_to_do_tool.md` | нет (П4ж-2) |
| `subagent_monitor/notify_main_agent_tool.md` | `P/hatch-agent/prompts/subagent_monitor/notify_main_agent_tool.md` | нет (П4ж-2) |

Удалён (П3д): `subagent/subagent_history_inheritance_isolated.md`. У muse изолированы только подагенты глубже первого, у нас таких нет.

Проверка «нет»: исходник после переименования меток совпадает с файлом байт в байт, `cmp` (27.11.2026).

Тексты, встроенные в код (описания инструментов, задания подагентам, заметки), записаны в комментарии рядом с константой: строка ядра и правки. Тест `agent/tools/tests/names.rs` проверяет, что в текстах, которые уходят модели, нет имён `muse`, `Muse` и `Meta`.

П4ж-2б: поле `escalation` из `extra.update.escalation` взято в схему `user_goal.update` и `tracking.update`. Своё (в разборе нет): описание инструмента `set_tracking_goal_lifecycle_copy` и его полей; вид улик (`transition`, `goal`, `recent_activity` рядом с именами ядра `status_before_transition`, `lifecycle_reason`, `current_state_hint`); событие создания дела для решения о внимании (`{"kind": "goal_created", …}`); вид вести будильника `[GOAL ESCALATION DUE]` (действия — по словам ядра «… should let the user know» / «… should ask the user what to do») и отметка разобранного срока в памяти службы расписаний.

## Браузер (П4к-1а)

Исходник — `P/hatch-agent/prompts/browser/`. Общие правки всех файлов: метки `*-muse-only` → `*-only`; «Muse» → «Hamlet»; блоки `browser-code-only` убраны (`browser.run` нет), у `no-browser-code-only` сняты только метки; блок `cookie-consent-only` убран (флаг не включён).

| Файл | Исходник | Правки |
|---|---|---|
| `browser/system.md` | `system.md` | убраны абзац о трёх специалистах (вход, оплата, проверки ботов) и «and the specialists», строки инструментов `browser.files`, `browser.cooldown`, `browser.bot_challenge`/`login`/`pay`, `browser.specialists`/`steer_specialist`/`kill_specialist`, шаг 3 о специалистах (шаг 4 стал 3) — П4к-2…4 |
| `browser/pages.md` | `pages.md` | убрано «Observe again after another agent returns…» (специалистов нет) |
| `browser/presentation_language.md` | `presentation_language.md` | без правок; идёт не в системный текст, а в задание и продолжение после «\n\nLanguage:\n», края обрезаны, `{locale}` подставлен (`0x8b1f920`, П5-2 часть 4) |
| задание и продолжение окна (`planner.rs`) | ядро: `0x8b1f310`, строки `0x539613`, `0x539652`, литерал «\n\nLanguage:\n» (`0x8b1f3af`) | «Muse's new instruction» → «Hamlet's new instruction» (П10) |
| `browser/instructions/assignment.md` | `instructions/assignment.md` | убрано предложение о передаче работы специалистам |
| `browser/instructions/browsing.md` | `instructions/browsing.md` | убраны «a route in the site's guide» и абзац о гайдах сайтов (П4к-3) |
| `browser/instructions/obstacles.md` | `instructions/obstacles.md` | убрано «Sign-in, checkout and bot checks belong to the specialists.» |
| `browser/instructions/asking.md` | `instructions/asking.md` | убраны обзор корзины и карта из кошелька (покупки — П4к-4) |
| `browser/instructions/reporting.md` | `instructions/reporting.md` | убраны находки специалистов и абзац о результате покупки |
| `browser/instructions/privacy.md` | `instructions/privacy.md` | убраны служба входа и абзац о файлах (`browser.files` нет) |
| `browser/instructions/{conduct,facts,commitments}.md` | те же | без правок, кроме общих |
| `browser/copyrighted_material.md` | ядро: запись `tools/copyrighted_material.md` раздела `0x64` в.1 (вид 2 — текст в ядре, `0x7ebeea`, 0x16c байт) | без правок |
| `browser/completion_gate/report_required.md` | `completion_gate/report_required.md` | без правок |
| `browser/tools/observe.{json,md}` | `tools/observe.*` | убраны поля `image` (осмотр и разметка снимков) и `listen` (звук) и их абзацы; «inspections of saved images» и `look` в последнем абзаце убраны |
| `browser/tools/act.{json,md}` | `tools/act.*` | виды действий — без `drag`, `hold`, `mouse_*`, `program`; поля `look`, `within`, `from_*`, `to_*`, `duration_ms`, `dx`, `dy`, `source` и их абзацы убраны (наведение по описанию и модель наведения — позже); описание `ref` и `timeout_ms` сокращено под это; из кодов отказа убраны `page_busy`, `site_in_use`, `entry_unsupported`, `picker_unavailable`, `pointer_*`; пример `needs_reference` — «choosing an option, ticking a box or scrolling inside an area»; модификатор `Meta` оставлен как у muse — имя клавиши, а не компании; в тесте П10 разрешён белым списком ровно здесь и в `window.rs` (слово ревизора, П4к-1б) |
| `browser/tools/tabs.{json,md}` | `tools/tabs.*` | без правок |
| `browser/tools/report.{json,md}` | `tools/report.*` | убраны поле `artifacts`, абзацы о специалистах и о результате покупки, «preserve useful findings … from agents you called» |
| `browser/tools/request_input.{json,md}` | `tools/request_input.*` | убраны обзор корзины, оплата, абзац об одноразовых кодах и `login.advance` (П4к-2) |

Своё в коде (`agent/browser/src/window.rs`, `planner.rs`; разметки вида у muse в разборе нет — её собирает отдельный «browser broker»): строки вида `- роль "имя" @eN [состояния]` из дерева доступности Chromium; предел текста вида 30 000 знаков; снимок JPEG; склейка фраз результата действий из строк ядра; ответ на `browser.request_input` («The main agent answered:»). Порядок системного текста — по профилю 1 (П5-2 часть 4). Модель исполнителя — назначение `BrowserAgent`.

Прослойка `openai_compat` (П4к-1а): картинки из результата инструмента уходят следом сообщением `user` с пометкой `[картинки результата инструмента <id>]` — роль `tool` в Chat Completions несёт только текст; у muse для Chat Completions свой модуль медиа (`kimi_chat_completions/prompt/media.rs`, `source-files.txt:5303`), вид пометки — своё.

## Окна главного агента (П4к-1б)

Исходник — строки ядра (`strings-all.txt`); общие правки — метки `*-muse-only` → `*-only`, «Muse» → «Hamlet», `muse.nothing_to_do` → `hamlet.nothing_to_do`.

| Файл | Исходник | Правки |
|---|---|---|
| `browser/main/browse.md` | описание `browser.browse` (смещения `75b11b`, `75b314`, `75bc83`, `75be00`), `{{START_ADDRESS}}` — `75ae17` | убраны: предложения о специалистах в первом абзаце, абзац о кодах входа (`75b314`, П4к-2), `files` (загрузка файлов), «A new task is not a way to retry an unresolved purchase.», абзацы о покупках, кошельке и входе (`75c0a0`–`75caf0`, П4к-2/П4к-4) |
| `browser/main/browse.json` | схема `browser.browse` (`tool-schemas.txt:2-23`, смещения `7598ac`–`75adf5`) | поля `task`, `task_id`, `profile`, `tab`, `url`, `answer_to`; не взяты `files`, `verification_code`, `user_code`, `account_hint`, `wallet`, `browser`, `sites`, `environment` (вход, коды, файлы, кошелёк, браузер устройства — П4к-2…4) |
| `browser/main/windows.md` | описание `browser.windows` (`75dd3a`–`75ea7c`) | «one sign-in covers every chat and scheduled job» → «every window»; убраны строка о сайтах с cookies профиля, окна расписаний (`owner`) и `activity`, отмена сохранённой оплаты, остановка запуска расписания, «the user can stop a window or take it over» |
| `browser/main/windows.json` | схема `browser.windows` (`75eca4`–`75f02c`) | без `cursor` (листания нет); описание `task_id` — без окон расписаний |
| `browser/main/accepted.md` | `a46f6b` | без правок |
| `browser/main/handoff.md` | «## Browser Task Handoff» (`1929e51`–`192acb2`) | убраны пункты о покупках и о Secure Vault, «plus anything you verified with your own wallet or credential tools», `{{SUBAGENT_HANDOFF_GUIDANCE}}` |
| `browser/main/outcome_completed.md` | `1923566`, `1923a5e` | убраны абзац о поиске номеров в почте владельца и предложение о слежении за покупкой |
| `browser/main/outcome_needs_input.md` | `browser_task_ask_for_information.md` (`19247fc`, `1924aa9`) | убраны абзац о поиске номеров в почте и «or purchase summary» |
| `browser/main/outcome_blocked.md` | `1922b15` | без правок |
| `browser/main/question.md` | `71f6a0`, `71fb14` | убраны «No user can answer during this run.» (фоновые запуски) и часть об обзоре корзины; `{question}` — блок полей вопроса (своё) |

Своё в коде (`agent/browser/src/browsers.rs`): вид списка окон, метаданных итога и блока вопроса; закрытие простаивающих окон — при обращении к окнам; один Chromium на профиль, профиль — папка `profiles/<id>`; новую вкладку от страницы забирает окно, чьё действие идёт. Не взяты в П4к-1б: сводка `<browser_status>` в каждом ходе, `browser.profiles`, окна расписаний.

## Глубокое исследование (П4з)

Исходник — `P/hatch-agent/prompts/blocks/deep_research/*.md`, `P/hatch-agent/prompts/research/resumed_after_restart.md` и строки ядра. Общие правки: метки `*-muse-only` → `*-only`; «Muse » → «Hamlet »; блоки `browser-code-only` убраны (`browser.run` нет), у `no-browser-code-only` сняты метки; `browser_search`/`browser_open` → `browser.search`/`browser.open` (имена наших инструментов).

| Файл | Исходник | Правки |
|---|---|---|
| `deep_research/{who_you_are,assignment,how_you_work,discretion,safety,security_authority,your_environment}.md` | те же | только общие |
| `deep_research/tools.md` | `tools.md` | убраны `look` (наведения по описанию нет) и `research_data` (рыночные данные — своей службы нет); `{tooling_lines}` подставляет код |
| `deep_research/reporting_back.md` | `reporting_back.md` | убрано предложение о файлах `data/*.json` (`research_data`) |
| `deep_research/resumed_after_restart.md` | `research/resumed_after_restart.md` | без правок; `{directory_clause}`, `{remaining_minutes}` подставляет код |
| `deep_research/tools.json` | строки ядра: описание и поля `browser_deep_research` (`strings-all.txt:75909-75911`), квитанция (`:55486`), `finish_research` и `research_notes` с ошибками (`:75909`, `:78572`, `:78634`, `:55462`), первое сообщение (`:15174-15175`, `:33048`), напоминание (`:55462`), `AGENTS.md` (`:36270-36273`), итог и сбой (`:225914`, `:42210-42211`, `:35746`), фразы о папке после перезапуска (`:55759-55762`, `:12944`) | имя поля папки — `topic` (в разборе не найдено, своё); в остальном без правок |

Своё в коде (`agent/research/src/lib.rs`): состояние запуска — `.run.json` в папке; истории исследователя не храним — после перезапуска он продолжает по своим заметкам; одно окно исследования на запуск; вид `{tooling_lines}` и `{acknowledgment}`.

## Сохранённые работы (П4и)

Исходник — строки ядра и `P/hatch-server/prompts/workflows/agent_structured_output.md`. Общая правка — метки `*-muse-only` → `*-only`.

| Файл | Исходник | Правки |
|---|---|---|
| `workflows/workflow_namespace.md` | ядро: описание пространства и «## Authoring contract» (`strings-all.txt:57828-57857`) | «the user or accepted Idea» → «the user»; «If chat, a native product surface, a schedule, a connector action, or a web artifact is the result» → «If chat, a schedule, or a connector action is the result»; убраны предложения о веб-артефакте в итоге и о `artifact.create_web_static`/`artifact.export` («A child without the artifact tools…»); «omit artifacts» убрано — артефактов и Ideas у Hamlet нет |
| `workflows/tools.json` | ядро: длинные описания `workflow.*` и описания полей (`strings-all.txt:332317-332347`) | без правок; у `limit` инструмента `workflow.list_runs` описания в разборе нет — поле без описания; у `workflow.view_run` взято общее описание `runId` |
| `workflows/agent_structured_output.md` | `P/hatch-server/prompts/workflows/agent_structured_output.md` | убрано «for a web artifact, say the web artifact is ready and do not include routes, refs, slugs, paths, source files, or implementation files;» |
| `workflows/execution_files.md` | ядро: «## Workflow execution files» (`strings-all.txt:308503-308546`, `templates.txt:4053`) | «download, or see in Library» → «or download» (Library нет); «the accepted Idea or user outcome» → «the user outcome»; «chat, a native product surface, a schedule, a connector action, or a web artifact» → «chat, a schedule, or a connector action»; «such as a Space source tree or scheduler storage» → «such as scheduler storage» |

Обвязка сценария `agent/workflows/src/harness.js` — текст ядра (`strings-all.txt:42448-42858`); в дампе нет строк короче четырёх знаков, закрывающие `  }` верхнего уровня возвращены. Подставляемые куски подсказки шага — тексты ядра: `{{RESULT_CONTRACT}}` (оба варианта), пример `ok` без схемы. Своё в коде (`agent/workflows/src/lib.rs`, шапка): пример `ok` для шага со схемой; блок `## Workflow execution files` идёт после задачи; текст передачи итога главному агенту (`[Workflow Result] run_id: …` и квитанция ядра «Workflow `…` completed.» и т. п.); прослойка `prelude.js`.

## Загрузка пространств инструментов (П4л-3)

| Файл | Исходник | Правки |
|---|---|---|
| `tool_search/tools.json` | ядро: описание `tool_search.load_tool_namespace` и поле `paths` (около `0xa10827`, `0xa109da`), маркеры (`0x9d9b51`), ответы загрузки (`0x6daf7e`, `0x990e85`, `0x96daa7`), ответ на вызов незагруженной функции (`0x99117a`), краткие описания пространств `cron`, `hooks`, `user_goal`, `tracking`, `workflow`, `tool_search` (`0x2cc9b77`–`0x2ccbae2`) | `muse.connector_search`, `muse.skill_search` → `hamlet.*` (П10); из подсказки «unavailable» убрано «Built-in presentation tools, including shopping cards, are available in their tool namespaces.» (карточек покупок нет); `{n}`, `{names}`, `{name}` — места аргументов шаблонов ядра |

Своё (код `agent/loop/src/tool_search.rs`): какие пространства отложены у главного агента (`cron`, `hooks`, `user_goal`, `tracking`, `workflow` — у кого есть полное описание пространства; полного списка в разборе нет); заглушка пространства — инструмент с именем пространства и пустой схемой, в конце списка (П5-1, часть 2; маркер в начале описания — форма ядра); вид схем в ответе загрузки; загруженные функции добавляются в конец списка инструментов запроса; какой текст ответа загрузки в каком случае.

## Разборы ядра без перенесённых текстов

- Нижний предел частоты расписаний («Please choose a cron interval of at least … interval is too frequent. Nothing was saved.», шаблон `0x437836`): функция `0xfdfd850` (машина async, вызывается из `0xfe014d7`, `0xfe05afc`). Предел `r13` = clamp(значение политики `r14`, 60, 3600) (`0xfdfe4c8`–`0xfdfe4e1`); запрошенное (`0x40(%rbx)`) ≥ max(предел, 300) — принято (`0xfdfe4f4`–`0xfdfe50e`); отказ — только когда флаг политики (`0x50(%rbx)`, итог вызова `0xfd597f0`) и запрошенное < предела (`0xfdfe514`–`0xfdfe51f`). Без политики `r13` = 60 и отказа нет. У Hamlet политики нет — отказа нет, как у muse без неё (решение ревизора, П4л-2).
- Запрет по адресам (`mase_all_urls_fingerprints.bin`, П4л-9) — не восстановлен, своего нет. Что известно:
  - формат файла: `MUFP`, u32 версия 1, u32 1 (`key_schema_version`), u32 0, u64 число записей (7 497 435), дальше отсортированные u64 LE;
  - ключ — нормализованный адрес из `mase_all_urls_normalized.fst` (`manifest.json`); этого файла (300 МБ) в снимке нет;
  - ядро файл не читает: в нём нет ни имён файлов чёрного списка, ни магии `MUFP`, ни крейтов `fst` и быстрых хэшей (`crates.txt`: только `sha2`, `sha1_smol`, `ahash`, `foldhash`). Проверка адресов у muse — внешняя: `url_blocklist_guard/lookup.rs` пишет `refusal_kind`, `reply_variant`, `expected_count`/`actual_count` (около `0x88e158`), рядом `BrowserCentralBlackholeBlocklist` (`0x4b29a0`) и «Poppy page blocklist check … answer … timed out» (`0x8bd058`);
  - ограниченная попытка: 20 000 доменов из `mase_all_domains.fst` × 9 форм записи (`d`, `d/`, `http(s)://d[/]`, `www.d[/]`, `https://www.d/`) × 19 хэшей (xxh3/xxh64/xxh32, murmur3, sha256, sha1, md5, blake3, blake2b, siphash, fnv1/1a, wyhash, city, farm; первые 8 байт LE и BE) — ни одного попадания.
- Повтор дозаполнения векторов памяти (замечание к П4л-8): срока повтора у muse не найдено (`memory_reindex_embedding` — только назначение модели, `maintenance_reindex trigger=` — без чисел). Своего срока нет: службу и так будит пульс (`heartbeat`, раз в 30 минут), на его такте сверка повторится.
- Предел вызовов workflow («workflow agent call limit exceeded: max {} calls per run», шаблон `0x3d426a`) — не найден (П4л-2, повторный заход П4л-10), своего предела нет. Что известно:
  - сравнение в `0x277a4dfa` (и у ссылок `0x277a6c6d`, `0x277ada15`): счётчик (`lock xadd` в `0x84`, минус `0x80`, +1) против поля `0x88`; при превышении флаг `0x8c` (`xchgb`);
  - структура — внутри `Arc` из task-local слота `%fs:-0x228`/`-0x230` (функция `0x277a4d40`), то есть поля данных — `0x70`/`0x74`/`0x78`/`0x7c`;
  - запись этих полей в области `0x27600000…0x27900000` не найдена (немедленных записей в `0x88(%reg)` с соседними `0x80`/`0x84` нет);
  - ручек настройки нет (`env-knobs.txt`, строки `*call_limit*`/`max_calls` — чужие).
- Число повторов вызова модели (а1, `MODEL_RETRIES = 1`) — не найдено (П4л-10), своё число не менялось. Что известно:
  - строки «retrying inference after streamed retryable anthropic error before visible output» (`0x6b1554`, ссылки `0x92a6b09`, `0x92a7585`) и причины остановки `retry_budget_exhausted` / `visible_output_emitted` (`0x6b15a5`/`0x6b15bb`, выбор по флагу в `0x92a6e0a`);
  - счётчик попытки — поле `0x4b850` машины async (увеличение с насыщением в `0x9294846`, `0x9296562`, `0x929b21b` и др.);
  - сравнения этого счётчика с числом в функции `0x9290000…0x92c0000` нет: флаг бюджета приходит готовым из другой функции.

## Продолжение по списку дел (П4л-5)

| Файл | Исходник | Правки |
|---|---|---|
| `todo/todo_continuation_handoff.md` | ядро: `session/todo_continuation_handoff.md` (около `0x9f40a0`, текст до `</system-reminder>`) | `muse.nothing_to_do` → `hamlet.nothing_to_do` (П10); `{{RECONCILIATION_GUIDANCE}}`, `{{TODO_SNAPSHOT}}` подставляет код |
| `todo/todo_reconciliation.md` | ядро: `session/todo_reconciliation.md` (около `0x9f3c3d`) | `muse.nothing_to_do` → `hamlet.nothing_to_do` (П10) |

## Запросы к своей базе `hamlet.db` (П4л-6)

| Файл | Исходник | Правки |
|---|---|---|
| `db/tools.json` | ядро: описание `muse.db` и поле `sql` (около `0x945f6e`), `struct MuseDbQueryArgs` (`0xa33ba7`) | «Muse» → «Hamlet» (П10); список примеров сужен до того, что есть в нашей базе («memories, scheduled work, goals and briefings, and workflow runs and their agent calls» вместо transcripts, messages, Feed и Ideas, artifacts, device-sync); путь `/opt/muse/skills/muse_db/references/schema.md` → `{schema_path}`, его подставляет код — настоящий путь навыка в папке системных навыков; короткое описание не взято (у нас его некому показывать); `texts` — свои (ниже) |
| `../skills/hamlet_db/` | у muse навык `muse_db` есть в списке навыков ядра (`tool-schemas.txt:3760`), но его папки в снимке нет и текста в ядре нет | своё: `skill.yaml`, `SKILL.md` — коротко, по описанию инструмента; `references/schema.md` и `references/querying.md` строит из каталога код (`Catalog::schema_md`, `querying_md`), тест сверяет файлы с базой |

Из ядра (`hatch-agent/src/tools/db.rs`, исполнение около `0x1f444281`…`0x1f445717`):
- тексты ошибок проверки (`0x18a9ce1`), отношения/функции/приведения (`0x3134c1`), повтор столбцов (`0x398a37`), общий срок и длина (`0x32b1e2`), «failed» (`0x34f99b`); `muse.db` → `hamlet.db`;
- разбор — sqlparser 0.62.0, PostgreSqlDialect; исполняется нормализованный текст, если он разбирается в то же дерево;
- длина запроса 16384 байта (`cmp $0x4000` у `0x1f4f1227`), общий срок 5000 мс (`0x49f2e0`), `statement_timeout` 3000 мс (`0x4a3318`), `lock_timeout` 250 мс (`0x4a3970`), `set transaction read only`, `search_path = pg_catalog`;
- строка — `to_jsonb(...)::text`, длиннее 65536 байт (`0x4a1f28`) — пометка `_hatch_db_row_omitted`; `limit 201` (`0x498088`) — шаблон у `0x1820e96`, аргументы у вызова `0x1f44471b`;
- каталог — форма вшитого JSON muse (`format_version` 3, около `0x18a9a6a`), функции и приведения — его `query_capabilities` как есть.

Своё (код `agent/db`): в каталоге — наши таблицы, которые есть и в каталоге muse (`READABLE`, 19; `runtime.event_hook_admissions` в каталоге muse нет); без `migrations_sha256` и `redaction` (скрытых столбцов нет); каталог вшит файлом `agent/db/catalog.json`, сверку с миграциями держит тест; ответ модели — строки JSON по одной на строку, из 201 — 200 и строка «`{n} rows shown; more rows matched. Narrow the query to see the rest.`», пусто — «`0 rows`» (вид ответа в разборе не найден); `hamlet.db` — в общем наборе (главный, подагент, работник расписаний, шаг работы), как поиск навыков: профили muse с этим инструментом в разборе не видны.

## Время дня расписаний `timing` / `window` (П4л-7)

`cron/tools.json` и `cron/cron_namespace.md` переведены на вариант с ключом схемы `cron_runtime_timing` (`gk.hatch_cron_runtime_timing`; ключ стоит в `schema_gates` профилей, `strings-all.txt:69993`, `:70435`, `:70582`):
- поле `schedule` — описание `0xbe39a4` (вместо `0xbe222a`);
- поле `schedule.flexible_time` убрано (при этом варианте ядро отвечает «schedule.flexible_time is not available; use schedule.timing»); добавлены `schedule.timing` (`0xbe3c7b`) и `schedule.window` (`0xbe3ed1`); в `timing` «this VM» → «this machine»: агент живёт не в ВМ;
- в `notification_requested` и в пространстве `cron` («Choosing the schedule time») ядро и в этом варианте говорит о `flexible_time`. Своё: `schedule.flexible_time` → `schedule.timing`, `flexible_time: false` → `timing: exact`, `flexible_time: true` → `timing: window` — по определениям `exact`/`window` из `0xbe3c7b`.

Код `agent/cron/src/schedule.rs`. Ошибки — тексты ядра: `strings-all.txt:84943` и соседние; шаблон «schedule.timing required for kind={kind}: exact, window, or anytime» — `0x332554`/`0x332579`. Затравки — `cron-timing-window:` (`0x42d9bd`) и `cron:timing-anytime` (`0x491360`).

Своё:
- «машина» — путь дома агента (`Core::machine`), у muse — ВМ;
- время внутри окна и дневное время берутся как FNV-1a от затравки с машиной, с точностью до минуты; дневное время `anytime` — любая минута суток;
- у `interval` `timing` необязателен (как в описании) и, если дан, действует на `at`;
- старт интервала без `at` не менялся: в разборе «the runtime picks the start» не прочитано.

## Пробуждение службы расписаний (П4л-8)

Текстов для модели нет. Основание в ядре: `notify-8.2.0` и `inotify-0.11.2` (`crates.txt`), `notify::inotify` (`strings-all.txt:344927`), «file watcher backend error» / «file watcher error» (`:17841`, `:21857`), сон до срока (`scheduler loop: … computing wake`). Своё (код `agent/cron/src/service.rs`, `agent/hamlet/src/lib.rs`): будят правки `cron.*` и `hooks.*`, конец прогона и наблюдатель файлов; наблюдатель смотрит файлы памяти дома (те же правила, что у сверки); уборщика корзины будит ближайший срок истечения; без сроков служба спит до события.

## История главного агента в Postgres (П3е-1)

Текстов для модели нет. Таблицы — `agent/history/migrations/0001_history.sql`:
- `agent.agents`, `agent.context_items`, `agent.volatile_context_pins` по каталогу ядра (около `0x18a9a6a`);
- перечисления `message_role`, `context_item_kind` — из списка типов ядра;
- запись — форма `INSERT INTO context_items … ON CONFLICT (agent_id, seq) DO UPDATE` (около `0x6cb97b`);
- граница подсказки — `agents.prompt_floor_seq` (запрос около `0x6e16f9`).

Строка — одна часть сообщения, части связывает `message_id`. Виды частей — как у ядра:
- `message`, `message_text`, `text_segments`, `tool_call`, `tool_output` (запрос `item_kind IN ('message', 'message_text', 'text_segments')` около `0x2ecebb`);
- `parts` в `item_json` (около `0x16f8c7b`);
- мышление — отдельные строки, их скрывает вид `inspection.context_items` (`redaction` каталога ядра у `context_items`).

В каталог `hamlet.db` добавлены `agent.agents`, `agent.context_items`, `agent.volatile_context_pins`. Поле `redaction` каталога снова есть: запрос к таблице с ним переписывается на её вид. Текст пометки — ядра, «muse.db» → «hamlet.db».

Своё — в заголовке `agent/history/src/lib.rs`:
- вид части: текст — `message_text`, мышление и картинка — `message`; `item_json` — наша часть как есть;
- `text_segments` не используется: порога в разборе нет;
- `message_id` — `<agent_id>:<seq>`;
- пометка заметки изменчивого;
- закреплённое начало `prompt_prefix`;
- агент главной сессии — `root`.

Не в этом срезе:
- ящик (П3е-2);
- вызовы, оставшиеся без результата при остановке посреди исполнения инструментов (П3е-5, точки перезапуска muse).

## Ящик главного агента в Postgres (П3е-2)

Текстов для модели нет. Таблица `agent.message_mailbox` — `agent/history/migrations/0002_mailbox.sql` по каталогу ядра. Запросы — форма ядра: вставка `accepted`; `attached` с `checkpoint_custody = 'executing'` и `stream_owner_message_id`; закрытие `completed`/`aborted`; выборка `accepted` по `accepted_ordinal`. Таблица добавлена в каталог `hamlet.db` (23 таблицы).

Своё — в `agent/history/src/lib.rs` (`AgentMailbox`):
- вид `submission_id`; `conversation_epoch` 0; `routing_scope` `root`;
- строка владельца — подача `user`, как в запросах ядра (`submission_kind = 'user'`): она идёт в ящик сразу, и во время хода тоже. Ход берёт её между шагами, а пришедшая во время остановки ждёт в базе;
- весть службы — `notice` (у ядра внутренние подачи — варианты `SavedSubmissionKind`, значений в столбце не видно);
- перед ходом hamlet забирает поданное: за строкой владельца — заметка изменчивого;
- имя хода `root:turn:<мс>:<n>`.

## Список дел и сторож продолжения в Postgres (П3е-3)

Текстов для модели нет. Таблица `runtime.agent_todo_snapshots` — `agent/history/migrations/0003_todo.sql` по каталогу ядра. Запросы — форма ядра (около `0x16f9853`):
- замена с `revision + 1` и новым поколением при новом `continuation_key`;
- напоминание — условный `UPDATE … continuation_reminders < $5`.

Сообщения «todo continuation skipped: snapshot unavailable / guard unavailable» — ядра. Таблица добавлена в каталог `hamlet.db` (24 таблицы).

Своё (код `agent/tools/src/todo.rs`, `agent/history/src/lib.rs`):
- вид `continuation_key` — незакрытые пункты (текст, статус) без порядка, JSON;
- `conversation_id` и `agent_identity` — `root`;
- без хранилища (подагенты, тесты) сторож живёт в памяти, как раньше.

## Сжатия и счёт токенов в Postgres (П3е-4)

Текстов для модели нет. Таблицы `agent.agent_compactions` и `agent.agent_message_token_usage` — `agent/history/migrations/0004_compactions.sql` по каталогу ядра; вставки — форма ядра (около `0x16f4750`, `0x1700cb4`).

Вид `inspection.agent_compactions` без `details_json` — `redaction` каталога ядра. Обе таблицы добавлены в каталог `hamlet.db` (26 таблиц) с этой пометкой.

Своё (`agent/history/src/lib.rs`):
- `trigger` — `context_pressure`: значения в разборе нет, рядом — «context-pressure compaction failed…»;
- `details_json` — `{"replacement_history": …}`;
- `first_kept_seq` — `seq` первого оставленного сообщения старой истории;
- `checkpoint_seq` — новая граница подсказки;
- `input_tokens` — вход вместе с чтением и записью кэша.

## Точка перезапуска и оборванные вызовы (П3е-5, часть 1 — главный агент)

`prompts/restart/interrupted_tool_call.md` — текст ядра как есть (около `0x8ada56`, рядом с разбором точки перезапуска при старте): результат вызова, который был отдан инструменту, но не кончился из-за перезапуска.

Таблица `agent.runtime_restart_checkpoints` — `agent/history/migrations/0005_restart.sql` по каталогу ядра. Запросы — форма ядра:
- запись — `INSERT … SELECT … WHERE EXISTS (… status NOT IN ('shutdown', 'not_found')) ON CONFLICT (agent_id) DO UPDATE` (около `0x1711918`);
- чтение — `SELECT payload_json … WHERE agent_id = $1 AND mode = $2` (около `0x1710f52`).

Режимы — значения ядра: ход пишет `before_inference` с `payload_json`, между ходами — `idle` без `payload_json`. У главного агента `recovery_class = 'foreground_root'`, `recovery_owner_id` — его id, `execution_config_json` не пуст (запрос ядра около `0x1729b52`). Таблица добавлена в каталог `hamlet.db` (27 таблиц).

При старте (`restart_rehydration` ядра):
- вести, взятые ходом, которого нет в точке, — `aborted`;
- вызовы последнего ответа без результата получают текст выше;
- ход из точки `before_inference` продолжается под тем же именем, если история кончается не ответом модели; его вести закрываются по итогу;
- если ход успел ответить, его вести — `completed`, точка — `idle`.

Своё (`agent/history/src/lib.rs`, `agent/loop/src/prompt.rs`, `agent/hamlet/src/daemon.rs`):
- `payload_json` — `{"message_id": <имя хода>}`; вести связаны с ходом через `stream_owner_message_id` (у ядра payload — состояние хода с незаконченными вызовами, его вида в разборе нет);
- `execution_config_json` — `{"model": …}`;
- точка пишется один раз в начале хода, а не перед каждым обращением к модели: наш payload за ход не меняется;
- отбор осиротевших вестей — `stream_owner_message_id` не равен ходу из точки;
- результат оборванного вызова — с `is_error = true` (признака в разборе не видно).

## Подагенты в базе и их перезапуск (П3е-5, часть 2а)

`prompts/restart/work_not_resumable.md` — текст ядра как есть (`restart_after_output_unrecoverable`, около `0x2a4cd73`). Это итог работы, которую после перезапуска нельзя безопасно продолжить.

Таблица `agent.subagent_spawns` — `agent/history/migrations/0006_subagents.sql` по каталогу ядра. Строка подагента в `agent.agents` — `kind = 'subagent'`, статусы — значения ядра (`running`, `completed`, `errored`, `interrupted`, `shutdown`). Запросы — форма ядра:
- строка агента — около `0x1707f0c`;
- заведение — около `0x6abfd4`;
- новая работа (`completed_at = NULL`) — около `0x16a2601`;
- конец работы — около `0x8b76dd` и `0x171a560`.

Подагент пишет свою историю в `agent.context_items` и точку перезапуска, как главный агент; начало подсказки закрепляется пином. Таблица добавлена в каталог `hamlet.db` (28 таблиц).

При старте (`restart_rehydration/orphan_reconciliation.rs` ядра: «marked orphaned active subagent errored after restart», счётчики `restart_checkpointed_subagent_count`, `orphaned_subagent_count`):
- незаконченный подагент с точкой `before_inference` продолжает ход; вызовы без результата закрываются текстом ядра, итог уходит родителю;
- незаконченный без такой точки — `errored`, родителю итог с текстом выше;
- законченные снова в `subagent.list` и принимают `subagent.send`, история — из базы по первой работе.

Особые подагенты (`Kind`: исследователь, дети `workflow`) в базу не пишутся. Их поднимает своя служба («runtime_owned_subagent_count», `parent_agent_runtime_owned_skip_count`).

Своё:
- id подагента остаётся `agent-<n>`; после перезапуска нумерация идёт с наибольшего;
- `metadata_json` — `{"finish_hook": true}`, если итог проходит через крючок родителя (исполнитель забывания). Такой подагент после перезапуска не продолжается: крючок не переживает остановку;
- имя хода подагента — `<id>:turn:<n>`, `recovery_class` его точки пуст;
- `seen_at` не ведётся: итог сразу уходит в ящик родителя.

## Владельцы восстановления: работники расписаний (П3е-5, часть 2б)

`prompts/restart/run_continues.md` — текст ядра как есть («continue scheduler recovery owner», около `0x2a4cf45`). Он встаёт в историю работника перед продолжением фазы исполнения.

Таблица `agent.recovery_owners` — `agent/history/migrations/0007_recovery_owners.sql` по каталогу ядра. Запросы — форма ядра (около `0x1717efb`):
- заведение — `on conflict (recovery_class, owner_id) do nothing`;
- закрытие — `terminal_at = now(), terminal_status`.

Классы фаз — значения ядра `scheduler_cron_execute` и `scheduler_cron_resolve`. Прогон — владелец класса `scheduler_cron_execute`. Работник пишет историю и точку перезапуска: `recovery_class` — класс текущей фазы, `recovery_owner_id` — прогон. `scheduler.job_runs.worker_agent_id` — id работника. Таблица добавлена в каталог `hamlet.db` (29 таблиц).

При старте (`Scheduler::recover`):
- прогон с незакрытым владельцем и точкой `before_inference` работника продолжается с записанного. Вызовы без результата закрываются, фаза исполнения идёт с заметкой ядра, фаза итога — заново по записанной истории;
- прочие идущие прогоны закрываются как неудачные текстом ядра «Stopped because the agent restarted mid-run…» (как было). Владелец закрывается с `failed`;
- владелец закрывается итогом прогона в `settle_run`.

Своё:
- id работника — `cron-worker:<run_id>`. Строка в `agent.agents` — `kind = 'subagent'`, `agent_type` — класс владельца, без родителя. `message_id` владельца — id работника;
- остаток срока исполнения при продолжении — полный срок минус время от начала прогона, не меньше секунды;
- договор итога записывается в историю до точки фазы итога. Поэтому продолжение итога всегда его видит;
- свидетельства для итога после перезапуска — по записанной истории; вышел ли срок — по `worker_execution_timed_out`;
- `agent.recovery_owner_terminal_events` не взята;
- наблюдатели (у muse — `runtime.execute_resolve_runs`, класс `execute_resolve_worker`), вызовы `workflow` и окна браузера — часть 2в.

## Прогоны наблюдателей и их перезапуск (П3е-5, часть 2в)

Текстов для модели нет. Таблица `runtime.execute_resolve_runs` — `agent/cron/migrations/0003_execute_resolve.sql` по каталогу ядра. Запросы — форма ядра (около `0x18c4fcf`…`0x18c6d27`):
- срабатывание наблюдателя — прогон `worker_kind = 'event_hook'`, `source_ref` — срабатывание, `lane_key` — наблюдатель;
- работник берёт прогон новым поколением (`worker_generation + 1`, `worker_agent_id`, `execute_message_id`, `resolve_message_id`);
- решение — `terminal_decision`; дальше `notify_pending` → `completed_notified`, либо `completed_noop`, либо `failed` с `last_error_stage`.

Работник — владелец восстановления класса ядра `execute_resolve_worker`, точки обеих фаз — с этим классом. Таблица добавлена в каталог `hamlet.db` (30 таблиц).

При старте (`execute_resolve_worker/notify_pending_replay.rs`, «fail execute-resolve recovery owner», «invalid restart checkpoint» ядра):
- `notify_pending` — итог доставляется снова;
- не взятый работником — берётся заново;
- взятый, с незакрытым владельцем и точкой `before_inference`, продолжается с записанного. Вызовы без результата закрываются, фаза — по имени хода;
- прочие — `failed` со стадией `invalid_restart_checkpoint`.

Своё:
- id работника — `hook-worker:<run_id>:<мс>`;
- `metadata_json` — `{"execute", "resolve"}`: тексты фаз, по ним прогон берётся заново;
- решение и `notify_pending` пишутся одним запросом;
- `handoff_message_id` — `<run_id>:handoff`;
- фаза оборванного хода — по суффиксу имени хода `:resolve`;
- заметки о продолжении у наблюдателя нет: текста ядра для `execute_resolve` в разборе не видно.

## Вызовы шагов workflow после перезапуска (П3е-5, часть 2г)

Новых текстов и таблиц нет. Вызов шага (`runtime.workflow_agent_calls`) — владелец восстановления класса ядра `workflow_subagent` («multiple active workflow calls name the same recovery owner» ядра). Подагент шага пишет историю и точки перезапуска с этим классом; владелец закрывается итогом подагента.

При старте (`Workflows::recover`, «load workflow recovery owner» / «fail workflow recovery owner» ядра):
- запуск, как и раньше, снова в очереди с повтором сценария;
- идущий вызов с незакрытым владельцем и точкой `before_inference` продолжается при повторе. Вызовы без результата закрываются текстом ядра, подагент идёт с записанной истории и закреплённым началом;
- прочие идущие вызовы — `failed` текстом ядра `restart_after_output_unrecoverable` (`prompts/restart/work_not_resumable.md`). Повтор сценария получает эту же ошибку и не исполняет шаг заново;
- раньше открытые вызовы сбрасывались в `pending` и исполнялись с нуля, а сделанное ими могло повториться.

Особый подагент (`Kind`) получил продолжение с истории (`resume`) и запись (`KindRecord`).

Своё:
- id подагента шага в базе — `workflow-call:<call_id>`;
- имя хода — `<call_id>:turn:<n>`;
- `terminal_status` владельца — статус подагента (`done`, `failed`, `interrupted`, `closed`).

## Окна браузера после перезапуска (П3е-5, часть 2д)

Новых текстов и таблиц нет. Окно (`task_id`) — владелец восстановления класса ядра `browser_task`: класс из списка `recovery_class` ядра, «open browser task owner store», «browser-task-restart-recovery». Исполнитель окна пишет историю и точки перезапуска; владелец закрывается, когда окно закрыто главным агентом или по простою.

При старте (`Browsers::recover`, из `Services::start`):
- окно незакрытого владельца открывается снова в своём профиле. Куки и вход сохранены в папке профиля;
- работа с точкой `before_inference` продолжается с записанной истории, вызовы без результата закрываются текстом ядра. Итог доходит главному агенту обычной заметкой окна;
- окно без идущей работы ждёт новых поручений под прежним `task_id`;
- окно, которое не открылось, закрывается итогом `blocked` с текстом ядра `restart_after_output_unrecoverable`.

Остановка программы (`shutdown`) владельцев не закрывает — после старта окна поднимаются.

Планировщик принимает `message: None` — продолжение записанной работы.

Своё:
- вкладки после перезапуска не восстанавливаются: окно открывается с пустой вкладкой `t1`, страницу исполнитель находит сам по истории (его вызов закрыт текстом «check what actually happened»);
- id исполнителя — `browser-window:<task_id>`;
- закреплённое начало — системный текст исполнителя без инструментов: их исполнитель собирает сам под окно;
- сведения окна (`spawn_metadata_json`) — профиль и задание. В `WorkerStore` для этого добавлены `metadata` и `list_active`;
- имя хода — `<task_id>:turn:<длина истории>`;
- вопросы главному агенту, ждавшие ответа до перезапуска, не переносятся: вызов вопроса закрыт текстом ядра, исполнитель спросит снова, если нужно.

## «Past conversations» в `hamlet.memory_search` и `chat.read_messages` (П3е-6)

Тексты — строки ядра как есть, `muse.memory_get` → `hamlet.memory_get`, `muse.memory_search` → `hamlet.memory_search`:
- описание `memory_search` с разделом «Past conversations» (личный и групповой варианты, около `0x8facc2`);
- описания `maxResults` и `minScore` для этого варианта (около `0x8fa116`);
- тексты сбоя половины поиска (около `0x6da6ff`);
- описание и схема `chat.read_messages` в режиме окна (`chat_read_messages.md`, около `0x8fa116`);
- тексты ошибок читалки (около `0x6da6ff`).

Без агента главного чата (`with_conversations`) инструмент остаётся прежним вариантом без раздела.

Поиск — форма ядра (около `0x175c52f`, `0x35da36`):
- `runtime.search_documents`, `websearch_to_tsquery('simple', q)` или префиксный `to_tsquery('simple', …)`;
- только реплики пользователя и ассистента главного чата, новые сверху;
- выдержка — `ts_headline(… 'StartSel="", StopSel="", MaxWords=24, MinWords=8')`;
- по умолчанию 6, не больше 100 совпадений; `minScore` разговоры не фильтрует.

Реплика пишется в индекс при записи истории (`AgentHistory::insert`): часть `message_text`, роль `user` или `assistant`, не заметка изменчивого. После сжатия старые реплики остаются в `context_items` и в поиске.

Своё:
- `owner_table` — `agent.context_items` (у muse — `runtime.events`, событий у Hamlet нет), `owner_key` — `<agent_id>:<seq>`;
- `runtime.search_documents` заводится и миграцией истории (та же схема, что у памяти) — на случай базы без памяти;
- старая история, записанная до П3е-6, в индекс не попадает;
- префиксный запрос — каждое слово из букв и цифр как `слово:*` через `&`;
- поля совпадения — `chat_id` (`main`: чат один), `message_id`, `timestamp`, `speaker`, `excerpt`; совпадения разных запросов сливаются по `message_id`;
- при сбое половины — `status: partial` и `warning` с текстом ядра;
- ответ читалки — `messages` (`matched` у найденного) и `more_before` / `more_after`;
- окно читалки — сообщения целиком, части текста сообщения склеиваются переводом строки;
- текст незнакомого `message_id` и чужого `chat_id`;
- режим истории чата (`chat_id` без `message_id`, `cursor`, `limit`) не взят: чат один, а `chat.list` у Hamlet нет.

## Проверка ответа (П3ж-1)

`prompts/verification/` — тексты muse `hatch-server/prompts/verification/` как есть: `classifier.md`, `execute_task.md`, `evidence.md`, `review_scope.md`, `resolve_note.md`, `health.md`, `health_relevance.md`, `bookings.md`. Кроме них:
- `not_relevant.md` — `hatch-agent/prompts/tools/verification_not_relevant.md`;
- `feedback.md` — `session/response_verification_feedback.md` из ядра (около `0x6bd81e`).

Правки:
- `muse.*` → `hamlet.*` в именах инструментов;
- метки `personal-muse-only` / `group-muse-only` → `personal-only` / `group-only`;
- П10: из `evidence.md` убраны два предложения о «Meta-maintained docs under `~/docs/`» — у Hamlet таких документов нет.

Устройство — как у muse:
- классификатор (назначение `ResponseVerificationClassifier`, место `request_verification_classifier`, инструмент `submit_request_verification` с описаниями ядра, категории `health` / `bookings` / `none`);
- проверяющий (место `verifier`: `read`, `browser.search` / `open` / `find` / `lookup_citation_url`, `nothing_to_do`, `notify_main_agent`, `verifier.not_relevant` — только у `health`, завершает исполнение и возвращает исходный черновик);
- прогон `worker_kind = 'verification'` в `runtime.execute_resolve_runs`, `metadata_json.category`;
- фаза исполнения — `execute_task.md` с `{category_guidance}` (у `health` — `health.md` и `health_relevance.md`, у `bookings` — `bookings.md`) и `{evidence_instructions}` (`evidence.md` с `{review_scope}`); фаза итога — `resolve_note.md`;
- `notify_main_agent` — исправленный черновик: главный агент получает заметку `feedback.md` и пишет окончательный ответ; `nothing_to_do` или сбой («Verification did not finish.») — отдаётся исходный черновик;
- переключатель `[verification] enabled` (по умолчанию включено) — ворота muse `HEALTH_RESPONSE_VERIFICATION_ENABLED`.

Своё:
- вход классификатора — до 10 недавних реплик в `<recent_conversation>` («говорящий: текст») и просьба в `<request>`;
- задание проверяющему — разделы `<verification-task>` (строка владельца), `<verification-draft>`, `<verification-trajectory>` (имена — строки ядра), затем текст фазы;
- путь проверяющего — вызовы хода с аргументами и первой строкой результата (до 500 знаков);
- системный текст проверяющего — системный текст агента;
- проверяется только ход со строкой владельца; проверка идёт внутри хода, а не фоновым прогоном с продолжением хода;
- id прогона — `verification:<мс>:<n>`, работника — `verifier:<run_id>`;
- прогон проверки, оборванный перезапуском, не продолжается (черновик не отдан, ход не возобновляется: история кончается ответом модели);

## Проверка отчётов расписаний (П3ж-2)

Тексты muse как есть: `cron_classifier.md` (`muse.*` → `hamlet.*`), `purchases.md` из `hatch-server/prompts/verification/`; `unverified_handoff.md` — из ядра (`verification/unverified_handoff.md`, около `0x2adc791`).

Устройство — как у muse (`hatch-db-scheduler/src/scheduler/cron_verification.rs`):
- классификатор — место `cron_verification_classifier` на модели `main_chat` (у нас — модель главного разговора), инструмент `submit_cron_verification` с описаниями ядра, категории `health` / `bookings` / `purchases` / `none`;
- проверяется только успешный отчёт («cron report is not successful»);
- проверяющий — тот же, что у ответа; у `purchases` — `purchases.md` в `{category_guidance}`;
- сбой проверки — к отчёту дописывается `unverified_handoff.md`;
- включается тем же `[verification] enabled`.

Своё:
- вход классификатора — `<task>` (тело задания) и `<report>`;
- путь проверяющего — вызовы работника (как у ответа);
- исправленный отчёт заменяет исходный в итоге прогона (`result_summary`) и в заметке главному агенту;
- пометка о сбое — после отчёта через пустую строку;
- (до долга П3ж) категория не сохранялась — теперь сохраняется, см. «Отложенный итог прогона».

Ворота завершения окна браузера (`browser/completion_gate/report_required.md`) уже были (П4к). Ворота артефактов, Spaces и Ideas — со своими строками (П17, П22).

## Карантин куска подсказки (П3ж-3)

Новых текстов нет; ошибка хода — строка ядра «request failed because part of the input context is unsupported; the failing input was excluded from future context to keep the session recoverable (…)» (`strings-all.txt:54781`).

Устройство — как у muse (`message_execution › prompt_quarantine.rs`, `turn_execution › anthropic_prompt_poison.rs`):
- «prompt poison» — отказ поставщика (`invalid_request`, 400) с признаком битой картинки; список признаков — строки ядра около `0x22478f2` («turbojpeg decompress failed», «premature end of jpeg file», «could not decode image», «could not process image», «unable to decode image», «failed to decode image», «image decode failed», «invalid image data», «invalid base64 image», «unsupported gif variant»);
- кусок удаляется из истории и из `agent.context_items` (`DELETE FROM agent.context_items WHERE agent_id = $1 AND seq = ANY($2)`, `removed_context_seq_count`), вызов инструмента — вместе с результатом (`tool_call_count`); строки поиска по разговорам — тоже;
- кусок из прошлого — карантин и повтор вызова модели («quarantine historical poison»); кусок текущего шага (`current_iteration_input_quarantined`) — карантин и ошибка хода; кандидата нет («prompt poison recovery found no quarantine candidate») — ошибка как есть; удаление не удалось — «prompt poison recovery failed».

Своё:
- место — из `messages.N` / `messages[N]` текста ошибки, если там картинка (строки `messages[`, `messages.` рядом со списком признаков), иначе последнее сообщение с картинкой;
- сообщение ассистента с вызовом уходит целиком;
- текущий шаг — сообщения после последнего ответа модели;
- в скобках ошибки — текст ошибки поставщика;
- правило одно для обоих поставщиков (у muse разбор — в `anthropic_prompt_poison.rs`, а признаки лежат рядом с кодом других поставщиков).

## Периодический сброс памяти (П3ж-4)

Новых файлов текстов нет. Строка ядра «Today's memory log at ~/{path} already ends with the entries below. Do not re-append anything already recorded there; append only new entries and corrections.\n\n{entries}» (около `0x6bf7fc`) — в `agent_loop::compaction::today_log_note`.

Устройство — как у muse (`memory_flush/periodic_coverage.rs`, `maintenance/handlers/periodic_memory_flush.rs`):
- порог `JARVIS_MEMORY_FLUSH_PERIODIC_THRESHOLD_TOKENS` — по умолчанию 50 000 (`mov $0xc350` после чтения ручки, ядро около `0xac1408c`);
- набралось столько токенов со времени прошлого сброса — тот же ход сброса (`memory_flush_prompt.md`) идёт без хода владельца, после ответа; журнал — «periodic memory flush started / completed / failed», «failed to persist periodic memory flush progress», «failed to read pending periodic memory flush»;
- граница — `periodic_memory_flush_progress` (до какого `seq` сброшено);
- сброс перед сжатием пропускается, если всё уже сброшено (`periodic_flush_covered`);
- перед просьбой о сбросе — сегодняшний журнал памяти, если файл есть (у обоих сбросов); дата — в поясе владельца («failed to read home timezone for memory flush»).

Своё:
- граница — столбец `agent.agents.periodic_memory_flush_progress` (у muse — поле заголовка сессии, заголовка у нас нет); после сжатия граница ставится на конец (сброс перед сжатием покрыл всё);
- токены набранного — o200k сообщений после границы;
- журнал показывается целиком; без пояса — дата UTC;
- в той же структуре настроек ядра рядом с порогом лежат ещё 4000 и 20 000 (`0xfa0`, `0x4e20`) — что это, в разборе не видно; не взяты.

Не взято (нет у Hamlet того, к чему это относится):
- `session_memory_capture` / `agent.session_memory_capture_deadlines` — сбор памяти побочных чатов при простое и архивации (`side_chat_idle`, `side_chat_archive`); побочных чатов нет;
- `idle_internalization` — «idle-session internalization after {N}s quiet window» запускает самоулучшение (`JARVIS_SELF_IMPROVEMENT_IDLE_*`), а не сброс памяти; придёт со срезом самоулучшения.

## Очистка разговора (П3ж-6)

Тексты ядра: «Conversation history before this boundary was cleared.» (граница, около `0x1705e09`), «Conversation cleared.» (`chat.command.conversation_cleared`, около `0x12bcc03`).

Устройство — как у muse (`session/impl_session/conversation_clear.rs`):
- команда `/clear` в чате; только у главной сессии и только между ходами («clear_conversation requires an idle root session»);
- принятые и не взятые ходом строки владельца отменяются («rejecting a follow-up a conversation clear superseded»), вести служб остаются;
- граница пишется в `agent.context_items` с `message_source = 'runtime.conversation_clear'`, граница подсказки сдвигается на неё (`UPDATE agents SET updated_at = $1, prompt_floor_seq = $2 WHERE id = $3`);
- старый разговор сбрасывается в память в фоне («background pre-clear memory flush finished», «… failed after conversation clear»).

Своё:
- граница — сообщение роли developer;
- строки, поданные после `/clear`, ждут, пока очистка не пройдёт;
- граница периодического сброса ставится на конец (старое покрыто сбросом перед очисткой);
- после перезапуска история, которая кончается границей очистки, — не оборванный ход;
- `conversation_epoch` ящика не ведётся (у нас 0), отмена — по состоянию `aborted`.

## Отложенный итог прогона (долг П3ж)

Устройство — как у muse (`scheduler/pending_terminal_result.rs`, `scheduler/cron_verification.rs`):
- столбец `scheduler.job_runs.pending_terminal_result_json text`;
- успешный отчёт работника при включённой проверке сначала ложится туда (прогон остаётся `running`), категория классификатора — туда же, ключ `verification_category` (`retain_cron_verification_category`: `select pending_terminal_result_json from scheduler.job_runs where run_id = $1 and status = 'running' and pending_terminal_result_json is not null for update`, `update scheduler.job_runs set pending_terminal_result_json = $2 where run_id = $1`);
- после перезапуска идущий прогон с отложенным итогом не возвращается к работнику: проверка продолжается с сохранённой категорией (`load_cron_verification_category`), классификатор снова не зовётся; затем обычный итог прогона.

Своё:
- содержимое — сигнал работника (`outcome`, `message`, `blocked_reason`, `blocked_dependency`), `timed_out` и категория; `none` тоже сохраняется;
- отложенный итог пишется, только когда проверка включена (без неё между работником и итогом шага нет);
- проверка, оборванная перезапуском, начинается заново с той же категорией (прогон `verification` в `runtime.execute_resolve_runs` не продолжается);
- путь проверяющего после перезапуска — история работника из базы.

## Сжатие по частям (П3ж-5)

Тексты muse как есть (метки `personal-muse-only` / `group-muse-only` → `personal-only` / `group-only`): `compaction/segmented_summarization.md`, `segmented_state.md`, `segmented_state_over_cap.md` (из `hatch-compaction/prompts/compact/`), `segmented_output_cap_feedback.md`, `output_cap_feedback.md` (из `hatch-agent/prompts/compact/`). Пометка «Older summaries were removed here to save space. Recover what happened in this span from saved memories, including the daily notes under `~/memory/`, or from conversation history.» — строка ядра `0xb3a3ca`.

Числа — декомпиляцией ядра (Ghidra 12.1.4, без анализа, адреса — виртуальные при базе 0):
- подготовка `0xccdb910`: пять пределов от окна модели W — W/10, (W/10)·2/5, min(W/50, 6000)/2, (W/10)·4/5, min(W/50, 6000); без окна — 0x6a40, 0x2a80, 0xaa0, 0x5500, 0x1540 (= те же формулы при W = 272 000);
- сборка просьбы `0xf1f67e0`: `{state_target_tokens}` — (W/10)·2/5, `{segment_target_tokens}` — min(W/50, 6000)/2; `segmented_state_over_cap.md` через пробел, если прошлое состояние (байты·2/7) длиннее цели;
- проверка ответа `0x974df76` (`segmented_block_limit`, `segmented_state_missing`): часть длиннее min(W/50, 6000) или состояние длиннее (W/10)·4/5 → замечание (`compaction_segmented_reply_feedback`);
- обрезка `0xcdadad0` («segmented compaction reply over its hard limits; cut to fit», поля `segment_limit_tokens`, `state_limit_tokens`) — по тем же пределам;
- вырезание старых частей `0xf205ab0`: пока отрисованные части длиннее W/10, удаляется самая старая (после пометки), новейшая остаётся; на месте удалённых — пометка;
- отрисовка `0xf203ed0`: `<segment from="…" to="…">\n…\n</segment>\n\n`, затем `<current_state>\n…\n</current_state>`; разбор `0xf204200` принимает разделы `New segment` / `Current state` и блоки `<new_segment>` / `<current_state>`.

Устройство: прошлая сводка остаётся в начале истории; модель пишет новую часть и состояние; включатель `[compaction] segmented` (у muse — флаг `hatch_segmented_compaction`); окно — `context_window` весов главной модели. Обычное сжатие при упоре в предел ответа — одно замечание `output_cap_feedback.md` (`compaction_output_token_cap_feedback`).

Своё:
- `from` / `to` у частей не пишутся (в ядре они необязательны; откуда берутся значения, не видно; у сообщений Hamlet нет времени);
- замечание — одно (в ядре есть `feedback_retry_exhausted`, число попыток не видно);
- новой части нет и после замечания — сводка пуста, сжатие не идёт («compaction summary cannot be empty»); состояния нет — остаётся прошлое;
- включатель по умолчанию включён (значение флага у muse не видно);
- `recent_conversation.md`, `chat_worker_conversation.md` не взяты — они для побочных чатов.

## Заметка «No tools available.» в запросе сводки (П3з-0)

`compaction/no_tools.md` — текст muse `hatch-agent/prompts/compact/no_tools.md` как есть. Подаётся в запросе сводки (обычной и по частям) сообщением роли developer: описания инструментов в запросе остаются (в старой части есть вызовы), а заметка говорит, что инструментов нет.

Своё: место — сразу перед просьбой о сводке (в ядре путь лежит в таблице текстов, место подачи не видно).

## Роли агентов (П3з-1)

`prompts/roles/<id>.toml` — определения ролей из ядра как есть (`strings-all.txt:69124-74375`): `foreground_root`, `owner_subagent_depth1`, `workflow_subagent_depth1`, `scheduler_cron_execute`, `scheduler_cron_resolve`, `execute_resolve_worker`, `verifier`, `memory_flush`. П10: имена флагов `muse_mail`, `muse_town`, `welcome_to_muse_tutorial` → `hamlet_*` (модель их не видит).

Устройство — как у muse: набор агента — его роль: из общего набора остаются только инструменты списка роли. Имена реестра ядра → имена Hamlet: с точкой — как есть, `browser_<x>` — `browser.<x>`, прочие — `hamlet.<x>`. Итог работника расписаний получает `exec` (роль `scheduler_cron_resolve`); ход сброса памяти — набор `memory_flush` (`exec`, `memory_get`, `memory_search`, `read`, `write`) вместо набора главного агента. У подагента и шага работы больше нет `todo.write` (он только у `foreground_root`); у работника наблюдателя — `cron.*`, `hooks.*`, `trash.*`; у работника расписаний — `hooks.*`, `cron.stop`, `user_goal.create`.

Своё:
- подагент главного агента — роль `owner_subagent_depth1` (подагент с правами владельца; в ядре правило выбора между `owner_*` и `generic_*` не видно);
- работнику расписаний не даны окна браузера (в роли они есть): окна Hamlet отдают итог главному агенту;
- инструменты ролей, которых у Hamlet нет, в наборе отсутствуют.

Часть 2:
- исполнитель окна браузера — роль `browser_task` (`prompts/roles/browser_task.toml`): набор планировщика окна отбирается её списком; модуль ролей — в `agent_loop::roles`, им пользуются и браузер, и сборка агента;
- `forget.plan` — у muse «ordinary untyped read-only planning subagent» (строка ядра около `0xa38dfb`): обычный подагент, отдельной роли нет, «только чтение» — заданием; у Hamlet так и есть (подагент роли `owner_subagent_depth1`, задание «Stay read-only…»);
- исследователь — в определениях ролей ядра своей роли нет (есть `feed_workflow_agent_research`, `proactivity_researcher`, `self_improvement_web_research` — другие службы); `research_delegate.rs` заводит подагента со своим набором — у Hamlet так же (`agent_research`);
- свой системный текст роли (`prompt_gates`) — тексты блоков системной подсказки, П5.

## Подагенты глубины 2 (П3з-2)

Тексты muse как есть (`hatch-agent/prompts/session/`): `subagent/subagent_history_inheritance_isolated.md`, `subagent_context_depth_one_coordinator.md`, `subagent_context_depth_one_spawn_inheritance.md`, `subagent_spawning.md`. Роль `prompts/roles/owner_subagent_depth2.toml` — определение ядра как есть (П10: `muse_*` → `hamlet_*`).

Устройство — как у muse:
- предел глубины — из ролей: у `owner_subagent_depth1` есть `subagent.*`, у `owner_subagent_depth2` нет; `MAX_SPAWN_DEPTH` = 2 (в первом сообщении «depth N/2»). Отказ ядра «Agent depth limit reached. Solve the task yourself.» (`0x883b6b`, код `agent_depth_limit`, `0x884450`) не взят: на глубине 2 инструмента нет;
- подагент глубины 1 наследует разговор родителя, глубины 2 — изолирован («Any subagents you spawn deeper than 1 start in isolated mode», `subagent_history_inheritance_inherited.md`): без копии разговора, без заметки «Above is the conversation history…», блок родителя — `…_isolated.md`;
- итог внука — в ящик его родителя («Your sub-agents will announce their results back to you automatically (not to your parent agent)», `subagent_spawning.md`): ход подагента забирает его между шагами; кончился ход — итог будит подагента новым ходом, его ответ снова уходит корню (отсюда `…_depth_one_coordinator.md`: «restate the full synthesized deliverable»);
- `scope` у `subagent.list` / `subagent.send`: `direct` — свои дети, `descendants` — всё поддерево (описания полей — строки ядра);
- ящик есть у каждого агента (`agent.message_mailbox` по `agent_id`): итог внука — весть `notice` в ящике подагента; ход подагента берёт её (`attached` за своим ходом `<id>:turn:<n>`), по итогу хода — `completed`; принятые и не взятые до перезапуска вести поднимаются при старте и доходят до подагента;
- в базе: `agents.depth` и `subagent_spawns.child_depth` — 2, `parent_agent_id` — подагент; после перезапуска дерево собирается по `subagent_spawns.parent_agent_id`, внук с точкой `before_inference` продолжает ход.

Своё:
- описания `subagent.*` — за прочими (раздел `subagent_spawning.md` убран в П5-2: профиль его не рисует);
- блоки глубины 1 в первом сообщении — у подагента глубины 1, который сам заводит подагентов (условие ядра не видно);
- id подагентов — один счёт `agent-<n>` на всё дерево;
- в записи поддерева `subagent.list` — поле `parent_agent_id` (имя — по столбцу `subagent_spawns`);
- `subagent.close` останавливает и подагентов закрытого (в разборе не видно);
- таблица `agent_ancestors` ядра не взята: поддерево читается из памяти, после перезапуска — из `subagent_spawns`;
- `routing_scope` вести в ящике подагента — `root`, как у корня (значения ядра для подагентов не видно).

## `items` в `subagent.spawn` и `subagent.send` (П3з-3)

Описания полей — строки ядра как есть: у `subagent.send` — «Multimodal input (text/image/file); takes precedence over message…» (`0x2cdc078`), у `subagent.spawn` — «Multimodal input for the child's first message; takes precedence over message…» (`0x2cdcede`). Поля элемента названы там же: `type` (`text`, `image`, `file`), `text`, `path`, `filename`, `caption`.

Устройство — как у muse:
- `items` важнее `message` («items takes precedence when both are set»), поэтому ни одно из полей не обязательно;
- `path` — «a file path as `read` takes it»: картинка и файл читаются тем же `hamlet.read` из набора подагента (картинка — картинкой, документ — переводом в Markdown, текст — строками);
- отказ — со стабильным кодом причины, как велит описание spawn: `invalid_spawn_input` у spawn (`0x6ed40e`), `input_error` у send (`0x6ed504`, тот же список кодов).

Своё (в ядре `subagents_tool/input_items.rs` не декомпилируется: функция у `0x9c93f20` уходит в таймаут):
- у spawn части `text` идут в шаблон первого сообщения (`[Subagent Task]: …`) через пустую строку, прочитанные файлы — за этим текстом в том же сообщении; у send части идут по порядку;
- перед вложением — строка `<filename или path>: <caption>`;
- `image`, который не прочитался картинкой, — отказ;
- `read_paths` в `runtime_tool_output` (ядро записывает прочитанные пути, `0xaaf3be0`) не ведутся: у Hamlet нет `runtime_tool_output`;
- в `subagent_spawns.prompt` — текст задачи без вложений (вложения — в истории подагента).

## Монитор подагентов на глубине 2 (П3з-4)

Тексты muse как есть: `subagent_monitor/handoff_child.md`, `subagent_monitor/parent_follow_up_silence_guidance.md` (П10: `muse.nothing_to_do` → `hamlet.nothing_to_do`), `subagent_monitor/subagent_monitor_nested_subagent_task_context.md` (у muse — `activity_monitor/`).

Устройство — как у muse (системный текст монитора `blocks/activity_monitor/subagent_monitor.md`):
- координатор — подагент корня, вершина поддерева; работник — агент, чей шаг вызвал событие (сам координатор или его подагент); в системном тексте — задача координатора (`{coordinator_task_context_section}`) и, если работник глубже, его задача (`{nested_subagent_task_context_section}`);
- заметка — «receiving parent agent» работника: работник-координатор — корню (`coordinator_agent_*`, `worker_agent_*`, `coordinator_children`), работник глубже — координатору по `handoff_child.md` (`child_agent_*`, `child_subagents`, `{{SUBAGENT_HANDOFF_GUIDANCE}}` — `parent_follow_up_silence_guidance.md`);
- координатор отвечает на такую заметку или молчит `hamlet.nothing_to_do` (`nothing_to_do` есть в роли `owner_subagent_depth1`; описание — `tools/nothing_to_do_without_reaction_description.md`): ход, кончившийся им, родителю ничего не шлёт («Assistant responses may be relayed to your parent, so write one only for a substantive coordinator result»).

Своё:
- снимок берётся в миг решения монитора: блоки «Coordinator» / «Worker» — `agent_id`, `depth`, `status` (у работника ещё `ordinal: n/m` — место среди детей его родителя, как `child_agent_ordinal`), «Coordinator Children» и `child_subagents` — строки `- <id> (depth N): <status>`, «Subtree Snapshot» — координатор и его дети с отступом;
- `hamlet.nothing_to_do` — только у подагента, который сам заводит подагентов (у прочих заметок монитора нет); после молчания состояние подагента — прежний итог;
- заметка корню — прежнего вида (П4ж-2а); шаблон ядра «## Subagent Progress» с `update_kind` — для итогов, его значения для промежуточных событий не видны.


## Тексты главного разговора — блоки `user_chat` и `blocks/chat` (П5-1, часть 1)

`prompts/user_chat/` — 101 блок ядра `blocks/user_chat/*.md`: 88 основных, 9 вариантов с `@` (`@without_todo_list`, `@texting_style`) и 4 `environment/*` (в описи П1б их не было). Текста в `prompts/` muse нет — только в ядре. Ядро держит блок записью из 72 байт: имя (указатель, длина), переменные, варианты, текст (указатель, длина) и вид (`2`). Записи лежат статическими таблицами в `.data.rel.ro` или собираются кодом функций-реестров разделов (`0xa9511e0`, `0xa9546f0`, `0xa954b90`, `0xa9551c0`, `0xa9c1090`, `0xa9cd930`, `0xcd54210`). Текст взят по указателю и длине из записи, байт в байт; где пара «имя — текст» выбирается флагом (`cmov`), — по дизассемблеру: `memory_search` / `memory_search_baseline` (`0xa951423`), `memory` / `memory_baseline` (`0xcd542eb`), `browser_and_sources_handoffs` (`0xa9519a0`), `turn_memory_search_reminder` / `turn_recollection_reminder` (`0xa952f0f`). Тексты `conversation.md` и `secure_vault.md`, которых П1б не нашёл, — в записях реестра (`0xa9511e0`, `0xa9cd930`).

`prompts/chat/` — 18 файлов `P/hatch-agent/prompts/blocks/chat/*.md`.

Шаблоны домашних файлов `agents/hamlet/home/` (`AGENTS.md`, `SOUL.md`, `IDENTITY.md`, `USER.md`, `MEMORY.md`) — тела шаблонов ядра (`0x8635c4`–`0x863fb0`) без шапки YAML, как muse кладёт их в домашнюю папку. `HEARTBEAT.md` — шаблон ядра (`0xbc8667`) как есть; лишняя строка-комментарий прежнего шаблона П4е убрана в части 2 (`agent/cron/src/service.rs:42`). Не взяты: `TOOLS.md` (в домашней папке muse его нет), `PROACTIVE_PREFERENCES.md` (П13).

Общие правки (П10), во всех файлах: метки `personal-muse-only` / `group-muse-only` → `personal-only` / `group-only`; `` `muse.<инструмент>` `` → `` `hamlet.<инструмент>` ``; `Muse` → `Hamlet` (и в `Muses`, `Muse-to-Muse`, `Muse-network`); `muse.md` → `hamlet.md`; `/opt/muse/` → `/opt/hamlet/`.

Правки по месту:

| Файл | Было | Стало |
|---|---|---|
| `user_chat/browser_and_sources_places.md` | «For Instagram, Facebook, Threads, and Meta AI links,» | «For Instagram, Facebook, and Threads links,» |
| `user_chat/docs.md` | « - one guide per device, including Meta Quest (`quest.md`), Meta glasses, Meta Ray-Ban Display and Meta Neural Band (`glasses.md`), and Muse Charm (`charm.md`).» | « - one guide per device.» |
| `user_chat/how_you_work_product_questions.md` | « In messaging apps without a section there, Muse app controls live in the Muse app or at muse.ai; say so.» | (убрано) |
| `user_chat/how_you_work_product_questions.md` | «Nobody opens the Muse app or muse.ai for you,» | «Nobody opens an app for you,» |
| `user_chat/how_you_work_product_questions.md` | «Muse's and Meta's policies» | «Hamlet's policies» |
| `user_chat/how_you_work_product_questions.md` | «Meta's policies» | «Hamlet's policies» |
| `user_chat/who_you_work_for.md` | «You are not an assistant to Meta, a service,» | «You are not an assistant to a company, a service,» |
| `chat/how_you_work.md` | «, where Muse UI is not available. Never give in-app navigation as if they can tap it there: say it's in the Muse app or at muse.ai.» | «.» |
| `chat/how_you_work.md` | «Meta's policies» | «Hamlet's policies» |
| `chat/how_you_work.md` | «Meta maintains these docs» | «The Hamlet team maintains these docs» |
| `chat/who_you_work_for.md` | «You are not an assistant to Meta, a service,» | «You are not an assistant to a company, a service,» |

Сборка подсказки из этих блоков — часть 2 (раздел ниже).

Адреса текстов в ядре:

| Файл | Текст в ядре | Байт |
|---|---|---|
| `user_chat/active_work.md` | `0x7b871c` | 1184 |
| `user_chat/active_work@without_todo_list.md` | `0x7b8569` | 435 |
| `user_chat/active_work_blockers.md` | `0x7b7bff` | 564 |
| `user_chat/active_work_blockers@without_todo_list.md` | `0x7b7e6d` | 393 |
| `user_chat/active_work_finishing.md` | `0x7b801e` | 742 |
| `user_chat/active_work_finishing@without_todo_list.md` | `0x7b833f` | 433 |
| `user_chat/active_work_updates.md` | `0x7b8de3` | 743 |
| `user_chat/active_work_updates@without_todo_list.md` | `0x7b8c1c` | 455 |
| `user_chat/artifacts.md` | `0x7d2c71` | 718 |
| `user_chat/background_results.md` | `0x7d2f5c` | 858 |
| `user_chat/background_results_delivery.md` | `0x7d3b0b` | 1654 |
| `user_chat/background_results_silence.md` | `0x7d32dc` | 747 |
| `user_chat/browser_and_sources.md` | `0x7c054a` | 3465 |
| `user_chat/browser_and_sources_handoffs.md` | `0x7c12fa` | 2207 |
| `user_chat/browser_and_sources_places.md` | `0x7c1c51` | 467 |
| `user_chat/captchas.md` | `0x7c4a74` | 2187 |
| `user_chat/commentary_visibility.md` | `0xa08ebc` | 160 |
| `user_chat/communication_and_delivery.md` | `0x7c66e6` | 608 |
| `user_chat/context_and_active_work.md` | `0x7b2ac0` | 27 |
| `user_chat/conversation.md` | `0x7c6974` | 3353 |
| `user_chat/conversation@texting_style.md` | `0x7c76bb` | 4380 |
| `user_chat/date_time_awareness.md` | `0x7a5649` | 1480 |
| `user_chat/discretion_and_alignment.md` | `0x7a03f0` | 6757 |
| `user_chat/docs.md` | `0x7ac111` | 4786 |
| `user_chat/due_diligence.md` | `0x7b98d6` | 5311 |
| `user_chat/due_diligence_follow_ups.md` | `0x7b93b3` | 1238 |
| `user_chat/due_diligence_live_information.md` | `0x7badc7` | 3674 |
| `user_chat/environment.md` | `0x7a6d7d` | 60 |
| `user_chat/environment/connected_accounts.md` | `0x7a6dd8` | 350 |
| `user_chat/environment/deployments.md` | `0x7a7361` | 279 |
| `user_chat/environment/secure_vault.md` | `0x7a6f68` | 394 |
| `user_chat/environment/wallet.md` | `0x7a711e` | 541 |
| `user_chat/goals.md` | `0x7b7559` | 1677 |
| `user_chat/handling_background_handoffs.md` | `0x7d6dc4` | 2955 |
| `user_chat/handoff_orientation.md` | `0x7d4425` | 962 |
| `user_chat/how_you_work.md` | `0x7a3fcc` | 1053 |
| `user_chat/how_you_work_limitation.md` | `0x7a1e81` | 231 |
| `user_chat/how_you_work_product_questions.md` | `0x7a441b` | 2107 |
| `user_chat/identifiers_and_dates.md` | `0x7bd5d8` | 589 |
| `user_chat/loading_tools.md` | `0x7a9c33` | 778 |
| `user_chat/memory.md` | `0xa065eb` | 3226 |
| `user_chat/memory_baseline.md` | `0xa05a0f` | 3001 |
| `user_chat/memory_correction_baseline.md` | `0x7b69e9` | 1186 |
| `user_chat/memory_recall.md` | `0x7a5561` | 28 |
| `user_chat/memory_scope_and_custody.md` | `0x7b6eb9` | 1652 |
| `user_chat/memory_search.md` | `0x7b4e94` | 3120 |
| `user_chat/memory_search_baseline.md` | `0x7b333e` | 6956 |
| `user_chat/offer_options.md` | `0xa04c7e` | 759 |
| `user_chat/peer_collaboration.md` | `0x7c87d7` | 42100 |
| `user_chat/proactive_work.md` | `0x7d797f` | 6164 |
| `user_chat/proactivity.md` | `0x7a6086` | 3288 |
| `user_chat/proactivity_follow_through.md` | `0x9fe5a6` | 421 |
| `user_chat/proactivity_guidance.md` | `0x7d35f5` | 1262 |
| `user_chat/proactivity_shared_preferences.md` | `0x7d57e8` | 5198 |
| `user_chat/purchases.md` | `0x7c531b` | 3777 |
| `user_chat/recurring_work.md` | `0x7aadab` | 519 |
| `user_chat/recurring_work_failures.md` | `0x7aa366` | 578 |
| `user_chat/recurring_work_hooks.md` | `0x7aa22d` | 273 |
| `user_chat/recurring_work_operations.md` | `0x7aaffb` | 2516 |
| `user_chat/recurring_work_recovery.md` | `0x7c61f9` | 1218 |
| `user_chat/recurring_work_results.md` | `0x7aa5d3` | 1932 |
| `user_chat/recurring_work_verification.md` | `0x7ab9fe` | 149 |
| `user_chat/retaining_memories.md` | `0x7b5ae5` | 1887 |
| `user_chat/retaining_memories@without_todo_list.md` | `0x7b627c` | 1863 |
| `user_chat/safety_core.md` | `0x7b0cb0` | 1309 |
| `user_chat/secure_vault.md` | `0x7c25a0` | 2322 |
| `user_chat/secure_vault_code_from_user.md` | `0x7c2117` | 1114 |
| `user_chat/secure_vault_credentials.md` | `0x7c2ede` | 6094 |
| `user_chat/secure_vault_entry_card.md` | `0x7c1e52` | 666 |
| `user_chat/secure_vault_resend.md` | `0x7c46d3` | 669 |
| `user_chat/security.md` | `0x7af18d` | 2011 |
| `user_chat/security_anti_phishing.md` | `0x7b05c7` | 660 |
| `user_chat/security_anti_phishing_tail.md` | `0x7b088a` | 652 |
| `user_chat/security_operations.md` | `0x7bd875` | 3531 |
| `user_chat/security_operations_anti_phishing.md` | `0x7be675` | 542 |
| `user_chat/security_operations_disclosure.md` | `0x7be8c5` | 2322 |
| `user_chat/security_operations_sensitive_values.md` | `0x7bf20f` | 1327 |
| `user_chat/security_secrets.md` | `0x7af98c` | 2509 |
| `user_chat/security_service_limits.md` | `0x7b0384` | 537 |
| `user_chat/service_limits.md` | `0x7bff31` | 1527 |
| `user_chat/shared_spaces.md` | `0x7bfdb4` | 348 |
| `user_chat/taking_ownership.md` | `0x7b2b06` | 1934 |
| `user_chat/tools.md` | `0x7a92a8` | 1317 |
| `user_chat/tools_approvals.md` | `0x7a980d` | 1029 |
| `user_chat/tools_confirmation.md` | `0x7a8883` | 1460 |
| `user_chat/tools_estimates.md` | `0x7a877e` | 226 |
| `user_chat/tools_outward_discretion.md` | `0x7a8e5d` | 1030 |
| `user_chat/turn_commentary_instructions.md` | `0x7bd590` | 24 |
| `user_chat/turn_diligence_reminder.md` | `0x7bd188` | 1032 |
| `user_chat/turn_diligence_reminder@texting_style.md` | `0x7bc1d2` | 1451 |
| `user_chat/turn_diligence_reminder@texting_style_without_todo_list.md` | `0x7bcbf5` | 1427 |
| `user_chat/turn_diligence_reminder@without_todo_list.md` | `0x7bc7ba` | 1008 |
| `user_chat/turn_memory_search_reminder.md` | `0x7bbe4b` | 756 |
| `user_chat/turn_proactivity_reminder.md` | `0x7d6c95` | 303 |
| `user_chat/turn_public_commentary_instructions.md` | `0x7bd590` | 24 |
| `user_chat/turn_recollection_reminder.md` | `0x7bbc21` | 508 |
| `user_chat/who_you_are.md` | `0x7a1fd5` | 6380 |
| `user_chat/who_you_work_for.md` | `0x79ff67` | 824 |
| `user_chat/who_you_work_for_household.md` | `0x7a3c8f` | 829 |
| `user_chat/who_you_work_for_principal.md` | `0x7a02c3` | 255 |
| `user_chat/working_with_tools.md` | `0x7bf7c8` | 1478 |

## Системный текст главного разговора — сборка (П5-1, часть 2)

Сборка — `agent/hamlet/src/main_prompt.rs`, по форме `prompt_assembly` muse (`hatch-agent/src/session/impl_session/prompt_assembly/`, `source-files.txt:1365-1394`).

**Откуда порядок.** Копия системного текста главного разговора, которую muse пишет при старте (`prompt_assembly/system_prompt_copy.rs`, `source-files.txt:1386`): `rootfs-full/home/hatch/workspace/system/system_prompt.md` в muse-live, стандартная обвязка. Взяты только порядок и состав блоков; данных пользователя muse в ней нет и в агент из неё ничего не перенесено. Сборка из блоков ядра повторяет эту копию строка в строку (кроме правок ниже и выкинутых блоков); стыки между блоками (сколько переводов строки) — оттуда же. Тексты верхних блоков лежат в ядре в том же порядке (`templates.txt:704-836`, адреса `0x7a1fe4`–`0x7b267a`) — это проверяет тест `blocks_in_core_order`; порядок заголовков — `headings_in_muse_order` (`agent/hamlet/tests/main_prompt.rs`).

**Условия — по набору инструментов главного агента**, как варианты muse:
- `todo.write` есть — `chat/keeping_track_of_active_work.md`, нет — `@without_todo_list`;
- `hamlet.memory_search` — `### Memory` (`user_chat/memory_recall.md` с `user_chat/memory.md`);
- `chat.read_messages` — пункт `chat/managing_context_chat_history.md`;
- `browser_deep_research` — правило `chat/tool_rules_deep_research.md`;
- есть отложенные пространства — блок правил «Loading Tools» (`user_chat/loading_tools.md`, часть `client-tool-loading-only`, метки сняты; побайтно как в копии muse, строки 278–283 — тест `loading_block_matches_muse_snapshot`). Списка пространств в системном тексте нет: каждое отложенное пространство уходит в списке инструментов запроса заглушкой — краткое описание с маркером в начале, без пробела (форма ядра: `strings-all.txt:332137`, маркеры — `templates.txt:2253`). Своё: заглушка — инструмент с именем пространства и пустой схемой (плоский список функций у поставщиков), в конце списка. Прежняя форма среза tool_search (список «- `имя` маркер: описание» в начале подсказки и урезанная копия `tool_search/loading_tools.md`) убрана. Второй раз в начало подсказки блок не добавляется — `Prefix::with_system`;
- `subagent.spawn` — `session/subagents_doctrine.md` и `session/subagents_status.md` со значениями главного разговора (`blocks/subagents/chat/*`, `blocks/subagents/inherited_context/*`, `asker` = «the user», `failure_action` = «tell the user»);
- `cron.add` — раздел расписаний; `hooks.add` — пункт `user_chat/recurring_work_hooks.md`.

Варианты `@texting_style` не уходят: у muse их включает флаг стиля переписки, которого в стандартной обвязке нет (в копии — основные блоки).

**Копия текста** — в `~/workspace/system/system_prompt.md` при сборке агента, как у muse (`blocks/chat/system_prompt_copy_file.md` говорит о ней модели).

**Своё:**
- дополнения владельца из `system.md` папки агента — после собранного текста через пустую строку (у muse такого файла нет; пустой файл ничего не добавляет). Системные тексты остальных ролей — пока `system.md`, их тексты — П5-2;
- правило глубокого исследования у muse — переменная `deep_research_delegation_rule` блока `user_chat/tools_approvals.md`, которого у Hamlet нет (согласий на вызовы нет); здесь — отдельным пунктом на его месте, имя инструмента — `browser_deep_research`;
- `cron_created_acknowledgement` подставлен без перевода строки в конце (в копии muse — с пробелом в конце, в ядре 25.11 — с переводом строки).

**Не уходят — частей ещё нет** (строка плана, которая их вернёт):

| Блок | Что | Строка плана |
|---|---|---|
| `user_chat/how_you_work_product_questions.md`, `user_chat/docs.md` | справочник о себе | П5-3 |
| `chat/how_you_evolve.md` (`session/self_evolution.md`), `chat/alignment_status_pointer.md` | самоулучшение | П11 |
| `chat/relationships.md` (`session/relationships.md`) | карта людей | П11 |
| `user_chat/proactivity.md` | проактивность | П13 |
| `chat/user_location_awareness.md` (`location/*`, `shared/current_location_*`), `shared/environment/devices.md` | устройства владельца | П21 |
| `chat/task_acknowledgment.md`, `chat/writing_style_reacji_bullet.md`, `chat/response_formatting_attachments.md` | реакции и вложения в мессенджере | П6 |
| `chat/voice_messages.md` | голос | П15 |
| `chat/response_formatting_structured_content.md`, `chat/response_formatting_interactive_ui.md`, `user_chat/environment/deployments.md`, `shared/environment/artifacts.md` | виджеты, артефакты, сайты | П17 |
| `chat/response_formatting_connect_links.md`, `user_chat/environment/connected_accounts.md`, `user_chat/environment/secure_vault.md` | коннекторы и хранилище секретов | П18 |
| `session/chats_legacy.md` (Side Chats), `shared/environment/chats.md` | побочные чаты, связь с другими | П19 |
| `chat/response_formatting_product_links.md`, `user_chat/environment/wallet.md`, `user_chat/tools_approvals.md` | покупки и согласия на оплату | П20 |
| `shared/environment/feed.md` | лента | П22 |
| `shared/who_built_you@subscriber_model.md` | вариант для подписки Meta — у Hamlet не будет | — |

**Правки по месту** (общие правки П10 — как в части 1):

| Файл | Было | Стало |
|---|---|---|
| `shared/who_built_you.md` | «You were created by Meta and are powered by Muse Spark 1.3 from the Muse model family, a new series of models from Meta. Muse is the product name for Meta's personal AI agent, as well as your default given name until changed. You may be referred to as Muse interchangeably. » | «Hamlet is your default given name until changed. » (модель у Hamlet выбирается настройкой) |
| `user_chat/who_you_are.md` | «You have a name, persona, voice, and an avatar.» | «You have a name and a persona.» (голоса — П15, аватара нет) |
| `shared/environment/browser.md` | «. The user can watch or take over a live session at any time» | (убрано: экрана живого окна нет) |
| `shared/environment/user_goals.md` | «, shown in the Goals tab» | (убрано: вкладки нет) |
| `shared/environment/tracking.md` | « live in the Goals tab and» | (убрано) |
| `chat/filesystem.md` | «`~` (`/home/hatch`)» | «`~`» (дом — из настроек агента) |
| `chat/filesystem.md` | два абзаца о встроенном хранилище файлов muse (`/opt/hatch/bin/remote-storage`) | (убраны: хранилище со ссылкой — П17) |

Адреса текстов в ядре (взяты по записи блока, как в части 1):

| Файл | Текст в ядре | Байт |
|---|---|---|
| `shared/who_built_you.md` | `0x7a3aa3` | 446 |
| `chat/personalization.md` | `0x7a4dc4` | 1895 |
| `chat/contextual_awareness.md` | `0x7a560d` | 25 |
| `chat/timezones_save.md` | `0x7a5c49` | 508 |
| `chat/date_validation_opener.md` | `0x7a5e62` | 285 |
| `chat/date_validation_rest.md` | `0x7a5fa4` | 54 |
| `chat/date_batch_check_available.md` | `0xa07372` | 532 |
| `chat/managing_context_opener.md` | `0x7e5647` | 398 |
| `chat/managing_context_retention.md` | `0x7e5a86` | 395 |
| `chat/managing_context_chat_history.md` | `0x7e57fb` | 173 |
| `chat/keeping_track_of_active_work.md` | `0x7e3cb2` | 3109 |
| `chat/keeping_track_of_active_work@without_todo_list.md` | `0x7e4914` | 3028 |
| `chat/writing_style.md` | `0x7e73e1` | 5765 |
| `chat/writing_style_follow_ups.md` | `0x7e7192` | 524 |
| `chat/writing_style_brevity.md` | `0x7e903c` | 774 |
| `chat/response_formatting.md` | `0x7e20ca` | 121 |
| `chat/response_formatting_markdown.md` | `0x7e216e` | 1193 |
| `chat/response_formatting_message_breaks.md` | `0x7e1f1d` | 346 |
| `chat/response_formatting_links.md` | `0x7e263f` | 719 |
| `chat/response_formatting_link_labels.md` | `0x7e2a74` | 968 |
| `chat/response_formatting_citations.md` | `0x7e60c6` | 275 |
| `chat/tool_rules_deep_research.md` | `0x9fe20b` | 877 |
| `chat/cron_created_acknowledgement.md` | `0xa086b0` | 454 |
| `chat/workspace_files.md` | `0x7abc05` | 1210 |
| `chat/runtime_files_list.md` | `0x7ad3fc` | 643 |
| `chat/filesystem.md` | `0x7ad6af` | 2120 |
| `chat/system_prompt_copy_file.md` | `0xa04808` | 589 |
| `chat/filesystem_memory.md` | `0x7adef7` | 540 |
| `chat/filesystem_workspace.md` | `0x7ae136` | 4155 |
| `chat/safety_oversight.md` | `0x7b120b` | 3241 |
| `shared/managing_context_body.md` | `0x7e58fa` | 355 |
| `shared/environment/computer.md` | `0x7a7587` | 145 |
| `shared/environment/browser.md` | `0x7a763c` | 311 |
| `shared/environment/runtime.md` | `0x7a7797` | 321 |
| `shared/environment/user_goals.md` | `0x7a817d` | 642 |
| `shared/environment/tracking.md` | `0x7a8424` | 547 |
| `shared/safety_hard_stops.md` | `0x7b1ed6` | 1837 |
| `shared/safety_self_modification.md` | `0x7b262c` | 78 |
| `shared/safety_sensitive_attributes.md` | `0x7b26a6` | 1006 |
| `session/subagents_doctrine.md` | `0xa04230` | 1299 |
| `session/subagents_status.md` | `0xa08209` | 644 |
| `subagents/chat/responsive_clause.md` | `0x9fe839` | 45 |
| `subagents/chat/overload_consequence.md` | `0x9fe893` | 36 |
| `subagents/chat/after_spawn_clause.md` | `0x9feb93` | 103 |
| `subagents/inherited_context/transcript_fact.md` | `0x9fe8ec` | 74 |
| `subagents/inherited_context/task_content_bullet.md` | `0x9fe96f` | 209 |
| `subagents/inherited_context/fanout_bullet.md` | `0x9fea73` | 245 |

## Системные тексты ролей — профили ядра и карта разделов (П5-2, часть 1)

**Откуда порядок.** У muse системный текст каждой роли собирается по профилю подсказки. Таблица профилей — `.data.rel.ro` `0x290bbdd0`–`0x290bcd18` (в файле ядра −`0x2000`): главный профиль и 69 пронумерованных. У профиля — срезы (указатель, длина) на массивы `u16` в `.rodata`; в номере раздела младший байт — раздел, старший — вариант. Всё выписано в `prompts/profiles/profiles.toml` (номер, адрес, роль, у кого роль у Hamlet или строка плана, срезы с адресами).

**Раздел → блоки.** Функция-диспетчер `0xa9b9dc0` (таблица переходов `0x786d40`, 203 ветки; номера меньше `0x25` уходят в реестр `user_chat` `0xa9511e0`, `0x26` — в `0xd32dc40`, это `blocks/background/*`) по номеру раздела возвращает список записей блоков (запись — 72 байта: имя, переменные, варианты, текст, вид). Функция зависит только от номера, поэтому список снят прогоном: свой маленький эмулятор x86-64 по тексту objdump (рабочий скрипт, в репозиторий не идёт) для каждого из 352 номеров всех профилей. Итог — `prompts/profiles/sections.toml`: по разделу — блоки по порядку, переменные, где текст (вшит в ядро — адрес, или файл `P/…`), записи без имени — вставки (`"\n"`, `{commentary_visibility}`).

Что карта ещё не закрывает:
- `via = "flag"` — диспетчер возвращает None (`b7`–`c4`, `c6`–`c9`, `12`, `23`, `24`, `82`, `224`): у раздела нет текста. Все они стоят в срезе 1 профилей; ядро проверяет, есть ли такой раздел в срезе, и по этому выводит другие разделы (П5-2 часть 4). (Раньше здесь стояло «assembler», потом «other_harness» — оба вывода неверны, поправлено в части 4.) Стек безопасности — не отдельные разделы, а значение переменной `{prompt_injection_defense}` блока `shared/safety_security_policy.md` (см. «Системный текст подагента»).
- `via = "unresolved"` — `0x64` (варианты 0–9) и `0x75`: ветка зовёт функцию через `0x29ace370` / `0x29ace378`; это профили 1–5 («не опознан»).
- Разделы `d0` с вариантами 0–`15` — заметки хода (`turn_directives/*`), в системный текст не идут.
- Правила отрисовки (когда блок `anthropic/cedar/*` вместо обычного; `agent/memory.md` и `agent/memory_read_only.md` в одном разделе) — в сборщике, следующая часть.

**Глубокое исследование** (профиль 6, `0x290bbf50`: `66 67 68 12e 69 6a 635 6b 6c 13b`). Порядок блоков в `agent/research/src/lib.rs` приведён к профилю: who_you_are, your_environment, assignment, tools, how_you_work, discretion (раздел `635`), safety, reporting_back. Было: who_you_are, assignment, how_you_work, tools, reporting_back, discretion, safety, security_authority, your_environment. Своё: `security_authority` — в конце, его место за сборщиком. Чего ещё нет: `shared/who_built_you` (раздел `66`), раздел времени `12e`, общие блоки раздела `635` (safety_core_detached, safety_self_modification_detached, safety_sensitive_attributes, discretion_alignment_task, safety_security_policy), `shared/runtime` (`13b`) — придут со сборкой по профилю.

У браузерного окна, монитора подагентов и сжатия профиля в таблице нет: их тексты собираются не по профилю (`prompts/browser/`, `prompts/subagent_monitor/`, `prompts/compaction/`).

Тест — `agent/hamlet/tests/profiles.rs`: вся таблица (70 профилей, каждый номер раздела в карте), порядок разделов ролей Hamlet, чья сборка ещё не по профилю, = массивы профилей ядра, блоки глубокого исследования в порядке профиля, карта на известных разделах.

Профили:

| Номер | Адрес | Роль | У Hamlet / строка плана | Срезы (адрес, разделов) |
|---|---|---|---|---|
| main | `0x290bbdd0` | главный разговор (`main_chat`) | agent/hamlet/src/main_prompt.rs (П5-1) | `0x9904d4` (21) · `0x9904fe` (42) · `0x990552` (25) |
| 0 | `0x290bbe00` | подагент (`subagent`) | agent/hamlet/src/subagent_prompt.rs (П5-2 ч.1); шаги workflow — тот же профиль (ч.3) | `0x990584` (13) · `0x99059e` (14) · `0x9905ba` (22) |
| 1 | `0x290bbe38` | исполнитель окна браузера (`browser_planner`): раздел 0x64 в.0 — `browser/system.md`, `browser/pages.md` | agent/browser/src/planner.rs (П5-2 ч.3) | `0x4a628c` (2) · `0x9905e6` (1) |
| 2 | `0x290bbe70` | специалист входа (`browser_login`): 0x64 в.2 | П4к-2 | `0x4a7c64` (2) · `0x9905e8` (1) |
| 3 | `0x290bbea8` | специалист покупки (`browser_checkout`): 0x64 в.4 | П4к-4 | `0x4a662c` (2) · `0x4a8440` (2) |
| 4 | `0x290bbee0` | специалист проверок ботов (`browser_bot_challenge`): 0x64 в.6 | П4к-3 | `0x4a93f0` (2) · `0x9905ea` (1) |
| 5 | `0x290bbf18` | poppy (`browser_poppy`): 0x64 в.8 | П4к-3 | `0x4a5748` (2) · `0x9905ec` (1) |
| 6 | `0x290bbf50` | глубокое исследование (`deep_research`) | agent/research/src/lib.rs | `0x9905ee` (10) |
| 7 | `0x290bbf88` | сборщик идей (`ideas_builder`) | П22 | `0x990602` (18) · `0x99059e` (14) · `0x9905ba` (22) |
| 8 | `0x290bbfc0` | синхронизация устройств (`device_sync`) | П21 | `0x990626` (7) · `0x990634` (13) · `0x9905ba` (22) |
| 9 | `0x290bbff8` | загрузка медиа (`media_upload`) | П14 | `0x99064e` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 10 | `0x290bc030` | работник наблюдателя (hooks) (`event_hook`) | agent/hamlet/src/background_prompt.rs (П5-2 ч.3) | `0x99065a` (7) · `0x4b0fa0` (16) · `0x9905ba` (22) |
| 11 | `0x290bc068` | работник контекста (`context_worker`) | П13 | `0x990668` (6) · `0x4b0fa0` (16) · `0x9905ba` (22) |
| 12 | `0x290bc0a0` | синхронизация с другим агентом (`peer_sync`) | П19 | `0x49af90` (4) · `0x990674` (13) · `0x9905ba` (22) |
| 13 | `0x290bc0d8` | работник контекста (второй вариант) (`context_worker`) | П13 | `0x990668` (6) · `0x99068e` (18) · `0x9905ba` (22) |
| 14 | `0x290bc110` | проактивность (`proactivity`) | П13 | `0x4ac194` (2) · `0x4696c0` (8) · `0x9905ba` (22) |
| 15 | `0x290bc148` | шаг цели (фон) (`objective_step`) | П11 | `0x9906b2` (5) · `0x9906bc` (7) |
| 16 | `0x290bc180` | итог работника (execute_resolve) (`execute_resolve`) | нет части: других работников execute_resolve у Hamlet нет | `0x4a3198` (4) · `0x9906ca` (10) · `0x9905ba` (22) |
| 17 | `0x290bc1b8` | почтовый разговор (`mail_conversation`) | П16 | `0x49e4a0` (4) · `0x9906ca` (10) · `0x9905ba` (22) |
| 18 | `0x290bc1f0` | разговор с компанией (`company_conversation`) | П19 | `0x9906de` (3) · `0x9905ba` (22) |
| 19 | `0x290bc228` | проверяющий (`verification`) | agent/hamlet/src/background_prompt.rs (П5-2 ч.3) | `0x49a180` (4) · `0x9906bc` (7) |
| 20 | `0x290bc260` | побочный чат (`side_chat`) | П6, П19 | `0x9906e4` (21) · `0x99070e` (44) · `0x990766` (23) |
| 21 | `0x290bc298` | чат с другими агентами (`peer_chat`) | П19 | `0x990794` (7) · `0x9907a2` (53) · `0x99080c` (23) |
| 22 | `0x290bc2d0` | работник расписаний (`scheduler_worker`) | agent/hamlet/src/scheduler_prompt.rs (П5-2 ч.2) | `0x99083a` (13) · `0x4b0fa0` (16) · `0x9905ba` (22) |
| 23 | `0x290bc308` | итог расписания (`scheduler_resolution`) | agent/hamlet/src/scheduler_prompt.rs (П5-2 ч.2) | `0x990854` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 24 | `0x290bc340` | проверка веб-артефактов (`web_artifacts_inspector`) | П17 | `0x495ba0` (8) · `0x99059e` (14) · `0x9905ba` (22) |
| 25 | `0x290bc378` | сборщик сайтов (новый) (`spaces_builder_new`) | П17 | `0x990860` (13) · `0x99059e` (14) · `0x9905ba` (22) |
| 26 | `0x290bc3b0` | сборщик сайтов (правка) (`spaces_builder_edit`) | П17 | `0x99087a` (15) · `0x99059e` (14) · `0x9905ba` (22) |
| 27 | `0x290bc3e8` | лёгкий сборщик сайтов (новый) (`spaces_builder_lite_new`) | П17 | `0x4c49a0` (16) · `0x99059e` (14) · `0x9905ba` (22) |
| 28 | `0x290bc420` | лёгкий сборщик сайтов (правка) (`spaces_builder_lite_edit`) | П17 | `0x990898` (17) · `0x99059e` (14) · `0x9905ba` (22) |
| 29 | `0x290bc458` | задача сайта (`space_task`) | П17 | `0x9908ba` (13) · `0x99059e` (14) · `0x9905ba` (22) |
| 30 | `0x290bc490` | сброс памяти (`memory_flush`) | agent/hamlet/src/background_prompt.rs (П5-2 ч.3) | `0x49a850` (4) · `0x99059e` (14) · `0x9905ba` (22) |
| 31 | `0x290bc4c8` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x9908d4` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 32 | `0x290bc500` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x9908e2` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 33 | `0x290bc538` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x9908ee` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 34 | `0x290bc570` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x9908fa` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 35 | `0x290bc5a8` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x990906` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 36 | `0x290bc5e0` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x990914` (10) · `0x99059e` (14) · `0x9905ba` (22) |
| 37 | `0x290bc618` | самоулучшение: ideas (`self_improvement_ideas`) | П22 | `0x990928` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 38 | `0x290bc650` | самоулучшение: product_step (`self_improvement_product_step`) | П11 | `0x990936` (6) · `0x9906bc` (7) |
| 39 | `0x290bc688` | самоулучшение: product_step (`self_improvement_product_step`) | П11 | `0x990942` (6) · `0x9906bc` (7) |
| 40 | `0x290bc6c0` | самоулучшение: product_step (`self_improvement_product_step`) | П11 | `0x99094e` (6) · `0x9906bc` (7) |
| 41 | `0x290bc6f8` | самоулучшение: product_step (`self_improvement_product_step`) | П11 | `0x99095a` (6) · `0x9906bc` (7) |
| 42 | `0x290bc730` | самоулучшение: relationships (`self_improvement_relationships`) | П11 | `0x990966` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 43 | `0x290bc768` | самоулучшение: relationships (`self_improvement_relationships`) | П11 | `0x990972` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 44 | `0x290bc7a0` | самоулучшение: relationships (`self_improvement_relationships`) | П11 | `0x99097e` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 45 | `0x290bc7d8` | самоулучшение: shopping (`self_improvement_shopping`) | П20 | `0x99098a` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 46 | `0x290bc810` | самоулучшение: shopping (`self_improvement_shopping`) | П20 | `0x990996` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 47 | `0x290bc848` | самоулучшение: travel (`self_improvement_travel`) | П20 | `0x9909a2` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 48 | `0x290bc880` | самоулучшение: travel (`self_improvement_travel`) | П20 | `0x9909ae` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 49 | `0x290bc8b8` | самоулучшение: alignment (`self_improvement_alignment`) | П11 | `0x9909ba` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 50 | `0x290bc8f0` | самоулучшение: alignment (`self_improvement_alignment`) | П11 | `0x9909c8` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 51 | `0x290bc928` | самоулучшение: alignment (`self_improvement_alignment`) | П11 | `0x9909d6` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 52 | `0x290bc960` | самоулучшение: memory (`self_improvement_memory`) | П11 | `0x9909e4` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 53 | `0x290bc998` | самоулучшение: memory (`self_improvement_memory`) | П11 | `0x9909f0` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 54 | `0x290bc9d0` | самоулучшение: memory (`self_improvement_memory`) | П11 | `0x9909fc` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 55 | `0x290bca08` | самоулучшение: studying (`self_improvement_studying`) | П11 | `0x990a08` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 56 | `0x290bca40` | самоулучшение: studying (`self_improvement_studying`) | П11 | `0x990a16` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 57 | `0x290bca78` | самоулучшение: studying (`self_improvement_studying`) | П11 | `0x990a24` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 58 | `0x290bcab0` | самоулучшение: studying (`self_improvement_studying`) | П11 | `0x990a32` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 59 | `0x290bcae8` | самоулучшение: skill_improvement (`self_improvement_skill_improvement`) | П11 | `0x990a40` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 60 | `0x290bcb20` | самоулучшение: skill_improvement (`self_improvement_skill_improvement`) | П11 | `0x990a4c` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 61 | `0x290bcb58` | самоулучшение: goals_bookkeeping (`self_improvement_goals_bookkeeping`) | П11 | `0x48c3f0` (8) · `0x99059e` (14) · `0x9905ba` (22) |
| 62 | `0x290bcb90` | самоулучшение: goals_bookkeeping (`self_improvement_goals_bookkeeping`) | П11 | `0x456f60` (8) · `0x99059e` (14) · `0x9905ba` (22) |
| 63 | `0x290bcbc8` | самоулучшение: goals_bookkeeping (`self_improvement_goals_bookkeeping`) | П11 | `0x454a60` (8) · `0x99059e` (14) · `0x9905ba` (22) |
| 64 | `0x290bcc00` | самоулучшение: goals_bookkeeping (`self_improvement_goals_bookkeeping`) | П11 | `0x990a58` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 65 | `0x290bcc38` | самоулучшение: proactive_notifier (`self_improvement_proactive_notifier`) | П13 | `0x990a66` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 66 | `0x290bcc70` | самоулучшение: proactive_notifier (`self_improvement_proactive_notifier`) | П13 | `0x990a72` (7) · `0x99059e` (14) · `0x9905ba` (22) |
| 67 | `0x290bcca8` | самоулучшение: flight_tracking (`self_improvement_flight_tracking`) | П20 | `0x990a80` (6) · `0x99059e` (14) · `0x9905ba` (22) |
| 68 | `0x290bcce0` | самоулучшение: flight_tracking (`self_improvement_flight_tracking`) | П20 | `0x990a8c` (6) · `0x99059e` (14) · `0x9905ba` (22) |

## Системный текст подагента — сборка по профилю 0 (П5-2, часть 1, продолжение)

**Порядок.** Профиль 0 (`0x290bbe00`), срез 0 (`0x990584`): `43 45 12e 132 39 3c 36 3f 35 42 44 2f 13b`. Раздел → записи — `prompts/profiles/sections.toml`. Сборка — `agent/hamlet/src/subagent_prompt.rs`: записи подряд, без разделителей (свёртка `0xbff27e0`), стыки — переводы строк самих блоков; запись — текст блока с подставленными переменными (`0x29aacef8`); записи без имени — вставки ядра (`"\n"` в разделе `2f`). Затем личный вариант меток (`personal`); края — без переводов строки (своё, как у главного). За текстом — дополнения владельца из `system.md` (своё, как у главного, `main_prompt::with_owner`) и файлы дома (`Prefix::with_system`). Подагент глубины 1 — `inherits = true`, свои подагенты есть; глубины 2 — изолирован, своих нет (`agent/hamlet/src/lib.rs`).

**Главный профиль** (срез 0 `0 27 30 3 1 2 32 31 37 38 34 4 5 6 7 2d 8 9 a b 3b`) с `main_prompt.rs` совпадает. Разделы `27` и `3b` в снимке пусты:
- `27` — `chat/conversation_context.md` («## Chats You're Part Of»: боковые чаты, группы, разговоры агент — агент). Заполняется, когда у агента есть другие чаты; у Hamlet чат один — нет части → П6 (боковые чаты), П19 (группы, агент — агент);
- `3b` — `shared/runtime.md` с `{runtime_facts}` (`0xcd4ea40`: сведения о машине через « | » — общие ядра клетки, Secure/Confidential, имя модели). Отрисовщик `0xcd56910` пропускает раздел, когда флаг (аргумент `r8`, поле `0x128`) не поднят. Клетки исполнения у Hamlet нет → П8. У подагента то же — раздел `13b` (`shared/runtime`).

**Таблица заполнения сборщика `0xcdbbdf0`** (28 переменных, имена — `0xcdbd87e`…`0xcdbdfce`). У подагента:

| переменная | откуда в ядре | у Hamlet |
|---|---|---|
| `subagent_identity_intro` | постоянная `0xa14f2c` | `prompts/agent/subagent_identity_intro.md` |
| `requester_description` | постоянная `0xa15016` (строителю идей — `0xa14faf`) | `prompts/agent/requester_description.md` |
| `history_inheritance_section` | `0xcdc26d0(изолирован, есть subagent.spawn, глубина > 0)`: наследует — `0xa15cd4`; изолирован при глубине > 0 — `0xa15a18`, иначе `0xa15b56`; без `subagent.spawn` — плюс `0xa15c7c` («You work alone…») | `prompts/agent/history_inheritance_{inherited,isolated,alone}.md`; глубина 1 — inherited, глубина 2 — isolated + alone |
| `time_tag_placement` | `0xcd51dd0`: бит поля `+0x383` контекста отрисовки поднят — `chat/time_tag_after_input` («are followed by», `0x290dd878`), нет — `chat/time_tag_before_input` («are prepended with», `0x290dd830`). Главный (`0xcd51461`) и сборщик агентов (`0xcdbd20b`, копия поля `0xb5b(%rsp)`; без контекста — 0, `0xcdbc1c0`) читают одно поле | before, как в снимке главного; метка у Hamlet — перед сообщением и у главного (`Conversation::push_user`), и у подагента (`Subagents::set_time_tag`) |
| `nearby_dates_reference` | `0xcd52990`: текст, когда в заметке есть соседние даты | пусто: соседних дат в заметке Hamlet нет (как у главного) |
| `date_batch_check_guidance` | `0xcd54390`: `chat/date_batch_check_available` / `…_unavailable` | available (как у главного: `hamlet.exec` есть всегда) |
| `cron_created_acknowledgement` | `0xcd54910`: пусто при глубине > 0 или варианте 7; иначе файл без краёв + « » (`0x3be75c`) | пусто — подагент |
| `subagents_section` | `0xcdc2560`: пусто без `subagent.spawn`; иначе `0xcd545c0`: учение `session/subagents_doctrine.md` (`0xcd53100`, клаузы глубины > 0 — `blocks/subagents/subagent/*`, таблица `0x290e09d0`; `extra_delegation_bullet` — «`\n- {}`» `0x33beeb`, `synthesis_sentence` — «` {}`» `0x3639e8`) и `session/subagents_status.md` (`asker`, `failure_action`, без краёв `0x29aa2030`) в формате `"{}\n\n{}"` (`0x385945`) | так же; `extra_delegation_bullet` пуст: пункт о том, что подагенты не делают артефактов, — артефактов у Hamlet нет → П17 |
| `prompt_injection_defense` | стек безопасности: `0xcd52e60` берёт таблицу по варианту раздела `0x35`/`0x33` (подагент — `0x35` v0 → `0x290546e8`, 12 записей), записи подряд, без краёв, формат `"\n{}\n\n"` (`0x393cc5`) | так же, 12 блоков `prompts/shared/security_*`, `prompts/agent/security_*` |
| `system_prompt_copy_file` | у агентов пусто | пусто |
| `year_anchor_line`, `current_date_line` | «The year is {y}, not {y-1}.» (`0x2e6831`); строка даты | в тексте блоков подагента нет (запись `date_time_awareness_head` их перечисляет, текст — нет) — не нужны |
| `temporal_awareness_window` (`0xcdc2380`), `requester_source`, `skills_section` | | в блоках подагента нет; у других ролей — с их сборкой (части 2–3) |
| `tool_capabilities`, `child_depth` | «running commands, handling files, searching the web[, building web artifacts]» по глубине; `0xce538d0` | в блоках подагента нет |
| `artifacts_environment_entry` | `shared/environment/artifacts.md` при инструментах артефактов | нет части → П17 (запись остаётся пустой) |
| `artifact_capability`, `artifact_image_guidance`, `artifact_content_playbooks`, `lite_*_verify_steps` | при инструментах артефактов | нет части → П17 |
| `current_location_section` | `0xcd549f0`: главный и `device.invoke` | нет части → П21 |
| `runtime_facts` | `0xcd4ea40` | нет части → П8 (раздел `13b` не уходит) |
| `ideas_build_hand_off_tool_name` | строитель идей | нет части → П22 |

**Блоки профиля, которых у Hamlet нет** (тест `ABSENT`, `agent/hamlet/tests/subagent_prompt.rs`; в `prompts/` их не кладём):
- `agent/environment_chats` — главный и боковые чаты → П6;
- `agent/environment_feed` — лента → П22;
- `shared/environment/devices` — устройства → П21;
- `anthropic/cedar/tools_runtime_pointer` — только при поиске инструментов на стороне поставщика; у Hamlet загрузка своя;
- `shared/discretion_connections` — весь в метке `agent-to-agent-only` → П19;
- `shared/runtime` (`13b`) → П8.

Условия по набору инструментов (как у главного): раздел `36` — при `cron.add`, `3f` — при `hamlet.memory_search`, `3c` — при своих подагентах, `shared/tools_runtime_pointer` — при отложенных пространствах (метки `client-tool-loading-only` снимаются, `Tools::client_loading`).

**Тексты.** Новые блоки — `prompts/agent/*` (28), `prompts/shared/*` (24), `prompts/subagents/subagent/*` (9; `extra_delegation_bullet` не кладём — П17) — файлы `P/hatch-agent/prompts/blocks/…` и тексты ядра как есть; вшиты в ядро: `shared/tools_intro` (`0x7e962f`), `shared/subagents` (`0x7aa1ed`), `shared/memory_recall_doctrine` (`0x7e9e25`), `agent/history_inheritance_*`, `agent/subagent_identity_intro`, `agent/requester_description` (адреса — в таблице). Отличия П10: метки `personal-muse-only`/`group-muse-only` → `personal-only`/`group-only`, `Muse` → `Hamlet`, `~/docs/muse.md` → `~/docs/hamlet.md`, `/opt/muse/` → `/opt/hamlet/`; в `agent/who_you_are.md` «Meta maintains these docs» → «The Hamlet team maintains these docs» (как в `chat/how_you_work.md`). Блоки, которые упоминают части, которых у Hamlet пока нет (Secure Vault в `agent/safety_credentials`, медиа в `agent/safety_oversight`), — как у главного (`user_chat/security_secrets`, `chat/safety_oversight`): блок целиком о части — не уходит, упоминание внутри общего правила — остаётся.

Своё:

**Метка времени входа** (продолжение части 1). Форма ядра: `0xbc4b050` склеивает через пробел `[Day YYYY-MM-DD HH:MM:SS TZ]` (`0xf19a160` → `0xf19a3d0`: дни `Mon`…`Sun`, `{:04}-{:02}-{:02} {:02}:{:02}:{:02}` — `0xb38799`, `%Z`, скобки — `0x2dcb58`) и `[client_timezone=…]` (`0x409b29`); это пункт контекста входа (`0xbc4b3a0`, вызов `0xbc4c503`), ставится, когда у входа есть пояс. Сборку контекста входа зовут главный ход (`0xbd0ab08` в `0xbd0a0b0`) и подготовка хода работника (`hatch-db-context/src/worker_turns/staging.rs`, `0xc59e74e` в `0xc59c3e0`: пояс источника — `0x290(%rsp)`), так что у подагентов muse свои метки на каждое сообщение. У Hamlet: `daemon::time_tag` (было своё «Current date and time: …»); пояс — `[agent].timezone` в `agent.toml`: нет — меток нет ни у главного, ни у подагентов, как у muse; неразборчивый — ошибка загрузки `agent.toml`. У главного — заметкой перед строкой владельца, у подагента — перед заданием и каждым `subagent.send` (`Subagents::set_time_tag`, наследуется глубже). Работник расписаний и прочие роли — с их сборкой (части 2–3 П5-2).

**Раздел `subagent_spawning.md`** убран из текста подагента и из `spawning_prefix`: `{subagent_spawning_section}` нет ни в одном блоке (ни в файлах `P`, ни в ядре; имя — только в таблице переменных сборщика, `0xa09cc3`), профиль его не рисует. Файл `prompts/subagent/subagent_spawning.md` удалён.

**Шаги сохранённых работ** (`workflow`) пока получают `system.md` владельца; их профиль — в части 3 П5-2 вместе с `event_hook` и остальными.

Тест — `agent/hamlet/tests/subagent_prompt.rs`: записи — в порядке блоков профиля 0 (раздел за разделом) без `ABSENT`, и текст каждой стоит в собранном тексте за предыдущей; разделы по набору инструментов; наследование по глубине; подстановки ядра (стек, учение, пустые переменные), ни одной незаполненной `{переменной}`, меток и «muse».

## Системные тексты работника расписаний (П5-2, часть 2)

Сборка — `agent/hamlet/src/scheduler_prompt.rs`, так же, как у подагента (записи подряд, личный вариант меток, края без переводов строки, за текстом — `system.md` владельца). Подключение — `agent/hamlet/src/lib.rs`, `Scheduler::cron_texts`. Раньше работник получал только `system.md` владельца.

**Исполнение** — профиль 22 (`0x290bc2d0`), срез 0 `0x99083a`: `96 95 97 12e 41 339 3c 40 235 133 44 2f 43b`. Это текст фазы исполнения: начало подсказки прогона (`Prefix`).

**Итог** — профиль 23 (`0x290bc308`), срез 0 `0x990854`: `9b 99 98 9a 835 63b`. Это системный текст хода итога (`worker::Kind::Cron { system }`). Собирается на момент прогона, потому что в нём строка года.

**Метка времени** стоит перед сообщением работнику (`Begin::Messages`), как у подагента. Основание — подготовка хода работника muse, `worker_turns/staging.rs`. Без пояса метки нет.

**Переменные:**

| переменная | откуда в ядре | у Hamlet |
|---|---|---|
| `time_tag_placement` | `0xcd51dd0`, поле `+0x383` | before |
| `nearby_dates_reference` | `0xcd52990`: при `exec` — пусто | пусто, `exec` у работника есть |
| `date_batch_check_guidance` | `0xcd54390` | available |
| `artifacts_environment_entry` | при артефактах | пусто → П17 |
| `subagents_section` (работник) | `0xcdc2560`, см. ниже | раздел `3c` не уходит: в наборе работника Hamlet нет `subagent.*` |
| `prompt_injection_defense` (работник) | раздел `0x33` вариант 1: `0xcd52e60` → таблица `0x290ebc58`[1] = `0x29054388`, длина `0xa23ff0`[1] = 864 байта, то есть 12 записей; без краёв, без обёртки (обёртка `"\n{}\n\n"` — только у `0x35`, `0xcd52fb6`) | 12 блоков: `chat/security_authority`, `agent/security_*_worker` и общие |
| `prompt_injection_defense` (итог) | раздел `0x35` вариант 8: `0x29055e88`, `0x288` байт, то есть 9 записей (`0xcd5309b`); формат `"\n{}\n\n"` | 9 блоков: `scheduler_resolution/security_authority`, `shared/security_gates`, `shared/security_precall_checklist` и общие |
| `temporal_awareness_window` (итог) | `0xcdc2380` → `0xcdbf080`, подробно ниже | по коду, вариант с `exec` |
| `system_prompt_copy_file` | у агентов пусто | пусто |
| `runtime_facts` (`43b`, `63b`) | `0xcd4ea40` | нет части → П8 |

`subagents_section` по `0xcdc2560`:
- без `subagent.spawn` — пусто;
- при глубине больше 0 — учение подагента (`0x290e09d0`);
- при глубине 0 (корень, например работник) — `## Subagents …` (`0x37c9e5`) и строка `Spawn subagents with \`{имя}\` …` (`0x3aa080`; имя — у записи инструмента, по умолчанию `subagent.spawn`, `0x9ebec5`). Понадобится, когда у работника будут подагенты.

**`temporal_awareness_window`.** Шаблон `0xa147a9` (формат Rust: `\xc0` — аргумент, литерал — длина байтом или `\x80` + u16) лежит в `prompts/shared/temporal_awareness_window.md`. Имена мест — свои: `{year_anchor}`, `{temporal_guidance}`, `{nearby_dates}`, `{weekday_clause}`, `{weekday_rule}`.
- `{year_anchor}` — «The year is {y}, not {y-1}.» (`0x2e6831`) + `"\n\n"` (`0x414d26`). Дата берётся в поясе, без пояса — `UTC` (`0x26a61690`: свой пояс, запасной, иначе `UTC`, `0x26a616f2`).
- `{temporal_guidance}` — `shared/temporal_guidance_detached` (`0x290decc8`) без краёв.
- С `exec` (`0xcdc2479`): `{nearby_dates}` пусто, `{weekday_clause}` — `0xa14246`, `{weekday_rule}` — `0xa14448` (`prompts/shared/temporal_weekday_*_exec.md`, П10: `muse.exec` → `hamlet.exec`). `exec` — в роли `scheduler_cron_resolve`.
- Без `exec` выбор идёт по полю `+0x384`. Вариант «listed above» перечисляет соседние даты: `0x26a60870`, «Nearby dates (in the user's timezone): » и 7 дней от −3 до +3 через « | » (`0x26a64a30`: `%A, %B %-d, %Y`, `(yesterday)`/`(today)`/`(tomorrow)`). У Hamlet этот вариант не нужен, тексты не взяты.

**Блоки, которых у Hamlet нет:**
- `agent/environment_chats` → П6;
- `agent/environment_feed` → П22;
- `anthropic/cedar/tools_runtime_pointer` — поиск инструментов на стороне поставщика;
- `shared/discretion_connections` → П19;
- `shared/runtime` → П8;
- `scheduler_resolution/tools` («The full tool list … is in the Runtime section below») — раздела `Runtime` нет, П8;
- `agent/environment_browser_worker` — только при `browser.browse`: у работника Hamlet его нет, окна расписаний — П4к-3;
- `shared/subagents` — см. выше.

**Тексты:** 26 новых файлов P и ядра. Правки П10: метки, `~/docs/muse.md` → `~/docs/hamlet.md`, «Meta maintains these docs» → «The Hamlet team maintains these docs» (`scheduler_worker/who_you_are`), `Muse` → `Hamlet` (`agent/safety_credentials_worker`: Wallet — П20, упоминание внутри общего правила, как у главного).

**Тест** — `agent/hamlet/tests/scheduler_prompt.rs`:
- записи обоих профилей идут в порядке блоков ядра без перечисленных, и по собранному тексту;
- подстановки: стеки, окно времени, строка года по поясу;
- разделы по набору инструментов;
- нет `{переменных}`, меток, muse.

Сквозной — `assembly.rs` (`conversation_writes_searches_schedules_and_wakes_on_handoff`): у исполнения текст профиля 22 и метка перед сообщением, у итога — текст профиля 23 со строкой года.

## Системные тексты фоновых ролей и браузера (П5-2, часть 3)

**Какой профиль у какой роли.** Профиль выбирается по виду контекста. Сборщик `0xcdbbdf0` берёт тег контекста (1–14) и по таблице `0xa242a0` получает индекс профиля в таблице описателей `0x290d0708`: 0 — главный, n+1 — профиль n. Без контекста индекс — 1, то есть профиль 0. Соответствие видов:
- 1–5, 7 — профили 24–29 (артефакты, пространства);
- 6 — профиль 0 (подагент);
- 8–12 — профиль 1 (браузер);
- 13 — профиль 6 (глубокое исследование);
- 14 — профиль 7 (строитель идей).

Фоновые профили 10/16/19/30 выбираются своими работниками.

| профиль | роль | у Hamlet |
|---|---|---|
| 10 `0x290bc030`, срез `0x99065a`: `7f 7d 3d 7e 7c 335 33b` | работник наблюдателя | `background_prompt::event_hook`, `Scheduler::hook_system` (на момент прогона) |
| 16 `0x290bc180`, срез `0x4a3198`: `126 3d 435 33b` | общий работник execute_resolve (`background/execute_resolve`) | нет части: у Hamlet через execute_resolve идут только наблюдатель (профиль 10) и проверка (профиль 19) |
| 19 `0x290bc228`, срез `0x49a180`: `326 3d 435 33b` | проверяющий (проверка ответа и отчётов расписаний) | `background_prompt::verification`, `Verifier::system` (на момент проверки) |
| 30 `0x290bc490`, срез `0x49a850`: `9c 3d 735 73b` | сброс памяти (перед сжатием и периодический) | `background_prompt::memory_flush`, `Compactor::flush_system`. Раньше ход сброса шёл с системным текстом главного |
| 0 | шаги workflow (роль `workflow_subagent_depth1`) | `subagent_prompt::assemble(набор шага, inherits = false, can_spawn = false)`. Своего профиля у шага в таблице нет: это подагент (вид 6), изолированный, без своих подагентов |
| 1–5 | браузер: исполнитель, вход, покупка, проверки ботов, poppy | см. ниже |

**Переменные:**
- `temporal_awareness_window` — `crate::temporal::window`, общий модуль; шаблон и правила — как в части 2.
  - С `exec` (`hamlet.exec` в наборе: наблюдатель — роль `execute_resolve_worker`, сброс — роль `memory_flush`): `{nearby_dates}` пуст, правило с `date -d`.
  - Без `exec` (проверяющий, роль `verifier`), при поле `+0x384` = 0 (в метке времени Hamlet соседних дат нет): `{nearby_dates}` = «Nearby dates (in the user's timezone): » (`0x3b06c0`) и 7 дней от −3 до +3 через « | » (`0x26a60870`), затем `"\n\n"`.
  - День соседних дат (`0x26a64a30`): «{день} {дата}» (`0x3be7b5`); день — короткое имя из таблицы `0x298df0c8` (воскресенье — `0x2caa505`), дата — `%Y-%m-%d` (`0x498548`). Пометки: −1 `(yesterday)` (`0x32d45a`), 0 `(today)` (`0x3f7d82`), +1 `(tomorrow)` (`0x3b06b2`). Правила — `0xa143ab`, `0xa1470a` (`prompts/shared/temporal_*_listed.md`, `temporal_nearby_dates.md`).
  - Без `exec` при поле `+0x384` ≠ 0: «listed next to the current time» (`0xa142fb`, `0xa14658`), соседние даты стоят в метке. У Hamlet не нужно.
- `prompt_injection_defense` — раздел `0x35`, обёртка `"\n{}\n\n"`:
  - наблюдатель — вариант 3 (`0x29054da8`, 12 записей; `shared/security_authority_detached_worker`);
  - проверяющий — вариант 4 (`0x29055108`, 12; `background/security_authority`);
  - сброс — вариант 7 (`0x29055c00`, 9; `memory_flush/security_authority`).
- `runtime_facts` (`33b`, `73b`) → П8.

Блоки, которых у Hamlet нет: `shared/discretion_connections` (П19), `shared/runtime` (П8). Блок `event_hook/tools` уходит целиком: в нём правила вызовов и упоминание раздела `Runtime` (П8). У `scheduler_resolution/tools` упоминание `Runtime` — весь блок, поэтому тот блок не уходит.

**Браузер.** Раздел `0x64` («не опознан» в части 1) выбирает тексты по варианту из таблицы `0x2906bd30`: вызов `0x29ace370` → `0xbd3db90`; записи по 40 байт — имя, путь, вид. Это профили браузерного исполнителя:

| профиль | срез 0 | срез 1 |
|---|---|---|
| 1 — исполнитель | в.0: `browser/system.md`, `browser/pages.md` | в.1: 12 файлов `browser/instructions/*` и `tools/copyrighted_material.md` |
| 2 — вход | в.2: `workflows/login/system.md`, `pages.md` | в.3 |
| 3 — покупка | в.4: `workflows/checkout/system.md`, `pages.md` | в.5 |
| 4 — проверки ботов | в.6: `workflows/bot_challenge/system.md`, `pages.md` | в.7 |
| 5 — poppy | в.8: `workflows/poppy/system.md` | в.9 |

Профиль 1 у Hamlet — `agent/browser/src/planner.rs`; тест порядка `browser_planner_follows_profile` (`agent/hamlet/tests/profiles.rs`) проверяет, что `system.md` стоит перед `pages.md`. Профили 2–5 → П4к-2/П4к-3/П4к-4. В `prompts/profiles/sections.toml` разделы `64`–`964` — `via = "browser_table"` с блоками. «Не опознан» остаётся только `0x75`.

**Блоки, которые уходят целиком, хотя части, о которых они говорят, у Hamlet появятся позже** (правило П5-1: блок целиком):
- `shared/tool_rules_detached_approvals` (работник расписаний) — согласия на вызовы → П20;
- `agent/safety_credentials`, `agent/safety_credentials_worker`, `shared/safety_credentials_detached_worker`, `shared/safety_credentials_transient_limits` — Secure Vault, ссылки `[credential:…]` → П18, Wallet → П20;
- `agent/safety_oversight`, `chat/safety_oversight` — медиа → П14/П15;
- `scheduler_worker/how_this_run_ends` — передача работы браузеру → П4к-3.

**Тексты:** 14 новых блоков (P и ядро), правки П10 — как в частях 1–2. Тексты наблюдателя — прежние `prompts/hooks/*` (совпадают с muse); `agent/cron::hooks::worker_system` удалён.

**Тесты:**
- `agent/hamlet/tests/background_prompt.rs` — порядок по собранному тексту профилей 10/19/30, стеки, окно времени с `exec` и без него (соседние даты в поясе);
- `assembly.rs` — у наблюдателя, проверяющего и сброса в запросах свои тексты;
- `agent/cron/tests/hooks.rs` — текст работника наблюдателя берётся на момент прогона;
- `profiles.rs` — браузер; список констант порядка удалён, все роли проверяются по тексту.

## П5-2, часть 4: срез 1 профиля и браузер

**Где выводится срез 1.** Срез 1 — не «другая обвязка» (так было записано в частях 1–3; это неверно).
- `0xcdbf740` берёт срез 1 через `0xcdb6520` и выводит его разделы по одному через `0xcd54d80` → диспетчер `0xa9b9dc0`. Строки этой функции — «embedded developer template render», «hot-swap prompt has a missing section» (`0x9ec45e`). Это текст developer-сообщения. Его место в разговоре разбирается в части 5.
- Кто зовёт `0xcdbf740`: ход `0xca92e40` (`0xcaaf2c9`, итог в поле `+0x8f90`) и голосовые ходы через `0xcdbf700` (`0xd8d0570`, `0xdb24e50`, `0xdcd3df0`, `0xddc2a30`).
- Другие обходы целого списка берут срез 0 (`0xcd56910`, `0xc4799e0`, `0xa9f0650`) или срез 2 — `turn_directives` (`0xcd552d0`, его зовут из `0xcdb67c0` и `0xcaa6bdb`).
- Остальные места, где читается срез 1 (`0xcdb6520` и `0xcdb1920`), только проверяют, есть ли в нём раздел (сравнение `0xabfb5d0`). По такой проверке выводятся отдельные разделы:
  - `0xf` — `0xb39a370`, при `0x325` или `0xc`;
  - `0x3b` — `0xcdb9b00`, при `0xc8`;
  - `0xcb` — `0xcdbf740`;
  - `0x88`, `0x8c`, `0x28`, `0x29`, `0x65`, `0x8f`, `0xcf` — `0xcdb57c0`, `0xcdb6960`, `0xcdb6cb0`.

**Условие** (`0xca92e40`):
- флаг `+0x8ecc` = `cfg.0x72e0 > 0` или `cfg.0x26e8 == 7` (`0xca97512`–`0xca97554`). Если он стоит, срез 1 не выводится;
- иначе, если готов текст `+0x3270`, берётся он;
- иначе, если контекста `+0xe60` нет, текста нет;
- иначе срез 1 выводится (`0xcaaf241`–`0xcaaf2c9`).

Это те же поля, что решают ветку в `0xcdb1920` (`+0x60` = `cfg.0x72e0`, `+0x50` = `&cfg.0x26e8`; сборка структуры — `0xca98a5a`, `0xca98b05`). При `+0x60 > 0` профиль выбирается по виду (таблица `0x9d842c`: вид 6 → подагент, 8–12 → браузер, 13 → глубокое исследование, 14 → профиль 7, 1–5 → 24–28), при виде 7 — индекс `0x1e`. Итог: у профилей по виду срез 1 не выводится. У профилей по роли (`0xc468b20`) выводится:
- главный (индекс 0);
- индексы `0x9`–`0x18`, то есть профили 8–23: наблюдатель, проверяющий, работник и итог расписаний и другие;
- если роль не нашлась — набор `0xa13f88`.

Индексы 1–6 (подагент и браузер) `0xc468b20` не отдаёт. Developer-сообщение по срезу 1 у главного и ролей по роли — часть 5.

**Браузер.** Тексты `browser/instructions/*` и `presentation_language.md` в ядре дословно не лежат: muse читает их с диска по пути. Ниже — все обращения к описаниям этих файлов (путь, длина, вид). Кроме них, пути лежат в реестре файлов подсказок (записи `inventory`: таблицы методов `0x29065338` и `0x28ed1178`) — это регистрация, не чтение.
- `assignment`, `browsing`, `obstacles`, `asking`, `commitments`, `facts`, `privacy`, `conduct`, `reporting`. Читает только диспетчер (`0xa9baa48`, раздел `0x64` в.1), то есть срез 1 профиля 1. (В части 4 здесь стоял вывод «срез 1 профиля 1 не выводится, файлы убраны» — неверно: его выводит `0xcdb6660` в составном тексте хода, см. часть 5а. Файлы возвращены.)
- `presentation_language.md` (описание `0x28ed1410`). Читает `0x8b1f920`: обрезает края (`0xf76f820`) и подставляет `{locale}` (`0x498590`). Его зовёт сборка задания `0x8b1f310`: строка начала или продолжения, затем задание, «\n\nLanguage:\n» и язык. Сборку зовут `0x97cf670`, `0xa85d260`, `0xc24b8b0`, `0xc252630`. Ещё файл читает итоговая сводка задачи hatch-server (`0x18748550`, `0x1c85b040`) — её у Hamlet нет. У Hamlet язык перенесён в `assignment_message` и `update_message`.
- Системный текст исполнителя — записи `system.md` и `pages.md` подряд (`0x64` в.0, профиль 1), личный вариант меток, края без переводов строки.

**Остальные `instructions/*`** — придут с П4к-2…4, читают:

| файл | кто читает |
|---|---|
| `popup_open`, `popup_tab`, `popup_pending`, `passkey_refused`, `personal_agent_protocol`, `personal_agent_protocol_poppy_reported`, `personal_agent_protocol_specialist`, `ucp_merchant_purchases` | `0x8a96ad0` |
| `site_not_found`, `site_unreachable` | `0x8a90400`, `0x8a908b0` |
| `session_unavailable` | `0x8a908b0`, `0x8a959c0`, `0x9b01140`, `0x9bfc000`, `0xb249940`, `0xcc65190`, `0xd52f7f0` |
| `personal_agent_protocol_poppy_started`, `_poppy_started_open`, `_browsing_disabled` | `0x8a906a0`, `0x8a90790`, `0x990bdf0`, `0xcbf0730` |
| `personal_agent_protocol_poppy_working` | `0x8a900c0`, `0x8a90340`, `0x8a90820` |
| `personal_agent_protocol_joined`, `_joined_guest`, `_join_failed`, `_suggested_page` | `0x9b22d70`, `0xa364ab0`, `0xa364b00`, `0xcc86900` |
| `control_return` и `control_return/*` | `0xa1f6470`; `timed_out` — ещё `0xaac4aa0` |
| `checkout_resumed` | `0xa148d40`, `0xa149260` |
| `relocated` | `0xaa39fb0` |
| `cookies_cleared` | `0xbbad120`, `0xc0a3ba0` |
| `site_requeued` | `0xbbad120`, `0xc390f90` |
| `checkpoint_required` | `0xd530240` |
| `site_concurrency` | `0x922e740`, `0xca24340` |
| `bot_checks`, `specialists`, `specialist_assignment`, `login`, `verification_codes`, `checkout`, `challenges`, `poppy` | только таблица `0x2906bd30` (срез 1 профилей 1–5, в.1/3/5/7/9) — не выводятся |

**Тесты:** `agent/browser/tests/planner_text.rs` — системный текст исполнителя равен `system.md` + `pages.md` по форме профиля, без `instructions/*` и без языка; первое сообщение и продолжение — побайтно по форме `0x8b1f310` с подставленным `locale`. `agent/hamlet/tests/profiles.rs` — `via = "flag"`.

## П5-2, часть 5а: тексты хода — механизм и браузер

**Что muse кладёт в ход.** Ход `0xca92e40` пишет в список хода `+0x69d8` два элемента:
- вид 2 (`0xcaaa2fb`) — `turn_directives`: срез 2 профиля, выводит `0xcdb67c0` (вызов `0xcaa9ff8`);
- вид 4 (`0xcaafc95`) — составной текст `0xcdb6cb0` (`+0x84a0`).

Порядок записи — сначала вид 2, потом вид 4. У обоих внутреннее значение одно: встроенный системный элемент (вариант 0, `0x8000000000000000`: `0xcaaa2e1`, `0xcaafc27`) подвида 1 (байт `+0x20` = 1: `0xcaaa2f3`, `0xcaafc8d`). `0xbc94680` (через `0x29ad9340`, вызов `0xcab0121`) переводит их в сообщения запроса: `0xb5227f0`, ветка `0xb522836`.

**Составной текст `0xcdb6cb0`** — части через «\n\n» (`0x9e0a89`):
- раздел `0xcf`, если он есть в срезе 1;
- `0xcdb6660`: срез 1 (берёт `0xcdb1920`, то есть и у профилей по виду) с фильтром `0xbfcd420`; выводятся только `0x64` (без условий), `0xcc` и `0xcd`/`0xce` (при флагах — часть 5б);
- developer-текст хода (`+0x8f90`): `0xcdb9b00` — `date_grounding_developer_message` (флаг `0xc8`), вывод среза 1 `0xcdbf740` (только у профилей по роли), раздел `0x3b` (`ctx+0x382`). Он хранится по `seq` сообщения хода (`capture_turn_developer_contexts`, `0x16f9283`) и при повторе хода берётся сохранённый (`+0x3270`, `load_turn_developer_contexts`, `0x1d98b540`).

**Формы у поставщиков** (`agent/model`):
- Anthropic (`convert_prompt.rs`, `0x20cd95b0`): вид сообщения — таблица `0x22730cc`, подвид — таблица `0x2273154`. Подвид 0 дописывается к параметру `system` через «\n\n». Подвид 1 (`0x20cd9f72`) — объект `{"role":"system","content":[{"type":"text","text":…}]}` в массиве `messages` на своём месте (`0x20cda00a`, `0x20cda16f`). У Hamlet: при `anthropic.mid_conversation_system_messages` каждая заметка (`Role::Developer`) — отдельным таким сообщением, если место допустимо (правила muse «system section must follow a user message / precede an assistant message or end the array», `strings-all.txt:235512`). Без умения — заметки одним текстом в `<system-reminder>` внутри сообщения пользователя (своё: ветку muse для таких весов не нашли в П1б).
- Chat Completions (`kimi_chat_completions/prompt.rs`, `0x20ad1520`): роль встроенного элемента — из таблицы `0x297732c0` по байту подвида (`0x20ad26da`, длины `0x2269d00`): 0 `system`, 1 `developer`, 2 `user`, 3 `assistant`. У Hamlet при `developer_role` — `{"role":"developer","content":…}` (раньше было `system`). Без умения — `<system-reminder>` (своё).

**Механизм у Hamlet.** `agent_loop::push_turn_texts` / `Conversation::push_turn_texts`: тексты хода кладутся за сообщением владельца этого хода, каждый отдельной заметкой, по порядку (`turn_directives`, потом составной текст); пустые не кладутся. Они лежат в истории хода: после перезапуска история из записи несёт их те же, при сжатии они уходят в сводку вместе со своим ходом.

**Открытый вопрос — место.** Где у muse элементы `+0x69d8` стоят относительно сообщения владельца и истории, по ядру не установлено. Дальше список идёт в оценку и отпечаток запроса (`0xb3fca90` через `0x29ad9348`, `0xcab0556`, «prompt envelope»), место вставки не найдено. За «после сообщения владельца этого хода» говорят:
- имена `turn_directives` / `turn_developer_context`;
- хранение по `seq` сообщения хода, загрузка сразу для нескольких ходов (`… seq = ANY($2) ORDER BY seq`, `0x16f90a6`);
- текст ядра «developer messages: time tags and reminders associated with user inputs» (`0xa08ca5`).

Это вывод, не адрес.

**Сжатие.** Сжатие пишет `first_kept_seq` (`agent_compactions`, `0x172330a`), тексты загружаются по `seq` видимых сообщений. Вывод: тексты ушедших в сводку ходов пропадают, у оставшихся — на месте. Отдельного пересчёта старых текстов не найдено.

**Браузер.** Срез 2 профиля 1 пуст. Составной текст = раздел `0x64` в.1 (`0xcdb6660` выводит `0x64` без условий): записи таблицы `0x2906bd30` подряд — `assignment`, `browsing`, `obstacles`, `asking`, `commitments`, `facts`, `privacy`, `conduct`, `tools/copyrighted_material.md`, `reporting`. `specialists.md` и `bot_checks.md` не берутся целиком (специалисты и проверки ботов — П4к-2…4). Developer-текста хода у профиля по виду нет (`+0x8ecc`). Затем личный вариант меток и края без переводов строки (своё, как у системного текста). У Hamlet — `agent_browser::planner::turn_texts`, кладётся за заданием и за продолжением, начинающим новую работу. Обновление посреди работы приходит в тот же ход, без новых текстов.

**Часть 5б** (не сделано): условия `0xcd552d0` (срез 2), `0xcdbf740` (таблица `0x9d84ec`), флаги `0xcc`/`0xcd`/`0xce` в `0xcdb6660` — затем наполнение у главного и ролей по роли.

**Тесты:**
- `agent/loop/tests/turn_texts.rs` (оба поставщика): с умением и без; порядок двух текстов; тексты те же после перезапуска; после сжатия у старого хода текстов нет;
- `agent/model/tests/anthropic.rs` — форма `content: [{"type":"text"}]`, заметки подряд — отдельными сообщениями;
- `agent/model/tests/openai_compat.rs` — роль `developer`;
- `agent/browser/tests/planner_text.rs` — составной текст исполнителя; `planner.rs` — он за заданием.
