//! Technology unlocks: buildings, tools, and capabilities per age

use serde::{Deserialize, Serialize};
use super::ages::Age;

/// Types of buildings that can be constructed
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingType {
    // Stone Age
    Hut,
    Campfire,
    StoragePit,

    // Copper Age
    WoodenHouse,
    Granary,
    Well,
    Shrine,

    // Bronze Age
    Forge,
    Workshop,
    Wall,
    Barracks,
    Temple,

    // Iron Age
    Blacksmith,
    Market,
    Watchtower,
    Stable,

    // Classical Age
    Library,
    Aqueduct,
    Forum,
    Arena,
    Bathhouse,

    // Medieval Age
    Castle,
    Cathedral,
    Guildhall,
    Windmill,
    Hospital,

    // Renaissance Age
    University,
    Bank,
    TheatreHouse,
    Observatory,
    PrintingPress,
}

impl BuildingType {
    /// Get the name of the building
    pub fn name(&self) -> &'static str {
        match self {
            BuildingType::Hut => "Hut",
            BuildingType::Campfire => "Campfire",
            BuildingType::StoragePit => "Storage Pit",
            BuildingType::WoodenHouse => "Wooden House",
            BuildingType::Granary => "Granary",
            BuildingType::Well => "Well",
            BuildingType::Shrine => "Shrine",
            BuildingType::Forge => "Forge",
            BuildingType::Workshop => "Workshop",
            BuildingType::Wall => "Wall",
            BuildingType::Barracks => "Barracks",
            BuildingType::Temple => "Temple",
            BuildingType::Blacksmith => "Blacksmith",
            BuildingType::Market => "Market",
            BuildingType::Watchtower => "Watchtower",
            BuildingType::Stable => "Stable",
            BuildingType::Library => "Library",
            BuildingType::Aqueduct => "Aqueduct",
            BuildingType::Forum => "Forum",
            BuildingType::Arena => "Arena",
            BuildingType::Bathhouse => "Bathhouse",
            BuildingType::Castle => "Castle",
            BuildingType::Cathedral => "Cathedral",
            BuildingType::Guildhall => "Guildhall",
            BuildingType::Windmill => "Windmill",
            BuildingType::Hospital => "Hospital",
            BuildingType::University => "University",
            BuildingType::Bank => "Bank",
            BuildingType::TheatreHouse => "Theatre House",
            BuildingType::Observatory => "Observatory",
            BuildingType::PrintingPress => "Printing Press",
        }
    }

    /// Shelter capacity provided by this building
    pub fn shelter_capacity(&self) -> u32 {
        match self {
            BuildingType::Hut => 5,
            BuildingType::Campfire => 0,
            BuildingType::StoragePit => 0,
            BuildingType::WoodenHouse => 10,
            BuildingType::Granary => 0,
            BuildingType::Well => 0,
            BuildingType::Shrine => 0,
            BuildingType::Forge => 0,
            BuildingType::Workshop => 0,
            BuildingType::Wall => 0,
            BuildingType::Barracks => 20,
            BuildingType::Temple => 0,
            BuildingType::Blacksmith => 0,
            BuildingType::Market => 0,
            BuildingType::Watchtower => 2,
            BuildingType::Stable => 0,
            BuildingType::Library => 0,
            BuildingType::Aqueduct => 0,
            BuildingType::Forum => 0,
            BuildingType::Arena => 0,
            BuildingType::Bathhouse => 0,
            BuildingType::Castle => 50,
            BuildingType::Cathedral => 0,
            BuildingType::Guildhall => 0,
            BuildingType::Windmill => 0,
            BuildingType::Hospital => 10,
            BuildingType::University => 0,
            BuildingType::Bank => 0,
            BuildingType::TheatreHouse => 0,
            BuildingType::Observatory => 0,
            BuildingType::PrintingPress => 0,
        }
    }

    /// Storage capacity bonus
    pub fn storage_bonus(&self) -> f32 {
        match self {
            BuildingType::StoragePit => 50.0,
            BuildingType::Granary => 200.0,
            BuildingType::Well => 100.0, // Water storage
            BuildingType::Market => 100.0,
            BuildingType::Bank => 500.0,
            _ => 0.0,
        }
    }

    /// Defense bonus (multiplier)
    pub fn defense_bonus(&self) -> f32 {
        match self {
            BuildingType::Wall => 1.3,
            BuildingType::Watchtower => 1.1,
            BuildingType::Castle => 1.8,
            BuildingType::Barracks => 1.1,
            _ => 1.0,
        }
    }

    /// Research bonus (points per tick)
    pub fn research_bonus(&self) -> f32 {
        match self {
            BuildingType::Shrine => 0.5,
            BuildingType::Temple => 1.0,
            BuildingType::Library => 3.0,
            BuildingType::University => 5.0,
            BuildingType::Observatory => 2.0,
            BuildingType::PrintingPress => 3.0,
            _ => 0.0,
        }
    }

    /// Health bonus (healing/disease prevention)
    pub fn health_bonus(&self) -> f32 {
        match self {
            BuildingType::Well => 0.1,
            BuildingType::Aqueduct => 0.2,
            BuildingType::Bathhouse => 0.15,
            BuildingType::Hospital => 0.3,
            _ => 0.0,
        }
    }

    /// Morale bonus
    pub fn morale_bonus(&self) -> f32 {
        match self {
            BuildingType::Campfire => 0.05,
            BuildingType::Shrine => 0.05,
            BuildingType::Temple => 0.1,
            BuildingType::Arena => 0.15,
            BuildingType::Bathhouse => 0.05,
            BuildingType::Cathedral => 0.2,
            BuildingType::TheatreHouse => 0.15,
            _ => 0.0,
        }
    }

    /// Production bonus (multiplier for resource extraction)
    pub fn production_bonus(&self) -> f32 {
        match self {
            BuildingType::Forge => 1.1,
            BuildingType::Workshop => 1.15,
            BuildingType::Blacksmith => 1.2,
            BuildingType::Guildhall => 1.15,
            BuildingType::Windmill => 1.25,
            _ => 1.0,
        }
    }
}

