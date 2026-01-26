//! Climate system for generating temperature and moisture maps
//! Based on latitude, elevation, and ocean proximity

use noise::{NoiseFn, Perlin, Seedable};
use rayon::prelude::*;
use crate::tilemap::Tilemap;
use crate::scale::{MapScale, scale_distance, scale_elevation};

// =============================================================================
// CLIMATE CONFIGURATION
// =============================================================================

/// Climate simulation mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClimateMode {
    /// Globe mode: Temperature varies by latitude (poles cold, equator hot)
    /// Realistic for planetary-scale maps
    #[default]
    Globe,
    /// Flat mode: Uniform base temperature across map
    /// Temperature only varies by elevation
    /// Good for regional/continental maps
    Flat,
    /// Temperate band: Simulates a mid-latitude region
    /// Moderate temperatures throughout
    TemperateBand,
    /// Tropical band: Simulates an equatorial region
    /// Warm temperatures throughout
    TropicalBand,
}

impl ClimateMode {
    pub fn all() -> &'static [Self] {
        &[Self::Globe, Self::Flat, Self::TemperateBand, Self::TropicalBand]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Globe => "Latitude-based (poles to equator)",
            Self::Flat => "Uniform temperature (elevation only)",
            Self::TemperateBand => "Mid-latitude region (temperate)",
            Self::TropicalBand => "Equatorial region (tropical)",
        }
    }
}

impl std::fmt::Display for ClimateMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Globe => write!(f, "globe"),
            Self::Flat => write!(f, "flat"),
            Self::TemperateBand => write!(f, "temperate"),
            Self::TropicalBand => write!(f, "tropical"),
        }
    }
}

/// Rainfall/moisture level preset
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RainfallLevel {
    /// Arid: Deserts dominate, very little moisture
    Arid,
    /// Normal: Earth-like moisture distribution
    #[default]
    Normal,
    /// Wet: More rainfall, larger forests
    Wet,
    /// Tropical: High moisture everywhere
    Tropical,
}

impl RainfallLevel {
    pub fn all() -> &'static [Self] {
        &[Self::Arid, Self::Normal, Self::Wet, Self::Tropical]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Arid => "Desert world (~20% moisture)",
            Self::Normal => "Earth-like moisture distribution",
            Self::Wet => "Rainy world (~60% moisture)",
            Self::Tropical => "Jungle world (~80% moisture)",
        }
    }

    /// Base moisture multiplier for this level
    pub fn moisture_multiplier(&self) -> f32 {
        match self {
            Self::Arid => 0.4,
            Self::Normal => 1.0,
            Self::Wet => 1.5,
            Self::Tropical => 2.0,
        }
    }

    /// Minimum moisture floor
    pub fn moisture_floor(&self) -> f32 {
        match self {
            Self::Arid => 0.01,
            Self::Normal => 0.02,
            Self::Wet => 0.15,
            Self::Tropical => 0.35,
        }
    }
}

impl std::fmt::Display for RainfallLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arid => write!(f, "arid"),
            Self::Normal => write!(f, "normal"),
            Self::Wet => write!(f, "wet"),
            Self::Tropical => write!(f, "tropical"),
        }
    }
}

/// Combined climate configuration
#[derive(Clone, Copy, Debug, Default)]
pub struct ClimateConfig {
    pub mode: ClimateMode,
    pub rainfall: RainfallLevel,
}

// =============================================================================
// CLIMATE PARAMETERS
// =============================================================================

/// Temperature at equator at sea level (Celsius)
const EQUATOR_TEMP: f32 = 30.0;

/// Temperature at poles at sea level (Celsius)
const POLE_TEMP: f32 = -30.0;

/// Temperature drop per 1000m elevation (lapse rate)
const ELEVATION_LAPSE_RATE: f32 = 6.5;

/// Ocean temperature moderation factor (0-1)
const OCEAN_MODERATION: f32 = 0.3;

// =============================================================================
// TEMPERATURE GENERATION
// =============================================================================

/// Generate temperature map based on latitude and elevation
/// Returns temperature in Celsius
pub fn generate_temperature(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
) -> Tilemap<f32> {
    generate_temperature_with_seed(heightmap, width, height, ClimateMode::Globe, 0)
}

/// Generate temperature map with configurable climate mode
pub fn generate_temperature_with_config(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    mode: ClimateMode,
) -> Tilemap<f32> {
    generate_temperature_with_seed(heightmap, width, height, mode, 0)
}

