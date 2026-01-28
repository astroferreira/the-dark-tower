//! Historical event system with causality tracking.

pub mod types;
pub mod chronicle;

pub use types::{Event, EventType, EventOutcome, Consequence};
pub use chronicle::Chronicle;
