//! Поиск и чтение страниц: `browser.search`, `browser.open`, `browser.find`,
//! `browser.lookup_citation_url` — по форме muse `browser_*.rs`
//! (`source-files.txt:1504-1510`) и крейта `web-search` (`:7359-7362`).
//!
//! Поставщик — Firecrawl (у muse он один из бэкендов инструментов браузера:
//! `BrowserToolBackend` perplexity / firecrawl / page_cache,
//! `strings-all.txt:245415`). Полученные выдачи и страницы держатся в памяти
//! процесса — бэкенд `page_cache` muse с операциями `url_fetch`,
//! `cached_page_scroll`, `url_outlink`, `cached_page_outlink` (строка ядра
//! 0x86c547): номер выдачи или страницы — большое целое, ссылки страницы —
//! `【idx†текст】`, по ним `browser.open` идёт дальше без адреса.
//! Чёрный список адресов muse — как есть (`config/blocklist/`).

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_loop::{personal, Tool, ToolError, ToolFuture, Tools};
use agent_model::{ToolResultPart, ToolSpec};
use serde::Deserialize;
use serde_json::{json, Value};

// Описания — строки ядра muse, полные варианты, личный вариант:
// browser.search — 0x999b16–0x99a5a9 и параметры 0x99a7d3…, 0x9983f0;
// browser.find — 0x86ae1a…; browser.open — 0x86afb8…; lookup — 0x870c8d….
const SEARCH_DESCRIPTION: &str = "Search the web using a text query: a question, topic, or descriptive keywords, not a URL, pasted link, or bare web address. Set `primary_query.language_code` on every call to the ISO 639-1 code of the language the query is written in. This finds information; it does not open a website in the<!-- personal-muse-only:start --> user's<!-- personal-muse-only:end --> live browser or carry out an action there. Do not translate a request to visit a website into a search for it. Use search to discover sources and answer questions its results can establish. Retrieve changing facts rather than relying on training data. For a product search or a product buying decision, except groceries and prepared meals, use the shopping skill instead to find, compare, or evaluate products from a curated catalog, Marketplace, and the web. This includes choosing between products or deciding whether one is worth buying.\n\nUse the matching vertical for supported current information: `news`, `sports`, `weather`, `finance`, or `datetime`. Check the result's source, time, entity and conditions. When its data establishes the required fact, a separate browser check is unnecessary. General search results may be stale and do not establish current inventory, merchant prices, booking availability or account state. Verify those through a relevant skill or connector that provides the required live information, or the actual site in the live browser. Go directly to that live source when the provider is known; search is not a prerequisite. Adding today's date or repeating a query does not verify the underlying state.\n\nThe output ends with `Full URLs for search results (use verbatim; never guess from domains):` when full result URLs are available. Treat each `N: URL` line as the exact URL for search result index `N`; copy it verbatim if you need to use or show that URL. Do not create, guess, shorten, expand, or modify URLs from result titles, snippets, or domains. If an index says no full URL was returned, do not use a URL for that result unless another tool returns one.\n\nUse runtime-supplied finance data only when needed for the <!-- personal-muse-only:start -->user's <!-- personal-muse-only:end -->current task.\n\nLink to or summarize copyrighted material instead of reproducing it. Quote no more than 50 words of any one copyrighted work in total, counting every turn, file, tool and background task together, and do not retell it in detail. Finding a work through another source does not make copying it permissible. Public-domain and openly licensed works are fine to quote.";
const SEARCH_PRIMARY: &str = "Search query with query text and language code.";
const SEARCH_QUERY: &str = "A text query describing the information to find, not a URL, link, or bare web address. Keep it brief while retaining specifics. Include a location or timeframe when needed to answer the user's question.";
const SEARCH_LANGUAGE: &str = "Language of the generated search query text. Expressed as an ISO 639-1 language code (e.g., 'en' for English, 'zh' for Chinese, 'es' for Spanish). Use null only when the language cannot be determined.";
const SEARCH_VERTICALS: &str = "Verticals relevant to the search. If you set this field, special per-vertical handling in this tool is triggered. You MUST set this field to a vertical if the user's message is related to the verticals. Include at most ONE vertical: if the message relates to multiple verticals, set this field to the most relevant one. For example, if the user is messaging about sports, including the 'sports' vertical enables this tool to pull real time data, such as scores and schedules.";
const FIND_DESCRIPTION: &str = "Find exact matches of `pattern` in a page already opened with `browser.open`.\n\nLink to or summarize copyrighted material instead of reproducing it. Quote no more than 50 words of any one copyrighted work in total, counting every turn, file, tool and background task together, and do not retell it in detail. Finding a work through another source does not make copying it permissible. Public-domain and openly licensed works are fine to quote.";
const FIND_PATTERN: &str = "Text to search for (case-insensitive exact match).";
const FIND_PAGE: &str = "Integer page ID from a previous browser.open result to search within.";
const FIND_LINE: &str = "0-indexed line number to start searching from. Useful for finding later occurrences after a previous browser.find call.";
const OPEN_DESCRIPTION: &str = "Fetch page text to read, summarize, or answer questions that the text can establish. This doesn't open the page in the <!-- personal-muse-only:start -->user's<!-- personal-muse-only:end --><!-- group-muse-only:start -->live<!-- group-muse-only:end --> browser. Do not use it for requests to go to, open, or visit a website. It cannot click, sign in, fill forms, or make purchases. For facts that require current site or account state, use a relevant skill or connector that provides the required live information, or the live browser; a text fetch is not a prerequisite.\n\nPrefer page references: pass a returned integer search-session or page ID as `url_id`, with `outlink_idx` to select a search result or a real link from a referenced page. A literal URL must be copied verbatim from a prior tool result or a link <!-- personal-muse-only:start -->the user<!-- personal-muse-only:end --><!-- group-muse-only:start -->a member<!-- group-muse-only:end --> supplied in this conversation. Never guess a URL or construct one from a domain. Use `line_start` with a returned page ID to continue reading its text; this doesn't scroll the <!-- personal-muse-only:start -->user's<!-- personal-muse-only:end --><!-- group-muse-only:start -->live<!-- group-muse-only:end --> browser. When search output warns that a live check found some URL destinations unavailable, do not open a result index whose mapping is absent from its `Full URLs for search results` section. If the tool cannot read a page, follow any recovery guidance provided by the runtime. Avoid repeating the same failed fetch unchanged.\n\nA failed or incomplete text fetch does not establish a login barrier, failure in the live browser, or absence of the requested option. Fetched text leaves out what a page shows only after a click, such as a tab's contents. A detail a live browser task read from a page is not disproved by its absence from fetched text or search results, or by the same detail on another site. Before telling the user it was wrong, have the live browser check that page again; what it finds there decides. Follow supported recovery guidance, preserve explicit restrictions and stop conditions, and report only limitations actually established. If the text cannot establish a required fact, use a permitted live-browser check. If you cannot use the live browser, state what still needs to be verified.\n\nWhen a page carries images, its content ends with an `Images:` footnote mapping the `[Image N]` markers in the text to direct image-file URLs: the page's real images, usable wherever a direct image link is needed.\n\nLink to or summarize copyrighted material instead of reproducing it. Quote no more than 50 words of any one copyrighted work in total, counting every turn, file, tool and background task together, and do not retell it in detail. Finding a work through another source does not make copying it permissible. Public-domain and openly licensed works are fine to quote.";
const OPEN_URL_ID: &str = "Prefer an integer search-session ID from browser.search or a page ID from a prior read. A literal URL must be copied verbatim from a prior tool result or a link the user supplied in this conversation. Never guess a URL or construct one from a domain.";
const OPEN_OUTLINK: &str = "Index of a search result or a real link in a referenced page (shown as 【idx†...】 in page content). Use page references rather than reconstructing URLs. When url_id is a search session ID, this selects which result to fetch (0 = first result, 1 = second, etc.).";
const OPEN_LINE: &str = "0-indexed line number to start displaying from in the returned page text. This does not scroll the user's browser.";
const LOOKUP_DESCRIPTION: &str = "Resolve search result URLs from a prior `browser.search` call without fetching the page. Returns URL and title for each requested result. Pass `outlink_indices` to select specific results, or omit to retrieve all results from the search session.\n\nCall this tool when you need actual URLs from prior `browser.search` results and the search output did not already provide the full URL in its `Full URLs for search results (use verbatim; never guess from domains):` section, or when you need to resolve selected result indices again. Include the resolved URLs directly in your response. Do not call if no URLs were requested or if no prior `browser.search` was performed.\n\nWhen search output warns that a live check found some URL destinations unavailable, an index whose mapping is absent from its `Full URLs for search results` section is intentionally unavailable. Do not request, use, or cite that index. Pass explicit surviving `outlink_indices`; do not omit `outlink_indices` to request every result.\n\nOnly include URLs returned by this tool. Use each URL verbatim: do not shorten, normalize, or reconstruct URLs. Never fabricate URLs or construct them from domain names seen in search results, even for well-known sites.";
const LOOKUP_PAGE: &str = "The search result page ID (large integer) returned by browser.search.";
const LOOKUP_INDICES: &str = "0-indexed positions of results to retrieve. Omit or pass empty array to get all results. Maximum 30 results per call.";

