# History simulation (`src/history/`)

Use `crate::history::det::{HashMap, HashSet}` for any map iterated while drawing from the RNG (see the root CLAUDE.md).

## Speed
- `--history-profile` times every phase of `simulate_step` (`step::profile`, off unless enabled).
  Seed 42 at 512x256: 250 years in ~7 s of history (ecology 3 s, the Shadow 2 s, trade 1 s).
  Trade routes used to take 112 of 118 s: their A* asked the river network (a scan of every
  segment) for each neighbour; `static_road_costs` now computes the unchanging part of the road
  cost once per world, with identical results (the dev journal is byte-identical).

## Causes
- `Chronicle::record` gives an event recorded without a cause one by an explicit rule per kind
  (`infer_cause`), from two unsaved indexes (`last_between` two peoples, `last_of` a participant):
  quarrels, war declarations, treaties, alliances, assassinations and conversions follow the last
  dealing between the two peoples; trade routes the peace they open on; a beast's raid its lair
  or previous raid; a quest its target's last deed (quests name their target); a colony its
  mother town; forests felled, game scarce and wolves in ruins their town. Sieges are caused by
  their war, siege outcomes by the siege, crownings by the predecessor's death. Conversions need
  a neighbouring people of the faith (its missionaries) and name it.
- `--causes` prints the share with a cause and the chain depth: dev 74-78% (median chain 6-7
  links), seed 42 74% (median 5). Was 25% / 15%.
- `Chronicle::get` is indexed (`by_id`, not saved; a loaded history falls back to a scan).

## Consistency and the present day
- `step_integrity` (end of every `simulate_step`) is the one place that reconciles what other
  systems leave behind: wars and sieges with a dissolved side end; an active people whose ruler is
  dead (any cause) gets `succeed`; each pair's `War` stance is re-derived from the active wars (the
  old bookkeeping left allies "at war" forever, which filled the 2-wars cap and made history go
  quiet after ~year 300); grudges fade a point a year at peace. `succeed` does nothing for a
  dissolved people.
- `simulation/invariants.rs::violations` lists contradictions (a people destroyed more times than
  founded or acting after its end, dead rulers ruling, crownings while the predecessor lives,
  rulers under 14, figures dying twice, two wars between one pair). Printed after every history
  ("Chronicle: consistent"), tested on a synthetic world (unit test) and on `--dev`
  (`tests/present_day.rs`). The original code had 24 on the dev world.
- War pacing (`WarMemory` in `step_diplomacy`, derived from `history.wars`, nothing saved): a
  12-year truce after a pair's war ends, and each recent war (20 years) halves a people's appetite.
  Wars end by exhaustion that grows each year (most 3-10 years). Fallen peoples rise again
  (`step_revivals`): their founding seat, held by another race, may declare for them (3%/yr,
  15-150 years after the fall), restoring the people with a war of independence. Slain beasts'
  broods rise in their lairs when fewer than a fifth of the starting beasts live (`step_broods`)
  and stay hidden from heroes for 20 years.
- `history/present.rs`: `WorldHistory::present()` = the state the game inherits (wars, sieges,
  grudges with their cause, living beasts and the nearest town, the Shadow's frontier (free towns in its visible reach), disputes,
  heirless rulers, recently fallen towns). `--present` prints it; the summary prints its counts.
  `tests/present_day.rs` pins the dev world: 2+ wars or sieges, 3+ beasts near a town, 5+
  grudges, 4+ peoples, 3+ frontier towns, a war in every half century, a consistent chronicle and
  a byte-identical journal across two runs.
- Names (`naming/generator.rs`): `pronounceable` (3-9 letters for persons, up to 12 for places,
  vowel runs <= 2, consonant runs <= 3, one apostrophe or hyphen) with retries; realms are named
  by government (`realm_name`: "The Kingdom of Galost", "The Bonegore Horde").

## Names in the annals
- Every name in the chronicle is something the world holds. Prehistory reign events
  (`setup::generate_reign_event`, templates in `data/defaults/backstory.json`) are written after
  every people is founded: {ENEMY}/{ADJ} = one of the three nearest peoples founded by then (its
  short name, `people_word`, and it joins the event), {PLACE} = the people's seat or (dealing with
  a neighbour) the neighbour's, {BEAST} = a species of a real legendary beast. Templates with
  {ARTIFACT} or {REBEL} are never used; the stock lists `enemy_names`, `faction_adjectives`,
  `beast_names` are no longer read. Enum names in text go through `step::plain_words`
  ("tin, fish and wood", "a village").
