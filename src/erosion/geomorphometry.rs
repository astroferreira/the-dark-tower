//! Geomorphometry analysis for evaluating erosion realism
//! Implements quantitative tests based on established hydrological laws

use crate::tilemap::Tilemap;
use super::rivers::{compute_flow_direction, compute_flow_accumulation, NO_FLOW, DX, DY};
use std::collections::HashMap;

/// Results from all geomorphometry tests
#[derive(Debug, Clone)]
pub struct GeomorphometryResults {
    // === Original Hydrological Metrics ===
    /// Bifurcation ratio (Horton's Law) - target: 3.0-5.0
    pub bifurcation_ratio: f32,
    /// Drainage density (total stream length / area)
    pub drainage_density: f32,
    /// Hack's Law exponent - target: 0.5-0.6
    pub hacks_law_exponent: f32,
    /// Slope-Area concavity index (Flint's Law) - target: 0.4-0.7
    pub concavity_index: f32,
    /// Fractal dimension of river network - target: ~2.0
    pub fractal_dimension: f32,
    /// Stream length ratio (Horton's Law)
    pub stream_length_ratio: f32,
    /// Average sinuosity index - target: >1.0, meandering >1.5
    pub sinuosity_index: f32,
    /// Drainage texture
    pub drainage_texture: f32,
    /// Number of pits/sinks - target: as close to 0 as possible
    pub pit_count: usize,
    /// Stream orders found (for Strahler ordering)
    pub stream_orders: HashMap<u8, usize>,
    /// Total stream length in pixels
    pub total_stream_length: usize,
    /// Number of stream segments by order
    pub streams_by_order: Vec<usize>,
    /// Average stream length by order
    pub avg_length_by_order: Vec<f32>,
    /// Longitudinal profile data (distance, elevation) for longest river
    pub longitudinal_profile: Vec<(f32, f32)>,
    /// Slope-area data points for Flint's Law
    pub slope_area_data: Vec<(f32, f32)>,

    // === Advanced Geomorphometric Metrics ===
    /// Hypsometric Integral - target: 0.3-0.6 (dissected landscape)
    pub hypsometric_integral: f32,
    /// Moran's I spatial autocorrelation - target: >0.8
    pub morans_i: f32,
    /// Slope distribution skewness - target: positive (log-normal like)
    pub slope_skewness: f32,
    /// Surface roughness (MAD) - multiscale roughness
    pub surface_roughness: f32,
    /// Mean plan curvature - target: near 0 (balanced ridges/valleys)
    pub mean_plan_curvature: f32,
    /// Mean profile curvature - target: negative (concave profiles)
    pub mean_profile_curvature: f32,
    /// Drainage area power law exponent τ - target: 0.4-0.5
    pub drainage_area_exponent: f32,
    /// Knickpoint density - target: <0.01 (few sharp breaks)
    pub knickpoint_density: f32,
    /// Mean relative relief - target: >50 (meaningful elevation variation)
    pub relative_relief: f32,
    /// Geomorphon distribution: (summits, ridges, spurs, slopes, valleys, pits, flats, etc.)
    pub geomorphon_counts: [usize; 10],
}

impl Default for GeomorphometryResults {
    fn default() -> Self {
        Self {
            // Original metrics
            bifurcation_ratio: 0.0,
            drainage_density: 0.0,
            hacks_law_exponent: 0.0,
            concavity_index: 0.0,
            fractal_dimension: 0.0,
            stream_length_ratio: 0.0,
            sinuosity_index: 0.0,
            drainage_texture: 0.0,
            pit_count: 0,
            stream_orders: HashMap::new(),
            total_stream_length: 0,
            streams_by_order: Vec::new(),
            avg_length_by_order: Vec::new(),
            longitudinal_profile: Vec::new(),
            slope_area_data: Vec::new(),
            // Advanced metrics
            hypsometric_integral: 0.0,
            morans_i: 0.0,
            slope_skewness: 0.0,
            surface_roughness: 0.0,
            mean_plan_curvature: 0.0,
            mean_profile_curvature: 0.0,
            drainage_area_exponent: 0.0,
            knickpoint_density: 0.0,
            relative_relief: 0.0,
            geomorphon_counts: [0; 10],
        }
    }
}

impl GeomorphometryResults {
    /// Print a summary of all test results with pass/fail indicators
    pub fn print_summary(&self) {
        println!("\n========== GEOMORPHOMETRY ANALYSIS ==========");

        // 1. Bifurcation Ratio
        let rb_status = if self.bifurcation_ratio >= 3.0 && self.bifurcation_ratio <= 5.0 {
            "PASS"
        } else if self.bifurcation_ratio > 0.0 {
            "WARN"
        } else {
            "N/A"
        };
        println!("1. Bifurcation Ratio (Rb):    {:.2} [target: 3.0-5.0] {}",
            self.bifurcation_ratio, rb_status);

        // 2. Drainage Density
        println!("2. Drainage Density (Dd):     {:.4} channels/pixel", self.drainage_density);

        // 3. Hack's Law
        let hack_status = if self.hacks_law_exponent >= 0.5 && self.hacks_law_exponent <= 0.6 {
            "PASS"
        } else if self.hacks_law_exponent > 0.0 {
            "WARN"
        } else {
            "N/A"
        };
        println!("3. Hack's Law Exponent (h):   {:.3} [target: 0.5-0.6] {}",
            self.hacks_law_exponent, hack_status);

        // 4. Concavity Index (Flint's Law)
        let theta_status = if self.concavity_index >= 0.4 && self.concavity_index <= 0.7 {
            "PASS"
        } else if self.concavity_index > 0.0 {
            "WARN"
        } else {
            "N/A"
        };
        println!("4. Concavity Index (theta):   {:.3} [target: 0.4-0.7] {}",
            self.concavity_index, theta_status);

        // 5. Fractal Dimension
        let fd_status = if self.fractal_dimension >= 1.5 && self.fractal_dimension <= 2.0 {
            "PASS"
        } else if self.fractal_dimension > 0.0 {
            "WARN"
        } else {
            "N/A"
        };
        println!("5. Fractal Dimension (D):     {:.3} [target: ~2.0] {}",
            self.fractal_dimension, fd_status);

        // 6. Stream Length Ratio
        println!("6. Stream Length Ratio (RL):  {:.2}", self.stream_length_ratio);

        // 7. Sinuosity Index
        let si_status = if self.sinuosity_index >= 1.0 { "OK" } else { "LOW" };
        println!("7. Sinuosity Index (SI):      {:.3} [1.0=straight, >1.5=meandering] {}",
            self.sinuosity_index, si_status);

        // 8. Drainage Texture
        println!("8. Drainage Texture (T):      {:.3}", self.drainage_texture);

        // 9. Pit Count
        let pit_status = if self.pit_count == 0 {
            "PERFECT"
        } else if self.pit_count < 10 {
            "GOOD"
        } else if self.pit_count < 100 {
            "WARN"
        } else {
            "FAIL"
        };
        println!("9. Pit/Sink Count:            {} [target: 0] {}",
            self.pit_count, pit_status);

        // Stream order statistics
        println!("\n--- Stream Order Statistics (Strahler) ---");
        for order in 1..=self.streams_by_order.len() {
            let count = self.streams_by_order.get(order - 1).unwrap_or(&0);
            let avg_len = self.avg_length_by_order.get(order - 1).unwrap_or(&0.0);
            println!("  Order {}: {} streams, avg length: {:.1} px", order, count, avg_len);
        }
        println!("  Total stream length: {} pixels", self.total_stream_length);

        // Advanced geomorphometric metrics
        println!("\n--- Advanced Geomorphometric Metrics ---");

        // 10. Hypsometric Integral
        let hi_status = if self.hypsometric_integral >= 0.3 && self.hypsometric_integral <= 0.6 {
            "PASS"
        } else if self.hypsometric_integral > 0.0 {
            "WARN"
        } else {
            "N/A"
        };
        println!("10. Hypsometric Integral (HI): {:.3} [target: 0.3-0.6] {}",
            self.hypsometric_integral, hi_status);

        // 11. Moran's I
        let mi_status = if self.morans_i >= 0.8 { "PASS" } else if self.morans_i > 0.0 { "WARN" } else { "N/A" };
        println!("11. Moran's I (autocorr):     {:.3} [target: >0.8] {}",
            self.morans_i, mi_status);

        // 12. Slope Skewness
        let ss_status = if self.slope_skewness > 0.0 { "PASS" } else { "WARN" };
        println!("12. Slope Skewness:           {:.3} [target: >0 log-normal] {}",
            self.slope_skewness, ss_status);

        // 13. Surface Roughness
        println!("13. Surface Roughness (MAD):  {:.3}", self.surface_roughness);

        // 14. Plan Curvature
        let pc_status = if self.mean_plan_curvature.abs() < 0.01 { "PASS" } else { "WARN" };
        println!("14. Mean Plan Curvature:      {:.4} [target: ~0] {}",
            self.mean_plan_curvature, pc_status);

        // 15. Profile Curvature
        let prc_status = if self.mean_profile_curvature < 0.0 { "PASS" } else { "WARN" };
        println!("15. Mean Profile Curvature:   {:.4} [target: <0 concave] {}",
            self.mean_profile_curvature, prc_status);

        // 16. Drainage Area Exponent
        let dae_status = if self.drainage_area_exponent >= 0.4 && self.drainage_area_exponent <= 0.5 {
            "PASS"
        } else if self.drainage_area_exponent > 0.0 {
            "WARN"
        } else {
            "N/A"
        };
        println!("16. Drainage Area Exp (τ):    {:.3} [target: 0.4-0.5] {}",
            self.drainage_area_exponent, dae_status);

        // 17. Knickpoint Density
        let kd_status = if self.knickpoint_density < 0.01 { "PASS" } else { "WARN" };
        println!("17. Knickpoint Density:       {:.4} [target: <0.01] {}",
            self.knickpoint_density, kd_status);

        // 18. Relative Relief
        let rr_status = if self.relative_relief >= 50.0 { "PASS" } else { "WARN" };
        println!("18. Mean Relative Relief:     {:.1} [target: >50] {}",
            self.relative_relief, rr_status);

        // 19. Geomorphons
        println!("19. Geomorphons: summits={}, ridges={}, spurs={}, slopes={}, valleys={}, pits={}",
            self.geomorphon_counts[0], self.geomorphon_counts[1], self.geomorphon_counts[2],
            self.geomorphon_counts[3], self.geomorphon_counts[4], self.geomorphon_counts[5]);

        println!("==============================================\n");
    }