/// Хвост выдачи — строки ядра 0x2fd0093.
const FULL_URLS: &str = "Full URLs for search results (use verbatim; never guess from domains):";
const FULL_URLS_NOTE: &str = "Each line maps to the search result with the same 0-based index above. Use only these exact full URLs, verbatim, when linking or citing results. If you only know the domain, or you can only guess an approximate path, slug, video ID, product ID, or article URL, do not use that URL. Never invent, append, shorten, expand, or modify a URL because a domain appeared in search results. If the exact full URL is not listed here, do not link to it.";
const LOOKUP_MAX: usize = 30;
const VERTICALS: [&str; 5] = ["news", "sports", "weather", "finance", "datetime"];

/// Настройка сети — `config/web.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    pub search: SearchConfig,
    pub blocklist: BlocklistConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key_env: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlocklistConfig {
    pub domains: String,
    pub regex: String,
}

impl WebConfig {
    pub fn parse(text: &str) -> Result<Self, String> {
        let c: WebConfig = toml::from_str(text).map_err(|e| format!("web config: {e}"))?;
        if c.search.provider != "firecrawl" {
            return Err(format!("unknown search provider `{}`; supported: firecrawl", c.search.provider));
        }
        Ok(c)
    }
}

/// Чёрный список: домены (FST muse) и шаблоны.
pub struct Blocklist {
    domains: fst::Set<Vec<u8>>,
    patterns: Vec<regex::Regex>,
}

