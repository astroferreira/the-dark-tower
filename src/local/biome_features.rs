//! Biome feature configurations for local map generation.
//!
//! Maps each ExtendedBiome to terrain types and feature probabilities.

use crate::biomes::ExtendedBiome;
use super::terrain::{LocalTerrainType, LocalFeature};

/// Configuration for generating a local map from a biome
#[derive(Clone, Debug)]
pub struct BiomeFeatureConfig {
    /// Primary terrain type
    pub primary_terrain: LocalTerrainType,
    /// Secondary terrain type (for variety)
    pub secondary_terrain: Option<LocalTerrainType>,
    /// Probability of secondary terrain (0.0 - 1.0)
    pub secondary_chance: f32,
    /// List of (feature, probability) pairs
    pub features: Vec<(LocalFeature, f32)>,
    /// Chance of water features (ponds, streams)
    pub water_chance: f32,
    /// Whether this biome can have cave entrances
    pub can_have_caves: bool,
}

impl Default for BiomeFeatureConfig {
    fn default() -> Self {
        Self {
            primary_terrain: LocalTerrainType::Grass,
            secondary_terrain: None,
            secondary_chance: 0.0,
            features: Vec::new(),
            water_chance: 0.0,
            can_have_caves: false,
        }
    }
}

impl BiomeFeatureConfig {
    /// Create a new config with primary terrain
    pub fn new(primary: LocalTerrainType) -> Self {
        Self {
            primary_terrain: primary,
            ..Default::default()
        }
    }

    /// Add secondary terrain
    pub fn with_secondary(mut self, terrain: LocalTerrainType, chance: f32) -> Self {
        self.secondary_terrain = Some(terrain);
        self.secondary_chance = chance;
        self
    }

    /// Add a feature with probability
    pub fn with_feature(mut self, feature: LocalFeature, chance: f32) -> Self {
        self.features.push((feature, chance));
        self
    }

    /// Set water chance
    pub fn with_water(mut self, chance: f32) -> Self {
        self.water_chance = chance;
        self
    }

    /// Enable caves
    pub fn with_caves(mut self) -> Self {
        self.can_have_caves = true;
        self
    }

    /// Get total feature density (sum of all feature probabilities)
    pub fn total_feature_density(&self) -> f32 {
        self.features.iter().map(|(_, p)| p).sum()
    }
}

