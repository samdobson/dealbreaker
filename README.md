<p align="center">
  <img src="docs/dealbreaker-logo.png" alt="Dealbreaker" width="520">
</p>

<p align="center">
  <strong>A fast patience (solitaire) solver written in Rust, where every game is data.</strong>
</p>

<p align="center">
  <a href="https://samdobson.github.io/dealbreaker/"><strong>Documentation</strong></a> &nbsp;·&nbsp;
  <a href="https://samdobson.github.io/dealbreaker/demo/"><strong>Live Web Demo</strong></a> &nbsp;·&nbsp;
  <a href="https://github.com/samdobson/dealbreaker/releases"><strong>Releases</strong></a>
</p>

---

Dealbreaker models solitaire variants as small JSON files and solves them with a single engine—finding winning lines, proving unsolvability, suggesting fair hints, and maximizing scores in points games.

- **39 built-in games** — Klondike, FreeCell, Spider, Yukon, TriPeaks, Poker Squares, and [more](https://samdobson.github.io/dealbreaker/reference/presets), plus custom games in JSON.
- **Runs everywhere** — CLI, Rust crate, JavaScript/TypeScript (WebAssembly), Python, and C ABI.
- **Fast** — solves 31,980 of 32,000 Microsoft FreeCell deals in 13.1 ms mean time, proving `#11982` unsolvable in 0.26 s ([benchmarks](https://samdobson.github.io/dealbreaker/explanation/benchmarks)).

## Quick start

```sh
cargo install dealbreaker-cli
dealbreaker solve freecell --ms 1
```

See the **[documentation](https://samdobson.github.io/dealbreaker/)** for pre-built binaries, Rust, JavaScript/WASM, Python, and C usage, or try it directly in your browser with the **[live web demo](https://samdobson.github.io/dealbreaker/demo/)**.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
