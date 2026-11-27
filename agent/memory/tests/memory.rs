//! Тесты П4д: память на Postgres. Временный кластер на 127.0.0.1 (`pg`),
//! поддельные модели — записанные ответы на 127.0.0.1; живых вызовов нет.

#[path = "../../loop/tests/common/mod.rs"]
mod common;
mod pg;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent_loop::{Inbox, Prefix, Subagents, Tool, ToolFuture, Tools, Transcript, Turn};
use agent_memory::{reindex, ForgetTools, MemoryTools, NewClaim, ProviderEmbedder, Store};
use agent_model::openai_compat::OpenaiCompatProvider;
use agent_model::{Message, OpenaiCompatFacts, Part, Role, ToolResultPart, ToolSpec};
use common::{model, temp_dir, Step, Wire};
use serde_json::{json, Value};

fn home(name: &str) -> PathBuf {
    temp_dir(&format!("memory-{name}"))
}

fn write(home: &Path, rel: &str, text: &str) {
    let p = home.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

async fn store() -> Store {
    Store::connect(&pg::database()).await.unwrap()
}

/// Инструменты памяти и её фоновая работа. Сверка с файлами идёт в фоне
/// (`maintain`); помощники ниже прогоняют её перед вызовом — так, будто
/// такт службы уже прошёл.
struct Mem {
    tools: Tools,
    m: MemoryTools,
}

impl std::ops::Deref for Mem {
    type Target = Tools;
    fn deref(&self) -> &Tools {
        &self.tools
    }
}

impl std::ops::DerefMut for Mem {
    fn deref_mut(&mut self) -> &mut Tools {
        &mut self.tools
    }
}

fn tools_for(m: &MemoryTools) -> Mem {
    let mut t = Tools::new();
    m.add_to(&mut t);
    Mem { tools: t, m: m.clone() }
}

async fn call(tools: &Mem, name: &str, args: Value) -> Result<String, String> {
    tools.m.maintain().await.unwrap();
    call_now(tools, name, args).await
}

/// Вызов без фоновой сверки.
async fn call_now(tools: &Tools, name: &str, args: Value) -> Result<String, String> {
    match tools.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(text.clone()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

async fn search(tools: &Mem, q: &str) -> Value {
    serde_json::from_str(&call(tools, "hamlet.memory_search", json!({"queries": [q]})).await.unwrap()).unwrap()
}

fn paths(v: &Value) -> Vec<String> {
    v["memories"].as_array().unwrap().iter().map(|m| m["citation"].as_str().unwrap().to_string()).collect()
}

fn claim(id: &str, text: &str, supersedes: Option<&str>) -> NewClaim {
    NewClaim {
        claim_id: id.into(),
        run_id: "run-1".into(),
        kind: "preference".into(),
        salience: "high".into(),
        claim_text: text.into(),
        quote: Some(text.into()),
        speaker: "user".into(),
        evidence_handles: vec!["message:m1".into()],
        confidence: 0.9,
        first_seen: Some("2026-11-20 10:00:00+00".into()),
        valid_until: None,
        source_path: "memory/2026-11-20.md".into(),
        source_line: 3,
        supersedes: supersedes.map(String::from),
    }
}

const MEMORY_MD: &str = "# Owner\n\nThe owner's cat is named Pushok.\n\nThe owner works as a bridge engineer in Lisbon.\n";

#[tokio::test]
async fn write_and_search_by_words() {
    let h = home("words");
    write(&h, "MEMORY.md", MEMORY_MD);
    let m = MemoryTools::new(&h, store().await, None);
    let t = tools_for(&m);
    let r = search(&t, "bridge engineer").await;
    assert_eq!(paths(&r), ["MEMORY.md#L5"], "{r}");
    let hit = &r["memories"][0];
    assert_eq!((hit["path"].as_str(), hit["from"].as_i64(), hit["source_type"].as_str()), (Some("MEMORY.md"), Some(5), Some("memory_md")));
    assert!(hit["snippet"].as_str().unwrap().contains("Lisbon"));
    assert!(r["note"].as_str().unwrap().contains("hamlet.memory_get"));
    // Файл поменялся — поиск видит новое и не видит старое.
    write(&h, "MEMORY.md", "# Owner\n\nThe owner moved to Porto and teaches physics.\n");
    assert!(paths(&search(&t, "bridge engineer").await).is_empty());
    assert_eq!(paths(&search(&t, "physics").await), ["MEMORY.md#L3"]);
}

#[tokio::test]
async fn memory_dir_files_are_searched_and_dropped() {
    let h = home("dir");
    write(&h, "memory/2026-11-20.md", "- 10:02 The owner booked a dentist visit for Friday.\n");
    write(&h, "memory/people/INDEX.md", "# People\n\nAnna is the owner's sister, a violinist.\n");
    write(&h, "notes.md", "violinist dentist outside memory\n");
    let s = store().await;
    let m = MemoryTools::new(&h, s.clone(), None);
    let t = tools_for(&m);
    let r = search(&t, "dentist").await;
    assert_eq!(paths(&r), ["memory/2026-11-20.md#L1"], "{r}");
    assert_eq!(r["memories"][0]["source_type"], "daily");
    let r = search(&t, "violinist").await;
    assert_eq!(paths(&r), ["memory/people/INDEX.md#L3"]);
    assert_eq!(r["memories"][0]["source_type"], "person");
    std::fs::remove_file(h.join("memory/people/INDEX.md")).unwrap();
    assert!(paths(&search(&t, "violinist").await).is_empty());
    let rep = reindex(&h, &s, None).await.unwrap();
    assert!(rep.files_indexed.is_empty() && rep.files_removed.is_empty(), "без изменений ничего не переписывается: {rep:?}");
}

#[tokio::test]
async fn claim_supersedes_old_one() {
    let s = store().await;
    assert_eq!(s.add_claim(&claim("c1", "The owner prefers tea.", None)).await.unwrap(), "c1");
    // То же утверждение другими пробелами и регистром — подкрепляется, не дублируется.
    assert_eq!(s.add_claim(&claim("c1b", "the owner  prefers TEA", None)).await.unwrap(), "c1");
    assert_eq!(s.add_claim(&claim("c2", "The owner now prefers coffee.", Some("c1"))).await.unwrap(), "c2");
    let old = s.claim("c1").await.unwrap().unwrap();
    let new = s.claim("c2").await.unwrap().unwrap();
    assert_eq!((old.status.as_str(), new.status.as_str()), ("superseded", "active"));
    assert_eq!(new.supersedes_claim_id.as_deref(), Some("c1"));
    assert!(s.claim("c1b").await.unwrap().is_none());
    let (earlier, later) = s.replacements("c2").await.unwrap();
    assert_eq!((earlier.len(), earlier[0].claim_id.as_str(), later.len()), (1, "c1", 0));

    // explain по строке дневника с `claim:`.
    let h = home("explain");
    write(&h, "memory/2026-11-20.md", "- The owner now prefers coffee.\n  claim: c2\n\n- Plain note without a claim.\n");
    let t = tools_for(&MemoryTools::new(&h, s.clone(), None));
    let out = call(&t, "hamlet.memory_explain", json!({"id": "memory/2026-11-20.md#L1"})).await.unwrap();
    assert!(out.contains("claim c2") && out.contains("Replaced these earlier claims:\n- c1 (superseded): The owner prefers tea."), "{out}");
    assert!(out.contains("indexed line memory/2026-11-20.md#L1-L2"), "{out}");
    let out = call(&t, "hamlet.memory_explain", json!({"id": "c1"})).await.unwrap();
    assert!(out.contains("Replaced by these later claims:\n- c2 (active)"), "{out}");
    let out = call(&t, "hamlet.memory_explain", json!({"id": "memory/2026-11-20.md#L4"})).await.unwrap();
    assert!(out.starts_with("No verified claim stands behind this memory"), "{out}");
    let r: Value = serde_json::from_str(&call(&t, "hamlet.memory_search", json!({"queries": ["coffee"]})).await.unwrap()).unwrap();
    assert_eq!(r["memories"][0]["metadata"]["claim_id"], "c2");
}

/// Поддельный `/v1/embeddings`: «кошачьи» слова — ось 0, остальное — ось 1.
fn embeddings_server() -> (String, Arc<Mutex<usize>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let calls = Arc::new(Mutex::new(0usize));
    let seen = calls.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                let h = h.trim_end().to_string();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    if k.trim().eq_ignore_ascii_case("content-length") {
                        len = v.trim().parse().unwrap();
                    }
                }
            }
            let mut raw = vec![0; len];
            r.read_exact(&mut raw).unwrap();
            *seen.lock().unwrap() += 1;
            let body: Value = serde_json::from_slice(&raw).unwrap();
            let data: Vec<Value> = body["input"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let t = t.as_str().unwrap().to_lowercase();
                    let mut v = vec![0.0f32; 384];
                    let cat = ["cat", "feline", "kitten"].iter().any(|w| t.contains(w));
                    v[if cat { 0 } else { 1 }] = 1.0;
                    json!({"index": i, "embedding": v})
                })
                .collect();
            let text = json!({"data": data}).to_string();
            let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}", text.len());
        }
    });
    (base, calls)
}

