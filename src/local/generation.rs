//! Local map generation with edge blending.
//!
//! Generates detailed local maps from overworld tiles, with smooth
//! transitions to neighboring biomes at the edges.

use noise::{NoiseFn, Perlin, Seedable, Fbm, MultiFractal};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use crate::biomes::ExtendedBiome;
use crate::world::WorldData;

use super::biome_features::{get_biome_features, BiomeFeatureConfig};
use super::terrain::{LocalFeature, LocalTerrainType};
use super::types::{LocalMap, LocalTile, NeighborInfo, DEFAULT_LOCAL_MAP_SIZE};

/// Width of the edge blend zone (in local map tiles)
const BLEND_WIDTH: f32 = 12.0;

/// Generate a local map for the given world tile
pub fn generate_local_map(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    size: usize,
) -> LocalMap {
    // Create deterministic seed from world seed and tile coordinates
    let local_seed = combine_seeds(world.seed, world_x as u64, world_y as u64);
    let mut rng = ChaCha8Rng::seed_from_u64(local_seed);

    // Get center biome and neighbors
    let center_biome = *world.biomes.get(world_x, world_y);
    let neighbors = get_neighbor_info(world, world_x, world_y);

    // Get biome configurations
    let center_config = get_biome_features(center_biome);

    // Create noise generators for organic blending
    let blend_noise = Perlin::new(1).set_seed(local_seed as u32);
    let feature_noise = Perlin::new(2).set_seed((local_seed + 1) as u32);
    let terrain_noise = Perlin::new(3).set_seed((local_seed + 2) as u32);

    // Create elevation noise generator (multi-octave for natural terrain)
    let elevation_noise: Fbm<Perlin> = Fbm::new((local_seed + 3) as u32)
        .set_octaves(4)
        .set_frequency(0.08)
        .set_persistence(0.5);

    // Create the local map
    let mut local_map = LocalMap::new(size, size, world_x, world_y, local_seed);

    // Phase 1 & 2: Generate terrain with edge blending
    for y in 0..size {
        for x in 0..size {
            let tile = generate_terrain_tile(
                x,
                y,
                size,
                center_biome,
                &center_config,
                &neighbors,
                &blend_noise,
                &terrain_noise,
                &mut rng,
            );
            local_map.set(x, y, tile);
        }
    }

    // Phase 3: Generate elevation offsets
    generate_elevation_offsets(&mut local_map, center_biome, &elevation_noise);

    // Phase 4: Place features with clustering
    place_features(
        &mut local_map,
        center_biome,
        &center_config,
        &neighbors,
        &feature_noise,
        &mut rng,
    );

    // Phase 5: Add water features
    add_water_features(&mut local_map, &center_config, &mut rng, world, world_x, world_y);

    local_map
}

/// Generate elevation offsets for all tiles in the local map
fn generate_elevation_offsets(
    local_map: &mut LocalMap,
    biome: ExtendedBiome,
    noise: &Fbm<Perlin>,
) {
    let amplitude = get_biome_elevation_amplitude(biome);
    let size = local_map.width;

    for y in 0..size {
        for x in 0..size {
            // Sample noise at this position
            let noise_val = noise.get([x as f64, y as f64]) as f32;

            // Scale by biome-specific amplitude
            let elevation = noise_val * amplitude;

            // Store in tile
            let tile = local_map.get_mut(x, y);
            tile.elevation_offset = elevation;

            // Slightly modify movement cost based on elevation steepness
            // (we check neighbors for slope)
            if tile.movement_cost.is_finite() {
                // Add small cost for steep terrain
                let base_cost = tile.movement_cost;
                tile.movement_cost = base_cost + elevation.abs() * 0.1;
            }
        }
    }
}

