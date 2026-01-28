//! Procedurally generated culture system.
//!
//! Each culture has values, aesthetic preferences, social structure,
//! and religious tendencies that drive faction behavior.

use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::history::{CultureId, NamingStyleId};

/// Cultural values (all 0.0 to 1.0).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CultureValues {
    /// Martial vs peaceful (high = warlike).
    pub martial: f32,
    /// Tradition vs innovation (high = traditional).
    pub tradition: f32,
    /// Community vs individual (high = collectivist).
    pub collectivism: f32,
    /// Domination vs harmony with nature (high = nature-loving).
    pub nature_harmony: f32,
    /// Acceptance of magic (high = magic-embracing).
    pub magic_acceptance: f32,
    /// Distrust of outsiders (high = xenophobic).
    pub xenophobia: f32,
    /// Importance of personal honor (high = honor-bound).
    pub honor_value: f32,
    /// Importance of wealth and trade (high = mercantile).
    pub wealth: f32,
}

impl CultureValues {
    /// Generate random culture values.
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            martial: rng.gen(),
            tradition: rng.gen(),
            collectivism: rng.gen(),
            nature_harmony: rng.gen(),
            magic_acceptance: rng.gen(),
            xenophobia: rng.gen(),
            honor_value: rng.gen(),
            wealth: rng.gen(),
        }
    }

    /// Generate culture values biased toward a race archetype.
    pub fn for_race(race_type: &super::races::RaceType, rng: &mut impl Rng) -> Self {
        use super::races::RaceType;

        fn bias(center: f32, spread: f32, rng: &mut impl Rng) -> f32 {
            let noise: f32 = rng.gen_range(-spread..=spread);
            (center + noise).clamp(0.0, 1.0)
        }

        match race_type {
            RaceType::Human => Self {
                martial: bias(0.5, 0.3, rng),
                tradition: bias(0.5, 0.3, rng),
                collectivism: bias(0.5, 0.3, rng),
                nature_harmony: bias(0.4, 0.3, rng),
                magic_acceptance: bias(0.5, 0.3, rng),
                xenophobia: bias(0.4, 0.3, rng),
                honor_value: bias(0.5, 0.3, rng),
                wealth: bias(0.6, 0.3, rng),
            },
            RaceType::Dwarf => Self {
                martial: bias(0.6, 0.2, rng),
                tradition: bias(0.8, 0.15, rng),
                collectivism: bias(0.7, 0.2, rng),
                nature_harmony: bias(0.2, 0.2, rng),
                magic_acceptance: bias(0.3, 0.2, rng),
                xenophobia: bias(0.6, 0.2, rng),
                honor_value: bias(0.7, 0.2, rng),
                wealth: bias(0.7, 0.2, rng),
            },
            RaceType::Elf => Self {
                martial: bias(0.3, 0.2, rng),
                tradition: bias(0.7, 0.2, rng),
                collectivism: bias(0.5, 0.2, rng),
                nature_harmony: bias(0.8, 0.15, rng),
                magic_acceptance: bias(0.8, 0.15, rng),
                xenophobia: bias(0.5, 0.25, rng),
                honor_value: bias(0.6, 0.2, rng),
                wealth: bias(0.3, 0.2, rng),
            },
            RaceType::Orc => Self {
                martial: bias(0.8, 0.15, rng),
                tradition: bias(0.6, 0.2, rng),
                collectivism: bias(0.6, 0.2, rng),
                nature_harmony: bias(0.3, 0.2, rng),
                magic_acceptance: bias(0.3, 0.25, rng),
                xenophobia: bias(0.7, 0.2, rng),
                honor_value: bias(0.5, 0.3, rng),
                wealth: bias(0.4, 0.2, rng),
            },
            RaceType::Goblin => Self {
                martial: bias(0.5, 0.3, rng),
                tradition: bias(0.3, 0.2, rng),
                collectivism: bias(0.6, 0.2, rng),
                nature_harmony: bias(0.3, 0.2, rng),
                magic_acceptance: bias(0.5, 0.3, rng),
                xenophobia: bias(0.5, 0.3, rng),
                honor_value: bias(0.2, 0.2, rng),
                wealth: bias(0.7, 0.2, rng),
            },
            RaceType::Halfling => Self {
                martial: bias(0.2, 0.2, rng),
                tradition: bias(0.6, 0.2, rng),
                collectivism: bias(0.7, 0.2, rng),
                nature_harmony: bias(0.6, 0.2, rng),
                magic_acceptance: bias(0.4, 0.2, rng),
                xenophobia: bias(0.3, 0.2, rng),
                honor_value: bias(0.5, 0.2, rng),
                wealth: bias(0.5, 0.2, rng),
            },
            RaceType::Reptilian => Self {
                martial: bias(0.6, 0.2, rng),
                tradition: bias(0.7, 0.2, rng),
                collectivism: bias(0.6, 0.2, rng),
                nature_harmony: bias(0.5, 0.2, rng),
                magic_acceptance: bias(0.4, 0.25, rng),
                xenophobia: bias(0.7, 0.2, rng),
                honor_value: bias(0.5, 0.2, rng),
                wealth: bias(0.4, 0.2, rng),
            },
            RaceType::Fey => Self {
                martial: bias(0.2, 0.2, rng),
                tradition: bias(0.4, 0.3, rng),
                collectivism: bias(0.4, 0.3, rng),
                nature_harmony: bias(0.9, 0.1, rng),
                magic_acceptance: bias(0.9, 0.1, rng),
                xenophobia: bias(0.5, 0.3, rng),
                honor_value: bias(0.3, 0.3, rng),
                wealth: bias(0.2, 0.2, rng),
            },
            RaceType::Undead => Self {
                martial: bias(0.6, 0.2, rng),
                tradition: bias(0.5, 0.3, rng),
                collectivism: bias(0.4, 0.3, rng),
                nature_harmony: bias(0.1, 0.1, rng),
                magic_acceptance: bias(0.9, 0.1, rng),
                xenophobia: bias(0.8, 0.15, rng),
                honor_value: bias(0.3, 0.2, rng),
                wealth: bias(0.5, 0.3, rng),
            },
            RaceType::Elemental => Self {
                martial: bias(0.5, 0.3, rng),
                tradition: bias(0.5, 0.3, rng),
                collectivism: bias(0.5, 0.3, rng),
                nature_harmony: bias(0.7, 0.2, rng),
                magic_acceptance: bias(0.8, 0.15, rng),
                xenophobia: bias(0.4, 0.3, rng),
                honor_value: bias(0.5, 0.3, rng),
                wealth: bias(0.3, 0.2, rng),
            },
            RaceType::Beastfolk => Self {
                martial: bias(0.6, 0.2, rng),
                tradition: bias(0.6, 0.2, rng),
                collectivism: bias(0.7, 0.2, rng),
                nature_harmony: bias(0.7, 0.2, rng),
                magic_acceptance: bias(0.4, 0.25, rng),
                xenophobia: bias(0.5, 0.25, rng),
                honor_value: bias(0.5, 0.25, rng),
                wealth: bias(0.3, 0.2, rng),
            },
            RaceType::Giant => Self {
                martial: bias(0.7, 0.2, rng),
                tradition: bias(0.8, 0.15, rng),
                collectivism: bias(0.4, 0.2, rng),
                nature_harmony: bias(0.4, 0.2, rng),
                magic_acceptance: bias(0.5, 0.25, rng),
                xenophobia: bias(0.6, 0.2, rng),
                honor_value: bias(0.6, 0.2, rng),
                wealth: bias(0.3, 0.2, rng),
            },
            RaceType::Construct => Self {
                martial: bias(0.5, 0.3, rng),
                tradition: bias(0.8, 0.1, rng),
                collectivism: bias(0.8, 0.1, rng),
                nature_harmony: bias(0.2, 0.2, rng),
                magic_acceptance: bias(0.7, 0.2, rng),
                xenophobia: bias(0.5, 0.3, rng),
                honor_value: bias(0.6, 0.2, rng),
                wealth: bias(0.3, 0.2, rng),
            },
            RaceType::Custom(_) => Self::random(rng),
        }
    }

    /// Generate culture values from GameData biases.
    /// Falls back to `for_race` if no data-driven bias is found.
    pub fn for_race_with_data(
        race_type: &super::races::RaceType,
        game_data: &crate::history::data::GameData,
        rng: &mut impl Rng,
    ) -> Self {
        let tag = race_type.tag();
        if let Some(bias_data) = game_data.culture_biases.bias_for(tag) {
            fn bias(center: f32, spread: f32, rng: &mut impl Rng) -> f32 {
                let noise: f32 = rng.gen_range(-spread..=spread);
                (center + noise).clamp(0.0, 1.0)
            }
            Self {
                martial: bias(bias_data.martial[0], bias_data.martial[1], rng),
                tradition: bias(bias_data.tradition[0], bias_data.tradition[1], rng),
                collectivism: bias(bias_data.collectivism[0], bias_data.collectivism[1], rng),
                nature_harmony: bias(bias_data.nature_harmony[0], bias_data.nature_harmony[1], rng),
                magic_acceptance: bias(bias_data.magic_acceptance[0], bias_data.magic_acceptance[1], rng),
                xenophobia: bias(bias_data.xenophobia[0], bias_data.xenophobia[1], rng),
                honor_value: bias(bias_data.honor_value[0], bias_data.honor_value[1], rng),
                wealth: bias(bias_data.wealth[0], bias_data.wealth[1], rng),
            }
        } else {
            Self::for_race(race_type, rng)
        }
    }

    /// Cultural similarity score between two cultures (0.0 = opposite, 1.0 = identical).
    pub fn similarity(&self, other: &CultureValues) -> f32 {
        let diffs = [
            (self.martial - other.martial).abs(),
            (self.tradition - other.tradition).abs(),
            (self.collectivism - other.collectivism).abs(),
            (self.nature_harmony - other.nature_harmony).abs(),
            (self.magic_acceptance - other.magic_acceptance).abs(),
            (self.xenophobia - other.xenophobia).abs(),
            (self.honor_value - other.honor_value).abs(),
            (self.wealth - other.wealth).abs(),
        ];
        let avg_diff: f32 = diffs.iter().sum::<f32>() / diffs.len() as f32;
        1.0 - avg_diff
    }
}

