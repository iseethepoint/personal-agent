//! П2а: реестр моделей и выбор модели по задаче (без сети).

use std::path::PathBuf;

use agent_model::{
    ImageResolution, OutputEffort, Part, ProviderError, ProviderErrorKind, ProviderKind, Purpose,
    Registry, RegistryError, Response, StopReason, Usage,
};

fn repo_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn base() -> String {
    fixture("registry_base.toml")
}

/// Подменяет одну строку образца, чтобы получить сломанный реестр.
fn broken(from: &str, to: &str) -> Result<Registry, RegistryError> {
    let b = base();
    assert!(b.contains(from), "в образце нет `{from}`");
    Registry::parse(&b.replacen(from, to, 1))
}

#[test]
fn repo_registry_loads_with_both_providers() {
    let reg = Registry::load(&repo_file("config/models.toml")).expect("config/models.toml");
    let main = reg.resolve(Purpose::Main, None).unwrap().unwrap();
    assert_eq!(main.option.provider, ProviderKind::Anthropic);
    assert_eq!(main.option.model, "claude-opus-5-5");
    let a = main.weights.anthropic.as_ref().expect("умения anthropic");
    assert!(a.adaptive_thinking);
    assert_eq!(a.output_effort, OutputEffort::Supported);
    assert_eq!(a.image_resolution, ImageResolution::High);
    assert!(main.weights.compaction_trigger <= main.weights.context_window);

    let local = reg.model("local/qwen3-8b").expect("локальная модель");
    assert_eq!(local.provider, ProviderKind::OpenaiCompat);
    assert_eq!(local.base_url.as_deref(), Some("http://127.0.0.1:11434/v1"));
    let r = reg.resolve(Purpose::Main, Some("local/qwen3-8b")).unwrap().unwrap();
    assert!(r.wire.local_inference);
    assert!(r.weights.openai_compat.is_some());
}

#[test]
fn base_fixture_loads() {
    Registry::parse(&base()).expect("образец реестра");
}

#[test]
fn resolve_by_purpose() {
    let reg = Registry::parse(&base()).unwrap();
    let id = |p, sel| {
        reg.resolve(p, sel)
            .unwrap()
            .map(|r| r.option.id.clone())
            .unwrap_or_default()
    };
    // Без выбора — [defaults].model.
    assert_eq!(id(Purpose::Main, None), "anthropic/main");
    assert_eq!(id(Purpose::CronWorker, None), "anthropic/main");
    // Сжатие и сброс памяти — на модели обслуживания.
    assert_eq!(id(Purpose::Compaction, None), "anthropic/small");
    assert_eq!(id(Purpose::MemoryFlush, None), "anthropic/small");
    // Эмбеддинги — своя модель из [defaults].
    assert_eq!(id(Purpose::Embeddings, None), "local/embed");
    // Выбрана локальная: главный разговор на ней.
    assert_eq!(id(Purpose::Main, Some("local/chat")), "local/chat");
    assert_eq!(id(Purpose::BackgroundWorker, Some("local/chat")), "local/chat");
    // setup.overrides: браузер — на другой модели.
    assert_eq!(id(Purpose::BrowserAgent, Some("local/chat")), "anthropic/main");
    // setup.default_model_purposes: расписания — на модели по умолчанию.
    assert_eq!(id(Purpose::CronWorker, Some("local/chat")), "anthropic/main");
    // setup.compaction_memory_flush_use_main: сжатие — на выбранной.
    assert_eq!(id(Purpose::Compaction, Some("local/chat")), "local/chat");
    // Решения по целям (П4ж) — назначения muse; без переопределения — на
    // выбранной модели.
    assert_eq!(id(Purpose::GoalActivityState, None), "anthropic/main");
    assert_eq!(id(Purpose::GoalSubgoalRollup, Some("local/chat")), "local/chat");
    // П4ж-2: итог расписаний и монитор подагентов — свои назначения.
    assert_eq!(id(Purpose::CronResolve, Some("local/chat")), "local/chat");
    assert_eq!(id(Purpose::SubagentMonitor, None), "anthropic/main");
}

#[test]
fn goal_purposes_follow_overrides() {
    let reg = broken(
        "  { purpose = \"BrowserAgent\", model = \"anthropic/main\" },",
        "  { purpose = \"BrowserAgent\", model = \"anthropic/main\" },\n  { purpose = \"GoalActivityState\", model = \"anthropic/small\" },\n  { purpose = \"GoalSubgoalRollup\", model = \"anthropic/main\" },",
    )
    .unwrap();
    let id = |p| reg.resolve(p, Some("local/chat")).unwrap().unwrap().option.id.clone();
    assert_eq!(id(Purpose::GoalActivityState), "anthropic/small");
    assert_eq!(id(Purpose::GoalSubgoalRollup), "anthropic/main");
    assert_eq!(id(Purpose::Main), "local/chat");
}

