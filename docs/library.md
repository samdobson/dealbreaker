# Libraries & Bindings

Use Dealbreaker directly inside your own app or script—in Rust, JavaScript/TypeScript (WebAssembly), Python, or via the C ABI.

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

Other entry points: `Game::legal_steps` / `Game::play_step` (interactive play), `hint` (fair advice), `maximize` / `Game::score` (points games), `solve_with_cancel` / `maximize_with_cancel` / `hint_with_cancel` and `SolveOptions::timeout_ms` / `max_duration` (cooperative cancellation and wall-clock timeouts), `microsoft_deal` (MS FreeCell deals), and `Game::with_hooks` (custom Rust rules).

## JavaScript / TypeScript (WebAssembly)

Install from npm:

```sh
npm install dealbreaker
```

Use in browsers, bundlers (Vite, webpack, esbuild), or Node.js (v18+):

```ts
import { init, Solver } from "dealbreaker";

await init(); // optional in Node.js (auto-initialized on import)

const solver = Solver.fromPreset("klondike"); // or new Solver(rules)
const state = solver.deal(42);
const result = solver.solve(state, { max_states: 500_000 });
// { outcome: "solved" | "unsolvable" | "gave-up", states, steps?: [...] }

const hint = solver.hint(state); // { step, wins, samples } | null
if (hint) {
  const next = solver.playStep(state, hint.step);
}

// Points games:
const squares = Solver.fromPreset("poker-squares");
const best = squares.best(squares.deal(1)); // { score, proven, states, steps }

solver.free();
squares.free();
```

### Running searches in a Web Worker (browsers)

`Solver` searches synchronously; for browser UIs, use `WorkerSolver` to keep the main thread responsive and cancel in-flight searches via `terminate()`:

```ts
import { WorkerSolver } from "dealbreaker/worker";

const solver = await WorkerSolver.fromPreset("klondike");
const state = await solver.deal(42);
const result = await solver.solve(state, { max_states: 500_000 });
solver.terminate();
```

### Building the WASM package from source

```sh
./scripts/build-npm.sh   # builds the npm package in js/
./scripts/build-wasm.sh  # raw wasm-bindgen output in pkg/<target>
```

## Python

Install from PyPI (Python 3.10+, pre-built wheels for Linux, macOS, and Windows):

```sh
pip install dealbreaker
```

```python
from dealbreaker import Solver

solver = Solver.from_preset("klondike")  # or Solver(rules_dict)
state = solver.deal(42)
result = solver.solve(state, max_states=500_000, timeout_ms=2_000)
# {"outcome": "solved" | "unsolvable" | "gave-up", "states": ..., "steps": [...]}

hint = solver.hint(state, samples=16)  # releases the GIL and uses rayon across cores
if hint:
    next_state = solver.play_step(state, hint["step"])
```

Long-running searches (`solve`, `best`, `hint`) release the Python GIL so `ThreadPoolExecutor` threads can solve deals concurrently.

## C / Swift / Kotlin (C ABI)

`crates/dealbreaker-ffi` exposes a minimal JSON-in/JSON-out C ABI ([dealbreaker.h](https://github.com/samdobson/dealbreaker/blob/main/crates/dealbreaker-ffi/include/dealbreaker.h)) with thread-safe cooperative cancellation (`DealbreakerCancel`), suitable for C, C++, Swift (iOS/macOS), Kotlin/JNI (Android), Go, and Zig:

```sh
cargo build --release -p dealbreaker-ffi   # produces libdealbreaker_ffi.{a,dylib,so,dll}
./scripts/build-xcframework.sh             # builds Apple Dealbreaker.xcframework (iOS + macOS)
```

## Stability (SemVer 1.x)

* Rule types are `#[non_exhaustive]`: construct via `Rules::new`, `GroupRules::new`, `Build::new`, etc., or JSON, and include a wildcard arm when matching rule enums.
* 1.0 JSON rules remain compatible across all 1.x releases.
* Heuristic/preset tuning may change solution paths or speed, never verdicts.

See [CHANGELOG.md](https://github.com/samdobson/dealbreaker/blob/main/CHANGELOG.md) for release history.