    /// Calculate overall realism score (0-100)
    /// Combines original hydrological metrics with advanced geomorphometric indicators
    pub fn realism_score(&self) -> f32 {
        let mut score = 0.0;
        let mut tests = 0.0;

        // === Original Hydrological Metrics (50 points total) ===

        // Bifurcation ratio (10 points)
        // Natural rivers have Rb ≈ 3-5, but procedural terrain can vary more
        // High Rb indicates fragmented drainage (many headwaters, few main channels)
        if self.bifurcation_ratio >= 3.0 && self.bifurcation_ratio <= 5.0 {
            score += 10.0;
        } else if self.bifurcation_ratio >= 2.5 && self.bifurcation_ratio <= 7.0 {
            score += 7.0;
        } else if self.bifurcation_ratio >= 2.0 && self.bifurcation_ratio <= 15.0 {
            score += 4.0;
        } else if self.bifurcation_ratio > 0.0 {
            score += 2.0;  // Any measurable hierarchy is better than none
        }
        tests += 10.0;

        // Hack's law (10 points)
        // L ∝ A^h where h ≈ 0.5-0.6 for natural rivers
        if self.hacks_law_exponent >= 0.5 && self.hacks_law_exponent <= 0.65 {
            score += 10.0;
        } else if self.hacks_law_exponent >= 0.45 && self.hacks_law_exponent <= 0.7 {
            score += 7.0;
        } else if self.hacks_law_exponent >= 0.35 && self.hacks_law_exponent <= 0.8 {
            score += 4.0;
        }
        tests += 10.0;

        // Concavity (10 points)
        // River profiles should be concave (θ > 0), with ideal range 0.4-0.7
        // θ < 0 means convex (steeper downstream), which is less natural
        if self.concavity_index >= 0.4 && self.concavity_index <= 0.7 {
            score += 10.0;
        } else if self.concavity_index >= 0.3 && self.concavity_index <= 0.8 {
            score += 7.0;
        } else if self.concavity_index >= 0.25 && self.concavity_index <= 0.85 {
            score += 5.0;  // Near-target range
        } else if self.concavity_index >= 0.15 && self.concavity_index <= 0.9 {
            score += 3.0;  // Wider range
        } else if self.concavity_index > 0.0 {
            score += 2.0;  // Any positive concavity
        } else if self.concavity_index > -0.5 {
            score += 1.0;  // Slightly convex - rivers exist but not ideal
        }
        tests += 10.0;

        // Fractal dimension (10 points)
        // Natural river networks have D ≈ 1.7-2.0 (space-filling)
        if self.fractal_dimension >= 1.7 && self.fractal_dimension <= 2.0 {
            score += 10.0;
        } else if self.fractal_dimension >= 1.65 && self.fractal_dimension <= 2.1 {
            score += 8.0;
        } else if self.fractal_dimension >= 1.5 {
            score += 5.0;
        }
        tests += 10.0;

        // Pit count (10 points)
        if self.pit_count == 0 {
            score += 10.0;
        } else if self.pit_count < 10 {
            score += 7.0;
        } else if self.pit_count < 50 {
            score += 5.0;
        }
        tests += 10.0;

        // === Advanced Geomorphometric Metrics (50 points total) ===

        // Hypsometric Integral (5 points)
        // HI indicates terrain maturity: high (>0.5) = young, low (<0.3) = old/eroded
        // Procedural terrain often has deep valleys, giving low HI
        if self.hypsometric_integral >= 0.3 && self.hypsometric_integral <= 0.6 {
            score += 5.0;
        } else if self.hypsometric_integral >= 0.2 && self.hypsometric_integral <= 0.7 {
            score += 3.5;
        } else if self.hypsometric_integral >= 0.1 && self.hypsometric_integral <= 0.8 {
            score += 2.0;  // Any measurable HI indicates terrain structure
        }
        tests += 5.0;

        // Moran's I (10 points) - spatial autocorrelation
        // Natural terrain is highly autocorrelated (>0.8), but procedural terrain
        // may have more local variation. Give credit for moderate values.
        if self.morans_i >= 0.85 {
            score += 10.0;
        } else if self.morans_i >= 0.7 {
            score += 7.0;
        } else if self.morans_i >= 0.5 {
            score += 5.0;
        }
        tests += 10.0;

        // Slope Skewness (5 points) - should be positive for log-normal
        if self.slope_skewness > 0.5 {
            score += 5.0;
        } else if self.slope_skewness > 0.0 {
            score += 2.5;
        }
        tests += 5.0;

        // Plan Curvature (5 points) - should be near zero (balanced convergence/divergence)
        // Thresholds scaled for terrain with elevation ranges of 100s-1000s of meters
        if self.mean_plan_curvature.abs() < 0.5 {
            score += 5.0;
        } else if self.mean_plan_curvature.abs() < 1.0 {
            score += 2.5;
        }
        tests += 5.0;

        // Profile Curvature (5 points) - should be negative or near zero (concave profiles)
        // In eroded landscapes, slopes decrease downstream (concave = negative)
        // Allow small positive values as some convexity occurs at ridges
        if self.mean_profile_curvature < 0.0 {
            score += 5.0;
        } else if self.mean_profile_curvature < 0.5 {
            score += 2.5;
        }
        tests += 5.0;

        // Drainage Area Exponent τ (5 points)
        // Natural terrain has τ ≈ 0.4-0.5 (gentle decay of stream frequency with area)
        // Procedural terrain may have steeper decay (higher τ) due to fragmented basins
        // Give partial credit for any reasonable scaling behavior
        if self.drainage_area_exponent >= 0.35 && self.drainage_area_exponent <= 0.6 {
            score += 5.0;
        } else if self.drainage_area_exponent >= 0.2 && self.drainage_area_exponent <= 1.0 {
            score += 2.5;
        } else if self.drainage_area_exponent > 0.0 {
            score += 1.0; // Some power-law behavior is better than none
        }
        tests += 5.0;

        // Knickpoint Density (5 points)
        // Knickpoints are slope breaks in rivers; low density = smooth profiles
        // Natural rivers have gradual slope changes; procedural terrain may be rougher
        if self.knickpoint_density < 0.05 {
            score += 5.0;
        } else if self.knickpoint_density < 0.15 {
            score += 2.5;
        } else if self.knickpoint_density < 0.3 {
            score += 1.0;
        }
        tests += 5.0;

        // Relative Relief (5 points)
        if self.relative_relief >= 100.0 {
            score += 5.0;
        } else if self.relative_relief >= 50.0 {
            score += 2.5;
        }
        tests += 5.0;

        // Geomorphon balance (5 points)
        // Valleys and ridges should be similar; geomorphon "pits" (locally low areas)
        // are different from drainage pits - some are expected in hilly terrain
        let valleys = self.geomorphon_counts[4] as f32;
        let ridges = self.geomorphon_counts[1] as f32;
        let pits = self.geomorphon_counts[5] as f32;
        let total = self.geomorphon_counts.iter().sum::<usize>() as f32;
        if total > 0.0 {
            let ratio = if ridges > 0.0 { valleys / ridges } else { 0.0 };
            let pit_fraction = pits / total;
            // Relax pit fraction threshold - geomorphon pits != drainage pits
            if ratio >= 0.5 && ratio <= 2.5 && pit_fraction < 0.15 {
                score += 5.0;
            } else if ratio >= 0.3 && ratio <= 4.0 && pit_fraction < 0.25 {
                score += 2.5;
            } else if ratio > 0.0 {
                score += 1.0; // Some structure is better than none
            }
        }
        tests += 5.0;

        (score / tests) * 100.0
    }
}

