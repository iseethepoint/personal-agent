//! `trash.put`, `trash.list`, `trash.restore` — корзина по форме muse
//! (`hatch-agent/src/tools/trash.rs`, `hatch-trash/src/lib.rs`,
//! `source-files.txt:1807, 6305`). Описания и поля — строки ядра
//! (`0x2cda44e`–`0x2cdb040`, поле `custom_path` — рядом); ответы и ошибки —
//! шаблоны ядра (`0x2de038`, `0x3217b2`, `0x342b10`, `0x2ce14bd`,
//! `0x2ff8e4`, `0x2f37d7`, `0x357838`, `0x361da1`, `0x3f7dc1`, `0x404da1`,
//! `0x6da6cd`).
//!
//! Запись — `~/.local/share/Trash/hamlet/<trash_id>/`: содержимое в
//! `files/`, метаданные `TrashInfo` (10 полей ядра) в `.trashinfo`; номер —
//! `%Y%m%d-%H%M%S-<32 hex>` (пример ядра `20260623-120000-0123…`); срок —
//! 30 дней.
//!
//! Правки (П10): «Muse» → «Hamlet», папка `Trash/hatch` → `Trash/hamlet`.
//! Своё: просроченные записи убираются при каждом обращении к корзине (у
//! muse — служба обслуживания `recoverable_trash_reaper`; своей службы
//! обслуживания у Hamlet нет); вид ответа `put`/`restore` — строки ядра
//! «- `ключ`: значение» по полям описания; дата — RFC 3339 UTC; метаданные —
//! JSON; ошибка, когда путь не существует или не переносится.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools};
use agent_model::{ToolResultPart, ToolSpec};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::paths::Home;

const PUT_DESCRIPTION: &str = "Move one file or directory into Hamlet's recoverable trash and get back a trash_id for later recovery. Use this instead of permanent deletion (such as rm through exec) whenever removing <!-- personal-only:start -->user<!-- personal-only:end --><!-- group-only:start -->workspace<!-- group-only:end --> files should be reversible: workspace cleanup, replacing an old version, or any destructive file-removal flow. The path resolves against the agent home directory; the entry must exist, and a trailing symlink is trashed as the symlink itself, never followed to its target. The move is a rename into `~/.local/share/Trash/hamlet/<trash_id>/files/`, so the source must live on the same filesystem as the home directory, and the trash directory itself is refused. The result reports the trash_id, the original path, the trashed location, and the expiry date. Entries are cleaned up automatically 30 days after trashing; until then the trashed content stays inspectable under `~/.local/share/Trash/hamlet/<trash_id>/files/`. To review what is in the trash use trash.list; to bring an entry back use trash.restore.";
const PUT_PATH: &str = "File or directory to move into recoverable trash. Relative paths resolve against the agent home directory. The entry must exist; a trailing symlink is trashed as the symlink itself, not its target.";
const LIST_DESCRIPTION: &str = "List every entry in Hamlet's recoverable trash. Read-only and takes no parameters. Returns a markdown table sorted newest first with each entry's trash_id, status (active, restored, or expired), expiry date, original path, and top-level contents. Only entries with status active can still be restored: a restored entry keeps its metadata as an audit record, and an expired entry's content has already been removed. Active content stays inspectable under `~/.local/share/Trash/hamlet/<trash_id>/files/` until it expires, 30 days after trashing. Use this to find the trash_id that trash.restore needs, or to confirm what an earlier trash.put captured. To trash something use trash.put; to recover an entry use trash.restore.";
const RESTORE_DESCRIPTION: &str = "Move an active trash entry back out of Hamlet's recoverable trash. trash_id is the id returned by trash.put or shown by trash.list; only an entry with status active can be restored, and a restored or expired entry fails with an error. By default the entry returns to the original path recorded when it was trashed; pass custom_path to restore it somewhere else. Both destinations resolve against the agent home directory, and missing parent directories are created as needed. Restore never overwrites an existing path: if the destination is already occupied the call fails, so inspect the occupant, pick a new name or directory, and call again with custom_path. On success the result reports the trash_id, the original path, and the restored path, and the entry's metadata stays behind as an audit record. To find an entry's id first use trash.list; to trash something use trash.put.";
const RESTORE_ID: &str = "Trash id returned by trash.put or shown by trash.list. Only an entry with status active can be restored.";
const RESTORE_CUSTOM: &str = "Optional destination path; resolves against the agent home directory. If omitted or blank, the entry is restored to its recorded original path. Restore never overwrites an existing path; on a conflict, retry with a new name or directory here.";

/// Папка корзины от дома и как её показывать модели.
pub const TRASH_DIR: &str = ".local/share/Trash/hamlet";
const SHOWN_ROOT: &str = "~/.local/share/Trash/hamlet";
/// Срок записи — 30 дней (описания ядра).
pub const EXPIRY_DAYS: i64 = 30;

