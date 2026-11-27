-- Цели — таблицы по схеме базы muse (strings-all.txt, JSON-схема базы,
-- схема goals): goals.goals, goals.updates, goals.briefings. Колонки, типы
-- и умолчания — как там; берутся колонки, которые заполняют инструменты.
-- Не взяты (их служб у Hamlet пока нет): у goals — connection_*, research_*,
-- settled_*, escalation*, watch, icon_generation_status, image_relpath,
-- activity_key, user_words, assistant_distillation, woop, implementation_intentions, momentum_dimensions, provenance и прочие
-- поля самоулучшения и изучения; таблицы actions, associations,
-- engagement_events, learning_state, momentum_history, sessions,
-- suggestions, threads, thread_actions.

create schema if not exists goals;

create table if not exists goals.goals (
    goal_id text primary key,
    slug text unique,
    title text not null,
    summary text,
    description text,
    momentum text,
    momentum_status text,
    emoji text,
    status text not null,
    created_at text not null,
    updated_at text not null,
    parent_goal_id text references goals.goals (goal_id),
    sort_order double precision not null default 0,
    completed_at text,
    created_at_ts timestamp with time zone,
    updated_at_ts timestamp with time zone,
    last_activity_at text,
    last_activity_at_ts timestamp with time zone,
    pushable boolean not null default true,
    category text,
    goal_kind text,
    value_alignment text,
    completed_at_ts timestamp with time zone,
    source text not null default 'user_goal',
    attention_kind text not null default 'does_not_need_attention',
    attention_updated_at timestamp with time zone not null default now(),
    next_due_at timestamp with time zone,
    fixed_position bigint
);

create table if not exists goals.updates (
    update_id text primary key,
    goal_id text not null references goals.goals (goal_id),
    title text,
    summary text,
    description text,
    effective_at text,
    created_at text not null,
    updated_at text not null,
    effective_at_ts timestamp with time zone,
    updated_at_ts timestamp with time zone,
    author_source text not null default 'user'
);

create table if not exists goals.briefings (
    briefing_id text primary key,
    goal_id text not null references goals.goals (goal_id),
    path text not null,
    created_at text not null,
    updated_at text not null,
    last_opened_at text,
    created_at_ts timestamp with time zone,
    updated_at_ts timestamp with time zone,
    last_opened_at_ts timestamp with time zone,
    hero_image_path text,
    unique (goal_id, path)
);
