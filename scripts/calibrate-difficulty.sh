#!/usr/bin/env bash
# Recalibrate the built-in difficulty thresholds (`CALIBRATION` in
# crates/dealbreaker/src/difficulty.rs): grade deals 1001..1200 of every preset with the default
# grading options and print one table row per preset (cut-offs at the 1/3 and 2/3 quantiles of
# the winnable deals' scores), plus the spread of grades. Games where few deals are won get
# deals 1001..2000 instead, so their cut-offs rest on more winnable deals. Accordion is left
# out: almost no deal is won within the default budget, so it uses the generic thresholds.
#
# Usage: scripts/calibrate-difficulty.sh [PRESET...]
# Takes about an hour on 4 cores. Grading is deterministic, so a rerun with unchanged solver
# and options reproduces the table exactly.
set -euo pipefail
cd "$(dirname "$0")/.."

# Games where fewer than ~60 of 200 deals are won.
big=" elevens russian nestor fortress scorpion golf sir-tommy forty-thieves spider "
cargo build --release -q -p dealbreaker-cli
bin=target/release/dealbreaker
presets=("$@")
if [ ${#presets[@]} -eq 0 ]; then
  mapfile -t presets < <("$bin" list | awk '$1 != "accordion" {print $1}')
fi

summary=""
for id in "${presets[@]}"; do
  seeds=1001..1200
  if [[ "$big" == *" $id "* ]]; then seeds=1001..2000; fi
  out=$("$bin" grade "$id" --seeds "$seeds" --calibrate --json)
  row=$(python3 -c '
import json, sys
t = json.loads(sys.argv[1])["thresholds"]
print("    (\"%s\", %.2f, %.2f)," % (sys.argv[2], t["medium"], t["hard"]))
' "$out" "$id")
  echo "$row"
  summary+=$(python3 -c '
import json, sys
d = json.loads(sys.argv[1])
won = d["deals"] - d["unwinnable"] - d["unknown"]
print("| %s | %d | %d | %d | %d | %d / %d / %d |" % (sys.argv[2], d["deals"], won,
      d["unwinnable"], d["unknown"], d["easy"], d["medium"], d["hard"]))
' "$out" "$id")$'\n'
done
echo
echo "| Preset | Deals | Won | Unwinnable | Not won in budget | Easy / medium / hard |"
echo "| --- | ---: | ---: | ---: | ---: | --- |"
printf "%s" "$summary"
