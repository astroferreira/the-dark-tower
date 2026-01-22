//! Extended biome system for procedural world-building
//!
//! Provides fantasy/alien biomes with configurable rarity and placement rules.

use std::collections::HashMap;
use noise::{NoiseFn, Perlin, Seedable};
use crate::climate::Biome;
use crate::tilemap::Tilemap;

/// Extended biome enum with fantasy variants
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExtendedBiome {
    // Base biomes (from climate.rs)
    DeepOcean,
    Ocean,
    CoastalWater,
    Ice,
    Tundra,
    BorealForest,
    TemperateGrassland,
    TemperateForest,
    TemperateRainforest,
    Desert,
    Savanna,
    TropicalForest,
    TropicalRainforest,
    AlpineTundra,
    SnowyPeaks,
    Foothills,      // Rolling hills at mountain bases (transitional terrain)
    Lagoon,         // Shallow protected waters behind barrier islands

    // Fantasy forests
    DeadForest,
    CrystalForest,
    BioluminescentForest,
    MushroomForest,
    PetrifiedForest,

    // Fantasy waters
    AcidLake,
    LavaLake,
    FrozenLake,
    BioluminescentWater,

    // Wastelands
    VolcanicWasteland,
    SaltFlats,
    Ashlands,
    CrystalWasteland,

    // Wetlands
    Swamp,
    Marsh,
    Bog,
    MangroveSaltmarsh,

    // Ultra-rare biomes - Ancient/Primeval
    AncientGrove,
    TitanBones,
    CoralPlateau,

    // Ultra-rare biomes - Geothermal/Volcanic
    ObsidianFields,
    Geysers,
    TarPits,

    // Ultra-rare biomes - Magical/Anomalous
    FloatingStones,
    Shadowfen,
    PrismaticPools,
    AuroraWastes,

    // Ultra-rare biomes - Desert variants
    SingingDunes,
    Oasis,
    GlassDesert,

    // Ultra-rare biomes - Aquatic
    AbyssalVents,
    Sargasso,

    // ============ NEW BIOMES ============

    // Mystical / Supernatural
    EtherealMist,
    StarfallCrater,
    LeyNexus,
    WhisperingStones,
    SpiritMarsh,

    // Extreme Geological
    SulfurVents,
    BasaltColumns,
    PaintedHills,
    RazorPeaks,
    SinkholeLakes,

    // Biological Wonders
    ColossalHive,
    BoneFields,
    CarnivorousBog,
    FungalBloom,
    KelpTowers,

    // Exotic Waters
    BrinePools,
    HotSprings,
    MirrorLake,
    InkSea,
    PhosphorShallows,

    // Alien / Corrupted
    VoidScar,
    SiliconGrove,
    SporeWastes,
    BleedingStone,
    HollowEarth,

    // Ancient Ruins
    SunkenCity,
    CyclopeanRuins,
    BuriedTemple,
    OvergrownCitadel,
    DarkTower,

    // ============ OCEAN BIOMES ============

    // Realistic Ocean - Shallow/Coastal
    CoralReef,
    KelpForest,
    SeagrassMeadow,

    // Realistic Ocean - Mid-depth
    ContinentalShelf,
    Seamount,

    // Realistic Ocean - Deep
    OceanicTrench,
    AbyssalPlain,
    MidOceanRidge,
    ColdSeep,
    BrinePool,

    // Fantasy Ocean
    CrystalDepths,
    LeviathanGraveyard,
    DrownedCitadel,
    VoidMaw,
    PearlGardens,
    SirenShallows,
    FrozenAbyss,
    ThermalVents,

    // ============ KARST & CAVE BIOMES ============
    KarstPlains,      // Limestone terrain with surface dissolution features
    TowerKarst,       // Dramatic limestone pillars (tropical karst like Guilin)
    Sinkhole,         // Collapsed cave/doline formations
    Cenote,           // Water-filled sinkholes (tropical)
    CaveEntrance,     // Surface openings to underground cave systems
    CockpitKarst,     // Star-shaped depressions between mogotes

    // ============ VOLCANIC BIOMES ============
    Caldera,          // Large volcanic crater from collapsed magma chamber
    ShieldVolcano,    // Broad, gently sloping volcanic terrain (like Hawaii)
    VolcanicCone,     // Classic stratovolcano peak
    LavaField,        // Solidified lava flows (basalt)
    FumaroleField,    // Steam vents and sulfurous terrain
    VolcanicBeach,    // Black sand beaches near volcanoes
    HotSpot,          // Active volcanic hot spot area
}

impl ExtendedBiome {
    /// Convert from base Biome to ExtendedBiome
    pub fn from_base(biome: Biome) -> Self {
        match biome {
            Biome::DeepOcean => ExtendedBiome::DeepOcean,
            Biome::Ocean => ExtendedBiome::Ocean,
            Biome::CoastalWater => ExtendedBiome::CoastalWater,
            Biome::Ice => ExtendedBiome::Ice,
            Biome::Tundra => ExtendedBiome::Tundra,
            Biome::BorealForest => ExtendedBiome::BorealForest,
            Biome::TemperateGrassland => ExtendedBiome::TemperateGrassland,
            Biome::TemperateForest => ExtendedBiome::TemperateForest,
            Biome::TemperateRainforest => ExtendedBiome::TemperateRainforest,
            Biome::Desert => ExtendedBiome::Desert,
            Biome::Savanna => ExtendedBiome::Savanna,
            Biome::TropicalForest => ExtendedBiome::TropicalForest,
            Biome::TropicalRainforest => ExtendedBiome::TropicalRainforest,
            Biome::AlpineTundra => ExtendedBiome::AlpineTundra,
            Biome::SnowyPeaks => ExtendedBiome::SnowyPeaks,
        }
    }

    /// Get color for rendering
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            // Base biomes (same as climate.rs)
            ExtendedBiome::DeepOcean => (20, 40, 80),
            ExtendedBiome::Ocean => (30, 60, 120),
            ExtendedBiome::CoastalWater => (60, 100, 160),
            ExtendedBiome::Ice => (240, 250, 255),
            ExtendedBiome::Tundra => (180, 190, 170),
            ExtendedBiome::BorealForest => (50, 80, 50),
            ExtendedBiome::TemperateGrassland => (140, 170, 80),
            ExtendedBiome::TemperateForest => (40, 100, 40),
            ExtendedBiome::TemperateRainforest => (30, 80, 50),
            ExtendedBiome::Desert => (210, 180, 120),
            ExtendedBiome::Savanna => (170, 160, 80),
            ExtendedBiome::TropicalForest => (30, 120, 30),
            ExtendedBiome::TropicalRainforest => (20, 90, 40),
            ExtendedBiome::AlpineTundra => (140, 140, 130),
            ExtendedBiome::SnowyPeaks => (255, 255, 255),
            ExtendedBiome::Foothills => (120, 145, 85),     // Darker olive green (rolling hills)
            ExtendedBiome::Lagoon => (90, 175, 195),        // Light turquoise (protected shallow water)

            // Fantasy forests
            ExtendedBiome::DeadForest => (80, 70, 60),
            ExtendedBiome::CrystalForest => (180, 220, 255),
            ExtendedBiome::BioluminescentForest => (40, 200, 150),
            ExtendedBiome::MushroomForest => (140, 80, 160),
            ExtendedBiome::PetrifiedForest => (100, 95, 90),

            // Fantasy waters
            ExtendedBiome::AcidLake => (150, 180, 50),
            ExtendedBiome::LavaLake => (255, 100, 20),
            ExtendedBiome::FrozenLake => (200, 230, 250),
            ExtendedBiome::BioluminescentWater => (50, 180, 200),

            // Wastelands
            ExtendedBiome::VolcanicWasteland => (50, 30, 30),
            ExtendedBiome::SaltFlats => (240, 235, 220),
            ExtendedBiome::Ashlands => (80, 80, 85),
            ExtendedBiome::CrystalWasteland => (200, 180, 220),

            // Wetlands
            ExtendedBiome::Swamp => (50, 80, 50),
            ExtendedBiome::Marsh => (80, 120, 70),
            ExtendedBiome::Bog => (90, 70, 50),
            ExtendedBiome::MangroveSaltmarsh => (60, 100, 80),

            // Ultra-rare - Ancient/Primeval
            ExtendedBiome::AncientGrove => (20, 60, 30),
            ExtendedBiome::TitanBones => (200, 195, 180),
            ExtendedBiome::CoralPlateau => (255, 180, 160),

            // Ultra-rare - Geothermal/Volcanic
            ExtendedBiome::ObsidianFields => (30, 25, 35),
            ExtendedBiome::Geysers => (180, 200, 220),
            ExtendedBiome::TarPits => (20, 15, 10),

            // Ultra-rare - Magical/Anomalous
            ExtendedBiome::FloatingStones => (160, 140, 180),
            ExtendedBiome::Shadowfen => (30, 40, 35),
            ExtendedBiome::PrismaticPools => (255, 150, 200),
            ExtendedBiome::AuroraWastes => (100, 200, 180),

            // Ultra-rare - Desert variants
            ExtendedBiome::SingingDunes => (230, 200, 140),
            ExtendedBiome::Oasis => (50, 180, 80),
            ExtendedBiome::GlassDesert => (200, 220, 230),

            // Ultra-rare - Aquatic
            ExtendedBiome::AbyssalVents => (80, 20, 30),
            ExtendedBiome::Sargasso => (60, 100, 50),

            // ============ NEW BIOMES ============

            // Mystical / Supernatural
            ExtendedBiome::EtherealMist => (180, 190, 210),       // Pale blue-gray mist
            ExtendedBiome::StarfallCrater => (90, 60, 120),       // Deep purple meteor
            ExtendedBiome::LeyNexus => (200, 180, 255),           // Bright magical purple
            ExtendedBiome::WhisperingStones => (140, 135, 125),   // Ancient gray stone
            ExtendedBiome::SpiritMarsh => (120, 150, 140),        // Ghostly green-gray

            // Extreme Geological
            ExtendedBiome::SulfurVents => (220, 200, 60),         // Bright yellow sulfur
            ExtendedBiome::BasaltColumns => (50, 50, 55),         // Dark basalt gray
            ExtendedBiome::PaintedHills => (200, 140, 100),       // Orange-red banded
            ExtendedBiome::RazorPeaks => (100, 95, 105),          // Sharp gray-purple
            ExtendedBiome::SinkholeLakes => (40, 80, 100),        // Deep blue sinkhole

            // Biological Wonders
            ExtendedBiome::ColossalHive => (180, 140, 80),        // Amber/honey color
            ExtendedBiome::BoneFields => (230, 225, 210),         // Pale bone white
            ExtendedBiome::CarnivorousBog => (100, 60, 70),       // Red-tinged dark
            ExtendedBiome::FungalBloom => (200, 100, 180),        // Bright pink-purple
            ExtendedBiome::KelpTowers => (40, 90, 60),            // Deep kelp green

            // Exotic Waters
            ExtendedBiome::BrinePools => (60, 80, 90),            // Dark salty blue
            ExtendedBiome::HotSprings => (100, 180, 190),         // Turquoise thermal
            ExtendedBiome::MirrorLake => (150, 180, 200),         // Reflective silver-blue
            ExtendedBiome::InkSea => (15, 15, 25),                // Near-black deep
            ExtendedBiome::PhosphorShallows => (80, 200, 180),    // Glowing cyan

            // Alien / Corrupted
            ExtendedBiome::VoidScar => (40, 0, 50),               // Deep void purple
            ExtendedBiome::SiliconGrove => (180, 200, 220),       // Crystalline blue-white
            ExtendedBiome::SporeWastes => (160, 140, 100),        // Sickly yellow-brown
            ExtendedBiome::BleedingStone => (150, 60, 50),        // Red iron-stained
            ExtendedBiome::HollowEarth => (60, 50, 45),           // Dark cavern brown

            // Ancient Ruins
            ExtendedBiome::SunkenCity => (70, 90, 110),           // Underwater stone
            ExtendedBiome::CyclopeanRuins => (110, 105, 95),      // Ancient weathered stone
            ExtendedBiome::BuriedTemple => (170, 150, 120),       // Sand-covered stone
            ExtendedBiome::OvergrownCitadel => (60, 90, 50),      // Vine-covered green
            ExtendedBiome::DarkTower => (25, 20, 30),              // Ominous dark obsidian

            // Ocean Biomes - Realistic Shallow
            ExtendedBiome::CoralReef => (255, 180, 150),          // Coral pink-orange
            ExtendedBiome::KelpForest => (35, 80, 45),            // Deep kelp green
            ExtendedBiome::SeagrassMeadow => (50, 120, 70),       // Seagrass green

            // Ocean Biomes - Realistic Mid-depth
            ExtendedBiome::ContinentalShelf => (45, 70, 110),     // Sandy blue
            ExtendedBiome::Seamount => (60, 50, 80),              // Dark volcanic purple

            // Ocean Biomes - Realistic Deep
            ExtendedBiome::OceanicTrench => (10, 15, 35),         // Ultra-deep blue-black
            ExtendedBiome::AbyssalPlain => (25, 35, 55),          // Deep gray-blue
            ExtendedBiome::MidOceanRidge => (70, 40, 50),         // Volcanic red-brown
            ExtendedBiome::ColdSeep => (40, 50, 45),              // Murky green-gray
            ExtendedBiome::BrinePool => (35, 45, 60),             // Dense blue-gray

            // Ocean Biomes - Fantasy
            ExtendedBiome::CrystalDepths => (120, 180, 220),      // Crystal blue
            ExtendedBiome::LeviathanGraveyard => (180, 175, 160), // Bone white-gray
            ExtendedBiome::DrownedCitadel => (80, 90, 100),       // Stone gray-blue
            ExtendedBiome::VoidMaw => (5, 0, 15),                 // Near-black purple
            ExtendedBiome::PearlGardens => (200, 210, 230),       // Pearl white-blue
            ExtendedBiome::SirenShallows => (100, 180, 200),      // Enchanted turquoise
            ExtendedBiome::FrozenAbyss => (150, 180, 200),        // Ice blue
            ExtendedBiome::ThermalVents => (200, 80, 40),         // Magma orange-red

