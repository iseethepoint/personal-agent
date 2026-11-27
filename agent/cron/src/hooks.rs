//! Наблюдатели (event hooks) — форма muse `hatch-server/.../runtime/event_hooks/*`
//! (`source-files.txt:5988-5992`: `admission.rs`, `definition_watch.rs`,
//! `poll.rs`). Наблюдатель — скрипт-проверка на Bash по ритму и
//! инструкция работнику, которого будит `wake`.
//!
//! - определение — `~/hooks/definitions/<id>.json` (`HookDefinition`,
//!   `strings-all.txt:15784-15786`, `:31218-31223`); скрипты — в
//!   `~/hooks/scripts/`, состояние — в `~/hooks/state/`, библиотека —
//!   `~/hooks/runtime/hamlet_hook_runtime.sh` (у muse
//!   `~/hooks/runtime/hatch_hook_runtime.sh`, переменные `HATCH_HOOK_*` →
//!   `HAMLET_HOOK_*`);
//! - допуск — `runtime.event_hook_admissions` (`sql.txt:908,1110`): опрос
//!   идёт, только пока определение и скрипт совпадают с допущенными;
//! - журнал вызовов — JSONL (`HookInvocationLog`); путь в разборе не найден,
//!   своё: `~/hooks/logs/<id>.jsonl`;
//! - работник — та же двухфазная схема, что у расписаний; задание —
//!   `[EVENT HOOK — EXECUTE]` (`strings-all.txt:178084-178093`, тире
//!   потеряно в разборе), итог — `[EVENT HOOK — RESOLVE]` (`:31220`,
//!   `:178094`), отчёт главному агенту — `[EVENT HOOK RESULT]`.
//!
//! Скрипт запускается своим запуском `bash` с окружением агента без ключей
//! моделей и поиска; у muse — через обычный исполнитель команд. Своё.
//! Не взято: `delivery` (один разговор — главный, П6), `connection.*`,
//! таблицы `execute_resolve_*` (итог идёт сразу в ящик).
//!
//! Пределы определения — ядро (функция 0x10243e80, objdump): `prompt` не
//! длиннее 65 536 байт, `poll_interval_secs` от 5 до 604 800,
//! `script_timeout_secs` от 1 до 600; проверки в этом порядке, тексты
//! ошибок — строки ядра (`0x366ffe`, `0x3950a0`, `0x437955`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use agent_loop::{personal, Tool, ToolError, ToolFuture};
use agent_model::{ToolResultPart, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::service::Core;

const RUNTIME_SH: &str = include_str!("../runtime/hamlet_hook_runtime.sh");
const TEXTS_JSON: &str = include_str!("../../../prompts/hooks/tools.json");

static TEXTS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(TEXTS_JSON).expect("prompts/hooks/tools.json"));

fn text(group: &str, key: &str) -> String {
    personal(TEXTS[group][key].as_str().unwrap_or_default())
}

/// `poll_interval_secs` по умолчанию — описание поля muse.
pub const DEFAULT_POLL_SECS: i64 = 60;
/// Срок скрипта по умолчанию. Числа в разборе нет — своё.
pub const DEFAULT_SCRIPT_TIMEOUT_SECS: i64 = 60;
/// Строк `hooks.logs` без `limit` — своё.
const DEFAULT_LOG_LIMIT: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookDef {
    pub version: u32,
    pub id: String,
    pub enabled: bool,
    pub script_path: String,
    pub prompt: String,
    pub poll_interval_secs: i64,
    pub script_timeout_secs: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Решение скрипта и всё, что он вывел.
#[derive(Debug, Clone, Default)]
pub struct Invocation {
    pub invocation_id: String,
    pub decision: Option<String>,
    pub reason: String,
    pub payload: Value,
    pub disable: bool,
    pub script_logs: Vec<Value>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
    pub started_at_ms: i64,
    pub finished_at_ms: i64,
}

/// Срабатывание, ждущее работника.
#[derive(Debug, Clone)]
pub struct Wake {
    pub def: HookDef,
    pub inv: Invocation,
}

fn dir(core: &Core, sub: &str) -> PathBuf {
    core.home.join("hooks").join(sub)
}

fn def_path(core: &Core, id: &str) -> PathBuf {
    dir(core, "definitions").join(format!("{id}.json"))
}

fn log_path(core: &Core, id: &str) -> PathBuf {
    dir(core, "logs").join(format!("{id}.jsonl"))
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Путь скрипта на диске: `~/hooks/scripts/…`.
fn script_file(core: &Core, script_path: &str) -> PathBuf {
    let rel = script_path.strip_prefix("~/").unwrap_or(script_path);
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        core.home.join(p)
    }
}

/// Проверить и привести путь скрипта к виду `~/hooks/scripts/<…>.sh`.
/// Тексты ошибок — ядро (`strings-all.txt`, «Отказы и ошибки hooks»).
fn normalize_script(core: &Core, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("script_path cannot be empty".into());
    }
    if !raw.ends_with(".sh") {
        return Err("hook script must use a .sh extension".into());
    }
    let file = script_file(core, raw);
    let scripts = dir(core, "scripts");
    let rel = file.strip_prefix(&scripts).map_err(|_| "Invalid hook script path".to_string())?;
    if rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err("Invalid hook script path".into());
    }
    if !std::fs::metadata(&file).map(|m| m.is_file()).unwrap_or(false) {
        return Err(format!("hook script is not a regular file: {raw}"));
    }
    Ok(format!("~/hooks/scripts/{}", rel.display()))
}

