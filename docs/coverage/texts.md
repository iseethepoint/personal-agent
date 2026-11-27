# Карта покрытия: тексты, конфиги, навыки, коннекторы, программы и справочник muse (черновик)

Рабочий черновик ревизора, 27.11.2026. Источник — `iseethepoint/muse-live` HEAD (`git ls-tree -r HEAD`, `git show HEAD:<путь>`). Репозиторий Hamlet сверен по `docs/PLAN.md`, `docs/cards/*.md`, `prompts/SOURCES.md`, а также по ссылкам на тексты muse в комментариях `agent/**`.

**Пометки** (правило ревизора с поправкой по слову владельца «нам нужно всё, что есть и умеет muse»):

- **перенесено** — текст есть в `prompts/` репозитория или записан в коде с указанием источника;
- **в плане** — его покрывает строка плана (П3…П10, П4а…П4ж, П5 — тексты, П6 — вход, П8 — сервер, П9 — расширение);
- **нет в плане — нужно** — у muse это есть, Hamlet это нужно, но ни одна строка плана этого не покрывает;
- **нужно — замена службы** — умение берём, а закрытую программу из `bin/` или службу Meta меняем на своё или на публичное API (на что именно — в примечании). Строки плана у таких групп почти никогда нет — см. последний раздел;
- **только приложение** — существует только как экран или виджет приложения muse и без приложения смысла не имеет. Всё такое собрано в отдельный список для решения владельца (раздел 8);
- **служебное** — не текст для модели (список файлов, контрольные суммы);
- **сознательно не взято** — перенесено и удалено с причиной (записано в `prompts/SOURCES.md`).

Сокращения: `P/` = `rootfs-full/opt/hatch-private/prompts/hatch-engine/crates/`, `A/` = `P/hatch-agent/prompts/`.

---

## 1. Тексты движка `opt/hatch-private/prompts/` — 1 561 файл

Группы — по папке (`crate/prompts/<папка>`); крупные папки разбиты там, где у частей разные пометки. Перенесённые файлы выделены в отдельные строки. Подсчёт — скриптом по `git ls-tree`: каждый файл попал ровно в одну строку, всего 1 561.