/// Get biome-specific elevation amplitude (terrain roughness)
fn get_biome_elevation_amplitude(biome: ExtendedBiome) -> f32 {
    match biome {
        // Flat biomes - minimal variation
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::AbyssalPlain => 0.1,
        ExtendedBiome::SaltFlats | ExtendedBiome::Marsh | ExtendedBiome::Bog => 0.15,
        ExtendedBiome::Desert | ExtendedBiome::SingingDunes => 0.3,
        ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna => 0.25,

        // Moderate variation - forests and wetlands
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest => 0.4,
        ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest => 0.35,
        ExtendedBiome::Swamp | ExtendedBiome::MangroveSaltmarsh => 0.2,
        ExtendedBiome::MushroomForest | ExtendedBiome::CrystalForest => 0.45,

        // Hilly/rough biomes
        ExtendedBiome::Foothills | ExtendedBiome::Tundra => 0.5,
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks => 0.7,
        ExtendedBiome::RazorPeaks | ExtendedBiome::BasaltColumns => 0.8,

        // Volcanic/geothermal - irregular terrain
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaField => 0.6,
        ExtendedBiome::Caldera | ExtendedBiome::VolcanicCone => 0.65,
        ExtendedBiome::Geysers | ExtendedBiome::FumaroleField => 0.5,

        // Karst terrain - very rough
        ExtendedBiome::KarstPlains | ExtendedBiome::TowerKarst => 0.75,
        ExtendedBiome::Sinkhole | ExtendedBiome::CockpitKarst => 0.7,

        // Crystal/magical - moderate to high
        ExtendedBiome::CrystalWasteland | ExtendedBiome::LeyNexus => 0.55,
        ExtendedBiome::FloatingStones | ExtendedBiome::StarfallCrater => 0.6,

        // Default moderate variation
        _ => 0.35,
    }
}

/// Generate a single terrain tile with edge blending
fn generate_terrain_tile(
    x: usize,
    y: usize,
    size: usize,
    center_biome: ExtendedBiome,
    center_config: &BiomeFeatureConfig,
    neighbors: &NeighborInfo,
    blend_noise: &Perlin,
    terrain_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) -> LocalTile {
    // Calculate blend factors for each edge
    let blend_north = calculate_edge_blend(y, size, true);
    let blend_south = calculate_edge_blend(y, size, false);
    let blend_west = calculate_edge_blend(x, size, true);
    let blend_east = calculate_edge_blend(x, size, false);

    // Get maximum blend factor and corresponding neighbor
    let (max_blend, neighbor_biome) = get_strongest_neighbor_blend(
        blend_north,
        blend_south,
        blend_east,
        blend_west,
        neighbors,
    );

    // Add noise to blend boundary for organic edges
    let noise_val = blend_noise.get([x as f64 * 0.1, y as f64 * 0.1]) as f32;
    let noisy_blend = (max_blend + noise_val * 0.3).clamp(0.0, 1.0);

    // Select terrain based on blend
    let terrain = if noisy_blend < 0.1 || neighbor_biome.is_none() {
        // Pure center biome
        select_terrain(center_config, terrain_noise, x, y, rng)
    } else {
        // Blended zone
        let neighbor = neighbor_biome.unwrap();
        let neighbor_config = get_biome_features(neighbor);

        if rng.gen::<f32>() < noisy_blend {
            // Use neighbor's terrain
            select_terrain(&neighbor_config, terrain_noise, x, y, rng)
        } else {
            // Use center's terrain
            select_terrain(center_config, terrain_noise, x, y, rng)
        }
    };

    LocalTile::new(terrain)
}

/// Calculate edge blend factor (1.0 at edge, 0.0 at blend_width distance from edge)
fn calculate_edge_blend(pos: usize, size: usize, is_start: bool) -> f32 {
    let dist = if is_start {
        pos as f32
    } else {
        (size - 1 - pos) as f32
    };

    if dist < BLEND_WIDTH {
        1.0 - (dist / BLEND_WIDTH)
    } else {
        0.0
    }
}

