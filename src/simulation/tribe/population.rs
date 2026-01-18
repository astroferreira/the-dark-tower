//! Population management for tribes

use serde::{Deserialize, Serialize};
use crate::simulation::params::SimulationParams;
use crate::simulation::tribe::TribeNeeds;

/// Population structure for a tribe
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Population {
    /// Total population
    total: u32,
    /// Fraction of population that are warriors (0.0-1.0)
    warrior_ratio: f32,
    /// Births this tick
    births: u32,
    /// Deaths this tick
    deaths: u32,
    /// Migration pressure (positive = wants to expand, negative = happy)
    migration_pressure: f32,
}

impl Population {
    pub fn new(total: u32) -> Self {
        Population {
            total,
            warrior_ratio: 0.1, // 10% warriors by default
            births: 0,
            deaths: 0,
            migration_pressure: 0.0,
        }
    }

    /// Get total population
    pub fn total(&self) -> u32 {
        self.total
    }

    /// Get warrior count
    pub fn warriors(&self) -> u32 {
        (self.total as f32 * self.warrior_ratio) as u32
    }

    /// Get worker count (non-warriors)
    pub fn workers(&self) -> u32 {
        self.total - self.warriors()
    }

    /// Set warrior ratio
    pub fn set_warrior_ratio(&mut self, ratio: f32) {
        self.warrior_ratio = ratio.clamp(0.0, 0.5); // Max 50% warriors
    }

    /// Get current warrior ratio
    pub fn warrior_ratio(&self) -> f32 {
        self.warrior_ratio
    }

    /// Get births this tick
    pub fn births(&self) -> u32 {
        self.births
    }

    /// Get deaths this tick
    pub fn deaths(&self) -> u32 {
        self.deaths
    }

    /// Get migration pressure
    pub fn migration_pressure(&self) -> f32 {
        self.migration_pressure
    }

    /// Add population (from births or immigration)
    pub fn add(&mut self, amount: u32) {
        self.total += amount;
    }

    /// Remove population (from deaths or emigration)
    pub fn remove(&mut self, amount: u32) {
        self.total = self.total.saturating_sub(amount);
    }

    /// Calculate and apply population changes for a tick
    pub fn tick(
        &mut self,
        needs: &TribeNeeds,
        territory_size: usize,
        params: &SimulationParams,
    ) -> PopulationChange {
        let pop = self.total as f32;

        // Calculate growth rate based on needs
        let effective_growth_rate = if needs.can_grow() {
            params.base_growth_rate * needs.growth_modifier
        } else if needs.is_starving() {
            -params.base_death_rate * 2.0 // Famine deaths
        } else {
            0.0 // Stagnant
        };

        // Calculate base deaths (always some natural deaths)
        let base_deaths = (pop * params.base_death_rate) as u32;

        // Calculate births
        let births = if effective_growth_rate > 0.0 {
            ((pop * effective_growth_rate).max(0.0)) as u32
        } else {
            0
        };

        // Additional deaths from critical needs
        let crisis_deaths = if needs.has_critical_need() {
            let severity = 1.0 - needs.overall_satisfaction();
            (pop * params.base_death_rate * severity * 2.0) as u32
        } else {
            0
        };

        let total_deaths = base_deaths + crisis_deaths;

        // Apply changes
        self.births = births;
        self.deaths = total_deaths;
        self.add(births);
        self.remove(total_deaths);

        // Calculate migration pressure
        let ideal_pop_for_territory = territory_size as f32 * params.pop_per_territory_tile;
        self.migration_pressure = if ideal_pop_for_territory > 0.0 {
            (self.total as f32 - ideal_pop_for_territory) / ideal_pop_for_territory
        } else {
            1.0
        };

        // Reduce morale with overpopulation
        let overcrowding = self.migration_pressure.max(0.0);

        PopulationChange {
            births,
            deaths: total_deaths,
            net_change: births as i32 - total_deaths as i32,
            growth_rate: effective_growth_rate,
            migration_pressure: self.migration_pressure,
            is_starving: needs.is_starving(),
            is_overcrowded: overcrowding > 0.5,
        }
    }

    /// Split population for tribe splitting
    pub fn split(&mut self, fraction: f32) -> Population {
        let split_amount = (self.total as f32 * fraction) as u32;
        self.remove(split_amount);

        let mut new_pop = Population::new(split_amount);
        new_pop.warrior_ratio = self.warrior_ratio;
        new_pop
    }

    /// Take warriors from population (for war parties)
    pub fn take_warriors(&mut self, count: u32) -> u32 {
        let available = self.warriors();
        let taken = count.min(available);
        self.remove(taken);
        taken
    }

    /// Apply casualties (affects warriors first)
    pub fn apply_casualties(&mut self, count: u32) {
        let warriors = self.warriors();
        if count <= warriors {
            // All casualties from warriors
            self.remove(count);
        } else {
            // Some civilian casualties too
            self.remove(count);
        }
    }

    /// Check if population is viable
    pub fn is_viable(&self) -> bool {
        self.total >= 10
    }

    /// Calculate population density for a territory size
    pub fn density(&self, territory_tiles: usize) -> f32 {
        if territory_tiles > 0 {
            self.total as f32 / territory_tiles as f32
        } else {
            f32::MAX
        }
    }
}

/// Result of a population tick
#[derive(Clone, Debug)]
pub struct PopulationChange {
    pub births: u32,
    pub deaths: u32,
    pub net_change: i32,
    pub growth_rate: f32,
    pub migration_pressure: f32,
    pub is_starving: bool,
    pub is_overcrowded: bool,
}

impl PopulationChange {
    pub fn is_declining(&self) -> bool {
        self.net_change < 0
    }

    pub fn is_growing(&self) -> bool {
        self.net_change > 0
    }
}
