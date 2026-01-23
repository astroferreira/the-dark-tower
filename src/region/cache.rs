//! Region cache with deterministic edge-coherent generation
//!
//! Generates regions using world-coordinate-based deterministic noise to ensure
//! seamless continuity across tile boundaries without post-processing stitching.
//! Uses level-of-detail (LOD) for distant regions and caches results for performance.

use std::collections::HashMap;
use noise::{NoiseFn, Perlin, Seedable};
use crate::world::WorldData;
use crate::biomes::ExtendedBiome;
use super::generator::{RegionMap, REGION_SIZE};

/// Level of detail for region generation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionLOD {
    /// Full detail - all generation passes
    Full,
    /// Medium detail - terrain + rivers, simplified vegetation
    Medium,
    /// Low detail - terrain only, no fine details
    Low,
}

/// Cached region with metadata
struct CachedRegion {
    region: RegionMap,
    lod: RegionLOD,
}

/// Region cache that manages multi-region generation
pub struct RegionCache {
    /// Cached regions indexed by (world_x, world_y)
    regions: HashMap<(usize, usize), CachedRegion>,
    /// Maximum number of cached regions
    max_cache_size: usize,
    /// Seed for generation
    seed: u64,
}

impl RegionCache {
    /// Create a new region cache
    pub fn new(seed: u64) -> Self {
        Self {
            regions: HashMap::new(),
            max_cache_size: 25, // 5x5 grid around cursor
            seed,
        }
    }

    /// Get a region, generating it and neighbors if needed
    pub fn get_region(&mut self, world: &WorldData, world_x: usize, world_y: usize) -> &RegionMap {
        // Normalize coordinates (wrap x, clamp y)
        let wx = world_x % world.width;
        let wy = world_y.min(world.height.saturating_sub(1));

        // Generate this region and its neighbors if not cached
        self.ensure_region_cluster(world, wx, wy);

        // Return the requested region
        &self.regions.get(&(wx, wy))
            .unwrap_or_else(|| panic!(
                "Region ({}, {}) not found after ensure_region_cluster. \
                 Original coords: ({}, {}), world size: {}x{}",
                wx, wy, world_x, world_y, world.width, world.height
            ))
            .region
    }

    /// Ensure a cluster of regions exists (center + 8 neighbors)
    fn ensure_region_cluster(&mut self, world: &WorldData, center_x: usize, center_y: usize) {
        // Check which regions need generation
        let mut to_generate: Vec<(usize, usize, RegionLOD)> = Vec::new();

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let wx = (center_x as i32 + dx).rem_euclid(world.width as i32) as usize;
                let wy = (center_y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;

                if !self.regions.contains_key(&(wx, wy)) {
                    // Center gets full detail, neighbors get medium
                    let lod = if dx == 0 && dy == 0 {
                        RegionLOD::Full
                    } else {
                        RegionLOD::Medium
                    };
                    to_generate.push((wx, wy, lod));
                }
            }
        }

        if to_generate.is_empty() {
            return;
        }

        // Generate regions using deterministic world-coordinate-based noise
        // No stitching needed - edges naturally match because:
        // 1. Base terrain comes from same world heightmap (consistent at boundaries)
        // 2. Noise is sampled at world coordinates (same point = same value)
        for (wx, wy, lod) in to_generate {
            let region = generate_region_deterministic(world, wx, wy, self.seed, lod);
            self.regions.insert((wx, wy), CachedRegion { region, lod });
        }

