# Опись muse для личного агента (П1, 27.11.2026)

Основание — карточка `docs/cards/P1-inventory.md`. Источник — только `iseethepoint/muse-live` (HEAD `8ab06ae59a`). `muse-clone-rs` не читался. Данные пользователя muse (`rootfs-full/home/hatch`) не открывались: из описи снимка взяты только имена путей.

## Как читать ссылки

| Метка | Файл в muse-live | Что значит номер |
|---|---|---|
| **S:n** | `core-v2-analysis/strings-all.txt` | строка файла (`grep -n`). Строки там — склейки из бинарника, поэтому нужное слово стоит *внутри* строки n |
| **SF:n** | `core-v2-analysis/source-files.txt` | строка с именем исходника ядра |
| **TS:n** | `core-v2-analysis/tool-schemas.txt` | строка. Первые ~2500 строк — JSON-схема базы данных, дальше описания параметров инструментов |
| **EK:n**, **FF:n**, **SQL:n**, **U:n**, **T:n** | `env-knobs.txt`, `feature-flags.txt`, `sql.txt`, `urls.txt`, `templates.txt` в `core-v2-analysis/` | строка |
| **P/…** | `rootfs-full/opt/hatch-private/prompts/hatch-engine/crates/…` | путь к тексту движка, `:n` — строка |
| **RC/…** | `rootfs-full/opt/hatch/runtime-cell/…` | скрипты запуска клетки |
| **L:n** | `rootfs-full.listing.txt` | строка описи архива |

Оговорка о клоне: в `~/muse-live` включён sparse-checkout (с 26.11 21:28 — не мной). На диске лежат `core-v2-analysis`, `docs`, `opt/hatch-private`, `opt/hatch/{connectors,prompts,skills,workflow-skills}`. Остальное (`etc`, `opt/hatch/runtime-cell`) читалось через `git show HEAD:<путь>`, номера строк сверены по HEAD.

## Главное — что меняет план

1. **muse не завязан на одну модель.** В ядре своя прослойка `hatch-inference` с пятью протоколами: `anthropic` (Messages API), `gpt_responses` (OpenAI Responses), `kimi_chat_completions` (chat/completions), `ipnext` и `magi_native` (внутренние протоколы Meta) — SF:5263–5316. Модели описаны вшитым в ядро реестром `config/model_registry.toml` (S:235532–237190). **По умолчанию у muse не Claude, а своя модель Meta** `ipnext/avocado-5.17-v2` (S:235569–235575); Claude (`cedar` = `claude-opus-5-5`, `aspen` = `claude-fable-5-2`) — одна из опций (S:235795–235806). Значит, «исключение» из CLAUDE.md («у muse модель одна») не подтверждается: прослойку модели тоже можно брать **по форме muse** (реестр + свой преобразователь на протокол + флаги возможностей у модели). Предложение ревизору — в «Открытых вопросах», п. 1.
2. **Серверных инструментов Claude у muse нет.** `web_search_20…`, `web_fetch_20…`, `code_execution_20…` в строках ядра встречаются 0 раз. Поиск и чтение страниц — свои инструменты `browser.search`, `browser.open`, `browser.find` поверх своих сервисов (stefi, perplexity, firecrawl, page_cache — S:245415). Из серверных возможностей Claude muse использует только три, и все включаются флагом модели: `tool_search` (bm25), серверное сжатие (`compact-2026-09-04`) и кэш подсказки (S:237329, S:245406).
3. **Навыков не 149, а 58.** Настоящие навыки лежат в `opt/hatch/workflow-skills/` (58 папок). `opt/muse/skills/` — те же файлы жёсткими ссылками (L:237673). Из 149 записей в `opt/hatch/skills/` 109 — ссылки на коннекторы, 31 — ссылки-псевдонимы старых имён, 9 — настоящие папки-ассеты.
4. **Ядро не говорит с поставщиком напрямую:** все вызовы идут через локальный прокси `/run/hatch/proxy/inference.sock` (S:237328, EK:52). У нас прокси нет — прослойка зовёт `base_url` сама.

## 1. Вещи muse — таблица

