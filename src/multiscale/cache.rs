//! LRU chunk cache for efficient memory management of local chunks.
//!
//! Provides caching for local chunks (embark sites) with configurable memory budgets.
//! Supports optional disk persistence to ensure generated chunks remain consistent.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::Path;

use crate::world::WorldData;
use super::local::{LocalChunk, BoundaryConditions, ChunkEdge, EdgeDirection, generate_local_chunk_with_boundaries};
use super::storage::ChunkStorage;
use super::DEFAULT_LOCAL_CACHE_SIZE;

/// Cache statistics for monitoring
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Number of evictions
    pub evictions: usize,
    /// Current number of cached local chunks
    pub local_count: usize,
    /// Estimated memory usage in bytes
    pub memory_bytes: usize,
}

impl CacheStats {
    /// Calculate hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }

    /// Format as human-readable string
    pub fn summary(&self) -> String {
        format!(
            "Hits: {} | Misses: {} | Rate: {:.1}% | Chunks: {} | Mem: {:.1}MB",
            self.hits,
            self.misses,
            self.hit_rate() * 100.0,
            self.local_count,
            self.memory_bytes as f32 / (1024.0 * 1024.0)
        )
    }
}

/// LRU cache for local chunks
struct LocalCache {
    /// Cached chunks by (world_x, world_y)
    chunks: HashMap<(usize, usize), LocalChunk>,
    /// LRU order (most recent at back)
    lru_order: VecDeque<(usize, usize)>,
    /// Maximum number of chunks
    max_size: usize,
}

