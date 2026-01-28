//! Seasonal climate variation system
//!
//! Provides temperature and moisture variation by season, including:
//! - Latitude-dependent temperature amplitude
//! - Climate-type-dependent moisture phase (Mediterranean vs. tropical)
//! - Hemisphere-aware season offsets

use serde::{Serialize, Deserialize};
use crate::tilemap::Tilemap;
use crate::biomes::ExtendedBiome;

// =============================================================================
// SEASON DEFINITIONS
// =============================================================================

/// The four seasons
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Season {
    Spring = 0,
    Summer = 1,
    Autumn = 2,
    Winter = 3,
}

impl Season {
    /// Get all seasons in order
    pub fn all() -> &'static [Season] {
        &[Season::Spring, Season::Summer, Season::Autumn, Season::Winter]
    }

    /// Get the next season
    pub fn next(&self) -> Season {
        match self {
            Season::Spring => Season::Summer,
            Season::Summer => Season::Autumn,
            Season::Autumn => Season::Winter,
            Season::Winter => Season::Spring,
        }
    }

    /// Get the previous season
    pub fn prev(&self) -> Season {
        match self {
            Season::Spring => Season::Winter,
            Season::Summer => Season::Spring,
            Season::Autumn => Season::Summer,
            Season::Winter => Season::Autumn,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }

    /// Convert to fraction of year (0.0 = start of spring, 0.25 = summer, etc.)
    pub fn to_year_fraction(&self) -> f32 {
        (*self as u8) as f32 / 4.0
    }

    /// Create from year fraction (0.0-1.0)
    pub fn from_year_fraction(fraction: f32) -> Season {
        let normalized = fraction.rem_euclid(1.0);
        match (normalized * 4.0) as u8 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// Get the opposite season in the other hemisphere
    pub fn opposite(&self) -> Season {
        match self {
            Season::Spring => Season::Autumn,
            Season::Summer => Season::Winter,
            Season::Autumn => Season::Spring,
            Season::Winter => Season::Summer,
        }
    }
}

// =============================================================================
// SEASONAL CLIMATE DATA
// =============================================================================

/// Climate type affecting moisture seasonality
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClimateSeasonType {
    /// Equatorial - minimal seasonal variation
    Equatorial,
    /// Tropical - wet summers, dry winters
    Tropical,
    /// Mediterranean - dry summers, wet winters
    Mediterranean,
    /// Continental - moderate summer rain, cold dry winters
    Continental,
    /// Oceanic - even moisture year-round
    Oceanic,
    /// Arctic/Antarctic - extreme cold winters
    Polar,
}

impl ClimateSeasonType {
    /// Get moisture phase offset (0 = wet summer, PI = wet winter)
    pub fn moisture_phase(&self) -> f32 {
        match self {
            Self::Equatorial => 0.0,     // Minimal variation
            Self::Tropical => 0.0,       // Wet summer (monsoon)
            Self::Mediterranean => std::f32::consts::PI, // Wet winter
            Self::Continental => 0.0,    // Slightly wet summer
            Self::Oceanic => 0.0,        // Even, slight summer
            Self::Polar => 0.0,          // Cold, little moisture
        }
    }

    /// Get moisture amplitude (how much moisture varies seasonally)
    pub fn moisture_amplitude(&self) -> f32 {
        match self {
            Self::Equatorial => 0.05,    // Very little variation
            Self::Tropical => 0.25,      // Strong monsoon effect
            Self::Mediterranean => 0.20, // Distinct wet/dry seasons
            Self::Continental => 0.10,   // Moderate variation
            Self::Oceanic => 0.05,       // Stable maritime influence
            Self::Polar => 0.08,         // Low variation (always dry)
        }
    }
}

/// Precomputed seasonal climate data for efficient runtime queries
#[derive(Clone)]
pub struct SeasonalClimate {
    /// Base temperature (annual mean)
    pub base_temperature: Tilemap<f32>,
    /// Base moisture (annual mean)
    pub base_moisture: Tilemap<f32>,
    /// Temperature amplitude per tile (half of annual range)
    pub temp_amplitude: Tilemap<f32>,
    /// Moisture amplitude per tile (half of annual range)
    pub moisture_amplitude: Tilemap<f32>,
    /// Moisture phase per tile (for Mediterranean vs tropical patterns)
    pub moisture_phase: Tilemap<f32>,
    /// Climate season type per tile
    pub climate_type: Tilemap<ClimateSeasonType>,
}

impl SeasonalClimate {
    /// Get temperature for a specific season
    pub fn get_temperature(&self, x: usize, y: usize, season: Season, is_northern_hemisphere: bool) -> f32 {
        let base = *self.base_temperature.get(x, y);
        let amplitude = *self.temp_amplitude.get(x, y);

        // Convert season to sinusoidal phase (summer = peak warmth)
        let mut phase = match season {
            Season::Summer => 0.0,          // Peak warmth
            Season::Spring => -std::f32::consts::FRAC_PI_2,
            Season::Autumn => std::f32::consts::FRAC_PI_2,
            Season::Winter => std::f32::consts::PI, // Peak cold
        };

        // Flip for southern hemisphere
        if !is_northern_hemisphere {
            phase += std::f32::consts::PI;
        }

        base + amplitude * phase.cos()
    }

    /// Get moisture for a specific season
    pub fn get_moisture(&self, x: usize, y: usize, season: Season, is_northern_hemisphere: bool) -> f32 {
        let base = *self.base_moisture.get(x, y);
        let amplitude = *self.moisture_amplitude.get(x, y);
        let phase_offset = *self.moisture_phase.get(x, y);

        // Convert season to sinusoidal phase
        let mut phase = match season {
            Season::Summer => 0.0,
            Season::Spring => -std::f32::consts::FRAC_PI_2,
            Season::Autumn => std::f32::consts::FRAC_PI_2,
            Season::Winter => std::f32::consts::PI,
        };

        // Apply moisture phase offset (Mediterranean shifts wet season to winter)
        phase += phase_offset;

        // Flip for southern hemisphere
        if !is_northern_hemisphere {
            phase += std::f32::consts::PI;
        }

        (base + amplitude * phase.cos()).clamp(0.0, 1.0)
    }

    /// Get seasonal biome (may differ from annual biome near boundaries)
    pub fn get_seasonal_biome(
        &self,
        x: usize,
        y: usize,
        elevation: f32,
        season: Season,
        is_northern_hemisphere: bool,
    ) -> crate::climate::Biome {
        let temp = self.get_temperature(x, y, season, is_northern_hemisphere);
        let moist = self.get_moisture(x, y, season, is_northern_hemisphere);
        crate::climate::Biome::classify(elevation, temp, moist)
    }

    /// Check if there's significant seasonal biome change
    pub fn has_seasonal_biome_change(&self, x: usize, y: usize, elevation: f32, is_northern_hemisphere: bool) -> bool {
        let summer_biome = self.get_seasonal_biome(x, y, elevation, Season::Summer, is_northern_hemisphere);
        let winter_biome = self.get_seasonal_biome(x, y, elevation, Season::Winter, is_northern_hemisphere);
        summer_biome != winter_biome
    }
}

// =============================================================================
// SEASONAL CLIMATE GENERATION
// =============================================================================

/// Generate seasonal climate data from base climate maps
pub fn generate_seasonal_climate(
    base_temperature: &Tilemap<f32>,
    base_moisture: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
) -> SeasonalClimate {
    let width = base_temperature.width;
    let height = base_temperature.height;

    let mut temp_amplitude = Tilemap::new_with(width, height, 0.0f32);
    let mut moisture_amplitude = Tilemap::new_with(width, height, 0.0f32);
    let mut moisture_phase = Tilemap::new_with(width, height, 0.0f32);
    let mut climate_type = Tilemap::new_with(width, height, ClimateSeasonType::Continental);

    for y in 0..height {
        // Calculate latitude (0 = equator, 1 = pole)
        let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;
        let latitude_degrees = latitude_normalized * 90.0;

        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            let base_temp = *base_temperature.get(x, y);
            let base_moist = *base_moisture.get(x, y);

            // Skip ocean (minimal seasonal variation in surface temp)
            if elevation <= 0.0 {
                temp_amplitude.set(x, y, 2.0); // Ocean has low variation
                moisture_amplitude.set(x, y, 0.02);
                climate_type.set(x, y, ClimateSeasonType::Oceanic);
                continue;
            }

            // Determine climate season type based on location and climate
            let ct = classify_climate_type(latitude_degrees, base_temp, base_moist, elevation);
            climate_type.set(x, y, ct);

            // Temperature amplitude increases with latitude
            // Near equator: ~2°C variation, at 60°: ~15°C, polar: ~20°C
            let lat_temp_amp = if latitude_normalized < 0.2 {
                2.0 + latitude_normalized * 10.0
            } else if latitude_normalized < 0.7 {
                4.0 + (latitude_normalized - 0.2) * 22.0 // 4 to 15
            } else {
                15.0 + (latitude_normalized - 0.7) * 16.7 // 15 to 20
            };

            // Continental interiors have higher amplitude than coastal
            // (approximated by base moisture - drier = more continental)
            let continental_factor = 1.0 + (1.0 - base_moist) * 0.3;

            // High elevation reduces amplitude slightly
            let elevation_factor = if elevation > 2000.0 {
                0.85
            } else if elevation > 1000.0 {
                0.92
            } else {
                1.0
            };

            temp_amplitude.set(x, y, lat_temp_amp * continental_factor * elevation_factor);

            // Moisture amplitude and phase from climate type
            moisture_amplitude.set(x, y, ct.moisture_amplitude() * base_moist);
            moisture_phase.set(x, y, ct.moisture_phase());
        }
    }

    SeasonalClimate {
        base_temperature: base_temperature.clone(),
        base_moisture: base_moisture.clone(),
        temp_amplitude,
        moisture_amplitude,
        moisture_phase,
        climate_type,
    }
}

