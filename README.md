<p align="center">
  <img src="docs/dealbreaker-logo.png" alt="Dealbreaker" width="600">
</p>

# Dealbreaker

A fast patience (solitaire) solver written in Rust, where **every game is data**.

Instead of hard-coding rules into a search engine, Dealbreaker models solitaire games as small JSON files and solves them all with the same engine. Give it a deal and it will find a winning line, prove no win exists, suggest a fair hint, or find the highest score in points games.

## Highlights

* **39 built-in games** — Klondike, FreeCell, Spider, Yukon, TriPeaks, Poker Squares, and [33 more](docs/games.md).
* **Custom games in JSON** — describe new variants without touching Rust; see the [rules format](docs/rules-format.md).
* **Use it anywhere** — ships as a CLI, a Rust crate, and a ~600 KB dependency-free WebAssembly module for JavaScript/TypeScript.
* **Fast** — solves 31,980 of the 32,000 Microsoft FreeCell deals in 13.1 ms mean time (proving #11982 unsolvable in 0.26 s).

## Benchmarks

Open-source FreeCell solvers on Microsoft deals `#1..#100` (single-threaded, 10 s timeout per deal):

![Open-source FreeCell solvers on Microsoft deals #1–#100](docs/bench-freecell.svg)

Across all **32,000 Microsoft FreeCell deals** (`--max-states 500000`), Dealbreaker solves **31,980** (99.94%) and proves `#11982` unsolvable in **0.26 s** at **13.1 ms** mean time per deal. See [benchmarks](docs/benchmarks.md) for methodology.

## Quick start

Install the CLI from [crates.io](https://crates.io/crates/dealbreaker-cli):

```sh
cargo install dealbreaker-cli
```

Solve a deal:

```
$ dealbreaker solve freecell --ms 1
   ...
   1. tableau 4 -> cell 1
   2. tableau 4 -> foundation 1
   ...
solved in 112 steps (1330 states, 0.003s)
```

Try a few more commands:

```sh
dealbreaker list                         # list all 39 presets
dealbreaker solve klondike --seed 42     # solve a seeded Klondike deal
dealbreaker hint klondike --seed 2       # get a hint without peeking at face-down cards
dealbreaker best poker-squares --seed 1  # maximize score in a points game
```

## Use it from Rust

```rust
use dealbreaker::{presets, solve, Game, Outcome, SolveOptions};

let game = Game::new(presets::klondike1())?;
let deal = game.deal(42);
let result = solve(&game, &deal, &SolveOptions::default());

if let Outcome::Solved(moves) = result.outcome {
    for step in game.steps(&deal, &moves) {
        println!("{step}");
    }
}
```

Dealbreaker also runs in browsers, Node, Deno, and edge workers via WebAssembly. See the [library guide](docs/library.md) for Rust and JavaScript/TypeScript examples.

## Documentation

* [Built-in games](docs/games.md) — all 39 presets and how to customize them
* [Command-line reference](docs/cli.md) — every command, flag, and replay option
* [Rust & JavaScript/TypeScript guide](docs/library.md) — library APIs, WASM build, and SemVer stability
* [What the answers mean](docs/outcomes.md) — verdicts, hidden cards, shuffles, and points games
* [Custom rules format](docs/rules-format.md) — writing new games in JSON
* [Benchmarks](docs/benchmarks.md) — FreeCell comparison methodology and 32,000 MS FreeCell deals
* [Architecture & design](DESIGN.md) — how the engine works and what's next
* [Changelog](CHANGELOG.md) — release history

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
