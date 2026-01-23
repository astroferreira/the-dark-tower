//! Region map generator
//!
//! Generates detailed 64x64 region maps from world tile data.
//! Samples heights from neighboring world tiles for accurate terrain,
//! traces rivers from the world river network, and adds procedural detail.

use noise::{NoiseFn, Perlin, Seedable};
use crate::world::WorldData;
use crate::biomes::ExtendedBiome;
use crate::erosion::RiverNetwork;
use crate::underground_water::SpringType;

/// Default region size in tiles
pub const REGION_SIZE: usize = 64;

/// Spring info for region map rendering
#[derive(Clone, Copy, Debug, Default)]
pub struct RegionSpring {
    /// Spring type (None if no spring)
    pub spring_type: SpringType,
    /// Flow rate (0.0-1.0)
    pub flow_rate: f32,
    /// Temperature modifier (for thermal springs)
    pub temperature_mod: f32,
}

/// Waterfall info for region map rendering
#[derive(Clone, Copy, Debug, Default)]
pub struct RegionWaterfall {
    /// Whether waterfall is present
    pub is_present: bool,
    /// Drop height in meters
    pub drop_height: f32,
    /// Width factor (0.0-1.0)
    pub width: f32,
}

/// A generated region map (64x64 detail for a single world tile)
#[derive(Clone)]
pub struct RegionMap {
    /// Size of the region (typically 64)
    pub size: usize,
    /// Detailed heightmap (size x size)
    pub heightmap: Vec<f32>,
    /// River presence/width at each cell (0 = no river)
    pub rivers: Vec<f32>,
    /// Vegetation density (0-1)
    pub vegetation: Vec<f32>,
    /// Rocky terrain (0-1)
    pub rocks: Vec<f32>,
    /// Local biome at each cell
    pub biomes: Vec<ExtendedBiome>,
    /// Spring locations and types
    pub springs: Vec<RegionSpring>,
    /// Waterfall locations
    pub waterfalls: Vec<RegionWaterfall>,
    /// World tile coordinates this region belongs to
    pub world_x: usize,
    pub world_y: usize,
    /// Min/max heights for normalization
    pub height_min: f32,
    pub height_max: f32,
}

impl RegionMap {
    /// Create a new empty region map
    pub fn new(size: usize, world_x: usize, world_y: usize) -> Self {
        let total = size * size;
        Self {
            size,
            heightmap: vec![0.0; total],
            rivers: vec![0.0; total],
            vegetation: vec![0.0; total],
            rocks: vec![0.0; total],
            biomes: vec![ExtendedBiome::Ocean; total],
            springs: vec![RegionSpring::default(); total],
            waterfalls: vec![RegionWaterfall::default(); total],
            world_x,
            world_y,
            height_min: 0.0,
            height_max: 1.0,
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.size + x
    }

    pub fn get_height(&self, x: usize, y: usize) -> f32 {
        self.heightmap[self.idx(x, y)]
    }

    pub fn set_height(&mut self, x: usize, y: usize, value: f32) {
        let idx = self.idx(x, y);
        self.heightmap[idx] = value;
    }

    pub fn get_river(&self, x: usize, y: usize) -> f32 {
        self.rivers[self.idx(x, y)]
    }

    pub fn set_river(&mut self, x: usize, y: usize, value: f32) {
        let idx = self.idx(x, y);
        self.rivers[idx] = value;
    }

    pub fn get_vegetation(&self, x: usize, y: usize) -> f32 {
        self.vegetation[self.idx(x, y)]
    }

    pub fn set_vegetation(&mut self, x: usize, y: usize, value: f32) {
        let idx = self.idx(x, y);
        self.vegetation[idx] = value;
    }

    pub fn get_rocks(&self, x: usize, y: usize) -> f32 {
        self.rocks[self.idx(x, y)]
    }

    pub fn set_rocks(&mut self, x: usize, y: usize, value: f32) {
        let idx = self.idx(x, y);
        self.rocks[idx] = value;
    }

    pub fn get_biome(&self, x: usize, y: usize) -> ExtendedBiome {
        self.biomes[self.idx(x, y)]
    }

    pub fn set_biome(&mut self, x: usize, y: usize, biome: ExtendedBiome) {
        let idx = self.idx(x, y);
        self.biomes[idx] = biome;
    }

    pub fn get_spring(&self, x: usize, y: usize) -> &RegionSpring {
        &self.springs[self.idx(x, y)]
    }

    pub fn set_spring(&mut self, x: usize, y: usize, spring: RegionSpring) {
        let idx = self.idx(x, y);
        self.springs[idx] = spring;
    }

    pub fn get_waterfall(&self, x: usize, y: usize) -> &RegionWaterfall {
        &self.waterfalls[self.idx(x, y)]
    }

    pub fn set_waterfall(&mut self, x: usize, y: usize, waterfall: RegionWaterfall) {
        let idx = self.idx(x, y);
        self.waterfalls[idx] = waterfall;
    }

    /// Get normalized height (0-1 range based on local min/max)
    pub fn get_height_normalized(&self, x: usize, y: usize) -> f32 {
        let h = self.get_height(x, y);
        let range = (self.height_max - self.height_min).max(1.0);
        ((h - self.height_min) / range).clamp(0.0, 1.0)
    }

    /// Calculate local slope at a point
    pub fn get_slope(&self, x: usize, y: usize) -> f32 {
        let h = self.get_height(x, y);
        let mut max_diff = 0.0f32;

        for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
            let nx = (x as i32 + dx).clamp(0, self.size as i32 - 1) as usize;
            let ny = (y as i32 + dy).clamp(0, self.size as i32 - 1) as usize;
            let nh = self.get_height(nx, ny);
            max_diff = max_diff.max((h - nh).abs());
        }

        max_diff
    }
}

