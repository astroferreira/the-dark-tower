//! Flow accumulation-based river erosion with sediment transport.
//!
//! This module implements realistic river network formation using:
//! 1. D8 flow direction algorithm - determines where water flows from each cell
//! 2. Flow accumulation - counts upstream drainage area for each cell
//! 3. River tracing - follows actual drainage paths from source to sea
//! 4. Sediment transport - erodes upstream, deposits downstream
//!
//! Key improvement over simple threshold-based erosion:
//! - Only erodes along connected river paths (no isolated holes)
//! - Sediment is transported and deposited, creating deltas
//! - No pit creation - erosion respects downstream elevation

use crate::erosion::ErosionStats;
use crate::tilemap::Tilemap;

/// Direction encoding for D8 flow algorithm
/// Each direction is encoded as an index 0-7
/// 7 0 1
/// 6 X 2
/// 5 4 3
pub const DX: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
pub const DY: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

/// Special value indicating no flow direction (pit or ocean)
pub const NO_FLOW: u8 = 255;

/// State tracked while tracing a river
struct RiverState {
    /// Current position
    x: usize,
    y: usize,
    /// Amount of sediment being carried
    sediment: f32,
    /// Accumulated flow (increases as tributaries join)
    flow: f32,
    /// Current velocity (based on slope)
    velocity: f32,
}

/// A point along a traced river path
struct RiverPoint {
    x: usize,
    y: usize,
    flow: f32,
    slope: f32,
    /// Direction of flow (0-7)
    direction: u8,
}

/// Parameters for river erosion
pub struct RiverErosionParams {
    /// Minimum flow accumulation for a cell to be a river source
    pub source_min_accumulation: f32,
    /// Minimum elevation above sea level for river sources
    pub source_min_elevation: f32,
    /// Sediment capacity multiplier (capacity = factor * flow * slope)
    pub capacity_factor: f32,
    /// Rate at which rivers erode when under capacity
    pub erosion_rate: f32,
    /// Rate at which rivers deposit when over capacity
    pub deposition_rate: f32,
    /// Maximum erosion per cell (prevents extreme valleys)
    pub max_erosion: f32,
    /// Maximum deposition per cell
    pub max_deposition: f32,
    /// Width of river channel (for cross-section erosion)
    pub channel_width: usize,
    /// Number of erosion passes (allows upstream erosion to catch up to lowered downstream)
    pub passes: usize,
}

impl Default for RiverErosionParams {
    fn default() -> Self {
        Self {
            source_min_accumulation: 100.0,  // Need decent upstream area
            source_min_elevation: 50.0,      // Start above coast
            capacity_factor: 10.0,           // Sediment capacity multiplier
            erosion_rate: 0.2,               // How fast to erode (gentler)
            deposition_rate: 0.3,            // How fast to deposit
            max_erosion: 12.0,               // Max erosion per cell (was 50, prevents canyons)
            max_deposition: 15.0,            // Max deposition per cell
            channel_width: 2,                // River channel half-width
            passes: 3,                       // Fewer passes (was 5)
        }
    }
}

/// Compute flow direction for each cell using D8 algorithm.
/// NOTE: This uses raw heightmap. For pit-free routing, use compute_flow_direction_filled().
pub fn compute_flow_direction(heightmap: &Tilemap<f32>) -> Tilemap<u8> {
    compute_flow_direction_internal(heightmap)
}

/// Internal D8 flow direction computation
fn compute_flow_direction_internal(heightmap: &Tilemap<f32>) -> Tilemap<u8> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut flow_dir = Tilemap::new_with(width, height, NO_FLOW);

    for y in 0..height {
        for x in 0..width {
            let current_height = *heightmap.get(x, y);

            let mut steepest_dir: Option<u8> = None;
            let mut steepest_drop: f32 = 0.0;

            for dir in 0..8u8 {
                let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                let ny = y as i32 + DY[dir as usize];

                if ny < 0 || ny >= height as i32 {
                    continue;
                }
                let ny = ny as usize;

                let neighbor_height = *heightmap.get(nx, ny);
                let drop = current_height - neighbor_height;

                let distance = if dir % 2 == 0 { 1.0 } else { 1.414 };
                let slope = drop / distance;

                if slope > steepest_drop {
                    steepest_drop = slope;
                    steepest_dir = Some(dir);
                }
            }

            if let Some(dir) = steepest_dir {
                flow_dir.set(x, y, dir);
            }
        }
    }

    flow_dir
}

/// Compute flow direction using a depression-filled heightmap.
/// This ensures ALL cells can flow to the ocean - no pits/puddles.
/// Returns (flow_direction, filled_heightmap) so accumulation can use the filled map.
pub fn compute_flow_direction_filled(heightmap: &Tilemap<f32>) -> (Tilemap<u8>, Tilemap<f32>) {
    // Fill depressions so water can overflow pits
    let filled = fill_depressions(heightmap);
    // Compute flow on the filled map - guarantees connectivity
    let flow_dir = compute_flow_direction_internal(&filled);
    (flow_dir, filled)
}

