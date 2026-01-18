# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

> **IMPORTANT: Any new feature MUST be documented in this file.**

---

## Quick Start

```bash
# Build
cargo build --release

# Launch interactive explorer (recommended)
cargo run --release -- -v

# Generate world with full pipeline
cargo run --release -- --seed 42 -o output/world

# Explorer with full world generation
cargo run --release -- --seed 42 --explore
```

---

## CLI Reference

```
planet_generator [OPTIONS]

BASIC:
  -W, --width <N>              Map width [default: 512]
  -H, --height <N>             Map height [default: 256]
  -s, --seed <N>               Random seed
  -o, --output <PREFIX>        Output file prefix [default: output]
  -p, --plates <N>             Number of plates (random 6-15 if omitted)

INTERACTIVE:
  -v, --view                   Quick launch explorer
  --explore                    Full world generation + explorer

EROSION:
  --no-erosion                 Skip erosion simulation
  --erosion-iterations <N>     Hydraulic droplets [default: 200000]
  --no-rivers                  Disable river erosion
  --no-hydraulic               Disable hydraulic erosion
  --no-glacial                 Disable glacial erosion
  --glacial-timesteps <N>      Glacial steps [default: 500]
  --analyze                    Enable terrain analysis
  --histogram                  Print height histogram

SIMULATION:
  --simulate                   Run headless simulation
  --sim-ticks <N>              Ticks to run [default: 100]
  --sim-tribes <N>             Initial tribes [default: 10]
  --sim-population <N>         Pop per tribe [default: 100]
  --sim-seed <N>               Simulation seed

LORE:
  --lore                       Enable lore generation
  --lore-wanderers <N>         Storytellers [default: 5]
  --lore-steps <N>             Steps per wanderer [default: 5000]
  --lore-output <PREFIX>       Lore output prefix [default: lore]
  --lore-seed <N>              Lore seed

LLM (requires --lore):
  --llm                        Use LLM for stories
  --llm-url <URL>              LLM server URL
  --llm-model <NAME>           Model name
  --llm-max-tokens <N>         Max tokens [default: 1024]
  --llm-temperature <F>        Temperature [default: 0.8]

IMAGES (requires --lore):
  --images                     Generate story images
  --image-url <URL>            Image server URL
  --max-images <N>             Max images [default: 10]

LOCAL MAP:
  --local-map <X,Y>            Generate local map at tile
  --local-size <N>             Local map size [default: 64]

EXPORT:
  --ascii-export <PATH>        Export ASCII world file
  --ascii-png <PATH>           Export ASCII as PNG
  --verbose                    Verbose ASCII export
```

---

## Output Directory

All outputs go to `output/` (git-ignored):

```bash
cargo run --release -- --seed 42 -o output/myworld
```

---

## Explorer Controls

### Navigation
- `Arrow keys / WASD / HJKL` - Move cursor
- `PgUp/PgDn` - Fast vertical
- `Home/End` - Fast horizontal
- `C` - Center on cursor
- `V` - Cycle view mode

### Simulation
- `Shift+S` - Start/stop simulation
- `Space` - Step (paused) / Pause (running)
- `+/-` - Change speed
- `T` - Toggle territories
- `Shift+L` - Toggle combat log

### Other
- `Enter` - View local map
- `N` - New world
- `?` - Help
- `Q/Esc` - Quit

---

## Module Structure

