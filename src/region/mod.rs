//! Region map system for hierarchical zoom (Dwarf Fortress-style)
//!
//! Pre-calculates "handshake" data during world generation to ensure seamless
//! stitching between adjacent 64x64 region maps.

pub mod handshake;
pub mod rivers;
pub mod generator;
pub mod cache;

pub use handshake::{
    TileHandshake, RegionHandshake, WorldHandshakes,
    VegetationPattern, RockLayer, HandshakeInput,
    calculate_world_handshakes_full,
};
pub use rivers::RiverEdgeCrossing;
pub use generator::{RegionMap, generate_region, REGION_SIZE};
pub use cache::{RegionCache, RegionLOD};