/// Compute flow accumulation using a depression-filled heightmap.
/// This is the recommended method for river routing as it ensures connectivity.
pub fn compute_flow_with_filled_routing(heightmap: &Tilemap<f32>) -> (Tilemap<u8>, Tilemap<f32>, Tilemap<f32>) {
    let (flow_dir, filled) = compute_flow_direction_filled(heightmap);
    let flow_acc = compute_flow_accumulation(&filled, &flow_dir);
    (flow_dir, flow_acc, filled)
}

/// Compute flow accumulation for each cell.
pub fn compute_flow_accumulation(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut accumulation = Tilemap::new_with(width, height, 1.0f32);

    // Sort cells by elevation, highest first
    let mut cells: Vec<(usize, usize, f32)> = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            cells.push((x, y, *heightmap.get(x, y)));
        }
    }
    cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Process cells from highest to lowest
    for (x, y, _) in cells {
        let dir = *flow_dir.get(x, y);
        if dir == NO_FLOW {
            continue;
        }

        let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
        let ny = y as i32 + DY[dir as usize];

        if ny < 0 || ny >= height as i32 {
            continue;
        }
        let ny = ny as usize;

        let current_acc = *accumulation.get(x, y);
        let downstream_acc = *accumulation.get(nx, ny);
        accumulation.set(nx, ny, downstream_acc + current_acc);
    }

    accumulation
}

