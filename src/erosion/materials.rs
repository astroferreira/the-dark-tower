//! Material and rock hardness system for differentiated erosion rates.
//!
//! Different rock types erode at vastly different rates. Basalt and granite are
//! highly resistant, while sandstone and sedimentary deposits erode quickly.
//! This creates realistic features like mesas, canyons with hard cap rocks,
//! and resistant ridges.

use crate::plates::{Plate, PlateId, PlateType};
use crate::tilemap::Tilemap;
use noise::{NoiseFn, Perlin, Seedable};

/// Rock/material type affecting erosion resistance
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RockType {
    /// Volcanic rock, very hard (oceanic crust, volcanic islands)
    Basalt,
    /// Continental basement, hard
    Granite,
    /// Sedimentary, medium hardness
    #[default]
    Sandstone,
    /// Sedimentary, medium-soft (can form karst features)
    Limestone,
    /// Sedimentary, soft
    Shale,
    /// Unconsolidated deposits, very soft
    Sediment,
    /// Ice cover (for glacial regions)
    Ice,
}

impl RockType {
    /// Hardness factor (0.0 = instant erosion, 1.0 = nearly indestructible)
    pub fn hardness(&self) -> f32 {
        match self {
            RockType::Basalt => 0.95,
            RockType::Granite => 0.85,
            RockType::Sandstone => 0.5,
            RockType::Limestone => 0.4,
            RockType::Shale => 0.25,
            RockType::Sediment => 0.1,
            RockType::Ice => 0.05,
        }
    }

    /// How easily this rock breaks into transportable sediment (inverse of hardness)
    pub fn friability(&self) -> f32 {
        1.0 - self.hardness()
    }

    /// Get a representative RGB color for visualization
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            RockType::Basalt => (40, 40, 45),      // Dark gray-black
            RockType::Granite => (160, 150, 140),  // Light gray-pink
            RockType::Sandstone => (194, 154, 108), // Tan/brown
            RockType::Limestone => (210, 200, 180), // Cream/beige
            RockType::Shale => (100, 95, 90),       // Dark gray
            RockType::Sediment => (139, 119, 101),  // Brown
            RockType::Ice => (200, 220, 255),       // Light blue-white
        }
    }
}

/// Generate material map based on plate types and terrain features.
///
/// Material assignment logic:
/// - Oceanic plates: Basalt (volcanic origin)
/// - Continental interior: Granite (ancient basement)
/// - Mountain ranges (high stress): Granite/Basalt (uplifted/volcanic)
/// - Low-lying continental: Sandstone/Shale (sedimentary basins)
/// - Coastal/delta regions: Sediment (deposited material)
/// - High altitude cold regions: May have ice layer
pub fn generate_material_map(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    heightmap: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> Tilemap<RockType> {
    let width = plate_map.width;
    let height = plate_map.height;
    let mut materials = Tilemap::new_with(width, height, RockType::Sandstone);

    // Noise for natural variation in rock types
    let variation_noise = Perlin::new(1).set_seed(seed as u32 + 5555);
    let strata_noise = Perlin::new(1).set_seed(seed as u32 + 6666);

    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            let elevation = *heightmap.get(x, y);
            let stress = *stress_map.get(x, y);

            // Normalized coordinates for noise sampling
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;

            // Base noise values for variation
            let var = variation_noise.get([nx * 20.0, ny * 20.0]) as f32;
            let strata = strata_noise.get([nx * 50.0, ny * 50.0, elevation as f64 * 0.001]) as f32;

            let rock_type = if plate_id.is_none() {
                // No plate - deep ocean sediment
                RockType::Sediment
            } else {
                let plate = &plates[plate_id.0 as usize];

                match plate.plate_type {
                    PlateType::Oceanic => {
                        // Oceanic plates are primarily basalt
                        if elevation > 0.0 {
                            // Volcanic islands
                            RockType::Basalt
                        } else if elevation > -1000.0 {
                            // Shallow ocean - some sediment cover
                            if var > 0.3 {
                                RockType::Sediment
                            } else {
                                RockType::Basalt
                            }
                        } else {
                            // Deep ocean floor
                            RockType::Basalt
                        }
                    }
                    PlateType::Continental => {
                        if elevation < 0.0 {
                            // Continental shelf - sedimentary
                            RockType::Sediment
                        } else if stress > 0.3 {
                            // High stress zones - mountain building
                            // Mix of uplifted granite and metamorphic rocks
                            if var > 0.2 {
                                RockType::Granite
                            } else {
                                RockType::Basalt  // Volcanic intrusions
                            }
                        } else if elevation > 2000.0 {
                            // High mountains - exposed granite
                            RockType::Granite
                        } else if elevation > 500.0 {
                            // Highland regions - mixed
                            if strata > 0.3 {
                                RockType::Granite
                            } else if strata > -0.3 {
                                RockType::Sandstone
                            } else {
                                RockType::Limestone
                            }
                        } else if elevation < 50.0 {
                            // Near sea level - coastal/delta sediments
                            if var > 0.0 {
                                RockType::Sediment
                            } else {
                                RockType::Shale
                            }
                        } else {
                            // Lowlands and basins - sedimentary sequence
                            if strata > 0.4 {
                                RockType::Sandstone
                            } else if strata > 0.0 {
                                RockType::Limestone
                            } else if strata > -0.4 {
                                RockType::Shale
                            } else {
                                RockType::Sandstone
                            }
                        }
                    }
                }
            };

            materials.set(x, y, rock_type);
        }
    }

    materials
}

