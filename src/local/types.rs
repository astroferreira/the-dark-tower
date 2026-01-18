//! Core types for local map generation.
//!
//! Defines the LocalMap and LocalTile structures that represent
//! detailed playable areas expanded from overworld tiles.

use super::terrain::{LocalTerrainType, LocalFeature};

/// Default size for local maps
pub const DEFAULT_LOCAL_MAP_SIZE: usize = 64;

/// A single tile in a local map
#[derive(Clone, Debug)]
pub struct LocalTile {
    /// The base terrain type
    pub terrain: LocalTerrainType,
    /// Optional feature placed on the terrain
    pub feature: Option<LocalFeature>,
    /// Whether this tile can be walked through
    pub walkable: bool,
    /// Movement cost multiplier (1.0 = normal, higher = slower)
    pub movement_cost: f32,
    /// Local elevation offset from overworld elevation
    pub elevation_offset: f32,
}

impl Default for LocalTile {
    fn default() -> Self {
        Self {
            terrain: LocalTerrainType::default(),
            feature: None,
            walkable: true,
            movement_cost: 1.0,
            elevation_offset: 0.0,
        }
    }
}

impl LocalTile {
    /// Create a new tile with just terrain
    pub fn new(terrain: LocalTerrainType) -> Self {
        let walkable = terrain.is_walkable();
        let movement_cost = terrain.movement_cost();
        Self {
            terrain,
            feature: None,
            walkable,
            movement_cost,
            elevation_offset: 0.0,
        }
    }

    /// Create a tile with terrain and feature
    pub fn with_feature(terrain: LocalTerrainType, feature: LocalFeature) -> Self {
        let mut tile = Self::new(terrain);
        tile.set_feature(feature);
        tile
    }

    /// Set a feature on this tile, updating walkability and movement cost
    pub fn set_feature(&mut self, feature: LocalFeature) {
        self.feature = Some(feature);
        if feature.blocks_movement() {
            self.walkable = false;
            self.movement_cost = f32::INFINITY;
        } else {
            self.movement_cost = self.terrain.movement_cost() + feature.movement_cost_modifier();
        }
    }

    /// Get the RGB color for rendering (feature color overrides terrain if present)
    pub fn color(&self) -> (u8, u8, u8) {
        if let Some(feature) = self.feature {
            feature.color()
        } else {
            self.terrain.color()
        }
    }

    /// Get ASCII character for display (feature takes precedence)
    pub fn ascii_char(&self) -> char {
        if let Some(feature) = self.feature {
            feature.ascii_char()
        } else {
            self.terrain.ascii_char()
        }
    }

    /// Get brightness multiplier based on elevation offset (0.7 to 1.3 range)
    /// Higher elevation = brighter, lower = darker
    pub fn elevation_brightness(&self) -> f32 {
        // Map elevation_offset from roughly -1.0..1.0 to 0.7..1.3
        let brightness = 1.0 + self.elevation_offset * 0.3;
        brightness.clamp(0.7, 1.3)
    }

    /// Get terrain color (not feature)
    pub fn terrain_color(&self) -> (u8, u8, u8) {
        self.terrain.color()
    }

    /// Get feature color if present
    pub fn feature_color(&self) -> Option<(u8, u8, u8)> {
        self.feature.map(|f| f.color())
    }
}

/// A detailed local map generated from an overworld tile
#[derive(Clone)]
pub struct LocalMap {
    /// Width of the local map in tiles
    pub width: usize,
    /// Height of the local map in tiles
    pub height: usize,
    /// The tiles in row-major order
    tiles: Vec<LocalTile>,
    /// World X coordinate of the source overworld tile
    pub world_x: usize,
    /// World Y coordinate of the source overworld tile
    pub world_y: usize,
    /// Seed used to generate this local map
    pub seed: u64,
}

impl LocalMap {
    /// Create a new local map with default tiles
    pub fn new(width: usize, height: usize, world_x: usize, world_y: usize, seed: u64) -> Self {
        Self {
            width,
            height,
            tiles: vec![LocalTile::default(); width * height],
            world_x,
            world_y,
            seed,
        }
    }

    /// Get tile at (x, y)
    pub fn get(&self, x: usize, y: usize) -> &LocalTile {
        &self.tiles[y * self.width + x]
    }

    /// Get mutable tile at (x, y)
    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut LocalTile {
        &mut self.tiles[y * self.width + x]
    }

    /// Set tile at (x, y)
    pub fn set(&mut self, x: usize, y: usize, tile: LocalTile) {
        self.tiles[y * self.width + x] = tile;
    }

    /// Set terrain at (x, y)
    pub fn set_terrain(&mut self, x: usize, y: usize, terrain: LocalTerrainType) {
        let tile = self.get_mut(x, y);
        tile.terrain = terrain;
        tile.walkable = terrain.is_walkable();
        tile.movement_cost = terrain.movement_cost();
    }

    /// Set feature at (x, y)
    pub fn set_feature(&mut self, x: usize, y: usize, feature: LocalFeature) {
        self.get_mut(x, y).set_feature(feature);
    }

    /// Iterate over all tiles with coordinates
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, &LocalTile)> {
        self.tiles.iter().enumerate().map(move |(idx, tile)| {
            let x = idx % self.width;
            let y = idx / self.width;
            (x, y, tile)
        })
    }

    /// Get 4-connected neighbors of a tile
    pub fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(4);

        if x > 0 {
            result.push((x - 1, y));
        }
        if x < self.width - 1 {
            result.push((x + 1, y));
        }
        if y > 0 {
            result.push((x, y - 1));
        }
        if y < self.height - 1 {
            result.push((x, y + 1));
        }

        result
    }

    /// Check if coordinates are valid
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }
}

/// Information about neighboring overworld tiles for edge blending
#[derive(Clone, Debug)]
pub struct NeighborInfo {
    /// North neighbor biome (y - 1)
    pub north: Option<crate::biomes::ExtendedBiome>,
    /// South neighbor biome (y + 1)
    pub south: Option<crate::biomes::ExtendedBiome>,
    /// East neighbor biome (x + 1)
    pub east: Option<crate::biomes::ExtendedBiome>,
    /// West neighbor biome (x - 1)
    pub west: Option<crate::biomes::ExtendedBiome>,
}

impl NeighborInfo {
    /// Create neighbor info with no neighbors (map edges)
    pub fn none() -> Self {
        Self {
            north: None,
            south: None,
            east: None,
            west: None,
        }
    }

    /// Check if any neighbors are different from the center biome
    pub fn has_different_neighbor(&self, center: crate::biomes::ExtendedBiome) -> bool {
        [self.north, self.south, self.east, self.west]
            .iter()
            .any(|n| n.map_or(false, |b| b != center))
    }
}
