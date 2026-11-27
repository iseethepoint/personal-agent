//! Реестр моделей — по форме muse `model_registry::registry_file`
//! (muse-live `core-v2-analysis/strings-all.txt:235532-237160`):
//! `[defaults]` — модели по умолчанию, `[weights.NAME]` — окно и умения весов,
//! `[wires]` — имя модели у поставщика → веса, `[[models]]` — выбираемые модели
//! с `setup` (переопределения по задачам).
//!
//! Умение, которого нет в весах, выключено. Незнакомое поле — ошибка загрузки,
//! чтобы опечатка не выключала умение молча.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

/// Задача, под которую выбирается модель. Имена — как у muse
/// (`strings-all.txt:56281`, `:237127-237137`, `:237150-237151`;
/// `MediaDescription` — `:236113`).
/// `Main` и `Embeddings` — свои: у muse главный разговор идёт на выбранной
/// модели, а эмбеддинги — отдельная служба (опись, раздел 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Purpose {
    Main,
    Compaction,
    BackgroundCompaction,
    MemoryFlush,
    BackgroundWorker,
    CronWorker,
    SelfImprovementStep,
    BrowserAgent,
    BrowserMain,
    MediaDescription,
    Embeddings,
}

impl Purpose {
    /// Задачи модели `[defaults].maintenance`: у muse это «artifact builds,
    /// compaction, memory flush and forget» (`strings-all.txt:235538-235540`).
    pub fn is_maintenance(self) -> bool {
        matches!(
            self,
            Purpose::Compaction | Purpose::BackgroundCompaction | Purpose::MemoryFlush
        )
    }
}

/// Поставщик, с которым говорит прослойка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    OpenaiCompat,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    /// Модель главного разговора и всех задач без переопределения.
    pub model: String,
    /// Модель сжатия и сброса памяти.
    pub maintenance: String,
    /// Модель эмбеддингов. Нет — векторный поиск выключен, остаётся поиск
    /// по словам (форма muse: «lexical-only», `strings-all.txt:55090`).
    #[serde(default)]
    pub embeddings: Option<String>,
}

/// Сила ответа у Anthropic (`anthropic.output_effort`, `strings-all.txt:235804`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputEffort {
    #[default]
    Unsupported,
    Supported,
    XhighAsMax,
}

/// Разрешение картинок у Anthropic (`anthropic.image_resolution`, `strings-all.txt:235805`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResolution {
    #[default]
    Standard,
    High,
}

/// Умения весов при поставщике `anthropic` — имена как у muse
/// (`strings-all.txt:235799-235806`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AnthropicFacts {
    pub adaptive_thinking: bool,
    pub omit_thinking_display: bool,
    pub mid_conversation_system_messages: bool,
    pub compaction: bool,
    pub tool_search: bool,
    pub output_effort: OutputEffort,
    pub image_resolution: ImageResolution,
    pub runtime_output_ceiling: Option<u32>,
    pub beta: Option<String>,
    /// `cache_control` на неизменном начале подсказки (system и инструменты).
    /// Своё имя: в списке умений весов muse такого флага нет.
    pub prompt_cache: bool,
    /// Принудительный вызов инструмента (`tool_choice` `tool`/`any`).
    /// На claude-opus-5-5, Sonnet 5.5, Fable 5.1/5.2 сервер отвечает 400 —
    /// там умение выключено. Своё имя: у muse такого флага нет.
    pub forced_tool_choice: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

/// Умения весов при поставщике `openai_compat`. Имена — по флагам muse
/// для весов не-Anthropic (`magi.*`, `strings-all.txt:235581-235588`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct OpenaiCompatFacts {
    /// Картинки на входе (`image_url`).
    pub vision: bool,
    /// Какие форматы картинок принимает сервер (у muse `magi.png_or_jpeg_images_only`).
    pub image_formats: Vec<ImageFormat>,
    /// Сервер понимает признак ошибки результата инструмента (`magi.tool_output_error_status`).
    pub tool_output_error_status: bool,
    /// Мышление: `reasoning_effort` / `reasoning_content` (`magi.reasoning`).
    pub reasoning: bool,
    /// Несколько вызовов инструментов в одном ответе (`parallel_tool_calls`).
    pub parallel_tool_calls: bool,
    /// Роль `developer`/`system` посреди разговора (`magi.developer_role`).
    pub developer_role: bool,
    /// Предел ответа полем `max_completion_tokens` вместо `max_tokens`: так
    /// требуют нынешние модели OpenAI (GPT-5.x, рассуждающие), а локальные
    /// серверы понимают `max_tokens`. Своё умение: у muse его нет
    /// (в `strings-all.txt` поле `max_completion_tokens` не встречается).
    pub max_completion_tokens: bool,
    pub runtime_output_ceiling: Option<u32>,
}

