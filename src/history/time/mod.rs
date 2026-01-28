//! Time and calendar system for history simulation.
//!
//! Provides seasonal calendar (4 seasons per year) and era tracking.

pub mod calendar;
pub mod timeline;

pub use calendar::Date;
pub use timeline::{Era, Timeline};