| Группа | Что это | Файлов | Строка плана | Пометка | Примечание |
|---|---|---|---|---|---|
| `RESCUE_AGENT` | агент-спасатель машины | 1 | П8 | **нужно — замена службы** | hatch-rescue → свой systemd/проверка здоровья |
| `content-understanding` | описание картинки отдельной моделью | 1 | П2 (назначение MediaDescription) | **нет в плане — нужно** | текст не перенесён |
| `hatch-activity` | лента дел для приложения | 13 | — | **только приложение** |  |
| `hatch-agent/activity_monitor` | лента дел для приложения | 4 | — | **только приложение** |  |
| `hatch-agent/artifacts` | сборка веб-артефактов | 37 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 |
| `hatch-agent/background_worker_paged_input` | постраничный ввод фонового работника | 1 | — | **нет в плане — нужно** |  |
| `blocks/activity_monitor` | лента дел подагентов для экрана приложения | 39 | — | **только приложение** |  |
| `blocks/agent` | системный текст подагента/работника | 38 | П5 (П3г) | **в плане** | в П3г тексты не перенесены — механизм на своих |
| `blocks/anthropic/cedar` | поиск инструментов поставщика Anthropic | 2 | П2 (флаг tool_search) | **нет в плане — нужно** | только при флаге anthropic.tool_search |
| `blocks/artifact_builder` | сборщик сайтов/документов (artifacts, Spaces) | 7 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 / Vercel, Cloudflare |
| `blocks/artifacts` | сборщик сайтов/документов (artifacts, Spaces) | 15 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 / Vercel, Cloudflare |
| `blocks/background` | фоновый исполнитель: шаг цели, проверка, почта | 6 | П5 | **в плане** | mail_conversation — почта агента: замена службы |
| `blocks/browser` | предварительное согласие на оплату в браузере | 1 | — | **нужно — замена службы** | кошелёк Meta → свой путь оплаты (Stripe Link) |
| `blocks/chat` | системный текст главного разговора (кто ты, как работаешь, безопасность) | 18 | П5 | **в плане** |  |
| `blocks/company_conversation` | переписка с компанией от имени владельца | 1 | — | **нужно — замена службы** | канал компаний Meta → почта/чат компании напрямую |
| `blocks/context_worker` | фоновый работник контекста (проактивность) | 3 | — | **нет в плане — нужно** |  |
| `blocks/deep_research` | агент-исследователь | 9 | — | **нет в плане — нужно** | глубокое исследование — нет строки |
| `blocks/device_sync` | синхронизация данных с телефона | 4 | — | **нужно — замена службы** | данные телефона → свой клиент/CalDAV, CardDAV |
| `blocks/event_hook/filesystem` | файлы работника наблюдателя | 1 | П4е-2 | **нет в плане — нужно** | не перенесён в П4е-2 |
| `blocks/event_hook` | текст работника наблюдателя | 3 | П4е-2 | **перенесено** | prompts/hooks |
| `blocks/group_chat` | память и состояние группового чата | 5 | — | **нет в плане — нужно** | групповые чаты (умение агента) |
| `blocks/group_muse` | групповой агент-работник | 1 | — | **нет в плане — нужно** | групповые чаты |
| `blocks/ideas_builder` | сборщик «идей» (вкладка Ideas) | 14 | — | **только приложение** | вкладка приложения Ideas |
| `blocks/media_upload` | разбор загруженных медиа | 2 | — | **нужно — замена службы** | загрузка из приложения → вложения мессенджера П6 |
| `blocks/memory_flush` | системный текст сброса памяти | 3 | П3в | **нет в плане — нужно** | в П3в взят только memory_flush_prompt |
| `blocks/peer_chat` | разговор агента с другим агентом/людьми от имени владельца | 28 | — | **нужно — замена службы** | сеть агентов Meta → свой протокол агент-агент (A2A) |
| `blocks/peer_sync` | синхронизация с агентом-собеседником | 3 | — | **нужно — замена службы** | то же: свой протокол агент-агент |
| `blocks/proactivity` | проактивная работа | 1 | — | **нет в плане — нужно** |  |
| `blocks/scheduler_resolution` | системный текст исхода расписания | 6 | П4е-1 | **нет в плане — нужно** | не перенесён; в П4е-1 взят contract |
| `blocks/scheduler_worker` | кто ты / как заканчивается запуск расписания | 2 | П4е-1 | **нет в плане — нужно** | не перенесён |
| `blocks/self_improvement` | системный текст циклов самоулучшения | 3 | — | **нет в плане — нужно** |  |
| `blocks/shared` | общие блоки безопасности, доверия, времени, правил инструментов | 38 | П5 | **в плане** |  |
| `blocks/shared/environment` | описание окружения: hooks, cron, навыки, подагенты | 4 | П5 (П4е/П4в/П3г) | **в плане** |  |
| `blocks/side_chat` | побочный чат: мессенджер, группы | 48 | П6 | **в плане** | групповая часть — нет строки |
| `blocks/space_task` | сборщик сайтов/документов (artifacts, Spaces) | 4 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 / Vercel, Cloudflare |
| `blocks/spaces_builder` | сборщик сайтов/документов (artifacts, Spaces) | 24 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 / Vercel, Cloudflare |
| `blocks/spaces_builder_lite` | сборщик сайтов/документов (artifacts, Spaces) | 12 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 / Vercel, Cloudflare |
| `blocks/subagents` | куски текста о порождении подагентов | 16 | П5 (П3г) | **в плане** |  |
| `blocks/sync_worker` | работник синхронизации устройств | 2 | — | **нужно — замена службы** | то же |
| `blocks/web_artifacts_inspector` | сборщик сайтов/документов (artifacts, Spaces) | 5 | — | **нужно — замена службы** | хостинг Spaces → свой сервер П8 / Vercel, Cloudflare |
| `hatch-agent/browser` | браузерный исполнитель: схемы, инструкции, сценарии | 163 | нет строки (карточка П4 «дальше») | **нет в плане — нужно** | живой браузер; Chromium — П8 |
| `hatch-agent/compact` | заметки сжатия: упор в предел ответа, без инструментов | 4 | П3в | **нет в плане — нужно** | в П3в не перенесено |
| `hatch-agent/completion_gate` | проверка завершения (идеи, Spaces) | 2 | — | **только приложение** | оба про идеи/Spaces; сам механизм completion_gate — нет строки |
| `hatch-agent/goals` | сводка активности цели | 2 | П4ж | **в плане** |  |
| `hatch-agent/inline` | шаги работника целей самоулучшения (JSON) | 9 | — | **нет в плане — нужно** |  |
| `hatch-agent/owner_mail_pi_*` | проверка писем владельцу на внедрённые команды | 8 | — | **нужно — замена службы** | почта агента → свой ящик; проверка — модель назначения |
| `hatch-agent/peer_*` | исследование для агента-собеседника | 2 | — | **нужно — замена службы** | свой протокол агент-агент |
| `hatch-agent/private_copy` | проактивная «личная копия» агента | 18 | — | **нет в плане — нужно** |  |
| `hatch-agent/proactivity` | карточки проактивности в общем пуле | 3 | — | **только приложение** | карточки контента приложения |
| `hatch-agent/product_messages` | сообщения продукта о связях | 2 | — | **только приложение** |  |
| `hatch-agent/push_notification` | push-уведомления | 2 | — | **нужно — замена службы** | push приложения → сообщение в мессенджер П6 |
| `hatch-agent/research` | возобновление исследования после перезапуска | 1 | — | **нет в плане — нужно** | глубокое исследование |
| `session: сжатие и подагенты` | сброс памяти, контекст подагента | 7 | П3в/П3г/П3д | **перенесено** | prompts/compaction, prompts/subagent |
| `session/artifact_routing` | маршрут к артефактам | 1 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `session/browser_*` | маршрут в браузер, время задачи | 3 | нет строки | **нет в плане — нужно** | живой браузер |
| `session/connection_*` | связи с другими людьми/агентами | 6 | — | **нужно — замена службы** | свой протокол агент-агент |
| `session/context_file_definitions` | SOUL/IDENTITY в побочном чате | 2 | П5/П6 | **в плане** |  |
| `session/прочее` | дата и время, вспоминание памяти, отслеживание, стоящий контекст | 4 | П5 | **в плане** | current_date_time, memory_recall_recommendations, standing_context_header, tracking_context_section_preamble |
| `session/device_sync_handoff_guidance` | передача с устройства | 1 | — | **нужно — замена службы** | свой клиент |
| `session/group_discretion_*` | осторожность в группе | 2 | — | **нет в плане — нужно** | групповые чаты |
| `session/mailbox_send_*` | отправка через «ящик» связи (письма агентам/людям) | 12 | — | **нужно — замена службы** | почтовый ящик muse → свой IMAP/SMTP |
| `session/peer_*` | собеседник-агент, проверка, отношения | 16 | — | **нужно — замена службы** | свой протокол агент-агент |
| `session/proactivity_*` | проактивность: доставка, навыки | 2 | — | **нет в плане — нужно** |  |
| `session/relationship_*` | маршрут связи | 3 | — | **нужно — замена службы** | то же |
| `session/share_card` | карточка «поделиться» | 1 | — | **только приложение** |  |
| `session/skills_blocked_escalation_*` | навык недоступен — поднять выше | 2 | П4в | **нет в плане — нужно** | не перенесено |
| `session: подагенты (не перенесены)` | координатор глубины 1, порождение | 3 | П3д | **нет в плане — нужно** | карточка П3д их называет, в prompts/ нет; проверить, встроены ли в код |
| `session: изоляция глубже 1` | подагент глубже первого | 1 | П3д | **сознательно не взято** | у Hamlet MAX_SPAWN_DEPTH=1; перенесён и удалён в П3д |
| `session/texting_reminder` | стиль переписки в мессенджере | 1 | П6 | **в плане** |  |
| `session/title*` | заголовок разговора | 2 | — | **только приложение** | список чатов в приложении |
| `hatch-agent/subagent_monitor` | наблюдатель за подагентами: notify/nothing_to_do | 7 | П3г | **нет в плане — нужно** | не перенесён |
| `tools/artifact_*` | описания инструментов артефактов | 6 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `tools/browser_*` | маршрут задачи браузера | 2 | нет строки | **нет в плане — нужно** | живой браузер |
| `tools/chat_*` | описания chat.* (чтение) | 3 | П6 | **в плане** |  |
| `tools/company_conversation` | разговор с компанией | 13 | — | **нужно — замена службы** | канал компаний Meta → почта/чат компании |
| `tools/connections` | доставка по своей связи | 6 | П6 | **в плане** |  |
| `tools/create_options_off_screen` | карточка выбора вне экрана | 1 | — | **только приложение** | виджет |
| `tools/create_shopping_*` | карточки покупок вне экрана | 2 | — | **только приложение** | виджет |
| `tools/credentials_*` | запрос входа | 1 | — | **нужно — замена службы** | хранилище muse → своё хранилище секретов |
| `tools/delegate*` | маршрут к главному агенту, статус артефакта | 4 | П3г | **нет в плане — нужно** |  |
| `tools/device*` | устройства, история SMS | 5 | — | **нужно — замена службы** | приложение телефона → свой клиент |
| `tools/feed_*` | лента | 2 | — | **только приложение** | вкладка Feed |
| `tools/generate_media_*` | генерация медиа | 3 | — | **нужно — замена службы** | публичное API генерации картинок |
| `tools/group_*` | реакции и осторожность в группе | 8 | — | **нет в плане — нужно** | групповые чаты |
| `tools/ideas_*` | передача в сборщик идей | 2 | — | **только приложение** | вкладка Ideas |
| `tools/linked_chat` | связанные чаты: чтение, отправка | 6 | П6 | **в плане** |  |
| `tools/local_search_*` | поиск мест рядом | 2 | — | **нужно — замена службы** | local-search → Google Places API |
| `tools/mail_*` | почтовые задачи | 7 | — | **нужно — замена службы** | почта агента → IMAP/SMTP |
| `tools/mailbox` | ящик связи: отправка, чтение | 24 | — | **нужно — замена службы** | свой протокол агент-агент / почта |
| `tools/media_style_*` | примерка товара | 2 | — | **нужно — замена службы** | публичное API генерации картинок |
| `tools/memory_reconcile_queued` | сверка памяти в очереди | 1 | П4д/П4е | **нет в плане — нужно** | не перенесено |
| `tools: notify/nothing_to_do` | описания инструментов | 2 | П4е | **перенесено** | в коде agent/cron и prompts/hooks |
| `tools/nothing_to_do_*_routing` | маршрут nothing_to_do | 1 | П4е | **нет в плане — нужно** |  |
| `tools/notify_main_agent_*` | маршрут/поле notify_main_agent | 2 | П4е | **нет в плане — нужно** | description перенесён, routing и message — нет |
| `tools/peer_gated` | инструменты, закрытые для собеседника | 4 | — | **нужно — замена службы** | свой протокол агент-агент |
| `tools/phone_destination_verification` | проверка номера звонка | 1 | — | **нужно — замена службы** | телефония → Twilio API |
| `tools/places_*` | сведения о месте | 2 | — | **нужно — замена службы** | Google Places API |
| `tools/poppy_*` | Poppy — магазинный агент Meta | 4 | — | **нужно — замена службы** | Poppy → живой браузер |
| `tools/proactivity` | проактивность: передача, запись | 7 | — | **нет в плане — нужно** |  |
| `tools/prompt_injection_browser_*` | заметки о подозрительной странице | 2 | — | **нет в плане — нужно** | проверка внедрённых команд — нет строки |
| `tools/relationships_*` | поиск чата/связи человека | 3 | — | **нет в плане — нужно** | люди и отношения |
| `tools/set_requested_voice_routing` | выбор голоса | 1 | — | **нужно — замена службы** | голос → свой TTS |
| `tools/sharing_settings` | доступ к артефакту | 1 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `tools/spaces_builder` | исследование для сборщика Spaces | 3 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `tools/ucp_*` | покупки через UCP (корзина, оплата) | 10 | — | **нужно — замена службы** | ucp-cli → открытый протокол UCP / магазин напрямую |
| `tools/verification_not_relevant` | проверка исхода не нужна | 1 | — | **нет в плане — нужно** | проверка исхода |
| `tools/widget_creation` | виджет вне экрана | 1 | — | **только приложение** |  |
| `tools/worker_input_page` | страница ввода работника | 1 | — | **нет в плане — нужно** |  |
| `hatch-agent/src/avatar` | аватар агента: изображения, танцы | 41 | — | **только приложение** | аватар на экране приложения |
| `hatch-agent/src/safety_classifiers` | классификаторы внедрённых команд и надзора | 12 | — | **нет в плане — нужно** | безопасность на модели назначения |
| `hatch-agent/src/subscriptions` | подписки на события (лента, устройства, Spaces) | 3 | — | **только приложение** |  |
| `hatch-agent/voice-prompts` | голосовой разговор, звонки, очки, компаньон | 98 | — | **нужно — замена службы** | голос Meta → свой TTS/ASR (публичное API или локально) |
| `hatch-alignment` | ночная рефлексия, исправление разрыва | 2 | — | **нет в плане — нужно** |  |
| `hatch-compaction/chat_worker_conversation` | сжатие для чат-работника (побочный чат) | 1 | П6 | **в плане** | с побочными чатами |
| `hatch-compaction/segmented_*` | сжатие по частям (длинная история) | 3 | П3в (форма) | **нет в плане — нужно** | в П3в не перенесено |
| `hatch-compaction: основное` | сжатие | 3 | П3в | **перенесено** | prompts/compaction |
| `hatch-config` | контрольные суммы | 1 | — | **служебное** |  |
| `hatch-context-workers` | проактивные работники: срочность, полёты, уведомления, исследование | 60 | — | **нет в плане — нужно** | проактивность |
| `hatch-cron/product_messages` | онбординг-тур приложения | 1 | — | **только приложение** |  |
| `hatch-cron/scheduler: работник` | текст работника расписания | 5 | П4е-1 | **перенесено** | prompts/cron |
| `hatch-cron/scheduler/goal_*` | расписание цели | 4 | П4ж | **в плане** |  |
| `hatch-cron/tools/cron/confirmation_*` | карточка подтверждения расписания (виджет) | 4 | — | **только приложение** | виджет приложения |
| `hatch-cron/tools: исход` | заметки о незавершённом запуске | 2 | П4е-1 | **перенесено** | prompts/cron |
| `hatch-feed` | редакция ленты (Feed) | 87 | — | **только приложение** | вкладка Feed; спорно — можно слать в мессенджер |
| `hatch-goals/briefing-designs` | оформление брифингов цели | 1 | — | **только приложение** | экран цели |
| `hatch-goals/studying` | изучение целей | 3 | П4ж (только форма) / самоулучшение | **нет в плане — нужно** | цикл — нет строки |
| `hatch-inference-proxy` | определение языка подсказки для журнала | 2 | — | **нет в плане — нужно** | мелкое, журнал |
| `hatch-inference/voice` | подбор голоса | 8 | — | **нужно — замена службы** | голос → свой TTS |
| `hatch-media-runner` | генерация картинок/видео, подбор картинок | 7 | — | **нужно — замена службы** | публичное API генерации медиа и поиска картинок |
| `hatch-media` | описание медиа | 1 | П2 (MediaDescription) | **нет в плане — нужно** |  |
| `hatch-objective-execution` | шаги цели памяти (JSON) | 1 | — | **нет в плане — нужно** | самоулучшение |
| `hatch-self-improvement (memory, skill_improvement, relationships, studying, alignment, goals_bookkeeping, travel, lenses, _shared, inline)` | фоновые циклы самоулучшения | 88 | — | **нет в плане — нужно** | нет строки плана |
| `hatch-self-improvement/ideas` | генерация идей для вкладки Ideas | 40 | — | **только приложение** |  |
| `hatch-self-improvement/inline (ideas)` | цель цикла идей | 1 | — | **только приложение** |  |
| `hatch-self-improvement/shopping` | профиль покупателя | 5 | — | **нет в плане — нужно** | цикл самоулучшения |
| `hatch-server/browser_* (taggers, terminal_update)` | разметка задач браузера, итог задачи | 31 | нет строки | **нет в плане — нужно** | живой браузер |
| `hatch-server/chat_worker_*` | работник побочного чата: ящик, передачи, итоги | 20 | П6 | **в плане** | часть про пиров — агент-агент |
| `hatch-server/company_conversation` | разговор с компанией: статусы | 18 | — | **нужно — замена службы** | канал компаний → почта/чат компании |
| `hatch-server/connection_hook_execute` | наблюдатель по связи | 1 | П4е | **нет в плане — нужно** |  |
| `hatch-server/coordination_*` | координация агентов | 2 | — | **нужно — замена службы** | свой протокол агент-агент |
| `hatch-server/device_sync` | кандидат проактивности с устройства | 1 | — | **нужно — замена службы** | свой клиент |
| `hatch-server/goals` | внимание к целям, жизненный цикл отслеживания | 4 | П4ж | **в плане** |  |
| `hatch-server/group_chat` | групповой чат: проверка осторожности | 17 | — | **нет в плане — нужно** | групповые чаты |
| `hatch-server/group_muse_peer_task` | задание группового агента | 1 | — | **нет в плане — нужно** | групповые чаты |
| `hatch-server/internal` | синхронизация контекста файлового сервера | 1 | П8 | **нет в плане — нужно** |  |
| `hatch-server/muse_mail` | почта агента | 5 | — | **нужно — замена службы** | почта Meta → свой IMAP/SMTP |
| `hatch-server/peer_space*` | правка Space собеседником | 6 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `hatch-server/product_messages` | туры по устройству, онбординг | 25 | — | **только приложение** |  |
| `hatch-server/purchase_approval_intent` | намерение одобрить покупку | 2 | — | **нужно — замена службы** | кошелёк → свой путь оплаты |
| `hatch-server/self_improvement` | лечение исполнения идей, лента идей | 4 | — | **только приложение** | вкладка Ideas |
| `hatch-server/space_*` | шаблоны и проверка Spaces | 4 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `hatch-server/user_sentiment` | настроение пользователя | 1 | — | **нет в плане — нужно** | самоулучшение/выравнивание |
| `hatch-server/verification` | проверка исхода задач (покупки, брони, расписания) | 10 | — | **нет в плане — нужно** |  |
| `hatch-server/voice` | голосовой звонок, подкасты | 10 | — | **нужно — замена службы** | голос → свой TTS/ASR |
| `hatch-server/workflows` | структурный ответ агента для workflow | 1 | — | **нет в плане — нужно** | workflow.* — нет строки |
| `hatch-server/src (phone)` | звонки: итог, судья | 6 | — | **нужно — замена службы** | телефония → Twilio API + свой TTS/ASR |
| `hatch-spaces` | иконка Space | 1 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `hatch-tools/artifact routing` | маршрут инструментов артефактов | 4 | — | **нужно — замена службы** | Spaces → свой хостинг |
| `hatch-tools/channel_routing` | маршрут канала | 1 | П6 | **в плане** |  |
| `hatch-safety` | проверки кибер/CBRNE/TBAC, отказ | 20 | — | **нет в плане — нужно** | безопасность на модели назначения |
| `hatch-sentinel` | проверка выхода в сеть | 5 | П8 | **нет в плане — нужно** | выход в сеть — П8 не называет |
| `hatch-skills/media-generation` | аватар: подписи кадров | 3 | — | **только приложение** | аватар |
| `hatch-skills/wearables` | интерфейс очков (genui) | 9 | — | **нужно — замена службы** | очки Meta → публичной замены нет; неясно |
| `manifest.txt` | список файлов | 1 | — | **служебное** |  |

