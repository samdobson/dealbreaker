# Benchmarks

All benchmarks were run single-threaded on an Apple-silicon laptop using release builds.

## Open-Source FreeCell Solvers (Microsoft Deals #1–#100)

Each solver was given the exact same 100 Microsoft FreeCell deals (`#1..#100`) with a 10-second timeout per deal.

![Open-source FreeCell solvers on Microsoft deals #1–#100](bench-freecell.svg)

* **Fastest mean time & 100% solve rate**: Dealbreaker solves all 100 deals in **20.3 ms** mean per deal (including CLI startup; **13.1 ms** in-process across all 32,000 deals).
* **Outperforms both general and dedicated solvers**: **2.4× faster** mean time than `fc-solve` (default), **34× faster** than `SolverLab`, **85× faster** than `Solvitaire`, and **153× faster** than `cfreecell`.

### Methodology & Versions

* **Dealbreaker**: `cargo build --release` (default options), timed per CLI invocation.
* **fc-solve**: built from `shlomif/fc-solve` `master` with AppleClang `-O3` (default settings), timed per CLI invocation.
* **SolverLab**: built from `jsgrrchg/SolverLab` `main` (`solver-core-rs` in release mode), timed in-process via `FreeCellSolver::solve(&board, 10.0, false)`.
* **Solvitaire**: built from `thecharlieblake/Solvitaire` commit `2398305` with AppleClang `-O3` (`--type free-cell`), timed per CLI invocation.
* **cfreecell**: built from `Julien00859/cfreecell` `master` with AppleClang `-O3 -std=c99`, timed per CLI invocation.

## Microsoft FreeCell (32,000 Deals)

On the full 32,000 Microsoft FreeCell deals (`dealbreaker bench freecell --microsoft --seeds 1..32000 --max-states 500000`, 6 min 58 s total):

* **31,980** solved (99.94%)
* **1** proven unsolvable (`#11982`, in **0.26 s**)
* **19** gave up within the 500k-state budget (every other deal is known to be solvable)
* **112** moves mean solution length
* **13.1 ms** mean time per deal
