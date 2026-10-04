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

# With specific seed
cargo run --release -- --seed 42

# Custom map size
cargo run --release -- --width 1024 --height 512
```

### Dev world (fast iteration on history and story)
`cargo run --release -- --dev` generates a 96x48 world (seed 76, 8 civilizations) with 250 years
of history in about a second, vs ~6 min at 512x256 (almost all of it history). It has 4 named rivers, 2 lakes, 6 mountain
ranges, forests, a desert and two continents. Any of `--width`/`--height`/`--seed`/
`--civilizations` given explicitly overrides the preset; combine with `--watch`, `--tiles`,
`--journal`, `--gazetteer`, `--director` as usual. Seed 76 came from a search over 96x48 seeds
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

## Front end

The tile viewer (`src/tiles/`, minifb window) is the default and the **only maintained** front
end. The old terminal (ASCII) front end, `explorer.rs` (ratatui explorer), `menu.rs` (the
pre-generation menu) and `ascii.rs`, is **LEGACY and FROZEN** (since 2026-10-03). It is
reachable only with `--legacy-explorer`. Don't add features to it, port new systems to it,
fix its visuals or document its controls; touch it only if the build breaks. If something in it
is still needed (`main` calls the image exporters `export_base_map_image` and
`export_freshwater_network_image` in `explorer.rs`), move it out of the legacy file first.

## Features

### Resources and history (`lore/resources.rs`, `history/simulation`)
- `world.resources()` (lazy, never serialized) derives the world's wealth from its geology:
  copper/gold/silver/gems in high-stress arcs and orogens and around volcanoes, tin and silver in
  granite uplands, iron in hard rock belts, coal in wet sedimentary lowlands, salt in dry basins;
  plus farmland (water, warmth, flat ground, floodplains, volcanic soil), timber and fish.
  `--resource-stats` prints the mix (metals average stress +0.4, coal/salt ~0).
- History uses it: settlement sites score fertility and nearby ore; `apply_local_economy` sets
  each settlement's local resources, production, carrying capacity and growth; trade partners
  and goods are chosen by what each side has and the other lacks; faction income includes
  extraction; resource envy between neighbours adds friction, produces "dispute over iron"
  incidents and Resource wars. `--civilizations N` (default 60) sets the founding powers.
- Settlements are founded (colonization from crowded towns), conquered, razed or abandoned, so
  ruins exist. Sieges launched when a war ends now carry on after it (they used to be lifted
  instantly). Disasters take a fraction of a settlement, not a flat number.
- Visible: the tile viewer's `R` toggles ore markers (size = richness); hover names deposits,
  farmland and fishing grounds; embarks contain ore veins (`Material::Ore`, recoloured flecks) in
  host rock near deposits.
- `ResourceType` gained Coal/Tin/Fish at the *end* of the enum (bincode-compatible with old saves).

### Ecology and scarred landscapes (`history/ecology.rs`)
- `history.ecology` (stepped once a year inside the history sim, no RNG) holds per-tile forest
  cover (vs. climax `forest_potential`), farmland, and densities for 9 species (`SPECIES`:
  deer, caribou, boar, aurochs, antelope, ibex as grazers; wolf, lion predators; bear omnivore)
  with biome habitats, logistic growth, predation, hunting and human-intolerance, and spread
  into neighbouring free habitat (migration / recolonisation).
- Settlements press on the land (`pressure`): fields within `0.3 + sqrt(pop/4000)` tiles,
  logging, hunting. Abandoned land regrows. Chronicle events (once per settlement):
  `ForestCleared`, `GameScarce`, `WildlifeReturned` (wolves/bears/lions den in a ruin).
- Anomaly biomes are caused, not rolled: with a history, `Ecology::naturalize` restores
  randomly placed anomaly tiles (`is_caused_biome`) and `update_scars` grows `scars` from
  events, each chronicled as `LandScarred` linked (`caused_by`) to its cause: bone fields where
  6+ battles were fought (battles are located at the defender's settlement nearest the
  attacker), titan bones where huge legendary beasts were slain, ashlands / dead woods /
  crystal woods around long-held lairs of huge beasts with elemental / necromantic / spell
  powers, crystal wastes where towers stood over razed towns, overgrown or cyclopean ruins a
  century after a town falls. `apply_scars` writes them into `world.biomes` (in `main`).
- Shown: tile viewer draws `Fields`, thinned/felled forests, `Bones` / `TitanBones` sprites;
  hover lists wildlife and the scar's chronicle entry. Embarks (`local/wildlife.rs`, via
  `RegionLore::wildlife`) get game trails (least-cost paths to water), burrows, nests, predator
  dens with bones, and bones on bone fields (`LocalMap::features`).
- The history summary prints an ecology report (forest %, farmed tiles, species vs. start,
  scars by kind, chronicle counts).

### Landmarks (`lore/landmarks.rs`)
- `find_landmarks(world, gazetteer)` picks the world's extremes: the highest peak ("the roof of
  the world") and each continent's summit, the longest river (measured along its main stem in
  km at the real tile size), the largest and the deepest lake, the greatest waterfall on a
  named river, the deepest gorge (river tile with high ground on both banks), up to 3 crater
  lakes and 3 groves of giant trees (CraterLake / AncientGrove patches), the largest desert and
  forest. Each has a name, an epithet and a measurement.
- Shown: `--gazetteer` prints them; tile-viewer labels of landmark features rank above their
  kind and show from 2 px/tile (falls, gorges, crater lakes and groves get their own labels);
  hover adds "the longest river in the world (4,700 km)" on every tile of the feature; the
  journal's opening names the longest river, largest lake, greatest falls and deepest gorge.
- Water bodies: the body holding every river tile has the reserved id `WaterBodyId::RIVER`
  (65535). It used to take the next free id, so `is_lake()` held on river tiles: the viewer
  drew river confluences as lake squares, the gazetteer named all rivers one huge "lake", and
  history/resources treated rivers as lakes.

### History journal (`lore/journal.rs`)
- `--journal PATH` (or `J` in the tile viewer, which writes `journal_<seed>.html` and opens it)
  writes the history as a self-contained HTML book: "The Annals of <largest continent>" with an
  opening on the geography, one book per age (merged timeline eras) with year-by-year entries,
  then the peoples, the 30 greatest wars, ~220 lives of note, ~160 beasts of legend and the
  land (fauna vs. start, scarred places). Routine events (raids, treaties, quarrels, trade,
  new villages, crafted artifacts, conversions) are folded into yearly or per-age tallies; a
  people's founding is told once. Names with entries are linked via event participants.
  Search box and category chips filter the annals. Styled like the ink map (parchment, sepia,
  red year rubrics; dark theme).

### The bard: LLM-written lore (`lore/bard.rs`)
- `--bard N` has a local model served by Ollama (`--bard-model`, default `gemma4:26b`;
  `--bard-url`, default `http://localhost:11434`) write N pieces from the history:
  founding songs, poems by notable figures (love / grief / a slaying / war / homeland, chosen
  from their own life), folk legends of beasts, laments for razed towns, artifact
  inscriptions and lore, soldiers' ballads of bone fields. `commissions()` builds the queue
  (most significant first, kinds interleaved, skipping what's already written); each prompt
  carries the writer's voice (by race), temperament and the real geography (biome, climate,
  named rivers/peaks/forests nearby). `--bard-prompts N` prints prompts without the model.
