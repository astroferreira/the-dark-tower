//! Configuration parameters for the civilization simulation

use serde::{Deserialize, Serialize};

/// Main configuration for the simulation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationParams {
    // Initialization
    /// Number of tribes to spawn at start
    pub initial_tribe_count: usize,
    /// Starting population per tribe
    pub initial_tribe_population: u32,
    /// Minimum tile distance between tribe spawns
    pub min_tribe_separation: usize,

    // Population dynamics
    /// Food consumed per population per tick
    pub food_per_pop_per_tick: f32,
    /// Water consumed per population per tick
    pub water_per_pop_per_tick: f32,
    /// Base growth rate per tick (0.02 = 2%)
    pub base_growth_rate: f32,
    /// Base death rate per tick (0.01 = 1%)
    pub base_death_rate: f32,
    /// Maximum growth rate even in perfect conditions
    pub max_growth_rate: f32,
    /// Population threshold for tribe splitting
    pub tribe_split_population: u32,
    /// Minimum population for new split tribe
    pub split_min_population: u32,

    // Territory
    /// Initial territory radius around capital
    pub initial_territory_radius: usize,
    /// Population required per tile of territory
    pub pop_per_territory_tile: f32,
    /// Maximum tiles a tribe can control
    pub max_territory_size: usize,

    // Combat
    /// Defender strength multiplier
    pub defender_bonus: f32,
    /// Minimum casualties for raids (fraction)
    pub raid_casualty_min: f32,
    /// Maximum casualties for raids (fraction)
    pub raid_casualty_max: f32,
    /// Minimum casualties for battles (fraction)
    pub battle_casualty_min: f32,
    /// Maximum casualties for battles (fraction)
    pub battle_casualty_max: f32,
    /// Loot percentage on successful raid
    pub raid_loot_fraction: f32,

    // Diplomacy
    /// Natural relation drift towards neutral per tick
    pub relation_drift_rate: f32,
    /// Relation boost from successful trade
    pub trade_relation_boost: i8,
    /// Relation penalty from raid
    pub raid_relation_penalty: i8,
    /// Relation penalty from broken treaty
    pub treaty_break_penalty: i8,

    // Technology
    /// Research points generated per worker per tick
    pub research_per_worker: f32,
    /// Base research required to advance an age
    pub base_age_research: f32,
    /// Age research multiplier (each age costs more)
    pub age_research_multiplier: f32,

    // Resources
    /// Maximum stockpile size relative to population
    pub max_stockpile_per_pop: f32,
    /// Resource extraction efficiency base
    pub extraction_efficiency: f32,

    // Needs thresholds
    /// Food satisfaction: well-fed threshold (food per pop)
    pub food_well_fed: f32,
    /// Food satisfaction: starving threshold
    pub food_starving: f32,
    /// Shelter satisfaction: good shelter threshold (buildings per pop)
    pub shelter_good: f32,
    /// Health satisfaction: healthy threshold
    pub health_good: f32,
    /// Security satisfaction: safe threshold (warriors per pop)
    pub security_safe: f32,

    // Monster settings
    /// Maximum monsters in the world
    pub max_monsters: usize,
    /// Minimum distance from tribe territory for monster spawning
    pub monster_min_tribe_distance: usize,
    /// Base monster spawn chance per tick
    pub monster_spawn_chance: f32,
    /// Monster spawn check interval (ticks)
    pub monster_spawn_interval: u64,
}

impl Default for SimulationParams {
    fn default() -> Self {
        SimulationParams {
            // Initialization
            initial_tribe_count: 10,
            initial_tribe_population: 100,
            min_tribe_separation: 20,

            // Population dynamics
            food_per_pop_per_tick: 0.1,
            water_per_pop_per_tick: 0.05,
            base_growth_rate: 0.02,
            base_death_rate: 0.01,
            max_growth_rate: 0.05,
            tribe_split_population: 500,
            split_min_population: 100,

            // Territory
            initial_territory_radius: 3,
            pop_per_territory_tile: 10.0,
            max_territory_size: 100,

            // Combat
            defender_bonus: 1.2,
            raid_casualty_min: 0.05,
            raid_casualty_max: 0.15,
            battle_casualty_min: 0.10,
            battle_casualty_max: 0.30,
            raid_loot_fraction: 0.2,

            // Diplomacy
            relation_drift_rate: 0.5,
            trade_relation_boost: 5,
            raid_relation_penalty: -20,
            treaty_break_penalty: -30,

            // Technology
            research_per_worker: 0.01,
            base_age_research: 1000.0,
            age_research_multiplier: 2.0,

            // Resources
            max_stockpile_per_pop: 10.0,
            extraction_efficiency: 1.0,

            // Needs thresholds
            food_well_fed: 2.0,
            food_starving: 0.5,
            shelter_good: 0.5,
            health_good: 0.7,
            security_safe: 0.1,

            // Monster settings
            max_monsters: 50,
            monster_min_tribe_distance: 5,
            monster_spawn_chance: 0.15,
            monster_spawn_interval: 4,
        }
    }
}

impl SimulationParams {
    /// Create params for a fast test run
    pub fn fast_test() -> Self {
        let mut params = Self::default();
        params.initial_tribe_count = 5;
        params.initial_tribe_population = 50;
        params.base_growth_rate = 0.05;
        params.tribe_split_population = 200;
        params
    }

    /// Create params for a detailed simulation
    pub fn detailed() -> Self {
        let mut params = Self::default();
        params.initial_tribe_count = 15;
        params.initial_tribe_population = 150;
        params.base_growth_rate = 0.015;
        params
    }
}
