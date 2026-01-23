//! Handshake data structures for seamless region stitching
//!
//! Stores pre-calculated boundary data that enables adjacent regions to match
//! terrain heights, river crossings, and other features at their shared edges.

use crate::erosion::materials::RockType;
use crate::tilemap::Tilemap;
use super::rivers::RiverEdgeCrossing;

/// Direction constants for flow encoding (D8)
pub const FLOW_NONE: u8 = 255;
pub const FLOW_N: u8 = 0;
pub const FLOW_NE: u8 = 1;
pub const FLOW_E: u8 = 2;
pub const FLOW_SE: u8 = 3;
pub const FLOW_S: u8 = 4;
pub const FLOW_SW: u8 = 5;
pub const FLOW_W: u8 = 6;
pub const FLOW_NW: u8 = 7;

/// Vegetation distribution pattern
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VegetationPattern {
    /// Evenly distributed vegetation
    #[default]
    Uniform,
    /// Clustered trees/shrubs with clearings
    Clumped,
    /// Vegetation concentrated along water (rivers/streams)
    Gallery,
    /// Sparse vegetation with large bare areas
    Sparse,
    /// Dense vegetation with small gaps
    Dense,
}

/// Geological layer in the rock stack
#[derive(Clone, Copy, Debug, Default)]
pub struct RockLayer {
    /// Rock type for this layer
    pub rock_type: RockType,
    /// Thickness in z-levels (1-255)
    pub thickness: u8,
}

/// Handshake data for a single world tile
/// Contains all information needed to generate a seamless 64x64 region
#[derive(Clone, Debug)]
pub struct TileHandshake {
    // === Existing fields ===

    /// Corner heights: [NW, NE, SE, SW]
    /// These are averaged from adjacent tile centers for smooth interpolation
    pub corner_heights: [f32; 4],
    /// Terrain gradient direction (dx, dy) - indicates slope
    pub gradient: (f32, f32),
    /// D8 flow direction (0-7, 255=none)
    pub flow_direction: u8,
    /// Upstream drainage area (flow accumulation)
    pub flow_accumulation: f32,

    // === New terrain properties ===

    /// Local terrain roughness (0.0-1.0) - noise amplitude multiplier for detail
    /// 0.0 = flat terrain, 1.0 = extremely rugged
    pub roughness: f32,
    /// Sediment/soil depth in z-levels (0-255)
    /// Number of soil layers before hitting bedrock
    pub sediment_depth: u8,
    /// Geological rock stack (surface to deep, max 8 layers)
    /// First layer is surface, last is deep bedrock
    pub rock_stack: Vec<RockLayer>,
    /// Surface mineral concentration (0.0-1.0)
    /// Higher values = more ore vein probability
    pub surface_minerals: f32,

    // === Hydrology properties ===

    /// Water table height (0.0-1.0 relative to tile elevation)
    /// 0.0 = deep underground, 1.0 = at surface level
    pub water_table: f32,
    /// Aquifer pressure (0.0-1.0)
    /// High pressure = artesian springs likely when digging
    pub aquifer_pressure: f32,

    // === Vegetation properties ===

    /// Vegetation density (0.0-1.0)
    /// Controls tree/shrub count in region generation
    pub vegetation_density: f32,
    /// Vegetation distribution pattern
    pub vegetation_pattern: VegetationPattern,
}

impl Default for TileHandshake {
    fn default() -> Self {
        Self {
            corner_heights: [0.0; 4],
            gradient: (0.0, 0.0),
            flow_direction: FLOW_NONE,
            flow_accumulation: 0.0,
            roughness: 0.3,
            sediment_depth: 3,
            rock_stack: vec![RockLayer { rock_type: RockType::Sediment, thickness: 3 }],
            surface_minerals: 0.1,
            water_table: 0.3,
            aquifer_pressure: 0.2,
            vegetation_density: 0.5,
            vegetation_pattern: VegetationPattern::Uniform,
        }
    }
}

