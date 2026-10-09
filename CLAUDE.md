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
of history in about a second, vs ~12 s at 512x256 (`--history-profile` prints time per phase). It has 4 named rivers, 2 lakes, 6 mountain
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
- The file has a magic header and version (`WORLD_FILE_VERSION` in `world.rs`, now 6 (v6 added `people`); at v2 the
  ecology is appended after the history; version-1 files still load, without ecology); bump it
  when any serialized type changes. New `EventType` variants go at the end of the enum so old
  histories still decode. `worlds/` is gitignored-worthy local data.
- Append-only enums: `EventType`, `ResourceType` and `ExtendedBiome` (`biomes.rs`) gain new
  variants only at the end, so old saves still decode.

---

## Dwarf Fortress as the model
The DF design guide (`../dfdecomp/guide/README.md`, read `00-ideas-catalogue.md` first) is the
reference for game systems: reuse ideas and algorithms, never its decompiled code. Done:
rolled individuals (`src/persona.rs`, `data/defaults/persona.json`), figure pages
(`inspector::figure_page`, `--who`), thoughts/stress/breaks and festivals (`colony/mind.rs`),
generated monsters (`src/monsters.rs`, `data/defaults/monsters.json`, `--bestiary`), cavern
layers (`local/caverns.rs`) and the mine that wakes a forgotten beast (`colony/mine.rs`), named
ages and event collections (`history/ages.rs`, `collections.rs`), arts (`history/arts.rs`,
`data/defaults/arts.json`, `--arts`), blows and wounds (`colony/fight.rs`), crafts that show
history (`colony/craft.rs`), caravans and migrants (`colony/trade.rs`), a speaker and mandates
(`colony/society.rs`), the dead walking under the Shadow (`colony/dead.rs`), outlaw bands of
exiles (`history/bands.rs`), `--require`; then (2026-10-08) strange moods of five kinds
(`colony/mood.rs`), the werebeast's curse (`curse.rs`), the camp's annals (`annals.rs`), temples
and justice (`society.rs`, `justice.rs`), a lost relic and its seeker (`relic.rs`), slain beasts
and their hoards (`fight.rs`), a militia (`militia.rs`), visitors from the world
(`visitors.rs`), engravings (`engrave.rs`), vampires (`night.rs`), pets (`pets.rs`), healing
(`heal.rs`), families (`family.rs`), ghosts and memorial slabs (`ghosts.rs`), expeditions against beasts
(`expedition.rs`), drink (`drink.rs`), war band leaders from the history, cage traps
(`traps.rs`), years and old age (`ageing.rs`), a lord sent from home
(`nobles.rs`), farms under the rock (`cavefarm.rs`), experience
that changes character (`temper.rs`), a tavern and sellswords (`tavern.rs`), books (`books.rs`), livestock
(`livestock.rs`), the deep shaft, adamantine and the hollow (`deep.rs`),
places in the hills found and robbed (`explore.rs`), prisoners (`prisoners.rs`), the world's regard for the camp (`regard.rs`), armour (`armour.rs`), snatchers (`snatch.rs`), sieges (`siege.rs`), evil weather (`weather.rs`), guilds (`guild.rs`), a rising against the lord (`rising.rs`), the caravan's liaison (`liaison.rs`), pets that defend their keepers (`pets.rs`), old comrades and enemies (`recognize.rs`), a slain beast's bones and hide worked into trophies and armour, caravans ambushed on the road, wolves' dens cleared and the restless dead burned (`respond.rs`), artifact thieves (`thieves.rs`), the temple's priest (`priest.rs`), the call to arms (`warcall.rs`), woods that grow back (`regrow.rs`), the tithe (`tithe.rs`), widows and widowers, vows of vengeance (`vow.rs`), peace after vengeance, the kitchen and its cook (`kitchen.rs`), clothes that wear out (`clothes.rs`), dreams of a lifetime (`dreams.rs`), childhood (`childhood.rs`), troubles that come again, rations (`rations.rs`), frozen water and wintering herds, tavern brawls, the patron's bell (`ring_bell`, key B), aquifers and gems (`local/mod.rs`, `dig.rs`), legends
of the camps kept with the world (`legend.rs`, `--sim-legend`), and space in three dimensions: settlers walk any level (`nav::path3`,
`Shape::Stair`), dig a stair spine with bedrooms and a great hall on its levels (`delve.rs`), and
raise the lookout as a tower; who knows what: per-town and per-people knowledge of the
chronicle with each people's own account (`history/knowledge.rs`, `--rumours`), which caravans,
visitors, migrants and bards carry to the camp (`colony/news.rs`); industries: a smelter,
forge, mason's, carpenter's and kiln cut below, ore -> bars -> tools and arms (`industry.rs`);
and blows resolved by a material model (`src/materials.rs`, `data/defaults/materials.json`:
density, hardness, edge, capability flags; weapon shapes; tissue layers and armour by part).
Each has its notes in `src/colony/CLAUDE.md` (knowledge in `src/history/CLAUDE.md`). Debug forcing: `PLANET_FORCE_WERE`, `PLANET_FORCE_SEEKER`,
`PLANET_FORCE_VAMPIRE`, `PLANET_FORCE_MOOD`, `PLANET_FORCE_HUNT`, `PLANET_FORCE_SNATCH`, `PLANET_FORCE_PET_PREY`, `PLANET_FORCE_ORE`, `PLANET_FORCE_HORROR`, `PLANET_FORCE_GHOST`. Board cards `df-*`; progress log in the Claude Doc
"Dwarf Fortress systems: progress log".
Minds that steer: DF's personality needs drive what settlers do between jobs (`colony/needs.rs`), their
needs and character voice the camp's next work (`voices.rs`), and lots and the wall's size follow
purpose and the founders (`projects.rs`); notes in `src/colony/CLAUDE.md`.
Legends mode as linked HTML pages: `--legends DIR` (`lore/legends.rs`; notes in `src/history/CLAUDE.md`).

### Rejecting worlds (`--require`)
`--require rivers=4,lakes=2,ranges=3,...` (rivers, lakes, ranges/mountains, forests, deserts,
islands, continents, marshes, jungles, seas, peaks, counted in the gazetteer) checks the built
world before history; a world that misses is rejected with its counts ("World rejected: seed 1
has 0 rivers (want 4), ...") and the process restarts on seed + 1 (`PLANET_REQUIRE_TRY`, 40 at
most), so the accepted world is exactly what a plain run of that seed gives. DF's worldgen idea;
its other worldgen ideas (midpoint fields, river walkers, a wind table) are simpler than this
pipeline and were not taken.

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
