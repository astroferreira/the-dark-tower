# Playable areas / embarks (`src/local/`)

**Myth (2026-10-13):** without `--myth` caverns grow no fungus trees (cave moss only), their
life is drawn from real cave fauna (`caverns::REAL_LIFE`), no forgotten beast sleeps in the
deepest, and there is no magma sea or pipe (real magma lies kilometres down).

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

## Towns in three dimensions (2026-10-08, `local/structures.rs`; DF's site realizations)
- A storey is two levels (its floor, then standing room). After the ground is stamped, `raise`
  builds each house from its footprint (`HouseCell`: column, wall/floor/door, house axes): walls
  from each column's ground up to the footprint's highest ground + the design's top, a floor
  laid at each storey, a stair (`Shape::Stair`) in the floor's back corner away from the door
  (it needs two floor cells beside it, else the house keeps one storey: a house turned to its
  street can pinch its corners to a cell between walls).
- The keep (capitals 3 storeys, forts 2; dressed stone): floors at +2 (+4), a walkable roof of
  flags at +6 (+4) with merlons on every other rim cell; it is now placed before the house lots
  (houses had been built into it). Towers on stone walls (`BuildingKind::Tower`, 5x5 cells, one
  per ~70 m of ring between the gates, at most 24, door to the town): walls to +2, roof at +3, a
  level over the wall's walk (one steps down onto it). Flat roofs (`RoofPlan::flat`) are drawn
  in the surface view as flags and merlons with the stair's head.
- Stone houses: in cities two in three have an upper storey (the town hall always), in towns
  half of those 10 m+ wide; timber and earth houses 10 m+ wide one in three a loft (a floor at +2
  over the back half). Villages none.
- Cellars (`cut_cellar`; DF's hillock houses and undercrofts): one house in four (villages too,
  not camps or outposts; the hall always): the stair cut on down 2 levels from inside (the surface cell becomes a stair,
  `surface_z` unchanged) to a room under the house within 1-2 cells of the stair; a carving
  people's (`Site::carved`) one house in two, 3 levels down, 5x5 to 7x7 running past the walls;
  the keep's undercroft 3 down. Cut only in whole ground (no cavern, water, magma or aquifer)
  under at least a level of ground, with ground left between two cellars; under 3 cells beside
  the stair, none. `LocalMap::buildings` (`Building`: kind, storeys, loft, stair, upper floors,
  platform, cellar cells) records it all; `local_map_is_consistent` allows cellar columns as it
  does places'.
- Ruins: 40% of wall cells stand, to 1 + u^2 of their height; stone upper floors survive in
  fragments (one cell in four where a standing wall still holds it), a third of the ground floor
  is rubble, the stair is broken after its first step; cellars are whole, and empty.
- The surface view casts a soft shadow down-light, 0.6 cells per level of built height (a
  two-storey house a cell, the keep four). The ramp pass leaves stairs alone.
