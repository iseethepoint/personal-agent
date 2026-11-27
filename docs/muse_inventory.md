# Опись muse для личного агента (П1)

Основание: строка П1 `docs/PLAN.md`, карточка `docs/cards/P1-inventory.md`. Мерило — только `iseethepoint/muse-live` (снимок 25.11.2026, ядро `hatch-2026-11-25-v2.bin`). `home/hatch/workspace/muse-clone-rs` не читался и не используется. Данные пользователя muse из `home/hatch` сюда не скопированы: из `home/hatch` взяты только ссылки на справочник `docs/` (его пишет Meta, а не пользователь) и на имена файлов разметки домашней папки.

## Как читать ссылки

Все пути — от корня muse-live. Сокращения:

- `R/` = `rootfs-full/`
- `P/` = `rootfs-full/opt/hatch-private/prompts/hatch-engine/crates/` (тексты движка)
- `A/` = `core-v2-analysis/` (разбор ядра)

Номер строки — настоящий номер строки файла, проверен `grep -n` / `sed -n` / `git grep -n`.

- `A/source-files.txt:N` — в строке N стоит имя исходника ядра (самого кода нет).
- `A/strings-all.txt:N` — строка N файла строк ядра (в начале строки — смещение в бинарнике). Тексты, которые есть только в ядре, читаются отсюда.
- `R/home/hatch/docs/*.md` и всё, чего нет в рабочей папке после сужения, читалось через `git show HEAD:<путь>`; номера строк — номера строк файла в HEAD.

Пометки в таблицах:

- **как есть** — берём текст, файл или форму muse без изменений (кроме названных отличий: имя продукта, Meta);
- **прослойка** — вещь зависит от модели и идёт через `agent/model`;
- **без пары** — у muse это завязано на закрытые службы Meta, приложение, устройства или платежи. Без слова владельца в шаблон не идёт, или у нас этого нет.

---

## 1. Главные находки (они меняют исходные допущения плана)

1. **Модель muse — не Claude.** muse работает на моделях Meta: Muse Spark и Muse Wonder (`R/home/hatch/docs/muse.md:57-63`). В реестре моделей ядра по умолчанию стоит `model = "ipnext/avocado-5.17-v2"` (`A/strings-all.txt:235570-235575`). Claude там тоже есть, но как одна из нескольких моделей: `providers = { anthropic = "anthropic/claude-opus-4-6", … }` (`A/strings-all.txt:235575`).
2. **muse уже умеет работать с разными поставщиками.** В `hatch-inference/src/model/` лежат поставщики: `anthropic/*` (`A/source-files.txt:5263-5275`), `gpt_responses` (OpenAI Responses, `:5280-5285`), `kimi_chat_completions` (Chat Completions, `:5301-5304`), `ipnext` и `magi_native` (Meta, `:5287-5306`), `rl_completion_server` (`:5314`). Ещё там фабрика (`model/factory.rs`, `:5279`), реестр (`model_registry.rs`, `:5318`) и выбор модели (`model_resolver/*`, `:5320-5330`). Реестр моделей — встроенный TOML (`A/strings-all.txt:235532-237160`) с разделами `[defaults]`, `[weights.*]`, `[wires]` и `[[models]]`. В нём перечислены поставщики `anthropic`, `azure`, `fireworks`, `kimi_chat_completions`, `gemma_chat_completions`, `ipnext`, `ipnext_responses`, `openai`, `llama`, `hybrid`. Вывод для П2: прослойка модели — не «исключение без образца». У muse есть своя форма: реестр весов с флагами возможностей и выбор модели по назначению. Её и берём. Исключение остаётся одно: наш набор поставщиков (`anthropic` + `openai_compat`), а не их.
3. **Поиск и чтение страниц в сети у muse идут не через сервер Claude.** Это свои инструменты `browser.search`, `browser.open`, `browser.find` (`R/home/hatch/docs/browser.md:18-20`; исходники `A/source-files.txt:1504,1507,1510`). Их обслуживает свой крейт `web-search` (`A/source-files.txt:7359-7362`). Строк `web_search_20…`, `web_fetch_20…` и `server_tool_use` в ядре нет (0 вхождений в `A/strings-all.txt`). Значит, у нас «свой инструмент той же формы» — это прямо форма muse, а не замена серверной вещи Claude.
4. **Главный системный текст разговора не лежит в `prompts/`.** 88 блоков `blocks/user_chat/*.md` есть только внутри ядра (`A/strings-all.txt:59570-59700` и далее; список имён — `grep -o 'blocks/user_chat/[a-z_]*\.md'`). В `P/hatch-agent/prompts/blocks/` такой папки нет. П5 берёт эти блоки из `strings-all.txt`.
5. **«149 навыков» в `R/opt/hatch/skills/` — в основном ссылки.** 109 ссылаются на `/opt/hatch/connectors/*`, 31 — на `/opt/muse/skills/*`, и только 9 — настоящие папки (`artifacts audio booking connections home-assistant instagram proactivity spaces voice-selector`). Сами тексты навыков лежат в `R/opt/hatch/workflow-skills/` (58 навыков `SKILL.md` + `skill.yaml`, 437 файлов; это та же папка, что `/opt/muse/skills`) и в `R/opt/hatch/connectors/` (208 описаний).
6. **Коннекторы опираются на закрытые программы.** Например, `connectors/gmail/connector.yaml:15` вызывает `/opt/hatch/bin/hatch_gws_auth`, а `connectors/github/CONNECTOR.md:3` — `/opt/hatch/bin/github` (`rootfs-full.listing.txt:208135`). Описания можно взять как есть. Программ из `opt/hatch/bin/` (248 файлов) у нас нет: каждую пишем сами или помечаем «без пары».

---

## 2. Что есть у muse