fn check_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.chars().count() > 64 {
        return Err("hook id must contain 1 to 64 characters".into());
    }
    let first = id.chars().next().unwrap();
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err("hook id must start with a lowercase ASCII letter or digit".into());
    }
    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        return Err("hook id may contain only lowercase ASCII letters, digits, hyphens, and underscores".into());
    }
    Ok(())
}

pub async fn read_def(core: &Core, id: &str) -> Result<Option<HookDef>, String> {
    match tokio::fs::read_to_string(def_path(core, id)).await {
        Ok(t) => serde_json::from_str(&t).map(Some).map_err(|e| format!("hook definition {id}: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("hook definition {id}: {e}")),
    }
}

pub async fn list_defs(core: &Core) -> Result<Vec<HookDef>, String> {
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(dir(core, "definitions")).await else { return Ok(out) };
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".json") {
            if let Some(d) = read_def(core, id).await? {
                out.push(d);
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn def_bytes(def: &HookDef) -> Vec<u8> {
    let mut v = serde_json::to_vec_pretty(def).unwrap_or_default();
    v.push(b'\n');
    v
}

/// Записать определение и допуск — `INSERT … ON CONFLICT` / `UPDATE …
/// revision = revision + 1` (`sql.txt:1110`).
async fn save(core: &Core, def: &HookDef) -> Result<(), String> {
    let bytes = def_bytes(def);
    let script = tokio::fs::read(script_file(core, &def.script_path)).await.map_err(|e| format!("hook script: {e}"))?;
    tokio::fs::create_dir_all(dir(core, "definitions")).await.map_err(|e| e.to_string())?;
    tokio::fs::write(def_path(core, &def.id), &bytes).await.map_err(|e| e.to_string())?;
    sqlx::query(
        "insert into runtime.event_hook_admissions (hook_id, revision, definition_sha256, script_sha256, enabled)
         values ($1, 1, $2, $3, $4)
         on conflict (hook_id) do update set revision = runtime.event_hook_admissions.revision + 1,
             definition_sha256 = excluded.definition_sha256, script_sha256 = excluded.script_sha256, enabled = excluded.enabled",
    )
    .bind(&def.id)
    .bind(sha(&bytes))
    .bind(sha(&script))
    .bind(def.enabled)
    .execute(core.store.pool())
    .await
    .map_err(|e| format!("Hook admission failed: {e}"))?;
    Ok(())
}

/// Совпадают ли определение и скрипт с допущенными.
async fn admitted(core: &Core, def: &HookDef) -> Result<bool, String> {
    let row = sqlx::query("select definition_sha256, script_sha256, enabled from runtime.event_hook_admissions where hook_id = $1")
        .bind(&def.id)
        .fetch_optional(core.store.pool())
        .await
        .map_err(|e| e.to_string())?;
    let Some(r) = row else { return Ok(false) };
    let script = tokio::fs::read(script_file(core, &def.script_path)).await.unwrap_or_default();
    Ok(r.get::<bool, _>("enabled") && r.get::<String, _>("definition_sha256") == sha(&def_bytes(def)) && r.get::<String, _>("script_sha256") == sha(&script))
}

/// Запустить скрипт. `dry_run` — `HAMLET_HOOK_DRY_RUN=1`.
pub async fn invoke(core: &Core, def: &HookDef, dry_run: bool) -> Invocation {
    let started = core.now();
    let id = format!("{}-{started}", def.id);
    let mut inv = Invocation { invocation_id: id.clone(), started_at_ms: started, payload: Value::Null, ..Default::default() };
    let runtime = dir(core, "runtime").join("hamlet_hook_runtime.sh");
    let state = dir(core, "state");
    let prepared = async {
        tokio::fs::create_dir_all(runtime.parent().unwrap()).await?;
        tokio::fs::create_dir_all(&state).await?;
        tokio::fs::write(&runtime, RUNTIME_SH).await
    }
    .await;
    if let Err(e) = prepared {
        inv.error = Some(format!("hook runtime: {e}"));
        inv.finished_at_ms = core.now();
        return inv;
    }
    let script = script_file(core, &def.script_path);
    let mut cmd = tokio::process::Command::new("/bin/bash");
    cmd.arg(&script)
        .current_dir(&core.home)
        .env("HAMLET_HOOK_RUNTIME", &runtime)
        .env("HAMLET_HOOK_ID", &def.id)
        .env("HAMLET_HOOK_INVOCATION_ID", &id)
        .env("HAMLET_HOOK_SCRIPT_PATH", &script)
        .env("HAMLET_HOOK_STATE_DIR", &state)
        .env("HAMLET_HOOK_DRY_RUN", if dry_run { "1" } else { "0" })
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    for name in &core.secret_envs {
        cmd.env_remove(name);
    }
    let out = tokio::time::timeout(Duration::from_secs(def.script_timeout_secs.max(1) as u64), cmd.output()).await;
    inv.finished_at_ms = core.now();
    let out = match out {
        Err(_) => {
            inv.error = Some(format!("hook script did not finish within {}s and was stopped", def.script_timeout_secs));
            return inv;
        }
        Ok(Err(e)) => {
            inv.error = Some(format!("hook script: {e}"));
            return inv;
        }
        Ok(Ok(o)) => o,
    };
    inv.stdout = String::from_utf8_lossy(&out.stdout).to_string();
    inv.stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let mut decisions = Vec::new();
    let mut plain_out = Vec::new();
    for line in inv.stdout.lines() {
        match line.strip_prefix("HAMLET_HOOK_RESULT:") {
            Some(j) => decisions.push(j.to_string()),
            None => plain_out.push(line),
        }
    }
    let mut plain_err = Vec::new();
    for line in inv.stderr.lines() {
        match line.strip_prefix("HAMLET_HOOK_LOG:").and_then(|j| serde_json::from_str::<Value>(j).ok()) {
            Some(v) => inv.script_logs.push(v),
            None => plain_err.push(line),
        }
    }
    inv.stdout = plain_out.join("\n");
    inv.stderr = plain_err.join("\n");
    if !out.status.success() {
        inv.error = Some("hook script exited unsuccessfully".into());
        return inv;
    }
    let raw = match decisions.as_slice() {
        [] => {
            inv.error = Some("script exited without calling silent() or wake()".into());
            return inv;
        }
        [one] => one,
        _ => {
            inv.error = Some("script emitted more than one hook decision".into());
            return inv;
        }
    };
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            inv.error = Some(format!("invalid hook decision JSON: {e}"));
            return inv;
        }
    };
    let decision = v["decision"].as_str().unwrap_or_default().to_string();
    if decision != "silent" && decision != "wake" {
        inv.error = Some(format!("unsupported hook decision `{decision}`"));
        return inv;
    }
    inv.decision = Some(decision);
    inv.reason = v["reason"].as_str().unwrap_or_default().to_string();
    inv.payload = v.get("payload").cloned().unwrap_or(Value::Null);
    inv.disable = v["disable_after_run"].as_bool().unwrap_or(false);
    inv
}

/// Запись журнала вызовов (`HookInvocationLog`).
async fn append_log(core: &Core, def: &HookDef, inv: &Invocation, dry_run: bool, outcome: &str, extra: Value) -> Value {
    let mut entry = json!({
        "invocation_id": inv.invocation_id,
        "hook_id": def.id,
        "started_at_ms": inv.started_at_ms,
        "finished_at_ms": inv.finished_at_ms,
        "duration_ms": inv.finished_at_ms - inv.started_at_ms,
        "outcome": outcome,
        "reason": inv.reason,
        "payload": inv.payload,
        "script_logs": inv.script_logs,
        "stdout": inv.stdout,
        "stderr": inv.stderr,
        "dry_run": dry_run,
        "disable_requested": inv.disable,
        "error": inv.error,
    });
    if let (Some(e), Value::Object(x)) = (entry.as_object_mut(), extra) {
        e.extend(x);
    }
    let path = log_path(core, &def.id);
    let line = format!("{entry}\n");
    let written = async {
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        use tokio::io::AsyncWriteExt;
        let mut f = tokio::fs::OpenOptions::new().create(true).append(true).open(&path).await?;
        f.write_all(line.as_bytes()).await
    }
    .await;
    if let Err(e) = written {
        eprintln!("append hook log {}: {e}", def.id);
    }
    entry
}

async fn read_log(core: &Core, id: &str) -> Vec<Value> {
    tokio::fs::read_to_string(log_path(core, id))
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Живой вызов: журнал, выключение по `disable_after_run`, срабатывание —
/// в очередь работников.
pub async fn live(core: &Core, def: &HookDef) -> Result<(Invocation, Value), String> {
    let inv = invoke(core, def, false).await;
    let outcome = match (&inv.error, inv.decision.as_deref()) {
        (Some(_), _) => "error",
        (None, Some(d)) => d,
        _ => "error",
    };
    let mut extra = json!({});
    if inv.error.is_none() && inv.disable {
        let mut d = def.clone();
        d.enabled = false;
        d.updated_at_ms = core.now();
        match save(core, &d).await {
            Ok(()) => extra["disable_outcome"] = "disabled".into(),
            Err(e) => {
                extra["disable_outcome"] = "failed".into();
                extra["disable_error"] = e.into();
            }
        }
    }
    if outcome == "wake" {
        let run_id = format!("hook:{}:{}", def.id, inv.invocation_id);
        extra["execute_resolve_run_id"] = run_id.into();
        core.wakes.lock().unwrap().push(Wake { def: def.clone(), inv: inv.clone() });
    }
    let entry = append_log(core, def, &inv, false, outcome, extra).await;
    Ok((inv, entry))
}

/// Опросы на такте: включённые наблюдатели, чей срок подошёл.
pub async fn poll(core: &Core) -> Result<(), String> {
    let now = core.now();
    for def in list_defs(core).await? {
        if !def.enabled {
            continue;
        }
        let due = core.next_poll.lock().unwrap().get(&def.id).is_none_or(|t| *t <= now);
        if !due {
            continue;
        }
        core.next_poll.lock().unwrap().insert(def.id.clone(), now + def.poll_interval_secs * 1000);
        if !admitted(core, &def).await? {
            let inv = Invocation { invocation_id: format!("{}-{now}", def.id), started_at_ms: now, finished_at_ms: now, ..Default::default() };
            append_log(core, &def, &inv, false, "definition_changed", json!({})).await;
            continue;
        }
        live(core, &def).await?;
    }
    Ok(())
}

/// Срок ближайшего опроса включённого наблюдателя (ещё не опрошенный —
/// сейчас).
pub async fn next_poll_at(core: &Core) -> Result<Option<i64>, String> {
    let now = core.now();
    let next = core.next_poll.lock().unwrap().clone();
    Ok(list_defs(core).await?.iter().filter(|d| d.enabled).map(|d| next.get(&d.id).copied().unwrap_or(now)).min())
}

/// Задание работнику — фаза исполнения.
pub fn execute_message(w: &Wake, run_id: &str) -> String {
    let event = json!({
        "hook_id": w.def.id,
        "invocation_id": w.inv.invocation_id,
        "script_path": w.def.script_path,
        "wake_reason": w.inv.reason,
        "wake_payload": w.inv.payload,
        "polled_at_ms": w.inv.started_at_ms,
    });
    format!(
        "[EVENT HOOK \u{2014} EXECUTE]\nworker_kind: event_hook\nrun_id: {run_id}\nsource_ref: hook:{}\n{}\n<event_data>\n{}\n</event_data>\n<hook_instruction>\n{}\n</hook_instruction>",
        w.def.id,
        text("texts", "execute_intro"),
        serde_json::to_string_pretty(&event).unwrap_or_default(),
        w.def.prompt.trim_end()
    )
}

/// Задание работнику — фаза итога.
pub fn resolve_message(w: &Wake, run_id: &str) -> String {
    format!(
        "[EVENT HOOK \u{2014} RESOLVE]\nworker_kind: event_hook\nrun_id: {run_id}\nsource_ref: hook:{}\n{}",
        w.def.id,
        text("texts", "resolve")
    )
}

/// Отчёт главному агенту.
pub fn result_text(message: &str) -> String {
    format!("[EVENT HOOK RESULT]{}\n\n{}", text("texts", "result"), message.trim_end())
}

// ---- инструменты ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    List,
    View,
    Add,
    Update,
    Enable,
    Disable,
    Remove,
    DryRun,
    Run,
    Logs,
}

impl Op {
    pub const ALL: [Op; 10] = [Op::List, Op::View, Op::Add, Op::Update, Op::Enable, Op::Disable, Op::Remove, Op::DryRun, Op::Run, Op::Logs];

    fn name(self) -> &'static str {
        match self {
            Op::List => "hooks.list",
            Op::View => "hooks.view",
            Op::Add => "hooks.add",
            Op::Update => "hooks.update",
            Op::Enable => "hooks.enable",
            Op::Disable => "hooks.disable",
            Op::Remove => "hooks.remove",
            Op::DryRun => "hooks.dry_run",
            Op::Run => "hooks.run",
            Op::Logs => "hooks.logs",
        }
    }
}

pub struct HookTool {
    op: Op,
    core: Arc<Core>,
}

pub fn tools(core: &Arc<Core>) -> Vec<HookTool> {
    Op::ALL.into_iter().map(|op| HookTool { op, core: core.clone() }).collect()
}

fn field(key: &str) -> String {
    text("fields", key)
}

impl Tool for HookTool {
    fn spec(&self) -> ToolSpec {
        let mut p = Map::new();
        let mut required = vec![];
        let id = json!({"type": "string", "pattern": "^[a-z0-9][a-z0-9_-]{0,63}$", "description": field("id")});
        match self.op {
            Op::List => {
                p.insert("include_disabled".into(), json!({"type": "boolean", "description": field("include_disabled")}));
            }
            Op::Add | Op::Update => {
                p.insert("id".into(), id);
                p.insert("script_path".into(), json!({"type": "string", "description": field("script_path")}));
                p.insert("prompt".into(), json!({"type": "string", "description": field("prompt")}));
                p.insert("poll_interval_secs".into(), json!({"type": "integer", "minimum": POLL_SECS.0, "maximum": POLL_SECS.1, "description": field("poll_interval_secs")}));
                // Описания `script_timeout_secs` в разборе нет — своё.
                p.insert(
                    "script_timeout_secs".into(),
                    json!({"type": "integer", "minimum": 1, "maximum": MAX_SCRIPT_TIMEOUT_SECS, "description": format!("Seconds the polling script may run before it is stopped. Defaults to {DEFAULT_SCRIPT_TIMEOUT_SECS}.")}),
                );
                required.push("id");
                if self.op == Op::Add {
                    required.extend(["script_path", "prompt"]);
                }
            }
            Op::Logs => {
                p.insert("id".into(), id);
                p.insert("limit".into(), json!({"type": "integer", "minimum": 1, "description": field("limit")}));
                required.push("id");
            }
            _ => {
                p.insert("id".into(), id);
                required.push("id");
            }
        }
        let description = text("tools", self.op.name());
        ToolSpec {
            name: self.op.name().into(),
            description,
            input_schema: json!({"type": "object", "properties": p, "required": required, "additionalProperties": false}),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let out = self.dispatch(&args).await.map_err(ToolError::Failed)?;
            // Правка наблюдателя меняет срок опроса.
            self.core.wake();
            Ok(vec![ToolResultPart::Text { text: serde_json::to_string_pretty(&out).unwrap_or_default() }])
        })
    }
}

