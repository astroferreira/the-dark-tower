//! Erosion simulation module
//!
//! Implements three complementary erosion techniques:
//! - **River erosion**: Flow accumulation-based river network carving
//! - **Hydraulic erosion**: Particle-based water droplet simulation for detail
//! - **Glacial erosion**: Shallow Ice Approximation (SIA) for U-shaped valleys and fjords

pub mod geomorphometry;
pub mod glacial;
pub mod gpu;
pub mod hydraulic;
pub mod materials;
pub mod params;
pub mod river_geometry;
pub mod rivers;
pub mod utils;

pub use materials::{RockType, generate_material_map, generate_hardness_map};
pub use params::{ErosionParams, ErosionPreset};
pub use rivers::{RiverErosionParams, RiverWidthStats, ConnectivityStats,
                 measure_river_widths, check_river_connectivity, print_river_validation};
pub use river_geometry::{RiverNetwork, RiverNetworkParams, trace_bezier_rivers};

use crate::tilemap::Tilemap;
use crate::plates::{Plate, PlateId};
use rand_chacha::ChaCha8Rng;

/// Export high-resolution heightmap as PNG for debugging.
fn export_hires_heightmap(heightmap: &Tilemap<f32>, seed: u64) {
    use image::{ImageBuffer, Rgb};

    let width = heightmap.width as u32;
    let height = heightmap.height as u32;

    // Find min/max for normalization
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h < min_h { min_h = h; }
        if h > max_h { max_h = h; }
    }

    // Create image
    let img = ImageBuffer::from_fn(width, height, |x, y| {
        let h = *heightmap.get(x as usize, y as usize);

        if h < 0.0 {
            // Ocean: blue gradient
            let depth_ratio = (h - min_h) / (-min_h).max(1.0);
            let blue = (100.0 + 155.0 * depth_ratio) as u8;
            Rgb([20u8, 50, blue])
        } else {
            // Land: green to brown to white
            let elev_ratio = h / max_h.max(1.0);
            if elev_ratio < 0.3 {
                // Low: green
                Rgb([
                    (50.0 + 100.0 * elev_ratio) as u8,
                    (120.0 + 80.0 * elev_ratio) as u8,
                    50,
                ])
            } else if elev_ratio < 0.7 {
                // Mid: brown
                let t = (elev_ratio - 0.3) / 0.4;
                Rgb([
                    (80.0 + 80.0 * t) as u8,
                    (150.0 - 50.0 * t) as u8,
                    (50.0 + 30.0 * t) as u8,
                ])
            } else {
                // High: gray/white (mountains)
                let t = (elev_ratio - 0.7) / 0.3;
                Rgb([
                    (160.0 + 95.0 * t) as u8,
                    (100.0 + 155.0 * t) as u8,
                    (80.0 + 175.0 * t) as u8,
                ])
            }
        }
    });

    // Save as PNG
    let filename = format!("hires_erosion_{}x{}_{}.png", width, height, seed);
    if img.save(&filename).is_ok() {
        println!("  Saved high-res map: {}", filename);
    }
}

