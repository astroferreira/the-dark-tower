# TODO

Short-term tasks. The long-term direction (six updates toward an autonomous colony
simulator) is in `ROADMAP.md`.

Remaining work, roughly in priority order. "Phase" letters refer to the richness plan
(A save/load + seasons, B living world, C resources + geographic history, D ecology + caused
anomalies, E adventurer). A and C are done.

## Phase D: ecology and anomalies with causes
- [x] Species populations with biome ranges, predators/prey, migration (`history/ecology.rs`: 9 species on the world grid, stepped yearly).
- [x] Visible feedback over time: forests shrink around cities, farmland spreads, game vanishes near big towns, wolves return to ruins (map tiles, hover, chronicle).
- [x] Replace dice-rolled fantasy biomes with caused ones (bone fields, titan bones, ashlands / dead / crystal woods at lairs, crystal wastes at fallen towers, overgrown ruins), each linked to a chronicle event.
- [x] Embark-scale signs of life: animal trails to water, burrows, nests, dens, bones.
- [ ] Ecology does not feed back into settlements yet (scarce game / felled forests don't change growth or timber).
- [ ] Old `history/creatures/populations.rs` (legendary-led monster populations) is separate from the ecology species; merge or retire it.
- [ ] Only some anomaly biomes have causes; the rest of `is_caused_biome` (void scars, ley nexus, spore wastes, ...) never appear with a history. No deities die, so "dead gods leave titan bones" uses slain giant beasts instead.
- [ ] Seasonal migration (herds moving between summer and winter ranges) and per-species map markers.

## Phase B: living world (superseded)
The generated history is the past; time only passes in the game (unpaused) after embarking,
so history no longer steps while the viewer runs. The living world after the present day is
ROADMAP Update 5; game-time seasons are Update 3.
- [ ] Rivers freeze / flood with the seasons (in game time).

## Phase E: embodied player
- [ ] Adventurer walking embarks and regions, entering towns.
- [ ] NPCs that speak from the gazetteer and chronicle ("my grandfather fought at the ford in year 143").
- [ ] Connect to the fortress-mode code on the `mystery_box` branch (`src/game/`).

## Landmarks and set pieces
- [x] Detect extremes: highest peak, longest river, biggest waterfall, deepest gorge, crater lakes, giant trees (`lore/landmarks.rs`; labels, hover, journal).
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
- [x] Coupled erosion and uplift (`erosion/landscape.rs`: stream power with the climate's precipitation, uplift from stress, flexural isostasy, sediment to lakes and shelves; `fill_pits` drainage repair after the finishing passes; 2 km erosion clamp removed).
- [x] The legacy hi-res erosion (~45 s at 512x256) is off by default (`--legacy-erosion`); the landscape step already gives the drainage and relief, and lakes are kept. A 512x256 world now takes ~4 s.
- [ ] Sub-tile detail (valleys narrower than a tile) only exists in zoomed regions; if the world map wants more texture, run the landscape step at 2x and downsample.
- [ ] Endorheic basins: the landscape step keeps arid hollows as lakes, but the legacy erosion fills them all; decide lakes vs. salt flats from the water balance (evaporation vs. inflow).
- [x] Sea level from the ocean's volume (priority flood: inland basins below sea level stay dry; volume fixed at birth to meet the style's land fraction, conserved through the landscape evolution).
- [x] River discharge and lake water balance from precipitation (Budyko runoff, open-water evaporation PET - P).
- [ ] Continental crust elevation varies a lot between seeds (mean -700 to +500 m over 47-65% of the map), so how much continental shelf is drowned depends on the seed.
- [x] Whittaker/Koppen biomes from simulated climate: precipitation model rewritten (land ~850 mm/yr), heat transport fixed, biome thresholds by aridity class (forests 22-37% of land).
- [ ] Soils from erosion history (sediment depth from the landscape step, weathering by climate).
- [ ] Biomes use annual means only: tree line by warmest-month temperature, Mediterranean (winter-wet) and monsoon climates from the seasonal fields.
- [x] Allow polar continents (one sea definition, `sea_mask`, instead of ocean seeded from the map edges; plate and land areas on the sphere).
- [ ] Polar land is stretched across the whole map width (equirectangular); consider how the tile viewer and history treat the polar rows (settlements, labels, the start screen preview).
- [x] Offshore islands and arcs looked like smooth ovals (`apply_island_coasts`); dark coastal halo in the exporter (its depth ramp was inverted).
- [ ] Zoomed regions straddling the date line (x wrap) don't match exactly.
- [x] `grid_export.rs` still used the legacy plate path (now `terrain::generate_terrain`, shared with main and terrain_lab).

## Viewer and tooling
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
