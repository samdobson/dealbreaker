# Difficulty Grading

[`grade`](/how-to/grade-difficulty) labels a deal easy, medium or hard, for any game, including games described only as data. This page explains what it measures, how the cut-offs were chosen, and what the grades do and don't mean.

## What Is Measured

There is no game-specific knowledge in the grade. Everything comes from the solver working on the deal, in two parts.

**Effort.** The solver's default search tries the moves that look like progress first and backs up only when that fails. On a deal where steady, obvious play wins, it finds a win almost at once. On a deal that needs a long detour, an early sacrifice, or a precise order of moves, it has to back up and look wider. *Effort* is the number of distinct positions it visited before finding a win.

**Traps.** Effort says how hard a winning line is to find; it doesn't say how forgiving the deal is once you're playing. So the grader also walks the winning line and, at 10 positions spread evenly along it (the deal itself first), tries *every* legal move. After each one, the solver gets a small budget (3,000 positions) to win from where that move leads. A move it can't win from is a *trap*: either the position is lost, or getting back on track takes more search than a careful player could do in their head. The *trap rate* is the share of moves tried that were traps.

The two are combined into one score:

```text
score = log2(1 + effort) + 10 × trap rate
```

Effort is taken on a log scale, so each doubling of effort adds 1. A trap rate of 10% adds as much as doubling the effort. Across the presets, the trap term accounts for a minority of the spread of scores (effort varies far more between deals), so it mostly separates deals with similar effort. The two signals mostly agree: across winnable deals their rank correlation is positive in every calibrated game except Elevens and Gaps, and mostly between 0.3 and 0.8.

## Unwinnable Deals

A deal the solver proves can't be won is graded **hard**, with `winnable: false`. A deal it can't win within the budget (500,000 positions by default) is also graded **hard**, with `winnable: null`: it may be winnable, but if so, it's hard to find.

The alternative was a fourth grade (`unwinnable`), but the task was a three-way grade, and an app that offers "easy" deals wants winnable ones. Check `winnable` when you need to tell the cases apart, for example to never deal an unwinnable game.

## Calibration

A raw score means different things in different games: a FreeCell deal is searched in hundreds of positions, a Spider deal in tens of thousands. So each preset has its own cut-offs. They were set by grading 200 deals of each preset (seeds 1001–1200; 1,000 deals, seeds 1001–2000, for games where few deals are won) and cutting at the 1/3 and 2/3 quantiles of the **winnable** deals' scores, so about a third of a game's winnable deals fall in each grade. The unwinnable and unknown deals are all hard, so games with many of those have more hard deals overall.

`scripts/calibrate-difficulty.sh` regenerates the table. Grading is deterministic, so with the same solver and options it reproduces the built-in thresholds exactly. The table below is the result: the spread of grades is over the same deals the cut-offs came from.