#[tokio::test]
async fn vector_search_only_with_embeddings_model() {
    let h = home("vectors");
    write(&h, "MEMORY.md", MEMORY_MD);
    // Без модели векторов: «feline companion» словами не находится.
    let words_only = tools_for(&MemoryTools::new(&h, store().await, None));
    assert!(paths(&search(&words_only, "feline companion").await).is_empty());
    // С моделью: находится по смыслу; векторы записаны в базу.
    let (base, calls) = embeddings_server();
    let emb = ProviderEmbedder {
        name: "fake-embed".into(),
        provider: OpenaiCompatProvider::new("fake-embed", OpenaiCompatFacts::default(), base, None).unwrap(),
    };
    let s = store().await;
    let t = tools_for(&MemoryTools::new(&h, s.clone(), Some(Arc::new(emb))));
    let r = search(&t, "feline companion").await;
    assert_eq!(paths(&r)[0], "MEMORY.md#L3", "{r}");
    assert!((r["memories"][0]["score"].as_f64().unwrap() - 1.0).abs() < 1e-6);
    let stored: i64 = sqlx::query_scalar("select count(*) from memory.embeddings").fetch_one(s.pool()).await.unwrap();
    assert_eq!(stored, 3, "по вектору на запись");
    assert!(*calls.lock().unwrap() >= 2);
}