/// Export downsampled heightmap as PNG for comparison (shows FINAL result after filtering).
fn export_downsampled_heightmap(heightmap: &Tilemap<f32>, seed: u64) {
    use image::{ImageBuffer, Rgb};

    let width = heightmap.width as u32;
    let height = heightmap.height as u32;

    // Find min/max for normalization
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h < min_h { min_h = h; }
        if h > max_h { max_h = h; }
    }

    // Create image (same color scheme as hires)
    let img = ImageBuffer::from_fn(width, height, |x, y| {
        let h = *heightmap.get(x as usize, y as usize);

        if h < 0.0 {
            // Ocean: blue gradient
            let depth_ratio = (h - min_h) / (-min_h).max(1.0);
            let blue = (100.0 + 155.0 * depth_ratio) as u8;
            Rgb([20u8, 50, blue])
        } else {
            // Land: green to brown to white
            let elev_ratio = h / max_h.max(1.0);
            if elev_ratio < 0.3 {
                Rgb([
                    (50.0 + 100.0 * elev_ratio) as u8,
                    (120.0 + 80.0 * elev_ratio) as u8,
                    50,
                ])
            } else if elev_ratio < 0.7 {
                let t = (elev_ratio - 0.3) / 0.4;
                Rgb([
                    (80.0 + 80.0 * t) as u8,
                    (150.0 - 50.0 * t) as u8,
                    (50.0 + 30.0 * t) as u8,
                ])
            } else {
                let t = (elev_ratio - 0.7) / 0.3;
                Rgb([
                    (160.0 + 95.0 * t) as u8,
                    (100.0 + 155.0 * t) as u8,
                    (80.0 + 175.0 * t) as u8,
                ])
            }
        }
    });

    // Save as PNG
    let filename = format!("final_erosion_{}x{}_{}.png", width, height, seed);
    if img.save(&filename).is_ok() {
        println!("  Saved final (downsampled) map: {}", filename);
    }
}

/// Scale erosion parameters for higher resolution simulation.
/// Flow thresholds scale by area (factor^2), step counts scale by factor.
fn scale_params_for_resolution(params: &ErosionParams, factor: usize) -> ErosionParams {
    let mut scaled = params.clone();
    let area_scale = (factor * factor) as f32;

    // Scale flow thresholds by area, but use 0.25x multiplier for dense capillary network
    // Lower threshold = more small tributaries visible
    scaled.river_source_min_accumulation *= area_scale * 0.25;

    // Scale max steps for larger map traversal
    scaled.droplet_max_steps *= factor;

    // Keep erosion radius small for sharp channels (max 1 at high res)
    scaled.droplet_erosion_radius = scaled.droplet_erosion_radius.min(1);

    // Don't scale again - we're already at high res
    scaled.simulation_scale = 1;

    // Copy hires params
    scaled.hires_roughness = params.hires_roughness;
    scaled.hires_warp = params.hires_warp;

    scaled
}

/// Statistics from erosion simulation
#[derive(Debug)]
pub struct ErosionStats {
    /// Total material eroded (in height units)
    pub total_eroded: f64,
    /// Total material deposited
    pub total_deposited: f64,
    /// Total number of simulation steps taken (e.g., droplet steps)
    pub steps_taken: u64,
    /// Number of iterations/droplets processed
    pub iterations: usize,
    /// Maximum erosion at any single point
    pub max_erosion: f32,
    /// Maximum deposition at any single point
    pub max_deposition: f32,
    /// Lengths of all rivers found in the network analysis
    pub river_lengths: Vec<usize>,
}

