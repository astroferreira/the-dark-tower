//! Colonist movement logic
//!
//! Handles movement state machine, pathfinding, and job-based destination finding.

use rand::Rng;
use std::collections::{HashMap, HashSet};

use crate::biomes::ExtendedBiome;
use crate::simulation::colonists::types::{Colonist, ColonistId, ColonistActivityState, ColonistRole};
use crate::simulation::jobs::types::JobType;
use crate::simulation::types::{TileCoord, GlobalLocalCoord};
use crate::world::WorldData;

/// Movement speed in ticks between moves
const MOVEMENT_COOLDOWN: u64 = 1;
/// How long to work before returning
const WORK_DURATION_TICKS: u64 = 8;
/// Chance to start working each tick when idle
const START_WORK_CHANCE: f32 = 0.15;

/// Process movement for all colonists in a tribe
pub fn process_colonist_movement<R: Rng>(
    colonists: &mut HashMap<ColonistId, Colonist>,
    territory: &HashSet<TileCoord>,
    capital: TileCoord,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    let colonist_ids: Vec<ColonistId> = colonists.keys().copied().collect();

    for id in colonist_ids {
        // Skip dead colonists
        if !colonists.get(&id).map_or(false, |c| c.is_alive) {
            continue;
        }

        // Get current state info
        let (state, can_move, has_job, role, location) = {
            let c = colonists.get(&id).unwrap();
            (
                c.activity_state,
                current_tick >= c.last_move_tick + MOVEMENT_COOLDOWN,
                c.current_job.is_some(),
                c.role,
                c.location,
            )
        };

        // Process state machine
        match state {
            ColonistActivityState::Idle => {
                process_idle_state(colonists.get_mut(&id).unwrap(), territory, capital, world, current_tick, rng);
            }
            ColonistActivityState::Traveling => {
                if can_move {
                    process_traveling_state(colonists.get_mut(&id).unwrap(), world, current_tick, rng);
                }
            }
            ColonistActivityState::Working => {
                process_working_state(colonists.get_mut(&id).unwrap(), current_tick);
            }
            ColonistActivityState::Returning => {
                if can_move {
                    process_returning_state(colonists.get_mut(&id).unwrap(), capital, world, current_tick, rng);
                }
            }
            ColonistActivityState::Patrolling => {
                if can_move {
                    process_patrolling_state(colonists.get_mut(&id).unwrap(), territory, world, current_tick, rng);
                }
            }
            ColonistActivityState::Scouting => {
                if can_move {
                    process_scouting_state(colonists.get_mut(&id).unwrap(), territory, world, current_tick, rng);
                }
            }
            ColonistActivityState::Fleeing => {
                if can_move {
                    process_fleeing_state(colonists.get_mut(&id).unwrap(), capital, world, current_tick, rng);
                }
            }
            ColonistActivityState::Socializing => {
                process_socializing_state(colonists.get_mut(&id).unwrap(), current_tick);
            }
        }
    }
}

/// Handle idle state - chance to start working
fn process_idle_state<R: Rng>(
    colonist: &mut Colonist,
    territory: &HashSet<TileCoord>,
    capital: TileCoord,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    // Leaders and priests rarely leave the capital
    if matches!(colonist.role, ColonistRole::Leader | ColonistRole::Priest) {
        if rng.gen::<f32>() < 0.02 {
            colonist.activity_state = ColonistActivityState::Socializing;
            colonist.last_move_tick = current_tick;
        }
        return;
    }

    // Guards and warriors patrol
    if colonist.current_job == Some(JobType::Guard) || colonist.current_job == Some(JobType::Warrior) {
        if rng.gen::<f32>() < START_WORK_CHANCE {
            colonist.activity_state = ColonistActivityState::Patrolling;
            colonist.destination = find_patrol_location(territory, capital, world, rng);
            colonist.last_move_tick = current_tick;
        }
        return;
    }

    // Scouts go beyond territory
    if colonist.current_job == Some(JobType::Scout) {
        if rng.gen::<f32>() < START_WORK_CHANCE {
            colonist.activity_state = ColonistActivityState::Scouting;
            colonist.destination = find_scout_location(territory, capital, world, rng);
            colonist.last_move_tick = current_tick;
        }
        return;
    }

    // Other workers go to work locations
    if colonist.current_job.is_some() && rng.gen::<f32>() < START_WORK_CHANCE {
        if let Some(dest) = find_work_location(colonist, territory, world, rng) {
            colonist.activity_state = ColonistActivityState::Traveling;
            colonist.destination = Some(dest);
            colonist.last_move_tick = current_tick;
        }
    }
}

