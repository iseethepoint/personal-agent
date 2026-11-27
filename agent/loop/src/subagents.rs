//! Подагенты — по форме muse `subagents_tool/*` (`source-files.txt:1757-1762`),
//! `subagent_execution.rs`, `subagent_completion_stream.rs` (`:1352-1353`).
//!
//! Подагент — тот же цикл хода со своей историей и своим набором
//! инструментов. Подагент глубины 1 наследует разговор родителя, глубже —
//! изолирован («Any subagents you spawn deeper than 1 start in isolated
//! mode», `subagent_history_inheritance_inherited.md`;
//! `subagent_context_depth_one_spawn_inheritance.md`). Глубин две, как у
//! ролей ядра: у `owner_subagent_depth1` есть `subagent.*`, у
//! `owner_subagent_depth2` нет (`prompts/roles/`); итог подагента глубины 2
//! приходит его родителю, а не корню («Your sub-agents will announce their
//! results back to you automatically (not to your parent agent)»,
//! `subagent_spawning.md`). Наследуется вся видимая модели история родителя
//! на момент вызова (у muse — от `root_prompt_floor_seq`, то есть после
//! сжатия; `strings-all.txt:84153`): копия, история родителя не меняется.
//! За ней — заметка muse «Above is the conversation history…»
//! (`active_turn_subagent_history_developer_message.md`), затем первое
//! сообщение подагента. Работа асинхронная, как велят описания инструментов muse:
//! `subagent.spawn` сразу отдаёт id, итог приходит родителю сам — в его
//! ящик (`Inbox`), а оттуда заметкой в историю перед следующим вызовом
//! модели. Сессия подагента живёт и после итога: `subagent.send` даёт ей
//! новую работу.
//!
//! С хранилищем (`SubagentStore`, П3е-5) подагент живёт в базе, как у muse
//! (`agent.agents` с `kind = 'subagent'`, `agent.subagent_spawns`, своя
//! история и точка перезапуска): после перезапуска подагент с точкой
//! `before_inference` продолжает ход (вызовы без результата закрываются
//! текстом ядра), незаконченный без такой точки — `errored` с итогом
//! родителю («marked orphaned active subagent errored after restart»,
//! `restart_rehydration/orphan_reconciliation.rs`), законченные — снова в
//! списке и принимают `subagent.send`. Особые подагенты (`Kind`) в базу не
//! пишутся: их поднимает своя служба (исследователь, `workflow`).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};

use crate::compaction::personal;
use crate::prompt::{close_interrupted, Prefix};
use crate::tool::{Tool, ToolError, ToolFuture, Tools};
use crate::turn::{Turn, TurnError};

/// Срок команды подагенту — 90 с (`JARVIS_SUBAGENT_CONTROL_OP_TIMEOUT_MS`;
/// опись, таблица 4.2).
pub const CONTROL_TIMEOUT: Duration = Duration::from_secs(90);

/// Предельная глубина — из ролей ядра: у `owner_subagent_depth2` нет
/// `subagent.*` (отказ ядра на глубине «Agent depth limit reached. Solve the
/// task yourself.», `0x883b6b`, не нужен: инструмента там нет).
pub const MAX_SPAWN_DEPTH: u32 = 2;

/// Тексты muse, перенесены как есть (`prompts/subagent/`).
pub const USER_MESSAGE_MD: &str = include_str!("../../../prompts/subagent/subagent_context_user_message.md");
pub const STANDARD_MD: &str = include_str!("../../../prompts/subagent/subagent_context_standard.md");
pub const PERSISTENT_SESSION_MD: &str =
    include_str!("../../../prompts/subagent/subagent_context_persistent_session.md");
pub const INHERITED_MD: &str = include_str!("../../../prompts/subagent/subagent_history_inheritance_inherited.md");
pub const PARENT_BACKGROUND_MD: &str = include_str!("../../../prompts/subagent/subagent_context_parent_background.md");
pub const ACTIVE_TURN_MD: &str =
    include_str!("../../../prompts/subagent/active_turn_subagent_history_developer_message.md");
pub const ISOLATED_MD: &str = include_str!("../../../prompts/subagent/subagent_history_inheritance_isolated.md");
pub const DEPTH_ONE_COORDINATOR_MD: &str = include_str!("../../../prompts/subagent/subagent_context_depth_one_coordinator.md");
pub const DEPTH_ONE_SPAWN_INHERITANCE_MD: &str =
    include_str!("../../../prompts/subagent/subagent_context_depth_one_spawn_inheritance.md");
/// Раздел системного текста подагента, который может заводить своих
/// (`subagent_spawning_section` при `can_spawn` в шаблоне ядра).
pub const SPAWNING_MD: &str = include_str!("../../../prompts/subagent/subagent_spawning.md");