impl Default for ErosionStats {
    fn default() -> Self {
        Self {
            total_eroded: 0.0,
            total_deposited: 0.0,
            steps_taken: 0,
            iterations: 0,
            max_erosion: 0.0,
            max_deposition: 0.0,
            river_lengths: Vec::new(),
        }
    }
}
/// Create a connected dendritic drainage network with proper hierarchy.
/// Enforces strict monotonic decrease along flow paths.
fn carve_river_network(heightmap: &mut Tilemap<f32>, source_threshold: f32) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Direction vectors for D8 flow
    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

    // Compute flow direction and accumulation on filled terrain
    let flow_dir = rivers::compute_flow_direction(heightmap);
    let flow_acc = rivers::compute_flow_accumulation(heightmap, &flow_dir);

    // Find maximum accumulation for normalization
    let mut max_acc: f32 = 1.0;
    for y in 0..height {
        for x in 0..width {
            let acc = *flow_acc.get(x, y);
            if acc > max_acc && *heightmap.get(x, y) >= 0.0 {
                max_acc = acc;
            }
        }
    }

    // Collect all land cells and sort by elevation (highest first)
    // Process from sources downstream to ensure monotonic decrease
    let mut land_cells: Vec<(usize, usize, f32, f32)> = Vec::new(); // (x, y, elevation, accumulation)
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h >= 0.0 {
                let acc = *flow_acc.get(x, y);
                land_cells.push((x, y, h, acc));
            }
        }
    }
    land_cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Track the carved elevation of each cell
    let mut carved_elev: Tilemap<f32> = Tilemap::new_with(width, height, f32::MAX);

    // Ocean cells have elevation at sea level
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) < 0.0 {
                carved_elev.set(x, y, 0.0);
            }
        }
    }

    // Process cells from highest to lowest
    // Each cell's elevation must be higher than its downstream neighbor
    let threshold = source_threshold * 0.5; // River cells

    for (x, y, h, acc) in &land_cells {
        let x = *x;
        let y = *y;
        let h = *h;
        let acc = *acc;

        let dir = *flow_dir.get(x, y);
        if dir >= 8 {
            // No flow direction - keep original elevation
            carved_elev.set(x, y, h);
            continue;
        }

        let nx = (x as i32 + dx[dir as usize]).rem_euclid(width as i32) as usize;
        let ny = (y as i32 + dy[dir as usize]).clamp(0, height as i32 - 1) as usize;

        let downstream_elev = *carved_elev.get(nx, ny);

        if downstream_elev >= f32::MAX {
            // Downstream not yet processed - keep original (will be fixed in next pass)
            carved_elev.set(x, y, h);
            continue;
        }

        // Minimum elevation step based on accumulation (concave profile)
        // High accumulation (downstream) = small step; Low accumulation (upstream) = larger step
        let theta = 0.5;
        let step = 2.0 / acc.powf(theta).max(0.1);

        // Our elevation must be at least step higher than downstream
        let min_elev = downstream_elev + step;

        // For river cells (high accumulation), use a lower elevation (channel)
        let channel_depth = if acc >= threshold {
            (acc / max_acc).powf(0.3) * 50.0 + 10.0
        } else {
            0.0
        };

        // Target elevation: the higher of min_elev or (original - channel_depth)
        let target_elev = min_elev.max(h - channel_depth);

        // Final elevation: at least min_elev, at most original
        let final_elev = target_elev.max(min_elev).min(h);

        carved_elev.set(x, y, final_elev);
    }

    // Apply carved elevations to heightmap
    for y in 0..height {
        for x in 0..width {
            let ce = *carved_elev.get(x, y);
            if ce < f32::MAX && ce >= 0.0 {
                let orig = *heightmap.get(x, y);
                if ce < orig {
                    heightmap.set(x, y, ce);
                }
            }
        }
    }

    // Multiple passes to enforce strict monotonic decrease
    for _ in 0..20 {
        let flow_dir = rivers::compute_flow_direction(heightmap);
        let mut any_changed = false;

        for y in 1..height-1 {
            for x in 0..width {
                let h = *heightmap.get(x, y);
                if h < 0.0 { continue; }

                let dir = *flow_dir.get(x, y);
                if dir >= 8 { continue; }

                let nx = (x as i32 + dx[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy[dir as usize]).clamp(0, height as i32 - 1) as usize;
                let nh = *heightmap.get(nx, ny);

                // Downstream must be strictly lower
                if nh >= 0.0 && nh >= h {
                    let new_nh = h - 0.5;
                    if new_nh > 0.0 {
                        heightmap.set(nx, ny, new_nh);
                        any_changed = true;
                    }
                }
            }
        }

        if !any_changed { break; }
    }
}

