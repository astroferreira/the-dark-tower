# The lab (`src/lab/`, `src/bin/planet_lab.rs`; 2026-10-17)

The world, its regions, embarks, history and colonies as tools an agent (or a shell) can call. One
long-lived session holds the worlds it opened and the regions it zoomed, so a run of questions
about one place costs one generation.

## Running it
- **As an MCP server for Claude Code:** `.mcp.json` at the root registers `planet-lab`
  (`./target/release/planet_lab mcp`). Build first (`cargo build --release`); after a rebuild,
  reconnect the server (`/mcp`) so it runs the new code. Tools appear as `mcp__planet-lab__*`.
- **From the shell:** `planet_lab call TOOL JSON [TOOL JSON ...]` runs tools in order in one
  session (`world` defaults to the last opened), e.g.
  `./target/release/planet_lab call open_world '{"dev":true}' history_why '{"id":190,"direction":"both"}'`.
  `planet_lab tools` lists them with their arguments.
- Renders and exports go to `out/lab/` (`PLANET_LAB_OUT`); worlds made by the lab to
  `worlds/lab/` (`PLANET_LAB_WORLDS`). `PLANET_MYTH=1` for the myth world (one setting per
  process, as in the game); terrain revision and every other env var pass through as usual.

## How it works
- Worlds are made by the game's own binary (`planet_generator ... --headless --save-world`),
  never by a copy of `main`'s pipeline, so the lab sees exactly what the game makes. A spec's
  file (`worlds/lab/<key>.world`, e.g. `dev.world`, `s42_512x256_sol_h0.world`) is reused while
  it is newer than the binary; `fresh: true` remakes it. Seed 42 at 512x256 without history:
  3.7 s to make, ~0.1 s to load.
- A world with history is made with `--save-prehistory` too (`<key>.pre.world`): the world just
  before its history ran. A saved world carries its history's scars and no record of the
  biomes they covered, so a replay on it diverges (dev: 2,710 events against 3,109); on the
  pre-history land it is exact. Opening a `.world` by path picks up a `.pre.world` beside it.
- `Session::call` runs a tool inside `catch_unwind` (a panic answers an error) and `capture`
  (fds 1 and 2 to a temporary file), so the library's `println!`s become the call's log instead
  of corrupting the protocol; `mcp::serve` moves the protocol to a duplicate of fd 1 and points
  fd 1 at stderr. Nested captures pass through.
- Output: text or JSON (`lab::pretty`: short values on one line, so a list of places reads as a
  list), then the files written, then the log's last 40 lines (an embark render's whole log:
  it is the embark's report).
- Regions are kept per world (the last 4, ~30 MB each) by (x, y, tiles, scale, erosion);
  `places`, `region_cell` and `views_agree` on one window generate it once.

## Tools
Land (`world_tools.rs`): `open_world`, `list_worlds`, `world_summary` (star, land share, the
gazetteer by kind, landmarks, history counts), `tile` (every field of a world tile, soil,
deposits, springs, gazetteer membership, and its owner, settlement, ruins and events),
`find_sites` (`--survey-find`), `region_survey` (`--region-survey` + its PNG), `places`
(structured, filterable by kind or text, with the caves), `region_cell` (one region cell's
fields, its world tile and the nearest places with distance and cause), `views_agree`
(`local::sites::views_agree`, shared with `--views-agree`), `place_census`, `render_world`
(`--tiles-snapshot`, season and overlay), `render_embark` (`--local-snapshot`, `place` as
`PLANET_EMBARK_PLACE`, `levels` as `PLANET_LOCAL_LEVELS`).

History (`history_tools.rs`; the debugger): `history_events` (filter by years, kind, text,
place and radius, a named entity, major, caused or not), `history_event` (the event whole:
participants by name, caused by, led to, the age/war/battle it is part of, its tale),
`history_why` (the causal chain as a tree, back to the roots and/or forward to what came of it),
`history_entity` (anything by name, or type and id: its record and timeline), `history_stats`
(counts by kind and century, cause coverage and chain depth, contradictions, the war, objects
and people reports, present day, records, rare outcomes, tales), `history_export`
(`events.jsonl`, `entities.jsonl`, `edges.csv` for jq/duckdb), `history_branch`.

`history_branch {from_year, seed}`: replays the history with its own seed (world seed + 1000
unless `history_seed`) on the pre-history land to the fork, clones the state, and runs on twice:
the baseline with the original stream and the branch with the stream reseeded. Reports whether
the baseline reproduces the saved history (`replay_matches_saved`: true on the dev world), the
first differing event, both futures' counts, present day, records and first major events, and
the peoples and towns alive in one future only. The branch is kept as a world
(`dev~y300s7`) that every history tool can query. Dev world: 0.3 s for both runs.

