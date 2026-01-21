//! World → Local geology derivation (Dwarf Fortress style).
//!
//! Transforms world-scale data (elevation, biome, temperature, moisture, stress)
//! into detailed local geology with proper z-level structure.

use crate::biomes::ExtendedBiome;
use crate::zlevel::{self, CAVERN_1_MIN, CAVERN_2_MIN, CAVERN_3_MIN};
use crate::world::WorldData;
use crate::water_bodies::WaterBodyType;

use super::local::{Material, SoilType, StoneType};

/// Parameters derived from world data for local chunk generation
#[derive(Clone, Debug)]
pub struct GeologyParams {
    /// Surface z-level (from world surface_z)
    pub surface_z: i16,
    /// Biome at this world tile
    pub biome: ExtendedBiome,
    /// Temperature in Celsius
    pub temperature: f32,
    /// Moisture (0.0-1.0)
    pub moisture: f32,
    /// Tectonic stress (-1.0 divergent to +1.0 convergent)
    pub stress: f32,
    /// Whether this tile is volcanic
    pub is_volcanic: bool,
    /// Whether this tile has water body
    pub water_body_type: WaterBodyType,
    /// Soil depth in z-levels
    pub soil_depth: i16,
    /// Primary stone type
    pub primary_stone: StoneType,
    /// Secondary stone type (for variety)
    pub secondary_stone: StoneType,
    /// Cavern presence flags [cavern1, cavern2, cavern3]
    pub has_caverns: [bool; 3],
    /// Whether magma sea is present at deep levels
    pub has_magma: bool,
    /// Aquifer depth (z-level where aquifer starts, or None)
    pub aquifer_z: Option<i16>,
}

impl GeologyParams {
    /// Get the z-level where solid rock ends and soil begins
    pub fn rock_surface_z(&self) -> i16 {
        self.surface_z - self.soil_depth
    }

    /// Check if a z-level is underground (below surface)
    pub fn is_underground(&self, z: i16) -> bool {
        z < self.surface_z
    }

    /// Check if a z-level is in the soil layer
    pub fn is_soil_layer(&self, z: i16) -> bool {
        z >= self.rock_surface_z() && z < self.surface_z
    }

    /// Check if a z-level is in the stone layer (below soil, above caverns)
    pub fn is_stone_layer(&self, z: i16) -> bool {
        z < self.rock_surface_z() && z >= CAVERN_1_MIN as i16
    }

    /// Get the cavern layer (0, 1, 2) for a z-level, or None if not in a cavern range
    pub fn cavern_layer(&self, z: i16) -> Option<usize> {
        let z32 = z as i32;
        if z32 >= CAVERN_1_MIN && z32 <= zlevel::CAVERN_1_MAX && self.has_caverns[0] {
            Some(0)
        } else if z32 >= CAVERN_2_MIN && z32 <= zlevel::CAVERN_2_MAX && self.has_caverns[1] {
            Some(1)
        } else if z32 >= CAVERN_3_MIN && z32 <= zlevel::CAVERN_3_MAX && self.has_caverns[2] {
            Some(2)
        } else {
            None
        }
    }
}

/// Derive geology parameters from world data at a specific world tile
pub fn derive_geology(world: &WorldData, world_x: usize, world_y: usize) -> GeologyParams {
    let surface_z = *world.surface_z.get(world_x, world_y) as i16;
    let biome = *world.biomes.get(world_x, world_y);
    let temperature = *world.temperature.get(world_x, world_y);
    let moisture = *world.moisture.get(world_x, world_y);
    let stress = *world.stress_map.get(world_x, world_y);

    // Determine water body type
    let water_body_id = *world.water_body_map.get(world_x, world_y);
    let water_body_type = world.water_bodies
        .iter()
        .find(|wb| wb.id == water_body_id)
        .map(|wb| wb.body_type)
        .unwrap_or(WaterBodyType::None);

    // Derive soil depth from biome and moisture
    let soil_depth = derive_soil_depth(biome, moisture);

    // Derive stone types from stress and temperature
    let (primary_stone, secondary_stone) = derive_stone_types(stress, temperature, biome);

    // Check for volcanic activity (high stress + specific biomes)
    let is_volcanic = stress > 0.6 || matches!(biome,
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::ObsidianFields |
        ExtendedBiome::Geysers |
        ExtendedBiome::SulfurVents
    );

    // Check cavern presence from world zlevel data
    let has_caverns = check_cavern_presence(world, world_x, world_y, surface_z);

    // Magma is present in volcanic areas or very deep with high stress
    let has_magma = is_volcanic || stress > 0.5;

    // Aquifer presence based on moisture and surface type
    let aquifer_z = derive_aquifer_depth(surface_z, moisture, biome);

    GeologyParams {
        surface_z,
        biome,
        temperature,
        moisture,
        stress,
        is_volcanic,
        water_body_type,
        soil_depth,
        primary_stone,
        secondary_stone,
        has_caverns,
        has_magma,
        aquifer_z,
    }
}

