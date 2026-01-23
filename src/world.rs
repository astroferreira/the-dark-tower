//! World data container module
//!
//! Bundles all generated world data into a single struct for easy passing between functions.

use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::{self, ExtendedBiome};
use crate::biome_feathering::{self, BiomeFeatherMap, FeatherConfig};
use crate::climate;
use crate::erosion::RiverNetwork;
use crate::heightmap;
use crate::plates::{self, Plate, PlateId};
use crate::scale::MapScale;
use crate::seeds::WorldSeeds;
use crate::tilemap::Tilemap;
use crate::water_bodies::{self, WaterBody, WaterBodyId, WaterBodyType};

/// All generated world data bundled together
pub struct WorldData {
    /// Seeds used for generation (allows recreation)
    pub seeds: WorldSeeds,
    /// Map width in tiles
    pub width: usize,
    /// Map height in tiles
    pub height: usize,
    /// Map scale configuration
    pub scale: MapScale,
    /// Elevation map (meters, negative = underwater)
    pub heightmap: Tilemap<f32>,
    /// Temperature map (Celsius)
    pub temperature: Tilemap<f32>,
    /// Moisture map (0.0-1.0)
    pub moisture: Tilemap<f32>,
    /// Extended biome classification
    pub biomes: Tilemap<ExtendedBiome>,
    /// Tectonic stress map (-1.0 divergent to +1.0 convergent)
    pub stress_map: Tilemap<f32>,
    /// Plate assignment per tile
    pub plate_map: Tilemap<PlateId>,
    /// List of tectonic plates
    pub plates: Vec<Plate>,
    /// Rock hardness map (0.0-1.0, from erosion)
    pub hardness_map: Option<Tilemap<f32>>,
    /// Water body ID map (ocean, lakes, rivers)
    pub water_body_map: Tilemap<WaterBodyId>,
    /// List of water bodies with metadata
    pub water_bodies: Vec<WaterBody>,
    /// Bezier curve river network (Phase 1)
    pub river_network: Option<RiverNetwork>,
    /// Biome feathering map for smooth transitions
    pub biome_feather_map: Option<BiomeFeatherMap>,
}

impl WorldData {
    /// Convenience accessor for master seed
    pub fn seed(&self) -> u64 {
        self.seeds.master
    }

    /// Create a new WorldData from generation outputs
    pub fn new(
        seeds: WorldSeeds,
        scale: MapScale,
        heightmap: Tilemap<f32>,
        temperature: Tilemap<f32>,
        moisture: Tilemap<f32>,
        biomes: Tilemap<ExtendedBiome>,
        stress_map: Tilemap<f32>,
        plate_map: Tilemap<PlateId>,
        plates: Vec<Plate>,
        hardness_map: Option<Tilemap<f32>>,
        water_body_map: Tilemap<WaterBodyId>,
        water_bodies: Vec<WaterBody>,
        river_network: Option<RiverNetwork>,
        biome_feather_map: Option<BiomeFeatherMap>,
    ) -> Self {
        let width = heightmap.width;
        let height = heightmap.height;
        Self {
            seeds,
            width,
            height,
            scale,
            heightmap,
            temperature,
            moisture,
            biomes,
            stress_map,
            plate_map,
            plates,
            hardness_map,
            water_body_map,
            water_bodies,
            river_network,
            biome_feather_map,
        }
    }

    /// Get tile info at coordinates
    pub fn get_tile_info(&self, x: usize, y: usize) -> TileInfo {
        let water_body_id = *self.water_body_map.get(x, y);
        let water_body = self.water_bodies.iter().find(|wb| wb.id == water_body_id);

        TileInfo {
            x,
            y,
            elevation: *self.heightmap.get(x, y),
            temperature: *self.temperature.get(x, y),
            moisture: *self.moisture.get(x, y),
            biome: *self.biomes.get(x, y),
            stress: *self.stress_map.get(x, y),
            plate_id: *self.plate_map.get(x, y),
            hardness: self.hardness_map.as_ref().map(|h| *h.get(x, y)),
            water_body_id,
            water_body_type: water_body.map(|wb| wb.body_type).unwrap_or(WaterBodyType::None),
            water_body_size: water_body.map(|wb| wb.tile_count),
        }
    }