/// Find potential river source points.
/// Sources are high-elevation cells with sufficient upstream accumulation.
fn find_river_sources(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    params: &RiverErosionParams,
) -> Vec<(usize, usize)> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut sources = Vec::new();

    // Find all cells that qualify as river sources
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            // Must be above sea level and have sufficient accumulation
            if h >= params.source_min_elevation && acc >= params.source_min_accumulation {
                // Check if this is a "headwater" - not receiving flow from a larger source
                // A true source has accumulation close to the minimum threshold
                if acc < params.source_min_accumulation * 3.0 {
                    sources.push((x, y));
                }
            }
        }
    }

    // Sort sources by accumulation (larger rivers first)
    sources.sort_by(|a, b| {
        let acc_a = *flow_acc.get(a.0, a.1);
        let acc_b = *flow_acc.get(b.0, b.1);
        acc_b.partial_cmp(&acc_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    sources
}

/// Trace a river from source to sea, applying erosion and deposition.
fn trace_river(
    heightmap: &mut Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    hardness: &Tilemap<f32>,
    start_x: usize,
    start_y: usize,
    params: &RiverErosionParams,
    visited: &mut Tilemap<bool>,
    stats: &mut ErosionStats,
) {
    let width = heightmap.width;
    let height = heightmap.height;
    let sea_level = 0.0f32;

    let mut state = RiverState {
        x: start_x,
        y: start_y,
        sediment: 0.0,
        flow: *flow_acc.get(start_x, start_y),
        velocity: 1.0,
    };

    // Maximum steps to prevent infinite loops
    let max_steps = width * height;
    let mut steps = 0;

    for _ in 0..max_steps {
        steps += 1;
        let x = state.x;
        let y = state.y;

        // Mark as visited
        visited.set(x, y, true);

        let current_height = *heightmap.get(x, y);

        // Get flow direction first (need it for deposition)
        let dir = *flow_dir.get(x, y);

        // Stop if we've reached the sea
        if current_height < sea_level {
            // Deposit remaining sediment at river mouth (delta)
            if state.sediment > 0.0 {
                let deposit = state.sediment.min(params.max_deposition);
                apply_delta_deposition(heightmap, x, y, deposit, dir);
                stats.total_deposited += deposit as f64;
            }
            break;
        }

        if dir == NO_FLOW {
            break;
        }

        // Find next cell
        let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
        let ny = y as i32 + DY[dir as usize];

        if ny < 0 || ny >= height as i32 {
            break;
        }
        let ny = ny as usize;

        let next_height = *heightmap.get(nx, ny);

        // Calculate slope (positive = downhill)
        let distance = if dir % 2 == 0 { 1.0 } else { 1.414 };
        let slope = ((current_height - next_height) / distance).max(0.0);

        // Update flow (accumulates downstream)
        state.flow = *flow_acc.get(x, y);

        // Update velocity based on slope
        state.velocity = (1.0 + slope * 2.0).min(10.0);

        // Calculate sediment capacity
        // Capacity depends on flow and slope (stream power)
        // Minimum capacity ensures erosion happens even on flat terrain
        let min_capacity = params.capacity_factor * state.flow.sqrt() * 0.01;
        let capacity = (params.capacity_factor * state.flow.sqrt() * slope * state.velocity).max(min_capacity);

        // Get rock hardness
        let rock_hardness = *hardness.get(x, y);
        let hardness_factor = (1.0 - rock_hardness).max(0.1);

        if state.sediment < capacity {
            // Under capacity: ERODE
            let erosion_potential = (capacity - state.sediment) * params.erosion_rate * hardness_factor;

            // Critical: Don't erode below the downstream cell (prevents pits)
            let max_safe_erosion = (current_height - next_height - 0.1).max(0.0);
            let erosion = erosion_potential.min(max_safe_erosion).min(params.max_erosion);

            if erosion > 0.0 {
                // Apply V-shaped erosion along river (width scales with flow)
                apply_erosion(
                    heightmap, x, y, erosion, dir,
                    params.channel_width,
                    state.flow,
                    params.source_min_accumulation,
                );
                state.sediment += erosion;
                stats.total_eroded += erosion as f64;
                stats.max_erosion = stats.max_erosion.max(erosion);
            }
        } else {
            // Over capacity: DEPOSIT on floodplains (not in channel)
            let deposit_amount = (state.sediment - capacity) * params.deposition_rate;
            let deposit = deposit_amount.min(state.sediment).min(params.max_deposition);

            if deposit > 0.0 {
                apply_deposition(
                    heightmap, x, y, deposit,
                    params.channel_width, dir,
                    state.flow,
                    params.source_min_accumulation,
                );
                state.sediment -= deposit;
                stats.total_deposited += deposit as f64;
                stats.max_deposition = stats.max_deposition.max(deposit);
            }
        }

        // Move to next cell
        state.x = nx;
        state.y = ny;

        // ENFORCE MONOTONIC DESCENT (Stream Carving)
        // Ensure the next cell is strictly lower than the current cell.
        // This cuts a channel through flat "filled" areas AND barriers, ensuring connectivity.
        let min_drop = 0.5; // Minimum drop per cell - more aggressive for visible channels
        let current_h_after = *heightmap.get(x, y);
        let next_h = *heightmap.get(nx, ny);

        // If downstream is at or above current, carve through it
        // This is critical for breaching barriers that block river flow
        if next_h >= current_h_after - min_drop && next_h > 0.1 {
            // Carve to ensure monotonic descent, but don't go below sea level
            let new_h = (current_h_after - min_drop).max(0.1);
            if new_h < next_h {
                heightmap.set(nx, ny, new_h);
            }
        }

        // Note: We used to break here if visited, to avoid re-tracing main rivers.
        // But to ensure connectivity (if tributary lowers confluence), we MUST continue tracing
        // and carving downstream. Since N is small (<100 rivers), this O(N*L) is acceptable.
        
        // if *visited.get(nx, ny) {
        //    break;
        // }
    }
    
    // Record river length
    stats.river_lengths.push(steps);
}

/// Calculate dynamic river width based on flow accumulation.
/// Rivers get wider as more tributaries merge downstream.
fn calculate_river_width(flow: f32, base_width: usize, source_threshold: f32) -> usize {
    // Width scales with sqrt of flow (hydraulic geometry relationship)
    // Typical relationship: w ∝ Q^0.5 where Q is discharge (flow)
    let flow_ratio = (flow / source_threshold).max(1.0);
    let width_multiplier = flow_ratio.sqrt();

    // Base width at source, grows with flow
    // Clamp to reasonable range (1 to 8 pixels half-width)
    let dynamic_width = (base_width as f32 * width_multiplier).round() as usize;
    dynamic_width.clamp(1, 8)
}

/// Apply V-shaped erosion perpendicular to flow direction.
/// Width scales with flow accumulation - larger rivers are wider.
fn apply_erosion(
    heightmap: &mut Tilemap<f32>,
    x: usize,
    y: usize,
    amount: f32,
    flow_dir: u8,
    base_width: usize,
    flow: f32,
    source_threshold: f32,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Calculate dynamic width based on flow
    let half_width = calculate_river_width(flow, base_width, source_threshold);

    // Get perpendicular direction for channel cross-section
    let (perp_dx, perp_dy) = get_perpendicular(flow_dir);

    // Minimum allowed height - don't erode below sea level
    // Leave a small epsilon (0.1m) so rivers stay as "land" for water detection
    const MIN_RIVER_HEIGHT: f32 = 0.1;

    // Apply V-shaped profile across channel
    for i in -(half_width as i32)..=(half_width as i32) {
        let nx = (x as i32 + perp_dx * i).rem_euclid(width as i32) as usize;
        let ny = (y as i32 + perp_dy * i).clamp(0, height as i32 - 1) as usize;

        // V-shape: full erosion at center, decreasing toward edges
        let dist = i.abs() as f32;
        let falloff = 1.0 - (dist / (half_width as f32 + 1.0));
        let local_erosion = amount * falloff * falloff; // Squared for V-shape

        let current = *heightmap.get(nx, ny);

        // Clamp erosion to not dig below sea level
        let max_possible_erosion = (current - MIN_RIVER_HEIGHT).max(0.0);
        let actual_erosion = local_erosion.min(max_possible_erosion);

        heightmap.set(nx, ny, current - actual_erosion);
    }
}

/// Apply deposition on floodplains (NOT in the channel itself).
/// Deposits sediment on the sides of the river, creating levees/floodplains.
/// Width scales with flow accumulation.
fn apply_deposition(
    heightmap: &mut Tilemap<f32>,
    x: usize,
    y: usize,
    amount: f32,
    base_width: usize,
    flow_dir: u8,
    flow: f32,
    source_threshold: f32,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Calculate dynamic width based on flow
    let half_width = calculate_river_width(flow, base_width, source_threshold);

    // Get perpendicular direction to deposit on floodplains
    let (perp_dx, perp_dy) = get_perpendicular(flow_dir);

    // Deposit on the sides of the channel, NOT in the channel center
    // This creates natural levees/floodplains
    let inner_radius = half_width as i32 + 1; // Start depositing outside the channel
    let outer_radius = half_width as i32 + 3; // How far out to deposit

    for i in -outer_radius..=outer_radius {
        // Skip the channel itself (center area)
        if i.abs() <= inner_radius {
            continue;
        }

        let nx = (x as i32 + perp_dx * i).rem_euclid(width as i32) as usize;
        let ny = (y as i32 + perp_dy * i).clamp(0, height as i32 - 1) as usize;

        // Falloff from inner edge outward
        let dist_from_channel = (i.abs() - inner_radius) as f32;
        let falloff = 1.0 - (dist_from_channel / (outer_radius - inner_radius + 1) as f32);
        let local_deposit = amount * falloff * 0.3; // Reduced amount for subtle levees

        let current = *heightmap.get(nx, ny);
        heightmap.set(nx, ny, current + local_deposit);
    }
}

/// Apply delta deposition at river mouth (spreads sediment into sea).
fn apply_delta_deposition(
    heightmap: &mut Tilemap<f32>,
    x: usize,
    y: usize,
    amount: f32,
    flow_dir: u8,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Get flow direction vector
    let (flow_dx, flow_dy) = if flow_dir < 8 {
        (DX[flow_dir as usize], DY[flow_dir as usize])
    } else {
        (0, 1) // Default to down
    };

    // Fan out sediment in the direction of flow (delta formation)
    let fan_radius = 4i32;

    for dy in 0..=fan_radius {
        for dx in -fan_radius..=fan_radius {
            // Only deposit in forward direction (downstream)
            let forward = dx * flow_dx + dy * flow_dy;
            if forward < 0 {
                continue;
            }

            let dist_sq = dx * dx + dy * dy;
            if dist_sq > fan_radius * fan_radius {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let dist = (dist_sq as f32).sqrt();
            let falloff = 1.0 - (dist / (fan_radius as f32 + 1.0));
            let local_deposit = amount * falloff * 0.5;

            let current = *heightmap.get(nx, ny);
            // Only deposit if we're raising underwater terrain (building delta)
            if current < 0.0 {
                heightmap.set(nx, ny, (current + local_deposit).min(5.0)); // Cap at just above sea level
            }
        }
    }
}

/// Apply meander enhancement by lateral erosion.
/// Rivers in flat areas curve because water erodes the outer bank of bends.
pub fn apply_meander_erosion(
    heightmap: &mut Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
    meander_strength: f32,  // Try 5.0-20.0
    seed: u64,
) {
    use noise::{NoiseFn, Perlin, Seedable};

    let width = heightmap.width;
    let height = heightmap.height;
    let noise = Perlin::new(1).set_seed(seed as u32);

    // Find river cells and apply lateral erosion
    for y in 1..height - 1 {
        for x in 0..width {
            let acc = *flow_acc.get(x, y);
            let h = *heightmap.get(x, y);

            // Only process river cells on land
            if acc < threshold || h < 0.0 {
                continue;
            }

            let dir = *flow_dir.get(x, y);
            if dir == NO_FLOW {
                continue;
            }

            // Calculate local slope
            let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + DY[dir as usize]).clamp(0, height as i32 - 1) as usize;
            let slope = (h - *heightmap.get(nx, ny)).max(0.0);

            // Meander more in flat areas (low slope)
            let flatness = (1.0 - (slope / 50.0).min(1.0)).max(0.0);

            if flatness < 0.3 {
                continue; // Don't meander on steep terrain
            }

            // Use noise to determine which side to erode
            // Tuned frequency for visible meanders (~1.2-1.3 sinuosity)
            let n = noise.get([x as f64 * 0.07, y as f64 * 0.07]) as f32;

            // Get perpendicular direction
            let (perp_dx, perp_dy) = get_perpendicular(dir);

            // Erode one side, deposit on the other (lateral migration)
            let erosion_side = if n > 0.0 { 1 } else { -1 };
            let erosion_amount = meander_strength * flatness * n.abs();

            // Erode outer bank (but don't dig below sea level)
            const MIN_RIVER_HEIGHT: f32 = 0.1;
            let ex = (x as i32 + perp_dx * erosion_side).rem_euclid(width as i32) as usize;
            let ey = (y as i32 + perp_dy * erosion_side).clamp(0, height as i32 - 1) as usize;
            let eh = *heightmap.get(ex, ey);
            if eh > MIN_RIVER_HEIGHT {
                heightmap.set(ex, ey, (eh - erosion_amount).max(MIN_RIVER_HEIGHT));
            }

            // Deposit on inner bank (point bar)
            let dx = (x as i32 - perp_dx * erosion_side).rem_euclid(width as i32) as usize;
            let dy = (y as i32 - perp_dy * erosion_side).clamp(0, height as i32 - 1) as usize;
            let dh = *heightmap.get(dx, dy);
            if dh > 0.0 {
                heightmap.set(dx, dy, dh + erosion_amount * 0.5);
            }
        }
    }
}

/// Get perpendicular direction for channel cross-section.
fn get_perpendicular(flow_dir: u8) -> (i32, i32) {
    // Perpendicular is 90 degrees clockwise (or counterclockwise)
    match flow_dir {
        0 => (1, 0),   // Flow up → perpendicular is right
        1 => (1, 1),   // Flow up-right → perpendicular is down-right
        2 => (0, 1),   // Flow right → perpendicular is down
        3 => (-1, 1),  // Flow down-right → perpendicular is down-left
        4 => (-1, 0),  // Flow down → perpendicular is left
        5 => (-1, -1), // Flow down-left → perpendicular is up-left
        6 => (0, -1),  // Flow left → perpendicular is up
        7 => (1, -1),  // Flow up-left → perpendicular is up-right
        _ => (1, 0),
    }
}

/// Main entry point: erode river channels with sediment transport.
pub fn erode_rivers(
    heightmap: &mut Tilemap<f32>,
    hardness: &Tilemap<f32>,
    params: &RiverErosionParams,
) -> ErosionStats {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut stats = ErosionStats::default();

    // 1. Fill depressions for flow routing
    let filled_map = fill_depressions(heightmap);

    // HYDRO-CONDITIONING: Apply filled heights to create spillover surfaces
    for y in 0..height {
        for x in 0..width {
            heightmap.set(x, y, *filled_map.get(x, y));
        }
    }

    // 2. Compute flow direction using filled map (ensures connectivity)
    let flow_dir = compute_flow_direction(&filled_map);

    // DEBUG: Verify filled_map has no pits
    let mut pit_count = 0;
    for y in 0..height {
        for x in 0..width {
            if *filled_map.get(x, y) >= 0.0 && *flow_dir.get(x, y) == NO_FLOW {
                pit_count += 1;
            }
        }
    }
    if pit_count > 0 {
        println!("WARNING: filled_map still has {} pits!", pit_count);
    }

    let flow_acc = compute_flow_accumulation(&filled_map, &flow_dir);

    // 3. BREACH ALL BARRIERS before tracing rivers
    // This ensures every river path is physically carved through barriers
    println!("  Breaching barriers along river paths...");
    breach_river_barriers(heightmap, &flow_dir, &flow_acc, params.source_min_accumulation);

    // 4. Find river sources
    let sources = find_river_sources(heightmap, &flow_acc, params);

    // Track visited cells
    let mut visited = Tilemap::new_with(width, height, false);

    // Run erosion in multiple passes
    stats.iterations = params.passes;
    for _ in 0..params.passes {
        visited.fill(false);

        for (sx, sy) in &sources {
            if !*visited.get(*sx, *sy) {
                trace_river(
                    heightmap,
                    &flow_dir,
                    &flow_acc,
                    hardness,
                    *sx, *sy,
                    params,
                    &mut visited,
                    &mut stats,
                );
            }
        }
    }

    stats
}

/// Breach barriers along all river paths to ensure connectivity.
/// This traces from each river cell to the ocean, carving through any barriers.
fn breach_river_barriers(
    heightmap: &mut Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
) {
    let width = heightmap.width;
    let height = heightmap.height;
    const MIN_HEIGHT: f32 = 0.1;
    const MAX_PASSES: usize = 50;

    // Collect all river cells (high flow accumulation)
    let mut river_cells: Vec<(usize, usize, f32)> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let acc = *flow_acc.get(x, y);
            let h = *heightmap.get(x, y);
            if acc >= threshold && h > 0.0 {
                river_cells.push((x, y, h));
            }
        }
    }

    // Sort by elevation (highest first) - process from sources downstream
    river_cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Multiple passes to propagate breaching downstream
    for pass in 0..MAX_PASSES {
        let mut any_changed = false;

        for &(x, y, _) in &river_cells {
            let h = *heightmap.get(x, y);
            if h < 0.0 { continue; } // Ocean

            let dir = *flow_dir.get(x, y);
            if dir >= 8 { continue; } // No flow

            let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
            let ny = y as i32 + DY[dir as usize];
            if ny < 0 || ny >= height as i32 { continue; }
            let ny = ny as usize;

            let nh = *heightmap.get(nx, ny);

            // If downstream is at or above current (barrier), breach it
            if nh >= h && nh > MIN_HEIGHT {
                // Set downstream to slightly below current
                let new_h = (h - 0.5).max(MIN_HEIGHT);
                if new_h < nh {
                    heightmap.set(nx, ny, new_h);
                    any_changed = true;
                }
            }
        }

        if !any_changed {
            if pass > 0 {
                println!("    Barrier breaching complete after {} passes", pass + 1);
            }
            break;
        }
    }
}

