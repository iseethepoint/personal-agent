-- Сжатия и счёт токенов — agent.agent_compactions и
-- agent.agent_message_token_usage по схеме базы muse (каталог ядра около
-- 0x18a9a6a; вставки около 0x16f4750 и 0x1700cb4). Вид
-- inspection.agent_compactions — `redaction` каталога ядра: details_json
-- скрыт («carries the model reasoning retained across the checkpoint»).

create table if not exists agent.agent_compactions (
    id bigserial primary key,
    agent_id text not null,
    summary text not null,
    first_kept_seq bigint,
    checkpoint_seq bigint,
    tokens_before bigint,
    tokens_after bigint,
    details_json text,
    trigger text not null,
    will_retry boolean not null default false,
    created_at bigint not null,
    has_replacement_history boolean not null default false
);

create table if not exists agent.agent_message_token_usage (
    agent_id text not null,
    message_id text not null,
    input_tokens bigint not null,
    output_tokens bigint not null,
    created_at bigint not null,
    primary key (agent_id, message_id)
);

create or replace view inspection.agent_compactions as
    select id, agent_id, summary, first_kept_seq, checkpoint_seq, tokens_before, tokens_after,
           trigger, will_retry, created_at, has_replacement_history
    from agent.agent_compactions;