/// Generate a detailed region map for a world tile
pub fn generate_region(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    seed: u64,
) -> RegionMap {
    let size = REGION_SIZE;
    let mut region = RegionMap::new(size, world_x, world_y);

    // Create noise generators
    let noise = Perlin::new(1).set_seed(seed as u32);
    let noise2 = Perlin::new(1).set_seed((seed.wrapping_add(12345)) as u32);
    let noise3 = Perlin::new(1).set_seed((seed.wrapping_add(67890)) as u32);

    // Get base data for this tile
    let base_biome = *world.biomes.get(world_x, world_y);
    let base_height = *world.heightmap.get(world_x, world_y);
    let base_moisture = *world.moisture.get(world_x, world_y);
    let base_temp = *world.temperature.get(world_x, world_y);

    // Get handshake data for this tile (contains pre-computed properties)
    let handshake = world.handshakes.as_ref().map(|h| &h.get(world_x, world_y).tile);

    // Phase 1: Generate terrain by sampling world heightmap with interpolation
    generate_terrain_from_world(world, &mut region, world_x, world_y, &noise, seed);

    // Phase 2: Add fractal detail based on terrain type and handshake roughness
    let roughness = handshake.map(|h| h.roughness).unwrap_or(0.3);
    add_terrain_detail_with_handshake(&mut region, base_biome, base_height, roughness, &noise, &noise2, seed);

    // Phase 3: Trace rivers from world river network (if any pass through)
    if let Some(ref river_network) = world.river_network {
        trace_rivers_from_world(&mut region, river_network, world_x, world_y);
    }

    // Phase 4: Generate local drainage network based on terrain
    // Use flow accumulation from handshake to enhance river generation
    let flow_acc = handshake.map(|h| h.flow_accumulation).unwrap_or(1.0);

    // Use underground water data for water table if available, otherwise use handshake
    let water_table = if let Some(ref uw) = world.underground_water {
        let aq = uw.aquifers.get(world_x, world_y);
        if aq.is_present() {
            // Convert aquifer depth to water table (shallower depth = higher table)
            (1.0 - (aq.depth / 100.0).clamp(0.0, 1.0)) * aq.yield_potential
        } else {
            handshake.map(|h| h.water_table).unwrap_or(0.3)
        }
    } else {
        handshake.map(|h| h.water_table).unwrap_or(0.3)
    };

    generate_drainage_network_enhanced(&mut region, world, world_x, world_y, flow_acc, water_table, &noise3, seed);

    // Phase 4b: Place springs and waterfalls from underground water data
    if world.underground_water.is_some() {
        place_springs_from_world(&mut region, world, world_x, world_y, &noise, seed);
        place_waterfalls_from_world(&mut region, world, world_x, world_y);
    }

    // Phase 5: Generate vegetation patterns using handshake data
    let veg_density = handshake.map(|h| h.vegetation_density).unwrap_or_else(|| biome_vegetation_density(base_biome));
    let veg_pattern = handshake.map(|h| h.vegetation_pattern).unwrap_or(super::handshake::VegetationPattern::Uniform);
    generate_vegetation_with_handshake(&mut region, base_biome, base_moisture, base_temp, veg_density, veg_pattern, &noise, seed);

    // Phase 6: Add rocky outcrops on steep terrain using handshake minerals data
    let surface_minerals = handshake.map(|h| h.surface_minerals).unwrap_or(0.1);
    generate_rocks_with_handshake(&mut region, base_biome, surface_minerals, &noise2, seed);

    // Phase 7: Fill in biomes with local variation
    generate_local_biomes(&mut region, world, world_x, world_y);

    // Calculate height stats for normalization
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

/// Generate terrain by sampling and interpolating from world heightmap
fn generate_terrain_from_world(
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

    // Sample a 3x3 grid of world tiles centered on current tile
    let mut samples = [[0.0f32; 3]; 3];
    for sy in 0..3 {
        for sx in 0..3 {
            let wx = (world_x as i32 + sx as i32 - 1).rem_euclid(world_width as i32) as usize;
            let wy = (world_y as i32 + sy as i32 - 1).clamp(0, world_height as i32 - 1) as usize;
            samples[sy][sx] = *world.heightmap.get(wx, wy);
        }
    }

    // Generate region heightmap using bicubic-like interpolation
    for y in 0..size {
        for x in 0..size {
            // Map region coords to world coords (0-1 maps to center tile)
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Bicubic interpolation weights
            let height = bicubic_sample(&samples, fx, fy);

            // Add medium-scale variation
            let nx = (world_x as f64 + fx as f64) * 3.0;
            let ny = (world_y as f64 + fy as f64) * 3.0;
            let medium_noise = noise.get([nx, ny, seed as f64 * 0.001]) as f32;

            // Scale variation by height (more variation in mountains)
            let variation_scale = if height > 500.0 {
                30.0
            } else if height > 100.0 {
                15.0
            } else if height > 0.0 {
                8.0
            } else {
                2.0 // Less variation underwater
            };

            region.set_height(x, y, height + medium_noise * variation_scale);
        }
    }
}

/// Bicubic-style interpolation from a 3x3 sample grid
fn bicubic_sample(samples: &[[f32; 3]; 3], fx: f32, fy: f32) -> f32 {
    // Map fx,fy (0-1) to sample space (0-2, centered at 1)
    let sx = fx + 0.5; // 0.5 to 1.5
    let sy = fy + 0.5;

    // Bilinear interpolation on the center 2x2
    let x0 = sx.floor() as usize;
    let y0 = sy.floor() as usize;
    let x1 = (x0 + 1).min(2);
    let y1 = (y0 + 1).min(2);

    let tx = sx - x0 as f32;
    let ty = sy - y0 as f32;

    // Smooth interpolation (smoothstep)
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

/// Add fractal detail to terrain based on biome type
fn add_terrain_detail(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    base_height: f32,
    noise: &Perlin,
    noise2: &Perlin,
    seed: u64,
) {
    let size = region.size;

    // Determine detail parameters based on biome
    let (amplitude, octaves, roughness) = match biome {
        // Mountains get lots of detail
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineMeadow => {
            (25.0, 5, 0.6)
        }
        // Forests get moderate detail with bumps
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
        ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest => {
            (12.0, 4, 0.5)
        }
        // Hills and foothills
        ExtendedBiome::Foothills | ExtendedBiome::MontaneForest => {
            (18.0, 4, 0.55)
        }
        // Grasslands are gently rolling
        ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna => {
            (6.0, 3, 0.4)
        }
        // Deserts have dunes
        ExtendedBiome::Desert => {
            (10.0, 3, 0.45)
        }
        // Wetlands are flat with slight variation
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => {
            (3.0, 2, 0.3)
        }
        // Tundra is relatively flat
        ExtendedBiome::Tundra => {
            (5.0, 3, 0.4)
        }
        // Water is mostly flat
        ExtendedBiome::Ocean | ExtendedBiome::DeepOcean | ExtendedBiome::CoastalWater => {
            (2.0, 2, 0.3)
        }
        // Default moderate detail
        _ => (8.0, 3, 0.5)
    };

    // Additional amplitude scaling based on absolute height
    let height_factor = if base_height > 1000.0 {
        1.5
    } else if base_height > 500.0 {
        1.2
    } else if base_height > 0.0 {
        1.0
    } else {
        0.5
    };

    let final_amplitude = amplitude * height_factor;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Multi-octave noise with domain warping
            let warp_x = noise2.get([
                (region.world_x as f64 + fx as f64) * 2.0,
                (region.world_y as f64 + fy as f64) * 2.0,
                seed as f64 * 0.003
            ]) as f32 * 0.3;
            let warp_y = noise2.get([
                (region.world_x as f64 + fx as f64) * 2.0 + 100.0,
                (region.world_y as f64 + fy as f64) * 2.0 + 100.0,
                seed as f64 * 0.003
            ]) as f32 * 0.3;

            let nx = (region.world_x as f64 + (fx + warp_x) as f64) * 8.0;
            let ny = (region.world_y as f64 + (fy + warp_y) as f64) * 8.0;

            let detail = fbm_noise(noise, nx, ny, seed as f64 * 0.001, octaves, roughness, 2.0) as f32;

            // Edge fade to ensure seamless tiling
            let edge_x = 1.0 - (2.0 * fx - 1.0).abs().powf(4.0);
            let edge_y = 1.0 - (2.0 * fy - 1.0).abs().powf(4.0);
            let edge_fade = (edge_x * edge_y).sqrt().clamp(0.0, 1.0);

            let current = region.get_height(x, y);
            let delta = detail * final_amplitude * edge_fade;
            region.set_height(x, y, current + delta);
        }
    }
}