/// Breach depressions by carving channels through pit walls.
/// Unlike filling, this preserves the carved river channels while ensuring connectivity.
fn breach_depressions(heightmap: &mut Tilemap<f32>) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Direction vectors for D8 flow
    let dx: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    let dy: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

    // Find and breach all pits iteratively
    let mut changed = true;
    let mut iterations = 0;
    let max_iterations = 1000;

    while changed && iterations < max_iterations {
        changed = false;
        iterations += 1;

        for y in 1..height-1 {
            for x in 0..width {
                let h = *heightmap.get(x, y);

                // Skip ocean cells
                if h < 0.0 {
                    continue;
                }

                // Check if this is a pit (no lower neighbor)
                let mut is_pit = true;
                let mut min_neighbor_h = f32::MAX;
                let mut min_dir = 0;

                for dir in 0..8 {
                    let nx = (x as i32 + dx[dir]).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + dy[dir]).clamp(0, height as i32 - 1) as usize;

                    let nh = *heightmap.get(nx, ny);
                    if nh < h {
                        is_pit = false;
                        break;
                    }
                    if nh < min_neighbor_h {
                        min_neighbor_h = nh;
                        min_dir = dir;
                    }
                }

                if is_pit && min_neighbor_h < f32::MAX {
                    // Breach: lower the lowest neighbor to just below current cell
                    // This carves a path OUT of the pit
                    let nx = (x as i32 + dx[min_dir]).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + dy[min_dir]).clamp(0, height as i32 - 1) as usize;

                    let breach_height = h - 0.01;  // Slightly below pit bottom
                    if breach_height < *heightmap.get(nx, ny) {
                        heightmap.set(nx, ny, breach_height);
                        changed = true;
                    }
                }
            }
        }
    }
}

/// Run the complete erosion simulation pipeline
///
/// If `params.simulation_scale > 1`, erosion runs on an upscaled heightmap
/// for sharper river channels, then downscales back preserving carved features.
pub fn simulate_erosion(
    heightmap: &mut Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    params: &ErosionParams,
    rng: &mut ChaCha8Rng,
    seed: u64,
) -> (ErosionStats, Tilemap<f32>) {
    let factor = params.simulation_scale;

    // If simulation_scale > 1, run erosion at higher resolution
    if factor > 1 {
        return simulate_erosion_hires(heightmap, plate_map, plates, stress_map, temperature, params, rng, seed);
    }

    // Standard resolution erosion
    simulate_erosion_internal(heightmap, plate_map, plates, stress_map, temperature, params, rng, seed)
}