        // Prune cache if too large
        self.prune_cache(center_x, center_y, world.width);
    }

    /// Remove distant regions from cache
    fn prune_cache(&mut self, center_x: usize, center_y: usize, world_width: usize) {
        if self.regions.len() <= self.max_cache_size {
            return;
        }

        // Find regions to remove (furthest from center)
        let mut distances: Vec<((usize, usize), i32)> = self.regions.keys()
            .map(|&(x, y)| {
                let dx = ((x as i32 - center_x as i32).abs()).min(
                    world_width as i32 - (x as i32 - center_x as i32).abs()
                );
                let dy = (y as i32 - center_y as i32).abs();
                ((x, y), dx + dy)
            })
            .collect();

        // Sort by distance ASCENDING (nearest first), so pop() gets the FURTHEST
        distances.sort_by_key(|(_, d)| *d);

        // Remove furthest regions until under limit
        while self.regions.len() > self.max_cache_size {
            if let Some((coord, _)) = distances.pop() {
                self.regions.remove(&coord);
            } else {
                break;
            }
        }
    }

    /// Clear the cache (e.g., when world changes)
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Check if a region is cached
    pub fn is_cached(&self, world_x: usize, world_y: usize) -> bool {
        self.regions.contains_key(&(world_x, world_y))
    }
}

/// Generate a region using deterministic world-coordinate-based generation
///
/// Edges naturally match without stitching because:
/// 1. Base terrain comes from same world heightmap (consistent at boundaries)
/// 2. Noise is sampled at world coordinates (same point = same value)
fn generate_region_deterministic(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    seed: u64,
    lod: RegionLOD,
) -> RegionMap {
    let size = REGION_SIZE;
    let mut region = RegionMap::new(size, world_x, world_y);

    // Create noise generators
    let noise = Perlin::new(1).set_seed(seed as u32);
    let noise2 = Perlin::new(1).set_seed((seed.wrapping_add(12345)) as u32);
    let noise3 = Perlin::new(1).set_seed((seed.wrapping_add(67890)) as u32);

    // Get base data
    let base_biome = *world.biomes.get(world_x, world_y);
    let base_height = *world.heightmap.get(world_x, world_y);
    let base_moisture = *world.moisture.get(world_x, world_y);
    let base_temp = *world.temperature.get(world_x, world_y);

    // Get handshake data
    let handshake = world.handshakes.as_ref().map(|h| &h.get(world_x, world_y).tile);

    // Phase 1: Generate terrain using extended 5x5 world sampling for smooth interpolation
    generate_terrain_deterministic(world, &mut region, world_x, world_y, &noise, seed);

    // Phase 2: Add detail (reduced for lower LOD)
    let roughness = handshake.map(|h| h.roughness).unwrap_or(0.3);
    let detail_scale = match lod {
        RegionLOD::Full => 1.0,
        RegionLOD::Medium => 0.7,
        RegionLOD::Low => 0.3,
    };
    add_terrain_detail_deterministic(&mut region, base_biome, base_height, roughness * detail_scale, &noise, &noise2, seed);

    // Phase 3-4: Rivers (skip for Low LOD)
    if lod != RegionLOD::Low {
        // Trace rivers from the world river network - these handle cross-tile continuity
        if let Some(ref river_network) = world.river_network {
            trace_rivers_from_world(&mut region, river_network, world_x, world_y);
        }

        // Generate simple local drainage based on terrain flow
        let flow_acc = handshake.map(|h| h.flow_accumulation).unwrap_or(1.0);
        let water_table = handshake.map(|h| h.water_table).unwrap_or(0.3);
        generate_simple_drainage(&mut region, world, world_x, world_y, flow_acc, water_table, &noise3, seed);
    }

    // Phase 5-6: Vegetation and rocks (simplified for Medium, skip for Low)
    if lod == RegionLOD::Full {
        let veg_density = handshake.map(|h| h.vegetation_density).unwrap_or_else(|| biome_vegetation_density(base_biome));
        let veg_pattern = handshake.map(|h| h.vegetation_pattern).unwrap_or(super::handshake::VegetationPattern::Uniform);
        generate_vegetation(&mut region, base_biome, base_moisture, base_temp, veg_density, veg_pattern, &noise, seed);

        let surface_minerals = handshake.map(|h| h.surface_minerals).unwrap_or(0.1);
        generate_rocks(&mut region, base_biome, surface_minerals, &noise2, seed);
    } else if lod == RegionLOD::Medium {
        // Simplified vegetation
        let veg_density = handshake.map(|h| h.vegetation_density).unwrap_or(0.5);
        for y in 0..size {
            for x in 0..size {
                let h = region.get_height(x, y);
                if h > 0.0 {
                    region.set_vegetation(x, y, veg_density * 0.8);
                }
            }
        }
    }

    // Phase 7: Biomes
    generate_local_biomes(&mut region, world, world_x, world_y);

    // Calculate height stats
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for y in 0..size {
        for x in 0..size {
            let h = region.get_height(x, y);
            if h < min_h { min_h = h; }
            if h > max_h { max_h = h; }
        }
    }
    region.height_min = min_h;
    region.height_max = max_h;

    region
}

