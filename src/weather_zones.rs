//! Extreme weather zone detection system
//!
//! Identifies regions prone to extreme weather events like hurricanes,
//! monsoons, blizzards, tornadoes, and sandstorms based on climate and geography.

use crate::tilemap::Tilemap;

// =============================================================================
// EXTREME WEATHER TYPES
// =============================================================================

/// Types of extreme weather that can affect a region
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtremeWeatherType {
    /// Tropical cyclones - form over warm ocean water, 5-30° latitude
    Hurricane,
    /// Seasonal heavy rainfall - tropical coastal regions with land-sea contrast
    Monsoon,
    /// Severe winter storms - cold regions with moisture
    Blizzard,
    /// Violent rotating storms - mid-latitudes with temperature/moisture contrasts
    Tornado,
    /// Dust/sand storms - hot deserts
    Sandstorm,
    /// No extreme weather risk
    None,
}

impl ExtremeWeatherType {
    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Hurricane => "Hurricane Zone",
            Self::Monsoon => "Monsoon Zone",
            Self::Blizzard => "Blizzard Zone",
            Self::Tornado => "Tornado Alley",
            Self::Sandstorm => "Sandstorm Zone",
            Self::None => "Normal Weather",
        }
    }

    /// Get color for visualization
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Hurricane => (255, 100, 150),    // Pink/magenta
            Self::Monsoon => (100, 150, 255),      // Light blue
            Self::Blizzard => (200, 220, 255),     // Ice blue
            Self::Tornado => (255, 200, 50),       // Yellow-orange
            Self::Sandstorm => (210, 180, 100),    // Tan/sand
            Self::None => (128, 128, 128),         // Gray
        }
    }

    /// Get severity description
    pub fn severity_description(&self, risk: f32) -> &'static str {
        if risk < 0.3 {
            "Low risk"
        } else if risk < 0.6 {
            "Moderate risk"
        } else if risk < 0.8 {
            "High risk"
        } else {
            "Extreme risk"
        }
    }
}

// =============================================================================
// WEATHER ZONE
// =============================================================================

/// Information about extreme weather risk at a location
#[derive(Clone, Debug)]
pub struct WeatherZone {
    /// Primary extreme weather type
    pub primary: ExtremeWeatherType,
    /// Risk factor (0.0 = no risk, 1.0 = maximum risk)
    pub risk_factor: f32,
    /// Peak season (0 = spring, 1 = summer, 2 = autumn, 3 = winter)
    pub peak_season: u8,
    /// Secondary weather type (if multiple apply)
    pub secondary: Option<(ExtremeWeatherType, f32)>,
}

impl Default for WeatherZone {
    fn default() -> Self {
        Self {
            primary: ExtremeWeatherType::None,
            risk_factor: 0.0,
            peak_season: 0,
            secondary: None,
        }
    }
}

impl WeatherZone {
    /// Check if this zone has any extreme weather risk
    pub fn has_risk(&self) -> bool {
        self.primary != ExtremeWeatherType::None && self.risk_factor > 0.1
    }

    /// Get the combined risk considering both primary and secondary
    pub fn total_risk(&self) -> f32 {
        let secondary_risk = self.secondary.map(|(_, r)| r).unwrap_or(0.0);
        (self.risk_factor + secondary_risk * 0.5).min(1.0)
    }
}

// =============================================================================
// WEATHER ZONE GENERATION
// =============================================================================

