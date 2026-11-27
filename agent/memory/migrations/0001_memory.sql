-- Память — таблицы по схеме базы muse (muse-live core-v2-analysis/strings-all.txt,
-- JSON-схема 201212-222912: memory.claims, memory.embedding_models,
-- memory.embeddings, memory.entries, memory.entry_attributes, memory.metadata,
-- runtime.search_documents). Колонки, типы, значения по умолчанию и ограничения —
-- как там. Пропуски в номерах колонок muse (entries 7, 8, 10; search_documents 5)
-- не повторяются: в схеме их нет.
--
-- Своё (в схеме muse не видно): индекс HNSW по векторам. У muse его выдаёт
-- запрос `set local hnsw.iterative_scan = 'strict_order'` перед поиском по
-- векторам (strings-all.txt:200455) — без индекса HNSW эта настройка ни на что
-- не влияет.

create extension if not exists vector;
create schema if not exists memory;
create schema if not exists runtime;

create table if not exists memory.entries (
    memory_entry_id bigserial primary key,
    memory_uri text not null unique,
    chunk_id text not null,
    source_type text not null,
    status text not null,
    privacy_class text not null,
    confidence double precision not null default 1.0,
    citation_path text,
    line_start bigint not null default 0,
    line_end bigint not null default 0,
    created_at timestamp with time zone not null default now(),
    created_at_unix bigint not null default 0,
    title_text text,
    body_text text not null,
    reason_text text
);

create table if not exists memory.entry_attributes (
    memory_entry_attribute_id bigserial primary key,
    memory_entry_id bigint not null references memory.entries (memory_entry_id),
    attribute_name text not null,
    scalar_value text not null,
    unique (memory_entry_id, attribute_name)
);

create table if not exists memory.embedding_models (
    embedding_model_id bigserial primary key,
    model_name text not null,
    dimensions integer not null,
    distance_metric text not null,
    created_at timestamp with time zone not null default now(),
    unique (model_name, dimensions, distance_metric)
);

create table if not exists memory.embeddings (
    memory_embedding_id bigserial primary key,
    memory_entry_id bigint not null references memory.entries (memory_entry_id),
    embedding_model_id bigint not null references memory.embedding_models (embedding_model_id),
    embedding vector(384) not null,
    created_at timestamp with time zone not null default now(),
    unique (memory_entry_id, embedding_model_id)
);

create index if not exists embeddings_embedding_hnsw
    on memory.embeddings using hnsw (embedding vector_cosine_ops);

create table if not exists memory.claims (
    claim_id text primary key,
    run_id text not null,
    kind text not null,
    salience text not null,
    claim_text text not null,
    quote text,
    speaker text not null,
    evidence_handles jsonb not null default '[]'::jsonb,
    supersedes_claim_id text,
    status text not null default 'active',
    confidence double precision not null,
    first_seen timestamp with time zone not null default now(),
    reinforced_at timestamp with time zone not null default now(),
    valid_until timestamp with time zone,
    source_path text not null,
    source_line bigint not null,
    created_at timestamp with time zone not null default now(),
    updated_at timestamp with time zone not null default now()
);

create table if not exists memory.metadata (
    key text primary key,
    value text not null,
    updated_at timestamp with time zone not null default now()
);

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
