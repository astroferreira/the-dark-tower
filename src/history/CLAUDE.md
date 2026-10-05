# History simulation (`src/history/`)

Use `crate::history::det::{HashMap, HashSet}` for any map iterated while drawing from the RNG (see the root CLAUDE.md).

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
  grudges with their cause, living beasts and the nearest town, the Shadow's frontier, disputes,
  heirless rulers, recently fallen towns). `--present` prints it; the summary prints its counts.
  `tests/present_day.rs` pins the dev world: 2+ wars or sieges, 3+ beasts near a town, 5+
  grudges, 4+ peoples, 3+ frontier towns, a war in every half century, a consistent chronicle and
  a byte-identical journal across two runs.
- Names (`naming/generator.rs`): `pronounceable` (3-9 letters for persons, up to 12 for places,
  vowel runs <= 2, consonant runs <= 3, one apostrophe or hyphen) with retries; realms are named
  by government (`realm_name`: "The Kingdom of Galost", "The Bonegore Horde").

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
  century after a town falls. `apply_scars` writes them into `world.biomes` (in `main`).
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
  a town it took may be freed (5%, `liberate`, `ShadowLiberated` caused by the conquest). If its seat falls it is broken
  (`ShadowBroken`) and returns ~20 years later in a new seat. Every deed is `caused_by` the
  previous one, back to `ShadowRose`. Measured 2026-10-04: 27-36% of strikes held on eight dev
  seeds, 29-31% at 256x128 and 512x256; it darkens 15-23% of the land on large maps (20-56% on
  the dev world) and holds 2-15% outright. The `Shadow:` report line prints both.
- Drawn in three layers (`render.rs::shadow_ink`, `classify.rs`): reach = a cold grey wash that
  deepens with corruption (land only); blight (corruption >= 0.6) = dead trees; dominion =
  cross-hatching (doubled where deepest) inside a jagged ink border; the seat is a black tower
  with a red eye. The watcher has a Shadow panel (lord, darkened/blighted land, towns held,
  fallen, held out) and marks its realm in red.
- Saved as the world file's last field (`WORLD_FILE_VERSION` 5; v1-v4 still load).
