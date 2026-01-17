//! Mythology and story seed generation
//!
//! Maps geographic features to mythological archetypes and generates story seeds.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::params::LoreParams;
use super::types::{
    Archetype, CosmicScale, CulturalLens, DangerType, DisasterType, EmotionalTone, FallCause,
    GeographicFeature, JourneyType, MoralTheme, NarrativeTheme, SanctitySource, StorySeed,
    StorySeedId, StorySeedType, SuggestedElements, WorldLocation,
};
use super::word_banks::{self, ClimateCategory, TerrainType};

/// Generate story seeds from a geographic feature encounter
pub fn generate_story_seeds(
    feature: &GeographicFeature,
    location: &WorldLocation,
    cultural_lens: &CulturalLens,
    wanderer_id: u32,
    seed_counter: &mut u32,
    params: &LoreParams,
    rng: &mut ChaCha8Rng,
) -> Vec<StorySeed> {
    let potential_types = classify_myth_potential(feature, cultural_lens);

    if potential_types.is_empty() {
        return Vec::new();
    }

    // Determine how many seeds to generate
    let num_seeds = rng.gen_range(params.story_seeds_per_encounter.0..=params.story_seeds_per_encounter.1);
    let num_seeds = num_seeds.min(potential_types.len());

    potential_types
        .into_iter()
        .take(num_seeds)
        .map(|seed_type| {
            let id = StorySeedId(*seed_counter);
            *seed_counter += 1;

            let themes = extract_themes(&seed_type, feature);
            let archetypes = extract_archetypes(&seed_type, cultural_lens);
            let emotional_tone = determine_emotional_tone(&seed_type, cultural_lens);
            let suggested_elements = generate_suggested_elements(feature, cultural_lens, rng);

            StorySeed {
                id,
                seed_type,
                primary_location: location.clone(),
                related_landmarks: Vec::new(),
                themes,
                archetypes,
                emotional_tone,
                source_wanderers: vec![wanderer_id],
                suggested_elements,
            }
        })
        .collect()
}

