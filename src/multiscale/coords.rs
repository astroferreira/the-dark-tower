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

// =============================================================================
// SEAMLESS CHUNK BOUNDARY FUNCTIONS
// =============================================================================

/// Calculate world-coordinate for noise sampling that is continuous across chunks.
///
/// This converts a local position within a chunk to absolute world coordinates,
/// then scales it for noise sampling. The result is continuous across chunk boundaries
/// because adjacent chunks use the same coordinate space.
///
/// # Arguments
/// * `world_x` - World tile X coordinate
/// * `world_y` - World tile Y coordinate
/// * `local_x` - Local tile X within chunk (0-47)
/// * `local_y` - Local tile Y within chunk (0-47)
/// * `scale` - Noise scale factor (smaller = larger features)
///
/// # Returns
/// `[f64; 2]` coordinates suitable for noise sampling
pub fn world_noise_coord(
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    scale: f64,
) -> [f64; 2] {
    let abs_x = (world_x * LOCAL_SIZE + local_x) as f64;
    let abs_y = (world_y * LOCAL_SIZE + local_y) as f64;
    [abs_x * scale, abs_y * scale]
}

/// Calculate world-coordinate for 3D noise sampling (includes z-level).
///
/// Same as `world_noise_coord` but for 3D noise patterns like caves.
pub fn world_noise_coord_3d(
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
    z: i16,
    scale_xy: f64,
    scale_z: f64,
) -> [f64; 3] {
    let abs_x = (world_x * LOCAL_SIZE + local_x) as f64;
    let abs_y = (world_y * LOCAL_SIZE + local_y) as f64;
    [abs_x * scale_xy, abs_y * scale_xy, z as f64 * scale_z]
}

/// Generate a deterministic seed for feature placement at an absolute position.
///
/// This ensures that the same world position always generates the same feature
/// decision, regardless of which chunk is generating it. Critical for seamless
/// feature placement at chunk boundaries.
///
/// # Arguments
/// * `world_seed` - The world's base seed
/// * `abs_x` - Absolute local X coordinate (world_x * 48 + local_x)
/// * `abs_y` - Absolute local Y coordinate (world_y * 48 + local_y)
///
/// # Returns
/// A deterministic seed unique to this position
pub fn feature_seed(world_seed: u64, abs_x: usize, abs_y: usize) -> u64 {
    // Use splitmix64-style mixing for good distribution
    let mut hash = world_seed;
    hash ^= (abs_x as u64).wrapping_mul(0x9E3779B97F4A7C15);
    hash ^= (abs_y as u64).wrapping_mul(0xBF58476D1CE4E5B9);
    hash ^= hash >> 33;
    hash.wrapping_mul(0x94D049BB133111EB)
}

/// Check if a feature should be placed at a position based on a deterministic seed.
///
/// Uses position-based hashing instead of RNG to ensure consistency across
/// chunk boundaries. The same position will always make the same decision.
///
/// # Arguments
/// * `seed` - Position-based seed from `feature_seed()`
/// * `density` - Probability of feature placement (0.0 to 1.0)
///
/// # Returns
/// `true` if a feature should be placed at this position
pub fn should_place_feature(seed: u64, density: f32) -> bool {
    // Apply additional mixing for better distribution
    let mixed = seed.wrapping_mul(0x94D049BB133111EB);
    let mixed = mixed ^ (mixed >> 31);
    // Convert to 0.0-1.0 range using full 64-bit precision
    let hash = (mixed as f64) / (u64::MAX as f64);
    hash < density as f64
}

/// Get a deterministic random value (0.0-1.0) for a position.
///
/// Useful for feature variations like tree height that need to be
/// consistent across chunk boundaries.
pub fn position_random(seed: u64, variant: u32) -> f32 {
    // Mix in variant for different random values at same position
    let mixed = seed.wrapping_add(variant as u64).wrapping_mul(0x9E3779B97F4A7C15);
    let mixed = mixed ^ (mixed >> 31);
    let mixed = mixed.wrapping_mul(0x94D049BB133111EB);
    (mixed as f64 / u64::MAX as f64) as f32
}

