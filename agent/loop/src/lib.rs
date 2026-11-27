//! `agent-loop` — цикл хода: модель → вызовы инструментов → результаты →
//! модель. С моделью говорит только через `agent-model`.

pub mod compaction;
pub mod monitor;
pub mod prompt;
pub mod quarantine;
pub mod roles;
pub mod segmented;
pub mod spill;
pub mod subagents;
pub mod tool;
pub mod tool_search;
pub mod turn;

pub use compaction::{personal, summarization_prompt, Compacted, Compactor, Plan, SUMMARY_HEADER};
pub use prompt::{close_interrupted, o200k_count, Conversation, Prefix, HOME_FILES, INTERRUPTED_TOOL_CALL};
pub use spill::SPILL_THRESHOLD_BYTES;
pub use subagents::{first_message, inherited_history, spawning_prefix, MAX_SPAWN_DEPTH, AfterDone, Checkpoint, ChildRecord, Finish, Inbox, Kind, KindRecord, Mailbox, SavedSubagent, Status, SubagentStore, SubagentTool, Subagents, Transcript, WorkerStore, ActiveOwner, CONTROL_TIMEOUT, WORK_NOT_RESUMABLE};
pub use tool::{Tool, ToolError, ToolFuture, Tools};
pub use tool_search::{Loaded, Namespace, LOAD_TOOL};
pub use turn::{
    forced_tool_choice, Record, StepCall, StepHook, Turn, TurnError, MODEL_RETRIES, OUTPUT_TOKEN_CAP_NOTICE, SUBMIT_RESULT, THINKING_ONLY_NOTICE, TOOL_TIMEOUT,
};