/// Handle traveling state - move toward destination
fn process_traveling_state<R: Rng>(
    colonist: &mut Colonist,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    if let Some(dest) = colonist.destination {
        if colonist.location == dest {
            // Arrived at destination, start working
            colonist.activity_state = ColonistActivityState::Working;
            colonist.last_move_tick = current_tick;
        } else {
            // Move toward destination
            move_toward(colonist, dest, world, current_tick);
        }
    } else {
        // No destination, go back to idle
        colonist.activity_state = ColonistActivityState::Idle;
    }
}

/// Handle working state - work for a while then return
fn process_working_state(colonist: &mut Colonist, current_tick: u64) {
    let work_time = current_tick - colonist.last_move_tick;
    if work_time >= WORK_DURATION_TICKS {
        colonist.activity_state = ColonistActivityState::Returning;
        colonist.destination = None;
        colonist.last_move_tick = current_tick;
    }
}

/// Handle returning state - move back to capital
fn process_returning_state<R: Rng>(
    colonist: &mut Colonist,
    capital: TileCoord,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    let dist = colonist.location.distance_wrapped(&capital, world.width);
    if dist <= 2 {
        // Close enough to capital, become idle
        colonist.activity_state = ColonistActivityState::Idle;
        colonist.destination = None;
    } else {
        move_toward(colonist, capital, world, current_tick);
    }
}

/// Handle patrolling state - guards moving around territory edges
fn process_patrolling_state<R: Rng>(
    colonist: &mut Colonist,
    territory: &HashSet<TileCoord>,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    if let Some(dest) = colonist.destination {
        if colonist.location == dest || colonist.location.distance_wrapped(&dest, world.width) <= 1 {
            // Reached patrol point, pick a new one or return
            if rng.gen::<f32>() < 0.3 {
                colonist.activity_state = ColonistActivityState::Returning;
                colonist.destination = None;
            } else {
                colonist.destination = find_patrol_location(territory, colonist.location, world, rng);
            }
            colonist.last_move_tick = current_tick;
        } else {
            move_toward(colonist, dest, world, current_tick);
        }
    } else {
        colonist.activity_state = ColonistActivityState::Returning;
    }
}

/// Handle scouting state - scouts exploring beyond territory
fn process_scouting_state<R: Rng>(
    colonist: &mut Colonist,
    territory: &HashSet<TileCoord>,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    if let Some(dest) = colonist.destination {
        if colonist.location == dest || colonist.location.distance_wrapped(&dest, world.width) <= 1 {
            // Reached scout point, return or continue
            if rng.gen::<f32>() < 0.4 {
                colonist.activity_state = ColonistActivityState::Returning;
                colonist.destination = None;
            } else {
                colonist.destination = find_scout_location(territory, colonist.location, world, rng);
            }
            colonist.last_move_tick = current_tick;
        } else {
            move_toward(colonist, dest, world, current_tick);
        }
    } else {
        colonist.activity_state = ColonistActivityState::Returning;
    }
}

/// Handle fleeing state - move quickly toward capital
fn process_fleeing_state<R: Rng>(
    colonist: &mut Colonist,
    capital: TileCoord,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    let dist = colonist.location.distance_wrapped(&capital, world.width);
    if dist <= 1 {
        colonist.activity_state = ColonistActivityState::Idle;
        colonist.destination = None;
    } else {
        // Move toward capital faster (update tick immediately)
        move_toward(colonist, capital, world, current_tick);
    }
}

/// Handle socializing state - brief interaction then back to idle
fn process_socializing_state(colonist: &mut Colonist, current_tick: u64) {
    let social_time = current_tick - colonist.last_move_tick;
    if social_time >= 4 {
        colonist.activity_state = ColonistActivityState::Idle;
    }
}

/// Move one step toward a target location
fn move_toward(colonist: &mut Colonist, target: TileCoord, world: &WorldData, current_tick: u64) {
    let curr = colonist.location;
    let width = world.width as i32;
    let height = world.height as i32;

    // Calculate direction considering wrapping
    let dx = {
        let direct = target.x as i32 - curr.x as i32;
        let wrapped_pos = direct + width;
        let wrapped_neg = direct - width;

        if direct.abs() <= wrapped_pos.abs() && direct.abs() <= wrapped_neg.abs() {
            direct.signum()
        } else if wrapped_pos.abs() < wrapped_neg.abs() {
            wrapped_pos.signum()
        } else {
            wrapped_neg.signum()
        }
    };

    let dy = (target.y as i32 - curr.y as i32).clamp(-1, 1);

    // Apply movement
    let new_x = ((curr.x as i32 + dx).rem_euclid(width)) as usize;
    let new_y = (curr.y as i32 + dy).clamp(0, height - 1) as usize;
    let new_coord = TileCoord::new(new_x, new_y);

    // Check if new location is passable
    let elevation = *world.heightmap.get(new_x, new_y);
    if elevation >= 0.0 {
        let old_location = colonist.location;
        colonist.location = new_coord;

        // Sync local_position when world tile changes
        if old_location != new_coord {
            colonist.local_position = GlobalLocalCoord::from_world_tile(new_coord);
        }
    }

    colonist.last_move_tick = current_tick;
}