/// Одни веса: окно, порог сжатия, умения (`[weights.NAME]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weights {
    #[serde(default)]
    pub knowledge_cutoff: Option<String>,
    pub context_window: u64,
    /// Порог сжатия в токенах; не больше окна — как проверка muse
    /// в функции `0x20954580` (опись, раздел 2.9).
    pub compaction_trigger: u64,
    #[serde(default)]
    pub anthropic: Option<AnthropicFacts>,
    #[serde(default)]
    pub openai_compat: Option<OpenaiCompatFacts>,
}

/// Имя модели у поставщика → веса (`[wires]`). `local_inference` — как у muse
/// (`strings-all.txt:235901`): модель крутится на своей машине.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub weights: String,
    #[serde(default)]
    pub local_inference: bool,
}

/// Переопределение модели для задачи (`setup.overrides`, `strings-all.txt:237149-237151`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurposeOverride {
    pub purpose: Purpose,
    pub model: String,
}

/// `setup` выбираемой модели: отклонения от обычного выбора.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Setup {
    pub overrides: Vec<PurposeOverride>,
    /// Задачи, которые идут на `[defaults].model`, даже если выбрана эта модель
    /// (`setup.default_model_purposes`, `strings-all.txt:237127`).
    pub default_model_purposes: Vec<Purpose>,
    /// Сжатие и сброс памяти — на этой модели, а не на `maintenance`
    /// (`setup.compaction_memory_flush_use_main`, `strings-all.txt:236111`).
    pub compaction_memory_flush_use_main: bool,
}

/// Выбираемая модель (`[[models]]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelOption {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub provider: ProviderKind,
    /// Имя модели у поставщика; ключ в `[wires]`.
    pub model: String,
    /// Адрес сервера; обязателен для `openai_compat`.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Имя переменной окружения с ключом. Сам ключ в реестр не пишется.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub setup: Setup,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    defaults: Defaults,
    #[serde(default)]
    weights: BTreeMap<String, Weights>,
    #[serde(default)]
    wires: BTreeMap<String, Wire>,
    #[serde(default)]
    models: Vec<ModelOption>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RegistryError {
    #[error("реестр не читается: {0}")]
    Read(String),
    #[error("реестр не разбирается: {0}")]
    Parse(String),
    #[error("модель `{id}` записана дважды")]
    DuplicateModel { id: String },
    #[error("{place} ссылается на модель `{id}`, которой нет в [[models]]")]
    UnknownModel { place: String, id: String },
    #[error("задача {purpose:?} без модели: {reason}")]
    PurposeWithoutModel { purpose: Purpose, reason: String },
    #[error("модель `{model}`: имени `{wire}` нет в [wires]")]
    UnknownWire { model: String, wire: String },
    #[error("[wires] `{wire}`: весов `{weights}` нет в [weights]")]
    UnknownWeights { wire: String, weights: String },
    #[error("веса `{weights}`: порог сжатия {trigger} больше окна {window}")]
    TriggerOverWindow {
        weights: String,
        trigger: u64,
        window: u64,
    },
    #[error("модель `{model}` (openai_compat) без base_url")]
    MissingBaseUrl { model: String },
    #[error("модель `{model}` ({provider:?}): в весах `{weights}` умения чужого поставщика")]
    ForeignFacts {
        model: String,
        provider: ProviderKind,
        weights: String,
    },
}

/// Итог выбора: модель, её имя у поставщика и веса.
#[derive(Debug, Clone, Copy)]
pub struct Resolved<'a> {
    pub option: &'a ModelOption,
    pub wire: &'a Wire,
    pub weights: &'a Weights,
}

