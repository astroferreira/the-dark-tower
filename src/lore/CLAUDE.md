# Lore (`src/lore/`)

Derived world lore: resources, landmarks, focal points, the journal and the bard.

## Resources and history (`lore/resources.rs`, `history/simulation`)
- `world.resources()` (lazy, never serialized) derives the world's wealth from its geology:
  copper/gold/silver/gems in high-stress arcs and orogens and around volcanoes, tin and silver in
  granite uplands, iron in hard rock belts, coal in wet sedimentary lowlands, salt in dry basins;
  plus farmland (water, warmth, flat ground, floodplains, volcanic soil), timber and fish.
  `--resource-stats` prints the mix (metals average stress +0.4, coal/salt ~0).
- History uses it: settlement sites score fertility and nearby ore; `apply_local_economy` sets
  each settlement's local resources, production, carrying capacity and growth; trade partners
  and goods are chosen by what each side has and the other lacks; faction income includes
  extraction; resource envy between neighbours adds friction, produces "dispute over iron"
  incidents and Resource wars. `--civilizations N` (default 60) sets the founding powers.
- Settlements are founded (colonization from crowded towns), conquered, razed or abandoned, so
  ruins exist. Sieges launched when a war ends now carry on after it (they used to be lifted
  instantly). Disasters take a fraction of a settlement, not a flat number.
- Visible: the tile viewer's `R` toggles ore markers (size = richness); hover names deposits,
  farmland and fishing grounds; embarks contain ore veins (`Material::Ore`, recoloured flecks) in
  host rock near deposits.
- `ResourceType` gained Coal/Tin/Fish at the *end* of the enum (bincode-compatible with old saves).

## Landmarks (`lore/landmarks.rs`)
- `find_landmarks(world, gazetteer)` picks the world's extremes: the highest peak ("the roof of
  the world") and each continent's summit, the longest river (measured along its main stem in
  km at the real tile size), the largest and the deepest lake, the greatest waterfall on a
  named river, the deepest gorge (river tile with high ground on both banks), up to 3 crater
  lakes and 3 groves of giant trees (CraterLake / AncientGrove patches), the largest desert and
  forest. Each has a name, an epithet and a measurement.
- Crater lakes and groves of giant trees occupy whole world tiles but are small things: their
  size comes from their kind (a crater 2-12 km across, a grove 20-400 hectares, hashed per site)
  and each has a name of its own ("Lake Galanor", "the Galiel Wood"; never repeated). Naming
  them after their region gave three "crater lakes of Neaslind" of a million km2 each.
- Shown: `--gazetteer` prints them; tile-viewer labels of landmark features rank above their
  kind and show from 2 px/tile (falls, gorges, crater lakes and groves get their own labels);
  hover adds "the longest river in the world (4,700 km)" on every tile of the feature; the
  journal's opening names the longest river, largest lake, greatest falls and deepest gorge.
- Water bodies: the body holding every river tile has the reserved id `WaterBodyId::RIVER`
  (65535). It used to take the next free id, so `is_lake()` held on river tiles: the viewer
  drew river confluences as lake squares, the gazetteer named all rivers one huge "lake", and
  history/resources treated rivers as lakes.

## Director pass: focal points (`lore/focal.rs`)
- After the world is assembled (in `main`, before history and saving), every named region
  (forest, jungle, desert, plains, tundra; 60+ tiles at 512x256) without a focal point (a named
  peak or lake inside it, a volcano, or a focal biome such as a grove, oasis, crater lake, karst,
  hot springs, ruins) gets one at its most interior tile: an ancient grove (giant trees, drawn
  with the big broadleaf sprite) in forests and jungles, an oasis (a few palm tiles) in deserts,
  a crater lake (drawn as lake water) on plains and tundra. Planted points keep 14 tiles (at
  512 wide, at least 5) from each other and from existing focal points. They are biome patches
  saved with the world; landmarks list up to 6 crater lakes and groves. Seed 42 gets 4, the dev
  world 10. Placed features are designed, not caused (unlike scars and salt flats).

## History journal (`lore/journal.rs`)
- `--journal PATH` (or `J` in the tile viewer, which writes `journal_<seed>.html` and opens it)
  writes the history as a self-contained HTML book: "The Annals of <largest continent>" with an
  opening on the geography, one book per age (merged timeline eras) with year-by-year entries,
  then the peoples, the 30 greatest wars, ~220 lives of note, ~160 beasts of legend and the
  land (fauna vs. start, scarred places). Routine events (raids, treaties, quarrels, trade,
  new villages, crafted artifacts, conversions) are folded into yearly or per-age tallies; a
  people's founding is told once. Names with entries are linked via event participants.
  Search box and category chips filter the annals. Styled like the ink map (parchment, sepia,
  red year rubrics; dark theme).

