//! Colonist types and structures
//!
//! Defines individual colonists (notable characters) with attributes, skills, and state.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::simulation::colonists::skills::SkillSet;
use crate::simulation::colonists::mood::MoodState;
use crate::simulation::types::{TileCoord, GlobalLocalCoord};

/// Unique identifier for a colonist
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColonistId(pub u64);

impl fmt::Display for ColonistId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Colonist#{}", self.0)
    }
}

/// Life stage of a colonist
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifeStage {
    /// 0-15 years: Cannot work, requires care
    Child,
    /// 16-64 years: Full worker
    Adult,
    /// 65+ years: Reduced capacity, provides wisdom
    Elder,
}

impl LifeStage {
    /// Get the life stage for a given age
    pub fn from_age(age: u32) -> Self {
        if age < 16 {
            LifeStage::Child
        } else if age < 65 {
            LifeStage::Adult
        } else {
            LifeStage::Elder
        }
    }

    /// Work capacity multiplier
    pub fn work_capacity(&self) -> f32 {
        match self {
            LifeStage::Child => 0.0,  // Cannot work
            LifeStage::Adult => 1.0,  // Full capacity
            LifeStage::Elder => 0.5,  // Reduced capacity
        }
    }

    /// Food consumption multiplier
    pub fn food_consumption(&self) -> f32 {
        match self {
            LifeStage::Child => 0.5,
            LifeStage::Adult => 1.0,
            LifeStage::Elder => 0.8,
        }
    }

    /// Can this life stage have children?
    pub fn can_reproduce(&self) -> bool {
        matches!(self, LifeStage::Adult)
    }
}

/// Biological sex of a colonist
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        if rng.gen_bool(0.5) {
            Gender::Male
        } else {
            Gender::Female
        }
    }
}

/// Role/importance of a colonist
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColonistRole {
    /// Regular citizen
    Citizen,
    /// Council member (advisor)
    CouncilMember,
    /// Leader of the tribe
    Leader,
    /// Elite warrior/champion
    Champion,
    /// Skilled specialist (master craftsman, etc.)
    Specialist,
    /// Religious figure (priest, shaman)
    Priest,
}

impl ColonistRole {
    /// Importance weight for tracking (higher = more important to track)
    pub fn importance(&self) -> u32 {
        match self {
            ColonistRole::Citizen => 1,
            ColonistRole::CouncilMember => 5,
            ColonistRole::Leader => 10,
            ColonistRole::Champion => 4,
            ColonistRole::Specialist => 3,
            ColonistRole::Priest => 4,
        }
    }

    /// Morale boost this role provides to the tribe
    pub fn morale_bonus(&self) -> f32 {
        match self {
            ColonistRole::Citizen => 0.0,
            ColonistRole::CouncilMember => 0.02,
            ColonistRole::Leader => 0.1,
            ColonistRole::Champion => 0.05,
            ColonistRole::Specialist => 0.01,
            ColonistRole::Priest => 0.03,
        }
    }
}

impl Default for ColonistRole {
    fn default() -> Self {
        ColonistRole::Citizen
    }
}

/// Activity state for a colonist (what they're currently doing)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColonistActivityState {
    /// Near capital, no current task
    Idle,
    /// Moving to work location
    Traveling,
    /// At work site, performing job
    Working,
    /// Going back home/capital
    Returning,
    /// Fleeing from danger
    Fleeing,
    /// Interacting with other colonists
    Socializing,
    /// Guards patrolling territory borders
    Patrolling,
    /// Scouts exploring beyond territory
    Scouting,
}

impl Default for ColonistActivityState {
    fn default() -> Self {
        ColonistActivityState::Idle
    }
}

/// Base attributes for a colonist (0-20 scale, 10 is average)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attributes {
    /// Physical strength - affects melee combat, hauling
    pub strength: u8,
    /// Speed and reflexes - affects combat, fleeing, work speed
    pub agility: u8,
    /// Health and stamina - affects disease resistance, recovery
    pub constitution: u8,
    /// Mental acuity - affects research, crafting quality
    pub intelligence: u8,
    /// Social skill - affects trade, diplomacy, leadership
    pub charisma: u8,
    /// Perceptiveness - affects hunting, spotting dangers
    pub perception: u8,
}