/// Government types for civilizations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GovernmentType {
    Monarchy,
    Theocracy,
    Republic,
    Oligarchy,
    TribalCouncil,
    Dictatorship,
    Magocracy,
    HiveCollective,
}

/// Architectural style preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchitectureStyle {
    Stone,
    Wood,
    Crystal,
    Bone,
    Earthen,
    Living,     // Grown from plants
    Metalwork,
    Carved,     // Carved into rock
    Woven,      // Woven structures
}

/// Art motifs used in decorations and monuments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtMotif {
    Geometric,
    Naturalistic,
    Abstract,
    Mythological,
    Historical,
    Celestial,
    Bestial,
    Runic,
}

/// Gender role configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GenderRoles {
    Egalitarian,
    MaleLeadership,
    FemaleLeadership,
    MeritBased,
}

/// Family structure type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FamilyStructure {
    Nuclear,
    Extended,
    Clan,
    Communal,
}

/// A complete procedurally generated culture.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Culture {
    pub id: CultureId,
    pub name: String,
    pub naming_style: NamingStyleId,

    pub values: CultureValues,

    pub architecture: ArchitectureStyle,
    pub art_motifs: Vec<ArtMotif>,

    pub government_preference: GovernmentType,
    pub gender_roles: GenderRoles,
    pub family_structure: FamilyStructure,

    /// 0.0 = secular, 1.0 = theocratic
    pub religiosity: f32,
    /// Likelihood to worship local legendary creatures as gods.
    pub monster_worship_tendency: f32,
}

