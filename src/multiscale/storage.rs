//! Chunk persistence for local maps.
//!
//! Provides save/load functionality for LocalChunks to ensure generated
//! maps remain consistent across sessions.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use super::local::LocalChunk;

/// Storage manager for persisting local chunks to disk.
///
/// Chunks are stored in a directory structure organized by world seed:
/// `{base_dir}/{world_seed}/chunk_{x}_{y}.bin`
pub struct ChunkStorage {
    /// Base directory for all chunk storage
    base_dir: PathBuf,
    /// World seed (used for directory organization)
    world_seed: u64,
}

impl ChunkStorage {
    /// Create a new chunk storage manager.
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for storing chunks (e.g., "saves/chunks")
    /// * `world_seed` - Seed of the world (for directory organization)
    pub fn new<P: AsRef<Path>>(base_dir: P, world_seed: u64) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            world_seed,
        }
    }

    /// Get the directory for this world's chunks
    fn world_dir(&self) -> PathBuf {
        self.base_dir.join(format!("world_{}", self.world_seed))
    }

    /// Get the file path for a specific chunk
    fn chunk_path(&self, world_x: usize, world_y: usize) -> PathBuf {
        self.world_dir().join(format!("chunk_{}_{}.bin", world_x, world_y))
    }

    /// Ensure the storage directory exists
    fn ensure_dir(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.world_dir())
    }

    /// Check if a chunk exists on disk
    pub fn chunk_exists(&self, world_x: usize, world_y: usize) -> bool {
        self.chunk_path(world_x, world_y).exists()
    }

    /// Save a chunk to disk.
    ///
    /// Uses bincode for efficient binary serialization.
    pub fn save_chunk(&self, chunk: &LocalChunk) -> Result<(), ChunkStorageError> {
        self.ensure_dir()?;

        let path = self.chunk_path(chunk.world_x, chunk.world_y);
        let file = File::create(&path)?;
        let writer = BufWriter::new(file);

        bincode::serialize_into(writer, chunk)
            .map_err(|e| ChunkStorageError::Serialization(e.to_string()))?;

        Ok(())
    }

    /// Load a chunk from disk.
    ///
    /// Returns None if the chunk doesn't exist.
    pub fn load_chunk(&self, world_x: usize, world_y: usize) -> Result<Option<LocalChunk>, ChunkStorageError> {
        let path = self.chunk_path(world_x, world_y);

        if !path.exists() {
            return Ok(None);
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let chunk: LocalChunk = bincode::deserialize_from(reader)
            .map_err(|e| ChunkStorageError::Deserialization(e.to_string()))?;

        Ok(Some(chunk))
    }

    /// Delete a chunk from disk (if it exists).
    pub fn delete_chunk(&self, world_x: usize, world_y: usize) -> std::io::Result<()> {
        let path = self.chunk_path(world_x, world_y);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// List all saved chunks for this world.
    ///
    /// Returns a list of (world_x, world_y) coordinates.
    pub fn list_chunks(&self) -> Result<Vec<(usize, usize)>, ChunkStorageError> {
        let dir = self.world_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                // Parse "chunk_X_Y" format
                if filename.starts_with("chunk_") {
                    let parts: Vec<&str> = filename[6..].split('_').collect();
                    if parts.len() == 2 {
                        if let (Ok(x), Ok(y)) = (parts[0].parse(), parts[1].parse()) {
                            chunks.push((x, y));
                        }
                    }
                }
            }
        }

        Ok(chunks)
    }

    /// Get the total size of stored chunks in bytes.
    pub fn total_size(&self) -> std::io::Result<u64> {
        let dir = self.world_dir();
        if !dir.exists() {
            return Ok(0);
        }

        let mut total = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            total += entry.metadata()?.len();
        }

        Ok(total)
    }

    /// Clear all stored chunks for this world.
    pub fn clear(&self) -> std::io::Result<()> {
        let dir = self.world_dir();
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}

/// Errors that can occur during chunk storage operations.
#[derive(Debug)]
pub enum ChunkStorageError {
    /// IO error (file not found, permissions, etc.)
    Io(std::io::Error),
    /// Serialization error
    Serialization(String),
    /// Deserialization error (corrupted file, version mismatch, etc.)
    Deserialization(String),
}

impl std::fmt::Display for ChunkStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkStorageError::Io(e) => write!(f, "IO error: {}", e),
            ChunkStorageError::Serialization(e) => write!(f, "Serialization error: {}", e),
            ChunkStorageError::Deserialization(e) => write!(f, "Deserialization error: {}", e),
        }
    }
}

impl std::error::Error for ChunkStorageError {}

impl From<std::io::Error> for ChunkStorageError {
    fn from(e: std::io::Error) -> Self {
        ChunkStorageError::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_test_chunk(x: usize, y: usize) -> LocalChunk {
        LocalChunk::new(x, y, 0)
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let storage = ChunkStorage::new(dir.path(), 12345);

        let chunk = make_test_chunk(10, 20);
        storage.save_chunk(&chunk).unwrap();

        assert!(storage.chunk_exists(10, 20));
        assert!(!storage.chunk_exists(10, 21));

        let loaded = storage.load_chunk(10, 20).unwrap().unwrap();
        assert_eq!(loaded.world_x, 10);
        assert_eq!(loaded.world_y, 20);
    }

    #[test]
    fn test_list_chunks() {
        let dir = tempdir().unwrap();
        let storage = ChunkStorage::new(dir.path(), 12345);

        storage.save_chunk(&make_test_chunk(1, 2)).unwrap();
        storage.save_chunk(&make_test_chunk(3, 4)).unwrap();
        storage.save_chunk(&make_test_chunk(5, 6)).unwrap();

        let chunks = storage.list_chunks().unwrap();
        assert_eq!(chunks.len(), 3);
        assert!(chunks.contains(&(1, 2)));
        assert!(chunks.contains(&(3, 4)));
        assert!(chunks.contains(&(5, 6)));
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempdir().unwrap();
        let storage = ChunkStorage::new(dir.path(), 12345);

        let result = storage.load_chunk(99, 99).unwrap();
        assert!(result.is_none());
    }
}
