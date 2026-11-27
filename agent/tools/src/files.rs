//! `muse.read`, `muse.write`, `muse.edit` — по форме muse `file_io*`
//! (`source-files.txt:1571-1579`), `edit.rs`, `edit_matching.rs` (`:1552-1553`).
//!
//! Описания и параметры — строки ядра muse, полные варианты, личный вариант:
//! `muse.write` и `muse.read` — 0x878143–0x878950, `muse.edit` — 0xa3ee6c–0xa3f121.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools};
use agent_model::{Image, ToolResultPart, ToolSpec};
use base64::Engine;
use serde_json::{json, Value};

use crate::paths::Roots;

const READ_DESCRIPTION: &str = "Read file contents or list directory contents. If the path is a directory, returns a sorted listing of non-hidden files and directories up to two levels deep. Text files are read as UTF-8 and returned as whole lines only, with optional 1-indexed offset and line limit parameters; output is bounded to avoid oversized responses and may include continuation guidance. For supported images (png/jpeg/gif/webp), injects the image into model context. Document files (PDF, Word, Excel, PowerPoint, OpenDocument, EPUB, RTF) are converted to GitHub-Flavored Markdown and paged like text. For reading a PDF, prefer reviewing pages visually: render them with the documents skill and read the images. Use the extracted text for wording and quick checks, not to understand layout, tables or figures. If you can't render the pages, say what you couldn't check.";
const READ_PATH: &str = "File or directory path: relative to your working directory, or absolute under /tmp.";
const READ_OFFSET: &str = "Optional 1-indexed starting line for text files. Defaults to 1.";
const READ_LIMIT: &str = "The maximum number of lines to return for a text file. Optional.";

const WRITE_DESCRIPTION: &str = "Write content to a file. For a small change to an existing file, use `muse.edit`. Creates the file if it does not exist. Default mode overwrites the full file; for long or incremental outputs, split content across multiple calls and use mode=\"append\" so you add chunks instead of replacing prior content. Overwriting a file that already has content fails unless the file was successfully read earlier in the conversation.";
const WRITE_PATH: &str = "File path: relative to your working directory, or any absolute path.";
const WRITE_CONTENT: &str = "Content to write.";
const WRITE_MODE: &str = "Write mode. 'overwrite' (default): replace file contents. 'append': add to end of file, creating it if needed; prefer this for second-and-later chunks of long outputs.";

const EDIT_DESCRIPTION: &str = "Edit a file by replacing exact text. `old_text` must match exactly, including whitespace. Use this for precise, surgical edits. For a new file or a full rewrite, use `muse.write`.";
const EDIT_PATH: &str = "File path: relative to your working directory, or absolute under /tmp.";
const EDIT_OLD: &str = "Exact text to find (first occurrence is replaced; a multi-match result lists the other locations). Must not be empty.";
const EDIT_NEW: &str = "Replacement text. Can be empty to delete old_text.";

/// Предел вывода `muse.read` за вызов. Числа в ядре не видно (функция
/// постраничного чтения 0x26e00e60 получает его аргументом) — взят порог
/// большого вывода из П3а, 100 КБ.
pub const READ_CAP_BYTES: usize = agent_loop::SPILL_THRESHOLD_BYTES;

fn prop(t: &str, d: &str) -> Value {
    json!({"type": t, "description": personal(d)})
}

/// Файловые инструменты одного разговора: общие папки и память о том, что
/// уже прочитано (сторож перезаписи, `overwrite_guard.rs`).
#[derive(Clone)]
pub struct FileTools {
    roots: Arc<Roots>,
    read: Arc<Mutex<HashSet<PathBuf>>>,
}

impl FileTools {
    pub fn new(roots: Roots) -> Self {
        FileTools { roots: Arc::new(roots), read: Arc::new(Mutex::new(HashSet::new())) }
    }

