//! Iterative testing harness for history simulation.
//!
//! Runs N simulations with different seeds, collects quality metrics,
//! and enables A/B comparison of parameter changes.

use crate::world::WorldData;
use crate::history::config::HistoryConfig;
use crate::history::data::GameData;
use crate::history::world_state::WorldHistory;
use super::engine::HistoryEngine;
use super::metrics::{SimulationMetrics, welch_t_test};

/// Configuration for a batch of simulation runs.
#[derive(Clone, Debug)]
pub struct BatchConfig {
    pub name: String,
    pub history_config: HistoryConfig,
    pub num_runs: u32,
    pub base_seed: u64,
}

/// Results collected from a batch of runs.
#[derive(Clone, Debug)]
pub struct BatchResults {
    pub name: String,
    pub metrics: Vec<SimulationMetrics>,
    pub mean_quality: f32,
    pub stddev_quality: f32,
    pub min_quality: f32,
    pub max_quality: f32,
}

impl BatchResults {
    /// Collect quality scores.
    pub fn quality_scores(&self) -> Vec<f32> {
        self.metrics.iter().map(|m| m.composite_score).collect()
    }

    /// Report aggregate results.
    pub fn report(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("=== Batch: {} ({} runs) ===\n", self.name, self.metrics.len()));
        s.push_str(&format!("Quality Score: {:.1} +/- {:.1} (min {:.1}, max {:.1})\n",
            self.mean_quality, self.stddev_quality, self.min_quality, self.max_quality));

        // Aggregate sub-scores
        let n = self.metrics.len() as f32;
        let mean_stat = self.metrics.iter().map(|m| m.statistical_realism_score).sum::<f32>() / n;
        let mean_narr = self.metrics.iter().map(|m| m.narrative_richness_score).sum::<f32>() / n;
        let mean_behav = self.metrics.iter().map(|m| m.behavioral_coherence_score).sum::<f32>() / n;
        let mean_relig = self.metrics.iter().map(|m| m.religious_impact_score).sum::<f32>() / n;

        s.push_str(&format!("  Statistical Realism:     {:.1}\n", mean_stat));
        s.push_str(&format!("  Narrative Richness:      {:.1}\n", mean_narr));
        s.push_str(&format!("  Behavioral Coherence:    {:.1}\n", mean_behav));
        s.push_str(&format!("  Religious Impact:        {:.1}\n", mean_relig));

        // Aggregate behavioral correlations
        let mean_war_corr = self.metrics.iter().map(|m| m.war_inclination_correlation).sum::<f32>() / n;
        let mean_dip_corr = self.metrics.iter().map(|m| m.diplomacy_inclination_correlation).sum::<f32>() / n;
        let mean_build_corr = self.metrics.iter().map(|m| m.builder_inclination_correlation).sum::<f32>() / n;

        s.push_str("\n  Avg Behavioral Correlations:\n");
        s.push_str(&format!("    War inclination:   {:.3}\n", mean_war_corr));
        s.push_str(&format!("    Diplomacy incl.:   {:.3}\n", mean_dip_corr));
        s.push_str(&format!("    Builder incl.:     {:.3}\n", mean_build_corr));

        s
    }
}

/// Run a batch of N simulations, collecting metrics per run.
pub fn run_batch(
    world: &WorldData,
    config: &BatchConfig,
    game_data: &GameData,
) -> BatchResults {
    let mut all_metrics = Vec::new();

    for run in 0..config.num_runs {
        let seed = config.base_seed.wrapping_add(run as u64);
        eprintln!("  Benchmark run {}/{} (seed: {})", run + 1, config.num_runs, seed);

        let mut engine = HistoryEngine::new(seed);
        let (_history, metrics) = engine.simulate_with_metrics(
            world,
            config.history_config.clone(),
            game_data,
        );

        all_metrics.push(metrics);
    }

    let scores: Vec<f32> = all_metrics.iter().map(|m| m.composite_score).collect();
    let n = scores.len() as f32;
    let mean = scores.iter().sum::<f32>() / n;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / (n - 1.0).max(1.0);
    let stddev = variance.sqrt();
    let min = scores.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    BatchResults {
        name: config.name.clone(),
        metrics: all_metrics,
        mean_quality: mean,
        stddev_quality: stddev,
        min_quality: min,
        max_quality: max,
    }
}

/// Compare two batch results using Welch's t-test.
pub fn compare_batches(a: &BatchResults, b: &BatchResults) -> String {
    let scores_a = a.quality_scores();
    let scores_b = b.quality_scores();

    let (t, df, significant) = welch_t_test(&scores_a, &scores_b);

    let mut s = String::new();
    s.push_str("=== A/B Comparison ===\n");
    s.push_str(&format!("  A: {} (mean {:.1}, n={})\n", a.name, a.mean_quality, a.metrics.len()));
    s.push_str(&format!("  B: {} (mean {:.1}, n={})\n", b.name, b.mean_quality, b.metrics.len()));
    s.push_str(&format!("  Difference: {:.1}\n", b.mean_quality - a.mean_quality));
    s.push_str(&format!("  Welch's t: {:.3}, df: {:.1}\n", t, df));
    s.push_str(&format!("  Significant (p<0.05): {}\n", significant));
    s
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
    fn test_batch_run() {
        let world = make_test_world();
        let game_data = GameData::defaults();
        let batch_config = BatchConfig {
            name: "test".to_string(),
            history_config: HistoryConfig {
                simulation_years: 20,
                initial_civilizations: 3,
                initial_legendary_creatures: 2,
                ..HistoryConfig::default()
            },
            num_runs: 2,
            base_seed: 42,
        };

        let results = run_batch(&world, &batch_config, &game_data);
        assert_eq!(results.metrics.len(), 2);
        assert!(results.mean_quality >= 0.0);
        assert!(results.mean_quality <= 100.0);
        eprintln!("{}", results.report());
    }

    #[test]
    fn test_compare_batches() {
        let world = make_test_world();
        let game_data = GameData::defaults();

        let config_a = BatchConfig {
            name: "baseline".to_string(),
            history_config: HistoryConfig {
                simulation_years: 20,
                initial_civilizations: 3,
                initial_legendary_creatures: 2,
                ..HistoryConfig::default()
            },
            num_runs: 2,
            base_seed: 42,
        };
        let config_b = BatchConfig {
            name: "modified".to_string(),
            history_config: HistoryConfig {
                simulation_years: 20,
                initial_civilizations: 3,
                initial_legendary_creatures: 2,
                war_frequency: 2.0,
                ..HistoryConfig::default()
            },
            num_runs: 2,
            base_seed: 42,
        };

        let results_a = run_batch(&world, &config_a, &game_data);
        let results_b = run_batch(&world, &config_b, &game_data);
        let comparison = compare_batches(&results_a, &results_b);
        eprintln!("{}", comparison);
    }
}
