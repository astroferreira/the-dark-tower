//! Biome-specific terrain generation for local maps.
//!
//! Each biome has unique terrain patterns, vegetation, and features.
//! This module provides terrain generation that matches the biome's character.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;

use super::local::{LocalChunk, LocalTile, LocalTerrain, LocalFeature, Material, SoilType, StoneType};
use super::geology::GeologyParams;
use super::LOCAL_SIZE;

/// Configuration for biome-specific terrain generation
#[derive(Clone, Debug)]
pub struct BiomeTerrainConfig {
    /// Primary surface terrain type
    pub surface_terrain: LocalTerrain,
    /// Primary surface material
    pub surface_material: Material,
    /// Tree density (0.0 - 1.0)
    pub tree_density: f32,
    /// Bush/shrub density (0.0 - 1.0)
    pub bush_density: f32,
    /// Boulder/rock density (0.0 - 1.0)
    pub boulder_density: f32,
    /// Water pool chance (0.0 - 1.0)
    pub water_chance: f32,
    /// Special feature chance (0.0 - 1.0)
    pub special_feature_chance: f32,
    /// Terrain variation amplitude (z-levels)
    pub terrain_variation: i16,
    /// Whether this biome has dense vegetation that blocks movement
    pub has_dense_vegetation: bool,
    /// Soil type for underground
    pub soil_type: SoilType,
    /// Primary stone type
    pub stone_type: StoneType,
}

impl Default for BiomeTerrainConfig {
    fn default() -> Self {
        Self {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.05,
            bush_density: 0.05,
            boulder_density: 0.02,
            water_chance: 0.0,
            special_feature_chance: 0.0,
            terrain_variation: 2,
            has_dense_vegetation: false,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
        }
    }
}

