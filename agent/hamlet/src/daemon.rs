//! Демон: службы и очередь ходов главной сессии — форма muse
//! (`daemon_loop/root_turn_arbiter`: ходы главной сессии по одному;
//! `scheduler loop` — такт расписаний и наблюдателей). Новый ход начинает
//! строка владельца или весть в ящике (итоги подагентов, расписаний,
//! наблюдателей, окон браузера, фоновых команд); весть, пришедшая во время
//! хода, забирается ходом между шагами (`Turn::inbox`).
//!
//! Вход для проверки — строки, ответ — текст хода (настоящий вход и
//! мессенджер — П6). Своё: заметка изменчивого — время и пояс по тексту muse
//! `session/browser_task_time_context.md` до П5.

use std::sync::Arc;

use agent_loop::{Conversation, Turn};
use agent_model::{Message, Part, Role};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::Hamlet;

/// Запущенные службы; останавливаются вместе с программой.
pub struct Services {
    handles: Vec<JoinHandle<()>>,
}

impl Services {
    /// Служба расписаний и наблюдателей (`Scheduler::run_forever`: сон до
    /// ближайшего срока или события; сверка памяти и уборщик корзины —
    /// фоновые работы её такта).
    pub fn start(h: &Hamlet) -> Services {
        // Исследования, прерванные перезапуском, — снова в работу
        // (`research_reattach` muse).
        let research = h.research.clone();
        let reattach = tokio::spawn(async move {
            research.reattach().await;
        });
        // Сохранённые работы, прерванные перезапуском, — повтор сценария
        // (`daemon_restarted_while_workflow_running`).
        let workflows = h.workflows.clone();
        let recover = tokio::spawn(async move {
            if let Err(e) = workflows.recover().await {
                eprintln!("workflow recovery: {e}");
            }
        });
        let s = h.scheduler.clone();
        let handle = tokio::spawn(async move { s.run_forever().await });
        Services { handles: vec![handle, reattach, recover] }
    }

    pub async fn stop(self, h: &Hamlet) {
        for x in self.handles {
            x.abort();
        }
        h.workflows.shutdown().await;
        h.browsers.shutdown().await;
    }
}

fn rfc3339(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (d, s) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = d + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
}

impl Hamlet {
    /// Заметка изменчивого: время и пояс владельца.
    pub fn volatile(&self) -> String {
        format!("Current date and time: {}\nUser timezone: {}", rfc3339(self.clock.now_ms()), self.timezone.as_deref().unwrap_or("not provided"))
    }

    /// Один ход главной сессии. Перед ходом — сжатие, если разговор дорос
    /// до порога.
    pub async fn turn(&self, conv: &mut Conversation) -> Result<String, String> {
        self.compactor().compact_if_needed(conv).await.map_err(|e| e.to_string())?;
        let mut t = Turn::from_prefix(&*self.main, &self.tools, &conv.prefix, self.spill_dir.clone());
        t.inbox = Some(self.inbox.clone());
        t.transcript = Some(self.transcript.clone());
        let resp = t.run(&mut conv.history, &mut |_| {}).await.map_err(|e| e.to_string())?;
        conv.record_usage(&resp.usage);
        Ok(resp.text())
    }

    /// Очередь ходов: строка владельца или весть в ящике — новый ход;
    /// ответ — в `out`. Кончается, когда вход закрыт.
    pub async fn run(self: &Arc<Self>, mut lines: mpsc::Receiver<String>, out: mpsc::UnboundedSender<String>) {
        let mut conv = self.conversation();
        let mut resume = false;
        loop {
            if !resume {
                let inbox = self.inbox.clone();
                tokio::select! {
                    biased;
                    line = lines.recv() => {
                        let Some(line) = line else { break };
                        conv.push_user(vec![Part::Text { text: line }], &self.volatile());
                    }
                    _ = inbox.wait() => {}
                }
            }
            resume = false;
            match self.turn(&mut conv).await {
                Ok(text) => {
                    let _ = out.send(text);
                    // Продолжение по списку дел (`todo_continuation`).
                    if let Some(note) = self.todo_continuation() {
                        conv.history.push(Message { role: Role::Developer, content: vec![Part::Text { text: note }] });
                        resume = true;
                    }
                }
                Err(e) => {
                    let _ = out.send(format!("[ошибка хода] {e}"));
                }
            }
        }
    }

    /// Записка хода-продолжения, если в списке дел осталась работа и нет
    /// делегированной работы в пути («todo continuation skipped: delegated
    /// work in flight»). Своё: делегированная работа — подагенты (и
    /// исследователь) и исполняемые запуски `workflow`.
    pub fn todo_continuation(&self) -> Option<String> {
        if self.subagents.running() > 0 || self.workflows.running() > 0 {
            return None;
        }
        self.continuation.next(&self.todo)
    }
}
