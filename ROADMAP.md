# Roadmap: a land worth exploring

Direction since 2026-10-15: **generation only**. The region view (`Z` on the world map) and the
embark map (`Enter` in the region) must shine on their own, as places to look at and explore:
landforms you can name, caves that are systems, springs and falls and gorges where the rock and
the water put them, and the marks of the people who lived here before the chronicle began. No
colony, charter, genesis or adventure work in this period (their code stays, behind their flags;
their notes are in `docs/colony/` and each module's `CLAUDE.md`). `TODO.md` is the short-term
list; this file is the direction. The six colony updates are kept in `docs/colony/ROADMAP.md`.

Everything here is judged on the realistic default (`--myth` off): no fungus forests, no
magma sea, every ancient people human. Procedural, but it has to look made.

---

## Where we are (2026-10-15; all six stages done in their first form on 2026-10-16, see the module notes and `TODO.md`'s follow-ups; the table below is the starting point)

Measured on the dev world's cave-and-lair site (`--dev --headless --local-snapshot cave
--tiles-center 42,32`) and on seed 42 at 512x256.

| Layer | What exists | What is missing |
|---|---|---|
| World (`terrain.rs`, audited 2026-10-13) | Plates, isostasy, a causal pipeline, rock hardness from the crust, climate from the star, landscape evolution, fjords, rifts, volcano shapes, soils, water tables (TWI), springs and waterfalls detected per tile (`underground_water.rs`) | Fine. The world's springs, waterfalls and aquifers are computed and then never reach the region or the embark |
| Region (`region/zoom.rs`, 128 cells a world tile: 610 m at 512 wide, 3.26 km on the dev world) | Seamless chunks: bicubic world heights + fractal detail, the world's rivers carving valleys, stream-power erosion on a drainage tree, lakes and rivers by width; the history's fields, roads and towns painted over | **No lithology and no climate in the landforms**: a limestone plateau, a basalt flow, a glaciated valley and a dune sea are all the same fractal hills with rivers. No cliffs, scree, karst, moraines, dunes, floodplains, deltas, cliffs on the coast, bogs, badlands |
| Region in ink (`tiles/region_ink.rs`) | Parchment washes, hatching on far slopes, contours every 100 m, water with inked banks, tree symbols, the lore muted | It reads as a drainage map: at 42,32 you see parchment, rivers, contour threads and one road. Nothing to point at, nothing named, no hover, no legend, no relief that reads as hills |
| Embark (`local/mod.rs`, 192x192 x 2 m, 52+ levels) | The river with banks and meanders, soil over the tile's rock stack, trees and shrubs by the world biome, terraces and scree chutes on mountain tiles, wildlife signs, graves, a furnished camp site, towns in three dimensions | One ground: grass hatching, dots of trees, stepped ledges. No outcrops, boulder fields, cliff faces off the mountains, paths, clearings with reasons, hedges, reed beds, gravel bars, fords, oxbows, dunes, rock pools. The cave and the lair at 42,32 are there and invisible from the surface |
| Underground (`local/caverns.rs`, `places.rs`) | Three cavern layers (Perlin blobs in bands at 22 / 52 / 86 m), places with causes from the history (lair, tomb, old mine, halls), a hashed limestone cave, aquifers, ore seams, gems | The section shows strata and cavern **blobs**, not caves: no passages, shafts, chambers, underground streams, resurgences; a cave is a tunnel to a den; nothing joins the surface feature to the rock |
| Places and landmarks (`lore/landmarks.rs`, `focal.rs`) | The world's extremes (roof of the world, longest river, greatest falls, deepest gorge, crater lakes, giant trees), focal points planted in empty regions | Everything lives at world scale (one tile = 78 km). Hot springs, karst, geysers, tower karst, sinkholes, cenotes and cave entrances are **rolled by noise** (`biomes.rs::should_spawn`), not put where rock and heat allow them. Nothing below world scale is named or described |
| The past on the land | Ruins, battlefields, bone fields, fields and roads from the 250-450 year chronicle | **No prehistory.** The first town of year 1 stands on untouched ground. No barrows, stone circles, hill forts, cave paintings, flint mines, middens, old trackways, drowned villages |
| Tooling | `terrain_lab` (per-stage metrics at world scale), `--local-snapshot`, `--embark-seams`, `--river-survey`, `--gazetteer`, a disk cache of region chunks, tests pinned to dev sites | No survey of a region (landform mix, geomorphometry, places), no census of places over a world, no gallery of standard sites to judge by eye, no test that a thing seen in the region is in its embark |

The scale chain is world tile (78 km at 512 wide) -> region cell (610 m) -> embark (384 m of
2 m cells, 2 m levels). A region cell is about an embark. The dev world's region cells (3.26 km)
are too coarse to judge landforms on: generation work is judged on **seed 42 at 512x256**
(`--star sol --no-history`, ~4 s a world, chunks cached on disk) and on 256x128 worlds, with
the dev world kept for what needs the chronicle (lairs, tombs, battlefields).

---

## Principles

- **Caused, never rolled.** A cave is in limestone that water drained through; a hot spring is
  over young volcanism on a fault; a barrow is on the skyline above the valley its builders
  farmed; a dune sea lies downwind of a desert's bare ground. Every place and landform can say
  why it is here, in one line, and the hover shows it. Noise picks *which* of the possible
  spots, never *whether*.
- **One truth at every scale.** A feature planned once (per world tile, at region resolution,
  deterministic from world position) is drawn by the region, carved by the embark, and shared by
  neighbouring embarks. The test `views_agree` holds it: the symbol's cell and the carved mouth
  are the same place (`[[views-must-agree]]`).
- **Judged by eye, pinned by number.** Each stage ships a headless survey with numbers (landform
  shares, geomorphometric ratios, a place census) and a gallery of the standard sites. Numbers
  catch regressions; the gallery decides whether it shines. The project's blind-review practice
  (a fresh reader scores the gallery 1-10 with reasons) ends each stage.
- **Realistic by default.** Real rock, real water, real cave life, human ancestors. The myth
  world (`--myth`) may add to what the realistic world has, never replace it.
- **Deterministic and seamless.** All noise keyed on absolute position, through
  `across_date_line`; the chunk cache's `FORMAT` bumped when a chunk's recipe changes; world
  codes keep making their worlds.
- **Shareable.** A world's wonders (its deepest cave, greatest stone circle, strangest spring)
  are listed, named and reachable, so a player can send a code and say "go there"
  (`[[shareability-goal]]`).

---

## The one architectural decision: a place layer

Today the region has fields (height, drainage, rivers, lakes, climate, biome colour) and the
embark reads them; places exist only in the embark (`LocalMap::places`) and only from the
history. The roadmap adds a **site layer** between the two:

- `region::sites` (name to be chosen): for each world tile, a deterministic plan of its
  *landform classes* per region cell (cliff, scree, karst pavement, doline, moraine, dune,
  floodplain, bog, lava, beach, tidal flat, badland, ...) and its *places* (point or small
  polygon features with a kind, a name, a cause line, a description, contents, and for caves a
  passage graph in absolute metres and levels). Planned from the world's fields alone (heights,
  rock stack, hardness, stress, water table, climate, winds, glaciation, the history's lore and
  the new prehistory), cached beside the chunks.
- The region's generator reads the plan to shape its terrain (a cliff is a step, a doline a
  closed hollow, a moraine a ridge across the valley); the region ink draws its symbols and
  labels; the embark carves it at 2 m; the gazetteer and journal list what deserves a name.

This is the same "coarse data drives fine data" rule the caverns already follow
(`caverns::plan`), applied to everything.

---

## Stages

Each stage ships something you can run headlessly and judge in a gallery, and ends with exit
criteria checked by a test or a recorded survey. Sizes are rough, at the current pace.

### Stage 1: Survey and measure *(the ruler before the work)*

- `--region-survey X,Y` (seed 42 by default): the landform mix per cell class, drainage density,
  Horton's bifurcation ratio, Hack's exponent, hypsometric integral, the slope histogram and
  geomorphon shares (`docs/reference/numerical_tests.md`, `realistic_map_indicators.md`), the
  places with their causes, and the frame times. Printed as text and as a `survey_<seed>_<x>_<y>.png`
  (height, landform class, slope, places).
- `--place-census`: every place on a world by kind and cause, places per region, regions with
  none; later the test floor ("every region of seed 42 has 5+ places of 3+ kinds").
- `scripts/gallery.sh`: the standard sites (below) rendered as region 4 px, embark surface,
  one level below, the section, into one contact sheet per seed, so a change is judged on
  twelve places at once.
- Standard sites for this roadmap (seed 42 at 512x256 under the Sun): a limestone plateau, a
  young volcano, a glaciated valley past 50 degrees, a desert interior, a big river's floodplain
  and its delta, a hard-rock coast and a soft one, a bog, a rift lake, a Mediterranean hillside;
  on the dev world the pinned sites of `src/local/CLAUDE.md` (42,32 the cave, 33,32 the tomb,
  36,23 the battlefield). Found with the survey and written into `CLAUDE.md`.
- Tests: `tests/region.rs` (a region's survey numbers within bands; two regions straddling a
  tile border agree) and `tests/places.rs` (the census floors; `views_agree`). Both start as
  pins of today's numbers.

*Exit:* one command prints a region's numbers and one script renders the gallery; the roadmap's
"where we are" table is backed by them.

### Stage 2: Landforms from rock, ice, wind and water *(the land has a vocabulary)*

The region's terrain gets a lithology and the processes the world's climate implies. Each
landform is a pass over the region chunk (keyed on absolute position, blended across tile
borders like `tile_weights`), driven by the world tile's fields.

- **Rock at the surface.** From the handshake rock stack and the world's hardness: a surface
  rock per region cell with a dip (strata outcrop as benches on hillsides), so limestone, shale,
  sandstone, granite and basalt make different ground. Hard caprock over soft: mesas, slot
  canyons, hoodoos in dry climates; soft shale: badlands with rills.
- **Cliffs and scree.** Where slope and hardness pass a threshold the region cell becomes a
  rock face (a step in the height, bare), with scree fans at the foot, talus boulder fields,
  rockfall blocks. Granite tors on old worn uplands.
- **Karst** on limestone (carbonate rock with water): limestone pavements on plateaus,
  dolines (closed hollows, drained underground), dry valleys, sinking streams (a region stream
  that ends in a sink and resurges at the plateau's foot: the region's drainage routes through
  the ground), blind valleys, gorges where a cave roof fell, tower karst and cockpits in the wet
  tropics, cenotes on flat young limestone. The cave graph (Stage 5) starts here.
- **Glacial legacy** past ~40 degrees where the world's ice reached (the audit's scoured
  basins and fjords give the extent): U-valleys with hanging side valleys, cirques and tarns,
  arêtes, moraines across the valley damming lakes, drumlin fields, erratics, outwash plains,
  kettle holes; living glaciers and snowfields on the high ground that stays below freezing.
- **Volcanic ground** near the world's volcanoes (kind and age known): cones with craters,
  lava fields with pressure ridges and tumuli, cinder cones on the flanks, calderas with lakes,
  columnar basalt in old flows, ash plains; hot springs, geysers, fumaroles and mud pots on
  faults (stress) over young volcanism, **caused** instead of rolled.
- **Wind** in dry country with bare ground: dune fields shaped by the climate's winds
  (barchans, linear dunes), sand seas downwind, yardangs, desert pavement, playas with polygon
  crusts (the world's salt flats, at region scale).
- **Rivers at region scale.** Floodplains with meander belts, oxbows, levees and backswamps on
  low gradient; braided reaches below mountains; terraces (the valley's incision history);
  waterfalls at knickpoints and hard bands (the world's `detect_waterfalls` brought down to the
  region and placed on the channel), plunge pools, gorges; alluvial fans at range fronts;
  deltas (bird's-foot, arcuate by wave energy) and estuaries with tidal flats at the mouths.
- **Coasts.** Cliffs on hard rock facing the swell, with stacks, arches and blowholes; beaches,
  spits, bars, barrier islands and lagoons on soft coasts; dunes behind the beaches; mangrove
  and salt marsh in warm sheltered water; raised beaches where the land rose; a drowned forest.
- **Wet ground.** Bogs and fens where cold wet flats hold water (peat soils), patterned ground
  past the tree line, reed beds on lake margins.
- Every pass respects the audit's rules: the sphere's metric, no seam at the date line, the
  drainage re-routed after the landforms so every cell still drains (dolines drain underground).

*Exit:* the survey names 12+ landform classes on seed 42 with plausible shares (cliffs under
3% of land, karst only on carbonate rock, dunes only downwind of bare dry ground); the
geomorphometry lands in the realistic bands (bifurcation 3-5, Hack 0.5-0.6); the gallery's
plateau, volcano, glacial valley, desert and coasts are told apart at a glance.

### Stage 3: The region in ink, readable

What Stage 2 makes has to read on parchment.

- **Relief that reads:** hachures or a soft hillshade in the ink style on top of the contours,
  cliff symbols (ticks along the face), scree stipple, bare-rock washes by rock kind (the
  `rock_wash` colours), snow and ice, lava black-brown.
- **Ground symbols:** woods with edges and kinds (broadleaf, fir, palm, mangrove, dead), heath,
  bog tufts, reeds, dune lines, sand, pavement, salt crust, fields and hedges, orchards.
- **Water:** rapids and falls marked, fords, springs as a symbol with a thread of water, dry
  valleys as dashed streams, sinks and resurgences.
- **Places:** a cartographer's glyph per kind (cave mouth, standing stone, tumulus, hill fort,
  ruin, spring, falls, hot spring, arch, wreck...) with a label in the lettering hierarchy;
  dense places thinned by rank.
- **Hover and a legend:** the walker's title names the landform, rock, slope and the nearest
  place with its cause; a legend card like the overlays'; `--region-survey` writes the same
  frame.
- Frame budget stays at ~10 ms at 1280x800 (`region_ink::draw_kept` is kept).

*Exit:* a fresh reader names 8+ kinds of ground on the seed-42 gallery without the legend and
finds every place the survey lists.

### Stage 4: Places: natural wonders and the ages before *(prehistoric sites)*

The site layer fills with things worth walking to. Each has a kind, a name, a cause line, a
description of 1-3 sentences, what it holds, and rules about where it can be.

**Natural wonders** (from Stage 2's landforms): the cave mouth (resurgence, pothole, cliff
cave, sea cave, lava tube, ice cave), the waterfall and its pool, the gorge and the narrows,
the natural arch and the sea stack, the erratic and the boulder field, the hot spring, geyser
and mud pot, the salt spring the game trails converge on, the tar seep, the petrified forest in
old ash, the fossil bed in a sedimentary cliff, amber on a cold wooded coast, the meteorite
crater (rare; the star system's neighbours set the rate), the oxbow, the tarn, the tor, the
limestone pavement, the blowhole, the giant tree (kept), the crater lake (kept, now in a real
crater).

**The ages before the chronicle.** A light prehistory per world tile, run once before the
history: who lived here through the cold and the warm (hunters, then herders and farmers where
the soil allows, then builders of stone and workers of the first metal where ore is near), in
the oldest tongue of the people who later settled there. It leaves sites placed by terrain
logic, not by roll:
- rock shelters at cliff feet and painted caves in limestone (paintings only in the dark zone
  of a cave that was dry then), hearths, flint-knapping floors;
- kill sites and bone beds at bog edges and in river gravels (aurochs, giant deer, mammoth in
  worlds that were cold: the ecology's beasts and their ice-age kin);
- shell middens on sheltered coasts and estuaries;
- flint and chert mines in chalk and limestone (shafts with galleries, which may break into a
  natural cave);
- long barrows and chambered tombs on false crests above the valleys their builders farmed,
  of the local stone; round-barrow cemeteries along the ridgeways;
- stone circles, alignments, standing stones and dolmens on plateaus and moors of hard rock
  (erratics reused where ice left them); cup-and-ring rock art on outcrops over valleys;
  petroglyphs in desert shelters;
- causewayed enclosures and henges on river terraces; hill forts on spurs with steep sides
  (ramparts following the contours, a spring at the foot); crannogs and lake dwellings on
  shallow margins;
- bog hoards and bog bodies in peat; copper and tin workings in the ore belts; desert kites and
  cairn lines in arid country;
- old trackways: ridgeways along the skyline, plank causeways across bogs, fords with worn
  banks; lynchets and clearance cairns of ancient fields on hillsides;
- the last age before the chronicle: a ruined watchtower, an old quarry, a mill ruin on the
  river, a drowned village in a dammed lake.
Sites cluster the way they do in the world: a farmed valley has its barrows on the skyline,
its enclosure on the terrace, its fort on the spur, its track along the ridge. The chronicle's
own ruins, battlefields and bone fields join the same layer (they already have causes).

- **Naming and telling:** names in the old tongue with an epithet ("the Hollow Hill, where the
  first farmers laid their dead"), a description, and a "why here" line; the gazetteer and the
  journal's geography opening list a world's wonders; the world map marks the greatest (the
  deepest cave, the widest stone circle) as landmarks.
- **Rarity by design:** spacing rules per kind (one stone circle a region, a painted cave in
  one limestone cave in five), so rare things stay rare and a seed has its own.

*Exit:* `--place-census` on seed 42: every region holds 5+ places of 3+ kinds, every place has
a cause line, 60%+ of a world's prehistoric sites stand on ground that fits their rule (a
barrow on a crest, a fort on a spur); the gallery shows them in the region and in their embarks.

### Stage 5: Caves as systems

Replace blobs with caves that are built the way caves are, from the site layer's cave graph.

- **Karst systems:** planned per world tile on carbonate rock: sinks (dolines, stream sinks)
  feed conduits that follow the hydraulic gradient toward a resurgence spring at the valley
  floor or the rock contact; vadose canyons (tall, narrow, a stream in the floor) above the
  water table, phreatic tubes (round) below it, chambers at junctions and where the roof broke
  down, shafts on joints, old dry levels above the present river (the terraces' incision
  history) with speleothems. The graph is in absolute metres and levels, so neighbouring
  embarks share one cave and the region draws its mouths and its resurgence.
- **Other caves:** lava tubes down a shield's flank with skylights; sea caves and blowholes in
  hard coastal cliffs; rock shelters and overhangs in sandstone; talus caves in boulder fields;
  ice caves at a glacier's snout; mine adits and galleries (prehistoric and the history's old
  mines) that meet natural passages.
- **The deep caverns** (the DF-style layers) stay as the deep world but take shape: a chasm,
  an underground lake fed by the karst's shafts, an underground river that resurges, pillars
  and breakdown, drawn as rooms and passages rather than Perlin holes; reached by the karst
  where limestone goes deep, else only by digging.
- **Life and leavings** (realistic): bats and guano, cave crickets, blind fish and salamanders
  in the streams, cave-bear bones and claw marks in the old levels, a hearth and paintings in
  the entrance zone, a burial in a side chamber, a hermit's cell, a hoard sunk in a pool.
- **Carving at 2 m:** passages cut from the graph with the right cross-sections, walkable from
  the mouth (`nav::path3`), with the water in them; the section and the level views draw
  passages, streams and chambers; the unknown stays blank until found (kept).

*Exit:* at the limestone site a walker enters the mouth the region shows and reaches a chamber
20+ levels down and an underground stream that comes out at a spring in the same embark; two
neighbouring embarks carve the same passages; the section shows passages, not blobs; the deep
cavern reads as a cavern.

### Stage 6: The embark worth a look, and exploring it

- **Ground vocabulary at 2 m:** outcrops and benches of the surface rock, boulder fields,
  cliff faces off the mountains (every Stage 2 class carved: dunes, pavement, dolines, moraine
  ridges, lava, salt crust, beach, tidal flat), screes, paths worn between places and to water
  (the game trails already), clearings with reasons (rock, wet, fire), heath and bracken and
  meadow flowers by season, reed beds, hedges and lynchets on ancient fields, cairns.
- **The river at 2 m:** riffles and pools, gravel bars, a ford, undercut banks, oxbows and
  backswamps, levees, a floodplain that reads as one; the waterfall and its pool; the spring
  as a rill from the rock; the sink where a stream goes under.
- **The coast at 2 m:** the cliff with its ledges and the sea cave, the beach and its dunes,
  rock pools, a stack, wrack on the strand.
- **Every place carved:** the barrow is a mound with its kerb and chamber, the circle stands,
  the fort's ramparts follow the contours, the shelter has its hearth, the midden its shells;
  the site report and the hover tell what they are and why.
- **Exploring from the viewer:** places labelled in the region walker; an inspector page per
  place (name, kind, description, why it is here, what it holds, what was found); "embark here"
  on a place; `--places` lists a world's; the world map's wonders; a world code that lands on a
  place. (Viewer work only as far as it serves finding and seeing what generation makes.)
- **Views agree, always:** `views_agree` runs over the standard sites: every place the survey
  lists is drawn in the region at its cell and carved in its embark at that cell.

*Exit:* the gallery of twelve standard embarks across biomes, each with a different ground and
3+ things to point at; from the world map you can find a named cave, walk to it in the region,
embark on it and go in; a blind review scores the gallery 7+/10.

---

### Stage 7: The violent land *(the sky, the faults and the slopes in a hurry; 2026-10-17)*

The first six stages made a land worn slowly by water, ice and wind. Real land is also made in
an afternoon: a stone from the sky, a fault that slips, a hillside that lets go, a volcano that
loses its side. Stage 7 adds those, each caused by something the world already knows, and each
leaving a mark the region draws, the embark carves and the chronicle remembers
(`src/upheaval.rs`; notes in `src/region/CLAUDE.md`, `src/history/CLAUDE.md`, `src/local/CLAUDE.md`).

- **Who gets it.** Every world under another star; none under the Sun unless asked
  (`--upheaval`, `PLANET_UPHEAVAL=1`; `--no-upheaval` turns it off everywhere), so `--star sol`,
  the dev world and the tests pinned on them stay as they were, as the sky's events do.
- **Impacts.** A record of every crater the world still shows, from the star system's impact
  flux (its age, belts, giants, a companion, its metals) by the Earth's size-frequency law and
  preservation (about 600 craters of 1 km+ on an Earth at the Earth's flux), with ages and wear.
  Shapes by size and gravity: bowls, complex craters with terraced walls and central peaks,
  peak-ring basins; scalloped rims, breaches where rivers got out, sediment infill by age,
  ejecta in lobes. The largest (1.5 tiles+) on the world map before its climate and rivers, the
  rest in the region before its rivers, the small fresh ones (50 m - 1.2 km) carved by the
  embark. The planted "crater lakes" of empty plains give way to the record's real ones.
- **Faults.** The plate boundaries kept with their kind (normal, thrust, strike-slip), strike
  and a seismic zone; the region breaks the ground into tilted blocks, horsts and grabens,
  scarps, strike-slip troughs with sag ponds and pressure ridges.
- **Slopes that fail.** Landslides, rock avalanches and fallen volcano flanks on the blended
  region, before the water: debris that crosses a valley dams a lake.
- **More fire.** Maars, flood-basalt traps (provinces over plumes, by the interior's heat),
  fissure vents along young rifts, lahar deposits below snowy cones, the summit craters, lava
  that runs down the valleys on the world map.
- **Ground that gives way, floods, killing snow.** Salt sinkholes, thaw lakes, scablands
  below the old ice's dammed lakes, abandoned river channels, crevasse fields, avalanche paths,
  burned woods.
- **Dangerous places** with their causes: gas hollows, sinkholes, quicksand, crevasse fields,
  tar pits, crown holes over old workings, landslides and their lakes.
- **The chronicle.** Earthquakes on the faults (landslides on the towns in high country,
  tsunamis on thrust coasts), eruptions under the volcanoes, by hash (no RNG draws), replacing
  the quakes and eruptions drawn by lot.

*Exit:* `--upheaval-report` lists the record and where to look; on seed 42's own star the
crater region (381,79) shows a breached crater lake, the fault region (402,140) fault scarps,
27 slides and four landslide lakes, and its embarks carve them (`--views-agree`); the unit
tests hold every pass's agreement between chunks and windows; the Sun's worlds are unchanged.

## Order and dependencies

Stage 1 first (it measures everything after). Stage 2 before 3 (nothing to draw otherwise),
but 3's hover and legend can start on 2's first classes. Stage 4 needs 2's landforms for its
rules and can run beside 3. Stage 5 needs 4's cave mouths (the graph is planned with the sites)
and 2's karst. Stage 6 needs all of them; its "views agree" test is written in Stage 1 and
grows with each stage.

## Risks

- **A region is big and an embark is small.** A place planned at 610 m cells has to land in a
  384 m embark at the right spot; the plan carries metres, not cells (`CELL_FRAC` already
  quantises embarks to 1/64 of a cell for the rivers).
- **Cost.** A cold region is ~0.6 s now; landform passes and the site plan must stay under ~2 s
  cold and ~0 warm (cached with the chunks, `FORMAT` bumped). Caves are planned per tile, carved
  per embark (~0.1 s today, keep it under 0.3).
- **Mush.** Many passes can blur into noise again. The gallery and the blind review are the
  guard: every stage must make the standard sites more different from each other, not less.
- **Determinism and old codes.** All of this changes what a world code shows at region and
  embark scale; world codes keep their worlds (the world tile layer is untouched) and the
  tests re-pin per stage.
- **Myth.** The myth world's places (lairs, hollow, magma, fungus) must sit on top of the
  realistic site layer, never replace it; `tests/realism.rs` keeps the default clean.
