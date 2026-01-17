//! World data container module
//!
//! Bundles all generated world data into a single struct for easy passing between functions.

use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::{self, ExtendedBiome};
use crate::climate;
use crate::heightmap;
use crate::plates::{self, Plate, PlateId};
use crate::scale::MapScale;
use crate::tilemap::Tilemap;
use crate::water_bodies::{self, WaterBody, WaterBodyId, WaterBodyType};

/// All generated world data bundled together
pub struct WorldData {
    /// Random seed used for generation
    pub seed: u64,
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
}

impl WorldData {
    /// Create a new WorldData from generation outputs
    pub fn new(
        seed: u64,
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
    ) -> Self {
        let width = heightmap.width;
        let height = heightmap.height;
        Self {
            seed,
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
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let scale = MapScale::default();

    // Generate tectonic plates
    let (plate_map, plates) = plates::generate_plates(width, height, None, &mut rng);

    // Calculate stress at plate boundaries
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    let heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seed);

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
        seed,
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
        seed,
    );

    // Apply fantasy lake conversions
    water_bodies::apply_fantasy_lake_conversions(
        &mut extended_biomes,
        &water_bodies_list,
        &water_body_map,
        &temperature,
        &stress_map,
        seed,
    );

    // Place unique biomes
    biomes::place_unique_biomes(
        &mut extended_biomes,
        &heightmap,
        seed,
    );

    WorldData::new(
        seed,
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
    )
}
