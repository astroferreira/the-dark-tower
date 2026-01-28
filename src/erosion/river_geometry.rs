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
            source_threshold: 100.0,
            source_max_threshold: 300.0,
            min_source_elevation: 50.0,
            points_per_segment: 6,
            meander_amplitude: 0.5,
            meander_frequency: 0.15,
            max_curvature: 0.3,
            base_width: 1.0,
            width_exponent: 0.5,
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
    params: &RiverNetworkParams,
    seed: u64,
) -> RiverNetwork {
    let mut network = RiverNetwork::new(params.clone());
    let noise = Perlin::new(1).set_seed(seed as u32);
    let width = heightmap.width;
    let height = heightmap.height;

    // Find river sources (headwaters)
    let sources = find_river_sources(heightmap, flow_accumulation, params);
    network.sources = sources.clone();

    // Track which cells have been assigned to a segment
    let mut visited: Tilemap<bool> = Tilemap::new_with(width, height, false);
    let mut segment_id = 0;

    // Trace each river from source to ocean/confluence
    for (sx, sy) in &sources {
        if *visited.get(*sx, *sy) {
            continue;
        }

        let segments = trace_river_bezier(
            heightmap,
            flow_accumulation,
            flow_direction,
            *sx, *sy,
            params,
            &noise,
            seed,
            &mut visited,
            &mut segment_id,
        );

        network.segments.extend(segments);
    }

    // Find confluence points
    network.confluences = find_confluences(&network.segments, heightmap.width, heightmap.height);

    network
}

/// Find river source points (headwaters)
fn find_river_sources(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    params: &RiverNetworkParams,
) -> Vec<(usize, usize)> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut sources = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let acc = *flow_acc.get(x, y);

            // Must be above sea level with sufficient accumulation
            // but not too much (avoid starting mid-river)
            if h >= params.min_source_elevation
                && acc >= params.source_threshold
                && acc < params.source_max_threshold
            {
                sources.push((x, y));
            }
        }
    }

    // Sort by accumulation (larger first)
    sources.sort_by(|a, b| {
        let acc_a = *flow_acc.get(a.0, a.1);
        let acc_b = *flow_acc.get(b.0, b.1);
        acc_b.partial_cmp(&acc_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    sources
}

/// Trace a single river from source to ocean, creating Bezier segments
fn trace_river_bezier(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    start_x: usize,
    start_y: usize,
    params: &RiverNetworkParams,
    noise: &Perlin,
    seed: u64,
    visited: &mut Tilemap<bool>,
    segment_id: &mut usize,
) -> Vec<BezierRiverSegment> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut segments = Vec::new();

    // Collect raw path points
    let mut path_points: Vec<RiverControlPoint> = Vec::new();
    let mut x = start_x;
    let mut y = start_y;
    let max_steps = width * height;

    for _ in 0..max_steps {
        visited.set(x, y, true);

        let h = *heightmap.get(x, y);
        let acc = *flow_acc.get(x, y);
        let river_width = calculate_river_width(acc, params);

        path_points.push(RiverControlPoint::new(
            x as f32,
            y as f32,
            acc,
            river_width,
            h,
        ));

        // Reached ocean?
        if h < 0.0 {
            break;
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

        x = nx;
        y = ny;
    }

    // Convert path points to Bezier segments
    if path_points.len() < 2 {
        return segments;
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

        let segment = create_bezier_segment(
            &meander_points,
            i,
            end_i,
            *segment_id,
        );

        segments.push(segment);
        *segment_id += 1;
        i = end_i;
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
    }
}

/// Calculate river width from flow accumulation using hydraulic geometry
fn calculate_river_width(flow_acc: f32, params: &RiverNetworkParams) -> f32 {
    // Width scales with flow^exponent (typically 0.5 for natural rivers)
    // w = w0 * (Q/Q0)^b where Q is discharge (proxy: flow accumulation)
    let flow_ratio = (flow_acc / params.source_threshold).max(1.0);
    let width = params.base_width * flow_ratio.powf(params.width_exponent);

    // Clamp to reasonable range
    width.clamp(0.5, 12.0)
}

/// Find confluence points where rivers merge
fn find_confluences(
    segments: &[BezierRiverSegment],
    _width: usize,
    _height: usize,
) -> Vec<ConfluencePoint> {
    let mut confluences = Vec::new();
    let merge_threshold = 3.0; // Distance threshold for considering points as same confluence

    // Group segment endpoints
    for (i, seg_i) in segments.iter().enumerate() {
        let end_pt = &seg_i.p3;

        // Check if this endpoint is near another segment's start/end
        for (j, seg_j) in segments.iter().enumerate() {
            if i >= j {
                continue;
            }

            let start_j = &seg_j.p0;
            let dx = end_pt.world_x - start_j.world_x;
            let dy = end_pt.world_y - start_j.world_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < merge_threshold {
                // Found a confluence
                let combined_flow = end_pt.flow_accumulation + start_j.flow_accumulation;

                // Check if we already have a confluence near here
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

/// Trace rivers from heightmap and create a Bezier network
/// This is the main entry point for integration with the world generation pipeline
pub fn trace_bezier_rivers(
    heightmap: &Tilemap<f32>,
    params: Option<RiverNetworkParams>,
    seed: u64,
) -> RiverNetwork {
    let params = params.unwrap_or_default();

    // Compute flow direction and accumulation
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

    generate_river_network(heightmap, &flow_acc, &flow_dir, &params, seed)
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
