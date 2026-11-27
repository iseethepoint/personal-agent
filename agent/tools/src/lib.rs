//! `agent-tools` — инструменты muse. Каждый подключается записью в реестре
//! инструментов цикла (`agent_loop::Tools`).

pub mod files;
pub mod paths;

pub use files::FileTools;
pub use paths::Roots;
