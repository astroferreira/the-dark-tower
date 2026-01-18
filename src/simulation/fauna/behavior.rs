//! Fauna behavior and AI system
//!
//! Handles animal behaviors: grazing, fleeing, breeding, migrating, hunting (for predators)

use rand::Rng;
use std::collections::HashMap;

use crate::simulation::types::{GlobalLocalCoord, TileCoord, TribeId};
use crate::simulation::fauna::types::{
    Fauna, FaunaActivity, FaunaDiet, FaunaId, FaunaSpecies, FaunaState,
};
use crate::simulation::monsters::{Monster, MonsterId};
use crate::simulation::tribe::Tribe;
use crate::world::WorldData;

/// Process behavior for all fauna
pub fn process_fauna_behavior<R: Rng>(
    fauna: &mut HashMap<FaunaId, Fauna>,
    tribes: &HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    monsters: &HashMap<MonsterId, Monster>,
    world: &WorldData,
    current_tick: u64,
    focus_point: Option<GlobalLocalCoord>,
    world_width: usize,
    rng: &mut R,
) {
    use crate::simulation::simulation::{FOCUS_RADIUS_MEDIUM, SPARSE_UPDATE_INTERVAL};

    let should_update_sparse = current_tick % SPARSE_UPDATE_INTERVAL == 0;

    // Collect fauna IDs to process
    let fauna_ids: Vec<FaunaId> = fauna.keys().copied().collect();

    for id in fauna_ids {
        if let Some(creature) = fauna.get_mut(&id) {
            if creature.is_dead() {
                creature.state = FaunaState::Dead;
                continue;
            }

            // Check if fauna is in focus
            let in_focus = match focus_point {
                Some(focus) => {
                    creature.local_position.distance_wrapped(&focus, world_width)
                        <= FOCUS_RADIUS_MEDIUM
                }
                None => true,
            };

            // Skip distant fauna on non-sparse ticks
            if !in_focus && !should_update_sparse {
                continue;
            }

            // Age the creature
            creature.age += 1;

            // Update hunger
            creature.hunger = (creature.hunger + 0.02).min(1.0);

            // Heal slowly when well-fed and resting
            if creature.state == FaunaState::Idle && creature.hunger < 0.3 {
                creature.heal(0.5);
            }

            // Starving creatures take damage
            if creature.hunger >= 1.0 {
                creature.take_damage(1.0);
            }

            // Process state machine
            let new_state = process_fauna_state(
                creature, tribes, territory_map, monsters, world, current_tick, in_focus, rng,
            );
            creature.state = new_state;
            creature.last_action_tick = current_tick;
        }
    }
}

