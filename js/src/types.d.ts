export type Suit = "clubs" | "diamonds" | "hearts" | "spades";

export type Color = "red" | "black";

export type PileKind =
  | "tableau"
  | "foundation"
  | "cell"
  | "reserve"
  | "stock"
  | "waste";

export type BuildBy =
  | "none"
  | "any"
  | "suit"
  | "alternate"
  | "same_color"
  | "other_suit"
  | "suit_or_rank";

export type Dir = "down" | "up" | "either" | "same" | "any";

export interface Build {
  by: BuildBy;
  dir?: Dir;
  wrap?: boolean;
  step?: number;
  blocked_ranks?: number[];
}

export type EmptyRule =
  | "none"
  | "any"
  | "dealt_base"
  | { rank: number }
  | { ranks: number[] }
  | { dealt_base_offset: number }
  | { after_left: { first: number } };

export type MoveUnit =
  | "none"
  | "top"
  | "run"
  | "suit_run"
  | "any_face_up"
  | "supermove"
  | "whole"
  | "top_or_whole";

export type Layout =
  | "tri_peaks"
  | { pyramid: { rows: number } }
  | { covers: { covered_by: number[][] } }
  | { grid: { columns: number } };

export interface PileOverride {
  piles: number[];
  build?: Build;
  empty?: EmptyRule;
  moves?: MoveUnit;
  max_cards?: number;
  max_accept?: number;
  empty_max_accept?: number;
  suit?: Suit;
  empty_suit?: Suit;
  color?: Color;
}

export interface GroupRules {
  name: string;
  kind: PileKind;
  count?: number;
  build?: Build;
  empty?: EmptyRule;
  moves?: MoveUnit;
  max_cards?: number;
  max_accept?: number;
  empty_max_accept?: number;
  accepts_from?: string[];
  refill_from?: string | string[];
  layout?: Layout;
  complete_runs?: boolean;
  reach?: number[];
  compact?: boolean;
  suit?: Suit;
  empty_suit?: Suit;
  color?: Color;
  piles?: PileOverride[];
  wild?: boolean;
  requires_rank_on_prev?: boolean;
  match_longest_rank?: boolean;
  accept_only_when_empty?: boolean;
  accept_when_stock_empty?: boolean;
  compact_face_up?: boolean;
  refill_largest?: boolean;
  refill_on_empty?: boolean;
}

export interface Pack {
  copies?: number;
  suits?: Suit[];
  ranks?: number[];
  jokers?: number;
}

export interface StockRules {
  to: string;
  count?: number;
  redeals?: number;
  unlimited_redeals?: boolean;
  require_no_empty?: boolean;
  auto?: boolean;
  first_empty?: boolean;
  only_when_empty?: boolean;
  skip_empty?: boolean;
  face_up?: boolean;
}

export type RedealHow =
  | "grid_shuffle"
  | { regroup: { per_pile: number } }
  | { shuffle: { per_pile: number } }
  | { compact: { refill_from?: string } };

export interface RedealRules {
  group: string;
  times?: number;
  unlimited?: boolean;
  how: RedealHow;
}

export interface DealStep {
  to: string;
  piles?: number[];
  count?: number;
  counts?: number[];
  rest?: boolean;
  ranks?: number[];
  face_up?: boolean;
  kings_to_bottom?: boolean;
  one_each?: boolean;
  collect?: number[];
  set_base?: boolean;
  of_base?: boolean;
}

export type Adjacency = "eight" | "four";

export type RemovalKind =
  | "lower_of_suit"
  | "same_rank"
  | { pairs_sum: { total: number; alone?: number[] } }
  | { rank_set: { ranks: number[] } };

export interface RemovalRules {
  kind: RemovalKind;
  from: string[];
  to: string;
  adjacent?: Adjacency;
}

export type PokerScoring = "american" | "english";

export interface PokerPoints {
  pair?: number;
  two_pair?: number;
  three?: number;
  straight?: number;
  flush?: number;
  full_house?: number;
  four?: number;
  straight_flush?: number;
  royal_flush?: number;
}

export type PokerTable = PokerScoring | PokerPoints;

export interface CribbagePoints {
  fifteen?: number;
  pair?: number;
  run_card?: number;
  flush?: number;
  flush_starter?: number;
  nobs?: number;
  heels?: number;
}

export interface CribbageRules {
  starter?: string;
  points?: CribbagePoints;
}

export type Hands = { poker: PokerTable } | { cribbage: CribbageRules };

export interface ScoreRules {
  group: string;
  hands: Hands;
}

export type WinRule =
  | "all_on_foundations"
  | { empty: string[] }
  | { count: { group: string; cards: number } }
  | { at_most_piles: { group: string; piles: number } }
  | { rows: { group: string; length?: number } }
  | { score: { at_least: number } };

export interface SearchHints {
  progress_weight?: number;
  down_weight?: number;
  disorder_weight?: number;
  blocker_weight?: number;
  cell_weight?: number;
  stock_weight?: number;
  fast_share?: number;
}

export interface Rules {
  name: string;
  pack?: Pack;
  groups: GroupRules[];
  stock?: StockRules;
  redeal?: RedealRules;
  deal?: DealStep[];
  removal?: RemovalRules | RemovalRules[];
  score?: ScoreRules;
  win?: WinRule;
  auto_flip?: boolean;
  safe_autoplay?: boolean;
  search?: SearchHints;
}

/**
 * A position keyed by group name. Cards are bottom-first, e.g. `"AS"`, `"10H"`, `"~KC"` (face down),
 * or `"JK"` / `"~JK"` (joker). In `hint()`, unknown face-down cards may be written as `"?"` or `"~?"`.
 */
export interface StateData {
  piles: Record<string, string[][]>;
  redeals_used?: number;
  base_rank?: number;
  seed?: number;
}

export interface PileRef {
  group: string;
  index: number;
}

export type Step =
  | { type: "deal" }
  | { type: "redeal" }
  | { type: "move"; from: PileRef; to: PileRef; count: number }
  | { type: "remove"; piles: PileRef[] };

export type Strategy = "auto" | "depth_first" | "best_first";

export interface SolveOptions {
  strategy?: Strategy;
  max_states?: number;
  /** Wall-clock timeout in milliseconds (enforced on native targets). */
  timeout_ms?: number;
  autoplay?: boolean;
  shorten?: boolean;
}

export type OutcomeLabel = "solved" | "unsolvable" | "gave-up";

export interface SolveResult {
  outcome: OutcomeLabel;
  states: number;
  steps?: Step[];
}

export interface MaxResult {
  score: number;
  proven: boolean;
  states: number;
  steps: Step[];
}

export interface HintOptions {
  samples?: number;
  solve?: SolveOptions;
  seed?: number;
}

export interface Hint {
  step: Step;
  wins: number;
  samples: number;
  score?: number;
}

export interface PresetInfo {
  id: string;
  name: string;
}

export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;
