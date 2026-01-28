//! Per-tile historical records.
//!
//! Tracks what has happened at each tile over the course of the simulation:
//! ownership changes, events, settlements, and creature presence.

use serde::{Serialize, Deserialize};
use crate::history::{FactionId, SettlementId, EventId, PopulationId};
use crate::history::time::Date;

/// A record of faction ownership at a tile.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnershipRecord {
    pub faction: FactionId,
    pub gained: Date,
    pub lost: Option<Date>,
}

/// Historical data for a single world tile.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TileHistory {
    /// Ownership history (most recent last).
    pub ownership: Vec<OwnershipRecord>,
    /// Current owner, if any.
    pub current_owner: Option<FactionId>,

    /// Settlement that existed or exists here.
    pub settlement: Option<SettlementId>,
    /// Former settlements destroyed here.
    pub former_settlements: Vec<SettlementId>,

    /// Events that occurred at this tile.
    pub events: Vec<EventId>,

    /// Creature populations present here.
    pub populations: Vec<PopulationId>,
    
    /// Road infrastructure - permanent once built.
    pub has_road: bool,
}

impl TileHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set current ownership.
    pub fn set_owner(&mut self, faction: FactionId, date: Date) {
        // Close previous ownership
        if let Some(current) = self.current_owner {
            if let Some(last) = self.ownership.last_mut() {
                if last.faction == current && last.lost.is_none() {
                    last.lost = Some(date);
                }
            }
        }
        self.ownership.push(OwnershipRecord {
            faction,
            gained: date,
            lost: None,
        });
        self.current_owner = Some(faction);
    }

    /// Remove ownership (tile becomes unowned).
    pub fn remove_owner(&mut self, date: Date) {
        if let Some(last) = self.ownership.last_mut() {
            if last.lost.is_none() {
                last.lost = Some(date);
            }
        }
        self.current_owner = None;
    }

    /// Record an event at this tile.
    pub fn record_event(&mut self, event_id: EventId) {
        self.events.push(event_id);
    }

    /// How many times this tile has changed hands.
    pub fn ownership_changes(&self) -> usize {
        self.ownership.len()
    }

    /// Whether this tile has ever been settled.
    pub fn was_ever_settled(&self) -> bool {
        self.settlement.is_some() || !self.former_settlements.is_empty()
    }
}

/// A grid of tile histories for the entire world map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileHistoryMap {
    pub width: usize,
    pub height: usize,
    tiles: Vec<TileHistory>,
}

impl TileHistoryMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            tiles: vec![TileHistory::new(); width * height],
        }
    }

    /// Get a tile's history.
    pub fn get(&self, x: usize, y: usize) -> &TileHistory {
        &self.tiles[y * self.width + x]
    }

    /// Get a mutable reference to a tile's history.
    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut TileHistory {
        &mut self.tiles[y * self.width + x]
    }

    /// Set faction ownership for a tile.
    pub fn set_owner(&mut self, x: usize, y: usize, faction: FactionId, date: Date) {
        self.get_mut(x, y).set_owner(faction, date);
    }

    /// Record an event at a tile.
    pub fn record_event(&mut self, x: usize, y: usize, event_id: EventId) {
        self.get_mut(x, y).record_event(event_id);
    }
    
    /// Check if a tile has a road.
    pub fn has_road(&self, x: usize, y: usize) -> bool {
        self.get(x, y).has_road
    }
    
    /// Build a road on a tile (permanent).
    pub fn build_road(&mut self, x: usize, y: usize) {
        self.get_mut(x, y).has_road = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_tile_ownership() {
        let mut tile = TileHistory::new();
        let d1 = Date::new(10, Season::Spring);
        let d2 = Date::new(50, Season::Autumn);

        tile.set_owner(FactionId(0), d1);
        assert_eq!(tile.current_owner, Some(FactionId(0)));

        tile.set_owner(FactionId(1), d2);
        assert_eq!(tile.current_owner, Some(FactionId(1)));
        assert_eq!(tile.ownership_changes(), 2);

        // First record should be closed
        assert_eq!(tile.ownership[0].lost, Some(d2));
    }

    #[test]
    fn test_tile_history_map() {
        let mut map = TileHistoryMap::new(10, 10);
        let date = Date::new(1, Season::Spring);

        map.set_owner(5, 5, FactionId(0), date);
        assert_eq!(map.get(5, 5).current_owner, Some(FactionId(0)));
        assert!(map.get(0, 0).current_owner.is_none());

        map.record_event(5, 5, EventId(0));
        assert_eq!(map.get(5, 5).events.len(), 1);
    }
}
