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
- The opening of the game: treasures made, found and lost are not key events; once a decade one
  line sums them ("In ten years 14 treasures were made, found or lost"; recoveries had been 13 of
  the last 20 key events). Playback is paced by drama: a season with a great event (major kinds,
  razings, the Shadow's conquests and its check) lingers 2.5x, a season with no key event
  passes at 0.35x. When the age is written a "The world today" card (sent by the simulation as
  `Msg::Present`: the world's sentence, peoples/wars/grudges, the Shadow's frontier and the
  stronghold in its path, its weakness, beasts near towns, recent falls) ends it, then "ENTER
  choose where to settle". The camera never runs past the poles (`clamp_cy`).
- `--watch-snapshot PREFIX --headless` renders frames at 1/4, 1/2, the end and a close-up to
  PNG without a window (for checking the look). The window doesn't redraw while hidden, so
  screen capture is unreliable; use this.

## Start screen (`tiles/start.rs`)
- Shown when the game runs with no arguments, or with `--start` (other flags pre-fill it).
  Rows: world size (Dev 96x48 / Small / Standard / Large), seed (type digits, R rerolls),
  shape of the lands (`WorldStyle`), tectonic plates, age of the crust (`--tectonic-myr`),
  fantasy, written history (years, 0 = none), founding peoples, the Shadow, watch it unfold;
  a help panel explains the selected row and a rough time estimate shows by Begin (fitted to
  M4 Pro timings, world and 250 years together: Small 3.4 s, Standard 11-14 s, Large 66 s).
  "Watch it unfold" is on by default: the history being written is the game's opening.
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
come from the seasonal climate; `--season` picks the season for `--tiles-snapshot`. The colony's
embark has its own winter (snow wash, ice); the region walker takes the season too (T there:
snow by the season's temperature and moisture, bare broadleaves, ice on lakes and rivers, autumn
leaves; `region_ink::draw`'s `season`; `--local-snapshot` writes `_region_4px_autumn.png` and
`_region_4px_winter.png`).

## Graphical tile viewer (`src/tiles/`)
`cargo run --release -- --seed 42` opens a window (minifb) titled "The Dark Tower" that draws
the world with pixel-art tiles (the default front end; `--tiles` is accepted but no longer
needed). Without `--tiles-center` it opens at 16 px on the stronghold in the Shadow's path
(`PresentDay::stronghold`), or fitted to the window when there is no history; a parchment chip
at the bottom says "Z: walk into the land under the mouse. Enter there: settle."
(`colony_hud::draw_hint`). Not yet tried in a real window.
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

### Timelapse (`tiles/watcher.rs::export_timelapse`)
- `--watch-timelapse FILE` (headless: simulates the history as `--watch` does) or `G` in the
  watcher window (writes `timelapse_<seed>.gif` from what is recorded so far) renders the
  recording as an animated GIF: one frame a year (the same season every frame, so snow doesn't
  flicker) at 10 fps, the map fitted, 960x600, the last frame held 4 s, drawn as if playing.
- Frames are deltas (`gif` crate): only the box of pixels that moved more than
  `LAPSE_TOLERANCE` (14 per channel) from what the GIF shows is written, the rest transparent.
  Full frames were 85 MB for the dev world; deltas of every exact change 29 MB (the Shadow wash
  and territories shift by a shade each season); with the tolerance and one frame a year: dev
  5.4 MB, seed 42 6.6 MB, ~5 s to write.

### Plates (`tiles/plates.rs`)
- `P` in the world view saves a plate, not a raw frame: the map without the interface (labels
  kept), a parchment margin with a ruled border, a cartouche ("The Lands of <largest continent>",
  season and year, seed), the world's one sentence as a caption (`lore::claims`), and a legend of
  the five realms with most land in view; numbered `plates/plate_<seed>_NNN.png`.
- The PNG text chunks carry `world-args` (the command line that makes the world, recorded in
  `main` via `plates::set_world_args`), `view` (x, y, px per tile), `seed` and `caption`.
  `--plate FILE` re-runs the game with those arguments plus `--tiles-center` and `--tiles-zoom`;
  with `--headless --tiles-snapshot P` it re-renders the plate (`P_plate.png`), byte-identical
  on the dev world. `--tiles-snapshot` always writes `<prefix>_plate.png` (16 px at the centre).
- Not yet: typed captions, automatic plates at great events (an album).

### Heraldry (`tiles/heraldry.rs`)
- `arms_of(world, history, faction)`: shield shape by race (heater, square for dwarves and
  giants, kite for elves and fey, round targe for orcs/goblins/beastfolk/halflings, banner for
  the undead and elementals); field, division (plain, per pale/fess/bend, chevron, quarterly) and
  tinctures from a hash of the realm (a metal charge on a colour); the charge from the seat's
  ground (waves on a coast, a wavy bar by a river, a mount over 1000 m, a tree in forest, a sun
  in desert or savanna, else a star). The Shadow's realm: a red eye on sable. `draw` renders it
  per pixel with ink outlines at any size.
- Shown on the watcher's realm list, the inspector's realm page (48 px by the title) and the
  plate legend. `--arms-sheet FILE` draws every living realm at 96 and 24 px.

### Lettering (`tiles/fonts.rs`, `text.rs::place_labels`)
- Map labels are set in IM Fell English (roman, italic, small caps; OFL, `assets/fonts/`, with
  `OFL.txt`) rasterised by `fontdue` with a glyph cache and a soft parchment halo. The 8x8 bitmap
  font remains for panels and UI.
- `LabelStyle` sets the hierarchy: oceans in wide-spaced italic capitals, seas and gulfs smaller,
  continents and islands in spaced small caps, ranges in small caps, regions in italic, rivers and
  lakes in blue italic, capitals in small caps, cities and towns in roman, ruins in italic.
- Placement: highest rank first; a label must fit wholly on screen (never clipped), overlap no
  placed label or `avoid` rectangle (the minimap: `minimap_box`), and a name is shown once.
  ~0.5 ms a frame. Ranges are lettered along their long axis (when 1.8x longer than wide) and
  rivers along their course near the label (`viewer::feature_angle`: principal axis of the
  range's tiles or of the main stem within 6 tiles; within +-60 degrees, reading left to right;
  `Label::angle`, drawn by `fonts::draw_rotated`, letters turned and sampled bilinearly; the
  collision box is the turned box's bounds). Not curved along a bending course.

### Poster (`viewer::save_poster`)
- `--poster FILE [--poster-width 6144]` renders the whole ink map at ~12 px/tile (seed 42:
  6144x3072, ~0.6 s, 24 MB PNG) with the lettering scaled (`place_labels_scaled`), realm
  borders, the Shadow's dominion and ruins as in the viewer, the forty bloodiest battles as red
  crossed swords with their year, a cartouche ("The Annals of <world>", the year, the world's
  sentence), a scale bar (round steps, 4 segments), a compass rose in the most open sea and the
  vintage ruled border (`cartography/decorations.rs`). Works with `--load-world`.
- Not yet: the atlas of ages (four small maps at years 201, 280, 360, 451).

### Portraits (`tiles/portraits.rs`)
- `of_settler` builds an ink head from parts: skin, ears, tusks and beards by race (orcs green
  with tusks, elves pale with pointed ears, dwarves bearded), hair, beard, dress and head shape by
  a hash of the name, headgear (hood, cap, circlet, helm), glowing eyes and pale wisps for the
  undead (all-undead parties had looked alike: 10% apart), wrinkles from age 55; marks from the past: a scar from the battle a
  veteran fought ("From the Battle of X", linked to its event) or the night of the raid they
  survived (`viewer::wounded_in`), an eye patch for some of the scarred, grey hair from 50 or
  from 35 for those who lost their town. `draw` renders it at any size with ink outlines.
- Shown on the inspector's settler page (60 px by the title, with a "Scar:" line linking to the
  event) and in the saga's cast (40 px). `--sim-snapshot` writes `<prefix>_faces.png` and
  prints how far apart the two most alike faces are at 48 px (dev 21%, seed 23 22%; tested >= 15%).

### Ground, scars and ranges (`render.rs`)
- Wet edges: from 4 px a second, looser warp (0.55 tiles) picks a neighbouring ground kind; where
  it differs (same land/water class) the two atlas textures blend ~40-60%, so biomes bleed into
  each other instead of meeting at a line.
- Scarred land has its own ink (`scar_ink`): ashlands (Ash ground) darker with a crack network
  and red-black cinders; dead woods (tundra + dead trees) grey-brown with fallen trunks; bone
  fields pale ochre with scattered bones.
- Ranges (`mountain_ink`, from 6 px, replacing the Mountain/SnowPeak sprites): peaks on a
  jittered half-tile grid wherever a mountain tile lies beneath, height 0.3-0.9 tiles by
  elevation (`TileWorld::elev`), overlapping front to back, lit face pale, shadow face
  hatched, inked slopes, snow caps only near the top of cold or high peaks; ground snow on
  mountain tiles is cut to 30% so the white sits on the peaks. Seed 42 render times unchanged
  (~16.5 ms at 16 and 32 px).

### The camp at a glance (`local_ink::draw_colony`)
- Settlers are inked head-and-shoulders figures (~14x18 px at 16 px a cell, scaled 0.55-1.4) in
  their portrait's skin, hair and dress (`settler_looks`: a dress that would make two living
  settlers alike shifts to a spare colour), drawn between cells as they walk, greyed when ill,
  with a pictogram for the job: axe, pick, hammer, basket, rod, a spear for the night's watch, a
  z for sleep, the item carried. Those asleep under a roof are lettered on it ("6 within").
  Names in IM Fell italic with a halo step aside (above, below, right, left) or are left out.
  Stumps are a small faint ring.
