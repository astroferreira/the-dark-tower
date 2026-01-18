//! Monster spawning system - biome-based monster generation

use rand::Rng;
use std::collections::HashMap;

use crate::simulation::types::TileCoord;
use crate::simulation::monsters::types::{Monster, MonsterId, MonsterSpecies, ALL_SPECIES};
use crate::world::WorldData;

/// Parameters for monster spawning
#[derive(Clone, Debug)]
pub struct MonsterSpawnParams {
    /// Maximum monsters in the world
    pub max_monsters: usize,
    /// Minimum distance from tribe territory to spawn
    pub min_tribe_distance: usize,
    /// Spawn check interval in ticks
    pub spawn_interval: u64,
    /// Base spawn chance per tick (0.0 - 1.0)
    pub base_spawn_chance: f32,
}

impl Default for MonsterSpawnParams {
    fn default() -> Self {
        MonsterSpawnParams {
            max_monsters: 50,
            min_tribe_distance: 5,
            spawn_interval: 4,
            base_spawn_chance: 0.15,
        }
    }
}

/// Try to spawn a monster in the world
pub fn try_spawn_monster<R: Rng>(
    world: &WorldData,
    existing_monsters: &HashMap<MonsterId, Monster>,
    territory_map: &HashMap<TileCoord, crate::simulation::types::TribeId>,
    params: &MonsterSpawnParams,
    next_monster_id: &mut u32,
    current_tick: u64,
    rng: &mut R,
) -> Option<Monster> {
    // Check if we're at the monster limit
    let living_monsters = existing_monsters.values().filter(|m| !m.is_dead()).count();
    if living_monsters >= params.max_monsters {
        return None;
    }

    // Random spawn chance check
    if rng.gen::<f32>() > params.base_spawn_chance {
        return None;
    }

    // Find a valid spawn location
    let max_attempts = 50;
    for _ in 0..max_attempts {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);
        let coord = TileCoord::new(x, y);

        // Check elevation (no spawning in water)
        let elevation = *world.heightmap.get(x, y);
        if elevation < 0.0 {
            continue;
        }

        // Check distance from tribe territory
        let too_close_to_tribe = is_near_tribe_territory(&coord, territory_map, params.min_tribe_distance, world.width);
        if too_close_to_tribe {
            continue;
        }

        // Check distance from other monsters
        let too_close_to_monster = existing_monsters.values().any(|m| {
            !m.is_dead() && m.distance_to(&coord, world.width) < 3
        });
        if too_close_to_monster {
            continue;
        }

        // Get biome and find valid species
        let biome = *world.biomes.get(x, y);
        let valid_species: Vec<MonsterSpecies> = ALL_SPECIES
            .iter()
            .filter(|s| s.can_spawn_in(biome))
            .copied()
            .collect();

        if valid_species.is_empty() {
            continue;
        }

        // Select species based on rarity
        if let Some(species) = select_species_by_rarity(&valid_species, rng) {
            let id = MonsterId(*next_monster_id);
            *next_monster_id += 1;

            let monster = Monster::new(id, species, coord, current_tick);
            return Some(monster);
        }
    }

    None
}

/// Check if a coordinate is near tribe territory
fn is_near_tribe_territory(
    coord: &TileCoord,
    territory_map: &HashMap<TileCoord, crate::simulation::types::TribeId>,
    min_distance: usize,
    map_width: usize,
) -> bool {
    // Quick check - is the coordinate itself in territory?
    if territory_map.contains_key(coord) {
        return true;
    }

    // Check tiles within min_distance
    for &(dx, dy) in &[(-1i32, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        for dist in 1..=min_distance {
            let nx = (coord.x as i32 + dx * dist as i32).rem_euclid(map_width as i32) as usize;
            let ny = (coord.y as i32 + dy * dist as i32).max(0) as usize;
            let neighbor = TileCoord::new(nx, ny);
            if territory_map.contains_key(&neighbor) {
                return true;
            }
        }
    }

    false
}

/// Select a species based on rarity weights
fn select_species_by_rarity<R: Rng>(species: &[MonsterSpecies], rng: &mut R) -> Option<MonsterSpecies> {
    if species.is_empty() {
        return None;
    }

    // Calculate total weight (inverse of rarity)
    let weights: Vec<f32> = species.iter().map(|s| 1.0 / s.rarity() as f32).collect();
    let total_weight: f32 = weights.iter().sum();

    // Random selection
    let mut roll = rng.gen::<f32>() * total_weight;
    for (i, weight) in weights.iter().enumerate() {
        roll -= weight;
        if roll <= 0.0 {
            return Some(species[i]);
        }
    }

    species.last().copied()
}

/// Get all species that can spawn in a specific biome
pub fn species_for_biome(biome: crate::biomes::ExtendedBiome) -> Vec<MonsterSpecies> {
    ALL_SPECIES
        .iter()
        .filter(|s| s.can_spawn_in(biome))
        .copied()
        .collect()
}