/// Get the terrain configuration for a specific biome
pub fn get_biome_config(biome: ExtendedBiome) -> BiomeTerrainConfig {
    match biome {
        // =====================================================================
        // BASE BIOMES - Ocean/Water
        // =====================================================================
        ExtendedBiome::DeepOcean => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.0,
            water_chance: 1.0,
            terrain_variation: 0,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::Ocean => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.0,
            water_chance: 1.0,
            terrain_variation: 0,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::CoastalWater => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.01,
            water_chance: 0.9,
            terrain_variation: 1,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::Lagoon => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.02,
            boulder_density: 0.0,
            water_chance: 0.8,
            terrain_variation: 1,
            soil_type: SoilType::Sand,
            ..Default::default()
        },

        // =====================================================================
        // BASE BIOMES - Cold
        // =====================================================================
        ExtendedBiome::Ice => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Ice,
            surface_material: Material::Ice,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.03,
            water_chance: 0.0,
            terrain_variation: 1,
            soil_type: SoilType::Permafrost,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::Tundra => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Snow,
            surface_material: Material::Snow,
            tree_density: 0.0,
            bush_density: 0.03,
            boulder_density: 0.04,
            water_chance: 0.05,
            terrain_variation: 1,
            soil_type: SoilType::Permafrost,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::BorealForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.12,
            bush_density: 0.05,
            boulder_density: 0.02,
            water_chance: 0.03,
            terrain_variation: 2,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        // =====================================================================
        // BASE BIOMES - Temperate
        // =====================================================================
        ExtendedBiome::TemperateGrassland => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.02,
            bush_density: 0.05,
            boulder_density: 0.01,
            water_chance: 0.02,
            terrain_variation: 2,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::TemperateForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.18,
            bush_density: 0.10,
            boulder_density: 0.02,
            water_chance: 0.03,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::TemperateRainforest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.25,
            bush_density: 0.15,
            boulder_density: 0.01,
            water_chance: 0.08,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // BASE BIOMES - Hot/Arid
        // =====================================================================
        ExtendedBiome::Desert => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Sand,
            surface_material: Material::Sand,
            tree_density: 0.0,
            bush_density: 0.01,
            boulder_density: 0.03,
            water_chance: 0.0,
            special_feature_chance: 0.02, // Cacti, dead shrubs
            terrain_variation: 3,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::Savanna => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.03, // Sparse trees (acacia-like)
            bush_density: 0.06,
            boulder_density: 0.02,
            water_chance: 0.02,
            terrain_variation: 2,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::TropicalForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.20,
            bush_density: 0.12,
            boulder_density: 0.01,
            water_chance: 0.05,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::TropicalRainforest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DenseVegetation,
            surface_material: Material::Grass,
            tree_density: 0.30,
            bush_density: 0.20,
            boulder_density: 0.01,
            water_chance: 0.10,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // BASE BIOMES - Mountain/Alpine
        // =====================================================================
        ExtendedBiome::AlpineTundra => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.02,
            boulder_density: 0.08,
            water_chance: 0.02,
            terrain_variation: 3,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::SnowyPeaks => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Snow,
            surface_material: Material::Snow,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.10,
            water_chance: 0.0,
            terrain_variation: 4,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::Foothills => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.08,
            bush_density: 0.06,
            boulder_density: 0.05,
            water_chance: 0.03,
            terrain_variation: 3,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        // =====================================================================
        // WETLANDS
        // =====================================================================
        ExtendedBiome::Swamp => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.10,
            bush_density: 0.08,
            boulder_density: 0.0,
            water_chance: 0.30,
            terrain_variation: 1,
            has_dense_vegetation: true,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::Marsh => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.02,
            bush_density: 0.15,
            boulder_density: 0.0,
            water_chance: 0.40,
            terrain_variation: 1,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::Bog => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.01,
            bush_density: 0.10,
            boulder_density: 0.0,
            water_chance: 0.50,
            terrain_variation: 1,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::MangroveSaltmarsh => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.15,
            bush_density: 0.05,
            boulder_density: 0.0,
            water_chance: 0.35,
            terrain_variation: 1,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // FANTASY FORESTS
        // =====================================================================
        ExtendedBiome::DeadForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            tree_density: 0.15, // Dead trees
            bush_density: 0.02,
            boulder_density: 0.03,
            water_chance: 0.0,
            special_feature_chance: 0.05,
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Slate,
            ..Default::default()
        },

        ExtendedBiome::CrystalForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Stone,
            tree_density: 0.12,
            bush_density: 0.0,
            boulder_density: 0.08, // Crystal formations
            water_chance: 0.05,
            special_feature_chance: 0.15,
            terrain_variation: 2,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::BioluminescentForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.20,
            bush_density: 0.15,
            boulder_density: 0.01,
            water_chance: 0.08,
            special_feature_chance: 0.20,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::MushroomForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.05,
            bush_density: 0.25, // Giant mushrooms as "bushes"
            boulder_density: 0.02,
            water_chance: 0.10,
            special_feature_chance: 0.30,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::PetrifiedForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.12, // Stone trees
            bush_density: 0.0,
            boulder_density: 0.15,
            water_chance: 0.0,
            terrain_variation: 2,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        // =====================================================================
        // FANTASY WATERS
        // =====================================================================
        ExtendedBiome::AcidLake => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.02,
            water_chance: 0.9,
            special_feature_chance: 0.10,
            terrain_variation: 1,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::LavaLake => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Magma,
            surface_material: Material::Magma,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.05,
            water_chance: 0.0,
            terrain_variation: 1,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        ExtendedBiome::FrozenLake => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Ice,
            surface_material: Material::Ice,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.02,
            water_chance: 0.0,
            terrain_variation: 0,
            soil_type: SoilType::Permafrost,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::BioluminescentWater => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.02,
            boulder_density: 0.01,
            water_chance: 0.85,
            special_feature_chance: 0.20,
            terrain_variation: 1,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // WASTELANDS
        // =====================================================================
        ExtendedBiome::VolcanicWasteland => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.12,
            water_chance: 0.0,
            special_feature_chance: 0.08, // Lava pools, vents
            terrain_variation: 3,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::SaltFlats => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Sand,
            surface_material: Material::Sand,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.01,
            water_chance: 0.0,
            terrain_variation: 0, // Flat!
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::Ashlands => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            tree_density: 0.0,
            bush_density: 0.01,
            boulder_density: 0.05,
            water_chance: 0.0,
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::CrystalWasteland => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.15, // Crystal formations
            water_chance: 0.0,
            special_feature_chance: 0.20,
            terrain_variation: 2,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        // =====================================================================
        // VOLCANIC BIOMES
        // =====================================================================
        ExtendedBiome::Caldera => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.08,
            water_chance: 0.20, // Crater lake
            special_feature_chance: 0.10,
            terrain_variation: 4,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::ShieldVolcano => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.01,
            boulder_density: 0.10,
            water_chance: 0.0,
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::VolcanicCone => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.12,
            water_chance: 0.0,
            special_feature_chance: 0.15, // Fumaroles
            terrain_variation: 4,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::LavaField => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.08,
            water_chance: 0.0,
            special_feature_chance: 0.20, // Lava tubes, cracks
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::FumaroleField => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.05,
            water_chance: 0.10, // Hot springs
            special_feature_chance: 0.30, // Steam vents
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::VolcanicBeach => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Sand,
            surface_material: Material::Sand,
            tree_density: 0.01,
            bush_density: 0.02,
            boulder_density: 0.05,
            water_chance: 0.30,
            terrain_variation: 1,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::HotSpot => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.10,
            water_chance: 0.05,
            special_feature_chance: 0.25,
            terrain_variation: 3,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        // =====================================================================
        // KARST/CAVE BIOMES
        // =====================================================================
        ExtendedBiome::KarstPlains => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.05,
            bush_density: 0.05,
            boulder_density: 0.08, // Exposed limestone
            water_chance: 0.05,
            special_feature_chance: 0.10, // Sinkholes
            terrain_variation: 3,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::TowerKarst => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.10,
            bush_density: 0.08,
            boulder_density: 0.15, // Limestone pillars
            water_chance: 0.10,
            special_feature_chance: 0.20,
            terrain_variation: 5, // Dramatic!
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::Sinkhole => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.08,
            bush_density: 0.06,
            boulder_density: 0.05,
            water_chance: 0.15,
            special_feature_chance: 0.30, // Cave openings
            terrain_variation: 4,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::Cenote => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.12,
            bush_density: 0.08,
            boulder_density: 0.03,
            water_chance: 0.40, // Water-filled sinkhole
            special_feature_chance: 0.15,
            terrain_variation: 3,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::CaveEntrance => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.05,
            bush_density: 0.08,
            boulder_density: 0.12,
            water_chance: 0.05,
            special_feature_chance: 0.50, // Cave openings!
            terrain_variation: 3,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::CockpitKarst => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.15,
            bush_density: 0.10,
            boulder_density: 0.10,
            water_chance: 0.08,
            special_feature_chance: 0.15,
            terrain_variation: 4,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // GEOTHERMAL
        // =====================================================================
        ExtendedBiome::ObsidianFields => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.15,
            water_chance: 0.0,
            special_feature_chance: 0.10,
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        ExtendedBiome::Geysers => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.01,
            boulder_density: 0.05,
            water_chance: 0.20,
            special_feature_chance: 0.40, // Geysers!
            terrain_variation: 2,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::TarPits => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.02,
            bush_density: 0.03,
            boulder_density: 0.02,
            water_chance: 0.30, // Tar pools
            special_feature_chance: 0.15, // Bones, bubbles
            terrain_variation: 1,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::SulfurVents => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Gravel,
            surface_material: Material::Stone,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.08,
            water_chance: 0.10,
            special_feature_chance: 0.35,
            terrain_variation: 2,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::HotSprings => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.05,
            bush_density: 0.08,
            boulder_density: 0.05,
            water_chance: 0.40,
            special_feature_chance: 0.20,
            terrain_variation: 2,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // OCEAN ZONES - Deep underwater biomes
        // =====================================================================
        ExtendedBiome::AbyssalPlain => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            tree_density: 0.0,
            bush_density: 0.0,
            boulder_density: 0.02,
            water_chance: 1.0,
            terrain_variation: 1,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::AbyssalVents => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.08,
            water_chance: 1.0,
            special_feature_chance: 0.15,
            terrain_variation: 3,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::ContinentalShelf => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            boulder_density: 0.03,
            water_chance: 0.95,
            terrain_variation: 2,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::Seamount => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.1,
            water_chance: 0.9,
            terrain_variation: 4,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::OceanicTrench => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.05,
            water_chance: 1.0,
            terrain_variation: 5,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::MidOceanRidge => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.12,
            water_chance: 1.0,
            special_feature_chance: 0.1,
            terrain_variation: 4,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::ColdSeep | ExtendedBiome::BrinePool | ExtendedBiome::BrinePools => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.04,
            water_chance: 1.0,
            special_feature_chance: 0.08,
            terrain_variation: 1,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::CoralReef => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            bush_density: 0.3, // Coral
            boulder_density: 0.15,
            water_chance: 0.95,
            special_feature_chance: 0.2,
            terrain_variation: 2,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::KelpForest => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            tree_density: 0.25, // Kelp stalks
            bush_density: 0.15,
            water_chance: 0.98,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::SeagrassMeadow => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            bush_density: 0.4, // Seagrass
            water_chance: 0.95,
            terrain_variation: 1,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        // =====================================================================
        // EXOTIC UNDERWATER
        // =====================================================================
        ExtendedBiome::DrownedCitadel => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.15,
            water_chance: 0.9,
            special_feature_chance: 0.25,
            terrain_variation: 3,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::PearlGardens => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            bush_density: 0.2,
            boulder_density: 0.1,
            water_chance: 0.95,
            special_feature_chance: 0.15,
            terrain_variation: 1,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::SirenShallows => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            boulder_density: 0.08,
            water_chance: 0.92,
            special_feature_chance: 0.12,
            terrain_variation: 2,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::FrozenAbyss => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Ice,
            boulder_density: 0.06,
            water_chance: 0.85,
            terrain_variation: 2,
            soil_type: SoilType::Permafrost,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::ThermalVents => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.1,
            water_chance: 0.9,
            special_feature_chance: 0.2,
            terrain_variation: 3,
            soil_type: SoilType::Ash,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::VoidMaw => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            boulder_density: 0.03,
            water_chance: 1.0,
            special_feature_chance: 0.05,
            terrain_variation: 6,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        // =====================================================================
        // MYSTICAL / MAGICAL BIOMES
        // =====================================================================
        ExtendedBiome::AuroraWastes => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Snow,
            surface_material: Material::Snow,
            boulder_density: 0.04,
            water_chance: 0.05,
            special_feature_chance: 0.15,
            terrain_variation: 2,
            soil_type: SoilType::Permafrost,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::EtherealMist => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.08,
            bush_density: 0.12,
            water_chance: 0.15,
            special_feature_chance: 0.2,
            terrain_variation: 2,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::StarfallCrater => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            boulder_density: 0.2,
            special_feature_chance: 0.25,
            terrain_variation: 4,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        ExtendedBiome::LeyNexus => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.05,
            bush_density: 0.1,
            boulder_density: 0.08,
            special_feature_chance: 0.3,
            terrain_variation: 2,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::WhisperingStones => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            boulder_density: 0.25,
            special_feature_chance: 0.15,
            terrain_variation: 3,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::SpiritMarsh => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.1,
            bush_density: 0.15,
            water_chance: 0.35,
            special_feature_chance: 0.18,
            terrain_variation: 1,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::FloatingStones => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            boulder_density: 0.3,
            special_feature_chance: 0.2,
            terrain_variation: 5,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::Shadowfen => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.15,
            bush_density: 0.2,
            water_chance: 0.25,
            special_feature_chance: 0.12,
            terrain_variation: 1,
            has_dense_vegetation: true,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::PrismaticPools => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            boulder_density: 0.05,
            water_chance: 0.4,
            special_feature_chance: 0.25,
            terrain_variation: 2,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        // =====================================================================
        // DESERT VARIANTS
        // =====================================================================
        ExtendedBiome::SingingDunes => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Sand,
            surface_material: Material::Sand,
            boulder_density: 0.01,
            special_feature_chance: 0.05,
            terrain_variation: 4,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::Oasis => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.2,
            bush_density: 0.15,
            water_chance: 0.3,
            terrain_variation: 1,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        ExtendedBiome::GlassDesert => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Sand,
            surface_material: Material::Sand,
            boulder_density: 0.15,
            special_feature_chance: 0.1,
            terrain_variation: 2,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        // =====================================================================
        // GEOLOGICAL FEATURES
        // =====================================================================
        ExtendedBiome::BasaltColumns => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::StoneFloor,
            surface_material: Material::Stone,
            boulder_density: 0.3,
            terrain_variation: 4,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Basalt,
            ..Default::default()
        },

        ExtendedBiome::PaintedHills => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            bush_density: 0.05,
            boulder_density: 0.08,
            terrain_variation: 3,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::RazorPeaks => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::StoneFloor,
            surface_material: Material::Stone,
            boulder_density: 0.25,
            terrain_variation: 6,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::SinkholeLakes => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            bush_density: 0.1,
            water_chance: 0.35,
            terrain_variation: 5,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // BIOLOGICAL / ALIEN
        // =====================================================================
        ExtendedBiome::ColossalHive => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            boulder_density: 0.15,
            special_feature_chance: 0.2,
            terrain_variation: 3,
            has_dense_vegetation: true,
            soil_type: SoilType::Clay,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::CarnivorousBog => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Mud,
            surface_material: Material::Mud,
            tree_density: 0.08,
            bush_density: 0.25,
            water_chance: 0.3,
            special_feature_chance: 0.15,
            terrain_variation: 1,
            has_dense_vegetation: true,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::KelpTowers => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            tree_density: 0.3,
            water_chance: 0.95,
            terrain_variation: 3,
            has_dense_vegetation: true,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::MirrorLake => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            boulder_density: 0.02,
            water_chance: 0.9,
            terrain_variation: 0,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::InkSea => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DeepWater,
            surface_material: Material::Water,
            water_chance: 1.0,
            terrain_variation: 1,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        ExtendedBiome::PhosphorShallows => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            bush_density: 0.1,
            water_chance: 0.9,
            special_feature_chance: 0.15,
            terrain_variation: 1,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        ExtendedBiome::Sargasso => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            bush_density: 0.5,
            water_chance: 0.85,
            terrain_variation: 1,
            has_dense_vegetation: true,
            soil_type: SoilType::Silt,
            stone_type: StoneType::Sandstone,
            ..Default::default()
        },

        // =====================================================================
        // ALIEN / EXOTIC
        // =====================================================================
        ExtendedBiome::VoidScar => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::StoneFloor,
            surface_material: Material::Stone,
            boulder_density: 0.1,
            special_feature_chance: 0.2,
            terrain_variation: 4,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Obsidian,
            ..Default::default()
        },

        ExtendedBiome::SiliconGrove => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            tree_density: 0.15,
            boulder_density: 0.1,
            special_feature_chance: 0.18,
            terrain_variation: 2,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Marble,
            ..Default::default()
        },

        ExtendedBiome::SporeWastes => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::DirtFloor,
            surface_material: Material::Dirt,
            bush_density: 0.3,
            special_feature_chance: 0.2,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Peat,
            stone_type: StoneType::Shale,
            ..Default::default()
        },

        ExtendedBiome::BleedingStone => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::StoneFloor,
            surface_material: Material::Stone,
            boulder_density: 0.2,
            water_chance: 0.1,
            special_feature_chance: 0.15,
            terrain_variation: 3,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::HollowEarth => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::CaveFloor,
            surface_material: Material::Stone,
            boulder_density: 0.12,
            special_feature_chance: 0.15,
            terrain_variation: 3,
            soil_type: SoilType::Gravel,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // SPECIAL FEATURES
        // =====================================================================
        ExtendedBiome::AncientGrove => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::Grass,
            surface_material: Material::Grass,
            tree_density: 0.4,
            bush_density: 0.2,
            special_feature_chance: 0.15,
            terrain_variation: 2,
            has_dense_vegetation: true,
            soil_type: SoilType::Loam,
            stone_type: StoneType::Granite,
            ..Default::default()
        },

        ExtendedBiome::CoralPlateau => BiomeTerrainConfig {
            surface_terrain: LocalTerrain::ShallowWater,
            surface_material: Material::Water,
            bush_density: 0.25,
            boulder_density: 0.15,
            water_chance: 0.8,
            special_feature_chance: 0.2,
            terrain_variation: 2,
            soil_type: SoilType::Sand,
            stone_type: StoneType::Limestone,
            ..Default::default()
        },

        // =====================================================================
        // DEFAULT - Use sensible defaults for any biome not explicitly handled
        // =====================================================================
        _ => BiomeTerrainConfig::default(),
    }
}