    /// Добавить `muse.read`, `muse.write`, `muse.edit` в реестр.
    pub fn add_to(&self, tools: &mut Tools) {
        tools.add(Read(self.clone())).add(Write(self.clone())).add(Edit(self.clone()));
    }

    fn resolve(&self, args: &Value) -> Result<PathBuf, ToolError> {
        let raw = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("`path` (string) is required".into()))?;
        self.roots.resolve(raw).map_err(ToolError::Failed)
    }
}

fn text(t: String) -> Result<Vec<ToolResultPart>, ToolError> {
    Ok(vec![ToolResultPart::Text { text: t }])
}

fn io(path: &Path, e: std::io::Error) -> ToolError {
    ToolError::Failed(format!("{}: {e}", path.display()))
}

struct Read(FileTools);
struct Write(FileTools);
struct Edit(FileTools);

impl Tool for Read {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "muse.read".into(),
            description: personal(READ_DESCRIPTION),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": prop("string", READ_PATH),
                    "offset": prop("integer", READ_OFFSET),
                    "limit": prop("integer", READ_LIMIT),
                },
                "required": ["path"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let path = self.0.resolve(&args)?;
            let meta = tokio::fs::metadata(&path).await.map_err(|e| io(&path, e))?;
            if meta.is_dir() {
                return text(list_dir(&path));
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let bytes = tokio::fs::read(&path).await.map_err(|e| io(&path, e))?;
            if let Some(media_type) = image_type(&ext) {
                self.0.read.lock().unwrap().insert(path);
                let data_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(vec![ToolResultPart::Image(Image { media_type: media_type.into(), data_base64 })]);
            }
            if DOCUMENTS.contains(&ext.as_str()) {
                return Err(ToolError::Failed(format!(
                    "converting .{ext} documents to Markdown is not available yet"
                )));
            }
            let body = String::from_utf8(bytes)
                .map_err(|_| ToolError::Failed(format!("{} is not UTF-8 text", path.display())))?;
            let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
            let limit = args.get("limit").and_then(Value::as_u64).map(|n| n as usize);
            self.0.read.lock().unwrap().insert(path);
            text(page(&body, offset, limit))
        })
    }
}

const DOCUMENTS: [&str; 12] = ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "epub", "rtf"];

fn image_type(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Целые строки с `offset` (с 1), не больше `limit` строк и не больше
/// `READ_CAP_BYTES`; дальше — указание muse, с какой строки продолжить
/// (строки ядра 0x404d1d, 0x404d66).
pub fn page(body: &str, offset: usize, limit: Option<usize>) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();
    let start = offset.min(total + 1);
    let mut out = String::new();
    let mut end = start - 1; // последняя показанная строка (с 1)
    let mut capped = false;
    for (i, line) in lines.iter().enumerate().skip(start - 1) {
        if limit.is_some_and(|l| i + 1 - (start - 1) > l) {
            break;
        }
        if !out.is_empty() && out.len() + 1 + line.len() > READ_CAP_BYTES {
            capped = true;
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        end = i + 1;
    }
    if end < total {
        let next = end + 1;
        let note = if capped {
            format!("[Showing lines {start}-{end} of {total} ({}KB limit). Use offset={next} to continue.]", READ_CAP_BYTES / 1024)
        } else {
            format!("[{} more lines in file. Use offset={next} to continue.]", total - end)
        };
        out.push_str("\n\n");
        out.push_str(&note);
    }
    out
}

/// Отсортированный список видимых файлов и папок на два уровня.
fn list_dir(path: &Path) -> String {
    fn entries(path: &Path) -> Vec<(String, PathBuf, bool)> {
        let mut v: Vec<(String, PathBuf, bool)> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                (!name.starts_with('.')).then(|| {
                    let dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    (name, e.path(), dir)
                })
            })
            .collect();
        v.sort();
        v
    }
    let mut out = Vec::new();
    for (name, p, dir) in entries(path) {
        out.push(if dir { format!("{name}/") } else { name });
        if dir {
            for (child, _, cdir) in entries(&p) {
                out.push(format!("  {child}{}", if cdir { "/" } else { "" }));
            }
        }
    }
    out.join("\n")
}

