//! Data-driven game data registry.
//!
//! Loads races, naming styles, backstory templates, and culture biases
//! from JSON data files. Defaults are embedded in the binary via `include_str!`;
//! an optional `data/` directory next to the binary can override them.

pub mod race_data;
pub mod naming_data;
pub mod backstory_data;
pub mod culture_data;

use std::collections::HashMap;
use std::path::Path;

pub use race_data::{RaceTemplate, RacesFile};
pub use naming_data::{NamingStyleTemplate, NamingStylesFile};
pub use backstory_data::BackstoryTemplates;
pub use culture_data::CultureBiasData;

// Embedded default data files
const DEFAULT_RACES_JSON: &str = include_str!("../../../data/defaults/races.json");
const DEFAULT_NAMING_STYLES_JSON: &str = include_str!("../../../data/defaults/naming_styles.json");
const DEFAULT_BACKSTORY_JSON: &str = include_str!("../../../data/defaults/backstory.json");
const DEFAULT_CULTURE_BIASES_JSON: &str = include_str!("../../../data/defaults/culture_biases.json");

/// Read-only game data registry, loaded once at startup.
///
/// NOT stored in save files — only the runtime `WorldHistory` is serialized.
#[derive(Clone, Debug)]
pub struct GameData {
    /// Race definitions keyed by tag (e.g. "dwarf", "elf").
    pub races: HashMap<String, RaceTemplate>,
    /// Ordered list of race tags (for iteration in deterministic order).
    pub race_tags: Vec<String>,
    /// Naming style definitions keyed by archetype name (e.g. "Harsh", "Flowing").
    pub naming_styles: HashMap<String, NamingStyleTemplate>,
    /// Backstory templates (epithets, titles, coronation/death/reign events, etc.).
    pub backstory: BackstoryTemplates,
    /// Culture bias data (value biases, architecture, gender roles, etc.).
    pub culture_biases: CultureBiasData,
}

impl GameData {
    /// Load from embedded defaults compiled into the binary.
    pub fn defaults() -> Self {
        let races_file: RacesFile = serde_json::from_str(DEFAULT_RACES_JSON)
            .expect("Failed to parse embedded races.json");
        let naming_file: NamingStylesFile = serde_json::from_str(DEFAULT_NAMING_STYLES_JSON)
            .expect("Failed to parse embedded naming_styles.json");
        let backstory: BackstoryTemplates = serde_json::from_str(DEFAULT_BACKSTORY_JSON)
            .expect("Failed to parse embedded backstory.json");
        let culture_biases: CultureBiasData = serde_json::from_str(DEFAULT_CULTURE_BIASES_JSON)
            .expect("Failed to parse embedded culture_biases.json");

        let mut races = HashMap::new();
        let mut race_tags = Vec::new();
        for race in races_file.races {
            race_tags.push(race.tag.clone());
            races.insert(race.tag.clone(), race);
        }

        Self {
            races,
            race_tags,
            naming_styles: naming_file.naming_styles,
            backstory,
            culture_biases,
        }
    }

    /// Load from a directory, merging with embedded defaults.
    ///
    /// Files in the directory override the corresponding default data.
    /// Missing files fall back to defaults.
    pub fn load_from(dir: &Path) -> Self {
        let mut data = Self::defaults();

        // Override races if file exists
        let races_path = dir.join("races.json");
        if races_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&races_path) {
                if let Ok(races_file) = serde_json::from_str::<RacesFile>(&contents) {
                    // Merge: add new races, override existing ones
                    for race in races_file.races {
                        if !data.races.contains_key(&race.tag) {
                            data.race_tags.push(race.tag.clone());
                        }
                        data.races.insert(race.tag.clone(), race);
                    }
                } else {
                    eprintln!("Warning: failed to parse {}", races_path.display());
                }
            }
        }

        // Override naming styles if file exists
        let naming_path = dir.join("naming_styles.json");
        if naming_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&naming_path) {
                if let Ok(naming_file) = serde_json::from_str::<NamingStylesFile>(&contents) {
                    for (key, style) in naming_file.naming_styles {
                        data.naming_styles.insert(key, style);
                    }
                } else {
                    eprintln!("Warning: failed to parse {}", naming_path.display());
                }
            }
        }

        // Override backstory if file exists
        let backstory_path = dir.join("backstory.json");
        if backstory_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&backstory_path) {
                if let Ok(backstory) = serde_json::from_str::<BackstoryTemplates>(&contents) {
                    data.backstory = backstory;
                } else {
                    eprintln!("Warning: failed to parse {}", backstory_path.display());
                }
            }
        }

        // Override culture biases if file exists
        let culture_path = dir.join("culture_biases.json");
        if culture_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&culture_path) {
                if let Ok(culture) = serde_json::from_str::<CultureBiasData>(&contents) {
                    data.culture_biases = culture;
                } else {
                    eprintln!("Warning: failed to parse {}", culture_path.display());
                }
            }
        }

        data
    }

    /// Get a race template by tag.
    pub fn race(&self, tag: &str) -> Option<&RaceTemplate> {
        self.races.get(tag)
    }

    /// Get a naming style template by archetype name.
    pub fn naming_style(&self, name: &str) -> Option<&NamingStyleTemplate> {
        self.naming_styles.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_load() {
        let data = GameData::defaults();
        assert_eq!(data.races.len(), 13);
        assert_eq!(data.race_tags.len(), 13);
        assert_eq!(data.naming_styles.len(), 17);
        assert!(!data.backstory.common_epithets.is_empty());
        assert!(!data.culture_biases.culture_biases.is_empty());
    }

    #[test]
    fn test_race_lookup() {
        let data = GameData::defaults();
        let dwarf = data.race("dwarf").unwrap();
        assert_eq!(dwarf.plural_name, "Dwarves");
        assert_eq!(dwarf.naming_archetype, "Harsh");
        assert!(dwarf.can_reproduce);
    }

    #[test]
    fn test_naming_style_lookup() {
        let data = GameData::defaults();
        let harsh = data.naming_style("Harsh").unwrap();
        assert!(!harsh.onset_consonants.is_empty());
        assert_eq!(harsh.syllable_range, [1, 3]);
    }

    #[test]
    fn test_backstory_helpers() {
        let data = GameData::defaults();
        let titles = data.backstory.ruler_titles_for("dwarf");
        assert!(titles.contains(&"Thane".to_string()));

        let default_titles = data.backstory.ruler_titles_for("nonexistent_race");
        assert!(!default_titles.is_empty());
    }

    #[test]
    fn test_culture_bias_lookup() {
        let data = GameData::defaults();
        let bias = data.culture_biases.bias_for("dwarf").unwrap();
        assert!(bias.tradition[0] > 0.7); // Dwarves are traditional
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        // Should fall back to defaults without panicking
        let data = GameData::load_from(Path::new("/nonexistent/path"));
        assert_eq!(data.races.len(), 13);
    }
}
