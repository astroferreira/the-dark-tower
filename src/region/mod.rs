//! Regions: the zoomed region (`zoom.rs`), its landforms (`landforms/`, ROADMAP Stage 2), its
//! places and caves (`sites/`, Stages 4-5) and the site layer's types (`plan.rs`).
//!
//! `handshake.rs` carries each world tile's rock stack (the embark reads it); `generator.rs`,
//! `cache.rs` and `rivers.rs` are the 64x64 `RegionMap` of the legacy explorer.

pub mod handshake;
pub mod rivers;
pub mod generator;
pub mod cache;
pub mod zoom;
pub mod plan;
pub mod landforms;
pub mod sites;
pub mod debug;

pub use handshake::{
    TileHandshake, RegionHandshake, WorldHandshakes,
    VegetationPattern, RockLayer, HandshakeInput,
    calculate_world_handshakes_full,
};
pub use rivers::RiverEdgeCrossing;
pub use generator::{RegionMap, generate_region, REGION_SIZE};
pub use cache::{RegionCache, RegionLOD};
pub use plan::{Landform, Place, PlaceKind, CaveGraph};
