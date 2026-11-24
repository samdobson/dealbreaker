# Contributing to Dealbreaker

Thanks for your interest in contributing to Dealbreaker!

## Development Setup

- **Rust**: 1.82+ (MSRV is checked in CI).
- **Node.js** (optional): 22+ for the JS/TS package (`js/`) and VitePress documentation (`docs/`).
- **Python** (optional): 3.10+ with `maturin` and `pytest` for `crates/dealbreaker-py`.
- **C compiler** (optional): `cc` for the C ABI smoke test in `crates/dealbreaker-ffi`.

## Running Tests & Checks

Always run the solver test suite in **release mode** — the brute-force differential tests in `crates/dealbreaker/tests/differential.rs` search hundreds of thousands of positions and are much faster optimized:

```sh
cargo test --workspace --release
```

To run a single differential test quickly:

```sh
cargo test --release --test differential -- freecell_alternate
```

Run the local CI mirror before opening a pull request:

```sh
./scripts/check-ci.sh
```

Run the reproducible solver benchmark suite:

```sh
cargo bench -p dealbreaker --bench solver
```

## Adding New Rule Mechanics

Dealbreaker's `"unsolvable"` verdicts are proofs. Any PR that adds or modifies a rule mechanic **must**:

1. Keep `Rules`, `GroupRules`, `StockRules`, and other public rule types **1.x SemVer-compatible** (all new fields must have backwards-compatible defaults; public structs/enums remain `#[non_exhaustive]`).
2. Add a small-pack brute-force differential test in `crates/dealbreaker/tests/differential.rs` verifying that `Auto`, `BestFirst`, and `DepthFirst` agree with exhaustive search across mixed solvable and unsolvable deals.
3. Regenerate JSON Schemas and reference docs if public rules, options, or CLI flags changed:
   ```sh
   ./scripts/gen-docs.sh
   ```
4. Update TypeScript definitions in `js/src/types.d.ts` if the JSON schema changed.

## Commit Style

This repository uses [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` new rule mechanics, CLI subcommands, or API additions
- `fix:` bug fixes and soundness fixes
- `refactor:` code structure improvements with no behavior change
- `docs:` documentation, examples, and schema updates
- `test:` test additions or improvements
- `ci:` / `chore:` CI, build, and maintenance changes

## Licensing

By contributing to Dealbreaker, you agree that your contributions will be dual-licensed under the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at the user's option.
