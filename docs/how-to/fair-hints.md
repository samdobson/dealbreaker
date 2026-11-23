# Configure Fair Hints & Cancellation

How to get responsive, non-cheating hints in interactive games and cancel long searches cleanly.

## Passing Unknown Face-Down Cards

When calling `hint`, Dealbreaker automatically hides face-down cards (written with `~`, like `"~5H"`). If your client doesn't even know what the face-down cards are—for example, in a server-authoritative game—write them as `"?"` (or `"~?"`):

```json
{
  "piles": {
    "tableau": [["~?", "~?", "KS"], ["QD"]],
    "foundation": [[], [], [], []]
  }
}
```

Dealbreaker infers the unseen cards by subtracting all visible cards from the game's pack and randomly samples valid arrangements.

## Tuning Hint Quality vs. Latency

Hint runtime scales roughly with `samples × max_states`:

- **Snappy mobile/web UI (~50–200 ms):** `samples: 8`–`16`, `max_states: 50_000`–`100_000`.
- **Stronger advice:** `samples: 32`, `max_states: 250_000`.
- **Deterministic hints for tests:** pass a fixed `seed` in `HintOptions`.

::: code-group

```ts [JavaScript / TS]
const hint = await solver.hint(state, {
  samples: 16,
  seed: 1,
  solve: { max_states: 100_000 },
});
```

```python [Python]
hint = solver.hint(state, samples=16, seed=1, max_states=100_000, timeout_ms=500)
```

```rust [Rust]
use dealbreaker::{hint, HintOptions, SolveOptions};

let opts = HintOptions::default()
    .samples(16)
    .seed(1)
    .solve(SolveOptions::default().max_states(100_000).timeout_ms(500));
let suggestion = hint(&game, &state, &opts);
```

:::

## Timeouts & Cooperative Cancellation

- **Wall-clock timeout (native Rust, Python, CLI, C ABI):** set `timeout_ms` in `SolveOptions` (or `--timeout-ms` on the CLI).
- **Rust cancellation:** pass an `AtomicBool` to `solve_with_cancel`, `hint_with_cancel`, or `maximize_with_cancel`.
- **C ABI cancellation:** create a `DealbreakerCancel *` with `dealbreaker_cancel_new()` and call `dealbreaker_cancel_trigger()` from another thread.
- **Browser cancellation:** use `WorkerSolver` (`dealbreaker/worker`) and call `solver.terminate()`.

See [Hidden Cards & Fair Hints](/explanation/hidden-cards) for how sampling works under the hood.
