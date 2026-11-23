# Getting Started

Dealbreaker models solitaire games as small JSON files and solves them all with the same fast Rust engine. Pick whichever way you want to use it—you'll be solving deals in under a minute.

::: tip Want to try it without installing anything?
Open the <a href="/dealbreaker/demo/" target="_self">Live Web Demo</a> to play, solve, get hints, and live-edit game rules right in your browser via WebAssembly.
:::

## Install & Run Your First Solve

::: code-group

```sh [CLI]
# Install from crates.io (or download a pre-built binary from GitHub Releases)
cargo install dealbreaker-cli

# Solve Microsoft FreeCell deal #1
dealbreaker solve freecell --ms 1
```

```ts [JavaScript / TS]
// npm install dealbreaker
import { init, Solver } from "dealbreaker";

await init(); // optional in Node.js (auto-initialized on import)

const solver = Solver.fromPreset("freecell");
const state = solver.deal(1);
const result = solver.solve(state);
console.log(result.outcome, result.steps?.length);

solver.free();
```

```python [Python]
# pip install dealbreaker
from dealbreaker import Solver

solver = Solver.from_preset("freecell")
state = solver.deal(1)
result = solver.solve(state)
print(result["outcome"], len(result.get("steps", [])))
```

```rust [Rust]
// cargo add dealbreaker
use dealbreaker::{presets, solve, Game, Outcome, SolveOptions};

let game = Game::new(presets::freecell())?;
let deal = game.deal(1);
let result = solve(&game, &deal, &SolveOptions::default());

if let Outcome::Solved(moves) = result.outcome {
    println!("Solved in {} steps!", game.steps(&deal, &moves).len());
}
```

:::

## Three Things You Can Ask Dealbreaker

1. **Can this deal be won?** — `solve` finds a winning sequence of moves or proves that no win exists.
2. **What should I play next?** — `hint` hides face-down cards and unknown future shuffles, sampling possible arrangements to suggest a fair move.
3. **What's the highest score?** — `best` / `maximize` finds the optimal score in points games like Poker Squares and Cribbage Squares.

## Where to Go Next

- [Built-in Games](/games) — see all 39 ready-to-play presets
- [How Answers Work](/outcomes) — understand verdicts, hidden cards, and search budgets
- [Command-Line Reference](/cli) — flags, benchmarking, and JSONL stdio server mode
- [Libraries & Bindings](/library) — full guides for Rust, JS/WASM (including Web Workers), Python, and C/Swift/Kotlin
- [Custom Rules (JSON)](/rules-format) — create your own solitaire variant in JSON
