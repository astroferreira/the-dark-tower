use noise::{NoiseFn, Perlin, Seedable};

use crate::plates::{Plate, PlateId, PlateType};
use crate::tilemap::Tilemap;

// =============================================================================
// TERRAIN PARAMETERS
// =============================================================================

/// Parameters for terrain generation
pub struct TerrainParams {
    /// Base frequency for noise (lower = larger features)
    pub base_frequency: f64,
    /// Number of noise octaves
    pub octaves: u32,
    /// Amplitude decay per octave (0.0-1.0)
    pub persistence: f64,
    /// Frequency multiplier per octave
    pub lacunarity: f64,
    /// Domain warping strength
    pub warp_strength: f64,
    /// Ridge noise power (higher = sharper ridges)
    pub ridge_power: f64,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            base_frequency: 0.008,
            octaves: 6,
            persistence: 0.5,
            lacunarity: 2.0,
            warp_strength: 0.15,  // Reduced from 0.4 to reduce swirly appearance
            ridge_power: 2.0,
        }
    }
}

// =============================================================================
// ELEVATION CONSTANTS
// =============================================================================

// Continental elevations (meters)
const CONTINENTAL_MIN: f32 = 50.0;       // Lowland plains
const CONTINENTAL_MAX: f32 = 400.0;      // Base highland plateaus
const COASTAL_HEIGHT: f32 = 5.0;         // Beach level
const SHELF_DEPTH: f32 = -150.0;         // Continental shelf

// Oceanic elevations
const OCEAN_FLOOR: f32 = -4000.0;        // Deep ocean
const OCEAN_RIDGE: f32 = -2000.0;        // Mid-ocean ridges

// Ridge parameters - now more prominent
const RIDGE_HEIGHT: f32 = 1200.0;        // Procedural ridge height
const RIDGE_FREQUENCY: f64 = 0.012;      // Ridge spacing (lower = larger features)

// Tectonic stress multiplier - boosted for bigger boundary mountains
const TECTONIC_SCALE: f32 = 800.0;

// Volcanic island parameters (oceanic convergence zones)
const VOLCANIC_THRESHOLD: f32 = 0.02;    // Very low threshold to ensure islands appear
const VOLCANIC_BASE: f32 = -500.0;       // Seamount base (underwater)
const VOLCANIC_PEAK: f32 = 800.0;        // Max island peak height
const VOLCANIC_ISLAND_FREQ: f64 = 0.12;  // Island clustering frequency (increased for more islands)

// Coastal fractal parameters
const COAST_FRACTAL_OCTAVES: u32 = 5;
const COAST_FRACTAL_SCALE: f64 = 0.15;

// =============================================================================
// MAIN HEIGHTMAP GENERATION
// =============================================================================

