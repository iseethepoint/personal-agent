-- История агентов — таблицы по схеме базы muse (вшитый каталог ядра около
-- 0x18a9a6a): agent.agents, agent.context_items, agent.volatile_context_pins.
-- Значения перечислений — из списка типов ядра («message_role developer
-- assistant … context_item_kind message_text text_segments tool_call
-- tool_output») и запросов (`item_kind IN ('message', 'message_text',
-- 'text_segments')`, около 0x2ecebb); роль `user` — из запросов
-- (`role IN ('user', 'developer')`).
-- Вид inspection.context_items — `redaction` каталога ядра у context_items:
-- «Rows holding model reasoning (thinking and redacted-thinking items) are
-- withheld»; у Hamlet это строки `message` с частью мышления.
-- Не взяты (служб у Hamlet нет): у agents — request_id (нет runtime.requests),
-- last_assistant_resources, presentation_root_session_id,
-- originating_location_context_json, handoff_context_high_water_seq; у
-- context_items — внешние ключи на runtime.messages/tool_calls/tool_outputs
-- (этих таблиц нет: сообщение целиком лежит в item_json) и data_* (очистка
-- данных Meta).

create schema if not exists agent;
create schema if not exists runtime;

do $$ begin
    create type runtime.message_role as enum ('user', 'assistant', 'developer');
exception when duplicate_object then null; end $$;

do $$ begin
    create type agent.context_item_kind as enum ('message', 'message_text', 'text_segments', 'tool_call', 'tool_output');
exception when duplicate_object then null; end $$;

create table if not exists agent.agents (
    agent_id text primary key,
    id text unique,
    session_id text,
    kind text not null default 'root',
    parent_id text,
    parent_agent_id text,
    root_session_id text,
    model text not null default '',
    status text not null,
    depth integer not null default 0,
    created_at bigint not null default (extract(epoch from now()))::bigint,
    updated_at bigint not null default (extract(epoch from now()))::bigint,
    last_assistant_message text,
    seen_at_ms bigint,
    prompt_floor_seq bigint,
    ephemeral boolean not null default false,
    agent_type text,
    spawn_metadata_json text
);

create table if not exists agent.context_items (
    context_item_id bigserial primary key,
    agent_id text not null,
    seq bigint not null,
    item_kind agent.context_item_kind not null,
    message_id text,
    role runtime.message_role,
    call_id text,
    tool_name text,
    success boolean,
    message_source text,
    created_at timestamp with time zone not null default now(),
    text_content text,
    item_json text,
    unique (agent_id, seq)
);

create table if not exists agent.volatile_context_pins (
    agent_id text not null,
    variant_hash text not null,
    variant_json text not null,
    prompt_floor_seq bigint not null,
    checkpoint_seq bigint not null,
    body text,
    created_at timestamp with time zone not null default now(),
    body_hash bytea,
    primary key (agent_id, variant_hash)
);

create schema if not exists inspection;

create or replace view inspection.context_items as
    select * from agent.context_items
    where not (item_kind = 'message' and item_json::jsonb ->> 'type' in ('thinking', 'redacted_thinking'));
