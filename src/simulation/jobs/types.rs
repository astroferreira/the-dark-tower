//! Job types and definitions for the colony simulation
//!
//! Defines all available jobs, their categories, and requirements.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::simulation::types::ResourceType;
use crate::simulation::colonists::skills::SkillType;

/// Unique identifier for a job instance
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Job#{}", self.0)
    }
}

/// Categories of jobs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobCategory {
    /// Basic survival jobs (farming, hunting, fishing, cooking)
    Essential,
    /// Resource gathering (mining, woodcutting)
    Production,
    /// Building and crafting
    Construction,
    /// Military and defense
    Military,
    /// Knowledge and research
    Research,
    /// Support roles (hauling, healing)
    Service,
}

impl JobCategory {
    /// Get all categories
    pub fn all() -> &'static [JobCategory] {
        &[
            JobCategory::Essential,
            JobCategory::Production,
            JobCategory::Construction,
            JobCategory::Military,
            JobCategory::Research,
            JobCategory::Service,
        ]
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            JobCategory::Essential => "Essential",
            JobCategory::Production => "Production",
            JobCategory::Construction => "Construction",
            JobCategory::Military => "Military",
            JobCategory::Research => "Research",
            JobCategory::Service => "Service",
        }
    }

    /// Base priority for this category (higher = more important)
    pub fn base_priority(&self) -> u32 {
        match self {
            JobCategory::Essential => 100,
            JobCategory::Military => 80,
            JobCategory::Production => 60,
            JobCategory::Construction => 50,
            JobCategory::Research => 40,
            JobCategory::Service => 30,
        }
    }
}

/// Types of jobs available
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobType {
    // Essential
    Farmer,
    Hunter,
    Fisher,
    Cook,

    // Production
    Miner,
    Woodcutter,
    Smith,
    Craftsperson,

    // Construction
    Builder,

    // Military
    Guard,
    Warrior,
    Scout,

    // Research
    Scholar,
    Priest,

    // Service
    Healer,
    Hauler,

    // Default
    Idle,
}

