/// <reference lib="esnext.disposable" />

export * from "./types.js";

import type {
  Difficulty,
  GradeOptions,
  Hint,
  HintOptions,
  MaxResult,
  Rules,
  SolveOptions,
  SolveResult,
  StateData,
  Step,
} from "./types.js";

/**
 * Runs a Dealbreaker `Solver` inside a dedicated Web Worker so searches never block the UI thread.
 * Call `terminate()` to stop the worker and cancel any in-flight search.
 */
export class WorkerSolver {
  static fromPreset(id: string, wasmUrl?: URL | string): Promise<WorkerSolver>;
  static fromRules(rules: Rules | string, wasmUrl?: URL | string): Promise<WorkerSolver>;

  rules(): Promise<Rules>;
  deal(seed: number): Promise<StateData>;
  dealMicrosoft(n: number): Promise<StateData>;
  dealOrdered(cards: readonly string[] | string): Promise<StateData>;
  validate(state: StateData | string): Promise<void>;
  solve(state: StateData | string, options?: SolveOptions | string): Promise<SolveResult>;
  best(state: StateData | string, options?: SolveOptions | string): Promise<MaxResult>;
  score(state: StateData | string): Promise<number | null>;
  hint(state: StateData | string, options?: HintOptions | string): Promise<Hint | null>;
  grade(state: StateData | string, options?: GradeOptions | string): Promise<Difficulty>;
  legalSteps(state: StateData | string): Promise<Step[]>;
  playStep(state: StateData | string, step: Step | string): Promise<StateData>;
  isWon(state: StateData | string): Promise<boolean>;
  render(state: StateData | string): Promise<string>;

  /** Terminate the worker immediately (also cancels any in-flight search). */
  terminate(): void;

  /** Alias for `terminate()` supporting explicit resource management (`using solver = ...`). */
  [Symbol.dispose](): void;
}
