//! Export local maps to PNG images.
//!
//! Generates all chunks in a region and exports them as a single seamless PNG image.

use image::{ImageBuffer, Rgb, RgbImage};
use std::path::Path;

use crate::world::WorldData;
use super::cache::ChunkCache;
use super::local::{LocalChunk, LocalTerrain, LocalFeature, Material};
use super::LOCAL_SIZE;

/// Export options for local map rendering
#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// Z-level to render (defaults to surface)
    pub z_level: Option<i16>,
    /// Whether to auto-detect surface z per tile
    pub auto_surface: bool,
    /// Whether to show features (trees, etc.)
    pub show_features: bool,
    /// Scale factor (1 = 1 pixel per tile, 2 = 2x2 pixels per tile)
    pub scale: u32,
    /// Whether to show grid lines between chunks
    pub show_chunk_grid: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            z_level: None,
            auto_surface: true,
            show_features: true,
            scale: 1,
            show_chunk_grid: false,
        }
    }
}

/// Get RGB color for a terrain type
fn terrain_color(terrain: &LocalTerrain) -> Rgb<u8> {
    match terrain {
        LocalTerrain::Air => Rgb([135, 206, 235]), // Sky blue
        LocalTerrain::Grass => Rgb([34, 139, 34]),  // Forest green
        LocalTerrain::Sand => Rgb([238, 214, 175]), // Sandy
        LocalTerrain::Mud => Rgb([139, 90, 43]),    // Brown
        LocalTerrain::Ice => Rgb([176, 224, 230]),  // Pale blue
        LocalTerrain::Snow => Rgb([255, 250, 250]), // Snow white
        LocalTerrain::Gravel => Rgb([128, 128, 128]), // Gray
        LocalTerrain::DenseVegetation => Rgb([0, 100, 0]), // Dark green
        LocalTerrain::ShallowWater => Rgb([64, 164, 223]), // Light blue
        LocalTerrain::DeepWater => Rgb([0, 105, 148]),     // Deep blue
        LocalTerrain::FlowingWater => Rgb([30, 144, 255]), // Dodger blue
        LocalTerrain::Magma => Rgb([255, 69, 0]),   // Orange-red
        LocalTerrain::Lava => Rgb([255, 69, 0]),    // Orange-red
        LocalTerrain::CaveFloor => Rgb([105, 105, 105]), // Dim gray
        LocalTerrain::CaveWall => Rgb([64, 64, 64]),     // Dark gray
        LocalTerrain::Soil { .. } => Rgb([139, 119, 101]), // Rosy brown
        LocalTerrain::Stone { .. } => Rgb([112, 128, 144]), // Slate gray
        LocalTerrain::StoneFloor => Rgb([169, 169, 169]), // Dark gray
        LocalTerrain::DirtFloor => Rgb([160, 82, 45]),    // Sienna
        LocalTerrain::WoodFloor => Rgb([205, 133, 63]),   // Peru
        LocalTerrain::Cobblestone => Rgb([128, 128, 128]), // Gray
        LocalTerrain::StoneWall => Rgb([105, 105, 105]),  // Dim gray
        LocalTerrain::BrickWall => Rgb([178, 34, 34]),    // Firebrick
        LocalTerrain::WoodWall => Rgb([139, 90, 43]),     // Saddle brown
        LocalTerrain::ConstructedFloor { material } => material_color(material),
        LocalTerrain::ConstructedWall { material } => {
            let base = material_color(material);
            // Darken walls slightly
            Rgb([base[0].saturating_sub(30), base[1].saturating_sub(30), base[2].saturating_sub(30)])
        }
    }
}

/// Get RGB color for a material
fn material_color(material: &Material) -> Rgb<u8> {
    match material {
        Material::Air => Rgb([135, 206, 235]),
        Material::Grass => Rgb([34, 139, 34]),
        Material::Dirt => Rgb([139, 90, 43]),
        Material::Sand => Rgb([238, 214, 175]),
        Material::Mud => Rgb([139, 90, 43]),
        Material::Ice => Rgb([176, 224, 230]),
        Material::Snow => Rgb([255, 250, 250]),
        Material::Stone => Rgb([112, 128, 144]),
        Material::Water => Rgb([64, 164, 223]),
        Material::Magma => Rgb([255, 69, 0]),
    }
}

