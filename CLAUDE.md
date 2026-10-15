# CLAUDE.md

Procedural world map generator with tectonic plates, erosion, climate, and biomes.
Direction: an autonomous, story-first colony simulator; see `ROADMAP.md` (six updates) and
`TODO.md` (short-term tasks).

**Realistic by default (2026-10-13):** the game takes a medieval, realistic road. Myth and
magic (`src/myth.rs`) are off unless `--myth` (or `PLANET_MYTH=1`, or the start screen's "Myth
and magic" row): no Shadow, no beasts of legend or of the deep, every people human (their
tongues still differ), no fantasy biomes or crystal wastes (lakes become only salt flats or
frozen lakes, never lava, acid or glowing water: `determine_lake_fantasy_biome`), no ghosts, risen dead, werebeasts,
vampires or hauntings, caverns with only real cave life and no fungus forests, no magma sea,
adamantine or hollow, no possessed moods, and adventure mode closed. With `--myth` the game is
as it was, bit for bit; the integration tests run with it (`myth_cmd` in each test file), and
`tests/realism.rs` checks the realistic default. Notes below that describe the Shadow, beasts,
races other than human or the uncanny describe the myth world.

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

# Myth and magic (the Shadow, beasts, dwarves and elves, the uncanny): off unless asked for
cargo run --release -- --myth
# The Shadow stays out of the colony even then unless asked for (its raiders, risen dead)
cargo run --release -- --myth --dev-embark --camp-shadow

# Adventure mode: one hero across the world (a fantasy: needs myth)
cargo run --release -- --myth --dev --adventure

