use std::cmp::Ordering;
use std::collections::BinaryHeap;

use noise::{NoiseFn, Perlin, Seedable};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::tilemap::Tilemap;

use super::types::{Plate, PlateId, PlateType, Vec2, WorldStyle};

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

/// Convert 2D map coordinates (normalized nx in [0, 1], ny in [0, 1]) to 3D cylindrical coordinates
/// to guarantee seamless periodic wrapping across longitude (X) without polar artifacts or grid-plane resonance.
fn cylindrical_coords(nx: f64, ny: f64, radius: f64) -> [f64; 3] {
    let angle = nx * std::f64::consts::TAU;
    let cx = angle.cos() * radius;
    let cz = angle.sin() * radius;
    let cy = (ny - 0.5) * radius * 2.0;

    // Rotate slightly so equator and parallels never align with Cartesian integer lattice planes
    let cos_a = 0.93969; // cos(20 deg)
    let sin_a = 0.34202; // sin(20 deg)
    let rx = cx;
    let ry = cy * cos_a - cz * sin_a + 17.382;
    let rz = cy * sin_a + cz * cos_a + 31.914;
    [rx + 11.234, ry, rz]
}

/// Fractional Brownian Motion in 3D - layers multiple octaves of noise for self-similar detail.
fn fbm_3d(
    noise: &Perlin,
    p: [f64; 3],
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += amplitude * noise.get([p[0] * frequency, p[1] * frequency, p[2] * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

/// Domain warping in 3D for natural, organic flowing boundaries.
fn domain_warp_cylindrical(
    p: [f64; 3],
    warp_noise: &Perlin,
    warp_strength: f64,
    warp_frequency: f64,
) -> [f64; 3] {
    let wx = warp_noise.get([p[0] * warp_frequency, p[1] * warp_frequency, p[2] * warp_frequency]);
    let wy = warp_noise.get([
        p[0] * warp_frequency + 100.0,
        p[1] * warp_frequency + 100.0,
        p[2] * warp_frequency + 100.0,
    ]);
    let wz = warp_noise.get([
        p[0] * warp_frequency + 200.0,
        p[1] * warp_frequency + 200.0,
        p[2] * warp_frequency + 200.0,
    ]);

    [
        p[0] + wx * warp_strength,
        p[1] + wy * warp_strength,
        p[2] + wz * warp_strength,
    ]
}

/// Generate planetary tectonic plates on a seamless cylindrical globe.
/// Plates wrap seamlessly in longitude (X) and feature a realistic hierarchy
/// of major plates and minor/micro-plates with organic fractal boundaries.
pub fn generate_plates(
    width: usize,
    height: usize,
    num_plates: Option<usize>,
    world_style: WorldStyle,
    rng: &mut ChaCha8Rng,
) -> (Tilemap<PlateId>, Vec<Plate>) {
    let (min_suggested, max_suggested) = world_style.suggested_plate_count();
    let total_plates_target = num_plates.unwrap_or_else(|| rng.gen_range(min_suggested..=max_suggested));
    // Ensure at least 6 plates for realistic tectonics, up to 24
    let total_plates = total_plates_target.clamp(6, 24);

    // Major plates form the primary lithosphere; minor plates form complex boundary zones
    let num_major = match world_style {
        WorldStyle::Pangaea => 3.min(total_plates / 2),
        WorldStyle::Continental => 4.min(total_plates / 2),
        WorldStyle::Archipelago | WorldStyle::Waterworld => 2.min(total_plates / 3),
        WorldStyle::Islands => 3.min(total_plates / 2),
        WorldStyle::Earthlike => (total_plates / 3).clamp(3, 6),
    };
    let num_minor = total_plates - num_major;

    // Boundary & lithospheric noises for fBm - creates organic fractal boundaries & weakness corridors
    let boundary_noise = Perlin::new(1).set_seed(rng.gen());
    let warp_noise_macro = Perlin::new(2).set_seed(rng.gen());
    let warp_noise_detail = Perlin::new(3).set_seed(rng.gen());
    let lithosphere_noise = Perlin::new(4).set_seed(rng.gen());

    let plate_noises: Vec<Perlin> = (0..total_plates)
        .map(|_| Perlin::new(rng.gen()).set_seed(rng.gen()))
        .collect();

    // Plate expansion biases: lower = expands faster (major plates), higher = slower (minor plates)
    let mut plate_biases: Vec<f32> = Vec::with_capacity(total_plates);
    // Plate directional elongation axes and aspect ratios for anisotropic drift
    let mut plate_angles: Vec<f64> = Vec::with_capacity(total_plates);
    let mut plate_elongations: Vec<f64> = Vec::with_capacity(total_plates);
    for i in 0..total_plates {
        if i < num_major {
            plate_biases.push(rng.gen_range(0.70..0.88));
            plate_angles.push(rng.gen_range(0.0..std::f64::consts::PI));
            plate_elongations.push(rng.gen_range(1.6..2.6));
        } else {
            plate_biases.push(rng.gen_range(1.05..1.35));
            plate_angles.push(rng.gen_range(0.0..std::f64::consts::PI));
            plate_elongations.push(rng.gen_range(1.3..2.0));
        }
    }

    let mut plate_map = Tilemap::new_with(width, height, PlateId::NONE);
    let mut heap: BinaryHeap<ExpansionCell> = BinaryHeap::new();

    // Seed plate points across the cylindrical planetary surface.
    // Use Poisson dart-throwing with cylindrical X-wrapping and cosine-latitude area weighting
    // so every plate has a single distinct seed spaced naturally around the globe.
    let mut seeds: Vec<(usize, usize)> = Vec::with_capacity(total_plates);
    let init_min_dist = ((width as f64 * height as f64) / (total_plates as f64 * 2.2)).sqrt();

    for i in 0..total_plates {
        let id = PlateId(i as u8);
        let mut placed_x = 0;
        let mut placed_y = 0;
        let mut placed = false;

        let max_sin_lat = if i < num_major { 0.65 } else { 0.85 };
        for attempt in 0..120 {
            let x = rng.gen_range(0..width);
            let sin_lat: f64 = rng.gen_range(-max_sin_lat..max_sin_lat);
            let y = (((1.0 - sin_lat) * 0.5 * (height as f64)) as usize).clamp(0, height - 1);

            let cur_min_dist = if attempt > 80 {
                init_min_dist * 0.4
            } else if attempt > 40 {
                init_min_dist * 0.7
            } else {
                init_min_dist
            };

            let dist_ok = seeds.iter().all(|&(sx, sy)| {
                let dx = (x as f64 - sx as f64).abs();
                let dx_wrapped = dx.min(width as f64 - dx);
                let dy = y as f64 - sy as f64;
                (dx_wrapped * dx_wrapped + dy * dy).sqrt() >= cur_min_dist
            });

            if dist_ok && plate_map.get(x, y).is_none() {
                placed_x = x;
                placed_y = y;
                placed = true;
                break;
            }
        }

        if !placed {
            let max_sin_lat = if i < num_major { 0.65 } else { 0.85 };
            placed_x = rng.gen_range(0..width);
            let sin_lat: f64 = rng.gen_range(-max_sin_lat..max_sin_lat);
            placed_y = (((1.0 - sin_lat) * 0.5 * (height as f64)) as usize).clamp(0, height - 1);
        }

        seeds.push((placed_x, placed_y));
        plate_map.set(placed_x, placed_y, id);
        heap.push(ExpansionCell {
            x: placed_x,
            y: placed_y,
            plate_id: id,
            priority: 0.0,
        });
    }

    // Planetary priority Dijkstra flood-fill with anisotropic drift and 2-stage cylindrical 3D noise
    while let Some(cell) = heap.pop() {
        let plate_idx = cell.plate_id.0 as usize;
        let plate_noise = &plate_noises[plate_idx];
        let bias = plate_biases[plate_idx];
        let plate_angle = plate_angles[plate_idx];
        let plate_elongation = plate_elongations[plate_idx];

        for (nx, ny) in plate_map.neighbors_8(cell.x, cell.y) {
            if plate_map.get(nx, ny).is_none() {
                plate_map.set(nx, ny, cell.plate_id);

                // Directional anisotropy: calculate step angle relative to plate drift axis
                let mut dx = nx as f64 - cell.x as f64;
                if dx > width as f64 * 0.5 { dx -= width as f64; }
                if dx < -(width as f64 * 0.5) { dx += width as f64; }
                let dy = ny as f64 - cell.y as f64;

                let step_angle = dy.atan2(dx);
                let angle_diff = (step_angle - plate_angle).abs();
                let sin_diff = angle_diff.sin();
                let dir_multiplier = 1.0 + (plate_elongation - 1.0) * (sin_diff * sin_diff);

                let fx = nx as f64 / width as f64;
                let fy = ny as f64 / height as f64;

                let p = cylindrical_coords(fx, fy, 1.0);
                let wp_macro = domain_warp_cylindrical(p, &warp_noise_macro, 0.70, 0.85);
                let wp_detail = domain_warp_cylindrical(wp_macro, &warp_noise_detail, 0.35, 2.2);

                let boundary_fbm = fbm_3d(&boundary_noise, [wp_detail[0] * 3.2, wp_detail[1] * 3.2, wp_detail[2] * 3.2], 5, 0.55, 2.0);
                let litho_corridors = fbm_3d(&lithosphere_noise, [wp_macro[0] * 1.3, wp_macro[1] * 1.3, wp_macro[2] * 1.3], 3, 0.5, 2.0);
                let local_fbm = fbm_3d(plate_noise, [p[0] * 3.5, p[1] * 3.5, p[2] * 3.5], 4, 0.5, 2.0) * 0.35;
                let jitter: f32 = rng.gen_range(0.0..0.08);

                let noise_cost = ((boundary_fbm + litho_corridors * 0.75 + local_fbm + 1.2).max(0.1) as f32) * 1.8;
                let step_cost = (0.4 + noise_cost) * (dir_multiplier as f32) * bias + jitter;
                let priority = cell.priority + step_cost;

                heap.push(ExpansionCell {
                    x: nx,
                    y: ny,
                    plate_id: cell.plate_id,
                    priority,
                });
            }
        }
    }

    // Count actual plate areas
    let mut plate_areas: Vec<usize> = vec![0; total_plates];
    for (_, _, &id) in plate_map.iter() {
        if !id.is_none() {
            plate_areas[id.0 as usize] += 1;
        }
    }

    // Select continental vs oceanic plates to target world_style land coverage
    let total_cells = width * height;
    let target_land_fraction = world_style.target_land_fraction();
    // Subduction arcs, hotspot tracks, and coastal shelves add ~5-8% extra land,
    // so scale the tectonic continent target so total emerged land hits target_land_fraction
    let target_plate_fraction = if matches!(world_style, WorldStyle::Pangaea) {
        target_land_fraction
    } else {
        target_land_fraction * 0.82
    };
    let target_land_cells = (total_cells as f64 * target_plate_fraction) as usize;
    let min_continental = world_style.min_continental_plates();
    let max_plate_fraction = world_style.max_continental_plate_fraction();
    let max_plate_cells = if max_plate_fraction > 0.0 {
        (total_cells as f64 * max_plate_fraction) as usize
    } else {
        usize::MAX
    };

    let mut best_subset: Vec<usize> = Vec::new();
    let mut best_diff = i64::MAX;

    if total_plates <= 16 {
        let num_combos = 1usize << total_plates;
        for mask in 1..num_combos {
            let count = mask.count_ones() as usize;
            if count < min_continental {
                continue;
            }
            let mut sum_area = 0usize;
            let mut exceeds_max = false;
            for i in 0..total_plates {
                if (mask & (1 << i)) != 0 {
                    let area = plate_areas[i];
                    if area > max_plate_cells && !matches!(world_style, WorldStyle::Pangaea) {
                        exceeds_max = true;
                        break;
                    }
                    sum_area += area;
                }
            }
            if exceeds_max {
                continue;
            }
            let diff = (sum_area as i64 - target_land_cells as i64).abs();
            if diff < best_diff {
                best_diff = diff;
                best_subset = (0..total_plates).filter(|&i| (mask & (1 << i)) != 0).collect();
            }
        }
    } else {
        for _ in 0..20_000 {
            let count = rng.gen_range(min_continental..=total_plates.min(min_continental + 4));
            let mut indices: Vec<usize> = (0..total_plates).collect();
            for i in 0..count {
                let j = rng.gen_range(i..total_plates);
                indices.swap(i, j);
            }
            let candidate = &indices[..count];
            let mut sum_area = 0usize;
            let mut exceeds_max = false;
            for &idx in candidate {
                let area = plate_areas[idx];
                if area > max_plate_cells && !matches!(world_style, WorldStyle::Pangaea) {
                    exceeds_max = true;
                    break;
                }
                sum_area += area;
            }
            if exceeds_max {
                continue;
            }
            let diff = (sum_area as i64 - target_land_cells as i64).abs();
            if diff < best_diff {
                best_diff = diff;
                best_subset = candidate.to_vec();
            }
        }
    }

    // Fallback if strict max_plate_fraction could not be satisfied
    if best_subset.is_empty() {
        let mut indexed: Vec<(usize, usize)> = (0..total_plates).map(|i| (i, plate_areas[i])).collect();
        if world_style.force_many_plates() {
            indexed.sort_by(|a, b| a.1.cmp(&b.1));
        } else {
            indexed.sort_by(|a, b| b.1.cmp(&a.1));
        }
        let mut cur = 0usize;
        for (idx, area) in indexed {
            let dist_without = (target_land_cells as i64 - cur as i64).abs();
            let dist_with = (target_land_cells as i64 - (cur + area) as i64).abs();
            if dist_with <= dist_without || best_subset.len() < min_continental {
                best_subset.push(idx);
                cur += area;
            }
        }
        while best_subset.len() < min_continental && best_subset.len() < total_plates {
            for i in 0..total_plates {
                if !best_subset.contains(&i) {
                    best_subset.push(i);
                    break;
                }
            }
        }
    }

    let continental_set: std::collections::HashSet<usize> = best_subset.into_iter().collect();

    // Create Plate instances with realistic kinematics (convergent, divergent, transform vectors)
    let plates: Vec<Plate> = (0..total_plates)
        .map(|i| {
            let is_continental = continental_set.contains(&i);
            let plate_type = if is_continental {
                PlateType::Continental
            } else {
                PlateType::Oceanic
            };

            let base_elevation = match plate_type {
                PlateType::Oceanic => rng.gen_range(-0.4..-0.1),
                PlateType::Continental => rng.gen_range(0.05..0.2),
            };

            // Random velocity direction and realistic drift magnitude
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let magnitude = rng.gen_range(0.15..0.55);
            let velocity = Vec2::new(angle.cos() * magnitude, angle.sin() * magnitude);

            let color = match plate_type {
                PlateType::Oceanic => [
                    rng.gen_range(30..80),
                    rng.gen_range(60..120),
                    rng.gen_range(150..220),
                ],
                PlateType::Continental => [
                    rng.gen_range(100..180),
                    rng.gen_range(140..200),
                    rng.gen_range(80..140),
                ],
            };

            Plate {
                id: PlateId(i as u8),
                plate_type,
                velocity,
                base_elevation,
                color,
            }
        })
        .collect();

    (plate_map, plates)
}