#[tokio::test]
async fn embeddings_model_down_falls_back_to_words() {
    let h = home("emb-down");
    write(&h, "MEMORY.md", MEMORY_MD);
    let dead = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let emb = ProviderEmbedder {
        name: "fake-embed".into(),
        provider: OpenaiCompatProvider::new("fake-embed", OpenaiCompatFacts::default(), format!("http://{dead}/v1"), None).unwrap(),
    };
    let t = tools_for(&MemoryTools::new(&h, store().await, Some(Arc::new(emb))));
    assert_eq!(paths(&search(&t, "Pushok").await), ["MEMORY.md#L3"]);
}

#[tokio::test]
async fn forget_retracts_claims_and_refreshes_search() {
    let h = home("forget");
    write(&h, "memory/2026-11-20.md", "- The owner's bank is Northwind.\n  claim: b1\n\n- The owner likes jazz.\n");
    let s = store().await;
    s.add_claim(&claim("b1", "The owner's bank is Northwind.", None)).await.unwrap();
    s.add_claim(&claim("b2", "The owner's bank is Northwind Plus.", Some("b1"))).await.unwrap();
    s.add_claim(&claim("j1", "The owner likes jazz.", None)).await.unwrap();
    let t = tools_for(&MemoryTools::new(&h, s.clone(), None));
    assert_eq!(paths(&search(&t, "Northwind").await), ["memory/2026-11-20.md#L1-L2"]);
    // Агент убрал строку из файла и записал только id и место — без текста.
    write(&h, "memory/2026-11-20.md", "- The owner likes jazz.\n");
    write(&h, "workspace/memory/forget/pending.json", r#"{"claims":["b1"],"citations":["memory/2026-11-20.md#L1"]}"#);
    assert!(paths(&search(&t, "Northwind").await).is_empty());
    for (id, status) in [("b1", "retracted"), ("b2", "retracted"), ("j1", "active")] {
        assert_eq!(s.claim(id).await.unwrap().unwrap().status, status, "{id}");
    }
    assert!(!h.join("workspace/memory/forget/pending.json").exists());
    assert_eq!(paths(&search(&t, "jazz").await), ["memory/2026-11-20.md#L1"]);
}

#[tokio::test]
async fn memory_get_reads_citation_window() {
    let h = home("get");
    write(&h, "memory/2026-08-10.md", "one\ntwo\nthree\nfour\nfive\n");
    let t = tools_for(&MemoryTools::new(&h, store().await, None));
    let get = |a: Value| {
        let t = &t;
        async move { serde_json::from_str::<Value>(&call(t, "hamlet.memory_get", a).await.unwrap()).unwrap() }
    };
    let r = get(json!({"path": "memory/2026-08-10.md#L2-L3"})).await;
    assert_eq!((r["text"].as_str(), r["from"].as_i64(), r["total_lines"].as_i64()), (Some("two\nthree"), Some(2), Some(5)));
    let r = get(json!({"path": "~/memory/2026-08-10.md#L2", "from": 4, "lines": 5})).await;
    assert_eq!(r["text"], "four\nfive");
    let abs = h.join("memory/2026-08-10.md").to_string_lossy().to_string();
    assert_eq!(get(json!({"path": abs})).await["lines"], 5);
    for bad in ["notes.md", "memory/../MEMORY.md", "/etc/passwd"] {
        let e = call(&t, "hamlet.memory_get", json!({"path": bad})).await.unwrap_err();
        assert!(e.contains("is not a memory file"), "{bad}: {e}");
    }
}

#[tokio::test]
async fn side_chat_memory_is_searched() {
    let h = home("side");
    write(&h, "side-chats/c7/MEMORY.md", "Trip to Kyoto is planned for April.\n");
    write(&h, "side-chats/c7/config.json", "{\"Kyoto\": true}");
    let t = tools_for(&MemoryTools::new(&h, store().await, None));
    assert_eq!(paths(&search(&t, "Kyoto").await), ["side-chats/c7/MEMORY.md#L1"]);
}

async fn one_turn(wire: Wire) {
    let h = home(&format!("turn-{wire:?}"));
    write(&h, "MEMORY.md", MEMORY_MD);
    let tools = tools_for(&MemoryTools::new(&h, store().await, None));
    tools.m.maintain().await.unwrap();
    let steps = vec![
        Step::Calls(vec![("c1", "hamlet.memory_search", json!({"queries": ["cat name", "Pushok"]}))]),
        Step::Text("Твою кошку зовут Пушок."),
    ];
    let (server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &tools.tools, "s", temp_dir("memory-spill"));
    let mut hist = vec![Message { role: Role::User, content: vec![Part::Text { text: "как зовут мою кошку?".into() }] }];
    assert_eq!(turn.run(&mut hist, &mut |_: &str| {}).await.unwrap().text(), "Твою кошку зовут Пушок.");
    let reqs = server.requests();
    let first = reqs[0].to_string();
    assert!(first.contains("hamlet.memory_search") && first.contains("hamlet.memory_explain"), "инструменты ушли модели");
    let last = reqs[1]["messages"].to_string();
    assert!(last.contains("Pushok") && last.contains("MEMORY.md#L3"), "{last}");
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}

/// Тестовый инструмент подагента: записать файл от дома.
struct Put(PathBuf);

impl Tool for Put {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "put".into(),
            description: "записать файл".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}, "text": {"type": "string"}}}),
        }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            write(&self.0, args["path"].as_str().unwrap(), args["text"].as_str().unwrap());
            Ok(vec![ToolResultPart::Text { text: "ok".into() }])
        })
    }
}