- Writings live in `history.library` (`Library`), saved as the world file's last field
  (`WORLD_FILE_VERSION` 3; v1/v2 still load). With `--save-world` the world is saved after
  every piece, so long runs can be interrupted and resumed (rerun adds the next N).
- The journal shows them in place (songs at foundings, laments at razings, poems in lives,
  legends with beasts, ballads with scars), plus a Treasures part and a Songs and Sayings index.
- Speed on this machine: ~10 s per piece with `gemma4:26b` (MoE, ~4B active, 44 tok/s) vs
  66-105 s with the dense `qwen3.8:27b` (3.5 tok/s); Gemma also keeps to the prompt rules
  better. `--bard-rewrite` rewrites already-written pieces with the current model.

### The director: LLM-authored events during history (`history/director.rs`)
- `--director N` (with a simulated history; uses `--bard-model` / `--bard-url`, default
  `gemma4:26b` via Ollama) lets the model author up to N events at turning points. After each
  step the director scores the step's new events by drama (`drama()`: fallen peoples, razed
  towns, succession crises, coups, holy wars, plagues...), paces the budget over the history
  and avoids peoples it wrote about in the last 12 years.
- The model gets a dossier (peoples with voice and faith, seat and its geography, ruler and
  notable figures with temperament, feelings toward others, recent events) and answers JSON
  constrained by `proposal_schema()`: title, 3-5 sentence chronicle text, up to 4 effects
  from `EFFECT_MENU` (opinion, war, peace, alliance, death, crown, marriage, exile, defect, title,
  epithet, artifact, monument, population, wealth, convert), optional thread. Names resolve
  only against the dossier's cast (`lookup`), amounts are clamped, invalid effects dropped;
  effects go through engine paths (`War::end`, `step::succeed` for dead/exiled rulers).
