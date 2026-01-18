//! Migration system - tribe splitting and movement

use rand::Rng;

use crate::simulation::types::{TribeId, TileCoord, TribeEventType};
use crate::simulation::params::SimulationParams;
use crate::simulation::simulation::SimulationState;
use crate::simulation::tribe::{Tribe, TribeCulture, Settlement};
use crate::simulation::tribe::culture::generate_tribe_name;
use crate::world::WorldData;
use crate::biomes::ExtendedBiome;

/// Process migration and tribe splitting for a tick
pub fn process_migration_tick<R: Rng>(
    state: &mut SimulationState,
    world: &WorldData,
    params: &SimulationParams,
    rng: &mut R,
) {
    let tribe_ids: Vec<TribeId> = state.tribes.keys().copied().collect();

    for tribe_id in tribe_ids {
        let should_split = {
            let tribe = match state.tribes.get(&tribe_id) {
                Some(t) if t.is_alive => t,
                _ => continue,
            };

            // Check split conditions
            tribe.population.total() >= params.tribe_split_population
                && tribe.population.migration_pressure() > 0.5
                && tribe.needs.food.satisfaction < 0.6 // Resource pressure
        };

        if should_split {
            if let Some(new_tribe_id) = split_tribe(state, tribe_id, world, params, rng) {
                // Record event in parent tribe
                if let Some(parent) = state.tribes.get_mut(&tribe_id) {
                    parent.record_event(
                        state.current_tick,
                        TribeEventType::TribeSplit { new_tribe: new_tribe_id },
                    );
                }
            }
        }
    }
}

/// Split a tribe into two
fn split_tribe<R: Rng>(
    state: &mut SimulationState,
    parent_id: TribeId,
    world: &WorldData,
    params: &SimulationParams,
    rng: &mut R,
) -> Option<TribeId> {
    // Find a location for the new tribe
    let new_capital = find_split_location(state, parent_id, world, params, rng)?;

    // Get parent tribe data
    let (culture, split_pop, split_resources, new_name) = {
        let parent = state.tribes.get_mut(&parent_id)?;

        // Calculate what the new tribe gets
        let split_fraction = 0.3; // 30% splits off
        let split_pop = parent.population.split(split_fraction);
        let split_resources = parent.stockpile.take_fraction(split_fraction);

        // Slightly mutate culture
        let mut new_culture = parent.culture.clone();
        mutate_culture(&mut new_culture, rng);

        let biome = *world.biomes.get(new_capital.x, new_capital.y);
        let name = generate_tribe_name(&new_culture, biome, rng);

        (new_culture, split_pop, split_resources, name)
    };

    // Create new tribe
    let new_id = TribeId(state.next_tribe_id);
    state.next_tribe_id += 1;

    let mut new_tribe = Tribe::new(
        new_id,
        new_name,
        new_capital,
        split_pop.total(),
        culture,
    );

    // Transfer resources
    new_tribe.stockpile.add_all(&split_resources);

    // Claim territory around new capital
    claim_split_territory(state, new_id, new_capital, world, params);

    // Initialize relations (starts friendly with parent)
    state.diplomacy.set_relation(parent_id, new_id, crate::simulation::types::RelationLevel(30));

    // Copy neighbor relations (slightly worse)
    let parent_neighbors: Vec<(TribeId, i8)> = state.diplomacy
        .get_related_tribes(parent_id)
        .iter()
        .map(|(id, rel)| (*id, (rel.0 as i16 - 10).clamp(-100, 100) as i8))
        .collect();

    for (neighbor_id, relation) in parent_neighbors {
        if neighbor_id != new_id {
            state.diplomacy.set_relation(new_id, neighbor_id, crate::simulation::types::RelationLevel(relation));
        }
    }

    // Record founding event
    new_tribe.record_event(
        state.current_tick,
        TribeEventType::Founded { location: new_capital },
    );

    state.tribes.insert(new_id, new_tribe);
    state.stats.total_tribes_created += 1;

    Some(new_id)
}

