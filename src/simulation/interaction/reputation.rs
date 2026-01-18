//! Reputation system - tribe-species relationships
//!
//! Tracks reputation between tribes and monster species. Species remember
//! tribal actions collectively (not per-individual monster).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::simulation::types::TribeId;
use crate::simulation::monsters::MonsterSpecies;

/// Species disposition category - determines baseline reputation and caps
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpeciesDisposition {
    /// Always hostile (Dragons, Hydras, etc.) - baseline -75, max -25
    AlwaysHostile,
    /// Territorial (Trolls, Bears, etc.) - baseline -30, max +20
    Territorial,
    /// Neutral (Wolves, Spiders, etc.) - baseline 0, max +50
    Neutral,
    /// Mythical (Phoenix) - baseline +20, max +80
    Mythical,
    /// Undead (BogWight) - baseline -100, max -50
    Undead,
}

impl SpeciesDisposition {
    /// Get the baseline reputation value for this disposition
    pub fn baseline(&self) -> i8 {
        match self {
            SpeciesDisposition::AlwaysHostile => -75,
            SpeciesDisposition::Territorial => -30,
            SpeciesDisposition::Neutral => 0,
            SpeciesDisposition::Mythical => 20,
            SpeciesDisposition::Undead => -100,
        }
    }

    /// Get the maximum positive reputation possible for this disposition
    pub fn max_positive(&self) -> i8 {
        match self {
            SpeciesDisposition::AlwaysHostile => -25,
            SpeciesDisposition::Territorial => 20,
            SpeciesDisposition::Neutral => 50,
            SpeciesDisposition::Mythical => 80,
            SpeciesDisposition::Undead => -50,
        }
    }

    /// Get the minimum (most negative) reputation possible
    pub fn min_negative(&self) -> i8 {
        -100
    }
}

/// Reputation with a specific species, including decay mechanics
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SpeciesReputation {
    /// Current reputation value (-100 to +100)
    pub current: i8,
    /// Natural resting point (determined by disposition)
    pub baseline: i8,
    /// Maximum reputation cap (determined by disposition)
    pub max_cap: i8,
    /// Ticks remaining until decay resumes (prevents immediate decay after events)
    pub momentum: u8,
}

impl SpeciesReputation {
    /// Create a new reputation with disposition-based defaults
    pub fn new(disposition: SpeciesDisposition) -> Self {
        SpeciesReputation {
            current: disposition.baseline(),
            baseline: disposition.baseline(),
            max_cap: disposition.max_positive(),
            momentum: 0,
        }
    }

    /// Adjust reputation by delta, respecting caps
    pub fn adjust(&mut self, delta: i8) {
        let new_value = (self.current as i16 + delta as i16).clamp(-100, self.max_cap as i16) as i8;
        self.current = new_value;
        // Set momentum to prevent immediate decay
        self.momentum = 20;
    }

    /// Process decay toward baseline (call once per year)
    pub fn decay(&mut self) {
        if self.momentum > 0 {
            self.momentum -= 1;
            return;
        }

        // Decay by 1 point toward baseline
        if self.current > self.baseline {
            self.current -= 1;
        } else if self.current < self.baseline {
            self.current += 1;
        }
    }

    /// Check if this reputation indicates the species is fearful of the tribe
    pub fn is_fearful(&self) -> bool {
        self.current >= 30
    }

    /// Check if this reputation indicates the species is tolerant
    pub fn is_tolerant(&self) -> bool {
        self.current >= 0
    }

    /// Check if this reputation indicates the species is hostile
    pub fn is_hostile(&self) -> bool {
        self.current <= -30
    }

    /// Check if this reputation indicates the species is vengeful
    pub fn is_vengeful(&self) -> bool {
        self.current <= -60
    }

    /// Get a display label for this reputation level
    pub fn status_label(&self) -> &'static str {
        if self.is_vengeful() {
            "Vengeful"
        } else if self.is_hostile() {
            "Hostile"
        } else if self.is_fearful() {
            "Fearful"
        } else if self.is_tolerant() {
            "Tolerant"
        } else {
            "Wary"
        }
    }
}

/// State tracking all tribe-species reputations
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReputationState {
    /// Map of (tribe, species) -> reputation
    tribe_species: HashMap<(TribeId, MonsterSpecies), SpeciesReputation>,
}

impl ReputationState {
    /// Create a new empty reputation state
    pub fn new() -> Self {
        ReputationState {
            tribe_species: HashMap::new(),
        }
    }

    /// Get reputation between a tribe and species, initializing if needed
    pub fn get(&self, tribe: TribeId, species: MonsterSpecies) -> SpeciesReputation {
        self.tribe_species
            .get(&(tribe, species))
            .copied()
            .unwrap_or_else(|| SpeciesReputation::new(species.disposition()))
    }