**Замечания к разделу 1.**

- «Перенесено» — 22 файла из `P/`: 4 текста сжатия (`summarization`, `summary_rules`, `system`, `session/memory_flush_prompt`), 6 текстов подагентов из `session/`, 7 текстов `hatch-cron` (5 в `scheduler/` и 2 в `tools/`), 3 текста `blocks/event_hook`, а также `tools/nothing_to_do_without_reaction_description.md` и `tools/notify_main_agent_description.md` (второй записан в коде, `agent/cron/src/worker.rs:43`). Ещё 3 файла `prompts/cron/` взяты из ядра (`main_agent_handoff_intro`, `cron_namespace`, `tools.json`) и 1 — из `hooks/tools.json`; в 1 561 они не входят.
- Тексты `blocks/agent`, `blocks/subagents`, `blocks/shared`, `blocks/chat`, `blocks/side_chat`, `session/прочее` и 88 блоков `blocks/user_chat` (они есть только в ядре, раздел 1а) отнесены к П5 «системный текст». Но П5 в плане описан одной строкой, без карточки: какие именно блоки в него войдут, не записано.
- `blocks/memory_flush`, `blocks/scheduler_resolution`, `blocks/scheduler_worker`, `blocks/event_hook/filesystem.md`, `hatch-agent/compact/*`, `hatch-compaction/segmented_*` — системные тексты работников, которые уже есть в движке (сжатие, расписания, наблюдатели). В срезах П3в и П4е их не взяли. Отнесены к «нет в плане — нужно»: неясно, войдут ли они в П5 или их надо добрать в уже готовые срезы.
- `session/subagent_context_depth_one_coordinator.md`, `subagent_context_depth_one_spawn_inheritance.md`, `subagent_spawning.md` прочитаны в П3д (на `depth_one_spawn_inheritance` есть ссылка в `agent/loop/src/subagents.rs:8`), но текстами не перенесены. У Hamlet `MAX_SPAWN_DEPTH = 1`, а muse умеет порождать подагентов глубже первого уровня (глубже первого они изолированы). Это расхождение с мерилом, и в плане его нет.
- `hatch-feed` (87 файлов, редакция ленты) помечена «только приложение», но это спорно: тот же конвейер мог бы присылать выпуски в мессенджер. Решает владелец.

### 1а. Блоки `blocks/user_chat/*.md` — только в `A/../core-v2-analysis/strings-all.txt`

`grep -o 'blocks/user_chat/[A-Za-z0-9_@]*\.md' core-v2-analysis/strings-all.txt | sort -u` даёт 97 имён: 88 основных и 9 вариантов с `@` (`@without_todo_list`, `@texting_style`). По П1б у 2 блоков (`conversation.md`, `secure_vault.md`) в ядре есть только имя, без текста.

| Что | Сколько | Строка плана | Пометка |
|---|---|---|---|
| Системный текст главного разговора: `who_you_are`, `how_you_work`, `memory*`, `tools*`, `security*`, `recurring_work*`, `goals`, `docs`, `loading_tools`, `active_work*`, `turn_*_reminder` и др. | 97 имён (88 + 9 вариантов) | П5 | **в плане** |
| Из них про службы, которые нужно заменить: `secure_vault*` (5), `purchases`, `shared_spaces`, `artifacts`, `browser_and_sources*` (3), `captchas` | 12 | П5 + замена службы | **нужно — замена службы** — хранилище секретов, оплата, хостинг артефактов, живой браузер |
| `loading_tools` — `tool_search.load_tool_namespace` | 1 | долг в строке П4 (своей строки нет) | долг |

---

## 2. Конфиги `opt/hatch-private/configs/` — 69 файлов

| Файл или папка | Что это | Файлов | Пометка |
|---|---|---|---|
| `blocklist/manifest.json`, `mase_all_domains.fst`, `mase_blocklist_regex.json` | чёрный список адресов для поиска и страниц | 3 | **перенесено** (П4г, `config/blocklist/`) |
| `blocklist/mase_all_urls_fingerprints.bin` (60 МБ, 7,5 млн адресов), `mase_reduced_domains.fst`, `mase_contract_domains.fst` | список отдельных адресов, укороченный и «договорной» списки доменов | 3 | **нет в плане — нужно** — в П4г взяты только домены и regex. Где muse применяет `reduced` и `contract`, неясно |
| `hatch-agent/src/prompt_injection_checker/config.json` + `policies/*.json` (9) | проверка внедрённых команд в выводе инструментов, браузере и чатах (отдельная модель) | 10 | **нет в плане — нужно** — проверка на модели назначения |
| `hatch-agent/src/safety_classifiers/policies/*.json` | проверка вызовов: кибер, CBRNE, утечка данных | 5 | **нет в плане — нужно** |
| `hatch-agent/src/safety_throttle.json` | пределы после нарушений (например, tbac — 5 за 24 ч, 10 за 168 ч) | 1 | **нет в плане — нужно** |
| `hatch-agent/src/tools/registry/safety_escalation.json` | эскалация опасных вызовов в реестре инструментов | 1 | **нет в плане — нужно** |
| `hatch-agent/src/browser_input_gateway/file_scan_policy.json` | проверка загрузки файлов в браузере | 1 | **нет в плане — нужно** (вместе с живым браузером) |
| `hatch-inference/src/policy_prompt.json` | строгость проверки безопасности | 1 | **нет в плане — нужно** |
| `hatch-inference/src/safety_classifier/activity_config.json` | настройка классификатора активности | 1 | **нет в плане — нужно** |
| `mango-safety-output/src/config/*` | проверка аватара и выходных картинок: несовершеннолетние, персонажи, публичные лица | 7 | **нужно — замена службы** — модерация созданных медиа на модели назначения (вместе с генерацией медиа) |
| `mango-safety/src/config/*` | проверка генерации картинок и видео, маршрутизатор, похожесть, публичные лица, регион India | 25 | **нужно — замена службы** — то же |
| `hatch-safety/crates/hatch-safety/**` (`metadata.json` ×2, `verification_code_triage/*` ×2, `hatch_cyber_prompt/config.json`, `cyber_response_jev_20261029.json`, `tbac_config.json`) | кибер, TBAC, разбор кодов подтверждения | 7 | **нет в плане — нужно** |
| `hatch-sentinel/policies/network_egress_*.json` | правила выхода в сеть (модель-классификатор) | 2 | **нет в плане — нужно** (рядом с П8, но в П8 не названо) |
| `hatch-config/src/safety_config/checksums.json` | контрольные суммы | 1 | **служебное** |
| `manifest.txt` | список файлов | 1 | **служебное** |

Итого: перенесено 3, нет в плане — нужно 32, нужно — замена службы 32, служебное 2 = **69**.

---

## 3. Навыки

### 3.1. `opt/hatch/workflow-skills/` — 58 навыков, 437 файлов

