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

## The audited crust (`audit.md`, terrain revision 2, 2026-10-13)
- Plate generation samples its noise on the sphere (`generation.rs::cylindrical_coords` ->
  `heightmap::spherical_coords`, tilted 20 degrees): the cylinder pinched polar noise into
  spokes. Worlds carry about half the polar land they did (256x128 seeds 1, 5, 11: 6-10% of the
  land poleward of 60 degrees vs 17-22%; the land fraction by area is the same).
- Distances on the sphere (`tilemap::geodesic_distance`: each cell inherits its neighbour's
  nearest source and measures the great-circle distance to it): `continental_interior_distance`,
  `distance_to_coast` and the legacy path's distance fields were breadth-first counts
  (max(|dx|, |dy|): squares and octagons).
- Foreland basins (`foreland_flexure`): the excess height of crust thicker than 38 km bends the
  plate beside it over the flexural length (alpha 110 km, w = e^(-r/a) cos(r/a)); off the load
  the ground sinks by 0.12 of the convolved load (a trough beside the belt, a low forebulge
  beyond). Below ~0.35 alpha per cell (the 96-wide dev world) it does nothing.
- A plateau ceiling on the isostatic base (4.5 km + 1 km tanh): thickened crust spreads rather
  than rising past ~5.5 km; the final peak compression starts at 5.2 km (was 4.5).
- Ridged detail is Musgrave's signal-weighted multifractal (each octave weighted by the last,
  gain 1.8): crags on the crests, smooth valleys.
- Margins: past 80 km from the shore a passive margin (trench < 0.1-0.4) is raised to the
  continental slope (sigmoid to -3.2 km over 70 km) and the rise (e-fold 180 km into the abyss)
  where the crust left it deeper (`heightmap::margin_depth`, raise-only, so wide shelves on drowned
  crust stay). Fracture zones (`heightmap::carve_fracture_zones`): from ridge-axis points (seafloor
  younger than 8 Myr, 450 km or 4 cells apart) traces up the age gradient both ways, up to 1800
  km, a 400 m trough fading outward with a 120 m step on one side (seed 42: 159).
- The ocean's depth (`build_heightmap`, revision 2). Three causes, three fixes. (1) The water
  poured in for the land fraction stood ~860 m above the age-depth law's datum, so every abyss
  sank 860 m too deep; the floor is now the law's depth below sea level (the shift fades over
  crust 14-26 km, blurred like the base). (2) The simulated seafloor was too old (means 69-111
  Myr, 10%+ at the 180 Myr cap; Earth ~64): `steady_state_ages` remaps the ages, in order and by
  area, to the Earth's steady-state age-area law (area falling linearly to 180 Myr, mean 60).
  (3) Depth by GDH1 (Stein & Stein 1992) instead of Parsons-Sclater, plus pelagic sediment
  (`sediment_lift_m`: up to ~700 m, standing at 0.4 of its thickness). Trenches are cut after
  the base's blur (it had left one-cell trenches at a seventh of their depth), 4.5 km deep at
  full subduction. River mouths build a real margin in the landscape evolution (a shelf to
  -130 m at ~80 km, the slope, the rest piled as the rise; revision 1 laid a 780 km wedge).
  Seed 42 / 7 / 3 at 512x256, % of the ocean by depth (Earth): 0-0.2 km 14/12/12 (7.5),
  0.2-1 8/8/9 (4.4), 1-2 3/3/3 (4.2), 2-3 3/3/2 (6.8), 3-4 13/11/12 (19.6), 4-5 35/34/32 (33),
  5-6 25/29/30 (22.7), > 6 km 0.2/0.8/0.8 (1.8); mean depth 3.5-3.7 km (Earth 3.7); below 6 km
  had been 35-47%. What is left is the drowned continental crust at 0-1 km where the Earth has
  slopes at 1-3 km (the style's land fraction is below the continental crust's area).
  `PLANET_TERRAIN_DEBUG=1` prints the sea level's height over the datum and the ocean's ages.
- Continental rifts (`simulation.rs::finish`, revision 2): continental crust within two cells
  of seafloor younger than ~30 Myr (a fresh rift's flank) is under tension (it had none: land
  never went below -0.012), and `propagate_rifts` runs rifts on into the continents where a
  ridge (floor under 8 Myr) meets continental crust, along the ridge's axis (perpendicular to
  the age gradient), up to 1200 km, fading, wandering, swelling and easing every ~320 km so the
  graben breaks into basins (Tanganyika, Malawi, Turkana); tips 2000 km apart, at least 5 cells
  inland, six a world at most. `carve_young_basins` sinks a graben where tension passes -0.03
  (500 m by -0.10) and raises shoulders beside it (0.3 of its depth). Seeds 1-11 at 512x256:
  30-136 rift cells; near the poles the fjord pass drowns them as inlets. Bedrock hardness
  takes only compression as folding; tension softens toward basin fill.
