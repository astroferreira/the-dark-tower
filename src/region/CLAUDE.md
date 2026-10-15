# Zoomed regions (`src/region/`)

## Zoomed regions (`region/zoom.rs`)
- `--zoom X,Y` (world tile at the window centre) or
  `--zoom auto`; `--zoom-tiles N` (default 8), `--zoom-scale S` cells per world tile (default
  128, ~610 m/cell on a 512-wide world), `--zoom-erosion I` iterations (default 40).
  Writes `zoom_<seed>_<x>_<y>.png` and a 16-bit heightmap into `--map-output-dir`.
- Seamless: terrain is assembled from fixed world-tile chunks (each simulated with a one-tile
  border, cached in memory) blended with tent weights, so a cell's height depends only on its
  world position; separately generated regions match exactly (tested). Hydrology runs on the
  blended terrain with a margin (below). The date line too (2026-10-12): chunks are kept and
  simulated by their wrapped tile (`wrap` in `generate_zoom`), and the climate's and biome
  colours' warp noise is read at the wrapped position, so a region across the date line reads
  the same chunks as one on the other side (`--embark-seams`: the same embark from the region
  round 95,10 and round 1,10 of the dev world differs 0.00 m; it had been 17 m).
- The hydrology margin is 3 tiles on the audited terrain (2 on revision 1,
  `final_margin_tiles`): its relief drains in longer catchments, and with 2 regions 3 tiles
  apart differed on 3 of 8 test worlds (up to 3.4% of pixels; `region_swap_is_invisible`). A
  cold 8x8 region still takes ~0.6 s.
- Pipeline (per chunk): bicubic world heights + relief-scaled fractal detail -> world Bezier rivers carve
  valleys and inject upstream area where they enter -> shallow hollows filled -> implicit
  stream-power erosion + hillslope diffusion on a priority-flood drainage tree -> lakes and
  rivers (moisture-dependent threshold, width ~ 0.8 sqrt(A km2) m). ~7 s for a cold
  1024x1024 region; neighbouring regions reuse cached chunks.
- Render uses the climate colour LUT only (world biome colours and raw world climate are
  tile-blocky when upsampled; climate is smoothed over ~1 tile first).
- `--province-snapshot PREFIX` (dev embark, or `--tiles-center X,Y`) renders the colony's
  theatre: the zoomed region around the embark, a 250 km ring, lines to its three nearest
  living places with the days to walk each (25 km/day x 1.3 winding), and prints them. See the
  play-scale decision in `docs/colony/ROADMAP.md` (Update 5).

## The site layer (`region/plan.rs`, `landforms/`, `sites/`, `debug.rs`; ROADMAP, 2026-10-15)
- `ZoomRegion` carries, beside heights and water, the site layer: `landform` (a `plan::Landform`
  per cell: cliff, doline, moraine, lava field, dune, floodplain, beach, bog... `None` is plain
  ground), `rock` (the `RockType` at the surface), `places` (`plan::Place`: kind, name, epithet,
  cause, description, contents, position in absolute region cells, footprint, floor height, the
  cave it opens into, a rank) and `caves` (`plan::CaveGraph`: nodes and passages in absolute
  cells and metres). Absolute region cells: `x = world tile x * cells_per_tile + i`, the frame
  the embark already uses; a region converts with `x - world_x0 * s`.
- Landforms (`landforms/mod.rs` fixes the order; one file per process: `rock`, `karst`,
  `glacial`, `volcanic`, `aeolian`, `fluvial`, `coast`, `wet`): `shape_before_erosion` and
  `shape_after_erosion` run inside `simulate_chunk` on the bordered chunk grid (`ChunkGrid`) and
  move heights as pure functions of world position, so chunk overlaps agree; `classify` runs on
  the blended region with its margin (`RegionFields`: heights, filled heights, receivers,
  accumulation, climate, `slope`, `tile_of`, `abs_cell`) and labels every cell. Bump
  `landforms::SHAPE_VERSION` when a shape pass changes heights: it is in the chunk cache's file
  names (`v1s<SHAPE>_...`).
- Sites (`sites/mod.rs` fixes the order: `water` the world's springs and waterfalls, `natural`
  wonders, `prehistory` the ages before, `caves` the graphs, then `names`): `sites::plan(fields,
  land, rock, Core)` plans the places of the window's core tiles from the tile and its neighbours
  (the margin of 3 tiles guarantees them), so any window plans a tile's places the same way.
- `--zoom X,Y` also writes `zoom_<seed>_<x>_<y>_landforms.png` (classes by family over a faint
  hillshade, places as red crosses) and `_rock.png`, and prints the landform mix and the counts
  of places and caves (`region::debug`). With every pass a stub the output is bit-identical to
  before the layer (checked on the dev cave site's four images).