#[derive(Debug, Clone)]
pub struct Registry {
    defaults: Defaults,
    weights: BTreeMap<String, Weights>,
    wires: BTreeMap<String, Wire>,
    models: BTreeMap<String, ModelOption>,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| RegistryError::Read(format!("{}: {e}", path.display())))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, RegistryError> {
        let file: RegistryFile =
            toml::from_str(text).map_err(|e| RegistryError::Parse(e.to_string()))?;
        let mut models = BTreeMap::new();
        for m in file.models {
            if models.contains_key(&m.id) {
                return Err(RegistryError::DuplicateModel { id: m.id });
            }
            models.insert(m.id.clone(), m);
        }
        let reg = Registry {
            defaults: file.defaults,
            weights: file.weights,
            wires: file.wires,
            models,
        };
        reg.check()?;
        Ok(reg)
    }

    fn check(&self) -> Result<(), RegistryError> {
        let need = |place: &str, id: &str| -> Result<(), RegistryError> {
            if self.models.contains_key(id) {
                Ok(())
            } else {
                Err(RegistryError::UnknownModel {
                    place: place.to_string(),
                    id: id.to_string(),
                })
            }
        };
        if self.defaults.model.is_empty() {
            return Err(RegistryError::PurposeWithoutModel {
                purpose: Purpose::Main,
                reason: "[defaults].model пуст".into(),
            });
        }
        if self.defaults.maintenance.is_empty() {
            return Err(RegistryError::PurposeWithoutModel {
                purpose: Purpose::Compaction,
                reason: "[defaults].maintenance пуст".into(),
            });
        }
        need("[defaults].model", &self.defaults.model)?;
        need("[defaults].maintenance", &self.defaults.maintenance)?;
        if let Some(e) = &self.defaults.embeddings {
            need("[defaults].embeddings", e)?;
        }
        for (name, w) in &self.weights {
            if w.compaction_trigger > w.context_window {
                return Err(RegistryError::TriggerOverWindow {
                    weights: name.clone(),
                    trigger: w.compaction_trigger,
                    window: w.context_window,
                });
            }
        }
        for (wire, w) in &self.wires {
            if !self.weights.contains_key(&w.weights) {
                return Err(RegistryError::UnknownWeights {
                    wire: wire.clone(),
                    weights: w.weights.clone(),
                });
            }
        }
        for m in self.models.values() {
            let wire = self
                .wires
                .get(&m.model)
                .ok_or_else(|| RegistryError::UnknownWire {
                    model: m.id.clone(),
                    wire: m.model.clone(),
                })?;
            let w = &self.weights[&wire.weights];
            let foreign = match m.provider {
                ProviderKind::Anthropic => w.openai_compat.is_some(),
                ProviderKind::OpenaiCompat => w.anthropic.is_some(),
            };
            if foreign {
                return Err(RegistryError::ForeignFacts {
                    model: m.id.clone(),
                    provider: m.provider,
                    weights: wire.weights.clone(),
                });
            }
            if m.provider == ProviderKind::OpenaiCompat
                && m.base_url.as_deref().is_none_or(str::is_empty)
            {
                return Err(RegistryError::MissingBaseUrl {
                    model: m.id.clone(),
                });
            }
            let mut seen = BTreeSet::new();
            for o in &m.setup.overrides {
                need(&format!("`{}` setup.overrides", m.id), &o.model)?;
                if !seen.insert(o.purpose) {
                    return Err(RegistryError::PurposeWithoutModel {
                        purpose: o.purpose,
                        reason: format!("в `{}` setup.overrides задача записана дважды", m.id),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn defaults(&self) -> &Defaults {
        &self.defaults
    }

    /// Имена переменных окружения с ключами всех моделей реестра — их
    /// значения скрываются в выводе команд (`redaction.rs` muse).
    pub fn api_key_envs(&self) -> Vec<String> {
        let mut names: Vec<String> = self.models.values().filter_map(|m| m.api_key_env.clone()).collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn model(&self, id: &str) -> Option<&ModelOption> {
        self.models.get(id)
    }

    fn resolved(&self, id: &str) -> Resolved<'_> {
        // Ссылки проверены в `check`, поэтому здесь индексация не падает.
        let option = &self.models[id];
        let wire = &self.wires[&option.model];
        let weights = &self.weights[&wire.weights];
        Resolved {
            option,
            wire,
            weights,
        }
    }

    /// Модель для задачи. `selection` — выбранная модель (у muse — выбор
    /// владельца или закрепление); `None` — `[defaults].model`.
    ///
    /// Порядок как у muse: переопределение выбранной модели →
    /// модель обслуживания для сжатия и сброса памяти →
    /// `default_model_purposes` → выбранная модель.
    /// `Ok(None)` — только для `Embeddings` без модели: умение выключено.
    pub fn resolve(
        &self,
        purpose: Purpose,
        selection: Option<&str>,
    ) -> Result<Option<Resolved<'_>>, RegistryError> {
        let selected_id = selection.unwrap_or(&self.defaults.model);
        let selected = self
            .models
            .get(selected_id)
            .ok_or_else(|| RegistryError::UnknownModel {
                place: "выбор модели".into(),
                id: selected_id.to_string(),
            })?;
        if let Some(o) = selected.setup.overrides.iter().find(|o| o.purpose == purpose) {
            return Ok(Some(self.resolved(&o.model)));
        }
        if purpose == Purpose::Embeddings {
            return Ok(self.defaults.embeddings.as_deref().map(|id| self.resolved(id)));
        }
        if purpose.is_maintenance() && !selected.setup.compaction_memory_flush_use_main {
            return Ok(Some(self.resolved(&self.defaults.maintenance)));
        }
        if selected.setup.default_model_purposes.contains(&purpose) {
            return Ok(Some(self.resolved(&self.defaults.model)));
        }
        Ok(Some(self.resolved(selected_id)))
    }
}