/// Get feature overlay color (returns None if feature shouldn't modify color)
fn feature_color(feature: &LocalFeature) -> Option<Rgb<u8>> {
    match feature {
        LocalFeature::None => None,
        LocalFeature::Tree { .. } => Some(Rgb([0, 80, 0])),      // Dark green tree
        LocalFeature::Bush => Some(Rgb([34, 120, 34])),          // Bush green
        LocalFeature::Boulder => Some(Rgb([128, 128, 128])),     // Gray
        LocalFeature::Mushroom => Some(Rgb([255, 182, 193])),    // Pink
        LocalFeature::GiantMushroom => Some(Rgb([255, 105, 180])), // Hot pink
        LocalFeature::Stalactite | LocalFeature::Stalagmite => Some(Rgb([169, 169, 169])),
        LocalFeature::Crystal => Some(Rgb([138, 43, 226])),      // Blue violet
        LocalFeature::OreVein => Some(Rgb([255, 215, 0])),       // Gold
        LocalFeature::StairsUp | LocalFeature::StairsDown => Some(Rgb([160, 160, 160])),
        LocalFeature::RampUp | LocalFeature::RampDown => Some(Rgb([140, 140, 140])),
        LocalFeature::Ladder => Some(Rgb([139, 90, 43])),        // Brown
        LocalFeature::Torch => Some(Rgb([255, 165, 0])),         // Orange
        LocalFeature::Door { open } => {
            if *open {
                Some(Rgb([139, 90, 43]))  // Brown (open)
            } else {
                Some(Rgb([101, 67, 33]))  // Darker brown (closed)
            }
        }
        LocalFeature::Chest => Some(Rgb([218, 165, 32])),        // Goldenrod
        LocalFeature::Altar => Some(Rgb([255, 255, 255])),       // White
        LocalFeature::Pillar => Some(Rgb([192, 192, 192])),      // Silver
        LocalFeature::Rubble => Some(Rgb([105, 105, 105])),      // Dim gray
        LocalFeature::Table => Some(Rgb([139, 90, 43])),
        LocalFeature::Chair => Some(Rgb([139, 90, 43])),
        LocalFeature::Bed => Some(Rgb([255, 228, 196])),         // Bisque
        LocalFeature::Bookshelf => Some(Rgb([139, 69, 19])),     // Saddle brown
        LocalFeature::Barrel => Some(Rgb([160, 82, 45])),        // Sienna
        LocalFeature::WeaponRack => Some(Rgb([192, 192, 192])),
        LocalFeature::Fountain | LocalFeature::Well => Some(Rgb([64, 164, 223])),
        LocalFeature::Statue => Some(Rgb([192, 192, 192])),
        LocalFeature::Trap { hidden } => {
            if *hidden { None } else { Some(Rgb([255, 0, 0])) }
        }
        LocalFeature::Lever { active } => {
            if *active {
                Some(Rgb([0, 255, 0]))    // Green (active)
            } else {
                Some(Rgb([128, 128, 128])) // Gray (inactive)
            }
        }
    }
}

/// Get the color for a tile at a specific position
fn get_tile_color(chunk: &LocalChunk, x: usize, y: usize, z: i16, options: &ExportOptions) -> Rgb<u8> {
    let tile = chunk.get(x, y, z);

    // Get base terrain color
    let mut color = terrain_color(&tile.terrain);

    // Apply feature overlay if enabled
    if options.show_features {
        if let Some(feature_col) = feature_color(&tile.feature) {
            color = feature_col;
        }
    }

    color
}

/// Render a single chunk to a section of an image buffer
fn render_chunk_to_buffer(
    chunk: &LocalChunk,
    img: &mut RgbImage,
    chunk_offset_x: u32,
    chunk_offset_y: u32,
    options: &ExportOptions,
) {
    let scale = options.scale;

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            // Determine z-level to render
            let z = if options.auto_surface {
                chunk.find_surface_z_at(x, y)
            } else {
                options.z_level.unwrap_or(chunk.surface_z)
            };

            let color = get_tile_color(chunk, x, y, z, options);

            // Write pixels (with scaling)
            let px = chunk_offset_x + (x as u32 * scale);
            let py = chunk_offset_y + (y as u32 * scale);

            for sy in 0..scale {
                for sx in 0..scale {
                    let final_x = px + sx;
                    let final_y = py + sy;
                    if final_x < img.width() && final_y < img.height() {
                        img.put_pixel(final_x, final_y, color);
                    }
                }
            }
        }
    }

    // Draw chunk grid if enabled
    if options.show_chunk_grid {
        let chunk_size = LOCAL_SIZE as u32 * scale;
        let grid_color = Rgb([64, 64, 64]);

        // Draw left edge
        for y in 0..chunk_size {
            let py = chunk_offset_y + y;
            if chunk_offset_x < img.width() && py < img.height() {
                img.put_pixel(chunk_offset_x, py, grid_color);
            }
        }

        // Draw top edge
        for x in 0..chunk_size {
            let px = chunk_offset_x + x;
            if px < img.width() && chunk_offset_y < img.height() {
                img.put_pixel(px, chunk_offset_y, grid_color);
            }
        }
    }
}

