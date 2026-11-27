//! `agent-hamlet` — сборка агента из крейтов: реестр моделей, все
//! инструменты главного агента, память, службы расписаний и наблюдателей,
//! браузер, подагенты и их монитор, сжатие, ящик. Форма — демон muse
//! (`hatch-daemon-core`, `hatch-daemon-services`,
//! `runtime/provider_config.rs`): один движок поднимает всё, ходы главной
//! сессии идут по очереди («root turn arbiter»,
//! `daemon_loop/root_turn_arbiter`), новый ход начинает строка владельца
//! или весть в ящике.
//!
//! Своё: подагенты — на модели `Main` (отдельного назначения у muse нет,
//! подагент идёт на выбранной модели); инструменты подагента и работника
//! расписаний — общий набор без `subagent.*`, `forget.*` и окон браузера
//! (подагент «can't use this browser yourself», `environment_browser.md`)
//! и с `workflow.*` (у muse они в профилях `generic_subagent_depth1`,
//! `scheduler_cron_execute`); шаг сохранённой работы — общий набор без
//! `workflow.*`, команды начинаются в папке запуска (`workflow_subagent`);
//! заметка изменчивого — время и пояс по тексту muse
//! `session/browser_task_time_context.md` (свой системный текст и заметка
//! главного агента — П5).

pub mod config;
pub mod daemon;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_browser::browsers::Browsers;
use agent_cron::{Clock, Core, Scheduler};
use agent_goals::{Goals, Models as GoalModels, OwnedCleanup, Plan as GoalPlan};
use agent_loop::monitor::Monitor;
use agent_loop::{Compactor, Conversation, Inbox, Plan as CompactionPlan, Prefix, Subagents, Tools, Transcript};
use agent_memory::{Embedder, ForgetTools, MemoryTools, ProviderEmbedder};
use agent_model::openai_compat::OpenaiCompatProvider;
use agent_model::{AnyProvider, Purpose, Registry, Resolved};
use agent_tools::{add_search, secret_envs, Blocklist, Catalog, ExecTools, FileTools, Home, Todo, WebConfig, WebTools};

pub use config::AgentDir;

/// Собранный агент.
pub struct Hamlet {
    pub dir: AgentDir,
    pub home: PathBuf,
    pub spill_dir: PathBuf,
    pub main: Arc<AnyProvider>,
    pub tools: Arc<Tools>,
    pub prefix: Prefix,
    pub inbox: Inbox,
    pub transcript: Transcript,
    pub scheduler: Arc<Scheduler<AnyProvider>>,
    pub browsers: Arc<Browsers<AnyProvider>>,
    /// Глубокое исследование (`browser_deep_research`).
    pub research: Arc<agent_research::DeepResearch<AnyProvider>>,
    /// Сохранённые работы (`workflow.*`).
    pub workflows: Arc<agent_workflows::Workflows<AnyProvider>>,
    pub compaction: (Arc<AnyProvider>, Arc<AnyProvider>, u64),
    pub timezone: Option<String>,
    pub clock: Arc<dyn Clock>,
}

fn provider(r: &Resolved<'_>) -> Result<AnyProvider, String> {
    AnyProvider::from_resolved(r).map_err(|e| format!("модель `{}`: {e}", r.option.id))
}

fn resolve<'r>(reg: &'r Registry, p: Purpose, sel: Option<&str>) -> Result<Resolved<'r>, String> {
    reg.resolve(p, sel).map_err(|e| e.to_string())?.ok_or_else(|| format!("нет модели для {p:?}"))
}

/// Части, из которых собираются наборы инструментов.
struct Parts {
    home: Home,
    secrets: Vec<String>,
    inbox: Inbox,
    skills_dir: PathBuf,
    connectors_dir: PathBuf,
    web: WebConfig,
    web_root: PathBuf,
    memory: agent_memory::Store,
    embedder: Option<Arc<dyn Embedder>>,
    core: Arc<Core>,
    goals: Arc<Goals<AnyProvider>>,
}

impl Parts {
    /// Общий набор: главный агент, подагент, работник расписаний и шаг
    /// сохранённой работы; `start` — где начинаются команды без `workdir`.
    fn base(&self, start: Option<&Path>) -> Result<Tools, String> {
        let mut t = Tools::new();
        FileTools::new(self.home.clone()).add_to(&mut t);
        ExecTools::starting_in(self.home.clone(), self.secrets.clone(), Some(self.inbox.clone()), start.map(Path::to_path_buf)).add_to(&mut t);
        t.add(Todo::new());
        let skills = Catalog::skills(self.skills_dir.clone(), &self.home);
        let connectors = Catalog::connectors(self.connectors_dir.clone(), &self.home);
        add_search(&mut t, &skills, &connectors);
        WebTools::new(self.web.search.clone(), Blocklist::load(&self.web.blocklist, &self.web_root)?).add_to(&mut t);
        MemoryTools::new(self.home.dir.clone(), self.memory.clone(), self.embedder.clone()).add_to(&mut t);
        for x in agent_cron::tools(&self.core) {
            t.add(x);
        }
        for x in agent_cron::hook_tools(&self.core) {
            t.add(x);
        }
        for x in agent_goals::tools(&self.goals) {
            t.add(x);
        }
        Ok(t)
    }
}