/// Map geographic features to potential mythological story types
fn classify_myth_potential(
    feature: &GeographicFeature,
    cultural_lens: &CulturalLens,
) -> Vec<StorySeedType> {
    let mut potentials = Vec::new();

    match feature {
        GeographicFeature::MountainPeak { height, is_volcanic } => {
            // Mountains are universal sacred sites
            potentials.push(StorySeedType::SacredPlace {
                sanctity_source: SanctitySource::ClosenessToSky,
                pilgrimage_worthy: *height > 3000.0,
            });

            if *is_volcanic {
                potentials.push(StorySeedType::CreationMyth {
                    origin_feature: feature.clone(),
                    cosmic_scale: CosmicScale::Regional,
                });
                potentials.push(StorySeedType::CataclysmMyth {
                    disaster_type: DisasterType::VolcanicEruption,
                    affected_region_description: "the surrounding lands".to_string(),
                });
            }

            // Cultural-specific interpretations
            match cultural_lens {
                CulturalLens::Highland { .. } => {
                    potentials.push(StorySeedType::HeroLegend {
                        journey_type: JourneyType::Ascent,
                        trial_features: vec!["treacherous slopes".to_string(), "thin air".to_string()],
                    });
                }
                CulturalLens::Maritime { .. } => {
                    potentials.push(StorySeedType::ForbiddenZone {
                        danger_type: DangerType::TooFarFromSea,
                        warning_signs: vec!["air too thin".to_string(), "no salt smell".to_string()],
                    });
                }
                CulturalLens::Steppe { sky_worship, .. } if *sky_worship => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::ClosenessToSky,
                        pilgrimage_worthy: true,
                    });
                }
                _ => {}
            }
        }

        GeographicFeature::Volcano { active } => {
            potentials.push(StorySeedType::CreationMyth {
                origin_feature: feature.clone(),
                cosmic_scale: CosmicScale::Regional,
            });

            if *active {
                potentials.push(StorySeedType::ForbiddenZone {
                    danger_type: DangerType::PhysicalHazard,
                    warning_signs: vec!["smoke rises".to_string(), "ground trembles".to_string()],
                });
            }

            potentials.push(StorySeedType::OriginStory {
                people_or_creature: "fire spirits".to_string(),
                birthplace_feature: "the volcanic heart".to_string(),
            });
        }

        GeographicFeature::Valley { river_carved, .. } => {
            if *river_carved {
                potentials.push(StorySeedType::HeroLegend {
                    journey_type: JourneyType::Crossing,
                    trial_features: vec!["raging waters".to_string(), "steep cliffs".to_string()],
                });
            }

            match cultural_lens {
                CulturalLens::Sylvan { .. } => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::AncientPresence,
                        pilgrimage_worthy: false,
                    });
                }
                CulturalLens::Subterranean { .. } => {
                    potentials.push(StorySeedType::OriginStory {
                        people_or_creature: "the deep folk".to_string(),
                        birthplace_feature: "where earth opens".to_string(),
                    });
                }
                _ => {}
            }
        }

        GeographicFeature::Lake { area, .. } => {
            potentials.push(StorySeedType::SacredPlace {
                sanctity_source: SanctitySource::SacredWaters,
                pilgrimage_worthy: *area > 100,
            });

            match cultural_lens {
                CulturalLens::Maritime { .. } => {
                    potentials.push(StorySeedType::CreationMyth {
                        origin_feature: feature.clone(),
                        cosmic_scale: CosmicScale::Local,
                    });
                }
                CulturalLens::Desert { water_sacred, .. } if *water_sacred => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::DivineManifestation,
                        pilgrimage_worthy: true,
                    });
                }
                _ => {}
            }

            potentials.push(StorySeedType::OriginStory {
                people_or_creature: "water spirits".to_string(),
                birthplace_feature: "the deep waters".to_string(),
            });
        }

        GeographicFeature::PlateBoundary { stress, convergent } => {
            if *convergent {
                potentials.push(StorySeedType::CreationMyth {
                    origin_feature: feature.clone(),
                    cosmic_scale: CosmicScale::Continental,
                });
                potentials.push(StorySeedType::HeroLegend {
                    journey_type: JourneyType::CosmicBattle,
                    trial_features: vec!["where lands collide".to_string()],
                });
            } else {
                potentials.push(StorySeedType::CataclysmMyth {
                    disaster_type: DisasterType::WorldRift,
                    affected_region_description: "the lands that were once one".to_string(),
                });
            }

            if stress.abs() > 0.6 {
                potentials.push(StorySeedType::ForbiddenZone {
                    danger_type: DangerType::CursedGround,
                    warning_signs: vec!["the earth trembles".to_string(), "cracks widen".to_string()],
                });
            }
        }

        GeographicFeature::AncientSite { biome } => {
            match biome.as_str() {
                "TitanBones" => {
                    potentials.push(StorySeedType::CreationMyth {
                        origin_feature: feature.clone(),
                        cosmic_scale: CosmicScale::Cosmic,
                    });
                    potentials.push(StorySeedType::OriginStory {
                        people_or_creature: "the giants who shaped the land".to_string(),
                        birthplace_feature: "before time began".to_string(),
                    });
                }
                "AncientGrove" => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::FirstForest,
                        pilgrimage_worthy: true,
                    });
                    potentials.push(StorySeedType::CreationMyth {
                        origin_feature: feature.clone(),
                        cosmic_scale: CosmicScale::Regional,
                    });
                }
                _ => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::AncientPresence,
                        pilgrimage_worthy: true,
                    });
                }
            }
        }

        GeographicFeature::MysticalAnomaly { biome } => {
            match biome.as_str() {
                "FloatingStones" | "VoidScar" | "VoidMaw" => {
                    potentials.push(StorySeedType::ForbiddenZone {
                        danger_type: DangerType::ThinReality,
                        warning_signs: vec!["reality bends".to_string(), "time flows strangely".to_string()],
                    });
                    potentials.push(StorySeedType::CreationMyth {
                        origin_feature: feature.clone(),
                        cosmic_scale: CosmicScale::Cosmic,
                    });
                }
                "Shadowfen" | "SpiritMarsh" | "EtherealMist" => {
                    potentials.push(StorySeedType::ForbiddenZone {
                        danger_type: DangerType::DwellingOfMonsters,
                        warning_signs: vec!["whispers in the mist".to_string(), "paths shift".to_string()],
                    });
                    potentials.push(StorySeedType::OriginStory {
                        people_or_creature: "the restless dead".to_string(),
                        birthplace_feature: "where the veil is thin".to_string(),
                    });
                }
                "LeyNexus" | "StarfallCrater" => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::ElementalConvergence,
                        pilgrimage_worthy: true,
                    });
                    potentials.push(StorySeedType::CreationMyth {
                        origin_feature: feature.clone(),
                        cosmic_scale: CosmicScale::Cosmic,
                    });
                }
                _ => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::DivineManifestation,
                        pilgrimage_worthy: true,
                    });
                }
            }
        }

        GeographicFeature::PrimordialRemnant { biome } => {
            let fall_cause = match biome.as_str() {
                "SunkenCity" | "DrownedCitadel" => FallCause::NaturalDisaster,
                "CyclopeanRuins" => FallCause::Unknown,
                "BuriedTemple" => FallCause::Abandonment,
                "OvergrownCitadel" => FallCause::War,
                "DarkTower" => FallCause::Corruption,
                _ => FallCause::Hubris,
            };

            potentials.push(StorySeedType::LostCivilization {
                ruin_biome: biome.clone(),
                fall_cause,
            });

            potentials.push(StorySeedType::ForbiddenZone {
                danger_type: DangerType::CursedGround,
                warning_signs: vec!["echoes of the past".to_string(), "restless spirits".to_string()],
            });

            potentials.push(StorySeedType::HeroLegend {
                journey_type: JourneyType::Quest,
                trial_features: vec!["ancient traps".to_string(), "forgotten guardians".to_string()],
            });
        }

        GeographicFeature::Coast => {
            match cultural_lens {
                CulturalLens::Maritime { .. } => {
                    potentials.push(StorySeedType::SacredPlace {
                        sanctity_source: SanctitySource::ElementalConvergence,
                        pilgrimage_worthy: false,
                    });
                }
                CulturalLens::Highland { .. } | CulturalLens::Desert { .. } => {
                    potentials.push(StorySeedType::Parable {
                        moral_theme: MoralTheme::Courage,
                        setting_feature: "the edge of the known world".to_string(),
                    });
                }
                _ => {}
            }
        }

        GeographicFeature::DesertHeart | GeographicFeature::FrozenWaste => {
            potentials.push(StorySeedType::HeroLegend {
                journey_type: JourneyType::Exile,
                trial_features: vec!["endless desolation".to_string(), "no shelter".to_string()],
            });
            potentials.push(StorySeedType::Parable {
                moral_theme: MoralTheme::Patience,
                setting_feature: feature.description(),
            });
        }

        _ => {
            // Generic feature - generate based on cultural lens with varied themes
            let themes = [
                MoralTheme::Wisdom,
                MoralTheme::Sacrifice,
                MoralTheme::Courage,
                MoralTheme::Patience,
                MoralTheme::Balance,
                MoralTheme::Transformation,
                MoralTheme::Harmony,
                MoralTheme::Hubris,
            ];
            // Use feature description hash to deterministically but variably pick theme
            let desc = feature.description();
            let hash = desc.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
            let theme = themes[hash % themes.len()];
            potentials.push(StorySeedType::Parable {
                moral_theme: theme,
                setting_feature: desc,
            });
        }
    }

    potentials
}

