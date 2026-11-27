//! Навыки и коннекторы — `muse.skill_search` (`skill_search.rs`,
//! `source-files.txt:1735`) и `muse.connector_search` (`connector_search.rs`,
//! `:1538`). Всё на файлах:
//!
//! - навык — папка с `skill.yaml` (name, title, description, category,
//!   include_in_prompt) и `SKILL.md`; коннектор — `connector.yaml` (id,
//!   title, description, aliases, include_in_prompt) и `CONNECTOR.md`;
//! - системные — только читаются (у muse `/opt/muse/skills`,
//!   `/opt/hatch/connectors`; у нас путь задаётся при запуске), свои —
//!   `~/workspace/custom-skills/`, `~/workspace/custom-connectors/`
//!   (`strings-all.txt:59686-59689`);
//! - свой с тем же именем — поправка поверх системного: описание своё, а
//!   путь системного и путь поправки идут вместе («Read useful skills and
//!   their listed local amendments», строка ядра 0x6d3896);
//! - поиск — `bm25` по словам (по умолчанию) или `regex` (описание muse);
//! - файл перечитывается, если изменился (форма `skill_invalidation`, без базы).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools};
use agent_model::{ToolResultPart, ToolSpec};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::paths::Home;

// Описания — строки ядра muse, полные варианты, личный вариант:
// `muse.skill_search` — 0x86c75d–0x86cb92, `muse.connector_search` — 0xa3b2b6–0xa3b64a.
// Правки (П10): `muse.*` → `hamlet.*`; из описания поиска навыков убран путь
// « in `/opt/muse/skills`» — системная папка навыков у нас задаётся при запуске.
const SKILL_DESCRIPTION: &str = "Search the full skill catalog, including bundled/preinstalled product skills and custom skills in `~/workspace/custom-skills`, for a task or capability when the skills in context don't clearly fit, before concluding it's unavailable; find products and services with hamlet.connector_search. `bm25` (default) ranks by keyword; `regex` matches a case-insensitive pattern over skill names and descriptors. Use multiple queries for different angles or options. Returns names, paths, and full skill descriptions (not full SKILL.md bodies). This is high-recall, so hits are candidates: read a matching `SKILL.md` to decide if it is relevant and should be used. If none fit, consider alternative directions: use the `custom_connector_setup` skill's template to build a skill for a reusable workflow.";
const SKILL_QUERIES: &str = "Up to 5 independent searches. Each returns its own ranked list; results are never blended across queries. In bm25 mode, keywords or a short phrase describing the task or capability; in regex mode, a regular expression matched against each skill's name, descriptor and instructions. Batch the tasks or capabilities you're checking instead of guessing one term.";
const SKILL_MODE: &str = "How to interpret every query: \"bm25\" (default) for keyword ranking, or \"regex\" for a case-insensitive regular-expression match.";
const SKILL_MAX: &str = "Maximum number of skill hits per query. Defaults to 8, capped at 25.";
const CONNECTOR_DESCRIPTION: &str = "Find connectors by name, alias or the action you need. Searches their names, aliases and descriptions, and returns each match's name, docs path and description. Read the docs for setup and usage. To find <!-- personal-muse-only:start -->the user's <!-- personal-muse-only:end -->messages, notes or files<!-- personal-muse-only:start -->, use the app's<!-- personal-muse-only:end --><!-- group-muse-only:start --> in a connected account, use that service's<!-- group-muse-only:end --> own search. This tool finds connector docs and does not connect accounts or perform actions.";
const CONNECTOR_QUERIES: &str = "Up to 5 searches by connector name, alias or the operation you need. Each returns its own ranked list.";
const CONNECTOR_MAX: &str = "Connectors to return per query. Defaults to 8.";

