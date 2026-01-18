//! Local map generation system.
//!
//! Generates detailed playable local maps (64x64 by default) from overworld tiles.
//! Each local map contains biome-appropriate terrain features with edge blending
//! to match neighboring tiles.
//!
//! # Example
//!
//! ```ignore
//! use planet_generation::local::{generate_local_map, LocalMap};
//! use planet_generation::world::WorldData;
//!
//! // Generate a local map for tile (100, 50)
//! let local = generate_local_map(&world, 100, 50, 64);
//!
//! // Export to PNG
//! export_local_map(&local, "local_100_50.png").unwrap();
//! ```

mod biome_features;
mod export;
mod generation;
mod terrain;
mod types;

// Re-export main types
pub use biome_features::{get_biome_features, BiomeFeatureConfig};
pub use export::{
    export_local_map, export_local_map_detailed, export_local_map_scaled,
    export_movement_cost_map, export_walkability_map,
    export_elevation_heatmap, export_blend_zone_map, export_local_map_shaded,
    export_local_map_layered,
};
pub use generation::{generate_local_map, generate_local_map_default};
pub use terrain::{LocalFeature, LocalTerrainType};
pub use types::{LocalMap, LocalTile, NeighborInfo, DEFAULT_LOCAL_MAP_SIZE};
