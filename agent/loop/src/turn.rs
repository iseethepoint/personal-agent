//! Ход: модель → пачка вызовов инструментов → результаты → модель, пока
//! ответ не придёт без вызовов. Форма — `session/impl_session/message_execution/*`
//! muse (`source-files.txt:1303-1363`): `model_execution.rs`,
//! `tool_dispatch_batch.rs`, `tool_batch_settlement.rs`.

use std::path::PathBuf;
use std::time::Duration;

use agent_model::{
    Message, OnText, Part, ProviderError, ProviderErrorKind, ProviderKind, Request, Resolved,
    Response, Role, Settings, StopReason, ToolChoice, ToolResultPart, ToolSpec,
};
use agent_model::Provider;
use futures_util::future::join_all;
use serde_json::{json, Value};

use crate::prompt::Prefix;
use crate::subagents::{Inbox, Transcript};
use crate::spill;
use crate::tool::{ToolError, Tools};

/// Срок на один инструмент — 180 с (`JARVIS_TOOL_DISPATCH_TIMEOUT_MS`,
/// ядро 0x26a6bf00; опись, таблица 4.2).
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(180);

/// Повторных попыток вызова модели. Числа в ядре не найдено (около
/// `inference_recovery` есть только `retry_budget_exhausted`), поэтому —
/// одна повторная попытка, как велит карточка П3а.
pub const MODEL_RETRIES: u32 = 1;

/// Вставка после упора в предел ответа — «Internal retry notice» muse
/// (`session/output_token_cap_retry_notice.md`, `strings-all.txt:54780`).
pub const OUTPUT_TOKEN_CAP_NOTICE: &str = "Your previous response reached the output limit before it finished, so none of it took effect: no reply was sent and no tool call ran. Do not repeat it the same way. If you were writing out a long value, such as file contents, encoded data or a long ID, do not type it by hand: have a command or script read it from the file or the earlier output and pass it on. Otherwise make the response much shorter, or split the work into smaller steps. Continue the task now.";

/// Вставка после ответа из одного мышления — «thinking-only retry notice»
/// muse (`strings-all.txt:54778`, ядро 0x6b1026), вариант личного muse
/// (`personal-only`: «sent to the user»).
pub const THINKING_ONLY_NOTICE: &str = "Your previous response ended after reasoning only, with no reply and no tool call, so nothing was sent to the user and nothing ran. Text inside reasoning is never executed or shown, including a tool call written there. Continue now: make the tool call you intended as a real tool call, or write your reply.";

/// Инструмент ответа в заданной форме (`output_contract.rs`,
/// `strings-all.txt:235518`).
pub const SUBMIT_RESULT: &str = "submit_result";
const SUBMIT_RESULT_DESCRIPTION: &str = "Submit the completed result. This is the only valid final response: call it exactly once and do not return the result as assistant text.";
const RESULT_DESCRIPTION: &str =
    "The completed result. The runtime validates it against the active output contract.";

#[derive(Debug, thiserror::Error)]
pub enum TurnError {
    /// Модель не ответила, а повторять нельзя.
    #[error("model: {0}")]
    Model(ProviderError),
    /// Сжатие не удалось.
    #[error("compaction: {0}")]
    Compaction(String),
    /// Ответ не в заданной форме.
    #[error("output contract: {0}")]
    OutputContract(String),
}

/// Умеют ли веса принудительный вызов инструмента: у anthropic — флаг
/// `anthropic.forced_tool_choice`; Chat Completions принимает `tool_choice`
/// с именем функции всегда.
pub fn forced_tool_choice(r: &Resolved<'_>) -> bool {
    match r.option.provider {
        ProviderKind::Anthropic => r.weights.anthropic.as_ref().is_some_and(|f| f.forced_tool_choice),
        ProviderKind::OpenaiCompat => true,
    }
}

