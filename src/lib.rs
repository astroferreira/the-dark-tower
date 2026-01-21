//! Planet generation library
//!
//! A procedural world map generator featuring:
//! - Tectonic plate simulation
//! - Hydraulic and glacial erosion
//! - Climate modeling (temperature, moisture)
//! - 50+ biome types
//! - Water body detection (oceans, lakes, rivers)
//! - Human-made structures (castles, cities, villages, roads)
//! - Historical world enrichment (factions, events, settlements, monsters, trade routes)
//! - Multi-scale zoom system (world -> regional -> local)

pub mod ascii;
pub mod biomes;
pub mod climate;
pub mod erosion;
pub mod heightmap;
pub mod history;
pub mod multiscale;
pub mod plates;
pub mod scale;
pub mod structures;
pub mod tilemap;
pub mod water_bodies;
pub mod world;
pub mod zlevel;
