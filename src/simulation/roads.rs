//! Road system - pathways connecting tribe settlements

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Ordering;

use crate::simulation::types::{TileCoord, TribeId};
use crate::world::WorldData;
use crate::biomes::ExtendedBiome;

/// A segment of road between two adjacent tiles
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadSegment {
    pub from: TileCoord,
    pub to: TileCoord,
    pub road_type: RoadType,
    pub owner: Option<TribeId>,
    pub condition: f32,
}

impl RoadSegment {
    pub fn new(from: TileCoord, to: TileCoord, road_type: RoadType, owner: Option<TribeId>) -> Self {
        RoadSegment {
            from,
            to,
            road_type,
            owner,
            condition: 1.0,
        }
    }

    /// Decay the road condition
    pub fn decay(&mut self, rate: f32) {
        self.condition = (self.condition - rate).max(0.0);
    }

    /// Repair the road
    pub fn repair(&mut self, amount: f32) {
        self.condition = (self.condition + amount).min(1.0);
    }
}

/// Types of roads with different qualities
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadType {
    Trail,      // '.' - Stone Age, basic path
    Road,       // '-' - Bronze Age+, improved road
    PavedRoad,  // '=' - Classical+, best quality
}

impl RoadType {
    /// Get the ASCII character for this road type
    pub fn map_char(&self) -> char {
        match self {
            RoadType::Trail => '.',
            RoadType::Road => '-',
            RoadType::PavedRoad => '=',
        }
    }

    /// Get the color for this road type (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            RoadType::Trail => (139, 119, 101),    // Dirt color
            RoadType::Road => (160, 140, 120),     // Tan
            RoadType::PavedRoad => (180, 180, 180), // Gray stone
        }
    }

    /// Movement speed multiplier on this road type
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            RoadType::Trail => 1.2,
            RoadType::Road => 1.5,
            RoadType::PavedRoad => 2.0,
        }
    }

    /// Decay rate per tick
    pub fn decay_rate(&self) -> f32 {
        match self {
            RoadType::Trail => 0.01,
            RoadType::Road => 0.005,
            RoadType::PavedRoad => 0.002,
        }
    }
}

/// Network of all roads in the world
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoadNetwork {
    /// Road segments indexed by starting tile
    pub segments: HashMap<TileCoord, Vec<RoadSegment>>,
    /// Set of all tiles that have roads
    pub road_tiles: HashSet<TileCoord>,
}

impl RoadNetwork {
    pub fn new() -> Self {
        RoadNetwork {
            segments: HashMap::new(),
            road_tiles: HashSet::new(),
        }
    }

    /// Check if there's a road at a coordinate
    pub fn has_road(&self, coord: &TileCoord) -> bool {
        self.road_tiles.contains(coord)
    }

    /// Get the road character for a tile based on connections
    pub fn get_road_char(&self, coord: &TileCoord) -> char {
        if let Some(segments) = self.segments.get(coord) {
            if segments.is_empty() {
                return '.';
            }

            // Find the best road type at this tile
            let best_type = segments.iter()
                .map(|s| s.road_type)
                .max_by_key(|t| match t {
                    RoadType::PavedRoad => 2,
                    RoadType::Road => 1,
                    RoadType::Trail => 0,
                })
                .unwrap_or(RoadType::Trail);

            // Determine direction based on connections
            let has_north = segments.iter().any(|s| s.to.y < coord.y || (s.from != *coord && s.from.y < coord.y));
            let has_south = segments.iter().any(|s| s.to.y > coord.y || (s.from != *coord && s.from.y > coord.y));
            let has_east = segments.iter().any(|s| s.to.x > coord.x || (s.from != *coord && s.from.x > coord.x));
            let has_west = segments.iter().any(|s| s.to.x < coord.x || (s.from != *coord && s.from.x < coord.x));

            // Choose character based on connections
            let is_intersection = (has_north || has_south) && (has_east || has_west);

            if is_intersection {
                '+'
            } else if has_north || has_south {
                '|'
            } else if has_east || has_west {
                best_type.map_char()
            } else {
                best_type.map_char()
            }
        } else {
            '.'
        }
    }

    /// Get the road color for a tile
    pub fn get_road_color(&self, coord: &TileCoord) -> (u8, u8, u8) {
        if let Some(segments) = self.segments.get(coord) {
            if let Some(seg) = segments.first() {
                return seg.road_type.color();
            }
        }
        RoadType::Trail.color()
    }

    /// Add a road segment
    pub fn add_segment(&mut self, segment: RoadSegment) {
        self.road_tiles.insert(segment.from);
        self.road_tiles.insert(segment.to);

        self.segments.entry(segment.from).or_default().push(segment.clone());

        // Add reverse segment for bidirectional roads
        let reverse = RoadSegment {
            from: segment.to,
            to: segment.from,
            road_type: segment.road_type,
            owner: segment.owner,
            condition: segment.condition,
        };
        self.segments.entry(segment.to).or_default().push(reverse);
    }