- `tests/flavour.rs` dumps the dev chronicle (`--chronicle-dump FILE`: events and every name the
  world holds) and fails on any capitalised word inside a sentence that is neither part of a
  real name nor a word from the code's string literals or the data files (minus the stock lists).
  It can't see lowercase inventions ("the bronze people"); those came only from the stock lists.

## Treasures and monuments (`history/remembrance.rs`)
- An artifact is made by a real hand (`remembrance::maker`: a smith of the people, the one at
  the seat first, else the ruler) to remember the people's latest memorable deed of the last 40
  years (`recent_deed` / `memory_of`: a battle, a war won, a beast slain, a founding, a crowning,
  a death, a town that held against the Shadow, a treaty...). Its name comes from the maker, the
  town or the deed, the first not already borne ("Agkh's Axe", "the Crown of Skullfang", "the
  Wispsigh Field Amulet"); the description and an inscription cite them; the creation event names
  the maker and is `caused_by` the deed. A monument is raised at the capital for the same kind of
  deed: `commemorates`, `honors` and an inscription are set, named "The Obelisk of Brolmdustoor
  Pass". The stock names are only a fallback (no maker, town or deed).
- The inspector has artifact and monument pages (description, inscription, where it is now, its
  story; what a monument remembers); a town's page lists its monuments and what was made there.
- The summary prints `Objects:`; `tests/flavour.rs` requires every artifact and monument on dev
  to name a real maker or deed, with no repeated names. Seed 42: 483 of 483 and 294 of 294.

## Battles and wars (`step_wars`)
- A battle (15% a season per war) is fought at the defender's town nearest the attacker
  (`battle_town`); its ground (`battle_ground`: stone walls, a river ford, a pass (high or
  rugged), woods, open field) helps the defender (x1.1-1.6) and names it ("the Battle of Ripu
  Ford", "...before the Walls of X", "...of the X Pass"). Each side is led by its captain nearest
  the field, else its ruler (`commander`). Strength = army^0.35 (army ~ souls/25, more for
  martial peoples) x the commander's fixed `talent` (0.8-1.25) x a roll; losses are a share of
  the real armies (loser 5-15% x margin, winner 2-6%). The beaten commander may die (12-24%),
  or escapes with a grudge (opinion -12); a rout earns the victor an epithet (the Victor/Hammer
  of X for attackers, the Shield/Wall of X for defenders; never one already borne).
- A war ends when a ruler falls in battle, when a side's seat is held by its enemy, after 20
  years, or by exhaustion (0.010 + 0.005 per year each season); the victor is the side that lost
  the smaller share of its army. Wars are named by cause and place (`war_name`: "The Salt War",
  "The Conquest of X", "The War of the X Succession", "The Vengeance of X"), with ordinals for
  repeats ("The Second War for X").
- The journal's great wars add a "Remembered for" line: the battle where a commander fell (else
  the first) and the great captain who won most of its battles. The summary's `Wars:` line
  (`invariants::war_report`) prints durations, battles naming a figure, distinct names and how
  peoples fell. Seed 42: 480 wars, 1-20 years (one per world ends within a year when a side is
  wiped out), median 6; every battle names a figure; 474 distinct names.
- `tests/present_day.rs` now averages its present-day bounds over six dev seeds: a single seed's
  story reshuffles whenever a change draws from the RNG, while the six-seed means don't
  (12-seed means after this change: 5.4 peoples, 1.8 wars, 8.3 grudges; the same as before).

## People in places (`history/people.rs`)
- `history.people` (`People`, saved as the world file's `people` field, `WORLD_FILE_VERSION` 6;
  v5 files load without it) gives every living figure a home town and a role: rulers live at
  their seat, others at their people's seat until moved. Each spring towns gain notables
  (`Captain of X` from 300 souls, a priest from 800 where the people keeps a faith, a smith from
  1,500 or 400 with iron/copper), replaced when they die. When a town burns or falls to another
  people its notables flee to the nearest town of their own (`FigureMoved`), go into exile, or
  (captains, 40%) die on the walls (`HeroDied`); each is `caused_by` the fall.
- Heirs and marriages: each spring a ruler under a dynastic law (aged 30+) with no living child
  gets one (role `Heir`, raised at the seat, a figure of the dynasty), and `step::succeed` crowns
  the dead ruler's eldest living child of their people instead of inventing a successor. Heirs
  (16+, unwed) marry a notable of a people that likes theirs (opinion 25+) with 8% a year:
  `EventType::Marriage` (appended), +10 opinion both ways, and the spouse moves to the heir's
  seat (`FigureMoved` "X leaves Y to wed", caused by the marriage). Dev: 11 marriages. The
  Shadow's bane (artifact importance 1000) can no longer be destroyed by chance.
- `people::link_lives` (in `HistoryEngine::finish`) fills `Figure::events` from event
  participants, so the journal's lives of note list their deeds (up to 8). The summary and
  `--present` print a `People:` line. Dev world: 91 of 93 living figures housed, 29 of 36 towns
  with notables, the 220 lives of note average 5.8 events (the journal used to show 82 lives with
  ~0.4 deeds each), ~95 flights.

## Ecology and scarred landscapes (`history/ecology.rs`)
- `history.ecology` (stepped once a year inside the history sim, no RNG) holds per-tile forest
  cover (vs. climax `forest_potential`), farmland, and densities for 9 species (`SPECIES`:
  deer, caribou, boar, aurochs, antelope, ibex as grazers; wolf, lion predators; bear omnivore)
  with biome habitats, logistic growth, predation, hunting and human-intolerance, and spread
  into neighbouring free habitat (migration / recolonisation).
- Settlements press on the land (`pressure`): fields within `0.3 + sqrt(pop/4000)` tiles,
  logging, hunting. Abandoned land regrows. Chronicle events (once per settlement):
  `ForestCleared`, `GameScarce`, `WildlifeReturned` (wolves/bears/lions den in a ruin).
- Anomaly biomes are caused, not rolled: with a history, `Ecology::naturalize` restores
  randomly placed anomaly tiles (`is_caused_biome`) and `update_scars` grows `scars` from
  events, each chronicled as `LandScarred` linked (`caused_by`) to its cause: bone fields where
  6+ battles were fought (battles are located at the defender's settlement nearest the
  attacker), titan bones where huge legendary beasts were slain, ashlands / dead woods /
  crystal woods around long-held lairs of huge beasts with elemental / necromantic / spell
  powers, crystal wastes where towers stood over razed towns, overgrown or cyclopean ruins a
  century after a town falls, and dead woods (where forest grows) or ashlands where the Shadow's
  blight (`shadow::BLIGHT`) lay 30 years (`blight_scars`: radius 2.2, at most 3 a year, caused by
  its latest deed, `ScarSource::Blight` appended; they outlast the Shadow). Dev: 14 of 53 scars;
  the summary prints "N killed by the Shadow's blight" and `tests/present_day.rs` requires one on
  every dev seed. `apply_scars` writes them into `world.biomes` (in `main`).
- Shown: tile viewer draws `Fields`, thinned/felled forests, `Bones` / `TitanBones` sprites;
  hover lists wildlife and the scar's chronicle entry. Embarks (`local/wildlife.rs`, via
  `RegionLore::wildlife`) get game trails (least-cost paths to water), burrows, nests, predator
  dens with bones, and bones on bone fields (`LocalMap::features`).
