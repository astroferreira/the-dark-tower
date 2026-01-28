//! Name generation system for civilizations, characters, places, and artifacts.
//!
//! Each civilization has a `NamingStyle` that defines phonetic preferences,
//! syllable patterns, and affixes. Names are generated from these styles
//! to create culturally distinct naming across races and factions.

pub mod styles;
pub mod generator;

pub use styles::NamingStyle;
pub use generator::NameGenerator;
