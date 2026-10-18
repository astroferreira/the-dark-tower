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
  names (`v1s<SHAPE>_...`; 4 since the summit craters, 2026-10-17).
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

## Landforms: fire, wind, river, coast and wet ground (`landforms/volcanic.rs`, `aeolian.rs`, `fluvial.rs`, `coast.rs`, `wet.rs`; Stage 2, 2026-10-16)
- Volcanic (`volcanic`), from `WorldData::volcanoes` (tile, kind, active) and `lava_map`: a
  vent's edifice reaches 4 tiles (shield), 2.5 (stratovolcano), 3 (caldera); "young" volcanism
  is an active vent within 2 tiles or lava on the tile. Before erosion: cinder cones on a hashed
  lattice 2.5 km apart on the flanks (0.3-1.8 tiles out, 350-900 m across, 25-80 m high, 35% of
  points on an active vent's flanks, 18% on an old one's) and the lava's pressure ridges (ridged
  noise up to 12 m over the lava's share of the ground). After erosion: the summit's crater (a
  600 m bowl 80 m deep on a stratovolcano, 2.5 km / 60 m on a shield, 0.3 tile / 150 m with a
  sunken floor for a caldera) with a low rim. Labels: Crater / Caldera, CinderCone, LavaField
  (lava share > 0.5, slope < 0.3), Fumarole (within 0.6 tiles of an active vent on steep
  ground, by noise), ThermalGround (stress > 0.15 within 3 tiles of young volcanism, gentle
  ground, by noise: the caused spots the site planner's hot springs, geysers and mud pots take),
  AshPlain (gentle ground within 1.5 tiles downwind of an active vent), ColumnarBasalt (old
  basalt on slopes over 0.12). `wind_dir(lat)`: the belts' prevailing wind (trades from the
  east to 30 degrees, westerlies to 60, polar easterlies beyond, blended over 4 degrees); the
  climate's wind field is not kept on `WorldData`.
- Wind (`aeolian`), in dry country (moisture < 0.2 at the cell and a desert biome, desert soil
  or moisture < 0.15 on the tile): a dune field is a wave in absolute cells along (40%) or
  across (60%, by 4-tile block) the wind, 2 km between crests, up to 30 m, warped by noise and
  scaled by a sand-supply noise (`SAND_MIN` 0.35), raised before erosion on flat ground (slope
  < 0.03); labelled Dune, or SandSea where the tile and its eight neighbours are all desert.
  Yardang: streaks along the wind cut into Shale / Sediment; DesertPavement: flat stony ground
  on hard rock off alluvium; Playa: the world's salt-flat tiles and lakes in moisture < 0.15.
- Rivers (`fluvial`), labels only (the channel and its drainage are known on the blended
  region; the embark carves the levee, the oxbow and the terrace from them in Stage 6):
  `channels(f)` gives every cell its nearest channel downstream, its height above it (HAND)
  and the steps to it, by path compression down the receiver chain; a channel is the region's
  rule (acc > 0.02 tile / (0.25 + moisture)). Floodplain where a big river (catchment 0.3 tile)
  runs at under 0.004 m/m: HAND < 5 m within a meander belt of 12 m per sqrt(km2) of catchment;
  Levee the first step from a great river (2 tiles), Backswamp the belt's far half under 2 m,
  Oxbow arcs (1.2 km) hashed at 0.4% of a great river's floodplain cells, Terrace at 8-40 m
  HAND within three belts on slopes under 0.03. On the channel: Gorge where both banks rise 60
  m within three cells across the flow, Waterfall where one step drops 8 m (scaled by the
  cell) and 2.5x the reaches above and below, Rapids a 3 m step on a gradient over 0.005,
  Braided below mountain tiles (3x3 relief over 1,500 m) at 0.003-0.02 m/m, Fan a 2 km cone
  below an apex where a steep tributary meets gentle ground (relief 800 m). Mouths: Delta where
  a big river meets the sea (radius 3 sqrt(tiles) cells, the whole fan on an open coast, three
  lobes in sheltered water: the fetch is the sea in a half-disc of 12 cells), Estuary (cells
  under 4 m) at smaller mouths.
