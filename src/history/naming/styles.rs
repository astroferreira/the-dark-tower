//! Naming style definitions.
//!
//! A `NamingStyle` captures the phonetic and structural traits of a culture's naming
//! conventions. Pre-built archetypes provide starting points for different race types.

use serde::{Serialize, Deserialize};
use crate::history::NamingStyleId;

/// Phonetic and structural traits for name generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamingStyle {
    pub id: NamingStyleId,

    // --- Sound preferences ---
    /// Consonant clusters that appear at the start of syllables.
    pub onset_consonants: Vec<String>,
    /// Consonant clusters that appear at the end of syllables.
    pub coda_consonants: Vec<String>,
    /// Vowel sounds (may include diphthongs like "ae", "ou").
    pub vowels: Vec<String>,

    // --- Structure ---
    /// Min and max syllable count for personal names.
    pub syllable_range: (u8, u8),
    /// Whether names can use apostrophes as breaks ("D'kari").
    pub uses_apostrophes: bool,
    /// Whether names can use hyphens ("Krath-Morul").
    pub uses_hyphens: bool,

    // --- Affixes ---
    /// Common prefixes for place names.
    pub place_prefixes: Vec<String>,
    /// Common suffixes for place names.
    pub place_suffixes: Vec<String>,
    /// Common title patterns for epithets.
    pub epithet_patterns: Vec<String>,
}

/// Pre-built naming archetypes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamingArchetype {
    /// Harsh, consonant-heavy (Dwarven): "Krath", "Durnok", "Grimjaw"
    Harsh,
    /// Flowing, vowel-rich (Elven): "Aelindra", "Thalion", "Elowen"
    Flowing,
    /// Compound, earthy (Human): "Blackstone", "Aldric", "Thornwall"
    Compound,
    /// Guttural, aggressive (Orcish): "Grukash", "Borzag", "Vrakk"
    Guttural,
    /// Mystical, ethereal (Fey): "Lyriel", "Thessan", "Whisperwind"
    Mystical,
    /// Sibilant, reptilian: "Ssithak", "Xalith", "Zekora"
    Sibilant,
    /// Ancient, ponderous (Giant/Construct): "Uthgard", "Kronmor", "Basalthem"
    Ancient,
}

impl NamingArchetype {
    /// All available archetypes.
    pub fn all() -> &'static [NamingArchetype] {
        &[
            NamingArchetype::Harsh,
            NamingArchetype::Flowing,
            NamingArchetype::Compound,
            NamingArchetype::Guttural,
            NamingArchetype::Mystical,
            NamingArchetype::Sibilant,
            NamingArchetype::Ancient,
        ]
    }
}

impl NamingStyle {
    /// Create a naming style from a pre-built archetype.
    pub fn from_archetype(id: NamingStyleId, archetype: NamingArchetype) -> Self {
        match archetype {
            NamingArchetype::Harsh => Self::harsh(id),
            NamingArchetype::Flowing => Self::flowing(id),
            NamingArchetype::Compound => Self::compound(id),
            NamingArchetype::Guttural => Self::guttural(id),
            NamingArchetype::Mystical => Self::mystical(id),
            NamingArchetype::Sibilant => Self::sibilant(id),
            NamingArchetype::Ancient => Self::ancient(id),
        }
    }