/// Generate the surface terrain for a local chunk based on biome configuration
pub fn generate_biome_surface(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    config: &BiomeTerrainConfig,
    surface_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) {
    let surface_z = geology.surface_z;

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Calculate surface variation
            let nx = x as f64 / LOCAL_SIZE as f64 * 4.0;
            let ny = y as f64 / LOCAL_SIZE as f64 * 4.0;
            let variation = (surface_noise.get([nx, ny]) * config.terrain_variation as f64) as i16;
            let local_z = (surface_z + variation).clamp(chunk.z_min, chunk.z_max);

            // Set all tiles from local_z to surface_z + max_variation as air
            for z in (local_z + 1)..=(surface_z + config.terrain_variation) {
                if z >= chunk.z_min && z <= chunk.z_max {
                    chunk.set(x, y, z, LocalTile::air());
                }
            }

            // Set the surface tile
            let mut surface_tile = LocalTile::new(config.surface_terrain, config.surface_material);
            surface_tile.temperature = geology.temperature;

            // Check for water pools
            if config.water_chance > 0.0 && rng.gen::<f32>() < config.water_chance {
                // Create water in depressions (negative variation)
                if variation < 0 {
                    surface_tile = LocalTile::new(
                        if config.water_chance > 0.5 {
                            LocalTerrain::DeepWater
                        } else {
                            LocalTerrain::ShallowWater
                        },
                        Material::Water,
                    );
                }
            }

            chunk.set(x, y, local_z, surface_tile);

            // Set underground tiles
            for z in chunk.z_min..local_z {
                let depth = local_z - z;
                let tile = if depth <= geology.soil_depth as i16 {
                    LocalTile::soil(config.soil_type)
                } else {
                    LocalTile::stone(config.stone_type)
                };
                chunk.set(x, y, z, tile);
            }
        }
    }
}

