//! Naming style template data loaded from JSON.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// A naming style definition loaded from data files.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamingStyleTemplate {
    pub onset_consonants: Vec<String>,
    pub coda_consonants: Vec<String>,
    pub vowels: Vec<String>,
    pub syllable_range: [u8; 2],
    pub uses_apostrophes: bool,
    pub uses_hyphens: bool,
    pub place_prefixes: Vec<String>,
    pub place_suffixes: Vec<String>,
    pub epithet_patterns: Vec<String>,
}

/// Container for deserializing the naming styles JSON file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamingStylesFile {
    pub naming_styles: HashMap<String, NamingStyleTemplate>,
}
