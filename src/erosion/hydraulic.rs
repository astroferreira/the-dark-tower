//! Hydraulic erosion simulation using particle-based water droplets.
//!
//! Simulates water flow carving river valleys and depositing sediment.
//! Each droplet follows the terrain gradient, picking up sediment on steep
//! slopes and depositing it when the flow slows down.
//!
//! Key improvements for realistic river formation:
//! - Droplets spawn preferentially at high elevations (rain on mountains)
//! - Higher inertia maintains flow direction for continuous channels
//! - Smaller erosion radius creates narrow river beds
//! - Droplets terminate at sea level (river mouths)
//!
//! Parallelization: Uses rayon for multi-threaded droplet simulation.

use crate::erosion::params::ErosionParams;
use crate::erosion::utils::{create_erosion_brush, gradient_at, height_at};
use crate::erosion::ErosionStats;
use crate::tilemap::Tilemap;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// A water droplet for hydraulic erosion simulation
struct WaterDroplet {
    /// Position (floating point for interpolation)
    x: f32,
    y: f32,
    /// Movement direction (normalized)
    dir_x: f32,
    dir_y: f32,
    /// Current speed
    velocity: f32,
    /// Water volume
    water: f32,
    /// Carried sediment
    sediment: f32,
}

impl WaterDroplet {
    fn new(x: f32, y: f32, initial_water: f32, initial_velocity: f32) -> Self {
        Self {
            x,
            y,
            dir_x: 0.0,
            dir_y: 0.0,
            velocity: initial_velocity,
            water: initial_water,
            sediment: 0.0,
        }
    }
}

