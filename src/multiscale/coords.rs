//! Coordinate system for multi-scale navigation (Dwarf Fortress style).
//!
//! Two-level system: World (5km/tile) and Local (2m/tile, 48×48 per world tile).
//! Local maps emphasize z-levels for underground depth.

use super::LOCAL_SIZE;
use crate::zlevel;

/// Scale levels for the multi-scale system (simplified: World and Local only)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleLevel {
    /// World scale (~5km/tile) - continents, biomes, factions
    World,
    /// Local scale (~2m/tile) - embark site with full z-level geology
    Local,
}

impl ScaleLevel {
    /// Get human-readable name for the scale level
    pub fn name(&self) -> &'static str {
        match self {
            ScaleLevel::World => "World",
            ScaleLevel::Local => "Local",
        }
    }

    /// Get meters per tile at this scale
    pub fn meters_per_tile(&self) -> f32 {
        match self {
            ScaleLevel::World => super::WORLD_METERS_PER_TILE,
            ScaleLevel::Local => super::LOCAL_METERS_PER_TILE,
        }
    }
}

/// Local coordinate specifying a position within an embark site.
///
/// A LocalCoord uniquely identifies any tile in the local map, including
/// the world tile it belongs to and its z-level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalCoord {
    /// World tile X coordinate (5km scale, embark location)
    pub world_x: usize,
    /// World tile Y coordinate (5km scale, embark location)
    pub world_y: usize,
    /// Local tile X within world tile (0-47, 2m scale)
    pub local_x: u8,
    /// Local tile Y within world tile (0-47, 2m scale)
    pub local_y: u8,
    /// Z-level (can be negative for deep underground)
    pub z: i16,
}

impl LocalCoord {
    /// Create a new local coordinate
    pub fn new(
        world_x: usize,
        world_y: usize,
        local_x: u8,
        local_y: u8,
        z: i16,
    ) -> Self {
        debug_assert!(local_x < LOCAL_SIZE as u8, "local_x out of bounds");
        debug_assert!(local_y < LOCAL_SIZE as u8, "local_y out of bounds");
        debug_assert!(z >= zlevel::MIN_Z as i16 && z <= zlevel::MAX_Z as i16, "z out of bounds");

        Self {
            world_x,
            world_y,
            local_x,
            local_y,
            z,
        }
    }

    /// Create a coordinate at world level only (local at center, z at surface)
    pub fn from_world(world_x: usize, world_y: usize, surface_z: i16) -> Self {
        Self::new(world_x, world_y, LOCAL_SIZE as u8 / 2, LOCAL_SIZE as u8 / 2, surface_z)
    }

    /// Create a coordinate at a specific local position on surface
    pub fn at_surface(world_x: usize, world_y: usize, local_x: u8, local_y: u8, surface_z: i16) -> Self {
        Self::new(world_x, world_y, local_x, local_y, surface_z)
    }

    /// Get the world-level key for chunk caching
    pub fn world_key(&self) -> (usize, usize) {
        (self.world_x, self.world_y)
    }

    /// Convert to absolute local coordinates (ignoring z)
    pub fn to_absolute_local(&self) -> (usize, usize) {
        let abs_x = self.world_x * LOCAL_SIZE + self.local_x as usize;
        let abs_y = self.world_y * LOCAL_SIZE + self.local_y as usize;
        (abs_x, abs_y)
    }

    /// Convert absolute local coordinates to LocalCoord
    pub fn from_absolute_local(abs_local_x: usize, abs_local_y: usize, z: i16) -> Self {
        let world_x = abs_local_x / LOCAL_SIZE;
        let world_y = abs_local_y / LOCAL_SIZE;
        let local_x = (abs_local_x % LOCAL_SIZE) as u8;
        let local_y = (abs_local_y % LOCAL_SIZE) as u8;

        Self::new(world_x, world_y, local_x, local_y, z)
    }

    /// Get physical position in meters from world origin
    pub fn to_meters(&self) -> (f64, f64, f64) {
        let (abs_local_x, abs_local_y) = self.to_absolute_local();
        let x_meters = abs_local_x as f64 * super::LOCAL_METERS_PER_TILE as f64;
        let y_meters = abs_local_y as f64 * super::LOCAL_METERS_PER_TILE as f64;
        let z_meters = self.z as f64 * zlevel::FLOOR_HEIGHT as f64;
        (x_meters, y_meters, z_meters)
    }

