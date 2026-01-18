//! Weapons and armor definitions
//!
//! Equipment affects combat effectiveness based on tribe's technological age.

use serde::{Deserialize, Serialize};

use crate::simulation::body::{BodyPartCategory, DamageType};
use crate::simulation::technology::Age;

/// Types of weapons
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponType {
    // Natural weapons (monsters)
    Bite,
    Claws,
    Tail,
    FireBreath,
    Venom,

    // Primitive
    Fist,
    Club,
    WoodSpear,
    StoneAxe,
    Sling,

    // Copper Age
    CopperKnife,
    CopperSpear,

    // Bronze Age
    BronzeSword,
    BronzeSpear,
    BronzeAxe,

    // Iron Age
    IronSword,
    IronAxe,
    IronSpear,

    // Classical
    Gladius,
    Pilum,
    Longbow,

    // Medieval
    Longsword,
    Warhammer,
    Crossbow,
    Halberd,

    // Renaissance
    Rapier,
    Pike,
}

impl WeaponType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Bite => "bite",
            Self::Claws => "claws",
            Self::Tail => "tail",
            Self::FireBreath => "fire breath",
            Self::Venom => "venomous sting",
            Self::Fist => "fist",
            Self::Club => "club",
            Self::WoodSpear => "wooden spear",
            Self::StoneAxe => "stone axe",
            Self::Sling => "sling",
            Self::CopperKnife => "copper knife",
            Self::CopperSpear => "copper spear",
            Self::BronzeSword => "bronze sword",
            Self::BronzeSpear => "bronze spear",
            Self::BronzeAxe => "bronze axe",
            Self::IronSword => "iron sword",
            Self::IronAxe => "iron axe",
            Self::IronSpear => "iron spear",
            Self::Gladius => "gladius",
            Self::Pilum => "pilum",
            Self::Longbow => "longbow",
            Self::Longsword => "longsword",
            Self::Warhammer => "warhammer",
            Self::Crossbow => "crossbow",
            Self::Halberd => "halberd",
            Self::Rapier => "rapier",
            Self::Pike => "pike",
        }
    }

    /// Get the verb for attacking with this weapon
    pub fn attack_verb(&self) -> &'static str {
        match self {
            Self::Bite => "bites",
            Self::Claws => "claws at",
            Self::Tail => "smashes with tail",
            Self::FireBreath => "breathes fire at",
            Self::Venom => "stings",
            Self::Fist => "punches",
            Self::Club | Self::Warhammer => "bashes",
            Self::WoodSpear | Self::CopperSpear | Self::BronzeSpear | Self::IronSpear => "thrusts at",
            Self::StoneAxe | Self::BronzeAxe | Self::IronAxe => "chops at",
            Self::Sling => "slings a stone at",
            Self::CopperKnife | Self::Rapier => "stabs at",
            Self::BronzeSword | Self::IronSword | Self::Gladius | Self::Longsword => "slashes at",
            Self::Pilum | Self::Pike => "thrusts at",
            Self::Longbow | Self::Crossbow => "shoots",
            Self::Halberd => "swings at",
        }
    }

    /// Get the primary damage type
    pub fn damage_type(&self) -> DamageType {
        match self {
            Self::Bite | Self::Venom => DamageType::Pierce,
            Self::Claws => DamageType::Slash,
            Self::Tail | Self::Fist | Self::Club | Self::Warhammer | Self::Sling => DamageType::Blunt,
            Self::FireBreath => DamageType::Fire,
            Self::WoodSpear
            | Self::CopperSpear
            | Self::BronzeSpear
            | Self::IronSpear
            | Self::Pilum
            | Self::Pike
            | Self::CopperKnife
            | Self::Rapier => DamageType::Pierce,
            Self::StoneAxe | Self::BronzeAxe | Self::IronAxe | Self::Halberd => DamageType::Slash,
            Self::BronzeSword | Self::IronSword | Self::Gladius | Self::Longsword => DamageType::Slash,
            Self::Longbow | Self::Crossbow => DamageType::Pierce,
        }
    }

    /// Check if this is a ranged weapon
    pub fn is_ranged(&self) -> bool {
        matches!(
            self,
            Self::Sling | Self::Longbow | Self::Crossbow | Self::FireBreath
        )
    }

    /// Check if this is a natural weapon
    pub fn is_natural(&self) -> bool {
        matches!(
            self,
            Self::Bite | Self::Claws | Self::Tail | Self::FireBreath | Self::Venom | Self::Fist
        )
    }
}

