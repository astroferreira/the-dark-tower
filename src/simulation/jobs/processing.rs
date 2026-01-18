//! Job processing - execute work and produce resources
//!
//! Processes all assigned jobs each tick, producing resources and gaining experience.

use rand::Rng;
use std::collections::HashMap;

use crate::simulation::types::ResourceType;
use crate::simulation::resources::Stockpile;
use crate::simulation::colonists::lifecycle::NotableColonists;
use crate::simulation::colonists::pool::PopulationPool;
use crate::simulation::colonists::skills::skill_productivity;
use crate::simulation::jobs::types::JobType;
use crate::simulation::jobs::definitions::JobDefinition;
use crate::simulation::jobs::assignment::JobManager;
use crate::simulation::society::SocietyState;

/// Result of processing all jobs for one tick
#[derive(Clone, Debug, Default)]
pub struct JobProcessingResult {
    /// Resources produced by job type
    pub production: HashMap<ResourceType, f32>,
    /// Resources consumed by job type
    pub consumption: HashMap<ResourceType, f32>,
    /// Research points generated
    pub research_points: f32,
    /// Military strength generated
    pub military_strength: f32,
    /// Morale bonus from jobs
    pub morale_modifier: f32,
    /// Number of workers that leveled up skills
    pub skill_ups: u32,
}

impl JobProcessingResult {
    /// Net production of a resource (produced - consumed)
    pub fn net(&self, resource: ResourceType) -> f32 {
        let produced = self.production.get(&resource).copied().unwrap_or(0.0);
        let consumed = self.consumption.get(&resource).copied().unwrap_or(0.0);
        produced - consumed
    }

    /// Apply results to stockpile
    pub fn apply_to_stockpile(&self, stockpile: &mut Stockpile) {
        for (resource, amount) in &self.production {
            stockpile.add(*resource, *amount);
        }
        for (resource, amount) in &self.consumption {
            stockpile.remove(*resource, *amount);
        }
    }
}

/// Process all jobs for one tick
pub fn process_jobs<R: Rng>(
    job_manager: &JobManager,
    notables: &mut NotableColonists,
    pool: &mut PopulationPool,
    stockpile: &Stockpile,
    society_state: &SocietyState,
    season_modifier: f32,
    rng: &mut R,
) -> JobProcessingResult {
    let mut result = JobProcessingResult::default();

    // Process each job type
    for job_type in JobType::all() {
        if *job_type == JobType::Idle {
            continue;
        }

        let job_result = process_single_job_type(
            *job_type,
            job_manager,
            notables,
            pool,
            stockpile,
            society_state,
            season_modifier,
            rng,
        );

        // Accumulate results
        for (resource, amount) in job_result.production {
            *result.production.entry(resource).or_insert(0.0) += amount;
        }
        for (resource, amount) in job_result.consumption {
            *result.consumption.entry(resource).or_insert(0.0) += amount;
        }
        result.research_points += job_result.research_points;
        result.military_strength += job_result.military_strength;
        result.morale_modifier += job_result.morale_modifier;
        result.skill_ups += job_result.skill_ups;
    }

    result
}

