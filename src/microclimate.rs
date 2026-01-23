//! Microclimate system for local terrain effects
//!
//! Provides localized climate modifiers based on terrain features like valleys,
//! ridges, slopes, lake proximity, and forest coverage.

use crate::tilemap::{Tilemap, DirectionalContext};
use crate::biomes::ExtendedBiome;
use crate::water_bodies::{WaterBodyId, WaterBody, WaterBodyType};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Configuration for microclimate calculation
#[derive(Clone, Debug)]
pub struct MicroclimateConfig {
    /// Temperature increase for valley locations (Celsius)
    pub valley_warmth: f32,
    /// Temperature decrease for ridge locations (Celsius)
    pub ridge_cooling: f32,
    /// Temperature bonus for south-facing slopes at high latitudes (Celsius)
    pub south_slope_bonus: f32,
    /// Moisture increase near lakes (fraction, e.g., 0.15 = 15%)
    pub lake_moisture_bonus: f32,
    /// Maximum distance for lake effect (tiles)
    pub lake_effect_range: usize,
    /// Moisture retention for forested areas (fraction)
    pub forest_moisture_retention: f32,
    /// Slope threshold for valley/ridge detection
    pub slope_threshold: f32,
    /// Curvature threshold for valley/ridge detection
    pub curvature_threshold: f32,
}

impl Default for MicroclimateConfig {
    fn default() -> Self {
        Self {
            valley_warmth: 2.0,
            ridge_cooling: 1.5,
            south_slope_bonus: 1.5,
            lake_moisture_bonus: 0.15,
            lake_effect_range: 10,
            forest_moisture_retention: 0.05,
            slope_threshold: 5.0,
            curvature_threshold: 0.5,
        }
    }
}

// =============================================================================
// MICROCLIMATE MODIFIERS
// =============================================================================

/// Per-tile microclimate modifiers that adjust base climate values
#[derive(Clone, Debug, Default)]
pub struct MicroclimateModifiers {
    /// Temperature adjustment (Celsius)
    /// Positive = warmer (valleys), Negative = cooler (ridges)
    pub temperature_mod: f32,
    /// Moisture adjustment (0-1 scale)
    /// Positive = more moisture (lake proximity, forest)
    pub moisture_mod: f32,
    /// Wind shelter factor (0 = exposed, 1 = fully sheltered)
    /// Valleys and areas behind ridges are more sheltered
    pub wind_shelter: f32,
    /// Frost risk factor (0 = low risk, 1 = high risk)
    /// Higher in valleys due to cold air pooling
    pub frost_risk: f32,
}

impl MicroclimateModifiers {
    /// Apply modifiers to base temperature
    pub fn apply_temperature(&self, base_temp: f32) -> f32 {
        base_temp + self.temperature_mod
    }

    /// Apply modifiers to base moisture (clamped to 0-1)
    pub fn apply_moisture(&self, base_moisture: f32) -> f32 {
        (base_moisture + self.moisture_mod).clamp(0.0, 1.0)
    }
}

// =============================================================================
// MICROCLIMATE GENERATION
// =============================================================================

/// Generate microclimate modifiers for the entire map
pub fn generate_microclimates(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    water_body_map: &Tilemap<WaterBodyId>,
    water_bodies: &[WaterBody],
    config: &MicroclimateConfig,
) -> Tilemap<MicroclimateModifiers> {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut microclimates = Tilemap::new_with(width, height, MicroclimateModifiers::default());

    // Compute lake distance map for efficient lookup
    let lake_distance = compute_lake_distance(water_body_map, water_bodies, width, height, config.lake_effect_range);

    for y in 0..height {
        // Calculate latitude for south-facing slope bonus
        let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;
        let in_northern_hemisphere = y < height / 2;

        for x in 0..width {
            let elevation = *heightmap.get(x, y);

            // Skip ocean tiles
            if elevation <= 0.0 {
                continue;
            }

            let context = heightmap.analyze_directional_context(x, y);
            let biome = *biomes.get(x, y);

            let mut modifiers = MicroclimateModifiers::default();

            // Valley/Ridge detection based on curvature
            // Negative curvature = valley (concave), Positive = ridge (convex)
            let is_valley = context.curvature < -config.curvature_threshold
                && context.gradient_magnitude > config.slope_threshold;
            let is_ridge = context.curvature > config.curvature_threshold
                && context.gradient_magnitude > config.slope_threshold;

            // Valley effects: warmer but higher frost risk
            if is_valley {
                modifiers.temperature_mod += config.valley_warmth;
                modifiers.wind_shelter = 0.8;
                modifiers.frost_risk = 0.7; // Cold air pools in valleys
            }

            // Ridge effects: cooler and exposed
            if is_ridge {
                modifiers.temperature_mod -= config.ridge_cooling;
                modifiers.wind_shelter = 0.1;
                modifiers.frost_risk = 0.2;
            }

            // South-facing slope bonus at high latitudes
            // (South in northern hemisphere gets more sun, North in southern)
            if latitude_normalized > 0.4 {
                let aspect = context.aspect;
                // Aspect: 0 = North, PI/2 = East, PI = South, 3PI/2 = West
                let south_facing = if in_northern_hemisphere {
                    // Northern hemisphere: south = PI
                    ((aspect - std::f32::consts::PI).abs() / std::f32::consts::PI).clamp(0.0, 1.0)
                } else {
                    // Southern hemisphere: north = 0 or 2*PI
                    let north_dist = aspect.abs().min((aspect - std::f32::consts::TAU).abs());
                    (1.0 - north_dist / std::f32::consts::PI).clamp(0.0, 1.0)
                };

                // Stronger effect at higher latitudes and steeper slopes
                let slope_factor = (context.gradient_magnitude / 20.0).clamp(0.0, 1.0);
                let lat_factor = ((latitude_normalized - 0.4) / 0.6).clamp(0.0, 1.0);

                modifiers.temperature_mod += config.south_slope_bonus * south_facing * slope_factor * lat_factor;
            }

            // Lake proximity moisture bonus
            let lake_dist = lake_distance.get(x, y);
            if *lake_dist < config.lake_effect_range as f32 {
                let proximity_factor = 1.0 - (*lake_dist / config.lake_effect_range as f32);
                modifiers.moisture_mod += config.lake_moisture_bonus * proximity_factor;
            }

            // Forest moisture retention
            if is_forest_biome(biome) {
                modifiers.moisture_mod += config.forest_moisture_retention;
            }

            // Set default values for non-valley/ridge areas
            if !is_valley && !is_ridge {
                modifiers.wind_shelter = 0.4;
                modifiers.frost_risk = 0.3;
            }

            microclimates.set(x, y, modifiers);
        }
    }

    microclimates
}