### 2.1. Цикл хода и сборка подсказки

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Исполнение хода | `A/source-files.txt:1303-1363` (`session/impl_session/message_execution/*`) | ход: сборка → модель → пачка вызовов инструментов → итог; там же `model_execution.rs` (`:1327`), `tool_dispatch_batch.rs` (`:1357-1361`), `tool_batch_settlement.rs` (`:1356`) | как есть (форма), код свой |
| Повтор после сбоя модели | `A/source-files.txt:1321-1323` (`inference_recovery/*`) | повтор вызова модели, ожидание | как есть |
| Проверка завершения | `A/source-files.txt:1311-1313` (`completion_gate*`), `A/source-files.txt:1530-1533` (`completion_checks*`, `completion_contract.rs`) | не даёт закончить ход без обещанного результата | как есть |
| Сборка подсказки | `A/source-files.txt:1365-1394` (`prompt_assembly/*`) | системный текст, контекст хода, описания пространств инструментов (`tool_namespace_descriptions.rs`, `:1392`), закрепление начала подсказки (`prompt_prefix_pin*`, `:1384-1387`), оценка длины (`context_estimate.rs`, `:1374`) | как есть (форма) |
| Хранение истории | `A/source-files.txt:1228-1239` (`history_persistence/*`); SQL `agent.*` (`A/sql.txt:2458,2514`) | запись шагов хода и событий в базу | как есть (форма) |
| Сроки ожидания | `A/env-knobs.txt:68` (`JARVIS_MODEL_STREAM_FIRST_CHUNK_TIMEOUT_MS`, `JARVIS_TOOL_DISPATCH_TIMEOUT_MS` и др.) | пределы на первый кусок потока, на инструмент и т.п. | как есть (только имена, значений в разборе нет) |
| Большой вывод инструмента — в файл | `A/source-files.txt:1805` (`tool_output_spill.rs`) | слишком длинный результат уходит в файл, модели — указатель | как есть |
| Системный текст главного разговора | `A/strings-all.txt:59570-59700…` (`blocks/user_chat/*`, 88 блоков); `P/hatch-agent/prompts/blocks/chat/*.md` (18) | кто ты, как работаешь, память, инструменты, безопасность, расписания | как есть (тексты; см. находку 4) |
| Разметка «личный / групповой» | `<!-- personal-muse-only:start -->` — 3050 вхождений, `group-muse-only` — 2950 (`A/strings-all.txt`) | один текст на два режима | как есть: оставляем только personal |
| Справочник `~/docs/` | `R/home/hatch/docs/*.md` (21 файл, 2300 строк); список — `P/hatch-agent/prompts/blocks/chat/how_you_work.md:15-35` | агент читает его, когда спрашивают про его возможности | как есть, с отличиями (не Meta, без приложения) — П5 |
| Файлы домашней папки | `A/strings-all.txt:59646-59655` (`AGENTS.md`, `SOUL.md`, `IDENTITY.md`, `USER.md`, `MEMORY.md`) | характер, личность, о владельце, долгая память | как есть (берём только разметку, содержимое пустое) |

### 2.2. Инструменты (имена — как видит модель)

Имена собраны по `A/strings-all.txt` (`grep -o '\b<пространство>\.[a-z_]+'`). Описания параметров — `A/tool-schemas.txt` (1047 описаний). Схемы инструментов браузерного исполнителя — `P/hatch-agent/prompts/browser/tools/*.json`.

| Инструмент muse | Где | Что делает | Пометка |
|---|---|---|---|
| `muse.read`, `muse.write`, `muse.edit` | `A/strings-all.txt:59657`; исходники `A/source-files.txt:1571-1579` (`file_io*`), `:1552-1553` (`edit.rs`, `edit_matching.rs`) | файлы в `~`, правка заменой | как есть |
| `muse.exec` + `process.list/poll/log/write/send_keys/kill/clear` | `A/strings-all.txt:58387,59667`; `A/source-files.txt:1764-1796` (`subprocesses/*`: `exec_tool.rs` `:1772`, `process_tool.rs` `:1788`, `executor/local*` `:1775-1783`, `executor/execd.rs` `:1774`) | команды оболочки, фоновые процессы | как есть (форма), код свой |
| `browser.search` | `A/source-files.txt:1510`; описание `A/strings-all.txt:76891` («Search the web using a text query…», параметр `primary_query.language_code`); правило выбора `A/strings-all.txt:58083` | поиск в сети | как есть (форма); бэкенд поиска свой (у muse — крейт `web-search`, `A/source-files.txt:7359-7362`) |
| `browser.open` | `A/source-files.txt:1507-1509`; `A/strings-all.txt:58084` | текст страницы по точному адресу или ссылке из выдачи | как есть (форма), код свой |
| `browser.find` | `A/source-files.txt:1504`; `R/home/hatch/docs/browser.md:18` | поиск по тексту полученной страницы | как есть |
| `browser.lookup_citation_url` | `A/source-files.txt:1506` | адрес источника из выдачи | как есть |
| `browser.browse`, `browser.windows` (+ `request_input`, `report`) | `R/home/hatch/docs/browser.md:3-16`; `A/tool-schemas.txt:2-23`; `A/source-files.txt:1475-1503` (`browser_delegate/*`) | живой браузер: задача передаётся браузерному исполнителю (отдельный агент) | прослойка (исполнитель — отдельная модель, назначение `BrowserAgent`, `A/strings-all.txt:237150`) |
| `muse.memory_search`, `muse.memory_get`, `muse.memory_explain` | `A/strings-all.txt:59746` (несколько формулировок в `queries`); `A/tool-schemas.txt:24-27`; `A/source-files.txt:1631-1635` | поиск и чтение памяти | как есть |
| забыть (навык `forget`) | `R/opt/hatch/workflow-skills/forget/SKILL.md`; `A/source-files.txt:1580` (`forget_tool.rs`) | удалить сохранённое | как есть |
| `muse.skill_search` | `A/source-files.txt:1735`; `R/opt/hatch/workflow-skills/self-awareness/SKILL.md` (раздел Available connectors) | найти навык | как есть |
| `muse.connector_search` | `A/source-files.txt:1538`; `P/hatch-agent/prompts/blocks/shared/environment/skills.md:2` | найти описание коннектора | как есть |
| `cron.add/list/view/update/remove/run/runs/status/stop` | `P/hatch-agent/prompts/blocks/shared/environment/scheduled_work.md:1`; `A/strings-all.txt:59588,59594`; `A/source-files.txt:2279` (`hatch-cron/src/tools/cron.rs`) | расписания | как есть |
| `hooks.list/add/update/remove/dry_run/enable/disable/run/logs/view` | `P/hatch-agent/prompts/blocks/shared/environment/hooks.md:1`; `A/strings-all.txt:59581`; `A/source-files.txt:5988-5989` (`event_hooks*`) | наблюдатель: дешёвый скрипт-проверка будит агента | как есть |
| `subagent.spawn/send/list/close/resume`, `subagents.status` | `A/strings-all.txt:29712`; `A/source-files.txt:1757-1762` (`subagents_tool/*`) | подагенты | как есть |
| `todo.write` | `A/strings-all.txt:58134`; `A/source-files.txt:1797-1798` | список дел внутри хода | как есть |
| `goals.*` (инструмент целей) | `A/source-files.txt:1594-1602` (`goals_tool/*`); `R/home/hatch/docs/goals.md:3-12` | цели и записи о ходе дел | как есть |
| `workflow.create/launch/list/view/…` | `A/source-files.txt:1827-1828` (`workflow_tool*`) | сохранённые многошаговые работы | как есть (форма) |
| `tool_search.load_tool_namespace` | `A/strings-all.txt:59572` (`blocks/user_chat/loading_tools.md`); `A/source-files.txt:1618` (`load_tool.rs`) | подгрузить отложенные инструменты пространства | как есть — работает с любой моделью |
| `muse.nothing_to_do`, `muse.nothing_to_say` | `A/source-files.txt:1641` | ход без ответа (для фоновых работ) | как есть |
| `muse.notify_main_agent` | `P/hatch-agent/prompts/tools/notify_main_agent_description.md` | подагент или фоновая работа сообщает главному агенту | как есть |
| `muse.db` | навык `R/opt/hatch/workflow-skills/muse_db/SKILL.md`; `R/home/hatch/docs/self_improvement.md:36-38` | SQL по своей базе (почему агент что-то решил) | как есть (форма), схема своя |
| `chat.*` (`list`, `read_messages`, `send_message`, `connection_status` …) | `A/strings-all.txt` (имена); `R/home/hatch/docs/chat-connections.md:11` | чаты и связи с мессенджерами | как есть (см. 2.9) |
| `widget.*`, `ui.*`, `avatar.*`, `media.*`, `shopping.*`, `wallet.*`, `phone.*`, `mail.*`, `feed.*`, `idea.*`, `artifact.*`, `spaces.*`, `devices.*`, `credentials.*` | `A/strings-all.txt` (имена); `A/source-files.txt:1465-1828` | карточки приложения, аватар, генерация медиа, покупки, кошелёк, звонки, почта muse, лента, идеи, сайты, устройства, хранилище паролей | без пары: держатся на приложении muse и службах Meta. Позже — по слову владельца |

