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
      --tectonic-myr <N>   Simulated tectonic history in Myr [default: 200]
      --legacy-tectonics   Use the old static-plate + noise terrain (no simulation)
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

### Saving worlds
- `--save-world worlds/x.world` writes the generated world plus any simulated history (bincode,
  ~170 MB at 512x256); `--load-world worlds/x.world` loads it in ~0.1 s instead of regenerating
  (~3 min world + ~6 min history). The tile viewer simulates 250 years of history by default
  (`--no-history` to skip), so save once with history and reload from then on.
- The file has a magic header and version (`WORLD_FILE_VERSION` in `world.rs`); bump it when any
  serialized type changes. `worlds/` is gitignored-worthy local data.

### Seasons in the tile viewer
`T` steps Spring/Summer/Autumn/Winter, `C` cycles them automatically. Snow cover (cold + moisture),
foliage colour (spring flush, summer drought, autumn orange), and frozen lakes/rivers/shallows
come from the seasonal climate; `--season` picks the season for `--tiles-snapshot`. Not yet applied
to zoomed regions or embarks.

### Graphical tile viewer (`src/tiles/`)
`cargo run --release -- --seed 42 --tiles` opens a window (minifb) that draws the world with
pixel-art tiles instead of the terminal explorer.
- Mouse wheel / `+` `-` zoom (1-64 px per tile, around the cursor); drag or arrows/WASD pan;
  click the minimap to jump; `N` minimap; `P` screenshot; `Q`/`Esc` quit. Hover info is shown
  in the window title.
- `Z` walks into the high-resolution region under the mouse: arrows/WASD walk (Shift runs),
  wheel zooms, `F` resets zoom, `X` saves the region PNG, `Esc`/`Z` returns to the map (centred
  where you walked to). Near a region edge the next region is generated on a background thread
  and swapped in; zoom terrain is seamless, so you can walk across the world.
- `--tiles-center X,Y` start position; `--tiles-snapshot PREFIX` renders overview/16px/32px
  frames to PNG without a window (for checking rendering).
- Tiles: `atlas.rs` draws a 16x16 pixel-art atlas (ground kinds + transparent sprites, 4
  variants each). `--export-tileset atlas.png` writes it (one row per kind, in `ALL_KINDS`
  order) for editing; `--tileset file.png` loads an edited atlas or a Dwarf Fortress style
  16x16 CP437 sheet (glyphs tinted with per-kind fg/bg colours, magenta = background).
- `classify.rs` maps world data to tiles (biome -> ground + sprite, relief overrides,
  beaches, lakes >= 4 tiles, rivers from D8 flow, one-tile-wide water strips drawn as river
  channels); `render.rs` is pure software rendering (river strokes and coastline foam are
  drawn per pixel, relief shading interpolated between tiles).

### Minimap and zoom
- `N` - Toggle the world minimap (bottom-right; yellow = visible area, white = zoom window)
- `Z` - Simulate the 8x8-tile region around the cursor at 128 cells/tile (~5 s) and show it
  full-screen. In the zoom view: arrows/WASD pan, `+`/`-` scale, `F` fit, `X` save PNG to the
  current directory, `Esc`/`Z` back. Zooming the same tile again reuses the cached region.

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
│   ├── generation.rs # BFS flood-fill plate generation (initial plate layout)
│   ├── simulation.rs # Time-stepped sphere simulation (Euler poles, subduction, rifting)
│   ├── crust.rs      # Isostasy + seafloor-age terrain, sea level, generate_tectonic_terrain()
│   └── stress.rs     # Legacy boundary stress (used by --legacy-tectonics)
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

1. **Tectonic Plates** - BFS flood-fill creates 6-15 initial plates
2. **Tectonic Simulation** - Plates rotate about Euler poles on a sphere for ~200 Myr; the
   simulation yields final plates, crust thickness/age, and a `stress_map` (see below)
3. **Heightmap** - Derived from crust: Airy isostasy on land, age-depth law at sea, solved sea level
4. **Erosion** - Hydraulic and glacial erosion sculpts terrain
5. **Climate** - Temperature (latitude + elevation) and moisture
6. **Biomes** - 50+ biome types based on climate
7. **Water Bodies** - Detect oceans, lakes, rivers

---

## Key Systems

### Tectonic Simulation (`plates/simulation.rs`, `plates/crust.rs`)
- Each plate is a rigid body rotating about its own Euler pole; its crust (thickness, age,
  volcanic edifices) lives in the plate's *body frame*, so motion never blurs the crust.
- Each step, every map cell asks which plates cover it: **overlap** = convergence (oceanic
  subducts under continental, older under younger, builds an arc; continent-continent
  collisions thicken crust), **gap** = divergence (new age-0 oceanic crust: ridges and rifts).
- Slab pull speeds plates up, continental collision locks them. Mantle hotspots are fixed in
  the global frame and leave volcanic chains. Mountains relax by erosion (tau = 140 Myr).
- Elevation: Airy isostasy (continents), Parsons-Sclater sqrt(age) law (ocean), trenches from
  recent subduction, then procedural detail. Sea level is solved for the style's land
  fraction but cannot drop below -300 m (continental area is seeded to make that sufficient).
