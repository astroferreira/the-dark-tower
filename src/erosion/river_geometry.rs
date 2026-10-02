//! Bezier Curve Rivers System (Phase 1)
//!
//! Replaces implicit flow-accumulation rivers with explicit Bezier curve geometry
//! for smooth, natural-looking waterways with proper width variation and confluences.

use crate::tilemap::Tilemap;
use super::rivers::{compute_flow_direction, compute_flow_accumulation, DX, DY, NO_FLOW};
use noise::{NoiseFn, Perlin, Seedable};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// A control point along a river's Bezier path
#[derive(Clone, Debug)]
pub struct RiverControlPoint {
    /// World X coordinate (can be fractional for smooth interpolation)
    pub world_x: f32,
    /// World Y coordinate
    pub world_y: f32,
    /// Flow accumulation at this point (determines river size)
    pub flow_accumulation: f32,
    /// River width at this point (in tiles)
    pub width: f32,
    /// Elevation at this point
    pub elevation: f32,
}

impl RiverControlPoint {
    pub fn new(x: f32, y: f32, flow: f32, width: f32, elevation: f32) -> Self {
        Self {
            world_x: x,
            world_y: y,
            flow_accumulation: flow,
            width,
            elevation,
        }
    }

    /// Interpolate between two control points
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            world_x: self.world_x + (other.world_x - self.world_x) * t,
            world_y: self.world_y + (other.world_y - self.world_y) * t,
            flow_accumulation: self.flow_accumulation + (other.flow_accumulation - self.flow_accumulation) * t,
            width: self.width + (other.width - self.width) * t,
            elevation: self.elevation + (other.elevation - self.elevation) * t,
        }
    }
}

/// A cubic Bezier segment of a river
#[derive(Clone, Debug)]
pub struct BezierRiverSegment {
    /// Start point (P0)
    pub p0: RiverControlPoint,
    /// First control point (P1)
    pub p1: RiverControlPoint,
    /// Second control point (P2)
    pub p2: RiverControlPoint,
    /// End point (P3)
    pub p3: RiverControlPoint,
    /// Indices of tributary segments that join at p0
    pub tributaries: Vec<usize>,
    /// Unique ID for this segment
    pub id: usize,
    /// Strahler stream order (1 for headwaters, increasing at confluences)
    pub stream_order: usize,
}

impl BezierRiverSegment {
    /// Evaluate the Bezier curve at parameter t (0.0 to 1.0)
    pub fn evaluate(&self, t: f32) -> RiverControlPoint {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        // Cubic Bezier formula: B(t) = (1-t)^3*P0 + 3*(1-t)^2*t*P1 + 3*(1-t)*t^2*P2 + t^3*P3
        let w0 = mt3;
        let w1 = 3.0 * mt2 * t;
        let w2 = 3.0 * mt * t2;
        let w3 = t3;

        RiverControlPoint {
            world_x: w0 * self.p0.world_x + w1 * self.p1.world_x + w2 * self.p2.world_x + w3 * self.p3.world_x,
            world_y: w0 * self.p0.world_y + w1 * self.p1.world_y + w2 * self.p2.world_y + w3 * self.p3.world_y,
            flow_accumulation: w0 * self.p0.flow_accumulation + w1 * self.p1.flow_accumulation + w2 * self.p2.flow_accumulation + w3 * self.p3.flow_accumulation,
            width: w0 * self.p0.width + w1 * self.p1.width + w2 * self.p2.width + w3 * self.p3.width,
            elevation: w0 * self.p0.elevation + w1 * self.p1.elevation + w2 * self.p2.elevation + w3 * self.p3.elevation,
        }
    }

