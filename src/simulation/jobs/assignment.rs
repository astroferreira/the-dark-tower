//! Job assignment algorithm
//!
//! Assigns colonists and pool workers to jobs based on skills and priorities.

use std::collections::HashMap;

use crate::simulation::colonists::types::{Colonist, ColonistId};
use crate::simulation::colonists::pool::PopulationPool;
use crate::simulation::colonists::lifecycle::NotableColonists;
use crate::simulation::jobs::types::{Job, JobId, JobType, JobDemand};
use crate::simulation::jobs::definitions::JobDefinition;
use crate::simulation::society::SocietyType;

/// Job assignment manager
#[derive(Clone, Debug, Default)]
pub struct JobManager {
    /// Active jobs
    pub jobs: HashMap<JobId, Job>,
    /// Next job ID
    pub next_job_id: u64,
    /// Demand for each job type
    pub demand: JobDemand,
}

impl JobManager {
    pub fn new() -> Self {
        JobManager {
            jobs: HashMap::new(),
            next_job_id: 0,
            demand: JobDemand::default(),
        }
    }

    /// Create a new job
    pub fn create_job(&mut self, job_type: JobType) -> JobId {
        let id = JobId(self.next_job_id);
        self.next_job_id += 1;
        self.jobs.insert(id, Job::new(id, job_type));
        id
    }

    /// Get or create a job of a type
    pub fn get_or_create_job(&mut self, job_type: JobType) -> JobId {
        // Find existing job of this type
        if let Some((&id, _)) = self.jobs.iter().find(|(_, j)| j.job_type == job_type) {
            return id;
        }
        self.create_job(job_type)
    }

    /// Update job demand based on tribe state
    pub fn update_demand(
        &mut self,
        population: u32,
        food_satisfaction: f32,
        security_satisfaction: f32,
        has_mines: bool,
        has_forests: bool,
        has_water: bool,
        at_war: bool,
        needs_buildings: bool,
        society_type: SocietyType,
    ) {
        // Calculate base demand
        self.demand = JobDemand::calculate(
            population,
            food_satisfaction,
            security_satisfaction,
            has_mines,
            has_forests,
            has_water,
            at_war,
            needs_buildings,
        );

        // Apply society modifiers
        let config = society_type.config();

        // Military society wants more warriors
        if config.military_mult > 1.2 {
            self.demand.guards_needed = (self.demand.guards_needed as f32 * 1.5) as u32;
            self.demand.warriors_needed = (self.demand.warriors_needed as f32 * 1.5) as u32;
        }

        // Research-focused society wants more scholars
        if config.research_mult > 1.2 {
            self.demand.scholars_needed = (self.demand.scholars_needed as f32 * 1.5) as u32;
        }

        // Trade society wants more craftspeople
        if config.trade_mult > 1.2 {
            self.demand.craftspeople_needed = (self.demand.craftspeople_needed as f32 * 1.3) as u32;
            self.demand.smiths_needed = (self.demand.smiths_needed as f32 * 1.2) as u32;
        }
    }

    /// Get total workers assigned to a job type
    pub fn workers_for_job(&self, job_type: JobType) -> u32 {
        self.jobs.values()
            .filter(|j| j.job_type == job_type)
            .map(|j| j.total_workers())
            .sum()
    }

    /// Get total workers demand
    pub fn total_demand(&self) -> u32 {
        self.demand.total()
    }

