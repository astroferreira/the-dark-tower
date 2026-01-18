//! Diplomacy system - relations, treaties, alliances

use std::collections::HashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::simulation::types::{TribeId, RelationLevel, Treaty, TreatyType, TribeEventType};
use crate::simulation::params::SimulationParams;
use crate::simulation::simulation::SimulationState;

/// Diplomatic state tracking relations and treaties
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiplomacyState {
    /// Relations between tribe pairs (smaller ID, larger ID) -> relation
    relations: HashMap<(TribeId, TribeId), RelationLevel>,
    /// Active treaties
    treaties: Vec<Treaty>,
}

impl DiplomacyState {
    pub fn new() -> Self {
        DiplomacyState {
            relations: HashMap::new(),
            treaties: Vec::new(),
        }
    }

    /// Normalize tribe pair order for consistent key lookup
    fn normalize_pair(a: TribeId, b: TribeId) -> (TribeId, TribeId) {
        if a.0 <= b.0 {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// Get relation between two tribes
    pub fn get_relation(&self, tribe_a: TribeId, tribe_b: TribeId) -> RelationLevel {
        let key = Self::normalize_pair(tribe_a, tribe_b);
        self.relations.get(&key).copied().unwrap_or(RelationLevel::NEUTRAL)
    }

    /// Set relation between two tribes
    pub fn set_relation(&mut self, tribe_a: TribeId, tribe_b: TribeId, level: RelationLevel) {
        let key = Self::normalize_pair(tribe_a, tribe_b);
        self.relations.insert(key, level);
    }

    /// Adjust relation between two tribes
    pub fn adjust_relation(&mut self, tribe_a: TribeId, tribe_b: TribeId, delta: i8) {
        let key = Self::normalize_pair(tribe_a, tribe_b);
        let current = self.relations.entry(key).or_insert(RelationLevel::NEUTRAL);
        current.adjust(delta);
    }

    /// Add a treaty
    pub fn add_treaty(&mut self, treaty: Treaty) {
        self.treaties.push(treaty);
    }

    /// Get treaties involving a tribe
    pub fn get_treaties(&self, tribe: TribeId) -> Vec<&Treaty> {
        self.treaties.iter().filter(|t| t.involves(tribe)).collect()
    }

    /// Check if two tribes have a specific treaty type
    pub fn has_treaty(&self, tribe_a: TribeId, tribe_b: TribeId, treaty_type: TreatyType) -> bool {
        self.treaties.iter().any(|t| {
            t.treaty_type == treaty_type
                && ((t.tribe_a == tribe_a && t.tribe_b == tribe_b)
                    || (t.tribe_a == tribe_b && t.tribe_b == tribe_a))
        })
    }

    /// Check if two tribes have any non-aggression treaty
    pub fn has_non_aggression(&self, tribe_a: TribeId, tribe_b: TribeId) -> bool {
        self.has_treaty(tribe_a, tribe_b, TreatyType::NonAggression)
            || self.has_treaty(tribe_a, tribe_b, TreatyType::DefensiveAlliance)
            || self.has_treaty(tribe_a, tribe_b, TreatyType::MilitaryAlliance)
    }

    /// Remove expired treaties
    pub fn cleanup_expired(&mut self, current_tick: crate::simulation::types::SimTick) {
        self.treaties.retain(|t| !t.is_expired(current_tick));
    }

    /// Remove all relations and treaties involving a tribe
    pub fn remove_tribe(&mut self, tribe: TribeId) {
        self.relations.retain(|(a, b), _| *a != tribe && *b != tribe);
        self.treaties.retain(|t| !t.involves(tribe));
    }

    /// Get all tribes with relations to a specific tribe
    pub fn get_related_tribes(&self, tribe: TribeId) -> Vec<(TribeId, RelationLevel)> {
        self.relations
            .iter()
            .filter_map(|(&(a, b), &level)| {
                if a == tribe {
                    Some((b, level))
                } else if b == tribe {
                    Some((a, level))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Process diplomacy for a tick
pub fn process_diplomacy_tick<R: Rng>(
    state: &mut SimulationState,
    params: &SimulationParams,
    rng: &mut R,
) {
    // Cleanup expired treaties
    state.diplomacy.cleanup_expired(state.current_tick);

    // Natural relation drift towards neutral
    drift_relations(&mut state.diplomacy, params);

    // Process potential treaty formation
    let tribe_ids: Vec<TribeId> = state.tribes.keys().copied().collect();

    for &tribe_a in &tribe_ids {
        if !state.tribes.get(&tribe_a).map(|t| t.is_alive).unwrap_or(false) {
            continue;
        }

        let neighbors = state.neighboring_tribes(tribe_a);

        for &tribe_b in &neighbors {
            if !state.tribes.get(&tribe_b).map(|t| t.is_alive).unwrap_or(false) {
                continue;
            }

            // Consider forming treaties based on relations
            let relation = state.diplomacy.get_relation(tribe_a, tribe_b);

            // High relation and no treaty -> consider alliance
            if relation.0 >= 40 && !state.diplomacy.has_treaty(tribe_a, tribe_b, TreatyType::TradeAgreement) {
                if rng.gen::<f32>() < 0.1 {
                    form_treaty(state, tribe_a, tribe_b, TreatyType::TradeAgreement);
                }
            }

            // Very high relation -> consider defensive alliance
            if relation.0 >= 60 && !state.diplomacy.has_treaty(tribe_a, tribe_b, TreatyType::DefensiveAlliance) {
                if rng.gen::<f32>() < 0.05 {
                    form_treaty(state, tribe_a, tribe_b, TreatyType::DefensiveAlliance);
                }
            }

            // Hostile neighbors without non-aggression might consider one
            if relation.0 >= -20 && relation.0 < 20 && !state.diplomacy.has_non_aggression(tribe_a, tribe_b) {
                if rng.gen::<f32>() < 0.02 {
                    form_treaty(state, tribe_a, tribe_b, TreatyType::NonAggression);
                }
            }
        }
    }
}

/// Natural drift of relations towards neutral
fn drift_relations(diplomacy: &mut DiplomacyState, params: &SimulationParams) {
    let keys: Vec<(TribeId, TribeId)> = diplomacy.relations.keys().copied().collect();

    for key in keys {
        if let Some(relation) = diplomacy.relations.get_mut(&key) {
            if relation.0 > 0 {
                relation.adjust(-(params.relation_drift_rate as i8).max(1));
            } else if relation.0 < 0 {
                relation.adjust((params.relation_drift_rate as i8).max(1));
            }
        }
    }
}

/// Form a treaty between two tribes
fn form_treaty(state: &mut SimulationState, tribe_a: TribeId, tribe_b: TribeId, treaty_type: TreatyType) {
    let treaty = Treaty::new(treaty_type, tribe_a, tribe_b, state.current_tick);
    state.diplomacy.add_treaty(treaty);
    state.stats.total_treaties += 1;

    // Record events
    if let Some(tribe) = state.tribes.get_mut(&tribe_a) {
        tribe.record_event(
            state.current_tick,
            TribeEventType::TreatyFormed {
                with: tribe_b,
                treaty_type,
            },
        );
    }
    if let Some(tribe) = state.tribes.get_mut(&tribe_b) {
        tribe.record_event(
            state.current_tick,
            TribeEventType::TreatyFormed {
                with: tribe_a,
                treaty_type,
            },
        );
    }
}