## The history explorer (`explorer.rs`, `explorer/`; 2026-10-17)
- `history_explorer {world, dir?, legends?, compare?, open?}` (or `planet_generator
  --load-world W --history-explorer DIR`) writes a folder: `index.html` (open it from disk; no
  server, no outside libraries), `explorer.js`/`.css` (the app, from `src/lab/explorer/`),
  `data.js` (the history as `window.HISTORY`: events as arrays with participant codes `F`
  people, `S` town, `P` person, `H` house, `W` war, `B` beast, `A` artifact, `M` monument, `R`
  faith, `G` god, `C` population, `L` land; every entity; the tiles' ownership records; ages,
  tales, contradictions; the gazetteer's lands and their tile layers), `data2.js` (a second
  history for Compare, else null), `map.png` (the land without the history, under the
  territories) and the full legends in `legends/` (linked from every page). Dev world: 0.4 s,
  0.8 MB of data; seed 42 at 256x128 with 250 years: 28,497 events, 7,284 named things, 6.4 MB.
- Views (hash routes, so any view is a link): **Legends** (an index by kind and a page for
  every thing, DF's legends mode: a person with kin, house, foes and their chronicle; a people
  with towns standing and lost, rulers, wars, houses and how it regards the others; a town with
  a map, its holders and story; a war's course; a house's blood; a land (range, forest, river,
  continent) with a map, its towns, the wars fought on it and what happened there; years and
  ages; an event with its place, its chain back and what it led to); **Timeline** (a row per
  people with its life, wars as red bands, events as ticks by category; ages on top; focus on any
  thing; below: events, each people's share of the held land, or of the towns); **Causes** (the
  causal graph round an event, levels back and on, click to re-centre); **Family** (ancestors
  above, descendants and spouses below, rulers crowned, the dead faded); **Connections** (the web
  of peoples, laid out where they lived or as a ring, over a span of years: war, raids, treaties,
  trade, marriages, faith, or today's regard; a person's web of kin, foes, teachers and those
  named beside them); **Map** (territories from the tiles' records, towns by their holder that
  year, ruins, the last years' events; it plays); **Checks** (contradictions, causal cycles,
  causes recorded and places recorded by kind, causal hubs, a heatmap of which kinds cause which
  with examples, chain depths, impossible lives and towns, events tied to nothing, events per
  decade); **Compare** (with `compare`: where the chronicles part, counts side by side, each
  people's life in both, events per decade, peoples and towns in one only).
- Graph views open the clicked thing in a drawer on the right and fit themselves into the part
  of the screen it leaves free. `?theme=light|dark` on the page URL picks the theme.
- Checked: every view rendered headless in Chrome with no script errors (the app records any on
  `<body data-error>`), and the interactions driven through the DevTools protocol (re-centring
  the causal graph, a timeline mark, re-rooting a family tree, a map town and the year slider, a
  line of the web, search and Enter, a matrix cell). `tests/lab.rs::the_history_explorer_holds_together`
  makes the dev world and checks that every cause, code, holder and tile in the data resolves and
  that the app is written whole (and parses, when `node` is installed).

## Colonies (`colony.rs`, `colony_tools.rs`, `colony_explorer.rs`; 2026-10-17)
- `colony_found {world?, x?, y?, settlers?, days?, script?, every?, frame_days?}` founds a camp
  through the game's own path (`viewer::colony_site` + `found_colony`; default tile the dev
  embark, `tiles::dev_embark::pick_dev_embark`, moved out of `main.rs` for this) on a world with
  history, and keeps it in the session (`Session::colonies`) with a `Recorder`. The camp is
  lived by `ColonyRun::live`: the patron's scripted acts at their ticks (as
  `run_days_scripted`), `Colony::tick`, then the recorder, which only reads. So a recorded colony
  tells the game's story: 30 days on the dev world give `--sim-snapshot`'s colony hash
  (`a70ea86113cc941a`, the same 133 log lines), and `colony_checks {"replay":true}` runs it
  again unrecorded to compare (tested: `tests/lab.rs::a_colony_is_recorded_as_the_game_lives_it`).
- The recorder keeps what the colony forgets: every settler's job and why as spans (a new span
  whenever the job or the why changes: ~150 a settler a day), positions and flags (alive, below,
  asleep, ill, guest, away, carrying, in a fit, walking) every `every` minutes (default 10),
  creatures by id, hourly needs (hunger, fatigue, exposure, stress, focus, minutes stuck) and the
  camp's series (food, logs, stone, alive, things lying, hostiles, temperature, darkness, days of
  food, food goal, readiness, stage, works), each work's loads as they are laid (begun, finished,
  set aside), opinions each dawn, status changes, and the oddities: a settler standing where
  `nav::standable` says no one can, and jumps farther than any walk between two samples. Frames
  of the whole embark at 3 px a cell each `frame_days` at noon (`render_map`: people, creatures
  and the banner hidden for the drawing and put back). Dev colony: 30 days recorded in 0.4 s.