/// High-resolution erosion simulation.
/// Upscales, runs erosion, then downscales with river preservation.
fn simulate_erosion_hires(
    heightmap: &mut Tilemap<f32>,
    _plate_map: &Tilemap<PlateId>,
    _plates: &[Plate],
    _stress_map: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    params: &ErosionParams,
    _rng: &mut ChaCha8Rng,
    seed: u64,
) -> (ErosionStats, Tilemap<f32>) {
    let factor = params.simulation_scale;
    let orig_width = heightmap.width;
    let orig_height = heightmap.height;

    println!("High-resolution erosion: {}x upscale ({} → {}x{})",
             factor,
             format!("{}x{}", orig_width, orig_height),
             orig_width * factor, orig_height * factor);

    // Step 1: Upscale heightmap with "crumple" effect for meandering rivers
    // Uses domain warping + roughness noise that's stronger in flat areas
    let mut hires_heightmap = heightmap.upscale_for_erosion(
        factor,
        params.hires_roughness,  // Terrain roughness for meandering (default 12.0)
        params.hires_warp,       // Domain warping for organic curves (default 0.0)
        seed,
    );

    // Step 1b: FUNNEL BLUR - melts sharp ridges between parallel noise lines
    // Radius 3 on 2048 map gently smooths without losing terrain features
    println!("  Applying pre-erosion blur (radius 3)...");
    hires_heightmap = hires_heightmap.gaussian_blur(3);

    // Step 2: Upscale temperature for glacial erosion
    let hires_temperature = temperature.upscale(factor);

    // Step 3: Scale parameters for higher resolution
    let hires_params = scale_params_for_resolution(params, factor);

    // Step 4: Create hardness map for high-res (constant for clean channels)
    let hires_hardness = Tilemap::new_with(hires_heightmap.width, hires_heightmap.height, 0.3f32);

    println!("  Running river erosion on high-res map...");

    // Step 5: Run river erosion (creates major drainage channels)
    let mut stats = ErosionStats::default();
    if hires_params.enable_rivers {
        let river_params = RiverErosionParams {
            source_min_accumulation: hires_params.river_source_min_accumulation,
            source_min_elevation: hires_params.river_source_min_elevation,
            capacity_factor: hires_params.river_capacity_factor,
            erosion_rate: hires_params.river_erosion_rate,
            deposition_rate: hires_params.river_deposition_rate,
            max_erosion: hires_params.river_max_erosion,
            max_deposition: hires_params.river_max_deposition,
            channel_width: hires_params.river_channel_width,
            passes: 1,
        };
        let river_stats = rivers::erode_rivers(&mut hires_heightmap, &hires_hardness, &river_params);
        stats.total_eroded += river_stats.total_eroded;
        stats.total_deposited += river_stats.total_deposited;
        stats.steps_taken += river_stats.steps_taken;
        stats.iterations += river_stats.iterations;
        stats.max_erosion = stats.max_erosion.max(river_stats.max_erosion);
        stats.max_deposition = stats.max_deposition.max(river_stats.max_deposition);
        stats.river_lengths.extend(river_stats.river_lengths);
    }

    println!("  Running hydraulic erosion on high-res map...");

    // Step 6: Run hydraulic erosion (adds detail)
    if hires_params.enable_hydraulic {
        let hydraulic_stats = if hires_params.use_gpu {
            gpu::simulate_gpu_or_cpu(&mut hires_heightmap, &hires_hardness, &hires_params, seed)
        } else {
            hydraulic::simulate_parallel(&mut hires_heightmap, &hires_hardness, &hires_params, seed)
        };
        stats.total_eroded += hydraulic_stats.total_eroded;
        stats.total_deposited += hydraulic_stats.total_deposited;
        stats.iterations += hydraulic_stats.iterations;
        stats.max_erosion = stats.max_erosion.max(hydraulic_stats.max_erosion);
        stats.max_deposition = stats.max_deposition.max(hydraulic_stats.max_deposition);
    }

    // Step 7: Run glacial erosion
    if hires_params.enable_glacial {
        let glacial_stats = glacial::simulate(&mut hires_heightmap, &hires_temperature, &hires_hardness, &hires_params);
        stats.total_eroded += glacial_stats.total_eroded;
        stats.total_deposited += glacial_stats.total_deposited;
        stats.iterations += glacial_stats.iterations;
        stats.max_erosion = stats.max_erosion.max(glacial_stats.max_erosion);
        stats.max_deposition = stats.max_deposition.max(glacial_stats.max_deposition);
    }

    // Step 8: Post-erosion pit filling and river carving on high-res map
    if hires_params.enable_rivers {
        let filled = rivers::fill_depressions_public(&hires_heightmap);
        for y in 0..hires_heightmap.height {
            for x in 0..hires_heightmap.width {
                hires_heightmap.set(x, y, *filled.get(x, y));
            }
        }
        carve_river_network(&mut hires_heightmap, hires_params.river_source_min_accumulation);

        // Step 8b: Apply meander erosion for more natural river curves
        // Recalculate flow each pass since terrain changes
        println!("  Applying meander erosion...");
        for pass in 0..12 {  // More passes
            let flow_dir = rivers::compute_flow_direction(&hires_heightmap);
            let flow_acc = rivers::compute_flow_accumulation(&hires_heightmap, &flow_dir);
            rivers::apply_meander_erosion(
                &mut hires_heightmap,
                &flow_dir,
                &flow_acc,
                hires_params.river_source_min_accumulation,
                40.0,  // Stronger meander erosion
                seed + pass as u64,
            );
        }

        let refilled = rivers::fill_depressions_public(&hires_heightmap);
        for y in 0..hires_heightmap.height {
            for x in 0..hires_heightmap.width {
                hires_heightmap.set(x, y, *refilled.get(x, y));
            }
        }
    }

    // Step 9: Export high-res map for debugging (optional)
    export_hires_heightmap(&hires_heightmap, seed);

    println!("  Downscaling with variance-based river preservation...");

    // Step 10: Smart downscale preserving rivers via variance detection
    // Higher threshold = only preserve well-defined channels (high variance = deep cut)
    // 15.0 gives best bifurcation ratio (~5.4) with good connectivity (~93%)
    let variance_threshold = 15.0;
    let result = hires_heightmap.downscale_preserve_rivers(factor, variance_threshold);

    // Export post-downsampled result for comparison with high-res
    export_downsampled_heightmap(&result, seed);

    // Copy result back to original heightmap
    for y in 0..orig_height {
        for x in 0..orig_width {
            heightmap.set(x, y, *result.get(x, y));
        }
    }

    // Create hardness map at original resolution
    let hardness = Tilemap::new_with(orig_width, orig_height, 0.3f32);

    // Print validation
    if hires_params.enable_rivers {
        let validation_params = RiverErosionParams {
            source_min_accumulation: params.river_source_min_accumulation,
            ..Default::default()
        };
        rivers::print_river_validation(heightmap, &validation_params);
    }

    // Run geomorphometry analysis if enabled
    if params.enable_analysis {
        let analysis_threshold = 5.0;
        let geo_results = geomorphometry::analyze(heightmap, analysis_threshold);
        geo_results.print_summary();

        let score = geo_results.realism_score();
        println!("Overall Realism Score: {:.1}/100", score);

        if !geo_results.longitudinal_profile.is_empty() {
            println!("\n{}", geomorphometry::plot_profile_ascii(&geo_results.longitudinal_profile, 60, 15));
        }
    }

    (stats, hardness)
}

