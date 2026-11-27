// Обвязка сценария работы — текст ядра muse (strings-all.txt:42448-42858,
// рядом с `workflow JavaScript failed: `). В дампе нет строк короче четырёх
// знаков — закрывающие `  }` верхнего уровня возвращены. Вставки (js.rs,
// `source`): имена фаз из meta, `args` запуска, тело сценария.
(() => {
  const __workflowAllowedPhases = new Set({{PHASES}});
  const __workflowState = {
    events: [],
    logs: [],
    currentPhase: null,
    statementPhase: null,
    phaseEventChain: Promise.resolve(),
    contextPath: [],
    agentCountsByPath: Object.create(null),
    replayKeys: new Set(),
  };
  const args = {{ARGS}};
  globalThis.eval = undefined;
  globalThis.Function = undefined;
  const __NativeDate = globalThis.Date;
  const __NativeMathRandom = globalThis.Math && globalThis.Math.random;
  function __fatalWorkflowError(message) {
    const error = new Error(message);
    error.__workflowFatal = true;
    return error;
  }
  function __isWorkflowFatalError(error) {
    return !!(error && error.__workflowFatal === true);
  }
  function __normalizeWorkflowBlockedDetails(details) {
    const source = details && typeof details === "object" ? details : {};
    const blockedReason = typeof source.blocked_reason === "string" && source.blocked_reason.trim() !== ""
      ? source.blocked_reason.trim()
      : "workflow_blocked";
    const message = typeof source.message === "string" && source.message.trim() !== ""
      ? source.message.trim()
      : blockedReason;
    return { blocked_reason: blockedReason, message };
  }
  function __workflowBlockedError(details) {
    const normalized = __normalizeWorkflowBlockedDetails(details);
    const error = __fatalWorkflowError(normalized.message);
    error.__workflowBlocked = true;
    error.__workflowBlockedDetails = normalized;
    return error;
  }
  function __isWorkflowBlockedError(error) {
    return !!(error && error.__workflowBlocked === true);
  }
  function __workflowBlockedDetails(error) {
    return __normalizeWorkflowBlockedDetails(error && error.__workflowBlockedDetails);
  }
  function __isWorkflowBlockedResult(value) {
    return !!(value && typeof value === "object" && value.__hatchWorkflowControl === "blocked");
  }
  function __normalizeWorkflowParkedReason(reason) {
    return typeof reason === "string" && reason.trim() !== ""
      ? reason.trim()
      : "durable recovery owner is still active";
  }
  function __workflowParkedError(reason) {
    const normalized = __normalizeWorkflowParkedReason(reason);
    const error = __fatalWorkflowError(normalized);
    error.__workflowParked = true;
    error.__workflowParkedReason = normalized;
    return error;
  }
  function __isWorkflowParkedError(error) {
    return !!(error && error.__workflowParked === true);
  }
  function __workflowParkedReason(error) {
    return __normalizeWorkflowParkedReason(error && error.__workflowParkedReason);
  }
  function __isWorkflowParkedResult(value) {
    return !!(value && typeof value === "object" && value.__hatchWorkflowControl === "parked");
  }
  function __workflowRetryableError(reason) {
    if (typeof reason !== "string" || reason.trim() === "") {
      throw __fatalWorkflowError("retryable workflow disposition requires a reason");
    }
    const normalized = reason.trim();
    const error = __fatalWorkflowError(normalized);
    error.__workflowRetryable = true;
    error.__workflowRetryableReason = normalized;
    return error;
  }
  function __isWorkflowRetryableError(error) {
    return !!(error && error.__workflowRetryable === true);
  }
  function __workflowRetryableReason(error) {
    return error.__workflowRetryableReason;
  }
  function __isWorkflowRetryableResult(value) {
    return !!(value && typeof value === "object" && value.__hatchWorkflowControl === "retryable");
  }
  function __throwNondeterministicApi(name) {
    throw __fatalWorkflowError(`${name} is not available in workflow scripts because it would break deterministic replay; pass timestamps or random choices through args`);
  }
  function __errorMessage(error) {
    if (error && typeof error.message === "string") {
      return error.message;
    }
    if (error && typeof error.stack === "string") {
      return error.stack;
    }
    return String(error);
  }
  function __formatLogValue(value) {
    if (typeof value === "string") {
      return value;
    }
    try {
      const rendered = JSON.stringify(value);
      return rendered === undefined ? String(value) : rendered;
    } catch (_error) {
      return String(value);
    }
  }
  function __captureLog(level, values) {
    if (__workflowState.logs.length >= 200) {
      return;
    }
    const line = `[${level}] ${values.map(__formatLogValue).join(" ")}`;
    __workflowState.logs.push(line.length > 2000 ? `${line.slice(0, 2000)}...` : line);
  }
  globalThis.console = Object.freeze({
    log: (...values) => __captureLog("log", values),
    warn: (...values) => __captureLog("warn", values),
    error: (...values) => __captureLog("error", values),
  });
  globalThis.log = (...values) => globalThis.console.log(...values);
  function __WorkflowDate(...values) {
    if (!new.target) {
      __throwNondeterministicApi("Date()");
    }
    if (values.length === 0) {
      __throwNondeterministicApi("new Date()");
    }
    return Reflect.construct(__NativeDate, values, new.target);
  }
  Object.setPrototypeOf(__WorkflowDate, __NativeDate);
  __WorkflowDate.prototype = __NativeDate.prototype;
  __WorkflowDate.parse = __NativeDate.parse.bind(__NativeDate);
  __WorkflowDate.UTC = __NativeDate.UTC.bind(__NativeDate);
  __WorkflowDate.now = () => __throwNondeterministicApi("Date.now()");
  globalThis.Date = __WorkflowDate;
  if (globalThis.Math && __NativeMathRandom) {
    Object.defineProperty(globalThis.Math, "random", {
      configurable: true,
      writable: true,
      value: () => __throwNondeterministicApi("Math.random()"),
    });
  }
  function __recordPhaseEvent(name, status, error = null) {
    __workflowState.events.push(error == null ? { name, status } : { name, status, error });
    __workflowState.phaseEventChain = __workflowState.phaseEventChain.then(() =>
      globalThis.__workflowPhaseEvent(name, status, error)
    );
    return __workflowState.phaseEventChain;
  }
  function __completeStatementPhase(error = null) {
    const name = __workflowState.statementPhase;
    if (name == null) {
      return __workflowState.phaseEventChain;
    }
    __workflowState.statementPhase = null;
    if (__workflowState.currentPhase === name) {
      __workflowState.currentPhase = null;
    }
    if (error == null) {
      return __recordPhaseEvent(name, "completed");
    } else {
      return __recordPhaseEvent(name, "failed", error);
    }
  }
  function __beginStatementPhase(name) {
    if (__workflowState.statementPhase === name) {
      __workflowState.currentPhase = name;
      return __workflowState.phaseEventChain;
    }
    __completeStatementPhase();
    __workflowState.statementPhase = name;
    __workflowState.currentPhase = name;
    return __recordPhaseEvent(name, "running");
  }
  function __withWorkflowContext(segment, body) {
    const previous = __workflowState.contextPath;
    __workflowState.contextPath = previous.concat(segment);
    try {
      return body();
    } finally {
      __workflowState.contextPath = previous;
    }
  }
  function __agentOptions(options) {
    if (options == null) {
      return {};
    }
    if (typeof options !== "object" || Array.isArray(options)) {
      throw new Error("agent options must be an object");
    }
    const copy = Object.assign({}, options);
    delete copy.key;
    delete copy.label;
    return copy;
  }
  function __agentLabel(options) {
    if (!options || !Object.prototype.hasOwnProperty.call(options, "label")) {
      return null;
    }
    if (options.label == null) {
      return null;
    }
    if (typeof options.label !== "string" || options.label.trim() === "") {
      throw __fatalWorkflowError("agent option label must be a non-empty string");
    }
    return options.label.trim();
  }
  function __agentPhase(options) {
    if (options && Object.prototype.hasOwnProperty.call(options, "phase")) {
      if (options.phase == null) {
        return null;
      }
      if (typeof options.phase !== "string" || options.phase.trim() === "") {
        throw __fatalWorkflowError("agent option phase must be a non-empty string");
      }
      if (!__workflowAllowedPhases.has(options.phase)) {
        throw __fatalWorkflowError(`unknown workflow phase: ${options.phase}`);
      }
      return options.phase;
    }
    return __workflowState.currentPhase ?? null;
  }
  function __workflowReplayKey(options, phaseName) {
    if (options && Object.prototype.hasOwnProperty.call(options, "key")) {
      if (typeof options.key !== "string" || options.key.trim() === "") {
        throw __fatalWorkflowError("agent option key must be a non-empty string");
      }
      return `explicit:${options.key}`;
    }
    const phase = phaseName ?? "workflow";
    const path = __workflowState.contextPath.length > 0
      ? __workflowState.contextPath.join("/")
      : "root";
    const base = `${phase}|${path}`;
    const ordinal = __workflowState.agentCountsByPath[base] ?? 0;
    __workflowState.agentCountsByPath[base] = ordinal + 1;
    return `auto:${base}|agent:${ordinal}`;
  }
  function __claimReplayKey(replayKey) {
    if (__workflowState.replayKeys.has(replayKey)) {
      throw __fatalWorkflowError(`duplicate workflow agent replay key: ${replayKey}`);
    }
    __workflowState.replayKeys.add(replayKey);
  }
  const phase = globalThis.phase = async function phase(name, body) {
    if (!__workflowAllowedPhases.has(name)) {
      throw __fatalWorkflowError(`unknown workflow phase: ${name}`);
    }
    if (body === undefined) {
      await __beginStatementPhase(name);
      return undefined;
    }
    if (typeof body !== "function") {
      throw __fatalWorkflowError("phase body must be a function");
    }
    await __completeStatementPhase();
    const priorPhase = __workflowState.currentPhase;
    __workflowState.currentPhase = name;
    try {
      await __recordPhaseEvent(name, "running");
      const result = await body();
      await __recordPhaseEvent(name, "completed");
      return result;
    } catch (error) {
      if (__isWorkflowBlockedError(error) || __isWorkflowParkedError(error) || __isWorkflowRetryableError(error)) {
        throw error;
      }
      await __recordPhaseEvent(name, "failed", __errorMessage(error));
      throw error;
    } finally {
      __workflowState.currentPhase = priorPhase;
    }
  };
  const parallel = globalThis.parallel = async function parallel(items, options = {}) {
    if (!Array.isArray(items)) {
      throw __fatalWorkflowError("parallel items must be an array");
    }
    const requested = Number(options && options.concurrency);
    const concurrency = Number.isFinite(requested) && requested > 0
      ? Math.min(Math.floor(requested), Math.max(items.length, 1))
      : Math.max(items.length, 1);
    const results = new Array(items.length);
    let nextIndex = 0;
    async function worker() {
      while (true) {
        const index = nextIndex++;
        if (index >= items.length) {
          return;
        }
        try {
          const item = items[index];
          const produced = __withWorkflowContext(`parallel:${index}`, () =>
            typeof item === "function" ? item() : item
          );
          results[index] = await produced;
        } catch (error) {
          if (__isWorkflowFatalError(error)) {
            throw error;
          }
          results[index] = null;
        }
      }
    }
    await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, worker));
    return results;
  };
  const pipeline = globalThis.pipeline = async function pipeline(items, ...stages) {
    if (!Array.isArray(items)) {
      throw __fatalWorkflowError("pipeline first argument must be an array");
    }
    if (stages.length === 0) {
      return items.slice();
    }
    for (const stage of stages) {
      if (typeof stage !== "function") {
        throw __fatalWorkflowError("pipeline stage must be a function");
      }
    }
    const results = new Array(items.length);
    await Promise.all(items.map(async (originalItem, index) => {
      let value = originalItem;
      try {
        for (const [stageIndex, stage] of stages.entries()) {
          value = await __withWorkflowContext(`pipeline:${index}/stage:${stageIndex}`, () =>
            stage(value, originalItem, index)
          );
        }
        results[index] = value;
      } catch (error) {
        if (__isWorkflowFatalError(error)) {
          throw error;
        }
        results[index] = null;
      }
    }));
    return results;
  };
  const agent = globalThis.agent = async function agent(prompt, options = {}) {
    globalThis.__workflowAdmitAgentCall();
    const phaseName = __agentPhase(options);
    const replayKey = __workflowReplayKey(options, phaseName);
    __claimReplayKey(replayKey);
    try {
      const result = await globalThis.__workflowAgent(
        prompt,
        __agentOptions(options),
        phaseName,
        replayKey,
        __agentLabel(options)
      );
      if (__isWorkflowBlockedResult(result)) {
        throw __workflowBlockedError(result.result);
      }
      if (__isWorkflowParkedResult(result)) {
        throw __workflowParkedError(result.reason);
      }
      if (__isWorkflowRetryableResult(result)) {
        throw __workflowRetryableError(result.reason);
      }
      return result;
    } catch (error) {
      if (__isWorkflowBlockedError(error) || __isWorkflowParkedError(error) || __isWorkflowRetryableError(error)) {
        throw error;
      }
      throw __fatalWorkflowError(__errorMessage(error));
    }
  };
  (async () => {
    try {
      const result = await (async () => {
{{BODY}}
      })();
      if (__isWorkflowBlockedResult(result)) {
        throw __workflowBlockedError(result.result);
      }
      await __completeStatementPhase();
      await __workflowState.phaseEventChain;
      globalThis.__workflowResult = {
        ok: true,
        result,
        events: __workflowState.events,
        logs: __workflowState.logs,
      };
    } catch (error) {
      if (__isWorkflowRetryableError(error)) {
        await __workflowState.phaseEventChain;
        const reason = __workflowRetryableReason(error);
        globalThis.__workflowResult = {
          ok: false,
          retryable: true,
          retryableReason: reason,
          error: reason,
          events: __workflowState.events,
          logs: __workflowState.logs,
        };
        return;
      }
      if (__isWorkflowParkedError(error)) {
        await __workflowState.phaseEventChain;
        const reason = __workflowParkedReason(error);
        globalThis.__workflowResult = {
          ok: false,
          parked: true,
          parkedReason: reason,
          error: reason,
          events: __workflowState.events,
          logs: __workflowState.logs,
        };
        return;
      }
      if (__isWorkflowBlockedError(error)) {
        await __workflowState.phaseEventChain;
        const details = __workflowBlockedDetails(error);
        globalThis.__workflowResult = {
          ok: false,
          blocked: true,
          blockedDetails: details,
          error: details.message,
          events: __workflowState.events,
          logs: __workflowState.logs,
        };
        return;
      }
      await __completeStatementPhase(__errorMessage(error));
      await __workflowState.phaseEventChain;
      globalThis.__workflowResult = {
        ok: false,
        error: __errorMessage(error),
        events: __workflowState.events,
        logs: __workflowState.logs,
      };
    }
  })();
})();
