# Extend Rules with Rust Hooks

Almost all solitaire games fit cleanly in [Rules JSON](/reference/rules-schema). When you have a one-off rule that doesn't—such as a special custom move restriction or unusual side effect—implement the `dealbreaker::Hooks` trait in Rust and attach it with `Game::with_hooks`.

## Basic Pattern

```rust
use dealbreaker::{presets, solve, Game, Hooks, SolveOptions};

struct MyCustomHooks;

impl Hooks for MyCustomHooks {
    // Override only the hook methods you need;
    // all methods have default no-op implementations.
}

let game = Game::with_hooks(presets::freecell(), MyCustomHooks)?;
let deal = game.deal(1);
let result = solve(&game, &deal, &SolveOptions::default());
```

## Soundness Checklist for Custom Hooks

If your hook changes legality or state transitions:

1. Keep moves deterministic and consistent between legal-move generation and move execution.
2. If your hook inspects interior cards of a pile that normally hashes only its top card, or breaks pile symmetry, test with a small-pack differential test against brute force.
3. Prefer expressing standard mechanics in JSON whenever possible—JSON rules automatically get bit-table compilation, canonical hashing, and safe autoplay.

See the [Rust API on docs.rs ↗](https://docs.rs/dealbreaker) for the full `Hooks` trait signature.