/// Описания инструментов — строки ядра muse (0x2cdb1b3–0x2cdd367), полные
/// варианты: первая строка и абзацы через пустую строку. Правка (П10): в
/// групповом куске `subagent.list` «this Muse's» → «this Hamlet's».
const LIST_DESCRIPTION: &str = "List your child agents and the agents of <!-- personal-only:start -->the user's<!-- personal-only:end --><!-- group-only:start -->this Hamlet's<!-- group-only:end --> chats and scheduled jobs, grouped into active and recent, with a normalized status (running, done, interrupted, failed), task and final response previews, timing, and whether a finished result was already seen.\n\nRunning entries include how long the agent has been running and how long since its last activity. A quiet agent is not proof it is stuck; it may be inside one long tool call. Entries carry the agent ids the other subagent tools need, plus the browser task id for browser task owners and the artifact slug for artifact builders. An entry a chat or a scheduled job owns carries `owner`, saying which, with the schedule and run ids for scheduled work, so a scheduled job's agents are never mistaken for missing. You can view agents owned by another chat or a scheduled job, but the subagent tools cannot steer or stop them: a scheduled job's in-flight run stops with the cron stop tool and the schedule id shown in `owner`, and another chat's agents cannot be closed from here at all.\n\nCheck status only to act on it: <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->a member<!-- group-only:end --> asks where things stand, you need an agent id, or you suspect slow or failed work. Finished results are delivered to you automatically; mentioning running work unprompted needs no fresh check, and calling this in a polling loop wastes turns.";
const LIST_SCOPE: &str = "'direct' (default): your immediate children. 'descendants': the full subtree of agents under you.";
const LIST_LIMIT: &str = "Max entries to return. Default 20, max 200.";
const SEND_DESCRIPTION: &str = "Deliver follow-up input to one of your running child agents by id.\n\nDelivery is ASYNC: a success response means the message was submitted, not applied. The instruction stays pending until the agent's handoff or completion result reports it, so do not describe it as done, do not poll for it, and do not fabricate the outcome. Handle anything else the turn needs, then end your response.\n\nProvide the input in `message`, or in `items` for images or files; `items` takes precedence when both are set. Set `interrupt` true to cut off the agent's current work first, so the new input starts a fresh turn instead of queueing behind the old one.\n\nBrowser task owners cannot be steered by agent id; their parent agent continues the task through its browser tools instead. To start ordinary delegated work when no suitable agent exists, use `subagent.spawn`.";
const TARGET_ID: &str = "Target agent id (from subagent.spawn or subagent.list output).";
const SEND_MESSAGE: &str = "Follow-up text for the agent. If items is also provided, items takes precedence.";
const SEND_ITEMS: &str = "Multimodal input (text/image/file); takes precedence over message. Type \"text\" requires `text`. Types \"image\" and \"file\" require `path`, a file path as `read` takes it; `filename` and `caption` are optional.";
const SPAWN_ITEMS: &str = "Multimodal input for the child's first message; takes precedence over message. Type \"text\" requires `text`. Types \"image\" and \"file\" require `path`, a file path as `read` takes it; `filename` and `caption` are optional.";
const SEND_SCOPE: &str = "Where the target may live: 'direct' (default) covers your immediate children, 'descendants' the full subtree under you.";
const SEND_INTERRUPT: &str = "If true, interrupt the agent's current work before delivering this input.";
const CLOSE_DESCRIPTION: &str = "Stop one of your child agents by id and end its work.\n\nUse this when the delegated work is no longer needed, the agent is stuck, or it should not keep going. To redirect an agent instead of stopping it, use `subagent.send`; to find the right id and check state first, use `subagent.list`.\n\nClosing a browser task owner stops the whole browser task, and the response includes its `browser_task_id`. If the owner's task identity cannot be recovered, the close fails and nothing is stopped.";
const SPAWN_DESCRIPTION: &str = "Delegate a task to a new child agent that works on it in the background while you continue.\n\nDo not spawn a generic child for requested website navigation, purchases, or other live-browser work: logged-in sites, clicking, forms, or multi-step interactions. That work belongs to the browser-task delegation route, not another generic child. Generic subagents may search the public web and fetch page text from links supplied by <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->members<!-- group-only:end --> or returned by tools; requests to visit or interact with websites are returned to the parent for appropriate delegation.\n\nThis call is ASYNC: the response only confirms the spawn and returns the new agent id. Results arrive later through the runtime, so never treat the response as completion and never produce results for delegated work yourself. After spawning, acknowledge what you delegated and end your response; do not poll, sleep, or promise a delivery time. Spawn all independent agents at once instead of one at a time.\n\nProvide the task in `message`, or in `items` when it needs images or files; `items` takes precedence when both are set. Manage the agent afterwards by its id: `subagent.list` shows status, `subagent.send` delivers follow-up input, `subagent.resume` restarts an interrupted one, and `subagent.close` stops one.\n\nSome work has its own dedicated parent-agent capability and cannot be spawned here, including browser tasks, artifacts (a page, app, document, deck, or spreadsheet), and accepted Idea builds. Artifacts do not need a subagent: the artifact tools already run the build in the background and report back. If <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->a member<!-- group-only:end --> asks for a subagent, tell the child to use the artifact tools instead of creating a file itself. A rejected spawn returns a structured error with a stable reason code; fix the cause it names instead of retrying the same call.";
const SPAWN_MESSAGE: &str = "Task instruction for the child. State the task, the outcome you want, any non-obvious constraint, and every fact it needs. If items is also provided, items takes precedence.";
const RESUME_DESCRIPTION: &str = "Resume one of your interrupted child agents by id so it continues its existing task from where it stopped.\n\nOnly a paused or interrupted agent can be resumed. An agent that already finished or shut down is not resumed; the call returns an error saying so, because resuming a terminal agent re-runs nothing. Spawn a new agent with `subagent.spawn` to continue that work. Resume restarts the agent on its own task; to give it new direction once it is running again, use `subagent.send`.\n\nTwo kinds of agents refuse generic resume: browser task owners continue through their parent agent's dedicated browser-task control, and a web artifact builder whose build is still in flight cannot be resumed; wait for it to finish or send it guidance with `subagent.send`.";

/// Состояние подагента — нормализованные статусы muse (running, done,
/// interrupted, failed) и закрытый.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Done,
    /// Ход прервался временной ошибкой модели; можно `subagent.resume`.
    Interrupted,
    Failed,
    Closed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Done => "done",
            Status::Interrupted => "interrupted",
            Status::Failed => "failed",
            Status::Closed => "closed",
        }
    }
}

/// Ящик родителя: итоги подагентов ждут здесь следующего вызова модели.
/// С хранилищем (`Mailbox`) весть сначала записывается (`accepted`), потом
/// встаёт в очередь — как `agent.message_mailbox` muse.
#[derive(Clone, Default)]
pub struct Inbox {
    queue: Arc<Mutex<VecDeque<Queued>>>,
    notify: Arc<Notify>,
    store: Option<Arc<dyn Mailbox>>,
}

/// Весть в очереди и её `submission_id`, если она записана.
type Queued = (Option<String>, Message);

type MailboxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, String>> + Send + 'a>>;

/// Хранилище ящика: принять весть, отметить взятые ходом, закрыть по
/// итогу хода, поднять непринятые после перезапуска.
pub trait Mailbox: Send + Sync {
    /// Записать подачу вида `kind` (`user` — строка владельца, `notice` —
    /// весть службы); вернуть её `submission_id`.
    fn accept<'a>(&'a self, m: &'a Message, kind: &'a str) -> MailboxFuture<'a, String>;
    /// Вести взяты ходом `owner` и уже лежат в его истории.
    fn attach<'a>(&'a self, ids: &'a [String], owner: &'a str) -> MailboxFuture<'a, ()>;
    /// Ход `owner` кончился: его вести — `completed` (или `aborted`).
    fn complete<'a>(&'a self, owner: &'a str, ok: bool) -> MailboxFuture<'a, ()>;
    /// Принятые и не взятые вести по порядку.
    fn pending<'a>(&'a self) -> MailboxFuture<'a, Vec<(String, Message)>>;
    /// Принятые и не взятые вести отменены (`aborted`): их заменила очистка
    /// разговора.
    fn abort<'a>(&'a self, ids: &'a [String]) -> MailboxFuture<'a, ()>;
}

/// Результат работы, которую после перезапуска нельзя безопасно
/// продолжить, — текст ядра muse (`restart_after_output_unrecoverable`,
/// около `0x2a4cd73`).
pub const WORK_NOT_RESUMABLE: &str = include_str!("../../../prompts/restart/work_not_resumable.md");

/// Подагент из базы.
#[derive(Debug, Clone)]
pub struct SavedSubagent {
    pub id: String,
    pub task: String,
    pub status: Status,
    pub result: Option<String>,
    /// Работа закончена (`completed_at`), итог отдан родителю.
    pub completed: bool,
    /// Итог проходит через `AfterDone` родителя — такой подагент после
    /// перезапуска не продолжается.
    pub finish_hook: bool,
}

/// Точка `before_inference`, оставленная оборванным ходом.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// `recovery_class` точки (у подагента пуст).
    pub class: Option<String>,
    /// Имя хода.
    pub message_id: String,
}

/// История и точка перезапуска одного агента (подагента или работника).
pub trait ChildRecord: crate::turn::Record {
    /// `Some(ход)` — ход идёт (`before_inference`), `None` — между ходами;
    /// `class` и `owner` — `recovery_class` и `recovery_owner_id` точки
    /// (`owner` пуст — id самого агента).
    fn checkpoint<'a>(&'a self, class: Option<&'a str>, owner: Option<&'a str>, turn: Option<&'a str>) -> MailboxFuture<'a, ()>;
    /// Точка хода, оборванного перезапуском.
    fn interrupted<'a>(&'a self) -> MailboxFuture<'a, Option<Checkpoint>>;
    /// Закреплённое начало и история из базы.
    fn load<'a>(&'a self) -> MailboxFuture<'a, (Prefix, Vec<Message>)>;
}

/// Незакрытый владелец восстановления: `owner_id`, работник, сведения.
pub type ActiveOwner = (String, Arc<dyn ChildRecord>, Option<String>);

