//! Тесты П4г: сеть. Только поддельный сервер Firecrawl на 127.0.0.1 —
//! ответы в форме документации Firecrawl (POST /v2/search, POST /v2/scrape);
//! живых вызовов нет.

#[path = "../../loop/tests/common/mod.rs"]
mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_loop::{Tools, Turn};
use agent_model::{Message, Part, Role, ToolResultPart};
use agent_tools::web::SearchConfig;
use agent_tools::{Blocklist, WebConfig, WebTools};
use common::{model, temp_dir, Step, Wire};
use serde_json::{json, Value};

#[derive(Clone, Debug)]
struct Seen {
    path: String,
    auth: String,
    body: Value,
}

/// Поддельный Firecrawl: ответ по пути и по `url` в теле scrape.
struct Firecrawl {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

fn firecrawl(search: (u16, Value), pages: HashMap<String, (u16, Value)>) -> Firecrawl {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v2", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let (mut len, mut auth) = (0usize, String::new());
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                let h = h.trim_end().to_string();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "content-length" => len = v.trim().parse().unwrap(),
                        "authorization" => auth = v.trim().to_string(),
                        _ => {}
                    }
                }
            }
            let mut raw = vec![0; len];
            r.read_exact(&mut raw).unwrap();
            let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
            log.lock().unwrap().push(Seen { path: path.clone(), auth, body: body.clone() });
            let (status, reply) = if path.ends_with("/search") {
                search.clone()
            } else {
                let url = body["url"].as_str().unwrap_or("");
                pages.get(url).cloned().unwrap_or((404, json!({"success": false, "error": "Not found", "code": "NOT_FOUND"})))
            };
            let text = reply.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            );
        }
    });
    Firecrawl { base, seen }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn config() -> WebConfig {
    WebConfig::parse(&std::fs::read_to_string(repo_root().join("config/web.toml")).unwrap()).unwrap()
}

/// Инструменты сети с поддельным сервером и своей переменной ключа.
fn web(server: &Firecrawl, key_env: &str, key: Option<&str>) -> Tools {
    let cfg = config();
    let search = SearchConfig { base_url: server.base.clone(), api_key_env: key_env.into(), ..cfg.search };
    match key {
        Some(k) => std::env::set_var(key_env, k),
        None => std::env::remove_var(key_env),
    }
    let mut tools = Tools::new();
    WebTools::new(search, Blocklist::load(&cfg.blocklist, &repo_root()).unwrap()).add_to(&mut tools);
    tools
}

async fn call(tools: &Tools, name: &str, args: Value) -> Result<String, String> {
    match tools.get(name).unwrap().call(args).await {
        Ok(parts) => match &parts[0] {
            ToolResultPart::Text { text } => Ok(text.clone()),
            _ => panic!(),
        },
        Err(e) => Err(e.to_string()),
    }
}

fn search_reply() -> (u16, Value) {
    (
        200,
        json!({"success": true, "data": {"web": [
            {"url": "https://example.org/rust", "title": "Rust language", "description": "A language empowering everyone."},
            {"url": "https://blog.example.net/post?id=7", "title": "Release notes", "description": "What is new in 1.100."},
        ]}, "creditsUsed": 1}),
    )
}

fn page(markdown: &str, title: &str, url: &str) -> (u16, Value) {
    (200, json!({"success": true, "data": {"markdown": markdown, "metadata": {"title": title, "sourceURL": url, "url": url, "statusCode": 200}}}))
}

fn pages() -> HashMap<String, (u16, Value)> {
    let mut m = HashMap::new();
    m.insert(
        "https://example.org/rust".into(),
        page(
            "# Rust\nRust is fast.\nSee [the book](/book) and [crates](https://crates.io/).\n![logo](https://example.org/logo.png)\nRUST is safe.",
            "Rust language",
            "https://example.org/rust",
        ),
    );
    m.insert("https://example.org/book".into(), page("# The Book\nChapter 1.", "The Book", "https://example.org/book"));
    m
}