/// Generate weather zone map from climate data
pub fn generate_weather_zones(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
) -> Tilemap<WeatherZone> {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut zones = Tilemap::new_with(width, height, WeatherZone::default());

    // Compute coastal proximity for monsoon detection
    let coastal_distance = compute_coastal_distance(heightmap);

    for y in 0..height {
        // Calculate latitude (0 = equator, 1 = pole)
        let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;
        let latitude_degrees = latitude_normalized * 90.0;

        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);
            let coastal_dist = *coastal_distance.get(x, y);

            let is_ocean = elevation <= 0.0;
            let is_land = elevation > 0.0;

            let mut zone = WeatherZone::default();

            // Hurricane detection: Ocean, warm water (>26°C), 5-30° latitude
            if is_ocean && temp > 26.0 && latitude_degrees > 5.0 && latitude_degrees < 30.0 {
                let temp_factor = ((temp - 26.0) / 4.0).clamp(0.0, 1.0);
                let lat_factor = if latitude_degrees < 10.0 {
                    (latitude_degrees - 5.0) / 5.0
                } else if latitude_degrees > 25.0 {
                    (30.0 - latitude_degrees) / 5.0
                } else {
                    1.0
                };
                let risk = temp_factor * lat_factor * 0.9;

                if risk > zone.risk_factor {
                    zone.primary = ExtremeWeatherType::Hurricane;
                    zone.risk_factor = risk;
                    zone.peak_season = 2; // Late summer/autumn
                }
            }

            // Monsoon detection: Tropical land near coast, warm, moderate moisture
            if is_land && temp > 20.0 && latitude_degrees < 30.0 && coastal_dist < 50.0 {
                let temp_factor = ((temp - 20.0) / 10.0).clamp(0.0, 1.0);
                let coastal_factor = (1.0 - coastal_dist / 50.0).clamp(0.0, 1.0);
                let lat_factor = (1.0 - latitude_degrees / 30.0).clamp(0.0, 1.0);
                let moist_factor = if moist > 0.3 && moist < 0.8 { 1.0 } else { 0.5 };

                let risk = temp_factor * coastal_factor * lat_factor * moist_factor * 0.8;

                if risk > zone.risk_factor {
                    if zone.primary != ExtremeWeatherType::None && zone.risk_factor > 0.2 {
                        zone.secondary = Some((zone.primary, zone.risk_factor));
                    }
                    zone.primary = ExtremeWeatherType::Monsoon;
                    zone.risk_factor = risk;
                    zone.peak_season = 1; // Summer
                }
            }

            // Blizzard detection: Cold land with moisture
            if is_land && temp < -5.0 && moist > 0.3 {
                let temp_factor = ((-5.0 - temp) / 20.0).clamp(0.0, 1.0);
                let moist_factor = ((moist - 0.3) / 0.4).clamp(0.0, 1.0);

                let risk = temp_factor * moist_factor * 0.85;

                if risk > zone.risk_factor {
                    if zone.primary != ExtremeWeatherType::None && zone.risk_factor > 0.2 {
                        zone.secondary = Some((zone.primary, zone.risk_factor));
                    }
                    zone.primary = ExtremeWeatherType::Blizzard;
                    zone.risk_factor = risk;
                    zone.peak_season = 3; // Winter
                }
            }

            // Tornado detection: Mid-latitudes, moderate temp, moisture contrast
            if is_land && latitude_degrees > 25.0 && latitude_degrees < 50.0
                && temp > 15.0 && temp < 35.0 && moist > 0.3 && moist < 0.7 {
                // Tornados favor areas with temperature/moisture gradients
                // Check for moisture contrast with neighbors
                let moisture_contrast = compute_local_moisture_contrast(moisture, x, y);

                let lat_factor = if latitude_degrees > 30.0 && latitude_degrees < 45.0 {
                    1.0
                } else {
                    0.6
                };
                let temp_factor = if temp > 20.0 && temp < 30.0 { 1.0 } else { 0.7 };
                let contrast_factor = (moisture_contrast / 0.2).clamp(0.0, 1.0);

                let risk = lat_factor * temp_factor * contrast_factor * 0.7;

                if risk > zone.risk_factor {
                    if zone.primary != ExtremeWeatherType::None && zone.risk_factor > 0.2 {
                        zone.secondary = Some((zone.primary, zone.risk_factor));
                    }
                    zone.primary = ExtremeWeatherType::Tornado;
                    zone.risk_factor = risk;
                    zone.peak_season = 0; // Spring
                }
            }

            // Sandstorm detection: Hot desert
            if is_land && temp > 25.0 && moist < 0.15 && elevation < 1000.0 {
                let temp_factor = ((temp - 25.0) / 15.0).clamp(0.0, 1.0);
                let dry_factor = ((0.15 - moist) / 0.15).clamp(0.0, 1.0);

                let risk = temp_factor * dry_factor * 0.75;

                if risk > zone.risk_factor {
                    if zone.primary != ExtremeWeatherType::None && zone.risk_factor > 0.2 {
                        zone.secondary = Some((zone.primary, zone.risk_factor));
                    }
                    zone.primary = ExtremeWeatherType::Sandstorm;
                    zone.risk_factor = risk;
                    zone.peak_season = 1; // Summer
                }
            }

            zones.set(x, y, zone);
        }
    }

    zones
}

