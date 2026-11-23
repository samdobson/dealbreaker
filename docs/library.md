# Using Dealbreaker as a Library

## Rust

Add `dealbreaker` to `Cargo.toml`, load a preset or JSON rules, deal a position, and solve:

```rust
use dealbreaker::{presets, solve, Game, Outcome, SolveOptions};

let game = Game::new(presets::klondike1())?; // or Rules::from_json(...)
let deal = game.deal(42);                    // or game.state_from_json(...)
let result = solve(&game, &deal, &SolveOptions::default());

match result.outcome {
    Outcome::Solved(moves) => {
        for step in game.steps(&deal, &moves) {
            println!("{step}");
        }
    }
    Outcome::Unsolvable => println!("no win exists"),
    Outcome::GaveUp => println!("out of budget after {} states", result.states),
}
```

Other entry points: `Game::legal_steps` / `Game::play_step` (interactive play), `hint` (fair advice), `maximize` / `Game::score` (points games), `microsoft_deal` (MS FreeCell deals), and `Game::with_hooks` (custom Rust rules).

## JavaScript / TypeScript (WebAssembly)

Build the ~600 KB dependency-free WASM package:

```sh
./scripts/build-wasm.sh # bundler target; also `web` or `nodejs`
```

```ts
import { Solver } from "./pkg/bundler/dealbreaker.js";

const solver = Solver.fromPreset("klondike"); // or new Solver(rulesJson)
const state = solver.deal(42);
const result = JSON.parse(solver.solve(state, JSON.stringify({ max_states: 500_000 })));
// { outcome: "solved" | "unsolvable" | "gave-up", states, steps?: [...] }

const hint = JSON.parse(solver.hint(state)); // { step, wins, samples } | null
const next = solver.playStep(state, JSON.stringify(hint.step));

// Points games:
const squares = Solver.fromPreset("poker-squares");
const best = JSON.parse(squares.best(squares.deal(1))); // { score, proven, states, steps }
```

Searches are synchronous; run long searches in a Web Worker.

## Stability (SemVer 1.x)

* Rule types are `#[non_exhaustive]`: construct via `Rules::new`, `GroupRules::new`, `Build::new`, etc., or JSON, and include a wildcard arm when matching rule enums.
* 1.0 JSON rules remain compatible across all 1.x releases.
* Heuristic/preset tuning may change solution paths or speed, never verdicts.

See [CHANGELOG.md](../CHANGELOG.md) for release history.
