# The first colony (`src/colony/`)

The vertical slice of ROADMAP Update 3: settlers living on a playable area with no orders.

- `Colony::found(map, names, seed)` takes ownership of a `LocalMap`, makes camp on the dry, flat,
  open ground nearest the centre (stockpile and fire), picks a flat 6x5 hut site nearby, and
  starts with two days of food. One tick = one game minute (`TICKS_PER_DAY` 1440); the clock
  starts at 06:00 on day 1. Deterministic (ChaCha8 from the seed; `det` maps).
- Needs per settler: hunger (+1 per 18 h), fatigue (+1 per 17 h awake, -1 per 7 h asleep),
  exposure (rises at night outside the hut). Starving 4 days kills.
- Choice by utility (`decide`): eat (stored food), sleep (night or tired; in the hut once built),
  forage shrubs (regrow in 12 days) and fish (beside water) while the camp has under 4 meals per
  settler, fell trees while the hut needs logs, haul loose items to the camp (more urgent for
  food when short), build (one log per 45 min, never more builders than stored logs). Each
  settler has a `taste` per kind of work (0.75-1.3) so seven don't move as a herd. A job is kept
  unless something scores 1.6x more, and never dropped for the same kind of work elsewhere (that
  made settlers swap trees forever). Foragers and fishers carry their catch home in one basket.
- Every choice is written with its reason to `decisions` and becomes the settler's `why`; the
  `log` keeps the moments (first berries, first tree, first log, the hut, hunger, deaths) and a
  dawn summary. A finished hut is stamped into the map (timber walls with a south door, floor,
  `RoofPlan`), so the ink renderer draws it.
- `nav.rs`: A* over surface columns (8-way, steps of at most one z-level, one level of water at
  a cost, no walls, no corner cutting).
- Run it: `--sim-snapshot PREFIX` (dev embark, 30 days; frames for days 1, 10, 30, the log and
  the decisions), `--sim-bench N` (N settlers, 2 days: 200 settlers ~2,900 ticks/s = ~48x where
  1x is a game hour a second), or `--dev-embark` in the window (Space pause, 1/2/3 = 1x/3x/10x,
  hover a settler for their why). `tests/colony.rs` checks 7 of 7 alive after 30 days, no stuck
  settler, the hut built, and an identical log on two runs.
- Known gaps (old): exposure had no consequence; a site without trees never got a hut (dev site
  70,6); after the hut there was nothing to do but keep food up -- all three fixed, see Projects; settlers have no pasts or names
  from the history yet (card "Settlers with pasts") -- done, see below.
- Settlers with pasts (`history/settlers.rs::roster`, via `viewer::found_colony` whenever there
  is a history): three survivors of the nearest town that fell in the last 45 years (an adult on
  the walls, a child, a youth; they follow a notable's real flight from it, `FigureMoved`),
  two veterans of their people's latest battle (under its commander; home when the war ended),
  and younger kin of living notables. Each has an age, 2-3 lines citing events, one feeling
  toward a real people or person, and a name in their people's tongue. No history RNG is used.
  Click a settler in the colony view to open the inspector on them (`inspector::settler_page`;
  their lines open the events). `--sim-snapshot` writes `<prefix>_settlers.txt` and
  `<prefix>_settler.png` and prints "Settlers: 7 with pasts, each citing at least 2 events; N
  shared"; `tests/colony.rs` requires 7, 2+ and 1+.
- The patron's verbs (`Patron`, no orders, only favour; 3 favour at most, one more each dawn,
  each verb costs one): `mark_place` blesses ground (work there is sought out, x1.4, "on the
  ground the patron blessed") or forbids it (`nearest` skips it, nobody wanders in; whys add
  "keeping off the forbidden ground"); `favour_settler` (x1.15 on work, "(the patron's
  favourite)", the others notice in the log); `send_dream` (Hut: build/fell +1.0, Plenty:
  forage/fish +0.8, Rest: sleep +1.2, for a day; whys start "Dreamt of ..."). Each use is a log
  line "(your doing)". Window: F bless and X forbid the ground under the mouse (radius 6; the
  same key on a marked place lifts the mark, free, the favour not returned), G favour the
  settler under it, R choose a dream for them (a card: 1 hut, 2 plenty, 3 rest, 4 the watch;
  Esc none; D only pans now), N name the settlement (typed, Enter keeps); marks are drawn as
  dashed rings (gold, red hatched). A key that does nothing says why (no one under the mouse,
  no favour, the hall's first log already laid). A new colony opens paused on a card (name it,
  place stones, Space to begin); Esc asks "Leave X?" (Enter leaves, Esc stays). `--sim-patron`
  tries the four verbs, lifting and a late hall stone on the dev colony (`Colony::patron_trial`);
  tested. The keys haven't been tried by hand yet.
- Founding stones and names (`Colony::place_stone`, five at most, no favour cost): a hall
  stone moves the hut site beside it while no log is laid (`find_hut_site` searches around it);
  a grove stone keeps the axe from trees within `GROVE_RADIUS` (7); a shrine is a standing stone
  half the settlers rest by. `name_colony` / `name_place` log "called X by its patron" and are
  lettered on the colony map (IM Fell). Window keys H/J/K set a hall/grove/shrine stone at the
  mouse (S is taken by panning). `--sim-founding PREFIX` runs the dev colony 100 days without
  and with stones: the hut moves (98,96 -> 116,86), the grove keeps 43 trees instead of 24; the
  pictures differ ~1.2% (the colony builds only the hut, so layouts can't differ much more until
  it builds more). `tests/colony.rs` pins both.
- Marks (`Colony::marks`, `ColonyMark`): moments leave permanent marks. When the hut is done the
  builders raise a stone by the door ("Hearthwater's hall was raised on day 2 by ..."); a death
  (`bury`, also called from starvation) leaves a grave at the camp's edge with an epitaph from
  the settler's past ("Here lies Noostond. Aged 21, kin of Sloarumth. Died of a fever on day 31.
  Remembered as one who misses Sloarumth."). Drawn in ink (mound and cross; an inked block);
  a click opens `inspector::mark_page`. `--sim-marks PREFIX` lives 30 days, buries one settler
  and renders the camp with the grave's page open; tested. Scorched ground and a ruined hut
  after a raid wait for the first arc.
- One seed, many fates: a world code `SEED.WxH.STYLE.PEOPLES.YEARS@X,Y` (printed by
  `--sim-snapshot`, shown in the colony's title; `--code CODE` opens that world embarked on that
  site, the dev preset skipped) gives the same world, site and settlers anywhere. Every patron act
  is recorded in `Colony::interventions` as "tick verb args" (bless/forbid x y r, favour i,
  dream i Hut|Plenty|Rest, stone Hall|Grove|Shrine x y, name ...); leaving the colony writes
  `interventions_<seed>.txt`, and `--interventions FILE` replays them (`run_days_scripted`;
  lines timed before the first morning run at once). `--sim-snapshot` prints "Colony hash" (FNV
  over the world chronicle and the colony log): the same code and interventions give the same
  hash (tested), different interventions a different one.