    /// Build a road path between two points using A*
    pub fn build_road(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        road_type: RoadType,
        owner: Option<TribeId>,
        world: &WorldData,
    ) -> bool {
        if let Some(path) = find_path(from, to, world) {
            for window in path.windows(2) {
                let segment = RoadSegment::new(window[0], window[1], road_type, owner);
                self.add_segment(segment);
            }
            true
        } else {
            false
        }
    }

    /// Decay all roads
    pub fn decay_roads(&mut self) {
        for segments in self.segments.values_mut() {
            for segment in segments.iter_mut() {
                segment.decay(segment.road_type.decay_rate());
            }
        }

        // Remove completely decayed roads
        self.segments.retain(|_, segs| {
            segs.retain(|s| s.condition > 0.0);
            !segs.is_empty()
        });

        // Update road tiles
        self.road_tiles.clear();
        for coord in self.segments.keys() {
            self.road_tiles.insert(*coord);
        }
    }

    /// Upgrade roads at a tile to a better type
    pub fn upgrade_road(&mut self, coord: &TileCoord, new_type: RoadType) {
        if let Some(segments) = self.segments.get_mut(coord) {
            for segment in segments.iter_mut() {
                if segment.road_type as u8 <= new_type as u8 {
                    segment.road_type = new_type;
                    segment.condition = 1.0;
                }
            }
        }
    }
}

/// Node for A* pathfinding
#[derive(Clone, Copy, Eq, PartialEq)]
struct PathNode {
    coord: TileCoord,
    cost: u32,
    estimated_total: u32,
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.estimated_total.cmp(&self.estimated_total)
            .then_with(|| other.cost.cmp(&self.cost))
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A* pathfinding to find a path between two points
pub fn find_path(from: TileCoord, to: TileCoord, world: &WorldData) -> Option<Vec<TileCoord>> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut g_score: HashMap<TileCoord, u32> = HashMap::new();

    let h = |coord: TileCoord| -> u32 {
        coord.distance_wrapped(&to, world.width) as u32
    };

    g_score.insert(from, 0);
    open_set.push(PathNode {
        coord: from,
        cost: 0,
        estimated_total: h(from),
    });

    let max_iterations = 10000;
    let mut iterations = 0;

    while let Some(current) = open_set.pop() {
        iterations += 1;
        if iterations > max_iterations {
            return None; // Prevent infinite loops
        }

        if current.coord == to {
            // Reconstruct path
            let mut path = vec![to];
            let mut current_coord = to;
            while let Some(&prev) = came_from.get(&current_coord) {
                path.push(prev);
                current_coord = prev;
            }
            path.reverse();
            return Some(path);
        }

        // Get neighbors (4-directional)
        let neighbors = get_neighbors(current.coord, world);

        for neighbor in neighbors {
            let move_cost = get_move_cost(neighbor, world);
            if move_cost == u32::MAX {
                continue; // Impassable
            }

            let tentative_g = g_score.get(&current.coord).unwrap_or(&u32::MAX)
                .saturating_add(move_cost);

            if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                came_from.insert(neighbor, current.coord);
                g_score.insert(neighbor, tentative_g);
                open_set.push(PathNode {
                    coord: neighbor,
                    cost: tentative_g,
                    estimated_total: tentative_g + h(neighbor),
                });
            }
        }
    }

    None // No path found
}

/// Get valid neighboring tiles
fn get_neighbors(coord: TileCoord, world: &WorldData) -> Vec<TileCoord> {
    let mut neighbors = Vec::with_capacity(4);

    // North
    if coord.y > 0 {
        neighbors.push(TileCoord::new(coord.x, coord.y - 1));
    }
    // South
    if coord.y < world.height - 1 {
        neighbors.push(TileCoord::new(coord.x, coord.y + 1));
    }
    // East (with wrapping)
    neighbors.push(TileCoord::new((coord.x + 1) % world.width, coord.y));
    // West (with wrapping)
    neighbors.push(TileCoord::new(
        (coord.x as i32 - 1).rem_euclid(world.width as i32) as usize,
        coord.y,
    ));

    neighbors
}

/// Get movement cost for a tile based on terrain
fn get_move_cost(coord: TileCoord, world: &WorldData) -> u32 {
    let elevation = *world.heightmap.get(coord.x, coord.y);
    let biome = *world.biomes.get(coord.x, coord.y);

    // Water is impassable for roads
    if elevation < 0.0 {
        return u32::MAX;
    }

    // Base cost depends on biome
    let base_cost: u32 = match biome {
        ExtendedBiome::Desert | ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna => 10,
        ExtendedBiome::TemperateForest | ExtendedBiome::TropicalForest | ExtendedBiome::TemperateRainforest => 15,
        ExtendedBiome::TropicalRainforest | ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => 25,
        ExtendedBiome::Tundra | ExtendedBiome::BorealForest | ExtendedBiome::AlpineTundra => 20,
        ExtendedBiome::Foothills | ExtendedBiome::RazorPeaks => 30,
        ExtendedBiome::SnowyPeaks => 50,
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaLake => u32::MAX,
        ExtendedBiome::Ice | ExtendedBiome::FrozenLake => 40,
        _ => 15,
    };

    // Increase cost for steep terrain
    let slope_penalty = ((elevation.abs() * 20.0) as u32).min(30);

    base_cost.saturating_add(slope_penalty)
}
