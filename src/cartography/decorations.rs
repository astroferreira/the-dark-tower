//! Antique cartography embellishments: Compass Rose, Rhumb Lines, Graticule, and Vintage Borders

use std::f32::consts::PI;
use image::{ImageBuffer, Rgb, Rgba};
use crate::tilemap::Tilemap;

/// Dark iron-gall ink color for primary linework
pub const INK_PRIMARY: [u8; 3] = [38, 26, 16];
/// Faded sepia ink for secondary linework
pub const INK_SEPIA: [u8; 3] = [95, 68, 48];
/// Antique crimson ink for cardinal rhumb lines
pub const INK_CRIMSON: [u8; 4] = [145, 45, 35, 90];
/// Antique navy/indigo ink for intercardinal rhumb lines
pub const INK_NAVY: [u8; 4] = [45, 75, 105, 70];
/// Faint graticule ink
pub const INK_GRATICULE: [u8; 4] = [70, 55, 40, 50];

/// Draw an antialiased line using Bresenham / Wu with RGBA blending
pub fn draw_line_alpha(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: [u8; 4],
) {
    let w = img.width() as f32;
    let h = img.height() as f32;

    let dx = x1 - x0;
    let dy = y1 - y0;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 0.2 {
        return;
    }

    let steps = (dist * 1.5).ceil().max(1.0) as usize;
    let alpha = (color[3] as f32) / 255.0;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let px = x0 + dx * t;
        let py = y0 + dy * t;

        let ix = px.round() as i32;
        let iy = py.round() as i32;

        if ix >= 0 && ix < w as i32 && iy >= 0 && iy < h as i32 {
            let pixel = img.get_pixel_mut(ix as u32, iy as u32);
            let r = (pixel[0] as f32 * (1.0 - alpha) + color[0] as f32 * alpha).round() as u8;
            let g = (pixel[1] as f32 * (1.0 - alpha) + color[1] as f32 * alpha).round() as u8;
            let b = (pixel[2] as f32 * (1.0 - alpha) + color[2] as f32 * alpha).round() as u8;
            *pixel = Rgb([r, g, b]);
        }
    }
}

/// Find good open ocean positions for nautical rhumb centers and compass rose
pub fn find_ocean_centers(
    heightmap: &Tilemap<f32>,
    count: usize,
) -> Vec<(usize, usize)> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Search on a coarse grid for points furthest from any land
    let step = 16;
    let mut candidates: Vec<((usize, usize), f32)> = Vec::new();

    for y in (step * 2..height - step * 2).step_by(step) {
        for x in (step * 2..width - step * 2).step_by(step) {
            let h = *heightmap.get(x, y);
            if h < -50.0 {
                // Approximate ocean clearance by sampling a small radius
                let mut ocean_score = 0.0f32;
                let mut is_pure_ocean = true;
                for dy in -3..=3 {
                    for dx in -3..=3 {
                        let nx = (x as i32 + dx * step as i32).rem_euclid(width as i32) as usize;
                        let ny = (y as i32 + dy * step as i32).clamp(0, height as i32 - 1) as usize;
                        let nh = *heightmap.get(nx, ny);
                        if nh >= 0.0 {
                            is_pure_ocean = false;
                            break;
                        }
                        ocean_score -= nh;
                    }
                    if !is_pure_ocean {
                        break;
                    }
                }

                if is_pure_ocean {
                    candidates.push(((x, y), ocean_score));
                }
            }
        }
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut selected: Vec<(usize, usize)> = Vec::new();
    let min_dist_sq = ((width.min(height) / (count + 1)) as f32).powi(2);

    for (pos, _) in candidates {
        let is_far_enough = selected.iter().all(|&other| {
            let mut dx = (pos.0 as f32 - other.0 as f32).abs();
            if dx > width as f32 * 0.5 {
                dx = width as f32 - dx;
            }
            let dy = pos.1 as f32 - other.1 as f32;
            (dx * dx + dy * dy) >= min_dist_sq
        });

        if is_far_enough {
            selected.push(pos);
            if selected.len() >= count {
                break;
            }
        }
    }

    // Default fallback if no deep ocean candidates found
    if selected.is_empty() {
        selected.push((width / 4, height / 3));
        selected.push((3 * width / 4, 2 * height / 3));
    }

    selected
}

