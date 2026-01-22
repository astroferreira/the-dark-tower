//! Structure types and definitions
//!
//! Defines the various types of human-made structures (castles, cities, villages, etc.)
//! and the Prefab format for pre-defined building templates.

use crate::zlevel::ZTile;

/// Type of structure
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureType {
    /// Large fortress on hilltops (30-60 tiles diameter)
    Castle,
    /// City/town ruins in flat fertile areas (50-100 tiles)
    City,
    /// Small settlement near resources (15-30 tiles)
    Village,
    /// Underground dwelling inside caves (10-30 tiles)
    CaveDwelling,
    /// Multi-level underground maze (20-50 tiles per level)
    Dungeon,
}

impl StructureType {
    /// Get the size range for this structure type (min, max tiles)
    pub fn size_range(&self) -> (usize, usize) {
        match self {
            StructureType::Castle => (30, 60),
            StructureType::City => (50, 100),
            StructureType::Village => (15, 30),
            StructureType::CaveDwelling => (10, 30),
            StructureType::Dungeon => (20, 50),
        }
    }

    /// Get the typical number of structures of this type to place
    pub fn typical_count(&self) -> (usize, usize) {
        match self {
            StructureType::Castle => (3, 5),
            StructureType::City => (2, 4),
            StructureType::Village => (5, 10),
            StructureType::CaveDwelling => (3, 8),
            StructureType::Dungeon => (2, 4),
        }
    }

    /// Get the decay percentage for this structure type (how ruined it is)
    pub fn decay_percentage(&self) -> f32 {
        match self {
            StructureType::Castle => 0.70,
            StructureType::City => 0.80,
            StructureType::Village => 0.60,
            StructureType::CaveDwelling => 0.30,
            StructureType::Dungeon => 0.50,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            StructureType::Castle => "Castle",
            StructureType::City => "City",
            StructureType::Village => "Village",
            StructureType::CaveDwelling => "Cave Dwelling",
            StructureType::Dungeon => "Dungeon",
        }
    }
}

/// A pre-defined building template (small structure)
#[derive(Clone, Debug)]
pub struct Prefab {
    pub name: &'static str,
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Vec<ZTile>>,
    pub tags: Vec<&'static str>,
}

impl Prefab {
    /// Create a new prefab from a tile grid
    pub fn new(
        name: &'static str,
        tiles: Vec<Vec<ZTile>>,
        tags: Vec<&'static str>,
    ) -> Self {
        let height = tiles.len();
        let width = if height > 0 { tiles[0].len() } else { 0 };
        Self {
            name,
            width,
            height,
            tiles,
            tags,
        }
    }

    /// Get a tile at local coordinates (returns None if out of bounds)
    pub fn get(&self, x: usize, y: usize) -> Option<ZTile> {
        self.tiles.get(y).and_then(|row| row.get(x)).copied()
    }

    /// Check if this prefab has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| *t == tag)
    }

    /// Rotate the prefab 90 degrees clockwise
    pub fn rotate_cw(&self) -> Self {
        let new_width = self.height;
        let new_height = self.width;
        let mut new_tiles = vec![vec![ZTile::Air; new_width]; new_height];

        for y in 0..self.height {
            for x in 0..self.width {
                new_tiles[x][self.height - 1 - y] = self.tiles[y][x];
            }
        }

        Self {
            name: self.name,
            width: new_width,
            height: new_height,
            tiles: new_tiles,
            tags: self.tags.clone(),
        }
    }

    /// Flip the prefab horizontally
    pub fn flip_h(&self) -> Self {
        let new_tiles: Vec<Vec<ZTile>> = self.tiles
            .iter()
            .map(|row| row.iter().rev().copied().collect())
            .collect();

        Self {
            name: self.name,
            width: self.width,
            height: self.height,
            tiles: new_tiles,
            tags: self.tags.clone(),
        }
    }
}

/// Unique identifier for a structure (mirrored from join_system for use without circular deps)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructureId(pub u32);

