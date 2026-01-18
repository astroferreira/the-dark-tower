//! Activity Log System
//!
//! Tracks local events and activities for display in the explorer UI.
//! This provides visibility into what's happening at the local level.

use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

use crate::simulation::types::TileCoord;
use crate::simulation::colonists::types::ColonistId;
use crate::simulation::monsters::MonsterId;
use crate::simulation::fauna::FaunaId;

/// Maximum number of entries to keep in the activity log
const MAX_ACTIVITY_ENTRIES: usize = 50;

/// Category of activity event
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityCategory {
    /// Colonist started/changed activity
    ColonistActivity,
    /// Monster behavior change
    MonsterBehavior,
    /// Fauna activity
    FaunaActivity,
    /// Resource gathering/production
    Resource,
    /// Social interaction
    Social,
    /// Movement/travel
    Movement,
    /// Danger/threat
    Danger,
    /// Discovery/exploration
    Discovery,
    /// Construction/building
    Construction,
}

impl ActivityCategory {
    /// Get display color (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            ActivityCategory::ColonistActivity => (255, 255, 100), // Yellow
            ActivityCategory::MonsterBehavior => (255, 100, 100),  // Red
            ActivityCategory::FaunaActivity => (100, 200, 100),    // Green
            ActivityCategory::Resource => (100, 200, 255),         // Cyan
            ActivityCategory::Social => (200, 150, 255),           // Purple
            ActivityCategory::Movement => (180, 180, 180),         // Gray
            ActivityCategory::Danger => (255, 50, 50),             // Bright red
            ActivityCategory::Discovery => (255, 200, 100),        // Orange
            ActivityCategory::Construction => (150, 100, 50),      // Brown
        }
    }

    /// Get short label for display
    pub fn label(&self) -> &'static str {
        match self {
            ActivityCategory::ColonistActivity => "ACT",
            ActivityCategory::MonsterBehavior => "MON",
            ActivityCategory::FaunaActivity => "FAU",
            ActivityCategory::Resource => "RES",
            ActivityCategory::Social => "SOC",
            ActivityCategory::Movement => "MOV",
            ActivityCategory::Danger => "!!",
            ActivityCategory::Discovery => "DIS",
            ActivityCategory::Construction => "BLD",
        }
    }
}

/// An activity log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Tick when this happened
    pub tick: u64,
    /// World tile location
    pub location: TileCoord,
    /// Category of event
    pub category: ActivityCategory,
    /// Short description
    pub message: String,
    /// Entity involved (if any)
    pub entity: Option<ActivityEntity>,
    /// Importance (higher = more important, shown first)
    pub importance: u8,
}

/// Entity involved in an activity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActivityEntity {
    Colonist { id: ColonistId, name: String },
    Monster { id: MonsterId, species: String },
    Fauna { id: FaunaId, species: String },
}

impl ActivityEntity {
    pub fn name(&self) -> &str {
        match self {
            ActivityEntity::Colonist { name, .. } => name,
            ActivityEntity::Monster { species, .. } => species,
            ActivityEntity::Fauna { species, .. } => species,
        }
    }
}

/// The activity log store
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActivityLog {
    entries: VecDeque<ActivityEntry>,
    /// Counters for stats
    pub stats: ActivityStats,
}

/// Statistics about activities
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActivityStats {
    pub total_events: u64,
    pub colonist_activities: u64,
    pub monster_events: u64,
    pub fauna_events: u64,
    pub danger_events: u64,
}

impl ActivityLog {
    pub fn new() -> Self {
        ActivityLog {
            entries: VecDeque::with_capacity(MAX_ACTIVITY_ENTRIES),
            stats: ActivityStats::default(),
        }
    }

    /// Add a new activity entry
    pub fn log(&mut self, entry: ActivityEntry) {
        // Update stats
        self.stats.total_events += 1;
        match entry.category {
            ActivityCategory::ColonistActivity => self.stats.colonist_activities += 1,
            ActivityCategory::MonsterBehavior => self.stats.monster_events += 1,
            ActivityCategory::FaunaActivity => self.stats.fauna_events += 1,
            ActivityCategory::Danger => self.stats.danger_events += 1,
            _ => {}
        }

        // Add entry
        self.entries.push_back(entry);

        // Trim if over limit
        while self.entries.len() > MAX_ACTIVITY_ENTRIES {
            self.entries.pop_front();
        }
    }

