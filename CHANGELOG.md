# Changelog

This project follows [Semantic Versioning](https://semver.org). Rule types, options and
results are `#[non_exhaustive]`, so new rule fields, rule kinds and options arrive in minor
releases. Build values with their constructors (`Rules::new`, `GroupRules::new`, `Build::new`,
`Pack::new`, `DealStep::to`, `StockRules::to`, …) or from JSON rather than struct literals, and
give `match`es on rule enums a wildcard arm. Changes to search heuristics or preset tuning may
change which solution is found and how fast, never a verdict ("unsolvable" stays a proof).

## Unreleased

### Added

- Difficulty grading: `dealbreaker::grade` (and `grade_with_cancel`) rates any position easy, medium or hard from solver effort and the share of trap moves along the winning line, with thresholds calibrated for every preset. Unwinnable deals, and deals not won within the budget, are graded hard and flagged by `winnable`. Exposed as the CLI `grade` command (with `--seeds` and `--calibrate` for batches and custom games) and `serve` `grade` requests, Python `Solver.grade`, JavaScript `Solver.grade` / `WorkerSolver.grade`, and C `dealbreaker_solver_grade`.

## 1.3.0

### Bindings, Distribution & Documentation

- Added Python bindings (`crates/dealbreaker-py`, PyO3/maturin) with GIL release during search and automated PyPI wheel/sdist publishing.
- Added C ABI crate (`crates/dealbreaker-ffi`) with C header (`dealbreaker.h`), module map, XCFramework build script, and pre-built FFI release archives.
- Added multi-arch `FROM scratch` Docker image publishing to GHCR (`ghcr.io/samdobson/dealbreaker`), `cargo-binstall` metadata, and static Linux `musl` CLI release binaries.
- Added VitePress documentation site with auto-generated reference pages and a live WebAssembly web demo deployed to GitHub Pages.
- Added optional `schema` feature (`schemars`) and checked-in JSON Schemas (`docs/schemas/*.schema.json`) for rules, states, steps, solve options, and hint options.

### Solver, Core & CLI

- Added optional `parallel` feature (`rayon`) for multi-threaded `hint()` Monte Carlo sampling and CLI `bench`.
- Allowed `"?"` / `"~?"` unknown face-down card placeholders in `hint()` positions (sampling only over the remaining unseen cards).
- Expanded CLI with `--version`, `validate`, `hint --json`, stdin (`-`) support for rules/positions, and strict rejection of unknown flags.
- Expanded rules model with:
  - Opening base-rank choice (`Rules::choose_base`), `Dir::UpAny`, `MoveUnit::AltColorRun` / `SameColorRun`, `EmptyRule::AfterNeighbor` / `AfterPile`, and `SwapBases`.
  - Group rules: `min_cards`, `empty_accepts_from`, `non_empty_max_accept`, `refill_count`, `refill_same_index`, `refill_nearest`, `refill_first_face_down`, `reach_both_ways`, `replace_on_drop`, `requires_prev_non_empty`, `max_same_rank`, `moves_when_stock_empty`, `moves_when_any_empty`, `moves_when_same_index_empty`, `flip_only_last`, `no_flip`, `final_from_same_index`, `lock_build_dir`, `lock_pile_dir`, `require_valid_base`, `supermove_complete_run`, `unrestricted_from`, `rank_sequence`, `unique_suits`, and `no_build_single_while_stock`.
  - Stock & redeal rules: `count_per_round`, `piles`, `piles_per_round`, `final_to`, `final_piles`, `final_split_to`, `deal_per_pile`, `deal_empty_first`, `divert_ranks`, `require_no_empty_group`, `redeal_reverse`, `redeal_shuffle`, `redeal_deal_all_alternating`, and `RedealHow::GridShuffleRandomGaps` / `RotateBottom` / `DealRows` / `RedistributePile` / `ShuffleRedeal`.
  - Deal, removal & win rules: `DealStep::{limit, one_per_suit, split_on_rank, auto_foundation, divert_ranks, drain_foundations}`, `RemovalKind::{ReachSuitOrRank, BetweenMatch, ContiguousSumMultiple}`, and `WinRule::{CountAndSpread, AllFull, RowsBuilt}`.
- Fixed talon compression when waste/stock refills are active and hardened soundness invariants in rule compilation.

## 1.2.0

### JavaScript / TypeScript & Distribution

- Published typed `dealbreaker` npm package (`js/`) with ESM exports for browsers, bundlers, and Node.js (`>=18`), full TypeScript definitions (`Rules`, `StateData`, `Step`, `SolveOptions`, `SolveResult`, `MaxResult`, `Hint`), and `dealbreaker/worker` (`WorkerSolver`) for off-main-thread searches.
- Added `Solver.dealOrdered(cards)` and `Solver.validate(state)` to the WASM/JS API.
- Added automated release workflow (`.github/workflows/release.yml`) for crates.io, npm provenance publishing, and pre-built CLI binaries (Linux x86_64/arm64, macOS arm64/x86_64, Windows x86_64) with `SHA256SUMS.txt`.

### Solver & CLI

- Cooperative cancellation (`solve_with_cancel`, `maximize_with_cancel`, `hint_with_cancel`) via `&AtomicBool`, checked periodically in search loops and guaranteed never to report `Outcome::Unsolvable`.
- Wall-clock search timeouts via `SolveOptions::timeout_ms` / `SolveOptions::max_duration` (native targets) and CLI `--timeout-ms N`.
- Long-lived JSONL stdio server mode (`dealbreaker serve <GAME>`): compiles the game once and serves `deal`, `solve`, `best`, `hint`, `legal_steps`, `play_step`, `validate`, `is_won`, `score`, `rules`, and `quit` requests over `stdin`/`stdout`.

## 1.1.0

The first stable release.

### Games as data

- Rules model: pile groups with build rules (suit, colour, alternate, other suit, suit or rank,
  any; up, down, either, same, any; wrap, step, blocked ranks), empty-pile rules (including
  `dealt_base_offset`), move units (top card to FreeCell supermoves), capacities, per-pile
  overrides, overlapping layouts (pyramid, peaks, any covering graph) and grids.
- Stocks dealing to a waste, foundations or the tableau, with redeals; refills from reserves,
  the waste or the stock; tableau redeals (regroup, shuffle, Montana's grid shuffle, Monte
  Carlo's compaction).
- Removal games: pairs to a total, same-rank pairs, rank sets, lower of suit, optionally only
  between grid neighbours.
- Win conditions: all cards home, groups emptied, a count reached, piles left, rows in
  sequence, a score reached.
- Jokers as wild cards, in building, empty piles, runs, foundations (opt-in), removals and
  scored hands.
- Scoring: grid rows and columns as poker hands (American, English or custom points) or as
  cribbage hands with a starter.
- `Hooks` for one-off rules in Rust.
- 39 presets, from Klondike, FreeCell and Spider to Montana, Accordion, Poker Squares and
  Cribbage Squares.

### Solving

- Depth-first and best-first search with canonical 128-bit hashing, talon compression, safe
  autoplay and solution shortening. "Unsolvable" is a proof: only sound searches report it.
- `maximize` finds the highest score in scored games (limited discrepancy branch and bound).
- `hint` gives advice without looking at face-down cards.
- Search hints (`search` in the rules) tune the heuristic per game, as data; the hardest
  presets ship tuned.

### Interfaces

- Rust library (`dealbreaker`), with JSON for rules, positions and solutions.
- Command line (`dealbreaker-cli`, binary `dealbreaker`): list, rules, deal, solve, replay,
  hint, best, bench, with `--search` to try other heuristic weights.
- WebAssembly bindings for JavaScript and TypeScript.

### Testing

- Differential tests: small versions of every mechanic, solved by brute force and by every
  strategy, must agree exactly. Every solution is replayed through the checked API.