fn id_of(out: &str, marker: &str) -> u64 {
    out.split(marker).nth(1).unwrap().split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

#[tokio::test]
async fn search_open_find_lookup() {
    let fc = firecrawl(search_reply(), pages());
    let tools = web(&fc, "AGENT_TEST_FIRECRAWL_KEY_1", Some("fc-fake-1"));

    let out = call(&tools, "browser.search", json!({"primary_query": {"query": "rust language", "language_code": "en"}})).await.unwrap();
    let session = id_of(&out, "(search session ");
    assert!(out.contains("【0†Rust language†example.org】\nA language empowering everyone."), "{out}");
    assert!(out.contains("Full URLs for search results (use verbatim; never guess from domains):\n0: https://example.org/rust\n1: https://blog.example.net/post?id=7\n"));
    assert!(out.ends_with("If the exact full URL is not listed here, do not link to it."));
    let s = fc.seen.lock().unwrap()[0].clone();
    assert_eq!((s.path.as_str(), s.auth.as_str()), ("/v2/search", "Bearer fc-fake-1"));
    assert_eq!(s.body, json!({"query": "rust language", "limit": 10}));

    // Ссылка из выдачи.
    let urls: Value = serde_json::from_str(&call(&tools, "browser.lookup_citation_url", json!({"page_id": session, "outlink_indices": [1]})).await.unwrap()).unwrap();
    assert_eq!(urls, json!([{"index": 1, "url": "https://blog.example.net/post?id=7", "title": "Release notes"}]));

    // Открыть результат 0 выдачи — без адреса.
    let out = call(&tools, "browser.open", json!({"url_id": session, "outlink_idx": 0})).await.unwrap();
    let pg = id_of(&out, "Page ");
    assert!(out.contains("URL: https://example.org/rust"));
    assert!(out.contains("L2: See 【0†the book】 and 【1†crates】."), "{out}");
    assert!(out.contains("L3: [Image 1]"));
    assert!(out.contains("Images:\nL") || out.contains("L6: Images:"), "{out}");
    assert!(out.contains("[Image 1]: https://example.org/logo.png"));
    assert_eq!(fc.seen.lock().unwrap()[1].body, json!({"url": "https://example.org/rust", "formats": ["markdown"]}));

    // find — по полученному тексту, без сети.
    let before = fc.seen.lock().unwrap().len();
    let f = call(&tools, "browser.find", json!({"pattern": "rust is", "url_id": pg})).await.unwrap();
    assert!(f.contains("L1: Rust is fast.") && f.contains("L4: RUST is safe."), "{f}");
    let f = call(&tools, "browser.find", json!({"pattern": "rust is", "url_id": pg, "line_start": 2})).await.unwrap();
    assert!(!f.contains("L1:") && f.contains("L4:"));
    assert!(call(&tools, "browser.find", json!({"pattern": "", "url_id": pg})).await.unwrap_err().contains("pattern must not be empty"));
    // line_start по уже полученной странице — тоже без сети.
    let out = call(&tools, "browser.open", json!({"url_id": pg, "line_start": 4})).await.unwrap();
    assert!(out.contains("L4: RUST is safe.") && !out.contains("L1:"));
    assert_eq!(fc.seen.lock().unwrap().len(), before);

    // Ссылка страницы по номеру; относительный адрес разрешён от страницы.
    let out = call(&tools, "browser.open", json!({"url_id": pg, "outlink_idx": 0})).await.unwrap();
    assert!(out.contains("URL: https://example.org/book") && out.contains("Chapter 1."));
    // Адрес строкой.
    let out = call(&tools, "browser.open", json!({"url_id": "https://example.org/book"})).await.unwrap();
    assert!(out.contains("The Book"));
    assert!(call(&tools, "browser.open", json!({"url_id": "ab12"})).await.unwrap_err().contains("looks like a citation ID"));
}

#[tokio::test]
async fn blocklisted_urls_are_not_opened() {
    let fc = firecrawl(search_reply(), pages());
    let tools = web(&fc, "AGENT_TEST_FIRECRAWL_KEY_2", Some("fc-fake-2"));
    for url in [
        // домен из mase_all_domains.fst
        "https://empresademudancapompeia.blogspot.hr/page",
        // шаблон из mase_blocklist_regex.json
        "https://news.celebri.com/x",
    ] {
        let err = call(&tools, "browser.open", json!({"url_id": url})).await.unwrap_err();
        assert!(err.contains("is on the URL blocklist. Nothing was opened."), "{url}: {err}");
    }
    // Настоящая маска из списка muse: «*.instagram-login.page» закрывает
    // сам домен и его поддомены, соседний домен открыт.
    let cfg = config();
    let bl = Blocklist::load(&cfg.blocklist, &repo_root()).unwrap();
    assert!(bl.blocks("https://instagram-login.page/"));
    assert!(bl.blocks("https://secure.instagram-login.page/login"));
    assert!(bl.blocks("https://a.b.instagram-login.page/"));
    assert!(!bl.blocks("https://instagram-login.pages.dev/"));
    assert!(!bl.blocks("https://notinstagram-login.page/"));
    let err = call(&tools, "browser.open", json!({"url_id": "https://secure.instagram-login.page/login"})).await.unwrap_err();
    assert!(err.contains("is on the URL blocklist"), "{err}");
    assert!(fc.seen.lock().unwrap().is_empty(), "до поставщика не дошло");
    assert!(call(&tools, "browser.open", json!({"url_id": "https://example.org/rust"})).await.is_ok());
}

#[tokio::test]
async fn provider_error_without_key_in_output() {
    let mut p = HashMap::new();
    p.insert(
        "https://example.org/paid".to_string(),
        (402, json!({"success": false, "error": "Insufficient credits for key fc-fake-secret-3", "code": "insufficient_credits"})),
    );
    let fc = firecrawl((500, json!({"success": false, "code": "UNKNOWN_ERROR", "error": "boom"})), p);
    let tools = web(&fc, "AGENT_TEST_FIRECRAWL_KEY_3", Some("fc-fake-secret-3"));
    let err = call(&tools, "browser.open", json!({"url_id": "https://example.org/paid"})).await.unwrap_err();
    assert!(err.contains("search provider error (HTTP 402): Insufficient credits"), "{err}");
    assert!(!err.contains("fc-fake-secret-3"), "{err}");
    let err = call(&tools, "browser.search", json!({"primary_query": {"query": "x", "language_code": "en"}})).await.unwrap_err();
    assert!(err.contains("HTTP 500") && err.contains("boom"), "{err}");
    assert!(call(&tools, "browser.search", json!({"primary_query": {"query": " ", "language_code": "en"}}))
        .await
        .unwrap_err()
        .contains("primary_query.query must be non-empty"));

    // Без ключа — понятная ошибка, до сервера не доходит.
    let fc = firecrawl(search_reply(), HashMap::new());
    let tools = web(&fc, "AGENT_TEST_FIRECRAWL_KEY_4", None);
    let err = call(&tools, "browser.search", json!({"primary_query": {"query": "x", "language_code": "en"}})).await.unwrap_err();
    assert!(err.contains("AGENT_TEST_FIRECRAWL_KEY_4"), "{err}");
    assert!(fc.seen.lock().unwrap().is_empty());
}

#[test]
fn config_and_specs() {
    let c = config();
    assert_eq!((c.search.provider.as_str(), c.search.api_key_env.as_str()), ("firecrawl", "FIRECRAWL_API_KEY"));
    assert_eq!(c.search.base_url, "https://api.firecrawl.dev/v2");
    assert!(WebConfig::parse("[search]\nprovider = \"other\"\nbase_url = \"x\"\napi_key_env = \"K\"\n[blocklist]\ndomains = \"a\"\nregex = \"b\"\n").is_err());
    let mut tools = Tools::new();
    WebTools::new(c.search.clone(), Blocklist::load(&c.blocklist, &repo_root()).unwrap()).add_to(&mut tools);
    let specs = tools.specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["browser.search", "browser.open", "browser.find", "browser.lookup_citation_url"]);
    assert!(specs[0].description.starts_with("Search the web using a text query"));
    assert!(!specs.iter().any(|s| s.description.contains("<!--")));
}

