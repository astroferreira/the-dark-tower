//! Wanderer agent system
//!
//! Handles wanderer creation, movement, and pathfinding heuristics.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::biomes::ExtendedBiome;
use crate::world::WorldData;

use super::landmarks::LandmarkRegistry;
use super::params::LoreParams;
use super::types::{CulturalLens, Direction, Wanderer, WorldLocation};

/// Name components for different cultures
const HIGHLAND_NAMES: &[&str] = &[
    "Grimm", "Thorne", "Crag", "Stone", "Peak", "Frost", "Iron", "Granite",
    "Boulder", "Ridge", "Cliff", "Summit", "Rook", "Slate", "Flint",
];

const MARITIME_NAMES: &[&str] = &[
    "Wave", "Tide", "Coral", "Pearl", "Shell", "Salt", "Storm", "Gull",
    "Anchor", "Sail", "Harbor", "Reef", "Kelp", "Brine", "Current",
];

const DESERT_NAMES: &[&str] = &[
    "Sand", "Dune", "Mirage", "Sun", "Oasis", "Wind", "Dust", "Shade",
    "Star", "Moon", "Heat", "Cactus", "Scorpion", "Viper", "Hawk",
];

const SYLVAN_NAMES: &[&str] = &[
    "Oak", "Willow", "Fern", "Moss", "Briar", "Leaf", "Root", "Branch",
    "Grove", "Ash", "Elm", "Birch", "Ivy", "Hazel", "Thorn",
];

const STEPPE_NAMES: &[&str] = &[
    "Wind", "Sky", "Grass", "Horse", "Eagle", "Arrow", "Bow", "Plain",
    "Thunder", "Cloud", "Swift", "Free", "Rider", "Falcon", "Horizon",
];

const SUBTERRANEAN_NAMES: &[&str] = &[
    "Deep", "Shadow", "Crystal", "Gem", "Tunnel", "Cavern", "Echo", "Dark",
    "Glimmer", "Vein", "Ore", "Delve", "Hollow", "Obsidian", "Onyx",
];

const NAME_SUFFIXES: &[&str] = &[
    "walker", "keeper", "singer", "speaker", "watcher", "finder",
    "seeker", "weaver", "teller", "dreamer", "wanderer", "knower",
];

/// Generate a name appropriate for the cultural lens
fn generate_name(lens: &CulturalLens, rng: &mut ChaCha8Rng) -> String {
    let names = match lens {
        CulturalLens::Highland { .. } => HIGHLAND_NAMES,
        CulturalLens::Maritime { .. } => MARITIME_NAMES,
        CulturalLens::Desert { .. } => DESERT_NAMES,
        CulturalLens::Sylvan { .. } => SYLVAN_NAMES,
        CulturalLens::Steppe { .. } => STEPPE_NAMES,
        CulturalLens::Subterranean { .. } => SUBTERRANEAN_NAMES,
    };

    let first = names[rng.gen_range(0..names.len())];
    let suffix = NAME_SUFFIXES[rng.gen_range(0..NAME_SUFFIXES.len())];

    format!("{}{}", first, suffix)
}

/// Generate a cultural lens appropriate for a starting biome
fn generate_cultural_lens(biome: ExtendedBiome, rng: &mut ChaCha8Rng) -> CulturalLens {
    // Determine base culture from biome
    let culture_type = match biome {
        // Mountains and cold highlands
        ExtendedBiome::SnowyPeaks
        | ExtendedBiome::AlpineTundra
        | ExtendedBiome::RazorPeaks
        | ExtendedBiome::Foothills => 0, // Highland

        // Coastal and water
        ExtendedBiome::CoastalWater
        | ExtendedBiome::CoralReef
        | ExtendedBiome::KelpForest
        | ExtendedBiome::SeagrassMeadow
        | ExtendedBiome::MangroveSaltmarsh
        | ExtendedBiome::Lagoon => 1, // Maritime

        // Deserts
        ExtendedBiome::Desert
        | ExtendedBiome::SaltFlats
        | ExtendedBiome::GlassDesert
        | ExtendedBiome::SingingDunes
        | ExtendedBiome::Oasis => 2, // Desert

        // Forests
        ExtendedBiome::BorealForest
        | ExtendedBiome::TemperateForest
        | ExtendedBiome::TropicalForest
        | ExtendedBiome::TropicalRainforest
        | ExtendedBiome::TemperateRainforest
        | ExtendedBiome::AncientGrove
        | ExtendedBiome::MushroomForest => 3, // Sylvan

        // Grasslands
        ExtendedBiome::TemperateGrassland
        | ExtendedBiome::Savanna
        | ExtendedBiome::Tundra => 4, // Steppe

        // Underground/caves
        ExtendedBiome::CaveEntrance
        | ExtendedBiome::Sinkhole
        | ExtendedBiome::Cenote
        | ExtendedBiome::HollowEarth => 5, // Subterranean

        // Default: random
        _ => rng.gen_range(0..6),
    };

    match culture_type {
        0 => CulturalLens::Highland {
            sacred_direction: Direction::all()[rng.gen_range(0..8)],
            ancestor_worship: rng.gen_bool(0.7),
        },
        1 => CulturalLens::Maritime {
            sea_deity_name: format!(
                "{}",
                ["Thalassa", "Nereus", "Pontus", "Oceanus", "Triton"][rng.gen_range(0..5)]
            ),
            fears_deep_water: rng.gen_bool(0.3),
        },
        2 => CulturalLens::Desert {
            follows_stars: rng.gen_bool(0.8),
            water_sacred: rng.gen_bool(0.9),
        },
        3 => CulturalLens::Sylvan {
            tree_worship: rng.gen_bool(0.7),
            fears_open_sky: rng.gen_bool(0.4),
        },
        4 => CulturalLens::Steppe {
            sky_worship: rng.gen_bool(0.6),
            values_movement: rng.gen_bool(0.8),
        },
        _ => CulturalLens::Subterranean {
            fears_sunlight: rng.gen_bool(0.5),
            crystal_worship: rng.gen_bool(0.6),
        },
    }
}