            // Karst & Cave biomes
            ExtendedBiome::KarstPlains => (195, 190, 175),        // Pale limestone gray
            ExtendedBiome::TowerKarst => (175, 180, 160),         // Gray-green limestone pillars
            ExtendedBiome::Sinkhole => (85, 75, 65),              // Dark depression
            ExtendedBiome::Cenote => (40, 120, 140),              // Deep turquoise water
            ExtendedBiome::CaveEntrance => (45, 40, 35),          // Dark cave mouth
            ExtendedBiome::CockpitKarst => (165, 175, 145),       // Green-gray mogotes

            // Volcanic biomes
            ExtendedBiome::Caldera => (70, 55, 50),               // Dark volcanic brown
            ExtendedBiome::ShieldVolcano => (60, 55, 45),         // Dark basalt
            ExtendedBiome::VolcanicCone => (90, 70, 60),          // Volcanic gray-brown
            ExtendedBiome::LavaField => (35, 25, 25),             // Near-black basalt
            ExtendedBiome::FumaroleField => (200, 190, 120),      // Sulfur yellow
            ExtendedBiome::VolcanicBeach => (40, 40, 45),         // Black sand
            ExtendedBiome::HotSpot => (180, 80, 50),              // Warm orange-red
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ExtendedBiome::DeepOcean => "Deep Ocean",
            ExtendedBiome::Ocean => "Ocean",
            ExtendedBiome::CoastalWater => "Coastal Water",
            ExtendedBiome::Ice => "Ice",
            ExtendedBiome::Tundra => "Tundra",
            ExtendedBiome::BorealForest => "Boreal Forest",
            ExtendedBiome::TemperateGrassland => "Temperate Grassland",
            ExtendedBiome::TemperateForest => "Temperate Forest",
            ExtendedBiome::TemperateRainforest => "Temperate Rainforest",
            ExtendedBiome::Desert => "Desert",
            ExtendedBiome::Savanna => "Savanna",
            ExtendedBiome::TropicalForest => "Tropical Forest",
            ExtendedBiome::TropicalRainforest => "Tropical Rainforest",
            ExtendedBiome::AlpineTundra => "Alpine Tundra",
            ExtendedBiome::SnowyPeaks => "Snowy Peaks",
            ExtendedBiome::Foothills => "Foothills",
            ExtendedBiome::Lagoon => "Lagoon",
            ExtendedBiome::DeadForest => "Dead Forest",
            ExtendedBiome::CrystalForest => "Crystal Forest",
            ExtendedBiome::BioluminescentForest => "Bioluminescent Forest",
            ExtendedBiome::MushroomForest => "Mushroom Forest",
            ExtendedBiome::PetrifiedForest => "Petrified Forest",
            ExtendedBiome::AcidLake => "Acid Lake",
            ExtendedBiome::LavaLake => "Lava Lake",
            ExtendedBiome::FrozenLake => "Frozen Lake",
            ExtendedBiome::BioluminescentWater => "Bioluminescent Water",
            ExtendedBiome::VolcanicWasteland => "Volcanic Wasteland",
            ExtendedBiome::SaltFlats => "Salt Flats",
            ExtendedBiome::Ashlands => "Ashlands",
            ExtendedBiome::CrystalWasteland => "Crystal Wasteland",
            ExtendedBiome::Swamp => "Swamp",
            ExtendedBiome::Marsh => "Marsh",
            ExtendedBiome::Bog => "Bog",
            ExtendedBiome::MangroveSaltmarsh => "Mangrove Saltmarsh",

            // Ultra-rare
            ExtendedBiome::AncientGrove => "Ancient Grove",
            ExtendedBiome::TitanBones => "Titan Bones",
            ExtendedBiome::CoralPlateau => "Coral Plateau",
            ExtendedBiome::ObsidianFields => "Obsidian Fields",
            ExtendedBiome::Geysers => "Geysers",
            ExtendedBiome::TarPits => "Tar Pits",
            ExtendedBiome::FloatingStones => "Floating Stones",
            ExtendedBiome::Shadowfen => "Shadowfen",
            ExtendedBiome::PrismaticPools => "Prismatic Pools",
            ExtendedBiome::AuroraWastes => "Aurora Wastes",
            ExtendedBiome::SingingDunes => "Singing Dunes",
            ExtendedBiome::Oasis => "Oasis",
            ExtendedBiome::GlassDesert => "Glass Desert",
            ExtendedBiome::AbyssalVents => "Abyssal Vents",
            ExtendedBiome::Sargasso => "Sargasso",

            // New biomes
            ExtendedBiome::EtherealMist => "Ethereal Mist",
            ExtendedBiome::StarfallCrater => "Starfall Crater",
            ExtendedBiome::LeyNexus => "Ley Nexus",
            ExtendedBiome::WhisperingStones => "Whispering Stones",
            ExtendedBiome::SpiritMarsh => "Spirit Marsh",
            ExtendedBiome::SulfurVents => "Sulfur Vents",
            ExtendedBiome::BasaltColumns => "Basalt Columns",
            ExtendedBiome::PaintedHills => "Painted Hills",
            ExtendedBiome::RazorPeaks => "Razor Peaks",
            ExtendedBiome::SinkholeLakes => "Sinkhole Lakes",
            ExtendedBiome::ColossalHive => "Colossal Hive",
            ExtendedBiome::BoneFields => "Bone Fields",
            ExtendedBiome::CarnivorousBog => "Carnivorous Bog",
            ExtendedBiome::FungalBloom => "Fungal Bloom",
            ExtendedBiome::KelpTowers => "Kelp Towers",
            ExtendedBiome::BrinePools => "Brine Pools",
            ExtendedBiome::HotSprings => "Hot Springs",
            ExtendedBiome::MirrorLake => "Mirror Lake",
            ExtendedBiome::InkSea => "Ink Sea",
            ExtendedBiome::PhosphorShallows => "Phosphor Shallows",
            ExtendedBiome::VoidScar => "Void Scar",
            ExtendedBiome::SiliconGrove => "Silicon Grove",
            ExtendedBiome::SporeWastes => "Spore Wastes",
            ExtendedBiome::BleedingStone => "Bleeding Stone",
            ExtendedBiome::HollowEarth => "Hollow Earth",
            ExtendedBiome::SunkenCity => "Sunken City",
            ExtendedBiome::CyclopeanRuins => "Cyclopean Ruins",
            ExtendedBiome::BuriedTemple => "Buried Temple",
            ExtendedBiome::OvergrownCitadel => "Overgrown Citadel",
            ExtendedBiome::DarkTower => "Dark Tower",

            // Ocean Biomes - Realistic
            ExtendedBiome::CoralReef => "Coral Reef",
            ExtendedBiome::KelpForest => "Kelp Forest",
            ExtendedBiome::SeagrassMeadow => "Seagrass Meadow",
            ExtendedBiome::ContinentalShelf => "Continental Shelf",
            ExtendedBiome::Seamount => "Seamount",
            ExtendedBiome::OceanicTrench => "Oceanic Trench",
            ExtendedBiome::AbyssalPlain => "Abyssal Plain",
            ExtendedBiome::MidOceanRidge => "Mid-Ocean Ridge",
            ExtendedBiome::ColdSeep => "Cold Seep",
            ExtendedBiome::BrinePool => "Brine Pool",

            // Ocean Biomes - Fantasy
            ExtendedBiome::CrystalDepths => "Crystal Depths",
            ExtendedBiome::LeviathanGraveyard => "Leviathan Graveyard",
            ExtendedBiome::DrownedCitadel => "Drowned Citadel",
            ExtendedBiome::VoidMaw => "Void Maw",
            ExtendedBiome::PearlGardens => "Pearl Gardens",
            ExtendedBiome::SirenShallows => "Siren Shallows",
            ExtendedBiome::FrozenAbyss => "Frozen Abyss",
            ExtendedBiome::ThermalVents => "Thermal Vents",

            // Karst & Cave biomes
            ExtendedBiome::KarstPlains => "Karst Plains",
            ExtendedBiome::TowerKarst => "Tower Karst",
            ExtendedBiome::Sinkhole => "Sinkhole",
            ExtendedBiome::Cenote => "Cenote",
            ExtendedBiome::CaveEntrance => "Cave Entrance",
            ExtendedBiome::CockpitKarst => "Cockpit Karst",

