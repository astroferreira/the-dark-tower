//! Water body detection and classification
//!
//! Identifies and classifies water bodies as ocean, lakes, or rivers based on
//! connectivity analysis. This enables transforming entire lakes into special
//! biomes rather than individual tiles.

use std::collections::VecDeque;
use crate::tilemap::Tilemap;
use crate::biomes::ExtendedBiome;
use crate::erosion::rivers::{compute_flow_direction, compute_flow_accumulation};

/// Type of water body
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum WaterBodyType {
    #[default]
    None,   // Land tile
    Ocean,  // Connected to map edge (top or bottom)
    Lake,   // Isolated inland water body
    River,  // Linear high-flow water feature (future)
}

impl WaterBodyType {
    pub fn display_name(&self) -> &'static str {
        match self {
            WaterBodyType::None => "Land",
            WaterBodyType::Ocean => "Ocean",
            WaterBodyType::Lake => "Lake",
            WaterBodyType::River => "River",
        }
    }
}

/// Water body identifier (0 = land/none, 1+ = water body ID)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WaterBodyId(pub u16);

impl WaterBodyId {
    pub const NONE: WaterBodyId = WaterBodyId(0);
    pub const OCEAN: WaterBodyId = WaterBodyId(1);

    pub fn is_none(&self) -> bool {
        self.0 == 0
    }

    pub fn is_ocean(&self) -> bool {
        self.0 == 1
    }

    pub fn is_lake(&self) -> bool {
        self.0 > 1
    }
}

/// Information about a water body
#[derive(Clone, Debug)]
pub struct WaterBody {
    pub id: WaterBodyId,
    pub body_type: WaterBodyType,
    pub tile_count: usize,
    pub min_elevation: f32,
    pub max_elevation: f32,
    pub avg_elevation: f32,
    pub touches_north_edge: bool,
    pub touches_south_edge: bool,
    /// Bounding box (min_x, min_y, max_x, max_y)
    pub bounds: (usize, usize, usize, usize),
}

impl WaterBody {
    fn new(id: WaterBodyId, body_type: WaterBodyType) -> Self {
        Self {
            id,
            body_type,
            tile_count: 0,
            min_elevation: f32::MAX,
            max_elevation: f32::MIN,
            avg_elevation: 0.0,
            touches_north_edge: false,
            touches_south_edge: false,
            bounds: (usize::MAX, usize::MAX, 0, 0),
        }
    }

    fn add_tile(&mut self, x: usize, y: usize, elevation: f32, height: usize) {
        self.tile_count += 1;
        self.min_elevation = self.min_elevation.min(elevation);
        self.max_elevation = self.max_elevation.max(elevation);

        // Running average
        let n = self.tile_count as f32;
        self.avg_elevation = self.avg_elevation * (n - 1.0) / n + elevation / n;

        // Update bounds
        self.bounds.0 = self.bounds.0.min(x);
        self.bounds.1 = self.bounds.1.min(y);
        self.bounds.2 = self.bounds.2.max(x);
        self.bounds.3 = self.bounds.3.max(y);

        // Check edge touches
        if y == 0 {
            self.touches_north_edge = true;
        }
        if y == height - 1 {
            self.touches_south_edge = true;
        }
    }

    /// Get the approximate width of the water body
    pub fn width(&self) -> usize {
        if self.bounds.2 >= self.bounds.0 {
            self.bounds.2 - self.bounds.0 + 1
        } else {
            0
        }
    }

    /// Get the approximate height of the water body
    pub fn height(&self) -> usize {
        if self.bounds.3 >= self.bounds.1 {
            self.bounds.3 - self.bounds.1 + 1
        } else {
            0
        }
    }
}

/// Threshold for flow accumulation to be considered a river
/// Higher = only major rivers, lower = includes small streams
const RIVER_FLOW_THRESHOLD: f32 = 50.0;

/// Sea level (0.0 by convention)
const SEA_LEVEL: f32 = 0.0;

/// Epsilon for floating point comparisons
const WATER_EPSILON: f32 = 1e-4;