- The first arc (`arc.rs`, planned by `arc::plan` at founding whenever there is a history): the
  threat is the nearest real one (a living beast laired within 4 tiles, else the Shadow if its
  reach comes within 6 tiles, else a war band of the people who took the nearest fallen town,
  else outlaws). Day 3 10:00 a rumour (caused by the threat's last deed in the chronicle); day 6
  16:00 two refugees from the nearest real fallen town join (`add_settler`, with pasts); after
  that a settler keeps watch at the watch post (`watch_post`, the camp's north edge; a 2.5
  option in `decide`, "for fear of ...") every other night, every night if the patron blessed
  the post, never if it is forbidden ground, and a settler with `Dream::Watch` keeps it that
  night (`set_watch`). The eve of the raid logs the tally. Day 14 02:00 the raid: danger (beast
  0.8, Shadow 0.7, war band 0.6, outlaws 0.4, + 0.4 x a seeded roll) against `readiness()`
  (0.05 a night of watch, 0.15 the hut, 0.15 x the palisade's share raised, 0.02 a settler,
  +0.1 when the patron's favourite is a veteran: they captain the watch; ~0.66 unattended, 0.98
  careful) gives a death (`bury`, danger > ready + 0.3), a rescue (a stone raised for the
  rescuer) or a rout, and always scorched ground. The raid's line ends with the tally and what
  the patron did. `--sim-raid` lives it three ways (`raid_trial`: no patron, careful, careless);
  dev seeds 76/11/23/58/3/5: no patron 3 rescues and 3 deaths, careful changes all 6 (3 routs,
  3 rescues), careless loses a settler on all 6; tested. Each step is an `ArcEvent` with a "because", a banner on the colony
  map for half a day, and a log line; `tale()` writes it as a journal page (`<prefix>_tale.html`
  from `--sim-snapshot`, with the chronicle events behind it). Dev seeds 76/11/23/58/3/5: two
  routs, two rescues, two deaths. `tests/colony.rs` checks the three steps and their causes.
  The colony summary now counts the real party ("8 of 9 alive").
- The saga page (`viewer::saga_plate`, written by `--sim-snapshot` as `<prefix>_saga.png`,
  1600x1000): "The Saga of <name>" with the days, the year and the world code; the colony's map;
  the company (each settler's people's arms, name, age and calling, fate: lives / died on day N);
  a timeline of ten moments chosen by weight (the arc, deaths, the hut and the camp, the patron's
  acts, then firsts) in day order, each its first clause. Not yet: composed by itself when a
  colony falls or passes a milestone, portraits.
- Projects (`projects.rs`): after the hut, each dawn with nothing under way the colony picks its
  next work from its state and logs why: a palisade "for fear of <the arc's threat>" (24 loads,
  a ring of radius 11 with four gates, rising as loads come in), a second hut when more than 8
  live there (the refugees make 9), a woodpile "against the cold nights" (a windbreak of stone
  where there is no timber), a drying rack when 3 meals a head are stored. Where no tree is in
  reach (`timber_near`) the hut and the projects are built of stone: `Job::Quarry` breaks
  boulders (carried off) or bare rock (left as gravel) into `ItemKind::Stone`. Builders lay one
  load at a time; finished works are stamped into the map. Only materials stored, on their way
  or within reach count as on hand. Felled trees leave stumps (`Feature::Stump`).
- Exposure matters: chilled settlers (exposure > 0.7) work at half pace; exposure at `ILL_AT`
  (0.85) makes them ill for two days (they keep to their bed, rising only to eat; logged).
  Outside at night exposure rises 0.1 an hour x the fire (0.85 within 7 cells of the camp, and
  half that again while the woodpile has wood or a windbreak stands, `fire_kept`) x frailty
  (1.25 under 14 or from 55) x the night's cold (`night_cold`, hashed per night 0.75-1.2; "The
  night comes on bitter cold" over 1.1). The first hut sleeps `HUT_BEDS` (6); the rest sleep by
  the fire until the second hut. The woodpile burns 2 logs a night and is restocked (it is the
  work under way again whenever short).
- Food spoils: each dawn 1/6 of the stored food is lost (logged), 1/20 once a drying rack stands
  (not logged). The camp aims for 6 meals a head. Fished spots recover in 3 days
  (`FISH_RECOVER_DAYS`; they used to be fished out for ever).
- Forbidden ground stops work already under way there, and nothing is built on it (a forbidden
  project waits; the others go on). `--sim-projects` prints "Cold: N fell ill with a woodpile,
  M with its ground forbidden" (`Colony::cold_trial`) and the share of idle decisions. Dev seeds
  76/11/23/58/3/5: forbidden >= with on all, more on three (76: 1 vs 3); seed 23's bitter first
  night makes all seven founders ill and all live. Idle is still 23-30% of decisions: there is
  not enough to do after the projects (card 'After the raid: the next trouble, and winter').
- `--sim-projects N [--tiles-center X,Y]` runs N unattended days and lists the projects. 60 days
  on dev 45,12, 20,35, 55,15 (timber) and 70,6 (stone): 3-4 projects each, all finished;
  tested.
- Habitable ground (`dry_patches`, `habitable`): the camp goes only on dry, walkable ground
  joined in one piece of `ENOUGH_LAND` (2,500 cells, a tenth of a km2). An embark where none
  exists (dev 30,30 lies under a crater lake's water; the site report had counted the lake bed's
  soil) is refused: Enter says "too much water here to make camp", walking mode's site line
  says "under water", and the trials print it. Before, all nine starved by day 10.
- The HUD (`tiles/colony_hud.rs`, drawn after `draw_colony` in the window and written by
  `--sim-snapshot` as `<prefix>_hud.png`): top left the colony's name, clock and speed, alive
  count, three favour pips and the answer to the player's last act; bottom left the last six
  log lines, newest darkest (a click on a line naming a settler opens them); at the mouse a chip
  with the settler's name, job and why; along the bottom the keys, the patron's verbs greyed
  while no favour is left; a gold ring round the favourite and a star over each dreamer. The
  window title keeps only the world code, view and cell. Not tried in a real window yet.
- Moments (`Colony::moments`, `Moment { tick, title, text, because, at }`): each arc beat, a
  death not in the raid (hunger: "because the camp had N meals stored for M mouths"), the hut
  standing and each finished project. In the window the clock stops on one (the tick loop breaks
  mid-frame), the camera eases to its place (~600 ms) and a card (`colony_hud::draw_moment`)
  shows title, text and because until Space, which resumes at the speed before (Space also
  remembers the speed when pausing). M turns the stops off (the moment goes to the status line
  instead). `--sim-snapshot` prints "Moments: ..." and writes `<prefix>_moment.png` (the raid);
  tested. Not done: lesser events' banners still last 720 game minutes, not 4 real seconds.
- Doomed sites say so (`survey`, `Survey::verdict`): before founding, a copy of the colony counts
  berry bushes and fishing spots on the camp's own dry ground within `WORK_RADIUS` and the meals
  a day they give (bush 2.5 per 12 days, spot 2 per 3 days) against `MEALS_NEEDED` (9.3 for
  seven). Under 0.4x: refused with the reason (Enter: "no camp here: too little to eat within
  reach ... ; walk on"; the walking site line starts "NO CAMP:"; the trials print it); under
  1.2x: founded, but the status and the site line say "hard: little to eat within reach". Dev
  76: good sites give 100-220 meals a day; the mountain 48,18 and crater 28,38 give 0 (all nine
  starved by day 6 before; tested). When settlers starve with nothing in reach the log says so;
  refugees don't come to an empty camp; the last death ends it ("X was the last. The camp fell
  on day 6, to hunger.", a moment). The site report itself (`local/site.rs`) still counts the
  whole map and the furnished extras.
- Receipts agree (tested in `tests/flavour.rs` on six dev seeds): the roster's survivors and the
  arc's refugees tell the same fall (the nearest town's latest; "fallen in 438, not its first
  fall" when it fell before); kin are old enough to have seen what they cite (6 then, 70 at
  most, else the line is dropped); a beast whose lair is more days' walk away than the days to
  the raid is "N nights' hunting for it" (150 km a night); the rumour's because is the beast's
  lair or the threat's deed by title and year, never the rumour again.
- The clock you can watch (DF-like): a day is still 1,440 ticks (24 s at 1x), but walking costs
  time: `WALK_PER_TICK` 2 against a plain cell's cost 10 (a cell every 5 minutes, diagonals x1.4,
  water and trees dearer; `Settler::stride`), so a settler crosses the 192-cell map in ~16 game
  hours and is drawn between cells (`Colony::draw_pos`). Rescaled to match: a meal takes 0.8
  hunger (`MEAL`, ~1.7 a day), foraging 60 min for 3-5 berries, felling 150 min and the feller
  drags both logs home (quarrying likewise), building 60 min a load, the hut 24 loads
  (`HUT_LOGS`). After the hut: the woodpile (or a windbreak) first, then the palisade, the
  second hut, the rack. Dev 76: hut day 3, woodpile day 4, palisade day 5, second hut day 7;
  9 of 9 at day 60; idle 16%. Six seeds: huts on days 2-5. A path blocked by a new wall is
  re-planned without counting as stuck. Window: 4 or Tab skips to the next log line or dawn
  (3 days at most). Not met: a hall stone far from the trees delaying the hut by a day (the dev
  trial's stone stands near trees, both huts finish on day 3).
- Settlers save themselves: with under two meals a head stored and someone near starving, the
  camp is "going hungry": nobody fells, quarries, builds or hauls anything but food, and food is
  sought across the whole map if none is in working reach. Each morning at 07:00
  (`reckon_food`) a camp whose ground gives under 0.6x what its mouths need (`survey_at`, now
  counting only within `FORAGE_RADIUS` 25 cells: at the new walking pace farther food can't feed
  a camp; bushes 4 meals per 12 days) looks over its own dry ground on a 12-cell grid: if a place
  gives enough and twice as much, they strike camp and walk there (`move_camp`: the store
  carried, an unfinished hut and projects left, the hall stone dropped; a moment with the
  because, the old camp left as a mark; at most once in five days); if nowhere gives 0.4x, they
  give the land up and leave (`depart`, `Colony::departed`; the colony stops). `--sim-move N`
  strips the berries within 45 cells of the camp: dev 18,30, 30,38 and 6,6 all move on day 1
  and live; `PLANET_FORCE_CAMP=1 --sim-projects` camps on refused sites: 48,18 and 28,38 leave on
  day 1 with 7 of 7 alive; tested. Illness from the cold now lasts a day and a settler is
  hardened for three days after (a bitter first night had made everyone ill again and again);
  the fire's shelter is 0.8.
- The beats keep no timetable (`Arc::rumour_day`/`refugee_day`/`raid_day`, hashed per colony:
  rumour 2-5, refugees 5-9, raid 11-18, a day or two sooner when a beast is near or the Shadow
  reaches the camp). The refugees wait at the edge of the camp (stage 5, a moment with
  `choice`): in the window Y takes them in and N turns them away (`answer_refugees`, recorded as
  the intervention "refugees take|turn", logged "(your doing)"); unanswered, the camp takes them
  in at dawn. Taken in: two settlers with pasts, and the raid comes two days sooner ("they were
  followed"). Turned away: a fire of their own at the clearing's edge (a mark), a founder from
  their town "takes it hard", 7 to fight, and on the raid's night a 60% chance the raiders find
  them first (a cairn). The watch now counts as its share of the nights between the refugees
  and the raid (0.4 for every night). `--sim-refugees` plays both answers: six dev seeds give six
  timetables; with no patron 2 rescues and 4 deaths, careful changes 5 of 6, careless loses a
  settler on all 6; tested.
- Pasts act (`arc.rs`: `is_veteran`, `battle_of`, `haters_of_threat`; `mod.rs`: `opinions`,
  `reckon_company`, `friend_of`). Veterans volunteer for the watch in turn ("keeps watch tonight,
  as at the Battle of X") and each such night adds 0.02 to readiness (0.08 at most). Settlers who
  hate the threat's people ("hates" / "has not forgiven", against `Threat::faction`) go quiet at
  the rumour and one of them keeps the watch on the raid's eve ("and will not sleep: they hate
  X, who took Y"); that is their why at the post. Opinions per pair: +1 for a meal eaten at the
  same time, +4 when a refugee is known from the old town ("knows them from before the fall"),
  +6 for a rescue, -8 where one hates the other's people; each dawn the worst pair (<= -5)
  quarrels once and the closest (>= 6) is logged once as close; a close friend's kind of work
  pulls +0.2 ("..., beside X"). The roster's two veterans fought in their people's two latest
  battles. `--sim-snapshot` prints "Pasts: N watches by veterans, M by others; K lines give a
  past as the reason" (six dev seeds: veterans keep most watches, 3-6 such lines); tested.
- Readiness now (after pasts and the seeded beats): the watch's share of the nights x 0.4, the
  hut 0.15, the palisade 0.15 x raised, 0.01 a settler, +0.1 a favoured veteran captain, +0.02 a
  veteran's night (0.08 at most). Six dev seeds: no patron 0.58-0.68 (2 rescues, 4 deaths),
  careful 0.71-0.89 (changes 5 of 6), careless 0.23 (a death on all 6). The woodpile trial now
  counts chilled nights (exposure 0.5+ at dawn; illness had become too rare to compare): dev 76
  18 with the woodpile, 26 with its ground forbidden.
- After the raid (`Arc::later`, `chapter`): `plan` lines up the present day's other threads near
  the camp, one of each, none the same people or beast as the first: a war band of the enemy in
  a war the settlers' people fight, an envoy of a people with a grudge against them
  (`ThreatKind::Envoy`: asks two meals a head in tribute; paid if the store can bear it, a
  tribute stone, else refused and a war band comes in 6 days), another beast within 10 tiles,
  the Shadow within 10, outlaws last. After each raid 10-20 quiet days, then the next chapter:
  rumour, the watch, the raid 5-8 days later (no refugees; its because "as was foretold"). Each
  chapter's raid has its own roll, scorch and outcome.
- Seasons (`SEASON_DAYS` 30, from spring; `LocalMap::season_temps` from the world tile's seasonal
  climate): the season turns are logged (autumn says how many days to winter and how many days
  of food are stored), a hard winter (under 4 °C) leaves the bushes bare, nights are colder the
  colder the season (x2 at -8 °C), the woodpile burns 4 a night, and the camp does not move for
  bare bushes. `--sim-projects` prints the beats after the first raid, the longest quiet stretch
  and the season lines. 365 days on six dev seeds: 3-7 beats after the first raid, at most a
  day without a log line, but only 0-4 of 9 alive (they enter winter with ~5 days of food; the
  plan-ahead card is to store for it); tested (120 days: 3+ beats, no quiet 10 days, a winter).
- Planning ahead (`plan_projects`, rewritten): with nothing under way, each dawn the camp ranks
  its shortfalls and takes the worst, said with its numbers in the log and on the HUD
  (`Colony::plan_line`, "Next: a smokehouse. Winter is 70 days off and the store holds 4 days of
  food..."): a palisade when a raid is foretold (sooner = more urgent), beds against heads (a
  second hut), firewood (woodpile, or windbreak in stone), a smokehouse when winter is within 70
  days (food then keeps 90 days), a woodshed within 50 (the woodpile holds 20), a drying rack, a
  lookout (+0.08 readiness; first after a raid that killed), a fence round the store, and the
  palisade mended every 20 days or so (so the list never ends). With a smokehouse and winter
  within 45 days the camp stores a winter's food (`food_goal`). Settlers eat at hunger 0.6 (they
  had eaten at 0.35 and wasted half of every meal); `MEALS_NEEDED` follows `MEAL`. Six dev
  seeds: five different orders of the first five works; idle 10-19% of decisions over 120 days;
  a year ends with 4, 8, 9, 1, 7, 8 of 9 alive (raids, and seed 58 starves in early spring after
  scraping through winter); tested (a year: 10+ works, each reason with a number, no two months
  without new work). Not done: a second work beside the first when hands are free.
- Also with the plan: a woodpile under 4 logs is restocked before any other work; in a hard
  winter a hut without a fire (no stocked woodpile) lets the cold in (40% of the outdoor rate),
  so the woodpile trial now runs 120 days through a winter (dev 76: 244 chilled nights with the
  woodpile, 315 with its ground forbidden). A settler wakes to eat when hunger reaches 0.95 and
  there is food. Fishing spots are listed once at founding (`fishing_spots`,
  `nearest_fishing`) and bushes aren't sought in a hard winter: a ring search over the whole map
  for food made a year take 250 s; now 12 s.
- A colony can end, be left and be found again (`viewer.rs`: `colony_code`, `colony_site`,
  `write_sagas`; `Colony::cell`, `milestones_hit`, `sagas_written`). A camp made where the walker
  stood carries its cell in its code ("SEED.WxH.STYLE.PEOPLES.YEARS@X,Y:CX,CY", parsed by
  `parse_world_code`; `--code` sets `set_start_cell`), so `--code` rebuilds it exactly (tested:
  same hash twice, different from the tile's centre). Leaving (Esc, Enter) keeps the colony in
  memory and writes `colonies/<code>.txt` (code, day, the patron's acts); the world map marks it
  with a gold ring and its name, and Enter within 24 cells of its ground (walking) resumes it as
  it was. `--code CODE --interventions FILE --day N` opens the window on that colony replayed to
  day N (`set_resume`). Milestones (the first raid, the first winter, a year, the end: the last
  death or a departure) each write a numbered saga `sagas/saga_<code>_NNN.png` in the window;
  P writes one too (not a raw frame). `--sim-snapshot` prints "Colony code: ...; milestones: ...".
  Not tried by hand: leaving, the marker, Enter to resume, --day.
- Choosing where to settle (`viewer::three_sites`, `SiteOffer`): when the history is written the
  watcher's closing card offers three sites (scored like the dev embark, then chosen greedily so
  each differs from those before in its threat kind or its settlers' people, 6+ tiles apart,
  livable): a gazetteer name, "Seven of <people>: <callings>", the first arc's threat and why,
  and "Gives <gift>; lacks <scarcity> (made livable: ...)" (`local::site::gift_and_lack`, counted
  within 40 cells less what furnishing added; `LocalMap::furnished` records what `furnish`
  added, and the site report says so). A click or 1/2/3 sets `set_chosen_site` and the viewer
  embarks there at once; Enter still lets the player walk the map. `--sites` prints them; six
  dev seeds: three distinct peoples or threats on each; tested. Known: most dev sites lack water
  (the embark holds only the furnished spring; the river is on a neighbouring tile), so the
  lacks read alike. The watcher card is still in the bitmap font; not tried in a window.
- The raid arrives on legs (`creatures.rs`, `Colony::creatures`, `Threat::from` / `size`): at 19:00
  on the raid's eve the attackers (a named beast at its size, or a band of three raiders) enter
  84 cells out on the side their lair, the Shadow's seat or their town lies ("Something moves at
  the edge of the clearing, out of the south-east", a moment), and walk to the camp at a
  settler's pace; in the window the clock runs at a third (never above 1x) while they are on
  the map. The raid (`raid_at`) is fought where they first come within two cells of a settler
  (or reach the fire): the nearest settler is struck, the nearest to them comes to their aid,
  the scorch is left there and the raid's line says the side ("Kronix the Unending, out of the
  south-east, came in the night"); then they withdraw. A closed palisade sends them round to a
  gate (they path like anyone). Fallback: at 05:00 the raid resolves at the camp. Wolves: where
  the embark has a den, two come out at 21:00, hunt a settler alone (nobody within 4 cells) and
  over 7 cells from the fire, bite (ill a day, logged) and go home; all go home at dawn. Drawn in
  ink: a dark bulk with red eyes, named in red; raiders in dark red; wolves low and grey.
  `--sim-snapshot` prints "Raid on legs: the attackers were on the map N ticks before the clash"
  (dev 355-400) and writes `<prefix>_raidnight.png`; tested. Also: a settler who cannot reach
  their bed sleeps by the fire (seed 3: two refugees were stuck 22,000 times beside a hut they
  could not reach; their second-hut beds were cut off by a lookout raised inside the hut; fixed 2026-10-08, see the speed fixes at the end).
- Skill makes roles (`Settler::skill`, `role`, `loads_laid`; `ROLES`, `skill_of`,
  `skills_from_past`, `reckon_roles`): five skills (forage, fish, fell/quarry, haul, build),
  0..1, seeded from the past (veterans: felling and carrying; those who fought on the walls:
  building; children and the bereaved: foraging; a notable's kin: fishing; refugees: foraging
  and carrying; 40+: building) plus a hashed 0-0.2. Work takes 1.3x its time when green, 0.7x
  for a master; each job done adds 2.5% of what is left to learn (twice that beside a better
  hand: a master teaches). Each dawn the best at a trade past 0.45 is named for it ("Gaunauth
  is the camp's builder now", a moment; a gold mark on their shoulder; their inspector page);
  when a holder dies the next best "takes up the hammer". The builder's Build pulls x1.5, the
  others' x0.3 ("leaving the laying to X"). `--sim-snapshot` prints "Roles: ..." (six dev
  seeds: 4-5 roles; the builder lays 50-86% of loads once named); `--sim-roles` kills the
  builder on day 40 (dev: a load took 45 min of their work, 51 of the new builder's; the best
  woodcutter at 0.90x against 1.30x green); tested. Not done: roles still fall mostly to the
  same kind of past (the wall-fighter builds), the far thickets.
- Buildings with a job (`ProjectKind::{Storehouse, Workshop, Field, Jetty, Well}`, each a
  candidate in `plan_projects` with its worry and number): a storehouse (4x3, roofed: food keeps
  1.5x as long) when 4+ meals a head lie by the fire; a workshop (5x4, open front: felling,
  quarrying and building 0.8x the time) after 60 loads; a fenced field (8x6 posts, crop rows,
  `sow_field` / `field_season`: reaped at autumn's first dawn into meals left for the carriers,
  sown again each spring; dev 76: 48 meals) in spring or summer above 4 C; a jetty at the
  nearest fishing place (its spot never runs out); a well (a stone ring by the fire) once 10
  hours have been walked for water (`draw_water`: each dawn the walk to the nearest water and
  back is counted when it is over 8 cells; it is bookkeeping, not a settler's job yet). With the
  smokehouse and the lookout that is the card's set. `raise_building_at` raises any roofed
  building; `building_at` names a building and why it was built (window hover chip). Six dev
  seeds by day 120: 8-11 kinds standing; seed 58 (water near after its move) digs no well.
- Buildings rise in stages (`Colony::rising`, `footprint`; drawn in `draw_colony`): an unfinished
  hut or work shows by its share of loads: a pegged dashed line with corner pegs, a timber
  frame of posts from a quarter, walls rising round the ring from three fifths; the roof when
  done. The raid breaks the palisade where it came in (`break_palisade`: the ring within ~4
  cells of the clash; `Colony::breach`), and mending the breach is the first work after
  ("the raid of day 9 broke the palisade at 104,87", urgency 4), closed in stone where stone is
  in reach ("They close the breach ... in stone this time"). Not done: a hut grown into a hall
  instead of a twin, a rack becoming a smokehouse.
- A village of their own making (partly): lots follow a plan (`find_site`): within 14 cells of
  the fire, along the camp's lane (`Colony::lane`: toward the nearest water, else the map's
  centre), behind the fire costing more; blessed ground first (-30), forbidden never; a fixed
  place (woodpile, windbreak, woodshed, well) on forbidden ground takes the next lot. Feet wear
  the ground (`Colony::steps`): walkers prefer worn cells (`nav::path_worn`, up to 35% cheaper),
  and tracks (15+ steps) and lanes (120+) are inked; dev 30 days: 273 track cells, 44 lane
  cells, mostly the yard. `--sim-plan`: blessing a meadow on day 3 puts 6 of the next 10
  buildings on it; forbidding the east leaves 0 there; tested. Not done: the wall following the
  ground (rivers, cliffs), fields on the best soil, the tower on the highest cell, stages by
  heads, the survey-line key; tracks show little at 6 px.
- Each people builds its own way (`data/defaults/building_ways.json`, `projects::BuildWay`,
  `build_way`, `adopt_way`; set by `viewer::found_colony` from the first settler's people's race):
  per race, works that come sooner (x1.6 urgency) and later (x0.5), stone before timber (only
  with 40+ cells to quarry in reach, else said and dropped), and keeping the trees inside the
  wall (no felling within 13 cells of the fire). Dwarves: store, workshop, well first, stone;
  elves: field and jetty first, the wood kept; orcs and goblins: wall and lookout first;
  halflings: field and store early, wall last; humans: fields and roofs first, wall last. The
  line is logged at founding. `--sim-ways` settles the dev site as dwarves, elves, orcs and
  humans: four orders of works, elves leave 0 stumps inside the wall (others ~100), frames 10-28%
  apart; tested. Adding a people is an edit to the JSON. Not done: the dwarves' hall dug into a
  hillside (z-levels), halfling burrows, elves building against the trunks.
- A watch post the patron blessed adds 0.1 to readiness (a keener watch), so care counts where
  the watch is kept every night anyway (after the Shadow's shapes changed the dev worlds: careful
  changes 4 of 6 raids).


- Settlers go under (`dig.rs`, 2026-10-06): see the module doc. A hall (passage into the nearest
  rise within 30 cells, then a room) or a cellar (ramp down, room); digs and buildings keep
  clear of each other (`built_near`); meals are eaten in the hall's larder when nearer
  (`eat_spot`); ore plus a workshop gives iron tools (0.65x) and arms (+0.05 readiness a seam).
  Dev 50,20: hall dug by day 54, 8 sleep in it (tested). Food work is now weighed by distance
  from the fire (1.0 at the camp, 0.6 at 40 cells), hunting sits below berries and within 25
  cells, a gatherer at hunger 0.8+ eats one of what they gathered, and a settler woken by hunger
  eats before sleeping again: dev 76 stores 430 meals for its first winter and ends the year 8
  of 9 alive. A camp that half-feeds and already starves gives the land up instead of waiting.
  A failed path waits 15 minutes before the next try (`retry_at`). Game counts 1.5 meals a day a
  herd in the survey. A work whose material ran out within reach switches material or is set
  aside (`reckon_materials`); the woodpile is restocked only while wood is to be had.
  Debug: PLANET_DEBUG_PATH, PLANET_DUMP_LOG / PLANET_DUMP_DECISIONS (also with --sim-move),
  PLANET_FRAMES=PREFIX (section, below, above frames from --sim-projects).

## Dwarf Fortress systems (2026-10-07; design guide at ../dfdecomp/guide, ideas only)
- People are rolled individuals (`src/persona.rs`, `data/defaults/persona.json`): each race's
  template holds 7-breakpoint ranges (six equal-odds gaps, uniform inside) for 19 attributes,
  38 facets (4 breakpoints, thirds), 33 values (race baseline + the culture's values via
  `culture_values` + a personal roll), appearance features, hair/skin/eye palettes with RGB,
  preferences. `Persona::roll(race, culture, seed)` is pure (no shared RNG). Text comes from
  bands next to the numbers (attributes: top/bottom sixth; facets 0-9/10-24/76-90/91-100;
  values |v| >= 26), so words always match numbers; `agree` fixes their/she/he. Kin of a figure
  take after them (`settlers::persona_for`). Settlers: `persona.work_time(k)` scales work
  (strength/endurance for felling, patience for fishing...), `walk()` the stride (`stride_frac`),
  `tiring()` fatigue, `hardiness()` the cold and the illness threshold, `healing()` illness
  length, `learning()` skill gain; work trains attributes toward their caps; bravery orders the
  watch (cowards last; "(she is brave)"); perseverance keeps a job; orderly haul, dutiful build;
  a liked material (oak, granite: `Colony::material_at`) x1.2 and "; she likes oak" in the why.
  Portraits draw the persona's colours, beard, hairstyle and face shape.
- Minds (`mind.rs`): `feel(i, Feel)` keeps a thought (10 kept) and moves `stress`; weight from
  character (stress vulnerability and willpower amplify the bad, cheer the good; empathy and
  love for grief, bravery and anxiety for the raid, values for felling trees, building, idleness,
  envy of the favourite...); the same thought again weighs half per repeat still remembered.
  `reckon_minds` at 06:00: the night, hunger, needs (company for the gregarious, a shrine for the
  devout, work for the industrious), easing, mood lines when it sinks (at most every 3 days),
  breaks at stress 1.2 (5 days' rest between): tantrum (angry/violent: tears a load off the
  work under way, shouts at the nearest), despair (gloomy: lies by the fire a day), wandering
  (walks off a day); at 2.0 after two breaks, unless held by perseverance, duty, loyalty or
  family, they leave for good (`Mind::left`, counted as not alive). Each break is a moment with
  "because they <heaviest thoughts>". Miserable work 1.12x slower, high spirits 0.92x. Settler
  page: "Feels <mood>", last four thoughts, then "Who" (the persona). Six dev years: seed 58
  (hunger, raids, deaths) 8 breaks, the others 0; nobody left. Tested (`minds_break_with_reasons`,
  `settlers_are_rolled_individuals`).
- Digging too deep (`mine.rs`): with a hall or cellar dug, a workshop (or a stone-first people;
  dwarves put the mine first) and day 12+, the camp sinks a mine (`ProjectKind::Mine`, a dig
  project): a switchback ramp of 5-cell rows x 7, one level down per cell (35 levels), on the
  nearest clear 5x7 block in rings out to 30 cells, its mouth reachable from the fire. Its reason
  names the stone dug without ore. When a dug cell (or its headroom, or a neighbour's) lies in a
  cavern (`cavern_at`) the mine breaks in (`breach_cavern`): a moment with what the lamplight
  shows, `Feel::Breach` (wonder for the curious and thrill-seeking, dread for the anxious), the
  mine stops; hunting cavern life (spiders, crawlers, serpents...) climbs the stair one night in
  three (`cavelife.rs`, see "Creatures in three dimensions" at the end; "set upon by a giant cave
  spider come up from the mine"), harmless life is a line. The breach wakes the
  forgotten beast (`wake_the_deep`): a `ThreatKind::Deep` chapter at the front of `Arc::later`
  (quiet cut to 3 days), its rumour "A sound from below" from the miners with the monster's
  warning, its attacker walking up the stair from its cavern ("Something climbs out of the mine", a moment,
  `Feel::TheDeep`), strength 0.85 + its attack's deadliness. Dev 45,12: mine day 19, breach day 23
  (13 levels), the sound day 26, Gru climbs out day 30 and breathes fire on a settler; 50,20's mine
  reaches 35 levels and misses the caverns. Forbidding the mouth's ground stops it (`plan_mine`
  skips forbidden cells). Tested (`the_mine_wakes_the_deep`). `skill_makes_roles` now asks the
  builder for a third of the loads (it fell to 42% once the beast killed a builder's hand).
- Blows and wounds (`fight.rs`, DF's combat report): the raid's outcome is still readiness
  against danger; `Colony::fight` tells the clash as blows: the foe (a beast's monster strikes
  with its tusks/horns/mandibles/tentacles/talons/jaws; raiders with a spear) against the
  struck; a lunge may be dodged (agility); defenders (the saviour, the watcher, else the
  strongest) strike back with what they work with (woodcutter an axe, builder a mallet, fisher a
  spear; iron once ore is worked), hit chance from agility and the foe's size, force from
  strength x weapon / size (now the material model: "Materials and blows" at the end): misses, glances off a hard substance ("glances off the basalt of its
  flank with a ring"), bruises, cuts, bites deep; the foe may bruise the boldest defender. A
  rescue wounds the struck on a weighted body part (head, body, arms, legs), broken when the foe
  is big against their toughness. `Settler::wounds` (`Wound`: part, severity 1-3, heals in 3/8/24
  days / `Persona::healing`, from): open arm wounds slow heavy work (`wound_work`), leg wounds the
  walk (`wound_walk`), each hurts (`Feel::Wounded`); the settler page lists them. A death by a
  special attack is told once ("It froze X with its gaze; X was killed ...", "The others come
  running, too late."). Tested (`raids_are_fought_blow_by_blow`).
- Works of their hands (`craft.rs`, DF's items and engravings): with a workshop standing and 4+
  stone/logs stored, settlers given to art or craft (`craft_wish`: art-inclination,
  craftsmanship value, creativity; at most two at once) spend spare hours there (`Job::Craft`,
  180 min, by day). A work uses a stored stone (the camp's bedrock: "flint", "granite") or log
  (the land's tree: "oak"); quality 0-5 (ordinary .. masterful) from the building skill, sure
  hands, creativity, perfectionism and luck (`(q - 0.35) * 9`); a musical maker may make their
  people's instrument ("a well-crafted clay rattle called the hrasnog"), else a figurine, carved
  stone/post, bowl or plaque showing an event of their past (`Past::images`, phrased from the
  chronicle: "the siege of Brolmdustoor") or the camp's raid. `Feel::Made` by quality (a
  perfectionist minds an ordinary one), a masterwork is a moment and art-lovers `Feel::Admired`.
  Crafting does not train the building skill. `--sim-projects` prints "Works: N made, M fine or
  better, K showing a world event; best: ...". Dev 120 days: 26 made, 6 fine or better, 18
  showing an event. Tested (`crafts_show_history`).
- Balance after crafts: `Persona::work_time` is now ^-0.15 (0.85-1.18x; skill dominates as in
  DF); roles go to the quickest hand (skill x body) and pass to anyone 15% quicker ("X has become
  surer at the work than Y"); `--sim-roles` measures minutes with the body included. Breaks are
  rarer (seed 58: days 117 and 145 in 150 days); `minds_break_with_reasons` runs 150 days.
- Caravans (`trade.rs`, DF's seasonal caravans): at founding `trade::partner` picks the nearest
  living town of the first settler's people (else the nearest living town), its days' walk
  (25 km a day), whether its people hold iron or copper, and its news (what the town has heard,
  as its people tell it: "News as it is told" below). The first caravan comes on day 15 + min(walk, 30), then each
  season at 08:00: three `CreatureKind::Trader` (drawn in brown with an ochre pack, labelled)
  walk in from the town's side; at the fire (`caravan_arrives`) they buy every unsold work but
  the masterworks, worth (1 + quality)^2 x 2, for twice that in meals (48 at most) and, once,
  iron tools when the people have iron and the deal is worth 12+ (`tools_bought` makes
  `iron_worked` true); they bring one item of news; then walk home. `Feel::Caravan` (sociable and
  greedy glad, those who hate the traders' people not). `--sim-projects` prints "Trade: N
  caravans from X". Dev 120 days: 3 caravans from Brolmdustoor, 25 works sold on the first.
  Tested (`caravans_trade_and_bring_news`).
- Migrant waves (`trade.rs::migrants_arrive`, DF's migrants): `found_colony` draws the roster six
  further (the founders are unchanged) and keeps the rest in `Colony::migrants`. A caravan that
  buys works worth 8+ sends word home: 12-20 days later (10:00) one to three arrive (1 + sold
  works / 15) through `add_settler`, with their pasts and personas, "Migrants arrive from X: A
  (kin of B) and C ...", a moment; the gregarious are glad. With under 3 meals a head stored they
  look and go back. Dev 150 days: 76 grows to 13 (waves on days 62 and 124), 11 to 15.
- Festivals (`mind::festival`, DF's festivals): at 20:00 on each season's first day, with 3+
  meals a head and no raid due by tomorrow, all near the fire eat an extra meal and keep their
  people's dance and music ("At the turn of autumn the camp holds a festival: all 8 eat together
  by the fire, then dance the Thabroon and play the Bamuld."), a moment, `Feel::Festival` (the
  gregarious and merry most, the bashful less), +1 opinion every pair. Else it says why not.
- Mind balance (2026-10-07, after songs, crafts, festivals and caravans made every camp
  content): contentment floors at stress -0.8 (was -1.5) and fades toward neutral 0.06 a day;
  heavy thoughts (weight -0.15 or worse: grief, wounds, the deep) weigh again a quarter as much
  each dawn for five days; hearing a song weighs 0.035 x (0.2 + art). A year on six dev seeds:
  0-3 breaks (seed 58, four dead: 3; seed 11: 0), nobody left. `minds_break_with_reasons` uses
  seed 11 as the calm camp.
- A speaker and mandates (`society.rs`, DF's positions and nobles' mandates): at dawn, with 10+
  alive or a month on (4+), the camp chooses a speaker (adults; summed opinions of the others +
  6 x (social awareness + linguistic ability)/1000 + 4 x confidence), a moment; chosen again when
  the speaker dies or leaves. Each season's first day (and at once) the speaker proclaims a
  mandate from their dearest value that has one (`Mandate`): Works (craft wish x1.6), SpareTrees
  (no felling within 20 cells of the fire), Watch (a watcher every night of an arc), Songs (a
  performance every evening), NoIdleHands (no idle wandering by day, craft wish x1.3), Feasts
  (festivals need 2 meals a head, not 3). Settlers whose values lean to it (sum 26+) like the
  speaker more (+2), those against (-26) less (-3) and `Feel::Mandate`. After a quarrel or a
  tantrum's shouting, a speaker of some empathy makes peace (+4 opinion, `Feel::Reconciled`).
  Dev: day 30 "A month on, the camp chooses Ounaurn to speak for it."; "Ounaurn proclaims that
  the watch is to be kept every night, raid or no raid (she values law)." Tested
  (`a_speaker_proclaims_a_mandate`).
- The dead walk (`dead.rs`, DF's evil regions with this world's own evil): `found_colony` records
  the Shadow's corruption on the tile (`darkness`, `shadow_name`; none if it is broken). At
  21:00 (before wolves), with darkness 0.2+ and one night in ~1/(darkness x 0.3), a grave not on
  blessed ground gives up its dead (the named first, on even days): a wolf-like hunter named
  "X, risen" or "the restless dead", hunting a lone settler far from the fire, back in the earth
  at dawn ("Under the Shadow of Ashmaw's darkness, Nawyth rises from the grave at 102,102.";
  "Sazhaik is set upon by Geth, risen in the dark"). The first time is a moment and everyone
  `Feel::TheDeep` ("the dead walk"). Blessing the graves' ground (patron) keeps them quiet. Seed
  11 (0.48): 8 risings in 60 days. Tested (`the_dead_walk_under_the_shadow`).
- Seeing the deep: `render_section_ink` deepens to show what was dug in its row and any breached
  cavern's floor (19-60 levels); `PLANET_FRAMES` sections go through the mine's deepest cell once
  a cavern is breached (dev 45,12: the stair breaking into the dark cavern with fungus stalks).
- Strange moods (`mood.rs`, DF): once a workshop stands, from day 20 about one dawn in forty,
  once per camp, the most creative adult (creativity, art-inclination, sure hands) is taken by
  a mood (a moment) and holds the workshop by day (`mood_choice`, before eating at 0.7 hunger).
  It wants 3 stone, 3 wood laid by and their liked material (`Mood::wants`: metals need ore
  struck or iron bought, bone/horn/leather a hunt, gems/marble/obsidian ore or a breached
  cavern; the land's wood and stone are always to hand); the others fell and quarry for it
  (0.95). With all of it three dawns running: an artifact (a named altar, statue, chest, throne or
  totem in the maker's tongue, `NameGenerator::artifact_name`, of stone, wood and the wanted
  material, showing the first event of their past; quality 5, kept from the caravans, building
  skill to 0.95, everyone admires). Without it in six days: madness (stress past leaving, two
  breaks, a three-day tantrum; they may then walk away). Six dev seeds by day 200: four
  artifacts, two madnesses (tin, iron). Tested (`strange_moods_make_artifacts_or_madness`).
- Speed fixes (2026-10-07): a failed way of any job, wandering included, now waits 15 minutes
  before the next try (a mood's or a madman's wander to an unreachable spot had re-run a full A*
  every minute: seed 3, 90 days, 50 s -> 22 s); a decide with no option at all waits 30 minutes;
  under the NoIdleHands mandate a faint "looking for work" wander stays as the last resort; the
  workshop spot is by its open front. A dev year (seed 76) runs in 3.6 s.
- News as it is told (`news.rs`, over `history::knowledge`; DF's per-entity knowledge): a
  caravan brings what its town has heard (`Knowledge::news_of_town`, 30 years, 6 items, the first
  not yet heard each time) told its people's way ("They bring news: Primalspire razed by the Ashpit
  Horde (450), a massacre, Primalspire burned with its children inside (as the Kingdom of Titankeep
  tells it)."); visitors bring one item their home knows when they come (`Visitor::news`,
  `news_of_figure`: "Aephre the Wanderer brings word from the world: ..."), migrants one their
  people know (`Colony::migrant_news`, "brings word from home"), and a bard, after each night's
  work, sings one of their people's slanted deeds (`Visitor::songs`, `sung_by`, not one they told
  on arrival: "Then Tathralt sings of the failed assassination of Mefley (448), as the Git Clans
  tell it: a slander: no agent of theirs was ever there."). `hear` keeps each in `Colony::heard`
  and moves listeners by their past: +-2 their people's stake, +-1 each town of `Past::towns`,
  +-2 the stake of whom their feeling names (inverted for "hates" / "has not forgiven"): one
  `Feel::News` ("heard of ..."), lines grouped by why ("take heart / grieve at the news: it is
  their people's, or their own town's", "are glad of the news: it is ill news for those they
  hate", "... touches someone dear to them"). A listener whose people tell it otherwise "will not
  hear it told so: the Republic of Moonvale calls it a plot foiled, as the gods meant it to be" (a
  bad `Feel::News`), or adds their account when it was told plainly. The annals' "News from the
  World" lists each item, its teller and the other peoples' accounts. Dev 76, 200 days: 3
  caravan items, 7 visitors' and migrants' words, 5 songs, 11 lines of listeners moved, 2
  refusals to hear. Tested (`news_comes_as_its_teller_tells_it`).
- Under the full moon (`curse.rs`, DF's werebeasts): `found_colony` takes the nearest living
  legendary beast with Shapeshifting (its own or its species') laired within 8 tiles as `were`
  (`PLANET_FORCE_WERE=<name>` forces one; `--bestiary` marks shapeshifters). Every 28th day
  (day % 28 == 14) at 21:00 it comes as a wolf-like hunter named "<beast> under the full moon";
  it spawns 30 cells out, paths widely (30,000 nodes, round the palisade to a gate) and, bolder
  than wolves, bites whoever it reaches, roof or company notwithstanding, before it is driven off
  ("Ashiel the Ancient breaks into the camp and bites X"); its bite curses ("X's bite heals
  strangely fast", `Colony::cursed`). On later full moons the
  cursed slip out into the dark (their decide is held all night) while "<name>, changed" hunts
  from where they stood, as bold, never biting their own body ("Y is set upon in the dark by a
  beast with X's eyes"); its bite curses too and is counted ("swears the thing that bit them had
  X's eyes"); at dawn those who changed come back "scratched and silent"; one who
  has bitten twice is cast out by the speaker (or the camp), a moment, and their friends grieve.
  Dev with a forced werebeast: one bitten on day 14, two more by its changed self, cast out on
  day 71. Tested (`the_werebeast_curse_spreads`).
- Hunting and likes: a settler who loves a creature (`fond_of`: liked "deer" matches "red deer")
  hunts it at 0.35 weight ("though she is fond of deer") and `Feel::KilledLiked` if they do; one
  grazing within 30 cells of the camp at dawn is `Feel::SawLiked`.
- Quality pass (2026-10-08): a work takes 360 min and one settler crafts at a time (two under
  the Works mandate; 62 works a year had eaten the camp's stone and logs); a caravan's sale names
  distinct works; `--sim-roles` kills the camp's quickest builder (named builder first) and
  compares with the next-best hand at that moment (dev 76: 53 -> 58 min a load).
- Riches draw trouble (DF): `arc::plan` also keeps `Arc::reserve`, up to three more living beasts
  laired within 12 tiles ("word of the camp's riches has reached it") and war bands of two peoples
  who think ill of the settlers' people ("... and the camp grows rich"). With no thread left, at
  07:00 after 25 quiet days and ten more works made (an artifact counts ten) since the last draw,
  the next is drawn in as a chapter (quiet cut to 3 days). Seed 11's year: four more chapters
  after day 150 (was none). Crafters with no material laid by fell or quarry for it
  (`craft_wish_any`, 0.8 x the wish): seed 11 makes 111 works a year (was 42, none after day
  200). Tested (`riches_draw_trouble`).
- The camp's annals (`annals.rs`, DF's legends for the colony): `Colony::annals(history)` is one
  HTML page in the journal's style: the days (every moment, "Day N. Title. text (because ...)"),
  the works of their hands (best eight, artifacts first, which were sold), and the people (age,
  calling, character and values from the persona, fate: lives / died ... / cast out / walked
  away, office, role, works made, wounds, the curse). `--sim-snapshot` writes
  `<prefix>_annals.html`; `PLANET_ANNALS=FILE` writes it from `--sim-projects`. `Colony::fate(i)`.
  Tested (`the_camp_keeps_annals`).
- Their gods (`society.rs`, from the history's religions): `Past::faith` = their people's state
  religion and its first deity, named by epithet or domains ("Balorn, the god of magic"). With 8+
  alive from day 20 and 3+ devout (piety 60+) sharing a god (`devout_faith`), the camp raises a
  temple (`ProjectKind::Temple`, 4x4 roofed, "4 of them are devout and pray to Balorn, the god of
  magic, with no roof but the sky"). Then the devout (piety 50+) pray there from 18:00 to 21:00
  (`Colony::temple`; meets the prayer need, which also counts the temple as a shrine) and
  festivals are kept "in honour of" the god. The settler page says whom they worship. Dev seeds
  by day 150: temples on four of six. Tested (`a_temple_to_their_god`).
- Crime and justice (`justice.rs`, DF): at 23:00 a settler whose greed or immoderation passes 85
  (+10 when hungry past 0.8) may steal meals (urge in 600, by hash; 2-4 meals); someone awake
  within 6 cells sees, else the sharpest intuition (1200+) guesses, else "no one saw who took
  them". A tantrum's wrecking is a crime too. At dawn the speaker judges: law value 26+, or a
  third offence unless they despise the law, puts the culprit in the stocks for a day (held by
  the fire, `stocks_choice`, `Feel::Punished`); otherwise give back the meals or mend what broke;
  law -26 or less lets it go and the law-minded resent the speaker; the speaker's own crimes go
  unjudged. The first judgement is a moment. Six dev seeds: 2-15 thefts in 200 days. Tested
  (`thieves_are_judged`).
- The HUD's standing lines (`Colony::standing`, under the status in the top-left card, four at
  most, the first in rubric when it is a mood, the moon or a fit): a mood and what it wants, the
  full moon (or "in N days" with a werebeast near), who is in the stocks, who is miserable or
  lost to a fit, the speaker and their mandate (`Mandate::short`), the temple, the relic, "the
  dead do not rest here". The settler chip adds their office and mood ("idle - unhappy").
- A lost thing of the world (`relic.rs`, DF's artifacts as story carriers): at founding
  `relic::lost_near` takes the nearest artifact the history lost (lost, not destroyed) within 6
  tiles: placed by the ArtifactLost event's place, else its last holder's last located event,
  else where it was made. It lies 35-70 cells from the fire, or (half the time where there are
  caverns) in the dark below, found only when the mine breaks in (`relic_below`). Settlers of its
  owners' people tell its tale at founding; else the first caravan does (`relic_told`). The
  curious and greedy who know it search by day (`relic_option`, 0.2-0.55; a hashed spot within
  16 cells of it, narrowing a cell every two days of searching); anyone within 2 cells finds it
  (a moment; kept in the temple or the hut). If its owners' people stand, 12-20 days later their
  envoy asks for it back: the speaker (else the oldest) gives it by fairness, law, tradition and
  altruism against greed (+40 if of that people): a gift back (iron tools once, else 2 meals a
  head); else a war band comes for it as the next chapter, and unless routed carries it off
  (`relic_after_raid`). In the annals ("A Thing of the Old World") and the HUD. Dev 76: the
  traders tell of the Staff of Greenburg on day 45, Austourd finds it day 51, the Git Clans'
  envoy day 67, Ounaurn covets it, the war band is routed day 76. Other dev seeds have no lost
  artifact within 6 tiles. Tested (`a_lost_relic_is_found_and_claimed`).
- Slaying beasts (`fight.rs::maybe_slay`, DF's megabeast kills): the fight now counts its blows
  (`Colony::blows`: harm and the hardest striker). In a rout the whole camp turns out (a third
  defender) and every able adult not shown crowds in for half an expected blow ("The rest of the
  camp crowds in with axes, stakes and torches."). A beast or deep threat whose harm reaches its
  size (x1.3 when it wounded someone, the rescue outcome; never on a death) falls: "X drives a
  fishing spear into its heart: Bornith ... falls, and does not rise", a moment, "The bones of X"
  stone, the striker's `Settler::deeds` ("slew X on day N", settler page "Deeds", the annals),
  `Feel::Slew` / `Feel::SawFall`, `Colony::slain`; a slain werebeast howls no more (the bitten
  stay cursed); later chapters with the same beast are dropped. Its hoard (`Monster::hoard`,
  from the history's `artifacts_owned`; `Monster::kills`) is fetched when the lair is within 2
  tiles (`hoard_due`, back in 2 + 2 x tiles days, `Colony::treasures`), else named as too far.
  Iron arms (1.3-1.5 vs 0.9-1.2) and more hands make it likelier; size 3+ beasts are out of
  reach. Six dev years: one slain (seed 11, Bornith, day 52; 2.9 harm against size 2.1); seed
  76's Baelfang (2.2) took 1.9. A found relic and the treasures count 10 each toward riches.
  Tested (`a_beast_can_be_slain`). No hoard has been fetched on the dev seeds yet.
- The militia (`militia.rs`, DF's squads): while trouble is foretold (`trouble_foretold`: arc
  stage 1, 2 or 5, from the rumour to the raid) the best builder makes spears at the workshop
  (`Job::Craft` with "Making spears", 1.1; a stored log each; "<the camp's stone>-tipped" 1.15 or
  "iron-headed" 1.45, x0.9-1.1 by hand) until each of the militia (adults with bravery 30+ or
  martial value, best fighters first, eight at most) has one; at dawn `arm_militia` hands them
  out. From 17:00 to 19:00 the militia drills at the drill ground (7 cells west of the fire)
  under its best fighter (`drill_option`, 0.35-0.9); at 19:00 those there gain 0.05 x
  `learning()` of `Settler::drill` (0.6 at most). `fight_skill` = 0.4 for a veteran (else 0.05)
  + drill: +0.25 x skill to hit, x(1 + 0.5 x skill) force. Readiness +0.015 an armed and drilled
  hand (0.06 at most); the tally says "N of them drilled and under arms". Settler page:
  "Drilled with the spear (green / steady / a seasoned hand)".
- Beasts and the militia, after balancing: the clash now counts the whole camp's blows on every
  outcome (a beast that killed can still fall, at 1.6x its size; a rescue 1.3x; a rout 1x). Six
  dev years: three beasts slain (seed 11 Fyron Plague-Bearer on day 16 by a drilled stake; seed
  3 Kaelgar and Zarnak, both by Grang's sandstone-tipped spear); every beast raid still costs a
  life or a wound. `a_beast_can_be_slain` (seed 11) and `the_militia_drills_and_arms` (dev, 45
  days) test it. The drills moved the dev timelines: Gru climbs out of the mine on day 36
  (`the_mine_wakes_the_deep` runs 45 days), `thieves_are_judged` accepts any judgement, and
  `crafts_show_history` allows two thirds fine (the best builder carves too).
- Visitors (`visitors.rs`, DF's visitors and petitions): `visitors::plan` at founding (viewer)
  takes real living figures. A hunter is the latest living hero whose QuestBegun ("X hunts Y")
  names one of the arc's beasts (first threat, later, reserve). Up to three bards are art-inclined
  (75+, `Persona::of_figure`) figures without titles whose people hold a town within 10 tiles and
  are not at war with the settlers'; "a loremaster" with memory 1200+, else "a teller of tales".
  15:00 (`visitors_arrive`): a hunter comes while their beast is foretold, a bard from day 40,
  one every 45+ days (1 in 4 dawns). They come in through `add_settler` with a `Past` built from
  the figure (persona, arts, faith) and `Settler::guest_until` (hunters a day after the raid,
  bards 4 nights) and `visitor` (the calling); a hunter's `drill` is 0.2 + 0.05 x combat. Guests
  hold no role, office or mood and are never chosen to speak; hunters stand first in the fight
  (`fight.rs`). 20:30 (`guests_perform`): a bard performs a work of their people with its
  description; from the second night, once, they teach one to the most art-inclined listener (35+) who lacks
  it, pushed into `Past::arts` as "taught by X", which `evening_arts` performs "as X taught
  them". 09:00 (`guests_leave`): a hunter whose beast lives follows its trail; a guest the camp
  likes (mean opinion 3+) who is of the same people or gregarious, with 3 meals a head stored,
  asks to stay and the speaker agrees (a moment); the rest walk on (`mind.left`; "moved on" in
  the annals; `Colony::company` leaves them out of "N of M alive"). Six dev seeds by day 200:
  four hunters (seed 11's Ielnveph the Brave kills Fyron Plague-Bearer, the beast of their
  quest; seed 3's Yelfsia the Seeker kills two), bards on five seeds, works taught and
  performed. Tested (`visitors_come_from_the_world`). The refugee test now compares "N to fight"
  numerically (a hunter makes it 10 and 8); the militia test runs 65 days.
- The relic's seeker (`relic.rs::seeker_asks` / `seeker_steals`, `VisitKind::Seeker`): `plan`
  also takes the relic's artifact and last holder: the latest living hero whose QuestBegun
  names it ("X seeks Y"), else the holder's eldest living child or spouse ("an heir seeking what
  was lost"). The seeker comes while the tale is known and the relic is unclaimed (3 days after
  it is found, or from day 25 while it lies lost; then they search at full weight). They stay up
  to 20 days. At 10:00 after it is found they ask: the speaker (else the oldest) gives it by
  fairness, tradition, altruism against greed, opinion of the seeker x2, +30 if of one people,
  -15 when its owners' people stand. Given: a moment, the seeker leaves with it the next morning
  (a deed). Refused: a greedy (60+), cunning or lawless seeker tries that night at 23:00: seen by
  someone awake within 6 cells, caught (a crime the speaker judges); unseen, gone with it (a
  moment; the camp resents it). An envoy who comes for a relic already gone says so and leaves.
  No dev seed has a living seeker or heir (the dev world's five lost artifacts all lie at one
  capital, with no quester left alive): `PLANET_FORCE_SEEKER=<name>` makes one (greedy, of the
  owners' people). Dev 76 forced: Orvel comes day 45, the staff is found day 53, Gaunauth refuses
  and Orvel slips away with it that night. Tested (`a_seeker_comes_for_the_relic`).
- Engravings (`engrave.rs`, DF): with a dug hall, one settler at a time who wants to make things
  (`craft_wish_any` 0.3+) engraves a bare wall by daylight (`Job::Craft` with "Engraving", no
  material; walls are neighbours of `hall_cells` with rock at the floor's headroom, z+1). The
  image is the camp's greatest moment not yet carved (`worth`: a death 10, an artifact 9, a
  relic found or a hoard 8, a raid 7, a cavern breached 6, a guest come or stayed 5, a madness or
  casting out 4), phrased "the death of X (day 44)", "the raid of day 49", "the coming of X",
  "X taking a place at the fire"; then a scene of the engraver's past (`Past::images`).
  Quality as for works; 4+ is admired. Drawn as a small framed panel on the wall face toward the
  floor (`draw_colony`), named on hover (`engraving_at`, via `building_at`), "The Walls of the
  Hall" in the annals. Dev 50,20 (hall day 44): Raagnux carves 10 walls from day 119, the camp's
  deaths, raids and guests first, then the fall of Titanroot. Tested
  (`the_hall_is_engraved_with_its_story`).
- A vampire among the migrants (`night.rs`, DF's night creatures): with the Shadow's darkness
  0.2+ on the camp, one migrant wave in three brings one (the wave's last; `Colony::vampire` =
  (who, day); `PLANET_FORCE_VAMPIRE=1` forces one in every wave). Their hunger is held at 0.2
  (they never eat) and they never steal food. From five days after arriving, one night in four
  at 02:00 they feed on a sleeper (one already fed on first): ill two days and a dread; a second
  feeding leaves them dead at dawn ("found dead at dawn, pale as ash, with two small wounds at the
  throat", a moment, everyone dreads it). Someone awake within 5 cells sees it half the time, and
  a victim wakes one time in five: a crime "drank the blood of X". Seven days on, the sharpest
  mind (intuition 1300+) notices "X is never seen to eat" (the hint), and after a feeding names
  them (a crime "is a thing of the night"). The speaker (`justice.rs`): put to death at dawn by
  the fire (law > -26) or cast out "into the light"; no speaker: driven out with torches. Only
  those crimes; a vampire's other crimes are judged as anyone's. `drained` and the noticing
  reset for each new one. Forced on dev seeds 76/11/3: caught after 0-2 feedings, one death
  each; unforced, seed 58 gets one on day 245. Tested (`a_vampire_comes_with_the_migrants`).
- Speed and the sealed hut (2026-10-08): seed 3's year took 48 s. A lookout went up at the fixed
  watch post, which lay inside the second hut, and walled the beds off behind its door. A
  despairing settler, never tired, then re-chose "sleep" every tick, searching the whole map for
  the bed each time (3.5 ms). Now: `watch_post` is open ground off any building or roof; the
  lookout goes there only when its 2x2 is clear, else `find_site`; a walled building whose inside
  can't be reached from the fire opens another side's middle (`raise_building_at`); a bed or
  watch post that can't be reached is remembered for a day (`bed_blocked_until`,
  `watch_blocked_until`); despair keeps them abed; a step blocked mid-path waits 15 minutes and
  a blocked final cell is worked from beside it. Dev years: seed 3 4.9 s, 76 4.4 s, 58 4.0 s, 11
  10.7 s. Seed 58 now starves through its first winter and its tantrums spiral (13-15 breaks in
  150 days; `minds_break_with_reasons` allows 16).
- Kinds of mood (`mood.rs::MoodKind`, DF's fey/secretive/possessed/macabre/fell): chosen from the
  moody settler's character: fell (cruelty 75+ and violence 60+, 5+ alive) on the first dawn drags
  the nearest settler into the workshop and kills them (a moment, everyone grieves, opinions of
  the maker -12), then makes "a chest of the bones of X, showing the death of X" with no loads;
  macabre (gloom 70+) wants bone (from a hunt or the camp's graves); possessed (imagination 80+
  and piety 60+, or a third of camps under darkness 0.3+) wants nothing and gains no skill;
  secretive (bashful 70+ or trust 20-) will not say its material ("draws shapes in the dust");
  else fey, as before. `PLANET_FORCE_MOOD=<kind>` forces one. Six dev seeds by day 220: one
  secretive, two possessed, two macabre, all artifacts. Tested (`moods_come_in_kinds`).
- Pets (`pets.rs`, DF): at dawn a settler fond of a herd (`fond_of`) grazing within 30 cells may
  tame a young one (1 dawn in 8, one pet each, no guests): "coaxes a young caribou to the fire
  with salt and bread, and names it Tha" (a personal name in their tongue). It becomes
  `CreatureKind::Pet` (not hunted; drawn smaller, paler, with a red collar), follows its keeper
  (`pets_follow`, every 15 minutes when 4+ cells behind), comforts them at dawn (`Feel::Pet`).
  Wolves with no lone settler to hunt stalk a pet strayed 8+ cells from the fire and take it
  (the keeper grieves as for a close friend). A keeper dead or gone: it goes back to the herd.
  `Colony::pet_of` in the annals. Six dev seeds by day 200: four pets (one timeline of seed 58 lost
  its caribou to wolves on day 140; chance). Tested (`settlers_keep_pets`: a pet, in the annals).
- Healing (`heal.rs`, DF's health care): `Wound` gains `tended` (the day; fresh wounds count as
  clean on their first day), `infected`, `fever_since`. At dawn (`reckon_wounds`), with anyone
  wounded the camp names a healer if it has none living (the most empathic and focused adult;
  office "Tends the wounded"; a moment the first time). A gashed or broken wound untended since
  yesterday festers 8% a day per severity (fever: ill a day at a time); four days of fever
  untended, one in three dies "of the fever from his gashed head that no one tended" (a moment).
  By day the healer goes to the nearest wounded settler not tended today, festering first
  (`tend_option`, 1.1, 1.6 festering; `Job::Wander` "Tending X's ..."), and at the bedside
  (`tend`) binds every untended wound: heals 20-35% sooner by empathy and focus, a festering
  wound cleaned, the patient grateful (+2, `Feel::SavedBy`). Dev 76: Austourd is named the morning
  after the first raid and binds the wounded; seed 3's cracked skull festers on day 137 and
  Cufibed cleans it the same morning. Tested
  (`the_wounded_are_tended`).
- Families (`family.rs`, DF's relationships): from day 30, two unwed adults (16+, one race, one
  of each sex, not guests) with opinion 20+ both ways who don't despise romance may wed at dawn
  (1 in 30): "X and Y are wed by the fire" (a moment; everyone `Feel::Festival`). A wedded
  couple with the wife 45 or younger, no child of hers under 120 days and 3 meals a head stored
  conceives 1 dawn in 40 ("is with child"); 60 days later the birth (a moment): a settler of age
  0 named in the mother's tongue, persona rolled for the race then each facet from one parent
  (+-10), hair, eyes and skin from one, a past "born in <camp> on day N to A and B", the
  mother's people, arts and faith; the parents like the child +12. `Settler::spouse`,
  `Colony::expecting`/`born`/`children`. Infants (all born in the camp) only eat, sleep and are
  carried by their mother (`infant_choice`); children under 12 keep no watch and steal nothing.
  `family_of` in the annals. Six dev seeds over 365 days (three game years): 2-8 weddings, 2-5
  births. Tested (`families_are_made_in_the_camp`).
- Aquifers (`LocalMap::aquifer` / `is_aquifer`, `dig.rs::strike_aquifer`, DF): where the world
  tile's water table is 0.45+, the levels 4-8 under the median ground are wet wherever the
  rock lets water through (sandstone, limestone, loose sediment, sand, gravel, soil). A dig
  that would open such a cell stops instead: "X's pick opens wet rock in a mine: water wells up"
  (a moment "Water in the rock"); the camp no longer walks for water (`draw_water`); the dig is
  paused (`dig_paused`) and `ProjectKind::Lining` (12 stone loads for a stone-first people, else
  20) is put ahead of it; when the lining stands, `aquifer_lined` and the dig goes on. The
  `--sim-projects` Dug line reports the aquifer. Dev 45,12: wet rock day 26, lined day 27, the
  first cavern day 31, Gru climbs out day 38. Tested (`the_mine_strikes_an_aquifer`).
- `--sim-roles` prints minutes to a tenth: with families, guests and migrants the dev camp has
  twenty by day 40 and a second near-master builder (53.7 vs 53.8 min a load), so the loss of the
  best hand barely shows; `skill_makes_roles` parses tenths.
- Gems (`local::gem_in`, DF's clusters): one rock cell in sixty holds a cluster, by a hash of its
  world place and by its rock (granite: rock crystal or garnets; basalt: obsidian or agate;
  limestone: marble; shale: jet; sandstone and sediment: amber). Digging it (`finish_dig`)
  prises out 1-3 (`Colony::gems`; the first a moment; those who like that stone are glad). A
  work at the workshop is set with one when the maker is a perfectionist, loves that stone, or
  by chance in three: "a superior flint bowl set with amber" (worth 6 more to a caravan; as in
  DF a decoration adds worth, not quality). Moods that want a gem
  need it in the store now (or a breached cavern). Dev 45,12: amber on day 12, set by day 38 and
  sold. Tested (`gems_are_found_and_set`).
- Legends of the camps (`legend.rs`, DF's legends): leaving a colony in the window (Esc, Enter)
  also keeps its legend (`Colony::legend`: name, tile, day, fate, up to 16 great moments, beasts
  slain, the relic and its fate) in `colonies/legends_<seed>.json` (`legend::save`, replacing
  an earlier telling of the same code). The window loads them with the world: camps of earlier
  sessions get a dashed gold ring and name on the world map, the tile's hover adds "<name>
  (<fate>, day N): <last deed>", and a fresh camp (not a `--day` replay; headless runs never)
  heeds them (`heed_legends`): beasts slain are dropped from its arc (a slain first threat is
  replaced by the next), a slain werebeast no longer howls, a relic taken by an earlier camp is
  gone, and the nearest earlier camp's tale is its first log line ("They know the songs of the
  camp at 80,17, 2 tiles away: ... Bornith the forgotten beast falls, and does not rise.").
  `--sim-legend` (dev center) lives one camp 60 days and founds a second two tiles east that
  heeds its legend through JSON; seed 11: Fyron Plague-Bearer gone from the second camp's
  troubles. Tested (`legends_outlive_the_camp`). Not tried in a real window.
- Ghosts and memorials (`ghosts.rs`, DF): `bury` calls `mourn`: a violent death (a raid, a
  beast, the night, a fell mood; not an execution) leaves the dead restless. A slab (stone) or
  post (wood) carved in their memory lays them to rest (`slab_option`: their closest friend at
  1.2, fetching stone or wood when none is stored; others by their craft wish; `Job::Craft`
  "Carving a slab ..."; set by the grave as a mark "In memory of X, who died ..."; friends
  reconciled). Five days unremembered, at 23:30 the ghost walks (two nights in three, one in four
  after five): seen by the one who was closest ("The ghost of X is seen by the well in the night;
  Y cannot sleep after", the first a moment), `Feel::TheDeep`, +0.3 fatigue; the anxious hear of
  it. Most camps carve one within days (seed 11: Nawyth's post for Shasiesh two days after the
  raid; no ghost). Seed 23's ghost walks from day 34 to 66; starving seed 58's from 16 to 66. Tested
  (`the_dead_who_died_badly_walk_until_remembered`).
- Expeditions (`expedition.rs`, DF's raids and missions): from day 30, one dawn in ten, with 3+
  drilled hands under arms and a speaker who values martial prowess or is brave (60+), a beast
  laired within 10 tiles (the one foretold and not yet come, or one in the later or reserve
  troubles) may be hunted: 3-5 of the best fighters (a visiting monster hunter first; never the
  speaker, the healer, a moody maker, a child, the wounded or ill) leave (`Settler::away_until`,
  `alive` false meanwhile; "away hunting" in the annals; "N away hunting X" on the HUD). To a
  lair at most 15 days' walk off (25 km a day; `Colony::world_width`) and back plus two days,
  or out to meet the foretold beast on the road, back within three days and before its raid.
  Their blows are reckoned as they leave (strength, spear, fighting skill, against its size);
  1.1x its size slays it: "comes home with the head of X: Y struck the blow that killed it" (a
  moment, a deed, the beast dropped from the troubles, the foretold raid stood down, its hoard
  home). Each may be wounded or fall at the lair (more likely the bigger it is against their
  blows). A beast killed at the camp meanwhile: they come home to the news. Six dev seeds by day
  300: only seed 3 hunts (Kaelgar on day 48, Zarnak on day 98, both slain on the road). Tested
  (`the_militia_goes_out_to_meet_the_beast`).
- Berry search (2026-10-08): `Colony::shrubs` lists the map's shrubs at founding;
  `any_ripe_shrub` (cached per day) skips the search when none is ripe, and
  `nearest_ripe_shrub` searches rings out to 12 cells then the list beyond (same order as the
  ring search, so the same choices): a hungry camp's whole-map ring search had cost seed 11 most
  of its year (10.7 s -> 8.3 s).
- War band leaders (`arc::plan`'s `band`): a war band of the history's peoples is "a war band of
  X, led by Y", Y the living figure of X with the most kills (none: no leader). In a rout, with
  the camp's blows at 1.4+ and one time in three, the hardest striker cuts the leader down ("...
  who led them; the band ... carries the body off into the dark"; "and who once sat at this
  camp's fire" when the leader had been a guest), a moment, a deed, and the name joins
  `Colony::slain` (so the camp's legend carries it). Six dev seeds by day 250: 3 leaders in 16
  routs; seed 76's Moonvale band is led by Aephre the Wanderer, the monster hunter who had stayed
  at the camp. Tested (`war_bands_are_led_by_real_warriors`).
- The saga plate's timeline (`viewer::saga_plate`, 2026-10-08) is drawn from `Colony::moments`
  (and the founding line), ten chosen by weight: deaths, raids, killings and castings-out first;
  artifacts, hunts, relics, breaches, madness and births next; weddings, guests, migrants, ghosts
  and the aquifer next; then the rest. Labels stay inside the page at the right edge.
- Drink (`drink.rs`, DF): with a workshop and 3 meals a head stored the camp raises a still
  (`ProjectKind::Still`, 2x2 posts, 6 loads; urgency 2.2 when half the camp are dwarves, else
  0.9). One hand at a time who likes a drink (immoderation 50+, or a dwarf) brews by day while
  there are under 2 cups a head and 4+ meals a head (`Job::Craft` "Brewing ..." at the still):
  3 stored meals make 5 cups (`Colony::drink`). Eating takes a cup once a day (`Feel::Drank`);
  a dwarf without a cup for three days (from day 30 or once a still stands) is thirsty at dawn
  (`Feel::Thirsty`) and works 1.08x slower (`thirst_pace`). A festival with a cup a head opens
  the wine. Dev: stills on days 16 (seed 3, dwarves), 23 and 34. Tested (`the_still_makes_wine`).
- Tests of chance events (ghosts, band leaders) try several dev seeds through `run_log` and
  pass on the first that shows the event; timelines move with every system added.
- Cage traps (`traps.rs`, DF): with the palisade done and a beast foretold (or a beast among the
  later troubles), `ProjectKind::Traps` (urgency 2.0, 8 loads, "a cage on a trip-stone at each of
  the 4 gates") sets `MarkKind::Cage` marks at the gates (`gates`: camp +-11 on the axes) and the
  mine's mouth. On a beast's (or the deep's) raid night, size 2.6 or less, `trap_takes` cages it
  before anyone is hurt: two in three when the clash is within 4 cells of a trap, one in three
  otherwise ("blunders onto the trip-stone at the mouth of the mine: the cage drops and the bars
  hold", a moment; `Colony::caged`, a treasure; dropped from the troubles; everyone admires it;
  the raid's line "was taken in a cage trap"; the mark becomes "The cage of X", drawn barred and
  dark within). Dev: seeds 11 and 23 cage their forgotten beasts at the mine's mouth (days 42 and
  29). Tested (`cage_traps_take_a_beast`). Seekers stay until they have asked for a found relic.
- Years (`ageing.rs`, DF's ages and death year rolled at birth): the camp's year is four
  seasons (`YEAR_DAYS` 120). At the dawn a year turns (day 121, 241, ...) every living settler's
  `Past::age` grows by one and the line "A year turns in the camp: its second year. N live here;
  X is on their own feet now" names children who reach four (out of `infant_choice`). Each
  settler's span (`Colony::span`) is their race's lifespan (human 72, dwarf 140, elf 450, orc 55,
  goblin 60, halfling 95, giant 200, fey/undead/elemental/construct 1000) x 0.8-1.2 by a hash of
  their name; reaching it, they die in their sleep at the turn ("of old age", a grave, no ghost;
  a moment for the camp's eldest). Seed 5 over ten years: three generations of children walk,
  no founder reached their span. Tested (`the_years_turn`; lib `spans_follow_the_races`).
- A lord from home (`nobles.rs`, DF's nobles): at founding `plan_lord` notes the first settler's
  people's ruler (`current_leader`) and their eldest living child or spouse, else sibling (not one
  who leads an outlaw band). From day 90, with 14 grown settlers (not guests or infants), the kin
  arrives: "X, kin of Y who rules Z, arrives with a writ: she is to be lord of the camp, and W
  speaks for it no longer" (a moment; `Colony::speaker` and office "Lord of the camp"; the lord's
  values make the season's mandates). Tradition, law and loyalty welcome them (+3); independence
  resents them, and so does the old speaker (-4, `Feel::Mandate`). They demand a hall
  (`ProjectKind::LordsHall`, 5x4 roofed, 20 loads, put first); unbuilt after 20 days, every 10
  days "X is displeased" and opinions sour. Seeds 23 and 11: lords on days 124 and 155. Tested
  (`a_lord_comes_to_rule`). Value names now read as words ("martial prowess").
- Quality pass (2026-10-08, reading whole logs): the winter store now counts what spoils
  (`keeps_days`, `days_of_food` = K ln(1 + S/cK); `food_goal` for a winter = cK(e^(W/K) - 1) x
  1.05); seed 3 had entered winter told "33 days of food" that lasted 24. "The camp has no food
  left" is said once a day at most (`food_warned_day`); a beast from the deep is caught by the
  trap at the mine's mouth (not "at the gate"), and the trap reason counts the 4 gates; settlers
  taught a work perform it one night in three (+8 to be chosen), not every other night. Still
  open: seed 58 starves through its winters (5 of 11 alive at a year); empty days in late winter
  on seed 76.
- Farms under the rock (`cavefarm.rs`, DF's underground farming): with a hall or a finished
  cellar, a stone-first people (dwarves) or any camp whose mine breached a cavern (spores) plants
  `ProjectKind::CaveFarm` (8 loads, urgency 2.2 with winter within 60 days, else 1.2). Every
  eighth day after it was begun, at 07:00, whatever the season, it yields 12 meals (20 with a
  breached cavern) laid by the hall or cellar for the carriers ("They pick the first pale cave
  mushrooms ... 20 meals", a moment; dwarves grow plump helmets); in a hard winter each harvest
  is a line. Seed 11: planted day 35, first harvest day 43. Tested (`a_farm_under_the_rock`).
- Thefts say "a meal" for one ("stole a meal from the store").
- Experience shapes character (`temper.rs`, DF's personality change and jaded dwarves): `feel`
  counts horrors (`TheDeep`, a close friend's death, being struck) and brave acts (`Saved`,
  `Slew`) per settler (`Mind::horrors_today`/`braved_today`). At dawn (`reckon_temper`): anxiety
  +2 a horror (95 at most; crossing into the 76+ band: "Since what she saw on day 20, X starts at
  every shadow"), bravery +3 a brave act ("X has found courage"), cheer +2 after thirty content
  days (stress under -0.4; "Life in the camp suits X"). Eight horrors: jaded (`Mind::jaded`, said
  once), and horror, death and the raid night weigh half on them from then on. The changed facets
  feed everything that reads them (the watch, the militia, breaks, feelings). Tested
  (`experience_changes_people`).
- The tavern (`tavern.rs`, DF's taverns): once a visitor has come and 12 live in the camp,
  `ProjectKind::Tavern` (5x4 roofed, 16 loads, urgency 1.3). With it bards come every 22+ days
  (not 45) and stay 8 nights (not 4), arrive "to the tavern" and perform "In the tavern"; at 20:00
  the sociable (gregarious 60+, 14+) near the fire share a cup there while drink lasts
  (`Feel::Friend` "an evening at the tavern", +1 opinion among them). A sellsword
  (`VisitKind::Sellsword`, planned in `visitors::plan`: a living figure with 2+ kills and no
  title, of a people not at war with the settlers', near their towns, no outlaw leader) may come
  at dawn while a raid is foretold (1 in 4): paid two meals a head if the store bears it plus three
  a head ("X, a sellsword of Y, is hired at the tavern to stand against Z: W pays 26 meals", a
  moment), they stand first in the fight with drill 0.45 and leave the morning after the raid;
  else they drink and go. Bards and sellswords are never outlaw leaders. Tested
  (`the_tavern_draws_the_world`).
- Written works (`books.rs`, DF's written content): a learned settler (values knowledge 20+ or
  analytical ability 1300+, 16+) with a quiet place (a temple, the tavern, the hall in the hill, a
  lord's hall) writes by day (`Job::Craft` "Writing ...", one at a time, a book a season each, no
  title twice): after day 120 the camp's chronicle ("The Chronicle of the Camp at 45,12" or its
  name), else a history of a scene of their past that reads as one ("On the siege of
  Brolmdustoor"), else a treatise on their best trade. A `craft::Work` of kind "book" (bark, or
  hide once a beast has been hunted), quality from analytical and linguistic ability, creativity
  and luck; caravans buy them, the annals list them; the chronicle is a moment. Dev 250 days: 3-5
  books a camp. Tested (`books_are_written`).
- The workshop's one-at-a-time limit (`craft_wish_any`) counts only those "Making something":
  writers, brewers, engravers and spear-makers had been blocking the carvers (fewer works, no gem
  settings, no migrants on dev 76).
- Fields may lie on gently sloping ground (`find_site_sloped`, one level across the lot) when no
  flat lot is free (hilly sites had no field for a year).
- The lord's demands (`nobles.rs::lord_demands`/`demand_met`, DF's noble demands): ten days after
  arriving and forty after each is met, "X demands a fine work of flint for her hall, within
  thirty days" (a material they love if the land gives it, else the land's stone and wood by
  turns: `land_stone`/`land_wood` in `craft.rs`). The workshop wants it (craft wish x1.5;
  `finish_craft` takes that material when stored; crafters break stone for it when none is laid
  by). A fine (2+) work of it is presented ("... the lord is pleased", kept from the caravans,
  +6 with the maker). Thirty days unmet: the best maker goes to the stocks (a moment, -2 with
  everyone) and the demand stands again. Seed 23: four demands in 300 days, all met within a week.
  Tested (`the_lord_demands_fine_work`).
- Livestock (`livestock.rs`, DF): from day 40 with 10 in the camp and a herd of two or more
  grazing within 30 cells, `ProjectKind::Pen` (6x5 posts, 10 loads, urgency 1.2); built, two of
  the herd are driven in (taken off the land; `Colony::pen` = kind and head). Each season's turn
  it grows by half (one at least from two; twelve at most). At dawn with the store under half the
  camp's goal and three head, one is slaughtered: 8 meals by the pen, a hunt for bone and hide,
  and those fond of the creature take it hard. Seed 23: red deer penned day 55, 3 then 4 head, one
  slaughtered on day 120. The HUD's standing lines show the lord's demand and its days left.
  Tested (`the_pen_breeds_and_feeds`).
- Materials (DF: the material decides what a thing is): `Colony::ores` records each kind of ore
  struck ("iron", "copper", "gold"...). `iron_worked` now needs iron or copper ore (or bought
  tools) (since the industries: forged or bought tools, and metal heads take bars); a mood that wants a metal needs that metal's own ore (iron also when bought). Spearheads
  are the best to hand (`finish_arm`): adamantine, iron, copper, obsidian from a gem cluster
  (uses one), bone where the land has no named stone, the land's stone (their old flat forces are
  gone: what a head does is its material's, "Materials and blows" at the end). No dev seed
  strikes ore yet, so its spears are flint or sandstone. Mithril has no deposits; adamantine comes
  from the deep shaft (`deep.rs`).
- The deep shaft (`deep.rs`, DF's adamantine and the hollow): from day 60, with a cavern breached
  and nothing of the deep still coming, a stone-first people or a greedy speaker (60+) sinks
  `ProjectKind::DeepShaft` (24 loads of timber shoring, told rather than walked: the column
  walking model cannot pass the open caverns). Done: with three cavern layers, one camp in two
  strikes adamantine ("glittering blue-white", a moment; `ores` gains it: spearheads of it at 2.2
  once a workshop stands). 12-20 days later (`reckon_hollow`, `hollow_day`) the miners break into
  "a hollow that should not be there" (a moment, everyone dreads it), and a demon (a generated
  forgotten-kind monster of fire and darkness, size 3.5, named in the demons' tongue) becomes the
  next trouble: it climbs out of the mine within days. Dev 76: adamantine day 63, hollow day 78,
  Jayjar the demon kills Austourd on day 85. Tested (`the_deep_shaft_opens_the_hollow`).
- A special attack's after-line reads "It froze X with its gaze: for days they could not see"
  (was "It had froze ...").
- Annals review (2026-10-08, reading a 300-day dev 76 chronicle): a death whose cause already
  made its moment (a fell mood, the drained, a festering wound, old age, an execution) no longer
  gets a second "The death of X" card from `bury`; a fell mood's artifact is of "bone" among the
  works ("the bones of X" stays in its own line); a visitor who comes to the camp is struck from
  any band they were to lead (dev 76: Aephre the Wanderer, staying in the camp, had led the
  Moonvale band against it and cut herself down); "a day's work", "1 level", and a builder named
  before any load is laid gets the skill reason.
- Kin of the raiders (DF: citizens do not fight their own civilisation): on a war band's raid
  night `raid_at` sets `fighting_people`; settlers of that people stand aside ("Aephre the
  Wanderer stands aside: they will not raise a hand against their own people, The Republic of
  Moonvale"; `Feel::Torn`, heavier for the loyal and loving) and strike no blow (`fight.rs`). At
  founding (`found_colony`), any planned war band or envoy of the founders' majority people comes
  instead as "deserters of X" (outlaws everyone fights): seed 23's first raid had been the
  settlers' own Skullmaw Dominion. Peoples who become the majority later (refugees, migrants) can
  still raid (seed 3: five of nine stand aside against Deepforge).
- Bands grow with the camp (DF's sieges grow with the fortress): a war band, outlaws or the
  Shadow's raiders come `band_size` strong (3, and one more for every eight in the camp, 8 at
  most), drawn on the map, each extra raider +0.05 to the raid's danger; a raid that overwhelms
  (danger over readiness + 0.65) takes a second life ("Before they are driven off X falls too").
  Established dev camps rarely see it: a year brings 5-6 raids and 0-3 deaths.
- Places in the hills (`explore.rs`, DF's discoveries): the embark's places (`local::places`:
  lairs, tombs, old mines, caves carved from the lore, each with a surface mouth) are found by
  anyone within 3 cells of a mouth (every ten minutes); the curious (65+, 14+) roam by day to a
  hashed spot 30-80 cells out (`explore_option`, 0.15-0.35). An old mine adds its ore to `ores`;
  a dead beast's lair gives up its hoard (treasures), a living one's is backed away from; a tomb
  is left sealed by the traditional (10+) or pious (60+), else robbed of the dead's arms (a
  treasure), and the dead rise at 21:00 that night under the Shadow or one time in three ("X,
  risen", hunting like the risen dead); a cave may show a vein of gems (two). Each a moment;
  `Colony::places_found`. Seed 58 robs the tomb of Glauzgagu on day 52 and he walks that night;
  seeds 76 and 3 find caves with crystal and amber. Tested (`places_in_the_hills_are_found`).
- Tuning after the places (2026-10-08): a sellsword comes the first dawn trouble is foretold
  with a tavern standing (the one-in-four roll had missed seven dawns running on seed 11); the
  militia hunts a foretold beast at once when it can (else one dawn in ten), with two drilled
  spears; any speaker sends it but one who values peace (20+) or is timid (bravery under 30)
  (`PLANET_FORCE_HUNT=1` overrides the speaker, for the test); a hunter whose beast is caged no
  longer leaves to follow its trail.
- Prisoners (`prisoners.rs`, DF's captives): a rout of a war band or outlaws leaves one of them
  taken alive one time in three ("As the rest flee, X drags one of them down and binds them: Y,
  of Z, is taken alive", a moment; `Colony::prisoner`). Next dawn the speaker (else the oldest)
  decides: cruel (70+) or martial without empathy, death at the gate (the altruistic resent it);
  lawful or fair with the prisoner's people standing, ransom (an envoy 6-10 days on pays two
  meals a head); else sent home with a warning. Held, one night in ten they slip the ropes.
  Tested (`a_raider_is_taken_alive`).
- The camp and the world (`regard.rs`, DF's diplomatic evaluation from enumerated causes):
  `Colony::regards` keeps, per people the camp has dealt with, a list of causes with their
  reasons: a war band driven off -5 (merged), its leader cut down -10, a prisoner put to death
  -20 or sent home unharmed +20, a ransom taken +3, a relic given back +25 or kept -15, tribute
  paid +5 or refused -5, a sellsword of theirs hired +3, each caravan +2 (trade +10 at most, so
  trade alone never makes friends). At dawn (`reckon_regard`, after the prisoner) a people at
  -15 or worse swears vengeance once (a moment; bad news for all; a war band of theirs goes to the
  front of the troubles, "they have sworn vengeance, for the camp put X ... to death"); one at +15
  or better comes in friendship once (two meals a head, 30 at most; their war bands leave the
  troubles). The annals list it ("The Camp and the World"). Dev 300 days: seed 76's kept staff
  and an executed Moonvale prisoner bring two vengeances; seed 23's trade and tribute make the
  Skullmaw Horde friends on day 166. Tested (`the_world_remembers_what_the_camp_did`).
- Armour (`armour.rs`, DF's armour layers): the best hand at the workshop makes armour for each
  spear borne ("Making armour", by day; 1.0 with trouble foretold, else 0.6; spears first while
  a raid is foretold, armour first in quiet times), of the best to hand (the shares were the old
  flat cover; now layers, "Materials and blows"): adamantine mail 0.75 (deep shaft), iron mail 0.55 (iron
  ore or bought tools, iron worked), copper scale 0.45, a leather jerkin 0.25 (a hide: `hunted`
  less `hides_used`; the pen's slaughter counts). `armour_up` gives the best to the best fighters
  with the spears; "Bears X in the militia, and wears Y" on the settler page. In the clash a
  rescue's wound is a step lighter when it turns the blow ("does not get through", "though the
  leather jerkin took the worst of it"), a defender's bruise is turned, and a killing blow is
  turned at half the cover / the foe's size: the raid becomes a rescue ("The blow that should
  have killed X is turned by the adamantine mail shirt", a moment the first time). Expedition
  danger x(1 - cover). With it (2026-10-08): spears are also made at leisure after the first raid
  (0.5, "against the next trouble"; `arms_wanted`), and the spear-bearers join the clash's
  defenders after the hunters, saviour and watcher (`fight.rs`). Dev 300 days: armour on four
  seeds (76 adamantine day 73; leather on 11, 5, 23); seed 23's adamantine mail turns a spear on
  day 148. A relic found by chance now makes its tale known (`find_relic`), so its seeker comes.
  Tested (`armour_turns_blows`; since the industries mail is forged from bars and leather or
  hide only lightens an edge, so the test forces ore: seeds 23 and 76 turn blows on days 69 and 101 (since needs);
  unforced, no blow was turned on six dev seeds in 300 days).
- Snatchers (`snatch.rs`, DF's baby-snatchers): `found_colony` notes the peoples among the
  troubles whose race steals children (goblins, orcs; `Colony::snatchers`); the Shadow's raiders
  steal them too. On their raid night (`snatch_in_the_raid`, end of `raid_at`) one time in two a
  child (an infant to 11) is carried off unless the watcher is within six cells ("X is gone from
  his bed: small tracks and larger ones lead off into the dark after Y", a moment; `alive`
  false, `Colony::snatched`; parents grieve as for a death; annals fate "carried off by Y on day
  N"). Sixty days later (`reckon_snatched`) the same raiders are put first among the troubles
  ("they came before and carried off X"), and the child is with them (`snatched_return`): under
  six, on a raider's back, pulled free only if the camp routs the band; older, at the edge of the
  firelight, called by a parent (or whoever cares most): 65% (45% after 120 days) they drop the
  spear and come home (older by the years gone), else run with the raiders; after two
  refusals "one of them now". A raid that brings one back takes no other; two taken at most.
  `PLANET_FORCE_SNATCH=1` makes every war band snatch (the dev world's snatching peoples rarely
  raid a camp with children: seed 5 only). Forced: seed 23's Skaash taken day 148, pulled free
  day 224; seed 3's Wock carried off again on day 210, pulled free day 273. Tested
  (`snatched_children_come_back_among_the_raiders`).
- Sieges (`siege.rs`, DF): after the first trouble, on the eve of a raid against a closed
  palisade by the Shadow's raiders, or by a war band of five or more (a camp of 16), one time
  in two (`siege_begins`, before `send_attackers`) the raiders camp 25 cells out on their side
  instead ("... do not come in the night: at dusk their fires are lit at the edge of the
  clearing", a moment; a scorch mark "The besiegers' camp"); the raid is put off 3-5 days
  (`Arc::raid_day`). While besieged (`Colony::siege`) ground more than 12 cells from the fire is
  out of bounds (`under_siege_out`, read by `marked(p, true)` like forbidden ground). Each dawn
  (`reckon_siege`): a brave (50+) or martial speaker with three drilled spears sallies out of the
  gate (the raid fought at once at their camp, +0.15 readiness); an empty store brings the
  assault that night; on the last dawn a camp at readiness 0.85+ sees the fires go cold (no
  raid, a moment); else "Day N of the siege: ... M meals in the store for K" and the assault
  comes as a raid. The HUD's standing lines say "Besieged by X: day N of the siege". The
  besiegers stand at their fires on the map (`CreatureKind::Besieger`, a band-size row, drawn as
  raiders with a fire's glow; cleared when the siege lifts or the camp sallies), and on the
  assault's eve the attackers set out from the siege camp (`send_attackers`). Dev 300
  days: sieges on 76 (lifted day 61; a sally day 147) and 11. Tested
  (`raiders_lay_siege_to_a_walled_camp`). The move trial accepts a raid death and a snatched
  child.
- Evil weather (`weather.rs`, DF's evil regions): where the Shadow's darkness on the tile is
  0.35+, from day 20, one day in each twelve-day span (every other span under 0.5), at an hour
  from 10:00 to 17:00, something comes over the camp (`evil_weather`, checked on the hour): a red
  rain, a black mist, a cloud of grey ash, a hail of black ice. Those within four cells of a roof
  (the huts, the hall in the rock, a second hut, storehouse, workshop, temple, tavern, lord's
  hall, smokehouse; `sheltered`) are safe; the rest fall ill 1-3 days by hardiness and dread it
  (`Feel::TheDeep`). Half the field's standing crop blackens; a pen of two loses a head. The
  first is a moment. Dev: seeds 11 (0.48), 5 (0.41) and 58 (0.35) get it; 76, 23, 3 never.
  Tested (`evil_weather_under_the_shadow`). `run_log` in the tests now uses a folder per call
  (two parallel tests on one seed and length had deleted each other's log); the tomb test runs
  seed 58 for 200 days (found day 182).
- Guilds (`guild.rs`, DF's nested organizations): from day 60, four grown settlers who are
  masters (0.8+) of a trade that is their best (forage, fish, fell, build; carrying is no craft)
  swear themselves to its guild ("The Fellowship of the Basket", a moment; +2 among them); new
  masters join. Ten days on, the best hand (not the speaker) petitions the speaker (else the
  oldest): granted by craftsmanship + hard work - tradition/2 + 3 x their mean opinion of the
  members (+15 if one of them): a `ProjectKind::GuildHall` (4x3 roofed, 12 loads; shelter from
  evil weather), members +3 toward the speaker; refused: -3 and `Feel::Mandate`, asked again in
  60 days. With the hall, members learn their trade 1.5x (`guild_learning` in `finish`) and
  feast at each season's turn (+1). Settler page "Sworn to X". Dev 300 days: a foragers' guild on
  all six seeds (days 60-202), builders' on 76; every petition so far granted (the speakers like
  their foragers). Tested (`masters_form_a_guild`).
- Legends carry regard (2026-10-08): `Legend::regards` keeps each people at |10|+ with its sum
  and weightiest cause (serde default: older legend files load); `heed_legends` gives the new
  camp half of it (12 at most) as a "legend" cause ("is remembered for the camp at 45,12: it would
  not give The Staff of Greenburg back"). `--sim-legend` prints both camps' regards; seed 23:
  the Greenburg League -20 toward the first camp, -10 toward the second (seed 76's grudge over the
  staff is cancelled since it sends its Git Clans prisoner home unharmed). Tested
  (`peoples_remember_earlier_camps`).
- Quality pass (2026-10-08, reading seed 76's days 120-200): a sally's raid line no longer says
  the band "came in the night" (it "was met at their own fires at first light, and broke before
  the spears"; the wounded are dragged back to the gate; no stale side); a maker's works show
  the scene they have shown least (their past's images, then each raid; `finish_craft`), where
  one hand had carved the same hunt five times.
- A rising against the lord (`rising.rs`, DF's nobles and unhappy subjects): while a lord rules,
  `feel` counts grievances (`Colony::grievances`): a `Feel::Mandate` 1, `Feel::Punished` 2 and
  1 to each friend (12+) of the punished. At dawn (`reckon_rising`), 30+ days after the lord
  came, with three aggrieved (3+) making a quarter of the grown camp, one dawn in four the
  boldest (bravery + independence + violence) leads them to the hall (a moment, once per lord).
  The loyal hold the lord at 20+ with at most one grievance and value loyalty, law or tradition.
  More aggrieved: the lord is killed (a cruel or violent leader) or put on the road home; the
  office is empty until the camp chooses again; the lord's people -35 / -20 (`regard.rs`, which
  now lets a friendship break into vengeance). Fewer: the leader goes to the stocks three days
  (death under a cruel lord). Seed 23: Baangh is killed on day 208 by sixteen (no one stands by
  her), and the Skullmaw Horde swear vengeance. Tested (`the_camp_rises_against_its_lord`).
- Opinions (2026-10-08): everyday warmth (shared meals, songs heard, festivals, the tavern,
  guild feasts) goes through `Colony::warm`, which stops at `FAMILIAR` (24); what people do for
  or against one another (rescues, punishments, quarrels, mandates) still moves opinions freely.
  A year of meals had put everyone at +100 for everyone. `Mind::scars` keeps the heaviest
  thoughts (-0.15 or worse) of the last 30 days (four) so a break names them after light days
  pushed them out of the ten remembered (seed 58 had broken "because nothing they will name");
  scars weigh nothing again. (Keeping heavy thoughts in the ten instead had doubled seed 58's
  breaks: fewer copies of daily hunger left to dull it.) Evil weather is a dread only the first
  time; after that `Feel::Ill`. "were put in the stocks" (was "was").
- The liaison (`liaison.rs`, DF's outpost liaison): after each caravan's trade the speaker (else
  the oldest) asks for the camp's worst want, with its reason (`worst_want`): healing herbs while
  someone lies wounded, salt while food keeps under 60 days (no smokehouse, or a low store), seed
  grain for a standing field, iron tools from an iron-holding town when the camp works none.
  The next caravan brings it if the camp traded (else "they take it home again"): salt doubles
  `keeps_days` for 60 days; seed grain makes the field give 3 meals a crop; herbs make six
  tendings heal a third sooner; tools set `tools_bought`. Dev: seed grain on 76, 5, 23, 3; salt
  then seed grain on 11; herbs on 3. Tested (`the_liaison_brings_what_was_asked`).
- Pets defend (`pets.rs::pet_defends`, DF): when a night hunter (wolves, cave hunters, the
  risen dead; not a werebeast) reaches a settler whose pet is within 12 cells, the pet comes
  running: a big beast (elk, bison, boar, bear, aurochs, caribou...) drives them off two times in
  three, a small one one in three (`Feel::SavedBy`); failing, one time in three it is dragged down
  in the keeper's place (grieved as a close friend); else the bite. Seed 58: Tha the caribou dies
  for Kurng on day 227. Tested (`a_pet_stands_by_its_keeper`); after news-as-told moved the
  timelines no keeper was caught alone at night on 12 dev seeds in 300 days, so the test sets
  `PLANET_FORCE_PET_PREY=1` (night hunters take a pet's keeper first among those alone; seed 1:
  Nash'fai the red deer stands over Ife on day 22).
- Old comrades and old enemies (`recognize.rs`, DF's per-figure knowledge of events): at dawn
  two settlers (guests too) whose `Past::lines` cite the same battle or siege event recognise
  each other once (`Colony::recognized`): of one people, comrades (+6, `Feel::Friend`, "find they
  both stood at the Battle of X"); of two, opposite sides: both vengefulness under 40, a cup and
  it lies (+2); else a grudge (-8, `Feel::Quarrel`, a moment "Old enemies at the fire"). Dev:
  migrants on 76 and 5 find the veterans of their battles. Tested
  (`old_comrades_find_each_other`; lib `battle_names_come_out_of_lines`).
- A slain beast's remains (DF: creature parts are materials): `maybe_slay` keeps
  `Colony::remains` (name, short name, 4 + 3 x size bones, 1 + size hides, its skin or "X plates"
  for a stone/metal hide). Half the workshop's works use a bone while they last ("a
  finely-crafted Baelfang-bone figurine showing the death of Baelfang Storm-Caller"; +8 to a
  caravan), and armour takes the hide before copper or leather ("a coat of Baelfang's fur", 0.45;
  plates 0.5). Dev: 76, 11, 3. Tested (`a_slain_beast_becomes_bone_and_armour`).
- The saga plate weighs the new moments (`viewer::saga_plate`): the siege, a lord deposed and
  vengeance sworn with deaths and raids; a child taken or come home, a sally, the siege lifted,
  the armour holding, a rising and old enemies with artifacts and hunts; friendships, guild
  halls and guilds founded with weddings. The annals' people name their guild and armour.
- Ambushed caravans (`trade.rs::caravan_ambushed`, DF): while a war band, outlaws or the
  Shadow's raiders are foretold (arc stage 1, 2 or 5; not the traders' own people; never the
  first caravan), one caravan in two is taken on the road ("The caravan from X does not come.
  Toward noon a mule walks in alone, its packs slashed", a moment): no trade that season, the
  liaison's request is lost ("lies scattered on the road"), settlers of the traders' people hear
  it as bad news. Seed 5: Regaldolmen's caravan of day 75 and its seed grain. Tested
  (`a_caravan_is_taken_on_the_road`).
- Staying put (2026-10-08): `reckon_food` counts what the camp makes for itself (the farm under
  the rock 1.5-2.5 meals a day, caravans 1.6, the store over 30 days) besides the wild, and a
  camp with five works standing moves or leaves only when people starve with an empty store
  (seed 11's camp of 14 had left its land on day 151 in summer). Six dev seeds now see a year
  out where they stand. Tested (`an_established_camp_does_not_walk_away`).
- Answering what comes back (`respond.rs`, DF): after four wolf bites (`wolf_bites`), at dawn two
  to four of the bold (spear-bearers or bravery 50+; grown, well, no guests) kill the pack at the
  nearest den with spears and fire (a moment, a deed for the leader, one time in three one is
  bitten; the den is gone from `dens()` via `dens_cleared`). A grave whose dead rose three times
  (`risings`, counted in `dead_rise`) is dug up and burned on a pyre (a moment; `burned` graves
  stay quiet; the dead's close friends grieve again). Seed 11: dens cleared days 13 and 34, Zok
  burned day 26 (he had risen every few nights to day 200); seeds 5 and 58 burn their dead.
  Tested (`the_camp_clears_the_den_and_burns_the_restless`).
- Reading seed 3 (2026-10-08): no festival in a week the store ran empty ("the store ran empty
  this week"); a sally's rescue drags the wounded "back through the gate". Chilled nights in
  summer are the night watch (everyone past the first hut's six sleeps in the second hut).
- Food first (2026-10-08): `hungry_days` counts dawns running that the camp went hungry (under
  two meals a head and someone at 0.85); at five, `plan_projects` plans a work that feeds it
  (`projects::feeds`: field, farm under the rock, pen, jetty) ahead of the work under way
  (inserted first), and builders go on building a feeding work while hungry (the hungry gate
  spares it). Seed 58 had sat the year hungry behind a well it never finished; now a field on
  day 26 and ten more works (8 of 12 alive at a year, was 7 of 11). Tested
  (`a_hungry_camp_plants_first`).
- Artifact thieves (`thieves.rs`, DF): twenty days after a mood's artifact (`is_artifact`), at
  23:00 one night in forty and sixty days at least between attempts (`thief_day`), a thief of a
  war band's people among the troubles comes (child-stealing peoples first; never a people come
  in friendship). Someone awake within six cells of the fire, or the watch, catches them: the
  camp's prisoner, judged at dawn (seed 11: Grark is sent home, and his people come in
  friendship). Else the artifact is gone (`Colony::stolen`, a moment; its maker grieves). A
  routed war band of that people leaves it behind one time in two (`recover_stolen`). Dev 400
  days: thefts on 76, 58; thieves caught on 11, 5. Tested (`thieves_come_for_the_artifact`).
- The priest (`priest.rs`, DF's temple positions and proper burial): with a temple standing, at
  dawn the most pious grown settler (60+, no other office, not the lord) keeps it (office "Keeps
  the temple of X"; the first a moment; chosen again when gone). The next dawn with graves they
  consecrate them (`Colony::consecrated`): no dead rise (`dead.rs`) unless the darkness is 0.6+.
  A violent death with a priest living gets the rites (`rites`, via `mourn`): one time in two the
  dead rest without a slab. Dev: priests on 76, 11, 5, 23, 3 (days 22-129); seed 76's Thano
  says the rites over Noostond on day 40. Tested (`a_priest_keeps_the_temple`).
- The call to arms (`warcall.rs`, DF's missions; ROADMAP Update 5 "outflow"): `warcall::plan` at
  founding notes the first war in the history that the founders' people fight
  (`Colony::war_call`). From day 60 with 8+ in the camp, once, a rider of that people calls for
  spears: up to two grown settlers of that people, brave (50+) and valuing arms or loyalty (20+),
  not the lord, priest, healer, a guest or a parent of an infant, go for 60-90 days (away like an
  expedition; "away at X" in the annals; friends hear it as bad news). Each comes home seasoned
  (a deed, drill +0.2), or wounded too, or falls ("Word comes ...: X fell in Y", a stone in their
  memory; annals "fell in Y"), by a hash: 2:1:1. Only seed 3's people are at war on the dev
  world: Tekkeburx comes home limping, Ord falls. Tested (`the_camp_is_called_to_war`).
- A place's contents start with a capital after its cause ("... in 246. Empty bins, ...").
- The woods grow back (`regrow.rs`, DF's regrowth; ROADMAP Update 5 ecology coupling): each
  felled tree is kept with its kind (`Colony::felled`); at each season's turn trees felled a year
  (120 days) ago return where the ground is open (not built on or beside, no roof, under 15
  footsteps), and the stump goes. Dev: 23-29 trees back on day 151, then 8-21 a season. Tested
  (`the_woods_grow_back`).
- Defenders behind the first are caught by the foe too (0.15 each, the first 0.3; `fight.rs`):
  armour had rarely been tested, since the first defender is usually a hunter or the watcher.
  Turned blows on 76 (day 161) and 23 (147, 224); `armour_turns_blows` runs 230 days.
- The tithe (`tithe.rs`, DF's tax collector; ROADMAP Update 5 "tax collectors and tribute
  demands"): from day 100, once a camp year, a collector of the trading town's people asks a
  tenth of the store (10 meals at least) and two unsold works (no artifact or masterwork); under
  two meals a head, nothing is taken. The speaker pays when of that people and loyalty + law +
  tradition - independence - greed/2 >= -10, or when not of it but the camp has under three
  spears; else refuses (a moment). Paid: regard +5, and the greedy and independent resent it
  (`Feel::Mandate`, a grievance under a lord). Refused: regard -10. Dev: paid on 11, 5, 23,
  refused on 76, 3, waived on 58. Tested (`the_tithe_is_paid_or_refused`).
- Former spouses (DF's never-deleted links): `bury` ends a marriage and keeps it in
  `Colony::widowed` (survivor, the dead, day); the widowed mourn 60 days before they may wed
  again; `family_of` says "widow of X" / "widower of X", or "once wed to X" after remarrying.
  Dev year: on 23 (Baangh's widower), 58, 76. Tested (`the_widowed_are_remembered`).
- Vows of vengeance (`vow.rs`, DF's revenge goals; ROADMAP's director threads on settlers): a
  raid's death (`raid_at`) makes the dead's closest (child, parent or widowed spouse, else the
  friend at 12+; grown, no guest, no vow already) swear by the grave to see the killer dead (a
  beast by name, a war band's leader; a moment; `Colony::vows`). Until kept they drill twice as
  fast and stand first among defenders against that foe (`sworn_against`; `fight` keeps the
  foe's name before raiders become "one of the raiders"). The killer's death (`maybe_slay`, a
  leader cut down, the hunting party) keeps it: by their hand, a moment, a deed and bravery +5;
  by another's, a line and peace. Settler page and annals carry it. Dev: vows on 76, 11, 3.
  Tested (`the_dead_are_avenged_by_vow`).
- Reading seed 5's second year (2026-10-08): a sixth theft gets the culprit cast out (a moment;
  their friends grieve), or three days in the stocks when the judge is their spouse or holds them
  at 20+ (seed 5's Rownd keeps protecting Ougo); the tithe never takes the store below ten meals
  a head (twenty with winter near or in a hard winter), and takes nothing when that leaves no
  meals and no works.
- Engravings know the later moments (`engrave.rs::worth`): a lord deposed, a vow kept or a fall
  in the war 9; the siege and a sally 7; a child taken, the siege lifted, a theft 6; homecomings,
  calls to war, vengeance sworn, a thief caught, armour holding 5; the wolves' den, a burning, a
  friendship, a guild founded 4; weddings, births and festivals 3; phrased "the founding of",
  "the homecoming of", "the theft of", "the fall of", "X keeping the vow". After the camp's story
  and the engraver's past: others' remembered scenes, then "the camp by its fire" once, then
  "the camp in its Nth year" (seed 5 had carved the camp by its fire four times).
- Those home from the camp's war count as veterans (`is_veteran` reads the deed "fought in X"): the watch, a veteran's fighting hand.
- A camp's legend keeps its sieges, sallies, risings, vows, children taken and come home, falls in war, thefts, vengeance and friendships among its deeds (`legend.rs`).
- Moods come again (DF: each dwarf at most once): a new mood may take someone who has not had
  one (`Colony::moods_had`) 120 days after the last began, in a camp of ten or more. Dev 500
  days: 2-3 moods a camp (two madnesses on seed 3), so thieves have more to come for.
- Reading seed 23's second year (2026-10-08): a people sworn to vengeance sends no caravans ("No
  caravan comes from X this season: Y have sworn vengeance on the camp") and no tithe collector;
  the pen gives one beast a week at most (`slaughter_day`; seed 23 had slaughtered five dawns
  running); an artifact is "an altar".
- Peace (`regard.rs`): a people sworn to vengeance (`acted_day`) offers peace 180 days on, or 90
  after one of its bands was driven off since, and not while a band of theirs is coming. The
  speaker takes it unless they despise peace (-26) or are vengeful (70+): the old grudge is
  mended to zero (a "peace" cause), their war bands leave the troubles, caravans and collectors
  come again (a moment); refused, they ask again later. A new vengeance names something done
  since the last peace or friendship. Seed 76: peace with Moonvale (day 237) and the Git Clans
  (243); Titankeep swears vengeance over a refused tithe, makes peace, then swears it again over
  a prisoner put to death. Tested (`vengeance_can_end_in_peace`).
- The HUD's standing lines name peoples sworn to vengeance on the camp (`Colony::standing`).
- Spoilage before a rack or smokehouse is said the first time and every fifth day (it had been a line every dawn).
- The kitchen (`kitchen.rs`, DF's cooking): with twelve in the camp and a storehouse,
  `ProjectKind::Kitchen` (3x3 roofed, 8 loads, urgency 1.1). At dawn a cook is named (the most
  immoderate, then patient, grown settler with no office; "Cooks for the camp"). At 16:00, with 3
  meals a head stored, the cook makes supper (`cook_supper`): a dish from what the camp has
  (`dish`: venison or the pen's meat once something was hunted, river fish, barley ("good barley"
  with seed grain), cave mushrooms, berries out of hard winter), "a stew/roast/pie/broth of A and
  B", fine by the cook's sure hands and patience. A meal eaten 16:00-22:00 that day is
  `Feel::AteWell` (more if fine, more for the immoderate), once a day (`last_supper`). Dev:
  kitchens on five seeds by days 91-153. Tested (`the_cook_makes_supper`).
- Clothes (`clothes.rs`, DF's clothing): each settler's clothes have an age (`Colony::clothes`;
  those who came wore clothes 0-150 days old, by name). At 180 days they are rags: every fifth
  dawn `Feel::Ragged` (vanity weighs it; the first time a line). With a workshop and a hide (the
  hunt's and the pen's, shared with armour) or caravan cloth (`Colony::cloth`, two suits a bolt),
  one settler at a time who likes making things sews for the most ragged (`sew_option`, 0.45 x
  their wish; "Sewing ..."), +2 from the one clothed. The liaison asks for bolts of cloth when
  three are ragged and no hide is to hand. Dev: rags from day 35; cloth brought on 76, 5, 23, 3.
- At the bench (2026-10-08): a settler at work on a six-hour craft eats only at hunger 0.85 and
  naps by day only at fatigue 0.85 (`decide`'s `at_bench`): meals at 0.6 had broken off nearly
  every piece, and a change in timing had cut seed 76's works from 37 to 9 in 120 days (now 19,
  the rest of the camp's days going to its winter store).
- Roamers follow old tracks: one roam in three heads within ten cells of a place not yet found
  that can be walked to (`explore_option`).
- A seeker who leaves before the relic is found comes back three days after it is ("X comes back
  to the camp: word has reached him that ... was found"; `Visitor::left_once`), the same settler
  revived, not a second one.
- Tests that wait on chance events try more seeds or longer (places on 58/11/5, friendship on
  76/23/5, the chronicle on 23/11, the relic and its seeker 110 days, the pet 260, seed 58's
  breaks up to 20; `--sim-legend` lives 130 days; `PLANET_FORCE_AMBUSH=1` takes every caravan after the first).
- Dreams of a lifetime (`dreams.rs`, DF's life goals; `LifeDream`, not the patron's `Dream`): a
  grown settler's dearest value at 15+ that has a dream gives it (`life_dream`): family or romance
  a child; craftsmanship or artwork a masterwork; martial prowess a foe slain by their hand;
  knowledge a book; power speaking for the camp; nature (or curiosity) finding what lies in the
  hills; peace a war set aside. At dawn (`reckon_dreams`) the camp's record is read for it; met,
  a moment, `Feel::Dreamt` (0.6) and cheer +5; once (`Colony::dreamt`). Settler page and annals
  carry "Dreams of ..." / "Realized a dream of ...". Dev 300 days: 1-3 a camp (a child on 3, a
  book on 5 and 76, a foe slain on 23, a discovery on 76 and 11). Tested (`dreams_come_true`).
- Dreams realized weigh on the saga plate (2) and in engravings (4, "the dream of X come true") and are kept in legends.
- Experience does not change infants' character (`temper.rs` skips those under four: "life in the camp suits" a newborn).
- The troubles come again (`arc.rs`, `Arc::recurring`: the plan's outlaws and Shadow raiders):
  when the later threads and the reserve are both spent, at 07:00 sixty quiet days after the last
  beat with the camp ten worth richer than at the last draw, one of them comes again ("word of
  the camp's riches has spread along the roads", "the Shadow of X reaches this far still, and
  the camp has grown rich"). Dev 600 days: 5-6 raids after day 300 (camps had gone quiet in
  their third year). Tested (`troubles_come_again_when_all_are_spent`).
- Childhood (`childhood.rs`, DF's children): a child born here, four to eleven (`child_choice`,
  after `infant_choice`), eats and sleeps like anyone and by day tags along after a parent at work
  ("Tagging along after X, who is foraging") or plays by the fire; each dawn they learn a tenth of
  the way to 0.4 in the abler parent's best trade (`reckon_childhood`); at twelve "X has come of
  age, and takes up the work of a ..." (`Colony::come_of_age`). Children reach four in a camp's
  fifth year (seed 3 by day 600). Tested (`children_play_and_tag_along`, 620 days).
- Ten camp years (seed 76, 1,200 days) run in 19 s. The pen gives a beast any day someone is
  starving (the week's limit is for a low store only). Seed 76's seventh winter is a famine that
  the systems make between them: Gaunauth refused Titankeep's tithe, Titankeep swore vengeance,
  its war band besieged the camp at winter's end with 26 meals in the store, and six starved on
  day 840; two founders then left for good.
- Rations (`rations.rs`): at dawn in a hard winter, when `days_of_food` falls short of the days
  until spring, the speaker (else the oldest) orders half rations (a line; the first a moment):
  every second meal a settler eats takes nothing from the store (`Settler::rationed`) and is
  `Feel::Rationed` (the greedy and immoderate mind it most); lifted when the store holds the rest
  of winter and a fifth more, or at spring. Seed 76 over ten years: no deaths by hunger (14
  before, six in one famine), 32 of 36 alive at day 1,201. Tested
  (`the_store_is_rationed_in_a_hard_winter`).
- Ice (DF's freezing water; TODO "rivers freeze with the seasons"): in a deep freeze (a hard
  winter below -3 °C, `Colony::frozen`) the fishing places are ice except within two cells of the
  jetty ("holes cut at the jetty"; `nearest_fishing`); the freeze and the thaw are lines at dawn
  (`reckon_ice`). Seed 3 (-7 °C) freezes from day 91 to 121; the other dev seeds' winters are
  milder. Tested (`the_water_freezes_in_a_deep_winter`).
- Seasonal herds (TODO Phase D's migration): at the first hard-winter dawn the embark's herds
  leave (`reckon_herds`: game creatures removed, `game_returns` waits) and at the first dawn
  after it they come back (`spawn_game`); a line each. Pets and the pen stay. Tested
  (`the_herds_winter_elsewhere`).
- Pacing (`Moment::major`, `viewer.rs`): only major moments stop the window's clock and show a
  card (deaths, raids, troubles foretold, births, moods, artifacts, choices...); the routine
  (works finished, roles named, the speaker chosen, festivals, weddings, the tithe, dreams,
  guilds, visitors, migrants...) goes to the status line. The dev camp's first month: 33
  moments, 10 major (one card every three days, was more than one a day). `--sim-snapshot`
  prints "Moments: N, M major". Tested (`only_major_moments_stop_the_clock`).
- The HUD's standing lines say when the camp is on half rations, with its days of food.
- Tavern brawls (`tavern.rs`, DF): at 20:00, among those sharing a cup, a pair who dislike each
  other (-3 or worse) with one of them angry (70+) or drawn to violence (60+) come to blows one
  evening in four (the same two not within twenty days): the other gets a split lip (a head
  wound), opinions -4, and the striker's crime ("struck X at the tavern") is judged at dawn. Dev:
  seeds 11 and 23. Tested (`brawls_break_out_at_the_tavern`).
- Raiders carry their people's arms (`fight.rs`): a spear, an axe, a sword, a mace, a club or a
  long knife, the same for the same people (by its name). Dreams read with the settler's own
  pronoun ("slaying a great foe with his own hand").
- The patron's bell (`Colony::ring_bell`, DF's civilian alert; window key B, "B bell" in the HUD;
  intervention "tick bell"): one favour, and until the next dawn (`bell_until`) everyone but
  guests keeps under a roof: they eat, go abed early ("Abed early at the patron's bell") or wait
  by their beds. Readiness +0.1 that night (the tally says "warned by the bell"), the raiders find
  no child out of doors (`snatch.rs`), and an evil sky finds everyone sheltered (`weather.rs`).
  `--sim-projects` replays a script from `PLANET_SCRIPT=FILE`. Tested
  (`the_bell_sends_everyone_indoors`). Not tried in a real window.
- On other worlds (seed 42 at 192x96: tiles 50,72, 92,59, 74,27) camps run 200 days without fault (15-17 alive; dwarves in stone). The first tree felled says "for the hut" only while a timber hut is going up.
- The HUD's standing lines count those away at the war (`warcall.rs`).
- The library (`ProjectKind::Library`, 4x3 roofed, 12 loads; DF's libraries): once two books are
  written and ten live in the camp ("N books lie about the huts, and the damp and the traders get
  at them"). With it caravans leave the books alone, writers may write twice a season, and a
  visiting loremaster leaves a written copy of one of their people's works ("The Words of
  Therdre", a book of quality 3 by the guest; once each). Libraries on 76, 11, 23, 3; seed 3's
  Dmozrua leaves a book on day 145. Tested (`the_camp_keeps_a_library`).

## Space in three dimensions (2026-10-08, DF's map, digging and constructions)
- Walking is 3D (`nav::path3`, `steps3`, `standable`, `cost3`; `P3 = (x, y, z)`): one can stand
  wherever solid ground or a stair (`Shape::Stair`, new) is underfoot with open space (air or a
  stair) above, under one level of water at most. Steps go to the 8 neighbours at the same level
  or one up/down (headroom to climb, room to step down into), and up or down stair cells. Surface
  cells use dense arrays, others (halls, stairs, caverns, a tower's platform) a det map. The old
  column walk (`path`, `path_worn`, `cost`, `passable`) stays for creatures and surface checks.
- Settlers carry `z` and a `Vec<P3>` path; `here3(i)` (falls back to the surface after a
  teleport), `below(i)` (under the ground: sheltered, never wolves' prey, never met by raiders
  in `attackers_clash`). `start` asks `spot_level` for the target's level: a bedroom's bed, the
  hall or cellar for sleep, the great hall or hall to eat, rooms for work there, the level of
  a settler walked to, the tower's platform for the watch, else the surface.
- Digging (`dig.rs`, rewritten): a cut `DigCell { p, z, stair }` works on the cell over the floor
  at level z: a room cut opens it and keeps the rock above as the roof (the ground above stays
  walkable; only opening the surface's own cell lowers `surface_z`), a stair cut makes it a
  stair. Diggers stand beside a room cut on ground known to be walked (`cut_stand`: the surface,
  the spine, a cut made, a room) or on the stair cut itself (digging down underfoot). As many dig
  at once as the camp has picks (`picks`: 2, +2 with a workshop, +2 with metal worked). Dug stone
  is carried up to the delve's mouth (`delve_mouth`). `solid_room_cell`: rock for floor, body
  and roof (the roof may be another room's floor), a level of ground over it, no cavern, no water.
- The delve (`delve.rs`, DF's fortress): the first dig makes the stair spine (`Spine { at, top,
  bottom }`): a cellar is a stair three levels down beside the camp and a 4x3 room off its foot;
  a hill hall keeps its farthest cell for the stair. `plan_level` puts a level of rooms off the
  stair at the shallowest free level two or more below its top (cutting the stair on when
  needed), in the first of four directions where all is solid with a wall of rock round it:
  `ProjectKind::Bedrooms` (2-8 rooms of 2x2 with a door off a corridor, a bed each; urgency 1.5
  for a stone-first people, else 0.8, from day 20 with 4+ grown settlers without a room; free
  bedrooms count as given) and `ProjectKind::GreatHall` (7x5 with a long table; from day 30, 10+
  alive, after bedrooms or for masons). `Colony::rooms` (`Room { kind, z, cells, owner, bed }`,
  `RoomKind`), `dig_rooms` until the dig is done (`dig_finished`). The mine is the spine going on
  down (up to 35 levels); a breach lets a timber stair down through the cavern's air to its floor
  (`let_down_stair`), so the dark can be walked. `reckon_rooms` at dawn gives free bedrooms to
  grown settlers without one, eldest first (a married pair share), "X and Y take a bedroom of
  their own, cut in the sediment 3 levels down"; sleeping there is `Feel::OwnRoom`. The camp eats
  in the great hall once it stands. Dev 45,12: cellar day 14, the mine breaks into the first
  cavern 15 levels down on day 26, 8 bedrooms on day 34-36, the great hall day ~71; dev 50,20:
  hall day 39, mine, bedrooms 2 levels under the hall day 80, great hall day 85. Tested
  (`the_camp_digs_a_delve`).
- The lookout is a tower (`raise_tower`, DF's constructions): on its 2x2 lot three cells walled
  two levels high (dressed blocks of the land's rock or timber) with a floor laid on top, the
  fourth a stair from the ground to the platform; `Colony::tower`, `watch_post` returns it, and
  the night's watch stands on the platform ("Keeping watch on the lookout's platform").
- Fixes the longer walks showed: a settler's own engraving, writing, sewing, brewing, carving or
  workshop work no longer counts as "someone else is at it" (the option vanished on the way and
  they dropped the job); a vow is sworn by a friend at 8+ (was 12); `PLANET_FORCE_RAID_DEATH=1`
  makes the first raid kill (the vow's test: raid deaths are rare in camps that dig in and arm).
  Debug: `PLANET_DEBUG_DIG` prints when each dig finishes. `PLANET_FRAMES` now writes the
  section through the spine, one frame per level with rooms or the tower's platform
  (`_below.png`, `_below2.png`...) and the surface. A dev year (seed 76, `--sim-projects 365`)
  takes ~7 s (camps of 20+).
- Wood from the dark (`delve.rs`: `cavern_floor_at`, `cavern_level`, `cavern_tree`,
  `fell_cavern_tree`): once a cavern is breached by the stair, a camp needing logs with no tree
  within 30 cells of the fire fells the cavern's fungus trees (within 30 cells of the stair, on
  each column's own cavern floor, never under a surface tree): two logs carried up to the mouth
  (the first a moment, "Wood from the dark"); the cavern's hunters, roaming its floor, may find
  the feller alone (`cavelife.rs`; the old one-in-eight roll is gone). Fish too: at the breach `find_cave_fishing` lists floor beside the
  cavern's water within 60 cells of the stair (`Colony::cave_fish`); with no water to fish above
  (or it is ice) and food short, fishers go below ("fishing the still water of the first cavern,
  15 levels down"; the catch carried up to the mouth; "the first blind white fish").
  `PLANET_FORCE_CAVERN=1` sends fellers and fishers below whenever a cavern is open (the dev
  camps have both near; seed 23 fishes below from day 21). The long climb means meals and sleep often cut
  a trip short. Tested (`fungus_trees_are_felled_in_the_cavern`).
- Furniture (`furnish_option`, `finish_furniture`, `Room::furnished`; DF's beds and tables make
  a dug space a room): by day one maker at a time (a hand who likes making things, or the camp's
  builder) makes the great hall's long table and benches (3 logs, else 3 dressed stones) and a
  bed for each owned bedroom (1 log or stone), "Making furniture: ... at the workshop". Until
  then a bedroom is a straw pallet (no `Feel::OwnRoom`) and meals are not eaten in the great
  hall. Drawn: bed or pallet; the table only once made. Dev 45,12: the first bed day 38, the
  table day 71.
- A dig no one can reach is given up (`give_up_dig`): forty failed ways in a row (`dig_fails`,
  reset by any cut made) strike the work off ("They give up a hall in the hill: no one can find a
  way to the rock they meant to cut") and its first cut is kept in `digs_given_up` so the same
  dig is not planned again. On dev 50,20 one hall cut out of reach had been retried by everyone
  every quarter hour (33,000 failed searches; 150 days in 177 s, now 13 s). The fellers' tree
  search runs once a decision (the cavern-wood change had made it twice).
- A ditch round the wall (`delve.rs::plan_moat`, `ProjectKind::Moat`, DF's moats and channels):
  with the palisade closed, a chapter of trouble past and day 30+, the ring two cells outside the
  palisade (radius 13) is cut down two levels, column by column (its surface cell, then the one
  under it), with three-cell crossings left at the four gates and the cells beside them only a
  level deep (steps out of the ditch). What walks the surface cannot climb two levels, so raiders
  and beasts come in by the gates, where the cages stand. Readiness +0.08, "the palisade closed
  and ditched" in the tally. Cells by a building or on forbidden ground are left (gaps a raider
  may find). Six dev seeds: ditches done on days 30-113. The surface ink shades ground two or
  more levels below its neighbours (`local_ink::View::new`). Lots for buildings now fall back to
  24 cells from the fire when 14 is full (the ditch takes the ring at 13; the lord's hall had found
  no lot).
- Tree search: `nearest_tree` looks the whole map over once a day and makes no search on a day
  with no tree to fell anywhere; otherwise it searches as before (a per-searcher "none found"
  cache was wrong: the search is within the work radius and skips claimed trees).
- Picking in the window (`Colony::settler_at`): in the surface view only those not below, in a
  level view those on that level (and surface settlers near it), for the hover chip, the title,
  clicks and the patron's verbs.
- The deep shaft is walked (`ProjectKind::DeepShaft` is a dig now, `plan_dig`): the stair cut on
  down from its foot to level 4; a stair cut into open dark lets a stair down to that cavern's
  floor at once (`let_down_stair_pub`), so it passes through the second and third caverns (each
  a breach with its life and moment; a breach no longer clears a dig that is not the mine). Done:
  `deep_shaft_done` as before (adamantine one camp in two with three layers, the hollow), and
  the magma: the rock over the magma sea is warm, a forge is set over a vent (`magma_forge`, "The
  sea of fire", a moment), and metal spears and armour made after are "magma-forged" (x1.12
  force, +0.05 cover). Dev 76: second cavern day 70, third day 85, adamantine and the sea of fire
  day 87. Tested (`the_deep_shaft_opens_the_hollow`, `the_deep_shaft_reaches_the_magma_sea`).
- `PLANET_FORCE_LEADER_FALL=1` lets a routed war band's leader fall whatever the blows (the test:
  led bands raid once or twice in 300 days). Chance tests that moved with the ditch now try two
  or three seeds (pet, caravan ambush).
- Workshops below (`ProjectKind::Workshops`, `RoomKind::Workshop`, DF's underground
  workshops): with the workshop standing, from day 40, for a stone-first people or after 20 works,
  a level off the stair gets a 5x5 room with two benches (`plan_level`); `workshop_spot` then
  returns its bench, so crafts (and a mood holding the workshop) go below. Dev 76/3/11: dug on days
  40-57; seed 76 still makes 158 works in 200 days. Drawn with benches and named.
- The farm under the rock is dug (`ProjectKind::CaveFarm` is a dig now, `RoomKind::Farm`): a 6x4
  room of plots off the stair (`plan_level`), planned only when no other dig is under way; the
  harvest is as before (every eighth day, carried up to the mouth). Drawn as rows of pale caps.
  Seed 3: dug on day 31, on the same level as the workshops below.
- Room value (DF): furniture has a quality from its maker (`Room::quality`, as for works:
  the building hand, sure hands, care, luck), and engravers carve every dug room's walls in turn
  (`engrave::bare_walls`: the hall's, then the great hall's, the owned bedrooms', the tombs';
  `Engraving::z`; drawn on their level by `draw_delve`, the hall's also on the surface view).
  `room_value` = 2 + the bed's quality once furnished, + 1 + quality for each engraving on its
  walls; `Feel::OwnRoom { value }` grows with it ("slept in a fine / splendid bedroom of their
  own"). Dev 76 and 3: the great hall is carved from days 221 and 154.
- The hatch (`ProjectKind::Hatch`, `delve.rs::seal_caverns`, DF's walled-off caverns): after the
  cavern's hunters have hurt someone twice (`cave_bites`: bites by those come up the mine, and
  fellers set upon in the dark), the camp sets a hatch in the stair at the first cavern's roof
  (6 loads, urgency 2.0, a moment): no creature's walk passes it (see "Creatures in three
  dimensions"), and later breaches below it set no new `cave_hunter`; `Colony::hatch` (column, level), drawn as barred planks. Seeds 76, 23, 58, 3, 5 sealed it (since caverns line up across embarks: 5 on day 47, 23 day 88, 76 day 101; seed 3 misses the caverns)
  on days 48-107; nothing comes up after. Tested (`the_caverns_are_sealed_with_a_hatch`).
- The lord's quarters (`nobles.rs::lord_quarters`, DF's nobles' room requirements): a lord takes
  the best bedroom (its owner is moved to the lord's old one, or none, and resents it: a grievance
  and -3), wants it worth 8 (`room_value`), and its bed and walls come first (`unfurnished`,
  `bare_walls`); the camp's builder carves them by order whatever their taste. Unmet 30 days on,
  every 20 days "finds the lord's rooms mean" (the lord likes everyone less); met, "is pleased
  with the lord's rooms" once (+1). Seed 23: Baangh takes Snaazo's room day 90, pleased day 126.
- The mine follows the draught (`delve.rs::mine_toward_cavern`): when no cavern lies under the
  stair, it goes down to a level above the floor of the nearest first-cavern column within 40
  cells and cuts a gallery along rows and columns to it (never through rock a place has already
  opened), breaking in from the side; else straight down as before. The farm under the rock comes
  before the comfort digs (`delve_candidates` returns early, but for the hatch, while a farm could
  be planted and is not). A cut is made from the level above only on open ground (a digger on the
  stair had cut a gallery's first cell a level early and left the next out of reach), and floors
  that places opened are never ground known to be walked (`cut_stand`). Seed 11 had starved
  and left its land in winter with its mine missing the caverns; now it breaks in on day 35,
  digs the farm next, and holds out. The still's spot is open ground beside its posts (brewers
  had walked to a post). `PLANET_DEBUG_DIG` also prints why a dig is given up.
- Reclaiming (DF): fallen halls found within 40 cells of the fire become the camp's great hall
  if it has none (`delve.rs::reclaim_halls`: the halls' busiest level, "the old stone table",
  furnished; a moment), and engravers carve its walls. A great hall is eaten in only when it is no
  farther than the fire plus 10 cells (`eat_spot`). Seed 3 camped on tile 27,9
  (`PLANET_FORCE_CAMP=1`) takes the halls of Slitash on day 11.
- Drawbridges (`ProjectKind::Drawbridges`, `delve.rs::build_drawbridges`/`set_bridges`, DF's
  raising bridges): with the ditch dug and a siege come (or three chapters of trouble), the four
  crossings are cut down two levels and spanned by a timber deck at the ground's level
  (`Colony::bridges`: cell and deck level). A siege raises them (deck taken away, the column's
  ground drops to the pit: walkers and raiders alike cannot cross) once nobody is outside the
  ring, its end or a sally lowers them. Raised: readiness +0.12 ("ditched and its bridges
  raised"), and the siege lifts at readiness 0.75 instead of 0.85. Six dev seeds build them on
  days 59-214; seeds 3 and 58 raise them in a siege (58's lifts).
- `PLANET_FORCE_BRAWL=1` lets any dislike at the tavern come to blows without the roll (the test:
  hot-tempered drinkers who dislike someone have grown rare in the camps).
- Industries (`industry.rs`, DF's production chains; type x material): five workshops cut off
  the stair as 4x3 rooms (`plan_level` tag 400; `ProjectKind::{MasonShop, CarpenterShop,
  Smelter, Forge, Kiln}`, `RoomKind::{Mason, Carpenter, Smelter, Forge, Kiln}`, planned in
  `industry_candidates` from `delve_candidates`): a smelter when ore lies waiting (1.6), a forge
  once the smelter is dug (1.5; none needed over the magma), a mason's from day 45 after 40
  stone loads (0.55, 0.9 for masons), a carpenter's from day 45 after 40 trees felled with a
  still or two unfurnished rooms (0.5), a kiln from day 60 with 6 loads of clay or sand dug
  (0.45; the ditch digs most). The stock is `Colony::industry` (ore, bars by metal, charcoal,
  blocks, barrels, clay, sand, smith skill per settler, forged tools), not `ItemKind`. Jobs
  (`industry_option`, `Job::Craft` with a why prefix, dispatched by `industry::is_industry` to
  `finish_industry`; one hand a shop; x0.6 while the store is under its goal): dressing stone
  (a stored stone -> 2 blocks, 150 min, to 12 blocks), a barrel (a log, 150 min; three kept,
  more for trade with 6 spare logs), burning charcoal (a log -> 1, 120 min, while ore waits),
  smelting (an ore + a charcoal, none at the magma -> 2 bars; adamantine a wafer a strand; 240
  min), tools (2 bars of iron or copper, once: `iron_worked`, which is now forged or bought
  tools only), forging a spear or mail of metal to replace a wooden or leather one, firing (2
  clay or sand + a log -> a pot or glass work, while under 3 unsold). Ore: each ore cell dug is
  a load (the stone load still drops), an old mine gives 6, the deep vein 6 adamantine strands,
  the liaison's "loads of iron ore" 8 (asked when the smelter stands cold and the town holds
  iron; "sacks of charcoal" 10 when ore waits and no wood is in reach). Links: furniture takes
  blocks first ("granite blocks"), made at the carpenter's or mason's (+0.1 quality); spears
  and mail of metal take 1 / 2 bars at a forge (`arm_metal`; `militia.rs`/`armour.rs` only
  consume, force and cover unchanged), made by the best smith (`smith_skill`: half the building
  hand at first, +4% of the rest a piece) whose hand sets the head's force (0.9 + 0.2 x skill)
  and quality word ("a superior iron-headed spear"); the still brews a cup more per barrel
  (three at most); caravans buy blocks past 6, bars past 4 (4 each; never adamantine), barrels
  past 3. `--sim-projects` prints "Industry: ...". `PLANET_FORCE_ORE=1` (`=N`: the first N; eight all go into spears and mail, so the sale is checked with 30) makes the first eight
  rock loads dug iron (no dev seed strikes ore): dev 76 strikes it day 12, smelter day 17, first
  bars day 25, forge and iron tools day 26, an iron spear day 30, bars sold day 45. Unforced, six
  dev years: mason's on all six (days 47-237), carpenter's on five, kiln on four (pots and green
  glass), smelters for the deep vein's adamantine on four (wafers at the magma, no fuel), 13-37
  stones dressed. A dev year ~10% more CPU (seed 76 11.2 -> 13.0 s, 11 14.8 -> 16.4 under load).
  Tested (`ore_becomes_bars_tools_and_iron_spears`); `the_camp_keeps_a_library` tries seed 76
  too (the shops' digs put seed 11's library at day 120, seed 3's past its first year). Also fixed on the way: a meal in a hall below
  from 60 cells out exceeded the path budget and two of seed 11's foragers starved with 90
  meals stored (Eat now falls back to the fire, like Sleep and Build); a slain or caged
  forgotten beast is not woken again by a later breach (`wake_the_deep`).
- Artifacts set in place (`delve.rs::place_artifacts`, `Colony::placed`; DF: an artifact makes a
  room): at dawn each artifact not yet placed goes to the great hall (a throne to the lord's room
  if there is one), on a free cell away from the table; a stolen one leaves its place. Each adds 10
  to its room's worth; drawn on a gilded plinth with its name. Dev: Dreamlight (76, day 169),
  Rumoor (5), Shimmerglade (11).
- Creatures in three dimensions (`creatures.rs`, `cavelife.rs`; DF: each cavern layer spawns its
  own population, design guide ch. 11): `Creature` carries `z` (None: a surface walker on the
  old column walk, unchanged), `path3` (followed first, `cost3` steps, x1.4 diagonal, +6 on a
  stair), `home_z`, `out`, `rest_until`; `creature_here3` / `creature_below`. The surface view
  draws only creatures not below; level views (`draw_delve`) draw those below on their level
  (faint a level off), hunters and beasts named. `PLANET_FRAMES` writes `_caverN.png` at each
  breached cavern's stair foot. New kinds `CaveHunter`, `CaveLife` (end of the enum).
  - A breach (`populate_cavern`) sets 2 hunters and 3 harmless (the life list's walkers; fish stay
    in the water; 15 at most) in dens on that cavern's floor 8-36 cells from the stair's foot
    (`Colony::cavern_feet`, a walk outward over its floor, 4000 places). Hourly they amble 2-4
    steps on their own floor (no search), back toward the den past 12 cells.
  - Every 20 minutes a hunter at home notices (1 in 4) someone at work alone on its cavern's floor
    within 10 cells (path3 chase, 3000 nodes; a failed chase waits an hour); adjacent: ill a day,
    `Feel::TheDeep`, the felling or fishing dropped, "X is set upon by a pale spider while
    fishing in the dark of the first cavern, and comes up the stair bleeding", `cave_bites` +1,
    and that cavern's hunters keep to their dens 6 days.
  - 20:00 one night in three (as before) the two hunters nearest the surface climb the stair to
    the mine's mouth (20,000 nodes; the second shares the first's way), hunt the surface as
    wolves do (lone, 8+ from the fire, unroofed, a pet may stand over its keeper; "come up from
    the mine, alone and far from the fire"), and walk back down at 06:00 (or after a bite).
  - The hatch is a real barrier (`nav::path3_barred`: no creature walk stands on it). It now sits
    where the way up from the shallowest cavern first climbs the stair (`hatch_level`; dev 76's
    first cavern lies beside the stair, and the old roof formula had put it under the second
    cavern, level 26 instead of 40). Setting it sends any hunter up for the night home.
  - What the deep sends (`deep_comes_up`) starts on its cavern's floor 10-20 cells from the stair
    (the beast's own layer if breached, else the deepest breached), or for the demon at the deep
    shaft's foot, and walks up the stair (it bursts the hatch): "Something stirs in the first
    cavern, 15 levels down: Gru the forgotten beast is coming up the stair" at 19:00, "Something
    climbs out of the mine" (the moment) when it reaches the surface, ~3 hours later. It clashes
    with a settler within 2 cells and a level of it (on the stair, in a room off it) as on the
    surface; it goes back down when the raid ends. With no way up, the old spawn at the mouth.
  - Seeds 3/76/23 (120/365/150 days): 3/2/0 bites in the dark, 2/3/4 on the surface (seed 23: a
    pet stands over one); hatches days 30/90/84 (base 38/88/82); seed 3 with
    `PLANET_FORCE_CAVERN` 5 bites in the dark in 60 days. Gru (dev 45,12) climbs out at 22:05 on
    day 29 (was spawned at the mouth 19:00). `a_slain_beast_becomes_bone_and_armour` runs 125 days
    (Gru now falls on day 107, its bone worked from 111; seed 11's bone figurine shifted out).
    Cavern life costs ~2% of a dev year's tick (`PLANET_DEBUG_CAVE` prints dens, climbs, the
    ways up past the hatch and the time spent).
- Speed, same results (2026-10-08; the colony log is byte-identical on dev 76, 11, 3 and 50,20 at
  300 days). User time of `--sim-projects` (world and history included, ~1.2 s): seed 76 300 days
  8.2 -> 5.8 s, a year 10.7 -> 6.8 s, 11 9.4 -> 7.2 s, 3 9.3 -> 6.5 s, 50,20 150 days
  16.2 -> 4.3 s, 300 days 26.7 -> 7.5 s (instructions 152G -> 92G, 525G -> 114G). How:
  trees are looked for only among the columns that held one at founding (`tree_buckets`,
  `nearest_tree_by`, `nearest_listed`: 16-cell squares, nearest first, same least
  (distance, row, column) as the rings; nothing else can ever grow a tree, see the field's doc);
  shrubs beyond 12 cells likewise (`shrub_buckets`); the ring search walks each ring in row order
  and stops at the first cell that passes, testing the map before the hashed claims;
  `claimed`/`unreachable`/`shrub_ready`/`opinions` use `det::FastMap`/`FastSet` (FxHash; only for
  maps never iterated); the A* (`nav.rs`) keeps its arrays between searches (generation marks),
  remembers each cell's standable/cost within a search (`Memo`: dense on the surface, a small
  table below it), asks each side once per node, and pops from buckets by estimate (`Open`, the
  same (f, k) order as the heap); `decide` borrows the settler instead of cloning it;
  `find_site_within` marks taken cells once per call; engravings phrase only moments that could
  win; `temple_at` skips reading the god. Still most of the time: the A* itself (~45%, ~300
  expansions a search underground), then `decide`'s options.

## Materials and blows (2026-10-08, DF's items: geometry on the item, physics on the material)
- `src/materials.rs` + `data/defaults/materials.json` (loaded like monsters.json; pure, no RNG)
  replace the flat spear forces (1.05-2.2) and armour covers (0.25-0.75). Materials: density,
  `hard` (shear yield), `edge`, `resist` (edge cost/mm), `absorb` (blunt loss/mm), `fracture`
  (rigid crack cost/mm), `rigid`, flags `weapon`/`armour` (DF's capability flags): the land's
  stones (flint 40 hard / edge 0.9, granite/basalt 35, sandstone/limestone 25), copper 45, iron
  70, adamantine 1000 (density 0.2), bone, wood, leather, beasts' coverings (fur, hide, chitin,
  scales...) and substances (granite plates, smoke, salt...), tissues. Weapon subtypes (spear,
  fishing spear, stake, axe, mallet, hammer; raiders' sword, mace, club, long knife): edge/blunt,
  contact, penetration mm, head cm3 of the material + haft cm3 of wood, velocity. Natural
  weapons (jaws, tusks, horns, mandibles, talons, tentacles): material (tooth, ivory, horn...,
  or the beast's substance if weapon-flagged: an iron beast bites with iron), force x size^1.3.
  Bodies: settler head/body/arm/leg, raider arm/shoulder/leg, beast head/flank/leg/wing/tail/
  tentacle/shell/horn, as skin/fat/muscle/bone mm (settlers x toughness, beasts x size; a beast's
  covering or 8 mm/size plates of its substance on top); armour subtypes are a layer over body
  and arms (mail 1.6 mm, scale 2, jerkin 4, coat 6, plate coat 4; x quality).
- `strike(blow, layers)`: momentum = 100 x strength (attr/1000 x (1 + 0.5 drill) x the maker's
  hand 0.9-1.1 x a hashed 0.85-1.15) x velocity x sqrt(kg). An edge bites a layer only if 1.5x
  as hard (else it lands blunt from there); cutting costs resist x mm x contact / keenness x
  (layer/edge hardness)^0.215. Blunt: soft layers absorb, rigid ones must crack. Outcome by where
  it stops: Glance (the outer rigid covering: "glances off the chitin of its shell with a ring",
  "glances off its horn"), Turned (armour), Bruise, Cut, Deep (60%+ of the muscle or to the
  bone), Broken (blunt past the bone's fracture, or an edge's leftover past 2x it; 3x under
  armour, which spreads the blow): "drives a flint-tipped spear into the raider's leg, and the bone
  breaks", "breaks/cracks its head with a mallet". Harm = 1.75 x the share of the part's
  resistance gone through (bone 5% of its cost to an edge; 1.2 at most); glances do none.
  Severity on settlers: bruise 1, cut/deep 2, broken 3.
- In the clash (`fight.rs`: `Foe`, `foe_of`, `body_of`, `blow_harm`): defenders' blows, the
  crowd's half blows, the rescue wound (struck with and without the victim's armour: "does not
  get through", "though X took the worst of it" when it is a step lighter), the counter-bruise
  (a half blow, turned when the armour stops it), the killing blow (`armour_turns_killing`: head
  or chest at 1.5x, turned when held to a bruise), expeditions (`blow_harm` over the beast's
  parts; danger x(1 - `armour_guard`), the share of the beast's blows the armour lightens). Arms
  and armour keep a material and quality (`Arm::rating`, `Armour::rating` for handing out);
  magma-forged metal x1.12 momentum, armour x1.15 thickness. A beast's coat is made only of
  armour-flagged remains (seed 3's Kaelgar's salt is no longer worn).
- What it gives (strength 1.2): flint spear vs a raider bites deep (harm ~1.3), vs a size-2
  furred beast cuts (~0.6), adamantine bites deep (~1.2) and cuts through granite plates that
  every stone and iron head glances off; iron mail holds a raider's iron sword to a bruise
  (iron cannot bite iron), leather only lightens it; size 2 jaws gash, 2.5 tusks break bones;
  chitin beasts glance off sandstone (seed 5's Skorvurr: 5 glances in 300 days).
- Balance, six dev seeds x 300 days (`PLANET_DEBUG_BLOWS=1` prints each beast's harm against its
  need): beasts slain at the camp 2/2/0/1/2/0 (76/11/23/58/3/5), the same as before, plus seed
  3's hunting party taking Zarnak on day 155; raid deaths 8 (was 7: seed 3 3, a moved timeline);
  armour turned 5 blows (was 4). The log has fewer "bruises" (stakes now cut) and broken bones
  on raiders where a strong drilled hand drives the spear home. `cargo test materials -- --nocapture`
  prints the whole table.

## Minds that steer what happens (2026-10-08, DF's personality needs)
- The complaint: every embark did the same thing and built the same things in the same places
  (six dev seeds: palisade, second hut, woodpile, traps, field, rack, well, storehouse, workshop,
  mine, in nearly that order, inside the same octagon of radius 11 with four gates on the axes).
- Needs (`needs.rs`, DF's `personality_needst`: 30 needs, focus to 400 when met, falling by
  need_level): each settler rolls up to 20 needs from facets and values (company from
  gregariousness, prayer from piety with a faith, the sight of beasts from nature, beautiful
  things from artwork, a walk alone from curiosity and independence, excitement, helping someone
  from altruism, something new to learn, time to think, merriment, the old ways, arms practice, a
  craft, something new to make, work, a drink (every dwarf), a good meal), strength 1-10. Focus
  falls by the strength each hour (`needs_hour`); things felt meet needs wherever they happen
  (`needs_from_feel`: a festival, prayer, a cup, the cook's supper, a song, a work made, a friend,
  a pet, a hunt, news...); work meets "work". `focus_of` (DF's current_focus) sets the work pace
  (x0.94 focused .. x1.06 distracted, `focus_pace` in `start`); a need badly unmet (-250) is a
  thought at dawn ("has gone 9 days without prayer").
- Between jobs only (`decide`, `free`), never when the camp goes hungry, a need well past due
  (-100) is an option (`need_option`; 0.15 + deficit x (0.2 + 0.06 x strength), +0.15 in the
  evening: below most work until a need has gone very long unmet; at 0.2 + 0.3 it had stalled a
  camp's building for months), x0.4 when the store is under half its goal, x0.5 for leisure under the no-idle-hands
  mandate, x0.75 with trouble foretold): a `Job::Wander` to a place with a reason, kept in
  `Settler::need_act` and met when it ends (`complete_need`): talking with the dearest awake
  settler where they are ("by the woodpile", `place_word`), time with a spouse or child, praying at
  the temple, the standing stone or on the highest ground near (`high_spot`), taking it easy by
  the water or under trees, watching the nearest herd or a pet, admiring an artifact in its room,
  an engraving (on its level, `spot_level`) or the best work at the fire, walking out alone 12-25
  cells toward the settler's own direction of the day, climbing the lookout or the high ground,
  sitting with the ill or hurt (else lending a builder a hand), reading at the library or
  watching the best hand at work (+0.01 skill), sitting with their thoughts on a rise, singing or
  telling their people's tales at the fire in the evening, practising at the drill ground,
  whittling by the fire where there is no workshop. Six dev seeds, 120 days: 120-460 acts a camp,
  each camp its own mix (23 drills and climbs, 3 sings, tells tales and prays, 11 watches beasts
  and kneels on rises). The settler page (`Colony::about`) says "Needs prayer (unmet 4 days),
  company...; distracted". `PLANET_NO_NEEDS=1` turns the acts off to compare.
- Voices (`voices.rs`): each grown settler's voice for a work kind (`voice`: the unmet needs it
  would meet, or a trait: the anxious for the wall, traps, ditch and lookout, the greedy and
  curious for the mine, the orderly for the storehouse, the bashful for bedrooms, the
  family-minded for food works); the camp's support is the mean, the speaker's twice. Candidates
  rise with it (x1 unheard .. x1.75, `weigh_voices`; nothing is lowered: a field nobody loves
  still feeds them) and the loudest voice (0.55+) is named in the plan ("Nawyth presses for it:
  he cannot sleep for fear of what comes"). A strongly pressed temple, tavern, workshop, library
  or pen comes before its usual time (`pressed_candidates`): seed 11's drinkers build a tavern on
  day 35 (was 106). `PLANET_NO_VOICES=1` plans as before.
- Places by purpose (`projects.rs`: `find_site_for`, `purpose_cost`, `near_things`): the temple
  and the lookout seek high ground, the workshop the timber, the smokehouse and the rack the
  fishing, the store, kitchen, still and second hut the hut, the field deep soil, the pen the herd,
  the library quiet, the tavern the fire. Lots keep off the wall's ring (wholly within 1.5 inside
  it, or wholly 4.5 beyond it, past the ditch; a hut on the ring had left a gap in it); fields and
  pens always lie outside the wall. The camp's own shape (`camp_spread`, `wall_r`): the founders'
  mean gregariousness and orderliness (about 50-65 on the dev seeds, centred at 57) scale the
  distance from the fire (x0.6 .. x1.4) and set the palisade's radius, 9 (close, orderly) to 13
  (independent); the gates, cages, ditch (`wall_r + 2`), drawbridges and siege bounds follow it.
  Dev seeds: radii 9 (11, 23), 10 (3), 11 (76), 12 (58), 13 (5). `--sim-projects` prints
  "Layout: wall radius N, spread X; M spare-hours acts".
- The ditch's trap (found on the way): a stretch of ditch closed at both ends by cells left
  uncut (by a building, wet or forbidden ground) had trapped its digger, who starved with 50
  meals in the store (seed 23). Every ditch cell beside an uncut one is now dug only one level
  deep, a step out, like those at the gates (`plan_moat`).
- Nine camps x 120 days: 14 deaths (16 before). Tests: `settlers_spend_spare_hours_by_their_needs`,
  `the_camp_presses_for_what_its_people_want`, `camps_lay_themselves_out_differently`. Moved:
  the bard may come "to the tavern"; Gru may meet someone on the stair (the raid "out of the
  mine"); gems at 120 days; news on seed 3; jaded, tombs on seed 1; suppers on 5, 1, 23.
- Haunts (`haunts.rs`): a need met out of doors (prayer, rest, time to think, a walk alone,
  whittling, the old ways) is met again where it was first met (`haunt_of`; the act's why says
  "at her cairn" / "at 111,103, where she always goes"); on the third visit the settler leaves a
  mark there (`MarkKind::{Cairn, Bench, Carving}`, or a raised stone for the old ways; inked as
  a heap of three stones, a plank on legs, a notched post; hover and click as any mark): "Grang
  raises a cairn of fieldstones on the rise at 111,103, where he prays to Heleleon". A newcomer
  whose spot lies within 5 cells of another's place of the same need shares it ("Ord adds a stone
  to Grang's cairn, and prays there too"; +1 opinion), and the devout without a place of their
  own pray at another's cairn one day in two (`shared_haunt`). Six dev seeds, 120 days: 7-15
  such marks a camp, no two camps alike. `Colony::haunts`.
- Talk (`talk.rs`, DF's conversations): a talk has a topic chosen when they sit down
  (`talk_topic`, in the why): a moment both lived through in the last 20 days (raids, deaths,
  births, weddings, festivals, masterworks...), a home both name in their callings, a value both
  hold dear, or small talk; where a value pulls them apart (both 20+ apart in sign), an argument,
  likelier for the quarrelsome (discord, anger). `talk_done` when they get up: agreement and a
  shared home +2 both ways; shared grief +2 and comfort; an argument -3 both ways, a quarrel
  thought when either is angry, said aloud once a pair in 20 days ("Snokh and Graadrozz argue
  under the trees about cunning: Snokh has no use for cunning; Graadrozz holds it dear, and voices
  are raised"). Idle hours become talk (`idle_talk`: with nothing to do, sit with the dearest
  idle, eating or resting settler within 12 cells; 0.06, 0.3 in the evening), so a camp talks
  90-390 times in 120 days. Spear-bearers owe the evening drill first (`drill_due`: no need act or
  idle talk then; drills had fallen a third and no hunting party went out).
- Gates where the paths go (`traps.rs`: `choose_gates`, `gate_dirs`, `gate_point`): when the
  palisade is begun the camp chooses its gates on the eight compass points, toward the water (the
  lane), the nearest timber outside the wall and the road to the trading town, a quarter turn
  apart at least; 2 for an uneasy people (founders' mean anxiety 58+), 4 for a bold one (under
  45), else 3 ("They will leave 2 gates in the wall: north toward the water and south-west toward
  the road to Swanworth, and no more: they are an uneasy people"). The palisade leaves the gaps
  there, the cages stand there, the ditch's crossings are where each gate's way meets its square
  (a corner for a diagonal; `crossings`), with steps beside, and the drawbridges span them.
- A work set with a gem is always logged. Tests: `settlers_make_places_their_own`,
  `settlers_talk_and_argue`, `gates_face_where_the_paths_go`; moved: risings on 58/5, the pyre on
  58, the taught work on seed 3, spare bars sold on day 105.
- Where the camp is founded (`Colony::found_by`, `Leaning`; `viewer::found_colony` passes the
  roster's personas): within 30 cells of the embark's middle, the founders' characters weigh the
  flat open ground: lovers of the wild want water near (`water_distance`, x2 a cell) and the
  woods' edge (trees within 8 cells), the anxious and proud and dwarves high ground (x6 a level);
  the founding line says what drew them ("7 settlers make camp at 91,66 ..., close to water").
  Thirteen seeds: camps from 82,84 to 94,112 (all had been within a few cells of 96,96 where the
  middle was open). `found` without founders is as before.
- The woods gate faces the eighth of the land beyond the wall (to 25 cells past it) with the most
  trees, when it holds 30+ and half again the mean (the nearest tree was usually inside the wall).
- Duty before needs (`drill_due`): spear-bearers at the evening drill, and whoever has someone
  to tend (`tend_option`). The healer hurt is tended by the most empathic other grown hand (a
  healer had died of a broken leg's fever no one tended; on `main` too). Outings are shorter
  (walks 12-25 cells, rises within 15-18, water within 20, herds within 30) and arguments rarer
  (8% + a fifth of the speaker's heat). Thirteen seeds x 120 days: 14-16 dead or gone with the
  spare-hours acts, 15 without (`PLANET_NO_NEEDS`); they had been 22 against 13.
- Tastes and rhythm (`rhythm.rs`): `taste_of` gives each settler's pull toward each kind of work
  from a little luck (by name) leaned by character (lovers of the wild forage and fish, the
  patient fish, the strong fell and quarry and lovers of the wild do not, the orderly haul,
  craftsmen and the dutiful build), set when the roster's personas are given and for every
  newcomer (`add_settler` had cloned the first settler's tastes onto all of them). Each settler
  rises at their own minute (`wake_minute`: 5:00 for the hard-working .. 7:30 for lovers of
  leisure) and lies down at theirs (`bed_minute`: 20:00 .. 22:30, the immoderate and
  thrill-seeking late); `abed` replaces the clock's night for sleep only (night work rules are
  unchanged). Dev 76: the first acts of the day spread from 04:00 to 08:00.
- Grief and courtship (`needs.rs`): `mourn_death` records who mourns whom (kin, or opinion 12+;
  `Colony::mourning`); for twelve days after, by day, once a day, they stand at the grave
  (`mourn_option`, 0.3 + 0.4 x love, +0.2 kin; before any other spare-hours act):
  `Feel::Remembered` eases a little. DF's MakeRomance: `Need::Romance` (romance value, love
  facet) walks out with an unwed settler of their people and the other sex whom they are fond of
  (opinion 10+, and 4+ back): +2 opinion both ways, so courting couples reach the wedding's 20.
  Eight seeds, 120 days: 2-5 weddings each, 3-19 walks.
- `PLANET_FORCE_HORROR=1` counts each horror three times (the jaded test: camps that keep their
  people alive no longer see eight). `PLANET_FORCE_PET_PREY=1` now lets night hunters take a
  pet's keeper wherever they are on the surface (keepers keeping their own hours were never caught
  alone).
