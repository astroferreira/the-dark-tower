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
- Known gaps: exposure has no consequence yet; a site without trees never gets a hut (dev site
  70,6); after the hut there is nothing to do but keep food up; settlers have no pasts or names
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
  line "(your doing)". Window: F bless and X forbid the ground under the mouse (radius 6),
  G favour and D dream the settler under it; marks are drawn as dashed rings (gold, red
  hatched); the title shows the favour left. `--sim-patron` tries all four on the dev colony
  (`Colony::patron_trial`); `tests/colony.rs` requires each to change behaviour within a day.
  The keys haven't been tried by hand yet.
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
