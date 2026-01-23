use std::cmp::Ordering;
use std::collections::BinaryHeap;

use noise::{NoiseFn, Perlin, Seedable};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::tilemap::Tilemap;

use super::types::{Plate, PlateId, WorldStyle};

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
///
/// The `world_style` parameter controls land/ocean distribution:
/// - Earthlike: ~35% land, standard Earth-like distribution
/// - Archipelago: ~18% land, many small islands scattered everywhere
/// - Islands: ~25% land, island chains and small continents
/// - Pangaea: ~40% land, one supercontinent
/// - Continental: ~50% land, multiple large continents
/// - Waterworld: ~8% land, sparse tiny islands
pub fn generate_plates(
    width: usize,
    height: usize,
    num_plates: Option<usize>,
    world_style: WorldStyle,
    rng: &mut ChaCha8Rng,
) -> (Tilemap<PlateId>, Vec<Plate>) {
    // Border margin for edge plates
    let border_margin = (width.min(height) / 10).max(10);

    // 4 border plates (always oceanic) + interior plates
    let num_border_plates: usize = 4;
    let (min_plates, max_plates) = world_style.suggested_plate_count();
    let num_interior_plates: usize = num_plates.unwrap_or_else(|| rng.gen_range(min_plates..=max_plates));
    let total_plates = num_border_plates + num_interior_plates;

    // Select size distribution mode based on world style
    let size_mode = match world_style {
        WorldStyle::Pangaea => PlateSizeMode::Supercontinent,
        WorldStyle::Continental => {
            // Prefer MajorMinor for large continents
            if rng.gen_bool(0.7) { PlateSizeMode::MajorMinor } else { PlateSizeMode::Balanced }
        }
        WorldStyle::Archipelago | WorldStyle::Waterworld => {
            // Force balanced or chaotic for even island distribution
            if rng.gen_bool(0.6) { PlateSizeMode::Balanced } else { PlateSizeMode::Chaotic }
        }
        WorldStyle::Islands => {
            // Mix of sizes for variety in island chains
            match rng.gen_range(0..3) {
                0 => PlateSizeMode::MajorMinor,
                1 => PlateSizeMode::Chaotic,
                _ => PlateSizeMode::Balanced,
            }
        }
        WorldStyle::Earthlike => {
            // Original random selection
            match rng.gen_range(0..100) {
                0..=20 => PlateSizeMode::Supercontinent,
                21..=45 => PlateSizeMode::MajorMinor,
                46..=70 => PlateSizeMode::Chaotic,
                _ => PlateSizeMode::Balanced,
            }
        }
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
            // One large plate (~35-45% area) plus a few medium plates for balance
            let dominant_idx = rng.gen_range(0..num_interior_plates);
            // Pick 2-3 secondary plates
            let num_secondary = rng.gen_range(2..=3.min(num_interior_plates - 1));
            let mut secondary_indices: Vec<usize> = (0..num_interior_plates)
                .filter(|&i| i != dominant_idx)
                .collect();
            for i in 0..num_secondary.min(secondary_indices.len()) {
                let j = rng.gen_range(i..secondary_indices.len());
                secondary_indices.swap(i, j);
            }
            let secondary_set: std::collections::HashSet<usize> =
                secondary_indices.iter().take(num_secondary).copied().collect();

            (0..num_interior_plates)
                .map(|i| {
                    if i == dominant_idx {
                        rng.gen_range(0.5..0.7)  // Large but not overwhelming
                    } else if secondary_set.contains(&i) {
                        rng.gen_range(0.9..1.2)  // Medium plates for balance
                    } else {
                        rng.gen_range(1.4..1.8)  // Smaller plates
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
            // Varied sizes - use distribution for natural variety, but capped
            (0..num_interior_plates)
                .map(|_| {
                    // Distribution: many medium-small, few large
                    let r: f32 = rng.gen();
                    0.6 + r * r * 1.4  // Range ~0.6 to ~2.0, more moderate variance
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

        for (nx, ny) in plate_map.neighbors_8(cell.x, cell.y) {
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

    // Count actual plate areas after flood-fill
    let mut plate_areas: Vec<usize> = vec![0; total_plates];
    for (_, _, &id) in plate_map.iter() {
        if !id.is_none() {
            plate_areas[id.0 as usize] += 1;
        }
    }

    // Target land percentage based on world style
    let total_cells = width * height;
    let target_land_fraction = world_style.target_land_fraction();
    let target_land_cells = (total_cells as f64 * target_land_fraction) as usize;
    let max_plate_fraction = world_style.max_continental_plate_fraction();
    let max_plate_cells = if max_plate_fraction > 0.0 {
        (total_cells as f64 * max_plate_fraction) as usize
    } else {
        usize::MAX
    };
    let min_continental = world_style.min_continental_plates();

    // Sort interior plates by actual area
    // For archipelago/waterworld, sort smallest first to prefer many small islands
    let mut interior_areas: Vec<(usize, usize)> = (0..num_interior_plates)
        .map(|i| {
            let global_idx = num_border_plates + i;
            (global_idx, plate_areas[global_idx])
        })
        .collect();

    if world_style.force_many_plates() {
        // Sort smallest first for archipelago-style worlds
        interior_areas.sort_by(|a, b| a.1.cmp(&b.1));
    } else {
        // Sort largest first (original behavior)
        interior_areas.sort_by(|a, b| b.1.cmp(&a.1));
    }

    // Select plates to be continental, trying to match target land coverage
    let mut continental_set: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut current_land = 0usize;

    for (plate_idx, area) in &interior_areas {
        // Skip plates that are too large for this world style
        if *area > max_plate_cells {
            continue;
        }

        // Calculate how far from target we'd be with vs without this plate
        let distance_without = (target_land_cells as i64 - current_land as i64).abs();
        let distance_with = (target_land_cells as i64 - (current_land + area) as i64).abs();

        // Add if it gets us closer to target, or if we need more continental plates
        let need_more = continental_set.len() < min_continental;
        if distance_with <= distance_without || need_more {
            continental_set.insert(*plate_idx);
            current_land += area;
        }
    }

    // Ensure at least minimum continental plates for landmass
    // If we couldn't meet the minimum due to size constraints, add smallest eligible plates
    if continental_set.len() < min_continental && !interior_areas.is_empty() {
        // Sort by size (smallest first) for this fallback
        let mut sorted_by_size: Vec<_> = interior_areas.iter()
            .filter(|(idx, _)| !continental_set.contains(idx))
            .collect();
        sorted_by_size.sort_by(|a, b| a.1.cmp(&b.1));

        for (plate_idx, _) in sorted_by_size {
            if continental_set.len() >= min_continental {
                break;
            }
            continental_set.insert(*plate_idx);
        }
    }

    // Final fallback: ensure at least one continental plate
    if continental_set.is_empty() && !interior_areas.is_empty() {
        continental_set.insert(interior_areas[0].0);
    }

    // Create plate objects
    // First 4 are border plates (oceanic)
    let mut plates: Vec<Plate> = (0..num_border_plates)
        .map(|i| Plate::oceanic_border(PlateId(i as u8)))
        .collect();

    // Create interior plates with types based on area-targeting
    plates.extend((0..num_interior_plates)
        .map(|i| {
            let global_idx = num_border_plates + i;
            let is_continental = continental_set.contains(&global_idx);
            Plate::new_with_type(PlateId(global_idx as u8), rng, is_continental)
        }));

    (plate_map, plates)
}
