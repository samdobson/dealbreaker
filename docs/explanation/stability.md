# Stability & SemVer (1.x)

Dealbreaker follows **Semantic Versioning** across its Rust crate, CLI, npm package, Python wheels, and JSON formats:

* **JSON Rules compatibility:** Any valid 1.0 Rules JSON file remains valid and keeps the same semantics across all 1.x releases.
* **Rust `#[non_exhaustive]` types:** Public rule structs and enums are marked `#[non_exhaustive]` so new fields and variants can be added in minor releases without breaking callers. Construct them via constructors (`Rules::new`, `GroupRules::new`, `Build::new`) or JSON, and include a wildcard arm when matching rule enums.
* **Verdict stability:** Heuristic improvements and search tuning in minor/patch releases may change solution paths or speed, never whether a deal is judged solvable or unsolvable.
* **Version parity:** `Cargo.toml`, `js/package.json`, and `crates/dealbreaker-py/pyproject.toml` always share the same version number, verified in CI.

See [`CHANGELOG.md`](https://github.com/samdobson/dealbreaker/blob/main/CHANGELOG.md) for the full release history.
