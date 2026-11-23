# How Answers Work

Dealbreaker gives you clear, trustworthy verdicts so you always know whether a result is proven or hit a search limit.

## Verdicts

* **`solved`** — Found a legal winning sequence of moves. Every solution in the test suite is replayed through the checked move API.
* **`unsolvable`** — Exhausted every distinct reachable position without finding a win. Positions are deduplicated by 128-bit hash, so this is a real proof (not a guess).
* **`gave-up`** — Hit the search budget (`max_states`, default `2,000,000`, or `timeout_ms`) before finding a win or exhausting the search space. Try raising `--max-states` if you want a deeper search.

## Hidden Cards & Shuffles (`solve` vs. `hint`)

* **`solve`** sees face-down cards and knows the deal's `seed` (so shuffled redeals in games like La Belle Lucie or Montana are deterministic). It answers: *"Is this deal winnable?"*
* **`hint`** plays fair—it masks face-down cards and treats future shuffles as unknown, sampling possible arrangements to recommend the strongest move.

## Points Games (`best`)

* **`best` / `maximize`** returns the highest score found for games like Poker Squares or Cribbage Squares, along with whether it is **proven** optimal (every better line ruled out). Like `solve`, it sees the cards still to come.