/// Get the strongest neighbor blend and corresponding biome
fn get_strongest_neighbor_blend(
    blend_north: f32,
    blend_south: f32,
    blend_east: f32,
    blend_west: f32,
    neighbors: &NeighborInfo,
) -> (f32, Option<ExtendedBiome>) {
    let candidates = [
        (blend_north, neighbors.north),
        (blend_south, neighbors.south),
        (blend_east, neighbors.east),
        (blend_west, neighbors.west),
    ];

    candidates
        .into_iter()
        .filter(|(_, biome)| biome.is_some())
        .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap())
        .map(|(blend, biome)| (blend, biome))
        .unwrap_or((0.0, None))
}

/// Select terrain type based on biome config
fn select_terrain(
    config: &BiomeFeatureConfig,
    noise: &Perlin,
    x: usize,
    y: usize,
    rng: &mut ChaCha8Rng,
) -> LocalTerrainType {
    // Use noise for natural variation
    let noise_val = noise.get([x as f64 * 0.15, y as f64 * 0.15]) as f32;
    let threshold = config.secondary_chance + noise_val * 0.1;

    if let Some(secondary) = config.secondary_terrain {
        if rng.gen::<f32>() < threshold {
            return secondary;
        }
    }

    config.primary_terrain
}

/// A cluster of features (trees, bushes, etc.)
struct FeatureCluster {
    center_x: f32,
    center_y: f32,
    radius: f32,
    feature: LocalFeature,
    density: f32,
}

/// Check if a feature type should be placed in clusters
fn is_clusterable_feature(feature: LocalFeature) -> bool {
    matches!(
        feature,
        LocalFeature::DeciduousTree
            | LocalFeature::ConiferTree
            | LocalFeature::PalmTree
            | LocalFeature::JungleTree
            | LocalFeature::WillowTree
            | LocalFeature::DeadTree
            | LocalFeature::BambooClump
            | LocalFeature::Bush
            | LocalFeature::Fern
            | LocalFeature::TallReeds
            | LocalFeature::MushroomPatch
            | LocalFeature::CrystalCluster
    )
}

/// Generate feature clusters for natural grouping
fn generate_feature_clusters(
    size: usize,
    config: &BiomeFeatureConfig,
    rng: &mut ChaCha8Rng,
) -> Vec<FeatureCluster> {
    let mut clusters = Vec::new();

    // Determine how many clusters based on biome density
    let total_density = config.total_feature_density();
    let num_clusters = ((3.0 + total_density * 10.0) as usize).min(12);

    for (feature, base_chance) in &config.features {
        if !is_clusterable_feature(*feature) {
            continue;
        }

        // Higher base chance = more clusters of this type
        let feature_clusters = ((num_clusters as f32 * base_chance * 2.0) as usize).max(1).min(5);

        for _ in 0..feature_clusters {
            // Random cluster center (with margin from edges)
            let margin = size as f32 * 0.1;
            let center_x = rng.gen_range(margin..(size as f32 - margin));
            let center_y = rng.gen_range(margin..(size as f32 - margin));

            // Cluster radius varies by feature type
            let base_radius = match feature {
                LocalFeature::DeciduousTree | LocalFeature::ConiferTree | LocalFeature::JungleTree => {
                    rng.gen_range(6.0..14.0)
                }
                LocalFeature::Bush | LocalFeature::Fern => rng.gen_range(4.0..10.0),
                LocalFeature::MushroomPatch => rng.gen_range(3.0..7.0),
                _ => rng.gen_range(5.0..12.0),
            };

            clusters.push(FeatureCluster {
                center_x,
                center_y,
                radius: base_radius,
                feature: *feature,
                density: *base_chance,
            });
        }
    }

    clusters
}

