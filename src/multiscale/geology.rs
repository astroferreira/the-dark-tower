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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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

// =============================================================================
// SEAMLESS CHUNK BOUNDARY - SURFACE INTERPOLATION
// =============================================================================

/// Surface heights for 4 corner world tiles (2x2 grid).
///
/// Used for bilinear interpolation of surface_z across chunk boundaries.
/// Layout: `corners[y][x]` where:
/// - `[0][0]` = this tile (world_x, world_y)
/// - `[0][1]` = east tile (world_x+1, world_y)
/// - `[1][0]` = south tile (world_x, world_y+1)
/// - `[1][1]` = southeast tile (world_x+1, world_y+1)
pub type CornerHeights = [[i16; 2]; 2];

/// Get surface_z for 4 corner world tiles (2x2 grid) for bilinear interpolation.
///
/// This fetches the surface heights from this tile and its east, south, and
/// southeast neighbors. The result is used to smoothly interpolate surface
/// elevation across chunk boundaries.
///
/// # Arguments
/// * `world` - World data
/// * `world_x` - World tile X coordinate
/// * `world_y` - World tile Y coordinate
///
/// # Returns
/// 2x2 array of surface heights for interpolation
pub fn get_corner_surface_heights(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
) -> CornerHeights {
    let width = world.heightmap.width;
    let height = world.heightmap.height;

    // East wraps horizontally
    let east_x = (world_x + 1) % width;
    // South clamps at bottom edge
    let south_y = (world_y + 1).min(height - 1);

    [
        [
            *world.surface_z.get(world_x, world_y) as i16,
            *world.surface_z.get(east_x, world_y) as i16,
        ],
        [
            *world.surface_z.get(world_x, south_y) as i16,
            *world.surface_z.get(east_x, south_y) as i16,
        ],
    ]
}

/// Bilinear interpolate surface_z at a local position within a chunk.
///
/// This creates smooth elevation transitions between adjacent world tiles
/// by interpolating between the 4 corner heights based on local position.
///
/// # Arguments
/// * `corners` - 2x2 array of corner heights from `get_corner_surface_heights()`
/// * `local_x` - Local X position within chunk (0 to LOCAL_SIZE-1)
/// * `local_y` - Local Y position within chunk (0 to LOCAL_SIZE-1)
/// * `local_size` - Size of local chunk (typically 48)
///
/// # Returns
/// Interpolated surface_z at this position
pub fn interpolate_surface_z(
    corners: &CornerHeights,
    local_x: usize,
    local_y: usize,
    local_size: usize,
) -> i16 {
    // Normalize position to 0.0-1.0 range
    // Use (local_size - 1) so that the edges reach exactly 0.0 and 1.0
    // This ensures adjacent chunks share the same interpolated values at boundaries
    let max_coord = (local_size - 1).max(1) as f32;
    let u = local_x as f32 / max_coord;
    let v = local_y as f32 / max_coord;

    // Bilinear interpolation
    let top = corners[0][0] as f32 * (1.0 - u) + corners[0][1] as f32 * u;
    let bottom = corners[1][0] as f32 * (1.0 - u) + corners[1][1] as f32 * u;
    (top * (1.0 - v) + bottom * v).round() as i16
}

/// Get biome weights for 4 corner world tiles for blending.
///
/// Returns the biome at each corner position, used for smooth biome
/// transitions across chunk boundaries.
pub fn get_corner_biomes(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
) -> [[ExtendedBiome; 2]; 2] {
    let width = world.biomes.width;
    let height = world.biomes.height;

    let east_x = (world_x + 1) % width;
    let south_y = (world_y + 1).min(height - 1);

    [
        [
            *world.biomes.get(world_x, world_y),
            *world.biomes.get(east_x, world_y),
        ],
        [
            *world.biomes.get(world_x, south_y),
            *world.biomes.get(east_x, south_y),
        ],
    ]
}

