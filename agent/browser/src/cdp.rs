//! Клиент Chrome DevTools Protocol поверх одного WebSocket браузера —
//! у muse свой клиент на `tokio-tungstenite` (`core-v2-analysis/crates.txt`),
//! готовой библиотеки CDP в ядре нет. Вкладки — сеансы `flatten`
//! (`Target.attachToTarget`), все по одному сокету с `sessionId`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as Ws;

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// Событие CDP.
#[derive(Debug, Clone)]
pub struct Event {
    pub method: String,
    pub params: Value,
    pub session: Option<String>,
}

pub struct Cdp {
    out: mpsc::UnboundedSender<Ws>,
    pending: Pending,
    next: AtomicU64,
    events: broadcast::Sender<Event>,
}

/// Срок ответа на одну команду CDP. Своё.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);

impl Cdp {
    pub async fn connect(url: &str) -> Result<Arc<Cdp>, String> {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.map_err(|e| format!("cdp connect: {e}"))?;
        let (mut sink, mut stream) = ws.split();
        let (out, mut rx) = mpsc::unbounded_channel::<Ws>();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel(1024);
        tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                if sink.send(m).await.is_err() {
                    break;
                }
            }
        });
        let p = pending.clone();
        let ev = events.clone();
        tokio::spawn(async move {
            while let Some(Ok(m)) = stream.next().await {
                let Ws::Text(t) = m else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&t) else { continue };
                if let Some(id) = v.get("id").and_then(Value::as_u64) {
                    if let Some(tx) = p.lock().unwrap().remove(&id) {
                        let r = match v.get("error") {
                            Some(e) => Err(e.get("message").and_then(Value::as_str).unwrap_or("cdp error").to_string()),
                            None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                        };
                        let _ = tx.send(r);
                    }
                } else if let Some(method) = v.get("method").and_then(Value::as_str) {
                    let _ = ev.send(Event {
                        method: method.to_string(),
                        params: v.get("params").cloned().unwrap_or(Value::Null),
                        session: v.get("sessionId").and_then(Value::as_str).map(String::from),
                    });
                }
            }
            // Сокет закрыт — ждущие получают ошибку.
            for (_, tx) in p.lock().unwrap().drain() {
                let _ = tx.send(Err("browser connection closed".into()));
            }
        });
        Ok(Arc::new(Cdp { out, pending, next: AtomicU64::new(1), events }))
    }

    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub async fn call(&self, session: Option<&str>, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.out.send(Ws::Text(msg.to_string().into())).map_err(|_| "browser connection closed".to_string())?;
        match tokio::time::timeout(CALL_TIMEOUT, rx).await {
            Ok(Ok(r)) => r.map_err(|e| format!("{method}: {e}")),
            Ok(Err(_)) => Err("browser connection closed".into()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("{method}: page_unresponsive"))
            }
        }
    }
}