    /// Get unfilled job positions
    pub fn unfilled_positions(&self) -> Vec<(JobType, u32)> {
        let job_types = [
            JobType::Farmer, JobType::Hunter, JobType::Fisher,
            JobType::Miner, JobType::Woodcutter, JobType::Builder,
            JobType::Guard, JobType::Warrior, JobType::Scholar,
            JobType::Healer, JobType::Smith, JobType::Craftsperson,
        ];

        job_types.iter()
            .filter_map(|&jt| {
                let needed = self.demand.get(jt);
                let assigned = self.workers_for_job(jt);
                if needed > assigned {
                    Some((jt, needed - assigned))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Assign notable colonists and pool workers to jobs
pub fn assign_all_jobs(
    job_manager: &mut JobManager,
    notables: &mut NotableColonists,
    pool: &mut PopulationPool,
    society_type: SocietyType,
) -> AssignmentResult {
    let mut result = AssignmentResult::default();

    // Clear previous assignments
    pool.clear_assignments();
    for colonist in notables.colonists.values_mut() {
        colonist.current_job = None;
    }

    // Get job priorities based on demand
    let priorities = calculate_priorities(&job_manager.demand, society_type);

    // Assign notable colonists first (they're specialists)
    for (job_type, _) in &priorities {
        assign_notables_to_job(
            *job_type,
            job_manager,
            notables,
            &mut result,
        );
    }

    // Then assign pool workers to fill remaining demand
    for (job_type, needed) in &priorities {
        let already_assigned = job_manager.workers_for_job(*job_type);
        let remaining = needed.saturating_sub(already_assigned);

        if remaining > 0 {
            let assigned = pool.assign_workers(*job_type, remaining);
            result.pool_assigned += assigned;

            // Update job record
            if let Some(job) = job_manager.jobs.values_mut()
                .find(|j| j.job_type == *job_type)
            {
                job.pool_workers += assigned;
            } else {
                let id = job_manager.create_job(*job_type);
                if let Some(job) = job_manager.jobs.get_mut(&id) {
                    job.pool_workers = assigned;
                }
            }
        }
    }

    // Set remaining workers as idle
    let idle_workers = pool.available_workers();
    if idle_workers > 0 {
        pool.assign_workers(JobType::Idle, idle_workers);
        result.idle_workers = idle_workers;
    }

    result
}

/// Calculate job priorities based on demand and society type
fn calculate_priorities(demand: &JobDemand, society_type: SocietyType) -> Vec<(JobType, u32)> {
    let config = society_type.config();

    // Build priority list
    let mut priorities: Vec<(JobType, u32, u32)> = vec![
        // (job_type, demand, priority_boost)
        (JobType::Farmer, demand.farmers_needed, 100),
        (JobType::Hunter, demand.hunters_needed, 80),
        (JobType::Fisher, demand.fishers_needed, 70),
        (JobType::Guard, demand.guards_needed,
            if config.military_mult > 1.2 { 95 } else { 60 }),
        (JobType::Warrior, demand.warriors_needed,
            if config.military_mult > 1.2 { 90 } else { 55 }),
        (JobType::Miner, demand.miners_needed, 50),
        (JobType::Woodcutter, demand.woodcutters_needed, 50),
        (JobType::Builder, demand.builders_needed, 45),
        (JobType::Scholar, demand.scholars_needed,
            if config.research_mult > 1.2 { 70 } else { 40 }),
        (JobType::Healer, demand.healers_needed, 60),
        (JobType::Smith, demand.smiths_needed, 35),
        (JobType::Craftsperson, demand.craftspeople_needed, 30),
    ];

    // Sort by priority
    priorities.sort_by(|a, b| b.2.cmp(&a.2));

    // Return (job_type, demand) pairs
    priorities.into_iter()
        .filter(|(_, demand, _)| *demand > 0)
        .map(|(jt, d, _)| (jt, d))
        .collect()
}

/// Assign notable colonists to a job based on their skills
fn assign_notables_to_job(
    job_type: JobType,
    job_manager: &mut JobManager,
    notables: &mut NotableColonists,
    result: &mut AssignmentResult,
) {
    let def = JobDefinition::for_job(job_type);
    let needed = job_manager.demand.get(job_type);
    let current = job_manager.workers_for_job(job_type);

    if current >= needed {
        return;
    }

    // Find best colonists for this job
    let mut candidates: Vec<_> = notables.colonists.values()
        .filter(|c| c.is_alive && c.can_work() && c.current_job.is_none())
        .map(|c| {
            let skill_level = if let Some(skill) = def.primary_skill {
                c.skills.get_level(skill)
            } else {
                5 // Default for unskilled jobs
            };
            (c.id, skill_level)
        })
        .collect();

    // Sort by skill level (best first)
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    // Assign up to needed workers
    let to_assign = (needed - current) as usize;
    for (colonist_id, _skill) in candidates.into_iter().take(to_assign) {
        // Assign colonist
        if let Some(colonist) = notables.colonists.get_mut(&colonist_id) {
            colonist.current_job = Some(job_type);
            result.notables_assigned += 1;
        }

        // Update job
        let job_id = job_manager.get_or_create_job(job_type);
        if let Some(job) = job_manager.jobs.get_mut(&job_id) {
            job.assigned_notables.push(colonist_id);
        }
    }
}

/// Result of job assignment
#[derive(Clone, Debug, Default)]
pub struct AssignmentResult {
    pub notables_assigned: u32,
    pub pool_assigned: u32,
    pub idle_workers: u32,
}

impl AssignmentResult {
    pub fn total_assigned(&self) -> u32 {
        self.notables_assigned + self.pool_assigned
    }
}

/// Get job suitability score for a colonist
pub fn job_suitability(colonist: &Colonist, job_type: JobType) -> f32 {
    let def = JobDefinition::for_job(job_type);

    let skill_score = if let Some(skill) = def.primary_skill {
        colonist.skills.get_level(skill) as f32 / 20.0
    } else {
        0.5
    };

    let secondary_score = if let Some(skill) = def.secondary_skill {
        colonist.skills.get_level(skill) as f32 / 40.0
    } else {
        0.0
    };

    // Attribute bonuses
    let attr_bonus = match def.category {
        crate::simulation::jobs::types::JobCategory::Production =>
            colonist.attributes.strength as f32 / 40.0,
        crate::simulation::jobs::types::JobCategory::Research =>
            colonist.attributes.intelligence as f32 / 40.0,
        crate::simulation::jobs::types::JobCategory::Military =>
            (colonist.attributes.strength + colonist.attributes.agility) as f32 / 80.0,
        _ => 0.0,
    };

    skill_score + secondary_score + attr_bonus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_manager() {
        let mut manager = JobManager::new();
        let id = manager.create_job(JobType::Farmer);
        assert!(manager.jobs.contains_key(&id));
    }

    #[test]
    fn test_calculate_priorities() {
        let demand = JobDemand::calculate(100, 0.5, 0.5, true, true, true, false, false);
        let priorities = calculate_priorities(&demand, SocietyType::TribalCouncil);

        // Farmers should be high priority
        assert!(priorities.iter().any(|(jt, _)| *jt == JobType::Farmer));
    }

    #[test]
    fn test_demand_calculation() {
        let mut manager = JobManager::new();
        manager.update_demand(
            100, 0.5, 0.5,
            true, true, true, false, false,
            SocietyType::TribalCouncil,
        );

        assert!(manager.demand.farmers_needed > 0);
    }
}