/// Run hydraulic erosion simulation.
///
/// Algorithm:
/// 1. Spawn droplet at high elevation (preferentially on land/mountains)
/// 2. While droplet has water and hasn't exceeded max steps:
///    a. Calculate terrain gradient at current position
///    b. Update direction using inertia + gradient (high inertia = continuous paths)
///    c. Move droplet in direction
///    d. Calculate height difference (old_height - new_height)
///    e. Calculate sediment capacity based on speed and water volume
///    f. If carrying more than capacity: deposit excess sediment
///    g. If carrying less: erode terrain (modulated by rock hardness)
///    h. Evaporate some water
/// 3. Stop when droplet reaches sea level or dies
pub fn simulate(
    heightmap: &mut Tilemap<f32>,
    hardness: &Tilemap<f32>,
    params: &ErosionParams,
    rng: &mut ChaCha8Rng,
) -> ErosionStats {
    let width = heightmap.width;
    let height = heightmap.height;
    let width_f = width as f32;
    let height_f = height as f32;

    // Pre-compute erosion brush (use smaller radius for narrow channels)
    let brush = create_erosion_brush(params.droplet_erosion_radius);

    let mut stats = ErosionStats::default();
    stats.iterations = params.hydraulic_iterations;

    // Find elevation range for spawning preference
    let mut max_height: f32 = f32::MIN;
    let mut min_height: f32 = f32::MAX;
    for (_, _, &h) in heightmap.iter() {
        if h > max_height { max_height = h; }
        if h < min_height { min_height = h; }
    }
    let height_range = (max_height - min_height).max(1.0);

    // Sea level threshold (areas below this are ocean)
    let sea_level = 0.0;

    for _ in 0..params.hydraulic_iterations {
        // Spawn droplet preferentially at high elevations (mountains)
        // Use rejection sampling to bias towards higher terrain
        let (start_x, start_y) = spawn_at_high_elevation(
            heightmap,
            rng,
            width_f,
            height_f,
            min_height,
            height_range,
            sea_level,
        );

        let start_height = height_at(heightmap, start_x, start_y);

        // Skip if starting below sea level (no rivers in ocean)
        if start_height < sea_level {
            continue;
        }

        let mut droplet = WaterDroplet::new(
            start_x,
            start_y,
            params.droplet_initial_water,
            params.droplet_initial_velocity,
        );

        for _ in 0..params.droplet_max_steps {
            // Calculate gradient at current position
            let (grad_x, grad_y) = gradient_at(heightmap, droplet.x, droplet.y);

            // Update direction with inertia
            // Higher inertia = droplet maintains direction longer = continuous channels
            droplet.dir_x = droplet.dir_x * params.droplet_inertia - grad_x * (1.0 - params.droplet_inertia);
            droplet.dir_y = droplet.dir_y * params.droplet_inertia - grad_y * (1.0 - params.droplet_inertia);

            // Normalize direction
            let dir_len = (droplet.dir_x * droplet.dir_x + droplet.dir_y * droplet.dir_y).sqrt();
            if dir_len > 0.0001 {
                droplet.dir_x /= dir_len;
                droplet.dir_y /= dir_len;
            } else {
                // No gradient (flat area or local minimum) - pick random direction
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                droplet.dir_x = angle.cos();
                droplet.dir_y = angle.sin();
            }

            // Move droplet
            let old_x = droplet.x;
            let old_y = droplet.y;
            let old_height = height_at(heightmap, old_x, old_y);

            droplet.x += droplet.dir_x;
            droplet.y += droplet.dir_y;

            // Handle horizontal wrapping (equirectangular projection)
            droplet.x = ((droplet.x % width_f) + width_f) % width_f;

            // Stop if we hit the top or bottom edge of the map
            if droplet.y < 0.0 || droplet.y >= height_f - 1.0 {
                break;
            }

            let new_height = height_at(heightmap, droplet.x, droplet.y);
            let delta_height = new_height - old_height;

            // Sanity check: if heights are corrupted, terminate droplet
            if !old_height.is_finite() || !new_height.is_finite() || delta_height.abs() > 10000.0 {
                break;
            }

            // Stop if we've reached sea level (river mouth)
            if new_height < sea_level {
                // Deposit some sediment at river mouth (delta formation)
                // Cap to prevent excessive coastal buildup
                let cell_x = old_x as usize % width;
                let cell_y = (old_y as usize).min(height - 1);
                let final_deposit = droplet.sediment.min(10.0);  // Reduced to prevent coastal buildup
                if final_deposit > 0.0 && final_deposit.is_finite() {
                    apply_deposit(&brush, heightmap, cell_x, cell_y, final_deposit, width, height);
                    stats.total_deposited += final_deposit as f64;
                }
                break;
            }

            // Calculate sediment capacity
            // Capacity depends on: slope (steeper = more), velocity, water volume
            // Clamp delta to reasonable range to prevent runaway erosion
            let slope = (-delta_height).clamp(0.0, 50.0);
            let capacity = (slope.max(params.droplet_min_volume)
                * droplet.velocity
                * droplet.water
                * params.droplet_capacity_factor)
                .clamp(0.0, 500.0);

            // Get hardness at current position
            let cell_x = old_x as usize % width;
            let cell_y = (old_y as usize).min(height - 1);
            let rock_hardness = *hardness.get(cell_x, cell_y);

            // Maximum erosion/deposition per step
            // Higher values create deeper, more visible channels
            const MAX_CHANGE_PER_STEP: f32 = 15.0;

            if droplet.sediment > capacity {
                // Deposit excess sediment (river slows down, drops sediment)
                let deposit_amount = ((droplet.sediment - capacity) * params.droplet_deposit_rate)
                    .min(MAX_CHANGE_PER_STEP);
                droplet.sediment -= deposit_amount;

                // Apply deposition with brush
                apply_deposit(&brush, heightmap, cell_x, cell_y, deposit_amount, width, height);

                stats.total_deposited += deposit_amount as f64;
                stats.max_deposition = stats.max_deposition.max(deposit_amount);
            } else {
                // Erode terrain (water carves into rock)
                // Harder rock is more resistant to erosion
                let hardness_factor = (1.0 - rock_hardness).max(0.1);
                let erode_amount = ((capacity - droplet.sediment)
                    * params.droplet_erosion_rate
                    * hardness_factor)
                    .min(slope)  // Don't erode more than height difference
                    .min(MAX_CHANGE_PER_STEP);

                if erode_amount > 0.0 && erode_amount.is_finite() {
                    droplet.sediment += erode_amount;

                    // Apply erosion with brush
                    apply_erosion(&brush, heightmap, cell_x, cell_y, erode_amount, width, height);

                    stats.total_eroded += erode_amount as f64;
                    stats.max_erosion = stats.max_erosion.max(erode_amount);
                }
            }

            // Update velocity based on height change (accelerate going downhill)
            droplet.velocity = (droplet.velocity * droplet.velocity + delta_height * params.droplet_gravity)
                .clamp(0.0, 10000.0)
                .sqrt()
                .min(50.0);

            // Evaporate water
            droplet.water *= 1.0 - params.droplet_evaporation;

            // Check if droplet has died (evaporated)
            if droplet.water < params.droplet_min_volume {
                // Deposit remaining sediment
                let final_deposit = droplet.sediment.min(MAX_CHANGE_PER_STEP * 3.0);
                if final_deposit > 0.0 && final_deposit.is_finite() {
                    apply_deposit(&brush, heightmap, cell_x, cell_y, final_deposit, width, height);
                    stats.total_deposited += final_deposit as f64;
                }
                break;
            }
        }
    }

    stats
}

