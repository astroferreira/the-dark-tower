//! Biome-specific terrain generation for local maps.
//!
//! Each biome has unique terrain patterns, vegetation, and features.
//! This module provides terrain generation that matches the biome's character.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;
use crate::biome_feathering::BiomeFeatherMap;

use super::local::{LocalChunk, LocalTile, LocalTerrain, LocalFeature, Material, SoilType, StoneType};
use super::geology::{GeologyParams, CornerHeights, interpolate_surface_z, calculate_coastline_info_with_noise, CoastlineTerrainHint, is_water_biome};
use super::coords::{world_noise_coord, feature_seed, should_place_feature, position_random_range, position_random};
use super::LOCAL_SIZE;

// =============================================================================
// FEATURE POOL SYSTEM (Phase 2b)
// =============================================================================

/// A pool of features for a specific biome with distance decay
#[derive(Clone, Debug)]
pub struct FeaturePool {
    /// The biome this pool is for
    pub biome: ExtendedBiome,
    /// Features in this pool with their weights
    pub features: Vec<FeatureEntry>,
    /// Feature density at biome center (0.0-1.0)
    pub center_density: f32,
    /// Feature density at biome edge (0.0-1.0)
    pub edge_density: f32,
    /// Decay exponent (1.0=linear, 2.0=quadratic, 0.5=sqrt)
    pub decay_exponent: f32,
}

/// An entry in a feature pool
#[derive(Clone, Debug)]
pub struct FeatureEntry {
    /// The feature type
    pub feature: LocalFeature,
    /// Base weight for selection (higher = more common)
    pub weight: f32,
    /// Only spawn far from edges (true = needs depth > threshold)
    pub center_only: bool,
    /// Minimum depth from edge to spawn (0.0-1.0 normalized)
    pub min_depth: f32,
}

impl Default for FeaturePool {
    fn default() -> Self {
        Self {
            biome: ExtendedBiome::TemperateGrassland,
            features: Vec::new(),
            center_density: 0.1,
            edge_density: 0.02,
            decay_exponent: 1.5,
        }
    }
}

impl FeaturePool {
    /// Create a new feature pool for a biome
    pub fn new(biome: ExtendedBiome) -> Self {
        Self {
            biome,
            ..Default::default()
        }
    }

    /// Add a feature to the pool
    pub fn add_feature(&mut self, feature: LocalFeature, weight: f32) -> &mut Self {
        self.features.push(FeatureEntry {
            feature,
            weight,
            center_only: false,
            min_depth: 0.0,
        });
        self
    }

    /// Add a center-only feature (only spawns away from edges)
    pub fn add_center_feature(&mut self, feature: LocalFeature, weight: f32, min_depth: f32) -> &mut Self {
        self.features.push(FeatureEntry {
            feature,
            weight,
            center_only: true,
            min_depth,
        });
        self
    }

    /// Set density parameters
    pub fn with_density(mut self, center: f32, edge: f32, exponent: f32) -> Self {
        self.center_density = center;
        self.edge_density = edge;
        self.decay_exponent = exponent;
        self
    }

    /// Calculate effective density at a given normalized depth (0=edge, 1=center)
    pub fn density_at_depth(&self, normalized_depth: f32) -> f32 {
        let factor = normalized_depth.powf(self.decay_exponent);
        self.edge_density + (self.center_density - self.edge_density) * factor
    }

    /// Select a random feature based on depth and weights
    pub fn select_feature(&self, normalized_depth: f32, rng: &mut ChaCha8Rng) -> Option<LocalFeature> {
        // Filter features available at this depth
        let available: Vec<&FeatureEntry> = self.features.iter()
            .filter(|e| !e.center_only || normalized_depth >= e.min_depth)
            .collect();

        if available.is_empty() {
            return None;
        }

        // Calculate total weight
        let total_weight: f32 = available.iter().map(|e| e.weight).sum();
        if total_weight <= 0.0 {
            return None;
        }

        // Random selection
        let mut roll = rng.gen::<f32>() * total_weight;
        for entry in &available {
            roll -= entry.weight;
            if roll <= 0.0 {
                return Some(entry.feature);
            }
        }

        // Fallback to last feature
        available.last().map(|e| e.feature)
    }
}

