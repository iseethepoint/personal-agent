//! Разрешённые папки агента — разметка muse (`strings-all.txt:59657-59690`):
//! домашняя папка `~` (с `~/workspace`) и временная `/tmp`. Относительный
//! путь и `~/…` считаются от `~` — у muse это рабочая папка файловых
//! инструментов. За пределы этих двух папок файловые инструменты не выходят:
//! ни через `..`, ни через символьные ссылки.

use std::path::{Component, Path, PathBuf};

/// Папки агента. Задаются при запуске; в тестах — временные.
#[derive(Debug, Clone)]
pub struct Roots {
    pub home: PathBuf,
    pub tmp: PathBuf,
}

impl Roots {
    pub fn new(home: impl Into<PathBuf>, tmp: impl Into<PathBuf>) -> Self {
        Roots { home: home.into(), tmp: tmp.into() }
    }

    /// Путь модели → путь на диске внутри разрешённых папок.
    pub fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("path must not be empty".into());
        }
        let path = if raw == "~" {
            self.home.clone()
        } else if let Some(rest) = raw.strip_prefix("~/") {
            self.home.join(rest)
        } else if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            self.home.join(raw)
        };
        let path = normalize(&path);
        let real = real_path(&path);
        let inside = |root: &Path| {
            let root = real_path(&normalize(root));
            real.starts_with(&root)
        };
        if inside(&self.home) || inside(&self.tmp) {
            Ok(path)
        } else {
            Err(format!("path `{raw}` is outside the allowed directories (`~` and `/tmp`)"))
        }
    }
}

/// `.` и `..` без обращения к диску.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Настоящий путь: ближайший существующий предок — через ссылки, остаток —
/// как есть. Так ссылка внутри `~`, ведущая наружу, не пропускается.
fn real_path(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}
