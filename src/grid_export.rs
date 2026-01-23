//! Grid export tool for comparing different simulation settings
//!
//! Generates comparison images showing the same world with different:
//! - Erosion presets
//! - Climate modes
//! - Rainfall levels

use std::error::Error;
use image::{ImageBuffer, Rgb, RgbImage};

use crate::biomes;
use crate::climate::{self, ClimateConfig, ClimateMode, RainfallLevel};
use crate::coastline;
use crate::erosion::{self, ErosionPreset};
use crate::heightmap;
use crate::plates::{self, WorldStyle};
use crate::seeds::WorldSeeds;
use crate::tilemap::Tilemap;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Configuration for grid export
pub struct GridExportConfig {
    pub width: usize,
    pub height: usize,
    pub seed: u64,
    pub world_style: WorldStyle,
    pub plates: Option<usize>,
    /// Padding between cells in pixels
    pub cell_padding: u32,
    /// Height of label area
    pub label_height: u32,
}

impl Default for GridExportConfig {
    fn default() -> Self {
        Self {
            width: 256,
            height: 128,
            seed: 42,
            world_style: WorldStyle::Earthlike,
            plates: None,
            cell_padding: 4,
            label_height: 24,
        }
    }
}

/// Generate a single world with specific settings and return biome colors
fn generate_world_image(
    config: &GridExportConfig,
    erosion_preset: ErosionPreset,
    climate_config: &ClimateConfig,
) -> RgbImage {
    let seeds = WorldSeeds::builder(config.seed).build();
    let mut tectonic_rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);

    // Generate tectonic plates
    let (plate_map, plates) = plates::generate_plates(
        config.width,
        config.height,
        config.plates,
        config.world_style,
        &mut tectonic_rng,
    );

    // Calculate stress
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    let _land_mask = heightmap::generate_land_mask(&plate_map, &plates, seeds.heightmap);
    let mut heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);

    // Generate climate with config
    let temperature = climate::generate_temperature_with_config(
        &heightmap,
        config.width,
        config.height,
        climate_config.mode,
    );

    // Apply erosion if not None
    if erosion_preset != ErosionPreset::None {
        let erosion_params = erosion::ErosionParams::from_preset(erosion_preset);
        let mut erosion_rng = ChaCha8Rng::seed_from_u64(seeds.erosion);

        let _ = erosion::simulate_erosion(
            &mut heightmap,
            &plate_map,
            &plates,
            &stress_map,
            &temperature,
            &erosion_params,
            &mut erosion_rng,
            seeds.erosion,
        );
    }

    // Apply coastline jittering
    let coastline_params = coastline::CoastlineParams::default();
    let coastline_network = coastline::generate_coastline_network(&heightmap, &coastline_params, seeds.coastline);
    coastline::apply_coastline_to_heightmap(&coastline_network, &mut heightmap, coastline_params.blend_width);

    // Apply terrain noise
    heightmap::apply_regional_noise_stacks(&mut heightmap, &stress_map, seeds.heightmap);

    // Generate moisture with config
    let moisture = climate::generate_moisture_with_config(
        &heightmap,
        config.width,
        config.height,
        climate_config,
    );

    // Generate biomes
    let biome_config = biomes::WorldBiomeConfig::default();
    let biomes = biomes::generate_extended_biomes(
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        &biome_config,
        seeds.biomes,
    );

    // Render to image with hillshading
    render_biome_image(&heightmap, &biomes)
}

/// Render biomes to an image with hillshading
fn render_biome_image(heightmap: &Tilemap<f32>, biomes: &Tilemap<biomes::ExtendedBiome>) -> RgbImage {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let biome = *biomes.get(x, y);
            let h = *heightmap.get(x, y);

            let (base_r, base_g, base_b) = biome.color();

            // Simple hillshading
            let mut shade = 1.0f32;
            if x >= 1 && y >= 1 && x < width - 1 && y < height - 1 {
                let h_left = *heightmap.get(x - 1, y);
                let h_right = *heightmap.get(x + 1, y);
                let h_up = *heightmap.get(x, y - 1);
                let h_down = *heightmap.get(x, y + 1);

                let slope_x = (h_right - h_left) / 100.0;
                let slope_y = (h_down - h_up) / 100.0;
                shade = (1.0 + slope_x * 0.3 - slope_y * 0.2).clamp(0.7, 1.3);
            }

            let elevation_factor = if h > 0.0 {
                0.95 + (h / 5000.0).min(0.1)
            } else {
                0.95 + (h / 2000.0).max(-0.1)
            };

            let factor = (elevation_factor * shade).clamp(0.6, 1.3);

            let r = ((base_r as f32 * factor).min(255.0)) as u8;
            let g = ((base_g as f32 * factor).min(255.0)) as u8;
            let b = ((base_b as f32 * factor).min(255.0)) as u8;

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img
}

