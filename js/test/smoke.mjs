import test from "node:test";
import assert from "node:assert/strict";
import { init, presets, formatStep, Solver } from "../src/node.js";

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
    assert.ok(fc.render(state).includes("tableau"));
    assert.ok(formatStep(legal[0]).includes("->"));

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

test("grade classifies deals and honours custom thresholds", () => {
  const fc = Solver.fromPreset("freecell");
  try {
    const state = fc.dealMicrosoft(1);
    const d = fc.grade(state);
    assert.ok(["easy", "medium", "hard"].includes(d.grade));
    assert.equal(d.winnable, true);
    assert.equal(d.source, "preset");
    assert.ok(d.score > 0 && d.effort > 0 && d.probes > 0);
    const easy = fc.grade(state, { thresholds: { medium: 1000, hard: 2000 } });
    assert.equal(easy.grade, "easy");
    assert.equal(easy.source, "custom");
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

test("Error paths, gave-up/unsolvable, unknown '?' hint cards, and Symbol.dispose", () => {
  assert.throws(() => Solver.fromPreset("does-not-exist"));
  assert.throws(() => new Solver("{not json"));

  const fc = Solver.fromPreset("freecell");
  try {
    const deal = fc.dealMicrosoft(11982);
    const gaveUp = fc.solve(deal, { max_states: 1 });
    assert.equal(gaveUp.outcome, "gave-up");

    const unsolvable = fc.solve(deal, { max_states: 5_000_000 });
    assert.equal(unsolvable.outcome, "unsolvable");

    // Invalid options JSON throws.
    assert.throws(() => fc.solve(deal, "{bad"));
  } finally {
    fc[Symbol.dispose]();
    // Double free is a safe no-op.
    fc.free();
  }

  // dealMicrosoft rejects non-52-card games (Spider uses 2 decks).
  const sp = Solver.fromPreset("spider");
  try {
    assert.throws(() => sp.dealMicrosoft(1));
  } finally {
    sp.free();
  }

  // hint() accepts "?" placeholders for face-down cards.
  const kl = Solver.fromPreset("klondike");
  try {
    const state = kl.deal(2);
    for (const piles of Object.values(state.piles)) {
      for (const pile of piles) {
        for (let i = 0; i < pile.length; i++) {
          if (pile[i].startsWith("~")) pile[i] = "?";
        }
      }
    }
    // Strict validate rejects "?", while hint accepts it.
    assert.throws(() => kl.validate(state));
    const h = kl.hint(state, { samples: 2, seed: 1, solve: { max_states: 5_000 } });
    assert.ok(h === null || typeof h.step?.type === "string");
  } finally {
    kl.free();
  }
});