/// Classify climate season type based on conditions
fn classify_climate_type(
    latitude: f32,
    temperature: f32,
    moisture: f32,
    elevation: f32,
) -> ClimateSeasonType {
    // Polar regions
    if latitude > 66.0 || temperature < -10.0 {
        return ClimateSeasonType::Polar;
    }

    // Equatorial (within ~15° of equator, warm, wet)
    if latitude < 15.0 && temperature > 20.0 && moisture > 0.5 {
        return ClimateSeasonType::Equatorial;
    }

    // Tropical (15-30°, warm, with seasonal moisture)
    if latitude < 30.0 && temperature > 18.0 && moisture > 0.3 {
        return ClimateSeasonType::Tropical;
    }

    // Mediterranean (30-45°, warm summers, moderate moisture, coastal typically)
    if latitude > 30.0 && latitude < 45.0 && temperature > 12.0 && moisture > 0.3 && moisture < 0.7 {
        // Mediterranean climates are typically on western continental coasts
        // We approximate this - in a full system, would check coastal proximity
        return ClimateSeasonType::Mediterranean;
    }

    // Oceanic (high moisture, moderate temps, typically maritime)
    if moisture > 0.6 && temperature > 5.0 && temperature < 20.0 {
        return ClimateSeasonType::Oceanic;
    }

    // Default to continental
    ClimateSeasonType::Continental
}

