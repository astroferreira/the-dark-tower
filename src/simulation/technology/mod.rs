//! Technology system for civilization progression

pub mod ages;
pub mod unlocks;

use std::collections::HashSet;
use serde::{Deserialize, Serialize};

pub use ages::{Age, AgeRequirements};
pub use unlocks::{BuildingType, TechUnlock};

/// Technology state for a tribe
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TechnologyState {
    /// Current technological age
    current_age: Age,
    /// Accumulated research points
    research_points: f32,
    /// Unlocked technologies/discoveries
    unlocked_techs: HashSet<String>,
    /// Unlocked building types
    unlocked_buildings: HashSet<BuildingType>,
}

impl Default for TechnologyState {
    fn default() -> Self {
        Self::new()
    }
}

impl TechnologyState {
    pub fn new() -> Self {
        let mut unlocked_buildings = HashSet::new();
        // Stone Age buildings are available by default
        unlocked_buildings.insert(BuildingType::Hut);
        unlocked_buildings.insert(BuildingType::Campfire);
        unlocked_buildings.insert(BuildingType::StoragePit);

        TechnologyState {
            current_age: Age::Stone,
            research_points: 0.0,
            unlocked_techs: HashSet::new(),
            unlocked_buildings,
        }
    }

    /// Get current age
    pub fn current_age(&self) -> Age {
        self.current_age
    }

    /// Get accumulated research points
    pub fn research_points(&self) -> f32 {
        self.research_points
    }

    /// Add research points
    pub fn add_research(&mut self, points: f32) {
        self.research_points += points;
    }

    /// Check if a tech is unlocked
    pub fn has_tech(&self, tech: &str) -> bool {
        self.unlocked_techs.contains(tech)
    }

    /// Unlock a technology
    pub fn unlock_tech(&mut self, tech: String) {
        self.unlocked_techs.insert(tech);
    }

    /// Check if a building type is unlocked
    pub fn can_build(&self, building: BuildingType) -> bool {
        self.unlocked_buildings.contains(&building)
    }

    /// Unlock a building type
    pub fn unlock_building(&mut self, building: BuildingType) {
        self.unlocked_buildings.insert(building);
    }

    /// Get unlocked buildings
    pub fn unlocked_buildings(&self) -> &HashSet<BuildingType> {
        &self.unlocked_buildings
    }

    /// Check if tribe can advance to next age
    pub fn can_advance(
        &self,
        population: u32,
        has_building: impl Fn(&str) -> bool,
        has_resource: impl Fn(crate::simulation::types::ResourceType, f32) -> bool,
    ) -> bool {
        let Some(next_age) = self.current_age.next() else {
            return false;
        };

        let requirements = AgeRequirements::for_age(next_age);

        // Check population
        if population < requirements.min_population {
            return false;
        }

        // Check research points
        if self.research_points < requirements.research_points {
            return false;
        }

        // Check buildings
        for building in &requirements.required_buildings {
            if !has_building(building) {
                return false;
            }
        }

        // Check resources
        for &(resource, amount) in &requirements.required_resources {
            if !has_resource(resource, amount) {
                return false;
            }
        }

        true
    }

    /// Advance to the next age
    pub fn advance_age(&mut self) -> Option<Age> {
        if let Some(next_age) = self.current_age.next() {
            self.current_age = next_age;

            // Unlock buildings for the new age
            for building in TechUnlock::buildings_for_age(next_age) {
                self.unlocked_buildings.insert(building);
            }

            Some(next_age)
        } else {
            None
        }
    }

    /// Military multiplier from technology
    pub fn military_multiplier(&self) -> f32 {
        let base = self.current_age.military_multiplier();

        // Bonus from specific techs
        let tech_bonus = if self.has_tech("IronWeapons") {
            1.2
        } else if self.has_tech("BronzeWeapons") {
            1.1
        } else {
            1.0
        };

        base * tech_bonus
    }

    /// Production multiplier from technology
    pub fn production_multiplier(&self) -> f32 {
        let base = self.current_age.production_multiplier();

        // Bonus from specific techs
        let tech_bonus = if self.has_tech("AdvancedTools") {
            1.2
        } else if self.has_tech("MetalTools") {
            1.1
        } else {
            1.0
        };

        base * tech_bonus
    }

    /// Research cost for advancing to next age
    pub fn research_cost_for_next_age(&self) -> Option<f32> {
        self.current_age.next().map(|next| {
            AgeRequirements::for_age(next).research_points
        })
    }

    /// Progress towards next age (0.0 - 1.0)
    pub fn age_progress(&self) -> f32 {
        if let Some(cost) = self.research_cost_for_next_age() {
            (self.research_points / cost).min(1.0)
        } else {
            1.0 // Max age reached
        }
    }

    /// Get list of unlocked tech names
    pub fn unlocked_techs(&self) -> Vec<&str> {
        self.unlocked_techs.iter().map(|s| s.as_str()).collect()
    }
}