impl Attributes {
    /// Create random attributes
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        Attributes {
            strength: Self::random_attribute(rng),
            agility: Self::random_attribute(rng),
            constitution: Self::random_attribute(rng),
            intelligence: Self::random_attribute(rng),
            charisma: Self::random_attribute(rng),
            perception: Self::random_attribute(rng),
        }
    }

    /// Generate a single random attribute (bell curve around 10)
    fn random_attribute<R: Rng>(rng: &mut R) -> u8 {
        // Roll 3d6 + 2 to get values mostly between 5-15
        let roll: u8 = (1..=3).map(|_| rng.gen_range(1..=6)).sum();
        (roll + 2).min(20)
    }

    /// Get average attribute value
    pub fn average(&self) -> f32 {
        (self.strength + self.agility + self.constitution +
         self.intelligence + self.charisma + self.perception) as f32 / 6.0
    }

    /// Inherit attributes from parents with some variation
    pub fn inherit<R: Rng>(parent_a: &Attributes, parent_b: &Attributes, rng: &mut R) -> Self {
        fn inherit_attr<R: Rng>(a: u8, b: u8, rng: &mut R) -> u8 {
            let base = if rng.gen_bool(0.5) { a } else { b };
            let variation = rng.gen_range(-2i8..=2i8);
            (base as i8 + variation).clamp(1, 20) as u8
        }

        Attributes {
            strength: inherit_attr(parent_a.strength, parent_b.strength, rng),
            agility: inherit_attr(parent_a.agility, parent_b.agility, rng),
            constitution: inherit_attr(parent_a.constitution, parent_b.constitution, rng),
            intelligence: inherit_attr(parent_a.intelligence, parent_b.intelligence, rng),
            charisma: inherit_attr(parent_a.charisma, parent_b.charisma, rng),
            perception: inherit_attr(parent_a.perception, parent_b.perception, rng),
        }
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Attributes {
            strength: 10,
            agility: 10,
            constitution: 10,
            intelligence: 10,
            charisma: 10,
            perception: 10,
        }
    }
}

/// A notable colonist tracked individually
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Colonist {
    /// Unique identifier
    pub id: ColonistId,
    /// Name of the colonist
    pub name: String,
    /// Age in years
    pub age: u32,
    /// Biological sex
    pub gender: Gender,
    /// Current life stage
    pub life_stage: LifeStage,
    /// Role in society
    pub role: ColonistRole,
    /// Base attributes
    pub attributes: Attributes,
    /// Skills and their levels
    pub skills: SkillSet,
    /// Current mood state
    pub mood: MoodState,
    /// Current job assignment (if any)
    pub current_job: Option<crate::simulation::jobs::types::JobType>,
    /// Health (0.0 = dead, 1.0 = full health)
    pub health: f32,
    /// Is this colonist alive?
    pub is_alive: bool,
    /// Tick when this colonist was born
    pub birth_tick: u64,
    /// Notable achievements or events
    pub notable_events: Vec<String>,
    /// Parent IDs (if known)
    pub parents: (Option<ColonistId>, Option<ColonistId>),
    /// Children IDs
    pub children: Vec<ColonistId>,
    /// Spouse ID (if married)
    pub spouse: Option<ColonistId>,
    /// Current location on the map (world tile)
    pub location: TileCoord,
    /// Position in global local coordinates (for local map rendering)
    pub local_position: GlobalLocalCoord,
    /// What the colonist is currently doing
    pub activity_state: ColonistActivityState,
    /// Destination for movement (if traveling)
    pub destination: Option<TileCoord>,
    /// Local destination for fine movement
    pub local_destination: Option<GlobalLocalCoord>,
    /// Tick when the colonist last moved
    pub last_move_tick: u64,
    /// Whether this colonist is under player control (skips automatic state changes)
    pub player_controlled: bool,
}

