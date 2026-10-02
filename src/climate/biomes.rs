//! Biome classification based on temperature, moisture, and elevation
//!
//! Provides altitudinal zonation and fuzzy transition logic.

use crate::tilemap::Tilemap;

/// Temperature drop per 1000m elevation (lapse rate in Celsius)
pub const ELEVATION_LAPSE_RATE: f32 = 6.5;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub fn classify_lowland(temperature: f32, moisture: f32) -> Biome {
        match (temperature, moisture) {
            // Freezing temperatures
            (t, _) if t < -10.0 => Biome::Ice,
            (t, _) if t < 0.0 => Biome::Tundra,

            // Cold temperatures (0 to 10°C)
            (t, m) if t < 10.0 => {
                if m > 0.45 {
                    Biome::BorealForest
                } else if m > 0.22 && t >= 3.0 {
                    // Cold steppe / prairie in moderate moisture
                    Biome::TemperateGrassland
                } else {
                    Biome::Tundra
                }
            }

            // Temperate temperatures (10 to 20°C)
            (t, m) if t < 20.0 => {
                if m > 0.65 {
                    Biome::TemperateRainforest
                } else if m > 0.38 {
                    Biome::TemperateForest
                } else {
                    Biome::TemperateGrassland
                }
            }

            // Warm/tropical temperatures (>= 20°C)
            (_, m) => {
                if m > 0.68 {
                    Biome::TropicalRainforest
                } else if m > 0.40 {
                    Biome::TropicalForest
                } else if m > 0.18 {
                    Biome::Savanna
                } else {
                    Biome::Desert
                }
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

        // Determine climate zone from sea-level temperature
        let sea_level_temp = temperature + (elevation / 1000.0) * ELEVATION_LAPSE_RATE;
        let is_tropical = sea_level_temp > 22.0;
        let is_temperate = sea_level_temp > 8.0 && sea_level_temp <= 22.0;

        // 1. High-altitude alpine & snowline checks (governed by actual local temperature & high altitude)
        let is_mountain = elevation > 1000.0;
        if is_mountain && ((elevation > 1400.0 && temperature < -8.0) || (elevation > 2200.0 && temperature < -4.0) || elevation > 4200.0) {
            return Biome::SnowyPeaks;
        }

        // 2. Mountain altitudinal zones (only for distinct elevated mountain terrain > 1000m)
        if elevation > 1000.0 {
            if is_tropical {
                if elevation > 4000.0 || temperature < 0.0 {
                    return Biome::AlpineTundra;
                } else if elevation > 3000.0 {
                    return if moisture > 0.45 { Biome::Paramo } else { Biome::AlpineTundra };
                } else if elevation > 2000.0 {
                    return if moisture > 0.45 { Biome::CloudForest } else { Biome::MontaneForest };
                } else if moisture > 0.35 {
                    return Biome::MontaneForest;
                }
            } else if is_temperate {
                if elevation > 2800.0 || temperature < -2.0 {
                    return Biome::AlpineTundra;
                } else if elevation > 1800.0 || temperature < 1.0 {
                    return if moisture > 0.35 { Biome::AlpineMeadow } else { Biome::AlpineTundra };
                } else if moisture > 0.35 {
                    return Biome::SubalpineForest;
                }
            } else {
                // Polar/subpolar mountain peaks
                if elevation > 1600.0 || temperature < -3.0 {
                    return Biome::AlpineTundra;
                }
            }
        }

        // 3. Lowlands, rolling hills, and plateaus:
        // Use physical temperature and moisture classification
        Self::classify_lowland(temperature, moisture)
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
            Biome::MontaneForest => (45, 90, 55),
            Biome::CloudForest => (60, 110, 80),
            Biome::Paramo => (160, 155, 120),
            Biome::SubalpineForest => (40, 70, 45),
            Biome::AlpineMeadow => (130, 160, 100),
            Biome::AlpineTundra => (140, 140, 130),
            Biome::SnowyPeaks => (255, 255, 255),
        }
    }

    /// Classify biome with fuzzy boundaries for smooth transitions
    pub fn classify_fuzzy(elevation: f32, temperature: f32, moisture: f32) -> FuzzyBiomeResult {
        const ELEV_TRANSITION: f32 = 150.0;
        const MOIST_TRANSITION: f32 = 0.1;

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

        let primary = Self::classify(elevation, temperature, moisture);
        let sea_level_temp = temperature + (elevation / 1000.0) * ELEVATION_LAPSE_RATE;
        let is_tropical = sea_level_temp > 22.0;
        let is_temperate = sea_level_temp > 8.0 && sea_level_temp <= 22.0;

        let elev_thresholds: Vec<(f32, Biome, Biome)> = if is_tropical {
            vec![
                (4500.0, Biome::AlpineTundra, Biome::SnowyPeaks),
                (3000.0, Biome::CloudForest, Biome::Paramo),
                (2000.0, Biome::MontaneForest, Biome::CloudForest),
                (1000.0, Self::classify_lowland(temperature, moisture), Biome::MontaneForest),
            ]
        } else if is_temperate {
            vec![
                (3200.0, Biome::AlpineTundra, Biome::SnowyPeaks),
                (2200.0, Biome::AlpineMeadow, Biome::AlpineTundra),
                (1600.0, Biome::SubalpineForest, Biome::AlpineMeadow),
                (1000.0, Self::classify_lowland(temperature, moisture), Biome::SubalpineForest),
            ]
        } else {
            vec![
                (2200.0, Biome::AlpineTundra, Biome::SnowyPeaks),
                (1400.0, Self::classify_lowland(temperature, moisture), Biome::AlpineTundra),
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

        let moist_thresholds = [0.2, 0.4, 0.5, 0.7];
        for threshold in moist_thresholds {
            if (moisture - threshold).abs() < MOIST_TRANSITION {
                let blend = smooth_step(
                    threshold - MOIST_TRANSITION,
                    threshold + MOIST_TRANSITION,
                    moisture
                );
                if blend > 0.0 && blend < 1.0 {
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