fn str_arg<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k).and_then(Value::as_str)
}

pub const MAX_PROMPT_BYTES: usize = 65_536;
pub const POLL_SECS: (i64, i64) = (5, 604_800);
pub const MAX_SCRIPT_TIMEOUT_SECS: i64 = 600;

/// Пределы определения наблюдателя (ядро, 0x10243e80).
fn check_limits(d: &HookDef) -> Result<(), String> {
    if d.prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!("prompt exceeds the {MAX_PROMPT_BYTES}-byte limit"));
    }
    if !(POLL_SECS.0..=POLL_SECS.1).contains(&d.poll_interval_secs) {
        return Err(format!("poll_interval_secs must be between {} and {}", POLL_SECS.0, POLL_SECS.1));
    }
    if !(1..=MAX_SCRIPT_TIMEOUT_SECS).contains(&d.script_timeout_secs) {
        return Err(format!("script_timeout_secs must be between 1 and {MAX_SCRIPT_TIMEOUT_SECS}"));
    }
    Ok(())
}

fn positive(args: &Value, k: &str) -> Result<Option<i64>, String> {
    match args.get(k) {
        None => Ok(None),
        Some(v) => v.as_i64().filter(|n| *n >= 1).map(Some).ok_or_else(|| format!("{k} must be a positive integer")),
    }
}