В задании названо 59, но в HEAD 58 папок (`git ls-tree` → 58 уникальных имён). Столько же записано в описи (`docs/muse_inventory.md:33`).

Пометки: **П5** — текст переносится, навык опирается только на общие инструменты и коннекторы; **П5 + замена службы** — в тексте навыка прямо вызываются программы из `bin/` или пространства служб Meta (`widget.*`, `shopping.*`, `media.*`, `wallet.*`, `phone.*`, `artifact.*`, `credentials.*`); **нет в плане — нужно**; **только приложение**. Колонка «Опирается» — что найдено в файлах навыка.

| Навык | Файлов | О чём | Опирается | Пометка |
|---|---|---|---|---|
| `advertising-campaigns` | 10 | платная реклама: цель, аудитория, бюджет, анализ | коннекторы (meta_ads и др.) | **П5** |
| `avatar-dance-videos` | 2 | танцующие видео аватара | `avatar.edit` | **только приложение** |
| `business-finance` | 2 | сверка счетов, инвойсы, оплаты | коннекторы | **П5** |
| `custom-connector-setup` | 11 | подключить службу по API/MCP, сохранить коннектор и навык | `credentials.request_api_access` | **П5 + замена службы** — своё хранилище секретов; связано с П9 |
| `customer-outreach` | 2 | рассылки клиентам с согласия | коннекторы | **П5** |
| `device-data` | 2 | кэш контактов и календаря с телефона | хранилище muse | **П5 + замена службы** — CardDAV/CalDAV или свой клиент |
| `documents` | 26 | документы, PDF, таблицы, презентации | `hatch-slide-style`, `media.generate_image`, `artifact.*` | **П5 + замена службы** — свой стиль слайдов, публичное API картинок, файлы на сервере |
| `email-management` | 2 | почта: разбор, черновики, отправка | коннекторы почты | **П5** |
| `event-tickets` | 3 | билеты: поиск, покупка, перепродажа | браузер, ticketmaster | **П5** |
| `file-hosting` | 2 | выложить файл и дать ссылку | `remote-storage` | **П5 + замена службы** — S3/R2 или сервер П8 |
| `financial-planning` | 2 | бюджет, долги | коннекторы (plaid) | **П5** |
| `fitness-tracking` | 2 | активность, сон, восстановление | коннекторы здоровья | **П5** |
| `flight-booking` | 8 | поиск и покупка перелёта | `flights`, `widget.create_list_widget`, `wallet.*` | **П5 + замена службы** — Duffel API / браузер, оплата своя |
| `flight-tracking` | 4 | статус рейса и слежение | `widget.create_agenda_widget` | **П5 + замена службы** — FlightAware AeroAPI, ответ текстом |
| `forget` | 3 | забыть по просьбе владельца | `product-lists` | **П5** (механизм `forget.plan/confirm` — П3д; `product-lists` — своё) |
| `form-design` | 2 | формы и опросы | `artifact.create_web_fullstack` | **П5 + замена службы** — свой хостинг |
| `generate_podcast` | 18 | сценарий и озвучка подкаста | TTS (`needs_speech`), `remote-storage`, `ui.list` | **П5 + замена службы** — публичное TTS API |
| `goals` | 16 | как вести цели | `goals.*` | **П5** (инструмент — П4ж) |
| `groceries` | 2 | продукты на доставку | `shopping.*` | **П5 + замена службы** — коннекторы instacart/walmart или браузер |
| `health-review` | 2 | сводка медицинских записей | коннекторы | **П5** |
| `home-control` | 2 | устройства дома, камеры | Home Link | **П5 + замена службы** — Home Assistant API с сервера |
| `home-search` | 2 | поиск жилья, просмотры | браузер | **П5** |
| `hotel-booking` | 5 | сравнение и бронь жилья | `hotels`, `wallet.*` | **П5 + замена службы** — браузер / партнёрские API |
| `image-search` | 2 | поиск картинок | `image-search` | **П5 + замена службы** — публичный поиск картинок |
| `knowledge-organization` | 2 | заметки: Notion, Evernote, Docs | коннекторы | **П5** |
| `live-page-checks` | 2 | цена и наличие по свежей копии страницы | `browser.open` (П4г) | **П5** |
| `marketplaces` | 2 | покупка и продажа б/у | браузер | **П5** |
| `media-library` | 2 | фотоархив владельца | `media-library` | **П5 + замена службы** — папка на сервере / Google Photos API |
| `meeting-follow-up` | 2 | итоги встречи, задачи | `artifact.create_file` | **П5 + замена службы** — файл на сервере |
| `messaging` | 2 | писать людям от имени владельца | `chat.send_message` | **П5** (каналы — П6) |
| `mortgage-comparison` | 2 | сравнение ипотеки | — | **П5** |
| `muse-early-access` | 2 | программа раннего доступа Muse | `feature-request` | **только приложение** (продукт muse) |
| `muse-feedback` | 2 | отзыв команде Muse | `feature-request` | **только приложение** (продукт muse) |
| `muse_db` | 199 | SQL по своей базе; 199 файлов — описание схемы muse | `spaces.*`, `feed.*`, `chat.*` и др. (таблицы) | **нет в плане — нужно** — инструмент `hamlet.db` и описание своей схемы |
| `music-discovery` | 2 | музыка, плейлисты, воспроизведение | коннекторы (spotify) | **П5** |
| `place-discovery` | 3 | места: рестораны, парки | `widget.create_map_widget` | **П5 + замена службы** — Google Places API, ссылка на карту |
| `proactive-work` | 16 | личная копия агента работает сама | `widget.*`, `media.*`, `artifact.*` | **нет в плане — нужно** (проактивность) |
| `remember-products` | 2 | сохранённые товары | `product-lists`, `shopping.*`, `artifact.*` | **П5 + замена службы** — список в файлах |
| `restaurant-reservations` | 3 | бронь столика | коннекторы, браузер | **П5** |
| `scheduling` | 2 | подбор времени, приглашения | `phone.begin_call` | **П5 + замена службы** — звонок через Twilio |
| `self-awareness` | 4 | ответы о себе по своим файлам | — | **П5** |
| `share-ideas` | 2 | публикация Idea на muse.ai | `idea.*`, `widget.embed_token` | **только приложение** |
| `shopping` | 6 | товары: поиск, сравнение, покупка | `shopping.*`, `facebook-cli` | **П5 + замена службы** — Firecrawl/браузер |
| `smb-product-shots` | 7 | улучшить фото товара | `media.generate_image` | **П5 + замена службы** — публичное API картинок |
| `social-content-performance` | 3 | статистика своих соцсетей | коннекторы | **П5** |
| `social-publishing` | 4 | посты для соцсетей | `media.generate_*` | **П5 + замена службы** — публичное API медиа |
| `spoken-audio` | 3 | голосовые сообщения, озвучка | `tts` | **П5 + замена службы** — публичное TTS API или локальный синтез |
| `store-operations` | 2 | интернет-магазин: каталог, заказы | коннекторы (shopify) | **П5** |
| `style-tryon` | 4 | примерка одежды на фото | `media.generate_image`, `shopping` | **П5 + замена службы** — публичное API картинок |
| `subscription-status` | 2 | подписка Muse | — | **только приложение** (продукт muse) |
| `task-management` | 2 | задачи в Todoist/Asana/Linear | коннекторы | **П5** |
| `transcribe-recordings` | 2 | расшифровка аудио и видео кусками | распознавание речи | **П5 + замена службы** — Whisper (API или локально) |
| `translation` | 2 | перевод | — | **П5** |
| `trip-planning` | 7 | маршрут поездки | `chat.create`, `ui.*` | **П5 + замена службы** — без экранов приложения, в одном разговоре |
| `trip-transport` | 3 | транспорт в поездке | браузер | **П5** |
| `wearables-comms` | 3 | звонки и SMS с очков | `device-data`, устройства | **П5 + замена службы** — публичной замены нет (очки Meta), неясно |
| `wide-research` | 2 | широкое параллельное исследование | подагенты | **П5** |
| `workout-planning` | 2 | программа тренировок | — | **П5** |

Итого: П5 — 26, П5 + замена службы — 25, нет в плане — нужно — 2, только приложение — 5 = **58**.

### 3.2. `opt/hatch/skills/` — 149 записей: 140 ссылок + 9 папок

- **140 ссылок**: 109 ведут на `/opt/hatch/connectors/*`, 31 — на `/opt/muse/skills/*` (это `workflow-skills`). Пометка: **в плане (П5/П9)** — общий список навыков и коннекторов, его повторяем списком или ссылками.
- **9 настоящих папок** — это ресурсы для навыков, а не навыки с `SKILL.md` (свой `SKILL.md` есть только у `artifacts/testing`):

| Папка | Файлов | Что | Пометка |
|---|---|---|---|
| `artifacts` | 48 | скрипты и справка для документов, PDF, таблиц, презентаций, карт (`map-runtime`), `testing/SKILL.md`; вызывает `local-search`, `places`, `product-lists`, `web-search`, `bun` | **нужно — замена службы** — Places API, свой поиск, файлы на сервере. Строки плана нет |
| `audio` | 1 | `transcribe_chunks.py` для `transcribe-recordings` | **П5 + замена службы** — Whisper |
| `booking` | 1 643 | справка по броням (`references/*`) и логотипы авиакомпаний | **П5** |
| `connections` | 2 | `README.md` (как показывать сообщения связи: виджеты, карточки), `cards.css` | **только приложение** |
| `home-assistant` | 5 | ресурсы для установки Home Assistant на машину агента | **П5 / П8** (нужно) |
| `instagram` | 1 | `engagement_brief.py` (`instagram-cli`, `facebook-cli`) | **П5 + замена службы** — Instagram Graph API |
| `proactivity` | 2 102 | HTML-шаблоны карточек проактивности (`cards/*`, `catalog.json`) | **только приложение** |
| `spaces` | 8 914 | шаблоны и среда исполнения Spaces (сайты агента), иконки | **нужно — замена службы** — свой хостинг (сервер П8 / Vercel, Cloudflare). Строки плана нет |
| `voice-selector` | 3 | каталог голосов для `voice.options` | **нужно — замена службы** — свой TTS |

---

## 4. Коннекторы `opt/hatch/connectors/` — 208 (713 файлов: 208 `connector.yaml`, 208 `CONNECTOR.md`, 205 `manifest.yaml`, 92 прочих)

