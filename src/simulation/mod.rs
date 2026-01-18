//! World Civilization Simulation System
//!
//! A world-scale civilization simulation where tribes operate as aggregate units.
//! Features complex needs, time-based progression through ages, and balanced
//! cooperation/competition interactions.
//!
//! # Module Structure
//!
//! - `types`: Core type definitions (TribeId, SimTick, ResourceType, etc.)
//! - `params`: Simulation configuration parameters
//! - `tribe`: Tribe structure and subsystems (population, needs, culture)
//! - `resources`: Resource stockpile and extraction systems
//! - `technology`: Age progression and tech tree
//! - `territory`: Territory management and expansion
//! - `interaction`: Diplomacy, trade, conflict, migration
//! - `environment`: Resource depletion tracking
//! - `simulation`: Main simulation state and tick loop
//! - `export`: JSON export and reporting
//! - `society`: Government types and succession systems
//! - `colonists`: Individual and aggregate population tracking
//! - `jobs`: Work assignments and resource production
//! - `workplaces`: Locations where jobs are performed
//!
//! # Usage
//!
//! ```ignore
//! use planet_generation::simulation::{SimulationState, SimulationParams, run_simulation};
//!
//! let params = SimulationParams::default();
//! let state = run_simulation(&world_data, &params, 100, &mut rng);
//! ```

pub mod types;
pub mod params;
pub mod tribe;
pub mod resources;
pub mod technology;
pub mod territory;
pub mod interaction;
pub mod environment;
pub mod simulation;
pub mod export;
pub mod structures;
pub mod roads;
pub mod monsters;
pub mod fauna;
pub mod body;
pub mod characters;
pub mod combat;
pub mod society;
pub mod colonists;
pub mod jobs;
pub mod workplaces;
pub mod activity_log;

// Re-export main types for convenience
pub use types::{TribeId, SimTick, Season, RelationLevel, ResourceType, TileCoord, TreatyType, Treaty};
pub use params::SimulationParams;
pub use tribe::{Tribe, TribeCulture, TribeNeeds, Population, Settlement};
pub use resources::Stockpile;
pub use technology::{Age, TechnologyState, BuildingType};
pub use simulation::{SimulationState, SimulationStats, run_simulation};
pub use export::{export_simulation, generate_summary, export_combat_logs, generate_combat_narrative};
pub use structures::{Structure, StructureId, StructureType, StructureManager};
pub use roads::{RoadSegment, RoadType, RoadNetwork};
pub use monsters::{Monster, MonsterId, MonsterSpecies, MonsterState, MonsterManager};
pub use fauna::{Fauna, FaunaId, FaunaSpecies, FaunaState, FaunaActivity, FaunaManager};
pub use body::{Body, BodyPart, BodyPartId, BodyPlan, DamageType, Wound, WoundType, CombatEffect};
pub use characters::{Character, CharacterId, CharacterManager, Weapon, Armor};
pub use combat::{CombatLogStore, CombatLogEntry, CombatEncounterLog, resolve_attack};

// Colony simulation types
pub use society::{SocietyType, SocietyState, SocietyConfig, SuccessionMethod, SpecialBonus};
pub use colonists::{
    Colonist, ColonistId, ColonistRole, LifeStage, Gender, Attributes,
    SkillType, SkillSet, Skill, skill_productivity,
    MoodState, MoodModifier, MoodModifierType,
    PopulationPool, PopulationCohort,
    NotableColonists, process_notable_lifecycle, target_notable_count,
};
pub use jobs::{
    JobId, JobType, JobCategory, Job, JobDemand, JobDefinition,
    JobManager, assign_all_jobs, process_jobs, JobProcessingResult,
};
pub use workplaces::{Workplace, WorkplaceId, WorkplaceType, WorkplaceManager};
pub use activity_log::{ActivityLog, ActivityEntry, ActivityCategory, ActivityEntity, ActivityStats};