/// Все ли наблюдённые значения нулевые или пустые (журнал скрипта и
/// полезная нагрузка, без `message`).
fn all_zero(inv: &Invocation) -> bool {
    fn leaves(v: &Value, out: &mut Vec<Value>) {
        match v {
            Value::Object(m) => m.iter().filter(|(k, _)| k.as_str() != "message").for_each(|(_, x)| leaves(x, out)),
            Value::Array(a) => a.iter().for_each(|x| leaves(x, out)),
            x => out.push(x.clone()),
        }
    }
    let mut vals = Vec::new();
    inv.script_logs.iter().for_each(|l| leaves(l, &mut vals));
    leaves(&inv.payload, &mut vals);
    let vals: Vec<Value> = vals.into_iter().filter(|v| !v.is_null()).collect();
    !vals.is_empty()
        && vals.iter().all(|v| match v {
            Value::Number(n) => n.as_f64() == Some(0.0),
            Value::String(s) => s.trim().is_empty(),
            Value::Bool(b) => !b,
            _ => false,
        })
}

impl HookTool {
    async fn def(&self, args: &Value) -> Result<HookDef, String> {
        let id = str_arg(args, "id").unwrap_or_default();
        check_id(id)?;
        read_def(&self.core, id).await?.ok_or_else(|| format!("hook does not exist: {id}"))
    }

