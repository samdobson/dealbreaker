# Your First Solve

In this tutorial you'll install Dealbreaker, solve Microsoft FreeCell deal `#1`, ask for a fair hint in Klondike, and inspect the result.

## 1. Install Dealbreaker

Pick whichever environment you're most comfortable with:

::: code-group

```sh [CLI]
# From crates.io (or download a pre-built binary from GitHub Releases)
cargo install dealbreaker-cli
```

```sh [JavaScript / TS]
npm install dealbreaker
```

```sh [Python]
pip install dealbreaker
```

```sh [Rust]
cargo add dealbreaker
```

:::

## 2. Solve FreeCell Deal #1

Let's load the built-in `freecell` preset, deal Microsoft FreeCell `#1`, and ask Dealbreaker for a winning line:

::: code-group

```sh [CLI]
dealbreaker solve freecell --ms 1
```

```ts [JavaScript / TS]
import { init, Solver } from "dealbreaker";

await init(); // optional in Node.js (auto-initialized on import)

const solver = Solver.fromPreset("freecell");
const state = solver.dealMicrosoft(1);
const result = solver.solve(state);

console.log(result.outcome, `${result.steps?.length ?? 0} steps`);
solver.free();
```

```python [Python]
from dealbreaker import Solver

solver = Solver.from_preset("freecell")
state = solver.deal_microsoft(1)
result = solver.solve(state)

print(result["outcome"], f"{len(result.get('steps', []))} steps")
```

```rust [Rust]
use dealbreaker::{microsoft_deal, presets, solve, Game, Outcome, SolveOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let game = Game::new(presets::freecell())?;
    let deal = game.deal_ordered(&microsoft_deal(1))?;
    let result = solve(&game, &deal, &SolveOptions::default());

    if let Outcome::Solved(moves) = result.outcome {
        println!("Solved in {} steps!", game.steps(&deal, &moves).len());
    }
    Ok(())
}
```

:::

You'll see a complete, step-by-step winning sequence found in a few milliseconds:

```text
   1. tableau 4 -> cell 1
   2. tableau 4 -> foundation 1
   ...
solved in 112 steps (1330 states, 0.003s)
```

## 3. Ask for a Fair Hint in Klondike

FreeCell has no face-down cards, so `solve` sees the whole board. In Klondike, most of the tableau is face-down. If you want a **fair hint** that never peeks at hidden cards, use `hint`:

::: code-group

```sh [CLI]
dealbreaker hint klondike --seed 2
```

```ts [JavaScript / TS]
const klondike = Solver.fromPreset("klondike");
const deal = klondike.deal(2);
const suggestion = klondike.hint(deal, { samples: 16 });
console.log(suggestion); // { step: ..., wins: ..., samples: 16 }
klondike.free();
```

```python [Python]
klondike = Solver.from_preset("klondike")
deal = klondike.deal(2)
suggestion = klondike.hint(deal, samples=16)
print(suggestion)
```

```rust [Rust]
use dealbreaker::{hint, presets, Game, HintOptions};

let game = Game::new(presets::klondike1())?;
let deal = game.deal(2);
if let Some(h) = hint(&game, &deal, &HintOptions::default()) {
    println!("Suggested: {} ({}/{} wins)", h.step, h.wins, h.samples);
}
```

:::

## Where to Go Next

- **Tutorial:** [Build a Custom Game in JSON](/tutorials/custom-game)
- **Explanation:** [Verdicts & Soundness](/explanation/verdicts-and-soundness) and [Hidden Cards & Fair Hints](/explanation/hidden-cards)
- **Reference:** [Built-in Presets](/reference/presets) · [CLI Reference](/reference/cli)
