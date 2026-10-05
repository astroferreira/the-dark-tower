# Tile viewer (`src/tiles/`)

The default and only maintained front end (minifb window).

## Watching history being written (`tiles/watcher.rs`)
- `--watch` simulates the history in a window (DF world-gen style, dressed as the ink map),
  then opens the tile viewer. The simulation runs on a background thread at full speed
  (`HistoryEngine::begin` / `step` / `finish`) and *records* every season: a `Step` with the
  almanac, realms, places, events, new roads and a `Delta` of the `HistoryOverlay` (owners,
  roads, sites, fields/forest cover, Shadow corruption; `classify.rs`), with a full keyframe
  every 40 seasons. The window *plays the recording back* at the chosen pace, independent of
  how slow the simulation is, and can jump anywhere already written.
- Playback: Space pause, `[` `]` pace, `<` `>` one season back/forward, click or drag the
  timeline bar (pale = written, gold = shown, red playhead) to jump; the chronicle is rebuilt
  for the shown season. Each shown season = `base.clone()` + `apply_overlay` (~1-20 ms).
- Smooth view: wheel zoom eases towards its target, WASD/arrows pan (Shift faster), drag pans;
  while the view moves the map renders as a half-resolution preview (`render_world_lod`, same
  look) when a full render is over ~14 ms, and sharpens once still. `render_world` is
  row-parallel (rayon): ~6-15 ms fitted on 512x256, ~10-28 ms on the dev world (11 px/tile,
  detailed path), previews 2-10 ms. Per-tile flags keep the costly per-pixel work local:
  `shadow_near` / `dominion_edge` (Shadow ink), `owner_edge` (borders), road curves per tile.
- Map: settlements, fields, territories (faction wash + borders) and roads appear as they
  spread; `overlay_realms` draws borders/roads at zooms where `render_world` omits them. New
  roads glow, foundings ring, battles/razings/disasters flare; great events get a banner.
- Panel: year/season, timeline, almanac, souls sparkline, the Shadow, great realms. Chronicle
  below: key events (`style()` decides glyph, colour, map mark and whether it is key), `L` = all.
- `--watch-snapshot PREFIX --headless` renders frames at 1/4, 1/2, the end and a close-up to
  PNG without a window (for checking the look). The window doesn't redraw while hidden, so
  screen capture is unreliable; use this.

## Start screen (`tiles/start.rs`)
- Shown when the game runs with no arguments, or with `--start` (other flags pre-fill it).
  Rows: world size (Dev 96x48 / Small / Standard / Large), seed (type digits, R rerolls),
  shape of the lands (`WorldStyle`), tectonic plates, age of the crust (`--tectonic-myr`),
  fantasy, written history (years, 0 = none), founding peoples, the Shadow, watch it unfold;
  a help panel explains the selected row and a rough time estimate shows by Begin.
- `main` copies the choices into `Args` (width, height, seed, world_style, plates,
  tectonic_myr, fantasy, history_years / no_history, civilizations, no_shadow, watch) and
  generation continues as with flags. The window closes during world generation (progress is
  printed); the watcher or tile viewer opens after.
- Shared drawing for the viewer's own screens (palette, parchment `card`, `heading`, `wrap`,
  ...) lives in `tiles/ui.rs`.

## Roads and tile colours (`tiles/classify.rs`)
- Roads and rivers are drawn as curves (`Strokes::build`): each linked tile gets a
  hash-jittered node (separate patterns for roads and rivers); links pass through the
  midpoints between nodes; a two-link tile is a quadratic curve bending at its node,
  ends/junctions are spokes. Stored per tile as segments with a half-width
  (`road_strokes`, `river_strokes`; river width blends between tiles and grows downstream);
  `Strokes::edge_distance` is the per-pixel query used by `render.rs`, and the watcher's
  far-zoom overlay draws the road segments.
- Flat tile colours (minimap, far zoom) composite the sprite over the ground by opacity
  (`Atlas::average_over`); averaging only a sprite's opaque pixels was mostly ink outline and
  speckled zoomed-out maps with dark dots.

## The inspector (`tiles/inspector.rs`)
- On the world map a click (press and release without moving) opens a parchment panel on the
  right for the tile: the town or ruin on it, else a beast's lair, else the realm whose land it
  is (`Subject::Tile` resolves in `page`). Pages: settlement (held by, founded, the Shadow's
  frontier, its story), realm (ruler, towns, seat, wars at present, lately), beast (alive or
  slain, lair, deeds), event (year, text, place and peoples, "Because" = its cause chain up to
  5 back, "It led to" = events caused by it). Every event line links to its event page and
  ends in a red "why?" when it has a cause. Clicks inside the panel follow links (a page stack),
  Backspace or a right click goes back, Esc closes it (Esc quits only with no panel open);
  pressing on the panel doesn't pan the map.
- `--inspect X,Y [--inspect-follow 2,0]` renders the map at 16 px with the panel open, and one
  frame per followed link, to `inspect_N.png` (`viewer::save_inspect_snapshots`). On the dev
  world, ruin 70,6 -> follow 2 shows the razing whose "Because" holds the siege and the war.
- The window wiring (click, Backspace, right click, Esc) has not been tried by hand yet.

