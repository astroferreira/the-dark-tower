# TODO

Remaining work, roughly in priority order. "Phase" letters refer to the richness plan
(A save/load + seasons, B living world, C resources + geographic history, D ecology + caused
anomalies, E adventurer). A and C are done.

## Phase D: ecology and anomalies with causes
- [ ] Species populations with biome ranges, predators/prey, migration (scaffold: `history/creatures/populations.rs`).
- [ ] Visible feedback over time: forests shrink around cities, farmland spreads, game vanishes near big towns, wolves return to ruins.
- [ ] Replace dice-rolled fantasy biomes with caused ones: dragon lair scorches forest to ashlands, battlefields become bone fields, dead gods leave titan bones, collapsed towers leave crystal wastelands. Each anomaly should link to a chronicle event.
- [ ] Embark-scale signs of life: animal trails to water, burrows, nests, bones.

## Phase B: living world (deferred by request)
- [ ] World clock: history keeps stepping while the viewer runs (pause / speed controls).
- [ ] Seasons advance with it; settlements grow, wars start, ruins appear on screen.
- [ ] Rivers freeze / flood with the seasons.

## Phase E: embodied player
- [ ] Adventurer walking embarks and regions, entering towns.
- [ ] NPCs that speak from the gazetteer and chronicle ("my grandfather fought at the ford in year 143").
- [ ] Connect to the fortress-mode code on the `mystery_box` branch (`src/game/`).

## Landmarks and set pieces
- [ ] Detect extremes: highest peak, longest river, biggest waterfall, deepest gorge, crater lakes, giant trees.
- [ ] Name them, tie legends/figures/deities to them, hand-tune their embark-scale look.
- [ ] "Director" pass: every region has a focal point, contrasts next to each other, rare features spaced out.

## History and economy depth
- [ ] Chokepoint wars (fords, passes, harbours); capitals sited on river mouths and defensible ground.
- [ ] Trade caravans that carry goods you can follow on the map; prices from scarcity.
- [ ] More ruins: plague, famine and depopulation (seed 202 has only ~33 ruins after 250 years).
- [ ] Longer or denser history option; save histories at several ages.
- [ ] Ore veins are hard to see on z-level views at default zoom (clear only in the cross-section): add an "ore only" overlay or brighter flecks.
- [ ] Underground: caves, aquifers (data exists in `underground_water.rs`), lava near volcanoes, ore veins worth digging.

## Towns and embarks
- [ ] Draw roofs in the surface view (currently walls + floors only, like a floor plan).
- [ ] Organic street layouts (bends, plazas at junctions, market squares) instead of a warped grid; follow the approach-road angle.
- [ ] Fix L-shaped houses where the street warp crosses a lot.
- [ ] Seasons in zoomed regions and embarks (currently annual colours; world map only).
- [ ] Neighbouring embarks should join: verify and test that two adjacent 192x192 areas match along the shared edge; walk out of one into the next.
- [ ] Settlement variety by culture: more architecture styles than stone / timber / earth (bone, crystal, living, woven, carved).

## Terrain and tectonics
- [ ] Coupled erosion and uplift (stream-power law with the climate's real precipitation, flexural isostasy, sediment to shelves). Mountains currently outpace what erosion can grade; closed-basin/pit counts and river connectivity at 1024x512 are worse than the legacy terrain (about 45% vs 75%).
- [ ] Sea level from a fixed ocean volume; priority-flood lakes; river discharge from precipitation.
- [ ] Whittaker/Koppen biomes from simulated climate; soils from erosion history. Temperate and tropical forests are only 2-5% of land because the climate sim is dry.
- [ ] Allow polar continents (currently the map's top/bottom rows are forced to ocean for flow routing).
- [ ] Offshore islands and arcs still look like smooth ovals; dark coastal halo from the exporter's shallow-water shading.
- [ ] Zoomed regions straddling the date line (x wrap) don't match exactly.
- [ ] `grid_export.rs` still uses the legacy plate path.

## Viewer and tooling
- [ ] Terminal explorer's `Z` shows a single region without walking; port walking, seasons, labels and resource markers or retire it in favour of the tile viewer.
- [ ] Zoom view (`Z`) shows a shaded-relief image, not tiles.
- [ ] Zoom size is fixed at 8x8 tiles x 128 cells inside the viewer (only CLI flags change it).
- [ ] Region generation freezes the window for ~5-7 s on a cold start (background load only happens while walking).
- [ ] Hover info, labels and resource markers for embarks; z-level legend.
- [ ] Interactive window controls (T, C, R, L, Enter, walking) have only been verified through headless renders, never in the real window: test them by hand.

## Housekeeping
- [ ] Decide whether `worlds/*.world` (170 MB each) should stay gitignored (currently yes) and document a standard seed set.
- [ ] `.DS_Store` files and `world_42_legend.txt` are untracked / modified noise: add to `.gitignore`.
- [ ] Remove or silence long-standing compiler warnings in `history/simulation/step.rs`, `geomorphometry.rs`.
- [ ] Add tests for settlement siting, road routing and town plans (only determinism / consistency tests exist today).
