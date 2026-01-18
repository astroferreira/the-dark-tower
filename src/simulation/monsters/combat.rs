//! Monster combat system - monster vs tribe and monster vs monster

use rand::Rng;
use std::collections::HashMap;

use crate::simulation::types::{TileCoord, TribeId, TribeEventType, SimTick};
use crate::simulation::monsters::types::{Monster, MonsterId, MonsterState, AttackTarget};
use crate::simulation::tribe::Tribe;
use crate::world::WorldData;

/// Result of a combat encounter
#[derive(Clone, Debug)]
pub struct CombatResult {
    pub attacker_damage: f32,
    pub defender_damage: f32,
    pub attacker_killed: bool,
    pub defender_killed: bool,
    pub casualties: u32,
    pub loot: f32,
}

/// Process combat for all monsters that are in attacking state
pub fn process_monster_combat<R: Rng>(
    monsters: &mut HashMap<MonsterId, Monster>,
    tribes: &mut HashMap<TribeId, Tribe>,
    territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
    current_tick: SimTick,
    rng: &mut R,
) -> Vec<CombatEvent> {
    let mut events = Vec::new();

    // Collect attacking monsters
    let attacking_monsters: Vec<(MonsterId, AttackTarget)> = monsters
        .iter()
        .filter_map(|(id, m)| {
            if let MonsterState::Attacking(target) = m.state {
                Some((*id, target))
            } else {
                None
            }
        })
        .collect();

    for (monster_id, target) in attacking_monsters {
        match target {
            AttackTarget::Tribe(tribe_id) => {
                if let Some(event) = process_monster_vs_tribe(
                    monster_id,
                    tribe_id,
                    monsters,
                    tribes,
                    territory_map,
                    world,
                    current_tick,
                    rng,
                ) {
                    events.push(event);
                }
            }
            AttackTarget::Monster(target_monster_id) => {
                if let Some(event) = process_monster_vs_monster(
                    monster_id,
                    target_monster_id,
                    monsters,
                    world,
                    rng,
                ) {
                    events.push(event);
                }
            }
        }
    }

    events
}

/// Process monster attacking a tribe
fn process_monster_vs_tribe<R: Rng>(
    monster_id: MonsterId,
    tribe_id: TribeId,
    monsters: &mut HashMap<MonsterId, Monster>,
    tribes: &mut HashMap<TribeId, Tribe>,
    _territory_map: &HashMap<TileCoord, TribeId>,
    world: &WorldData,
    current_tick: SimTick,
    rng: &mut R,
) -> Option<CombatEvent> {
    let monster = monsters.get(&monster_id)?;
    let tribe = tribes.get(&tribe_id)?;

    if monster.is_dead() || !tribe.is_alive {
        return None;
    }

    // Check if monster is adjacent to tribe territory
    let in_range = tribe.territory.iter().any(|coord| {
        monster.distance_to(coord, world.width) <= 2
    });

    if !in_range {
        return None;
    }

    // Combat calculation
    let monster_strength = monster.strength;
    let tribe_defense = tribe.military_strength() * 0.5; // Defenders get half strength for passive defense

    // Roll for combat
    let monster_roll = rng.gen::<f32>() * monster_strength;
    let tribe_roll = rng.gen::<f32>() * tribe_defense;

    let monster_damage;
    let tribe_casualties;

    if monster_roll > tribe_roll {
        // Monster wins the exchange
        let damage_ratio = (monster_roll - tribe_roll) / monster_strength;
        tribe_casualties = ((tribe.population.total() as f32 * damage_ratio * 0.05) as u32).max(1).min(20);

        // Monster takes some damage from retaliation
        monster_damage = (tribe_defense * rng.gen::<f32>() * 0.3).max(1.0);
    } else {
        // Tribe successfully defends
        tribe_casualties = 0;

        // Monster takes more damage
        monster_damage = (tribe_defense * rng.gen::<f32>() * 0.5).max(2.0);
    }

    // Apply damage
    let monster = monsters.get_mut(&monster_id)?;
    let monster_killed = monster.take_damage(monster_damage);

    if monster_killed {
        monster.state = MonsterState::Dead;
    } else if monster.should_flee() {
        monster.state = MonsterState::Fleeing;
    }

    let tribe = tribes.get_mut(&tribe_id)?;
    if tribe_casualties > 0 {
        // Apply casualties to population
        let current_pop = tribe.population.total();
        if current_pop > tribe_casualties {
            tribe.population.apply_casualties(tribe_casualties);
        } else {
            tribe.is_alive = false;
        }

        // Record event
        tribe.record_event(
            current_tick,
            TribeEventType::NaturalDisaster {
                disaster_type: format!("{} attack", monster.species.name()),
            },
        );
    }

    if !monster_killed {
        monster.kills += tribe_casualties;
    }

    Some(CombatEvent::MonsterVsTribe {
        monster_id,
        tribe_id,
        monster_damage,
        tribe_casualties,
        monster_killed,
    })
}

