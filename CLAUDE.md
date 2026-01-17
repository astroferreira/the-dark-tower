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
-v, --view                    Open interactive viewer

# Erosion Options
    --erosion                 Enable erosion simulation
    --erosion-iterations <N>  Number of hydraulic droplets [default: 50000]
    --hydraulic <BOOL>        Enable hydraulic erosion [default: true]
    --glacial <BOOL>          Enable glacial erosion [default: true]
    --glacial-timesteps <N>   Glacial simulation steps [default: 500]
```

### Erosion Examples

```bash
# Run with both hydraulic and glacial erosion (512x256 default)
cargo run --release -- --erosion --seed 42 --output eroded

# For larger maps, increase iterations proportionally:
# 512x256 (default): 200k iterations
# 1024x512: 500k-1M iterations
# 2048x1024: 2M+ iterations
cargo run --release -- -W 1024 -H 512 --erosion --erosion-iterations 500000

# Hydraulic erosion only (faster)
cargo run --release -- --erosion --glacial false

# Glacial erosion only
cargo run --release -- --erosion --hydraulic false
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

- **`climate.rs`**: Temperature and moisture maps, biome classification

- **`erosion/`**: Erosion simulation system
  - `mod.rs`: Main orchestration and shared types
  - `params.rs`: Erosion configuration parameters
  - `utils.rs`: Gradient calculation, bilinear interpolation utilities
  - `materials.rs`: Rock types with hardness values (Basalt, Granite, etc.)
  - `hydraulic.rs`: Particle-based water droplet erosion
  - `glacial.rs`: Shallow Ice Approximation (SIA) glacial erosion

- **`export.rs`**: PNG image export using `image` crate

- **`viewer.rs`**: Interactive OpenGL viewer

### Generation Pipeline

```
Seed → Place 6-15 random seed points
     → BFS flood-fill until plates meet
     → Assign ocean/continental types (~60%/40%)
     → Generate random velocity vectors per plate
     → Calculate stress at boundaries (relative velocity dot boundary normal)
     → Spread stress into plate interiors
     → Heightmap = base_elevation + stress * scale
     → Generate climate (temperature, moisture)
     → [Optional] Run erosion simulation:
       → Generate material map (rock types by region)
       → Hydraulic: Simulate water droplets carving valleys
       → Glacial: Simulate ice sheets using SIA model
     → Generate biomes from climate + elevation
     → Export PNG images
```

### Erosion System

**Hydraulic Erosion** - Particle-based water droplet simulation:
- Droplets spawn randomly, follow terrain gradient
- Pick up sediment on steep slopes (modulated by rock hardness)
- Deposit sediment when flow slows
- Creates V-shaped river valleys, alluvial fans

**Glacial Erosion** - Shallow Ice Approximation (SIA):
- Ice accumulates above snowline (cold temperatures)
- Ice flows downhill following SIA flux equation
- Erodes bedrock based on basal sliding velocity
- Creates U-shaped valleys, cirques, fjords

**Rock Hardness** - Different erosion rates:
- Basalt (0.95): Very hard, volcanic rock
- Granite (0.85): Hard continental basement
- Sandstone (0.5): Medium, sedimentary
- Limestone (0.4): Medium-soft, karst-forming
- Shale (0.25): Soft sedimentary
- Sediment (0.1): Unconsolidated deposits

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