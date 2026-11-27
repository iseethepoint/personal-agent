# Реестр упрощений и решений «по-своему» (черновик, 27.11.2026)

Источники: тексты коммитов инженера в `main` (разделы «Своё», «Не найдено», «Отличия», «Не сделано», «Не взято»), комментарии в `agent/*/src`, `prompts/SOURCES.md`, `docs/PLAN.md`, `docs/cards/*`. Проверки в ядре — `objdump -d` (только чтение) и поиск ссылок `lea [rip+…]` на строки `.rodata` (свой сканер, ядро не запускалось).

Виды: **а** — правила muse в разборе нет, решение вынужденное; **б** — у muse есть, у нас нет, отложено или упрощено (долг); **в** — отличие по слову владельца или по смыслу (модель, Hamlet, нет служб Meta).

## Вид (б) — долги

| № | Срез | Наш код | Что | Источник | Строка плана |
|---|---|---|---|---|---|
| б1 | П4е-2 | `agent/cron/src/hooks.rs:23-26` | Пределы наблюдателей «не взяты, чисел в разборе нет». **Числа найдены в ядре:** `poll_interval_secs` от 5 до 604 800 (функция около `0x10244065`: `poll−5 < 0x93a7c`, константы `0x49a7f0`=5, `0x49f958`=604800, текст «poll_interval_secs must be between» `0x3950a0`); `script_timeout_secs` от 1 до 600 (`0x10244078`, константа `0x497928`=600, текст `0x437955`); `prompt` не больше 65 536 байт (`cmp r15,0x10000` в `0x10243ff4`, текст «prompt exceeds the 65536-byte limit» `0x366ffe`) | коммит 3224e3f «Не взято»; проверено objdump | П4е-2 (доделка) |
| б2 | П4е-1 | `agent/cron/src/schedule.rs` (проверки `every`) | Нет нижнего предела частоты расписания. У muse: «Please choose a cron interval of at least {…}. The requested {…} interval is too frequent. Nothing was saved.» (`0x437836`, `strings-all.txt:41922-41924`, `:76801`). Предел приходит в регистре (функция около `0xfdfe7a0`, ветка «1 hour» при 3600); само число ещё не вынуто — нужен вызывающий | найдено при сверке; objdump | П4е-1 (доделка) |
| б3 | П4е-1 | `agent/model/src/http.rs:13`, работник `agent/cron/src/worker.rs` | Работник расписаний ждёт первый кусок потока 600 с, как главный разговор. У muse свой срок `JARVIS_CRON_MODEL_STREAM_FIRST_CHUNK_TIMEOUT_MS` = 1 800 000 мс (`0x26a6c020`) | `docs/muse_inventory.md:252`; в коде нет | П4е-1 (доделка) |
| б4 | П4е-1 | `agent/cron/src/service.rs:5-7, :51` | Ровный такт 15 с вместо наблюдателя файлов и пробуждения к сроку ближайшего прогона; сверка памяти — на такте, у muse — `notify::inotify` (`strings-all.txt:344927`, `:17841`, `:19814`) | коммит 36ab126 «Своё»; PLAN П4 «Долг: сверка … фоновая» | П4 (долг в строке), закрыт наполовину |
| б5 | П4е-1 | `agent/cron/src/schedule.rs:14` | Поля расписания `timing` и `window` (второй вариант описания muse) не сделаны | коммит 36ab126 «Не взято» | П4е-1 / новая |
| б6 | П4е-1, П4е-2 | `agent/cron/src/tools.rs:11`, `hooks.rs:450`; `prompts/SOURCES.md` (cron_namespace, hooks/tools.json) | `owner` заданий, пути целей `workspace/goals/…`, блок `event_hook/filesystem.md`, абзацы о целях в описании `cron` | коммиты 36ab126, 3224e3f | П4ж (выдан) |
| б7 | П4е-1 | `agent/cron/src/tools.rs:5-7` | Нет `tool_search.load_tool_namespace`; описание пространства `cron` прикреплено к `cron.add` (и hooks — к `hooks.add`) | PLAN П4 «Долг»; опись 2.2 | П4 (долг в строке) |
| б8 | П4е-2 | `agent/cron/src/hooks.rs:24-25` | Таблицы `execute_resolve_*` не взяты: итог наблюдателя сразу в ящик | коммит 3224e3f «Не взято» | П4е-2 / новая |
| б9 | П4е-2 | `agent/cron/src/hooks.rs:21-22` | Скрипт наблюдателя запускается своим `bash`, у muse — обычным исполнителем команд | коммит 3224e3f «Своё» | П4е-2 |
| б10 | П4а | `agent/tools/src/files.rs:120, :133` | Перевод документов (PDF, Word, Excel, …) в Markdown не сделан — ошибка «not available yet». У muse — крейт `anydoc-0.1.7` (`source-files.txt:51-70`); папка с номером версии, как у крейтов crates.io (рядом `lopdf`, `calamine`, `pdf-inspector` в `crates.txt`) — вероятно, его можно взять зависимостью (не проверено, сети нет) | коммит 2f3daa6 «Отличия» | новая |
| б11 | П4б | `agent/tools/src/exec.rs` | Не сделано: ошибка `process.poll`, когда итог уже ждёт доставки; добивание потомков, ушедших из группы процессов; пульс инструмента 60 с (`JARVIS_TOOL_DISPATCH_HEARTBEAT_SECS`) | коммит 2e5c6f4 «Не сделано» | П4б (доделка); пульс — П6 |
| б12 | П4б | — | Изоляция исполнения (cgroups, `runtime-cell`) | карточка П4, п. 4 | П8 |
| б13 | П3г | `agent/loop/src/subagents.rs:453-463` | `items` (картинки, файлы) в `subagent.spawn/send` не сделаны — только `message`; описание muse про `items` модель видит | коммит 6f7f275 «Отличия» | П3 / новая |
| б14 | П3г, П3д | `agent/loop/src/subagents.rs:38`; `prompts/subagent/subagent_history_inheritance_inherited.md` | Глубина подагентов только 1 (`MAX_SPAWN_DEPTH`); у muse глубже — изолированные; `descendants` = `direct`; блоки `depth_one_*` пусты; из текста убран пункт «deeper than 1» | коммиты 6f7f275, 4dc44f1; SOURCES.md | новая |
| б15 | П3д | `agent/loop/src/subagents.rs`; `agent/memory/src/forget.rs` | Нет набора инструментов на подагента (ролей): «только чтение» плана forget держится на задании. У muse — определения ролей (`strings-all.txt:69124-74375`) | коммит 4dc44f1 «Своё» | новая |
| б16 | П3г | — | Подагенты не хранятся между перезапусками | коммит 6f7f275 «Отличия» (карточка, п. 3) | новая (история в Postgres `agent.*`, слово 27.11 в шапке плана) |
| б17 | П3г | — | `subagents.status` — маршрут HTTP демона muse (`strings-all.txt:20518`), у нас провода нет | коммит 6f7f275 | П6 |
| б18 | П3в | `prompts/compaction/` | Тексты `segmented_*`, `chat_worker_conversation` из `compact/` не перенесены — сжатия по частям нет | коммит f6ef533 «Отличия» | П3 / новая |
| б19 | П3б, П3г, П4в | `agent/loop/src/prompt.rs:16`; `agent/tools/src/skills.rs:208` | Системный текст muse, текст заметки изменчивого, `blocks/agent/*`, `subagent_spawning.md`, блок списка навыков, сами навыки и коннекторы | коммиты b7ad25a, 6f7f275, eda76fa | П5 |
| б20 | П4д | `agent/memory/src/tools.rs:10` | В выдаче памяти нет «Past conversations»: разговоров в базе нет | коммит 2afa437 | новая (история в `agent.*`) |
| б21 | П4г | `agent/tools/src/web.rs` | Не взят `mase_all_urls_fingerprints.bin` (60 МБ, формат MUFP, хэш не восстановлен) — запрет по адресам, не только по доменам | коммит 0b76ed6 «Не найдено» | П4г / новая; можно копать дизассемблером (`url_blocklist_guard`) |
| б22 | П4г | — | Живой браузер `browser.browse/windows` (подагент на модели `BrowserAgent`) | карточка P4-tools.md (конец: «следующей карточкой») | новая (в плане строки нет) |
| б23 | П2б | `agent/model/src/anthropic/convert.rs:16-25` | Умения весов `anthropic.compaction` и `anthropic.tool_search` не поддержаны — ошибка настройки | коммит 47eb6ad | П2 / решить, нужно ли (сжатие своё — П3в; tool_search — см. б7) |