/// Find suitable starting positions for wanderers
fn find_starting_positions(world: &WorldData, count: usize, rng: &mut ChaCha8Rng) -> Vec<(usize, usize)> {
    let mut positions = Vec::with_capacity(count);
    let min_separation = (world.width + world.height) / (count * 2);

    // Try to spread wanderers across the map
    let mut attempts = 0;
    while positions.len() < count && attempts < 10000 {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);

        let tile = world.get_tile_info(x, y);

        // Prefer land tiles that aren't extreme
        if tile.elevation > 0.0
            && tile.elevation < 3000.0
            && tile.temperature > -20.0
            && tile.temperature < 45.0
        {
            // Check separation from existing positions
            let far_enough = positions
                .iter()
                .all(|&(px, py)| {
                    let dx = (x as i32 - px as i32).abs() as usize;
                    let dy = (y as i32 - py as i32).abs() as usize;
                    // Handle horizontal wrapping
                    let dx = dx.min(world.width - dx);
                    dx + dy > min_separation
                });

            if far_enough {
                positions.push((x, y));
            }
        }

        attempts += 1;
    }

    // Fill remaining with random land tiles if needed
    while positions.len() < count {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);
        let tile = world.get_tile_info(x, y);
        if tile.elevation > 0.0 {
            positions.push((x, y));
        }
    }

    positions
}

/// Create wanderer agents
pub fn create_wanderers(
    world: &WorldData,
    count: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<Wanderer> {
    let positions = find_starting_positions(world, count, rng);

    positions
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let tile = world.get_tile_info(x, y);
            let (km_x, km_y) = world.get_physical_coords(x, y);

            let cultural_lens = generate_cultural_lens(tile.biome, rng);
            let name = generate_name(&cultural_lens, rng);

            let origin = WorldLocation::from_tile(
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

            Wanderer::new(i as u32, name, origin, cultural_lens)
        })
        .collect()
}

/// Evaluate attractiveness of a potential destination tile
fn evaluate_direction(
    wanderer: &Wanderer,
    target: (usize, usize),
    world: &WorldData,
    _landmarks: &LandmarkRegistry,
    params: &LoreParams,
) -> f32 {
    let mut score = 0.0;

    let tile = world.get_tile_info(target.0, target.1);
    let biome_str = format!("{:?}", tile.biome);

    // 1. Novelty bonus - prefer unvisited biomes
    if !wanderer.visited_biomes.contains(&biome_str) {
        let rarity_bonus = if is_ultra_rare_biome(tile.biome) {
            3.0
        } else if is_fantasy_biome(tile.biome) {
            2.0
        } else {
            1.0
        };
        score += params.biome_novelty_weight * rarity_bonus;
    }

    // 2. Cultural terrain preference
    let is_water = tile.elevation < 0.0;
    let terrain_pref = wanderer
        .cultural_lens
        .terrain_preference(tile.biome, tile.elevation, is_water);
    score += params.cultural_bias_weight * terrain_pref;

    // 3. Avoid recently visited tiles
    if let Some(steps_ago) = wanderer
        .path_history
        .iter()
        .rev()
        .take(50)
        .position(|&p| p == target)
    {
        score -= params.avoid_revisit_weight * (1.0 - steps_ago as f32 / 50.0);
    }

    // 4. Feature attraction (significant geographic features)
    if tile.elevation > params.min_elevation_for_peak {
        score += params.feature_attraction_weight * 1.5;
    }
    if tile.stress.abs() > params.min_stress_for_boundary {
        score += params.feature_attraction_weight;
    }
    if is_ultra_rare_biome(tile.biome) {
        score += params.feature_attraction_weight * 2.0;
    }

    // 5. Fatigue affects preferences
    if wanderer.fatigue > 0.7 {
        // Tired wanderers seek shelter
        if matches!(
            tile.biome,
            ExtendedBiome::Oasis | ExtendedBiome::HotSprings | ExtendedBiome::AncientGrove
        ) {
            score += 2.0;
        }
        // Avoid harsh terrain
        if tile.temperature < -10.0 || tile.temperature > 40.0 {
            score -= 1.0;
        }
    }

    score
}

