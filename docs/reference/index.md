# Reference

Technical specifications for Dealbreaker's rules, data formats, CLI, and language bindings. Every page here is **auto-generated** from the codebase and verified in CI.

## Games & Data Formats

- [Built-in Presets](/reference/presets) — all 39 presets with summaries and full JSON rules
- [Rules Schema](/reference/rules-schema) — complete JSON Schema reference for defining games
- [Data Formats](/reference/data-formats) — JSON schemas for positions (`StateData`), moves (`Step`), `SolveOptions`, and `HintOptions`

## CLI & Language APIs

- [CLI Reference](/reference/cli) — every command, flag, default, and the `serve` JSONL stdio protocol
- [JavaScript / TypeScript](/reference/javascript) — `Solver`, `WorkerSolver`, and TypeScript types (`dealbreaker` npm package)
- [Python](/reference/python) — `Solver` and `presets()` (`dealbreaker` PyPI package)
- [C ABI](/reference/c-abi) — C functions, opaque handles, and thread-safe cancellation (`dealbreaker.h`)
- [Rust API (docs.rs) ↗](https://docs.rs/dealbreaker) — full `rustdoc` reference for the `dealbreaker` crate
