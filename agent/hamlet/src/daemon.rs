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

/// Команда очистки разговора и тексты ядра.
pub const CLEAR_COMMAND: &str = "/clear";
pub const CLEAR_BOUNDARY: &str = "Conversation history before this boundary was cleared.";
pub const CLEARED: &str = "Conversation cleared.";

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
        // Окна, открытые до перезапуска, — снова открыты, оборванная работа
        // продолжается.
        let browsers = h.browsers.clone();
        let windows = tokio::spawn(async move {
            if let Err(e) = browsers.recover().await {
                eprintln!("browser recovery: {e}");
            }
        });
        let s = h.scheduler.clone();
        let handle = tokio::spawn(async move { s.run_forever().await });
        Services { handles: vec![handle, reattach, recover, windows] }
    }

    pub async fn stop(self, h: &Hamlet) {
        for x in self.handles {
            x.abort();
        }
        h.workflows.shutdown().await;
        h.browsers.shutdown().await;
    }
}

/// Метка времени входа — форма ядра: `0xbc4b050` склеивает через пробел
/// время `[Day YYYY-MM-DD HH:MM:SS TZ]` (`0xf19a160` → `0xf19a3d0`: дни
/// `Mon`…`Sun`, `{:04}-{:02}-{:02} {:02}:{:02}:{:02}` `0xb38799`, `%Z`,
/// скобки `0x2dcb58`) и `[client_timezone=…]` (`0x409b29`). Без пояса метки
/// нет, как у muse (`0xbc4c4cd`); пояс проверен при загрузке `agent.toml`.
pub fn time_tag(ms: i64, timezone: Option<&str>) -> Option<String> {
    use chrono::TimeZone;
    let tz: chrono_tz::Tz = timezone?.parse().ok()?;
    let t = tz.timestamp_millis_opt(ms).single()?;
    Some(format!("[{}] [client_timezone={}]", t.format("%a %Y-%m-%d %H:%M:%S %Z"), tz.name()))
}

impl Hamlet {
    /// Заметка изменчивого — метка времени входа (`time_tag`); без пояса — нет.
    pub fn volatile(&self) -> Option<String> {
        time_tag(self.clock.now_ms(), self.timezone.as_deref())
    }

    /// Перезапуск (`restart_rehydration` muse): вести, взятые ходом, которого
    /// нет в точке перезапуска, — `aborted`; вызовы без результата получают
    /// результат «service restarted»; ход из точки `before_inference`, если
    /// история кончается не ответом модели, — продолжается под тем же именем
    /// (его вести закроются по итогу), иначе он считается законченным: вести —
    /// `completed`, точка — `idle`. Возвращает имя хода для продолжения.
    pub async fn recover(&self, conv: &mut Conversation) -> Result<Option<String>, String> {
        let interrupted = self.history.interrupted().await?.map(|c| c.message_id);
        self.history.abort_orphaned(interrupted.as_deref()).await?;
        if conv.close_interrupted() > 0 {
            self.history.sync(&conv.history, conv.last_volatile()).await?;
        }
        let Some(id) = interrupted else { return Ok(None) };
        if conv.history.last().is_some_and(|m| m.role != Role::Assistant) {
            return Ok(Some(id));
        }
        self.inbox.complete(&id, true).await?;
        self.history.checkpoint(Some(crate::ROOT_RECOVERY_CLASS), None, None).await?;
        Ok(None)
    }

