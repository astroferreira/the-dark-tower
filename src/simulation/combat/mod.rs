//! Combat system module
//!
//! Provides detailed combat resolution with body parts, wounds, and logging.

pub mod damage;
pub mod log;
pub mod resolution;

// Re-export commonly used types
pub use damage::{apply_damage_to_part, check_death, select_target_part, DamageResult};
pub use log::{
    CombatAction, CombatEncounterLog, CombatLogEntry, CombatLogStats, CombatLogStore,
    CombatResult, CombatantRef, EncounterOutcome,
};
pub use resolution::resolve_attack;
