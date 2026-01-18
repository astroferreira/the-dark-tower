//! Export functionality for local maps.
//!
//! Provides PNG export for local map visualization.

use image::{ImageBuffer, Rgb, RgbImage};

use super::types::LocalMap;

/// Export a local map as a PNG image
pub fn export_local_map(local_map: &LocalMap, path: &str) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(local_map.width as u32, local_map.height as u32);

    for (x, y, tile) in local_map.iter() {
        let (r, g, b) = tile.color();
        img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
    }

    img.save(path)
}

/// Export a local map with scaling (each tile becomes scale x scale pixels)
pub fn export_local_map_scaled(
    local_map: &LocalMap,
    path: &str,
    scale: u32,
) -> Result<(), image::ImageError> {
    let width = local_map.width as u32 * scale;
    let height = local_map.height as u32 * scale;
    let mut img: RgbImage = ImageBuffer::new(width, height);

    for (x, y, tile) in local_map.iter() {
        let (r, g, b) = tile.color();
        let color = Rgb([r, g, b]);

        // Fill scale x scale pixels
        for dy in 0..scale {
            for dx in 0..scale {
                let px = x as u32 * scale + dx;
                let py = y as u32 * scale + dy;
                img.put_pixel(px, py, color);
            }
        }
    }

    img.save(path)
}

/// Export a local map with terrain and feature overlay
pub fn export_local_map_detailed(
    local_map: &LocalMap,
    path: &str,
    scale: u32,
) -> Result<(), image::ImageError> {
    let width = local_map.width as u32 * scale;
    let height = local_map.height as u32 * scale;
    let mut img: RgbImage = ImageBuffer::new(width, height);

    for (x, y, tile) in local_map.iter() {
        // Get terrain color as base
        let (tr, tg, tb) = tile.terrain.color();

        // Fill base terrain
        for dy in 0..scale {
            for dx in 0..scale {
                let px = x as u32 * scale + dx;
                let py = y as u32 * scale + dy;
                img.put_pixel(px, py, Rgb([tr, tg, tb]));
            }
        }

        // If there's a feature, draw it in the center
        if let Some(feature) = tile.feature {
            let (fr, fg, fb) = feature.color();

            // Draw feature in center portion
            let margin = scale / 4;
            let feature_size = scale - margin * 2;

            if feature_size > 0 {
                for dy in margin..(margin + feature_size) {
                    for dx in margin..(margin + feature_size) {
                        let px = x as u32 * scale + dx;
                        let py = y as u32 * scale + dy;
                        img.put_pixel(px, py, Rgb([fr, fg, fb]));
                    }
                }
            }
        }

        // Add grid lines if scale is large enough
        if scale >= 8 {
            // Draw right and bottom edges
            for d in 0..scale {
                let right_x = x as u32 * scale + scale - 1;
                let right_y = y as u32 * scale + d;
                let bottom_x = x as u32 * scale + d;
                let bottom_y = y as u32 * scale + scale - 1;

                // Dim grid lines (check bounds manually)
                if right_x < width && right_y < height {
                    let pixel = img.get_pixel(right_x, right_y);
                    img.put_pixel(right_x, right_y, Rgb([
                        (pixel[0] as u16 * 8 / 10) as u8,
                        (pixel[1] as u16 * 8 / 10) as u8,
                        (pixel[2] as u16 * 8 / 10) as u8,
                    ]));
                }
                if bottom_x < width && bottom_y < height {
                    let pixel = img.get_pixel(bottom_x, bottom_y);
                    img.put_pixel(bottom_x, bottom_y, Rgb([
                        (pixel[0] as u16 * 8 / 10) as u8,
                        (pixel[1] as u16 * 8 / 10) as u8,
                        (pixel[2] as u16 * 8 / 10) as u8,
                    ]));
                }
            }
        }
    }

    img.save(path)
}

/// Export a walkability map (white = walkable, black = blocked)
pub fn export_walkability_map(
    local_map: &LocalMap,
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(local_map.width as u32, local_map.height as u32);

    for (x, y, tile) in local_map.iter() {
        let color = if tile.walkable {
            Rgb([255, 255, 255])
        } else {
            Rgb([0, 0, 0])
        };
        img.put_pixel(x as u32, y as u32, color);
    }

    img.save(path)
}

