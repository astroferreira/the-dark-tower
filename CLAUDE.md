# CLAUDE.md

Procedural world map generator with tectonic plates, erosion, climate, and biomes.

---

## Quick Start

```bash
# Build
cargo build --release

# Run (generates world and launches explorer)
cargo run --release

# With specific seed
cargo run --release -- --seed 42

# Custom map size
cargo run --release -- --width 1024 --height 512
```

---

## CLI Reference

```
planet_generator [OPTIONS]

OPTIONS:
  -W, --width <N>     Map width in tiles [default: 512]
  -H, --height <N>    Map height in tiles [default: 256]
  -s, --seed <N>      Random seed (random if not specified)
  -p, --plates <N>    Number of tectonic plates (random 6-15 if omitted)
```

---

## Explorer Controls

### Navigation
- `Arrow keys / WASD / HJKL` - Move cursor
- `PgUp/PgDn` - Fast vertical movement
- `Home/End` - Fast horizontal movement
- Click - Move cursor to position

### View Modes (press V to cycle)
- **Biome** - Shows biome types with colors
- **Height** - Elevation map
- **Temperature** - Temperature distribution
- **Moisture** - Moisture/precipitation
- **Plates** - Tectonic plate boundaries
- **Stress** - Tectonic stress (mountain building)

### Other
- `?` - Help
- `Q/Esc` - Quit

---

## Module Structure

```
src/
├── main.rs           # CLI entry point
├── explorer.rs       # Terminal UI (ratatui)
├── world.rs          # WorldData structure
├── tilemap.rs        # 2D grid with wrapping
├── heightmap.rs      # Terrain generation
├── climate.rs        # Temperature/moisture
├── biomes.rs         # 50+ biome types
├── water_bodies.rs   # Lakes/rivers/ocean detection
├── scale.rs          # Physical scale (km/tile)
├── ascii.rs          # ASCII rendering utilities
│
├── plates/           # Tectonic plates
│   ├── types.rs      # Plate, PlateType, velocity
│   ├── generation.rs # BFS flood-fill plate generation
│   └── stress.rs     # Boundary stress calculation
│
└── erosion/          # Terrain erosion
    ├── hydraulic.rs  # Water droplet erosion
    ├── glacial.rs    # Ice sheet erosion (SIA)
    ├── rivers.rs     # Flow accumulation
    ├── materials.rs  # Rock hardness
    └── geomorphometry.rs # Terrain analysis
```

---

## World Generation Pipeline

1. **Tectonic Plates** - BFS flood-fill creates 6-15 plates
2. **Plate Stress** - Calculate convergent/divergent boundaries
3. **Heightmap** - Generate terrain from plate interactions
4. **Erosion** - Hydraulic and glacial erosion sculpts terrain
5. **Climate** - Temperature (latitude + elevation) and moisture
6. **Biomes** - 50+ biome types based on climate
7. **Water Bodies** - Detect oceans, lakes, rivers

---

## Key Systems

### Tectonic Plates
- Continental plates: Higher elevation, thicker crust
- Oceanic plates: Lower elevation, thinner crust
- Plate boundaries create mountains (convergent) or rifts (divergent)

### Erosion
- **Hydraulic**: Water droplets carve valleys and deposit sediment
- **Glacial**: Ice sheets using Shallow Ice Approximation (SIA)
- **Rivers**: Flow accumulation creates river channels

### Climate
- Temperature: Decreases with latitude and elevation
- Moisture: Trade winds, rain shadows, ocean proximity
- Creates realistic climate zones

### Biomes (50+ types)
- Ocean biomes: DeepOcean, Ocean, CoastalWater
- Cold biomes: Ice, Tundra, BorealForest, AlpineTundra
- Temperate: Grassland, Forest, Rainforest
- Hot: Desert, Savanna, TropicalForest, TropicalRainforest
- Special: VolcanicWasteland, CrystalDesert, GlowingMarsh, etc.

---

## Output

The generator creates a complete world with:
- Heightmap (elevation in meters)
- Temperature map (Celsius)
- Moisture map (0-1 scale)
- Biome map (50+ types)
- Plate map (tectonic boundaries)
- Water body map (oceans, lakes, rivers)

All data is accessible through the `WorldData` struct for export or further processing.
