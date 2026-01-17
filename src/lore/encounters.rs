//! Encounter detection system
//!
//! Detects significant events during a wanderer's journey.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::biomes::ExtendedBiome;
use crate::world::{TileInfo, WorldData};

use super::landmarks::LandmarkRegistry;
use super::params::LoreParams;
use super::types::{
    EmotionalTone, Encounter, EncounterType, GeographicFeature, Wanderer, WandererReaction,
    WorldLocation,
};

/// Check if a biome is ultra-rare (triggers automatic encounter)
fn is_ultra_rare(biome: ExtendedBiome) -> bool {
    matches!(
        biome,
        ExtendedBiome::AncientGrove
            | ExtendedBiome::TitanBones
            | ExtendedBiome::CoralPlateau
            | ExtendedBiome::ObsidianFields
            | ExtendedBiome::Geysers
            | ExtendedBiome::TarPits
            | ExtendedBiome::FloatingStones
            | ExtendedBiome::Shadowfen
            | ExtendedBiome::PrismaticPools
            | ExtendedBiome::AuroraWastes
            | ExtendedBiome::SingingDunes
            | ExtendedBiome::GlassDesert
            | ExtendedBiome::AbyssalVents
            | ExtendedBiome::Sargasso
            | ExtendedBiome::EtherealMist
            | ExtendedBiome::StarfallCrater
            | ExtendedBiome::LeyNexus
            | ExtendedBiome::WhisperingStones
            | ExtendedBiome::SpiritMarsh
            | ExtendedBiome::VoidScar
            | ExtendedBiome::SiliconGrove
            | ExtendedBiome::SporeWastes
            | ExtendedBiome::BleedingStone
            | ExtendedBiome::HollowEarth
            | ExtendedBiome::SunkenCity
            | ExtendedBiome::CyclopeanRuins
            | ExtendedBiome::BuriedTemple
            | ExtendedBiome::OvergrownCitadel
            | ExtendedBiome::DarkTower
            | ExtendedBiome::LeviathanGraveyard
            | ExtendedBiome::DrownedCitadel
            | ExtendedBiome::VoidMaw
    )
}

/// Check if a biome is a ruin type
fn is_ruin_biome(biome: ExtendedBiome) -> bool {
    matches!(
        biome,
        ExtendedBiome::SunkenCity
            | ExtendedBiome::CyclopeanRuins
            | ExtendedBiome::BuriedTemple
            | ExtendedBiome::OvergrownCitadel
            | ExtendedBiome::DarkTower
            | ExtendedBiome::DrownedCitadel
    )
}

/// Check if a biome is mystical/anomalous
fn is_mystical_biome(biome: ExtendedBiome) -> bool {
    matches!(
        biome,
        ExtendedBiome::FloatingStones
            | ExtendedBiome::Shadowfen
            | ExtendedBiome::PrismaticPools
            | ExtendedBiome::AuroraWastes
            | ExtendedBiome::EtherealMist
            | ExtendedBiome::StarfallCrater
            | ExtendedBiome::LeyNexus
            | ExtendedBiome::WhisperingStones
            | ExtendedBiome::SpiritMarsh
            | ExtendedBiome::VoidScar
            | ExtendedBiome::VoidMaw
    )
}

/// Check if a tile is a rest location
pub fn is_rest_location(tile: &TileInfo) -> bool {
    matches!(
        tile.biome,
        ExtendedBiome::Oasis
            | ExtendedBiome::HotSprings
            | ExtendedBiome::AncientGrove
            | ExtendedBiome::TemperateForest
            | ExtendedBiome::BorealForest
    ) || (tile.temperature > 15.0
        && tile.temperature < 30.0
        && tile.moisture > 0.4
        && tile.elevation > 0.0
        && tile.elevation < 500.0)
}

