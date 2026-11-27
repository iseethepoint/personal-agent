-- Сохранённые работы — таблицы по схеме базы muse (tool-schemas.txt:2085-2211,
-- запросы strings-all.txt:196000-196540): runtime.workflow_runs,
-- workflow_phase_runs, workflow_agent_calls. Берутся только колонки, которые
-- служба заполняет. Не взяты (их служб у Hamlet нет): у workflow_runs —
-- root_agent_id, parent_message_id, launching_*, launch_occurrence_* и
-- таблица workflow_launch_occurrence_aliases (у нас нет повторной доставки
-- вызова инструмента), task_id, request_trace_json, executor_id,
-- execution_attempt, heartbeat_at, lease_expires_at (одна служба в одном
-- процессе — аренды нет), terminal_handoff_transport/delivery_target/
-- chat_context_json/attempt_count/last_attempt_at/next_attempt_at/last_error
-- (передача — в ящик того же процесса), sync_wait_deadline_at,
-- magi_workload_class; у workflow_agent_calls — child_message_id.
-- Состояния — значения из запросов ядра: запуск queued, running, paused,
-- stopping, stopped, completed, failed, syntax_error; вызов pending,
-- running, blocked, completed, cached, failed, stopped.

create schema if not exists runtime;

create table if not exists runtime.workflow_runs (
    run_id text primary key,
    launch_mode text not null default 'async',
    resume_from_run_id text references runtime.workflow_runs(run_id),
    workflow_name text not null,
    description text not null default '',
    phases_json jsonb not null default '[]',
    args_json jsonb not null default '{}',
    workspace_root text not null,
    script_path text not null,
    script_sha256 text not null,
    recovery_count integer not null default 0,
    last_recovery_reason text,
    last_recovered_at timestamptz,
    status text not null default 'queued',
    error text,
    final_result text,
    created_at timestamptz not null default now(),
    started_at timestamptz,
    updated_at timestamptz not null default now(),
    completed_at timestamptz,
    terminal_handoff_message_id text,
    terminal_handoff_delivered_at timestamptz,
    terminal_handoff_disposition text,
    terminal_handoff_generation integer not null default 0
);

create table if not exists runtime.workflow_phase_runs (
    phase_run_id text primary key,
    run_id text not null references runtime.workflow_runs(run_id) on delete cascade,
    phase_name text not null,
    ordinal integer not null,
    status text not null default 'pending',
    agent_total integer not null default 0,
    agent_completed integer not null default 0,
    input_tokens bigint not null default 0,
    output_tokens bigint not null default 0,
    started_at timestamptz,
    updated_at timestamptz not null default now(),
    completed_at timestamptz,
    error text,
    constraint workflow_phase_runs_unique_phase unique (run_id, phase_name)
);

create table if not exists runtime.workflow_agent_calls (
    call_id text primary key,
    run_id text not null references runtime.workflow_runs(run_id) on delete cascade,
    phase_run_id text references runtime.workflow_phase_runs(phase_run_id),
    replay_key text not null,
    cache_key text not null,
    call_ordinal integer not null,
    prompt text not null,
    options_json jsonb not null default '{}',
    child_agent_id text,
    status text not null default 'pending',
    cached_from_call_id text references runtime.workflow_agent_calls(call_id),
    final_response text,
    input_tokens bigint not null default 0,
    output_tokens bigint not null default 0,
    tool_call_count integer not null default 0,
    duration_ms bigint,
    created_at timestamptz not null default now(),
    started_at timestamptz,
    updated_at timestamptz not null default now(),
    completed_at timestamptz,
    error text,
    unique (run_id, replay_key)
);
