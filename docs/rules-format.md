# Rules Format

Games are defined as JSON (or via the Rust types in `rules.rs`). Run `dealbreaker rules <preset>` to inspect any built-in game.

```json
{
  "name": "FreeCell",
  "groups": [
    { "name": "cell", "kind": "cell", "count": 4 },
    { "name": "foundation", "kind": "foundation", "count": 4 },
    { "name": "tableau", "kind": "tableau", "count": 8,
      "build": { "by": "alternate", "dir": "down" }, "moves": "supermove" }
  ],
  "deal": [ { "to": "tableau", "rest": true } ]
}
```

## Top-Level Fields

| Field | Default | Meaning |
|---|---|---|
| `name` | required | Display name |
| `pack` | 1 French pack | `{ "copies", "suits", "ranks", "jokers" }` (`ranks` order defines sequence; e.g. `[2..13, 1]` for aces high) |
| `groups` | required | Ordered list of pile groups (sets pile numbering) |
| `stock` | none | Stock dealing rules (requires a `stock` group; stocks without this only feed refills) |
| `redeal` | none | Player-triggered tableau redeal (not used with `stock`) |
| `deal` | `[]` | Opening deal steps (must deal every card in the pack) |
| `removal` | none | Card-removal rules (object or list) |
| `score` | none | Grid scoring rules (`poker` or `cribbage`) |
| `win` | `"all_on_foundations"` | Win condition |
| `auto_flip` | `true` | Auto-flip exposed face-down top cards |
| `safe_autoplay` | `true` | Auto-play provably safe cards to in-suit foundations |
| `search` | neutral | Heuristic weights for speed (never affects verdicts) |

## Groups

| Field | Default | Meaning |
|---|---|---|
| `name` | required | Group identifier used by `deal`, `accepts_from`, states, and solutions |
| `kind` | required | `tableau`, `foundation`, `cell`, `reserve`, `stock`, or `waste` |
| `count` | `1` | Number of piles (`stock` and `waste` must be `1`) |
| `build` | by kind | Rule for building on a non-empty pile |
| `empty` | by kind | Empty-pile rule: `"any"`, `"none"`, `{"rank": 13}`, `{"ranks": [1, 13]}`, `"dealt_base"`, `{"dealt_base_offset": -1}`, or `{"after_left": {"first": 2}}` |
| `moves` | by kind | Movable unit: `"none"`, `"top"`, `"run"` (built sequence), `"suit_run"` (Spider), `"any_face_up"` (Yukon), `"supermove"` (FreeCell), `"whole"`, `"top_or_whole"` |
| `max_cards` | `1` for cells | Maximum cards per pile |
| `max_accept` / `empty_max_accept` | `1` for foundations | Max cards accepted in one move (total, or on an empty pile) |
| `accepts_from` | any | List of source group names |
| `refill_from` | none | Group name(s) that auto-refill an emptied pile (`refill_largest` picks largest-base source; `refill_on_empty` refills only when empty) |
| `layout` | none | `{"pyramid": {"rows": 7}}`, `"tri_peaks"`, `{"covers": {"covered_by": [[...]]}}`, or `{"grid": {"columns": 13}}` |
| `complete_runs` | `false` | Foundation accepts only a full suit sequence at once (Spider) |
| `suit`, `empty_suit`, `color` | none | Restrict pile to a suit/colour (`empty_suit` restricts only the first card on an empty pile) |
| `reach` | none | Accept only from piles in this group at these forward offsets (e.g. `[1, 3]` for Accordion) |
| `compact` / `compact_face_up` | `false` | Slide piles forward to close gaps (`compact_face_up` shifts only face-up cards) |
| `requires_rank_on_prev` / `match_longest_rank` | `false` | Require matching rank on previous pile / longest run |
| `accept_only_when_empty` / `accept_when_stock_empty` | `false` | Gate acceptance on target being empty / stock being empty |
| `wild` | all except foundations | Whether jokers are wild on this group |
| `piles` | none | Per-pile overrides (`{ "piles": [1], "build": ... }`; disables group symmetry) |

**Kind defaults:** `tableau` (`none` / `any` / `top`), `foundation` (in suit, up / pack start or end / `none`), `cell` (`none` / `any` / `top`), `reserve` (`none` / `none` / `top`), `waste` (`none` / `none` / `top`), `stock` (`none` / `none` / `none`).

### `build`

```json
{ "by": "alternate", "dir": "down", "wrap": false, "step": 1, "blocked_ranks": [] }
```