/// Check if a biome is a water biome (ocean, lake, etc.)
pub fn is_water_biome(biome: ExtendedBiome) -> bool {
    matches!(biome,
        ExtendedBiome::DeepOcean |
        ExtendedBiome::Ocean |
        ExtendedBiome::CoastalWater |
        ExtendedBiome::OceanicTrench |
        ExtendedBiome::MidOceanRidge |
        ExtendedBiome::FrozenLake |
        ExtendedBiome::SeagrassMeadow |
        ExtendedBiome::KelpForest |
        ExtendedBiome::CoralReef |
        ExtendedBiome::BioluminescentWater |
        ExtendedBiome::Cenote |
        ExtendedBiome::Lagoon |
        ExtendedBiome::AcidLake |
        ExtendedBiome::LavaLake |
        ExtendedBiome::AbyssalVents |
        ExtendedBiome::Sargasso
    )
}

/// Get the water factor for 4 corner biomes (0.0 = all land, 1.0 = all water)
pub fn get_corner_water_factors(corner_biomes: &[[ExtendedBiome; 2]; 2]) -> [[f32; 2]; 2] {
    [
        [
            if is_water_biome(corner_biomes[0][0]) { 1.0 } else { 0.0 },
            if is_water_biome(corner_biomes[0][1]) { 1.0 } else { 0.0 },
        ],
        [
            if is_water_biome(corner_biomes[1][0]) { 1.0 } else { 0.0 },
            if is_water_biome(corner_biomes[1][1]) { 1.0 } else { 0.0 },
        ],
    ]
}

