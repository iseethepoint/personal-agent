//! `agent-model` — единственное место, где агент говорит с моделью.
//! Реестр моделей и выбор модели по задаче — по форме muse `hatch-inference`;
//! один вход и один выход для всех поставщиков.

pub mod anthropic;
pub mod http;
pub mod openai_compat;
pub mod registry;
pub mod types;

pub use registry::{
    AnthropicFacts, Defaults, ImageFormat, ImageResolution, ModelOption, OpenaiCompatFacts,
    OutputEffort, ProviderKind, Purpose, PurposeOverride, Registry, RegistryError, Resolved,
    Setup, Weights, Wire,
};
pub use types::{
    Effort, Image, Message, Part, ProviderError, ProviderErrorKind, Request, Response, Role,
    Settings, StopReason, ToolChoice, ToolResultPart, ToolSpec, Usage,
};

/// Получатель видимого текста ответа: куски приходят по мере потока.
pub type OnText<'a> = dyn FnMut(&str) + Send + 'a;

/// Один вход → один выход. Каждый поставщик переводит их в свой протокол.
/// Async и `Send`: цикл хода исполняет вызовы параллельно, как muse
/// (`tool_dispatch_batch`, `source-files.txt:1357`).
pub trait Provider: Sync {
    /// Ответ модели; куски видимого текста по ходу потока отдаются в
    /// `on_text`. По ним цикл знает, видел ли владелец что-то: повтор у muse
    /// — только «before visible output» (`strings-all.txt:54780`).
    fn stream(
        &self,
        req: &Request,
        on_text: &mut OnText<'_>,
    ) -> impl std::future::Future<Output = Result<Response, ProviderError>> + Send;

    /// То же без получателя текста.
    fn complete(
        &self,
        req: &Request,
    ) -> impl std::future::Future<Output = Result<Response, ProviderError>> + Send {
        async move { self.stream(req, &mut |_: &str| {}).await }
    }
}