impl Tool for Write {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "muse.write".into(),
            description: personal(WRITE_DESCRIPTION),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": prop("string", WRITE_PATH),
                    "content": prop("string", WRITE_CONTENT),
                    "mode": {"type": "string", "enum": ["overwrite", "append"], "description": personal(WRITE_MODE)},
                },
                "required": ["path", "content"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let path = self.0.resolve(&args)?;
            let content = args
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("`content` (string) is required".into()))?;
            let append = match args.get("mode").and_then(Value::as_str).unwrap_or("overwrite") {
                "overwrite" => false,
                "append" => true,
                other => return Err(ToolError::InvalidArguments(format!("`mode` must be 'overwrite' or 'append', not '{other}'"))),
            };
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| io(parent, e))?;
            }
            if append {
                use tokio::io::AsyncWriteExt;
                let mut f = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .await
                    .map_err(|e| io(&path, e))?;
                f.write_all(content.as_bytes()).await.map_err(|e| io(&path, e))?;
            } else {
                let has_content = tokio::fs::metadata(&path).await.map(|m| m.len() > 0).unwrap_or(false);
                if has_content && !self.0.read.lock().unwrap().contains(&path) {
                    return Err(ToolError::Failed(format!(
                        "{} already has content and was not read in this conversation: read it with `muse.read` before overwriting, or use `muse.edit` or mode=\"append\"",
                        path.display()
                    )));
                }
                tokio::fs::write(&path, content).await.map_err(|e| io(&path, e))?;
            }
            // Записанное нами считается известным: можно перезаписать дальше.
            self.0.read.lock().unwrap().insert(path.clone());
            text(format!("Wrote {} bytes to {}", content.len(), path.display()))
        })
    }
}

impl Tool for Edit {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "muse.edit".into(),
            description: personal(EDIT_DESCRIPTION),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": prop("string", EDIT_PATH),
                    "old_text": prop("string", EDIT_OLD),
                    "new_text": prop("string", EDIT_NEW),
                },
                "required": ["path", "old_text", "new_text"],
            }),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let path = self.0.resolve(&args)?;
            let s = |k: &str| {
                args.get(k)
                    .and_then(Value::as_str)
                    .ok_or_else(|| ToolError::InvalidArguments(format!("`{k}` (string) is required")))
            };
            let (old, new) = (s("old_text")?, s("new_text")?);
            if old.is_empty() {
                return Err(ToolError::InvalidArguments("`old_text` must not be empty".into()));
            }
            let body = tokio::fs::read_to_string(&path).await.map_err(|e| io(&path, e))?;
            let at: Vec<usize> = body.match_indices(old).map(|(i, _)| i).collect();
            let Some(&first) = at.first() else {
                // Текст muse, `strings-all.txt:30812`.
                return Err(ToolError::Failed(format!(
                    "text not found in '{}': old_text was not present",
                    path.display()
                )));
            };
            let edited = format!("{}{new}{}", &body[..first], &body[first + old.len()..]);
            tokio::fs::write(&path, &edited).await.map_err(|e| io(&path, e))?;
            let mut out = format!("Edited {}", path.display());
            if at.len() > 1 {
                // Текст muse (строка ядра 0x9442ba): заменено первое.
                let lines: Vec<String> = at.iter().map(|&i| (body[..i].matches('\n').count() + 1).to_string()).collect();
                out.push_str(&format!(
                    "\nnote: old_text matched {} locations (lines {}); the first was replaced. If you meant a different occurrence, edit again with enough surrounding context to make the match unique.",
                    at.len(),
                    lines.join(", ")
                ));
            }
            text(out)
        })
    }
}
