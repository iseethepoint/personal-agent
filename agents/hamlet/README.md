# agents/hamlet — пример папки агента

- `agent.toml` — модель, пути, имя переменной с адресом базы, браузер.
- `system.md` — системный текст главного агента (тексты muse придут с П5; пока пусто).
- `home/` — домашняя папка агента: `SOUL.md`, `USER.md`, `MEMORY.md`, `HEARTBEAT.md`…

Запуск: `HAMLET_DATABASE_URL=postgres://… ANTHROPIC_API_KEY=… cargo run -p agent-hamlet -- --agent agents/hamlet`.
Строка в stdin — сообщение владельца, ответ — в stdout.
