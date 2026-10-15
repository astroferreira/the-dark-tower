# The `mystery_box` branch: what is worth taking (written 2026-10-12)

The branch (`git log mystery_box`, last commit 0b0d272 "mistery box") holds an older game layer
under `src/game/`: a fortress mode and an "Island 815" survival mode, with a ratatui UI. It
diverged from today's code before the colony (`src/colony/`), the tile viewer and adventure mode
were written, and it should not be merged. Update 3 ("the settlement comes alive"), which this
note was first asked for, has since been built another way: settlers choose their own work by
utility (`colony/mod.rs::decide`, `needs.rs`), and the patron gives favour, not orders. So most of
the branch is superseded. Read module by module, what is left worth taking is small.

## Worth taking
- **Fluids (`src/game/fluids.rs`, 258 lines).** A DF-style water and magma grid: levels 0-7
  per cell, water falls first, then spreads to one lower neighbour a tick (N, S, E, W), dirty
  z-levels skipped, `create_pool`, total volumes. The colony has no fluid simulation: a breached
  aquifer, a flood or the magma sea is an event and a stamped result (`dig.rs`, `deep.rs`), and
  the cards 'Rivers freeze and flood in game time', 'Fields on the floodplain, and spring floods'
  and 'Dig too deep' ask for water that moves. The grid fits `LocalMap` (its cells already carry
  `water: u8` levels) if it runs only over a dirty window round a breach or a riverbank, not
  every cell every tick. Take the idea and the level rules, not the code (it is written against
  `LocalFortressMap`).
- **Anomalies (`src/game/anomalies.rs`, 198 lines).** It maps each strange biome to a named
  anomaly (crystal formations, glowing grove, gravity anomaly, humming rocks, impact crater,
  rainbow springs, persistent fog, glass flats, hot springs, columnar basalt, tar seeps, toxic
  vents, bone field, spore cloud, mega skeleton) and groups them by island. This is useful
  wording for the card 'Causes for the remaining anomaly biomes' and for embark places, where
  such a tile should be something to walk to.
- **Archipelago (`src/game/archipelago.rs`, 352 lines).** It finds islands, sorts them by size
  and measures BFS sea distances between island centres. Adventure mode's passage by sea
  (`Game::passages`) prices a voyage by straight distance; distances measured over the sea would
  be truer where a continent lies between two ports.

## Superseded (no need to take)
- Jobs and labors (`jobs.rs`, `designations.rs`, `crew_manager.rs`): a job queue fed by the
  player's designations, labours matched by skill. The colony has no orders by design (decided
  2026-10-05: watched like Dwarf Fortress, but the settlers choose); skills and roles are in
  `Settler::skill` / `role`, and the patron's marks (bless, forbid) take the place of designations.
- Items, crafting, buildings and materials (`items.rs`, `crafting.rs`, `buildings.rs`,
  `materials.rs`): replaced by `colony::Item` / `Stuff`, `industry.rs` (ore to bars to tools and
  arms), `craft.rs`, `projects.rs` (works with a reason) and `src/materials.rs` (densities,
  hardness, edges, tissue layers).
- Crew, survival, origins, shelter, boats, tower and chat (`crew.rs`, `survival.rs`,
  `origins.rs`, `shelter.rs`, `boat.rs`, `tower.rs`, `chat.rs`): the Island 815 mode. Settlers'
  pasts come from the history (`history/settlers.rs`), adventure mode has needs, the cold, a
  boat across the sea and talk from what a town knows (`src/adventure/`).
- Its UI (`src/game/ui/`): ratatui, like the frozen legacy explorer.
