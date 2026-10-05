# CLAUDE.md

Procedural world map generator with tectonic plates, erosion, climate, and biomes.
Direction: an autonomous, story-first colony simulator; see `ROADMAP.md` (six updates) and
`TODO.md` (short-term tasks).

---

## Quick Start

```bash
# Build
cargo build --release

# Start screen: tailor the world (size, seed, style, plates, fantasy, history, peoples,
# the Shadow, watching), then generate it and open the tile viewer
cargo run --release
# ...or with options pre-filled
cargo run --release -- --start --seed 42 --world-style pangaea

# Skip the history (faster), or open the frozen legacy terminal explorer
cargo run --release -- --no-history
cargo run --release -- --legacy-explorer
```

---

### Dev world (fast iteration on history and story)
`cargo run --release -- --dev` generates a 96x48 world (seed 76, 8 civilizations) with 250 years
of history in about a second, vs ~3 min at 512x256 (almost all of it history). It has 4 named rivers, 2 lakes, 6 mountain
ranges, forests, a desert and two continents. Any of `--width`/`--height`/`--seed`/
`--civilizations` given explicitly overrides the preset; combine with `--watch`, `--tiles`,
`--journal`, `--gazetteer`, `--director`, `--present` (the state the game starts in) as usual.
`cargo test --release --test present_day` pins the dev world's present day and consistency. Seed 76 came from a search over 96x48 seeds
scored by gazetteer landmarks (`scripts/dev_seed_search.sh`, then the top seeds checked with
`--dev --seed N --headless --gazetteer`, since the history renames and reshapes features;
`DEV_WORLD` in `main.rs`); 64x32 worlds get no rivers at all. If worldgen changes move the
landmarks, re-run that search (it was re-run 2026-10-04 after the polar/area changes).
- Small maps: the river threshold (`water_bodies::river_flow_threshold`) scales with
  (width/512)^2 below 512 wide (unchanged at 512+), and legendary creatures scale with map
  area (`legendary_creatures_for` in `main.rs`; 1500 at 512x256).
- History is deterministic: `history/` and `lore/` use `crate::history::det::{HashMap,
  HashSet}` (fixed hasher) so the same seed tells the same story every run (the journal is
  byte-identical). Use these, not `std::collections::HashMap`, for any map the simulation
  iterates while drawing from the RNG.

---

## Front end

The tile viewer (`src/tiles/`, minifb window) is the default and the **only maintained** front
end. The old terminal (ASCII) front end, `explorer.rs` (ratatui explorer), `menu.rs` (the
pre-generation menu) and `ascii.rs`, is **LEGACY and FROZEN** (since 2026-10-03). It is
reachable only with `--legacy-explorer`. Don't add features to it, port new systems to it,
fix its visuals or document its controls; touch it only if the build breaks. If something in it
is still needed (`main` calls the image exporters `export_base_map_image` and
`export_freshwater_network_image` in `explorer.rs`), move it out of the legacy file first.

---

## Features

### Soils (`soils.rs`)
- `world.soils()` (lazy, never serialized) gives each land tile a soil kind and depth from its
  place in the landscape and its climate: depth 0.4-3 m by weathering (warm and wet deepest),
  +4 m on floodplains and in closed hollows, thinned by local relief; kinds alluvium
  (floodplains), black earth (temperate grassland), volcanic soil, brown forest soil, red clay
  (Mediterranean), leached laterite (wet tropics), podzol (taiga), peat, desert soil, thin stony
  soil, frozen ground. `SoilKind::fertility` (black earth 1.0 ... frozen 0.05).
- Used by farmland (`resources.rs`: fertility = climate x altitude x soil; seed 42 fertile land
  44.7% -> 28.6%), embarks (soil levels = depth / 2 m, material by kind: clay for laterite and
  red clay, sand for podzol and desert, gravel for stony) and the viewer's hover.
  `--resource-stats` prints the soil mix.

