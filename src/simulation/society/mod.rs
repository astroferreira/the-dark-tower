//! Society system - government types and succession
//!
//! This module handles different society types (Theocracy, Monarchy, Democracy, etc.)
//! and their effects on tribe behavior, production, and military strength.

pub mod types;
pub mod succession;

pub use types::{
    SocietyType, SocietyConfig, SocietyState, SuccessionMethod, SpecialBonus,
};
pub use succession::{process_succession, check_leader_death};
