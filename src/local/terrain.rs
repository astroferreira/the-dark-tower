//! Local terrain and feature types for detailed local maps.
//!
//! Defines the terrain types (ground tiles) and features (objects placed on terrain)
//! that make up local maps.

use serde::{Deserialize, Serialize};

/// Terrain types for local map cells (~25 types)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LocalTerrainType {
    // Ground types
    #[default]
    Grass,
    TallGrass,
    Dirt,
    Sand,
    Gravel,
    Stone,
    Snow,
    Ice,
    Mud,

    // Water types
    ShallowWater,
    DeepWater,
    Stream,
    Marsh,

    // Special ground
    ForestFloor,
    JungleFloor,
    VolcanicRock,
    CrystalGround,
    Ash,
    Salt,
    Lava,
    AcidPool,
    FrozenGround,
    Coral,
    Bone,
    Obsidian,
}

impl LocalTerrainType {
    /// Check if terrain is walkable by default
    pub fn is_walkable(&self) -> bool {
        match self {
            LocalTerrainType::DeepWater => false,
            LocalTerrainType::Lava => false,
            LocalTerrainType::AcidPool => false,
            _ => true,
        }
    }

    /// Get base movement cost (1.0 = normal, higher = slower)
    pub fn movement_cost(&self) -> f32 {
        match self {
            LocalTerrainType::Grass => 1.0,
            LocalTerrainType::TallGrass => 1.3,
            LocalTerrainType::Dirt => 1.0,
            LocalTerrainType::Sand => 1.4,
            LocalTerrainType::Gravel => 1.2,
            LocalTerrainType::Stone => 1.0,
            LocalTerrainType::Snow => 1.5,
            LocalTerrainType::Ice => 1.2,
            LocalTerrainType::Mud => 2.0,
            LocalTerrainType::ShallowWater => 2.0,
            LocalTerrainType::DeepWater => f32::INFINITY,
            LocalTerrainType::Stream => 1.8,
            LocalTerrainType::Marsh => 2.5,
            LocalTerrainType::ForestFloor => 1.2,
            LocalTerrainType::JungleFloor => 1.5,
            LocalTerrainType::VolcanicRock => 1.3,
            LocalTerrainType::CrystalGround => 1.2,
            LocalTerrainType::Ash => 1.4,
            LocalTerrainType::Salt => 1.1,
            LocalTerrainType::Lava => f32::INFINITY,
            LocalTerrainType::AcidPool => f32::INFINITY,
            LocalTerrainType::FrozenGround => 1.3,
            LocalTerrainType::Coral => 1.5,
            LocalTerrainType::Bone => 1.2,
            LocalTerrainType::Obsidian => 1.1,
        }
    }

    /// Get RGB color for rendering
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            LocalTerrainType::Grass => (90, 140, 60),
            LocalTerrainType::TallGrass => (70, 120, 50),
            LocalTerrainType::Dirt => (130, 100, 70),
            LocalTerrainType::Sand => (210, 190, 140),
            LocalTerrainType::Gravel => (140, 135, 125),
            LocalTerrainType::Stone => (120, 115, 110),
            LocalTerrainType::Snow => (245, 250, 255),
            LocalTerrainType::Ice => (180, 210, 230),
            LocalTerrainType::Mud => (80, 65, 45),
            LocalTerrainType::ShallowWater => (80, 130, 180),
            LocalTerrainType::DeepWater => (40, 80, 140),
            LocalTerrainType::Stream => (70, 120, 170),
            LocalTerrainType::Marsh => (70, 100, 60),
            LocalTerrainType::ForestFloor => (60, 80, 40),
            LocalTerrainType::JungleFloor => (40, 70, 35),
            LocalTerrainType::VolcanicRock => (50, 40, 40),
            LocalTerrainType::CrystalGround => (180, 200, 220),
            LocalTerrainType::Ash => (80, 80, 85),
            LocalTerrainType::Salt => (240, 235, 225),
            LocalTerrainType::Lava => (255, 100, 20),
            LocalTerrainType::AcidPool => (150, 180, 50),
            LocalTerrainType::FrozenGround => (200, 210, 220),
            LocalTerrainType::Coral => (255, 180, 160),
            LocalTerrainType::Bone => (230, 225, 210),
            LocalTerrainType::Obsidian => (30, 25, 35),
        }
    }

    /// Get ASCII character for terminal display
    pub fn ascii_char(&self) -> char {
        match self {
            LocalTerrainType::Grass => '.',
            LocalTerrainType::TallGrass => ',',
            LocalTerrainType::Dirt => '·',
            LocalTerrainType::Sand => '∴',
            LocalTerrainType::Gravel => '░',
            LocalTerrainType::Stone => '▓',
            LocalTerrainType::Snow => '❄',
            LocalTerrainType::Ice => '═',
            LocalTerrainType::Mud => '~',
            LocalTerrainType::ShallowWater => '≈',
            LocalTerrainType::DeepWater => '▒',
            LocalTerrainType::Stream => '~',
            LocalTerrainType::Marsh => '%',
            LocalTerrainType::ForestFloor => '.',
            LocalTerrainType::JungleFloor => '.',
            LocalTerrainType::VolcanicRock => '▒',
            LocalTerrainType::CrystalGround => '✧',
            LocalTerrainType::Ash => '░',
            LocalTerrainType::Salt => '░',
            LocalTerrainType::Lava => '▓',
            LocalTerrainType::AcidPool => '▒',
            LocalTerrainType::FrozenGround => '░',
            LocalTerrainType::Coral => '❀',
            LocalTerrainType::Bone => '░',
            LocalTerrainType::Obsidian => '▓',
        }
    }
}

