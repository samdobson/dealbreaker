function toJson(value) {
  return typeof value === "string" ? value : JSON.stringify(value);
}

function toOptJson(value) {
  if (value === undefined || value === null) return undefined;
  return typeof value === "string" ? value : JSON.stringify(value);
}

export function createBindings(wasm, ensureReady) {
  function presets() {
    ensureReady();
    return JSON.parse(wasm.presets());
  }

  function formatStep(step) {
    ensureReady();
    return wasm.formatStep(toJson(step));
  }

  class Solver {
    #inner;

    constructor(rules) {
      if (rules instanceof wasm.Solver) {
        this.#inner = rules;
      } else {
        ensureReady();
        this.#inner = new wasm.Solver(toJson(rules));
      }
    }

    static fromPreset(id) {
      ensureReady();
      return new Solver(wasm.Solver.fromPreset(id));
    }

    rules() {
      return JSON.parse(this.#inner.rules());
    }

    deal(seed) {
      return JSON.parse(this.#inner.deal(seed));
    }

    dealMicrosoft(n) {
      return JSON.parse(this.#inner.dealMicrosoft(n));
    }

    dealOrdered(cards) {
      return JSON.parse(this.#inner.dealOrdered(toJson(cards)));
    }

    validate(state) {
      this.#inner.validate(toJson(state));
    }

    solve(state, options) {
      return JSON.parse(this.#inner.solve(toJson(state), toOptJson(options)));
    }

    best(state, options) {
      return JSON.parse(this.#inner.best(toJson(state), toOptJson(options)));
    }

    score(state) {
      const s = this.#inner.score(toJson(state));
      return s === undefined ? null : s;
    }

    hint(state, options) {
      return JSON.parse(this.#inner.hint(toJson(state), toOptJson(options)));
    }

    grade(state, options) {
      return JSON.parse(this.#inner.grade(toJson(state), toOptJson(options)));
    }

    legalSteps(state) {
      return JSON.parse(this.#inner.legalSteps(toJson(state)));
    }

    playStep(state, step) {
      return JSON.parse(this.#inner.playStep(toJson(state), toJson(step)));
    }

    isWon(state) {
      return this.#inner.isWon(toJson(state));
    }

    render(state) {
      return this.#inner.render(toJson(state));
    }

    free() {
      if (this.#inner) {
        this.#inner.free();
        this.#inner = null;
      }
    }

    [Symbol.dispose]() {
      this.free();
    }
  }

  return { presets, formatStep, Solver };
}
