//! Structure placement using desirability maps
//!
//! Computes desirability maps for different structure types based on terrain,
//! climate, and existing features. Uses these maps to intelligently place structures.

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::water_bodies::WaterBodyId;
use super::types::{DesirabilityMap, PlacedStructure, StructureType};

/// Compute desirability map for castle placement
///
/// Castles prefer:
/// - High elevation (hilltops)
/// - Some distance from water (defensible)
/// - Near tectonic stress zones (dramatic mountain locations)
/// - Flat enough for foundations
/// - Away from existing structures
pub fn compute_castle_desirability(
    heightmap: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    biomes: &Tilemap<ExtendedBiome>,
    existing_structures: &[PlacedStructure],
) -> DesirabilityMap {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut desirability = DesirabilityMap::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let stress = stress_map.get(x, y).abs();
            let water_id = *water_bodies.get(x, y);
            let biome = *biomes.get(x, y);

            // Skip underwater tiles
            if elev <= 0.0 || !water_id.is_none() {
                desirability.set(x, y, f32::MIN);
                continue;
            }

            // Skip unsuitable biomes
            if !is_buildable_biome(&biome) {
                desirability.set(x, y, f32::MIN);
                continue;
            }

            let mut score = 0.0;

            // Prefer high elevation (hills, mountains) - major factor
            score += (elev / 500.0).clamp(0.0, 2.0) * 3.0;

            // Bonus for being a local peak
            let local_peak_bonus = compute_local_peak_bonus(heightmap, x, y);
            score += local_peak_bonus * 5.0;

            // Moderate preference for tectonic stress (dramatic terrain)
            score += stress * 1.5;

            // Penalize if too close to water (less defensible)
            let water_dist = compute_water_distance(water_bodies, x, y, 10);
            if water_dist < 3.0 {
                score -= (3.0 - water_dist) * 0.5;
            }

            // Penalize if too close to existing structures
            for structure in existing_structures {
                let (cx, cy) = structure.center();
                let dist = ((x as f32 - cx as f32).powi(2) + (y as f32 - cy as f32).powi(2)).sqrt();
                if dist < 50.0 {
                    score -= (50.0 - dist) * 0.1;
                }
            }

            // Compute slope penalty (need flat foundation)
            let slope = compute_slope(heightmap, x, y);
            if slope > 0.5 {
                score -= (slope - 0.5) * 2.0;
            }

            desirability.set(x, y, score);
        }
    }

    desirability
}

/// Compute desirability map for city placement
///
/// Cities prefer:
/// - Near fresh water (rivers, lakes)
/// - Flat terrain
/// - Fertile biomes (grassland, forest edge)
/// - Moderate temperature
/// - Away from mountains and deserts
pub fn compute_city_desirability(
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    biomes: &Tilemap<ExtendedBiome>,
    existing_structures: &[PlacedStructure],
) -> DesirabilityMap {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut desirability = DesirabilityMap::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let moist = *moisture.get(x, y);
            let temp = *temperature.get(x, y);
            let water_id = *water_bodies.get(x, y);
            let biome = *biomes.get(x, y);

            // Skip underwater tiles
            if elev <= 0.0 || !water_id.is_none() {
                desirability.set(x, y, f32::MIN);
                continue;
            }

            // Skip unsuitable biomes
            if !is_buildable_biome(&biome) {
                desirability.set(x, y, f32::MIN);
                continue;
            }

            let mut score = 0.0;

            // Strong preference for proximity to fresh water
            let water_dist = compute_water_distance(water_bodies, x, y, 20);
            if water_dist <= 5.0 {
                score += (5.0 - water_dist) * 2.0;
            } else if water_dist <= 15.0 {
                score += (15.0 - water_dist) * 0.3;
            }

            // Prefer flat terrain - major factor for cities
            let slope = compute_slope(heightmap, x, y);
            score += (1.0 - slope.min(1.0)) * 4.0;

            // Prefer low to moderate elevation (not mountains)
            if elev > 0.0 && elev < 500.0 {
                score += 2.0;
            } else if elev >= 500.0 && elev < 1000.0 {
                score += 1.0;
            } else if elev >= 1000.0 {
                score -= (elev - 1000.0) / 500.0;
            }

            // Prefer moderate moisture (fertile land)
            if moist > 0.3 && moist < 0.8 {
                score += 2.0;
            }

            // Prefer moderate temperature
            if temp > 5.0 && temp < 25.0 {
                score += 1.5;
            } else if temp <= 5.0 {
                score -= (5.0 - temp) * 0.1;
            } else if temp >= 25.0 {
                score -= (temp - 25.0) * 0.1;
            }

            // Bonus for fertile biomes
            match biome {
                ExtendedBiome::TemperateGrassland | ExtendedBiome::TemperateRainforest |
                ExtendedBiome::TemperateForest | ExtendedBiome::Foothills => {
                    score += 2.0;
                }
                ExtendedBiome::Savanna | ExtendedBiome::TropicalForest => {
                    score += 1.5;
                }
                _ => {}
            }

            // Penalize if too close to existing structures
            for structure in existing_structures {
                let (cx, cy) = structure.center();
                let dist = ((x as f32 - cx as f32).powi(2) + (y as f32 - cy as f32).powi(2)).sqrt();
                if dist < 80.0 {
                    score -= (80.0 - dist) * 0.1;
                }
            }

            desirability.set(x, y, score);
        }
    }

    desirability
}

