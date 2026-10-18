# TODO

Short-term tasks for the generation roadmap (`ROADMAP.md`: the region view and the embark map
as places worth exploring). Colony, charter, genesis and adventure tasks are parked in
`docs/colony/TODO.md` and on the board; nothing there is worked on in this period.

## Stages 1-5: done (2026-10-16; notes in `src/region/CLAUDE.md`, `src/local/CLAUDE.md`, `src/tiles/CLAUDE.md`)
Survey (`--region-survey`, `--place-census`, `--survey-find`, `scripts/gallery.sh`), landforms
(rock, karst, ice, fire, wind, river, coast, wet ground), the region in ink (ground washes and
marks, glyphs, labels, the key), places (natural wonders, the ages before, names), caves as
graphs planned in the region and carved in the embark, caverns as rooms and passages.

## Stage 6: the embark worth a look, and exploring it (done 2026-10-16; notes in `src/local/CLAUDE.md`)
The ground from the landform classes (`local/ground.rs`), the waterfall's step, every region
place carved at its column (`local/sites.rs`), `--views-agree X,Y` (seed 42: 96% and 95% of a
window's places shown in their embarks; the dev world 88%) and its test, P in the walker,
`PLANET_EMBARK_PLACE` for places and grounds, the gallery of twelve embarks
(`scripts/gallery.sh`).

## Stage 6 follow-ups
- [ ] Fords and crannogs miss where the river or the lake does not cross the embark's middle
      (the place sits on the cell, the water a few hundred metres off): put the ford on the
      channel's nearest column and the crannog on the lake's nearest shallows.
- [ ] A place inspector in the walker and the embark (name, kind, why here, what it holds,
      what was found); "embark here" from the world map's wonders; a world code that lands on
      a place.
- [ ] The river at 2 m beyond the falls and the ford: riffles and pools, gravel bars on the
      inside of bends, undercut banks, the levee as a ridge.
- [ ] The coast at 2 m: rock pools, wrack, the cliff's ledges.
- [ ] Mounds read as stepped diamonds at 2 m levels; a barrow wants a rounder profile (half
      levels are not drawn) or a drawn hummock in the ink.
- [ ] Lynchets and cairn lines on hillsides; the rock shelter's hearth; the midden's shells as
      a material.
- [ ] A blind review of the gallery (7+/10 is the exit).

## Follow-ups from Stages 4-5
- [ ] `--survey-find` lacks a limestone plateau, a glaciated valley and a rift lake on seed 42.
- [ ] The deep caverns: an underground lake fed by the karst's shafts, a river that resurges,
      a chasm; ice and talus caves; mine adits that meet natural passages.
- [ ] Cave mouths: the natural planner's sinks and cliff mouths should be the graphs' own
      mouths (today a graph's mouth joins a natural one only within 2 cells).
- [ ] A section through a column (`PLANET_LOCAL_COL`), since a cave's gallery runs off the
      embark along one.

## Terrain and regions (carried over)
- [ ] Sub-tile detail (valleys narrower than a tile) only exists in zoomed regions; if the
      world map wants more texture, run the landscape step at 2x and downsample.
- [ ] Polar land is stretched across the whole map width (equirectangular); consider how the
      tile viewer treats the polar rows (labels, the start screen preview).
- [ ] Zoomed regions straddling the date line (x wrap) match since 2026-10-12 for chunks and
      embarks; check the region ink's tree grid and labels there too.
- [ ] Ore veins are hard to see on z-level views at default zoom: an "ore only" overlay or
      brighter flecks.
- [ ] Hover info, labels and a legend for the region walker and embarks (Stage 3).
- [ ] Caused landform biomes at world scale: `is_caused_biome`'s remaining kinds either get a
      cause or stop appearing (karst, volcanic and thermal kinds move to the region's site
      layer in Stage 2/4).

## Stage 7: the violent land (`ROADMAP.md`, `src/upheaval.rs`; 2026-10-17)
- [x] The record (`upheaval::Impacts`, `Faults`, `provinces`), from the star system
      (`StarSystem::upheaval`); `--upheaval`, `--no-upheaval`, `PLANET_UPHEAVAL`,
      `--upheaval-report`.
- [x] World map: the largest craters and the flood basalts before the climate; lava down the
      valleys (`run_lava_downhill`); real crater lakes instead of planted ones (`lore/focal.rs`).
- [x] Region: craters (`landforms/impact.rs`), faults (`tectonic.rs`), slides
      (`mass.rs`, on the blended region), maars, traps, fissures, lahars (`fire.rs`), salt sinks
      and thaw lakes (`sinking.rs`), scablands and abandoned channels (`flood.rs`), crevasses,
      avalanche paths, burns (`hazard.rs`); places (`sites/violent.rs`); ink and glyphs.
- [x] Embark: ground rules for every class, dead trees in burns, carved meteorite craters,
      debris, fault steps, sinkholes, crown holes, crevasses, quicksand, gas hollows, tar pits.
- [x] History: quakes, landslides, tsunamis, eruptions where the ground has them
      (`history/geology.rs`; `EventType::Landslide`, `Tsunami` appended).
- [x] The chronicle's quakes and slides in the region: `WorldData::set_ground_events` (set
      by `load_world` and `main` after the history), a slide on a struck town's tile carries the
      chronicle's words ("the hillside fell on Ploockwey in the year 410"), a fault scarp names
      its last great rupture.
- [x] Stratovolcanoes: the volcano pass's draws (`(rng >> 33) / u32::MAX`) never passed 0.5,
      so no world had one; an upheaval world draws over [0, 1) (`find_volcano_locations_with`).
      Seed 7: 23 of 62; 24,24 has lost its flank, 272,88 and 232,96 carry lahars and maars.
- [x] The summit craters (`volcanic::shape_after`, missing from the shape order since Stage 2)
      are cut on every world (`landforms::SHAPE_VERSION` 4).
- [x] The faults meet across the date line (`tectonic::FaultField::at`: x read wrapped, the two
      sides blended within 2 tiles of the line; `faults_meet_across_the_date_line`).
- [x] Decided (2026-10-18): the Sun's worlds keep their volcano kinds' old draws (no
      stratovolcano). They are the Earth reference and the dev world every pinned test stands on;
      the corrected draws (`find_volcano_locations_with`) are the upheaval worlds'.
- [x] (colony) Settlers walked for fishing spots and loads a building had gone up over (stuck 6
      times on the dev camp since Stage 6's ground moved the layout): `nearest_fishing` and the
      haul choice skip what `nav::passable` refuses; `PLANET_DEBUG_PATH_MAP=1` prints the cells
      round a failed walk. `seven_settlers_live_thirty_days_unattended` passes again (its
      spoilage check accepts a smokehouse in place of the rack).
- [x] (colony) The tests pinned to the dev world before Stage 6 re-pinned (2026-10-18; from a
      365-day record of dev seeds 1-40 and sweeps of sites): the still 11/8, the militia 11, the
      den and pyre 24/7, tombs 31, armour 19/11/8, children 18, an inherited grudge 3, the mine
      seed 9, the river camp 70,7, a camp without stone 6,36, the dwarves' hall 24, the plan 6,
      the depth trial 9, the woodpile 5, crafts 5, the relic, its seeker and the fell mood 1; the
      explore trial accepts a tomb robbed as its find.

## Tools (the lab, `src/lab/CLAUDE.md`; plan of 2026-10-17)
- [x] The agent harness: `planet_lab` over MCP and the shell (worlds, tiles, regions, places,
      renders) and the history debugger (query, event, why, entity, stats, export, branch).
- [ ] Sieges and their ends carry no location: record the town's tile.
- [ ] A treaty recorded as the cause of a raid between the same peoples (`infer_cause`).
- [x] The history explorer (`history_explorer`, `--history-explorer DIR`): legends reader,
      timeline, causal graph, family trees, the web of peoples, the map through the years,
      checks, compare.
- [ ] Heirs born to parents of 2-11 years, in chains (explorer checks: 26 on dev, 163 at
      256x128); no one has more than two children (houses are single-heir lines).
- [ ] Sieges, war declarations, quests, treaties and miracles carry no place (0 of 364 sieges
      at 256x128).
- [ ] A town gets two trade routes to the same town in successive years (Southview and
      Ravenpool, dev, 261 and 262).
- [ ] Wars between peoples across the ocean (long lines in the web's geography layout): check
      the war's reach rule.
- [ ] Interventions in `history_branch` (a figure dies, a town falls at the fork).
- [x] Colony tools (`colony_*`, the recorder) and the colony explorer (days, activity, needs,
      map replay, bonds, works, checks, compare).
- [x] A colony on a world loaded from its file was not the fresh world's: the chronicle's
      indexes were not rebuilt on load (`Chronicle::reindex`).
- [ ] (colony, paused) Diggers deep in the mine eat at 85% hunger and then climb ~3.5 h: 100%
      hunger with 50-90 meals stored (dev, 10 times in 60 days; Jofulot Day 33 17:28).
- [ ] (colony, paused) A settler in a building's footprint when it is stamped is moved 11 cells
      off and back onto their old path six minutes later (dev Day 6 12:08, Renildpun).
- [ ] `--sim-*` trials with `--load-world` drop the saved history (they force `--dev`; `main`
      drops a loaded history when `history_years` is set).
- [ ] Tunables and structured causes (a place's `Why` with its factors), then the Workbench
      window; the seed census and visual diff; place kinds as data; the set-piece editor.

## Housekeeping
- [ ] Remove or silence long-standing compiler warnings in `history/simulation/step.rs`,
      `geomorphometry.rs`.
- [ ] `src/region/generator.rs`, `cache.rs`, `rivers.rs` (the 64x64 `RegionMap` of the legacy
      explorer) are dead outside `explorer.rs` except `rivers::calculate_river_crossings` and
      `handshake.rs` (the rock stack the embark reads): move what the embark needs out, mark
      the rest legacy or delete it with the explorer.
- [ ] The `worktree-agent-a05fd18863756a8c4` branch (one charter art commit) and the
      `.claude/worktrees/` checkout: merge or drop when charter work resumes.
