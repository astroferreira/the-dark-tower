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