/// Detect and classify all water bodies in the world.
///
/// Algorithm:
/// 1. Use water_level map to find submerged tiles (water_level > terrain)
/// 2. Flood-fill from map edges to identify ocean (below sea level + connected)
/// 3. Remaining submerged tiles are lakes (including alpine lakes)
/// 4. Use flow accumulation to detect rivers on DRY land
///
/// Returns a tilemap of water body IDs, a list of water body info, and the water depth map.
pub fn detect_water_bodies(
    heightmap: &Tilemap<f32>,
) -> (Tilemap<WaterBodyId>, Vec<WaterBody>, Tilemap<f32>) {
    // Compute water level (fills depressions to find lake surfaces)
    let water_level = compute_water_level(heightmap);
    // Compute flow for river detection
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);
    detect_water_bodies_full(heightmap, &water_level, &flow_acc)
}

/// Compute water surface level using depression filling.
/// Returns a map where water_level >= terrain_height everywhere.
/// If water_level > terrain_height, the tile is submerged (lake/ocean).
fn compute_water_level(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    use crate::erosion::rivers::fill_depressions_public;
    fill_depressions_public(heightmap)
}

/// Check if a tile is submerged (water surface above terrain)
fn is_submerged(terrain_h: f32, water_h: f32) -> bool {
    water_h > terrain_h + WATER_EPSILON
}

/// Check if a tile is below sea level
fn is_below_sea_level(terrain_h: f32) -> bool {
    terrain_h <= SEA_LEVEL
}

