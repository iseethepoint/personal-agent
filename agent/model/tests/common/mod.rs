//! Поддельный HTTP-сервер для тестов: только 127.0.0.1, один запрос,
//! записанный ответ. Живых вызовов моделей нет.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub fn fixture(rel: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Что сервер получил.
pub struct Captured {
    pub request_line: String,
    pub headers: Vec<(String, String)>,
    pub body: serde_json::Value,
}

impl Captured {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct FakeServer {
    pub base_url: String,
    handle: JoinHandle<Captured>,
}

impl FakeServer {
    /// Ответит на один запрос: код, лишние заголовки, тело.
    pub fn start(status: u16, content_type: &str, extra_headers: &[(&str, &str)], body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let mut head = format!("HTTP/1.1 {status} X\r\ncontent-type: {content_type}\r\nconnection: close\r\n");
        for (k, v) in extra_headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut headers = Vec::new();
            let mut len = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end().to_string();
                if line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    let (k, v) = (k.trim().to_string(), v.trim().to_string());
                    if k.eq_ignore_ascii_case("content-length") {
                        len = v.parse().unwrap();
                    }
                    headers.push((k, v));
                }
            }
            let mut raw = vec![0u8; len];
            reader.read_exact(&mut raw).unwrap();
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(body.as_bytes()).unwrap();
            stream.flush().unwrap();
            Captured {
                request_line: request_line.trim_end().to_string(),
                headers,
                body: serde_json::from_slice(&raw).unwrap_or(serde_json::Value::Null),
            }
        });
        FakeServer { base_url, handle }
    }

    pub fn sse(fixture_name: &str) -> Self {
        Self::start(200, "text/event-stream", &[], fixture(fixture_name))
    }

    pub fn captured(self) -> Captured {
        self.handle.join().expect("сервер")
    }
}