/// Generate temperature map with domain warping to break horizontal bands
/// Parallelized with rayon for improved performance.
pub fn generate_temperature_with_seed(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    mode: ClimateMode,
    seed: u64,
) -> Tilemap<f32> {
    // Domain warping noise - distorts latitude lines for organic climate zones
    let warp_noise = Perlin::new(1).set_seed(seed as u32);
    let detail_noise = Perlin::new(1).set_seed(seed as u32 + 100);

    // Warp strength: how much the latitude lines deviate (in normalized units)
    // 0.15 means up to ~15% of map height deviation
    const WARP_STRENGTH: f32 = 0.12;
    const WARP_SCALE: f64 = 3.0;  // Frequency of the warping (lower = larger blobs)

    // Compute temperature values in parallel by rows
    let rows: Vec<Vec<f32>> = (0..height)
        .into_par_iter()
        .map(|y| {
            let ny = y as f64 / height as f64;
            (0..width).map(|x| {
                let elevation = *heightmap.get(x, y);
                let nx = x as f64 / width as f64;

                // Base temperature depends on climate mode
                let base_temp = match mode {
                    ClimateMode::Globe => {
                        // DOMAIN WARPING: Distort the Y-coordinate using noise
                        // This creates wavy, organic climate zone boundaries
                        let warp = warp_noise.get([nx * WARP_SCALE, ny * WARP_SCALE]) as f32;
                        let detail = detail_noise.get([nx * WARP_SCALE * 2.0, ny * WARP_SCALE * 2.0]) as f32 * 0.3;
                        let total_warp = (warp + detail) * WARP_STRENGTH;

                        // Warped Y position for latitude calculation
                        let warped_y = (y as f32 / height as f32) + total_warp;

                        // Latitude factor: 0 at equator, 1 at poles
                        let latitude_normalized = (warped_y - 0.5).abs().clamp(0.0, 0.5) * 2.0;
                        let lat_factor = latitude_normalized.powf(1.5);
                        EQUATOR_TEMP - (EQUATOR_TEMP - POLE_TEMP) * lat_factor
                    }
                    ClimateMode::Flat => {
                        // Uniform base temperature (mild temperate)
                        15.0
                    }
                    ClimateMode::TemperateBand => {
                        // Simulates ~45° latitude band with slight variation
                        let warp = warp_noise.get([nx * WARP_SCALE, ny * WARP_SCALE]) as f32 * 0.05;
                        let y_factor = ((y as f32 / height as f32 - 0.5) + warp).abs();
                        12.0 + y_factor * 8.0
                    }
                    ClimateMode::TropicalBand => {
                        // Simulates equatorial region with minimal variation
                        let warp = warp_noise.get([nx * WARP_SCALE, ny * WARP_SCALE]) as f32 * 0.05;
                        let y_factor = ((y as f32 / height as f32 - 0.5) + warp).abs();
                        28.0 - y_factor * 6.0
                    }
                };

                // Elevation adjustment (only for land above sea level)
                let elevation_adjustment = if elevation > 0.0 {
                    // Lapse rate: temperature drops with altitude
                    -(elevation / 1000.0) * ELEVATION_LAPSE_RATE
                } else {
                    // Ocean: slight warming effect in shallow water
                    0.0
                };

                base_temp + elevation_adjustment
            }).collect()
        })
        .collect();

    // Flatten rows into single vector
    let data: Vec<f32> = rows.into_iter().flatten().collect();
    Tilemap::from_vec(width, height, data)
}

// =============================================================================
// PREVAILING WINDS
// =============================================================================

/// Calculate prevailing wind direction based on latitude
/// Returns unit vector (dx, dy) pointing in wind direction (where wind is blowing TO)
/// latitude_normalized: 0 = equator, 1 = pole
fn get_prevailing_wind(latitude_normalized: f32) -> (f32, f32) {
    if latitude_normalized < 0.15 {
        // Equatorial doldrums - weak/variable winds, slight easterly
        (0.3, 0.0)
    } else if latitude_normalized < 0.35 {
        // Trade winds (15-35° latitude) - blow from east (toward west)
        // NE trades in northern hemisphere, SE in southern
        (-0.85, -0.2)  // Strong westward component
    } else if latitude_normalized < 0.65 {
        // Westerlies (35-65° latitude) - blow from west (toward east)
        // Strongest mid-latitude winds, responsible for weather patterns
        (0.9, 0.15)  // Strong eastward component
    } else {
        // Polar easterlies (65-90° latitude) - blow from east
        (-0.6, 0.0)
    }
}

// =============================================================================
// LONGITUDE VARIATION (Break Vertical Bands)
// =============================================================================

/// Calculate longitude-based moisture variation to break uniform horizontal bands.
/// Uses multi-scale Perlin noise to create continental and regional moisture patches.
///
/// Returns a modifier in range [-0.3, +0.3] that should be added to moisture calculations.
fn get_longitude_moisture_variation(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    noise: &Perlin,
    latitude_normalized: f32,
) -> f32 {
    let nx = x as f64 / width as f64;
    let ny = y as f64 / height as f64;

    // Continental-scale variation (3x frequency) - large moisture patches
    let continental = noise.get([nx * 3.0, ny * 3.0, 0.0]) as f32 * 0.15;

    // Regional-scale variation (8x frequency) - smaller patches
    let regional = noise.get([nx * 8.0, ny * 8.0, 1.0]) as f32 * 0.10;

    // Westerly belt asymmetry: western coasts wetter in 35-65° latitude
    // This mimics how westerlies bring moisture from oceans to western continental coasts
    let westerly_asymmetry = if latitude_normalized > 0.35 && latitude_normalized < 0.65 {
        // nx = 0 is "western edge" of continent (arbitrary, but provides variation)
        // Create a gradient that makes western portions wetter
        let longitude_factor = nx as f32;  // 0 at west, 1 at east
        let westerly_strength = 1.0 - ((latitude_normalized - 0.5).abs() / 0.15).min(1.0);
        // Western side gets bonus, eastern side gets penalty
        -0.08 * (longitude_factor - 0.5) * westerly_strength
    } else {
        0.0
    };

    // Combine all variations and clamp to [-0.3, +0.3]
    (continental + regional + westerly_asymmetry).clamp(-0.3, 0.3)
}

