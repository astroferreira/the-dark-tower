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

## Landforms: rock, karst and ice (`landforms/rock.rs`, `karst.rs`, `glacial.rs`; Stage 2, 2026-10-15)
- The beds (`rock::Beds`): each tile's handshake rock stack is laid as horizontal beds with a
  dip hashed per tile (40-300 m across half a tile), their top `TOP_M` (60 m) above the tile's
  mean height, so hillsides cut down through the layers and valleys show the beds under the
  plateau's; the border between two tiles' beds is bent by position-keyed noise (0.18 tiles) so
  no rock boundary follows the grid. `surface_rock` is the bed at the cell's own height, thin
  surface sediment (<= 8 levels) skipped as regolith, alluvium (`Sediment`) on low ground where
  the world's hardness is under 0.25. `Beds::rock_at(x, y, z)` is the same reading at any
  height, for the embark to take up (Stage 6; today it drapes the stack under its surface).
- Rock classes (`rock::classify`): Cliff where the slope passes `0.38 * (1.6 - 0.8 hardness)`
  m/m at 610 m cells on a run of 2+ cells, Scree 3 cells down the receiver chain, Outcrop on
  convex crests of hard rock, Tor on old worn granite summits (hardness > 0.8, |stress| < 0.1,
  one in two), in dry country (moisture < 0.3) Mesa (a hard bed over a softer one, flat top, 40 m
  over the ground within 6 cells), SlotCanyon (a channel cut through the cap), Hoodoo (the soft
  bed under a mesa's rim, one cell in three), Badland (shale on slopes over 0.1). Slope
  thresholds scale by `sqrt(610 m / cell)` (`rock::scale`) so the dev world's 3.26 km cells read.
  `rock::shape_*` are stubs (cliffs are not yet cut square).
- Karst (`karst`), on carbonate rock (`plan::is_carbonate`: limestone) with moisture over 0.25:
  dolines on a jittered lattice 8 cells apart (radius 1-3 cells, 5-30 m deep, 70% of points
  scaled by moisture), cut by `shape_after` on off-channel ground (acc below the river
  threshold, slope under 0.1) and labelled by `classify` over the whole bowl; a cut doline holds
  water until the crop (`generate_zoom`) drains every Doline cell, since dolines drain
  underground; a lattice point in a real lake is never taken for one (the ring of cells just
  outside the footprint must be dry). Tower karst in the wet tropics (temp > 20, moisture > 0.6:
  towers 30-100 m on a 5-cell lattice, `1 - d^4` sides), cenotes on flat low (< 200 m) warm
  limestone (10-20 m pits, 12-cell lattice; they keep their water), Pavement on gentle plateau
  ground 60 m over the ground within 10 cells, DryValley on valley lines (acc over half the
  threshold) without a channel, KarstGorge where a channel's banks rise 80 m within 3 cells,
  Sink on the first limestone cell a channel enters and Resurgence on the last it leaves (both
  on the limestone, so a cave can hold them; `sites::caves` reads them).
- Ice (`glacial`): `glaciated(world, tile)` gives `once` (the ice age reached ground colder than
  8-2 C past 35-45 degrees, and mountains colder than 2 to -4 C anywhere) and `now` (under 0 to
  -4 C); `IceMap` samples `once` smoothly between tiles and its gradient gives the ice's flow.
  Before erosion: the terminal moraine (a lumpy ridge up to 22 m where `once` crosses 0.5, bell
  width 0.06) and drumlins (ellipses 3 x 1.2 cells along the flow, 10-25 m, on flat ground under
  600 m) are raised, never on the local low line (cells within 5 m of the lowest within 3
  cells): a ridge across a valley floor dammed the drainage (the dev cave region gained 13,000
  lake cells; with the gap it gains 500). After erosion: trunk valleys (catchment over 0.5% of a
  tile) widened to U sections (parabolic flanks over a half-width of 2-10 cells by catchment),
  cirque bowls (45 m) at valley heads above 1,500 m with 150 m of relief within 4 cells, kettle
  holes (3-8 m) on a 7-cell lattice in the outwash (`once` 0.15-0.45). Labels: Glacier where the
  mean at height is under -2 C in valleys and on caps, Snowfield on steep cold ground and under
  0 C elsewhere, Cirque, Tarn (a cirque's lake), Arete (convex ridges over 1,500 m), UValley on
  the widened flanks, Moraine on the front's ridge, Outwash and Kettle beyond it, Erratic one
  cell in 250 of low once-glaciated ground, PatternedGround on cold flats (-1 to 2 C).
- Seams: every shape pass is a pure function of position (lattices by absolute cell, the ice
  map by tile), and `shape_after` takes the chunk's drainage from the evolution's last step
  instead of routing again. Tests: `chunks_cut_the_same_dolines`, `chunks_shape_the_same_ice`
  (two chunks a tile apart agree bit for bit on the overlap inside the chunk border),
  `limestone_plateau_gets_its_karst`, `labels_stay_on_cold_land`, `ice_follows_the_climate`.
- Cost and debugging: `PLANET_REGION_TIMING=1` prints each phase of `generate_zoom` (the dev
  cave region cold: chunks 17.3 s, blend 0.2, hydrology 0.3, landforms 0.5, sites 0.0; warm
  1.2 s in all; the scaffold alone was 17.6 s cold, so the passes cost nothing visible).
  `PLANET_SHAPE_SKIP=karst,glacial_before,...` leaves shape passes out (turn the chunk cache off
  with it). `--zoom` prints "Lake cells by landform". `SHAPE_VERSION` is 2.
- Seed 42 at 512x256 under the Sun: Mount Buld's massif (392,97) glacier 52%, snowfield 20%,
  outcrop 4%, glacial valley 3%, moraine 0.3%, cirque 0.2%, tarn 0.1%; Mount Fielein (18,106)
  the same picture; the Falls region (177,133) doline 0.5%, pavement, dry valleys, a sink and a
  resurgence; the dev cave region (42,32) doline 0.6%, pavement 0.5%.
