//! Utility functions for erosion calculations
//!
//! Provides gradient calculation, bilinear interpolation, and erosion brush utilities.

use crate::tilemap::Tilemap;

/// Sample height at a floating-point position using bilinear interpolation.
/// Handles horizontal wrapping (equirectangular projection).
pub fn height_at(heightmap: &Tilemap<f32>, x: f32, y: f32) -> f32 {
    let width = heightmap.width as f32;
    let height = heightmap.height as f32;

    // Handle horizontal wrapping
    let x = ((x % width) + width) % width;
    // Clamp y to valid range
    let y = y.clamp(0.0, height - 1.001);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1) % heightmap.width;  // Wrap horizontally
    let y1 = (y0 + 1).min(heightmap.height - 1);

    let fx = x.fract();
    let fy = y.fract();

    let h00 = *heightmap.get(x0, y0);
    let h10 = *heightmap.get(x1, y0);
    let h01 = *heightmap.get(x0, y1);
    let h11 = *heightmap.get(x1, y1);

    // Bilinear interpolation
    let h0 = h00 * (1.0 - fx) + h10 * fx;
    let h1 = h01 * (1.0 - fx) + h11 * fx;
    h0 * (1.0 - fy) + h1 * fy
}

/// Calculate gradient at a floating-point position using bilinear interpolation.
/// Returns (grad_x, grad_y) pointing in the direction of steepest ascent.
pub fn gradient_at(heightmap: &Tilemap<f32>, x: f32, y: f32) -> (f32, f32) {
    let width = heightmap.width as f32;
    let height = heightmap.height as f32;

    // Handle horizontal wrapping
    let x = ((x % width) + width) % width;
    // Clamp y to valid range
    let y = y.clamp(0.0, height - 1.001);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1) % heightmap.width;
    let y1 = (y0 + 1).min(heightmap.height - 1);

    let fx = x.fract();
    let fy = y.fract();

    let h00 = *heightmap.get(x0, y0);
    let h10 = *heightmap.get(x1, y0);
    let h01 = *heightmap.get(x0, y1);
    let h11 = *heightmap.get(x1, y1);

    // Gradient using bilinear interpolation
    // dh/dx at y0 and y1
    let gx0 = h10 - h00;
    let gx1 = h11 - h01;
    // Interpolate in y
    let grad_x = gx0 * (1.0 - fy) + gx1 * fy;

    // dh/dy at x0 and x1
    let gy0 = h01 - h00;
    let gy1 = h11 - h10;
    // Interpolate in x
    let grad_y = gy0 * (1.0 - fx) + gy1 * fx;

    (grad_x, grad_y)
}

/// Calculate gradient at an integer cell position using central differences.
/// Handles horizontal wrapping.
pub fn gradient_at_cell(heightmap: &Tilemap<f32>, x: usize, y: usize) -> (f32, f32) {
    let width = heightmap.width;
    let height = heightmap.height;

    // X gradient with wrapping
    let x_left = if x == 0 { width - 1 } else { x - 1 };
    let x_right = if x == width - 1 { 0 } else { x + 1 };
    let grad_x = (*heightmap.get(x_right, y) - *heightmap.get(x_left, y)) / 2.0;

    // Y gradient without wrapping (clamped at edges)
    let grad_y = if y == 0 {
        *heightmap.get(x, 1) - *heightmap.get(x, 0)
    } else if y == height - 1 {
        *heightmap.get(x, y) - *heightmap.get(x, y - 1)
    } else {
        (*heightmap.get(x, y + 1) - *heightmap.get(x, y - 1)) / 2.0
    };

    (grad_x, grad_y)
}