/// Feature types that can be placed on terrain (~30 types)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LocalFeature {
    // Trees
    DeciduousTree,
    ConiferTree,
    PalmTree,
    DeadTree,
    JungleTree,
    WillowTree,
    BambooClump,

    // Vegetation
    Bush,
    FlowerPatch,
    Fern,
    Cactus,
    TallReeds,
    MushroomPatch,
    VineTangle,
    GlowingMoss,
    CrystalFlower,

    // Rocks
    Boulder,
    RockPile,
    CrystalCluster,
    Stalagmite,
    IceFormation,

    // Water features
    Pond,
    Spring,
    Geyser,

    // Structures
    StoneRuin,
    Shrine,
    CaveOpening,
    AncientMonolith,
    BoneRemains,
    Campfire,

    // Animal-related features
    AnimalDen,
    BirdNest,
    Beehive,
    AnimalTrail,
    WateringHole,
    BurrowEntrance,

    // Civilization features
    Signpost,
    WellStructure,
    FenceSection,
    Scarecrow,
    HayBale,
    Firepit,
    StorageShed,
    WatchTower,
    Bridge,
    Dock,

    // Buildings (colonist-built)
    Hut,
    WoodenHouse,
    StoneHouse,
    Farmland,
    MineEntrance,
    Workshop,
    Blacksmith,
    Granary,
    Barracks,
    TownHall,
    ConstructionSite,

    // Monster structures
    MonsterLair,
    MonsterNest,
    BoneHeap,

    // Natural details
    FallenLog,
    MossyRock,
    Termitemound,
    AntHill,
    Wildflowers,
    BerryBush,
    HerbPatch,
    Driftwood,
}

impl LocalFeature {
    /// Check if feature blocks movement
    pub fn blocks_movement(&self) -> bool {
        match self {
            LocalFeature::DeciduousTree => true,
            LocalFeature::ConiferTree => true,
            LocalFeature::PalmTree => true,
            LocalFeature::DeadTree => true,
            LocalFeature::JungleTree => true,
            LocalFeature::WillowTree => true,
            LocalFeature::BambooClump => true,
            LocalFeature::Boulder => true,
            LocalFeature::RockPile => true,
            LocalFeature::CrystalCluster => true,
            LocalFeature::Stalagmite => true,
            LocalFeature::IceFormation => true,
            LocalFeature::Pond => false, // Can walk through shallow pond
            LocalFeature::StoneRuin => false, // Passable ruins
            LocalFeature::AncientMonolith => true,
            LocalFeature::CaveOpening => false,
            LocalFeature::AnimalDen => false,
            LocalFeature::BirdNest => false,
            LocalFeature::Beehive => false,
            LocalFeature::AnimalTrail => false,
            LocalFeature::WateringHole => false,
            LocalFeature::BurrowEntrance => false,
            LocalFeature::Signpost => false,
            LocalFeature::WellStructure => true,
            LocalFeature::FenceSection => true,
            LocalFeature::Scarecrow => false,
            LocalFeature::HayBale => false,
            LocalFeature::Firepit => false,
            LocalFeature::StorageShed => true,
            LocalFeature::WatchTower => true,
            LocalFeature::Bridge => false,
            LocalFeature::Dock => false,
            LocalFeature::FallenLog => false,
            LocalFeature::MossyRock => false,
            LocalFeature::Termitemound => false,
            LocalFeature::AntHill => false,
            LocalFeature::Wildflowers => false,
            LocalFeature::BerryBush => false,
            LocalFeature::HerbPatch => false,
            LocalFeature::Driftwood => false,
            // Buildings
            LocalFeature::Hut => true,
            LocalFeature::WoodenHouse => true,
            LocalFeature::StoneHouse => true,
            LocalFeature::Farmland => false,
            LocalFeature::MineEntrance => false,
            LocalFeature::Workshop => true,
            LocalFeature::Blacksmith => true,
            LocalFeature::Granary => true,
            LocalFeature::Barracks => true,
            LocalFeature::TownHall => true,
            LocalFeature::ConstructionSite => false,
            // Monster structures
            LocalFeature::MonsterLair => false,
            LocalFeature::MonsterNest => false,
            LocalFeature::BoneHeap => false,
            _ => false,
        }
    }