// =============================================================================
// SEASONAL EFFECTS
// =============================================================================

/// Information about seasonal conditions at a location
#[derive(Clone, Debug)]
pub struct SeasonalConditions {
    /// Current temperature
    pub temperature: f32,
    /// Current moisture
    pub moisture: f32,
    /// Growing season factor (0 = dormant, 1 = peak growth)
    pub growing_factor: f32,
    /// Snow cover factor (0 = none, 1 = full)
    pub snow_cover: f32,
    /// Frost risk (0 = none, 1 = certain)
    pub frost_risk: f32,
    /// Day length factor (0 = polar night, 1 = midnight sun, 0.5 = equinox)
    pub day_length: f32,
}

/// Calculate detailed seasonal conditions
pub fn calculate_seasonal_conditions(
    seasonal_climate: &SeasonalClimate,
    x: usize,
    y: usize,
    elevation: f32,
    season: Season,
    map_height: usize,
) -> SeasonalConditions {
    let is_northern = y < map_height / 2;
    let latitude_normalized = (y as f32 / map_height as f32 - 0.5).abs() * 2.0;

    let temperature = seasonal_climate.get_temperature(x, y, season, is_northern);
    let moisture = seasonal_climate.get_moisture(x, y, season, is_northern);

    // Growing season (temp > 5°C and not too dry)
    let growing_factor = if temperature > 5.0 && moisture > 0.15 {
        ((temperature - 5.0) / 15.0).clamp(0.0, 1.0) * ((moisture - 0.15) / 0.35).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Snow cover (accumulates in winter when cold, melts in warmer seasons)
    let snow_cover = if elevation > 0.0 && temperature < 0.0 {
        ((-temperature) / 10.0).clamp(0.0, 1.0) * (moisture).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Frost risk (highest in winter, spring, autumn at mid-high latitudes)
    let frost_risk = if temperature < 5.0 {
        ((5.0 - temperature) / 15.0).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Day length varies by latitude and season
    let day_length = calculate_day_length(latitude_normalized, season, is_northern);

    SeasonalConditions {
        temperature,
        moisture,
        growing_factor,
        snow_cover,
        frost_risk,
        day_length,
    }
}

/// Calculate approximate day length factor
fn calculate_day_length(latitude_normalized: f32, season: Season, is_northern: bool) -> f32 {
    // Simplified model: day length varies with latitude and season
    // At equator: always ~0.5 (12 hours)
    // At poles: 0 in winter, 1 in summer

    let seasonal_tilt = match season {
        Season::Summer => 1.0,
        Season::Spring | Season::Autumn => 0.5,
        Season::Winter => 0.0,
    };

    // Flip for southern hemisphere
    let adjusted_tilt = if is_northern { seasonal_tilt } else { 1.0 - seasonal_tilt };

    // At equator, day length is always 0.5
    // At poles, it varies from 0 to 1
    let equator_length = 0.5;
    let latitude_effect = latitude_normalized * (adjusted_tilt - 0.5);

    (equator_length + latitude_effect).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_season_cycle() {
        let s = Season::Spring;
        assert_eq!(s.next(), Season::Summer);
        assert_eq!(s.next().next(), Season::Autumn);
        assert_eq!(s.next().next().next(), Season::Winter);
        assert_eq!(s.next().next().next().next(), Season::Spring);
    }

    #[test]
    fn test_season_opposite() {
        assert_eq!(Season::Summer.opposite(), Season::Winter);
        assert_eq!(Season::Spring.opposite(), Season::Autumn);
    }

    #[test]
    fn test_climate_type_phase() {
        // Mediterranean should have opposite phase from tropical
        let med = ClimateSeasonType::Mediterranean;
        let trop = ClimateSeasonType::Tropical;
        assert!((med.moisture_phase() - trop.moisture_phase()).abs() > 2.0);
    }

    #[test]
    fn test_day_length_equator() {
        // At equator, day length should always be ~0.5
        for season in Season::all() {
            let dl = calculate_day_length(0.0, *season, true);
            assert!((dl - 0.5).abs() < 0.01);
        }
    }
}
