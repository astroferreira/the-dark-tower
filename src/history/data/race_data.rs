//! Race template data loaded from JSON.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// A race definition loaded from data files.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaceTemplate {
    pub tag: String,
    pub plural_name: String,
    pub naming_archetype: String,
    pub lifespan: [u32; 2],
    pub maturity_age: u32,
    pub can_reproduce: bool,
    pub preferred_biomes: Vec<String>,
    pub innate_abilities: Vec<String>,
    #[serde(default)]
    pub biome_settlement_weights: HashMap<String, f32>,
}

/// Container for deserializing the races JSON file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RacesFile {
    pub races: Vec<RaceTemplate>,
}