    /// Move by delta at local scale, handling overflow to adjacent world tiles
    pub fn offset_local(&self, dx: i32, dy: i32, world_width: usize, world_height: usize) -> Self {
        let (abs_x, abs_y) = self.to_absolute_local();
        let max_x = world_width * LOCAL_SIZE;
        let max_y = world_height * LOCAL_SIZE;

        // Handle wrapping/clamping
        let new_abs_x = ((abs_x as i64 + dx as i64).rem_euclid(max_x as i64)) as usize;
        let new_abs_y = (abs_y as i64 + dy as i64).clamp(0, max_y as i64 - 1) as usize;

        Self::from_absolute_local(new_abs_x, new_abs_y, self.z)
    }

    /// Move z-level, clamping to valid range
    pub fn offset_z(&self, dz: i32) -> Self {
        let new_z = (self.z as i32 + dz).clamp(zlevel::MIN_Z, zlevel::MAX_Z) as i16;
        Self {
            z: new_z,
            ..*self
        }
    }
}

impl std::fmt::Display for LocalCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({},{}):({},{}):Z{}",
            self.world_x, self.world_y,
            self.local_x, self.local_y,
            self.z
        )
    }
}

/// Generate a deterministic seed for procedural generation at a specific coordinate.
///
/// The seed is derived from the world seed combined with the coordinate,
/// ensuring that the same world seed + coordinate always produces identical results.
pub fn local_seed(world_seed: u64, coord: &LocalCoord) -> u64 {
    // Use a simple but effective hash combining
    // Based on splitmix64-style mixing
    let mut hash = world_seed;

    // Mix in world coordinates
    hash = hash.wrapping_add(coord.world_x as u64);
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58476d1ce4e5b9);

    hash = hash.wrapping_add(coord.world_y as u64);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d049bb133111eb);

    // Mix in z-level (important for z-dependent generation)
    hash = hash.wrapping_add(coord.z as u64);
    hash ^= hash >> 31;
    hash = hash.wrapping_mul(0xbf58476d1ce4e5b9);

    // Final mix
    hash ^= hash >> 33;

    hash
}

/// Generate a seed specifically for local chunk generation (at world tile level)
pub fn chunk_seed(world_seed: u64, world_x: usize, world_y: usize) -> u64 {
    let coord = LocalCoord::from_world(world_x, world_y, 0);
    // Use a different offset to differentiate from per-tile seeds
    local_seed(world_seed.wrapping_add(0x7E610741), &coord)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_coord_basic() {
        let coord = LocalCoord::new(5, 10, 24, 24, 0);
        assert_eq!(coord.world_x, 5);
        assert_eq!(coord.world_y, 10);
        assert_eq!(coord.local_x, 24);
        assert_eq!(coord.local_y, 24);
        assert_eq!(coord.z, 0);
    }

    #[test]
    fn test_local_coord_round_trip() {
        let coord = LocalCoord::new(5, 10, 20, 30, -5);

        let (abs_local_x, abs_local_y) = coord.to_absolute_local();
        let recovered = LocalCoord::from_absolute_local(abs_local_x, abs_local_y, -5);

        assert_eq!(coord, recovered);
    }

    #[test]
    fn test_local_seed_determinism() {
        let coord = LocalCoord::new(10, 20, 25, 35, 5);
        let seed = 12345u64;

        let result1 = local_seed(seed, &coord);
        let result2 = local_seed(seed, &coord);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_local_seed_uniqueness() {
        let coord1 = LocalCoord::new(10, 20, 25, 35, 5);
        let coord2 = LocalCoord::new(10, 20, 25, 35, 6); // Different z
        let seed = 12345u64;

        let result1 = local_seed(seed, &coord1);
        let result2 = local_seed(seed, &coord2);

        assert_ne!(result1, result2);
    }

    #[test]
    fn test_offset_local() {
        let coord = LocalCoord::new(0, 0, 24, 24, 0);

        // Move right
        let moved = coord.offset_local(10, 0, 100, 100);
        assert_eq!(moved.local_x, 34);

        // Move across world boundary
        let moved = coord.offset_local(30, 0, 100, 100);
        assert_eq!(moved.world_x, 1);
        assert_eq!(moved.local_x, 6); // 24 + 30 - 48 = 6
    }

    #[test]
    fn test_offset_z() {
        let coord = LocalCoord::new(0, 0, 24, 24, 0);

        let up = coord.offset_z(5);
        assert_eq!(up.z, 5);

        let down = coord.offset_z(-20);
        assert_eq!(down.z, zlevel::MIN_Z as i16); // Clamped

        let way_up = coord.offset_z(100);
        assert_eq!(way_up.z, zlevel::MAX_Z as i16); // Clamped
    }
}
