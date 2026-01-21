//! Organic and geometric shape generation
//!
//! Provides functions for generating circles, ellipses, irregular polygons,
//! and other non-rectangular shapes for more natural-looking structures.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use noise::{NoiseFn, Perlin};
use std::f32::consts::PI;

/// Generate points forming a filled circle
pub fn filled_circle(center_x: i32, center_y: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                points.push((center_x + dx, center_y + dy));
            }
        }
    }

    points
}

/// Generate points forming a circle outline (walls)
pub fn circle_outline(center_x: i32, center_y: i32, radius: i32, thickness: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let inner_radius = (radius - thickness).max(0);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius * radius && dist_sq >= inner_radius * inner_radius {
                points.push((center_x + dx, center_y + dy));
            }
        }
    }

    points
}

/// Generate points forming the interior of a circle (floor)
pub fn circle_interior(center_x: i32, center_y: i32, radius: i32, wall_thickness: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let inner_radius = (radius - wall_thickness).max(1);

    for dy in -inner_radius..=inner_radius {
        for dx in -inner_radius..=inner_radius {
            if dx * dx + dy * dy < inner_radius * inner_radius {
                points.push((center_x + dx, center_y + dy));
            }
        }
    }

    points
}

/// Generate an irregular/wobbly circle using noise
pub fn irregular_circle(
    center_x: i32,
    center_y: i32,
    base_radius: i32,
    variation: f32,  // 0.0 to 1.0, how much the radius varies
    seed: u32,
) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let noise = Perlin::new(seed);

    // Sample angles to determine boundary
    let num_samples = (base_radius * 8).max(32) as usize;
    let mut boundary_points: Vec<(f32, f32)> = Vec::new();

    for i in 0..num_samples {
        let angle = (i as f32 / num_samples as f32) * 2.0 * PI;

        // Use noise to vary the radius
        let noise_val = noise.get([angle.cos() as f64 * 2.0, angle.sin() as f64 * 2.0]) as f32;
        let radius_mult = 1.0 + noise_val * variation;
        let r = base_radius as f32 * radius_mult;

        let px = center_x as f32 + angle.cos() * r;
        let py = center_y as f32 + angle.sin() * r;
        boundary_points.push((px, py));
    }

    // Fill the shape using point-in-polygon for each potential point
    let max_r = (base_radius as f32 * (1.0 + variation)).ceil() as i32 + 1;

    for dy in -max_r..=max_r {
        for dx in -max_r..=max_r {
            let px = center_x + dx;
            let py = center_y + dy;

            if point_in_polygon(px as f32, py as f32, &boundary_points) {
                points.push((px, py));
            }
        }
    }

    points
}

/// Check if a point is inside a polygon using ray casting
fn point_in_polygon(x: f32, y: f32, polygon: &[(f32, f32)]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];

        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }

        j = i;
    }

    inside
}

/// Generate an ellipse
pub fn filled_ellipse(center_x: i32, center_y: i32, radius_x: i32, radius_y: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    for dy in -radius_y..=radius_y {
        for dx in -radius_x..=radius_x {
            let nx = dx as f32 / radius_x as f32;
            let ny = dy as f32 / radius_y as f32;
            if nx * nx + ny * ny <= 1.0 {
                points.push((center_x + dx, center_y + dy));
            }
        }
    }

    points
}

/// Generate an L-shaped room
pub fn l_shape(
    x: i32,
    y: i32,
    width1: i32,
    height1: i32,
    width2: i32,
    height2: i32,
    corner: u8,  // 0=TL, 1=TR, 2=BL, 3=BR
) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    match corner {
        0 => {
            // L with corner at top-left: horizontal part at top, vertical part at left
            for dy in 0..height2 {
                for dx in 0..width1 {
                    if dy < height1 || dx < width2 {
                        points.push((x + dx, y + dy));
                    }
                }
            }
        }
        1 => {
            // L with corner at top-right
            for dy in 0..height2 {
                for dx in 0..width1 {
                    if dy < height1 || dx >= width1 - width2 {
                        points.push((x + dx, y + dy));
                    }
                }
            }
        }
        2 => {
            // L with corner at bottom-left
            for dy in 0..height2 {
                for dx in 0..width1 {
                    if dy >= height2 - height1 || dx < width2 {
                        points.push((x + dx, y + dy));
                    }
                }
            }
        }
        _ => {
            // L with corner at bottom-right
            for dy in 0..height2 {
                for dx in 0..width1 {
                    if dy >= height2 - height1 || dx >= width1 - width2 {
                        points.push((x + dx, y + dy));
                    }
                }
            }
        }
    }

    points
}

