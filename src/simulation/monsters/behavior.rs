//! Monster behavior and AI system

use rand::Rng;
use std::collections::HashMap;

use crate::simulation::types::{TileCoord, TribeId};
use crate::simulation::monsters::types::{Monster, MonsterId, MonsterState, AttackTarget};
use crate::simulation::tribe::Tribe;
use crate::world::WorldData;

/// Process behavior for all monsters
pub fn process_monster_behavior<R: Rng>(
    monsters: &mut HashMap<MonsterId, Monster>,
    tribes: &HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
    current_tick: u64,
    rng: &mut R,
) {
    // Collect monster IDs to process (avoid borrow issues)
    let monster_ids: Vec<MonsterId> = monsters.keys().copied().collect();

    for id in monster_ids {
        if let Some(monster) = monsters.get_mut(&id) {
            if monster.is_dead() {
                monster.state = MonsterState::Dead;
                continue;
            }

            // Heal slowly when idle
            if monster.state == MonsterState::Idle && monster.health < monster.max_health {
                monster.heal(1.0);
            }

            // Process state machine
            let new_state = process_monster_state(
                monster,
                tribes,
                territory_map,
                world,
                current_tick,
                rng,
            );
            monster.state = new_state;
            monster.last_action_tick = current_tick;
        }
    }
}

/// Process a single monster's state and return the new state
fn process_monster_state<R: Rng>(
    monster: &mut Monster,
    tribes: &HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
    _current_tick: u64,
    rng: &mut R,
) -> MonsterState {
    let aggression = monster.species.stats().aggression;

    match monster.state {
        MonsterState::Dead => MonsterState::Dead,

        MonsterState::Idle => {
            // Check if should flee
            if monster.should_flee() {
                return MonsterState::Fleeing;
            }

            // Random chance to start roaming
            if rng.gen::<f32>() < 0.3 {
                return MonsterState::Roaming;
            }

            // Random chance to start hunting (based on aggression)
            if rng.gen::<f32>() < aggression * 0.5 {
                // Look for nearby targets
                if let Some(target) = find_nearby_target(monster, tribes, territory_map, world) {
                    return MonsterState::Hunting;
                }
            }

            MonsterState::Idle
        }

        MonsterState::Roaming => {
            // Move within territory
            move_randomly(monster, world, rng);

            // Check if should flee
            if monster.should_flee() {
                return MonsterState::Fleeing;
            }

            // Random chance to find prey while roaming
            if rng.gen::<f32>() < aggression * 0.3 {
                if let Some(target) = find_nearby_target(monster, tribes, territory_map, world) {
                    return MonsterState::Hunting;
                }
            }

            // Chance to return to idle
            if rng.gen::<f32>() < 0.2 {
                return MonsterState::Idle;
            }

            MonsterState::Roaming
        }

        MonsterState::Hunting => {
            // Check if should flee
            if monster.should_flee() {
                return MonsterState::Fleeing;
            }

            // Look for target
            if let Some(target) = find_nearby_target(monster, tribes, territory_map, world) {
                // Move toward target
                move_toward_target(monster, target, tribes, world, rng);

                // Check if in attack range (adjacent)
                let target_coord = get_target_coord(target, tribes);
                if let Some(coord) = target_coord {
                    if monster.distance_to(&coord, world.width) <= 1 {
                        return MonsterState::Attacking(target);
                    }
                }

                return MonsterState::Hunting;
            }

            // Lost target, return to roaming
            MonsterState::Roaming
        }

        MonsterState::Attacking(target) => {
            // Check if should flee
            if monster.should_flee() {
                return MonsterState::Fleeing;
            }

            // Check if target is still valid/adjacent
            let target_coord = get_target_coord(target, tribes);
            if let Some(coord) = target_coord {
                if monster.distance_to(&coord, world.width) <= 2 {
                    // Stay attacking
                    return MonsterState::Attacking(target);
                }
            }

            // Target moved away or died, return to hunting
            MonsterState::Hunting
        }

        MonsterState::Fleeing => {
            // Move away from threats
            flee_from_danger(monster, tribes, territory_map, world, rng);

            // Check if recovered enough to stop fleeing
            if monster.health > monster.max_health * 0.5 {
                return MonsterState::Idle;
            }

            // If far from danger, stop fleeing
            if !is_in_danger(monster, tribes, territory_map, world) {
                return MonsterState::Idle;
            }

            MonsterState::Fleeing
        }
    }
}