/// Render 16-point nautical rhumb lines radiating across the oceans
pub fn render_rhumb_lines(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    centers: &[(usize, usize)],
    border_margin: usize,
) {
    let w = img.width() as f32;
    let h = img.height() as f32;
    let margin = border_margin as f32;

    let max_len = (w * w + h * h).sqrt();

    for &center in centers {
        let cx = center.0 as f32;
        let cy = center.1 as f32;

        // 16 rays: 0, 22.5, 45, 67.5, 90, ...
        for ray in 0..16 {
            let angle = ray as f32 * (2.0 * PI / 16.0);
            let ex = cx + angle.cos() * max_len;
            let ey = cy + angle.sin() * max_len;

            // Cardinal rays (N, S, E, W) in crimson
            // Intercardinal (NE, NW, SE, SW) in navy
            // Minor rays in faint sepia
            let color = match ray % 4 {
                0 => INK_CRIMSON,
                2 => INK_NAVY,
                _ => [INK_SEPIA[0], INK_SEPIA[1], INK_SEPIA[2], 40],
            };

            // Clip line to map margin safely
            let min_x = margin.min(w * 0.49);
            let max_x = (w - margin).max(min_x);
            let min_y = margin.min(h * 0.49);
            let max_y = (h - margin).max(min_y);

            let x0 = cx.clamp(min_x, max_x);
            let y0 = cy.clamp(min_y, max_y);
            let x1 = ex.clamp(min_x, max_x);
            let y1 = ey.clamp(min_y, max_y);

            draw_line_alpha(img, x0, y0, x1, y1, color);
        }
    }
}

/// Render latitude and longitude graticule lines with antique styling
pub fn render_graticule(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    border_margin: usize,
) {
    let w = img.width() as f32;
    let h = img.height() as f32;
    let margin = (border_margin as f32).min(w * 0.45).min(h * 0.45);

    // Equator (y = h / 2)
    let equator_y = h * 0.5;
    draw_line_alpha(img, margin, equator_y, w - margin, equator_y, [INK_PRIMARY[0], INK_PRIMARY[1], INK_PRIMARY[2], 90]);

    // Tropics (+- 23.5 deg latitude -> y = 0.5 +- 23.5/180)
    let tropic_offset = (23.5 / 180.0) * h;
    if equator_y - tropic_offset > margin {
        draw_line_alpha(img, margin, equator_y - tropic_offset, w - margin, equator_y - tropic_offset, INK_GRATICULE);
    }
    if equator_y + tropic_offset < h - margin {
        draw_line_alpha(img, margin, equator_y + tropic_offset, w - margin, equator_y + tropic_offset, INK_GRATICULE);
    }

    // Arctic / Antarctic circles (+- 66.5 deg latitude)
    let polar_offset = (66.5 / 180.0) * h;
    if equator_y - polar_offset > margin {
        draw_line_alpha(img, margin, equator_y - polar_offset, w - margin, equator_y - polar_offset, INK_GRATICULE);
    }
    if equator_y + polar_offset < h - margin {
        draw_line_alpha(img, margin, equator_y + polar_offset, w - margin, equator_y + polar_offset, INK_GRATICULE);
    }

    // Longitude meridians every 30 degrees (12 meridians)
    for i in 1..12 {
        let x = margin + (w - 2.0 * margin) * (i as f32 / 12.0);
        draw_line_alpha(img, x, margin, x, h - margin, INK_GRATICULE);
    }
}

