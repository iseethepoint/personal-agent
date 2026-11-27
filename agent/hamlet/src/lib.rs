//! `agent-hamlet` — сборка агента из крейтов: реестр моделей, все
//! инструменты главного агента, память, службы расписаний и наблюдателей,
//! браузер, подагенты и их монитор, сжатие, ящик. Форма — демон muse
//! (`hatch-daemon-core`, `hatch-daemon-services`,
//! `runtime/provider_config.rs`): один движок поднимает всё, ходы главной
//! сессии идут по очереди («root turn arbiter»,
//! `daemon_loop/root_turn_arbiter`), новый ход начинает строка владельца
//! или весть в ящике.
//!
//! Наборы инструментов — по ролям muse (`roles.rs`): главный агент —
//! `foreground_root`, подагент — `owner_subagent_depth1`, шаг сохранённой
//! работы — `workflow_subagent_depth1` (команды начинаются в папке запуска),
//! работник расписаний — `scheduler_cron_execute`, его итог —
//! `scheduler_cron_resolve`, работник наблюдателя — `execute_resolve_worker`,
//! проверяющий — `verifier`, сброс памяти — `memory_flush`.
//!
//! Своё: подагенты — на модели `Main` (у ролей muse `model = "session"`);
//! работнику расписаний не даны окна браузера, хотя они в его роли: окна
//! Hamlet отдают итог в ящик главного агента;
//! заметка изменчивого — время и пояс по тексту muse
//! `session/browser_task_time_context.md` (свой системный текст и заметка
//! главного агента — П5); `hamlet.db` — в общем наборе, как поиск навыков
//! (в разборе профилей, где он есть у muse, не видно).

pub mod config;
pub mod daemon;
pub mod main_prompt;
pub mod scheduler_prompt;
pub mod subagent_prompt;
pub mod temporal;
pub mod background_prompt;
pub use agent_loop::roles;

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use agent_browser::browsers::Browsers;
use agent_cron::{Clock, Core, Scheduler};
use agent_goals::{Goals, Models as GoalModels, OwnedCleanup, Plan as GoalPlan};
use agent_loop::monitor::Monitor;
use agent_loop::{Compactor, Conversation, Inbox, Plan as CompactionPlan, Prefix, Subagents, Tools, Transcript};
use agent_memory::{Embedder, ForgetTools, MemoryTools, ProviderEmbedder};
use agent_model::openai_compat::OpenaiCompatProvider;
use agent_model::http::CRON_FIRST_CHUNK_TIMEOUT;
use agent_model::{AnyProvider, Purpose, Registry, Resolved};
use agent_tools::{add_search, secret_envs, Blocklist, Catalog, ExecTools, FileTools, Home, Todo, Trash, WebConfig, WebTools};

pub use config::AgentDir;

/// Агент главной сессии в `agent.agents`.
pub const ROOT_AGENT: &str = "root";
/// `recovery_class` точки перезапуска главного агента (значение ядра).
pub const ROOT_RECOVERY_CLASS: &str = "foreground_root";

/// Проверка ответа главного агента (`agent_cron::verification`).
pub struct Verification {
    pub classifier: AnyProvider,
    pub verifier: agent_cron::verification::Verifier<AnyProvider>,
}

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
    /// Подагенты главного агента (и исследователь).
    pub subagents: Subagents<AnyProvider>,
    /// Список дел главного агента и продолжение по нему.
    pub todo: Todo,
    pub continuation: agent_tools::Continuation,
    /// Сжатие: модели сброса и сводки, порог, окно главной модели, по частям ли.
    pub compaction: (Arc<AnyProvider>, Arc<AnyProvider>, u64, u64, bool),
    /// Набор хода сброса памяти (роль `memory_flush`).
    pub flush_tools: Tools,
    /// История главного агента в базе (`agent.context_items`).
    pub history: Arc<agent_history::AgentHistory>,
    /// Проверка ответа (`[verification] enabled`): классификатор и
    /// проверяющий.
    pub verification: Option<Verification>,
    pub timezone: Option<String>,
    /// Дополнения владельца к системным текстам (`system.md`).
    pub owner_system: String,
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
    /// Корзина — на часах службы: её уборщик идёт на такте.
    trash: Trash,
    /// Запросы к своей базе (`hamlet.db`).
    db: agent_db::Db,
    /// История в базе: «Past conversations» и `chat.read_messages`.
    history: agent_history::Store,
}