- Night (`Colony::darkness`: 0.55 from 21 to 5, ramps at dusk 19-21 and dawn 5-7) washes the
  frame toward sea-ink blue, with a warm glow round the fire (7 cells) and the watcher (3).
  `--sim-snapshot` writes `<prefix>_noon.png` / `<prefix>_midnight.png` (the day after the run)
  and prints "Figures: N of N ... look different" and "Night: the midnight frame is 33% darker";
  tested. `draw_colony` takes the history (for the portraits' races).

### One hand for the words (IM Fell on the reading surfaces)
- The inspector panel (`inspector::draw`), the watcher's chronicle (`draw_log`) and its closing
  card (the world today and the three sites to settle, drawn last so nothing shows through, over
  the whole window when it needs the room) are lettered in IM Fell: small-caps titles and
  headings, 15 px roman body, red italic year rubrics, great events in small caps, faded text in
  a darker brown (0x5A4634, ~5:1 on parchment). `fonts::wrap` wraps to a pixel width.
- The viewer's camera is clamped at the poles (`viewer::clamp_cy`, as the watcher's) and the
  world beyond the map is parchment (`render::OFF_MAP`), so 16 px frames and plates have no
  black band.
- Since the ui-graphics branch every surface is lettered in IM Fell: the watcher's side panel,
  the start screen, plates, overlay legends, the debug sheets (province, faces, arms); the 8x8
  bitmap font (`draw_ink`) is no longer called. The minimap is framed in parchment and sepia.

