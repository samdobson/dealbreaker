# Design

## Goal

One solver for every patience game, where adding a game means writing data, not search code.
Hundreds of traditional games are combinations of a few dozen ideas: what a pile accepts, what
may leave it, how the stock deals, how the game is dealt and won. So the solver is built in two
halves that know nothing of particular games:

1. **A rules model** (`rules.rs`): plain, serializable data. A game is a list of pile groups,
   each with a build rule, an empty rule and a move unit, plus a stock, a deal, an optional
   removal mechanic and a win condition. JSON in, JSON out.
2. **An engine and a search** (`game.rs`, `solver.rs`): compile the rules into lookup tables,
   generate legal moves, play and undo them exactly, and search.

## Engine

* **Cards** are one byte: face (suit × 13 + rank, or 52 for a joker) plus a face-down bit.
  Copies from different packs are deliberately identical, so positions that differ only in which
  copy is where merge.
* **Positions** are piles of cards plus two counters (redeals used, the dealt base rank). Piles
  are numbered across groups in the order the rules list them.
* **Moves are reversible.** Every change is a primitive op (`Shift`, `Turn`, `Flip`, `Redeal`)
  pushed on a journal; undo replays the journal backwards. The search never clones a position
  to try a move.
* **Automatic actions** (turning up exposed cards, refilling from a reserve) run after every move
  as part of it, as the rules say.
* **Rule checks are table lookups.** Each group's build rule is compiled to a 52 × 52 bit table
  (`fits[under] >> over & 1`), so suit, colour, direction, wrap, step and blocked ranks cost one
  shift.
* **Jokers are sets.** A wild joker's identity is the set of faces it could be, worked out from
  the cards beneath: a real card c gives `fits[c]`, and each joker above steps the set on by
  OR-ing the rows of its members. What a pile accepts, whether a run is built through jokers,
  and whether a moved run stays built on its new base are all bit-set walks over the jokers
  involved, so games without jokers pay nothing.

## Search

* **Canonical 128-bit hashing.** Piles in interchangeable groups (free cells, most tableaux,
  foundations) are sorted by hash before combining, so mirror-image positions are searched once.
  Piles that nothing can leave (foundations, discards) hash as top card plus count, since nothing
  below the top can matter again. Each pile keeps prefix hashes, and only the part a move touched
  is rehashed.
* **Talon compression.** When the stock deals to a waste pile, runs of deals are folded into the
  move that plays the card they turn up ("deal 3 times, then play the waste to tableau 4"). This
  is sound because dealing commutes with every move that doesn't involve the waste, and it
  removes most of the depth from Klondike-like games. Solutions are expanded back into single
  deals for display.
* **Safe autoplay.** A card may go to an in-suit foundation automatically when every card that
  could ever be built on it elsewhere is already on a foundation. That's computed from the build
  tables, not hand-written per game. Complete Spider runs and lone Pyramid kings are also forced.
