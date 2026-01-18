//! Conflict system - raids, battles, territory capture

use rand::Rng;

use crate::simulation::types::{TribeId, TileCoord, TribeEventType};
use crate::simulation::params::SimulationParams;
use crate::simulation::simulation::SimulationState;

/// Process potential conflicts for a tick
pub fn process_conflict_tick<R: Rng>(
    state: &mut SimulationState,
    params: &SimulationParams,
    rng: &mut R,
) {
    let tribe_ids: Vec<TribeId> = state.tribes.keys().copied().collect();

    for &attacker_id in &tribe_ids {
        // Get attacker info
        let (culture, strength, neighbors) = {
            let tribe = match state.tribes.get(&attacker_id) {
                Some(t) if t.is_alive => t,
                _ => continue,
            };

            // Skip if tribe is struggling
            if tribe.needs.food.satisfaction < 0.3 || tribe.population.warriors() < 10 {
                continue;
            }

            (
                tribe.culture.clone(),
                tribe.military_strength(),
                state.neighboring_tribes(attacker_id),
            )
        };

        for &defender_id in &neighbors {
            // Check relation
            let relation = state.diplomacy.get_relation(attacker_id, defender_id);

            // Don't attack allies
            if state.diplomacy.has_non_aggression(attacker_id, defender_id) {
                continue;
            }

            // Culture decides whether to consider attack
            if !culture.will_consider_attack(relation.0, rng) {
                continue;
            }

            // Decide between raid or battle
            let defender_strength = state.tribes.get(&defender_id)
                .map(|t| t.military_strength())
                .unwrap_or(0.0);

            let strength_ratio = strength / defender_strength.max(1.0);

            if strength_ratio > 1.5 && rng.gen::<f32>() < 0.2 {
                // Full battle if significantly stronger
                execute_battle(state, attacker_id, defender_id, params, rng);
            } else if strength_ratio > 0.8 && rng.gen::<f32>() < 0.3 {
                // Raid if roughly equal or stronger
                execute_raid(state, attacker_id, defender_id, params, rng);
            }

            // Only one attack per tribe per tick
            break;
        }
    }
}

/// Execute a raid (quick attack for loot)
fn execute_raid<R: Rng>(
    state: &mut SimulationState,
    attacker_id: TribeId,
    defender_id: TribeId,
    params: &SimulationParams,
    rng: &mut R,
) {
    let attacker_strength = state.tribes.get(&attacker_id)
        .map(|t| t.military_strength())
        .unwrap_or(0.0);

    let defender_strength = state.tribes.get(&defender_id)
        .map(|t| t.military_strength() * params.defender_bonus)
        .unwrap_or(0.0);

    let success_chance = attacker_strength / (attacker_strength + defender_strength);
    let success = rng.gen::<f32>() < success_chance;

    // Calculate casualties
    let casualty_rate = rng.gen_range(params.raid_casualty_min..params.raid_casualty_max);

    if success {
        // Attacker wins
        let attacker_casualties = (casualty_rate * 0.5) as u32; // Lower casualties on win
        let defender_casualties = (casualty_rate * 1.5) as u32;

        // Loot resources - extract loot first
        let loot = state.tribes.get_mut(&defender_id)
            .map(|d| d.stockpile.take_fraction(params.raid_loot_fraction))
            .unwrap_or_default();

        // Apply to attacker
        if let Some(attacker) = state.tribes.get_mut(&attacker_id) {
            attacker.stockpile.add_all(&loot);
            attacker.population.apply_casualties(attacker_casualties.max(1));
            attacker.record_event(
                state.current_tick,
                TribeEventType::RaidLaunched {
                    target: defender_id,
                    success: true,
                },
            );
        }

        // Apply to defender
        if let Some(defender) = state.tribes.get_mut(&defender_id) {
            defender.population.apply_casualties(defender_casualties.max(1));
            defender.record_event(
                state.current_tick,
                TribeEventType::RaidDefended {
                    attacker: attacker_id,
                    success: false,
                },
            );
        }
    } else {
        // Defender wins
        let attacker_casualties = (casualty_rate * 1.5) as u32;
        let defender_casualties = (casualty_rate * 0.5) as u32;

        if let Some(attacker) = state.tribes.get_mut(&attacker_id) {
            attacker.population.apply_casualties(attacker_casualties.max(1));
            attacker.record_event(
                state.current_tick,
                TribeEventType::RaidLaunched {
                    target: defender_id,
                    success: false,
                },
            );
        }

        if let Some(defender) = state.tribes.get_mut(&defender_id) {
            defender.population.apply_casualties(defender_casualties.max(1));
            defender.record_event(
                state.current_tick,
                TribeEventType::RaidDefended {
                    attacker: attacker_id,
                    success: true,
                },
            );
        }
    }

    // Worsen relations
    state.diplomacy.adjust_relation(attacker_id, defender_id, params.raid_relation_penalty);
    state.stats.total_raids += 1;
}

