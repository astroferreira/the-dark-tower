//! Join-Based Structures System (Phase 4a)
//!
//! Structures connect via typed join points with priority-based placement,
//! creating coherent settlements and road networks.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::types::{StructureType, PlacedStructure};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Unique identifier for a structure
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructureId(pub u32);

impl StructureId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Cardinal and ordinal directions
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Direction {
    /// Get the opposite direction
    pub fn opposite(&self) -> Self {
        match self {
            Direction::North => Direction::South,
            Direction::NorthEast => Direction::SouthWest,
            Direction::East => Direction::West,
            Direction::SouthEast => Direction::NorthWest,
            Direction::South => Direction::North,
            Direction::SouthWest => Direction::NorthEast,
            Direction::West => Direction::East,
            Direction::NorthWest => Direction::SouthEast,
        }
    }

    /// Get direction offset (dx, dy)
    pub fn offset(&self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::NorthEast => (1, -1),
            Direction::East => (1, 0),
            Direction::SouthEast => (1, 1),
            Direction::South => (0, 1),
            Direction::SouthWest => (-1, 1),
            Direction::West => (-1, 0),
            Direction::NorthWest => (-1, -1),
        }
    }

    /// Get all cardinal directions
    pub fn cardinals() -> [Direction; 4] {
        [Direction::North, Direction::East, Direction::South, Direction::West]
    }

    /// Get all directions
    pub fn all() -> [Direction; 8] {
        [
            Direction::North, Direction::NorthEast, Direction::East, Direction::SouthEast,
            Direction::South, Direction::SouthWest, Direction::West, Direction::NorthWest,
        ]
    }
}

/// Type of join point connection
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoinType {
    /// Door connection (building entrances)
    Door,
    /// Gate connection (wall openings, castle gates)
    Gate,
    /// Road connection (paths between structures)
    Road,
    /// Bridge connection (over water)
    Bridge,
    /// Tunnel connection (underground)
    Tunnel,
    /// Stairs connection (between levels)
    Stairs,
}

impl JoinType {
    /// Check if two join types are compatible for connection
    pub fn is_compatible_with(&self, other: &JoinType) -> bool {
        match (self, other) {
            // Doors connect to doors, gates, or roads
            (JoinType::Door, JoinType::Door) => true,
            (JoinType::Door, JoinType::Gate) => true,
            (JoinType::Door, JoinType::Road) => true,

            // Gates connect to gates, roads, or doors
            (JoinType::Gate, JoinType::Gate) => true,
            (JoinType::Gate, JoinType::Road) => true,
            (JoinType::Gate, JoinType::Door) => true,

            // Roads connect to roads, doors, gates, or bridges
            (JoinType::Road, JoinType::Road) => true,
            (JoinType::Road, JoinType::Door) => true,
            (JoinType::Road, JoinType::Gate) => true,
            (JoinType::Road, JoinType::Bridge) => true,

            // Bridges connect to roads or other bridges
            (JoinType::Bridge, JoinType::Road) => true,
            (JoinType::Bridge, JoinType::Bridge) => true,

            // Tunnels connect to tunnels or stairs
            (JoinType::Tunnel, JoinType::Tunnel) => true,
            (JoinType::Tunnel, JoinType::Stairs) => true,

            // Stairs connect to stairs or tunnels
            (JoinType::Stairs, JoinType::Stairs) => true,
            (JoinType::Stairs, JoinType::Tunnel) => true,

            _ => false,
        }
    }

    /// Get maximum connection distance for this type
    pub fn max_distance(&self) -> f32 {
        match self {
            JoinType::Door => 5.0,
            JoinType::Gate => 10.0,
            JoinType::Road => 20.0,
            JoinType::Bridge => 15.0,
            JoinType::Tunnel => 30.0,
            JoinType::Stairs => 3.0,
        }
    }
}

/// A join point on a structure
#[derive(Clone, Debug)]
pub struct JoinPoint {
    /// Local X coordinate within structure bounds
    pub local_x: i32,
    /// Local Y coordinate within structure bounds
    pub local_y: i32,
    /// Direction the join faces
    pub direction: Direction,
    /// Type of join
    pub join_type: JoinType,
    /// Priority for connection (higher = connect first)
    pub priority: u8,
    /// ID of connected structure (if any)
    pub connected_to: Option<StructureId>,
}

