# What the Answers Mean

## Verdicts

* **`solved`** — A legal winning sequence of moves. Every solution in the test suite is replayed through the checked API.
* **`unsolvable`** — Every distinct reachable position was exhausted without finding a win. Positions are deduplicated by 128-bit hash.
* **`gave-up`** — The search hit `max_states` (default `2,000,000`) before finding a win or exhausting the search space.

## Hidden Cards & Shuffles

* **`solve`** sees face-down cards and knows the deal's `seed` (so shuffled redeals in games like La Belle Lucie or Montana are deterministic). It answers whether the *deal* can be won.
* **`hint`** masks face-down cards and treats future shuffles as unknown, sampling possible arrangements to recommend a move.

## Points Games

* **`best` / `maximize`** returns the highest score found for games like Poker Squares or Cribbage Squares, and whether it is **proven** optimal (all better lines ruled out). Like `solve`, it sees the cards still to come.