/// Как собрать: часы и шаг такта служб (у muse такт будят наблюдатель
/// файлов и срок ближайшего прогона; у нас — ровный такт `agent_cron::TICK`).
pub struct Options {
    pub clock: Arc<dyn Clock>,
    pub tick: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options { clock: Arc::new(agent_cron::SystemClock), tick: agent_cron::TICK }
    }
}

impl Hamlet {
    pub async fn build(dir: AgentDir, opts: &Options) -> Result<Hamlet, String> {
        let t = &dir.toml;
        let sel = t.agent.model.as_deref();
        let reg = Registry::load(&dir.path(&t.paths.models)).map_err(|e| e.to_string())?;
        let web_file = dir.path(&t.paths.web);
        let web = WebConfig::parse(&std::fs::read_to_string(&web_file).map_err(|e| format!("{}: {e}", web_file.display()))?)?;
        // Пути чёрного списка — от папки, где лежит `config/`.
        let web_root = web_file.parent().and_then(|p| p.parent()).map(PathBuf::from).unwrap_or_else(|| dir.dir.clone());
        let secrets = secret_envs(&reg, &web);
        let home_dir = dir.path(&t.paths.home);
        std::fs::create_dir_all(&home_dir).map_err(|e| format!("{}: {e}", home_dir.display()))?;
        let spill_dir = home_dir.join(".spill");
        let home = Home::new(&home_dir);
        let url = dir.database_url()?;

        let main = Arc::new(provider(&resolve(&reg, Purpose::Main, sel)?)?);
        let memory = agent_memory::Store::connect(&url).await?;
        let embedder: Option<Arc<dyn Embedder>> = match reg.resolve(Purpose::Embeddings, sel).map_err(|e| e.to_string())? {
            Some(r) => Some(Arc::new(ProviderEmbedder { name: r.option.model.clone(), provider: OpenaiCompatProvider::from_resolved(&r).map_err(|e| e.to_string())? })),
            None => None,
        };
        let clock = opts.clock.clone();
        let now_clock = clock.clone();
        let goals_store = agent_goals::Store::connect(&url, Arc::new(move || now_clock.now_ms())).await?;
        let core = Core::build(agent_cron::Store::connect(&url).await?, clock.clone(), &home_dir, t.agent.timezone.clone(), secrets.clone(), Some(goals_store.clone()));
        let goal_plan = GoalPlan::from_registry(&reg, sel).map_err(|e| e.to_string())?;
        let goal_models = GoalModels::from_plan(&goal_plan, provider)?;
        let goals = Goals::new(goals_store, home_dir.clone(), Some(goal_models), Some(core.clone() as Arc<dyn OwnedCleanup>));
        let inbox = Inbox::new();
        let transcript = Transcript::new();
        let parts = Parts {
            home: home.clone(),
            secrets,
            inbox: inbox.clone(),
            skills_dir: dir.path(&t.paths.skills),
            connectors_dir: dir.path(&t.paths.connectors),
            web,
            web_root,
            memory: memory.clone(),
            embedder: embedder.clone(),
            core: core.clone(),
            goals,
        };
        let system = std::fs::read_to_string(dir.path(&t.paths.system)).unwrap_or_default();
        let parts = Arc::new(parts);

        // Сохранённые работы: шаги — на модели сессии, рабочая папка —
        // `workspace` в доме (`.jarvis/workflows`, `.jarvis/workflow-runs`).
        // Набор шага собирается на каждый запуск. Своё: если чтение списка
        // блокировок сети при этом упадёт (при сборке оно уже удалось), шаг
        // получит пустой набор, а ошибка уйдёт в журнал.
        let step_parts = parts.clone();
        let workflows = agent_workflows::Workflows::new(
            agent_workflows::store::Store::connect(&url).await?,
            main.clone(),
            Arc::new(move |dir: &Path| {
                step_parts.base(Some(dir)).unwrap_or_else(|e| {
                    eprintln!("workflow step tools: {e}");
                    Tools::new()
                })
            }),
            system.clone(),
            home_dir.clone(),
            home_dir.join("workspace"),
            spill_dir.clone(),
            inbox.clone(),
        );
        let with_workflows = |mut t: Tools| {
            for x in workflows.tools() {
                t.add(x);
            }
            t
        };

        // Подагенты: общий набор, своё начало подсказки; монитор — модель
        // `SubagentMonitor`.
        let sub_tools = Arc::new(with_workflows(parts.base(None)?));
        let sub_prefix = Prefix::assemble(&system, &home_dir, &sub_tools);
        let subagents = Subagents::new(main.clone(), sub_tools, sub_prefix, spill_dir.clone(), inbox.clone(), transcript.clone());
        subagents.set_monitor(Monitor::new(Arc::new(provider(&resolve(&reg, Purpose::SubagentMonitor, sel)?)?), &home_dir));

        // Браузер: модель `BrowserAgent`, профили в домашней папке.
        let chromium = t.browser.chromium.clone().map(|p| dir.path(&p)).or_else(agent_browser::chromium::find).unwrap_or_default();
        let browsers = Browsers::new(
            chromium,
            home_dir.join("browser"),
            t.browser.headless,
            Arc::new(provider(&agent_browser::planner::model(&reg, sel).map_err(|e| e.to_string())?)?),
            t.agent.locale.clone(),
            spill_dir.clone(),
            inbox.clone(),
        );

        // Главный агент: общий набор + подагенты, забывание, окна браузера.
        let mut tools = with_workflows(parts.base(None)?);
        for x in subagents.tools() {
            tools.add(x);
        }
        ForgetTools::new(subagents.clone(), &home_dir, memory.clone(), embedder.clone(), home_dir.join("skills/forget/SKILL.md")).add_to(&mut tools);
        let (browse, windows) = browsers.tools();
        tools.add(browse);
        tools.add(windows);
        // Исследователь — особый подагент главного агента: модель сессии,
        // веб-индекс, своё окно, `research_notes`, `finish_research`.
        let research_clock = clock.clone();
        let research = Arc::new(agent_research::DeepResearch {
            subagents: subagents.clone(),
            web: WebTools::new(parts.web.search.clone(), Blocklist::load(&parts.web.blocklist, &parts.web_root)?),
            home: home_dir.clone(),
            chromium: Some(t.browser.chromium.clone().map(|p| dir.path(&p)).or_else(agent_browser::chromium::find).unwrap_or_default()),
            headless: t.browser.headless,
            now: Arc::new(move || research_clock.now_ms()),
            deadline: agent_research::DEADLINE,
        });
        tools.add(research.tool());
        let tools = Arc::new(tools);
        let prefix = Prefix::assemble(&system, &home_dir, &tools);

        // Сжатие: модели `MemoryFlush` / `Compaction`, порог главной.
        let cp = CompactionPlan::from_registry(&reg, sel).map_err(|e| e.to_string())?;
        let compaction = (Arc::new(provider(&cp.flush)?), Arc::new(provider(&cp.compaction)?), cp.trigger);

        // Службы: работник расписаний (`CronWorker` / `CronResolve`) и
        // сверка памяти на такте.
        let cron_plan = agent_cron::Plan::from_registry(&reg, sel).map_err(|e| e.to_string())?;
        let mut scheduler = Scheduler::new(core, Arc::new(provider(&cron_plan.worker)?), system.clone(), Arc::new(with_workflows(parts.base(None)?)), spill_dir.clone(), inbox.clone());
        scheduler.resolver(Arc::new(provider(&cron_plan.resolve)?));
        let (mhome, mstore, memb) = (home_dir.clone(), memory.clone(), embedder.clone());
        scheduler.background(Arc::new(move || {
            let (h, s, e) = (mhome.clone(), mstore.clone(), memb.clone());
            Box::pin(async move { agent_memory::reindex(&h, &s, e.as_deref()).await.map(|_| ()) })
        }));

        Ok(Hamlet {
            dir: dir.clone(),
            home: home_dir,
            spill_dir,
            main,
            tools,
            prefix,
            inbox,
            transcript,
            scheduler: Arc::new(scheduler),
            browsers,
            research,
            workflows,
            compaction,
            timezone: t.agent.timezone.clone(),
            clock,
        })
    }

    /// Имена инструментов главного агента.
    pub fn tool_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tools.specs().into_iter().map(|s| s.name).collect();
        v.sort();
        v
    }

    pub fn conversation(&self) -> Conversation {
        Conversation::new(self.prefix.clone())
    }

    pub fn compactor(&self) -> Compactor<'_, AnyProvider, AnyProvider> {
        Compactor { flush: &self.compaction.0, compaction: &self.compaction.1, tools: &self.tools, trigger: self.compaction.2, spill_dir: self.spill_dir.clone() }
    }
}
