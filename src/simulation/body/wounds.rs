//! Wound types, severity, and effects
//!
//! Defines the wound system for tracking damage to body parts.

use serde::{Deserialize, Serialize};

/// Severity of a wound
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WoundSeverity {
    Minor,
    Moderate,
    Severe,
    Critical,
}

impl WoundSeverity {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Minor => "minor",
            Self::Moderate => "moderate",
            Self::Severe => "severe",
            Self::Critical => "critical",
        }
    }

    /// Get base impairment for this severity
    pub fn impairment(&self) -> f32 {
        match self {
            Self::Minor => 0.1,
            Self::Moderate => 0.3,
            Self::Severe => 0.6,
            Self::Critical => 0.9,
        }
    }

    /// Get base bleeding rate
    pub fn bleeding_rate(&self) -> f32 {
        match self {
            Self::Minor => 0.0,
            Self::Moderate => 0.5,
            Self::Severe => 1.5,
            Self::Critical => 3.0,
        }
    }

    /// Get base pain value
    pub fn pain(&self) -> f32 {
        match self {
            Self::Minor => 5.0,
            Self::Moderate => 15.0,
            Self::Severe => 30.0,
            Self::Critical => 50.0,
        }
    }

    /// Get from damage percentage (damage / max_health)
    pub fn from_damage_ratio(ratio: f32) -> Self {
        if ratio >= 0.8 {
            Self::Critical
        } else if ratio >= 0.5 {
            Self::Severe
        } else if ratio >= 0.25 {
            Self::Moderate
        } else {
            Self::Minor
        }
    }
}

/// Types of wounds that can be inflicted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WoundType {
    // Cutting wounds
    Scratch,
    Cut,
    Gash,

    // Blunt wounds
    Bruise,
    Contusion,
    Fracture,
    CompoundFracture,
    Crush,

    // Piercing wounds
    Puncture,
    Impalement,

    // Burns
    FirstDegreeBurn,
    SecondDegreeBurn,
    ThirdDegreeBurn,

    // Special
    Frostbite,
    Necrosis,
    Severed,
    Destroyed,
}

impl WoundType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Scratch => "scratch",
            Self::Cut => "cut",
            Self::Gash => "gash",
            Self::Bruise => "bruise",
            Self::Contusion => "contusion",
            Self::Fracture => "fracture",
            Self::CompoundFracture => "compound fracture",
            Self::Crush => "crush",
            Self::Puncture => "puncture",
            Self::Impalement => "impalement",
            Self::FirstDegreeBurn => "first-degree burn",
            Self::SecondDegreeBurn => "second-degree burn",
            Self::ThirdDegreeBurn => "third-degree burn",
            Self::Frostbite => "frostbite",
            Self::Necrosis => "necrosis",
            Self::Severed => "severed",
            Self::Destroyed => "destroyed",
        }
    }

    /// Returns true if this wound type causes bleeding
    pub fn causes_bleeding(&self) -> bool {
        matches!(
            self,
            Self::Cut
                | Self::Gash
                | Self::Puncture
                | Self::Impalement
                | Self::CompoundFracture
                | Self::Severed
        )
    }

    /// Get the verb for inflicting this wound (past tense)
    pub fn infliction_verb(&self) -> &'static str {
        match self {
            Self::Scratch => "scratched",
            Self::Cut => "cut",
            Self::Gash => "gashed",
            Self::Bruise => "bruised",
            Self::Contusion => "contused",
            Self::Fracture => "fractured",
            Self::CompoundFracture => "shattered",
            Self::Crush => "crushed",
            Self::Puncture => "punctured",
            Self::Impalement => "impaled",
            Self::FirstDegreeBurn => "burned",
            Self::SecondDegreeBurn => "burned",
            Self::ThirdDegreeBurn => "charred",
            Self::Frostbite => "frosted",
            Self::Necrosis => "necrotized",
            Self::Severed => "severed",
            Self::Destroyed => "destroyed",
        }
    }

    /// Get the present participle for narrative (e.g., "inflicting a gash")
    pub fn infliction_participle(&self) -> &'static str {
        match self {
            Self::Scratch => "inflicting a scratch",
            Self::Cut => "inflicting a cut",
            Self::Gash => "inflicting a gash",
            Self::Bruise => "inflicting a bruise",
            Self::Contusion => "inflicting a contusion",
            Self::Fracture => "fracturing the bone",
            Self::CompoundFracture => "shattering the bone",
            Self::Crush => "crushing",
            Self::Puncture => "puncturing",
            Self::Impalement => "impaling",
            Self::FirstDegreeBurn => "burning",
            Self::SecondDegreeBurn => "badly burning",
            Self::ThirdDegreeBurn => "charring",
            Self::Frostbite => "freezing",
            Self::Necrosis => "causing tissue death",
            Self::Severed => "severing",
            Self::Destroyed => "destroying",
        }
    }

    /// Returns true if this wound type is immediately incapacitating
    pub fn is_incapacitating(&self) -> bool {
        matches!(
            self,
            Self::Severed | Self::Destroyed | Self::Crush | Self::CompoundFracture
        )
    }
}

/// Damage types that can cause wounds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageType {
    Slash,
    Blunt,
    Pierce,
    Fire,
    Cold,
    Poison,
}