* `by`: `none`, `any`, `suit`, `alternate`, `same_color`, `other_suit`, `suit_or_rank`
* `dir`: `down`, `up`, `either`, `same`, `any`
* `wrap`: King and Ace are adjacent (`false` by default)
* `step`: rank distance between consecutive cards (`1` by default)
* `blocked_ranks`: ranks that cannot be built upon (e.g. `[13]` in Golf)

## Stock & Redeals

**`stock`**: `{ "to": "waste", "count": 3, "redeals": 2, "unlimited_redeals": false }`
Deals `count` cards to `waste`/`foundation`, or 1 per pile to `tableau` (or `count` to first empty with `"first_empty": true`). Flags: `require_no_empty`, `only_when_empty`, `auto`, `skip_empty`, `face_up`.

**`redeal`**: `{ "group": "tableau", "times": 2, "unlimited": false, "how": ... }`
`how` is one of `{"regroup": {"per_pile": 4}}`, `{"shuffle": {"per_pile": 3}}`, `"grid_shuffle"` (Montana), or `{"compact": {"refill_from": "stock"}}` (Monte Carlo). Shuffles use the position's `seed`; `solve` knows future shuffles, `hint` treats them as unknown.

## Deal Steps

Each step in `deal` takes cards from the top of the shuffled pack:

| Field | Meaning |
|---|---|
| `to` | Target group name |
| `piles` | Pile indices within the group (default: all) |
| `count` / `counts` | Cards per pile (dealt in rows) |
| `rest` | Deal all remaining cards round-robin |
| `ranks` | Pull all cards of these ranks from the pack (`one_each`: first of each rank) |
| `face_up` | Default `true` |
| `kings_to_bottom` | Move each pile's kings to the bottom (Baker's Dozen) |
| `set_base` / `of_base` | First card sets `dealt_base` / pull all remaining cards of `dealt_base` |
| `collect` | After dealing, move all cards of these ranks from the table to this group |

## Removals, Jokers & Winning

**`removal`**: `{ "kind": ..., "from": ["tableau", "waste"], "to": "discard", "adjacent": "eight" | "four" }`
* `{"pairs_sum": {"total": 13, "alone": [13]}}` — pairs summing to `total`, or single `alone` ranks
* `"same_rank"` — two playable cards of equal rank
* `{"rank_set": {"ranks": [11, 12, 13]}}` — one of each listed rank
* `"lower_of_suit"` — lower rank when a higher card of the same suit is playable

**Jokers** (`"jokers": N` in `pack`, `"JK"` / `"~JK"` in positions): On `wild` piles, a joker stands for any card legal in its spot (dynamically). On non-wild piles, it can only sit on an empty `"any"` pile. Jokers are wild in `pairs_sum`, `same_rank`, and `rank_set` removals, and are excluded from `all_on_foundations`.

**`win`**: `"all_on_foundations"` (default), `{"empty": ["tableau"]}`, `{"count": {"group": "discard", "cards": 48}}`, `{"at_most_piles": {"group": "tableau", "piles": 1}}`, `{"rows": {"group": "tableau", "length": 12}}`, or `{"score": {"at_least": 200}}`.

## Scoring (Points Games)

`score` evaluates every row and column of a `grid` group:
* **Poker**: `{ "group": "tableau", "hands": { "poker": "american" | "english" | { ... } } }` — scores each line's highest poker hand (straights/flushes require 5 cards; pairs/threes/fours score in partial lines).
* **Cribbage**: `{ "group": "tableau", "hands": { "cribbage": { "starter": "waste", "points": { ... } } } }` — scores fifteens, pairs, runs, flushes, nobs, and his heels (starter joins once the grid is full).
* **Jokers in hands**: Each joker independently takes the card that maximizes that line's score.

## Search Hints, Hooks & I/O

* **`search`**: Tune heuristic weights (`progress_weight`, `down_weight`, `disorder_weight`, `blocker_weight`, `cell_weight`, `stock_weight`, `fast_share`). Override via CLI with `--search '{...}'`.
* **`Hooks`**: Implement `dealbreaker::Hooks` (`Game::with_hooks`) in Rust for rules that don't fit JSON.
* **Positions**: `{"piles": {"tableau": [["~5H", "KS"]]}, "redeals_used": 0, "base_rank": null, "seed": 0}` (bottom-first; `~` = face-down).
* **Solutions**: Array of `move`, `deal`, `redeal`, and `remove` steps (auto-flips and refills happen automatically; `supermove` steps move a whole run at once).