/// Classify a biome transition for significance
fn is_significant_transition(from: ExtendedBiome, to: ExtendedBiome) -> bool {
    // Transitions involving ultra-rare biomes are always significant
    if is_ultra_rare(from) || is_ultra_rare(to) {
        return true;
    }

    // Major biome category changes
    let from_category = biome_category(from);
    let to_category = biome_category(to);

    from_category != to_category
}

/// Get broad category of a biome
fn biome_category(biome: ExtendedBiome) -> u8 {
    match biome {
        // Water
        ExtendedBiome::DeepOcean
        | ExtendedBiome::Ocean
        | ExtendedBiome::CoastalWater
        | ExtendedBiome::CoralReef
        | ExtendedBiome::KelpForest => 0,

        // Cold/Ice
        ExtendedBiome::Ice
        | ExtendedBiome::Tundra
        | ExtendedBiome::AlpineTundra
        | ExtendedBiome::SnowyPeaks
        | ExtendedBiome::FrozenLake => 1,

        // Forest
        ExtendedBiome::BorealForest
        | ExtendedBiome::TemperateForest
        | ExtendedBiome::TropicalForest
        | ExtendedBiome::TropicalRainforest
        | ExtendedBiome::TemperateRainforest => 2,

        // Desert/Arid
        ExtendedBiome::Desert
        | ExtendedBiome::SaltFlats
        | ExtendedBiome::GlassDesert
        | ExtendedBiome::SingingDunes => 3,

        // Grassland
        ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna => 4,

        // Volcanic
        ExtendedBiome::VolcanicWasteland
        | ExtendedBiome::Ashlands
        | ExtendedBiome::LavaLake
        | ExtendedBiome::Caldera
        | ExtendedBiome::VolcanicCone => 5,

        // Wetland
        ExtendedBiome::Swamp
        | ExtendedBiome::Marsh
        | ExtendedBiome::Bog
        | ExtendedBiome::MangroveSaltmarsh => 6,

        // Mystical (always distinct)
        _ if is_mystical_biome(biome) => 7,

        // Ruins (always distinct)
        _ if is_ruin_biome(biome) => 8,

        // Other
        _ => 9,
    }
}

/// Check if a position is a local elevation peak
fn is_local_peak(world: &WorldData, x: usize, y: usize) -> bool {
    let center_elevation = *world.heightmap.get(x, y);

    // Check all 8 neighbors
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(world.width as i32) as usize;
            let ny = (y as i32 + dy).max(0).min(world.height as i32 - 1) as usize;

            if *world.heightmap.get(nx, ny) > center_elevation {
                return false;
            }
        }
    }

    true
}