impl Parts {
    /// Общий набор: главный агент, подагент, работник расписаний и шаг
    /// сохранённой работы; `start` — где начинаются команды без `workdir`.
    fn base(&self, start: Option<&Path>) -> Result<Tools, String> {
        let mut t = Tools::new();
        FileTools::new(self.home.clone()).add_to(&mut t);
        ExecTools::starting_in(self.home.clone(), self.secrets.clone(), Some(self.inbox.clone()), start.map(Path::to_path_buf)).add_to(&mut t);
        t.add(Todo::new());
        self.trash.add_to(&mut t);
        t.add(self.db.clone());
        let skills = Catalog::skills(self.skills_dir.clone(), &self.home);
        let connectors = Catalog::connectors(self.connectors_dir.clone(), &self.home);
        add_search(&mut t, &skills, &connectors);
        WebTools::new(self.web.search.clone(), Blocklist::load(&self.web.blocklist, &self.web_root)?).add_to(&mut t);
        MemoryTools::new(self.home.dir.clone(), self.memory.clone(), self.embedder.clone()).with_conversations(ROOT_AGENT).add_to(&mut t);
        t.add(agent_history::ChatReader::new(&self.history, ROOT_AGENT));
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

/// Как собрать: часы службы (в тестах — поддельные).
pub struct Options {
    pub clock: Arc<dyn Clock>,
}

impl Default for Options {
    fn default() -> Self {
        Options { clock: Arc::new(agent_cron::SystemClock) }
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

        let main_resolved = resolve(&reg, Purpose::Main, sel)?;
        let main_model = main_resolved.option.id.clone();
        let main = Arc::new(provider(&main_resolved)?);
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
        // История и ящик главного агента в базе: агент `root` (своё имя; у
        // muse — id сессии); непринятые до перезапуска вести — снова в ящике.
        let history_store = agent_history::Store::connect(&url).await?;
        let history_clock = clock.clone();
        let history = Arc::new(agent_history::AgentHistory::open(&history_store, ROOT_AGENT, "root", &main_model, Arc::new(move || history_clock.now_ms())).await?);
        let inbox = Inbox::persistent(Arc::new(agent_history::AgentMailbox::new(&history_store, ROOT_AGENT))).await?;
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
            trash: {
                let c = clock.clone();
                Trash::with_clock(home.clone(), Arc::new(move || chrono::DateTime::from_timestamp_millis(c.now_ms()).unwrap_or_default()))
            },
            db: agent_db::Db::connect(&url, &dir.path(&t.paths.skills)).await?,
            history: history_store.clone(),
        };
        let system = std::fs::read_to_string(dir.path(&t.paths.system)).unwrap_or_default();
        // Проверка ответа: классификатор — назначение
        // `ResponseVerificationClassifier`, проверяющий — модель сессии,
        // чтение файлов и веб (место `verifier` muse).
        let verification = if t.verification.enabled {
            let web = WebTools::new(parts.web.search.clone(), Blocklist::load(&parts.web.blocklist, &parts.web_root)?);
            let files = FileTools::new(parts.home.clone());
            let vclock = clock.clone();
            Some(Verification {
                classifier: provider(&resolve(&reg, Purpose::ResponseVerificationClassifier, sel)?)?,
                verifier: agent_cron::verification::Verifier {
                    store: core.store.clone(),
                    provider: main.clone(),
                    tools: Arc::new(move || {
                        let mut t = Tools::new();
                        files.add_read_to(&mut t);
                        web.add_to(&mut t);
                        roles::Role::get("verifier").apply(t)
                    }),
                    // Профиль `verification` на момент проверки; `exec` — по
                    // набору роли `verifier` (его там нет).
                    system: {
                        let (c, tz, owner) = (clock.clone(), t.agent.timezone.clone(), system.clone());
                        let exec = roles::Role::get("verifier").allows("hamlet.exec");
                        Arc::new(move || main_prompt::with_owner(background_prompt::verification(c.now_ms(), tz.as_deref(), exec), &owner))
                    },
                    spill_dir: spill_dir.clone(),
                    now: Arc::new(move || vclock.now_ms()),
                },
            })
        } else {
            None
        };
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
                let t = step_parts.base(Some(dir)).unwrap_or_else(|e| {
                    eprintln!("workflow step tools: {e}");
                    Tools::new()
                });
                roles::Role::get("workflow_subagent_depth1").apply(t)
            }),
            // Шаг — подагент роли `workflow_subagent_depth1`: профиль 0,
            // изолирован, своих подагентов нет (`subagent_prompt`).
            main_prompt::with_owner(subagent_prompt::assemble(&roles::Role::get("workflow_subagent_depth1").apply(parts.base(None)?), false, false), &system),
            home_dir.clone(),
            home_dir.join("workspace"),
            spill_dir.clone(),
            inbox.clone(),
        );
        // Подагенты шагов — в базе: вызов, оборванный перезапуском,
        // продолжается при повторе сценария (`recovery_owners` muse).
        let wf_clock = clock.clone();
        workflows.set_workers(Arc::new(agent_history::AgentWorkers::new(&history_store, &main_model, Arc::new(move || wf_clock.now_ms()))));
        let with_workflows = |mut t: Tools| {
            for x in workflows.tools() {
                t.add(x);
            }
            t
        };

        // Подагенты: общий набор, своё начало подсказки; монитор — модель
        // `SubagentMonitor`.
        // Глубина — по ролям: у кого в роли `subagent.spawn`, тот заводит
        // своих (`owner_subagent_depth1` — да, `owner_subagent_depth2` — нет).
        let depth1 = roles::Role::get("owner_subagent_depth1");
        let sub_tools = Arc::new(depth1.apply(with_workflows(parts.base(None)?)));
        let spawns = depth1.allows("subagent.spawn");
        // Системный текст подагента — профиль 0 ядра (`subagent_prompt`):
        // глубина 1 наследует разговор родителя, глубина 2 изолирована и
        // своих не заводит.
        let sub_text = main_prompt::with_owner(subagent_prompt::assemble(&sub_tools, true, spawns), &system);
        let sub_prefix = Prefix::with_system(sub_text, &home_dir, &sub_tools);
        let subagents = Subagents::new(
            main.clone(),
            sub_tools,
            if spawns { agent_loop::spawning_prefix(sub_prefix) } else { sub_prefix },
            spill_dir.clone(),
            inbox.clone(),
            transcript.clone(),
        );
        if spawns {
            let deep_tools = Arc::new(roles::Role::get("owner_subagent_depth2").apply(with_workflows(parts.base(None)?)));
            let deep_text = main_prompt::with_owner(subagent_prompt::assemble(&deep_tools, false, false), &system);
            let deep_prefix = Prefix::with_system(deep_text, &home_dir, &deep_tools);
            subagents.set_deeper(deep_tools, deep_prefix);
        }
        subagents.set_monitor(Monitor::new(Arc::new(provider(&resolve(&reg, Purpose::SubagentMonitor, sel)?)?), &home_dir));
        // Метка времени на входы подагентов — как у главного (`daemon::time_tag`);
        // без пояса меток нет.
        if let Some(tz) = t.agent.timezone.clone() {
            let tag_clock = clock.clone();
            subagents.set_time_tag(Arc::new(move || daemon::time_tag(tag_clock.now_ms(), Some(&tz)).unwrap_or_default()));
        }
        // Подагенты в базе: после перезапуска — снова в списке, оборванные
        // ходы продолжаются (`restart_rehydration` muse).
        let sub_clock = clock.clone();
        subagents.persistent(Arc::new(agent_history::AgentSubagents::new(&history_store, ROOT_AGENT, &main_model, Arc::new(move || sub_clock.now_ms())))).await?;

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
        // Окна — в базе: работа окна, оборванная перезапуском, продолжается
        // (`recovery_owners` muse).
        let br_clock = clock.clone();
        browsers.set_workers(Arc::new(agent_history::AgentWorkers::new(&history_store, &main_model, Arc::new(move || br_clock.now_ms()))));

        // Главный агент: общий набор + подагенты, забывание, окна браузера.
        let mut tools = with_workflows(parts.base(None)?);
        // Свой список дел главного агента — по нему продолжение хода; снимок
        // и сторож — в базе.
        let todo = Todo::persistent(Arc::new(agent_history::AgentTodo::new(&history_store, ROOT_AGENT))).await?;
        tools.add(todo.clone());
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
        // Отложенные пространства главного агента (`tool_loading =
        // "deferred"` у muse): функции уходят модели после
        // `tool_search.load_tool_namespace`, полное описание — при загрузке.
        // Своё: какие пространства отложены (полного списка в разборе нет) —
        // те, у кого есть полное описание пространства.
        tools
            .defer("cron", &agent_cron::cron_namespace())
            .defer("hooks", &agent_cron::hooks_namespace())
            .defer("user_goal", &agent_goals::namespace_description(agent_goals::Ns::UserGoal))
            .defer("tracking", &agent_goals::namespace_description(agent_goals::Ns::Tracking))
            .defer("workflow", &agent_workflows::tools::namespace_description());
        let tools = Arc::new(roles::Role::get("foreground_root").apply(tools));
        // Главный разговор: системный текст из блоков muse (`main_prompt`),
        // его копия — в рабочей папке, как пишет muse при старте
        // (`system_prompt_copy.rs`).
        let main_text = main_prompt::main_system(&tools, &system);
        let copy = home_dir.join(main_prompt::COPY_PATH);
        if let Some(parent) = copy.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&copy, &main_text).map_err(|e| format!("{}: {e}", copy.display()))?;
        let prefix = Prefix::with_system(main_text, &home_dir, &tools);
        // Сброс памяти — набор роли `memory_flush`.
        let flush_tools = roles::Role::get("memory_flush").apply((*tools).clone());

        // Сжатие: модели `MemoryFlush` / `Compaction`, порог главной.
        let cp = CompactionPlan::from_registry(&reg, sel).map_err(|e| e.to_string())?;
        let compaction = (Arc::new(provider(&cp.flush)?), Arc::new(provider(&cp.compaction)?), cp.trigger, cp.window, t.compaction.segmented);

        // Службы: работник расписаний (`CronWorker` / `CronResolve`, срок
        // первого куска потока — 30 мин, как у расписаний muse) и сверка
        // памяти на такте.
        let cron_plan = agent_cron::Plan::from_registry(&reg, sel).map_err(|e| e.to_string())?;
        // Наборы ролей `scheduler_cron_execute`, `scheduler_cron_resolve`
        // (инструменты итога сверх терминальных — из набора исполнения) и
        // `execute_resolve_worker`.
        let cron_tools = roles::Role::get("scheduler_cron_execute").apply(with_workflows(parts.base(None)?));
        let resolve_role = roles::Role::get("scheduler_cron_resolve");
        let resolve_tools: Vec<String> = resolve_role.tool_names().into_iter().filter(|n| cron_tools.get(n).is_some()).collect();
        // Тексты работника по профилям ядра (`scheduler_prompt`): исполнение —
        // профиль 22, итог — профиль 23 на момент прогона (строка года);
        // метка времени — перед сообщением работнику, без пояса — нет.
        let cron_text = main_prompt::with_owner(scheduler_prompt::worker(&cron_tools), &system);
        let (res_clock, res_tz, res_owner) = (clock.clone(), t.agent.timezone.clone(), system.clone());
        let resolve_text: Arc<dyn Fn() -> String + Send + Sync> =
            Arc::new(move || main_prompt::with_owner(scheduler_prompt::resolution(res_clock.now_ms(), res_tz.as_deref()), &res_owner));
        let mut scheduler = Scheduler::new(core, Arc::new(provider(&cron_plan.worker)?.with_first_chunk(CRON_FIRST_CHUNK_TIMEOUT)), system.clone(), Arc::new(cron_tools), spill_dir.clone(), inbox.clone());
        scheduler.cron_texts(cron_text, resolve_text);
        if let Some(tz) = t.agent.timezone.clone() {
            let tag_clock = clock.clone();
            scheduler.time_tag(Arc::new(move || daemon::time_tag(tag_clock.now_ms(), Some(&tz)).unwrap_or_default()));
        }
        scheduler.resolve_tools(resolve_tools);
        let hook_tools = Arc::new(roles::Role::get("execute_resolve_worker").apply(parts.base(None)?));
        // Работник наблюдателя — профиль `event_hook` на момент прогона.
        {
            let (c, tz, owner) = (clock.clone(), t.agent.timezone.clone(), system.clone());
            let exec = hook_tools.get("hamlet.exec").is_some();
            scheduler.hook_system(Arc::new(move || main_prompt::with_owner(background_prompt::event_hook(c.now_ms(), tz.as_deref(), exec), &owner)));
        }
        scheduler.hook_tools(hook_tools);
        scheduler.resolver(Arc::new(provider(&cron_plan.resolve)?.with_first_chunk(CRON_FIRST_CHUNK_TIMEOUT)));
        // Проверка отчётов расписаний: классификатор — модель главного
        // разговора (место `cron_verification_classifier` muse, `main_chat`),
        // проверяющий — тот же, что у ответа.
        if let Some(v) = &verification {
            scheduler.verification(agent_cron::verification::CronVerification { classifier: main.clone(), verifier: v.verifier.clone() });
        }
        // Работники расписаний — в базе: прогон, оборванный перезапуском,
        // продолжается с записанного (`recovery_owners` muse).
        let workers_clock = clock.clone();
        scheduler.workers(Arc::new(agent_history::AgentWorkers::new(&history_store, &cron_plan.worker.option.id, Arc::new(move || workers_clock.now_ms()))));
        // Сверка памяти — когда наблюдатель файлов увидел правку файла
        // памяти (и на первом такте). Своё: если векторы не дозаполнились,
        // сверка повторится на следующем такте.
        // Пути событий бывают каноническими (`/private/var/…` на macOS).
        let homes = [home_dir.clone(), home_dir.canonicalize().unwrap_or_else(|_| home_dir.clone())];
        let dirty = scheduler.watch(&home_dir, move |p| homes.iter().any(|h| p.strip_prefix(h).is_ok_and(agent_memory::is_memory_path)))?;
        let (mhome, mstore, memb) = (home_dir.clone(), memory.clone(), embedder.clone());
        scheduler.background(Arc::new(move || {
            let (h, s, e, dirty) = (mhome.clone(), mstore.clone(), memb.clone(), dirty.clone());
            Box::pin(async move {
                if dirty.swap(false, Ordering::SeqCst) {
                    let r = agent_memory::reindex(&h, &s, e.as_deref()).await?;
                    if r.embed_error.is_some() {
                        dirty.store(true, Ordering::SeqCst);
                    }
                }
                Ok(None)
            })
        }));
        // Уборщик корзины — фоновая работа службы (`recoverable_trash_reaper`);
        // будится к ближайшему сроку истечения.
        let reaper = parts.trash.clone();
        scheduler.background(Arc::new(move || {
            let t = reaper.clone();
            Box::pin(async move {
                let next = tokio::task::spawn_blocking(move || t.reap().and_then(|_| t.next_expiry())).await.map_err(|e| e.to_string())??;
                Ok(next.map(|d| d.timestamp_millis()))
            })
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
            subagents,
            todo,
            continuation: agent_tools::Continuation::new(),
            compaction,
            flush_tools,
            history,
            verification,
            timezone: t.agent.timezone.clone(),
            owner_system: system.clone(),
            clock,
        })
    }

    /// Имена инструментов главного агента.
    pub fn tool_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tools.specs().into_iter().map(|s| s.name).collect();
        v.sort();
        v
    }

    /// Разговор главного агента: после перезапуска — из базы (закреплённое
    /// начало и история байт в байт), впервые — новый, с закреплением начала.
    pub async fn conversation(&self) -> Result<Conversation, String> {
        if let Some(saved) = self.history.load().await? {
            return Ok(Conversation::restore(saved.prefix, saved.history, saved.last_volatile));
        }
        self.history.pin_prefix(&self.prefix).await?;
        Ok(Conversation::new(self.prefix.clone()))
    }

    /// Сжатие и сброс памяти; `flush_covered` — всё уже сброшено.
    pub fn compactor(&self, flush_covered: bool) -> Compactor<'_, AnyProvider, AnyProvider> {
        Compactor {
            flush: &self.compaction.0,
            compaction: &self.compaction.1,
            tools: &self.flush_tools,
            trigger: self.compaction.2,
            segmented: self.compaction.4,
            window: Some(self.compaction.3),
            spill_dir: self.spill_dir.clone(),
            today: self.today_log(),
            flush_covered,
            // Профиль `memory_flush` на момент сброса.
            flush_system: Some(main_prompt::with_owner(
                background_prompt::memory_flush(self.clock.now_ms(), self.timezone.as_deref(), self.flush_tools.get("hamlet.exec").is_some()),
                &self.owner_system,
            )),
        }
    }

    /// Сегодняшний журнал памяти `memory/YYYY-MM-DD.md` (дата — в поясе
    /// владельца, без пояса — UTC), если файл есть и не пуст.
    pub fn today_log(&self) -> Option<(String, String)> {
        let now = chrono::DateTime::from_timestamp_millis(self.clock.now_ms())?;
        let date = match self.timezone.as_deref().and_then(|t| t.parse::<chrono_tz::Tz>().ok()) {
            Some(tz) => now.with_timezone(&tz).format("%Y-%m-%d").to_string(),
            None => now.format("%Y-%m-%d").to_string(),
        };
        let rel = format!("memory/{date}.md");
        let content = std::fs::read_to_string(self.home.join(&rel)).ok()?;
        (!content.trim().is_empty()).then_some((rel, content))
    }
}
