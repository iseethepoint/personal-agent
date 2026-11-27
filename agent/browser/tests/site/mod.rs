//! Поддельный сайт на 127.0.0.1: форма, ссылка, ссылка в новую вкладку.

#![allow(dead_code)]

use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const INDEX: &str = r#"<!doctype html><html><head><title>Fake shop</title></head><body>
<h1>Order a mug</h1>
<form action="/done" method="get">
<label for="name">Your name</label> <input id="name" name="name">
<label for="color">Color</label> <select id="color" name="color"><option value="red">Red</option><option value="blue">Blue</option></select>
<label><input type="checkbox" name="agree"> I agree to the terms</label>
<button type="submit">Place order</button>
</form>
<p><a href="/details">Mug details</a></p>
<p><a href="/help" target="_blank">Open help</a></p>
</body></html>"#;

fn page(path: &str) -> (String, String) {
    let (p, q) = path.split_once('?').unwrap_or((path, ""));
    match p {
        "/" => ("200 OK".into(), INDEX.into()),
        "/details" => ("200 OK".into(), "<!doctype html><title>Mug details</title><h1>Mug details</h1><p>Ceramic mug, 350 ml, price 12 EUR.</p>".into()),
        "/help" => ("200 OK".into(), "<!doctype html><title>Help</title><h1>Help</h1><p>Orders ship in 2 days.</p>".into()),
        "/done" => {
            let mut name = String::new();
            let mut color = String::new();
            let mut agree = "no";
            for kv in q.split('&') {
                let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                let v = v.replace('+', " ");
                match k {
                    "name" => name = v,
                    "color" => color = v,
                    "agree" => agree = "yes",
                    _ => {}
                }
            }
            ("200 OK".into(), format!("<!doctype html><title>Order placed</title><h1>Order placed</h1><p>Thanks {name}: {color} mug, terms {agree}. Order number A-1042.</p>"))
        }
        _ => ("404 Not Found".into(), "<!doctype html><title>Not found</title><h1>Not found</h1>".into()),
    }
}

/// Запустить сайт; вернуть его адрес.
pub async fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = l.accept().await else { break };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = page(&path);
                let resp = format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

/// Браузер для тестов; нет — тест пропускается с причиной.
pub fn chromium() -> Option<PathBuf> {
    let b = agent_browser::chromium::find();
    if b.is_none() {
        eprintln!("SKIP: Chromium не найден (HAMLET_CHROMIUM или известные места установки)");
    }
    b
}

/// Временная папка профиля; удаляется и при падении теста.
pub struct Profile(pub PathBuf);

impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn profile(name: &str) -> Profile {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    Profile(std::env::temp_dir().join(format!("agent-browser-{name}-{}-{nanos}", std::process::id())))
}
