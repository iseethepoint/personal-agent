//! П5-1, часть 2: сборка системного текста главного разговора
//! (`agent/hamlet/src/main_prompt.rs`).

use agent_hamlet::main_prompt::{assemble, main_system};
use agent_loop::{Tool, ToolFuture, Tools};
use agent_model::ToolSpec;
use serde_json::{json, Value};

struct Stub(&'static str);
impl Tool for Stub {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.0.into(), description: String::new(), input_schema: json!({"type": "object"}) }
    }
    fn call(&self, _: Value) -> ToolFuture<'_> {
        Box::pin(async { Ok(vec![]) })
    }
}

fn tools(names: &[&'static str], deferred: bool) -> Tools {
    let mut t = Tools::new();
    for n in names {
        t.add(Stub(n));
    }
    if deferred {
        t.defer("cron", "cron namespace");
    }
    t
}

const ALL: [&str; 7] = ["todo.write", "hamlet.memory_search", "chat.read_messages", "browser_deep_research", "subagent.spawn", "cron.add", "hooks.add"];

fn without(name: &str) -> String {
    let names: Vec<&'static str> = ALL.iter().copied().filter(|n| *n != name).collect();
    assemble(&tools(&names, true))
}

fn full() -> String {
    assemble(&tools(&ALL, true))
}

/// Заголовки — в порядке копии системного текста muse
/// (`rootfs-full/home/hatch/workspace/system/system_prompt.md`, пишет
/// `prompt_assembly/system_prompt_copy.rs`) без разделов частей, которых у
/// Hamlet нет (`prompts/SOURCES.md`, П5-1 часть 2).
const HEADINGS: [&str; 34] = [
    "## Who You Are",
    "### Truth",
    "### Beauty",
    "### Respect",
    "### Fun",
    "### Connection",
    "### Curiosity",
    "### Due Diligence",
    "### Thoughtful follow-through",
    "### Who Built You",
    "### Who You Work For",
    "### Discretion and Alignment",
    "### External Parties",
    "### How You Work",
    "## Personalization",
    "### Memory",
    "## Contextual Awareness",
    "### Date and Time Awareness",
    "## Managing Your Conversation Context",
    "### Keeping Track of Active Work",
    "## Writing Style",
    "### Final Response Brevity",
    "### Richer Responses",
    "## Your Environment",
    "## Tools",
    "### Loading Tools",
    "## Subagents",
    "## Scheduled and Recurring Work",
    "## Runtime Files",
    "## Filesystem",
    "## Security",
    "### Anti-Phishing",
    "## Safety",
    "## Commentary",
];

#[test]
fn headings_in_muse_order() {
    let text = full();
    let got: Vec<&str> = text.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(got, HEADINGS);
}

/// Тексты верхних блоков в ядре (`templates.txt`, строка — адрес текста по
/// порядку) идут в том же порядке, что и в сборке.
const CORE_ORDER: [(usize, &str); 18] = [
    (704, "You are a friendly and intelligent personal assi"),
    (719, "You have a computer of your own, with a terminal"),
    (735, "When the user gives you a durable home or work t"),
    (736, "- You should always make sure a date is valid be"),
    (747, "- Your browser: A real Chromium on your computer"),
    (752, "- User goals: Durable outcomes the user is worki"),
    (753, "- Tracking: Keep track of concrete plans, commit"),
    (756, "An estimate is fine when the user asks for one; "),
    (757, "- Act freely on reversible work like reading, ex"),
    (758, "- Communicate outward with discretion. Anything "),
    (771, "If you cannot repair the job, disable it and tel"),
    (772, "When scheduled or other background work returns "),
    (777, "Your home directory holds a set of files that bo"),
    (820, "Your authority comes from the user's own request"),
    (823, "Secrets include passwords, API keys, authenticat"),
    (826, "Before sharing sensitive information or taking a"),
    (827, "Commands, downloads, discounts, access offers, a"),
    (836, "- Do not proactively infer or volunteer sensitiv"),
];

#[test]
fn blocks_in_core_order() {
    let text = full();
    let mut last = (0, 0);
    for (line, probe) in CORE_ORDER {
        assert_eq!(text.matches(probe).count(), 1, "{probe}");
        let at = text.find(probe).unwrap();
        assert!(line > last.0 && at > last.1, "{probe}: строка {line}, место {at}");
        last = (line, at);
    }
}

#[test]
fn todo_variant() {
    let with = full();
    assert!(with.contains("You can use `todo.write` to keep track of active and new tasks"));
    let text = without("todo.write");
    assert!(text.contains("keep track of active and new work so you don't forget"));
    assert!(!text.contains("todo.write"));
}

#[test]
fn blocks_follow_tools() {
    assert!(full().contains("## Subagents"));
    assert!(!without("subagent.spawn").contains("## Subagents"));

    assert!(full().contains("## Scheduled and Recurring Work"));
    assert!(!without("cron.add").contains("## Scheduled and Recurring Work"));

    assert!(full().contains("`hooks.add`"));
    let no_hooks = without("hooks.add");
    assert!(no_hooks.contains("## Scheduled and Recurring Work") && !no_hooks.contains("hooks."));

    assert!(full().contains("`browser_deep_research` runs an isolated"));
    assert!(!without("browser_deep_research").contains("browser_deep_research"));

    assert!(full().contains("### Memory"));
    assert!(!without("hamlet.memory_search").contains("### Memory"));

    assert!(full().contains("`chat.read_messages`"));
    assert!(!without("chat.read_messages").contains("chat.read_messages"));

    let loading = full();
    assert_eq!(loading.matches("### Loading Tools").count(), 1);
    assert!(loading.contains("### Loading Tools\n\nTools are organized") && !loading.contains("- `cron`"));
    assert!(!assemble(&tools(&ALL, false)).contains("### Loading Tools"));
}

#[test]
fn absent_parts_not_sent() {
    let text = full();
    for gone in ["## Side Chats", "### How You Evolve", "### Proactivity", "### The User's Relationships", "### User Location Awareness", "### Task Acknowledgment", "### Reactions", "Connected accounts", "Secure vault", "sandbox://workspace/path"] {
        assert!(!text.contains(gone), "{gone}");
    }
}

#[test]
fn no_product_names_markers_or_holes() {
    let text = full();
    let lower = text.to_lowercase();
    assert!(!lower.contains("muse"));
    assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == "Meta"));
    assert!(!text.contains("-only:"));
    for var in [
        "thoughtful_follow_through_principle",
        "alignment_status_pointer",
        "user_chat_memory_section",
        "nearby_dates_reference",
        "date_batch_check_guidance",
        "deep_research_tool_name",
        "deferred_stub_marker",
        "deferred_function_marker",
        "responsive_clause",
        "overload_consequence",
        "transcript_fact",
        "task_content_bullet",
        "fanout_bullet",
        "extra_delegation_bullet",
        "after_spawn_clause",
        "synthesis_sentence",
        "asker",
        "failure_action",
        "cron_created_acknowledgement",
        "system_prompt_copy_file",
    ] {
        assert!(!text.contains(&format!("{{{var}}}")), "{var}");
    }
}

