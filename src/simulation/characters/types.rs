//! Character types and core data structures
//!
//! Defines individual characters for detailed combat resolution.

use serde::{Deserialize, Serialize};

use crate::simulation::body::{Body, BodyPlan, BodyPartFunction, Tissue};
use crate::simulation::monsters::{Monster, MonsterSpecies};
use crate::simulation::technology::Age;
use crate::simulation::types::TribeId;

/// Unique identifier for a character
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CharacterId(pub u64);

impl CharacterId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for CharacterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Char#{}", self.0)
    }
}

/// Origin of a character
#[derive(Clone, Debug, PartialEq)]
pub enum CharacterOrigin {
    TribeMember {
        tribe_id: TribeId,
        role: TribeRole,
    },
    MonsterIndividual {
        species: MonsterSpecies,
    },
}

/// Role of a tribe member
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TribeRole {
    Warrior,
    Hunter,
    Civilian,
    Leader,
}

impl TribeRole {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Warrior => "Warrior",
            Self::Hunter => "Hunter",
            Self::Civilian => "Civilian",
            Self::Leader => "Leader",
        }
    }
}

/// Character attributes (1-100 scale)
#[derive(Clone, Copy, Debug)]
pub struct Attributes {
    /// Physical power - affects melee damage
    pub strength: u8,
    /// Speed and reflexes - affects hit chance and dodge
    pub agility: u8,
    /// Resilience - reduces damage taken
    pub toughness: u8,
    /// Stamina recovery and max stamina
    pub endurance: u8,
    /// Mental fortitude - resistance to pain and fear
    pub willpower: u8,
}

impl Attributes {
    pub fn new(strength: u8, agility: u8, toughness: u8, endurance: u8, willpower: u8) -> Self {
        Self {
            strength: strength.clamp(1, 100),
            agility: agility.clamp(1, 100),
            toughness: toughness.clamp(1, 100),
            endurance: endurance.clamp(1, 100),
            willpower: willpower.clamp(1, 100),
        }
    }

    /// Create average human attributes
    pub fn average_human() -> Self {
        Self::new(50, 50, 50, 50, 50)
    }

    /// Create warrior attributes
    pub fn warrior() -> Self {
        Self::new(65, 55, 60, 55, 55)
    }

    /// Get strength modifier (0.5-1.5)
    pub fn strength_modifier(&self) -> f32 {
        0.5 + (self.strength as f32 / 100.0)
    }

    /// Get agility modifier (0.5-1.5)
    pub fn agility_modifier(&self) -> f32 {
        0.5 + (self.agility as f32 / 100.0)
    }

    /// Get toughness modifier (0.5-1.5)
    pub fn toughness_modifier(&self) -> f32 {
        0.5 + (self.toughness as f32 / 100.0)
    }

    /// Get endurance modifier (0.5-1.5)
    pub fn endurance_modifier(&self) -> f32 {
        0.5 + (self.endurance as f32 / 100.0)
    }

    /// Get willpower modifier (0.5-1.5)
    pub fn willpower_modifier(&self) -> f32 {
        0.5 + (self.willpower as f32 / 100.0)
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Self::average_human()
    }
}

/// An individual character for combat
#[derive(Debug, Clone)]
pub struct Character {
    pub id: CharacterId,
    pub name: String,
    pub origin: CharacterOrigin,
    pub body: Body,
    pub attributes: Attributes,
    pub stamina: f32,
    pub max_stamina: f32,
    pub pain: f32,
    pub is_conscious: bool,
    pub is_alive: bool,
}

impl Character {
    /// Create a new character
    pub fn new(
        id: CharacterId,
        name: String,
        origin: CharacterOrigin,
        body: Body,
        attributes: Attributes,
    ) -> Self {
        let max_stamina = 100.0 * attributes.endurance_modifier();
        Self {
            id,
            name,
            origin,
            body,
            attributes,
            stamina: max_stamina,
            max_stamina,
            pain: 0.0,
            is_conscious: true,
            is_alive: true,
        }
    }

    /// Check if character can attack
    pub fn can_attack(&self) -> bool {
        self.is_alive
            && self.is_conscious
            && self.stamina > 10.0
            && (self.body.can_perform(BodyPartFunction::Grasping)
                || self.body.can_perform(BodyPartFunction::Attacking))
    }

    /// Check if character can move
    pub fn can_move(&self) -> bool {
        self.is_alive && self.is_conscious && self.body.can_perform(BodyPartFunction::Locomotion)
    }

    /// Consume stamina for an action
    pub fn consume_stamina(&mut self, amount: f32) {
        self.stamina = (self.stamina - amount).max(0.0);
    }

    /// Recover stamina
    pub fn recover_stamina(&mut self, amount: f32) {
        self.stamina = (self.stamina + amount).min(self.max_stamina);
    }