/// Trace rivers from the world river network into the region
fn trace_rivers_from_world(
    region: &mut RegionMap,
    river_network: &RiverNetwork,
    world_x: usize,
    world_y: usize,
) {
    let size = region.size;

    // Check each river segment for intersections with this tile
    for segment in &river_network.segments {
        // Sample along the segment at high resolution
        let samples = 50;
        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let pt = segment.evaluate(t);

            // Check if point is within or near this tile
            let rel_x = pt.world_x - world_x as f32;
            let rel_y = pt.world_y - world_y as f32;

            // Include points slightly outside for smooth edges
            if rel_x >= -0.1 && rel_x <= 1.1 && rel_y >= -0.1 && rel_y <= 1.1 {
                // Map to region coordinates
                let rx = (rel_x * size as f32).clamp(0.0, (size - 1) as f32);
                let ry = (rel_y * size as f32).clamp(0.0, (size - 1) as f32);

                // River width based on flow accumulation
                let width = (pt.width * 1.5).clamp(1.0, 10.0);

                draw_river_point(region, rx, ry, width);
            }
        }
    }
}

/// Draw a river point with smooth falloff
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

/// Generate a local drainage network based on terrain gradient
/// This creates rivers that flow from high to low, using flow accumulation
fn generate_drainage_network(
    region: &mut RegionMap,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;

    // Check if this tile should have water flow based on moisture
    let moisture = *world.moisture.get(world_x, world_y);
    let base_height = *world.heightmap.get(world_x, world_y);

    // Skip if underwater or very dry
    if base_height < -10.0 || moisture < 0.2 {
        return;
    }

    // Step 1: Compute local flow direction (D8 algorithm)
    let mut flow_dir: Vec<u8> = vec![255; size * size]; // 255 = no flow

    for y in 0..size {
        for x in 0..size {
            let h = region.get_height(x, y);

            // Skip underwater cells
            if h < 0.0 {
                continue;
            }

            // Find steepest downhill direction
            let mut best_drop = 0.0f32;
            let mut best_dir: u8 = 255;

            // D8 directions: N, NE, E, SE, S, SW, W, NW
            let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
            let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
            let dist: [f32; 8] = [1.0, 1.414, 1.0, 1.414, 1.0, 1.414, 1.0, 1.414];

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

    // Step 2: Compute flow accumulation
    let mut flow_acc: Vec<f32> = vec![1.0; size * size];

    // Sort cells by height (highest first)
    let mut cells: Vec<(usize, usize, f32)> = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            cells.push((x, y, region.get_height(x, y)));
        }
    }
    cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // D8 offsets
    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

    // Propagate flow accumulation from high to low
    for (x, y, _) in &cells {
        let idx = y * size + x;
        let dir = flow_dir[idx];

        if dir < 8 {
            let nx = (*x as i32 + dx[dir as usize]) as usize;
            let ny = (*y as i32 + dy[dir as usize]) as usize;

            if nx < size && ny < size {
                let n_idx = ny * size + nx;
                flow_acc[n_idx] += flow_acc[idx];
            }
        }
    }

    // Step 3: Draw rivers where flow accumulation exceeds threshold
    // Threshold scales with moisture (wetter = more rivers)
    let river_threshold = 30.0 / (moisture + 0.3);

    for y in 0..size {
        for x in 0..size {
            let idx = y * size + x;
            let acc = flow_acc[idx];
            let h = region.get_height(x, y);

            // Skip underwater
            if h < 0.0 {
                continue;
            }

            if acc > river_threshold {
                // River intensity based on accumulation
                let intensity = ((acc - river_threshold) / (river_threshold * 2.0)).clamp(0.3, 1.0);

                // River width based on accumulation (hydraulic geometry)
                let width = (acc / 50.0).sqrt().clamp(0.5, 4.0);

                // Draw river with width
                draw_river_point(region, x as f32, y as f32, width * intensity);
            }
        }
    }

    // Step 4: Add some noise-based variation to make rivers look more natural
    // Add small tributaries from random high points
    let num_tributaries = ((moisture * 5.0) as usize).clamp(1, 4);

    for i in 0..num_tributaries {
        // Find a random starting point on higher ground
        let noise_x = noise.get([seed as f64 * 0.1, i as f64]) as f32;
        let noise_y = noise.get([seed as f64 * 0.1 + 100.0, i as f64]) as f32;

        let start_x = ((noise_x * 0.5 + 0.5) * size as f32) as usize;
        let start_y = ((noise_y * 0.5 + 0.5) * size as f32) as usize;

        let start_h = region.get_height(start_x.min(size - 1), start_y.min(size - 1));

        // Only start from land above sea level
        if start_h < 20.0 {
            continue;
        }

        // Trace downhill
        let mut x = start_x.min(size - 1);
        let mut y = start_y.min(size - 1);

        for step in 0..60 {
            let idx = y * size + x;
            let dir = flow_dir[idx];

            if dir >= 8 {
                break; // No flow direction
            }

            // Only draw if we're above a certain height
            let h = region.get_height(x, y);
            if h < 0.0 {
                break;
            }

            // Stream gets stronger as it flows
            let stream_strength = (step as f32 / 30.0).clamp(0.2, 0.6);
            let current = region.get_river(x, y);
            region.set_river(x, y, current.max(stream_strength));

            // Move downstream
            let nx = (x as i32 + dx[dir as usize]).clamp(0, size as i32 - 1) as usize;
            let ny = (y as i32 + dy[dir as usize]).clamp(0, size as i32 - 1) as usize;

            // Stop if we hit a major river
            if region.get_river(nx, ny) > 0.7 {
                // Draw connection point
                region.set_river(x, y, region.get_river(x, y).max(0.6));
                break;
            }

            x = nx;
            y = ny;
        }
    }
}