/// Simple 5x7 pixel font for labels (uppercase only + numbers)
const FONT_5X7: &[(&str, [u8; 7])] = &[
    ("A", [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ("B", [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
    ("C", [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
    ("D", [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
    ("E", [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
    ("F", [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
    ("G", [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]),
    ("H", [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ("I", [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ("J", [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
    ("K", [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
    ("L", [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
    ("M", [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
    ("N", [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001]),
    ("O", [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ("P", [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    ("Q", [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
    ("R", [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
    ("S", [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110]),
    ("T", [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
    ("U", [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ("V", [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
    ("W", [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010]),
    ("X", [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
    ("Y", [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
    ("Z", [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
    ("0", [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
    ("1", [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ("2", [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
    ("3", [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
    ("4", [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
    ("5", [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
    ("6", [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
    ("7", [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    ("8", [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
    ("9", [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
    (" ", [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
    ("-", [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
    (".", [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100]),
];

/// Draw a character at position
fn draw_char(img: &mut RgbImage, x: i32, y: i32, ch: char, color: Rgb<u8>) {
    let ch_upper = ch.to_ascii_uppercase();
    let ch_str = ch_upper.to_string();

    let glyph = FONT_5X7.iter().find(|(c, _)| *c == ch_str);

    if let Some((_, bits)) = glyph {
        for (row, &byte) in bits.iter().enumerate() {
            for col in 0..5 {
                if byte & (0b10000 >> col) != 0 {
                    let px = x + col;
                    let py = y + row as i32;
                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                        img.put_pixel(px as u32, py as u32, color);
                    }
                }
            }
        }
    }
}

/// Draw text string at position
fn draw_text(img: &mut RgbImage, x: i32, y: i32, text: &str, color: Rgb<u8>) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char(img, cx, y, ch, color);
        cx += 6; // 5 pixels + 1 spacing
    }
}

/// Draw label with background
fn draw_label(img: &mut RgbImage, x: u32, y: u32, text: &str, bg_width: u32) {
    // Draw dark background
    for dy in 0..config_label_height() {
        for dx in 0..bg_width {
            if x + dx < img.width() && y + dy < img.height() {
                img.put_pixel(x + dx, y + dy, Rgb([30, 30, 35]));
            }
        }
    }

    // Center text
    let text_width = (text.len() * 6) as u32;
    let text_x = x + (bg_width.saturating_sub(text_width)) / 2;
    let text_y = y + (config_label_height() - 7) / 2;

    draw_text(img, text_x as i32, text_y as i32, text, Rgb([220, 220, 220]));
}

fn config_label_height() -> u32 {
    16
}

/// Export a grid comparing all erosion presets
pub fn export_erosion_grid(config: &GridExportConfig, filename: &str) -> Result<(), Box<dyn Error>> {
    let presets = ErosionPreset::all();
    let climate_config = ClimateConfig::default();

    let cell_w = config.width as u32;
    let cell_h = config.height as u32 + config.label_height;
    let padding = config.cell_padding;

    let grid_w = presets.len() as u32 * cell_w + (presets.len() as u32 - 1) * padding;
    let grid_h = cell_h;

    let mut grid = ImageBuffer::new(grid_w, grid_h);

    for pixel in grid.pixels_mut() {
        *pixel = Rgb([20, 20, 20]);
    }

    println!("Generating erosion comparison grid...");
    for (i, &preset) in presets.iter().enumerate() {
        println!("  Generating {} ({}/{})...", preset, i + 1, presets.len());

        let world_img = generate_world_image(config, preset, &climate_config);

        let x_offset = i as u32 * (cell_w + padding);

        // Copy world image
        for y in 0..config.height as u32 {
            for x in 0..config.width as u32 {
                let pixel = world_img.get_pixel(x, y);
                grid.put_pixel(x_offset + x, y, *pixel);
            }
        }

        // Draw label
        let label = format!("{}", preset).to_uppercase();
        draw_label(&mut grid, x_offset, config.height as u32 + 4, &label, cell_w);
    }

    grid.save(filename)?;
    println!("Exported erosion grid to {}", filename);
    Ok(())
}

/// Export a grid comparing all climate modes
pub fn export_climate_grid(config: &GridExportConfig, filename: &str) -> Result<(), Box<dyn Error>> {
    let modes = ClimateMode::all();
    let erosion_preset = ErosionPreset::Normal;

    let cell_w = config.width as u32;
    let cell_h = config.height as u32 + config.label_height;
    let padding = config.cell_padding;

    let grid_w = modes.len() as u32 * cell_w + (modes.len() as u32 - 1) * padding;
    let grid_h = cell_h;

    let mut grid = ImageBuffer::new(grid_w, grid_h);

    for pixel in grid.pixels_mut() {
        *pixel = Rgb([20, 20, 20]);
    }

    println!("Generating climate mode comparison grid...");
    for (i, &mode) in modes.iter().enumerate() {
        println!("  Generating {} ({}/{})...", mode, i + 1, modes.len());

        let climate_config = ClimateConfig {
            mode,
            rainfall: RainfallLevel::Normal,
        };
        let world_img = generate_world_image(config, erosion_preset, &climate_config);

        let x_offset = i as u32 * (cell_w + padding);

        for y in 0..config.height as u32 {
            for x in 0..config.width as u32 {
                let pixel = world_img.get_pixel(x, y);
                grid.put_pixel(x_offset + x, y, *pixel);
            }
        }

        let label = format!("{}", mode).to_uppercase();
        draw_label(&mut grid, x_offset, config.height as u32 + 4, &label, cell_w);
    }

    grid.save(filename)?;
    println!("Exported climate grid to {}", filename);
    Ok(())
}

/// Export a grid comparing all rainfall levels
pub fn export_rainfall_grid(config: &GridExportConfig, filename: &str) -> Result<(), Box<dyn Error>> {
    let levels = RainfallLevel::all();
    let erosion_preset = ErosionPreset::Normal;

    let cell_w = config.width as u32;
    let cell_h = config.height as u32 + config.label_height;
    let padding = config.cell_padding;

    let grid_w = levels.len() as u32 * cell_w + (levels.len() as u32 - 1) * padding;
    let grid_h = cell_h;

    let mut grid = ImageBuffer::new(grid_w, grid_h);

    for pixel in grid.pixels_mut() {
        *pixel = Rgb([20, 20, 20]);
    }

    println!("Generating rainfall level comparison grid...");
    for (i, &level) in levels.iter().enumerate() {
        println!("  Generating {} ({}/{})...", level, i + 1, levels.len());

        let climate_config = ClimateConfig {
            mode: ClimateMode::Globe,
            rainfall: level,
        };
        let world_img = generate_world_image(config, erosion_preset, &climate_config);

        let x_offset = i as u32 * (cell_w + padding);

        for y in 0..config.height as u32 {
            for x in 0..config.width as u32 {
                let pixel = world_img.get_pixel(x, y);
                grid.put_pixel(x_offset + x, y, *pixel);
            }
        }

        let label = format!("{}", level).to_uppercase();
        draw_label(&mut grid, x_offset, config.height as u32 + 4, &label, cell_w);
    }

    grid.save(filename)?;
    println!("Exported rainfall grid to {}", filename);
    Ok(())
}

/// Export a full comparison grid with all combinations
/// Rows: Erosion presets, Columns: Climate modes
pub fn export_full_grid(config: &GridExportConfig, filename: &str) -> Result<(), Box<dyn Error>> {
    let erosion_presets = ErosionPreset::all();
    let climate_modes = ClimateMode::all();

    let cell_w = config.width as u32;
    let cell_h = config.height as u32;
    let padding = config.cell_padding;

    // Grid dimensions
    let cols = climate_modes.len() as u32;
    let rows = erosion_presets.len() as u32;

    // Add space for row/column labels
    let row_label_w = 70u32;
    let col_label_h = 20u32;

    let grid_w = row_label_w + cols * cell_w + (cols - 1) * padding;
    let grid_h = col_label_h + rows * cell_h + (rows - 1) * padding;

    let mut grid = ImageBuffer::new(grid_w, grid_h);

    for pixel in grid.pixels_mut() {
        *pixel = Rgb([20, 20, 20]);
    }

    // Draw column labels (climate modes)
    for (col, &mode) in climate_modes.iter().enumerate() {
        let x = row_label_w + col as u32 * (cell_w + padding);
        let label = format!("{}", mode).to_uppercase();
        draw_text(&mut grid, (x + 4) as i32, 6, &label, Rgb([180, 180, 180]));
    }

    let total = erosion_presets.len() * climate_modes.len();
    let mut count = 0;

    println!("Generating full comparison grid ({} combinations)...", total);
    for (row, &erosion) in erosion_presets.iter().enumerate() {
        // Draw row label
        let y = col_label_h + row as u32 * (cell_h + padding);
        let label = format!("{}", erosion).to_uppercase();
        draw_text(&mut grid, 4, (y + cell_h / 2 - 3) as i32, &label, Rgb([180, 180, 180]));

        for (col, &mode) in climate_modes.iter().enumerate() {
            count += 1;
            println!("  [{}/{}] erosion={}, climate={}...", count, total, erosion, mode);

            let climate_config = ClimateConfig {
                mode,
                rainfall: RainfallLevel::Normal,
            };
            let world_img = generate_world_image(config, erosion, &climate_config);

            let x_offset = row_label_w + col as u32 * (cell_w + padding);
            let y_offset = col_label_h + row as u32 * (cell_h + padding);

            for wy in 0..config.height as u32 {
                for wx in 0..config.width as u32 {
                    let pixel = world_img.get_pixel(wx, wy);
                    grid.put_pixel(x_offset + wx, y_offset + wy, *pixel);
                }
            }
        }
    }

    grid.save(filename)?;
    println!("Exported full grid to {}", filename);
    Ok(())
}

/// Export all comparison grids
pub fn export_all_grids(config: &GridExportConfig, prefix: &str) -> Result<(), Box<dyn Error>> {
    export_erosion_grid(config, &format!("{}_erosion.png", prefix))?;
    export_climate_grid(config, &format!("{}_climate.png", prefix))?;
    export_rainfall_grid(config, &format!("{}_rainfall.png", prefix))?;
    Ok(())
}