            // Volcanic biomes
            ExtendedBiome::Caldera => "Caldera",
            ExtendedBiome::ShieldVolcano => "Shield Volcano",
            ExtendedBiome::VolcanicCone => "Volcanic Cone",
            ExtendedBiome::LavaField => "Lava Field",
            ExtendedBiome::FumaroleField => "Fumarole Field",
            ExtendedBiome::VolcanicBeach => "Volcanic Beach",
            ExtendedBiome::HotSpot => "Hot Spot",
        }
    }

    /// Check if this is a fantasy biome
    pub fn is_fantasy(&self) -> bool {
        matches!(self,
            ExtendedBiome::DeadForest |
            ExtendedBiome::CrystalForest |
            ExtendedBiome::BioluminescentForest |
            ExtendedBiome::MushroomForest |
            ExtendedBiome::PetrifiedForest |
            ExtendedBiome::AcidLake |
            ExtendedBiome::LavaLake |
            ExtendedBiome::FrozenLake |
            ExtendedBiome::BioluminescentWater |
            ExtendedBiome::VolcanicWasteland |
            ExtendedBiome::SaltFlats |
            ExtendedBiome::Ashlands |
            ExtendedBiome::CrystalWasteland |
            ExtendedBiome::Swamp |
            ExtendedBiome::Marsh |
            ExtendedBiome::Bog |
            ExtendedBiome::MangroveSaltmarsh |
            // Ultra-rare (original 15)
            ExtendedBiome::AncientGrove |
            ExtendedBiome::TitanBones |
            ExtendedBiome::CoralPlateau |
            ExtendedBiome::ObsidianFields |
            ExtendedBiome::Geysers |
            ExtendedBiome::TarPits |
            ExtendedBiome::FloatingStones |
            ExtendedBiome::Shadowfen |
            ExtendedBiome::PrismaticPools |
            ExtendedBiome::AuroraWastes |
            ExtendedBiome::SingingDunes |
            ExtendedBiome::Oasis |
            ExtendedBiome::GlassDesert |
            ExtendedBiome::AbyssalVents |
            ExtendedBiome::Sargasso |
            // New biomes (29)
            ExtendedBiome::EtherealMist |
            ExtendedBiome::StarfallCrater |
            ExtendedBiome::LeyNexus |
            ExtendedBiome::WhisperingStones |
            ExtendedBiome::SpiritMarsh |
            ExtendedBiome::SulfurVents |
            ExtendedBiome::BasaltColumns |
            ExtendedBiome::PaintedHills |
            ExtendedBiome::RazorPeaks |
            ExtendedBiome::SinkholeLakes |
            ExtendedBiome::ColossalHive |
            ExtendedBiome::BoneFields |
            ExtendedBiome::CarnivorousBog |
            ExtendedBiome::FungalBloom |
            ExtendedBiome::KelpTowers |
            ExtendedBiome::BrinePools |
            ExtendedBiome::HotSprings |
            ExtendedBiome::MirrorLake |
            ExtendedBiome::InkSea |
            ExtendedBiome::PhosphorShallows |
            ExtendedBiome::VoidScar |
            ExtendedBiome::SiliconGrove |
            ExtendedBiome::SporeWastes |
            ExtendedBiome::BleedingStone |
            ExtendedBiome::HollowEarth |
            ExtendedBiome::SunkenCity |
            ExtendedBiome::CyclopeanRuins |
            ExtendedBiome::BuriedTemple |
            ExtendedBiome::OvergrownCitadel
        )
    }

    /// Check if this is an ultra-rare biome
    pub fn is_ultra_rare(&self) -> bool {
        matches!(self,
            // Original 15
            ExtendedBiome::AncientGrove |
            ExtendedBiome::TitanBones |
            ExtendedBiome::CoralPlateau |
            ExtendedBiome::ObsidianFields |
            ExtendedBiome::Geysers |
            ExtendedBiome::TarPits |
            ExtendedBiome::FloatingStones |
            ExtendedBiome::Shadowfen |
            ExtendedBiome::PrismaticPools |
            ExtendedBiome::AuroraWastes |
            ExtendedBiome::SingingDunes |
            ExtendedBiome::Oasis |
            ExtendedBiome::GlassDesert |
            ExtendedBiome::AbyssalVents |
            ExtendedBiome::Sargasso |
            // New 29
            ExtendedBiome::EtherealMist |
            ExtendedBiome::StarfallCrater |
            ExtendedBiome::LeyNexus |
            ExtendedBiome::WhisperingStones |
            ExtendedBiome::SpiritMarsh |
            ExtendedBiome::SulfurVents |
            ExtendedBiome::BasaltColumns |
            ExtendedBiome::PaintedHills |
            ExtendedBiome::RazorPeaks |
            ExtendedBiome::SinkholeLakes |
            ExtendedBiome::ColossalHive |
            ExtendedBiome::BoneFields |
            ExtendedBiome::CarnivorousBog |
            ExtendedBiome::FungalBloom |
            ExtendedBiome::KelpTowers |
            ExtendedBiome::BrinePools |
            ExtendedBiome::HotSprings |
            ExtendedBiome::MirrorLake |
            ExtendedBiome::InkSea |
            ExtendedBiome::PhosphorShallows |
            ExtendedBiome::VoidScar |
            ExtendedBiome::SiliconGrove |
            ExtendedBiome::SporeWastes |
            ExtendedBiome::BleedingStone |
            ExtendedBiome::HollowEarth |
            ExtendedBiome::SunkenCity |
            ExtendedBiome::CyclopeanRuins |
            ExtendedBiome::BuriedTemple |
            ExtendedBiome::OvergrownCitadel
        )
    }

    /// Get all fantasy biomes
    pub fn fantasy_biomes() -> &'static [ExtendedBiome] {
        &[
            ExtendedBiome::DeadForest,
            ExtendedBiome::CrystalForest,
            ExtendedBiome::BioluminescentForest,
            ExtendedBiome::MushroomForest,
            ExtendedBiome::PetrifiedForest,
            ExtendedBiome::AcidLake,
            ExtendedBiome::LavaLake,
            ExtendedBiome::FrozenLake,
            ExtendedBiome::BioluminescentWater,
            ExtendedBiome::VolcanicWasteland,
            ExtendedBiome::SaltFlats,
            ExtendedBiome::Ashlands,
            ExtendedBiome::CrystalWasteland,
            ExtendedBiome::Swamp,
            ExtendedBiome::Marsh,
            ExtendedBiome::Bog,
            ExtendedBiome::MangroveSaltmarsh,
            // Ultra-rare (original 15)
            ExtendedBiome::AncientGrove,
            ExtendedBiome::TitanBones,
            ExtendedBiome::CoralPlateau,
            ExtendedBiome::ObsidianFields,
            ExtendedBiome::Geysers,
            ExtendedBiome::TarPits,
            ExtendedBiome::FloatingStones,
            ExtendedBiome::Shadowfen,
            ExtendedBiome::PrismaticPools,
            ExtendedBiome::AuroraWastes,
            ExtendedBiome::SingingDunes,
            ExtendedBiome::Oasis,
            ExtendedBiome::GlassDesert,
            ExtendedBiome::AbyssalVents,
            ExtendedBiome::Sargasso,
            // New biomes (29)
            ExtendedBiome::EtherealMist,
            ExtendedBiome::StarfallCrater,
            ExtendedBiome::LeyNexus,
            ExtendedBiome::WhisperingStones,
            ExtendedBiome::SpiritMarsh,
            ExtendedBiome::SulfurVents,
            ExtendedBiome::BasaltColumns,
            ExtendedBiome::PaintedHills,
            ExtendedBiome::RazorPeaks,
            ExtendedBiome::SinkholeLakes,
            ExtendedBiome::ColossalHive,
            ExtendedBiome::BoneFields,
            ExtendedBiome::CarnivorousBog,
            ExtendedBiome::FungalBloom,
            ExtendedBiome::KelpTowers,
            ExtendedBiome::BrinePools,
            ExtendedBiome::HotSprings,
            ExtendedBiome::MirrorLake,
            ExtendedBiome::InkSea,
            ExtendedBiome::PhosphorShallows,
            ExtendedBiome::VoidScar,
            ExtendedBiome::SiliconGrove,
            ExtendedBiome::SporeWastes,
            ExtendedBiome::BleedingStone,
            ExtendedBiome::HollowEarth,
            ExtendedBiome::SunkenCity,
            ExtendedBiome::CyclopeanRuins,
            ExtendedBiome::BuriedTemple,
            ExtendedBiome::OvergrownCitadel,
        ]
    }

    /// Get all ultra-rare biomes
    pub fn ultra_rare_biomes() -> &'static [ExtendedBiome] {
        &[
            // Original 15
            ExtendedBiome::AncientGrove,
            ExtendedBiome::TitanBones,
            ExtendedBiome::CoralPlateau,
            ExtendedBiome::ObsidianFields,
            ExtendedBiome::Geysers,
            ExtendedBiome::TarPits,
            ExtendedBiome::FloatingStones,
            ExtendedBiome::Shadowfen,
            ExtendedBiome::PrismaticPools,
            ExtendedBiome::AuroraWastes,
            ExtendedBiome::SingingDunes,
            ExtendedBiome::Oasis,
            ExtendedBiome::GlassDesert,
            ExtendedBiome::AbyssalVents,
            ExtendedBiome::Sargasso,
            // New 29
            ExtendedBiome::EtherealMist,
            ExtendedBiome::StarfallCrater,
            ExtendedBiome::LeyNexus,
            ExtendedBiome::WhisperingStones,
            ExtendedBiome::SpiritMarsh,
            ExtendedBiome::SulfurVents,
            ExtendedBiome::BasaltColumns,
            ExtendedBiome::PaintedHills,
            ExtendedBiome::RazorPeaks,
            ExtendedBiome::SinkholeLakes,
            ExtendedBiome::ColossalHive,
            ExtendedBiome::BoneFields,
            ExtendedBiome::CarnivorousBog,
            ExtendedBiome::FungalBloom,
            ExtendedBiome::KelpTowers,
            ExtendedBiome::BrinePools,
            ExtendedBiome::HotSprings,
            ExtendedBiome::MirrorLake,
            ExtendedBiome::InkSea,
            ExtendedBiome::PhosphorShallows,
            ExtendedBiome::VoidScar,
            ExtendedBiome::SiliconGrove,
            ExtendedBiome::SporeWastes,
            ExtendedBiome::BleedingStone,
            ExtendedBiome::HollowEarth,
            ExtendedBiome::SunkenCity,
            ExtendedBiome::CyclopeanRuins,
            ExtendedBiome::BuriedTemple,
            ExtendedBiome::OvergrownCitadel,
        ]
    }

    /// Check if this is a unique biome (exactly one per map, guaranteed)
    pub fn is_unique(&self) -> bool {
        matches!(self, ExtendedBiome::DarkTower)
    }

    /// Get biome category for UI grouping
    pub fn category(&self) -> BiomeCategory {
        match self {
            ExtendedBiome::DeadForest |
            ExtendedBiome::CrystalForest |
            ExtendedBiome::BioluminescentForest |
            ExtendedBiome::MushroomForest |
            ExtendedBiome::PetrifiedForest => BiomeCategory::Forests,

            ExtendedBiome::AcidLake |
            ExtendedBiome::LavaLake |
            ExtendedBiome::FrozenLake |
            ExtendedBiome::BioluminescentWater => BiomeCategory::Waters,

            ExtendedBiome::VolcanicWasteland |
            ExtendedBiome::SaltFlats |
            ExtendedBiome::Ashlands |
            ExtendedBiome::CrystalWasteland => BiomeCategory::Wastelands,

            ExtendedBiome::Swamp |
            ExtendedBiome::Marsh |
            ExtendedBiome::Bog |
            ExtendedBiome::MangroveSaltmarsh => BiomeCategory::Wetlands,

            // Ultra-rare biomes (original 15)
            ExtendedBiome::AncientGrove |
            ExtendedBiome::TitanBones |
            ExtendedBiome::CoralPlateau |
            ExtendedBiome::ObsidianFields |
            ExtendedBiome::Geysers |
            ExtendedBiome::TarPits |
            ExtendedBiome::FloatingStones |
            ExtendedBiome::Shadowfen |
            ExtendedBiome::PrismaticPools |
            ExtendedBiome::AuroraWastes |
            ExtendedBiome::SingingDunes |
            ExtendedBiome::Oasis |
            ExtendedBiome::GlassDesert |
            ExtendedBiome::AbyssalVents |
            ExtendedBiome::Sargasso => BiomeCategory::UltraRare,

            // Mystical / Supernatural
            ExtendedBiome::EtherealMist |
            ExtendedBiome::StarfallCrater |
            ExtendedBiome::LeyNexus |
            ExtendedBiome::WhisperingStones |
            ExtendedBiome::SpiritMarsh => BiomeCategory::Mystical,

            // Extreme Geological
            ExtendedBiome::SulfurVents |
            ExtendedBiome::BasaltColumns |
            ExtendedBiome::PaintedHills |
            ExtendedBiome::RazorPeaks |
            ExtendedBiome::SinkholeLakes => BiomeCategory::Geological,

            // Biological Wonders
            ExtendedBiome::ColossalHive |
            ExtendedBiome::BoneFields |
            ExtendedBiome::CarnivorousBog |
            ExtendedBiome::FungalBloom |
            ExtendedBiome::KelpTowers => BiomeCategory::Biological,

            // Exotic Waters
            ExtendedBiome::BrinePools |
            ExtendedBiome::HotSprings |
            ExtendedBiome::MirrorLake |
            ExtendedBiome::InkSea |
            ExtendedBiome::PhosphorShallows => BiomeCategory::ExoticWaters,

            // Alien / Corrupted
            ExtendedBiome::VoidScar |
            ExtendedBiome::SiliconGrove |
            ExtendedBiome::SporeWastes |
            ExtendedBiome::BleedingStone |
            ExtendedBiome::HollowEarth => BiomeCategory::Alien,

            // Ancient Ruins
            ExtendedBiome::SunkenCity |
            ExtendedBiome::CyclopeanRuins |
            ExtendedBiome::BuriedTemple |
            ExtendedBiome::OvergrownCitadel |
            ExtendedBiome::DarkTower => BiomeCategory::Ruins,

            // Ocean Zones - Realistic and Fantasy underwater biomes
            ExtendedBiome::CoralReef |
            ExtendedBiome::KelpForest |
            ExtendedBiome::SeagrassMeadow |
            ExtendedBiome::ContinentalShelf |
            ExtendedBiome::Seamount |
            ExtendedBiome::OceanicTrench |
            ExtendedBiome::AbyssalPlain |
            ExtendedBiome::MidOceanRidge |
            ExtendedBiome::ColdSeep |
            ExtendedBiome::BrinePool |
            ExtendedBiome::CrystalDepths |
            ExtendedBiome::LeviathanGraveyard |
            ExtendedBiome::DrownedCitadel |
            ExtendedBiome::VoidMaw |
            ExtendedBiome::PearlGardens |
            ExtendedBiome::SirenShallows |
            ExtendedBiome::FrozenAbyss |
            ExtendedBiome::ThermalVents => BiomeCategory::OceanZones,

            _ => BiomeCategory::Base,
        }
    }
}

/// Biome categories for UI grouping
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeCategory {
    Base,
    Forests,
    Waters,
    Wastelands,
    Wetlands,
    UltraRare,
    // New categories for the 29 biomes
    Mystical,
    Geological,
    Biological,
    ExoticWaters,
    Alien,
    Ruins,
    // Ocean biomes
    OceanZones,
}

impl BiomeCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            BiomeCategory::Base => "Base Biomes",
            BiomeCategory::Forests => "Fantasy Forests",
            BiomeCategory::Waters => "Fantasy Waters",
            BiomeCategory::Wastelands => "Wastelands",
            BiomeCategory::Wetlands => "Wetlands",
            BiomeCategory::UltraRare => "Ultra-Rare",
            BiomeCategory::Mystical => "Mystical",
            BiomeCategory::Geological => "Geological",
            BiomeCategory::Biological => "Biological",
            BiomeCategory::ExoticWaters => "Exotic Waters",
            BiomeCategory::Alien => "Alien",
            BiomeCategory::Ruins => "Ruins",
            BiomeCategory::OceanZones => "Ocean Zones",
        }
    }

    pub fn all_fantasy() -> &'static [BiomeCategory] {
        &[
            BiomeCategory::Forests,
            BiomeCategory::Waters,
            BiomeCategory::Wastelands,
            BiomeCategory::Wetlands,
            BiomeCategory::UltraRare,
            BiomeCategory::Mystical,
            BiomeCategory::Geological,
            BiomeCategory::Biological,
            BiomeCategory::ExoticWaters,
            BiomeCategory::Alien,
            BiomeCategory::Ruins,
            BiomeCategory::OceanZones,
        ]
    }
}

/// Configuration for a single biome
#[derive(Clone, Debug)]
pub struct BiomeConfig {
    pub enabled: bool,
    pub rarity: f32,  // 0.0 = never, 1.0 = common
}

impl Default for BiomeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rarity: 0.3,
        }
    }
}

/// World biome configuration
#[derive(Clone, Debug)]
pub struct WorldBiomeConfig {
    pub biomes: HashMap<ExtendedBiome, BiomeConfig>,
    pub fantasy_intensity: f32,  // 0.0 = realistic, 1.0 = full fantasy
}