/// Extract narrative themes from story seed type
fn extract_themes(seed_type: &StorySeedType, feature: &GeographicFeature) -> Vec<NarrativeTheme> {
    let mut themes = match seed_type {
        StorySeedType::CreationMyth { .. } => vec![NarrativeTheme::Creation, NarrativeTheme::Power],
        StorySeedType::HeroLegend { journey_type, .. } => {
            let mut t = vec![NarrativeTheme::Journey];
            match journey_type {
                JourneyType::Ascent | JourneyType::Descent => t.push(NarrativeTheme::Transformation),
                JourneyType::CosmicBattle => t.push(NarrativeTheme::Conflict),
                JourneyType::Quest => t.push(NarrativeTheme::Discovery),
                JourneyType::Exile => t.push(NarrativeTheme::Loss),
                JourneyType::Pilgrimage => t.push(NarrativeTheme::Mystery),
                JourneyType::Crossing => t.push(NarrativeTheme::Journey),
            }
            t
        }
        StorySeedType::Parable { moral_theme, .. } => {
            match moral_theme {
                MoralTheme::Sacrifice => vec![NarrativeTheme::Sacrifice, NarrativeTheme::Transformation],
                MoralTheme::Wisdom => vec![NarrativeTheme::Discovery, NarrativeTheme::Mystery],
                MoralTheme::Courage => vec![NarrativeTheme::Conflict, NarrativeTheme::Journey],
                MoralTheme::Transformation => vec![NarrativeTheme::Transformation, NarrativeTheme::Rebirth],
                _ => vec![NarrativeTheme::Mystery],
            }
        }
        StorySeedType::OriginStory { .. } => vec![NarrativeTheme::Creation, NarrativeTheme::Nature],
        StorySeedType::CataclysmMyth { disaster_type, .. } => {
            let mut t = vec![NarrativeTheme::Destruction];
            match disaster_type {
                DisasterType::DivineWrath => t.push(NarrativeTheme::Power),
                DisasterType::Corruption => t.push(NarrativeTheme::Loss),
                _ => t.push(NarrativeTheme::Nature),
            }
            t
        }
        StorySeedType::SacredPlace { .. } => vec![NarrativeTheme::Mystery, NarrativeTheme::Power],
        StorySeedType::ForbiddenZone { .. } => vec![NarrativeTheme::Mystery, NarrativeTheme::Conflict],
        StorySeedType::LostCivilization { fall_cause, .. } => {
            let mut t = vec![NarrativeTheme::Loss, NarrativeTheme::Mystery];
            match fall_cause {
                FallCause::Hubris => t.push(NarrativeTheme::Power),
                FallCause::War => t.push(NarrativeTheme::Conflict),
                FallCause::Corruption => t.push(NarrativeTheme::Destruction),
                _ => {}
            }
            t
        }
    };

    // Add feature-specific themes
    match feature {
        GeographicFeature::Volcano { .. } => {
            if !themes.contains(&NarrativeTheme::Destruction) {
                themes.push(NarrativeTheme::Destruction);
            }
            if !themes.contains(&NarrativeTheme::Rebirth) {
                themes.push(NarrativeTheme::Rebirth);
            }
        }
        GeographicFeature::Lake { .. } => {
            if !themes.contains(&NarrativeTheme::Nature) {
                themes.push(NarrativeTheme::Nature);
            }
        }
        _ => {}
    }

    themes
}

