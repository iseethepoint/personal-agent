//! Глубокое исследование — `tools/research_delegate*` muse (`one_dive`,
//! `mailbox`, `completion_record`, `restart_resume`, `scaffold`),
//! `research_notes.rs`, `agent_manager/control/research_reattach.rs`
//! (`source-files.txt:828, 1715-1721, 5085, 6290`).
//!
//! Главный агент зовёт `browser_deep_research` (описание и поля — ядро,
//! `strings-all.txt:75909-75911`); исследователь — особый подагент
//! (`agent_loop::Kind`): профиль `deep_research` muse (`:74383-74415`) —
//! модель сессии (у нас `Main`, как у подагентов), инструменты
//! `browser_search`/`browser_open` (у нас `browser.search`/`browser.open`),
//! окно исследования `browser.observe/act/tabs`, `research_notes` (единственная
//! запись, только в своей папке) и терминальный `finish_research`; памяти,
//! чата, файлов, команд и подагентов у него нет («data-starved by
//! construction»). Один запуск — один «dive»: подагентов у исследователя
//! нет, глубина 1. Дополнения — `subagent.send` (почтовый ящик подагента),
//! остановка — `subagent.close` с итоговой заметкой; ход — `subagent.list`.
//!
//! Папка — `~/workspace/research_notes/<тема>-<%Y%m%d-%H%M>` (`:10498`,
//! `:55078`, `:68140`) с `notes/` и `AGENTS.md` по шаблону ядра
//! (`:36270-36273`). Срок — 60 минут, одно напоминание
//! (`blocks/deep_research/how_you_work.md`); итог — заметка «Deep research
//! finished…» с путём к папке и `report.md`, сбой — «Deep research did not
//! complete…» (`:225914`, `:42210`, `:35746`).
//!
//! Повторное подключение после перезапуска (`research_reattach`,
//! `restart_resume`): незавершённые запуски поднимаются заново с текстом
//! `research/resumed_after_restart.md` и остатком срока. Своё: состояние
//! запуска — файл `.run.json` в папке (у muse — своё хранилище), истории
//! исследователя мы не храним — продолжает он по своим заметкам, как велит
//! текст muse; одно окно исследования на весь запуск (у muse — новое на
//! каждый ход); перечень инструментов `{tooling_lines}` — имя и первая
//! фраза описания; квитанция `{acknowledgment}` — `agent_id` и папка.
//! Не взяты: `research_data` (рыночные данные — своей службы нет),
//! `browser.run`, `look`-цели, предел 512 KiB наблюдений окна.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use agent_browser::planner::{Act, LazyWindow, Observe, Tabs, WindowRef};
use agent_loop::{personal, Finish, Kind, Prefix, Status, Subagents, Tool, ToolError, ToolFuture, Tools};
use agent_model::{Message, Part, Provider, Role, ToolResultPart, ToolSpec};
use agent_tools::WebTools;
use serde_json::{json, Value};

const TEXTS_JSON: &str = include_str!("../../../prompts/deep_research/tools.json");
static TEXTS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(TEXTS_JSON).expect("prompts/deep_research/tools.json"));
const BLOCKS: [&str; 9] = [
    include_str!("../../../prompts/deep_research/who_you_are.md"),
    include_str!("../../../prompts/deep_research/assignment.md"),
    include_str!("../../../prompts/deep_research/how_you_work.md"),
    include_str!("../../../prompts/deep_research/tools.md"),
    include_str!("../../../prompts/deep_research/reporting_back.md"),
    include_str!("../../../prompts/deep_research/discretion.md"),
    include_str!("../../../prompts/deep_research/safety.md"),
    include_str!("../../../prompts/deep_research/security_authority.md"),
    include_str!("../../../prompts/deep_research/your_environment.md"),
];
const RESUMED_MD: &str = include_str!("../../../prompts/deep_research/resumed_after_restart.md");

pub const DELEGATE: &str = "browser_deep_research";
pub const FINISH: &str = "finish_research";
pub const NOTES: &str = "research_notes";
/// «the mission ends at 60 minutes».
pub const DEADLINE: Duration = Duration::from_secs(60 * 60);
const RUN_FILE: &str = ".run.json";

fn t(path: &str) -> String {
    let mut v = &*TEXTS;
    for k in path.split('.') {
        v = &v[k];
    }
    personal(v.as_str().unwrap_or_default())
}

