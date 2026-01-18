//! Territory management for tribes

pub mod expansion;

use crate::simulation::types::TileCoord;
use crate::simulation::simulation::SimulationState;
use crate::simulation::params::SimulationParams;
use crate::world::WorldData;
use rand::Rng;

pub use expansion::process_expansion_tick;

/// Territory statistics for a tribe
#[derive(Clone, Debug, Default)]
pub struct TerritoryStats {
    pub total_tiles: usize,
    pub land_tiles: usize,
    pub coastal_tiles: usize,
    pub mountain_tiles: usize,
    pub forest_tiles: usize,
    pub resource_value: f32,
}

/// Calculate territory statistics
pub fn calculate_territory_stats(
    territory: &std::collections::HashSet<TileCoord>,
    world: &WorldData,
) -> TerritoryStats {
    use crate::biomes::ExtendedBiome;

    let mut stats = TerritoryStats {
        total_tiles: territory.len(),
        ..Default::default()
    };

    for coord in territory {
        let elevation = *world.heightmap.get(coord.x, coord.y);
        let biome = *world.biomes.get(coord.x, coord.y);

        if elevation >= 0.0 {
            stats.land_tiles += 1;
        }

        // Check for coastal (land adjacent to water)
        if elevation >= 0.0 && is_coastal(coord, world) {
            stats.coastal_tiles += 1;
        }

        // Categorize by biome type
        match biome {
            ExtendedBiome::SnowyPeaks
            | ExtendedBiome::AlpineTundra
            | ExtendedBiome::RazorPeaks => {
                stats.mountain_tiles += 1;
            }
            ExtendedBiome::BorealForest
            | ExtendedBiome::TemperateForest
            | ExtendedBiome::TropicalForest
            | ExtendedBiome::TropicalRainforest => {
                stats.forest_tiles += 1;
            }
            _ => {}
        }

        // Add resource value
        stats.resource_value += estimate_tile_value(biome);
    }

    stats
}

/// Check if a tile is coastal (land adjacent to water)
fn is_coastal(coord: &TileCoord, world: &WorldData) -> bool {
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (coord.x as i32 + dx).rem_euclid(world.heightmap.width as i32) as usize;
            let ny = (coord.y as i32 + dy).clamp(0, world.heightmap.height as i32 - 1) as usize;

            let neighbor_elevation = *world.heightmap.get(nx, ny);
            if neighbor_elevation < 0.0 {
                return true;
            }
        }
    }
    false
}

/// Estimate the resource value of a tile based on biome
fn estimate_tile_value(biome: crate::biomes::ExtendedBiome) -> f32 {
    use crate::biomes::ExtendedBiome;

    match biome {
        // High value
        ExtendedBiome::Oasis => 3.0,
        ExtendedBiome::TemperateForest | ExtendedBiome::TemperateGrassland => 2.5,
        ExtendedBiome::TropicalForest | ExtendedBiome::Savanna => 2.0,

        // Medium value
        ExtendedBiome::BorealForest | ExtendedBiome::Foothills => 1.5,
        ExtendedBiome::Swamp | ExtendedBiome::Marsh => 1.2,

        // Low value but strategic
        ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineTundra => 1.0,
        ExtendedBiome::Desert | ExtendedBiome::Tundra => 0.8,

        // Very low
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::Ashlands => 0.5,

        // Default
        _ => 1.0,
    }
}
