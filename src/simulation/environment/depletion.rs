//! Resource depletion tracking

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::simulation::types::{TileCoord, ResourceType};

/// Tracks resource depletion across the map
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceDepletion {
    /// Depletion level per tile per resource (0.0 = full, 1.0 = depleted)
    depletion: HashMap<(TileCoord, ResourceType), f32>,
    /// Regeneration rate per tick
    regen_rate: f32,
}

impl ResourceDepletion {
    pub fn new() -> Self {
        ResourceDepletion {
            depletion: HashMap::new(),
            regen_rate: 0.001, // 0.1% regeneration per tick
        }
    }

    /// Get depletion level for a tile/resource (0.0 = full, 1.0 = depleted)
    pub fn get_depletion(&self, coord: TileCoord, resource: ResourceType) -> f32 {
        *self.depletion.get(&(coord, resource)).unwrap_or(&0.0)
    }

    /// Get extraction multiplier (1.0 = full yield, 0.0 = no yield)
    pub fn get_yield_multiplier(&self, coord: TileCoord, resource: ResourceType) -> f32 {
        1.0 - self.get_depletion(coord, resource)
    }

    /// Add depletion from extraction
    pub fn add_depletion(&mut self, coord: TileCoord, resource: ResourceType, amount: f32) {
        let key = (coord, resource);
        let current = *self.depletion.get(&key).unwrap_or(&0.0);
        let new_value = (current + amount).min(1.0);

        if new_value > 0.0 {
            self.depletion.insert(key, new_value);
        }
    }

    /// Apply regeneration to all depleted tiles
    pub fn tick_regeneration(&mut self) {
        let keys: Vec<(TileCoord, ResourceType)> = self.depletion.keys().copied().collect();

        for key in keys {
            if let Some(depletion) = self.depletion.get_mut(&key) {
                *depletion = (*depletion - self.regen_rate).max(0.0);

                // Remove if fully regenerated
                if *depletion <= 0.0 {
                    self.depletion.remove(&key);
                }
            }
        }
    }

    /// Check if a resource is significantly depleted
    pub fn is_depleted(&self, coord: TileCoord, resource: ResourceType) -> bool {
        self.get_depletion(coord, resource) > 0.8
    }

    /// Get total depleted tiles count
    pub fn depleted_tile_count(&self) -> usize {
        self.depletion.values().filter(|&&v| v > 0.5).count()
    }

    /// Get depletion statistics
    pub fn stats(&self) -> DepletionStats {
        let mut stats = DepletionStats::default();

        for &depletion in self.depletion.values() {
            if depletion > 0.8 {
                stats.severely_depleted += 1;
            } else if depletion > 0.5 {
                stats.moderately_depleted += 1;
            } else if depletion > 0.2 {
                stats.lightly_depleted += 1;
            }
        }

        stats.total_tracked = self.depletion.len();
        stats
    }
}

/// Statistics about resource depletion
#[derive(Clone, Debug, Default)]
pub struct DepletionStats {
    pub total_tracked: usize,
    pub severely_depleted: usize,
    pub moderately_depleted: usize,
    pub lightly_depleted: usize,
}