/// Get the feature configuration for a given biome
pub fn get_biome_features(biome: ExtendedBiome) -> BiomeFeatureConfig {
    match biome {
        // ========== BASE BIOMES ==========

        // Ocean biomes
        ExtendedBiome::DeepOcean => BiomeFeatureConfig::new(LocalTerrainType::DeepWater),

        ExtendedBiome::Ocean => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::ShallowWater, 0.1),

        ExtendedBiome::CoastalWater => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Sand, 0.15)
            .with_feature(LocalFeature::RockPile, 0.03),

        // Cold biomes
        ExtendedBiome::Ice => BiomeFeatureConfig::new(LocalTerrainType::Ice)
            .with_secondary(LocalTerrainType::Snow, 0.2)
            .with_feature(LocalFeature::IceFormation, 0.08),

        ExtendedBiome::Tundra => BiomeFeatureConfig::new(LocalTerrainType::FrozenGround)
            .with_secondary(LocalTerrainType::Snow, 0.3)
            .with_feature(LocalFeature::Boulder, 0.05)
            .with_feature(LocalFeature::RockPile, 0.03)
            .with_water(0.05),

        ExtendedBiome::BorealForest => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Snow, 0.15)
            .with_feature(LocalFeature::ConiferTree, 0.35)
            .with_feature(LocalFeature::Bush, 0.08)
            .with_feature(LocalFeature::MushroomPatch, 0.04)
            .with_water(0.08)
            .with_caves(),

        // Temperate biomes
        ExtendedBiome::TemperateGrassland => BiomeFeatureConfig::new(LocalTerrainType::Grass)
            .with_secondary(LocalTerrainType::TallGrass, 0.3)
            .with_feature(LocalFeature::FlowerPatch, 0.08)
            .with_feature(LocalFeature::Bush, 0.03)
            .with_feature(LocalFeature::Boulder, 0.02)
            .with_water(0.05),

        ExtendedBiome::TemperateForest => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Grass, 0.15)
            .with_feature(LocalFeature::DeciduousTree, 0.35)
            .with_feature(LocalFeature::Bush, 0.15)
            .with_feature(LocalFeature::Fern, 0.08)
            .with_feature(LocalFeature::FlowerPatch, 0.05)
            .with_feature(LocalFeature::MushroomPatch, 0.04)
            .with_water(0.08)
            .with_caves(),

        ExtendedBiome::TemperateRainforest => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Mud, 0.1)
            .with_feature(LocalFeature::DeciduousTree, 0.40)
            .with_feature(LocalFeature::Fern, 0.15)
            .with_feature(LocalFeature::VineTangle, 0.08)
            .with_feature(LocalFeature::MushroomPatch, 0.06)
            .with_water(0.15)
            .with_caves(),

        // Warm biomes
        ExtendedBiome::Desert => BiomeFeatureConfig::new(LocalTerrainType::Sand)
            .with_secondary(LocalTerrainType::Gravel, 0.1)
            .with_feature(LocalFeature::Cactus, 0.05)
            .with_feature(LocalFeature::RockPile, 0.03)
            .with_feature(LocalFeature::Boulder, 0.02)
            .with_feature(LocalFeature::BoneRemains, 0.01),

        ExtendedBiome::Savanna => BiomeFeatureConfig::new(LocalTerrainType::TallGrass)
            .with_secondary(LocalTerrainType::Dirt, 0.2)
            .with_feature(LocalFeature::DeciduousTree, 0.05)
            .with_feature(LocalFeature::Bush, 0.08)
            .with_feature(LocalFeature::Boulder, 0.02)
            .with_water(0.03),

        ExtendedBiome::TropicalForest => BiomeFeatureConfig::new(LocalTerrainType::JungleFloor)
            .with_secondary(LocalTerrainType::Mud, 0.1)
            .with_feature(LocalFeature::JungleTree, 0.30)
            .with_feature(LocalFeature::PalmTree, 0.10)
            .with_feature(LocalFeature::Fern, 0.12)
            .with_feature(LocalFeature::VineTangle, 0.08)
            .with_feature(LocalFeature::FlowerPatch, 0.05)
            .with_water(0.10)
            .with_caves(),

        ExtendedBiome::TropicalRainforest => BiomeFeatureConfig::new(LocalTerrainType::JungleFloor)
            .with_secondary(LocalTerrainType::Mud, 0.15)
            .with_feature(LocalFeature::JungleTree, 0.40)
            .with_feature(LocalFeature::PalmTree, 0.08)
            .with_feature(LocalFeature::Fern, 0.15)
            .with_feature(LocalFeature::VineTangle, 0.12)
            .with_feature(LocalFeature::FlowerPatch, 0.06)
            .with_feature(LocalFeature::MushroomPatch, 0.05)
            .with_water(0.15)
            .with_caves(),

        // Mountain biomes
        ExtendedBiome::AlpineTundra => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.3)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_feature(LocalFeature::RockPile, 0.08)
            .with_feature(LocalFeature::Bush, 0.03)
            .with_caves(),

        ExtendedBiome::SnowyPeaks => BiomeFeatureConfig::new(LocalTerrainType::Snow)
            .with_secondary(LocalTerrainType::Ice, 0.2)
            .with_feature(LocalFeature::IceFormation, 0.10)
            .with_feature(LocalFeature::Boulder, 0.05)
            .with_caves(),

        ExtendedBiome::Foothills => BiomeFeatureConfig::new(LocalTerrainType::Grass)
            .with_secondary(LocalTerrainType::Stone, 0.15)
            .with_feature(LocalFeature::Boulder, 0.08)
            .with_feature(LocalFeature::Bush, 0.10)
            .with_feature(LocalFeature::DeciduousTree, 0.08)
            .with_water(0.05)
            .with_caves(),

        ExtendedBiome::Lagoon => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Sand, 0.2)
            .with_feature(LocalFeature::TallReeds, 0.10)
            .with_feature(LocalFeature::RockPile, 0.03),

        // ========== FANTASY FORESTS ==========

        ExtendedBiome::DeadForest => BiomeFeatureConfig::new(LocalTerrainType::Dirt)
            .with_secondary(LocalTerrainType::Ash, 0.2)
            .with_feature(LocalFeature::DeadTree, 0.30)
            .with_feature(LocalFeature::BoneRemains, 0.05)
            .with_feature(LocalFeature::RockPile, 0.04)
            .with_caves(),

        ExtendedBiome::CrystalForest => BiomeFeatureConfig::new(LocalTerrainType::CrystalGround)
            .with_secondary(LocalTerrainType::Stone, 0.15)
            .with_feature(LocalFeature::CrystalCluster, 0.25)
            .with_feature(LocalFeature::CrystalFlower, 0.15)
            .with_feature(LocalFeature::GlowingMoss, 0.08)
            .with_caves(),

        ExtendedBiome::BioluminescentForest => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Mud, 0.1)
            .with_feature(LocalFeature::JungleTree, 0.30)
            .with_feature(LocalFeature::GlowingMoss, 0.20)
            .with_feature(LocalFeature::MushroomPatch, 0.15)
            .with_feature(LocalFeature::Fern, 0.08)
            .with_water(0.10)
            .with_caves(),

        ExtendedBiome::MushroomForest => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Mud, 0.15)
            .with_feature(LocalFeature::MushroomPatch, 0.35)
            .with_feature(LocalFeature::GlowingMoss, 0.12)
            .with_feature(LocalFeature::Fern, 0.08)
            .with_water(0.12)
            .with_caves(),

        ExtendedBiome::PetrifiedForest => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.2)
            .with_feature(LocalFeature::DeadTree, 0.25)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_feature(LocalFeature::RockPile, 0.08)
            .with_caves(),

        // ========== FANTASY WATERS ==========

        ExtendedBiome::AcidLake => BiomeFeatureConfig::new(LocalTerrainType::AcidPool)
            .with_secondary(LocalTerrainType::Stone, 0.15)
            .with_feature(LocalFeature::RockPile, 0.05)
            .with_feature(LocalFeature::BoneRemains, 0.03),

        ExtendedBiome::LavaLake => BiomeFeatureConfig::new(LocalTerrainType::Lava)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.2)
            .with_feature(LocalFeature::Boulder, 0.05),

        ExtendedBiome::FrozenLake => BiomeFeatureConfig::new(LocalTerrainType::Ice)
            .with_secondary(LocalTerrainType::Snow, 0.15)
            .with_feature(LocalFeature::IceFormation, 0.12),

        ExtendedBiome::BioluminescentWater => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::DeepWater, 0.3)
            .with_feature(LocalFeature::GlowingMoss, 0.10)
            .with_feature(LocalFeature::TallReeds, 0.05),

        // ========== WASTELANDS ==========

        ExtendedBiome::VolcanicWasteland => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Ash, 0.25)
            .with_feature(LocalFeature::Boulder, 0.08)
            .with_feature(LocalFeature::RockPile, 0.06)
            .with_feature(LocalFeature::Geyser, 0.02)
            .with_caves(),

        ExtendedBiome::SaltFlats => BiomeFeatureConfig::new(LocalTerrainType::Salt)
            .with_secondary(LocalTerrainType::Gravel, 0.1)
            .with_feature(LocalFeature::RockPile, 0.02)
            .with_feature(LocalFeature::BoneRemains, 0.01),

        ExtendedBiome::Ashlands => BiomeFeatureConfig::new(LocalTerrainType::Ash)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.2)
            .with_feature(LocalFeature::DeadTree, 0.08)
            .with_feature(LocalFeature::Boulder, 0.05)
            .with_feature(LocalFeature::BoneRemains, 0.03)
            .with_caves(),

        ExtendedBiome::CrystalWasteland => BiomeFeatureConfig::new(LocalTerrainType::CrystalGround)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.15)
            .with_feature(LocalFeature::Boulder, 0.05),

        // ========== WETLANDS ==========

        ExtendedBiome::Swamp => BiomeFeatureConfig::new(LocalTerrainType::Marsh)
            .with_secondary(LocalTerrainType::Mud, 0.25)
            .with_feature(LocalFeature::WillowTree, 0.15)
            .with_feature(LocalFeature::DeadTree, 0.08)
            .with_feature(LocalFeature::TallReeds, 0.20)
            .with_feature(LocalFeature::MushroomPatch, 0.06)
            .with_water(0.25)
            .with_caves(),

        ExtendedBiome::Marsh => BiomeFeatureConfig::new(LocalTerrainType::Marsh)
            .with_secondary(LocalTerrainType::ShallowWater, 0.2)
            .with_feature(LocalFeature::TallReeds, 0.25)
            .with_feature(LocalFeature::Bush, 0.05)
            .with_water(0.30),

        ExtendedBiome::Bog => BiomeFeatureConfig::new(LocalTerrainType::Mud)
            .with_secondary(LocalTerrainType::Marsh, 0.25)
            .with_feature(LocalFeature::DeadTree, 0.10)
            .with_feature(LocalFeature::TallReeds, 0.12)
            .with_feature(LocalFeature::MushroomPatch, 0.08)
            .with_water(0.20)
            .with_caves(),

        ExtendedBiome::MangroveSaltmarsh => BiomeFeatureConfig::new(LocalTerrainType::Marsh)
            .with_secondary(LocalTerrainType::ShallowWater, 0.3)
            .with_feature(LocalFeature::JungleTree, 0.20)
            .with_feature(LocalFeature::TallReeds, 0.15)
            .with_water(0.35),

        // ========== ULTRA-RARE - ANCIENT/PRIMEVAL ==========

        ExtendedBiome::AncientGrove => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_feature(LocalFeature::DeciduousTree, 0.40)
            .with_feature(LocalFeature::GlowingMoss, 0.15)
            .with_feature(LocalFeature::Fern, 0.12)
            .with_feature(LocalFeature::Shrine, 0.02)
            .with_feature(LocalFeature::AncientMonolith, 0.01)
            .with_water(0.08)
            .with_caves(),

        ExtendedBiome::TitanBones => BiomeFeatureConfig::new(LocalTerrainType::Bone)
            .with_secondary(LocalTerrainType::Dirt, 0.3)
            .with_feature(LocalFeature::BoneRemains, 0.20)
            .with_feature(LocalFeature::Boulder, 0.05)
            .with_caves(),

        ExtendedBiome::CoralPlateau => BiomeFeatureConfig::new(LocalTerrainType::Coral)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.10)
            .with_feature(LocalFeature::Spring, 0.05)
            .with_water(0.20),

        // ========== ULTRA-RARE - GEOTHERMAL/VOLCANIC ==========

        ExtendedBiome::ObsidianFields => BiomeFeatureConfig::new(LocalTerrainType::Obsidian)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.08)
            .with_feature(LocalFeature::Boulder, 0.06)
            .with_caves(),

        ExtendedBiome::Geysers => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.2)
            .with_feature(LocalFeature::Geyser, 0.15)
            .with_feature(LocalFeature::Spring, 0.10)
            .with_feature(LocalFeature::RockPile, 0.05)
            .with_water(0.15),

        ExtendedBiome::TarPits => BiomeFeatureConfig::new(LocalTerrainType::Mud)
            .with_secondary(LocalTerrainType::Dirt, 0.2)
            .with_feature(LocalFeature::BoneRemains, 0.10)
            .with_feature(LocalFeature::DeadTree, 0.05),

        // ========== ULTRA-RARE - MAGICAL/ANOMALOUS ==========

        ExtendedBiome::FloatingStones => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::CrystalGround, 0.2)
            .with_feature(LocalFeature::Boulder, 0.15)
            .with_feature(LocalFeature::CrystalCluster, 0.10)
            .with_feature(LocalFeature::AncientMonolith, 0.03),

        ExtendedBiome::Shadowfen => BiomeFeatureConfig::new(LocalTerrainType::Marsh)
            .with_secondary(LocalTerrainType::Mud, 0.3)
            .with_feature(LocalFeature::DeadTree, 0.20)
            .with_feature(LocalFeature::TallReeds, 0.12)
            .with_feature(LocalFeature::GlowingMoss, 0.08)
            .with_water(0.25)
            .with_caves(),

        ExtendedBiome::PrismaticPools => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::CrystalGround, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.15)
            .with_feature(LocalFeature::CrystalFlower, 0.12)
            .with_feature(LocalFeature::Spring, 0.08)
            .with_water(0.40),

        ExtendedBiome::AuroraWastes => BiomeFeatureConfig::new(LocalTerrainType::Snow)
            .with_secondary(LocalTerrainType::Ice, 0.25)
            .with_feature(LocalFeature::IceFormation, 0.12)
            .with_feature(LocalFeature::CrystalCluster, 0.08)
            .with_feature(LocalFeature::GlowingMoss, 0.05),

        // ========== ULTRA-RARE - DESERT VARIANTS ==========

        ExtendedBiome::SingingDunes => BiomeFeatureConfig::new(LocalTerrainType::Sand)
            .with_feature(LocalFeature::RockPile, 0.03)
            .with_feature(LocalFeature::BoneRemains, 0.02),

        ExtendedBiome::Oasis => BiomeFeatureConfig::new(LocalTerrainType::Grass)
            .with_secondary(LocalTerrainType::Sand, 0.2)
            .with_feature(LocalFeature::PalmTree, 0.20)
            .with_feature(LocalFeature::Bush, 0.10)
            .with_feature(LocalFeature::FlowerPatch, 0.08)
            .with_feature(LocalFeature::Pond, 0.15)
            .with_water(0.30),

        ExtendedBiome::GlassDesert => BiomeFeatureConfig::new(LocalTerrainType::CrystalGround)
            .with_secondary(LocalTerrainType::Sand, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.10),

        // ========== ULTRA-RARE - AQUATIC ==========

        ExtendedBiome::AbyssalVents => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.15)
            .with_feature(LocalFeature::Geyser, 0.08),

        ExtendedBiome::Sargasso => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::DeepWater, 0.3)
            .with_feature(LocalFeature::TallReeds, 0.25)
            .with_feature(LocalFeature::VineTangle, 0.15),

        // ========== NEW BIOMES - MYSTICAL/SUPERNATURAL ==========

        ExtendedBiome::EtherealMist => BiomeFeatureConfig::new(LocalTerrainType::Grass)
            .with_secondary(LocalTerrainType::Stone, 0.15)
            .with_feature(LocalFeature::GlowingMoss, 0.15)
            .with_feature(LocalFeature::CrystalFlower, 0.08)
            .with_feature(LocalFeature::Shrine, 0.02)
            .with_water(0.10),

        ExtendedBiome::StarfallCrater => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::CrystalGround, 0.25)
            .with_feature(LocalFeature::CrystalCluster, 0.20)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_caves(),

        ExtendedBiome::LeyNexus => BiomeFeatureConfig::new(LocalTerrainType::CrystalGround)
            .with_feature(LocalFeature::CrystalCluster, 0.25)
            .with_feature(LocalFeature::AncientMonolith, 0.05)
            .with_feature(LocalFeature::GlowingMoss, 0.12),

        ExtendedBiome::WhisperingStones => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.2)
            .with_feature(LocalFeature::AncientMonolith, 0.10)
            .with_feature(LocalFeature::Boulder, 0.12)
            .with_feature(LocalFeature::StoneRuin, 0.05)
            .with_caves(),

        ExtendedBiome::SpiritMarsh => BiomeFeatureConfig::new(LocalTerrainType::Marsh)
            .with_secondary(LocalTerrainType::ShallowWater, 0.25)
            .with_feature(LocalFeature::GlowingMoss, 0.20)
            .with_feature(LocalFeature::DeadTree, 0.10)
            .with_feature(LocalFeature::TallReeds, 0.12)
            .with_water(0.30)
            .with_caves(),

        // ========== NEW BIOMES - EXTREME GEOLOGICAL ==========

        ExtendedBiome::SulfurVents => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::Geyser, 0.12)
            .with_feature(LocalFeature::Spring, 0.08)
            .with_feature(LocalFeature::RockPile, 0.05),

        ExtendedBiome::BasaltColumns => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.25)
            .with_feature(LocalFeature::Stalagmite, 0.20)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_caves(),

        ExtendedBiome::PaintedHills => BiomeFeatureConfig::new(LocalTerrainType::Dirt)
            .with_secondary(LocalTerrainType::Gravel, 0.25)
            .with_feature(LocalFeature::Boulder, 0.08)
            .with_feature(LocalFeature::RockPile, 0.06),

        ExtendedBiome::RazorPeaks => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.2)
            .with_feature(LocalFeature::Stalagmite, 0.15)
            .with_feature(LocalFeature::Boulder, 0.12)
            .with_caves(),

        ExtendedBiome::SinkholeLakes => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::DeepWater, 0.3)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::RockPile, 0.08)
            .with_water(0.50)
            .with_caves(),

        // ========== NEW BIOMES - BIOLOGICAL WONDERS ==========

        ExtendedBiome::ColossalHive => BiomeFeatureConfig::new(LocalTerrainType::Dirt)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::MushroomPatch, 0.15)
            .with_feature(LocalFeature::VineTangle, 0.10)
            .with_caves(),

        ExtendedBiome::BoneFields => BiomeFeatureConfig::new(LocalTerrainType::Bone)
            .with_secondary(LocalTerrainType::Dirt, 0.25)
            .with_feature(LocalFeature::BoneRemains, 0.25)
            .with_feature(LocalFeature::Boulder, 0.05),

        ExtendedBiome::CarnivorousBog => BiomeFeatureConfig::new(LocalTerrainType::Marsh)
            .with_secondary(LocalTerrainType::Mud, 0.3)
            .with_feature(LocalFeature::FlowerPatch, 0.15) // Carnivorous plants
            .with_feature(LocalFeature::TallReeds, 0.12)
            .with_feature(LocalFeature::BoneRemains, 0.05)
            .with_water(0.25),

        ExtendedBiome::FungalBloom => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Mud, 0.15)
            .with_feature(LocalFeature::MushroomPatch, 0.40)
            .with_feature(LocalFeature::GlowingMoss, 0.12)
            .with_water(0.10)
            .with_caves(),

        ExtendedBiome::KelpTowers => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::DeepWater, 0.3)
            .with_feature(LocalFeature::TallReeds, 0.30), // Represent kelp

        // ========== NEW BIOMES - EXOTIC WATERS ==========

        ExtendedBiome::BrinePools => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Salt, 0.25)
            .with_feature(LocalFeature::RockPile, 0.05)
            .with_water(0.60),

        ExtendedBiome::HotSprings => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::Spring, 0.20)
            .with_feature(LocalFeature::Geyser, 0.08)
            .with_feature(LocalFeature::GlowingMoss, 0.05)
            .with_water(0.40),

        ExtendedBiome::MirrorLake => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::DeepWater, 0.4)
            .with_feature(LocalFeature::TallReeds, 0.05)
            .with_water(0.70),

        ExtendedBiome::InkSea => BiomeFeatureConfig::new(LocalTerrainType::DeepWater),

        ExtendedBiome::PhosphorShallows => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_feature(LocalFeature::GlowingMoss, 0.15)
            .with_feature(LocalFeature::TallReeds, 0.08)
            .with_water(0.50),

        // ========== NEW BIOMES - ALIEN/CORRUPTED ==========

        ExtendedBiome::VoidScar => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::CrystalGround, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.15)
            .with_feature(LocalFeature::AncientMonolith, 0.05)
            .with_caves(),

        ExtendedBiome::SiliconGrove => BiomeFeatureConfig::new(LocalTerrainType::CrystalGround)
            .with_secondary(LocalTerrainType::Stone, 0.15)
            .with_feature(LocalFeature::CrystalCluster, 0.30)
            .with_feature(LocalFeature::CrystalFlower, 0.15),

        ExtendedBiome::SporeWastes => BiomeFeatureConfig::new(LocalTerrainType::Dirt)
            .with_secondary(LocalTerrainType::Ash, 0.2)
            .with_feature(LocalFeature::MushroomPatch, 0.25)
            .with_feature(LocalFeature::GlowingMoss, 0.10)
            .with_feature(LocalFeature::DeadTree, 0.08),

        ExtendedBiome::BleedingStone => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.2)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_feature(LocalFeature::Spring, 0.05)
            .with_caves(),

        ExtendedBiome::HollowEarth => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.25)
            .with_feature(LocalFeature::CaveOpening, 0.10)
            .with_feature(LocalFeature::Stalagmite, 0.12)
            .with_feature(LocalFeature::GlowingMoss, 0.08)
            .with_caves(),

        // ========== NEW BIOMES - ANCIENT RUINS ==========

        ExtendedBiome::SunkenCity => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Stone, 0.25)
            .with_feature(LocalFeature::StoneRuin, 0.15)
            .with_feature(LocalFeature::AncientMonolith, 0.05)
            .with_water(0.40),

        ExtendedBiome::CyclopeanRuins => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.2)
            .with_feature(LocalFeature::StoneRuin, 0.20)
            .with_feature(LocalFeature::AncientMonolith, 0.08)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_caves(),

        ExtendedBiome::BuriedTemple => BiomeFeatureConfig::new(LocalTerrainType::Sand)
            .with_secondary(LocalTerrainType::Stone, 0.25)
            .with_feature(LocalFeature::StoneRuin, 0.12)
            .with_feature(LocalFeature::Shrine, 0.05)
            .with_feature(LocalFeature::AncientMonolith, 0.03)
            .with_caves(),

        ExtendedBiome::OvergrownCitadel => BiomeFeatureConfig::new(LocalTerrainType::ForestFloor)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::DeciduousTree, 0.20)
            .with_feature(LocalFeature::VineTangle, 0.15)
            .with_feature(LocalFeature::StoneRuin, 0.12)
            .with_feature(LocalFeature::Fern, 0.08)
            .with_caves(),

        ExtendedBiome::DarkTower => BiomeFeatureConfig::new(LocalTerrainType::Obsidian)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::AncientMonolith, 0.10)
            .with_feature(LocalFeature::StoneRuin, 0.08)
            .with_caves(),

        // ========== OCEAN BIOMES - REALISTIC ==========

        ExtendedBiome::CoralReef => BiomeFeatureConfig::new(LocalTerrainType::Coral)
            .with_secondary(LocalTerrainType::ShallowWater, 0.3)
            .with_feature(LocalFeature::CrystalCluster, 0.08), // Represent coral

        ExtendedBiome::KelpForest => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_feature(LocalFeature::TallReeds, 0.35), // Represent kelp

        ExtendedBiome::SeagrassMeadow => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_feature(LocalFeature::TallReeds, 0.20),

        ExtendedBiome::ContinentalShelf => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Sand, 0.2),

        ExtendedBiome::Seamount => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::Stone, 0.15),

        ExtendedBiome::OceanicTrench => BiomeFeatureConfig::new(LocalTerrainType::DeepWater),

        ExtendedBiome::AbyssalPlain => BiomeFeatureConfig::new(LocalTerrainType::DeepWater),

        ExtendedBiome::MidOceanRidge => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.15)
            .with_feature(LocalFeature::Geyser, 0.05),

        ExtendedBiome::ColdSeep => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_feature(LocalFeature::Spring, 0.08),

        ExtendedBiome::BrinePool => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::Salt, 0.1),

        // ========== OCEAN BIOMES - FANTASY ==========

        ExtendedBiome::CrystalDepths => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::CrystalGround, 0.2)
            .with_feature(LocalFeature::CrystalCluster, 0.15),

        ExtendedBiome::LeviathanGraveyard => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::Bone, 0.25)
            .with_feature(LocalFeature::BoneRemains, 0.20),

        ExtendedBiome::DrownedCitadel => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::StoneRuin, 0.15),

        ExtendedBiome::VoidMaw => BiomeFeatureConfig::new(LocalTerrainType::DeepWater),

        ExtendedBiome::PearlGardens => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_feature(LocalFeature::CrystalCluster, 0.12)
            .with_feature(LocalFeature::CrystalFlower, 0.08),

        ExtendedBiome::SirenShallows => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_feature(LocalFeature::TallReeds, 0.15)
            .with_feature(LocalFeature::GlowingMoss, 0.08),

        ExtendedBiome::FrozenAbyss => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::Ice, 0.15)
            .with_feature(LocalFeature::IceFormation, 0.08),

        ExtendedBiome::ThermalVents => BiomeFeatureConfig::new(LocalTerrainType::DeepWater)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.15)
            .with_feature(LocalFeature::Geyser, 0.12),

        // ========== KARST & CAVE BIOMES ==========

        ExtendedBiome::KarstPlains => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Grass, 0.25)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_feature(LocalFeature::CaveOpening, 0.05)
            .with_caves(),

        ExtendedBiome::TowerKarst => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Grass, 0.2)
            .with_feature(LocalFeature::Stalagmite, 0.15) // Represent karst pillars
            .with_feature(LocalFeature::DeciduousTree, 0.10)
            .with_feature(LocalFeature::Fern, 0.08)
            .with_water(0.10)
            .with_caves(),

        ExtendedBiome::Sinkhole => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.25)
            .with_feature(LocalFeature::CaveOpening, 0.15)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_caves(),

        ExtendedBiome::Cenote => BiomeFeatureConfig::new(LocalTerrainType::ShallowWater)
            .with_secondary(LocalTerrainType::Stone, 0.25)
            .with_feature(LocalFeature::VineTangle, 0.10)
            .with_feature(LocalFeature::Fern, 0.08)
            .with_water(0.50)
            .with_caves(),

        ExtendedBiome::CaveEntrance => BiomeFeatureConfig::new(LocalTerrainType::Stone)
            .with_secondary(LocalTerrainType::Gravel, 0.2)
            .with_feature(LocalFeature::CaveOpening, 0.25)
            .with_feature(LocalFeature::Stalagmite, 0.10)
            .with_feature(LocalFeature::GlowingMoss, 0.05)
            .with_caves(),

        ExtendedBiome::CockpitKarst => BiomeFeatureConfig::new(LocalTerrainType::Grass)
            .with_secondary(LocalTerrainType::Stone, 0.25)
            .with_feature(LocalFeature::Boulder, 0.12)
            .with_feature(LocalFeature::DeciduousTree, 0.15)
            .with_feature(LocalFeature::Bush, 0.10)
            .with_water(0.08)
            .with_caves(),

        // ========== VOLCANIC BIOMES ==========

        ExtendedBiome::Caldera => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Ash, 0.2)
            .with_feature(LocalFeature::Boulder, 0.08)
            .with_feature(LocalFeature::Geyser, 0.05)
            .with_water(0.10) // Caldera lake
            .with_caves(),

        ExtendedBiome::ShieldVolcano => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Ash, 0.15)
            .with_feature(LocalFeature::Boulder, 0.06)
            .with_feature(LocalFeature::RockPile, 0.08)
            .with_caves(),

        ExtendedBiome::VolcanicCone => BiomeFeatureConfig::new(LocalTerrainType::Ash)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.25)
            .with_feature(LocalFeature::Boulder, 0.10)
            .with_feature(LocalFeature::RockPile, 0.08)
            .with_caves(),

        ExtendedBiome::LavaField => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Lava, 0.15)
            .with_feature(LocalFeature::Boulder, 0.08),

        ExtendedBiome::FumaroleField => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Stone, 0.2)
            .with_feature(LocalFeature::Geyser, 0.15)
            .with_feature(LocalFeature::Spring, 0.10),

        ExtendedBiome::VolcanicBeach => BiomeFeatureConfig::new(LocalTerrainType::Sand)
            .with_secondary(LocalTerrainType::VolcanicRock, 0.2)
            .with_feature(LocalFeature::RockPile, 0.05)
            .with_water(0.15),

        ExtendedBiome::HotSpot => BiomeFeatureConfig::new(LocalTerrainType::VolcanicRock)
            .with_secondary(LocalTerrainType::Lava, 0.1)
            .with_feature(LocalFeature::Geyser, 0.10)
            .with_feature(LocalFeature::Boulder, 0.08)
            .with_caves(),
    }
}
