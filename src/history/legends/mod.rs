//! Legends mode - interactive history exploration.
//!
//! Browse factions, figures, creatures, artifacts, and events
//! from the simulated world history.

pub mod mode;
pub mod queries;
#[cfg(feature = "legacy")]
pub mod renderer;

pub use mode::LegendsMode;