/// Extract archetypes from story seed type and cultural lens
fn extract_archetypes(seed_type: &StorySeedType, cultural_lens: &CulturalLens) -> Vec<Archetype> {
    let mut archetypes = match seed_type {
        StorySeedType::CreationMyth { .. } => vec![Archetype::Creator],
        StorySeedType::HeroLegend { .. } => vec![Archetype::Hero],
        StorySeedType::Parable { .. } => vec![Archetype::Sage],
        StorySeedType::OriginStory { .. } => vec![Archetype::Creator, Archetype::Innocent],
        StorySeedType::CataclysmMyth { .. } => vec![Archetype::Destroyer],
        StorySeedType::SacredPlace { .. } => vec![Archetype::Guardian],
        StorySeedType::ForbiddenZone { .. } => vec![Archetype::Monster, Archetype::Shadow],
        StorySeedType::LostCivilization { .. } => vec![Archetype::Wanderer, Archetype::Shadow],
    };

    // Add cultural-specific archetypes
    match cultural_lens {
        CulturalLens::Highland { ancestor_worship, .. } if *ancestor_worship => {
            archetypes.push(Archetype::Guardian);
        }
        CulturalLens::Maritime { .. } => {
            archetypes.push(Archetype::Wanderer);
        }
        CulturalLens::Sylvan { .. } => {
            if !archetypes.contains(&Archetype::Sage) {
                archetypes.push(Archetype::Sage);
            }
        }
        CulturalLens::Steppe { .. } => {
            if !archetypes.contains(&Archetype::Wanderer) {
                archetypes.push(Archetype::Wanderer);
            }
        }
        _ => {}
    }

    // Add trickster occasionally for variety
    if archetypes.len() < 3 {
        archetypes.push(Archetype::Trickster);
    }

    archetypes
}