- Initial continents are the zero contour of a noise-perturbed spherical signed-distance field;
  each continent and its shelf belongs to one plate, plate borders are domain-warped, and
  detached plate fragments are absorbed (otherwise they plough trails through continents).
- Continental collision polarity is decided by each plate's continental area, so island arcs
  are accreted as terranes instead of drilling through continents.
- Step count scales with map width (`steps` is calibrated for 512 wide); deposit kernels and
  stress/trench memory are tuned so moving boundaries don't leave stripes.
- Terrain: interior seaward dome, drainage integration (priority-flood from the open ocean before
  detail noise; hollows shallower than 50 m filled afterwards), coastline roughness confined to
  a band around the shore, and a polar margin that keeps the map's top/bottom rows oceanic
  (flow routing needs ocean connected to those edges).
- `stress_map` is derived from simulated convergence/divergence + standing orogens, scaled to
  the range downstream passes expect (0.15 volcanic, 0.3 mountain building).
- Preview tool: `cargo run --release --bin tectonic_preview -- <seed> <w> <h> <style> <myr> <steps> <out_dir>`
  writes `final.png` (plates | thickness | age | elevation | stress), `timelapse.png`, and
  `basins_sim.png` / `basins_legacy.png` (land trapped in closed basins, with % printed).

### Playable areas (`src/local/`)
Dwarf Fortress style embarks: 192x192 tiles of 2 m with 2 m z-levels, generated (~20 ms) from
the zoomed region around a point.
- In the tile viewer's walking mode a yellow box shows the area; `Enter` embarks. Inside:
  `<` / `>` (or PgUp/PgDn) change z-level, `V` toggles the surface view, wheel zooms, arrows /
  drag pan, `P` screenshot, `Esc` back to walking. Hover info is in the window title.
- Surface: bicubic region elevation + metre-scale relief scaled by slope. Rivers follow the
  region's drainage links (`ZoomRegion::receiver`) and continue upstream as creeks (catchment
  >= `CREEK_AREA_FRACTION` of a tile), with meanders, carved beds and banks; lakes keep their
  level; water freezes over below `FREEZE_TEMP_C`. Underground: soil (sand/clay/gravel/loam,
  thinner on slopes) over the world tile's handshake rock stack with undulating strata.
  Trees/shrubs/grass/boulders from the biome, with groves and clearings. Ramps where the
  surface steps up one level. All noise is keyed on absolute position.
- `--local-snapshot PREFIX --tiles-center X,Y` renders an embark (surface, two z-levels, a
  16 px close-up, and a cross-section) without a window.

### Biomes
- Lowland classification (`climate/biomes.rs::classify_lowland`) uses Earth-calibrated mean
  annual temperature: ice sheets below -22 C (or -15 C when wet), tundra to -7 C, taiga
  -7..4 C; moisture cut-offs match the climate sim's range (rarely above ~0.6).
- `--fantasy 0..1` (default 0.2, old behaviour 0.5) scales fantasy/special biomes, including
  the rare-biome replacement pass. `--biome-stats` prints the land-biome mix, land
  temperature/moisture percentiles and a zonal temperature/land profile.

### Zoomed regions (`region/zoom.rs`)
- `--zoom X,Y` (world tile at the window centre, as the explorer's `W:(x,y)` shows) or
  `--zoom auto`; `--zoom-tiles N` (default 8), `--zoom-scale S` cells per world tile (default
  128, ~610 m/cell on a 512-wide world), `--zoom-erosion I` iterations (default 40).
  Writes `zoom_<seed>_<x>_<y>.png` and a 16-bit heightmap into `--map-output-dir`.
- Seamless: terrain is assembled from fixed world-tile chunks (each simulated with a one-tile
  border, cached in memory) blended with tent weights, so a cell's height depends only on its
  world position; separately generated regions match exactly (tested). Hydrology runs on the
  blended terrain with a two-tile margin. Known exception: the date line (x wrap).
- Pipeline (per chunk): bicubic world heights + relief-scaled fractal detail -> world Bezier rivers carve
  valleys and inject upstream area where they enter -> shallow hollows filled -> implicit
  stream-power erosion + hillslope diffusion on a priority-flood drainage tree -> lakes and
  rivers (moisture-dependent threshold, width ~ 0.8 sqrt(A km2) m). ~7 s for a cold
  1024x1024 region; neighbouring regions reuse cached chunks.
- Render uses the climate colour LUT only (world biome colours and raw world climate are
  tile-blocky when upsampled; climate is smoothed over ~1 tile first).

### Map export
- `--export-maps --upscale-factor N` renders at N× the simulation size; rivers are drawn from
  the Bezier river network as anti-aliased strokes at output resolution (width/opacity follow
  discharge), not from per-cell flow accumulation.

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

---

## Development Workflow

**IMPORTANT**: Always test changes before considering work complete:

1. After making code changes, run `cargo build --release` to check for compilation errors
2. Run the program with a known seed: `cargo run --release -- --seed 42`
3. Navigate to relevant areas and visually verify the changes work correctly
4. For local map changes, embark (Z/Enter) and test at multiple z-levels with `<` and `>`
5. Only report completion after confirming the feature works as expected

Debug tools:
- `src/multiscale/debug_export.rs` - Export chunk data for analysis
- Status bar shows `W:(x,y)` for world position to help locate issues
