//! Character management module
//!
//! Spawns and manages individual characters for detailed combat resolution.

pub mod equipment;
pub mod types;

use rand::Rng;
use std::collections::HashMap;

pub use equipment::{Armor, ArmorType, Weapon, WeaponType};
pub use types::{
    Attributes, Character, CharacterId, CharacterOrigin, TribeRole,
    attributes_from_monster, body_from_species, warrior_attributes_for_age,
};

use crate::simulation::body::{Body, Tissue};
use crate::simulation::monsters::Monster;
use crate::simulation::technology::Age;
use crate::simulation::tribe::Tribe;
use crate::simulation::types::TribeId;

/// Names for generating warrior names
const WARRIOR_PREFIXES: &[&str] = &[
    "Gor", "Thrag", "Bor", "Kar", "Mog", "Drak", "Vorn", "Grim", "Krol", "Zar",
    "Ulf", "Bjorn", "Sig", "Tor", "Erik", "Rolf", "Sven", "Leif", "Odin", "Freya",
    "Ash", "Oak", "Stone", "Iron", "Storm", "Fire", "Ice", "Dark", "Blood", "Shadow",
];

const WARRIOR_SUFFIXES: &[&str] = &[
    "nak", "rim", "dak", "gor", "mur", "lok", "ven", "kar", "dor", "mar",
    "son", "sen", "mund", "heim", "gard", "ric", "ald", "win", "hard", "ward",
    "heart", "hand", "blade", "shield", "fist", "eye", "tooth", "claw", "bane", "slayer",
];

/// Manages character creation and lifecycle
#[derive(Debug, Clone)]
pub struct CharacterManager {
    characters: HashMap<CharacterId, Character>,
    next_id: u64,
}

impl Default for CharacterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterManager {
    pub fn new() -> Self {
        Self {
            characters: HashMap::new(),
            next_id: 1,
        }
    }

    /// Generate a unique character ID
    fn next_id(&mut self) -> CharacterId {
        let id = CharacterId::new(self.next_id);
        self.next_id += 1;
        id
    }

    /// Generate a random warrior name
    pub fn generate_warrior_name<R: Rng>(&self, rng: &mut R) -> String {
        let prefix = WARRIOR_PREFIXES[rng.gen_range(0..WARRIOR_PREFIXES.len())];
        let suffix = WARRIOR_SUFFIXES[rng.gen_range(0..WARRIOR_SUFFIXES.len())];
        format!("{}{}", prefix, suffix)
    }

    /// Spawn warriors from a tribe for combat
    pub fn spawn_tribe_warriors<R: Rng>(
        &mut self,
        tribe: &Tribe,
        count: u32,
        rng: &mut R,
    ) -> Vec<CharacterId> {
        let mut spawned = Vec::new();
        let age = tribe.tech_state.current_age();

        for _ in 0..count {
            let id = self.next_id();
            let name = self.generate_warrior_name(rng);

            let body = Body::humanoid(Tissue::Flesh);
            let attributes = warrior_attributes_for_age(age);

            let character = Character::new(
                id,
                name,
                CharacterOrigin::TribeMember {
                    tribe_id: tribe.id,
                    role: TribeRole::Warrior,
                },
                body,
                attributes,
            );

            self.characters.insert(id, character);
            spawned.push(id);
        }

        spawned
    }

    /// Create a character from a monster
    pub fn create_monster_character(&mut self, monster: &Monster) -> CharacterId {
        let id = self.next_id();
        let name = format!("{}#{}", monster.species.name(), monster.id.0);

        let body = body_from_species(monster.species);
        let attributes = attributes_from_monster(monster);

        let character = Character::new(
            id,
            name,
            CharacterOrigin::MonsterIndividual {
                species: monster.species,
            },
            body,
            attributes,
        );

        self.characters.insert(id, character);
        id
    }

    /// Get a character by ID
    pub fn get(&self, id: &CharacterId) -> Option<&Character> {
        self.characters.get(id)
    }

    /// Get a mutable character by ID
    pub fn get_mut(&mut self, id: &CharacterId) -> Option<&mut Character> {
        self.characters.get_mut(id)
    }

    /// Remove a character (despawn)
    pub fn despawn(&mut self, id: CharacterId) {
        self.characters.remove(&id);
    }

    /// Despawn multiple characters
    pub fn despawn_all(&mut self, ids: &[CharacterId]) {
        for id in ids {
            self.characters.remove(id);
        }
    }

    /// Get all living characters with a specific origin tribe
    pub fn tribe_characters(&self, tribe_id: TribeId) -> Vec<CharacterId> {
        self.characters
            .iter()
            .filter_map(|(id, c)| {
                if c.is_alive {
                    if let CharacterOrigin::TribeMember { tribe_id: tid, .. } = &c.origin {
                        if *tid == tribe_id {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect()
    }

    /// Get number of active characters
    pub fn active_count(&self) -> usize {
        self.characters.len()
    }

    /// Clear all characters
    pub fn clear(&mut self) {
        self.characters.clear();
    }
}

/// Get weapon for a character based on their origin
pub fn weapon_for_character(character: &Character, age: Option<Age>) -> Weapon {
    match &character.origin {
        CharacterOrigin::TribeMember { .. } => {
            equipment::default_weapon_for_age(age.unwrap_or(Age::Stone))
        }
        CharacterOrigin::MonsterIndividual { species } => {
            // Monsters use natural weapons based on their species
            use crate::simulation::monsters::MonsterSpecies;
            let weapon_type = match species {
                MonsterSpecies::Wolf | MonsterSpecies::IceWolf | MonsterSpecies::Bear => {
                    WeaponType::Bite
                }
                MonsterSpecies::GiantSpider => WeaponType::Bite,
                MonsterSpecies::Troll | MonsterSpecies::Yeti => WeaponType::Claws,
                MonsterSpecies::Griffin => WeaponType::Claws,
                MonsterSpecies::Dragon => WeaponType::FireBreath,
                MonsterSpecies::Hydra => WeaponType::Bite,
                MonsterSpecies::BogWight => WeaponType::Claws,
                MonsterSpecies::Sandworm => WeaponType::Bite,
                MonsterSpecies::Scorpion => WeaponType::Venom,
                MonsterSpecies::Basilisk => WeaponType::Bite,
                MonsterSpecies::Phoenix => WeaponType::Claws, // Fire attacks handled separately
            };
            Weapon::new(weapon_type)
        }
    }
}

/// Get armor for a character based on their origin
pub fn armor_for_character(character: &Character, age: Option<Age>) -> Armor {
    match &character.origin {
        CharacterOrigin::TribeMember { .. } => {
            equipment::default_armor_for_age(age.unwrap_or(Age::Stone))
        }
        CharacterOrigin::MonsterIndividual { .. } => {
            // Monsters don't wear armor (their tissue type provides natural protection)
            Armor::new(ArmorType::None)
        }
    }
}
