//! Colonist daily routines and activities
//!
//! Provides detailed daily schedules and activities for individual colonists,
//! making the simulation more lifelike and visible.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::simulation::types::{GlobalLocalCoord, TileCoord};
use crate::simulation::colonists::types::{
    Colonist, ColonistActivityState, ColonistRole, LifeStage,
};
use crate::simulation::jobs::types::JobType;

/// Time of day (based on tick within a year - 4 ticks = 1 year, so we simulate within ticks)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDay {
    Dawn,      // 6-8 AM
    Morning,   // 8-12 PM
    Midday,    // 12-2 PM
    Afternoon, // 2-6 PM
    Evening,   // 6-10 PM
    Night,     // 10 PM - 6 AM
}

impl TimeOfDay {
    /// Get time of day from sub-tick (0-99 represents a day within the tick)
    pub fn from_subtick(subtick: u32) -> Self {
        match subtick % 100 {
            0..=9 => TimeOfDay::Night,
            10..=19 => TimeOfDay::Dawn,
            20..=39 => TimeOfDay::Morning,
            40..=49 => TimeOfDay::Midday,
            50..=74 => TimeOfDay::Afternoon,
            75..=89 => TimeOfDay::Evening,
            _ => TimeOfDay::Night,
        }
    }

    /// Is this a working time?
    pub fn is_work_time(&self) -> bool {
        matches!(self, TimeOfDay::Morning | TimeOfDay::Afternoon)
    }

    /// Is this a social time?
    pub fn is_social_time(&self) -> bool {
        matches!(self, TimeOfDay::Evening | TimeOfDay::Midday)
    }

    /// Is this a rest time?
    pub fn is_rest_time(&self) -> bool {
        matches!(self, TimeOfDay::Night | TimeOfDay::Dawn)
    }
}

/// Detailed activity a colonist is performing
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailedActivity {
    // Rest activities
    Sleeping,
    WakingUp,
    Resting,

    // Personal care
    Eating,
    Drinking,
    Bathing,
    Dressing,

    // Work activities
    Farming,
    Mining,
    Woodcutting,
    Hunting,
    Fishing,
    Building,
    Crafting,
    Smithing,
    Healing,
    Researching,
    Guarding,
    Patrolling,
    Scouting,
    Training,

    // Social activities
    Talking,
    Trading,
    Teaching,
    Learning,
    Celebrating,
    Mourning,
    Praying,
    Storytelling,

    // Movement
    Walking,
    Running,
    Riding,
    Swimming,

    // Recreation
    Playing,
    Relaxing,
    DrinkingSocially,
    Gambling,
    Singing,
    Dancing,

    // Leadership
    Commanding,
    Judging,
    Planning,
    Inspecting,

    // Child activities
    BeingCaredFor,
    PlayingGames,
    Exploring,

    // Elder activities
    Advising,
    Reminiscing,
    MentoringYouth,
}