### Fewer freezes (card 'One window from Begin to the camp', partly)
- Z on an unvisited tile surveys the region on a worker (`surveying` in the viewer loop): the
  map stays live under a chip "Surveying the land around X,Y... N s", and the walker steps in
  when it is ready. Before, the window froze 11-15 s with the frame dimmed.
- Walking: the site line ("what an embark here would hold") is read only once the walker has
  stopped for 0.35 s ("the site is read when you stop"); generating it near a town costs up to
  a second and stalled walking past one.
- Not done: keeping the start window open during world generation (generation runs on the main
  thread in `main`), bucketing river segments and houses, writing the timelapse on a worker.
  Not tried in a real window.

- The colony's winter (`local_ink::draw_colony`, 2026-10-08): in a hard winter the embark pales
  toward snow (28%, 42% in a deep freeze), lighter ground more than ink, before the night wash;
  the map's own colours stay annual. Checked on seed 3's day-100 frame (`PLANET_FRAMES`).

- The delve in the window (2026-10-08): `<` / `>` go through the levels; each level is drawn by
  `render_level_ink` and `local_ink::draw_delve` (replacing the dots of `draw_settlers_by_level`,
  now a wrapper): a caption ("Level 51 - 3 below the camp's ground: the cellar, bedrooms (being
  dug)"), the dig's cuts still to make as dashed red outlines (the one being dug filled), a bed
  and door in each bedroom with its owner's name when they are not in it, the great hall's table
  and benches, room names, and the settlers on that level as on the surface (a level off: faint).
  The surface view hides those below and letters "N below" by the delve's mouth, which is drawn
  as a stair going down. The section (U) draws stairs, room floors as solid, and settlers at
  their own level.
