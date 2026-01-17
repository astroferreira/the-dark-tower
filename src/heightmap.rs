use noise::{NoiseFn, Perlin, Seedable};

use crate::plates::{Plate, PlateId, PlateType};
use crate::scale::{MapScale, scale_distance, scale_frequency, scale_elevation};
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
const CONTINENTAL_MAX: f32 = 600.0;      // Base highland plateaus (increased)
const COASTAL_HEIGHT: f32 = 5.0;         // Beach level
const SHELF_DEPTH: f32 = -150.0;         // Continental shelf

// Oceanic elevations
const OCEAN_FLOOR: f32 = -5000.0;        // Deep ocean baseline (was -4000)
const OCEAN_RIDGE: f32 = -2500.0;        // Mid-ocean ridges (was -2000)
const TRENCH_SCALE: f32 = 4000.0;        // Additional depth for oceanic trenches

// Ridge parameters - prominent mountain ranges
const RIDGE_HEIGHT: f32 = 2500.0;        // Procedural ridge height (increased for real mountains)
const RIDGE_FREQUENCY: f64 = 0.012;      // Ridge spacing (lower = larger features)

// Tectonic stress multiplier - dramatic boundary mountains
const TECTONIC_SCALE: f32 = 2000.0;      // Increased for proper mountain ranges

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
    generate_heightmap_scaled(plate_map, plates, stress_map, seed, &MapScale::default())
}