### 2.3. Память

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Дерево памяти `~/memory/`, `~/MEMORY.md` | `A/strings-all.txt:59655,59667-59671`; `R/home/hatch/docs/self_improvement.md:7-11` | `MEMORY.md` — отобранная долгая память; `~/memory/` — датированные заметки; `bank/` и `index/` ведёт движок | как есть |
| Люди и группы | `A/strings-all.txt:59669-59671`; `R/home/hatch/docs/self_improvement.md:12-14` | страница на каждого человека, `INDEX.md` | как есть |
| Хранилище памяти | `A/source-files.txt:5463-5476` (`hatch-memory/*`: ingest, retrieval, maintenance, reindex); `A/source-files.txt:4804-4807` (`hatch-db-memory`: claims, entries, notes) | записи, утверждения, поиск | как есть (форма), код свой |
| Таблицы | `A/sql.txt:4451-4461` (`memory.entries`, `memory.claims`, `memory.embeddings`, `memory.entry_attributes`, `memory.metadata`) | схема памяти | как есть (форма) |
| Векторы для поиска | `memory.embeddings`, `embedding_models` (`A/sql.txt:4453`); `A/source-files.txt:5472` (`retrieval/models/inference.rs`) | поиск по смыслу | прослойка (модель эмбеддингов, см. раздел 3) |
| Сброс памяти перед сжатием | `A/source-files.txt:1296-1302` (`memory_flush/*`, `pre_compaction_*`); `P/hatch-agent/prompts/blocks/memory_flush/*.md`; подсказка `P/hatch-agent/prompts/session/memory_flush_prompt.md` | перед сжатием важное записывается в память | как есть |
| Фоновая уборка памяти (раз в час) | `P/hatch-self-improvement/prompts/memory/system/core.md`, `steps/{extract,judge,reconcile}.md`; `R/home/hatch/docs/self_improvement.md:7-11` | сводит новое в `MEMORY.md` | как есть |
| Справочник о памяти | `R/home/hatch/docs/memory.md:1-42` | правила для агента | как есть, с отличиями (без импорта из приложения) |

### 2.4. Цели

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Цели и записи о ходе | `R/home/hatch/docs/goals.md:3-12`; `A/source-files.txt:5205-5227` (`hatch-goals/*`); таблицы `goals.goals`, `goals.updates`, `goals.briefings` (`A/sql.txt:269,515,4319`) | цель, подцели, датированные записи | как есть |
| Папка цели | `A/strings-all.txt:59674-59677` (`~/workspace/goals/<slug>/GOAL.md`, `files/`, `hidden_files/`) | рабочая папка цели | как есть |
| Изучение целей ночью | `P/hatch-goals/prompts/studying/*.md`; `R/home/hatch/docs/self_improvement.md:18-20` | разбор материалов по целям | как есть |
| Расписание, принадлежащее цели | `A/strings-all.txt:59588` («goal-owned cron»); `P/hatch-cron/prompts/scheduler/goal_*.md` | расписание внутри цели | как есть |