/// Detect water bodies with pre-computed water level and flow accumulation.
/// Optimized to combine multiple passes into fewer iterations.
pub fn detect_water_bodies_full(
    heightmap: &Tilemap<f32>,
    water_level: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
) -> (Tilemap<WaterBodyId>, Vec<WaterBody>, Tilemap<f32>) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Create water body ID map and water depth map
    let mut water_map = Tilemap::new_with(width, height, WaterBodyId::NONE);
    let mut water_depth = Tilemap::new_with(width, height, 0.0f32);
    let mut water_bodies = Vec::new();

    // COMBINED PASS: Calculate water depth AND identify water candidates simultaneously
    // This replaces two separate full-map iterations with one
    let mut is_water_candidate = Tilemap::new_with(width, height, false);
    let mut next_id = 2u16; // 1 is reserved for ocean

    for y in 0..height {
        for x in 0..width {
            let terrain_h = *heightmap.get(x, y);
            let water_h = *water_level.get(x, y);

            // Calculate water depth (positive = submerged, zero = dry)
            let depth = (water_h - terrain_h).max(0.0);
            water_depth.set(x, y, depth);

            // Water if: submerged (alpine lake) OR below sea level (ocean/coastal)
            if is_submerged(terrain_h, water_h) || is_below_sea_level(terrain_h) {
                is_water_candidate.set(x, y, true);
            }
        }
    }

    // Step 2: Ocean detection - flood fill from polar edges (top/bottom of map)
    // Only below-sea-level tiles connected to edges become ocean
    let mut visited = Tilemap::new_with(width, height, false);
    let mut ocean = WaterBody::new(WaterBodyId::OCEAN, WaterBodyType::Ocean);
    let mut queue = VecDeque::new();

    // Seed from top edge (below sea level tiles only)
    for x in 0..width {
        let terrain_h = *heightmap.get(x, 0);
        if is_below_sea_level(terrain_h) {
            queue.push_back((x, 0));
            visited.set(x, 0, true);
        }
    }

    // Seed from bottom edge (below sea level tiles only)
    for x in 0..width {
        let terrain_h = *heightmap.get(x, height - 1);
        if is_below_sea_level(terrain_h) {
            queue.push_back((x, height - 1));
            visited.set(x, height - 1, true);
        }
    }

    // BFS to find all ocean-connected water (only below sea level can be ocean)
    while let Some((x, y)) = queue.pop_front() {
        water_map.set(x, y, WaterBodyId::OCEAN);
        ocean.add_tile(x, y, *heightmap.get(x, y), height);

        for (nx, ny) in heightmap.neighbors(x, y) {
            let neighbor_h = *heightmap.get(nx, ny);
            // Ocean can only spread to below-sea-level tiles
            if is_below_sea_level(neighbor_h) && !*visited.get(nx, ny) {
                visited.set(nx, ny, true);
                queue.push_back((nx, ny));
            }
        }
    }

    if ocean.tile_count > 0 {
        water_bodies.push(ocean);
    }

    // Step 3: Lake detection - find water candidates NOT connected to ocean
    // This includes alpine lakes (above sea level) and inland seas (below sea level)
    for y in 0..height {
        for x in 0..width {
            // Skip if already visited (ocean) or not a water candidate
            if *visited.get(x, y) || !*is_water_candidate.get(x, y) {
                continue;
            }

            // Found a new lake - flood fill to find all connected tiles
            let lake_id = WaterBodyId(next_id);
            next_id += 1;

            let mut lake = WaterBody::new(lake_id, WaterBodyType::Lake);
            let mut lake_queue = VecDeque::new();

            lake_queue.push_back((x, y));
            visited.set(x, y, true);

            while let Some((lx, ly)) = lake_queue.pop_front() {
                water_map.set(lx, ly, lake_id);
                lake.add_tile(lx, ly, *heightmap.get(lx, ly), height);

                for (nx, ny) in heightmap.neighbors(lx, ly) {
                    // Lakes can spread to any water candidate tile
                    if *is_water_candidate.get(nx, ny) && !*visited.get(nx, ny) {
                        visited.set(nx, ny, true);
                        lake_queue.push_back((nx, ny));
                    }
                }
            }

            // If lake touches an edge AND is below sea level, it's actually ocean
            let is_below_sea = lake.min_elevation <= SEA_LEVEL;
            if (lake.touches_north_edge || lake.touches_south_edge) && is_below_sea {
                // Reclassify this lake as ocean
                for ly in lake.bounds.1..=lake.bounds.3 {
                    for lx in lake.bounds.0..=lake.bounds.2 {
                        if lx < width && ly < height && water_map.get(lx, ly).0 == lake_id.0 {
                            water_map.set(lx, ly, WaterBodyId::OCEAN);
                        }
                    }
                }

                // Merge with ocean body
                if let Some(ocean_body) = water_bodies.iter_mut().find(|wb| wb.id == WaterBodyId::OCEAN) {
                    ocean_body.tile_count += lake.tile_count;
                    ocean_body.min_elevation = ocean_body.min_elevation.min(lake.min_elevation);
                    ocean_body.max_elevation = ocean_body.max_elevation.max(lake.max_elevation);
                    let total = ocean_body.tile_count as f32;
                    let old = (total - lake.tile_count as f32) / total;
                    let new = lake.tile_count as f32 / total;
                    ocean_body.avg_elevation = ocean_body.avg_elevation * old + lake.avg_elevation * new;
                    ocean_body.bounds.0 = ocean_body.bounds.0.min(lake.bounds.0);
                    ocean_body.bounds.1 = ocean_body.bounds.1.min(lake.bounds.1);
                    ocean_body.bounds.2 = ocean_body.bounds.2.max(lake.bounds.2);
                    ocean_body.bounds.3 = ocean_body.bounds.3.max(lake.bounds.3);
                } else {
                    lake.body_type = WaterBodyType::Ocean;
                    lake.id = WaterBodyId::OCEAN;
                    water_bodies.push(lake);
                }
                next_id -= 1;
            } else {
                // It's a real lake (including alpine lakes above sea level)
                water_bodies.push(lake);
            }
        }
    }

    // Step 4: River detection - DRY LAND tiles with high flow accumulation
    // Rivers are NOT submerged - they flow on the surface
    let river_id = WaterBodyId(next_id);
    let mut river = WaterBody::new(river_id, WaterBodyType::River);

    for y in 0..height {
        for x in 0..width {
            // Only mark as river if:
            // 1. Not already water (lake/ocean)
            // 2. High flow accumulation
            if water_map.get(x, y).is_none() {
                let flow = *flow_acc.get(x, y);
                if flow >= RIVER_FLOW_THRESHOLD {
                    water_map.set(x, y, river_id);
                    river.add_tile(x, y, *heightmap.get(x, y), height);
                }
            }
        }
    }

    if river.tile_count > 0 {
        water_bodies.push(river);
    }

    (water_map, water_bodies, water_depth)
}

/// Get all tiles belonging to a specific water body.
pub fn get_water_body_tiles(
    water_map: &Tilemap<WaterBodyId>,
    body_id: WaterBodyId,
) -> Vec<(usize, usize)> {
    let mut tiles = Vec::new();

    for y in 0..water_map.height {
        for x in 0..water_map.width {
            if *water_map.get(x, y) == body_id {
                tiles.push((x, y));
            }
        }
    }

    tiles
}