/// Render an ornate antique 16-point Compass Rose at a designated ocean center
pub fn render_compass_rose(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    center: (usize, usize),
    radius: f32,
) {
    let cx = center.0 as f32;
    let cy = center.1 as f32;

    let r_outer = radius;
    let r_inner = radius * 0.45;
    let r_core = radius * 0.15;

    // Draw circular rings
    let ring_steps = 180;
    for ring_r in [r_outer * 1.08, r_outer * 0.95, r_inner * 1.05, r_core] {
        for i in 0..ring_steps {
            let a0 = i as f32 * (2.0 * PI / ring_steps as f32);
            let a1 = (i + 1) as f32 * (2.0 * PI / ring_steps as f32);
            draw_line_alpha(
                img,
                cx + a0.cos() * ring_r,
                cy + a0.sin() * ring_r,
                cx + a1.cos() * ring_r,
                cy + a1.sin() * ring_r,
                [INK_PRIMARY[0], INK_PRIMARY[1], INK_PRIMARY[2], 180],
            );
        }
    }

    // 16-Point Compass Star:
    // 4 Cardinal points (Large, length = r_outer)
    // 4 Intercardinal points (Medium, length = r_outer * 0.75)
    // 8 Minor points (Small, length = r_inner * 0.9)
    for i in 0..16 {
        let angle = i as f32 * (2.0 * PI / 16.0) - PI * 0.5; // 0 = North
        let point_len = match i % 4 {
            0 => r_outer,
            2 => r_outer * 0.75,
            _ => r_inner * 0.9,
        };

        let tip_x = cx + angle.cos() * point_len;
        let tip_y = cy + angle.sin() * point_len;

        let half_angle = PI / 16.0;
        let base_l_x = cx + (angle - half_angle).cos() * r_core;
        let base_l_y = cy + (angle - half_angle).sin() * r_core;
        let base_r_x = cx + (angle + half_angle).cos() * r_core;
        let base_r_y = cy + (angle + half_angle).sin() * r_core;

        // Draw left and right wing of star point
        // One half dark ink, one half parchment/light ink for 3D engraved look
        let ink_dark = [INK_PRIMARY[0], INK_PRIMARY[1], INK_PRIMARY[2], 220];
        let ink_light = [INK_SEPIA[0], INK_SEPIA[1], INK_SEPIA[2], 120];

        draw_line_alpha(img, cx, cy, tip_x, tip_y, ink_dark);
        draw_line_alpha(img, base_l_x, base_l_y, tip_x, tip_y, ink_dark);
        draw_line_alpha(img, base_r_x, base_r_y, tip_x, tip_y, ink_light);

        // Fill triangular halves with subtle lines
        for step in 1..=5 {
            let t = step as f32 / 6.0;
            let mid_l_x = cx + (base_l_x - cx) * t;
            let mid_l_y = cy + (base_l_y - cy) * t;
            draw_line_alpha(img, mid_l_x, mid_l_y, tip_x, tip_y, [ink_dark[0], ink_dark[1], ink_dark[2], 70]);
        }
    }

    // North Fleur-de-lis accent pointer
    let north_tip_y = cy - r_outer * 1.25;
    draw_line_alpha(img, cx, cy - r_outer * 0.8, cx, north_tip_y, [180, 40, 30, 240]);
    draw_line_alpha(img, cx - 4.0, cy - r_outer * 1.05, cx, north_tip_y, [180, 40, 30, 240]);
    draw_line_alpha(img, cx + 4.0, cy - r_outer * 1.05, cx, north_tip_y, [180, 40, 30, 240]);
}

/// Render a classic 17th-century double-line ruler border around the map
pub fn render_vintage_border(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    border_width: usize,
) {
    let w = img.width() as usize;
    let h = img.height() as usize;
    if w < 16 || h < 16 {
        return;
    }
    let max_bw = (w.min(h) / 4).max(4);
    let bw = border_width.clamp(4, max_bw);

    let ink_dark = Rgb([INK_PRIMARY[0], INK_PRIMARY[1], INK_PRIMARY[2]]);
    let parchment_light = Rgb([238, 226, 202]);
    let parchment_dark = Rgb([200, 182, 148]);

    // Fill the outer border with vintage aged ruler pattern
    for y in 0..h {
        for x in 0..w {
            let in_outer_frame = x < bw || x >= w - bw || y < bw || y >= h - bw;
            if in_outer_frame {
                // Outermost solid ink edge
                if x == 0 || x == w - 1 || y == 0 || y == h - 1 ||
                   x == 2 || x == w - 3 || y == 2 || y == h - 3 {
                    img.put_pixel(x as u32, y as u32, ink_dark);
                } else if x == bw - 1 || x == w - bw || y == bw - 1 || y == h - bw {
                    // Innermost framing border
                    img.put_pixel(x as u32, y as u32, ink_dark);
                } else {
                    // Alternating black and parchment degree ruler blocks
                    let segment_size = 24;
                    let is_segment_black = if y < bw || y >= h - bw {
                        (x / segment_size) % 2 == 0
                    } else {
                        (y / segment_size) % 2 == 0
                    };

                    // Corner block detection
                    let is_corner = (x < bw && y < bw) || (x >= w - bw && y < bw) ||
                                    (x < bw && y >= h - bw) || (x >= w - bw && y >= h - bw);

                    if is_corner {
                        // Decorative solid rosette corner
                        if (x == bw / 2 || y == bw / 2) || ((x + y) % 4 == 0) {
                            img.put_pixel(x as u32, y as u32, ink_dark);
                        } else {
                            img.put_pixel(x as u32, y as u32, parchment_dark);
                        }
                    } else if is_segment_black {
                        img.put_pixel(x as u32, y as u32, ink_dark);
                    } else {
                        img.put_pixel(x as u32, y as u32, parchment_light);
                    }
                }
            }
        }
    }
}