// =============================================================================
// MOISTURE GENERATION
// =============================================================================

/// Generate moisture map based on ocean proximity and elevation
/// Returns moisture as 0.0-1.0
pub fn generate_moisture(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
) -> Tilemap<f32> {
    // Use default regional scale
    generate_moisture_scaled(heightmap, width, height, &crate::scale::MapScale::default())
}

/// Generate moisture map with full climate configuration
pub fn generate_moisture_with_config(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    config: &ClimateConfig,
) -> Tilemap<f32> {
    let map_scale = crate::scale::MapScale::default();
    generate_moisture_full(heightmap, width, height, &map_scale, config)
}

/// Generate moisture map with full climate configuration and explicit seed
pub fn generate_moisture_with_config_and_seed(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    config: &ClimateConfig,
    seed: u64,
) -> Tilemap<f32> {
    let map_scale = crate::scale::MapScale::default();
    generate_moisture_full_with_seed(heightmap, width, height, &map_scale, config, seed)
}

/// Compute ocean distance field using BFS.
/// This is cached/shared between moisture generation functions to avoid duplicate computation.
pub fn compute_ocean_distance(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;
    let mut ocean_distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();

    // Initialize with ocean cells
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) <= 0.0 {
                ocean_distance.set(x, y, 0.0);
                queue.push_back((x, y, 0.0));
            }
        }
    }

    // BFS to compute distance
    while let Some((x, y, dist)) = queue.pop_front() {
        let neighbors = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];

        for (nx, ny) in neighbors {
            if nx >= width || ny >= height {
                continue;
            }
            let new_dist = dist + 1.0;
            if new_dist < *ocean_distance.get(nx, ny) {
                ocean_distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }

    ocean_distance
}