/// Parallel hydraulic erosion simulation using rayon.
/// Processes droplets in batches for better performance on multi-core CPUs.
pub fn simulate_parallel(
    heightmap: &mut Tilemap<f32>,
    hardness: &Tilemap<f32>,
    params: &ErosionParams,
    base_seed: u64,
) -> ErosionStats {
    let width = heightmap.width;
    let height = heightmap.height;
    let width_f = width as f32;
    let height_f = height as f32;

    // Find elevation range for spawning preference
    let mut max_height: f32 = f32::MIN;
    let mut min_height: f32 = f32::MAX;
    for (_, _, &h) in heightmap.iter() {
        if h > max_height { max_height = h; }
        if h < min_height { min_height = h; }
    }
    let height_range = (max_height - min_height).max(1.0);

    // Pre-compute erosion brush
    let brush = create_erosion_brush(params.droplet_erosion_radius);

    // Process in batches - each batch runs in parallel, then we apply changes
    let batch_size = 10_000;
    let num_batches = (params.hydraulic_iterations + batch_size - 1) / batch_size;

    // Atomic counters for statistics
    let total_eroded = AtomicU64::new(0);
    let total_deposited = AtomicU64::new(0);

    // Create a delta map to accumulate changes (avoids race conditions)
    let mut delta: Vec<f32> = vec![0.0; width * height];

    for batch in 0..num_batches {
        let batch_start = batch * batch_size;
        let batch_end = (batch_start + batch_size).min(params.hydraulic_iterations);
        let batch_count = batch_end - batch_start;

        // Take a snapshot of the heightmap for this batch
        let mut heightmap_snapshot: Vec<f32> = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                heightmap_snapshot.push(*heightmap.get(x, y));
            }
        }

        // Process droplets in parallel
        let batch_deltas: Vec<(Vec<(usize, f32)>, f64, f64)> = (0..batch_count)
            .into_par_iter()
            .map(|i| {
                let droplet_seed = base_seed.wrapping_add((batch_start + i) as u64);
                let mut rng = ChaCha8Rng::seed_from_u64(droplet_seed);

                simulate_single_droplet(
                    &heightmap_snapshot,
                    hardness,
                    &brush,
                    &params,
                    &mut rng,
                    width,
                    height,
                    width_f,
                    height_f,
                    min_height,
                    height_range,
                )
            })
            .collect();

        // Apply all deltas from this batch
        for (changes, eroded, deposited) in batch_deltas {
            total_eroded.fetch_add((eroded * 1000.0) as u64, Ordering::Relaxed);
            total_deposited.fetch_add((deposited * 1000.0) as u64, Ordering::Relaxed);

            for (idx, change) in changes {
                delta[idx] += change;
            }
        }

        // Apply accumulated deltas to heightmap
        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                if delta[idx].abs() > 0.0001 {
                    let current = *heightmap.get(x, y);
                    let new_h = (current + delta[idx]).clamp(-5000.0, 2000.0);
                    heightmap.set(x, y, new_h);
                    delta[idx] = 0.0; // Reset for next batch
                }
            }
        }
    }

    ErosionStats {
        total_eroded: total_eroded.load(Ordering::Relaxed) as f64 / 1000.0,
        total_deposited: total_deposited.load(Ordering::Relaxed) as f64 / 1000.0,
        max_erosion: 0.0,
        max_deposition: 0.0,
        iterations: params.hydraulic_iterations,
        river_lengths: Vec::new(),
        steps_taken: 0,
    }
}