impl JobType {
    /// Get all job types
    pub fn all() -> &'static [JobType] {
        &[
            JobType::Farmer,
            JobType::Hunter,
            JobType::Fisher,
            JobType::Cook,
            JobType::Miner,
            JobType::Woodcutter,
            JobType::Smith,
            JobType::Craftsperson,
            JobType::Builder,
            JobType::Guard,
            JobType::Warrior,
            JobType::Scout,
            JobType::Scholar,
            JobType::Priest,
            JobType::Healer,
            JobType::Hauler,
            JobType::Idle,
        ]
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            JobType::Farmer => "Farmer",
            JobType::Hunter => "Hunter",
            JobType::Fisher => "Fisher",
            JobType::Cook => "Cook",
            JobType::Miner => "Miner",
            JobType::Woodcutter => "Woodcutter",
            JobType::Smith => "Smith",
            JobType::Craftsperson => "Craftsperson",
            JobType::Builder => "Builder",
            JobType::Guard => "Guard",
            JobType::Warrior => "Warrior",
            JobType::Scout => "Scout",
            JobType::Scholar => "Scholar",
            JobType::Priest => "Priest",
            JobType::Healer => "Healer",
            JobType::Hauler => "Hauler",
            JobType::Idle => "Idle",
        }
    }

    /// Get the category for this job
    pub fn category(&self) -> JobCategory {
        match self {
            JobType::Farmer | JobType::Hunter | JobType::Fisher | JobType::Cook =>
                JobCategory::Essential,
            JobType::Miner | JobType::Woodcutter | JobType::Smith | JobType::Craftsperson =>
                JobCategory::Production,
            JobType::Builder => JobCategory::Construction,
            JobType::Guard | JobType::Warrior | JobType::Scout => JobCategory::Military,
            JobType::Scholar | JobType::Priest => JobCategory::Research,
            JobType::Healer | JobType::Hauler | JobType::Idle => JobCategory::Service,
        }
    }

    /// Get the primary skill used by this job
    pub fn primary_skill(&self) -> Option<SkillType> {
        match self {
            JobType::Farmer => Some(SkillType::Farming),
            JobType::Hunter => Some(SkillType::Hunting),
            JobType::Fisher => Some(SkillType::Fishing),
            JobType::Cook => Some(SkillType::Cooking),
            JobType::Miner => Some(SkillType::Mining),
            JobType::Woodcutter => Some(SkillType::Woodcutting),
            JobType::Smith => Some(SkillType::Smithing),
            JobType::Craftsperson => Some(SkillType::Crafting),
            JobType::Builder => Some(SkillType::Building),
            JobType::Guard | JobType::Warrior | JobType::Scout => Some(SkillType::Combat),
            JobType::Scholar => Some(SkillType::Research),
            JobType::Priest => Some(SkillType::Research),
            JobType::Healer => Some(SkillType::Medicine),
            JobType::Hauler | JobType::Idle => None,
        }
    }

    /// Get the resources produced by this job (if any)
    pub fn produces(&self) -> &'static [(ResourceType, f32)] {
        match self {
            JobType::Farmer => &[(ResourceType::Food, 2.0)],
            JobType::Hunter => &[(ResourceType::Food, 1.5), (ResourceType::Leather, 0.5)],
            JobType::Fisher => &[(ResourceType::Food, 1.5)],
            JobType::Miner => &[(ResourceType::Stone, 1.5), (ResourceType::Iron, 0.3)],
            JobType::Woodcutter => &[(ResourceType::Wood, 2.0)],
            JobType::Smith => &[(ResourceType::Tools, 0.5), (ResourceType::Weapons, 0.3)],
            JobType::Craftsperson => &[(ResourceType::Cloth, 0.5), (ResourceType::Tools, 0.3)],
            _ => &[],
        }
    }

    /// Get resources consumed by this job per tick
    pub fn consumes(&self) -> &'static [(ResourceType, f32)] {
        match self {
            JobType::Smith => &[(ResourceType::Iron, 0.5), (ResourceType::Coal, 0.3)],
            JobType::Craftsperson => &[(ResourceType::Wood, 0.2), (ResourceType::Leather, 0.1)],
            JobType::Cook => &[(ResourceType::Food, 0.5)], // Raw food in, cooked food out
            _ => &[],
        }
    }

    /// Base productivity for this job
    pub fn base_productivity(&self) -> f32 {
        match self {
            JobType::Farmer => 1.0,
            JobType::Hunter => 0.8,
            JobType::Fisher => 0.9,
            JobType::Cook => 1.0,
            JobType::Miner => 0.8,
            JobType::Woodcutter => 1.0,
            JobType::Smith => 0.5,
            JobType::Craftsperson => 0.6,
            JobType::Builder => 0.5,
            JobType::Guard => 0.0,      // Guards don't produce resources
            JobType::Warrior => 0.0,
            JobType::Scout => 0.0,
            JobType::Scholar => 1.0,    // Research points
            JobType::Priest => 0.5,     // Morale + some research
            JobType::Healer => 0.5,     // Healing output
            JobType::Hauler => 0.5,     // Transfer efficiency
            JobType::Idle => 0.0,
        }
    }

    /// Does this job require a specific building?
    pub fn requires_building(&self) -> Option<&'static str> {
        match self {
            JobType::Smith => Some("Smithy"),
            JobType::Scholar => Some("Library"),
            JobType::Priest => Some("Temple"),
            JobType::Healer => Some("Hospital"),
            _ => None,
        }
    }

    /// Get jobs suitable for a specific need
    pub fn jobs_for_resource(resource: ResourceType) -> Vec<JobType> {
        match resource {
            ResourceType::Food => vec![JobType::Farmer, JobType::Hunter, JobType::Fisher],
            ResourceType::Wood => vec![JobType::Woodcutter],
            ResourceType::Stone => vec![JobType::Miner],
            ResourceType::Iron | ResourceType::Copper => vec![JobType::Miner],
            ResourceType::Tools | ResourceType::Weapons => vec![JobType::Smith],
            ResourceType::Leather => vec![JobType::Hunter],
            ResourceType::Cloth => vec![JobType::Craftsperson],
            _ => vec![],
        }
    }
}

impl Default for JobType {
    fn default() -> Self {
        JobType::Idle
    }
}

/// A job instance with assigned workers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub job_type: JobType,
    /// Priority (higher = more important)
    pub priority: u32,
    /// Number of workers wanted for this job
    pub workers_wanted: u32,
    /// IDs of notable colonists assigned
    pub assigned_notables: Vec<crate::simulation::colonists::types::ColonistId>,
    /// Number of pool workers assigned
    pub pool_workers: u32,
    /// Efficiency modifier for this specific job
    pub efficiency_modifier: f32,
    /// Is this job currently active?
    pub is_active: bool,
}

impl Job {
    pub fn new(id: JobId, job_type: JobType) -> Self {
        Job {
            id,
            job_type,
            priority: job_type.category().base_priority(),
            workers_wanted: 1,
            assigned_notables: Vec::new(),
            pool_workers: 0,
            efficiency_modifier: 1.0,
            is_active: true,
        }
    }

