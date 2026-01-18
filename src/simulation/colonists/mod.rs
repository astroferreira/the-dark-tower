//! Colonist system - individual and aggregate population tracking
//!
//! This module implements a hybrid population tracking system:
//! - Notable colonists (~5%) are tracked individually with full attributes
//! - Population pool (~95%) is tracked as demographic cohorts
//!
//! This balances simulation depth with performance.

pub mod types;
pub mod skills;
pub mod mood;
pub mod pool;
pub mod lifecycle;
pub mod movement;
pub mod routines;
pub mod relationships;

pub use types::{
    Colonist, ColonistId, ColonistRole, ColonistActivityState, LifeStage, Gender,
    Attributes, NameGenerator,
};
pub use skills::{SkillType, SkillCategory, SkillSet, Skill, skill_productivity, skill_level_name};
pub use mood::{MoodState, MoodModifier, MoodModifierType, apply_needs_modifiers};
pub use pool::{PopulationPool, PopulationCohort, PoolDynamicsResult, PoolSummary};
pub use lifecycle::{
    NotableColonists, process_notable_lifecycle, process_notable_births,
    promote_to_notable, target_notable_count, LifecycleResult,
};
pub use movement::{
    process_colonist_movement, trigger_flee, wander_locally, process_fast_local_movement,
    find_work_location, find_patrol_location, find_scout_location,
};
pub use routines::{
    TimeOfDay, DetailedActivity, RoutineState, determine_activity, process_colonist_routines,
};
pub use relationships::{
    RelationshipType, Relationship, RelationshipMemory, MemoryEvent,
    RelationshipManager, generate_social_interaction, can_be_romantic, try_romance,
};