/// Execute a full battle (for territory)
fn execute_battle<R: Rng>(
    state: &mut SimulationState,
    attacker_id: TribeId,
    defender_id: TribeId,
    params: &SimulationParams,
    rng: &mut R,
) {
    let attacker_strength = state.tribes.get(&attacker_id)
        .map(|t| t.military_strength())
        .unwrap_or(0.0);

    let defender_strength = state.tribes.get(&defender_id)
        .map(|t| t.military_strength() * params.defender_bonus)
        .unwrap_or(0.0);

    let total_strength = attacker_strength + defender_strength;
    let success_chance = attacker_strength / total_strength.max(1.0);
    let success = rng.gen::<f32>() < success_chance;

    // Higher casualties in battles
    let casualty_rate = rng.gen_range(params.battle_casualty_min..params.battle_casualty_max);

    if success {
        // Attacker wins
        let attacker_casualties = ((casualty_rate * 0.7) * attacker_strength) as u32;
        let defender_casualties = ((casualty_rate * 1.3) * defender_strength) as u32;

        // Capture territory
        let captured_tile = capture_territory(state, attacker_id, defender_id, rng);

        if let Some(attacker) = state.tribes.get_mut(&attacker_id) {
            attacker.population.apply_casualties(attacker_casualties.max(1));
            attacker.record_event(
                state.current_tick,
                TribeEventType::BattleWon { against: defender_id },
            );
        }

        if let Some(defender) = state.tribes.get_mut(&defender_id) {
            defender.population.apply_casualties(defender_casualties.max(1));
            defender.record_event(
                state.current_tick,
                TribeEventType::BattleLost { against: attacker_id },
            );
            if let Some(tile) = captured_tile {
                defender.record_event(
                    state.current_tick,
                    TribeEventType::TerritoryLost {
                        tile,
                        to: Some(attacker_id),
                    },
                );
            }
        }
    } else {
        // Defender wins
        let attacker_casualties = ((casualty_rate * 1.3) * attacker_strength) as u32;
        let defender_casualties = ((casualty_rate * 0.7) * defender_strength) as u32;

        if let Some(attacker) = state.tribes.get_mut(&attacker_id) {
            attacker.population.apply_casualties(attacker_casualties.max(1));
            attacker.record_event(
                state.current_tick,
                TribeEventType::BattleLost { against: defender_id },
            );
        }

        if let Some(defender) = state.tribes.get_mut(&defender_id) {
            defender.population.apply_casualties(defender_casualties.max(1));
            defender.record_event(
                state.current_tick,
                TribeEventType::BattleWon { against: attacker_id },
            );
        }
    }

    // Severely worsen relations
    state.diplomacy.adjust_relation(attacker_id, defender_id, params.raid_relation_penalty * 2);
    state.stats.total_battles += 1;
}

/// Capture a tile from the defender
fn capture_territory<R: Rng>(
    state: &mut SimulationState,
    attacker_id: TribeId,
    defender_id: TribeId,
    rng: &mut R,
) -> Option<TileCoord> {
    // Find a border tile owned by defender adjacent to attacker's territory
    let attacker_territory: std::collections::HashSet<TileCoord> = state.tribes.get(&attacker_id)
        .map(|t| t.territory.clone())
        .unwrap_or_default();

    let defender_territory: Vec<TileCoord> = state.tribes.get(&defender_id)
        .map(|t| t.territory.iter().copied().collect())
        .unwrap_or_default();

    // Find defender tiles adjacent to attacker
    let border_tiles: Vec<TileCoord> = defender_territory
        .iter()
        .filter(|coord| {
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let neighbor = TileCoord::new(
                        (coord.x as i32 + dx).rem_euclid(512) as usize, // TODO: actual width
                        (coord.y as i32 + dy).clamp(0, 255) as usize,
                    );
                    if attacker_territory.contains(&neighbor) {
                        return true;
                    }
                }
            }
            false
        })
        .copied()
        .collect();

    if border_tiles.is_empty() {
        return None;
    }

    // Pick a random border tile
    let captured = border_tiles[rng.gen_range(0..border_tiles.len())];

    // Transfer ownership
    if let Some(defender) = state.tribes.get_mut(&defender_id) {
        defender.lose_tile(&captured);
    }

    if let Some(attacker) = state.tribes.get_mut(&attacker_id) {
        attacker.claim_tile(captured);
    }

    state.territory_map.insert(captured, attacker_id);

    Some(captured)
}
