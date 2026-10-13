//! History simulation engine.
//!
//! Runs the world history simulation step by step, generating events,
//! growing civilizations, spawning creatures, and tracking everything
//! in the WorldHistory database.

pub mod engine;
pub mod harness;
pub mod invariants;
pub mod metrics;
pub mod playback;
pub mod setup;
pub mod step;

pub use engine::HistoryEngine;
pub use metrics::SimulationMetrics;
pub use playback::PlaybackController;

/// One of the two rows nearest a pole: on the map each spans its whole width, on the globe a
/// sliver round the pole. No town is founded there (`setup::find_settlement_sites`,
/// `step_colonization`); under a steep tilt the poles can be warm enough to tempt one.
pub fn polar_row(y: usize, height: usize) -> bool { y < 2 || y + 2 >= height }