/// A join point position for road/connection endpoints
/// Simplified version that stores world coordinates directly
#[derive(Clone, Debug)]
pub struct JoinPointRef {
    /// World X coordinate
    pub world_x: usize,
    /// World Y coordinate
    pub world_y: usize,
    /// Whether this join point is connected
    pub connected: bool,
}

/// A placed structure in the world
#[derive(Clone, Debug)]
pub struct PlacedStructure {
    /// World position (top-left corner)
    pub x: usize,
    pub y: usize,
    /// Z-level of the structure
    pub z: i32,
    /// Bounding box width
    pub width: usize,
    /// Bounding box height
    pub height: usize,
    /// Type of structure
    pub structure_type: StructureType,
    /// Optional name
    pub name: Option<String>,
    /// Join points for road/connection endpoints (Phase 3 integration)
    pub join_points: Vec<JoinPointRef>,
    /// IDs of structures this one is connected to
    pub connections: Vec<StructureId>,
}

impl PlacedStructure {
    pub fn new(
        x: usize,
        y: usize,
        z: i32,
        width: usize,
        height: usize,
        structure_type: StructureType,
    ) -> Self {
        // Generate default join points based on structure type
        let join_points = Self::generate_default_join_points(x, y, width, height, structure_type);

        Self {
            x,
            y,
            z,
            width,
            height,
            structure_type,
            name: None,
            join_points,
            connections: Vec::new(),
        }
    }

    /// Generate default join points based on structure type
    fn generate_default_join_points(
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        structure_type: StructureType,
    ) -> Vec<JoinPointRef> {
        let hw = width / 2;
        let hh = height / 2;

        match structure_type {
            StructureType::Castle => {
                // Castle has gates on all sides
                vec![
                    JoinPointRef { world_x: x + hw, world_y: y, connected: false }, // North gate
                    JoinPointRef { world_x: x + hw, world_y: y + height, connected: false }, // South gate
                    JoinPointRef { world_x: x, world_y: y + hh, connected: false }, // West gate
                    JoinPointRef { world_x: x + width, world_y: y + hh, connected: false }, // East gate
                ]
            }
            StructureType::City => {
                // City has road connections on all sides
                vec![
                    JoinPointRef { world_x: x + hw, world_y: y, connected: false },
                    JoinPointRef { world_x: x + hw, world_y: y + height, connected: false },
                    JoinPointRef { world_x: x, world_y: y + hh, connected: false },
                    JoinPointRef { world_x: x + width, world_y: y + hh, connected: false },
                ]
            }
            StructureType::Village => {
                // Village has fewer connections
                vec![
                    JoinPointRef { world_x: x + hw, world_y: y, connected: false },
                    JoinPointRef { world_x: x + hw, world_y: y + height, connected: false },
                ]
            }
            StructureType::CaveDwelling | StructureType::Dungeon => {
                // Underground structures have entrances
                vec![
                    JoinPointRef { world_x: x + hw, world_y: y + height, connected: false },
                ]
            }
        }
    }

    /// Get the center position
    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Check if a point is within this structure's bounds
    pub fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x && px < self.x + self.width &&
        py >= self.y && py < self.y + self.height
    }

    /// Get distance to another structure's center
    pub fn distance_to(&self, other: &PlacedStructure) -> f32 {
        let (cx, cy) = self.center();
        let (ox, oy) = other.center();
        let dx = cx as f32 - ox as f32;
        let dy = cy as f32 - oy as f32;
        (dx * dx + dy * dy).sqrt()
    }
}

/// A BSP node for room generation
#[derive(Clone, Debug)]
pub struct BspNode {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub left: Option<Box<BspNode>>,
    pub right: Option<Box<BspNode>>,
    /// Room within this leaf node (if any)
    pub room: Option<Room>,
}

impl BspNode {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
            left: None,
            right: None,
            room: None,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }

    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// Shape type for rooms
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomShape {
    /// Standard rectangular room
    Rectangular,
    /// Rounded rectangle with curved corners
    Rounded,
    /// Circular room
    Circular,
    /// L-shaped room
    LShape,
    /// Irregular organic blob
    Organic,
}

