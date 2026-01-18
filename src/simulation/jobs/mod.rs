//! Job system - work assignments and resource production
//!
//! This module handles job types, assignments, and work processing.
//! Jobs are how colonists produce resources and contribute to the tribe.

pub mod types;
pub mod definitions;
pub mod assignment;
pub mod processing;

pub use types::{JobId, JobType, JobCategory, Job, JobDemand};
pub use definitions::{JobDefinition, all_job_definitions, jobs_producing, jobs_in_category};
pub use assignment::{JobManager, assign_all_jobs, AssignmentResult, job_suitability};
pub use processing::{
    process_jobs, JobProcessingResult, estimate_production, workers_needed_for_production,
};