/// Generate moisture map with explicit scale parameter
pub fn generate_moisture_scaled(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    map_scale: &MapScale,
) -> Tilemap<f32> {
    // Continental moisture noise - creates large wet/dry patches independent of latitude
    // This breaks the "layer cake" horizontal banding effect
    let continental_noise = Perlin::new(1).set_seed(42);
    let regional_noise = Perlin::new(1).set_seed(142);

    // Compute ocean distance (single BFS, reused)
    let ocean_distance = compute_ocean_distance(heightmap);

    // Second pass: compute moisture from distance
    // Key insight: Start DRY and only add moisture near ocean
    let mut moisture = Tilemap::new_with(width, height, 0.0f32);

    // Scale distance thresholds based on physical map scale
    let coastal_range = scale_distance(8.0, map_scale);   // Coastal moisture zone
    let max_range = scale_distance(25.0, map_scale);      // Maximum moisture reach

    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            let dist = *ocean_distance.get(x, y);

            // Ocean is always max moisture
            if elevation <= 0.0 {
                moisture.set(x, y, 1.0);
                continue;
            }

            // Normalized coordinates for noise sampling
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;

            // Latitude factor (0 = equator, 1 = pole)
            let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;

            // CONTINENTAL MOISTURE PATCHES - breaks horizontal banding
            // Large-scale noise creates "wet continent" vs "dry continent" areas
            // Strong enough to override latitude effects and create patchy biomes
            let continental_wet = continental_noise.get([nx * 2.0, ny * 2.0]) as f32;
            let regional_wet = regional_noise.get([nx * 4.0, ny * 4.0]) as f32;
            // Combined patch value: positive = wetter, negative = drier
            // Increased strength to effectively break horizontal bands
            let moisture_patch = continental_wet * 0.5 + regional_wet * 0.25;

            // BASE MOISTURE: Very conservative - only areas near ocean get moisture
            // Exponential decay from coastline
            let base_moisture = if dist < coastal_range {
                // Very close to ocean - high moisture
                0.7 * (1.0 - dist / coastal_range).powf(0.5)
            } else if dist < max_range {
                // Moderate distance - drops to very low
                0.15 * (1.0 - (dist - coastal_range) / (max_range - coastal_range)).powf(2.0)
            } else {
                // Far from ocean - essentially dry
                0.02
            };

            // LATITUDE MODIFIERS
            // Equatorial wet zone (ITCZ) - adds moisture in tropics
            let equatorial_bonus = if latitude_normalized < 0.2 {
                0.35 * (1.0 - latitude_normalized / 0.2)
            } else {
                0.0
            };

            // Subtropical DRY belt (Hadley cell) - drying at 15-45° latitude
            // NOW MODULATED BY NOISE: deserts form in patches, not bands
            // Where moisture_patch is positive (wet patch), the penalty is reduced
            let subtropical_base = if latitude_normalized > 0.15 && latitude_normalized < 0.55 {
                let belt_center = 0.35;
                let dist_from_center = (latitude_normalized - belt_center).abs();
                0.35 * (1.0 - (dist_from_center / 0.20).min(1.0))
            } else {
                0.0
            };
            // Reduce penalty in wet patches, increase in dry patches
            // Multiplier of 2.0 ensures wet patches can fully eliminate the penalty
            let subtropical_penalty = (subtropical_base * (1.0 - moisture_patch.max(0.0) * 2.0))
                .max(0.0);

            // Mid-latitude westerlies (40-65°) - some moisture from polar fronts
            let midlat_bonus = if latitude_normalized > 0.5 && latitude_normalized < 0.8 {
                0.15 * (1.0 - ((latitude_normalized - 0.65) / 0.15).abs().min(1.0))
            } else {
                0.0
            };

            // Polar dry (cold air holds less moisture)
            let polar_penalty = if latitude_normalized > 0.75 {
                0.2 * ((latitude_normalized - 0.75) / 0.25)
            } else {
                0.0
            };

            // ELEVATION MODIFIERS (scaled by elevation_scale)
            let orographic_threshold = scale_elevation(300.0, map_scale);
            let orographic_ref = scale_elevation(1500.0, map_scale);
            let altitude_start = scale_elevation(1500.0, map_scale);
            let altitude_ref = scale_elevation(2000.0, map_scale);
            let rain_shadow_elev = scale_elevation(500.0, map_scale);

            // Mountains near coast catch rain (orographic lift)
            let orographic_bonus = if elevation > orographic_threshold && dist < coastal_range * 2.0 {
                0.15 * (elevation / orographic_ref).min(1.0) * (1.0 - dist / (coastal_range * 2.0))
            } else {
                0.0
            };

            // High altitude is always dry (above cloud level)
            let altitude_penalty = if elevation > altitude_start {
                0.3 * ((elevation - altitude_start) / altitude_ref).min(1.0)
            } else {
                0.0
            };

            // Rain shadow - check for mountains UPWIND based on prevailing wind direction
            // Wind carries moisture; mountains block it, creating dry lee (downwind) sides
            let wind = get_prevailing_wind(latitude_normalized);
            // Look upwind (opposite of wind direction) for blocking mountains
            let upwind_dir = (-wind.0, -wind.1);

            let rain_shadow_range = scale_distance(20.0, map_scale);  // Check 20km upwind
            let rain_shadow = if elevation > 0.0 && elevation < rain_shadow_elev {
                let mut max_blocking = 0.0f32;
                let steps = 12;

                // Sample along the upwind direction
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let sample_dist = t * rain_shadow_range;
                    let sx = (x as f32 + upwind_dir.0 * sample_dist) as i32;
                    let sy = (y as f32 + upwind_dir.1 * sample_dist) as i32;
                    let sx = sx.rem_euclid(width as i32) as usize;
                    let sy = sy.clamp(0, height as i32 - 1) as usize;

                    let blocking_elev = *heightmap.get(sx, sy);

                    // Mountain blocks moisture if it's significantly higher than current point
                    if blocking_elev > elevation + 400.0 {
                        // Stronger effect for taller mountains and closer blocking
                        let height_factor = ((blocking_elev - elevation) / 2000.0).min(1.0);
                        let dist_factor = 1.0 - t * 0.5;  // Closer mountains have stronger effect
                        max_blocking = max_blocking.max(height_factor * dist_factor * 0.55);
                    }
                }

                // Also check at slight angles (wind doesn't blow perfectly straight)
                for angle_offset in [-0.3f32, 0.3f32] {
                    let cos_off = angle_offset.cos();
                    let sin_off = angle_offset.sin();
                    let offset_dir = (
                        upwind_dir.0 * cos_off - upwind_dir.1 * sin_off,
                        upwind_dir.0 * sin_off + upwind_dir.1 * cos_off,
                    );

                    for step in 1..=6 {
                        let t = step as f32 / 6.0;
                        let sample_dist = t * rain_shadow_range * 0.7;
                        let sx = (x as f32 + offset_dir.0 * sample_dist) as i32;
                        let sy = (y as f32 + offset_dir.1 * sample_dist) as i32;
                        let sx = sx.rem_euclid(width as i32) as usize;
                        let sy = sy.clamp(0, height as i32 - 1) as usize;

                        let blocking_elev = *heightmap.get(sx, sy);
                        if blocking_elev > elevation + 400.0 {
                            let height_factor = ((blocking_elev - elevation) / 2000.0).min(1.0);
                            max_blocking = max_blocking.max(height_factor * 0.3);
                        }
                    }
                }

                max_blocking
            } else {
                0.0
            };

            // Combine all factors
            // moisture_patch adds wet patches even in normally dry areas
            let patch_bonus = moisture_patch.max(0.0) * 0.5;  // Strong enough to create forests in desert latitudes
            let final_moisture = (base_moisture
                + equatorial_bonus + midlat_bonus + orographic_bonus + patch_bonus
                - subtropical_penalty - polar_penalty - altitude_penalty - rain_shadow)
                .clamp(0.02, 1.0);

            moisture.set(x, y, final_moisture);
        }
    }

    moisture
}