impl JoinPoint {
    /// Create a new join point
    pub fn new(local_x: i32, local_y: i32, direction: Direction, join_type: JoinType) -> Self {
        Self {
            local_x,
            local_y,
            direction,
            join_type,
            priority: 50, // Default medium priority
            connected_to: None,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Get world position given structure position
    pub fn world_position(&self, structure_x: usize, structure_y: usize) -> (usize, usize) {
        let x = (structure_x as i32 + self.local_x).max(0) as usize;
        let y = (structure_y as i32 + self.local_y).max(0) as usize;
        (x, y)
    }

    /// Check if this join point can connect to another
    pub fn can_connect_to(&self, other: &JoinPoint, distance: f32) -> bool {
        // Must face opposite directions
        if self.direction.opposite() != other.direction {
            return false;
        }

        // Types must be compatible
        if !self.join_type.is_compatible_with(&other.join_type) {
            return false;
        }

        // Must be within range
        if distance > self.join_type.max_distance() || distance > other.join_type.max_distance() {
            return false;
        }

        true
    }
}

/// A structure with join points for the join system
#[derive(Clone, Debug)]
pub struct MutableStructure {
    /// Unique ID
    pub id: StructureId,
    /// Type of structure
    pub structure_type: StructureType,
    /// World position (top-left corner)
    pub x: usize,
    pub y: usize,
    /// Z-level
    pub z: i32,
    /// Bounding box size
    pub width: usize,
    pub height: usize,
    /// Join points for connections
    pub join_points: Vec<JoinPoint>,
    /// Structures this one is connected to
    pub connections: Vec<StructureId>,
}

impl MutableStructure {
    /// Create from a PlacedStructure
    pub fn from_placed(placed: &PlacedStructure, id: StructureId) -> Self {
        Self {
            id,
            structure_type: placed.structure_type,
            x: placed.x,
            y: placed.y,
            z: placed.z,
            width: placed.width,
            height: placed.height,
            join_points: generate_default_join_points(placed.structure_type, placed.width, placed.height),
            connections: Vec::new(),
        }
    }

    /// Get center position
    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Get distance to another structure
    pub fn distance_to(&self, other: &MutableStructure) -> f32 {
        let (cx, cy) = self.center();
        let (ox, oy) = other.center();
        let dx = cx as f32 - ox as f32;
        let dy = cy as f32 - oy as f32;
        (dx * dx + dy * dy).sqrt()
    }

    /// Find best unconnected join point for connecting to another structure
    pub fn find_best_join_for(&self, other: &MutableStructure) -> Option<(usize, f32)> {
        let (ox, oy) = other.center();

        let mut best: Option<(usize, f32)> = None;

        for (i, jp) in self.join_points.iter().enumerate() {
            if jp.connected_to.is_some() {
                continue;
            }

            let (wx, wy) = jp.world_position(self.x, self.y);
            let dx = ox as f32 - wx as f32;
            let dy = oy as f32 - wy as f32;
            let dist = (dx * dx + dy * dy).sqrt();

            match best {
                None => best = Some((i, dist)),
                Some((_, best_dist)) if dist < best_dist => best = Some((i, dist)),
                _ => {}
            }
        }

        best
    }
}

/// Generate default join points for a structure type
fn generate_default_join_points(structure_type: StructureType, width: usize, height: usize) -> Vec<JoinPoint> {
    let mut points = Vec::new();
    let hw = width as i32 / 2;
    let hh = height as i32 / 2;

    match structure_type {
        StructureType::Castle => {
            // Castle has a main gate (high priority) and secondary gates
            points.push(JoinPoint::new(hw, 0, Direction::North, JoinType::Gate).with_priority(100));
            points.push(JoinPoint::new(hw, height as i32, Direction::South, JoinType::Gate).with_priority(60));
            points.push(JoinPoint::new(0, hh, Direction::West, JoinType::Gate).with_priority(40));
            points.push(JoinPoint::new(width as i32, hh, Direction::East, JoinType::Gate).with_priority(40));
        }
        StructureType::City => {
            // City has multiple road connections
            for dir in Direction::cardinals() {
                let (dx, dy) = dir.offset();
                let x = hw + dx * hw;
                let y = hh + dy * hh;
                points.push(JoinPoint::new(x, y, dir, JoinType::Road).with_priority(80));
            }
        }
        StructureType::Village => {
            // Village has paths
            points.push(JoinPoint::new(hw, 0, Direction::North, JoinType::Road).with_priority(70));
            points.push(JoinPoint::new(hw, height as i32, Direction::South, JoinType::Road).with_priority(70));
            // Door entrances
            points.push(JoinPoint::new(0, hh, Direction::West, JoinType::Door).with_priority(50));
            points.push(JoinPoint::new(width as i32, hh, Direction::East, JoinType::Door).with_priority(50));
        }
        StructureType::CaveDwelling => {
            // Cave has tunnel entrances
            points.push(JoinPoint::new(hw, height as i32, Direction::South, JoinType::Tunnel).with_priority(60));
            points.push(JoinPoint::new(hw, 0, Direction::North, JoinType::Stairs).with_priority(40));
        }
        StructureType::Dungeon => {
            // Dungeon has stairs and tunnels
            points.push(JoinPoint::new(hw, 0, Direction::North, JoinType::Stairs).with_priority(80));
            points.push(JoinPoint::new(hw, height as i32, Direction::South, JoinType::Tunnel).with_priority(50));
            points.push(JoinPoint::new(0, hh, Direction::West, JoinType::Tunnel).with_priority(50));
            points.push(JoinPoint::new(width as i32, hh, Direction::East, JoinType::Tunnel).with_priority(50));
        }
    }

    points
}

// =============================================================================
// PLACEMENT QUEUE
// =============================================================================

/// A candidate for structure placement
#[derive(Clone, Debug)]
pub struct PlacementCandidate {
    /// Position for placement
    pub x: usize,
    pub y: usize,
    /// Structure type to place
    pub structure_type: StructureType,
    /// Priority score (higher = place first)
    pub priority: f32,
    /// Structure ID that spawned this candidate (for connectivity boost)
    pub spawned_by: Option<StructureId>,
}

impl PartialEq for PlacementCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PlacementCandidate {}

impl PartialOrd for PlacementCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.priority.partial_cmp(&other.priority)
    }
}

