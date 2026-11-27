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
//!
//! Отчёт расписания — как у muse (`hatch-db-scheduler/src/scheduler/
//! cron_verification.rs`, место `cron_verification_classifier` на модели
//! `main_chat`, инструмент `submit_cron_verification`, текст
//! `cron_classifier.md`): только успешный отчёт («cron report is not
//! successful»), категории `health`, `bookings`, `purchases` (`purchases.md`)
//! или `none`. Проверяющий тот же. Сбой проверки — к отчёту дописывается
//! `unverified_handoff.md` («Verification did not finish. Present
//! consequential factual claims in this report as unverified.»).
//!
//! Своё для отчётов: вход классификатора — `<task>` (тело задания) и
//! `<report>`; исправленный отчёт заменяет исходный в итоге прогона и в
//! заметке главному агенту; пометка о сбое — после отчёта через пустую
//! строку. Итог работника до проверки лежит в `pending_terminal_result_json`
//! прогона, категория — там же (`retain_cron_verification_category`); после
//! перезапуска проверка продолжается с сохранённой категорией
//! (`load_cron_verification_category`), классификатор снова не зовётся.

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
const CRON_CLASSIFIER_MD: &str = include_str!("../../../prompts/verification/cron_classifier.md");
const PURCHASES_MD: &str = include_str!("../../../prompts/verification/purchases.md");
const UNVERIFIED_MD: &str = include_str!("../../../prompts/verification/unverified_handoff.md");

pub const SUBMIT: &str = "hamlet.submit_request_verification";
pub const SUBMIT_CRON: &str = "hamlet.submit_cron_verification";
pub const NOT_RELEVANT: &str = "verifier.not_relevant";
/// Сколько недавних реплик видит классификатор. Своё: в разборе не видно.
pub const RECENT_MESSAGES: usize = 10;

/// Категория проверки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Health,
    Bookings,
    Purchases,
}

