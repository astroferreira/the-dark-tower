//! Unique named legendary creatures with personal history.
//!
//! Legendary creatures are narrative anchors - named beings like Smaug or Shelob
//! that have personal histories, lairs, hoards, and can be worshipped by cults.

use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::history::{LegendaryCreatureId, CreatureSpeciesId, FactionId, ArtifactId, EntityId};
use crate::history::time::Date;
use super::anatomy::MagicAbility;

/// A unique legendary creature with personal history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegendaryCreature {
    pub id: LegendaryCreatureId,
    pub species_id: CreatureSpeciesId,
    pub name: String,
    pub epithet: String,

    /// Unique abilities beyond the base species.
    pub unique_abilities: Vec<MagicAbility>,
    /// Size multiplier relative to species base (1.0 = normal, 2.0 = twice as large).
    pub size_multiplier: f32,

    // Lifecycle
    pub birth_date: Option<Date>,
    pub death_date: Option<Date>,

    // Territory
    pub lair_location: Option<(usize, usize)>,
    pub territory: Vec<(usize, usize)>,

    // History
    pub kills: Vec<EntityId>,
    pub artifacts_owned: Vec<ArtifactId>,

    // Worship
    pub cult_faction: Option<FactionId>,
    pub worshipper_count: u32,
}

impl LegendaryCreature {
    /// Create a new legendary creature.
    pub fn new(
        id: LegendaryCreatureId,
        species_id: CreatureSpeciesId,
        name: String,
        epithet: String,
        birth_date: Option<Date>,
    ) -> Self {
        Self {
            id,
            species_id,
            name,
            epithet,
            unique_abilities: Vec::new(),
            size_multiplier: 1.0,
            birth_date,
            death_date: None,
            lair_location: None,
            territory: Vec::new(),
            kills: Vec::new(),
            artifacts_owned: Vec::new(),
            cult_faction: None,
            worshipper_count: 0,
        }
    }

    /// Full display name: "Vrakorath the Devourer"
    pub fn full_name(&self) -> String {
        format!("{} {}", self.name, self.epithet)
    }

    /// Whether this creature is alive.
    pub fn is_alive(&self) -> bool {
        self.death_date.is_none()
    }

    /// Whether this creature has a cult following.
    pub fn is_worshipped(&self) -> bool {
        self.cult_faction.is_some() || self.worshipper_count > 0
    }

    /// Generate unique abilities for this legendary creature.
    pub fn generate_unique_abilities(&mut self, rng: &mut impl Rng) {
        let all_abilities = [
            MagicAbility::Spellcasting, MagicAbility::Illusions,
            MagicAbility::Shapeshifting, MagicAbility::Teleportation,
            MagicAbility::MindControl, MagicAbility::Necromancy,
            MagicAbility::ElementalControl, MagicAbility::CurseWeaving,
        ];
        let count = rng.gen_range(1..=3);
        for _ in 0..count {
            let ability = all_abilities[rng.gen_range(0..all_abilities.len())].clone();
            if !self.unique_abilities.contains(&ability) {
                self.unique_abilities.push(ability);
            }
        }
    }

    /// Generate a size multiplier (legendary creatures are often larger than normal).
    pub fn generate_size_multiplier(&mut self, rng: &mut impl Rng) {
        self.size_multiplier = rng.gen_range(1.2..3.0);
    }

    /// Kill this creature at the given date.
    pub fn kill(&mut self, date: Date) {
        self.death_date = Some(date);
    }
}

/// Generate a legendary creature name and epithet.
pub fn generate_legendary_name(rng: &mut impl Rng) -> (String, String) {
    let prefixes = [
        "Vrak", "Thorn", "Kael", "Drak", "Sha", "Grim", "Mol", "Zar",
        "Bael", "Kor", "Nyx", "Ash", "Syl", "Mor", "Xar", "Ith",
        "Gol", "Fyr", "Vel", "Kron",
    ];
    let suffixes = [
        "orath", "maw", "fang", "gor", "thax", "moth", "zul", "nak",
        "drek", "rok", "iel", "ath", "en", "ur", "ax", "ul",
        "gar", "esh", "ix", "on",
    ];
    let epithets = [
        "the Devourer", "the Eternal", "the Ravenous", "the Undying",
        "the Terrible", "the Ancient", "the Dreaded", "the Corrupted",
        "the Desolator", "the Nightmare", "the Shadow", "the Merciless",
        "World-Eater", "Flame-Born", "Death-Bringer", "Soul-Reaper",
        "the Insatiable", "the Voracious", "Plague-Bearer", "Storm-Caller",
        "the Unending", "the Profane", "Bone-Crusher", "Sky-Render",
    ];

    let prefix = prefixes[rng.gen_range(0..prefixes.len())];
    let suffix = suffixes[rng.gen_range(0..suffixes.len())];
    let epithet = epithets[rng.gen_range(0..epithets.len())];

    (format!("{}{}", prefix, suffix), epithet.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_legendary_creature_creation() {
        let creature = LegendaryCreature::new(
            LegendaryCreatureId(0),
            CreatureSpeciesId(0),
            "Vrakorath".to_string(),
            "the Devourer".to_string(),
            Some(Date::new(1, Season::Spring)),
        );
        assert!(creature.is_alive());
        assert!(!creature.is_worshipped());
        assert_eq!(creature.full_name(), "Vrakorath the Devourer");
    }

    #[test]
    fn test_legendary_name_generation() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for _ in 0..10 {
            let (name, epithet) = generate_legendary_name(&mut rng);
            assert!(!name.is_empty());
            assert!(!epithet.is_empty());
            eprintln!("  {} {}", name, epithet);
        }
    }

    #[test]
    fn test_legendary_kill() {
        let mut creature = LegendaryCreature::new(
            LegendaryCreatureId(0),
            CreatureSpeciesId(0),
            "Grimfang".to_string(),
            "the Terrible".to_string(),
            Some(Date::new(1, Season::Spring)),
        );
        assert!(creature.is_alive());
        creature.kill(Date::new(50, Season::Autumn));
        assert!(!creature.is_alive());
    }
}