/// Export a movement cost map (gradient from green to red based on cost)
pub fn export_movement_cost_map(
    local_map: &LocalMap,
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(local_map.width as u32, local_map.height as u32);

    // Find max movement cost (excluding infinity)
    let max_cost = local_map
        .iter()
        .map(|(_, _, t)| t.movement_cost)
        .filter(|c| c.is_finite())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0);

    for (x, y, tile) in local_map.iter() {
        let color = if !tile.movement_cost.is_finite() {
            Rgb([0, 0, 0]) // Black for impassable
        } else {
            let t = (tile.movement_cost / max_cost).clamp(0.0, 1.0);
            // Green (low cost) to Yellow to Red (high cost)
            let r = (t * 2.0 * 255.0).min(255.0) as u8;
            let g = ((1.0 - t) * 2.0 * 255.0).min(255.0) as u8;
            Rgb([r, g, 0])
        };
        img.put_pixel(x as u32, y as u32, color);
    }

    img.save(path)
}

/// Export an elevation heatmap (Blue→Green→Brown→White gradient)
pub fn export_elevation_heatmap(
    local_map: &LocalMap,
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(local_map.width as u32, local_map.height as u32);

    // Find min/max elevation for normalization
    let (min_elev, max_elev) = local_map.iter().fold((f32::MAX, f32::MIN), |(min, max), (_, _, t)| {
        (min.min(t.elevation_offset), max.max(t.elevation_offset))
    });

    let range = (max_elev - min_elev).max(0.01); // Avoid division by zero

    for (x, y, tile) in local_map.iter() {
        // Normalize elevation to 0.0 - 1.0
        let t = ((tile.elevation_offset - min_elev) / range).clamp(0.0, 1.0);

        // Blue (low) → Green (mid-low) → Brown (mid-high) → White (high)
        let color = if t < 0.25 {
            // Blue to cyan (low areas, water-adjacent)
            let s = t / 0.25;
            Rgb([
                (40.0 + s * 40.0) as u8,
                (60.0 + s * 80.0) as u8,
                (140.0 + s * 40.0) as u8,
            ])
        } else if t < 0.5 {
            // Cyan to green (low-mid areas)
            let s = (t - 0.25) / 0.25;
            Rgb([
                (80.0 - s * 40.0) as u8,
                (140.0 + s * 40.0) as u8,
                (180.0 - s * 100.0) as u8,
            ])
        } else if t < 0.75 {
            // Green to brown (mid-high areas)
            let s = (t - 0.5) / 0.25;
            Rgb([
                (40.0 + s * 100.0) as u8,
                (180.0 - s * 80.0) as u8,
                (80.0 - s * 40.0) as u8,
            ])
        } else {
            // Brown to white (high areas, peaks)
            let s = (t - 0.75) / 0.25;
            Rgb([
                (140.0 + s * 115.0) as u8,
                (100.0 + s * 155.0) as u8,
                (40.0 + s * 215.0) as u8,
            ])
        };

        img.put_pixel(x as u32, y as u32, color);
    }

    img.save(path)
}