Сгруппированы по программе из `bin/`, которую вызывают `connector.yaml` / `CONNECTOR.md` / `manifest.yaml`. Тексты всех 208 — **П5** (описания берём). Пометки ниже относятся к программе, на которой коннектор работает.

### 4.1. На общем MCP-клиенте `adobe-connector-cli` — 130

Все 130 имеют в `manifest.yaml` адрес официального MCP-сервера службы (`mcp.endpoint` или `mcp.path`) и OAuth. **Пометка: нужно — замена службы: свой MCP-клиент + OAuth к тому же адресу.** Закрытая программа не нужна. Строки плана нет (П9 даёт только «коннектор описанием»).

`agentman_skills`, `agenttask`, `ahrefs`, `ai_manga_studio`, `airtable`, `app_00widget`, `argos`, `arketa`, `aura_historia`, `basecamp`, `beehiiv`, `bulkpublish`, `canva`, `capital_one_shopping`, `carrefour`, `certarc`, `chavruti`, `circle`, `circleback`, `close`, `cloudflare`, `clutch`, `crosswalk`, `crunchbase`, `cult_beauty`, `deal_ai`, `debenhams`, `deepl`, `deglaze`, `descript`, `despegar`, `dns_doctor`, `dreamrender`, `easyjet`, `eight_sleep`, `elevenlabs`, `engentfy_cadence`, `estimatekit`, `fidexa`, `filament`, `financial_datasets`, `find_a_pro`, `findtime_io`, `flow_state_endurance`, `flux`, `forthclear`, `gamma`, `getmattered`, `gopuff`, `grab_mcp`, `groundquote`, `gumroad_mcp`, `gusto`, `hootsuite`, `hubspot`, `ifttt`, `ilu_rennik`, `inkmotion`, `inkspired`, `instaport`, `intellist`, `jellypod`, `justpark`, `klynx`, `krea`, `kukini`, `lemonpost`, `local_falcon`, `lookfantastic`, `luau`, `luma`, `manufact`, `meltflex`, `mercury`, `moneyforward_me`, `movemate`, `myprotein`, `opendoor`, `opusclip`, `otter`, `pandadoc`, `pantry_persona`, `pantryfy`, `paperoffice_ai`, `partypal`, `pcexpress`, `peristyle_grocery_cart`, `pethubble`, `piki`, `playdrop`, `pool`, `postiz`, `postpone`, `rankify`, `rappi_mcp`, `readwise_mcp`, `rememberthemilk`, `revup`, `runway_mcp`, `ryze_ai`, `secondpage`, `sentisense`, `shelvio`, `shopback_mcp`, `shopsbuilder`, `siftivo`, `solmarrow`, `spreeflo`, `studocu`, `super_ai`, `superhuman`, `swarm_by_foursquare`, `sweetgreen`, `tabico`, `taim`, `tella`, `testbunny`, `the_prompting_company`, `trainingdojo`, `trustandwill`, `typeform`, `vidiq_mcp`, `vritto`, `vurbalize`, `walmart`, `warp`, `wavelength`, `wendy`, `wix`, `workouter`

### 4.2. На `hatch_gws_cli` + `hatch_gws_auth` (Google Workspace) — 9

**Нужно — замена службы: Google Workspace API напрямую (OAuth Google).** `gmail`, `google_calendar`, `google_contacts`, `google_docs`, `google_drive`, `google_forms`, `google_sheets`, `google_slides`, `google_tasks`

### 4.3. На своей программе для одной службы — 63

| Коннектор | Программа | Пометка: нужно — замена службы, на что |
|---|---|---|
| `adobe` | `adobe-connector-cli,remote-storage` | MCP Adobe (адрес в manifest — особый для muse, публичен ли — неясно) |
| `agoda` | `agoda` | партнёрское API Agoda; иначе живой браузер (неясно) |
| `apple_healthkit` | `health-cli` | данные на iPhone — свой клиент на телефоне |
| `asana` | `asana` | Asana REST API |
| `box` | `box-cli` | Box API |
| `calendly` | `calendly` | Calendly API |
| `dropbox` | `dropbox` | Dropbox API |
| `duffel` | `duffel` | Duffel API (публичное) |
| `evernote` | `evernote` | Evernote API |
| `expedia` | `expedia` | партнёрское API Expedia; иначе браузер (неясно) |
| `facebook_cli` | `facebook-cli` | Graph API Facebook (страницы); личное — неясно |
| `figma` | `figma` | Figma REST API |
| `flightaware` | `flightaware` | FlightAware AeroAPI |
| `function_health` | `function-health` | публичного API не знаю — неясно |
| `ghl` | `ghl` | HighLevel API |
| `github` | `github` | GitHub REST/GraphQL API |
| `google_health_connect` | `health-cli` | данные на Android — свой клиент на телефоне |
| `granola` | `granola-cli` | публичное API неясно |
| `healthex` | `healthex` | публичное API неясно |
| `instacart` | `instacart` | Instacart Developer Platform (неясно, хватит ли) |
| `instagram` | `instagram-cli` | Instagram Graph API |
| `instagram_messages` | `instagram-cli,instagram-messages-cli` | личные сообщения — API только для бизнес-аккаунтов; неясно |
| `kavak` | `kavak` | публичного API не знаю — браузер |
| `klaviyo` | `klaviyo` | Klaviyo API |
| `linear` | `linear` | Linear GraphQL API |
| `lovable` | `lovable` | публичное API неясно |
| `messenger_read` | `hatch_messenger_cli` | чтение личного Messenger — публичного API нет; неясно |
| `meta_ads` | `meta-ads-cli` | Meta Marketing API |
| `meta_catalog_search` | `meta-catalog-search,shopping` | внутренний каталог Meta — замены нет; поиск товаров браузером |
| `notion` | `notion-cli` | Notion API |
| `onepassword` | `onepassword` | 1Password CLI / Connect |
| `opentable` | `opentable` | партнёрское API; иначе браузер |
| `outlook_calendar` | `outlook-calendar` | Microsoft Graph |
| `outlook_contacts` | `outlook-contacts` | Microsoft Graph |
| `outlook_mail` | `outlook-mail` | Microsoft Graph |
| `paypal` | `paypal` | PayPal REST API |
| `peloton` | `peloton` | официального API нет — неясно |
| `philips_hue` | `philips-hue` | Hue local API |
| `places` | `places` | Google Places API |
| `plaid` | `plaid` | Plaid API |
| `printify` | `printify` | Printify API |
| `quickbooks` | `quickbooks` | QuickBooks Online API |
| `replit` | `replit` | публичное API неясно |
| `ryanair` | `ryanair` | публичного API нет — браузер |
| `shop-pay` | `shop-wallet-cli` | Shop Pay — неясно |
| `shopify` | `shopify` | Shopify Admin API |
| `slack` | `slack` | Slack Web API |
| `spotify` | `save-to-spotify,spotify-api` | Spotify Web API |
| `stripe` | `stripe` | Stripe API |
| `stripe-link` | `stripe-link` | Stripe Link — неясно (выдача разовой карты) |
| `tailscale` | `tailscale` | Tailscale API |
| `tessie` | `tessie-api` | Tessie API |
| `threads` | `threads-cli` | Threads API |
| `threads_messages` | `threads-cli,threads-messages-cli` | личные сообщения Threads — неясно |
| `ticketmaster` | `ticketmaster` | Ticketmaster Discovery API; покупка — браузер |
| `todoist` | `todoist` | Todoist API |
| `uber_courier` | `uber-courier` | Uber Direct API |
| `ucp` | `ucp-cli` | открытый протокол UCP |
| `vercel` | `vercel` | Vercel API |
| `withings` | `withings` | Withings API |
| `zapier` | `zapier` | Zapier MCP/API |
| `zocdoc` | `zocdoc` | партнёрское API; иначе браузер |
| `zoom` | `zoom` | Zoom API |

### 4.4. На `authdc` (службы muse/Meta) — 3

| Коннектор | Программа | Пометка |
|---|---|---|
| `google-travel` | `google-travel`, `hotels` | нужно — замена службы: Google Flights/Hotels через живой браузер |
| `imap_email` | `imap-email` | нужно — замена службы: свой IMAP/SMTP |
| `muse_mail` | `muse-mail` | нужно — замена службы: свой почтовый ящик агента (IMAP/SMTP) |

### 4.5. Без программы — 3

| Коннектор | Что | Пометка |
|---|---|---|
| `apple_music` | Apple Music на iPhone владельца | нужно — замена службы: Apple MusicKit API / клиент на телефоне |
| `poppy` | Personal Agent Protocol: сессии с компаниями, вход, их API (в manifest — `mcp.path`) | нужно — замена службы: свой клиент протокола; описание протокола — неясно, открыто ли |
| `security_ctf` | секрет для проверки обхода разрешений коннекторов | переносим как есть — тест разрешений (П9) |

Итого коннекторов: общий MCP-клиент 130 + Google Workspace 9 + своя программа 63 + `authdc` 3 + без программы 3 = **208**. Пометки: нужно — замена службы — 207; переносим как есть (`security_ctf`) — 1. Публичная замена неясна или её, насколько известно, нет у 18 из 63 в разделе 4.3: `adobe`, `agoda`, `expedia`, `facebook_cli` (личное), `function_health`, `granola`, `healthex`, `instacart`, `instagram_messages`, `lovable`, `messenger_read`, `meta_catalog_search`, `peloton`, `replit`, `ryanair`, `shop-pay`, `stripe-link`, `threads_messages`. Для каждого из них есть запасной путь — живой браузер (строки плана нет). Публичные API в колонке «на что» указаны по общим знаниям и в снимке не проверялись.

---

## 5. Программы `opt/hatch/bin/` — 249 файлов (только имена и размеры из `rootfs-full.listing.txt`)

Назначение — догадка по имени, размеру, жёстким ссылкам и упоминаниям в навыках и коннекторах. Программы не запускались.

### 5.1. Общий MCP-клиент коннекторов — 131 файл

`adobe-connector-cli` (96,5 МБ), его копия того же размера `dreamrender-connector-cli` и 129 жёстких ссылок на него. Догадка: одна программа, которая по `manifest.yaml` коннектора ходит в официальный MCP-сервер службы с OAuth (у всех 130 коннекторов этой группы в `manifest.yaml` есть `mcp.endpoint` или `mcp.path`). **Нужно своё:** один MCP-клиент с OAuth в движке Hamlet.

