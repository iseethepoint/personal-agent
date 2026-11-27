//! Проверка ответа — как у muse (`message_execution › response_verification`,
//! `hatch-server/src/daemon/runtime/response_verification.rs`): перед отдачей
//! ответа главного агента классификатор (назначение
//! `ResponseVerificationClassifier`, место `request_verification_classifier`,
//! инструмент `submit_request_verification`, текст
//! `prompts/verification/classifier.md`) относит просьбу к `health`,
//! `bookings` или `none`. При категории черновик не отдаётся: проверяющий
//! (место `verifier`: чтение файлов и веб, `nothing_to_do`,
//! `notify_main_agent`, `verifier.not_relevant`) — прогон `verification` в
//! `runtime.execute_resolve_runs` с фазами исполнения (`execute_task.md`,
//! `{category_guidance}` — `health.md` и `health_relevance.md` или
//! `bookings.md`, `{evidence_instructions}` — `evidence.md` с
//! `review_scope.md`) и итога (`resolve_note.md`). `notify_main_agent` —
//! исправленный черновик, его главный агент получает заметкой
//! `feedback.md` («[SCHEDULED TASK HANDOFF] job_id: response_verification»)
//! и пишет окончательный ответ; `nothing_to_do` и `verifier.not_relevant` —
//! отдаётся исходный черновик.
//!
//! Своё (в разборе не видно): вход классификатора — недавний разговор в
//! `<recent_conversation>` и просьба в `<request>`; разделы задания
//! проверяющего — `<verification-task>`, `<verification-draft>`,
//! `<verification-trajectory>` (имена — строки ядра); ход проверяющего —
//! вызовы этого хода с первой строкой результата; системный текст
//! проверяющего — системный текст агента; проверка идёт внутри хода, а не
//! фоновым прогоном; `run_id` — `verification:<мс>:<n>`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools, Turn};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use serde_json::{json, Value};

use crate::store::Store;
use crate::worker::{self, Signal};

const CLASSIFIER_MD: &str = include_str!("../../../prompts/verification/classifier.md");
const EXECUTE_MD: &str = include_str!("../../../prompts/verification/execute_task.md");
const EVIDENCE_MD: &str = include_str!("../../../prompts/verification/evidence.md");
const REVIEW_SCOPE_MD: &str = include_str!("../../../prompts/verification/review_scope.md");
const RESOLVE_MD: &str = include_str!("../../../prompts/verification/resolve_note.md");
const HEALTH_MD: &str = include_str!("../../../prompts/verification/health.md");
const HEALTH_RELEVANCE_MD: &str = include_str!("../../../prompts/verification/health_relevance.md");
const BOOKINGS_MD: &str = include_str!("../../../prompts/verification/bookings.md");
const NOT_RELEVANT_MD: &str = include_str!("../../../prompts/verification/not_relevant.md");
const FEEDBACK_MD: &str = include_str!("../../../prompts/verification/feedback.md");

pub const SUBMIT: &str = "hamlet.submit_request_verification";
pub const NOT_RELEVANT: &str = "verifier.not_relevant";
/// Сколько недавних реплик видит классификатор. Своё: в разборе не видно.
pub const RECENT_MESSAGES: usize = 10;

/// Категория проверки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Health,
    Bookings,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Health => "health",
            Category::Bookings => "bookings",
        }
    }
}

/// Итог проверки.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Исправленный черновик — заметка главному агенту (`feedback`).
    Revised(String),
    /// Черновик отдаётся как есть.
    Original,
}

/// Заметка главному агенту с исправленным черновиком.
pub fn feedback(revised: &str) -> String {
    personal(FEEDBACK_MD).replace("{worker_message}", revised).trim_end().to_string()
}

struct Submit(Arc<Mutex<Option<String>>>);