### 2.5. Расписания и наблюдатели

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Планировщик | `A/source-files.txt:2220-2297` (`hatch-cron`, `hatch-cron-core`); `A/source-files.txt:4880…` (`hatch-db-scheduler`); таблицы `scheduler.jobs`, `scheduler.job_runs`, `scheduler.cron_mutations`, `scheduler.delivery_outbox` (`A/sql.txt:4431,310,4591,4594`) | задания по времени; запуск исполняет фоновый агент | как есть (форма), код свой |
| Тексты исполнителя расписания | `P/hatch-cron/prompts/scheduler/*.md` (`worker_instruction.md`, `worker_phase_fence.md` …) | что делает фоновый агент на запуске | как есть |
| Правила честности | `R/home/hatch/docs/scheduling-and-watching.md:7-31` | «опрос, а не слежение», итог — главному агенту | как есть |
| Наблюдатели (hooks) | `P/hatch-agent/prompts/blocks/shared/environment/hooks.md:1`; `A/source-files.txt:5988-5989`; `runtime.event_hook_admissions` (`A/sql.txt:908`) | скрипт-проверка + агент | как есть |
| Службы systemd | `A/strings-all.txt:59589` | постоянно работающая программа | как есть (на своём сервере П8) |
| `HEARTBEAT.md` | `P/hatch-agent/prompts/blocks/shared/environment/hooks.md:1` (упомянут) | проверка с заданным ритмом | как есть |

### 2.6. Навыки и коннекторы

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Системные навыки | `R/opt/hatch/workflow-skills/*/SKILL.md` + `skill.yaml` (58 навыков, 437 файлов); пример `wide-research/skill.yaml:1-7` (`name`, `title`, `description`, `category`, `include_in_prompt`, `availability`) | инструкция, которую агент читает и выполняет | как есть, с отличиями |
| Ссылки `opt/hatch/skills` | `R/opt/hatch/skills/*` (140 ссылок + 9 папок) | общий список навыков и коннекторов | как есть (повторить ссылками или списком) |
| Свои навыки агента | `A/strings-all.txt:59686-59689` (`/opt/muse/skills/` только чтение; `~/workspace/custom-skills/`, `~/workspace/custom-connectors/`); шаблон `R/opt/hatch/workflow-skills/custom-connector-setup/templates/skill/SKILL.md:1-15` | агент сам пишет навык по образцу; системный навык правит «поправкой», а не копией | как есть |
| Коннекторы | `R/opt/hatch/connectors/*/` (208: `CONNECTOR.md`, `connector.yaml`, `manifest.yaml`); пример `gmail/connector.yaml:1-20` | описание службы: вход, методы, синхронизация | как есть — тексты; программы (`/opt/hatch/bin/*`) без пары |
| Навыки в подсказке | `P/hatch-agent/prompts/blocks/shared/environment/skills.md:1-2`; `include_in_prompt` в `skill.yaml` | какие навыки видны сразу, какие ищутся | как есть |
| Сброс устаревших навыков | `runtime.skill_invalidation_receipts` (`A/sql.txt:3645`) | агент перечитывает навык после правки | как есть (форма) |

### 2.7. Улучшение себя (self_improvement, skill_improvement)

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Рамка фоновых циклов | `A/source-files.txt:5714-5770` (`hatch-self-improvement/src/framework*`), `:5771-…` (`objectives/*`) | шаги, оценки, публикация изменений | как есть (форма), код свой |
| Улучшение навыков (раз в день) | `P/hatch-self-improvement/prompts/skill_improvement/system/core.md:1-12`, `steps/{audit,judge}.md`; `R/home/hatch/docs/self_improvement.md:25-29` | ищет, что повторяется и ломается; пишет навык в `custom-skills`, ненужное снимает | как есть |
| Память, люди, «сны», изучение, идеи | `P/hatch-self-improvement/prompts/{memory,relationships,studying,ideas,…}`; `R/home/hatch/docs/self_improvement.md:7-31` | фоновые циклы по расписанию | как есть: память, люди, сны, изучение; идеи и покупки — без пары (вкладки приложения) |
| Пределы циклов | `A/env-knobs.txt:45` (`JARVIS_SELF_IMPROVEMENT_IDLE_*`, `PER_DAY_CEILING`) | когда и сколько раз запускать | как есть (имена; значений в разборе нет) |
| Модель для фоновых шагов | `setup.default_model_purposes` (`A/strings-all.txt:237127-237137`: `SelfImprovementStep`, `CronWorker`, `BackgroundWorker` …) | фон может идти на другой модели | прослойка |

### 2.8. Подагенты и глубокие исследования

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Подагенты | `A/source-files.txt:1757-1762`; `A/source-files.txt:1352-1353` (`subagent_completion_stream.rs`, `subagent_execution.rs`); `A/source-files.txt:788-…` (`agent_manager/*`); `agent.subagent_spawns`, `agent.subagent_progress` (`A/sql.txt:717,3779`) | порождение, ход, итог возвращается родителю | как есть |
| Тексты подагента | `P/hatch-agent/prompts/blocks/agent/who_you_are.md:1-10`; `P/hatch-agent/prompts/session/subagent_*.md`; `P/hatch-agent/prompts/blocks/subagents/{subagent,detached}/*.md` | кто такой подагент, наследование истории | как есть |
| Наблюдатель за подагентами | `P/hatch-agent/prompts/activity_monitor/*`, `P/hatch-activity/*` | ведёт ленту дел подагентов для приложения | без пары (это для интерфейса приложения) |
| Глубокое исследование | `A/source-files.txt:1715-1721` (`research_delegate/*`, `research_notes.rs`); `P/hatch-agent/prompts/blocks/deep_research/*.md` (9) | отдельный агент-исследователь | как есть |
| Широкое исследование | `R/opt/hatch/workflow-skills/wide-research/SKILL.md:1-12` | один управляющий подагент раздаёт одинаковые задачи | как есть |

### 2.9. Сжатие разговора

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Своё сжатие | `A/source-files.txt:2145-2150` (`hatch-compaction`: `quality.rs`, `segmented.rs`, `transcript/*`); `A/source-files.txt:1221-1223` (`eager_compaction*`), `:1310` (`compaction_model.rs`) | сводка старой части разговора | как есть (форма), код свой |
| Тексты сжатия | `P/hatch-compaction/prompts/compact/{system,summarization,summary_rules,segmented_*}.md`; `P/hatch-agent/prompts/compact/*.md` | что сохранять в сводке | как есть |
| Порог | `A/env-knobs.txt:12` (`JARVIS_AVOCADO_COMPACTION_TRIGGER_TOKENS`, `JARVIS_AVOCADO_CONTEXT_WINDOW_TOKENS`); окно «avocado» = 350 000 (`A/strings-all.txt:235542-235543`) | когда сжимать | прослойка (окно у каждой модели своё) |
| Модель сжатия | `azure/avocado-compaction-v1` (`A/strings-all.txt:237073`); `setup.compaction_memory_flush_use_main = true` (`A/strings-all.txt:236111`) | сжатие идёт на отдельной модели или на главной | прослойка |
| Таблицы | `agent.agent_compactions`, `agent.compactions` (`A/sql.txt:2458,2514`) | история сжатий | как есть (форма) |

