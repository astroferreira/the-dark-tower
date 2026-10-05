# Tectonics (`src/plates/`)

## Tectonic Simulation (`plates/simulation.rs`, `plates/crust.rs`)
- Each plate is a rigid body rotating about its own Euler pole; its crust (thickness, age,
  volcanic edifices) lives in the plate's *body frame*, so motion never blurs the crust.
- Each step, every map cell asks which plates cover it: **overlap** = convergence (oceanic
  subducts under continental, older under younger, builds an arc; continent-continent
  collisions thicken crust), **gap** = divergence (new age-0 oceanic crust: ridges and rifts).
- Slab pull speeds plates up, continental collision locks them. Mantle hotspots are fixed in
  the global frame and leave volcanic chains. Mountains relax by erosion (tau = 140 Myr).
- Elevation: Airy isostasy (continents), Parsons-Sclater sqrt(age) law (ocean), trenches from
  recent subduction, then procedural detail.
- Sea level is the planet's water poured into the basins (`crust.rs`): a priority flood from the
  deepest cell gives each cell the water level at which it joins the ocean, so closed basins
  inland stay dry even below sea level. The volume is set at birth so the style's land fraction
  is met (`sea_level_for_land_fraction`, area-weighted), stored as
  `TectonicTerrain::ocean_gel_m` (global equivalent layer; earthlike ~2.9-3.1 km, Earth 2.64),
  and kept after the landscape evolution (`relevel_to_volume`: shelf sediment displaces water,
  the sea rises ~30 m at 512x256, ~35-50 m on the dev world). A fixed volume per style was
  tried first: land then swung 4-42% between earthlike seeds because continental crust stood at
  a mean -700 to +500 m; since the polar/area fixes it stands at 157-281 m, and a fixed 3015 m
  ocean would give 27-38% land (`terrain_lab` with `LAB_GEL=1 LAB_GEL_FIXED=3015`). The legacy
  erosion and finishing passes don't conserve mass, so the sea is not re-levelled after them.
- Initial continents are the zero contour of a noise-perturbed spherical signed-distance field;
  each continent and its shelf belongs to one plate, plate borders are domain-warped, and
  detached plate fragments are absorbed (otherwise they plough trails through continents).
- Continental collision polarity is decided by each plate's continental area, so island arcs
  are accreted as terranes instead of drilling through continents.
- Step count scales with map width (`steps` is calibrated for 512 wide); deposit kernels and
  stress/trench memory are tuned so moving boundaries don't leave stripes.
- Terrain: interior seaward dome, drainage integration (priority-flood from the sea before
  detail noise; hollows shallower than 50 m filled afterwards), coastline roughness confined to
  a band around the shore.
- Land may sit on the poles. "The sea" is one definition everywhere
  (`erosion::landscape::sea_mask`): connected bodies at or below sea level of 50+ cells at
  512x256 (scaled by area, at least 8), wherever they are; routing, depression filling (`rivers.rs`) and
  water-body detection seed from it instead of from the map's top/bottom rows. Plate areas,
  continental-crust area and the land fraction are measured on the sphere (rows weighted by
  cos latitude); counting map cells had made polar plates look big, so they were picked as
  continents and the poles came out continental. Collision and arc deposits are scaled by
  the source/receiving cell area too (`Grid::deposit_scale`): every overlapping polar cell used
  to add a full cell's crust, piling crust up to 9-11 km elevations at the poles (hidden while
  the poles were forced underwater). Some worlds now get a polar continent (seed 7), most
  little polar land.
- `stress_map` is derived from simulated convergence/divergence + standing orogens, scaled to
  the range downstream passes expect (0.15 volcanic, 0.3 mountain building).
- Preview tool: `cargo run --release --bin tectonic_preview -- <seed> <w> <h> <style> <myr> <steps> <out_dir>`
  writes `final.png` (plates | thickness | age | elevation | stress), `timelapse.png`, and
  `basins_sim.png` / `basins_legacy.png` (land trapped in closed basins, with % printed).