/// Один ход разговора.
pub struct Turn<'a, P> {
    pub provider: &'a P,
    pub tools: &'a Tools,
    pub system: String,
    /// Описания инструментов, которые уходят модели.
    pub tool_specs: Vec<ToolSpec>,
    pub settings: Settings,
    /// Куда уходит большой вывод инструментов.
    pub spill_dir: PathBuf,
    pub tool_timeout: Duration,
    pub retries: u32,
    /// Ящик, куда приходят итоги подагентов; перед каждым вызовом модели
    /// дошедшее ложится в историю.
    pub inbox: Option<Inbox>,
    /// Куда ход кладёт снимок своей истории перед исполнением вызовов —
    /// оттуда подагент берёт разговор родителя. Сама история не меняется.
    pub transcript: Option<Transcript>,
    /// Завершающие инструменты: их успешный вызов кончает ход сразу, без
    /// нового вызова модели (у muse `nothing_to_report`, `notify_main_agent`:
    /// «Terminal: the run ends immediately», `strings-all.txt:85281`).
    pub terminal: Vec<String>,
    /// Узнаёт о каждой исполненной пачке вызовов (монитор подагентов).
    /// Не блокирует: крючок отдаёт событие и ход сразу идёт дальше.
    pub on_step: Option<StepHook>,
}

/// Исполненный вызов: имя, аргументы, ошибка ли, первая строка результата.
#[derive(Debug, Clone)]
pub struct StepCall {
    pub name: String,
    pub arguments: Value,
    pub is_error: bool,
    pub result: String,
}

pub type StepHook = std::sync::Arc<dyn Fn(Vec<StepCall>) + Send + Sync>;

impl<'a, P: Provider> Turn<'a, P> {
    pub fn new(provider: &'a P, tools: &'a Tools, system: impl Into<String>, spill_dir: impl Into<PathBuf>) -> Self {
        Turn {
            provider,
            tools,
            system: system.into(),
            tool_specs: tools.specs(),
            settings: Settings::default(),
            spill_dir: spill_dir.into(),
            tool_timeout: TOOL_TIMEOUT,
            retries: MODEL_RETRIES,
            inbox: None,
            transcript: None,
            terminal: Vec::new(),
            on_step: None,
        }
    }

    /// Ход на неизменном начале разговора: системный текст и описания
    /// инструментов — из `prefix`, байт в байт одни и те же на каждом ходе.
    pub fn from_prefix(provider: &'a P, tools: &'a Tools, prefix: &Prefix, spill_dir: impl Into<PathBuf>) -> Self {
        let mut turn = Self::new(provider, tools, prefix.system.clone(), spill_dir);
        turn.tool_specs = prefix.tools.clone();
        turn
    }

    /// Ход до ответа без вызовов. Видимый текст по мере потока уходит в
    /// `on_text`; история пополняется ответами модели и результатами.
    pub async fn run(&self, history: &mut Vec<Message>, on_text: &mut OnText<'_>) -> Result<Response, TurnError> {
        self.steps(history, on_text, &self.tool_specs, &self.settings, None).await
    }

    /// Ход, ответ которого — значение по JSON Schema. У весов с
    /// принудительным вызовом — `tool_choice` на `submit_result`; без него —
    /// `tool_choice: auto` и указание muse «Complete the task by calling …
    /// exactly once» заметкой движка в хвосте.
    pub async fn run_contract(
        &self,
        history: &mut Vec<Message>,
        on_text: &mut OnText<'_>,
        schema: &Value,
        forced: bool,
    ) -> Result<Value, TurnError> {
        let mut result = schema.clone();
        if let Some(o) = result.as_object_mut() {
            o.insert("description".into(), RESULT_DESCRIPTION.into());
        }
        let mut tools = self.tool_specs.clone();
        tools.push(ToolSpec {
            name: SUBMIT_RESULT.into(),
            description: SUBMIT_RESULT_DESCRIPTION.into(),
            input_schema: json!({
                "type": "object",
                "properties": {"result": result},
                "required": ["result"],
                "additionalProperties": false,
            }),
        });
        let mut settings = self.settings.clone();
        if forced {
            settings.tool_choice = ToolChoice::Tool { name: SUBMIT_RESULT.into() };
        } else {
            settings.tool_choice = ToolChoice::Auto;
            let schema_text = serde_json::to_string_pretty(schema).unwrap_or_default();
            history.push(Message {
                role: Role::Developer,
                content: vec![Part::Text {
                    text: format!(
                        "Complete the task by calling `{SUBMIT_RESULT}` exactly once. Put the entire completed value in its `result` argument and do not return that value as assistant text. The `result` argument must validate against this JSON Schema:\n```json\n{schema_text}\n```"
                    ),
                }],
            });
        }
        let resp = self.steps(history, on_text, &tools, &settings, Some(SUBMIT_RESULT)).await?;
        let value = resp
            .tool_calls()
            .find_map(|p| match p {
                Part::ToolCall { arguments, .. } => arguments.get("result").cloned(),
                _ => None,
            })
            .ok_or_else(|| TurnError::OutputContract(format!("`{SUBMIT_RESULT}` without `result`")))?;
        Ok(value)
    }