    /// Get mutable reputation, initializing if needed
    pub fn get_mut(&mut self, tribe: TribeId, species: MonsterSpecies) -> &mut SpeciesReputation {
        self.tribe_species
            .entry((tribe, species))
            .or_insert_with(|| SpeciesReputation::new(species.disposition()))
    }

    /// Adjust reputation between tribe and species
    pub fn adjust(&mut self, tribe: TribeId, species: MonsterSpecies, delta: i8) {
        let rep = self.get_mut(tribe, species);
        rep.adjust(delta);
    }

    /// Process decay for all reputations (call once per year)
    pub fn process_decay(&mut self) {
        for rep in self.tribe_species.values_mut() {
            rep.decay();
        }
    }

    /// Get all species reputations for a tribe
    pub fn get_tribe_reputations(&self, tribe: TribeId) -> Vec<(MonsterSpecies, SpeciesReputation)> {
        self.tribe_species
            .iter()
            .filter_map(|(&(t, species), &rep)| {
                if t == tribe {
                    Some((species, rep))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Remove all reputations involving a tribe (when tribe goes extinct)
    pub fn remove_tribe(&mut self, tribe: TribeId) {
        self.tribe_species.retain(|(t, _), _| *t != tribe);
    }

    /// Check if a monster species should attack a tribe based on reputation
    /// Returns an aggression modifier: positive = more aggressive, negative = less
    pub fn aggression_modifier(&self, tribe: TribeId, species: MonsterSpecies) -> f32 {
        let rep = self.get(tribe, species);
        if rep.is_vengeful() {
            0.3 // +30% aggression
        } else if rep.is_hostile() {
            0.1 // +10% aggression
        } else if rep.is_tolerant() {
            -0.2 // -20% aggression
        } else if rep.is_fearful() {
            -0.5 // -50% aggression (mostly avoid)
        } else {
            0.0 // No modifier
        }
    }

    /// Check if a monster should skip attacking this tribe entirely
    pub fn should_skip_tribe(&self, tribe: TribeId, species: MonsterSpecies) -> bool {
        let rep = self.get(tribe, species);
        rep.is_fearful() // Species avoids tribes they fear
    }
}

/// Reputation change events
#[derive(Clone, Copy, Debug)]
pub enum ReputationEvent {
    /// Killed a significant monster (Dragon, Hydra, etc.)
    KilledSignificant,
    /// Killed a regular monster
    KilledRegular,
    /// Attacked a monster but didn't kill
    AttackedNoKill,
    /// Monster fled from tribe territory
    MonsterFled,
    /// Peaceful coexistence (monster in/near territory without conflict)
    PeacefulCoexistence,
}

impl ReputationEvent {
    /// Get the reputation change for this event
    pub fn reputation_change(&self) -> i8 {
        match self {
            ReputationEvent::KilledSignificant => -25,
            ReputationEvent::KilledRegular => -15,
            ReputationEvent::AttackedNoKill => -5,
            ReputationEvent::MonsterFled => 2,
            ReputationEvent::PeacefulCoexistence => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disposition_baselines() {
        assert_eq!(SpeciesDisposition::AlwaysHostile.baseline(), -75);
        assert_eq!(SpeciesDisposition::Neutral.baseline(), 0);
        assert_eq!(SpeciesDisposition::Mythical.baseline(), 20);
    }

    #[test]
    fn test_reputation_caps() {
        let mut rep = SpeciesReputation::new(SpeciesDisposition::AlwaysHostile);
        // Try to go above cap
        rep.adjust(100);
        assert_eq!(rep.current, -25); // Capped at max_cap

        let mut rep2 = SpeciesReputation::new(SpeciesDisposition::Neutral);
        rep2.adjust(100);
        assert_eq!(rep2.current, 50); // Capped at +50
    }

    #[test]
    fn test_reputation_decay() {
        let mut rep = SpeciesReputation::new(SpeciesDisposition::Neutral);
        rep.adjust(-50); // Go to -50, momentum = 20

        // First 20 decays should not change value due to momentum
        for _ in 0..20 {
            rep.decay();
        }
        assert_eq!(rep.current, -50);
        assert_eq!(rep.momentum, 0);

        // Now decay should start
        rep.decay();
        assert_eq!(rep.current, -49);
    }

    #[test]
    fn test_status_labels() {
        let mut rep = SpeciesReputation::new(SpeciesDisposition::Neutral);
        assert_eq!(rep.status_label(), "Tolerant");

        rep.current = 30;
        assert_eq!(rep.status_label(), "Fearful");

        rep.current = -30;
        assert_eq!(rep.status_label(), "Hostile");

        rep.current = -60;
        assert_eq!(rep.status_label(), "Vengeful");
    }
}
