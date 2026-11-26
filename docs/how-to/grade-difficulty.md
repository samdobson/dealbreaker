# Grade Deal Difficulty

How to label deals easy, medium or hard — to offer players a difficulty setting, pick a daily deal, or filter out deals that can't be won. For how the grade is measured, see [Difficulty Grading](/explanation/difficulty).

## Grade One Deal

::: code-group

```sh [CLI]
dealbreaker grade freecell --ms 1
# medium (score 10.55: effort 1087 states, 6 of 130 moves tried are traps, 115 steps) (0.134s)

dealbreaker grade klondike --seed 42 --json
```

```rust [Rust]
use dealbreaker::{grade, presets, Game, Grade, GradeOptions};

let game = Game::new(presets::klondike1()).unwrap();
let d = grade(&game, &game.deal(42), &GradeOptions::default());
println!("{} (winnable: {:?}, score: {:?})", d.grade, d.winnable, d.score);
```

```python [Python]
from dealbreaker import Solver

solver = Solver.from_preset("klondike")
d = solver.grade(solver.deal(42))
print(d["grade"], d["winnable"], d.get("score"))
```

```ts [JavaScript / TS]
const solver = Solver.fromPreset("klondike");
const d = solver.grade(solver.deal(42));
console.log(d.grade, d.winnable, d.score);
```

```c [C]
char *json = dealbreaker_solver_grade(solver, state_json, NULL, NULL);
/* {"grade":"medium","winnable":true,"score":10.55,...} */
dealbreaker_string_free(json);
```

:::

The result has the grade and the measurements behind it:

| Field | Meaning |
| --- | --- |
| `grade` | `easy`, `medium` or `hard`. |
| `winnable` | `true` (won), `false` (proven unwinnable), `null` (not won within `max_states`). Only winnable deals can be easy or medium. |
| `score` | The difficulty score that was compared with the thresholds (winnable deals only). |
| `effort` | Distinct positions the solver visited before finding a win. |
| `probes`, `traps`, `trap_rate` | Moves tried along the winning line, and how many of them the solver couldn't recover from. |
| `steps` | Length of the winning line found. |
| `thresholds`, `source` | The cut-offs used, and whether they're the preset's own (`preset`), yours (`custom`), or a fallback for uncalibrated games (`generic`). |

Grading is deterministic: the same deal always gets the same grade. It typically takes 0.01–0.5 s per deal (see the [calibration table](/explanation/difficulty#calibration-results)), and up to a few seconds for deals the solver can't win within the budget.

## Find Deals of a Given Grade

Grade a range of seeds and keep the ones you want. In a game app, do this offline and ship a list of seeds per difficulty:

```sh
dealbreaker grade klondike --seeds 1..500 --json --verbose \
  | jq '[.grades[] | select(.grade == "easy") | .seed]'
```

The summary line shows how the grades spread:

```sh
dealbreaker grade freecell --seeds 1001..1200
# FreeCell: 200 deals: 67 easy (34%), 67 medium (34%), 66 hard (33%; 0 unwinnable, 0 not won in budget); 0.065s/deal
```

## Grade a Custom Game

Built-in thresholds exist for every preset. A game of your own (or a preset with changed rules) falls back to generic thresholds — `"source": "generic"` — which won't spread its deals evenly. Calibrate it once from a sample of deals, then pass the thresholds it prints:

```sh
dealbreaker grade my-game.json --seeds 1..200 --calibrate
# My Game: 200 deals: ...
# calibrated from 180 winnable deals: --thresholds 9.85,12.40

dealbreaker grade my-game.json --seed 7 --thresholds 9.85,12.40
```

Pass the same thresholds from code: `GradeOptions::default().thresholds(GradeThresholds::new(9.85, 12.40))` in Rust, `thresholds=(9.85, 12.40)` in Python, or `{ thresholds: { medium: 9.85, hard: 12.40 } }` in JavaScript and C.

You can also shift a preset's grades the same way, for example to make "easy" stricter.

## Budgets

`GradeOptions` (CLI flags in brackets):

- `max_states` (`--max-states`, default 500,000): budget for finding a win. Deals not won within it are graded hard with `winnable: null`.
- `probe_positions` (`--probe-positions`, default 10) and `probe_states` (`--probe-states`, default 3,000): where moves are tried along the winning line, and the budget for each.

The built-in thresholds were calibrated with these defaults. Effort and trap rate both depend on the budgets, so if you change them, calibrate your own thresholds with the same options.