/// Метаданные записи — `struct TrashInfo with 10 elements` ядра.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrashInfo {
    trash_id: String,
    original_path: String,
    trashed_path: String,
    deletion_date: String,
    expiry_date: String,
    status: String,
    top_level_entries: Vec<String>,
    expired_at: Option<String>,
    restored_at: Option<String>,
    restored_path: Option<String>,
}

/// Корзина одного агента.
#[derive(Clone)]
pub struct Trash {
    home: Arc<Home>,
    now: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

fn stamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn inspect_hint() -> String {
    format!("Use the trash.list tool to inspect trash ids. Trashed files and directories live under `{SHOWN_ROOT}/<trash_id>/files/`.")
}

fn lines(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("- `{k}`: {v}\n")).collect()
}

impl Trash {
    pub fn new(home: Home) -> Self {
        Trash { home: Arc::new(home), now: Arc::new(Utc::now) }
    }

    /// Свои часы — для проверки срока.
    pub fn with_clock(home: Home, now: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>) -> Self {
        Trash { home: Arc::new(home), now }
    }

    pub fn add_to(&self, tools: &mut Tools) {
        for op in [Op::Put, Op::List, Op::Restore] {
            tools.add(TrashTool { op, trash: self.clone() });
        }
    }

    fn root(&self) -> PathBuf {
        self.home.dir.join(TRASH_DIR)
    }

    fn entry(&self, id: &str) -> Result<PathBuf, String> {
        let ok = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !ok {
            return Err(format!("invalid trash_id: {id}"));
        }
        Ok(self.root().join(id))
    }

    fn read_info(dir: &Path) -> Result<TrashInfo, String> {
        let p = dir.join(".trashinfo");
        let text = std::fs::read_to_string(&p).map_err(|e| format!("failed to inspect trash entry '{}': {e}", p.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("failed to parse trash metadata '{}': {e}", p.display()))
    }

    fn write_info(dir: &Path, info: &TrashInfo) -> Result<(), String> {
        let text = serde_json::to_string_pretty(info).map_err(|e| format!("failed to serialize trash metadata: {e}"))?;
        std::fs::write(dir.join(".trashinfo"), text).map_err(|e| e.to_string())
    }

    /// Все записи, новые сверху; просроченные активные — в `expired`.
    fn entries(&self) -> Result<Vec<(PathBuf, TrashInfo)>, String> {
        let Ok(rd) = std::fs::read_dir(self.root()) else { return Ok(vec![]) };
        let now = (self.now)();
        let mut out = vec![];
        for e in rd.flatten() {
            let dir = e.path();
            if !dir.is_dir() {
                continue;
            }
            let Ok(mut info) = Self::read_info(&dir) else { continue };
            let due = DateTime::parse_from_rfc3339(&info.expiry_date).map(|t| t.with_timezone(&Utc) <= now).unwrap_or(false);
            if info.status == "active" && due {
                let files = dir.join("files");
                if files.exists() {
                    std::fs::remove_dir_all(&files).map_err(|e| format!("failed to remove expired payload entry '{}': {e}", files.display()))?;
                }
                info.status = "expired".into();
                info.expired_at = Some(stamp(now));
                Self::write_info(&dir, &info)?;
            }
            out.push((dir, info));
        }
        out.sort_by(|a, b| b.1.trash_id.cmp(&a.1.trash_id));
        Ok(out)
    }

    pub fn put(&self, raw: &str) -> Result<String, String> {
        if raw.trim().is_empty() {
            return Err("path is required for trash action `put`".into());
        }
        self.entries()?;
        let path = self.home.resolve(raw)?;
        std::fs::symlink_metadata(&path).map_err(|e| format!("failed to inspect trash entry '{}': {e}", path.display()))?;
        let root = self.root();
        if path.starts_with(&root) || root.starts_with(&path) {
            return Err("refusing to trash the Hamlet trash directory itself".into());
        }
        let now = (self.now)();
        let suffix: [u8; 16] = rand::thread_rng().gen();
        let id = format!("{}-{}", now.format("%Y%m%d-%H%M%S"), suffix.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let dir = root.join(&id);
        let files = dir.join("files");
        std::fs::create_dir_all(&files).map_err(|e| format!("failed to create directory: {e}"))?;
        let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| "trash entry name is not valid UTF-8".to_string())?.to_string();
        let target = files.join(&name);
        if let Err(e) = std::fs::rename(&path, &target) {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!("failed to move '{}' into the trash: {e}", path.display()));
        }
        let info = TrashInfo {
            trash_id: id.clone(),
            original_path: path.display().to_string(),
            trashed_path: target.display().to_string(),
            deletion_date: stamp(now),
            expiry_date: stamp(now + Duration::days(EXPIRY_DAYS)),
            status: "active".into(),
            top_level_entries: vec![name],
            expired_at: None,
            restored_at: None,
            restored_path: None,
        };
        Self::write_info(&dir, &info)?;
        Ok(format!(
            "{}\nRecover with the trash.restore tool using this `trash_id` when needed. Trashed files and directories are inspectable at `{SHOWN_ROOT}/<trash_id>/files/`.",
            lines(&[("trash_id", &info.trash_id), ("original_path", &info.original_path), ("trashed_path", &info.trashed_path), ("expiry_date", &info.expiry_date)])
        ))
    }

