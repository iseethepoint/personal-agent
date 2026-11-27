# personal-agent

Движок личного агента на Rust: цикл хода, прослойка модели (Claude, OpenAI, локальные серверы «как OpenAI»), инструменты, память на Postgres. Правила работы — `CLAUDE.md`, план — `docs/PLAN.md`.

## Сборка и тесты

1. Rust: <https://rustup.rs>, затем `rustup default stable`.
2. Большие файлы (чёрный список адресов `config/blocklist/*.fst`) хранятся в Git LFS. После `git clone`:

   ```sh
   git lfs install --local && git lfs pull
   ```

   Без этого на месте файла остаётся короткая ссылка LFS, и тесты сети падают.
3. Postgres 18 с pgvector — для тестов памяти, целей и расписаний (`agent/memory`, `agent/goals`, `agent/cron`). На Mac:

   ```sh
   brew install postgresql@18 pgvector
   ```

   Службу запускать не нужно: тесты сами поднимают временный кластер (только `127.0.0.1`, свободный порт, без unix-сокета) и удаляют его после прогона. Другой путь к бинарникам Postgres задаётся переменной `AGENT_PG_BIN`. pgvector из Homebrew собран для Postgres 18 и 19, для 17 его нет.
4. Тесты:

   ```sh
   cargo test --workspace
   ```

   Тесты не ходят в сеть: модели, Firecrawl и модель векторов подменены записанными ответами на `127.0.0.1`.

## Ключи

Ключи только в переменных окружения, в репозиторий не идут. В `config/models.toml` и `config/web.toml` записаны лишь имена переменных.
