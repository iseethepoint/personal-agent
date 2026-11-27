-- Прогоны наблюдателей — runtime.execute_resolve_runs по схеме базы muse
-- (каталог ядра; `execute_resolve_runs_source_unique` по (worker_kind,
-- source_ref)): срабатывание наблюдателя — прогон `event_hook`, работник
-- берёт его новым поколением (worker_generation), итог — terminal_decision,
-- notify_pending → completed_notified, completed_noop или failed.

create schema if not exists runtime;

create table if not exists runtime.execute_resolve_runs (
    run_id text primary key,
    worker_kind text not null,
    source_ref text not null,
    lane_key text not null,
    status text not null,
    terminal_decision text,
    terminal_message text,
    handoff_message_id text,
    last_error_stage text,
    last_error_message text,
    metadata_json jsonb not null default '{}'::jsonb,
    created_at timestamp with time zone not null default now(),
    updated_at timestamp with time zone not null default now(),
    worker_generation bigint not null default 0,
    worker_phase text,
    worker_agent_id text,
    worker_started_at timestamp with time zone,
    execute_message_id text,
    resolve_message_id text,
    constraint execute_resolve_runs_source_unique unique (worker_kind, source_ref)
);
