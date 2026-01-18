//! Monster system - creatures that roam, attack tribes, and fight each other

pub mod types;
pub mod spawning;
pub mod behavior;
pub mod combat;

use std::collections::HashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::simulation::types::{TileCoord, TribeId, SimTick};
use crate::simulation::tribe::Tribe;
use crate::world::WorldData;

pub use types::{Monster, MonsterId, MonsterSpecies, MonsterState, AttackTarget};
pub use spawning::MonsterSpawnParams;
pub use combat::CombatEvent;

/// Manager for all monsters in the simulation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MonsterManager {
    pub monsters: HashMap<MonsterId, Monster>,
    pub monster_map: HashMap<TileCoord, MonsterId>,
    pub next_monster_id: u32,
    #[serde(skip)]
    pub spawn_params: MonsterSpawnParams,
}

impl MonsterManager {
    pub fn new() -> Self {
        MonsterManager {
            monsters: HashMap::new(),
            monster_map: HashMap::new(),
            next_monster_id: 0,
            spawn_params: MonsterSpawnParams::default(),
        }
    }

    /// Try to spawn a new monster
    pub fn try_spawn<R: Rng>(
        &mut self,
        world: &WorldData,
        territory_map: &HashMap<TileCoord, TribeId>,
        current_tick: u64,
        rng: &mut R,
    ) -> Option<MonsterId> {
        let monster = spawning::try_spawn_monster(
            world,
            &self.monsters,
            territory_map,
            &self.spawn_params,
            &mut self.next_monster_id,
            current_tick,
            rng,
        )?;

        let id = monster.id;
        let location = monster.location;
        self.monsters.insert(id, monster);
        self.monster_map.insert(location, id);

        Some(id)
    }

    /// Process behavior for all monsters
    pub fn process_behavior<R: Rng>(
        &mut self,
        tribes: &HashMap<TribeId, Tribe>,
        territory_map: &HashMap<TileCoord, TribeId>,
        world: &WorldData,
        current_tick: u64,
        rng: &mut R,
    ) {
        behavior::process_monster_behavior(
            &mut self.monsters,
            tribes,
            territory_map,
            world,
            current_tick,
            rng,
        );

        // Update spatial index
        self.update_monster_map();
    }

    /// Process combat for all monsters
    pub fn process_combat<R: Rng>(
        &mut self,
        tribes: &mut HashMap<TribeId, Tribe>,
        territory_map: &HashMap<TileCoord, TribeId>,
        world: &WorldData,
        current_tick: SimTick,
        rng: &mut R,
    ) -> Vec<CombatEvent> {
        // Find monster vs monster targets
        combat::find_monster_combat_targets(&mut self.monsters, world, rng);

        // Process all combat
        combat::process_monster_combat(
            &mut self.monsters,
            tribes,
            territory_map,
            world,
            current_tick,
            rng,
        )
    }

    /// Update the spatial index for monster positions
    fn update_monster_map(&mut self) {
        self.monster_map.clear();
        for (id, monster) in &self.monsters {
            if !monster.is_dead() {
                self.monster_map.insert(monster.location, *id);
            }
        }
    }

    /// Get monster at a location
    pub fn get_at(&self, coord: &TileCoord) -> Option<&Monster> {
        self.monster_map
            .get(coord)
            .and_then(|id| self.monsters.get(id))
    }

    /// Get monster by ID
    pub fn get(&self, id: MonsterId) -> Option<&Monster> {
        self.monsters.get(&id)
    }

    /// Remove dead monsters
    pub fn cleanup_dead(&mut self) {
        // Collect dead monster IDs
        let dead_ids: Vec<MonsterId> = self.monsters
            .iter()
            .filter(|(_, m)| m.is_dead())
            .map(|(id, _)| *id)
            .collect();

        // Remove dead monsters
        for id in dead_ids {
            if let Some(monster) = self.monsters.remove(&id) {
                self.monster_map.remove(&monster.location);
            }
        }
    }

    /// Get count of living monsters
    pub fn living_count(&self) -> usize {
        self.monsters.values().filter(|m| !m.is_dead()).count()
    }

    /// Get all living monsters
    pub fn living_monsters(&self) -> Vec<&Monster> {
        self.monsters.values().filter(|m| !m.is_dead()).collect()
    }
}