/// Convert an entire water body to a specific biome.
pub fn convert_water_body(
    biomes: &mut Tilemap<ExtendedBiome>,
    water_map: &Tilemap<WaterBodyId>,
    body_id: WaterBodyId,
    target_biome: ExtendedBiome,
) {
    for y in 0..water_map.height {
        for x in 0..water_map.width {
            if *water_map.get(x, y) == body_id {
                biomes.set(x, y, target_biome);
            }
        }
    }
}

/// Find lakes suitable for fantasy conversion.
pub fn find_convertible_lakes(
    water_bodies: &[WaterBody],
    min_size: usize,
    max_size: usize,
) -> Vec<WaterBodyId> {
    water_bodies
        .iter()
        .filter(|wb| {
            wb.body_type == WaterBodyType::Lake
                && wb.tile_count >= min_size
                && wb.tile_count <= max_size
        })
        .map(|wb| wb.id)
        .collect()
}

/// Get a water body by its ID.
pub fn get_water_body(water_bodies: &[WaterBody], id: WaterBodyId) -> Option<&WaterBody> {
    water_bodies.iter().find(|wb| wb.id == id)
}

/// Count lakes in the water body list.
pub fn count_lakes(water_bodies: &[WaterBody]) -> usize {
    water_bodies
        .iter()
        .filter(|wb| wb.body_type == WaterBodyType::Lake)
        .count()
}

/// Get statistics about water bodies.
pub fn water_body_stats(water_bodies: &[WaterBody]) -> WaterBodyStats {
    let mut stats = WaterBodyStats::default();

    for wb in water_bodies {
        match wb.body_type {
            WaterBodyType::Ocean => {
                stats.ocean_tiles += wb.tile_count;
            }
            WaterBodyType::Lake => {
                stats.lake_count += 1;
                stats.lake_tiles += wb.tile_count;
                stats.smallest_lake = stats.smallest_lake.min(wb.tile_count);
                stats.largest_lake = stats.largest_lake.max(wb.tile_count);
            }
            WaterBodyType::River => {
                stats.river_tiles += wb.tile_count;
            }
            WaterBodyType::None => {}
        }
    }

    if stats.lake_count > 0 {
        stats.avg_lake_size = stats.lake_tiles as f32 / stats.lake_count as f32;
    }

    stats
}

/// Statistics about water bodies
#[derive(Clone, Debug, Default)]
pub struct WaterBodyStats {
    pub ocean_tiles: usize,
    pub lake_count: usize,
    pub lake_tiles: usize,
    pub river_tiles: usize,
    pub smallest_lake: usize,
    pub largest_lake: usize,
    pub avg_lake_size: f32,
}

impl WaterBodyStats {
    pub fn total_water_tiles(&self) -> usize {
        self.ocean_tiles + self.lake_tiles + self.river_tiles
    }
}

/// Get average temperature for tiles in a water body
pub fn get_lake_avg_temperature(
    water_body: &WaterBody,
    water_map: &Tilemap<WaterBodyId>,
    temperature: &Tilemap<f32>,
) -> f32 {
    let mut sum = 0.0;
    let mut count = 0;

    for y in water_body.bounds.1..=water_body.bounds.3.min(water_map.height - 1) {
        for x in water_body.bounds.0..=water_body.bounds.2.min(water_map.width - 1) {
            if *water_map.get(x, y) == water_body.id {
                sum += *temperature.get(x, y);
                count += 1;
            }
        }
    }

    if count > 0 { sum / count as f32 } else { 15.0 }
}

/// Get average stress (volcanic activity) for tiles near a water body
pub fn get_lake_avg_stress(
    water_body: &WaterBody,
    water_map: &Tilemap<WaterBodyId>,
    stress_map: &Tilemap<f32>,
) -> f32 {
    let mut sum = 0.0;
    let mut count = 0;

    // Check tiles in and around the lake bounds
    let y_start = water_body.bounds.1.saturating_sub(2);
    let y_end = (water_body.bounds.3 + 2).min(water_map.height - 1);
    let x_start = water_body.bounds.0.saturating_sub(2);
    let x_end = (water_body.bounds.2 + 2).min(water_map.width - 1);

    for y in y_start..=y_end {
        for x in x_start..=x_end {
            sum += stress_map.get(x, y).abs();
            count += 1;
        }
    }

    if count > 0 { sum / count as f32 } else { 0.0 }
}