/// Generate a heightmap using layered terrain synthesis:
/// 1. Multi-octave fBm for base terrain variation
/// 2. Domain warping for natural-looking features
/// 3. Procedural ridges for internal mountains
/// 4. Tectonic stress for plate boundary mountains
/// 5. Smooth blending with continental mask
pub fn generate_heightmap(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> Tilemap<f32> {
    let width = plate_map.width;
    let height = plate_map.height;
    let params = TerrainParams::default();
    
    // Initialize noise generators with different seeds for variety
    let terrain_noise = Perlin::new(1).set_seed(seed as u32);
    let warp_noise = Perlin::new(1).set_seed(seed as u32 + 1111);
    let ridge_noise = Perlin::new(1).set_seed(seed as u32 + 2222);
    let detail_noise = Perlin::new(1).set_seed(seed as u32 + 3333);
    let coast_noise = Perlin::new(1).set_seed(seed as u32 + 4444);  // For fractal coastlines
    
    // Pre-compute continental distance field for smooth blending
    let continental_distance = compute_continental_distance(plate_map, plates);
    
    // Pre-compute distance from coast for gradient
    let coast_distance = compute_coast_distance(plate_map, plates);
    
    let mut heightmap = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() {
                heightmap.set(x, y, OCEAN_FLOOR);
                continue;
            }
            
            let plate = &plates[plate_id.0 as usize];
            let stress = *stress_map.get(x, y);
            let cont_dist = *continental_distance.get(x, y);
            let raw_coast_dist = *coast_distance.get(x, y);
            
            // Normalize coordinates for noise sampling
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;
            
            // Apply domain warping for organic shapes
            let (warped_x, warped_y) = apply_domain_warp(
                nx, ny, &warp_noise, params.warp_strength, seed
            );
            
            // Fractal perturbation for coastline - creates jagged edges
            let coast_fractal = fbm(
                &coast_noise, 
                nx * COAST_FRACTAL_SCALE * 100.0, 
                ny * COAST_FRACTAL_SCALE * 100.0, 
                COAST_FRACTAL_OCTAVES, 
                0.6, 
                2.2
            ) as f32;
            
            // Perturb coast distance - larger perturbation near coast
            let coast_perturbation = if raw_coast_dist.abs() < 50.0 {
                coast_fractal * 25.0 * (1.0 - raw_coast_dist.abs() / 50.0)
            } else {
                0.0
            };
            let coast_dist = raw_coast_dist + coast_perturbation;
            
            // Base elevation depends on plate type
            let elevation = match plate.plate_type {
                PlateType::Continental => {
                    generate_continental_elevation(
                        warped_x, warped_y,
                        coast_dist,
                        stress,
                        &terrain_noise,
                        &ridge_noise,
                        &detail_noise,
                        &params,
                        seed,
                    )
                }
                PlateType::Oceanic => {
                    // Use original coordinates for ocean/islands - no domain warping
                    generate_oceanic_elevation(
                        nx, ny,
                        cont_dist,
                        stress,
                        &terrain_noise,
                        &detail_noise,
                        &params,
                        seed,
                    )
                }
            };
            
            heightmap.set(x, y, elevation);
        }
    }
    
    // Apply smoothing pass to reduce harsh transitions
    smooth_heightmap(&heightmap, 2)
}

// =============================================================================
// CONTINENTAL TERRAIN
// =============================================================================

