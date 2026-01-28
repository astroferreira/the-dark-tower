//! Territory management for factions.

use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use crate::history::FactionId;

/// Territory tracker for a faction.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Territory {
    /// All tiles controlled by this faction.
    pub tiles: HashSet<(usize, usize)>,
}

impl Territory {
    pub fn new() -> Self {
        Self { tiles: HashSet::new() }
    }

    /// Claim a tile.
    pub fn claim(&mut self, x: usize, y: usize) {
        self.tiles.insert((x, y));
    }

    /// Release a tile.
    pub fn release(&mut self, x: usize, y: usize) {
        self.tiles.remove(&(x, y));
    }

    /// Check if a tile is claimed.
    pub fn contains(&self, x: usize, y: usize) -> bool {
        self.tiles.contains(&(x, y))
    }

    /// Total number of tiles controlled.
    pub fn size(&self) -> usize {
        self.tiles.len()
    }

    /// Check if this territory borders another.
    pub fn borders(&self, other: &Territory) -> bool {
        for &(x, y) in &self.tiles {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    if other.contains(nx, ny) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get boundary tiles (tiles adjacent to non-owned tiles).
    pub fn border_tiles(&self, map_width: usize, map_height: usize) -> Vec<(usize, usize)> {
        let mut borders = Vec::new();
        for &(x, y) in &self.tiles {
            let mut is_border = false;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = (x as i32 + dx).rem_euclid(map_width as i32) as usize;
                    let ny = (y as i32 + dy).clamp(0, map_height as i32 - 1) as usize;
                    if !self.tiles.contains(&(nx, ny)) {
                        is_border = true;
                        break;
                    }
                }
                if is_border { break; }
            }
            if is_border {
                borders.push((x, y));
            }
        }
        borders
    }
}

/// World-level territory map tracking which faction owns each tile.
#[derive(Clone, Debug)]
pub struct TerritoryMap {
    width: usize,
    height: usize,
    owners: Vec<Option<FactionId>>,
}

impl TerritoryMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            owners: vec![None; width * height],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<FactionId> {
        if x < self.width && y < self.height {
            self.owners[y * self.width + x]
        } else {
            None
        }
    }

    pub fn set(&mut self, x: usize, y: usize, faction: Option<FactionId>) {
        if x < self.width && y < self.height {
            self.owners[y * self.width + x] = faction;
        }
    }

    /// Count tiles owned by a faction.
    pub fn count_tiles(&self, faction: FactionId) -> usize {
        self.owners.iter().filter(|&&o| o == Some(faction)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_territory_claim() {
        let mut t = Territory::new();
        t.claim(5, 5);
        t.claim(5, 6);
        assert!(t.contains(5, 5));
        assert!(!t.contains(10, 10));
        assert_eq!(t.size(), 2);
        t.release(5, 5);
        assert_eq!(t.size(), 1);
    }

    #[test]
    fn test_territory_borders() {
        let mut a = Territory::new();
        a.claim(5, 5);
        let mut b = Territory::new();
        b.claim(6, 5);
        assert!(a.borders(&b));

        let mut c = Territory::new();
        c.claim(100, 100);
        assert!(!a.borders(&c));
    }

    #[test]
    fn test_territory_map() {
        let mut map = TerritoryMap::new(10, 10);
        map.set(3, 3, Some(FactionId(0)));
        map.set(3, 4, Some(FactionId(0)));
        map.set(5, 5, Some(FactionId(1)));
        assert_eq!(map.count_tiles(FactionId(0)), 2);
        assert_eq!(map.count_tiles(FactionId(1)), 1);
    }
}
