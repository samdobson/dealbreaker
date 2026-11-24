import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as browserEntry from "../src/index.js";
import { WorkerSolver } from "../src/worker.js";

const wasmBytes = readFileSync(
  new URL("../wasm/dealbreaker_bg.wasm", import.meta.url),
);

// In-process Web Worker shim wired to ../src/worker-runtime.js so WorkerSolver
// and worker-runtime.js can be tested end-to-end under Node without a browser.
let activeClient = null;
const workerSelf = {
  onmessage: null,
  postMessage(data) {
    const client = activeClient;
    if (client && !client.terminated && typeof client.onmessage === "function") {
      queueMicrotask(() => {
        if (!client.terminated) client.onmessage({ data });
      });
    }
  },
};
globalThis.self = workerSelf;
await import("../src/worker-runtime.js");

class MockWorker {
  constructor() {
    this.onmessage = null;
    this.onerror = null;
    this.terminated = false;
    activeClient = this;
  }

  postMessage(data) {
    if (this.terminated) return;
    queueMicrotask(() => {
      if (!this.terminated && typeof workerSelf.onmessage === "function") {
        workerSelf.onmessage({ data });
      }
    });
  }

  terminate() {
    this.terminated = true;
    if (activeClient === this) activeClient = null;
  }
}

globalThis.Worker = MockWorker;

test("browser entrypoint rejects calls before init()", () => {
  assert.throws(() => browserEntry.presets(), /not initialized/);
  assert.throws(() => browserEntry.Solver.fromPreset("freecell"), /not initialized/);
});

test("WorkerSolver.fromPreset initializes runtime and runs full lifecycle", async () => {
  const ws = await WorkerSolver.fromPreset("freecell", wasmBytes);
  try {
    const rules = await ws.rules();
    assert.equal(rules.name, "FreeCell");

    const state = await ws.dealMicrosoft(1);
    await ws.validate(state);
    assert.equal(await ws.isWon(state), false);
    assert.equal(await ws.score(state), null);

    const legal = await ws.legalSteps(state);
    assert.ok(legal.length > 0);

    const solved = await ws.solve(state, { max_states: 200_000 });
    assert.equal(solved.outcome, "solved");
    assert.ok(solved.steps.length > 0);

    const next = await ws.playStep(state, solved.steps[0]);
    assert.notDeepEqual(next, state);

    const h = await ws.hint(await ws.dealMicrosoft(5), { samples: 1 });
    assert.ok(h !== null);
    assert.equal(h.wins, 1);

    // Illegal step propagates worker error as a rejected Promise.
    await assert.rejects(() => ws.playStep(state, { type: "deal" }));
  } finally {
    ws.terminate();
  }
});

test("WorkerSolver.fromRules works with scored games and Symbol.dispose", async () => {
  const psSeed = await WorkerSolver.fromPreset("poker-squares", wasmBytes);
  const rules = await psSeed.rules();
  psSeed.terminate();

  const ws = await WorkerSolver.fromRules(rules, wasmBytes);
  try {
    const state = await ws.deal(1);
    assert.equal(await ws.score(state), 0);
    const best = await ws.best(state, { max_states: 5_000 });
    assert.ok(best.score > 0);
    assert.ok(best.steps.length > 0);
  } finally {
    ws[Symbol.dispose]();
  }

  await assert.rejects(() => ws.rules(), /terminated/);
});

test("WorkerSolver.fromPreset rejects and terminates on invalid preset", async () => {
  await assert.rejects(
    () => WorkerSolver.fromPreset("no-such-preset", wasmBytes),
    /no preset/,
  );
  assert.equal(activeClient, null);
});

test("WorkerSolver.terminate() rejects pending calls and onerror rejects all", async () => {
  // Custom slow/hanging mock worker to deterministically test terminate() and onerror.
  const hangingWorker = {
    onmessage: null,
    onerror: null,
    terminated: false,
    postMessage() {},
    terminate() {
      this.terminated = true;
    },
  };
  const client = new WorkerSolver(hangingWorker);
  const p1 = client.rules();
  const p2 = client.deal(1);
  client.terminate();
  assert.equal(hangingWorker.terminated, true);
  await assert.rejects(p1, /terminated/);
  await assert.rejects(p2, /terminated/);
  await assert.rejects(() => client.rules(), /terminated/);

  // Simulate worker crash via onerror.
  const crashingWorker = {
    onmessage: null,
    onerror: null,
    postMessage() {},
    terminate() {},
  };
  const crashClient = new WorkerSolver(crashingWorker);
  const cp = crashClient.rules();
  crashingWorker.onerror({ message: "worker crashed" });
  await assert.rejects(cp, /worker crashed/);
});
