//! World data container module
//!
//! Bundles all generated world data into a single struct for easy passing between functions.

use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::{self, ExtendedBiome};
use crate::biome_feathering::{self, BiomeFeatherMap, FeatherConfig};
use crate::climate;
use crate::erosion::RiverNetwork;
use crate::heightmap::{self, LavaState, VolcanoLocation};
use crate::microclimate::{self, MicroclimateModifiers, MicroclimateConfig};
use crate::plates::{self, Plate, PlateId};
use crate::region::{self, WorldHandshakes};
use crate::scale::MapScale;
use crate::seasons::{self, SeasonalClimate, Season};
use crate::seeds::WorldSeeds;
use crate::tilemap::Tilemap;
use crate::underground_water::{self, UndergroundWater, UndergroundWaterParams, TileWaterFeatures};
use crate::water_bodies::{self, WaterBody, WaterBodyId, WaterBodyType};
use crate::weather_zones::{self, WeatherZone};

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
    /// Water depth map (water_surface - terrain, 0 = dry land)
    pub water_depth: Tilemap<f32>,
    /// Flow accumulation map (D8 drainage area, higher = more upstream catchment)
    /// Used to identify rivers: cells with flow_accumulation > threshold are rivers
    pub flow_accumulation: Option<Tilemap<f32>>,
    /// Bezier curve river network (Phase 1)
    pub river_network: Option<RiverNetwork>,
    /// Biome feathering map for smooth transitions
    pub biome_feather_map: Option<BiomeFeatherMap>,
    /// Microclimate modifiers (valley warmth, ridge cooling, etc.)
    pub microclimate: Option<Tilemap<MicroclimateModifiers>>,
    /// Seasonal climate data (temperature/moisture by season)
    pub seasonal_climate: Option<SeasonalClimate>,
    /// Extreme weather zones (hurricane, monsoon, etc.)
    pub weather_zones: Option<Tilemap<WeatherZone>>,
    /// Region handshake data for hierarchical zoom
    pub handshakes: Option<WorldHandshakes>,
    /// Current season for display (can be cycled in explorer)
    pub current_season: Season,
    /// Underground water features (aquifers, springs, waterfalls)
    pub underground_water: Option<UndergroundWater>,
    /// Lava state map for active volcanoes (molten, flowing, cooled)
    pub lava_map: Option<Tilemap<LavaState>>,
    /// List of volcano locations
    pub volcanoes: Vec<VolcanoLocation>,
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
        water_depth: Tilemap<f32>,
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
            water_depth,
            flow_accumulation: None,
            river_network,
            biome_feather_map,
            microclimate: None,
            seasonal_climate: None,
            weather_zones: None,
            handshakes: None,
            current_season: Season::Summer,
            underground_water: None,
            lava_map: None,
            volcanoes: Vec::new(),
        }
    }

    /// Create a new WorldData with all climate features
    pub fn new_with_climate(
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
        water_depth: Tilemap<f32>,
        river_network: Option<RiverNetwork>,
        biome_feather_map: Option<BiomeFeatherMap>,
        microclimate: Option<Tilemap<MicroclimateModifiers>>,
        seasonal_climate: Option<SeasonalClimate>,
        weather_zones: Option<Tilemap<WeatherZone>>,
        underground_water: Option<UndergroundWater>,
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
            water_depth,
            flow_accumulation: None,
            river_network,
            biome_feather_map,
            microclimate,
            seasonal_climate,
            weather_zones,
            handshakes: None,
            current_season: Season::Summer,
            underground_water,
            lava_map: None,
            volcanoes: Vec::new(),
        }
    }

    /// Set flow accumulation map (computed after erosion)
    pub fn set_flow_accumulation(&mut self, flow_acc: Tilemap<f32>) {
        self.flow_accumulation = Some(flow_acc);
    }

    /// Set volcanic features (lava map and volcano locations)
    pub fn set_volcanic_features(&mut self, lava_map: Tilemap<LavaState>, volcanoes: Vec<VolcanoLocation>) {
        self.lava_map = Some(lava_map);
        self.volcanoes = volcanoes;
    }

    /// Get tile info at coordinates
    pub fn get_tile_info(&self, x: usize, y: usize) -> TileInfo {
        let water_body_id = *self.water_body_map.get(x, y);
        let water_body = self.water_bodies.iter().find(|wb| wb.id == water_body_id);

        // Get underground water features if available
        let water_features = self.underground_water
            .as_ref()
            .map(|uw: &UndergroundWater| uw.get_tile_features(x, y))
            .unwrap_or_default();

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
            water_depth: *self.water_depth.get(x, y),
            water_features,
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

    /// Check if a tile is in the northern hemisphere
    pub fn is_northern_hemisphere(&self, y: usize) -> bool {
        y < self.height / 2
    }

    /// Get seasonal temperature at a tile (if seasonal data is available)
    pub fn get_seasonal_temperature(&self, x: usize, y: usize) -> f32 {
        if let Some(ref seasonal) = self.seasonal_climate {
            let is_north = self.is_northern_hemisphere(y);
            seasonal.get_temperature(x, y, self.current_season, is_north)
        } else {
            *self.temperature.get(x, y)
        }
    }

    /// Get seasonal moisture at a tile (if seasonal data is available)
    pub fn get_seasonal_moisture(&self, x: usize, y: usize) -> f32 {
        if let Some(ref seasonal) = self.seasonal_climate {
            let is_north = self.is_northern_hemisphere(y);
            seasonal.get_moisture(x, y, self.current_season, is_north)
        } else {
            *self.moisture.get(x, y)
        }
    }

    /// Get weather zone at a tile (if weather data is available)
    pub fn get_weather_zone(&self, x: usize, y: usize) -> Option<&WeatherZone> {
        self.weather_zones.as_ref().map(|wz: &Tilemap<WeatherZone>| wz.get(x, y))
    }

    /// Get microclimate modifiers at a tile
    pub fn get_microclimate(&self, x: usize, y: usize) -> Option<&MicroclimateModifiers> {
        self.microclimate.as_ref().map(|mc: &Tilemap<MicroclimateModifiers>| mc.get(x, y))
    }

    /// Cycle to the next season
    pub fn next_season(&mut self) {
        self.current_season = self.current_season.next();
    }

    /// Cycle to the previous season
    pub fn prev_season(&mut self) {
        self.current_season = self.current_season.prev();
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
    /// Water depth at this tile (water_surface - terrain, 0 = dry land)
    pub water_depth: f32,
    /// Underground water features (aquifers, springs, waterfalls)
    pub water_features: TileWaterFeatures,
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

    /// Format underground water features as string
    pub fn water_features_str(&self) -> String {
        self.water_features.to_string()
    }

    /// Check if tile has any underground water features
    pub fn has_water_features(&self) -> bool {
        self.water_features.has_any()
    }
}

impl WorldData {
    /// Get underground water statistics if available
    pub fn underground_water_stats(&self) -> Option<underground_water::UndergroundWaterStats> {
        self.underground_water.as_ref().map(|uw: &UndergroundWater| uw.stats())
    }
}

/// Generate a complete world with the given parameters.
/// This is a convenience function that encapsulates the full generation pipeline.
///
/// Note: This version skips erosion for faster generation (useful for exploration/preview).
/// For full quality with erosion, use the main generation pipeline.
pub fn generate_world(width: usize, height: usize, seed: u64) -> WorldData {
    generate_world_with_style(width, height, seed, plates::WorldStyle::default())
}

/// Generate a complete world with a specific world style.
pub fn generate_world_with_style(width: usize, height: usize, seed: u64, world_style: plates::WorldStyle) -> WorldData {
    // Special seed 666: Generate a minimal test world (4x4) for debugging
    if seed == 666 {
        return generate_test_world();
    }

    let seeds = WorldSeeds::from_master(seed);
    let mut rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
    let scale = MapScale::default();

    // Generate tectonic plates
    let (plate_map, plates) = plates::generate_plates(width, height, None, world_style, &mut rng);

    // Calculate stress at plate boundaries
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    let heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);

    // Generate climate with domain warping for organic zone boundaries
    let temperature = climate::generate_temperature_with_seed(
        &heightmap, width, height, climate::ClimateMode::Globe, seeds.climate
    );
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

    // Detect water bodies with water depth
    let (water_body_map, water_bodies_list, water_depth) = water_bodies::detect_water_bodies(&heightmap);

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

    // Calculate region handshakes for hierarchical zoom
    let handshake_input = region::HandshakeInput {
        heightmap: &heightmap,
        moisture: &moisture,
        temperature: &temperature,
        stress_map: &stress_map,
        biomes: &extended_biomes,
        hardness_map: None,
    };
    let mut world_handshakes = region::calculate_world_handshakes_full(&handshake_input);
    // Add river crossings to handshakes
    region::rivers::calculate_river_crossings(&mut world_handshakes.handshakes, &river_network);

    // Generate microclimate modifiers
    let microclimate_config = MicroclimateConfig::default();
    let microclimate_map = microclimate::generate_microclimates(
        &heightmap,
        &extended_biomes,
        &water_body_map,
        &water_bodies_list,
        &microclimate_config,
    );

    // Generate seasonal climate data
    let seasonal_climate = seasons::generate_seasonal_climate(
        &temperature,
        &moisture,
        &heightmap,
    );

    // Generate weather zones
    let weather_zone_map = weather_zones::generate_weather_zones(
        &heightmap,
        &temperature,
        &moisture,
    );

    // Generate underground water features (aquifers, springs, waterfalls)
    println!("Generating underground water features...");
    let underground_water_params = UndergroundWaterParams::default();
    let underground_water_features = UndergroundWater::generate(
        &heightmap,
        &moisture,
        &stress_map,
        None, // No hardness map without erosion
        &underground_water_params,
    );

    // Log underground water statistics
    let uw_stats = underground_water_features.stats();
    println!("Underground water: {} aquifer tiles ({} unconfined, {} confined, {} perched)",
             uw_stats.aquifer_tiles,
             uw_stats.unconfined_aquifers,
             uw_stats.confined_aquifers,
             uw_stats.perched_aquifers);
    println!("Springs: {} total ({} seepage, {} artesian, {} thermal, {} karst)",
             uw_stats.spring_count,
             uw_stats.seepage_springs,
             uw_stats.artesian_springs,
             uw_stats.thermal_springs,
             uw_stats.karst_springs);
    if uw_stats.waterfall_count > 0 {
        println!("Waterfalls: {} (max height: {:.0}m)", uw_stats.waterfall_count, uw_stats.max_waterfall_height);
    }

    let mut world = WorldData::new_with_climate(
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
        water_depth,
        Some(river_network),
        Some(biome_feather_map),
        Some(microclimate_map),
        Some(seasonal_climate),
        Some(weather_zone_map),
        Some(underground_water_features),
    );

    // Attach region handshakes
    world.handshakes = Some(world_handshakes);

    world
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
    let water_depth = Tilemap::new_with(SIZE, SIZE, 0.0f32);

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
        water_depth,
        flow_accumulation: None,
        river_network: None,
        biome_feather_map: None,
        microclimate: None,
        seasonal_climate: None,
        weather_zones: None,
        handshakes: None,
        current_season: Season::Summer,
        underground_water: None,
        lava_map: None,
        volcanoes: Vec::new(),
    }
}