/// Add biome-specific surface features (trees, boulders, special features)
pub fn add_biome_features(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    config: &BiomeTerrainConfig,
    rng: &mut ChaCha8Rng,
) {
    let surface_z = geology.surface_z;

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Find the actual surface at this position
            let mut local_surface_z = surface_z;
            for z in (chunk.z_min..=surface_z + config.terrain_variation).rev() {
                let tile = chunk.get(x, y, z);
                if !tile.terrain.is_solid() && tile.terrain != LocalTerrain::Air {
                    local_surface_z = z;
                    break;
                }
            }

            let tile = chunk.get(x, y, local_surface_z);

            // Only add features on passable, non-water terrain
            if !tile.terrain.is_passable() || tile.terrain.is_water() {
                continue;
            }

            // Trees
            if config.tree_density > 0.0 && rng.gen::<f32>() < config.tree_density {
                let height = rng.gen_range(3..8);
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Tree { height };
                continue;
            }

            // Bushes
            if config.bush_density > 0.0 && rng.gen::<f32>() < config.bush_density {
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Bush;
                continue;
            }

            // Boulders
            if config.boulder_density > 0.0 && rng.gen::<f32>() < config.boulder_density {
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Boulder;
                continue;
            }

            // Special features (biome-specific)
            if config.special_feature_chance > 0.0 && rng.gen::<f32>() < config.special_feature_chance {
                let feature = get_special_feature(geology.biome, rng);
                chunk.get_mut(x, y, local_surface_z).feature = feature;
            }
        }
    }
}

