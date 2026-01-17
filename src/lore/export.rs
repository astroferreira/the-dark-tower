//! Export functionality for lore data
//!
//! Generates JSON, narrative text, and LLM prompts from lore results.

use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

use crate::world::WorldData;

use super::params::LoreParams;
use super::types::{
    Landmark, StorySeed, StorySeedType, Wanderer,
};
use super::LoreResult;

/// Metadata about the lore generation
#[derive(Serialize)]
pub struct LoreMetadata {
    pub world_seed: u64,
    pub generation_params: LoreParamsSummary,
    pub wanderer_count: usize,
    pub landmark_count: usize,
    pub story_seed_count: usize,
    pub total_steps: usize,
}

/// Summary of params for serialization
#[derive(Serialize)]
pub struct LoreParamsSummary {
    pub num_wanderers: usize,
    pub max_steps: usize,
    pub narrative_style: String,
}

/// World summary for context
#[derive(Serialize)]
pub struct WorldSummary {
    pub width: usize,
    pub height: usize,
    pub scale_km_per_tile: f32,
    pub plate_count: usize,
    pub water_body_count: usize,
}

/// Relationship graph between elements
#[derive(Serialize)]
pub struct RelationshipGraph {
    pub landmark_to_story_seeds: HashMap<u32, Vec<u32>>,
    pub wanderer_discoveries: HashMap<u32, Vec<u32>>,
    pub conflicting_interpretations: Vec<ConflictingInterpretation>,
}

/// Record of conflicting cultural interpretations
#[derive(Serialize)]
pub struct ConflictingInterpretation {
    pub landmark_id: u32,
    pub landmark_name: String,
    pub interpretations: Vec<InterpretationSummary>,
}

#[derive(Serialize)]
pub struct InterpretationSummary {
    pub culture: String,
    pub perceived_name: String,
    pub role: String,
}

/// LLM prompt template
#[derive(Serialize)]
pub struct LlmPrompt {
    pub prompt_type: String,
    pub system_context: String,
    pub user_prompt: String,
    pub suggested_length: String,
    pub tone_guidance: String,
}

/// Collection of LLM prompts
#[derive(Serialize)]
pub struct LlmPrompts {
    pub creation_myth_prompts: Vec<LlmPrompt>,
    pub legend_prompts: Vec<LlmPrompt>,
    pub sacred_place_prompts: Vec<LlmPrompt>,
    pub forbidden_zone_prompts: Vec<LlmPrompt>,
    pub lost_civilization_prompts: Vec<LlmPrompt>,
    pub world_overview_prompt: LlmPrompt,
}

/// Complete export structure
#[derive(Serialize)]
pub struct LoreExport {
    pub metadata: LoreMetadata,
    pub world_summary: WorldSummary,
    pub landmarks: Vec<Landmark>,
    pub wanderers: Vec<WandererExport>,
    pub story_seeds: Vec<StorySeed>,
    pub relationships: RelationshipGraph,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_prompts: Option<LlmPrompts>,
}

/// Wanderer export with journey summary
#[derive(Serialize)]
pub struct WandererExport {
    #[serde(flatten)]
    pub wanderer: Wanderer,
    pub journey_summary: JourneySummary,
}

#[derive(Serialize)]
pub struct JourneySummary {
    pub total_distance_tiles: usize,
    pub biomes_visited: usize,
    pub landmarks_discovered: usize,
    pub encounters_had: usize,
}

/// Export lore result to JSON
pub fn export_json(
    result: &LoreResult,
    path: &str,
    world: &WorldData,
    params: &LoreParams,
) -> std::io::Result<()> {
    // Build metadata
    let metadata = LoreMetadata {
        world_seed: world.seed,
        generation_params: LoreParamsSummary {
            num_wanderers: params.num_wanderers,
            max_steps: params.max_steps_per_wanderer,
            narrative_style: format!("{:?}", params.narrative_style),
        },
        wanderer_count: result.wanderers.len(),
        landmark_count: result.landmarks.len(),
        story_seed_count: result.story_seeds.len(),
        total_steps: result.stats.total_steps_taken,
    };

    // Build world summary
    let world_summary = WorldSummary {
        width: world.width,
        height: world.height,
        scale_km_per_tile: world.scale.km_per_tile,
        plate_count: world.plates.len(),
        water_body_count: world.water_bodies.len(),
    };

    // Build relationships
    let mut landmark_to_seeds: HashMap<u32, Vec<u32>> = HashMap::new();
    for seed in &result.story_seeds {
        for landmark_id in &seed.related_landmarks {
            landmark_to_seeds
                .entry(landmark_id.0)
                .or_insert_with(Vec::new)
                .push(seed.id.0);
        }
    }

    let mut wanderer_discoveries: HashMap<u32, Vec<u32>> = HashMap::new();
    for wanderer in &result.wanderers {
        wanderer_discoveries.insert(
            wanderer.id,
            wanderer.discovered_landmarks.iter().map(|l| l.0).collect(),
        );
    }

    // Find conflicting interpretations
    let mut conflicting = Vec::new();
    for landmark in &result.landmarks {
        if landmark.interpretations.len() > 1 {
            let interpretations: Vec<InterpretationSummary> = landmark
                .interpretations
                .iter()
                .map(|i| InterpretationSummary {
                    culture: i.cultural_lens_type.clone(),
                    perceived_name: i.perceived_name.clone(),
                    role: i.mythological_role.clone(),
                })
                .collect();

            conflicting.push(ConflictingInterpretation {
                landmark_id: landmark.id.0,
                landmark_name: landmark.name.clone(),
                interpretations,
            });
        }
    }

    let relationships = RelationshipGraph {
        landmark_to_story_seeds: landmark_to_seeds,
        wanderer_discoveries,
        conflicting_interpretations: conflicting,
    };

    // Build wanderer exports
    let wanderers: Vec<WandererExport> = result
        .wanderers
        .iter()
        .map(|w| WandererExport {
            wanderer: w.clone(),
            journey_summary: JourneySummary {
                total_distance_tiles: w.path_history.len(),
                biomes_visited: w.visited_biomes.len(),
                landmarks_discovered: w.discovered_landmarks.len(),
                encounters_had: w.encounters.len(),
            },
        })
        .collect();

    // Build LLM prompts if requested
    let llm_prompts = if params.include_llm_prompts {
        Some(generate_llm_prompts(result, world))
    } else {
        None
    };

    let export = LoreExport {
        metadata,
        world_summary,
        landmarks: result.landmarks.clone(),
        wanderers,
        story_seeds: result.story_seeds.clone(),
        relationships,
        llm_prompts,
    };

    // Write to file
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &export)?;

    Ok(())
}