/// Process a single job type
fn process_single_job_type<R: Rng>(
    job_type: JobType,
    job_manager: &JobManager,
    notables: &mut NotableColonists,
    pool: &mut PopulationPool,
    stockpile: &Stockpile,
    society_state: &SocietyState,
    season_modifier: f32,
    rng: &mut R,
) -> JobProcessingResult {
    let mut result = JobProcessingResult::default();
    let def = JobDefinition::for_job(job_type);

    // Skip if no workers
    let notable_count = job_manager.jobs.values()
        .filter(|j| j.job_type == job_type)
        .flat_map(|j| &j.assigned_notables)
        .count() as u32;
    let pool_count = pool.workers_for_job(job_type);

    if notable_count == 0 && pool_count == 0 {
        return result;
    }

    // Check if resources are available for consumption
    let can_work = def.consumes.iter()
        .all(|(resource, amount)| stockpile.has(*resource, *amount));

    if !can_work && !def.consumes.is_empty() {
        return result; // Can't work without resources
    }

    // Calculate base efficiency
    let society_prod_mod = society_state.production_modifier();
    let base_efficiency = season_modifier * society_prod_mod;

    // Process notable colonist production
    for job in job_manager.jobs.values() {
        if job.job_type != job_type {
            continue;
        }

        for &colonist_id in &job.assigned_notables {
            if let Some(colonist) = notables.colonists.get_mut(&colonist_id) {
                if !colonist.is_alive || !colonist.can_work() {
                    continue;
                }

                // Get skill level
                let skill_level = if let Some(skill) = def.primary_skill {
                    colonist.skills.get_level(skill)
                } else {
                    5
                };

                let skill_mult = skill_productivity(skill_level);
                let mood_mult = colonist.mood.work_modifier();
                let total_mult = base_efficiency * skill_mult * mood_mult;

                // Produce resources
                for (resource, base_amount) in def.produces {
                    let amount = base_amount * total_mult;
                    *result.production.entry(*resource).or_insert(0.0) += amount;
                }

                // Consume resources
                for (resource, base_amount) in def.consumes {
                    let amount = base_amount * total_mult;
                    *result.consumption.entry(*resource).or_insert(0.0) += amount;
                }

                // Gain experience
                if let Some(skill) = def.primary_skill {
                    let exp_gain = def.experience_per_tick();
                    if colonist.skills.add_experience(skill, exp_gain) {
                        result.skill_ups += 1;
                        colonist.add_event(format!(
                            "Improved {} to level {}",
                            skill.name(),
                            colonist.skills.get_level(skill)
                        ));
                    }
                }

                // Apply morale modifier
                result.morale_modifier += def.morale_modifier / 10.0; // Scaled down for individuals
            }
        }
    }

    // Process pool worker production
    if pool_count > 0 {
        let pool_productivity = pool.job_productivity(job_type);
        let total_mult = base_efficiency * pool_productivity / pool_count as f32;

        for (resource, base_amount) in def.produces {
            let amount = base_amount * total_mult * pool_count as f32;
            *result.production.entry(*resource).or_insert(0.0) += amount;
        }

        for (resource, base_amount) in def.consumes {
            let amount = base_amount * total_mult * pool_count as f32;
            *result.consumption.entry(*resource).or_insert(0.0) += amount;
        }

        // Pool skill improvement (slower than notables)
        if rng.gen::<f32>() < 0.1 {
            pool.improve_skills(job_type, 0.01);
        }

        result.morale_modifier += def.morale_modifier * pool_count as f32 / 100.0;
    }

    // Special job effects
    match job_type {
        JobType::Scholar => {
            let total_workers = notable_count + pool_count;
            let research_mult = society_state.research_modifier();
            result.research_points = def.base_productivity * total_workers as f32 * base_efficiency * research_mult;
        }
        JobType::Priest => {
            let total_workers = notable_count + pool_count;
            result.research_points += def.base_productivity * total_workers as f32 * 0.3 * base_efficiency;
            result.morale_modifier += total_workers as f32 * 0.02; // Spiritual bonus
        }
        JobType::Guard | JobType::Warrior | JobType::Scout => {
            let military_mult = society_state.military_modifier();
            result.military_strength = (notable_count + pool_count) as f32 * military_mult;
        }
        JobType::Healer => {
            // Healers provide health bonus tracked elsewhere
        }
        _ => {}
    }

    result
}

/// Get production estimate for a job type (for planning)
pub fn estimate_production(
    job_type: JobType,
    worker_count: u32,
    average_skill: u8,
    efficiency_modifier: f32,
) -> HashMap<ResourceType, f32> {
    let def = JobDefinition::for_job(job_type);
    let skill_mult = skill_productivity(average_skill);
    let total_mult = efficiency_modifier * skill_mult;

    let mut production = HashMap::new();
    for (resource, base_amount) in def.produces {
        let amount = base_amount * total_mult * worker_count as f32;
        production.insert(*resource, amount);
    }
    production
}

/// Get required workers to meet a production target
pub fn workers_needed_for_production(
    job_type: JobType,
    resource: ResourceType,
    target_amount: f32,
    average_skill: u8,
    efficiency_modifier: f32,
) -> u32 {
    let def = JobDefinition::for_job(job_type);
    let skill_mult = skill_productivity(average_skill);
    let total_mult = efficiency_modifier * skill_mult;

    for (res, base_amount) in def.produces {
        if *res == resource {
            let per_worker = base_amount * total_mult;
            if per_worker > 0.0 {
                return (target_amount / per_worker).ceil() as u32;
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_production() {
        let production = estimate_production(JobType::Farmer, 5, 10, 1.0);
        assert!(production.get(&ResourceType::Food).unwrap_or(&0.0) > &0.0);
    }

    #[test]
    fn test_workers_needed() {
        let workers = workers_needed_for_production(
            JobType::Farmer,
            ResourceType::Food,
            10.0,
            10,
            1.0,
        );
        assert!(workers > 0);
    }

    #[test]
    fn test_job_processing_result() {
        let mut result = JobProcessingResult::default();
        result.production.insert(ResourceType::Food, 10.0);
        result.consumption.insert(ResourceType::Food, 2.0);

        assert_eq!(result.net(ResourceType::Food), 8.0);
    }
}
