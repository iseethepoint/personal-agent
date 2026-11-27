// Своё (см. js.rs): глобальные функции хозяина, которых ждёт обвязка ядра
// muse, — поверх одной родной `__workflowPost`.
(() => {
  const post = globalThis.__workflowPost;
  const pending = new Map();
  let seq = 0;
  globalThis.__workflowAdmitAgentCall = () => undefined;
  globalThis.__workflowPhaseEvent = (name, status, error) => {
    post(JSON.stringify({ kind: "phase", name, status, error: error ?? null }));
  };
  globalThis.__workflowAgent = (prompt, options, phase, replayKey, label) =>
    new Promise((resolve, reject) => {
      seq += 1;
      pending.set(seq, { resolve, reject });
      post(JSON.stringify({ kind: "agent", seq, prompt: String(prompt), options, phase, replayKey, label }));
    });
  globalThis.__workflowResolve = (id, ok, value) => {
    const p = pending.get(id);
    pending.delete(id);
    if (ok) {
      p.resolve(value);
    } else {
      p.reject(new Error(value));
    }
  };
})();