    /// Один ход главной сессии. Перед ходом — сжатие, если разговор дорос
    /// до порога. `resumed` — имя хода, оборванного перезапуском.
    pub async fn turn(&self, conv: &mut Conversation, resumed: Option<String>) -> Result<String, String> {
        // Сброс перед сжатием пропускается, если периодический уже покрыл
        // весь разговор (`periodic_flush_covered`).
        let covered = self.history.flush_progress().await? >= conv.history.len();
        if let Some(done) = self.compactor(covered).compact_if_needed(conv).await.map_err(|e| e.to_string())? {
            self.history.replace(&conv.history, conv.last_volatile(), &done).await?;
            // Сброс перед сжатием покрыл всё, что было до него.
            self.history.set_flush_progress().await?;
        }
        // Поданное в ящик до хода: строка владельца — за заметкой изменчивого
        // (метка времени перед сообщением, как у главного muse), весть
        // службы — как есть; записывается историей и
        // отмечается за ходом.
        static TURNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let turn_id = resumed.unwrap_or_else(|| format!("{}:turn:{}:{}", crate::ROOT_AGENT, self.clock.now_ms(), TURNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let start = conv.history.len();
        let (ids, taken) = self.inbox.take();
        for m in taken {
            if m.role == Role::User {
                match self.volatile() {
                    Some(tag) => conv.push_user(m.content, &tag),
                    None => conv.history.push(m),
                }
            } else {
                conv.history.push(m);
            }
        }
        self.history.sync(&conv.history, conv.last_volatile()).await?;
        self.inbox.attach(&ids, &turn_id).await?;
        // Точка перезапуска: ход идёт.
        self.history.checkpoint(Some(crate::ROOT_RECOVERY_CLASS), None, Some(&turn_id)).await?;
        let mut t = Turn::from_prefix(&*self.main, &self.tools, &conv.prefix, self.spill_dir.clone());
        t.inbox = Some(self.inbox.clone());
        t.transcript = Some(self.transcript.clone());
        t.record = Some(self.history.clone());
        // Имя хода для ящика (`stream_owner_message_id`). Своё: `root:turn:<мс>:<n>`.
        t.turn_id = Some(turn_id.clone());
        let mut result = t.run(&mut conv.history, &mut |_| {}).await;
        // Проверка ответа перед отдачей: исправленный черновик — заметкой
        // главному агенту, ход продолжается и пишет окончательный ответ.
        if let Ok(resp) = &result {
            if let Some(revised) = self.verify(&conv.history, start, &resp.text()).await {
                conv.history.push(Message { role: Role::Developer, content: vec![Part::Text { text: agent_cron::verification::feedback(&revised) }] });
                result = t.run(&mut conv.history, &mut |_| {}).await;
            }
        }
        // Вести, взятые ходом, закрываются по его итогу.
        if let Err(e) = self.inbox.complete(&turn_id, result.is_ok()).await {
            eprintln!("mailbox complete: {e}");
        }
        // Точка перезапуска: между ходами.
        if let Err(e) = self.history.checkpoint(Some(crate::ROOT_RECOVERY_CLASS), None, None).await {
            eprintln!("restart checkpoint: {e}");
        }
        let resp = result.map_err(|e| e.to_string())?;
        conv.record_usage(&resp.usage);
        Ok(resp.text())
    }

    /// Проверка черновика хода (`agent_cron::verification`): только у хода
    /// со строкой владельца; классификатор видит строку и недавние реплики,
    /// проверяющий — строку, черновик и вызовы хода. `Some` — исправленный
    /// черновик.
    pub async fn verify(&self, history: &[Message], start: usize, draft: &str) -> Option<String> {
        let v = self.verification.as_ref()?;
        if draft.trim().is_empty() {
            return None;
        }
        let texts = |m: &Message| m.content.iter().filter_map(|p| if let Part::Text { text } = p { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n");
        let at = (start..history.len()).find(|&i| history[i].role == Role::User && !texts(&history[i]).is_empty())?;
        let request = texts(&history[at]);
        let mut recent: Vec<(String, String)> = history[..at]
            .iter()
            .filter(|m| matches!(m.role, Role::User | Role::Assistant))
            .filter_map(|m| {
                let t = texts(m);
                (!t.is_empty()).then(|| (if m.role == Role::User { "user" } else { "assistant" }.to_string(), t))
            })
            .collect();
        recent = recent.split_off(recent.len().saturating_sub(agent_cron::verification::RECENT_MESSAGES));
        let category = agent_cron::verification::classify(&v.classifier, &request, &recent, &self.spill_dir).await?;
        let trajectory = agent_cron::verification::trajectory(&history[at + 1..history.len().saturating_sub(1)]);
        match v.verifier.verify(category, &request, draft, &trajectory).await {
            agent_cron::verification::Verdict::Revised(r) => Some(r),
            agent_cron::verification::Verdict::Original | agent_cron::verification::Verdict::Unfinished => None,
        }
    }

    /// Очередь ходов: строка владельца или весть в ящике — новый ход;
    /// ответ — в `out`. Кончается, когда вход закрыт.
    pub async fn run(self: &Arc<Self>, mut lines: mpsc::Receiver<String>, out: mpsc::UnboundedSender<String>) {
        let mut conv = match self.conversation().await {
            Ok(c) => c,
            Err(e) => {
                let _ = out.send(format!("[ошибка истории] {e}"));
                return;
            }
        };
        let mut resumed = match self.recover(&mut conv).await {
            Ok(r) => r,
            Err(e) => {
                let _ = out.send(format!("[ошибка истории] {e}"));
                return;
            }
        };
        // Строка владельца сразу подаётся в ящик (`submission_kind = 'user'`)
        // — и во время хода: ход берёт её между шагами, а после остановки она
        // ждёт в базе.
        // `/clear` идёт в цикл между ходами; строки после неё ждут, пока
        // очистка не пройдёт.
        let inbox = self.inbox.clone();
        let (done_tx, mut done) = tokio::sync::oneshot::channel::<()>();
        let (clear_tx, mut clears) = mpsc::unbounded_channel::<tokio::sync::oneshot::Sender<()>>();
        let reader = tokio::spawn(async move {
            while let Some(line) = lines.recv().await {
                if line.trim() == CLEAR_COMMAND {
                    let (ack, wait) = tokio::sync::oneshot::channel();
                    if clear_tx.send(ack).is_err() {
                        break;
                    }
                    let _ = wait.await;
                    continue;
                }
                inbox.push_user(Message { role: Role::User, content: vec![Part::Text { text: line }] }).await;
            }
            let _ = done_tx.send(());
        });
        let mut closed = false;
        // После перезапуска: история кончается не ответом модели — ход
        // продолжается сразу. Граница очистки — не оборванный ход.
        let mut resume = conv.history.last().is_some_and(|m| m.role != Role::Assistant && m.text() != CLEAR_BOUNDARY);
        loop {
            if !resume {
                if closed {
                    if self.inbox.is_empty() {
                        break;
                    }
                } else {
                    tokio::select! {
                        biased;
                        Some(ack) = clears.recv() => {
                            let reply = self.clear(&mut conv).await.unwrap_or_else(|e| format!("[ошибка истории] {e}"));
                            let _ = out.send(reply);
                            let _ = ack.send(());
                            continue;
                        }
                        _ = self.inbox.wait() => {}
                        _ = &mut done => {
                            closed = true;
                            continue;
                        }
                    }
                }
            }
            resume = false;
            match self.turn(&mut conv, resumed.take()).await {
                Ok(text) => {
                    let _ = out.send(text);
                    self.periodic_flush(&conv).await;
                    // Продолжение по списку дел (`todo_continuation`).
                    if let Some(note) = self.todo_continuation().await {
                        conv.history.push(Message { role: Role::Developer, content: vec![Part::Text { text: note }] });
                        if let Err(e) = self.history.sync(&conv.history, conv.last_volatile()).await {
                            let _ = out.send(format!("[ошибка истории] {e}"));
                        }
                        resume = true;
                    }
                }
                Err(e) => {
                    let _ = out.send(format!("[ошибка хода] {e}"));
                }
            }
        }
        reader.abort();
    }

    /// Очистка разговора (`/clear`) — как у muse (`conversation_clear.rs`):
    /// только между ходами («clear_conversation requires an idle root
    /// session»); не взятые строки владельца отменяются; старый разговор
    /// сбрасывается в память в фоне («background pre-clear memory flush
    /// finished / failed after conversation clear»); история начинается с
    /// границы «Conversation history before this boundary was cleared.»;
    /// ответ — «Conversation cleared.» (`chat.command.conversation_cleared`).
    pub async fn clear(self: &Arc<Self>, conv: &mut Conversation) -> Result<String, String> {
        self.inbox.supersede_user().await?;
        let old = conv.clone();
        let boundary = Message { role: Role::Developer, content: vec![Part::Text { text: CLEAR_BOUNDARY.into() }] };
        self.history.clear(&boundary).await?;
        conv.clear(boundary);
        let h = self.clone();
        tokio::spawn(async move {
            match h.compactor(false).flush(&old).await {
                Ok(()) => eprintln!("background pre-clear memory flush finished"),
                Err(e) => eprintln!("background pre-clear memory flush failed after conversation clear: {e}"),
            }
        });
        Ok(CLEARED.into())
    }

    /// Периодический сброс памяти между ходами: со времени прошлого
    /// сброса в разговоре набралось `PERIODIC_THRESHOLD_TOKENS` токенов.
    pub async fn periodic_flush(&self, conv: &Conversation) {
        let from = match self.history.flush_progress().await {
            Ok(i) => i,
            Err(e) => {
                eprintln!("failed to read pending periodic memory flush: {e}");
                return;
            }
        };
        if conv.tokens_from(from) < agent_loop::compaction::PERIODIC_THRESHOLD_TOKENS {
            return;
        }
        eprintln!("periodic memory flush started");
        match self.compactor(false).flush(conv).await {
            Ok(()) => {
                if let Err(e) = self.history.set_flush_progress().await {
                    eprintln!("failed to persist periodic memory flush progress: {e}");
                    return;
                }
                eprintln!("periodic memory flush completed");
            }
            Err(e) => eprintln!("periodic memory flush failed: {e}"),
        }
    }

    /// Записка хода-продолжения, если в списке дел осталась работа и нет
    /// делегированной работы в пути («todo continuation skipped: delegated
    /// work in flight»). Своё: делегированная работа — подагенты (и
    /// исследователь) и исполняемые запуски `workflow`.
    pub async fn todo_continuation(&self) -> Option<String> {
        if self.subagents.running() > 0 || self.workflows.running() > 0 {
            return None;
        }
        self.continuation.next(&self.todo).await
    }
}
