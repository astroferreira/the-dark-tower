//! Simulation playback controller.
//!
//! Provides pause/resume, stepping, and speed controls for interactive
//! history simulation viewing. Events are logged as they occur for
//! live display in the explorer.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::world::WorldData;
use crate::history::config::HistoryConfig;
use crate::history::data::GameData;
use crate::history::time::Date;
use crate::history::world_state::WorldHistory;
use crate::seasons::Season;

use super::setup::initialize_world;
use super::step::simulate_step;

/// Playback speed settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackSpeed {
    /// 1 season per tick
    Slow,
    /// 4 seasons (1 year) per tick
    Normal,
    /// 40 seasons (10 years) per tick
    Fast,
    /// 200 seasons (50 years) per tick
    VeryFast,
}

impl PlaybackSpeed {
    /// Steps per tick for this speed.
    pub fn steps_per_tick(&self) -> u32 {
        match self {
            PlaybackSpeed::Slow => 1,
            PlaybackSpeed::Normal => 4,
            PlaybackSpeed::Fast => 40,
            PlaybackSpeed::VeryFast => 200,
        }
    }

    /// Cycle to next speed.
    pub fn next(&self) -> Self {
        match self {
            PlaybackSpeed::Slow => PlaybackSpeed::Normal,
            PlaybackSpeed::Normal => PlaybackSpeed::Fast,
            PlaybackSpeed::Fast => PlaybackSpeed::VeryFast,
            PlaybackSpeed::VeryFast => PlaybackSpeed::Slow,
        }
    }

    /// Display name.
    pub fn name(&self) -> &'static str {
        match self {
            PlaybackSpeed::Slow => "1 season",
            PlaybackSpeed::Normal => "1 year",
            PlaybackSpeed::Fast => "10 years",
            PlaybackSpeed::VeryFast => "50 years",
        }
    }
}

/// Playback state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    /// Not started yet
    Ready,
    /// Currently simulating
    Playing,
    /// Paused
    Paused,
    /// Simulation complete
    Finished,
}

/// Recent event log entry for display.
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub date: Date,
    pub text: String,
    pub is_major: bool,
}

/// Interactive playback controller for step-by-step simulation viewing.
pub struct PlaybackController {
    /// The simulation RNG
    rng: ChaCha8Rng,
    /// Current playback state
    pub state: PlaybackState,
    /// Playback speed
    pub speed: PlaybackSpeed,
    /// Whether auto-play is active (continuously advancing)
    pub auto_play: bool,
    /// Total steps to simulate
    total_steps: u32,
    /// Steps completed so far
    pub steps_completed: u32,
    /// Recent event log (last N events for display)
    pub event_log: Vec<LogEntry>,
    /// Maximum log entries to keep
    max_log_entries: usize,
    /// Event count at the start of last tick (for detecting new events)
    last_event_count: usize,
}