/// Complete handshake data for a region including river crossings
#[derive(Clone, Debug)]
pub struct RegionHandshake {
    /// Base tile handshake data
    pub tile: TileHandshake,
    /// River crossings at region boundaries
    pub river_crossings: Vec<RiverEdgeCrossing>,
}

impl Default for RegionHandshake {
    fn default() -> Self {
        Self {
            tile: TileHandshake::default(),
            river_crossings: Vec::new(),
        }
    }
}

/// World-wide handshake data storage
#[derive(Clone)]
pub struct WorldHandshakes {
    /// Handshake data per tile
    pub handshakes: Tilemap<RegionHandshake>,
    /// Flow direction map (D8 encoding)
    pub flow_direction: Tilemap<u8>,
    /// Flow accumulation map
    pub flow_accumulation: Tilemap<f32>,
}

impl WorldHandshakes {
    /// Create a new WorldHandshakes with default values
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            handshakes: Tilemap::new_with(width, height, RegionHandshake::default()),
            flow_direction: Tilemap::new_with(width, height, FLOW_NONE),
            flow_accumulation: Tilemap::new_with(width, height, 0.0),
        }
    }

    /// Get handshake data for a specific tile
    pub fn get(&self, x: usize, y: usize) -> &RegionHandshake {
        self.handshakes.get(x, y)
    }
}

/// Calculate corner heights for a tile by averaging adjacent tile centers
/// Corners are: 0=NW, 1=NE, 2=SE, 3=SW
pub fn calculate_corner_heights(heightmap: &Tilemap<f32>, x: usize, y: usize) -> [f32; 4] {
    let width = heightmap.width;
    let height = heightmap.height;

    let h_center = *heightmap.get(x, y);

    // Get heights of 4 adjacent tiles (with wrapping/clamping)
    let h_west = if x > 0 {
        *heightmap.get(x - 1, y)
    } else {
        *heightmap.get(width - 1, y) // wrap
    };

    let h_east = if x < width - 1 {
        *heightmap.get(x + 1, y)
    } else {
        *heightmap.get(0, y) // wrap
    };

    let h_north = if y > 0 {
        *heightmap.get(x, y - 1)
    } else {
        h_center // clamp at top
    };

    let h_south = if y < height - 1 {
        *heightmap.get(x, y + 1)
    } else {
        h_center // clamp at bottom
    };

    // Diagonal neighbors
    let h_nw = if y > 0 {
        if x > 0 {
            *heightmap.get(x - 1, y - 1)
        } else {
            *heightmap.get(width - 1, y - 1)
        }
    } else {
        h_west
    };

    let h_ne = if y > 0 {
        if x < width - 1 {
            *heightmap.get(x + 1, y - 1)
        } else {
            *heightmap.get(0, y - 1)
        }
    } else {
        h_east
    };

    let h_sw = if y < height - 1 {
        if x > 0 {
            *heightmap.get(x - 1, y + 1)
        } else {
            *heightmap.get(width - 1, y + 1)
        }
    } else {
        h_west
    };

    let h_se = if y < height - 1 {
        if x < width - 1 {
            *heightmap.get(x + 1, y + 1)
        } else {
            *heightmap.get(0, y + 1)
        }
    } else {
        h_east
    };

    // Average corners from the 4 tiles that share each corner
    // NW corner: average of NW tile, N tile, W tile, center tile
    let nw = (h_nw + h_north + h_west + h_center) / 4.0;
    // NE corner: average of NE tile, N tile, E tile, center tile
    let ne = (h_ne + h_north + h_east + h_center) / 4.0;
    // SE corner: average of SE tile, S tile, E tile, center tile
    let se = (h_se + h_south + h_east + h_center) / 4.0;
    // SW corner: average of SW tile, S tile, W tile, center tile
    let sw = (h_sw + h_south + h_west + h_center) / 4.0;

    [nw, ne, se, sw]
}

