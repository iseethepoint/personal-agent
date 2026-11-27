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