/// Compute desirability map for village placement
///
/// Villages prefer:
/// - Near roads (after roads are placed)
/// - Moderate distance from cities
/// - Near resources (forests, water)
/// - Flat terrain
pub fn compute_village_desirability(
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    biomes: &Tilemap<ExtendedBiome>,
    existing_structures: &[PlacedStructure],
) -> DesirabilityMap {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut desirability = DesirabilityMap::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let moist = *moisture.get(x, y);
            let water_id = *water_bodies.get(x, y);
            let biome = *biomes.get(x, y);

            // Skip underwater tiles
            if elev <= 0.0 || !water_id.is_none() {
                desirability.set(x, y, f32::MIN);
                continue;
            }

            // Skip unsuitable biomes
            if !is_buildable_biome(&biome) {
                desirability.set(x, y, f32::MIN);
                continue;
            }

            let mut score = 0.0;

            // Prefer flat terrain
            let slope = compute_slope(heightmap, x, y);
            score += (1.0 - slope.min(1.0)) * 2.0;

            // Prefer proximity to fresh water
            let water_dist = compute_water_distance(water_bodies, x, y, 15);
            if water_dist <= 5.0 {
                score += (5.0 - water_dist) * 1.0;
            }

            // Prefer moderate moisture
            if moist > 0.2 && moist < 0.7 {
                score += 1.0;
            }

            // Prefer being near (but not too close to) major structures
            let mut near_major = false;
            for structure in existing_structures {
                if matches!(structure.structure_type, StructureType::Castle | StructureType::City) {
                    let (cx, cy) = structure.center();
                    let dist = ((x as f32 - cx as f32).powi(2) + (y as f32 - cy as f32).powi(2)).sqrt();

                    if dist > 30.0 && dist < 100.0 {
                        score += 2.0; // Sweet spot distance
                        near_major = true;
                    } else if dist <= 30.0 {
                        score -= 3.0; // Too close to major structure
                    }
                }
            }

            // Bonus for areas along probable trade routes (between major structures)
            if !near_major && existing_structures.len() >= 2 {
                let mut min_path_dist = f32::MAX;
                for i in 0..existing_structures.len() {
                    for j in (i + 1)..existing_structures.len() {
                        let (ax, ay) = existing_structures[i].center();
                        let (bx, by) = existing_structures[j].center();
                        let path_dist = point_to_line_distance(
                            x as f32, y as f32,
                            ax as f32, ay as f32,
                            bx as f32, by as f32,
                        );
                        min_path_dist = min_path_dist.min(path_dist);
                    }
                }
                if min_path_dist < 20.0 {
                    score += (20.0 - min_path_dist) * 0.2;
                }
            }

            // Penalize if too close to existing villages
            for structure in existing_structures {
                if structure.structure_type == StructureType::Village {
                    let (cx, cy) = structure.center();
                    let dist = ((x as f32 - cx as f32).powi(2) + (y as f32 - cy as f32).powi(2)).sqrt();
                    if dist < 25.0 {
                        score -= (25.0 - dist) * 0.3;
                    }
                }
            }

            desirability.set(x, y, score);
        }
    }

    desirability
}

