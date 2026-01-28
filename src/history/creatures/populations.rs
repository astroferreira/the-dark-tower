//! Creature population tracking.
//!
//! Populations represent groups of the same species. Leaderless populations
//! are scattered threats; populations with a legendary leader become organized
//! forces capable of raids and territorial control.

use serde::{Serialize, Deserialize};
use crate::history::{PopulationId, CreatureSpeciesId, LegendaryCreatureId};
use crate::history::time::Date;

/// A population of creatures of the same species.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreaturePopulation {
    pub id: PopulationId,
    pub species_id: CreatureSpeciesId,
    pub count: u32,
    pub location: (usize, usize),
    pub territory: Vec<(usize, usize)>,

    /// If led by a legendary creature, the population is organized.
    pub leader: Option<LegendaryCreatureId>,
    pub aggression_level: f32,
    pub last_raid: Option<Date>,
}

impl CreaturePopulation {
    pub fn new(
        id: PopulationId,
        species_id: CreatureSpeciesId,
        count: u32,
        location: (usize, usize),
    ) -> Self {
        Self {
            id,
            species_id,
            count,
            location,
            territory: vec![location],
            leader: None,
            aggression_level: 0.3,
            last_raid: None,
        }
    }

    /// Whether this population has an organizing leader.
    pub fn is_organized(&self) -> bool {
        self.leader.is_some()
    }

    /// Set a new leader for this population.
    pub fn set_leader(&mut self, leader: LegendaryCreatureId) {
        self.leader = Some(leader);
        // Organized populations become more aggressive
        self.aggression_level = (self.aggression_level + 0.3).min(1.0);
    }

    /// Remove the leader (killed, fled, etc.).
    pub fn remove_leader(&mut self) {
        self.leader = None;
        // Leaderless populations scatter
        self.aggression_level = (self.aggression_level - 0.4).max(0.0);
    }

    /// Whether the population is aggressive enough to raid settlements.
    pub fn will_raid(&self) -> bool {
        self.is_organized() && self.aggression_level > 0.5
    }

    /// Grow or shrink the population.
    pub fn adjust_count(&mut self, delta: i32) {
        if delta >= 0 {
            self.count = self.count.saturating_add(delta as u32);
        } else {
            self.count = self.count.saturating_sub((-delta) as u32);
        }
    }

    /// Whether the population has been wiped out.
    pub fn is_extinct(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_population_basics() {
        let mut pop = CreaturePopulation::new(
            PopulationId(0),
            CreatureSpeciesId(0),
            50,
            (10, 20),
        );
        assert!(!pop.is_organized());
        assert!(!pop.will_raid());

        pop.set_leader(LegendaryCreatureId(0));
        assert!(pop.is_organized());
        assert!(pop.will_raid()); // aggression should be 0.6+

        pop.remove_leader();
        assert!(!pop.is_organized());
    }

    #[test]
    fn test_population_adjust() {
        let mut pop = CreaturePopulation::new(
            PopulationId(0), CreatureSpeciesId(0), 50, (0, 0),
        );
        pop.adjust_count(10);
        assert_eq!(pop.count, 60);
        pop.adjust_count(-100);
        assert_eq!(pop.count, 0);
        assert!(pop.is_extinct());
    }
}