- The history summary prints an ecology report (forest %, farmed tiles, species vs. start,
  scars by kind, chronicle counts).

## The director: LLM-authored events during history (`history/director.rs`)
- `--director N` (with a simulated history; uses `--bard-model` / `--bard-url`, default
  `gemma4:26b` via Ollama) lets the model author up to N events at turning points. After each
  step the director scores the step's new events by drama (`drama()`: fallen peoples, razed
  towns, succession crises, coups, holy wars, plagues...), paces the budget over the history
  and avoids peoples it wrote about in the last 12 years.
- The model gets a dossier (peoples with voice and faith, seat and its geography, ruler and
  notable figures with temperament, feelings toward others, recent events) and answers JSON
  constrained by `proposal_schema()`: title, 3-5 sentence chronicle text, up to 4 effects
  from `EFFECT_MENU` (opinion, war, peace, alliance, death, crown, marriage, exile, defect, title,
  epithet, artifact, monument, population, wealth, convert), optional thread. Names resolve
  only against the dossier's cast (`lookup`), amounts are clamped, invalid effects dropped;
  effects go through engine paths (`War::end`, `step::succeed` for dead/exiled rulers).
- Threads (prophecy/feud/curse/vow/secret) carry a checkable `Trigger` (years, ruler or
  figure dies, war between two peoples, town falls); when due they are paid off first, and
  the payoff event is `caused_by` the origin. Stored in `history.tales` (`Tales`), saved as
  the world file's last field (`WORLD_FILE_VERSION` 4). Journal: authored events are "Tales"
  entries; a Threads of Fate part lists arcs.