/// Generate vegetation patterns
fn generate_vegetation(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    moisture: f32,
    temperature: f32,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;
    let base_density = biome_vegetation_density(biome);

    for y in 0..size {
        for x in 0..size {
            // Multi-scale noise for natural clumping
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            let nx1 = (region.world_x as f64 + fx as f64) * 6.0;
            let ny1 = (region.world_y as f64 + fy as f64) * 6.0;
            let nx2 = (region.world_x as f64 + fx as f64) * 15.0;
            let ny2 = (region.world_y as f64 + fy as f64) * 15.0;

            // Large-scale variation (clearings, groves)
            let large_noise = noise.get([nx1, ny1, seed as f64 * 0.002]) as f32;
            // Small-scale variation (individual trees)
            let small_noise = noise.get([nx2, ny2, seed as f64 * 0.005]) as f32;

            // Combine noise at different scales
            let noise_value = large_noise * 0.6 + small_noise * 0.4;

            // Vegetation grows more near rivers
            let river = region.get_river(x, y);
            let river_bonus = if river > 0.1 { 0.2 } else { 0.0 };

            // Vegetation grows less on steep slopes
            let slope = region.get_slope(x, y);
            let slope_penalty = (slope / 50.0).min(0.5);

            // Vegetation grows less at high altitudes
            let height = region.get_height(x, y);
            let altitude_factor = if height > 2000.0 {
                0.2
            } else if height > 1000.0 {
                0.6
            } else if height > 0.0 {
                1.0
            } else {
                0.0 // No vegetation underwater
            };

            // Temperature affects vegetation
            let temp_factor = if temperature < -10.0 {
                0.2
            } else if temperature < 0.0 {
                0.5
            } else {
                1.0
            };

            let density = (base_density + noise_value * 0.4 + river_bonus - slope_penalty)
                * moisture.max(0.3)
                * altitude_factor
                * temp_factor;

            region.set_vegetation(x, y, density.clamp(0.0, 1.0));
        }
    }
}