/// Get a special feature appropriate for the biome
fn get_special_feature(biome: ExtendedBiome, rng: &mut ChaCha8Rng) -> LocalFeature {
    match biome {
        // Crystal biomes get crystal features
        ExtendedBiome::CrystalForest |
        ExtendedBiome::CrystalWasteland |
        ExtendedBiome::CrystalDepths => LocalFeature::Crystal,

        // Mushroom biomes get mushroom features
        ExtendedBiome::MushroomForest |
        ExtendedBiome::FungalBloom => {
            if rng.gen_bool(0.3) {
                LocalFeature::GiantMushroom
            } else {
                LocalFeature::Mushroom
            }
        }

        // Cave-related biomes get cave features
        ExtendedBiome::CaveEntrance |
        ExtendedBiome::Sinkhole |
        ExtendedBiome::Cenote |
        ExtendedBiome::KarstPlains |
        ExtendedBiome::TowerKarst => {
            if rng.gen_bool(0.5) {
                LocalFeature::RampDown
            } else {
                LocalFeature::Stalactite
            }
        }

        // Geothermal biomes get steam/heat features
        ExtendedBiome::Geysers |
        ExtendedBiome::SulfurVents |
        ExtendedBiome::FumaroleField |
        ExtendedBiome::HotSpot => LocalFeature::Fountain, // Steam vent

        // Volcanic biomes
        ExtendedBiome::VolcanicWasteland |
        ExtendedBiome::LavaField |
        ExtendedBiome::VolcanicCone => {
            if rng.gen_bool(0.3) {
                LocalFeature::RampDown // Lava tube entrance
            } else {
                LocalFeature::Boulder
            }
        }

        // Ruins biomes get structural features
        ExtendedBiome::SunkenCity |
        ExtendedBiome::CyclopeanRuins |
        ExtendedBiome::BuriedTemple |
        ExtendedBiome::OvergrownCitadel |
        ExtendedBiome::DarkTower => {
            match rng.gen_range(0..5) {
                0 => LocalFeature::Pillar,
                1 => LocalFeature::Rubble,
                2 => LocalFeature::Statue,
                3 => LocalFeature::Altar,
                _ => LocalFeature::StairsDown,
            }
        }

        // Bone fields
        ExtendedBiome::TitanBones |
        ExtendedBiome::BoneFields |
        ExtendedBiome::LeviathanGraveyard => LocalFeature::Rubble, // Bone piles

        // Bioluminescent biomes get light features
        ExtendedBiome::BioluminescentForest |
        ExtendedBiome::BioluminescentWater => LocalFeature::Crystal, // Glowing crystals

        // Tar pits
        ExtendedBiome::TarPits => LocalFeature::Rubble, // Bones in tar

        // Default: boulder or nothing
        _ => {
            if rng.gen_bool(0.5) {
                LocalFeature::Boulder
            } else {
                LocalFeature::None
            }
        }
    }
}

