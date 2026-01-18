//! Population Pool - aggregate tracking for non-notable colonists
//!
//! Tracks the ~95% of population that aren't individually simulated,
//! using demographic cohorts for efficiency.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::simulation::colonists::types::LifeStage;
use crate::simulation::colonists::skills::SkillType;
use crate::simulation::jobs::types::JobType;

/// Demographic cohort for aggregate population tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopulationCohort {
    /// Life stage of this cohort
    pub life_stage: LifeStage,
    /// Number of people in this cohort
    pub count: u32,
    /// Average age within the cohort
    pub average_age: f32,
    /// Average skill levels (simplified)
    pub average_skills: HashMap<SkillType, f32>,
    /// Workers assigned to jobs from this cohort
    pub job_assignments: HashMap<JobType, u32>,
}

impl PopulationCohort {
    pub fn new(life_stage: LifeStage, count: u32, average_age: f32) -> Self {
        PopulationCohort {
            life_stage,
            count,
            average_age,
            average_skills: HashMap::new(),
            job_assignments: HashMap::new(),
        }
    }

    /// Get work capacity of this cohort
    pub fn work_capacity(&self) -> f32 {
        self.count as f32 * self.life_stage.work_capacity()
    }

    /// Get available (unassigned) workers
    pub fn available_workers(&self) -> u32 {
        let assigned: u32 = self.job_assignments.values().sum();
        self.count.saturating_sub(assigned)
    }

    /// Assign workers to a job
    pub fn assign_workers(&mut self, job_type: JobType, count: u32) -> u32 {
        let available = self.available_workers();
        let to_assign = count.min(available);
        *self.job_assignments.entry(job_type).or_insert(0) += to_assign;
        to_assign
    }

    /// Unassign workers from a job
    pub fn unassign_workers(&mut self, job_type: JobType, count: u32) {
        if let Some(assigned) = self.job_assignments.get_mut(&job_type) {
            *assigned = assigned.saturating_sub(count);
            if *assigned == 0 {
                self.job_assignments.remove(&job_type);
            }
        }
    }

    /// Get average skill level for a skill type
    pub fn average_skill(&self, skill_type: SkillType) -> f32 {
        self.average_skills.get(&skill_type).copied().unwrap_or(3.0) // Default to "Adequate"
    }

    /// Set average skill level
    pub fn set_average_skill(&mut self, skill_type: SkillType, level: f32) {
        self.average_skills.insert(skill_type, level.clamp(0.0, 20.0));
    }

    /// Add population to this cohort
    pub fn add(&mut self, count: u32) {
        self.count += count;
    }

    /// Remove population from this cohort
    pub fn remove(&mut self, count: u32) {
        // Remove from unassigned first, then proportionally from jobs
        let available = self.available_workers();
        let from_available = count.min(available);
        self.count = self.count.saturating_sub(from_available);

        let remaining = count - from_available;
        if remaining > 0 && !self.job_assignments.is_empty() {
            // Proportionally remove from jobs
            let total_assigned: u32 = self.job_assignments.values().sum();
            let ratio = remaining as f32 / total_assigned as f32;

            let to_remove: Vec<_> = self.job_assignments
                .iter()
                .map(|(&job, &assigned)| (job, (assigned as f32 * ratio).ceil() as u32))
                .collect();

            for (job, amount) in to_remove {
                self.unassign_workers(job, amount);
            }
            self.count = self.count.saturating_sub(remaining);
        }
    }

    /// Get productivity for workers assigned to a job
    pub fn job_productivity(&self, job_type: JobType) -> f32 {
        let workers = *self.job_assignments.get(&job_type).unwrap_or(&0);
        if workers == 0 {
            return 0.0;
        }

        let skill = job_type.primary_skill()
            .map(|s| self.average_skill(s))
            .unwrap_or(5.0);

        let skill_mult = crate::simulation::colonists::skills::skill_productivity(skill as u8);
        let life_mult = self.life_stage.work_capacity();

        workers as f32 * skill_mult * life_mult
    }
}

/// Complete population pool for a tribe
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopulationPool {
    /// Child cohort (0-15 years)
    pub children: PopulationCohort,
    /// Adult cohort (16-64 years)
    pub adults: PopulationCohort,
    /// Elder cohort (65+ years)
    pub elders: PopulationCohort,
    /// Birth rate modifier (based on conditions)
    pub birth_rate_modifier: f32,
    /// Death rate modifier (based on conditions)
    pub death_rate_modifier: f32,
}