- Threads (prophecy/feud/curse/vow/secret) carry a checkable `Trigger` (years, ruler or
  figure dies, war between two peoples, town falls); when due they are paid off first, and
  the payoff event is `caused_by` the origin. Stored in `history.tales` (`Tales`), saved as
  the world file's last field (`WORLD_FILE_VERSION` 4). Journal: authored events are "Tales"
  entries; a Threads of Fate part lists arcs.
- The model is behind the `Author` trait; tests use a scripted author (no Ollama needed).
- Without Ollama (or with `--director 0`) history is fully procedural as before.

### The Shadow (`history/shadow.rs`)
- A dark power raised at the dawn of history (on by default; `--no-shadow` for sandbox
  worlds; `HistoryEngine::shadow`). `pick_realm` chooses the realm with the most foreign towns
  near its capital (orcs/goblins/undead weigh more); its capital is the seat; the archetype
  (Warlord / Necromancer / Tyrant) follows the race and names the lord ("the Dark Lord", ...).
- Each season (`step`, called from `simulate_step`; no RNG draws, rolls hash seed+season+target):
  a corruption field spreads one tile from its sources (seat 1.0, its towns 0.8, its land 0.5),
  fading by terrain conductance (fast on roads/rivers, slow over mountains, none over water),
  held back by other peoples' towns; reach grows with `strength`. Wild land above 0.55 becomes
  its dominion. Every 6-24 seasons it strikes the town deepest in its shadow: captured or
  burned (`ShadowConquest`) or holds (`ShadowRepelled`; it then leaves that town alone for 40
  seasons). Falls stiffen the free peoples' defence (rally). If its seat falls it is broken
  (`ShadowBroken`) and returns ~20 years later in a new seat. Every deed is `caused_by` the
  previous one, back to `ShadowRose`. Tuned on `--dev`: ~13 falls, ~20-30 held, a third of the
  land darkened over 250 years; the end-of-history report prints a `Shadow:` line.
- Drawn in three layers (`render.rs::shadow_ink`, `classify.rs`): reach = a cold grey wash that
  deepens with corruption (land only); blight (corruption >= 0.6) = dead trees; dominion =
  cross-hatching (doubled where deepest) inside a jagged ink border; the seat is a black tower
  with a red eye. The watcher has a Shadow panel (lord, darkened/blighted land, towns held,
  fallen, held out) and marks its realm in red.
- Saved as the world file's last field (`WORLD_FILE_VERSION` 5; v1-v4 still load).