## Story sifting (`lore/sifting.rs`)
- `sift(history)` finds the coincidences people retell, as queries over the chronicle's
  participants and causes: FellWhereTheyWon (a figure named "the Victor of X" dies in battle at
  X), Echo (3+ battles at one town over 40+ years), Reversal (a town taken, then won back by its
  old people; burnings don't count), BeastsBane (a beast raids one town 3+ times and is slain
  within 30 years of the last raid; better if the slayer lives there), LastStand (a people loses
  its seat and holds out 5-40 years; once per people), BladeAstray (the Shadow's bane ends in a
  hoard or a stranger's hands). Each is a `Tale` of 2-3 sentences, 3 linked events and a score
  (events + span + rarity); kinds take turns in the list (`interleave`).
- Shown: the journal's "Tales Worth Telling" part (top 30, events linked to their years), the
  inspector's event pages ("A tale worth telling"), `--present` (top 12), and the summary's
  `Tales:` line. Dev: 60 tales of 6 kinds; `tests/flavour.rs` requires 10+ of 3+ events and 4+
  kinds. Lookups are indexed (battles by figure, slayings by beast): fast on 512x256.

## A world in one sentence (`lore/claims.rs`)
- `claims(history)` gathers the history's records (the longest reign, the bloodiest battle, the
  town that changed hands most, the beast with the most dead, the longest war, the captain with
  the most victories, the Shadow's tally, the greatest city, the town raided most), each scored
  against the median record over dev seeds; `sentence(world, claims)` joins the three most
  unusual: "In Neaslind, Newway was raided 34 times; Skathels the Warden of Newway won 33
  battles; and Baelen Storm-Caller killed 1,239 in its raids and lives yet."
- Shown under the journal's title, first in `--present` (named after the largest continent) and
  as the summary's `Records:` line; the present-day test requires six dev seeds to give six
  different sentences. Plates (when they exist) should carry it too.

## Rare outcomes (`lore/rare.rs`)
- `find(world, history)` detects seven world-defining outcomes, each with a cause line: the
  Undying King (crowned 50+ years before written history, reigns still), Never slain (a beast
  from the dawn with 15+ raids, alive), a People in exile (fell 40+ years ago, 4+ notables in
  exile), an Empire across the sea (5+ towns on each of two landmasses), the Unbroken seat (the
  one founding capital never besieged, its people at 5+ wars), Risen again (fell, rose, now the
  greatest realm), Unbroken peace (neighbouring peoples from the dawn who never fought).
- `RATES` are measured per 100 dev worlds (mean of seeds 1-100 and 101-200 with
  `scripts/rare_rates.sh N`): 10, 24, 31, 21, 5, 6, 25. Re-measure when history changes.
  `--present` lists the ones a world has with "1 world in N"; the summary prints `Rare:`.

## Marginalia (`lore/notes.rs`)
- The player pins notes to places: `M` in the tile viewer at the tile under the mouse (type,
  Enter pins, Esc drops; the other keys are quiet while typing), or `--note "X,Y:text"`
  (repeatable). Notes are kept beside the world (`notes_<seed>.json`, or `<world file>.notes.json`
  with `--load-world` / `--save-world`), drawn on the map with a pin in a hand (Indie Flower, OFL,
  `fonts::Face::Hand`) and on plates. The journal sets each note in the margin beside the first
  annals entry within a tile of its place, and lists them all in a Marginalia part. Tested
  (`tests/present_day.rs`). The bard stays an option, not the default voice.

## The bard: LLM-written lore (`lore/bard.rs`)
- `--bard N` has a local model served by Ollama (`--bard-model`, default `gemma4:26b`;
  `--bard-url`, default `http://localhost:11434`) write N pieces from the history:
  founding songs, poems by notable figures (love / grief / a slaying / war / homeland, chosen
  from their own life), folk legends of beasts, laments for razed towns, artifact
  inscriptions and lore, soldiers' ballads of bone fields. `commissions()` builds the queue
  (most significant first, kinds interleaved, skipping what's already written); each prompt
  carries the writer's voice (by race), temperament and the real geography (biome, climate,
  named rivers/peaks/forests nearby). `--bard-prompts N` prints prompts without the model.
- Writings live in `history.library` (`Library`), saved as the world file's last field
  (`WORLD_FILE_VERSION` 3; v1/v2 still load). With `--save-world` the world is saved after
  every piece, so long runs can be interrupted and resumed (rerun adds the next N).
- The journal shows them in place (songs at foundings, laments at razings, poems in lives,
  legends with beasts, ballads with scars), plus a Treasures part and a Songs and Sayings index.
- Speed on this machine: ~10 s per piece with `gemma4:26b` (MoE, ~4B active, 44 tok/s) vs
  66-105 s with the dense `qwen3.8:27b` (3.5 tok/s); Gemma also keeps to the prompt rules
  better. `--bard-rewrite` rewrites already-written pieces with the current model.