/// Generate terrain using extended 5x5 world sampling for smooth edge interpolation
///
/// Uses world-coordinate-based noise so edges naturally match between adjacent regions.
fn generate_terrain_deterministic(
    world: &WorldData,
    region: &mut RegionMap,
    world_x: usize,
    world_y: usize,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;
    let world_width = world.width;
    let world_height = world.height;

    // Sample a 5x5 grid of world tiles for better edge interpolation
    let mut samples = [[0.0f32; 5]; 5];
    for sy in 0..5 {
        for sx in 0..5 {
            let wx = (world_x as i32 + sx as i32 - 2).rem_euclid(world_width as i32) as usize;
            let wy = (world_y as i32 + sy as i32 - 2).clamp(0, world_height as i32 - 1) as usize;
            samples[sy][sx] = *world.heightmap.get(wx, wy);
        }
    }

    // Generate base terrain - this naturally matches at boundaries
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Bicubic interpolation from extended 5x5 world samples
            let base_height = bicubic_sample_5x5(&samples, fx, fy);

            // Add medium-scale variation using world coordinates (deterministic at boundaries)
            let world_fx = world_x as f64 + fx as f64;
            let world_fy = world_y as f64 + fy as f64;
            let medium_noise = noise.get([world_fx * 3.0, world_fy * 3.0, seed as f64 * 0.001]) as f32;

            let variation_scale = if base_height > 500.0 {
                30.0
            } else if base_height > 100.0 {
                15.0
            } else if base_height > 0.0 {
                8.0
            } else {
                2.0
            };

            region.set_height(x, y, base_height + medium_noise * variation_scale);
        }
    }
}

// ============================================================================
// Helper functions for deterministic edge-coherent generation
// ============================================================================

/// Bicubic interpolation from a 5x5 sample grid for smoother edge transitions
fn bicubic_sample_5x5(samples: &[[f32; 5]; 5], fx: f32, fy: f32) -> f32 {
    // Map fx,fy (0-1) to sample space (centered in the 5x5 grid)
    // fx=0 maps to sample x=1.5, fx=1 maps to x=2.5
    let sx = fx + 1.5;
    let sy = fy + 1.5;

    // Get the four corners for bilinear interpolation
    let x0 = sx.floor() as usize;
    let y0 = sy.floor() as usize;
    let x1 = (x0 + 1).min(4);
    let y1 = (y0 + 1).min(4);

    let tx = sx - x0 as f32;
    let ty = sy - y0 as f32;

    // Smoothstep interpolation
    let tx = tx * tx * (3.0 - 2.0 * tx);
    let ty = ty * ty * (3.0 - 2.0 * ty);

    let v00 = samples[y0][x0];
    let v10 = samples[y0][x1];
    let v01 = samples[y1][x0];
    let v11 = samples[y1][x1];

    let top = v00 * (1.0 - tx) + v10 * tx;
    let bottom = v01 * (1.0 - tx) + v11 * tx;

    top * (1.0 - ty) + bottom * ty
}

/// Smooth edge fade for detail noise only (not base terrain)
/// Only fades in the outer margin to allow natural blending without affecting core terrain
fn smooth_edge_fade(fx: f32, fy: f32, margin: f32) -> f32 {
    let fade_x = if fx < margin {
        fx / margin
    } else if fx > 1.0 - margin {
        (1.0 - fx) / margin
    } else {
        1.0
    };

    let fade_y = if fy < margin {
        fy / margin
    } else if fy > 1.0 - margin {
        (1.0 - fy) / margin
    } else {
        1.0
    };

    // Use minimum to ensure corners fade properly
    fade_x.min(fade_y).clamp(0.0, 1.0)
}

