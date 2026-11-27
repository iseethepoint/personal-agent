//! `agent-cron` — расписания по форме muse `hatch-cron` /
//! `hatch-db-scheduler`: таблицы `scheduler.*` в Postgres, инструменты
//! `cron.*`, служба с тактом и работник на модели `CronWorker`; итог
//! прогона — скрытой заметкой в ящик главного агента.

pub mod clock;
pub mod schedule;
pub mod service;
pub mod store;
pub mod tools;
pub mod worker;

pub use clock::{Clock, FakeClock, SystemClock};
pub use schedule::Schedule;
pub use service::{handoff, Background, Core, Scheduler, TICK};
pub use store::{Definition, Job, Run, Store};
pub use tools::{tools, CronTool};
pub use worker::{resolution_specs, worker_message, Outcome, Signal, NOTHING_TO_REPORT, NOTIFY_MAIN_AGENT};