/// Generate heightmap with explicit scale parameter
pub fn generate_heightmap_scaled(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    seed: u64,
    map_scale: &MapScale,
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
            
            // Scale noise frequencies and distances based on map scale
            let coast_fractal_freq = scale_frequency(COAST_FRACTAL_SCALE * 100.0, map_scale);
            let coast_perturb_range = scale_distance(50.0, map_scale);
            let coast_perturb_mag = scale_distance(25.0, map_scale);

            // Fractal perturbation for coastline - creates jagged edges
            let coast_fractal = fbm(
                &coast_noise,
                nx * coast_fractal_freq,
                ny * coast_fractal_freq,
                COAST_FRACTAL_OCTAVES,
                0.6,
                2.2
            ) as f32;

            // Perturb coast distance - larger perturbation near coast
            let coast_perturbation = if raw_coast_dist.abs() < coast_perturb_range {
                coast_fractal * coast_perturb_mag * (1.0 - raw_coast_dist.abs() / coast_perturb_range)
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
                        map_scale,
                    )
                }
                PlateType::Oceanic => {
                    // Compute stress gradient for island arc alignment
                    // This tells us the boundary direction for curving island chains
                    let stress_dx = if x > 0 && x < width - 1 {
                        *stress_map.get(x + 1, y) - *stress_map.get(x - 1, y)
                    } else { 0.0 };
                    let stress_dy = if y > 0 && y < height - 1 {
                        *stress_map.get(x, y + 1) - *stress_map.get(x, y - 1)
                    } else { 0.0 };
                    let stress_gradient = (stress_dx, stress_dy);

                    // Use original coordinates for ocean/islands - no domain warping
                    generate_oceanic_elevation(
                        nx, ny,
                        cont_dist,
                        stress,
                        stress_gradient,
                        &terrain_noise,
                        &detail_noise,
                        &params,
                        seed,
                        map_scale,
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
    map_scale: &MapScale,
) -> f32 {
    // Scale distance and elevation thresholds
    let shelf_blend_dist = scale_distance(50.0, map_scale);
    let coastal_grad_dist = scale_distance(150.0, map_scale);
    let ridge_height = scale_elevation(RIDGE_HEIGHT, map_scale);
    let detail_height = scale_elevation(50.0, map_scale);
    let tectonic_scale = scale_elevation(TECTONIC_SCALE, map_scale);
    let detail_freq = scale_frequency(25.0, map_scale);

    // Underwater continental shelf - with barrier island check
    if coast_distance < 0.0 {
        // Check for barrier islands first (they rise above sea level)
        let barrier_island = generate_barrier_islands(
            x, y, coast_distance, terrain_noise, detail_noise, seed, map_scale
        );

        if barrier_island > 0.0 {
            // Barrier island rises above sea level
            return barrier_island;
        }

        // Normal shelf depth
        let shelf_blend = (-coast_distance / shelf_blend_dist).min(1.0);
        let shelf_noise = fbm(terrain_noise, x * 2.0, y * 2.0, 3, 0.5, 2.0) as f32;
        return SHELF_DEPTH * shelf_blend + shelf_noise * scale_elevation(20.0, map_scale);
    }

    // Distance-based gradient (still use for blending, but less restrictive)
    let distance_factor = (coast_distance / coastal_grad_dist).min(1.0);
    let coastal_gradient = smooth_step(0.0, 1.0, distance_factor);

    // Scale base frequency for terrain
    let base_freq = scale_frequency(params.base_frequency * 80.0, map_scale);

    // Multi-octave fBm for base terrain - always present, not just inland
    let base_fbm = fbm(
        terrain_noise,
        x * base_freq,
        y * base_freq,
        params.octaves,
        params.persistence,
        params.lacunarity,
    ) as f32;

    // Normalize fBm to 0-1 range
    let base_terrain = (base_fbm + 1.0) * 0.5;

    // Procedural ridges for internal mountain ranges - NOT limited by coast
    // Use squared ridge for sharper peaks
    let ridge = generate_ridges_scaled(x, y, ridge_noise, params.ridge_power, map_scale) as f32;
    let ridge_squared = ridge * ridge; // Sharper peaks
    // Ridges are present everywhere but slightly higher inland
    let ridge_contribution = ridge_squared * ridge_height * (0.5 + coastal_gradient * 0.5);

    // Fine detail noise for texture
    let detail = fbm(detail_noise, x * detail_freq, y * detail_freq, 4, 0.6, 2.0) as f32;
    let detail_contribution = detail * detail_height;
    
    // Scale frequencies for tectonic noise
    let peak_freq = scale_frequency(150.0, map_scale);
    let chain_freq = scale_frequency(40.0, map_scale);
    let rift_freq = scale_frequency(60.0, map_scale);

    // Tectonic stress contribution (mountains at plate boundaries)
    // Add noise modulation for organic, irregular mountain ranges
    let tectonic = if stress > 0.05 {
        // Ridged noise along stress zones for irregular peaks
        let tectonic_ridge = generate_ridges_scaled(x * 1.5, y * 1.5, ridge_noise, 1.5, map_scale) as f32;

        // High-frequency detail for individual peak variation
        let peak_variation = detail_noise.get([x * peak_freq, y * peak_freq, 0.5]) as f32;
        let peak_factor = 0.6 + peak_variation * 0.4; // 0.2 to 1.0 range

        // Medium-frequency noise for mountain chain continuity
        let chain_noise = terrain_noise.get([x * chain_freq, y * chain_freq, 1.0]) as f32;
        let chain_factor = (chain_noise + 1.0) * 0.5; // 0 to 1

        // Combine: stress provides envelope, noise creates organic variation
        let base_height = stress.sqrt() * tectonic_scale;
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
        // Enhanced rift valleys at divergent continental boundaries
        // Creates deep, linear depressions like the East African Rift
        let rift_strength = (-stress - 0.05).min(0.5);

        // Low-frequency noise for linear rift coherence (elongated pattern)
        let rift_linear = terrain_noise.get([x * 0.03, y * 0.03, 2.0]) as f32;

        // High-frequency detail for rift floor variation
        let rift_detail = detail_noise.get([x * rift_freq, y * rift_freq, 2.5]) as f32;

        // Deeper rifts (0.6 scale vs old 0.2) with linear pattern
        let rift_depth = rift_strength * tectonic_scale * 0.6;
        let rift_floor = 0.7 + rift_linear * 0.3; // 70-100% of full depth

        // Final rift elevation (negative = depression)
        -rift_depth * rift_floor * (0.8 + rift_detail * 0.2)
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
    stress_gradient: (f32, f32),  // Gradient for island arc alignment
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Scale parameters
    let ocean_freq = scale_frequency(params.base_frequency * 50.0, map_scale);
    let shelf_blend_dist = scale_distance(15.0, map_scale);  // Reduced from 100 - shelf transition is narrow
    let ocean_variation = scale_elevation(1500.0, map_scale);  // Increased from 500 for more depth variety
    let shelf_noise_height = scale_elevation(50.0, map_scale);

    // Base ocean floor with variation
    let base_fbm = fbm(
        terrain_noise,
        x * ocean_freq,
        y * ocean_freq,
        4,
        0.5,
        2.0,
    ) as f32;

    let variation = base_fbm * ocean_variation;
    let base = OCEAN_FLOOR + variation;

    // Mid-ocean ridges with visible linear structure (spreading centers)
    // Real ridges have: elevated terrain, parallel ridge peaks, and central axial valley
    let ridge_contribution = if stress < -0.1 {
        let ridge_strength = (-stress - 0.1).min(1.0);
        let base_lift = (OCEAN_RIDGE - OCEAN_FLOOR) * ridge_strength;

        // Linear ridge texture - creates parallel peaks perpendicular to spreading
        let ridge_texture = terrain_noise.get([x * 0.08, y * 0.08, 5.0]) as f32;
        let ridge_peaks = (ridge_texture * std::f32::consts::PI).sin().abs();

        // Central axial rift valley along ridge axis (characteristic of mid-ocean ridges)
        let axial_noise = detail_noise.get([x * 0.15, y * 0.15, 6.0]) as f32;
        let axial_valley = if axial_noise.abs() < 0.15 { 200.0 } else { 0.0 };

        // Combine: base elevation lift + ridge peaks - central valley
        base_lift + ridge_peaks * 300.0 * ridge_strength - axial_valley * ridge_strength
    } else {
        0.0
    };

    // Oceanic trenches at convergent boundaries (subduction zones)
    // High positive stress in ocean = deep trenches (like Mariana, Puerto Rico)
    let trench_contribution = if stress > 0.25 {
        let trench_strength = ((stress - 0.25) / 0.5).min(1.0);
        -trench_strength * TRENCH_SCALE  // Negative = deeper
    } else {
        0.0
    };

    // Calculate base ocean elevation
    let ocean_elevation = base + ridge_contribution + trench_contribution;

    // Island arcs at convergent boundaries (subduction zones)
    // Creates curving volcanic chains parallel to trenches (like Japan, Aleutians, Caribbean)
    let volcanic_elevation = if stress > VOLCANIC_THRESHOLD {
        let v = generate_island_arc(
            x, y, stress, stress_gradient, terrain_noise, detail_noise, seed, map_scale
        );
        v
    } else {
        f32::MIN
    };

    // Use the higher of ocean floor or volcanic island
    let final_ocean = ocean_elevation.max(volcanic_elevation);

    // Transition zone near continental shelf
    let shelf_blend = if continental_distance < shelf_blend_dist {
        let t = continental_distance / shelf_blend_dist;
        smooth_step(0.0, 1.0, t)
    } else {
        1.0
    };

    // Blend from shelf depth to ocean floor
    let shelf_elevation = SHELF_DEPTH + base_fbm * shelf_noise_height;

    shelf_elevation * (1.0 - shelf_blend) + final_ocean * shelf_blend
}

/// Generate island arc chains parallel to subduction trenches
/// Creates curving volcanic chains like Japan, Aleutians, Caribbean island arcs
fn generate_island_arc(
    x: f64,
    y: f64,
    stress: f32,
    stress_gradient: (f32, f32),
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Only generate in convergent zones with significant stress
    if stress < 0.08 { return f32::MIN; }

    let stress_factor = (stress / 0.3).min(1.0);

    // Calculate boundary tangent (perpendicular to stress gradient)
    // This gives us the direction along which the island arc curves
    let grad_mag = (stress_gradient.0 * stress_gradient.0 + stress_gradient.1 * stress_gradient.1).sqrt();

    // If gradient is too weak, fall back to scattered generation
    if grad_mag < 0.01 {
        return generate_volcanic_islands_scaled(x, y, stress, detail_noise, seed, map_scale);
    }

    // Boundary tangent (perpendicular to gradient = along the boundary)
    let tangent = (-stress_gradient.1 / grad_mag, stress_gradient.0 / grad_mag);

    // Create arc-aligned coordinate system
    // u = distance along the arc, v = distance from the arc center
    let u = x * tangent.0 as f64 + y * tangent.1 as f64;
    let v = x * (-tangent.1) as f64 + y * tangent.0 as f64;

    // Scale frequencies for map scale
    let arc_freq = scale_frequency(0.15, map_scale);
    let spacing_freq = scale_frequency(0.4, map_scale);
    let detail_freq = scale_frequency(200.0, map_scale);

    // Island placement along the arc (creates chain pattern)
    // Use sine wave along tangent direction for regular spacing
    let arc_position = terrain_noise.get([u * arc_freq, v * 0.02, seed as f64 * 0.004]) as f32;

    // Island spacing along the arc (~50-100km apart in chain)
    let chain_pattern = (u * spacing_freq + arc_position as f64 * 0.5).sin() as f32;
    let is_in_chain = chain_pattern > 0.3;  // Creates discrete island spots along arc

    // Width of the island arc band (narrower = more linear chain)
    let arc_width_noise = detail_noise.get([x * 0.08, y * 0.08, seed as f64 * 0.005]) as f32;
    let arc_band = 0.15 + arc_width_noise * 0.05;  // Narrow band for arc

    // Check if we're in the arc band
    let distance_from_center = (terrain_noise.get([v * 0.1, u * 0.02, seed as f64 * 0.006]) as f32).abs();
    let in_arc_band = distance_from_center < arc_band;

    if !is_in_chain || !in_arc_band {
        return f32::MIN;
    }

    // High-frequency detail for island peaks
    let peak_noise = detail_noise.get([x * detail_freq, y * detail_freq, seed as f64 * 0.007]) as f32;
    let is_peak = peak_noise > 0.2;

    if !is_peak {
        return f32::MIN;
    }

    // Scale island heights
    let volcanic_base = scale_elevation(120.0, map_scale);
    let volcanic_extra = scale_elevation(400.0, map_scale);
    let stress_bonus = scale_elevation(80.0, map_scale);

    // Island height based on peak quality and stress
    let peak_factor = ((peak_noise - 0.2) / 0.8).min(1.0);
    let base_height = volcanic_base + peak_factor * volcanic_extra;
    let height = base_height + stress_factor * stress_bonus;

    height
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

/// Generate volcanic islands with explicit scale parameter
fn generate_volcanic_islands_scaled(
    x: f64,
    y: f64,
    stress: f32,
    noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Scale frequencies
    let spot_freq1 = scale_frequency(500.0, map_scale);
    let spot_freq2 = scale_frequency(450.0, map_scale);
    let cluster_freq = scale_frequency(80.0, map_scale);

    // Scale stress to 0-1 range for probability
    let stress_factor = (stress / 0.2).min(1.0);

    // High-frequency noise for isolated island spots
    let spot1 = noise.get([x * spot_freq1, y * spot_freq1, seed as f64 * 0.001]);
    let spot2 = noise.get([x * spot_freq2 + 77.0, y * spot_freq2 + 33.0, seed as f64 * 0.002]);

    // Cluster zones - medium frequency
    let cluster = noise.get([x * cluster_freq, y * cluster_freq, seed as f64 * 0.003]);
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

    // Scale island heights
    let volcanic_base = scale_elevation(150.0, map_scale);
    let volcanic_extra = scale_elevation(250.0, map_scale);
    let small_base = scale_elevation(30.0, map_scale);
    let small_extra = scale_elevation(120.0, map_scale);
    let stress_bonus = scale_elevation(50.0, map_scale);

    // Chance for larger volcanic islands - the highest peaks become volcanoes
    let is_volcanic = peak_factor > 0.7;
    let base_height = if is_volcanic {
        // Volcanic peaks
        volcanic_base + (peak_factor - 0.7) / 0.3 * volcanic_extra
    } else {
        // Small islands
        small_base + peak_factor * small_extra
    };

    // Stress bonus for all islands
    let height = base_height + stress_factor * stress_bonus;

    height
}

// =============================================================================
// BARRIER ISLANDS
// =============================================================================

/// Generate barrier islands parallel to coastlines
/// Creates long, thin sandy islands that run parallel to the coast (like the Outer Banks, Texas coast)
/// These form in shallow water and create protected lagoons behind them
fn generate_barrier_islands(
    x: f64,
    y: f64,
    coast_distance: f32,  // Negative = water, positive = land
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Barrier islands form offshore, in the shallow water zone
    // coast_distance is negative for water, so -5 to -35 range
    let min_offshore = scale_distance(-5.0, map_scale);   // Not too close to shore
    let max_offshore = scale_distance(-40.0, map_scale);  // Not too far out

    // Only in the right distance range
    if coast_distance > min_offshore || coast_distance < max_offshore {
        return f32::MIN;
    }

    // Optimal formation zone is 15-25 units offshore
    let optimal_dist = scale_distance(-20.0, map_scale);
    let dist_from_optimal = (coast_distance - optimal_dist).abs();
    let dist_factor = 1.0 - (dist_from_optimal / scale_distance(18.0, map_scale)).min(1.0);

    if dist_factor < 0.2 {
        return f32::MIN;
    }

    // Elongated pattern: low frequency parallel to coast (long axis), high freq perpendicular (narrow)
    // Using different frequency scales for the two axes creates elongated shapes
    let parallel_freq = scale_frequency(0.015, map_scale);  // Long axis - low freq = long features
    let perp_freq = scale_frequency(0.12, map_scale);       // Short axis - high freq = narrow

    // Sample noise at both frequencies
    let parallel_noise = terrain_noise.get([x * parallel_freq, y * parallel_freq, seed as f64 + 7.0]) as f32;
    let perp_noise = detail_noise.get([x * perp_freq, y * perp_freq, seed as f64 + 8.0]) as f32;

    // Combine: weight heavily toward parallel (elongated) pattern
    // The perpendicular noise creates breaks in the chain (inlets)
    let island_pattern = parallel_noise * 0.8 + perp_noise * 0.2;

    // Threshold for island formation
    if island_pattern < 0.25 {
        return f32::MIN;
    }

    // Island height: barrier islands are low and sandy (3-12m above sea level)
    let pattern_strength = (island_pattern - 0.25) / 0.75;  // 0-1 normalized
    let base_height = scale_elevation(3.0, map_scale);
    let max_extra = scale_elevation(9.0, map_scale);

    let height = base_height + pattern_strength * max_extra * dist_factor;

    // Add small-scale dune detail
    let dune_freq = scale_frequency(0.5, map_scale);
    let dune_noise = detail_noise.get([x * dune_freq, y * dune_freq, seed as f64 + 9.0]) as f32;
    let dune_height = scale_elevation(2.0, map_scale) * dune_noise.abs();

    height + dune_height
}

// =============================================================================
// KARST TERRAIN GENERATION
// =============================================================================

/// Calculate karst potential based on conditions
/// Returns 0.0-1.0 indicating likelihood of karst formation
/// Karst forms in wet areas with limestone bedrock (simulated via noise)
pub fn calculate_karst_potential(
    x: f64,
    y: f64,
    elevation: f32,
    moisture: f32,
    temperature: f32,
    limestone_noise: &Perlin,
    map_scale: &MapScale,
) -> f32 {
    // Must be on land
    if elevation <= 0.0 {
        return 0.0;
    }

    // Limestone presence (noise-based "geology")
    let limestone_freq = scale_frequency(0.03, map_scale);
    let limestone = limestone_noise.get([x * limestone_freq, y * limestone_freq, 3.14]) as f32;
    let has_limestone = limestone > 0.1;  // ~45% of land can have limestone

    if !has_limestone {
        return 0.0;
    }

    // Moisture factor - karst needs water for dissolution
    let moisture_factor = if moisture > 0.3 {
        ((moisture - 0.3) / 0.5).min(1.0)
    } else {
        0.0
    };

    // Temperature factor - dissolution works better in warm climates
    let temp_factor = if temperature > 5.0 {
        ((temperature - 5.0) / 20.0).min(1.0)
    } else {
        0.2  // Some karst even in cold climates
    };

    // Elevation factor - karst most common at low-moderate elevations
    let elev_factor = if elevation < 800.0 {
        1.0 - (elevation / 1200.0)
    } else {
        0.2
    };

    // Combine factors
    let limestone_strength = (limestone - 0.1) / 0.9;  // 0-1 for limestone presence
    limestone_strength * moisture_factor * temp_factor * elev_factor
}

/// Generate sinkhole/doline features - circular depressions
/// Returns negative value for depression depth
pub fn generate_sinkhole_terrain(
    x: f64,
    y: f64,
    karst_potential: f32,
    sinkhole_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    if karst_potential < 0.2 {
        return 0.0;
    }

    // High-frequency noise for sinkhole placement
    let spot_freq = scale_frequency(0.4, map_scale);
    let spot_noise = sinkhole_noise.get([x * spot_freq, y * spot_freq, seed as f64 + 20.0]) as f32;

    // Only create sinkholes at local maxima of noise (isolated spots)
    let threshold = 0.6 - karst_potential * 0.2;  // Higher karst = more sinkholes
    if spot_noise < threshold {
        return 0.0;
    }

    // Sinkhole depth based on how much it exceeds threshold
    let strength = (spot_noise - threshold) / (1.0 - threshold);
    let base_depth = scale_elevation(15.0, map_scale);  // 15m base depth
    let max_extra = scale_elevation(35.0, map_scale);   // Up to 50m total

    // Add variation
    let detail = detail_noise.get([x * spot_freq * 3.0, y * spot_freq * 3.0, seed as f64 + 21.0]) as f32;

    // Return negative value (depression)
    -(base_depth + strength * max_extra) * (0.7 + detail.abs() * 0.3) * karst_potential
}

/// Generate tower karst terrain - tall limestone pillars
/// Returns positive value for tower height
pub fn generate_tower_karst_terrain(
    x: f64,
    y: f64,
    karst_potential: f32,
    temperature: f32,
    tower_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Tower karst only in tropical climates with high karst potential
    if karst_potential < 0.4 || temperature < 18.0 {
        return 0.0;
    }

    let tropical_factor = ((temperature - 18.0) / 12.0).min(1.0);

    // Tower placement - creates isolated pillars
    let tower_freq = scale_frequency(0.25, map_scale);
    let tower_base = tower_noise.get([x * tower_freq, y * tower_freq, seed as f64 + 30.0]) as f32;

    // Secondary frequency for grouping towers
    let group_freq = scale_frequency(0.08, map_scale);
    let group_noise = tower_noise.get([x * group_freq, y * group_freq, seed as f64 + 31.0]) as f32;
    let in_tower_zone = group_noise > 0.0;

    if !in_tower_zone {
        return 0.0;
    }

    // Create isolated tower peaks
    let threshold = 0.55;
    if tower_base < threshold {
        return 0.0;
    }

    let strength = (tower_base - threshold) / (1.0 - threshold);

    // Tower heights - dramatic pillars
    let base_height = scale_elevation(50.0, map_scale);   // 50m base
    let max_extra = scale_elevation(150.0, map_scale);    // Up to 200m

    // Add detail for varied tower shapes
    let detail_freq = scale_frequency(0.8, map_scale);
    let detail = detail_noise.get([x * detail_freq, y * detail_freq, seed as f64 + 32.0]) as f32;

    (base_height + strength * max_extra) * karst_potential * tropical_factor * (0.8 + detail.abs() * 0.2)
}

/// Generate karst surface roughness - small-scale dissolution features
pub fn generate_karst_surface(
    x: f64,
    y: f64,
    karst_potential: f32,
    surface_noise: &Perlin,
    map_scale: &MapScale,
) -> f32 {
    if karst_potential < 0.1 {
        return 0.0;
    }

    // High-frequency roughness (karren, rillenkarren)
    let rough_freq = scale_frequency(1.5, map_scale);
    let roughness = surface_noise.get([x * rough_freq, y * rough_freq, 40.0]) as f32;

    // Scale roughness by karst potential
    let amplitude = scale_elevation(5.0, map_scale);  // Up to 5m surface variation
    roughness * amplitude * karst_potential * 0.5
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

/// Generate procedural ridges with explicit scale parameter
fn generate_ridges_scaled(x: f64, y: f64, noise: &Perlin, power: f64, map_scale: &MapScale) -> f64 {
    let freq = scale_frequency(RIDGE_FREQUENCY * 100.0, map_scale);

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
                let borders_ocean = plate_map.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
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
        for (nx, ny) in plate_map.neighbors_8(x, y) {
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
                let borders_ocean = plate_map.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
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
        for (nx, ny) in plate_map.neighbors_8(x, y) {
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
        for (nx, ny) in plate_map.neighbors_8(x, y) {
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

/// Print a histogram of height values for debugging.
/// Shows distribution across bins and key statistics.
pub fn print_height_histogram(heightmap: &Tilemap<f32>, num_bins: usize) {
    let num_bins = num_bins.max(5).min(50);

    // Collect all heights and compute statistics
    let mut heights: Vec<f32> = Vec::with_capacity(heightmap.width * heightmap.height);
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    let mut sum = 0.0f64;

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let h = *heightmap.get(x, y);
            heights.push(h);
            min_h = min_h.min(h);
            max_h = max_h.max(h);
            sum += h as f64;
        }
    }

    let count = heights.len();
    let mean = sum / count as f64;

    // Compute standard deviation
    let variance: f64 = heights.iter()
        .map(|h| {
            let diff = *h as f64 - mean;
            diff * diff
        })
        .sum::<f64>() / count as f64;
    let std_dev = variance.sqrt();

    // Compute median
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if count % 2 == 0 {
        (heights[count / 2 - 1] + heights[count / 2]) / 2.0
    } else {
        heights[count / 2]
    };

    // Count above/below sea level
    let above_sea = heights.iter().filter(|h| **h >= 0.0).count();
    let below_sea = count - above_sea;

    // Create bins
    let range = max_h - min_h;
    let bin_width = range / num_bins as f32;
    let mut bins = vec![0usize; num_bins];

    for h in &heights {
        let bin_idx = ((*h - min_h) / bin_width) as usize;
        let bin_idx = bin_idx.min(num_bins - 1);
        bins[bin_idx] += 1;
    }

    // Find max bin for scaling
    let max_bin = *bins.iter().max().unwrap_or(&1);
    let bar_max_width = 50;

    // Print header
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                     HEIGHT DISTRIBUTION HISTOGRAM                     ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");

    // Print statistics
    println!("║ Statistics:                                                          ║");
    println!("║   Min: {:>10.2}m    Max: {:>10.2}m    Range: {:>10.2}m           ║", min_h, max_h, range);
    println!("║   Mean: {:>9.2}m    Median: {:>8.2}m    Std Dev: {:>8.2}m          ║", mean, median, std_dev);
    println!("║   Above sea level: {:>6} ({:>5.1}%)    Below: {:>6} ({:>5.1}%)       ║",
        above_sea, 100.0 * above_sea as f64 / count as f64,
        below_sea, 100.0 * below_sea as f64 / count as f64);
    println!("╠══════════════════════════════════════════════════════════════════════╣");

    // Print histogram
    for (i, &bin_count) in bins.iter().enumerate() {
        let bin_start = min_h + i as f32 * bin_width;
        let bin_end = bin_start + bin_width;
        let bar_len = (bin_count as f64 / max_bin as f64 * bar_max_width as f64) as usize;
        let bar = "█".repeat(bar_len);
        let pct = 100.0 * bin_count as f64 / count as f64;

        // Mark sea level bin
        let marker = if bin_start <= 0.0 && bin_end > 0.0 { "◄SEA" } else { "    " };

        println!("║ {:>7.0} - {:>6.0}m │{:<50}│{:>5.1}% {} ║",
            bin_start, bin_end, bar, pct, marker);
    }

    println!("╚══════════════════════════════════════════════════════════════════════╝\n");
}