/// Add terrain detail using deterministic world-coordinate noise
///
/// Uses world coordinates for noise sampling so adjacent regions produce
/// identical values at shared boundaries.
fn add_terrain_detail_deterministic(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    base_height: f32,
    roughness: f32,
    noise: &Perlin,
    noise2: &Perlin,
    seed: u64,
) {
    let size = region.size;

    let (base_amplitude, octaves, persistence) = match biome {
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineMeadow => (25.0, 5, 0.6),
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
        ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest => (12.0, 4, 0.5),
        ExtendedBiome::Foothills | ExtendedBiome::MontaneForest => (18.0, 4, 0.55),
        ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna => (6.0, 3, 0.4),
        ExtendedBiome::Desert => (10.0, 3, 0.45),
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => (3.0, 2, 0.3),
        ExtendedBiome::Tundra => (5.0, 3, 0.4),
        ExtendedBiome::Ocean | ExtendedBiome::DeepOcean | ExtendedBiome::CoastalWater => (2.0, 2, 0.3),
        _ => (8.0, 3, 0.5)
    };

    let roughness_multiplier = 0.3 + roughness * 1.7;
    let amplitude = base_amplitude * roughness_multiplier;

    let height_factor = if base_height > 1000.0 { 1.5 }
        else if base_height > 500.0 { 1.2 }
        else if base_height > 0.0 { 1.0 }
        else { 0.5 };

    let final_amplitude = amplitude * height_factor;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Use world coordinates for deterministic noise at boundaries
            let world_fx = region.world_x as f64 + fx as f64;
            let world_fy = region.world_y as f64 + fy as f64;

            // Domain warping using world coordinates
            let warp_x = noise2.get([world_fx * 2.0, world_fy * 2.0, seed as f64 * 0.003]) as f32 * 0.3;
            let warp_y = noise2.get([world_fx * 2.0 + 100.0, world_fy * 2.0 + 100.0, seed as f64 * 0.003]) as f32 * 0.3;

            let nx = (world_fx + warp_x as f64) * 8.0;
            let ny = (world_fy + warp_y as f64) * 8.0;

            let detail = fbm_noise(noise, nx, ny, seed as f64 * 0.001, octaves, persistence, 2.0) as f32;

            // Edge fade for detail only - keeps base terrain intact at edges
            // This ensures the world heightmap interpolation dominates at boundaries
            let edge_fade = smooth_edge_fade(fx, fy, 0.15);

            let current = region.get_height(x, y);
            let delta = detail * final_amplitude * edge_fade;
            region.set_height(x, y, current + delta);
        }
    }
}