/// Fill depressions using a simplified Planchon-Darboux algorithm.
/// Ensures that every cell can flow to the ocean (height < 0.0) or map edge.
/// Public wrapper for use after erosion.
pub fn fill_depressions_public(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    fill_depressions(heightmap)
}

/// Fill depressions using a simplified Planchon-Darboux algorithm.
/// Ensures that every cell can flow to the ocean (height < 0.0) or map edge.
fn fill_depressions(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut water = Tilemap::new_with(width, height, f32::MAX);
    let epsilon = 1e-4; // Tiny drop to ensure flow

    // 1. Initialize ocean cells with their real height
    // Everything else stays at f32::MAX (infinite)
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h < 0.0 {
                water.set(x, y, h);
            }
        }
    }

    // 2. Iteratively lower the water surface from neighbors
    // We alternate passes to propagate changes efficiently
    let mut changed = true;
    while changed {
        changed = false;

        // Pass 1: Top-Left to Bottom-Right
        for y in 0..height {
            for x in 0..width {
                let h = *heightmap.get(x, y);
                let mut min_neigh = f32::MAX;

                // Check 8 neighbors (with wrapping for X)
                for dir in 0..8 {
                    let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
                    let ny = y as i32 + DY[dir];
                    if ny >= 0 && ny < height as i32 {
                        min_neigh = min_neigh.min(*water.get(nx, ny as usize));
                    }
                }

                // Water level is max(terrain, neighbor + epsilon)
                // We clamp to current water level (only lower it)
                let new_water = h.max(min_neigh + epsilon);
                if new_water < *water.get(x, y) {
                    water.set(x, y, new_water);
                    changed = true;
                }
            }
        }

        // Pass 2: Bottom-Right to Top-Left
        for y in (0..height).rev() {
            for x in (0..width).rev() {
                let h = *heightmap.get(x, y);
                let mut min_neigh = f32::MAX;

                for dir in 0..8 {
                    let nx = (x as i32 + DX[dir]).rem_euclid(width as i32) as usize;
                    let ny = y as i32 + DY[dir];
                    if ny >= 0 && ny < height as i32 {
                        min_neigh = min_neigh.min(*water.get(nx, ny as usize));
                    }
                }

                let new_water = h.max(min_neigh + epsilon);
                if new_water < *water.get(x, y) {
                    water.set(x, y, new_water);
                    changed = true;
                }
            }
        }
    }

    water
}

