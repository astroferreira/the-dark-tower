# Roadmap: from world generator to autonomous colony chronicle

Six big updates take the project from today's world-and-history generator to a playable colony
simulator in the line of Dwarf Fortress and RimWorld. The difference is that it is built to be
*watched*: the settlers live their own lives, and the game's job is to make their story
legible, causal and worth reading. `TODO.md` stays the short-term task list; this file is the
direction.

---

## Where we are (v0.1)

| Layer | What exists | What's missing for a colony sim |
|---|---|---|
| Planet | Tectonics, isostasy, climate, erosion, hydrology, 50+ biomes, resources from geology | Soils, endorheic lakes, a faster detail-erosion pass (see `TODO.md`); good enough to build on |
| Region / embark | Seamless zoomed regions; 192x192 embarks with z-levels, strata, ore, rivers, plants, wildlife signs, towns with streets | Nothing *moves*: no clock, no agents, no jobs, no items on the ground |
| History | 250-year sim at season resolution: factions, settlements, ~300 named figures (personality, family, enemies, skills, life events), wars, religions, beasts, ecology, scars, a causal chronicle (`caused_by`) | Common people are population counts; figures don't plan; colony-scale events don't exist |
| Narrative | Journal (HTML annals), bard (LLM prose), director (LLM-authored events with checkable threads), watcher window | Story is told *after* the fact; nothing picks out arcs while they happen |
| Engineering | Deterministic history (fixed-hasher maps), `--dev` world (96x48, ~1 s), save/load, headless snapshots | Agent-scale performance, the tick loop, tests that pin stories |
| Old prototype | `mystery_box` branch, `src/game/`: job queue, labors/skills, designations, items/crafting, needs, fluids, combat (terminal, Jan 2026) | Diverged from today's world and embark code: mine it for designs, don't merge it |

---

## Time model

The generated history is the world's **past**. It runs up to a "present day" (the end of the
simulated years), and the game always starts there: you embark into the world as it is now,
with the living towns, current wars and the people alive at that moment. From then on, the
only time is **game time**, and it passes only while the game is unpaused. The world beyond
the colony keeps living in that same game time (more coarsely the further away it is, see
Update 5); nothing advances history while you browse the map or the past. The past is read-only:
it is the story the colony inherits.

---

## How we beat DF and RimWorld

Both games prove the genre and show its limits. **DF** simulates enormously but is illegible:
the stories are in the data, and you dig them out of legends mode after the fact. World
history mostly stops while you play, and your fortress barely touches it. **RimWorld** is
legible but shallow. Its storyteller rolls raids against your wealth, pawns are need meters
with moods, nobody has a past, and the world outside the map is a menu. In both, the player
micromanages, so the "story" is mostly the player's.

Our pillars, each tied to a mechanism, not a slogan:

1. **Every event has receipts.** One causal graph runs from plate collisions to tavern
   brawls. A grudge in the colony points to the chronicle entry that caused it ("his
   grandfather died at the Redton ford, year 143, in a war over this valley's iron"). Any
   event can answer *"why?"* by walking its causes. Neither DF nor RimWorld can do this, and
   we already have `caused_by`, participants and places on events.
2. **People with intentions, not need meters.** Agents have memories (references to real
   events), beliefs (which can be wrong, and spread as rumour), relationships, and long-term
   goals generated from personality, culture and their own history. They make plans that span
   days to years. Needs drive the next hour; ambitions drive the story.
3. **The world keeps living around you.** The colony is embedded in the running history:
   caravans, envoys, refugees, war bands and plagues arrive because something happened out
   there, and what the colony does flows back out. A level-of-detail scheme simulates people
   in full near the colony and coarsely far away, promoting and demoting them as they move.
4. **Legibility is a feature, not a wiki.** The watcher, character pages, a live annals view,
   and an *arc detector* that notices rivalries, romances, feuds and falls while they build and
   points the camera at them, like a sports broadcast director. The bard writes prose about
   what actually happened.
5. **A drama manager that plays fair.** RimWorld's storyteller drops raids from the sky. Ours
   (the director) may only pull diegetic levers: an envoy arrives, a rumour spreads, a lair
   stirs. Its threads have checkable triggers, and the engine validates every proposal. The LLM
   is optional, never authoritative, and its outputs are saved so replays stay deterministic.
6. **Autonomy first; the player is a patron, not a foreman.** The default is to watch. Influence
   is a limited budget of omens, favours, laws and priorities, not orders to individuals.
   Micromanagement (DF-style designations) is an opt-in mode, and possession (step into one
   settler, adventurer-style) is another.
7. **Rewind your colony and ask "what if".** The simulation is deterministic, so a colony's
   run is its world, its embark and the list of your interventions. Go back to an earlier
   point in *your* game, withhold the omen that started the feud, and compare the two
   chronicles side by side. Share a colony's story as a seed and a few bytes. Nobody in the
   genre can do this. (The world's past stays fixed: it's what you inherit.)

Anti-goals: no 3D; no requirement to micromanage; no unexplained randomness (every "random"
event is drawn from causes in the world); no hard dependency on an LLM; no system that can't
explain itself.

---

## The six updates

Each update ships something you can run and judge on the `--dev` world, and ends with
**exit criteria** checked by a test or a recorded watcher session. Rough sizes assume the
current pace of work.

### Update 1: The present day *(foundation)*

Draw the line between the past and play, and make the past usable as the starting state.

- **Present-day snapshot.** History ends at a present date, and the world state at that date
  is the game's starting state: which settlements stand, who rules them, which wars are on,
  which beasts are alive, the roads, fields and scars on the land. One explicit
  `WorldHistory::present()` view is what the game reads; the chronicle before it is read-only.
- **Choosing an embark.** Pick a site and a people on the world map (tile viewer). The site
  panel tells you what is true *now*: the nearest towns and their mood toward newcomers,
  claimed or free land, ruins nearby and why they fell, resources, game, dangers. The embark
  map shows the present: ruins, roads, fields, the scars history left.
- **A party with pasts.** The starting settlers come from the history: younger kin of real
  figures, survivors of a real razing, veterans of a real war. Each settler gets a short
  backstory whose lines link to chronicle events (Update 2 deepens them into full agents).
- **Causal graph as an API.** Every event has a kind, place, participants, causes and effects
  (the `Event` struct mostly has this). Add `why(event)` and `consequences(event)` queries and a
  "why?" panel in the viewer, watcher and journal. Rule: new systems create events only through
  this API, with at least one cause or an explicit "spontaneous" tag. Game-time events will
  use the same graph, so the colony's story continues the chronicle.
- **Determinism as a contract.** A CI test that runs `--dev` twice and compares chronicle
  hashes. A seeded RNG stream per subsystem, so adding a system doesn't reshuffle every other
  one. Director/bard outputs are cached in the save.
- **Story tests.** Golden tests on the dev world ("by the present day, at least one war over a
  resource and one razed town, each with a cause chain of 2 or more"), so later updates can't
  silently flatten the inherited story.
- **Housekeeping.** Decide the fate of `mystery_box` (extract the job, labor and designation
  designs into notes).

*Exit:* on the dev world, pick a site and a party; the site panel and every settler's
backstory link to the chronicle, and "why?" walks any event back three or more causes.

### Update 2: People, not populations

Give the world individuals with inner lives, first at world scale, where it's cheap.

- **Agent model** (shared by all later updates): personality (extend `Personality`), needs,
  skills, health, relationships (an opinion graph with reasons pointing at events), memories
  (event references with emotional weight, fading over time), beliefs (facts with a
  source, possibly wrong) and goals.
- **Ambitions.** Goals generated from personality, culture, faith and personal history:
  avenge, found a town, win a rival's heart, master a craft, reclaim a ruin, convert a
  people. Each has a plan (HTN or GOAP) executed through the world sim, so a vow of revenge is
  carried out by the figure, not scripted by the director.
- **Notables and commoners (LOD).** Settlements keep aggregate populations, but notable
  individuals are promoted out of them on demand: children of figures, survivors of a
  razing, a town's best smith. They're demoted back when nothing is watching them.
- **Rumour and knowledge.** Who knows what: news travels along roads and trade routes, gets
  distorted, and drives decisions (a war declared over a false rumour is a great story).
- **Character pages** in the viewer and journal: life, relationships, ambitions, memories,
  each line linked to its events.

*Exit:* in 250 dev-world years, at least 20 ambitions are completed or failed by figures'
own plans, each traceable in the causal graph; a character page reads like a biography.

### Update 3: The settlement comes alive *(local simulation)*

The embark gets a tick loop: one settlement simulated tile by tile, unattended.

- **Game time.** The clock starts at the present day and runs only while unpaused, with
  speed controls; DF-style ticks (~1 game-minute) locally, with day, night and seasons.
- **Local sim core** on `LocalMap`: a tick scheduler, pathfinding across z-levels (ramps, stairs,
  water), weather from the world's climate (TODO: seasons in embarks).
- **Things.** Items and materials on the ground (reuse `Material`, ore), stockpiles, containers,
  food and spoilage, tools.
- **Work.** A job system (study the `mystery_box` job queue and labors): chop, haul, build, farm,
  fish, hunt (using `RegionLore::wildlife`), mine, cook, craft. Jobs come from the settlement's
  own needs and plans, not from player designations.
- **Building.** Settlers extend the town with the culture's architecture (today's town
  generator becomes the plan they grow toward, so roofs and organic streets from `TODO.md`
  land here).
- **Agent brain, local half.** Utility AI for moment-to-moment choices (eat, sleep, work,
  socialise, flee), feeding into the goals from Update 2. Each agent keeps a short "why am I
  doing this" trace you can inspect.
- **Observer UI.** Follow-cam on a settler, speech and thought marks in the ink style, a
  local chronicle panel; embarks fully interactive (verify by hand, per `TODO.md`).
- **Performance budget.** 200 agents at 10x speed on one core, with a profiling harness from day one.

*Exit:* a band of 7 settlers founds a village on the dev world and survives three years with
no input: fields planted, houses built, a winter endured. Its log reads as a sensible
sequence of decisions.

### Update 4: Society *(the story engine inside the colony)*

Turn a working village into a society that produces drama.

- **Relationships over time:** friendship, rivalry, romance, marriage, children, aging,
  inheritance, all from the Update 2 model, now at local tick resolution.
- **Roles and status:** elder, smith, priest, guard, healer. Roles come from skills and
  standing, and the community fills them itself; succession fights are possible.
- **Culture and faith as rules:** norms from the culture and religion (`history/religion`,
  cultures) define what's shameful, sacred or punishable. Disputes, trials, exile and feuds
  follow from them.
- **Mental states that act:** grief, pride, jealousy and despair produce behaviour (a brawl, a
  masterwork, a desertion, a poem), not just a mood penalty. Artifacts and masterworks
  (`ArtifactCreated`) are made by settlers in specific moods, about specific memories.
- **Arc detector:** pattern rules over the event graph that recognise arcs in progress
  (rivalry escalating, a romance forming, a feud brewing, a fall from grace) and score them.
  It drives the watcher's camera, banners and the bard's commissions.
- **Death and memory:** funerals, graves, ghosts if the culture believes in them; the dead
  stay in memories and keep shaping decisions.

*Exit:* ten unattended years on the dev world yield at least five distinct multi-step arcs
(for example rivalry, then duel, then a blood feud across a generation), each recognised by the
arc detector and readable in the journal.

### Update 5: The world reaches in *(two-way coupling)*

Embed the colony in the running history, at the right level of detail.

- **Scale bridge:** after the present day, the world sim continues in game time (a world step
  per game season) while the colony ticks; local events are
  summarised up (the colony's population, wealth, deeds and output become the settlement's
  numbers in the world sim), and world events are pushed down as concrete arrivals.
- **Arrivals with pasts:** caravans that carry real goods priced by real scarcity; migrants and
  refugees from actually razed towns, carrying their memories; envoys, tax collectors and
  tribute demands from the faction that claims the land; war bands from actual wars; beasts
  from real lairs; plague from real plagues.
- **Outflow:** the colony's iron arms a faction's war; its people leave to found towns, join
  armies, go adventuring, and come back changed (or don't).
- **LOD promotion and demotion** of agents crossing the colony border, keeping identity,
  memories and relationships intact.
- **Ecology coupling** (TODO Phase D): hunting and logging deplete what the world's ecology
  provides; game returns when the colony shrinks.
- **The director, in situ:** paces the drama with diegetic levers only, with its threads
  (prophecies, feuds, vows) now able to land on settlers.

**Decision: the play scale (2026-10-05).** The colony lives in a *theatre* about 250 km
around it, at region resolution (`region/zoom.rs`, ~0.6 km cells at 512 wide), with the real
towns, roads, lairs, ruins and the Shadow's frontier placed in it (`lore/settle.rs` already
places sites inside their world tiles), and travel counted in days: 25 km a day on foot, paths
1.3 times the straight line (roads and rivers faster, mountains slower, once routed). Arrivals
are scheduled from real places by those days (a caravan six days out is news before it is a
sight). Beyond the theatre the planet is backdrop and legend: it sends news, letters and rare
expeditions, not neighbours.
- Measured with `--province-snapshot` (the embark, a 250 km ring, its three nearest places and
  the days to each): at 512x256 (78 km tiles) the seed-42 embark at 250,90 has a village 102 km
  away, 6 days on foot; on the dev world (96x48, 417 km tiles) the nearest towns are 1,210 and
  1,254 km away, 63 and 66 days. So the dev world cannot carry Update 5: its work is judged on
  256x128 or 512x256 worlds, or on the dev world with a province pass that seeds the theatre.
- Province pass (to build with Update 5): when a theatre holds fewer than three places, the
  nearest people founds hamlets, waystations and a shrine inside it, recorded in the chronicle as
  colonies of real towns, so every embark has neighbours within two weeks' walk.

*Exit:* the causal graph contains chains that cross scales in both directions (a colony
event causes a world event and vice versa), shown on a dev-world run; a migrant's grudge
traces back to a world-history battle.

### Update 6: The patron's game *(playable release)*

Wrap it into something a new player can pick up, play for an evening, and remember.

- **Modes:** *Observer* (pure watching), *Patron* (an influence budget spent on omens,
  favours, laws and priorities; you are the settlement's god, spirit or distant lord, depending
  on the culture), *Steward* (opt-in DF-style designations for players who want them), and
  *Possess* (play one settler as an adventurer; TODO Phase E).
- **Start and finish:** generate a world and watch its past being written (the watcher as the
  intro), then pick a people and a site in the present day; fail states and endings (the colony thrives, falls, or becomes a city);
  legacy, so your colony becomes part of the next world's history.
- **Timelines UI:** rewind your colony to an earlier point, fork and compare chronicles; share
  a colony's story as seed plus log.
- **Books:** export the colony's chronicle as a journal and a bard-written book.
- **Onboarding, balance and performance:** a tutorial world (the `--dev` world), difficulty
  as "how much the world pushes", 500+ local agents, saves under a few MB plus the world.
- **Modding:** data-driven races, cultures, jobs and buildings via `data/` (already partly
  data-driven through `GameData`).

*Exit:* a person who has never seen the project starts a colony, watches and nudges it for
20 years, and can retell its story afterwards, unprompted.

---

## Principles that hold across all six

- **Dev world first.** Every feature is built and judged on `--dev` (seconds per run), then
  scaled up.
- **Deterministic or it didn't happen.** Same seed and same interventions give the same
  story; the LLM's words are cached.
- **Causes before content.** A new event kind needs a cause in the world, not a dice roll.
- **Legible by construction.** Each system ships with its explanation (the "why?" trace,
  hover text, a chronicle line).
- **Headless-checkable.** Every view has a snapshot mode, and every update has story tests.

## Risks

- **Performance at agent scale.** Mitigated by LOD, a profiling budget from Update 3, and the
  dev world for fast iteration.
- **Emergent mush.** Lots of simulation, but no story. Mitigated by the arc detector, the
  director, and golden story tests that fail when drama flattens.
- **Scope.** Each update must ship standalone; nothing waits on Update 6 to be enjoyable.
- **LLM cost and latency.** Async, budgeted, cached, always optional; the game is complete
  without it.