/// Process a single fauna creature's state
fn process_fauna_state<R: Rng>(
    creature: &mut Fauna,
    tribes: &HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    monsters: &HashMap<MonsterId, Monster>,
    world: &WorldData,
    current_tick: u64,
    in_focus: bool,
    rng: &mut R,
) -> FaunaState {
    let stats = creature.species.stats();

    // Check for nearby threats
    if should_flee(creature, tribes, territory_map, monsters, world) {
        creature.current_activity = FaunaActivity::Running;
        if in_focus {
            flee_from_threat(creature, tribes, territory_map, monsters, world, rng);
        }
        return FaunaState::Fleeing;
    }

    match creature.state {
        FaunaState::Dead => FaunaState::Dead,

        FaunaState::Idle => {
            // Decide next action based on needs
            if creature.hunger > 0.6 {
                creature.current_activity = if stats.diet == FaunaDiet::Carnivore {
                    FaunaActivity::Hunting
                } else {
                    FaunaActivity::Eating
                };
                if stats.diet == FaunaDiet::Carnivore || stats.diet == FaunaDiet::Piscivore {
                    return FaunaState::Hunting;
                }
                return FaunaState::Grazing;
            }

            // Random activities
            let roll = rng.gen::<f32>();
            if roll < 0.2 {
                creature.current_activity = FaunaActivity::Wandering;
                return FaunaState::Roaming;
            } else if roll < 0.25 && creature.can_breed(current_tick) {
                creature.current_activity = FaunaActivity::Nesting;
                return FaunaState::Breeding;
            } else if roll < 0.3 {
                creature.current_activity = FaunaActivity::Grooming;
            } else if roll < 0.35 {
                creature.current_activity = FaunaActivity::Playing;
            } else if roll < 0.4 {
                creature.current_activity = FaunaActivity::Drinking;
            } else {
                creature.current_activity = FaunaActivity::Resting;
            }

            FaunaState::Idle
        }

        FaunaState::Grazing => {
            // Reduce hunger while grazing
            creature.hunger = (creature.hunger - 0.1).max(0.0);
            creature.current_activity = FaunaActivity::Eating;

            if in_focus {
                // Occasionally move while grazing
                if rng.gen::<f32>() < 0.3 {
                    move_randomly(creature, world, rng);
                }
            }

            // Return to idle when fed
            if creature.hunger < 0.2 {
                return FaunaState::Idle;
            }

            FaunaState::Grazing
        }

        FaunaState::Roaming => {
            creature.current_activity = FaunaActivity::Wandering;

            if in_focus {
                move_randomly(creature, world, rng);
            }

            // Chance to stop and do something else
            if rng.gen::<f32>() < 0.2 {
                return FaunaState::Idle;
            }

            // If hungry, start grazing
            if creature.hunger > 0.5 {
                return FaunaState::Grazing;
            }

            FaunaState::Roaming
        }

        FaunaState::Fleeing => {
            creature.current_activity = FaunaActivity::Running;

            if in_focus {
                flee_from_threat(creature, tribes, territory_map, monsters, world, rng);
            }

            // Stop fleeing if no longer in danger
            if !should_flee(creature, tribes, territory_map, monsters, world) {
                creature.current_activity = FaunaActivity::Resting;
                return FaunaState::Idle;
            }

            FaunaState::Fleeing
        }

        FaunaState::Breeding => {
            creature.current_activity = FaunaActivity::Nesting;

            // Breeding takes a few ticks
            if rng.gen::<f32>() < 0.3 {
                creature.last_breed_tick = current_tick;
                return FaunaState::Idle;
            }

            FaunaState::Breeding
        }

        FaunaState::Migrating => {
            creature.current_activity = FaunaActivity::Wandering;

            if in_focus {
                // Move in a consistent direction
                migrate(creature, world, rng);
            }

            // Migration ends randomly
            if rng.gen::<f32>() < 0.05 {
                creature.home_range_center = creature.location;
                return FaunaState::Idle;
            }

            FaunaState::Migrating
        }

        FaunaState::Hunting => {
            creature.current_activity = FaunaActivity::Hunting;

            // Predators hunt other fauna or forage
            // For now, just reduce hunger as if successful sometimes
            if rng.gen::<f32>() < 0.3 {
                creature.hunger = (creature.hunger - 0.2).max(0.0);
            }

            if in_focus {
                move_randomly(creature, world, rng);
            }

            // Stop hunting when not hungry
            if creature.hunger < 0.3 {
                return FaunaState::Idle;
            }

            FaunaState::Hunting
        }
    }
}

/// Check if fauna should flee from threats
fn should_flee(
    creature: &Fauna,
    tribes: &HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    monsters: &HashMap<MonsterId, Monster>,
    world: &WorldData,
) -> bool {
    let stats = creature.species.stats();
    let flee_range = (5.0 / stats.alertness) as usize;

    // Check for nearby tribe activity (hunters)
    if let Some(&tribe_id) = territory_map.get(&creature.location) {
        if let Some(tribe) = tribes.get(&tribe_id) {
            // More likely to flee from populated areas
            if tribe.population.total() > 50 {
                return true;
            }
        }
    }

    // Check for nearby predatory monsters
    for monster in monsters.values() {
        if monster.is_dead() {
            continue;
        }
        // Only flee from predatory monsters
        let is_predator = matches!(
            monster.species,
            crate::simulation::monsters::MonsterSpecies::Wolf
                | crate::simulation::monsters::MonsterSpecies::Bear
                | crate::simulation::monsters::MonsterSpecies::IceWolf
                | crate::simulation::monsters::MonsterSpecies::GiantSpider
        );
        if is_predator {
            let dist = creature.distance_to(&monster.location, world.width);
            if dist <= flee_range {
                return true;
            }
        }
    }

    false
}

