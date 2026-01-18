//! Technology ages and progression

use serde::{Deserialize, Serialize};

/// Technological age of a civilization
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Age {
    Stone,
    Copper,
    Bronze,
    Iron,
    Classical,
    Medieval,
    Renaissance,
}

impl Default for Age {
    fn default() -> Self {
        Age::Stone
    }
}

impl Age {
    /// Get the next age, if any
    pub fn next(&self) -> Option<Age> {
        match self {
            Age::Stone => Some(Age::Copper),
            Age::Copper => Some(Age::Bronze),
            Age::Bronze => Some(Age::Iron),
            Age::Iron => Some(Age::Classical),
            Age::Classical => Some(Age::Medieval),
            Age::Medieval => Some(Age::Renaissance),
            Age::Renaissance => None,
        }
    }

    /// Get the previous age, if any
    pub fn previous(&self) -> Option<Age> {
        match self {
            Age::Stone => None,
            Age::Copper => Some(Age::Stone),
            Age::Bronze => Some(Age::Copper),
            Age::Iron => Some(Age::Bronze),
            Age::Classical => Some(Age::Iron),
            Age::Medieval => Some(Age::Classical),
            Age::Renaissance => Some(Age::Medieval),
        }
    }

    /// Military strength multiplier for this age
    pub fn military_multiplier(&self) -> f32 {
        match self {
            Age::Stone => 1.0,
            Age::Copper => 1.2,
            Age::Bronze => 1.5,
            Age::Iron => 2.0,
            Age::Classical => 2.5,
            Age::Medieval => 3.0,
            Age::Renaissance => 4.0,
        }
    }

    /// Production efficiency multiplier
    pub fn production_multiplier(&self) -> f32 {
        match self {
            Age::Stone => 1.0,
            Age::Copper => 1.1,
            Age::Bronze => 1.2,
            Age::Iron => 1.4,
            Age::Classical => 1.6,
            Age::Medieval => 1.8,
            Age::Renaissance => 2.2,
        }
    }

    /// Required population to enter this age
    pub fn required_population(&self) -> u32 {
        match self {
            Age::Stone => 0,
            Age::Copper => 50,
            Age::Bronze => 100,
            Age::Iron => 200,
            Age::Classical => 350,
            Age::Medieval => 500,
            Age::Renaissance => 750,
        }
    }

    /// Age index for calculations
    pub fn index(&self) -> usize {
        match self {
            Age::Stone => 0,
            Age::Copper => 1,
            Age::Bronze => 2,
            Age::Iron => 3,
            Age::Classical => 4,
            Age::Medieval => 5,
            Age::Renaissance => 6,
        }
    }

    /// Get all ages in order
    pub fn all() -> &'static [Age] {
        &[
            Age::Stone,
            Age::Copper,
            Age::Bronze,
            Age::Iron,
            Age::Classical,
            Age::Medieval,
            Age::Renaissance,
        ]
    }
}

/// Requirements to advance to a new age
#[derive(Clone, Debug)]
pub struct AgeRequirements {
    pub age: Age,
    pub min_population: u32,
    pub research_points: f32,
    pub required_buildings: Vec<String>,
    pub required_resources: Vec<(crate::simulation::types::ResourceType, f32)>,
}

impl AgeRequirements {
    pub fn for_age(age: Age) -> Self {
        use crate::simulation::types::ResourceType;

        match age {
            Age::Stone => AgeRequirements {
                age,
                min_population: 0,
                research_points: 0.0,
                required_buildings: vec![],
                required_resources: vec![],
            },
            Age::Copper => AgeRequirements {
                age,
                min_population: 50,
                research_points: 500.0,
                required_buildings: vec![],
                required_resources: vec![(ResourceType::Copper, 20.0)],
            },
            Age::Bronze => AgeRequirements {
                age,
                min_population: 100,
                research_points: 1000.0,
                required_buildings: vec!["Forge".to_string()],
                required_resources: vec![
                    (ResourceType::Copper, 30.0),
                    (ResourceType::Tin, 15.0),
                ],
            },
            Age::Iron => AgeRequirements {
                age,
                min_population: 200,
                research_points: 2000.0,
                required_buildings: vec!["Forge".to_string()],
                required_resources: vec![(ResourceType::Iron, 50.0)],
            },
            Age::Classical => AgeRequirements {
                age,
                min_population: 350,
                research_points: 4000.0,
                required_buildings: vec![
                    "Forge".to_string(),
                    "Library".to_string(),
                ],
                required_resources: vec![(ResourceType::Iron, 100.0)],
            },
            Age::Medieval => AgeRequirements {
                age,
                min_population: 500,
                research_points: 8000.0,
                required_buildings: vec![
                    "Forge".to_string(),
                    "Library".to_string(),
                    "Castle".to_string(),
                ],
                required_resources: vec![
                    (ResourceType::Iron, 200.0),
                    (ResourceType::Coal, 50.0),
                ],
            },
            Age::Renaissance => AgeRequirements {
                age,
                min_population: 750,
                research_points: 16000.0,
                required_buildings: vec![
                    "Forge".to_string(),
                    "Library".to_string(),
                    "University".to_string(),
                ],
                required_resources: vec![
                    (ResourceType::Iron, 300.0),
                    (ResourceType::Coal, 100.0),
                    (ResourceType::Gold, 50.0),
                ],
            },
        }
    }
}