impl DetailedActivity {
    /// Get the descriptive text for this activity
    pub fn description(&self) -> &'static str {
        match self {
            DetailedActivity::Sleeping => "sleeping peacefully",
            DetailedActivity::WakingUp => "waking up",
            DetailedActivity::Resting => "resting",
            DetailedActivity::Eating => "eating a meal",
            DetailedActivity::Drinking => "having a drink",
            DetailedActivity::Bathing => "bathing",
            DetailedActivity::Dressing => "getting dressed",
            DetailedActivity::Farming => "working the fields",
            DetailedActivity::Mining => "mining ore",
            DetailedActivity::Woodcutting => "chopping wood",
            DetailedActivity::Hunting => "hunting game",
            DetailedActivity::Fishing => "fishing",
            DetailedActivity::Building => "constructing a building",
            DetailedActivity::Crafting => "crafting items",
            DetailedActivity::Smithing => "working the forge",
            DetailedActivity::Healing => "tending to the sick",
            DetailedActivity::Researching => "studying scrolls",
            DetailedActivity::Guarding => "standing guard",
            DetailedActivity::Patrolling => "patrolling the area",
            DetailedActivity::Scouting => "scouting the wilderness",
            DetailedActivity::Training => "practicing combat",
            DetailedActivity::Talking => "chatting with others",
            DetailedActivity::Trading => "bartering goods",
            DetailedActivity::Teaching => "teaching skills",
            DetailedActivity::Learning => "learning new things",
            DetailedActivity::Celebrating => "celebrating",
            DetailedActivity::Mourning => "mourning the fallen",
            DetailedActivity::Praying => "praying at the shrine",
            DetailedActivity::Storytelling => "telling stories",
            DetailedActivity::Walking => "walking",
            DetailedActivity::Running => "running",
            DetailedActivity::Riding => "riding",
            DetailedActivity::Swimming => "swimming",
            DetailedActivity::Playing => "playing games",
            DetailedActivity::Relaxing => "relaxing",
            DetailedActivity::DrinkingSocially => "enjoying drinks",
            DetailedActivity::Gambling => "gambling",
            DetailedActivity::Singing => "singing songs",
            DetailedActivity::Dancing => "dancing",
            DetailedActivity::Commanding => "giving orders",
            DetailedActivity::Judging => "resolving disputes",
            DetailedActivity::Planning => "making plans",
            DetailedActivity::Inspecting => "inspecting the settlement",
            DetailedActivity::BeingCaredFor => "being cared for",
            DetailedActivity::PlayingGames => "playing childhood games",
            DetailedActivity::Exploring => "exploring curiously",
            DetailedActivity::Advising => "giving wise advice",
            DetailedActivity::Reminiscing => "reminiscing about old times",
            DetailedActivity::MentoringYouth => "mentoring the young",
        }
    }

    /// Convert to basic activity state
    pub fn to_activity_state(&self) -> ColonistActivityState {
        match self {
            DetailedActivity::Walking
            | DetailedActivity::Running
            | DetailedActivity::Riding
            | DetailedActivity::Swimming => ColonistActivityState::Traveling,

            DetailedActivity::Farming
            | DetailedActivity::Mining
            | DetailedActivity::Woodcutting
            | DetailedActivity::Hunting
            | DetailedActivity::Fishing
            | DetailedActivity::Building
            | DetailedActivity::Crafting
            | DetailedActivity::Smithing
            | DetailedActivity::Healing
            | DetailedActivity::Researching => ColonistActivityState::Working,

            DetailedActivity::Guarding => ColonistActivityState::Patrolling,
            DetailedActivity::Patrolling => ColonistActivityState::Patrolling,
            DetailedActivity::Scouting => ColonistActivityState::Scouting,

            DetailedActivity::Talking
            | DetailedActivity::Trading
            | DetailedActivity::Teaching
            | DetailedActivity::Learning
            | DetailedActivity::Celebrating
            | DetailedActivity::Storytelling
            | DetailedActivity::Playing
            | DetailedActivity::Dancing
            | DetailedActivity::Singing
            | DetailedActivity::DrinkingSocially
            | DetailedActivity::Gambling => ColonistActivityState::Socializing,

            _ => ColonistActivityState::Idle,
        }
    }
}

/// A colonist's current routine state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutineState {
    /// Current detailed activity
    pub current_activity: DetailedActivity,
    /// Sub-tick counter (0-99 represents progress through the day)
    pub subtick: u32,
    /// Target position for current activity
    pub activity_target: Option<GlobalLocalCoord>,
    /// How long they've been doing the current activity
    pub activity_duration: u32,
    /// Interaction partner (if socializing)
    pub interaction_partner: Option<crate::simulation::colonists::types::ColonistId>,
}

impl Default for RoutineState {
    fn default() -> Self {
        RoutineState {
            current_activity: DetailedActivity::Sleeping,
            subtick: 0,
            activity_target: None,
            activity_duration: 0,
            interaction_partner: None,
        }
    }
}

impl RoutineState {
    /// Get the current time of day
    pub fn time_of_day(&self) -> TimeOfDay {
        TimeOfDay::from_subtick(self.subtick)
    }

    /// Advance the routine by one step
    pub fn advance(&mut self) {
        self.subtick = (self.subtick + 1) % 100;
        self.activity_duration += 1;
    }
}

