use noise::{NoiseFn, Perlin, Seedable};

use crate::tilemap::Tilemap;

use super::types::{Plate, PlateId};

/// Calculate the tectonic stress at each cell based on plate velocities.
/// Stress width is proportional to relative velocity of converging plates.
pub fn calculate_stress(plate_map: &Tilemap<PlateId>, plates: &[Plate]) -> Tilemap<f32> {
    use std::collections::HashMap;

    let width = plate_map.width;
    let height = plate_map.height;

    // Calculate stress and spread width between each pair of continental plates
    // (stress_value, spread_radius)
    let mut plate_pair_info: HashMap<(u8, u8), (f32, usize)> = HashMap::new();

    for (i, plate_a) in plates.iter().enumerate() {
        for (j, plate_b) in plates.iter().enumerate() {
            if i >= j {
                continue;
            }

            // Calculate stress for all plate boundaries

            // Relative velocity
            let rel_vx = plate_a.velocity.x - plate_b.velocity.x;
            let rel_vy = plate_a.velocity.y - plate_b.velocity.y;
            let rel_speed = (rel_vx * rel_vx + rel_vy * rel_vy).sqrt();

            // Use the angle between velocities to determine convergent/divergent
            let dot = plate_a.velocity.x * plate_b.velocity.x + plate_a.velocity.y * plate_b.velocity.y;
            let mag_a = plate_a.velocity.length();
            let mag_b = plate_b.velocity.length();

            let stress = if mag_a > 0.01 && mag_b > 0.01 {
                let cos_angle = dot / (mag_a * mag_b);
                -cos_angle * rel_speed
            } else {
                rel_speed * 0.5
            };

            // Spread radius based on relative velocity (faster = wider mountains)
            // Scale: velocity 1.0 -> ~8 pixels, velocity 3.0 -> ~24 pixels
            let base_spread = (width / 64).max(4);
            let spread_radius = (base_spread as f32 * rel_speed * 0.8) as usize;
            let spread_radius = spread_radius.clamp(2, width / 16);

            plate_pair_info.insert((i as u8, j as u8), (stress, spread_radius));
        }
    }

    // First pass: mark boundary cells with their stress and spread info
    let mut boundary_cells: Vec<(usize, usize, f32, usize)> = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let my_plate_id = *plate_map.get(x, y);
            if my_plate_id.is_none() {
                continue;
            }

            for (nx, ny) in plate_map.neighbors(x, y) {
                let neighbor_plate_id = *plate_map.get(nx, ny);
                if neighbor_plate_id.is_none() || neighbor_plate_id == my_plate_id {
                    continue;
                }

                let (id_a, id_b) = if my_plate_id.0 < neighbor_plate_id.0 {
                    (my_plate_id.0, neighbor_plate_id.0)
                } else {
                    (neighbor_plate_id.0, my_plate_id.0)
                };

                if let Some(&(stress, spread)) = plate_pair_info.get(&(id_a, id_b)) {
                    // Include both convergent (positive) and divergent (negative) stress
                    if stress.abs() > 0.01 {
                        boundary_cells.push((x, y, stress, spread));
                        break; // Only add once per cell
                    }
                }
            }
        }
    }

    // Second pass: spread stress from boundary cells based on velocity
    let mut stress_map = Tilemap::new_with(width, height, 0.0f32);

    // High frequency noise for texture
    let noise = Perlin::new(1).set_seed(42);

    for (bx, by, stress, spread) in boundary_cells {
        // Add random variation to spread based on position
        // Simple hash from position for deterministic randomness
        let hash = ((bx * 73856093) ^ (by * 19349663)) as f32;
        let variation = ((hash % 1000.0) / 1000.0) * 0.6 + 0.7; // Range: 0.7 to 1.3
        let spread = ((spread as f32) * variation) as usize;
        let spread = spread.max(2);

        // Apply stress in a radius around the boundary cell
        let spread_i = spread as i32;

        for dy in -spread_i..=spread_i {
            for dx in -spread_i..=spread_i {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist > spread as f32 {
                    continue;
                }

                let target_x = ((bx as i32 + dx).rem_euclid(width as i32)) as usize;
                let target_y = (by as i32 + dy).clamp(0, height as i32 - 1) as usize;

                // Gaussian falloff for sharp peak, smooth descent to ocean level
                // exp(-k * t^2) gives a bell curve shape
                let t = dist / spread as f32;
                let k = 3.0; // Controls steepness - higher = sharper peak
                let falloff = (-k * t * t).exp();

                // High frequency noise for texture (multiply to modulate stress)
                let nx_val = target_x as f64 / width as f64;
                let ny_val = target_y as f64 / height as f64;
                let noise_val = noise.get([nx_val * 40.0, ny_val * 40.0]) as f32;
                let noise_factor = 0.7 + (noise_val + 1.0) * 0.3; // Range: 0.7 to 1.3

                let cell_stress = stress * falloff * noise_factor;

                // Keep maximum magnitude stress at each cell
                // Positive stress = convergent (mountains), negative = divergent (rifts)
                let current = *stress_map.get(target_x, target_y);
                if (stress > 0.0 && cell_stress > current) ||
                   (stress < 0.0 && cell_stress < current) {
                    stress_map.set(target_x, target_y, cell_stress);
                }
            }
        }
    }

    stress_map
}

