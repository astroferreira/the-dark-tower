//! Job definitions and metadata
//!
//! Detailed definitions for each job type including requirements and outputs.

use crate::simulation::jobs::types::{JobType, JobCategory};
use crate::simulation::colonists::skills::SkillType;
use crate::simulation::types::ResourceType;

/// Complete definition for a job type
#[derive(Clone, Debug)]
pub struct JobDefinition {
    pub job_type: JobType,
    pub name: &'static str,
    pub description: &'static str,
    pub category: JobCategory,
    pub primary_skill: Option<SkillType>,
    pub secondary_skill: Option<SkillType>,
    /// Base productivity (resources per worker per tick)
    pub base_productivity: f32,
    /// Resources produced
    pub produces: &'static [(ResourceType, f32)],
    /// Resources consumed
    pub consumes: &'static [(ResourceType, f32)],
    /// Required building (if any)
    pub required_building: Option<&'static str>,
    /// Minimum skill level to be effective
    pub min_skill_level: u8,
    /// Is this a dangerous job?
    pub is_dangerous: bool,
    /// Morale modifier for working this job
    pub morale_modifier: f32,
}

impl JobDefinition {
    /// Get the job definition for a job type
    pub fn for_job(job_type: JobType) -> Self {
        match job_type {
            JobType::Farmer => JobDefinition {
                job_type,
                name: "Farmer",
                description: "Tends crops and harvests food",
                category: JobCategory::Essential,
                primary_skill: Some(SkillType::Farming),
                secondary_skill: None,
                base_productivity: 2.0,
                produces: &[(ResourceType::Food, 2.0)],
                consumes: &[],
                required_building: None,
                min_skill_level: 0,
                is_dangerous: false,
                morale_modifier: 0.0,
            },
            JobType::Hunter => JobDefinition {
                job_type,
                name: "Hunter",
                description: "Hunts wild animals for food and leather",
                category: JobCategory::Essential,
                primary_skill: Some(SkillType::Hunting),
                secondary_skill: Some(SkillType::Combat),
                base_productivity: 1.5,
                produces: &[(ResourceType::Food, 1.5), (ResourceType::Leather, 0.5)],
                consumes: &[],
                required_building: None,
                min_skill_level: 2,
                is_dangerous: true,
                morale_modifier: 0.05,
            },
            JobType::Fisher => JobDefinition {
                job_type,
                name: "Fisher",
                description: "Catches fish from water sources",
                category: JobCategory::Essential,
                primary_skill: Some(SkillType::Fishing),
                secondary_skill: None,
                base_productivity: 1.5,
                produces: &[(ResourceType::Food, 1.5)],
                consumes: &[],
                required_building: None,
                min_skill_level: 1,
                is_dangerous: false,
                morale_modifier: 0.02,
            },
            JobType::Cook => JobDefinition {
                job_type,
                name: "Cook",
                description: "Prepares food for the tribe",
                category: JobCategory::Essential,
                primary_skill: Some(SkillType::Cooking),
                secondary_skill: None,
                base_productivity: 1.5,
                produces: &[(ResourceType::Food, 0.5)], // Bonus food from cooking
                consumes: &[],
                required_building: Some("Kitchen"),
                min_skill_level: 2,
                is_dangerous: false,
                morale_modifier: 0.05, // Good food = happy people
            },
            JobType::Miner => JobDefinition {
                job_type,
                name: "Miner",
                description: "Extracts stone and ore from the earth",
                category: JobCategory::Production,
                primary_skill: Some(SkillType::Mining),
                secondary_skill: None,
                base_productivity: 1.5,
                produces: &[(ResourceType::Stone, 1.5), (ResourceType::Iron, 0.3)],
                consumes: &[],
                required_building: None,
                min_skill_level: 2,
                is_dangerous: true,
                morale_modifier: -0.05, // Hard work
            },
            JobType::Woodcutter => JobDefinition {
                job_type,
                name: "Woodcutter",
                description: "Fells trees and gathers wood",
                category: JobCategory::Production,
                primary_skill: Some(SkillType::Woodcutting),
                secondary_skill: None,
                base_productivity: 2.0,
                produces: &[(ResourceType::Wood, 2.0)],
                consumes: &[],
                required_building: None,
                min_skill_level: 1,
                is_dangerous: false,
                morale_modifier: 0.0,
            },
            JobType::Smith => JobDefinition {
                job_type,
                name: "Smith",
                description: "Forges tools and weapons from metal",
                category: JobCategory::Production,
                primary_skill: Some(SkillType::Smithing),
                secondary_skill: Some(SkillType::Crafting),
                base_productivity: 0.5,
                produces: &[(ResourceType::Tools, 0.5), (ResourceType::Weapons, 0.3)],
                consumes: &[(ResourceType::Iron, 0.5), (ResourceType::Coal, 0.3)],
                required_building: Some("Smithy"),
                min_skill_level: 5,
                is_dangerous: false,
                morale_modifier: 0.1, // Respected work
            },
            JobType::Craftsperson => JobDefinition {
                job_type,
                name: "Craftsperson",
                description: "Creates goods from raw materials",
                category: JobCategory::Production,
                primary_skill: Some(SkillType::Crafting),
                secondary_skill: None,
                base_productivity: 0.6,
                produces: &[(ResourceType::Cloth, 0.5), (ResourceType::Tools, 0.3)],
                consumes: &[(ResourceType::Wood, 0.2), (ResourceType::Leather, 0.1)],
                required_building: Some("Workshop"),
                min_skill_level: 3,
                is_dangerous: false,
                morale_modifier: 0.05,
            },
            JobType::Builder => JobDefinition {
                job_type,
                name: "Builder",
                description: "Constructs buildings and structures",
                category: JobCategory::Construction,
                primary_skill: Some(SkillType::Building),
                secondary_skill: None,
                base_productivity: 0.5,
                produces: &[],
                consumes: &[(ResourceType::Wood, 0.3), (ResourceType::Stone, 0.3)],
                required_building: None,
                min_skill_level: 3,
                is_dangerous: true,
                morale_modifier: 0.05,
            },
            JobType::Guard => JobDefinition {
                job_type,
                name: "Guard",
                description: "Protects the settlement from threats",
                category: JobCategory::Military,
                primary_skill: Some(SkillType::Combat),
                secondary_skill: None,
                base_productivity: 0.0, // Guards don't produce
                produces: &[],
                consumes: &[],
                required_building: None,
                min_skill_level: 2,
                is_dangerous: true,
                morale_modifier: 0.0,
            },
            JobType::Warrior => JobDefinition {
                job_type,
                name: "Warrior",
                description: "Trained fighter for offensive operations",
                category: JobCategory::Military,
                primary_skill: Some(SkillType::Combat),
                secondary_skill: None,
                base_productivity: 0.0,
                produces: &[],
                consumes: &[],
                required_building: Some("Barracks"),
                min_skill_level: 4,
                is_dangerous: true,
                morale_modifier: 0.1, // Warriors are proud
            },
            JobType::Scout => JobDefinition {
                job_type,
                name: "Scout",
                description: "Explores and gathers intelligence",
                category: JobCategory::Military,
                primary_skill: Some(SkillType::Combat),
                secondary_skill: Some(SkillType::Hunting),
                base_productivity: 0.0,
                produces: &[],
                consumes: &[],
                required_building: None,
                min_skill_level: 3,
                is_dangerous: true,
                morale_modifier: 0.05,
            },
            JobType::Scholar => JobDefinition {
                job_type,
                name: "Scholar",
                description: "Studies and advances knowledge",
                category: JobCategory::Research,
                primary_skill: Some(SkillType::Research),
                secondary_skill: None,
                base_productivity: 1.0, // Research points
                produces: &[],
                consumes: &[],
                required_building: Some("Library"),
                min_skill_level: 5,
                is_dangerous: false,
                morale_modifier: 0.1,
            },
            JobType::Priest => JobDefinition {
                job_type,
                name: "Priest",
                description: "Tends to spiritual needs of the tribe",
                category: JobCategory::Research,
                primary_skill: Some(SkillType::Research),
                secondary_skill: Some(SkillType::Leadership),
                base_productivity: 0.5,
                produces: &[],
                consumes: &[],
                required_building: Some("Temple"),
                min_skill_level: 4,
                is_dangerous: false,
                morale_modifier: 0.1, // Spiritual fulfillment
            },
            JobType::Healer => JobDefinition {
                job_type,
                name: "Healer",
                description: "Treats the sick and wounded",
                category: JobCategory::Service,
                primary_skill: Some(SkillType::Medicine),
                secondary_skill: None,
                base_productivity: 0.5, // Healing capacity
                produces: &[],
                consumes: &[],
                required_building: Some("Hospital"),
                min_skill_level: 4,
                is_dangerous: false,
                morale_modifier: 0.05,
            },
            JobType::Hauler => JobDefinition {
                job_type,
                name: "Hauler",
                description: "Moves resources and goods",
                category: JobCategory::Service,
                primary_skill: None, // Unskilled
                secondary_skill: None,
                base_productivity: 0.5,
                produces: &[],
                consumes: &[],
                required_building: None,
                min_skill_level: 0,
                is_dangerous: false,
                morale_modifier: -0.05, // Menial work
            },
            JobType::Idle => JobDefinition {
                job_type,
                name: "Idle",
                description: "Not currently working",
                category: JobCategory::Service,
                primary_skill: None,
                secondary_skill: None,
                base_productivity: 0.0,
                produces: &[],
                consumes: &[],
                required_building: None,
                min_skill_level: 0,
                is_dangerous: false,
                morale_modifier: -0.1, // Idle hands...
            },
        }
    }

