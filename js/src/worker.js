export class WorkerSolver {
  #worker;
  #nextId = 1;
  #pending = new Map();
  #terminated = false;

  constructor(worker) {
    this.#worker = worker;
    this.#worker.onmessage = (event) => {
      const { id, ok, result, error } = event.data;
      const entry = this.#pending.get(id);
      if (!entry) return;
      this.#pending.delete(id);
      if (ok) entry.resolve(result);
      else entry.reject(new Error(error));
    };
    this.#worker.onerror = (event) => {
      const err = new Error(event.message || "Dealbreaker worker error");
      for (const entry of this.#pending.values()) entry.reject(err);
      this.#pending.clear();
    };
  }

  static async fromPreset(id, wasmUrl) {
    const worker = new Worker(new URL("./worker-runtime.js", import.meta.url), {
      type: "module",
    });
    const client = new WorkerSolver(worker);
    try {
      await client.#call("initPreset", [id, wasmUrl]);
      return client;
    } catch (err) {
      client.terminate();
      throw err;
    }
  }

  static async fromRules(rules, wasmUrl) {
    const worker = new Worker(new URL("./worker-runtime.js", import.meta.url), {
      type: "module",
    });
    const client = new WorkerSolver(worker);
    try {
      await client.#call("initRules", [rules, wasmUrl]);
      return client;
    } catch (err) {
      client.terminate();
      throw err;
    }
  }

  #call(method, args) {
    if (this.#terminated) {
      return Promise.reject(new Error("WorkerSolver has been terminated"));
    }
    const id = this.#nextId++;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage({ id, method, args });
    });
  }

  rules() {
    return this.#call("rules", []);
  }

  deal(seed) {
    return this.#call("deal", [seed]);
  }

  dealMicrosoft(n) {
    return this.#call("dealMicrosoft", [n]);
  }

  dealOrdered(cards) {
    return this.#call("dealOrdered", [cards]);
  }

  validate(state) {
    return this.#call("validate", [state]);
  }

  solve(state, options) {
    return this.#call("solve", [state, options]);
  }

  best(state, options) {
    return this.#call("best", [state, options]);
  }

  score(state) {
    return this.#call("score", [state]);
  }

  hint(state, options) {
    return this.#call("hint", [state, options]);
  }

  grade(state, options) {
    return this.#call("grade", [state, options]);
  }

  legalSteps(state) {
    return this.#call("legalSteps", [state]);
  }

  playStep(state, step) {
    return this.#call("playStep", [state, step]);
  }

  isWon(state) {
    return this.#call("isWon", [state]);
  }

  render(state) {
    return this.#call("render", [state]);
  }

  /** Terminate the worker immediately (also cancels any in-flight search). */
  terminate() {
    if (this.#terminated) return;
    this.#terminated = true;
    this.#worker.terminate();
    const err = new Error("WorkerSolver terminated");
    for (const entry of this.#pending.values()) entry.reject(err);
    this.#pending.clear();
  }

  [Symbol.dispose]() {
    this.terminate();
  }
}