// Заметки к результату — строки ядра 0x6d3837, 0x6d3896, 0x6d60c4.
const SKILL_HITS_NOTE: &str = "Read useful skills and their listed local amendments. Combine or adapt them to the task. Use hamlet.connector_search for docs about connecting and using apps or services.";
const SKILL_MISS_NOTE: &str = "No matching skill was found. Adapt related skills and use connector docs, available tools or research to complete the task. Save a useful method as a custom skill. A search miss does not establish that a capability is unavailable.";
const CONNECTOR_NOTE: &str = "Read the matching connector docs for setup and usage, then check the connection when the task needs it. Use the app's own operations to find <!-- personal-muse-only:start -->user<!-- personal-muse-only:end --><!-- group-muse-only:start -->the group's<!-- group-muse-only:end --> content. Use hamlet.skill_search to find skills.";

const MAX_QUERIES: usize = 5;
const DEFAULT_HITS: usize = 8;
const MAX_HITS: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skill,
    Connector,
}

impl Kind {
    fn files(self) -> (&'static str, &'static str) {
        match self {
            Kind::Skill => ("skill.yaml", "SKILL.md"),
            Kind::Connector => ("connector.yaml", "CONNECTOR.md"),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct Meta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    include_in_prompt: bool,
}

/// Навык или коннектор каталога.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub title: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub include_in_prompt: bool,
    /// `SKILL.md` / `CONNECTOR.md`.
    pub doc: PathBuf,
    /// Текст doc-файла — для regex по «instructions».
    pub body: String,
    pub custom: bool,
}

/// Каталог одного вида: системная папка и своя. Разобранные файлы
/// держатся в памяти и перечитываются, если файл изменился.
#[derive(Clone)]
pub struct Catalog {
    kind: Kind,
    system: PathBuf,
    custom: PathBuf,
    cache: Arc<Mutex<Cache>>,
}

/// Отметка файлов записи: время и длина описания и doc-файла.
type Stamp = [(Option<SystemTime>, u64); 2];
/// Разобранные записи по пути описания.
type Cache = HashMap<PathBuf, (Stamp, Option<Entry>)>;

fn stamp(p: &Path) -> (Option<SystemTime>, u64) {
    std::fs::metadata(p).map(|m| (m.modified().ok(), m.len())).unwrap_or((None, 0))
}

/// Найденное: поправка поверх системного.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub entry: Entry,
    /// Системный doc под поправкой.
    pub base: Option<PathBuf>,
}

impl Catalog {
    pub fn skills(system: impl Into<PathBuf>, home: &Home) -> Self {
        Self::new(Kind::Skill, system.into(), home.dir.join("workspace/custom-skills"))
    }

    pub fn connectors(system: impl Into<PathBuf>, home: &Home) -> Self {
        Self::new(Kind::Connector, system.into(), home.dir.join("workspace/custom-connectors"))
    }