- Coasts (`coast`): the shore is land within 2 cells of the sea; openness is the sea's share of
  the 21x21 cells round a cell (a summed-area table). After erosion, hard shores (hardness >
  0.6) facing the open sea are raised toward the ground 3-4 cells inland (0.6 and 0.8 of it), so
  the sea meets a step. Labels: SeaCliff (hard, open > 0.3, slope over 0.05 at 610 m, 12 m up),
  Stack (one shallow sea cell in twelve beside a cliff), Beach (soft, flat, under 12 m),
  SaltMarsh / Mangrove (under 3 m, flat, sheltered; mangrove over 20 C, marsh over 5 C),
  TidalFlat (sheltered sea within 3 cells of land, shallower than 2 m), Bar / Spit /
  BarrierIsland (hashed 8-cell blocks of shallow soft open shore 2-4 cells out, 35% of blocks;
  stretches under 3 cells are bars, the rest spits or islands) with Lagoon behind them,
  CoastalDune 2-3 cells behind a beach where it is dry or in the westerlies, RaisedBeach a
  5-20 m bench 2-5 cells in on hard rising coasts (stress > 0).
- Wet ground (`wet`, last in the order): Bog on flats (slope < 0.02) colder than 10 C and
  wetter than 0.6 (14 C and 0.45 on peat soil), Fen within 3 steps of a channel on wet flats
  under 3 m HAND, Marsh on a lake's margin, ReedBed in a lake's 0.5-2 m shallows.
- Tests (one per file at least): chunk-overlap agreement for the cones and craters, the dunes
  and the cliff step; a crater at the summit and cones on the flanks and nothing volcanic far
  from a vent; dunes only in the desert; a great river builds a floodplain and a delta;
  channels resolve every cell; soft shores are beaches and hard steep ones cliffs; cold wet
  flats are bog with marsh and reeds round a lake. Synthetic planes need a little hashed
  relief, else every column drains straight down and each shore cell is a channel.
