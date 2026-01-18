//! Fauna system - passive and neutral wildlife
//!
//! Provides ambient life to the world, hunting resources for tribes,
//! and natural ecosystem dynamics.

pub mod types;
pub mod behavior;

pub use types::{
    Fauna, FaunaActivity, FaunaDiet, FaunaId, FaunaSpecies, FaunaState, FaunaStats,
    ALL_FAUNA_SPECIES,
};
pub use behavior::{find_breeding_partner, process_fauna_behavior};

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::biomes::ExtendedBiome;
use crate::simulation::monsters::{Monster, MonsterId};
use crate::simulation::tribe::Tribe;
use crate::simulation::types::{GlobalLocalCoord, TileCoord, TribeId};
use crate::world::WorldData;

/// Fauna spawning parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaunaSpawnParams {
    /// Maximum total fauna in the world
    pub max_fauna: usize,
    /// Spawn interval (ticks between spawn attempts)
    pub spawn_interval: u64,
    /// Base spawn chance per attempt
    pub base_spawn_chance: f32,
    /// Whether to spawn fauna in herds
    pub herd_spawning: bool,
}

impl Default for FaunaSpawnParams {
    fn default() -> Self {
        FaunaSpawnParams {
            max_fauna: 2000,
            spawn_interval: 2,
            base_spawn_chance: 0.4,
            herd_spawning: true,
        }
    }
}

/// Manager for all fauna in the simulation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaunaManager {
    /// All fauna creatures
    pub fauna: HashMap<FaunaId, Fauna>,
    /// Spatial index: tile -> fauna IDs
    pub fauna_map: HashMap<TileCoord, Vec<FaunaId>>,
    /// Next fauna ID
    next_id: u32,
    /// Spawning parameters
    pub spawn_params: FaunaSpawnParams,
}

impl FaunaManager {
    pub fn new() -> Self {
        FaunaManager {
            fauna: HashMap::new(),
            fauna_map: HashMap::new(),
            next_id: 0,
            spawn_params: FaunaSpawnParams::default(),
        }
    }