impl Default for WorldBiomeConfig {
    fn default() -> Self {
        let mut biomes = HashMap::new();

        // Set default rarity for each fantasy biome
        biomes.insert(ExtendedBiome::DeadForest, BiomeConfig { enabled: true, rarity: 0.4 });
        biomes.insert(ExtendedBiome::CrystalForest, BiomeConfig { enabled: true, rarity: 0.1 });
        biomes.insert(ExtendedBiome::BioluminescentForest, BiomeConfig { enabled: true, rarity: 0.2 });
        biomes.insert(ExtendedBiome::MushroomForest, BiomeConfig { enabled: true, rarity: 0.25 });
        biomes.insert(ExtendedBiome::PetrifiedForest, BiomeConfig { enabled: true, rarity: 0.3 });

        biomes.insert(ExtendedBiome::AcidLake, BiomeConfig { enabled: true, rarity: 0.2 });
        biomes.insert(ExtendedBiome::LavaLake, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::FrozenLake, BiomeConfig { enabled: true, rarity: 0.3 });
        biomes.insert(ExtendedBiome::BioluminescentWater, BiomeConfig { enabled: true, rarity: 0.15 });

        biomes.insert(ExtendedBiome::VolcanicWasteland, BiomeConfig { enabled: true, rarity: 0.4 });
        biomes.insert(ExtendedBiome::SaltFlats, BiomeConfig { enabled: true, rarity: 0.35 });
        biomes.insert(ExtendedBiome::Ashlands, BiomeConfig { enabled: true, rarity: 0.3 });
        biomes.insert(ExtendedBiome::CrystalWasteland, BiomeConfig { enabled: true, rarity: 0.1 });

        biomes.insert(ExtendedBiome::Swamp, BiomeConfig { enabled: true, rarity: 0.5 });
        biomes.insert(ExtendedBiome::Marsh, BiomeConfig { enabled: true, rarity: 0.5 });
        biomes.insert(ExtendedBiome::Bog, BiomeConfig { enabled: true, rarity: 0.4 });
        biomes.insert(ExtendedBiome::MangroveSaltmarsh, BiomeConfig { enabled: true, rarity: 0.35 });

        // Ultra-rare biomes (low but visible spawn rates)
        biomes.insert(ExtendedBiome::AncientGrove, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::TitanBones, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::CoralPlateau, BiomeConfig { enabled: true, rarity: 0.18 });
        biomes.insert(ExtendedBiome::ObsidianFields, BiomeConfig { enabled: true, rarity: 0.18 });
        biomes.insert(ExtendedBiome::Geysers, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::TarPits, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::FloatingStones, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::Shadowfen, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::PrismaticPools, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::AuroraWastes, BiomeConfig { enabled: true, rarity: 0.18 });
        biomes.insert(ExtendedBiome::SingingDunes, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::Oasis, BiomeConfig { enabled: true, rarity: 0.18 });
        biomes.insert(ExtendedBiome::GlassDesert, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::AbyssalVents, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::Sargasso, BiomeConfig { enabled: true, rarity: 0.18 });

        // New biomes - Mystical / Supernatural
        biomes.insert(ExtendedBiome::EtherealMist, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::StarfallCrater, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::LeyNexus, BiomeConfig { enabled: true, rarity: 0.08 });
        biomes.insert(ExtendedBiome::WhisperingStones, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::SpiritMarsh, BiomeConfig { enabled: true, rarity: 0.12 });

        // New biomes - Extreme Geological
        biomes.insert(ExtendedBiome::SulfurVents, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::BasaltColumns, BiomeConfig { enabled: true, rarity: 0.14 });
        biomes.insert(ExtendedBiome::PaintedHills, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::RazorPeaks, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::SinkholeLakes, BiomeConfig { enabled: true, rarity: 0.14 });

        // New biomes - Biological Wonders
        biomes.insert(ExtendedBiome::ColossalHive, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::BoneFields, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::CarnivorousBog, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::FungalBloom, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::KelpTowers, BiomeConfig { enabled: true, rarity: 0.14 });

        // New biomes - Exotic Waters
        biomes.insert(ExtendedBiome::BrinePools, BiomeConfig { enabled: true, rarity: 0.14 });
        biomes.insert(ExtendedBiome::HotSprings, BiomeConfig { enabled: true, rarity: 0.15 });
        biomes.insert(ExtendedBiome::MirrorLake, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::InkSea, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::PhosphorShallows, BiomeConfig { enabled: true, rarity: 0.14 });

        // New biomes - Alien / Corrupted
        biomes.insert(ExtendedBiome::VoidScar, BiomeConfig { enabled: true, rarity: 0.08 });
        biomes.insert(ExtendedBiome::SiliconGrove, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::SporeWastes, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::BleedingStone, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::HollowEarth, BiomeConfig { enabled: true, rarity: 0.10 });

        // New biomes - Ancient Ruins
        biomes.insert(ExtendedBiome::SunkenCity, BiomeConfig { enabled: true, rarity: 0.12 });
        biomes.insert(ExtendedBiome::CyclopeanRuins, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::BuriedTemple, BiomeConfig { enabled: true, rarity: 0.10 });
        biomes.insert(ExtendedBiome::OvergrownCitadel, BiomeConfig { enabled: true, rarity: 0.12 });

        Self {
            biomes,
            fantasy_intensity: 0.5,
        }
    }
}

impl WorldBiomeConfig {
    /// Check if a fantasy biome should spawn based on its config
    pub fn should_spawn(&self, biome: ExtendedBiome, noise: f32) -> bool {
        if let Some(config) = self.biomes.get(&biome) {
            if !config.enabled {
                return false;
            }
            // Noise threshold based on rarity and fantasy intensity
            // Higher rarity = lower threshold = more spawns
            let threshold = 1.0 - (config.rarity * self.fantasy_intensity);
            noise > threshold
        } else {
            false
        }
    }
}

/// Classify a cell into an extended biome
pub fn classify_extended(
    elevation: f32,
    temperature: f32,
    moisture: f32,
    stress: f32,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    config: &WorldBiomeConfig,
    noise: &Perlin,
) -> ExtendedBiome {
    // First, get the base biome
    let base = Biome::classify(elevation, temperature, moisture);
    let base_extended = ExtendedBiome::from_base(base);

    // If fantasy intensity is 0, just return base biome
    if config.fantasy_intensity <= 0.0 {
        return base_extended;
    }

    // Sample noise for this position
    let nx = x as f64 / width as f64 * 10.0;
    let ny = y as f64 / height as f64 * 10.0;
    let noise_val = (noise.get([nx, ny]) as f32 + 1.0) * 0.5; // Normalize to 0-1

    // Try to convert to fantasy biome based on conditions
    maybe_convert_to_fantasy(
        base_extended,
        elevation,
        temperature,
        moisture,
        stress,
        noise_val,
        config,
    )
}

