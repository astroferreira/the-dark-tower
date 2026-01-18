//! Lore and storytelling generation system
//!
//! Generates contextual narratives, histories, and named landmarks
//! based on world geomorphology, biomes, and geological history.
//!
//! # Overview
//!
//! The lore system uses wandering storyteller agents that traverse the
//! generated world, encountering geographic features and generating
//! structured "story seeds" which can be exported as JSON and used
//! as prompts for LLM-based prose generation.
//!
//! # Usage
//!
//! ```ignore
//! use planet_generator::lore::{generate_lore, LoreParams};
//! use planet_generator::world::WorldData;
//!
//! let params = LoreParams::default();
//! let mut rng = ChaCha8Rng::seed_from_u64(42);
//! let result = generate_lore(&world_data, &params, &mut rng);
//! ```

pub mod encounters;
pub mod export;
pub mod image_gen;
pub mod landmarks;
pub mod llm;
pub mod mythology;
pub mod params;
pub mod types;
pub mod wanderer;
pub mod word_banks;

pub use params::LoreParams;
pub use types::{
    Archetype, CulturalLens, Direction, EmotionalTone, Encounter, EncounterType,
    GeographicFeature, Landmark, LandmarkId, NarrativeTheme, StorySeed, StorySeedId,
    StorySeedType, SuggestedElements, Wanderer, WorldLocation,
};

// LLM and image generation exports
pub use llm::{LlmClient, LlmConfig, LlmError, RichStories, generate_rich_stories, generate_creation_poem, export_creation_poem};
pub use image_gen::{ImageGenClient, ImageGenConfig, ImageGenError, StoryImageGenerator};

use rand_chacha::ChaCha8Rng;

use crate::world::WorldData;

/// Statistics from lore generation
#[derive(Debug, Default)]
pub struct LoreStats {
    pub wanderers_created: usize,
    pub total_steps_taken: usize,
    pub landmarks_discovered: usize,
    pub story_seeds_generated: usize,
    pub encounters_processed: usize,
    pub unique_biomes_visited: usize,
}

/// Complete lore generation result
#[derive(Debug)]
pub struct LoreResult {
    pub wanderers: Vec<Wanderer>,
    pub landmarks: Vec<Landmark>,
    pub story_seeds: Vec<StorySeed>,
    pub stats: LoreStats,
}

/// Main lore generation function
///
/// Generates lore by:
/// 1. Creating wanderer agents with diverse cultural lenses
/// 2. Having each wanderer traverse the world
/// 3. Detecting encounters with geographic features
/// 4. Generating story seeds from significant encounters
/// 5. Building named landmarks from discoveries
pub fn generate_lore(
    world: &WorldData,
    params: &LoreParams,
    rng: &mut ChaCha8Rng,
) -> LoreResult {
    let mut stats = LoreStats::default();
    let mut landmark_registry = landmarks::LandmarkRegistry::new(params.min_landmark_separation);
    let mut all_story_seeds: Vec<StorySeed> = Vec::new();
    let mut story_seed_counter = 0u32;

    // Generate wanderers with diverse origins and cultural lenses
    let mut wanderers = wanderer::create_wanderers(world, params.num_wanderers, rng);
    stats.wanderers_created = wanderers.len();

    // Run each wanderer's journey
    for wanderer in &mut wanderers {
        for _step in 0..params.max_steps_per_wanderer {
            // Move wanderer
            if !wanderer::step_wanderer(wanderer, world, &landmark_registry, params, rng) {
                break; // Wanderer is stuck or exhausted
            }
            stats.total_steps_taken += 1;

            // Check for encounters
            if let Some(mut encounter) = encounters::detect_encounter(
                wanderer,
                world,
                &mut landmark_registry,
                params,
                rng,
            ) {
                // Generate story seeds from significant encounters
                if let Some(ref feature) = encounter.feature_discovered {
                    let seeds = mythology::generate_story_seeds(
                        feature,
                        &encounter.location,
                        &wanderer.cultural_lens,
                        wanderer.id,
                        &mut story_seed_counter,
                        params,
                        rng,
                    );

                    if !seeds.is_empty() {
                        encounter.story_seed_generated = Some(seeds[0].id);
                        all_story_seeds.extend(seeds);
                    }
                }

                wanderer.add_encounter(encounter);
                stats.encounters_processed += 1;
            }

            // Update fatigue
            wanderer.fatigue = (wanderer.fatigue + params.wanderer_fatigue_rate).min(1.0);

            // Check for rest locations (oases, hot springs, etc.)
            let tile = world.get_tile_info(wanderer.current_position.0, wanderer.current_position.1);
            if encounters::is_rest_location(&tile) {
                wanderer.fatigue = (wanderer.fatigue - params.wanderer_recovery_rate).max(0.0);
            }
        }
    }

    // Collect all unique biomes visited
    let mut all_biomes = std::collections::HashSet::new();
    for w in &wanderers {
        all_biomes.extend(w.visited_biomes.iter().cloned());
    }
    stats.unique_biomes_visited = all_biomes.len();

    // Finalize landmarks
    let landmarks = landmark_registry.finalize();
    stats.landmarks_discovered = landmarks.len();
    stats.story_seeds_generated = all_story_seeds.len();

    LoreResult {
        wanderers,
        landmarks,
        story_seeds: all_story_seeds,
        stats,
    }
}

/// Export lore to JSON file
pub fn export_json(result: &LoreResult, path: &str, world: &WorldData, params: &LoreParams) -> std::io::Result<()> {
    export::export_json(result, path, world, params)
}

/// Export lore as narrative text
pub fn export_narrative(result: &LoreResult, path: &str) -> std::io::Result<()> {
    export::export_narrative(result, path)
}