    /// Get the next fauna ID
    fn next_fauna_id(&mut self) -> FaunaId {
        let id = FaunaId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Spawn a fauna creature at a location
    pub fn spawn(
        &mut self,
        species: FaunaSpecies,
        location: TileCoord,
        is_male: bool,
        current_tick: u64,
    ) -> FaunaId {
        let id = self.next_fauna_id();
        let creature = Fauna::new(id, species, location, is_male, current_tick);
        self.fauna.insert(id, creature);

        // Update spatial index
        self.fauna_map
            .entry(location)
            .or_insert_with(Vec::new)
            .push(id);

        id
    }

    /// Try to spawn fauna somewhere in the world
    pub fn try_spawn<R: Rng>(
        &mut self,
        world: &WorldData,
        territory_map: &HashMap<TileCoord, TribeId>,
        current_tick: u64,
        rng: &mut R,
    ) -> Option<FaunaId> {
        // Check population cap
        if self.living_count() >= self.spawn_params.max_fauna {
            return None;
        }

        // Random spawn chance
        if rng.gen::<f32>() > self.spawn_params.base_spawn_chance {
            return None;
        }

        // Find a suitable spawn location
        let max_attempts = 50;
        for _ in 0..max_attempts {
            let x = rng.gen_range(0..world.width);
            let y = rng.gen_range(0..world.height);
            let coord = TileCoord::new(x, y);

            let elevation = *world.heightmap.get(x, y);
            let biome = *world.biomes.get(x, y);

            // Determine valid species for this biome
            let valid_species: Vec<FaunaSpecies> = ALL_FAUNA_SPECIES
                .iter()
                .filter(|s| s.can_spawn_in(biome))
                .copied()
                .collect();

            if valid_species.is_empty() {
                continue;
            }

            // Select species weighted by rarity
            let species = select_species_weighted(&valid_species, rng);

            // Aquatic species can spawn in water
            let is_aquatic = matches!(
                species,
                FaunaSpecies::Fish
                    | FaunaSpecies::Salmon
                    | FaunaSpecies::Crab
                    | FaunaSpecies::Seal
            );

            if !is_aquatic && elevation < 0.0 {
                continue;
            }

            // Avoid spawning in heavily populated tribe territories
            if let Some(&tribe_id) = territory_map.get(&coord) {
                // Small chance to skip tribe territory
                if rng.gen::<f32>() < 0.7 {
                    continue;
                }
            }

            // Spawn the creature
            let is_male = rng.gen::<bool>();
            let id = self.spawn(species, coord, is_male, current_tick);

            // Spawn herd if applicable
            if self.spawn_params.herd_spawning {
                let stats = species.stats();
                let herd_size =
                    rng.gen_range(stats.herd_size_min..=stats.herd_size_max) as usize;

                // Spawn additional herd members nearby
                for _ in 1..herd_size {
                    if self.living_count() >= self.spawn_params.max_fauna {
                        break;
                    }

                    let offset_x = rng.gen_range(-2..=2);
                    let offset_y = rng.gen_range(-2..=2);
                    let nx = (x as i32 + offset_x).rem_euclid(world.width as i32) as usize;
                    let ny = (y as i32 + offset_y).clamp(0, world.height as i32 - 1) as usize;
                    let neighbor_coord = TileCoord::new(nx, ny);

                    let neighbor_elevation = *world.heightmap.get(nx, ny);
                    if (!is_aquatic && neighbor_elevation >= 0.0)
                        || (is_aquatic && neighbor_elevation < 0.0)
                    {
                        let herd_is_male = rng.gen::<bool>();
                        self.spawn(species, neighbor_coord, herd_is_male, current_tick);
                    }
                }
            }

            return Some(id);
        }

        None
    }

    /// Process fauna behavior for one tick
    pub fn process_behavior<R: Rng>(
        &mut self,
        tribes: &HashMap<TribeId, Tribe>,
        territory_map: &HashMap<TileCoord, TribeId>,
        monsters: &HashMap<MonsterId, Monster>,
        world: &WorldData,
        current_tick: u64,
        focus_point: Option<GlobalLocalCoord>,
        world_width: usize,
        rng: &mut R,
    ) {
        process_fauna_behavior(
            &mut self.fauna,
            tribes,
            territory_map,
            monsters,
            world,
            current_tick,
            focus_point,
            world_width,
            rng,
        );

        // Update spatial index after movement
        self.update_spatial_index();

        // Process breeding
        self.process_breeding(current_tick, world_width, rng);
    }

    /// Process breeding for all fauna
    fn process_breeding<R: Rng>(
        &mut self,
        current_tick: u64,
        world_width: usize,
        rng: &mut R,
    ) {
        // Collect breeding pairs
        let mut new_fauna: Vec<(FaunaSpecies, TileCoord)> = Vec::new();

        for (id, creature) in &self.fauna {
            if creature.is_dead() || creature.state != FaunaState::Breeding {
                continue;
            }
            if !creature.can_breed(current_tick) {
                continue;
            }

            // Find partner
            if let Some(_partner_id) =
                find_breeding_partner(creature, &self.fauna, current_tick, world_width)
            {
                // Successful breeding
                let stats = creature.species.stats();
                for _ in 0..stats.offspring_count.min(5) {
                    // Cap offspring per tick
                    if new_fauna.len() < 20 {
                        // Cap total new fauna per tick
                        new_fauna.push((creature.species, creature.location));
                    }
                }
            }
        }

        // Spawn offspring
        for (species, location) in new_fauna {
            if self.living_count() < self.spawn_params.max_fauna {
                let is_male = rng.gen::<bool>();
                self.spawn(species, location, is_male, current_tick);
            }
        }

        // Update breed ticks for creatures that bred
        for creature in self.fauna.values_mut() {
            if creature.state == FaunaState::Breeding {
                creature.last_breed_tick = current_tick;
                creature.state = FaunaState::Idle;
            }
        }
    }

    /// Update the spatial index after fauna movement
    fn update_spatial_index(&mut self) {
        self.fauna_map.clear();
        for (id, creature) in &self.fauna {
            if !creature.is_dead() {
                self.fauna_map
                    .entry(creature.location)
                    .or_insert_with(Vec::new)
                    .push(*id);
            }
        }
    }

    /// Get fauna at a specific tile
    pub fn get_at(&self, coord: &TileCoord) -> Vec<&Fauna> {
        self.fauna_map
            .get(coord)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.fauna.get(id))
                    .filter(|f| !f.is_dead())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get mutable fauna by ID
    pub fn get_mut(&mut self, id: &FaunaId) -> Option<&mut Fauna> {
        self.fauna.get_mut(id)
    }

    /// Count living fauna
    pub fn living_count(&self) -> usize {
        self.fauna.values().filter(|f| !f.is_dead()).count()
    }

    /// Clean up dead fauna
    pub fn cleanup_dead(&mut self) {
        self.fauna.retain(|_, f| !f.is_dead());
        self.update_spatial_index();
    }

    /// Hunt fauna at a location, returning food and material values
    pub fn hunt_at<R: Rng>(
        &mut self,
        coord: &TileCoord,
        hunting_skill: f32,
        rng: &mut R,
    ) -> (f32, f32) {
        let mut food = 0.0;
        let mut materials = 0.0;

        if let Some(ids) = self.fauna_map.get(coord).cloned() {
            for id in ids {
                if let Some(creature) = self.fauna.get_mut(&id) {
                    if creature.is_dead() {
                        continue;
                    }

                    let stats = creature.species.stats();

                    // Hunt success based on alertness vs hunting skill
                    let success_chance = hunting_skill / (stats.alertness + hunting_skill);
                    if rng.gen::<f32>() < success_chance {
                        // Successful hunt
                        food += stats.food_value;
                        materials += stats.material_value;
                        creature.take_damage(creature.max_health); // Kill
                        break; // One hunt per call
                    }
                }
            }
        }

        (food, materials)
    }

    /// Get fauna near a location
    pub fn get_fauna_near(
        &self,
        coord: &TileCoord,
        radius: usize,
        world_width: usize,
    ) -> Vec<&Fauna> {
        let mut result = Vec::new();
        for creature in self.fauna.values() {
            if creature.is_dead() {
                continue;
            }
            if creature.distance_to(coord, world_width) <= radius {
                result.push(creature);
            }
        }
        result
    }

    /// Get species counts for statistics
    pub fn species_counts(&self) -> HashMap<FaunaSpecies, usize> {
        let mut counts = HashMap::new();
        for creature in self.fauna.values() {
            if !creature.is_dead() {
                *counts.entry(creature.species).or_insert(0) += 1;
            }
        }
        counts
    }
}

impl Default for FaunaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Select a species weighted by rarity (rarer = less likely)
fn select_species_weighted<R: Rng>(species: &[FaunaSpecies], rng: &mut R) -> FaunaSpecies {
    // Convert rarity to weight (lower rarity = higher weight)
    let weights: Vec<f32> = species.iter().map(|s| 1.0 / s.rarity() as f32).collect();

    let total_weight: f32 = weights.iter().sum();
    let mut roll = rng.gen::<f32>() * total_weight;

    for (i, &weight) in weights.iter().enumerate() {
        roll -= weight;
        if roll <= 0.0 {
            return species[i];
        }
    }

    species[0] // Fallback
}
