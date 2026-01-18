//! Interaction system - diplomacy, trade, conflict, migration, reputation

pub mod diplomacy;
pub mod trade;
pub mod conflict;
pub mod migration;
pub mod reputation;

pub use diplomacy::{DiplomacyState, process_diplomacy_tick};
pub use trade::process_trade_tick;
pub use conflict::process_conflict_tick;
pub use migration::process_migration_tick;
pub use reputation::{ReputationState, SpeciesReputation, SpeciesDisposition, ReputationEvent};
