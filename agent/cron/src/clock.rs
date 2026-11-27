//! Часы службы расписаний. В работе — системные; в тестах — поддельные,
//! чтобы не ждать минуты (карточка П4е).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

pub type Sleep<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait Clock: Send + Sync {
    /// Сейчас, миллисекунды Unix.
    fn now_ms(&self) -> i64;

    /// Спать до отметки `deadline_ms` по этим часам.
    fn sleep_until(&self, deadline_ms: i64) -> Sleep<'_> {
        let wait = (deadline_ms - self.now_ms()).max(0) as u64;
        Box::pin(tokio::time::sleep(std::time::Duration::from_millis(wait)))
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}

/// Часы, которые двигает тест.
#[derive(Clone)]
pub struct FakeClock {
    ms: Arc<AtomicI64>,
    moved: Arc<Notify>,
}

impl FakeClock {
    pub fn at(ms: i64) -> Self {
        FakeClock { ms: Arc::new(AtomicI64::new(ms)), moved: Arc::new(Notify::new()) }
    }

    pub fn set(&self, ms: i64) {
        self.ms.store(ms, Ordering::SeqCst);
        self.moved.notify_waiters();
    }

    pub fn advance_secs(&self, secs: i64) {
        self.ms.fetch_add(secs * 1000, Ordering::SeqCst);
        self.moved.notify_waiters();
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.ms.load(Ordering::SeqCst)
    }

    /// Просыпается, когда часы переведены за отметку.
    fn sleep_until(&self, deadline_ms: i64) -> Sleep<'_> {
        Box::pin(async move {
            loop {
                let moved = self.moved.notified();
                if self.now_ms() >= deadline_ms {
                    return;
                }
                moved.await;
            }
        })
    }
}