    fn new(kind: Kind, system: PathBuf, custom: PathBuf) -> Self {
        Catalog { kind, system, custom, cache: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn load(&self, meta_path: &Path, custom: bool) -> Option<Entry> {
        let (_, docname) = self.kind.files();
        let doc = meta_path.with_file_name(docname);
        let now: Stamp = [stamp(meta_path), stamp(&doc)];
        let mut cache = self.cache.lock().unwrap();
        if let Some((was, e)) = cache.get(meta_path) {
            if *was == now {
                return e.clone();
            }
        }
        let entry = (|| {
            let meta: Meta = serde_yaml::from_str(&std::fs::read_to_string(meta_path).ok()?).ok()?;
            let dir_name = meta_path.parent()?.file_name()?.to_string_lossy().to_string();
            let name = meta.name.or(meta.id).unwrap_or(dir_name);
            let body = std::fs::read_to_string(&doc).unwrap_or_default();
            Some(Entry {
                title: meta.title.unwrap_or_else(|| name.clone()),
                name,
                description: meta.description.unwrap_or_default(),
                aliases: meta.aliases,
                include_in_prompt: meta.include_in_prompt,
                doc,
                body,
                custom,
            })
        })();
        cache.insert(meta_path.to_path_buf(), (now, entry.clone()));
        entry
    }

    fn scan(&self, root: &Path, custom: bool, out: &mut Vec<Entry>) {
        let (yaml, _) = self.kind.files();
        let Ok(rd) = std::fs::read_dir(root) else { return };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            let meta = d.join(yaml);
            if meta.is_file() {
                if let Some(e) = self.load(&meta, custom) {
                    out.push(e);
                }
            } else {
                // Навыки бывают вложены группами (`artifacts/testing/`).
                self.scan(&d, custom, out);
            }
        }
    }

    /// Весь каталог: свой с тем же именем — поверх системного.
    pub fn items(&self) -> Vec<Item> {
        let (mut sys, mut own) = (Vec::new(), Vec::new());
        self.scan(&self.system, false, &mut sys);
        self.scan(&self.custom, true, &mut own);
        let mut items: Vec<Item> = Vec::new();
        for e in sys {
            items.push(Item { entry: e, base: None });
        }
        for e in own {
            match items.iter_mut().find(|i| i.entry.name == e.name) {
                Some(i) => {
                    let base = i.entry.doc.clone();
                    *i = Item { entry: e, base: Some(base) };
                }
                None => items.push(Item { entry: e, base: None }),
            }
        }
        items
    }

    /// Строки для неизменного начала подсказки: навыки с `include_in_prompt`.
    /// Текст блока muse переносится в П5; здесь — только список.
    pub fn prompt_listing(&self) -> String {
        self.items()
            .iter()
            .filter(|i| i.entry.include_in_prompt)
            .map(|i| format!("- `{}`: {} ({})", i.entry.name, i.entry.description, i.entry.doc.display()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Поиск: `bm25` по словам или `regex` без учёта регистра.
    pub fn search(&self, query: &str, regex: bool, limit: usize) -> Result<Vec<Item>, String> {
        let items = self.items();
        if regex {
            let re = regex::RegexBuilder::new(query)
                .case_insensitive(true)
                .build()
                .map_err(|e| format!("invalid regex `{query}`: {e}"))?;
            return Ok(items
                .into_iter()
                .filter(|i| {
                    let e = &i.entry;
                    re.is_match(&e.name) || re.is_match(&self.descriptor(e)) || re.is_match(&e.body)
                })
                .take(limit)
                .collect());
        }
        let docs: Vec<Vec<String>> = items.iter().map(|i| words(&self.descriptor(&i.entry))).collect();
        let scores = bm25(&words(query), &docs);
        let mut ranked: Vec<(f64, Item)> =
            scores.into_iter().zip(items).filter(|(s, _)| *s > 0.0).collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(ranked.into_iter().take(limit).map(|(_, i)| i).collect())
    }

    /// Описание для поиска по словам: у навыка — name, title, description;
    /// у коннектора ещё aliases (описание muse.connector_search).
    fn descriptor(&self, e: &Entry) -> String {
        let mut s = format!("{} {} {}", e.name.replace(['_', '-'], " "), e.title, e.description);
        if self.kind == Kind::Connector {
            for a in &e.aliases {
                s.push(' ');
                s.push_str(a);
            }
        }
        s
    }
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

/// BM25 с обычными k1 = 1.2, b = 0.75 (чисел muse в разборе нет).
fn bm25(query: &[String], docs: &[Vec<String>]) -> Vec<f64> {
    let (k1, b) = (1.2, 0.75);
    let n = docs.len() as f64;
    let avg = docs.iter().map(|d| d.len()).sum::<usize>() as f64 / n.max(1.0);
    let mut terms: Vec<&String> = query.iter().collect();
    terms.sort();
    terms.dedup();
    docs.iter()
        .map(|d| {
            terms
                .iter()
                .map(|t| {
                    let df = docs.iter().filter(|x| x.contains(t)).count() as f64;
                    let tf = d.iter().filter(|w| w == t).count() as f64;
                    if tf == 0.0 {
                        return 0.0;
                    }
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    idf * tf * (k1 + 1.0) / (tf + k1 * (1.0 - b + b * d.len() as f64 / avg.max(1.0)))
                })
                .sum()
        })
        .collect()
}

struct Search {
    catalog: Catalog,
}

fn prop(t: &str, d: &str) -> Value {
    json!({"type": t, "description": personal(d)})
}

impl Tool for Search {
    fn spec(&self) -> ToolSpec {
        match self.catalog.kind {
            Kind::Skill => ToolSpec {
                name: "hamlet.skill_search".into(),
                description: personal(SKILL_DESCRIPTION),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "queries": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_QUERIES, "description": personal(SKILL_QUERIES)},
                        "mode": {"type": "string", "enum": ["bm25", "regex"], "description": personal(SKILL_MODE)},
                        "maxResults": prop("integer", SKILL_MAX),
                    },
                    "required": ["queries"],
                }),
            },
            Kind::Connector => ToolSpec {
                name: "hamlet.connector_search".into(),
                description: personal(CONNECTOR_DESCRIPTION),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "queries": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_QUERIES, "description": personal(CONNECTOR_QUERIES)},
                        "maxResults": prop("integer", CONNECTOR_MAX),
                    },
                    "required": ["queries"],
                }),
            },
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let queries: Vec<String> = args
                .get("queries")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|q| q.as_str().map(str::to_string)).filter(|q| !q.trim().is_empty()).collect())
                .unwrap_or_default();
            if queries.is_empty() {
                // Строки ядра: «queries cannot be empty», «Supply nonempty queries…».
                return Err(ToolError::InvalidArguments("queries cannot be empty".into()));
            }
            if queries.len() > MAX_QUERIES {
                return Err(ToolError::InvalidArguments(format!("at most {MAX_QUERIES} queries")));
            }
            let regex = match args.get("mode").and_then(Value::as_str).unwrap_or("bm25") {
                "bm25" => false,
                "regex" if self.catalog.kind == Kind::Skill => true,
                other => {
                    // Строка ядра 0x44c0a0.
                    return Err(ToolError::InvalidArguments(format!("unknown mode '{other}'; expected 'bm25' or 'regex'")));
                }
            };
            let limit = args.get("maxResults").and_then(Value::as_u64).unwrap_or(DEFAULT_HITS as u64);
            if limit == 0 {
                return Err(ToolError::InvalidArguments("maxResults must be greater than zero".into()));
            }
            let limit = (limit as usize).min(MAX_HITS);
            let mut results = Vec::new();
            let mut any = false;
            for q in &queries {
                let hits = self.catalog.search(q, regex, limit).map_err(ToolError::InvalidArguments)?;
                any |= !hits.is_empty();
                let hits: Vec<Value> = hits
                    .iter()
                    .map(|i| {
                        let mut h = json!({
                            "name": i.entry.name,
                            "title": i.entry.title,
                            "path": i.entry.doc.display().to_string(),
                            "description": i.entry.description,
                        });
                        if let Some(base) = &i.base {
                            // Поправка поверх системного: читаются оба.
                            h["path"] = base.display().to_string().into();
                            h["amendments"] = json!([i.entry.doc.display().to_string()]);
                        }
                        h
                    })
                    .collect();
                results.push(json!({"query": q, "hits": hits}));
            }
            let note = match (self.catalog.kind, any) {
                (Kind::Skill, true) => SKILL_HITS_NOTE.to_string(),
                (Kind::Skill, false) => SKILL_MISS_NOTE.to_string(),
                (Kind::Connector, _) => personal(CONNECTOR_NOTE),
            };
            Ok(vec![ToolResultPart::Text { text: json!({"results": results, "note": note}).to_string() }])
        })
    }
}

/// Добавить `muse.skill_search` и `muse.connector_search` в реестр.
pub fn add_search(tools: &mut Tools, skills: &Catalog, connectors: &Catalog) {
    tools.add(Search { catalog: skills.clone() }).add(Search { catalog: connectors.clone() });
}