/// Generate LLM prompts for story generation
fn generate_llm_prompts(result: &LoreResult, world: &WorldData) -> LlmPrompts {
    let system_base = format!(
        r#"You are a mythologist and storyteller creating origin stories for a procedurally generated fantasy world.

World Context:
- Map size: {}x{} tiles ({:.0}km x {:.0}km)
- Tectonic plates: {} (creating mountains, rifts, and volcanic activity)
- Climate varies from frozen poles to tropical equator
- Ancient and mystical places dot the landscape
- Multiple cultures interpret the same features differently

Your task is to transform geographic features into mythological narratives that feel authentic to the culture telling them. Write in a style appropriate for oral tradition - stories told around fires, passed down through generations."#,
        world.width,
        world.height,
        world.width as f32 * world.scale.km_per_tile,
        world.height as f32 * world.scale.km_per_tile,
        world.plates.len()
    );

    let mut creation_myths = Vec::new();
    let mut legends = Vec::new();
    let mut sacred_places = Vec::new();
    let mut forbidden_zones = Vec::new();
    let mut lost_civs = Vec::new();

    for seed in &result.story_seeds {
        let wanderer = result.wanderers.iter().find(|w| seed.source_wanderers.contains(&w.id));
        let culture_name = wanderer
            .map(|w| w.cultural_lens.culture_name())
            .unwrap_or("Unknown");

        let themes_str = seed
            .themes
            .iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>()
            .join(", ");

        let archetypes_str = seed
            .archetypes
            .iter()
            .map(|a| format!("{:?}", a))
            .collect::<Vec<_>>()
            .join(", ");

        let elements_str = format!(
            "Deities: {}\nCreatures: {}\nArtifacts: {}\nRituals: {}\nTaboos: {}",
            seed.suggested_elements.deity_names.join(", "),
            seed.suggested_elements.creature_types.join(", "),
            seed.suggested_elements.artifact_types.join(", "),
            seed.suggested_elements.ritual_types.join(", "),
            seed.suggested_elements.taboos.join(", ")
        );

        let location_context = format!(
            "Location: {:.0}km east, {:.0}km south\nElevation: {:.0}m\nTemperature: {:.1}°C\nBiome: {}",
            seed.primary_location.km_x,
            seed.primary_location.km_y,
            seed.primary_location.elevation,
            seed.primary_location.temperature,
            seed.primary_location.biome
        );

        match &seed.seed_type {
            StorySeedType::CreationMyth { origin_feature, cosmic_scale } => {
                let prompt = LlmPrompt {
                    prompt_type: "creation_myth".to_string(),
                    system_context: format!(
                        "{}\n\nCultural Perspective: {}\nThis culture values: {:?}",
                        system_base,
                        culture_name,
                        wanderer.map(|w| w.cultural_lens.values()).unwrap_or_default()
                    ),
                    user_prompt: format!(
                        r#"Write a creation myth explaining how {} came to be.

{}

Themes to incorporate: {}
Archetypes to include: {}
Cosmic scale: {:?}

Suggested Elements:
{}

The myth should:
1. Explain why this place exists
2. Give it cosmic/spiritual significance
3. Include at least one deity or primordial being
4. Reference the {}'s cultural values
5. End with why this place is sacred today

Write 400-600 words in an epic, mythic style."#,
                        origin_feature.description(),
                        location_context,
                        themes_str,
                        archetypes_str,
                        cosmic_scale,
                        elements_str,
                        culture_name
                    ),
                    suggested_length: "400-600 words".to_string(),
                    tone_guidance: seed.emotional_tone.to_prompt_guidance().to_string(),
                };
                creation_myths.push(prompt);
            }

            StorySeedType::HeroLegend { journey_type, trial_features } => {
                let prompt = LlmPrompt {
                    prompt_type: "hero_legend".to_string(),
                    system_context: system_base.clone(),
                    user_prompt: format!(
                        r#"Write a hero legend about a {:?} journey.

{}

Themes: {}
Archetypes: {}
Trials faced: {}

Suggested Elements:
{}

The legend should feature a hero from the {} who must overcome trials. Include moments of doubt, revelation, and transformation.

Write 500-800 words."#,
                        journey_type,
                        location_context,
                        themes_str,
                        archetypes_str,
                        trial_features.join(", "),
                        elements_str,
                        culture_name
                    ),
                    suggested_length: "500-800 words".to_string(),
                    tone_guidance: seed.emotional_tone.to_prompt_guidance().to_string(),
                };
                legends.push(prompt);
            }

            StorySeedType::SacredPlace { sanctity_source, pilgrimage_worthy } => {
                let prompt = LlmPrompt {
                    prompt_type: "sacred_place".to_string(),
                    system_context: system_base.clone(),
                    user_prompt: format!(
                        r#"Describe a sacred place and its significance.

{}

Source of sanctity: {:?}
Pilgrimage worthy: {}

Themes: {}

Suggested Elements:
{}

Write about:
1. How this place was discovered/sanctified
2. What rituals are performed here
3. What pilgrims/visitors experience
4. Any taboos or protocols

Write 300-500 words with a reverent tone."#,
                        location_context,
                        sanctity_source,
                        if *pilgrimage_worthy { "Yes" } else { "No" },
                        themes_str,
                        elements_str
                    ),
                    suggested_length: "300-500 words".to_string(),
                    tone_guidance: "Reverent, solemn, spiritual depth".to_string(),
                };
                sacred_places.push(prompt);
            }

            StorySeedType::ForbiddenZone { danger_type, warning_signs } => {
                let prompt = LlmPrompt {
                    prompt_type: "forbidden_zone".to_string(),
                    system_context: system_base.clone(),
                    user_prompt: format!(
                        r#"Write about a forbidden place that all know to avoid.

{}

Type of danger: {:?}
Warning signs: {}

Themes: {}

Suggested Elements:
{}

Include:
1. Why this place is forbidden
2. Stories of those who entered
3. Warning signs travelers learn to recognize
4. What the {} believe lurks there

Write 300-500 words with an ominous, cautionary tone."#,
                        location_context,
                        danger_type,
                        warning_signs.join(", "),
                        themes_str,
                        elements_str,
                        culture_name
                    ),
                    suggested_length: "300-500 words".to_string(),
                    tone_guidance: "Ominous, foreboding, cautionary".to_string(),
                };
                forbidden_zones.push(prompt);
            }

            StorySeedType::LostCivilization { ruin_biome, fall_cause } => {
                let prompt = LlmPrompt {
                    prompt_type: "lost_civilization".to_string(),
                    system_context: system_base.clone(),
                    user_prompt: format!(
                        r#"Write the legend of a lost civilization.

{}

Ruin type: {}
Cause of fall: {:?}

Themes: {}

Suggested Elements:
{}

Include:
1. The glory of what once was
2. The hubris or tragedy that led to their fall
3. What remains and what is lost
4. Rumors of treasures or dangers
5. Why some seek these ruins despite the risks

Write 400-600 words with a melancholic, mysterious tone."#,
                        location_context,
                        ruin_biome,
                        fall_cause,
                        themes_str,
                        elements_str
                    ),
                    suggested_length: "400-600 words".to_string(),
                    tone_guidance: "Melancholic, mysterious, hint of danger".to_string(),
                };
                lost_civs.push(prompt);
            }

            _ => {}
        }
    }

    // World overview prompt
    let landmark_summary: Vec<String> = result
        .landmarks
        .iter()
        .take(10)
        .map(|l| format!("- {} ({})", l.name, l.feature_type.description()))
        .collect();

    let world_overview = LlmPrompt {
        prompt_type: "world_overview".to_string(),
        system_context: system_base.clone(),
        user_prompt: format!(
            r#"Create a world overview that weaves together the mythology of this land.

Major landmarks discovered:
{}

Number of wandering storytellers: {}
Total story seeds generated: {}

Write a 500-800 word overview that:
1. Describes the world's cosmological origins
2. References major landmarks and their significance
3. Hints at the conflicts between different cultural interpretations
4. Creates a sense of deep history and mystery
5. Leaves hooks for further exploration

This should feel like the opening chapter of a world bible or fantasy encyclopedia."#,
            landmark_summary.join("\n"),
            result.wanderers.len(),
            result.story_seeds.len()
        ),
        suggested_length: "500-800 words".to_string(),
        tone_guidance: "Epic, comprehensive, inviting further exploration".to_string(),
    };

    LlmPrompts {
        creation_myth_prompts: creation_myths,
        legend_prompts: legends,
        sacred_place_prompts: sacred_places,
        forbidden_zone_prompts: forbidden_zones,
        lost_civilization_prompts: lost_civs,
        world_overview_prompt: world_overview,
    }
}