impl Culture {
    /// Generate a culture for a given race type.
    pub fn generate(
        id: CultureId,
        name: String,
        naming_style: NamingStyleId,
        race_type: &super::races::RaceType,
        rng: &mut impl Rng,
    ) -> Self {
        Self::generate_with_data(id, name, naming_style, race_type, None, rng)
    }

    pub fn generate_with_data(
        id: CultureId,
        name: String,
        naming_style: NamingStyleId,
        race_type: &super::races::RaceType,
        game_data: Option<&crate::history::data::GameData>,
        rng: &mut impl Rng,
    ) -> Self {
        use super::races::RaceType;

        let values = if let Some(gd) = game_data {
            CultureValues::for_race_with_data(race_type, gd, rng)
        } else {
            CultureValues::for_race(race_type, rng)
        };

        let architecture = match race_type {
            RaceType::Dwarf => *pick(rng, &[ArchitectureStyle::Stone, ArchitectureStyle::Carved, ArchitectureStyle::Metalwork]),
            RaceType::Elf => *pick(rng, &[ArchitectureStyle::Living, ArchitectureStyle::Wood, ArchitectureStyle::Crystal]),
            RaceType::Orc => *pick(rng, &[ArchitectureStyle::Bone, ArchitectureStyle::Stone, ArchitectureStyle::Earthen]),
            RaceType::Goblin => *pick(rng, &[ArchitectureStyle::Earthen, ArchitectureStyle::Woven, ArchitectureStyle::Bone]),
            RaceType::Fey => *pick(rng, &[ArchitectureStyle::Living, ArchitectureStyle::Crystal, ArchitectureStyle::Woven]),
            RaceType::Undead => *pick(rng, &[ArchitectureStyle::Bone, ArchitectureStyle::Stone, ArchitectureStyle::Carved]),
            RaceType::Reptilian => *pick(rng, &[ArchitectureStyle::Stone, ArchitectureStyle::Earthen, ArchitectureStyle::Carved]),
            RaceType::Giant => *pick(rng, &[ArchitectureStyle::Stone, ArchitectureStyle::Carved]),
            RaceType::Construct => *pick(rng, &[ArchitectureStyle::Metalwork, ArchitectureStyle::Stone, ArchitectureStyle::Crystal]),
            _ => *pick(rng, &[ArchitectureStyle::Stone, ArchitectureStyle::Wood, ArchitectureStyle::Earthen]),
        };

        let all_motifs = [
            ArtMotif::Geometric, ArtMotif::Naturalistic, ArtMotif::Abstract,
            ArtMotif::Mythological, ArtMotif::Historical, ArtMotif::Celestial,
            ArtMotif::Bestial, ArtMotif::Runic,
        ];
        let motif_count = rng.gen_range(1..=3);
        let mut art_motifs = Vec::new();
        for _ in 0..motif_count {
            let m = *pick(rng, &all_motifs);
            if !art_motifs.contains(&m) {
                art_motifs.push(m);
            }
        }

        let government_preference = if values.magic_acceptance > 0.8 && rng.gen_bool(0.3) {
            GovernmentType::Magocracy
        } else if values.tradition > 0.7 && values.collectivism > 0.6 {
            *pick(rng, &[GovernmentType::Monarchy, GovernmentType::Theocracy, GovernmentType::TribalCouncil])
        } else if values.collectivism > 0.7 {
            *pick(rng, &[GovernmentType::Republic, GovernmentType::TribalCouncil])
        } else if values.martial > 0.7 {
            *pick(rng, &[GovernmentType::Dictatorship, GovernmentType::Monarchy])
        } else {
            *pick(rng, &[GovernmentType::Monarchy, GovernmentType::Oligarchy, GovernmentType::Republic])
        };

        let gender_roles = match race_type {
            RaceType::Elf | RaceType::Fey => GenderRoles::Egalitarian,
            RaceType::Dwarf => *pick(rng, &[GenderRoles::Egalitarian, GenderRoles::MeritBased]),
            _ => *pick(rng, &[GenderRoles::Egalitarian, GenderRoles::MaleLeadership, GenderRoles::FemaleLeadership, GenderRoles::MeritBased]),
        };

        let family_structure = match race_type {
            RaceType::Orc | RaceType::Beastfolk => *pick(rng, &[FamilyStructure::Clan, FamilyStructure::Extended]),
            RaceType::Halfling => *pick(rng, &[FamilyStructure::Extended, FamilyStructure::Nuclear]),
            RaceType::Construct => FamilyStructure::Communal,
            _ => *pick(rng, &[FamilyStructure::Nuclear, FamilyStructure::Extended, FamilyStructure::Clan, FamilyStructure::Communal]),
        };

        let religiosity = if matches!(race_type, RaceType::Construct) {
            rng.gen_range(0.0..0.3)
        } else {
            rng.gen()
        };

        let monster_worship_tendency = if values.nature_harmony > 0.6 {
            rng.gen_range(0.1..0.5)
        } else if values.tradition > 0.7 {
            rng.gen_range(0.0..0.2)
        } else {
            rng.gen_range(0.0..0.3)
        };

        Self {
            id,
            name,
            naming_style,
            values,
            architecture,
            art_motifs,
            government_preference,
            gender_roles,
            family_structure,
            religiosity,
            monster_worship_tendency,
        }
    }
}

