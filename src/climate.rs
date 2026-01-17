//! Climate system for generating temperature and moisture maps
//! Based on latitude, elevation, and ocean proximity

use crate::tilemap::Tilemap;
use crate::scale::{MapScale, scale_distance, scale_elevation};

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
    let mut temperature = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            
            // Latitude factor: 0 at equator, 1 at poles
            // Map y to latitude: y=0 is north pole, y=height/2 is equator, y=height is south pole
            let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;
            
            // Base temperature from latitude (cosine curve for smoother transition)
            let lat_factor = latitude_normalized.powf(1.5); // More gradual near equator
            let base_temp = EQUATOR_TEMP - (EQUATOR_TEMP - POLE_TEMP) * lat_factor;
            
            // Elevation adjustment (only for land above sea level)
            let elevation_adjustment = if elevation > 0.0 {
                // Lapse rate: temperature drops with altitude
                -(elevation / 1000.0) * ELEVATION_LAPSE_RATE
            } else {
                // Ocean: slight warming effect in shallow water
                0.0
            };
            
            let temp = base_temp + elevation_adjustment;
            temperature.set(x, y, temp);
        }
    }
    
    temperature
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

/// Generate moisture map with explicit scale parameter
pub fn generate_moisture_scaled(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
    map_scale: &MapScale,
) -> Tilemap<f32> {
    use std::collections::VecDeque;

    // First pass: compute distance from ocean
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

            // Latitude factor (0 = equator, 1 = pole)
            let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;

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

            // Subtropical DRY belt (Hadley cell) - MAJOR drying at 15-45° latitude
            // This is where real-world deserts form (Sahara, Arabian, Sonoran, etc.)
            let subtropical_penalty = if latitude_normalized > 0.15 && latitude_normalized < 0.55 {
                let belt_center = 0.35;
                let dist_from_center = (latitude_normalized - belt_center).abs();
                // Strong penalty with wide coverage
                0.5 * (1.0 - (dist_from_center / 0.20).min(1.0))
            } else {
                0.0
            };

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
            let final_moisture = (base_moisture
                + equatorial_bonus + midlat_bonus + orographic_bonus
                - subtropical_penalty - polar_penalty - altitude_penalty - rain_shadow)
                .clamp(0.02, 1.0);

            moisture.set(x, y, final_moisture);
        }
    }

    moisture
}

// =============================================================================
// BIOME CLASSIFICATION
// =============================================================================

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
    
    // Mountain biomes
    AlpineTundra,
    SnowyPeaks,
}

impl Biome {
    /// Classify biome based on temperature (Celsius) and moisture (0-1)
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
        
        // High elevation biomes - adjusted for realistic terrain heights
        // Snowy peaks at highest elevations with very cold temps
        if elevation > 1500.0 && temperature < -15.0 {
            return Biome::SnowyPeaks;
        }
        // Alpine tundra at high elevations
        if elevation > 1000.0 {
            if temperature < -5.0 {
                return Biome::SnowyPeaks;
            } else if temperature < 10.0 {
                return Biome::AlpineTundra;
            }
        }
        
        // Land biomes based on temperature and moisture
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
            
            // Mountain
            Biome::AlpineTundra => (140, 140, 130),
            Biome::SnowyPeaks => (255, 255, 255),
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
