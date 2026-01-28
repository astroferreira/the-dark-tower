//! History simulation engine.
//!
//! Orchestrates the complete history generation process:
//! initialization, step-by-step simulation, and era definition.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::world::WorldData;
use crate::history::config::HistoryConfig;
use crate::history::data::GameData;
use crate::history::time::{Date, Era};
use crate::history::world_state::WorldHistory;
use crate::seasons::Season;

use super::metrics::SimulationMetrics;
use super::setup::initialize_world;
use super::step::simulate_step;

/// The history simulation engine.
pub struct HistoryEngine {
    pub rng: ChaCha8Rng,
}

impl HistoryEngine {
    /// Create a new engine with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Run the complete history simulation.
    pub fn simulate(
        &mut self,
        world: &WorldData,
        config: HistoryConfig,
    ) -> WorldHistory {
        self.simulate_with_data(world, config, &GameData::defaults())
    }

    /// Run the complete history simulation with custom game data.
    pub fn simulate_with_data(
        &mut self,
        world: &WorldData,
        config: HistoryConfig,
        game_data: &GameData,
    ) -> WorldHistory {
        let total_steps = config.total_steps();

        // Initialize world
        let mut history = initialize_world(world, config, game_data, &mut self.rng);

        // Run simulation steps
        for step in 0..total_steps {
            simulate_step(&mut history, world, game_data, &mut self.rng);

            // Progress reporting every 100 years
            if step > 0 && step % 400 == 0 {
                let year = history.current_date.year;
                eprintln!(
                    "  History: year {}, {} factions, {} events, pop {}",
                    year,
                    history.active_faction_count(),
                    history.chronicle.len(),
                    history.total_population(),
                );
            }
        }

        // Define eras from major events
        self.define_eras(&mut history);

        // Compute and print quality metrics
        let metrics = SimulationMetrics::compute(&history);
        eprintln!("{}", metrics.report());

        history
    }

    /// Run simulation and return both history and metrics.
    pub fn simulate_with_metrics(
        &mut self,
        world: &WorldData,
        config: HistoryConfig,
        game_data: &GameData,
    ) -> (WorldHistory, SimulationMetrics) {
        let total_steps = config.total_steps();
        let mut history = initialize_world(world, config, game_data, &mut self.rng);

        for step in 0..total_steps {
            simulate_step(&mut history, world, game_data, &mut self.rng);

            if step > 0 && step % 400 == 0 {
                let year = history.current_date.year;
                eprintln!(
                    "  History: year {}, {} factions, {} events, pop {}",
                    year,
                    history.active_faction_count(),
                    history.chronicle.len(),
                    history.total_population(),
                );
            }
        }
        self.define_eras(&mut history);
        let metrics = SimulationMetrics::compute(&history);
        (history, metrics)
    }

    /// Run the simulation for a specific number of steps (for playback).
    pub fn simulate_steps(
        &mut self,
        history: &mut WorldHistory,
        world: &WorldData,
        game_data: &GameData,
        steps: u32,
    ) {
        for _ in 0..steps {
            simulate_step(history, world, game_data, &mut self.rng);
        }
    }