/// Calculate water factor using world-continuous noise instead of bilinear interpolation.
///
/// This creates organic coastline shapes by:
/// 1. Using world-position noise as the primary driver
/// 2. Biasing toward water/land based on nearby biome centers
/// 3. Never interpolating binary values directly
///
/// # Arguments
/// * `corner_biomes` - Biomes at the 4 corners (2x2 grid)
/// * `world_x`, `world_y` - World tile coordinates
/// * `local_x`, `local_y` - Local position within chunk (0..LOCAL_SIZE)
/// * `local_size` - Size of local chunk (usually 48)
/// * `coastline_noise` - World-seeded Perlin noise for coastline shapes
///
/// # Returns
/// Water factor from 0.0 (land) to 1.0 (water) with organic, noise-driven boundaries
pub fn calculate_noise_water_factor(
    corner_biomes: &[[ExtendedBiome; 2]; 2],
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    local_size: usize,
    coastline_noise: &noise::Perlin,
) -> f32 {
    use noise::NoiseFn;
    use super::coords::world_noise_coord;

    // Check how many corners are water
    let water_count = corner_biomes.iter().flatten()
        .filter(|&&b| is_water_biome(b)).count();

    // Base: normalized position within chunk (0-1)
    // Use (local_size - 1) so edges reach exactly 0.0 and 1.0 for boundary continuity
    let max_coord = (local_size - 1).max(1) as f32;
    let u = local_x as f32 / max_coord;
    let v = local_y as f32 / max_coord;

    // Distance to nearest chunk edge (0 at edges, 0.5 at center)
    let edge_dist = u.min(1.0 - u).min(v).min(1.0 - v);

    // World-continuous noise for coastline shapes
    let [nx, ny] = world_noise_coord(world_x, world_y, local_x, local_y, 0.015);
    let noise1 = coastline_noise.get([nx, ny]) as f32;
    let noise2 = coastline_noise.get([nx * 2.3 + 100.0, ny * 2.3 + 100.0]) as f32 * 0.4;
    let noise3 = coastline_noise.get([nx * 5.1 + 200.0, ny * 5.1 + 200.0]) as f32 * 0.2;
    let combined_noise = noise1 + noise2 + noise3;  // Range roughly -1.6 to 1.6

    // For pure land/water cases, allow noise to create variation near chunk edges
    // This creates smooth blending with neighboring chunks that may have different biomes
    if water_count == 0 {
        // All land corners - but allow water to bleed in near edges via noise
        // Edge blend zone is ~20% of chunk from each edge
        let edge_blend = (0.2 - edge_dist).max(0.0) / 0.2;  // 1 at edge, 0 at 20% in
        if edge_blend > 0.0 {
            // Use noise to potentially show water near edges
            // Noise threshold is higher (harder to become water) as we go further from edge
            let threshold = 0.8 + (1.0 - edge_blend) * 0.8;  // 0.8 at edge, 1.6 at 20% in
            if combined_noise > threshold {
                // Smooth transition based on how much noise exceeds threshold
                let excess = (combined_noise - threshold) / 0.8;
                return excess.clamp(0.0, 1.0).min(edge_blend);
            }
        }
        return 0.0;  // All land, not near edge or noise didn't trigger
    }
    if water_count == 4 {
        // All water corners - but allow land to bleed in near edges via noise
        let edge_blend = (0.2 - edge_dist).max(0.0) / 0.2;
        if edge_blend > 0.0 {
            let threshold = -0.8 - (1.0 - edge_blend) * 0.8;  // -0.8 at edge, -1.6 at 20% in
            if combined_noise < threshold {
                let excess = (threshold - combined_noise) / 0.8;
                return 1.0 - excess.clamp(0.0, 1.0).min(edge_blend);
            }
        }
        return 1.0;  // All water, not near edge or noise didn't trigger
    }

    // MIXED CASE: Full noise-based blending
    // Calculate radial distance-based bias toward each corner
    let nw_dist = (u * u + v * v).sqrt();
    let ne_dist = ((1.0 - u) * (1.0 - u) + v * v).sqrt();
    let sw_dist = (u * u + (1.0 - v) * (1.0 - v)).sqrt();
    let se_dist = ((1.0 - u) * (1.0 - u) + (1.0 - v) * (1.0 - v)).sqrt();

    // Invert and normalize distances (closer = higher weight)
    let max_dist = 1.415; // sqrt(2)
    let nw_w = 1.0 - nw_dist / max_dist;
    let ne_w = 1.0 - ne_dist / max_dist;
    let sw_w = 1.0 - sw_dist / max_dist;
    let se_w = 1.0 - se_dist / max_dist;

    // Weight by corner water status
    let weighted_water =
        nw_w * if is_water_biome(corner_biomes[0][0]) { 1.0 } else { 0.0 } +
        ne_w * if is_water_biome(corner_biomes[0][1]) { 1.0 } else { 0.0 } +
        sw_w * if is_water_biome(corner_biomes[1][0]) { 1.0 } else { 0.0 } +
        se_w * if is_water_biome(corner_biomes[1][1]) { 1.0 } else { 0.0 };
    let total_w = nw_w + ne_w + sw_w + se_w;
    let base_water = weighted_water / total_w;

    // Use noise to SHIFT the threshold
    let noise_influence = 1.0 - (2.0 * (base_water - 0.5)).abs();
    let threshold = 0.5 + combined_noise * 0.4 * noise_influence;

    // Map distance from threshold to a 0-1 range with soft edges
    let distance_from_threshold = base_water - threshold;
    let soft_edge_width = 0.15;
    let soft_factor = (distance_from_threshold / soft_edge_width).clamp(-1.0, 1.0);

    // Smoothstep for soft transition
    let t = (soft_factor + 1.0) / 2.0;
    let smoothed = t * t * (3.0 - 2.0 * t);

    smoothed
}

/// Interpolate water factor at a local position using bilinear interpolation.
///
/// Returns a value from 0.0 (land) to 1.0 (water) that smoothly transitions
/// at boundaries between water and land biomes.
pub fn interpolate_water_factor(
    water_factors: &[[f32; 2]; 2],
    local_x: usize,
    local_y: usize,
    local_size: usize,
) -> f32 {
    // Use (local_size - 1) so edges reach exactly 0.0 and 1.0 for boundary continuity
    let max_coord = (local_size - 1).max(1) as f32;
    let u = local_x as f32 / max_coord;
    let v = local_y as f32 / max_coord;

    let top = water_factors[0][0] * (1.0 - u) + water_factors[0][1] * u;
    let bottom = water_factors[1][0] * (1.0 - u) + water_factors[1][1] * u;
    top * (1.0 - v) + bottom * v
}