#[test]
fn embeddings_off_without_model() {
    let reg = broken("embeddings = \"local/embed\"\n", "").unwrap();
    assert!(reg.resolve(Purpose::Embeddings, None).unwrap().is_none());
}

#[test]
fn unknown_selection_is_error() {
    let reg = Registry::parse(&base()).unwrap();
    assert!(matches!(
        reg.resolve(Purpose::Main, Some("nope/model")),
        Err(RegistryError::UnknownModel { .. })
    ));
}

#[test]
fn error_unknown_default_model() {
    assert!(matches!(
        broken("model = \"anthropic/main\"\n", "model = \"anthropic/ghost\"\n"),
        Err(RegistryError::UnknownModel { id, .. }) if id == "anthropic/ghost"
    ));
}

#[test]
fn error_unknown_override_model() {
    assert!(matches!(
        broken(
            "{ purpose = \"BrowserAgent\", model = \"anthropic/main\" }",
            "{ purpose = \"BrowserAgent\", model = \"anthropic/ghost\" }"
        ),
        Err(RegistryError::UnknownModel { id, .. }) if id == "anthropic/ghost"
    ));
}

#[test]
fn error_purpose_without_model() {
    // Нет модели обслуживания — у сжатия нет модели.
    assert!(matches!(
        broken("maintenance = \"anthropic/small\"\n", "maintenance = \"\"\n"),
        Err(RegistryError::PurposeWithoutModel { purpose: Purpose::Compaction, .. })
    ));
    // Задача в переопределениях записана дважды.
    assert!(matches!(
        broken(
            "{ purpose = \"BrowserAgent\", model = \"anthropic/main\" },",
            "{ purpose = \"BrowserAgent\", model = \"anthropic/main\" },\n  { purpose = \"BrowserAgent\", model = \"anthropic/small\" },"
        ),
        Err(RegistryError::PurposeWithoutModel { purpose: Purpose::BrowserAgent, .. })
    ));
    // Незнакомая задача — ошибка разбора.
    assert!(matches!(
        broken("purpose = \"BrowserAgent\"", "purpose = \"Browser\""),
        Err(RegistryError::Parse(_))
    ));
}

#[test]
fn error_trigger_over_window() {
    assert!(matches!(
        broken("compaction_trigger = 150000\n", "compaction_trigger = 250000\n"),
        Err(RegistryError::TriggerOverWindow { trigger: 250000, window: 200000, .. })
    ));
}

#[test]
fn error_unknown_wire_and_weights() {
    assert!(matches!(
        broken("model = \"qwen3:8b\"", "model = \"qwen9:1b\""),
        Err(RegistryError::UnknownWire { .. })
    ));
    assert!(matches!(
        broken("\"qwen3:8b\" = { weights = \"qwen\"", "\"qwen3:8b\" = { weights = \"ghost\""),
        Err(RegistryError::UnknownWeights { .. })
    ));
}

#[test]
fn error_openai_compat_without_base_url() {
    assert!(matches!(
        broken("base_url = \"http://127.0.0.1:11434/v1\"\n", ""),
        Err(RegistryError::MissingBaseUrl { model }) if model == "local/chat"
    ));
}

#[test]
fn error_foreign_facts_and_typos() {
    // Умения anthropic у весов модели openai_compat молча не работали бы.
    assert!(matches!(
        broken("openai_compat.vision = true", "anthropic.adaptive_thinking = true"),
        Err(RegistryError::ForeignFacts { .. })
    ));
    // Опечатка в имени умения — ошибка, а не тихое «выключено».
    assert!(matches!(
        broken("anthropic.adaptive_thinking = true", "anthropic.adaptiv_thinking = true"),
        Err(RegistryError::Parse(_))
    ));
}

#[test]
fn provider_error_retryable_by_status() {
    assert!(ProviderError::http(429, "rate", Some(1000)).retryable);
    assert!(ProviderError::http(500, "server", None).retryable);
    assert!(ProviderError::http(529, "overloaded", None).retryable);
    assert!(!ProviderError::http(400, "bad", None).retryable);
    assert!(ProviderError::new(ProviderErrorKind::Stream, "обрыв").retryable);
    assert!(!ProviderError::new(ProviderErrorKind::Config, "нет ключа").retryable);
}

#[test]
fn response_helpers_keep_order() {
    let r = Response {
        content: vec![
            Part::Thinking {
                text: "думаю".into(),
                signature: Some("sig".into()),
            },
            Part::Text { text: "а".into() },
            Part::ToolCall {
                id: "c1".into(),
                name: "hamlet.read".into(),
                arguments: serde_json::json!({"path": "~/x"}),
            },
            Part::Text { text: "б".into() },
        ],
        stop_reason: StopReason::ToolUse,
        usage: Usage::default(),
    };
    assert_eq!(r.text(), "аб");
    assert_eq!(r.tool_calls().count(), 1);
    assert!(!r.usage.reported);
}