/// Generate elevation for continental plates
fn generate_continental_elevation(
    x: f64,
    y: f64,
    coast_distance: f32,
    stress: f32,
    terrain_noise: &Perlin,
    ridge_noise: &Perlin,
    detail_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
) -> f32 {
    // Underwater continental shelf
    if coast_distance < 0.0 {
        let shelf_blend = (-coast_distance / 50.0).min(1.0);
        let shelf_noise = fbm(terrain_noise, x * 2.0, y * 2.0, 3, 0.5, 2.0) as f32;
        return SHELF_DEPTH * shelf_blend + shelf_noise * 20.0;
    }
    
    // Distance-based gradient (still use for blending, but less restrictive)
    let distance_factor = (coast_distance / 150.0).min(1.0);
    let coastal_gradient = smooth_step(0.0, 1.0, distance_factor);
    
    // Multi-octave fBm for base terrain - always present, not just inland
    let base_fbm = fbm(
        terrain_noise,
        x * params.base_frequency * 80.0,
        y * params.base_frequency * 80.0,
        params.octaves,
        params.persistence,
        params.lacunarity,
    ) as f32;
    
    // Normalize fBm to 0-1 range
    let base_terrain = (base_fbm + 1.0) * 0.5;
    
    // Procedural ridges for internal mountain ranges - NOT limited by coast
    // Use squared ridge for sharper peaks
    let ridge = generate_ridges(x, y, ridge_noise, params.ridge_power) as f32;
    let ridge_squared = ridge * ridge; // Sharper peaks
    // Ridges are present everywhere but slightly higher inland
    let ridge_contribution = ridge_squared * RIDGE_HEIGHT * (0.5 + coastal_gradient * 0.5);
    
    // Fine detail noise for texture
    let detail = fbm(detail_noise, x * 25.0, y * 25.0, 4, 0.6, 2.0) as f32;
    let detail_contribution = detail * 50.0;
    
    // Tectonic stress contribution (mountains at plate boundaries)
    // Add noise modulation for organic, irregular mountain ranges
    let tectonic = if stress > 0.05 {
        // Ridged noise along stress zones for irregular peaks
        let tectonic_ridge = generate_ridges(x * 1.5, y * 1.5, ridge_noise, 1.5) as f32;
        
        // High-frequency detail for individual peak variation
        let peak_variation = detail_noise.get([x * 150.0, y * 150.0, 0.5]) as f32;
        let peak_factor = 0.6 + peak_variation * 0.4; // 0.2 to 1.0 range
        
        // Medium-frequency noise for mountain chain continuity
        let chain_noise = terrain_noise.get([x * 40.0, y * 40.0, 1.0]) as f32;
        let chain_factor = (chain_noise + 1.0) * 0.5; // 0 to 1
        
        // Combine: stress provides envelope, noise creates organic variation
        let base_height = stress.sqrt() * TECTONIC_SCALE;
        let ridge_modulation = 0.3 + tectonic_ridge * 0.7; // 0.3 to 1.0
        let organic_height = base_height * ridge_modulation * peak_factor;
        
        // Add some peaks that exceed the stress envelope for dramatic effect
        let dramatic_peaks = if tectonic_ridge > 0.7 && chain_factor > 0.6 {
            base_height * 0.3 * (tectonic_ridge - 0.7) / 0.3
        } else {
            0.0
        };
        
        organic_height + dramatic_peaks
    } else if stress < -0.05 {
        // Divergent zones create rifts/valleys with irregular depth
        let rift_noise = detail_noise.get([x * 60.0, y * 60.0, 2.0]) as f32;
        let rift_variation = 0.5 + rift_noise * 0.5;
        stress * TECTONIC_SCALE * 0.2 * rift_variation
    } else {
        0.0
    };
    
    // Combine all layers:
    // - Base elevation provides underlying terrain variation (always present)
    // - Coastal gradient mainly affects minimum elevation  
    let min_elevation = COASTAL_HEIGHT + CONTINENTAL_MIN * coastal_gradient;
    let base_variation = base_terrain * CONTINENTAL_MAX * (0.3 + coastal_gradient * 0.7);
    
    min_elevation + base_variation + ridge_contribution + detail_contribution + tectonic
}

/// Generate small coastal islands near continental edges
fn generate_coastal_islands(
    x: f64,
    y: f64,
    coast_distance: f32,
    coast_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
) -> f32 {
    // Island probability increases closer to coast, peaks around -20 distance
    let distance_factor = (-coast_distance - 5.0) / 55.0; // 0.0 at -5, 1.0 at -60
    let proximity_factor = if coast_distance > -30.0 {
        // Peak probability near coast
        1.0 - ((-coast_distance - 15.0).abs() / 15.0).min(1.0)
    } else {
        // Decreasing further out
        1.0 - distance_factor.min(1.0)
    };
    
    // Multi-scale noise for island clusters
    let large_cluster = coast_noise.get([
        x * 120.0,
        y * 120.0,
        seed as f64 * 0.001,
    ]);
    
    let medium_cluster = coast_noise.get([
        x * 250.0 + 5.2,
        y * 250.0 + 3.1,
        seed as f64 * 0.002,
    ]);
    
    let small_peaks = detail_noise.get([
        x * 500.0,
        y * 500.0,
        seed as f64 * 0.003,
    ]);
    
    // Combine scales - larger features guide smaller ones
    let combined = (large_cluster * 0.4 + medium_cluster * 0.35 + small_peaks * 0.25 + 0.5) as f32;
    
    // Threshold for island formation - affected by proximity
    let base_threshold = 0.65;
    let threshold = base_threshold - proximity_factor * 0.12;
    
    if combined < threshold {
        return f32::MIN; // No island - return very low so it doesn't override ocean
    }
    
    // Island height - smaller islands near edge of range
    let peak_factor = ((combined - threshold) / (1.0 - threshold)).min(1.0);
    let max_height = 150.0 * proximity_factor; // Smaller islands further from coast
    
    // Some islands are just rocks, some are proper islands
    let height = 5.0 + peak_factor * max_height;
    
    height
}

