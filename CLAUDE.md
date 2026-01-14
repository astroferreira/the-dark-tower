# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
# Build the project
cargo build

# Run with default settings (512x256, random seed)
cargo run

# Run with specific options
cargo run -- --width 1024 --height 512 --seed 42 --output myplanet

# Build release version (optimized)
cargo build --release
```

## CLI Options

```
-W, --width <WIDTH>           Width of tilemap in pixels [default: 512]
-H, --height <HEIGHT>         Height of tilemap in pixels [default: 256]
-s, --seed <SEED>             Random seed (random if not specified)
-o, --output <OUTPUT>         Output file prefix [default: output]
    --stress-spread <N>       Stress spreading iterations [default: 10]
```

## Output Files

- `{output}_plates.png` - Colored plate map (blue=oceanic, green=continental)
- `{output}_heightmap.png` - Grayscale elevation map
- `{output}_stress.png` - Stress visualization (red=convergent, blue=divergent)

## Architecture Overview

2D tilemap-based procedural planet generator using flood-fill tectonic plates and velocity-based stress for heightmaps.

### Module Structure

- **`tilemap.rs`**: Generic 2D grid with horizontal wrapping (equirectangular projection)

- **`plates/`**: Tectonic plate system
  - `types.rs`: `Plate`, `PlateId`, `PlateType` (Ocean/Continental), `Vec2` velocity
  - `generation.rs`: BFS flood-fill from random seeds (6-15 plates)
  - `stress.rs`: Velocity-based border stress calculation and spreading

- **`heightmap.rs`**: Combines plate base elevation + stress into final heightmap

- **`export.rs`**: PNG image export using `image` crate

### Generation Pipeline

```
Seed → Place 6-15 random seed points
     → BFS flood-fill until plates meet
     → Assign ocean/continental types (~60%/40%)
     → Generate random velocity vectors per plate
     → Calculate stress at boundaries (relative velocity dot boundary normal)
     → Spread stress into plate interiors
     → Heightmap = base_elevation + stress * scale
     → Export PNG images
```

### Stress Calculation

At each boundary cell:
1. Find adjacent cells with different plate IDs
2. Calculate boundary normal (direction to neighbor plate)
3. Get relative velocity between plates
4. Stress = -dot(relative_velocity, boundary_normal)
   - Positive (converging) = mountains
   - Negative (diverging) = rifts/trenches


## EVALUATION

Always check the output files to ensure the generated planet is valid and if the request of the user is met. If not, try to fix it.