Accordion has no thresholds of its own: only 1 of 200 deals was won within the default budget, so it uses the generic thresholds (the median of the other presets') and nearly every deal grades hard.

### Calibration Results

| Game | Deals | Won | Unwinnable | Not won in budget | Easy / medium / hard | Cut-offs | Wall time per deal (4 threads) |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |
| `klondike` | 200 | 176 | 2 | 22 | 59 / 59 / 82 | 9.10 / 10.84 | 0.15 s |
| `klondike-draw3` | 200 | 161 | 18 | 21 | 54 / 54 / 92 | 9.24 / 11.17 | 0.12 s |
| `klondike-vegas` | 200 | 120 | 47 | 33 | 40 / 40 / 120 | 13.33 / 15.95 | 0.14 s |
| `klondike-jokers` | 200 | 200 | 0 | 0 | 67 / 67 / 66 | 10.98 / 12.04 | 0.07 s |
| `freecell` | 200 | 200 | 0 | 0 | 67 / 67 / 66 | 9.24 / 10.88 | 0.09 s |
| `bakers-game` | 200 | 151 | 49 | 0 | 51 / 50 / 99 | 9.53 / 11.13 | 0.02 s |
| `eight-off` | 200 | 200 | 0 | 0 | 67 / 66 / 67 | 8.37 / 10.50 | 0.04 s |
| `seahaven-towers` | 200 | 177 | 21 | 2 | 59 / 59 / 82 | 9.84 / 13.12 | 0.07 s |
| `spider` | 1000 | 201 | 0 | 799 | 67 / 67 / 866 | 22.28 / 24.95 | 0.44 s |
| `spider-2suit` | 200 | 178 | 0 | 22 | 61 / 58 / 81 | 14.74 / 18.83 | 0.45 s |
| `spider-1suit` | 200 | 188 | 0 | 12 | 63 / 63 / 74 | 9.75 / 10.93 | 0.23 s |
| `spiderette` | 200 | 162 | 2 | 36 | 54 / 54 / 92 | 16.96 / 21.18 | 0.22 s |
| `scorpion` | 1000 | 106 | 735 | 159 | 36 / 35 / 929 | 8.10 / 9.92 | 0.13 s |
| `yukon` | 200 | 179 | 6 | 15 | 58 / 62 / 80 | 8.28 / 10.66 | 0.08 s |
| `russian` | 1000 | 90 | 910 | 0 | 30 / 29 / 941 | 7.23 / 7.56 | 0.00 s |
| `forty-thieves` | 1000 | 290 | 2 | 708 | 97 / 97 / 806 | 15.91 / 19.43 | 0.46 s |
| `gypsy` | 200 | 157 | 0 | 43 | 53 / 52 / 95 | 12.03 / 18.00 | 0.30 s |
| `bakers-dozen` | 200 | 137 | 0 | 63 | 46 / 46 / 108 | 11.53 / 19.08 | 0.25 s |
| `beleaguered-castle` | 200 | 128 | 52 | 20 | 43 / 43 / 114 | 12.84 / 19.10 | 0.08 s |
| `streets-and-alleys` | 200 | 73 | 94 | 33 | 25 / 24 / 151 | 15.24 / 20.73 | 0.11 s |
| `fortress` | 1000 | 213 | 787 | 0 | 72 / 70 / 858 | 9.56 / 10.15 | 0.00 s |
| `canfield` | 200 | 170 | 15 | 15 | 58 / 56 / 86 | 8.78 / 10.88 | 0.07 s |
| `golf` | 1000 | 191 | 453 | 356 | 64 / 64 / 872 | 21.79 / 24.17 | 0.06 s |
| `tripeaks` | 200 | 189 | 6 | 5 | 64 / 62 / 74 | 6.60 / 11.30 | 0.02 s |
| `pyramid` | 200 | 125 | 47 | 28 | 42 / 42 / 116 | 7.14 / 9.79 | 0.22 s |
| `aces-up` | 200 | 97 | 91 | 12 | 33 / 32 / 135 | 13.45 / 18.36 | 0.02 s |
| `calculation` | 200 | 96 | 0 | 104 | 32 / 32 / 136 | 16.61 / 21.45 | 0.12 s |
| `sir-tommy` | 1000 | 246 | 0 | 754 | 82 / 82 / 836 | 18.13 / 21.01 | 0.13 s |
| `bisley` | 200 | 128 | 10 | 62 | 43 / 43 / 114 | 9.87 / 16.07 | 0.24 s |
| `elevens` | 1000 | 103 | 897 | 0 | 36 / 34 / 930 | 6.07 / 6.25 | 0.00 s |
| `nestor` | 1000 | 159 | 841 | 0 | 54 / 52 / 894 | 9.99 / 12.36 | 0.00 s |
| `gaps-single-deal` | 200 | 54 | 5 | 141 | 18 / 18 / 164 | 21.39 / 23.12 | 2.25 s |
| `montana` | 200 | 200 | 0 | 0 | 67 / 68 / 65 | 18.07 / 20.90 | 0.44 s |
| `cruel` | 200 | 184 | 2 | 14 | 62 / 60 / 78 | 8.41 / 8.92 | 0.05 s |
| `monte-carlo` | 200 | 173 | 25 | 2 | 58 / 58 / 84 | 6.87 / 7.61 | 0.01 s |
| `accordion` | 200 | 1 | 0 | 199 | 0 / 0 / 200 | generic | 1.84 s |
| `la-belle-lucie` | 200 | 150 | 0 | 50 | 50 / 50 / 100 | 13.46 / 20.89 | 0.34 s |
| `poker-squares` | 200 | 101 | 5 | 94 | 34 / 34 / 132 | 16.37 / 23.97 | 2.09 s |
| `cribbage-squares` | 200 | 187 | 0 | 13 | 63 / 62 / 75 | 7.53 / 14.05 | 0.40 s |

## Limitations

- **It measures the deal, not the player's view.** Like `solve`, the grader sees face-down cards. In Klondike, Spider or Yukon, a deal graded easy may still need some luck when you can't see the cards. Use [`hint`](/explanation/hidden-cards) to study play without peeking.
- **It is solver difficulty, used as a proxy.** Effort and traps track what makes deals hard for people (few good moves, long plans, unforgiving mistakes), but the solver's heuristics are not a human. Grades are best used as ordered buckets, not as an absolute scale, and nothing here has been validated against human play data.
- **Grades are relative within a game.** "Easy" means easy *for that game*: an easy Spider (4 suits) deal is still much harder than an easy Klondike deal.
- **Small samples where wins are rare.** In games where few deals are won within the budget, the cut-offs rest on few winnable deals (see the table), so they are rougher.
- **Budgets matter.** Thresholds are valid for the default `GradeOptions`. Changing `max_states` changes how the solver splits its budget between phases, which changes effort for deals it doesn't win quickly; calibrate again if you change the options.
- **Solver changes may move grades.** A faster or smarter solver changes effort. Recalibrate when search heuristics change; the regression tests check that every preset has thresholds, not that a particular deal keeps its grade.