// =============================================================================
// BIOME BLENDING SYSTEM
// =============================================================================

/// Information about adjacent biomes for blending
#[derive(Clone, Debug)]
pub struct AdjacentBiomes {
    /// Biome to the north (y-1)
    pub north: Option<ExtendedBiome>,
    /// Biome to the south (y+1)
    pub south: Option<ExtendedBiome>,
    /// Biome to the east (x+1)
    pub east: Option<ExtendedBiome>,
    /// Biome to the west (x-1)
    pub west: Option<ExtendedBiome>,
}

impl AdjacentBiomes {
    /// Create from world data by sampling adjacent tiles
    pub fn from_world(
        world_biomes: &crate::tilemap::Tilemap<ExtendedBiome>,
        world_x: usize,
        world_y: usize,
    ) -> Self {
        let width = world_biomes.width;
        let height = world_biomes.height;

        Self {
            north: if world_y > 0 {
                Some(*world_biomes.get(world_x, world_y - 1))
            } else {
                None
            },
            south: if world_y < height - 1 {
                Some(*world_biomes.get(world_x, world_y + 1))
            } else {
                None
            },
            east: if world_x < width - 1 {
                Some(*world_biomes.get(world_x + 1, world_y))
            } else {
                // World wraps horizontally
                Some(*world_biomes.get(0, world_y))
            },
            west: if world_x > 0 {
                Some(*world_biomes.get(world_x - 1, world_y))
            } else {
                // World wraps horizontally
                Some(*world_biomes.get(width - 1, world_y))
            },
        }
    }
}

/// Calculate blend weight for a position within a chunk based on edge distance
/// Returns a value from 0.0 (center) to 1.0 (edge)
fn edge_blend_factor(local_x: usize, local_y: usize, blend_width: usize) -> (f32, f32, f32, f32) {
    let blend_width_f = blend_width as f32;

    // North edge (y = 0)
    let north_blend = if local_y < blend_width {
        1.0 - (local_y as f32 / blend_width_f)
    } else {
        0.0
    };

    // South edge (y = LOCAL_SIZE - 1)
    let south_blend = if local_y >= LOCAL_SIZE - blend_width {
        (local_y - (LOCAL_SIZE - blend_width)) as f32 / blend_width_f
    } else {
        0.0
    };

    // West edge (x = 0)
    let west_blend = if local_x < blend_width {
        1.0 - (local_x as f32 / blend_width_f)
    } else {
        0.0
    };

    // East edge (x = LOCAL_SIZE - 1)
    let east_blend = if local_x >= LOCAL_SIZE - blend_width {
        (local_x - (LOCAL_SIZE - blend_width)) as f32 / blend_width_f
    } else {
        0.0
    };

    (north_blend, south_blend, west_blend, east_blend)
}