    /// Get experience gained per tick of work
    pub fn experience_per_tick(&self) -> u32 {
        // More skilled jobs = more XP
        match self.min_skill_level {
            0..=2 => 5,
            3..=4 => 10,
            5..=6 => 15,
            _ => 20,
        }
    }

    /// Check if colonist meets minimum requirements
    pub fn meets_requirements(&self, skill_level: u8) -> bool {
        skill_level >= self.min_skill_level
    }

    /// Calculate productivity for a given skill level
    pub fn productivity_for_skill(&self, skill_level: u8) -> f32 {
        let skill_mult = crate::simulation::colonists::skills::skill_productivity(skill_level);
        self.base_productivity * skill_mult
    }
}

/// Get all job definitions
pub fn all_job_definitions() -> Vec<JobDefinition> {
    JobType::all().iter()
        .map(|&jt| JobDefinition::for_job(jt))
        .collect()
}

/// Get jobs that produce a specific resource
pub fn jobs_producing(resource: ResourceType) -> Vec<JobType> {
    all_job_definitions()
        .into_iter()
        .filter(|def| def.produces.iter().any(|(r, _)| *r == resource))
        .map(|def| def.job_type)
        .collect()
}

/// Get jobs by category
pub fn jobs_in_category(category: JobCategory) -> Vec<JobType> {
    all_job_definitions()
        .into_iter()
        .filter(|def| def.category == category)
        .map(|def| def.job_type)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_definitions() {
        for job_type in JobType::all() {
            let def = JobDefinition::for_job(*job_type);
            assert_eq!(def.job_type, *job_type);
            assert!(!def.name.is_empty());
        }
    }

    #[test]
    fn test_jobs_producing() {
        let food_jobs = jobs_producing(ResourceType::Food);
        assert!(food_jobs.contains(&JobType::Farmer));
        assert!(food_jobs.contains(&JobType::Hunter));
    }

    #[test]
    fn test_productivity_scaling() {
        let def = JobDefinition::for_job(JobType::Farmer);
        let low_skill = def.productivity_for_skill(0);
        let high_skill = def.productivity_for_skill(15);
        assert!(high_skill > low_skill);
    }
}