/// Find a nearby target for the monster
fn find_nearby_target(
    monster: &Monster,
    tribes: &HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
) -> Option<AttackTarget> {
    let detection_range = monster.territory_radius * 2;

    // Check for nearby tribe settlements
    for (tribe_id, tribe) in tribes.iter() {
        if !tribe.is_alive {
            continue;
        }

        // Check if any tribe territory is in range
        for &coord in &tribe.territory {
            if monster.distance_to(&coord, world.width) <= detection_range {
                return Some(AttackTarget::Tribe(*tribe_id));
            }
        }
    }

    None
}

/// Move the monster randomly within its territory
fn move_randomly<R: Rng>(monster: &mut Monster, world: &WorldData, rng: &mut R) {
    let directions = [(0i32, -1), (0, 1), (-1, 0), (1, 0)];
    let (dx, dy) = directions[rng.gen_range(0..4)];

    let new_x = (monster.location.x as i32 + dx).rem_euclid(world.width as i32) as usize;
    let new_y = (monster.location.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
    let new_coord = TileCoord::new(new_x, new_y);

    // Check if new position is valid (not water, within territory)
    let elevation = *world.heightmap.get(new_x, new_y);
    if elevation >= 0.0 && monster.in_territory(&new_coord, world.width) {
        monster.location = new_coord;
    }
}

/// Move toward a target
fn move_toward_target<R: Rng>(
    monster: &mut Monster,
    target: AttackTarget,
    tribes: &HashMap<TribeId, Tribe>,
    world: &WorldData,
    _rng: &mut R,
) {
    let target_coord = match get_target_coord(target, tribes) {
        Some(c) => c,
        None => return,
    };

    // Simple pathfinding - move in the direction of target
    let dx = (target_coord.x as i32 - monster.location.x as i32).signum();
    let dy = (target_coord.y as i32 - monster.location.y as i32).signum();

    // Try horizontal movement first
    if dx != 0 {
        let new_x = (monster.location.x as i32 + dx).rem_euclid(world.width as i32) as usize;
        let new_coord = TileCoord::new(new_x, monster.location.y);
        let elevation = *world.heightmap.get(new_x, monster.location.y);
        if elevation >= 0.0 {
            monster.location = new_coord;
            return;
        }
    }

    // Try vertical movement
    if dy != 0 {
        let new_y = (monster.location.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
        let new_coord = TileCoord::new(monster.location.x, new_y);
        let elevation = *world.heightmap.get(monster.location.x, new_y);
        if elevation >= 0.0 {
            monster.location = new_coord;
        }
    }
}

/// Move away from danger
fn flee_from_danger<R: Rng>(
    monster: &mut Monster,
    tribes: &HashMap<TribeId, Tribe>,
    _territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
    rng: &mut R,
) {
    // Find direction away from nearest tribe
    let mut nearest_tribe_coord: Option<TileCoord> = None;
    let mut nearest_dist = usize::MAX;

    for tribe in tribes.values() {
        if !tribe.is_alive {
            continue;
        }
        let dist = monster.distance_to(&tribe.capital, world.width);
        if dist < nearest_dist {
            nearest_dist = dist;
            nearest_tribe_coord = Some(tribe.capital);
        }
    }

    if let Some(threat_coord) = nearest_tribe_coord {
        // Move away from threat
        let dx = (monster.location.x as i32 - threat_coord.x as i32).signum();
        let dy = (monster.location.y as i32 - threat_coord.y as i32).signum();

        // Add some randomness to avoid getting stuck
        let dx = if rng.gen::<f32>() < 0.3 { -dx } else { dx };

        let new_x = (monster.location.x as i32 + dx).rem_euclid(world.width as i32) as usize;
        let new_y = (monster.location.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;

        let elevation = *world.heightmap.get(new_x, new_y);
        if elevation >= 0.0 {
            monster.location = TileCoord::new(new_x, new_y);
            // Expand territory center when fleeing
            monster.territory_center = monster.location;
        }
    } else {
        // No threats, just move randomly
        move_randomly(monster, world, rng);
    }
}

/// Check if monster is in danger
fn is_in_danger(
    monster: &Monster,
    tribes: &HashMap<TribeId, Tribe>,
    _territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
) -> bool {
    let danger_range = 5;

    for tribe in tribes.values() {
        if !tribe.is_alive {
            continue;
        }

        // Check if any tribe territory is too close
        for &coord in &tribe.territory {
            if monster.distance_to(&coord, world.width) <= danger_range {
                return true;
            }
        }
    }

    false
}

/// Get the coordinate of a target
fn get_target_coord(target: AttackTarget, tribes: &HashMap<TribeId, Tribe>) -> Option<TileCoord> {
    match target {
        AttackTarget::Tribe(tribe_id) => {
            tribes.get(&tribe_id).map(|t| t.capital)
        }
        AttackTarget::Monster(_monster_id) => {
            // Would need access to monsters map to get this
            // For now, return None and handle monster vs monster separately
            None
        }
    }
}
