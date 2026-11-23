# Tune Search Speed & Budgets

Dealbreaker's defaults work well for almost every game, but you can trade memory, speed, and solution length when you need to.

## Choosing a Strategy

| Strategy | Best For | Memory | Proves Unsolvable? |
|---|---|---|---|
| `auto` *(default)* | General use — fast greedy best-first first, then sound depth-first | Medium | Yes |
| `best-first` (`best_first`) | Hard open-information games (FreeCell, Yukon) where heuristic guidance shines | Higher | Yes |
| `dfs` (`depth_first`) | Low-memory environments or proving small deals unsolvable quickly | Lower | Yes |

```sh
dealbreaker solve freecell --ms 11982 --strategy dfs
```

## Faster Solvability Checks (`--no-shorten`)

After finding a win, Dealbreaker normally shortens the solution by cutting out detours. If you only care about *whether* a deal is winnable (for example, generating winnable deals in bulk), disable shortening:

```sh
dealbreaker solve klondike --seed 42 --no-shorten --max-states 200000
```

## Tuning Heuristic Weights (`search`)

Every game accepts optional `search` weights in its Rules JSON (or via `--search '{...}'` on the CLI):

```json
{
  "search": {
    "progress_weight": 6,
    "down_weight": 5,
    "disorder_weight": 2,
    "blocker_weight": 4,
    "cell_weight": 3,
    "stock_weight": 2,
    "fast_share": 0.5
  }
}
```

::: tip Soundness Guarantee
Heuristic weights only change *how fast* a win is found—they **never** change whether a deal is judged solvable or unsolvable.
:::

See [`SearchHints`](/reference/rules-schema#searchhints) in the Rules Schema Reference for every weight.
