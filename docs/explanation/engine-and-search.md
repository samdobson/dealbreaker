# Engine & Search Architecture

Dealbreaker is built in two halves that know nothing about specific games:

1. **A rules model** — plain, serializable JSON describing pile groups, build rules, stocks, deals, removals, scoring, and win conditions.
2. **A compiled engine and search** — compiles any rules file into compact lookup tables and runs the same fast search over all of them.

## Compact, Reversible Engine

* **One-byte cards:** Each card is a single byte (face `0..51` or `52` for a joker, plus a face-down bit). Copies from multi-pack decks are identical, so equivalent positions merge automatically.
* **Bit-table rule checks:** Each group's `build` rule compiles to a `52 × 52` bit table (`fits[under] >> over & 1`), turning suit, color, direction, wrap, step, and blocked-rank checks into a single bit shift.
* **Journaled undo:** Moves push tiny primitive ops (`Shift`, `Turn`, `Flip`, `Redeal`) onto a journal. Undoing a move replays the journal backwards—the search never clones a board state to explore a branch.
* **Set-valued jokers:** A wild joker's possible faces are tracked as a 52-bit set derived from the cards beneath it, so games without jokers pay zero overhead.

## Fast, Sound Search

* **Canonical 128-bit hashing:** Interchangeable piles (like free cells or identical tableau columns) are sorted by hash before combining, so mirror-image states are visited once. Write-only piles (foundations, discards) hash only their top card and count. Each pile maintains prefix hashes so only touched piles are rehashed.
* **Talon compression:** When a stock deals to a waste pile, runs of deals are folded into the move that plays the turned-up card (*"deal 3 times, then play waste → tableau 4"*). Dealing commutes with non-waste moves, collapsing depth dramatically; winning lines are expanded back into individual deals for display.
* **Safe autoplay:** A card is played to an in-suit foundation automatically when the compiled build tables prove no remaining card could ever need to be built on it.
* **Solution shortening:** Once a winning line is found, any detour (a later state reachable in one move from an earlier one) is snipped out.
