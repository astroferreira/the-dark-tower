//! Configuration parameters for lore generation
//!
//! Provides sensible defaults and presets for different use cases.

use serde::{Deserialize, Serialize};

/// Narrative style for generated text
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NarrativeStyle {
    Epic,       // Grand, sweeping, heroic
    Mythic,     // Ancient, symbolic, archetypal
    Folkloric,  // Homespun, practical, cautionary
    Poetic,     // Lyrical, metaphorical, beautiful
    Chronicle,  // Historical, factual, detailed
}

impl Default for NarrativeStyle {
    fn default() -> Self {
        NarrativeStyle::Mythic
    }
}

/// Configuration for lore generation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoreParams {
    // Wanderer configuration
    /// Number of wandering storytellers
    pub num_wanderers: usize,
    /// Maximum steps each wanderer takes
    pub max_steps_per_wanderer: usize,
    /// Fatigue accumulation per step (0.0-1.0)
    pub wanderer_fatigue_rate: f32,
    /// Fatigue recovery when resting in favorable biomes
    pub wanderer_recovery_rate: f32,

    // Pathfinding heuristics
    /// Randomness in direction selection (0.0-1.0)
    pub exploration_randomness: f32,
    /// Weight for attraction to geographic features
    pub feature_attraction_weight: f32,
    /// Weight for preferring unvisited biomes
    pub biome_novelty_weight: f32,
    /// Weight for avoiding recently visited tiles
    pub avoid_revisit_weight: f32,
    /// Weight for cultural terrain preferences
    pub cultural_bias_weight: f32,

    // Encounter thresholds
    /// Minimum elevation to trigger peak encounter (meters)
    pub min_elevation_for_peak: f32,
    /// Minimum stress magnitude for tectonic encounters
    pub min_stress_for_boundary: f32,
    /// Chance to trigger encounter at rare biomes (0.0-1.0)
    pub rare_biome_encounter_chance: f32,
    /// Minimum biome change significance for transition encounter
    pub min_biome_transition_significance: f32,

    // Landmark generation
    /// Radius for clustering nearby features into single landmark
    pub landmark_cluster_radius: usize,
    /// Minimum distance between landmarks
    pub min_landmark_separation: usize,

    // Story generation
    /// Range of story seeds per significant encounter (min, max)
    pub story_seeds_per_encounter: (usize, usize),
    /// Feature significance threshold for creation myths
    pub creation_myth_threshold: f32,

    // Output configuration
    /// Generate JSON output
    pub generate_json: bool,
    /// Generate narrative text output
    pub generate_narrative: bool,
    /// Include LLM prompt templates in output
    pub include_llm_prompts: bool,
    /// Style for narrative generation
    pub narrative_style: NarrativeStyle,
}

impl Default for LoreParams {
    fn default() -> Self {
        Self {
            // Wanderer configuration
            num_wanderers: 5,
            max_steps_per_wanderer: 100_000, // Long journeys across the world
            wanderer_fatigue_rate: 0.0001,   // Lower fatigue for longer journeys
            wanderer_recovery_rate: 0.1,

            // Pathfinding heuristics
            exploration_randomness: 0.3,
            feature_attraction_weight: 0.5,
            biome_novelty_weight: 0.4,
            avoid_revisit_weight: 0.6,
            cultural_bias_weight: 0.3,

            // Encounter thresholds
            min_elevation_for_peak: 2000.0,
            min_stress_for_boundary: 0.3,
            rare_biome_encounter_chance: 0.1, // Lower chance - only notable encounters
            min_biome_transition_significance: 0.7, // Higher threshold

            // Landmark generation
            landmark_cluster_radius: 5,
            min_landmark_separation: 20,

            // Story generation - much less frequent
            story_seeds_per_encounter: (0, 1), // At most 1 story seed per encounter
            creation_myth_threshold: 0.9,      // Higher threshold = fewer myths

            // Output configuration
            generate_json: true,
            generate_narrative: true,
            include_llm_prompts: true,
            narrative_style: NarrativeStyle::default(),
        }
    }
}

impl LoreParams {
    /// Minimal configuration for quick testing
    pub fn minimal() -> Self {
        Self {
            num_wanderers: 2,
            max_steps_per_wanderer: 200,
            include_llm_prompts: false,
            ..Default::default()
        }
    }

    /// Detailed configuration for rich output
    pub fn detailed() -> Self {
        Self {
            num_wanderers: 8,
            max_steps_per_wanderer: 2000,
            exploration_randomness: 0.2,
            story_seeds_per_encounter: (1, 3),
            ..Default::default()
        }
    }

    /// Configuration optimized for LLM prompt generation
    pub fn llm_focused() -> Self {
        Self {
            num_wanderers: 5,
            max_steps_per_wanderer: 1500,
            include_llm_prompts: true,
            narrative_style: NarrativeStyle::Mythic,
            creation_myth_threshold: 0.5, // Lower threshold = more myths
            ..Default::default()
        }
    }
}