/// Generate moisture map with full configuration (scale + climate mode + rainfall)
pub fn generate_moisture_full(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    map_scale: &MapScale,
    config: &ClimateConfig,
) -> Tilemap<f32> {
    // Use a default seed for backwards compatibility
    generate_moisture_full_with_seed(heightmap, width, height, map_scale, config, 42)
}

/// Generate moisture map with full configuration and explicit seed for longitude variation
pub fn generate_moisture_full_with_seed(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    map_scale: &MapScale,
    config: &ClimateConfig,
    seed: u64,
) -> Tilemap<f32> {
    // Create Perlin noise for longitude variation
    let longitude_noise = Perlin::new(1).set_seed(seed as u32);

    // Continental moisture noise - creates large wet/dry patches independent of latitude
    // This breaks the "layer cake" horizontal banding effect
    let continental_noise = Perlin::new(1).set_seed(seed as u32 + 200);
    let regional_noise = Perlin::new(1).set_seed(seed as u32 + 300);

    // Compute ocean distance (single BFS, reused)
    let ocean_distance = compute_ocean_distance(heightmap);

    let mut moisture = Tilemap::new_with(width, height, 0.0f32);

    let coastal_range = scale_distance(8.0, map_scale);
    let max_range = scale_distance(25.0, map_scale);

    // Get rainfall modifiers
    let moisture_mult = config.rainfall.moisture_multiplier();
    let moisture_floor = config.rainfall.moisture_floor();

    for y in 0..height {
        // Calculate latitude once per row
        let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;

        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            let dist = *ocean_distance.get(x, y);

            if elevation <= 0.0 {
                moisture.set(x, y, 1.0);
                continue;
            }

            // Base moisture from ocean distance
            let base_moisture = if dist < coastal_range {
                0.7 * (1.0 - dist / coastal_range).powf(0.5)
            } else if dist < max_range {
                0.15 * (1.0 - (dist - coastal_range) / (max_range - coastal_range)).powf(2.0)
            } else {
                0.02
            };

            // Normalized coordinates for noise sampling
            let nx_coord = x as f64 / width as f64;
            let ny_coord = y as f64 / height as f64;

            // CONTINENTAL MOISTURE PATCHES - breaks horizontal banding
            // Large-scale noise creates "wet continent" vs "dry continent" areas
            // Strong enough to override latitude effects and create patchy biomes
            let continental_wet = continental_noise.get([nx_coord * 2.0, ny_coord * 2.0]) as f32;
            let regional_wet = regional_noise.get([nx_coord * 4.0, ny_coord * 4.0]) as f32;
            // Combined patch value: positive = wetter, negative = drier
            // Increased strength to effectively break horizontal bands
            let moisture_patch = continental_wet * 0.5 + regional_wet * 0.25;

            // Longitude variation to break vertical bands
            let longitude_variation = get_longitude_moisture_variation(
                x, y, width, height, &longitude_noise, latitude_normalized
            );

            // Latitude-based modifiers (only for Globe mode)
            // NOW MODULATED BY MOISTURE PATCHES to create patchy deserts instead of bands
            let (equatorial_bonus, subtropical_penalty, midlat_bonus, polar_penalty) = match config.mode {
                ClimateMode::Globe => {
                    let eq_bonus = if latitude_normalized < 0.2 {
                        0.35 * (1.0 - latitude_normalized / 0.2)
                    } else {
                        0.0
                    };

                    // Subtropical penalty is now modulated by moisture patches
                    // Wet patches reduce the penalty, allowing forests in subtropical zones
                    let subtr_base = if latitude_normalized > 0.15 && latitude_normalized < 0.55 {
                        let belt_center = 0.35;
                        let dist_from_center = (latitude_normalized - belt_center).abs();
                        0.35 * (1.0 - (dist_from_center / 0.20).min(1.0))
                    } else {
                        0.0
                    };
                    // Reduce penalty in wet patches, creating patchy deserts
                    // Multiplier of 2.0 ensures wet patches can fully eliminate the penalty
                    let subtr_penalty = (subtr_base * (1.0 - moisture_patch.max(0.0) * 2.0))
                        .max(0.0);

                    let mid_bonus = if latitude_normalized > 0.5 && latitude_normalized < 0.8 {
                        0.15 * (1.0 - ((latitude_normalized - 0.65) / 0.15).abs().min(1.0))
                    } else {
                        0.0
                    };

                    let pol_penalty = if latitude_normalized > 0.75 {
                        0.2 * ((latitude_normalized - 0.75) / 0.25)
                    } else {
                        0.0
                    };

                    (eq_bonus, subtr_penalty, mid_bonus, pol_penalty)
                }
                ClimateMode::Flat => {
                    // No latitude effects
                    (0.0, 0.0, 0.0, 0.0)
                }
                ClimateMode::TemperateBand => {
                    // Slight mid-latitude bonus throughout
                    (0.0, 0.0, 0.1, 0.0)
                }
                ClimateMode::TropicalBand => {
                    // Strong equatorial bonus throughout
                    (0.3, 0.0, 0.0, 0.0)
                }
            };

            // Elevation modifiers (same for all modes)
            let orographic_threshold = scale_elevation(300.0, map_scale);
            let orographic_ref = scale_elevation(1500.0, map_scale);
            let altitude_start = scale_elevation(1500.0, map_scale);
            let altitude_ref = scale_elevation(2000.0, map_scale);

            let orographic_bonus = if elevation > orographic_threshold && dist < coastal_range * 2.0 {
                0.15 * (elevation / orographic_ref).min(1.0) * (1.0 - dist / (coastal_range * 2.0))
            } else {
                0.0
            };

            let altitude_penalty = if elevation > altitude_start {
                0.3 * ((elevation - altitude_start) / altitude_ref).min(1.0)
            } else {
                0.0
            };

            // Combine factors including longitude variation and apply rainfall multiplier
            // moisture_patch adds wet patches even in normally dry areas
            let patch_bonus = moisture_patch.max(0.0) * 0.5;  // Strong enough to create forests in desert latitudes
            let raw_moisture = base_moisture
                + equatorial_bonus + midlat_bonus + orographic_bonus + longitude_variation + patch_bonus
                - subtropical_penalty - polar_penalty - altitude_penalty;

            let final_moisture = (raw_moisture * moisture_mult)
                .max(moisture_floor)
                .min(1.0);

            moisture.set(x, y, final_moisture);
        }
    }

    moisture
}