    async fn steps(
        &self,
        history: &mut Vec<Message>,
        on_text: &mut OnText<'_>,
        tools: &[ToolSpec],
        settings: &Settings,
        contract: Option<&str>,
    ) -> Result<Response, TurnError> {
        loop {
            if let Some(inbox) = &self.inbox {
                history.extend(inbox.drain());
            }
            let resp = self.infer(history, on_text, tools, settings).await?;
            let calls: Vec<(String, String, Value)> = resp
                .content
                .iter()
                .filter_map(|p| match p {
                    Part::ToolCall { id, name, arguments } => Some((id.clone(), name.clone(), arguments.clone())),
                    _ => None,
                })
                .collect();
            if resp.stop_reason == StopReason::MaxTokens {
                // Повторить уже нельзя: оборванные вызовы не исполняются.
                let kept: Vec<Part> =
                    resp.content.iter().filter(|p| !matches!(p, Part::ToolCall { .. })).cloned().collect();
                if !kept.is_empty() {
                    history.push(Message { role: Role::Assistant, content: kept });
                }
                return Ok(resp);
            }
            if let Some(name) = contract {
                let submits = calls.iter().filter(|c| c.1 == name).count();
                if submits > 0 && calls.len() != 1 {
                    return Err(TurnError::OutputContract(format!(
                        "`{name}` must be called exactly once and alone; got {submits} of {} calls",
                        calls.len()
                    )));
                }
                if calls.is_empty() {
                    history.push(Message { role: Role::Assistant, content: resp.content.clone() });
                    return Err(TurnError::OutputContract(format!("the response has no `{name}` call")));
                }
                if submits == 1 {
                    history.push(Message { role: Role::Assistant, content: resp.content.clone() });
                    history.push(Message {
                        role: Role::User,
                        content: vec![Part::ToolResult { call_id: calls[0].0.clone(), content: vec![], is_error: false }],
                    });
                    return Ok(resp);
                }
            }
            history.push(Message { role: Role::Assistant, content: resp.content.clone() });
            if calls.is_empty() {
                return Ok(resp);
            }
            if let Some(t) = &self.transcript {
                t.set(history);
            }
            let names: Vec<String> = calls.iter().map(|c| c.1.clone()).collect();
            let args: Vec<Value> = calls.iter().map(|c| c.2.clone()).collect();
            let results = join_all(calls.into_iter().map(|(id, name, args)| self.dispatch(id, name, args))).await;
            let ended = results.iter().zip(&names).any(|(r, n)| {
                self.terminal.contains(n) && matches!(r, Part::ToolResult { is_error: false, .. })
            });
            if let Some(hook) = &self.on_step {
                let step = results
                    .iter()
                    .zip(names.iter().zip(args))
                    .map(|(r, (name, arguments))| {
                        let (is_error, result) = match r {
                            Part::ToolResult { is_error, content, .. } => (
                                *is_error,
                                content
                                    .iter()
                                    .find_map(|c| match c {
                                        ToolResultPart::Text { text } => text.lines().next().map(str::to_string),
                                        _ => None,
                                    })
                                    .unwrap_or_default(),
                            ),
                            _ => (false, String::new()),
                        };
                        StepCall { name: name.clone(), arguments, is_error, result }
                    })
                    .collect();
                hook(step);
            }
            history.push(Message { role: Role::User, content: results });
            if ended {
                return Ok(resp);
            }
        }
    }

