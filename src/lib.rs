//! Planet generation library
//!
//! A procedural world map generator featuring:
//! - Tectonic plate simulation
//! - Hydraulic and glacial erosion
//! - Climate modeling (temperature, moisture)
//! - Climate-based biome types
//! - Water body detection (oceans, lakes, rivers)
//! - Microclimate effects (valleys, ridges, lake proximity)
//! - Seasonal climate variation
//! - Extreme weather zone detection

// Suppress warnings for unused code - many utilities are kept for future use
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod ascii;
pub mod biome_feathering;
pub mod biomes;
pub mod climate;
pub mod coastline;
pub mod erosion;
pub mod exr_export;
pub mod heightmap;
pub mod map_export;
pub mod microclimate;
pub mod plates;
pub mod region;
pub mod scale;
pub mod seasons;
pub mod seeds;
pub mod tilemap;
pub mod underground_water;
pub mod water_bodies;
pub mod weather_zones;
pub mod world;
