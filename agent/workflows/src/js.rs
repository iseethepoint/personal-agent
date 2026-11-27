//! Исполнение сценария — встроенный движок boa (как у muse:
//! `boa_engine-0.21.1`, `hatch-workflows/src/runtime.rs`). Сценарий идёт
//! внутри обвязки ядра (`harness.js`); `agent()` и события фаз уходят
//! хозяину, ответы на `agent()` хозяин возвращает в тот же контекст.
//!
//! Контекст boa не `Send` — машина живёт в своём потоке, с хозяином говорит
//! каналами. Своё: прослойка `prelude.js` (у muse глобальные
//! `__workflowAgent`, `__workflowPhaseEvent`, `__workflowAdmitAgentCall` —
//! родные функции ядра, их кода в разборе нет); у нас одна родная функция
//! `__workflowPost` и обещания на стороне JS.

use std::cell::RefCell;
use std::sync::mpsc as std_mpsc;

use boa_engine::{js_string, Context, JsResult, JsValue, NativeFunction, Source};
use serde_json::Value;
use tokio::sync::mpsc;

const HARNESS: &str = include_str!("harness.js");
const PRELUDE: &str = include_str!("prelude.js");

/// Событие машины для хозяина.
#[derive(Debug)]
pub enum Out {
    /// `agent(prompt, options)`: номер ответа, подсказка, опции без
    /// `key`/`label`, фаза, ключ повтора, подпись.
    Agent { seq: u64, prompt: String, options: Value, phase: Option<String>, replay_key: String, label: Option<String> },
    /// Событие фазы: running / completed / failed.
    Phase { name: String, status: String, error: Option<String> },
    /// Сценарий кончился: `__workflowResult` (`WorkflowJsEnvelope`) или
    /// ошибка разбора/исполнения вне обвязки.
    Done(Result<Value, Failure>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// Сценарий не разобрался (`syntax_error`).
    Syntax(String),
    /// Ошибка исполнения вне обвязки.
    Runtime(String),
}

/// Ответ хозяина на `agent()`: результат (или объект-управление
/// `__hatchWorkflowControl`) либо текст ошибки.
pub struct Reply {
    pub seq: u64,
    pub result: Result<Value, String>,
}

thread_local! {
    static POSTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn post(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let s = args.first().cloned().unwrap_or_default().to_string(ctx)?.to_std_string_escaped();
    POSTED.with(|p| p.borrow_mut().push(s));
    Ok(JsValue::undefined())
}

/// Полный текст для движка: обвязка с фазами, `args` и телом.
pub fn source(phases: &[String], args: &Value, body: &str) -> String {
    HARNESS
        .replace("{{PHASES}}", &Value::from(phases.to_vec()).to_string())
        .replace("{{ARGS}}", &args.to_string())
        .replace("{{BODY}}", body)
}

fn js_err(e: boa_engine::JsError) -> String {
    e.to_string()
}

fn eval_str(ctx: &mut Context, code: &str) -> Result<JsValue, String> {
    ctx.eval(Source::from_bytes(code)).map_err(js_err)
}

fn drain(tx: &mpsc::UnboundedSender<Out>, outstanding: &mut usize) {
    let posted: Vec<String> = POSTED.with(|p| std::mem::take(&mut *p.borrow_mut()));
    for s in posted {
        let Ok(v) = serde_json::from_str::<Value>(&s) else { continue };
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        let out = match v["kind"].as_str() {
            Some("agent") => {
                *outstanding += 1;
                Out::Agent {
                    seq: v["seq"].as_u64().unwrap_or_default(),
                    prompt: text("prompt").unwrap_or_default(),
                    options: v.get("options").cloned().unwrap_or(Value::Null),
                    phase: text("phase"),
                    replay_key: text("replayKey").unwrap_or_default(),
                    label: text("label"),
                }
            }
            Some("phase") => Out::Phase { name: text("name").unwrap_or_default(), status: text("status").unwrap_or_default(), error: text("error") },
            _ => continue,
        };
        let _ = tx.send(out);
    }
}

fn result(ctx: &mut Context) -> Result<Option<Value>, String> {
    let v = eval_str(ctx, "globalThis.__workflowResult === undefined ? undefined : JSON.stringify(globalThis.__workflowResult)")?;
    match v.as_string() {
        Some(s) => serde_json::from_str(&s.to_std_string_escaped()).map(Some).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

/// Исполнить сценарий в этом потоке: события — в `tx`, ответы на `agent()`
/// — из `rx`. Канал ответов закрыт — работа снята (пауза, остановка),
/// машина просто кончается.
pub fn run(code: String, tx: mpsc::UnboundedSender<Out>, rx: std_mpsc::Receiver<Reply>) {
    POSTED.with(|p| p.borrow_mut().clear());
    let mut ctx = Context::default();
    if let Err(e) = ctx.register_global_callable(js_string!("__workflowPost"), 1, NativeFunction::from_fn_ptr(post)) {
        let _ = tx.send(Out::Done(Err(Failure::Runtime(js_err(e)))));
        return;
    }
    if let Err(e) = eval_str(&mut ctx, PRELUDE) {
        let _ = tx.send(Out::Done(Err(Failure::Runtime(e))));
        return;
    }
    if let Err(e) = boa_engine::Script::parse(Source::from_bytes(&code), None, &mut ctx) {
        let _ = tx.send(Out::Done(Err(Failure::Syntax(js_err(e)))));
        return;
    }
    if let Err(e) = eval_str(&mut ctx, &code) {
        let _ = tx.send(Out::Done(Err(Failure::Runtime(e))));
        return;
    }
    let mut outstanding = 0usize;
    loop {
        if let Err(e) = ctx.run_jobs() {
            let _ = tx.send(Out::Done(Err(Failure::Runtime(js_err(e)))));
            return;
        }
        drain(&tx, &mut outstanding);
        match result(&mut ctx) {
            Ok(Some(v)) => {
                let _ = tx.send(Out::Done(Ok(v)));
                return;
            }
            Ok(None) => {}
            Err(e) => {
                let _ = tx.send(Out::Done(Err(Failure::Runtime(e))));
                return;
            }
        }
        if outstanding == 0 {
            // Своё: обещания сценария ничего не ждут, а итога нет.
            let _ = tx.send(Out::Done(Err(Failure::Runtime("workflow script stalled without a result".into()))));
            return;
        }
        let Ok(reply) = rx.recv() else { return };
        outstanding -= 1;
        let (ok, value) = match reply.result {
            Ok(v) => (true, v),
            Err(e) => (false, Value::String(e)),
        };
        if let Err(e) = eval_str(&mut ctx, &format!("__workflowResolve({}, {ok}, {value})", reply.seq)) {
            let _ = tx.send(Out::Done(Err(Failure::Runtime(e))));
            return;
        }
    }
}

/// Запустить машину в своём потоке.
pub fn spawn(code: String) -> (mpsc::UnboundedReceiver<Out>, std_mpsc::Sender<Reply>) {
    let (out_tx, out_rx) = mpsc::unbounded_channel();
    let (in_tx, in_rx) = std_mpsc::channel();
    std::thread::Builder::new()
        .name("workflow-js".into())
        .stack_size(16 << 20)
        .spawn(move || run(code, out_tx, in_rx))
        .expect("workflow js thread");
    (out_rx, in_tx)
}

/// Вычислить литерал объекта meta в чистом контексте → JSON.
pub fn literal(object: &str) -> Result<Value, String> {
    let mut ctx = Context::default();
    let v = eval_str(&mut ctx, &format!("JSON.stringify(({object}))"))?;
    let s = v.as_string().ok_or_else(|| "meta is not an object".to_string())?;
    serde_json::from_str(&s.to_std_string_escaped()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn drive(body: &str, answer: impl Fn(&str) -> Result<Value, String>) -> (Value, Vec<String>) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let (mut out, tx) = spawn(source(&["a".into(), "b".into()], &json!({"n": 2}), body));
            let mut seen = vec![];
            loop {
                match out.recv().await.expect("event") {
                    Out::Agent { seq, prompt, replay_key, .. } => {
                        seen.push(format!("agent {replay_key}"));
                        tx.send(Reply { seq, result: answer(&prompt) }).unwrap();
                    }
                    Out::Phase { name, status, .. } => seen.push(format!("phase {name} {status}")),
                    Out::Done(r) => return (r.expect("done"), seen),
                }
            }
        })
    }

    #[test]
    fn harness_runs_agents_phases_and_parallel() {
        let body = r#"
            await phase("a");
            const x = await agent("first", { key: "x" });
            await phase("b");
            const ys = await parallel([1, 2].map((i) => () => agent(`item ${i}`)), { concurrency: 2 });
            return `${x}:${ys.join(",")}:${args.n}`;
        "#;
        let (env, seen) = drive(body, |p| Ok(json!(p.to_uppercase())));
        assert_eq!(env["ok"], true, "{env}");
        assert_eq!(env["result"], "FIRST:ITEM 1,ITEM 2:2");
        assert_eq!(seen[0], "phase a running");
        assert!(seen.contains(&"agent explicit:x".to_string()));
        assert!(seen.contains(&"agent auto:b|parallel:0|agent:0".to_string()), "{seen:?}");
        assert!(seen.contains(&"phase b completed".to_string()), "{seen:?}");
    }

    #[test]
    fn blocked_agent_result_pauses_and_nondeterminism_fails() {
        let (env, _) = drive(r#"await agent("ask", { key: "q" }); return "never";"#, |_| Ok(json!({"__hatchWorkflowControl": "blocked", "result": {"blocked_reason": "needs_user", "message": "Which city?"}})));
        assert_eq!(env["blocked"], true, "{env}");
        assert_eq!(env["blockedDetails"]["message"], "Which city?");
        let (env, _) = drive("return Math.random();", |_| Ok(Value::Null));
        assert_eq!(env["ok"], false);
        assert!(env["error"].as_str().unwrap().starts_with("Math.random() is not available in workflow scripts"), "{env}");
        let (env, _) = drive(r#"await agent("x"); return 1;"#, |_| Err("boom".into()));
        assert_eq!(env["ok"], false);
        assert_eq!(env["error"], "boom");
    }

    #[test]
    fn syntax_error_is_reported() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let (mut out, _tx) = spawn(source(&["a".into()], &json!({}), "return (;"));
            match out.recv().await.unwrap() {
                Out::Done(Err(Failure::Syntax(_))) => {}
                other => panic!("{other:?}"),
            }
        });
    }
}
