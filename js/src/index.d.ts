/// <reference lib="esnext.disposable" />

export * from "./types.js";

import type {
  Difficulty,
  GradeOptions,
  Hint,
  HintOptions,
  InitInput,
  MaxResult,
  PresetInfo,
  Rules,
  SolveOptions,
  SolveResult,
  StateData,
  Step,
} from "./types.js";

/**
 * Initialize the WebAssembly module. Safe to call multiple times.
 *
 * In Node.js, the module is initialized automatically on import, so calling `init()` is optional.
 * In browsers and bundlers, `await init()` must resolve before calling `presets()` or constructing
 * a `Solver`.
 */
export function init(moduleOrPath?: InitInput): Promise<void>;

/** List all built-in game presets. */
export function presets(): PresetInfo[];

/** Format a single step as human-readable text (e.g. `"tableau 4 -> cell 1"`). */
export function formatStep(step: Step | string): string;

/** One game's compiled rules. Create once, reuse across many positions. */
export class Solver {
  /** Compile a game from a `Rules` object or rules JSON string. */
  constructor(rules: Rules | string);

  /** Compile a built-in preset by id (see `presets()`). */
  static fromPreset(id: string): Solver;

  /** The game's compiled rules. */
  rules(): Rules;

  /** Deal a position from a seed (0 to 2^53). */
  deal(seed: number): StateData;

  /** Deal Microsoft FreeCell deal `n` (1 to 2^31 - 1). */
  dealMicrosoft(n: number): StateData;

  /** Deal an explicit card order (`cards[0]` dealt first), e.g. `["AS", "2S", ...]`. */
  dealOrdered(cards: readonly string[] | string): StateData;

  /** Validate a position against this game's pack and pile capacities. Throws if invalid. */
  validate(state: StateData | string): void;

  /** Solve a position. */
  solve(state: StateData | string, options?: SolveOptions | string): SolveResult;

  /** For games played for points: find the line of play that ends with the most points. */
  best(state: StateData | string, options?: SolveOptions | string): MaxResult;

  /** Points for a position, or `null` for games not played for points. */
  score(state: StateData | string): number | null;

  /** Suggest a next step without peeking at face-down cards, or `null` if no win was found. */
  hint(state: StateData | string, options?: HintOptions | string): Hint | null;

  /**
   * Grade a deal as easy, medium or hard. Thresholds are calibrated per preset for the default
   * budgets; unwinnable deals, and deals not won within the budget, are hard.
   */
  grade(state: StateData | string, options?: GradeOptions | string): Difficulty;

  /** Every legal single step in this position. */
  legalSteps(state: StateData | string): Step[];

  /** Play one step and return the resulting position. Throws if the step is illegal. */
  playStep(state: StateData | string, step: Step | string): StateData;

  /** Whether the position is won. */
  isWon(state: StateData | string): boolean;

  /** Render a position as a human-readable ASCII board. */
  render(state: StateData | string): string;

  /** Free the underlying WebAssembly instance. */
  free(): void;

  /** Alias for `free()` supporting explicit resource management (`using solver = ...`). */
  [Symbol.dispose](): void;
}