    /// Get total workers (notables + pool)
    pub fn total_workers(&self) -> u32 {
        self.assigned_notables.len() as u32 + self.pool_workers
    }

    /// Check if job is fully staffed
    pub fn is_fully_staffed(&self) -> bool {
        self.total_workers() >= self.workers_wanted
    }

    /// How many more workers are needed?
    pub fn workers_needed(&self) -> u32 {
        self.workers_wanted.saturating_sub(self.total_workers())
    }
}

/// Job requirements and demands for a tribe
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JobDemand {
    pub farmers_needed: u32,
    pub hunters_needed: u32,
    pub fishers_needed: u32,
    pub miners_needed: u32,
    pub woodcutters_needed: u32,
    pub builders_needed: u32,
    pub guards_needed: u32,
    pub warriors_needed: u32,
    pub scholars_needed: u32,
    pub healers_needed: u32,
    pub smiths_needed: u32,
    pub craftspeople_needed: u32,
}

impl JobDemand {
    /// Calculate job demand based on tribe needs and resources
    pub fn calculate(
        population: u32,
        food_satisfaction: f32,
        security_satisfaction: f32,
        has_mines: bool,
        has_forests: bool,
        has_water: bool,
        at_war: bool,
        needs_buildings: bool,
    ) -> Self {
        let pop_factor = (population as f32 / 100.0).max(1.0);

        // Base food workers
        let food_workers = if food_satisfaction < 0.5 {
            (pop_factor * 3.0) as u32 // More food workers when hungry
        } else {
            (pop_factor * 1.5) as u32
        };

        // Military needs
        let military_base = if at_war {
            (pop_factor * 2.0) as u32
        } else if security_satisfaction < 0.5 {
            (pop_factor * 1.0) as u32
        } else {
            (pop_factor * 0.5) as u32
        };

        JobDemand {
            farmers_needed: (food_workers * 2 / 3).max(1),
            hunters_needed: if has_forests { food_workers / 4 } else { 0 },
            fishers_needed: if has_water { food_workers / 4 } else { 0 },
            miners_needed: if has_mines { (pop_factor * 0.5) as u32 } else { 0 },
            woodcutters_needed: if has_forests { (pop_factor * 0.5) as u32 } else { 0 },
            builders_needed: if needs_buildings { (pop_factor * 0.3) as u32 } else { 0 },
            guards_needed: military_base / 2,
            warriors_needed: military_base / 2,
            scholars_needed: (pop_factor * 0.1) as u32,
            healers_needed: (pop_factor * 0.1) as u32,
            smiths_needed: (pop_factor * 0.1) as u32,
            craftspeople_needed: (pop_factor * 0.1) as u32,
        }
    }

    /// Get total workers demanded
    pub fn total(&self) -> u32 {
        self.farmers_needed + self.hunters_needed + self.fishers_needed +
        self.miners_needed + self.woodcutters_needed + self.builders_needed +
        self.guards_needed + self.warriors_needed + self.scholars_needed +
        self.healers_needed + self.smiths_needed + self.craftspeople_needed
    }

    /// Get demand for a specific job type
    pub fn get(&self, job_type: JobType) -> u32 {
        match job_type {
            JobType::Farmer => self.farmers_needed,
            JobType::Hunter => self.hunters_needed,
            JobType::Fisher => self.fishers_needed,
            JobType::Miner => self.miners_needed,
            JobType::Woodcutter => self.woodcutters_needed,
            JobType::Builder => self.builders_needed,
            JobType::Guard => self.guards_needed,
            JobType::Warrior => self.warriors_needed,
            JobType::Scholar => self.scholars_needed,
            JobType::Healer => self.healers_needed,
            JobType::Smith => self.smiths_needed,
            JobType::Craftsperson => self.craftspeople_needed,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_type_category() {
        assert_eq!(JobType::Farmer.category(), JobCategory::Essential);
        assert_eq!(JobType::Miner.category(), JobCategory::Production);
        assert_eq!(JobType::Guard.category(), JobCategory::Military);
    }

    #[test]
    fn test_job_demand_calculation() {
        let demand = JobDemand::calculate(
            100, // population
            0.5, // food satisfaction
            0.5, // security
            true, // has mines
            true, // has forests
            true, // has water
            false, // at war
            false, // needs buildings
        );

        assert!(demand.farmers_needed > 0);
        assert!(demand.total() > 0);
    }

    #[test]
    fn test_job_instance() {
        let job = Job::new(JobId(1), JobType::Farmer);
        assert_eq!(job.job_type, JobType::Farmer);
        assert!(job.is_active);
        assert!(!job.is_fully_staffed());
    }
}