fn text(s: String) -> Vec<ToolResultPart> {
    vec![ToolResultPart::Text { text: s }]
}

fn note(s: String) -> Message {
    Message { role: Role::Developer, content: vec![Part::Text { text: s }] }
}

/// Тема → имя папки: пробелы — дефисы, только буквы, цифры и дефисы.
pub fn slug(topic: &str) -> String {
    let s: String = topic.trim().chars().map(|c| if c.is_whitespace() { '-' } else { c.to_ascii_lowercase() }).filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "research".into() } else { s }
}

/// Глубокое исследование одного главного агента.
pub struct DeepResearch<P> {
    pub subagents: Subagents<P>,
    pub web: WebTools,
    pub home: PathBuf,
    /// Chromium для окна исследования; нет — окно недоступно, исследователь
    /// идёт по веб-индексу.
    pub chromium: Option<PathBuf>,
    pub headless: bool,
    /// Часы, мс (имя папки, остаток срока после перезапуска).
    pub now: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub deadline: Duration,
}

/// `research_notes`: запись только в папке исследования.
struct NotesTool {
    dir: PathBuf,
}

impl NotesTool {
    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        if path.trim().is_empty() {
            return Err(t("notes.path_empty"));
        }
        let p = Path::new(path);
        if p.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(t("notes.path_relative"));
        }
        let full = self.dir.join(p);
        if !full.starts_with(&self.dir) {
            return Err(t("notes.path_escapes"));
        }
        Ok(full)
    }
}

impl Tool for NotesTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NOTES.into(),
            description: t("notes.description"),
            input_schema: json!({"type": "object", "properties": {
                "action": {"type": "string", "enum": ["write", "read", "list"], "description": t("notes.action")},
                "path": {"type": "string", "description": t("notes.path")},
                "content": {"type": "string", "description": t("notes.content")},
            }, "required": ["action"], "additionalProperties": false}),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
            match args.get("action").and_then(Value::as_str).unwrap_or_default() {
                "write" => {
                    let full = self.resolve(path).map_err(ToolError::InvalidArguments)?;
                    let first = Path::new(path).components().next().map(|c| c.as_os_str().to_string_lossy().to_string()).unwrap_or_default();
                    if first == "AGENTS.md" || first == RUN_FILE {
                        return Err(ToolError::Failed(t("notes.agents_readonly")));
                    }
                    let content = args.get("content").and_then(Value::as_str).unwrap_or_default();
                    if let Some(parent) = full.parent() {
                        tokio::fs::create_dir_all(parent).await.map_err(|e| ToolError::Failed(e.to_string()))?;
                    }
                    tokio::fs::write(&full, content).await.map_err(|e| ToolError::Failed(e.to_string()))?;
                    Ok(text(format!("Wrote {} bytes to {path}.", content.len())))
                }
                "read" => {
                    let full = self.resolve(path).map_err(ToolError::InvalidArguments)?;
                    let body = tokio::fs::read_to_string(&full).await.map_err(|_| ToolError::Failed(format!("{path} does not exist in the research directory")))?;
                    Ok(text(body))
                }
                "list" => Ok(text(list_files(&self.dir).iter().map(|(p, n)| format!("{p} ({n} bytes)")).collect::<Vec<_>>().join("\n"))),
                other => Err(ToolError::InvalidArguments(format!("unknown action `{other}`"))),
            }
        })
    }
}

/// Файлы папки (без служебных `.…`), путь от папки и размер.
fn list_files(dir: &Path) -> Vec<(String, u64)> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<(String, u64)>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().to_string();
                out.push((rel, e.metadata().map(|m| m.len()).unwrap_or(0)));
            }
        }
    }
    let mut out = vec![];
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// `finish_research`: итог в слот.
struct FinishTool(Arc<Mutex<Option<String>>>);

impl Tool for FinishTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: FINISH.into(),
            description: t("finish.description"),
            input_schema: json!({"type": "object", "properties": {"summary": {"type": "string", "description": t("finish.summary")}}, "required": ["summary"], "additionalProperties": false}),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let s = args.get("summary").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| ToolError::InvalidArguments(t("finish.summary_required")))?;
            *self.0.lock().unwrap() = Some(s.to_string());
            Ok(text("The agent that called you receives it now.".into()))
        })
    }
}