- Tools: `colony_run {days|hours, until}` (stop at a log line), `colony_status` (with the hash),
  `colony_settler`, `colony_log {kind: log|moments|decisions}`, `colony_why {who, at}` (the job
  and why at that minute, the choices before and after, needs, place, the store then),
  `colony_track {who, from, to}` (their walk, sample by sample: where a job's hours went),
  `colony_cell` (ground, levels, standable, building, marks, footsteps, reachable from the fire),
  `colony_path` (`nav::path3`, or how far one gets and the nearest reachable cell),
  `colony_act` (the patron, recorded), `colony_render {view: camp|map|level}`, `colony_checks`,
  `colony_branch {from_day|at, acts}` (refounded and replayed with the acts at the fork: whether
  the replay matches the original before it, the first differing line, works, fates; kept as
  `<id>~d10_0600`), `colony_explorer {compare?, history?}`.
- The colony explorer (`colony_explorer/`; open `out/lab/colony_<id>/index.html`): **Days** (an
  index of days, people, works and things made; home with the counts, the troubles and their
  history links, the great moments, regards; a day with its activity strip, moments, log and the
  camp at noon; a settler's page: past lines linked to the history explorer's events, character,
  skills, time shares, needs over time, thoughts and scars, needs, the log naming them, their last
  choices; a work's page with its loads over time); **Activity** (a row per settler, a bar per
  stretch coloured by kind of work, night shaded, moments on top, how many at each kind of work
  below; hover for the why, click for the moment: job, place, needs, the choices before and after;
  wheel and drag); **Needs** (the camp's charts and one per need with a line per settler);
  **Map** (the frames replayed with everyone's place and trail, creatures, marks, works and the
  moments of the last half day; play, follow a settler, the delve's levels as they end); **Bonds**
  (opinions on a day, fond and at odds, the wed and parents; a pair's opinion over time);
  **Works** (each work's bar with its loads laid; things made); **Checks** (the findings with
  links to the map at their minute, each settler's time, the reasons that fill the days, choices
  a day); **Compare** (`compare`: where the logs part, counts, alive/food/works over time, fates,
  the order of works, moments in one telling only). `annals.html` (the game's own annals) and the
  history explorer (written beside it, `history: true`) are linked from the bar.
- Checked: every view headless in Chrome, dark and light, no script errors; 22 interactions driven
  through the DevTools protocol (a bar's why and moment, zoom and pan, rows reordered, a settler
  clicked on the map, the slider, play, a delve level, a bond line and its chart, a work, a chart
  to its day, a day's strip, search and Enter, the settler's links into the history, a check's
  link, compare, the theme).
- Findings (dev world, 60 days): a world loaded from its file founded a different camp from the
  fresh `--dev` world (the chronicle's `last_entity` index was not rebuilt on load, so the
  roster's kin became wanderers; fixed: `Chronicle::reindex`). Diggers in a deep mine work to 85%
  hunger and then climb 32 levels for ~3.5 hours to eat: hunger 100% with 50-90 meals in the
  store, 10 times in 60 days (`colony_why`/`colony_track` on Jofulot, Day 33 17:28: "eating for
  3h 59m"). When the second hut is stamped (Day 6 12:08) a settler walking through its footprint
  is put 11 cells away (107,98) and six minutes later is back on their old path (96,87): two
  jumps no walk could make. The `--sim-*` trials with `--load-world` throw the saved history away
  (they force `--dev`, whose `history_years` drops a loaded history in `main`) and simulate a new
  one on the saved land; the lab's colonies keep the saved history.

## Findings from the first runs (2026-10-17)
- Sieges and their ends carry no location (`SiegeBegun`/`SiegeEnded` `at: null` on the dev
  world), so a town's tile does not list its sieges and `history_events` by place misses them.
- A causal link worth a look: "Raid: territorial encroachment" caused by a `TreatySigned`
  between the same two peoples (`infer_cause` takes the last dealing between the pair).
- Dev seed 10, fork at 300 with seed 7: the same four peoples survive, 8 towns stand only in the
  baseline and 9 only in the branch; wars 54 against 48.
- The explorer's checks (dev world): heirs born to parents of 2-11 years, in chains (26 lives;
  163 at 256x128); no one has more than two children, so houses are single-heir lines; sieges
  (0 of 364 at 256x128), war declarations, quests, treaties, miracles and most deaths carry no
  place; a town gets two trade routes to the same town in successive years; peoples at war with
  peoples across the ocean (the web's geography layout shows the long lines).
- The dev world's pinned sites in `src/local/CLAUDE.md` (the tomb of Zreetsio...) are the myth
  world's (the integration tests run with `--myth`); the lab's default is the realistic world.

## Not yet
- The explorer: armies and battles drawn on the map as movements; a town's population over time
  (the history keeps only today's); tales as a reading list.
- The tunables and structured causes, the Workbench window, the seed census and the visual
  diff, place kinds as data and the set-piece editor (the tool plan of 2026-10-17).
- Interventions in a branch (kill a figure, raze a town at the fork), not only a reseed.
- Colonies: a recorded colony saved to disk and reloaded (a `Colony` is not serializable; the
  session's colonies live as long as the process); the explorer's map draws the delve's levels only
  as they stand at the end; who laid which load (the recorder sees builders, not the load's work).
- An embark-level `trace` (a 2 m cell up to its region cell and world tile).
- Progress notifications for long calls (a 512x256 history takes minutes; raise
  `MCP_TOOL_TIMEOUT` if a call is cut off).
