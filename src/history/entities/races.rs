//! Race type definitions and race generation.
//!
//! Fixed base race types provide familiar archetypes, while procedural
//! culture (in culture.rs) ensures each playthrough is unique.

use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::biomes::ExtendedBiome;
use crate::history::{RaceId, CultureId, NamingStyleId};
use crate::history::naming::styles::NamingArchetype;
use crate::history::entities::traits::Ability;

/// Base race types (fixed archetypes).
///
/// The built-in variants cover 13 classic fantasy races.
/// `Custom(String)` allows data-driven race definitions loaded from JSON.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RaceType {
    Human,
    Dwarf,
    Elf,
    Orc,
    Goblin,
    Halfling,
    Reptilian,
    Fey,
    Undead,
    Elemental,
    Beastfolk,
    Giant,
    Construct,
    /// User-defined race loaded from data files.
    Custom(String),
}

impl RaceType {
    pub fn all() -> &'static [RaceType] {
        &[
            RaceType::Human, RaceType::Dwarf, RaceType::Elf, RaceType::Orc,
            RaceType::Goblin, RaceType::Halfling, RaceType::Reptilian,
            RaceType::Fey, RaceType::Undead, RaceType::Elemental,
            RaceType::Beastfolk, RaceType::Giant, RaceType::Construct,
        ]
    }

    /// Get the string tag for this race type (used for data file lookups).
    pub fn tag(&self) -> &str {
        match self {
            RaceType::Human => "human",
            RaceType::Dwarf => "dwarf",
            RaceType::Elf => "elf",
            RaceType::Orc => "orc",
            RaceType::Goblin => "goblin",
            RaceType::Halfling => "halfling",
            RaceType::Reptilian => "reptilian",
            RaceType::Fey => "fey",
            RaceType::Undead => "undead",
            RaceType::Elemental => "elemental",
            RaceType::Beastfolk => "beastfolk",
            RaceType::Giant => "giant",
            RaceType::Construct => "construct",
            RaceType::Custom(s) => s.as_str(),
        }
    }

    /// Create a RaceType from a string tag.
    pub fn from_tag(tag: &str) -> Self {
        match tag {
            "human" => RaceType::Human,
            "dwarf" => RaceType::Dwarf,
            "elf" => RaceType::Elf,
            "orc" => RaceType::Orc,
            "goblin" => RaceType::Goblin,
            "halfling" => RaceType::Halfling,
            "reptilian" => RaceType::Reptilian,
            "fey" => RaceType::Fey,
            "undead" => RaceType::Undead,
            "elemental" => RaceType::Elemental,
            "beastfolk" => RaceType::Beastfolk,
            "giant" => RaceType::Giant,
            "construct" => RaceType::Construct,
            other => RaceType::Custom(other.to_string()),
        }
    }

    /// Default display name for the race type (plural).
    pub fn plural_name(&self) -> &str {
        match self {
            RaceType::Human => "Humans",
            RaceType::Dwarf => "Dwarves",
            RaceType::Elf => "Elves",
            RaceType::Orc => "Orcs",
            RaceType::Goblin => "Goblins",
            RaceType::Halfling => "Halflings",
            RaceType::Reptilian => "Reptilians",
            RaceType::Fey => "Fey",
            RaceType::Undead => "Undead",
            RaceType::Elemental => "Elementals",
            RaceType::Beastfolk => "Beastfolk",
            RaceType::Giant => "Giants",
            RaceType::Construct => "Constructs",
            RaceType::Custom(s) => s.as_str(),
        }
    }

    /// Naming archetype best suited for this race type.
    pub fn default_naming_archetype(&self) -> NamingArchetype {
        match self {
            RaceType::Human => NamingArchetype::Compound,
            RaceType::Dwarf => NamingArchetype::Harsh,
            RaceType::Elf => NamingArchetype::Flowing,
            RaceType::Orc => NamingArchetype::Guttural,
            RaceType::Goblin => NamingArchetype::Guttural,
            RaceType::Halfling => NamingArchetype::Compound,
            RaceType::Reptilian => NamingArchetype::Sibilant,
            RaceType::Fey => NamingArchetype::Mystical,
            RaceType::Undead => NamingArchetype::Ancient,
            RaceType::Elemental => NamingArchetype::Mystical,
            RaceType::Beastfolk => NamingArchetype::Harsh,
            RaceType::Giant => NamingArchetype::Ancient,
            RaceType::Construct => NamingArchetype::Ancient,
            RaceType::Custom(_) => NamingArchetype::Compound,
        }
    }

    /// Preferred biomes for settlement.
    pub fn preferred_biomes(&self) -> Vec<ExtendedBiome> {
        match self {
            RaceType::Human => vec![
                ExtendedBiome::TemperateGrassland, ExtendedBiome::TemperateForest,
                ExtendedBiome::Savanna, ExtendedBiome::Foothills,
            ],
            RaceType::Dwarf => vec![
                ExtendedBiome::AlpineTundra, ExtendedBiome::SnowyPeaks,
                ExtendedBiome::Foothills, ExtendedBiome::SubalpineForest,
            ],
            RaceType::Elf => vec![
                ExtendedBiome::TemperateForest, ExtendedBiome::TemperateRainforest,
                ExtendedBiome::BorealForest, ExtendedBiome::AncientGrove,
            ],
            RaceType::Orc => vec![
                ExtendedBiome::Savanna, ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Tundra, ExtendedBiome::Ashlands,
            ],
            RaceType::Goblin => vec![
                ExtendedBiome::Swamp, ExtendedBiome::Marsh,
                ExtendedBiome::DeadForest, ExtendedBiome::Foothills,
            ],
            RaceType::Halfling => vec![
                ExtendedBiome::TemperateGrassland, ExtendedBiome::TemperateForest,
                ExtendedBiome::Foothills,
            ],
            RaceType::Reptilian => vec![
                ExtendedBiome::TropicalForest, ExtendedBiome::Swamp,
                ExtendedBiome::Marsh, ExtendedBiome::Desert,
            ],
            RaceType::Fey => vec![
                ExtendedBiome::BioluminescentForest, ExtendedBiome::MushroomForest,
                ExtendedBiome::TemperateRainforest, ExtendedBiome::AncientGrove,
            ],
            RaceType::Undead => vec![
                ExtendedBiome::DeadForest, ExtendedBiome::Shadowfen,
                ExtendedBiome::Ashlands, ExtendedBiome::Bog,
            ],
            RaceType::Elemental => vec![
                ExtendedBiome::VolcanicWasteland, ExtendedBiome::CrystalWasteland,
                ExtendedBiome::Geysers, ExtendedBiome::LeyNexus,
            ],
            RaceType::Beastfolk => vec![
                ExtendedBiome::Savanna, ExtendedBiome::TemperateGrassland,
                ExtendedBiome::TropicalForest, ExtendedBiome::BorealForest,
            ],
            RaceType::Giant => vec![
                ExtendedBiome::SnowyPeaks, ExtendedBiome::AlpineTundra,
                ExtendedBiome::Tundra, ExtendedBiome::AlpineMeadow,
            ],
            RaceType::Construct => vec![
                ExtendedBiome::CrystalWasteland, ExtendedBiome::ObsidianFields,
                ExtendedBiome::BasaltColumns, ExtendedBiome::SaltFlats,
            ],
            RaceType::Custom(_) => vec![
                ExtendedBiome::TemperateGrassland, ExtendedBiome::TemperateForest,
            ],
        }
    }

    /// Lifespan range in years (min, max).
    pub fn lifespan(&self) -> (u32, u32) {
        match self {
            RaceType::Human => (60, 90),
            RaceType::Dwarf => (200, 400),
            RaceType::Elf => (500, 1000),
            RaceType::Orc => (40, 70),
            RaceType::Goblin => (30, 60),
            RaceType::Halfling => (80, 130),
            RaceType::Reptilian => (80, 150),
            RaceType::Fey => (300, 800),
            RaceType::Undead => (0, 0),       // Immortal (doesn't die of age)
            RaceType::Elemental => (0, 0),    // Immortal
            RaceType::Beastfolk => (50, 80),
            RaceType::Giant => (300, 600),
            RaceType::Construct => (0, 0),    // Immortal
            RaceType::Custom(_) => (60, 100),
        }
    }

    /// Age of maturity in years.
    pub fn maturity_age(&self) -> u32 {
        match self {
            RaceType::Human => 16,
            RaceType::Dwarf => 40,
            RaceType::Elf => 80,
            RaceType::Orc => 12,
            RaceType::Goblin => 8,
            RaceType::Halfling => 20,
            RaceType::Reptilian => 14,
            RaceType::Fey => 50,
            RaceType::Undead => 0,
            RaceType::Elemental => 0,
            RaceType::Beastfolk => 14,
            RaceType::Giant => 60,
            RaceType::Construct => 0,
            RaceType::Custom(_) => 16,
        }
    }

    /// Innate racial abilities.
    pub fn innate_abilities(&self) -> Vec<Ability> {
        match self {
            RaceType::Human => vec![],
            RaceType::Dwarf => vec![Ability::DarkVision, Ability::StoneAffinity, Ability::PoisonResistance],
            RaceType::Elf => vec![Ability::Longevity, Ability::DarkVision],
            RaceType::Orc => vec![Ability::BattleRage, Ability::NaturalArmor],
            RaceType::Goblin => vec![Ability::DarkVision],
            RaceType::Halfling => vec![],
            RaceType::Reptilian => vec![Ability::NaturalArmor, Ability::WaterBreathing, Ability::HeatResistance],
            RaceType::Fey => vec![Ability::ArcaneGift, Ability::Shapeshifter],
            RaceType::Undead => vec![Ability::DarkVision, Ability::PoisonResistance, Ability::ColdResistance],
            RaceType::Elemental => vec![Ability::Regeneration],
            RaceType::Beastfolk => vec![Ability::BeastSpeaker],
            RaceType::Giant => vec![Ability::MountainEndurance, Ability::NaturalArmor],
            RaceType::Construct => vec![Ability::PoisonResistance, Ability::ColdResistance, Ability::HeatResistance],
            RaceType::Custom(_) => vec![],
        }
    }

    /// Whether this race type can reproduce naturally (for dynasty/lineage).
    pub fn can_reproduce(&self) -> bool {
        !matches!(self, RaceType::Undead | RaceType::Construct | RaceType::Elemental)
    }

    /// Check whether this is a Custom race.
    pub fn is_custom(&self) -> bool {
        matches!(self, RaceType::Custom(_))
    }
}

