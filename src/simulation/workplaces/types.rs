//! Workplace types and structures
//!
//! Defines workplaces where jobs can be performed.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::simulation::types::TileCoord;
use crate::simulation::jobs::types::JobType;

/// Unique identifier for a workplace
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkplaceId(pub u64);

impl fmt::Display for WorkplaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Workplace#{}", self.0)
    }
}

/// Types of workplaces
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkplaceType {
    /// Open field for farming
    Farm,
    /// Mining site
    Mine,
    /// Logging area
    LoggingCamp,
    /// Fishing spot
    FishingSpot,
    /// Smithy for metalworking
    Smithy,
    /// Workshop for crafting
    Workshop,
    /// Kitchen for food preparation
    Kitchen,
    /// Library for research
    Library,
    /// Temple for religious activities
    Temple,
    /// Hospital for healing
    Hospital,
    /// Barracks for military training
    Barracks,
    /// Guard post
    GuardPost,
}

impl WorkplaceType {
    /// Get all workplace types
    pub fn all() -> &'static [WorkplaceType] {
        &[
            WorkplaceType::Farm,
            WorkplaceType::Mine,
            WorkplaceType::LoggingCamp,
            WorkplaceType::FishingSpot,
            WorkplaceType::Smithy,
            WorkplaceType::Workshop,
            WorkplaceType::Kitchen,
            WorkplaceType::Library,
            WorkplaceType::Temple,
            WorkplaceType::Hospital,
            WorkplaceType::Barracks,
            WorkplaceType::GuardPost,
        ]
    }

    /// Get the jobs this workplace supports
    pub fn supported_jobs(&self) -> &'static [JobType] {
        match self {
            WorkplaceType::Farm => &[JobType::Farmer],
            WorkplaceType::Mine => &[JobType::Miner],
            WorkplaceType::LoggingCamp => &[JobType::Woodcutter],
            WorkplaceType::FishingSpot => &[JobType::Fisher],
            WorkplaceType::Smithy => &[JobType::Smith],
            WorkplaceType::Workshop => &[JobType::Craftsperson],
            WorkplaceType::Kitchen => &[JobType::Cook],
            WorkplaceType::Library => &[JobType::Scholar],
            WorkplaceType::Temple => &[JobType::Priest],
            WorkplaceType::Hospital => &[JobType::Healer],
            WorkplaceType::Barracks => &[JobType::Warrior],
            WorkplaceType::GuardPost => &[JobType::Guard],
        }
    }

    /// Maximum workers this workplace can support
    pub fn max_workers(&self) -> u32 {
        match self {
            WorkplaceType::Farm => 10,
            WorkplaceType::Mine => 5,
            WorkplaceType::LoggingCamp => 5,
            WorkplaceType::FishingSpot => 3,
            WorkplaceType::Smithy => 3,
            WorkplaceType::Workshop => 5,
            WorkplaceType::Kitchen => 3,
            WorkplaceType::Library => 5,
            WorkplaceType::Temple => 5,
            WorkplaceType::Hospital => 5,
            WorkplaceType::Barracks => 20,
            WorkplaceType::GuardPost => 5,
        }
    }

    /// Efficiency bonus from this workplace
    pub fn efficiency_bonus(&self) -> f32 {
        match self {
            WorkplaceType::Smithy => 1.5,  // Smithy boosts metal production
            WorkplaceType::Workshop => 1.3,
            WorkplaceType::Library => 1.5,  // Research bonus
            WorkplaceType::Kitchen => 1.2,  // Food bonus
            WorkplaceType::Barracks => 1.4, // Training bonus
            _ => 1.0,
        }
    }

    /// Building required for this workplace
    pub fn required_building(&self) -> Option<&'static str> {
        match self {
            WorkplaceType::Smithy => Some("Smithy"),
            WorkplaceType::Workshop => Some("Workshop"),
            WorkplaceType::Kitchen => Some("Kitchen"),
            WorkplaceType::Library => Some("Library"),
            WorkplaceType::Temple => Some("Temple"),
            WorkplaceType::Hospital => Some("Hospital"),
            WorkplaceType::Barracks => Some("Barracks"),
            _ => None, // Outdoor workplaces don't need buildings
        }
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            WorkplaceType::Farm => "Farm",
            WorkplaceType::Mine => "Mine",
            WorkplaceType::LoggingCamp => "Logging Camp",
            WorkplaceType::FishingSpot => "Fishing Spot",
            WorkplaceType::Smithy => "Smithy",
            WorkplaceType::Workshop => "Workshop",
            WorkplaceType::Kitchen => "Kitchen",
            WorkplaceType::Library => "Library",
            WorkplaceType::Temple => "Temple",
            WorkplaceType::Hospital => "Hospital",
            WorkplaceType::Barracks => "Barracks",
            WorkplaceType::GuardPost => "Guard Post",
        }
    }
}