/// Attempt to convert a base biome to a fantasy variant
fn maybe_convert_to_fantasy(
    base: ExtendedBiome,
    elevation: f32,
    temperature: f32,
    moisture: f32,
    stress: f32,
    noise: f32,
    config: &WorldBiomeConfig,
) -> ExtendedBiome {
    // Check each possible conversion based on the base biome
    match base {
        // Forest conversions
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest => {
            // ULTRA-RARE: Ancient Grove - primeval forest with colossal trees
            if moisture > 0.55 && elevation > 50.0 && elevation < 400.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::AncientGrove, noise) {
                    return ExtendedBiome::AncientGrove;
                }
            }
            // NEW: Silicon Grove - crystallized/alien forest
            if stress > 0.35 && temperature < 10.0 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::SiliconGrove, noise) {
                    return ExtendedBiome::SiliconGrove;
                }
            }
            // NEW: Ethereal Mist - fog-shrouded mysterious forest
            if moisture > 0.65 && temperature > 5.0 && temperature < 15.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::EtherealMist, noise) {
                    return ExtendedBiome::EtherealMist;
                }
            }
            // NEW: Overgrown Citadel - ruins reclaimed by nature
            if elevation > 100.0 && elevation < 300.0 && stress > 0.25 && noise > 0.73 {
                if config.should_spawn(ExtendedBiome::OvergrownCitadel, noise) {
                    return ExtendedBiome::OvergrownCitadel;
                }
            }
            // Dead Forest: dry, cold areas
            if moisture < 0.35 && temperature < 8.0 {
                if config.should_spawn(ExtendedBiome::DeadForest, noise) {
                    return ExtendedBiome::DeadForest;
                }
            }
            // Crystal Forest: very rare, random
            if config.should_spawn(ExtendedBiome::CrystalForest, noise * 0.5) {
                return ExtendedBiome::CrystalForest;
            }
            // Petrified Forest: near volcanic areas
            if stress > 0.3 {
                if config.should_spawn(ExtendedBiome::PetrifiedForest, noise) {
                    return ExtendedBiome::PetrifiedForest;
                }
            }
            // ULTRA-RARE: Titan Bones - massive skeletal remains
            if stress > 0.4 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::TitanBones, noise) {
                    return ExtendedBiome::TitanBones;
                }
            }
        }

        ExtendedBiome::TropicalRainforest => {
            // ULTRA-RARE: Ancient Grove in tropical setting
            if moisture > 0.7 && elevation > 30.0 && elevation < 250.0 && noise > 0.65 {
                if config.should_spawn(ExtendedBiome::AncientGrove, noise) {
                    return ExtendedBiome::AncientGrove;
                }
            }
            // NEW: Fungal Bloom - massive fungal outgrowth
            if moisture > 0.7 && temperature > 22.0 && noise > 0.68 {
                if config.should_spawn(ExtendedBiome::FungalBloom, noise) {
                    return ExtendedBiome::FungalBloom;
                }
            }
            // NEW: Colossal Hive - giant insect colonies
            if temperature > 20.0 && moisture > 0.6 && stress < 0.3 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::ColossalHive, noise) {
                    return ExtendedBiome::ColossalHive;
                }
            }
            // NEW: Spore Wastes - fungal corruption spreading
            if moisture > 0.55 && stress > 0.2 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::SporeWastes, noise) {
                    return ExtendedBiome::SporeWastes;
                }
            }
            // NEW: Overgrown Citadel in jungle
            if elevation > 50.0 && elevation < 200.0 && noise > 0.74 {
                if config.should_spawn(ExtendedBiome::OvergrownCitadel, noise) {
                    return ExtendedBiome::OvergrownCitadel;
                }
            }
            // ULTRA-RARE: Shadowfen - dark, light-absorbing swamp
            if elevation < 30.0 && elevation >= 0.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::Shadowfen, noise) {
                    return ExtendedBiome::Shadowfen;
                }
            }
            // Bioluminescent Forest: very wet tropical
            if moisture > 0.75 {
                if config.should_spawn(ExtendedBiome::BioluminescentForest, noise) {
                    return ExtendedBiome::BioluminescentForest;
                }
            }
        }

        ExtendedBiome::TropicalForest => {
            // Mushroom Forest: warm, wet
            if moisture > 0.6 && temperature > 18.0 {
                if config.should_spawn(ExtendedBiome::MushroomForest, noise) {
                    return ExtendedBiome::MushroomForest;
                }
            }
        }

        // Desert conversions
        ExtendedBiome::Desert => {
            // NEW: Starfall Crater - meteor impact site
            if stress > 0.35 && noise > 0.73 {
                if config.should_spawn(ExtendedBiome::StarfallCrater, noise) {
                    return ExtendedBiome::StarfallCrater;
                }
            }
            // NEW: Buried Temple - sand-covered ancient temple
            if elevation < 200.0 && elevation > 50.0 && noise > 0.74 {
                if config.should_spawn(ExtendedBiome::BuriedTemple, noise) {
                    return ExtendedBiome::BuriedTemple;
                }
            }
            // NEW: Painted Hills - colorful layered rock formations
            if stress > 0.2 && stress < 0.4 && noise > 0.68 {
                if config.should_spawn(ExtendedBiome::PaintedHills, noise) {
                    return ExtendedBiome::PaintedHills;
                }
            }
            // NEW: Bone Fields - ancient mass extinction site
            if temperature > 25.0 && moisture < 0.15 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::BoneFields, noise) {
                    return ExtendedBiome::BoneFields;
                }
            }
            // NEW: Cyclopean Ruins - massive ancient structures
            if elevation > 100.0 && stress > 0.25 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::CyclopeanRuins, noise) {
                    return ExtendedBiome::CyclopeanRuins;
                }
            }
            // NEW: Brine Pools - super-salty mineral pools
            if elevation < 80.0 && moisture > 0.1 && moisture < 0.25 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::BrinePools, noise) {
                    return ExtendedBiome::BrinePools;
                }
            }
            // NEW: Sulfur Vents - volcanic sulfur emissions
            if stress > 0.45 && temperature > 30.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::SulfurVents, noise) {
                    return ExtendedBiome::SulfurVents;
                }
            }
            // ULTRA-RARE: Glass Desert - sand fused to glass by ancient heat
            if stress > 0.45 && temperature > 25.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::GlassDesert, noise) {
                    return ExtendedBiome::GlassDesert;
                }
            }
            // ULTRA-RARE: Obsidian Fields - volcanic glass plains
            if stress > 0.4 && noise > 0.65 {
                if config.should_spawn(ExtendedBiome::ObsidianFields, noise) {
                    return ExtendedBiome::ObsidianFields;
                }
            }
            // ULTRA-RARE: Singing Dunes - eerie sound-producing sands
            if temperature > 25.0 && moisture < 0.2 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::SingingDunes, noise) {
                    return ExtendedBiome::SingingDunes;
                }
            }
            // ULTRA-RARE: Oasis - fertile spring in desert
            if moisture > 0.15 && stress < 0.3 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::Oasis, noise) {
                    return ExtendedBiome::Oasis;
                }
            }
            // ULTRA-RARE: Tar Pits - natural asphalt lakes
            if elevation < 100.0 && temperature > 20.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::TarPits, noise) {
                    return ExtendedBiome::TarPits;
                }
            }
            // Volcanic Wasteland: high stress
            if stress > 0.4 {
                if config.should_spawn(ExtendedBiome::VolcanicWasteland, noise) {
                    return ExtendedBiome::VolcanicWasteland;
                }
            }
            // Salt Flats: very hot, very dry
            if temperature > 25.0 && moisture < 0.15 {
                if config.should_spawn(ExtendedBiome::SaltFlats, noise) {
                    return ExtendedBiome::SaltFlats;
                }
            }
            // Crystal Wasteland: rare random
            if config.should_spawn(ExtendedBiome::CrystalWasteland, noise * 0.4) {
                return ExtendedBiome::CrystalWasteland;
            }
        }

        // Savanna conversions
        ExtendedBiome::Savanna => {
            // NEW: Colossal Hive - giant insect colonies
            if temperature > 20.0 && moisture > 0.3 && stress < 0.3 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::ColossalHive, noise) {
                    return ExtendedBiome::ColossalHive;
                }
            }
            // NEW: Bone Fields - ancient mass extinction
            if moisture < 0.35 && temperature > 20.0 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::BoneFields, noise) {
                    return ExtendedBiome::BoneFields;
                }
            }
            // Ashlands: volcanic stress
            if stress > 0.25 {
                if config.should_spawn(ExtendedBiome::Ashlands, noise) {
                    return ExtendedBiome::Ashlands;
                }
            }
        }

        // Water conversions
        ExtendedBiome::CoastalWater => {
            // NEW: Kelp Towers - giant kelp forest
            if temperature > 8.0 && temperature < 20.0 && moisture > 0.6 && noise > 0.68 {
                if config.should_spawn(ExtendedBiome::KelpTowers, noise) {
                    return ExtendedBiome::KelpTowers;
                }
            }
            // NEW: Phosphor Shallows - bioluminescent shallow waters
            if temperature > 15.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::PhosphorShallows, noise) {
                    return ExtendedBiome::PhosphorShallows;
                }
            }
            // NEW: Sunken City - underwater ruins
            if elevation > -80.0 && elevation < -20.0 && noise > 0.74 {
                if config.should_spawn(ExtendedBiome::SunkenCity, noise) {
                    return ExtendedBiome::SunkenCity;
                }
            }
            // NEW: Basalt Columns - coastal volcanic columns
            if stress > 0.3 && temperature < 15.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::BasaltColumns, noise) {
                    return ExtendedBiome::BasaltColumns;
                }
            }
            // ULTRA-RARE: Prismatic Pools - rainbow mineral springs
            if stress > 0.25 && temperature > 10.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::PrismaticPools, noise) {
                    return ExtendedBiome::PrismaticPools;
                }
            }
            // ULTRA-RARE: Sargasso - floating seaweed mass
            if temperature > 5.0 && temperature < 28.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::Sargasso, noise) {
                    return ExtendedBiome::Sargasso;
                }
            }
            // Acid Lake: volcanic stress, shallow
            if stress > 0.5 && elevation > -100.0 {
                if config.should_spawn(ExtendedBiome::AcidLake, noise) {
                    return ExtendedBiome::AcidLake;
                }
            }
            // Bioluminescent Water: rare coastal
            if config.should_spawn(ExtendedBiome::BioluminescentWater, noise * 0.6) {
                return ExtendedBiome::BioluminescentWater;
            }
        }

        ExtendedBiome::Ocean => {
            // NEW: Sunken City - underwater ruins in ocean
            if elevation > -200.0 && elevation < -50.0 && noise > 0.73 {
                if config.should_spawn(ExtendedBiome::SunkenCity, noise) {
                    return ExtendedBiome::SunkenCity;
                }
            }
            // ULTRA-RARE: Abyssal Vents - deep sea hydrothermal vents
            if stress > 0.4 && elevation < -150.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::AbyssalVents, noise) {
                    return ExtendedBiome::AbyssalVents;
                }
            }
            // ULTRA-RARE: Sargasso in open ocean
            if temperature > 8.0 && temperature < 25.0 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::Sargasso, noise) {
                    return ExtendedBiome::Sargasso;
                }
            }
            // Lava Lake: very high stress, relatively shallow
            if stress > 0.7 && elevation > -500.0 {
                if config.should_spawn(ExtendedBiome::LavaLake, noise) {
                    return ExtendedBiome::LavaLake;
                }
            }
        }

        ExtendedBiome::DeepOcean => {
            // NEW: Ink Sea - dark mysterious deep waters
            if temperature < 5.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::InkSea, noise) {
                    return ExtendedBiome::InkSea;
                }
            }
            // NEW: Void Scar - reality tear in the deep
            if stress > 0.5 && noise > 0.76 {
                if config.should_spawn(ExtendedBiome::VoidScar, noise) {
                    return ExtendedBiome::VoidScar;
                }
            }
            // ULTRA-RARE: Abyssal Vents - hydrothermal vents in deep ocean
            if stress > 0.35 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::AbyssalVents, noise) {
                    return ExtendedBiome::AbyssalVents;
                }
            }
        }

        // Cold water / ice conversions
        ExtendedBiome::Ice => {
            // NEW: Hot Springs - thermal pools in frozen areas
            if stress > 0.25 && noise > 0.68 {
                if config.should_spawn(ExtendedBiome::HotSprings, noise) {
                    return ExtendedBiome::HotSprings;
                }
            }
            // NEW: Mirror Lake - perfectly still reflective ice lake
            if stress < 0.2 && elevation > -30.0 && elevation < 50.0 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::MirrorLake, noise) {
                    return ExtendedBiome::MirrorLake;
                }
            }
            // ULTRA-RARE: Geysers - hot springs in frozen areas
            if stress > 0.3 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::Geysers, noise) {
                    return ExtendedBiome::Geysers;
                }
            }
            // Frozen Lake: flat cold areas
            if elevation > -50.0 && elevation < 100.0 {
                if config.should_spawn(ExtendedBiome::FrozenLake, noise) {
                    return ExtendedBiome::FrozenLake;
                }
            }
        }

        // Grassland / low area conversions (wetlands)
        ExtendedBiome::TemperateGrassland => {
            // NEW: Starfall Crater - meteor impact in grassland
            if stress > 0.3 && noise > 0.74 {
                if config.should_spawn(ExtendedBiome::StarfallCrater, noise) {
                    return ExtendedBiome::StarfallCrater;
                }
            }
            // NEW: Whispering Stones - ancient standing stones
            if elevation > 80.0 && elevation < 250.0 && stress > 0.2 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::WhisperingStones, noise) {
                    return ExtendedBiome::WhisperingStones;
                }
            }
            // NEW: Ley Nexus - magical convergence point
            if stress > 0.35 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::LeyNexus, noise) {
                    return ExtendedBiome::LeyNexus;
                }
            }
            // NEW: Sinkhole Lakes - collapse lakes
            if elevation < 100.0 && moisture > 0.4 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::SinkholeLakes, noise) {
                    return ExtendedBiome::SinkholeLakes;
                }
            }
            // NEW: Cyclopean Ruins in grassland
            if elevation > 50.0 && elevation < 200.0 && stress > 0.2 && noise > 0.74 {
                if config.should_spawn(ExtendedBiome::CyclopeanRuins, noise) {
                    return ExtendedBiome::CyclopeanRuins;
                }
            }
            // NEW: Bone Fields - ancient mass extinction
            if moisture < 0.4 && temperature > 10.0 && noise > 0.73 {
                if config.should_spawn(ExtendedBiome::BoneFields, noise) {
                    return ExtendedBiome::BoneFields;
                }
            }
            // NEW: Colossal Hive in grassland
            if temperature > 15.0 && moisture > 0.35 && stress < 0.3 && noise > 0.73 {
                if config.should_spawn(ExtendedBiome::ColossalHive, noise) {
                    return ExtendedBiome::ColossalHive;
                }
            }
            // NEW: Hollow Earth - entrance to underground
            if stress > 0.4 && elevation > 100.0 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::HollowEarth, noise) {
                    return ExtendedBiome::HollowEarth;
                }
            }
            // ULTRA-RARE: Coral Plateau - fossilized coral on land (ancient seabed)
            if elevation < 150.0 && elevation > 5.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::CoralPlateau, noise) {
                    return ExtendedBiome::CoralPlateau;
                }
            }
            // ULTRA-RARE: Titan Bones in grassland
            if stress > 0.25 && noise > 0.75 {
                if config.should_spawn(ExtendedBiome::TitanBones, noise) {
                    return ExtendedBiome::TitanBones;
                }
            }
            // Swamp: low, wet, warm
            if elevation < 30.0 && elevation >= 0.0 && moisture > 0.55 && temperature > 10.0 {
                if config.should_spawn(ExtendedBiome::Swamp, noise) {
                    return ExtendedBiome::Swamp;
                }
            }
            // Marsh: low, moderately wet
            if elevation < 50.0 && elevation >= 0.0 && moisture > 0.45 {
                if config.should_spawn(ExtendedBiome::Marsh, noise) {
                    return ExtendedBiome::Marsh;
                }
            }
            // Mangrove: coastal, warm, wet
            if elevation < 15.0 && elevation >= 0.0 && moisture > 0.5 && temperature > 15.0 {
                if config.should_spawn(ExtendedBiome::MangroveSaltmarsh, noise) {
                    return ExtendedBiome::MangroveSaltmarsh;
                }
            }
        }

        // Tundra conversions
        ExtendedBiome::Tundra => {
            // NEW: Whispering Stones in tundra
            if elevation > 50.0 && stress > 0.2 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::WhisperingStones, noise) {
                    return ExtendedBiome::WhisperingStones;
                }
            }
            // NEW: Spirit Marsh - haunted frozen marshland
            if moisture > 0.5 && elevation < 50.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::SpiritMarsh, noise) {
                    return ExtendedBiome::SpiritMarsh;
                }
            }
            // NEW: Hot Springs in tundra
            if stress > 0.25 && noise > 0.68 {
                if config.should_spawn(ExtendedBiome::HotSprings, noise) {
                    return ExtendedBiome::HotSprings;
                }
            }
            // ULTRA-RARE: Aurora Wastes - frozen tundra under permanent aurora
            if temperature < -10.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::AuroraWastes, noise) {
                    return ExtendedBiome::AuroraWastes;
                }
            }
            // ULTRA-RARE: Geysers in tundra
            if stress > 0.3 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::Geysers, noise) {
                    return ExtendedBiome::Geysers;
                }
            }
            // Bog: cold, wet
            if moisture > 0.6 {
                if config.should_spawn(ExtendedBiome::Bog, noise) {
                    return ExtendedBiome::Bog;
                }
            }
        }

        // Alpine / Mountain conversions
        ExtendedBiome::AlpineTundra => {
            // NEW: Razor Peaks - sharp jagged mountain ridges
            if stress > 0.4 && elevation > 200.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::RazorPeaks, noise) {
                    return ExtendedBiome::RazorPeaks;
                }
            }
            // NEW: Bleeding Stone - iron-red oozing rock
            if stress > 0.35 && moisture > 0.3 && noise > 0.72 {
                if config.should_spawn(ExtendedBiome::BleedingStone, noise) {
                    return ExtendedBiome::BleedingStone;
                }
            }
            // NEW: Basalt Columns - volcanic rock formations
            if stress > 0.35 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::BasaltColumns, noise) {
                    return ExtendedBiome::BasaltColumns;
                }
            }
            // NEW: Hollow Earth - entrance to underground
            if stress > 0.4 && noise > 0.74 {
                if config.should_spawn(ExtendedBiome::HollowEarth, noise) {
                    return ExtendedBiome::HollowEarth;
                }
            }
            // ULTRA-RARE: Floating Stones - magnetic anomaly zone
            if stress > 0.35 && elevation > 150.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::FloatingStones, noise) {
                    return ExtendedBiome::FloatingStones;
                }
            }
            // ULTRA-RARE: Geysers in alpine
            if stress > 0.3 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::Geysers, noise) {
                    return ExtendedBiome::Geysers;
                }
            }
        }

        ExtendedBiome::SnowyPeaks => {
            // NEW: Razor Peaks - sharp jagged mountain ridges
            if stress > 0.45 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::RazorPeaks, noise) {
                    return ExtendedBiome::RazorPeaks;
                }
            }
            // NEW: Void Scar - reality tear at highest peaks
            if stress > 0.5 && noise > 0.77 {
                if config.should_spawn(ExtendedBiome::VoidScar, noise) {
                    return ExtendedBiome::VoidScar;
                }
            }
            // ULTRA-RARE: Floating Stones at highest peaks
            if stress > 0.4 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::FloatingStones, noise) {
                    return ExtendedBiome::FloatingStones;
                }
            }
        }

        // Wetland additions for Swamp/Marsh/Bog base conversions
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => {
            // NEW: Carnivorous Bog - predatory plant bog
            if moisture > 0.6 && temperature > 10.0 && noise > 0.7 {
                if config.should_spawn(ExtendedBiome::CarnivorousBog, noise) {
                    return ExtendedBiome::CarnivorousBog;
                }
            }
            // NEW: Spirit Marsh - haunted marshland
            if noise > 0.72 {
                if config.should_spawn(ExtendedBiome::SpiritMarsh, noise) {
                    return ExtendedBiome::SpiritMarsh;
                }
            }
        }

        _ => {}
    }

    // No conversion, return base biome
    base
}

/// Generate extended biome map
pub fn generate_extended_biomes(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    config: &WorldBiomeConfig,
    seed: u64,
) -> Tilemap<ExtendedBiome> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Create noise generator for biome variation
    let noise = Perlin::new(1).set_seed(seed as u32);

    let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::Ocean);

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);
            let stress = *stress_map.get(x, y);

            let biome = classify_extended(
                elev, temp, moist, stress,
                x, y, width, height,
                config, &noise,
            );

            biomes.set(x, y, biome);
        }
    }

    biomes
}

// ============================================================================
// BIOME REPLACEMENT SYSTEM
// ============================================================================
// Rare biomes appear by replacing common biomes under specific conditions.
// This creates natural-feeling placement where special biomes emerge from
// the base terrain.

/// Conditions for biome replacement
#[derive(Clone, Debug)]
pub struct ReplacementCondition {
    /// Minimum temperature (-30 to 30)
    pub temp_min: f32,
    /// Maximum temperature
    pub temp_max: f32,
    /// Minimum moisture (0-1)
    pub moisture_min: f32,
    /// Maximum moisture
    pub moisture_max: f32,
    /// Minimum stress (volcanic activity, 0-1)
    pub stress_min: f32,
    /// Maximum stress
    pub stress_max: f32,
    /// Minimum elevation (meters)
    pub elevation_min: f32,
    /// Maximum elevation
    pub elevation_max: f32,
}

impl Default for ReplacementCondition {
    fn default() -> Self {
        Self {
            temp_min: -50.0,
            temp_max: 50.0,
            moisture_min: 0.0,
            moisture_max: 1.0,
            stress_min: 0.0,
            stress_max: 1.0,
            elevation_min: -10000.0,
            elevation_max: 10000.0,
        }
    }
}

impl ReplacementCondition {
    fn matches(&self, temp: f32, moisture: f32, stress: f32, elevation: f32) -> bool {
        temp >= self.temp_min && temp <= self.temp_max
            && moisture >= self.moisture_min && moisture <= self.moisture_max
            && stress >= self.stress_min && stress <= self.stress_max
            && elevation >= self.elevation_min && elevation <= self.elevation_max
    }
}

