# Erosion and landscape evolution (`src/erosion/`)

## Landscape evolution (`erosion/landscape.rs`)
- `landscape::evolve` runs in `terrain::generate_terrain` after the climate (tectonic terrain
  only; followed by `relevel_to_volume`, then the finishing passes; the legacy erosion only with
  `--legacy-erosion`): 40 implicit steps over 10 Myr on the world grid. Each step: flexural response
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
- (Revision 1) `landscape::fill_pits(hm, 4, 10.0)` runs after the finishing passes (coastline, fjords,
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

## The audited terrain (`audit.md`, terrain revision 2, 2026-10-13)
- Order (`terrain::generate_terrain`): tectonics -> volcanoes -> regional noise -> island coasts
  -> coastline jitter -> climate -> landscape evolution -> (legacy erosion) -> young basins ->
  fjords -> beaches -> last pit fill and flow routing -> temperatures to the final heights. The
  rivers now carve the volcanoes' flanks and the regional relief into drainage instead of the
  relief being pasted over graded valleys (seed 42 at 512x256: big-river water trapped short of
  the sea 6.4% -> 4.4%). Revision 1 (`--terrain-v1`) is the old order, bit for bit.
- Rock hardness (`terrain::bedrock_hardness`, from `CrustFields`): old stable continental crust
  0.82, fold belts (stress 0.08-0.3 or crust 38-48 km) toward 0.6, low continental basins and
  thinned crust toward 0.25, volcanic edifices and young accreted crust 0.85, ocean floor
  0.7-0.85, +-0.08 of bedding noise; after the evolution, land raised over 30 m by sediment
  outside the uplifting belts becomes alluvium (0.18, `lay_alluvium`). It is the world's
  `hardness_map` (aquifers, waterfalls, resources, soils, embarks); it was 0.3 everywhere.
- `landscape::evolve_with_hardness`: K (1 - hardness)^2, normalised by its mean over the land
  weighted by height above the sea, so the world loses as much rock as on uniform ground (an
  unweighted mean let the hard uplands erode a third as fast: 14M km3 instead of 32M).
- The water: after the evolution the sea level is solved again for the style's land fraction
  (`plates::crust::land_fraction_for`) instead of keeping the birth volume: the relief built
  before the rivers sheds 1.5-2x the sediment onto the shelves, and keeping the volume drowned
  4-5% of the planet's land (256x128 seeds 1-11: final land 23-36% of cells vs 25-41% on
  revision 1, the difference mostly revision 1's polar land).
- Young basins (`heightmap::carve_young_basins`, after the evolution, which fills older hollows
  with 10 Myr of sediment): ice-scoured hollows (up to 120 m, where ridged noise peaks) on hard
  (> 0.55), low (< 1500 m) ground past 40 degrees that is between -12 and 6 C a year (glaciated
  and since left by the ice; colder ground is still under its ice sheet), and grabens along
  continental rifts (tension below -0.03, 500 m deep by -0.10, with shoulders raised 0.3 of the
  depth beside them; the crust's rifts are in `src/plates/CLAUDE.md`). Seed 42 at 512x256: ~1400
  cells scoured, ~100 sunk along rifts, 30 lakes (revision 1: 24, most from pits the finishing
  passes left).
- River mouths (revision 2) build a margin, not a wedge: the sediment fills toward
  `heightmap::margin_depth` (a shelf to -130 m at ~80 km, the slope, the rise at its foot),
  raise-only, ten cells out; what does not fit goes to the deep sea (seed 42: shelves 8.9M km3,
  deep sea 15.6M of 24.9M eroded). Revision 1 filled to 40 m + 60 m a cell.
- The last pit fill (`landscape::fill_pits_keeping`): hollows made after the evolution are filled
  whatever their size; the evolution's lakes (a `lake_depth` taken before the fjords) stay if
  at least 4 cells at 512 wide (scaled by area, at least 1) and 10 m deep. Then
  `Terrain::flow_accumulation` is routed on the final map (it had been the pre-finishing map).
- Tile sizes: `apply_coastal_beaches` takes its strip widths from the real tile size (it used the
  5 km `MapScale`, ~190 km out at 512 wide; with distances on the sphere that lifted deep polar
  sea floor by up to 1.2 km).
- Fjords (`heightmap::carve_glacial_fjords`): from coastal river mouths with a catchment (3
  cells at 512 wide, by area), past 40 degrees under coastal mountains (> 600 m) or past 55
  under hills (> 300 m), the trunk valley is traced upstream (~300 km) and drowned as a U
  trough: half-width 0.9 (Q/Q_mouth)^0.35 cells (x sqrt(78 km / tile)), floor
  -D (1 - (r/W)^2) (0.25 + 0.75 sin(pi s/L)), D = 250 m + 0.35 x the walls' height (at most
  1200 m), so each fjord is one water with the sea, shallow at its sill; fjords keep two cells
  apart (seed 42: 108). Revision 1 carved wherever a noise field crossed a threshold.
- Volcanoes (`mark_volcano_profiles`): a shield a broad dome (Gaussian, 2.4 cells, reach 4), a
  stratovolcano a concave cone ((1 - r/2.5)^1.8), a caldera a ring wall at 1 cell round a
  collapsed crater; on the sphere's metric. They are built before the rivers, which then cut
  their flanks. Lava (`generate_lava_map`) spreads breadth first, strictly downhill, and stops at
  the poles (revision 1: a stack, 50 m uphill allowed, and the rows wrapped pole to pole).
- The legacy heightmap (`--legacy-tectonics`) on the audited terrain: noise on the sphere, round
  distance fields, signal-weighted ridges, mountain belts along the convergent boundaries
  (`orogen_field`, `Orogen::profile`: a steep pro-wedge and a plateau on the overriding plate,
  wide where convergence is strong, a steep front on the plate going under, a flexural foreland
  beyond either, the 4.5-5.5 km ceiling; the noise corridors stay as worn old fold belts),
  shelf-slope-rise margins (`margin_depth`: -130 m at ~80 km as d^0.6, a sigmoid slope to -3.2 km
  over 70 km, the rise into the abyss) and fracture zones off the spreading ridges.
- Hydrogeology (`underground_water.rs`): the water table from the Beven-Kirkby wetness index
  (`wetness_index`: ln(a / max(tan b, 0.001)), a = catchment x cell width, less its minimum on
  land): d = 120 m exp(-0.6 moisture porosity TWI), near the surface in wet valley floors, deep
  under dry ridges; confined aquifers where the ground is concave (curvature < -5 m) and the
  uplands within 8 tiles stand 150 m higher on average; artesian springs where that aquifer's
  head (uplands less 30% friction) is above the ground on a fault (stress > 0.15). Waterfalls are
  exactly the drop (revision 1 added up to half again for hard rock); a caprock ledge (hardness
  0.15 above the rock downstream) halves the drop needed. Revision 1 put the table at a share of
  the elevation, so every range was "confined" and no basin could be.
- Curvature (`Tilemap::analyze_directional_context`) is center minus the neighbours' mean, so
  ridges are positive (revision 1 had it inverted: `is_ridge` found valleys and the microclimate
  warmed ridges and frosted valleys the wrong way round); `plan_curvature` and
  `profile_curvature` from the 3x3 derivatives (Zevenbergen & Thorne). Gradients, the flow
  direction's step lengths and `gaussian_blur`'s east-west kernel take cos(latitude)
  (`metric_cos`, at least 0.15), and the upscalers' detail is sampled on the sphere (no seam at
  the date line).
