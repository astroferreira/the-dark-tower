//! Tribe needs system - aggregate satisfaction levels

use serde::{Deserialize, Serialize};
use crate::simulation::params::SimulationParams;
use crate::simulation::resources::Stockpile;
use crate::simulation::types::ResourceType;

/// State of a single need
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct NeedState {
    /// Satisfaction level (0.0 = critical, 1.0 = fully satisfied)
    pub satisfaction: f32,
    /// Rate of change per tick
    pub trend: f32,
}

impl NeedState {
    pub fn new(satisfaction: f32) -> Self {
        NeedState {
            satisfaction: satisfaction.clamp(0.0, 1.0),
            trend: 0.0,
        }
    }

    pub fn update(&mut self, new_satisfaction: f32) {
        let new_sat = new_satisfaction.clamp(0.0, 1.0);
        self.trend = new_sat - self.satisfaction;
        self.satisfaction = new_sat;
    }

    pub fn is_critical(&self) -> bool {
        self.satisfaction < 0.2
    }

    pub fn is_poor(&self) -> bool {
        self.satisfaction < 0.4
    }

    pub fn is_good(&self) -> bool {
        self.satisfaction >= 0.7
    }
}

/// Aggregate needs for a tribe
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TribeNeeds {
    /// Food need - stockpile vs consumption
    pub food: NeedState,
    /// Water need - territory water access
    pub water: NeedState,
    /// Shelter need - buildings vs climate
    pub shelter: NeedState,
    /// Health need - sanitation, crowding
    pub health: NeedState,
    /// Morale need - composite satisfaction
    pub morale: NeedState,
    /// Security need - military strength
    pub security: NeedState,

    // Derived modifiers
    /// Affects population growth
    pub growth_modifier: f32,
    /// Affects resource production
    pub production_modifier: f32,
    /// Affects combat effectiveness
    pub military_modifier: f32,
}

impl TribeNeeds {
    pub fn new() -> Self {
        TribeNeeds {
            food: NeedState::new(0.5),
            water: NeedState::new(0.5),
            shelter: NeedState::new(0.5),
            health: NeedState::new(0.5),
            morale: NeedState::new(0.5),
            security: NeedState::new(0.5),
            growth_modifier: 1.0,
            production_modifier: 1.0,
            military_modifier: 1.0,
        }
    }

    /// Calculate all needs based on current state
    pub fn calculate(
        &mut self,
        population: u32,
        stockpile: &Stockpile,
        shelter_capacity: u32,
        health_bonus: f32,
        morale_bonus: f32,
        warriors: u32,
        params: &SimulationParams,
    ) {
        // Food satisfaction
        let food_needed = population as f32 * params.food_per_pop_per_tick * 10.0; // 10 ticks reserve
        let food_available = stockpile.get(ResourceType::Food);
        let food_sat = if food_needed > 0.0 {
            (food_available / food_needed).min(1.0)
        } else {
            1.0
        };
        self.food.update(food_sat);

        // Water satisfaction
        let water_needed = population as f32 * params.water_per_pop_per_tick * 10.0;
        let water_available = stockpile.get(ResourceType::Water);
        let water_sat = if water_needed > 0.0 {
            (water_available / water_needed).min(1.0)
        } else {
            1.0
        };
        self.water.update(water_sat);

        // Shelter satisfaction
        let shelter_sat = if population > 0 {
            (shelter_capacity as f32 / population as f32).min(1.0)
        } else {
            1.0
        };
        self.shelter.update(shelter_sat);

        // Health satisfaction (base from shelter + building bonuses)
        let base_health = (shelter_sat + self.food.satisfaction + self.water.satisfaction) / 3.0;
        let health_sat = (base_health + health_bonus).min(1.0);
        self.health.update(health_sat);

        // Security satisfaction
        let security_ratio = warriors as f32 / (population as f32 * params.security_safe).max(1.0);
        let security_sat = security_ratio.min(1.0);
        self.security.update(security_sat);

        // Morale is composite of all other needs + building bonuses
        let base_morale = (self.food.satisfaction
            + self.water.satisfaction
            + self.shelter.satisfaction
            + self.health.satisfaction
            + self.security.satisfaction)
            / 5.0;
        let morale_sat = (base_morale + morale_bonus).min(1.0);
        self.morale.update(morale_sat);

        // Calculate derived modifiers
        self.calculate_modifiers();
    }

    /// Calculate growth, production, and military modifiers
    fn calculate_modifiers(&mut self) {
        // Growth modifier: heavily affected by food, water, health
        let survival_needs = (self.food.satisfaction + self.water.satisfaction + self.health.satisfaction) / 3.0;
        self.growth_modifier = if survival_needs < 0.3 {
            0.0 // Population decline
        } else if survival_needs < 0.5 {
            0.5 // Reduced growth
        } else {
            survival_needs + (self.morale.satisfaction * 0.2) // Normal to boosted growth
        };

        // Production modifier: affected by morale, health, food
        self.production_modifier = 0.5 + (self.morale.satisfaction * 0.3) + (self.food.satisfaction * 0.2);

        // Military modifier: affected by morale, food, security
        self.military_modifier = 0.5 + (self.morale.satisfaction * 0.3) + (self.food.satisfaction * 0.2);
    }

    /// Get overall satisfaction (0.0-1.0)
    pub fn overall_satisfaction(&self) -> f32 {
        (self.food.satisfaction
            + self.water.satisfaction
            + self.shelter.satisfaction
            + self.health.satisfaction
            + self.morale.satisfaction
            + self.security.satisfaction)
            / 6.0
    }

    /// Check if any critical need is unmet
    pub fn has_critical_need(&self) -> bool {
        self.food.is_critical()
            || self.water.is_critical()
            || self.shelter.is_critical()
            || self.health.is_critical()
    }

    /// Get the most critical need
    pub fn most_critical_need(&self) -> (&'static str, f32) {
        let needs = [
            ("food", self.food.satisfaction),
            ("water", self.water.satisfaction),
            ("shelter", self.shelter.satisfaction),
            ("health", self.health.satisfaction),
            ("morale", self.morale.satisfaction),
            ("security", self.security.satisfaction),
        ];

        needs
            .into_iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or(("food", 0.0))
    }

    /// Check if the tribe is starving
    pub fn is_starving(&self) -> bool {
        self.food.satisfaction < 0.2
    }

    /// Check if the tribe has good conditions for growth
    pub fn can_grow(&self) -> bool {
        self.food.satisfaction > 0.5 && self.water.satisfaction > 0.5 && self.health.satisfaction > 0.4
    }
}
