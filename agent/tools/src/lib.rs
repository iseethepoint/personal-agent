//! `agent-tools` — инструменты muse. Каждый подключается записью в реестре
//! инструментов цикла (`agent_loop::Tools`).

pub mod exec;
pub mod files;
pub mod paths;
pub mod secrets;
pub mod skills;
pub mod todo;
pub mod trash;
pub mod web;

pub use exec::ExecTools;
pub use files::FileTools;
pub use paths::Home;
pub use secrets::secret_envs;
pub use skills::{add_search, Catalog};
pub use todo::Todo;
pub use trash::Trash;
pub use web::{Blocklist, WebConfig, WebTools};
