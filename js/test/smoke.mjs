import test from "node:test";
import assert from "node:assert/strict";
import { init, presets, Solver } from "../src/node.js";

test("init() is idempotent in Node", async () => {
  await init();
  await init();
});

test("presets() returns built-in games", () => {
  const list = presets();
  assert.ok(Array.isArray(list));
  assert.ok(list.length >= 39);
  assert.ok(list.some((p) => p.id === "freecell" && p.name === "FreeCell"));
  assert.ok(list.some((p) => p.id === "klondike"));
});

test("FreeCell Microsoft #1 solves and replays via playStep", () => {
  const fc = Solver.fromPreset("freecell");
  try {
    const state = fc.dealMicrosoft(1);
    assert.ok(state.piles.tableau.length === 8);
    fc.validate(state);
    assert.equal(fc.score(state), null);
    assert.equal(fc.isWon(state), false);

    const legal = fc.legalSteps(state);
    assert.ok(legal.length > 0);

    const result = fc.solve(state, { max_states: 200_000 });
    assert.equal(result.outcome, "solved");
    assert.ok(Array.isArray(result.steps) && result.steps.length > 0);

    let cur = state;
    for (const step of result.steps) {
      cur = fc.playStep(cur, step);
    }
    assert.equal(fc.isWon(cur), true);

    assert.throws(() => fc.playStep(state, { type: "deal" }));
  } finally {
    fc.free();
  }
});

test("Solver round-trips Rules object and accepts raw JSON strings", () => {
  const k1 = Solver.fromPreset("klondike");
  const rules = k1.rules();
  k1.free();

  const k2 = new Solver(rules);
  const k3 = new Solver(JSON.stringify(rules));
  try {
    const s2 = k2.deal(3);
    const s3 = k3.deal(3);
    assert.deepEqual(s2, s3);

    const h = k2.hint(s2, { samples: 4, seed: 1 });
    assert.ok(h !== null);
    assert.equal(h.samples, 4);
    assert.ok(typeof h.wins === "number");
    assert.ok(h.step && typeof h.step.type === "string");
  } finally {
    k2.free();
    k3.free();
  }
});

test("dealOrdered and validate handle valid and invalid inputs", () => {
  const fc = Solver.fromPreset("freecell");
  try {
    const suits = ["C", "D", "H", "S"];
    const ranks = ["A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K"];
    const deck = [];
    for (const s of suits) {
      for (const r of ranks) deck.push(`${r}${s}`);
    }
    const state = fc.dealOrdered(deck);
    fc.validate(state);

    // Invalid deck (missing cards) throws
    assert.throws(() => fc.dealOrdered(deck.slice(0, 10)));

    // Invalid state (missing a card) throws
    const bad = structuredClone(state);
    bad.piles.tableau[0].pop();
    assert.throws(() => fc.validate(bad));
  } finally {
    fc.free();
  }
});

test("Scored games (Poker Squares & Cribbage Squares) best() and score()", () => {
  for (const id of ["poker-squares", "cribbage-squares"]) {
    const solver = Solver.fromPreset(id);
    try {
      const state = solver.deal(1);
      const best = solver.best(state, { max_states: 20_000 });
      assert.ok(best.score > 0);
      let cur = state;
      for (const step of best.steps) {
        cur = solver.playStep(cur, step);
      }
      assert.equal(solver.score(cur), best.score);
    } finally {
      solver.free();
    }
  }
});