    /// Get the tangent vector at parameter t
    pub fn tangent(&self, t: f32) -> (f32, f32) {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;

        // Derivative of cubic Bezier
        let w0 = -3.0 * mt2;
        let w1 = 3.0 * mt2 - 6.0 * mt * t;
        let w2 = 6.0 * mt * t - 3.0 * t2;
        let w3 = 3.0 * t2;

        let dx = w0 * self.p0.world_x + w1 * self.p1.world_x + w2 * self.p2.world_x + w3 * self.p3.world_x;
        let dy = w0 * self.p0.world_y + w1 * self.p1.world_y + w2 * self.p2.world_y + w3 * self.p3.world_y;

        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0001 {
            (dx / len, dy / len)
        } else {
            (1.0, 0.0)
        }
    }

    /// Get perpendicular vector (for river width)
    pub fn perpendicular(&self, t: f32) -> (f32, f32) {
        let (tx, ty) = self.tangent(t);
        (-ty, tx)
    }

    /// Get length of the segment (approximation by sampling)
    pub fn approximate_length(&self, samples: usize) -> f32 {
        let mut length = 0.0;
        let mut prev = self.evaluate(0.0);

        for i in 1..=samples {
            let t = i as f32 / samples as f32;
            let current = self.evaluate(t);
            let dx = current.world_x - prev.world_x;
            let dy = current.world_y - prev.world_y;
            length += (dx * dx + dy * dy).sqrt();
            prev = current;
        }

        length
    }
}

/// A confluence point where rivers merge
#[derive(Clone, Debug)]
pub struct ConfluencePoint {
    /// World position
    pub x: f32,
    pub y: f32,
    /// Indices of river segments meeting here
    pub segment_indices: Vec<usize>,
    /// Combined flow after confluence
    pub combined_flow: f32,
}

/// The complete river network
#[derive(Clone, Debug)]
pub struct RiverNetwork {
    /// All Bezier segments in the network
    pub segments: Vec<BezierRiverSegment>,
    /// Confluence points where rivers merge
    pub confluences: Vec<ConfluencePoint>,
    /// Source points (river headwaters)
    pub sources: Vec<(usize, usize)>,
    /// Parameters used to generate this network
    pub params: RiverNetworkParams,
}

/// Parameters for river network generation
#[derive(Clone, Debug)]
pub struct RiverNetworkParams {
    /// Minimum flow accumulation to be considered a river source
    pub source_threshold: f32,
    /// Maximum flow accumulation for source selection (avoid mid-river starts)
    pub source_max_threshold: f32,
    /// Minimum elevation for river sources
    pub min_source_elevation: f32,
    /// Points per Bezier segment (controls smoothness)
    pub points_per_segment: usize,
    /// Perpendicular noise amplitude for meandering (0.0-1.0)
    pub meander_amplitude: f32,
    /// Meander frequency (higher = more curves per unit length)
    pub meander_frequency: f32,
    /// Maximum curvature constraint (radians per unit length)
    pub max_curvature: f32,
    /// Base river width at minimum flow
    pub base_width: f32,
    /// Width scaling exponent (typically 0.4-0.6 for hydraulic geometry)
    pub width_exponent: f32,
}

impl Default for RiverNetworkParams {
    fn default() -> Self {
        Self {
            source_threshold: 12.0,
            source_max_threshold: 50000.0,
            min_source_elevation: 5.0,
            points_per_segment: 5,
            meander_amplitude: 0.30,
            meander_frequency: 0.12,
            max_curvature: 0.3,
            base_width: 0.35,
            width_exponent: 0.45,
        }
    }
}

// =============================================================================
// RIVER NETWORK GENERATION
// =============================================================================

impl RiverNetwork {
    /// Create an empty river network
    pub fn new(params: RiverNetworkParams) -> Self {
        Self {
            segments: Vec::new(),
            confluences: Vec::new(),
            sources: Vec::new(),
            params,
        }
    }

    /// Get the number of river segments
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Get total river length (sum of all segments)
    pub fn total_length(&self) -> f32 {
        self.segments.iter().map(|s| s.approximate_length(10)).sum()
    }