/// Classify feature from tile info
fn classify_feature(tile: &TileInfo, world: &WorldData, x: usize, y: usize) -> Option<GeographicFeature> {
    let biome = tile.biome;

    // Check for rare biome features first
    if is_ultra_rare(biome) {
        if is_ruin_biome(biome) {
            return Some(GeographicFeature::PrimordialRemnant {
                biome: format!("{:?}", biome),
            });
        } else if is_mystical_biome(biome) {
            return Some(GeographicFeature::MysticalAnomaly {
                biome: format!("{:?}", biome),
            });
        } else {
            return Some(GeographicFeature::AncientSite {
                biome: format!("{:?}", biome),
            });
        }
    }

    // Check for mountain peaks
    if tile.elevation > 2000.0 && is_local_peak(world, x, y) {
        return Some(GeographicFeature::MountainPeak {
            height: tile.elevation,
            is_volcanic: tile.stress > 0.4
                || matches!(
                    biome,
                    ExtendedBiome::VolcanicCone | ExtendedBiome::Caldera | ExtendedBiome::VolcanicWasteland
                ),
        });
    }

    // Check for volcanic features
    if matches!(
        biome,
        ExtendedBiome::Caldera | ExtendedBiome::VolcanicCone | ExtendedBiome::LavaLake
    ) {
        return Some(GeographicFeature::Volcano {
            active: matches!(biome, ExtendedBiome::LavaLake) || tile.stress > 0.5,
        });
    }

    // Check for plate boundaries
    if tile.stress.abs() > 0.4 {
        return Some(GeographicFeature::PlateBoundary {
            stress: tile.stress,
            convergent: tile.stress > 0.0,
        });
    }

    // Check for lakes
    if let Some(size) = tile.water_body_size {
        if matches!(tile.water_body_type, crate::water_bodies::WaterBodyType::Lake) {
            return Some(GeographicFeature::Lake {
                area: size,
                depth: tile.elevation.abs(),
            });
        }
    }

    // Check for valleys (local low points on land)
    if tile.elevation > 0.0 && tile.elevation < 200.0 && tile.stress < -0.2 {
        return Some(GeographicFeature::Valley {
            depth: 200.0 - tile.elevation,
            river_carved: tile.moisture > 0.5,
        });
    }

    // Check for hot springs
    if matches!(biome, ExtendedBiome::HotSprings | ExtendedBiome::Geysers) {
        return Some(GeographicFeature::HotSpring);
    }

    // Check for coastal features
    if tile.elevation > -10.0 && tile.elevation < 50.0 {
        // Could be coast
        // Check if adjacent to water
        let has_water_neighbor = world.heightmap.neighbors_8(x, y).iter().any(|&(nx, ny)| {
            *world.heightmap.get(nx, ny) < 0.0
        });
        if has_water_neighbor && tile.elevation > 0.0 {
            return Some(GeographicFeature::Coast);
        }
    }

    // Check for climate extremes
    if tile.temperature < -30.0 {
        return Some(GeographicFeature::FrozenWaste);
    }
    if tile.temperature > 45.0 && tile.moisture < 0.1 {
        return Some(GeographicFeature::DesertHeart);
    }
    if tile.temperature > 25.0 && tile.moisture > 0.8 {
        return Some(GeographicFeature::JungleCore);
    }
    if tile.elevation > 2500.0 && tile.temperature < 0.0 {
        return Some(GeographicFeature::GlacialField);
    }

    None
}