/// Export lore as narrative text - rich fantasy prose for each wanderer
pub fn export_narrative(result: &LoreResult, path: &str) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Epic opening
    writeln!(writer, "# The Chronicles of the Wandering Sages\n")?;
    writeln!(writer, "*Being an account of those who walked the world in the age before kingdoms,*")?;
    writeln!(writer, "*who mapped the sacred places and gave names to the nameless,*")?;
    writeln!(writer, "*whose tales became the foundation of all that would follow.*\n")?;
    writeln!(writer, "In the time when the world was young and the boundaries between realms grew thin, \
        {} souls set forth from their homelands to walk the paths between places of power. \
        They were the first cartographers of the sacred, the namers of mountains and the witnesses \
        of wonders. These are their tales.\n", result.wanderers.len())?;
    writeln!(writer, "---\n")?;

    // Generate rich narrative for each wanderer
    for (i, wanderer) in result.wanderers.iter().enumerate() {
        write_rich_narrative(&mut writer, wanderer, result, i)?;
        writeln!(writer, "\n---\n")?;
    }

    // Appendix of sacred places
    writeln!(writer, "# Appendix: The Named Places\n")?;
    writeln!(writer, "*These are the landmarks given true names by the wanderers, \
        places where the veil between worlds grows thin and the old powers still linger.*\n")?;

    for landmark in result.landmarks.iter().take(50) {
        writeln!(writer, "**{}** — Position ({}, {}), Elevation {:.0}m",
            landmark.name,
            landmark.primary_location.x,
            landmark.primary_location.y,
            landmark.primary_location.elevation
        )?;
        writeln!(writer, "   {}\n", landmark_description(&landmark.feature_type))?;
    }
    if result.landmarks.len() > 50 {
        writeln!(writer, "*...and {} more sacred places await discovery.*\n",
            result.landmarks.len() - 50)?;
    }

    Ok(())
}

/// Write a rich fantasy narrative for a wanderer
fn write_rich_narrative<W: Write>(
    writer: &mut W,
    wanderer: &Wanderer,
    result: &LoreResult,
    wanderer_index: usize,
) -> std::io::Result<()> {
    let culture = wanderer.cultural_lens.culture_name();
    let title = wanderer_title(&wanderer.cultural_lens);

    // Book header
    writeln!(writer, "# Book {}: The Saga of {}\n",
        roman_numeral(wanderer_index + 1), wanderer.name)?;
    writeln!(writer, "*{} of the {}, Walker of {} Realms*\n",
        title, culture, wanderer.visited_biomes.len())?;

    // Prologue - cultural background and calling
    writeln!(writer, "## Prologue: The Calling\n")?;
    write_prologue(writer, wanderer)?;

    // Part I - The departure and early journey
    writeln!(writer, "\n## Part I: The Departure\n")?;
    write_departure(writer, wanderer)?;

    // Get significant encounters (not biome transitions)
    let significant_encounters: Vec<_> = wanderer.encounters
        .iter()
        .filter(|e| {
            if let Some(ref feature) = e.feature_discovered {
                !matches!(feature, super::types::GeographicFeature::BiomeTransition { .. })
            } else {
                false
            }
        })
        .collect();

    // Part II - The journey chapters
    if !significant_encounters.is_empty() {
        writeln!(writer, "\n## Part II: The Wandering\n")?;

        // Categorize encounters by feature type for variety
        let mut by_type: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
        for enc in &significant_encounters {
            if let Some(ref feature) = enc.feature_discovered {
                let key = feature_type_category(feature);
                by_type.entry(key).or_default().push(enc);
            }
        }

        // Select diverse encounters: aim for max 2-3 per type, up to 15 total
        let max_chapters = 15;
        let max_per_type = 3;
        let mut selected: Vec<_> = Vec::new();

        // First pass: take at most max_per_type from each category, spread across journey
        for (_type_name, encounters) in &by_type {
            let count = encounters.len().min(max_per_type);
            let step = (encounters.len() / count).max(1);
            for (i, enc) in encounters.iter().enumerate() {
                if i % step == 0 && selected.len() < max_chapters {
                    selected.push(*enc);
                }
                if selected.iter().filter(|e| {
                    if let (Some(f1), Some(f2)) = (&e.feature_discovered, &enc.feature_discovered) {
                        feature_type_category(f1) == feature_type_category(f2)
                    } else {
                        false
                    }
                }).count() >= max_per_type {
                    break;
                }
            }
        }

        // Sort by step number to maintain journey chronology
        selected.sort_by_key(|e| e.step_number);

        // Limit to max chapters
        selected.truncate(max_chapters);

        let mut chapter = 0;
        for encounter in selected {
            chapter += 1;
            if let Some(ref feature) = encounter.feature_discovered {
                write_encounter_chapter(writer, wanderer, encounter, feature, chapter)?;
            }
        }
    }

    // Part III - Key discoveries
    if !wanderer.discovered_landmarks.is_empty() {
        writeln!(writer, "\n## Part III: The Sacred Discoveries\n")?;
        write_discoveries(writer, wanderer, result)?;
    }

    // Epilogue - Return and legacy
    writeln!(writer, "\n## Epilogue: The Return\n")?;
    write_epilogue(writer, wanderer)?;

    Ok(())
}

fn write_prologue<W: Write>(writer: &mut W, wanderer: &Wanderer) -> std::io::Result<()> {
    let culture = wanderer.cultural_lens.culture_name();
    let cultural_beliefs = cultural_worldview(&wanderer.cultural_lens);

    writeln!(writer, "Among the {} people, it was said that {}\n", culture, cultural_beliefs)?;

    writeln!(writer, "In the village at ({}, {}), nestled within the {}, there lived one called {}. \
        From childhood, {} had dreamed of distant horizons—dreams that the elders recognized as \
        the mark of a true seeker. When the signs aligned and the omens spoke clearly, \
        the village gathered to perform the ancient rites of departure.\n",
        wanderer.origin.x, wanderer.origin.y,
        biome_poetic_description(&wanderer.origin.biome),
        wanderer.name, wanderer.name
    )?;

    writeln!(writer, "\"Go forth,\" spoke the eldest, \"and return to us with the names of \
        places where gods once walked. Find the boundaries of our world and learn what lies beyond. \
        May the {} guide your steps.\"\n", cultural_deity(&wanderer.cultural_lens))?;

    Ok(())
}

fn write_departure<W: Write>(writer: &mut W, wanderer: &Wanderer) -> std::io::Result<()> {
    let departure_prose = departure_description(&wanderer.cultural_lens);

    writeln!(writer, "{}\n", departure_prose)?;

    writeln!(writer, "And so {} set forth from ({}, {}) on what would become a journey of \
        {} steps across {} distinct realms of existence. The path ahead held {} moments \
        of profound discovery—encounters with places where the fabric of reality wore thin \
        and the ancient powers still stirred.\n",
        wanderer.name,
        wanderer.origin.x, wanderer.origin.y,
        wanderer.steps_taken,
        wanderer.visited_biomes.len(),
        wanderer.encounters.len()
    )?;

    Ok(())
}