/// Process monster vs monster combat
fn process_monster_vs_monster<R: Rng>(
    attacker_id: MonsterId,
    defender_id: MonsterId,
    monsters: &mut HashMap<MonsterId, Monster>,
    _world: &WorldData,
    rng: &mut R,
) -> Option<CombatEvent> {
    // Get both monsters (need to check they exist and aren't dead)
    let attacker_strength = monsters.get(&attacker_id)?.strength;
    let defender_strength = monsters.get(&defender_id)?.strength;

    if monsters.get(&attacker_id)?.is_dead() || monsters.get(&defender_id)?.is_dead() {
        return None;
    }

    // Combat rolls
    let attacker_roll = rng.gen::<f32>() * attacker_strength;
    let defender_roll = rng.gen::<f32>() * defender_strength;

    let attacker_damage;
    let defender_damage;

    if attacker_roll > defender_roll {
        // Attacker wins
        defender_damage = attacker_strength * 0.3 * rng.gen::<f32>();
        attacker_damage = defender_strength * 0.1 * rng.gen::<f32>();
    } else {
        // Defender wins
        attacker_damage = defender_strength * 0.3 * rng.gen::<f32>();
        defender_damage = attacker_strength * 0.1 * rng.gen::<f32>();
    }

    // Apply damage to attacker
    let attacker_killed = {
        let attacker = monsters.get_mut(&attacker_id)?;
        let killed = attacker.take_damage(attacker_damage);
        if killed {
            attacker.state = MonsterState::Dead;
        } else if attacker.should_flee() {
            attacker.state = MonsterState::Fleeing;
        }
        killed
    };

    // Apply damage to defender
    let defender_killed = {
        let defender = monsters.get_mut(&defender_id)?;
        let killed = defender.take_damage(defender_damage);
        if killed {
            defender.state = MonsterState::Dead;
        } else if defender.should_flee() {
            defender.state = MonsterState::Fleeing;
        }
        killed
    };

    // Update kill counts
    if defender_killed {
        if let Some(attacker) = monsters.get_mut(&attacker_id) {
            attacker.kills += 1;
        }
    }
    if attacker_killed {
        if let Some(defender) = monsters.get_mut(&defender_id) {
            defender.kills += 1;
        }
    }

    Some(CombatEvent::MonsterVsMonster {
        attacker_id,
        defender_id,
        attacker_damage,
        defender_damage,
        attacker_killed,
        defender_killed,
    })
}

/// Find monster vs monster combat opportunities
pub fn find_monster_combat_targets<R: Rng>(
    monsters: &mut HashMap<MonsterId, Monster>,
    world: &WorldData,
    rng: &mut R,
) {
    let monster_ids: Vec<MonsterId> = monsters.keys().copied().collect();

    for id in &monster_ids {
        let monster = match monsters.get(id) {
            Some(m) if !m.is_dead() && m.state == MonsterState::Hunting => m,
            _ => continue,
        };

        let location = monster.location;
        let aggression = monster.species.stats().aggression;

        // Check for nearby monsters to fight
        if rng.gen::<f32>() < aggression * 0.2 {
            for other_id in &monster_ids {
                if id == other_id {
                    continue;
                }

                let other = match monsters.get(other_id) {
                    Some(m) if !m.is_dead() => m,
                    _ => continue,
                };

                // Check if adjacent
                if location.distance_wrapped(&other.location, world.width) <= 2 {
                    // Set to attack this monster
                    if let Some(monster) = monsters.get_mut(id) {
                        monster.state = MonsterState::Attacking(AttackTarget::Monster(*other_id));
                    }
                    break;
                }
            }
        }
    }
}

/// Combat event for logging/display
#[derive(Clone, Debug)]
pub enum CombatEvent {
    MonsterVsTribe {
        monster_id: MonsterId,
        tribe_id: TribeId,
        monster_damage: f32,
        tribe_casualties: u32,
        monster_killed: bool,
    },
    MonsterVsMonster {
        attacker_id: MonsterId,
        defender_id: MonsterId,
        attacker_damage: f32,
        defender_damage: f32,
        attacker_killed: bool,
        defender_killed: bool,
    },
}