```
src/
├── main.rs              # CLI entry point
├── explorer.rs          # Terminal UI (ratatui)
├── world.rs             # WorldData structure
├── tilemap.rs           # 2D grid with wrapping
├── heightmap.rs         # Terrain generation
├── climate.rs           # Temperature/moisture
├── biomes.rs            # 50+ biome types
├── water_bodies.rs      # Lakes/rivers/ocean
├── scale.rs             # Physical scale (km/tile)
├── ascii.rs             # ASCII export
├── export.rs            # PNG export
│
├── plates/              # Tectonic plates
│   ├── types.rs         # Plate, PlateType, velocity
│   ├── generation.rs    # BFS flood-fill
│   └── stress.rs        # Boundary stress
│
├── erosion/             # Terrain erosion
│   ├── hydraulic.rs     # Water droplet erosion
│   ├── glacial.rs       # Ice sheet erosion (SIA)
│   ├── rivers.rs        # Flow accumulation
│   ├── materials.rs     # Rock hardness
│   ├── gpu.rs           # GPU acceleration
│   └── geomorphometry.rs # Terrain analysis
│
├── local/               # Local map generation
│   ├── generation.rs    # Procedural detail
│   ├── terrain.rs       # Terrain types
│   ├── biome_features.rs # Feature placement
│   └── export.rs        # Local map export
│
├── lore/                # Story generation
│   ├── wanderer.rs      # Storyteller simulation
│   ├── landmarks.rs     # Landmark discovery
│   ├── encounters.rs    # Event generation
│   ├── mythology.rs     # Myth generation
│   ├── llm.rs           # LLM client
│   └── image_gen.rs     # Image generation
│
└── simulation/          # Civilization sim
    ├── simulation.rs    # Main tick loop
    ├── types.rs         # TribeId, TileCoord, etc.
    ├── params.rs        # Configuration
    │
    ├── tribe/           # Tribe system
    │   ├── mod.rs       # Tribe struct
    │   ├── population.rs # Demographics
    │   ├── needs.rs     # Need satisfaction
    │   └── culture.rs   # Cultural traits
    │
    ├── technology/      # Tech progression
    │   ├── ages.rs      # Stone→Iron→Industrial
    │   └── unlocks.rs   # Tech bonuses
    │
    ├── resources/       # Resource system
    │   ├── stockpile.rs # Storage
    │   └── extraction.rs # Gathering
    │
    ├── territory/       # Land control
    │   └── expansion.rs # Territory growth
    │
    ├── interaction/     # Inter-tribe
    │   ├── diplomacy.rs # Relations
    │   ├── trade.rs     # Trade routes
    │   ├── conflict.rs  # Warfare
    │   └── migration.rs # Population movement
    │
    ├── monsters/        # Monster system
    │   ├── types.rs     # 14 species
    │   ├── spawning.rs  # Biome spawning
    │   ├── behavior.rs  # AI behavior
    │   └── combat.rs    # Combat resolution
    │
    ├── body/            # Body part system
    │   ├── parts.rs     # BodyPart, tissues
    │   ├── templates.rs # Humanoid, Dragon, etc.
    │   └── wounds.rs    # Wound types
    │
    ├── characters/      # Character system
    │   ├── types.rs     # Character, Attributes
    │   └── equipment.rs # Weapons, Armor
    │
    ├── combat/          # Combat system
    │   ├── resolution.rs # Attack resolution
    │   ├── damage.rs    # Damage calculation
    │   └── log.rs       # Combat logging
    │
    ├── roads.rs         # Road network
    ├── structures.rs    # Buildings
    └── export.rs        # JSON export
```

---

## Key Systems

### World Generation
1. Generate tectonic plates (BFS flood-fill)
2. Calculate plate stress at boundaries
3. Generate heightmap from stress
4. Apply erosion (hydraulic, glacial, rivers)
5. Generate climate (temp, moisture)
6. Assign biomes (50+ types)
7. Detect water bodies

### Simulation (4 ticks = 1 year)
- Population growth/mortality
- Resource extraction
- Territory expansion
- Technology progression
- Diplomacy and trade
- Warfare and raids
- Monster spawning and combat

### Combat (Dwarf Fortress-style)
- Body parts: Head, Torso, Limbs, etc.
- Tissues: Flesh, Bone, Scale, Chitin
- Wounds: Cut, Fracture, Severed, etc.
- Detailed combat narratives

### Monster Species (14)
Wolf, Bear, IceWolf, GiantSpider, Troll, Griffin, Dragon, Hydra, BogWight, Yeti, Sandworm, Scorpion, Basilisk, Phoenix

### Tech Ages
Stone → Copper (200 pop) → Bronze (500) → Iron (1000) → Steel (2000) → Industrial (5000)

---

## Evaluation

When testing changes:
1. `cargo build` - Check compilation
2. `cargo run --release -- -v` - Test explorer
3. Start simulation with `Shift+S`
4. Check combat log with `Shift+L`
5. Verify territory expansion
6. Test monster spawning

---

## Adding Features

When adding new features:
1. **Document CLI options** if adding new flags
2. **Update module structure** if adding new files
3. **Add to Key Systems** if significant
4. **Test in explorer** - it's the main interface
