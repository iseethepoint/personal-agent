//! Пути файловых инструментов — как у muse: относительный путь и `~/…` —
//! от домашней папки агента (у muse это рабочая папка файловых
//! инструментов, `strings-all.txt:59657`), абсолютный — любой («or any
//! absolute path», описание `muse.write`). Своих границ нет: граница — права
//! пользователя ОС, под которым работает агент.

use std::path::{Path, PathBuf};

/// Домашняя папка агента. Задаётся при запуске; в тестах — временная.
#[derive(Debug, Clone)]
pub struct Home {
    pub dir: PathBuf,
}

impl Home {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Home { dir: dir.into() }
    }

    /// Путь модели → путь на диске.
    pub fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("path must not be empty".into());
        }
        Ok(if raw == "~" {
            self.dir.clone()
        } else if let Some(rest) = raw.strip_prefix("~/") {
            self.dir.join(rest)
        } else if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            self.dir.join(raw)
        })
    }
}
