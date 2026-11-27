//! Сохранённые работы — файлы `<workspace>/.jarvis/workflows/<name>.js`
//! (форма `hatch-workflows/src/saved.rs`, `strings-all.txt:337746`).
//! Ошибки — тексты ядра.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::meta;

pub const SAVED_DIR: &str = ".jarvis/workflows";
pub const RUNS_DIR: &str = ".jarvis/workflow-runs";

pub fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn path(workspace: &Path, name: &str) -> PathBuf {
    workspace.join(SAVED_DIR).join(format!("{name}.js"))
}

fn modified(p: &Path) -> Option<String> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Прочитать файл сценария: не ссылка и обычный файл.
pub fn read_regular(p: &Path) -> Result<String, String> {
    let m = std::fs::symlink_metadata(p).map_err(|_| format!("workflow script `{}` was not found", p.display()))?;
    if m.file_type().is_symlink() {
        return Err(format!("saved workflow path is a symlink: {}", p.display()));
    }
    if !m.is_file() {
        return Err(format!("saved workflow path is not a regular file: {}", p.display()));
    }
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// Описание сохранённой работы (`phases phaseDetails scriptSha256
/// updatedAt scriptContent`).
pub fn describe(workspace: &Path, name: &str, with_content: bool) -> Result<Value, String> {
    meta::check_name(name)?;
    let p = path(workspace, name);
    if std::fs::symlink_metadata(&p).is_err() {
        return Err(format!("workflow script `{name}` was not found"));
    }
    let text = read_regular(&p)?;
    let mut v = json!({"name": name, "scriptPath": p.display().to_string(), "scriptSha256": sha256(&text), "updatedAt": modified(&p)});
    match meta::parse(&text) {
        Ok(s) => {
            v["workflowName"] = s.meta.name.into();
            v["description"] = s.meta.description.into();
            v["phases"] = s.meta.phases.into();
            v["phaseDetails"] = s.meta.phase_details.into();
        }
        Err(e) => v["error"] = e.into(),
    }
    if with_content {
        v["scriptContent"] = text.into();
    }
    Ok(v)
}

/// Все сохранённые работы по имени; нечитаемая — запись с ошибкой.
pub fn list(workspace: &Path, limit: usize) -> Vec<Value> {
    let Ok(rd) = std::fs::read_dir(workspace.join(SAVED_DIR)) else { return vec![] };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".js")).map(String::from)).collect();
    names.sort();
    names
        .into_iter()
        .take(limit)
        .map(|n| describe(workspace, &n, false).unwrap_or_else(|e| json!({"name": n, "scriptPath": path(workspace, &n).display().to_string(), "error": e})))
        .collect()
}

/// Записать сохранённую работу (`create` — только новую, `update` —
/// только существующую, `save_run` — любую).
pub fn write(workspace: &Path, name: &str, script: &str, must_exist: Option<bool>) -> Result<Value, String> {
    meta::check_name(name)?;
    meta::parse(script)?;
    let p = path(workspace, name);
    let exists = std::fs::symlink_metadata(&p).is_ok();
    match must_exist {
        Some(false) if exists => return Err(format!("saved workflow `{name}` already exists")),
        Some(true) if !exists => return Err(format!("workflow script `{name}` was not found")),
        _ => {}
    }
    if exists {
        read_regular(&p)?;
    }
    std::fs::create_dir_all(workspace.join(SAVED_DIR)).map_err(|e| e.to_string())?;
    std::fs::write(&p, script).map_err(|e| format!("{}: {e}", p.display()))?;
    describe(workspace, name, false)
}

pub fn delete(workspace: &Path, name: &str) -> Result<Value, String> {
    let v = describe(workspace, name, true)?;
    std::fs::remove_file(path(workspace, name)).map_err(|e| e.to_string())?;
    Ok(v)
}

/// Путь `scriptPath`: в пределах рабочей папки.
pub fn script_path(workspace: &Path, p: &str) -> Result<PathBuf, String> {
    let raw = Path::new(p);
    let full = if raw.is_absolute() { raw.to_path_buf() } else { workspace.join(raw) };
    let mut norm = PathBuf::new();
    for c in full.components() {
        match c {
            std::path::Component::ParentDir => {
                norm.pop();
            }
            std::path::Component::CurDir => {}
            c => norm.push(c),
        }
    }
    if !norm.starts_with(workspace) {
        return Err(format!("invalid workflow control: saved workflow path must stay inside workspace root {}", workspace.display()));
    }
    Ok(norm)
}