/// Generate rocky outcrops on steep terrain
fn generate_rocks(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;

    // Some biomes are rockier than others
    let rock_bias = match biome {
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks => 0.4,
        ExtendedBiome::Desert | ExtendedBiome::VolcanicWasteland => 0.3,
        ExtendedBiome::Tundra => 0.2,
        ExtendedBiome::Foothills => 0.15,
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest => 0.05,
        _ => 0.0,
    };

    for y in 0..size {
        for x in 0..size {
            let slope = region.get_slope(x, y);
            let height = region.get_height(x, y);

            // Steep slopes have rocks
            let slope_rocks = (slope / 30.0).clamp(0.0, 1.0);

            // Noise for rock placement
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;
            let nx = (region.world_x as f64 + fx as f64) * 12.0;
            let ny = (region.world_y as f64 + fy as f64) * 12.0;
            let rock_noise = (noise.get([nx, ny, seed as f64 * 0.007]) as f32 + 1.0) * 0.5;

            // No rocks underwater or on rivers
            let water_factor = if height < 0.0 || region.get_river(x, y) > 0.2 {
                0.0
            } else {
                1.0
            };

            let rock_value = (slope_rocks * 0.6 + rock_noise * 0.3 + rock_bias) * water_factor;
            region.set_rocks(x, y, rock_value.clamp(0.0, 1.0));
        }
    }
}

/// Fill in local biomes based on world data and local conditions
fn generate_local_biomes(
    region: &mut RegionMap,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
) {
    let size = region.size;
    let base_biome = *world.biomes.get(world_x, world_y);

    for y in 0..size {
        for x in 0..size {
            let height = region.get_height(x, y);
            let river = region.get_river(x, y);

            // Override biome based on local conditions
            let local_biome = if river > 0.5 {
                // Strong river presence
                if height < 0.0 {
                    ExtendedBiome::CoastalWater
                } else {
                    base_biome // Keep base biome near rivers
                }
            } else if height < -50.0 {
                ExtendedBiome::Ocean
            } else if height < 0.0 {
                ExtendedBiome::CoastalWater
            } else {
                base_biome
            };

            region.set_biome(x, y, local_biome);
        }
    }
}

/// Get base vegetation density for a biome
fn biome_vegetation_density(biome: ExtendedBiome) -> f32 {
    match biome {
        // Dense forests
        ExtendedBiome::TropicalRainforest => 1.0,
        ExtendedBiome::TemperateRainforest => 0.95,
        ExtendedBiome::TropicalForest => 0.85,
        ExtendedBiome::TemperateForest => 0.8,
        ExtendedBiome::BorealForest => 0.7,
        ExtendedBiome::CloudForest => 0.9,
        ExtendedBiome::MontaneForest => 0.75,
        ExtendedBiome::SubalpineForest => 0.6,
        // Grasslands
        ExtendedBiome::Savanna => 0.4,
        ExtendedBiome::TemperateGrassland => 0.35,
        ExtendedBiome::AlpineMeadow => 0.3,
        ExtendedBiome::Paramo => 0.25,
        // Low vegetation
        ExtendedBiome::Tundra | ExtendedBiome::AlpineTundra => 0.15,
        ExtendedBiome::Foothills => 0.35,
        // Desert/barren
        ExtendedBiome::Desert | ExtendedBiome::SaltFlats => 0.05,
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::Ashlands => 0.02,
        // Ice
        ExtendedBiome::Ice | ExtendedBiome::SnowyPeaks => 0.0,
        // Wetlands (high vegetation)
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => 0.7,
        ExtendedBiome::MangroveSaltmarsh => 0.65,
        // Fantasy forests
        ExtendedBiome::BioluminescentForest | ExtendedBiome::MushroomForest => 0.8,
        ExtendedBiome::CrystalForest => 0.3,
        ExtendedBiome::DeadForest | ExtendedBiome::PetrifiedForest => 0.1,
        // Water biomes
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::CoastalWater => 0.0,
        ExtendedBiome::AcidLake | ExtendedBiome::LavaLake | ExtendedBiome::FrozenLake => 0.0,
        // Default for others
        _ => 0.3,
    }
}

