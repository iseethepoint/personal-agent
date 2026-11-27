-- Отложенный итог прогона — как у muse (`scheduler/pending_terminal_result.rs`,
-- столбец scheduler.job_runs.pending_terminal_result_json text): итог
-- работника, ждущий проверки; категория проверки хранится в нём же
-- (`retain_cron_verification_category`, ключ `verification_category`).

alter table scheduler.job_runs add column if not exists pending_terminal_result_json text;