/// A rule for replacing a common biome with a rare one
#[derive(Clone, Debug)]
pub struct ReplacementRule {
    /// The rare biome that will appear
    pub target: ExtendedBiome,
    /// Biomes that can be replaced by this one
    pub replaces: Vec<ExtendedBiome>,
    /// Conditions required for replacement
    pub condition: ReplacementCondition,
    /// Base chance of replacement (0.0-1.0)
    pub chance: f32,
    /// Whether this forms clusters or isolated tiles
    pub cluster_size: usize,
    /// Description for debugging
    pub description: &'static str,
}

/// Get all biome replacement rules
pub fn get_replacement_rules() -> Vec<ReplacementRule> {
    vec![
        // ===== FOREST REPLACEMENTS =====

        // Mushroom Forest - replaces forests in warm, moist areas
        ReplacementRule {
            target: ExtendedBiome::MushroomForest,
            replaces: vec![
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TemperateRainforest,
            ],
            condition: ReplacementCondition {
                temp_min: 5.0,
                temp_max: 25.0,
                moisture_min: 0.5,
                ..Default::default()
            },
            chance: 0.03,
            cluster_size: 5,
            description: "Mushroom forests in warm, moist areas",
        },

        // Crystal Forest - replaces forests in cold, high-stress areas
        ReplacementRule {
            target: ExtendedBiome::CrystalForest,
            replaces: vec![
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateForest,
            ],
            condition: ReplacementCondition {
                temp_min: -10.0,
                temp_max: 10.0,
                stress_min: 0.2,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Crystal forests in cold, geologically active areas",
        },

        // Bioluminescent Forest - replaces tropical forests
        ReplacementRule {
            target: ExtendedBiome::BioluminescentForest,
            replaces: vec![
                ExtendedBiome::TropicalForest,
                ExtendedBiome::TropicalRainforest,
            ],
            condition: ReplacementCondition {
                temp_min: 20.0,
                moisture_min: 0.6,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 6,
            description: "Glowing forests in warm, wet jungles",
        },

        // Petrified Forest - replaces forests in dry, high-stress areas
        ReplacementRule {
            target: ExtendedBiome::PetrifiedForest,
            replaces: vec![
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateForest,
                ExtendedBiome::Savanna,
            ],
            condition: ReplacementCondition {
                moisture_max: 0.4,
                stress_min: 0.15,
                ..Default::default()
            },
            chance: 0.025,
            cluster_size: 4,
            description: "Ancient petrified forests in dry areas",
        },

        // Dead Forest - replaces forests near volcanic areas
        ReplacementRule {
            target: ExtendedBiome::DeadForest,
            replaces: vec![
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TropicalForest,
            ],
            condition: ReplacementCondition {
                stress_min: 0.3,
                ..Default::default()
            },
            chance: 0.04,
            cluster_size: 5,
            description: "Dead forests near volcanic activity",
        },

        // Ancient Grove - replaces old forests
        ReplacementRule {
            target: ExtendedBiome::AncientGrove,
            replaces: vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TemperateRainforest,
                ExtendedBiome::TropicalRainforest,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.5,
                stress_max: 0.1,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 3,
            description: "Primeval ancient groves",
        },

        // Silicon Grove - alien forest replacing normal forests
        ReplacementRule {
            target: ExtendedBiome::SiliconGrove,
            replaces: vec![
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateForest,
            ],
            condition: ReplacementCondition {
                stress_min: 0.25,
                temp_min: -5.0,
                temp_max: 15.0,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 4,
            description: "Alien silicon-based forests",
        },

        // ===== DESERT REPLACEMENTS =====

        // Salt Flats - replaces desert
        ReplacementRule {
            target: ExtendedBiome::SaltFlats,
            replaces: vec![ExtendedBiome::Desert],
            condition: ReplacementCondition {
                moisture_max: 0.2,
                elevation_min: -50.0,
                elevation_max: 200.0,
                ..Default::default()
            },
            chance: 0.06,
            cluster_size: 8,
            description: "Salt flats in low desert areas",
        },

        // Glass Desert - replaces desert near volcanic
        ReplacementRule {
            target: ExtendedBiome::GlassDesert,
            replaces: vec![ExtendedBiome::Desert],
            condition: ReplacementCondition {
                stress_min: 0.25,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Glass deserts from ancient impacts/volcanism",
        },

        // Singing Dunes - replaces desert
        ReplacementRule {
            target: ExtendedBiome::SingingDunes,
            replaces: vec![ExtendedBiome::Desert],
            condition: ReplacementCondition {
                moisture_max: 0.15,
                stress_max: 0.15,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 6,
            description: "Musical sand dunes",
        },

        // Oasis - replaces desert near moisture
        ReplacementRule {
            target: ExtendedBiome::Oasis,
            replaces: vec![ExtendedBiome::Desert, ExtendedBiome::Savanna],
            condition: ReplacementCondition {
                moisture_min: 0.15,
                moisture_max: 0.35,
                temp_min: 15.0,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 2,
            description: "Desert oases",
        },

        // Bone Fields - replaces desert/savanna
        ReplacementRule {
            target: ExtendedBiome::BoneFields,
            replaces: vec![ExtendedBiome::Desert, ExtendedBiome::Savanna],
            condition: ReplacementCondition {
                moisture_max: 0.3,
                temp_min: 10.0,
                ..Default::default()
            },
            chance: 0.012,
            cluster_size: 4,
            description: "Ancient bone graveyards",
        },

        // ===== GRASSLAND REPLACEMENTS =====

        // Fungal Bloom - replaces grasslands
        ReplacementRule {
            target: ExtendedBiome::FungalBloom,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.3,
                moisture_max: 0.6,
                temp_min: 10.0,
                temp_max: 25.0,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 5,
            description: "Giant fungal blooms",
        },

        // Painted Hills - replaces grasslands/savanna
        ReplacementRule {
            target: ExtendedBiome::PaintedHills,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
                ExtendedBiome::Desert,
            ],
            condition: ReplacementCondition {
                moisture_max: 0.4,
                stress_min: 0.1,
                elevation_min: 100.0,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 5,
            description: "Colorful layered hills",
        },

        // Titan Bones - replaces grassland/savanna
        ReplacementRule {
            target: ExtendedBiome::TitanBones,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
            ],
            condition: ReplacementCondition {
                moisture_max: 0.4,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 3,
            description: "Giant creature remains",
        },

        // ===== TUNDRA/COLD REPLACEMENTS =====

        // Aurora Wastes - replaces tundra near poles
        ReplacementRule {
            target: ExtendedBiome::AuroraWastes,
            replaces: vec![ExtendedBiome::Tundra, ExtendedBiome::Ice],
            condition: ReplacementCondition {
                temp_max: -5.0,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 6,
            description: "Aurora-lit frozen wastes",
        },

        // Razor Peaks - replaces alpine areas
        ReplacementRule {
            target: ExtendedBiome::RazorPeaks,
            replaces: vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
            ],
            condition: ReplacementCondition {
                elevation_min: 500.0,
                stress_min: 0.2,
                ..Default::default()
            },
            chance: 0.03,
            cluster_size: 4,
            description: "Jagged crystalline peaks",
        },

        // Whispering Stones - replaces tundra
        ReplacementRule {
            target: ExtendedBiome::WhisperingStones,
            replaces: vec![ExtendedBiome::Tundra, ExtendedBiome::AlpineTundra],
            condition: ReplacementCondition {
                temp_max: 5.0,
                stress_min: 0.1,
                ..Default::default()
            },
            chance: 0.012,
            cluster_size: 3,
            description: "Wind-carved singing stones",
        },

        // ===== VOLCANIC REPLACEMENTS =====

        // Volcanic Wasteland - replaces near high stress
        ReplacementRule {
            target: ExtendedBiome::VolcanicWasteland,
            replaces: vec![
                ExtendedBiome::Desert,
                ExtendedBiome::Savanna,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                stress_min: 0.4,
                ..Default::default()
            },
            chance: 0.08,
            cluster_size: 6,
            description: "Active volcanic wastelands",
        },

        // Ashlands - replaces areas near volcanic
        ReplacementRule {
            target: ExtendedBiome::Ashlands,
            replaces: vec![
                ExtendedBiome::Desert,
                ExtendedBiome::Savanna,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Tundra,
            ],
            condition: ReplacementCondition {
                stress_min: 0.3,
                stress_max: 0.5,
                ..Default::default()
            },
            chance: 0.05,
            cluster_size: 5,
            description: "Ash-covered lands",
        },

        // Obsidian Fields - replaces volcanic areas
        ReplacementRule {
            target: ExtendedBiome::ObsidianFields,
            replaces: vec![
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Ashlands,
            ],
            condition: ReplacementCondition {
                stress_min: 0.35,
                ..Default::default()
            },
            chance: 0.04,
            cluster_size: 4,
            description: "Black glass volcanic fields",
        },

        // Sulfur Vents - replaces volcanic areas
        ReplacementRule {
            target: ExtendedBiome::SulfurVents,
            replaces: vec![
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Ashlands,
                ExtendedBiome::Desert,
            ],
            condition: ReplacementCondition {
                stress_min: 0.3,
                moisture_max: 0.3,
                ..Default::default()
            },
            chance: 0.025,
            cluster_size: 3,
            description: "Sulfurous fumaroles",
        },

        // Geysers - replaces volcanic/tundra
        ReplacementRule {
            target: ExtendedBiome::Geysers,
            replaces: vec![
                ExtendedBiome::Tundra,
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Ashlands,
            ],
            condition: ReplacementCondition {
                stress_min: 0.2,
                moisture_min: 0.2,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 3,
            description: "Geyser fields",
        },

        // Basalt Columns - replaces volcanic areas
        ReplacementRule {
            target: ExtendedBiome::BasaltColumns,
            replaces: vec![
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Ashlands,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                stress_min: 0.25,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Hexagonal basalt formations",
        },

        // ===== WETLAND REPLACEMENTS =====

        // Carnivorous Bog - replaces wetlands
        ReplacementRule {
            target: ExtendedBiome::CarnivorousBog,
            replaces: vec![
                ExtendedBiome::Bog,
                ExtendedBiome::Marsh,
                ExtendedBiome::Swamp,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.6,
                temp_min: 10.0,
                ..Default::default()
            },
            chance: 0.04,
            cluster_size: 4,
            description: "Bogs with carnivorous plants",
        },

        // Shadowfen - replaces wetlands
        ReplacementRule {
            target: ExtendedBiome::Shadowfen,
            replaces: vec![
                ExtendedBiome::Swamp,
                ExtendedBiome::Marsh,
                ExtendedBiome::Bog,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.5,
                stress_min: 0.1,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Dark, misty fens",
        },

        // Spirit Marsh - replaces wetlands
        ReplacementRule {
            target: ExtendedBiome::SpiritMarsh,
            replaces: vec![
                ExtendedBiome::Marsh,
                ExtendedBiome::Swamp,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.5,
                temp_max: 15.0,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 4,
            description: "Haunted marshlands",
        },

        // Tar Pits - replaces wetlands/desert edge
        ReplacementRule {
            target: ExtendedBiome::TarPits,
            replaces: vec![
                ExtendedBiome::Bog,
                ExtendedBiome::Marsh,
                ExtendedBiome::Savanna,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.2,
                moisture_max: 0.5,
                stress_min: 0.15,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 3,
            description: "Bubbling tar pits",
        },

        // ===== MYSTICAL/MAGICAL REPLACEMENTS =====

        // Ethereal Mist - replaces forests/wetlands
        ReplacementRule {
            target: ExtendedBiome::EtherealMist,
            replaces: vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::Swamp,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.5,
                temp_min: 0.0,
                temp_max: 20.0,
                stress_max: 0.15,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 5,
            description: "Perpetually misty magical areas",
        },

        // Starfall Crater - replaces various terrains
        ReplacementRule {
            target: ExtendedBiome::StarfallCrater,
            replaces: vec![
                ExtendedBiome::Desert,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Tundra,
            ],
            condition: ReplacementCondition {
                ..Default::default()
            },
            chance: 0.005,
            cluster_size: 3,
            description: "Meteorite impact sites",
        },

        // Ley Nexus - replaces forests/grassland
        ReplacementRule {
            target: ExtendedBiome::LeyNexus,
            replaces: vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                stress_min: 0.15,
                stress_max: 0.35,
                ..Default::default()
            },
            chance: 0.006,
            cluster_size: 2,
            description: "Magical energy convergence points",
        },

        // Prismatic Pools - replaces wetlands/coastal
        ReplacementRule {
            target: ExtendedBiome::PrismaticPools,
            replaces: vec![
                ExtendedBiome::Marsh,
                ExtendedBiome::Swamp,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.4,
                stress_min: 0.1,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 3,
            description: "Rainbow-colored mineral pools",
        },

        // Floating Stones - replaces highlands
        ReplacementRule {
            target: ExtendedBiome::FloatingStones,
            replaces: vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                elevation_min: 300.0,
                stress_min: 0.2,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 3,
            description: "Mysteriously floating rock formations",
        },

        // ===== ALIEN/CORRUPTED REPLACEMENTS =====

        // Void Scar - replaces any terrain with high stress
        ReplacementRule {
            target: ExtendedBiome::VoidScar,
            replaces: vec![
                ExtendedBiome::Desert,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Tundra,
            ],
            condition: ReplacementCondition {
                stress_min: 0.5,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 3,
            description: "Reality-torn areas",
        },

        // Spore Wastes - replaces forests/grasslands
        ReplacementRule {
            target: ExtendedBiome::SporeWastes,
            replaces: vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.3,
                stress_min: 0.2,
                ..Default::default()
            },
            chance: 0.012,
            cluster_size: 5,
            description: "Alien spore-covered wastelands",
        },

        // Bleeding Stone - replaces rocky areas
        ReplacementRule {
            target: ExtendedBiome::BleedingStone,
            replaces: vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::Desert,
                ExtendedBiome::VolcanicWasteland,
            ],
            condition: ReplacementCondition {
                stress_min: 0.3,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 3,
            description: "Red mineral-weeping rocks",
        },

        // Hollow Earth - replaces various
        ReplacementRule {
            target: ExtendedBiome::HollowEarth,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
            ],
            condition: ReplacementCondition {
                stress_min: 0.25,
                elevation_min: 50.0,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 3,
            description: "Areas with cave systems visible",
        },

        // ===== RUINS REPLACEMENTS =====

        // Cyclopean Ruins - replaces forests/grassland
        ReplacementRule {
            target: ExtendedBiome::CyclopeanRuins,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
                ExtendedBiome::Desert,
            ],
            condition: ReplacementCondition {
                elevation_min: 0.0,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 3,
            description: "Massive ancient ruins",
        },

        // Buried Temple - replaces desert/jungle
        ReplacementRule {
            target: ExtendedBiome::BuriedTemple,
            replaces: vec![
                ExtendedBiome::Desert,
                ExtendedBiome::TropicalForest,
                ExtendedBiome::TropicalRainforest,
            ],
            condition: ReplacementCondition {
                ..Default::default()
            },
            chance: 0.006,
            cluster_size: 2,
            description: "Half-buried ancient temples",
        },

        // Overgrown Citadel - replaces forests
        ReplacementRule {
            target: ExtendedBiome::OvergrownCitadel,
            replaces: vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TropicalForest,
                ExtendedBiome::BorealForest,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.4,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 2,
            description: "Forest-covered ruined cities",
        },

        // ===== SPECIAL BIOME REPLACEMENTS =====

        // Colossal Hive - replaces savanna/grassland
        ReplacementRule {
            target: ExtendedBiome::ColossalHive,
            replaces: vec![
                ExtendedBiome::Savanna,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                temp_min: 15.0,
                moisture_min: 0.2,
                moisture_max: 0.5,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 4,
            description: "Giant insect hive structures",
        },

        // Sinkhole Lakes - replaces grassland/forest
        ReplacementRule {
            target: ExtendedBiome::SinkholeLakes,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TropicalForest,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.3,
                stress_min: 0.1,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 2,
            description: "Collapsed sinkhole lakes",
        },

        // Hot Springs - replaces tundra/grassland near volcanic
        ReplacementRule {
            target: ExtendedBiome::HotSprings,
            replaces: vec![
                ExtendedBiome::Tundra,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::BorealForest,
            ],
            condition: ReplacementCondition {
                stress_min: 0.15,
                moisture_min: 0.3,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 2,
            description: "Geothermal hot springs",
        },

        // Crystal Wasteland - replaces desert
        ReplacementRule {
            target: ExtendedBiome::CrystalWasteland,
            replaces: vec![ExtendedBiome::Desert, ExtendedBiome::SaltFlats],
            condition: ReplacementCondition {
                moisture_max: 0.2,
                stress_min: 0.15,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Crystal-studded wastelands",
        },

        // ===== OCEAN BIOME REPLACEMENTS =====
        // Coastal/Shallow zones (replace CoastalWater)

        // Coral Reef - warm shallow waters
        ReplacementRule {
            target: ExtendedBiome::CoralReef,
            replaces: vec![ExtendedBiome::CoastalWater],
            condition: ReplacementCondition {
                temp_min: 20.0,
                temp_max: 30.0,
                stress_max: 0.15,
                elevation_min: -100.0,
                elevation_max: -5.0,
                ..Default::default()
            },
            chance: 0.08,
            cluster_size: 10,
            description: "Tropical coral reef formations",
        },

        // Kelp Forest - temperate coastal waters
        ReplacementRule {
            target: ExtendedBiome::KelpForest,
            replaces: vec![ExtendedBiome::CoastalWater],
            condition: ReplacementCondition {
                temp_min: 8.0,
                temp_max: 18.0,
                elevation_min: -80.0,
                elevation_max: -10.0,
                ..Default::default()
            },
            chance: 0.06,
            cluster_size: 8,
            description: "Dense kelp forest beds",
        },

        // Seagrass Meadow - shallow warm waters
        ReplacementRule {
            target: ExtendedBiome::SeagrassMeadow,
            replaces: vec![ExtendedBiome::CoastalWater],
            condition: ReplacementCondition {
                temp_min: 15.0,
                elevation_min: -50.0,
                elevation_max: -2.0,
                ..Default::default()
            },
            chance: 0.07,
            cluster_size: 12,
            description: "Shallow seagrass beds",
        },

        // Pearl Gardens - fantasy coastal (warm, calm waters)
        ReplacementRule {
            target: ExtendedBiome::PearlGardens,
            replaces: vec![ExtendedBiome::CoastalWater],
            condition: ReplacementCondition {
                temp_min: 18.0,
                temp_max: 28.0,
                stress_max: 0.1,
                elevation_min: -120.0,
                elevation_max: -10.0,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 5,
            description: "Luminescent pearl-producing waters",
        },

        // Siren Shallows - fantasy enchanted coastal
        ReplacementRule {
            target: ExtendedBiome::SirenShallows,
            replaces: vec![ExtendedBiome::CoastalWater],
            condition: ReplacementCondition {
                temp_min: 15.0,
                temp_max: 28.0,
                elevation_min: -80.0,
                elevation_max: -5.0,
                ..Default::default()
            },
            chance: 0.012,
            cluster_size: 6,
            description: "Enchanted waters with hypnotic qualities",
        },

        // ===== MID-OCEAN REPLACEMENTS =====
        // Replace Ocean biome

        // Continental Shelf - shallow ocean floor
        ReplacementRule {
            target: ExtendedBiome::ContinentalShelf,
            replaces: vec![ExtendedBiome::Ocean],
            condition: ReplacementCondition {
                stress_max: 0.15,
                elevation_min: -300.0,
                elevation_max: -100.0,
                ..Default::default()
            },
            chance: 0.10,
            cluster_size: 15,
            description: "Flat continental shelf sediment",
        },

        // Seamount - underwater volcanic mountains
        ReplacementRule {
            target: ExtendedBiome::Seamount,
            replaces: vec![ExtendedBiome::Ocean],
            condition: ReplacementCondition {
                stress_min: 0.2,
                elevation_min: -1500.0,
                elevation_max: -300.0,
                ..Default::default()
            },
            chance: 0.04,
            cluster_size: 5,
            description: "Underwater volcanic seamounts",
        },

        // Drowned Citadel - fantasy sunken civilization
        ReplacementRule {
            target: ExtendedBiome::DrownedCitadel,
            replaces: vec![ExtendedBiome::Ocean, ExtendedBiome::ContinentalShelf],
            condition: ReplacementCondition {
                elevation_min: -500.0,
                elevation_max: -100.0,
                stress_max: 0.2,
                ..Default::default()
            },
            chance: 0.008,
            cluster_size: 4,
            description: "Massive sunken citadel ruins",
        },

        // Leviathan Graveyard - fantasy ocean bone fields (very rare)
        ReplacementRule {
            target: ExtendedBiome::LeviathanGraveyard,
            replaces: vec![ExtendedBiome::Ocean],
            condition: ReplacementCondition {
                temp_max: 5.0,              // Very cold waters only
                stress_max: 0.1,            // Very calm areas
                elevation_min: -1500.0,
                elevation_max: -600.0,      // Narrower depth range
                ..Default::default()
            },
            chance: 0.003,                  // Much rarer (was 0.012)
            cluster_size: 3,                // Smaller clusters
            description: "Ancient sea creature bone graveyards",
        },

        // ===== DEEP OCEAN REPLACEMENTS =====
        // Replace Ocean in deep areas

        // Oceanic Trench - ultra-deep subduction zones
        ReplacementRule {
            target: ExtendedBiome::OceanicTrench,
            replaces: vec![ExtendedBiome::DeepOcean],
            condition: ReplacementCondition {
                stress_min: 0.35,
                elevation_max: -3000.0,
                ..Default::default()
            },
            chance: 0.05,
            cluster_size: 8,
            description: "Ultra-deep oceanic trenches",
        },

        // Abyssal Plain - flat deep ocean floor
        ReplacementRule {
            target: ExtendedBiome::AbyssalPlain,
            replaces: vec![ExtendedBiome::DeepOcean],
            condition: ReplacementCondition {
                stress_max: 0.15,
                elevation_min: -5500.0,
                elevation_max: -2500.0,
                ..Default::default()
            },
            chance: 0.12,
            cluster_size: 20,
            description: "Flat abyssal ocean plains",
        },

        // Mid-Ocean Ridge - divergent plate boundaries
        ReplacementRule {
            target: ExtendedBiome::MidOceanRidge,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::Ocean],
            condition: ReplacementCondition {
                stress_min: -0.3,  // Divergent (negative) stress
                stress_max: -0.1,
                elevation_min: -4000.0,
                elevation_max: -1500.0,
                ..Default::default()
            },
            chance: 0.06,
            cluster_size: 10,
            description: "Mid-ocean spreading ridges",
        },

        // Cold Seep - methane seepage areas
        ReplacementRule {
            target: ExtendedBiome::ColdSeep,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::AbyssalPlain],
            condition: ReplacementCondition {
                temp_max: 6.0,
                stress_min: 0.1,
                stress_max: 0.3,
                elevation_max: -1000.0,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Methane-seeping ocean floor",
        },

        // Brine Pool - hypersaline underwater lakes
        ReplacementRule {
            target: ExtendedBiome::BrinePool,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::AbyssalPlain],
            condition: ReplacementCondition {
                temp_max: 4.0,
                elevation_max: -2000.0,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 3,
            description: "Hypersaline brine pools",
        },

        // Crystal Depths - fantasy deep crystal formations
        ReplacementRule {
            target: ExtendedBiome::CrystalDepths,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::AbyssalPlain],
            condition: ReplacementCondition {
                temp_max: 8.0,
                stress_min: 0.2,
                elevation_max: -1500.0,
                ..Default::default()
            },
            chance: 0.01,
            cluster_size: 5,
            description: "Magical crystalline deep formations",
        },

        // Void Maw - fantasy reality-torn abyssal holes
        ReplacementRule {
            target: ExtendedBiome::VoidMaw,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::OceanicTrench],
            condition: ReplacementCondition {
                stress_min: 0.5,
                elevation_max: -2500.0,
                ..Default::default()
            },
            chance: 0.006,
            cluster_size: 3,
            description: "Reality-torn abyssal voids",
        },

        // Frozen Abyss - polar deep ocean (rare fantasy)
        ReplacementRule {
            target: ExtendedBiome::FrozenAbyss,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::Ocean],
            condition: ReplacementCondition {
                temp_max: -10.0,            // Very cold polar waters only
                elevation_max: -800.0,
                elevation_min: -2500.0,
                ..Default::default()
            },
            chance: 0.008,                  // Much rarer (was 0.04)
            cluster_size: 5,                // Smaller clusters
            description: "Frozen polar deep waters",
        },

        // Thermal Vents - deep sea hydrothermal vents
        ReplacementRule {
            target: ExtendedBiome::ThermalVents,
            replaces: vec![ExtendedBiome::DeepOcean, ExtendedBiome::Ocean, ExtendedBiome::MidOceanRidge],
            condition: ReplacementCondition {
                stress_min: 0.25,
                elevation_max: -1500.0,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 4,
            description: "Hydrothermal vent fields",
        },

        // ===== TRANSITIONAL TERRAIN =====

        // Foothills - rolling hills at mountain bases, transitional terrain
        ReplacementRule {
            target: ExtendedBiome::Foothills,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
                ExtendedBiome::Tundra,
            ],
            condition: ReplacementCondition {
                elevation_min: 150.0,      // Above lowlands
                elevation_max: 850.0,      // Below true mountains
                stress_min: 0.06,          // Near tectonic activity (mountain-building)
                ..Default::default()
            },
            chance: 0.20,                  // Fairly common near mountains
            cluster_size: 18,              // Large transitional zones
            description: "Rolling foothills at mountain bases",
        },

        // ===== COASTAL FEATURES =====

        // Lagoon - shallow protected waters (behind barrier islands, in bays)
        ReplacementRule {
            target: ExtendedBiome::Lagoon,
            replaces: vec![ExtendedBiome::CoastalWater, ExtendedBiome::Ocean],
            condition: ReplacementCondition {
                elevation_min: -25.0,      // Very shallow water
                elevation_max: -1.0,       // Not quite at sea level
                temp_min: 12.0,            // Warmer waters
                ..Default::default()
            },
            chance: 0.12,                  // Moderate occurrence
            cluster_size: 10,              // Medium-sized protected areas
            description: "Shallow protected lagoons",
        },

        // ===== KARST & CAVE SYSTEMS =====

        // Karst Plains - limestone terrain with dissolution features
        // Forms in wet temperate/tropical areas with carbonate bedrock
        ReplacementRule {
            target: ExtendedBiome::KarstPlains,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::TemperateForest,
                ExtendedBiome::Savanna,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.4,         // Needs water for dissolution
                temp_min: 8.0,             // Not too cold
                elevation_min: 50.0,       // Above sea level
                elevation_max: 600.0,      // Low to moderate elevation
                ..Default::default()
            },
            chance: 0.08,                  // Moderate occurrence
            cluster_size: 20,              // Large karst regions
            description: "Limestone karst plains",
        },

        // Tower Karst - dramatic limestone pillars (tropical karst)
        // Forms in hot, wet tropical climates - like Guilin, Halong Bay
        ReplacementRule {
            target: ExtendedBiome::TowerKarst,
            replaces: vec![
                ExtendedBiome::TropicalForest,
                ExtendedBiome::TropicalRainforest,
                ExtendedBiome::KarstPlains,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.6,         // High moisture needed
                temp_min: 20.0,            // Tropical temperatures
                elevation_min: 100.0,      // Hills/plateaus
                elevation_max: 500.0,      // Not mountain-top
                ..Default::default()
            },
            chance: 0.04,                  // Rare
            cluster_size: 12,              // Medium clusters
            description: "Tower karst formations",
        },

        // Cockpit Karst - star-shaped depressions between mogotes
        // Typical of Jamaica, Puerto Rico - transition from tower karst
        ReplacementRule {
            target: ExtendedBiome::CockpitKarst,
            replaces: vec![
                ExtendedBiome::TropicalForest,
                ExtendedBiome::KarstPlains,
                ExtendedBiome::TowerKarst,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.55,
                temp_min: 18.0,
                elevation_min: 150.0,
                elevation_max: 450.0,
                ..Default::default()
            },
            chance: 0.03,                  // Rare
            cluster_size: 10,
            description: "Cockpit karst with mogotes",
        },

        // Sinkholes - collapsed caves/dolines scattered in karst areas
        ReplacementRule {
            target: ExtendedBiome::Sinkhole,
            replaces: vec![
                ExtendedBiome::KarstPlains,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::TemperateForest,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.35,
                elevation_min: 30.0,
                elevation_max: 500.0,
                ..Default::default()
            },
            chance: 0.015,                 // Sparse
            cluster_size: 3,               // Small isolated features
            description: "Collapse sinkholes",
        },

        // Cenotes - water-filled sinkholes (tropical)
        // Like the Yucatan Peninsula
        ReplacementRule {
            target: ExtendedBiome::Cenote,
            replaces: vec![
                ExtendedBiome::KarstPlains,
                ExtendedBiome::Sinkhole,
                ExtendedBiome::TropicalForest,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.4,
                temp_min: 20.0,            // Tropical
                elevation_min: 10.0,       // Near sea level
                elevation_max: 200.0,      // Low elevation
                ..Default::default()
            },
            chance: 0.012,                 // Rare
            cluster_size: 2,               // Usually isolated
            description: "Water-filled cenotes",
        },

        // Cave Entrances - surface openings to cave systems
        ReplacementRule {
            target: ExtendedBiome::CaveEntrance,
            replaces: vec![
                ExtendedBiome::KarstPlains,
                ExtendedBiome::Sinkhole,
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::Foothills,
            ],
            condition: ReplacementCondition {
                moisture_min: 0.3,
                elevation_min: 100.0,
                elevation_max: 1200.0,     // Can be at moderate elevation
                ..Default::default()
            },
            chance: 0.008,                 // Very rare
            cluster_size: 1,               // Single entrances
            description: "Cave system entrances",
        },

        // ===== VOLCANIC FEATURES =====

        // Caldera - large volcanic crater from collapsed magma chamber
        // Forms at high stress areas (major volcanic centers)
        ReplacementRule {
            target: ExtendedBiome::Caldera,
            replaces: vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Foothills,
            ],
            condition: ReplacementCondition {
                stress_min: 0.2,           // Lowered for more occurrence
                elevation_min: 500.0,      // Mountain terrain
                ..Default::default()
            },
            chance: 0.03,                  // Slightly more common
            cluster_size: 10,              // Large crater
            description: "Volcanic caldera crater",
        },

        // Shield Volcano - broad, gently sloping volcanic terrain
        // Forms at hot spots and divergent boundaries (like Hawaii, Iceland)
        ReplacementRule {
            target: ExtendedBiome::ShieldVolcano,
            replaces: vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
                ExtendedBiome::TropicalForest,
                ExtendedBiome::Foothills,
            ],
            condition: ReplacementCondition {
                stress_min: 0.08,          // Lower threshold for more coverage
                elevation_min: 100.0,
                elevation_max: 1500.0,
                ..Default::default()
            },
            chance: 0.05,                  // More common
            cluster_size: 18,              // Broad volcanic terrain
            description: "Shield volcano slopes",
        },

        // Volcanic Cone - classic stratovolcano peak
        // Forms at high stress convergent boundaries
        ReplacementRule {
            target: ExtendedBiome::VolcanicCone,
            replaces: vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::Foothills,
                ExtendedBiome::ShieldVolcano,
            ],
            condition: ReplacementCondition {
                stress_min: 0.15,          // Moderate stress
                elevation_min: 400.0,      // Mountain terrain
                ..Default::default()
            },
            chance: 0.04,                  // More common
            cluster_size: 6,               // Conical peak
            description: "Stratovolcano cone",
        },

        // Lava Field - solidified basalt flows
        // Forms near volcanic centers
        ReplacementRule {
            target: ExtendedBiome::LavaField,
            replaces: vec![
                ExtendedBiome::ShieldVolcano,
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Desert,
                ExtendedBiome::Ashlands,
                ExtendedBiome::TemperateGrassland,
            ],
            condition: ReplacementCondition {
                stress_min: 0.12,          // Lower threshold
                elevation_min: 50.0,
                elevation_max: 1200.0,
                ..Default::default()
            },
            chance: 0.06,                  // More common
            cluster_size: 14,              // Spreading lava flows
            description: "Basalt lava fields",
        },

        // Fumarole Field - steam vents and sulfurous terrain
        // Near active volcanic areas
        ReplacementRule {
            target: ExtendedBiome::FumaroleField,
            replaces: vec![
                ExtendedBiome::Caldera,
                ExtendedBiome::VolcanicCone,
                ExtendedBiome::ShieldVolcano,
                ExtendedBiome::AlpineTundra,
            ],
            condition: ReplacementCondition {
                stress_min: 0.3,           // High volcanic activity
                elevation_min: 400.0,
                ..Default::default()
            },
            chance: 0.015,
            cluster_size: 4,
            description: "Volcanic fumarole vents",
        },

        // Volcanic Beach - black sand beaches
        // Coastal areas near volcanic terrain
        ReplacementRule {
            target: ExtendedBiome::VolcanicBeach,
            replaces: vec![
                ExtendedBiome::CoastalWater,
                ExtendedBiome::Savanna,
                ExtendedBiome::TropicalForest,
            ],
            condition: ReplacementCondition {
                stress_min: 0.15,          // Near volcanic activity
                elevation_min: -5.0,       // At/near sea level
                elevation_max: 30.0,
                ..Default::default()
            },
            chance: 0.02,
            cluster_size: 6,
            description: "Black volcanic sand beaches",
        },

        // Hot Spot - active volcanic hot spot area
        // Can form anywhere (simulating mantle plumes)
        ReplacementRule {
            target: ExtendedBiome::HotSpot,
            replaces: vec![
                ExtendedBiome::ShieldVolcano,
                ExtendedBiome::LavaField,
                ExtendedBiome::VolcanicWasteland,
            ],
            condition: ReplacementCondition {
                stress_min: 0.25,
                elevation_min: 50.0,
                ..Default::default()
            },
            chance: 0.01,                  // Rare
            cluster_size: 3,               // Small active area
            description: "Active volcanic hot spot",
        },
    ]
}

/// Apply biome replacement rules to transform common biomes into rare ones.
/// This creates natural-feeling clusters of rare biomes.
pub fn apply_biome_replacements(
    biomes: &mut Tilemap<ExtendedBiome>,
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> usize {
    use rand::SeedableRng;
    use rand::Rng;
    use rand_chacha::ChaCha8Rng;

    let width = biomes.width;
    let height = biomes.height;
    let rules = get_replacement_rules();

    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xB10E));
    let mut replacement_count = 0;

    // Create a noise generator for clustering
    let cluster_noise = Perlin::new(1).set_seed(seed as u32 + 12345);

    // Track which tiles have been replaced (to avoid double-replacement)
    let mut replaced = Tilemap::new_with(width, height, false);

    // Apply each rule
    for rule in &rules {
        for y in 0..height {
            for x in 0..width {
                // Skip already replaced tiles
                if *replaced.get(x, y) {
                    continue;
                }

                let current_biome = *biomes.get(x, y);

                // Check if this biome can be replaced by this rule
                if !rule.replaces.contains(&current_biome) {
                    continue;
                }

                // Check conditions
                let elev = *heightmap.get(x, y);
                let temp = *temperature.get(x, y);
                let moist = *moisture.get(x, y);
                let stress = *stress_map.get(x, y);

                if !rule.condition.matches(temp, moist, stress, elev) {
                    continue;
                }

                // Check chance (modified by noise for clustering)
                let noise_val = cluster_noise.get([x as f64 * 0.1, y as f64 * 0.1]) as f32;
                let cluster_bonus = (noise_val + 1.0) * 0.5; // 0-1 range
                let effective_chance = rule.chance * (0.5 + cluster_bonus);

                if rng.gen::<f32>() > effective_chance {
                    continue;
                }

                // Apply replacement with clustering
                let cluster_size = rule.cluster_size.max(1);
                apply_cluster(
                    biomes,
                    &mut replaced,
                    x, y,
                    rule.target,
                    &rule.replaces,
                    cluster_size,
                    heightmap,
                    temperature,
                    moisture,
                    stress_map,
                    &rule.condition,
                    &mut rng,
                );
                replacement_count += 1;
            }
        }
    }

    replacement_count
}

/// Apply a cluster of replacement biomes around a seed point
fn apply_cluster(
    biomes: &mut Tilemap<ExtendedBiome>,
    replaced: &mut Tilemap<bool>,
    start_x: usize,
    start_y: usize,
    target: ExtendedBiome,
    source_biomes: &[ExtendedBiome],
    max_size: usize,
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    condition: &ReplacementCondition,
    rng: &mut impl rand::Rng,
) {
    use std::collections::VecDeque;

    let width = biomes.width;
    let height = biomes.height;

    let mut queue = VecDeque::new();
    queue.push_back((start_x, start_y));

    let mut placed = 0;

    while let Some((x, y)) = queue.pop_front() {
        if placed >= max_size {
            break;
        }

        if *replaced.get(x, y) {
            continue;
        }

        let current = *biomes.get(x, y);
        if !source_biomes.contains(&current) {
            continue;
        }

        // Check conditions still match
        let elev = *heightmap.get(x, y);
        let temp = *temperature.get(x, y);
        let moist = *moisture.get(x, y);
        let stress = *stress_map.get(x, y);

        if !condition.matches(temp, moist, stress, elev) {
            continue;
        }

        // Place the biome
        biomes.set(x, y, target);
        replaced.set(x, y, true);
        placed += 1;

        // Add neighbors with decreasing probability
        let neighbors = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];

        for (nx, ny) in neighbors {
            if nx < width && ny < height && !*replaced.get(nx, ny) {
                // Higher chance to expand in the beginning
                let expand_chance = 0.7 - (placed as f32 / max_size as f32) * 0.4;
                if rng.gen::<f32>() < expand_chance {
                    queue.push_back((nx, ny));
                }
            }
        }
    }
}

// ============================================================================
// UNIQUE BIOME PLACEMENT
// ============================================================================
// Some biomes appear only once per map (e.g., Dark Tower).

/// Place unique biomes (exactly one per map).
/// Returns the number of unique biomes placed.
pub fn place_unique_biomes(
    biomes: &mut Tilemap<ExtendedBiome>,
    heightmap: &Tilemap<f32>,
    seed: u64,
) -> usize {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xDA4C_70BE));
    let mut placed_count = 0;

    // Place the Dark Tower
    if place_dark_tower(biomes, heightmap, &mut rng) {
        placed_count += 1;
    }

    placed_count
}

/// Place exactly one Dark Tower on the map.
/// The Dark Tower appears on high ground, preferring dramatic locations.
fn place_dark_tower(
    biomes: &mut Tilemap<ExtendedBiome>,
    heightmap: &Tilemap<f32>,
    rng: &mut impl rand::Rng,
) -> bool {
    let width = biomes.width;
    let height = biomes.height;

    // Collect valid candidates for the Dark Tower location
    // Must be: land (elevation > 0), high elevation preferred
    // Preferably near or on ruins, mountains, or desolate areas
    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let biome = *biomes.get(x, y);

            // Must be land
            if elev <= 0.0 {
                continue;
            }

            // Calculate suitability score
            let mut score: f32 = 0.0;

            // Higher elevation is better (towers on high ground)
            score += (elev / 1000.0).min(2.0);

            // Bonus for certain biomes that make thematic sense
            match biome {
                // Ancient ruins - perfect location
                ExtendedBiome::CyclopeanRuins |
                ExtendedBiome::BuriedTemple |
                ExtendedBiome::OvergrownCitadel => score += 3.0,

                // Mountain/alpine areas - dramatic locations
                ExtendedBiome::AlpineTundra |
                ExtendedBiome::SnowyPeaks |
                ExtendedBiome::RazorPeaks => score += 2.5,

                // Wastelands and desolate areas
                ExtendedBiome::VolcanicWasteland |
                ExtendedBiome::Ashlands |
                ExtendedBiome::BoneFields |
                ExtendedBiome::VoidScar => score += 2.0,

                // Other interesting locations
                ExtendedBiome::WhisperingStones |
                ExtendedBiome::StarfallCrater |
                ExtendedBiome::LeyNexus => score += 1.5,

                // Tundra and sparse lands work
                ExtendedBiome::Tundra |
                ExtendedBiome::Desert |
                ExtendedBiome::Savanna => score += 0.5,

                // Forests and other biomes are less ideal but possible
                _ => score += 0.1,
            }

            // Only consider tiles with reasonable scores
            if score > 0.5 {
                candidates.push((x, y, score));
            }
        }
    }

    if candidates.is_empty() {
        return false;
    }

    // Weight selection by score - higher scores more likely
    let total_score: f32 = candidates.iter().map(|(_, _, s)| s).sum();

    let mut pick = rng.gen::<f32>() * total_score;
    let mut chosen = candidates.last().unwrap();

    for candidate in &candidates {
        pick -= candidate.2;
        if pick <= 0.0 {
            chosen = candidate;
            break;
        }
    }

    let (x, y, _) = *chosen;
    biomes.set(x, y, ExtendedBiome::DarkTower);
    true
}
