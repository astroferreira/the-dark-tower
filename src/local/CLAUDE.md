# Playable areas / embarks (`src/local/`)

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