- The level view and the surface view agree (2026-10-08, after "they do not seem compatible"):
  `local_ink::draw_level` draws, within two levels of the camp's ground, the camp exactly as the
  surface view does (`draw_colony`: fire, store, marks, crops, creatures, night and winter, those
  on the surface) before the delve's rooms and those below; ground one or two levels above the
  viewed level is the surface as drawn, shaded with hachures and inked where it meets this
  level (it had been cut-rock hatching, which made a slope look like a quarry face), and only
  ground deeper than that is cut; `<` / `>` or V from the surface view start from the ground's
  level under the view's centre (they had started from the map centre's). `PLANET_FRAMES` also
  writes `_camp_level.png` and `_camp_surface.png` to compare the two.
- `[` / `]` in the embark (2026-10-08): up or down to the next level worth looking at
  (`Colony::delve_levels`: the tower's platform, the camp's ground, levels with rooms or rooms
  being dug, the cavern the stair reaches, the stair's foot); `[` past the top returns to the
  surface view. "[/] delve" in the key bar.
- Visual pass on the embark frames (2026-10-08): worn ground is a smooth dusty wash blended
  between cells (was a disc per cell: polka dots over every camp), never on roofs or walls;
  in a level view the camp's marks, worn ground, night and winter are drawn only where the level
  shows the surface (`level_shows_surface` mask: no dots or "cut away" hatching over rock and
  dug rooms); labels share one placement list between `draw_colony` and `draw_delve` (roof
  counts, "N below", patron names first; settlers' names and bed names step aside, 3 px air);
  level views draw settlers with the surface figure (`draw_figure`: pictograms; a pick for the
  dig, a hammer for craft, a spear for the hunt). Ground below the viewed level fades with depth
  (10% a level down ... 72%) instead of 72% at once with square halos round trees. Rock cut at
  a level (`cut_rock`, shared with the cover over unfound places, which now takes the nearest
  rock of the level so no stratum ghost shows): the aquifer a blue tint with a few ripples (was
  blue noise), gems inked. Doors are a plank leaf across the doorway (were crate squares); a
  tower's top has planks and a merlon parapet; stair marks were inverted (up drew v): up ^,
  down v, both X (`stair_mark`). Drawbridges drawn as planked decks with rails, a hatched leaf
  when raised. Stumps faint and off-grid. Magma mottled with thin crust veins (level and
  section). Section: stairs in profile (treads and risers), fungus as stalk and cap, strata named
  down the left, settlers as inked figures in their colours, deep rock under the last level
  (was sky). `PLANET_FRAMES` also writes `_foot.png` (stair's foot) and `_magma.png` (the pipe)
  and prints best-of-three frame times (~12-13 ms each at 1280x800, 16 px, unloaded M4 Pro).
  `--local-snapshot`'s `_section.png` is the ink section too, down through the strata and caverns
  to the magma (`render_section_ink_to`, a camp founded on a copy of the map for its ground).

### The colony's ledger and things on the ground (2026-10-08, half-done)
- `colony_ui.rs`: a parchment panel on the right (40% of the window, 400-600 px) with five tabs
  (Settlers C, Stocks I, Works O, Annals L, Camp T) and a settler sheet; a button bar along the
  bottom for the patron's verbs, stones, names and the clock. Leaves scroll in their own buffer
  (`Pane`), every click is an `Action` hit for the viewer to carry out. `--sim-snapshot` writes
  `<prefix>_ui_settlers/_stocks/_works/_annals/_camp/_sheet.png` and `_ui_store.png` (the store
  close up with the hover chip and every glyph in a strip) and prints "UI leaf: N things to
  click"; `tests/colony.rs::the_ledger_leaves_render`.
- Wired into the window (2026-10-08, not yet tried by hand): `viewer.rs` keeps a `UiState`,
  draws `colony_ui::draw` after the HUD (`right: reserve_right`, `selected`, `hide_chip` over
  the UI, `bar: false`), C/I/O/L/T toggle the tabs, the wheel over `body_rect` scrolls the leaf
  (and does not zoom), a press on the UI starts no drag, clicks run `Action`s through the same
  calls as the keys (Space/skip by flags into the existing code), armed `Tool`s apply on the next
  map click (bless/forbid stay armed; the hint chip shows the prompt), a click on a settler on
  the map opens their sheet (`actions_sheet`: the clock stops, `paused_by_sheet` restarts it),
  `dream_buttons` under the dream card, Esc order: inspector/card/dream first, then tool, sheet,
  panel, then leave. `Tool::NamePlace` asks for the name (`place_input`) and calls `name_place`,
  which is NOT recorded in `interventions` (a replay loses the name). The refugees' card carries
  its two answers as buttons (`choice_buttons`); `--ui-drive` clicks "take them in" (step 25,
  `_ui_drive_refugees.png`).
- Items: `colony::Item` carries `what: Stuff` (berries, fish, meat, grain, cave fungus,
  provisions, timber, stone; set where the load is made, kept through hauling; the simulation
  never reads it: the colony hash is unchanged). `glyphs.rs` draws 24 ink glyphs (one stamp:
  wash, ink rim, lit side pale, far side hatched from 9 px). The map shows loose things as their
  glyph (piled per cell, a count past 3) and the store as one heap per kind round the fire
  (`local_ink::store_heaps`, fixed slots; counts lettered from 10 px); carriers show the load's
  glyph; the hover chip names a heap or a thing on the ground.

### A sprite for everything (branch ui-graphics, 2026-10-08)
One ink treatment for every sprite (`ink.rs`: `Pen` draws shapes in a unit box with the atlas's
wash, sepia rim, lit top-left edge and hatched far side; `facing_left` turns a sprite while the
light stays top-left; `bone` = a pale stroke with an ink rim, for horns, poles, ropes). Review
everything with `--sprite-sheet FILE.png` (bestiary; `FILE_folk.png` bubbles, strangers,
furniture, artifacts; `FILE_moments.png` the moment roundels). The progress doc with images is
the Claude Doc "Ink graphics: sprites and examples".
- `beasts.rs`: body plans (profile quadruped with species proportions; spider, insect, crab,
  lizard, serpent, worm from above; bat, fish, bird, blob, great two-legged) + parts.
  `of_name` (game, herds, pets, cave life, the risen dead, werebeasts; whole-word match, "ox"
  is not in "fox"), `of_monster` (base, tweaks, colour or substance, eyes, glow), `px_for` (56 px
  per settler-width at the figures' scale), `world_beasts`/`draw_world` (legendary beasts at
  their lairs on the world map from 6 px a tile; names via `build_labels`, rank 650).
- `folk.rs`: raiders by race and threat kind (Shadow: horned, red-eyed; war band: helms, spears,
  shields in `band_colours`; outlaws: hoods, clubs, bows), traders; drawn at 1.2x a settler.
- `local_ink::draw_creature`/`creature_look`: Beast -> the arc's or a cavern's or an
  expedition's monster; faces its next step, legs step by cell parity, game grazes; traders lead
  a laden mule.
- `camp_ink.rs`: fire, palisade stakes (wall cells within the ring not in a footprint), each
  work's sprite (earth painted over stamped wall blocks first), jetty, chimneys, bell-cote,
  banners, signboards (`emblem`), graves, standing/hall stones, stone marks by title, cages.
  Store heaps step off roofs and the fire's ring (`store_heaps`).
- `furniture.rs`: level-view fittings (beds in the owner's dress colour, table, coffins,
  workshops, cellar, hatch, lair, ore cart, artifacts by kind).
- `status_ink.rs`: `emblem_of` (break > strange mood > wound > need act > meal > drink > gloom)
  drawn as a bubble by the head; `figure_marks` (bandage, office headgear, visitor's hat).
  Children are drawn smaller by `past.age`.
- `fx_ink.rs`: blows and blood (attacker within 1.6 cells), beast breath by its attack's effect,
  the clash's melee and aftermath (`Colony::clash_tick`, cosmetic), siege tents, ghosts at night,
  the bell, evil weather (`Colony::evil_weather_over`, read only) and snow.
- `vignette.rs`: the moment card's roundel (`draw_moment` takes the colony; cards are 640 px with
  it); a raid's beast from `Colony::foes_seen` (cosmetic, filled in `send_attackers`).
- `--sim-snapshot` adds `_raidclose.png`, `_clash.png`, `_wild.png`. The colony hash (chronicle +
  log) is unchanged by all of this.
- Later the same day: job pictograms are glyphs (axe, pick, hammer, berries, rod and fish,
  spear); worn armour over the dress; clothes fade and patch at 120 days, rags at 180; the stocks;
  role badges with the role's glyph; children smaller. The pen's beasts amble (`colony.pen` is a
  count); crops by season; timber walls rising (`camp_ink::draw_rising`); a woodshed's lean-to;
  a head-frame over the mine; haunts (cairn, bench, carved post); engravings as carved panels with
  a scene (`draw_engraving`); kept works by their keeper's bed; a prisoner bound to a post; a
  signpost for those away. Glyphs: 46 (`Glyph::ALL`, `Glyph::of_thing`), animals in the ledger
  as sprites (`ledger_beast`). Section view: trees, roofs, creatures. Watcher: ink event icons
  (`watcher::draw_event_icon`), panel/banner in IM Fell; start screen in IM Fell with a picture
  per row (`--start-snapshot FILE`); plates/overlay legend in IM Fell (`text::draw_fell`);
  inspector pages carry a `Picture` (a beast's sprite, an artifact's glyph); beasts on plates and
  the poster. `tests/sprites.rs` checks the sheet.
- Speed: the camp's overlay costs ~3 ms at 1280x800 (sprites ~1 ms; worn ground ~2 ms; the night
  wash is row-parallel now). `PLANET_FRAMES` prints "the ground alone" beside the frame times.
- World map's living things (`world_ink.rs`, `world_life` once per world): monuments by type,
  a war host at each active war's latest battle (the history never fills `armies`), siege
  camps, outlaw camps (`bands::of`), caravans on active trade routes, creature populations (none
  on the dev world), cult altars; up to five per tile, the living first. `--tiles-snapshot`
  prints "World life: ...". Guests by calling (`status_ink::guest_marks`), the vampire noticed,
  the lost relic, the hall in the hill's hearth, the lined shaft, cave and halls ends, a dream
  bubble, the envoy's roundel, scorched ground, a cradle by an expecting mother's bed, a snatched
  child on a raider's back, the mandate's notice post.
- The ledger is verified by `--ui-drive PREFIX` (`apply_ui_action` / `apply_map_click` are the
  window's own click code): 24 of 24 steps; `tests/sprites.rs`. Not yet tried by a person.
- The full inventory (every simulated kind and its sprite) is in the Claude Doc "Ink graphics:
  sprites and examples".
- `--inventory PREFIX` renders every simulated kind in the game's own frame (a test camp, works
  raised by `project_step`; `<prefix>_inventory.png`); tested. It found legibility bugs the sprite
  sheet hid (office headgear 2 px wide at the camp's zoom, names over hats, fliers too high).
- The real window can be driven by `PLANET_UI_SCRIPT=FILE` (with `--dev-embark`): lines
  `<frame> click X Y | key K | act <Action> | clicksettler | report | shot FILE | quit`; `act`
  clicks the centre of the hit area the window returned for that action. A full session
  (welcome card, tabs, sheet and pause, shrine and bless tools on the map, bell, speed, a
  settler clicked on the map, Esc order) runs as it should; it found the hover chip running
  under the open panel (now clamped beside it). Needs a desktop session; not in the tests.
- Thoughts, the dream of a lifetime, a vow, the patron's dream and the trading town's request
  have icons in the ledger (`status_ink::thought_emblem`, `life_emblem`, `draw_icon`); the talk
  bubble shows its topic. `PatronStyle` is a test harness (`raid_trial`), not game state.
- Coverage the compiler checks (`coverage.rs`, `--coverage`): every variant of 25 simulated enums
  (creatures, works, rooms, marks, stones, stuff, items, jobs, needs, breaks, moods, dreams, life
  dreams, mandates, visits, threats, places, talk topics, wants, thoughts (`Feel`), monuments,
  shapes, plants, trees) maps to the routine that draws it in a match with no wildcard arm, so a
  new kind without a sprite does not compile; `--coverage` prints the 251 rows;
  `tests/sprites.rs::every_simulated_kind_names_its_drawing`.
- `--inventory` also writes `<prefix>_zooms.png`: the snatched child, office headgear, a bandage,
  a bubble, the guest's hat, the beast and the fire drawn afresh at 8, 12, 16, 24 and 32 px a
  cell. It showed the snatched child drawn larger than the raider carrying them (28 px at
  scale 1); the child is now 19 px, sat on the raider's head (`folk::draw` returns its top).
- The region walker in ink (`region_ink.rs`, 2026-10-09): Z's zoomed region had been
  `render_rgb` blown up (flat climate colours, pixel rivers), the one view out of style. Now per
  pixel from the region's fields sampled between cells: washes muted toward parchment, hatching
  on the far side of slopes, contours every 100 m (500 m heavier), sea/lakes/rivers as one water
  field with an ink bank on its 0.5 contour, an offshore line and wave marks, tree symbols on a
  jittered 14 px grid (broadleaf/fir by temperature, nearer trees over farther), the lore's
  fields and roads muted. ~7-11 ms at 1280x800. The marker and the embark box are rubric ink
  (dashed box). `--local-snapshot` writes `_region.png` (1 px a cell), `_region_4px.png`,
  `_region_12px.png` and prints the frame times.
- `PLANET_UI_SCRIPT` keys: any letter or digit, Space, Escape, Enter, Tab, Backspace, `<`, `>`,
  `[`, `]` (`viewer::script_key`). A full pass (tabs, sheet, bell, speed, a settler clicked,
  level view, `[`/`]`, section, leave to the region, the world map, the inspector on a town and on
  the sea, overlays) runs in the real window. It found the level caption under the moment banner
  and the section title under the camp card (both now top centre below the banner), the
  section's trees as thin poplars (now crowns as wide as the surface view's), and the sea titled
  "Unclaimed land" (now "Open water").
- Thoughts by what was felt (2026-10-09): `Thought::feel` keeps the `Feel` (drawing only; the
  colony hash is unchanged), and the ledger draws its sign through `coverage::feel` (the
  exhaustive map), not by guessing from the text. `coverage::feels()` lists one of each of the 51
  thoughts (the table now has a row each: 302 rows); `--sprite-sheet` writes a fourth page,
  `_thoughts.png`; `tests/sprites.rs::every_thought_has_its_sign` counts the enum's variants in
  the source. Nine signs were added where thoughts had shared one (alone, no shrine, ill news,
  rags, a need unmet, thirst, a grievance (a mandate against their values had shown a smile),
  sickness, half rations), then 24 more (2026-10-09) so that every thought has its own sign
  (a memorial, an empty basket, a red slash, a shield, clasped hands, a cave mouth, an eye in the
  dark, a fallen horned head, a green eye, the stocks, a split shield, two cups, a wall being
  laid, a key, a spear through a skull, a stump and axe, stone and log with a heart, bunting, a
  bed, a paw, a letter with a green ribbon, a trader's sack, a drum, a star in a laurel); the
  test fails if two thoughts share one.
- The section's dug rooms show their fittings in profile and their names beside them (bed,
  table and benches, barrels, mushroom plots, coffin, benches, furnaces with their glow).
- The minimap: a parchment mat in a sepia rule, the view a rubric box (was grey and yellow).
- Live pass on a grown camp (`--code 76.96x48.earthlike.8.250@45,12 --day 200` with
  `PLANET_UI_SCRIPT`): `]` steps through levels 52 (cellar, farm), 48 (great hall, workshops),
  44 (bedrooms, kiln) and on; `[` comes back to the surface; the section and the minimap checked.
