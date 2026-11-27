-- Список дел и сторож продолжения — runtime.agent_todo_snapshots по схеме
-- базы muse (каталог ядра около 0x18a9a6a; запросы около 0x16f9853).

create table if not exists runtime.agent_todo_snapshots (
    conversation_id text not null,
    agent_identity text not null,
    items_json text not null default '[]',
    revision bigint not null default 1,
    updated_at_ms bigint not null default ((extract(epoch from now()) * (1000)::numeric))::bigint,
    continuation_key text,
    continuation_generation bigint not null default 0,
    continuation_reminders smallint not null default 0,
    primary key (conversation_id, agent_identity)
);
