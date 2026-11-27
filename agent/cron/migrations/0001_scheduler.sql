-- Расписания — таблицы по схеме базы muse (strings-all.txt, JSON-схема
-- 201212-222912: scheduler.jobs, job_definitions, job_runs, cron_mutations,
-- terminal_signals, delivery_outbox). Колонки, типы и умолчания — как там;
-- берутся только колонки, которые служба заполняет. Не взяты (их службы у
-- Hamlet нет): у jobs — blocked_*, device_*, public_ref; у job_runs —
-- worker_boot_generation и прочие поля восстановления после рестарта
-- службы, device_*, connection_origin_json, onboarding_tour_body,
-- presentation_locale, product_endpoint_deadline_at_ms; у
-- delivery_outbox — dispatch_boot_generation.
-- Перечисление run_status — значения из запросов ядра
-- (strings-all.txt:223260-223900: queued, running, succeeded, failed, cancelled).

create schema if not exists scheduler;

do $$ begin
    create type scheduler.run_status as enum ('queued', 'running', 'succeeded', 'failed', 'cancelled');
exception when duplicate_object then null;
end $$;

create table if not exists scheduler.jobs (
    job_id text primary key,
    is_heartbeat boolean not null default false,
    source_path text,
    source_hash text,
    schedule_kind text not null,
    enabled boolean not null default true,
    schedule_expr text not null,
    timezone text not null default 'UTC',
    retry_on_failure boolean not null default false,
    max_retries integer not null default 0,
    delivery_targets_json text not null default '[]',
    next_run_at timestamp with time zone,
    next_run_at_utc bigint not null default 0,
    last_run_at_utc bigint,
    last_success_at_utc bigint,
    last_status text,
    consecutive_failures bigint not null default 0,
    updated_at_utc bigint not null default 0,
    created_at timestamp with time zone not null default now(),
    updated_at timestamp with time zone not null default now()
);

create table if not exists scheduler.job_definitions (
    job_definition_id bigserial primary key,
    job_id text not null,
    version integer not null,
    prompt_template_id bigint,
    created_at timestamp with time zone not null default now(),
    retired_at timestamp with time zone,
    task_text text not null
);

create table if not exists scheduler.job_runs (
    run_id text primary key,
    job_id text not null,
    job_definition_id bigint,
    request_id text,
    scheduled_for timestamp with time zone not null,
    scheduled_for_utc bigint not null,
    trigger_reason text,
    started_at timestamp with time zone,
    started_at_utc bigint,
    finished_at timestamp with time zone,
    finished_at_utc bigint,
    status scheduler.run_status not null,
    result_summary text,
    error_text text,
    attempt integer not null default 1,
    worker_phase text,
    worker_agent_id text,
    worker_execution_timed_out boolean not null default false,
    notification_policy_json text
);

create table if not exists scheduler.cron_mutations (
    mutation_seq bigserial primary key,
    job_id text not null,
    action text not null,
    occurred_at_ms bigint not null,
    carrier_message_id text not null,
    request_id text not null,
    tool_call_id text not null,
    input_message_ids text[] not null,
    created_at timestamp with time zone not null default now()
);

create table if not exists scheduler.terminal_signals (
    run_id text primary key,
    signal_type text not null,
    message text,
    producer_request_trace_json text,
    created_at timestamp with time zone not null default now(),
    created_at_utc bigint not null default 0,
    producer_agent_id text,
    producer_phase_attempt integer,
    blocked_reason text,
    blocked_dependency text
);

create table if not exists scheduler.delivery_outbox (
    delivery_key text primary key,
    job_id text not null,
    run_id text not null,
    payload_json text not null,
    state text not null,
    attempt_count integer not null default 0,
    next_attempt_at_utc bigint not null,
    last_error text,
    created_at_utc bigint not null,
    updated_at_utc bigint not null,
    delivered_at_utc bigint
);