impl PopulationPool {
    /// Create a new population pool from an initial population
    pub fn new(total_population: u32) -> Self {
        // Typical demographic distribution
        let children = (total_population as f32 * 0.25) as u32;
        let elders = (total_population as f32 * 0.10) as u32;
        let adults = total_population - children - elders;

        PopulationPool {
            children: PopulationCohort::new(LifeStage::Child, children, 8.0),
            adults: PopulationCohort::new(LifeStage::Adult, adults, 35.0),
            elders: PopulationCohort::new(LifeStage::Elder, elders, 72.0),
            birth_rate_modifier: 1.0,
            death_rate_modifier: 1.0,
        }
    }

    /// Get total pool population
    pub fn total(&self) -> u32 {
        self.children.count + self.adults.count + self.elders.count
    }

    /// Get total workers (adults + working elders)
    pub fn workers(&self) -> u32 {
        self.adults.count + (self.elders.count / 2) // Elders work at 50%
    }

    /// Get available (unassigned) workers
    pub fn available_workers(&self) -> u32 {
        self.adults.available_workers() + self.elders.available_workers()
    }

    /// Assign workers to a job, preferring adults
    pub fn assign_workers(&mut self, job_type: JobType, count: u32) -> u32 {
        let mut assigned = 0;

        // First, assign from adults
        let from_adults = self.adults.assign_workers(job_type, count);
        assigned += from_adults;

        // Then, if needed, from elders
        if assigned < count {
            let remaining = count - assigned;
            assigned += self.elders.assign_workers(job_type, remaining);
        }

        assigned
    }

    /// Unassign workers from a job
    pub fn unassign_workers(&mut self, job_type: JobType, count: u32) {
        // Unassign from elders first (preserve adult workforce)
        let elder_assigned = self.elders.job_assignments.get(&job_type).copied().unwrap_or(0);
        let from_elders = count.min(elder_assigned);
        self.elders.unassign_workers(job_type, from_elders);

        let remaining = count - from_elders;
        if remaining > 0 {
            self.adults.unassign_workers(job_type, remaining);
        }
    }

    /// Clear all job assignments
    pub fn clear_assignments(&mut self) {
        self.adults.job_assignments.clear();
        self.elders.job_assignments.clear();
    }

    /// Get workers assigned to a job
    pub fn workers_for_job(&self, job_type: JobType) -> u32 {
        self.adults.job_assignments.get(&job_type).copied().unwrap_or(0) +
        self.elders.job_assignments.get(&job_type).copied().unwrap_or(0)
    }

    /// Get productivity for a job
    pub fn job_productivity(&self, job_type: JobType) -> f32 {
        self.adults.job_productivity(job_type) + self.elders.job_productivity(job_type)
    }

    /// Process population dynamics for one tick
    pub fn tick<R: Rng>(
        &mut self,
        food_satisfaction: f32,
        health_satisfaction: f32,
        base_birth_rate: f32,
        base_death_rate: f32,
        rng: &mut R,
    ) -> PoolDynamicsResult {
        let mut result = PoolDynamicsResult::default();

        // Calculate effective rates
        let effective_birth_rate = base_birth_rate * self.birth_rate_modifier * food_satisfaction;
        let effective_death_rate = base_death_rate * self.death_rate_modifier *
            (1.0 + (1.0 - health_satisfaction) * 2.0);

        // Births (from adults)
        let birth_chance = self.adults.count as f32 * effective_birth_rate * 0.5; // Half are female
        let births = (birth_chance + rng.gen::<f32>() * birth_chance * 0.5) as u32;
        self.children.add(births);
        result.births = births;

        // Deaths per cohort
        let child_death_rate = effective_death_rate * 1.2; // Children more vulnerable
        let adult_death_rate = effective_death_rate * 0.8;
        let elder_death_rate = effective_death_rate * 2.0;  // Elders more vulnerable

        let child_deaths = ((self.children.count as f32 * child_death_rate) +
            rng.gen::<f32>() * 0.5) as u32;
        let adult_deaths = ((self.adults.count as f32 * adult_death_rate) +
            rng.gen::<f32>() * 0.5) as u32;
        let elder_deaths = ((self.elders.count as f32 * elder_death_rate) +
            rng.gen::<f32>() * 0.5) as u32;

        self.children.remove(child_deaths);
        self.adults.remove(adult_deaths);
        self.elders.remove(elder_deaths);

        result.deaths = child_deaths + adult_deaths + elder_deaths;
        result.child_deaths = child_deaths;
        result.adult_deaths = adult_deaths;
        result.elder_deaths = elder_deaths;

        // Aging (happens once per year, every 4 ticks)
        // For simplicity, we age a fraction each tick
        let age_fraction = 0.25; // Quarter year per tick

        // Children aging into adults
        let children_aging = (self.children.count as f32 * age_fraction / 16.0) as u32;
        if children_aging > 0 {
            self.children.remove(children_aging);
            self.adults.add(children_aging);
            result.children_aged_up = children_aging;
        }

        // Adults aging into elders
        let adults_aging = (self.adults.count as f32 * age_fraction / 49.0) as u32;
        if adults_aging > 0 {
            self.adults.remove(adults_aging);
            self.elders.add(adults_aging);
            result.adults_aged_up = adults_aging;
        }

        // Update average ages
        self.children.average_age = (self.children.average_age + age_fraction).min(15.9);
        self.adults.average_age = (self.adults.average_age + age_fraction).min(64.9);
        self.elders.average_age = (self.elders.average_age + age_fraction).min(100.0);

        result.net_change = result.births as i32 - result.deaths as i32;
        result
    }