impl Ord for PlacementCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Queue for managing structure placement
#[derive(Debug)]
pub struct StructurePlacementQueue {
    /// Priority queue of placement candidates
    pub queue: BinaryHeap<PlacementCandidate>,
    /// Placed structures
    pub placed: Vec<MutableStructure>,
    /// Connection graph (structure ID -> connected structure IDs)
    pub connection_graph: HashMap<StructureId, Vec<StructureId>>,
    /// Used positions (for collision detection)
    used_positions: HashSet<(usize, usize)>,
    /// Next structure ID
    next_id: u32,
}

impl StructurePlacementQueue {
    /// Create a new empty queue
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            placed: Vec::new(),
            connection_graph: HashMap::new(),
            used_positions: HashSet::new(),
            next_id: 0,
        }
    }

    /// Add a placement candidate
    pub fn add_candidate(&mut self, candidate: PlacementCandidate) {
        self.queue.push(candidate);
    }

    /// Get next structure ID
    fn next_structure_id(&mut self) -> StructureId {
        let id = StructureId::new(self.next_id);
        self.next_id += 1;
        id
    }

    /// Check if a position is available
    pub fn is_position_available(&self, x: usize, y: usize, width: usize, height: usize) -> bool {
        for dy in 0..height {
            for dx in 0..width {
                if self.used_positions.contains(&(x + dx, y + dy)) {
                    return false;
                }
            }
        }
        true
    }

    /// Mark a position as used
    fn mark_position_used(&mut self, x: usize, y: usize, width: usize, height: usize) {
        for dy in 0..height {
            for dx in 0..width {
                self.used_positions.insert((x + dx, y + dy));
            }
        }
    }

    /// Place a structure
    pub fn place_structure(
        &mut self,
        x: usize,
        y: usize,
        z: i32,
        structure_type: StructureType,
        spawned_by: Option<StructureId>,
    ) -> Option<StructureId> {
        let (min_size, max_size) = structure_type.size_range();
        let size = (min_size + max_size) / 2; // Use average size

        if !self.is_position_available(x, y, size, size) {
            return None;
        }

        let id = self.next_structure_id();

        // Create placed structure
        let placed = PlacedStructure::new(x, y, z, size, size, structure_type);
        let mut mutable = MutableStructure::from_placed(&placed, id);

        // Try to connect to spawning structure
        if let Some(spawner_id) = spawned_by {
            if let Some(spawner) = self.placed.iter_mut().find(|s| s.id == spawner_id) {
                // Find compatible join points and connect
                if let Some((spawner_jp_idx, _)) = spawner.find_best_join_for(&mutable) {
                    if let Some((new_jp_idx, _)) = mutable.find_best_join_for(spawner) {
                        // Connect the structures
                        spawner.join_points[spawner_jp_idx].connected_to = Some(id);
                        mutable.join_points[new_jp_idx].connected_to = Some(spawner_id);
                        spawner.connections.push(id);
                        mutable.connections.push(spawner_id);

                        self.connection_graph.entry(spawner_id).or_default().push(id);
                        self.connection_graph.entry(id).or_default().push(spawner_id);
                    }
                }
            }
        }

        self.mark_position_used(x, y, size, size);
        self.placed.push(mutable);

        Some(id)
    }

    /// Process queue until empty or max structures reached
    pub fn process(
        &mut self,
        max_structures: usize,
        get_z: impl Fn(usize, usize) -> i32,
        rng: &mut ChaCha8Rng,
    ) {
        while self.placed.len() < max_structures {
            let candidate = match self.queue.pop() {
                Some(c) => c,
                None => break,
            };

            // Try to place
            let z = get_z(candidate.x, candidate.y);
            let id = self.place_structure(
                candidate.x,
                candidate.y,
                z,
                candidate.structure_type,
                candidate.spawned_by,
            );

            // If placed, generate new candidates nearby
            if let Some(structure_id) = id {
                self.generate_nearby_candidates(structure_id, rng);
            }
        }
    }

    /// Generate placement candidates near a placed structure
    fn generate_nearby_candidates(&mut self, structure_id: StructureId, rng: &mut ChaCha8Rng) {
        let structure = match self.placed.iter().find(|s| s.id == structure_id) {
            Some(s) => s.clone(),
            None => return,
        };

        // For each unconnected join point, generate a candidate
        for jp in &structure.join_points {
            if jp.connected_to.is_some() {
                continue;
            }

            let (dx, dy) = jp.direction.offset();
            let dist = jp.join_type.max_distance() as i32;

            // Position for new structure
            let new_x = (structure.x as i32 + dx * dist + rng.gen_range(-3..=3)).max(0) as usize;
            let new_y = (structure.y as i32 + dy * dist + rng.gen_range(-3..=3)).max(0) as usize;

            // Determine what type of structure to place
            let new_type = match jp.join_type {
                JoinType::Road | JoinType::Gate => {
                    if rng.gen_bool(0.3) { StructureType::Village } else { continue }
                }
                JoinType::Door => {
                    if rng.gen_bool(0.2) { StructureType::Village } else { continue }
                }
                JoinType::Tunnel | JoinType::Stairs => {
                    if rng.gen_bool(0.1) { StructureType::CaveDwelling } else { continue }
                }
                _ => continue,
            };

            // Priority boost for being connected to existing structure
            let priority = jp.priority as f32 * 1.5 + rng.gen::<f32>() * 10.0;

            self.add_candidate(PlacementCandidate {
                x: new_x,
                y: new_y,
                structure_type: new_type,
                priority,
                spawned_by: Some(structure_id),
            });
        }
    }

    /// Convert to PlacedStructure list
    pub fn to_placed_structures(&self) -> Vec<PlacedStructure> {
        self.placed.iter().map(|ms| {
            PlacedStructure::new(
                ms.x,
                ms.y,
                ms.z,
                ms.width,
                ms.height,
                ms.structure_type,
            )
        }).collect()
    }
}