/// Blend two terrain types based on weight
fn blend_terrain(
    primary: LocalTerrain,
    secondary: LocalTerrain,
    weight: f32,
    rng: &mut ChaCha8Rng,
) -> LocalTerrain {
    // Use weighted random selection
    if rng.gen::<f32>() < weight {
        secondary
    } else {
        primary
    }
}

/// Blend two materials based on weight
fn blend_material(
    primary: Material,
    secondary: Material,
    weight: f32,
    rng: &mut ChaCha8Rng,
) -> Material {
    if rng.gen::<f32>() < weight {
        secondary
    } else {
        primary
    }
}

/// Generate blended surface terrain considering adjacent biomes
pub fn generate_blended_biome_surface(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    primary_config: &BiomeTerrainConfig,
    adjacent: &AdjacentBiomes,
    surface_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) {
    let surface_z = geology.surface_z;
    let blend_width = 8; // Blend over 8 tiles at edges

    // Get configs for adjacent biomes
    let north_config = adjacent.north.map(get_biome_config);
    let south_config = adjacent.south.map(get_biome_config);
    let east_config = adjacent.east.map(get_biome_config);
    let west_config = adjacent.west.map(get_biome_config);

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Calculate blend factors for each edge
            let (north_blend, south_blend, west_blend, east_blend) =
                edge_blend_factor(x, y, blend_width);

            // Calculate surface variation using primary config
            let nx = x as f64 / LOCAL_SIZE as f64 * 4.0;
            let ny = y as f64 / LOCAL_SIZE as f64 * 4.0;
            let variation = (surface_noise.get([nx, ny]) * primary_config.terrain_variation as f64) as i16;
            let local_z = (surface_z + variation).clamp(chunk.z_min, chunk.z_max);

            // Set air above surface
            for z in (local_z + 1)..=(surface_z + primary_config.terrain_variation) {
                if z >= chunk.z_min && z <= chunk.z_max {
                    chunk.set(x, y, z, LocalTile::air());
                }
            }

            // Determine blended terrain and material
            let mut terrain = primary_config.surface_terrain;
            let mut material = primary_config.surface_material;

            // Apply blending from adjacent biomes
            if north_blend > 0.0 {
                if let Some(ref config) = north_config {
                    terrain = blend_terrain(terrain, config.surface_terrain, north_blend * 0.5, rng);
                    material = blend_material(material, config.surface_material, north_blend * 0.5, rng);
                }
            }
            if south_blend > 0.0 {
                if let Some(ref config) = south_config {
                    terrain = blend_terrain(terrain, config.surface_terrain, south_blend * 0.5, rng);
                    material = blend_material(material, config.surface_material, south_blend * 0.5, rng);
                }
            }
            if west_blend > 0.0 {
                if let Some(ref config) = west_config {
                    terrain = blend_terrain(terrain, config.surface_terrain, west_blend * 0.5, rng);
                    material = blend_material(material, config.surface_material, west_blend * 0.5, rng);
                }
            }
            if east_blend > 0.0 {
                if let Some(ref config) = east_config {
                    terrain = blend_terrain(terrain, config.surface_terrain, east_blend * 0.5, rng);
                    material = blend_material(material, config.surface_material, east_blend * 0.5, rng);
                }
            }

            // Create surface tile
            let mut surface_tile = LocalTile::new(terrain, material);
            surface_tile.temperature = geology.temperature;

            // Check for water pools (use blended water chance)
            let mut water_chance = primary_config.water_chance;
            if north_blend > 0.0 {
                if let Some(ref config) = north_config {
                    water_chance = water_chance * (1.0 - north_blend) + config.water_chance * north_blend;
                }
            }
            // Similar for other directions...

            if water_chance > 0.0 && rng.gen::<f32>() < water_chance && variation < 0 {
                surface_tile = LocalTile::new(
                    if water_chance > 0.5 {
                        LocalTerrain::DeepWater
                    } else {
                        LocalTerrain::ShallowWater
                    },
                    Material::Water,
                );
            }

            chunk.set(x, y, local_z, surface_tile);

            // Set underground tiles
            for z in chunk.z_min..local_z {
                let depth = local_z - z;
                let tile = if depth <= geology.soil_depth as i16 {
                    LocalTile::soil(primary_config.soil_type)
                } else {
                    LocalTile::stone(primary_config.stone_type)
                };
                chunk.set(x, y, z, tile);
            }
        }
    }
}

