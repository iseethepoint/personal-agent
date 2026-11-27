//! Общее для поставщиков: HTTP-клиент, отправка со сроками muse, нарезка
//! потока SSE на события, ошибка HTTP. Async на tokio и reqwest, как у muse
//! (`crates.txt:10,94`).

use std::time::Duration;

use futures_util::StreamExt;

use crate::types::{ProviderError, ProviderErrorKind};

/// Ожидание ответа сервера до начала потока: 600 с —
/// `JARVIS_MODEL_STREAM_FIRST_CHUNK_TIMEOUT_MS` muse (ядро, `0x26a6bfa0`).
pub const FIRST_CHUNK_TIMEOUT: Duration = Duration::from_millis(600_000);
/// То же у работника расписаний: 1 800 000 мс (30 мин) —
/// `JARVIS_CRON_MODEL_STREAM_FIRST_CHUNK_TIMEOUT_MS` muse (ядро, `0x26a6c020`).
pub const CRON_FIRST_CHUNK_TIMEOUT: Duration = Duration::from_millis(1_800_000);
/// Тишина между кусками потока: 90 с —
/// `JARVIS_MODEL_STREAM_CHUNK_IDLE_TIMEOUT_MS` muse (ядро, `0x26a6bf80`).
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_millis(90_000);

/// Сроки ожидания одного вызова.
#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    pub first_chunk: Duration,
    pub idle: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            first_chunk: FIRST_CHUNK_TIMEOUT,
            idle: STREAM_IDLE_TIMEOUT,
        }
    }
}

pub(crate) fn config(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Config, msg)
}

pub(crate) fn stream_err(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Stream, msg)
}

pub(crate) fn decode_err(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Decode, msg)
}

pub(crate) fn client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| config(format!("HTTP-клиент: {e}")))
}

/// Текст ошибки из тела. Формы: `{"error": {"type", "message"}}`
/// (Anthropic, OpenAI) и `{"error": "текст"}` (Ollama).
pub(crate) fn error_text(body: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(v) => match &v["error"] {
            serde_json::Value::String(s) => s.clone(),
            e if e.is_object() => {
                let kind = e["type"].as_str().or(e["code"].as_str()).unwrap_or("");
                format!("{kind}: {}", e["message"].as_str().unwrap_or(""))
            }
            _ => body.to_string(),
        },
        Err(_) => body.to_string(),
    }
}

async fn http_error(resp: reqwest::Response) -> ProviderError {
    let status = resp.status().as_u16();
    let retry_after_ms = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|s| (s * 1000.0) as u64);
    let body = resp.text().await.unwrap_or_default();
    ProviderError::http(status, error_text(&body), retry_after_ms)
}

/// Отправка с ожиданием до начала ответа; код ошибки — `ProviderError`.
pub(crate) async fn send(
    call: reqwest::RequestBuilder,
    t: Timeouts,
) -> Result<reqwest::Response, ProviderError> {
    let resp = tokio::time::timeout(t.first_chunk, call.send())
        .await
        .map_err(|_| stream_err("сервер не ответил до начала потока"))?
        .map_err(|e| ProviderError::new(ProviderErrorKind::Transport, e.without_url().to_string()))?;
    if !resp.status().is_success() {
        return Err(http_error(resp).await);
    }
    Ok(resp)
}

/// Нарезка потока на события SSE: строки `data:` до пустой строки.
#[derive(Default)]
struct Framer {
    data: String,
    buf: Vec<u8>,
}

impl Framer {
    fn line(
        &mut self,
        line: &str,
        on_event: &mut impl FnMut(&str) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if !self.data.is_empty() {
                let data = std::mem::take(&mut self.data);
                on_event(&data)?;
            }
            return Ok(());
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // Строки `event:` и комментарии не нужны: тип есть в самом JSON.
        Ok(())
    }

    fn feed(
        &mut self,
        chunk: &[u8],
        on_event: &mut impl FnMut(&str) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError> {
        self.buf.extend_from_slice(chunk);
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = std::str::from_utf8(&raw[..raw.len() - 1])
                .map_err(|e| decode_err(format!("SSE не UTF-8: {e}")))?
                .to_string();
            self.line(&line, on_event)?;
        }
        Ok(())
    }

    fn finish(
        mut self,
        on_event: &mut impl FnMut(&str) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError> {
        if !self.buf.is_empty() {
            let rest = String::from_utf8_lossy(&std::mem::take(&mut self.buf)).to_string();
            self.line(&rest, on_event)?;
        }
        self.line("", on_event)
    }
}

/// Читает поток ответа до конца; каждое событие — в `on_event`.
/// Тишина дольше `t.idle` и обрыв чтения — ошибка потока, её можно повторить.
pub(crate) async fn read_events(
    resp: reqwest::Response,
    t: Timeouts,
    mut on_event: impl FnMut(&str) -> Result<(), ProviderError>,
) -> Result<(), ProviderError> {
    let mut stream = resp.bytes_stream();
    let mut framer = Framer::default();
    loop {
        match tokio::time::timeout(t.idle, stream.next()).await {
            Err(_) => return Err(stream_err("тишина в потоке дольше предела")),
            Ok(None) => break,
            Ok(Some(Err(e))) => {
                return Err(stream_err(format!("чтение потока: {}", e.without_url())))
            }
            Ok(Some(Ok(chunk))) => framer.feed(&chunk, &mut on_event)?,
        }
    }
    framer.finish(&mut on_event)
}
