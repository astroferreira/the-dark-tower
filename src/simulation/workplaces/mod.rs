//! Workplace system - locations where jobs are performed
//!
//! Workplaces provide structure for job assignments and efficiency bonuses.

pub mod types;
pub mod production;

pub use types::{Workplace, WorkplaceId, WorkplaceType, WorkOrder};
pub use production::{WorkplaceManager, WorkplaceSummary};
