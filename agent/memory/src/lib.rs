//! `agent-memory` — память агента на Postgres, по форме muse `hatch-memory`
//! (`source-files.txt:5463-5476`): записи из файлов памяти, утверждения,
//! поиск по словам и по векторам, забывание.

pub mod files;
pub mod forget;
pub mod store;
pub mod tools;

use std::future::Future;
use std::pin::Pin;

use agent_model::openai_compat::OpenaiCompatProvider;

pub use files::{is_memory_path, reindex, Reindex};
pub use forget::{apply_pending, ForgetTools, Forgotten};
pub use store::{Claim, Entry, NewClaim, NewEntry, Store};
pub use tools::MemoryTools;

pub type EmbedFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<Vec<f32>>, String>> + Send + 'a>>;

/// Модель векторов — задача `Embeddings` реестра. Без неё поиск только по
/// словам; модель не ответила — тоже (форма muse «embedding task failed;
/// lexical-only» у поиска ленты, `strings-all.txt:55090`).
pub trait Embedder: Send + Sync {
    /// Имя модели в `memory.embedding_models`.
    fn model_name(&self) -> String;
    fn embed<'a>(&'a self, inputs: &'a [String]) -> EmbedFuture<'a>;
}

/// Модель векторов через `openai_compat` (`/v1/embeddings`).
pub struct ProviderEmbedder {
    pub name: String,
    pub provider: OpenaiCompatProvider,
}

impl Embedder for ProviderEmbedder {
    fn model_name(&self) -> String {
        self.name.clone()
    }

    fn embed<'a>(&'a self, inputs: &'a [String]) -> EmbedFuture<'a> {
        Box::pin(async move { self.provider.embed(inputs).await.map_err(|e| e.to_string()) })
    }
}