    /// Add pain from a wound
    pub fn add_pain(&mut self, amount: f32) {
        self.pain += amount;
        let pain_threshold = 50.0 * self.attributes.willpower_modifier();
        if self.pain >= pain_threshold {
            self.is_conscious = false;
        }
    }

    /// Check and update alive status based on body
    pub fn update_status(&mut self) {
        if self.body.is_dead() {
            self.is_alive = false;
            self.is_conscious = false;
        }
    }

    /// Get faction string for logging
    pub fn faction(&self) -> String {
        match &self.origin {
            CharacterOrigin::TribeMember { tribe_id, .. } => format!("Tribe#{}", tribe_id.0),
            CharacterOrigin::MonsterIndividual { species } => species.name().to_string(),
        }
    }
}

/// Create a body from a monster species
pub fn body_from_species(species: MonsterSpecies) -> Body {
    match species {
        // Humanoid bodies
        MonsterSpecies::Troll => Body::humanoid(Tissue::Flesh),
        MonsterSpecies::Yeti => Body::humanoid(Tissue::Flesh),
        MonsterSpecies::BogWight => Body::spectral(),

        // Quadruped bodies
        MonsterSpecies::Wolf | MonsterSpecies::IceWolf => Body::quadruped(Tissue::Flesh),
        MonsterSpecies::Bear => Body::quadruped(Tissue::Flesh),
        MonsterSpecies::Basilisk => Body::quadruped(Tissue::Scale),

        // Arachnid
        MonsterSpecies::GiantSpider => Body::arachnid(Tissue::Chitin),

        // Insectoid
        MonsterSpecies::Scorpion => Body::insectoid(Tissue::Chitin),

        // Avian
        MonsterSpecies::Griffin => Body::avian(Tissue::Flesh),
        MonsterSpecies::Phoenix => Body::avian(Tissue::Fire),

        // Dragon
        MonsterSpecies::Dragon => Body::dragon(),

        // Serpentine
        MonsterSpecies::Hydra => Body::serpentine(5, Tissue::Scale),

        // Worm
        MonsterSpecies::Sandworm => Body::worm(Tissue::Flesh),
    }
}

/// Create attributes from monster stats
pub fn attributes_from_monster(monster: &Monster) -> Attributes {
    let stats = monster.species.stats();

    // Scale strength based on monster strength stat (5-100 range)
    let strength = ((stats.strength / 100.0) * 100.0).clamp(30.0, 100.0) as u8;

    // Agility based on territory radius (more mobile = more agile)
    let agility = ((stats.territory_radius as f32 / 20.0) * 80.0 + 20.0).clamp(20.0, 90.0) as u8;

    // Toughness based on health
    let toughness = ((stats.health / 500.0) * 100.0).clamp(30.0, 100.0) as u8;

    // Endurance based on aggression (more aggressive = more endurance for fighting)
    let endurance = ((stats.aggression * 50.0) + 40.0).clamp(30.0, 80.0) as u8;

    // Willpower based on rarity (rarer monsters are more fearsome)
    let willpower =
        ((monster.species.rarity() as f32 / 200.0) * 60.0 + 40.0).clamp(30.0, 100.0) as u8;

    Attributes::new(strength, agility, toughness, endurance, willpower)
}

/// Create attributes for a tribe warrior based on age
pub fn warrior_attributes_for_age(age: Age) -> Attributes {
    match age {
        Age::Stone => Attributes::new(55, 50, 50, 55, 45),
        Age::Copper => Attributes::new(58, 52, 52, 55, 48),
        Age::Bronze => Attributes::new(60, 55, 55, 55, 50),
        Age::Iron => Attributes::new(65, 58, 58, 55, 52),
        Age::Classical => Attributes::new(68, 60, 60, 55, 55),
        Age::Medieval => Attributes::new(70, 62, 65, 55, 58),
        Age::Renaissance => Attributes::new(72, 65, 68, 55, 60),
    }
}

/// Body plan associated with a species (for display/reference)
pub fn body_plan_for_species(species: MonsterSpecies) -> BodyPlan {
    match species {
        MonsterSpecies::Troll | MonsterSpecies::Yeti => BodyPlan::Humanoid,
        MonsterSpecies::BogWight => BodyPlan::Spectral,
        MonsterSpecies::Wolf | MonsterSpecies::IceWolf | MonsterSpecies::Bear => BodyPlan::Quadruped,
        MonsterSpecies::Basilisk => BodyPlan::Quadruped,
        MonsterSpecies::GiantSpider => BodyPlan::Arachnid,
        MonsterSpecies::Scorpion => BodyPlan::Insectoid,
        MonsterSpecies::Griffin | MonsterSpecies::Phoenix => BodyPlan::Avian,
        MonsterSpecies::Dragon => BodyPlan::Dragon,
        MonsterSpecies::Hydra => BodyPlan::Serpentine,
        MonsterSpecies::Sandworm => BodyPlan::Worm,
    }
}