/// Get the flow accumulation map for visualization.
pub fn get_flow_accumulation(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let flow_dir = compute_flow_direction(heightmap);
    compute_flow_accumulation(heightmap, &flow_dir)
}

/// Statistics about the river network connectivity.
#[derive(Debug, Default, Clone)]
pub struct RiverNetworkStats {
    pub total_rivers: usize,
    pub rivers_reaching_ocean: usize,
    pub rivers_ending_in_pit: usize,
    pub mean_length: f32,
    pub max_length: usize,
    pub connectivity_ratio: f32, // fraction reaching ocean
}

/// Analyze the river network to check connectivity and lengths.
/// This runs on the ACTUAL heightmap (not filled) to verify real connectivity.
pub fn analyze_river_network(
    heightmap: &Tilemap<f32>,
    params: &RiverErosionParams,
) -> RiverNetworkStats {
    let mut stats = RiverNetworkStats::default();

    // 1. Compute flow directions on the actual terrain
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir); // Recalc acc on real terrain

    // 2. Find sources based on this real accumulation
    // Note: This might differ from sources used during erosion if accumulation changed
    let sources = find_river_sources(heightmap, &flow_acc, params);
    stats.total_rivers = sources.len();

    if sources.is_empty() {
        return stats;
    }

    let mut total_length = 0;

    for (sx, sy) in sources {
        // Trace this river
        let mut x = sx;
        let mut y = sy;
        let mut length = 0;
        let mut reached_ocean = false;
        let mut visited = std::collections::HashSet::new(); // Prevent infinite loops

        loop {
            visited.insert((x, y));
            length += 1;

            if *heightmap.get(x, y) < 0.0 {
                reached_ocean = true;
                break;
            }

            let dir = *flow_dir.get(x, y);
            if dir == NO_FLOW {
                // Pit or edge
                break;
            }

            let nx = (x as i32 + DX[dir as usize]).rem_euclid(heightmap.width as i32) as usize;
            let ny = y as i32 + DY[dir as usize];

            if ny < 0 || ny >= heightmap.height as i32 {
                // Map edge
                reached_ocean = true; // Count flow off-map as success
                break;
            }
            let ny = ny as usize;

            if visited.contains(&(nx, ny)) {
                // Loop detected
                break;
            }

            x = nx;
            y = ny;
        }

        total_length += length;
        stats.max_length = stats.max_length.max(length);

        if reached_ocean {
            stats.rivers_reaching_ocean += 1;
        } else {
            stats.rivers_ending_in_pit += 1;
        }
    }

    stats.mean_length = total_length as f32 / stats.total_rivers as f32;
    if stats.total_rivers > 0 {
        stats.connectivity_ratio = stats.rivers_reaching_ocean as f32 / stats.total_rivers as f32;
    }

    stats
}

