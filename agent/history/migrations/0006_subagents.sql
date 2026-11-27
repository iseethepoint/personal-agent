-- Подагенты — agent.subagent_spawns по схеме базы muse (каталог ядра,
-- `subagent_spawns_child_agent_id_key`, уникальность (parent_agent_id,
-- child_agent_id)); строка подагента в agent.agents — `kind = 'subagent'`.
-- Не взят внешний ключ request_id (нет runtime.requests).

create table if not exists agent.subagent_spawns (
    spawn_id bigserial primary key,
    parent_agent_id text not null,
    child_agent_id text not null unique,
    parent_message_id text,
    parent_request_id text,
    root_request_id text,
    root_message_execution_id text,
    request_id text,
    created_at bigint not null default (extract(epoch from now()))::bigint,
    status text,
    deferred_terminal_status text,
    final_response text,
    completed_at bigint,
    prompt text,
    requester_source text,
    requester_chat_context_json text,
    seen_at bigint,
    child_depth integer not null default 0,
    agent_type text,
    metadata_json text,
    spawn_call_id text,
    handoff_state_json jsonb,
    public_forget_confirmed_at bigint,
    unique (parent_agent_id, child_agent_id)
);