async fn one_turn(wire: Wire) {
    let fc = firecrawl(search_reply(), pages());
    let tools = web(&fc, &format!("AGENT_TEST_FIRECRAWL_KEY_TURN_{wire:?}"), Some("fc-fake-turn"));
    // Номер выдачи заранее неизвестен — модель открывает адрес из выдачи.
    let steps = vec![
        Step::Calls(vec![("c1", "browser.search", json!({"primary_query": {"query": "rust", "language_code": "en"}}))]),
        Step::Calls(vec![("c2", "browser.open", json!({"url_id": "https://example.org/rust"}))]),
        Step::Text("Rust — быстрый язык."),
    ];
    let (server, p) = model(wire, steps, false);
    let turn = Turn::new(&p, &tools, "s", temp_dir("web-spill"));
    let mut h = vec![Message { role: Role::User, content: vec![Part::Text { text: "что такое rust?".into() }] }];
    assert_eq!(turn.run(&mut h, &mut |_: &str| {}).await.unwrap().text(), "Rust — быстрый язык.");
    let last = server.requests()[2]["messages"].to_string();
    assert!(last.contains("Rust is fast.") && last.contains("Full URLs for search results"), "{last}");
    assert!(!last.contains("fc-fake-turn"));
}

#[tokio::test]
async fn one_turn_anthropic() {
    one_turn(Wire::Anthropic).await
}

#[tokio::test]
async fn one_turn_openai_compat() {
    one_turn(Wire::OpenaiCompat).await
}