/// Calculate cluster influence at a position
fn cluster_influence(cluster: &FeatureCluster, x: usize, y: usize, noise: &Perlin) -> f32 {
    let dx = x as f32 - cluster.center_x;
    let dy = y as f32 - cluster.center_y;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist > cluster.radius * 1.5 {
        return 0.0;
    }

    // Smooth falloff with noise for organic edges
    let noise_val = noise.get([x as f64 * 0.3, y as f64 * 0.3]) as f32;
    let noisy_radius = cluster.radius * (1.0 + noise_val * 0.3);

    if dist > noisy_radius {
        // Gradual falloff outside noisy radius
        let falloff = 1.0 - (dist - noisy_radius) / (cluster.radius * 0.5);
        (falloff * cluster.density).max(0.0)
    } else {
        // Full density inside, with slight center boost
        let center_boost = 1.0 + (1.0 - dist / noisy_radius) * 0.3;
        cluster.density * center_boost
    }
}

/// Place features on the local map with clustering
fn place_features(
    local_map: &mut LocalMap,
    center_biome: ExtendedBiome,
    center_config: &BiomeFeatureConfig,
    neighbors: &NeighborInfo,
    feature_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) {
    let size = local_map.width;

    // Generate clusters for clusterable features
    let clusters = generate_feature_clusters(size, center_config, rng);

    // Separate features into clusterable and scattered
    let scattered_features: Vec<_> = center_config
        .features
        .iter()
        .filter(|(f, _)| !is_clusterable_feature(*f))
        .cloned()
        .collect();

    for y in 0..size {
        for x in 0..size {
            // Calculate blend factor for feature density reduction
            let blend_north = calculate_edge_blend(y, size, true);
            let blend_south = calculate_edge_blend(y, size, false);
            let blend_west = calculate_edge_blend(x, size, true);
            let blend_east = calculate_edge_blend(x, size, false);
            let max_blend = blend_north.max(blend_south).max(blend_west).max(blend_east);

            // Reduce feature density in blend zones
            let density_modifier = 1.0 - max_blend * 0.5;

            // Skip if terrain isn't walkable (can't place features on water, etc.)
            let tile = local_map.get(x, y);
            if !tile.terrain.is_walkable() {
                continue;
            }

            // First, try cluster-based placement for trees/vegetation
            let mut placed = false;
            for cluster in &clusters {
                let influence = cluster_influence(cluster, x, y, feature_noise);
                if influence > 0.0 {
                    // Probability based on cluster influence
                    let chance = influence * density_modifier;
                    if rng.gen::<f32>() < chance {
                        local_map.set_feature(x, y, cluster.feature);
                        placed = true;
                        break;
                    }
                }
            }

            // If not placed by cluster, try scattered features
            if !placed {
                let noise_val = feature_noise.get([x as f64 * 0.2, y as f64 * 0.2]) as f32;

                for (feature, base_chance) in &scattered_features {
                    let chance = base_chance * density_modifier * (1.0 + noise_val * 0.3);
                    if rng.gen::<f32>() < chance {
                        local_map.set_feature(x, y, *feature);
                        placed = true;
                        break;
                    }
                }
            }

            // In blend zones, also consider neighbor biome features
            if !placed && max_blend > 0.1 {
                let neighbor_biome = get_dominant_neighbor_at(
                    x, y, size, neighbors, blend_north, blend_south, blend_east, blend_west
                );

                if let Some(neighbor) = neighbor_biome {
                    let neighbor_config = get_biome_features(neighbor);

                    // Small chance to place neighbor's features in blend zone
                    for (feature, base_chance) in &neighbor_config.features {
                        let chance = base_chance * max_blend * 0.5;
                        if rng.gen::<f32>() < chance {
                            local_map.set_feature(x, y, *feature);
                            break;
                        }
                    }
                }
            }
        }
    }

    // Place rare features (caves, ruins)
    if center_config.can_have_caves {
        place_rare_features(local_map, rng);
    }
}