/// Tech unlock information
pub struct TechUnlock;

impl TechUnlock {
    /// Get buildings unlocked at a specific age
    pub fn buildings_for_age(age: Age) -> Vec<BuildingType> {
        match age {
            Age::Stone => vec![
                BuildingType::Hut,
                BuildingType::Campfire,
                BuildingType::StoragePit,
            ],
            Age::Copper => vec![
                BuildingType::WoodenHouse,
                BuildingType::Granary,
                BuildingType::Well,
                BuildingType::Shrine,
            ],
            Age::Bronze => vec![
                BuildingType::Forge,
                BuildingType::Workshop,
                BuildingType::Wall,
                BuildingType::Barracks,
                BuildingType::Temple,
            ],
            Age::Iron => vec![
                BuildingType::Blacksmith,
                BuildingType::Market,
                BuildingType::Watchtower,
                BuildingType::Stable,
            ],
            Age::Classical => vec![
                BuildingType::Library,
                BuildingType::Aqueduct,
                BuildingType::Forum,
                BuildingType::Arena,
                BuildingType::Bathhouse,
            ],
            Age::Medieval => vec![
                BuildingType::Castle,
                BuildingType::Cathedral,
                BuildingType::Guildhall,
                BuildingType::Windmill,
                BuildingType::Hospital,
            ],
            Age::Renaissance => vec![
                BuildingType::University,
                BuildingType::Bank,
                BuildingType::TheatreHouse,
                BuildingType::Observatory,
                BuildingType::PrintingPress,
            ],
        }
    }

    /// Get resources that can be extracted at a specific age
    pub fn extractable_resources(age: Age) -> Vec<crate::simulation::types::ResourceType> {
        use crate::simulation::types::ResourceType;

        let mut resources = vec![
            ResourceType::Food,
            ResourceType::Water,
            ResourceType::Wood,
            ResourceType::Stone,
        ];

        if age >= Age::Copper {
            resources.push(ResourceType::Copper);
            resources.push(ResourceType::Clay);
            resources.push(ResourceType::Leather);
        }

        if age >= Age::Bronze {
            resources.push(ResourceType::Tin);
            resources.push(ResourceType::Bronze);
            resources.push(ResourceType::Cloth);
        }

        if age >= Age::Iron {
            resources.push(ResourceType::Iron);
            resources.push(ResourceType::Coal);
            resources.push(ResourceType::Salt);
            resources.push(ResourceType::Tools);
        }

        if age >= Age::Classical {
            resources.push(ResourceType::Gold);
            resources.push(ResourceType::Silver);
            resources.push(ResourceType::Spices);
            resources.push(ResourceType::Weapons);
        }

        if age >= Age::Medieval {
            resources.push(ResourceType::Gems);
            resources.push(ResourceType::Obsidian);
        }

        resources
    }
}
