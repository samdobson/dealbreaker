# CLI & Scripting

Practical patterns for automating Dealbreaker from the shell or driving it from another language over stdio.

## Machine-Readable Output (`--json`)

Add `--json` to `solve`, `best`, `replay`, or `bench` to get clean JSON on `stdout`:

```sh
dealbreaker solve freecell --ms 1 --json
```

Save a deal to a file, inspect it, or pass a custom position back in:

```sh
dealbreaker deal klondike --seed 42 > pos.json
dealbreaker solve klondike --state pos.json --json
```

## Replay a Solution Step-by-Step

Feed the output of `solve --json` into `replay` to inspect every intermediate board state (add `--legal` to include every legal move at each step):

```sh
dealbreaker solve freecell --ms 1 --json > sol.json
dealbreaker replay freecell --ms 1 --steps sol.json --legal --json
```

## Benchmark a Range of Deals

`bench` runs in parallel across all CPU cores by default (use `--jobs 1` for single-threaded timing):

```sh
# 100 Microsoft FreeCell deals
dealbreaker bench freecell --microsoft --seeds 1..100 --max-states 500000

# 200 Klondike deals with per-deal details
dealbreaker bench klondike --seeds 1..200 --verbose --json
```

## Drive Dealbreaker from Any Process (`serve`)

If you're solving many positions from a script or backend service, `dealbreaker serve <GAME>` compiles the game once and speaks newline-delimited JSON (JSONL) over `stdin` / `stdout`:

```sh
dealbreaker serve freecell <<'EOF'
{"id": 1, "cmd": "deal", "ms": 1}
{"id": 2, "cmd": "quit"}
EOF
```

Every request may include an optional `"id"` field that is echoed in the response. See the [CLI Reference](/reference/cli#jsonl-stdio-server-dealbreaker-serve) for all supported `serve` commands.

## Run in Docker

Pre-built multi-arch (`linux/amd64`, `linux/arm64`) `scratch` images are published to GitHub Container Registry on every release:

```sh
docker run --rm ghcr.io/samdobson/dealbreaker solve freecell --ms 1
```

Mount your working directory to pass custom rules or state JSON files:

```sh
docker run --rm -v "$PWD:/work" -w /work ghcr.io/samdobson/dealbreaker solve my-game.json --state pos.json --json
```

Pass `-i` to keep `stdin` open when driving `dealbreaker serve`:

```sh
docker run --rm -i ghcr.io/samdobson/dealbreaker serve freecell <<'EOF'
{"id": 1, "cmd": "deal", "ms": 1}
{"id": 2, "cmd": "quit"}
EOF
```

To build the image locally from source:

```sh
docker build -t dealbreaker .
```
