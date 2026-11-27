//! Поставщик выбранной модели — тот, что записан в реестре: у muse выбор
//! поставщика тоже по записи реестра (`model/factory.rs`,
//! `source-files.txt`, «unsupported model provider `…` for `…`»,
//! `strings-all.txt`). Одно место, где запись реестра становится
//! поставщиком.

use std::time::Duration;

use crate::anthropic::AnthropicProvider;
use crate::http::Timeouts;
use crate::openai_compat::OpenaiCompatProvider;
use crate::registry::{ProviderKind, Resolved};
use crate::types::{ProviderError, Request, Response};
use crate::{OnText, Provider};

pub enum AnyProvider {
    Anthropic(AnthropicProvider),
    OpenaiCompat(OpenaiCompatProvider),
}

impl AnyProvider {
    pub fn from_resolved(r: &Resolved<'_>) -> Result<Self, ProviderError> {
        Ok(match r.option.provider {
            ProviderKind::Anthropic => AnyProvider::Anthropic(AnthropicProvider::from_resolved(r)?),
            ProviderKind::OpenaiCompat => AnyProvider::OpenaiCompat(OpenaiCompatProvider::from_resolved(r)?),
        })
    }

    /// Свой срок ожидания первого куска потока (у расписаний muse — свой,
    /// `JARVIS_CRON_MODEL_STREAM_FIRST_CHUNK_TIMEOUT_MS`); тишина между
    /// кусками — прежняя.
    pub fn with_first_chunk(self, first_chunk: Duration) -> Self {
        match self {
            AnyProvider::Anthropic(p) => {
                let idle = p.timeouts().idle;
                AnyProvider::Anthropic(p.with_timeouts(first_chunk, idle))
            }
            AnyProvider::OpenaiCompat(p) => {
                let idle = p.timeouts().idle;
                AnyProvider::OpenaiCompat(p.with_timeouts(first_chunk, idle))
            }
        }
    }

    pub fn timeouts(&self) -> Timeouts {
        match self {
            AnyProvider::Anthropic(p) => p.timeouts(),
            AnyProvider::OpenaiCompat(p) => p.timeouts(),
        }
    }
}

impl Provider for AnyProvider {
    async fn stream(&self, req: &Request, on_text: &mut OnText<'_>) -> Result<Response, ProviderError> {
        match self {
            AnyProvider::Anthropic(p) => p.stream(req, on_text).await,
            AnyProvider::OpenaiCompat(p) => p.stream(req, on_text).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{CRON_FIRST_CHUNK_TIMEOUT, FIRST_CHUNK_TIMEOUT, STREAM_IDLE_TIMEOUT};
    use crate::OpenaiCompatFacts;

    #[test]
    fn cron_first_chunk_is_thirty_minutes() {
        let p = AnyProvider::OpenaiCompat(OpenaiCompatProvider::new("m", OpenaiCompatFacts::default(), "http://127.0.0.1:9/v1", None).unwrap());
        assert_eq!(p.timeouts().first_chunk, FIRST_CHUNK_TIMEOUT);
        let p = p.with_first_chunk(CRON_FIRST_CHUNK_TIMEOUT);
        assert_eq!(p.timeouts().first_chunk, Duration::from_secs(1800));
        assert_eq!(p.timeouts().idle, STREAM_IDLE_TIMEOUT);
    }
}
