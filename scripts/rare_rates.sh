#!/bin/sh
# Measure how often each rare outcome (lore/rare.rs) appears over dev seeds 1..N (default 100):
# prints "Kind count" per kind. Put the counts into RATES in lore/rare.rs.
N=${1:-100}
BIN=./target/release/planet_generator
for s in $(seq 1 $N); do
  $BIN --dev --seed $s --headless 2>&1 | grep '^Rare:' | sed 's/.*(\(.*\))/\1/' | tr ',' '\n' | sed 's/^ *//'
done | grep -v '^$' | sort | uniq -c