/// Smoothstep interpolation for gentle falloff
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Enhance stress with multiple blur passes to create continuous falloff to zero.
/// Creates foothills that smoothly merge with sea level.
/// Adds sin wave wiggle to make mountain ranges less straight.
pub fn enhance_stress(
    stress_map: &Tilemap<f32>,
    passes: usize,
) -> Tilemap<f32> {
    let width = stress_map.width;
    let height = stress_map.height;

    // First, add sin wave displacement to the stress
    let wiggled = add_wiggle(stress_map);

    // Start with wiggled stress (the peaks)
    let mut result = wiggled.clone();

    // Add multiple blur passes at increasing radii
    // Each pass creates a wider, lower "skirt" around the peaks
    for i in 0..passes {
        let radius = (width / 64).max(2) * (i + 1);
        let sigma = radius as f32 * 0.7;
        let blend = 0.4 / (i + 1) as f32; // Decreasing blend for wider passes

        let blurred = smooth_stress(&wiggled, radius, sigma);

        // Add blurred version to result
        for y in 0..height {
            for x in 0..width {
                let current = *result.get(x, y);
                let blur_val = *blurred.get(x, y);

                // Only add positive stress (mountains), scaled down
                if blur_val > 0.0 {
                    result.set(x, y, current + blur_val * blend);
                }
            }
        }
    }

    // Final smoothing pass to ensure continuous gradients
    smooth_stress(&result, (width / 128).max(2), 1.5)
}

/// Add sin wave wiggle to stress map to create non-straight mountain ranges
pub fn add_wiggle(stress_map: &Tilemap<f32>) -> Tilemap<f32> {
    let width = stress_map.width;
    let height = stress_map.height;

    let mut result = Tilemap::new_with(width, height, 0.0f32);

    // Multiple sin frequencies for more natural wiggle
    let freq1 = 0.05;
    let freq2 = 0.12;
    let freq3 = 0.03;
    let amplitude = (width as f32 / 80.0).max(2.0);

    for y in 0..height {
        for x in 0..width {
            let stress = *stress_map.get(x, y);

            if stress.abs() > 0.01 {
                // Calculate displacement using multiple sin waves
                let wave1 = (y as f32 * freq1).sin() * amplitude;
                let wave2 = (y as f32 * freq2 + 1.5).sin() * amplitude * 0.5;
                let wave3 = (x as f32 * freq3).sin() * amplitude * 0.3;

                let displacement = (wave1 + wave2 + wave3) as i32;

                // Apply displacement in x direction
                let new_x = ((x as i32 + displacement).rem_euclid(width as i32)) as usize;

                // Add to displaced position (accumulate if multiple sources)
                let current = *result.get(new_x, y);
                result.set(new_x, y, current.max(stress));
            }
        }
    }

    // Light blur to smooth the displaced stress
    smooth_stress(&result, 2, 1.0)
}