### Saving worlds
- `--save-world worlds/x.world` writes the generated world plus any simulated history (bincode,
  ~170 MB at 512x256); `--load-world worlds/x.world` loads it in ~0.1 s instead of regenerating
  (~4 s world + ~6 min history). The tile viewer simulates 250 years of history by default
  (`--no-history` to skip), so save once with history and reload from then on.
- The file has a magic header and version (`WORLD_FILE_VERSION` in `world.rs`, now 2: the
  ecology is appended after the history; version-1 files still load, without ecology); bump it
  when any serialized type changes. New `EventType` variants go at the end of the enum so old
  histories still decode. `worlds/` is gitignored-worthy local data.
- Append-only enums: `EventType`, `ResourceType` and `ExtendedBiome` (`biomes.rs`) gain new
  variants only at the end, so old saves still decode.

---

## Module notes (load when working in that folder)

Detailed notes, tuning numbers and gotchas live next to the code:
- `src/tiles/CLAUDE.md`: tile viewer, watcher, start screen, roads/rivers drawing, seasons, atlas
- `src/history/CLAUDE.md`: ecology and scars, the director (LLM events), the Shadow
- `src/lore/CLAUDE.md`: resources, landmarks, focal points, journal, the bard
- `src/plates/CLAUDE.md`: tectonic simulation, sea level, `tectonic_preview`
- `src/erosion/CLAUDE.md`: landscape evolution, `terrain_lab` and its env vars, legacy erosion
- `src/climate/CLAUDE.md`: climate (EBM, precipitation, runoff) and biome classification
- `src/local/CLAUDE.md`: embarks; `src/region/CLAUDE.md`: zoomed regions
- `src/colony/CLAUDE.md`: the first colony (settlers, needs, jobs, `--sim-snapshot`, `--dev-embark`)

---

## World Generation Pipeline

`terrain::generate_terrain(&TerrainConfig, &seeds, on_stage)` runs steps 1-6 below plus the
finishing passes and drainage repair; `main`, `terrain_lab` and the comparison grids
(`grid_export.rs`) all call it, so they can't drift apart. `on_stage` sees the heightmap (and
crust/climate) after each named stage and can stop early (the lab's metrics hook in there).

1. **Tectonic Plates** - BFS flood-fill creates 6-15 initial plates
2. **Tectonic Simulation** - Plates rotate about Euler poles on a sphere for ~200 Myr; the
   simulation yields final plates, crust thickness/age, and a `stress_map` (see below)
3. **Heightmap** - Derived from crust: Airy isostasy on land, age-depth law at sea, solved sea level
4. **Climate** - Temperature (latitude + elevation), moisture and precipitation
5. **Landscape evolution** - Uplift vs. precipitation-driven river incision, sediment, rebound
6. **Finishing passes** - Coastlines, fjords, regional noise, volcanoes, island coasts, beaches;
   drainage repair (legacy hydraulic/glacial erosion only with `--legacy-erosion`)
7. **Biomes** - 50+ biome types based on climate
8. **Water Bodies** - Detect oceans, lakes, rivers

---

## Map export
- `--export-maps --upscale-factor N` renders at N× the simulation size; rivers are drawn from
  the Bezier river network as anti-aliased strokes at output resolution (width/opacity follow
  discharge), not from per-cell flow accumulation.

---

## Development Workflow

**IMPORTANT**: Always test changes before considering work complete:

1. After making code changes, run `cargo build --release` to check for compilation errors
2. Run the program with a known seed: `cargo run --release -- --seed 42` (or `--dev` for the
   ~1 s development world); check rendering headlessly with `--tiles-snapshot` /
   `--watch-snapshot`
3. Navigate to relevant areas and visually verify the changes work correctly
4. For local map changes, embark (Z/Enter) and test at multiple z-levels with `<` and `>`
5. Only report completion after confirming the feature works as expected

Debug tools:
- `src/multiscale/debug_export.rs` - Export chunk data for analysis
- The tile viewer's window title shows the hovered tile and what is on it