- The model is behind the `Author` trait; tests use a scripted author (no Ollama needed).
- Without Ollama (or with `--director 0`) history is fully procedural as before.

## The Shadow (`history/shadow.rs`)
- A dark power raised at the dawn of history (on by default; `--no-shadow` for sandbox
  worlds; `HistoryEngine::shadow`). `pick_realm` chooses the realm with the most foreign towns
  near its capital (orcs/goblins/undead weigh more); its capital is the seat; the archetype
  (Warlord / Necromancer / Tyrant) follows the race and names the lord ("the Dark Lord", ...).
- Each season (`step`, called from `simulate_step`; no RNG draws, rolls hash seed+season+target):
  a corruption field spreads one tile from its sources (seat 1.0, its towns 0.8, its land 0.35),
  fading by terrain conductance (fast on roads/rivers, slow over mountains, none over water),
  held back by other peoples' towns; reach grows with `strength`. Wild land above 0.55 becomes
  its dominion. Every 12-24 seasons it strikes the town deepest in its shadow: captured or
  burned (`ShadowConquest`) or holds (`ShadowRepelled`; it then leaves that town alone for 40
  seasons). The free peoples' defence follows its last 12 strikes (`rally`: +0.25 per fall,
  -0.5 per town held), which settles near one strike in three held at every map size; each spring
  a town it took may be freed (5%, `liberate`, `ShadowLiberated` caused by the conquest). With
  no town in striking reach it gathers strength (+0.05) so it can't stall short of every town. If its seat falls it is broken
  (`ShadowBroken`) and returns ~20 years later in a new seat. Every deed is `caused_by` the
  previous one, back to `ShadowRose`. Measured 2026-10-04: 27-36% of strikes held on eight dev
  seeds, 29-31% at 256x128 and 512x256; it darkens 15-23% of the land on large maps (20-56% on
  the dev world) and holds 2-15% outright. The `Shadow:` report line prints both.
- Its story has four acts (`present.rs` reads them off the chronicle; `--present` prints them):
  I the Rising, II the First Great Fall (the greatest town it took before the check), III the
  Check, IV the Return. The check (`shadow::check`, at 55-65% of the simulated years, no RNG):
  the four free peoples nearest the seat unite under the ruler of the most populous
  (`ShadowAlliance`, "The Last Alliance storms X"); the champion kills the Dark Lord and dies;
  the seat goes to the champion's people, so the Shadow is broken and returns ~20 years later in
  a new seat. The champion's weapon becomes a real legendary artifact ("the Lance of X") lost at
  the seat (`ShadowBane`): the Shadow's known weakness. History goes on with it (beasts hoard
  it, figures pick it up), so the present day says where it is now ("in the hoard of Golrok
  the Profane (lair at 31,39)", "carried by Aysk"). The present day also names the stronghold
  in its path (the frontier town with the best `defense_strength`). Its counts line has
  "N known weakness of the Shadow"; the present-day test requires one on every seed.
  Checked on dev seeds, 256x128 and 512x256: 1 weakness, 4-29 frontier towns.
- Drawn in three layers (`render.rs::shadow_ink`, `classify.rs`): reach = a cold grey wash that
  deepens with corruption (land only); blight (corruption >= 0.6) = dead trees; dominion =
  cross-hatching (doubled where deepest) inside a jagged ink border; the seat is a black tower
  with a red eye. The watcher has a Shadow panel (lord, darkened/blighted land, towns held,
  fallen, held out) and marks its realm in red.
- Saved as the world file's last field (`WORLD_FILE_VERSION` 5; v1-v4 still load).
