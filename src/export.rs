use std::f64::consts::PI;

use image::{ImageBuffer, Rgb, RgbImage};

use crate::biomes::ExtendedBiome;
use crate::climate::Biome as ClimateBiome;
use crate::plates::{Plate, PlateId, PlateType};
use crate::tilemap::Tilemap;

/// Export a heightmap using spectral colormap.
/// Values are expected to be normalized (0.0-1.0).
pub fn export_heightmap(heightmap: &Tilemap<f32>, path: &str) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(heightmap.width as u32, heightmap.height as u32);

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let val = *heightmap.get(x, y);
            let color = spectral_colormap(val.clamp(0.0, 1.0));
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }

    img.save(path)
}

/// Spectral colormap (matplotlib style): dark blue -> cyan -> green -> yellow -> orange -> red
fn spectral_colormap(t: f32) -> [u8; 3] {
    let colors: [[f32; 3]; 11] = [
        [0.37, 0.31, 0.64],  // Dark blue/purple (low)
        [0.20, 0.53, 0.74],  // Blue
        [0.40, 0.76, 0.65],  // Teal
        [0.67, 0.87, 0.64],  // Light green
        [0.90, 0.96, 0.60],  // Yellow-green
        [1.00, 1.00, 0.75],  // Light yellow / white
        [1.00, 0.88, 0.55],  // Yellow
        [0.99, 0.68, 0.38],  // Light orange
        [0.96, 0.43, 0.26],  // Orange
        [0.84, 0.24, 0.31],  // Red
        [0.62, 0.00, 0.26],  // Dark red (high)
    ];

    let t_scaled = t * 10.0;
    let idx = (t_scaled as usize).min(9);
    let frac = t_scaled - idx as f32;

    let c1 = colors[idx];
    let c2 = colors[idx + 1];

    [
        ((c1[0] + (c2[0] - c1[0]) * frac) * 255.0) as u8,
        ((c1[1] + (c2[1] - c1[1]) * frac) * 255.0) as u8,
        ((c1[2] + (c2[2] - c1[2]) * frac) * 255.0) as u8,
    ]
}