fn write_encounter_chapter<W: Write>(
    writer: &mut W,
    wanderer: &Wanderer,
    encounter: &super::types::Encounter,
    feature: &super::types::GeographicFeature,
    chapter_num: usize,
) -> std::io::Result<()> {
    let chapter_title = encounter_chapter_title(feature, chapter_num);

    writeln!(writer, "### Chapter {}: {}\n", chapter_num, chapter_title)?;

    // Location and atmosphere
    let atmosphere = biome_atmosphere(&encounter.location.biome);
    writeln!(writer, "At coordinates ({}, {}), where the land {} at {:.0} meters above the \
        distant sea, {} came upon {}.\n",
        encounter.location.x, encounter.location.y,
        elevation_description(encounter.location.elevation),
        encounter.location.elevation,
        wanderer.name,
        feature_discovery_prose(feature)
    )?;

    // The encounter itself
    writeln!(writer, "{}\n", atmosphere)?;

    // Cultural interpretation
    let interpretation = cultural_interpretation(feature, &wanderer.cultural_lens);
    writeln!(writer, "{}\n", interpretation)?;

    // Mark the location
    writeln!(writer, "{} marked this place in memory: *{} at ({}, {}), elevation {:.0}m, \
        within the realm of {}.*\n",
        wanderer.name,
        encounter_short_name(feature),
        encounter.location.x, encounter.location.y,
        encounter.location.elevation,
        encounter.location.biome
    )?;

    Ok(())
}

fn write_discoveries<W: Write>(
    writer: &mut W,
    wanderer: &Wanderer,
    result: &LoreResult,
) -> std::io::Result<()> {
    writeln!(writer, "Throughout the long wandering, {} bestowed true names upon {} places \
        of power—sites where the boundary between the mortal realm and the otherworld \
        had worn gossamer-thin. These were not mere geographic features but living nexuses \
        of ancient energy, and to name them was to claim a small measure of their power.\n",
        wanderer.name, wanderer.discovered_landmarks.len()
    )?;

    writeln!(writer, "The most significant of these sacred discoveries:\n")?;

    for landmark_id in wanderer.discovered_landmarks.iter().take(10) {
        if let Some(landmark) = result.landmarks.iter().find(|l| l.id == *landmark_id) {
            writeln!(writer, "**{}** — At ({}, {}), {:.0}m elevation",
                landmark.name,
                landmark.primary_location.x,
                landmark.primary_location.y,
                landmark.primary_location.elevation
            )?;
            writeln!(writer, "   {}\n", landmark_description(&landmark.feature_type))?;
        }
    }

    if wanderer.discovered_landmarks.len() > 10 {
        writeln!(writer, "*Plus {} additional sites of lesser—but still considerable—significance, \
            each carefully recorded in the wanderer's memory.*\n",
            wanderer.discovered_landmarks.len() - 10)?;
    }

    Ok(())
}

fn write_epilogue<W: Write>(writer: &mut W, wanderer: &Wanderer) -> std::io::Result<()> {
    let culture = wanderer.cultural_lens.culture_name();

    writeln!(writer, "After {} steps through {} realms, {} turned at last toward home. \
        The journey had taken its toll—lines of wisdom now marked a face that had been young \
        at departure, and eyes that had once held only curiosity now carried the weight of \
        wonders witnessed.\n",
        wanderer.steps_taken,
        wanderer.visited_biomes.len(),
        wanderer.name
    )?;

    writeln!(writer, "When {} finally returned to the {} at ({}, {}), the entire village \
        gathered to hear the tales. For seven days and seven nights, the wanderer spoke of \
        {} encounters with the otherworldly, of {} named places where gods once walked, \
        of realms where {} and {} stood side by side.\n",
        wanderer.name, culture,
        wanderer.origin.x, wanderer.origin.y,
        wanderer.encounters.len(),
        wanderer.discovered_landmarks.len(),
        sample_biomes(&wanderer.visited_biomes, 0),
        sample_biomes(&wanderer.visited_biomes, 1)
    )?;

    writeln!(writer, "These tales became the sacred texts of the {} people—the foundation \
        of their understanding of the world beyond their borders. And though {} would \
        eventually pass into memory, the names given to those distant places of power \
        would endure for generations, guiding future seekers along paths first walked \
        in the age of wandering.\n",
        culture, wanderer.name
    )?;

    writeln!(writer, "*Thus ends the saga of {}, {} of the {}, \
        who walked {} steps and witnessed {} wonders.*",
        wanderer.name,
        wanderer_title(&wanderer.cultural_lens),
        culture,
        wanderer.steps_taken,
        wanderer.encounters.len()
    )?;

    Ok(())
}

// Helper functions for rich prose generation

fn roman_numeral(n: usize) -> &'static str {
    match n {
        1 => "I", 2 => "II", 3 => "III", 4 => "IV", 5 => "V",
        6 => "VI", 7 => "VII", 8 => "VIII", 9 => "IX", 10 => "X",
        _ => "XI"
    }
}

fn wanderer_title(lens: &super::types::CulturalLens) -> &'static str {
    match lens {
        super::types::CulturalLens::Highland { .. } => "Sage of the High Places",
        super::types::CulturalLens::Maritime { .. } => "Walker of Tides",
        super::types::CulturalLens::Desert { .. } => "Seeker Beneath Stars",
        super::types::CulturalLens::Sylvan { .. } => "Keeper of Green Secrets",
        super::types::CulturalLens::Steppe { .. } => "Rider of Wind",
        super::types::CulturalLens::Subterranean { .. } => "Delver of Depths",
    }
}

fn cultural_worldview(lens: &super::types::CulturalLens) -> &'static str {
    match lens {
        super::types::CulturalLens::Highland { ancestor_worship: true, .. } =>
            "the mountains were the bones of ancient titans, and atop each peak dwelt the spirits of ancestors \
            who had earned their place close to the sky. To climb was to commune with the dead; to descend was \
            to carry their wisdom back to the living",
        super::types::CulturalLens::Highland { .. } =>
            "stone endures what flesh cannot. The mountains taught patience, and patience was the greatest \
            of all virtues—for what is a human lifetime against the slow dreaming of granite?",
        super::types::CulturalLens::Maritime { .. } =>
            "all life came from the sea, and to the sea all things must return. The tides were the breathing \
            of the world-serpent who coiled beneath the waves, and those who learned to read the waters \
            could glimpse the future in foam and current",
        super::types::CulturalLens::Desert { follows_stars: true, .. } =>
            "the stars were the eyes of gods watching from beyond the veil. Each constellation told a story \
            of creation and destruction, and those who could read the celestial script would never lose their way—\
            not even in the trackless wastes where sand swallowed all other signs",
        super::types::CulturalLens::Desert { .. } =>
            "water was sacred, for it was the blood of the earth. Every oasis was a temple, every well a shrine, \
            and those who wasted water committed an unforgivable sin against the world itself",
        super::types::CulturalLens::Sylvan { tree_worship: true, .. } =>
            "trees were the eldest of all living things, their roots drinking from the underworld while their \
            branches touched the realm of sky-spirits. In the heartwood of the oldest oaks lived memories \
            of the world's first dawn",
        super::types::CulturalLens::Sylvan { .. } =>
            "the forest was a single dreaming mind, each tree a thought, each creature a fleeting fancy. \
            To walk among the trees was to traverse the consciousness of something vast and ancient",
        super::types::CulturalLens::Steppe { sky_worship: true, .. } =>
            "the sky was the great tent of the Eternal Blue, stretched above the world by the First Rider. \
            Beneath it, all creatures were equal—bound only by the horizon and freed by the wind",
        super::types::CulturalLens::Steppe { .. } =>
            "movement was life, and stillness was death. The grasses bent before the wind, the herds migrated \
            with the seasons, and a people who did not wander would wither like cut flowers",
        super::types::CulturalLens::Subterranean { crystal_worship: true, .. } =>
            "crystals were frozen light, the dreams of stone made manifest. Deep beneath the earth, in caverns \
            no sunlight had ever touched, grew gardens of gemstone where the thoughts of the sleeping earth-god \
            took physical form",
        super::types::CulturalLens::Subterranean { .. } =>
            "the surface world was an illusion, a thin skin over the true reality below. In the depths, \
            where darkness was absolute, one learned to see with other senses—and discovered that \
            the world was far stranger than surface-dwellers ever imagined",
    }
}