/// Internal erosion simulation (standard resolution).
fn simulate_erosion_internal(
    heightmap: &mut Tilemap<f32>,
    _plate_map: &Tilemap<PlateId>,
    _plates: &[Plate],
    _stress_map: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    params: &ErosionParams,
    _rng: &mut ChaCha8Rng,
    seed: u64,
) -> (ErosionStats, Tilemap<f32>) {
    let mut stats = ErosionStats::default();

    // Use constant hardness for cleaner river channels (like debug tool)
    // Variable hardness creates too much noise
    let hardness = Tilemap::new_with(heightmap.width, heightmap.height, 0.3f32);

    // Run flow-based river erosion first (carves major drainage channels)
    if params.enable_rivers {
        let river_params = RiverErosionParams {
            source_min_accumulation: params.river_source_min_accumulation,
            source_min_elevation: params.river_source_min_elevation,
            capacity_factor: params.river_capacity_factor,
            erosion_rate: params.river_erosion_rate,
            deposition_rate: params.river_deposition_rate,
            max_erosion: params.river_max_erosion,
            max_deposition: params.river_max_deposition,
            channel_width: params.river_channel_width,
            passes: 1,  // Single pass prevents over-deepening
        };
        let river_stats = rivers::erode_rivers(heightmap, &hardness, &river_params);
        stats.total_eroded += river_stats.total_eroded;
        stats.total_deposited += river_stats.total_deposited;
        stats.steps_taken += river_stats.steps_taken;
        stats.iterations += river_stats.iterations;
        stats.max_erosion = stats.max_erosion.max(river_stats.max_erosion);
        stats.max_deposition = stats.max_deposition.max(river_stats.max_deposition);
        stats.river_lengths.extend(river_stats.river_lengths);
    }

    // Run particle-based hydraulic erosion (adds detail to channels)
    // Uses GPU if available and enabled, otherwise parallel CPU implementation
    if params.enable_hydraulic {
        let hydraulic_stats = if params.use_gpu {
            gpu::simulate_gpu_or_cpu(heightmap, &hardness, params, seed)
        } else {
            hydraulic::simulate_parallel(heightmap, &hardness, params, seed)
        };
        stats.total_eroded += hydraulic_stats.total_eroded;
        stats.total_deposited += hydraulic_stats.total_deposited;
        stats.iterations += hydraulic_stats.iterations;
        stats.max_erosion = stats.max_erosion.max(hydraulic_stats.max_erosion);
        stats.max_deposition = stats.max_deposition.max(hydraulic_stats.max_deposition);
    }

    // Run glacial erosion
    if params.enable_glacial {
        let glacial_stats = glacial::simulate(heightmap, temperature, &hardness, params);
        stats.total_eroded += glacial_stats.total_eroded;
        stats.total_deposited += glacial_stats.total_deposited;
        stats.iterations += glacial_stats.iterations;
        stats.max_erosion = stats.max_erosion.max(glacial_stats.max_erosion);
        stats.max_deposition = stats.max_deposition.max(glacial_stats.max_deposition);
    }

    // Analyze river network connectivity (numerical verification)
    if params.enable_rivers {
        let river_params = RiverErosionParams {
            source_min_accumulation: params.river_source_min_accumulation,
            source_min_elevation: params.river_source_min_elevation,
            // ... other params ...
            ..Default::default() // Use other defaults or properly map all
        };
        // We need to re-create params or better yet, reuse the ones from above if possible.
        // Actually best to just create a struct with the fields we have in ErosionParams
        let river_params_analysis = RiverErosionParams {
            source_min_accumulation: params.river_source_min_accumulation,
            source_min_elevation: params.river_source_min_elevation,
            ..Default::default()
        };
        let network_stats = rivers::analyze_river_network(heightmap, &river_params_analysis);
        println!("River Network Analysis:");
        println!("  Rivers found: {}", network_stats.total_rivers);
        println!("  Connected to ocean: {} ({:.1}%)", 
            network_stats.rivers_reaching_ocean, 
            network_stats.connectivity_ratio * 100.0
        );
        println!("  Ending in pits: {}", network_stats.rivers_ending_in_pit);
        println!("  Mean length: {:.1} pixels", network_stats.mean_length);
        println!("  Max length: {} pixels", network_stats.max_length);
    }

    // Print SIMULATED river stats (what actually happened during erosion)
    if !stats.river_lengths.is_empty() {
        let total_len: usize = stats.river_lengths.iter().sum();
        let count = stats.river_lengths.len();
        let mean = total_len as f32 / count as f32;
        let max = stats.river_lengths.iter().max().unwrap_or(&0);

        println!("Simulated River Stats (Ground Truth):");
        println!("  Rivers traced: {}", count);
        println!("  Mean trace length: {:.1} pixels", mean);
        println!("  Max trace length: {} pixels", *max);

        if mean > 100.0 {
            println!("SUCCESS: Rivers are physically long and connected!");
        } else {
             println!("WARNING: Rivers are physically short. Tracing is stopping early.");
        }
    }

    // POST-EROSION: Fill depressions and carve connected river network
    if params.enable_rivers {
        // Step 1: Fill all depressions first
        let filled = rivers::fill_depressions_public(heightmap);
        for y in 0..heightmap.height {
            for x in 0..heightmap.width {
                heightmap.set(x, y, *filled.get(x, y));
            }
        }

        // Step 2: Carve proper river channels with enforced gradients
        carve_river_network(heightmap, params.river_source_min_accumulation);

        // Step 3: Fill any new pits created by carving
        let refilled = rivers::fill_depressions_public(heightmap);
        for y in 0..heightmap.height {
            for x in 0..heightmap.width {
                heightmap.set(x, y, *refilled.get(x, y));
            }
        }
    }

    // Run geomorphometry analysis (quantitative realism tests) if enabled
    if params.enable_analysis {
        let analysis_threshold = 5.0;
        let geo_results = geomorphometry::analyze(heightmap, analysis_threshold);
        geo_results.print_summary();

        // Print realism score
        let score = geo_results.realism_score();
        println!("Overall Realism Score: {:.1}/100", score);

        // Print longitudinal profile if available
        if !geo_results.longitudinal_profile.is_empty() {
            println!("\n{}", geomorphometry::plot_profile_ascii(&geo_results.longitudinal_profile, 60, 15));
        }
    }

    (stats, hardness)
}