- Dry ground holds no water (`generate_zoom`'s crop): a doline, a playa and a dune field's
  hollows lose their standing water, and a stream that reaches a playa ends there (its flow
  still crosses the bed; on the drained bed the routing had drawn ruled diagonal channels). The
  Mistlight desert (124,153): 333,000 lake cells to 12,000, river cells 21,500 to 7,100; the
  salt flats at the date line (508,123): 218,000 to 272. The aeolian pass gates on `desert_at`
  and `salt_at` (the per-tile desert and salt-flat tests read bilinearly through a 0.18-tile
  warp, `tile_field_at`): per-tile gates had switched the dunes and playas along the tile grid.
  A river's own cells are never dunes, so a river keeps its course through a sand sea.
- Seed 42 at 512x256 under the Sun, after all eight passes: the active caldera at 384,88 lava
  field 26%, glacial valley 9%, snowfield 5%, cinder cones 1.2%, moraine 1%, rapids 0.9%, gorge
  0.5%; the Mistlight desert (124,153) playa 35%, sand sea 28%, dunes 13%, pavement 4%; the
  great river's mouth (382,62) fen 6%, marsh 2%, coastal dunes 1.6%, lagoon 1.1%, beach 0.9%,
  estuary 0.6%; the Falls region (177,133) marsh 2.5%, terraces 2.3%, backswamp 1.3%,
  floodplain 1%, levees 0.7%, oxbows; the dev cave region terraces 2.4%, marsh 2.1%, bog 1.3%.

## Survey (`region/survey.rs`; ROADMAP Stage 1, 2026-10-16)
- `--region-survey X,Y` (with `--zoom-tiles`, `--zoom-scale`, `--zoom-erosion` as `--zoom`)
  generates the region and prints the land, river and lake cells, the landform and rock mix,
  the drainage density (km of channel per km2 of land), the streams by Strahler order and
  Horton's bifurcation ratio, Hack's exponent (log L against log A over the window's outlets
  with 50+ cells), the hypsometric integral (Pike and Wilson's elevation-relief ratio), slope
  percentiles in degrees, the geomorphon shares (Jasiewicz and Stepinski's ten forms, lookup 8
  cells, flatness 1 degree), every place with its cause and the caves, the phase timings
  (`zoom::last_timings`), `check`'s violations, and how far the window agrees with the one three
  tiles east on their overlap (landform, rock and places); it writes `survey_<seed>_<x>_<y>.png`
  (four panels: hillshaded height, the landform classes, the slope, the places lettered by
  kind). The realistic bands (`BIFURCATION_BAND` 3-5, `HACK_BAND` 0.5-0.6, `HYPSOMETRIC_BAND`
  0.3-0.6, `DRAINAGE_DENSITY_BAND` 0.1-1 km/km2) come from `docs/reference/`. Nothing in it
  runs on the generation path.
- `--place-census [N]` samples N windows (default 24; 0 every window) centred on land tiles
  chosen by hash, and prints the places by kind and by age, the windows with none, the cause
  coverage, each kind's median spacing and `check`'s violations. `--survey-find` scans the world
  for the gallery's standard sites (a limestone plateau, a young volcano, a glaciated valley, a
  desert interior, a big river's floodplain and its delta, a hard and a soft coast, a bog, a rift
  lake, a Mediterranean hillside) and prints the sentence for CLAUDE.md.
- `check(region)`: every place inside the window and on land (unless its kind stands in water),
  a Waterfall on a channel with a 2 m drop within three steps, a Spring by a channel head or at a
  slope foot, a CaveMouth of an underground kind, a `cave` index pointing at a cave whose Mouth
  node lies within 2 cells. Kinds without a rule pass.
- `sites/water.rs`: the world's springs and waterfalls (`WorldData::underground_water`) brought
  down as the first places: per core tile a Waterfall at the lip on the tile's main stem (channel
  cells with a tenth of the tile's largest catchment) with the greatest fall within three
  receivers, its pool's height, a rank by the world's drops and a cause from the hardness (a
  caprock ledge, hard rock, the upland's edge); up to two Springs where the spring's kind fits
  (artesian and karst at a slope foot, a seepage spring under a scarp at a foot, else at the
  channel head with the largest catchment), thermal springs left to `natural.rs`.
- Numbers (seed 42 under the Sun): the volcano 16,104 drainage density 0.052 km/km2,
  bifurcation 8.05, Hack 0.45 over 70 basins, hypsometric 0.45, slope p50 2.8 degrees, geomorphons
  slope 55% / spur 12% / hollow 8%; the floodplain 399,67 flat 99.5%, hypsometric 0.59, Hack 0.17
  (basins of a plain); the hard coast 395,116 bifurcation 9.9, Hack 0.25; the Falls region
  177,133 bifurcation 5.9, Hack 0.28, hypsometric 0.55. Drainage density sits under the band and
  the bifurcation ratio over it on every window: only permanent streams are drawn (2% of a tile's
  area) and first-order streams are many. The dev world at 3.26 km cells is flat to the
  geomorphons (100%) and no measure of realism. Two windows three tiles apart agree 99.7-100%
  on landform and rock and on nearly every place (136 of 137 on the dev cave region, 156 of 158
  on the Falls region): the fluvial labels and the places that read the channel distance can
  differ where a receiver chain leaves the margin.
- Cost: the survey itself ~0.3 s on an 8-tile window; `classify and plan` ~1.1 s with the places.

## Places: natural wonders and the ages before (`sites/natural.rs`, `prehistory.rs`, `names.rs`; ROADMAP Stage 4, 2026-10-16)
- Every planner works on a `Scan` (`natural.rs`): the blended fields and labels, a `ScanData`
  (sea, lake, river width, bog, the running minima and maxima over a short (750 m) and a long
  (3 km) radius: `rel_s`/`rel_l` the height over the lowest ground about, `prom_s`/`prom_l` the
  height under the highest, van Herk running extrema; summed-area tables of sea, lake, river
  and bog: `Sat::count`/`share`) made once a window and handed from planner to planner
  (`ScanData::take_or_new` / `hand_over`), and `TileFacts` of the world tile (climate, soil,
  fertility, ore within 3 tiles, volcanoes, `once_cold`, `desert()`). Rules: `best(tile, test)`
  the best-scoring cell of the tile (every other cell, `STRIDE`), `several(tile, max, test)`,
  `put(out, kind, i, j, cause)` keeps places `MIN_APART` (3 cells) from each other and
  `MIN_APART_SAME` (8) from their own kind. Rates are per tile by hash (`one_in(tile, salt,
  n)`, `lattice(tile, salt, n)`: one tile in an n x n block), never a roll.
- Natural wonders (`natural.rs`, after the world's springs and waterfalls of `water.rs`): a
  gorge (one tile in 5, walls 100 m within 3 cells on a channel), a tarn (one in 5, 150 m
  walls), a hot spring only on `ThermalGround` or one tile in 2 near young volcanism, geysers
  and mud pots there, a natural arch and sea stacks on hard open coasts, blowholes, erratics
  (one in 10 where the ice reached) and boulder fields, a salt spring, a tar seep on sediment
  near oil, a petrified forest in old ash, a fossil bed in a sedimentary cliff, amber on a cold
  wooded shore, a meteorite crater (one tile in 4,000), an oxbow (one in 8 beside a slow river
  40 m+ wide, on an `Oxbow` cell first), a tor on worn granite, a limestone pavement, a giant
  tree, a crater lake in a real crater; cave mouths at limestone cliff feet (one tile in 8), at
  a sink or resurgence (one in 6), a doline or cenote (one in 8) and in sea cliffs (one in 8).
- The ages before (`prehistory.rs`): per tile `Ages::of` says who lived there: hunters (not
  under ice, warmer than -5 C), farmers (fertility 0.3+, over 3 C, moisture over 0.18),
  builders of stone (farmers with 15% of the tile's land on hard rock), the first metal (every
  farmed tile: it came by trade; its workings only with ore within 3 tiles), the last age
  (fertility 0.55+). One farmed tile in five is a **farmed valley** (`VALLEY_ONE_IN`), where the
  works cluster: the barrow on the crest, the enclosure on the terrace, the fort on the spur,
  the track along the ridge; the other tiles carry a stray thing or two (`gate(ages, tile,
  salt, valley, stray)`: one in `valley` on a valley tile, one in `stray` elsewhere, 0 never).
  Hunters: rock shelters under a face (`cliff_foot`: gentle ground under 25 m of rise with a
  steep neighbour; sandstone and limestone first), one limestone cave mouth in eight repainted
  as a painted cave, kill sites at bog and lake edges and bone beds in river gravels where the
  land was once cold, shell middens on sheltered shores. Farmers: flint mines on limestone
  plateaus, a long barrow on a convex crest (`crest`: gentle, 30-150 m over the long low, near
  the local top), an enclosure on a terrace, lynchets on the hillside below the barrows,
  clearance cairns on stony fields. Builders: a stone circle (one tile in a 4x4 block, on a
  hard flat with the sky round it), standing stones, dolmens, rock art on bare outcrops over
  the valley, a henge on a terrace (one in 5x5), the ridgeway (`ridge_walk`: 6-14 cells along
  the crest from the tile's highest gentle ground, `Place::cells`), a causeway across a bog
  (`causeway_walk`: dry shore to dry shore, 2-10 cells of bog), a ford where the river runs
  15-150 m wide at under 0.01 m/m with dry land on both banks. Metal: a line of round barrows
  along the ridgeway (else on a crest), the hill fort on a spur (`spur_score`: the ground falls
  20 m within 1.5 km in 5+ of 8 directions), a crannog on a 1-4 m lake margin, a hoard and a
  body in the bog, copper workings on steep hard ground near ore, desert kites. The last age: a
  watchtower on a height (near the sea first), a quarry in bare hard rock, a mill on a quick
  stream 4-60 m wide, a drowned village under a lake 5 m+ deep.
- Names and words (`names.rs`, last): each 6x6 block of tiles has a tongue (`Tongue::of`: nine
  onsets, five vowels, seven codas drawn by the block's hash, and how often a name runs to a
  third syllable), so one valley's names sound alike and the next valley's differ; a place's
  name is two or three syllables keyed on its cell, its epithet one of its kind's ("where the
  first farmers laid their dead"), its description the cause woven into the age's sentence,
  its contents what a visitor or a spade finds (`contents`, half of them by hash), its rank by
  rarity (`rank_of`: a meteorite crater 20 ... a ford 170) where the planner set none.
  `--places X,Y` lists a window's places by age (`listing`).
- Seed 42 under the Sun, 8-tile windows: the Falls region 177,133 168 places (52 natural, 4
  hunters', 41 farmers', 44 builders', 26 metal, 1 last age) and 21 caves; the floodplain
  399,67 144 places and 25 caves; the volcano 16,104 76 (61 natural: gorges, hot springs,
  tarns); the desert interior 142,147 ~90. Before the farmed-valley gate every tile fired every
  rule it could (231-266 a window, 33 fords and 32 barrows on 64 tiles). `--place-census 12`:
  every window has places of several kinds, 100% with a cause, 0 `check` violations.
  `tests/places.rs` keeps the floors.

## Caves as systems (`sites/caves.rs`; ROADMAP Stage 5, 2026-10-16)
- Cave graphs (`plan::CaveGraph`: nodes in absolute cells and metres, passages with a kind,
  width, height and whether a stream runs) planned per core tile, after the natural wonders:
  a karst cave on one carbonate tile in three (`KARST_ONE_IN`; limestone at the surface or
  within 30 m of it in the tile's rock stack, a twelfth of the tile's cells at least): the sink
  is flat carbonate ground off the channels, 15 m+ over the tile's lowest land, picked by hash;
  the stream's way under follows the steepest descent on the raw heights (a pure function of
  position, unlike the window's routing) until it leaves the carbonate, meets a channel or a
  lake 12 m+ below the sink, or reaches the sea, within two tiles. `build_karst`: the
  resurgence at the chain's end, the mouth 40-70 m up the slope toward it, a dry upper gallery
  (`Phreatic`, the old level, rising 1% back to the chain's middle under 8 m of roof), a
  breakdown `Chamber` (14-40 m wide), a `Shaft` 10-30 m down to the stream level (at least
  `MIN_DROP_M` 10 m under the mouth), the `Vadose` canyon (1.5-4 m wide, 8-20 m tall, the
  stream in its floor) climbing to the sink's shaft, the flooded `Phreatic` sump from under the
  chamber to the spring, and where limestone lies at the surface a shaft on down to the first
  cavern (`FIRST_CAVERN_M` 22). Lava tubes on a volcano's own tile (1-2 from the vent down the
  steepest descent, 5 m under the ground, a skylight every other node, the mouth at the lower
  end), sea caves on one hard coast tile in five (`SEA_CAVE_ONE_IN`; 30-70 m into the
  headland), rock shelters under a sandstone slope (one in 4; 8 m back). Every size and wander
  is hashed on absolute cells (`f.abs_cell`), so two windows plan a tile's caves bit for bit
  (`two_windows_plan_the_same_caves`; a sea cave's depth hashed on grid cells had differed by a
  millimetre). A graph's mouth becomes a `CaveMouth` (or `RockShelter`) place with
  `Place::cave`, its floor the deepest node, or takes over a natural mouth within 2 cells;
  `cave_cause` / `cave_description` / `cave_contents` word them.
- Region cost: ~0.1 s a window. The embark carves the graphs (`local/caves.rs`, notes in
  `src/local/CLAUDE.md`); the region draws the mouths as places.

## The violent land (`landforms/impact.rs`, `tectonic.rs`, `mass.rs`, `fire.rs`, `sinking.rs`, `flood.rs`, `hazard.rs`, `sites/violent.rs`; ROADMAP Stage 7, 2026-10-17)
- Every pass returns at once without upheaval (`WorldData::upheaval`, `crate::upheaval`), so
  the Sun's regions are as they were; with it the record's numbers join the chunk cache's key.
- Craters (`impact`): the record's craters from 2 cells to `upheaval::WORLD_SCALE_TILES` (1.5
  tiles; larger ones the world map shapes itself) are cut before the rivers by
  `Crater::profile_at` (bowl under the transition diameter 3.2 km / g, else flat floor, three
  terraces and a central peak, a peak ring over 30 times it; rim scalloped by angle, its
  height varying, one breach once worn 12%, two past 50%, where the rim falls to the ground;
  infill 18 m a Myr; ejecta lobed): inside the walls the crater replaces the ground at the
  world's height at its centre with half the land's own detail kept. Labels: CentralPeak,
  ImpactCrater (x < 0.85 rims), ImpactRim (to 1.12), Ejecta (to the lobe's reach, worn under 60%).
- Faults (`tectonic::FaultField`): from `WorldData::faults` (per tile kind, strike as a doubled
  angle, intensity spread `ZONE_KM` 260 km from the boundaries and the stress map's rifts).
  Normal: blocks 18 km apart tilted by hash (horsts where they tilt apart, grabens where
  together), throw 750 m x intensity x size, tapered over 28 km segments, active one in
  `intensity x 1.15`; thrust 20 km, 520 m, a fold on the upper edge; strike-slip: a trough 14 m
  deep a cell wide on traces 45 km apart with ponds (12 m) and ridges (18 m) where it steps.
  Before the rivers, which cut the scarps into facets. x is read wrapped and the two sides of
  the date line are blended within 2 tiles of it (half and half on the line), so the faults
  meet there. Labels FaultScarp (on the face, slope
  over 0.05 at 610 m), SagPond (a pond with a lake), PressureRidge, FaultTrough.
- Slides (`mass::disturb`, on the blended region in `generate_zoom` before the routing, so a
  debris dam makes its lake): a lattice 2.4 km apart; a slope fails with p = (factor - 1) x 0.6
  (at most 0.75), factor = slope / ((0.08 + 0.14 hardness) x scale) x (0.7 + 0.6 moisture) x (1 +
  1.2 seismic); never under -8 C. The scar a bowl cut into the upper slope (depth a quarter of
  the relief within 4 cells, at most 15-75 m), its volume laid along the steepest descent for
  relief / 0.5 (a rock avalanche off hard faces with relief over 350 m: / 0.2), widening, with
  hummocks. One stratovolcano in three has lost a flank: an amphitheatre 2.2 km at the summit
  opening downhill, its debris run 18 km. Every decision is read from the heights before any
  slide is cut, so two windows agree (`steep_slopes_fail_the_same_way_in_two_windows`).
  `PLANET_DEBUG_SLIDES=1` prints the slides planned. Labels LandslideScar / CollapseScar,
  LandslideDebris / RockAvalanche; `Slide`s go to the site planner.
- Fire (`fire`): maars on a 5.2 km lattice (one in four) near young volcanism on wet ground
  under 1,500 m (450-1,100 m across, 40-120 m deep, an ash ring 18 m); the provinces' traps
  (steps of the flow's thickness, 70% weight, basalt at the surface where the share passes
  0.35); fissure vents on normal-fault faces near young volcanism or lava; lahars on the valley
  floors (HAND under 6 m) within 2.5 tiles of a stratovolcano colder than 4 C. The summit
  craters (`volcanic::shape_after`) had been left out of the shape order: they are cut on every
  world since 2026-10-17 (`SHAPE_VERSION` 4).
- `sinking`: salt sinks within 3 cells of a playa's edge in 3-cell clusters (35%); thaw lakes on
  a 4.8 km lattice (half the points) where the mean is -12 to -2 C, moist over 0.3, flat and
  under 600 m, drawn out across the wind, 3-9 m deep (they fill). `flood`: scablands on big
  valley floors (catchment a tenth of a tile, HAND under 25 m) in the zone beyond the old ice
  (`IceMap::once` 0.15-0.55), one tile in three; abandoned channels on floodplains, fans and
  deltas along a slow noise's zero line off the live channel. `hazard`: crevasses on glaciers
  steeper than 0.12 (0.06 at the edge, 30%), avalanche paths on 24-50 degree slopes with a snowy
  winter (-6 to 4 C, moist over 0.35) along a noise's zero line, burns in boreal, savanna and
  subalpine woods and the warm half-dry ones (moisture 0.42-0.7) in noise patches on 60% of
  tiles.
- Places (`sites/violent`, before the natural wonders; `after` once the prehistory is planned):
  see its header. The old one-tile-in-4,000 meteorite crater is not rolled on an upheaval world.
- The chronicle in the places (`WorldData::ground_events_at`, set where a world meets its
  history): a region slide on a tile the history's landslide struck takes its words, else a
  Landslide place is put on the steepest dry slope within 4 km of the tile's middle; a fault
  scarp on a tile an earthquake struck names its last great rupture.
- Seed 42 under its own star: the 95 km crater at 381,79 holds a breached lake spilling into a
  second; the fault region 402,140: scarps, 27 slides, four landslide lakes, 65 of 65 places
  shown in their embarks. Seed 7 (512x256, its own star; `worlds/s7.world`): 23 stratovolcanoes,
  24,24 has lost its flank (`--upheaval-report` marks them), lahars at 272,88 and 232,96, maars
  and fissure vents at 232,96, the traps of a flood basalt at 78,183 (15% of the window).