### Watching history being written (`tiles/watcher.rs`)
- `--watch` simulates the history in a window (DF world-gen style, dressed as the ink map),
  then opens the tile viewer. The simulation runs on a background thread at full speed
  (`HistoryEngine::begin` / `step` / `finish`) and *records* every season: a `Step` with the
  almanac, realms, places, events, new roads and a `Delta` of the `HistoryOverlay` (owners,
  roads, sites, fields/forest cover, Shadow corruption; `classify.rs`), with a full keyframe
  every 40 seasons. The window *plays the recording back* at the chosen pace, independent of
  how slow the simulation is, and can jump anywhere already written.
- Playback: Space pause, `[` `]` pace, `<` `>` one season back/forward, click or drag the
  timeline bar (pale = written, gold = shown, red playhead) to jump; the chronicle is rebuilt
  for the shown season. Each shown season = `base.clone()` + `apply_overlay` (~1-20 ms).
- Smooth view: wheel zoom eases towards its target, WASD/arrows pan (Shift faster), drag pans;
  while the view moves the map renders as a half-resolution preview (`render_world_lod`, same
  look) when a full render is over ~14 ms, and sharpens once still. `render_world` is
  row-parallel (rayon): ~6-15 ms fitted on 512x256, ~10-28 ms on the dev world (11 px/tile,
  detailed path), previews 2-10 ms. Per-tile flags keep the costly per-pixel work local:
  `shadow_near` / `dominion_edge` (Shadow ink), `owner_edge` (borders), road curves per tile.
- Map: settlements, fields, territories (faction wash + borders) and roads appear as they
  spread; `overlay_realms` draws borders/roads at zooms where `render_world` omits them. New
  roads glow, foundings ring, battles/razings/disasters flare; great events get a banner.
- Panel: year/season, timeline, almanac, souls sparkline, the Shadow, great realms. Chronicle
  below: key events (`style()` decides glyph, colour, map mark and whether it is key), `L` = all.
- `--watch-snapshot PREFIX --headless` renders frames at 1/4, 1/2, the end and a close-up to
  PNG without a window (for checking the look). The window doesn't redraw while hidden, so
  screen capture is unreliable; use this.

### Start screen (`tiles/start.rs`)
- Shown when the game runs with no arguments, or with `--start` (other flags pre-fill it).
  Rows: world size (Dev 96x48 / Small / Standard / Large), seed (type digits, R rerolls),
  shape of the lands (`WorldStyle`), tectonic plates, age of the crust (`--tectonic-myr`),
  fantasy, written history (years, 0 = none), founding peoples, the Shadow, watch it unfold;
  a help panel explains the selected row and a rough time estimate shows by Begin.
- `main` copies the choices into `Args` (width, height, seed, world_style, plates,
  tectonic_myr, fantasy, history_years / no_history, civilizations, no_shadow, watch) and
  generation continues as with flags. The window closes during world generation (progress is
  printed); the watcher or tile viewer opens after.
- Shared drawing for the viewer's own screens (palette, parchment `card`, `heading`, `wrap`,
  ...) lives in `tiles/ui.rs`.

### Roads and tile colours (`tiles/classify.rs`)
- Roads and rivers are drawn as curves (`Strokes::build`): each linked tile gets a
  hash-jittered node (separate patterns for roads and rivers); links pass through the
  midpoints between nodes; a two-link tile is a quadratic curve bending at its node,
  ends/junctions are spokes. Stored per tile as segments with a half-width
  (`road_strokes`, `river_strokes`; river width blends between tiles and grows downstream);
  `Strokes::edge_distance` is the per-pixel query used by `render.rs`, and the watcher's
  far-zoom overlay draws the road segments.
- Flat tile colours (minimap, far zoom) composite the sprite over the ground by opacity
  (`Atlas::average_over`); averaging only a sprite's opaque pixels was mostly ink outline and
  speckled zoomed-out maps with dark dots.

### Saving worlds
- `--save-world worlds/x.world` writes the generated world plus any simulated history (bincode,
  ~170 MB at 512x256); `--load-world worlds/x.world` loads it in ~0.1 s instead of regenerating
  (~4 s world + ~6 min history). The tile viewer simulates 250 years of history by default
  (`--no-history` to skip), so save once with history and reload from then on.