/// Information about coastline at a local position
#[derive(Clone, Debug)]
pub struct CoastlineInfo {
    /// Water factor (0.0 = land, 1.0 = water, between = transition zone)
    pub water_factor: f32,
    /// Whether this is in a transition zone (water factor between 0 and 1)
    pub is_transition: bool,
    /// Suggested terrain type for transitions
    pub terrain_hint: CoastlineTerrainHint,
}

/// Hint for what terrain to generate at coastline
#[derive(Clone, Debug, PartialEq)]
pub enum CoastlineTerrainHint {
    /// Deep water (far from shore)
    DeepWater,
    /// Shallow water (near shore)
    ShallowWater,
    /// Beach/sand
    Beach,
    /// Land (above water line)
    Land,
}

/// Calculate coastline information at a local position.
///
/// Uses interpolated water factor and elevation to determine coastline placement.
/// For natural-looking coastlines, use `calculate_coastline_info_with_noise` which
/// adds world-continuous noise to create organic coastline shapes.
pub fn calculate_coastline_info(
    corner_biomes: &[[ExtendedBiome; 2]; 2],
    corner_heights: &CornerHeights,
    local_x: usize,
    local_y: usize,
    local_size: usize,
) -> CoastlineInfo {
    let water_factors = get_corner_water_factors(corner_biomes);
    let water_factor = interpolate_water_factor(&water_factors, local_x, local_y, local_size);

    // Check if any corner differs (i.e., we're at a coastline)
    let has_water = water_factors.iter().flatten().any(|&f| f > 0.5);
    let has_land = water_factors.iter().flatten().any(|&f| f < 0.5);
    let is_transition = has_water && has_land;

    // Get interpolated surface height for this position
    let surface_z = interpolate_surface_z(corner_heights, local_x, local_y, local_size);

    // Determine terrain hint based on water factor and elevation
    let terrain_hint = if water_factor > 0.7 {
        // Mostly water - check depth
        if surface_z < -2 {
            CoastlineTerrainHint::DeepWater
        } else {
            CoastlineTerrainHint::ShallowWater
        }
    } else if water_factor > 0.3 {
        // Transition zone - beach or shallow water based on elevation
        if surface_z >= 0 {
            CoastlineTerrainHint::Beach
        } else {
            CoastlineTerrainHint::ShallowWater
        }
    } else {
        // Mostly land
        CoastlineTerrainHint::Land
    };

    CoastlineInfo {
        water_factor,
        is_transition,
        terrain_hint,
    }
}

/// Calculate coastline information with noise for natural-looking shapes.
///
/// This version uses world-continuous noise-based water factor calculation
/// that creates organic coastline curves without chunk-aligned diagonal artifacts.
///
/// # Arguments
/// * `corner_biomes` - Biomes at the 4 corners (2x2 grid)
/// * `corner_heights` - Surface heights at corners
/// * `world_x`, `world_y` - World tile coordinates
/// * `local_x`, `local_y` - Local position within chunk (0..LOCAL_SIZE)
/// * `local_size` - Size of local chunk (usually 48)
/// * `coastline_noise` - World-seeded Perlin noise for coastline shapes
pub fn calculate_coastline_info_with_noise(
    corner_biomes: &[[ExtendedBiome; 2]; 2],
    corner_heights: &CornerHeights,
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    local_size: usize,
    coastline_noise: &noise::Perlin,
) -> CoastlineInfo {
    // Use noise-based water factor to eliminate diagonal interpolation artifacts
    let water_factor = calculate_noise_water_factor(
        corner_biomes, world_x, world_y, local_x, local_y, local_size, coastline_noise
    );

    // Check if any corner differs (i.e., we're at a coastline transition zone)
    let water_factors = get_corner_water_factors(corner_biomes);
    let has_water = water_factors.iter().flatten().any(|&f| f > 0.5);
    let has_land = water_factors.iter().flatten().any(|&f| f < 0.5);
    let is_transition = has_water && has_land;

    // Get interpolated surface height for this position
    let surface_z = interpolate_surface_z(corner_heights, local_x, local_y, local_size);

    // Determine terrain hint based on water factor and elevation
    let terrain_hint = if water_factor > 0.7 {
        // Mostly water - check depth
        if surface_z < -2 {
            CoastlineTerrainHint::DeepWater
        } else {
            CoastlineTerrainHint::ShallowWater
        }
    } else if water_factor > 0.3 {
        // Transition zone - beach or shallow water based on elevation
        if surface_z >= 0 {
            CoastlineTerrainHint::Beach
        } else {
            CoastlineTerrainHint::ShallowWater
        }
    } else {
        // Mostly land
        CoastlineTerrainHint::Land
    };

    CoastlineInfo {
        water_factor,
        is_transition,
        terrain_hint,
    }
}