/// Generate a precomputed hardness map from materials for fast erosion lookups.
/// Applies Perlin noise to vary hardness within rock types, creating more natural, non-uniform erosion.
pub fn generate_hardness_map(materials: &Tilemap<RockType>, seed: u64) -> Tilemap<f32> {
    let width = materials.width;
    let height = materials.height;
    let mut hardness = Tilemap::new_with(width, height, 0.5f32);

    // Use noise to vary hardness (simulates local fractures, density changes)
    let noise = Perlin::new(1).set_seed(seed as u32 + 7777);

    for y in 0..height {
        for x in 0..width {
            let rock = *materials.get(x, y);
            let base_hardness = rock.hardness();

            // Normalized coordinates for noise
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;

            // Get noise value (-1.0 to 1.0)
            // Frequency 20.0 gives good local variation without being too high-frequency
            let n = noise.get([nx * 30.0, ny * 30.0]) as f32;

            // Apply noise to hardness
            // Variation of +/- 0.15 allows significant deviations but ensures rock type still matters
            // e.g. Sandstone (0.5) can range 0.35-0.65, overlapping slightly with Granite (0.85) at extremes
            let noisy_hardness = (base_hardness + n * 0.15).clamp(0.05, 1.0);

            hardness.set(x, y, noisy_hardness);
        }
    }

    hardness
}

/// Export material map as an RGB image.
pub fn export_material_map(
    materials: &Tilemap<RockType>,
    path: &str,
) -> Result<(), image::ImageError> {
    use image::{ImageBuffer, Rgb};

    let width = materials.width as u32;
    let height = materials.height as u32;

    let img = ImageBuffer::from_fn(width, height, |x, y| {
        let rock = *materials.get(x as usize, y as usize);
        let (r, g, b) = rock.color();
        Rgb([r, g, b])
    });

    img.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardness_ordering() {
        // Verify hardness is ordered correctly
        assert!(RockType::Basalt.hardness() > RockType::Granite.hardness());
        assert!(RockType::Granite.hardness() > RockType::Sandstone.hardness());
        assert!(RockType::Sandstone.hardness() > RockType::Limestone.hardness());
        assert!(RockType::Limestone.hardness() > RockType::Shale.hardness());
        assert!(RockType::Shale.hardness() > RockType::Sediment.hardness());
        assert!(RockType::Sediment.hardness() > RockType::Ice.hardness());
    }

    #[test]
    fn test_hardness_bounds() {
        for rock in [
            RockType::Basalt,
            RockType::Granite,
            RockType::Sandstone,
            RockType::Limestone,
            RockType::Shale,
            RockType::Sediment,
            RockType::Ice,
        ] {
            let h = rock.hardness();
            assert!(h >= 0.0 && h <= 1.0, "{:?} hardness out of bounds: {}", rock, h);
        }
    }

    #[test]
    fn test_friability_inverse() {
        for rock in [
            RockType::Basalt,
            RockType::Granite,
            RockType::Sandstone,
        ] {
            let h = rock.hardness();
            let f = rock.friability();
            assert!((h + f - 1.0).abs() < 0.001);
        }
    }
}