/// Владельцы восстановления (`agent.recovery_owners` muse): работа службы
/// (`class`, `owner_id` — например прогон расписания) и агент, который её
/// исполняет.
pub trait WorkerStore: Send + Sync {
    /// Новый работник: строка агента, закреплённое начало, владелец;
    /// `metadata` — сведения службы для подъёма после перезапуска.
    fn begin<'a>(&'a self, class: &'a str, owner_id: &'a str, agent_id: &'a str, prefix: &'a Prefix, metadata: Option<&'a str>) -> MailboxFuture<'a, Arc<dyn ChildRecord>>;
    /// Все незакрытые владельцы класса: (`owner_id`, работник, сведения).
    fn list_active<'a>(&'a self, class: &'a str) -> MailboxFuture<'a, Vec<ActiveOwner>>;
    /// Работник незакрытого владельца.
    fn active<'a>(&'a self, class: &'a str, owner_id: &'a str) -> MailboxFuture<'a, Option<Arc<dyn ChildRecord>>>;
    /// Владелец закрыт с итогом `status`.
    fn terminal<'a>(&'a self, class: &'a str, owner_id: &'a str, status: &'a str) -> MailboxFuture<'a, ()>;
}

/// Хранилище подагентов одного родителя.
pub trait SubagentStore: Send + Sync {
    /// Все подагенты родителя по порядку заведения.
    fn saved<'a>(&'a self) -> MailboxFuture<'a, Vec<SavedSubagent>>;
    /// Новый подагент: строки агента и заведения, закреплённое начало.
    fn spawned<'a>(&'a self, id: &'a str, task: &'a str, prefix: &'a Prefix, finish_hook: bool) -> MailboxFuture<'a, Arc<dyn ChildRecord>>;
    /// Уже заведённый подагент.
    fn child<'a>(&'a self, id: &'a str) -> MailboxFuture<'a, Arc<dyn ChildRecord>>;
    /// Новое состояние; `result` — итог законченной работы.
    fn status<'a>(&'a self, id: &'a str, status: Status, result: Option<&'a str>) -> MailboxFuture<'a, ()>;
    /// Хранилище подагентов подагента `parent` — на глубину ниже.
    fn nested(&self, parent: &str) -> Arc<dyn SubagentStore>;
    /// Ящик подагента `id` (`agent.message_mailbox` по его `agent_id`).
    fn mailbox(&self, id: &str) -> Arc<dyn Mailbox>;
}

impl Inbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ящик с хранилищем; непринятые до перезапуска вести — снова в очереди.
    pub async fn persistent(store: Arc<dyn Mailbox>) -> Result<Self, String> {
        let pending = store.pending().await?;
        Ok(Self::with(store, pending))
    }

    /// Ящик с хранилищем и уже поднятыми непринятыми вестями.
    pub fn with(store: Arc<dyn Mailbox>, pending: Vec<(String, Message)>) -> Self {
        let inbox = Inbox { store: Some(store), ..Default::default() };
        inbox.queue.lock().unwrap().extend(pending.into_iter().map(|(id, m)| (Some(id), m)));
        inbox
    }

    /// Положить заметку для родителя (итог подагента, итог фоновой команды).
    /// Не записалась в хранилище — всё равно встаёт в очередь (ошибка — в журнал).
    pub async fn push(&self, m: Message) {
        self.submit(m, "notice").await
    }

    /// Подать строку владельца (`submission_kind = 'user'`).
    pub async fn push_user(&self, m: Message) {
        self.submit(m, "user").await
    }

    async fn submit(&self, m: Message, kind: &str) {
        let id = match &self.store {
            Some(s) => match s.accept(&m, kind).await {
                Ok(id) => Some(id),
                Err(e) => {
                    eprintln!("mailbox accept: {e}");
                    None
                }
            },
            None => None,
        };
        self.queue.lock().unwrap().push_back((id, m));
        self.notify.notify_one();
    }

    /// Забрать всё дошедшее.
    pub fn drain(&self) -> Vec<Message> {
        self.take().1
    }

    /// Забрать всё дошедшее вместе с `submission_id` записанных вестей.
    pub fn take(&self) -> (Vec<String>, Vec<Message>) {
        let (ids, msgs): (Vec<Option<String>>, Vec<Message>) = self.queue.lock().unwrap().drain(..).unzip();
        (ids.into_iter().flatten().collect(), msgs)
    }

    /// Взятые вести легли в историю хода `owner`.
    pub async fn attach(&self, ids: &[String], owner: &str) -> Result<(), String> {
        match &self.store {
            Some(s) if !ids.is_empty() => s.attach(ids, owner).await,
            _ => Ok(()),
        }
    }

    /// Ход `owner` кончился.
    pub async fn complete(&self, owner: &str, ok: bool) -> Result<(), String> {
        match &self.store {
            Some(s) => s.complete(owner, ok).await,
            None => Ok(()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }

    /// Очистка разговора: не взятые ходом строки владельца отменяются
    /// («rejecting a follow-up a conversation clear superseded»), вести
    /// служб остаются.
    pub async fn supersede_user(&self) -> Result<(), String> {
        let ids: Vec<String> = {
            let mut q = self.queue.lock().unwrap();
            let (user, rest): (VecDeque<Queued>, VecDeque<Queued>) = q.drain(..).partition(|(_, m)| m.role == Role::User);
            *q = rest;
            user.into_iter().filter_map(|(id, _)| id).collect()
        };
        match &self.store {
            Some(s) if !ids.is_empty() => s.abort(&ids).await,
            _ => Ok(()),
        }
    }

    /// Дождаться, пока что-нибудь дойдёт (для начала нового хода родителя,
    /// когда его ход уже кончился, а подагенты ещё работали).
    pub async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            if !self.queue.lock().unwrap().is_empty() {
                return;
            }
            notified.await;
        }
    }
}

/// Снимок истории родителя: ход кладёт сюда копию перед исполнением
/// вызовов, `subagent.spawn` берёт её отсюда.
#[derive(Clone, Default)]
pub struct Transcript {
    messages: Arc<Mutex<Vec<Message>>>,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, history: &[Message]) {
        *self.messages.lock().unwrap() = history.to_vec();
    }

    pub fn snapshot(&self) -> Vec<Message> {
        self.messages.lock().unwrap().clone()
    }
}

/// История родителя для подагента: копия без вызовов последнего ответа —
/// они ещё исполняются (среди них и сам `subagent.spawn`), результатов у них
/// нет, а поставщик не примет вызов без результата.
pub fn inherited_history(parent: &[Message]) -> Vec<Message> {
    let mut h = parent.to_vec();
    if let Some(last) = h.last_mut() {
        if last.role == Role::Assistant {
            last.content.retain(|p| !matches!(p, Part::ToolCall { .. }));
            if last.content.is_empty() {
                h.pop();
            }
        }
    }
    h
}

/// Что сделать с итогом подагента, прежде чем он уйдёт родителю (у muse
/// так завершает работу исполнитель забывания: «this executor's terminal
/// boundary owns the memory-search refresh», `strings-all.txt:55420`).
pub type AfterDone = Arc<dyn Fn(Status, String) -> futures_util::future::BoxFuture<'static, String> + Send + Sync>;

/// Заводит очередь монитора для подагента: (id, задача, ящик родителя,
/// идёт ли работа) → крючок шага.
type WatchFactory = Arc<dyn Fn(String, String, Inbox, Arc<dyn Fn() -> bool + Send + Sync>) -> crate::turn::StepHook + Send + Sync>;

enum Input {
    /// Новая работа: части сообщения роли user.
    Message(Vec<Part>),
    Resume,
}

/// Итог особого подагента для ящика родителя: (статус, последний текст) →
/// заметка. Зовётся и при `subagent.close` — «a terminal handoff still
/// arrives» (`strings-all.txt:55755`).
pub type Finish = Arc<dyn Fn(Status, String) -> futures_util::future::BoxFuture<'static, Message> + Send + Sync>;