    /// Define eras based on major events in the chronicle.
    fn define_eras(&self, history: &mut WorldHistory) {
        let major_events = history.chronicle.major_events();
        if major_events.is_empty() {
            // Single era for the whole history
            let mut era = Era::new(
                history.id_generators.next_era(),
                "The Age of Beginning".to_string(),
                Date::new(1, Season::Spring),
            );
            era.close(history.current_date);
            history.timeline.begin_era(era);
            return;
        }

        // Group major events into eras (roughly one era per 50 years)
        let era_length = 50;
        let total_years = history.current_date.year;
        let mut era_num = 1;

        let mut year = 1u32;
        while year < total_years {
            let era_end_year = (year + era_length).min(total_years);
            let era_end = Date::new(era_end_year, Season::Winter);
            let era_start = Date::new(year, Season::Spring);

            // Find the most significant event in this era's span
            let era_events: Vec<_> = major_events.iter()
                .filter(|e| e.date >= era_start && e.date <= era_end)
                .collect();

            let defining_ids: Vec<_> = era_events.iter().map(|e| e.id).collect();
            let name = if let Some(event) = era_events.first() {
                match event.event_type {
                    crate::history::events::types::EventType::WarDeclared |
                    crate::history::events::types::EventType::WarEnded => {
                        format!("The Age of Conflict (Era {})", era_num)
                    }
                    crate::history::events::types::EventType::FactionFounded => {
                        format!("The Age of Founding (Era {})", era_num)
                    }
                    crate::history::events::types::EventType::FactionDestroyed => {
                        format!("The Age of Decline (Era {})", era_num)
                    }
                    crate::history::events::types::EventType::Plague |
                    crate::history::events::types::EventType::MagicalCatastrophe => {
                        format!("The Age of Calamity (Era {})", era_num)
                    }
                    crate::history::events::types::EventType::CreatureSlain => {
                        format!("The Age of Heroes (Era {})", era_num)
                    }
                    crate::history::events::types::EventType::ReligionFounded => {
                        format!("The Age of Faith (Era {})", era_num)
                    }
                    _ => format!("Era {}", era_num),
                }
            } else {
                format!("The Quiet Age (Era {})", era_num)
            };

            let mut era = Era::new(
                history.id_generators.next_era(),
                name,
                era_start,
            );
            era.defining_events = defining_ids;
            era.close(era_end);
            history.timeline.begin_era(era);

            year = era_end_year + 1;
            era_num += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::biomes::ExtendedBiome;
    use crate::tilemap::Tilemap;
    use crate::seeds::WorldSeeds;
    use crate::scale::MapScale;
    use crate::plates::types::PlateId;
    use crate::water_bodies::WaterBodyId;

    fn make_test_world() -> WorldData {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.3);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);

        for x in 0..width {
            *biomes.get_mut(x, 0) = ExtendedBiome::Ocean;
            *heightmap.get_mut(x, 0) = -0.1;
        }

        let seeds = WorldSeeds::from_master(42);
        let scale = MapScale::new(1.0);
        let temperature = Tilemap::new_with(width, height, 15.0);
        let moisture = Tilemap::new_with(width, height, 0.5);
        let stress_map = Tilemap::new_with(width, height, 0.0);
        let plate_map = Tilemap::new_with(width, height, PlateId(0));
        let water_body_map = Tilemap::new_with(width, height, WaterBodyId::NONE);
        let water_depth = Tilemap::new_with(width, height, 0.0);

        WorldData::new(
            seeds, scale, heightmap, temperature, moisture,
            biomes, stress_map, plate_map, Vec::new(),
            None, water_body_map, Vec::new(), water_depth,
            None, None,
        )
    }

    #[test]
    fn test_full_simulation() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            initial_legendary_creatures: 3,
            ..HistoryConfig::default()
        };
        let prehistory_depth = config.prehistory_depth;

        let mut engine = HistoryEngine::new(42);
        let history = engine.simulate(&world, config);

        let summary = history.summary();
        eprintln!("{}", summary);

        // years_simulated includes prehistory offset + simulation years
        assert!(summary.years_simulated >= 50 + prehistory_depth);
        assert!(summary.total_events > 10);
        assert!(summary.active_factions >= 1);
        assert!(!history.timeline.eras.is_empty());
    }

    #[test]
    fn test_era_definition() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 100,
            initial_civilizations: 4,
            initial_legendary_creatures: 5,
            ..HistoryConfig::default()
        };

        let mut engine = HistoryEngine::new(99);
        let history = engine.simulate(&world, config);

        assert!(!history.timeline.eras.is_empty());
        for era in &history.timeline.eras {
            eprintln!("  {} ({} - {:?})", era.name, era.start, era.end);
        }
    }
}
