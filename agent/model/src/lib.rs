//! `agent-model` — единственное место, где агент говорит с моделью.
//! Реестр моделей и выбор модели по задаче — по форме muse `hatch-inference`;
//! один вход и один выход для всех поставщиков.

pub mod anthropic;
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

/// Один вход → один выход. Каждый поставщик переводит их в свой протокол.
pub trait Provider {
    fn complete(&self, req: &Request) -> Result<Response, ProviderError>;
}