## Embarks in ink (`tiles/local_ink.rs`)
- The surface view of a playable area (`render_local` with `surface_view`, the default on
  embarking; `V` switches to z-level slices, which keep the atlas tiles) is drawn per pixel from
  the map, not from atlas tiles: material washes blended between cell centres with watercolour
  mottling; flat tree symbols like the world map's (broadleaf ~4 m across, scalloped; firs
  ~3 m, pointed) with an ink outline, a few leaf flecks and a faint offset shadow (no hatched
  shading: at 2 m cells it read as 3D, oversized canopy); water as a smooth contour of a blended wet field with an ink
  bank, pale shallows and ripple strokes; steps in the ground inked with hachures downhill;
  pitched roofs over standing houses (`LocalMap::roofs` / `houses`, filled by
  `local/structures.rs`: ridge along the footprint's principal axis, tile or thatch courses,
  shadow slope hatched, ink eaves); cobbles, furrows, tufts, burrows, boulders.
- Each frame precomputes every column once (`View::new`); pixels only index arrays. 1024x640
  at 16 px/cell ~14 ms, 1280x800 at 6 px ~29 ms (dense forest, the worst case), ~8-12 ms in town.
- `--local-snapshot` prints those timings; `PLANET_LOCAL_CLOSE="x,y"` moves its 16 px close-up.

## Seasons in the tile viewer
`T` steps Spring/Summer/Autumn/Winter, `C` cycles them automatically. Snow cover (cold + moisture),
foliage colour (spring flush, summer drought, autumn orange), and frozen lakes/rivers/shallows
come from the seasonal climate; `--season` picks the season for `--tiles-snapshot`. Not yet applied
to zoomed regions or embarks.

## Graphical tile viewer (`src/tiles/`)
`cargo run --release -- --seed 42` opens a window (minifb) that draws the world with
pixel-art tiles (the default front end; `--tiles` is accepted but no longer needed).
- Mouse wheel / `+` `-` zoom (1-64 px per tile, around the cursor); drag or arrows/WASD pan;
  click the minimap to jump; `N` minimap; `P` screenshot; `Q`/`Esc` quit. Hover info is shown
  in the window title.
- `Z` walks into the high-resolution region under the mouse: arrows/WASD walk (Shift runs),
  wheel zooms, `F` resets zoom, `X` saves the region PNG, `Esc`/`Z` returns to the map (centred
  where you walked to). Near a region edge the next region is generated on a background thread
  and swapped in; zoom terrain is seamless, so you can walk across the world.
- `--tiles-center X,Y` start position; `--tiles-snapshot PREFIX` renders overview/3px/16px/32px
  frames to PNG without a window (for checking rendering; 3.5 px/tile is just below the
  detailed-tile threshold) and prints each frame's render time (best of three; seed 42: ~11 ms
  overview and 3px, ~16-21 ms at 16/32 px).
- Far zoom (< 4 px/tile) draws flat tile colours: land/water per pixel from the smooth shoreline
  field and the colour from a domain-warped tile lookup of the same class (`far_color`), so
  lakes, shallows and biome patches don't show the tile grid.
- Data overlays (`tiles/overlays.rs`; the old terminal explorer's V views): `O` cycles (Shift+O
  back) Map, Height, Temperature, Moisture, Drainage (log flow), Tectonic plates, Tectonic
  stress, Biomes. Muted palettes washed over the map (`render.rs::overlay_tint`, modulated by
  the map's own light/dark so ink lines stay readable), a legend card bottom-left, and the
  value under the cursor in the window title. `--overlay NAME` applies one to
  `--tiles-snapshot`.
- Tiles: `atlas.rs` draws a 32x32 atlas in an ink-cartography style (ground kinds +
  transparent sprites, 4 variants each): muted washes on parchment, sepia ink outlines, shadow
  sides hatched (never darkened), symbols built from shape masks so all share one treatment.
  Keep new tiles in that style (palette consts `INK`, `SEA_INK`, `PAPER`, `STONE`, ...). `--export-tileset atlas.png` writes it (one row per kind, in `ALL_KINDS`
  order) for editing; `--tileset file.png` loads an edited atlas or a Dwarf Fortress style
  16x16 CP437 sheet (glyphs tinted with per-kind fg/bg colours, magenta = background).
- `classify.rs` maps world data to tiles (biome -> ground + sprite, relief overrides,
  beaches, lakes >= 4 tiles (frozen lakes stay `Lake` tiles flagged `lake_ice` and are drawn
  iced over, so they keep the smooth inked shore), rivers from D8 flow, one-tile-wide water
  strips drawn as river channels); `render.rs` is pure software rendering, per pixel: land/water comes from a smooth
  noisy field between tile centres, sampled through a domain warp (0.42 tiles) so lakes and
  coasts don't keep the tiles' rounded-rectangle outline (ink coastline, pale wash and two
  offshore ripple lines on its 0.5 contour), ground kinds and territory borders are looked up through a domain warp so
  borders meander, the deep-ocean edge is a contour of a smooth depth field, snow/season tint
  blend between tiles, rivers get ink banks, and atlas tiles larger than the screen cell are
  2x2 supersampled. Labels are ink on a parchment halo.