// =============================================================================
// BIOME CLASSIFICATION
// =============================================================================

/// Smooth step interpolation (Hermite smoothstep)
/// Returns 0 for x <= edge0, 1 for x >= edge1, smooth transition in between
pub fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Result of fuzzy biome classification with smooth transitions
#[derive(Debug, Clone)]
pub struct FuzzyBiomeResult {
    /// The primary biome at this location
    pub primary: Biome,
    /// Optional secondary biome with blend weight (0.0-1.0)
    /// When close to a biome boundary, this contains the adjacent biome
    pub secondary: Option<(Biome, f32)>,
    /// Overall transition factor (0.0 = center of biome, 1.0 = right at boundary)
    pub transition_factor: f32,
}

impl FuzzyBiomeResult {
    /// Get interpolated color between primary and secondary biomes
    pub fn blended_color(&self) -> (u8, u8, u8) {
        let (pr, pg, pb) = self.primary.color();
        match &self.secondary {
            Some((secondary, weight)) => {
                let (sr, sg, sb) = secondary.color();
                let w = *weight;
                (
                    ((pr as f32 * (1.0 - w) + sr as f32 * w) as u8),
                    ((pg as f32 * (1.0 - w) + sg as f32 * w) as u8),
                    ((pb as f32 * (1.0 - w) + sb as f32 * w) as u8),
                )
            }
            None => (pr, pg, pb),
        }
    }
}

/// Biome types based on temperature and moisture
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Biome {
    // Ocean biomes
    DeepOcean,
    Ocean,
    CoastalWater,

    // Cold biomes
    Ice,
    Tundra,
    BorealForest,

    // Temperate biomes
    TemperateGrassland,
    TemperateForest,
    TemperateRainforest,

    // Warm biomes
    Desert,
    Savanna,
    TropicalForest,
    TropicalRainforest,

    // Mountain biomes (altitudinal zones)
    MontaneForest,      // 1000-2000m tropical, cool humid forest
    CloudForest,        // 2000-3000m tropical, misty, epiphytes
    Paramo,             // 3000-4000m tropical, highland grassland
    SubalpineForest,    // Temperate high elevation conifer forest
    AlpineMeadow,       // Temperate high elevation grassland
    AlpineTundra,
    SnowyPeaks,
}

impl Biome {
    /// Classify lowland biomes (below mountain elevation thresholds)
    /// Based purely on temperature and moisture
    fn classify_lowland(temperature: f32, moisture: f32) -> Biome {
        match (temperature, moisture) {
            // Freezing temperatures
            (t, _) if t < -10.0 => Biome::Ice,
            (t, _) if t < 0.0 => Biome::Tundra,

            // Cold temperatures
            (t, m) if t < 10.0 => {
                if m > 0.5 { Biome::BorealForest } else { Biome::Tundra }
            }

            // Temperate temperatures
            (t, m) if t < 20.0 => {
                if m > 0.7 { Biome::TemperateRainforest }
                else if m > 0.4 { Biome::TemperateForest }
                else { Biome::TemperateGrassland }
            }

            // Warm/tropical temperatures
            (_, m) => {
                if m > 0.7 { Biome::TropicalRainforest }
                else if m > 0.4 { Biome::TropicalForest }
                else if m > 0.2 { Biome::Savanna }
                else { Biome::Desert }
            }
        }
    }

