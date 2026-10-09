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
- `--adventure-snapshot PREFIX` (temple, talk, sewer, world, surface, deep, boss frames by the
  bot's play) and `--adventure-gallery PREFIX` (one revealed floor of each kind of place).
- Tests: `tests/adventure.rs` (a commoner grows: level 12+, a calling, bosses, quests, chests,
  never stuck in the first half; the same seed twice; a save loads whole and plays on the same),
  unit tests in `site.rs` (13 kinds x 6 seeds: every way down reachable, deterministic),
  `town.rs`, `hero.rs`, `item.rs`, `data.rs`.

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
  ambushes by the land's danger as a Wilds floor), the companion, saving and loading (bincode,
  header `ADVENT01` with the world's size).
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

## The window (`tiles/adventure.rs`, `tiles/adventure_ink.rs`)
- The place as an inked plan (`adventure_ink::Plan`, kept per floor and zoom, cells redrawn when
  their `cell_sig` changes), light and fog per pixel (unseen underground dark, outdoors a
  parchment haze; remembered faded; in sight lit by the light's reach), things, corpses,
  townsfolk (`folk` by role), monsters (`beasts::of_name` / `of_monster`, `folk` for people of
  the bestiary "folk:RACE:HELM:ARM") with life bars, the companion, the adventurer (dressed in
  what they wear), effects (numbers, missiles, areas, speech; only in sight).
- Panel: name, level, life/mana/experience bars, status, paper doll, attack/defense/armour,
  tabs Pack (click to use or wear, right click to drop; hover describes), Skills, Spells, Quests.
  The log bottom-left; talk and quest-chest cards; the death or victory banner; ? the keys.
- World map: `render_world_cached` with known places marked by kind (inked), names near, the
  adventurer. Frames: 1-2.3 ms kept, ~15 ms the first frame of a floor.

## Balance (dev world, 2026-10-09)
- Bot, 60,000 acts: knight 38, paladin 38, sorcerer 41, druid 41; 0-4 deaths. 150,000 acts:
  level 53. Most experience comes from tasks (bounties, deliveries) and grinding returning
  monsters; bosses ~6. Gold piles up (gold sinks: blessing, the smith, sellswords, spells).
- Exploits found and closed by the bot: parcels between two towns (one per pair, cooldown,
  small experience), the same quest for the same beast (slain bosses remembered).

## Not yet
- Factions' regard (killing a people's soldiers angers their towns), level doors, runes,
  houses, a bank, night and day, the world's wars and the Shadow moving while one plays.
