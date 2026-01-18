#!/bin/bash
# Generate erosion comparison grid
# Creates multiple outputs with different erosion parameters for visual comparison

SEED=42
WIDTH=512
HEIGHT=256

echo "Generating erosion comparison with seed $SEED..."

# Build release first
cargo build --release

# 1. No erosion (baseline)
echo "1/9: Baseline (no erosion)..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_01_baseline

# 2. Rivers only - low strength
echo "2/9: Rivers only - low strength..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_02_rivers_low --erosion --no-hydraulic --no-glacial

# 3. Rivers only - high strength (current settings)
echo "3/9: Rivers only - high strength..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_03_rivers_high --erosion --no-hydraulic --no-glacial

# 4. Hydraulic only - few iterations
echo "4/9: Hydraulic only - 50k iterations..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_04_hydraulic_50k --erosion --no-rivers --no-glacial --erosion-iterations 50000

# 5. Hydraulic only - many iterations
echo "5/9: Hydraulic only - 200k iterations..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_05_hydraulic_200k --erosion --no-rivers --no-glacial --erosion-iterations 200000

# 6. Rivers + Hydraulic combined
echo "6/9: Rivers + Hydraulic combined..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_06_rivers_hydraulic --erosion --no-glacial --erosion-iterations 100000

# 7. All erosion types
echo "7/9: All erosion (rivers + hydraulic + glacial)..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_07_all_erosion --erosion --erosion-iterations 100000

# 8. Glacial only
echo "8/9: Glacial only..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_08_glacial_only --erosion --no-rivers --no-hydraulic

# 9. High iteration hydraulic
echo "9/9: Hydraulic 500k iterations..."
cargo run --release -- -W $WIDTH -H $HEIGHT --seed $SEED --output debug_09_hydraulic_500k --erosion --no-rivers --no-glacial --erosion-iterations 500000

echo "Done! Generated debug_01 through debug_09 PNG files."
echo "Compare visually to see erosion effects."
