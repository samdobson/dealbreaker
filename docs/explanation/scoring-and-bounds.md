# Scoring & Branch-and-Bound

Points games like **Poker Squares** and **Cribbage Squares** have a `score` rule that evaluates every row and column of a grid as hands. To find and *prove* the highest possible score, Dealbreaker needs two things:

1. **Exact score** for any position.
2. **An admissible upper bound** on the highest score reachable from an incomplete position (guaranteed never to underestimate).

## Admissible Upper Bounds

* **Poker Squares:** Each row and column is bounded independently using the cards already in the line, its remaining empty slots, and the exact cards still to arrive from the stock/waste. Rank bitmasks test possible straights, flushes, full houses, and multiples without enumerating every completion.
* **Cribbage Squares:** Because fifteens, pairs, and runs never lose points when another card is added, Dealbreaker precomputes a table of the best rank points reachable from every multiset of up to 5 ranks (8,568 entries) when compiling the game, and combines it with flush/nobs bounds.
* **Grid symmetry:** When all grid piles share the same rules, permuting empty rows or empty columns produces equivalent positions—so a card placed into an empty row is only tried in the first empty row (and likewise for columns).

## Limited Discrepancy Search (LDS)

`best` / `maximize` runs depth-first branch-and-bound with **limited discrepancy search**:

* Children are ordered by best upper bound first.
* Each round starts from the greedy line and allows a doubling budget of deviations from the best-bound choice.
* Any branch whose upper bound cannot beat the best score found so far is pruned immediately.
* When a round completes without hitting the discrepancy limit, every better line has been ruled out—so `proven` is `true`.

See [Solve Points Games](/how-to/points-games) for CLI and library usage.
