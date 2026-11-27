-- Поиск по прошлым разговорам — как у muse: реплики пользователя и
-- ассистента главного чата лежат в runtime.search_documents (у muse —
-- owner_table 'runtime.events', у Hamlet событий нет — 'agent.context_items',
-- owner_key `<agent_id>:<seq>`), словарь 'simple'. Таблица та же, что
-- заводит agent-memory (0001_memory.sql), — здесь на случай базы без памяти.

create schema if not exists runtime;

create table if not exists runtime.search_documents (
    search_document_id bigserial primary key,
    owner_table text not null,
    owner_key text not null,
    language regconfig not null default 'english'::regconfig,
    search_vector tsvector not null,
    updated_at timestamp with time zone not null default now(),
    search_text text not null,
    unique (owner_table, owner_key)
);
