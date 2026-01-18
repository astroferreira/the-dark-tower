//! Resource system for the civilization simulation

pub mod stockpile;
pub mod extraction;

pub use stockpile::Stockpile;
pub use extraction::{BiomeResources, extract_resources, biome_resources};
