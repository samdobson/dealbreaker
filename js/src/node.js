import { readFileSync } from "node:fs";
import * as wasm from "../wasm/dealbreaker.js";
import { createBindings } from "./core.js";

const wasmBytes = readFileSync(
  new URL("../wasm/dealbreaker_bg.wasm", import.meta.url),
);
wasm.initSync({ module: wasmBytes });

export function init() {
  return Promise.resolve();
}

export const { presets, formatStep, Solver } = createBindings(wasm, () => {});
