//! Имена переменных окружения с ключами — одно место для всего агента:
//! ключи моделей (`api_key_env` реестра) и ключ поиска (`api_key_env` из
//! `config/web.toml`). Их значения скрываются в выводе команд
//! (`ExecTools`, форма `redaction.rs` muse).

use agent_model::Registry;

use crate::web::WebConfig;

pub fn secret_envs(registry: &Registry, web: &WebConfig) -> Vec<String> {
    let mut names = registry.api_key_envs();
    names.push(web.search.api_key_env.clone());
    names.sort();
    names.dedup();
    names
}