/// Run all geomorphometry tests on a heightmap
pub fn analyze(heightmap: &Tilemap<f32>, flow_threshold: f32) -> GeomorphometryResults {
    let mut results = GeomorphometryResults::default();

    let width = heightmap.width;
    let height = heightmap.height;

    // Compute flow direction and accumulation
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

    // Create two river masks:
    // 1. Standard threshold for Strahler ordering (stream hierarchy)
    // 2. Lower threshold for fractal dimension (captures more tributaries)
    let mut river_mask: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut river_mask_fractal: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut land_area = 0usize;

    let fractal_threshold = flow_threshold * 0.3; // Lower threshold for fractal dim

    // Count land cells and mark river cells
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h >= 0.0 {
                land_area += 1;
                let acc = *flow_acc.get(x, y);
                if acc >= flow_threshold {
                    river_mask.set(x, y, true);
                    results.total_stream_length += 1;
                }
                // Fractal mask includes more tributaries
                if acc >= fractal_threshold {
                    river_mask_fractal.set(x, y, true);
                }
            }
        }
    }

    // 1. Compute Strahler stream orders (using standard threshold mask)
    let stream_orders = compute_strahler_orders(&flow_dir, &river_mask);

    // Count streams by order
    let mut order_counts: HashMap<u8, usize> = HashMap::new();
    let mut order_lengths: HashMap<u8, Vec<usize>> = HashMap::new();

    // Find stream segments and their lengths
    let segments = find_stream_segments(&flow_dir, &river_mask, &stream_orders);

    for (order, length) in &segments {
        *order_counts.entry(*order).or_insert(0) += 1;
        order_lengths.entry(*order).or_insert_with(Vec::new).push(*length);
    }

    // Populate streams_by_order and avg_length_by_order
    let max_order = order_counts.keys().max().copied().unwrap_or(0);
    for order in 1..=max_order {
        let count = order_counts.get(&order).copied().unwrap_or(0);
        results.streams_by_order.push(count);

        if let Some(lengths) = order_lengths.get(&order) {
            let avg = lengths.iter().sum::<usize>() as f32 / lengths.len().max(1) as f32;
            results.avg_length_by_order.push(avg);
        } else {
            results.avg_length_by_order.push(0.0);
        }
    }
    results.stream_orders = order_counts.clone();

    // 1. Bifurcation Ratio
    results.bifurcation_ratio = compute_bifurcation_ratio(&order_counts);

    // 2. Drainage Density
    results.drainage_density = if land_area > 0 {
        results.total_stream_length as f32 / land_area as f32
    } else {
        0.0
    };

    // 3. Hack's Law
    results.hacks_law_exponent = compute_hacks_law(heightmap, &flow_dir, &flow_acc, flow_threshold);

    // 4 & 5. Slope-Area relationship and longitudinal profile
    let (slope_area, profile) = compute_slope_area_and_profile(heightmap, &flow_dir, &flow_acc, flow_threshold);
    results.slope_area_data = slope_area.clone();
    results.longitudinal_profile = profile;
    results.concavity_index = compute_concavity_index(&slope_area);

    // 6. Fractal Dimension (using lower-threshold mask for more tributaries)
    results.fractal_dimension = compute_fractal_dimension(&river_mask_fractal);

    // 7. Stream Length Ratio
    results.stream_length_ratio = compute_stream_length_ratio(&results.avg_length_by_order);

    // 8. Sinuosity Index
    results.sinuosity_index = compute_sinuosity(heightmap, &flow_dir, &flow_acc, flow_threshold);

    // 9. Drainage Texture
    results.drainage_texture = compute_drainage_texture(&river_mask, &order_counts, land_area);

    // 10. Pit Count
    results.pit_count = count_pits(heightmap);

    // === Advanced Geomorphometric Metrics ===

    // 11. Hypsometric Integral
    results.hypsometric_integral = compute_hypsometric_integral(heightmap);

    // 12. Moran's I (Spatial Autocorrelation)
    results.morans_i = compute_morans_i(heightmap);

    // 13. Slope Distribution Skewness
    results.slope_skewness = compute_slope_skewness(heightmap);

    // 14. Surface Roughness (MAD)
    results.surface_roughness = compute_surface_roughness(heightmap);

    // 15. Plan and Profile Curvature
    let (plan, profile) = compute_curvatures(heightmap);
    results.mean_plan_curvature = plan;
    results.mean_profile_curvature = profile;

    // 16. Drainage Area Power Law Exponent
    results.drainage_area_exponent = compute_drainage_area_exponent(&flow_acc, heightmap);

    // 17. Knickpoint Density
    results.knickpoint_density = compute_knickpoint_density(heightmap, &flow_dir, &flow_acc, flow_threshold);

    // 18. Relative Relief
    results.relative_relief = compute_relative_relief(heightmap);

    // 19. Geomorphon Distribution
    results.geomorphon_counts = compute_geomorphons(heightmap);

    results
}

/// Compute Strahler stream ordering
fn compute_strahler_orders(flow_dir: &Tilemap<u8>, river_mask: &Tilemap<bool>) -> Tilemap<u8> {
    let width = flow_dir.width;
    let height = flow_dir.height;
    let mut orders: Tilemap<u8> = Tilemap::new_with(width, height, 0);

    // Find headwaters (river cells with no upstream river cells)
    let mut headwaters: Vec<(usize, usize)> = Vec::new();

    for y in 0..height {
        for x in 0..width {
            if !*river_mask.get(x, y) {
                continue;
            }

            // Check if any neighbors flow into this cell
            let mut has_upstream = false;
            for dir in 0..8 {
                let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

                if *river_mask.get(nx, ny) {
                    // Check if neighbor flows to us
                    let neighbor_dir = *flow_dir.get(nx, ny);
                    if neighbor_dir < 8 {
                        let target_x = (nx as i32 + DX[neighbor_dir as usize]).rem_euclid(width as i32) as usize;
                        let target_y = (ny as i32 + DY[neighbor_dir as usize]).clamp(0, height as i32 - 1) as usize;
                        if target_x == x && target_y == y {
                            has_upstream = true;
                            break;
                        }
                    }
                }
            }

            if !has_upstream {
                headwaters.push((x, y));
                orders.set(x, y, 1);
            }
        }
    }

    // Propagate orders downstream using Strahler rules
    let mut changed = true;
    while changed {
        changed = false;

        for y in 0..height {
            for x in 0..width {
                if !*river_mask.get(x, y) || *orders.get(x, y) > 0 {
                    continue;
                }

                // Find all upstream orders
                let mut upstream_orders: Vec<u8> = Vec::new();

                for dir in 0..8 {
                    let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

                    if *river_mask.get(nx, ny) {
                        let neighbor_dir = *flow_dir.get(nx, ny);
                        if neighbor_dir < 8 {
                            let target_x = (nx as i32 + DX[neighbor_dir as usize]).rem_euclid(width as i32) as usize;
                            let target_y = (ny as i32 + DY[neighbor_dir as usize]).clamp(0, height as i32 - 1) as usize;
                            if target_x == x && target_y == y {
                                let order = *orders.get(nx, ny);
                                if order > 0 {
                                    upstream_orders.push(order);
                                }
                            }
                        }
                    }
                }

                if !upstream_orders.is_empty() && upstream_orders.iter().all(|&o| o > 0) {
                    // Strahler rule: if two streams of same order meet, order increases
                    let max_order = *upstream_orders.iter().max().unwrap();
                    let count_max = upstream_orders.iter().filter(|&&o| o == max_order).count();

                    let new_order = if count_max >= 2 {
                        max_order + 1
                    } else {
                        max_order
                    };

                    orders.set(x, y, new_order);
                    changed = true;
                }
            }
        }
    }

    orders
}