### 2.10. Мессенджер

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Связь с мессенджером как «побочный чат» | `R/home/hatch/docs/chat-connections.md:1-20` | мессенджер — только канал для чата, второй истории нет | как есть (форма) |
| WhatsApp | `A/source-files.txt:2020-2128` (`hatch-chat-transport-whatsapp/*`); `R/home/hatch/docs/chat-connections.md:15` | единственный канал muse | без пары до слова владельца (П6: какой мессенджер) |
| Пометка источника хода | `P/hatch-agent/prompts/blocks/chat/how_you_work.md:11` (`[whatsapp]`) | агент знает, откуда пришло сообщение | как есть |
| Доставка событий | `A/source-files.txt:3144-…` (`hatch-daemon-messaging`) | события агента для клиентов | как есть (форма) |

### 2.11. Браузер

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Окна, вкладки, профили | `R/home/hatch/docs/browser.md:3-16`; `A/tool-schemas.txt:2-23` | одна задача = одно окно; общий профиль входа | как есть |
| Браузерный исполнитель | `A/source-files.txt:914-…` (`hatch-agent/src/browser/*`, 81 файл), `:1475-1503`; тексты `P/hatch-agent/prompts/browser/*` (163 файла: `tools/*.json` — схемы, `instructions/*`, `workflows/*`) | отдельный агент ведёт браузер | прослойка (своя модель; картинки страницы) |
| Сам браузер | `opt/meta-chromium/` (`rootfs-full.listing.txt:223453`), `opt/hatch/bin/browser-broker` (`:208080`), `hatch-chromium-sandbox` (`:208144`) | Chromium Meta | без пары: ставим свой Chromium (П8) |
| Расширение Chrome | `R/home/hatch/docs/browser.md:22-40` | браузер владельца | без пары |

### 2.12. Песочница и свой компьютер

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Свой компьютер | `R/home/hatch/docs/muse.md:3-5`; `P/hatch-agent/prompts/blocks/chat/how_you_work.md:3` | у агента своя машина: терминал, браузер, файлы | как есть (П8) |
| Разметка папок | `A/strings-all.txt:59657-59690` | `~` живёт между перезапусками, `~/workspace`, `/tmp`, `~/.ssh` | как есть |
| Исполнитель команд | `A/source-files.txt:1774-1784` (`executor/execd.rs`, `local/*`, `prepared_exec_cgroups.rs` `:1779`, `redaction.rs` `:1780`); `opt/hatch/bin/hatch-execd` (`rootfs-full.listing.txt:208147`) | запуск в cgroups, скрытие секретов в выводе | как есть (форма), код свой |
| Клетка исполнения | `opt/hatch/runtime-cell/*` (`rootfs-full.listing.txt:209264-209270…`) | скрипты изоляции | как есть (форма) — П8 |
| Выход в сеть | `R/opt/hatch-private/configs/hatch-sentinel/crates/hatch-sentinel/policies/network_egress_*.json` | правила выхода в сеть | прослойка: проверка этих правил у muse идёт через модель-классификатор |
| Сторож личности (BPF) | `A/strings-all.txt` (около смещения `223b5cb`, `hatch-identity-guard/src/guard.rs`) | защищает файлы личности от правки | без пары (ядро Linux, решение — П8) |

### 2.13. Безопасность на отдельных моделях

| Вещь muse | Где | Что делает | Пометка |
|---|---|---|---|
| Проверка внедрённых команд | `R/opt/hatch-private/configs/hatch-engine/crates/hatch-agent/src/prompt_injection_checker/*` | отдельная модель проверяет вывод инструментов | прослойка (назначение модели); включать или нет — слово ревизора |
| Проверка вызовов (кибер, CBRNE, утечка данных) | `A/source-files.txt:1689-1712` (`registry/*_guard*`); политики `R/opt/hatch-private/configs/hatch-engine/crates/hatch-agent/src/safety_classifiers/policies/*.json` | проверка вызова до исполнения | прослойка |
| Модели проверок | `[weights."safety-classifier"]` (`A/strings-all.txt:235771`); `gemma-4-e4b-hatch-monitor` (`A/strings-all.txt:236757`); `JARVIS_SAFETY_MODEL` (`A/env-knobs.txt:12`) | отдельные малые модели | прослойка |
| Чёрный список адресов | `R/opt/hatch-private/configs/blocklist/*` | запрет адресов | как есть |

---

## 3. Привязки к модели

Здесь перечислены все места, где muse опирается на свойства модели: Claude и другие поставщики из его реестра. Флаги возможностей у muse записаны у весов в реестре (`[weights.*]`, `A/strings-all.txt:235576-236000`). Например, для `cedar` — `lanes = "claude-opus-5-5"` (`A/strings-all.txt:236107`) — заданы флаги `anthropic.adaptive_thinking`, `.compaction`, `.tool_search`, `.output_effort`, `.image_resolution`, `.runtime_output_ceiling`, `.mid_conversation_system_messages` (`A/strings-all.txt:235795-235806`). Для весов Meta — `magi.reasoning`, `magi.developer_role`, `magi.commentary_in_input`, `magi.guided_decoding`, `magi.tool_output_error_status`, `magi.image_only_messages`, `magi.png_or_jpeg_images_only` (`A/strings-all.txt:235581-235588`). Прослойка П2 повторяет эту форму: возможность либо записана в настройке модели и включена, либо её нет.