Решение:
- **как есть** — файл muse берём в репозиторий (тексты, навыки, коннекторы, схемы) с названными отличиями;
- **по форме** — у muse это код ядра, исходников нет, пишем своё на Rust по устройству из разбора;
- **прослойка** — зависит от модели, идёт через `agent/model`;
- **без пары** — у агента этого нет (продукт Meta, чужая платформа).

### Движок

| Вещь muse | Где | Что делает | Решение |
|---|---|---|---|
| Определения агентов (роли) | S:69124–74375; корневой `foreground_root` S:69986 (`provider = "model_registry"`, `tool_loading = "deferred"`) | TOML на каждую роль: модель, усилие, список `tools`, `[[gated_tools]]`, `prompt_gates`. Около 100 ролей: generic_subagent_depth1/2 S:70575/S:70743, verifier S:69565, browser_task S:69233, scheduler_cron_execute S:73005 | по форме (файл ролей в папке настроек агента — это и есть «шаблон») |
| Цикл хода | SF:1303 `message_execution.rs`, SF:1327 `model_execution.rs`, SF:1328 `new_user_turn.rs`, SF:1357–1361 `tool_dispatch_batch*` | модель → пачка вызовов (параллельно) → сводка результатов → модель | по форме |
| Шлюз конца хода | SF:1311–1313 `completion_gate*`; P/hatch-agent/prompts/completion_gate/*.md | возвращает модель на шаг, если не сделан обязательный терминальный вызов | по форме |
| Терминальные «пустые» вызовы | S:67835 (`nothing_to_do, skip_reply, nothing_to_report, finish_step, worker.finish`) | закончить ход без ответа / вернуть итог | по форме |
| Пределы хода | EK:68 (таймауты `JARVIS_*_TIMEOUT_MS`); S:48651 `WorkflowNoProgressContext` | счётчика шагов нет — таймауты и детектор «нет прогресса» | по форме (своих пределов не добавляем) |
| Конфиг упряжи | S:235531 (`HarnessConfig with 18 elements`, `CompactionConfig with 5`) | флаги поведения цикла: `output_budget_tokens, parallel_tool_call_guidance, eager_tool_schemas, provider_tool_search, memory_flush_history_first…` | по форме → часть `config/` |
| Очередь ввода | SF:1260–1269 `lifecycle/submission_*`; таблица `message_mailbox` TS:302 | принимает сообщения во время идущего хода | по форме |
| Сборка подсказки | SF:1144–1148 `prompt_composition/*`, `gated_markdown.rs`; SF:1365–1394 `prompt_assembly/*` | склейка блоков роли + `shared/`, условные куски `<!-- personal-muse-only -->`, подстановки `{name}`/`{{NAME}}`, закрепление префикса ради кэша | по форме |
| Блоки системного текста | P/hatch-agent/prompts/blocks/ (368 файлов: chat 18, side_chat 48, agent 38, shared 42, subagents 16, memory_flush 3…) | текст каждой роли | как есть |
| Переменные текста | S:78542 (`goals_section, project_context, depth, max_spawn_depth, can_spawn, os, shell…`); T:2480 (`{hatch_environment}{memory_directory}`) | что движок подставляет | по форме |
| Хранение разговора | TS:272 `context_items`; TS:1907 `messages`; TS:360 `sessions`; SF:4438–4480 `hatch-db-conversations/src/agent_state/*`; SQL:521–523 | поток элементов контекста на агента (сообщение, вызов, результат) | по форме |
| Восстановление после рестарта | TS:336 `runtime_restart_checkpoints`; SF:1170–1180 | доигрывает прерванный ход | по форме |
| Сжатие | SF:1050–1058 `compact*`, SF:2145–2150 `hatch-compaction`, SF:1310 `compaction_model.rs`, SF:1316 `context_window.rs`; TS:156 `agent_compactions` | сводка-чекпойнт по порогу, хвост последних токенов остаётся, есть сегментный режим и заблаговременное фоновое сжатие | по форме + прослойка (см. §2) |
| Тексты сжатия | P/hatch-compaction/prompts/compact/ (7: system, summarization, summary_rules, segmented_summarization, segmented_state, segmented_state_over_cap, chat_worker_conversation); P/hatch-agent/prompts/compact/ (4) | что и как писать в сводку | как есть |
| Сброс памяти перед сжатием | SF:1296–1302 `memory_flush/*`; P/hatch-agent/prompts/session/memory_flush_prompt.md; S:248630 (`sync/manual/eager_pre_compaction…`) | перед сжатием дописать факты в `~/memory/YYYY-MM-DD.md` | по форме, текст как есть |
| Подагенты | SF:1757–1762 `subagents_tool*`, SF:837–842 `agent_manager/control/subagent_*`; TS:191 `agents`, TS:143 `agent_ancestors`, TS:438 `subagent_spawns` | `subagent.spawn/send/close/list/resume`, глубина до 2, наследование истории inherited/isolated | по форме |
| Тексты подагентов | P/hatch-agent/prompts/session/subagent_*.md; blocks/subagents/ | первое сообщение подагента, режимы истории | как есть |
| Монитор подагентов | SF:5074–5077; SF:724–736 `hatch-activity`; P/hatch-activity/prompts/activity_monitor/subagents_monitoring.md | отдельный вызов модели на поддерево, по умолчанию молчит; итоги детей доставляются без него | по форме |
| Воркфлоу | `workflow.*` S:332320–332345; SF:1827 `workflow_tool.rs` | JS-сценарии, оркеструющие дочерних агентов | по форме (позже П4) |
| Фоновые работники | SF:904–913 `background_worker*`; P/hatch-agent/prompts/background_worker_paged_input.md | страничный ввод `worker.read_input`, терминал `worker.finish` | по форме |
| Шаги objective | SF:1643–1649 `objective_worker/*`; P/hatch-agent/prompts/inline/tools/objective_worker/step_execution.json | шаг конвейера, заканчивается `muse.finish_step` с JSON-схемой | по форме |

### Инструменты модели (П4)

Имена у модели — `пространство.функция`. Таблица внутреннего и видимого имени — S:332177, описания пространств — S:332137–332175. Отложенные пространства подгружаются `tool_search.load_tool_namespace` (S:59572).

| Инструменты | Где | Что делают | Решение |
|---|---|---|---|
| `muse.exec`, `process.list/poll/log/write/send_keys/kill/clear` | S:74955–74965; SF:1772, SF:1788 | команда в терминале (pty, фон, `yield_ms`) и управление фоновыми | по форме |
| `muse.read`, `muse.write`, `muse.edit` | S:68066–68071, S:78711–78714; SF:1571, SF:1552 | файлы; `read` отдаёт картинки моделью и документы markdown | по форме (картинки — прослойка) |
| `trash.put/list/restore` | S:332145; SF:1807 | корзина на 30 дней | по форме |
| `muse.db` | S:75815–75817; SF:1540 | один SELECT по своей базе | по форме |
| `muse.session_status` | S:67911; SF:1725 | заполненность контекста, модель, время | по форме |
| `muse.nothing_to_do / skip_reply / notify_main_agent / nothing_to_report` | P/hatch-agent/prompts/tools/notify_main_agent_description.md:1; SF:1641 | конец хода без ответа / передать итог | по форме |
| `todo.write` | S:332148; SF:1797 | чек-лист до 25 пунктов | по форме |
| `muse.memory_search / memory_get / memory_explain`, `forget.plan/confirm` | S:74526–74531, S:332206–332226; SF:1631–1635, SF:1580 | память (см. ниже) | по форме |
| `browser.search` | S:76896, параметры S:75897; SF:1510 | поиск в сети (вертикали sports/weather/finance/local) | по форме, свой поставщик поиска — вопрос 3 |
| `browser.open`, `browser.find`, `browser.lookup_citation_url` | S:67900, S:77058; SF:1504–1507 | текст страницы по URL без живого браузера, поиск на странице | по форме |
| `browser.deep_research` | S:77042; SF:1715 | изолированный исследователь, возвращает источники | по форме |
| `browser.browse`, `browser.windows`, `browser.profiles` | TS:2–23; SF:1475, SF:1495–1499 | задача живому браузеру через отдельного планировщика | по форме (П4, после П8 — нужен Chromium на сервере) |
| `muse.skill_search`, `muse.connector_search` | S:67909, S:78704; SF:1735, SF:1538 | поиск по навыкам и коннекторам, тело читается через `muse.read` | по форме |
| `credentials.list/request_login/…` | S:332171; SF:1539 | защищённое хранилище логинов | по форме |
| `subagent.*`, `workflow.*` | S:332149, S:332141 | подагенты, воркфлоу | по форме |
| `user_goal.*`, `tracking.*` | S:332144, S:332146, S:58190–58230; SF:1594–1601, SF:6289 | цели и отслеживаемые дела | по форме |
| `cron.add/update/remove/list/view/status/run/runs/stop` | S:332170, S:85120–85185; SF:2277–2279 | расписания | по форме |
| `hooks.add/…/dry_run/logs` | S:332167, S:84902; SF:2245–2247 | автоматизации на событие | по форме |
| `chat.list/create/read_messages/send_message/…` | S:332172, S:74515–74521; TS:25–28; SF:1514 | главный и боковые чаты | по форме (провод — П6) |
| `artifact.*`, `widget.*`, `ui.*` | S:332175, S:332142, S:332137; SF:1468, SF:1672, SF:1811 | артефакты и карточки в приложении muse | без пары, пока нет своего приложения; `artifact.create_file` — вопрос 5 |
| `media.generate_image/video/icon` | S:332165; SF:1581 | генерация медиа | без пары (нет модели генерации) |
| `device.*` | S:332169; SF:1541 | команды на телефоне и Mac пользователя | без пары |

Продуктовые инструменты Meta **без пары**: `wallet.*`, `permissions.preapprove…`, `shopping.*`, `ucp.*`, `checkout.*`, `travel.*`, `traveler_details.*`, `phone.*`, `voice.*`, `avatar.*`, `feed.*`, `idea.*`, `ads.*`, `onboarding.*`, `proactivity.*` (в части UI), `company_conversation.*`, `connection.*` (агент↔агент), `mail_*`, `map.*`, `places.*`, `local.search`, `social.search`.

Реестр инструментов: SF:1688 `tools/registry.rs`, SF:1695 `registry/dispatch.rs`; классы риска S:59123; сторожа на каждом вызове SF:1689–1713. Длинный вывод пишется в файл с подсказкой читать через `read` (S:332187–332198; SF:1805). Недоверенный вывод оборачивается в `[BEGIN EXTERNAL CONTENT…]` (S:332176–332183). Схемы — JSON Schema, пример P/hatch-agent/prompts/browser/tools/tabs.json:1–37.

### Память, цели, расписания, навыки

| Вещь | Где | Что делает | Решение |
|---|---|---|---|
| Стоящие файлы `~/MEMORY.md`, `~/USER.md`, `~/SOUL.md`, `~/IDENTITY.md`, `~/AGENTS.md` | S:59646; бюджеты S:76380 | подкладываются в подсказку, у каждого свой бюджет | по форме; содержимое — своё |
| Дневные журналы `~/memory/YYYY-MM-DD.md` | P/hatch-self-improvement/prompts/memory/system/pipeline.md:4–7 | только дописываются | по форме |
| `~/memory/people/`, `groups/` с `INDEX.md`; `bank/`, `index/` ведёт среда | S:17363, S:59667, S:78441 | люди, группы, производный индекс | по форме |
| Таблицы памяти `memory.entries/claims/embeddings` | TS:1513–1576; SQL:4451–4467 | индекс по файлам, утверждения с источником, векторы | по форме (векторы — прослойка: нужна модель векторов, вопрос 4) |
| Обязательное чтение памяти | S:59792, S:59830, S:78437 | на каждое сообщение — `MEMORY.md` и `memory_search` | текст как есть |
| Сверка памяти (objective memory) | P/hatch-self-improvement/prompts/memory/ (12); pipeline.md:9–55; S:248948 («hourly») | extract → verify → apply → reconcile `MEMORY.md` точечными правками; код пишет, модель предлагает | по форме, тексты как есть |
| Цели `goals.*` | TS:945–1075; SF:1594–1602, SF:1439 `prompt_goals.rs`; P/hatch-goals/prompts/ (4); workflow-skills/goals/SKILL.md:17–27 | цели с коучингом, записями, таймлайном, папкой `workspace/goals/<slug>/GOAL.md` | по форме, тексты как есть |
| Расписания: файлы `~/workspace/cron.d/<kind>/<id>__<schedule>.md` | S:63546, S:85156; L (имена `cron.d/…`) | файлы — источник истины, таблица `scheduler.jobs` — проекция (TS:2220–2380) | по форме |
| Прогон задания | SF:2220–2276; P/hatch-cron/prompts/scheduler/worker_instruction.md:1–20, cron_resolution_contract.md:1–20 | две фазы: исполнение работником → один вызов `nothing_to_report`/`notify_main_agent`; отвечает пользователю основной агент | по форме, 16 текстов как есть |
| Сердцебиение `HEARTBEAT.md` | S:84815, S:84903 | без инструкций в файле модель не зовётся | по форме |
| Навык: `skill.yaml` + `SKILL.md` | P/hatch-self-improvement/prompts/skill_improvement/system/steps/audit.md:35–66 | `name, title, description, category, include_in_prompt, availability, requires, amends, skill_improvement`; в SKILL.md фронтматтера нет | как есть (58 из `workflow-skills/`) |
| Навыки в подсказке | S:78410–78432; S:43920–43923; S:67908 | блок «# Skills» для `include_in_prompt`, остальное — через `muse.skill_search` (bm25/regex), тело — `muse.read` | по форме |
| Свои навыки и коннекторы владельца | `~/workspace/custom-skills/`, `~/workspace/custom-connectors/` (S:67908) | добавляются папкой, без правки движка | по форме (это и есть П9) |
| Коннекторы | `opt/hatch/connectors/` (208): `connector.yaml` + `manifest.yaml` + `CONNECTOR.md` — пример connectors/deepl/{connector.yaml:1–10, manifest.yaml:1–60, CONNECTOR.md:1–38}; каталог TS:3794–4075; SF:5792 | не инструменты модели: найти через `connector_search`, прочитать CONNECTOR.md, звать CLI через `muse.exec` | описания как есть; CLI из `opt/hatch/bin` — чужие бинарники, без пары → свои по одному (вопрос 6) |
| Улучшение навыков | P/hatch-self-improvement/prompts/skill_improvement/ (6); pipeline.md:5–11; audit.md:9 | по журналам прогонов правит только свои навыки с `managed: true`; к системным — amendment; подсказки и политики не трогает | по форме, тексты как есть |
| Запуск самоулучшения | EK:45 `JARVIS_SELF_IMPROVEMENT_IDLE_*`; S:37030–37034; TS:2530–2547 `self_improvement.runs` | по простою разговора | по форме |
| Прочие конвейеры самоулучшения | P/hatch-self-improvement/prompts/: `_shared` 10, `lenses` 11, `alignment` 5, `relationships` 9, `goals_bookkeeping` 13, `studying` 6, `inline` 12 | общие линзы проверки, ночная рефлексия, страницы людей, ведение целей | как есть |
| `ideas` 40, `shopping` 5, `travel` 5, hatch-feed (87), hatch-context-workers (60) | P/hatch-self-improvement/prompts/ideas/; P/hatch-feed/; P/hatch-context-workers/ | витрины приложения muse | без пары |
| Проактивность (приватная копия, `[PROACTIVE HANDOFF]`) | P/hatch-agent/prompts/tools/proactivity/handoff.md:1; SF:1123–1137; workflow-skills/proactive-work/SKILL.md | работа без запроса, основной агент решает, говорить ли | по форме, позже П4 |

### Рабочее место

| Вещь | Где | Что делает | Решение |
|---|---|---|---|
| Клетка `systemd-nspawn` `htch-runtime` | RC/launch-daemon.sh:83–97; RC/ensure-rootfs.sh:44–51 | ядро и команды живут в контейнере с user namespace | по форме (П8) |
| Ядро — один бинарник `hatch daemon` | RC/control-daemon.sh:112; RC/run-daemon.sh:30–34 | один процесс-демон | по форме |
| Исполнитель команд `hatch-execd` | `etc/systemd/system/hatch-execd.{socket,service}`; RC/control-execd.sh:21–29 | команды агента идут через отдельный сокет | по форме (П8) |
| Окружение и область файлов | RC/guest.env:14–22; P/hatch-agent/prompts/blocks/event_hook/filesystem.md:2 | `HOME=/home/hatch`, рабочая область `~/workspace` (`your_files/`, `goals/<slug>/`) | по форме |
| База — PostgreSQL 18 на хосте, сокет в клетку | RC/ensure-rootfs.sh:1370–1374; EK:73–75 | все таблицы выше | по форме (выбор базы — вопрос 2) |
| Исходящий трафик через прокси, DNS мёртв | RC/guest.env:25–42 | всё наружу через `hatch-egress-proxy` с подменой TLS | без пары (шлюз Sentinel — платформа Meta) |
| Провод приложения | SF:3144–3184 (WebSocket `chat.send`, `stream`) | приложение muse ↔ ход | без пары (своего приложения нет) |
| Telegram | SF:1993–2018; S:28848 (long-poll `getUpdates`); U:430–431 | бот владельца, опрос Bot API самим демоном | по форме (П6, слово владельца) |
| WhatsApp, Messenger, Texting, голос | SF:2020–2128, SF:1991–2019, SF:3033–3143 | каналы Meta | без пары |
| Живой браузер — отдельная клетка и Chromium | RC/launch-daemon.sh:37–49; P/hatch-agent/prompts/browser/system.md:2–30 | планировщик-агент со своими инструментами `browser.observe/act/run/tabs/login/report`, специалисты login/checkout/bot-challenge, отдельная модель «grounding» (P/hatch-agent/prompts/browser/visual_grounding.md:1) | по форме (П4/П8), тексты браузера (163) как есть, кроме оплаты и покупок |
| Файл-сервер, Spaces, артефакты | SF:5172…; hatch-web-artifact-tools; SF:6189–6195 (Cloudflare) | доступ приложения к файлам, мини-приложения | без пары |
| Сторожа безопасности | configs/: mango-safety*, hatch-safety, hatch-sentinel, blocklist; SF:1150… `prompt_injection_checker`; SF:1689–1713 | модерация платформы, проверка инъекций в выводе инструментов | без пары, кроме обёртки недоверенного вывода `[BEGIN EXTERNAL CONTENT]` (S:332176) — это текст, берём как есть |

## 2. Привязки к модели

Как у muse: у каждой модели в реестре (`[weights.*]`) есть набор флагов возможностей, и на каждый протокол свой преобразователь (S:235532–235568). Мы делаем так же: `config/model.toml` содержит поставщика, адрес и **флаги возможностей**; чего у поставщика нет, то выключено и видно в настройке.

| Привязка | Как у muse (где) | `anthropic` | `openai_compat` (OpenAI) | локальная (Ollama, LM Studio, llama.cpp, vLLM) |
|---|---|---|---|---|
| **Форма описания инструментов** | свой преобразователь на протокол: SF:5266 `anthropic/convert_tool_spec.rs`, SF:5284 `gpt_responses/prompt/tool_schema.rs`, каталог Kimi S:245406. Описание одно, на выходе разное | `tools[]` с `input_schema` | `tools[].function.parameters` | то же, что openai_compat; держит ли модель вызовы — проверка П7 |
| **Имена с точкой** (`muse.exec`) | S:332177 — внутреннее имя ↔ видимое | точка не разрешена в имени инструмента → преобразователь меняет имя и ведёт таблицу обратно (вывод: muse держит оба имени ради этого же) | так же | так же |
| **Параллельные вызовы** | `parallel_tool_calls` S:237328; для протоколов без них — текст «emit each call as its own message» S:237386 | есть | `parallel_tool_calls` | часто нет → тот же текст muse, вызовы по одному |
| **Мышление и усилие** | `anthropic.adaptive_thinking`, `output_effort = supported / xhigh_as_max / unsupported` (S:235795–235806); шкала `minimal…max` S:235531; повторы при обрыве на мышлении FF:3 `gk.hatch_retry_on_thinking_eot`, текст досыла S:54777 | `thinking: adaptive` + `effort` | `reasoning.effort` у моделей с рассуждением, иначе выкл | обычно выкл; у моделей с `<think>` — только отрезать из ответа |
| **Кэш подсказки** | `cache_control`, `ttl`, `prefix_mismatch_behavior` S:237329; закреплённый префикс SF:1396, SF:5311 | `cache_control` на префиксе | автоматический у OpenAI, явной разметки нет | нет (у llama.cpp/vLLM — свой кэш префикса на сервере, нам ничего делать не надо) |
| **Сжатие разговора** | два пути: серверное Anthropic (бета `compact-2026-09-04`, флаг `anthropic.compaction`, S:237329) и своё по текстам P/hatch-compaction (SF:2145–2150); проверки `trigger_tokens` ≤ окна (S:245406) | можно оба; по умолчанию — своё, серверное — флаг | только своё | только своё |
| **Окно контекста и порог сжатия** | `context_window` в реестре; `compaction_trigger_tokens`, `retained_tail_tokens` (S:67911, S:77386); ручка `JARVIS_AVOCADO_CONTEXT_WINDOW_TOKENS` EK:12 | из настройки | из настройки | из настройки — обязательно (окно задаёт сервер: `num_ctx` у Ollama) |
| **Подсчёт токенов** | свой o200k_base для всех моделей, приблизительно (SF:5307, S:237417) | o200k по форме muse; точный счёт — из `usage` ответа | o200k | o200k (приблизительно, как у muse) |
| **Потолок вывода и обрыв** | `runtime_output_ceiling` (S:235806); «output token cap reached» + повтор FF:3 | `max_tokens` | `max_tokens`/`max_completion_tokens` | `max_tokens` |
| **Поиск инструментов** (отложенная загрузка) | серверный `tool_search_tool_bm25_20251119` только у cedar/aspen (S:245406); иначе свой `tool_search.load_tool_namespace` (S:59572) и маркеры `<!-- provider-tool-search-only -->` (P/hatch-agent/prompts/blocks/anthropic/cedar/tools_runtime_pointer.md:1–3) | свой путь; серверный — флаг | свой | свой |
| **Поиск в сети и чтение страниц** | свои `browser.search/open/find` (S:76896, S:67900); серверных инструментов Claude нет (0 совпадений) | свой инструмент | свой | свой |
| **Картинки на входе** | `prepare_inline_image_bytes.rs` SF:5270, предел запроса 32 MiB и ужатие (S:35783–35784); типы jpeg/png/gif/webp (S:237329, S:237359), у Cerebras только PNG и JPEG (S:235758); `image_resolution` в реестре | base64 `image` | `image_url` data-URL | только у моделей со зрением (флаг `vision`), иначе `muse.read` картинки не отдаёт — пишет, что модель их не видит |
| **Файлы (PDF и пр.)** | `read` переводит документ в markdown сам (S:68071) | текстом | текстом | текстом |
| **Описание картинок отдельной моделью** | `content_understanding` SF:5253, P/content-understanding/prompts/image_describe.md | — | — | замена зрению для локальной без него: флаг «модель описания» в настройке (вывод) |
| **Структурный вывод** | `output_contract.rs` (SF:5332): один обязательный вызов инструмента с `result` по JSON Schema — работает с любым протоколом | так же | так же | так же |
| **Роль system посреди разговора** | `anthropic.mid_conversation_system_messages` (S:235795…) | флаг | `developer`/`system` | `system` |
| **Модели по ролям** | полосы `lanes = { main, browser, cron, voice }`, назначения `CompactionMemoryFlush, CronWorker, SubagentMonitor…` (S:56281, S:236533, S:237126–237150) | в `config/model.toml` — модель на роль, по умолчанию одна на всё | так же | так же |
| **Протокол передачи** | SSE (SF:5269), chat/completions поток до `[DONE]` (S:237328) | SSE Messages | SSE chat/completions | то же |
| **Ограничение времени** | `JARVIS_ANTHROPIC_TIMEOUT_MS` EK:64, первые байты потока EK:68 | по форме | по форме | по форме |

Responses API OpenAI (`gpt_responses`) у muse есть, но CLAUDE.md даёт нам `openai_compat` как chat/completions — локальные серверы говорят именно так. Отдельный поставщик Responses — вопрос 1.

## 3. Движок по частям — задание для П2–П4

**П2 — `agent/model`.** По форме `hatch-inference`:
- реестр моделей — файл настройки: умолчания, наборы весов с окном и флагами возможностей, полосы по ролям (S:235532–237190);
- один вход и один выход на протокол, свой преобразователь сообщений и инструментов (SF:5263–5304);
- поток SSE;
- таблица имён «с точкой ↔ без точки»;
- картинки по флагу, `max_tokens` по потолку, повтор при обрыве на мышлении и на лимите вывода (FF:3);
- структурный вывод через обязательный вызов инструмента (SF:5332).

Тесты: подделка `anthropic` и подделка `openai_compat` на записанных ответах.

**П3 — `agent/loop`.**
- Очередь ввода (SF:1260–1269).
- Сборка подсказки из блоков роли с условными кусками и подстановками (SF:1144–1148, SF:1365–1394).
- Ход: модель → пачка вызовов параллельно → сводка → модель (SF:1357–1361).
- Шлюз конца хода и терминальные вызовы (SF:1311–1313, S:67835). Счётчика шагов нет — только таймауты (EK:68).
- Сжатие:
  - порог от окна модели, хвост остаётся (S:77386);
  - перед сжатием — сброс памяти (SF:1296–1302);
  - сводка в `agent_compactions` (TS:156);
  - тексты P/hatch-compaction как есть.
- Подагенты:
  - дерево `agents`/`agent_ancestors`/`subagent_spawns` (TS:143, 191, 438);
  - глубина до 2, на глубине > 1 история только isolated;
  - итог ребёнка доставляется родителю сам (P/hatch-agent/prompts/session/subagent_*.md).
- Хранение — `context_items` (TS:272).

**П4 — `agent/tools`.** Порядок по нужности владельцу:
1. Файлы (`read/write/edit`, `trash`).
2. Терминал (`exec`, `process.*`).
3. Память (`memory_search/get/explain`, стоящие файлы, дневные журналы).
4. Навыки и коннекторы (`skill_search`, `connector_search`, `custom-skills/`).
5. Поиск в сети и страницы (`browser.search/open/find`, `deep_research`).
6. Цели (`user_goal.*`, `tracking.*`).
7. Расписания (`cron.*`, `cron.d/` файлами, сердцебиение) и `hooks.*`.
8. Подагенты и `todo.write`.
9. Живой браузер (`browser.browse`) — после П8.

Реестр и отложенная загрузка пространств — по форме SF:1688, S:59572. Длинный вывод — в файл (SF:1805).

**П5 — тексты:**
- `blocks/` — без ролей Meta: avatar, feed, ideas, spaces_builder, voice;
- `session/`, `compact/`, `hatch-compaction`;
- `hatch-cron`, `hatch-goals`;
- `hatch-self-improvement` — без ideas, shopping, travel;
- `browser/` — без checkout, pay, poppy;
- навыки `workflow-skills/` (58) и описания коннекторов.

Отличия называются при переносе: условные куски `group-muse-only` вырезаются, `personal-muse-only` остаются.

## 4. Открытые вопросы — ревизору

1. **Исключение в CLAUDE.md.** У muse прослойка многомодельная (п. 1 «Главного»), значит П2 можно вести без слова «исключение», по форме `hatch-inference`. Нужна ли правка CLAUDE.md и плана П2: реестр моделей с флагами возможностей и полосами по ролям вместо одного `model.toml`? И нужен ли отдельный поставщик OpenAI Responses (у muse есть `gpt_responses`, SF:5280) или хватит chat/completions?
2. **База.** У muse PostgreSQL 18 (RC/ensure-rootfs.sh:1370). Берём Postgres на сервере владельца или нужна другая база — слово владельца?
3. **Поставщик поиска в сети.** У muse свои сервисы через прокси (stefi, perplexity, firecrawl — S:245415). У нас нужен внешний поиск: какой (платный ключ API = деньги) — слово владельца.
4. **Векторы памяти.** У muse таблица `memory.embeddings` (TS:1541–1549). Для поиска по памяти нужна модель векторов — какая (у локальной это ещё одна модель)?
5. **Артефакты.** `artifact.create_file` (документ файлом) может пригодиться и без приложения muse — брать или нет?
6. **CLI коннекторов.** 208 описаний берём как есть, но они зовут чужие бинарники `opt/hatch/bin` (не запускаем). Писать свои CLI — отдельной строкой плана, по одному на коннектор, который нужен владельцу?
7. **Справки `home/hatch/docs/`** (memory.md, goals.md, scheduling-and-watching.md, self_improvement.md) — по S:61211 это шаблоны движка, вшитые `include_str!("templates/workspace_docs/…")`, а не данные пользователя. Можно ли их читать и брать?
8. **Sparse-checkout клона** `~/muse-live` включён не мной — так задумано?
