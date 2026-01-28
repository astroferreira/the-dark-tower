//! Deity and divine domain definitions.

use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::history::{DeityId, LegendaryCreatureId, ArtifactId, CreatureSpeciesId};

/// Type of divine entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeityType {
    God,
    Spirit,
    Ancestor,
    Monster,
    Concept,
    Demon,
}

/// Divine domains (areas of influence).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Domain {
    War,
    Death,
    Life,
    Nature,
    Fire,
    Water,
    Earth,
    Air,
    Magic,
    Knowledge,
    Crafts,
    Trade,
    Chaos,
    Order,
    Love,
    Vengeance,
    Trickery,
    Darkness,
    Light,
}

impl Domain {
    pub fn all() -> &'static [Domain] {
        &[
            Domain::War, Domain::Death, Domain::Life, Domain::Nature,
            Domain::Fire, Domain::Water, Domain::Earth, Domain::Air,
            Domain::Magic, Domain::Knowledge, Domain::Crafts, Domain::Trade,
            Domain::Chaos, Domain::Order, Domain::Love, Domain::Vengeance,
            Domain::Trickery, Domain::Darkness, Domain::Light,
        ]
    }
}

/// Alignment of a deity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    Benevolent,
    Neutral,
    Malevolent,
    Capricious,
}

/// A deity or worshipped entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deity {
    pub id: DeityId,
    pub name: String,
    pub epithets: Vec<String>,

    pub deity_type: DeityType,
    pub domains: Vec<Domain>,
    pub alignment: Alignment,

    /// If this deity is based on a living legendary creature.
    pub associated_monster: Option<LegendaryCreatureId>,

    pub sacred_animals: Vec<CreatureSpeciesId>,
    pub sacred_places: Vec<(usize, usize)>,

    pub divine_artifacts: Vec<ArtifactId>,
}

impl Deity {
    /// Create a standard god with random domains.
    pub fn new_god(id: DeityId, name: String, rng: &mut impl Rng) -> Self {
        let domain_count = rng.gen_range(1..=3);
        let mut domains = Vec::new();
        let all = Domain::all();
        for _ in 0..domain_count {
            let d = all[rng.gen_range(0..all.len())];
            if !domains.contains(&d) {
                domains.push(d);
            }
        }

        let alignment = match rng.gen_range(0..4) {
            0 => Alignment::Benevolent,
            1 => Alignment::Malevolent,
            2 => Alignment::Capricious,
            _ => Alignment::Neutral,
        };

        Self {
            id,
            name,
            epithets: Vec::new(),
            deity_type: DeityType::God,
            domains,
            alignment,
            associated_monster: None,
            sacred_animals: Vec::new(),
            sacred_places: Vec::new(),
            divine_artifacts: Vec::new(),
        }
    }

    /// Create a deity based on a legendary monster.
    pub fn from_monster(
        id: DeityId,
        name: String,
        monster_id: LegendaryCreatureId,
    ) -> Self {
        Self {
            id,
            name,
            epithets: Vec::new(),
            deity_type: DeityType::Monster,
            domains: vec![Domain::Chaos, Domain::Death],
            alignment: Alignment::Malevolent,
            associated_monster: Some(monster_id),
            sacred_animals: Vec::new(),
            sacred_places: Vec::new(),
            divine_artifacts: Vec::new(),
        }
    }

    /// Create an ancestor deity from a deified figure.
    pub fn from_ancestor(id: DeityId, name: String, domains: Vec<Domain>) -> Self {
        Self {
            id,
            name,
            epithets: Vec::new(),
            deity_type: DeityType::Ancestor,
            domains,
            alignment: Alignment::Benevolent,
            associated_monster: None,
            sacred_animals: Vec::new(),
            sacred_places: Vec::new(),
            divine_artifacts: Vec::new(),
        }
    }

    /// Whether this deity is based on a living creature.
    pub fn is_monster_deity(&self) -> bool {
        self.associated_monster.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_deity_creation() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let deity = Deity::new_god(DeityId(0), "Thalos".to_string(), &mut rng);
        assert!(!deity.domains.is_empty());
        assert!(!deity.is_monster_deity());
    }

    #[test]
    fn test_monster_deity() {
        let deity = Deity::from_monster(
            DeityId(0),
            "Vrakorath".to_string(),
            LegendaryCreatureId(0),
        );
        assert!(deity.is_monster_deity());
        assert_eq!(deity.deity_type, DeityType::Monster);
    }
}