/// Pick a random element from a slice.
fn pick<'a, T>(rng: &mut impl Rng, items: &'a [T]) -> &'a T {
    &items[rng.gen_range(0..items.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::entities::races::RaceType;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_culture_values_similarity() {
        let a = CultureValues {
            martial: 0.8, tradition: 0.7, collectivism: 0.6,
            nature_harmony: 0.3, magic_acceptance: 0.4,
            xenophobia: 0.5, honor_value: 0.6, wealth: 0.5,
        };
        let b = a.clone();
        assert!((a.similarity(&b) - 1.0).abs() < 0.001);

        let c = CultureValues {
            martial: 0.2, tradition: 0.3, collectivism: 0.4,
            nature_harmony: 0.7, magic_acceptance: 0.6,
            xenophobia: 0.5, honor_value: 0.4, wealth: 0.5,
        };
        let sim = a.similarity(&c);
        assert!(sim < 0.8, "Different cultures should have lower similarity: {}", sim);
    }

    #[test]
    fn test_culture_generation() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for race_type in RaceType::all() {
            let culture = Culture::generate(
                CultureId(0),
                "Test Culture".to_string(),
                NamingStyleId(0),
                race_type,
                &mut rng,
            );
            assert!((0.0..=1.0).contains(&culture.values.martial));
            assert!((0.0..=1.0).contains(&culture.religiosity));
            assert!(!culture.art_motifs.is_empty());
        }
    }
}
