//! `agent-loop` — цикл хода: модель → вызовы инструментов → результаты →
//! модель. С моделью говорит только через `agent-model`.

pub mod spill;
pub mod tool;
pub mod turn;

pub use spill::SPILL_THRESHOLD_BYTES;
pub use tool::{Tool, ToolError, ToolFuture, Tools};
pub use turn::{
    forced_tool_choice, Turn, TurnError, MODEL_RETRIES, OUTPUT_TOKEN_CAP_NOTICE, SUBMIT_RESULT, TOOL_TIMEOUT,
};