/// Get interpolated temperature at a local position.
pub fn interpolate_temperature(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    local_size: usize,
) -> f32 {
    let width = world.temperature.width;
    let height = world.temperature.height;

    let east_x = (world_x + 1) % width;
    let south_y = (world_y + 1).min(height - 1);

    let corners = [
        [
            *world.temperature.get(world_x, world_y),
            *world.temperature.get(east_x, world_y),
        ],
        [
            *world.temperature.get(world_x, south_y),
            *world.temperature.get(east_x, south_y),
        ],
    ];

    // Use (local_size - 1) so edges reach exactly 0.0 and 1.0 for boundary continuity
    let max_coord = (local_size - 1).max(1) as f32;
    let u = local_x as f32 / max_coord;
    let v = local_y as f32 / max_coord;

    let top = corners[0][0] * (1.0 - u) + corners[0][1] * u;
    let bottom = corners[1][0] * (1.0 - u) + corners[1][1] * u;
    top * (1.0 - v) + bottom * v
}

/// Get interpolated moisture at a local position.
pub fn interpolate_moisture(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    local_size: usize,
) -> f32 {
    let width = world.moisture.width;
    let height = world.moisture.height;

    let east_x = (world_x + 1) % width;
    let south_y = (world_y + 1).min(height - 1);

    let corners = [
        [
            *world.moisture.get(world_x, world_y),
            *world.moisture.get(east_x, world_y),
        ],
        [
            *world.moisture.get(world_x, south_y),
            *world.moisture.get(east_x, south_y),
        ],
    ];

    // Use (local_size - 1) so edges reach exactly 0.0 and 1.0 for boundary continuity
    let max_coord = (local_size - 1).max(1) as f32;
    let u = local_x as f32 / max_coord;
    let v = local_y as f32 / max_coord;

    let top = corners[0][0] * (1.0 - u) + corners[0][1] * u;
    let bottom = corners[1][0] * (1.0 - u) + corners[1][1] * u;
    top * (1.0 - v) + bottom * v
}

// =============================================================================
// RIVER INTEGRATION
// =============================================================================

use crate::erosion::river_geometry::RiverNetwork;

/// Information about a river at a local position
#[derive(Clone, Debug)]
pub struct RiverInfo {
    /// Whether there's a river at this position
    pub is_river: bool,
    /// River width in world tiles (0 if no river)
    pub width: f32,
    /// Distance from river center (0.0 = center, 1.0 = edge)
    pub distance_factor: f32,
    /// Depth of river channel (how much to carve into terrain)
    pub depth: i16,
    /// Flow direction (normalized, for visual effects)
    pub flow_dx: f32,
    pub flow_dy: f32,
}

impl Default for RiverInfo {
    fn default() -> Self {
        Self {
            is_river: false,
            width: 0.0,
            distance_factor: 1.0,
            depth: 0,
            flow_dx: 0.0,
            flow_dy: 1.0,
        }
    }
}