/// Derive soil depth based on biome and moisture
fn derive_soil_depth(biome: ExtendedBiome, moisture: f32) -> i16 {
    // Base depth by biome category
    let base_depth = match biome {
        // Forest/grassland: 4-8 z-levels
        ExtendedBiome::TemperateGrassland |
        ExtendedBiome::Savanna => 5,

        ExtendedBiome::TemperateForest |
        ExtendedBiome::BorealForest => 6,

        ExtendedBiome::TropicalForest |
        ExtendedBiome::TropicalRainforest |
        ExtendedBiome::TemperateRainforest => 7,

        // Desert: 1-2 z-levels (sandy/rocky)
        ExtendedBiome::Desert |
        ExtendedBiome::SaltFlats |
        ExtendedBiome::GlassDesert |
        ExtendedBiome::SingingDunes => 1,

        // Mountain: 0-1 z-levels (exposed rock)
        ExtendedBiome::SnowyPeaks |
        ExtendedBiome::AlpineTundra |
        ExtendedBiome::Foothills => 1,

        // Swamp: 6-10 z-levels (deep peat/mud)
        ExtendedBiome::Swamp |
        ExtendedBiome::Marsh |
        ExtendedBiome::Bog |
        ExtendedBiome::MangroveSaltmarsh |
        ExtendedBiome::SpiritMarsh |
        ExtendedBiome::Shadowfen => 8,

        // Tundra: 2-4 z-levels (permafrost)
        ExtendedBiome::Tundra |
        ExtendedBiome::AuroraWastes => 3,

        // Volcanic: 0-1 z-levels (exposed rock)
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::ObsidianFields |
        ExtendedBiome::Geysers |
        ExtendedBiome::SulfurVents |
        ExtendedBiome::Ashlands => 0,

        // Water biomes: minimal soil
        ExtendedBiome::DeepOcean |
        ExtendedBiome::Ocean |
        ExtendedBiome::CoastalWater |
        ExtendedBiome::Lagoon |
        ExtendedBiome::FrozenLake => 0,

        // Default moderate soil
        _ => 4,
    };

    // Modify by moisture (wetter = deeper soil)
    let moisture_modifier = (moisture * 2.0) as i16;

    (base_depth + moisture_modifier).min(10).max(0)
}

/// Derive stone types based on tectonic stress and temperature
fn derive_stone_types(stress: f32, temperature: f32, biome: ExtendedBiome) -> (StoneType, StoneType) {
    // Volcanic regions
    if stress > 0.6 || matches!(biome,
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::ObsidianFields |
        ExtendedBiome::Geysers
    ) {
        return (StoneType::Basalt, StoneType::Obsidian);
    }

    // High stress (convergent/mountains) = metamorphic
    if stress > 0.3 {
        return (StoneType::Granite, StoneType::Marble);
    }

    // Low stress (divergent/rift) = igneous
    if stress < -0.3 {
        return (StoneType::Basalt, StoneType::Granite);
    }

    // Sedimentary regions (most common)
    if temperature > 20.0 {
        // Warm regions: limestone common
        (StoneType::Limestone, StoneType::Sandstone)
    } else if temperature < 0.0 {
        // Cold regions: harder stone
        (StoneType::Granite, StoneType::Slate)
    } else {
        // Temperate: mixed
        (StoneType::Limestone, StoneType::Shale)
    }
}

/// Check for cavern presence by examining world zlevel data
fn check_cavern_presence(world: &WorldData, world_x: usize, world_y: usize, surface_z: i16) -> [bool; 3] {
    let mut has_caverns = [false, false, false];

    // Only check for caverns if we're above sea level (land)
    if surface_z <= zlevel::SEA_LEVEL_Z as i16 {
        return has_caverns;
    }

    // Check each cavern layer range for cave tiles
    for z in zlevel::MIN_Z..surface_z as i32 {
        let ztile = *world.zlevels.get(world_x, world_y, z);
        if ztile.is_cave() {
            if z >= CAVERN_1_MIN && z <= zlevel::CAVERN_1_MAX {
                has_caverns[0] = true;
            } else if z >= CAVERN_2_MIN && z <= zlevel::CAVERN_2_MAX {
                has_caverns[1] = true;
            } else if z >= CAVERN_3_MIN && z <= zlevel::CAVERN_3_MAX {
                has_caverns[2] = true;
            }
        }
    }

    has_caverns
}

/// Derive aquifer depth based on surface z, moisture, and biome
fn derive_aquifer_depth(surface_z: i16, moisture: f32, biome: ExtendedBiome) -> Option<i16> {
    // No aquifers underwater or in very dry areas
    if surface_z <= zlevel::SEA_LEVEL_Z as i16 || moisture < 0.3 {
        return None;
    }

    // No aquifers in volcanic or frozen regions
    if matches!(biome,
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::ObsidianFields |
        ExtendedBiome::Ice |
        ExtendedBiome::AuroraWastes |
        ExtendedBiome::FrozenLake
    ) {
        return None;
    }

    // Aquifer depth: 3-8 levels below surface based on moisture
    let depth = ((moisture * 5.0) as i16 + 3).min(8);
    let aquifer_z = (surface_z - depth).max(zlevel::MIN_Z as i16);

    Some(aquifer_z)
}

