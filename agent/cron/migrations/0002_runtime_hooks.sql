-- Допуск наблюдателей — runtime.event_hook_admissions по запросам ядра muse
-- (sql.txt:908, :1110). Не взята колонка originating_context_json
-- (наблюдатели разговоров с другими людьми — со связями, П6).

create schema if not exists runtime;

create table if not exists runtime.event_hook_admissions (
    hook_id text primary key,
    revision bigint not null,
    definition_sha256 text not null,
    script_sha256 text not null,
    enabled boolean not null
);