fn cultural_deity(lens: &super::types::CulturalLens) -> &'static str {
    match lens {
        super::types::CulturalLens::Highland { .. } => "Ancestors of Stone",
        super::types::CulturalLens::Maritime { sea_deity_name, .. } => {
            // Can't easily return the String, so use a default
            "Great Tide-Keeper"
        }
        super::types::CulturalLens::Desert { .. } => "Eternal Stars",
        super::types::CulturalLens::Sylvan { .. } => "Green Mother of Roots",
        super::types::CulturalLens::Steppe { .. } => "Eternal Blue Sky",
        super::types::CulturalLens::Subterranean { .. } => "Crystal Heart of the Deep",
    }
}

fn departure_description(lens: &super::types::CulturalLens) -> &'static str {
    match lens {
        super::types::CulturalLens::Highland { .. } =>
            "The departure ritual lasted three days. On the first, the seeker fasted and meditated \
            upon the highest accessible peak. On the second, the elders marked the seeker's skin \
            with sacred ash—symbols of protection against the spirits of foreign places. On the third, \
            as dawn broke golden over the eastern ridges, the seeker walked down from the mountain \
            and did not look back, for to look back was to invite the ancestors to call one home too soon.",
        super::types::CulturalLens::Maritime { .. } =>
            "The tide-priests read the currents and declared the moment auspicious. A small boat \
            carried the seeker to the edge of known waters, where the great kelp forests gave way \
            to open sea. There, standing in the shallows where waves met shore, the seeker was anointed \
            with salt water and blessed with words older than memory. Then the boat departed, \
            and the seeker walked alone onto the foreign land.",
        super::types::CulturalLens::Desert { .. } =>
            "On the night of departure, the stars aligned in the ancient pattern called the Seeker's Road. \
            The village astrologers had waited three years for this configuration. Water was blessed \
            under starlight, sand was gathered from seven sacred dunes, and the seeker was given \
            a fragment of meteoric iron—a piece of the sky itself—to carry as protection against \
            the demons that dwelt in waterless places.",
        super::types::CulturalLens::Sylvan { .. } =>
            "The oldest tree in the grove—a sentinel that had stood since before the first human \
            entered the forest—was asked for its blessing. Its answer came in the rustle of leaves, \
            interpreted by the tree-speakers. The seeker was given a living branch, cut with prayers \
            and bound with spider-silk, to plant in distant soil and thus extend the forest's dreaming \
            into unknown lands.",
        super::types::CulturalLens::Steppe { .. } =>
            "The entire tribe gathered to witness the departure. A white horse was consecrated to \
            the Eternal Sky, and its spirit was bound to the seeker through the ritual of shared breath. \
            Though the seeker would walk rather than ride—for the horse's spirit traveled on the wind—\
            its strength would lend endurance for the journey ahead. The tribe's song followed \
            the departing figure until distance swallowed all sound.",
        super::types::CulturalLens::Subterranean { .. } =>
            "The departure began with descent—three days of meditation in the deepest accessible cavern, \
            where no light had touched since the world's making. There, in absolute darkness, \
            the seeker learned to navigate by echo and intuition. Only then, reborn from stone's womb, \
            could they emerge to walk the blinding surface world. They carried a crystal that had \
            never seen light, wrapped in black cloth, to be uncovered only in moments of greatest need.",
    }
}

fn biome_poetic_description(biome: &str) -> String {
    if biome.contains("Forest") || biome.contains("Grove") {
        "realm of ancient trees where sunlight fell in cathedral shafts through canopies older than memory".to_string()
    } else if biome.contains("Desert") || biome.contains("Dune") {
        "sea of sand where the wind sculpted dunes into waves frozen in amber light".to_string()
    } else if biome.contains("Mountain") || biome.contains("Alpine") {
        "kingdom of stone and snow where peaks pierced the belly of clouds".to_string()
    } else if biome.contains("Tundra") || biome.contains("Ice") || biome.contains("Frost") {
        "frozen realm where the world's edge seemed close enough to touch".to_string()
    } else if biome.contains("Grass") || biome.contains("Plain") || biome.contains("Savanna") {
        "endless expanse of wind-rippled grass stretching to every horizon".to_string()
    } else if biome.contains("Swamp") || biome.contains("Marsh") || biome.contains("Fen") {
        "twilight realm of mist and still water where solid ground was never certain".to_string()
    } else if biome.contains("Jungle") || biome.contains("Rain") || biome.contains("Tropical") {
        "verdant labyrinth of life layered upon life, where the air itself was thick with growing things".to_string()
    } else if biome.contains("Void") || biome.contains("Ethereal") || biome.contains("Starfall") {
        "place where reality wore thin and the light of other worlds leaked through".to_string()
    } else if biome.contains("Titan") || biome.contains("Colossal") || biome.contains("Cyclopean") {
        "land of impossible ruins where beings of unthinkable scale once walked".to_string()
    } else {
        format!("lands known as {}", biome)
    }
}