    /// Вызов модели с повтором по правилам muse (`strings-all.txt:54780`):
    /// повтор — только пока владелец ничего не увидел («before visible
    /// output»): при ошибке поставщика «можно повторить», при сбое разбора
    /// потока, при неверном имени инструмента, после упора в предел ответа
    /// (с вставкой «Internal retry notice»), после ответа из одного
    /// мышления (с вставкой «thinking-only retry notice»). Если у ошибки
    /// есть `retry_after_ms` — перед повтором ждём столько. После видимого текста ошибка
    /// не повторяется, увиденное остаётся в истории («preserving partial
    /// assistant output without retry»).
    async fn infer(
        &self,
        history: &mut Vec<Message>,
        on_text: &mut OnText<'_>,
        tools: &[ToolSpec],
        settings: &Settings,
    ) -> Result<Response, TurnError> {
        let mut attempt = 0;
        loop {
            let req = Request {
                system: self.system.clone(),
                messages: history.clone(),
                tools: tools.to_vec(),
                settings: settings.clone(),
            };
            let mut shown = String::new();
            let result = {
                let mut sink = |t: &str| {
                    shown.push_str(t);
                    on_text(t)
                };
                self.provider.stream(&req, &mut sink).await
            };
            let can_retry = shown.is_empty() && attempt < self.retries;
            match result {
                Err(e) => {
                    let repeatable = e.retryable || e.kind == ProviderErrorKind::Decode;
                    if repeatable && can_retry {
                        // Срок повтора называет сервер (`retry-after`); нет его —
                        // повтор сразу.
                        if let Some(ms) = e.retry_after_ms {
                            tokio::time::sleep(Duration::from_millis(ms)).await;
                        }
                        attempt += 1;
                        continue;
                    }
                    if !shown.is_empty() {
                        history.push(Message { role: Role::Assistant, content: vec![Part::Text { text: shown }] });
                    }
                    return Err(TurnError::Model(e));
                }
                Ok(resp) => {
                    let bad_name = resp.tool_calls().any(|p| matches!(p, Part::ToolCall { name, .. } if !valid_tool_name(name)));
                    if can_retry && bad_name {
                        attempt += 1;
                        continue;
                    }
                    let notice = if resp.stop_reason == StopReason::MaxTokens {
                        Some(OUTPUT_TOKEN_CAP_NOTICE)
                    } else if thinking_only(&resp) {
                        Some(THINKING_ONLY_NOTICE)
                    } else {
                        None
                    };
                    if let (true, Some(text)) = (can_retry, notice) {
                        history.push(Message {
                            role: Role::Developer,
                            content: vec![Part::Text { text: text.into() }],
                        });
                        attempt += 1;
                        continue;
                    }
                    return Ok(resp);
                }
            }
        }
    }

    async fn dispatch(&self, id: String, name: String, args: Value) -> Part {
        let (content, is_error) = match self.execute(&id, &name, args).await {
            Ok(content) => (content, false),
            Err(text) => (vec![ToolResultPart::Text { text }], true),
        };
        Part::ToolResult { call_id: id, content, is_error }
    }

    async fn execute(&self, id: &str, name: &str, args: Value) -> Result<Vec<ToolResultPart>, String> {
        // Текст muse, `strings-all.txt:19390`.
        let tool = self.tools.get(name).ok_or_else(|| {
            format!(
                "Unknown tool `{name}`: no exact match exists in this agent's advertised tool list. \
                 This is a non-retryable availability result; do not repeat the unchanged call."
            )
        })?;
        if !args.is_object() {
            // Тексты muse, `strings-all.txt:11963`, `:54787`.
            return Err(format!(
                "Invalid arguments for {name}: invalid arguments JSON: arguments did not match the tool schema"
            ));
        }
        match tokio::time::timeout(self.tool_timeout, tool.call(args)).await {
            Err(_) => Err(format!("tool `{name}` did not finish within {:?} and was stopped", self.tool_timeout)),
            Ok(Err(ToolError::InvalidArguments(d))) => Err(format!("Invalid arguments for {name}: {d}")),
            Ok(Err(ToolError::Failed(d))) => Err(d),
            Ok(Ok(content)) => spill::bound(id, content, &self.spill_dir).await,
        }
    }
}

/// Ответ из одного мышления: ни текста, ни вызовов.
fn thinking_only(resp: &Response) -> bool {
    resp.content.iter().any(|p| matches!(p, Part::Thinking { .. } | Part::RedactedThinking { .. }))
        && !resp
            .content
            .iter()
            .any(|p| matches!(p, Part::ToolCall { .. }) || matches!(p, Part::Text { text } if !text.trim().is_empty()))
}

/// Имя вызова, которое не может быть именем инструмента: пустое, длиннее
/// 128 знаков или со знаками вне `[A-Za-z0-9_.-]`. Такой ответ у muse
/// повторяется («invalid model tool call name», `strings-all.txt:54780`);
/// правильное по форме, но незнакомое имя — результат с ошибкой.
fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}