    /// Get additional movement cost from feature
    pub fn movement_cost_modifier(&self) -> f32 {
        match self {
            LocalFeature::Bush => 0.5,
            LocalFeature::FlowerPatch => 0.0,
            LocalFeature::Fern => 0.2,
            LocalFeature::TallReeds => 0.5,
            LocalFeature::MushroomPatch => 0.2,
            LocalFeature::VineTangle => 0.8,
            LocalFeature::Pond => 1.0,
            LocalFeature::StoneRuin => 0.3,
            LocalFeature::AnimalTrail => -0.2, // Easier to walk on
            LocalFeature::WateringHole => 0.5,
            LocalFeature::FallenLog => 0.4,
            LocalFeature::HayBale => 0.3,
            LocalFeature::Bridge => -0.5, // Easier to cross
            LocalFeature::BerryBush => 0.3,
            LocalFeature::HerbPatch => 0.1,
            LocalFeature::Wildflowers => 0.0,
            LocalFeature::Driftwood => 0.3,
            _ => 0.0,
        }
    }

    /// Get RGB color for rendering
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            LocalFeature::DeciduousTree => (35, 90, 35),
            LocalFeature::ConiferTree => (25, 70, 40),
            LocalFeature::PalmTree => (45, 110, 45),
            LocalFeature::DeadTree => (80, 70, 60),
            LocalFeature::JungleTree => (20, 80, 30),
            LocalFeature::WillowTree => (50, 100, 50),
            LocalFeature::BambooClump => (100, 140, 60),
            LocalFeature::Bush => (50, 100, 40),
            LocalFeature::FlowerPatch => (200, 120, 160),
            LocalFeature::Fern => (60, 120, 50),
            LocalFeature::Cactus => (80, 130, 70),
            LocalFeature::TallReeds => (90, 120, 70),
            LocalFeature::MushroomPatch => (160, 100, 140),
            LocalFeature::VineTangle => (40, 90, 35),
            LocalFeature::GlowingMoss => (80, 200, 150),
            LocalFeature::CrystalFlower => (180, 200, 255),
            LocalFeature::Boulder => (100, 95, 90),
            LocalFeature::RockPile => (110, 105, 100),
            LocalFeature::CrystalCluster => (160, 180, 220),
            LocalFeature::Stalagmite => (90, 85, 80),
            LocalFeature::IceFormation => (200, 220, 240),
            LocalFeature::Pond => (60, 100, 150),
            LocalFeature::Spring => (100, 150, 200),
            LocalFeature::Geyser => (180, 190, 200),
            LocalFeature::StoneRuin => (130, 125, 115),
            LocalFeature::Shrine => (180, 170, 150),
            LocalFeature::CaveOpening => (30, 25, 20),
            LocalFeature::AncientMonolith => (70, 65, 75),
            LocalFeature::BoneRemains => (220, 215, 200),
            LocalFeature::Campfire => (200, 100, 50),
            // Animal features
            LocalFeature::AnimalDen => (100, 80, 60),
            LocalFeature::BirdNest => (140, 120, 90),
            LocalFeature::Beehive => (200, 180, 100),
            LocalFeature::AnimalTrail => (140, 120, 80),
            LocalFeature::WateringHole => (70, 110, 140),
            LocalFeature::BurrowEntrance => (90, 70, 50),
            // Civilization features
            LocalFeature::Signpost => (120, 90, 60),
            LocalFeature::WellStructure => (100, 100, 105),
            LocalFeature::FenceSection => (130, 100, 70),
            LocalFeature::Scarecrow => (150, 120, 80),
            LocalFeature::HayBale => (200, 180, 100),
            LocalFeature::Firepit => (80, 70, 60),
            LocalFeature::StorageShed => (110, 90, 70),
            LocalFeature::WatchTower => (100, 90, 80),
            LocalFeature::Bridge => (120, 100, 80),
            LocalFeature::Dock => (110, 90, 70),
            // Natural details
            LocalFeature::FallenLog => (80, 60, 45),
            LocalFeature::MossyRock => (80, 100, 70),
            LocalFeature::Termitemound => (130, 110, 80),
            LocalFeature::AntHill => (110, 90, 60),
            LocalFeature::Wildflowers => (200, 150, 180),
            LocalFeature::BerryBush => (70, 100, 50),
            LocalFeature::HerbPatch => (80, 130, 70),
            LocalFeature::Driftwood => (140, 120, 100),
            // Buildings (colonist-built)
            LocalFeature::Hut => (160, 130, 90),           // Light brown wood
            LocalFeature::WoodenHouse => (140, 100, 60),   // Darker wood
            LocalFeature::StoneHouse => (130, 130, 135),   // Gray stone
            LocalFeature::Farmland => (140, 120, 60),      // Tilled earth
            LocalFeature::MineEntrance => (60, 50, 45),    // Dark entrance
            LocalFeature::Workshop => (150, 120, 80),      // Wooden building
            LocalFeature::Blacksmith => (70, 60, 60),      // Dark with soot
            LocalFeature::Granary => (180, 160, 100),      // Light wood/thatch
            LocalFeature::Barracks => (100, 90, 80),       // Stone/wood
            LocalFeature::TownHall => (160, 150, 140),     // Large stone building
            LocalFeature::ConstructionSite => (170, 140, 100), // Scaffolding
            // Monster structures
            LocalFeature::MonsterLair => (50, 40, 35),     // Dark cave-like
            LocalFeature::MonsterNest => (80, 70, 50),     // Organic matter
            LocalFeature::BoneHeap => (200, 195, 180),     // Pale bones
        }
    }

    /// Get ASCII character for terminal display
    pub fn ascii_char(&self) -> char {
        match self {
            LocalFeature::DeciduousTree => '♣',
            LocalFeature::ConiferTree => '▲',
            LocalFeature::PalmTree => '♠',
            LocalFeature::DeadTree => '†',
            LocalFeature::JungleTree => '♣',
            LocalFeature::WillowTree => '♣',
            LocalFeature::BambooClump => '|',
            LocalFeature::Bush => '*',
            LocalFeature::FlowerPatch => '❀',
            LocalFeature::Fern => '∿',
            LocalFeature::Cactus => '¥',
            LocalFeature::TallReeds => '|',
            LocalFeature::MushroomPatch => '♠',
            LocalFeature::VineTangle => '~',
            LocalFeature::GlowingMoss => '○',
            LocalFeature::CrystalFlower => '✦',
            LocalFeature::Boulder => '●',
            LocalFeature::RockPile => '○',
            LocalFeature::CrystalCluster => '◆',
            LocalFeature::Stalagmite => '▲',
            LocalFeature::IceFormation => '◇',
            LocalFeature::Pond => '○',
            LocalFeature::Spring => '◎',
            LocalFeature::Geyser => '◉',
            LocalFeature::StoneRuin => '□',
            LocalFeature::Shrine => '⌂',
            LocalFeature::CaveOpening => '◯',
            LocalFeature::AncientMonolith => '▮',
            LocalFeature::BoneRemains => '☠',
            LocalFeature::Campfire => '♨',
            // Animal features
            LocalFeature::AnimalDen => '◎',
            LocalFeature::BirdNest => '○',
            LocalFeature::Beehive => '◇',
            LocalFeature::AnimalTrail => '·',
            LocalFeature::WateringHole => '○',
            LocalFeature::BurrowEntrance => '•',
            // Civilization features
            LocalFeature::Signpost => '†',
            LocalFeature::WellStructure => '◎',
            LocalFeature::FenceSection => '═',
            LocalFeature::Scarecrow => '†',
            LocalFeature::HayBale => '○',
            LocalFeature::Firepit => '◉',
            LocalFeature::StorageShed => '□',
            LocalFeature::WatchTower => '▲',
            LocalFeature::Bridge => '═',
            LocalFeature::Dock => '▬',
            // Natural details
            LocalFeature::FallenLog => '=',
            LocalFeature::MossyRock => '●',
            LocalFeature::Termitemound => '▲',
            LocalFeature::AntHill => '▴',
            LocalFeature::Wildflowers => '❀',
            LocalFeature::BerryBush => '✿',
            LocalFeature::HerbPatch => '♣',
            LocalFeature::Driftwood => '~',
            // Buildings (colonist-built)
            LocalFeature::Hut => '⌂',
            LocalFeature::WoodenHouse => '⌂',
            LocalFeature::StoneHouse => '◼',
            LocalFeature::Farmland => '≡',
            LocalFeature::MineEntrance => '▼',
            LocalFeature::Workshop => '⌂',
            LocalFeature::Blacksmith => '▣',
            LocalFeature::Granary => '◎',
            LocalFeature::Barracks => '▣',
            LocalFeature::TownHall => '▣',
            LocalFeature::ConstructionSite => '□',
            // Monster structures
            LocalFeature::MonsterLair => '◙',
            LocalFeature::MonsterNest => '◉',
            LocalFeature::BoneHeap => '☠',
        }
    }
}