fn fbm_noise(noise: &Perlin, x: f64, y: f64, z: f64, octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += amplitude * noise.get([x * frequency, y * frequency, z]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

fn trace_rivers_from_world(region: &mut RegionMap, river_network: &crate::erosion::RiverNetwork, world_x: usize, world_y: usize) {
    let size = region.size;

    for segment in &river_network.segments {
        let samples = 50;
        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let pt = segment.evaluate(t);

            let rel_x = pt.world_x - world_x as f32;
            let rel_y = pt.world_y - world_y as f32;

            if rel_x >= -0.1 && rel_x <= 1.1 && rel_y >= -0.1 && rel_y <= 1.1 {
                let rx = (rel_x * size as f32).clamp(0.0, (size - 1) as f32);
                let ry = (rel_y * size as f32).clamp(0.0, (size - 1) as f32);
                let width = (pt.width * 1.5).clamp(1.0, 10.0);
                draw_river_point(region, rx, ry, width);
            }
        }
    }
}

fn draw_river_point(region: &mut RegionMap, x: f32, y: f32, width: f32) {
    let size = region.size;
    let radius = (width / 2.0).max(1.0);
    let r_int = (radius + 1.0).ceil() as i32;

    let cx = x.round() as i32;
    let cy = y.round() as i32;

    for dy in -r_int..=r_int {
        for dx in -r_int..=r_int {
            let px = cx + dx;
            let py = cy + dy;

            if px >= 0 && px < size as i32 && py >= 0 && py < size as i32 {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist <= radius {
                    let intensity = 1.0 - (dist / radius).powi(2);
                    let current = region.get_river(px as usize, py as usize);
                    region.set_river(px as usize, py as usize, current.max(intensity));
                }
            }
        }
    }
}

/// Generate simple drainage based on terrain flow
/// World river network handles cross-tile continuity; this adds local streams
fn generate_simple_drainage(
    region: &mut RegionMap,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    world_flow_acc: f32,
    water_table: f32,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;
    let moisture = *world.moisture.get(world_x, world_y);
    let base_height = *world.heightmap.get(world_x, world_y);

    // Skip if underwater or very dry
    if base_height < 0.0 || moisture < 0.2 {
        return;
    }

    // Higher threshold = fewer local streams (world network handles major rivers)
    let base_threshold = 80.0 / (moisture + 0.2);
    let flow_boost = (world_flow_acc.log10() / 3.0).clamp(0.0, 1.0);
    let adjusted_threshold = base_threshold * (1.0 - flow_boost * 0.2);

    // Compute flow direction (D8)
    let mut flow_dir: Vec<u8> = vec![255; size * size];
    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
    let dist: [f32; 8] = [1.0, 1.414, 1.0, 1.414, 1.0, 1.414, 1.0, 1.414];

    for y in 0..size {
        for x in 0..size {
            let h = region.get_height(x, y);
            if h < 0.0 { continue; }

            let mut best_drop = 0.0f32;
            let mut best_dir: u8 = 255;

            for dir in 0..8u8 {
                let nx = x as i32 + dx[dir as usize];
                let ny = y as i32 + dy[dir as usize];

                if nx < 0 || nx >= size as i32 || ny < 0 || ny >= size as i32 {
                    continue;
                }

                let nh = region.get_height(nx as usize, ny as usize);
                let drop = h - nh;
                let slope = drop / dist[dir as usize];

                if slope > best_drop {
                    best_drop = slope;
                    best_dir = dir;
                }
            }

            flow_dir[y * size + x] = best_dir;
        }
    }

    // Compute flow accumulation
    let mut flow_acc: Vec<f32> = vec![1.0; size * size];
    let mut cells: Vec<(usize, usize, f32)> = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            cells.push((x, y, region.get_height(x, y)));
        }
    }
    cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    for (x, y, _) in &cells {
        let idx = y * size + x;
        let dir = flow_dir[idx];

        if dir < 8 {
            let nx = (*x as i32 + dx[dir as usize]) as usize;
            let ny = (*y as i32 + dy[dir as usize]) as usize;

            if nx < size && ny < size {
                flow_acc[ny * size + nx] += flow_acc[idx];
            }
        }
    }

    // Draw streams only in interior (world network handles edges)
    let margin = 3;
    for y in margin..(size - margin) {
        for x in margin..(size - margin) {
            let acc = flow_acc[y * size + x];
            let h = region.get_height(x, y);

            if h < 1.0 { continue; }

            // Only draw if high flow accumulation AND connects to existing water
            if acc > adjusted_threshold {
                // Check if this stream connects to an existing river (from world network)
                let connects_to_river = check_connects_to_river(region, &flow_dir, x, y, size, &dx, &dy);

                if connects_to_river || acc > adjusted_threshold * 2.0 {
                    let intensity = ((acc - adjusted_threshold) / (adjusted_threshold * 3.0)).clamp(0.2, 0.8);
                    let width = (acc / 100.0).sqrt().clamp(0.5, 2.5);
                    draw_river_point(region, x as f32, y as f32, width * intensity);
                }
            }
        }
    }
}

