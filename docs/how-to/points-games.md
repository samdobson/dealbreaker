# Solve Points Games

Games like **Poker Squares** and **Cribbage Squares** aren't just win/loss—you're trying to build the highest-scoring grid. Dealbreaker's `best` / `maximize` API finds the highest score and tells you whether it's **proven optimal**.

## Find the Highest Score

::: code-group

```sh [CLI]
dealbreaker best poker-squares --seed 1
dealbreaker best cribbage-squares --seed 1 --json
```

```ts [JavaScript / TS]
const solver = Solver.fromPreset("poker-squares");
const state = solver.deal(1);
const result = solver.best(state, { max_states: 500_000 });
// { score: 245, proven: true, states: ..., steps: [...] }
```

```python [Python]
solver = Solver.from_preset("poker-squares")
state = solver.deal(1)
result = solver.best(state, max_states=500_000)
print(result["score"], "proven?" , result["proven"])
```

```rust [Rust]
use dealbreaker::{maximize, presets, Game, SolveOptions};

let game = Game::new(presets::poker_squares())?;
let deal = game.deal(1);
let result = maximize(&game, &deal, &SolveOptions::default());
println!("Best score: {} (proven: {})", result.score, result.proven);
```

:::

## Check the Current Position's Score

Use `score()` to evaluate any in-progress or completed grid (returns `None` / `null` for non-scored games):

```ts
const currentScore = solver.score(state);
```

## Target a Specific Score (`solve` vs. `best`)

If your custom game defines `"win": { "score": { "at_least": 200 } }`, then regular `solve` stops as soon as it finds **any** line reaching 200+ points (much faster than proving the global maximum with `best`).

## Benchmark Points Games

Pass `--best` to `dealbreaker bench` to report average and proven-optimal scores across a range of deals:

```sh
dealbreaker bench poker-squares --seeds 1..20 --best --max-states 200000
```

See [Scoring & Branch-and-Bound](/explanation/scoring-and-bounds) for how Dealbreaker proves optimality.