/// Check if a biome is suitable for building structures
fn is_buildable_biome(biome: &ExtendedBiome) -> bool {
    !matches!(
        biome,
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::CoastalWater |
        ExtendedBiome::Lagoon | ExtendedBiome::FrozenLake | ExtendedBiome::LavaLake |
        ExtendedBiome::AcidLake | ExtendedBiome::BioluminescentWater | ExtendedBiome::Ice |
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::Ashlands | ExtendedBiome::SulfurVents
    )
}

/// Compute how much this tile is a local peak compared to neighbors
fn compute_local_peak_bonus(heightmap: &Tilemap<f32>, x: usize, y: usize) -> f32 {
    let center_elev = *heightmap.get(x, y);
    let mut higher_neighbors = 0;
    let mut total_neighbors = 0;

    let radius = 5;
    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(heightmap.width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, heightmap.height as i32 - 1) as usize;

            let neighbor_elev = *heightmap.get(nx, ny);
            total_neighbors += 1;

            if neighbor_elev > center_elev {
                higher_neighbors += 1;
            }
        }
    }

    // Return how "peak-like" this location is (1.0 = no higher neighbors)
    if total_neighbors > 0 {
        1.0 - (higher_neighbors as f32 / total_neighbors as f32)
    } else {
        0.0
    }
}

/// Compute distance to nearest water body
fn compute_water_distance(
    water_bodies: &Tilemap<WaterBodyId>,
    x: usize,
    y: usize,
    max_search: usize,
) -> f32 {
    let width = water_bodies.width;
    let height = water_bodies.height;

    for radius in 1..=max_search {
        let r = radius as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue; // Only check perimeter
                }

                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                let water_id = *water_bodies.get(nx, ny);
                if !water_id.is_none() {
                    return ((dx * dx + dy * dy) as f32).sqrt();
                }
            }
        }
    }

    max_search as f32 + 1.0
}

/// Compute local slope at a position
fn compute_slope(heightmap: &Tilemap<f32>, x: usize, y: usize) -> f32 {
    let center = *heightmap.get(x, y);
    let mut max_diff = 0.0f32;

    for (nx, ny) in heightmap.neighbors(x, y) {
        let neighbor = *heightmap.get(nx, ny);
        let diff = (center - neighbor).abs();
        max_diff = max_diff.max(diff);
    }

    // Normalize slope (0 = flat, 1 = very steep)
    (max_diff / 100.0).min(1.0)
}

/// Compute distance from a point to a line segment
fn point_to_line_distance(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;

    let ab_sq = abx * abx + aby * aby;
    if ab_sq < 0.001 {
        return (apx * apx + apy * apy).sqrt();
    }

    let t = ((apx * abx + apy * aby) / ab_sq).clamp(0.0, 1.0);

    let closest_x = ax + t * abx;
    let closest_y = ay + t * aby;

    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}

/// Place structures using the desirability map
pub fn place_structures_from_desirability(
    desirability: &mut DesirabilityMap,
    structure_type: StructureType,
    count: usize,
    surface_z: &Tilemap<i32>,
) -> Vec<PlacedStructure> {
    let mut structures = Vec::new();
    let (min_size, max_size) = structure_type.size_range();
    let avg_size = (min_size + max_size) / 2;

    for _ in 0..count {
        if let Some((x, y, score)) = desirability.find_best() {
            // Skip if no good location found
            if score == f32::MIN {
                break;
            }

            let z = *surface_z.get(x, y);

            let structure = PlacedStructure::new(
                x.saturating_sub(avg_size / 2),
                y.saturating_sub(avg_size / 2),
                z,
                avg_size,
                avg_size,
                structure_type,
            );

            // Mark the area as used
            desirability.mark_used(x, y, avg_size);

            structures.push(structure);
        } else {
            break;
        }
    }

    structures
}
