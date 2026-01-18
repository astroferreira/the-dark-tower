//! Territory expansion for tribes

use std::collections::HashSet;
use rand::Rng;

use crate::world::WorldData;
use crate::biomes::ExtendedBiome;
use crate::simulation::types::{TileCoord, TribeId, TribeEventType};
use crate::simulation::params::SimulationParams;
use crate::simulation::simulation::SimulationState;

/// Process territory expansion for all tribes
pub fn process_expansion_tick<R: Rng>(
    state: &mut SimulationState,
    world: &WorldData,
    params: &SimulationParams,
    rng: &mut R,
) {
    let tribe_ids: Vec<TribeId> = state.tribes.keys().copied().collect();

    for tribe_id in tribe_ids {
        let should_expand = {
            let tribe = match state.tribes.get(&tribe_id) {
                Some(t) if t.is_alive => t,
                _ => continue,
            };

            // Check if tribe should expand
            let pop = tribe.population.total();
            let territory_size = tribe.territory.len();
            let max_territory = params.max_territory_size;

            // Can support more territory?
            let supported_territory = (pop as f32 / params.pop_per_territory_tile) as usize;

            // Expansion drive from culture
            let expansion_drive = tribe.culture.expansion_drive;

            // Conditions for expansion
            territory_size < max_territory
                && territory_size < supported_territory
                && tribe.needs.food.satisfaction > 0.4
                && rng.gen::<f32>() < expansion_drive
        };

        if should_expand {
            expand_tribe_territory(state, tribe_id, world, params, rng);
        }
    }
}

/// Expand a tribe's territory
fn expand_tribe_territory<R: Rng>(
    state: &mut SimulationState,
    tribe_id: TribeId,
    world: &WorldData,
    params: &SimulationParams,
    rng: &mut R,
) {
    // Find candidate tiles for expansion
    let candidates = find_expansion_candidates(state, tribe_id, world);

    if candidates.is_empty() {
        return;
    }

    // Get tribe's culture for preference
    let culture = match state.tribes.get(&tribe_id) {
        Some(t) => t.culture.clone(),
        None => return,
    };

    // Score and sort candidates
    let mut scored_candidates: Vec<(TileCoord, f32)> = candidates
        .into_iter()
        .map(|coord| {
            let score = score_expansion_tile(&coord, &culture, world, state);
            (coord, score)
        })
        .collect();

    scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Expand to best tile(s)
    let expand_count = 1; // Could be more based on population/tech

    for (coord, _score) in scored_candidates.into_iter().take(expand_count) {
        // Check if tile is still unclaimed
        if state.territory_map.contains_key(&coord) {
            continue;
        }

        // Claim the tile
        state.territory_map.insert(coord, tribe_id);

        if let Some(tribe) = state.tribes.get_mut(&tribe_id) {
            tribe.claim_tile(coord);
            tribe.record_event(
                state.current_tick,
                TribeEventType::TerritoryExpanded { tile: coord },
            );
        }
    }
}

/// Find tiles adjacent to tribe's territory that could be expanded into
fn find_expansion_candidates(
    state: &SimulationState,
    tribe_id: TribeId,
    world: &WorldData,
) -> Vec<TileCoord> {
    let tribe = match state.tribes.get(&tribe_id) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut candidates = HashSet::new();

    for coord in &tribe.territory {
        // Check all 8 neighbors
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (coord.x as i32 + dx).rem_euclid(world.heightmap.width as i32) as usize;
                let ny = (coord.y as i32 + dy).clamp(0, world.heightmap.height as i32 - 1) as usize;

                let neighbor = TileCoord::new(nx, ny);

                // Skip if already owned (by anyone)
                if state.territory_map.contains_key(&neighbor) {
                    continue;
                }

                // Skip water tiles
                let elevation = *world.heightmap.get(nx, ny);
                if elevation < 0.0 {
                    continue;
                }

                // Skip impassable biomes
                let biome = *world.biomes.get(nx, ny);
                if !is_expandable_biome(biome) {
                    continue;
                }

                candidates.insert(neighbor);
            }
        }
    }

    candidates.into_iter().collect()
}

/// Score a tile for expansion desirability
fn score_expansion_tile(
    coord: &TileCoord,
    culture: &crate::simulation::tribe::TribeCulture,
    world: &WorldData,
    state: &SimulationState,
) -> f32 {
    let biome = *world.biomes.get(coord.x, coord.y);
    let elevation = *world.heightmap.get(coord.x, coord.y);

    // Base value from resources
    let mut score = estimate_resource_value(biome);

    // Cultural preference bonus
    let is_water = elevation < 0.0;
    let cultural_pref = culture.terrain_preference(biome, elevation, is_water);
    score *= cultural_pref;

    // Preferred biome bonus
    if culture.prefers_biome(&biome) {
        score *= 1.5;
    }

    // Proximity to other tribes (contested = valuable but risky)
    let is_contested = is_contested_tile(coord, state, world);
    if is_contested {
        score *= 0.7; // Slightly lower score for contested areas
    }

    // Strategic features bonus
    if is_strategic_tile(coord, world) {
        score *= 1.3;
    }

    score
}

/// Estimate resource value of a biome
fn estimate_resource_value(biome: ExtendedBiome) -> f32 {
    use crate::simulation::resources::biome_resources;

    let resources = biome_resources(biome);

    let primary_value: f32 = resources.primary.iter().map(|(_, amt)| amt).sum();
    let secondary_value: f32 = resources.secondary.iter().map(|(_, amt)| amt * 0.5).sum();
    let rare_value: f32 = resources.rare.iter().map(|(_, amt)| amt * 2.0).sum();

    primary_value + secondary_value + rare_value
}

/// Check if a tile is being contested by multiple tribes
fn is_contested_tile(coord: &TileCoord, state: &SimulationState, world: &WorldData) -> bool {
    let mut nearby_tribes = HashSet::new();

    for dx in -2i32..=2 {
        for dy in -2i32..=2 {
            let nx = (coord.x as i32 + dx).rem_euclid(world.heightmap.width as i32) as usize;
            let ny = (coord.y as i32 + dy).clamp(0, world.heightmap.height as i32 - 1) as usize;

            let neighbor = TileCoord::new(nx, ny);

            if let Some(&owner) = state.territory_map.get(&neighbor) {
                nearby_tribes.insert(owner);
            }
        }
    }

    nearby_tribes.len() > 1
}

/// Check if a tile is strategically valuable
fn is_strategic_tile(coord: &TileCoord, world: &WorldData) -> bool {
    let biome = *world.biomes.get(coord.x, coord.y);
    let elevation = *world.heightmap.get(coord.x, coord.y);

    // Mountain passes
    if elevation > 500.0 && elevation < 1500.0 {
        return true;
    }

    // Resource-rich biomes
    matches!(
        biome,
        ExtendedBiome::Oasis
            | ExtendedBiome::ObsidianFields
            | ExtendedBiome::Geysers
            | ExtendedBiome::CrystalForest
    )
}

/// Check if a biome can be expanded into
fn is_expandable_biome(biome: ExtendedBiome) -> bool {
    !matches!(
        biome,
        ExtendedBiome::DeepOcean
            | ExtendedBiome::Ocean
            | ExtendedBiome::LavaLake
            | ExtendedBiome::AcidLake
            | ExtendedBiome::VoidScar
            | ExtendedBiome::VoidMaw
            | ExtendedBiome::Ice
    )
}