/// Get the dominant neighbor biome at a position
fn get_dominant_neighbor_at(
    x: usize,
    y: usize,
    size: usize,
    neighbors: &NeighborInfo,
    blend_north: f32,
    blend_south: f32,
    blend_east: f32,
    blend_west: f32,
) -> Option<ExtendedBiome> {
    let candidates = [
        (blend_north, neighbors.north),
        (blend_south, neighbors.south),
        (blend_east, neighbors.east),
        (blend_west, neighbors.west),
    ];

    candidates
        .into_iter()
        .filter(|(blend, biome)| *blend > 0.0 && biome.is_some())
        .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap())
        .and_then(|(_, biome)| biome)
}

/// Place rare features like cave openings
fn place_rare_features(local_map: &mut LocalMap, rng: &mut ChaCha8Rng) {
    let size = local_map.width;

    // Try to place 0-2 cave entrances
    let num_caves = if rng.gen::<f32>() < 0.3 { rng.gen_range(1..=2) } else { 0 };

    for _ in 0..num_caves {
        // Prefer edges and corners
        let x = if rng.gen::<bool>() {
            rng.gen_range(0..size / 4)
        } else {
            rng.gen_range(size * 3 / 4..size)
        };
        let y = if rng.gen::<bool>() {
            rng.gen_range(0..size / 4)
        } else {
            rng.gen_range(size * 3 / 4..size)
        };

        let tile = local_map.get(x, y);
        if tile.feature.is_none()
            && matches!(
                tile.terrain,
                LocalTerrainType::Stone | LocalTerrainType::Gravel
            )
        {
            local_map.set_feature(x, y, LocalFeature::CaveOpening);
        }
    }

    // Small chance for ancient ruins or monoliths
    if rng.gen::<f32>() < 0.1 {
        let x = rng.gen_range(size / 4..size * 3 / 4);
        let y = rng.gen_range(size / 4..size * 3 / 4);

        let tile = local_map.get(x, y);
        if tile.feature.is_none() && tile.terrain.is_walkable() {
            let feature = if rng.gen::<bool>() {
                LocalFeature::StoneRuin
            } else {
                LocalFeature::AncientMonolith
            };
            local_map.set_feature(x, y, feature);
        }
    }
}

/// Add water features based on biome configuration
fn add_water_features(
    local_map: &mut LocalMap,
    config: &BiomeFeatureConfig,
    rng: &mut ChaCha8Rng,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
) {
    if config.water_chance <= 0.0 {
        return;
    }

    let size = local_map.width;

    // Check if the overworld tile has a river or lake
    let has_river = check_for_river(world, world_x, world_y);

    if has_river {
        // Generate a stream across the local map
        generate_stream(local_map, rng);
    } else if rng.gen::<f32>() < config.water_chance {
        // Generate small ponds
        let num_ponds = rng.gen_range(1..=3);
        for _ in 0..num_ponds {
            let cx = rng.gen_range(size / 4..size * 3 / 4);
            let cy = rng.gen_range(size / 4..size * 3 / 4);
            let radius = rng.gen_range(2..=5);

            generate_pond(local_map, cx, cy, radius, rng);
        }
    }
}

/// Check if there's a river at the overworld tile
fn check_for_river(world: &WorldData, x: usize, y: usize) -> bool {
    let water_id = world.water_body_map.get(x, y);
    // WaterBodyId(0) typically means no water body
    if water_id.0 == 0 {
        return false;
    }
    // Find the water body and check if it's a river
    world.water_bodies
        .iter()
        .find(|wb| wb.id == *water_id)
        .map(|wb| matches!(wb.body_type, crate::water_bodies::WaterBodyType::River))
        .unwrap_or(false)
}