/// Add blended biome features considering adjacent biomes
pub fn add_blended_biome_features(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    primary_config: &BiomeTerrainConfig,
    adjacent: &AdjacentBiomes,
    rng: &mut ChaCha8Rng,
) {
    let surface_z = geology.surface_z;
    let blend_width = 8;

    // Get configs for adjacent biomes
    let north_config = adjacent.north.map(get_biome_config);
    let south_config = adjacent.south.map(get_biome_config);
    let east_config = adjacent.east.map(get_biome_config);
    let west_config = adjacent.west.map(get_biome_config);

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Calculate blend factors
            let (north_blend, south_blend, west_blend, east_blend) =
                edge_blend_factor(x, y, blend_width);

            // Find actual surface
            let mut local_surface_z = surface_z;
            for z in (chunk.z_min..=surface_z + primary_config.terrain_variation).rev() {
                let tile = chunk.get(x, y, z);
                if !tile.terrain.is_solid() && tile.terrain != LocalTerrain::Air {
                    local_surface_z = z;
                    break;
                }
            }

            let tile = chunk.get(x, y, local_surface_z);
            if !tile.terrain.is_passable() || tile.terrain.is_water() {
                continue;
            }

            // Blend feature densities
            let mut tree_density = primary_config.tree_density;
            let mut bush_density = primary_config.bush_density;
            let mut boulder_density = primary_config.boulder_density;

            // Blend with adjacent biomes
            let total_blend = north_blend + south_blend + west_blend + east_blend;
            if total_blend > 0.0 {
                let blend_factor = total_blend.min(1.0);

                // Calculate weighted average from adjacent biomes
                let mut adj_tree = 0.0f32;
                let mut adj_bush = 0.0f32;
                let mut adj_boulder = 0.0f32;
                let mut adj_weight = 0.0f32;

                if north_blend > 0.0 {
                    if let Some(ref config) = north_config {
                        adj_tree += config.tree_density * north_blend;
                        adj_bush += config.bush_density * north_blend;
                        adj_boulder += config.boulder_density * north_blend;
                        adj_weight += north_blend;
                    }
                }
                if south_blend > 0.0 {
                    if let Some(ref config) = south_config {
                        adj_tree += config.tree_density * south_blend;
                        adj_bush += config.bush_density * south_blend;
                        adj_boulder += config.boulder_density * south_blend;
                        adj_weight += south_blend;
                    }
                }
                if west_blend > 0.0 {
                    if let Some(ref config) = west_config {
                        adj_tree += config.tree_density * west_blend;
                        adj_bush += config.bush_density * west_blend;
                        adj_boulder += config.boulder_density * west_blend;
                        adj_weight += west_blend;
                    }
                }
                if east_blend > 0.0 {
                    if let Some(ref config) = east_config {
                        adj_tree += config.tree_density * east_blend;
                        adj_bush += config.bush_density * east_blend;
                        adj_boulder += config.boulder_density * east_blend;
                        adj_weight += east_blend;
                    }
                }

                if adj_weight > 0.0 {
                    adj_tree /= adj_weight;
                    adj_bush /= adj_weight;
                    adj_boulder /= adj_weight;

                    tree_density = tree_density * (1.0 - blend_factor) + adj_tree * blend_factor;
                    bush_density = bush_density * (1.0 - blend_factor) + adj_bush * blend_factor;
                    boulder_density = boulder_density * (1.0 - blend_factor) + adj_boulder * blend_factor;
                }
            }

            // Place features with blended densities
            if tree_density > 0.0 && rng.gen::<f32>() < tree_density {
                let height = rng.gen_range(3..8);
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Tree { height };
                continue;
            }

            if bush_density > 0.0 && rng.gen::<f32>() < bush_density {
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Bush;
                continue;
            }

            if boulder_density > 0.0 && rng.gen::<f32>() < boulder_density {
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Boulder;
                continue;
            }

            // Special features (use primary biome)
            if primary_config.special_feature_chance > 0.0
                && rng.gen::<f32>() < primary_config.special_feature_chance
            {
                let feature = get_special_feature(geology.biome, rng);
                chunk.get_mut(x, y, local_surface_z).feature = feature;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biome_config_defaults() {
        let config = BiomeTerrainConfig::default();
        assert_eq!(config.surface_terrain, LocalTerrain::Grass);
        assert!(config.tree_density >= 0.0 && config.tree_density <= 1.0);
    }

    #[test]
    fn test_all_biomes_have_config() {
        // Test a sample of biomes to ensure they have valid configs
        let biomes = [
            ExtendedBiome::Desert,
            ExtendedBiome::Savanna,
            ExtendedBiome::TropicalRainforest,
            ExtendedBiome::Swamp,
            ExtendedBiome::VolcanicWasteland,
            ExtendedBiome::CrystalForest,
        ];

        for biome in biomes {
            let config = get_biome_config(biome);
            assert!(config.terrain_variation >= 0);
            assert!(config.tree_density >= 0.0 && config.tree_density <= 1.0);
        }
    }

    #[test]
    fn test_desert_is_sandy() {
        let config = get_biome_config(ExtendedBiome::Desert);
        assert_eq!(config.surface_terrain, LocalTerrain::Sand);
        assert_eq!(config.surface_material, Material::Sand);
        assert_eq!(config.tree_density, 0.0);
    }

    #[test]
    fn test_swamp_is_muddy() {
        let config = get_biome_config(ExtendedBiome::Swamp);
        assert_eq!(config.surface_terrain, LocalTerrain::Mud);
        assert!(config.water_chance > 0.2);
    }
}
