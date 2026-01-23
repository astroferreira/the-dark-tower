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
pub use rivers::RiverErosionParams;
pub use river_geometry::{RiverNetwork, RiverNetworkParams, trace_bezier_rivers};

use crate::tilemap::Tilemap;
use crate::plates::{Plate, PlateId};
use rand_chacha::ChaCha8Rng;

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