/// Check if a point eventually flows to an existing river
fn check_connects_to_river(
    region: &RegionMap,
    flow_dir: &[u8],
    start_x: usize,
    start_y: usize,
    size: usize,
    dx: &[i32; 8],
    dy: &[i32; 8],
) -> bool {
    let mut x = start_x;
    let mut y = start_y;

    for _ in 0..20 {
        // Check if current position has river from world network
        if region.get_river(x, y) > 0.3 {
            return true;
        }

        let idx = y * size + x;
        let dir = flow_dir[idx];
        if dir >= 8 { break; }

        let nx = (x as i32 + dx[dir as usize]).clamp(0, size as i32 - 1) as usize;
        let ny = (y as i32 + dy[dir as usize]).clamp(0, size as i32 - 1) as usize;

        if nx == x && ny == y { break; }
        x = nx;
        y = ny;
    }

    false
}
fn generate_vegetation(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    moisture: f32,
    temperature: f32,
    vegetation_density: f32,
    vegetation_pattern: super::handshake::VegetationPattern,
    noise: &Perlin,
    seed: u64,
) {
    use super::handshake::VegetationPattern;
    let size = region.size;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            let nx1 = (region.world_x as f64 + fx as f64) * 6.0;
            let ny1 = (region.world_y as f64 + fy as f64) * 6.0;
            let nx2 = (region.world_x as f64 + fx as f64) * 15.0;
            let ny2 = (region.world_y as f64 + fy as f64) * 15.0;

            let large_noise = noise.get([nx1, ny1, seed as f64 * 0.002]) as f32;
            let small_noise = noise.get([nx2, ny2, seed as f64 * 0.005]) as f32;

            let noise_value = match vegetation_pattern {
                VegetationPattern::Uniform => small_noise * 0.3,
                VegetationPattern::Clumped => {
                    let clump = large_noise * 0.7 + small_noise * 0.3;
                    if clump > 0.0 { clump * 0.8 } else { clump * 0.3 - 0.2 }
                }
                VegetationPattern::Gallery => {
                    let river = region.get_river(x, y);
                    if river > 0.1 { 0.4 + small_noise * 0.2 } else { large_noise * 0.2 - 0.3 }
                }
                VegetationPattern::Sparse => {
                    let sparse = large_noise * 0.6 + small_noise * 0.4;
                    if sparse > 0.3 { sparse * 0.5 } else { -0.3 }
                }
                VegetationPattern::Dense => {
                    let dense = large_noise * 0.3 + small_noise * 0.2;
                    0.3 + dense.max(-0.2)
                }
            };

            let river = region.get_river(x, y);
            let river_bonus = if river > 0.1 { 0.15 } else { 0.0 };
            let slope = region.get_slope(x, y);
            let slope_penalty = (slope / 50.0).min(0.5);
            let height = region.get_height(x, y);

            let altitude_factor = if height > 2000.0 { 0.2 }
                else if height > 1000.0 { 0.6 }
                else if height > 0.0 { 1.0 }
                else { 0.0 };

            let temp_factor = if temperature < -10.0 { 0.2 }
                else if temperature < 0.0 { 0.5 }
                else { 1.0 };

            let density = (vegetation_density + noise_value * 0.4 + river_bonus - slope_penalty)
                * moisture.max(0.3) * altitude_factor * temp_factor;

            region.set_vegetation(x, y, density.clamp(0.0, 1.0));
        }
    }
}

fn generate_rocks(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    surface_minerals: f32,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;

    let rock_bias = match biome {
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks => 0.4,
        ExtendedBiome::Desert | ExtendedBiome::VolcanicWasteland => 0.3,
        ExtendedBiome::Tundra => 0.2,
        ExtendedBiome::Foothills => 0.15,
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest => 0.05,
        _ => 0.0,
    };

    let mineral_bonus = surface_minerals * 0.3;

    for y in 0..size {
        for x in 0..size {
            let slope = region.get_slope(x, y);
            let height = region.get_height(x, y);
            let slope_rocks = (slope / 30.0).clamp(0.0, 1.0);

            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;
            let nx = (region.world_x as f64 + fx as f64) * 12.0;
            let ny = (region.world_y as f64 + fy as f64) * 12.0;
            let rock_noise = (noise.get([nx, ny, seed as f64 * 0.007]) as f32 + 1.0) * 0.5;

            let nx2 = (region.world_x as f64 + fx as f64) * 20.0;
            let ny2 = (region.world_y as f64 + fy as f64) * 20.0;
            let mineral_noise = (noise.get([nx2, ny2, seed as f64 * 0.011]) as f32 + 1.0) * 0.5;
            let mineral_outcrop = if mineral_noise > 0.7 { mineral_bonus * 1.5 } else { mineral_bonus * 0.5 };

            let water_factor = if height < 0.0 || region.get_river(x, y) > 0.2 { 0.0 } else { 1.0 };

            let rock_value = (slope_rocks * 0.5 + rock_noise * 0.3 + rock_bias + mineral_outcrop) * water_factor;
            region.set_rocks(x, y, rock_value.clamp(0.0, 1.0));
        }
    }
}