| Свойство | Как у muse | `anthropic` | `openai_compat` (OpenAI) | `openai_compat` (локальная: Ollama, LM Studio, llama.cpp, vLLM) |
|---|---|---|---|---|
| Форма вызова инструментов | для каждого поставщика свой перевод: `anthropic/convert_tool_spec.rs` (`A/source-files.txt:5266`), `gpt_responses/prompt/tool_schema.rs` (`:5284`), Chat Completions с полями `tools`, `tool_choice`, `parallel_tool_calls` (`A/strings-all.txt:237328`), у Meta — свой формат подсказки `magi.prompt_format` (`A/strings-all.txt:235581`) | `tools` + `tool_use`/`tool_result` | `tools` + `tool_calls` + роль `tool` | то же, что OpenAI, если сервер и шаблон модели поддерживают инструменты. Если нет — модель не годится для агента; прослойка говорит об этом при запуске и инструменты не подделывает |
| Параллельные вызовы | пачка `tool_dispatch_batch` (`A/source-files.txt:1357`); `parallel_tool_calls` (`A/strings-all.txt:237328`) | несколько `tool_use` в одном ответе | `parallel_tool_calls` | зависит от сервера; если нет — по одному вызову |
| Ошибка инструмента | `magi.tool_output_error_status` (`A/strings-all.txt:235586`) | `is_error` | флага нет, ошибка пишется текстом результата | так же |
| Мышление | `anthropic.adaptive_thinking`, `omit_thinking_display` (`A/strings-all.txt:235799-235800`); бета `thinking-binding-controls-2026-08-01` (`A/strings-all.txt:237329`); `redacted_thinking` (`A/strings-all.txt:68150`); `magi.reasoning = "up_to_high"` (`A/strings-all.txt:235582`); зашифрованное мышление `MAGI_ENCRYPTED_REASONING` (`A/strings-all.txt:235506`), «encrypted history requires the native IPNext provider» (`A/strings-all.txt:235521`) | `thinking: adaptive`; блоки мышления возвращаются в историю как есть | `reasoning_effort` у моделей, которые его знают; иначе выключено | только у думающих моделей (`reasoning_content` или `<think>` в тексте, смотря по серверу); иначе выключено |
| Сила ответа | `anthropic.output_effort` (`A/strings-all.txt:235804`), `setup.output_effort.by_purpose` (`A/strings-all.txt:236109`); `JARVIS_IPNEXT_REASONING_EFFORT` (`A/env-knobs.txt:12`) | `effort` | `reasoning_effort` | нет — выключено |
| Кэш подсказки | `cache_control` (`A/strings-all.txt:235512,237328`); «scheduler prompt cache fallback» (`A/strings-all.txt:237328`); закрепление начала подсказки `prompt_prefix_pin*` (`A/source-files.txt:1384-1387`), `volatile_context_pin.rs` (`:1394`); учёт `cached_tokens`/`prompt_tokens_details` (`A/strings-all.txt:235508`) | `cache_control` на неизменном начале | метки кэша нет, кэш начала подсказки работает сам; счёт — `cached_tokens` | у llama.cpp и vLLM кэш начала подсказки работает сам, у Ollama — по серверу; меток нет. Закрепление начала (форма muse) полезно всем |
| Сжатие разговора | своё сжатие (`A/source-files.txt:2145-2150`) + сжатие у поставщика: `anthropic.compaction = true` (`A/strings-all.txt:235802`), бета `compact-2026-09-04` (`A/strings-all.txt:237329`), «this checkpoint does not support provider compaction» (`A/strings-all.txt:237328`) | своё; сжатие у поставщика — только если флаг включён | своё | своё (на той же или на отдельной модели, см. «модель по назначению») |
| Поиск отложенных инструментов | у поставщика: `anthropic.tool_search` (`A/strings-all.txt:235803`), `defer_loading`, «built-in tool search cannot be deferred» (`A/strings-all.txt:235512`), тексты `P/hatch-agent/prompts/blocks/anthropic/cedar/*.md`; на своей стороне: `tool_search.load_tool_namespace` (`A/strings-all.txt:59572`, `A/source-files.txt:1618`) | свой поиск; поиск поставщика — по флагу | свой (`load_tool_namespace`) | свой |
| Встроенные инструменты сервера (поиск, выборка страниц) | **у muse нет**: свои `browser.search`/`browser.open`/`browser.find` (раздел 2.2) | не используются | не используются | не используются — одинаково для всех |
| Длина окна | `context_window` у весов: 1 000 000 у `cedar` (`A/strings-all.txt:235797`), 200 000 у `claude-opus-5` (`A/strings-all.txt:235819-235821`), «avocado» = 350 000 (`A/strings-all.txt:235542-235543`, замена — `JARVIS_AVOCADO_CONTEXT_WINDOW_TOKENS`, `A/env-knobs.txt:12`); бета `context-1m-2025-08-07` (`A/strings-all.txt:235856`); оценка длины `context_estimate.rs` (`A/source-files.txt:1374`), `context_window.rs` (`:1316`), счёт токенов `o200k_count.rs` (`:5307`) | из настройки модели | из настройки модели | из настройки модели (у локальной — как запущен сервер, `num_ctx` / `--ctx-size`); счёт токенов приблизительный, как `o200k_count` у muse |
| Предел ответа | `anthropic.runtime_output_ceiling = 64000` (`A/strings-all.txt:235806`); проверка размера запроса `enforce_request_size_budget.rs` (`A/source-files.txt:5267`) | `max_tokens` | `max_tokens` / `max_completion_tokens` | `max_tokens` |
| Системный текст посреди разговора | `anthropic.mid_conversation_system_messages` (`A/strings-all.txt:235801`); `magi.developer_role` (`A/strings-all.txt:235583`) | по флагу | роль `developer`/`system` | зависит от шаблона модели; как muse поступает без флага — в разборе не найдено |
| Картинки на входе | `prepare_inline_image_bytes.rs` (`A/source-files.txt:5270`); `anthropic.image_resolution` (`A/strings-all.txt:235805`); `magi.image_only_messages`, `png_or_jpeg_images_only` (`A/strings-all.txt:235587-235588`; «Cerebras /generate rejects every image type except PNG and JPEG», `A/strings-all.txt:235758`); описание картинки отдельной моделью `P/content-understanding/prompts/image_describe.md`, назначение `MediaDescription` (`A/strings-all.txt:236113`) | блок `image` (base64) | `image_url` (data: base64) | только у моделей со зрением; иначе картинки выключены или описываются отдельной моделью (форма muse, `MediaDescription`) |
| Файлы на входе (PDF и др.) | `file_io/document_convert.rs` (`A/source-files.txt:1573`) — перевод документа в текст на своей стороне | текст | текст | текст |
| Ответ в заданной форме | `output_contract.rs` (`A/source-files.txt:5332`): «Complete the task by calling `…` exactly once … must validate against this JSON Schema» (`A/strings-all.txt:235518`) — обязательный вызов инструмента; `guided_decoding.rs` (`:5256`), `magi.guided_decoding` | через вызов инструмента | через вызов инструмента | через вызов инструмента (если инструменты есть) |
| Видимый промежуточный текст | `model/commentary.rs` (`A/source-files.txt:5277`), `magi.commentary_in_input` (`A/strings-all.txt:235584`); `blocks/anthropic/text_visibility.md`, `turn_private_tool_call_text_instructions.md` (`A/strings-all.txt:78526,78540`) | текст рядом с `tool_use` | `content` рядом с `tool_calls` | так же, если сервер отдаёт оба |
| Поток (стрим) | `anthropic/parse_sse_stream.rs`, `sse_parser_state_machine.rs` (`A/source-files.txt:5269,5273`); `gpt_responses/stream.rs` (`:5285`); Chat Completions — «stream ended before [DONE]» (`A/strings-all.txt:237328`) | SSE Messages | SSE Chat Completions | SSE Chat Completions |
| Счёт токенов | `prompt_tokens`, `cached_tokens`, `completion_tokens` (`A/strings-all.txt:235508`); `agent.agent_message_token_usage` (`A/tool-schemas.txt:164`) | `usage` | `usage` | `usage`, если сервер отдаёт; иначе ноль и пометка |
| Модель по назначению | `[defaults]` `auto`, `model`, `maintenance` (сжатие, сброс памяти) (`A/strings-all.txt:235569-235575`); `setup.default_model_purposes` (`A/strings-all.txt:237127-237137`); `setup.overrides` по назначению (`A/strings-all.txt:236112-236114,237149-237151`) | любая роль | любая роль | любая роль; у слабой локальной модели фон и сжатие можно отдать другой модели |
| Модель эмбеддингов | `memory.embeddings`, `embedding_models` (`A/sql.txt:4453`); `hatch-memory/src/retrieval/models/inference.rs` (`A/source-files.txt:5472`) | у Anthropic эмбеддингов нет — нужен второй поставщик | `/v1/embeddings` | `/v1/embeddings` (Ollama, llama.cpp, vLLM); что делает muse без эмбеддингов — в разборе не найдено |
| Локальная модель в реестре | `local_inference = true` у проводов (`A/strings-all.txt:235901`) | — | — | у muse такой флаг уже есть: берём его форму |
| Сроки ожидания по поставщику | `JARVIS_ANTHROPIC_TIMEOUT_MS` (`A/env-knobs.txt:64`); `slow_first_token = true` (`A/strings-all.txt:237069`) | из настройки | из настройки | из настройки (у локальной первый токен медленный) |
| Голос (TTS, распознавание речи) | `hatch-inference/src/voice/*` (`A/source-files.txt:5345-5369`) | — | — | без пары в шаблоне |

