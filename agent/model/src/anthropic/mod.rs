//! Поставщик `anthropic` — свой HTTP-клиент на Messages API, по форме muse
//! `hatch-inference/src/model/anthropic/*` (`source-files.txt:5263-5275`):
//! перевод подсказки (`convert`), поток SSE (`sse`), запрос и ошибка HTTP (здесь).
//! Async на tokio и reqwest, как у muse (`crates.txt:10,94`).

mod convert;
mod sse;

use std::time::Duration;

use crate::http::{self, config, Timeouts};
use crate::registry::{AnthropicFacts, ProviderKind, Resolved};
use crate::types::{ProviderError, Request, Response};
use crate::{OnText, Provider};

pub use convert::{request_body, MAX_REQUEST_BYTES};

/// Версия API, как у muse (`strings-all.txt:237329`).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

pub struct AnthropicProvider {
    model: String,
    facts: AnthropicFacts,
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    timeouts: Timeouts,
}

impl AnthropicProvider {
    pub fn new(
        model: impl Into<String>,
        facts: AnthropicFacts,
        base_url: Option<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        convert::check_supported(&facts)?;
        Ok(AnthropicProvider {
            model: model.into(),
            facts,
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            api_key: api_key.into(),
            client: http::client()?,
            timeouts: Timeouts::default(),
        })
    }

    /// Другие сроки ожидания (для тестов и настройки).
    pub fn with_timeouts(mut self, first_chunk: Duration, idle: Duration) -> Self {
        self.timeouts = Timeouts { first_chunk, idle };
        self
    }

    pub fn timeouts(&self) -> Timeouts {
        self.timeouts
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

impl Provider for AnthropicProvider {
    async fn stream(&self, req: &Request, on_text: &mut OnText<'_>) -> Result<Response, ProviderError> {
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
        let resp = http::send(call, self.timeouts).await?;
        let mut state = sse::State::default();
        http::read_events(resp, self.timeouts, |data| state.event(data, on_text)).await?;
        state.finish()
    }
}