// =============================================================================
// CONNECTION GENERATION
// =============================================================================

/// Connection between two structures
#[derive(Clone, Debug)]
pub struct StructureConnection {
    /// Source structure
    pub from_id: StructureId,
    /// Destination structure
    pub to_id: StructureId,
    /// Path points (world coordinates)
    pub path: Vec<(usize, usize)>,
    /// Type of connection (based on join types)
    pub connection_type: JoinType,
}

/// Generate connections between placed structures
pub fn generate_connections(
    queue: &mut StructurePlacementQueue,
    max_distance: f32,
) -> Vec<StructureConnection> {
    let mut connections = Vec::new();

    // For each structure, try to connect to nearby structures
    let structures: Vec<MutableStructure> = queue.placed.clone();

    for i in 0..structures.len() {
        for j in (i + 1)..structures.len() {
            let s1 = &structures[i];
            let s2 = &structures[j];

            // Skip if already connected
            if s1.connections.contains(&s2.id) {
                continue;
            }

            let dist = s1.distance_to(s2);
            if dist > max_distance {
                continue;
            }

            // Find best join points to connect
            if let (Some((jp1_idx, _)), Some((jp2_idx, _))) = (
                s1.find_best_join_for(s2),
                s2.find_best_join_for(s1),
            ) {
                let jp1 = &s1.join_points[jp1_idx];
                let jp2 = &s2.join_points[jp2_idx];

                if !jp1.join_type.is_compatible_with(&jp2.join_type) {
                    continue;
                }

                // Generate path between join points
                let (x1, y1) = jp1.world_position(s1.x, s1.y);
                let (x2, y2) = jp2.world_position(s2.x, s2.y);
                let path = generate_path(x1, y1, x2, y2);

                // Determine connection type
                let connection_type = if jp1.join_type == JoinType::Road || jp2.join_type == JoinType::Road {
                    JoinType::Road
                } else if jp1.join_type == JoinType::Bridge || jp2.join_type == JoinType::Bridge {
                    JoinType::Bridge
                } else {
                    jp1.join_type
                };

                connections.push(StructureConnection {
                    from_id: s1.id,
                    to_id: s2.id,
                    path,
                    connection_type,
                });

                // Update the structures in the queue
                if let Some(ms1) = queue.placed.iter_mut().find(|s| s.id == s1.id) {
                    ms1.join_points[jp1_idx].connected_to = Some(s2.id);
                    ms1.connections.push(s2.id);
                }
                if let Some(ms2) = queue.placed.iter_mut().find(|s| s.id == s2.id) {
                    ms2.join_points[jp2_idx].connected_to = Some(s1.id);
                    ms2.connections.push(s1.id);
                }

                queue.connection_graph.entry(s1.id).or_default().push(s2.id);
                queue.connection_graph.entry(s2.id).or_default().push(s1.id);
            }
        }
    }

    connections
}

