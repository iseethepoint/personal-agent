-- Эскалация цели — колонки схемы базы muse `goals.goals.escalation` (jsonb
-- {condition, due_at, action}) и `escalation_due_at`; срок вынесен в
-- колонку для выборки будильника (`goals.upcoming_escalations`,
-- sql.txt:4314) и снимается вместе с эскалацией при завершении
-- (`escalation_due_at=case when $5='completed' then null …`,
-- strings-all.txt, «clear goal escalation on completion»).

alter table goals.goals add column if not exists escalation jsonb;
alter table goals.goals add column if not exists escalation_due_at timestamp with time zone;
