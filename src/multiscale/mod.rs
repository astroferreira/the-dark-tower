//! Multi-scale zoom system for seamless exploration from world to local scale.
//!
//! This module enables Dwarf Fortress-style zooming from world-scale (~5km/tile)
//! down to local-scale (~2m/tile) for detailed embark sites with full z-level geology.
//!
//! # Scale Hierarchy (Dwarf Fortress Style)
//!
//! | Level | Scale      | Resolution           | Purpose                              |
//! |-------|------------|----------------------|--------------------------------------|
//! | World | 5 km/tile  | 512×256 (existing)   | Continents, biomes, faction territories |
//! | Local | 2 m/tile   | 48×48×Z per world tile | Embark site with full z-level geology |
//!
//! Conversion: 1 world tile = 48×48 local tiles (2304 tiles per world tile)
//!
//! # Z-Level Structure
//!
//! Local maps emphasize vertical depth (z-levels). The z-level range comes from
//! the world zlevel system (MIN_Z to MAX_Z, currently -16 to +16).
//!
//! From top to bottom:
//! - Sky (above surface_z)
//! - Surface (biome-dependent terrain)
//! - Soil layers (depth varies by biome)
//! - Stone layers
//! - Cavern layers (from world cave system)
//! - Magma sea (if volcanic)

pub mod biome_terrain;
pub mod cache;
pub mod coords;
pub mod geology;
pub mod local;
pub mod structures;
pub mod terrain;
pub mod verify;

pub use biome_terrain::{
    BiomeTerrainConfig, AdjacentBiomes,
    get_biome_config, generate_biome_surface, add_biome_features,
    generate_blended_biome_surface, add_blended_biome_features,
};
pub use cache::{ChunkCache, CacheStats};
pub use coords::{LocalCoord, ScaleLevel, local_seed, chunk_seed};
pub use geology::{GeologyParams, derive_geology};
pub use local::{LocalChunk, LocalTile, LocalFeature, LocalTerrain, Material, SoilType, StoneType, LairType, StructureType};
pub use verify::{
    VerifyResult, VerifyCategory, Severity, VerificationStatus, VerificationReport,
    verify_chunk, verify_world_sample, verify_world_quick, verify_world_thorough,
};

/// Tiles per world tile at local scale (48×48 local tiles per world tile)
pub const LOCAL_SIZE: usize = 48;

/// Scale in meters per tile at each level
pub const WORLD_METERS_PER_TILE: f32 = 5000.0;     // 5 km
pub const LOCAL_METERS_PER_TILE: f32 = 2.0;        // 2 m

/// Default maximum local chunks to cache
pub const DEFAULT_LOCAL_CACHE_SIZE: usize = 64;

/// Meters per z-level (from zlevel module, re-exported for convenience)
pub use crate::zlevel::FLOOR_HEIGHT as METERS_PER_Z_LEVEL;