/// Особый подагент — свой системный текст и набор инструментов, а не
/// родительские (у muse — «seat»-профиль, например `deep_research`,
/// `strings-all.txt:74383`): первое сообщение, конец работы — вызов
/// терминального инструмента (`done`), одно напоминание, если ход кончился
/// без него, срок работы и своя заметка-итог. Без монитора и без истории
/// родителя.
pub struct Kind {
    pub task: String,
    pub first: String,
    pub prefix: Prefix,
    pub tools: Arc<Tools>,
    pub terminal: Vec<String>,
    /// Дошла ли работа до терминального вызова.
    pub done: Arc<dyn Fn() -> bool + Send + Sync>,
    /// Напоминание, если ход кончился без терминального вызова.
    pub nudge: Option<String>,
    /// Срок всей работы.
    pub deadline: Option<std::time::Duration>,
    pub finish: Finish,
    /// История для продолжения после перезапуска: работа идёт с неё
    /// (`Input::Resume`), а не с первого сообщения.
    pub resume: Option<Vec<Message>>,
    /// Запись истории и точек перезапуска (владелец восстановления службы).
    pub record: Option<KindRecord>,
}

/// Запись особого подагента: история и точки с `recovery_class` и
/// `recovery_owner_id` его владельца восстановления.
#[derive(Clone)]
pub struct KindRecord {
    pub record: Arc<dyn ChildRecord>,
    pub class: &'static str,
    pub owner: String,
}

struct Agent<P> {
    id: String,
    task: String,
    status: Status,
    result: Option<String>,
    tx: mpsc::UnboundedSender<Input>,
    interrupt: Arc<Notify>,
    handle: tokio::task::JoinHandle<()>,
    /// Итог особого подагента при закрытии.
    finish: Option<Finish>,
    /// Подагенты этого подагента (глубже); итоги — в их ящик.
    nested: Option<Subagents<P>>,
}

/// Начало сессии обычного подагента.
struct Start {
    id: String,
    task: String,
    /// Начало подсказки и история; `None` — загрузить из базы по первой работе.
    state: Option<(Prefix, Vec<Message>)>,
    record: Option<Arc<dyn ChildRecord>>,
    /// Новый подагент: записать его в хранилище.
    fresh: bool,
    after: Option<AfterDone>,
    first: Option<Input>,
    status: Status,
    result: Option<String>,
    /// Вести ящика подагента, не взятые до перезапуска.
    pending: Vec<(String, Message)>,
}

struct Inner<P> {
    provider: Arc<P>,
    tools: Arc<Tools>,
    prefix: Prefix,
    spill_dir: std::path::PathBuf,
    inbox: Inbox,
    parent: Transcript,
    agents: Mutex<Vec<Agent<P>>>,
    /// Номер следующего подагента (`agent-<n>`) — один на всё дерево: id
    /// подагентов разных глубин не совпадают.
    next: Arc<std::sync::atomic::AtomicUsize>,
    store: std::sync::OnceLock<Arc<dyn SubagentStore>>,
    /// Монитор промежуточных событий (`SubagentMonitor`); без него —
    /// только итоги.
    monitor: std::sync::OnceLock<WatchFactory>,
    /// Глубина подагентов этого родителя (у корня — 1).
    depth: u32,
    /// Набор и начало подсказки подагентов следующей глубины; есть — подагенты
    /// этого родителя заводят своих.
    deeper: std::sync::OnceLock<(Arc<Tools>, Prefix)>,
}

/// Подагенты одного родителя. Инструменты `subagent.*` — из `tools()`.
pub struct Subagents<P> {
    inner: Arc<Inner<P>>,
}

impl<P> Clone for Subagents<P> {
    fn clone(&self) -> Self {
        Subagents { inner: self.inner.clone() }
    }
}

/// Первое сообщение подагента — `subagent_context_user_message.md` muse,
/// личный вариант. Подагент глубины 1 наследует разговор родителя: блок
/// родителя — `subagent_context_parent_background.md` и
/// `…_inheritance_inherited.md`; глубже — изолирован:
/// `…_inheritance_isolated.md`, без заметки о разговоре родителя (его нет).
/// Блоки глубины 1 (`…_depth_one_coordinator.md`,
/// `…_depth_one_spawn_inheritance.md`) — только у того, кто сам заводит
/// подагентов (имена блоков ядра; условие `can_spawn` — своё прочтение).
pub fn first_message(task: &str, depth: u32, can_spawn: bool) -> String {
    let one = depth == 1 && can_spawn;
    let block = |text: &str| if one { format!("{}\n", personal(text).trim_end()) } else { String::new() };
    let standard = personal(STANDARD_MD)
        .replace("{{DEPTH_ONE_COORDINATOR_BLOCK}}\n", &block(DEPTH_ONE_COORDINATOR_MD))
        .replace("{{DEPTH_ONE_SPAWN_INHERITANCE_BLOCK}}\n", &block(DEPTH_ONE_SPAWN_INHERITANCE_MD));
    let parent = if depth == 1 {
        format!("{}\n{}", personal(PARENT_BACKGROUND_MD).trim_end(), personal(INHERITED_MD).trim_end())
    } else {
        personal(ISOLATED_MD).trim_end().to_string()
    };
    personal(USER_MESSAGE_MD)
        .replace("{{CHILD_DEPTH}}", &depth.to_string())
        .replace("{{MAX_SPAWN_DEPTH}}", &MAX_SPAWN_DEPTH.to_string())
        .replace("{{PARENT_CONTEXT_BLOCK}}", &parent)
        .replace("{{STANDARD_SUBAGENT_BLOCK}}", &standard)
        .replace("{{PERSISTENT_SESSION_BLOCK}}", &personal(PERSISTENT_SESSION_MD))
        .replace("{{SESSION_LINES}}\n", "")
        .replace("{{METADATA_BLOCK}}\n", "")
        .replace("{{TASK_INPUT_START}}", task)
}

/// Начало подсказки подагента, который заводит своих: к системному тексту —
/// раздел `subagent_spawning.md`, к описаниям — `subagent.*`.
pub fn spawning_prefix(mut prefix: Prefix) -> Prefix {
    prefix.system = format!("{}\n\n{}", prefix.system.trim_end(), personal(SPAWNING_MD).trim_end());
    prefix.tools.extend([Op::Spawn, Op::Send, Op::List, Op::Close, Op::Resume].into_iter().map(spec));
    prefix
}

/// Итог подагента для родителя. Формы заметки muse в разборе нет — своя.
fn handoff(id: &str, status: Status, text: &str) -> Message {
    Message {
        role: Role::Developer,
        content: vec![Part::Text {
            text: format!("[Subagent Result] agent_id: {id}, status: {}\n\n{text}", status.as_str()),
        }],
    }
}

/// После прерванного хода у вызовов без результата появляется результат с
/// ошибкой — история остаётся годной для поставщика.
fn close_dangling_calls(history: &mut Vec<Message>) {
    let Some(last) = history.last() else { return };
    if last.role != Role::Assistant {
        return;
    }
    let results: Vec<Part> = last
        .content
        .iter()
        .filter_map(|p| match p {
            Part::ToolCall { id, .. } => Some(Part::ToolResult {
                call_id: id.clone(),
                content: vec![ToolResultPart::Text { text: "interrupted before the tool finished".into() }],
                is_error: true,
            }),
            _ => None,
        })
        .collect();
    if !results.is_empty() {
        history.push(Message { role: Role::User, content: results });
    }
}