/// Find a suitable location for split tribe capital
fn find_split_location<R: Rng>(
    state: &SimulationState,
    parent_id: TribeId,
    world: &WorldData,
    params: &SimulationParams,
    rng: &mut R,
) -> Option<TileCoord> {
    let parent = state.tribes.get(&parent_id)?;

    // Find unclaimed tiles within reasonable distance of parent
    let mut candidates: Vec<TileCoord> = Vec::new();

    for &coord in &parent.territory {
        // Check surrounding area
        for dx in -10i32..=10 {
            for dy in -10i32..=10 {
                let nx = (coord.x as i32 + dx).rem_euclid(world.heightmap.width as i32) as usize;
                let ny = (coord.y as i32 + dy).clamp(0, world.heightmap.height as i32 - 1) as usize;

                let candidate = TileCoord::new(nx, ny);

                // Skip if already owned
                if state.territory_map.contains_key(&candidate) {
                    continue;
                }

                // Skip water
                let elevation = *world.heightmap.get(nx, ny);
                if elevation < 0.0 {
                    continue;
                }

                // Skip unsuitable biomes
                let biome = *world.biomes.get(nx, ny);
                if !is_habitable(biome) {
                    continue;
                }

                // Must be far enough from parent capital
                let dist = candidate.distance_wrapped(&parent.capital, world.heightmap.width);
                if dist >= params.min_tribe_separation / 2 && dist <= params.min_tribe_separation * 2 {
                    candidates.push(candidate);
                }
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Pick random candidate with cultural preference weighting
    let culture = &parent.culture;
    let mut weighted: Vec<(TileCoord, f32)> = candidates
        .into_iter()
        .map(|coord| {
            let biome = *world.biomes.get(coord.x, coord.y);
            let elevation = *world.heightmap.get(coord.x, coord.y);
            let pref = culture.terrain_preference(biome, elevation, false);
            (coord, pref)
        })
        .collect();

    weighted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Pick from top candidates with some randomness
    let top_count = (weighted.len() / 4).max(1);
    let choice = rng.gen_range(0..top_count.min(weighted.len()));
    Some(weighted[choice].0)
}

/// Claim territory for a split tribe
fn claim_split_territory(
    state: &mut SimulationState,
    tribe_id: TribeId,
    center: TileCoord,
    world: &WorldData,
    params: &SimulationParams,
) {
    let radius = params.initial_territory_radius / 2 + 1;

    let mut claimed = Vec::new();

    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            let nx = (center.x as i32 + dx).rem_euclid(world.heightmap.width as i32) as usize;
            let ny = (center.y as i32 + dy).clamp(0, world.heightmap.height as i32 - 1) as usize;

            let dist = (dx.abs() + dy.abs()) as usize;
            if dist > radius {
                continue;
            }

            let coord = TileCoord::new(nx, ny);

            // Skip if already owned
            if state.territory_map.contains_key(&coord) {
                continue;
            }

            // Skip water
            let elevation = *world.heightmap.get(nx, ny);
            if elevation < 0.0 {
                continue;
            }

            claimed.push(coord);
        }
    }

    // Add to tribe and territory map
    if let Some(tribe) = state.tribes.get_mut(&tribe_id) {
        for coord in claimed {
            tribe.claim_tile(coord);
            state.territory_map.insert(coord, tribe_id);
        }
    }
}

/// Slightly mutate a culture for the split tribe
fn mutate_culture<R: Rng>(culture: &mut TribeCulture, rng: &mut R) {
    let variance = 0.1;

    culture.aggression = (culture.aggression + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
    culture.trade_affinity = (culture.trade_affinity + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
    culture.expansion_drive = (culture.expansion_drive + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
    culture.research_priority = (culture.research_priority + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
}

/// Check if a biome is habitable
fn is_habitable(biome: ExtendedBiome) -> bool {
    !matches!(
        biome,
        ExtendedBiome::DeepOcean
            | ExtendedBiome::Ocean
            | ExtendedBiome::CoastalWater
            | ExtendedBiome::Ice
            | ExtendedBiome::LavaLake
            | ExtendedBiome::AcidLake
            | ExtendedBiome::VolcanicWasteland
            | ExtendedBiome::VoidScar
            | ExtendedBiome::VoidMaw
    )
}