Ссылки: `agentman-skills-connector-cli`, `agenttask-connector-cli`, `ahrefs-connector-cli`, `ai-manga-studio-connector-cli`, `airtable`, `app-00widget`, `argos`, `arketa-connector-cli`, `aura-historia-connector-cli`, `basecamp`, `beehiiv-connector-cli`, `bulkpublish`, `canva`, `capital-one-shopping-connector-cli`, `carrefour`, `certarc-connector-cli`, `chavruti-connector-cli`, `circle-connector-cli`, `circleback-connector-cli`, `close-connector-cli`, `cloudflare`, `clutch-connector-cli`, `crosswalk-connector-cli`, `crunchbase-connector-cli`, `cult-beauty`, `deal-ai-connector-cli`, `debenhams`, `deepl-connector-cli`, `deglaze-connector-cli`, `descript-connector-cli`, `despegar`, `dns-doctor-connector-cli`, `easyjet`, `eight-sleep`, `elevenlabs-connector-cli`, `engentfy-cadence-connector-cli`, `estimatekit-connector-cli`, `fidexa-connector-cli`, `filament-connector-cli`, `financial-datasets-connector-cli`, `find-a-pro-connector-cli`, `findtime-io-connector-cli`, `flow-state-endurance-connector-cli`, `flux`, `forthclear-connector-cli`, `gamma-connector-cli`, `getmattered`, `gopuff-connector-cli`, `grab-mcp`, `groundquote-connector-cli`, `gumroad-mcp`, `gusto-connector-cli`, `hootsuite-connector-cli`, `hubspot-connector-cli`, `ifttt-connector-cli`, `ilu-rennik-connector-cli`, `inkmotion`, `inkspired-connector-cli`, `instaport-connector-cli`, `intellist-connector-cli`, `jellypod-connector-cli`, `justpark`, `klynx-connector-cli`, `krea-connector-cli`, `kukini`, `lemonpost-connector-cli`, `local-falcon-connector-cli`, `lookfantastic`, `luau-connector-cli`, `luma-connector-cli`, `manufact-connector-cli`, `meltflex-connector-cli`, `mercury`, `moneyforward-me`, `movemate-connector-cli`, `myprotein`, `opendoor-connector-cli`, `opusclip-connector-cli`, `otter-connector-cli`, `pandadoc-connector-cli`, `pantry-persona-connector-cli`, `pantryfy-connector-cli`, `paperoffice-ai`, `partypal-connector-cli`, `pcexpress`, `peristyle-grocery-cart-connector-cli`, `pethubble-connector-cli`, `piki-connector-cli`, `playdrop-connector-cli`, `pool`, `postiz-connector-cli`, `postpone-connector-cli`, `rankify-connector-cli`, `rappi-mcp`, `readwise-mcp`, `rememberthemilk-connector-cli`, `revup-connector-cli`, `runway-mcp`, `ryze-ai-connector-cli`, `secondpage-connector-cli`, `sentisense-connector-cli`, `shelvio-connector-cli`, `shopback-mcp`, `shopsbuilder-connector-cli`, `siftivo-connector-cli`, `solmarrow-connector-cli`, `spreeflo-connector-cli`, `studocu-connector-cli`, `super-ai-connector-cli`, `superhuman`, `swarm-by-foursquare-connector-cli`, `sweetgreen`, `tabico-connector-cli`, `taim-connector-cli`, `tella-connector-cli`, `testbunny-connector-cli`, `the-prompting-company-connector-cli`, `trainingdojo-connector-cli`, `trustandwill-connector-cli`, `typeform-connector-cli`, `vidiq-mcp`, `vritto-connector-cli`, `vurbalize-connector-cli`, `walmart`, `warp-connector-cli`, `wavelength-connector-cli`, `wendy-connector-cli`, `wix-connector-cli`, `workouter-connector-cli`

### 5.2. `authdc` — клиент служб muse/Meta — 23 файла

`authdc` (61,5 МБ) и 22 жёсткие ссылки; по имени ссылки программа выбирает службу. **Нужно своё по каждой** (кроме помеченных):

| Имя | Назначение и замена |
|---|---|
| `authdc` | общая программа — своей не нужно, нужны замены служб ниже |
| `avatar-media-generation` | медиа аватара — только приложение |
| `browser-service` | служба браузера — П8 |
| `builtin-artifacts` | встроенные артефакты (догадка) — свой хостинг |
| `device-data` | кэш контактов/календаря телефона — CardDAV/CalDAV |
| `edits` | правка медиа (догадка) — публичное API |
| `feature-request` | отзыв команде Muse — только продукт muse |
| `flights` | поиск перелётов — Duffel API / браузер Google Flights |
| `geocode` | геокодирование — публичное API (Google/Nominatim) |
| `google-travel` | Google Flights/Hotels — браузер |
| `hatch-multicall` | общий вход (догадка) — не нужно |
| `hotels` | поиск отелей — браузер / партнёрские API |
| `image-search` | поиск картинок — публичный поиск (Firecrawl/Brave) |
| `imap-email` | чужая почта по IMAP — своё (IMAP/SMTP) |
| `media-generation` | генерация медиа — публичное API |
| `media-library` | фотоархив — папка на сервере / Google Photos API |
| `muse-mail` | почта агента — свой IMAP/SMTP ящик |
| `privsep-test-fully-isolated` | тест изоляции — не нужно (П8 своё) |
| `share` | ссылка «поделиться» (догадка) — свой хостинг |
| `shopping` | поиск товаров — Firecrawl/браузер |
| `subscription-status` | подписка Muse — только продукт muse |
| `tts` | синтез речи — публичное TTS API или локально |
| `web-search` | поиск в сети — сделано своё в П4г (Firecrawl) |

### 5.3. Программы на одну службу — 64 файлов

Каждая — клиент API одной службы. **Нужно своё**: клиент публичного API службы (подробно по службам — раздел 4.3).

`agoda`, `asana`, `box-cli`, `calendly`, `dropbox`, `duffel`, `evernote`, `facebook-cli`, `figma`, `flightaware`, `function-health`, `ghl`, `github`, `granola-cli`, `hatch_gws_auth`, `hatch_gws_cli`, `hatch_messenger_cli`, `health-cli`, `healthex`, `instacart`, `instagram-cli`, `instagram-messages-cli`, `kavak`, `klaviyo`, `linear`, `lovable`, `meta-ads-cli`, `meta-catalog-search`, `notion-cli`, `onepassword`, `opentable`, `outlook-calendar`, `outlook-contacts`, `outlook-mail`, `paypal`, `peloton`, `philips-hue`, `places`, `plaid`, `printify`, `quickbooks`, `replit`, `ryanair`, `save-to-spotify`, `shop-wallet-cli`, `shopify`, `slack`, `spotify-api`, `stripe`, `stripe-link`, `tailscale`, `tessie-api`, `threads-cli`, `threads-messages-cli`, `ticketmaster`, `todoist`, `uber-courier`, `ucp-cli`, `vercel`, `withings`, `zapier`, `zocdoc`, `zoom`, `expedia`

### 5.4. Инфраструктура и прочее — 29 файлов

| Имя | Догадка по назначению | Нужно ли Hamlet |
|---|---|---|
| `aftership` | отслеживание посылок (коннектора нет) | нужно своё — AfterShip API |
| `browser-broker` | раздача окон браузера | нужно своё — П8 (Chromium, живой браузер) |
| `etsy-cli` | Etsy (коннектора в снимке нет) | нужно своё — Etsy Open API |
| `genui-display-render` | отрисовка интерфейса очков | без замены: устройство Meta (решение владельца) |
| `hatch-browser-lease-helper` | аренда окна браузера (догадка) | нужно своё — П8 |
| `hatch-chromium-sandbox` | песочница Chromium | нужно своё — П8 |
| `hatch-connector-output` | обработка вывода коннекторов (догадка: сжатие/в файл) | нужно своё — вместе с коннекторами |
| `hatch-doctor` | проверка здоровья машины | нужно своё — П8 |
| `hatch-execd` | служба исполнения команд | сделано по форме — П4б (своё в движке) |
| `hatch-healthd` | служба здоровья (догадка) | нужно своё — П8 |
| `hatch-rescue` | агент-спасатель: замена машины | нужно своё — П8 (systemd/перезапуск) |
| `hatch-rescue-systemctl` | systemctl для спасателя | нужно своё — П8 |
| `hatch-rescuectl` | перезапуск частей спасателем | нужно своё — П8 |
| `hatch-slide-style` | стиль слайдов для документов | нужно своё — навык documents |
| `hatch-vault` | хранилище секретов | нужно своё — хранилище секретов (нет строки) |
| `hatch-ws-client` | клиент websocket к службам (догадка) | не нужно (провод к службам Meta) |
| `hatch-zeitgeist` | тренды/обсуждаемое (упомянут в instagram content-questions) | нужно своё — поиск трендов через сеть (неясно) |
| `hatch_igd_cli` | Instagram (догадка: direct/IG data) | нужно своё — Instagram Graph API (неясно) |
| `healthkit-cli` | данные Apple Health с телефона | нужно своё — свой клиент на телефоне |
| `ingress-rev-proxy` | обратный прокси входа на машину | нужно своё — П8 (вход к агенту) |
| `local-search` | поиск мест рядом | нужно своё — Google Places API |
| `native-comms-cli` | звонки/SMS с очков (навык wearables-comms) | без замены: устройство Meta |
| `nutrition-cli` | питание (догадка) | неясно |
| `pickup-briefing-animation` | анимация брифинга (догадка: для приложения/очков) | не нужно: только приложение |
| `product-lists` | сохранённые списки товаров | нужно своё — список в файлах |
| `remote-storage` | хранилище файлов со ссылкой | нужно своё — S3/R2 или сервер П8 |
| `security-ctf` | тест обхода разрешений | нужно своё — тест разрешений коннекторов (П9) |
| `spawnd` | служба порождения процессов/клеток (догадка) | нужно своё — П8 (клетка исполнения) |
| `wearables-display` | экран очков | без замены: устройство Meta (решение владельца) |