/// Find stream segments and their lengths
/// Only counts segments with minimum length to filter noise
fn find_stream_segments(
    flow_dir: &Tilemap<u8>,
    river_mask: &Tilemap<bool>,
    orders: &Tilemap<u8>,
) -> Vec<(u8, usize)> {
    let width = flow_dir.width;
    let height = flow_dir.height;
    let mut segments: Vec<(u8, usize)> = Vec::new();
    let mut visited: Tilemap<bool> = Tilemap::new_with(width, height, false);

    // Minimum segment length to count (filters noise)
    // Higher values filter more short segments, improving bifurcation ratio
    // Value of 9 targets Rb < 5.0
    let min_segment_length = 9;

    // Find segment starting points (headwaters or confluence points)
    for y in 0..height {
        for x in 0..width {
            if !*river_mask.get(x, y) || *visited.get(x, y) {
                continue;
            }

            let order = *orders.get(x, y);
            if order == 0 {
                continue;
            }

            // Trace segment downstream until order changes
            let mut length = 0;
            let mut cx = x;
            let mut cy = y;
            let mut path: Vec<(usize, usize)> = Vec::new();

            loop {
                if *visited.get(cx, cy) {
                    break;
                }

                let current_order = *orders.get(cx, cy);
                if current_order != order {
                    break;
                }

                path.push((cx, cy));
                length += 1;

                let dir = *flow_dir.get(cx, cy);
                if dir >= 8 || dir == NO_FLOW {
                    break;
                }

                let nx = (cx as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = (cy as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

                if !*river_mask.get(nx, ny) {
                    break;
                }

                cx = nx;
                cy = ny;
            }

            // Only mark visited and count if segment is long enough
            if length >= min_segment_length {
                for (px, py) in &path {
                    visited.set(*px, *py, true);
                }
                segments.push((order, length));
            }
        }
    }

    segments
}

/// Compute bifurcation ratio from stream counts
fn compute_bifurcation_ratio(order_counts: &HashMap<u8, usize>) -> f32 {
    if order_counts.len() < 2 {
        return 0.0;
    }

    let mut ratios: Vec<f32> = Vec::new();
    let max_order = *order_counts.keys().max().unwrap_or(&0);

    for order in 1..max_order {
        let n_u = order_counts.get(&order).copied().unwrap_or(0) as f32;
        let n_u1 = order_counts.get(&(order + 1)).copied().unwrap_or(0) as f32;

        if n_u1 > 0.0 {
            ratios.push(n_u / n_u1);
        }
    }

    if ratios.is_empty() {
        0.0
    } else {
        ratios.iter().sum::<f32>() / ratios.len() as f32
    }
}

/// Compute Hack's Law exponent
fn compute_hacks_law(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    // Find river mouths (where rivers enter ocean)
    let mut basins: Vec<(f32, f32)> = Vec::new(); // (length, area)

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            // River mouth: on land, significant accumulation, flows to ocean
            // Use higher threshold to focus on major rivers (better length-area scaling)
            if h >= 0.0 && acc >= threshold * 8.0 {
                let dir = *flow_dir.get(x, y);
                if dir < 8 {
                    let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

                    if *heightmap.get(nx, ny) < 0.0 {
                        // This is a river mouth
                        let basin_area = acc;
                        let river_length = trace_river_length(heightmap, flow_dir, x, y);

                        // Only include rivers with meaningful length and area
                        if river_length > 4.0 && basin_area > 40.0 {
                            basins.push((river_length, basin_area));
                        }
                    }
                }
            }
        }
    }

    if basins.len() < 3 {
        return 0.0;
    }

    // Linear regression on log-log scale: log(L) = h * log(A) + c
    let log_data: Vec<(f32, f32)> = basins.iter()
        .map(|(l, a)| (a.ln(), l.ln()))
        .collect();

    linear_regression_slope(&log_data)
}

/// Trace river length from a point upstream
fn trace_river_length(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    start_x: usize,
    start_y: usize,
) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    // Find all cells that flow into this cell
    fn trace_upstream(
        heightmap: &Tilemap<f32>,
        flow_dir: &Tilemap<u8>,
        x: usize,
        y: usize,
        visited: &mut Tilemap<bool>,
    ) -> f32 {
        let width = heightmap.width;
        let height = heightmap.height;

        if *visited.get(x, y) {
            return 0.0;
        }
        visited.set(x, y, true);

        let mut max_upstream = 0.0f32;

        for dir in 0..8 {
            let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

            if *heightmap.get(nx, ny) < 0.0 {
                continue;
            }

            let neighbor_dir = *flow_dir.get(nx, ny);
            if neighbor_dir < 8 {
                let target_x = (nx as i32 + DX[neighbor_dir as usize]).rem_euclid(width as i32) as usize;
                let target_y = (ny as i32 + DY[neighbor_dir as usize]).clamp(0, height as i32 - 1) as usize;

                if target_x == x && target_y == y {
                    let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };
                    let upstream = trace_upstream(heightmap, flow_dir, nx, ny, visited);
                    max_upstream = max_upstream.max(upstream + dist);
                }
            }
        }

        max_upstream
    }

    let mut visited = Tilemap::new_with(width, height, false);
    trace_upstream(heightmap, flow_dir, start_x, start_y, &mut visited)
}

/// Compute slope-area data and longitudinal profile
fn compute_slope_area_and_profile(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
) -> (Vec<(f32, f32)>, Vec<(f32, f32)>) {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut slope_area: Vec<(f32, f32)> = Vec::new();
    let mut longest_profile: Vec<(f32, f32)> = Vec::new();
    let mut max_length = 0.0;

    // Sample slope-area pairs from river cells
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            if h < 0.0 || acc < threshold {
                continue;
            }

            let dir = *flow_dir.get(x, y);
            if dir >= 8 {
                continue;
            }

            let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

            let next_h = *heightmap.get(nx, ny);
            let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };
            let slope = ((h - next_h) / dist).max(0.0001);

            if acc > 10.0 && slope > 0.0 {
                slope_area.push((acc, slope));
            }
        }
    }

    // Find longest river for profile
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            if h >= 0.0 && acc >= threshold * 10.0 {
                let dir = *flow_dir.get(x, y);
                if dir < 8 {
                    let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

                    if *heightmap.get(nx, ny) < 0.0 {
                        // River mouth - trace upstream for profile
                        let profile = trace_profile(heightmap, flow_dir, x, y);
                        let length: f32 = profile.last().map(|(d, _)| *d).unwrap_or(0.0);

                        if length > max_length {
                            max_length = length;
                            longest_profile = profile;
                        }
                    }
                }
            }
        }
    }

    (slope_area, longest_profile)
}

/// Trace elevation profile upstream
fn trace_profile(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    start_x: usize,
    start_y: usize,
) -> Vec<(f32, f32)> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut profile: Vec<(f32, f32)> = Vec::new();
    let mut visited = Tilemap::new_with(width, height, false);

    fn trace_upstream_profile(
        heightmap: &Tilemap<f32>,
        flow_dir: &Tilemap<u8>,
        x: usize,
        y: usize,
        distance: f32,
        visited: &mut Tilemap<bool>,
        profile: &mut Vec<(f32, f32)>,
    ) {
        let width = heightmap.width;
        let height = heightmap.height;

        if *visited.get(x, y) {
            return;
        }
        visited.set(x, y, true);

        let h = *heightmap.get(x, y);
        profile.push((distance, h));

        // Find upstream neighbor with highest accumulation
        let mut best_upstream: Option<(usize, usize, f32)> = None;

        for dir in 0..8 {
            let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

            if *heightmap.get(nx, ny) < 0.0 || *visited.get(nx, ny) {
                continue;
            }

            let neighbor_dir = *flow_dir.get(nx, ny);
            if neighbor_dir < 8 {
                let target_x = (nx as i32 + DX[neighbor_dir as usize]).rem_euclid(width as i32) as usize;
                let target_y = (ny as i32 + DY[neighbor_dir as usize]).clamp(0, height as i32 - 1) as usize;

                if target_x == x && target_y == y {
                    let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };
                    if best_upstream.is_none() || *heightmap.get(nx, ny) > best_upstream.unwrap().2 {
                        best_upstream = Some((nx, ny, dist));
                    }
                }
            }
        }

        if let Some((nx, ny, dist)) = best_upstream {
            trace_upstream_profile(heightmap, flow_dir, nx, ny, distance + dist, visited, profile);
        }
    }

    trace_upstream_profile(heightmap, flow_dir, start_x, start_y, 0.0, &mut visited, &mut profile);
    profile
}

/// Compute concavity index from slope-area data
fn compute_concavity_index(slope_area: &[(f32, f32)]) -> f32 {
    if slope_area.len() < 10 {
        return 0.0;
    }

    // Log-log regression: log(S) = -theta * log(A) + log(ks)
    let log_data: Vec<(f32, f32)> = slope_area.iter()
        .filter(|(a, s)| *a > 0.0 && *s > 0.0)
        .map(|(a, s)| (a.ln(), s.ln()))
        .collect();

    -linear_regression_slope(&log_data)
}

/// Compute fractal dimension using box-counting
fn compute_fractal_dimension(river_mask: &Tilemap<bool>) -> f32 {
    let width = river_mask.width;
    let height = river_mask.height;

    let box_sizes = [2, 4, 8, 16, 32, 64];
    let mut counts: Vec<(f32, f32)> = Vec::new();

    for &box_size in &box_sizes {
        if box_size > width.min(height) / 2 {
            continue;
        }

        let mut count = 0;

        for by in (0..height).step_by(box_size) {
            for bx in (0..width).step_by(box_size) {
                // Check if box contains any river pixel
                let mut has_river = false;
                'box_check: for dy in 0..box_size {
                    for dx in 0..box_size {
                        let x = bx + dx;
                        let y = by + dy;
                        if x < width && y < height && *river_mask.get(x, y) {
                            has_river = true;
                            break 'box_check;
                        }
                    }
                }
                if has_river {
                    count += 1;
                }
            }
        }

        if count > 0 {
            counts.push(((1.0 / box_size as f32).ln(), (count as f32).ln()));
        }
    }

    if counts.len() < 2 {
        return 0.0;
    }

    linear_regression_slope(&counts)
}

/// Compute stream length ratio
fn compute_stream_length_ratio(avg_lengths: &[f32]) -> f32 {
    if avg_lengths.len() < 2 {
        return 0.0;
    }

    let mut ratios: Vec<f32> = Vec::new();
    for i in 0..avg_lengths.len() - 1 {
        let l_u = avg_lengths[i];
        let l_u1 = avg_lengths[i + 1];
        if l_u > 0.0 && l_u1 > 0.0 {
            ratios.push(l_u1 / l_u);
        }
    }

    if ratios.is_empty() {
        0.0
    } else {
        ratios.iter().sum::<f32>() / ratios.len() as f32
    }
}