- `--local-snapshot` prints "Embarking on <town> (...)" and "Town in three dimensions: ..." with
  walks from the street (`nav::path3`) to each keep and tower roof, upper floor and cellar, and
  the count of open cells below the ground outside caverns, places and cellars (0).
  `PLANET_LOCAL_LEVELS="2,4,-2"` writes level frames relative to the centre's ground (whole, and
  16 px at `PLANET_LOCAL_CLOSE`), `PLANET_LOCAL_ROW` the section's row, `PLANET_LOCAL_OFFSET=
  "dx,dy"` (m) moves the embark off the town's middle.
- Dev: Greenburg (44,9, the default): 546 buildings, 396 of two storeys or more, the keep's roof
  6 levels up walked in 34 steps, 137 cellars (690 cells); 394 of 397 upper floors and 136 of
  137 cellars reached (the rest have their doors off the map or built over). Dreammere's wall
  (71,12, offset 731,0): 2 towers, roofs reached in 102-105 steps. Slaglair (61,19, earth): 86
  lofts. Seed 3 Swanworth (77,11, dwarves): 183 cellars, 8286 cells. A forced camp in ruined
  Slagmaw (15,31) and in Venomash (19,33) lives 60 days. Tested (`tests/towns.rs`).
- Not yet: every house is 8 m deep (`plan_for`: `unit(hsh >> 9)` shifts away the bits `unit`
  reads; old, left so towns keep their layout); a house can lose its door to a neighbour; the
  colony doesn't use town storeys or cellars; cellars hold nothing.

## Sites worth settling (`local/site.rs`)
- `furnish` (end of `generate_local`) adds what a first camp needs within 48 cells (~100 m) of
  the centre and nature left out: a spring and a pool on a dry site (`Feature::Spring`), a
  grove of the local trees when there are fewer than 20 (the hut needs logs), an outcrop of bare
  rock and boulders when there is no stone, berry thickets when there are few shrubs, and a
  landmark: graves (`Feature::Grave`, 3 + the named dead per battle) where the history fought on
  this world tile (`RegionLore::battles`), else a standing stone (`Feature::Stone`) if nothing
  else is there. Spots are the free cells (dry floor, unbuilt, no feature) with the most room,
  chosen by hash: deterministic.
- `report` lists what a site holds (water, wood, stone, berries, soil, game, graves, bones,
  stone, buildings); `--local-snapshot` prints "Site (N kinds): ...", and walking mode shows it
  in the title, recomputed when the embark box moves 24 cells (~30 ms; not tried by hand yet).
  Seven dev sites hold 5-7 kinds each.
- Graves with names (`LocalMap::graves`): `RegionLore::battles` holds each tile's battles and
  battle deaths (HeroDied, e.g. a captain on the walls) with the named dead who fell in battle
  ("Ukh, captain of Skullfang"). `furnish` lays the named first, then three nameless per battle
  ("A soldier whose name is lost"), 16 at most, each with its words ("Fell at the Battle of the
  Brolmdustoor Pass in 223."). A colony founded there adopts them as marks
  (`Colony::adopt_graves`): drawn as graves, named on hover, opened by a click. Dev
  Brolmdustoor Pass (47,14): 16 graves, 10 named.
- What grows follows the world's biome (`world_vegetation`): each column looks up the world
  biome of its tile (through a ~24-cell jitter so tile borders blend) and takes its tree and
  shrub density and species from it; the caused landscapes are drawn as what they are (dead
  forest: dead trees; ashlands, salt flats, crystal wastes: bare; bone fields: grass and bones;
  ancient grove: dense; swamp, marsh, bog, Mediterranean scrub, foothills, monsoon forest each
  their own); water tiles leave the shore to the local climate. Before, plants came from the
  local climate alone, so an Ashlands embark was a temperate forest and a boreal forest at
  -9 C bare tundra. `--embark-survey` prints one embark per world biome with its plant shares.
- `LocalMap::furnished` / `found` record what `furnish` added (a spring, a grove, an outcrop, a
  berry thicket) and what was there before; the site report says so.
- Level slices are drawn in ink (`local_ink::render_level_ink`): the surface first; ground at
  this level as drawn; open air above the ground ghosts the land below; rock and soil below are
  hatched parchment in their colour with ore flecked in its metal and inked edges against open
  space; water blue. `PLANET_TILE_LEVELS=1` brings back the old atlas tiles.
- Levels below the ground are cut at standing height (2026-10-08): the rock one would walk into
  hatched in its layer's colour (ore flecked, gem clusters as small marks, wet aquifer rock
  stippled blue), floors to stand on (halls, rooms, cavern floors with fungus) a pale wash inked
  at the rock, stairs (`Shape::Stair`, cut by the colony or built down into a cavern) as steps
  with a red chevron up, down or both, open dark dusk-grey. Above the ground a level shows what
  was built up to it (a tower's platform, walls at standing height). Each stone has its own
  wash (`local_ink::rock_wash`: granite pinkish grey, basalt slate, sandstone ochre, limestone
  cream, shale blue-grey, sediment buff), so the strata read as layers in levels and sections.


## Something down there (`local/places.rs`)
- `LocalMap::places` (`UnderPlace`: kind, name, cause, contents, cells, mouth, found) is filled at
  the end of `generate_local` from causes, never a bare roll: a beast laired on the tile
  (`RegionLore::lairs`: name, alive, hoard by artifact name, kills) gets its lair; the battle
  with the most named dead on the tile a tomb ("Zromp the Scourge ... fell here in the Battle of
  the Brolmdustoor Pass (366)"); a town that fell on the tile its old mine (ore near) or
  undercroft; limestone on half the tiles and sandstone/shale on a third (hashed by the tile;
  karst biomes always) a cave; the first cavern layer where it lies within 13 levels (see Caverns below; the old hashed deep cavern is gone). Spots are dry ground 30-80 cells from
  the middle, tried in a hashed order; each is carved like a colony dig (a passage into a rise,
  else a sinkhole ramp down to a room) and is walkable from its mouth.
- Places are laid out on several levels (2026-10-08, `places::layout`, DF's site layouts), cut
  under a roof that leaves the ground above: an old mine is a stair shaft eight levels down with
  a gallery (and a side working) at three levels, its seam of ore left in the last gallery's end
  wall; a tomb a stair four levels down to an antechamber, a passage, the crypt and a second
  stair to the inner tomb; a lair or cave a tunnel winding down from a pit (a level every other
  step, cut two high so it can be walked down) to a den two levels high (a lair's wide). A layout
  must fit whole in solid rock out of the caverns and water (`cuttable`), else the old 2.5-D
  passage or sinkhole. `UnderPlace::cells` are (column, floor level), the far end last. The level
  view draws a found place's far end (a sarcophagus, bones and a hoard, an ore cart) and names
  it; `PLANET_FRAMES` writes `_placeN.png` at each place's deepest level. Dev: the lair, the tomb
  and the caves span six levels, all walkable from their mouths. The unknown is blank: in the
  level view a place the camp has not found is drawn as the rock round it (`draw_delve`, with
  the renderer's own wash and hatching, and the rock beside it too so no inked edge gives it
  away); its mouth on the surface shows as it is.
- `--sim-projects N` prints a `Place:` line per place (with its levels and "walkable from the
  surface", checked with `nav::path3`).
- The halls of a fallen dwarven town (DF's mountain halls; `PlaceKind::Halls`; `Site::carved`:
  dwarves always, any people whose architecture is carving): a town of theirs that fell on the
  tile leaves its halls instead of an old mine: a stair five levels down to a great hall two
  levels high (7x9), a chamber off each side, and a stair on down to a deep chamber. Found, they
  give up a carved stone chest (a treasure) and three gems. Dev seed 3, tile 27,9: the halls of
  Slitash, 106 cells on 8 levels. `PLANET_REVEAL=1` draws unfound places in level frames; the
  halls' frame shows the great hall's level.
  Dev: 46,13 the living Baelfang's lair, 47,14 a tomb, 50,20 a limestone cave; 7 of 18
  livable embarks hold nothing. Tested (`tests/colony.rs` something_down_there). Not yet: the
  places' pages, drawing the hoard and bones, the found/unknown state (card 'The unknown is
  blank'); dev beasts' hoards are empty (beasts in the history own no artifacts).

## Caverns (`local/caverns.rs`; DF design guide ch. 11, "coarse data drives fine data")
- Every embark now keeps 52 solid levels (was 30) and three cavern layers whose middles lie
  22 / 52 / 86 m under the region's smooth surface (`Col::base`, the zoom's bicubic elevation
  without metre relief) +-10 m of broad noise; bands 5 / 6.5 / 8 m half-height, varying. A cell is
  open where 3-D Perlin over absolute position (+ detail) + the rock's openness (limestone 0.22 ...
  granite -0.08, `openness`) + a band-centre bonus passes 0.82; one open run per column per layer,
  at least 4 levels of roof. The cell under the run becomes the floor (`cavern_z[col][k]` = floor,
  top). Under 40 floor cells a layer is filled back in. Pools fill hollows to a line set by the
  tile's water table; floors grow `TreeKind::Fungus` and cave moss (Grass). Life per layer from a
  dry/wet list. A forgotten beast (`monsters::generate`, kind "forgotten") sleeps in the deepest
  layer with 300+ floor cells, seeded by the 3x3 block of world tiles (neighbours share it,
  dev: "Gru" under 45,12 and 46,13), named in the Harsh style. Dev coverage: first layer 20-26%
  of the embark, 11-28 levels down; deeper layers 30-60%. Embark generation ~0.06 s.
- Layers meet across neighbouring embarks (2026-10-08): `caverns::plan` works out every column's
  open run in absolute levels before the map is laid out, from place alone: the rock's openness
  and the water table come per column from the world tiles (`caverns::tile_fields`, blended over
  the last 2 km before a tile border by `local::tile_weights`; a tile-centre embark takes its
  own tile's values exactly); the band stops `BOTTOM_M` (100 m) under the smooth surface, and
  the map is made deeper (`LocalMap::deepened` levels) when a run would reach its floor (ore
  seams, gems and the magma sea count levels from the usual bottom, so they don't move); pools
  stand `POOL_M` under each column's band centre (was the embark's mean floor); a layer under
  40 floor cells is still carved, only not listed. Measured by `neighbouring_embarks_line_up`
  (two embarks overlapping by half, test world): runs agreed in 89% of columns inside a tile and
  7-61% across a border where the rock changes (33% overall), pools 94.5%; now 100% / 100%.
  Dev 45,12 caverns keep their floors (7424 / 12637 / 23447 cells, pools 469 / 202 / 88, were
  332 / 657 / 0); 50,20 is 7 levels deeper and its third layer is no longer clipped (3330 ->
  8548 cells). A cavern floor at level 0 had trapped settlers whose `z` was still 0 before
  their first step (floors are kept at level 1+); open dark over the floor did it again on seed
  8 on the audited terrain, so settlers are now set down at the camp's level (`Colony::camp_z`). Mountain terraces blend over tile borders the
  same way (`mountain_at`), so hillsides meet too.
- `LocalMap::cavern_at(x, y, z)`; `places` records the first cavern only where it lies within 13
  levels (`PlaceKind::Cavern`, no carving); `host_rock` now reads the top 30 levels (the deeper map
  had made deep granite outvote the sandstone a cave is cut in). Both section renderers draw
  cavern air dark, fungus mauve, moss dim. `--sim-projects` prints a `Cavern:` line per layer.

## Embarks beside the river (`viewer::river_bank`, `local::bank_near`, `local::channel_width`)
- Embark positions ("cells" in world codes, `START_CELL`, `Colony::cell`) are now in 1/64ths of a
  region cell (`viewer::CELL_FRAC`): on the dev world a region cell is 3.26 km, so whole cells put
  every embark on a cell corner, ~1.6 km from the river centrelines through cell centres, and the
  384 m embark missed them. Walking embarks go through `colony_site` at the quantised position, so
  a code rebuilds exactly the camp that was played. Old codes with cells are not compatible.
- The offered sites (`three_sites`) embark beside the tile's river (DF: the world's river runs
  through the embark tiles it crosses): the region cell with a drawn channel 3 m+ wide
  (`channel_width`, the embark's own rule) nearest the tile centre, then `bank_near` samples a
  25 m grid within 0.6 cells with the embark's meander warp and segment distances and takes the
  nearest point 20-60 m from the water's edge. `SiteOffer::cell` carries it and choosing the site
  sets the start cell; the first pass offers only tiles with a bank. Six dev seeds: 14 of 18
  offers no longer lack water (3, 2, 1, 2, 3, 3; was 1 of 18). Tested in `three_sites_to_choose_from`.
- What lies below (`site::below`, appended to `site::report` as "below: ..."; DF's embark
  screen): the host rock, ore seams and gems in the top thirty levels near the centre (sampled
  every other cell), an aquifer and how deep ("an aquifer 4-8 levels down", `LocalMap::aquifer`:
  where the tile's water table is 0.45+, levels 4-8 under the median ground), and the caverns
  ("3 cavern layers (the first 16 levels down); the old songs say something sleeps in the deep",
  the beast unnamed). Gems: `local::gem_in` (one rock cell in sixty, by world place and rock).

- The world's river runs through its tile's embark (2026-10-08, DF ch. 11 "the world river
  becomes the local channel"): a world tile is 417 km on the dev world, a region cell 3.26 km,
  an embark 384 m, so the tile-centre embark (`colony_site` with no cell) missed the river the
  world map draws on its tile. Now, where `local::world_river_width` > 0 (the world map's river
  on the tile; width 0.8 sqrt(world discharge km2)), it embarks on the bank
  (`viewer::world_river_bank`: the region channel at least half the world's width nearest the
  tile centre, then `bank_near`). The river is where the region puts it, so neighbouring embarks
  line up. `bank_near` now also skips lakes and wants land behind the point (5 of 8 points 150 m
  out dry), so a great river's bank is not an island. Channel width counts each cell's true
  area (`ZoomRegion::lat_cos`; region `river_width_m` too), depth `0.3 w^0.6` up to 16 m (was
  8), and the bare sand strip along a river is at most 12 m (was 0.6 x half-width: hundreds of
  metres of barren sand by a great river). `--river-survey` (`PLANET_SURVEY_RADIUS`, default 6)
  around the dev embark: 5 of 5 world-river tiles' embarks carry a river (were 0 of 5), widths
  611-972 m against the world's 606-659 m (before the latitude fix 331-1312 m); the 92 other
  land tiles unchanged (2 hold a river), so dev 45,12 (no world river) is the same place.
  Tested in `world_rivers_run_through_their_embarks`. Not done: walking embarks (an explicit
  cell) are not moved onto the river; "borders" can't be honoured at this scale (a neighbouring
  tile's river is 200+ km away); small streams (the creek threshold is a share of a tile, so on
  the dev world every drawn channel is 25 m+ wide and most embarks still hold no water).

## The magma sea (2026-10-08, DF)
- The bottom three levels of every embark (1-3, `LocalMap::magma_top`) are a sea of magma
  (`Material::Magma`, a liquid: `water` 7) wherever the caverns leave rock. Under ground a volcano
  stands near (within 8 world tiles; `PLANET_FORCE_MAGMA=1`), a pipe of magma (radius ~2) rises
  from it to three levels under the surface, 40-70 cells from the middle (`magma_pipe`). Drawn
  glowing orange in level views and sections; the section reaches down to it once the colony's
  stair comes near. `--sim-projects` prints a `Magma:` line.
- Testing aids (2026-10-08): `LocalMap::audit()` (`local/audit.rs`) lists violated invariants (magma only
  in the sea and pipe and never within 2 levels of the ground, no floating water, no hollow under the
  ground that is no cavern/place/water, loose ground under rock...); `PLANET_LOCAL_AUDIT=1` prints it
  with `--local-snapshot`, and the unit test asserts it is empty. `PLANET_LOCAL_Z="1,2,3"` writes
  extra level slices (`_lvl<z>.png`). `LocalMap::ground_z` is the ground under ice or water
  (`surface_z` is the ice over a frozen lake): the magma pipe stops 3 levels under it, not under the ice.

## Embarks join their neighbours (`--embark-seams`; card 'Neighbouring embarks join seamlessly', 2026-10-12)
- `--embark-seams [--tiles-center X,Y]` makes the embark at the tile's centre and the ones an
  embark (384 m) east and south of it, and a pair straddling the tile's east border, and prints
  the absolute levels across each shared edge against the same step inside one embark, cells
  with water on one side only and where the ground's kind differs; then the same embark on the
  border made from this region and from the region three tiles east ("Two regions: ... differs
  0.00 m"). Each embark counts levels from its own `z_min_m`, so compare `z_min_m / Z_STEP_M +
  surface_z`. Dev 45,12, 48,18 (mountain), 50,20, 47,14: across 0.01-0.20 levels where inside
  is 0.00-0.30; at most 1-2. One exception: a place's stair mouth cut into one embark at its
  edge (47,14 south: one cell 10 levels down) meets plain ground in the other; places belong to
  the embark they are carved in.
- The date line: x wraps but the noise does not. An embark's origin is shifted by whole world
  widths so its centre lies in the first (`canonical_x0`); 2 m tiles are hashed by their wrapped
  place (trees, boulders, sand and gravel, the wildlife's signs); every metre-scale noise read
  (relief, ridges, terraces, meanders, strata, groves, ore seams; `bank_near` too) goes through
  `across_date_line`, which within 2 km of the line mixes the reading with the one a world's
  width over, half and half on the line, so the two sides meet (95,10: 4.5 levels across the
  line before, 0.10 now against 0.07 inside). Nothing changes further than 2 km from the line.
  Tested (`tests/towns.rs` neighbouring_embarks_join: 48,18 and 95,10).

## Towns in embarks: roofs by people, rigid houses, squares (`structures.rs`; 2026-10-12)
- Roofs by the builders' architecture (card 'Architecture styles by culture'): `RoofPlan::style`
  (`RoofStyle::of(Site::arch, stone)`): tiles over stone and thatch over timber or earth as
  before, a canopy of growing leaves (living, in the star's leaf colour), wicker over and under
  (woven), hides on pale bone ribs (bone), faceted crystal with glints (crystal), standing seams
  (metalwork), turf (carved). `--local-snapshot` prints "Towns in the region, by their roofs: ..."
  and `PLANET_EMBARK_TOWN=<name>` embarks on that town's tile (dev: Dreammere crystal,
  Brolmdustoor bone, Slimestomp woven). In the coverage table (`coverage::roof`).
- Streets (card 'Organic street layouts'): each house is a rigid rectangle moved by the street
  warp at its middle (`House::off`, `Plan::rigid`; bent with the warp, a lot crossed by its slope
  had become an L) and squared to the grid (a slanting rectangle is a jagged diamond on 2 m cells;
  squared off a slanting street, its corners must keep off every street). The market square is a
  square, and lanes crossing main streets get a small paved square one time in two
  (`Plan::squares`). Streets still bend with the warp and follow the approach roads' angles.

## Mountains at human scale (card 'Mountains at human scale', 2026-10-12)
- Mountain and hill tiles were already cut into terraces (`terrace` in `generate_local`: a
  broad hillside of 7 steps, each a ledge and a face, amplitude from the tile's height and
  ruggedness, `mountain_at`). Faces of four levels had cut a camp off its own ground: now in
  about a fifth of each face's length a scree chute eases it over most of the step
  (`chute_noise`), so a walker can climb from ledge to ledge. This moved every hill embark (and
  the seed-pinned colony tests with them).
- On a mountain tile (amplitude 10+, not a hill) a face is the whole band where the ground falls
  three levels or more within two cells: bare rock (no snow or soil holds on it), and its foot is
  gravel scree with boulders. Hills keep only the step's top edge bare. The ink surface view
  (`local_ink::pixel`) shades rock faces like the world map's ranges: pale toward the light
  (north-west), hatched where they fall away from it (two levels or more across four cells, so a
  plain's bare rock is untouched); the steps keep their hachures.
- A living: ibex and wild goats on a mountain whose ecology keeps no other game; a berry
  thicket drawn toward the middle (where the camp is made) on a mountain, and summer shrubs on
  the lower slope (relief 12+). The site report says the trade: "a mountain (18 levels of
  relief): rich in stone and ore, easy to hold, cold, poor in timber" (food and timber when
  short).
- `--local-snapshot` prints "Relief: N levels across the embark; the highest face M levels".
  Dev 48,18: 27 levels, faces of 4; 9 of 9 alive at day 60 (all had starved by day 6; it was
  the doomed site of the colony tests, now 46,38). Mount Mistvale, the roof of the world
  (`--seed 42 --star sol --width 256 --height 128`, tile 104,104): 25 levels, faces of 6, a
  gorge by its lake. Plains are unchanged (no terrace, no face band, no shading).
- Doorways (`structures::raise`): a door on ground below the house's floor is open up to the
  floor's height (the lintel at the low ground's +2 left no headroom to step up). The chutes had
  put a step under one of Dreammere's wall towers and closed it off.

## Pinned sites on the audited terrain (2026-10-14)
- The integration tests run on terrain revision 2; the dev world is seed 10 (embark 30,34) and
  the sites quoted above (Greenburg 44,9, 46,13, 47,14, 50,20, 70,6, 48,8...) are seed 76's on
  revision 1. The tests now pin: the capital Sturmarkgate at 12,36 (`PLANET_EMBARK_TOWN`) and a
  walled town at offset 1250,0 (Thunderkeep) for towns on several levels; 42,32 a living beast's
  lair (Korix the Terrible) and a limestone cave walkable from the surface (caverns at 11, 26 and
  43 levels); 33,32 the tomb of Zreetsio, captain of Aiz'shaz (also the explore trial's site: the
  dev embark has no way in); 36,32 four world-river tiles for `--river-survey`; 69,8 a river camp
  (the wall left to the river by day 11, the field on the bottomland, the spring flood on day
  121); 0,35 a delve (cellar day 12, bedrooms 52, great hall 55); 6,44 a camp without stone that
  cuts a gallery for it (day 19); 36,23 the nameless dead of the Battle of the Mambaash Pass; 72,8
  the mine that wakes the deep, an aquifer and gems; 33,26 a hall in the hill and its engravings;
  48,8 and 36,41 sites refused or warned for want of food; 53,13 a camp that moves.

## Caves from the site layer (`local/caves.rs`; ROADMAP Stage 5, 2026-10-16)
- `caves::carve(map, region, cx, cy)` runs after the caverns and before `furnish` and
  `places::place`: every `region.caves` graph whose footprint crosses the embark is cut in
  absolute levels (`level = (z_m - z_min_m) / Z_STEP_M`), so two neighbouring embarks carve one
  cave and it runs on under their shared edge (`--embark-seams` on 42,32: 0.01 levels across
  the edge, as before). Node positions map from absolute region cells (cell k's centre at k +
  0.5, the frame `bicubic` reads) to columns (`col`). The passages are walked breadth-first from
  the `Mouth` node, so the place's cells begin at the mouth and end at the far end.
- Cuts: a passage (`cut_passage`) a column's step at a time along the segment, its floor
  following the nodes' levels a level at a time at most (walkable: `nav::steps3` climbs one
  level with headroom), kept `ROOF` (2) levels under the ground over the whole of its width
  (`roofed`), `width_m / 2` columns either side (a vadose canyon one column, a gallery three),
  `height_m / 2` levels of headroom (2-8); only the mouth's own passage may cut the ground open
  within `ENTRANCE_COLS` (4) of the mouth (the way in; a sump passing under the mouth stays
  under it). A `Chamber` (`cut_room`) is a round room of one floor level as low as the roof over
  all of it allows. A `Shaft` (`cut_shaft`) is a stair column (`Shape::Stair`) from the upper
  level to the lower, from the surface itself when the upper node is at the ground (a sink, a
  skylight). A `Skylight` opens a hole from the tube's floor to the sky. A flooded sump holds
  `WATER_FULL` in its cells, a vadose canyon `STREAM_WATER` (2) in its floor cell; a cleanup pass
  over the cave's columns drops water that a later cut left in rock or over open air (the
  audit's "water in a solid cell" and "floating water"). Columns with water in the way are left
  uncut (a river bed, a lake). `open_column` makes the floor `Floor` (gravel where it was air).
- Each graph becomes an `UnderPlace` (`PlaceKind::Cave`, the graph's name, the region place's
  cause and contents where its mouth has one, the mouth's column if it lies on the map), so the
  colony's finding, exploring and "the unknown is blank" work unchanged; `places::place` makes
  no hashed cave where a planned one was cut (`has_cave`). A cave is kilometres long and an
  embark 384 m across, so most embarks hold a piece: at seed 42's Skodroeskae (the Falls
  region, `PLANET_EMBARK_PLACE="cave mouth" --tiles-center 177,133`) the mouth at the middle,
  the dry gallery running south off the map, the sump's tail and the resurgence 30 columns
  north: 1,264 cells on 6 levels, audit clean.
- `PLANET_EMBARK_PLACE="<text>"` with `--local-snapshot` embarks with the region's nearest
  place whose kind's word or name holds the text at the embark's middle ("Embarking at X, a
  cave mouth (region cell ...)"); `--local-snapshot` prints a `Place:` line per place (cells,
  levels, mouth, cause) and draws the places' mouths on the surface image without a colony
  (`local_ink::draw_place_mouths`).
- The caverns take shape (`caverns::plan`): a cell opens where the broad noise peaks (rooms, as
  before) or along the zero lines of a second field (`lines`, scale 55 m across and 30 m down:
  tunnels ~8 m wide at the band's middle, `TUNNEL_NARROWNESS` 7, narrowing toward the band's
  edges, `TUNNEL_OPEN` 0.75), the detail roughening both; still a pure function of position
  (`neighbouring_embarks_line_up` holds). The dev cave site's first layer grew from 8,382 to
  15,144 floor cells; sections show rooms joined by narrow winding passages instead of blobs
  alone. Not done: an underground lake fed by the karst's shafts, a river that resurges, a
  chasm; ice and talus caves and mine adits.