### 5.5. `zbar-lib/` — 2 файла

`zbar-lib/COPYING`, `zbar-lib/libzbar.so.0` — открытая библиотека чтения штрихкодов и QR (LGPL). Нужна, если Hamlet читает QR с картинок: берётся открытая `zbar`, не закрытая программа.

Итого программ: общий MCP-клиент 131 + `authdc` 23 + программы на одну службу 64 + инфраструктура 29 + `zbar-lib` 2 = **249**.

Нужно Hamlet (писать своё):
- один MCP-клиент с OAuth — заменяет 131 файл;
- замены служб `authdc`: поиск перелётов и отелей, TTS, поиск картинок, фотоархив, генерация медиа, почта агента, IMAP, поиск товаров, геокодирование, кэш устройств (поиск в сети уже сделан в П4г);
- клиенты публичных API для 64 программ на одну службу;
- из инфраструктуры: браузер (`browser-broker`, `hatch-browser-lease-helper`, `hatch-chromium-sandbox`, `browser-service`), здоровье и спасение машины (`hatch-doctor`, `hatch-healthd`, `hatch-rescue*`), вход (`ingress-rev-proxy`), клетка исполнения (`spawnd`) — всё это П8, но в строке П8 названы только «установка движка, область файлов, исполнение команд, браузер, связь с моделью»; хранилище секретов (`hatch-vault`); стиль слайдов, списки товаров, хранилище файлов, поиск мест.

Не нужно: `hatch-ws-client`, `hatch-multicall`, `privsep-test-fully-isolated`, `authdc` как программа. Только приложение или продукт muse: `pickup-briefing-animation`, `avatar-media-generation`, `feature-request`, `subscription-status`. Устройства Meta без публичной замены (решает владелец): `genui-display-render`, `wearables-display`, `native-comms-cli`. Неясно: `nutrition-cli`, `hatch-zeitgeist`, `hatch_igd_cli`, `hatch-connector-output`.

---

## 6. Справочник `home/hatch/docs/` — 36 файлов

В описи (`muse_inventory.md:53`) указано «21 файл» — это файлы верхнего уровня. В HEAD всего 36 файлов вместе с подпапками `chat/`, `chat-connections/`, `devices/`.

| Файл | Строк | О чём | Пометка |
|---|---|---|---|
| `artifacts.md` | 61 | что агент строит для владельца | **нужно — замена службы** — свой хостинг артефактов |
| `browser.md` | 125 | `browser.browse`, окна, поиск, расширение Chrome | **в плане (П5)**; живой браузер — строки нет; расширение Chrome — неясно |
| `calls-texts-notifications.md` | 176 | звонки компаниям, SMS, уведомления | **нужно — замена службы** — Twilio; push заменяется сообщением в мессенджер |
| `chat-connections.md` | 40 | мессенджер как побочный чат | **в плане (П6/П5)** |
| `chat-connections/whatsapp.md` | 76 | WhatsApp | **в плане (П6)** |
| `chat/browser-purchase-preapproval.md` | 222 | `permissions.preapprove`: согласие на оплату | **нужно — замена службы** — свой путь оплаты |
| `chat/payments-and-purchases.md` | 178 | маршрут покупки | **нужно — замена службы** — свой путь оплаты |
| `chat/scheduling-and-watching.md` | 60 | фоновая работа из чата | **в плане (П5)** |
| `client-surfaces.md` | 615 | приложения iOS/Android/web/Mac | **только приложение** |
| `connectors.md` | 181 | как работать с коннекторами | **в плане (П5)**; среда коннекторов — нет строки |
| `data-handling.md` | 53 | обращение с данными | **в плане (П5)** |
| `devices/charm.md` | 27 | Muse Charm (устройство Meta) | **нужно — замена службы** — публичной замены нет, решает владелец |
| `devices/gadget.md` | 29 | самодельные устройства на открытом Muse Gadget SDK | **нужно — замена службы** — свой протокол устройства (неясно) |
| `devices/glasses.md` | 58 | очки Meta | **нужно — замена службы** — публичной замены нет, решает владелец |
| `devices/home_link.md` | 269 | Home Link — мост Meta к домашней сети | **нужно — замена службы** — локальные API с сервера / Tailscale |
| `devices/home_link/integrations/brother_printers.md` | 17 | принтеры Brother | **нужно — замена службы** — то же |
| `devices/home_link/integrations/home_assistant.md` | 238 | Home Assistant | **нужно — замена службы** — Home Assistant API напрямую |
| `devices/home_link/integrations/lutron_smart_bridges.md` | 32 | Lutron | **нужно — замена службы** — локальное API |
| `devices/home_link/integrations/shelly_plugs.md` | 103 | розетки Shelly | **нужно — замена службы** — локальное API |
| `devices/mac_app.md` | 112 | Mac владельца как устройство | **нужно — замена службы** — свой клиент на Mac |
| `devices/quest.md` | 42 | Meta Quest | **нужно — замена службы** — публичной замены нет, решает владелец |
| `devices/tailscale.md` | 118 | Tailscale до устройств владельца | **в плане (П8)** |
| `feed.md` | 103 | вкладка Feed | **только приложение** |
| `files-and-library.md` | 58 | свой компьютер и файлы | **в плане (П5)** |
| `goals.md` | 47 | цели | **в плане (П5, П4ж)** |
| `ideas.md` | 28 | вкладка Ideas | **только приложение** |
| `media.md` | 39 | генерация картинок и видео | **нужно — замена службы** — публичное API |
| `memory.md` | 42 | память | **в плане (П5)** |
| `muse.md` | 223 | кто такой агент | **в плане (П5, переписать под Hamlet — П10)** |
| `payments-and-purchases.md` | 37 | лимиты кошелька, Link, возвраты | **нужно — замена службы** — свой путь оплаты |
| `privacy-and-credentials.md` | 180 | входы и секреты | **в плане (П5)**; хранилище секретов — нет строки |
| `referrals.md` | 66 | приглашения в Muse | **только приложение** |
| `scheduling-and-watching.md` | 84 | расписания и наблюдение | **в плане (П5)** |
| `self_improvement.md` | 59 | фоновое самоулучшение | **в плане (П5)**; сами циклы — нет строки |
| `spaces.md` | 54 | Spaces — сайты агента | **нужно — замена службы** — свой хостинг |
| `voice.md` | 29 | голосовой разговор | **нужно — замена службы** — TTS и распознавание речи |

Итого: в плане 14, нужно — замена службы 18, только приложение 4 = **36**.

---

## 7. Сверка сумм

| Раздел | Всего | По пометкам |
|---|---|---|
| Тексты `prompts/` | 1 561 | перенесено 22 · в плане 222 · нет в плане — нужно 549 · нужно — замена службы 467 · только приложение 298 · служебное 2 · сознательно не взято 1 → 1 561 |
| Блоки `user_chat` (ядро) | 97 имён (88 + 9 вариантов) | в плане (П5) 97; из них 12 требуют замены службы |
| Конфиги | 69 | перенесено 3 · нет в плане — нужно 32 · нужно — замена службы 32 · служебное 2 → 69 |
| Навыки `workflow-skills` | 58 (437 файлов) | П5 26 · П5 + замена службы 25 · нет в плане — нужно 2 · только приложение 5 → 58 |
| `skills/` | 149 записей | ссылки 140 (в плане, П5/П9) · папки 9: П5 1 (`booking`), П5/П8 1 (`home-assistant`), П5 + замена 2 (`audio`, `instagram`), замена службы 3 (`artifacts`, `spaces`, `voice-selector`), только приложение 2 (`connections`, `proactivity`) → 149 |
| Коннекторы | 208 (713 файлов) | MCP 130 · Google Workspace 9 · своя программа 63 · `authdc` 3 · без программы 3 → 208; замена службы 207, как есть 1 |
| Программы `bin/` | 249 | MCP-клиент 131 · `authdc` 23 · программы на одну службу 64 · инфраструктура 29 · `zbar` 2 → 249 |
| Справочник `docs/` | 36 | в плане 14 · замена службы 18 · только приложение 4 → 36 |

---

## 8. «Только приложение» — список для решения владельца

Тексты (298 файлов):
- лента Feed — `hatch-feed` (87), `tools/feed_*` (2) — **спорно**: выпуски можно присылать в мессенджер;
- Ideas — `hatch-self-improvement/ideas` (40) + `inline/objectives/ideas.json` (1), `blocks/ideas_builder` (14), `tools/ideas_*` (2), `hatch-server/self_improvement` (4);
- лента дел для экрана — `blocks/activity_monitor` (39), `hatch-activity` (13), `hatch-agent/activity_monitor` (4), `hatch-agent/src/subscriptions` (3);
- аватар — `hatch-agent/src/avatar` (41), `hatch-skills/media-generation` (3);
- туры и сообщения продукта — `hatch-server/product_messages` (25), `hatch-agent/product_messages` (2), `hatch-cron/product_messages` (1);
- виджеты и карточки — `hatch-cron/tools/cron/confirmation_*` (4), `tools/create_shopping_*` (2), `tools/create_options_off_screen` (1), `tools/widget_creation` (1), `hatch-agent/proactivity` (3), `session/share_card` (1);
- прочее — `session/title*` (2, заголовок разговора для списка чатов), `hatch-agent/completion_gate` (2, про Ideas и Spaces), `hatch-goals/briefing-designs` (1).

Навыки: `avatar-dance-videos`, `share-ideas`, `muse-early-access`, `muse-feedback`, `subscription-status`; папки `skills/connections`, `skills/proactivity` (2 102 файла карточек).
Справочник: `client-surfaces.md`, `feed.md`, `ideas.md`, `referrals.md`.
Программы: `pickup-briefing-animation`, `avatar-media-generation`, `feature-request`, `subscription-status`.
Отдельно, тоже на решение владельца: устройства Meta без публичной замены — очки, Charm, Quest (`devices/*.md`, `hatch-skills/wearables` (9), `wearables-comms`, `genui-display-render`, `wearables-display`, `native-comms-cli`).

---

## 9. Нет в плане — нужно

У muse это есть, Hamlet это нужно, но ни одна строка `docs/PLAN.md` этого не покрывает. В скобках — основание в muse-live.