async fn wait_inbox(inbox: &Inbox) -> Vec<Message> {
    tokio::time::timeout(std::time::Duration::from_secs(10), inbox.wait()).await.expect("итог не пришёл");
    inbox.drain()
}

async fn forget_flow(wire: Wire) {
    let h = home(&format!("forget-flow-{wire:?}"));
    write(&h, "memory/2026-11-20.md", "- The owner's bank is Northwind.\n  claim: b1\n\n- The owner likes jazz.\n");
    let s = store().await;
    s.add_claim(&claim("b1", "The owner's bank is Northwind.", None)).await.unwrap();
    write(&h, "skills/forget/SKILL.md", "# Forgetting\n");
    let skill = h.join("skills/forget/SKILL.md");

    // Подагенты: план, затем исполнитель (правит файл и пишет pending.json).
    let child_steps = vec![
        Step::Text("План: убрать строку о банке и утверждение b1."),
        Step::Calls(vec![
            ("e1", "put", json!({"path": "memory/2026-11-20.md", "text": "- The owner likes jazz.\n"})),
            ("e2", "put", json!({"path": "workspace/memory/forget/pending.json", "text": "{\"claims\":[\"b1\"],\"citations\":[\"memory/2026-11-20.md#L1\"]}"})),
        ]),
        Step::Text("Убрал."),
    ];
    let (child_server, cp) = model(wire, child_steps, false);
    let mut child_tools = Tools::new();
    child_tools.add(Put(h.clone()));
    let transcript = Transcript::new();
    let subs = Subagents::new(
        Arc::new(cp),
        Arc::new(child_tools.clone()),
        Prefix { system: "Ты — подагент.".into(), tools: child_tools.specs() },
        temp_dir("forget-child"),
        Inbox::new(),
        transcript.clone(),
    );
    let memory = MemoryTools::new(&h, s.clone(), None);
    let mut tools = tools_for(&memory);
    ForgetTools::new(subs.clone(), &h, s.clone(), None, &skill).add_to(&mut tools);
    assert_eq!(paths(&search(&tools, "Northwind").await), ["memory/2026-11-20.md#L1-L2"]);

    // Без плана исполнять нечего.
    let e = call(&tools, "forget.confirm", json!({})).await.unwrap_err();
    assert!(e.contains("requires a completed forget plan"), "{e}");

    let parent_steps = vec![
        Step::Calls(vec![("p1", "forget.plan", json!({}))]),
        Step::Text("Готовлю план."),
        Step::Calls(vec![("p2", "forget.confirm", json!({}))]),
        Step::Text("Убираю."),
    ];
    let (_parent_server, pp) = model(wire, parent_steps, false);
    let mut turn = Turn::new(&pp, &tools.tools, "Ты — Hamlet.", temp_dir("forget-parent"));
    turn.transcript = Some(transcript);
    let mut hist = vec![Message { role: Role::User, content: vec![Part::Text { text: "забудь, в каком я банке".into() }] }];
    turn.run(&mut hist, &mut |_: &str| {}).await.unwrap();
    let plan = wait_inbox(subs.inbox()).await;
    assert!(plan[0].text().contains("План: убрать строку"), "{plan:?}");
    // План унаследовал разговор: просьба владельца в нём есть, задание — текст ядра.
    let first = child_server.requests()[0].to_string();
    assert!(first.contains("забудь, в каком я банке") && first.contains("This turn is planning only."), "{first}");
    assert!(first.contains(&skill.display().to_string()));

    hist.extend(plan);
    hist.push(Message { role: Role::User, content: vec![Part::Text { text: "да, убирай".into() }] });
    turn.run(&mut hist, &mut |_: &str| {}).await.unwrap();
    let done = wait_inbox(subs.inbox()).await;
    let note = done[0].text();
    assert!(note.contains("Убрал.") && note.contains("[Memory refresh] Retracted 1 claims"), "{note}");
    // Исполнитель видел план и согласие.
    let exec = child_server.requests()[1].to_string();
    assert!(exec.contains("План: убрать строку") && exec.contains("да, убирай") && exec.contains("pending.json"), "{exec}");
    assert_eq!(s.claim("b1").await.unwrap().unwrap().status, "retracted");
    assert!(paths(&search(&tools, "Northwind").await).is_empty());
    assert_eq!(paths(&search(&tools, "jazz").await), ["memory/2026-11-20.md#L1"]);
    assert!(!h.join("workspace/memory/forget/pending.json").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_flow_anthropic() {
    forget_flow(Wire::Anthropic).await
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_flow_openai_compat() {
    forget_flow(Wire::OpenaiCompat).await
}

#[tokio::test]
async fn search_sees_files_after_background_maintenance() {
    let h = home("lag");
    write(&h, "MEMORY.md", MEMORY_MD);
    let m = MemoryTools::new(&h, store().await, None);
    let t = tools_for(&m);
    // Поиск сам базу с файлами не сверяет — выдача отстаёт от записи.
    let r: Value = serde_json::from_str(&call_now(&t, "hamlet.memory_search", json!({"queries": ["Pushok"]})).await.unwrap()).unwrap();
    assert!(paths(&r).is_empty(), "{r}");
    m.maintain().await.unwrap();
    let r: Value = serde_json::from_str(&call_now(&t, "hamlet.memory_search", json!({"queries": ["Pushok"]})).await.unwrap()).unwrap();
    assert_eq!(paths(&r), ["MEMORY.md#L3"]);
}
