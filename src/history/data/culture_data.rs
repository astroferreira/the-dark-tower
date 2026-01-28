//! Culture bias data loaded from JSON.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Culture value biases: [center, spread] pairs for each value axis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CultureBias {
    pub martial: [f32; 2],
    pub tradition: [f32; 2],
    pub collectivism: [f32; 2],
    pub nature_harmony: [f32; 2],
    pub magic_acceptance: [f32; 2],
    pub xenophobia: [f32; 2],
    pub honor_value: [f32; 2],
    pub wealth: [f32; 2],
}

/// All culture-related data loaded from JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CultureBiasData {
    pub culture_biases: HashMap<String, CultureBias>,
    pub architecture_preferences: HashMap<String, Vec<String>>,
    pub gender_roles: HashMap<String, Vec<String>>,
    pub family_structure: HashMap<String, Vec<String>>,
    pub government_preferences: HashMap<String, Vec<String>>,
}

impl CultureBiasData {
    /// Get culture biases for a race tag, returning None if not found.
    pub fn bias_for(&self, tag: &str) -> Option<&CultureBias> {
        self.culture_biases.get(tag)
    }

    /// Get architecture preferences for a race tag, falling back to "_default".
    pub fn architecture_for(&self, tag: &str) -> &[String] {
        self.architecture_preferences.get(tag)
            .or_else(|| self.architecture_preferences.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get gender role options for a race tag, falling back to "_default".
    pub fn gender_roles_for(&self, tag: &str) -> &[String] {
        self.gender_roles.get(tag)
            .or_else(|| self.gender_roles.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get family structure options for a race tag, falling back to "_default".
    pub fn family_structure_for(&self, tag: &str) -> &[String] {
        self.family_structure.get(tag)
            .or_else(|| self.family_structure.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get government preferences for a race tag, falling back to "_default".
    pub fn government_for(&self, tag: &str) -> &[String] {
        self.government_preferences.get(tag)
            .or_else(|| self.government_preferences.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