/// Determine emotional tone based on story type and cultural lens
fn determine_emotional_tone(seed_type: &StorySeedType, _cultural_lens: &CulturalLens) -> EmotionalTone {
    match seed_type {
        StorySeedType::CreationMyth { cosmic_scale, .. } => {
            match cosmic_scale {
                CosmicScale::Cosmic => EmotionalTone::Awe,
                CosmicScale::Continental => EmotionalTone::Reverence,
                _ => EmotionalTone::Wonder,
            }
        }
        StorySeedType::HeroLegend { journey_type, .. } => {
            match journey_type {
                JourneyType::Exile => EmotionalTone::Melancholy,
                JourneyType::CosmicBattle => EmotionalTone::Awe,
                JourneyType::Pilgrimage => EmotionalTone::Reverence,
                _ => EmotionalTone::Curiosity,
            }
        }
        StorySeedType::CataclysmMyth { .. } => EmotionalTone::Dread,
        StorySeedType::ForbiddenZone { danger_type, .. } => {
            match danger_type {
                DangerType::ThinReality => EmotionalTone::Dread,
                DangerType::CursedGround => EmotionalTone::Unease,
                DangerType::DwellingOfMonsters => EmotionalTone::Fear,
                _ => EmotionalTone::Fear,
            }
        }
        StorySeedType::SacredPlace { .. } => EmotionalTone::Reverence,
        StorySeedType::LostCivilization { .. } => EmotionalTone::Melancholy,
        StorySeedType::OriginStory { .. } => EmotionalTone::Wonder,
        StorySeedType::Parable { .. } => EmotionalTone::Curiosity,
    }
}

/// Determine terrain type from geographic feature
fn terrain_from_feature(feature: &GeographicFeature) -> TerrainType {
    match feature {
        GeographicFeature::MountainPeak { .. }
        | GeographicFeature::MountainRange { .. }
        | GeographicFeature::Plateau { .. }
        | GeographicFeature::Cliff { .. } => TerrainType::Mountain,

        GeographicFeature::Lake { .. }
        | GeographicFeature::RiverSource { .. }
        | GeographicFeature::RiverMouth { .. }
        | GeographicFeature::RiverConfluence
        | GeographicFeature::Waterfall { .. }
        | GeographicFeature::HotSpring => TerrainType::Water,

        GeographicFeature::Coast
        | GeographicFeature::Peninsula
        | GeographicFeature::Bay
        | GeographicFeature::Island { .. }
        | GeographicFeature::Strait => TerrainType::Coastal,

        GeographicFeature::Valley { .. } => TerrainType::Forest,

        GeographicFeature::Volcano { .. }
        | GeographicFeature::Rift { .. }
        | GeographicFeature::PlateBoundary { .. } => TerrainType::Mountain,

        GeographicFeature::DesertHeart => TerrainType::Desert,
        GeographicFeature::FrozenWaste | GeographicFeature::GlacialField => TerrainType::Mountain,
        GeographicFeature::JungleCore => TerrainType::Forest,

        GeographicFeature::MysticalAnomaly { .. } => TerrainType::Mystical,
        GeographicFeature::AncientSite { biome } => {
            if biome.contains("Grove") || biome.contains("Forest") {
                TerrainType::Forest
            } else {
                TerrainType::Plains
            }
        }
        GeographicFeature::PrimordialRemnant { biome } => {
            if biome.contains("Sunken") || biome.contains("Drowned") {
                TerrainType::Water
            } else if biome.contains("Tower") {
                TerrainType::Mountain
            } else {
                TerrainType::Underground
            }
        }

        GeographicFeature::BiomeTransition { from: _, to } => {
            // Use the "to" biome to determine terrain
            word_banks::terrain_from_feature(to)
        }
    }
}

/// Determine climate from geographic feature
fn climate_from_feature(feature: &GeographicFeature) -> Option<ClimateCategory> {
    match feature {
        GeographicFeature::Volcano { .. } | GeographicFeature::HotSpring => Some(ClimateCategory::Hot),
        GeographicFeature::FrozenWaste | GeographicFeature::GlacialField => Some(ClimateCategory::Cold),
        GeographicFeature::DesertHeart => Some(ClimateCategory::Dry),
        GeographicFeature::JungleCore => Some(ClimateCategory::Wet),
        GeographicFeature::Lake { .. }
        | GeographicFeature::RiverSource { .. }
        | GeographicFeature::Waterfall { .. } => Some(ClimateCategory::Wet),
        _ => None,
    }
}