/// Export a blend zone map showing biome transition zones
pub fn export_blend_zone_map(
    local_map: &LocalMap,
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(local_map.width as u32, local_map.height as u32);

    let size = local_map.width;
    let blend_width = 12.0_f32;

    for (x, y, tile) in local_map.iter() {
        // Calculate blend factors for each edge
        let dist_north = y as f32;
        let dist_south = (size - 1 - y) as f32;
        let dist_west = x as f32;
        let dist_east = (size - 1 - x) as f32;

        let blend_north = if dist_north < blend_width { 1.0 - dist_north / blend_width } else { 0.0 };
        let blend_south = if dist_south < blend_width { 1.0 - dist_south / blend_width } else { 0.0 };
        let blend_west = if dist_west < blend_width { 1.0 - dist_west / blend_width } else { 0.0 };
        let blend_east = if dist_east < blend_width { 1.0 - dist_east / blend_width } else { 0.0 };

        let max_blend = blend_north.max(blend_south).max(blend_west).max(blend_east);

        // Base terrain color
        let (tr, tg, tb) = tile.terrain.color();

        if max_blend < 0.05 {
            // Pure center - normal terrain color
            img.put_pixel(x as u32, y as u32, Rgb([tr, tg, tb]));
        } else {
            // Blend zone - tint based on direction
            // Red = north, Green = south, Blue = west, Yellow = east
            let r = tr as f32 * (1.0 - max_blend * 0.3) + 255.0 * blend_north * 0.5 + 200.0 * blend_east * 0.3;
            let g = tg as f32 * (1.0 - max_blend * 0.3) + 255.0 * blend_south * 0.5 + 200.0 * blend_east * 0.3;
            let b = tb as f32 * (1.0 - max_blend * 0.3) + 255.0 * blend_west * 0.5;

            img.put_pixel(
                x as u32,
                y as u32,
                Rgb([r.min(255.0) as u8, g.min(255.0) as u8, b.min(255.0) as u8]),
            );
        }
    }

    img.save(path)
}

/// Export a shaded local map with terrain, features, and elevation shading
pub fn export_local_map_shaded(
    local_map: &LocalMap,
    path: &str,
) -> Result<(), image::ImageError> {
    let mut img: RgbImage = ImageBuffer::new(local_map.width as u32, local_map.height as u32);

    for (x, y, tile) in local_map.iter() {
        // Get elevation brightness (0.7 to 1.3)
        let brightness = tile.elevation_brightness();

        // Get base color (feature overrides terrain)
        let (r, g, b) = tile.color();

        // Apply elevation shading
        let shaded_r = ((r as f32 * brightness).min(255.0)) as u8;
        let shaded_g = ((g as f32 * brightness).min(255.0)) as u8;
        let shaded_b = ((b as f32 * brightness).min(255.0)) as u8;

        img.put_pixel(x as u32, y as u32, Rgb([shaded_r, shaded_g, shaded_b]));
    }

    img.save(path)
}

/// Export a local map with scaled tiles showing terrain + features separately
pub fn export_local_map_layered(
    local_map: &LocalMap,
    path: &str,
    scale: u32,
) -> Result<(), image::ImageError> {
    let width = local_map.width as u32 * scale;
    let height = local_map.height as u32 * scale;
    let mut img: RgbImage = ImageBuffer::new(width, height);

    for (x, y, tile) in local_map.iter() {
        let brightness = tile.elevation_brightness();

        // Get terrain color with elevation shading
        let (tr, tg, tb) = tile.terrain.color();
        let base_r = ((tr as f32 * brightness).min(255.0)) as u8;
        let base_g = ((tg as f32 * brightness).min(255.0)) as u8;
        let base_b = ((tb as f32 * brightness).min(255.0)) as u8;

        // Fill base terrain for entire tile
        for dy in 0..scale {
            for dx in 0..scale {
                let px = x as u32 * scale + dx;
                let py = y as u32 * scale + dy;
                img.put_pixel(px, py, Rgb([base_r, base_g, base_b]));
            }
        }

        // If there's a feature, draw it in the center with its own shading
        if let Some(feature) = tile.feature {
            let (fr, fg, fb) = feature.color();
            let feat_r = ((fr as f32 * brightness * 1.1).min(255.0)) as u8;
            let feat_g = ((fg as f32 * brightness * 1.1).min(255.0)) as u8;
            let feat_b = ((fb as f32 * brightness * 1.1).min(255.0)) as u8;

            // Draw feature in center portion (60% of tile)
            let margin = scale * 2 / 10;
            let feature_size = scale - margin * 2;

            if feature_size > 0 {
                for dy in margin..(margin + feature_size) {
                    for dx in margin..(margin + feature_size) {
                        let px = x as u32 * scale + dx;
                        let py = y as u32 * scale + dy;
                        img.put_pixel(px, py, Rgb([feat_r, feat_g, feat_b]));
                    }
                }
            }
        }
    }

    img.save(path)
}