/// A weapon with its stats
#[derive(Clone, Debug)]
pub struct Weapon {
    pub weapon_type: WeaponType,
    pub base_damage: f32,
    pub speed: f32,       // Attacks per round (higher = faster)
    pub accuracy: f32,    // Hit chance modifier
    pub armor_pierce: f32, // Armor penetration (0.0-1.0)
}

impl Weapon {
    pub fn new(weapon_type: WeaponType) -> Self {
        let (base_damage, speed, accuracy, armor_pierce) = match weapon_type {
            // Natural weapons
            WeaponType::Bite => (8.0, 1.2, 0.7, 0.1),
            WeaponType::Claws => (6.0, 1.5, 0.8, 0.0),
            WeaponType::Tail => (12.0, 0.6, 0.6, 0.0),
            WeaponType::FireBreath => (30.0, 0.4, 0.9, 0.5),
            WeaponType::Venom => (15.0, 0.8, 0.7, 0.3),

            // Primitive
            WeaponType::Fist => (3.0, 1.5, 0.8, 0.0),
            WeaponType::Club => (6.0, 1.0, 0.7, 0.0),
            WeaponType::WoodSpear => (8.0, 0.9, 0.75, 0.1),
            WeaponType::StoneAxe => (7.0, 0.8, 0.7, 0.1),
            WeaponType::Sling => (5.0, 0.7, 0.6, 0.0),

            // Copper Age
            WeaponType::CopperKnife => (5.0, 1.4, 0.85, 0.15),
            WeaponType::CopperSpear => (10.0, 0.9, 0.8, 0.2),

            // Bronze Age
            WeaponType::BronzeSword => (12.0, 1.1, 0.85, 0.25),
            WeaponType::BronzeSpear => (11.0, 1.0, 0.85, 0.25),
            WeaponType::BronzeAxe => (14.0, 0.8, 0.75, 0.3),

            // Iron Age
            WeaponType::IronSword => (15.0, 1.1, 0.88, 0.35),
            WeaponType::IronAxe => (18.0, 0.8, 0.78, 0.4),
            WeaponType::IronSpear => (14.0, 1.0, 0.88, 0.35),

            // Classical
            WeaponType::Gladius => (14.0, 1.2, 0.9, 0.35),
            WeaponType::Pilum => (16.0, 0.5, 0.7, 0.5),
            WeaponType::Longbow => (12.0, 0.8, 0.75, 0.4),

            // Medieval
            WeaponType::Longsword => (18.0, 1.0, 0.88, 0.4),
            WeaponType::Warhammer => (20.0, 0.7, 0.75, 0.6),
            WeaponType::Crossbow => (18.0, 0.5, 0.85, 0.55),
            WeaponType::Halberd => (22.0, 0.6, 0.7, 0.5),

            // Renaissance
            WeaponType::Rapier => (12.0, 1.4, 0.92, 0.45),
            WeaponType::Pike => (20.0, 0.5, 0.7, 0.45),
        };

        Self {
            weapon_type,
            base_damage,
            speed,
            accuracy,
            armor_pierce,
        }
    }

    /// Get effective damage based on strength
    pub fn damage_with_strength(&self, strength_modifier: f32) -> f32 {
        if self.weapon_type.is_ranged() {
            // Ranged weapons less affected by strength
            self.base_damage * (0.7 + strength_modifier * 0.3)
        } else {
            self.base_damage * strength_modifier
        }
    }
}

/// Get default weapon for a tribe's age
pub fn default_weapon_for_age(age: Age) -> Weapon {
    let weapon_type = match age {
        Age::Stone => WeaponType::WoodSpear,
        Age::Copper => WeaponType::CopperSpear,
        Age::Bronze => WeaponType::BronzeSword,
        Age::Iron => WeaponType::IronSword,
        Age::Classical => WeaponType::Gladius,
        Age::Medieval => WeaponType::Longsword,
        Age::Renaissance => WeaponType::Rapier,
    };
    Weapon::new(weapon_type)
}