fn biome_atmosphere(biome: &str) -> String {
    if biome.contains("Aurora") || biome.contains("Starfall") {
        "The sky here danced with impossible colors—curtains of light that moved like living things, \
        casting shadows that seemed to whisper secrets in languages older than speech. The air itself \
        hummed with potential, as if the boundary between what was and what could be had worn thin enough \
        to step through.".to_string()
    } else if biome.contains("Titan") || biome.contains("Bone") {
        "Massive structures—or were they bones?—jutted from the earth like the fingers of buried giants. \
        Each one dwarfed the mightiest trees, their surfaces worn by countless ages but still bearing \
        faint traces of patterns that might have been veins, might have been script.".to_string()
    } else if biome.contains("Void") || biome.contains("Scar") {
        "Here the world simply... stopped. Where land should have continued, there was instead an absence—\
        not darkness, for darkness is merely the lack of light, but something more fundamental. \
        Looking too long at the edge brought vertigo and whispered voices.".to_string()
    } else if biome.contains("Crystal") || biome.contains("Silicon") {
        "Crystalline formations rose like frozen lightning, their facets catching light that seemed to \
        come from within rather than without. Colors shifted as one moved, and occasionally the structures \
        chimed with notes too pure for natural stone.".to_string()
    } else if biome.contains("Ruin") || biome.contains("Citadel") || biome.contains("Temple") {
        "Ancient walls rose from creeping vegetation—or perhaps the vegetation had always been part of \
        the design. Architecture followed geometries that hurt to contemplate, suggesting builders who \
        thought in more dimensions than three.".to_string()
    } else if biome.contains("Mushroom") || biome.contains("Fungal") {
        "Towering fungi replaced trees here, their caps spanning distances that defied belief. \
        Bioluminescent veins pulsed with slow rhythm, and the air carried spores that sparkled like \
        living stars. The silence was absolute—even footsteps made no sound.".to_string()
    } else if biome.contains("Geyser") || biome.contains("HotSpring") {
        "Steam rose from countless vents, creating a perpetual mist that transformed the landscape into \
        a fever-dream of half-seen shapes. The earth's breath was warm and sulfurous, carrying hints of \
        the molten heart that beat somewhere far below.".to_string()
    } else if biome.contains("Water") || biome.contains("Ocean") || biome.contains("Sea") {
        "The waters stretched endlessly, dark depths hiding secrets that predated memory—\
        currents carrying whispers from the world's beginning, surfaces reflecting skies that \
        seemed somehow closer to the divine.".to_string()
    } else if biome.contains("Volcanic") || biome.contains("Lava") {
        "The earth here bore fresh wounds—rivers of molten stone that had cooled into \
        twisted sculptures, vents that still breathed sulfurous fumes, and rock that remembered \
        being liquid in ages past.".to_string()
    } else if biome.contains("Coral") || biome.contains("Reef") {
        "Living stone rose in impossible architectures—corals that had grown for millennia, \
        forming cities for creatures that knew nothing of the surface world. Colors existed \
        here that had no names in any human tongue.".to_string()
    } else if biome.contains("Kelp") || biome.contains("Seagrass") {
        "Forests grew beneath the waves—great fronds reaching toward distant light, swaying \
        in currents that were the ocean's breath. Fish moved like birds through these \
        submerged canopies, and strange creatures dwelt in the twilight below.".to_string()
    } else if biome.contains("Oasis") {
        "Life erupted improbably from the barren waste—palms casting precious shade, water \
        clear as prophecy pooling in basins of ancient stone. This was holy ground, \
        unmistakably, a place where the gods had paused to rest.".to_string()
    } else if biome.contains("Canyon") || biome.contains("Gorge") {
        "Walls of striated stone rose on either side, each layer a chapter in a book \
        written over countless millennia. The sky was a distant ribbon overhead, and \
        echoes multiplied any sound into ghostly chorus.".to_string()
    } else {
        format!("The {} realm spread before the wanderer's eyes—a land with its own ancient \
        character, its own spirits that watched from every shadow and whispered in every breeze. \
        This was a place that remembered the world's youth.", biome)
    }
}

fn elevation_description(elevation: f32) -> &'static str {
    if elevation > 1000.0 {
        "soared into thin air"
    } else if elevation > 500.0 {
        "rose toward the clouds"
    } else if elevation > 100.0 {
        "lifted gently above the surrounding terrain"
    } else if elevation > 0.0 {
        "rested upon solid ground"
    } else if elevation > -50.0 {
        "dipped slightly below the waterline"
    } else {
        "descended into submerged depths"
    }
}

fn feature_discovery_prose(feature: &super::types::GeographicFeature) -> String {
    match feature {
        super::types::GeographicFeature::MountainPeak { height, is_volcanic } => {
            if *is_volcanic {
                format!("a smoking peak that rose {:.0}m into the heavens, its summit wreathed in \
                    ash-clouds and its flanks scarred by ancient flows of molten stone", height)
            } else {
                format!("a mountain that pierced the sky at {:.0}m, its peak a throne of ice and \
                    stone where eagles feared to fly and only the wind dared linger", height)
            }
        }
        super::types::GeographicFeature::Volcano { active } => {
            if *active {
                "a living mountain whose heart still burned with the fire of creation—smoke rose \
                from its caldera like the breath of a sleeping dragon, and the ground trembled \
                with the rhythm of something vast and molten".to_string()
            } else {
                "the remains of a great volcano, now silent but not dead—merely sleeping, \
                its fire withdrawn deep into the earth, waiting for the age when it would \
                wake again".to_string()
            }
        }
        super::types::GeographicFeature::Lake { area, depth } => {
            format!("waters of impossible stillness stretching across {} measures, \
                depths plunging {:.0}m where light never reached and creatures \
                of pure darkness made their home", area, depth)
        }
        super::types::GeographicFeature::Valley { depth, river_carved } => {
            if *river_carved {
                format!("a river-carved vale {:.0}m deep, where ancient waters had \
                    written their history in stone over countless ages", depth)
            } else {
                format!("a hidden valley {:.0}m deep, cradled between walls of stone \
                    like a secret the mountains kept from the sky", depth)
            }
        }
        super::types::GeographicFeature::Coast => {
            "the edge of all known lands, where solid earth surrendered to the endless \
            hunger of the sea—the boundary between the world of humans and the realm \
            of deeper things".to_string()
        }
        super::types::GeographicFeature::PlateBoundary { convergent, .. } => {
            if *convergent {
                "a place where the bones of the world ground against each other—\
                two landmasses in eternal collision, their meeting marked by \
                tortured stone and earth that would not rest".to_string()
            } else {
                "a great rift where the world was slowly tearing itself apart—\
                the wound of some primordial cataclysm that had never healed, \
                and perhaps never would".to_string()
            }
        }
        super::types::GeographicFeature::AncientSite { biome } => {
            format!("an ancient site of {} origin—ruins or relics or perhaps something \
                that had never been built at all, but had simply always existed, \
                waiting for the right eyes to find it", biome)
        }
        super::types::GeographicFeature::MysticalAnomaly { biome } => {
            format!("an anomaly known as {}—a place where the normal rules of existence \
                seemed to have been suspended, replaced by laws that mortal minds \
                struggled to comprehend", biome)
        }
        super::types::GeographicFeature::PrimordialRemnant { biome } => {
            format!("the remnants of something called {}—whether it had been built \
                by gods or mortals, whether it had fallen to war or time, none could say. \
                Only the stones remained, and they kept their own counsel", biome)
        }
        super::types::GeographicFeature::DesertHeart => {
            "the heart of absolute desolation—a place where even the desert itself \
            seemed to have given up, where sand gave way to bare rock and rock \
            to something older still".to_string()
        }
        super::types::GeographicFeature::FrozenWaste => {
            "a frozen waste beyond the reach of any thaw—ice older than human memory, \
            holding within it the shadows of creatures that had walked when the world \
            was young".to_string()
        }
        _ => "a place of unmistakable power, humming with energies that predated \
            the first dawn".to_string()
    }
}

