<p align="center">
  <img src="docs/dealbreaker-logo.png" alt="Dealbreaker" width="520">
</p>

<p align="center">
  <strong>A fast patience (solitaire) solver written in Rust, where every game is data.</strong>
</p>

<p align="center">
  <a href="https://github.com/samdobson/dealbreaker/actions/workflows/ci.yml"><img src="https://github.com/samdobson/dealbreaker/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/dealbreaker"><img src="https://img.shields.io/crates/v/dealbreaker.svg" alt="crates.io"></a>
  <a href="https://docs.rs/dealbreaker"><img src="https://docs.rs/dealbreaker/badge.svg" alt="docs.rs"></a>
  <a href="https://pypi.org/project/dealbreaker/"><img src="https://img.shields.io/pypi/v/dealbreaker.svg" alt="PyPI"></a>
  <a href="https://www.npmjs.com/package/dealbreaker"><img src="https://img.shields.io/npm/v/dealbreaker.svg" alt="npm"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License: MIT OR Apache-2.0"></a>
</p>

<p align="center">
  <a href="https://samdobson.github.io/dealbreaker/"><strong>Documentation</strong></a> &nbsp;·&nbsp;
  <a href="https://samdobson.github.io/dealbreaker/demo/"><strong>Live Web Demo</strong></a> &nbsp;·&nbsp;
  <a href="https://github.com/samdobson/dealbreaker/releases"><strong>Releases</strong></a>
</p>

---

Dealbreaker models solitaire variants as small JSON files and solves them with a single engine — finding winning lines, proving unsolvability, suggesting fair hints, and maximising scores in points games.

- **39 built-in games** — Klondike, FreeCell, Spider, Yukon, TriPeaks, Poker Squares, and [more](https://samdobson.github.io/dealbreaker/reference/presets), plus custom games in JSON.
- **Runs everywhere** — CLI, Rust crate, JavaScript/TypeScript (WebAssembly), Python, and C ABI.
- **Fast** — solves 31,980 of 32,000 Microsoft FreeCell deals in 13.1 ms mean time, proving `#11982` unsolvable in 0.26 s ([benchmarks](https://samdobson.github.io/dealbreaker/explanation/benchmarks)).

## Quick start

**CLI** (or download a pre-built binary from [Releases](https://github.com/samdobson/dealbreaker/releases)):

```sh
cargo binstall dealbreaker-cli   # or: cargo install dealbreaker-cli
dealbreaker solve freecell --ms 1
```

**Docker**:

```sh
docker run --rm ghcr.io/samdobson/dealbreaker solve freecell --ms 1
```

**Python** (`pip install dealbreaker`):

```python
from dealbreaker import Solver

solver = Solver.from_preset("freecell")
result = solver.solve(solver.deal_microsoft(1))
print(result["outcome"], len(result["steps"]), "steps")
```

**JavaScript / TypeScript** (`npm install dealbreaker`):

```js
import { Solver } from "dealbreaker";

const solver = Solver.fromPreset("freecell");
const result = solver.solve(solver.dealMicrosoft(1));
console.log(result.outcome, result.steps.length, "steps");
solver.free();
```

See the **[documentation](https://samdobson.github.io/dealbreaker/)** for Rust, C/Swift/Kotlin, Web Workers, and custom JSON games, or try it directly in your browser with the **[live web demo](https://samdobson.github.io/dealbreaker/demo/)**.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
