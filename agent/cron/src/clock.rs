//! Часы службы расписаний. В работе — системные; в тестах — поддельные,
//! чтобы не ждать минуты (карточка П4е).

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

pub trait Clock: Send + Sync {
    /// Сейчас, миллисекунды Unix.
    fn now_ms(&self) -> i64;
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
}

impl FakeClock {
    pub fn at(ms: i64) -> Self {
        FakeClock { ms: Arc::new(AtomicI64::new(ms)) }
    }

    pub fn set(&self, ms: i64) {
        self.ms.store(ms, Ordering::SeqCst);
    }

    pub fn advance_secs(&self, secs: i64) {
        self.ms.fetch_add(secs * 1000, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.ms.load(Ordering::SeqCst)
    }
}
