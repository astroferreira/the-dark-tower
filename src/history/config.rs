//! Configuration for the history simulation.

use serde::{Serialize, Deserialize};

/// Configuration parameters for history generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Number of years to simulate (default: 500).
    pub simulation_years: u32,

    /// Initial number of civilizations to spawn.
    pub initial_civilizations: u32,

    /// Initial legendary creatures to place.
    pub initial_legendary_creatures: u32,

    /// War frequency multiplier (1.0 = normal).
    pub war_frequency: f32,

    /// Monster activity multiplier (1.0 = normal).
    pub monster_activity: f32,

    /// Artifact creation rate multiplier (1.0 = normal).
    pub artifact_creation_rate: f32,

    /// Kaiju spawn probability per season (ultra-rare).
    pub kaiju_spawn_chance: f32,

    /// Magic intensity multiplier (1.0 = normal).
    pub magic_level: f32,

    /// Religion complexity multiplier (1.0 = normal).
    pub religion_complexity: f32,

    /// Maximum age (in years) of the oldest factions before simulation starts.
    /// Synthetic backstory records are placed in this window.
    pub prehistory_depth: u32,

    /// Maximum ancestor generations per dynasty in prehistory.
    pub prehistory_generations: u32,

    /// Trade route creation rate multiplier (1.0 = normal).
    pub trade_frequency: f32,

    /// Peaceful diplomacy event rate multiplier (1.0 = normal).
    pub diplomacy_rate: f32,

    /// Assassination attempt rate multiplier (1.0 = normal).
    pub assassination_rate: f32,

    /// Hero quest frequency multiplier (1.0 = normal).
    pub quest_rate: f32,

    /// Siege duration factor (1.0 = normal length sieges).
    pub siege_duration: f32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            simulation_years: 500,
            initial_civilizations: 800,
            initial_legendary_creatures: 1500,
            war_frequency: 1.0,
            monster_activity: 1.0,
            artifact_creation_rate: 1.0,
            kaiju_spawn_chance: 0.001,
            magic_level: 1.0,
            religion_complexity: 1.0,
            prehistory_depth: 200,
            prehistory_generations: 3,
            trade_frequency: 1.0,
            diplomacy_rate: 1.0,
            assassination_rate: 1.0,
            quest_rate: 1.0,
            siege_duration: 1.0,
        }
    }
}

impl HistoryConfig {
    /// Total number of simulation steps (seasons).
    pub fn total_steps(&self) -> u32 {
        self.simulation_years * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HistoryConfig::default();
        assert_eq!(config.simulation_years, 500);
        assert_eq!(config.total_steps(), 2000);
        assert_eq!(config.initial_civilizations, 800);
        assert_eq!(config.initial_legendary_creatures, 1500);
    }
}