impl Category {
    pub fn parse(s: &str) -> Option<Category> {
        match s {
            "health" => Some(Category::Health),
            "bookings" => Some(Category::Bookings),
            "purchases" => Some(Category::Purchases),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Health => "health",
            Category::Bookings => "bookings",
            Category::Purchases => "purchases",
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
    /// Проверка не закончилась («Verification did not finish.»).
    Unfinished,
}

/// Заметка главному агенту с исправленным черновиком.
pub fn feedback(revised: &str) -> String {
    personal(FEEDBACK_MD).replace("{worker_message}", revised).trim_end().to_string()
}

/// Инструмент классификатора: одна категория из `choices`.
struct Submit {
    slot: Arc<Mutex<Option<String>>>,
    name: &'static str,
    description: &'static str,
    schema: &'static str,
    choices: &'static [&'static str],
}

impl Tool for Submit {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.into(),
            description: self.description.into(),
            input_schema: json!({
                "type": "object",
                "description": self.schema,
                "properties": {"category": {"type": "string", "enum": self.choices}},
                "required": ["category"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let c = args.get("category").and_then(Value::as_str).unwrap_or_default();
            if !self.choices.contains(&c) {
                return Err(ToolError::InvalidArguments(format!("category must be one of {}", self.choices.join("|"))));
            }
            *self.slot.lock().unwrap() = Some(c.to_string());
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

/// Ход классификатора: одна категория или `None` (`none`, сбой модели).
async fn submit_category<P: Provider>(provider: &P, tool: Submit, system: &str, user: String, spill: &std::path::Path, what: &str) -> Option<Category> {
    let slot = tool.slot.clone();
    let name = tool.name;
    let mut tools = Tools::new();
    tools.add(tool);
    let mut history = vec![Message { role: Role::User, content: vec![Part::Text { text: user }] }];
    let mut turn = Turn::new(provider, &tools, personal(system).trim_end().to_string(), spill);
    turn.terminal = vec![name.into()];
    if let Err(e) = turn.run(&mut history, &mut |_| {}).await {
        eprintln!("{what} verification classifier failed; delivering without verification: {e}");
        return None;
    }
    let got = slot.lock().unwrap().clone();
    got.as_deref().and_then(Category::parse)
}

/// Отнести просьбу к категории. `recent` — недавние реплики (говорящий,
/// текст) до просьбы. Сбой модели — без проверки.
pub async fn classify<P: Provider>(provider: &P, request: &str, recent: &[(String, String)], spill: &std::path::Path) -> Option<Category> {
    let tool = Submit {
        slot: Arc::new(Mutex::new(None)),
        name: SUBMIT,
        description: "Submit whether the supplied request needs factual verification.",
        schema: "Submit the request verification category.",
        choices: &["health", "bookings", "none"],
    };
    let lines: Vec<String> = recent.iter().map(|(who, text)| format!("{who}: {text}")).collect();
    let user = format!("<recent_conversation>\n{}\n</recent_conversation>\n\n<request>\n{request}\n</request>", lines.join("\n\n"));
    submit_category(provider, tool, CLASSIFIER_MD, user, spill, "request").await
}

/// Отнести отчёт расписания к категории. Сбой модели — без проверки.
pub async fn classify_cron<P: Provider>(provider: &P, task: &str, report: &str, spill: &std::path::Path) -> Option<Category> {
    let tool = Submit {
        slot: Arc::new(Mutex::new(None)),
        name: SUBMIT_CRON,
        description: "Submit whether the supplied cron report needs factual verification.",
        schema: "Submit the cron report verification category.",
        choices: &["health", "bookings", "purchases", "none"],
    };
    let user = format!("<task>\n{task}\n</task>\n\n<report>\n{report}\n</report>");
    submit_category(provider, tool, CRON_CLASSIFIER_MD, user, spill, "cron report").await
}

/// Ход для проверяющего: вызовы с аргументами и первая строка результата
/// (до 500 знаков). Своё: в разборе не видно.
pub fn trajectory(messages: &[Message]) -> String {
    let mut out = vec![];
    for m in messages {
        for p in &m.content {
            match p {
                Part::ToolCall { name, arguments, .. } => out.push(format!("call {name} {arguments}")),
                Part::ToolResult { content, is_error, .. } => {
                    let first = content.iter().find_map(|c| if let ToolResultPart::Text { text } = c { Some(text.as_str()) } else { None }).unwrap_or("");
                    let first: String = first.lines().next().unwrap_or("").chars().take(500).collect();
                    out.push(format!("{} {first}", if *is_error { "error" } else { "result" }));
                }
                _ => {}
            }
        }
    }
    out.join("\n")
}

/// Отчёт, проверка которого не закончилась.
pub fn unverified(report: &str) -> String {
    format!("{}\n\n{}", report.trim_end(), UNVERIFIED_MD.trim_end())
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
        Category::Purchases => PURCHASES_MD.trim_end().to_string(),
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

impl<P> Clone for Verifier<P> {
    fn clone(&self) -> Self {
        Verifier {
            store: self.store.clone(),
            provider: self.provider.clone(),
            tools: self.tools.clone(),
            system: self.system.clone(),
            spill_dir: self.spill_dir.clone(),
            now: self.now.clone(),
        }
    }
}

static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl<P: Provider + Send + Sync + 'static> Verifier<P> {
    /// Проверить черновик; сбой — `Unfinished` («Verification did not
    /// finish.» ядра — в журнал).
    pub async fn verify(&self, category: Category, task: &str, draft: &str, trajectory: &str) -> Verdict {
        let run_id = format!("verification:{}:{}", (self.now)(), RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        let meta = json!({"category": category.as_str()});
        if let Err(e) = self.store.run_insert("verification", &run_id, &run_id, "verification", &meta).await {
            eprintln!("Verification did not finish. {e}");
            return Verdict::Unfinished;
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
                (Verdict::Unfinished, self.store.hook_run_fail(&run_id, "resolve", &err).await)
            }
        };
        if let Err(e) = settled {
            eprintln!("verification {run_id}: {e}");
        }
        verdict
    }
}

/// Проверка отчётов расписаний: классификатор (модель `main_chat`) и
/// проверяющий.
pub struct CronVerification<P> {
    pub classifier: Arc<P>,
    pub verifier: Verifier<P>,
}

impl<P: Provider + Send + Sync + 'static> CronVerification<P> {
    /// Отчёт после проверки: исправленный, исходный или с пометкой о сбое.
    /// `worker` — история работника (для хода проверяющего). Категория —
    /// сохранённая в отложенном итоге прогона `run_id`, иначе — от
    /// классификатора, и сразу сохраняется.
    pub async fn check(&self, run_id: &str, task: &str, report: &str, worker: &[Message], spill: &std::path::Path) -> String {
        let store = &self.verifier.store;
        let retained = match store.pending(run_id).await {
            Ok(p) => p.and_then(|p| p["verification_category"].as_str().map(str::to_string)),
            Err(e) => {
                eprintln!("load_cron_verification_category {run_id}: {e}");
                None
            }
        };
        let category = match retained {
            Some(c) => Category::parse(&c),
            None => {
                let c = classify_cron(&*self.classifier, task, report, spill).await;
                if let Err(e) = store.retain_category(run_id, c.map_or("none", Category::as_str)).await {
                    eprintln!("retain_cron_verification_category {run_id}: {e}");
                }
                c
            }
        };
        let Some(category) = category else {
            return report.to_string();
        };
        match self.verifier.verify(category, task, report, &trajectory(worker)).await {
            Verdict::Revised(r) => r,
            Verdict::Original => report.to_string(),
            Verdict::Unfinished => unverified(report),
        }
    }
}