/// Query river information at a local position.
///
/// Converts local chunk coordinates to world coordinates and queries the river network.
///
/// # Arguments
/// * `river_network` - The world's river network
/// * `world_x` - World tile X coordinate
/// * `world_y` - World tile Y coordinate
/// * `local_x` - Local tile X within chunk (0 to LOCAL_SIZE-1)
/// * `local_y` - Local tile Y within chunk (0 to LOCAL_SIZE-1)
/// * `local_size` - Size of local chunk (typically 48)
///
/// # Returns
/// `RiverInfo` with river details if present
pub fn query_river_at_local(
    river_network: &RiverNetwork,
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    local_size: usize,
) -> RiverInfo {
    // Convert local position to world coordinates (fractional)
    let wx = world_x as f32 + local_x as f32 / local_size as f32;
    let wy = world_y as f32 + local_y as f32 / local_size as f32;

    // Find distance to nearest river centerline and its properties
    let mut min_dist = f32::MAX;
    let mut river_width = 0.0f32;
    let mut best_tangent = (0.0f32, 1.0f32);

    for segment in &river_network.segments {
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let pt = segment.evaluate(t);
            let dx = pt.world_x - wx;
            let dy = pt.world_y - wy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < min_dist {
                min_dist = dist;
                river_width = pt.width;
                best_tangent = segment.tangent(t);
            }
        }
    }

    // Check if we're within the river width
    let half_width = river_width / 2.0;
    if river_width <= 0.0 || min_dist > half_width + 0.1 {
        return RiverInfo::default();
    }

    // Calculate distance factor (0 = center, 1 = edge)
    let distance_factor = (min_dist / half_width).clamp(0.0, 1.0);

    // Calculate river depth based on width and distance from center
    // Center is deeper than edges (parabolic cross-section)
    let center_factor = 1.0 - distance_factor * distance_factor;
    let base_depth = (1.0 + river_width.log2().max(0.0) * 1.5).round() as i16;
    let depth = ((base_depth as f32) * center_factor).round() as i16;

    RiverInfo {
        is_river: true,
        width: river_width,
        distance_factor,
        depth: depth.max(1).min(6), // Clamp depth to 1-6
        flow_dx: best_tangent.0,
        flow_dy: best_tangent.1,
    }
}

/// Check if any river passes through a world tile (for quick filtering).
///
/// This is faster than checking every local position - use it to skip
/// river processing for chunks that have no rivers.
pub fn world_tile_has_river(
    river_network: &RiverNetwork,
    world_x: usize,
    world_y: usize,
) -> bool {
    // Check if any river segment is within 1 tile of this position
    let wx = world_x as f32 + 0.5;
    let wy = world_y as f32 + 0.5;

    for segment in &river_network.segments {
        // Check segment bounding box first (fast rejection)
        let min_x = segment.p0.world_x.min(segment.p1.world_x)
            .min(segment.p2.world_x).min(segment.p3.world_x) - segment.p0.width;
        let max_x = segment.p0.world_x.max(segment.p1.world_x)
            .max(segment.p2.world_x).max(segment.p3.world_x) + segment.p3.width;
        let min_y = segment.p0.world_y.min(segment.p1.world_y)
            .min(segment.p2.world_y).min(segment.p3.world_y) - segment.p0.width;
        let max_y = segment.p0.world_y.max(segment.p1.world_y)
            .max(segment.p2.world_y).max(segment.p3.world_y) + segment.p3.width;

        // Skip if world tile is outside bounding box (with margin)
        if wx < min_x - 1.0 || wx > max_x + 1.0 || wy < min_y - 1.0 || wy > max_y + 1.0 {
            continue;
        }

        // Check a few points along the segment
        for i in 0..=5 {
            let t = i as f32 / 5.0;
            let pt = segment.evaluate(t);
            let dx = (pt.world_x - wx).abs();
            let dy = (pt.world_y - wy).abs();

            // River is within this tile if distance is less than 1 + half width
            if dx < 1.0 + pt.width / 2.0 && dy < 1.0 + pt.width / 2.0 {
                return true;
            }
        }
    }

    false
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