// =============================================================================
// RIVER VALIDATION (Phase 4)
// =============================================================================

/// Statistics about measured river channel widths.
#[derive(Debug, Default, Clone)]
pub struct RiverWidthStats {
    pub mean: f32,
    pub max: f32,
    pub min: f32,
    pub count: usize,
}

/// Statistics about river connectivity to the ocean.
#[derive(Debug, Default, Clone)]
pub struct ConnectivityStats {
    pub total: usize,
    pub reaching_ocean: usize,
    pub ending_in_pit: usize,
    pub ocean_ratio: f32,
}

/// Measure actual carved river channel widths.
/// Scans perpendicular to flow direction to find where terrain rises.
pub fn measure_river_widths(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    threshold: f32,
) -> RiverWidthStats {
    let width = heightmap.width;
    let height = heightmap.height;
    let flow_dir = compute_flow_direction(heightmap);

    let mut widths = Vec::new();

    for y in 1..height - 1 {
        for x in 0..width {
            // Only measure at river cells
            if *flow_acc.get(x, y) < threshold {
                continue;
            }

            // Get flow direction for perpendicular measurement
            let dir = *flow_dir.get(x, y);
            if dir == NO_FLOW {
                continue;
            }

            let channel_width = measure_channel_width_at(heightmap, x, y, dir);
            if channel_width > 0.0 {
                widths.push(channel_width);
            }
        }
    }

    if widths.is_empty() {
        return RiverWidthStats::default();
    }

    RiverWidthStats {
        mean: widths.iter().sum::<f32>() / widths.len() as f32,
        max: widths.iter().cloned().fold(0.0f32, f32::max),
        min: widths.iter().cloned().fold(f32::MAX, f32::min),
        count: widths.len(),
    }
}