impl<P: Provider + Send + Sync + 'static> Subagents<P> {
    /// `prefix` и `tools` — неизменное начало и набор инструментов подагента;
    /// `parent` — снимок истории родителя (его же отдать ходу родителя,
    /// `Turn::transcript`).
    pub fn new(
        provider: Arc<P>,
        tools: Arc<Tools>,
        prefix: Prefix,
        spill_dir: impl Into<std::path::PathBuf>,
        inbox: Inbox,
        parent: Transcript,
    ) -> Self {
        Subagents {
            inner: Arc::new(Inner {
                provider,
                tools,
                prefix,
                spill_dir: spill_dir.into(),
                inbox,
                parent,
                agents: Mutex::new(Vec::new()),
                next: Arc::new(std::sync::atomic::AtomicUsize::new(1)),
                store: std::sync::OnceLock::new(),
                monitor: std::sync::OnceLock::new(),
                depth: 1,
                deeper: std::sync::OnceLock::new(),
            }),
        }
    }

    pub fn inbox(&self) -> &Inbox {
        &self.inner.inbox
    }

    /// Включить монитор (модель `SubagentMonitor`).
    pub fn set_monitor<M: Provider + Send + Sync + 'static>(&self, m: Arc<crate::monitor::Monitor<M>>) {
        let f: WatchFactory = Arc::new(move |id, task, inbox, running| m.watch(id, task, inbox, running));
        let _ = self.inner.monitor.set(f);
    }

    /// Подагенты этого родителя заводят своих — на глубину ниже, с набором
    /// `tools` и началом `prefix` (роль следующей глубины). Начало самих
    /// подагентов должно уже нести `subagent.*` (`spawning_prefix`).
    pub fn set_deeper(&self, tools: Arc<Tools>, prefix: Prefix) {
        let _ = self.inner.deeper.set((tools, prefix));
    }

    /// Инструменты `subagent.spawn/send/list/close/resume` для родителя.
    pub fn tools(&self) -> Vec<SubagentTool<P>> {
        [Op::Spawn, Op::Send, Op::List, Op::Close, Op::Resume]
            .into_iter()
            .map(|op| SubagentTool { op, agents: self.clone() })
            .collect()
    }

    pub fn status(&self, id: &str) -> Option<Status> {
        self.inner.agents.lock().unwrap().iter().find(|a| a.id == id).map(|a| a.status)
    }

    fn set(inner: &Inner<P>, id: &str, status: Status, result: Option<String>) {
        if let Some(a) = inner.agents.lock().unwrap().iter_mut().find(|a| a.id == id) {
            if a.status == Status::Closed {
                return;
            }
            a.status = status;
            if result.is_some() {
                a.result = result;
            }
        }
    }

    pub fn spawn(&self, task: &str) -> String {
        self.spawn_then(task, None)
    }

    /// `spawn`, но итог перед отправкой родителю проходит через `after`.
    pub fn spawn_then(&self, task: &str, after: Option<AfterDone>) -> String {
        self.spawn_with(task, Vec::new(), after)
    }

    /// `spawn` с вложениями: `extra` (картинки, файлы из `items`) — в
    /// первом сообщении за его текстом.
    pub fn spawn_with(&self, task: &str, extra: Vec<Part>, after: Option<AfterDone>) -> String {
        let id = self.next_id();
        let mut first = vec![Part::Text { text: first_message(task, self.inner.depth, self.inner.deeper.get().is_some()) }];
        first.extend(extra);
        // Глубже 1 — изолированно: без разговора родителя.
        let mut history = if self.inner.depth == 1 { inherited_history(&self.inner.parent.snapshot()) } else { Vec::new() };
        if !history.is_empty() {
            history.push(Message { role: Role::Developer, content: vec![Part::Text { text: personal(ACTIVE_TURN_MD).trim_end().to_string() }] });
        }
        let agent = self.start(Start {
            id: id.clone(),
            task: task.to_string(),
            state: Some((self.inner.prefix.clone(), history)),
            record: None,
            fresh: true,
            after,
            first: Some(Input::Message(first)),
            status: Status::Running,
            result: None,
            pending: Vec::new(),
        });
        self.inner.agents.lock().unwrap().push(agent);
        id
    }

    fn next_id(&self) -> String {
        format!("agent-{}", self.inner.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }

    fn watch(&self, id: &str, task: &str) -> Option<crate::turn::StepHook> {
        self.inner.monitor.get().map(|f| {
            let me = Arc::downgrade(&self.inner);
            let watched = id.to_string();
            let running: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(move || {
                me.upgrade()
                    .is_some_and(|i| i.agents.lock().unwrap().iter().any(|a| a.id == watched && a.status == Status::Running))
            });
            f(id.to_string(), task.to_string(), self.inner.inbox.clone(), running)
        })
    }

    /// Сессия обычного подагента: очередь работы, ход за ходом; с
    /// хранилищем — история, точка перезапуска и состояние в базе.
    /// Подагенты одного подагента — следующая глубина; `None`, если глубже
    /// заводить нельзя.
    fn nested(&self, inbox: Inbox) -> Option<Subagents<P>> {
        let (tools, prefix) = self.inner.deeper.get()?.clone();
        Some(Subagents {
            inner: Arc::new(Inner {
                provider: self.inner.provider.clone(),
                tools,
                prefix,
                spill_dir: self.inner.spill_dir.clone(),
                inbox,
                parent: Transcript::new(),
                agents: Mutex::new(Vec::new()),
                next: self.inner.next.clone(),
                store: std::sync::OnceLock::new(),
                monitor: std::sync::OnceLock::new(),
                depth: self.inner.depth + 1,
                deeper: std::sync::OnceLock::new(),
            }),
        })
    }

    fn start(&self, s: Start) -> Agent<P> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let interrupt = Arc::new(Notify::new());
        if let Some(first) = s.first {
            let _ = tx.send(first);
        }
        let inner = self.inner.clone();
        let stop = interrupt.clone();
        let agent_id = s.id.clone();
        let watch = self.watch(&s.id, &s.task);
        let (task, after, fresh) = (s.task.clone(), s.after, s.fresh);
        let (mut state, mut record) = (s.state, s.record);
        // Свои подагенты: их инструменты — к общему набору, их итоги — в свой
        // ящик, который ход забирает между шагами. С хранилищем ящик — в
        // `agent.message_mailbox` под id подагента, как у корня.
        let inbox = match self.inner.store.get() {
            Some(st) => Inbox::with(st.mailbox(&s.id), s.pending),
            None => Inbox::new(),
        };
        let nested = self.nested(inbox);
        let tools = match &nested {
            Some(n) => {
                let mut t = (*self.inner.tools).clone();
                for x in n.tools() {
                    t.add(x);
                }
                Arc::new(t)
            }
            None => self.inner.tools.clone(),
        };
        if let (Some(n), Some(st)) = (&nested, self.inner.store.get()) {
            let _ = n.inner.store.set(st.nested(&s.id));
        }
        let own = nested.clone();
        let handle = tokio::spawn(async move {
            let store = inner.store.get().cloned();
            if let (true, Some(st)) = (fresh, &store) {
                match st.spawned(&agent_id, &task, &inner.prefix, after.is_some()).await {
                    Ok(r) => record = Some(r),
                    Err(e) => eprintln!("subagent store: {e}"),
                }
            }
            let mut turns = 0u64;
            loop {
                // Работа — новая задача родителя или итог своего подагента в
                // ящике (ход забирает его сам).
                let input = match &own {
                    Some(n) if rx.is_empty() && !n.inner.inbox.is_empty() => Input::Resume,
                    // Сначала очередь работы: иначе продолжение после
                    // перезапуска осталось бы в ней и дало лишний ход.
                    Some(n) => tokio::select! {
                        biased;
                        i = rx.recv() => match i { Some(i) => i, None => break },
                        _ = n.inner.inbox.wait() => Input::Resume,
                    },
                    None => match rx.recv().await { Some(i) => i, None => break },
                };
                // Законченный до перезапуска — история из базы по первой работе.
                if state.is_none() {
                    let loaded = match &record {
                        Some(r) => r.load().await.unwrap_or_else(|e| {
                            eprintln!("subagent history: {e}");
                            (inner.prefix.clone(), Vec::new())
                        }),
                        None => (inner.prefix.clone(), Vec::new()),
                    };
                    state = Some(loaded);
                }
                let (prefix, history) = state.as_mut().unwrap();
                if let Input::Message(content) = input {
                    history.push(Message { role: Role::User, content });
                }
                Self::set(&inner, &agent_id, Status::Running, None);
                if let Some(st) = &store {
                    if let Err(e) = st.status(&agent_id, Status::Running, None).await {
                        eprintln!("subagent store: {e}");
                    }
                }
                turns += 1;
                // Имя хода. Своё: `<id>:turn:<n>`.
                let turn_id = format!("{agent_id}:turn:{turns}");
                if let Some(r) = &record {
                    if let Err(e) = r.checkpoint(None, None, Some(&turn_id)).await {
                        eprintln!("subagent checkpoint: {e}");
                    }
                }
                let mut turn = Turn::from_prefix(&*inner.provider, &tools, prefix, inner.spill_dir.clone());
                turn.on_step = watch.clone();
                turn.inbox = own.as_ref().map(|n| n.inner.inbox.clone());
                turn.turn_id = Some(turn_id.clone());
                turn.record = record.clone().map(|r| r as Arc<dyn crate::turn::Record>);
                let mut quiet = |_: &str| {};
                let outcome = tokio::select! {
                    r = turn.run(history, &mut quiet) => Some(r),
                    _ = stop.notified() => None,
                };
                // Вести ящика, взятые ходом, закрываются по его итогу (как у
                // корня); прерванный ход их уже записал в историю.
                if let Some(n) = &own {
                    if let Err(e) = n.inner.inbox.complete(&turn_id, !matches!(outcome, Some(Err(_)))).await {
                        eprintln!("mailbox complete: {e}");
                    }
                }
                let Some(outcome) = outcome else {
                    // Прервано `send` с `interrupt`: новая работа уже в очереди.
                    close_dangling_calls(history);
                    continue;
                };
                if !rx.is_empty() {
                    // Пришла новая работа — итог отдаётся после неё.
                    continue;
                }
                let (status, text) = match outcome {
                    Ok(resp) => (Status::Done, resp.text()),
                    Err(TurnError::Model(e)) if e.retryable => (Status::Interrupted, e.to_string()),
                    Err(e) => (Status::Failed, e.to_string()),
                };
                let text = match &after {
                    Some(f) => f(status, text).await,
                    None => text,
                };
                if let Some(r) = &record {
                    if let Err(e) = r.checkpoint(None, None, None).await {
                        eprintln!("subagent checkpoint: {e}");
                    }
                }
                Self::set(&inner, &agent_id, status, Some(text.clone()));
                if let Some(st) = &store {
                    if let Err(e) = st.status(&agent_id, status, Some(&text)).await {
                        eprintln!("subagent store: {e}");
                    }
                }
                inner.inbox.push(handoff(&agent_id, status, &text)).await;
            }
        });
        Agent { id: s.id, task: s.task, status: s.status, result: s.result, tx, interrupt, handle, finish: None, nested }
    }

    /// Подагенты из базы (после перезапуска): законченные — снова в списке;
    /// с точкой `before_inference` — ход продолжается; прочие незаконченные
    /// — `errored`, родителю итог с текстом ядра. Дальше состояние пишется
    /// в `store`. Возвращает id продолживших.
    pub fn persistent(&self, store: Arc<dyn SubagentStore>) -> futures_util::future::BoxFuture<'_, Result<Vec<String>, String>> {
        Box::pin(async move {
            let saved = store.saved().await?;
            let _ = self.inner.store.set(store.clone());
            let mut resumed = vec![];
            for sv in saved {
                if let Some(n) = sv.id.strip_prefix("agent-").and_then(|n| n.parse::<usize>().ok()) {
                    self.inner.next.fetch_max(n + 1, std::sync::atomic::Ordering::SeqCst);
                }
                let record = store.child(&sv.id).await?;
                let mut start = Start {
                    id: sv.id.clone(),
                    task: sv.task.clone(),
                    state: None,
                    record: Some(record.clone()),
                    fresh: false,
                    after: None,
                    first: None,
                    status: sv.status,
                    result: sv.result.clone(),
                    // Ящик подагента: итоги его подагентов, не взятые до
                    // перезапуска, — снова в очереди.
                    pending: match self.inner.deeper.get() {
                        Some(_) => store.mailbox(&sv.id).pending().await?,
                        None => Vec::new(),
                    },
                };
                if !sv.completed {
                    if !sv.finish_hook && record.interrupted().await?.is_some() {
                        let (prefix, mut history) = record.load().await?;
                        close_interrupted(&mut history);
                        record.record(&history).await?;
                        start.state = Some((prefix, history));
                        start.first = Some(Input::Resume);
                        start.status = Status::Running;
                        resumed.push(sv.id.clone());
                    } else {
                        store.status(&sv.id, Status::Failed, Some(WORK_NOT_RESUMABLE)).await?;
                        self.inner.inbox.push(handoff(&sv.id, Status::Failed, WORK_NOT_RESUMABLE)).await;
                        start.status = Status::Failed;
                        start.result = Some(WORK_NOT_RESUMABLE.to_string());
                    }
                }
                let agent = self.start(start);
                // Дерево: подагенты подагента — из базы под ним.
                let nested = agent.nested.clone();
                self.inner.agents.lock().unwrap().push(agent);
                if let Some(n) = nested {
                    resumed.extend(n.persistent(store.nested(&sv.id)).await?);
                }
            }
            Ok(resumed)
        })
    }

    /// Запустить особого подагента (`Kind`); дополнения — тем же
    /// `subagent.send`, остановка — `subagent.close`.
    pub fn spawn_kind(&self, kind: Kind) -> String {
        let id = self.next_id();
        let mut agents = self.inner.agents.lock().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let interrupt = Arc::new(Notify::new());
        let _ = tx.send(if kind.resume.is_some() { Input::Resume } else { Input::Message(vec![Part::Text { text: kind.first.clone() }]) });
        let inner = self.inner.clone();
        let stop = interrupt.clone();
        let agent_id = id.clone();
        let finish = kind.finish.clone();
        let started = tokio::time::Instant::now();
        let again = tx.clone();
        let mut kind = kind;
        let start_history = kind.resume.take().unwrap_or_default();
        let handle = tokio::spawn(async move {
            let mut history: Vec<Message> = start_history;
            let mut nudged = false;
            let mut turns = 0u64;
            let rec = kind.record.clone();
            while let Some(input) = rx.recv().await {
                if let Input::Message(content) = input {
                    history.push(Message { role: Role::User, content });
                }
                Self::set(&inner, &agent_id, Status::Running, None);
                turns += 1;
                if let Some(r) = &rec {
                    // Имя хода. Своё: `<владелец>:turn:<n>`.
                    if let Err(e) = r.record.checkpoint(Some(r.class), Some(&r.owner), Some(&format!("{}:turn:{turns}", r.owner))).await {
                        eprintln!("subagent checkpoint: {e}");
                    }
                }
                let mut turn = Turn::from_prefix(&*inner.provider, &kind.tools, &kind.prefix, inner.spill_dir.clone());
                turn.terminal = kind.terminal.clone();
                turn.record = rec.as_ref().map(|r| r.record.clone() as Arc<dyn crate::turn::Record>);
                let mut quiet = |_: &str| {};
                let left = kind.deadline.map(|d| d.saturating_sub(started.elapsed()));
                let outcome = tokio::select! {
                    r = turn.run(&mut history, &mut quiet) => Some(Some(r)),
                    _ = stop.notified() => None,
                    _ = async { match left { Some(l) => tokio::time::sleep(l).await, None => std::future::pending().await } } => Some(None),
                };
                let Some(outcome) = outcome else {
                    close_dangling_calls(&mut history);
                    continue;
                };
                if !rx.is_empty() && outcome.is_some() {
                    continue;
                }
                let (status, text) = match outcome {
                    None => (Status::Failed, "deadline reached before the terminal call".to_string()),
                    Some(Ok(resp)) if (kind.done)() => (Status::Done, resp.text()),
                    Some(Ok(_)) if !nudged && kind.nudge.is_some() => {
                        nudged = true;
                        let _ = again.send(Input::Message(vec![Part::Text { text: kind.nudge.clone().unwrap_or_default() }]));
                        continue;
                    }
                    Some(Ok(resp)) => (Status::Failed, resp.text()),
                    Some(Err(TurnError::Model(e))) if e.retryable => (Status::Interrupted, e.to_string()),
                    Some(Err(e)) => (Status::Failed, e.to_string()),
                };
                if let Some(r) = &rec {
                    if let Err(e) = r.record.checkpoint(Some(r.class), Some(&r.owner), None).await {
                        eprintln!("subagent checkpoint: {e}");
                    }
                }
                let note = (kind.finish)(status, text.clone()).await;
                Self::set(&inner, &agent_id, status, Some(text));
                inner.inbox.push(note).await;
                if status != Status::Interrupted {
                    break;
                }
            }
        });
        agents.push(Agent { id: id.clone(), task: kind.task, status: Status::Running, result: None, tx, interrupt, handle, finish: Some(finish), nested: None });
        id
    }

    pub fn send(&self, id: &str, message: &str, interrupt: bool) -> Result<(), String> {
        self.send_in(id, vec![Part::Text { text: message.to_string() }], interrupt, false)
    }

    /// `send` с `scope`: `descendants` — цель ищется во всём поддереве.
    pub fn send_in(&self, id: &str, content: Vec<Part>, interrupt: bool, descendants: bool) -> Result<(), String> {
        let agents = self.inner.agents.lock().unwrap();
        if let Some(a) = agents.iter().find(|a| a.id == id) {
            if a.status == Status::Closed {
                return Err(format!("agent {id} is closed"));
            }
            a.tx.send(Input::Message(content)).map_err(|_| format!("agent {id} is gone"))?;
            if interrupt {
                a.interrupt.notify_one();
            }
            return Ok(());
        }
        if descendants {
            for n in agents.iter().filter_map(|a| a.nested.as_ref()) {
                match n.send_in(id, content.clone(), interrupt, true) {
                    Err(e) if e == unknown(id) => {}
                    r => return r,
                }
            }
        }
        Err(unknown(id))
    }

    pub fn resume(&self, id: &str) -> Result<(), String> {
        let agents = self.inner.agents.lock().unwrap();
        let a = agents.iter().find(|a| a.id == id).ok_or_else(|| unknown(id))?;
        if a.status != Status::Interrupted {
            return Err(format!(
                "agent {id} is {}: only a paused or interrupted agent can be resumed; spawn a new agent with `subagent.spawn` to continue that work",
                a.status.as_str()
            ));
        }
        a.tx.send(Input::Resume).map_err(|_| format!("agent {id} is gone"))
    }

    /// Остановить подагента и дождаться конца его работы — не дольше срока
    /// команды.
    pub async fn close(&self, id: &str) -> Result<(), String> {
        let (handle, finish, nested) = {
            let mut agents = self.inner.agents.lock().unwrap();
            let a = agents.iter_mut().find(|a| a.id == id).ok_or_else(|| unknown(id))?;
            let was_running = a.status == Status::Running;
            a.status = Status::Closed;
            a.handle.abort();
            let finish = if was_running { a.finish.clone() } else { None };
            (std::mem::replace(&mut a.handle, tokio::spawn(async {})), finish, a.nested.clone())
        };
        // Работа подагента — и работа его подагентов: они тоже
        // останавливаются (своё: в разборе не видно).
        if let Some(n) = nested {
            n.shutdown().await;
        }
        if let Some(f) = finish {
            let note = f(Status::Closed, "stopped by subagent.close".into()).await;
            self.inner.inbox.push(note).await;
        }
        if let Some(st) = self.inner.store.get() {
            if let Err(e) = st.status(id, Status::Closed, None).await {
                eprintln!("subagent store: {e}");
            }
        }
        match tokio::time::timeout(CONTROL_TIMEOUT, handle).await {
            Ok(_) => Ok(()),
            Err(_) => Err(format!("closing agent {id} did not finish within {CONTROL_TIMEOUT:?}")),
        }
    }

    /// Остановить всех подагентов этого родителя и их поддеревья.
    fn shutdown(&self) -> futures_util::future::BoxFuture<'_, ()> {
        Box::pin(async move {
            let stopped: Vec<(String, Option<Subagents<P>>)> = {
                let mut agents = self.inner.agents.lock().unwrap();
                agents
                    .iter_mut()
                    .filter(|a| a.status != Status::Closed)
                    .map(|a| {
                        a.status = Status::Closed;
                        a.handle.abort();
                        (a.id.clone(), a.nested.clone())
                    })
                    .collect()
            };
            for (id, nested) in stopped {
                if let Some(n) = nested {
                    n.shutdown().await;
                }
                if let Some(st) = self.inner.store.get() {
                    if let Err(e) = st.status(&id, Status::Closed, None).await {
                        eprintln!("subagent store: {e}");
                    }
                }
            }
        })
    }

    /// Сколько подагентов сейчас работает.
    pub fn running(&self) -> usize {
        self.inner.agents.lock().unwrap().iter().filter(|a| a.status == Status::Running).count()
    }

    pub fn list(&self, limit: usize) -> Value {
        self.list_in(limit, false)
    }

    /// `list` с `scope`: `descendants` — всё поддерево; у записи поддерева
    /// — `parent_agent_id` (своё имя поля, по столбцу `subagent_spawns`).
    pub fn list_in(&self, limit: usize, descendants: bool) -> Value {
        let mut entries = vec![];
        self.collect(descendants, None, &mut entries);
        let (active, recent): (Vec<_>, Vec<_>) = entries.into_iter().partition(|(running, _)| *running);
        json!({
            "active": active.into_iter().take(limit).map(|e| e.1).collect::<Vec<_>>(),
            "recent": recent.into_iter().take(limit).map(|e| e.1).collect::<Vec<_>>(),
        })
    }

    fn collect(&self, descendants: bool, parent: Option<&str>, out: &mut Vec<(bool, Value)>) {
        let agents = self.inner.agents.lock().unwrap();
        for a in agents.iter() {
            let mut e = json!({
                "agent_id": a.id,
                "status": a.status.as_str(),
                "task": preview(&a.task),
                "result": a.result.as_deref().map(preview),
            });
            if let Some(p) = parent {
                e["parent_agent_id"] = p.into();
            }
            out.push((a.status == Status::Running, e));
            if let (true, Some(n)) = (descendants, &a.nested) {
                n.collect(true, Some(&a.id), out);
            }
        }
    }
}