fn cultural_interpretation(
    feature: &super::types::GeographicFeature,
    lens: &super::types::CulturalLens,
) -> String {
    let base_interpretation = match feature {
        super::types::GeographicFeature::MountainPeak { .. } |
        super::types::GeographicFeature::Volcano { .. } => {
            match lens {
                super::types::CulturalLens::Highland { .. } =>
                    "To one raised among peaks, this was a throne of the ancestors—a place where \
                    the veil between living and dead grew thin enough to whisper through.",
                super::types::CulturalLens::Maritime { .. } =>
                    "To one who knew the sea, this seemed an intrusion—a fist of stone thrust up \
                    from depths that should have remained buried, a challenge to the waters' dominion.",
                super::types::CulturalLens::Steppe { .. } =>
                    "To one born beneath open sky, this obstruction of the horizon felt almost \
                    blasphemous—yet also undeniably sacred, a pillar holding up the dome of heaven.",
                _ => "The place spoke of powers that transcended mortal understanding."
            }
        }
        super::types::GeographicFeature::Lake { .. } => {
            match lens {
                super::types::CulturalLens::Maritime { .. } =>
                    "Here was water trapped, unable to reach its mother the sea—a lake was a sea \
                    in mourning, and its spirits carried an ancient grief.",
                super::types::CulturalLens::Desert { .. } =>
                    "Water in such abundance seemed miraculous, holy. This was no mere lake but a \
                    gift from the gods themselves, a promise that the world was not only sand.",
                _ => "The still waters held reflections of more than the sky above."
            }
        }
        super::types::GeographicFeature::AncientSite { .. } |
        super::types::GeographicFeature::PrimordialRemnant { .. } => {
            match lens {
                super::types::CulturalLens::Highland { ancestor_worship: true, .. } =>
                    "These were the works of the First Ancestors—those who had walked when the \
                    world was new and stone itself was soft enough to shape with bare hands.",
                super::types::CulturalLens::Subterranean { .. } =>
                    "Here was proof that others had delved deep before—builders who had understood \
                    that true civilization was born in darkness, away from the blinding lie of the sun.",
                _ => "Whoever had made this place, they were gone now—but their power lingered."
            }
        }
        super::types::GeographicFeature::MysticalAnomaly { .. } => {
            match lens {
                super::types::CulturalLens::Sylvan { .. } =>
                    "The trees here—if trees they were—grew according to no natural law. \
                    This was a place where the forest's dreaming had turned strange.",
                super::types::CulturalLens::Desert { follows_stars: true, .. } =>
                    "The stars above this place formed no pattern known to any chart. Here, \
                    the sky itself had forgotten its own story.",
                _ => "Reality here was a suggestion rather than a rule. This was a wound in \
                    the world, or perhaps a doorway—the difference, if any, was beyond mortal \
                    judgment to determine."
            }
        }
        _ => "The significance of this place transcended simple description."
    };

    base_interpretation.to_string()
}

fn encounter_chapter_title(feature: &super::types::GeographicFeature, chapter: usize) -> String {
    // Use chapter number to vary titles for same feature types
    let variant = chapter % 4;
    match feature {
        super::types::GeographicFeature::MountainPeak { is_volcanic, .. } => {
            if *is_volcanic {
                match variant {
                    0 => "The Fire That Births Mountains".to_string(),
                    1 => "Where Smoke Veils the Summit".to_string(),
                    2 => "The Mountain's Burning Heart".to_string(),
                    _ => "Ash and Thunder".to_string(),
                }
            } else {
                match variant {
                    0 => "Where Stone Touches Sky".to_string(),
                    1 => "The Throne of Eagles".to_string(),
                    2 => "Upon the Roof of the World".to_string(),
                    _ => "The Silent Peak".to_string(),
                }
            }
        }
        super::types::GeographicFeature::Volcano { active } => {
            if *active {
                match variant {
                    0 => "The Breathing Earth".to_string(),
                    1 => "Where Dragons Sleep".to_string(),
                    2 => "The World's Forge".to_string(),
                    _ => "Fire Made Flesh".to_string(),
                }
            } else {
                match variant {
                    0 => "The Sleeping Fire".to_string(),
                    1 => "The Dreaming Mountain".to_string(),
                    _ => "Embers Beneath Stone".to_string(),
                }
            }
        }
        super::types::GeographicFeature::Lake { .. } => {
            match variant {
                0 => "The Waters of Contemplation".to_string(),
                1 => "The Mirror of Depths".to_string(),
                2 => "Where Still Waters Hide Secrets".to_string(),
                _ => "The Drowned Sky".to_string(),
            }
        }
        super::types::GeographicFeature::Valley { river_carved, .. } => {
            if *river_carved {
                match variant {
                    0 => "The River's Memory".to_string(),
                    1 => "Where Water Carved Time".to_string(),
                    _ => "The Ancient Gorge".to_string(),
                }
            } else {
                match variant {
                    0 => "The Hidden Vale".to_string(),
                    1 => "The Secret Between Mountains".to_string(),
                    _ => "The Sheltered Place".to_string(),
                }
            }
        }
        super::types::GeographicFeature::Coast => {
            match variant {
                0 => "Where the World Ends".to_string(),
                1 => "The Edge of All Things".to_string(),
                2 => "Where Land Surrenders".to_string(),
                _ => "The Boundary Waters".to_string(),
            }
        }
        super::types::GeographicFeature::PlateBoundary { convergent, .. } => {
            if *convergent {
                match variant {
                    0 => "The Collision of Ages".to_string(),
                    1 => "Where Continents War".to_string(),
                    2 => "The Grinding of Worlds".to_string(),
                    _ => "The Tortured Earth".to_string(),
                }
            } else {
                match variant {
                    0 => "The Wound That Will Not Heal".to_string(),
                    1 => "The World's Rift".to_string(),
                    2 => "Where the Earth Tears".to_string(),
                    _ => "The Great Divide".to_string(),
                }
            }
        }
        super::types::GeographicFeature::AncientSite { biome } => {
            match variant {
                0 => format!("Echoes of {}", biome),
                1 => format!("Whispers from {}", biome),
                2 => format!("The {} Remembers", biome),
                _ => format!("Footsteps in {}", biome),
            }
        }
        super::types::GeographicFeature::MysticalAnomaly { biome } => {
            match variant {
                0 => format!("The {} Enigma", biome),
                1 => format!("Beyond {} Laws", biome),
                2 => format!("Where {} Bends", biome),
                _ => format!("The {} Threshold", biome),
            }
        }
        super::types::GeographicFeature::PrimordialRemnant { .. } => {
            match variant {
                0 => "Memories in Stone".to_string(),
                1 => "The Fallen Age".to_string(),
                2 => "Ruins of the Before-Time".to_string(),
                _ => "What Once Was".to_string(),
            }
        }
        super::types::GeographicFeature::DesertHeart => {
            match variant {
                0 => "The Heart of Emptiness".to_string(),
                1 => "Where Even Sand Surrenders".to_string(),
                _ => "The Absolute Waste".to_string(),
            }
        }
        super::types::GeographicFeature::FrozenWaste |
        super::types::GeographicFeature::GlacialField => {
            match variant {
                0 => "The Silence of Ice".to_string(),
                1 => "The Frozen Beyond".to_string(),
                2 => "Where Winter Never Ends".to_string(),
                _ => "The Glacial Realm".to_string(),
            }
        }
        super::types::GeographicFeature::JungleCore => {
            match variant {
                0 => "The Green Heart".to_string(),
                1 => "Where Life Devours Life".to_string(),
                _ => "The Verdant Depths".to_string(),
            }
        }
        super::types::GeographicFeature::Waterfall { .. } => {
            match variant {
                0 => "The Falling Waters".to_string(),
                1 => "Where Rivers Take Flight".to_string(),
                _ => "The Mist-Shrouded Cascade".to_string(),
            }
        }
        super::types::GeographicFeature::HotSpring => {
            match variant {
                0 => "The Earth's Warm Breath".to_string(),
                1 => "Waters of the Deep Fire".to_string(),
                _ => "The Healing Springs".to_string(),
            }
        }
        super::types::GeographicFeature::Island { .. } => {
            match variant {
                0 => "The Solitary Land".to_string(),
                1 => "Realm Adrift".to_string(),
                _ => "The World Apart".to_string(),
            }
        }
        super::types::GeographicFeature::Peninsula |
        super::types::GeographicFeature::Bay |
        super::types::GeographicFeature::Strait => {
            match variant {
                0 => "Where Land Reaches for Sea".to_string(),
                1 => "The Waters' Edge".to_string(),
                _ => "The Meeting of Elements".to_string(),
            }
        }
        super::types::GeographicFeature::MountainRange { .. } => {
            match variant {
                0 => "The Spine of the World".to_string(),
                1 => "Where Giants Sleep".to_string(),
                _ => "The Stone Ramparts".to_string(),
            }
        }
        super::types::GeographicFeature::Rift { .. } => {
            match variant {
                0 => "The World's Scar".to_string(),
                1 => "The Abyss Below".to_string(),
                _ => "Where Earth Falls Away".to_string(),
            }
        }
        super::types::GeographicFeature::Plateau { .. } |
        super::types::GeographicFeature::Cliff { .. } => {
            match variant {
                0 => "The High Table".to_string(),
                1 => "The Stone Throne".to_string(),
                _ => "Above the World".to_string(),
            }
        }
        super::types::GeographicFeature::RiverSource { .. } => {
            match variant {
                0 => "The Birthplace of Waters".to_string(),
                1 => "Where Rivers Are Born".to_string(),
                _ => "The First Spring".to_string(),
            }
        }
        super::types::GeographicFeature::RiverMouth { .. } => {
            match variant {
                0 => "Where Rivers Die".to_string(),
                1 => "The Water's End".to_string(),
                _ => "The Final Journey".to_string(),
            }
        }
        super::types::GeographicFeature::RiverConfluence => {
            match variant {
                0 => "The Meeting of Waters".to_string(),
                1 => "Where Rivers Embrace".to_string(),
                _ => "The Confluence".to_string(),
            }
        }
        super::types::GeographicFeature::BiomeTransition { .. } => {
            match variant {
                0 => "The Threshold Between".to_string(),
                1 => "Where Realms Meet".to_string(),
                _ => "The Changing Lands".to_string(),
            }
        }
    }
}

