# Adventure mode (`src/adventure/`, window `src/tiles/adventure.rs`)

One person of the world, from nobody to a name in its history (branch `adventure-mode`,
2026-10-09). DF's adventure mode for the world's side (places from the history, realized on
demand from their seed and cause; skills by use; townsfolk who know what their town heard; the
history's treasures where they were lost), Tibia for the hero's side (levels, callings, spells,
loot, multi-floor dungeons with keys, levers, rope holes and one-reward quest chests, tasks,
death that costs experience). Turn-based and deterministic (`Game::act`).

## Running it
- `--adventure` opens the window on the generated world (the start screen's Play row too);
  `--adventure-load adventures/SEED_NAME.adv` resumes (F10 saves; leaving saves).
- `--adventure-bot N` plays N acts headless on the dev world with the bot (`bot.rs`) and prints
  its record every N/10 acts (`PLANET_ADV_CALLING=knight|paladin|sorcerer|druid`,
  `PLANET_ADV_DUMP=1` dumps the floor as text when a thousand acts pass with no progress,
  `PLANET_ADV_TRACE=a,b` prints each act's choice, `PLANET_ADV_SAVE=FILE` checks a save round
  trip, `PLANET_ADV_DEBUG=1` counts the history's sources of places).
- `--adventure-snapshot PREFIX` (temple, talk, sewer, land, night, town, world, surface fight,
  deep, boss frames by the bot's play), `--adventure-gallery PREFIX` (one revealed floor of each
  kind of place) and `--adventure-landscape PREFIX` (the land about a tile of each kind: home,
  woods, mountains, desert, snow, plains, jungle, a city, a port, a hamlet, a ruin, a cave mouth,
  a ford; revealed, from above at 8 px a cell; prints the creatures about).
- `PLANET_ADV_LOAD=FILE` plays the bot on from a save (with the save's world: `--seed`,
  `--width`, `--height`), `PLANET_ADV_SITES=NAME` lists matching places, `PLANET_ADV_DEATHS=1`
  prints each death as it happens.
- Tests: `tests/adventure.rs` (a commoner grows: level 12+, a calling, bosses, quests, chests,
  never stuck in the first half; the same seed twice; a save loads whole and plays on the same),
  unit tests in `site.rs` (13 kinds x 6 seeds: every way down reachable, deterministic),
  `town.rs` (six town shapes: every trade, homes, reachable from the gate, sewers; towns differ),
  `surface.rs` (a road and a river cross a tile's border and meet themselves), `land.rs`
  (walking east over three tiles: no jump, the land re-centred, the cell underfoot as made; a
  cave's mouth in and out at the same spot; a dropped rope still there after travelling),
  `hero.rs`, `item.rs`, `data.rs`.

## The parts
- `data.rs` + `data/defaults/adventure.json`: 34 monsters (rat .. demon: hp, attack, defense,
  armour, speed, xp, look, ai melee/ranged/caster/slow, tier, habitats, loot, sounds, poison,
  heals, regen, packs, undead, lifesteal), ~90 items (weapons by skill, ammunition, wands,
  shields, armour by slot, jewels, potions, food, light, tools, creature loot), materials
  (attack and armour factors, colour, holy), callings (life and mana a level, learning rates),
  15 spells (heal, light, haste, strikes, waves, balls, a ring of blows).
- `item.rs`: type x material x quality (DF's - + * ≡ ☼, and 6 = an artifact of the history with
  its own name and story); attack/defense/armour/value from all three; `describe`, `stats`.
- `map.rs`: floors of cells (ground, wall, feature) with things lying on them; features: doors
  (locks by key tag), portcullis and lever, stairs, ladders, a hole (one way; a rope climbs the
  rope spot below; every hole floor also has a ladder up far off), the way out, chests, quest
  chests ("take one"), sarcophagi (the dead may rise), altars, braziers, statues, pillars, webs,
  bones, wells, tents, campfires, sconces, plinths, signs, traps, rails. Stairs down at (x, y)
  arrive at stairs up at (x, y) below (all floors of a place share one size). Sight by rays
  (walls beside seen floor are seen; trees and boulders do not block it); distance maps.
- `site.rs`: `SiteKind` (Town, Cave, Lair, Mine, Ruin, Tomb, Temple, Shrine, Castle, Labyrinth,
  Camp, Halls, DarkFortress, Wilds), `SiteSpec` (seed, tier 1-6, cause, boss, treasures, the
  land's ground, people, god, the town's news, a capital's ruler), `realize`: layouts (cellular
  caves, rooms and corridors with doors, catacombs with burial niches, a maze, mine galleries
  with rails, a building on open ground: temple nave, keep with towers, mausoleum, ruin with
  rubble, a war camp's palisade and tents), `connect` (regions joined by tunnels), the way down
  at the far end, the last floor's door locked with its key on the floor above, portcullis and
  lever in labyrinths and halls, furnishing by kind, monsters by habitat and tier (deeper is
  worse), the boss at the far end of the last floor with a quest chest beside it, a lost
  treasure on a plinth where no boss holds it.
- `town.rs`: walls and gate, a cross of streets, the square with a well and a sign, temple
  (priest), inn, smithy, shop, the lord's hall, guardhouse, a sage's house, homes, townsfolk;
  a grate into two sewer floors (rats, then worse; the Rat King). `person_name` names in a
  people's tongue.
- `world.rs`: `WorldInfo` (land, ground by biome, danger: distance from towns, the Shadow's
  corruption, lairs; `reachable`, `step_toward` over land) and `build`: towns (news from
  `history::knowledge`, a capital's ruler), ruins of razed towns (castles where the history's
  forts or capitals fell), lairs of the history's living beasts (the boss is the beast:
  `monsters::of_legend`, its hoard of the history's artifacts by name), tombs of the fallen
  (figures who died in battle, matched to the battle by year; beside the field), monuments
  (temples, keeps and towers as castles, tombs and pyramids, altars and obelisks as shrines),
  religions' holy sites, cults' headquarters, the Shadow's seat and shrines of its cult, lost
  artifacts placed in the nearest ruin or tomb (or a ruin of their own); then caves, old mines,
  labyrinths, war camps and dwarven halls from the land (names unique). Tiers grow with the
  distance from the start town (a living town near places, of the people asked for).
- `hero.rs`: rolled from the persona (`persona.rs`: skin, hair, beard, strength); level from
  experience (Tibia's cubic curve at a third), seven skills trained by use (Tibia's tries x the
  calling's rate), eight slots, pack, spells, food, torch, poison, haste, blessing.
- `game.rs`: the state (places realized when first entered and kept; slain monsters come back
  after 12,000 ticks, bosses never), `Action`s, bump to act, combat (Tibia's formula: skill x
  attack, block by defense, soak by armour; wound words by the share of life and the weapon;
  body parts), ranged and thrown and wands, spells, poison, monster turns (wake on sight, sounds,
  chase by distance map, open doors, flee when beaten unless cornered, shoot, heal, regenerate,
  a beast of the history's own attack), death (a tenth of experience and half the gold where
  one fell, unless blessed; wake at the temple), travel on the world map (food, healing,
  ambushes by the land's danger as a Wilds floor), the companion, saving and loading (header
  `ADVENT02`, then gzipped JSON of the world's size and the game: a field added to any saved
  type needs `#[serde(default)]` so older saves still load; the first builds' bincode saves,
  `ADVENT01`, are refused with a message). `--adventure-inspect FILE` prints a save without its
  world.
- `npc.rs`: talk by keyword choices: name, job, trade (buy by role and level; sell loot or old
  gear), quest and report, rumours (an unknown place near and the town's news of the world),
  the sage's maps, the priest's healing (free to 15; bread for the hungry poor), calling at
  level 8 (with its first weapon), spells (40 gold a level), blessing; the smith's improving
  (a quality step); the inn's bed and sellsword.
- `quest.rs`: the lord's slayings (beasts and bosses of places near that fit the hero), the
  priest's (the risen of tombs and temples), the guard's bounties (Tibia's tasks), the sage's
  search for a lost artifact of the history, the trader's parcels (to a town a few days off by
  land, once each); reported to the giver for gold, experience and things.
- `bot.rs`: the bot (errands in town, the road by land, exploring floor by floor, fights,
  retreats, gives up what it cannot reach or catch) and `--adventure-bot`'s report.

## The land (`surface.rs`, `land.rs`; 2026-10-09, the seamless goal)
Every world tile is walkable: a chunk of `surface::CH` = 96 x 96 cells. One walks out of a town's
gate, down its road, over the border into the next tile's woods and into a cave's mouth, with no
menu in between (the first play's complaint: the world map's Enter put one in the same fixed town
everywhere).
- `surface::generate(land, tx, ty, sites)` builds a chunk from the world's fields keyed on world
  cells (so it is the same chunk however it is reached): land and sea by bilinear tile land plus
  noise (a wandering shore with beaches, shallows, sea ice), the biome per cell from the nearest
  tile's middle after a noise warp (borders interlock), elevation for ridges with passes and rock
  ground (rough = (elev - 700)/1800, `Kind::Mountain` +0.35, capped 1), ground by `Kind` (11:
  plains, woods, jungle, pine, desert, snow, marsh with pools, mountains, dry grassland, blasted
  waste, lake country), woods clustered and capped at 0.38 a cell (woods are walked through,
  never a wall; the history's forest cover thins or thickens them), the Shadow's ash and black
  stone, farmland as hedged fields on an 11 x 9 grid, lakes, then rivers and roads: each runs the
  full line between two tiles' anchors (`Land::anchor`, a town's square or the nudged middle) on
  a wiggle seeded by the ordered pair, drawn by every chunk within two tiles and clipped, so
  both sides of a border draw the same line (fords every 24 cells; bridges where roads cross
  water; a road to a town ends at its gate, `gate_cell`). Order: ground, lakes, fields, rivers,
  the town, roads, other places, finds.
- Places in the land: a town is laid in its tile (`town::lay_out`, stamped by
  `town::footprint`: the walls' rect and its piers; its people come with it; the grate is
  `Feature::Entrance { site, z: 1 }`). A surface place (ruin, tomb, temple, shrine, castle, war
  camp, the dark fortress) stands where it fits (land, clear of the rest): its first floor's
  built cells (walls, made ground, fittings) are copied in, its way down becomes an Entrance to
  floor 1, the dwellers within its walls move to the land (`Place.top = 1`, `Place.origin` the
  world cell of its (0, 0): going up from floor 1 comes out there). Too big for the room left (a
  castle beside a city): a gatehouse with an Entrance to its own floor 0. Under the ground (cave,
  lair, mine, labyrinth, halls): a mouth (a rock mound, a timbered adit, a carved door, a brick
  ring) with `Entrance { site, z: 0 }`; its Exit comes out at `Place.mouth`.
- Finds (`finds`): a camp by the road (bandits and highwaymen far from towns), a fallen
  traveller's bones and gear (sometimes a treasure map), the old stones (an altar in a ring:
  bump to mend a quarter of one's life), a hermit (a sage: places, runes, buys charts) far from
  towns, a farmstead and its farmer, a battlefield's bones and rusted arms (the history's battles,
  `WorldInfo::tales` told when one first walks there), herbs (25 life).
- Perils (`perils`): monsters by habitat words (plains, forest, jungle, desert, snow, swamp,
  mountain, savanna, waste, road, shadowland, battlefield and night ones: night, KIND_night,
  full_moon), tier 1 + danger x 3 / 255 (+1 in the Shadow's lands far from towns), bands 2 +
  danger / 40 (+2 at night); within a tile of a town tier 1 (2 in the Shadow) and two bands.
  Land creatures in `adventure.json`: hare, deer, mountain goat, aurochs (shy), giant toad,
  scorpion, vulture, crocodile, lion, jaguar, winter wolf, highwayman (shoots; carries maps),
  barrow wight (night), white bear, giant scorpion, werewolf (full moon), shade (Shadow), hill
  giant; the old ones gained land habitats.
- `land.rs`: the land floor is place `LAND` (u32::MAX - 7), the 3 x 3 chunks about `centre` as
  one 288 x 288 floor (`build_land`); `store_land` writes it back per chunk (`Chunk`: tiles that
  differ from as made, things, creatures, people, seen bits, when stocked), so saves keep only
  changes (`chunk_map` serde). Walking 4 cells into a side tile re-centres (`recentre`: the hero,
  companion, corpses shift; `Game.shifted` tells the window). `enter_tile` updates the tile, the
  land's spec (a town tile's spec is the town's, so its people talk as townsfolk: `npc::town_id`
  is the speaker's `Npc.home`), the route, the map, news of places and the history's tales.
  `ensure_chunk` makes a chunk the first time (its places realized and kept, creatures given
  uids from `Game.next_uid` >= 1,000,000, stocked by a seed of the chunk); chunks left 40,000
  ticks are restocked (not towns). `pristine` keeps the made tiles (<= 40, the window's kept).
- Coming and going: `go_in` (an Entrance underfoot or Climb on it), `come_out` (up from a
  place's top floor or its Exit), `to_world_map` (T; refused while an awake foe can walk to one
  within 10 steps), `land_here` (Enter or T on the map; beside the named place for
  `EnterSite`), `ambush` (travel's ambush lands one among the attackers, tier capped by level).
  Death wakes one before the temple's altar on the land.
- Day and night: `DAY` = 144,000 ticks from 8 am; night 20:30-5:00; full moon every 28th night.
  At night sight under the sky is the light + 2 (5 at least, 8 under a full moon), the window
  tints blue with the torch warm, every 3,000 ticks something of the night may come out of the
  dark out of sight (`land_tick`: wights, the dead of battlefields, werewolves; not in towns) and
  dawn sends them off (`Monster.night`).
- The Mapmaker (the twist): `Game.mapped` per tile (0 blank, 1 heard of, 2 inked). The world map
  shows blank parchment where one has not been, a faint grey sketch where one has heard
  (rumours, the sage's old maps, quests), the world where one has walked (edges bleed); the
  route walked is dotted; treasure-map crosses. Walking a new tile inks it and the tiles about
  (from 1,800 m up, two tiles about) for 4 experience a tile; a sage (or a hermit) buys the
  charts (`npc::Topic::Charts`, 3 + level/4 gold a tile). Travel over a road tile takes 1,600
  ticks, over inked country 2,400, over blank 3,600 with ambushes 1.5 times as likely (1.4 at
  night). Treasure maps (`kind: map`, tag = the target tile) mark a cross; a shovel digs at the
  cache's cell (`surface::cache_cell`) within a cell: treasure of the land's tier + 1 and a deed.
- Old saves (before the land) load into it: `seamless` false makes `into_the_land` (places made
  anew, the hero wakes in their temple).
- Bot: `on_land` does a town's errands on a town tile, else fights and walks to the Entrance of
  its target (`land_way`) or takes the road; its paths never step on a way in, up or down that
  is not the goal (it fell into a castle far above its level walking past), and it flees a
  floor only when the way up is near. Balance (dev world, 40,000 acts): all four callings level
  20-21, no deaths (as before the land).

## The living world (`living.rs`; cards adv-world-moves, adv-deeds-chronicle)
- The history goes on while one plays: a season every `living::SEASON` ticks (15 days, so a
  play sees wars and towns fall). The host (the window's `run`, the bot's `report`) keeps a
  `Living` (a clone of the world's history + the game data from `data/`) and calls
  `sync(&mut game, world)` after every act: deeds queued by the game go in, the seasons the
  clock passed are stepped (`simulate_step` with a per-season seed), and `Game::on_history`
  brings the changes in: a town razed becomes `SiteKind::Ruin` with its `TownShape.razed` (its
  streets stamped burned and broken, looters and the dead in them, `stamp_town`), a town founded
  becomes a new site, a capital's new ruler takes the hall's chair (`rename_lord`), every town's
  news is this season's (`world::town_news`, which now tells the headline: it used to give only
  "as X tell it"), and up to two great events the nearest town knows reach the log. Tiles whose
  places changed are made anew (`remake_tiles`: their saved chunks let go). A razed home sends
  the hero's temple to the nearest living town.
- The adventurer is a figure of the history (`Game::hero_figure`, made by the `Arrived` deed,
  epithet "the Wanderer"; the legends roll a persona for them). Deeds (`living::DeedKind`:
  beasts of the history slain (`SiteSpec::creature`, sets the beast's death), bosses, relics
  found (`ArtifactFound`), quests done (`QuestCompleted`), falls, the Shadow broken) are
  recorded as chronicle events (`EventType::AdventurerDeed`, appended; fame Great for slayings
  and finds, Legend for the Shadow). Word spreads by `history::knowledge`; `Game::songs` holds
  what each town has heard (`refresh_talk`), sung by the inn's bard (`npc::Topic::Songs`). The
  window writes the world's legends with the hero in them beside the save on leaving
  (`SAVE.legends/`); the bot with `PLANET_ADV_LEGENDS=DIR`.
- Saves keep `seasons` and `hero_events` (each with its season); a loaded adventure is
  replayed (`sync` steps the same seasons and puts in the same deeds at the same seasons, quietly:
  the save holds the results). `PLANET_ADV_SAVE` checks the replay ("History: ... the same");
  `PLANET_ADV_WORLD=1` prints the world's changes as they reach the bot. Sites gained
  `settlement` and `creature` (the history's ids). The bot is ~500 us an act with the history
  stepping (was ~235).
- Test: `tests/adventure.rs::the_world_moves_and_remembers` (in 16,000 acts a town falls or is
  founded or a lord changes, the history replays the same, 2+ towns sing, the legends have the
  hero's page of deeds).

## People (`people.rs`; card adv-personas)
- Every townsperson has a persona rolled from their name and town (`people::persona`, the same
  `Persona::roll` as the history's figures) and a temper read off its strongest facets
  (`Temper`: kindly, grasping, gruff, timid, cheerful, gloomy, proud, curious, plain). Their
  greeting (`people::greeting`), their prices (`price_factor`: grasping 1.2, proud 1.1, kindly
  0.9, timid 0.95; a friend 0.9 more; each wrong +25%), their refusal (`refusal`) and the Name
  topic (looks, character, likes) follow it. A timid priest heals a famous hero (3+ towns sing
  of them) for nothing.
- `Npc.met` (`people::Met`: times met, last meeting, deeds done for them, wrongs): greetings
  change for one met before, met long ago, a friend ("We still speak of the beast of the Bat
  Pit"), or one who wronged them; a quest reported is remembered by its giver. It is kept with
  the land's chunks. A stranger famous in songs is greeted as such.
- Companions have tempers too (rolled from their name) and `morale`: a day mends it (kindly and
  cheerful more), being badly hurt lowers it (timid more), a boss slain raises it, grasping ones
  want 8 x level gold every ten days; at 0 they leave with a line in their temper.
- Tests: `people::tests::people_differ_and_remember` (12 priests, 3+ greetings and 2+ terms;
  a deed named), `land::tests::townsfolk_remember_the_hero` (met twice, the deed named, kept
  after leaving and coming back).

## Asking (`lore.rs`; card adv-knowledge-talk)
- Talk's "Ask about..." lists what the speaker's town knows worth asking about (`lore::subjects`:
  beasts, people and treasures in its notable news) and takes a typed name (the window's talk
  card: type, Enter; `npc::ask_typed`). `lore::find` matches the name among the history's beasts,
  figures, towns, peoples and treasures (exact, then a beginning, then a word, then within;
  ties to the most told of). `lore::ask` tells the most famous events about it that the
  speaker's town knows (a sage: their whole people), as their people tell them
  (`knowledge::account`), and where it is (`whereabouts`: a beast's lair, a town, a people's
  seat, a person's home, a treasure's place) in days and a direction, marking the map.
- The inn's drunk (`Npc.of == "drunk"`, placed in every inn by `town::lay_out`) has heard
  everything and tells it wrong: the dead and the deeds grown ("raids" become "burned half of"),
  a confident tail, the wrong direction, nothing marked.
- `Game::history` is the host's living history shared by `Arc` (copied on write only while
  the game holds it during a step; `sync` lets go first).
- Test: `tests/adventure.rs::a_sage_and_a_drunk_tell_it_differently` (`PLANET_ADV_ASK=1`).

## Tales (`tales.rs`; card adv-world-quests)
- Quests from the world's state, offered before the old commissions (`quest::offer` asks
  `tales::offer` first), one of each kind a town, the hero's level fitting: the guard's
  **snatched child** (raiders of a camp, cave, lair or ruin near hold them; entering it the
  raiders call their price: fight or pay; the child comes home and lives in the town), the
  trader's **lost caravan** (on a real trade route of the history to its partner town; the wreck
  is set in the land where the hero comes on it, with its raiders; return the goods or keep them),
  the lord's **cult in the cellar** (`SiteKind::Cellar`, appended: cells of the history's cults
  under the nearest town, and of the Shadow's servants under the two towns nearest its seat,
  made at build so no town is remade; the door is beside a bed in a house; the keeper's ledger
  names a townsman: expose them (taken in chains) or sell them their silence), a
  townsperson's **feud** with another house (make peace for a feast, or side with one: the other
  leaves town), a **beast's tribute** (carry it to the lair's mouth, go in and end the beast, or
  keep the gold), the priest's **plague** (moonpetal set on the last floor of a place near:
  bring it, or sell it to a rival town).
- `quest::Goal::Tale(Tale)` (kind, stage, site, tile, the other one, the other town, a tag,
  a price, what was chosen), `State::Failed` (appended), `Stats::tales_done`. Turning points
  are choice cards (`Game::choice`, `Action::Decide`; the window draws them, 1-9 or a click; no
  other act while one waits). Hooks: `on_accept` (the tribute chest; the herb placed),
  `on_tile` (the wreck, the lair's mouth), `on_enter` (the raiders' price), `on_kill` (the
  chief, the keeper's ledger, the beast), `on_found` (the goods, the herb), `on_greet` (the other
  house), `decide`, `on_report`. Ends change the town: people leave (`Game::remove_npc`) or come
  (`add_townsperson`), the town remembers (`SiteSpec::notes`, told first in rumours), the
  townsfolk remember who helped and who wronged them (`people::Met`).
- New items (tagged, never sold): trade goods, a tribute chest, moonpetal, a ledger.
- Bot: decides every card by its first way; takes tales from the lord, guard, sage, trader,
  priest and a townsperson (at most three open); travels to a tale's place or tile, picks up a
  wreck's goods, settles a feud, reports to any giver by name; gives up a tale after 6 fruitless
  arrivals and a walk to a way in after 200 steps. Fixed on the way: talk now finds this town's
  own people (a neighbouring town's lord shares the land floor), a town visit lasts until its
  errands are done whichever side of a tile border the hero stands, errands reset only on the
  road or in a place, approaching a foe never targets a way out, and a chase never steps on
  one. 40,000 acts: knight 19-25, paladin 24, casters 19-26, 6-12 tales of 3 kinds done.
- Test: `land::tests::a_feud_changes_the_town` (siding: the other house leaves; peace: both
  stay; the town's notes).

## Regard (`regard.rs`; card adv-regard)
- `Game::regard` per town (-100..100), half of each change spreading to the towns of the same
  people; it fades a point a day. Causes (`regard::Cause`): striking one of its people (-25; Shift
  + a direction, `Action::Assault`: they turn into a fleeing townsman or the watchman they were,
  the watch is called), killing them (-60, a deed), a beast slain within six tiles (+35, every
  townsperson names the deed in their greeting), work done (+8), a tale betrayed (-30).
- Effects: prices (`regard::price_factor`, 0.85..1.4 on top of temper), greetings ("Friend of X"
  at 30+), talk barred at -40 or worse ("Get out of my sight", only Bye), the watch at the gate
  each time one walks into the town's tile (`at_the_gate`, three watchmen), bounty hunters on the
  road while a town within 12 tiles is at -30 or worse (`hunted_by`, a fifth of the tiles
  travelled). New monsters: watchman, townsman, bounty hunter (`Monster::town` marks a town's own).
  The panel says how the town here sees the hero.
- Test: `land::tests::the_town_remembers_blood_and_deeds`.

## Rooms and puzzles (`rooms.rs`, `data/defaults/adventure_rooms.json`; card adv-set-pieces)
- 23 authored rooms (flooded crypt, ossuary, hidden reliquary, throne of bones, lever vault,
  riddle chapel, plate corridor, spider nursery, beast's larder, collapsed gallery, dwarven
  forge, secret armoury, scriptorium, cult altar, drowned hall, fallen adventurers, statue
  garden, ritual circle, guard room, fountain court, prison cells, smugglers' cache, hermit's
  hole), each for some kinds of place and floors (any, first, last, deep), cut into solid rock
  where it fits after the ways down are placed (`site::realize`: none hides a stair), joined by
  a passage from its one way in (door, hidden door, riddle door or gap), else not cut. A room's
  text is told the first time one steps into it (`Place::rooms`); its M cells get the place's
  monsters.
- History in the walls: `Feature::Lore { text, look }` (appended; 0 an engraving, 1 remains
  walked over, 2 a plaque): a tomb's engraving is its cause (the battle and its dead; every tomb
  also carves it by the stair of its first and last floors), a temple's hymn its god, a ruin's
  or keep's who held it, a cult's the name it worships; remains in a lair are what its beast ate,
  elsewhere a dead adventurer's last word.
- Hidden doors (`Feature::SecretDoor`, a walled cell): noticed by a draft within two cells
  (8% + level/250 a step), or found by searching (`Action::Search`, X: 70% within three cells,
  200 ticks); found they are doors (`Stats::secrets`).
- Puzzles, one kind per kind of place: levers in an order cut in an engraving (labyrinths,
  halls, keeps: `Place::levers`, a wrong pull springs them all back, the last opens the vault's
  portcullis), a riddle door (temples, shrines, cellars: a choice card, 8 riddles; right opens it
  with experience, wrong spits fire), pressure plates (tombs, ruins, the dark fortress: darts
  unless one treads on the skull-marked ones).
- Bot: answers riddles right, pulls levers in their order, searches the walls of a walked floor
  that hides a door (12 tries). `Rooms:` line in its report; the 40,000-act knight sees 8+ rooms
  and finds hidden doors.
- Tests: `rooms::tests::rooms_are_whole`, `rooms::made::places_get_their_rooms` (rooms in 84
  places, hidden doors, a tomb naming its dead), the integration test's Rooms line.

## Fights with decisions (card adv-combat-depth)
- Flanking: each other foe beside the hero adds a fifth to a blow and splits the guard
  (`monster_strikes`: block 0..defense / (1 + flank)). A doorway holds: the hero a step inside a
  corridor meets one at a time (nothing cuts a corner between two walls).
- Wounds (`Hero::wounds`, part and turns left, 300): a blow of a seventh of one's life or more
  may hurt a leg (steps x1.5), an arm (the shield halved, blows x0.7) or the head (defence
  x0.7); they mend with time, a health potion mends one, a spring all. The panel lists them.
- Stances (V: balanced, defensive x1.5 defence and x0.7 blows, offensive the reverse) and
  sneaking (N: steps x1.5; sleeping things wake at 2 cells, not 5). A blow on the unaware is
  x2.5 (x3.5 with a dagger or fists): "You fall on X unawares!".
- Monsters: a breaking blow lames a leg (`slowed`) or maims a forelimb (`maimed`: blows x0.6);
  a kin slain within 6 cells frightens a third of the pack (`fear` 25 turns: they run, and turn
  when cornered); a boss under half its life calls two of its floor's kind once (`called`).
- Fire (`Floor::fire`, cells and turns left; `Tile::flammable`: trees, hedges, palisades,
  timber, grass, fields, moss, planks, carpet, webs): Ctrl + a direction with a lit torch, and
  fire spells' cells. It spreads by a hash of cell and turn (woods 18%, webs 30%, timber 12%,
  grass alone 3%: a meadow fire dies out, a wood burns), fewer catch the more burns (160 at
  once at most), burns what stands in it (8 a turn; the hero 10), and leaves ash (walls fall to
  ash or rubble, furniture burns). The land floor's fires go out when it is put together anew.
- Tests: `game::fight_tests::a_doorway_holds_where_the_open_does_not` (four winter wolves, a
  level-2 swordsman, 8 seeds: the open costs 2.5x the life, 7 of 8 die there and 1 in the
  doorway), `a_torch_sets_a_wood_burning` (a tree lit, 116 cells of wood ash, the wolf in it dead).

## Exploration that pays (`wonders.rs`; card adv-landmarks)
- Wonders (`WorldInfo::wonders`, by tile; `wonders::find` at build): the focal biomes
  (`lore::focal`) and the gazetteer's named peaks, one a patch (21 on the dev world, 7 kinds).
  Stamped near the tile's middle (`surface::stamp_wonder`) as `Feature::Wonder { kind, used }`
  (appended) and drawn three cells across over the plan (`adventure_ink::draw_wonder`). Bump
  for the gift (`Game::wonder`): the Eldest Tree's blessing (`Hero::boons` "grove", +10 life for
  good), a spring or oasis (all mended, fed), a summit's cairn (the map inked 7 tiles about,
  experience), a star-crater (star-iron), a fire mountain (a fire salamander climbs out; fire-
  glass), a giant's bones (a giant's bone; its wight rises), a monolith (the oldest event of
  the chronicle, experience), a deep well (a coin: paid back, a potion, or nothing).
- Stories on standing stones (`WorldInfo::stones`, `stories`): each war of the history with
  four located fights (`collections::war_events`: the first, two between, the last) is carved
  on four stones (`Feature::Lore` look 3) by where they were fought (40 wars, 160 stones on the
  dev world). Reading one marks the others on the map and says where the nearest is; all four
  tell the whole tale (experience, a deed; `Game::stones_read`, `Stats::stories`).
- No wild land is empty (`Gen::finds`): roads get a waystone naming the two nearest towns in
  days and a direction (`Atlas::names`), now and then a roadside inn (an innkeeper, a bed;
  not within two tiles of a town) or a wayside shrine (an altar: bump to mend); a tile still
  bare gets a cairn over a cache, a lone grave with its board, or an empty hut with a chest.
- Far sight: tall things (a castle's towers, the dark fortress, a temple's spire, a ruin's
  walls, a camp's smoke, a tomb's dome, a town's roofs, every wonder: `Gen::tall`, kept per chunk
  in `Game::tall` with `pristine`, `Game::far` in land cells) are seen from 64 cells by day
  (24 at night): their cells are inked and the nearest three named under the title ("Far off:
  the towers of Highkeep to the east"; `Game::far_seen`).
- `--adventure-landscape` adds a frame per kind of wonder and prints them.
- Tests: `surface::tests::every_land_has_something`, `land::tests::a_tower_is_seen_from_far_off`,
  `land::tests::four_stones_tell_a_tale`.

## A voice for the log (`prose.rs`; card adv-voice)
- Arrival paragraphs (`Game::arrival`): the first time one stands in a land (walking in, or
  coming down off the world map; `Game::lands_told`), the log says what it is like: the land's
  kind, the season (barren lands get their own lines), the hour and the weather, what the eye
  falls on (the nearest tall thing far off, a wonder, a river, a road) and what is wrong with it
  (the Shadow, danger, a town's smoke near). Each sentence is picked by a hash of the tile and
  the day: the same walk reads the same, two lands never alike.
- A town's mood (`Game::town_mood`) as one walks in: at war (the history's open wars on its
  people: "the walls are manned"), a sickness (an open plague tale), a feast or market day, its
  notes; how they look at the hero (regard); and "Travellers from X say:" the nearest town's
  latest note within 8 tiles, so what the hero did in one town is heard in the next.
- A place's first entry (`Game::place_arrival`, `Game::entered`): a line by its kind and a
  warning by its tier. Blows vary (`Game::blow_word`).
- Repeats collapse: `Game::say` counts a line said again (`Line::n`, drawn "(x3)").
- Tests: `prose::tests::five_lands_read_five_ways`, `the_log_does_not_stutter` (4,000 bot acts,
  no line four times running).

## Weather and the cold (`weather.rs`; card adv-land-weather)
- `Game::weather_at(tile)`: clear, cloudy, rain, storm, snow or fog, a hash of the region (3 x 3
  tiles) and the half-day weighted by the land's moisture and kind (fog in marsh and lake
  country, little rain in deserts); snow where the air is below 1 degree. The air
  (`temperature_at`) is the tile's, the season's (spring 0, summer +6, autumn -2, winter -10;
  the season from the living history's calendar, else the clock) and the night's (-5).
  `Game::weather_set` holds it fixed (tests, the landscape tool).
- Effects: sight under the sky x0.5 in fog, x0.6 storm, x0.7 snow, x0.85 rain
  (`Game::sight_radius`; never below the torch's reach); far sight cut the same way (none in
  fog); rain turns earth and grass to mud (steps x1.2) and puts fires out; snow shows a nearby
  unseen creature's fresh tracks and their heading.
- The cold (`Game::freezing`): on cold land at night (or below -12 by day), with no `furs` in the
  pack (the trader sells them) and no campfire or brazier within 3 cells, the hero does not heal
  and loses a point every 6 turns, down to a quarter of their life (numb, not dead); rest is
  refused. B makes a campfire with a torch (`Action::Camp`, appended); resting within 3 cells of
  a fire mends twice as fast.
- Window: the weather in the title and status, fog thickening beyond one's sight, rain streaks,
  snow; `--adventure-landscape` adds fog, rain and snow frames of the home land.
- Tests: `weather::tests::fog_halves_sight`, `a_night_in_the_snow_without_a_fire_costs_life`.

## The window (`tiles/adventure.rs`, `tiles/adventure_ink.rs`)
- The place as an inked plan (`adventure_ink::Plan`, kept per floor and zoom, cells redrawn when
  their `cell_sig` changes; a region keyed on world cells: a small floor whole, the land's the
  view and 24 cells about it, `rebase`d as the camera moves, so a re-centred land is not
  redrawn), light and fog per pixel (unseen underground dark, outdoors a
  parchment haze; remembered faded; in sight lit by the light's reach), things, corpses,
  townsfolk (`folk` by role), monsters (`beasts::of_name` / `of_monster`, `folk` for people of
  the bestiary "folk:RACE:HELM:ARM") with life bars, the companion, the adventurer (dressed in
  what they wear), effects (numbers, missiles, areas, speech; only in sight).
- Panel: name, level, life/mana/experience bars, status, paper doll, attack/defense/armour,
  tabs Pack (click to use or wear, right click to drop; hover describes), Skills, Spells, Quests.
  The log bottom-left; talk and quest-chest cards; the death or victory banner; ? the keys.
- World map: `render_world_cached` under the Mapmaker's fog (blank, sketched, inked), the route
  dotted, crosses, known places marked by kind with names (towns first, none over another), the
  adventurer. Land: the title says the region and the time of day; the night tint; T travels.
  Frames: 1-2.4 ms kept, ~15 ms the first frame of a floor.

## Balance (dev world, 2026-10-09)
- Bot, 60,000 acts: knight, paladin, sorcerer and druid all level 25, no deaths (ambushes are
  as dangerous as the land but never past the walker: tier <= 1 + level/7; before that cap,
  cyclopes on the road killed mid-level heroes nine times). Most experience comes from tasks
  (bounties, deliveries) and grinding returning monsters; bosses ~6. Gold sinks: blessing, the
  smith, sellswords, spells, runes.
- Runes (Tibia): one-use spells for anyone (`kind: rune`, `spell`), sold by the sage by level
  and found in deep treasure: flame strike, divine missile, intense healing, stone shower, great
  fireball. Level doors (`Feature::LevelDoor`) wall off a treasure room on the last floor of
  places of tier 3+. Leaving writes the adventurer's legend (`Game::legend_html`, deeds kept in
  `Game::deeds`; `PLANET_ADV_LEGEND=FILE` from the bot).
- Exploits found and closed by the bot: parcels between two towns (one per pair, cooldown,
  small experience), the same quest for the same beast (slain bosses remembered).

## From the first real play (2026-10-09)
- A halfling commoner died at level 2 in the upper sewer: a 30-exchange fight with a cave spider
  at 1-4 damage a blow, "Too far to strike" when targeting at range, hungry (the hint named a key
  that does not exist), then a bat. Now: attacking out of reach walks toward the foe (Tibia's
  chase); every blow does a third of its best at least; a commoner has 80 life and sets out fed;
  the upper sewer holds rats and the odd bat (spiders and cave rats below); quests are offered
  from four levels under their place's level, and the priest's only for the risen dead.

## Not yet
- Houses, a bank; boats over the sea; rivers through towns (a town
  is laid over its river).