/// Fractional Brownian Motion noise
fn fbm_noise(
    noise: &Perlin,
    x: f64,
    y: f64,
    z: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
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

// ============================================================================
// Handshake-enhanced generation functions
// These use pre-computed tile properties for more accurate region generation
// ============================================================================

/// Add fractal detail to terrain using handshake roughness data
fn add_terrain_detail_with_handshake(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    base_height: f32,
    roughness: f32,
    noise: &Perlin,
    noise2: &Perlin,
    seed: u64,
) {
    let size = region.size;

    // Base parameters from biome (for octaves and persistence)
    let (base_amplitude, octaves, persistence) = match biome {
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineMeadow => {
            (25.0, 5, 0.6)
        }
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
        ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest => {
            (12.0, 4, 0.5)
        }
        ExtendedBiome::Foothills | ExtendedBiome::MontaneForest => {
            (18.0, 4, 0.55)
        }
        ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna => {
            (6.0, 3, 0.4)
        }
        ExtendedBiome::Desert => {
            (10.0, 3, 0.45)
        }
        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => {
            (3.0, 2, 0.3)
        }
        ExtendedBiome::Tundra => {
            (5.0, 3, 0.4)
        }
        ExtendedBiome::Ocean | ExtendedBiome::DeepOcean | ExtendedBiome::CoastalWater => {
            (2.0, 2, 0.3)
        }
        _ => (8.0, 3, 0.5)
    };

    // Scale amplitude by handshake roughness (0.0-1.0 maps to 0.3x-2.0x)
    let roughness_multiplier = 0.3 + roughness * 1.7;
    let amplitude = base_amplitude * roughness_multiplier;

    // Additional amplitude scaling based on absolute height
    let height_factor = if base_height > 1000.0 {
        1.5
    } else if base_height > 500.0 {
        1.2
    } else if base_height > 0.0 {
        1.0
    } else {
        0.5
    };

    let final_amplitude = amplitude * height_factor;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Multi-octave noise with domain warping
            let warp_x = noise2.get([
                (region.world_x as f64 + fx as f64) * 2.0,
                (region.world_y as f64 + fy as f64) * 2.0,
                seed as f64 * 0.003
            ]) as f32 * 0.3;
            let warp_y = noise2.get([
                (region.world_x as f64 + fx as f64) * 2.0 + 100.0,
                (region.world_y as f64 + fy as f64) * 2.0 + 100.0,
                seed as f64 * 0.003
            ]) as f32 * 0.3;

            let nx = (region.world_x as f64 + (fx + warp_x) as f64) * 8.0;
            let ny = (region.world_y as f64 + (fy + warp_y) as f64) * 8.0;

            let detail = fbm_noise(noise, nx, ny, seed as f64 * 0.001, octaves, persistence, 2.0) as f32;

            // Edge fade to ensure seamless tiling
            let edge_x = 1.0 - (2.0 * fx - 1.0).abs().powf(4.0);
            let edge_y = 1.0 - (2.0 * fy - 1.0).abs().powf(4.0);
            let edge_fade = (edge_x * edge_y).sqrt().clamp(0.0, 1.0);

            let current = region.get_height(x, y);
            let delta = detail * final_amplitude * edge_fade;
            region.set_height(x, y, current + delta);
        }
    }
}

/// Generate drainage network using handshake flow accumulation and water table data
fn generate_drainage_network_enhanced(
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
    if base_height < -10.0 || moisture < 0.15 {
        return;
    }

    // Use handshake flow accumulation to boost river likelihood
    // High world flow = more water flowing through this tile
    let flow_boost = (world_flow_acc.log10() / 3.0).clamp(0.0, 1.0);

    // High water table = more springs and streams
    let water_table_boost = water_table * 0.5;

    // Step 1: Compute local flow direction (D8 algorithm)
    let mut flow_dir: Vec<u8> = vec![255; size * size];

    for y in 0..size {
        for x in 0..size {
            let h = region.get_height(x, y);
            if h < 0.0 {
                continue;
            }

            let mut best_drop = 0.0f32;
            let mut best_dir: u8 = 255;

            let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
            let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
            let dist: [f32; 8] = [1.0, 1.414, 1.0, 1.414, 1.0, 1.414, 1.0, 1.414];

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

    // Step 2: Compute flow accumulation
    let mut flow_acc: Vec<f32> = vec![1.0; size * size];

    let mut cells: Vec<(usize, usize, f32)> = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            cells.push((x, y, region.get_height(x, y)));
        }
    }
    cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

    for (x, y, _) in &cells {
        let idx = y * size + x;
        let dir = flow_dir[idx];

        if dir < 8 {
            let nx = (*x as i32 + dx[dir as usize]) as usize;
            let ny = (*y as i32 + dy[dir as usize]) as usize;

            if nx < size && ny < size {
                let n_idx = ny * size + nx;
                flow_acc[n_idx] += flow_acc[idx];
            }
        }
    }

    // Step 3: Draw rivers with enhanced thresholds
    // Lower threshold when flow accumulation and water table are high
    let base_threshold = 30.0 / (moisture + 0.3);
    let adjusted_threshold = base_threshold * (1.0 - flow_boost * 0.3 - water_table_boost * 0.3);

    for y in 0..size {
        for x in 0..size {
            let idx = y * size + x;
            let acc = flow_acc[idx];
            let h = region.get_height(x, y);

            if h < 0.0 {
                continue;
            }

            if acc > adjusted_threshold {
                let intensity = ((acc - adjusted_threshold) / (adjusted_threshold * 2.0)).clamp(0.3, 1.0);
                let width = (acc / 50.0).sqrt().clamp(0.5, 4.0);
                draw_river_point(region, x as f32, y as f32, width * intensity);
            }
        }
    }

    // Step 4: Add springs from high water table areas
    if water_table > 0.5 {
        let num_springs = ((water_table * 4.0) as usize).clamp(0, 3);
        for i in 0..num_springs {
            let noise_x = noise.get([seed as f64 * 0.15, i as f64 + 50.0]) as f32;
            let noise_y = noise.get([seed as f64 * 0.15 + 200.0, i as f64 + 50.0]) as f32;

            let start_x = ((noise_x * 0.5 + 0.5) * size as f32) as usize;
            let start_y = ((noise_y * 0.5 + 0.5) * size as f32) as usize;

            let start_h = region.get_height(start_x.min(size - 1), start_y.min(size - 1));

            if start_h < 10.0 {
                continue;
            }

            // Trace spring flow downhill
            let mut x = start_x.min(size - 1);
            let mut y = start_y.min(size - 1);

            for step in 0..40 {
                let idx = y * size + x;
                let dir = flow_dir[idx];

                if dir >= 8 {
                    break;
                }

                let h = region.get_height(x, y);
                if h < 0.0 {
                    break;
                }

                let stream_strength = (step as f32 / 20.0).clamp(0.3, 0.7);
                let current = region.get_river(x, y);
                region.set_river(x, y, current.max(stream_strength));

                let nx = (x as i32 + dx[dir as usize]).clamp(0, size as i32 - 1) as usize;
                let ny = (y as i32 + dy[dir as usize]).clamp(0, size as i32 - 1) as usize;

                if region.get_river(nx, ny) > 0.7 {
                    region.set_river(x, y, region.get_river(x, y).max(0.6));
                    break;
                }

                x = nx;
                y = ny;
            }
        }
    }
}