/// Calculate surface elevation (bedrock + ice) gradient at a cell.
pub fn surface_gradient_at_cell(
    bedrock: &Tilemap<f32>,
    ice: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> (f32, f32) {
    let width = bedrock.width;
    let height = bedrock.height;

    let surface = |x: usize, y: usize| -> f32 {
        *bedrock.get(x, y) + *ice.get(x, y)
    };

    // X gradient with wrapping
    let x_left = if x == 0 { width - 1 } else { x - 1 };
    let x_right = if x == width - 1 { 0 } else { x + 1 };
    let grad_x = (surface(x_right, y) - surface(x_left, y)) / 2.0;

    // Y gradient without wrapping
    let grad_y = if y == 0 {
        surface(x, 1) - surface(x, 0)
    } else if y == height - 1 {
        surface(x, y) - surface(x, y - 1)
    } else {
        (surface(x, y + 1) - surface(x, y - 1)) / 2.0
    };

    (grad_x, grad_y)
}

/// Create a circular erosion brush with Gaussian-like falloff.
/// Returns weights for cells within the radius, normalized to sum to 1.
pub fn create_erosion_brush(radius: usize) -> Vec<(i32, i32, f32)> {
    let mut brush = Vec::new();
    let r = radius as i32;
    let r_sq = (r * r) as f32;
    let mut total_weight = 0.0;

    for dy in -r..=r {
        for dx in -r..=r {
            let dist_sq = (dx * dx + dy * dy) as f32;
            if dist_sq <= r_sq {
                let weight = (1.0 - dist_sq / r_sq).max(0.0);
                brush.push((dx, dy, weight));
                total_weight += weight;
            }
        }
    }

    // Normalize weights
    for (_, _, w) in brush.iter_mut() {
        *w /= total_weight;
    }

    brush
}

/// Apply erosion using a brush pattern.
/// Distributes the erosion amount across cells within the brush radius.
pub fn apply_erosion_brush(
    heightmap: &mut Tilemap<f32>,
    brush: &[(i32, i32, f32)],
    x: usize,
    y: usize,
    amount: f32,
) {
    let width = heightmap.width as i32;
    let height = heightmap.height as i32;

    for &(dx, dy, weight) in brush {
        let nx = ((x as i32 + dx).rem_euclid(width)) as usize;
        let ny = (y as i32 + dy).clamp(0, height - 1) as usize;

        let current = *heightmap.get(nx, ny);
        heightmap.set(nx, ny, current - amount * weight);
    }
}

/// Apply deposition using a brush pattern.
pub fn apply_deposit_brush(
    heightmap: &mut Tilemap<f32>,
    brush: &[(i32, i32, f32)],
    x: usize,
    y: usize,
    amount: f32,
) {
    let width = heightmap.width as i32;
    let height = heightmap.height as i32;

    for &(dx, dy, weight) in brush {
        let nx = ((x as i32 + dx).rem_euclid(width)) as usize;
        let ny = (y as i32 + dy).clamp(0, height - 1) as usize;

        let current = *heightmap.get(nx, ny);
        heightmap.set(nx, ny, current + amount * weight);
    }
}

/// Calculate divergence of a 2D vector field at a cell.
/// Used for ice flux divergence in glacial erosion.
pub fn divergence_at_cell(
    flux_x: &Tilemap<f32>,
    flux_y: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> f32 {
    let width = flux_x.width;
    let height = flux_x.height;

    // dFx/dx with wrapping
    let x_left = if x == 0 { width - 1 } else { x - 1 };
    let x_right = if x == width - 1 { 0 } else { x + 1 };
    let dfx_dx = (*flux_x.get(x_right, y) - *flux_x.get(x_left, y)) / 2.0;

    // dFy/dy without wrapping
    let dfy_dy = if y == 0 {
        *flux_y.get(x, 1) - *flux_y.get(x, 0)
    } else if y == height - 1 {
        *flux_y.get(x, y) - *flux_y.get(x, y - 1)
    } else {
        (*flux_y.get(x, y + 1) - *flux_y.get(x, y - 1)) / 2.0
    };

    dfx_dx + dfy_dy
}

/// Smooth a tilemap using a simple box blur.
pub fn smooth_tilemap(map: &Tilemap<f32>, radius: usize) -> Tilemap<f32> {
    let width = map.width;
    let height = map.height;
    let mut result = Tilemap::new_with(width, height, 0.0f32);
    let r = radius as i32;

    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            let mut count = 0.0;

            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    sum += *map.get(nx, ny);
                    count += 1.0;
                }
            }

            result.set(x, y, sum / count);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_height_at_corners() {
        let mut heightmap = Tilemap::new_with(4, 4, 0.0f32);
        heightmap.set(0, 0, 1.0);
        heightmap.set(1, 0, 2.0);
        heightmap.set(0, 1, 3.0);
        heightmap.set(1, 1, 4.0);

        // Test exact corners
        assert!((height_at(&heightmap, 0.0, 0.0) - 1.0).abs() < 0.001);
        assert!((height_at(&heightmap, 1.0, 0.0) - 2.0).abs() < 0.001);

        // Test interpolated center
        let center = height_at(&heightmap, 0.5, 0.5);
        assert!((center - 2.5).abs() < 0.001);  // Average of 1,2,3,4
    }

    #[test]
    fn test_gradient_flat() {
        let heightmap = Tilemap::new_with(4, 4, 5.0f32);
        let (gx, gy) = gradient_at(&heightmap, 1.5, 1.5);
        assert!(gx.abs() < 0.001);
        assert!(gy.abs() < 0.001);
    }

    #[test]
    fn test_erosion_brush_normalized() {
        let brush = create_erosion_brush(3);
        let total: f32 = brush.iter().map(|(_, _, w)| w).sum();
        assert!((total - 1.0).abs() < 0.001);
    }
}