    fn view(&self, d: &HookDef) -> Value {
        let mut v = json!(d);
        v["definition_path"] = format!("~/hooks/definitions/{}.json", d.id).into();
        v
    }

    async fn dispatch(&self, args: &Value) -> Result<Value, String> {
        let core = &self.core;
        match self.op {
            Op::List => {
                let all = args.get("include_disabled").and_then(Value::as_bool).unwrap_or(false);
                let defs = list_defs(core).await?;
                Ok(json!({"hooks": defs.iter().filter(|d| all || d.enabled).map(|d| self.view(d)).collect::<Vec<_>>()}))
            }
            Op::View => Ok(self.view(&self.def(args).await?)),
            Op::Add => {
                let id = str_arg(args, "id").unwrap_or_default();
                check_id(id)?;
                if read_def(core, id).await?.is_some() {
                    return Err(format!("hook already exists: {id}"));
                }
                let script_path = normalize_script(core, str_arg(args, "script_path").unwrap_or_default())?;
                let prompt = str_arg(args, "prompt").unwrap_or_default().trim().to_string();
                if prompt.is_empty() {
                    return Err("prompt cannot be empty".into());
                }
                let now = core.now();
                let d = HookDef {
                    version: 1,
                    id: id.into(),
                    enabled: false,
                    script_path,
                    prompt,
                    poll_interval_secs: positive(args, "poll_interval_secs")?.unwrap_or(DEFAULT_POLL_SECS),
                    script_timeout_secs: positive(args, "script_timeout_secs")?.unwrap_or(DEFAULT_SCRIPT_TIMEOUT_SECS),
                    created_at_ms: now,
                    updated_at_ms: now,
                };
                check_limits(&d)?;
                save(core, &d).await?;
                let mut v = self.view(&d);
                v["message"] = text("texts", "created").into();
                Ok(v)
            }
            Op::Update => {
                let mut d = self.def(args).await?;
                if let Some(s) = str_arg(args, "script_path") {
                    d.script_path = normalize_script(core, s)?;
                } else {
                    normalize_script(core, &d.script_path.clone())?;
                }
                if let Some(p) = str_arg(args, "prompt") {
                    if p.trim().is_empty() {
                        return Err("prompt cannot be empty".into());
                    }
                    d.prompt = p.trim().into();
                }
                if let Some(n) = positive(args, "poll_interval_secs")? {
                    d.poll_interval_secs = n;
                }
                if let Some(n) = positive(args, "script_timeout_secs")? {
                    d.script_timeout_secs = n;
                }
                check_limits(&d)?;
                d.updated_at_ms = core.now();
                save(core, &d).await?;
                Ok(self.view(&d))
            }
            Op::Enable | Op::Disable => {
                let mut d = self.def(args).await?;
                d.enabled = self.op == Op::Enable;
                d.updated_at_ms = core.now();
                save(core, &d).await?;
                core.next_poll.lock().unwrap().remove(&d.id);
                Ok(self.view(&d))
            }
            Op::Remove => {
                let d = self.def(args).await?;
                tokio::fs::remove_file(def_path(core, &d.id)).await.map_err(|e| e.to_string())?;
                sqlx::query("delete from runtime.event_hook_admissions where hook_id = $1")
                    .bind(&d.id)
                    .execute(core.store.pool())
                    .await
                    .map_err(|e| e.to_string())?;
                core.next_poll.lock().unwrap().remove(&d.id);
                Ok(json!({"id": d.id, "removed": true}))
            }
            Op::DryRun => {
                let d = self.def(args).await?;
                let history = read_log(core, &d.id).await;
                let inv = invoke(core, &d, true).await;
                let outcome = inv.error.as_ref().map(|_| "error").or(inv.decision.as_deref()).unwrap_or("error").to_string();
                let entry = append_log(core, &d, &inv, true, &outcome, json!({})).await;
                let would_wake = inv.decision.as_deref() == Some("wake");
                let mut v = json!({
                    "id": d.id,
                    "decision": inv.decision,
                    "reason": inv.reason,
                    "payload": inv.payload,
                    "error": inv.error,
                    "script_logs": inv.script_logs,
                    "jsonl": entry,
                    "would_wake": would_wake,
                    "would_disable": inv.disable,
                });
                // Ключи заметок — свои, кроме `state_note` и `wake_note`
                // (имена ядра; к каким текстам они относятся — вывод).
                if would_wake {
                    v["state_note"] = text("texts", "dry_run_wake").into();
                }
                if all_zero(&inv) {
                    v["observation_note"] = text("texts", "all_zero").into();
                }
                let ever_woke = history.iter().any(|e| e["outcome"] == "wake");
                if !would_wake && inv.error.is_none() && !ever_woke {
                    v["wake_note"] = text("texts", "never_woke").into();
                }
                Ok(v)
            }
            Op::Run => {
                let d = self.def(args).await?;
                let (inv, entry) = live(core, &d).await?;
                Ok(json!({"id": d.id, "decision": inv.decision, "reason": inv.reason, "payload": inv.payload, "error": inv.error, "jsonl": entry}))
            }
            Op::Logs => {
                let d = self.def(args).await?;
                let limit = args.get("limit").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(DEFAULT_LOG_LIMIT);
                let log = read_log(core, &d.id).await;
                let tail = log[log.len().saturating_sub(limit)..].to_vec();
                Ok(json!({"id": d.id, "entries": tail}))
            }
        }
    }
}

/// Своя запись `next_poll`: срок следующего опроса каждого наблюдателя
/// (`HookPollSchedule` у muse держит `scheduled_at_ms`).
pub type PollSchedule = std::sync::Mutex<HashMap<String, i64>>;

/// Полное описание пространства `hooks` — уходит при его загрузке
/// (`tool_search.load_tool_namespace`).
pub fn namespace_description() -> String {
    personal(TEXTS["namespace"].as_str().unwrap_or_default())
}
