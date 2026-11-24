import { init, Solver, presets } from "./index.js";

const ALLOWED_METHODS = new Set([
  "rules",
  "deal",
  "dealMicrosoft",
  "dealOrdered",
  "validate",
  "solve",
  "best",
  "score",
  "hint",
  "legalSteps",
  "playStep",
  "isWon",
  "render",
]);

let solver = null;

self.onmessage = async (event) => {
  const { id, method, args } = event.data;
  try {
    let result;
    switch (method) {
      case "initPreset": {
        await init(args[1]);
        if (solver) solver.free();
        solver = Solver.fromPreset(args[0]);
        result = null;
        break;
      }
      case "initRules": {
        await init(args[1]);
        if (solver) solver.free();
        solver = new Solver(args[0]);
        result = null;
        break;
      }
      case "presets": {
        await init(args[0]);
        result = presets();
        break;
      }
      default: {
        if (!solver) {
          throw new Error("WorkerSolver is not initialized");
        }
        if (!ALLOWED_METHODS.has(method) || typeof solver[method] !== "function") {
          throw new Error(`Unknown method: ${method}`);
        }
        result = solver[method](...args);
      }
    }
    self.postMessage({ id, ok: true, result });
  } catch (err) {
    self.postMessage({
      id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};
