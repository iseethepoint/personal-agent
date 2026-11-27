//! Поставщик выбранной модели — тот, что записан в реестре: у muse выбор
//! поставщика тоже по записи реестра (`model/factory.rs`,
//! `source-files.txt`, «unsupported model provider `…` for `…`»,
//! `strings-all.txt`). Одно место, где запись реестра становится
//! поставщиком.

use crate::anthropic::AnthropicProvider;
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
}

impl Provider for AnyProvider {
    async fn stream(&self, req: &Request, on_text: &mut OnText<'_>) -> Result<Response, ProviderError> {
        match self {
            AnyProvider::Anthropic(p) => p.stream(req, on_text).await,
            AnyProvider::OpenaiCompat(p) => p.stream(req, on_text).await,
        }
    }
}