/// Find a work location based on colonist's job
fn find_work_location<R: Rng>(
    colonist: &Colonist,
    territory: &HashSet<TileCoord>,
    world: &WorldData,
    rng: &mut R,
) -> Option<TileCoord> {
    let target_biomes = match colonist.current_job {
        Some(JobType::Farmer) => vec![
            ExtendedBiome::TemperateGrassland,
            ExtendedBiome::Savanna,
            ExtendedBiome::TemperateForest,
        ],
        Some(JobType::Miner) => vec![
            ExtendedBiome::AlpineTundra,
            ExtendedBiome::SnowyPeaks,
            ExtendedBiome::Foothills,
        ],
        Some(JobType::Woodcutter) => vec![
            ExtendedBiome::TemperateForest,
            ExtendedBiome::BorealForest,
            ExtendedBiome::TropicalForest,
            ExtendedBiome::TropicalRainforest,
        ],
        Some(JobType::Hunter) => vec![
            ExtendedBiome::TemperateForest,
            ExtendedBiome::Savanna,
            ExtendedBiome::BorealForest,
            ExtendedBiome::TemperateGrassland,
        ],
        Some(JobType::Fisher) => vec![
            ExtendedBiome::CoastalWater,
            ExtendedBiome::Lagoon,
            ExtendedBiome::MirrorLake,
            ExtendedBiome::Marsh,
        ],
        _ => return random_territory_location(territory, rng),
    };

    // Find matching tiles in territory
    let matching: Vec<TileCoord> = territory
        .iter()
        .filter(|coord| {
            let biome = *world.biomes.get(coord.x, coord.y);
            target_biomes.contains(&biome)
        })
        .copied()
        .collect();

    if matching.is_empty() {
        random_territory_location(territory, rng)
    } else {
        Some(matching[rng.gen_range(0..matching.len())])
    }
}

/// Find a patrol location near territory edge
fn find_patrol_location<R: Rng>(
    territory: &HashSet<TileCoord>,
    from: TileCoord,
    world: &WorldData,
    rng: &mut R,
) -> Option<TileCoord> {
    // Find edge tiles (tiles with at least one non-territory neighbor)
    let edge_tiles: Vec<TileCoord> = territory
        .iter()
        .filter(|coord| {
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = ((coord.x as i32 + dx).rem_euclid(world.width as i32)) as usize;
                    let ny = (coord.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
                    let neighbor = TileCoord::new(nx, ny);
                    if !territory.contains(&neighbor) {
                        return true;
                    }
                }
            }
            false
        })
        .copied()
        .collect();

    if edge_tiles.is_empty() {
        random_territory_location(territory, rng)
    } else {
        Some(edge_tiles[rng.gen_range(0..edge_tiles.len())])
    }
}

/// Find a scout location beyond territory
fn find_scout_location<R: Rng>(
    territory: &HashSet<TileCoord>,
    from: TileCoord,
    world: &WorldData,
    rng: &mut R,
) -> Option<TileCoord> {
    // Pick a random direction and distance
    let angle = rng.gen::<f32>() * std::f32::consts::PI * 2.0;
    let distance = rng.gen_range(5..15);

    let dx = (angle.cos() * distance as f32) as i32;
    let dy = (angle.sin() * distance as f32) as i32;

    let nx = ((from.x as i32 + dx).rem_euclid(world.width as i32)) as usize;
    let ny = (from.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
    let target = TileCoord::new(nx, ny);

    // Make sure it's passable
    let elevation = *world.heightmap.get(nx, ny);
    if elevation >= 0.0 {
        Some(target)
    } else {
        // Try again with a land tile
        random_territory_location(territory, rng)
    }
}

/// Get a random location within territory
fn random_territory_location<R: Rng>(
    territory: &HashSet<TileCoord>,
    rng: &mut R,
) -> Option<TileCoord> {
    if territory.is_empty() {
        return None;
    }
    let tiles: Vec<_> = territory.iter().copied().collect();
    Some(tiles[rng.gen_range(0..tiles.len())])
}

/// Make a colonist flee (called externally when danger is detected)
pub fn trigger_flee(colonist: &mut Colonist, current_tick: u64) {
    colonist.activity_state = ColonistActivityState::Fleeing;
    colonist.destination = None;
    colonist.last_move_tick = current_tick;
}