/// Sample height from flat heightmap array using bilinear interpolation
#[inline]
fn sample_height_flat(heightmap: &[f32], x: f32, y: f32, width: usize, height: usize) -> f32 {
    let width_f = width as f32;
    let height_f = height as f32;
    let x = ((x % width_f) + width_f) % width_f;
    let y = y.clamp(0.0, height_f - 1.001);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1) % width;
    let y1 = (y0 + 1).min(height - 1);
    let fx = x.fract();
    let fy = y.fract();
    let h00 = heightmap[y0 * width + x0];
    let h10 = heightmap[y0 * width + x1];
    let h01 = heightmap[y1 * width + x0];
    let h11 = heightmap[y1 * width + x1];
    let h0 = h00 * (1.0 - fx) + h10 * fx;
    let h1 = h01 * (1.0 - fx) + h11 * fx;
    h0 * (1.0 - fy) + h1 * fy
}

/// Sample gradient from flat heightmap array
#[inline]
fn sample_gradient_flat(heightmap: &[f32], x: f32, y: f32, width: usize, height: usize) -> (f32, f32) {
    let width_f = width as f32;
    let height_f = height as f32;
    let x = ((x % width_f) + width_f) % width_f;
    let y = y.clamp(0.0, height_f - 1.001);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1) % width;
    let y1 = (y0 + 1).min(height - 1);
    let fx = x.fract();
    let fy = y.fract();
    let h00 = heightmap[y0 * width + x0];
    let h10 = heightmap[y0 * width + x1];
    let h01 = heightmap[y1 * width + x0];
    let h11 = heightmap[y1 * width + x1];
    let gx0 = h10 - h00;
    let gx1 = h11 - h01;
    let gy0 = h01 - h00;
    let gy1 = h11 - h10;
    (gx0 * (1.0 - fy) + gx1 * fy, gy0 * (1.0 - fx) + gy1 * fx)
}