/// A workplace instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workplace {
    pub id: WorkplaceId,
    pub workplace_type: WorkplaceType,
    pub location: TileCoord,
    pub current_workers: u32,
    pub efficiency: f32,
    pub is_active: bool,
}

impl Workplace {
    pub fn new(id: WorkplaceId, workplace_type: WorkplaceType, location: TileCoord) -> Self {
        Workplace {
            id,
            workplace_type,
            location,
            current_workers: 0,
            efficiency: workplace_type.efficiency_bonus(),
            is_active: true,
        }
    }

    /// Get available worker slots
    pub fn available_slots(&self) -> u32 {
        self.workplace_type.max_workers().saturating_sub(self.current_workers)
    }

    /// Can this workplace accept more workers?
    pub fn can_accept_workers(&self) -> bool {
        self.is_active && self.available_slots() > 0
    }

    /// Add workers to this workplace
    pub fn add_workers(&mut self, count: u32) -> u32 {
        let can_add = self.available_slots().min(count);
        self.current_workers += can_add;
        can_add
    }

    /// Remove workers from this workplace
    pub fn remove_workers(&mut self, count: u32) {
        self.current_workers = self.current_workers.saturating_sub(count);
    }
}

/// Work order for production queue
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkOrder {
    pub id: u64,
    pub workplace_id: WorkplaceId,
    pub job_type: JobType,
    pub target_amount: f32,
    pub completed_amount: f32,
    pub priority: u32,
}

impl WorkOrder {
    pub fn new(id: u64, workplace_id: WorkplaceId, job_type: JobType, target: f32) -> Self {
        WorkOrder {
            id,
            workplace_id,
            job_type,
            target_amount: target,
            completed_amount: 0.0,
            priority: 50,
        }
    }

    /// Progress ratio (0.0 - 1.0)
    pub fn progress(&self) -> f32 {
        if self.target_amount <= 0.0 {
            1.0
        } else {
            (self.completed_amount / self.target_amount).min(1.0)
        }
    }

    /// Is this order complete?
    pub fn is_complete(&self) -> bool {
        self.completed_amount >= self.target_amount
    }

    /// Add progress to this order
    pub fn add_progress(&mut self, amount: f32) {
        self.completed_amount += amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workplace_types() {
        for wt in WorkplaceType::all() {
            assert!(wt.max_workers() > 0);
            assert!(!wt.supported_jobs().is_empty());
        }
    }

    #[test]
    fn test_workplace_instance() {
        let wp = Workplace::new(
            WorkplaceId(1),
            WorkplaceType::Farm,
            TileCoord::new(10, 10),
        );

        assert!(wp.is_active);
        assert!(wp.available_slots() > 0);
    }

    #[test]
    fn test_work_order() {
        let mut order = WorkOrder::new(
            1,
            WorkplaceId(1),
            JobType::Farmer,
            100.0,
        );

        assert!(!order.is_complete());
        order.add_progress(50.0);
        assert_eq!(order.progress(), 0.5);
        order.add_progress(50.0);
        assert!(order.is_complete());
    }
}