/// Detect encounter at wanderer's current position
pub fn detect_encounter(
    wanderer: &Wanderer,
    world: &WorldData,
    landmarks: &mut LandmarkRegistry,
    params: &LoreParams,
    rng: &mut ChaCha8Rng,
) -> Option<Encounter> {
    let (x, y) = wanderer.current_position;
    let tile = world.get_tile_info(x, y);
    let (km_x, km_y) = world.get_physical_coords(x, y);

    let location = WorldLocation::from_tile(
        x,
        y,
        km_x,
        km_y,
        tile.elevation,
        tile.temperature,
        tile.moisture,
        tile.biome,
        tile.plate_id,
        tile.stress,
        tile.water_body_type,
    );

    // Get previous tile if available
    let prev_biome = if wanderer.path_history.len() > 1 {
        let (px, py) = wanderer.path_history[wanderer.path_history.len() - 2];
        Some(world.get_tile_info(px, py).biome)
    } else {
        None
    };

    let biome_str = format!("{:?}", tile.biome);

    // 1. Rare biome discovery (always triggers)
    if is_ultra_rare(tile.biome) && !wanderer.visited_biomes.contains(&biome_str) {
        let feature = classify_feature(&tile, world, x, y);

        let landmark_id = if let Some(ref f) = feature {
            Some(landmarks.register_or_get(
                location.clone(),
                f,
                wanderer.id,
                &wanderer.cultural_lens,
                rng,
            ))
        } else {
            None
        };

        let reaction = create_reaction(&wanderer.cultural_lens, &feature, &tile, rng);

        return Some(Encounter {
            location,
            encounter_type: EncounterType::RareDiscovery {
                biome: biome_str,
            },
            feature_discovered: feature,
            landmark_discovered: landmark_id,
            story_seed_generated: None,
            wanderer_reaction: reaction,
            step_number: wanderer.steps_taken,
        });
    }

    // 2. Biome transition
    if let Some(prev) = prev_biome {
        if prev != tile.biome && is_significant_transition(prev, tile.biome) {
            // Check random chance for non-ultra-rare transitions
            if rng.gen::<f32>() > params.min_biome_transition_significance {
                return None;
            }

            let feature = Some(GeographicFeature::BiomeTransition {
                from: format!("{:?}", prev),
                to: format!("{:?}", tile.biome),
            });

            let reaction = create_reaction(&wanderer.cultural_lens, &feature, &tile, rng);

            return Some(Encounter {
                location,
                encounter_type: EncounterType::BiomeTransition {
                    from: format!("{:?}", prev),
                    to: biome_str,
                },
                feature_discovered: feature,
                landmark_discovered: None,
                story_seed_generated: None,
                wanderer_reaction: reaction,
                step_number: wanderer.steps_taken,
            });
        }
    }

    // 3. Mountain peak
    if tile.elevation > params.min_elevation_for_peak && is_local_peak(world, x, y) {
        let feature = classify_feature(&tile, world, x, y);

        let landmark_id = if let Some(ref f) = feature {
            Some(landmarks.register_or_get(
                location.clone(),
                f,
                wanderer.id,
                &wanderer.cultural_lens,
                rng,
            ))
        } else {
            None
        };

        let reaction = create_reaction(&wanderer.cultural_lens, &feature, &tile, rng);

        return Some(Encounter {
            location,
            encounter_type: EncounterType::FirstSighting {
                feature: feature.clone().unwrap_or(GeographicFeature::MountainPeak {
                    height: tile.elevation,
                    is_volcanic: false,
                }),
            },
            feature_discovered: feature,
            landmark_discovered: landmark_id,
            story_seed_generated: None,
            wanderer_reaction: reaction,
            step_number: wanderer.steps_taken,
        });
    }

    // 4. Significant tectonic feature
    if tile.stress.abs() > params.min_stress_for_boundary {
        if rng.gen::<f32>() < 0.1 {
            // 10% chance to notice
            let feature = classify_feature(&tile, world, x, y);

            let landmark_id = if let Some(ref f) = feature {
                Some(landmarks.register_or_get(
                    location.clone(),
                    f,
                    wanderer.id,
                    &wanderer.cultural_lens,
                    rng,
                ))
            } else {
                None
            };

            let reaction = create_reaction(&wanderer.cultural_lens, &feature, &tile, rng);

            return Some(Encounter {
                location,
                encounter_type: EncounterType::TectonicEvidence {
                    stress: tile.stress,
                },
                feature_discovered: feature,
                landmark_discovered: landmark_id,
                story_seed_generated: None,
                wanderer_reaction: reaction,
                step_number: wanderer.steps_taken,
            });
        }
    }

    // 5. Other notable features (lower probability)
    if rng.gen::<f32>() < 0.02 {
        if let Some(feature) = classify_feature(&tile, world, x, y) {
            let landmark_id = landmarks.register_or_get(
                location.clone(),
                &feature,
                wanderer.id,
                &wanderer.cultural_lens,
                rng,
            );

            let reaction = create_reaction(&wanderer.cultural_lens, &Some(feature.clone()), &tile, rng);

            return Some(Encounter {
                location,
                encounter_type: EncounterType::FirstSighting {
                    feature: feature.clone(),
                },
                feature_discovered: Some(feature),
                landmark_discovered: Some(landmark_id),
                story_seed_generated: None,
                wanderer_reaction: reaction,
                step_number: wanderer.steps_taken,
            });
        }
    }

    None
}

