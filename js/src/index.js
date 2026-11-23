import wasmInit, * as wasm from "../wasm/dealbreaker.js";
import { createBindings } from "./core.js";

let ready = false;
let initPromise = null;

export function init(moduleOrPath) {
  if (ready) return Promise.resolve();
  if (!initPromise) {
    const arg =
      moduleOrPath === undefined ? undefined : { module_or_path: moduleOrPath };
    initPromise = wasmInit(arg).then(
      () => {
        ready = true;
      },
      (err) => {
        initPromise = null;
        throw err;
      },
    );
  }
  return initPromise;
}

function ensureReady() {
  if (!ready) {
    throw new Error(
      'Dealbreaker WASM is not initialized yet. Call "await init()" first.',
    );
  }
}

export const { presets, Solver } = createBindings(wasm, ensureReady);
