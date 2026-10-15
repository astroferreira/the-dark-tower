# TODO

Short-term tasks for the generation roadmap (`ROADMAP.md`: the region view and the embark map
as places worth exploring). Colony, charter, genesis and adventure tasks are parked in
`docs/colony/TODO.md` and on the board; nothing there is worked on in this period.

## Stage 1: survey and measure (now)
- [ ] `--region-survey X,Y`: landform mix, drainage density, Horton's bifurcation ratio, Hack's
      exponent, hypsometric integral, slope histogram, geomorphon shares, the places with
      causes, frame times; a `survey_<seed>_<x>_<y>.png` (height, class, slope, places).
      Reference: `docs/reference/numerical_tests.md`, `realistic_map_indicators.md`.
- [ ] `--place-census`: places by kind and cause over a world, per region, regions with none.
- [ ] `scripts/gallery.sh`: the standard sites as one contact sheet per seed (region 4 px,
      embark surface, one level below, the section).
- [ ] Choose and pin the standard sites on seed 42 at 512x256 under the Sun (a limestone
      plateau, a young volcano, a glaciated valley, a desert interior, a floodplain and its
      delta, a hard coast and a soft one, a bog, a rift lake, a Mediterranean hillside); write
      them into `CLAUDE.md`'s standard seeds.
- [ ] `tests/region.rs` and `tests/places.rs`: pins of today's numbers; `views_agree` (a place
      listed by the survey is drawn at its region cell and carved at that cell in the embark).
- [ ] Bring the world's springs and waterfalls (`underground_water.rs`, `TileWaterFeatures`)
      down to the region: the first "caused, not rolled" places, and the first `views_agree`
      cases.

## Stage 2 first cuts
- [ ] Surface rock per region cell from the handshake rock stack and the hardness map, with a
      dip; cliffs and scree where slope and hardness pass a threshold.
- [ ] Karst on limestone: dolines and a sinking stream that resurges at the plateau's foot
      (the region's drainage routed through the ground).
- [ ] Hot springs, geysers and fumaroles placed by young volcanism and stress instead of
      `biomes.rs::should_spawn`.

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