/// Generate a rounded rectangle
pub fn rounded_rectangle(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    corner_radius: i32,
) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let r = corner_radius.min(width / 2).min(height / 2);

    for dy in 0..height {
        for dx in 0..width {
            let px = x + dx;
            let py = y + dy;

            // Check if in corner regions
            let in_tl_corner = dx < r && dy < r;
            let in_tr_corner = dx >= width - r && dy < r;
            let in_bl_corner = dx < r && dy >= height - r;
            let in_br_corner = dx >= width - r && dy >= height - r;

            if in_tl_corner {
                let cdx = dx - r;
                let cdy = dy - r;
                if cdx * cdx + cdy * cdy <= r * r {
                    points.push((px, py));
                }
            } else if in_tr_corner {
                let cdx = dx - (width - r - 1);
                let cdy = dy - r;
                if cdx * cdx + cdy * cdy <= r * r {
                    points.push((px, py));
                }
            } else if in_bl_corner {
                let cdx = dx - r;
                let cdy = dy - (height - r - 1);
                if cdx * cdx + cdy * cdy <= r * r {
                    points.push((px, py));
                }
            } else if in_br_corner {
                let cdx = dx - (width - r - 1);
                let cdy = dy - (height - r - 1);
                if cdx * cdx + cdy * cdy <= r * r {
                    points.push((px, py));
                }
            } else {
                points.push((px, py));
            }
        }
    }

    points
}

/// Extract the outline (edge) points from a set of filled points
pub fn extract_outline(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    use std::collections::HashSet;

    let point_set: HashSet<(i32, i32)> = points.iter().copied().collect();
    let mut outline = Vec::new();

    for &(x, y) in points {
        // Check if any neighbor is not in the set
        let mut is_edge = false;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if !point_set.contains(&(x + dx, y + dy)) {
                is_edge = true;
                break;
            }
        }

        if is_edge {
            outline.push((x, y));
        }
    }

    outline
}

/// Extract interior points (not on edge)
pub fn extract_interior(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    use std::collections::HashSet;

    let point_set: HashSet<(i32, i32)> = points.iter().copied().collect();
    let mut interior = Vec::new();

    for &(x, y) in points {
        // Check if all neighbors are in the set
        let mut is_interior = true;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if !point_set.contains(&(x + dx, y + dy)) {
                is_interior = false;
                break;
            }
        }

        if is_interior {
            interior.push((x, y));
        }
    }

    interior
}

/// Generate a star/polygon shape
pub fn star_polygon(
    center_x: i32,
    center_y: i32,
    outer_radius: i32,
    inner_radius: i32,
    points_count: usize,
) -> Vec<(i32, i32)> {
    let mut boundary = Vec::new();
    let total_points = points_count * 2;

    for i in 0..total_points {
        let angle = (i as f32 / total_points as f32) * 2.0 * PI - PI / 2.0;
        let radius = if i % 2 == 0 { outer_radius } else { inner_radius };

        let px = center_x as f32 + angle.cos() * radius as f32;
        let py = center_y as f32 + angle.sin() * radius as f32;
        boundary.push((px, py));
    }

    // Fill the star
    let mut points = Vec::new();
    let max_r = outer_radius + 1;

    for dy in -max_r..=max_r {
        for dx in -max_r..=max_r {
            let px = center_x + dx;
            let py = center_y + dy;

            if point_in_polygon(px as f32, py as f32, &boundary) {
                points.push((px, py));
            }
        }
    }

    points
}

/// Generate organic blob shape using multiple overlapping circles
pub fn organic_blob(
    center_x: i32,
    center_y: i32,
    base_radius: i32,
    rng: &mut ChaCha8Rng,
) -> Vec<(i32, i32)> {
    use std::collections::HashSet;

    let mut all_points: HashSet<(i32, i32)> = HashSet::new();

    // Main center circle
    for p in filled_circle(center_x, center_y, base_radius) {
        all_points.insert(p);
    }

    // Add 3-6 overlapping circles
    let num_blobs = rng.gen_range(3..=6);
    for _ in 0..num_blobs {
        let angle = rng.gen_range(0.0..2.0 * PI);
        let dist = rng.gen_range(base_radius as f32 * 0.3..base_radius as f32 * 0.7);
        let blob_radius = rng.gen_range(base_radius / 3..=base_radius * 2 / 3);

        let bx = center_x + (angle.cos() * dist) as i32;
        let by = center_y + (angle.sin() * dist) as i32;

        for p in filled_circle(bx, by, blob_radius) {
            all_points.insert(p);
        }
    }

    all_points.into_iter().collect()
}

/// Generate a crescent/arc shape
pub fn crescent(
    center_x: i32,
    center_y: i32,
    outer_radius: i32,
    inner_radius: i32,
    start_angle: f32,  // in radians
    end_angle: f32,
) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    for dy in -outer_radius..=outer_radius {
        for dx in -outer_radius..=outer_radius {
            let dist_sq = dx * dx + dy * dy;

            // Check if in the ring
            if dist_sq <= outer_radius * outer_radius && dist_sq >= inner_radius * inner_radius {
                // Check angle
                let angle = (dy as f32).atan2(dx as f32);
                let mut a = angle;
                if a < 0.0 {
                    a += 2.0 * PI;
                }

                let mut s = start_angle;
                let mut e = end_angle;
                if s < 0.0 { s += 2.0 * PI; }
                if e < 0.0 { e += 2.0 * PI; }

                let in_arc = if s <= e {
                    a >= s && a <= e
                } else {
                    a >= s || a <= e
                };

                if in_arc {
                    points.push((center_x + dx, center_y + dy));
                }
            }
        }
    }

    points
}