/// Compute distance from each tile to nearest lake
fn compute_lake_distance(
    water_body_map: &Tilemap<WaterBodyId>,
    water_bodies: &[WaterBody],
    width: usize,
    height: usize,
    max_distance: usize,
) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();

    // Find all lake tiles
    for y in 0..height {
        for x in 0..width {
            let wb_id = *water_body_map.get(x, y);
            if let Some(wb) = water_bodies.iter().find(|w| w.id == wb_id) {
                if wb.body_type == WaterBodyType::Lake {
                    distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                }
            }
        }
    }

    // BFS to compute distance
    while let Some((x, y, dist)) = queue.pop_front() {
        if dist >= max_distance as f32 {
            continue;
        }

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
            if new_dist < *distance.get(nx, ny) {
                distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }

    distance
}

/// Check if a biome is considered a forest type
fn is_forest_biome(biome: ExtendedBiome) -> bool {
    use ExtendedBiome::*;
    matches!(biome,
        BorealForest | TemperateForest | TemperateRainforest |
        TropicalForest | TropicalRainforest | DeadForest |
        CrystalForest | BioluminescentForest | MushroomForest |
        PetrifiedForest | AncientGrove | SiliconGrove
    )
}

// =============================================================================
// ADVANCED TERRAIN ANALYSIS
// =============================================================================

/// Analyze if a location is in a rain shadow
pub fn is_rain_shadow(
    heightmap: &Tilemap<f32>,
    x: usize,
    y: usize,
    wind_direction: (f32, f32),
    range: usize,
) -> (bool, f32) {
    let width = heightmap.width;
    let height = heightmap.height;
    let center_elev = *heightmap.get(x, y);

    // Look upwind for blocking mountains
    let upwind = (-wind_direction.0, -wind_direction.1);
    let mut max_blocking = 0.0f32;

    for step in 1..=range {
        let t = step as f32;
        let sx = (x as f32 + upwind.0 * t) as i32;
        let sy = (y as f32 + upwind.1 * t) as i32;

        let sx = sx.rem_euclid(width as i32) as usize;
        let sy = sy.clamp(0, height as i32 - 1) as usize;

        let blocking_elev = *heightmap.get(sx, sy);

        if blocking_elev > center_elev + 400.0 {
            let height_factor = ((blocking_elev - center_elev) / 2000.0).min(1.0);
            let dist_factor = 1.0 - (step as f32 / range as f32) * 0.5;
            max_blocking = max_blocking.max(height_factor * dist_factor);
        }
    }

    (max_blocking > 0.2, max_blocking)
}

/// Analyze local terrain ruggedness
pub fn terrain_ruggedness(heightmap: &Tilemap<f32>, x: usize, y: usize, radius: usize) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;
    let center = *heightmap.get(x, y);

    let mut variance_sum = 0.0f32;
    let mut count = 0;

    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let neighbor = *heightmap.get(nx, ny);
            let diff = (neighbor - center).abs();
            variance_sum += diff * diff;
            count += 1;
        }
    }

    if count > 0 {
        (variance_sum / count as f32).sqrt()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_microclimate_config_default() {
        let config = MicroclimateConfig::default();
        assert!(config.valley_warmth > 0.0);
        assert!(config.ridge_cooling > 0.0);
        assert!(config.lake_effect_range > 0);
    }

    #[test]
    fn test_modifiers_apply() {
        let modifiers = MicroclimateModifiers {
            temperature_mod: 2.0,
            moisture_mod: 0.1,
            wind_shelter: 0.5,
            frost_risk: 0.3,
        };

        assert_eq!(modifiers.apply_temperature(20.0), 22.0);
        assert_eq!(modifiers.apply_moisture(0.5), 0.6);
        assert_eq!(modifiers.apply_moisture(0.95), 1.0); // Clamped
    }
}