/// Generate a simple path between two points
fn generate_path(x1: usize, y1: usize, x2: usize, y2: usize) -> Vec<(usize, usize)> {
    let mut path = Vec::new();
    let mut x = x1 as i32;
    let mut y = y1 as i32;
    let dx = (x2 as i32 - x1 as i32).signum();
    let dy = (y2 as i32 - y1 as i32).signum();

    // Simple L-shaped path
    while x != x2 as i32 {
        path.push((x as usize, y as usize));
        x += dx;
    }
    while y != y2 as i32 {
        path.push((x as usize, y as usize));
        y += dy;
    }
    path.push((x2, y2));

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_opposite() {
        assert_eq!(Direction::North.opposite(), Direction::South);
        assert_eq!(Direction::East.opposite(), Direction::West);
        assert_eq!(Direction::NorthEast.opposite(), Direction::SouthWest);
    }

    #[test]
    fn test_join_type_compatibility() {
        assert!(JoinType::Door.is_compatible_with(&JoinType::Road));
        assert!(JoinType::Gate.is_compatible_with(&JoinType::Road));
        assert!(!JoinType::Door.is_compatible_with(&JoinType::Bridge));
    }

    #[test]
    fn test_structure_placement_queue() {
        let mut queue = StructurePlacementQueue::new();

        queue.add_candidate(PlacementCandidate {
            x: 100,
            y: 100,
            structure_type: StructureType::Village,
            priority: 50.0,
            spawned_by: None,
        });

        assert_eq!(queue.queue.len(), 1);
    }
}