/// Export a plate map as a colored PNG.
/// Each plate gets its own color based on its type.
pub fn export_plate_map(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(plate_map.width as u32, plate_map.height as u32);

    for y in 0..plate_map.height {
        for x in 0..plate_map.width {
            let plate_id = *plate_map.get(x, y);
            let color = if plate_id.is_none() {
                [0, 0, 0]
            } else {
                plates[plate_id.0 as usize].color
            };
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }

    img.save(path)
}

/// Export a plate type map showing oceanic vs continental plates clearly.
/// Blue = oceanic, Green/Brown = continental, with boundary lines.
pub fn export_plate_types(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(plate_map.width as u32, plate_map.height as u32);

    // Define clear colors for each type
    let ocean_color = [30u8, 90, 160]; // Deep blue
    let land_color = [120u8, 160, 80]; // Green/brown

    for y in 0..plate_map.height {
        for x in 0..plate_map.width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() {
                img.put_pixel(x as u32, y as u32, Rgb([0, 0, 0]));
                continue;
            }

            let plate = &plates[plate_id.0 as usize];

            // Check if this is a boundary cell
            let mut is_boundary = false;
            for (nx, ny) in plate_map.neighbors(x, y) {
                let neighbor_id = *plate_map.get(nx, ny);
                if neighbor_id != plate_id {
                    is_boundary = true;
                    break;
                }
            }

            let base_color = match plate.plate_type {
                PlateType::Oceanic => ocean_color,
                PlateType::Continental => land_color,
            };

            // Darken boundaries for visibility
            let color = if is_boundary {
                [
                    (base_color[0] as f32 * 0.5) as u8,
                    (base_color[1] as f32 * 0.5) as u8,
                    (base_color[2] as f32 * 0.5) as u8,
                ]
            } else {
                base_color
            };

            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }

    img.save(path)
}

/// Export a stress map as a colored PNG.
/// Red = convergent (mountains), Blue = divergent (rifts), Gray = neutral.
pub fn export_stress_map(stress_map: &Tilemap<f32>, path: &str) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(stress_map.width as u32, stress_map.height as u32);

    // Find max absolute stress for normalization
    let mut max_abs = 0.0f32;
    for (_, _, &val) in stress_map.iter() {
        if val.abs() > max_abs {
            max_abs = val.abs();
        }
    }
    if max_abs < 0.001 {
        max_abs = 1.0;
    }

    for y in 0..stress_map.height {
        for x in 0..stress_map.width {
            let stress = *stress_map.get(x, y);
            let normalized = stress / max_abs;

            let color = if normalized > 0.0 {
                // Convergent = red/orange (mountains)
                let intensity = (normalized * 255.0) as u8;
                [200u8.saturating_add(intensity / 4), 100 - (intensity / 3), 50]
            } else if normalized < 0.0 {
                // Divergent = blue (rifts/trenches)
                let intensity = (-normalized * 255.0) as u8;
                [50, 100 - (intensity / 3), 200u8.saturating_add(intensity / 4)]
            } else {
                // Neutral = gray
                [128, 128, 128]
            };

            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }

    img.save(path)
}

/// Export a globe projection of the heightmap.
/// Renders the equirectangular map onto a 3D sphere with basic shading.
pub fn export_globe(
    heightmap: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    path: &str,
    rotation: f64, // Longitude rotation in radians
) -> Result<(), image::ImageError> {
    let size = heightmap.height.max(heightmap.width / 2);
    let mut img: RgbImage = ImageBuffer::new(size as u32, size as u32);

    let radius = size as f64 / 2.0 - 10.0;
    let center_x = size as f64 / 2.0;
    let center_y = size as f64 / 2.0;

    // Light direction (from upper-right)
    let light_dir = normalize_vec(1.0, 1.0, 0.8);

    for py in 0..size {
        for px in 0..size {
            let x = (px as f64 - center_x) / radius;
            let y = (center_y - py as f64) / radius; // Flip Y for correct orientation

            // Check if point is on the sphere
            let r_squared = x * x + y * y;
            if r_squared > 1.0 {
                // Background - dark space
                img.put_pixel(px as u32, py as u32, Rgb([5, 5, 15]));
                continue;
            }

            // Calculate Z coordinate on sphere surface
            let z = (1.0 - r_squared).sqrt();

            // Convert to latitude/longitude
            let lat = y.asin(); // -PI/2 to PI/2
            let lon = x.atan2(z) + rotation; // -PI to PI, plus rotation

            // Normalize longitude to 0..2*PI
            let lon = ((lon % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

            // Convert to map coordinates
            let map_x = (lon / (2.0 * PI) * heightmap.width as f64) as usize % heightmap.width;
            let map_y =
                ((0.5 - lat / PI) * heightmap.height as f64).clamp(0.0, heightmap.height as f64 - 1.0) as usize;

            // Get height and plate color
            let height = *heightmap.get(map_x, map_y);
            let plate_id = *plate_map.get(map_x, map_y);
            let base_color = if plate_id.is_none() {
                [50, 50, 80]
            } else {
                plates[plate_id.0 as usize].color
            };

            // Calculate lighting (Lambert shading)
            let normal = (x, y, z);
            let diffuse = (normal.0 * light_dir.0 + normal.1 * light_dir.1 + normal.2 * light_dir.2)
                .max(0.0);

            // Ambient + diffuse lighting
            let ambient = 0.3;
            let light_intensity = ambient + (1.0 - ambient) * diffuse;

            // Height-based shading (higher = brighter)
            let height_boost = 1.0 + (height as f64 - 0.5) * 0.3;

            // Apply lighting to color
            let final_intensity = (light_intensity * height_boost).clamp(0.3, 1.3);
            let r = ((base_color[0] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;

            img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
        }
    }

    // Add atmosphere glow
    add_atmosphere_glow(&mut img, size, radius, center_x, center_y);

    img.save(path)
}

/// Add a subtle atmospheric glow around the planet edge
fn add_atmosphere_glow(
    img: &mut RgbImage,
    size: usize,
    radius: f64,
    center_x: f64,
    center_y: f64,
) {
    let glow_radius = radius * 1.15;
    let glow_color = [100u8, 150, 255]; // Blueish atmosphere

    for py in 0..size {
        for px in 0..size {
            let x = px as f64 - center_x;
            let y = py as f64 - center_y;
            let dist = (x * x + y * y).sqrt();

            // Only affect the area between planet edge and glow edge
            if dist > radius && dist < glow_radius {
                let t = (dist - radius) / (glow_radius - radius);
                let glow_strength = (1.0 - t).powi(2) * 0.4; // Fade out quadratically

                let pixel = img.get_pixel(px as u32, py as u32);
                let r = (pixel[0] as f64 + glow_color[0] as f64 * glow_strength).min(255.0) as u8;
                let g = (pixel[1] as f64 + glow_color[1] as f64 * glow_strength).min(255.0) as u8;
                let b = (pixel[2] as f64 + glow_color[2] as f64 * glow_strength).min(255.0) as u8;
                img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
            }
        }
    }
}

fn normalize_vec(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let len = (x * x + y * y + z * z).sqrt();
    (x / len, y / len, z / len)
}

/// Export a terrain map based on heightmap.
/// Sea level is 0: negative = ocean (blue), positive = land (green/brown/white).
pub fn export_terrain_map(
    heightmap: &Tilemap<f32>,
    _plate_map: &Tilemap<PlateId>,
    _plates: &[Plate],
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(heightmap.width as u32, heightmap.height as u32);

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let height = *heightmap.get(x, y);

            // Sea level is 0: below = ocean, above = land
            let color = if height < 0.0 {
                ocean_color(height)
            } else {
                land_color(height)
            };

            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }

    img.save(path)
}

/// Biome types for terrain classification
#[derive(Clone, Copy, Debug, PartialEq)]
enum Biome {
    DeepOcean,
    Ocean,
    ShallowWater,
    Beach,
    Lowland,
    Plains,
    Forest,
    Hills,
    Highland,
    Mountain,
    SnowyPeak,
}

/// Classify terrain into discrete biomes based on elevation
fn classify_biome(height: f32) -> Biome {
    if height < -500.0 {
        Biome::DeepOcean
    } else if height < -100.0 {
        Biome::Ocean
    } else if height < 0.0 {
        Biome::ShallowWater
    } else if height < 10.0 {
        Biome::Beach
    } else if height < 40.0 {
        Biome::Lowland
    } else if height < 80.0 {
        Biome::Plains
    } else if height < 130.0 {
        Biome::Forest
    } else if height < 200.0 {
        Biome::Hills
    } else if height < 300.0 {
        Biome::Highland
    } else if height < 450.0 {
        Biome::Mountain
    } else {
        Biome::SnowyPeak
    }
}

/// Get color for a biome
fn biome_color(biome: Biome) -> [u8; 3] {
    match biome {
        Biome::DeepOcean => [20, 40, 80],
        Biome::Ocean => [30, 60, 120],
        Biome::ShallowWater => [60, 100, 150],
        Biome::Beach => [210, 190, 140],
        Biome::Lowland => [80, 160, 60],
        Biome::Plains => [100, 180, 80],
        Biome::Forest => [40, 120, 50],
        Biome::Hills => [110, 140, 70],
        Biome::Highland => [140, 130, 100],
        Biome::Mountain => [120, 110, 100],
        Biome::SnowyPeak => [240, 240, 245],
    }
}

/// Color for terrain based on biome classification (discrete)
fn terrain_color(height: f32) -> [u8; 3] {
    biome_color(classify_biome(height))
}

/// Color gradient for ocean based on depth (for shaded view)
fn ocean_color(height: f32) -> [u8; 3] {
    let biome = classify_biome(height);
    biome_color(biome)
}

/// Color gradient for land based on elevation (for shaded view)
fn land_color(height: f32) -> [u8; 3] {
    let biome = classify_biome(height);
    biome_color(biome)
}

fn lerp_color(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// Export terrain map with hillshade (fake 3D shadows)
pub fn export_terrain_shaded(
    heightmap: &Tilemap<f32>,
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(heightmap.width as u32, heightmap.height as u32);

    // Light direction (from northwest, elevated)
    let light_dir: [f32; 3] = normalize_vec3(-1.0, -1.0, 2.0);

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let height = *heightmap.get(x, y);

            // Calculate normal from heightmap gradient
            let normal = calculate_normal(heightmap, x, y);

            // Lambert shading
            let diffuse = (normal[0] * light_dir[0] + normal[1] * light_dir[1] + normal[2] * light_dir[2])
                .max(0.0);

            // Ambient + diffuse
            let ambient = 0.4;
            let shade = ambient + (1.0 - ambient) * diffuse;

            // Get base color
            let base_color = if height < 0.0 {
                ocean_color(height)
            } else {
                land_color(height)
            };

            // Apply shading
            let r = ((base_color[0] as f32 * shade).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f32 * shade).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f32 * shade).clamp(0.0, 255.0)) as u8;

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(path)
}

/// Calculate surface normal from heightmap gradient
fn calculate_normal(heightmap: &Tilemap<f32>, x: usize, y: usize) -> [f32; 3] {
    let width = heightmap.width;
    let height = heightmap.height;

    // Sample neighboring heights with wrapping for x (cylindrical map)
    let x_left = if x == 0 { width - 1 } else { x - 1 };
    let x_right = if x == width - 1 { 0 } else { x + 1 };
    let y_up = if y == 0 { 0 } else { y - 1 };
    let y_down = if y == height - 1 { height - 1 } else { y + 1 };

    let h_left = *heightmap.get(x_left, y);
    let h_right = *heightmap.get(x_right, y);
    let h_up = *heightmap.get(x, y_up);
    let h_down = *heightmap.get(x, y_down);

    // Calculate gradient with height exaggeration
    // Scale relative to typical elevation range (~5000m) vs pixel distance
    let height_scale = 0.002; // Adjust for visual effect
    let dx = (h_right - h_left) * height_scale;
    let dy = (h_down - h_up) * height_scale;

    // Normal from gradient: (-dx, -dy, 1) normalized
    // Note: dy sign is inverted because screen Y increases downward
    normalize_vec3(-dx, dy, 1.0)
}

fn normalize_vec3(x: f32, y: f32, z: f32) -> [f32; 3] {
    let len = (x * x + y * y + z * z).sqrt();
    [x / len, y / len, z / len]
}

/// Export all visualizations in a single grid image.
/// Layout: 3 rows x 3 columns
/// Row 1: Plates, Types, Stress
/// Row 2: Land, Terrain, Shaded
/// Row 3: Heightmap, Globe, Info
/// Export all visualizations in a single grid image.
/// Layout: 3 rows x 3 columns
/// Row 1: Plates, Types, Stress
/// Row 2: Hardness, Biomes, Shaded
/// Row 3: Heightmap, Globe, Info
pub fn export_combined_grid(
    heightmap: &Tilemap<f32>,
    heightmap_normalized: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    biomes: &Tilemap<ClimateBiome>,
    hardness_map: &Tilemap<f32>,
    path: &str,
    seed: u64,
) -> Result<(), image::ImageError> {
    // Note: land_mask generation removed as it's no longer used in the grid
    // use crate::heightmap::generate_land_mask;

    let tile_w = heightmap.width as u32;
    let tile_h = heightmap.height as u32;

    // Grid: 3 columns x 3 rows
    let cols = 3u32;
    let rows = 3u32;
    let label_height = 20u32;

    let grid_w = tile_w * cols;
    let grid_h = (tile_h + label_height) * rows;

    let mut grid: RgbImage = ImageBuffer::from_pixel(grid_w, grid_h, Rgb([30, 30, 30]));

    // Generate each tile and copy to grid
    let info_label = format!("Info");
    let tiles: Vec<(&str, RgbImage)> = vec![
        ("Plates", render_plate_map(plate_map, plates)),
        ("Types", render_plate_types(plate_map, plates)),
        ("Stress", render_stress_map(stress_map)),
        ("Hardness", render_hardness_map(hardness_map)), // Replaces Land mask
        ("Biomes", render_biome_map(biomes)),
        ("Shaded", render_terrain_shaded(heightmap)),
        ("Heightmap", render_heightmap(heightmap_normalized)),
        ("Globe", render_globe(heightmap_normalized, plate_map, plates, 0.0)),
        (&info_label, render_info_panel(tile_w, tile_h, seed, plates.len())),
    ];

    for (idx, (label, tile)) in tiles.into_iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;
        let offset_x = col * tile_w;
        let offset_y = row * (tile_h + label_height) + label_height;

        // Copy tile to grid
        for ty in 0..tile.height().min(tile_h) {
            for tx in 0..tile.width().min(tile_w) {
                let pixel = tile.get_pixel(tx, ty);
                let gx = offset_x + tx;
                let gy = offset_y + ty;
                if gx < grid_w && gy < grid_h {
                    grid.put_pixel(gx, gy, *pixel);
                }
            }
        }

        // Draw label background
        let label_y = row * (tile_h + label_height);
        for ly in 0..label_height {
            for lx in 0..tile_w {
                grid.put_pixel(offset_x + lx, label_y + ly, Rgb([50, 50, 50]));
            }
        }

        // Draw simple text label (basic pixel font)
        draw_label(&mut grid, label, offset_x + 4, label_y + 4);
    }

    grid.save(path)
}

/// Generate combined grid image without saving (for interactive viewer).
pub fn generate_combined_grid(
    heightmap: &Tilemap<f32>,
    heightmap_normalized: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    hardness_map: &Tilemap<f32>, // Added argument
    seed: u64,
) -> RgbImage {
    // use crate::heightmap::generate_land_mask;

    let tile_w = heightmap.width as u32;
    let tile_h = heightmap.height as u32;

    // Grid: 3 columns x 3 rows
    let cols = 3u32;
    let rows = 3u32;
    let label_height = 20u32;

    let grid_w = tile_w * cols;
    let grid_h = (tile_h + label_height) * rows;

    let mut grid: RgbImage = ImageBuffer::from_pixel(grid_w, grid_h, Rgb([30, 30, 30]));

    // Generate each tile and copy to grid
    let info_label = format!("Info");
    let tiles: Vec<(&str, RgbImage)> = vec![
        ("Plates", render_plate_map(plate_map, plates)),
        ("Types", render_plate_types(plate_map, plates)),
        ("Stress", render_stress_map(stress_map)),
        ("Hardness", render_hardness_map(hardness_map)), // Replaces Land mask
        ("Terrain", render_terrain_map(heightmap)),
        ("Shaded", render_terrain_shaded(heightmap)),
        ("Heightmap", render_heightmap(heightmap_normalized)),
        ("Globe", render_globe(heightmap_normalized, plate_map, plates, 0.0)),
        (&info_label, render_info_panel(tile_w, tile_h, seed, plates.len())),
    ];

    for (idx, (label, tile)) in tiles.into_iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;
        let offset_x = col * tile_w;
        let offset_y = row * (tile_h + label_height) + label_height;

        // Copy tile to grid
        for ty in 0..tile.height().min(tile_h) {
            for tx in 0..tile.width().min(tile_w) {
                let pixel = tile.get_pixel(tx, ty);
                let gx = offset_x + tx;
                let gy = offset_y + ty;
                if gx < grid_w && gy < grid_h {
                    grid.put_pixel(gx, gy, *pixel);
                }
            }
        }

        // Draw label background
        let label_y = row * (tile_h + label_height);
        for ly in 0..label_height {
            for lx in 0..tile_w {
                grid.put_pixel(offset_x + lx, label_y + ly, Rgb([50, 50, 50]));
            }
        }

        // Draw simple text label
        draw_label(&mut grid, label, offset_x + 4, label_y + 4);
    }

    grid
}

/// Render hardness map to grayscale image
pub fn render_hardness_map(hardness: &Tilemap<f32>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(hardness.width as u32, hardness.height as u32);
    for y in 0..hardness.height {
        for x in 0..hardness.width {
            let val = *hardness.get(x, y);
            // Hardness is 0.0 to 1.0. Map to grayscale 0-255.
            let gray = (val.clamp(0.0, 1.0) * 255.0) as u8;
            img.put_pixel(x as u32, y as u32, Rgb([gray, gray, gray]));
        }
    }
    img
}

/// Render plate map to image buffer
pub fn render_plate_map(plate_map: &Tilemap<PlateId>, plates: &[Plate]) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(plate_map.width as u32, plate_map.height as u32);
    for y in 0..plate_map.height {
        for x in 0..plate_map.width {
            let plate_id = *plate_map.get(x, y);
            let color = if plate_id.is_none() {
                [0, 0, 0]
            } else {
                plates[plate_id.0 as usize].color
            };
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
    img
}

/// Render land mask - binary land/ocean visualization
fn render_land_mask(_plate_map: &Tilemap<PlateId>, _plates: &[Plate], land_mask: &Tilemap<bool>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(land_mask.width as u32, land_mask.height as u32);

    let land_color = [180u8, 160, 120];       // Tan/beige for land
    let ocean_color = [40u8, 60, 100];        // Dark blue for ocean

    for y in 0..land_mask.height {
        for x in 0..land_mask.width {
            let is_land = *land_mask.get(x, y);
            let color = if is_land { land_color } else { ocean_color };
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
    img
}

/// Render plate types to image buffer
pub fn render_plate_types(plate_map: &Tilemap<PlateId>, plates: &[Plate]) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(plate_map.width as u32, plate_map.height as u32);
    let ocean_color = [30u8, 90, 160];
    let land_color = [120u8, 160, 80];

    for y in 0..plate_map.height {
        for x in 0..plate_map.width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() {
                img.put_pixel(x as u32, y as u32, Rgb([0, 0, 0]));
                continue;
            }
            let plate = &plates[plate_id.0 as usize];
            let mut is_boundary = false;
            for (nx, ny) in plate_map.neighbors(x, y) {
                if *plate_map.get(nx, ny) != plate_id {
                    is_boundary = true;
                    break;
                }
            }
            let base_color = match plate.plate_type {
                PlateType::Oceanic => ocean_color,
                PlateType::Continental => land_color,
            };
            let color = if is_boundary {
                [(base_color[0] as f32 * 0.5) as u8, (base_color[1] as f32 * 0.5) as u8, (base_color[2] as f32 * 0.5) as u8]
            } else {
                base_color
            };
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
    img
}

/// Render stress map to image buffer
pub fn render_stress_map(stress_map: &Tilemap<f32>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(stress_map.width as u32, stress_map.height as u32);
    let mut max_abs = 0.0f32;
    for (_, _, &val) in stress_map.iter() {
        if val.abs() > max_abs { max_abs = val.abs(); }
    }
    if max_abs < 0.001 { max_abs = 1.0; }

    for y in 0..stress_map.height {
        for x in 0..stress_map.width {
            let stress = *stress_map.get(x, y);
            let normalized = stress / max_abs;
            let color = if normalized > 0.0 {
                let intensity = (normalized * 255.0) as u8;
                [200u8.saturating_add(intensity / 4), 100 - (intensity / 3), 50]
            } else if normalized < 0.0 {
                let intensity = (-normalized * 255.0) as u8;
                [50, 100 - (intensity / 3), 200u8.saturating_add(intensity / 4)]
            } else {
                [128, 128, 128]
            };
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
    img
}

/// Render heightmap to image buffer using spectral colormap.
/// Automatically normalizes values to 0-1 range.
pub fn render_heightmap(heightmap: &Tilemap<f32>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(heightmap.width as u32, heightmap.height as u32);

    // Find min/max for normalization
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    for (_, _, &val) in heightmap.iter() {
        if val < min_val { min_val = val; }
        if val > max_val { max_val = val; }
    }
    let range = max_val - min_val;
    if range < 0.001 {
        // Flat heightmap, return gray
        for y in 0..heightmap.height {
            for x in 0..heightmap.width {
                img.put_pixel(x as u32, y as u32, Rgb([128, 128, 128]));
            }
        }
        return img;
    }

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let val = *heightmap.get(x, y);
            let normalized = (val - min_val) / range;
            let color = spectral_colormap(normalized);
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
    img
}

/// Render biome map using climate-based biomes
pub fn render_biome_map(biomes: &Tilemap<ClimateBiome>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(biomes.width as u32, biomes.height as u32);
    for y in 0..biomes.height {
        for x in 0..biomes.width {
            let biome = *biomes.get(x, y);
            let (r, g, b) = biome.color();
            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }
    img
}

/// Render terrain map to image buffer
pub fn render_terrain_map(heightmap: &Tilemap<f32>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(heightmap.width as u32, heightmap.height as u32);
    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let height = *heightmap.get(x, y);
            let color = terrain_color(height);
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
    img
}

/// Render terrain with extended biomes
pub fn render_terrain_extended(biomes: &Tilemap<ExtendedBiome>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(biomes.width as u32, biomes.height as u32);
    for y in 0..biomes.height {
        for x in 0..biomes.width {
            let biome = *biomes.get(x, y);
            let color = biome.color();
            img.put_pixel(x as u32, y as u32, Rgb([color.0, color.1, color.2]));
        }
    }
    img
}

/// Render shaded terrain with extended biomes
pub fn render_terrain_extended_shaded(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
) -> RgbImage {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut img: RgbImage = ImageBuffer::new(width as u32, height as u32);

    // Light direction (northwest, slightly above)
    let lx = -1.0f32;
    let ly = -1.0f32;
    let lz = 2.0f32;
    let len = (lx * lx + ly * ly + lz * lz).sqrt();
    let (lx, ly, lz) = (lx / len, ly / len, lz / len);

    for y in 0..height {
        for x in 0..width {
            let biome = *biomes.get(x, y);
            let base_color = biome.color();

            // Calculate hillshade
            let get_h = |dx: i32, dy: i32| -> f32 {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                *heightmap.get(nx, ny)
            };

            // Sobel operator for gradient
            let gx = (get_h(1, -1) + 2.0 * get_h(1, 0) + get_h(1, 1))
                   - (get_h(-1, -1) + 2.0 * get_h(-1, 0) + get_h(-1, 1));
            let gy = (get_h(-1, 1) + 2.0 * get_h(0, 1) + get_h(1, 1))
                   - (get_h(-1, -1) + 2.0 * get_h(0, -1) + get_h(1, -1));

            // Normal vector
            let scale = 0.0003;
            let nx = -gx * scale;
            let ny = -gy * scale;
            let nz = 1.0f32;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
            let (nx, ny, nz) = (nx / nlen, ny / nlen, nz / nlen);

            // Diffuse lighting
            let diffuse = (nx * lx + ny * ly + nz * lz).max(0.0);
            let ambient = 0.35;
            let lighting = (ambient + (1.0 - ambient) * diffuse).min(1.0);

            // Apply lighting to biome color
            let r = (base_color.0 as f32 * lighting).clamp(0.0, 255.0) as u8;
            let g = (base_color.1 as f32 * lighting).clamp(0.0, 255.0) as u8;
            let b = (base_color.2 as f32 * lighting).clamp(0.0, 255.0) as u8;

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img
}

/// Hash function for value noise grid points
fn hash_2d(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393);
    h = h.wrapping_add((y as u32).wrapping_mul(668265263));
    h = h.wrapping_add(seed);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h = h ^ (h >> 16);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Smooth interpolation (smoothstep)
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise with bilinear interpolation - creates smooth, coherent noise
/// scale: larger = bigger features, smaller = finer detail
fn value_noise(x: f32, y: f32, scale: f32, seed: u32) -> f32 {
    let sx = x / scale;
    let sy = y / scale;

    let x0 = sx.floor() as i32;
    let y0 = sy.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let fx = smoothstep(sx.fract());
    let fy = smoothstep(sy.fract());

    let n00 = hash_2d(x0, y0, seed);
    let n10 = hash_2d(x1, y0, seed);
    let n01 = hash_2d(x0, y1, seed);
    let n11 = hash_2d(x1, y1, seed);

    // Bilinear interpolation
    let n0 = n00 * (1.0 - fx) + n10 * fx;
    let n1 = n01 * (1.0 - fx) + n11 * fx;
    n0 * (1.0 - fy) + n1 * fy
}

/// Multi-octave value noise (fractal noise) for natural-looking variation
fn fractal_noise(x: f32, y: f32, base_scale: f32, octaves: u32, persistence: f32, seed: u32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut scale = base_scale;
    let mut max_value = 0.0;

    for i in 0..octaves {
        total += value_noise(x, y, scale, seed.wrapping_add(i * 1000)) * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        scale *= 0.5;
    }

    total / max_value
}

/// Render shaded terrain to image buffer with proper hillshade and river water
/// Enhanced version with temperature, moisture and stress for biome-based coloring
pub fn render_terrain_shaded_enhanced(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    stress: &Tilemap<f32>,
) -> RgbImage {
    // Call with default biome config
    render_terrain_shaded_extended(heightmap, temperature, stress, &crate::biomes::WorldBiomeConfig::default(), 12345)
}

/// Render terrain with extended fantasy biomes
pub fn render_terrain_shaded_extended(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    stress: &Tilemap<f32>,
    biome_config: &crate::biomes::WorldBiomeConfig,
    seed: u64,
) -> RgbImage {
    use crate::climate::generate_moisture;
    use crate::biomes::{ExtendedBiome, classify_extended};
    use crate::erosion::rivers::{compute_flow_direction, compute_flow_accumulation};
    use noise::{Perlin, Seedable};

    let width = heightmap.width;
    let height = heightmap.height;
    let mut img: RgbImage = ImageBuffer::new(width as u32, height as u32);

    // Generate moisture map for proper biome classification
    let moisture = generate_moisture(heightmap, width, height);

    // Create noise generator for fantasy biome variation
    let biome_noise = Perlin::new(1).set_seed(seed as u32);

    // Compute flow accumulation to find rivers
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

    // Find max flow for normalization
    let mut max_flow: f32 = 0.0;
    for (_, _, &acc) in flow_acc.iter() {
        if acc > max_flow { max_flow = acc; }
    }

    // Find stress range for hardness-based coloring
    let mut max_stress: f32 = 0.0;
    for (_, _, &s) in stress.iter() {
        if s.abs() > max_stress { max_stress = s.abs(); }
    }
    let max_stress = max_stress.max(1.0);

    // Scale thresholds based on map resolution
    let cell_count = (width * height) as f32;
    let base_cell_count = 131072.0f32;
    let scale_factor = (cell_count / base_cell_count).sqrt();

    // River thresholds - higher values = fewer, cleaner rivers
    let river_threshold = 250.0 * scale_factor;
    let min_river_length = (15.0 * scale_factor).max(15.0) as usize;

    // Beach detection distance scales with resolution - WIDER beaches
    let beach_dist = (10.0 * scale_factor.sqrt()).max(5.0) as i32;

    // D8 direction offsets
    const DX: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
    const DY: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    const NO_FLOW: u8 = 255;

    // Find river headwaters (cells above threshold that don't receive flow from upstream above-threshold cells)
    // This is much faster than tracing from every cell
    let mut is_headwater: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut above_threshold: Tilemap<bool> = Tilemap::new_with(width, height, false);

    // First pass: mark all cells above threshold
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let flow = *flow_acc.get(x, y);
            if h >= 0.0 && flow > river_threshold {
                above_threshold.set(x, y, true);
            }
        }
    }

    // Second pass: find headwaters (above threshold but no upstream above-threshold neighbor flows into them)
    for y in 0..height {
        for x in 0..width {
            if !*above_threshold.get(x, y) {
                continue;
            }

            // Check if any neighbor flows into this cell and is also above threshold
            let mut has_upstream_river = false;
            for dir in 0..8u8 {
                let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

                // Skip if neighbor is not above threshold
                if !*above_threshold.get(nx, ny) {
                    continue;
                }

                // Check if neighbor flows into this cell (opposite direction)
                let neighbor_dir = *flow_dir.get(nx, ny);
                if neighbor_dir < 8 {
                    let opposite = (dir + 4) % 8;
                    if neighbor_dir == opposite {
                        has_upstream_river = true;
                        break;
                    }
                }
            }

            if !has_upstream_river {
                is_headwater.set(x, y, true);
            }
        }
    }

    // Trace from headwaters and filter short rivers
    let mut valid_river: Tilemap<bool> = Tilemap::new_with(width, height, false);

    for y in 0..height {
        for x in 0..width {
            if !*is_headwater.get(x, y) || *valid_river.get(x, y) {
                continue;
            }

            // Trace downstream from this headwater
            let mut path: Vec<(usize, usize)> = Vec::new();
            let mut cx = x;
            let mut cy = y;
            let mut reached_ocean = false;
            let mut joined_valid_river = false;

            for _ in 0..10000 { // Safety limit
                let h = *heightmap.get(cx, cy);

                // Check if we reached ocean
                if h < 0.0 {
                    reached_ocean = true;
                    break;
                }

                // Check if we joined an already-valid river
                if *valid_river.get(cx, cy) {
                    joined_valid_river = true;
                    break;
                }

                // Add to path if above threshold
                if *above_threshold.get(cx, cy) {
                    path.push((cx, cy));
                }

                // Get flow direction
                let dir = *flow_dir.get(cx, cy);
                if dir == NO_FLOW || dir >= 8 {
                    break;
                }

                // Move to next cell
                let nx = (cx as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = (cy as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

                if nx == cx && ny == cy {
                    break;
                }

                cx = nx;
                cy = ny;
            }

            // Mark path as valid if it reaches ocean, joins a valid river, or is long enough
            let is_valid = reached_ocean || joined_valid_river || path.len() >= min_river_length;

            if is_valid {
                for (px, py) in path {
                    valid_river.set(px, py, true);
                }
            }
        }
    }

    // Pre-compute river width map based on flow (only for valid rivers)
    let mut river_width: Tilemap<u8> = Tilemap::new_with(width, height, 0u8);
    for y in 0..height {
        for x in 0..width {
            if !*valid_river.get(x, y) {
                continue;
            }

            let flow = *flow_acc.get(x, y);
            // Width based on flow: small streams = 1px, big rivers = 3-4px
            let w = if flow > 500.0 { 4 }
                else if flow > 200.0 { 3 }
                else if flow > 100.0 { 2 }
                else { 1 };
            river_width.set(x, y, w);
        }
    }

    // Expand rivers to their calculated width
    let mut is_river: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut river_flow: Tilemap<f32> = Tilemap::new_with(width, height, 0.0);
    for y in 0..height {
        for x in 0..width {
            let w = *river_width.get(x, y) as i32;
            if w > 0 {
                let flow = *flow_acc.get(x, y);
                // Mark surrounding pixels as river based on width
                for dy in -w..=w {
                    for dx in -w..=w {
                        if dx*dx + dy*dy <= w*w { // Circular shape
                            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                            if *heightmap.get(nx, ny) >= 0.0 { // Only on land
                                is_river.set(nx, ny, true);
                                let existing = *river_flow.get(nx, ny);
                                if flow > existing {
                                    river_flow.set(nx, ny, flow);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Find height range for land only
    let mut min_land = f32::MAX;
    let mut max_land = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h >= 0.0 {
            if h < min_land { min_land = h; }
            if h > max_land { max_land = h; }
        }
    }
    let land_range = (max_land - min_land).max(1.0);

    // Light direction (from upper-left)
    let light_x = -0.7f32;
    let light_y = -0.7f32;
    let light_z = 0.5f32;
    let light_len = (light_x * light_x + light_y * light_y + light_z * light_z).sqrt();
    let (lx, ly, lz) = (light_x / light_len, light_y / light_len, light_z / light_len);

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);

            // Ocean - temperature and depth-based coloring for regional variation
            if h < 0.0 {
                let depth = -h;
                let temp = *temperature.get(x, y);

                // Temperature affects water color:
                // Cold (<0°C): gray-blue arctic
                // Temperate (0-20°C): standard blue
                // Tropical (>20°C): turquoise/cyan
                let temp_factor = ((temp + 10.0) / 40.0).clamp(0.0, 1.0); // 0=arctic, 1=tropical

                let (r, g, b) = if depth < 30.0 {
                    // Very shallow/coastal - highly temperature dependent
                    let t = depth / 30.0;
                    if temp > 20.0 {
                        // Tropical shallow - bright turquoise/cyan
                        let tropical = ((temp - 20.0) / 15.0).min(1.0);
                        (
                            (120.0 - t * 30.0 - tropical * 20.0) as u8,
                            (200.0 - t * 20.0 + tropical * 20.0) as u8,
                            (210.0 - t * 10.0) as u8,
                        )
                    } else if temp < 5.0 {
                        // Arctic shallow - gray-blue with ice hints
                        let arctic = ((5.0 - temp) / 15.0).min(1.0);
                        (
                            (140.0 - t * 40.0 + arctic * 40.0) as u8,
                            (170.0 - t * 30.0 + arctic * 30.0) as u8,
                            (190.0 - t * 10.0 + arctic * 20.0) as u8,
                        )
                    } else {
                        // Temperate shallow - standard blue-green
                        (
                            (100.0 - t * 40.0) as u8,
                            (175.0 - t * 25.0) as u8,
                            (195.0 - t * 10.0) as u8,
                        )
                    }
                } else if depth < 100.0 {
                    // Shallow continental - blends by temperature
                    let t = (depth - 30.0) / 70.0;
                    let base_r = 70.0 - temp_factor * 20.0;
                    let base_g = 140.0 + temp_factor * 30.0;
                    let base_b = 180.0 + temp_factor * 15.0;
                    (
                        (base_r - t * 20.0) as u8,
                        (base_g - t * 30.0) as u8,
                        (base_b - t * 10.0) as u8,
                    )
                } else if depth < 500.0 {
                    // Continental shelf - medium blue, less temperature influence
                    let t = (depth - 100.0) / 400.0;
                    let temp_shift = temp_factor * 15.0;
                    (
                        (50.0 - t * 20.0 - temp_shift * 0.5) as u8,
                        (110.0 - t * 30.0 + temp_shift) as u8,
                        (170.0 - t * 20.0 + temp_shift * 0.5) as u8,
                    )
                } else if depth < 2000.0 {
                    // Deep ocean - darker, minimal temperature effect
                    let t = (depth - 500.0) / 1500.0;
                    (
                        (30.0 - t * 15.0) as u8,
                        (80.0 - t * 30.0) as u8,
                        (150.0 - t * 30.0) as u8,
                    )
                } else {
                    // Abyssal - very dark blue
                    let t = ((depth - 2000.0) / 2000.0).min(1.0);
                    (
                        (15.0 - t * 10.0) as u8,
                        (50.0 - t * 25.0) as u8,
                        (120.0 - t * 40.0) as u8,
                    )
                };
                img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
                continue;
            }

            // Check if this is a river cell (using pre-computed expanded river map)
            if *is_river.get(x, y) {
                let flow = *river_flow.get(x, y);
                // River water - blue color, intensity based on flow
                let flow_intensity = ((flow - river_threshold) / (max_flow - river_threshold + 1.0)).min(1.0);

                // Blue water color - darker/deeper blue for stronger flow (main rivers)
                let r = (40.0 - flow_intensity * 20.0) as u8;
                let g = (100.0 + flow_intensity * 50.0) as u8;
                let b = (180.0 + flow_intensity * 60.0) as u8;
                img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
                continue;
            }

            // Get local values for coloring
            let local_stress = stress.get(x, y).abs() / max_stress;
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);

            // Beach detection - compute distance to ocean
            let mut ocean_dist = f32::MAX;
            for dy in -beach_dist..=beach_dist {
                for dx in -beach_dist..=beach_dist {
                    let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    if *heightmap.get(nx, ny) < 0.0 {
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        if dist < ocean_dist {
                            ocean_dist = dist;
                        }
                    }
                }
            }
            let near_ocean = ocean_dist < beach_dist as f32;
            let beach_factor = if near_ocean { 1.0 - (ocean_dist / beach_dist as f32) } else { 0.0 };

            // Calculate surface normal for hillshading
            let get_h = |dx: i32, dy: i32| -> f32 {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                *heightmap.get(nx, ny)
            };

            let gx = (get_h(1, -1) + 2.0 * get_h(1, 0) + get_h(1, 1))
                   - (get_h(-1, -1) + 2.0 * get_h(-1, 0) + get_h(-1, 1));
            let gy = (get_h(-1, 1) + 2.0 * get_h(0, 1) + get_h(1, 1))
                   - (get_h(-1, -1) + 2.0 * get_h(0, -1) + get_h(1, -1));

            let scale = 0.0003;
            let snx = -gx * scale;
            let sny = -gy * scale;
            let snz = 1.0f32;
            let nlen = (snx * snx + sny * sny + snz * snz).sqrt();
            let (snx, sny, snz) = (snx / nlen, sny / nlen, snz / nlen);

            let diffuse = (snx * lx + sny * ly + snz * lz).max(0.0);
            let ambient = 0.35;
            let lighting = (ambient + (1.0 - ambient) * diffuse).min(1.0);

            // Normalized elevation for noise variation
            let normalized = (h - min_land) / land_range;

            // Use extended biome classification with fantasy variants
            let local_stress_val = *stress.get(x, y);
            let biome = classify_extended(
                h, temp, moist, local_stress_val,
                x, y, width, height,
                biome_config, &biome_noise,
            );

            // Get base color from extended biome
            let base = if h < 35.0 && near_ocean && beach_factor > 0.1 && h > 0.0 {
                // BEACH - override biome near coastline
                let sand_t = beach_factor.powf(0.7);
                if temp > 20.0 {
                    Rgb([(245.0 - sand_t * 25.0) as u8, (225.0 - sand_t * 35.0) as u8, (150.0 - sand_t * 50.0) as u8])
                } else if temp < 0.0 {
                    Rgb([(210.0 - sand_t * 15.0) as u8, (205.0 - sand_t * 15.0) as u8, (195.0 - sand_t * 10.0) as u8])
                } else {
                    Rgb([(235.0 - sand_t * 30.0) as u8, (210.0 - sand_t * 40.0) as u8, (155.0 - sand_t * 55.0) as u8])
                }
            } else {
                // Use extended biome colors
                match biome {
                    // Ocean biomes - temperature affects color
                    ExtendedBiome::DeepOcean => {
                        if temp < 0.0 { Rgb([25, 45, 70]) }
                        else if temp > 25.0 { Rgb([15, 50, 95]) }
                        else { Rgb([20, 45, 85]) }
                    }
                    ExtendedBiome::Ocean => {
                        if temp < 0.0 { Rgb([45, 70, 110]) }
                        else if temp > 25.0 { Rgb([30, 80, 140]) }
                        else { Rgb([35, 70, 125]) }
                    }
                    ExtendedBiome::CoastalWater => {
                        if temp < 0.0 { Rgb([100, 130, 150]) }
                        else if temp > 25.0 { Rgb([70, 170, 190]) }
                        else { Rgb([75, 140, 170]) }
                    }

                    // Cold biomes
                    ExtendedBiome::Ice => Rgb([235, 245, 255]),
                    ExtendedBiome::Tundra => Rgb([165, 160, 140]),
                    ExtendedBiome::BorealForest => Rgb([45, 85, 55]),

                    // Temperate biomes
                    ExtendedBiome::TemperateGrassland => Rgb([155, 175, 85]),
                    ExtendedBiome::TemperateForest => Rgb([50, 115, 50]),
                    ExtendedBiome::TemperateRainforest => Rgb([35, 95, 55]),

                    // Warm/dry biomes
                    ExtendedBiome::Desert => Rgb([220, 190, 130]),
                    ExtendedBiome::Savanna => Rgb([180, 165, 75]),
                    ExtendedBiome::TropicalForest => Rgb([40, 130, 45]),
                    ExtendedBiome::TropicalRainforest => Rgb([25, 100, 50]),

                    // Mountain biomes
                    ExtendedBiome::AlpineTundra => {
                        let volcanic = local_stress.powf(0.5);
                        if volcanic > 0.3 {
                            Rgb([(150.0 + volcanic * 60.0) as u8, (120.0 - volcanic * 30.0) as u8, (100.0 - volcanic * 20.0) as u8])
                        } else {
                            Rgb([145, 140, 130])
                        }
                    }
                    ExtendedBiome::SnowyPeaks => Rgb([250, 252, 255]),
                    ExtendedBiome::Foothills => Rgb([125, 150, 90]),             // Rolling olive-green hills
                    ExtendedBiome::Lagoon => Rgb([90, 175, 195]),                // Calm turquoise protected water

                    // ============ FANTASY BIOMES ============

                    // Fantasy Forests - vivid, otherworldly colors
                    ExtendedBiome::DeadForest => Rgb([75, 65, 55]),           // Gray-brown dead trees
                    ExtendedBiome::CrystalForest => Rgb([170, 210, 245]),     // Ice-blue crystalline
                    ExtendedBiome::BioluminescentForest => Rgb([30, 180, 140]), // Glowing cyan-green
                    ExtendedBiome::MushroomForest => Rgb([130, 70, 150]),     // Purple mushrooms
                    ExtendedBiome::PetrifiedForest => Rgb([95, 90, 85]),      // Stone gray

                    // Fantasy Waters - striking unusual colors
                    ExtendedBiome::AcidLake => Rgb([140, 170, 40]),           // Toxic yellow-green
                    ExtendedBiome::LavaLake => Rgb([255, 90, 15]),            // Bright orange-red
                    ExtendedBiome::FrozenLake => Rgb([190, 220, 240]),        // Pale ice blue
                    ExtendedBiome::BioluminescentWater => Rgb([40, 170, 190]), // Glowing cyan

                    // Wastelands - harsh, desolate colors
                    ExtendedBiome::VolcanicWasteland => Rgb([45, 25, 25]),    // Dark volcanic red-black
                    ExtendedBiome::SaltFlats => Rgb([235, 230, 215]),         // Bright white-cream
                    ExtendedBiome::Ashlands => Rgb([75, 75, 80]),             // Gray ash
                    ExtendedBiome::CrystalWasteland => Rgb([190, 170, 210]),  // Pale purple crystal

                    // Wetlands - murky, organic colors
                    ExtendedBiome::Swamp => Rgb([45, 75, 45]),                // Dark murky green
                    ExtendedBiome::Marsh => Rgb([75, 115, 65]),               // Muddy green
                    ExtendedBiome::Bog => Rgb([85, 65, 45]),                  // Brown peat
                    ExtendedBiome::MangroveSaltmarsh => Rgb([55, 95, 75]),    // Coastal green

                    // ============ ULTRA-RARE BIOMES ============

                    // Ancient/Primeval - mystical, ancient colors
                    ExtendedBiome::AncientGrove => Rgb([15, 55, 25]),         // Deep primeval green
                    ExtendedBiome::TitanBones => Rgb([195, 190, 175]),        // Bleached bone white
                    ExtendedBiome::CoralPlateau => Rgb([250, 175, 155]),      // Coral pink-orange

                    // Geothermal/Volcanic - intense thermal colors
                    ExtendedBiome::ObsidianFields => Rgb([25, 20, 30]),       // Deep black-purple
                    ExtendedBiome::Geysers => Rgb([175, 195, 215]),           // Steam blue-white
                    ExtendedBiome::TarPits => Rgb([15, 10, 5]),               // Pure black

                    // Magical/Anomalous - supernatural colors
                    ExtendedBiome::FloatingStones => Rgb([155, 135, 175]),    // Ethereal purple-gray
                    ExtendedBiome::Shadowfen => Rgb([25, 35, 30]),            // Deep shadow green
                    ExtendedBiome::PrismaticPools => Rgb([250, 145, 195]),    // Rainbow pink
                    ExtendedBiome::AuroraWastes => Rgb([95, 195, 175]),       // Aurora green-cyan

                    // Desert variants - distinctive desert colors
                    ExtendedBiome::SingingDunes => Rgb([225, 195, 135]),      // Golden sand
                    ExtendedBiome::Oasis => Rgb([45, 175, 75]),               // Vibrant green
                    ExtendedBiome::GlassDesert => Rgb([195, 215, 225]),       // Reflective blue-white

                    // Aquatic - deep sea colors
                    ExtendedBiome::AbyssalVents => Rgb([75, 15, 25]),         // Deep red-black
                    ExtendedBiome::Sargasso => Rgb([55, 95, 45]),             // Seaweed green

                    // NEW BIOMES - Mystical / Supernatural
                    ExtendedBiome::EtherealMist => Rgb([180, 190, 210]),      // Pale blue-gray mist
                    ExtendedBiome::StarfallCrater => Rgb([90, 60, 120]),      // Deep purple meteor
                    ExtendedBiome::LeyNexus => Rgb([200, 180, 255]),          // Bright magical purple
                    ExtendedBiome::WhisperingStones => Rgb([140, 135, 125]),  // Ancient gray stone
                    ExtendedBiome::SpiritMarsh => Rgb([120, 150, 140]),       // Ghostly green-gray

                    // NEW BIOMES - Extreme Geological
                    ExtendedBiome::SulfurVents => Rgb([220, 200, 60]),        // Bright yellow sulfur
                    ExtendedBiome::BasaltColumns => Rgb([50, 50, 55]),        // Dark basalt gray
                    ExtendedBiome::PaintedHills => Rgb([200, 140, 100]),      // Orange-red banded
                    ExtendedBiome::RazorPeaks => Rgb([100, 95, 105]),         // Sharp gray-purple
                    ExtendedBiome::SinkholeLakes => Rgb([40, 80, 100]),       // Deep blue sinkhole

                    // NEW BIOMES - Biological Wonders
                    ExtendedBiome::ColossalHive => Rgb([180, 140, 80]),       // Amber/honey color
                    ExtendedBiome::BoneFields => Rgb([230, 225, 210]),        // Pale bone white
                    ExtendedBiome::CarnivorousBog => Rgb([100, 60, 70]),      // Red-tinged dark
                    ExtendedBiome::FungalBloom => Rgb([200, 100, 180]),       // Bright pink-purple
                    ExtendedBiome::KelpTowers => Rgb([40, 90, 60]),           // Deep kelp green

                    // NEW BIOMES - Exotic Waters
                    ExtendedBiome::BrinePools => Rgb([60, 80, 90]),           // Dark salty blue
                    ExtendedBiome::HotSprings => Rgb([100, 180, 190]),        // Turquoise thermal
                    ExtendedBiome::MirrorLake => Rgb([150, 180, 200]),        // Reflective silver-blue
                    ExtendedBiome::InkSea => Rgb([15, 15, 25]),               // Near-black deep
                    ExtendedBiome::PhosphorShallows => Rgb([80, 200, 180]),   // Glowing cyan

                    // NEW BIOMES - Alien / Corrupted
                    ExtendedBiome::VoidScar => Rgb([40, 0, 50]),              // Deep void purple
                    ExtendedBiome::SiliconGrove => Rgb([180, 200, 220]),      // Crystalline blue-white
                    ExtendedBiome::SporeWastes => Rgb([160, 140, 100]),       // Sickly yellow-brown
                    ExtendedBiome::BleedingStone => Rgb([150, 60, 50]),       // Red iron-stained
                    ExtendedBiome::HollowEarth => Rgb([60, 50, 45]),          // Dark cavern brown

                    // NEW BIOMES - Ancient Ruins
                    ExtendedBiome::SunkenCity => Rgb([70, 90, 110]),          // Underwater stone
                    ExtendedBiome::CyclopeanRuins => Rgb([110, 105, 95]),     // Ancient weathered stone
                    ExtendedBiome::BuriedTemple => Rgb([170, 150, 120]),      // Sand-covered stone
                    ExtendedBiome::OvergrownCitadel => Rgb([60, 90, 50]),     // Vine-covered green
                    ExtendedBiome::DarkTower => Rgb([25, 20, 30]),            // Ominous dark obsidian

                    // OCEAN BIOMES - Realistic Shallow/Coastal
                    ExtendedBiome::CoralReef => Rgb([255, 180, 150]),         // Coral pink-orange
                    ExtendedBiome::KelpForest => Rgb([35, 80, 45]),           // Deep kelp green
                    ExtendedBiome::SeagrassMeadow => Rgb([50, 120, 70]),      // Seagrass green

                    // OCEAN BIOMES - Realistic Mid-depth
                    ExtendedBiome::ContinentalShelf => Rgb([45, 70, 110]),    // Sandy blue
                    ExtendedBiome::Seamount => Rgb([60, 50, 80]),             // Dark volcanic purple

                    // OCEAN BIOMES - Realistic Deep
                    ExtendedBiome::OceanicTrench => Rgb([10, 15, 35]),        // Ultra-deep blue-black
                    ExtendedBiome::AbyssalPlain => Rgb([25, 35, 55]),         // Deep gray-blue
                    ExtendedBiome::MidOceanRidge => Rgb([70, 40, 50]),        // Volcanic red-brown
                    ExtendedBiome::ColdSeep => Rgb([40, 50, 45]),             // Murky green-gray
                    ExtendedBiome::BrinePool => Rgb([35, 45, 60]),            // Dense blue-gray

                    // OCEAN BIOMES - Fantasy
                    ExtendedBiome::CrystalDepths => Rgb([120, 180, 220]),     // Crystal blue
                    ExtendedBiome::LeviathanGraveyard => Rgb([180, 175, 160]), // Bone white-gray
                    ExtendedBiome::DrownedCitadel => Rgb([80, 90, 100]),      // Stone gray-blue
                    ExtendedBiome::VoidMaw => Rgb([5, 0, 15]),                // Near-black purple
                    ExtendedBiome::PearlGardens => Rgb([200, 210, 230]),      // Pearl white-blue
                    ExtendedBiome::SirenShallows => Rgb([100, 180, 200]),     // Enchanted turquoise
                    ExtendedBiome::FrozenAbyss => Rgb([150, 180, 200]),       // Ice blue
                    ExtendedBiome::ThermalVents => Rgb([200, 80, 40]),        // Magma orange-red

                    // Karst & Cave biomes
                    ExtendedBiome::KarstPlains => Rgb([195, 190, 175]),       // Pale limestone gray
                    ExtendedBiome::TowerKarst => Rgb([175, 180, 160]),        // Gray-green pillars
                    ExtendedBiome::Sinkhole => Rgb([85, 75, 65]),             // Dark depression
                    ExtendedBiome::Cenote => Rgb([40, 120, 140]),             // Deep turquoise
                    ExtendedBiome::CaveEntrance => Rgb([45, 40, 35]),         // Dark cave mouth
                    ExtendedBiome::CockpitKarst => Rgb([165, 175, 145]),      // Green-gray mogotes

                    // Volcanic biomes
                    ExtendedBiome::Caldera => Rgb([70, 55, 50]),              // Dark volcanic brown
                    ExtendedBiome::ShieldVolcano => Rgb([60, 55, 45]),        // Dark basalt
                    ExtendedBiome::VolcanicCone => Rgb([90, 70, 60]),         // Volcanic gray-brown
                    ExtendedBiome::LavaField => Rgb([35, 25, 25]),            // Near-black basalt
                    ExtendedBiome::FumaroleField => Rgb([200, 190, 120]),     // Sulfur yellow
                    ExtendedBiome::VolcanicBeach => Rgb([40, 40, 45]),        // Black sand
                    ExtendedBiome::HotSpot => Rgb([180, 80, 50]),             // Warm orange-red
                }
            };

            // Add coherent texture variation using multi-scale fractal noise
            // This creates smooth, natural-looking variation instead of grid patterns
            let xf = x as f32;
            let yf = y as f32;

            // Different noise scales for different terrain types
            let (noise_scale, noise_strength) = if normalized < 0.25 {
                // Vegetation - larger scale patches (forests/grasslands)
                (12.0, 10.0)
            } else if normalized < 0.65 {
                // Highland/foothills - medium scale rocky variation
                (8.0, 8.0)
            } else if normalized < 0.90 {
                // Mountain rock - finer detail
                (6.0, 6.0)
            } else {
                // Snow - subtle variation
                (10.0, 3.0)
            };

            // Use fractal noise for natural variation (2 octaves for smoother look)
            let noise = fractal_noise(xf, yf, noise_scale, 2, 0.5, 42);

            // Apply noise as color modulation (same noise for all channels to avoid color shift)
            let color_mod = noise * noise_strength;

            // Apply lighting with coherent texture variation
            let r = ((base[0] as f32 + color_mod) * lighting).clamp(0.0, 255.0) as u8;
            let g = ((base[1] as f32 + color_mod * 0.8) * lighting).clamp(0.0, 255.0) as u8;
            let b = ((base[2] as f32 + color_mod * 0.6) * lighting).clamp(0.0, 255.0) as u8;
            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }
    img
}

/// Backward-compatible wrapper - generates approximate temperature/stress from heightmap
pub fn render_terrain_shaded(heightmap: &Tilemap<f32>) -> RgbImage {
    // Generate simple latitude-based temperature
    let width = heightmap.width;
    let height = heightmap.height;
    let mut temperature = Tilemap::new_with(width, height, 15.0f32);
    let mut stress = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        // Latitude-based temperature: warm at equator, cold at poles
        let lat = (y as f32 / height as f32 - 0.5).abs() * 2.0; // 0 at equator, 1 at poles
        let base_temp = 30.0 - lat * 60.0; // 30°C at equator, -30°C at poles
        for x in 0..width {
            let h = *heightmap.get(x, y);
            // Altitude lapse rate: ~6.5°C per 1000m
            let altitude_effect = if h > 0.0 { h * 0.0065 } else { 0.0 };
            temperature.set(x, y, base_temp - altitude_effect);
        }
    }

    render_terrain_shaded_enhanced(heightmap, &temperature, &stress)
}

/// Render shaded terrain with animated water effects
/// `time` is in seconds, used for wave animation
pub fn render_terrain_shaded_animated(heightmap: &Tilemap<f32>, time: f64) -> RgbImage {
    use crate::erosion::rivers::{compute_flow_direction, compute_flow_accumulation};

    let width = heightmap.width;
    let height = heightmap.height;
    let mut img: RgbImage = ImageBuffer::new(width as u32, height as u32);

    // Compute flow accumulation to find rivers
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

    // Find max flow for normalization
    let mut max_flow: f32 = 0.0;
    for (_, _, &acc) in flow_acc.iter() {
        if acc > max_flow { max_flow = acc; }
    }

    // Scale thresholds based on map resolution (same as non-animated version)
    let cell_count = (width * height) as f32;
    let base_cell_count = 131072.0f32;
    let scale_factor = (cell_count / base_cell_count).sqrt();

    // River thresholds - higher values = fewer, cleaner rivers
    let river_threshold = 250.0 * scale_factor;
    let min_river_length = (15.0 * scale_factor).max(15.0) as usize;

    // D8 direction offsets
    const DX: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
    const DY: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    const NO_FLOW: u8 = 255;

    // Compute river maps (same as non-animated version)
    let mut is_headwater: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut above_threshold: Tilemap<bool> = Tilemap::new_with(width, height, false);

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let flow = *flow_acc.get(x, y);
            if h >= 0.0 && flow > river_threshold {
                above_threshold.set(x, y, true);
            }
        }
    }

    for y in 0..height {
        for x in 0..width {
            if !*above_threshold.get(x, y) { continue; }
            let mut has_upstream_river = false;
            for dir in 0..8u8 {
                let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;
                if !*above_threshold.get(nx, ny) { continue; }
                let neighbor_dir = *flow_dir.get(nx, ny);
                if neighbor_dir < 8 {
                    let opposite = (dir + 4) % 8;
                    if neighbor_dir == opposite {
                        has_upstream_river = true;
                        break;
                    }
                }
            }
            if !has_upstream_river { is_headwater.set(x, y, true); }
        }
    }

    let mut valid_river: Tilemap<bool> = Tilemap::new_with(width, height, false);
    for y in 0..height {
        for x in 0..width {
            if !*is_headwater.get(x, y) || *valid_river.get(x, y) { continue; }
            let mut path: Vec<(usize, usize)> = Vec::new();
            let mut cx = x;
            let mut cy = y;
            let mut reached_ocean = false;
            let mut joined_valid_river = false;
            for _ in 0..10000 {
                let h = *heightmap.get(cx, cy);
                if h < 0.0 { reached_ocean = true; break; }
                if *valid_river.get(cx, cy) { joined_valid_river = true; break; }
                if *above_threshold.get(cx, cy) { path.push((cx, cy)); }
                let dir = *flow_dir.get(cx, cy);
                if dir == NO_FLOW || dir >= 8 { break; }
                let nx = (cx as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = (cy as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;
                if nx == cx && ny == cy { break; }
                cx = nx; cy = ny;
            }
            if reached_ocean || joined_valid_river || path.len() >= min_river_length {
                for (px, py) in path { valid_river.set(px, py, true); }
            }
        }
    }

    let mut river_width_map: Tilemap<u8> = Tilemap::new_with(width, height, 0u8);
    for y in 0..height {
        for x in 0..width {
            if !*valid_river.get(x, y) { continue; }
            let flow = *flow_acc.get(x, y);
            let w = if flow > 500.0 { 4 } else if flow > 200.0 { 3 } else if flow > 100.0 { 2 } else { 1 };
            river_width_map.set(x, y, w);
        }
    }

    let mut is_river: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut river_flow: Tilemap<f32> = Tilemap::new_with(width, height, 0.0);
    for y in 0..height {
        for x in 0..width {
            let w = *river_width_map.get(x, y) as i32;
            if w > 0 {
                let flow = *flow_acc.get(x, y);
                for dy in -w..=w {
                    for dx in -w..=w {
                        if dx*dx + dy*dy <= w*w {
                            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                            if *heightmap.get(nx, ny) >= 0.0 {
                                is_river.set(nx, ny, true);
                                let existing = *river_flow.get(nx, ny);
                                if flow > existing { river_flow.set(nx, ny, flow); }
                            }
                        }
                    }
                }
            }
        }
    }

    // Compute distance to shore for wave effects
    let mut shore_distance: Tilemap<f32> = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: std::collections::VecDeque<(usize, usize, f32)> = std::collections::VecDeque::new();

    // Find shore cells (ocean cells adjacent to land)
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h < 0.0 {
                // Check if adjacent to land
                let mut near_land = false;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 { continue; }
                        let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                        if *heightmap.get(nx, ny) >= 0.0 {
                            near_land = true;
                            break;
                        }
                    }
                    if near_land { break; }
                }
                if near_land {
                    shore_distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                }
            }
        }
    }

    // BFS to compute distance from shore
    while let Some((x, y, dist)) = queue.pop_front() {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                if *heightmap.get(nx, ny) >= 0.0 { continue; } // Skip land
                let step = if dx == 0 || dy == 0 { 1.0 } else { 1.414 };
                let new_dist = dist + step;
                if new_dist < *shore_distance.get(nx, ny) && new_dist < 50.0 {
                    shore_distance.set(nx, ny, new_dist);
                    queue.push_back((nx, ny, new_dist));
                }
            }
        }
    }

    // Find height range for land
    let mut min_land = f32::MAX;
    let mut max_land = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h >= 0.0 {
            if h < min_land { min_land = h; }
            if h > max_land { max_land = h; }
        }
    }
    let land_range = (max_land - min_land).max(1.0);

    // Light direction
    let light_x = -0.7f32;
    let light_y = -0.7f32;
    let light_z = 0.5f32;
    let light_len = (light_x * light_x + light_y * light_y + light_z * light_z).sqrt();
    let (lx, ly, lz) = (light_x / light_len, light_y / light_len, light_z / light_len);

    // Animation parameters
    let t = time as f32;

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let xf = x as f32;
            let yf = y as f32;

            // Ocean with animated waves
            if h < 0.0 {
                let depth = -h;
                let dist_to_shore = *shore_distance.get(x, y);

                // Base ocean color based on depth
                let (base_r, base_g, base_b) = if depth < 50.0 {
                    let t_depth = depth / 50.0;
                    (100.0 - t_depth * 40.0, 180.0 - t_depth * 30.0, 200.0 - t_depth * 10.0)
                } else if depth < 200.0 {
                    let t_depth = (depth - 50.0) / 150.0;
                    (60.0 - t_depth * 30.0, 150.0 - t_depth * 40.0, 190.0 - t_depth * 20.0)
                } else if depth < 1000.0 {
                    let t_depth = (depth - 200.0) / 800.0;
                    (30.0 - t_depth * 15.0, 110.0 - t_depth * 40.0, 170.0 - t_depth * 30.0)
                } else if depth < 3000.0 {
                    let t_depth = (depth - 1000.0) / 2000.0;
                    (15.0 - t_depth * 10.0, 70.0 - t_depth * 30.0, 140.0 - t_depth * 40.0)
                } else {
                    (5.0, 40.0 - ((depth - 3000.0) / 2000.0).min(1.0) * 20.0, 100.0 - ((depth - 3000.0) / 2000.0).min(1.0) * 30.0)
                };

                // Wave animation - multiple overlapping waves
                let wave1 = ((xf * 0.05 + t * 1.2).sin() * (yf * 0.03 + t * 0.8).cos()) * 0.5;
                let wave2 = ((xf * 0.08 - t * 0.9).cos() * (yf * 0.06 + t * 1.1).sin()) * 0.3;
                let wave3 = ((xf * 0.12 + yf * 0.04 + t * 1.5).sin()) * 0.2;
                let wave_height = (wave1 + wave2 + wave3) * 0.5 + 0.5; // Normalize to 0-1

                // Foam near shore - more intense closer to land
                let foam_intensity = if dist_to_shore < 15.0 {
                    let base_foam = (1.0 - dist_to_shore / 15.0).powf(1.5);
                    // Animated foam pattern
                    let foam_wave = ((xf * 0.15 + t * 2.0).sin() * (yf * 0.1 - t * 1.5).cos() + 1.0) * 0.5;
                    // Breaking wave effect - foam pulses (waves move towards shore)
                    let break_pattern = ((dist_to_shore * 0.3 + t * 3.0).sin() + 1.0) * 0.5;
                    base_foam * foam_wave * break_pattern
                } else {
                    0.0
                };

                // Specular highlights (sun reflection on waves)
                let specular = if depth < 500.0 {
                    let spec_wave = ((xf * 0.1 + t * 0.5).sin() * (yf * 0.08 + t * 0.7).cos() + 1.0) * 0.5;
                    let spec_intensity = spec_wave.powf(8.0) * 0.3 * (1.0 - depth / 500.0);
                    spec_intensity
                } else {
                    0.0
                };

                // Caustics effect for shallow water
                let caustics = if depth < 100.0 {
                    let c1 = ((xf * 0.2 + t * 0.8).sin() * (yf * 0.25 + t * 0.6).cos());
                    let c2 = ((xf * 0.18 - t * 0.7).cos() * (yf * 0.22 - t * 0.9).sin());
                    let caustic_pattern = (c1 * c2 + 0.5).max(0.0).powf(2.0);
                    caustic_pattern * 0.15 * (1.0 - depth / 100.0)
                } else {
                    0.0
                };

                // Combine effects
                let wave_brightness = 1.0 + wave_height * 0.15 * (1.0 - (depth / 1000.0).min(1.0));

                let r = ((base_r * wave_brightness + foam_intensity * 200.0 + specular * 255.0 + caustics * 80.0).clamp(0.0, 255.0)) as u8;
                let g = ((base_g * wave_brightness + foam_intensity * 220.0 + specular * 255.0 + caustics * 100.0).clamp(0.0, 255.0)) as u8;
                let b = ((base_b * wave_brightness + foam_intensity * 240.0 + specular * 255.0 + caustics * 50.0).clamp(0.0, 255.0)) as u8;

                img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
                continue;
            }

            // Rivers (static, no animation)
            if *is_river.get(x, y) {
                let flow = *river_flow.get(x, y);
                let flow_intensity = ((flow - river_threshold) / (max_flow - river_threshold + 1.0)).min(1.0);

                let r = (40.0 - flow_intensity * 20.0) as u8;
                let g = (100.0 + flow_intensity * 50.0).min(255.0) as u8;
                let b = (180.0 + flow_intensity * 60.0).min(255.0) as u8;
                img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
                continue;
            }

            // Land rendering (same as non-animated version)
            let mut near_ocean = false;
            let beach_check_dist = 3i32;
            'beach_check: for dy in -beach_check_dist..=beach_check_dist {
                for dx in -beach_check_dist..=beach_check_dist {
                    let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    if *heightmap.get(nx, ny) < 0.0 {
                        near_ocean = true;
                        break 'beach_check;
                    }
                }
            }

            let get_h = |dx: i32, dy: i32| -> f32 {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                *heightmap.get(nx, ny)
            };

            let gx = (get_h(1, -1) + 2.0 * get_h(1, 0) + get_h(1, 1))
                   - (get_h(-1, -1) + 2.0 * get_h(-1, 0) + get_h(-1, 1));
            let gy = (get_h(-1, 1) + 2.0 * get_h(0, 1) + get_h(1, 1))
                   - (get_h(-1, -1) + 2.0 * get_h(0, -1) + get_h(1, -1));

            let scale = 0.0003;
            let nx = -gx * scale;
            let ny = -gy * scale;
            let nz = 1.0f32;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
            let (nx, ny, nz) = (nx / nlen, ny / nlen, nz / nlen);

            let diffuse = (nx * lx + ny * ly + nz * lz).max(0.0);
            let ambient = 0.35;
            let lighting = (ambient + (1.0 - ambient) * diffuse).min(1.0);

            let normalized = (h - min_land) / land_range;

            let base = if h < 8.0 && near_ocean {
                let sand_intensity = (1.0 - h / 8.0).max(0.0);
                Rgb([
                    (210.0 - (1.0 - sand_intensity) * 60.0) as u8,
                    (190.0 - (1.0 - sand_intensity) * 50.0) as u8,
                    (140.0 - (1.0 - sand_intensity) * 40.0) as u8,
                ])
            } else if normalized < 0.08 {
                let t_n = normalized / 0.08;
                Rgb([(150.0 - t_n * 70.0) as u8, (160.0 - t_n * 10.0) as u8, (100.0 - t_n * 40.0) as u8])
            } else if normalized < 0.25 {
                let t_n = (normalized - 0.08) / 0.17;
                Rgb([(80.0 - t_n * 20.0) as u8, (150.0 - t_n * 30.0) as u8, (60.0 - t_n * 15.0) as u8])
            } else if normalized < 0.45 {
                let t_n = (normalized - 0.25) / 0.20;
                Rgb([(60.0 + t_n * 80.0) as u8, (120.0 - t_n * 30.0) as u8, (45.0 + t_n * 15.0) as u8])
            } else if normalized < 0.65 {
                let t_n = (normalized - 0.45) / 0.20;
                Rgb([(140.0 + t_n * 20.0) as u8, (90.0 + t_n * 10.0) as u8, (60.0 + t_n * 20.0) as u8])
            } else if normalized < 0.82 {
                let t_n = (normalized - 0.65) / 0.17;
                Rgb([(160.0 - t_n * 30.0) as u8, (100.0 + t_n * 20.0) as u8, (80.0 + t_n * 30.0) as u8])
            } else if normalized < 0.92 {
                let t_n = (normalized - 0.82) / 0.10;
                Rgb([(130.0 + t_n * 90.0) as u8, (120.0 + t_n * 100.0) as u8, (110.0 + t_n * 115.0) as u8])
            } else {
                let t_n = ((normalized - 0.92) / 0.08).min(1.0);
                let base_snow = 220.0 + t_n * 25.0;
                Rgb([base_snow as u8, base_snow as u8, (base_snow + 5.0) as u8])
            };

            let (noise_scale, noise_strength) = if normalized < 0.25 { (12.0, 10.0) }
                else if normalized < 0.65 { (8.0, 8.0) }
                else if normalized < 0.90 { (6.0, 6.0) }
                else { (10.0, 3.0) };

            let noise = fractal_noise(xf, yf, noise_scale, 2, 0.5, 42);
            let color_mod = noise * noise_strength;

            let r = ((base[0] as f32 + color_mod) * lighting).clamp(0.0, 255.0) as u8;
            let g = ((base[1] as f32 + color_mod * 0.8) * lighting).clamp(0.0, 255.0) as u8;
            let b = ((base[2] as f32 + color_mod * 0.6) * lighting).clamp(0.0, 255.0) as u8;
            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }
    img
}

/// Render globe projection to image buffer with terrain colors
pub fn render_globe(heightmap: &Tilemap<f32>, _plate_map: &Tilemap<PlateId>, _plates: &[Plate], rotation: f64) -> RgbImage {
    let size = heightmap.height.max(heightmap.width / 2);
    let mut img: RgbImage = ImageBuffer::new(size as u32, size as u32);

    let radius = size as f64 / 2.0 - 10.0;
    let center_x = size as f64 / 2.0;
    let center_y = size as f64 / 2.0;
    let light_dir = normalize_vec(1.0, 1.0, 0.8);

    // Find height range for land normalization
    let mut min_land = f32::MAX;
    let mut max_land = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h >= 0.0 {
            if h < min_land { min_land = h; }
            if h > max_land { max_land = h; }
        }
    }
    let land_range = (max_land - min_land).max(1.0);

    for py in 0..size {
        for px in 0..size {
            let x = (px as f64 - center_x) / radius;
            let y = (center_y - py as f64) / radius;
            let r_squared = x * x + y * y;

            if r_squared > 1.0 {
                img.put_pixel(px as u32, py as u32, Rgb([5, 5, 15]));
                continue;
            }

            let z = (1.0 - r_squared).sqrt();
            let lat = y.asin();
            let lon = ((x.atan2(z) + rotation) % (2.0 * PI) + 2.0 * PI) % (2.0 * PI);
            let map_x = (lon / (2.0 * PI) * heightmap.width as f64) as usize % heightmap.width;
            let map_y = ((0.5 - lat / PI) * heightmap.height as f64).clamp(0.0, heightmap.height as f64 - 1.0) as usize;

            let h = *heightmap.get(map_x, map_y);

            // Get terrain color based on elevation (similar to render_terrain_shaded)
            let base_color: [u8; 3] = if h < 0.0 {
                // Ocean - depth-based coloring
                let depth = -h;
                if depth < 50.0 {
                    let t = depth / 50.0;
                    [(100.0 - t * 40.0) as u8, (180.0 - t * 30.0) as u8, (200.0 - t * 10.0) as u8]
                } else if depth < 200.0 {
                    let t = (depth - 50.0) / 150.0;
                    [(60.0 - t * 30.0) as u8, (150.0 - t * 40.0) as u8, (190.0 - t * 20.0) as u8]
                } else if depth < 1000.0 {
                    let t = (depth - 200.0) / 800.0;
                    [(30.0 - t * 15.0) as u8, (110.0 - t * 40.0) as u8, (170.0 - t * 30.0) as u8]
                } else {
                    let t = ((depth - 1000.0) / 2000.0).min(1.0);
                    [(15.0 - t * 10.0) as u8, (70.0 - t * 30.0) as u8, (140.0 - t * 40.0) as u8]
                }
            } else {
                // Land - elevation-based coloring
                let normalized = (h - min_land) / land_range;
                if normalized < 0.15 {
                    // Coastal lowland - green
                    [80, 150, 60]
                } else if normalized < 0.35 {
                    // Lowland/plains - varied green
                    let t = (normalized - 0.15) / 0.20;
                    [(70.0 + t * 30.0) as u8, (140.0 - t * 20.0) as u8, (55.0) as u8]
                } else if normalized < 0.55 {
                    // Foothills - tan/brown
                    let t = (normalized - 0.35) / 0.20;
                    [(100.0 + t * 40.0) as u8, (120.0 - t * 20.0) as u8, (55.0 + t * 15.0) as u8]
                } else if normalized < 0.75 {
                    // Mountains - gray
                    let t = (normalized - 0.55) / 0.20;
                    let v = (140.0 - t * 20.0) as u8;
                    [v, (100.0 + t * 15.0) as u8, (70.0 + t * 25.0) as u8]
                } else if normalized < 0.90 {
                    // High mountains - lighter gray
                    let t = (normalized - 0.75) / 0.15;
                    [(120.0 + t * 80.0) as u8, (115.0 + t * 85.0) as u8, (95.0 + t * 100.0) as u8]
                } else {
                    // Snow peaks
                    [230, 235, 240]
                }
            };

            // Calculate sphere lighting
            let normal = (x, y, z);
            let diffuse = (normal.0 * light_dir.0 + normal.1 * light_dir.1 + normal.2 * light_dir.2).max(0.0);
            let ambient = 0.35;
            let light_intensity = ambient + (1.0 - ambient) * diffuse;

            // Slight specular for oceans (water reflection)
            let specular = if h < 0.0 {
                let reflect = 2.0 * diffuse * z - light_dir.2;
                reflect.max(0.0).powi(8) * 0.15
            } else {
                0.0
            };

            let final_intensity = (light_intensity + specular).clamp(0.25, 1.2);

            let r = ((base_color[0] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
        }
    }

    // Atmosphere glow - blueish for Earth-like appearance
    let glow_radius = radius * 1.12;
    for py in 0..size {
        for px in 0..size {
            let x = px as f64 - center_x;
            let y = py as f64 - center_y;
            let dist = (x * x + y * y).sqrt();
            if dist > radius && dist < glow_radius {
                let t = (dist - radius) / (glow_radius - radius);
                let glow_strength = (1.0 - t).powi(3) * 0.5;
                let pixel = img.get_pixel(px as u32, py as u32);
                let r = (pixel[0] as f64 + 80.0 * glow_strength).min(255.0) as u8;
                let g = (pixel[1] as f64 + 140.0 * glow_strength).min(255.0) as u8;
                let b = (pixel[2] as f64 + 220.0 * glow_strength).min(255.0) as u8;
                img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
            }
        }
    }

    img
}

/// Render info panel
fn render_info_panel(width: u32, height: u32, seed: u64, num_plates: usize) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(width, height, Rgb([40, 40, 50]));

    // Draw some info text
    draw_label(&mut img, &format!("Seed: {}", seed), 10, 30);
    draw_label(&mut img, &format!("Plates: {}", num_plates), 10, 60);
    draw_label(&mut img, &format!("Size: {}x{}", width, height), 10, 90);

    img
}

/// Simple pixel-based text drawing (very basic 5x7 font)
fn draw_label(img: &mut RgbImage, text: &str, x: u32, y: u32) {
    // Simple approach: just draw white pixels for each character position
    // This is a minimal implementation - each char is ~6 pixels wide
    let color = Rgb([220, 220, 220]);

    for (i, c) in text.chars().enumerate() {
        let cx = x + (i as u32) * 6;
        draw_char(img, c, cx, y, color);
    }
}

/// Draw a single character using a minimal 5x7 bitmap font
fn draw_char(img: &mut RgbImage, c: char, x: u32, y: u32, color: Rgb<u8>) {
    let bitmap = get_char_bitmap(c);
    for (row, bits) in bitmap.iter().enumerate() {
        for col in 0..5 {
            if (bits >> (4 - col)) & 1 == 1 {
                let px = x + col;
                let py = y + row as u32;
                if px < img.width() && py < img.height() {
                    img.put_pixel(px, py, color);
                }
            }
        }
    }
}

/// Get 5x7 bitmap for a character (returns 7 rows of 5-bit patterns)
fn get_char_bitmap(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        'a' => [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
        'b' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110],
        'c' => [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110],
        'd' => [0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111],
        'e' => [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
        'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
        'g' => [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'h' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        'i' => [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        'j' => [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100],
        'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
        'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'm' => [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001],
        'n' => [0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        'o' => [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'p' => [0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'q' => [0b00000, 0b00000, 0b01101, 0b10011, 0b01111, 0b00001, 0b00001],
        'r' => [0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
        's' => [0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110],
        't' => [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110],
        'u' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101],
        'v' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'w' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
        'x' => [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        'y' => [0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'z' => [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ':' => [0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000],
        ' ' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        _ => [0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111], // Unknown char
    }
}