/// Determine what fantasy biome a lake should become based on conditions
pub fn determine_lake_fantasy_biome(
    water_body: &WaterBody,
    avg_temp: f32,
    avg_stress: f32,
    rng_value: f32, // 0.0-1.0 random value for this lake
) -> Option<ExtendedBiome> {
    // Only convert some lakes (based on rng_value and conditions)
    // Higher chance for more extreme conditions

    // Frozen Lake: Very cold regions
    if avg_temp < -5.0 && rng_value < 0.7 {
        return Some(ExtendedBiome::FrozenLake);
    }

    // Lava Lake: High volcanic stress
    if avg_stress > 0.4 && rng_value < 0.5 {
        return Some(ExtendedBiome::LavaLake);
    }

    // Acid Lake: Moderate volcanic + moderate cold (geothermal)
    if avg_stress > 0.2 && avg_temp < 10.0 && rng_value < 0.3 {
        return Some(ExtendedBiome::AcidLake);
    }

    // Bioluminescent Water: Warm, deep lakes
    if avg_temp > 20.0 && water_body.tile_count > 10 && rng_value < 0.2 {
        return Some(ExtendedBiome::BioluminescentWater);
    }

    // Small chance for any lake to become special
    if rng_value < 0.05 {
        // Pick based on conditions
        if avg_temp < 5.0 {
            return Some(ExtendedBiome::FrozenLake);
        } else if avg_stress > 0.15 {
            return Some(ExtendedBiome::LavaLake);
        }
    }

    None
}

/// Apply fantasy biome conversions to lakes based on conditions.
/// This transforms entire lakes into special biomes like LavaLake, FrozenLake, etc.
pub fn apply_fantasy_lake_conversions(
    biomes: &mut Tilemap<ExtendedBiome>,
    water_bodies: &[WaterBody],
    water_map: &Tilemap<WaterBodyId>,
    temperature: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> usize {
    use rand::SeedableRng;
    use rand::Rng;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0x1A4E));
    let mut converted_count = 0;

    for water_body in water_bodies {
        // Only process lakes (not ocean or rivers)
        if water_body.body_type != WaterBodyType::Lake {
            continue;
        }

        // Skip very tiny lakes (1-2 tiles)
        if water_body.tile_count < 3 {
            continue;
        }

        let avg_temp = get_lake_avg_temperature(water_body, water_map, temperature);
        let avg_stress = get_lake_avg_stress(water_body, water_map, stress_map);
        let rng_value: f32 = rng.gen();

        if let Some(fantasy_biome) = determine_lake_fantasy_biome(
            water_body,
            avg_temp,
            avg_stress,
            rng_value,
        ) {
            convert_water_body(biomes, water_map, water_body.id, fantasy_biome);
            converted_count += 1;
        }
    }

    converted_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_body_detection_simple() {
        // Create a simple 10x10 heightmap with a lake in the middle
        let mut heightmap = Tilemap::new_with(10, 10, 100.0);

        // Create ocean at top edge
        for x in 0..10 {
            heightmap.set(x, 0, -50.0);
            heightmap.set(x, 1, -30.0);
        }

        // Create a lake in the middle (not touching edges)
        heightmap.set(4, 5, -20.0);
        heightmap.set(5, 5, -25.0);
        heightmap.set(5, 6, -15.0);

        let (water_map, water_bodies, _water_depth) = detect_water_bodies(&heightmap);

        // Should have ocean and one lake
        assert!(water_bodies.len() >= 2);

        // Check that top edge is ocean
        assert!(water_map.get(0, 0).is_ocean());
        assert!(water_map.get(5, 0).is_ocean());

        // Check that the lake is identified as lake (not ocean)
        let lake_id = *water_map.get(5, 5);
        assert!(lake_id.is_lake());

        // Check stats
        let stats = water_body_stats(&water_bodies);
        assert!(stats.lake_count >= 1);
    }
}