    /// Find the segment closest to a point
    pub fn find_nearest_segment(&self, x: f32, y: f32) -> Option<(usize, f32)> {
        let mut best_dist = f32::MAX;
        let mut best_idx = None;

        for (idx, segment) in self.segments.iter().enumerate() {
            // Sample along the segment
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let pt = segment.evaluate(t);
                let dx = pt.world_x - x;
                let dy = pt.world_y - y;
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(idx);
                }
            }
        }

        best_idx.map(|idx| (idx, best_dist.sqrt()))
    }

    /// Get width at a world position (returns 0 if not on a river)
    pub fn get_width_at(&self, x: f32, y: f32, tolerance: f32) -> f32 {
        for segment in &self.segments {
            // Sample along the segment
            for i in 0..=20 {
                let t = i as f32 / 20.0;
                let pt = segment.evaluate(t);
                let dx = pt.world_x - x;
                let dy = pt.world_y - y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < pt.width + tolerance {
                    return pt.width;
                }
            }
        }
        0.0
    }
    
    /// Check if there's a significant river flow at this tile (for pathfinding penalty)
    pub fn has_significant_flow(&self, x: usize, y: usize) -> bool {
        // Rivers with width > 0.5 are significant obstacles
        self.get_width_at(x as f32, y as f32, 1.0) > 0.5
    }

    /// Pre-compute a boolean tilemap marking all tiles that have significant river flow.
    /// This turns O(segments × 21) per-pixel lookups into O(1) lookups.
    pub fn build_tile_cache(&self, width: usize, height: usize) -> Tilemap<bool> {
        let mut cache = Tilemap::new_with(width, height, false);
        let tolerance = 1.0f32;

        for segment in &self.segments {
            let length = segment.approximate_length(10);
            // Sample densely: ~2 samples per unit length, minimum 50
            let samples = ((length * 2.0) as usize + 10).max(50);

            for i in 0..=samples {
                let t = i as f32 / samples as f32;
                let pt = segment.evaluate(t);
                let half_width = pt.width / 2.0;
                let radius = half_width + tolerance;

                // Stamp all tiles within radius
                let min_x = (pt.world_x - radius).floor() as i32;
                let max_x = (pt.world_x + radius).ceil() as i32;
                let min_y = (pt.world_y - radius).floor() as i32;
                let max_y = (pt.world_y + radius).ceil() as i32;

                for ty in min_y..=max_y {
                    if ty < 0 || ty >= height as i32 {
                        continue;
                    }
                    for tx in min_x..=max_x {
                        let wx = tx.rem_euclid(width as i32) as usize;
                        let wy = ty as usize;
                        if !*cache.get(wx, wy) {
                            let dx = tx as f32 - pt.world_x;
                            let dy = ty as f32 - pt.world_y;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist < pt.width + tolerance && pt.width > 0.5 {
                                cache.set(wx, wy, true);
                            }
                        }
                    }
                }
            }
        }

        cache
    }
}