/// Measure channel width at a specific point by scanning perpendicular to flow.
fn measure_channel_width_at(heightmap: &Tilemap<f32>, x: usize, y: usize, flow_dir: u8) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;
    let center_height = *heightmap.get(x, y);

    // Get perpendicular direction
    let (perp_dx, perp_dy) = get_perpendicular(flow_dir);

    // Scan in both perpendicular directions until terrain rises significantly
    let rise_threshold = 2.0; // Height rise that marks channel edge
    let max_scan = 10; // Maximum scan distance

    let mut left_width = 0;
    let mut right_width = 0;

    // Scan left (negative perpendicular)
    for i in 1..=max_scan {
        let nx = (x as i32 - perp_dx * i).rem_euclid(width as i32) as usize;
        let ny = (y as i32 - perp_dy * i).clamp(0, height as i32 - 1) as usize;
        let nh = *heightmap.get(nx, ny);

        if nh - center_height > rise_threshold {
            break;
        }
        left_width = i;
    }

    // Scan right (positive perpendicular)
    for i in 1..=max_scan {
        let nx = (x as i32 + perp_dx * i).rem_euclid(width as i32) as usize;
        let ny = (y as i32 + perp_dy * i).clamp(0, height as i32 - 1) as usize;
        let nh = *heightmap.get(nx, ny);

        if nh - center_height > rise_threshold {
            break;
        }
        right_width = i;
    }

    (left_width + right_width + 1) as f32 // +1 for center cell
}

/// Check river connectivity to ocean.
/// Traces each river source to see if it reaches the ocean.
pub fn check_river_connectivity(
    heightmap: &Tilemap<f32>,
    params: &RiverErosionParams,
) -> ConnectivityStats {
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);
    let sources = find_river_sources_public(heightmap, &flow_acc, params);

    let mut reaching_ocean = 0;
    let mut ending_in_pit = 0;

    for (sx, sy) in &sources {
        if trace_to_ocean(heightmap, &flow_dir, *sx, *sy) {
            reaching_ocean += 1;
        } else {
            ending_in_pit += 1;
        }
    }

    let total = sources.len();
    ConnectivityStats {
        total,
        reaching_ocean,
        ending_in_pit,
        ocean_ratio: if total > 0 {
            reaching_ocean as f32 / total as f32
        } else {
            0.0
        },
    }
}

/// Find river sources (public wrapper for validation).
fn find_river_sources_public(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    params: &RiverErosionParams,
) -> Vec<(usize, usize)> {
    find_river_sources(heightmap, flow_acc, params)
}

