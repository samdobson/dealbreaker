# Verdicts & Soundness

When you ask Dealbreaker to solve a deal, you get one of three clear verdicts:

* **`solved`** — A legal winning sequence of moves was found. Every solution in the test suite is replayed step-by-step through the checked move API.
* **`unsolvable`** — Every distinct reachable position was exhausted without finding a win. Positions are deduplicated by 128-bit canonical hash.
* **`gave-up`** — The search hit its budget (`max_states` or `timeout_ms`) before finding a win or proving none exists.

## Why "Unsolvable" Is Trustworthy

A fast solver is only useful if its shortcuts never throw away the only winning line. Dealbreaker enforces two strict rules:

1. **Unsound shortcuts never report `unsolvable`.** The default `auto` strategy runs a fast best-first pass with one aggressive prune for half the budget; if it doesn't win, it falls back to a completely sound search before ever claiming `unsolvable`.
2. **Differential testing against brute force.** Every game mechanic—draw-1/draw-3 talons, grid redeals, neighbor pairing, compacting rows, jokers, and points games—is tested on small-pack mini-games across dozens of seeds against an unpruned brute-force solver. All three strategies (`auto`, `best_first`, `depth_first`) must agree with brute force on every single verdict.

See [Engine & Search Architecture](/explanation/engine-and-search) for how pruning and canonical hashing stay sound.