/// Get a feature pool for a specific biome
pub fn get_biome_feature_pool(biome: ExtendedBiome) -> FeaturePool {
    use ExtendedBiome::*;
    use LocalFeature::*;

    match biome {
        // Forests have trees at center, bushes everywhere
        TemperateForest | BorealForest => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.25, 0.05, 1.5);
            pool.add_center_feature(Tree { height: 5 }, 5.0, 0.3);  // Trees need depth > 0.3
            pool.add_feature(Bush, 3.0);
            pool.add_feature(Mushroom, 1.0);
            pool
        }

        TropicalRainforest | TemperateRainforest => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.35, 0.10, 1.2);
            pool.add_center_feature(Tree { height: 6 }, 6.0, 0.2);
            pool.add_center_feature(Tree { height: 10 }, 2.0, 0.5);  // Tall trees deep in forest
            pool.add_feature(Bush, 4.0);
            pool.add_feature(Mushroom, 2.0);
            pool
        }

        // Grasslands have scattered trees, more bushes
        TemperateGrassland | Savanna => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.12, 0.03, 2.0);
            pool.add_center_feature(Tree { height: 4 }, 1.0, 0.5);  // Rare trees only at center
            pool.add_feature(Bush, 3.0);
            pool.add_feature(Bush, 5.0);  // More bushes to simulate tall grass
            pool
        }

        // Deserts have sparse vegetation
        Desert => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.03, 0.01, 1.0);
            pool.add_center_feature(Bush, 2.0, 0.4);  // Sparse bushes
            pool.add_feature(Rubble, 3.0);  // Dead vegetation as rubble
            pool.add_feature(Boulder, 1.0);
            pool
        }

        // Tundra has minimal vegetation
        Tundra | AlpineTundra => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.05, 0.01, 1.5);
            pool.add_feature(Bush, 1.0);
            pool.add_feature(Boulder, 2.0);
            pool.add_feature(Mushroom, 1.0);  // Lichen/moss as mushroom
            pool
        }

        // Mountains have rocks
        SnowyPeaks => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.08, 0.03, 1.0);
            pool.add_feature(Boulder, 5.0);
            pool.add_feature(Rubble, 3.0);
            pool
        }

        // Swamps have unique vegetation
        Swamp | Marsh | Bog => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.20, 0.08, 1.3);
            pool.add_center_feature(Tree { height: 4 }, 2.0, 0.3);
            pool.add_feature(Bush, 2.0);
            pool.add_feature(GiantMushroom, 2.0);  // Swamp vegetation
            pool.add_feature(Mushroom, 3.0);
            pool
        }

        // Crystal/fantasy biomes
        CrystalForest | CrystalWasteland => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.15, 0.05, 1.5);
            pool.add_center_feature(Stalagmite, 4.0, 0.3);  // Crystal formations
            pool.add_feature(Boulder, 2.0);
            pool
        }

        // Volcanic areas
        VolcanicWasteland | Ashlands => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.06, 0.02, 1.0);
            pool.add_feature(Boulder, 3.0);
            pool.add_feature(Rubble, 4.0);
            pool
        }

        // Default for other biomes
        _ => {
            let mut pool = FeaturePool::new(biome)
                .with_density(0.10, 0.02, 1.5);
            pool.add_feature(Bush, 2.0);
            pool.add_feature(Boulder, 1.0);
            pool
        }
    }
}

/// Apply feature pool with distance decay to a chunk
pub fn apply_feature_pool(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    biome: ExtendedBiome,
    edge_depths: &[[f32; LOCAL_SIZE]; LOCAL_SIZE],  // Normalized depth map (0=edge, 1=center)
    rng: &mut ChaCha8Rng,
) {
    let pool = get_biome_feature_pool(biome);
    let surface_z = geology.surface_z;

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            let depth = edge_depths[y][x];
            let density = pool.density_at_depth(depth);

            // Roll for feature placement
            if rng.gen::<f32>() > density {
                continue;
            }

            // Find actual surface at this position
            let mut local_surface_z = surface_z;
            for z in (chunk.z_min..=surface_z + 4).rev() {
                let tile = chunk.get(x, y, z);
                if tile.terrain.is_solid() {
                    local_surface_z = z;
                    break;
                }
            }

            // Check if surface is suitable
            let tile = chunk.get(x, y, local_surface_z);
            if !tile.terrain.is_solid() || tile.terrain.is_water() {
                continue;
            }

            // Select and place feature
            if let Some(feature) = pool.select_feature(depth, rng) {
                let mut new_tile = chunk.get(x, y, local_surface_z + 1).clone();
                new_tile.feature = feature;
                chunk.set(x, y, local_surface_z + 1, new_tile);
            }
        }
    }
}