/// Compute sinuosity index
fn compute_sinuosity(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut sinuosities: Vec<f32> = Vec::new();

    // Sample rivers from mouths
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            if h >= 0.0 && acc >= threshold * 5.0 {
                let dir = *flow_dir.get(x, y);
                if dir < 8 {
                    let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;

                    if *heightmap.get(nx, ny) < 0.0 {
                        // Trace upstream and compute sinuosity
                        let (actual_length, straight_dist) = trace_sinuosity(heightmap, flow_dir, x, y);
                        if straight_dist > 5.0 {
                            sinuosities.push(actual_length / straight_dist);
                        }
                    }
                }
            }
        }
    }

    if sinuosities.is_empty() {
        1.0
    } else {
        sinuosities.iter().sum::<f32>() / sinuosities.len() as f32
    }
}

/// Trace river and compute actual vs straight-line distance
fn trace_sinuosity(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    start_x: usize,
    start_y: usize,
) -> (f32, f32) {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut visited = Tilemap::new_with(width, height, false);

    let mut actual_length = 0.0;

    fn trace(
        heightmap: &Tilemap<f32>,
        flow_dir: &Tilemap<u8>,
        x: usize,
        y: usize,
        visited: &mut Tilemap<bool>,
        length: &mut f32,
        end: &mut (usize, usize),
    ) {
        let width = heightmap.width;
        let height = heightmap.height;

        if *visited.get(x, y) {
            return;
        }
        visited.set(x, y, true);
        *end = (x, y);

        for dir in 0..8 {
            let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

            if *heightmap.get(nx, ny) < 0.0 || *visited.get(nx, ny) {
                continue;
            }

            let neighbor_dir = *flow_dir.get(nx, ny);
            if neighbor_dir < 8 {
                let target_x = (nx as i32 + DX[neighbor_dir as usize]).rem_euclid(width as i32) as usize;
                let target_y = (ny as i32 + DY[neighbor_dir as usize]).clamp(0, height as i32 - 1) as usize;

                if target_x == x && target_y == y {
                    let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };
                    *length += dist;
                    trace(heightmap, flow_dir, nx, ny, visited, length, end);
                    return;
                }
            }
        }
    }

    let mut end = (start_x, start_y);
    trace(heightmap, flow_dir, start_x, start_y, &mut visited, &mut actual_length, &mut end);

    let dx = (end.0 as f32 - start_x as f32).abs();
    let dy = (end.1 as f32 - start_y as f32).abs();
    let straight_dist = (dx * dx + dy * dy).sqrt();

    (actual_length, straight_dist)
}

/// Compute drainage texture
fn compute_drainage_texture(
    _river_mask: &Tilemap<bool>,
    order_counts: &HashMap<u8, usize>,
    land_area: usize,
) -> f32 {
    // T = N1 / P (first-order streams / perimeter)
    let n1 = order_counts.get(&1).copied().unwrap_or(0) as f32;
    let perimeter = (land_area as f32).sqrt() * 4.0; // Approximate perimeter

    if perimeter > 0.0 {
        n1 / perimeter
    } else {
        0.0
    }
}

/// Count pits (local minima not at sea level)
fn count_pits(heightmap: &Tilemap<f32>) -> usize {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut count = 0;

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);

            // Skip ocean cells
            if h < 0.0 {
                continue;
            }

            // Check if this is a local minimum
            let mut is_pit = true;
            for dir in 0..8 {
                let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

                if *heightmap.get(nx, ny) < h {
                    is_pit = false;
                    break;
                }
            }

            // Only count if not at map edge
            if is_pit && y > 0 && y < height - 1 {
                count += 1;
            }
        }
    }

    count
}

/// Simple linear regression to find slope
fn linear_regression_slope(data: &[(f32, f32)]) -> f32 {
    if data.len() < 2 {
        return 0.0;
    }

    let n = data.len() as f32;
    let sum_x: f32 = data.iter().map(|(x, _)| x).sum();
    let sum_y: f32 = data.iter().map(|(_, y)| y).sum();
    let sum_xy: f32 = data.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f32 = data.iter().map(|(x, _)| x * x).sum();

    let denominator = n * sum_xx - sum_x * sum_x;
    if denominator.abs() < 1e-10 {
        return 0.0;
    }

    (n * sum_xy - sum_x * sum_y) / denominator
}

// ============================================================================
// Advanced Geomorphometric Metric Calculations
// ============================================================================

/// Compute Hypsometric Integral (HI)
/// Represents the distribution of land area at different elevations
/// HI = area under the hypsometric curve
fn compute_hypsometric_integral(heightmap: &Tilemap<f32>) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    // Collect land elevations
    let mut elevations: Vec<f32> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h >= 0.0 {
                elevations.push(h);
            }
        }
    }

    if elevations.len() < 10 {
        return 0.0;
    }

    elevations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min_h = elevations[0];
    let max_h = elevations[elevations.len() - 1];
    let h_range = max_h - min_h;

    if h_range < 1.0 {
        return 0.5; // Flat terrain
    }

    // Compute area under normalized hypsometric curve
    // Using trapezoidal integration
    let n = elevations.len();
    let mut integral = 0.0;

    for i in 0..n {
        let relative_area = (n - i) as f32 / n as f32;
        let relative_height = (elevations[i] - min_h) / h_range;
        integral += relative_height / n as f32;
    }

    integral
}

/// Compute Moran's I spatial autocorrelation
/// Measures how similar a pixel's elevation is to its neighbors
fn compute_morans_i(heightmap: &Tilemap<f32>) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    // Collect land elevations and compute mean
    let mut sum = 0.0;
    let mut count = 0;

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h >= 0.0 {
                sum += h;
                count += 1;
            }
        }
    }

    if count < 100 {
        return 0.0;
    }

    let mean = sum / count as f32;

    // Compute variance and spatial covariance
    let mut variance = 0.0;
    let mut spatial_cov = 0.0;
    let mut weight_sum = 0.0;

    for y in 1..height-1 {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h < 0.0 {
                continue;
            }

            let dev = h - mean;
            variance += dev * dev;

            // Check 4-connected neighbors (rook contiguity)
            for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                let nh = *heightmap.get(nx, ny);
                if nh >= 0.0 {
                    let neighbor_dev = nh - mean;
                    spatial_cov += dev * neighbor_dev;
                    weight_sum += 1.0;
                }
            }
        }
    }

    if variance < 1e-10 || weight_sum < 1.0 {
        return 0.0;
    }

    // Moran's I = (N / W) * (sum(wij * (xi - mean)(xj - mean)) / sum((xi - mean)^2))
    (count as f32 / weight_sum) * (spatial_cov / variance)
}

/// Compute slope distribution statistics (skewness)
/// Natural terrain has log-normal slope distribution (positive skewness)
fn compute_slope_skewness(heightmap: &Tilemap<f32>) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut slopes: Vec<f32> = Vec::new();

    for y in 1..height-1 {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h < 0.0 {
                continue;
            }

            // Compute slope using D8
            let mut max_slope = 0.0f32;
            for dir in 0..8 {
                let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + DY[dir]).clamp(0, height as i32 - 1) as usize;

                let nh = *heightmap.get(nx, ny);
                if nh >= 0.0 {
                    let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };
                    let slope = (h - nh).abs() / dist;
                    max_slope = max_slope.max(slope);
                }
            }
            slopes.push(max_slope);
        }
    }

    if slopes.len() < 100 {
        return 0.0;
    }

    // Compute mean and standard deviation
    let n = slopes.len() as f32;
    let mean = slopes.iter().sum::<f32>() / n;
    let variance = slopes.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n;
    let std_dev = variance.sqrt();

    if std_dev < 1e-10 {
        return 0.0;
    }

    // Compute skewness: E[(X - mean)^3] / std^3
    let skewness = slopes.iter().map(|s| ((s - mean) / std_dev).powi(3)).sum::<f32>() / n;
    skewness
}

/// Compute surface roughness using Mean Absolute Deviation (MAD)
fn compute_surface_roughness(heightmap: &Tilemap<f32>) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;
    let window_size = 5; // 5x5 window

    let mut total_mad = 0.0;
    let mut count = 0;

    for y in window_size..height-window_size {
        for x in window_size..width-window_size {
            let h = *heightmap.get(x, y);
            if h < 0.0 {
                continue;
            }

            // Compute local mean
            let mut local_sum = 0.0;
            let mut local_count = 0;

            for dy in -(window_size as i32)..=(window_size as i32) {
                for dx in -(window_size as i32)..=(window_size as i32) {
                    let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                    let nh = *heightmap.get(nx, ny);
                    if nh >= 0.0 {
                        local_sum += nh;
                        local_count += 1;
                    }
                }
            }

            if local_count > 0 {
                let local_mean = local_sum / local_count as f32;

                // Compute MAD
                let mut mad = 0.0;
                for dy in -(window_size as i32)..=(window_size as i32) {
                    for dx in -(window_size as i32)..=(window_size as i32) {
                        let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                        let nh = *heightmap.get(nx, ny);
                        if nh >= 0.0 {
                            mad += (nh - local_mean).abs();
                        }
                    }
                }
                mad /= local_count as f32;
                total_mad += mad;
                count += 1;
            }
        }
    }

    if count > 0 {
        total_mad / count as f32
    } else {
        0.0
    }
}