impl LocalCache {
    fn new(max_size: usize) -> Self {
        Self {
            chunks: HashMap::with_capacity(max_size),
            lru_order: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    fn get(&mut self, key: (usize, usize)) -> Option<&LocalChunk> {
        if self.chunks.contains_key(&key) {
            // Move to back of LRU
            self.lru_order.retain(|k| *k != key);
            self.lru_order.push_back(key);
            self.chunks.get(&key)
        } else {
            None
        }
    }

    fn get_mut(&mut self, key: (usize, usize)) -> Option<&mut LocalChunk> {
        if self.chunks.contains_key(&key) {
            // Move to back of LRU
            self.lru_order.retain(|k| *k != key);
            self.lru_order.push_back(key);
            self.chunks.get_mut(&key)
        } else {
            None
        }
    }

    fn insert(&mut self, key: (usize, usize), chunk: LocalChunk) -> Option<(usize, usize)> {
        let mut evicted = None;

        // Check if we need to evict
        if self.chunks.len() >= self.max_size && !self.chunks.contains_key(&key) {
            if let Some(old_key) = self.lru_order.pop_front() {
                self.chunks.remove(&old_key);
                evicted = Some(old_key);
            }
        }

        // Remove from LRU if already present
        self.lru_order.retain(|k| *k != key);
        self.lru_order.push_back(key);
        self.chunks.insert(key, chunk);

        evicted
    }

    fn len(&self) -> usize {
        self.chunks.len()
    }

    fn memory_size(&self) -> usize {
        self.chunks.values().map(|c| c.memory_size()).sum()
    }

    fn clear(&mut self) {
        self.chunks.clear();
        self.lru_order.clear();
    }

    fn contains(&self, key: &(usize, usize)) -> bool {
        self.chunks.contains_key(key)
    }
}

/// Chunk cache for local chunks (embark sites).
///
/// Provides LRU eviction to stay within memory budget.
/// Each world tile can have one cached local chunk.
/// Optionally persists chunks to disk for consistency across sessions.
pub struct ChunkCache {
    /// Local chunk cache
    local: LocalCache,
    /// Statistics
    stats: CacheStats,
    /// Optional disk storage for persistence
    storage: Option<ChunkStorage>,
    /// Number of chunks loaded from disk
    disk_loads: usize,
    /// Number of chunks saved to disk
    disk_saves: usize,
}

impl ChunkCache {
    /// Create a new chunk cache with default size (no persistence)
    pub fn new() -> Self {
        Self {
            local: LocalCache::new(DEFAULT_LOCAL_CACHE_SIZE),
            stats: CacheStats::default(),
            storage: None,
            disk_loads: 0,
            disk_saves: 0,
        }
    }

    /// Create a new chunk cache with custom size (no persistence)
    pub fn with_size(local_max: usize) -> Self {
        Self {
            local: LocalCache::new(local_max),
            stats: CacheStats::default(),
            storage: None,
            disk_loads: 0,
            disk_saves: 0,
        }
    }

    /// Create a new chunk cache with disk persistence enabled.
    ///
    /// Chunks will be saved to disk when generated and loaded from disk
    /// when requested, ensuring consistency across sessions.
    pub fn with_persistence<P: AsRef<Path>>(base_dir: P, world_seed: u64, local_max: usize) -> Self {
        Self {
            local: LocalCache::new(local_max),
            stats: CacheStats::default(),
            storage: Some(ChunkStorage::new(base_dir, world_seed)),
            disk_loads: 0,
            disk_saves: 0,
        }
    }

    /// Enable disk persistence for this cache.
    pub fn enable_persistence<P: AsRef<Path>>(&mut self, base_dir: P, world_seed: u64) {
        self.storage = Some(ChunkStorage::new(base_dir, world_seed));
    }

    /// Check if persistence is enabled
    pub fn has_persistence(&self) -> bool {
        self.storage.is_some()
    }

    /// Get disk load count
    pub fn disk_loads(&self) -> usize {
        self.disk_loads
    }

    /// Get disk save count
    pub fn disk_saves(&self) -> usize {
        self.disk_saves
    }

    /// Get a local chunk from cache
    pub fn get_local(&mut self, world_x: usize, world_y: usize) -> Option<&LocalChunk> {
        let key = (world_x, world_y);
        if let Some(chunk) = self.local.get(key) {
            self.stats.hits += 1;
            Some(chunk)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Get a mutable local chunk from cache
    pub fn get_local_mut(&mut self, world_x: usize, world_y: usize) -> Option<&mut LocalChunk> {
        let key = (world_x, world_y);
        if let Some(chunk) = self.local.get_mut(key) {
            self.stats.hits += 1;
            Some(chunk)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Insert a local chunk into cache
    pub fn insert_local(&mut self, chunk: LocalChunk) {
        let key = (chunk.world_x, chunk.world_y);
        if self.local.insert(key, chunk).is_some() {
            self.stats.evictions += 1;
        }
        self.update_stats();
    }

    /// Check if a local chunk is cached
    pub fn has_local(&self, world_x: usize, world_y: usize) -> bool {
        self.local.contains(&(world_x, world_y))
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Clear all caches
    pub fn clear(&mut self) {
        self.local.clear();
        self.stats = CacheStats::default();
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.local_count = self.local.len();
        self.stats.memory_bytes = self.local.memory_size();
    }

    /// Pre-warm cache hint (actual generation is lazy)
    pub fn warm_local(&mut self, _center_x: usize, _center_y: usize, _radius: usize) {
        // This is just a hint - actual chunks are generated on demand
        // Could be used to trigger background generation in future
    }

    /// Extract boundary conditions from already-cached neighboring chunks.
    ///
    /// For a chunk at (world_x, world_y), this extracts edges from:
    /// - North neighbor (world_x, world_y - 1): their south edge becomes our north boundary
    /// - South neighbor (world_x, world_y + 1): their north edge becomes our south boundary
    /// - West neighbor (world_x - 1, world_y): their east edge becomes our west boundary
    /// - East neighbor (world_x + 1, world_y): their west edge becomes our east boundary
    pub fn get_boundary_conditions(&self, world_x: usize, world_y: usize) -> BoundaryConditions {
        let mut boundaries = BoundaryConditions::new();

        // North neighbor (y - 1) provides our north edge from their south edge
        if world_y > 0 {
            if let Some(north_chunk) = self.local.chunks.get(&(world_x, world_y - 1)) {
                boundaries.north = Some(north_chunk.extract_edge(EdgeDirection::South));
            }
        }

        // South neighbor (y + 1) provides our south edge from their north edge
        if let Some(south_chunk) = self.local.chunks.get(&(world_x, world_y + 1)) {
            boundaries.south = Some(south_chunk.extract_edge(EdgeDirection::North));
        }

        // West neighbor (x - 1) provides our west edge from their east edge
        if world_x > 0 {
            if let Some(west_chunk) = self.local.chunks.get(&(world_x - 1, world_y)) {
                boundaries.west = Some(west_chunk.extract_edge(EdgeDirection::East));
            }
        }

        // East neighbor (x + 1) provides our east edge from their west edge
        if let Some(east_chunk) = self.local.chunks.get(&(world_x + 1, world_y)) {
            boundaries.east = Some(east_chunk.extract_edge(EdgeDirection::West));
        }

        boundaries
    }

    /// Get or generate a local chunk, using boundary conditions from cached neighbors.
    ///
    /// This is the preferred way to get chunks when seamless boundaries are required.
    /// It automatically:
    /// 1. Checks the memory cache
    /// 2. Checks disk storage (if persistence is enabled)
    /// 3. Generates with boundary conditions from neighbors
    /// 4. Saves to disk (if persistence is enabled)
    pub fn get_or_generate_local(
        &mut self,
        world: &WorldData,
        world_x: usize,
        world_y: usize,
    ) -> &LocalChunk {
        let key = (world_x, world_y);

        // Check memory cache first
        if self.local.contains(&key) {
            self.stats.hits += 1;
            return self.local.chunks.get(&key).unwrap();
        }

        // Check disk storage if enabled
        if let Some(ref storage) = self.storage {
            if let Ok(Some(chunk)) = storage.load_chunk(world_x, world_y) {
                // Found on disk - insert into memory cache
                if self.local.insert(key, chunk).is_some() {
                    self.stats.evictions += 1;
                }
                self.disk_loads += 1;
                self.stats.hits += 1; // Count as hit since it existed
                self.update_stats();
                return self.local.chunks.get(&key).unwrap();
            }
        }

        // Not in cache or on disk - need to generate
        // First, try to load neighbors from disk for boundary conditions
        self.ensure_neighbors_loaded(world, world_x, world_y);

        // Get boundary conditions from cached neighbors (now including disk-loaded ones)
        let boundaries = self.get_boundary_conditions(world_x, world_y);

        // Generate the chunk with boundary conditions
        let chunk = generate_local_chunk_with_boundaries(world, world_x, world_y, &boundaries);

        // Save to disk if persistence is enabled
        if let Some(ref storage) = self.storage {
            if let Err(e) = storage.save_chunk(&chunk) {
                eprintln!("Warning: Failed to save chunk ({}, {}): {}", world_x, world_y, e);
            } else {
                self.disk_saves += 1;
            }
        }

        // Insert into memory cache
        if self.local.insert(key, chunk).is_some() {
            self.stats.evictions += 1;
        }
        self.stats.misses += 1;
        self.update_stats();

        self.local.chunks.get(&key).unwrap()
    }

    /// Ensure neighboring chunks are loaded (from disk if available) for boundary conditions.
    fn ensure_neighbors_loaded(&mut self, world: &WorldData, world_x: usize, world_y: usize) {
        if self.storage.is_none() {
            return;
        }

        let neighbors = [
            (world_x.wrapping_sub(1), world_y),  // West
            (world_x + 1, world_y),               // East
            (world_x, world_y.wrapping_sub(1)),  // North
            (world_x, world_y + 1),               // South
        ];

        for (nx, ny) in neighbors {
            // Skip if already in memory cache
            if self.local.contains(&(nx, ny)) {
                continue;
            }

            // Skip invalid coordinates (wrapping overflow)
            if nx >= world.heightmap.width || ny >= world.heightmap.height {
                continue;
            }

            // Try to load from disk
            if let Some(ref storage) = self.storage {
                if let Ok(Some(chunk)) = storage.load_chunk(nx, ny) {
                    if self.local.insert((nx, ny), chunk).is_some() {
                        self.stats.evictions += 1;
                    }
                    self.disk_loads += 1;
                    self.update_stats();
                }
            }
        }
    }

    /// Generate a local chunk with explicit boundary conditions.
    ///
    /// Use this when you have specific boundary requirements, or when you
    /// want to control exactly which edges are constrained.
    pub fn generate_with_boundaries(
        &mut self,
        world: &WorldData,
        world_x: usize,
        world_y: usize,
        boundaries: &BoundaryConditions,
    ) -> &LocalChunk {
        // Generate the chunk with boundary conditions
        let chunk = generate_local_chunk_with_boundaries(world, world_x, world_y, boundaries);

        // Insert into cache (potentially evicting old chunk)
        if self.local.insert((world_x, world_y), chunk).is_some() {
            self.stats.evictions += 1;
        }
        self.update_stats();

        self.local.chunks.get(&(world_x, world_y)).unwrap()
    }

    /// Get or generate a local chunk with validation.
    ///
    /// Like `get_or_generate_local`, but validates the chunk against boundary
    /// conditions and world data. If validation fails, the chunk is regenerated
    /// up to `max_retries` times.
    ///
    /// Returns the chunk and whether it passed validation.
    pub fn get_or_generate_validated(
        &mut self,
        world: &WorldData,
        world_x: usize,
        world_y: usize,
        max_retries: usize,
    ) -> (&LocalChunk, bool) {
        use super::verify::{verify_boundary_conditions, verify_geology_consistency, Severity};

        let key = (world_x, world_y);

        // Check memory cache first
        if self.local.contains(&key) {
            self.stats.hits += 1;
            // Already cached chunks are assumed valid
            return (self.local.chunks.get(&key).unwrap(), true);
        }

        // Check disk storage if enabled
        if let Some(ref storage) = self.storage {
            if let Ok(Some(chunk)) = storage.load_chunk(world_x, world_y) {
                // Found on disk - insert into memory cache (trusted)
                if self.local.insert(key, chunk).is_some() {
                    self.stats.evictions += 1;
                }
                self.disk_loads += 1;
                self.stats.hits += 1;
                self.update_stats();
                return (self.local.chunks.get(&key).unwrap(), true);
            }
        }

        // Not in cache - need to generate with validation
        self.ensure_neighbors_loaded(world, world_x, world_y);
        let boundaries = self.get_boundary_conditions(world_x, world_y);

        let mut last_valid = false;

        for attempt in 0..=max_retries {
            // Generate the chunk
            let chunk = generate_local_chunk_with_boundaries(world, world_x, world_y, &boundaries);

            // Validate against boundary conditions
            let boundary_results = verify_boundary_conditions(&chunk, &boundaries);
            let has_boundary_critical = boundary_results.iter()
                .any(|r| !r.passed && r.severity >= Severity::High);

            // Validate against world data
            let geology_results = verify_geology_consistency(world, &chunk, world_x, world_y);
            let has_geology_critical = geology_results.iter()
                .any(|r| !r.passed && r.severity >= Severity::Critical);

            let is_valid = !has_boundary_critical && !has_geology_critical;

            if is_valid || attempt == max_retries {
                // Accept this chunk (either valid or final retry)
                last_valid = is_valid;

                // Save to disk if persistence is enabled
                if let Some(ref storage) = self.storage {
                    if let Err(e) = storage.save_chunk(&chunk) {
                        eprintln!("Warning: Failed to save chunk ({}, {}): {}", world_x, world_y, e);
                    } else {
                        self.disk_saves += 1;
                    }
                }

                // Insert into memory cache
                if self.local.insert(key, chunk).is_some() {
                    self.stats.evictions += 1;
                }
                self.stats.misses += 1;
                self.update_stats();

                return (self.local.chunks.get(&key).unwrap(), last_valid);
            }

            // Log retry in debug builds
            #[cfg(debug_assertions)]
            eprintln!(
                "Chunk ({}, {}) failed validation, retry {}/{}",
                world_x, world_y, attempt + 1, max_retries
            );
        }

        // Should not reach here, but just in case
        (self.local.chunks.get(&key).unwrap(), last_valid)
    }

    /// Validate all cached chunks and get a summary.
    ///
    /// Returns (valid_count, invalid_count, issues).
    pub fn validate_cached_chunks(
        &self,
        world: &WorldData,
    ) -> (usize, usize, Vec<String>) {
        use super::verify::{verify_boundary_conditions, Severity};

        let mut valid = 0;
        let mut invalid = 0;
        let mut issues = Vec::new();

        for (&(world_x, world_y), chunk) in &self.local.chunks {
            let boundaries = self.get_boundary_conditions(world_x, world_y);
            let results = verify_boundary_conditions(chunk, &boundaries);

            let has_critical = results.iter()
                .any(|r| !r.passed && r.severity >= Severity::High);

            if has_critical {
                invalid += 1;
                for r in &results {
                    if !r.passed && r.severity >= Severity::High {
                        issues.push(format!(
                            "Chunk ({}, {}): {}",
                            world_x, world_y, r.message
                        ));
                    }
                }
            } else {
                valid += 1;
            }
        }

        (valid, invalid, issues)
    }
}

impl Default for ChunkCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_chunk(world_x: usize, world_y: usize) -> LocalChunk {
        LocalChunk::new(world_x, world_y, 0)
    }

    #[test]
    fn test_cache_basic() {
        let mut cache = ChunkCache::new();

        // Insert a local chunk
        let chunk = make_test_chunk(10, 20);
        cache.insert_local(chunk);

        assert!(cache.has_local(10, 20));
        assert!(!cache.has_local(10, 21));

        // Get should hit
        assert!(cache.get_local(10, 20).is_some());
        assert_eq!(cache.stats().hits, 1);

        // Miss
        assert!(cache.get_local(10, 21).is_none());
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = ChunkCache::with_size(3);

        // Fill cache
        for i in 0..3 {
            cache.insert_local(make_test_chunk(i, 0));
        }

        assert_eq!(cache.stats().local_count, 3);

        // Insert one more - should evict oldest (0, 0)
        cache.insert_local(make_test_chunk(3, 0));

        assert!(!cache.has_local(0, 0));
        assert!(cache.has_local(1, 0));
        assert!(cache.has_local(2, 0));
        assert!(cache.has_local(3, 0));
    }

    #[test]
    fn test_lru_access_updates_order() {
        let mut cache = ChunkCache::with_size(3);

        // Fill cache
        for i in 0..3 {
            cache.insert_local(make_test_chunk(i, 0));
        }

        // Access chunk 0 - should move it to end of LRU
        let _ = cache.get_local(0, 0);

        // Insert new chunk - should evict chunk 1 (now oldest)
        cache.insert_local(make_test_chunk(3, 0));

        assert!(cache.has_local(0, 0)); // Still present (accessed recently)
        assert!(!cache.has_local(1, 0)); // Evicted
        assert!(cache.has_local(2, 0));
        assert!(cache.has_local(3, 0));
    }
}