/// Check if wanderer can traverse a tile
fn can_traverse(wanderer: &Wanderer, world: &WorldData, x: usize, y: usize) -> bool {
    let tile = world.get_tile_info(x, y);

    // Most wanderers can't cross deep water without special abilities
    if tile.elevation < -100.0 {
        match &wanderer.cultural_lens {
            CulturalLens::Maritime { .. } => tile.elevation > -500.0,
            _ => false,
        }
    } else {
        true
    }
}

/// Move wanderer one step
pub fn step_wanderer(
    wanderer: &mut Wanderer,
    world: &WorldData,
    landmarks: &LandmarkRegistry,
    params: &LoreParams,
    rng: &mut ChaCha8Rng,
) -> bool {
    let (cx, cy) = wanderer.current_position;

    // Get all valid neighbors
    let neighbors: Vec<(usize, usize)> = Direction::all()
        .iter()
        .filter_map(|dir| {
            let (dx, dy) = dir.offset();
            let nx = (cx as i32 + dx).rem_euclid(world.width as i32) as usize;
            let ny = cy as i32 + dy;
            if ny >= 0 && ny < world.height as i32 {
                let ny = ny as usize;
                if can_traverse(wanderer, world, nx, ny) {
                    Some((nx, ny))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    if neighbors.is_empty() {
        return false; // Stuck
    }

    // Score each neighbor
    let mut scored: Vec<((usize, usize), f32)> = neighbors
        .into_iter()
        .map(|pos| {
            let score = evaluate_direction(wanderer, pos, world, landmarks, params);
            // Add randomness
            let random_factor = rng.gen::<f32>() * params.exploration_randomness;
            (pos, score + random_factor)
        })
        .collect();

    // Sort by score (descending)
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Weighted selection favoring top choices
    let weights: Vec<f32> = scored
        .iter()
        .enumerate()
        .map(|(i, _)| 1.0 / (i as f32 + 1.0).powf(1.5))
        .collect();
    let total: f32 = weights.iter().sum();
    let mut r = rng.gen::<f32>() * total;

    let mut selected = scored[0].0;
    for (i, w) in weights.iter().enumerate() {
        r -= w;
        if r <= 0.0 {
            selected = scored[i].0;
            break;
        }
    }

    // Move to selected position
    wanderer.current_position = selected;
    wanderer.path_history.push(selected);
    wanderer.steps_taken += 1;

    // Update visited biomes
    let tile = world.get_tile_info(selected.0, selected.1);
    wanderer.visited_biomes.insert(format!("{:?}", tile.biome));

    true
}

/// Check if a biome is ultra-rare
fn is_ultra_rare_biome(biome: ExtendedBiome) -> bool {
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
            | ExtendedBiome::LeviathanGraveyard
            | ExtendedBiome::DrownedCitadel
            | ExtendedBiome::VoidMaw
    )
}

/// Check if a biome is fantasy (non-realistic)
fn is_fantasy_biome(biome: ExtendedBiome) -> bool {
    matches!(
        biome,
        ExtendedBiome::DeadForest
            | ExtendedBiome::CrystalForest
            | ExtendedBiome::BioluminescentForest
            | ExtendedBiome::MushroomForest
            | ExtendedBiome::PetrifiedForest
            | ExtendedBiome::AcidLake
            | ExtendedBiome::LavaLake
            | ExtendedBiome::BioluminescentWater
            | ExtendedBiome::VolcanicWasteland
            | ExtendedBiome::Ashlands
            | ExtendedBiome::CrystalWasteland
            | ExtendedBiome::SunkenCity
            | ExtendedBiome::CyclopeanRuins
            | ExtendedBiome::BuriedTemple
            | ExtendedBiome::OvergrownCitadel
            | ExtendedBiome::DarkTower
            | ExtendedBiome::CrystalDepths
            | ExtendedBiome::PearlGardens
            | ExtendedBiome::SirenShallows
            | ExtendedBiome::FrozenAbyss
    ) || is_ultra_rare_biome(biome)
}