    /// Log a colonist activity
    pub fn log_colonist_activity(
        &mut self,
        tick: u64,
        location: TileCoord,
        colonist_id: ColonistId,
        name: &str,
        message: String,
        importance: u8,
    ) {
        self.log(ActivityEntry {
            tick,
            location,
            category: ActivityCategory::ColonistActivity,
            message,
            entity: Some(ActivityEntity::Colonist {
                id: colonist_id,
                name: name.to_string(),
            }),
            importance,
        });
    }

    /// Log a monster event
    pub fn log_monster_event(
        &mut self,
        tick: u64,
        location: TileCoord,
        monster_id: MonsterId,
        species: &str,
        message: String,
        is_danger: bool,
    ) {
        self.log(ActivityEntry {
            tick,
            location,
            category: if is_danger {
                ActivityCategory::Danger
            } else {
                ActivityCategory::MonsterBehavior
            },
            message,
            entity: Some(ActivityEntity::Monster {
                id: monster_id,
                species: species.to_string(),
            }),
            importance: if is_danger { 10 } else { 3 },
        });
    }

    /// Log a fauna event
    pub fn log_fauna_event(
        &mut self,
        tick: u64,
        location: TileCoord,
        fauna_id: FaunaId,
        species: &str,
        message: String,
    ) {
        self.log(ActivityEntry {
            tick,
            location,
            category: ActivityCategory::FaunaActivity,
            message,
            entity: Some(ActivityEntity::Fauna {
                id: fauna_id,
                species: species.to_string(),
            }),
            importance: 1,
        });
    }

    /// Log a resource event
    pub fn log_resource(&mut self, tick: u64, location: TileCoord, message: String) {
        self.log(ActivityEntry {
            tick,
            location,
            category: ActivityCategory::Resource,
            message,
            entity: None,
            importance: 2,
        });
    }

    /// Log a social event
    pub fn log_social(
        &mut self,
        tick: u64,
        location: TileCoord,
        colonist_id: ColonistId,
        name: &str,
        message: String,
    ) {
        self.log(ActivityEntry {
            tick,
            location,
            category: ActivityCategory::Social,
            message,
            entity: Some(ActivityEntity::Colonist {
                id: colonist_id,
                name: name.to_string(),
            }),
            importance: 4,
        });
    }

    /// Log a danger event (high priority)
    pub fn log_danger(&mut self, tick: u64, location: TileCoord, message: String) {
        self.log(ActivityEntry {
            tick,
            location,
            category: ActivityCategory::Danger,
            message,
            entity: None,
            importance: 10,
        });
    }

    /// Log a construction event
    pub fn log_construction(&mut self, tick: u64, location: TileCoord, message: String) {
        self.log(ActivityEntry {
            tick,
            location,
            category: ActivityCategory::Construction,
            message,
            entity: None,
            importance: 5,
        });
    }

    /// Get recent entries (newest first)
    pub fn recent_entries(&self, count: usize) -> Vec<&ActivityEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// Get entries for a specific location
    pub fn entries_at(&self, location: TileCoord, count: usize) -> Vec<&ActivityEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.location == location)
            .take(count)
            .collect()
    }

    /// Get entries near a location (within range)
    pub fn entries_near(&self, location: TileCoord, range: i32, count: usize) -> Vec<&ActivityEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| {
                let dx = (e.location.x as i32 - location.x as i32).abs();
                let dy = (e.location.y as i32 - location.y as i32).abs();
                dx <= range && dy <= range
            })
            .take(count)
            .collect()
    }

    /// Get high-importance entries (dangers, discoveries)
    pub fn important_entries(&self, count: usize) -> Vec<&ActivityEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.importance.cmp(&a.importance));
        entries.into_iter().take(count).collect()
    }

    /// Clear old entries (keep only recent ticks)
    pub fn clear_old(&mut self, current_tick: u64, keep_ticks: u64) {
        self.entries
            .retain(|e| current_tick.saturating_sub(e.tick) < keep_ticks);
    }

    /// Get total entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is empty?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
