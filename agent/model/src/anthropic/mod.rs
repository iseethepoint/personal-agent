//! Поставщик `anthropic` — свой HTTP-клиент на Messages API, по форме muse
//! `hatch-inference/src/model/anthropic/*` (`source-files.txt:5263-5275`):
//! перевод подсказки (`convert`), поток SSE (`sse`), запрос и ошибка HTTP (здесь).
//! Async на tokio и reqwest, как у muse (`crates.txt:10,94`).

mod convert;
mod sse;

use std::time::Duration;

use futures_util::StreamExt;

use crate::registry::{AnthropicFacts, ProviderKind, Resolved};
use crate::types::{ProviderError, ProviderErrorKind, Request, Response};
use crate::Provider;

pub use convert::{request_body, MAX_REQUEST_BYTES};
pub use sse::SseParser;

/// Версия API, как у muse (`strings-all.txt:237329`).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Ожидание ответа сервера до начала потока: 600 с —
/// `JARVIS_MODEL_STREAM_FIRST_CHUNK_TIMEOUT_MS` muse (ядро, `0x26a6bfa0`).
pub const FIRST_CHUNK_TIMEOUT: Duration = Duration::from_millis(600_000);
/// Тишина между кусками потока: 90 с —
/// `JARVIS_MODEL_STREAM_CHUNK_IDLE_TIMEOUT_MS` muse (ядро, `0x26a6bf80`).
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_millis(90_000);

pub struct AnthropicProvider {
    model: String,
    facts: AnthropicFacts,
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    first_chunk_timeout: Duration,
    idle_timeout: Duration,
}

fn config(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Config, msg)
}

fn stream_err(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Stream, msg)
}

impl AnthropicProvider {
    pub fn new(
        model: impl Into<String>,
        facts: AnthropicFacts,
        base_url: Option<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        convert::check_supported(&facts)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| config(format!("HTTP-клиент: {e}")))?;
        Ok(AnthropicProvider {
            model: model.into(),
            facts,
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            api_key: api_key.into(),
            client,
            first_chunk_timeout: FIRST_CHUNK_TIMEOUT,
            idle_timeout: STREAM_IDLE_TIMEOUT,
        })
    }

    /// Другие сроки ожидания (для тестов и настройки).
    pub fn with_timeouts(mut self, first_chunk: Duration, idle: Duration) -> Self {
        self.first_chunk_timeout = first_chunk;
        self.idle_timeout = idle;
        self
    }

    /// Поставщик для выбранной модели. Ключ — из переменной окружения,
    /// имя которой записано в реестре.
    pub fn from_resolved(r: &Resolved<'_>) -> Result<Self, ProviderError> {
        if r.option.provider != ProviderKind::Anthropic {
            return Err(config(format!("модель `{}` не anthropic", r.option.id)));
        }
        let var = r
            .option
            .api_key_env
            .as_deref()
            .ok_or_else(|| config(format!("у модели `{}` нет api_key_env", r.option.id)))?;
        let key = std::env::var(var)
            .map_err(|_| config(format!("переменная `{var}` с ключом не задана")))?;
        Self::new(
            r.option.model.clone(),
            r.weights.anthropic.clone().unwrap_or_default(),
            r.option.base_url.clone(),
            key,
        )
    }
}

/// Ошибка HTTP Messages API: тип и текст из тела, `retry-after` в секундах.
async fn http_error(resp: reqwest::Response) -> ProviderError {
    let status = resp.status().as_u16();
    let retry_after_ms = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|s| (s * 1000.0) as u64);
    let body = resp.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .map(|v| {
            format!(
                "{}: {}",
                v["error"]["type"].as_str().unwrap_or(""),
                v["error"]["message"].as_str().unwrap_or("")
            )
        })
        .unwrap_or(body);
    ProviderError::http(status, message, retry_after_ms)
}

fn transport(e: reqwest::Error) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Transport, e.without_url().to_string())
}

impl Provider for AnthropicProvider {
    async fn complete(&self, req: &Request) -> Result<Response, ProviderError> {
        let body = request_body(&self.model, req, &self.facts)?;
        let mut call = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-api-key", &self.api_key)
            .json(&body);
        if let Some(beta) = &self.facts.beta {
            call = call.header("anthropic-beta", beta);
        }
        let resp = tokio::time::timeout(self.first_chunk_timeout, call.send())
            .await
            .map_err(|_| stream_err("сервер не ответил до начала потока"))?
            .map_err(transport)?;
        if !resp.status().is_success() {
            return Err(http_error(resp).await);
        }
        let mut stream = resp.bytes_stream();
        let mut parser = SseParser::default();
        loop {
            match tokio::time::timeout(self.idle_timeout, stream.next()).await {
                Err(_) => return Err(stream_err("тишина в потоке дольше предела")),
                Ok(None) => break,
                Ok(Some(Err(e))) => {
                    return Err(stream_err(format!("чтение потока: {}", e.without_url())))
                }
                Ok(Some(Ok(chunk))) => parser.feed(&chunk)?,
            }
        }
        parser.finish()
    }
}
