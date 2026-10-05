# Playable areas / embarks (`src/local/`)

## Playable areas (`src/local/`)
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
- `--dev-embark` (implies `--dev`) opens the viewer already embarked at a fixed dev-world site
  (`pick_dev_embark` in `main.rs`: temperate, river, woods, high ground, a town 1-3 tiles away);
  with `--headless` it writes `dev_embark_*.png`. Zoom chunks are cached on disk
  (`region/zoom.rs` `mod disk`, `~/.cache/planet_generator/chunks`, `$PLANET_CHUNK_CACHE`, empty
  = off; bump `FORMAT` when `simulate_chunk` changes), so a second run takes ~1.2 s instead of
  10-15 s.
- Houses record their footprints (`LocalMap::roofs`, `houses`) for roofs in the ink surface view.

## Sites worth settling (`local/site.rs`)
- `furnish` (end of `generate_local`) adds what a first camp needs within 48 cells (~100 m) of
  the centre and nature left out: a spring and a pool on a dry site (`Feature::Spring`), a
  grove of the local trees when there are fewer than 20 (the hut needs logs), an outcrop of bare
  rock and boulders when there is no stone, berry thickets when there are few shrubs, and a
  landmark: graves (`Feature::Grave`, 3 + the named dead per battle) where the history fought on
  this world tile (`RegionLore::battles`), else a standing stone (`Feature::Stone`) if nothing
  else is there. Spots are the free cells (dry floor, unbuilt, no feature) with the most room,
  chosen by hash: deterministic.
- `report` lists what a site holds (water, wood, stone, berries, soil, game, graves, bones,
  stone, buildings); `--local-snapshot` prints "Site (N kinds): ...", and walking mode shows it
  in the title, recomputed when the embark box moves 24 cells (~30 ms; not tried by hand yet).
  Seven dev sites hold 5-7 kinds each.
