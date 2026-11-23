// Smoke test for the nodejs build: node scripts/smoke.mjs
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { Solver, presets } = require("../pkg/nodejs/dealbreaker.js");

const list = JSON.parse(presets());
console.log(`${list.length} presets`);
const fc = Solver.fromPreset("freecell");
const state = fc.dealMicrosoft(1);
const r = JSON.parse(fc.solve(state, JSON.stringify({ max_states: 200000 })));
console.log("freecell #1:", r.outcome, r.steps.length, "steps");
let s = state;
for (const step of r.steps) s = fc.playStep(s, JSON.stringify(step));
if (!fc.isWon(s)) throw new Error("replayed solution doesn't win");
console.log("replayed through playStep: won");

const k = Solver.fromPreset("klondike");
const ks = k.deal(3);
console.log("klondike hint:", k.hint(ks, JSON.stringify({ samples: 4 })));
try { fc.playStep(state, JSON.stringify({ type: "deal" })); throw new Error("should throw"); }
catch (e) { console.log("illegal step rejected:", e.message); }

const ps = Solver.fromPreset("poker-squares");
const pstate = ps.deal(1);
const best = JSON.parse(ps.best(pstate, JSON.stringify({ max_states: 20000 })));
let p = pstate;
for (const step of best.steps) p = ps.playStep(p, JSON.stringify(step));
if (ps.score(p) !== best.score) throw new Error("best line doesn't score as claimed");
console.log("poker squares best:", best.score, "points, replayed");

const cs = Solver.fromPreset("cribbage-squares");
const cstate = cs.deal(1);
const cbest = JSON.parse(cs.best(cstate, JSON.stringify({ max_states: 20000 })));
let c = cstate;
for (const step of cbest.steps) c = cs.playStep(c, JSON.stringify(step));
if (cs.score(c) !== cbest.score) throw new Error("cribbage best line doesn't score as claimed");
console.log("cribbage squares best:", cbest.score, "points, replayed");
