# TODO

Short-term tasks for the generation roadmap (`ROADMAP.md`: the region view and the embark map
as places worth exploring). Colony, charter, genesis and adventure tasks are parked in
`docs/colony/TODO.md` and on the board; nothing there is worked on in this period.

## Stages 1-5: done (2026-10-16; notes in `src/region/CLAUDE.md`, `src/local/CLAUDE.md`, `src/tiles/CLAUDE.md`)
Survey (`--region-survey`, `--place-census`, `--survey-find`, `scripts/gallery.sh`), landforms
(rock, karst, ice, fire, wind, river, coast, wet ground), the region in ink (ground washes and
marks, glyphs, labels, the key), places (natural wonders, the ages before, names), caves as
graphs planned in the region and carved in the embark, caverns as rooms and passages.

## Stage 6: the embark worth a look, and exploring it (now)
- [ ] The embark's surface from the landform classes: outcrops, cliff faces off the mountains,
      scree, boulder fields, dunes, playa, beach, bog, reed beds, gravel bars, fords, oxbows.
- [ ] Carve the places: barrow mounds, stone circles, standing stones, hill-fort ramparts,
      enclosures' banks, lynchets, cairns, kill-site bones, middens, flint-mine shafts; cave
      shafts the colony can climb.
- [ ] `views_agree`: a place's region cell is its embark's carving (`PLANET_EMBARK_PLACE`).
- [ ] The viewer: place labels and an inspector line in walking mode; "embark here" on a place.
- [ ] The gallery of twelve sites (region, surface, a level, the section) looked at.

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