1. **Фоновые циклы самоулучшения**: уборка памяти раз в час, улучшение навыков раз в день, люди и отношения, изучение (в том числе изучение целей), ночная рефлексия и исправление разрыва, учёт целей (`goals_bookkeeping`), профили поездок и покупок, «линзы» проверки, рамка циклов (`P/hatch-self-improvement/**` — 93 файла без Ideas, `hatch-alignment` 2, `hatch-objective-execution` 1, `hatch-goals/studying` 3, `blocks/self_improvement` 3, `hatch-agent/inline` 9, `hatch-server/user_sentiment`). В П4ж изучение отложено «до среза самоулучшения», но такой строки в плане нет.
2. **Глубокое исследование** — отдельный агент-исследователь (`blocks/deep_research` 9, `research/resumed_after_restart`, `research_delegate/*` в разборе ядра).
3. **Живой браузер** — `browser.browse/windows`, браузерный исполнитель (`A/browser/**` 163, `hatch-server/browser_*` 31, `session/browser_*` 3, `tools/browser_*` 2, `file_scan_policy.json`, справочник `browser.md`). Есть только в карточке П4, в разделе «Дальше — после слова владельца»; в таблице плана строки нет (Chromium — П8). На нём же держится запасной путь для коннекторов без публичного API.
4. **Проактивность** — личная копия агента, которая работает без просьбы (`A/private_copy` 18, навык `proactive-work`), работники контекста — срочность, слежение за рейсами, уведомления, исследование (`hatch-context-workers` 60, `blocks/context_worker` 3, `blocks/proactivity`, `session/proactivity_*`, `tools/proactivity` 7).
5. **Безопасность на отдельных моделях** — проверка внедрённых команд (`prompt_injection_checker` 10 конфигов, `src/safety_classifiers` 12 текстов, `owner_mail_pi_*`, `tools/prompt_injection_browser_*`), проверка вызовов кибер/CBRNE/утечка/TBAC (`hatch-safety` 20 текстов + 7 конфигов, `safety_classifiers/policies` 5), пределы и эскалация (`safety_throttle.json`, `safety_escalation.json`, `policy_prompt.json`, `activity_config.json`), выход в сеть (`hatch-sentinel` 5 текстов + 2 политики).
6. **Проверка исхода и завершения** — `hatch-server/verification` 10, `tools/verification_not_relevant`, механизм `completion_gate` (`source-files.txt:1311-1313,1530-1533`).
7. **Сохранённые работы `workflow.*`** (`workflow_tool*`, `hatch-server/workflows/agent_structured_output.md`).
8. **`hamlet.db`** — SQL по своей базе и навык `muse_db` с описанием схемы (199 файлов — под свою схему).
9. **Подагенты глубже первого уровня** и наблюдатель за подагентами — у Hamlet `MAX_SPAWN_DEPTH = 1`, а muse умеет глубже. Не перенесены тексты `subagent_context_depth_one_coordinator`, `subagent_spawning`, `hatch-agent/subagent_monitor` (7), `tools/delegate*` (4).
10. **Недобранные тексты уже готовых срезов** — сжатие по частям `hatch-compaction/segmented_*` (3), `hatch-agent/compact/*` (4: упор в предел ответа, сжатие без инструментов); системные тексты работников `blocks/memory_flush` (3), `blocks/scheduler_resolution` (6), `blocks/scheduler_worker` (2), `blocks/event_hook/filesystem.md`; `tools/notify_main_agent_routing/message`, `tools/nothing_to_do_without_reaction_routing`, `tools/memory_reconcile_queued`, `hatch-server/connection_hook_execute`, `session/skills_blocked_escalation_*`, `background_worker_paged_input`, `worker_input_page`. Неясно, войдут ли они в П5 или их надо добрать в П3в и П4е.
11. **Описание картинок отдельной моделью** — `content-understanding/image_describe.md`, `hatch-media/media_description.md`. В П2 есть назначение `MediaDescription`, текстов нет.
12. **Поиск инструментов на стороне Anthropic** — `blocks/anthropic/cedar` (2): при флаге `anthropic.tool_search`. Сам `tool_search.load_tool_namespace` записан долгом в строке П4, своей строки нет.
13. **Групповые чаты** — как умение агента: `blocks/group_chat` 5, `blocks/group_muse`, `hatch-server/group_chat` 17, `group_muse_peer_task`, `tools/group_*` 8, `tools/group_discretion` 3, `session/group_discretion_*` 2; групповая часть `blocks/side_chat`. В П6 названы только «мессенджер или терминал».
14. **Связь агент — агент и с другими людьми** (замена службы: свой протокол вместо сети агентов Meta): `blocks/peer_chat` 28, `peer_sync` 3, `session/peer_*` 16, `connection_*` 6, `relationship_*` 3, `mailbox_send_*` 12, `tools/mailbox` 24, `peer_gated` 4, `hatch-server/coordination_*` 2, `hatch-agent/peer_*` 2, `tools/relationships_*` 3.
15. **Разговор с компаниями от имени владельца** (замена: почта или чат компании напрямую): `blocks/company_conversation`, `tools/company_conversation` 13, `hatch-server/company_conversation` 18; коннектор `poppy` (Personal Agent Protocol).
16. **Почта агента** (замена: свой ящик IMAP/SMTP): `hatch-server/muse_mail` 5, `tools/mail_*` 7, `blocks/background/mail_conversation`, коннекторы `muse_mail`, `imap_email`.
17. **Голос** (замена: TTS и распознавание речи через публичное API или локально): `hatch-agent/voice-prompts` 98, `hatch-inference/voice` 8, `hatch-server/voice` 10, `set_requested_voice_routing`, навыки `spoken-audio`, `generate_podcast`, `transcribe-recordings`, `skills/voice-selector`, `skills/audio`, справочник `voice.md`.
18. **Телефон** (замена: Twilio + голос): `hatch-server/src` (звонки) 6, `tools/phone_destination_verification`, `phone.begin_call` в `scheduling`, справочник `calls-texts-notifications.md`.
19. **Генерация и правка медиа** (замена: публичные API картинок и видео) с модерацией результата: `hatch-media-runner` 7, `tools/generate_media_*` 3, `media_style_*` 2, `mango-safety*` 32 конфига, навыки `smb-product-shots`, `style-tryon`, `social-publishing`, справочник `media.md`.
20. **Артефакты и сайты (Spaces), документы** (замена: свой хостинг на сервере П8 / Vercel, Cloudflare): `A/artifacts` 37, `blocks/artifact_builder`/`artifacts`/`web_artifacts_inspector`/`spaces_builder`/`spaces_builder_lite`/`space_task` — 67, `tools/artifact_*` 6, `sharing_settings`, `spaces_builder` 3, `hatch-tools` 4, `hatch-spaces`, `hatch-server/space_*` 4, `peer_space*` 6, `session/artifact_routing`, `skills/artifacts` 48, `skills/spaces` 8 914, справочники `artifacts.md`, `spaces.md`. CLAUDE.md прямо требует «пишет и правит код и сайты».
21. **Среда коннекторов** — свой MCP-клиент с OAuth (130 коннекторов), Google Workspace API (9), клиенты публичных API (63), хранение токенов. В П9 есть только «коннектор описанием»; П5 говорит о навыках, не о коннекторах; программ для коннекторов нет ни в одной строке.
22. **Хранилище секретов и входов** — `hatch-vault`, `credentials.*`, блоки `secure_vault*` (5) в `user_chat`, навык `custom-connector-setup`, справочник `privacy-and-credentials.md`.
23. **Покупки и оплата** (замена: свой путь оплаты — Stripe Link или карта под согласие; UCP — открытый протокол): `tools/ucp_*` 10, `poppy_*` 4, `blocks/browser/checkout_preapproval`, `hatch-server/purchase_approval_intent` 2, `user_chat/purchases`, справочники `payments-and-purchases.md` (2 файла) и `browser-purchase-preapproval.md`, коннекторы `stripe-link`, `shop-pay`, `paypal`, `ucp`.
24. **Устройства владельца** (замена: свой клиент на телефоне и Mac; CardDAV/CalDAV; Home Assistant и локальные API вместо Home Link): `blocks/device_sync` 4, `sync_worker` 2, `media_upload` 2, `session/device_sync_*`, `hatch-server/device_sync`, `tools/device*` 5, навыки `device-data`, `media-library`, `home-control`, коннекторы `apple_healthkit`, `google_health_connect`, `apple_music`, справочники `devices/home_link*` (5), `mac_app.md`, `gadget.md`.
25. **Места, перелёты, отели, поиск картинок, товары** (замена служб `authdc` и CLI): `places`/`local-search`/`geocode` → Google Places API; `flights`/`hotels`/`google-travel` → Duffel и браузер; `image-search` → публичный поиск; `shopping`/`meta-catalog-search` → Firecrawl и браузер; `product-lists` → список в файлах; `tools/local_search_*`, `places_*`.
26. **Хранилище файлов со ссылкой** (`remote-storage` → S3/R2 или сервер П8) и **стиль слайдов** (`hatch-slide-style`) — для навыков `file-hosting`, `documents`, `generate_podcast`.
27. **Push-уведомления** → сообщение в мессенджер (`hatch-agent/push_notification` 2). В П6 не названо.
28. **Здоровье и спасение машины, вход, клетка, выход в сеть** — `RESCUE_AGENT.md`, `hatch-doctor`, `hatch-healthd`, `hatch-rescue*`, `ingress-rev-proxy`, `spawnd`, `hatch-sentinel`. Это место П8, но в строке П8 их нет.
29. **Остаток чёрного списка** — `mase_all_urls_fingerprints.bin`, `mase_reduced_domains.fst`, `mase_contract_domains.fst` (в П4г взяты только домены и regex).
30. **Определение языка подсказки для журнала** (`hatch-inference-proxy` 2) — мелочь. **Синхронизация контекста файлового сервера** (`hatch-server/internal`) — рядом с П8.

Неясное, записано как есть: роль `reduced` и `contract` списков доменов; назначение `nutrition-cli`, `hatch-zeitgeist`, `hatch_igd_cli`, `hatch-connector-output`, `share`, `edits`, `builtin-artifacts`; публичные API у 18 коннекторов (раздел 4); открыт ли Personal Agent Protocol (`poppy`); тексты `user_chat/conversation.md` и `secure_vault.md` (в ядре только имена).