## Вид (а) — правила в разборе нет

| № | Срез | Наш код | Что решено своё | Источник | Достать глубже? |
|---|---|---|---|---|---|
| а1 | П3а | `agent/loop/src/turn.rs:29` | Одна повторная попытка вызова модели (`MODEL_RETRIES = 1`) | коммит ddea866 | Можно, долго: ссылки на строки повтора (`0x6b1554`, `retry_budget_exhausted` `0x6b15a5`) — из `0x929ac0c`, `0x92a6b09`…; это большая машина async, счётчик быстро не найден |
| а2 | П3а | `turn.rs:347` | Текст ошибки срока инструмента | ddea866 | Строки поиском — не проверял |
| а3 | П3а | `turn.rs:79` | Папка большого вывода — у вызывающего, имя `<call_id>.txt`; начало — целые строки; offset — следующая строка | ddea866 | — |
| а4 | П3а | `turn.rs:365` | «Неверное имя» вызова: пусто, >128 знаков, знаки вне `[A-Za-z0-9_.-]` | ddea866 | — |
| а5 | П3а | `turn.rs` (`submit_result`) | Указание без принудительного вызова — заметкой Developer; в историю — пустой результат | ddea866 | — |
| а6 | П3б | `agent/loop/src/prompt.rs:104, :128` | Оценка длины = счёт поставщика + o200k дописанного; картинки не считаются | b7ad25a | — |
| а7 | П3б | `prompt.rs` | Заметка изменчивого — сразу после сообщения владельца; отсутствующий файл — пустой | b7ad25a | — |
| а8 | П3в | `agent/loop/src/compaction.rs:110` | После сжатия остаётся последний ход владельца целиком; один ход — без сжатия | f6ef533 | — |
| а9 | П3в | `compaction.rs` | Сброс памяти — заметкой Developer на копии; сводка — сообщением User, один вызов без повтора; в запрос сводки — описания инструментов и `tool_choice none`; проверка порога — функция для вызывающего | f6ef533 | — |
| а10 | П3г | `agent/loop/src/subagents.rs:223-228` | Заметка «[Subagent Result] …», ответы JSON, свои ошибки; имена параметров по описаниям; правило interrupted/failed | 6f7f275 | — |
| а11 | П3г | `subagents.rs:218-219` | `{{SESSION_LINES}}`, `{{METADATA_BLOCK}}` пусты | 6f7f275 | — |
| а12 | П3д | `agent/memory/src/forget.rs:97-101` | Форма заметки «[Memory refresh] …»; описание forget.* — две строки ядра через пустую | 4dc44f1 | — |
| а13 | П4а | `agent/tools/src/files.rs:34-37` | Предел `read` 100 КБ (у muse — аргумент косвенного вызова функции `0x26e00e60`) | 2f3daa6 | Можно: найти вызывающих `0x26e00e60` |
| а14 | П4а | `files.rs:150` | «Read output capped» — без `limit`, «Showing lines» — с `limit` (седьмой аргумент не опознан) | 4824367 | То же |
| а15 | П4а | `files.rs:376` | Похожесть = 1 − Левенштейн по байтам / длина большего (у muse — внешняя функция `0x29aded88`) | 4824367 | Похоже, можно: в `crates.txt` есть `strsim-0.11.1` и `similar-2.7.0` — вероятно, похожесть из `strsim`, diff из `similar`; проверить по символам рядом с `0x29aded88` |
| а16 | П4а | `files.rs:341, :391` | Пределы снимка и diff 2000/4000 байт; diff по LCS построчно; поле «lines a-b» | 4824367 | См. а15 (`similar`) |
| а17 | П4а | `files.rs:249, :275` | Свои тексты ошибок и ответов («Wrote …», «Edited …»), форма списка папки; `write` создаёт родительские папки | 2f3daa6 | — |
| а18 | П4б | `agent/tools/src/exec.rs:62-64` | Предел буфера вывода сессии 100 КБ | 5f4f56e | — |
| а19 | П4б | `exec.rs:60, :262` | Ответы JSON с полями из описаний; id `proc_N`; хвост 2000 знаков | 2e5c6f4 | — |
| а20 | П4б | `exec.rs:336-339` | Заметка «[Background exec finished] …» | 2e5c6f4 | — |
| а21 | П4б | `exec.rs:55` | Знак `<redacted>` (строка `0x869117`, связь с `redaction.rs` не доказана) | 2e5c6f4 | — |
| а22 | П4б | `exec.rs:252` | Конец ввода в pty — EOT (Ctrl-D) | 5f4f56e | — |
| а23 | П4в | `agent/tools/src/skills.rs:260` | BM25 k1 = 1.2, b = 0.75; разбиение на слова | eda76fa | Почти снято: у muse крейт `bm25-2.3.2` (`crates.txt:396`) — можно взять его же |
| а24 | П4в | `skills.rs:14` и др. | Форма поправки своего навыка; ответ JSON; строка списка; поля `todo.write` (`todos/text/status`), ответ `{items,in_progress}` | eda76fa | — |
| а25 | П4в | `skills.rs:208` | Порог `include_in_prompt` — при сборке начала (правка видна в следующем разговоре) | eda76fa | — |
| а26 | П4г | `agent/tools/src/web.rs:320, :178` | Текст отказа чёрного списка; правило совпадения; вид выдачи и страницы, нумерация строк с 0; ответ lookup JSON; срок Firecrawl 120 с | 0b76ed6 | — |
| а27 | П4д | `agent/memory/src/files.rs:11, :58`; `tools.rs:51, :203`; `migrations/0001_memory.sql:9` | Нарезка по абзацам, `chunk_id` FNV-1a; сводка двух поисков; индекс HNSW выведен; окно `memory_get` 40 строк | 2afa437 | — |
| а28 | П4д | `agent/memory/src/store.rs:303, :479, :629` | Свои выборки по файлу, по приставке, «всё выросшее из» утверждения | комментарии | — |
| а29 | П4е-1 | `agent/cron/src/worker.rs:13-15`; `service.rs:421` | Склейка сообщения работнику и заметки главному (в разборе только метки) | 36ab126 | — |
| а30 | П4е-1 | `agent/cron/src/schedule.rs:11-14` | Ключ файла для daily–yearly; минута `flexible_time` из отпечатка id | 36ab126 | — |
| а31 | П4е-1 | `agent/cron/src/tools.rs:218, :418, :504`; `worker.rs:215` | Проверка id kebab-case (текст ошибки свой; в описании muse — «a concise kebab-case identity», `strings-all.txt:85089`); «small hours» = 0–5 ч; `cron.runs` без limit — 20; описания `outcome`/`blocked_*` | 36ab126 | — |
| а32 | П4е-1 | `agent/cron/src/store.rs:9-13` | `job_definitions.task_text` — файл целиком (у muse, вероятно, только тело) | комментарий | — |
| а33 | П4е-2 | `agent/cron/src/hooks.rs:15, :56, :58, :528, :718`; `service.rs:39` | Путь журнала `~/hooks/logs/<id>.jsonl`; срок скрипта по умолчанию 60 с; описание `script_timeout_secs`; `hooks.logs` — 20; ключ `observation_note`; что считается инструкцией в `HEARTBEAT.md`; тире в `[EVENT HOOK — EXECUTE]` | 3224e3f | Умолчание 60 с — можно рядом с проверкой `0x10244065` (не смотрел); предел 1..600 найден (б1) |

