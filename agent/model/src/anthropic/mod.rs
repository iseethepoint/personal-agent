//! Поставщик `anthropic` — свой HTTP-клиент на Messages API, по форме muse
//! `hatch-inference/src/model/anthropic/*` (`source-files.txt:5263-5275`):
//! перевод подсказки (`convert`), поток SSE (`sse`), запрос и ошибка HTTP (здесь).

mod convert;
mod sse;

use std::io::{BufReader, Read};
use std::time::Duration;

use crate::registry::{AnthropicFacts, ProviderKind, Resolved};
use crate::types::{ProviderError, ProviderErrorKind, Request, Response};
use crate::Provider;

pub use convert::{request_body, MAX_REQUEST_BYTES};
pub use sse::read_stream;

/// Версия API, как у muse (`strings-all.txt:237329`).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Тишина в потоке, после которой чтение обрывается: 90 с —
/// `JARVIS_MODEL_STREAM_CHUNK_IDLE_TIMEOUT_MS` muse (ядро, `0x26a6bf80`).
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_millis(90_000);

pub struct AnthropicProvider {
    model: String,
    facts: AnthropicFacts,
    base_url: String,
    api_key: String,
    agent: ureq::Agent,
}

fn config(msg: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Config, msg)
}

impl AnthropicProvider {
    pub fn new(
        model: impl Into<String>,
        facts: AnthropicFacts,
        base_url: Option<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        convert::check_supported(&facts)?;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .timeout_read(STREAM_IDLE_TIMEOUT)
            .build();
        Ok(AnthropicProvider {
            model: model.into(),
            facts,
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            api_key: api_key.into(),
            agent,
        })
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
fn http_error(status: u16, resp: ureq::Response) -> ProviderError {
    let retry_after_ms = resp
        .header("retry-after")
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|s| (s * 1000.0) as u64);
    let mut body = String::new();
    let _ = resp.into_reader().take(64 * 1024).read_to_string(&mut body);
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

impl Provider for AnthropicProvider {
    fn complete(&self, req: &Request) -> Result<Response, ProviderError> {
        let body = request_body(&self.model, req, &self.facts)?;
        let mut call = self
            .agent
            .post(&format!("{}/v1/messages", self.base_url))
            .set("content-type", "application/json")
            .set("accept", "text/event-stream")
            .set("anthropic-version", ANTHROPIC_VERSION)
            .set("x-api-key", &self.api_key);
        if let Some(beta) = &self.facts.beta {
            call = call.set("anthropic-beta", beta);
        }
        match call.send_json(body) {
            Ok(resp) => read_stream(BufReader::new(resp.into_reader())),
            Err(ureq::Error::Status(status, resp)) => Err(http_error(status, resp)),
            Err(ureq::Error::Transport(t)) => Err(ProviderError::new(
                ProviderErrorKind::Transport,
                t.to_string(),
            )),
        }
    }
}