- The file has a magic header and version (`WORLD_FILE_VERSION` in `world.rs`, now 2: the
  ecology is appended after the history; version-1 files still load, without ecology); bump it
  when any serialized type changes. New `EventType` variants go at the end of the enum so old
  histories still decode. `worlds/` is gitignored-worthy local data.

### Seasons in the tile viewer
`T` steps Spring/Summer/Autumn/Winter, `C` cycles them automatically. Snow cover (cold + moisture),
foliage colour (spring flush, summer drought, autumn orange), and frozen lakes/rivers/shallows
come from the seasonal climate; `--season` picks the season for `--tiles-snapshot`. Not yet applied
to zoomed regions or embarks.

### Graphical tile viewer (`src/tiles/`)
`cargo run --release -- --seed 42` opens a window (minifb) that draws the world with
pixel-art tiles (the default front end; `--tiles` is accepted but no longer needed).
- Mouse wheel / `+` `-` zoom (1-64 px per tile, around the cursor); drag or arrows/WASD pan;
  click the minimap to jump; `N` minimap; `P` screenshot; `Q`/`Esc` quit. Hover info is shown
  in the window title.
- `Z` walks into the high-resolution region under the mouse: arrows/WASD walk (Shift runs),
  wheel zooms, `F` resets zoom, `X` saves the region PNG, `Esc`/`Z` returns to the map (centred
  where you walked to). Near a region edge the next region is generated on a background thread
  and swapped in; zoom terrain is seamless, so you can walk across the world.
- `--tiles-center X,Y` start position; `--tiles-snapshot PREFIX` renders overview/3px/16px/32px
  frames to PNG without a window (for checking rendering; 3.5 px/tile is just below the
  detailed-tile threshold) and prints each frame's render time (best of three; seed 42: ~11 ms
  overview and 3px, ~16-21 ms at 16/32 px).
- Far zoom (< 4 px/tile) draws flat tile colours: land/water per pixel from the smooth shoreline
  field and the colour from a domain-warped tile lookup of the same class (`far_color`), so
  lakes, shallows and biome patches don't show the tile grid.