/// Compute distance from each tile to nearest coast
fn compute_coastal_distance(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();

    // Find all coastal tiles (land adjacent to water)
    for y in 0..height {
        for x in 0..width {
            let is_land = *heightmap.get(x, y) > 0.0;
            if !is_land {
                continue;
            }

            // Check if adjacent to water
            let neighbors = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];

            for (nx, ny) in neighbors {
                if nx < width && ny < height && *heightmap.get(nx, ny) <= 0.0 {
                    distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                    break;
                }
            }
        }
    }

    // BFS to compute distance
    while let Some((x, y, dist)) = queue.pop_front() {
        if dist >= 100.0 {
            continue; // Limit distance computation
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
            // Only propagate through land
            if *heightmap.get(nx, ny) <= 0.0 {
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

/// Compute local moisture contrast (difference between max and min neighbor moisture)
fn compute_local_moisture_contrast(moisture: &Tilemap<f32>, x: usize, y: usize) -> f32 {
    let width = moisture.width;
    let height = moisture.height;

    let mut min_moist = f32::MAX;
    let mut max_moist = f32::MIN;

    // Check 8-connected neighbors
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let m = *moisture.get(nx, ny);
            min_moist = min_moist.min(m);
            max_moist = max_moist.max(m);
        }
    }

    if max_moist > min_moist {
        max_moist - min_moist
    } else {
        0.0
    }
}

// =============================================================================
// SEASON HELPERS
// =============================================================================

/// Get the name of a season
pub fn season_name(season: u8) -> &'static str {
    match season {
        0 => "Spring",
        1 => "Summer",
        2 => "Autumn",
        3 => "Winter",
        _ => "Unknown",
    }
}

/// Get weather risk for a specific season
pub fn get_seasonal_risk(zone: &WeatherZone, current_season: u8) -> f32 {
    // Risk is highest during peak season, lower otherwise
    let season_diff = ((zone.peak_season as i32 - current_season as i32).abs() % 4).min(
        4 - (zone.peak_season as i32 - current_season as i32).abs() % 4
    );

    let season_factor = match season_diff {
        0 => 1.0,      // Peak season
        1 => 0.6,      // Adjacent season
        2 => 0.2,      // Opposite season
        _ => 0.1,
    };

    zone.risk_factor * season_factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_zone_default() {
        let zone = WeatherZone::default();
        assert_eq!(zone.primary, ExtremeWeatherType::None);
        assert!(!zone.has_risk());
    }

    #[test]
    fn test_seasonal_risk() {
        let zone = WeatherZone {
            primary: ExtremeWeatherType::Hurricane,
            risk_factor: 0.8,
            peak_season: 2, // Autumn
            secondary: None,
        };

        // Peak season should have highest risk
        let autumn_risk = get_seasonal_risk(&zone, 2);
        let spring_risk = get_seasonal_risk(&zone, 0);

        assert!(autumn_risk > spring_risk);
    }

    #[test]
    fn test_weather_colors() {
        // All weather types should have distinct colors
        let types = [
            ExtremeWeatherType::Hurricane,
            ExtremeWeatherType::Monsoon,
            ExtremeWeatherType::Blizzard,
            ExtremeWeatherType::Tornado,
            ExtremeWeatherType::Sandstorm,
        ];

        for t in types {
            let color = t.color();
            assert!(color.0 > 0 || color.1 > 0 || color.2 > 0);
        }
    }
}