impl DamageType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Slash => "slash",
            Self::Blunt => "blunt",
            Self::Pierce => "pierce",
            Self::Fire => "fire",
            Self::Cold => "cold",
            Self::Poison => "poison",
        }
    }

    /// Get appropriate wound type for this damage type and severity
    pub fn wound_type(&self, severity: WoundSeverity) -> WoundType {
        match self {
            Self::Slash => match severity {
                WoundSeverity::Minor => WoundType::Scratch,
                WoundSeverity::Moderate => WoundType::Cut,
                WoundSeverity::Severe => WoundType::Gash,
                WoundSeverity::Critical => WoundType::Severed,
            },
            Self::Blunt => match severity {
                WoundSeverity::Minor => WoundType::Bruise,
                WoundSeverity::Moderate => WoundType::Contusion,
                WoundSeverity::Severe => WoundType::Fracture,
                WoundSeverity::Critical => WoundType::CompoundFracture,
            },
            Self::Pierce => match severity {
                WoundSeverity::Minor => WoundType::Scratch,
                WoundSeverity::Moderate => WoundType::Puncture,
                WoundSeverity::Severe => WoundType::Puncture,
                WoundSeverity::Critical => WoundType::Impalement,
            },
            Self::Fire => match severity {
                WoundSeverity::Minor => WoundType::FirstDegreeBurn,
                WoundSeverity::Moderate => WoundType::SecondDegreeBurn,
                WoundSeverity::Severe => WoundType::ThirdDegreeBurn,
                WoundSeverity::Critical => WoundType::Destroyed,
            },
            Self::Cold => match severity {
                WoundSeverity::Minor => WoundType::Frostbite,
                WoundSeverity::Moderate => WoundType::Frostbite,
                WoundSeverity::Severe => WoundType::Frostbite,
                WoundSeverity::Critical => WoundType::Destroyed,
            },
            Self::Poison => match severity {
                WoundSeverity::Minor => WoundType::Bruise,
                WoundSeverity::Moderate => WoundType::Necrosis,
                WoundSeverity::Severe => WoundType::Necrosis,
                WoundSeverity::Critical => WoundType::Necrosis,
            },
        }
    }
}

/// A wound on a body part
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wound {
    pub wound_type: WoundType,
    pub severity: WoundSeverity,
    pub damage_type: DamageType,
    pub bleeding_rate: f32,
    pub pain: f32,
    pub infection_risk: f32,
    pub tick_inflicted: u64,
    pub is_treated: bool,
}

impl Wound {
    pub fn new(
        wound_type: WoundType,
        severity: WoundSeverity,
        damage_type: DamageType,
        tick: u64,
    ) -> Self {
        let base_bleeding = if wound_type.causes_bleeding() {
            severity.bleeding_rate()
        } else {
            0.0
        };

        Self {
            wound_type,
            severity,
            damage_type,
            bleeding_rate: base_bleeding,
            pain: severity.pain(),
            infection_risk: match severity {
                WoundSeverity::Minor => 0.05,
                WoundSeverity::Moderate => 0.15,
                WoundSeverity::Severe => 0.30,
                WoundSeverity::Critical => 0.50,
            },
            tick_inflicted: tick,
            is_treated: false,
        }
    }

    /// Get the impairment this wound causes (0.0 - 1.0)
    pub fn impairment(&self) -> f32 {
        let base = self.severity.impairment();
        // Treated wounds cause less impairment
        if self.is_treated {
            base * 0.5
        } else {
            base
        }
    }

    /// Treat this wound to reduce bleeding and infection
    pub fn treat(&mut self) {
        self.is_treated = true;
        self.bleeding_rate *= 0.2;
        self.infection_risk *= 0.3;
    }

    /// Get a description of this wound
    pub fn description(&self) -> String {
        format!("{} {}", self.severity.display_name(), self.wound_type.display_name())
    }
}

/// Effects that can result from combat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatEffect {
    Staggered,
    Knockdown,
    Stunned,
    LimbSevered { part_name: String },
    Blinded,
    Deafened,
    Winded,
    Disarmed,
    Unconscious,
    Dead { cause: String },
}

impl CombatEffect {
    pub fn display_name(&self) -> &str {
        match self {
            Self::Staggered => "staggered",
            Self::Knockdown => "knocked down",
            Self::Stunned => "stunned",
            Self::LimbSevered { .. } => "limb severed",
            Self::Blinded => "blinded",
            Self::Deafened => "deafened",
            Self::Winded => "winded",
            Self::Disarmed => "disarmed",
            Self::Unconscious => "unconscious",
            Self::Dead { .. } => "dead",
        }
    }

    pub fn narrative(&self) -> String {
        match self {
            Self::Staggered => "staggers".to_string(),
            Self::Knockdown => "is knocked to the ground".to_string(),
            Self::Stunned => "is stunned".to_string(),
            Self::LimbSevered { part_name } => format!("'s {} is severed", part_name),
            Self::Blinded => "is blinded".to_string(),
            Self::Deafened => "is deafened".to_string(),
            Self::Winded => "is winded".to_string(),
            Self::Disarmed => "is disarmed".to_string(),
            Self::Unconscious => "falls unconscious".to_string(),
            Self::Dead { cause } => format!("dies from {}", cause),
        }
    }
}