/// Determine the best activity for a colonist based on time, role, and needs
pub fn determine_activity<R: Rng>(
    colonist: &Colonist,
    time: TimeOfDay,
    rng: &mut R,
) -> DetailedActivity {
    // Children have different routines
    if colonist.life_stage == LifeStage::Child {
        return determine_child_activity(time, rng);
    }

    // Elders have different routines
    if colonist.life_stage == LifeStage::Elder {
        return determine_elder_activity(colonist, time, rng);
    }

    // Role-based activities take priority during work hours
    if time.is_work_time() {
        return determine_work_activity(colonist, rng);
    }

    // Social time
    if time.is_social_time() {
        return determine_social_activity(colonist, rng);
    }

    // Rest time
    if time.is_rest_time() {
        if time == TimeOfDay::Dawn {
            return DetailedActivity::WakingUp;
        }
        return DetailedActivity::Sleeping;
    }

    // Default based on needs
    if colonist.health < 0.5 {
        return DetailedActivity::Resting;
    }

    DetailedActivity::Resting
}

/// Determine activity for a child
fn determine_child_activity<R: Rng>(time: TimeOfDay, rng: &mut R) -> DetailedActivity {
    match time {
        TimeOfDay::Night => DetailedActivity::Sleeping,
        TimeOfDay::Dawn => DetailedActivity::WakingUp,
        TimeOfDay::Morning | TimeOfDay::Afternoon => {
            let roll = rng.gen::<f32>();
            if roll < 0.4 {
                DetailedActivity::PlayingGames
            } else if roll < 0.6 {
                DetailedActivity::Learning
            } else if roll < 0.8 {
                DetailedActivity::Exploring
            } else {
                DetailedActivity::BeingCaredFor
            }
        }
        TimeOfDay::Midday => DetailedActivity::Eating,
        TimeOfDay::Evening => {
            let roll = rng.gen::<f32>();
            if roll < 0.5 {
                DetailedActivity::PlayingGames
            } else {
                DetailedActivity::Eating
            }
        }
    }
}

/// Determine activity for an elder
fn determine_elder_activity<R: Rng>(
    colonist: &Colonist,
    time: TimeOfDay,
    rng: &mut R,
) -> DetailedActivity {
    match time {
        TimeOfDay::Night => DetailedActivity::Sleeping,
        TimeOfDay::Dawn => DetailedActivity::WakingUp,
        TimeOfDay::Morning => {
            let roll = rng.gen::<f32>();
            if roll < 0.3 {
                DetailedActivity::Advising
            } else if roll < 0.5 {
                DetailedActivity::MentoringYouth
            } else if roll < 0.7 {
                DetailedActivity::Praying
            } else {
                DetailedActivity::Resting
            }
        }
        TimeOfDay::Midday => DetailedActivity::Eating,
        TimeOfDay::Afternoon => {
            let roll = rng.gen::<f32>();
            if roll < 0.3 {
                DetailedActivity::Storytelling
            } else if roll < 0.5 {
                DetailedActivity::Reminiscing
            } else if roll < 0.7 {
                DetailedActivity::Teaching
            } else {
                DetailedActivity::Resting
            }
        }
        TimeOfDay::Evening => {
            let roll = rng.gen::<f32>();
            if roll < 0.4 {
                DetailedActivity::Storytelling
            } else if roll < 0.6 {
                DetailedActivity::Talking
            } else {
                DetailedActivity::Resting
            }
        }
    }
}