impl Colonist {
    /// Create a new colonist
    pub fn new<R: Rng>(
        id: ColonistId,
        name: String,
        age: u32,
        gender: Gender,
        current_tick: u64,
        location: TileCoord,
        rng: &mut R,
    ) -> Self {
        let life_stage = LifeStage::from_age(age);

        // Spread colonists within a radius of the tile center
        let base_pos = GlobalLocalCoord::from_world_tile(location);
        let scatter_radius = 15i32;
        let local_position = GlobalLocalCoord::new(
            (base_pos.x as i32 + rng.gen_range(-scatter_radius..=scatter_radius)).max(0) as u32,
            (base_pos.y as i32 + rng.gen_range(-scatter_radius..=scatter_radius)).max(0) as u32,
        );

        Colonist {
            id,
            name,
            age,
            gender,
            life_stage,
            role: ColonistRole::Citizen,
            attributes: Attributes::random(rng),
            skills: SkillSet::random(rng),
            mood: MoodState::default(),
            current_job: None,
            health: 1.0,
            is_alive: true,
            birth_tick: current_tick.saturating_sub((age * 4) as u64),
            notable_events: Vec::new(),
            parents: (None, None),
            children: Vec::new(),
            spouse: None,
            location,
            local_position,
            activity_state: ColonistActivityState::Idle,
            destination: None,
            local_destination: None,
            last_move_tick: current_tick,
            player_controlled: false,
        }
    }

    /// Create a child colonist with inherited attributes
    pub fn create_child<R: Rng>(
        id: ColonistId,
        name: String,
        parent_a: &Colonist,
        parent_b: &Colonist,
        current_tick: u64,
        location: TileCoord,
        rng: &mut R,
    ) -> Self {
        let gender = Gender::random(rng);
        let attributes = Attributes::inherit(&parent_a.attributes, &parent_b.attributes, rng);

        // Place child near parents
        let base_pos = GlobalLocalCoord::from_world_tile(location);
        let scatter_radius = 5i32;
        let local_position = GlobalLocalCoord::new(
            (base_pos.x as i32 + rng.gen_range(-scatter_radius..=scatter_radius)).max(0) as u32,
            (base_pos.y as i32 + rng.gen_range(-scatter_radius..=scatter_radius)).max(0) as u32,
        );

        Colonist {
            id,
            name,
            age: 0,
            gender,
            life_stage: LifeStage::Child,
            role: ColonistRole::Citizen,
            attributes,
            skills: SkillSet::new(), // Children start with no skills
            mood: MoodState::default(),
            current_job: None,
            health: 1.0,
            is_alive: true,
            birth_tick: current_tick,
            notable_events: vec!["Born".to_string()],
            parents: (Some(parent_a.id), Some(parent_b.id)),
            children: Vec::new(),
            spouse: None,
            location,
            local_position,
            activity_state: ColonistActivityState::Idle,
            destination: None,
            local_destination: None,
            last_move_tick: current_tick,
            player_controlled: false,
        }
    }

    /// Age the colonist by one year
    pub fn age_one_year(&mut self) {
        self.age += 1;
        self.life_stage = LifeStage::from_age(self.age);

        // Elders may lose some physical attributes
        if self.life_stage == LifeStage::Elder {
            self.attributes.strength = self.attributes.strength.saturating_sub(1).max(5);
            self.attributes.agility = self.attributes.agility.saturating_sub(1).max(5);
        }
    }

    /// Check if colonist can work
    pub fn can_work(&self) -> bool {
        self.is_alive && self.life_stage != LifeStage::Child && self.health > 0.2
    }

    /// Get work efficiency (0.0 - 2.0+)
    pub fn work_efficiency(&self) -> f32 {
        let base = self.life_stage.work_capacity();
        let health_mod = self.health;
        let mood_mod = self.mood.work_modifier();

        base * health_mod * mood_mod
    }

    /// Get combat effectiveness
    pub fn combat_effectiveness(&self) -> f32 {
        let str_mod = self.attributes.strength as f32 / 10.0;
        let agi_mod = self.attributes.agility as f32 / 10.0;
        let health_mod = self.health;
        let skill_mod = self.skills.get_level(crate::simulation::colonists::skills::SkillType::Combat) as f32 / 10.0;

        (str_mod + agi_mod) * 0.5 * health_mod * (0.5 + skill_mod * 0.5)
    }

    /// Get leadership ability
    pub fn leadership_ability(&self) -> f32 {
        let cha_mod = self.attributes.charisma as f32 / 10.0;
        let int_mod = self.attributes.intelligence as f32 / 10.0;
        let skill_mod = self.skills.get_level(crate::simulation::colonists::skills::SkillType::Leadership) as f32 / 10.0;

        (cha_mod + int_mod * 0.5) * (0.5 + skill_mod * 0.5)
    }

