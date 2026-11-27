//! Поддельный Firecrawl (копия из `agent/tools/tests/web.rs`).
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct Seen {
    path: String,
    auth: String,
    body: Value,
}

/// Поддельный Firecrawl: ответ по пути и по `url` в теле scrape.
pub struct Firecrawl {
    pub base: String,
    pub seen: Arc<Mutex<Vec<Seen>>>,
}

pub fn firecrawl(search: (u16, Value), pages: HashMap<String, (u16, Value)>) -> Firecrawl {
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