/// A room generated by BSP
#[derive(Clone, Debug)]
pub struct Room {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    /// Shape of the room (default: Rectangular)
    pub shape: RoomShape,
    /// Corner for L-shaped rooms (0=TL, 1=TR, 2=BL, 3=BR)
    pub corner: u8,
}

impl Room {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height, shape: RoomShape::Rectangular, corner: 0 }
    }

    pub fn with_shape(x: usize, y: usize, width: usize, height: usize, shape: RoomShape) -> Self {
        Self { x, y, width, height, shape, corner: 0 }
    }

    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    pub fn area(&self) -> usize {
        self.width * self.height
    }
}

/// L-system rule for procedural wall generation
#[derive(Clone, Debug)]
pub struct LSystemRule {
    pub symbol: char,
    pub replacement: &'static str,
}

/// L-system configuration for castle walls
#[derive(Clone, Debug)]
pub struct LSystemConfig {
    pub axiom: &'static str,
    pub rules: Vec<LSystemRule>,
    pub iterations: usize,
    pub angle: f32, // Turn angle in degrees
    pub segment_length: usize,
}

impl Default for LSystemConfig {
    fn default() -> Self {
        Self {
            axiom: "F",
            rules: vec![
                LSystemRule { symbol: 'F', replacement: "F+F-F-F+F" },
            ],
            iterations: 2,
            angle: 90.0,
            segment_length: 3,
        }
    }
}

/// Road segment connecting two points
#[derive(Clone, Debug)]
pub struct RoadSegment {
    pub start: (usize, usize),
    pub end: (usize, usize),
    pub road_type: RoadType,
    /// Path of tiles for this segment
    pub path: Vec<(usize, usize)>,
}

/// Type of road
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadType {
    /// Major road connecting cities/castles
    Main,
    /// Secondary road to villages
    Secondary,
    /// Small path
    Path,
}

impl RoadType {
    pub fn to_tile(&self) -> ZTile {
        match self {
            RoadType::Main => ZTile::StoneRoad,
            RoadType::Secondary => ZTile::DirtRoad,
            RoadType::Path => ZTile::DirtRoad,
        }
    }

    pub fn width(&self) -> usize {
        match self {
            RoadType::Main => 2,
            RoadType::Secondary => 1,
            RoadType::Path => 1,
        }
    }
}

/// Desirability map for structure placement
#[derive(Clone)]
pub struct DesirabilityMap {
    pub width: usize,
    pub height: usize,
    pub scores: Vec<f32>,
}

impl DesirabilityMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            scores: vec![0.0; width * height],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.scores[y * self.width + x]
        } else {
            f32::MIN
        }
    }

    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.scores[y * self.width + x] = value;
        }
    }

    pub fn add(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.scores[y * self.width + x] += value;
        }
    }

    /// Find the position with highest desirability
    pub fn find_best(&self) -> Option<(usize, usize, f32)> {
        let mut best_score = f32::MIN;
        let mut best_pos = None;

        for y in 0..self.height {
            for x in 0..self.width {
                let score = self.get(x, y);
                if score > best_score {
                    best_score = score;
                    best_pos = Some((x, y, score));
                }
            }
        }

        best_pos
    }

    /// Find top N positions with highest desirability
    pub fn find_top_n(&self, n: usize) -> Vec<(usize, usize, f32)> {
        let mut positions: Vec<(usize, usize, f32)> = Vec::new();

        for y in 0..self.height {
            for x in 0..self.width {
                let score = self.get(x, y);
                positions.push((x, y, score));
            }
        }

        positions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        positions.truncate(n);
        positions
    }

    /// Mark an area as used (set to very low desirability)
    pub fn mark_used(&mut self, x: usize, y: usize, radius: usize) {
        let r = radius as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let nx = (x as i32 + dx).rem_euclid(self.width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, self.height as i32 - 1) as usize;
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist <= radius as f32 {
                    self.set(nx, ny, f32::MIN);
                }
            }
        }
    }
}