/// Flee from the nearest threat
fn flee_from_threat<R: Rng>(
    creature: &mut Fauna,
    tribes: &HashMap<TribeId, Tribe>,
    _territory_map: &HashMap<TileCoord, TribeId>,
    monsters: &HashMap<MonsterId, Monster>,
    world: &WorldData,
    rng: &mut R,
) {
    // Find nearest threat
    let mut nearest_threat: Option<TileCoord> = None;
    let mut nearest_dist = usize::MAX;

    // Check tribes
    for tribe in tribes.values() {
        if !tribe.is_alive {
            continue;
        }
        let dist = creature.distance_to(&tribe.capital, world.width);
        if dist < nearest_dist && dist < 10 {
            nearest_dist = dist;
            nearest_threat = Some(tribe.capital);
        }
    }

    // Check monsters
    for monster in monsters.values() {
        if monster.is_dead() {
            continue;
        }
        let dist = creature.distance_to(&monster.location, world.width);
        if dist < nearest_dist {
            nearest_dist = dist;
            nearest_threat = Some(monster.location);
        }
    }

    if let Some(threat_coord) = nearest_threat {
        // Move away from threat
        let dx = (creature.location.x as i32 - threat_coord.x as i32).signum();
        let dy = (creature.location.y as i32 - threat_coord.y as i32).signum();

        // Add randomness
        let dx = if rng.gen::<f32>() < 0.2 {
            rng.gen_range(-1..=1)
        } else {
            dx
        };
        let dy = if rng.gen::<f32>() < 0.2 {
            rng.gen_range(-1..=1)
        } else {
            dy
        };

        let stats = creature.species.stats();
        let speed = stats.speed.ceil() as i32;

        let new_x =
            (creature.location.x as i32 + dx * speed).rem_euclid(world.width as i32) as usize;
        let new_y =
            (creature.location.y as i32 + dy * speed).clamp(0, world.height as i32 - 1) as usize;

        let elevation = *world.heightmap.get(new_x, new_y);
        if elevation >= 0.0 {
            let new_coord = TileCoord::new(new_x, new_y);
            if creature.location != new_coord {
                creature.location = new_coord;
                creature.local_position = GlobalLocalCoord::from_world_tile(new_coord);
            }
        }
    } else {
        move_randomly(creature, world, rng);
    }
}

/// Move the creature randomly within its home range
fn move_randomly<R: Rng>(creature: &mut Fauna, world: &WorldData, rng: &mut R) {
    let directions = [(0i32, -1), (0, 1), (-1, 0), (1, 0)];
    let (dx, dy) = directions[rng.gen_range(0..4)];

    let new_x = (creature.location.x as i32 + dx).rem_euclid(world.width as i32) as usize;
    let new_y = (creature.location.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
    let new_coord = TileCoord::new(new_x, new_y);

    // Check if new position is valid
    let elevation = *world.heightmap.get(new_x, new_y);
    let biome = *world.biomes.get(new_x, new_y);

    // Aquatic species need water
    let is_aquatic = matches!(
        creature.species,
        FaunaSpecies::Fish | FaunaSpecies::Salmon | FaunaSpecies::Crab | FaunaSpecies::Seal
    );

    let valid = if is_aquatic {
        elevation < 0.0 || creature.species.can_spawn_in(biome)
    } else {
        elevation >= 0.0
    };

    if valid && creature.in_home_range(&new_coord, world.width) {
        if creature.location != new_coord {
            creature.location = new_coord;
            creature.local_position = GlobalLocalCoord::from_world_tile(new_coord);
        }
    }
}

/// Migration movement (consistent direction)
fn migrate<R: Rng>(creature: &mut Fauna, world: &WorldData, rng: &mut R) {
    // Use creature ID to determine migration direction for consistency
    let dir_idx = (creature.id.0 % 4) as usize;
    let directions = [(0i32, -1), (0, 1), (-1, 0), (1, 0)];
    let (dx, dy) = directions[dir_idx];

    // Add slight randomness
    let dx = if rng.gen::<f32>() < 0.2 {
        dx + rng.gen_range(-1..=1)
    } else {
        dx
    };
    let dy = if rng.gen::<f32>() < 0.2 {
        dy + rng.gen_range(-1..=1)
    } else {
        dy
    };

    let new_x = (creature.location.x as i32 + dx).rem_euclid(world.width as i32) as usize;
    let new_y = (creature.location.y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
    let new_coord = TileCoord::new(new_x, new_y);

    let elevation = *world.heightmap.get(new_x, new_y);
    if elevation >= 0.0 {
        if creature.location != new_coord {
            creature.location = new_coord;
            creature.local_position = GlobalLocalCoord::from_world_tile(new_coord);
        }
    }
}

/// Find a breeding partner for the creature
pub fn find_breeding_partner(
    creature: &Fauna,
    fauna: &HashMap<FaunaId, Fauna>,
    current_tick: u64,
    world_width: usize,
) -> Option<FaunaId> {
    let range = 3; // Must be close to breed

    for (id, other) in fauna.iter() {
        if *id == creature.id {
            continue;
        }
        if other.species != creature.species {
            continue;
        }
        if other.is_male == creature.is_male {
            continue;
        }
        if !other.can_breed(current_tick) {
            continue;
        }
        if creature.distance_to(&other.location, world_width) <= range {
            return Some(*id);
        }
    }

    None
}