    /// Take damage, returns true if colonist dies
    pub fn take_damage(&mut self, amount: f32) -> bool {
        self.health = (self.health - amount).max(0.0);
        if self.health <= 0.0 {
            self.is_alive = false;
            true
        } else {
            false
        }
    }

    /// Heal the colonist
    pub fn heal(&mut self, amount: f32) {
        if self.is_alive {
            self.health = (self.health + amount).min(1.0);
        }
    }

    /// Add a notable event to history
    pub fn add_event(&mut self, event: String) {
        self.notable_events.push(event);
        // Keep only last 10 events
        if self.notable_events.len() > 10 {
            self.notable_events.remove(0);
        }
    }

    /// Get a short description
    pub fn short_description(&self) -> String {
        let role_str = match self.role {
            ColonistRole::Leader => "Leader",
            ColonistRole::CouncilMember => "Council",
            ColonistRole::Champion => "Champion",
            ColonistRole::Specialist => "Specialist",
            ColonistRole::Priest => "Priest",
            ColonistRole::Citizen => "Citizen",
        };
        format!("{} ({}, {}, {})", self.name, role_str, self.age,
                if self.gender == Gender::Male { "M" } else { "F" })
    }

    /// Get the ASCII character for map display
    pub fn map_char(&self) -> char {
        use crate::simulation::jobs::types::JobType;

        match self.role {
            ColonistRole::Leader => 'K',
            ColonistRole::Champion => 'C',
            ColonistRole::Priest => 'P',
            ColonistRole::CouncilMember => 'c',
            ColonistRole::Specialist => 's',
            ColonistRole::Citizen => match self.current_job {
                Some(JobType::Farmer) => 'f',
                Some(JobType::Miner) => 'm',
                Some(JobType::Guard) => 'g',
                Some(JobType::Warrior) => 'W',
                Some(JobType::Scout) => 'S',
                Some(JobType::Woodcutter) => 'w',
                Some(JobType::Hunter) => 'h',
                Some(JobType::Fisher) => 'F',
                Some(JobType::Builder) => 'b',
                Some(JobType::Healer) => 'H',
                Some(JobType::Scholar) => 'R',
                Some(JobType::Smith) => 'A',
                _ => '@',
            },
        }
    }

    /// Get the RGB color for map display
    pub fn color(&self) -> (u8, u8, u8) {
        // First check activity state for visual feedback
        match self.activity_state {
            ColonistActivityState::Working => (100, 255, 100),  // Bright green when working
            ColonistActivityState::Traveling => (255, 255, 100), // Yellow when traveling
            ColonistActivityState::Fleeing => (255, 100, 100),   // Red when fleeing
            ColonistActivityState::Patrolling => (100, 200, 255), // Light blue when patrolling
            ColonistActivityState::Scouting => (200, 150, 255),  // Purple when scouting
            ColonistActivityState::Returning => (255, 200, 100), // Orange when returning
            ColonistActivityState::Socializing => (255, 200, 200), // Pink when socializing
            ColonistActivityState::Idle => {
                // Default to role-based color when idle
                match self.role {
                    ColonistRole::Leader => (255, 215, 0),      // Gold
                    ColonistRole::Champion => (220, 20, 60),    // Crimson
                    ColonistRole::Priest => (148, 0, 211),      // Purple
                    ColonistRole::CouncilMember => (100, 149, 237), // Cornflower blue
                    ColonistRole::Specialist => (255, 165, 0),  // Orange
                    ColonistRole::Citizen => (200, 200, 200),   // Light gray
                }
            }
        }
    }

    /// Get a description of the current activity
    pub fn activity_description(&self) -> &'static str {
        match self.activity_state {
            ColonistActivityState::Idle => "Idle",
            ColonistActivityState::Working => "Working",
            ColonistActivityState::Traveling => "Traveling",
            ColonistActivityState::Returning => "Returning home",
            ColonistActivityState::Patrolling => "Patrolling",
            ColonistActivityState::Scouting => "Scouting",
            ColonistActivityState::Fleeing => "Fleeing!",
            ColonistActivityState::Socializing => "Socializing",
        }
    }
}

/// Name generation for colonists
#[derive(Clone, Debug)]
pub struct NameGenerator {
    male_first: Vec<&'static str>,
    female_first: Vec<&'static str>,
    surnames: Vec<&'static str>,
}