- Data overlays (`tiles/overlays.rs`; the old terminal explorer's V views): `O` cycles (Shift+O
  back) Map, Height, Temperature, Moisture, Drainage (log flow), Tectonic plates, Tectonic
  stress, Biomes. Muted palettes washed over the map (`render.rs::overlay_tint`, modulated by
  the map's own light/dark so ink lines stay readable), a legend card bottom-left, and the
  value under the cursor in the window title. `--overlay NAME` applies one to
  `--tiles-snapshot`.
- Tiles: `atlas.rs` draws a 32x32 atlas in an ink-cartography style (ground kinds +
  transparent sprites, 4 variants each): muted washes on parchment, sepia ink outlines, shadow
  sides hatched (never darkened), symbols built from shape masks so all share one treatment.
  Keep new tiles in that style (palette consts `INK`, `SEA_INK`, `PAPER`, `STONE`, ...). `--export-tileset atlas.png` writes it (one row per kind, in `ALL_KINDS`
  order) for editing; `--tileset file.png` loads an edited atlas or a Dwarf Fortress style
  16x16 CP437 sheet (glyphs tinted with per-kind fg/bg colours, magenta = background).
- `classify.rs` maps world data to tiles (biome -> ground + sprite, relief overrides,
  beaches, lakes >= 4 tiles (frozen lakes stay `Lake` tiles flagged `lake_ice` and are drawn
  iced over, so they keep the smooth inked shore), rivers from D8 flow, one-tile-wide water
  strips drawn as river channels); `render.rs` is pure software rendering, per pixel: land/water comes from a smooth
  noisy field between tile centres, sampled through a domain warp (0.42 tiles) so lakes and
  coasts don't keep the tiles' rounded-rectangle outline (ink coastline, pale wash and two
  offshore ripple lines on its 0.5 contour), ground kinds and territory borders are looked up through a domain warp so
  borders meander, the deep-ocean edge is a contour of a smooth depth field, snow/season tint
  blend between tiles, rivers get ink banks, and atlas tiles larger than the screen cell are
  2x2 supersampled. Labels are ink on a parchment halo.

---

## Module Structure

```
src/
├── main.rs           # CLI entry point
├── explorer.rs       # LEGACY, frozen: terminal UI (ratatui), --legacy-explorer only
├── menu.rs           # LEGACY, frozen: terminal pre-generation menu
├── terrain.rs        # The terrain pipeline (generate_terrain), shared by main, terrain_lab, grid exports
├── world.rs          # WorldData structure
├── tilemap.rs        # 2D grid with wrapping
├── heightmap.rs      # Terrain generation
├── climate.rs        # Temperature/moisture
├── biomes.rs         # 50+ biome types
├── water_bodies.rs   # Lakes/rivers/ocean detection
├── scale.rs          # Physical scale (km/tile)
├── ascii.rs          # LEGACY, frozen: ASCII rendering for the terminal UI
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
  recent subduction, then procedural detail.
- Sea level is the planet's water poured into the basins (`crust.rs`): a priority flood from the
  deepest cell gives each cell the water level at which it joins the ocean, so closed basins
  inland stay dry even below sea level. The volume is set at birth so the style's land fraction
  is met (`sea_level_for_land_fraction`), stored as `TectonicTerrain::ocean_gel_m` (global
  equivalent layer; earthlike ~2.4-2.8 km, Earth 2.64), and kept after the landscape evolution
  (`relevel_to_volume`: shelf sediment displaces water, the sea rises ~40 m at 512x256, ~165 m
  on the dev world). A fixed volume per style was tried: land fraction then swung 4-42% between
  earthlike seeds, because the simulated continental crust (47-65% of the map) stands at a mean
  -700 to +500 m depending on the seed. The legacy erosion and finishing passes don't conserve
  mass, so the sea is not re-levelled after them.
- Initial continents are the zero contour of a noise-perturbed spherical signed-distance field;
  each continent and its shelf belongs to one plate, plate borders are domain-warped, and
  detached plate fragments are absorbed (otherwise they plough trails through continents).
- Continental collision polarity is decided by each plate's continental area, so island arcs
  are accreted as terranes instead of drilling through continents.
- Step count scales with map width (`steps` is calibrated for 512 wide); deposit kernels and
  stress/trench memory are tuned so moving boundaries don't leave stripes.
- Terrain: interior seaward dome, drainage integration (priority-flood from the sea before
  detail noise; hollows shallower than 50 m filled afterwards), coastline roughness confined to
  a band around the shore.
- Land may sit on the poles. "The sea" is one definition everywhere
  (`erosion::landscape::sea_mask`): connected bodies at or below sea level of 50+ cells at
  512x256 (scaled by area, at least 8), wherever they are; routing, depression filling (`rivers.rs`) and
  water-body detection seed from it instead of from the map's top/bottom rows. Plate areas,
  continental-crust area and the land fraction are measured on the sphere (rows weighted by
  cos latitude); counting map cells had made polar plates look big, so they were picked as
  continents and the poles came out continental. Collision and arc deposits are scaled by
  the source/receiving cell area too (`Grid::deposit_scale`): every overlapping polar cell used
  to add a full cell's crust, piling crust up to 9-11 km elevations at the poles (hidden while
  the poles were forced underwater). Some worlds now get a polar continent (seed 7), most
  little polar land.
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
- Lowland classification (`climate/biomes.rs::classify_lowland`) is a Whittaker diagram on
  mean annual temperature (ice below -22 C or -15 C when very wet, tundra to -10 C, taiga
  -10..4 C) and the moisture index P / (P + PET) (`climate::moisture_index`, the UNEP aridity
  index AI as AI / (1 + AI)): deserts below 0.17 (AI 0.2), steppe / savanna to 0.33-0.42,
  forests above ~0.4 (AI ~0.67), rainforest above 0.67 (AI 2). PET rises with temperature,
  so the same rain makes forest in the cold and steppe in the heat (Koppen's B rule).
  Seed 42: forests 22%, desert 21%, grass/savanna 26%, tundra 27% of land tiles (seeds 7 / 99:
  forests 37% / 28%); tiles over-weight high latitudes, so tundra is smaller by area.
- `--fantasy 0..1` (default 0.2, old behaviour 0.5) scales fantasy/special biomes, including
  the rare-biome replacement pass. `--biome-stats` prints the land-biome mix, land
  temperature/moisture percentiles and a zonal temperature/land profile.

### Zoomed regions (`region/zoom.rs`)
- `--zoom X,Y` (world tile at the window centre) or
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

### Landscape evolution (`erosion/landscape.rs`)
- `landscape::evolve` runs in `main` after the climate and before the legacy erosion (tectonic
  terrain only; followed by `relevel_to_volume`): 40 implicit steps over 10 Myr on the world grid. Each step: flexural response
  to the last step's load (rebound 0.82 of rock removed, subsidence 0.6 of sediment, Gaussian
  over 150 km), hillslope creep (sea = base level), priority-flood routing from the sea (enclosed
  seas of 50+ cells at 512x256 count; hollows fill to their spill level and drain through it),
  discharge = climate precipitation x runoff summed downstream, uplift of land from the
  `stress_map`, implicit stream power `K Q^0.5 S` (K = 1.2e-7 with Q in m3/yr; erodes ~3 km3/yr
  at 512x256, Earth's sediment flux is ~8), then sediment routed down: it settles in lakes up
  to the spill level and on shelves around river mouths (40 m deep + 60 m per cell, 10 cells
  out), the rest to the deep sea. ~0.6 s at 512x256, ~4 s at 1024x512.
- K was retuned (5e-7 -> 1.2e-7) when precipitation went from ~100 to ~850 mm/yr on land, to
  keep ~29M km3 eroded over 10 Myr; retune it again if precipitation changes a lot. K is
  defined at 512 wide and scaled by (cell size / 78 km)^-0.27, so the mean depth eroded is the
  same at any map size (it was ~1.5x deeper on the 96x48 dev world).
- `heightmap::apply_island_coasts` (a finishing pass, before the beaches) breaks up the coasts of
  small islands (< 200 cells at 512x256) with tile-scale ridged noise near sea level (+-420 m,
  fading by 500 m), so arcs and hotspot islands get bays, headlands and satellite islets instead
  of eroded ovals (seed 42: 21 -> 34 islands, raggedness 2.9 -> 3.5). Shaping the volcanic
  edifices in the crust instead made no difference after erosion.
- `landscape::fill_pits(hm, 4, 10.0)` runs after the finishing passes (coastline, fjords,
  regional noise, volcanoes, island coasts, beaches), which pock the land with pits: hollows under 4 cells or
  10 m deep are filled and flats tilted (0.05 m/cell) so every cell outside a lake drains;
  bigger hollows stay lakes. `landscape::lake_depth` gives the standing water per cell.
- `terrain_lab` (`cargo run --release --bin terrain_lab -- <seed> <w> <h> <style> <out_dir>`)
  runs the terrain pipeline without history or window and prints, per stage, land %,
  elevation quantiles, closed-basin share and where water from big-river cells ends up by
  raw steepest descent (sea / lake / pit or flat), plus `terrain_<stage>.png`.
  It also counts islands (land bodies under 200 cells) and their raggedness (perimeter^2 /
  (4 pi area); a grid disc is ~1.6). `LAB_NO_ISLANDS=1` skips `apply_island_coasts`.
  `LAB_VERBOSE=1` reports after every finishing pass; `LAB_ONLY_LEM=1` stops after the
  landscape step; `LAB_NO_LEM=1` skips it; `LEM_K`, `LEM_U`, `LEM_D`, `LEM_T`, `LEM_STEPS`,
  `LEM_FLEX` override its parameters.
- Seed 42: final map at 512x256, big-river water reaches the sea 87% / a lake 12% / a pit
  1.5% (legacy 30 / 49 / 21); at 1024x512 78 / 22 / 0 (legacy 26 / 38 / 37). Land p99 2.5 km
  (legacy 1.7 km, at 1024 0.8 km).
- The particle erosion used to clamp heights to [-5000, 2000] m (every range became a 2 km
  mesa); it now only clamps to [-11000, 9000]. `apply_coastal_beaches` takes the tile size in
  km and only moves a tile by the share of it the beach strip covers (it used to flatten every
  coastal tile to ~130 m on 78 km tiles).

### Erosion
- The legacy erosion pass (`erosion::simulate_erosion`: below) is off by default since the
  landscape evolution does the physical erosion: it took ~45 s of a 512x256 world (~6 min at
  1024x512, and its GPU step sometimes hung) for little visible change (seed 42: slightly denser
  tributaries; it flat-filled every depression, so 17 lakes vs 23). A 512x256 world now
  generates in ~4 s. `--legacy-erosion` (or `LAB_LEGACY_EROSION=1` in terrain_lab) runs it; the
  legacy tectonics path and the erosion-preset comparison grids still use it.
- **Hydraulic**: Water droplets carve valleys and deposit sediment
- **Glacial**: Ice sheets using Shallow Ice Approximation (SIA)
- **Rivers**: Flow accumulation creates river channels

### Climate (`climate/`)
- Temperature: Budyko-Sellers energy balance per season (`ebm.rs`), heat transport D = 3.0
  W/(m2 C) (zonal land means within ~2-4 C of Earth's), continentality, lapse rate.
- Precipitation (`moisture.rs`): vapour evaporates from the sea (toward 80% humidity), rides
  the steering-level wind (3.5x the surface wind, ~a cell per step: one step = a cell
  crossing, so the scheme is resolution-independent), and rains out at (step / 9-day
  residence) x 3 x humidity^2 x ascent, where ascent comes from sea-level pressure (ITCZ and
  subpolar lows wet, subtropical highs dry; capped at 1.15), plus orographic rain where the
  wind climbs the smoothed terrain. Land returns 92% of its rain to the air (40% in the cold),
  which carries rain into the interiors. 120 steps, mean of the last 80, 2-pass smoothing,
  precipitable water 2.5 mm per g/kg. Seed 42: land mean ~850 mm/yr (median ~390), ocean
  ~900, equatorial ocean ~2000, subtropical ~600, storm tracks ~1700.
- Closed basins: water-body detection marks a lake endorheic when its inflow can't match
  open-water evaporation (PET - P); it then covers only the area that balance allows.
  `water_bodies::apply_salt_flats` turns the rest of the basin floor (below the spill level,
  not under water, mean above 0 C) into `SaltFlats` (seed 42: 112 tiles, e.g. Lake Noubroorn in
  a rain shadow). Salt flats are no longer rolled onto random low desert.
- Runoff (`climate::runoff_mm`): precipitation minus actual evapotranspiration on Fu's Budyko
  curve (w 2.6), PET = 300 + 50 T mm/yr (`pet_mm`). Water-body detection sums it as flow
  (`water_bodies::RUNOFF_MM_PER_UNIT` = 3500 mm per unit keeps the river thresholds'
  scale); endorheic lakes balance inflow against open-water evaporation (PET - P).
- `terrain_lab` with `LAB_CLIMATE=1` stops after the climate and prints land precipitation
  quantiles, zonal land/ocean precipitation, precipitation and wind by distance from the
  coast, Budyko runoff, the biome mix, and writes `precip.png`.

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
2. Run the program with a known seed: `cargo run --release -- --seed 42` (or `--dev` for the
   ~1 s development world); check rendering headlessly with `--tiles-snapshot` /
   `--watch-snapshot`
3. Navigate to relevant areas and visually verify the changes work correctly
4. For local map changes, embark (Z/Enter) and test at multiple z-levels with `<` and `>`
5. Only report completion after confirming the feature works as expected

Debug tools:
- `src/multiscale/debug_export.rs` - Export chunk data for analysis
- The tile viewer's window title shows the hovered tile and what is on it