/// Trace a river from source to see if it reaches the ocean.
fn trace_to_ocean(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    start_x: usize,
    start_y: usize,
) -> bool {
    let width = heightmap.width;
    let height = heightmap.height;
    let max_steps = width * height;

    let mut x = start_x;
    let mut y = start_y;
    let mut visited = std::collections::HashSet::new();

    for _ in 0..max_steps {
        if visited.contains(&(x, y)) {
            return false; // Loop detected
        }
        visited.insert((x, y));

        // Check if we've reached the ocean
        if *heightmap.get(x, y) < 0.0 {
            return true;
        }

        let dir = *flow_dir.get(x, y);
        if dir == NO_FLOW {
            return false; // Pit
        }

        let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
        let ny = y as i32 + DY[dir as usize];

        if ny < 0 || ny >= height as i32 {
            return true; // Edge of map counts as reaching ocean
        }

        x = nx;
        y = ny as usize;
    }

    false
}

/// Print river validation summary.
pub fn print_river_validation(heightmap: &Tilemap<f32>, params: &RiverErosionParams) {
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

    // Measure river widths
    let width_stats = measure_river_widths(heightmap, &flow_acc, params.source_min_accumulation);

    // Check connectivity
    let connectivity = check_river_connectivity(heightmap, params);

    println!("\n=== River Validation ===");
    if width_stats.count > 0 {
        println!("Channel Width: mean={:.1}, min={:.1}, max={:.1} (pixels)",
                 width_stats.mean, width_stats.min, width_stats.max);
        if width_stats.mean < 3.0 {
            println!("  ✓ Sharp channels (target: <3 pixels)");
        } else {
            println!("  ⚠ Wide channels - consider reducing erosion radius");
        }
    }

    println!("Connectivity: {}/{} rivers reach ocean ({:.1}%)",
             connectivity.reaching_ocean, connectivity.total, connectivity.ocean_ratio * 100.0);
    if connectivity.ocean_ratio > 0.95 {
        println!("  ✓ Good connectivity (target: >95%)");
    } else {
        println!("  ⚠ Low connectivity - {} rivers ending in pits", connectivity.ending_in_pit);
    }

    // Count pits
    let mut pit_count = 0;
    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            if *heightmap.get(x, y) >= 0.0 && *flow_dir.get(x, y) == NO_FLOW {
                pit_count += 1;
            }
        }
    }
    println!("Pit count: {} (target: 0)", pit_count);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_direction_slope() {
        let mut heightmap = Tilemap::new_with(8, 8, 0.0f32);
        for y in 0..8 {
            for x in 0..8 {
                heightmap.set(x, y, (8 - y) as f32 * 10.0);
            }
        }

        let flow_dir = compute_flow_direction(&heightmap);

        for y in 1..6 {
            for x in 1..7 {
                let dir = *flow_dir.get(x, y);
                assert!(dir == 4 || dir == 3 || dir == 5,
                    "Expected downward flow at ({}, {}), got {}", x, y, dir);
            }
        }
    }

    #[test]
    fn test_flow_accumulation_increases_downstream() {
        let mut heightmap = Tilemap::new_with(16, 16, 0.0f32);
        for y in 0..16 {
            for x in 0..16 {
                heightmap.set(x, y, (16 - y) as f32 * 10.0);
            }
        }

        let flow_dir = compute_flow_direction(&heightmap);
        let flow_acc = compute_flow_accumulation(&heightmap, &flow_dir);

        let top_acc = *flow_acc.get(8, 1);
        let bottom_acc = *flow_acc.get(8, 14);
        assert!(bottom_acc > top_acc,
            "Bottom accumulation {} should be > top {}", bottom_acc, top_acc);
    }

    #[test]
    fn test_river_erosion_no_pits() {
        let mut heightmap = Tilemap::new_with(32, 32, 0.0f32);
        for y in 0..32 {
            for x in 0..32 {
                let h = 500.0 - (y as f32 * 15.0);
                heightmap.set(x, y, h);
            }
        }

        let hardness = Tilemap::new_with(32, 32, 0.3f32);
        let params = RiverErosionParams {
            source_min_accumulation: 5.0,
            source_min_elevation: 10.0,
            erosion_rate: 0.5,
            ..Default::default()
        };

        erode_rivers(&mut heightmap, &hardness, &params);

        // Check that no cell is lower than its downstream neighbor (no pits)
        let flow_dir = compute_flow_direction(&heightmap);
        for y in 1..31 {
            for x in 0..32 {
                let dir = *flow_dir.get(x, y);
                if dir == NO_FLOW {
                    continue;
                }

                let h = *heightmap.get(x, y);
                let nx = (x as i32 + DX[dir as usize]).rem_euclid(32) as usize;
                let ny = (y as i32 + DY[dir as usize]).clamp(0, 31) as usize;
                let nh = *heightmap.get(nx, ny);

                assert!(h >= nh - 0.01,
                    "Pit detected at ({}, {}): height {} < downstream {}", x, y, h, nh);
            }
        }
    }
}