/// Simulate a single droplet and return height changes as (index, delta) pairs
fn simulate_single_droplet(
    heightmap: &[f32],
    hardness: &Tilemap<f32>,
    brush: &[(i32, i32, f32)],
    params: &ErosionParams,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
    width_f: f32,
    height_f: f32,
    min_height: f32,
    height_range: f32,
) -> (Vec<(usize, f32)>, f64, f64) {
    let mut changes: Vec<(usize, f32)> = Vec::with_capacity(params.droplet_max_steps * brush.len());
    let mut eroded = 0.0f64;
    let mut deposited = 0.0f64;

    // Spawn droplet at high elevation
    let sea_level = 0.0f32;
    let (mut x, mut y) = {
        let mut spawn_x = 0.0f32;
        let mut spawn_y = 0.0f32;
        for _ in 0..10 {
            spawn_x = rng.gen_range(0.0..width_f);
            spawn_y = rng.gen_range(0.0..height_f);
            let h = sample_height_flat(heightmap, spawn_x, spawn_y, width, height);
            if h >= sea_level {
                let norm_h = ((h - min_height) / height_range).clamp(0.0, 1.0);
                if rng.gen::<f32>() < (norm_h * norm_h).max(0.1) {
                    break;
                }
            }
        }
        (spawn_x, spawn_y)
    };

    let start_height = sample_height_flat(heightmap, x, y, width, height);
    if start_height < sea_level {
        return (changes, eroded, deposited);
    }

    let mut dir_x = 0.0f32;
    let mut dir_y = 0.0f32;
    let mut velocity = params.droplet_initial_velocity;
    let mut water = params.droplet_initial_water;
    let mut sediment = 0.0f32;

    for _ in 0..params.droplet_max_steps {
        let (grad_x, grad_y) = sample_gradient_flat(heightmap, x, y, width, height);

        // Update direction with inertia
        dir_x = dir_x * params.droplet_inertia - grad_x * (1.0 - params.droplet_inertia);
        dir_y = dir_y * params.droplet_inertia - grad_y * (1.0 - params.droplet_inertia);

        let dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if dir_len > 0.0001 {
            dir_x /= dir_len;
            dir_y /= dir_len;
        } else {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            dir_x = angle.cos();
            dir_y = angle.sin();
        }

        let old_x = x;
        let old_y = y;
        let old_height = sample_height_flat(heightmap, old_x, old_y, width, height);

        x += dir_x;
        y += dir_y;
        x = ((x % width_f) + width_f) % width_f;

        if y < 0.0 || y >= height_f - 1.0 {
            break;
        }

        let new_height = sample_height_flat(heightmap, x, y, width, height);
        let delta_height = new_height - old_height;

        if !old_height.is_finite() || !new_height.is_finite() || delta_height.abs() > 10000.0 {
            break;
        }

        if new_height < sea_level {
            // Deposit at river mouth
            let cell_x = old_x as usize % width;
            let cell_y = (old_y as usize).min(height - 1);
            let final_deposit = sediment.min(10.0);
            if final_deposit > 0.0 {
                for &(dx, dy, weight) in brush {
                    let nx = ((cell_x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (cell_y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let idx = ny * width + nx;
                    changes.push((idx, final_deposit * weight));
                }
                deposited += final_deposit as f64;
            }
            break;
        }

        let slope = (-delta_height).clamp(0.0, 50.0);
        let capacity = (slope.max(params.droplet_min_volume) * velocity * water * params.droplet_capacity_factor).clamp(0.0, 500.0);

        let cell_x = old_x as usize % width;
        let cell_y = (old_y as usize).min(height - 1);
        let rock_hardness = *hardness.get(cell_x, cell_y);

        const MAX_CHANGE: f32 = 15.0;

        if sediment > capacity {
            // Deposit
            let deposit_amount = ((sediment - capacity) * params.droplet_deposit_rate).min(MAX_CHANGE);
            sediment -= deposit_amount;
            for &(dx, dy, weight) in brush {
                let nx = ((cell_x as i32 + dx).rem_euclid(width as i32)) as usize;
                let ny = (cell_y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                let idx = ny * width + nx;
                changes.push((idx, deposit_amount * weight));
            }
            deposited += deposit_amount as f64;
        } else {
            // Erode
            let hardness_factor = (1.0 - rock_hardness).max(0.1);
            let erode_amount = ((capacity - sediment) * params.droplet_erosion_rate * hardness_factor)
                .min(slope)
                .min(MAX_CHANGE);
            if erode_amount > 0.0 {
                sediment += erode_amount;
                for &(dx, dy, weight) in brush {
                    let nx = ((cell_x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (cell_y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let idx = ny * width + nx;
                    changes.push((idx, -erode_amount * weight));
                }
                eroded += erode_amount as f64;
            }
        }

        velocity = (velocity * velocity + delta_height * params.droplet_gravity)
            .clamp(0.0, 10000.0)
            .sqrt()
            .min(50.0);

        water *= 1.0 - params.droplet_evaporation;

        if water < params.droplet_min_volume {
            let final_deposit = sediment.min(MAX_CHANGE * 3.0);
            if final_deposit > 0.0 {
                for &(dx, dy, weight) in brush {
                    let nx = ((cell_x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (cell_y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let idx = ny * width + nx;
                    changes.push((idx, final_deposit * weight));
                }
                deposited += final_deposit as f64;
            }
            break;
        }
    }

    (changes, eroded, deposited)
}

/// Spawn a droplet at a high elevation using rejection sampling.
/// This simulates rain falling preferentially on mountains.
fn spawn_at_high_elevation(
    heightmap: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
    width_f: f32,
    height_f: f32,
    min_height: f32,
    height_range: f32,
    sea_level: f32,
) -> (f32, f32) {
    // Try up to 10 times to find a good spawn point
    for _ in 0..10 {
        let x = rng.gen_range(0.0..width_f);
        let y = rng.gen_range(0.0..height_f);
        let h = height_at(heightmap, x, y);

        // Must be above sea level
        if h < sea_level {
            continue;
        }

        // Probability of accepting this point increases with elevation
        // normalized_height is 0 at min_height, 1 at max_height
        let normalized_height = ((h - min_height) / height_range).clamp(0.0, 1.0);

        // Square the normalized height to strongly prefer high elevations
        // This means mountaintops are much more likely to spawn droplets
        let accept_probability = normalized_height * normalized_height;

        if rng.gen::<f32>() < accept_probability.max(0.1) {
            return (x, y);
        }
    }

    // Fallback: just return a random land point
    loop {
        let x = rng.gen_range(0.0..width_f);
        let y = rng.gen_range(0.0..height_f);
        let h = height_at(heightmap, x, y);
        if h >= sea_level {
            return (x, y);
        }
        // Emergency fallback after too many tries
        if rng.gen::<f32>() < 0.01 {
            return (x, y);
        }
    }
}

/// Apply erosion using brush pattern
/// Caps minimum height to prevent unrealistic ocean trenches
fn apply_erosion(
    brush: &[(i32, i32, f32)],
    heightmap: &mut Tilemap<f32>,
    x: usize,
    y: usize,
    amount: f32,
    width: usize,
    height: usize,
) {
    // Minimum height cap to prevent unrealistic trenches
    const MIN_TERRAIN_HEIGHT: f32 = -5000.0;

    for &(dx, dy, weight) in brush {
        let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

        let current = *heightmap.get(nx, ny);
        let new_height = (current - amount * weight).max(MIN_TERRAIN_HEIGHT);
        heightmap.set(nx, ny, new_height);
    }
}

/// Apply deposition using brush pattern
/// Caps maximum height to prevent unrealistic terrain buildup
fn apply_deposit(
    brush: &[(i32, i32, f32)],
    heightmap: &mut Tilemap<f32>,
    x: usize,
    y: usize,
    amount: f32,
    width: usize,
    height: usize,
) {
    // Maximum height cap to prevent unrealistic buildup
    const MAX_TERRAIN_HEIGHT: f32 = 2000.0;

    for &(dx, dy, weight) in brush {
        let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

        let current = *heightmap.get(nx, ny);
        let new_height = (current + amount * weight).min(MAX_TERRAIN_HEIGHT);
        heightmap.set(nx, ny, new_height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_droplet_erodes_slope() {
        // Create a simple sloped terrain
        let mut heightmap = Tilemap::new_with(32, 32, 0.0f32);
        for y in 0..32 {
            for x in 0..32 {
                // Slope from top-left (high) to bottom-right (low)
                let h = (32.0 - x as f32) + (32.0 - y as f32);
                heightmap.set(x, y, h);
            }
        }

        // Uniform hardness
        let hardness = Tilemap::new_with(32, 32, 0.3f32);

        let mut rng = ChaCha8Rng::seed_from_u64(12345);
        let params = ErosionParams {
            hydraulic_iterations: 1000,
            ..ErosionParams::default()
        };

        let stats = simulate(&mut heightmap, &hardness, &params, &mut rng);

        // Should have eroded something
        assert!(stats.total_eroded > 0.0);
        // Should have deposited something
        assert!(stats.total_deposited > 0.0);
    }

    #[test]
    fn test_hard_rock_resists_erosion() {
        let mut heightmap1 = Tilemap::new_with(32, 32, 0.0f32);
        let mut heightmap2 = Tilemap::new_with(32, 32, 0.0f32);

        for y in 0..32 {
            for x in 0..32 {
                let h = (32.0 - x as f32) + (32.0 - y as f32);
                heightmap1.set(x, y, h);
                heightmap2.set(x, y, h);
            }
        }

        // Soft rock
        let soft_hardness = Tilemap::new_with(32, 32, 0.1f32);
        // Hard rock
        let hard_hardness = Tilemap::new_with(32, 32, 0.9f32);

        let params = ErosionParams {
            hydraulic_iterations: 1000,
            ..ErosionParams::default()
        };

        let mut rng1 = ChaCha8Rng::seed_from_u64(12345);
        let mut rng2 = ChaCha8Rng::seed_from_u64(12345);

        let soft_stats = simulate(&mut heightmap1, &soft_hardness, &params, &mut rng1);
        let hard_stats = simulate(&mut heightmap2, &hard_hardness, &params, &mut rng2);

        // Soft rock should erode more
        assert!(soft_stats.total_eroded > hard_stats.total_eroded);
    }
}