    /// Classify biome based on elevation, temperature (Celsius), and moisture (0-1)
    /// Uses proper altitudinal zonation that varies by latitude/climate zone
    pub fn classify(elevation: f32, temperature: f32, moisture: f32) -> Biome {
        // Ocean biomes
        if elevation <= 0.0 {
            if elevation < -2000.0 {
                return Biome::DeepOcean;
            } else if elevation < -100.0 {
                return Biome::Ocean;
            } else {
                return Biome::CoastalWater;
            }
        }

        // Calculate sea-level temperature by reversing the lapse rate
        // This tells us what climate zone we're in (tropical, temperate, polar)
        let sea_level_temp = temperature + (elevation / 1000.0) * ELEVATION_LAPSE_RATE;

        // Determine climate zone from sea-level temperature
        let is_tropical = sea_level_temp > 22.0;      // Near equator
        let is_temperate = sea_level_temp > 8.0 && sea_level_temp <= 22.0;
        // Polar: sea_level_temp <= 8.0

        // ALTITUDINAL ZONATION (elevation-first for mountains)
        // Different elevation thresholds and biome sequences for each climate zone

        if is_tropical {
            // Tropical mountain zones (Andes-style)
            // | 0-1000m    | Tierra Caliente | 24-30°C  | Tropical Forest     |
            // | 1000-2000m | Tierra Templada | 18-24°C  | Montane Forest      |
            // | 2000-3000m | Tierra Fría     | 12-18°C  | Cloud Forest        |
            // | 3000-4000m | Páramo          | 6-12°C   | Highland Grassland  |
            // | 4000-4800m | Puna            | 0-6°C    | Alpine Tundra       |
            // | 4800m+     | Tierra Helada   | <0°C     | Permanent Snow      |
            match elevation {
                e if e > 4800.0 => Biome::SnowyPeaks,
                e if e > 4000.0 => Biome::AlpineTundra,
                e if e > 3000.0 => {
                    // Páramo - highland grassland (dry) or wet páramo
                    if moisture > 0.5 { Biome::Paramo } else { Biome::AlpineTundra }
                }
                e if e > 2000.0 => {
                    // Cloud forest zone - needs moisture for true cloud forest
                    if moisture > 0.5 { Biome::CloudForest } else { Biome::MontaneForest }
                }
                e if e > 1000.0 => {
                    // Montane forest zone
                    if moisture > 0.4 { Biome::MontaneForest }
                    else { Self::classify_lowland(temperature, moisture) }
                }
                _ => Self::classify_lowland(temperature, moisture),
            }
        } else if is_temperate {
            // Temperate mountain zones (Alps/Rockies-style)
            // | 0-1000m    | Lowland   | 10-20°C | Temperate Forest  |
            // | 1000-1800m | Montane   | 6-12°C  | Subalpine Forest  |
            // | 1800-2500m | Subalpine | 2-8°C   | Alpine Meadow     |
            // | 2500-3500m | Alpine    | -4-4°C  | Alpine Tundra     |
            // | 3500m+     | Nival     | <-4°C   | Permanent Snow    |
            match elevation {
                e if e > 3500.0 => Biome::SnowyPeaks,
                e if e > 2500.0 => Biome::AlpineTundra,
                e if e > 1800.0 => {
                    // Alpine meadow zone
                    if moisture > 0.4 { Biome::AlpineMeadow } else { Biome::AlpineTundra }
                }
                e if e > 1000.0 => {
                    // Subalpine forest zone
                    if moisture > 0.4 { Biome::SubalpineForest }
                    else { Self::classify_lowland(temperature, moisture) }
                }
                _ => Self::classify_lowland(temperature, moisture),
            }
        } else {
            // Polar/Subpolar mountain zones (lower treeline)
            // | 0-500m   | Lowland    | Tundra/Boreal     |
            // | 500-1000m | Low Alpine | Tundra            |
            // | 1000-2000m | High Alpine | Alpine Tundra   |
            // | 2000m+    | Nival      | Permanent Snow    |
            match elevation {
                e if e > 2000.0 => Biome::SnowyPeaks,
                e if e > 1000.0 => Biome::AlpineTundra,
                e if e > 500.0 => Biome::Tundra,
                _ => Self::classify_lowland(temperature, moisture),
            }
        }
    }
    
    /// Get RGB color for biome visualization
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            // Ocean
            Biome::DeepOcean => (20, 40, 80),
            Biome::Ocean => (30, 60, 120),
            Biome::CoastalWater => (60, 100, 160),

            // Cold
            Biome::Ice => (240, 250, 255),
            Biome::Tundra => (180, 190, 170),
            Biome::BorealForest => (50, 80, 50),

            // Temperate
            Biome::TemperateGrassland => (140, 170, 80),
            Biome::TemperateForest => (40, 100, 40),
            Biome::TemperateRainforest => (30, 80, 50),

            // Warm
            Biome::Desert => (210, 180, 120),
            Biome::Savanna => (170, 160, 80),
            Biome::TropicalForest => (30, 120, 30),
            Biome::TropicalRainforest => (20, 90, 40),

