# Changelog

This project follows [Semantic Versioning](https://semver.org). Rule types, options and
results are `#[non_exhaustive]`, so new rule fields, rule kinds and options arrive in minor
releases. Build values with their constructors (`Rules::new`, `GroupRules::new`, `Build::new`,
`Pack::new`, `DealStep::to`, `StockRules::to`, …) or from JSON rather than struct literals, and
give `match`es on rule enums a wildcard arm. Changes to search heuristics or preset tuning may
change which solution is found and how fast, never a verdict ("unsolvable" stays a proof).

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