    /// Get physical coordinates in km from tile position
    pub fn get_physical_coords(&self, x: usize, y: usize) -> (f32, f32) {
        let x_km = x as f32 * self.scale.km_per_tile;
        let y_km = y as f32 * self.scale.km_per_tile;
        (x_km, y_km)
    }

    /// Get map size in km
    pub fn map_size_km(&self) -> (f32, f32) {
        self.scale.map_size_km(self.width, self.height)
    }

    /// Check if a tile is coastal (land adjacent to water)
    pub fn is_coastal(&self, x: usize, y: usize) -> bool {
        // Must be land first
        let elevation = *self.heightmap.get(x, y);
        if elevation < 0.0 {
            return false;
        }

        // Check adjacent tiles for water
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (x as i32 + dx).rem_euclid(self.width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, self.height as i32 - 1) as usize;

                let neighbor_elev = *self.heightmap.get(nx, ny);
                if neighbor_elev < 0.0 {
                    return true; // Has adjacent water
                }
            }
        }

        false
    }
}

/// Information about a single tile
#[derive(Clone, Debug)]
pub struct TileInfo {
    pub x: usize,
    pub y: usize,
    pub elevation: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub biome: ExtendedBiome,
    pub stress: f32,
    pub plate_id: PlateId,
    pub hardness: Option<f32>,
    pub water_body_id: WaterBodyId,
    pub water_body_type: WaterBodyType,
    pub water_body_size: Option<usize>,
}

impl TileInfo {
    /// Format elevation as string
    pub fn elevation_str(&self) -> String {
        if self.elevation < 0.0 {
            format!("{:.0}m (underwater)", self.elevation)
        } else {
            format!("{:.0}m", self.elevation)
        }
    }

    /// Format temperature as string
    pub fn temperature_str(&self) -> String {
        format!("{:.1}°C", self.temperature)
    }

    /// Format moisture as string
    pub fn moisture_str(&self) -> String {
        let desc = if self.moisture < 0.2 {
            "arid"
        } else if self.moisture < 0.4 {
            "dry"
        } else if self.moisture < 0.6 {
            "moderate"
        } else if self.moisture < 0.8 {
            "wet"
        } else {
            "saturated"
        };
        format!("{:.2} ({})", self.moisture, desc)
    }

    /// Format stress as string
    pub fn stress_str(&self) -> String {
        if self.stress > 0.3 {
            format!("+{:.2} (convergent/mountains)", self.stress)
        } else if self.stress < -0.3 {
            format!("{:.2} (divergent/rift)", self.stress)
        } else {
            format!("{:.2} (stable)", self.stress)
        }
    }

    /// Check if tile is underwater
    pub fn is_underwater(&self) -> bool {
        self.elevation < 0.0
    }

    /// Format water body info as string
    pub fn water_body_str(&self) -> String {
        match self.water_body_type {
            WaterBodyType::None => "Land".to_string(),
            WaterBodyType::Ocean => "Ocean".to_string(),
            WaterBodyType::Lake => {
                if let Some(size) = self.water_body_size {
                    format!("Lake ({} tiles)", size)
                } else {
                    "Lake".to_string()
                }
            }
            WaterBodyType::River => "River".to_string(),
        }
    }
}

