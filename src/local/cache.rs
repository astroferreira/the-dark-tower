//! Local map caching system
//!
//! Provides an LRU cache for generated local maps, enabling seamless
//! navigation across world tile boundaries without regenerating maps.

use std::collections::HashMap;
use std::time::Instant;

use crate::local::{generate_local_map_default, LocalMap, LocalTile};
use crate::simulation::types::{TileCoord, GlobalLocalCoord, LOCAL_MAP_SIZE};
use crate::world::WorldData;

/// Maximum number of local maps to keep in cache
const DEFAULT_MAX_CACHED: usize = 25;

/// Distance in local tiles from edge to trigger preloading
const PRELOAD_DISTANCE: u32 = 16;

/// A cached local map with metadata
pub struct CachedLocalMap {
    pub map: LocalMap,
    pub last_accessed: Instant,
}

impl CachedLocalMap {
    fn new(map: LocalMap) -> Self {
        CachedLocalMap {
            map,
            last_accessed: Instant::now(),
        }
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// Cache for local maps with LRU eviction
pub struct LocalMapCache {
    /// Cached local maps indexed by world tile coordinate
    maps: HashMap<TileCoord, CachedLocalMap>,
    /// Maximum number of maps to keep in cache
    max_cached: usize,
    /// Current camera world tile (for preloading decisions)
    camera_tile: TileCoord,
    /// World dimensions for wrapping
    world_width: usize,
    world_height: usize,
}

impl LocalMapCache {
    /// Create a new cache with default settings
    pub fn new(world_width: usize, world_height: usize) -> Self {
        LocalMapCache {
            maps: HashMap::new(),
            max_cached: DEFAULT_MAX_CACHED,
            camera_tile: TileCoord::new(0, 0),
            world_width,
            world_height,
        }
    }

    /// Create a new cache with custom max size
    pub fn with_capacity(max_cached: usize, world_width: usize, world_height: usize) -> Self {
        LocalMapCache {
            maps: HashMap::new(),
            max_cached,
            camera_tile: TileCoord::new(0, 0),
            world_width,
            world_height,
        }
    }

    /// Update the camera position and preload adjacent tiles
    pub fn update_camera(&mut self, world: &WorldData, camera: GlobalLocalCoord) {
        self.camera_tile = camera.world_tile();
        self.preload_adjacent(world, camera);
        self.evict_distant();
    }

    /// Get a tile at a global local coordinate
    pub fn get_tile(&mut self, world: &WorldData, coord: GlobalLocalCoord) -> Option<&LocalTile> {
        let (world_tile, local_offset) = coord.to_hierarchical();

        // Ensure the map is loaded
        let map = self.get_or_load_map(world, world_tile);

        // Bounds check
        if local_offset.x < map.width && local_offset.y < map.height {
            Some(map.get(local_offset.x, local_offset.y))
        } else {
            None
        }
    }

    /// Get a local map, loading it if necessary
    pub fn get_or_load_map(&mut self, world: &WorldData, tile: TileCoord) -> &LocalMap {
        // Check bounds
        let tile = TileCoord::new(
            tile.x % self.world_width,
            tile.y.min(self.world_height - 1),
        );

        if !self.maps.contains_key(&tile) {
            // Generate and cache the map
            let map = generate_local_map_default(world, tile.x, tile.y);
            self.maps.insert(tile, CachedLocalMap::new(map));
        }

        // Touch to update access time
        if let Some(cached) = self.maps.get_mut(&tile) {
            cached.touch();
        }

        &self.maps.get(&tile).unwrap().map
    }

    /// Get a mutable reference to a local map
    pub fn get_map_mut(&mut self, world: &WorldData, tile: TileCoord) -> &mut LocalMap {
        // Check bounds
        let tile = TileCoord::new(
            tile.x % self.world_width,
            tile.y.min(self.world_height - 1),
        );

        if !self.maps.contains_key(&tile) {
            let map = generate_local_map_default(world, tile.x, tile.y);
            self.maps.insert(tile, CachedLocalMap::new(map));
        }

        if let Some(cached) = self.maps.get_mut(&tile) {
            cached.touch();
        }

        &mut self.maps.get_mut(&tile).unwrap().map
    }

    /// Check if a map is cached
    pub fn is_cached(&self, tile: &TileCoord) -> bool {
        self.maps.contains_key(tile)
    }

    /// Preload adjacent tiles based on camera position
    fn preload_adjacent(&mut self, world: &WorldData, camera: GlobalLocalCoord) {
        let local_offset = camera.local_offset();
        let camera_tile = camera.world_tile();

        // Determine which adjacent tiles need preloading based on position within current tile
        let mut tiles_to_load = Vec::new();

        // Always load the current tile
        tiles_to_load.push(camera_tile);

        // Check proximity to edges and preload adjacent tiles
        let near_left = local_offset.x < PRELOAD_DISTANCE as usize;
        let near_right = local_offset.x >= (LOCAL_MAP_SIZE as usize - PRELOAD_DISTANCE as usize);
        let near_top = local_offset.y < PRELOAD_DISTANCE as usize;
        let near_bottom = local_offset.y >= (LOCAL_MAP_SIZE as usize - PRELOAD_DISTANCE as usize);

        // Cardinal neighbors
        if near_left {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32 - 1, camera_tile.y as i32));
        }
        if near_right {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32 + 1, camera_tile.y as i32));
        }
        if near_top {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32, camera_tile.y as i32 - 1));
        }
        if near_bottom {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32, camera_tile.y as i32 + 1));
        }

        // Diagonal neighbors if near corners
        if near_left && near_top {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32 - 1, camera_tile.y as i32 - 1));
        }
        if near_right && near_top {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32 + 1, camera_tile.y as i32 - 1));
        }
        if near_left && near_bottom {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32 - 1, camera_tile.y as i32 + 1));
        }
        if near_right && near_bottom {
            tiles_to_load.push(self.wrap_tile(camera_tile.x as i32 + 1, camera_tile.y as i32 + 1));
        }

        // Load all needed tiles
        for tile in tiles_to_load {
            if !self.maps.contains_key(&tile) {
                let map = generate_local_map_default(world, tile.x, tile.y);
                self.maps.insert(tile, CachedLocalMap::new(map));
            }
        }
    }

    /// Wrap tile coordinates with horizontal wrapping
    fn wrap_tile(&self, x: i32, y: i32) -> TileCoord {
        let wrapped_x = x.rem_euclid(self.world_width as i32) as usize;
        let clamped_y = y.clamp(0, self.world_height as i32 - 1) as usize;
        TileCoord::new(wrapped_x, clamped_y)
    }

    /// Evict distant tiles from cache when over capacity
    fn evict_distant(&mut self) {
        if self.maps.len() <= self.max_cached {
            return;
        }

        // Calculate distance from camera for each cached tile
        let mut distances: Vec<(TileCoord, usize, Instant)> = self
            .maps
            .iter()
            .map(|(tile, cached)| {
                let dist = self.camera_tile.distance_wrapped(tile, self.world_width);
                (*tile, dist, cached.last_accessed)
            })
            .collect();

        // Sort by distance (furthest first), then by access time (oldest first)
        distances.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2))
        });

        // Remove tiles until under capacity
        while self.maps.len() > self.max_cached && !distances.is_empty() {
            let (tile, _, _) = distances.remove(0);
            self.maps.remove(&tile);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            cached_count: self.maps.len(),
            max_capacity: self.max_cached,
            camera_tile: self.camera_tile,
        }
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.maps.clear();
    }

    /// Get all cached tile coordinates
    pub fn cached_tiles(&self) -> Vec<TileCoord> {
        self.maps.keys().copied().collect()
    }

    /// Force load a 3x3 grid around a position
    pub fn preload_grid(&mut self, world: &WorldData, center: TileCoord, radius: usize) {
        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                let tile = self.wrap_tile(center.x as i32 + dx, center.y as i32 + dy);
                if !self.maps.contains_key(&tile) {
                    let map = generate_local_map_default(world, tile.x, tile.y);
                    self.maps.insert(tile, CachedLocalMap::new(map));
                }
            }
        }
    }
}

/// Statistics about the cache
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cached_count: usize,
    pub max_capacity: usize,
    pub camera_tile: TileCoord,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_local_coord_conversion() {
        let global = GlobalLocalCoord::new(100, 200);
        let (tile, offset) = global.to_hierarchical();

        assert_eq!(tile.x, 1); // 100 / 64 = 1
        assert_eq!(tile.y, 3); // 200 / 64 = 3
        assert_eq!(offset.x, 36); // 100 % 64 = 36
        assert_eq!(offset.y, 8); // 200 % 64 = 8

        // Test round-trip
        let reconstructed = GlobalLocalCoord::from_hierarchical(tile, offset);
        assert_eq!(global, reconstructed);
    }

    #[test]
    fn test_from_world_tile() {
        let tile = TileCoord::new(5, 10);
        let global = GlobalLocalCoord::from_world_tile(tile);

        assert_eq!(global.x, 5 * 64 + 32); // center of tile
        assert_eq!(global.y, 10 * 64 + 32);
        assert_eq!(global.world_tile(), tile);
    }
}