impl Default for NameGenerator {
    fn default() -> Self {
        NameGenerator {
            male_first: vec![
                "Aldric", "Beren", "Cadmus", "Doran", "Edmund", "Falk", "Gareth", "Harald",
                "Ingvar", "Jorund", "Kael", "Leofric", "Magnus", "Nils", "Osric", "Ragnar",
                "Sigurd", "Theron", "Ulric", "Valdis", "Wulfric", "Yngvar", "Zoran", "Aeric",
                "Brynn", "Cormac", "Derrick", "Egon", "Finn", "Godfrey", "Halvar", "Ivan",
                "Jasper", "Kellan", "Lars", "Milo", "Niall", "Odin", "Pierce", "Quinn",
                "Roland", "Stefan", "Torsten", "Uther", "Viktor", "Willem", "Xander", "Yorick",
            ],
            female_first: vec![
                "Astrid", "Brenna", "Cara", "Dagny", "Eira", "Freya", "Greta", "Hilda",
                "Ingrid", "Jorunn", "Kira", "Liv", "Maren", "Nadia", "Olga", "Petra",
                "Quinn", "Ragna", "Sigrid", "Thora", "Una", "Vera", "Wren", "Xena",
                "Ylva", "Zara", "Alva", "Bodil", "Celine", "Dagmar", "Edda", "Freja",
                "Gudrun", "Helga", "Ida", "Johanna", "Katrin", "Linnea", "Maja", "Nora",
                "Olena", "Pia", "Rosa", "Signe", "Tova", "Ulla", "Viola", "Wilma",
            ],
            surnames: vec![
                "Ironhand", "Stoneheart", "Goldmantle", "Silverbrow", "Blackwood", "Redmane",
                "Whitestorm", "Greywolf", "Darkwater", "Brightforge", "Swiftarrow", "Strongbow",
                "Firebrand", "Frostborn", "Thunderhelm", "Shadowbane", "Lightbringer", "Dawnguard",
                "Nightfall", "Stormwind", "Riverdale", "Mountaincrest", "Valleyford", "Oakenshield",
                "Flamekeeper", "Windrunner", "Earthshaker", "Skywalker", "Deepdelver", "Highborn",
                "Longshadow", "Wildwood", "Clearwater", "Steelhammer", "Coppersmith", "Bronzefist",
            ],
        }
    }
}

impl NameGenerator {
    /// Generate a random name
    pub fn generate<R: Rng>(&self, gender: Gender, rng: &mut R) -> String {
        let first = match gender {
            Gender::Male => self.male_first[rng.gen_range(0..self.male_first.len())],
            Gender::Female => self.female_first[rng.gen_range(0..self.female_first.len())],
        };
        let surname = self.surnames[rng.gen_range(0..self.surnames.len())];
        format!("{} {}", first, surname)
    }

    /// Generate a name inheriting surname from parent
    pub fn generate_child_name<R: Rng>(
        &self,
        gender: Gender,
        parent_surname: &str,
        rng: &mut R,
    ) -> String {
        let first = match gender {
            Gender::Male => self.male_first[rng.gen_range(0..self.male_first.len())],
            Gender::Female => self.female_first[rng.gen_range(0..self.female_first.len())],
        };
        format!("{} {}", first, parent_surname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_life_stage_from_age() {
        assert_eq!(LifeStage::from_age(0), LifeStage::Child);
        assert_eq!(LifeStage::from_age(15), LifeStage::Child);
        assert_eq!(LifeStage::from_age(16), LifeStage::Adult);
        assert_eq!(LifeStage::from_age(64), LifeStage::Adult);
        assert_eq!(LifeStage::from_age(65), LifeStage::Elder);
    }

    #[test]
    fn test_attributes_random() {
        let mut rng = rand::thread_rng();
        let attrs = Attributes::random(&mut rng);
        assert!(attrs.strength >= 1 && attrs.strength <= 20);
        assert!(attrs.agility >= 1 && attrs.agility <= 20);
    }

    #[test]
    fn test_name_generator() {
        let gen = NameGenerator::default();
        let mut rng = rand::thread_rng();

        let male_name = gen.generate(Gender::Male, &mut rng);
        let female_name = gen.generate(Gender::Female, &mut rng);

        assert!(!male_name.is_empty());
        assert!(!female_name.is_empty());
        assert!(male_name.contains(' '));
        assert!(female_name.contains(' '));
    }
}