            // Mountain (altitudinal zones)
            Biome::MontaneForest => (45, 90, 55),       // Dark green
            Biome::CloudForest => (60, 110, 80),        // Misty green
            Biome::Paramo => (160, 155, 120),           // Tan-green highland
            Biome::SubalpineForest => (40, 70, 45),     // Dark conifer green
            Biome::AlpineMeadow => (130, 160, 100),     // Bright alpine grass
            Biome::AlpineTundra => (140, 140, 130),
            Biome::SnowyPeaks => (255, 255, 255),
        }
    }

    /// Classify biome with fuzzy boundaries for smooth transitions
    /// Uses elevation transition zones for mountain biomes and temperature/moisture transitions for lowlands
    pub fn classify_fuzzy(elevation: f32, temperature: f32, moisture: f32) -> FuzzyBiomeResult {
        // Transition zone widths
        const ELEV_TRANSITION: f32 = 150.0;  // 150m elevation transition zone
        const MOIST_TRANSITION: f32 = 0.1;   // 10% moisture transition zone

        // Ocean biomes don't blend with land
        if elevation <= 0.0 {
            let primary = if elevation < -2000.0 {
                Biome::DeepOcean
            } else if elevation < -100.0 {
                Biome::Ocean
            } else {
                Biome::CoastalWater
            };
            return FuzzyBiomeResult {
                primary,
                secondary: None,
                transition_factor: 0.0,
            };
        }

        // Calculate primary biome using the new altitudinal classification
        let primary = Self::classify(elevation, temperature, moisture);

        // Calculate sea-level temperature for climate zone detection
        let sea_level_temp = temperature + (elevation / 1000.0) * ELEVATION_LAPSE_RATE;
        let is_tropical = sea_level_temp > 22.0;
        let is_temperate = sea_level_temp > 8.0 && sea_level_temp <= 22.0;

        // Check for elevation transitions between mountain biomes
        let elev_thresholds: Vec<(f32, Biome, Biome)> = if is_tropical {
            vec![
                (4800.0, Biome::AlpineTundra, Biome::SnowyPeaks),
                (4000.0, Biome::Paramo, Biome::AlpineTundra),
                (3000.0, Biome::CloudForest, Biome::Paramo),
                (2000.0, Biome::MontaneForest, Biome::CloudForest),
                (1000.0, Self::classify_lowland(temperature, moisture), Biome::MontaneForest),
            ]
        } else if is_temperate {
            vec![
                (3500.0, Biome::AlpineTundra, Biome::SnowyPeaks),
                (2500.0, Biome::AlpineMeadow, Biome::AlpineTundra),
                (1800.0, Biome::SubalpineForest, Biome::AlpineMeadow),
                (1000.0, Self::classify_lowland(temperature, moisture), Biome::SubalpineForest),
            ]
        } else {
            // Polar
            vec![
                (2000.0, Biome::AlpineTundra, Biome::SnowyPeaks),
                (1000.0, Biome::Tundra, Biome::AlpineTundra),
                (500.0, Self::classify_lowland(temperature, moisture), Biome::Tundra),
            ]
        };

        for (threshold, lower_biome, upper_biome) in elev_thresholds {
            if (elevation - threshold).abs() < ELEV_TRANSITION {
                let blend = smooth_step(
                    threshold - ELEV_TRANSITION,
                    threshold + ELEV_TRANSITION,
                    elevation
                );
                if blend > 0.0 && blend < 1.0 {
                    let (actual_primary, actual_secondary) = if elevation < threshold {
                        (lower_biome, upper_biome)
                    } else {
                        (upper_biome, lower_biome)
                    };
                    return FuzzyBiomeResult {
                        primary: actual_primary,
                        secondary: Some((actual_secondary, blend)),
                        transition_factor: blend,
                    };
                }
            }
        }

        // Check moisture transitions for lowland biomes
        let moist_thresholds = [0.2, 0.4, 0.5, 0.7];
        for threshold in moist_thresholds {
            if (moisture - threshold).abs() < MOIST_TRANSITION {
                let blend = smooth_step(
                    threshold - MOIST_TRANSITION,
                    threshold + MOIST_TRANSITION,
                    moisture
                );
                if blend > 0.0 && blend < 1.0 {
                    // Get biome just below and above threshold
                    let dry_biome = Self::classify(elevation, temperature, threshold - MOIST_TRANSITION - 0.01);
                    let wet_biome = Self::classify(elevation, temperature, threshold + MOIST_TRANSITION + 0.01);
                    if dry_biome != wet_biome {
                        return FuzzyBiomeResult {
                            primary,
                            secondary: Some((if moisture < threshold { wet_biome } else { dry_biome }, blend)),
                            transition_factor: blend,
                        };
                    }
                }
            }
        }

        // No transition zone - pure biome
        FuzzyBiomeResult {
            primary,
            secondary: None,
            transition_factor: 0.0,
        }
    }
}

/// Generate biome map from heightmap, temperature, and moisture
pub fn generate_biomes(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
) -> Tilemap<Biome> {
    let width = heightmap.width;
    let height = heightmap.height;
    
    let mut biomes = Tilemap::new_with(width, height, Biome::Ocean);
    
    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);
            
            let biome = Biome::classify(elev, temp, moist);
            biomes.set(x, y, biome);
        }
    }
    
    biomes
}