// =============================================================================
// OCEANIC TERRAIN
// =============================================================================

/// Generate elevation for oceanic plates
fn generate_oceanic_elevation(
    x: f64,
    y: f64,
    continental_distance: f32,
    stress: f32,
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
) -> f32 {
    // Base ocean floor with variation
    let base_fbm = fbm(
        terrain_noise,
        x * params.base_frequency * 50.0,
        y * params.base_frequency * 50.0,
        4,
        0.5,
        2.0,
    ) as f32;
    
    let variation = base_fbm * 500.0;
    let base = OCEAN_FLOOR + variation;
    
    // Mid-ocean ridges from divergent stress (spreading centers)
    let ridge_contribution = if stress < -0.1 {
        let ridge_strength = (-stress - 0.1).min(1.0);
        (OCEAN_RIDGE - OCEAN_FLOOR) * ridge_strength
    } else {
        0.0
    };
    
    // Calculate base ocean elevation
    let ocean_elevation = base + ridge_contribution;
    
    // Volcanic islands at convergent boundaries (island arcs)
    // These OVERRIDE ocean floor, not add to it
    let volcanic_elevation = if stress > VOLCANIC_THRESHOLD {
        let v = generate_volcanic_islands(x, y, stress, detail_noise, seed);
        v
    } else {
        f32::MIN
    };
    
    // Use the higher of ocean floor or volcanic island
    let final_ocean = ocean_elevation.max(volcanic_elevation);
    
    // Transition zone near continental shelf
    let shelf_blend = if continental_distance < 100.0 {
        let t = continental_distance / 100.0;
        smooth_step(0.0, 1.0, t)
    } else {
        1.0
    };
    
    // Blend from shelf depth to ocean floor
    let shelf_elevation = SHELF_DEPTH + base_fbm * 50.0;
    
    shelf_elevation * (1.0 - shelf_blend) + final_ocean * shelf_blend
}

/// Generate volcanic islands at oceanic convergence zones (island arcs)
/// Creates scattered archipelago-like clusters of small islands, NOT continuous ridges
fn generate_volcanic_islands(
    x: f64,
    y: f64,
    stress: f32,
    noise: &Perlin,
    seed: u64,
) -> f32 {
    // Scale stress to 0-1 range for probability
    let stress_factor = (stress / 0.2).min(1.0);
    
    // High-frequency noise for isolated island spots
    let spot1 = noise.get([x * 500.0, y * 500.0, seed as f64 * 0.001]);
    let spot2 = noise.get([x * 450.0 + 77.0, y * 450.0 + 33.0, seed as f64 * 0.002]);
    
    // Cluster zones - medium frequency  
    let cluster = noise.get([x * 80.0, y * 80.0, seed as f64 * 0.003]);
    let in_cluster = cluster > -0.4; // ~70% of stressed areas can have islands
    
    if !in_cluster {
        return f32::MIN;
    }
    
    // Take max of spots for isolated peaks (not average - creates dots not lines)
    let best_spot = spot1.max(spot2) as f32;
    
    // Higher stress = lower threshold = more islands
    // Lower base threshold for more islands overall
    let threshold = 0.3 - stress_factor * 0.25;
    
    if best_spot < threshold {
        return f32::MIN;
    }
    
    // Island height - scale with how much we exceeded threshold
    let peak_factor = ((best_spot - threshold) / (1.0 - threshold)).min(1.0);
    
    // Chance for larger volcanic islands - the highest peaks become volcanoes
    let is_volcanic = peak_factor > 0.7;
    let base_height = if is_volcanic {
        // Volcanic peaks: 150-400m
        150.0 + (peak_factor - 0.7) / 0.3 * 250.0
    } else {
        // Small islands: 30-150m
        30.0 + peak_factor * 120.0
    };
    
    // Stress bonus for all islands
    let height = base_height + stress_factor * 50.0;
    
    height
}