/// Generate a Bezier river network from flow accumulation data
pub fn generate_river_network(
    heightmap: &Tilemap<f32>,
    flow_accumulation: &Tilemap<f32>,
    flow_direction: &Tilemap<u8>,
    water_map: Option<&Tilemap<crate::water_bodies::WaterBodyId>>,
    water_bodies: Option<&[crate::water_bodies::WaterBody]>,
    params: &RiverNetworkParams,
    seed: u64,
) -> RiverNetwork {
    let mut network = RiverNetwork::new(params.clone());
    let noise = Perlin::new(1).set_seed(seed as u32);
    let width = heightmap.width;
    let height = heightmap.height;

    // Find true river sources (headwaters where channels initiate)
    let mut sources = find_river_sources(heightmap, flow_accumulation, flow_direction, params);

    // Also include lake spillway outlets for exorheic lakes
    if let Some(bodies) = water_bodies {
        for body in bodies {
            if body.body_type == crate::water_bodies::WaterBodyType::Lake && !body.is_endorheic {
                if let Some(spillway) = body.spillway {
                    if !sources.contains(&spillway) && *heightmap.get(spillway.0, spillway.1) >= 0.0 {
                        sources.push(spillway);
                    }
                }
            }
        }
    }

    network.sources = sources.clone();

    // Track which simulation cells have been claimed by existing river segments (storing the segment ID)
    let mut cell_to_segment: Tilemap<Option<usize>> = Tilemap::new_with(width, height, None);
    let mut tributary_links: Vec<(usize, usize)> = Vec::new(); // (downstream_seg_id, incoming_trib_id)
    let mut confluences: Vec<ConfluencePoint> = Vec::new();
    let mut segment_id = 0;

    // Trace each river from source to ocean/confluence/lake
    for (sx, sy) in &sources {
        if cell_to_segment.get(*sx, *sy).is_some() {
            continue;
        }

        let segments = trace_river_bezier(
            heightmap,
            flow_accumulation,
            flow_direction,
            water_map,
            *sx, *sy,
            params,
            &noise,
            seed,
            &mut cell_to_segment,
            &mut segment_id,
            &mut confluences,
            &mut tributary_links,
        );

        network.segments.extend(segments);
    }

    // Connect tributary links into the downstream segments' tributaries lists
    for &(target_id, trib_id) in &tributary_links {
        if let Some(target_seg) = network.segments.iter_mut().find(|s| s.id == target_id) {
            if !target_seg.tributaries.contains(&trib_id) {
                target_seg.tributaries.push(trib_id);
            }
        }
    }

    network.confluences = confluences;
    compute_strahler_orders(&mut network.segments);

    network
}

