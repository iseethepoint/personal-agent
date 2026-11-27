-- Точка перезапуска — agent.runtime_restart_checkpoints по схеме базы muse
-- (каталог ядра, `runtime_restart_checkpoints_pkey` по agent_id): ход
-- главного агента пишет `before_inference` с payload_json, между ходами —
-- `idle` без payload_json (запросы ядра около `0x1711918`, `0x1729b52`).

create table if not exists agent.runtime_restart_checkpoints (
    agent_id text primary key,
    mode text not null,
    payload_json text,
    created_at bigint not null,
    updated_at bigint not null,
    recovery_class text,
    recovery_owner_id text,
    execution_config_json text
);