/// Export a rectangular region of local maps to a PNG file.
///
/// # Arguments
/// * `world` - The world data
/// * `start_x`, `start_y` - Top-left world tile coordinates
/// * `width`, `height` - Number of world tiles (chunks) to include
/// * `path` - Output file path
/// * `options` - Export options
///
/// # Returns
/// The dimensions of the exported image (width, height)
pub fn export_local_region<P: AsRef<Path>>(
    world: &WorldData,
    start_x: usize,
    start_y: usize,
    width: usize,
    height: usize,
    path: P,
    options: &ExportOptions,
) -> Result<(u32, u32), ExportError> {
    let scale = options.scale;
    let img_width = (width * LOCAL_SIZE) as u32 * scale;
    let img_height = (height * LOCAL_SIZE) as u32 * scale;

    // Sanity check - don't create absurdly large images
    let max_pixels = 100_000_000u64; // 100 megapixels
    let total_pixels = img_width as u64 * img_height as u64;
    if total_pixels > max_pixels {
        return Err(ExportError::ImageTooLarge {
            requested_width: img_width,
            requested_height: img_height,
            max_pixels,
        });
    }

    let mut img: RgbImage = ImageBuffer::new(img_width, img_height);
    let mut cache = ChunkCache::new();

    // Track progress
    let total_chunks = width * height;
    let mut chunks_done = 0;

    for cy in 0..height {
        for cx in 0..width {
            let world_x = start_x + cx;
            let world_y = start_y + cy;

            // Skip if out of world bounds
            if world_x >= world.heightmap.width || world_y >= world.heightmap.height {
                continue;
            }

            // Generate/get chunk
            let chunk = cache.get_or_generate_local(world, world_x, world_y);

            // Calculate pixel offset for this chunk
            let chunk_offset_x = (cx * LOCAL_SIZE) as u32 * scale;
            let chunk_offset_y = (cy * LOCAL_SIZE) as u32 * scale;

            // Render chunk to buffer
            render_chunk_to_buffer(chunk, &mut img, chunk_offset_x, chunk_offset_y, options);

            chunks_done += 1;
            if chunks_done % 10 == 0 || chunks_done == total_chunks {
                eprintln!("Exported {}/{} chunks...", chunks_done, total_chunks);
            }
        }
    }

    // Save image
    img.save(&path).map_err(|e| ExportError::SaveFailed(e.to_string()))?;

    Ok((img_width, img_height))
}

/// Export all local maps for the entire world (WARNING: can be very large!)
///
/// For a 512x256 world, this creates a 24,576 x 12,288 pixel image (~300 megapixels).
/// Consider using `export_local_region` for smaller areas.
pub fn export_full_world<P: AsRef<Path>>(
    world: &WorldData,
    path: P,
    options: &ExportOptions,
) -> Result<(u32, u32), ExportError> {
    export_local_region(world, 0, 0, world.heightmap.width, world.heightmap.height, path, options)
}

/// Export local maps around a center point
pub fn export_local_area<P: AsRef<Path>>(
    world: &WorldData,
    center_x: usize,
    center_y: usize,
    radius: usize,
    path: P,
    options: &ExportOptions,
) -> Result<(u32, u32), ExportError> {
    let start_x = center_x.saturating_sub(radius);
    let start_y = center_y.saturating_sub(radius);
    let width = (radius * 2 + 1).min(world.heightmap.width - start_x);
    let height = (radius * 2 + 1).min(world.heightmap.height - start_y);

    export_local_region(world, start_x, start_y, width, height, path, options)
}

/// Export errors
#[derive(Debug)]
pub enum ExportError {
    /// Image dimensions would exceed maximum allowed
    ImageTooLarge {
        requested_width: u32,
        requested_height: u32,
        max_pixels: u64,
    },
    /// Failed to save image
    SaveFailed(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::ImageTooLarge { requested_width, requested_height, max_pixels } => {
                write!(
                    f,
                    "Requested image size {}x{} ({} pixels) exceeds maximum {} pixels",
                    requested_width, requested_height,
                    *requested_width as u64 * *requested_height as u64,
                    max_pixels
                )
            }
            ExportError::SaveFailed(msg) => write!(f, "Failed to save image: {}", msg),
        }
    }
}

impl std::error::Error for ExportError {}

/// Quick export helper - exports a 5x5 area around a point
pub fn quick_export<P: AsRef<Path>>(
    world: &WorldData,
    center_x: usize,
    center_y: usize,
    path: P,
) -> Result<(u32, u32), ExportError> {
    export_local_area(world, center_x, center_y, 2, path, &ExportOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_color() {
        let color = terrain_color(&LocalTerrain::Grass);
        assert_eq!(color, Rgb([34, 139, 34]));

        let water = terrain_color(&LocalTerrain::DeepWater);
        assert_eq!(water, Rgb([0, 105, 148]));
    }

    #[test]
    fn test_export_options_default() {
        let opts = ExportOptions::default();
        assert!(opts.auto_surface);
        assert!(opts.show_features);
        assert_eq!(opts.scale, 1);
    }
}