/// Find true river source points (headwaters where channels initiate)
fn find_river_sources(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    params: &RiverNetworkParams,
) -> Vec<(usize, usize)> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut sources = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            if h >= params.min_source_elevation
                && acc >= params.source_threshold
                && acc < params.source_max_threshold
            {
                // Check if any upstream neighbor cell flows into (x, y) with accumulation >= source_threshold
                let mut has_upstream_channel = false;
                for (nx, ny) in heightmap.neighbors_8(x, y) {
                    let dir = *flow_dir.get(nx, ny);
                    if dir != NO_FLOW && (dir as usize) < 8 {
                        let target_x = (nx as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
                        let target_y = ny as i32 + DY[dir as usize];
                        if target_x == x && target_y == y as i32 {
                            if *flow_acc.get(nx, ny) >= params.source_threshold {
                                has_upstream_channel = true;
                                break;
                            }
                        }
                    }
                }

                if !has_upstream_channel {
                    sources.push((x, y));
                }
            }
        }
    }

    // Sort by accumulation descending: trace major drainage basins first
    // so tributaries terminate into existing trunks seamlessly
    sources.sort_by(|a, b| {
        let acc_a = *flow_acc.get(a.0, a.1);
        let acc_b = *flow_acc.get(b.0, b.1);
        acc_b.partial_cmp(&acc_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    sources
}

/// Trace a single river from source to ocean, lake, or existing river confluence
fn trace_river_bezier(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    water_map: Option<&Tilemap<crate::water_bodies::WaterBodyId>>,
    start_x: usize,
    start_y: usize,
    params: &RiverNetworkParams,
    noise: &Perlin,
    seed: u64,
    cell_to_segment: &mut Tilemap<Option<usize>>,
    segment_id: &mut usize,
    confluences: &mut Vec<ConfluencePoint>,
    tributary_links: &mut Vec<(usize, usize)>,
) -> Vec<BezierRiverSegment> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut segments = Vec::new();

    // Collect raw path points, splitting into contiguous subpaths at periodic X wrap boundary
    let mut all_paths: Vec<Vec<RiverControlPoint>> = Vec::new();
    let mut current_path: Vec<RiverControlPoint> = Vec::new();
    let mut local_visited = std::collections::HashSet::new();
    let mut x = start_x;
    let mut y = start_y;
    let max_steps = width * height;
    let mut joined_segment_id: Option<usize> = None;

    for step in 0..max_steps {
        if !local_visited.insert((x, y)) {
            break;
        }

        let h = *heightmap.get(x, y);
        let acc = *flow_acc.get(x, y);
        let river_width = calculate_river_width(acc, params);

        current_path.push(RiverControlPoint::new(
            x as f32,
            y as f32,
            acc,
            river_width,
            h,
        ));

        // Reached ocean coastline? (Coastline is at 0.0)
        if h <= 0.0 {
            break;
        }

        // Reached a lake? (Only stop if step > 0 to allow spillway sources to emerge)
        if step > 0 {
            if let Some(wm) = water_map {
                if wm.get(x, y).is_lake() {
                    break;
                }
            }
        }

        // Reached an existing river channel? (Confluence!)
        if step > 0 {
            if let Some(existing_seg_id) = *cell_to_segment.get(x, y) {
                joined_segment_id = Some(existing_seg_id);
                confluences.push(ConfluencePoint {
                    x: x as f32,
                    y: y as f32,
                    segment_indices: vec![existing_seg_id],
                    combined_flow: acc,
                });
                break;
            }
        }

        // Get flow direction
        let dir = *flow_dir.get(x, y);
        if dir == NO_FLOW || dir >= 8 {
            break;
        }

        // Move to next cell
        let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
        let ny = y as i32 + DY[dir as usize];

        if ny < 0 || ny >= height as i32 {
            break;
        }
        let ny = ny as usize;

        // Check if next step (nx, ny) flows into an existing river channel (confluence)
        if step > 0 {
            if let Some(existing_seg_id) = *cell_to_segment.get(nx, ny) {
                let next_h = *heightmap.get(nx, ny);
                let next_acc = *flow_acc.get(nx, ny);
                let next_w = calculate_river_width(next_acc, params);
                current_path.push(RiverControlPoint::new(
                    nx as f32,
                    ny as f32,
                    next_acc,
                    next_w,
                    next_h,
                ));
                joined_segment_id = Some(existing_seg_id);
                confluences.push(ConfluencePoint {
                    x: nx as f32,
                    y: ny as f32,
                    segment_indices: vec![existing_seg_id],
                    combined_flow: next_acc,
                });
                break;
            }
        }

        // If crossing periodic boundary (e.g. x=0 <-> x=width-1), split into separate subpath
        if (nx as i32 - x as i32).abs() > 1 {
            if current_path.len() >= 2 {
                all_paths.push(std::mem::take(&mut current_path));
            } else {
                current_path.clear();
            }
        }

        x = nx;
        y = ny;
    }

    if current_path.len() >= 2 {
        all_paths.push(current_path);
    }

    let num_paths = all_paths.len();
    let mut prev_subseg_id: Option<usize> = None;

    for (path_idx, path_points) in all_paths.into_iter().enumerate() {
        if path_points.len() < 2 {
            continue;
        }

        // Apply meandering noise to path points
        let meander_points = apply_meander(&path_points, params, noise, seed);

        // Create Bezier segments from points
        let points_per_seg = params.points_per_segment.max(2);
        let mut i = 0;

        while i + 1 < meander_points.len() {
            let end_i = (i + points_per_seg).min(meander_points.len() - 1);

            if end_i <= i {
                break;
            }

            let mut segment = create_bezier_segment(
                &meander_points,
                i,
                end_i,
                *segment_id,
            );

            // Connect upstream segment of the same path as incoming stream
            if let Some(prev_id) = prev_subseg_id {
                segment.tributaries.push(prev_id);
            }

            // Register exact simulation grid cells covered by this segment in cell_to_segment
            for pt in &path_points[i..=end_i] {
                let cx = (pt.world_x.round() as usize).rem_euclid(width);
                let cy = (pt.world_y.round() as usize).clamp(0, height - 1);
                cell_to_segment.set(cx, cy, Some(segment.id));
            }

            // If this is the final segment of the final subpath and we joined an existing river:
            let is_final = path_idx == num_paths - 1 && end_i == meander_points.len() - 1;
            if is_final {
                if let Some(target_seg_id) = joined_segment_id {
                    tributary_links.push((target_seg_id, segment.id));
                    if let Some(conf) = confluences.last_mut() {
                        if !conf.segment_indices.contains(&segment.id) {
                            conf.segment_indices.push(segment.id);
                        }
                    }
                }
            }

            prev_subseg_id = Some(segment.id);
            segments.push(segment);
            *segment_id += 1;
            i = end_i;
        }
    }

    segments
}

/// Apply meandering noise to path points
fn apply_meander(
    points: &[RiverControlPoint],
    params: &RiverNetworkParams,
    noise: &Perlin,
    seed: u64,
) -> Vec<RiverControlPoint> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len());

    for (i, pt) in points.iter().enumerate() {
        if i == 0 || i == points.len() - 1 {
            // Keep endpoints fixed
            result.push(pt.clone());
            continue;
        }

        // Calculate tangent from neighbors
        let prev = &points[i - 1];
        let next = &points[i + 1];
        let tx = next.world_x - prev.world_x;
        let ty = next.world_y - prev.world_y;
        let len = (tx * tx + ty * ty).sqrt();

        if len < 0.001 {
            result.push(pt.clone());
            continue;
        }

        // Perpendicular direction
        let px = -ty / len;
        let py = tx / len;

        // Sample noise for meander offset
        let noise_x = pt.world_x * params.meander_frequency as f32;
        let noise_y = pt.world_y * params.meander_frequency as f32;
        let noise_val = noise.get([noise_x as f64, noise_y as f64, seed as f64 * 0.001]) as f32;

        // Apply offset perpendicular to flow
        // Scale by width (wider rivers meander more)
        let offset_scale = params.meander_amplitude * pt.width * 2.0;
        let offset = noise_val * offset_scale;

        result.push(RiverControlPoint::new(
            pt.world_x + px * offset,
            pt.world_y + py * offset,
            pt.flow_accumulation,
            pt.width,
            pt.elevation,
        ));
    }

    result
}