/// Types of armor
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorType {
    None,
    Hide,
    Leather,
    PaddedLeather,
    BronzeHelm,
    BronzeChainmail,
    IronChainmail,
    IronPlate,
    SteelPlate,
}

impl ArmorType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "no armor",
            Self::Hide => "hide armor",
            Self::Leather => "leather armor",
            Self::PaddedLeather => "padded leather",
            Self::BronzeHelm => "bronze helm",
            Self::BronzeChainmail => "bronze chainmail",
            Self::IronChainmail => "iron chainmail",
            Self::IronPlate => "iron plate armor",
            Self::SteelPlate => "steel plate armor",
        }
    }
}

/// Armor with its stats
#[derive(Clone, Debug)]
pub struct Armor {
    pub armor_type: ArmorType,
    pub damage_reduction: f32,
    pub coverage: Vec<BodyPartCategory>, // Which body parts are protected
    pub speed_penalty: f32,              // Movement/attack speed reduction
}

impl Armor {
    pub fn new(armor_type: ArmorType) -> Self {
        let (damage_reduction, coverage, speed_penalty) = match armor_type {
            ArmorType::None => (0.0, vec![], 0.0),
            ArmorType::Hide => (
                0.1,
                vec![BodyPartCategory::Torso],
                0.05,
            ),
            ArmorType::Leather => (
                0.15,
                vec![BodyPartCategory::Torso, BodyPartCategory::UpperLimb],
                0.08,
            ),
            ArmorType::PaddedLeather => (
                0.2,
                vec![
                    BodyPartCategory::Torso,
                    BodyPartCategory::UpperLimb,
                    BodyPartCategory::LowerLimb,
                ],
                0.12,
            ),
            ArmorType::BronzeHelm => (
                0.25,
                vec![BodyPartCategory::Head],
                0.05,
            ),
            ArmorType::BronzeChainmail => (
                0.3,
                vec![
                    BodyPartCategory::Torso,
                    BodyPartCategory::UpperLimb,
                ],
                0.18,
            ),
            ArmorType::IronChainmail => (
                0.4,
                vec![
                    BodyPartCategory::Torso,
                    BodyPartCategory::UpperLimb,
                    BodyPartCategory::Head,
                ],
                0.2,
            ),
            ArmorType::IronPlate => (
                0.55,
                vec![
                    BodyPartCategory::Torso,
                    BodyPartCategory::UpperLimb,
                    BodyPartCategory::LowerLimb,
                    BodyPartCategory::Head,
                ],
                0.3,
            ),
            ArmorType::SteelPlate => (
                0.65,
                vec![
                    BodyPartCategory::Torso,
                    BodyPartCategory::UpperLimb,
                    BodyPartCategory::LowerLimb,
                    BodyPartCategory::Head,
                    BodyPartCategory::Extremity,
                ],
                0.35,
            ),
        };

        Self {
            armor_type,
            damage_reduction,
            coverage,
            speed_penalty,
        }
    }

    /// Check if this armor covers a body part category
    pub fn covers(&self, category: BodyPartCategory) -> bool {
        self.coverage.contains(&category)
    }

    /// Get damage after armor reduction
    pub fn reduce_damage(&self, damage: f32, target_category: BodyPartCategory, armor_pierce: f32) -> f32 {
        if self.covers(target_category) {
            let effective_reduction = self.damage_reduction * (1.0 - armor_pierce);
            damage * (1.0 - effective_reduction)
        } else {
            damage
        }
    }
}

/// Get default armor for a tribe's age
pub fn default_armor_for_age(age: Age) -> Armor {
    let armor_type = match age {
        Age::Stone => ArmorType::Hide,
        Age::Copper => ArmorType::Leather,
        Age::Bronze => ArmorType::BronzeChainmail,
        Age::Iron => ArmorType::IronChainmail,
        Age::Classical => ArmorType::IronChainmail,
        Age::Medieval => ArmorType::IronPlate,
        Age::Renaissance => ArmorType::SteelPlate,
    };
    Armor::new(armor_type)
}