/// Categorize feature types for diverse encounter selection
fn feature_type_category(feature: &super::types::GeographicFeature) -> &'static str {
    match feature {
        super::types::GeographicFeature::MountainPeak { .. } |
        super::types::GeographicFeature::MountainRange { .. } => "peak",
        super::types::GeographicFeature::Volcano { .. } => "volcano",
        super::types::GeographicFeature::Lake { .. } |
        super::types::GeographicFeature::Waterfall { .. } |
        super::types::GeographicFeature::HotSpring => "water",
        super::types::GeographicFeature::Valley { .. } |
        super::types::GeographicFeature::Plateau { .. } |
        super::types::GeographicFeature::Cliff { .. } => "terrain",
        super::types::GeographicFeature::Coast |
        super::types::GeographicFeature::Peninsula |
        super::types::GeographicFeature::Bay |
        super::types::GeographicFeature::Island { .. } |
        super::types::GeographicFeature::Strait => "coastal",
        super::types::GeographicFeature::PlateBoundary { .. } |
        super::types::GeographicFeature::Rift { .. } => "tectonic",
        super::types::GeographicFeature::AncientSite { .. } => "ancient",
        super::types::GeographicFeature::MysticalAnomaly { .. } => "mystical",
        super::types::GeographicFeature::PrimordialRemnant { .. } => "ruins",
        super::types::GeographicFeature::DesertHeart |
        super::types::GeographicFeature::JungleCore => "extreme",
        super::types::GeographicFeature::FrozenWaste |
        super::types::GeographicFeature::GlacialField => "frozen",
        super::types::GeographicFeature::RiverSource { .. } |
        super::types::GeographicFeature::RiverMouth { .. } |
        super::types::GeographicFeature::RiverConfluence => "river",
        super::types::GeographicFeature::BiomeTransition { .. } => "transition",
    }
}

fn encounter_short_name(feature: &super::types::GeographicFeature) -> String {
    match feature {
        super::types::GeographicFeature::MountainPeak { height, .. } =>
            format!("Peak at {:.0}m", height),
        super::types::GeographicFeature::Volcano { active } =>
            if *active { "Active Volcano".to_string() } else { "Dormant Volcano".to_string() },
        super::types::GeographicFeature::Lake { area, .. } =>
            format!("Lake ({} tiles)", area),
        super::types::GeographicFeature::Valley { depth, .. } =>
            format!("Valley ({:.0}m deep)", depth),
        super::types::GeographicFeature::Coast => "Coastline".to_string(),
        super::types::GeographicFeature::PlateBoundary { convergent, .. } =>
            if *convergent { "Convergent Boundary".to_string() } else { "Divergent Rift".to_string() },
        super::types::GeographicFeature::AncientSite { biome } => biome.clone(),
        super::types::GeographicFeature::MysticalAnomaly { biome } => biome.clone(),
        super::types::GeographicFeature::PrimordialRemnant { biome } => format!("Ruins of {}", biome),
        _ => "Site of Power".to_string()
    }
}

fn landmark_description(feature: &super::types::GeographicFeature) -> String {
    match feature {
        super::types::GeographicFeature::MountainPeak { height, is_volcanic } => {
            if *is_volcanic {
                format!("A volcanic peak rising {:.0}m, its summit forever wreathed in smoke and ash.", height)
            } else {
                format!("A mountain summit at {:.0}m where the wind carries voices of the ancient past.", height)
            }
        }
        super::types::GeographicFeature::Volcano { active } => {
            if *active {
                "A living volcano whose fires still burn with world-shaping power.".to_string()
            } else {
                "A dormant volcano, its fires withdrawn but not extinguished.".to_string()
            }
        }
        super::types::GeographicFeature::Lake { area, depth } => {
            format!("A lake of {} tiles, depths reaching {:.0}m into darkness below.", area, depth)
        }
        super::types::GeographicFeature::Valley { depth, river_carved } => {
            if *river_carved {
                format!("A river-carved gorge plunging {:.0}m into the earth.", depth)
            } else {
                format!("A secluded valley {:.0}m deep, hidden from the world above.", depth)
            }
        }
        super::types::GeographicFeature::Coast => {
            "The meeting of land and sea, where the known world ends.".to_string()
        }
        super::types::GeographicFeature::PlateBoundary { convergent, .. } => {
            if *convergent {
                "A zone of tectonic collision where continents war in geological time.".to_string()
            } else {
                "A rift where the world slowly tears itself apart.".to_string()
            }
        }
        super::types::GeographicFeature::AncientSite { biome } => {
            format!("An ancient {} site, remnant of powers that walked before humanity.", biome)
        }
        super::types::GeographicFeature::MysticalAnomaly { biome } => {
            format!("A {} anomaly where natural laws hold no sway.", biome)
        }
        super::types::GeographicFeature::PrimordialRemnant { biome } => {
            format!("Ruins of {}, memories of a fallen age.", biome)
        }
        _ => feature.description()
    }
}

fn sample_biomes(biomes: &std::collections::HashSet<String>, index: usize) -> String {
    biomes.iter().nth(index).cloned().unwrap_or_else(|| "unknown lands".to_string())
}