/// Create a Bezier segment from a sequence of points
fn create_bezier_segment(
    points: &[RiverControlPoint],
    start_i: usize,
    end_i: usize,
    id: usize,
) -> BezierRiverSegment {
    let p0 = points[start_i].clone();
    let p3 = points[end_i].clone();

    // Calculate control points for smooth curve
    // Using 1/3 rule for natural curves
    let third = (end_i - start_i) as f32 / 3.0;
    let p1_i = start_i + (third as usize).max(1).min(end_i - start_i - 1);
    let p2_i = end_i.saturating_sub((third as usize).max(1));

    let p1 = if p1_i < points.len() {
        points[p1_i].clone()
    } else {
        p0.lerp(&p3, 0.33)
    };

    let p2 = if p2_i < points.len() && p2_i != p1_i {
        points[p2_i].clone()
    } else {
        p0.lerp(&p3, 0.67)
    };

    BezierRiverSegment {
        p0,
        p1,
        p2,
        p3,
        tributaries: Vec::new(),
        id,
        stream_order: 1,
    }
}

/// Calculate river width from flow accumulation using hydraulic geometry
fn calculate_river_width(flow_acc: f32, params: &RiverNetworkParams) -> f32 {
    // Width scales with flow^exponent (typically 0.45 for natural rivers)
    let flow_ratio = (flow_acc / params.source_threshold).max(1.0);
    let width = params.base_width * flow_ratio.powf(params.width_exponent);

    // Clamp: 0.35 (capillary brook) to 3.5 tiles max width (major trunk)
    width.clamp(0.35, 3.5)
}

