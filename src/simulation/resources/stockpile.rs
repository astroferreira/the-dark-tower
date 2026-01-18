//! Resource stockpile management for tribes

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::simulation::types::ResourceType;

/// Resource storage for a tribe
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stockpile {
    /// Current resource amounts
    resources: HashMap<ResourceType, f32>,
    /// Maximum storage capacity per resource type
    capacity: HashMap<ResourceType, f32>,
}

impl Stockpile {
    pub fn new() -> Self {
        Stockpile {
            resources: HashMap::new(),
            capacity: HashMap::new(),
        }
    }

    /// Create stockpile with initial resources
    pub fn with_initial(food: f32, water: f32, wood: f32, stone: f32) -> Self {
        let mut stockpile = Self::new();
        stockpile.add(ResourceType::Food, food);
        stockpile.add(ResourceType::Water, water);
        stockpile.add(ResourceType::Wood, wood);
        stockpile.add(ResourceType::Stone, stone);
        stockpile
    }

    /// Get current amount of a resource
    pub fn get(&self, resource: ResourceType) -> f32 {
        *self.resources.get(&resource).unwrap_or(&0.0)
    }

    /// Add resources, respecting capacity limits
    pub fn add(&mut self, resource: ResourceType, amount: f32) -> f32 {
        let current = self.get(resource);
        let cap = self.get_capacity(resource);
        let new_amount = (current + amount).min(cap);
        let actually_added = new_amount - current;
        self.resources.insert(resource, new_amount);
        actually_added
    }

    /// Remove resources, returns actual amount removed
    pub fn remove(&mut self, resource: ResourceType, amount: f32) -> f32 {
        let current = self.get(resource);
        let to_remove = amount.min(current);
        let new_amount = current - to_remove;
        if new_amount > 0.0 {
            self.resources.insert(resource, new_amount);
        } else {
            self.resources.remove(&resource);
        }
        to_remove
    }

    /// Check if we have at least the specified amount
    pub fn has(&self, resource: ResourceType, amount: f32) -> bool {
        self.get(resource) >= amount
    }

    /// Try to consume resources, returns true if successful
    pub fn consume(&mut self, resource: ResourceType, amount: f32) -> bool {
        if self.has(resource, amount) {
            self.remove(resource, amount);
            true
        } else {
            false
        }
    }

    /// Set capacity for a resource type
    pub fn set_capacity(&mut self, resource: ResourceType, capacity: f32) {
        self.capacity.insert(resource, capacity);
        // Clamp current amount to new capacity
        if let Some(current) = self.resources.get_mut(&resource) {
            *current = current.min(capacity);
        }
    }

    /// Get capacity for a resource type (default: unlimited)
    pub fn get_capacity(&self, resource: ResourceType) -> f32 {
        *self.capacity.get(&resource).unwrap_or(&f32::MAX)
    }

    /// Update capacity based on population
    pub fn update_capacity_for_population(&mut self, population: u32, per_pop_multiplier: f32) {
        let base_cap = population as f32 * per_pop_multiplier;

        // Basic resources have standard capacity
        self.set_capacity(ResourceType::Food, base_cap);
        self.set_capacity(ResourceType::Water, base_cap);
        self.set_capacity(ResourceType::Wood, base_cap * 0.5);
        self.set_capacity(ResourceType::Stone, base_cap * 0.5);

        // Metals have lower capacity
        for metal in [
            ResourceType::Copper,
            ResourceType::Tin,
            ResourceType::Bronze,
            ResourceType::Iron,
            ResourceType::Gold,
            ResourceType::Silver,
        ] {
            self.set_capacity(metal, base_cap * 0.2);
        }

        // Luxury goods have lowest capacity
        for luxury in [
            ResourceType::Gems,
            ResourceType::Spices,
            ResourceType::Salt,
            ResourceType::Obsidian,
        ] {
            self.set_capacity(luxury, base_cap * 0.1);
        }
    }

    /// Apply decay to perishable resources
    pub fn apply_decay(&mut self) {
        let decayable: Vec<_> = self.resources.keys().copied().collect();
        for resource in decayable {
            let decay_rate = resource.decay_rate();
            if decay_rate > 0.0 {
                let current = self.get(resource);
                let lost = current * decay_rate;
                self.remove(resource, lost);
            }
        }
    }

    /// Get total trade value of stockpile
    pub fn total_value(&self) -> f32 {
        self.resources
            .iter()
            .map(|(r, &amt)| amt * r.trade_value())
            .sum()
    }

    /// Calculate scarcity of a resource (0.0 = abundant, 1.0 = completely lacking)
    pub fn scarcity(&self, resource: ResourceType, needed_per_tick: f32, ticks_reserve: f32) -> f32 {
        let current = self.get(resource);
        let needed = needed_per_tick * ticks_reserve;
        if needed <= 0.0 {
            0.0
        } else {
            (1.0 - current / needed).clamp(0.0, 1.0)
        }
    }

    /// Iterate over all resources with amounts > 0
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceType, &f32)> {
        self.resources.iter()
    }

    /// Get all resource types currently stored
    pub fn resource_types(&self) -> Vec<ResourceType> {
        self.resources.keys().copied().collect()
    }

    /// Check if stockpile is empty
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty() || self.resources.values().all(|&v| v <= 0.0)
    }

    /// Transfer resources from another stockpile
    pub fn transfer_from(&mut self, other: &mut Stockpile, resource: ResourceType, amount: f32) -> f32 {
        let removed = other.remove(resource, amount);
        self.add(resource, removed)
    }

    /// Take a fraction of all resources (for looting/splitting)
    pub fn take_fraction(&mut self, fraction: f32) -> HashMap<ResourceType, f32> {
        let mut taken = HashMap::new();
        let resources: Vec<_> = self.resources.keys().copied().collect();
        for resource in resources {
            let amount = self.get(resource) * fraction;
            let removed = self.remove(resource, amount);
            if removed > 0.0 {
                taken.insert(resource, removed);
            }
        }
        taken
    }

    /// Add resources from a hashmap
    pub fn add_all(&mut self, resources: &HashMap<ResourceType, f32>) {
        for (&resource, &amount) in resources {
            self.add(resource, amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut stockpile = Stockpile::new();
        stockpile.set_capacity(ResourceType::Food, 100.0);

        assert_eq!(stockpile.get(ResourceType::Food), 0.0);
        stockpile.add(ResourceType::Food, 50.0);
        assert_eq!(stockpile.get(ResourceType::Food), 50.0);

        assert!(stockpile.has(ResourceType::Food, 50.0));
        assert!(!stockpile.has(ResourceType::Food, 51.0));

        stockpile.remove(ResourceType::Food, 20.0);
        assert_eq!(stockpile.get(ResourceType::Food), 30.0);
    }

    #[test]
    fn test_capacity_limits() {
        let mut stockpile = Stockpile::new();
        stockpile.set_capacity(ResourceType::Food, 100.0);

        stockpile.add(ResourceType::Food, 150.0);
        assert_eq!(stockpile.get(ResourceType::Food), 100.0);
    }

    #[test]
    fn test_decay() {
        let mut stockpile = Stockpile::new();
        stockpile.add(ResourceType::Food, 100.0);
        stockpile.apply_decay();
        assert!(stockpile.get(ResourceType::Food) < 100.0);

        // Stone doesn't decay
        stockpile.add(ResourceType::Stone, 100.0);
        stockpile.apply_decay();
        assert_eq!(stockpile.get(ResourceType::Stone), 100.0);
    }
}