impl Blocklist {
    /// Пути — относительно `root` (корень репозитория), если не абсолютные.
    pub fn load(cfg: &BlocklistConfig, root: &Path) -> Result<Self, String> {
        let at = |p: &str| if Path::new(p).is_absolute() { p.into() } else { root.join(p) };
        let bytes = std::fs::read(at(&cfg.domains)).map_err(|e| format!("blocklist domains: {e}"))?;
        let domains = fst::Set::new(bytes).map_err(|e| format!("blocklist domains: {e}"))?;
        #[derive(Deserialize)]
        struct Rx {
            blocked_regex_patterns: Vec<String>,
        }
        let rx: Rx = serde_json::from_str(&std::fs::read_to_string(at(&cfg.regex)).map_err(|e| format!("blocklist regex: {e}"))?)
            .map_err(|e| format!("blocklist regex: {e}"))?;
        let patterns = rx
            .blocked_regex_patterns
            .iter()
            .map(|p| regex::RegexBuilder::new(&format!("^(?:{p})$")).case_insensitive(true).build())
            .collect::<Result<_, _>>()
            .map_err(|e| format!("blocklist regex: {e}"))?;
        Ok(Blocklist { domains, patterns })
    }

    /// Адрес в списке: хост целиком; маска `*.X` закрывает X и все его
    /// поддомены; или шаблон по хосту.
    pub fn blocks(&self, url: &str) -> bool {
        let Some(host) = reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_lowercase())) else {
            return false;
        };
        if self.domains.contains(&host) || self.patterns.iter().any(|r| r.is_match(&host)) {
            return true;
        }
        let mut rest = host.as_str();
        loop {
            if self.domains.contains(format!("*.{rest}")) {
                return true;
            }
            match rest.split_once('.') {
                Some((_, parent)) if parent.contains('.') => rest = parent,
                _ => return false,
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Hit {
    title: String,
    url: String,
    description: String,
}

#[derive(Debug, Clone)]
struct Page {
    url: String,
    title: String,
    lines: Vec<String>,
    outlinks: Vec<String>,
}

#[derive(Default)]
struct Cache {
    searches: HashMap<u64, Vec<Hit>>,
    pages: HashMap<u64, Page>,
}

struct Inner {
    cfg: SearchConfig,
    blocklist: Blocklist,
    client: reqwest::Client,
    cache: Mutex<Cache>,
}

/// Инструменты сети одного разговора.
#[derive(Clone)]
pub struct WebTools {
    inner: Arc<Inner>,
}

fn new_id() -> u64 {
    // Большое целое, как номера страниц muse; 0 не бывает.
    std::collections::hash_map::RandomState::new().build_hasher().finish() | (1 << 63)
}

impl WebTools {
    pub fn new(cfg: SearchConfig, blocklist: Blocklist) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("http client");
        WebTools { inner: Arc::new(Inner { cfg, blocklist, client, cache: Mutex::new(Cache::default()) }) }
    }

    pub fn add_to(&self, tools: &mut Tools) {
        for op in [Op::Search, Op::Open, Op::Find, Op::Lookup] {
            tools.add(WebTool { op, web: self.clone() });
        }
    }

    fn key(&self) -> Result<String, ToolError> {
        std::env::var(&self.inner.cfg.api_key_env)
            .ok()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ToolError::Failed(format!("search provider key is not set (environment variable {})", self.inner.cfg.api_key_env)))
    }

    /// Текст ошибки без ключа.
    fn clean(&self, text: String) -> String {
        match std::env::var(&self.inner.cfg.api_key_env) {
            Ok(k) if !k.is_empty() => text.replace(&k, crate::exec::REDACTED),
            _ => text,
        }
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value, ToolError> {
        let key = self.key()?;
        let url = format!("{}/{path}", self.inner.cfg.base_url.trim_end_matches('/'));
        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(&key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::Failed(self.clean(format!("search provider is unreachable: {e}"))))?;
        let status = resp.status().as_u16();
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        if status >= 400 || v["success"] == false {
            let msg = v["error"].as_str().unwrap_or("no error text");
            return Err(ToolError::Failed(self.clean(format!("search provider error (HTTP {status}): {msg}"))));
        }
        Ok(v)
    }

    async fn search(&self, args: &Value) -> Result<String, ToolError> {
        let query = args["primary_query"]["query"].as_str().unwrap_or("").trim().to_string();
        if query.is_empty() {
            // Строка ядра 0x2fcfe7e.
            return Err(ToolError::InvalidArguments("missing query: primary_query.query must be non-empty".into()));
        }
        if let Some(v) = args.get("verticals").and_then(Value::as_array) {
            for x in v {
                if !VERTICALS.contains(&x.as_str().unwrap_or("")) {
                    return Err(ToolError::InvalidArguments(format!("unknown vertical {x}")));
                }
            }
        }
        let v = self.post("search", json!({"query": query, "limit": self.inner.cfg.limit})).await?;
        let hits: Vec<Hit> = v["data"]["web"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|h| Hit {
                title: h["title"].as_str().unwrap_or("").to_string(),
                url: h["url"].as_str().unwrap_or("").to_string(),
                description: h["description"].as_str().unwrap_or("").to_string(),
            })
            .collect();
        let id = new_id();
        let mut out = format!("Search results for \"{query}\" (search session {id}):\n");
        if hits.is_empty() {
            out.push_str("\nNo results.\n");
        }
        for (i, h) in hits.iter().enumerate() {
            let host = reqwest::Url::parse(&h.url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
            out.push_str(&format!("\n【{i}†{}†{host}】\n{}\n", h.title, h.description));
        }
        if !hits.is_empty() {
            out.push_str(&format!("\n{FULL_URLS}\n"));
            for (i, h) in hits.iter().enumerate() {
                out.push_str(&format!("{i}: {}\n", h.url));
            }
            out.push_str(FULL_URLS_NOTE);
        }
        self.inner.cache.lock().unwrap().searches.insert(id, hits);
        Ok(out)
    }

    /// Адрес по ссылке: выдача + номер результата или страница + номер ссылки.
    fn resolve(&self, url_id: &Value, outlink: Option<u64>) -> Result<Target, ToolError> {
        if let Some(s) = url_id.as_str() {
            if let Ok(n) = s.trim().parse::<u64>() {
                return self.resolve(&json!(n), outlink);
            }
            if s.len() == 4 && s.chars().all(|c| c.is_ascii_alphanumeric()) {
                // Строка ядра 0x415625.
                return Err(ToolError::InvalidArguments(format!(
                    "browser_open error: url_id \"{s}\" looks like a citation ID (4 alphanumeric characters), not a URL. Use the actual page URL instead."
                )));
            }
            if reqwest::Url::parse(s).is_err() {
                return Err(ToolError::InvalidArguments(format!("`{s}` is not a URL or a page ID")));
            }
            return Ok(Target::Fetch(s.to_string()));
        }
        let Some(id) = url_id.as_u64() else {
            // Строка ядра 0x86c4f3.
            return Err(ToolError::InvalidArguments("url_id must be a string URL or integer page ID".into()));
        };
        let cache = self.inner.cache.lock().unwrap();
        if let Some(hits) = cache.searches.get(&id) {
            let i = outlink.unwrap_or(0) as usize;
            let h = hits.get(i).ok_or_else(|| ToolError::InvalidArguments(format!("search session {id} has no result {i}")))?;
            return Ok(Target::Fetch(h.url.clone()));
        }
        if let Some(p) = cache.pages.get(&id) {
            return Ok(match outlink {
                Some(i) => Target::Fetch(
                    p.outlinks
                        .get(i as usize)
                        .cloned()
                        .ok_or_else(|| ToolError::InvalidArguments(format!("page {id} has no link {i}")))?,
                ),
                None => Target::Cached(id),
            });
        }
        Err(ToolError::InvalidArguments(format!("unknown search session or page ID {id}")))
    }

    async fn open(&self, args: &Value) -> Result<String, ToolError> {
        let url_id = args.get("url_id").ok_or_else(|| ToolError::InvalidArguments("`url_id` is required".into()))?;
        let outlink = args.get("outlink_idx").and_then(Value::as_u64);
        let line_start = args.get("line_start").and_then(Value::as_u64).unwrap_or(0) as usize;
        let id = match self.resolve(url_id, outlink)? {
            Target::Cached(id) => id,
            Target::Fetch(url) => {
                if self.inner.blocklist.blocks(&url) {
                    return Err(ToolError::Failed(format!(
                        "browser_open error: {url} is on the URL blocklist. Nothing was opened."
                    )));
                }
                let v = self.post("scrape", json!({"url": url, "formats": ["markdown"]})).await?;
                let md = v["data"]["markdown"].as_str().unwrap_or("");
                let meta = &v["data"]["metadata"];
                let title = meta["title"].as_str().unwrap_or("").to_string();
                let final_url = meta["url"].as_str().or(meta["sourceURL"].as_str()).unwrap_or(&url).to_string();
                let page = render(&final_url, &title, md);
                let id = new_id();
                self.inner.cache.lock().unwrap().pages.insert(id, page);
                id
            }
        };
        let cache = self.inner.cache.lock().unwrap();
        let p = &cache.pages[&id];
        let mut out = format!("Page {id}: {}\nURL: {}\nLines: {}\n\n", p.title, p.url, p.lines.len());
        for (n, l) in p.lines.iter().enumerate().skip(line_start) {
            out.push_str(&format!("L{n}: {l}\n"));
        }
        Ok(out)
    }

    fn find(&self, args: &Value) -> Result<String, ToolError> {
        let pattern = args.get("pattern").and_then(Value::as_str).unwrap_or("");
        if pattern.is_empty() {
            // Строка ядра 0x86af99.
            return Err(ToolError::InvalidArguments("pattern must not be empty".into()));
        }
        let id = match args.get("url_id") {
            Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
            Some(v) => v.as_u64(),
            None => None,
        }
        .ok_or_else(|| ToolError::InvalidArguments("`url_id` (integer page ID) is required".into()))?;
        let start = args.get("line_start").and_then(Value::as_u64).unwrap_or(0) as usize;
        let cache = self.inner.cache.lock().unwrap();
        let p = cache
            .pages
            .get(&id)
            .ok_or_else(|| ToolError::InvalidArguments(format!("page {id} was not opened with browser.open")))?;
        let needle = pattern.to_lowercase();
        let hits: Vec<String> = p
            .lines
            .iter()
            .enumerate()
            .skip(start)
            .filter(|(_, l)| l.to_lowercase().contains(&needle))
            .map(|(n, l)| format!("L{n}: {l}"))
            .collect();
        Ok(if hits.is_empty() {
            format!("No matches for \"{pattern}\" in page {id} from line {start}.")
        } else {
            format!("Matches for \"{pattern}\" in page {id}:\n{}", hits.join("\n"))
        })
    }

    fn lookup(&self, args: &Value) -> Result<Value, ToolError> {
        let id = match args.get("page_id") {
            Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
            Some(v) => v.as_u64(),
            None => None,
        }
        .ok_or_else(|| ToolError::InvalidArguments("`page_id` (search result page ID) is required".into()))?;
        let cache = self.inner.cache.lock().unwrap();
        let hits = cache
            .searches
            .get(&id)
            .ok_or_else(|| ToolError::InvalidArguments(format!("no browser.search session {id}")))?;
        let wanted: Vec<usize> = match args.get("outlink_indices").and_then(Value::as_array) {
            Some(a) if !a.is_empty() => a.iter().filter_map(Value::as_u64).map(|i| i as usize).collect(),
            _ => (0..hits.len()).collect(),
        };
        if wanted.len() > LOOKUP_MAX {
            return Err(ToolError::InvalidArguments(format!("at most {LOOKUP_MAX} results per call")));
        }
        Ok(Value::Array(
            wanted
                .into_iter()
                .map(|i| match hits.get(i) {
                    Some(h) => json!({"index": i, "url": h.url, "title": h.title}),
                    None => json!({"index": i, "error": "no such result"}),
                })
                .collect(),
        ))
    }
}

enum Target {
    Fetch(String),
    Cached(u64),
}

/// Markdown страницы → строки. Ссылки `[текст](адрес)` → `【idx†текст】`,
/// картинки `![alt](src)` → `[Image N]` и сноска `Images:` в конце — как в
/// описании `browser.open`.
fn render(url: &str, title: &str, md: &str) -> Page {
    let link = regex::Regex::new(r"(!?)\[([^\]]*)\]\(([^)\s]+)(?:\s+[^)]*)?\)").expect("regex");
    let base = reqwest::Url::parse(url).ok();
    let mut outlinks: Vec<String> = Vec::new();
    let mut images: Vec<String> = Vec::new();
    let absolute = |href: &str| base.as_ref().and_then(|b| b.join(href).ok()).map(|u| u.to_string()).unwrap_or_else(|| href.to_string());
    let mut lines: Vec<String> = md
        .lines()
        .map(|l| {
            link.replace_all(l, |c: &regex::Captures| {
                let href = absolute(&c[3]);
                if &c[1] == "!" {
                    images.push(href);
                    format!("[Image {}]", images.len())
                } else {
                    outlinks.push(href);
                    format!("【{}†{}】", outlinks.len() - 1, &c[2])
                }
            })
            .to_string()
        })
        .collect();
    if !images.is_empty() {
        lines.push(String::new());
        lines.push("Images:".into());
        for (i, src) in images.iter().enumerate() {
            lines.push(format!("[Image {}]: {src}", i + 1));
        }
    }
    Page { url: url.to_string(), title: title.to_string(), lines, outlinks }
}

#[derive(Clone, Copy)]
enum Op {
    Search,
    Open,
    Find,
    Lookup,
}

struct WebTool {
    op: Op,
    web: WebTools,
}

fn p(t: &str, d: &str) -> Value {
    json!({"type": t, "description": personal(d)})
}

impl Tool for WebTool {
    fn spec(&self) -> ToolSpec {
        let (name, d, props, req) = match self.op {
            Op::Search => (
                "browser.search",
                SEARCH_DESCRIPTION,
                json!({
                    "primary_query": {
                        "type": "object",
                        "description": SEARCH_PRIMARY,
                        "properties": {
                            "query": p("string", SEARCH_QUERY),
                            "language_code": {"type": ["string", "null"], "description": SEARCH_LANGUAGE},
                        },
                        "required": ["query", "language_code"],
                    },
                    "verticals": {"type": "array", "maxItems": 1, "items": {"type": "string", "enum": VERTICALS}, "description": personal(SEARCH_VERTICALS)},
                }),
                json!(["primary_query"]),
            ),
            Op::Open => (
                "browser.open",
                OPEN_DESCRIPTION,
                json!({
                    "url_id": {"type": ["string", "integer"], "description": OPEN_URL_ID},
                    "outlink_idx": p("integer", OPEN_OUTLINK),
                    "line_start": p("integer", OPEN_LINE),
                }),
                json!(["url_id"]),
            ),
            Op::Find => (
                "browser.find",
                FIND_DESCRIPTION,
                json!({
                    "pattern": p("string", FIND_PATTERN),
                    "url_id": p("integer", FIND_PAGE),
                    "line_start": p("integer", FIND_LINE),
                }),
                json!(["pattern", "url_id"]),
            ),
            Op::Lookup => (
                "browser.lookup_citation_url",
                LOOKUP_DESCRIPTION,
                json!({
                    "page_id": p("integer", LOOKUP_PAGE),
                    "outlink_indices": {"type": "array", "items": {"type": "integer"}, "maxItems": LOOKUP_MAX, "description": LOOKUP_INDICES},
                }),
                json!(["page_id"]),
            ),
        };
        ToolSpec { name: name.into(), description: personal(d), input_schema: json!({"type": "object", "properties": props, "required": req}) }
    }

    fn call(&self, args: Value) -> ToolFuture<'_> {
        Box::pin(async move {
            let text = match self.op {
                Op::Search => self.web.search(&args).await?,
                Op::Open => self.web.open(&args).await?,
                Op::Find => self.web.find(&args)?,
                Op::Lookup => self.web.lookup(&args)?.to_string(),
            };
            Ok(vec![ToolResultPart::Text { text }])
        })
    }
}