/// Get the soil type for a biome
pub fn biome_soil_type(biome: ExtendedBiome, depth: i16, moisture: f32) -> SoilType {
    match biome {
        // Sandy soils
        ExtendedBiome::Desert |
        ExtendedBiome::SaltFlats |
        ExtendedBiome::SingingDunes |
        ExtendedBiome::GlassDesert => SoilType::Sand,

        // Clay-rich soils (wet areas)
        ExtendedBiome::Swamp |
        ExtendedBiome::Marsh |
        ExtendedBiome::Bog |
        ExtendedBiome::MangroveSaltmarsh => {
            if depth == 0 {
                SoilType::Peat
            } else {
                SoilType::Clay
            }
        }

        // Frozen soils
        ExtendedBiome::Tundra |
        ExtendedBiome::AuroraWastes |
        ExtendedBiome::AlpineTundra => SoilType::Permafrost,

        // Rich soils (forests, grasslands)
        ExtendedBiome::TemperateForest |
        ExtendedBiome::BorealForest |
        ExtendedBiome::TropicalForest |
        ExtendedBiome::TropicalRainforest => {
            if depth == 0 {
                SoilType::Loam
            } else if moisture > 0.6 {
                SoilType::Clay
            } else {
                SoilType::Loam
            }
        }

        // Grassland soils
        ExtendedBiome::TemperateGrassland |
        ExtendedBiome::Savanna => {
            if depth == 0 {
                SoilType::Loam
            } else {
                SoilType::Silt
            }
        }

        // Rocky soils (mountains)
        ExtendedBiome::SnowyPeaks |
        ExtendedBiome::Foothills => SoilType::Gravel,

        // Volcanic soils
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::Ashlands => SoilType::Ash,

        // Default
        _ => {
            if moisture > 0.6 {
                SoilType::Clay
            } else if moisture > 0.3 {
                SoilType::Loam
            } else {
                SoilType::Sand
            }
        }
    }
}

/// Get the surface material for a biome
pub fn biome_surface_material(biome: ExtendedBiome, is_water: bool) -> Material {
    if is_water {
        return Material::Water;
    }

    match biome {
        // Ice/frozen
        ExtendedBiome::Ice |
        ExtendedBiome::FrozenLake |
        ExtendedBiome::SnowyPeaks => Material::Ice,

        // Sand
        ExtendedBiome::Desert |
        ExtendedBiome::SingingDunes |
        ExtendedBiome::GlassDesert |
        ExtendedBiome::SaltFlats => Material::Sand,

        // Stone/rock
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::ObsidianFields |
        ExtendedBiome::Foothills |
        ExtendedBiome::AlpineTundra => Material::Stone,

        // Mud/swamp
        ExtendedBiome::Swamp |
        ExtendedBiome::Marsh |
        ExtendedBiome::Bog |
        ExtendedBiome::Shadowfen => Material::Mud,

        // Grass (most land biomes)
        _ => Material::Grass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soil_depth_by_biome() {
        // Desert should have minimal soil
        let desert_depth = derive_soil_depth(ExtendedBiome::Desert, 0.1);
        assert!(desert_depth <= 2);

        // Forest should have moderate soil
        let forest_depth = derive_soil_depth(ExtendedBiome::TemperateForest, 0.5);
        assert!(forest_depth >= 4 && forest_depth <= 8);

        // Swamp should have deep soil
        let swamp_depth = derive_soil_depth(ExtendedBiome::Swamp, 0.8);
        assert!(swamp_depth >= 6);
    }

    #[test]
    fn test_stone_types_by_stress() {
        // High stress = metamorphic
        let (primary, _) = derive_stone_types(0.5, 15.0, ExtendedBiome::TemperateGrassland);
        assert_eq!(primary, StoneType::Granite);

        // Volcanic = basalt/obsidian
        let (primary, secondary) = derive_stone_types(0.7, 30.0, ExtendedBiome::VolcanicWasteland);
        assert_eq!(primary, StoneType::Basalt);
        assert_eq!(secondary, StoneType::Obsidian);
    }

    #[test]
    fn test_geology_params_layers() {
        let params = GeologyParams {
            surface_z: 5,
            biome: ExtendedBiome::TemperateForest,
            temperature: 15.0,
            moisture: 0.5,
            stress: 0.0,
            is_volcanic: false,
            water_body_type: WaterBodyType::None,
            soil_depth: 4,
            primary_stone: StoneType::Limestone,
            secondary_stone: StoneType::Sandstone,
            has_caverns: [true, false, false],
            has_magma: false,
            aquifer_z: Some(0),
        };

        // Surface
        assert!(!params.is_underground(5));
        assert!(params.is_underground(4));

        // Soil layer
        assert!(params.is_soil_layer(4)); // surface - 1
        assert!(params.is_soil_layer(1)); // surface - soil_depth
        assert!(!params.is_soil_layer(0)); // rock starts here

        // Rock surface
        assert_eq!(params.rock_surface_z(), 1); // 5 - 4 = 1
    }
}
