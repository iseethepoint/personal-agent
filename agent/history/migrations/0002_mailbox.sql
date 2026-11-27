-- Ящик агента — agent.message_mailbox по схеме базы muse (каталог ядра около
-- 0x18a9a6a): весть записывается (`accepted`), ход берёт её (`attached`,
-- stream_owner_message_id, checkpoint_custody = 'executing') и закрывает
-- (`completed` / `aborted`, terminalized_at). Не взят внешний ключ
-- transcript_event_seq (нет runtime.events).

create table if not exists agent.message_mailbox (
    submission_id text primary key,
    accepted_ordinal bigint not null,
    agent_id text not null references agent.agents(agent_id),
    conversation_epoch bigint not null,
    submission_kind text not null,
    routing_scope text not null,
    state text not null,
    stream_owner_message_id text,
    submission_json text not null,
    transcript_event_seq bigint,
    accepted_at timestamp with time zone not null default now(),
    attached_at timestamp with time zone,
    terminalized_at timestamp with time zone,
    checkpoint_custody text,
    unique (agent_id, conversation_epoch, accepted_ordinal)
);
