//! `agent-workflows` — сохранённые многошаговые работы `workflow.*`. Форма
//! muse: крейт `hatch-workflows` (`lib.rs`, `runtime.rs`, `saved.rs`),
//! `hatch-server/src/daemon/workflows/` (исполнитель, восстановление,
//! передача итога), инструмент `hatch-agent/src/tools/workflow_tool.rs`.
//!
//! Работа — сценарий на JS (`export const meta = {…}` и вызовы `agent()`,
//! `phase()`, `parallel()`, `pipeline()`), его исполняет встроенный движок
//! (`js.rs`). Каждый `agent()` — подагент шага (`workflow_subagent`,
//! модель сессии, набор без `workflow.*` и подагентов,
//! `strings-all.txt:71745-71900`), ответ — конверт `{status, result}`
//! (`prompts/workflows/agent_structured_output.md`). Ход — в таблицах
//! `runtime.workflow_*`; готовые вызовы при повторе сценария берутся из базы
//! по ключу повтора, поэтому пауза, возобновление и восстановление после
//! рестарта — повтор сценария с начала. Шаг, которому нужен ответ, отвечает
//! `blocked` — запуск встаёт на паузу (`mark workflow blocked`), после
//! `workflow.resume` этот вызов исполняется снова (`requeue blocked workflow
//! agent call`). Итог фонового запуска и пауза на `blocked` уходят главному
//! агенту в ящик (`workflow_terminal_handoff`).
//!
//! Своё (в разборе нет): текст передачи итога — квитанции ядра
//! («Workflow `…` completed.» и т. п.) и итог; пример `ok` для шага со
//! схемой; место блока `## Workflow execution files` — после задачи;
//! подпись вызова хранится в `options_json.label`; `cache_key` — sha256 от
//! ключа повтора, подсказки и опций; запуск с `resumeFromRunId` — только от
//! запуска в состоянии `failed` (так в описании поля); `list_runs` по
//! умолчанию 100, не больше 200 (как `workflow.list`). Нет у нас: предела
//! вызовов на запуск (числа в разборе нет), аренды исполнителя (одна служба
//! в одном процессе), карточек одобрения для чужих чатов (один владелец).

pub mod js;
pub mod meta;
pub mod saved;
pub mod store;
pub mod tools;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_loop::{personal, ChildRecord, Finish, Inbox, Kind, KindRecord, Prefix, Status, Subagents, Tools, Transcript, WorkerStore, WORK_NOT_RESUMABLE};
use agent_model::{Message, Part, Provider, Role};
use rand::Rng;
use serde_json::{json, Value};
use tokio::sync::{oneshot, Notify};
use tokio::task::{JoinHandle, JoinSet};

use crate::js::{Failure, Out, Reply};
use crate::store::{Call, NewRun, Run, Store};

const OUTPUT_CONTRACT: &str = include_str!("../../../prompts/workflows/agent_structured_output.md");
const EXECUTION_FILES: &str = include_str!("../../../prompts/workflows/execution_files.md");

/// Набор инструментов шага для папки запуска (команды начинаются там).
pub type ChildTools = Arc<dyn Fn(&Path) -> Tools + Send + Sync>;

type Kids = Arc<Mutex<HashMap<String, String>>>;

/// Класс владельца восстановления вызова шага — значение ядра
/// (`workflow_subagent`); владелец — вызов (`call_id`).
pub const CHILD_CLASS: &str = "workflow_subagent";

/// Вызов, который после перезапуска продолжается: запись, начало
/// подсказки и история подагента шага.
type Resumable = (Arc<dyn ChildRecord>, Prefix, Vec<Message>);

struct Active {
    cancel: Arc<Notify>,
    handle: JoinHandle<()>,
}

/// Служба сохранённых работ одного агента.
pub struct Workflows<P> {
    store: Store,
    children: Subagents<P>,
    child_tools: ChildTools,
    system: String,
    home: PathBuf,
    workspace: PathBuf,
    /// Ящик главного агента — передача итогов.
    inbox: Inbox,
    active: Mutex<HashMap<String, Active>>,
    /// Подагенты шагов идущих запусков: call_id → id подагента.
    kids: Kids,
    changed: Notify,
    /// Владельцы восстановления и история подагентов шагов.
    workers: std::sync::OnceLock<Arc<dyn WorkerStore>>,
    /// Вызовы, которые продолжатся при повторе сценария: call_id → запись.
    resumable: Mutex<HashMap<String, Resumable>>,
}

