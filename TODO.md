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

## Housekeeping
- [ ] Remove or silence long-standing compiler warnings in `history/simulation/step.rs`,
      `geomorphometry.rs`.
- [ ] `src/region/generator.rs`, `cache.rs`, `rivers.rs` (the 64x64 `RegionMap` of the legacy
      explorer) are dead outside `explorer.rs` except `rivers::calculate_river_crossings` and
      `handshake.rs` (the rock stack the embark reads): move what the embark needs out, mark
      the rest legacy or delete it with the explorer.
- [ ] The `worktree-agent-a05fd18863756a8c4` branch (one charter art commit) and the
      `.claude/worktrees/` checkout: merge or drop when charter work resumes.