/// Apply Gaussian blur to smooth the stress map.
/// This creates realistic stress falloff from plate boundaries.
pub fn smooth_stress(stress_map: &Tilemap<f32>, radius: usize, sigma: f32) -> Tilemap<f32> {
    // Generate 1D Gaussian kernel
    let kernel = generate_gaussian_kernel(radius, sigma);
    let kernel_size = kernel.len();
    let half_kernel = kernel_size / 2;

    // Separable Gaussian blur: horizontal pass
    let mut horizontal = Tilemap::new_with(stress_map.width, stress_map.height, 0.0);

    for y in 0..stress_map.height {
        for x in 0..stress_map.width {
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for (ki, &weight) in kernel.iter().enumerate() {
                let offset = ki as i32 - half_kernel as i32;
                let sx = ((x as i32 + offset).rem_euclid(stress_map.width as i32)) as usize;

                let val = *stress_map.get(sx, y);
                sum += val * weight;
                weight_sum += weight;
            }

            horizontal.set(x, y, sum / weight_sum);
        }
    }

    // Vertical pass
    let mut result = Tilemap::new_with(stress_map.width, stress_map.height, 0.0);

    for y in 0..stress_map.height {
        for x in 0..stress_map.width {
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for (ki, &weight) in kernel.iter().enumerate() {
                let offset = ki as i32 - half_kernel as i32;
                let sy = (y as i32 + offset).clamp(0, stress_map.height as i32 - 1) as usize;

                let val = *horizontal.get(x, sy);
                sum += val * weight;
                weight_sum += weight;
            }

            result.set(x, y, sum / weight_sum);
        }
    }

    result
}

/// Generate a 1D Gaussian kernel.
fn generate_gaussian_kernel(radius: usize, sigma: f32) -> Vec<f32> {
    let size = radius * 2 + 1;
    let mut kernel = Vec::with_capacity(size);

    let sigma_sq = sigma * sigma;
    let norm = 1.0 / (2.0 * std::f32::consts::PI * sigma_sq).sqrt();

    for i in 0..size {
        let x = i as f32 - radius as f32;
        let weight = norm * (-x * x / (2.0 * sigma_sq)).exp();
        kernel.push(weight);
    }

    kernel
}

/// Spread stress from boundaries into plate interiors using distance-based falloff.
/// Combines with Gaussian smoothing for natural-looking stress distribution.
pub fn spread_stress(
    stress_map: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    spread_distance: usize,
    falloff: f32,
) -> Tilemap<f32> {
    let mut result = stress_map.clone();

    // Multiple passes to spread stress inward
    for _ in 0..spread_distance {
        let current = result.clone();

        for y in 0..current.height {
            for x in 0..current.width {
                let current_val = *current.get(x, y);
                let my_plate = *plate_map.get(x, y);

                // Only spread to cells with low stress
                if current_val.abs() < 0.01 {
                    let mut max_neighbor_stress = 0.0f32;

                    for (nx, ny) in current.neighbors(x, y) {
                        // Only spread within same plate
                        if *plate_map.get(nx, ny) == my_plate {
                            let neighbor_stress = *current.get(nx, ny);
                            if neighbor_stress.abs() > max_neighbor_stress.abs() {
                                max_neighbor_stress = neighbor_stress;
                            }
                        }
                    }

                    // Apply falloff
                    if max_neighbor_stress.abs() > 0.01 {
                        result.set(x, y, max_neighbor_stress * falloff);
                    }
                }
            }
        }
    }

    result
}