impl Tool for Submit {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: SUBMIT.into(),
            description: "Submit whether the supplied request needs factual verification.".into(),
            input_schema: json!({
                "type": "object",
                "description": "Submit the request verification category.",
                "properties": {"category": {"type": "string", "enum": ["health", "bookings", "none"]}},
                "required": ["category"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let c = args.get("category").and_then(Value::as_str).unwrap_or_default();
            if !["health", "bookings", "none"].contains(&c) {
                return Err(ToolError::InvalidArguments("category must be one of health|bookings|none".into()));
            }
            *self.0.lock().unwrap() = Some(c.to_string());
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

/// Отнести просьбу к категории. `recent` — недавние реплики (говорящий,
/// текст) до просьбы. Сбой модели — без проверки.
pub async fn classify<P: Provider>(provider: &P, request: &str, recent: &[(String, String)], spill: &std::path::Path) -> Option<Category> {
    let slot = Arc::new(Mutex::new(None));
    let mut tools = Tools::new();
    tools.add(Submit(slot.clone()));
    let lines: Vec<String> = recent.iter().map(|(who, text)| format!("{who}: {text}")).collect();
    let user = format!("<recent_conversation>\n{}\n</recent_conversation>\n\n<request>\n{request}\n</request>", lines.join("\n\n"));
    let mut history = vec![Message { role: Role::User, content: vec![Part::Text { text: user }] }];
    let mut turn = Turn::new(provider, &tools, personal(CLASSIFIER_MD).trim_end().to_string(), spill);
    turn.terminal = vec![SUBMIT.into()];
    if let Err(e) = turn.run(&mut history, &mut |_| {}).await {
        eprintln!("request verification classifier failed; delivering without verification: {e}");
        return None;
    }
    let got = slot.lock().unwrap().clone();
    match got.as_deref() {
        Some("health") => Some(Category::Health),
        Some("bookings") => Some(Category::Bookings),
        _ => None,
    }
}

struct NotRelevant(Arc<Mutex<bool>>);

impl Tool for NotRelevant {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NOT_RELEVANT.into(),
            description: NOT_RELEVANT_MD.trim_end().into(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        }
    }

    fn call(&self, _args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            *self.0.lock().unwrap() = true;
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

/// Задание проверяющему.
pub fn execute_message(category: Category, task: &str, draft: &str, trajectory: &str) -> String {
    let guidance = match category {
        Category::Health => format!("{}\n\n{}", HEALTH_MD.trim_end(), HEALTH_RELEVANCE_MD.trim_end()),
        Category::Bookings => BOOKINGS_MD.trim_end().to_string(),
    };
    let evidence = EVIDENCE_MD.trim_end().replace("{review_scope}", REVIEW_SCOPE_MD.trim_end());
    let body = EXECUTE_MD.trim_end().replace("{category_guidance}", &guidance).replace("{evidence_instructions}", &evidence);
    format!("<verification-task>\n{task}\n</verification-task>\n\n<verification-draft>\n{draft}\n</verification-draft>\n\n<verification-trajectory>\n{trajectory}\n</verification-trajectory>\n\n{body}")
}

/// Проверяющий ответов.
pub struct Verifier<P> {
    pub store: Store,
    pub provider: Arc<P>,
    /// Набор проверяющего: чтение файлов и веб.
    pub tools: Arc<dyn Fn() -> Tools + Send + Sync>,
    pub system: String,
    pub spill_dir: PathBuf,
    pub now: Arc<dyn Fn() -> i64 + Send + Sync>,
}

static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl<P: Provider + Send + Sync + 'static> Verifier<P> {
    /// Проверить черновик; сбой — черновик как есть («Verification did not
    /// finish.» ядра — в журнал).
    pub async fn verify(&self, category: Category, task: &str, draft: &str, trajectory: &str) -> Verdict {
        let run_id = format!("verification:{}:{}", (self.now)(), RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        let meta = json!({"category": category.as_str()});
        if let Err(e) = self.store.run_insert("verification", &run_id, &run_id, "verification", &meta).await {
            eprintln!("Verification did not finish. {e}");
            return Verdict::Original;
        }
        let agent_id = format!("verifier:{run_id}");
        let _ = self.store.hook_run_claim(&run_id, &agent_id, &format!("{run_id}:execute"), &format!("{run_id}:resolve")).await;
        let stopped = Arc::new(Mutex::new(false));
        let mut tools = (self.tools)();
        let mut terminal = vec![];
        if category == Category::Health {
            tools.add(NotRelevant(stopped.clone()));
            terminal.push(NOT_RELEVANT.to_string());
        }
        let prefix = agent_loop::Prefix { system: self.system.clone(), tools: tools.upfront_specs() };
        let flag = stopped.clone();
        let kind = worker::Kind::Verify { resolve: personal(RESOLVE_MD).trim_end().to_string(), terminal, stop: Arc::new(move || *flag.lock().unwrap()) };
        let store = self.store.clone();
        let phase_run = run_id.clone();
        let on_phase = move |p: &'static str, _: bool| {
            let store = store.clone();
            let run = phase_run.clone();
            tokio::spawn(async move {
                let _ = store.hook_run_phase(&run, p).await;
            });
        };
        let first = execute_message(category, task, draft, trajectory);
        let out = worker::run(&*self.provider, &*self.provider, &prefix, &tools, &self.spill_dir, worker::Begin::Fresh(first), None, kind, on_phase, None).await;
        let (verdict, settled) = match out.signal {
            Some(Signal::Notify { message, .. }) => {
                let r = async {
                    self.store.hook_run_decide(&run_id, "notify_main_agent", Some(&message)).await?;
                    self.store.hook_run_notified(&run_id).await
                }
                .await;
                (Verdict::Revised(message), r)
            }
            Some(Signal::Quiet) => (Verdict::Original, self.store.hook_run_decide(&run_id, "nothing_to_do", None).await),
            None => {
                let err = out.error.unwrap_or_else(|| "verification completed without a response".into());
                eprintln!("Verification did not finish. {err}");
                (Verdict::Original, self.store.hook_run_fail(&run_id, "resolve", &err).await)
            }
        };
        if let Err(e) = settled {
            eprintln!("verification {run_id}: {e}");
        }
        verdict
    }
}
