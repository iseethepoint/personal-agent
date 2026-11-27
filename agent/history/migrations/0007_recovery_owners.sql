-- Владельцы восстановления — agent.recovery_owners по схеме базы muse
-- (каталог ядра: ключ (recovery_class, owner_id), agent_id уникален):
-- работа службы (прогон расписания) и агент-работник, который её исполняет.
-- Не взята agent.recovery_owner_terminal_events (журнал закрытий служб,
-- которых у Hamlet нет).

create table if not exists agent.recovery_owners (
    recovery_class text not null,
    owner_id text not null,
    generation bigint not null default 1,
    agent_id text not null unique,
    message_id text not null,
    active_at timestamp with time zone not null default now(),
    terminal_at timestamp with time zone,
    terminal_status text,
    primary key (recovery_class, owner_id)
);
