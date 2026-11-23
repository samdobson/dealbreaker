# Command-Line Reference

Download a pre-built binary for macOS, Linux, or Windows from [GitHub Releases](https://github.com/samdobson/dealbreaker/releases), or install from [crates.io](https://crates.io/crates/dealbreaker-cli):

```sh
cargo install dealbreaker-cli
```

## Quick Examples

```sh
dealbreaker list                           # list all 39 presets
dealbreaker solve freecell --ms 1          # solve Microsoft FreeCell deal #1
dealbreaker solve klondike --seed 42       # solve a seeded Klondike deal
dealbreaker solve klondike --seed 1 --json # machine-readable output
dealbreaker hint klondike --seed 2         # fair hint (no peeking at hidden cards)
dealbreaker best poker-squares --seed 1    # highest score in a points game
```

## Commands

| Command | Purpose |
|---|---|
| `dealbreaker list` | List all built-in presets |
| `dealbreaker rules <GAME>` | Print a preset's JSON rules |
| `dealbreaker deal <GAME> [DEAL]` | Deal a position as JSON |
| `dealbreaker solve <GAME> [DEAL] [SEARCH] [--json]` | Find a winning line or prove unsolvability |
| `dealbreaker hint <GAME> [DEAL] [SEARCH] [--samples N]` | Suggest a move without peeking at hidden cards |
| `dealbreaker replay <GAME> [DEAL] --steps FILE [--legal] [--json]` | Replay steps and print each resulting position (`--legal` includes all legal moves) |
| `dealbreaker best <GAME> [DEAL] [SEARCH] [--json]` | Maximize score in a points game |
| `dealbreaker bench <GAME> --seeds A..B [--microsoft] [--best] [SEARCH] [--json] [--verbose]` | Benchmark a range of deals |
| `dealbreaker serve <GAME> [--search JSON]` | Long-lived JSONL stdio server (`stdin` requests -> `stdout` responses) |

`<GAME>` is a [preset id](games.md) or a path to a [rules JSON file](rules-format.md).

**Deal flags** (default `--seed 1`): `--seed N`, `--ms N` (Microsoft FreeCell numbering), or `--state FILE` (position JSON).

**Search flags**: `--max-states N` (default `2,000,000`), `--timeout-ms N` (wall-clock limit), `--strategy auto|dfs|best-first` (default `auto`), `--no-autoplay`, `--no-shorten`, `--search JSON`.

## JSONL Stdio Mode (`serve`)

`dealbreaker serve <GAME>` compiles the game once and processes one JSON request per line on `stdin`, writing one JSON response line per request to `stdout` (echoing any optional `"id"` field):

```json
{"id": 1, "cmd": "deal", "ms": 1}
{"id": 2, "cmd": "solve", "state": {...}, "options": {"max_states": 500000, "timeout_ms": 500}}
{"id": 3, "cmd": "hint", "state": {...}, "options": {"samples": 8}}
{"id": 4, "cmd": "legal_steps", "state": {...}}
{"id": 5, "cmd": "play_step", "state": {...}, "step": {...}}
{"id": 6, "cmd": "quit"}
```

Supported commands: `rules`, `deal` (`seed` | `ms` | `cards`), `validate`, `solve`, `best`, `hint`, `legal_steps`, `play_step`, `is_won`, `score`, `quit`. Every response has `"ok": true` (plus command-specific fields) or `"ok": false, "error": "..."`.

See [What the answers mean](outcomes.md) for verdicts, hidden-card handling, and points games.