/// Get a deterministic random integer range for a position.
pub fn position_random_range(seed: u64, variant: u32, min: i32, max: i32) -> i32 {
    let f = position_random(seed, variant);
    min + (f * (max - min + 1) as f32) as i32
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

    // =========================================================================
    // SEAMLESS CHUNK BOUNDARY TESTS
    // =========================================================================

    #[test]
    fn test_world_noise_coord_continuity() {
        // Test that noise coordinates are continuous across chunk boundaries
        // The last tile of chunk A should have coords close to the first tile of chunk B

        let scale = 0.02;

        // Last tile of chunk (0, 0)
        let [x1, y1] = world_noise_coord(0, 0, LOCAL_SIZE - 1, LOCAL_SIZE - 1, scale);

        // First tile of chunk (1, 1)
        let [x2, y2] = world_noise_coord(1, 1, 0, 0, scale);

        // These should differ by exactly 1 * scale (one tile apart)
        let expected_diff = scale;
        let actual_diff_x = x2 - x1;
        let actual_diff_y = y2 - y1;

        assert!((actual_diff_x - expected_diff).abs() < 0.0001,
            "X coords should differ by scale: {} vs {}", actual_diff_x, expected_diff);
        assert!((actual_diff_y - expected_diff).abs() < 0.0001,
            "Y coords should differ by scale: {} vs {}", actual_diff_y, expected_diff);
    }

    #[test]
    fn test_feature_seed_determinism() {
        let world_seed = 12345u64;
        let abs_x = 1000;
        let abs_y = 2000;

        // Same position should always produce the same seed
        let seed1 = feature_seed(world_seed, abs_x, abs_y);
        let seed2 = feature_seed(world_seed, abs_x, abs_y);

        assert_eq!(seed1, seed2, "Feature seed should be deterministic");
    }

    #[test]
    fn test_feature_seed_uniqueness() {
        let world_seed = 12345u64;

        // Adjacent positions should produce different seeds
        let seed_a = feature_seed(world_seed, 100, 100);
        let seed_b = feature_seed(world_seed, 101, 100);
        let seed_c = feature_seed(world_seed, 100, 101);

        assert_ne!(seed_a, seed_b, "Adjacent X positions should have different seeds");
        assert_ne!(seed_a, seed_c, "Adjacent Y positions should have different seeds");
    }

    #[test]
    fn test_should_place_feature_distribution() {
        // Test that should_place_feature produces reasonable distributions
        // Note: We use multiple seeds to test overall behavior across different regions
        let density = 0.1f32; // 10% chance
        let mut total_count = 0usize;
        let samples = 10usize;
        let positions_per_sample = 1000usize;

        for seed_idx in 0..samples {
            let world_seed = (seed_idx as u64) * 12345u64;
            for x in 0..positions_per_sample {
                let pos_seed = feature_seed(world_seed, x, 0);
                if should_place_feature(pos_seed, density) {
                    total_count += 1;
                }
            }
        }

        // Should be roughly 10% across all samples (allow ±30% tolerance)
        let expected = (samples * positions_per_sample) as f32 * density;
        let tolerance = expected * 0.3; // 30% tolerance
        assert!(
            total_count as f32 >= expected - tolerance && total_count as f32 <= expected + tolerance,
            "Expected ~{} features at {}% density, got {}",
            expected, density * 100.0, total_count
        );
    }

    #[test]
    fn test_position_random_range() {
        let seed = 12345u64;

        // Test that values stay in range
        for variant in 0..100 {
            let val = position_random_range(seed, variant, 3, 7);
            assert!(val >= 3 && val <= 7,
                "Value {} should be in range 3-7", val);
        }
    }

    #[test]
    fn test_world_noise_coord_3d() {
        let scale_xy = 0.05;
        let scale_z = 0.08;

        let [x, y, z] = world_noise_coord_3d(1, 2, 10, 20, 5, scale_xy, scale_z);

        // Expected: (1*48 + 10) * 0.05 = 58 * 0.05 = 2.9
        // Expected: (2*48 + 20) * 0.05 = 116 * 0.05 = 5.8
        // Expected: 5 * 0.08 = 0.4
        assert!((x - 2.9).abs() < 0.0001, "X should be 2.9, got {}", x);
        assert!((y - 5.8).abs() < 0.0001, "Y should be 5.8, got {}", y);
        assert!((z - 0.4).abs() < 0.0001, "Z should be 0.4, got {}", z);
    }
}