/// Calculate terrain roughness (0.0-1.0) from local height variance
/// This is used as a noise amplitude multiplier in region generation
pub fn calculate_roughness(heightmap: &Tilemap<f32>, x: usize, y: usize) -> f32 {
    let center = *heightmap.get(x, y);
    let neighbors = heightmap.neighbors_8(x, y);

    if neighbors.is_empty() {
        return 0.0;
    }

    // Calculate variance of height differences
    let mut sum_sq_diff = 0.0;
    let mut max_diff: f32 = 0.0;
    for (nx, ny) in &neighbors {
        let diff = (*heightmap.get(*nx, *ny) - center).abs();
        sum_sq_diff += diff * diff;
        max_diff = max_diff.max(diff);
    }

    let variance = (sum_sq_diff / neighbors.len() as f32).sqrt();

    // Normalize to 0.0-1.0 range
    // Typical variance is 0-500m, so we scale accordingly
    // Also factor in the max difference for extreme cases
    let variance_factor = (variance / 200.0).clamp(0.0, 1.0);
    let max_factor = (max_diff / 500.0).clamp(0.0, 1.0);

    // Blend both factors
    (variance_factor * 0.7 + max_factor * 0.3).clamp(0.0, 1.0)
}

/// Calculate sediment depth in z-levels (0-255) from hardness
/// Softer rock = more sediment accumulation
pub fn calculate_sediment_depth_zlevels(
    hardness_map: Option<&Tilemap<f32>>,
    moisture: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> u8 {
    let elevation = *heightmap.get(x, y);
    let moist = *moisture.get(x, y);

    // Base sediment from hardness
    let base_sediment = match hardness_map {
        Some(hardness) => {
            let h = *hardness.get(x, y);
            // Inverse relationship: soft rock (low hardness) = deep sediment
            (1.0 - h.clamp(0.0, 1.0)) * 10.0
        }
        None => 5.0, // Default medium sediment
    };

    // More sediment in lowlands and wet areas
    let elevation_factor = if elevation < 0.0 {
        1.5 // Underwater accumulates more
    } else if elevation < 100.0 {
        1.3 // Lowlands
    } else if elevation > 2000.0 {
        0.3 // High mountains - thin soil
    } else {
        1.0
    };

    // Moisture increases sediment (organic material)
    let moisture_factor = 1.0 + moist * 0.5;

    let depth = (base_sediment * elevation_factor * moisture_factor).round() as u8;
    depth.clamp(1, 20) // 1-20 z-levels of soil
}

/// Generate rock stack based on elevation, stress, and plate type
pub fn calculate_rock_stack(
    heightmap: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    hardness_map: Option<&Tilemap<f32>>,
    x: usize,
    y: usize,
) -> Vec<RockLayer> {
    let elevation = *heightmap.get(x, y);
    let stress = *stress_map.get(x, y);
    let hardness = hardness_map.map(|h| *h.get(x, y)).unwrap_or(0.5);

    let mut stack = Vec::with_capacity(8);

    // Surface layer based on conditions
    if elevation < -1000.0 {
        // Deep ocean - basalt oceanic crust
        stack.push(RockLayer { rock_type: RockType::Sediment, thickness: 2 });
        stack.push(RockLayer { rock_type: RockType::Basalt, thickness: 50 });
    } else if elevation < 0.0 {
        // Shallow water - sedimentary
        stack.push(RockLayer { rock_type: RockType::Sediment, thickness: 5 });
        stack.push(RockLayer { rock_type: RockType::Shale, thickness: 10 });
        stack.push(RockLayer { rock_type: RockType::Limestone, thickness: 15 });
        stack.push(RockLayer { rock_type: RockType::Granite, thickness: 50 });
    } else if stress > 0.5 {
        // High stress (mountains) - metamorphic/igneous
        if hardness > 0.7 {
            stack.push(RockLayer { rock_type: RockType::Granite, thickness: 30 });
            stack.push(RockLayer { rock_type: RockType::Basalt, thickness: 50 });
        } else {
            stack.push(RockLayer { rock_type: RockType::Sandstone, thickness: 5 });
            stack.push(RockLayer { rock_type: RockType::Granite, thickness: 25 });
            stack.push(RockLayer { rock_type: RockType::Basalt, thickness: 50 });
        }
    } else if elevation > 1500.0 {
        // High altitude - granite with thin soil
        stack.push(RockLayer { rock_type: RockType::Sediment, thickness: 1 });
        stack.push(RockLayer { rock_type: RockType::Granite, thickness: 30 });
        stack.push(RockLayer { rock_type: RockType::Basalt, thickness: 50 });
    } else if elevation < 200.0 {
        // Lowlands - thick sedimentary sequence
        stack.push(RockLayer { rock_type: RockType::Sediment, thickness: 8 });
        stack.push(RockLayer { rock_type: RockType::Shale, thickness: 10 });
        stack.push(RockLayer { rock_type: RockType::Sandstone, thickness: 15 });
        stack.push(RockLayer { rock_type: RockType::Limestone, thickness: 20 });
        stack.push(RockLayer { rock_type: RockType::Granite, thickness: 50 });
    } else {
        // Mid-elevation - standard continental sequence
        stack.push(RockLayer { rock_type: RockType::Sediment, thickness: 4 });
        stack.push(RockLayer { rock_type: RockType::Sandstone, thickness: 12 });
        stack.push(RockLayer { rock_type: RockType::Limestone, thickness: 15 });
        stack.push(RockLayer { rock_type: RockType::Granite, thickness: 50 });
    }

    stack
}

/// Calculate surface mineral concentration (0.0-1.0)
/// Higher in mountainous/volcanic areas and at plate boundaries
pub fn calculate_surface_minerals(
    stress_map: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> f32 {
    let stress = stress_map.get(x, y).abs();
    let elevation = *heightmap.get(x, y);

    // Base mineral concentration from tectonic activity
    let stress_factor = stress * 0.6;

    // Mountains have more exposed minerals
    let elevation_factor = if elevation > 2000.0 {
        0.4
    } else if elevation > 1000.0 {
        0.2
    } else {
        0.05
    };

    (stress_factor + elevation_factor).clamp(0.0, 1.0)
}

/// Calculate water table height (0.0-1.0 relative to surface)
/// Based on moisture, elevation, and proximity to water
pub fn calculate_water_table(
    moisture: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
    flow_accumulation: f32,
    x: usize,
    y: usize,
) -> f32 {
    let moist = *moisture.get(x, y);
    let elevation = *heightmap.get(x, y);

    // Base water table from moisture
    let base = moist * 0.5;

    // Lowlands have higher water table
    let elevation_factor = if elevation < 50.0 {
        0.4
    } else if elevation < 200.0 {
        0.2
    } else if elevation > 1000.0 {
        -0.2 // Mountains have deeper water table
    } else {
        0.0
    };

    // High flow accumulation means water converges here
    let flow_factor = (flow_accumulation.log10() / 5.0).clamp(0.0, 0.3);

    (base + elevation_factor + flow_factor).clamp(0.0, 1.0)
}

/// Calculate aquifer pressure (0.0-1.0)
/// Higher in areas with elevation difference and confined aquifers
pub fn calculate_aquifer_pressure(
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    gradient: (f32, f32),
    x: usize,
    y: usize,
) -> f32 {
    let elevation = *heightmap.get(x, y);
    let moist = *moisture.get(x, y);

    // Steeper gradient = more pressure potential
    let slope = (gradient.0 * gradient.0 + gradient.1 * gradient.1).sqrt();
    let slope_factor = (slope / 100.0).clamp(0.0, 0.5);

    // Mid-elevations in wet areas have highest pressure (artesian conditions)
    let elevation_factor = if elevation > 200.0 && elevation < 1000.0 {
        0.3
    } else if elevation > 1000.0 {
        0.1 // Recharge zone - low pressure
    } else {
        0.2 // Discharge zone - moderate
    };

    let moisture_factor = moist * 0.3;

    (slope_factor + elevation_factor + moisture_factor).clamp(0.0, 1.0)
}

/// Calculate vegetation density (0.0-1.0) from biome and moisture
pub fn calculate_vegetation_density(
    biomes: &Tilemap<crate::biomes::ExtendedBiome>,
    moisture: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> f32 {
    use crate::biomes::ExtendedBiome;

    let biome = *biomes.get(x, y);
    let moist = *moisture.get(x, y);
    let temp = *temperature.get(x, y);

    // Base density from biome type
    let base = match biome {
        // Dense vegetation
        ExtendedBiome::TropicalRainforest | ExtendedBiome::TemperateRainforest => 0.95,
        ExtendedBiome::TropicalForest | ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest => 0.85,
        ExtendedBiome::CloudForest | ExtendedBiome::MontaneForest => 0.8,
        ExtendedBiome::MushroomForest | ExtendedBiome::CrystalForest | ExtendedBiome::BioluminescentForest => 0.75,
        ExtendedBiome::SubalpineForest => 0.7,

        // Moderate vegetation
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => 0.65,
        ExtendedBiome::Savanna | ExtendedBiome::TemperateGrassland => 0.5,
        ExtendedBiome::MangroveSaltmarsh => 0.55,
        ExtendedBiome::AlpineMeadow | ExtendedBiome::Paramo => 0.4,

        // Sparse vegetation
        ExtendedBiome::Tundra | ExtendedBiome::AlpineTundra => 0.25,
        ExtendedBiome::Desert | ExtendedBiome::SaltFlats => 0.05,
        ExtendedBiome::Oasis => 0.7, // Exception - high in oasis

        // No vegetation
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::CoastalWater => 0.0,
        ExtendedBiome::Ice | ExtendedBiome::SnowyPeaks => 0.02,
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::Ashlands | ExtendedBiome::LavaLake => 0.0,

        // Dead/special
        ExtendedBiome::DeadForest | ExtendedBiome::PetrifiedForest => 0.15,

        // Default for other biomes
        _ => 0.4,
    };

    // Modify by moisture and temperature
    let moisture_mod: f32 = if moist < 0.2 { 0.5 } else { 1.0 };
    let temp_mod: f32 = if temp < -10.0 { 0.3 } else if temp < 0.0 { 0.6 } else { 1.0 };

    (base * moisture_mod * temp_mod).clamp(0.0, 1.0)
}

/// Determine vegetation pattern based on biome and terrain
pub fn calculate_vegetation_pattern(
    biomes: &Tilemap<crate::biomes::ExtendedBiome>,
    flow_accumulation: f32,
    moisture: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> VegetationPattern {
    use crate::biomes::ExtendedBiome;

    let biome = *biomes.get(x, y);
    let moist = *moisture.get(x, y);

    // Check for gallery forest conditions (high flow = river corridor)
    if flow_accumulation > 100.0 && moist < 0.5 {
        return VegetationPattern::Gallery;
    }

    match biome {
        // Dense uniform forests
        ExtendedBiome::TropicalRainforest | ExtendedBiome::TemperateRainforest |
        ExtendedBiome::TropicalForest | ExtendedBiome::TemperateForest => VegetationPattern::Dense,

        // Clumped patterns in drier areas
        ExtendedBiome::Savanna | ExtendedBiome::TemperateGrassland => VegetationPattern::Clumped,

        // Sparse patterns in harsh environments
        ExtendedBiome::Desert | ExtendedBiome::Tundra | ExtendedBiome::AlpineTundra => VegetationPattern::Sparse,

        // Gallery along water in semi-arid
        ExtendedBiome::SaltFlats => VegetationPattern::Gallery,

        // Default uniform
        _ => VegetationPattern::Uniform,
    }
}

/// Calculate terrain gradient at a point
pub fn calculate_gradient(heightmap: &Tilemap<f32>, x: usize, y: usize) -> (f32, f32) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Get heights for gradient calculation
    let h_west = if x > 0 {
        *heightmap.get(x - 1, y)
    } else {
        *heightmap.get(width - 1, y)
    };

    let h_east = if x < width - 1 {
        *heightmap.get(x + 1, y)
    } else {
        *heightmap.get(0, y)
    };

    let h_center = *heightmap.get(x, y);

    let h_north = if y > 0 {
        *heightmap.get(x, y - 1)
    } else {
        h_center
    };

    let h_south = if y < height - 1 {
        *heightmap.get(x, y + 1)
    } else {
        h_center
    };

    // Central difference gradient
    let dx = (h_east - h_west) / 2.0;
    let dy = (h_south - h_north) / 2.0;

    (dx, dy)
}

/// Input data for world handshake calculation
pub struct HandshakeInput<'a> {
    pub heightmap: &'a Tilemap<f32>,
    pub moisture: &'a Tilemap<f32>,
    pub temperature: &'a Tilemap<f32>,
    pub stress_map: &'a Tilemap<f32>,
    pub biomes: &'a Tilemap<crate::biomes::ExtendedBiome>,
    pub hardness_map: Option<&'a Tilemap<f32>>,
}

/// Calculate world-wide handshakes from heightmap and optional hardness (simplified API)
pub fn calculate_world_handshakes(
    heightmap: &Tilemap<f32>,
    hardness_map: Option<&Tilemap<f32>>,
) -> WorldHandshakes {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut handshakes = WorldHandshakes::new(width, height);

    // First pass: compute flow direction using D8 algorithm
    for y in 0..height {
        for x in 0..width {
            let flow_dir = compute_d8_flow_direction(heightmap, x, y);
            handshakes.flow_direction.set(x, y, flow_dir);
        }
    }

    // Second pass: compute flow accumulation
    handshakes.flow_accumulation = compute_flow_accumulation(heightmap, &handshakes.flow_direction);

    // Third pass: compute tile handshakes (basic version)
    for y in 0..height {
        for x in 0..width {
            let corner_heights = calculate_corner_heights(heightmap, x, y);
            let gradient = calculate_gradient(heightmap, x, y);
            let roughness = calculate_roughness(heightmap, x, y);
            let flow_direction = *handshakes.flow_direction.get(x, y);
            let flow_accumulation = *handshakes.flow_accumulation.get(x, y);

            // Basic rock stack without detailed geology
            let rock_stack = calculate_rock_stack(
                heightmap,
                &Tilemap::new_with(width, height, 0.0f32), // dummy stress
                hardness_map,
                x, y,
            );

            let sediment_depth = rock_stack.first()
                .filter(|l| l.rock_type == RockType::Sediment)
                .map(|l| l.thickness)
                .unwrap_or(3);

            let tile_handshake = TileHandshake {
                corner_heights,
                gradient,
                flow_direction,
                flow_accumulation,
                roughness,
                sediment_depth,
                rock_stack,
                surface_minerals: 0.1,
                water_table: 0.3,
                aquifer_pressure: 0.2,
                vegetation_density: 0.5,
                vegetation_pattern: VegetationPattern::Uniform,
            };

            handshakes.handshakes.get_mut(x, y).tile = tile_handshake;
        }
    }

    handshakes
}

/// Calculate world-wide handshakes with full input data
/// This version computes all tile properties including vegetation, hydrology, and geology
pub fn calculate_world_handshakes_full(input: &HandshakeInput) -> WorldHandshakes {
    let width = input.heightmap.width;
    let height = input.heightmap.height;

    let mut handshakes = WorldHandshakes::new(width, height);

    // First pass: compute flow direction using D8 algorithm
    for y in 0..height {
        for x in 0..width {
            let flow_dir = compute_d8_flow_direction(input.heightmap, x, y);
            handshakes.flow_direction.set(x, y, flow_dir);
        }
    }

    // Second pass: compute flow accumulation
    handshakes.flow_accumulation = compute_flow_accumulation(input.heightmap, &handshakes.flow_direction);

    // Third pass: compute full tile handshakes with all properties
    for y in 0..height {
        for x in 0..width {
            let corner_heights = calculate_corner_heights(input.heightmap, x, y);
            let gradient = calculate_gradient(input.heightmap, x, y);
            let roughness = calculate_roughness(input.heightmap, x, y);
            let flow_direction = *handshakes.flow_direction.get(x, y);
            let flow_accumulation = *handshakes.flow_accumulation.get(x, y);

            // Geology
            let rock_stack = calculate_rock_stack(
                input.heightmap,
                input.stress_map,
                input.hardness_map,
                x, y,
            );
            let sediment_depth = calculate_sediment_depth_zlevels(
                input.hardness_map,
                input.moisture,
                input.heightmap,
                x, y,
            );
            let surface_minerals = calculate_surface_minerals(
                input.stress_map,
                input.heightmap,
                x, y,
            );

            // Hydrology
            let water_table = calculate_water_table(
                input.moisture,
                input.heightmap,
                flow_accumulation,
                x, y,
            );
            let aquifer_pressure = calculate_aquifer_pressure(
                input.heightmap,
                input.moisture,
                gradient,
                x, y,
            );

            // Vegetation
            let vegetation_density = calculate_vegetation_density(
                input.biomes,
                input.moisture,
                input.temperature,
                x, y,
            );
            let vegetation_pattern = calculate_vegetation_pattern(
                input.biomes,
                flow_accumulation,
                input.moisture,
                x, y,
            );

            let tile_handshake = TileHandshake {
                corner_heights,
                gradient,
                flow_direction,
                flow_accumulation,
                roughness,
                sediment_depth,
                rock_stack,
                surface_minerals,
                water_table,
                aquifer_pressure,
                vegetation_density,
                vegetation_pattern,
            };

            handshakes.handshakes.get_mut(x, y).tile = tile_handshake;
        }
    }

    handshakes
}

/// Compute D8 flow direction for a single cell
fn compute_d8_flow_direction(heightmap: &Tilemap<f32>, x: usize, y: usize) -> u8 {
    let h_center = *heightmap.get(x, y);

    // D8 offsets: N, NE, E, SE, S, SW, W, NW
    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
    let dist: [f32; 8] = [1.0, 1.414, 1.0, 1.414, 1.0, 1.414, 1.0, 1.414];

    let width = heightmap.width;
    let height = heightmap.height;

    let mut best_slope = 0.0;
    let mut best_dir = FLOW_NONE;

    for dir in 0..8 {
        let nx = (x as i32 + dx[dir]).rem_euclid(width as i32) as usize;
        let ny = y as i32 + dy[dir];

        if ny < 0 || ny >= height as i32 {
            continue;
        }
        let ny = ny as usize;

        let h_neighbor = *heightmap.get(nx, ny);
        let drop = h_center - h_neighbor;
        let slope = drop / dist[dir];

        if slope > best_slope {
            best_slope = slope;
            best_dir = dir as u8;
        }
    }

    best_dir
}

/// Compute flow accumulation using topological sort
fn compute_flow_accumulation(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;

    // D8 offsets
    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

    // Count incoming flows for each cell
    let mut in_degree: Tilemap<usize> = Tilemap::new_with(width, height, 0);

    for y in 0..height {
        for x in 0..width {
            let dir = *flow_dir.get(x, y);
            if dir < 8 {
                let nx = (x as i32 + dx[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = y as i32 + dy[dir as usize];
                if ny >= 0 && ny < height as i32 {
                    *in_degree.get_mut(nx, ny as usize) += 1;
                }
            }
        }
    }

    // Initialize accumulation
    let mut accumulation: Tilemap<f32> = Tilemap::new_with(width, height, 1.0);

    // Collect cells with no incoming flow (sources)
    // (Note: topological sort uses elevation order instead of BFS queue)

    // Sort all cells by elevation (highest first)
    let mut cells: Vec<(usize, usize, f32)> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            cells.push((x, y, *heightmap.get(x, y)));
        }
    }
    cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Process in elevation order (highest first)
    for (x, y, _) in cells {
        let dir = *flow_dir.get(x, y);
        if dir < 8 {
            let nx = (x as i32 + dx[dir as usize]).rem_euclid(width as i32) as usize;
            let ny = y as i32 + dy[dir as usize];
            if ny >= 0 && ny < height as i32 {
                let ny = ny as usize;
                let acc = *accumulation.get(x, y);
                *accumulation.get_mut(nx, ny) += acc;
            }
        }
    }

    accumulation
}