---

## 4. Движок по частям (задание для П2–П4, Rust)

Код движка muse в снимке отсутствует. Ниже по каждой части: что о ней известно из разбора и текстов, и что из этого следует писать.

### 4.1. Прослойка модели и провод — П2

- **Разбор:** `A/source-files.txt:5253-5344`: `model.rs`, `model/factory.rs`, `model/collect_completion.rs`, поставщики `anthropic/*` (13 файлов: `convert_prompt`, `convert_tool_spec`, `request_flow`, `response_handling`, `parse_sse_stream`, `tool_search`, `validate`, `enforce_request_size_budget`, `prepare_inline_image_bytes`, `anthropic_http_error`), `gpt_responses/*`, `kimi_chat_completions/*`, `model_registry.rs`, `model_registry_file/load.rs`, `model_resolver/*`, `provider_error.rs`, `output_contract.rs`, `structured_json.rs`, `o200k_count.rs`. Реестр — `A/strings-all.txt:235532-237160`. Версия API Anthropic — `anthropic-version 2023-06-01` (`A/strings-all.txt:237329`).
- **Задание:** один вход (сообщения, инструменты, настройки) → один выход (текст, вызовы, мышление, счёт). Два поставщика: `anthropic` (по форме `model/anthropic/*`) и `openai_compat` (по форме `kimi_chat_completions/*`: Chat Completions, `tools`, `tool_choice`, `parallel_tool_calls`, SSE до `[DONE]`). `config/model.toml` — по форме `[weights]`/`[[models]]` muse: `provider`, `model`, `base_url`, `context_window`, флаги возможностей (`thinking`, `effort`, `prompt_cache`, `provider_compaction`, `provider_tool_search`, `vision`, `image_formats`, `mid_conversation_system`, `tool_error_flag`, `output_ceiling`) и модели по назначению (`main`, `maintenance` для сжатия и сброса памяти, `background`, `browser`, `media_description`, `embeddings`). Ошибки поставщика — единым видом (`provider_error.rs`). Ответ в заданной форме — через обязательный вызов инструмента (`output_contract.rs`).
- **Тесты:** записанные ответы `anthropic` и `openai_compat` (с инструментами, с мышлением и без, с ошибкой, с обрывом потока).

### 4.2. Цикл хода — П3