/// Generate a complete world with the given parameters.
/// This is a convenience function that encapsulates the full generation pipeline.
///
/// Note: This version skips erosion for faster generation (useful for exploration/preview).
/// For full quality with erosion, use the main generation pipeline.
pub fn generate_world(width: usize, height: usize, seed: u64) -> WorldData {
    // Special seed 666: Generate a minimal test world (4x4) for debugging
    if seed == 666 {
        return generate_test_world();
    }

    let seeds = WorldSeeds::from_master(seed);
    let mut rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
    let scale = MapScale::default();

    // Generate tectonic plates
    let (plate_map, plates) = plates::generate_plates(width, height, None, &mut rng);

    // Calculate stress at plate boundaries
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    let heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);

    // Generate climate
    let temperature = climate::generate_temperature(&heightmap, width, height);
    let moisture = climate::generate_moisture(&heightmap, width, height);

    // Generate extended biomes
    let biome_config = biomes::WorldBiomeConfig::default();
    let mut extended_biomes = biomes::generate_extended_biomes(
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        &biome_config,
        seeds.biomes,
    );

    // Detect water bodies
    let (water_body_map, water_bodies_list) = water_bodies::detect_water_bodies(&heightmap);

    // Apply rare biome replacements
    biomes::apply_biome_replacements(
        &mut extended_biomes,
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        seeds.biomes,
    );

    // Apply fantasy lake conversions
    water_bodies::apply_fantasy_lake_conversions(
        &mut extended_biomes,
        &water_bodies_list,
        &water_body_map,
        &temperature,
        &stress_map,
        seeds.biomes,
    );

    // Place unique biomes
    biomes::place_unique_biomes(
        &mut extended_biomes,
        &heightmap,
        seeds.biomes,
    );

    // Compute biome feathering map for smooth transitions
    let feather_config = FeatherConfig::default();
    let biome_feather_map = biome_feathering::compute_biome_feathering(
        &extended_biomes,
        &feather_config,
        seeds.biomes,
    );

    // Generate Bezier river network
    let river_network = crate::erosion::trace_bezier_rivers(&heightmap, None, seeds.rivers);

    WorldData::new(
        seeds,
        scale,
        heightmap,
        temperature,
        moisture,
        extended_biomes,
        stress_map,
        plate_map,
        plates,
        None, // No hardness map without erosion
        water_body_map,
        water_bodies_list,
        Some(river_network),
        Some(biome_feather_map),
    )
}

/// Generate a minimal test world (4x4) for debugging colonist behavior.
/// Used when seed 666 is specified.
/// All tiles are flat grassland, perfect for testing simulation mechanics.
pub fn generate_test_world() -> WorldData {
    const SIZE: usize = 4;

    // Create flat terrain (all land at 0.5 elevation)
    let heightmap = Tilemap::new_with(SIZE, SIZE, 0.5);

    // Mild temperate climate
    let temperature = Tilemap::new_with(SIZE, SIZE, 15.0);

    // Moderate moisture
    let moisture = Tilemap::new_with(SIZE, SIZE, 0.5);

    // All temperate grassland - ideal for all activities
    let biomes = Tilemap::new_with(SIZE, SIZE, ExtendedBiome::TemperateGrassland);

    // No tectonic stress
    let stress_map = Tilemap::new_with(SIZE, SIZE, 0.0);

    // Single plate
    let plate_map = Tilemap::new_with(SIZE, SIZE, PlateId(0));
    let plates = vec![plates::Plate {
        id: PlateId(0),
        plate_type: plates::PlateType::Continental,
        velocity: plates::Vec2::new(0.0, 0.0),
        base_elevation: 0.5,
        color: [100, 180, 100],
    }];

    // No water bodies in test world
    let water_body_map = Tilemap::new_with(SIZE, SIZE, WaterBodyId::NONE);
    let water_bodies = vec![];

    WorldData {
        seeds: WorldSeeds::from_master(666),
        width: SIZE,
        height: SIZE,
        scale: MapScale::default(),
        heightmap,
        temperature,
        moisture,
        biomes,
        stress_map,
        plate_map,
        plates,
        hardness_map: None,
        water_body_map,
        water_bodies,
        river_network: None,
        biome_feather_map: None,
    }
}
