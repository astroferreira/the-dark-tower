//! Planet generation library
//!
//! A procedural world map generator featuring:
//! - Tectonic plate simulation
//! - Hydraulic and glacial erosion
//! - Climate modeling (temperature, moisture)
//! - Climate-based biome types
//! - Water body detection (oceans, lakes, rivers)

pub mod ascii;
pub mod biome_feathering;
pub mod biomes;
pub mod climate;
pub mod coastline;
pub mod erosion;
pub mod heightmap;
pub mod plates;
pub mod scale;
pub mod seeds;
pub mod tilemap;
pub mod water_bodies;
pub mod world;