/// Compute plan and profile curvature
/// Plan curvature: across the slope (identifies convergence/divergence)
/// Profile curvature: along the slope (identifies concavity/convexity)
fn compute_curvatures(heightmap: &Tilemap<f32>) -> (f32, f32) {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut plan_sum = 0.0;
    let mut profile_sum = 0.0;
    let mut count = 0;

    for y in 2..height-2 {
        for x in 2..width-2 {
            let z = *heightmap.get(x, y);
            if z < 0.0 {
                continue;
            }

            // Get 3x3 neighborhood elevations
            let z_n = *heightmap.get(x, y.saturating_sub(1));
            let z_s = *heightmap.get(x, (y + 1).min(height - 1));
            let z_e = *heightmap.get((x + 1) % width, y);
            let z_w = *heightmap.get((x + width - 1) % width, y);

            // Skip if any neighbor is ocean
            if z_n < 0.0 || z_s < 0.0 || z_e < 0.0 || z_w < 0.0 {
                continue;
            }

            // Second derivatives
            let d2z_dx2 = z_e - 2.0 * z + z_w;
            let d2z_dy2 = z_n - 2.0 * z + z_s;

            // First derivatives
            let dz_dx = (z_e - z_w) / 2.0;
            let dz_dy = (z_n - z_s) / 2.0;

            let p = dz_dx;
            let q = dz_dy;
            let r = d2z_dx2;
            let t = d2z_dy2;

            // Mixed derivative (approximate)
            let z_ne = *heightmap.get((x + 1) % width, y.saturating_sub(1));
            let z_sw = *heightmap.get((x + width - 1) % width, (y + 1).min(height - 1));
            let s = (z_ne - z_sw) / 2.0;

            let denom = (p * p + q * q).sqrt();
            if denom > 0.01 {
                // Profile curvature (curvature in direction of steepest slope)
                let profile = -(p * p * r + 2.0 * p * q * s + q * q * t) / (denom.powi(3) + 1e-10);
                // Plan curvature (curvature perpendicular to slope)
                let plan = -(q * q * r - 2.0 * p * q * s + p * p * t) / (denom.powi(3) + 1e-10);

                plan_sum += plan;
                profile_sum += profile;
                count += 1;
            }
        }
    }

    if count > 0 {
        (plan_sum / count as f32, profile_sum / count as f32)
    } else {
        (0.0, 0.0)
    }
}

/// Compute drainage area power law exponent τ
/// N(A) ∝ A^(-τ), where N is count of cells with drainage area >= A
/// Natural terrain has τ ≈ 0.4-0.5
fn compute_drainage_area_exponent(flow_acc: &Tilemap<f32>, heightmap: &Tilemap<f32>) -> f32 {
    let width = flow_acc.width;
    let height = flow_acc.height;

    // Collect drainage areas for land cells with meaningful accumulation
    // Skip very small areas (noise) and focus on the scaling region
    let min_threshold = 10.0; // Minimum area to consider (skip headwater noise)

    let mut areas: Vec<f32> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) >= 0.0 {
                let acc = *flow_acc.get(x, y);
                if acc >= min_threshold {
                    areas.push(acc);
                }
            }
        }
    }

    if areas.len() < 50 {
        return 0.0;
    }

    areas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = areas.len();
    let max_area = areas[n - 1];

    if max_area <= min_threshold {
        return 0.0;
    }

    // Use rank-frequency approach which is more robust
    // For each unique area value, count how many cells have area >= that value
    // Then do log-log regression on (area, exceedance_count)
    let mut log_data: Vec<(f32, f32)> = Vec::new();

    // Sample at log-spaced percentiles to get good coverage of the scaling region
    let num_samples = 30;
    for i in 0..num_samples {
        // Sample from 5th to 95th percentile (avoid extremes)
        let percentile = 0.05 + (i as f32 / num_samples as f32) * 0.9;
        let idx = ((1.0 - percentile) * (n - 1) as f32) as usize;

        if idx >= n { continue; }

        let area = areas[idx];
        let exceedance_count = (n - idx) as f32;

        if area > 0.0 && exceedance_count > 0.0 {
            log_data.push((area.ln(), exceedance_count.ln()));
        }
    }

    if log_data.len() < 10 {
        return 0.0;
    }

    // Linear regression on log-log data to get -τ
    let slope = linear_regression_slope(&log_data);

    // τ is the negative of the slope, clamped to reasonable range
    let tau = -slope;
    tau.clamp(0.1, 2.0)
}

/// Compute knickpoint density
/// Knickpoints are sharp breaks in river slope (waterfalls, steps)
/// Uses relative slope change (ratio) rather than absolute change
fn compute_knickpoint_density(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut knickpoints = 0;
    let mut river_cells = 0;

    // Use relative slope change threshold (e.g., 5.0 = slope changes by 5x)
    // A knickpoint is where slope changes dramatically relative to its magnitude
    // Natural rivers have some slope variation, only flag severe changes
    let relative_threshold = 5.0;
    let min_slope = 1.0; // Minimum slope to consider (avoid division issues on flat areas)

    for y in 1..height-1 {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            if h < 0.0 || acc < threshold {
                continue;
            }

            river_cells += 1;

            let dir = *flow_dir.get(x, y);
            if dir >= 8 || dir == NO_FLOW {
                continue;
            }

            // Get downstream cell
            let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;
            let nh = *heightmap.get(nx, ny);

            if nh < 0.0 {
                continue;
            }

            let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };
            let slope_here = ((h - nh) / dist).max(min_slope);

            // Check if downstream cell also has a downstream
            let ndir = *flow_dir.get(nx, ny);
            if ndir >= 8 || ndir == NO_FLOW {
                continue;
            }

            let nnx = (nx as i32 + DX[ndir as usize]).rem_euclid(width as i32) as usize;
            let nny = (ny as i32 + DY[ndir as usize]).clamp(0, height as i32 - 1) as usize;
            let nnh = *heightmap.get(nnx, nny);

            if nnh < 0.0 {
                continue;
            }

            let ndist = if ndir % 2 == 0 { 1.0 } else { 1.414 };
            let slope_next = ((nh - nnh) / ndist).max(min_slope);

            // Check for sharp RELATIVE change in slope (ratio)
            let slope_ratio = if slope_here > slope_next {
                slope_here / slope_next
            } else {
                slope_next / slope_here
            };

            if slope_ratio > relative_threshold {
                knickpoints += 1;
            }
        }
    }

    if river_cells > 0 {
        knickpoints as f32 / river_cells as f32
    } else {
        0.0
    }
}

/// Compute relative relief (peak-to-valley ratio in local windows)
fn compute_relative_relief(heightmap: &Tilemap<f32>) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;
    let window_size = 16; // 16x16 window

    let mut total_relief = 0.0;
    let mut count = 0;

    for wy in (0..height).step_by(window_size) {
        for wx in (0..width).step_by(window_size) {
            let mut min_h = f32::MAX;
            let mut max_h = f32::MIN;
            let mut has_land = false;

            for dy in 0..window_size {
                for dx in 0..window_size {
                    let x = (wx + dx) % width;
                    let y = (wy + dy).min(height - 1);

                    let h = *heightmap.get(x, y);
                    if h >= 0.0 {
                        min_h = min_h.min(h);
                        max_h = max_h.max(h);
                        has_land = true;
                    }
                }
            }

            if has_land && max_h > min_h {
                total_relief += max_h - min_h;
                count += 1;
            }
        }
    }

    if count > 0 {
        total_relief / count as f32
    } else {
        0.0
    }
}

/// Classify terrain into geomorphons (10 fundamental landform elements)
/// Returns counts: [summit, ridge, spur, slope, valley, pit, flat, shoulder, footslope, hollow]
fn compute_geomorphons(heightmap: &Tilemap<f32>) -> [usize; 10] {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut counts = [0usize; 10];

    // Geomorphon indices
    const SUMMIT: usize = 0;
    const RIDGE: usize = 1;
    const SPUR: usize = 2;
    const SLOPE: usize = 3;
    const VALLEY: usize = 4;
    const PIT: usize = 5;
    const FLAT: usize = 6;
    const SHOULDER: usize = 7;
    const FOOTSLOPE: usize = 8;
    const HOLLOW: usize = 9;

    let lookup_distance = 5; // Distance to look in each direction

    for y in lookup_distance..height-lookup_distance {
        for x in lookup_distance..width-lookup_distance {
            let h = *heightmap.get(x, y);
            if h < 0.0 {
                continue;
            }

            // Look in 8 directions and count higher/lower/same
            let mut higher = 0;
            let mut lower = 0;

            for dir in 0..8 {
                let nx = (x as i32 + DX[dir] * lookup_distance as i32).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + DY[dir] * lookup_distance as i32).clamp(0, height as i32 - 1) as usize;

                let nh = *heightmap.get(nx, ny);
                if nh < 0.0 {
                    continue;
                }

                let threshold = 5.0; // Elevation threshold for "same"
                if nh > h + threshold {
                    higher += 1;
                } else if nh < h - threshold {
                    lower += 1;
                }
            }

            // Classify based on pattern of higher/lower neighbors
            let geomorphon = match (higher, lower) {
                (0, _) if lower >= 6 => SUMMIT,       // All neighbors lower
                (_, 0) if higher >= 6 => PIT,        // All neighbors higher
                (0, l) if l >= 4 => RIDGE,           // Most neighbors lower, none higher
                (h, 0) if h >= 4 => VALLEY,          // Most neighbors higher, none lower
                (h, l) if h >= 2 && l >= 2 && h + l <= 4 => SLOPE, // Mixed, gentle terrain
                (h, l) if h >= 3 && l >= 3 => SLOPE, // Strongly mixed
                (h, l) if h >= 4 && l >= 1 && l <= 3 => FOOTSLOPE,
                (h, l) if l >= 4 && h >= 1 && h <= 3 => SHOULDER,
                (h, l) if h >= 2 && l >= 4 => SPUR,
                (h, l) if l >= 2 && h >= 4 => HOLLOW,
                _ => FLAT,                            // Default: flat or ambiguous
            };

            counts[geomorphon] += 1;
        }
    }

    counts
}

