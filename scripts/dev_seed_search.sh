#!/bin/zsh
# Score 96x48 seeds by gazetteer landmarks (no history) to pick the --dev world (DEV_WORLD in
# src/main.rs). Prints the 12 best as "score seed counts"; check the top ones with
# `--dev --seed N --headless --gazetteer`, since the history renames and reshapes features.
# Usage: scripts/dev_seed_search.sh [max_seed]   (after cargo build --release)
B=${0:A:h}/../target/release/planet_generator
for sd in $(seq 1 ${1:-120}); do
  out=$($B --width 96 --height 48 --seed $sd --no-history --headless --gazetteer 2>/dev/null)
  cnt() { echo "$out" | grep -E "^ *$1 x" | sed -E "s/^ *$1 x([0-9]+).*/\1/" | head -1; }
  r=$(cnt River); l=$(cnt Lake); m=$(cnt MountainRange); f=$(cnt Forest); d=$(cnt Desert); i=$(cnt Island); c=$(cnt Continent)
  r=${r:-0}; l=${l:-0}; m=${m:-0}; f=${f:-0}; d=${d:-0}; i=${i:-0}; c=${c:-0}
  score=$(( (r<3?r:3)*3 + (l<1?l:1)*2 + (m<3?m:3) + (f>0?1:0) + (d>0?1:0) + (i>0?1:0) + (c>=2?2:0) ))
  echo "$score $sd rivers=$r lakes=$l ranges=$m forests=$f deserts=$d islands=$i continents=$c"
done | sort -rn | head -12