/// Generate a stream flowing across the local map
fn generate_stream(local_map: &mut LocalMap, rng: &mut ChaCha8Rng) {
    let size = local_map.width;

    // Pick entry and exit points on opposite edges
    let (start_x, start_y, end_x, end_y) = if rng.gen::<bool>() {
        // Horizontal flow
        (0, rng.gen_range(size / 4..size * 3 / 4), size - 1, rng.gen_range(size / 4..size * 3 / 4))
    } else {
        // Vertical flow
        (rng.gen_range(size / 4..size * 3 / 4), 0, rng.gen_range(size / 4..size * 3 / 4), size - 1)
    };

    // Create a meandering path
    let mut x = start_x as f32;
    let mut y = start_y as f32;

    let dx = (end_x as f32 - start_x as f32) / size as f32;
    let dy = (end_y as f32 - start_y as f32) / size as f32;

    for _ in 0..size * 2 {
        let ix = x.round() as usize;
        let iy = y.round() as usize;

        if ix < size && iy < size {
            local_map.set_terrain(ix, iy, LocalTerrainType::Stream);

            // Sometimes widen the stream
            if rng.gen::<f32>() < 0.3 {
                for (nx, ny) in local_map.neighbors(ix, iy) {
                    if rng.gen::<f32>() < 0.5 {
                        local_map.set_terrain(nx, ny, LocalTerrainType::Stream);
                    }
                }
            }
        }

        // Move toward end with some randomness
        x += dx + rng.gen_range(-0.3..0.3);
        y += dy + rng.gen_range(-0.3..0.3);

        if x < 0.0 || x >= size as f32 || y < 0.0 || y >= size as f32 {
            break;
        }
    }
}

/// Generate a small pond
fn generate_pond(local_map: &mut LocalMap, cx: usize, cy: usize, radius: usize, rng: &mut ChaCha8Rng) {
    let size = local_map.width;

    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            let nx = (cx as i32 + dx) as usize;
            let ny = (cy as i32 + dy) as usize;

            if nx >= size || ny >= size {
                continue;
            }

            let dist_sq = (dx * dx + dy * dy) as f32;
            let radius_sq = (radius * radius) as f32;

            // Organic shape with noise
            let noise = rng.gen::<f32>() * 0.4;
            if dist_sq < radius_sq * (1.0 + noise) {
                let tile = local_map.get(nx, ny);
                if tile.feature.is_none() {
                    if dist_sq < radius_sq * 0.5 {
                        local_map.set_terrain(nx, ny, LocalTerrainType::ShallowWater);
                    } else {
                        // Marshy edges
                        if rng.gen::<f32>() < 0.5 {
                            local_map.set_terrain(nx, ny, LocalTerrainType::Mud);
                        }
                    }
                }
            }
        }
    }
}

/// Get neighbor biome information for a world tile
fn get_neighbor_info(world: &WorldData, x: usize, y: usize) -> NeighborInfo {
    let center = *world.biomes.get(x, y);

    NeighborInfo {
        north: if y > 0 {
            let biome = *world.biomes.get(x, y - 1);
            if biome != center { Some(biome) } else { None }
        } else {
            None
        },
        south: if y < world.height - 1 {
            let biome = *world.biomes.get(x, y + 1);
            if biome != center { Some(biome) } else { None }
        } else {
            None
        },
        east: {
            // Wraps horizontally
            let ex = if x == world.width - 1 { 0 } else { x + 1 };
            let biome = *world.biomes.get(ex, y);
            if biome != center { Some(biome) } else { None }
        },
        west: {
            let wx = if x == 0 { world.width - 1 } else { x - 1 };
            let biome = *world.biomes.get(wx, y);
            if biome != center { Some(biome) } else { None }
        },
    }
}

/// Combine seeds deterministically
fn combine_seeds(world_seed: u64, x: u64, y: u64) -> u64 {
    // Use a simple mixing function
    let mut h = world_seed;
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= x;
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= y;
    h = h.wrapping_mul(0x517cc1b727220a95);
    h
}

/// Generate a local map with default size (64x64)
pub fn generate_local_map_default(world: &WorldData, world_x: usize, world_y: usize) -> LocalMap {
    generate_local_map(world, world_x, world_y, DEFAULT_LOCAL_MAP_SIZE)
}
