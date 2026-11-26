# Explanation

Background concepts, design trade-offs, and how Dealbreaker works under the hood.

- [Verdicts & Soundness](/explanation/verdicts-and-soundness) — what `solved`, `unsolvable`, and `gave-up` guarantee, and how differential tests prove soundness.
- [Hidden Cards & Fair Hints](/explanation/hidden-cards) — why `solve` sees face-down cards while `hint` samples unseen arrangements.
- [Engine & Search Architecture](/explanation/engine-and-search) — bit-table rules, journal undo, 128-bit canonical hashing, talon compression, and safe autoplay.
- [Scoring & Branch-and-Bound](/explanation/scoring-and-bounds) — how points games compute admissible upper bounds and prove optimal scores.
- [Difficulty Grading](/explanation/difficulty) — how deals are graded easy, medium or hard from solver effort and traps, per-game calibration, and limitations.
- [Benchmarks](/explanation/benchmarks) — FreeCell solver comparisons and 32,000 Microsoft FreeCell results.
- [Stability & SemVer](/explanation/stability) — 1.x API and Rules JSON compatibility guarantees.