impl<P: Provider + Send + Sync + 'static> DeepResearch<P> {
    fn root(&self) -> PathBuf {
        self.home.join("workspace/research_notes")
    }

    fn rel(&self, p: &Path) -> String {
        format!("~/{}", p.strip_prefix(&self.home).unwrap_or(p).display())
    }

    /// Системный текст исследователя с перечнем его инструментов.
    pub fn system(tools: &Tools) -> String {
        let lines: Vec<String> = tools
            .specs()
            .iter()
            .map(|s| {
                let first = s.description.split(". ").next().unwrap_or_default().trim_end_matches('.');
                format!("- `{}`: {first}.", s.name)
            })
            .collect();
        BLOCKS.iter().map(|b| personal(b).replace("{tooling_lines}", &lines.join("\n")).trim_end().to_string()).collect::<Vec<_>>().join("\n\n")
    }

    /// Начать исследование; `resume` — подъём после перезапуска (папка,
    /// остаток срока).
    fn launch(&self, mission: &str, dir: PathBuf, extra: Option<String>, left: Duration) -> String {
        let slot = Arc::new(Mutex::new(None));
        let window = LazyWindow::new(
            self.chromium.clone().unwrap_or_default(),
            std::env::temp_dir().join(format!("research-window-{}-{}", std::process::id(), (self.now)())),
            self.headless,
        );
        let mut tools = Tools::new();
        self.web.add_index_to(&mut tools);
        tools.add(Observe(WindowRef::Lazy(window.clone())));
        tools.add(Act(WindowRef::Lazy(window.clone())));
        tools.add(Tabs(WindowRef::Lazy(window.clone())));
        tools.add(NotesTool { dir: dir.clone() });
        tools.add(FinishTool(slot.clone()));
        let system = Self::system(&tools);
        let prefix = Prefix { system, tools: tools.specs() };
        let rel = self.rel(&dir);
        let mut first = format!("{}\n\n{}\n\n{mission}\n\n{}", t("run.first"), t("run.directory").replace("{dir}", &rel), t("run.first_end"));
        if let Some(x) = extra {
            first.push_str("\n\n");
            first.push_str(&x);
        }
        let done_slot = slot.clone();
        let fin_dir = dir.clone();
        let fin_rel = rel.clone();
        let finish: Finish = Arc::new(move |_status: Status, last: String| {
            let (slot, dir, rel, window) = (slot.clone(), fin_dir.clone(), fin_rel.clone(), window.clone());
            Box::pin(async move {
                window.close().await;
                let _ = mark_done(&dir).await;
                let report = dir.join("report.md");
                let has_report = report.exists();
                let summary = slot.lock().unwrap().clone();
                match summary {
                    Some(s) => {
                        let mut out = format!("{}\n\n{s}\n\n{}{rel}", t("run.finished"), t("run.research_directory"));
                        if has_report {
                            out.push_str(&format!("\n{}{rel}/report.md", t("run.full_report")));
                        } else {
                            out.push_str(&t("run.no_report"));
                        }
                        note(out)
                    }
                    None => {
                        let mut out = format!("{}\n\n{last}\n\n{}{rel}", t("run.failed"), t("run.research_directory"));
                        if !has_report {
                            out.push_str(&t("run.no_report"));
                        }
                        note(out)
                    }
                }
            })
        });
        self.subagents.spawn_kind(Kind {
            task: mission.to_string(),
            first,
            prefix,
            tools: Arc::new(tools),
            terminal: vec![FINISH.into()],
            done: Arc::new(move || done_slot.lock().unwrap().is_some()),
            nudge: Some(t("run.nudge")),
            deadline: Some(left),
            finish,
        })
    }

    /// `browser_deep_research`: папка, запуск, квитанция.
    pub async fn delegate(&self, mission: &str, topic: Option<&str>) -> Result<String, String> {
        if mission.trim().is_empty() {
            return Err(t("delegate.mission_required"));
        }
        let now = (self.now)();
        let stamp = chrono::DateTime::from_timestamp_millis(now).unwrap_or_default().format("%Y%m%d-%H%M").to_string();
        let base = format!("{}-{stamp}", slug(topic.unwrap_or("research")));
        let mut dir = self.root().join(&base);
        let mut n = 2;
        while dir.exists() {
            dir = self.root().join(format!("{base}-{n}"));
            n += 1;
        }
        tokio::fs::create_dir_all(dir.join("notes")).await.map_err(|e| format!("research delegate could not scaffold the research directory: {e}"))?;
        let created = chrono::DateTime::from_timestamp_millis(now).unwrap_or_default().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        tokio::fs::write(dir.join("AGENTS.md"), t("run.agents_md").replace("{created}", &created).replace("{mission}", mission)).await.map_err(|e| format!("research delegate could not write the research AGENTS.md: {e}"))?;
        tokio::fs::write(dir.join(RUN_FILE), json!({"mission": mission, "started_at_ms": now, "status": "running"}).to_string()).await.map_err(|e| e.to_string())?;
        let id = self.launch(mission, dir.clone(), None, self.deadline);
        let ack = format!("agent_id: {id}. Research folder: {}.", self.rel(&dir));
        Ok(format!("{}\n\n{}", t("delegate.receipt").replace("{acknowledgment}", &ack), t("delegate.receipt_tell")))
    }

    /// Поднять исследования, прерванные перезапуском. Возвращает id
    /// поднятых подагентов; просроченные кончаются заметкой о сбое.
    pub async fn reattach(&self) -> Vec<String> {
        let mut ids = vec![];
        let Ok(rd) = std::fs::read_dir(self.root()) else { return ids };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.join(RUN_FILE).exists()).collect();
        dirs.sort();
        for dir in dirs {
            let Ok(raw) = std::fs::read_to_string(dir.join(RUN_FILE)) else { continue };
            let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            if v["status"] != "running" {
                continue;
            }
            let mission = v["mission"].as_str().unwrap_or_default().to_string();
            let elapsed = Duration::from_millis(((self.now)() - v["started_at_ms"].as_i64().unwrap_or(0)).max(0) as u64);
            let rel = self.rel(&dir);
            if elapsed >= self.deadline {
                let _ = mark_done(&dir).await;
                let mut out = format!(
                    "{}\n\nresearcher ended before finish_research (deadline, nudge exhausted, or close)\n\n{}{rel}",
                    t("run.failed"),
                    t("run.research_directory")
                );
                if !dir.join("report.md").exists() {
                    out.push_str(&t("run.no_report"));
                }
                self.subagents.inbox().push(note(out));
                continue;
            }
            let left = self.deadline - elapsed;
            let files: Vec<String> = list_files(&dir).into_iter().map(|(p, _)| p).filter(|p| p != "AGENTS.md").collect();
            let clause = if files.is_empty() { t("run.dir_empty").replace("{dir}", &rel) } else { t("run.dir_files").replace("{dir}", &rel).replace("{files}", &files.join(", ")) };
            let resumed = personal(RESUMED_MD).replace("{directory_clause}", &clause).replace("{remaining_minutes}", &left.as_secs().div_ceil(60).to_string());
            ids.push(self.launch(&mission, dir, Some(resumed.trim_end().to_string()), left));
        }
        ids
    }

    pub fn tool(self: &Arc<Self>) -> DelegateTool<P> {
        DelegateTool(self.clone())
    }
}

async fn mark_done(dir: &Path) -> std::io::Result<()> {
    let p = dir.join(RUN_FILE);
    let raw = tokio::fs::read_to_string(&p).await?;
    let mut v: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
    v["status"] = json!("done");
    tokio::fs::write(&p, v.to_string()).await
}

/// Инструмент главного агента `browser_deep_research`.
pub struct DelegateTool<P>(Arc<DeepResearch<P>>);

impl<P: Provider + Send + Sync + 'static> Tool for DelegateTool<P> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: DELEGATE.into(),
            description: t("delegate.description"),
            input_schema: json!({"type": "object", "properties": {
                "mission": {"type": "string", "description": t("delegate.mission")},
                "topic": {"type": "string", "description": t("delegate.topic")},
            }, "required": ["mission"], "additionalProperties": false}),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let mission = args.get("mission").and_then(Value::as_str).unwrap_or_default();
            let out = self.0.delegate(mission, args.get("topic").and_then(Value::as_str)).await.map_err(ToolError::Failed)?;
            Ok(text(out))
        })
    }
}
