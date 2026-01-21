//! LRU chunk cache for efficient memory management of local chunks.
//!
//! Provides caching for local chunks (embark sites) with configurable memory budgets.

use std::collections::HashMap;
use std::collections::VecDeque;

use super::local::LocalChunk;
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
pub struct ChunkCache {
    /// Local chunk cache
    local: LocalCache,
    /// Statistics
    stats: CacheStats,
}

impl ChunkCache {
    /// Create a new chunk cache with default size
    pub fn new() -> Self {
        Self {
            local: LocalCache::new(DEFAULT_LOCAL_CACHE_SIZE),
            stats: CacheStats::default(),
        }
    }

    /// Create a new chunk cache with custom size
    pub fn with_size(local_max: usize) -> Self {
        Self {
            local: LocalCache::new(local_max),
            stats: CacheStats::default(),
        }
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