- `terrain_lab`: `LAB_DUMP_HEIGHTS=tectonic,final` writes those stages' heights as raw f32
  (`heights_<stage>_<w>x<h>.f32`) for outside analysis (hypsometry, bathymetry renders).

## Erosion
- The legacy erosion pass (`erosion::simulate_erosion`: hydraulic droplets, glacial SIA, flow accumulation) is off by default since the
  landscape evolution does the physical erosion: it took ~45 s of a 512x256 world (~6 min at
  1024x512, and its GPU step sometimes hung) for little visible change (seed 42: slightly denser
  tributaries; it flat-filled every depression, so 17 lakes vs 23). A 512x256 world now
  generates in ~4 s. `--legacy-erosion` (or `LAB_LEGACY_EROSION=1` in terrain_lab) runs it; the
  legacy tectonics path and the erosion-preset comparison grids still use it.

## The GPU pass has a deadline (`gpu::simulate_gpu_or_cpu`, `--features gpu`; 2026-10-12)
- Twice the GPU hydraulic pass stalled 40+ minutes (about a minute of CPU) waiting on the device
  where reruns took ~70 s. It now runs on a worker; past `PLANET_GPU_TIMEOUT` seconds (240 by
  default) it prints "GPU erosion stalled for N s; finishing it on the CPU instead" and the
  rayon pass does the work (the stalled worker is left alone). `PLANET_GPU_TIMEOUT=0` forces the
  fallback (checked on seed 5, 96x48, `--legacy-erosion`). Only the legacy erosion, legacy
  tectonics and erosion grids run this pass, and only in a `--features gpu` build.
