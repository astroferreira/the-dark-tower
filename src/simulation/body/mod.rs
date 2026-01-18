//! Body system module
//!
//! Provides body structures, body parts, wounds, and related functionality
//! for detailed combat simulation.

pub mod parts;
pub mod templates;
pub mod wounds;

// Re-export commonly used types
pub use parts::{
    BodyPart, BodyPartCategory, BodyPartFunction, BodyPartId, BodyPartSize, Tissue,
};
pub use templates::{Body, BodyPlan};
pub use wounds::{CombatEffect, DamageType, Wound, WoundSeverity, WoundType};
