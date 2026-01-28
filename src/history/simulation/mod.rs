//! History simulation engine.
//!
//! Runs the world history simulation step by step, generating events,
//! growing civilizations, spawning creatures, and tracking everything
//! in the WorldHistory database.

pub mod engine;
pub mod harness;
pub mod metrics;
pub mod playback;
pub mod setup;
pub mod step;

pub use engine::HistoryEngine;
pub use metrics::SimulationMetrics;
pub use playback::PlaybackController;