impl PlaybackController {
    /// Create a new playback controller.
    pub fn new(config: &HistoryConfig) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(0), // Will be set during init
            state: PlaybackState::Ready,
            speed: PlaybackSpeed::Normal,
            auto_play: false,
            total_steps: config.total_steps(),
            steps_completed: 0,
            event_log: Vec::new(),
            max_log_entries: 200,
            last_event_count: 0,
        }
    }

    /// Initialize the world and prepare for playback.
    /// Returns the initial WorldHistory ready for stepping.
    pub fn initialize(
        &mut self,
        world: &WorldData,
        config: HistoryConfig,
        game_data: &GameData,
        seed: u64,
    ) -> WorldHistory {
        self.rng = ChaCha8Rng::seed_from_u64(seed);
        self.total_steps = config.total_steps();
        self.steps_completed = 0;
        self.event_log.clear();
        self.state = PlaybackState::Paused;

        let history = initialize_world(world, config, game_data, &mut self.rng);
        self.last_event_count = history.chronicle.len();

        // Log initial state
        self.event_log.push(LogEntry {
            date: history.current_date,
            text: format!(
                "World initialized: {} factions, {} creatures",
                history.active_faction_count(),
                history.legendary_creatures.len(),
            ),
            is_major: true,
        });

        history
    }

    /// Advance the simulation by one tick (speed-dependent number of steps).
    /// Returns the number of new events generated.
    pub fn tick(&mut self, history: &mut WorldHistory, world: &WorldData, game_data: &GameData) -> usize {
        if self.state == PlaybackState::Finished || self.state == PlaybackState::Ready {
            return 0;
        }

        let steps = self.speed.steps_per_tick()
            .min(self.total_steps.saturating_sub(self.steps_completed));

        if steps == 0 {
            self.state = PlaybackState::Finished;
            self.auto_play = false;
            return 0;
        }

        self.state = PlaybackState::Playing;
        let event_count_before = history.chronicle.len();

        for _ in 0..steps {
            simulate_step(history, world, game_data, &mut self.rng);
            self.steps_completed += 1;

            if self.steps_completed >= self.total_steps {
                self.state = PlaybackState::Finished;
                self.auto_play = false;
                break;
            }
        }

        // Collect new events for the log
        let new_event_count = history.chronicle.len();
        let new_events = new_event_count - event_count_before;

        if new_events > 0 {
            for event in history.chronicle.events.iter().skip(event_count_before) {
                self.event_log.push(LogEntry {
                    date: event.date,
                    text: event.title.clone(),
                    is_major: event.is_major,
                });
            }

            // Trim log if too long
            if self.event_log.len() > self.max_log_entries {
                let excess = self.event_log.len() - self.max_log_entries;
                self.event_log.drain(..excess);
            }
        }

        self.last_event_count = new_event_count;

        if self.state != PlaybackState::Finished {
            self.state = PlaybackState::Paused;
        }

        new_events
    }

    /// Step forward by exactly one season.
    pub fn step_season(&mut self, history: &mut WorldHistory, world: &WorldData, game_data: &GameData) -> usize {
        let old_speed = self.speed;
        self.speed = PlaybackSpeed::Slow;
        let result = self.tick(history, world, game_data);
        self.speed = old_speed;
        result
    }

    /// Step forward by one year (4 seasons).
    pub fn step_year(&mut self, history: &mut WorldHistory, world: &WorldData, game_data: &GameData) -> usize {
        let old_speed = self.speed;
        self.speed = PlaybackSpeed::Normal;
        let result = self.tick(history, world, game_data);
        self.speed = old_speed;
        result
    }

    /// Step forward by 10 years.
    pub fn step_decade(&mut self, history: &mut WorldHistory, world: &WorldData, game_data: &GameData) -> usize {
        let old_speed = self.speed;
        self.speed = PlaybackSpeed::Fast;
        let result = self.tick(history, world, game_data);
        self.speed = old_speed;
        result
    }

    /// Toggle auto-play on/off.
    pub fn toggle_auto_play(&mut self) {
        if self.state == PlaybackState::Finished || self.state == PlaybackState::Ready {
            return;
        }
        self.auto_play = !self.auto_play;
    }

    /// Cycle to the next playback speed.
    pub fn cycle_speed(&mut self) {
        self.speed = self.speed.next();
    }

    /// Get the progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.total_steps == 0 {
            return 1.0;
        }
        self.steps_completed as f32 / self.total_steps as f32
    }

    /// Get the progress as a percentage string.
    pub fn progress_label(&self) -> String {
        let pct = (self.progress() * 100.0) as u32;
        format!("{}%", pct)
    }

    /// Get the current simulation year.
    pub fn current_year(&self, history: &WorldHistory) -> u32 {
        history.current_date.year
    }

    /// Get a status string for display.
    pub fn status(&self, history: &WorldHistory) -> String {
        let state_str = match self.state {
            PlaybackState::Ready => "Ready",
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Finished => "Complete",
        };

        format!(
            "{} | Year {} | {} | {} events | {} factions | pop {}",
            state_str,
            history.current_date.year,
            self.progress_label(),
            history.chronicle.len(),
            history.active_faction_count(),
            history.total_population(),
        )
    }

    /// Get the most recent log entries (up to `count`).
    pub fn recent_log(&self, count: usize) -> &[LogEntry] {
        let start = self.event_log.len().saturating_sub(count);
        &self.event_log[start..]
    }

    /// Check if the simulation is complete.
    pub fn is_finished(&self) -> bool {
        self.state == PlaybackState::Finished
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
    fn test_playback_initialization() {
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut controller = PlaybackController::new(&config);
        let history = controller.initialize(&world, config, &game_data, 42);

        assert_eq!(controller.state, PlaybackState::Paused);
        assert_eq!(controller.steps_completed, 0);
        assert!(!controller.event_log.is_empty());
        assert!(history.active_faction_count() >= 1);
    }

    #[test]
    fn test_playback_stepping() {
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            simulation_years: 10,
            initial_civilizations: 2,
            ..HistoryConfig::default()
        };
        let mut controller = PlaybackController::new(&config);
        let mut history = controller.initialize(&world, config, &game_data, 42);

        // Step one season
        controller.step_season(&mut history, &world, &game_data);
        assert!(controller.steps_completed >= 1);

        // Step one year
        controller.step_year(&mut history, &world, &game_data);
        assert!(controller.steps_completed >= 5);

        // Step a decade
        controller.step_decade(&mut history, &world, &game_data);

        // Should eventually finish
        while !controller.is_finished() {
            controller.tick(&mut history, &world, &game_data);
        }
        assert_eq!(controller.state, PlaybackState::Finished);
    }

    #[test]
    fn test_playback_speed_cycle() {
        let config = HistoryConfig::default();
        let controller = PlaybackController::new(&config);
        assert_eq!(controller.speed, PlaybackSpeed::Normal);

        let mut speed = controller.speed;
        speed = speed.next();
        assert_eq!(speed, PlaybackSpeed::Fast);
        speed = speed.next();
        assert_eq!(speed, PlaybackSpeed::VeryFast);
        speed = speed.next();
        assert_eq!(speed, PlaybackSpeed::Slow);
        speed = speed.next();
        assert_eq!(speed, PlaybackSpeed::Normal);
    }

    #[test]
    fn test_playback_progress() {
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            simulation_years: 10,
            initial_civilizations: 2,
            ..HistoryConfig::default()
        };
        let mut controller = PlaybackController::new(&config);
        let mut history = controller.initialize(&world, config, &game_data, 42);

        assert!(controller.progress() < 0.01);

        // Run to completion
        while !controller.is_finished() {
            controller.tick(&mut history, &world, &game_data);
        }

        assert!((controller.progress() - 1.0).abs() < 0.01);
    }
}
