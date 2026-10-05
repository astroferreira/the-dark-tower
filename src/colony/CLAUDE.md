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
  from the history yet (card "Settlers with pasts").