/// Place springs in the region based on world-level underground water data
fn place_springs_from_world(
    region: &mut RegionMap,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;

    // Get spring info for this world tile
    let spring_info = if let Some(ref uw) = world.underground_water {
        *uw.springs.get(world_x, world_y)
    } else {
        return;
    };

    // Skip if no spring at this world tile
    if !spring_info.is_present() {
        return;
    }

    // Determine number of spring points based on flow rate
    // Higher flow = more/larger spring area
    let num_springs = ((spring_info.flow_rate * 5.0) as usize).clamp(1, 4);

    for i in 0..num_springs {
        // Use noise to place springs semi-randomly within the region
        let noise_x = noise.get([seed as f64 * 0.17 + i as f64 * 100.0, world_x as f64]) as f32;
        let noise_y = noise.get([seed as f64 * 0.17 + i as f64 * 100.0 + 300.0, world_y as f64]) as f32;

        // Prefer springs at elevation breaks (use heightmap gradient)
        let base_x = ((noise_x * 0.5 + 0.5) * size as f32) as usize;
        let base_y = ((noise_y * 0.5 + 0.5) * size as f32) as usize;

        // Find best spring location near the noise point (look for elevation breaks)
        let mut best_x = base_x.min(size - 1);
        let mut best_y = base_y.min(size - 1);
        let mut best_gradient = 0.0f32;

        let search_radius = 8;
        for dy in -(search_radius as i32)..=search_radius as i32 {
            for dx in -(search_radius as i32)..=search_radius as i32 {
                let sx = (base_x as i32 + dx).clamp(0, size as i32 - 1) as usize;
                let sy = (base_y as i32 + dy).clamp(0, size as i32 - 1) as usize;

                let h = region.get_height(sx, sy);
                if h < 0.0 {
                    continue; // Skip underwater
                }

                // Calculate local gradient
                let gradient = region.get_slope(sx, sy);
                if gradient > best_gradient && gradient < 50.0 {
                    best_gradient = gradient;
                    best_x = sx;
                    best_y = sy;
                }
            }
        }

        // Place the spring
        let spring = RegionSpring {
            spring_type: spring_info.spring_type,
            flow_rate: spring_info.flow_rate * (0.5 + noise_x.abs() * 0.5),
            temperature_mod: spring_info.temperature_mod,
        };
        region.set_spring(best_x, best_y, spring);

        // Springs create water flow - add to river map and trace downstream
        let initial_flow = spring_info.flow_rate * 0.6;
        region.set_river(best_x, best_y, region.get_river(best_x, best_y).max(initial_flow));

        // Trace spring water downhill for a short distance
        let mut x = best_x;
        let mut y = best_y;
        let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
        let d_y: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

        for step in 0..25 {
            let h = region.get_height(x, y);
            let mut best_dir: Option<u8> = None;
            let mut best_drop = 0.0f32;

            for dir in 0..8u8 {
                let nx = (x as i32 + dx[dir as usize]).clamp(0, size as i32 - 1) as usize;
                let ny = (y as i32 + d_y[dir as usize]).clamp(0, size as i32 - 1) as usize;
                let nh = region.get_height(nx, ny);
                let drop = h - nh;

                if drop > best_drop {
                    best_drop = drop;
                    best_dir = Some(dir);
                }
            }

            if let Some(dir) = best_dir {
                let nx = (x as i32 + dx[dir as usize]).clamp(0, size as i32 - 1) as usize;
                let ny = (y as i32 + d_y[dir as usize]).clamp(0, size as i32 - 1) as usize;

                // Fade flow as we go downstream
                let flow_strength = initial_flow * (1.0 - step as f32 / 30.0);
                region.set_river(nx, ny, region.get_river(nx, ny).max(flow_strength));

                // Stop if we hit an existing stronger river
                if region.get_river(nx, ny) > flow_strength + 0.2 {
                    break;
                }

                x = nx;
                y = ny;
            } else {
                break;
            }
        }
    }
}

/// Place waterfalls in the region based on world-level underground water data
fn place_waterfalls_from_world(
    region: &mut RegionMap,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
) {
    let size = region.size;

    // Get waterfall info for this world tile
    let waterfall_info = if let Some(ref uw) = world.underground_water {
        *uw.waterfalls.get(world_x, world_y)
    } else {
        return;
    };

    // Skip if no waterfall at this world tile
    if !waterfall_info.is_present {
        return;
    }

    // Find the steepest drop along river paths in the region
    let mut best_x = size / 2;
    let mut best_y = size / 2;
    let mut best_drop = 0.0f32;
    let mut best_river = 0.0f32;

    // Scan for the best waterfall location (river + steep drop)
    for y in 1..size-1 {
        for x in 1..size-1 {
            let river = region.get_river(x, y);
            if river < 0.3 {
                continue; // Need river presence
            }

            let h = region.get_height(x, y);
            let slope = region.get_slope(x, y);

            // Look for steep drops
            if slope > best_drop && slope >= 15.0 {
                // Prefer locations with stronger rivers
                let score = slope * (river + 0.5);
                if score > best_drop * (best_river + 0.5) {
                    best_drop = slope;
                    best_river = river;
                    best_x = x;
                    best_y = y;
                }
            }
        }
    }

    // Place the waterfall if we found a suitable location
    if best_drop >= 15.0 {
        // Scale the world-level drop height to region scale
        let region_drop = (waterfall_info.drop_height / 10.0).clamp(best_drop, best_drop * 3.0);

        let waterfall = RegionWaterfall {
            is_present: true,
            drop_height: region_drop,
            width: waterfall_info.width * (best_river + 0.5),
        };
        region.set_waterfall(best_x, best_y, waterfall);

        // Mark adjacent cells as part of the waterfall for wider falls
        if waterfall_info.width > 0.5 {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = (best_x as i32 + dx).clamp(0, size as i32 - 1) as usize;
                    let ny = (best_y as i32 + dy).clamp(0, size as i32 - 1) as usize;

                    if region.get_river(nx, ny) > 0.2 {
                        let adjacent_fall = RegionWaterfall {
                            is_present: true,
                            drop_height: region_drop * 0.7,
                            width: waterfall_info.width * 0.5,
                        };
                        region.set_waterfall(nx, ny, adjacent_fall);
                    }
                }
            }
        }
    }
}

