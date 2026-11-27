//! Поставщик `openai_compat` — всё, что говорит «как OpenAI»: OpenAI и
//! локальные серверы (Ollama, LM Studio, llama.cpp, vLLM) по `base_url`.
//! Форма — muse `kimi_chat_completions/*` (`source-files.txt:5301-5304`).
//! Эмбеддинги — `/v1/embeddings` (у muse служба `embeddings`, 384 измерения,
//! cosine; опись, раздел 3).

mod convert;
mod stream;

use std::time::Duration;

use serde_json::{json, Value};

use crate::http::{self, config, decode_err, Timeouts};
use crate::registry::{OpenaiCompatFacts, ProviderKind, Resolved};
use crate::types::{ProviderError, ProviderErrorKind, Request, Response, ToolSpec};
use crate::{OnText, Provider};

pub use convert::{request_body, TOOL_ERROR_PREFIX};

pub struct OpenaiCompatProvider {
    model: String,
    facts: OpenaiCompatFacts,
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
    timeouts: Timeouts,
}

/// Признак ответа сервера «модель не умеет инструменты» (так пишут Ollama
/// и llama.cpp). Такой ответ — ошибка настройки: модель не годится агенту.
fn no_tools(e: &ProviderError) -> bool {
    matches!(e.kind, ProviderErrorKind::Http { status: 400 })
        && e.message.to_lowercase().contains("tool")
        && (e.message.contains("not support") || e.message.contains("unsupported"))
}

impl OpenaiCompatProvider {
    pub fn new(
        model: impl Into<String>,
        facts: OpenaiCompatFacts,
        base_url: impl Into<String>,
        api_key: Option<String>,
    ) -> Result<Self, ProviderError> {
        convert::check_supported(&facts)?;
        Ok(OpenaiCompatProvider {
            model: model.into(),
            facts,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            client: http::client()?,
            timeouts: Timeouts::default(),
        })
    }

    /// Другие сроки ожидания (для тестов и настройки).
    pub fn with_timeouts(mut self, first_chunk: Duration, idle: Duration) -> Self {
        self.timeouts = Timeouts { first_chunk, idle };
        self
    }

    /// Поставщик для выбранной модели. Ключ нужен, только если в реестре
    /// записано имя переменной (локальному серверу ключ не нужен).
    pub fn from_resolved(r: &Resolved<'_>) -> Result<Self, ProviderError> {
        if r.option.provider != ProviderKind::OpenaiCompat {
            return Err(config(format!("модель `{}` не openai_compat", r.option.id)));
        }
        let base_url = r
            .option
            .base_url
            .clone()
            .ok_or_else(|| config(format!("у модели `{}` нет base_url", r.option.id)))?;
        let api_key = match r.option.api_key_env.as_deref() {
            None => None,
            Some(var) => Some(
                std::env::var(var)
                    .map_err(|_| config(format!("переменная `{var}` с ключом не задана")))?,
            ),
        };
        Self::new(
            r.option.model.clone(),
            r.weights.openai_compat.clone().unwrap_or_default(),
            base_url,
            api_key,
        )
    }

    fn post(&self, path: &str, body: &Value) -> reqwest::RequestBuilder {
        let mut call = self
            .client
            .post(format!("{}{path}", self.base_url))
            .header("content-type", "application/json")
            .json(body);
        if let Some(k) = &self.api_key {
            call = call.bearer_auth(k);
        }
        call
    }

    /// Проверка при запуске: понимает ли сервер инструменты. Нет — ошибка
    /// настройки (агент без инструментов не работает), подделки нет.
    pub async fn check_tools(&self) -> Result<(), ProviderError> {
        let req = Request {
            messages: vec![crate::types::Message {
                role: crate::types::Role::User,
                content: vec![crate::types::Part::Text { text: "ok".into() }],
            }],
            tools: vec![ToolSpec {
                name: "noop".into(),
                description: "Проверка поддержки инструментов.".into(),
                input_schema: json!({"type": "object", "properties": {}}),
            }],
            settings: crate::types::Settings {
                max_output_tokens: Some(1),
                ..Default::default()
            },
            ..Default::default()
        };
        match self.complete(&req).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind == ProviderErrorKind::Config => Err(e),
            // Обрезанный ответ (max_tokens 1) — не повод считать сервер негодным.
            Err(e) if e.kind == ProviderErrorKind::Decode => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Эмбеддинги для задачи `Embeddings`: по вектору на каждую строку, по порядку.
    pub async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        let body = json!({"model": self.model, "input": inputs});
        let resp = http::send(self.post("/embeddings", &body), self.timeouts).await?;
        let v: Value = tokio::time::timeout(self.timeouts.idle, resp.json())
            .await
            .map_err(|_| http::stream_err("тишина при чтении эмбеддингов"))?
            .map_err(|e| decode_err(format!("эмбеддинги: {}", e.without_url())))?;
        let data = v["data"]
            .as_array()
            .ok_or_else(|| decode_err("в ответе эмбеддингов нет data"))?;
        let mut out: Vec<Option<Vec<f32>>> = vec![None; inputs.len()];
        for (pos, d) in data.iter().enumerate() {
            let i = d["index"].as_u64().map(|i| i as usize).unwrap_or(pos);
            let vec: Vec<f32> = d["embedding"]
                .as_array()
                .ok_or_else(|| decode_err("эмбеддинг не массив"))?
                .iter()
                .map(|x| x.as_f64().map(|f| f as f32).ok_or_else(|| decode_err("эмбеддинг не число")))
                .collect::<Result<_, _>>()?;
            let slot = out
                .get_mut(i)
                .ok_or_else(|| decode_err(format!("эмбеддинг с индексом {i} вне входа")))?;
            *slot = Some(vec);
        }
        out.into_iter()
            .enumerate()
            .map(|(i, v)| v.ok_or_else(|| decode_err(format!("нет эмбеддинга для строки {i}"))))
            .collect()
    }
}

impl Provider for OpenaiCompatProvider {
    async fn stream(&self, req: &Request, on_text: &mut OnText<'_>) -> Result<Response, ProviderError> {
        let body = request_body(&self.model, req, &self.facts)?;
        let resp = match http::send(
            self.post("/chat/completions", &body).header("accept", "text/event-stream"),
            self.timeouts,
        )
        .await
        {
            Err(e) if no_tools(&e) => {
                return Err(config(format!(
                    "сервер не поддерживает инструменты у модели `{}`: {}",
                    self.model, e.message
                )))
            }
            other => other?,
        };
        let mut state = stream::State::new(self.facts.reasoning);
        http::read_events(resp, self.timeouts, |data| state.event(data, on_text)).await?;
        state.finish()
    }
}
