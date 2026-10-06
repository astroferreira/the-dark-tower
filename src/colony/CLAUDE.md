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
  could not reach; why their second-hut beds were cut off is not found yet).