/// Create a wanderer's reaction based on cultural lens and feature
fn create_reaction(
    lens: &super::types::CulturalLens,
    feature: &Option<GeographicFeature>,
    tile: &TileInfo,
    rng: &mut ChaCha8Rng,
) -> WandererReaction {
    use super::types::CulturalLens;

    let (emotional_response, interpretation, significance) = match (lens, feature) {
        // Highland reactions
        (CulturalLens::Highland { ancestor_worship, .. }, Some(GeographicFeature::MountainPeak { height, .. })) => {
            if *ancestor_worship {
                (EmotionalTone::Reverence, "A place where ancestors dwell".to_string(), 0.9)
            } else {
                (EmotionalTone::Awe, format!("A great peak reaching {}m toward the sky", *height as i32), 0.8)
            }
        }

        (CulturalLens::Highland { .. }, Some(GeographicFeature::Valley { .. })) => {
            (EmotionalTone::Unease, "The low places are not for our kind".to_string(), 0.4)
        }

        // Maritime reactions
        (CulturalLens::Maritime { sea_deity_name, .. }, Some(GeographicFeature::Lake { .. })) => {
            (EmotionalTone::Wonder, format!("A mirror of {}'s domain", sea_deity_name), 0.7)
        }

        (CulturalLens::Maritime { .. }, Some(GeographicFeature::MountainPeak { .. })) => {
            (EmotionalTone::Dread, "Too far from the sea's embrace".to_string(), 0.3)
        }

        // Desert reactions
        (CulturalLens::Desert { water_sacred, .. }, Some(GeographicFeature::Lake { .. })) => {
            if *water_sacred {
                (EmotionalTone::Reverence, "A sacred gift in the wasteland".to_string(), 1.0)
            } else {
                (EmotionalTone::Joy, "Life-giving waters".to_string(), 0.8)
            }
        }

        (CulturalLens::Desert { follows_stars, .. }, Some(GeographicFeature::DesertHeart)) => {
            if *follows_stars {
                (EmotionalTone::Awe, "Where the stars touch the sand".to_string(), 0.9)
            } else {
                (EmotionalTone::Fear, "The burning heart of the world".to_string(), 0.6)
            }
        }

        // Sylvan reactions
        (CulturalLens::Sylvan { tree_worship, .. }, Some(GeographicFeature::AncientSite { .. })) => {
            if *tree_worship {
                (EmotionalTone::Reverence, "The first trees grew here".to_string(), 1.0)
            } else {
                (EmotionalTone::Wonder, "Spirits linger in this place".to_string(), 0.9)
            }
        }

        // Steppe reactions
        (CulturalLens::Steppe { sky_worship, .. }, Some(GeographicFeature::MountainPeak { .. })) => {
            if *sky_worship {
                (EmotionalTone::Awe, "A ladder to the sky realm".to_string(), 0.8)
            } else {
                (EmotionalTone::Curiosity, "A landmark for navigation".to_string(), 0.5)
            }
        }

        // Subterranean reactions
        (CulturalLens::Subterranean { crystal_worship, .. }, Some(GeographicFeature::Valley { .. })) => {
            if *crystal_worship {
                (EmotionalTone::Curiosity, "Perhaps crystals lie beneath".to_string(), 0.7)
            } else {
                (EmotionalTone::Wonder, "A path to the depths".to_string(), 0.6)
            }
        }

        (CulturalLens::Subterranean { .. }, Some(GeographicFeature::MountainPeak { .. })) => {
            (EmotionalTone::Dread, "Exposed to the burning sky".to_string(), 0.2)
        }

        // Mystical features - all cultures react with wonder or fear
        (_, Some(GeographicFeature::MysticalAnomaly { .. })) => {
            (EmotionalTone::Wonder, "The world grows strange here".to_string(), 0.95)
        }

        (_, Some(GeographicFeature::PrimordialRemnant { .. })) => {
            (EmotionalTone::Melancholy, "Echoes of those who came before".to_string(), 0.9)
        }

        // Default reactions
        (_, Some(feature)) => {
            let desc = feature.description();
            (EmotionalTone::Curiosity, format!("Discovered {}", desc), 0.5)
        }

        (_, None) => {
            (EmotionalTone::Curiosity, "A place of note".to_string(), 0.3)
        }
    };

    WandererReaction {
        emotional_response,
        interpretation,
        cultural_significance: significance,
    }
}