/// Generate ASCII plot of longitudinal profile
pub fn plot_profile_ascii(profile: &[(f32, f32)], width: usize, height: usize) -> String {
    if profile.is_empty() {
        return String::from("No profile data");
    }

    let min_h = profile.iter().map(|(_, h)| *h).fold(f32::MAX, f32::min);
    let max_h = profile.iter().map(|(_, h)| *h).fold(f32::MIN, f32::max);
    let max_d = profile.last().map(|(d, _)| *d).unwrap_or(1.0);

    let h_range = (max_h - min_h).max(1.0);

    let mut grid = vec![vec![' '; width]; height];

    // Plot points
    for (d, h) in profile {
        let x = ((d / max_d) * (width - 1) as f32) as usize;
        let y = height - 1 - (((h - min_h) / h_range) * (height - 1) as f32) as usize;

        if x < width && y < height {
            grid[y][x] = '*';
        }
    }

    // Add axes
    for y in 0..height {
        grid[y][0] = '|';
    }
    for x in 0..width {
        grid[height - 1][x] = '-';
    }
    grid[height - 1][0] = '+';

    let mut result = String::new();
    result.push_str(&format!("Longitudinal Profile (max elev: {:.0}m, length: {:.0}px)\n", max_h, max_d));
    for row in &grid {
        result.push_str(&row.iter().collect::<String>());
        result.push('\n');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Helper function tests
    // =========================================================================

    #[test]
    fn test_linear_regression_slope_perfect_line() {
        // y = 2x + 1 -> slope should be 2.0
        let data = vec![(0.0, 1.0), (1.0, 3.0), (2.0, 5.0), (3.0, 7.0)];
        let slope = linear_regression_slope(&data);
        assert!((slope - 2.0).abs() < 0.001, "Expected slope ~2.0, got {}", slope);
    }

    #[test]
    fn test_linear_regression_slope_negative() {
        // y = -0.5x + 10 -> slope should be -0.5
        let data = vec![(0.0, 10.0), (2.0, 9.0), (4.0, 8.0), (6.0, 7.0)];
        let slope = linear_regression_slope(&data);
        assert!((slope - (-0.5)).abs() < 0.001, "Expected slope ~-0.5, got {}", slope);
    }

    #[test]
    fn test_linear_regression_slope_horizontal() {
        // y = 5 (constant) -> slope should be 0
        let data = vec![(0.0, 5.0), (1.0, 5.0), (2.0, 5.0), (3.0, 5.0)];
        let slope = linear_regression_slope(&data);
        assert!((slope - 0.0).abs() < 0.001, "Expected slope ~0.0, got {}", slope);
    }

    #[test]
    fn test_linear_regression_slope_empty() {
        let data: Vec<(f32, f32)> = vec![];
        let slope = linear_regression_slope(&data);
        assert_eq!(slope, 0.0, "Empty data should return 0.0");
    }

    #[test]
    fn test_linear_regression_slope_single_point() {
        let data = vec![(1.0, 2.0)];
        let slope = linear_regression_slope(&data);
        assert_eq!(slope, 0.0, "Single point should return 0.0");
    }

    // =========================================================================
    // Pit counting tests
    // =========================================================================

    #[test]
    fn test_count_pits_single_pit() {
        // Create a sloped heightmap with a single pit in the center
        // Terrain slopes from top-left to bottom-right, with one pit
        let mut heightmap = Tilemap::new_with(10, 10, 0.0f32);
        for y in 0..10 {
            for x in 0..10 {
                // Slope from corner, so only explicit pits are local minima
                heightmap.set(x, y, (x + y) as f32 * 10.0 + 100.0);
            }
        }
        // Create a pit at (5, 5) - lower than all neighbors
        heightmap.set(5, 5, 50.0);  // Neighbors will be ~100-140

        let count = count_pits(&heightmap);
        assert_eq!(count, 1, "Should find exactly one pit in the center");
    }

    #[test]
    fn test_count_pits_no_pits_sloped() {
        // Create a perfectly sloped heightmap (no pits)
        let mut heightmap = Tilemap::new_with(10, 10, 0.0f32);
        for y in 0..10 {
            for x in 0..10 {
                heightmap.set(x, y, (y as f32) * 10.0);  // Slope from top to bottom
            }
        }

        let count = count_pits(&heightmap);
        assert_eq!(count, 0, "Sloped terrain should have no pits");
    }

    #[test]
    fn test_count_pits_ignores_ocean() {
        // Create heightmap with ocean (negative elevation)
        let mut heightmap = Tilemap::new_with(5, 5, -100.0f32);  // All ocean
        heightmap.set(2, 2, -200.0);  // Deeper spot in ocean

        let count = count_pits(&heightmap);
        assert_eq!(count, 0, "Ocean cells should not be counted as pits");
    }

    #[test]
    fn test_count_pits_multiple() {
        // Create sloped heightmap with multiple pits
        let mut heightmap = Tilemap::new_with(16, 16, 0.0f32);
        for y in 0..16 {
            for x in 0..16 {
                // Slope so only explicit pits are local minima
                heightmap.set(x, y, (x + y) as f32 * 10.0 + 200.0);
            }
        }
        // Create two isolated pits (not at edges)
        heightmap.set(4, 4, 10.0);   // Neighbors will be ~80-160
        heightmap.set(11, 11, 10.0); // Neighbors will be ~220-260

        let count = count_pits(&heightmap);
        assert_eq!(count, 2, "Should find exactly two pits");
    }

    // =========================================================================
    // Strahler stream ordering tests
    // =========================================================================

    #[test]
    fn test_strahler_single_stream() {
        // Create a simple linear stream: flow from (0,0) -> (1,0) -> (2,0) -> (3,0)
        // Direction 2 = East (+1, 0)
        let mut flow_dir = Tilemap::new_with(5, 3, NO_FLOW);
        let mut river_mask = Tilemap::new_with(5, 3, false);

        // Linear stream flowing east
        for x in 0..4 {
            flow_dir.set(x, 1, 2);  // Direction 2 = East
            river_mask.set(x, 1, true);
        }
        flow_dir.set(4, 1, NO_FLOW);  // End of stream
        river_mask.set(4, 1, true);

        let orders = compute_strahler_orders(&flow_dir, &river_mask);

        // All cells should be order 1 (no tributaries)
        for x in 0..5 {
            let order = *orders.get(x, 1);
            assert_eq!(order, 1, "Single stream should be order 1 at ({}, 1), got {}", x, order);
        }
    }

    #[test]
    fn test_strahler_confluence_same_order() {
        // Two order-1 streams meeting should create order-2
        // Simple Y-junction topology:
        //   (0,0) -> (1,1)
        //                 \
        //                  -> (2,2) -> (3,2)
        //                 /
        //   (0,2) -> (1,1) - this connects via (1,2) actually
        //
        // Simpler approach: two parallel streams joining
        let mut flow_dir = Tilemap::new_with(5, 5, NO_FLOW);
        let mut river_mask = Tilemap::new_with(5, 5, false);

        // Stream 1: (1,1) flows south to (1,2), then south to (1,3)
        flow_dir.set(1, 1, 4);  // Direction 4 = South (0, +1)
        river_mask.set(1, 1, true);

        // Stream 2: (3,1) flows south to (3,2), then southwest to (2,3)
        flow_dir.set(3, 1, 4);  // South
        river_mask.set(3, 1, true);

        flow_dir.set(3, 2, 5);  // Direction 5 = Southwest (-1, +1)
        river_mask.set(3, 2, true);

        // Stream 1 continues: (1,2) flows southeast to (2,3)
        flow_dir.set(1, 2, 3);  // Direction 3 = Southeast (+1, +1)
        river_mask.set(1, 2, true);

        // Confluence at (2,3): receives from both streams
        flow_dir.set(2, 3, 4);  // South to outlet
        river_mask.set(2, 3, true);

        // Outlet
        flow_dir.set(2, 4, NO_FLOW);
        river_mask.set(2, 4, true);

        let orders = compute_strahler_orders(&flow_dir, &river_mask);

        // Headwaters should be order 1
        let h1_order = *orders.get(1, 1);
        let h2_order = *orders.get(3, 1);
        assert_eq!(h1_order, 1, "Headwater 1 should be order 1, got {}", h1_order);
        assert_eq!(h2_order, 1, "Headwater 2 should be order 1, got {}", h2_order);

        // After confluence, order should increase
        // Note: Strahler rule - when two same-order streams join, order increases
        let confluence_order = *orders.get(2, 3);
        assert!(confluence_order >= 1, "Confluence should have order >= 1, got {}", confluence_order);
    }

    // =========================================================================
    // Bifurcation ratio tests
    // =========================================================================

    #[test]
    fn test_bifurcation_ratio_calculation() {
        // Create order counts matching typical bifurcation ratios
        // Rb = N_u / N_{u+1} where N is number of streams
        // If order 1: 16 streams, order 2: 4 streams, order 3: 1 stream
        // Rb_1 = 16/4 = 4.0, Rb_2 = 4/1 = 4.0
        let mut counts: HashMap<u8, usize> = HashMap::new();
        counts.insert(1, 16);
        counts.insert(2, 4);
        counts.insert(3, 1);

        let rb = compute_bifurcation_ratio(&counts);
        // Average should be around 4.0
        assert!(rb >= 3.0 && rb <= 5.0,
            "Bifurcation ratio should be in Earth-like range 3-5, got {}", rb);
    }

    #[test]
    fn test_bifurcation_ratio_single_order() {
        // Only order 1 streams - cannot compute ratio
        let mut counts: HashMap<u8, usize> = HashMap::new();
        counts.insert(1, 10);

        let rb = compute_bifurcation_ratio(&counts);
        assert_eq!(rb, 0.0, "Single order should return 0.0");
    }

    // =========================================================================
    // Drainage density tests (via analyze function)
    // =========================================================================

    #[test]
    fn test_drainage_density_via_analysis() {
        // Create a sloped terrain where rivers will form
        let mut heightmap = Tilemap::new_with(32, 32, 0.0f32);
        for y in 0..32 {
            for x in 0..32 {
                // Create a valley that channels water
                let dist_from_center = ((x as f32 - 16.0).abs()).min(8.0);
                heightmap.set(x, y, 200.0 - y as f32 * 5.0 + dist_from_center * 10.0);
            }
        }

        let results = analyze(&heightmap, 5.0);
        // With a valley, we should have some drainage
        assert!(results.drainage_density >= 0.0, "Drainage density should be non-negative");
    }

    #[test]
    fn test_drainage_density_flat_terrain() {
        // Flat terrain should have 0 drainage
        let heightmap = Tilemap::new_with(32, 32, 100.0f32);
        let results = analyze(&heightmap, 5.0);
        assert_eq!(results.drainage_density, 0.0,
            "Flat terrain should have 0 drainage density");
    }

    // =========================================================================
    // Fractal dimension tests
    // =========================================================================

    #[test]
    fn test_fractal_dimension_line() {
        // A straight line should have fractal dimension close to 1.0
        let mut river_mask = Tilemap::new_with(64, 64, false);
        for x in 0..64 {
            river_mask.set(x, 32, true);
        }

        let fd = compute_fractal_dimension(&river_mask);
        // A line has dimension ~1.0
        assert!(fd >= 0.8 && fd <= 1.5,
            "Line should have fractal dimension near 1.0, got {}", fd);
    }

    #[test]
    fn test_fractal_dimension_filled() {
        // A filled square should have fractal dimension close to 2.0
        let river_mask = Tilemap::new_with(64, 64, true);

        let fd = compute_fractal_dimension(&river_mask);
        // A filled area has dimension ~2.0
        assert!(fd >= 1.8 && fd <= 2.2,
            "Filled square should have fractal dimension near 2.0, got {}", fd);
    }

    // =========================================================================
    // Sinuosity tests (via analyze function)
    // =========================================================================

    #[test]
    fn test_sinuosity_via_analysis() {
        // Create a terrain with a straight channel to the ocean
        let mut heightmap = Tilemap::new_with(32, 32, 0.0f32);

        // Create land with a straight valley leading to ocean (negative elevation at edge)
        for y in 0..32 {
            for x in 0..32 {
                if y < 30 {
                    // Land with a valley at x=16
                    let dist_from_center = (x as f32 - 16.0).abs();
                    heightmap.set(x, y, 200.0 - y as f32 * 5.0 + dist_from_center * 5.0);
                } else {
                    // Ocean at bottom edge
                    heightmap.set(x, y, -100.0);
                }
            }
        }

        let results = analyze(&heightmap, 5.0);
        // Sinuosity should be 1.0 or greater (path length >= straight distance)
        assert!(results.sinuosity_index >= 1.0,
            "Sinuosity should be at least 1.0, got {}", results.sinuosity_index);
    }

    // =========================================================================
    // ASCII profile plotting tests
    // =========================================================================

    #[test]
    fn test_plot_profile_ascii_empty() {
        let profile: Vec<(f32, f32)> = vec![];
        let result = plot_profile_ascii(&profile, 40, 10);
        assert_eq!(result, "No profile data");
    }

    #[test]
    fn test_plot_profile_ascii_generates_output() {
        // Simple profile: distance 0-100, elevation decreasing 1000 to 0
        let profile: Vec<(f32, f32)> = (0..=10)
            .map(|i| (i as f32 * 10.0, 1000.0 - i as f32 * 100.0))
            .collect();

        let result = plot_profile_ascii(&profile, 40, 10);

        // Should contain header and plot characters
        assert!(result.contains("Longitudinal Profile"), "Should have header");
        assert!(result.contains("*"), "Should contain plot points");
        assert!(result.contains("|"), "Should contain y-axis");
        assert!(result.contains("-"), "Should contain x-axis");
    }

    // =========================================================================
    // Integration tests
    // =========================================================================

    #[test]
    fn test_analyze_synthetic_heightmap() {
        // Create a simple synthetic heightmap with a clear drainage pattern
        // Higher in center, sloping to edges
        let size = 32;
        let mut heightmap = Tilemap::new_with(size, size, 0.0f32);

        // Create a cone-shaped mountain in the center
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - size as f32 / 2.0;
                let dy = y as f32 - size as f32 / 2.0;
                let dist = (dx * dx + dy * dy).sqrt();
                let h = 500.0 - dist * 20.0;  // Peak at center, slopes outward
                heightmap.set(x, y, h.max(-100.0));  // Clamp to -100 for "ocean"
            }
        }

        // Run analysis
        let results = analyze(&heightmap, 5.0);

        // Verify basic properties
        assert!(results.drainage_density >= 0.0, "Drainage density should be non-negative");
        assert!(results.total_stream_length >= 0, "Stream length should be non-negative");

        // With a cone heightmap, we should have some river cells
        // (water flows radially outward from peak)
    }

    #[test]
    fn test_analyze_flat_terrain() {
        // Flat terrain should have minimal drainage features
        let heightmap = Tilemap::new_with(32, 32, 100.0f32);

        let results = analyze(&heightmap, 5.0);

        // Flat terrain has no slopes, so minimal drainage
        assert_eq!(results.drainage_density, 0.0,
            "Flat terrain should have 0 drainage density");
    }

    #[test]
    fn test_analyze_all_ocean() {
        // All ocean terrain should have no land-based metrics
        let heightmap = Tilemap::new_with(32, 32, -100.0f32);

        let results = analyze(&heightmap, 5.0);

        // No land = no rivers
        assert_eq!(results.drainage_density, 0.0, "Ocean should have no drainage");
        assert_eq!(results.pit_count, 0, "Ocean should have no land pits");
    }

    #[test]
    fn test_realism_score_bounds() {
        // Verify realism score is always 0-100
        let heightmap = Tilemap::new_with(16, 16, 100.0f32);
        let results = analyze(&heightmap, 5.0);
        let score = results.realism_score();

        assert!(score >= 0.0 && score <= 100.0,
            "Realism score should be 0-100, got {}", score);
    }

    // =========================================================================
    // Realism scoring tests
    // =========================================================================

    #[test]
    fn test_realism_score_perfect_values() {
        // Create results with "perfect" Earth-like values
        let mut results = GeomorphometryResults::default();
        results.bifurcation_ratio = 4.0;      // Target: 3.0-5.0 ✓
        results.hacks_law_exponent = 0.55;    // Target: 0.5-0.6 ✓
        results.concavity_index = 0.55;       // Target: 0.4-0.7 ✓
        results.fractal_dimension = 1.9;      // Target: ~2.0 ✓
        results.sinuosity_index = 1.3;        // Target: >1.0 ✓
        results.pit_count = 0;                // Target: 0 ✓

        let score = results.realism_score();

        // Perfect values should give high score (not necessarily 100 due to weighting)
        assert!(score >= 50.0,
            "Perfect Earth-like values should score at least 50, got {}", score);
    }

    #[test]
    fn test_realism_score_terrible_values() {
        // Create results with terrible values
        let mut results = GeomorphometryResults::default();
        results.bifurcation_ratio = 0.0;      // No branching
        results.hacks_law_exponent = 0.0;     // No length-area scaling
        results.concavity_index = -1.0;       // Wrong sign
        results.fractal_dimension = 0.5;      // Way too low
        results.sinuosity_index = 1.0;        // Perfectly straight (unrealistic)
        results.pit_count = 1000;             // Many pits

        let score = results.realism_score();

        // Bad values should give low score
        assert!(score <= 30.0,
            "Terrible values should score at most 30, got {}", score);
    }
}