fn generate_local_biomes(region: &mut RegionMap, world: &WorldData, world_x: usize, world_y: usize) {
    let size = region.size;
    let world_width = world.width;
    let world_height = world.height;

    // Sample 3x3 grid of neighboring biomes for edge blending
    let mut neighbor_biomes = [[ExtendedBiome::Ocean; 3]; 3];
    for sy in 0..3 {
        for sx in 0..3 {
            let wx = (world_x as i32 + sx as i32 - 1).rem_euclid(world_width as i32) as usize;
            let wy = (world_y as i32 + sy as i32 - 1).clamp(0, world_height as i32 - 1) as usize;
            neighbor_biomes[sy][sx] = *world.biomes.get(wx, wy);
        }
    }

    let base_biome = neighbor_biomes[1][1]; // Center tile

    // Edge blend width as fraction of region size (blend in outer ~20%)
    let blend_margin = 0.20;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            let height = region.get_height(x, y);
            let river = region.get_river(x, y);

            // Handle water biomes first
            let local_biome = if river > 0.5 {
                if height < 0.0 { ExtendedBiome::CoastalWater } else { base_biome }
            } else if height < -50.0 {
                ExtendedBiome::Ocean
            } else if height < 0.0 {
                ExtendedBiome::CoastalWater
            } else {
                // Calculate blend weights for edge transitions
                let blend_biome = calculate_edge_biome_blend(
                    fx, fy, blend_margin,
                    &neighbor_biomes,
                    base_biome,
                    region.world_x, region.world_y,
                    x, y,
                );
                blend_biome
            };

            region.set_biome(x, y, local_biome);
        }
    }
}