/// Find confluence points where rivers merge
fn find_confluences(
    segments: &[BezierRiverSegment],
    _width: usize,
    _height: usize,
) -> Vec<ConfluencePoint> {
    let mut confluences = Vec::new();
    let merge_threshold = 3.0;

    for (i, seg_i) in segments.iter().enumerate() {
        let end_pt = &seg_i.p3;
        for (j, seg_j) in segments.iter().enumerate() {
            if i >= j {
                continue;
            }

            let start_j = &seg_j.p0;
            let dx = end_pt.world_x - start_j.world_x;
            let dy = end_pt.world_y - start_j.world_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < merge_threshold {
                let combined_flow = end_pt.flow_accumulation + start_j.flow_accumulation;
                let existing = confluences.iter_mut().find(|c: &&mut ConfluencePoint| {
                    let cdx = c.x - end_pt.world_x;
                    let cdy = c.y - end_pt.world_y;
                    (cdx * cdx + cdy * cdy).sqrt() < merge_threshold
                });

                if let Some(conf) = existing {
                    if !conf.segment_indices.contains(&i) {
                        conf.segment_indices.push(i);
                    }
                    if !conf.segment_indices.contains(&j) {
                        conf.segment_indices.push(j);
                    }
                    conf.combined_flow = conf.combined_flow.max(combined_flow);
                } else {
                    confluences.push(ConfluencePoint {
                        x: (end_pt.world_x + start_j.world_x) / 2.0,
                        y: (end_pt.world_y + start_j.world_y) / 2.0,
                        segment_indices: vec![i, j],
                        combined_flow,
                    });
                }
            }
        }
    }

    confluences
}

/// Compute exact Horton-Strahler stream orders across all segments in the network
fn compute_strahler_orders(segments: &mut [BezierRiverSegment]) {
    for _ in 0..20 {
        let mut changed = false;
        let prev_orders: std::collections::HashMap<usize, usize> =
            segments.iter().map(|s| (s.id, s.stream_order)).collect();

        for seg in segments.iter_mut() {
            if seg.tributaries.is_empty() {
                if seg.stream_order != 1 {
                    seg.stream_order = 1;
                    changed = true;
                }
            } else {
                let mut max_order = 1;
                let mut count_max = 0;
                for &trib_id in &seg.tributaries {
                    let trib_order = prev_orders.get(&trib_id).copied().unwrap_or(1);
                    if trib_order > max_order {
                        max_order = trib_order;
                        count_max = 1;
                    } else if trib_order == max_order {
                        count_max += 1;
                    }
                }

                let new_order = if count_max >= 2 { max_order + 1 } else { max_order };
                if new_order != seg.stream_order {
                    seg.stream_order = new_order;
                    changed = true;
                }
            }
        }

        if !changed {
            break;
        }
    }

    // Ensure stream order reflects both branching topology and massive accumulated discharge
    for seg in segments.iter_mut() {
        let max_acc = seg.p3.flow_accumulation.max(seg.p0.flow_accumulation);
        if max_acc > 1200.0 {
            seg.stream_order = seg.stream_order.max(4);
        } else if max_acc > 250.0 {
            seg.stream_order = seg.stream_order.max(3);
        } else if max_acc > 50.0 {
            seg.stream_order = seg.stream_order.max(2);
        }
    }
}

// =============================================================================
// RIVER RASTERIZATION
// =============================================================================

/// Rasterize the river network to a tilemap (for visualization or application)
pub fn rasterize_river_network(
    network: &RiverNetwork,
    width: usize,
    height: usize,
) -> Tilemap<f32> {
    let mut river_map = Tilemap::new_with(width, height, 0.0f32);

    for segment in &network.segments {
        rasterize_segment(segment, &mut river_map);
    }

    river_map
}