- **Разбор:** `A/source-files.txt:1303-1363` (`message_execution/*`), `:1365-1394` (`prompt_assembly/*`), `:1228-1239` (`history_persistence/*`), `:1221-1223` (`eager_compaction*`), `:1296-1302` (`memory_flush/*`), `:1805` (`tool_output_spill.rs`); сроки — `A/env-knobs.txt:68`; таблицы `agent.*` (`A/sql.txt:2458,2514,3575,3779`).
- **Задание:** ход «модель → пачка вызовов → результаты → модель» до ответа без вызовов. Пачка исполняется параллельно (`tool_dispatch_batch`). Повтор при сбое модели (`inference_recovery`). Проверка завершения (`completion_gate`). Большой вывод уходит в файл (`tool_output_spill`). Сборка подсказки: неизменное начало (системный текст, файлы `~`, описания инструментов) + изменчивый хвост (время, контекст хода) — по форме `prompt_prefix_pin`/`volatile_context_pin`.
- **Сжатие:** по форме `hatch-compaction` и текстам `P/hatch-compaction/prompts/compact/*.md`. Порог — доля окна из настройки модели. Перед сжатием — сброс памяти (`memory_flush`, текст `P/hatch-agent/prompts/session/memory_flush_prompt.md`). Сводка пишется моделью назначения `maintenance`.
- **Подагенты:** по форме `subagents_tool/*` и `subagent_execution.rs`: `subagent.spawn/send/list/close/resume`, итог приходит родителю; тексты `P/hatch-agent/prompts/session/subagent_*.md`, `blocks/agent/*.md`; таблица `agent.subagent_spawns` (`A/sql.txt:717`).

### 4.3. Инструменты — П4

- **Разбор:** реестр и раздача `A/source-files.txt:1688,1695` (`registry.rs`, `registry/dispatch.rs`). Описания пространств — `prompt_assembly/tool_namespace_descriptions.rs` (`:1392`), тексты — `A/strings-all.txt:58081-58087` и далее. Параметры — `A/tool-schemas.txt`. Схемы браузерного исполнителя — `P/hatch-agent/prompts/browser/tools/*.json`. Отложенная загрузка — `tool_search.load_tool_namespace` (`A/strings-all.txt:59572`).
- **Задание, по порядку:**
  1. файлы — `muse.read/write/edit` (`file_io*`, `edit_matching.rs`);
  2. исполнение — `muse.exec` + `process.*` (`subprocesses/*`, скрытие секретов `redaction.rs`);
  3. сеть — `browser.search` (бэкенд поиска выбирается настройкой; у muse свой крейт `web-search`, его поставщик в разборе не назван), `browser.open` (текст страницы), `browser.find`, `browser.lookup_citation_url`;
  4. память — `muse.memory_search/get/explain`, забывание (`hatch-memory/*`, таблицы `memory.*`);
  5. навыки — `muse.skill_search`, `muse.connector_search` (поиск по `skill.yaml`/`connector.yaml`, своим и системным);
  6. расписания — `cron.*`, наблюдатели — `hooks.*` (таблицы `scheduler.*`, `runtime.event_hook_*`);
  7. цели — `goals.*`;
  8. дела — `todo.write`;
  9. подагенты — `subagent.*`;
  10. живой браузер — `browser.browse/windows` (исполнитель — подагент с моделью назначения `browser`; свой Chromium, П8).
- Один инструмент = одна запись в реестре: имя, описание, схема. Описание общее для всех поставщиков (CLAUDE.md, «Устройство»).

### 4.4. Память — П4

- **Разбор:** `A/source-files.txt:5463-5476`, `:4804-4807`; `A/sql.txt:4451-4461`; файлы `~/MEMORY.md`, `~/memory/` (`A/strings-all.txt:59655,59667-59671`); фоновая уборка — `P/hatch-self-improvement/prompts/memory/system/*`.
- **Задание:** записи и утверждения (новое заменяет старое, след хранится в датированных заметках), поиск по нескольким формулировкам. Векторный поиск — только если в настройке есть модель эмбеддингов. Иначе поиск по тексту; как поступает muse без эмбеддингов, не найдено — ревизору решить.

### 4.5. Расписания — П4

- **Разбор:** `A/source-files.txt:2220-2297` (`hatch-cron*`), `:4880-…` (`hatch-db-scheduler`); `A/sql.txt:310,4431,4591,4594,4623`; тексты `P/hatch-cron/prompts/scheduler/*.md`; правила — `R/home/hatch/docs/scheduling-and-watching.md:7-31`.
- **Задание:** задания по времени и разовые. Каждый запуск — фоновый агент с текстом `worker_instruction.md`; итог отдаётся главному агенту, а он решает, говорить ли владельцу. Наблюдатели: скрипт-проверка по ритму, при срабатывании будит агента.

### 4.6. Навыки и их улучшение — П4/П5/П9

- **Разбор:** `R/opt/hatch/workflow-skills/*`, `R/opt/hatch/connectors/*`; разметка `A/strings-all.txt:59686-59689`; шаблон `custom-connector-setup/templates/skill/SKILL.md`; улучшение — `P/hatch-self-improvement/prompts/skill_improvement/system/*`, рамка `A/source-files.txt:5714-5770`.
- **Задание:** навык — папка с `SKILL.md` + `skill.yaml`. Системные навыки только читаются, свои лежат в `workspace/custom-skills/`, правка системного навыка — «поправкой». Улучшение навыков — фоновый цикл раз в день (шаги `audit` → `judge`).

### 4.7. Чего в П2–П4 нет

Мессенджер — П6 (по слову владельца). Свой сервер, Chromium, клетка исполнения, выход в сеть — П8. Карточки приложения, медиа, покупки, кошелёк, звонки, почта muse, лента, идеи, сайты, устройства — без пары.

---

## 5. Что не найдено или не проверено

- Кода движка нет. Поведение частей восстановлено по именам исходников, строкам, SQL и текстам; частные правила (пороги, значения сроков) в разборе не видны — есть только имена настроек.
- Поставщик поиска за крейтом `web-search` (`A/source-files.txt:7359-7362`) в строках не назван. Нашлись только `space_web_search/cagi_worker.rs` (`A/source-files.txt:2916`) и `jina_fallback_planner` (`A/tool-schemas.txt:24`); что они значат, не установлено.
- Модель эмбеддингов памяти по имени не найдена. Не найдено и то, что muse делает без эмбеддингов.
- Как muse передаёт системный текст посреди разговора модели без флага `mid_conversation_system_messages`, не найдено.
- Тексты `blocks/user_chat/*` (88) есть только в `strings-all.txt`. Их полноту (не обрезаны ли строки) проверит П5.
- Номера строк `R/home/hatch/docs/*.md` сверены через `git show HEAD:` (после сужения рабочей папки этих файлов на диске нет).
