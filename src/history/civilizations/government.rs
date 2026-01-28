//! Government and succession systems.

use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::history::FigureId;
use crate::history::entities::culture::GovernmentType;

/// Succession laws for leadership transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuccessionLaw {
    /// Eldest child inherits.
    Primogeniture,
    /// Youngest child inherits.
    Ultimogeniture,
    /// Eldest son inherits.
    MalePrimogeniture,
    /// Eldest daughter inherits.
    FemalePrimogeniture,
    /// Nobles vote.
    Elective,
    /// Council chooses from ruling family.
    ElectiveMonarchy,
    /// Ruler designates successor.
    Designation,
    /// Oldest dynasty member.
    Seniority,
    /// Elected from extended family (Celtic).
    Tanistry,
    /// Anyone can claim (usually by force).
    OpenSuccession,
}

impl SuccessionLaw {
    /// Get a default succession law for a government type.
    pub fn for_government(gov: GovernmentType, rng: &mut impl Rng) -> Self {
        match gov {
            GovernmentType::Monarchy => {
                *pick(rng, &[
                    SuccessionLaw::Primogeniture,
                    SuccessionLaw::MalePrimogeniture,
                    SuccessionLaw::FemalePrimogeniture,
                    SuccessionLaw::Designation,
                ])
            }
            GovernmentType::Theocracy => {
                *pick(rng, &[
                    SuccessionLaw::Elective,
                    SuccessionLaw::Designation,
                    SuccessionLaw::Seniority,
                ])
            }
            GovernmentType::Republic => SuccessionLaw::Elective,
            GovernmentType::Oligarchy => SuccessionLaw::ElectiveMonarchy,
            GovernmentType::TribalCouncil => {
                *pick(rng, &[SuccessionLaw::Tanistry, SuccessionLaw::Elective])
            }
            GovernmentType::Dictatorship => SuccessionLaw::OpenSuccession,
            GovernmentType::Magocracy => SuccessionLaw::Designation,
            GovernmentType::HiveCollective => SuccessionLaw::Seniority,
        }
    }

    /// Whether this succession law can cause a crisis (disputed heir).
    pub fn crisis_prone(&self) -> bool {
        matches!(self,
            SuccessionLaw::OpenSuccession |
            SuccessionLaw::Tanistry |
            SuccessionLaw::ElectiveMonarchy
        )
    }

    /// Whether this succession law requires an heir from the dynasty.
    pub fn requires_dynasty(&self) -> bool {
        matches!(self,
            SuccessionLaw::Primogeniture |
            SuccessionLaw::Ultimogeniture |
            SuccessionLaw::MalePrimogeniture |
            SuccessionLaw::FemalePrimogeniture |
            SuccessionLaw::ElectiveMonarchy |
            SuccessionLaw::Seniority |
            SuccessionLaw::Tanistry
        )
    }
}

/// A leadership position within a faction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub title: String,
    pub holder: Option<FigureId>,
    pub position_type: PositionType,
}

/// Types of positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionType {
    Ruler,
    Heir,
    General,
    Advisor,
    ReligiousLeader,
    GuildMaster,
    SpyMaster,
    Ambassador,
}

fn pick<'a, T>(rng: &mut impl Rng, items: &'a [T]) -> &'a T {
    &items[rng.gen_range(0..items.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_succession_for_government() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let law = SuccessionLaw::for_government(GovernmentType::Monarchy, &mut rng);
        assert!(law.requires_dynasty());

        let law = SuccessionLaw::for_government(GovernmentType::Republic, &mut rng);
        assert_eq!(law, SuccessionLaw::Elective);
    }

    #[test]
    fn test_crisis_prone() {
        assert!(SuccessionLaw::OpenSuccession.crisis_prone());
        assert!(!SuccessionLaw::Primogeniture.crisis_prone());
    }
}