    fn harsh(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "k", "kr", "d", "dr", "g", "gr", "th", "b", "br", "n", "m",
                "t", "tr", "v", "st", "sk",
            ]),
            coda_consonants: strs(&[
                "k", "rk", "th", "n", "m", "r", "rd", "ng", "lk", "ld", "x",
            ]),
            vowels: strs(&["a", "o", "u", "i", "e", "ur", "or"]),
            syllable_range: (1, 3),
            uses_apostrophes: false,
            uses_hyphens: true,
            place_prefixes: strs(&["Iron", "Black", "Bitter", "Stone", "Dark", "Deep"]),
            place_suffixes: strs(&["hold", "forge", "delve", "helm", "guard", "hall", "gate"]),
            epithet_patterns: strs(&[
                "the Unyielding", "Ironhand", "Stoneheart", "the Grim",
                "Hammerfist", "the Merciless",
            ]),
        }
    }

    fn flowing(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "l", "th", "s", "n", "r", "f", "v", "el", "al", "gl", "br",
                "m", "c", "t",
            ]),
            coda_consonants: strs(&[
                "n", "l", "r", "s", "th", "nd", "ll", "rn", "wen", "iel",
            ]),
            vowels: strs(&["ae", "a", "e", "i", "o", "ei", "ia", "io", "ea"]),
            syllable_range: (2, 4),
            uses_apostrophes: false,
            uses_hyphens: false,
            place_prefixes: strs(&["Sil", "Lor", "Thal", "Cel", "Ael", "Gal"]),
            place_suffixes: strs(&["wen", "oth", "dor", "ion", "iel", "ost", "anor"]),
            epithet_patterns: strs(&[
                "the Radiant", "Starweaver", "the Evergreen", "Dawnbringer",
                "the Ageless", "Moonwhisper",
            ]),
        }
    }

    fn compound(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "b", "d", "g", "h", "l", "m", "n", "r", "s", "t", "w", "j",
                "f", "p", "c",
            ]),
            coda_consonants: strs(&[
                "n", "d", "r", "l", "s", "t", "ld", "rd", "nd", "ck",
            ]),
            vowels: strs(&["a", "e", "i", "o", "u", "ay", "ow"]),
            syllable_range: (1, 3),
            uses_apostrophes: false,
            uses_hyphens: false,
            place_prefixes: strs(&[
                "North", "South", "East", "West", "Red", "White", "Green",
                "High", "Low", "Old",
            ]),
            place_suffixes: strs(&[
                "ton", "burg", "dale", "ford", "wick", "field", "bridge",
                "stead", "haven", "mere",
            ]),
            epithet_patterns: strs(&[
                "the Bold", "the Wise", "the Brave", "the Just",
                "the Conqueror", "the Peacemaker",
            ]),
        }
    }

    fn guttural(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "gr", "kr", "g", "z", "b", "dr", "sk", "gh", "v", "r",
                "hr", "sn", "gn",
            ]),
            coda_consonants: strs(&[
                "k", "g", "gh", "rk", "zz", "sh", "rg", "gk", "kh", "x",
            ]),
            vowels: strs(&["a", "u", "o", "aa", "uu"]),
            syllable_range: (1, 3),
            uses_apostrophes: false,
            uses_hyphens: false,
            place_prefixes: strs(&["Blood", "Skull", "War", "Bone", "Rot", "Ash"]),
            place_suffixes: strs(&["maw", "pit", "gore", "fang", "crush", "break"]),
            epithet_patterns: strs(&[
                "the Destroyer", "Skullcrusher", "Bonegnawer", "the Savage",
                "Blooddrinker", "the Dread",
            ]),
        }
    }

    fn mystical(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "l", "n", "s", "w", "f", "th", "sh", "wh", "ph", "m",
                "r", "v",
            ]),
            coda_consonants: strs(&[
                "ss", "th", "n", "l", "r", "ll", "nn", "sh",
            ]),
            vowels: strs(&["i", "y", "e", "a", "ie", "ea", "ai"]),
            syllable_range: (2, 4),
            uses_apostrophes: true,
            uses_hyphens: false,
            place_prefixes: strs(&["Whisper", "Mist", "Dream", "Shimmer", "Moon", "Star"]),
            place_suffixes: strs(&["wind", "vale", "mere", "glade", "song", "light"]),
            epithet_patterns: strs(&[
                "the Dreaming", "Mistwalker", "the Fey-touched", "Glamourweave",
                "the Changeling", "Starborn",
            ]),
        }
    }

    fn sibilant(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "ss", "s", "z", "x", "sh", "th", "ts", "sk", "sl",
                "zh", "ks",
            ]),
            coda_consonants: strs(&[
                "ss", "th", "x", "k", "sh", "z", "sk", "ks",
            ]),
            vowels: strs(&["i", "a", "e", "o", "ai", "ei"]),
            syllable_range: (2, 3),
            uses_apostrophes: true,
            uses_hyphens: false,
            place_prefixes: strs(&["Scale", "Fang", "Venom", "Sand", "Sun", "Salt"]),
            place_suffixes: strs(&["spire", "nest", "coil", "den", "rock", "marsh"]),
            epithet_patterns: strs(&[
                "the Venomous", "Scaleborn", "the Cold-blooded", "Sandstrider",
                "Sunbasker", "the Scaled",
            ]),
        }
    }

    fn ancient(id: NamingStyleId) -> Self {
        Self {
            id,
            onset_consonants: strs(&[
                "kr", "b", "g", "th", "m", "d", "n", "r", "st",
                "br", "tr",
            ]),
            coda_consonants: strs(&[
                "rn", "rd", "th", "m", "n", "r", "ld", "nd", "lm",
            ]),
            vowels: strs(&["o", "u", "a", "au", "ou", "oo"]),
            syllable_range: (2, 3),
            uses_apostrophes: false,
            uses_hyphens: true,
            place_prefixes: strs(&["Grand", "Titan", "Elder", "Basalt", "Thunder", "Crown"]),
            place_suffixes: strs(&["mount", "spire", "throne", "cairn", "monolith", "keep"]),
            epithet_patterns: strs(&[
                "the Eternal", "Worldshaker", "the Colossal", "Mountainborn",
                "the Undying", "Stormfather",
            ]),
        }
    }
}

/// Helper to convert &[&str] to Vec<String>.
fn strs(slice: &[&str]) -> Vec<String> {
    slice.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_archetypes_create() {
        for archetype in NamingArchetype::all() {
            let style = NamingStyle::from_archetype(NamingStyleId(0), *archetype);
            assert!(!style.onset_consonants.is_empty());
            assert!(!style.coda_consonants.is_empty());
            assert!(!style.vowels.is_empty());
            assert!(style.syllable_range.0 <= style.syllable_range.1);
            assert!(!style.place_prefixes.is_empty());
            assert!(!style.place_suffixes.is_empty());
            assert!(!style.epithet_patterns.is_empty());
        }
    }
}
