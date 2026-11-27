//! Папка агента — `agent.toml` и пути от неё. Агент — это папка
//! настроек: новая папка — новый агент, без копирования кода (CLAUDE.md,
//! «Шаблон»). Ключи и адрес базы — только в переменных окружения, в папке
//! лишь их имена.
//!
//! Своё: вид `agent.toml` (у muse настройки демона — его конфиги
//! `opt/hatch-private/configs/` и `runtime/provider_config.rs`, форма другая:
//! одна машина — один агент).

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentToml {
    #[serde(default)]
    pub agent: AgentSection,
    #[serde(default)]
    pub paths: Paths,
    pub database: Database,
    #[serde(default)]
    pub browser: BrowserSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSection {
    /// Выбранная модель (`[[models]].id`); нет — `[defaults].model`.
    pub model: Option<String>,
    /// Язык, на котором исполнитель браузера пишет людям.
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Пояс владельца (IANA), если известен.
    pub timezone: Option<String>,
}

fn default_locale() -> String {
    "en-US".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Paths {
    /// Домашняя папка агента: `SOUL.md`, `USER.md`, `MEMORY.md`, `HEARTBEAT.md`…
    pub home: PathBuf,
    /// Системный текст главного агента (тексты muse — П5).
    pub system: PathBuf,
    pub models: PathBuf,
    pub web: PathBuf,
    /// Системные навыки и коннекторы.
    pub skills: PathBuf,
    pub connectors: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        Paths {
            home: "home".into(),
            system: "system.md".into(),
            models: "config/models.toml".into(),
            web: "config/web.toml".into(),
            skills: "skills".into(),
            connectors: "connectors".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Database {
    /// Имя переменной окружения с адресом Postgres.
    pub url_env: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserSection {
    /// Путь к Chromium; нет — `HAMLET_CHROMIUM` или известные места.
    pub chromium: Option<PathBuf>,
    #[serde(default = "yes")]
    pub headless: bool,
}

impl Default for BrowserSection {
    fn default() -> Self {
        BrowserSection { chromium: None, headless: true }
    }
}

fn yes() -> bool {
    true
}

/// Папка агента с разобранным `agent.toml`; пути — уже от папки.
#[derive(Debug, Clone)]
pub struct AgentDir {
    pub dir: PathBuf,
    pub toml: AgentToml,
}

impl AgentDir {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let file = dir.join("agent.toml");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let toml: AgentToml = toml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(AgentDir { dir: dir.to_path_buf(), toml })
    }

    pub fn path(&self, p: &Path) -> PathBuf {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.dir.join(p)
        }
    }

    pub fn database_url(&self) -> Result<String, String> {
        let var = &self.toml.database.url_env;
        std::env::var(var).map_err(|_| format!("переменная `{var}` с адресом базы не задана"))
    }
}