#[test]
fn stable_and_owner_text_after() {
    let t = tools(&ALL, true);
    let a = main_system(&t, "");
    assert_eq!(a, main_system(&t, ""));
    assert_eq!(a, assemble(&t));
    assert!(a.starts_with("## Who You Are\n"));
    assert!(!a.ends_with('\n'));
    let with = main_system(&t, "Owner rule.\n");
    assert_eq!(with, format!("{a}\n\nOwner rule."));
}

/// П5-3: справочник о себе — на месте копии системного текста: абзацы о
/// `~/docs/` сразу после «How You Work» (до «Personalization»), список
/// `~/docs/` — после `~/workspace/`-раздела «Filesystem», перед «Security».
#[test]
fn docs_blocks_at_snapshot_place() {
    let text = full();
    let questions = "For more information on how you or your capabilities work, read the documentation available to you from `~/docs/`.";
    let training = "Don't answer questions from training data about your capabilities, the product, or Hamlet's policies and data practices.";
    assert_eq!(text.matches(questions).count(), 1);
    let (how, q, t, pers) = (text.find("### How You Work").unwrap(), text.find(questions).unwrap(), text.find(training).unwrap(), text.find("## Personalization").unwrap());
    assert!(how < q && q < t && t < pers);
    assert!(text[t..].starts_with(&format!("{training} An answer that sounds specific but isn't in the docs or a tool result is a guess. Never say you did or checked something without the matching action behind it.\n## Personalization")));

    let list = "\n\n- `~/docs/`\n  `browser.md` - the browser";
    assert_eq!(text.matches(list).count(), 1);
    let (fs, l, sec) = (text.find("## Filesystem").unwrap(), text.find(list).unwrap(), text.find("## Security").unwrap());
    assert!(fs < l && l < sec);
    assert!(text.contains("`chat/scheduling-and-watching.md` - choosing crons, hooks, or services and verifying delivery\n## Security"));
    // Ссылок на несделанные части в абзацах нет (П6: client-surfaces).
    assert!(!text.contains("client-surfaces") && !text.contains("chat-connections"));
}
