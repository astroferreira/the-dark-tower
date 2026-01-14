use std::f64::consts::PI;

use image::{ImageBuffer, Rgb, RgbImage};

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
pub fn export_combined_grid(
    heightmap: &Tilemap<f32>,
    heightmap_normalized: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    biomes: &Tilemap<ClimateBiome>,
    path: &str,
    seed: u64,
) -> Result<(), image::ImageError> {
    use crate::heightmap::generate_land_mask;

    let tile_w = heightmap.width as u32;
    let tile_h = heightmap.height as u32;

    // Grid: 3 columns x 3 rows
    let cols = 3u32;
    let rows = 3u32;
    let label_height = 20u32;

    let grid_w = tile_w * cols;
    let grid_h = (tile_h + label_height) * rows;

    let mut grid: RgbImage = ImageBuffer::from_pixel(grid_w, grid_h, Rgb([30, 30, 30]));

    // Generate land mask for visualization
    let land_mask = generate_land_mask(plate_map, plates, seed);

    // Generate each tile and copy to grid
    let info_label = format!("Info");
    let tiles: Vec<(&str, RgbImage)> = vec![
        ("Plates", render_plate_map(plate_map, plates)),
        ("Types", render_plate_types(plate_map, plates)),
        ("Stress", render_stress_map(stress_map)),
        ("Land", render_land_mask(plate_map, plates, &land_mask)),
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
    seed: u64,
) -> RgbImage {
    use crate::heightmap::generate_land_mask;

    let tile_w = heightmap.width as u32;
    let tile_h = heightmap.height as u32;

    // Grid: 3 columns x 3 rows
    let cols = 3u32;
    let rows = 3u32;
    let label_height = 20u32;

    let grid_w = tile_w * cols;
    let grid_h = (tile_h + label_height) * rows;

    let mut grid: RgbImage = ImageBuffer::from_pixel(grid_w, grid_h, Rgb([30, 30, 30]));

    // Generate land mask for visualization
    let land_mask = generate_land_mask(plate_map, plates, seed);

    // Generate each tile and copy to grid
    let info_label = format!("Info");
    let tiles: Vec<(&str, RgbImage)> = vec![
        ("Plates", render_plate_map(plate_map, plates)),
        ("Types", render_plate_types(plate_map, plates)),
        ("Stress", render_stress_map(stress_map)),
        ("Land", render_land_mask(plate_map, plates, &land_mask)),
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

/// Render shaded terrain to image buffer with proper hillshade
pub fn render_terrain_shaded(heightmap: &Tilemap<f32>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::new(heightmap.width as u32, heightmap.height as u32);
    
    // Light from northwest at 45° elevation (like traditional hillshade)
    // Azimuth: 315° (NW), Elevation: 45°
    let azimuth_rad = 315.0_f32.to_radians();
    let elevation_rad = 45.0_f32.to_radians();
    let light_dir: [f32; 3] = [
        azimuth_rad.cos() * elevation_rad.cos(),
        -azimuth_rad.sin() * elevation_rad.cos(), // Negative because screen Y is inverted
        elevation_rad.sin(),
    ];

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let height = *heightmap.get(x, y);
            
            // Ocean is masked out - dark background
            if height < 0.0 {
                img.put_pixel(x as u32, y as u32, Rgb([15, 25, 40]));
                continue;
            }
            
            let normal = calculate_normal(heightmap, x, y);
            
            // Lambert diffuse lighting
            let diffuse = (normal[0] * light_dir[0] + normal[1] * light_dir[1] + normal[2] * light_dir[2]).max(0.0);
            
            // Ambient + diffuse, with subtle highlighting
            let ambient = 0.35;
            let shade = ambient + (1.0 - ambient) * diffuse;
            
            let base_color = land_color(height);
            let r = ((base_color[0] as f32 * shade).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f32 * shade).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f32 * shade).clamp(0.0, 255.0)) as u8;
            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }
    img
}

/// Render globe projection to image buffer
pub fn render_globe(heightmap: &Tilemap<f32>, plate_map: &Tilemap<PlateId>, plates: &[Plate], rotation: f64) -> RgbImage {
    let size = heightmap.height.max(heightmap.width / 2);
    let mut img: RgbImage = ImageBuffer::new(size as u32, size as u32);

    let radius = size as f64 / 2.0 - 10.0;
    let center_x = size as f64 / 2.0;
    let center_y = size as f64 / 2.0;
    let light_dir = normalize_vec(1.0, 1.0, 0.8);

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

            let height = *heightmap.get(map_x, map_y);
            let plate_id = *plate_map.get(map_x, map_y);
            let base_color = if plate_id.is_none() { [50, 50, 80] } else { plates[plate_id.0 as usize].color };

            let normal = (x, y, z);
            let diffuse = (normal.0 * light_dir.0 + normal.1 * light_dir.1 + normal.2 * light_dir.2).max(0.0);
            let light_intensity = 0.3 + 0.7 * diffuse;
            let height_boost = 1.0 + (height as f64 - 0.5) * 0.3;
            let final_intensity = (light_intensity * height_boost).clamp(0.3, 1.3);

            let r = ((base_color[0] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f64 * final_intensity).clamp(0.0, 255.0)) as u8;
            img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
        }
    }

    // Atmosphere glow
    let glow_radius = radius * 1.15;
    for py in 0..size {
        for px in 0..size {
            let x = px as f64 - center_x;
            let y = py as f64 - center_y;
            let dist = (x * x + y * y).sqrt();
            if dist > radius && dist < glow_radius {
                let t = (dist - radius) / (glow_radius - radius);
                let glow_strength = (1.0 - t).powi(2) * 0.4;
                let pixel = img.get_pixel(px as u32, py as u32);
                let r = (pixel[0] as f64 + 100.0 * glow_strength).min(255.0) as u8;
                let g = (pixel[1] as f64 + 150.0 * glow_strength).min(255.0) as u8;
                let b = (pixel[2] as f64 + 255.0 * glow_strength).min(255.0) as u8;
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