# Each seed circles its own star; --star sol for the Sun (the original climate), or pin it
cargo run --release -- --seed 42 --star K3 --hz 0.15 --obliquity 60
cargo run --release -- --seed 42 --headless --no-history --system   # print the star system
```

---

### Dev world (fast iteration on history and story)
`cargo run --release -- --dev` generates a 96x48 world (seed 10, 8 civilizations) with 250 years
of history in about a second, vs ~12 s at 512x256 (`--history-profile` prints time per phase). It has 7 named rivers, 5 lakes, 6 mountain
ranges, 5 forests, a desert, an island and three continents, on the audited terrain the tests
run on (on terrain revision 1, `--terrain-v1`, the dev world is seed 76). Any of `--width`/`--height`/`--seed`/
`--civilizations` given explicitly overrides the preset; combine with `--watch`, `--tiles`,
`--journal`, `--gazetteer`, `--director`, `--present` (the state the game starts in) as usual.
`cargo test --release --test present_day` pins the dev world's present day and consistency.
The dev world circles the Sun; `--dev-sky eyeball|slow|tilted|twosuns` is a dev world under
another sky (a red dwarf's locked world, a 600-hour day, a 70-degree tilt, a close pair of suns;
`DEV_SKIES` in `main.rs`, each with rivers, woods and a camp that lives). Seed 10 came from a search over 96x48 seeds
scored by gazetteer landmarks (`scripts/dev_seed_search.sh`, then the top seeds checked with
`--dev --seed N --headless --gazetteer`, since the history renames and reshapes features;
`DEV_WORLD` in `main.rs`); 64x32 worlds get no rivers at all. If worldgen changes move the
landmarks, re-run that search (re-run 2026-10-04 after the polar/area changes, and 2026-10-13
for the audited terrain: seed 76 kept 2 rivers and no lake on it).
- Standard seeds (what the notes and tests measure against): the dev world (seed 10, 96x48, the
  Sun, its embark 30,34; seed 76 on terrain revision 1); dev colonies on `--dev --seed N` for N
  in 1-40 (tests pick the seeds that show their event; re-pin when a change moves the camps'
  days; the tests were re-pinned on the audited terrain on 2026-10-14, notes quoting seeds 23, 58
  and 76 before that date are of revision 1); dev sites by `--tiles-center` (33,32 the tomb of
  Zreetsio, 42,32 a living beast's lair and a cave, 69,8 a river camp, 0,35 a delve, 36,23 a
  battlefield's nameless dead); the dev skies (`--dev-sky`: 114 M3, 316 K4 + 600-hour day, 302 G
  + 70 degree tilt, 20 a close F1 pair asked for with `F1+b1`; on revision 1 106, 309, 302, 9 with
  its own star); seed 42 at 512x256 for the
  default size (it circles a K6 dwarf; numbers quoted for it are `--star sol`'s) and 256x128 for
  the Shadow's scale; seed 202 for ruins and ecology. `worlds/` stays gitignored (a world file is
  ~170 MB at 512x256): save one locally with `--save-world` and reload it.
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

### Star systems (`astro/`)
- Every world circles a star drawn from its seed (spectral type F-M, age, metallicity), on an
  orbit in the star's habitable zone (Kopparapu), with a spin (day, tilt, tidal locking around
  dim stars, thermal tides), moons, a bulk (gravity, interior heat, water, magnetic field), an
  air and neighbour planets (giants by metallicity, Hill-stable). The climate takes the
  starlight, eccentric seasons, spin (Hadley cells and jets), year length, a locked face, the
  star's colour (albedo of ground and ice) and a CO2 greenhouse from the carbonate-silicate
  cycle (`climate/thermostat.rs`); the tectonics take the water (land fraction) and the heat
  (hotspots). `--star sol` (and `--dev`, and old world codes) is the Sun and the original climate,
  bit for bit. H in the viewer shows the heavens; the journal has "The Heavens"; the start
  screen has "The heavens" rows. Notes in `src/astro/CLAUDE.md`.

### The terrain audit (`terrain_rev.rs`, `audit.md`, 2026-10-13)
- The terrain follows the audit in `audit.md`: the pipeline in causal order (what builds the
  land, then the climate, then the rivers; see the pipeline below), rock hardness from the crust
  steering erosion, temperatures that follow the final heights, curvature with the right sign,
  the sphere's metric (cos latitude) in gradients, blurs and distances, noise sampled on the
  sphere, round distance fields, signal-weighted ridges, orogens with plateaus and foreland
  basins, shelf-slope-rise margins and fracture zones, fjords up glacial valleys, volcano shapes
  by kind, lava that runs downhill, water tables from the wetness index, exact waterfalls,
  biome blending across the whole border zone, day length from the sunrise hour angle and the
  sea's seasonal lag. Notes by module in `src/erosion/`, `src/plates/` and `src/climate/`
  `CLAUDE.md`.
- `--terrain-v1` (or `PLANET_TERRAIN_V1=1`) is the terrain as it was, bit for bit (checked with
  `terrain_lab` on seed 42 at 512x256, every stage's image byte-identical). The integration tests
  run on the audited terrain (re-pinned 2026-10-14; they ran on revision 1 until then), and
  `tests/terrain.rs` checks it. World codes on
  the audited terrain end in ".t2" (`10.96x48.earthlike.8.250.t2@37,29`); a code without it is
  loaded on revision 1, so codes shared before the audit still make their worlds.
- Seed 42 at 512x256 (audited vs revision 1): big-river water trapped short of the sea 4.4% vs
  6.4%, closed basins 0.8% vs 0.9% of land, 33 lakes vs 24, 394 river tiles vs 280.

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
- The file has a magic header and version (`WORLD_FILE_VERSION` in `world.rs`, now 7 (v6 added `people`, v7 the star system); at v2 the
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
`PLANET_FORCE_VAMPIRE`, `PLANET_FORCE_MOOD`, `PLANET_FORCE_HUNT`, `PLANET_FORCE_SNATCH`, `PLANET_FORCE_PET_PREY`, `PLANET_FORCE_ORE`, `PLANET_FORCE_HORROR`, `PLANET_FORCE_GHOST`, `PLANET_FORCE_RELIC`; `PLANET_NO_HARD_TIMES` leaves the history's famine and drift out (`history/hard_times.rs`). Board cards `df-*`; progress log in the Claude Doc
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
- `src/astro/CLAUDE.md`: the star system (star, orbit, spin, moons, air, neighbours, the sky)
- `src/local/CLAUDE.md`: embarks; `src/region/CLAUDE.md`: zoomed regions
- `src/colony/CLAUDE.md`: the first colony (settlers, needs, jobs, `--sim-snapshot`, `--dev-embark`)
- `src/adventure/CLAUDE.md`: adventure mode (one hero across the walkable land, places, the bot)
- `src/adventure/CLAUDE.md`: adventure mode (one hero across the world: places from the history,
  Tibia's levels, callings and dungeons; `--adventure`, `--adventure-bot`, the ink window)

---

## World Generation Pipeline

`terrain::generate_terrain(&TerrainConfig, &seeds, on_stage)` runs steps 0-7 below; `main`,
`terrain_lab` and the comparison grids (`grid_export.rs`) all call it, so they can't drift
apart. `on_stage` sees the heightmap (and crust/climate) after each named stage ("tectonic",
"volcanoes", "noise", "islands", "coastline", "climate", "landscape", "erosion", "fjords",
"beaches", "final") and can stop early (the lab's metrics hook in there). Terrain revision 1
(`--terrain-v1`) ran the climate and erosion first and every finishing pass after.

0. **Star system** - `astro::StarSystem::generate` from the seed: the climate's forcing, the
   tectonics' land fraction (water) and hotspots (interior heat)
1. **Tectonic Plates** - BFS flood-fill creates 6-15 initial plates
2. **Tectonic Simulation** - Plates rotate about Euler poles on a sphere for ~200 Myr; the
   simulation yields final plates, crust thickness/age, and a `stress_map` (see below)
3. **Heightmap** - Derived from crust: Airy isostasy on land (foreland basins, a plateau
   ceiling), age-depth law at sea (slope and rise, fracture zones), solved sea level
4. **Constructive passes** - Volcanoes (shaped by kind), regional relief, island coasts,
   coastline jitter: the land before the rivers wear it
5. **Climate** - Temperature (latitude + elevation), moisture and precipitation
6. **Landscape evolution** - Uplift vs. precipitation-driven river incision on rock of the
   crust's hardness, sediment, rebound; the water re-poured for the style's land; ice-scoured
   and rift basins too young to have filled (legacy hydraulic/glacial erosion only with
   `--legacy-erosion`)
7. **Finishing passes** - Fjords up the glacial valleys, beaches; the last pits filled (the
   rivers' lakes kept), flow routed again, temperatures moved by the lapse rate to the final
   heights
8. **Biomes** - 50+ biome types based on climate
9. **Water Bodies** - Detect oceans, lakes, rivers

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
- Frame budget: `--frame-bench SPEED`, `--frame-bench-world`, `--frame-bench-watch` (headless,
  the window's per-frame work; see "Frame budget" in `src/tiles/CLAUDE.md`). The colony's ground
  and the world map are kept between frames and redrawn only where they change.
- `src/multiscale/debug_export.rs` - Export chunk data for analysis
- The tile viewer's window title shows the hovered tile and what is on it
