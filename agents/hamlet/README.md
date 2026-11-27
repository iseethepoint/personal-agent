# agents/hamlet — пример папки агента

- `agent.toml` — модель, пути, имя переменной с адресом базы, браузер.
- `system.md` — дополнения владельца к системному тексту главного агента: идут после текста, собранного из блоков muse (`agent/hamlet/src/main_prompt.rs`); пусто — ничего не добавляется. Остальные роли пока берут его как свой системный текст (П5-2).
- `home/` — домашняя папка агента: `SOUL.md`, `USER.md`, `MEMORY.md`, `HEARTBEAT.md`…

Запуск: `HAMLET_DATABASE_URL=postgres://… ANTHROPIC_API_KEY=… cargo run -p agent-hamlet -- --agent agents/hamlet`.
Строка в stdin — сообщение владельца, ответ — в stdout.