/// Rasterize a single Bezier segment
fn rasterize_segment(segment: &BezierRiverSegment, river_map: &mut Tilemap<f32>) {
    let length = segment.approximate_length(10);
    let samples = (length * 2.0) as usize + 10; // 2 samples per unit length

    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let pt = segment.evaluate(t);
        let (px, py) = segment.perpendicular(t);
        let half_width = pt.width / 2.0;

        // Draw perpendicular line at this point
        let steps = (pt.width * 2.0) as i32 + 1;
        for s in -steps..=steps {
            let offset = s as f32 / steps as f32 * half_width;
            let rx = pt.world_x + px * offset;
            let ry = pt.world_y + py * offset;

            // Convert to tile coordinates
            let tx = rx.round() as i32;
            let ty = ry.round() as i32;

            if tx >= 0 && tx < river_map.width as i32 && ty >= 0 && ty < river_map.height as i32 {
                let dist_from_center = offset.abs() / half_width;
                let intensity = 1.0 - dist_from_center * dist_from_center; // Smooth falloff
                let current = *river_map.get(tx as usize, ty as usize);
                river_map.set(tx as usize, ty as usize, current.max(intensity.max(0.0)));
            }
        }
    }
}

// =============================================================================
// INTEGRATION HELPERS
// =============================================================================

/// Trace rivers with pre-computed flow accumulation and direction, respecting lakes and terrain boundaries.
pub fn trace_bezier_rivers_with_flow(
    heightmap: &Tilemap<f32>,
    flow_accumulation: &Tilemap<f32>,
    flow_direction: &Tilemap<u8>,
    water_map: Option<&Tilemap<crate::water_bodies::WaterBodyId>>,
    water_bodies: Option<&[crate::water_bodies::WaterBody]>,
    params: Option<RiverNetworkParams>,
    seed: u64,
) -> RiverNetwork {
    let params = params.unwrap_or_default();
    generate_river_network(
        heightmap,
        flow_accumulation,
        flow_direction,
        water_map,
        water_bodies,
        &params,
        seed,
    )
}

/// Trace rivers from heightmap and create a Bezier network
/// This is the convenience entry point for integration with the world generation pipeline
pub fn trace_bezier_rivers(
    heightmap: &Tilemap<f32>,
    params: Option<RiverNetworkParams>,
    seed: u64,
) -> RiverNetwork {
    let params = params.unwrap_or_default();
    let (filled, flat_dir) = super::rivers::fill_depressions_and_route(heightmap);
    let mut flow_dir = compute_flow_direction(&filled);
    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let fd = *flat_dir.get(x, y);
            if fd != NO_FLOW {
                flow_dir.set(x, y, fd);
            }
        }
    }
    let flow_acc = compute_flow_accumulation(&filled, &flow_dir);
    trace_bezier_rivers_with_flow(heightmap, &flow_acc, &flow_dir, None, None, Some(params), seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_evaluation() {
        let p0 = RiverControlPoint::new(0.0, 0.0, 100.0, 1.0, 100.0);
        let p1 = RiverControlPoint::new(1.0, 2.0, 150.0, 1.5, 80.0);
        let p2 = RiverControlPoint::new(3.0, 2.0, 200.0, 2.0, 60.0);
        let p3 = RiverControlPoint::new(4.0, 0.0, 250.0, 2.5, 40.0);

        let segment = BezierRiverSegment {
            p0,
            p1,
            p2,
            p3,
            tributaries: vec![],
            id: 0,
            stream_order: 1,
        };

        // Test endpoints
        let start = segment.evaluate(0.0);
        assert!((start.world_x - 0.0).abs() < 0.001);
        assert!((start.world_y - 0.0).abs() < 0.001);

        let end = segment.evaluate(1.0);
        assert!((end.world_x - 4.0).abs() < 0.001);
        assert!((end.world_y - 0.0).abs() < 0.001);

        // Test midpoint is smooth
        let mid = segment.evaluate(0.5);
        assert!(mid.world_x > 0.0 && mid.world_x < 4.0);
    }

    #[test]
    fn test_river_width_calculation() {
        let params = RiverNetworkParams::default();

        // Small river
        let w1 = calculate_river_width(100.0, &params);
        // Large river
        let w2 = calculate_river_width(1000.0, &params);

        assert!(w2 > w1);
    }
}