/// Determine work activity based on job
fn determine_work_activity<R: Rng>(colonist: &Colonist, rng: &mut R) -> DetailedActivity {
    // Role-based activities
    match colonist.role {
        ColonistRole::Leader => {
            let roll = rng.gen::<f32>();
            if roll < 0.3 {
                return DetailedActivity::Commanding;
            } else if roll < 0.5 {
                return DetailedActivity::Planning;
            } else if roll < 0.7 {
                return DetailedActivity::Inspecting;
            } else {
                return DetailedActivity::Judging;
            }
        }
        ColonistRole::Champion => {
            let roll = rng.gen::<f32>();
            if roll < 0.5 {
                return DetailedActivity::Training;
            } else if roll < 0.8 {
                return DetailedActivity::Patrolling;
            } else {
                return DetailedActivity::Commanding;
            }
        }
        ColonistRole::Priest => {
            let roll = rng.gen::<f32>();
            if roll < 0.4 {
                return DetailedActivity::Praying;
            } else if roll < 0.7 {
                return DetailedActivity::Teaching;
            } else {
                return DetailedActivity::Healing;
            }
        }
        ColonistRole::CouncilMember => {
            let roll = rng.gen::<f32>();
            if roll < 0.4 {
                return DetailedActivity::Planning;
            } else if roll < 0.7 {
                return DetailedActivity::Inspecting;
            } else {
                return DetailedActivity::Trading;
            }
        }
        _ => {}
    }

    // Job-based activities
    if let Some(job) = colonist.current_job {
        return job_to_activity(job, rng);
    }

    // Default work activity
    let roll = rng.gen::<f32>();
    if roll < 0.3 {
        DetailedActivity::Crafting
    } else if roll < 0.5 {
        DetailedActivity::Building
    } else if roll < 0.7 {
        DetailedActivity::Farming
    } else {
        DetailedActivity::Walking
    }
}

/// Convert job type to activity
fn job_to_activity<R: Rng>(job: JobType, rng: &mut R) -> DetailedActivity {
    match job {
        JobType::Farmer => DetailedActivity::Farming,
        JobType::Miner => DetailedActivity::Mining,
        JobType::Woodcutter => DetailedActivity::Woodcutting,
        JobType::Hunter => {
            if rng.gen::<f32>() < 0.3 {
                DetailedActivity::Walking
            } else {
                DetailedActivity::Hunting
            }
        }
        JobType::Fisher => DetailedActivity::Fishing,
        JobType::Builder => DetailedActivity::Building,
        JobType::Smith => DetailedActivity::Smithing,
        JobType::Healer => DetailedActivity::Healing,
        JobType::Scholar => DetailedActivity::Researching,
        JobType::Guard => {
            if rng.gen::<f32>() < 0.5 {
                DetailedActivity::Guarding
            } else {
                DetailedActivity::Patrolling
            }
        }
        JobType::Warrior => DetailedActivity::Training,
        JobType::Scout => DetailedActivity::Scouting,
        JobType::Cook => DetailedActivity::Crafting,
        JobType::Craftsperson => DetailedActivity::Crafting,
        JobType::Priest => DetailedActivity::Praying,
        JobType::Hauler => DetailedActivity::Walking,
        JobType::Idle => DetailedActivity::Resting,
    }
}

/// Determine social activity
fn determine_social_activity<R: Rng>(colonist: &Colonist, rng: &mut R) -> DetailedActivity {
    let roll = rng.gen::<f32>();

    // More extroverted people have different social activities
    let is_social = colonist.attributes.charisma > 12;

    if roll < 0.25 {
        DetailedActivity::Eating
    } else if roll < 0.4 {
        DetailedActivity::Talking
    } else if roll < 0.5 {
        if is_social {
            DetailedActivity::Dancing
        } else {
            DetailedActivity::Relaxing
        }
    } else if roll < 0.6 {
        if is_social {
            DetailedActivity::Singing
        } else {
            DetailedActivity::DrinkingSocially
        }
    } else if roll < 0.7 {
        DetailedActivity::Playing
    } else if roll < 0.8 {
        DetailedActivity::Trading
    } else if roll < 0.9 {
        DetailedActivity::Storytelling
    } else {
        DetailedActivity::Praying
    }
}

/// Process routines for all colonists in a collection
pub fn process_colonist_routines<R: Rng>(
    colonists: &mut std::collections::HashMap<
        crate::simulation::colonists::types::ColonistId,
        Colonist,
    >,
    current_tick: u64,
    rng: &mut R,
) {
    // Use the tick to determine a pseudo-subtick for time of day
    let subtick = ((current_tick * 25) % 100) as u32;
    let time = TimeOfDay::from_subtick(subtick);

    for colonist in colonists.values_mut() {
        if !colonist.is_alive {
            continue;
        }

        // Skip player-controlled colonists (they manage their own state)
        if colonist.player_controlled {
            continue;
        }

        // Determine appropriate activity based on time and role
        let activity = determine_activity(colonist, time, rng);
        colonist.activity_state = activity.to_activity_state();
    }
}