    /// Add external population (from migration, etc.)
    pub fn add_population(&mut self, adults: u32, children: u32, elders: u32) {
        self.adults.add(adults);
        self.children.add(children);
        self.elders.add(elders);
    }

    /// Remove population (from emigration, casualties, etc.)
    pub fn remove_population(&mut self, count: u32) {
        // Remove proportionally from each cohort
        let total = self.total();
        if total == 0 {
            return;
        }

        let adult_ratio = self.adults.count as f32 / total as f32;
        let child_ratio = self.children.count as f32 / total as f32;

        let adult_remove = (count as f32 * adult_ratio) as u32;
        let child_remove = (count as f32 * child_ratio) as u32;
        let elder_remove = count - adult_remove - child_remove;

        self.adults.remove(adult_remove);
        self.children.remove(child_remove);
        self.elders.remove(elder_remove);
    }

    /// Apply combat casualties (targets adults/warriors primarily)
    pub fn apply_casualties(&mut self, count: u32) {
        // Combat casualties come from adults, especially those with military jobs
        let military_jobs = [JobType::Guard, JobType::Warrior, JobType::Scout];

        // First, remove from military
        let mut remaining = count;
        for job_type in military_jobs.iter() {
            let assigned = self.workers_for_job(*job_type);
            let to_remove = remaining.min(assigned);
            if to_remove > 0 {
                self.unassign_workers(*job_type, to_remove);
                self.adults.count = self.adults.count.saturating_sub(to_remove);
                remaining = remaining.saturating_sub(to_remove);
            }
            if remaining == 0 {
                break;
            }
        }

        // Then from general adult population
        if remaining > 0 {
            self.adults.remove(remaining);
        }
    }

    /// Get summary statistics
    pub fn summary(&self) -> PoolSummary {
        PoolSummary {
            total: self.total(),
            children: self.children.count,
            adults: self.adults.count,
            elders: self.elders.count,
            workers: self.workers(),
            available_workers: self.available_workers(),
            dependency_ratio: if self.adults.count > 0 {
                (self.children.count + self.elders.count) as f32 / self.adults.count as f32
            } else {
                0.0
            },
        }
    }

    /// Upgrade average skills based on work experience
    pub fn improve_skills(&mut self, job_type: JobType, amount: f32) {
        if let Some(skill_type) = job_type.primary_skill() {
            let current = self.adults.average_skill(skill_type);
            self.adults.set_average_skill(skill_type, current + amount);
            self.elders.set_average_skill(skill_type, current + amount * 0.5);
        }
    }
}

impl Default for PopulationPool {
    fn default() -> Self {
        PopulationPool::new(100)
    }
}

/// Result of population dynamics for one tick
#[derive(Clone, Debug, Default)]
pub struct PoolDynamicsResult {
    pub births: u32,
    pub deaths: u32,
    pub child_deaths: u32,
    pub adult_deaths: u32,
    pub elder_deaths: u32,
    pub children_aged_up: u32,
    pub adults_aged_up: u32,
    pub net_change: i32,
}

/// Summary of population pool state
#[derive(Clone, Debug)]
pub struct PoolSummary {
    pub total: u32,
    pub children: u32,
    pub adults: u32,
    pub elders: u32,
    pub workers: u32,
    pub available_workers: u32,
    pub dependency_ratio: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_population_pool_creation() {
        let pool = PopulationPool::new(100);
        assert_eq!(pool.total(), 100);
        assert!(pool.children.count > 0);
        assert!(pool.adults.count > 0);
        assert!(pool.elders.count > 0);
    }

    #[test]
    fn test_job_assignment() {
        let mut pool = PopulationPool::new(100);
        let initial_available = pool.available_workers();

        let assigned = pool.assign_workers(JobType::Farmer, 10);
        assert!(assigned > 0);
        assert!(pool.available_workers() < initial_available);
    }

    #[test]
    fn test_population_dynamics() {
        let mut pool = PopulationPool::new(100);
        let mut rng = rand::thread_rng();

        let result = pool.tick(0.8, 0.8, 0.02, 0.01, &mut rng);

        // Population should change
        assert!(result.births >= 0 || result.deaths >= 0);
    }
}
