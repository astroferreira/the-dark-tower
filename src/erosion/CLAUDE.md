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
- `landscape::fill_pits(hm, 4, 10.0)` runs after the finishing passes (coastline, fjords,
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

## Erosion
- The legacy erosion pass (`erosion::simulate_erosion`: hydraulic droplets, glacial SIA, flow accumulation) is off by default since the
  landscape evolution does the physical erosion: it took ~45 s of a 512x256 world (~6 min at
  1024x512, and its GPU step sometimes hung) for little visible change (seed 42: slightly denser
  tributaries; it flat-filled every depression, so 17 lakes vs 23). A 512x256 world now
  generates in ~4 s. `--legacy-erosion` (or `LAB_LEGACY_EROSION=1` in terrain_lab) runs it; the
  legacy tectonics path and the erosion-preset comparison grids still use it.