/// Generate vegetation patterns using handshake vegetation data
fn generate_vegetation_with_handshake(
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

            // Pattern-based noise weighting
            let noise_value = match vegetation_pattern {
                VegetationPattern::Uniform => {
                    // Even distribution with small-scale variation
                    small_noise * 0.3
                }
                VegetationPattern::Clumped => {
                    // Large clearings and groves
                    let clump = large_noise * 0.7 + small_noise * 0.3;
                    // Create distinct clumps by thresholding
                    if clump > 0.0 { clump * 0.8 } else { clump * 0.3 - 0.2 }
                }
                VegetationPattern::Gallery => {
                    // Vegetation concentrated near water
                    let river = region.get_river(x, y);
                    if river > 0.1 {
                        0.4 + small_noise * 0.2
                    } else {
                        large_noise * 0.2 - 0.3 // Very sparse away from water
                    }
                }
                VegetationPattern::Sparse => {
                    // Occasional isolated plants
                    let sparse = large_noise * 0.6 + small_noise * 0.4;
                    if sparse > 0.3 { sparse * 0.5 } else { -0.3 }
                }
                VegetationPattern::Dense => {
                    // Mostly covered with small gaps
                    let dense = large_noise * 0.3 + small_noise * 0.2;
                    0.3 + dense.max(-0.2)
                }
            };

            // Vegetation grows more near rivers
            let river = region.get_river(x, y);
            let river_bonus = if river > 0.1 { 0.15 } else { 0.0 };

            // Vegetation grows less on steep slopes
            let slope = region.get_slope(x, y);
            let slope_penalty = (slope / 50.0).min(0.5);

            // Vegetation grows less at high altitudes
            let height = region.get_height(x, y);
            let altitude_factor = if height > 2000.0 {
                0.2
            } else if height > 1000.0 {
                0.6
            } else if height > 0.0 {
                1.0
            } else {
                0.0
            };

            // Temperature affects vegetation
            let temp_factor = if temperature < -10.0 {
                0.2
            } else if temperature < 0.0 {
                0.5
            } else {
                1.0
            };

            // Use handshake vegetation_density as the primary factor
            let density = (vegetation_density + noise_value * 0.4 + river_bonus - slope_penalty)
                * moisture.max(0.3)
                * altitude_factor
                * temp_factor;

            region.set_vegetation(x, y, density.clamp(0.0, 1.0));
        }
    }
}

/// Generate rocky outcrops using handshake surface minerals data
fn generate_rocks_with_handshake(
    region: &mut RegionMap,
    biome: ExtendedBiome,
    surface_minerals: f32,
    noise: &Perlin,
    seed: u64,
) {
    let size = region.size;

    // Biome affects rock visibility (in addition to minerals)
    let rock_bias = match biome {
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks => 0.4,
        ExtendedBiome::Desert | ExtendedBiome::VolcanicWasteland => 0.3,
        ExtendedBiome::Tundra => 0.2,
        ExtendedBiome::Foothills => 0.15,
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest => 0.05,
        _ => 0.0,
    };

    // Surface minerals contribute to rock/ore visibility
    let mineral_bonus = surface_minerals * 0.3;

    for y in 0..size {
        for x in 0..size {
            let slope = region.get_slope(x, y);
            let height = region.get_height(x, y);

            // Steep slopes have rocks
            let slope_rocks = (slope / 30.0).clamp(0.0, 1.0);

            // Noise for rock placement
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;
            let nx = (region.world_x as f64 + fx as f64) * 12.0;
            let ny = (region.world_y as f64 + fy as f64) * 12.0;
            let rock_noise = (noise.get([nx, ny, seed as f64 * 0.007]) as f32 + 1.0) * 0.5;

            // Mineral deposits can appear as rocky outcrops
            let nx2 = (region.world_x as f64 + fx as f64) * 20.0;
            let ny2 = (region.world_y as f64 + fy as f64) * 20.0;
            let mineral_noise = (noise.get([nx2, ny2, seed as f64 * 0.011]) as f32 + 1.0) * 0.5;
            let mineral_outcrop = if mineral_noise > 0.7 { mineral_bonus * 1.5 } else { mineral_bonus * 0.5 };

            // No rocks underwater or on rivers
            let water_factor = if height < 0.0 || region.get_river(x, y) > 0.2 {
                0.0
            } else {
                1.0
            };

            let rock_value = (slope_rocks * 0.5 + rock_noise * 0.3 + rock_bias + mineral_outcrop) * water_factor;
            region.set_rocks(x, y, rock_value.clamp(0.0, 1.0));
        }
    }
}
