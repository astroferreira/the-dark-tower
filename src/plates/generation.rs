use std::cmp::Ordering;
use std::collections::BinaryHeap;

use noise::{NoiseFn, Perlin, Seedable};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::tilemap::Tilemap;

use super::types::{Plate, PlateId};

/// Entry in the priority queue for plate expansion.
#[derive(Clone)]
struct ExpansionCell {
    x: usize,
    y: usize,
    plate_id: PlateId,
    priority: f32,
}

impl PartialEq for ExpansionCell {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for ExpansionCell {}

impl PartialOrd for ExpansionCell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExpansionCell {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .partial_cmp(&self.priority)
            .unwrap_or(Ordering::Equal)
    }
}

/// Fractional Brownian Motion - layers multiple octaves of noise for self-similar detail.
/// Creates fractal patterns that look equally detailed at all scales.
fn fbm(
    noise: &Perlin,
    x: f64,
    y: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += amplitude * noise.get([x * frequency, y * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

/// Domain warping - distorts coordinates using noise for organic shapes.
fn domain_warp(
    x: f64,
    y: f64,
    warp_noise: &Perlin,
    warp_strength: f64,
    warp_frequency: f64,
) -> (f64, f64) {
    let warp_x = warp_noise.get([x * warp_frequency, y * warp_frequency]);
    let warp_y = warp_noise.get([x * warp_frequency + 100.0, y * warp_frequency + 100.0]);

    (x + warp_x * warp_strength, y + warp_y * warp_strength)
}

/// Plate size distribution modes for variety in generation
#[derive(Clone, Copy, Debug)]
enum PlateSizeMode {
    /// One dominant plate (~40-60% of area), rest small
    Supercontinent,
    /// 2-3 major plates (~20-30% each), rest small
    MajorMinor,
    /// Highly varied sizes - some big, some medium, some tiny
    Chaotic,
    /// All plates roughly similar size (original behavior)
    Balanced,
}

/// Generate tectonic plates using fractal noise-modulated flood-fill.
/// Uses fBm and domain warping for realistic, self-similar coastlines.
/// Includes 4 border plates (oceanic) at map edges to create natural coastlines.
/// Randomly varies plate sizes for natural variety.
pub fn generate_plates(
    width: usize,
    height: usize,
    num_plates: Option<usize>,
    rng: &mut ChaCha8Rng,
) -> (Tilemap<PlateId>, Vec<Plate>) {
    // Border margin for edge plates
    let border_margin = (width.min(height) / 10).max(10);

    // 4 border plates (always oceanic) + interior plates
    let num_border_plates: usize = 4;
    let num_interior_plates: usize = num_plates.unwrap_or_else(|| rng.gen_range(6..=15));
    let total_plates = num_border_plates + num_interior_plates;

    // Randomly select a size distribution mode
    let size_mode = match rng.gen_range(0..100) {
        0..=20 => PlateSizeMode::Supercontinent,  // 20% chance
        21..=45 => PlateSizeMode::MajorMinor,     // 25% chance
        46..=70 => PlateSizeMode::Chaotic,        // 25% chance
        _ => PlateSizeMode::Balanced,             // 30% chance
    };

    // Boundary noise for fBm - creates fractal coastlines
    let boundary_noise = Perlin::new(1).set_seed(rng.gen());

    // Domain warping noise - creates organic flowing shapes
    let warp_noise = Perlin::new(2).set_seed(rng.gen());

    // Per-plate noise for local variation
    let plate_noises: Vec<Perlin> = (0..total_plates)
        .map(|_| Perlin::new(rng.gen()).set_seed(rng.gen()))
        .collect();

    // Generate plate biases based on size mode
    // Lower bias = faster expansion = bigger plate
    let mut plate_bias: Vec<f32> = vec![1.1; num_border_plates]; // Border plates always 1.1

    let interior_biases: Vec<f32> = match size_mode {
        PlateSizeMode::Supercontinent => {
            // One dominant plate (very low bias), rest are small (high bias)
            let dominant_idx = rng.gen_range(0..num_interior_plates);
            (0..num_interior_plates)
                .map(|i| {
                    if i == dominant_idx {
                        rng.gen_range(0.3..0.5)  // Very fast expansion
                    } else {
                        rng.gen_range(1.5..2.5)  // Slow expansion
                    }
                })
                .collect()
        }
        PlateSizeMode::MajorMinor => {
            // 2-3 major plates, rest small
            let num_major = rng.gen_range(2..=3.min(num_interior_plates));
            let mut major_indices: Vec<usize> = (0..num_interior_plates).collect();
            // Shuffle and take first num_major as major plates
            for i in 0..num_major {
                let j = rng.gen_range(i..num_interior_plates);
                major_indices.swap(i, j);
            }
            let major_set: std::collections::HashSet<usize> =
                major_indices[..num_major].iter().copied().collect();

            (0..num_interior_plates)
                .map(|i| {
                    if major_set.contains(&i) {
                        rng.gen_range(0.4..0.7)  // Fast expansion
                    } else {
                        rng.gen_range(1.3..2.0)  // Slower expansion
                    }
                })
                .collect()
        }
        PlateSizeMode::Chaotic => {
            // Highly varied - use exponential distribution for natural variety
            (0..num_interior_plates)
                .map(|_| {
                    // Exponential-like distribution: many small, few large
                    let r: f32 = rng.gen();
                    0.4 + r * r * 2.0  // Range ~0.4 to ~2.4, skewed toward higher values
                })
                .collect()
        }
        PlateSizeMode::Balanced => {
            // Original behavior - similar sizes with small variation
            (0..num_interior_plates)
                .map(|_| rng.gen_range(0.8..1.2))
                .collect()
        }
    };
    plate_bias.extend(interior_biases);

    // Calculate extra seed points for dominant plates (lower bias = more seeds)
    let extra_seeds: Vec<usize> = plate_bias[num_border_plates..]
        .iter()
        .map(|&bias| {
            if bias < 0.6 {
                rng.gen_range(3..6)  // Dominant plates get 3-5 extra seeds
            } else if bias < 0.9 {
                rng.gen_range(1..3)  // Major plates get 1-2 extra seeds
            } else {
                0  // Normal/small plates get no extra seeds
            }
        })
        .collect();

    let mut plate_map = Tilemap::new_with(width, height, PlateId::NONE);
    let mut heap: BinaryHeap<ExpansionCell> = BinaryHeap::new();

    // Seed border plates along edges
    let seed_step = (border_margin / 2).max(1);

    // Top edge (plate 0)
    for x in (0..width).step_by(seed_step) {
        plate_map.set(x, 0, PlateId(0));
        heap.push(ExpansionCell { x, y: 0, plate_id: PlateId(0), priority: 0.0 });
    }
    // Bottom edge (plate 1)
    for x in (0..width).step_by(seed_step) {
        plate_map.set(x, height - 1, PlateId(1));
        heap.push(ExpansionCell { x, y: height - 1, plate_id: PlateId(1), priority: 0.0 });
    }
    // Left edge (plate 2)
    for y in (0..height).step_by(seed_step) {
        plate_map.set(0, y, PlateId(2));
        heap.push(ExpansionCell { x: 0, y, plate_id: PlateId(2), priority: 0.0 });
    }
    // Right edge (plate 3)
    for y in (0..height).step_by(seed_step) {
        plate_map.set(width - 1, y, PlateId(3));
        heap.push(ExpansionCell { x: width - 1, y, plate_id: PlateId(3), priority: 0.0 });
    }

    // Seed interior plates (starting at index 4) - away from edges
    // Plates with lower bias get extra seed points for faster coverage
    for i in 0..num_interior_plates {
        let id = PlateId((num_border_plates + i) as u8);
        let num_seeds = 1 + extra_seeds[i];

        for _ in 0..num_seeds {
            let x = rng.gen_range(border_margin..width - border_margin);
            let y = rng.gen_range(border_margin..height - border_margin);

            // Only set if not already claimed
            if plate_map.get(x, y).is_none() {
                plate_map.set(x, y, id);
                heap.push(ExpansionCell {
                    x,
                    y,
                    plate_id: id,
                    priority: 0.0,
                });
            }
        }
    }

    // Expansion with fractal noise (fBm + domain warping)
    while let Some(cell) = heap.pop() {
        let plate_idx = cell.plate_id.0 as usize;
        let plate_noise = &plate_noises[plate_idx];
        let bias = plate_bias[plate_idx];

        for (nx, ny) in plate_map.neighbors(cell.x, cell.y) {
            if plate_map.get(nx, ny).is_none() {
                plate_map.set(nx, ny, cell.plate_id);

                let fx = nx as f64 / width as f64;
                let fy = ny as f64 / height as f64;

                // Domain warp the coordinates for organic flowing shapes
                // High warp strength creates dramatic flowing distortions
                let (wx, wy) = domain_warp(fx, fy, &warp_noise, 0.8, 2.0);

                // fBm with multiple octaves for fractal, self-similar boundaries
                // 8 octaves with higher persistence = more high-frequency detail
                let boundary_fbm = fbm(&boundary_noise, wx * 8.0, wy * 8.0, 8, 0.65, 2.0);

                // Per-plate fBm for local variation - also more aggressive
                let local_fbm = fbm(plate_noise, fx * 6.0, fy * 6.0, 6, 0.6, 2.0) * 0.6;

                // Small jitter for natural feel
                let jitter: f32 = rng.gen_range(0.0..0.15);

                // Priority: distance + fractal noise (stronger noise influence)
                let noise_cost = (boundary_fbm + local_fbm + 1.0) as f32 * 2.5;
                let priority = cell.priority + (0.5 + noise_cost + jitter) * bias;

                heap.push(ExpansionCell {
                    x: nx,
                    y: ny,
                    plate_id: cell.plate_id,
                    priority,
                });
            }
        }
    }

    // Create plate objects
    // First 4 are border plates (oceanic)
    let mut plates: Vec<Plate> = (0..num_border_plates)
        .map(|i| Plate::oceanic_border(PlateId(i as u8)))
        .collect();
    // Rest are random interior plates
    plates.extend((0..num_interior_plates)
        .map(|i| Plate::random(PlateId((num_border_plates + i) as u8), rng)));

    (plate_map, plates)
}