// =============================================================================
// NOISE FUNCTIONS
// =============================================================================

/// Fractional Brownian Motion - multi-octave noise
fn fbm(
    noise: &Perlin,
    x: f64,
    y: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        total += amplitude * noise.get([x * frequency, y * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    
    total / max_value
}

/// Domain warping - distort coordinates for organic shapes
fn apply_domain_warp(
    x: f64,
    y: f64,
    noise: &Perlin,
    strength: f64,
    seed: u64,
) -> (f64, f64) {
    let warp_scale = 4.0;
    
    // First warp layer
    let warp_x1 = noise.get([x * warp_scale, y * warp_scale]);
    let warp_y1 = noise.get([x * warp_scale + 5.2, y * warp_scale + 1.3]);
    
    // Second warp layer (warp the warp for more organic feel)
    let x2 = x + warp_x1 * strength;
    let y2 = y + warp_y1 * strength;
    
    let warp_x2 = noise.get([x2 * warp_scale * 2.0, y2 * warp_scale * 2.0]);
    let warp_y2 = noise.get([x2 * warp_scale * 2.0 + 3.7, y2 * warp_scale * 2.0 + 8.1]);
    
    (
        x + (warp_x1 + warp_x2 * 0.5) * strength,
        y + (warp_y1 + warp_y2 * 0.5) * strength,
    )
}

/// Generate procedural ridges using ridged noise
fn generate_ridges(x: f64, y: f64, noise: &Perlin, power: f64) -> f64 {
    let freq = RIDGE_FREQUENCY * 100.0;
    
    // Multi-octave ridged noise
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_val = 0.0;
    
    for i in 0..4 {
        let n = noise.get([
            x * freq * frequency,
            y * freq * frequency,
            i as f64 * 0.5,
        ]);
        
        // Ridge function: 1 - |noise| creates ridges at zero crossings
        let ridge = 1.0 - n.abs();
        // Sharpen with power function
        let ridge = ridge.powf(power);
        
        total += amplitude * ridge;
        max_val += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    (total / max_val).max(0.0)
}

/// Smooth step interpolation
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// =============================================================================
// DISTANCE FIELDS
// =============================================================================

/// Compute distance from each cell to nearest continental plate
fn compute_continental_distance(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
) -> Tilemap<f32> {
    use std::collections::VecDeque;
    
    let width = plate_map.width;
    let height = plate_map.height;
    
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    // Initialize with continental plate boundaries
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() {
                continue;
            }
            
            let plate = &plates[plate_id.0 as usize];
            if plate.plate_type == PlateType::Continental {
                // Find cells that border oceanic plates
                let borders_ocean = plate_map.neighbors(x, y).into_iter().any(|(nx, ny)| {
                    let n_id = *plate_map.get(nx, ny);
                    !n_id.is_none() && plates[n_id.0 as usize].plate_type == PlateType::Oceanic
                });
                
                if borders_ocean {
                    distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                }
            }
        }
    }
    
    // BFS to fill distance field
    while let Some((x, y, dist)) = queue.pop_front() {
        for (nx, ny) in plate_map.neighbors(x, y) {
            let new_dist = dist + 1.0;
            if new_dist < *distance.get(nx, ny) {
                distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    distance
}

/// Compute signed distance from coast (positive = land, negative = water)
fn compute_coast_distance(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
) -> Tilemap<f32> {
    use std::collections::VecDeque;
    
    let width = plate_map.width;
    let height = plate_map.height;
    
    // First, identify all continental cells
    let mut is_continental = Tilemap::new_with(width, height, false);
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if !plate_id.is_none() && plates[plate_id.0 as usize].plate_type == PlateType::Continental {
                is_continental.set(x, y, true);
            }
        }
    }
    
    // Find coastal cells: continental cells that border oceanic cells
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    for y in 0..height {
        for x in 0..width {
            if *is_continental.get(x, y) {
                // Check if any neighbor is oceanic (not continental)
                let borders_ocean = plate_map.neighbors(x, y).into_iter().any(|(nx, ny)| {
                    let n_id = *plate_map.get(nx, ny);
                    // Borders ocean if neighbor is oceanic plate (not continental, not none)
                    !n_id.is_none() && plates[n_id.0 as usize].plate_type == PlateType::Oceanic
                });
                
                if borders_ocean {
                    distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                }
            }
        }
    }
    
    // BFS for land cells only - propagate distance from coast
    while let Some((x, y, dist)) = queue.pop_front() {
        for (nx, ny) in plate_map.neighbors(x, y) {
            if !*is_continental.get(nx, ny) {
                continue; // Only propagate within continental
            }
            let new_dist = dist + 1.0;
            if new_dist < *distance.get(nx, ny) {
                distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    // Now compute negative distances for water cells
    let mut water_distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    // Start from same coastal cells but propagate into water
    for y in 0..height {
        for x in 0..width {
            if *is_continental.get(x, y) && *distance.get(x, y) == 0.0 {
                water_distance.set(x, y, 0.0);
                queue.push_back((x, y, 0.0));
            }
        }
    }
    
    while let Some((x, y, dist)) = queue.pop_front() {
        for (nx, ny) in plate_map.neighbors(x, y) {
            if *is_continental.get(nx, ny) {
                continue; // Only propagate into water
            }
            let new_dist = dist + 1.0;
            if new_dist < *water_distance.get(nx, ny) {
                water_distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    // Combine: positive for land, negative for water
    // Note: f32::MAX means cell was not reached by BFS from coast
    // For continental cells, this means very far inland (or isolated from ocean)
    // For water cells, this means very far from any continent
    let mut signed_distance = Tilemap::new_with(width, height, 0.0f32);
    for y in 0..height {
        for x in 0..width {
            if *is_continental.get(x, y) {
                let d = *distance.get(x, y);
                // Unreachable continental = very far inland
                signed_distance.set(x, y, if d == f32::MAX { 200.0 } else { d });
            } else {
                let d = *water_distance.get(x, y);
                signed_distance.set(x, y, if d == f32::MAX { -1000.0 } else { -d });
            }
        }
    }
    
    signed_distance
}

// =============================================================================
// POST-PROCESSING
// =============================================================================

/// Apply smoothing to reduce harsh transitions
fn smooth_heightmap(heightmap: &Tilemap<f32>, radius: usize) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut result = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            
            for dy in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist <= radius as f32 {
                        let weight = 1.0 - dist / (radius as f32 + 1.0);
                        sum += *heightmap.get(nx, ny) * weight;
                        count += weight;
                    }
                }
            }
            
            result.set(x, y, sum / count);
        }
    }
    
    result
}

/// Normalize heightmap values to 0.0-1.0 range.
pub fn normalize_heightmap(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;

    for (_, _, &val) in heightmap.iter() {
        if val < min_val {
            min_val = val;
        }
        if val > max_val {
            max_val = val;
        }
    }

    let range = max_val - min_val;
    if range < 0.0001 {
        return heightmap.clone();
    }

    let mut normalized = Tilemap::new_with(heightmap.width, heightmap.height, 0.0);
    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let val = *heightmap.get(x, y);
            normalized.set(x, y, (val - min_val) / range);
        }
    }

    normalized
}

/// Generate land mask for continental plates (for compatibility with existing code)
pub fn generate_land_mask(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    _seed: u64,
) -> Tilemap<bool> {
    let width = plate_map.width;
    let height = plate_map.height;
    let mut land_mask = Tilemap::new_with(width, height, false);
    
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if !plate_id.is_none() && plates[plate_id.0 as usize].plate_type == PlateType::Continental {
                land_mask.set(x, y, true);
            }
        }
    }
    
    land_mask
}