* **Strategies.** Best-first (greedy on a generic heuristic, deeper-first among equals) and
  depth-first (heuristic ordering, transposition table). `Auto`, the default, runs best-first
  with one unsound prune ("don't break a built sequence unless it frees a card for a
  foundation") for half the budget, then a sound depth-first search with the rest. Only sound
  searches report **unsolvable**, and only after exhausting every distinct reachable position.
* **Generic heuristic.** Cards not yet home, face-down cards, cards out of sequence, cards
  sitting on lower cards of their suit, occupied cells, non-empty building piles, and cards still
  in a stock that deals to the tableau.
* **Solution shortening.** After a win is found, any detour (a later position reachable in one
  move from an earlier one) is cut out.

## Scores

Games played for points (Poker Squares, Cribbage Squares) have a score rule: a grid whose rows
and columns are scored as hands (`score.rs`). The solver needs two things from it: a position's exact score and
an upper bound on the score of any position reachable from it.

* **The bound** treats each line on its own: given its cards, how many more it can take and the
  cards that can still arrive, which hands could it still make? Each hand is tested alone (a
  flush needs five of a suit among held and arriving cards, a full house a cheap three and a
  cheap pair of another rank), using rank bit masks for straights. When cards only arrive one by
  one from the waste, the arriving cards are known exactly: the waste card and the next ones in
  the stock. It is never too low, which a unit test checks against every completion of random
  lines.
* **Cribbage** bounds fifteens, pairs and runs together, and flush and nobs separately. Those
  rank points never fall when a card is added, so a table of the best rank points reachable from
  every multiset of up to five ranks (8,568 of them, computed when the game is compiled) bounds a
  line with three or more cards to come. With one or two to come, every choice of ranks the
  arriving cards (and starter) could have is tried. The starter counts only once the grid is
  full, so a position's score never exceeds a full grid's that extends it, and the bound covers
  the unfinished positions a score target might stop at. The starter is known exactly when it's
  the next waste card after the grid fills, or sits on a pile nothing touches.
* **Jokers** in hands: the exact score tries each joker as every card, except in poker with
  points that follow the hands' ranking, where the relaxed test with the joker as a free card
  is exact. The bound treats held jokers as free cards that need no slot, and arriving jokers as
  cards of any rank and suit.
* **Symmetry.** When the grid's piles are alike, permuting rows or columns gives an equally good
  position, so a card going into an empty row is only tried in the first empty row (and the
  same for columns).
* **Maximizing** (`maximize`) is depth-first branch and bound, children best bound first. By
  default it runs as limited discrepancy search: starting from the greedy line, each round may
  stray from the best-bound move by a total that doubles (taking the i-th child costs i). That
  finds high scores far sooner than plain depth-first, which commits to its first choices, or
  best-first, which drowns in shallow positions with loose bounds. A round that never hits the
  discrepancy limit has covered everything, so its best is proven.
* **Score targets** (`win: {score: …}`) use the same limited discrepancy search, stopping at the
  first position that reaches the target; a position whose bound is below it is dead. A round
  that never hits its limit proves the target out of reach.
* **Hints** in scored games try every distinct next step on every guess at the hidden cards and
  recommend the best average score.

## Hidden cards

The solver sees face-down cards: it answers "can this deal be won?" (the "thoughtful" variant).
For in-game advice that mustn't leak hidden cards, `hint()` replaces every face-down card with a
random arrangement of the unseen cards, solves each guess, and recommends the first step that wins
most often.

## How it's tested

* **Differential tests** (`tests/differential.rs`): small versions of every mechanic, from
  draw-1 and draw-3 talons to grid redeals, neighbour pairing, rows that close up and a hooked
  game, are solved by brute force (no pruning of any kind, exact position equality) and by all
  three strategies. The verdicts must match exactly, on 40 deals each, plus every ordering of a
  tiny Accordion pack. This is what makes "unsolvable" trustworthy. Jokers get the same
  treatment in eight of these games; scored games compare best scores and target verdicts.
* **Rule tests** (`tests/jokers.rs`): differential tests only prove the pruning is faithful to
  the rules, so what a joker may do is checked move by move on hand-built positions.
* **Scoring properties**: cribbage scores against a slow reference written from the
  definitions, joker hands against trying every card, and both bounds against every completion
  of random lines.
* **Replay**: every solution is replayed through the fully checked public API (`Game::play`),
  validating the position after each move, and must end in a win.
* **Known results**: Microsoft FreeCell deal #1's layout, and #11982 proven unsolvable.
* Presets round-trip through JSON; positions round-trip; bad rules and positions are rejected.

## What the model covers, and what's next

Covered now, as data:

* **Building**: by suit, colour, alternate colour, other suit, suit-or-rank or any; up, down,
  either way, same rank or any rank; wrapping, steps, blocked ranks; limits by suit or colour.
* **Piles**: tableaux, foundations, cells, reserves, stock and waste, with per-pile overrides
  (foundations with their own bases and steps); empty piles that take anything, nothing, a rank,
  the dealt base, or the card after their left neighbour; move units from single cards to
  sequences, suit sequences, any face-up group, whole piles and FreeCell supermoves; whole-suit
  foundations.
* **Layouts**: overlapping piles (pyramids, peaks, any covering graph), grids, rows that close
  up, and piles that only accept from a fixed distance away.
* **Stock and refills**: stocks dealing to a waste (with or without redeals), to foundations,
  across the tableau or into the first gap; dealing by itself or only into an empty waste;
  emptied piles refilled from reserves, the waste or the stock.
* **Redeals**: regrouping in order, shuffling, Montana's grid redeal, Monte Carlo's
  consolidation.
* **Removal**: pairs to a total, same-rank pairs, rank sets, lower-of-suit, optionally only
  between grid neighbours.
* **Jokers**: wild cards standing for any card that would be legal where they lie, in building,
  on empty piles, in runs, on foundations (opt-in), in removals and in scored hands.
* **Winning**: all cards home, groups emptied, a count reached, one pile left, rows in sequence,
  a score reached.
* **Scoring**: rows and columns of a grid scored as poker hands (American, English or custom
  points) or as cribbage hands with a starter, with a search for the highest score.
* **Packs**: several packs, stripped packs, aces high, jokers.
* **Hooks**: a Rust trait for one-off rules, checked by the same differential tests.

The presets show each of these on a real game. Still to add, roughly in order of how many
traditional games they unlock:

1. **More scoring layouts**: diagonals, and scored hands that aren't grid lines (a separate
   hand and crib).
2. **Joker variants** where traditional rules differ from the one model here: a joker that
   keeps the identity it took when placed, or one that can be swapped for the card it stands
   for.
3. **Smaller pieces** as they turn up: more fill and auto-play patterns, unusual deals, more
   ways to win. Each should become a data field once two games need it; a hook covers the
   first.

Search strength is the other axis. The hardest presets (Forty Thieves, Calculation, Sir Tommy,
Accordion, Gaps, two- and four-suit Spider, and the best scores in Poker and Cribbage Squares,
where the bound treats each line as if it had the arriving cards to itself) gain little from
heuristic weights alone: tuning the `search` weights, on one set of deals and checked on another,
helped only four-suit Spider, Spiderette and Aces Up. The rest need search changes: game-family
heuristics that weights can't express (which card a stock-and-reserve game should hold back),
better unsolvability proofs (dominance pruning between transpositions), and multithreading for
native use.

## Soundness notes

The differential tests have caught three ways a pruning rule can quietly break with a new
mechanic. Each is now handled, and each is a pattern to check when adding the next mechanic:

* A shortcut phase must never report "unsolvable": only a sound search can.
* With unlimited redeals, the redeal count must not be part of the position, or the state space
  becomes infinite.
* "Safe" foundation plays aren't safe when the cards left on the table decide something else:
  how a tableau redeal regroups or shuffles, or which card refills an emptied pile.

Scoring added a fourth check, built in from the start rather than caught: a pile nothing leaves
hashes as its top card and count, which is wrong once every card in it counts towards a score.
Scored piles hash in full, and autoplay is off in scored games, since a foundation play can take
a card a line wanted.

Jokers added a fifth: with a joker about, no move is free of consequences. A card no real card
could build on is still somewhere a joker could stand, and a lone Pyramid king might be the last
partner a joker has. So safe autoplay and forced moves are off when the pack has jokers.
