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
| `browser/presentation_language.md` | `presentation_language.md` | без правок; `{locale}` подставляет код |
| `browser/completion_gate/report_required.md` | `completion_gate/report_required.md` | без правок |
| `browser/instructions/assignment.md` | `instructions/assignment.md` | убрано предложение о передаче работы специалистам |
| `browser/instructions/browsing.md` | `instructions/browsing.md` | убраны «a route in the site's guide» и абзац о гайдах сайтов (П4к-3) |
| `browser/instructions/obstacles.md` | `instructions/obstacles.md` | убрано «Sign-in, checkout and bot checks belong to the specialists.» |
| `browser/instructions/asking.md` | `instructions/asking.md` | убраны обзор корзины и карта из кошелька (покупки — П4к-4) |
| `browser/instructions/reporting.md` | `instructions/reporting.md` | убраны находки специалистов и абзац о результате покупки |
| `browser/instructions/privacy.md` | `instructions/privacy.md` | убраны служба входа и абзац о файлах (`browser.files` нет) |
| `browser/instructions/{conduct,facts,commitments}.md` | те же | без правок, кроме общих |
| `browser/tools/observe.{json,md}` | `tools/observe.*` | убраны поля `image` (осмотр и разметка снимков) и `listen` (звук) и их абзацы; «inspections of saved images» и `look` в последнем абзаце убраны |
| `browser/tools/act.{json,md}` | `tools/act.*` | виды действий — без `drag`, `hold`, `mouse_*`, `program`; поля `look`, `within`, `from_*`, `to_*`, `duration_ms`, `dx`, `dy`, `source` и их абзацы убраны (наведение по описанию и модель наведения — позже); описание `ref` и `timeout_ms` сокращено под это; из кодов отказа убраны `page_busy`, `site_in_use`, `entry_unsupported`, `picker_unavailable`, `pointer_*`; пример `needs_reference` — «choosing an option, ticking a box or scrolling inside an area»; модификатор `Meta` оставлен как у muse — имя клавиши, а не компании; в тесте П10 разрешён белым списком ровно здесь и в `window.rs` (слово ревизора, П4к-1б) |
| `browser/tools/tabs.{json,md}` | `tools/tabs.*` | без правок |
| `browser/tools/report.{json,md}` | `tools/report.*` | убраны поле `artifacts`, абзацы о специалистах и о результате покупки, «preserve useful findings … from agents you called» |
| `browser/tools/request_input.{json,md}` | `tools/request_input.*` | убраны обзор корзины, оплата, абзац об одноразовых кодах и `login.advance` (П4к-2) |

Своё в коде (`agent/browser/src/window.rs`, `planner.rs`; разметки вида у muse в разборе нет — её собирает отдельный «browser broker»): строки вида `- роль "имя" @eN [состояния]` из дерева доступности Chromium; предел текста вида 30 000 знаков; снимок JPEG; склейка фраз результата действий из строк ядра; ответ на `browser.request_input` («The main agent answered:»); порядок блоков системного текста. Модель исполнителя — назначение `BrowserAgent`.

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
| `tool_search/loading_tools.md` | ядро: `blocks/user_chat/loading_tools.md` (`strings-all.txt:59570-59576`), часть `client-tool-loading-only` | метки `client-tool-loading-only` сняты; часть `provider-tool-search-only` (встроенный поиск инструментов Anthropic) не взята; `{deferred_stub_marker}`, `{deferred_function_marker}` подставляет код |
| `tool_search/tools.json` | ядро: описание `tool_search.load_tool_namespace` и поле `paths` (около `0xa10827`, `0xa109da`), маркеры (`0x9d9b51`), ответы загрузки (`0x6daf7e`, `0x990e85`, `0x96daa7`), ответ на вызов незагруженной функции (`0x99117a`), краткие описания пространств `cron`, `hooks`, `user_goal`, `tracking`, `workflow`, `tool_search` (`0x2cc9b77`–`0x2ccbae2`) | `muse.connector_search`, `muse.skill_search` → `hamlet.*` (П10); из подсказки «unavailable» убрано «Built-in presentation tools, including shopping cards, are available in their tool namespaces.» (карточек покупок нет); `{n}`, `{names}`, `{name}` — места аргументов шаблонов ядра |

Своё (код `agent/loop/src/tool_search.rs`): какие пространства отложены у главного агента (`cron`, `hooks`, `user_goal`, `tracking`, `workflow` — у кого есть полное описание пространства; полного списка в разборе нет); вид строки списка пространств и место блока — после системного текста; вид схем в ответе загрузки; загруженные функции добавляются в конец списка инструментов запроса; какой текст ответа загрузки в каком случае.

## Разборы ядра без перенесённых текстов

- Нижний предел частоты расписаний («Please choose a cron interval of at least … interval is too frequent. Nothing was saved.», шаблон `0x437836`): функция `0xfdfd850` (машина async, вызывается из `0xfe014d7`, `0xfe05afc`). Предел `r13` = clamp(значение политики `r14`, 60, 3600) (`0xfdfe4c8`–`0xfdfe4e1`); запрошенное (`0x40(%rbx)`) ≥ max(предел, 300) — принято (`0xfdfe4f4`–`0xfdfe50e`); отказ — только когда флаг политики (`0x50(%rbx)`, итог вызова `0xfd597f0`) и запрошенное < предела (`0xfdfe514`–`0xfdfe51f`). Без политики `r13` = 60 и отказа нет. У Hamlet политики нет — отказа нет, как у muse без неё (решение ревизора, П4л-2).
- Предел вызовов workflow («workflow agent call limit exceeded: max {} calls per run», шаблон `0x3d426a`): сравнение в `0x277a4dfa` (и у ссылок `0x277a6c6d`, `0x277ada15`) — счётчик (`xadd 0x84`, минус `0x80`, +1) против поля `0x88` состояния, взятого через thread-local; где поле заполняется — не найдено (П4л-2).

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