/// Generate suggested story elements based on feature and culture
fn generate_suggested_elements(
    feature: &GeographicFeature,
    cultural_lens: &CulturalLens,
    rng: &mut ChaCha8Rng,
) -> SuggestedElements {
    let mut elements = SuggestedElements::default();

    // Determine context for word banks
    let terrain = terrain_from_feature(feature);
    let climate = climate_from_feature(feature);

    // Generate deities (1-3)
    let deity_count = rng.gen_range(1..=3);
    for _ in 0..deity_count {
        let deity = word_banks::generate_deity_name(climate, Some(terrain), cultural_lens, rng);
        if !elements.deity_names.contains(&deity) {
            elements.deity_names.push(deity);
        }
    }

    // Generate creatures (1-3)
    let creature_count = rng.gen_range(1..=3);
    elements.creature_types = word_banks::generate_creatures(climate, Some(terrain), cultural_lens, creature_count, rng);

    // Generate artifacts (1-2)
    let artifact_count = rng.gen_range(1..=2);
    elements.artifact_types = word_banks::generate_artifacts(Some(terrain), artifact_count, rng);

    // Generate rituals (1-2)
    let ritual_count = rng.gen_range(1..=2);
    elements.ritual_types = word_banks::generate_rituals(cultural_lens, ritual_count, rng);

    // Generate taboos (50% chance for 1-2)
    if rng.gen_bool(0.5) {
        let taboo_count = rng.gen_range(1..=2);
        elements.taboos = word_banks::generate_taboos(Some(terrain), cultural_lens, taboo_count, rng);
    }

    // Feature-specific overrides for very specific cases
    match feature {
        GeographicFeature::Volcano { active } => {
            // Always include fire-themed elements for volcanoes
            if *active {
                elements.taboos.push("never turn your back to the mountain".to_string());
            }
            if !elements.deity_names.iter().any(|n| n.to_lowercase().contains("flame") || n.to_lowercase().contains("fire") || n.to_lowercase().contains("molten")) {
                elements.deity_names.insert(0, word_banks::generate_deity_name(Some(ClimateCategory::Hot), Some(TerrainType::Mountain), cultural_lens, rng));
            }
        }

        GeographicFeature::MysticalAnomaly { biome } => {
            // Mystical biomes get special creatures
            match biome.as_str() {
                "VoidScar" | "VoidMaw" => {
                    elements.taboos.push("looking directly into the void".to_string());
                }
                "Shadowfen" => {
                    elements.taboos.push("speaking names aloud in the mist".to_string());
                }
                "FloatingStones" => {
                    elements.taboos.push("touching the ground with bare feet".to_string());
                }
                _ => {}
            }
        }

        GeographicFeature::AncientSite { biome } => {
            match biome.as_str() {
                "TitanBones" => {
                    elements.creature_types.insert(0, "bone golems".to_string());
                    elements.taboos.push("disturbing the bones of the first ones".to_string());
                }
                "AncientGrove" => {
                    elements.ritual_types.insert(0, "the remembering of first roots".to_string());
                }
                _ => {}
            }
        }

        GeographicFeature::PrimordialRemnant { biome: _ } => {
            elements.creature_types.insert(0, "forgotten guardians".to_string());
            elements.taboos.push("taking without offering".to_string());
        }

        _ => {}
    }

    // Cultural overlays - add culture-specific elements
    match cultural_lens {
        CulturalLens::Highland { ancestor_worship, .. } if *ancestor_worship => {
            elements.ritual_types.push("ancestor calling".to_string());
        }
        CulturalLens::Maritime { sea_deity_name, .. } => {
            // Add the named sea deity if not already present
            if !elements.deity_names.iter().any(|n| n == sea_deity_name) {
                elements.deity_names.push(sea_deity_name.clone());
            }
        }
        CulturalLens::Desert { follows_stars, .. } if *follows_stars => {
            elements.ritual_types.push("star reading at journey's end".to_string());
        }
        CulturalLens::Sylvan { tree_worship, .. } if *tree_worship => {
            elements.ritual_types.push("tree binding ceremony".to_string());
        }
        CulturalLens::Steppe { sky_worship, .. } if *sky_worship => {
            elements.ritual_types.push("sky blessing under open heavens".to_string());
        }
        CulturalLens::Subterranean { crystal_worship, .. } if *crystal_worship => {
            elements.ritual_types.push("crystal communion in darkness".to_string());
        }
        _ => {}
    }

    elements
}