## Вид (в) — по слову владельца или по смыслу

| № | Срез | Наш код | Что | Основание |
|---|---|---|---|---|
| в1 | П2а | `config/models.toml` | Имя файла `models.toml`, не `model.toml` | коммит 26ee2df; CLAUDE.md правлен 7721632 |
| в2 | П2б | `agent/model/src/anthropic/mod.rs` | Свой HTTP-клиент (reqwest) вместо SDK — SDK на Rust нет, как у muse | 1023fc9; слово 27.11 (7721632) |
| в3 | П2б | `agent/model/src/registry.rs:101-106`; `config/models.toml` | Свои флаги `anthropic.prompt_cache`, `anthropic.forced_tool_choice`; веса `claude-haiku-5-5` по документации API | 47eb6ad, 1023fc9 — исключение «прослойка модели» |
| в4 | П2в | `registry.rs:137`; `openai_compat/convert.rs:24` | Флаг `openai_compat.max_completion_tokens`; приставка «[ошибка инструмента]» в результате; мышление назад не идёт; `tool_output_error_status` → ошибка настройки | 50fcfba, ed56c4a — исключение «прослойка модели» |
| в5 | П10 | инструменты, `prompts/` | `muse.*` → `hamlet.*`, «Muse» → «Hamlet», путь `/opt/muse/skills` убран, путь навыка forget — из настройки, имя `memory-default` | d5e8e5a, 2afa437, 4dc44f1; слово 27.11 |
| в6 | П3в, П3д | `agent/loop/src/compaction.rs:40` | Только личный вариант текстов (куски `group-only` срезаются); метки переименованы | f6ef533, 4dc44f1 — агент одного владельца |
| в7 | П3д, П4е | `prompts/SOURCES.md` | Из текстов убраны службы, которых у Hamlet нет: профили shopping/travel (forget), артефакты, `chat.send_message`, `device.read_history`, виджеты, окна браузера в `cron.stop`, согласования, `vm_authentication`, `delivery_chat_id`, прокси в hooks, `@user.home/@user.work` | SOURCES.md; коммиты 4dc44f1, 36ab126, 3224e3f. Замечание: артефакты и окна браузера у muse есть — при их появлении тексты вернуть |
| в8 | П4е-1, П4е-2 | `agent/cron/src/tools.rs:9-11`; `hooks.rs:23` | Поля не взяты: `delivery`, `connection_id`, `execution`, `predicted_connector_permissions`, `confirmation`, `cron.add_artifact_action`, `connection.notify_when` | 36ab126, 3224e3f («нет служб»; `delivery` — П6) |
| в9 | П4г | `agent/tools/src/web.rs` | Поиск через Firecrawl вместо служб Meta (MSL/CAGI, perplexity); `verticals` без особой обработки (Finnhub и др. — службы muse); `language_code` в Firecrawl не уходит | b307dbb (слово 27.11), 0b76ed6 |
| в10 | П4г | `web.rs` | Чёрный список проверяется на месте по FST; у muse — удалённым запросом `url_blocklist_guard/lookup.rs` | 0b76ed6 |

## Закрыто позже (для справки)

- П4а: папки ограничены `~` и `/tmp` → пути как у muse (4824367); «closest match» и «Read output capped» сделаны (4824367).
- П4б: pty и `truncated` сделаны (5f4f56e).
- П3а: повтор без ожидания `retry_after` → ждёт (37f3615).
- П3г: подагент изолирован → наследует разговор (П3д, 4dc44f1).
- П4д: forget.plan/confirm сделаны (4dc44f1); сверка памяти перед каждым поиском → на такте службы (36ab126; см. б4).

Счёт: а — 33, б — 23, в — 10.
