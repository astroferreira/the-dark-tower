//! Tileset rendering system for pixel-art style map visualization

use image::{RgbImage, GenericImageView, DynamicImage};
use crate::climate::Biome;
use crate::tilemap::Tilemap;

/// Tile size in pixels (each tile is 32x32)
pub const TILE_SIZE: u32 = 32;

/// Number of tile columns in the tileset (2816 / 32 = 88)
pub const TILESET_COLS: u32 = 88;

/// Number of tile rows in the tileset (1536 / 32 = 48)
pub const TILESET_ROWS: u32 = 48;

/// A loaded tileset with individual tiles extracted
pub struct Tileset {
    /// Individual tiles indexed by (col, row)
    tiles: Vec<Vec<RgbImage>>,
}

impl Tileset {
    /// Load tileset from embedded bytes or file path
    pub fn load() -> Option<Self> {
        // Try to load from the docs folder
        let tileset_path = "docs/tileset.png";
        let img = image::open(tileset_path).ok()?;

        Some(Self::from_image(&img))
    }

    /// Create tileset from a DynamicImage
    pub fn from_image(img: &DynamicImage) -> Self {
        let mut tiles = Vec::with_capacity(TILESET_ROWS as usize);

        for row in 0..TILESET_ROWS {
            let mut row_tiles = Vec::with_capacity(TILESET_COLS as usize);
            for col in 0..TILESET_COLS {
                let x = col * TILE_SIZE;
                let y = row * TILE_SIZE;

                // Extract tile from tileset
                let tile = img.crop_imm(x, y, TILE_SIZE, TILE_SIZE).to_rgb8();
                row_tiles.push(tile);
            }
            tiles.push(row_tiles);
        }

        Self { tiles }
    }

    /// Get a tile by column and row
    pub fn get_tile(&self, col: usize, row: usize) -> Option<&RgbImage> {
        self.tiles.get(row)?.get(col)
    }

    /// Get tile coordinates for a biome
    /// Returns (col, row) in the tileset
    /// The tileset is 88x48 tiles organized in horizontal bands:
    /// - Rows 0-9: Water/ocean/beach
    /// - Rows 10-19: Grass/plains
    /// - Rows 20-29: Forest variations
    /// - Rows 30-39: Mountains/snow/desert
    /// - Rows 40-47: Rocky/volcanic/transitions
    pub fn biome_to_tile(biome: Biome, variation: u8) -> (usize, usize) {
        // Use variation (0-7) to add visual variety within each biome region
        let var = (variation % 8) as usize;

        match biome {
            // Ocean biomes - Rows 0-9 (water area)
            Biome::DeepOcean => (var, 1),           // Deep water tiles
            Biome::Ocean => (8 + var, 1),           // Regular ocean
            Biome::CoastalWater => (16 + var, 1),   // Shallow/coastal water

            // Cold biomes - snow/ice area (rows 30-39)
            Biome::Ice => (16 + var, 31),           // Ice/snow tiles
            Biome::Tundra => (24 + var, 31),        // Tundra (sparse snow)
            Biome::BorealForest => (8 + var, 22),   // Dark conifer forest

            // Temperate biomes - grass/forest area (rows 10-29)
            Biome::TemperateGrassland => (var, 10),       // Green grass
            Biome::TemperateForest => (8 + var, 18),      // Deciduous forest
            Biome::TemperateRainforest => (var, 20),      // Dense forest

            // Warm biomes - desert/savanna
            Biome::Desert => (32 + var, 32),              // Desert sand
            Biome::Savanna => (40 + var, 12),             // Dry grass/savanna
            Biome::TropicalForest => (16 + var, 20),      // Tropical forest
            Biome::TropicalRainforest => (var, 22),       // Dense rainforest

            // Mountain biomes (rows 30-39)
            Biome::AlpineTundra => (var, 32),             // Rocky alpine
            Biome::SnowyPeaks => (var, 30),               // Snow-capped mountains
        }
    }
}

/// Render a map using the tileset
/// Each cell in the heightmap becomes one tile
pub fn render_tileset_map(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    tileset: &Tileset,
) -> RgbImage {
    let map_width = heightmap.width;
    let map_height = heightmap.height;

    // Output image size
    let img_width = map_width as u32 * TILE_SIZE;
    let img_height = map_height as u32 * TILE_SIZE;

    let mut img = RgbImage::new(img_width, img_height);

    for y in 0..map_height {
        for x in 0..map_width {
            let elev = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);

            // Classify biome
            let biome = Biome::classify(elev, temp, moist);

            // Use position-based variation for visual interest
            let variation = ((x * 7 + y * 13) % 4) as u8;

            // Get tile coordinates
            let (tile_col, tile_row) = Tileset::biome_to_tile(biome, variation);

            // Get the tile
            if let Some(tile) = tileset.get_tile(tile_col, tile_row) {
                // Copy tile to output image
                let dest_x = x as u32 * TILE_SIZE;
                let dest_y = y as u32 * TILE_SIZE;

                for ty in 0..TILE_SIZE {
                    for tx in 0..TILE_SIZE {
                        let pixel = tile.get_pixel(tx, ty);
                        img.put_pixel(dest_x + tx, dest_y + ty, *pixel);
                    }
                }
            }
        }
    }

    img
}

/// Render a scaled-down version where multiple map cells share one tile
/// This is useful for large maps where 1:1 tile rendering would be too large
pub fn render_tileset_map_scaled(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    tileset: &Tileset,
    scale: usize, // How many map cells per tile
) -> RgbImage {
    let map_width = heightmap.width;
    let map_height = heightmap.height;

    // Number of tiles in output
    let tiles_x = (map_width + scale - 1) / scale;
    let tiles_y = (map_height + scale - 1) / scale;

    // Output image size
    let img_width = tiles_x as u32 * TILE_SIZE;
    let img_height = tiles_y as u32 * TILE_SIZE;

    let mut img = RgbImage::new(img_width, img_height);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            // Sample the center of this tile's region
            let sample_x = (tx * scale + scale / 2).min(map_width - 1);
            let sample_y = (ty * scale + scale / 2).min(map_height - 1);

            let elev = *heightmap.get(sample_x, sample_y);
            let temp = *temperature.get(sample_x, sample_y);
            let moist = *moisture.get(sample_x, sample_y);

            // Classify biome
            let biome = Biome::classify(elev, temp, moist);

            // Use position-based variation
            let variation = ((tx * 7 + ty * 13) % 4) as u8;

            // Get tile coordinates
            let (tile_col, tile_row) = Tileset::biome_to_tile(biome, variation);

            // Get and copy the tile
            if let Some(tile) = tileset.get_tile(tile_col, tile_row) {
                let dest_x = tx as u32 * TILE_SIZE;
                let dest_y = ty as u32 * TILE_SIZE;

                for py in 0..TILE_SIZE {
                    for px in 0..TILE_SIZE {
                        let pixel = tile.get_pixel(px, py);
                        img.put_pixel(dest_x + px, dest_y + py, *pixel);
                    }
                }
            }
        }
    }

    img
}
