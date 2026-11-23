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
      link: /tutorials/first-solve
    - theme: alt
      text: Try Live Demo
      link: /demo/
      target: _self
    - theme: alt
      text: GitHub
      link: https://github.com/samdobson/dealbreaker

features:
  - title: Tutorials
    details: Learn by doing—solve your first deal, build a custom game in JSON, or add fair hints to a web app.
    link: /tutorials/
    linkText: Start a tutorial
  - title: How-to Guides
    details: Practical recipes for CLI scripting, points games, search tuning, Rust hooks, and mobile/C embedding.
    link: /how-to/
    linkText: Browse guides
  - title: Reference
    details: Auto-generated specs for all 39 presets, Rules JSON Schema, CLI flags, JS/TS, Python, and C ABI.
    link: /reference/
    linkText: Explore reference
  - title: Explanation
    details: Understand verdicts, hidden-card sampling, bit-table search, branch-and-bound scoring, and benchmarks.
    link: /explanation/
    linkText: Read concepts
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
