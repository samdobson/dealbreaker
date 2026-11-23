---
layout: home

hero:
  name: Dealbreaker
  text: Every solitaire game is data.
  tagline: A fast patience solver written in Rust. Solve deals, prove unsolvability, or get fair hints—from the CLI, Rust, JavaScript/WASM, Python, or C.
  image:
    src: /dealbreaker-logo.png
    alt: Dealbreaker
  actions:
    - theme: brand
      text: Get Started
      link: /getting-started
    - theme: alt
      text: Try Live Demo
      link: /demo/
      target: _self
    - theme: alt
      text: GitHub
      link: https://github.com/samdobson/dealbreaker

features:
  - title: 39 Built-in Games
    details: Klondike, FreeCell, Spider, Yukon, TriPeaks, Poker Squares, and 33 more—ready out of the box.
    link: /games
    linkText: Browse games
  - title: Custom Games in JSON
    details: Describe new variants, layouts, and scoring rules in a small JSON file without touching Rust.
    link: /rules-format
    linkText: Rules format
  - title: Blazing Fast
    details: Solves 31,980 of 32,000 Microsoft FreeCell deals in 13.1 ms mean time and proves #11982 unsolvable in 0.26 s.
    link: /benchmarks
    linkText: See benchmarks
  - title: Use It Anywhere
    details: Ships as a CLI, a Rust crate, a ~600 KB dependency-free WebAssembly npm package, Python wheels, and a C ABI.
    link: /library
    linkText: Explore bindings
---

## A quick taste

Give Dealbreaker any game and deal, and it finds a winning line, proves no win exists, or suggests a fair hint.

::: code-group

```sh [CLI]
cargo install dealbreaker-cli

dealbreaker solve freecell --ms 1
# solved in 112 steps (1330 states, 0.003s)

dealbreaker hint klondike --seed 2
```

```ts [JavaScript / TS]
import { init, Solver } from "dealbreaker";

await init(); // optional in Node.js
const solver = Solver.fromPreset("klondike");
const deal = solver.deal(42);
const result = solver.solve(deal, { max_states: 500_000 });
```

```python [Python]
from dealbreaker import Solver

solver = Solver.from_preset("klondike")
deal = solver.deal(42)
result = solver.solve(deal, max_states=500_000)
```

```rust [Rust]
use dealbreaker::{presets, solve, Game, SolveOptions};

let game = Game::new(presets::klondike1())?;
let deal = game.deal(42);
let result = solve(&game, &deal, &SolveOptions::default());
```

:::
