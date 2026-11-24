# Build a Custom Game in JSON

Dealbreaker's core idea is that **every game is data**. In this tutorial, you'll create a brand-new solitaire variant in a small JSON file—no Rust required—and immediately solve deals with it.

## 1. Start with a Minimal Game

Let's build **Mini FreeCell**: a snappy, half-deck FreeCell variant with 2 suits, 2 free cells, and 4 tableau columns.

Create a file called `mini-freecell.json`:

```json
{
  "$schema": "https://raw.githubusercontent.com/samdobson/dealbreaker/main/docs/schemas/rules.schema.json",
  "name": "Mini FreeCell",
  "pack": {
    "suits": ["hearts", "spades"],
    "ranks": [1, 2, 3, 4, 5, 6]
  },
  "groups": [
    { "name": "cell", "kind": "cell", "count": 2 },
    { "name": "foundation", "kind": "foundation", "count": 2 },
    {
      "name": "tableau",
      "kind": "tableau",
      "count": 4,
      "build": { "by": "alternate", "dir": "down" },
      "moves": "supermove"
    }
  ],
  "deal": [
    { "to": "tableau", "rest": true }
  ]
}
```

::: tip Editor Autocomplete
That `"$schema"` line gives you live autocomplete, hover docs, and inline validation in VS Code and JetBrains IDEs. The solver ignores it at runtime.
:::

## 2. Solve a Deal

Pass the file path anywhere Dealbreaker accepts a preset name:

```sh
dealbreaker solve ./mini-freecell.json --seed 1
```

You can also benchmark 50 seeds to see how winnable your new variant is:

```sh
dealbreaker bench ./mini-freecell.json --seeds 1..50
```

## 3. Tweak a Rule

What if tableau piles build **by suit** instead of alternating colors (like Baker's Game)? Change one line in `mini-freecell.json`:

```json
"build": { "by": "suit", "dir": "down" }
```

Re-run the benchmark and watch the win rate drop:

```sh
dealbreaker bench ./mini-freecell.json --seeds 1..50
```

## 4. Borrow from Any Built-in Preset

The fastest way to design a larger game is to start from a built-in preset and modify it:

```sh
dealbreaker rules klondike > my-klondike.json
```

Edit `my-klondike.json`—for example, change `"redeals"` or add `"jokers": 2` to `"pack"`—and pass `./my-klondike.json` to `dealbreaker solve`, `hint`, or `new Solver(rulesJson)` in JS/Python/Rust.

## Where to Go Next

- **Reference:** [Rules Schema Reference](/reference/rules-schema) — every field, enum, and default
- **Reference:** [Built-in Presets](/reference/presets) — inspect the JSON of all 50 presets for inspiration
- **How-to:** [Custom Rust Hooks](/how-to/custom-rust-hooks) — for exotic rules that don't fit JSON