fn new_run_id() -> String {
    let b: [u8; 16] = rand::thread_rng().gen();
    format!("workflow-run-{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

fn new_call_id() -> String {
    let b: [u8; 12] = rand::thread_rng().gen();
    format!("call-{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

fn field<'a>(v: &'a Value, camel: &str, snake: &str) -> Option<&'a Value> {
    v.get(camel).or_else(|| v.get(snake)).filter(|x| !x.is_null())
}

fn text_field(v: &Value, camel: &str, snake: &str) -> Option<String> {
    field(v, camel, snake).and_then(Value::as_str).map(String::from)
}

/// Схема — нижняя граница: лишние поля объектов принимаются на всех уровнях
/// («additional object properties are accepted recursively»).
fn minimum_contract(schema: &Value) -> Value {
    match schema {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, v)| !((k.as_str() == "additionalProperties" || k.as_str() == "unevaluatedProperties") && **v == Value::Bool(false)))
                .map(|(k, v)| (k.clone(), minimum_contract(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(minimum_contract).collect()),
        v => v.clone(),
    }
}

fn check_schema(schema: &Value, value: &Value) -> Result<(), String> {
    let v = jsonschema::validator_for(&minimum_contract(schema)).map_err(|e| format!("invalid workflow agent schema: {e}"))?;
    v.validate(value).map_err(|e| format!("workflow agent output did not match schema: {e}"))
}

/// Ответ шага.
#[derive(Debug, PartialEq)]
enum Envelope {
    Ok(Value),
    Blocked(Value),
}

/// Разбор конверта `{status, result}`. Своё: если весь текст не JSON,
/// берётся кусок от первой `{` до последней `}`.
fn envelope(text: &str, schema: Option<&Value>) -> Result<Envelope, String> {
    let t = text.trim();
    let v: Value = match serde_json::from_str(t) {
        Ok(v) => v,
        Err(e) => match (t.find('{'), t.rfind('}')) {
            (Some(a), Some(b)) if a < b => serde_json::from_str(&t[a..=b]).map_err(|_| format!("workflow agent output was not JSON: {e}"))?,
            _ => return Err(format!("workflow agent output was not JSON: {e}")),
        },
    };
    let Some(status) = v.get("status").and_then(Value::as_str) else {
        // «workflow agent accepted raw schema output without status envelope»
        if let Some(s) = schema {
            if check_schema(s, &v).is_ok() {
                return Ok(Envelope::Ok(v));
            }
        }
        return Err("workflow agent status must be ok, error, or blocked; got ``".into());
    };
    let result = v.get("result").cloned().unwrap_or(Value::Null);
    match status {
        "ok" => {
            match schema {
                Some(s) => check_schema(s, &result)?,
                None if result.as_str().is_some_and(|s| !s.trim().is_empty()) => {}
                None => return Err("workflow stored agent result must be a non-empty string when no schema is provided".into()),
            }
            Ok(Envelope::Ok(result))
        }
        "blocked" => {
            let get = |k: &str| result.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
            let reason = get("blocked_reason").ok_or("workflow agent blocked result blocked_reason cannot be empty")?;
            let message = get("message").ok_or("workflow agent blocked result message cannot be empty")?;
            Ok(Envelope::Blocked(json!({"blocked_reason": reason, "message": message})))
        }
        "error" => {
            let m = result.get("message").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
            Err(m.ok_or("workflow agent error result message cannot be empty")?.to_string())
        }
        other => Err(format!("workflow agent status must be ok, error, or blocked; got `{other}`")),
    }
}

/// Квитанция запуска — тексты ядра.
pub fn receipt(run: &Run) -> String {
    let n = &run.workflow_name;
    match run.status.as_str() {
        "completed" => format!("Workflow `{n}` completed."),
        "failed" | "syntax_error" => format!("Workflow `{n}` failed."),
        "stopped" => format!("Workflow `{n}` stopped."),
        "paused" => match run.blocked() {
            Some(b) => format!("Workflow `{n}` blocked: {}", b["message"].as_str().unwrap_or_default()),
            None => format!("Workflow `{n}` paused."),
        },
        s => format!("Workflow `{n}` is {s}."),
    }
}

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        v => serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

/// Передача итога фонового запуска главному агенту (своё, см. шапку).
fn handoff_text(run: &Run) -> String {
    let body = match run.status.as_str() {
        "completed" => shown(&run.final_value()),
        "paused" => String::new(),
        _ => run.error.clone().unwrap_or_default(),
    };
    let head = format!("[Workflow Result] run_id: {}\n{}", run.run_id, receipt(run));
    if body.is_empty() {
        head
    } else {
        format!("{head}\n\n{body}")
    }
}

struct RunCtx {
    run: Run,
    workdir: PathBuf,
    phases: HashMap<String, String>,
}

impl<P: Provider + Send + Sync + 'static> Workflows<P> {
    /// `provider` — модель сессии (шаги идут на ней, `model = "session"`);
    /// `child_tools` — набор шага; `system`/`home` — начало подсказки шага;
    /// `workspace` — рабочая папка (сохранённые работы и папки запусков);
    /// `inbox` — ящик главного агента.
    #[allow(clippy::too_many_arguments)]
    pub fn new(store: Store, provider: Arc<P>, child_tools: ChildTools, system: String, home: PathBuf, workspace: PathBuf, spill_dir: PathBuf, inbox: Inbox) -> Arc<Self> {
        let tools = Arc::new(child_tools(&workspace));
        let prefix = Prefix::assemble(&system, &home, &tools);
        // Итоги шагов служба забирает сама — у подагентов шагов свой ящик.
        let children = Subagents::new(provider, tools, prefix, spill_dir, Inbox::new(), Transcript::new());
        Arc::new(Workflows { store, children, child_tools, system, home, workspace, inbox, active: Mutex::new(HashMap::new()), kids: Arc::new(Mutex::new(HashMap::new())), changed: Notify::new(), workers: std::sync::OnceLock::new(), resumable: Mutex::new(HashMap::new()) })
    }

    /// Вызовы шагов пишут историю подагента и точки перезапуска, вызов —
    /// владелец восстановления (`agent.recovery_owners`).
    pub fn set_workers(&self, w: Arc<dyn WorkerStore>) {
        let _ = self.workers.set(w);
    }

    /// Сколько запусков сейчас исполняется.
    pub fn running(&self) -> usize {
        self.active.lock().unwrap().len()
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    fn start_executor(self: &Arc<Self>, run_id: &str) {
        let cancel = Arc::new(Notify::new());
        let mut active = self.active.lock().unwrap();
        let me = self.clone();
        let (id, c) = (run_id.to_string(), cancel.clone());
        let handle = tokio::spawn(async move { me.execute(id, c).await });
        active.insert(run_id.to_string(), Active { cancel, handle });
    }

    async fn execute(self: Arc<Self>, run_id: String, cancel: Arc<Notify>) {
        if let Err(e) = self.drive(&run_id, &cancel).await {
            let _ = self.store.set_status(&run_id, Some("running"), "failed", Some(&e), None).await;
        }
        {
            let mut active = self.active.lock().unwrap();
            if active.get(&run_id).is_some_and(|a| Arc::ptr_eq(&a.cancel, &cancel)) {
                active.remove(&run_id);
            }
        }
        self.handoff(&run_id).await;
        self.changed.notify_waiters();
    }

    async fn close_kids(&self, calls: &[String]) {
        let ids: Vec<String> = {
            let mut k = self.kids.lock().unwrap();
            calls.iter().filter_map(|c| k.remove(c)).collect()
        };
        for id in ids {
            let _ = self.children.close(&id).await;
        }
    }

    async fn drive(self: &Arc<Self>, run_id: &str, cancel: &Notify) -> Result<(), String> {
        if !self.store.start(run_id).await? {
            return Ok(());
        }
        let run = self.store.run(run_id).await?.ok_or_else(|| format!("workflow run disappeared after resume: {run_id}"))?;
        let path = PathBuf::from(&run.script_path);
        let text = saved::read_regular(&path)?;
        if saved::sha256(&text) != run.script_sha256 {
            return Err(format!("workflow script hash mismatch for {}", run.script_path));
        }
        let script = meta::parse(&text)?;
        let phases = self.store.phases(run_id).await?.into_iter().map(|p| (p.name, p.phase_run_id)).collect();
        let workdir = path.parent().map(|d| d.join("work")).unwrap_or_else(|| self.workspace.clone());
        let ctx = Arc::new(RunCtx { run, workdir, phases });
        let (mut out, replies) = js::spawn(js::source(&script.meta.phases, &ctx.run.args, &script.body));
        let mut calls: JoinSet<()> = JoinSet::new();
        let mut call_ids: Vec<String> = vec![];
        let started: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        loop {
            tokio::select! {
                _ = cancel.notified() => {
                    calls.abort_all();
                    call_ids.extend(started.lock().unwrap().drain(..));
                    self.close_kids(&call_ids).await;
                    return Ok(());
                }
                ev = out.recv() => match ev {
                    None => return Err("workflow JavaScript failed: script runtime stopped".into()),
                    Some(Out::Phase { name, status, error }) => self.store.phase_event(run_id, &name, &status, error.as_deref()).await?,
                    Some(Out::Agent { seq, prompt, options, phase, replay_key, label }) => {
                        let (me, ctx, tx, started) = (self.clone(), ctx.clone(), replies.clone(), started.clone());
                        calls.spawn(async move {
                            let result = me.agent_call(&ctx, &started, prompt, options, phase, replay_key, label).await;
                            let _ = tx.send(Reply { seq, result });
                        });
                    }
                    Some(Out::Done(r)) => {
                        calls.abort_all();
                        call_ids.extend(started.lock().unwrap().drain(..));
                        self.close_kids(&call_ids).await;
                        self.store.settle_open_calls(run_id, "stopped", Some("workflow finished before the call")).await?;
                        return self.finish(run_id, r).await;
                    }
                },
                Some(_) = calls.join_next(), if !calls.is_empty() => {}
            }
        }
    }

    async fn finish(&self, run_id: &str, r: Result<Value, Failure>) -> Result<(), String> {
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let (status, error, final_result) = match r {
            Ok(env) if env["ok"] == true => ("completed", None, Some(env.get("result").cloned().unwrap_or(Value::Null).to_string())),
            Ok(env) if env["blocked"] == true => {
                let d = &env["blockedDetails"];
                let fin = json!({"status": "blocked", "blocked_reason": s(d, "blocked_reason"), "message": s(d, "message")});
                ("paused", Some(s(&env, "error")), Some(fin.to_string()))
            }
            Ok(env) if env["parked"] == true => ("paused", Some(s(&env, "parkedReason")), None),
            Ok(env) => ("failed", Some(s(&env, "error")), None),
            Err(Failure::Syntax(e)) => ("syntax_error", Some(format!("invalid workflow script: {e}")), None),
            Err(Failure::Runtime(e)) => ("failed", Some(format!("workflow JavaScript failed: {e}")), None),
        };
        self.store.set_status(run_id, Some("running"), status, error.as_deref(), final_result.as_deref()).await?;
        Ok(())
    }

    fn child_prompt(&self, prompt: &str, schema: Option<&Value>, workdir: &Path) -> String {
        let (contract, example, section) = match schema {
            Some(s) => (
                "When `status` is `ok`, `result` must validate against the JSON Schema below. The JSON Schema applies to `result` only, not to the top-level envelope.",
                r#"{"status":"ok","result":{...}}"#.to_string(),
                format!("```json\n{}\n```", serde_json::to_string_pretty(s).unwrap_or_default()),
            ),
            None => ("When `status` is `ok`, `result` must be a plain string message summarizing the completed step.", r#"{"status":"ok","result":"Completed the requested step."}"#.to_string(), String::new()),
        };
        let body = personal(OUTPUT_CONTRACT)
            .replace("{{RESULT_CONTRACT}}", contract)
            .replace("{{OK_EXAMPLE}}", &example)
            .replace("{{JSON_SCHEMA_SECTION}}\n\n", &if section.is_empty() { String::new() } else { format!("{section}\n\n") })
            .replace("{{TASK_PROMPT}}", prompt);
        let files = personal(EXECUTION_FILES).replace("{{WORKFLOW_WORK_DIR}}", &workdir.display().to_string());
        format!("{}\n\n{}", body.trim_end(), files.trim_end())
    }

    fn stored(c: &Call) -> Result<Value, String> {
        let s = c.final_response.as_deref().ok_or("workflow stored agent result is missing")?;
        serde_json::from_str(s).map_err(|e| e.to_string())
    }

    #[allow(clippy::too_many_arguments)]
    async fn agent_call(&self, ctx: &RunCtx, started: &Mutex<Vec<String>>, prompt: String, options: Value, phase: Option<String>, replay_key: String, label: Option<String>) -> Result<Value, String> {
        let run_id = ctx.run.run_id.as_str();
        let schema = options.get("schema").cloned().filter(|s| !s.is_null());
        let cache_key = saved::sha256(&json!([replay_key, prompt, options]).to_string());
        let mut stored_opts = if options.is_object() { options.clone() } else { json!({}) };
        if let Some(l) = &label {
            stored_opts["label"] = l.clone().into();
        }
        let phase_run_id = phase.as_ref().and_then(|p| ctx.phases.get(p)).map(String::as_str);
        let mut resumed: Option<Resumable> = None;
        let call = match self.store.call(run_id, &replay_key).await? {
            Some(c) if c.status == "completed" || c.status == "cached" => return Self::stored(&c),
            // Вызов, который перезапуск оборвал без возможности продолжить:
            // повтор сценария получает ту же ошибку.
            Some(c) if c.status == "failed" && c.error.as_deref() == Some(WORK_NOT_RESUMABLE) => return Err(WORK_NOT_RESUMABLE.to_string()),
            Some(c) if c.status == "running" && self.resumable.lock().unwrap().contains_key(&c.call_id) => {
                resumed = self.resumable.lock().unwrap().remove(&c.call_id);
                c
            }
            Some(c) => {
                self.store.requeue_call(&c.call_id).await?;
                c
            }
            None => {
                if let Some(src) = &ctx.run.resume_from_run_id {
                    if let Some(hit) = self.store.cached(src, &cache_key).await? {
                        let c = self.store.insert_call(&new_call_id(), run_id, phase_run_id, &replay_key, &cache_key, &prompt, &stored_opts, "cached", Some(&hit)).await?;
                        return Self::stored(&c);
                    }
                }
                self.store.insert_call(&new_call_id(), run_id, phase_run_id, &replay_key, &cache_key, &prompt, &stored_opts, "pending", None).await?
            }
        };
        let timeout_ms = options.get("timeoutMs").and_then(Value::as_u64);
        let (tx, rx) = oneshot::channel::<(Status, String)>();
        let tx = Arc::new(Mutex::new(Some(tx)));
        let finish: Finish = Arc::new(move |st, text| {
            if let Some(t) = tx.lock().unwrap().take() {
                let _ = t.send((st, text.clone()));
            }
            Box::pin(async move { Message { role: Role::Developer, content: vec![Part::Text { text }] } })
        });
        let tools = Arc::new((self.child_tools)(&ctx.workdir));
        let (prefix, resume, record) = match resumed {
            Some((record, prefix, history)) => (prefix, Some(history), Some(record)),
            None => {
                let prefix = Prefix::assemble(&self.system, &self.home, &tools);
                // Подагент шага в базе. Своё: id — `workflow-call:<call_id>`.
                let record = match self.workers.get() {
                    Some(w) => Some(w.begin(CHILD_CLASS, &call.call_id, &format!("workflow-call:{}", call.call_id), &prefix, None).await?),
                    None => None,
                };
                (prefix, None, record)
            }
        };
        let record = record.map(|record| KindRecord { record, class: CHILD_CLASS, owner: call.call_id.clone() });
        let child = self.children.spawn_kind(Kind {
            task: label.clone().unwrap_or_else(|| prompt.clone()),
            first: self.child_prompt(&prompt, schema.as_ref(), &ctx.workdir),
            prefix,
            tools,
            terminal: vec![],
            done: Arc::new(|| true),
            nudge: None,
            deadline: timeout_ms.map(Duration::from_millis),
            finish,
            resume,
            record,
        });
        self.kids.lock().unwrap().insert(call.call_id.clone(), child.clone());
        started.lock().unwrap().push(call.call_id.clone());
        self.store.call_running(&call.call_id, &child).await?;
        let t0 = Instant::now();
        let (st, text) = rx.await.unwrap_or((Status::Failed, "workflow child owner disappeared during transition".into()));
        let ms = t0.elapsed().as_millis() as i64;
        if self.kids.lock().unwrap().remove(&call.call_id).is_some() && st == Status::Interrupted {
            let _ = self.children.close(&child).await;
        }
        self.children.inbox().drain();
        if let Some(w) = self.workers.get() {
            w.terminal(CHILD_CLASS, &call.call_id, st.as_str()).await?;
        }
        let call = &call;
        let fail = |e: String| async move {
            self.store.finish_call(call, "failed", None, ms, Some(&e)).await?;
            Err::<Value, String>(e)
        };
        match st {
            Status::Done => match envelope(&text, schema.as_ref()) {
                Ok(Envelope::Ok(v)) => {
                    self.store.finish_call(call, "completed", Some(&v.to_string()), ms, None).await?;
                    Ok(v)
                }
                Ok(Envelope::Blocked(d)) => {
                    self.store.finish_call(call, "blocked", Some(&d.to_string()), ms, Some(d["message"].as_str().unwrap_or_default())).await?;
                    Ok(json!({"__hatchWorkflowControl": "blocked", "result": d}))
                }
                Err(e) => fail(e).await,
            },
            Status::Closed => {
                let e = "workflow agent call stopped".to_string();
                self.store.finish_call(call, "stopped", None, ms, Some(&e)).await?;
                Err(e)
            }
            Status::Failed if timeout_ms.is_some() && text == "deadline reached before the terminal call" => fail(format!("workflow agent timed out after {} ms", timeout_ms.unwrap_or_default())).await,
            _ => fail(text).await,
        }
    }

    async fn handoff(&self, run_id: &str) {
        if let Ok(Some(run)) = self.store.deliver(run_id).await {
            self.inbox.push(Message { role: Role::Developer, content: vec![Part::Text { text: handoff_text(&run) }] }).await;
        }
    }

    async fn run_of(&self, run_id: &str) -> Result<Run, String> {
        self.store.run(run_id).await?.ok_or_else(|| format!("workflow run not found: {run_id}"))
    }

    /// Ждать, пока запуск выйдет из queued / running / stopping.
    pub async fn wait(&self, run_id: &str) -> Result<Run, String> {
        loop {
            let n = self.changed.notified();
            tokio::pin!(n);
            n.as_mut().enable();
            let run = self.run_of(run_id).await?;
            if !matches!(run.status.as_str(), "queued" | "running" | "stopping") {
                return Ok(run);
            }
            n.await;
        }
    }

    /// Снять исполнителя запуска и дождаться его конца.
    async fn cancel(&self, run_id: &str) {
        let a = self.active.lock().unwrap().remove(run_id);
        if let Some(a) = a {
            a.cancel.notify_one();
            let _ = a.handle.await;
        }
    }

    fn outcome(run: &Run) -> Value {
        let run_dir = Path::new(&run.script_path).parent().map(|p| p.display().to_string());
        json!({"workflowName": run.workflow_name, "runDir": run_dir, "finalResult": run.final_value(), "run": run.json()})
    }

    /// `workflow.launch` (`sync`) и `workflow.launch_async` (`async`).
    pub async fn launch(self: &Arc<Self>, input: &Value, mode: &str) -> Result<(String, Value), String> {
        let ws = &self.workspace;
        let text = if let Some(p) = text_field(input, "scriptPath", "script_path") {
            let path = saved::script_path(ws, &p)?;
            if std::fs::symlink_metadata(&path).is_err() {
                return Err(format!("workflow script `{p}` was not found"));
            }
            saved::read_regular(&path)?
        } else if let Some(s) = text_field(input, "script", "script") {
            s
        } else if let Some(n) = text_field(input, "name", "name") {
            meta::check_name(&n)?;
            let path = saved::path(ws, &n);
            if std::fs::symlink_metadata(&path).is_err() {
                return Err(format!("workflow script `{n}` was not found"));
            }
            saved::read_regular(&path)?
        } else {
            return Err("workflow input requires one of scriptPath, script, or name".into());
        };
        let script = meta::parse(&text)?;
        let args = field(input, "args", "args").cloned().unwrap_or_else(|| json!({}));
        let resume_from = text_field(input, "resumeFromRunId", "resume_from_run_id");
        if let Some(src) = &resume_from {
            let s = self.store.run(src).await?.ok_or_else(|| format!("resumeFromRunId workflow run not found: {src}"))?;
            if s.workflow_name != script.meta.name {
                return Err("resumeFromRunId must belong to the same workflow name".into());
            }
            if Path::new(&s.workspace_root) != ws.as_path() {
                return Err("resumeFromRunId must belong to the same workspace".into());
            }
            if s.status != "failed" {
                return Err(format!("resumeFromRunId must name a failed workflow run; {src} is {}", s.status));
            }
        }
        let run_id = new_run_id();
        let run_dir = ws.join(saved::RUNS_DIR).join(&run_id);
        std::fs::create_dir_all(run_dir.join("work")).map_err(|e| format!("{}: {e}", run_dir.display()))?;
        let snapshot = run_dir.join("workflow.js");
        std::fs::write(&snapshot, &text).map_err(|e| format!("{}: {e}", snapshot.display()))?;
        self.store
            .admit(
                NewRun {
                    run_id: &run_id,
                    launch_mode: mode,
                    resume_from_run_id: resume_from.as_deref(),
                    workflow_name: &script.meta.name,
                    description: &script.meta.description,
                    phases: &Value::from(script.meta.phase_details.clone()),
                    args: &args,
                    workspace_root: &ws.display().to_string(),
                    script_path: &snapshot.display().to_string(),
                    script_sha256: &saved::sha256(&text),
                },
                &script.meta.phases,
            )
            .await?;
        self.start_executor(&run_id);
        if mode == "async" {
            let run = self.run_of(&run_id).await?;
            return Ok(("Workflow started in background.".into(), json!({"runId": run_id, "workflowName": run.workflow_name, "runDir": run_dir.display().to_string(), "run": run.json()})));
        }
        let run = self.wait(&run_id).await?;
        Ok((receipt(&run), Self::outcome(&run)))
    }

    pub async fn pause(self: &Arc<Self>, run_id: &str) -> Result<(String, Value), String> {
        let run = self.run_of(run_id).await?;
        if run.status == "paused" {
            return Ok((receipt(&run), run.json()));
        }
        if run.is_terminal() || run.status == "stopping" {
            return Err(format!("invalid workflow control: cannot pause workflow run {run_id}: run is {}", run.status));
        }
        self.cancel(run_id).await;
        let now = self.run_of(run_id).await?;
        if matches!(now.status.as_str(), "queued" | "running") {
            self.store.set_status(run_id, Some(&now.status), "paused", None, None).await?;
            self.store.settle_open_calls(run_id, "pending", None).await?;
        }
        self.changed.notify_waiters();
        let run = self.run_of(run_id).await?;
        Ok(("Applied workflow control `pause`.".into(), run.json()))
    }

    pub async fn resume(self: &Arc<Self>, run_id: &str) -> Result<(String, Value), String> {
        let run = self.run_of(run_id).await?;
        if run.status != "paused" {
            return Err(format!("invalid workflow control: cannot resume workflow run {run_id}: run is {}", run.status));
        }
        let run = self.store.resume(run_id).await?.ok_or_else(|| format!("workflow run disappeared after resume: {run_id}"))?;
        self.start_executor(run_id);
        if run.launch_mode == "sync" {
            let run = self.wait(run_id).await?;
            return Ok((receipt(&run), Self::outcome(&run)));
        }
        let run = self.run_of(run_id).await?;
        Ok(("Applied workflow control `resume`.".into(), run.json()))
    }

    pub async fn stop(self: &Arc<Self>, run_id: &str) -> Result<(String, Value), String> {
        let run = self.run_of(run_id).await?;
        if run.is_terminal() {
            return Err(format!("invalid workflow control: cannot stop workflow run {run_id}: run is {}", run.status));
        }
        self.store.set_status(run_id, None, "stopping", None, run.final_result.as_deref()).await?;
        self.cancel(run_id).await;
        self.store.settle_open_calls(run_id, "stopped", Some("workflow run stopped")).await?;
        self.store.set_status(run_id, Some("stopping"), "stopped", None, None).await?;
        self.handoff(run_id).await;
        self.changed.notify_waiters();
        let run = self.run_of(run_id).await?;
        Ok((receipt(&run), run.json()))
    }

    pub async fn stop_agent(&self, run_id: &str, call_id: &str) -> Result<(String, Value), String> {
        let run = self.run_of(run_id).await?;
        if run.is_terminal() {
            return Err(format!("invalid workflow control: cannot stop an agent for terminal workflow run {run_id}"));
        }
        let calls = self.store.calls(run_id).await?;
        let call = calls.iter().find(|c| c.call_id == call_id).ok_or_else(|| format!("invalid workflow control: agent call `{call_id}` does not belong to workflow run {run_id}"))?;
        let child = self.kids.lock().unwrap().get(call_id).cloned();
        match child {
            Some(id) => self.children.close(&id).await?,
            None if call.status == "pending" => self.store.finish_call(call, "stopped", None, 0, Some("workflow agent call stopped")).await?,
            None => {}
        }
        let after = self.store.call(run_id, &call.replay_key).await?.map(|c| c.status).unwrap_or_default();
        Ok(("Applied workflow control `stop_agent`.".into(), json!({"runId": run_id, "callId": call_id, "status": after})))
    }

    pub async fn save_run(&self, run_id: &str, name: Option<&str>) -> Result<(String, Value), String> {
        let run = self.run_of(run_id).await?;
        let text = saved::read_regular(Path::new(&run.script_path))?;
        if saved::sha256(&text) != run.script_sha256 {
            return Err(format!("workflow script hash mismatch for {}", run.script_path));
        }
        let name = name.unwrap_or(&run.workflow_name);
        let v = saved::write(&self.workspace, name, &text, None)?;
        Ok(("Applied workflow control `save_run`.".into(), v))
    }

    pub async fn view_run(&self, run_id: &str) -> Result<Value, String> {
        let run = self.run_of(run_id).await?;
        let phases: Vec<Value> = self
            .store
            .phases(run_id)
            .await?
            .into_iter()
            .map(|p| json!({"name": p.name, "ordinal": p.ordinal, "status": p.status, "agentTotal": p.agent_total, "agentCompleted": p.agent_completed, "error": p.error}))
            .collect();
        let calls: Vec<Value> = self
            .store
            .calls(run_id)
            .await?
            .into_iter()
            .map(|c| {
                json!({
                    "callId": c.call_id,
                    "replayKey": c.replay_key,
                    "label": c.options.get("label"),
                    "status": c.status,
                    "childAgentId": c.child_agent_id,
                    "cachedFromCallId": c.cached_from_call_id,
                    "durationMs": c.duration_ms,
                    "result": c.final_response.as_deref().and_then(|s| serde_json::from_str::<Value>(s).ok()),
                    "error": c.error,
                })
            })
            .collect();
        let mut v = run.json();
        v["phases"] = phases.into();
        v["agentCalls"] = calls.into();
        Ok(v)
    }

    pub async fn list_runs(&self, limit: i64) -> Result<Vec<Value>, String> {
        Ok(self.store.runs(limit).await?.iter().map(Run::json).collect())
    }

    /// Восстановление после рестарта: прерванные запуски — снова в работу с
    /// повтором сценария (`daemon_restarted_while_workflow_running`),
    /// недоостановленные — в `stopped`.
    pub async fn recover(self: &Arc<Self>) -> Result<Vec<String>, String> {
        let mut ids = vec![];
        for run in self.store.unfinished().await? {
            if run.status == "stopping" {
                self.store.settle_open_calls(&run.run_id, "stopped", Some("workflow run stopped")).await?;
                self.store.set_status(&run.run_id, Some("stopping"), "stopped", None, None).await?;
                self.handoff(&run.run_id).await;
                continue;
            }
            self.store.requeue(&run.run_id, "daemon_restarted_while_workflow_running").await?;
            // Идущие вызовы: с незакрытым владельцем и точкой
            // `before_inference` подагента шага — продолжатся при повторе
            // сценария («load workflow recovery owner»), прочие — `failed`
            // текстом ядра («fail workflow recovery owner»,
            // `restart_after_output_unrecoverable`).
            for c in self.store.running_calls(&run.run_id).await? {
                let resumable = match self.workers.get() {
                    Some(w) => match w.active(CHILD_CLASS, &c.call_id).await? {
                        Some(rec) if rec.interrupted().await?.is_some() => {
                            let (prefix, mut history) = rec.load().await?;
                            agent_loop::close_interrupted(&mut history);
                            rec.record(&history).await?;
                            Some((rec, prefix, history))
                        }
                        _ => None,
                    },
                    None => None,
                };
                match resumable {
                    Some(r) => {
                        self.resumable.lock().unwrap().insert(c.call_id.clone(), r);
                    }
                    None => {
                        self.store.finish_call(&c, "failed", None, c.duration_ms.unwrap_or(0), Some(WORK_NOT_RESUMABLE)).await?;
                        if let Some(w) = self.workers.get() {
                            w.terminal(CHILD_CLASS, &c.call_id, "failed").await?;
                        }
                    }
                }
            }
            self.start_executor(&run.run_id);
            ids.push(run.run_id);
        }
        Ok(ids)
    }

    /// Остановить исполнителей (конец программы): запуски остаются в
    /// базе и продолжатся при следующем старте.
    pub async fn shutdown(&self) {
        let ids: Vec<String> = self.active.lock().unwrap().keys().cloned().collect();
        for id in ids {
            self.cancel(&id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_forms() {
        let schema = json!({"type": "object", "required": ["days"], "properties": {"days": {"type": "integer"}}, "additionalProperties": false});
        assert_eq!(envelope(r#"{"status":"ok","result":{"days":3,"extra":1}}"#, Some(&schema)), Ok(Envelope::Ok(json!({"days": 3, "extra": 1}))));
        assert!(envelope(r#"{"status":"ok","result":{"days":"x"}}"#, Some(&schema)).unwrap_err().starts_with("workflow agent output did not match schema"));
        assert_eq!(envelope("Готово:\n```json\n{\"status\":\"ok\",\"result\":\"done\"}\n```", None), Ok(Envelope::Ok(json!("done"))));
        assert_eq!(envelope(r#"{"days":2}"#, Some(&schema)), Ok(Envelope::Ok(json!({"days": 2}))));
        assert_eq!(envelope(r#"{"status":"error","result":{"message":"no access"}}"#, None), Err("no access".into()));
        assert!(envelope("plain", None).unwrap_err().starts_with("workflow agent output was not JSON"));
        assert_eq!(
            envelope(r#"{"status":"blocked","result":{"blocked_reason":"needs_user","message":"Which city?"}}"#, None),
            Ok(Envelope::Blocked(json!({"blocked_reason": "needs_user", "message": "Which city?"})))
        );
        assert_eq!(envelope(r#"{"status":"done","result":"x"}"#, None), Err("workflow agent status must be ok, error, or blocked; got `done`".into()));
    }
}
