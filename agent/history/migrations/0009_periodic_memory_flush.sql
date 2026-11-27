-- Периодический сброс памяти — как у muse (`memory_flush/periodic_coverage.rs`,
-- поле состояния сессии `periodic_memory_flush_progress`): до какого `seq`
-- разговор уже сброшен в память. У muse поле лежит в заголовке сессии, у
-- Hamlet заголовка нет — столбец agent.agents.

alter table agent.agents add column if not exists periodic_memory_flush_progress bigint;