/// Compute edge depth map for a chunk (0=edge, 1=center)
pub fn compute_edge_depth_map(blend_width: usize) -> [[f32; LOCAL_SIZE]; LOCAL_SIZE] {
    let mut depths = [[0.0f32; LOCAL_SIZE]; LOCAL_SIZE];
    let max_dist = (LOCAL_SIZE / 2) as f32;

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Distance from nearest edge
            let dist_x = x.min(LOCAL_SIZE - 1 - x) as f32;
            let dist_y = y.min(LOCAL_SIZE - 1 - y) as f32;
            let dist = dist_x.min(dist_y);

            // Normalize to 0-1 range
            depths[y][x] = (dist / blend_width as f32).min(1.0);
        }
    }

    depths
}

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

/// Get a special feature for a biome using deterministic position-based selection.
/// This ensures the same position always generates the same feature across chunk boundaries.
fn get_special_feature_deterministic(biome: ExtendedBiome, seed: u64) -> LocalFeature {
    // Use seed to generate deterministic random values
    let rand_val = ((seed >> 33) as u32) as f32 / (u32::MAX as f32);
    let rand_choice = ((seed >> 16) as u32) % 10;

    match biome {
        // Crystal biomes get crystal features
        ExtendedBiome::CrystalForest |
        ExtendedBiome::CrystalWasteland |
        ExtendedBiome::CrystalDepths => LocalFeature::Crystal,

        // Mushroom biomes get mushroom features
        ExtendedBiome::MushroomForest |
        ExtendedBiome::FungalBloom => {
            if rand_val < 0.3 {
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
            if rand_val < 0.5 {
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
            if rand_val < 0.3 {
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
            match rand_choice % 5 {
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
            if rand_val < 0.5 {
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

/// Calculate radial blend factors for full-chunk biome blending.
///
/// Instead of only blending at 8-pixel edges, this uses radial distance
/// from corners to blend across the entire chunk smoothly.
///
/// Returns weights for (NW, NE, SW, SE) corners based on radial distance.
fn radial_corner_weights(local_x: usize, local_y: usize, local_size: usize, noise_offset: f32) -> (f32, f32, f32, f32) {
    // Normalized position (0-1)
    // Use (local_size - 1) so edges reach exactly 0.0 and 1.0 for boundary continuity
    let max_coord = (local_size - 1).max(1) as f32;
    let u = local_x as f32 / max_coord;
    let v = local_y as f32 / max_coord;

    // Calculate radial distance from each corner
    let nw_dist = (u * u + v * v).sqrt();
    let ne_dist = ((1.0 - u) * (1.0 - u) + v * v).sqrt();
    let sw_dist = (u * u + (1.0 - v) * (1.0 - v)).sqrt();
    let se_dist = ((1.0 - u) * (1.0 - u) + (1.0 - v) * (1.0 - v)).sqrt();

    // Inverse distance weighting with noise perturbation
    // The noise_offset shifts the weights slightly for more organic transitions
    let epsilon = 0.1 + noise_offset.abs() * 0.05;  // Prevent division by zero
    let nw_w = 1.0 / (nw_dist + epsilon);
    let ne_w = 1.0 / (ne_dist + epsilon);
    let sw_w = 1.0 / (sw_dist + epsilon);
    let se_w = 1.0 / (se_dist + epsilon);

    // Normalize weights
    let total = nw_w + ne_w + sw_w + se_w;
    (nw_w / total, ne_w / total, sw_w / total, se_w / total)
}

/// Interpolate biome configuration across the entire chunk using radial falloff.
///
/// This creates smooth biome transitions across the whole chunk area,
/// not just the 8-pixel edges, eliminating rectangular biome patterns.
///
/// # Arguments
/// * `primary_config` - Config for the primary biome at chunk center
/// * `corner_biomes` - Biomes at the 4 corners (2x2 grid)
/// * `local_x`, `local_y` - Position within chunk
/// * `local_size` - Chunk size (usually 48)
/// * `noise_offset` - World-continuous noise for variation
fn get_interpolated_config(
    primary_config: &BiomeTerrainConfig,
    corner_biomes: &[[ExtendedBiome; 2]; 2],
    local_x: usize,
    local_y: usize,
    local_size: usize,
    noise_offset: f32,
) -> BiomeTerrainConfig {
    let (nw_w, ne_w, sw_w, se_w) = radial_corner_weights(local_x, local_y, local_size, noise_offset);

    // Get configs for each corner
    let nw_config = get_biome_config(corner_biomes[0][0]);
    let ne_config = get_biome_config(corner_biomes[0][1]);
    let sw_config = get_biome_config(corner_biomes[1][0]);
    let se_config = get_biome_config(corner_biomes[1][1]);

    // Interpolate density values
    let tree_density =
        nw_config.tree_density * nw_w +
        ne_config.tree_density * ne_w +
        sw_config.tree_density * sw_w +
        se_config.tree_density * se_w;

    let bush_density =
        nw_config.bush_density * nw_w +
        ne_config.bush_density * ne_w +
        sw_config.bush_density * sw_w +
        se_config.bush_density * se_w;

    let boulder_density =
        nw_config.boulder_density * nw_w +
        ne_config.boulder_density * ne_w +
        sw_config.boulder_density * sw_w +
        se_config.boulder_density * se_w;

    let water_chance =
        nw_config.water_chance * nw_w +
        ne_config.water_chance * ne_w +
        sw_config.water_chance * sw_w +
        se_config.water_chance * se_w;

    let terrain_variation = (
        nw_config.terrain_variation as f32 * nw_w +
        ne_config.terrain_variation as f32 * ne_w +
        sw_config.terrain_variation as f32 * sw_w +
        se_config.terrain_variation as f32 * se_w
    ).round() as i16;

    BiomeTerrainConfig {
        tree_density,
        bush_density,
        boulder_density,
        water_chance,
        terrain_variation,
        // Keep primary config for non-interpolatable fields
        surface_terrain: primary_config.surface_terrain,
        surface_material: primary_config.surface_material,
        special_feature_chance: primary_config.special_feature_chance,
        has_dense_vegetation: primary_config.has_dense_vegetation,
        soil_type: primary_config.soil_type,
        stone_type: primary_config.stone_type,
    }
}

/// Blend two terrain types based on weight using RNG
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

/// Blend two terrain types deterministically based on position
/// This ensures seamless blending across chunk boundaries
fn blend_terrain_deterministic(
    primary: LocalTerrain,
    secondary: LocalTerrain,
    weight: f32,
    pos_seed: u64,
    variant: u32,
) -> LocalTerrain {
    let random = position_random(pos_seed, variant);
    if random < weight {
        secondary
    } else {
        primary
    }
}

/// Blend two materials based on weight using RNG
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

/// Blend two materials deterministically based on position
fn blend_material_deterministic(
    primary: Material,
    secondary: Material,
    weight: f32,
    pos_seed: u64,
    variant: u32,
) -> Material {
    let random = position_random(pos_seed, variant);
    if random < weight {
        secondary
    } else {
        primary
    }
}

/// Generate blended surface terrain considering adjacent biomes
///
/// If a `feather_map` is provided along with world coordinates, the blending
/// will use precomputed biome boundary data for smoother, more natural transitions.
/// Falls back to hard-coded edge blending when feather_map is None.
///
/// Uses bilinear interpolation of surface heights and world-coordinate noise
/// for seamless terrain across chunk boundaries.
///
/// The `coastline_noise` parameter should be a world-seeded Perlin noise generator
/// for creating natural-looking coastline shapes that flow across chunk boundaries.
pub fn generate_blended_biome_surface(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    primary_config: &BiomeTerrainConfig,
    adjacent: &AdjacentBiomes,
    surface_noise: &Perlin,
    coastline_noise: &Perlin,  // World-seeded noise for coastline shapes
    _rng: &mut ChaCha8Rng,  // Kept for API compatibility, but blending is now deterministic
    feather_map: Option<&BiomeFeatherMap>,
    world_coords: Option<(usize, usize)>,
    corner_heights: &CornerHeights,
    corner_biomes: &[[ExtendedBiome; 2]; 2],
    world_seed: u64,
) {
    // Unused parameters kept for API compatibility
    let _ = (adjacent, feather_map);

    // Get world coordinates for world-coordinate noise
    let (world_x, world_y) = world_coords.unwrap_or((chunk.world_x, chunk.world_y));

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Get interpolated base surface_z for this position (seamless across chunks)
            let base_surface_z = interpolate_surface_z(corner_heights, x, y, LOCAL_SIZE);

            // Calculate surface variation using world-coordinate noise (continuous across chunks)
            // Scale: 0.02 gives features about 50 tiles wide, spanning chunk boundaries
            let [nx, ny] = world_noise_coord(world_x, world_y, x, y, 0.02);
            let variation = (surface_noise.get([nx, ny]) * primary_config.terrain_variation as f64) as i16;
            let local_z = (base_surface_z + variation).clamp(chunk.z_min, chunk.z_max);

            // Set air above surface (use interpolated base + max variation as upper bound)
            let max_surface = base_surface_z + primary_config.terrain_variation;
            for z in (local_z + 1)..=max_surface {
                if z >= chunk.z_min && z <= chunk.z_max {
                    chunk.set(x, y, z, LocalTile::air());
                }
            }

            // Calculate coastline info for water/land blending across chunk boundaries
            // Uses world-continuous noise to create natural-looking coastline shapes
            let coastline = calculate_coastline_info_with_noise(
                corner_biomes, corner_heights, world_x, world_y, x, y, LOCAL_SIZE, coastline_noise
            );

            // Determine blended terrain and material using world-continuous noise
            // This creates natural-looking transitions that flow across chunk boundaries
            let mut terrain = primary_config.surface_terrain;
            let mut material = primary_config.surface_material;

            // Get world-continuous noise values for terrain transitions
            // Different scales for different features
            let [coast_nx, coast_ny] = world_noise_coord(world_x, world_y, x, y, 0.025);  // ~40 tile features
            let [detail_nx, detail_ny] = world_noise_coord(world_x, world_y, x, y, 0.08); // ~12 tile features

            // Multi-octave noise for natural coastline shapes
            let coast_noise1 = coastline_noise.get([coast_nx, coast_ny]) as f32;
            let coast_noise2 = coastline_noise.get([coast_nx * 2.0 + 50.0, coast_ny * 2.0 + 50.0]) as f32 * 0.5;
            let coast_noise = coast_noise1 + coast_noise2;  // Range roughly -1.5 to 1.5

            // Detail noise for beach/water edge variation
            let detail_noise = coastline_noise.get([detail_nx + 200.0, detail_ny + 200.0]) as f32;

            // Calculate absolute position for deterministic dithering
            let abs_x = world_x * LOCAL_SIZE + x;
            let abs_y = world_y * LOCAL_SIZE + y;
            let pos_seed = feature_seed(world_seed, abs_x, abs_y);

            // The water_factor from calculate_noise_water_factor is already noise-driven
            // Apply additional noise perturbation for extra organic variation
            let noise_adjusted_water_factor = (coastline.water_factor + coast_noise * 0.15).clamp(0.0, 1.0);

            // Position-based dithering for soft transitions
            // This eliminates visible threshold bands by varying thresholds per-position
            let dither = position_random(pos_seed, 100) * 0.15 - 0.075;  // ±0.075
            let depth_dither = position_random(pos_seed, 101) * 0.12;     // 0 to 0.12
            let beach_dither = position_random(pos_seed, 102) * 0.08 - 0.04;  // ±0.04

            // Soft thresholds with position-based dithering
            let deep_water_threshold = 0.72 + dither;
            let shallow_water_threshold = 0.50 + dither;
            let beach_threshold = 0.30 + beach_dither;

            // Determine terrain based on dithered thresholds
            // IMPORTANT: This overrides the biome default when water_factor disagrees
            if noise_adjusted_water_factor > deep_water_threshold {
                // Deep water - use dither for depth variation instead of noise
                if depth_dither > 0.06 {
                    terrain = LocalTerrain::DeepWater;
                } else {
                    terrain = LocalTerrain::ShallowWater;
                }
                material = Material::Water;
            } else if noise_adjusted_water_factor > shallow_water_threshold {
                // Shallow water zone with dithered boundary
                terrain = LocalTerrain::ShallowWater;
                material = Material::Water;
            } else if noise_adjusted_water_factor > beach_threshold {
                // Beach/sand zone with dithered edge
                terrain = LocalTerrain::Sand;
                material = Material::Sand;
            } else {
                // Land zone - OVERRIDE water biome defaults to land
                // This ensures CoastalWater/Ocean biomes become land when water_factor is low
                if terrain.is_water() {
                    // Find the nearest non-water corner biome to get appropriate land terrain
                    let land_configs: Vec<_> = [
                        corner_biomes[0][0], corner_biomes[0][1],
                        corner_biomes[1][0], corner_biomes[1][1],
                    ].iter()
                        .filter(|&&b| !is_water_biome(b))
                        .map(|&b| get_biome_config(b))
                        .collect();

                    if let Some(land_config) = land_configs.first() {
                        terrain = land_config.surface_terrain;
                        material = land_config.surface_material;
                    } else {
                        // All corners are water biomes but water_factor is low
                        // Use grass as fallback
                        terrain = LocalTerrain::Grass;
                        material = Material::Grass;
                    }
                }
            }

            // Apply radial blending from corner biomes for non-water terrain
            // This uses full-chunk blending instead of 8-pixel edge strips
            if noise_adjusted_water_factor < beach_threshold && !terrain.is_water() {
                // Use radial corner weights for smooth full-chunk blending
                let blend_noise = coastline_noise.get([detail_nx + 400.0, detail_ny + 400.0]) as f32;
                let (nw_w, ne_w, sw_w, se_w) = radial_corner_weights(x, y, LOCAL_SIZE, blend_noise);

                // Get configs for each corner
                let nw_config = get_biome_config(corner_biomes[0][0]);
                let ne_config = get_biome_config(corner_biomes[0][1]);
                let sw_config = get_biome_config(corner_biomes[1][0]);
                let se_config = get_biome_config(corner_biomes[1][1]);

                // Check if corners have different non-water biomes (need blending)
                let biomes_differ = corner_biomes[0][0] != corner_biomes[0][1]
                    || corner_biomes[0][0] != corner_biomes[1][0]
                    || corner_biomes[0][0] != corner_biomes[1][1];

                if biomes_differ {
                    // Collect non-water corner configs with their weights
                    let mut candidates: Vec<(&BiomeTerrainConfig, f32)> = Vec::with_capacity(4);
                    if !nw_config.surface_terrain.is_water() {
                        candidates.push((&nw_config, nw_w));
                    }
                    if !ne_config.surface_terrain.is_water() {
                        candidates.push((&ne_config, ne_w));
                    }
                    if !sw_config.surface_terrain.is_water() {
                        candidates.push((&sw_config, sw_w));
                    }
                    if !se_config.surface_terrain.is_water() {
                        candidates.push((&se_config, se_w));
                    }

                    if candidates.len() > 1 {
                        // Position-based selection from weighted candidates
                        // Use radial weights + position dither to create organic transitions
                        let blend_roll = position_random(pos_seed, 200);
                        let mut cumulative = 0.0f32;
                        let total_weight: f32 = candidates.iter().map(|(_, w)| w).sum();

                        for (config, weight) in &candidates {
                            cumulative += weight / total_weight;
                            if blend_roll < cumulative {
                                terrain = config.surface_terrain;
                                material = config.surface_material;
                                break;
                            }
                        }
                    }
                }
            }

            // Create surface tile
            let mut surface_tile = LocalTile::new(terrain, material);
            surface_tile.temperature = geology.temperature;

            // Check for inland water pools using world-continuous noise
            // Only apply on land areas (not coastlines or water biomes)
            if noise_adjusted_water_factor < beach_threshold && !terrain.is_water() {
                // Use radial interpolation for water_chance
                let blend_noise_for_pools = coastline_noise.get([detail_nx + 600.0, detail_ny + 600.0]) as f32;
                let interp_config = get_interpolated_config(
                    primary_config, corner_biomes, x, y, LOCAL_SIZE, blend_noise_for_pools
                );
                let water_chance = interp_config.water_chance;

                // Use world-continuous noise for water pool regions
                // Different offset than coastlines for distinct pool patterns
                if water_chance > 0.0 {
                    let [pool_nx, pool_ny] = world_noise_coord(world_x, world_y, x, y, 0.06);
                    let pool_noise = coastline_noise.get([pool_nx + 500.0, pool_ny + 500.0]) as f32;

                    // Pool forms where noise exceeds threshold (adjusted by water_chance)
                    let pool_threshold = 0.7 - water_chance;  // water_chance 0.3 -> threshold 0.4

                    // Also require low elevation (variation < 0) for pools
                    if pool_noise > pool_threshold && variation < 0 {
                        surface_tile = LocalTile::new(
                            if pool_noise > pool_threshold + 0.15 {
                                LocalTerrain::DeepWater
                            } else {
                                LocalTerrain::ShallowWater
                            },
                            Material::Water,
                        );
                    }
                }
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
///
/// If a `feather_map` is provided along with world coordinates, feature densities
/// will be scaled based on precomputed biome boundary data.
///
/// Uses position-based deterministic placement for seamless features across chunk boundaries.
pub fn add_blended_biome_features(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    primary_config: &BiomeTerrainConfig,
    _adjacent: &AdjacentBiomes,  // Kept for API compatibility
    _rng: &mut ChaCha8Rng,
    _feather_map: Option<&BiomeFeatherMap>,  // Kept for API compatibility
    world_coords: Option<(usize, usize)>,
    world_seed: u64,
    corner_biomes: Option<&[[ExtendedBiome; 2]; 2]>,  // Added for radial blending
) {
    let surface_z = geology.surface_z;

    // Get world coordinates for position-based feature placement
    let (world_x, world_y) = world_coords.unwrap_or((chunk.world_x, chunk.world_y));

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Calculate absolute position for deterministic feature placement
            let abs_x = world_x * LOCAL_SIZE + x;
            let abs_y = world_y * LOCAL_SIZE + y;
            let pos_seed = feature_seed(world_seed, abs_x, abs_y);

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

            // Get interpolated feature densities using radial blending
            let (tree_density, bush_density, boulder_density) = if let Some(corners) = corner_biomes {
                // Use position-based noise offset for variation
                let noise_offset = position_random(pos_seed, 50) * 0.2 - 0.1;
                let interp = get_interpolated_config(primary_config, corners, x, y, LOCAL_SIZE, noise_offset);
                (interp.tree_density, interp.bush_density, interp.boulder_density)
            } else {
                (primary_config.tree_density, primary_config.bush_density, primary_config.boulder_density)
            };

            // Place features using position-based deterministic placement
            // This ensures the same position always generates the same feature
            // across chunk boundaries
            if tree_density > 0.0 && should_place_feature(pos_seed, tree_density) {
                // Use position-based random for tree height (variant 0)
                let height = position_random_range(pos_seed, 0, 3, 7) as u8;
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Tree { height };
                continue;
            }

            // Use different seed variants for different feature types
            let bush_seed = feature_seed(world_seed.wrapping_add(1), abs_x, abs_y);
            if bush_density > 0.0 && should_place_feature(bush_seed, bush_density) {
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Bush;
                continue;
            }

            let boulder_seed = feature_seed(world_seed.wrapping_add(2), abs_x, abs_y);
            if boulder_density > 0.0 && should_place_feature(boulder_seed, boulder_density) {
                chunk.get_mut(x, y, local_surface_z).feature = LocalFeature::Boulder;
                continue;
            }

            // Special features (use position-based placement)
            if primary_config.special_feature_chance > 0.0 {
                let special_seed = feature_seed(world_seed.wrapping_add(3), abs_x, abs_y);
                if should_place_feature(special_seed, primary_config.special_feature_chance) {
                    let feature = get_special_feature_deterministic(geology.biome, special_seed);
                    chunk.get_mut(x, y, local_surface_z).feature = feature;
                }
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