/// A complete race definition (base type + culture).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Race {
    pub id: RaceId,
    pub base_type: RaceType,
    pub culture_id: CultureId,
    pub name: String,

    pub lifespan: (u32, u32),
    pub maturity_age: u32,

    pub preferred_biomes: Vec<ExtendedBiome>,
    pub innate_abilities: Vec<Ability>,
}

impl Race {
    /// Create a new race with auto-populated fields from the base type.
    pub fn new(
        id: RaceId,
        base_type: RaceType,
        culture_id: CultureId,
        name: String,
    ) -> Self {
        let lifespan = base_type.lifespan();
        let maturity_age = base_type.maturity_age();
        let preferred_biomes = base_type.preferred_biomes();
        let innate_abilities = base_type.innate_abilities();
        Self {
            id,
            base_type,
            culture_id,
            name,
            lifespan,
            maturity_age,
            preferred_biomes,
            innate_abilities,
        }
    }

    /// Check if a given biome is suitable for this race.
    pub fn likes_biome(&self, biome: &ExtendedBiome) -> bool {
        self.preferred_biomes.contains(biome)
    }

    /// Whether this race has natural lifespans (not immortal).
    pub fn is_mortal(&self) -> bool {
        self.lifespan.1 > 0
    }

    /// Generate a random lifespan for an individual of this race.
    pub fn random_lifespan(&self, rng: &mut impl Rng) -> Option<u32> {
        if self.lifespan.1 == 0 {
            None // Immortal
        } else {
            Some(rng.gen_range(self.lifespan.0..=self.lifespan.1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_race_types() {
        assert_eq!(RaceType::all().len(), 13);
    }

    #[test]
    fn test_race_creation() {
        let race = Race::new(
            RaceId(0),
            RaceType::Dwarf,
            CultureId(0),
            "Irondelve Dwarves".to_string(),
        );
        assert_eq!(race.base_type, RaceType::Dwarf);
        assert!(race.lifespan.0 > 100);
        assert!(race.likes_biome(&ExtendedBiome::SnowyPeaks));
        assert!(!race.likes_biome(&ExtendedBiome::TropicalRainforest));
        assert!(race.is_mortal());
    }

    #[test]
    fn test_immortal_races() {
        let race = Race::new(
            RaceId(0),
            RaceType::Undead,
            CultureId(0),
            "The Risen".to_string(),
        );
        assert!(!race.is_mortal());
    }

    #[test]
    fn test_preferred_biomes_not_empty() {
        for rt in RaceType::all() {
            assert!(
                !rt.preferred_biomes().is_empty(),
                "{:?} should have preferred biomes", rt
            );
        }
    }
}