fn unknown(id: &str) -> String {
    format!("no child agent with id `{id}`")
}

fn preview(s: &str) -> String {
    const MAX: usize = 200;
    match s.char_indices().nth(MAX) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

#[derive(Clone, Copy)]
enum Op {
    Spawn,
    Send,
    List,
    Close,
    Resume,
}

/// Инструмент `subagent.*`.
pub struct SubagentTool<P> {
    op: Op,
    agents: Subagents<P>,
}

fn prop(d: &str, t: &str) -> Value {
    json!({"type": t, "description": d})
}

/// Схема `items`: описание — строка ядра, поля — названные в нём.
fn items(d: &str) -> Value {
    json!({
        "type": "array",
        "description": d,
        "items": {
            "type": "object",
            "properties": {
                "type": {"type": "string", "enum": ["text", "image", "file"]},
                "text": {"type": "string"},
                "path": {"type": "string"},
                "filename": {"type": "string"},
                "caption": {"type": "string"},
            },
            "required": ["type"],
        },
    })
}

/// Элемент `items`: текст или прочитанный файл.
enum Item {
    Text(String),
    File(Vec<Part>),
}

/// Вход `items` по порядку. `image` и `file` читаются инструментом
/// `hamlet.read` набора подагента («a file path as `read` takes it»): его
/// результат — вложение, перед ним — строка с `filename` (или путём) и
/// `caption`. Ошибка — код ядра `reason` (`invalid_spawn_input` у spawn).
async fn read_items(tools: &Tools, list: &[Value], reason: &str) -> Result<Vec<Item>, ToolError> {
    let bad = |m: String| ToolError::InvalidArguments(json!({"reason": reason, "error": m}).to_string());
    let read = tools.get("hamlet.read").cloned();
    let mut out = Vec::new();
    for (i, item) in list.iter().enumerate() {
        let field = |k: &str| item.get(k).and_then(Value::as_str);
        match field("type") {
            Some("text") => out.push(Item::Text(field("text").ok_or_else(|| bad(format!("items[{i}]: type \"text\" requires `text`")))?.to_string())),
            Some(kind @ ("image" | "file")) => {
                let path = field("path").ok_or_else(|| bad(format!("items[{i}]: type \"{kind}\" requires `path`")))?;
                let read = read.as_ref().ok_or_else(|| bad("reading files is not available here".into()))?;
                let got = read.call(json!({"path": path})).await.map_err(|e| bad(format!("items[{i}]: {e}")))?;
                if kind == "image" && !got.iter().any(|p| matches!(p, ToolResultPart::Image(_))) {
                    return Err(bad(format!("items[{i}]: {path} is not a supported image")));
                }
                let mut label = field("filename").unwrap_or(path).to_string();
                if let Some(c) = field("caption") {
                    label = format!("{label}: {c}");
                }
                let mut parts = vec![Part::Text { text: label }];
                parts.extend(got.into_iter().map(|p| match p {
                    ToolResultPart::Text { text } => Part::Text { text },
                    ToolResultPart::Image(img) => Part::Image(img),
                }));
                out.push(Item::File(parts));
            }
            other => return Err(bad(format!("items[{i}]: unknown type {other:?}; expected \"text\", \"image\" or \"file\""))),
        }
    }
    Ok(out)
}

/// Описание инструмента `subagent.*`.
fn spec(op: Op) -> ToolSpec {
    let (name, description, props, required) = match op {
        Op::Spawn => ("subagent.spawn", SPAWN_DESCRIPTION, json!({"message": prop(SPAWN_MESSAGE, "string"), "items": items(SPAWN_ITEMS)}), json!([])),
        Op::Send => (
            "subagent.send",
            SEND_DESCRIPTION,
            json!({
                "agent_id": prop(TARGET_ID, "string"),
                "message": prop(SEND_MESSAGE, "string"),
                "items": items(SEND_ITEMS),
                "scope": {"type": "string", "enum": ["direct", "descendants"], "description": SEND_SCOPE},
                "interrupt": prop(SEND_INTERRUPT, "boolean"),
            }),
            json!(["agent_id"]),
        ),
        Op::List => (
            "subagent.list",
            LIST_DESCRIPTION,
            json!({
                "scope": {"type": "string", "enum": ["direct", "descendants"], "description": LIST_SCOPE},
                "limit": prop(LIST_LIMIT, "integer"),
            }),
            json!([]),
        ),
        Op::Close => ("subagent.close", CLOSE_DESCRIPTION, json!({"agent_id": prop(TARGET_ID, "string")}), json!(["agent_id"])),
        Op::Resume => ("subagent.resume", RESUME_DESCRIPTION, json!({"agent_id": prop(TARGET_ID, "string")}), json!(["agent_id"])),
    };
    let mut props = props;
    for p in props.as_object_mut().unwrap().values_mut() {
        if let Some(d) = p.get("description").and_then(Value::as_str) {
            p["description"] = personal(d).into();
        }
    }
    ToolSpec {
        name: name.into(),
        description: personal(description),
        input_schema: json!({"type": "object", "properties": props, "required": required}),
    }
}

impl<P: Provider + Send + Sync + 'static> Tool for SubagentTool<P> {
    fn spec(&self) -> ToolSpec {
        spec(self.op)
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let s = |k: &str| args.get(k).and_then(Value::as_str);
            let need = |k: &str| s(k).ok_or_else(|| ToolError::InvalidArguments(format!("`{k}` (string) is required")));
            let text = |v: Value| Ok(vec![ToolResultPart::Text { text: v.to_string() }]);
            match self.op {
                Op::Spawn => {
                    // `items` важнее `message`.
                    let id = match args.get("items").and_then(Value::as_array).filter(|l| !l.is_empty()) {
                        Some(list) => {
                            // Текст — в шаблон первого сообщения (через пустую
                            // строку), файлы — за ним.
                            let (mut texts, mut extra) = (Vec::new(), Vec::new());
                            for it in read_items(&self.agents.inner.tools, list, "invalid_spawn_input").await? {
                                match it {
                                    Item::Text(t) => texts.push(t),
                                    Item::File(p) => extra.extend(p),
                                }
                            }
                            self.agents.spawn_with(&texts.join("\n\n"), extra, None)
                        }
                        None => self.agents.spawn(need("message")?),
                    };
                    text(json!({"agent_id": id, "status": "running"}))
                }
                Op::Send => {
                    let interrupt = args.get("interrupt").and_then(Value::as_bool).unwrap_or(false);
                    let id = need("agent_id")?;
                    let descendants = s("scope") == Some("descendants");
                    let content = match args.get("items").and_then(Value::as_array).filter(|l| !l.is_empty()) {
                        Some(list) => {
                            let mut content = Vec::new();
                            for it in read_items(&self.agents.inner.tools, list, "input_error").await? {
                                match it {
                                    Item::Text(text) => content.push(Part::Text { text }),
                                    Item::File(p) => content.extend(p),
                                }
                            }
                            content
                        }
                        None => vec![Part::Text { text: need("message")?.to_string() }],
                    };
                    self.agents.send_in(id, content, interrupt, descendants).map_err(ToolError::Failed)?;
                    text(json!({"agent_id": id, "submitted": true}))
                }
                Op::List => {
                    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20).clamp(1, 200) as usize;
                    text(self.agents.list_in(limit, s("scope") == Some("descendants")))
                }
                Op::Close => {
                    let id = need("agent_id")?;
                    self.agents.close(id).await.map_err(ToolError::Failed)?;
                    text(json!({"agent_id": id, "status": "closed"}))
                }
                Op::Resume => {
                    let id = need("agent_id")?;
                    self.agents.resume(id).map_err(ToolError::Failed)?;
                    text(json!({"agent_id": id, "status": "running"}))
                }
            }
        })
    }
}