/// Calculate which biome to use based on position and neighboring biomes
/// Uses deterministic noise to create natural-looking transitions
fn calculate_edge_biome_blend(
    fx: f32,
    fy: f32,
    margin: f32,
    neighbors: &[[ExtendedBiome; 3]; 3],
    base_biome: ExtendedBiome,
    world_x: usize,
    world_y: usize,
    local_x: usize,
    local_y: usize,
) -> ExtendedBiome {
    // Check if we're in the blend zone of any edge
    let near_west = fx < margin;
    let near_east = fx > 1.0 - margin;
    let near_north = fy < margin;
    let near_south = fy > 1.0 - margin;

    // If not near any edge, use base biome
    if !near_west && !near_east && !near_north && !near_south {
        return base_biome;
    }

    // Get the neighboring biome for each edge we're near
    let mut candidates: Vec<(ExtendedBiome, f32)> = vec![(base_biome, 1.0)];

    // Calculate blend weights based on distance from edge
    if near_north {
        let north_biome = neighbors[0][1];
        if north_biome != base_biome {
            let weight = 1.0 - (fy / margin); // 1.0 at edge, 0.0 at margin boundary
            candidates.push((north_biome, weight * 0.5));
        }
    }
    if near_south {
        let south_biome = neighbors[2][1];
        if south_biome != base_biome {
            let weight = 1.0 - ((1.0 - fy) / margin);
            candidates.push((south_biome, weight * 0.5));
        }
    }
    if near_west {
        let west_biome = neighbors[1][0];
        if west_biome != base_biome {
            let weight = 1.0 - (fx / margin);
            candidates.push((west_biome, weight * 0.5));
        }
    }
    if near_east {
        let east_biome = neighbors[1][2];
        if east_biome != base_biome {
            let weight = 1.0 - ((1.0 - fx) / margin);
            candidates.push((east_biome, weight * 0.5));
        }
    }

    // Corner blending
    if near_north && near_west {
        let nw_biome = neighbors[0][0];
        if nw_biome != base_biome {
            let weight = (1.0 - (fy / margin)) * (1.0 - (fx / margin));
            candidates.push((nw_biome, weight * 0.3));
        }
    }
    if near_north && near_east {
        let ne_biome = neighbors[0][2];
        if ne_biome != base_biome {
            let weight = (1.0 - (fy / margin)) * (1.0 - ((1.0 - fx) / margin));
            candidates.push((ne_biome, weight * 0.3));
        }
    }
    if near_south && near_west {
        let sw_biome = neighbors[2][0];
        if sw_biome != base_biome {
            let weight = (1.0 - ((1.0 - fy) / margin)) * (1.0 - (fx / margin));
            candidates.push((sw_biome, weight * 0.3));
        }
    }
    if near_south && near_east {
        let se_biome = neighbors[2][2];
        if se_biome != base_biome {
            let weight = (1.0 - ((1.0 - fy) / margin)) * (1.0 - ((1.0 - fx) / margin));
            candidates.push((se_biome, weight * 0.3));
        }
    }

    // If only base biome, return it
    if candidates.len() == 1 {
        return base_biome;
    }

    // Use deterministic selection based on position
    // This creates a natural-looking boundary that's consistent across regions
    let hash = simple_hash(world_x, world_y, local_x, local_y);
    let threshold = (hash as f32) / (u32::MAX as f32);

    // Normalize weights and select
    let total_weight: f32 = candidates.iter().map(|(_, w)| w).sum();
    let mut cumulative = 0.0;

    for (biome, weight) in &candidates {
        cumulative += weight / total_weight;
        if threshold < cumulative {
            return *biome;
        }
    }

    base_biome
}

/// Simple deterministic hash for biome selection
fn simple_hash(world_x: usize, world_y: usize, local_x: usize, local_y: usize) -> u32 {
    let mut h = 2166136261u32;
    h = h.wrapping_mul(16777619) ^ (world_x as u32);
    h = h.wrapping_mul(16777619) ^ (world_y as u32);
    h = h.wrapping_mul(16777619) ^ (local_x as u32);
    h = h.wrapping_mul(16777619) ^ (local_y as u32);
    h
}

fn biome_vegetation_density(biome: ExtendedBiome) -> f32 {
    match biome {
        ExtendedBiome::TropicalRainforest => 1.0,
        ExtendedBiome::TemperateRainforest => 0.95,
        ExtendedBiome::TropicalForest => 0.85,
        ExtendedBiome::TemperateForest => 0.8,
        ExtendedBiome::BorealForest => 0.7,
        ExtendedBiome::CloudForest => 0.9,
        ExtendedBiome::MontaneForest => 0.75,
        ExtendedBiome::SubalpineForest => 0.6,
        ExtendedBiome::Savanna => 0.4,
        ExtendedBiome::TemperateGrassland => 0.35,
        ExtendedBiome::AlpineMeadow => 0.3,
        ExtendedBiome::Paramo => 0.25,
        ExtendedBiome::Tundra | ExtendedBiome::AlpineTundra => 0.15,
        ExtendedBiome::Foothills => 0.35,
        ExtendedBiome::Desert | ExtendedBiome::SaltFlats => 0.05,
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::Ashlands => 0.02,
        ExtendedBiome::Ice | ExtendedBiome::SnowyPeaks => 0.0,
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => 0.7,
        ExtendedBiome::MangroveSaltmarsh => 0.65,
        ExtendedBiome::BioluminescentForest | ExtendedBiome::MushroomForest => 0.8,
        ExtendedBiome::CrystalForest => 0.3,
        ExtendedBiome::DeadForest | ExtendedBiome::PetrifiedForest => 0.1,
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::CoastalWater => 0.0,
        ExtendedBiome::AcidLake | ExtendedBiome::LavaLake | ExtendedBiome::FrozenLake => 0.0,
        _ => 0.3,
    }
}