    pub fn list(&self) -> Result<String, String> {
        let entries = self.entries()?;
        if entries.is_empty() {
            return Ok("No Hamlet trash entries found.".into());
        }
        let mut out = String::from("| trash_id | status | expiry_date | original_path | top_level_entries |\n|---|---|---|---|---|\n");
        for (_, i) in &entries {
            let names = i.top_level_entries.join(", ").replace('|', "\\|");
            out.push_str(&format!("| `{}` | {} | {} | `{}` | {} |\n", i.trash_id, i.status, i.expiry_date, i.original_path.replace('|', "\\|"), names));
        }
        out.push_str(&format!("\nUse trash.restore with a `trash_id` to recover an active entry when needed. Trashed files and directories live under `{SHOWN_ROOT}/<trash_id>/files/`."));
        Ok(out)
    }

    pub fn restore(&self, id: &str, custom: Option<&str>) -> Result<String, String> {
        if id.trim().is_empty() {
            return Err("trash_id is required for trash action `restore`".into());
        }
        self.entries()?;
        let dir = self.entry(id.trim())?;
        if !dir.is_dir() {
            return Err(format!("failed to resolve trash entry '{id}'\n{}", inspect_hint()));
        }
        let mut info = Self::read_info(&dir)?;
        if info.status != "active" {
            return Err(format!("trash_id '{id}' is not restorable because its status is '{}'", info.status));
        }
        let files = dir.join("files");
        if !files.is_dir() {
            return Err(format!("trash_id '{id}' has no payload directory; it may have expired"));
        }
        let items: Vec<PathBuf> = std::fs::read_dir(&files).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
        let item = match items.as_slice() {
            [] => return Err(format!("trash_id '{id}' has no payload to restore")),
            [one] => one.clone(),
            _ => return Err(format!("trash_id '{id}' contains multiple payload entries; restore only supports one item")),
        };
        let target = match custom.map(str::trim).filter(|s| !s.is_empty()) {
            Some(c) => self.home.resolve(c)?,
            None => PathBuf::from(&info.original_path),
        };
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(format!(
                "restore conflict: '{}' already exists. Inspect that path, choose a new name or directory, and call restore again with custom_path.",
                target.display()
            ));
        }
        let parent = target.parent().ok_or_else(|| format!("restore target has no parent: {}", target.display()))?;
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create directory: {e}"))?;
        std::fs::rename(&item, &target).map_err(|e| format!("failed to restore '{id}' to '{}': {e}", target.display()))?;
        info.status = "restored".into();
        info.restored_at = Some(stamp((self.now)()));
        info.restored_path = Some(target.display().to_string());
        Self::write_info(&dir, &info)?;
        Ok(format!(
            "{}\nThe trash metadata remains at `{SHOWN_ROOT}/<trash_id>/.trashinfo` as an audit record.",
            lines(&[("trash_id", &info.trash_id), ("original_path", &info.original_path), ("restored_path", &target.display().to_string())])
        ))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Put,
    List,
    Restore,
}

struct TrashTool {
    op: Op,
    trash: Trash,
}

fn prop(d: &str) -> Value {
    json!({"type": "string", "description": personal(d)})
}

impl Tool for TrashTool {
    fn spec(&self) -> ToolSpec {
        let (name, description, schema) = match self.op {
            Op::Put => ("trash.put", PUT_DESCRIPTION, json!({"type": "object", "properties": {"path": prop(PUT_PATH)}, "required": ["path"], "additionalProperties": false})),
            Op::List => ("trash.list", LIST_DESCRIPTION, json!({"type": "object", "properties": {}, "additionalProperties": false})),
            Op::Restore => (
                "trash.restore",
                RESTORE_DESCRIPTION,
                json!({"type": "object", "properties": {"trash_id": prop(RESTORE_ID), "custom_path": prop(RESTORE_CUSTOM)}, "required": ["trash_id"], "additionalProperties": false}),
            ),
        };
        ToolSpec { name: name.into(), description: personal(description), input_schema: schema }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let s = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let t = self.trash.clone();
            let op = self.op;
            let (path, id, custom) = (s("path"), s("trash_id"), args.get("custom_path").and_then(Value::as_str).map(String::from));
            let out = tokio::task::spawn_blocking(move || match op {
                Op::Put => t.put(&path),
                Op::List => t.list(),
                Op::Restore => t.restore(&id, custom.as_deref()),
            })
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?
            .map_err(ToolError::Failed)?;
            Ok(vec![ToolResultPart::Text { text: out }])
        })
    }
}
