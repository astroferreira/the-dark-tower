//! Jittered Midpoint Subdivision for Shorelines (Phase 3a)
//!
//! Creates organic, irregular coastlines by recursively subdividing
//! coastline segments with perpendicular random offsets.

use noise::{NoiseFn, Perlin, Seedable};
use crate::tilemap::Tilemap;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// A single point on the coastline
#[derive(Clone, Debug)]
pub struct CoastlinePoint {
    pub x: f32,
    pub y: f32,
    /// How rough/jagged this section should be (0.0-1.0)
    pub roughness: f32,
    /// Whether this is a cliff (steep coastline)
    pub is_cliff: bool,
}

impl CoastlinePoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            roughness: 0.5,
            is_cliff: false,
        }
    }

    pub fn with_roughness(x: f32, y: f32, roughness: f32) -> Self {
        Self {
            x,
            y,
            roughness,
            is_cliff: false,
        }
    }
}

/// A segment of the coastline as a polyline
#[derive(Clone, Debug)]
pub struct CoastlineSegment {
    /// Ordered points along this segment
    pub points: Vec<CoastlinePoint>,
    /// Average roughness for this segment
    pub roughness: f32,
    /// Whether this segment is a cliff
    pub is_cliff: bool,
    /// Unique ID
    pub id: usize,
}

impl CoastlineSegment {
    pub fn new(points: Vec<CoastlinePoint>, id: usize) -> Self {
        let roughness = if points.is_empty() {
            0.5
        } else {
            points.iter().map(|p| p.roughness).sum::<f32>() / points.len() as f32
        };

        Self {
            points,
            roughness,
            is_cliff: false,
            id,
        }
    }

    /// Get the length of this segment
    pub fn length(&self) -> f32 {
        if self.points.len() < 2 {
            return 0.0;
        }

        let mut len = 0.0;
        for i in 1..self.points.len() {
            let dx = self.points[i].x - self.points[i - 1].x;
            let dy = self.points[i].y - self.points[i - 1].y;
            len += (dx * dx + dy * dy).sqrt();
        }
        len
    }
}

/// A bay (concave coastal feature)
#[derive(Clone, Debug)]
pub struct Bay {
    /// Center position
    pub x: f32,
    pub y: f32,
    /// Approximate size (width)
    pub size: f32,
    /// Depth into land
    pub depth: f32,
    /// Segment IDs that form this bay
    pub segment_ids: Vec<usize>,
}

/// A peninsula (convex coastal feature)
#[derive(Clone, Debug)]
pub struct Peninsula {
    /// Base position (where it connects to land)
    pub base_x: f32,
    pub base_y: f32,
    /// Tip position
    pub tip_x: f32,
    pub tip_y: f32,
    /// Width at base
    pub width: f32,
    /// Segment IDs that form this peninsula
    pub segment_ids: Vec<usize>,
}

/// The complete coastline network
#[derive(Clone, Debug)]
pub struct CoastlineNetwork {
    /// All coastline segments
    pub segments: Vec<CoastlineSegment>,
    /// Identified bays
    pub bays: Vec<Bay>,
    /// Identified peninsulas
    pub peninsulas: Vec<Peninsula>,
    /// Parameters used for generation
    pub params: CoastlineParams,
}

/// Parameters for coastline generation
#[derive(Clone, Debug)]
pub struct CoastlineParams {
    /// Number of subdivision iterations (typically 4-6)
    pub iterations: usize,
    /// Base jitter amplitude (fraction of segment length)
    pub base_jitter: f32,
    /// Jitter decay per iteration (typically 0.4-0.6)
    pub jitter_decay: f32,
    /// Perlin noise frequency for consistent variation
    pub noise_frequency: f64,
    /// Perlin noise amplitude multiplier
    pub noise_amplitude: f32,
    /// Minimum segment length to subdivide (in tiles)
    pub min_segment_length: f32,
    /// Douglas-Peucker simplification epsilon
    pub simplify_epsilon: f32,
    /// Blend width when applying to heightmap (in tiles)
    pub blend_width: f32,
    /// Curvature threshold for bay detection (radians)
    pub bay_curvature_threshold: f32,
    /// Curvature threshold for peninsula detection (radians)
    pub peninsula_curvature_threshold: f32,
}

impl Default for CoastlineParams {
    fn default() -> Self {
        Self {
            iterations: 5,
            base_jitter: 0.5,
            jitter_decay: 0.5,
            noise_frequency: 0.1,
            noise_amplitude: 0.3,
            min_segment_length: 2.0,
            simplify_epsilon: 2.0,
            blend_width: 3.0,
            bay_curvature_threshold: 0.5, // ~30 degrees
            peninsula_curvature_threshold: -0.3, // Convex
        }
    }
}

// =============================================================================
// COASTLINE EXTRACTION
// =============================================================================

/// Extract raw coastline from heightmap (threshold at sea level)
pub fn extract_coastline(
    heightmap: &Tilemap<f32>,
    sea_level: f32,
) -> Vec<(usize, usize)> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut coastline_pixels = Vec::new();

    for y in 1..height - 1 {
        for x in 0..width {
            let h = *heightmap.get(x, y);

            // Must be land
            if h < sea_level {
                continue;
            }

            // Check if any neighbor is water
            let is_coastal = heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                *heightmap.get(nx, ny) < sea_level
            });

            if is_coastal {
                coastline_pixels.push((x, y));
            }
        }
    }

    coastline_pixels
}

/// Convert coastline pixels to ordered polyline using contour tracing
fn trace_coastline_contour(
    pixels: &[(usize, usize)],
    width: usize,
    height: usize,
) -> Vec<Vec<CoastlinePoint>> {
    use std::collections::HashSet;

    let pixel_set: HashSet<(usize, usize)> = pixels.iter().copied().collect();
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut contours = Vec::new();

    // Direction vectors for Moore neighborhood tracing
    let dx = [0i32, 1, 1, 1, 0, -1, -1, -1];
    let dy = [-1i32, -1, 0, 1, 1, 1, 0, -1];

    for &start in pixels {
        if visited.contains(&start) {
            continue;
        }

        // Trace contour starting from this pixel
        let mut contour = Vec::new();
        let mut current = start;
        let mut prev_dir = 0usize;

        loop {
            if visited.contains(&current) && !contour.is_empty() {
                break;
            }

            visited.insert(current);
            contour.push(CoastlinePoint::new(current.0 as f32, current.1 as f32));

            // Find next pixel in contour (Moore neighborhood)
            let mut found = false;
            for i in 0..8 {
                let dir = (prev_dir + 5 + i) % 8; // Start from backtrack direction
                let nx = (current.0 as i32 + dx[dir]).rem_euclid(width as i32) as usize;
                let ny = (current.1 as i32 + dy[dir]).clamp(0, height as i32 - 1) as usize;

                if pixel_set.contains(&(nx, ny)) && !visited.contains(&(nx, ny)) {
                    current = (nx, ny);
                    prev_dir = dir;
                    found = true;
                    break;
                }
            }

            if !found {
                break;
            }

            // Limit contour length to prevent infinite loops
            if contour.len() > width * height {
                break;
            }
        }

        if contour.len() >= 3 {
            contours.push(contour);
        }
    }

    contours
}

// =============================================================================
// MIDPOINT SUBDIVISION
// =============================================================================

/// Apply midpoint subdivision to a polyline
fn subdivide_polyline(
    points: &[CoastlinePoint],
    noise: &Perlin,
    iteration: usize,
    params: &CoastlineParams,
    seed: u64,
) -> Vec<CoastlinePoint> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len() * 2);

    // Jitter amplitude decreases with each iteration
    let jitter_scale = params.base_jitter * params.jitter_decay.powi(iteration as i32);

    for i in 0..points.len() - 1 {
        let p1 = &points[i];
        let p2 = &points[i + 1];

        // Add first point
        result.push(p1.clone());

        // Calculate segment properties
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let len = (dx * dx + dy * dy).sqrt();

        // Skip subdivision if segment is too short
        if len < params.min_segment_length {
            continue;
        }

        // Midpoint
        let mx = (p1.x + p2.x) / 2.0;
        let my = (p1.y + p2.y) / 2.0;

        // Perpendicular direction
        let px = -dy / len;
        let py = dx / len;

        // Calculate jitter using Perlin noise for spatial coherence
        let noise_val = noise.get([
            mx as f64 * params.noise_frequency,
            my as f64 * params.noise_frequency,
            seed as f64 * 0.001 + iteration as f64,
        ]) as f32;

        // Combine noise with random jitter
        let jitter = noise_val * params.noise_amplitude + (noise_val * 2.0 - 1.0) * jitter_scale;
        let offset = len * jitter_scale * 0.5 * (1.0 + jitter);

        // Apply perpendicular offset
        let new_x = mx + px * offset;
        let new_y = my + py * offset;

        // Inherit roughness from neighbors
        let roughness = (p1.roughness + p2.roughness) / 2.0;

        result.push(CoastlinePoint::with_roughness(new_x, new_y, roughness));
    }

    // Add last point
    if let Some(last) = points.last() {
        result.push(last.clone());
    }

    result
}

/// Apply multiple iterations of midpoint subdivision
pub fn apply_midpoint_subdivision(
    points: &[CoastlinePoint],
    params: &CoastlineParams,
    seed: u64,
) -> Vec<CoastlinePoint> {
    let noise = Perlin::new(1).set_seed(seed as u32);

    let mut result = points.to_vec();

    for iteration in 0..params.iterations {
        result = subdivide_polyline(&result, &noise, iteration, params, seed);
    }

    result
}

// =============================================================================
// SIMPLIFICATION (Douglas-Peucker)
// =============================================================================

/// Simplify polyline using Douglas-Peucker algorithm
pub fn simplify_polyline(points: &[CoastlinePoint], epsilon: f32) -> Vec<CoastlinePoint> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut result = Vec::new();
    simplify_recursive(points, epsilon, &mut result);

    // Add last point
    if let Some(last) = points.last() {
        result.push(last.clone());
    }

    result
}

fn simplify_recursive(points: &[CoastlinePoint], epsilon: f32, result: &mut Vec<CoastlinePoint>) {
    if points.len() < 2 {
        return;
    }

    // Find point with maximum distance from line (first to last)
    let (max_dist, max_idx) = find_max_distance(points);

    if max_dist > epsilon {
        // Recursively simplify
        simplify_recursive(&points[..=max_idx], epsilon, result);
        simplify_recursive(&points[max_idx..], epsilon, result);
    } else {
        // Add first point only (last will be added by parent)
        result.push(points[0].clone());
    }
}

fn find_max_distance(points: &[CoastlinePoint]) -> (f32, usize) {
    if points.len() < 3 {
        return (0.0, 0);
    }

    let first = &points[0];
    let last = &points[points.len() - 1];

    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let len_sq = dx * dx + dy * dy;

    let mut max_dist = 0.0f32;
    let mut max_idx = 0;

    for i in 1..points.len() - 1 {
        let dist = if len_sq < 0.0001 {
            // First and last are same point - use direct distance
            let px = points[i].x - first.x;
            let py = points[i].y - first.y;
            (px * px + py * py).sqrt()
        } else {
            // Perpendicular distance to line
            let px = points[i].x - first.x;
            let py = points[i].y - first.y;
            let t = (px * dx + py * dy) / len_sq;
            let proj_x = first.x + t * dx;
            let proj_y = first.y + t * dy;
            let dist_x = points[i].x - proj_x;
            let dist_y = points[i].y - proj_y;
            (dist_x * dist_x + dist_y * dist_y).sqrt()
        };

        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    (max_dist, max_idx)
}

// =============================================================================
// FEATURE DETECTION
// =============================================================================

/// Detect bays and peninsulas from coastline curvature
fn detect_features(segments: &[CoastlineSegment], params: &CoastlineParams) -> (Vec<Bay>, Vec<Peninsula>) {
    let mut bays = Vec::new();
    let mut peninsulas = Vec::new();

    for segment in segments {
        if segment.points.len() < 5 {
            continue;
        }

        // Calculate curvature at each point
        let curvatures = calculate_curvatures(&segment.points);

        // Find regions of high positive curvature (bays) and negative (peninsulas)
        let mut i = 2;
        while i < curvatures.len() - 2 {
            let curv = curvatures[i];

            if curv > params.bay_curvature_threshold {
                // Potential bay - find extent
                let mut start = i;
                let mut end = i;
                while start > 0 && curvatures[start - 1] > params.bay_curvature_threshold * 0.5 {
                    start -= 1;
                }
                while end < curvatures.len() - 1 && curvatures[end + 1] > params.bay_curvature_threshold * 0.5 {
                    end += 1;
                }

                if end - start >= 3 {
                    let center_idx = (start + end) / 2;
                    let center = &segment.points[center_idx];

                    bays.push(Bay {
                        x: center.x,
                        y: center.y,
                        size: (end - start) as f32 * 2.0,
                        depth: curv * 10.0,
                        segment_ids: vec![segment.id],
                    });

                    i = end + 1;
                    continue;
                }
            } else if curv < params.peninsula_curvature_threshold {
                // Potential peninsula
                let mut start = i;
                let mut end = i;
                while start > 0 && curvatures[start - 1] < params.peninsula_curvature_threshold * 0.5 {
                    start -= 1;
                }
                while end < curvatures.len() - 1 && curvatures[end + 1] < params.peninsula_curvature_threshold * 0.5 {
                    end += 1;
                }

                if end - start >= 3 {
                    let base = &segment.points[start];
                    let tip_idx = (start + end) / 2;
                    let tip = &segment.points[tip_idx];

                    peninsulas.push(Peninsula {
                        base_x: base.x,
                        base_y: base.y,
                        tip_x: tip.x,
                        tip_y: tip.y,
                        width: (end - start) as f32,
                        segment_ids: vec![segment.id],
                    });

                    i = end + 1;
                    continue;
                }
            }

            i += 1;
        }
    }

    (bays, peninsulas)
}

/// Calculate discrete curvature at each point
fn calculate_curvatures(points: &[CoastlinePoint]) -> Vec<f32> {
    let mut curvatures = vec![0.0f32; points.len()];

    for i in 1..points.len() - 1 {
        let p0 = &points[i - 1];
        let p1 = &points[i];
        let p2 = &points[i + 1];

        // Vectors
        let v1x = p1.x - p0.x;
        let v1y = p1.y - p0.y;
        let v2x = p2.x - p1.x;
        let v2y = p2.y - p1.y;

        // Cross product gives signed curvature
        let cross = v1x * v2y - v1y * v2x;
        let len1 = (v1x * v1x + v1y * v1y).sqrt();
        let len2 = (v2x * v2x + v2y * v2y).sqrt();

        if len1 > 0.001 && len2 > 0.001 {
            curvatures[i] = cross / (len1 * len2);
        }
    }

    curvatures
}

// =============================================================================
// MAIN GENERATION
// =============================================================================

/// Generate coastline network from heightmap
pub fn generate_coastline_network(
    heightmap: &Tilemap<f32>,
    params: &CoastlineParams,
    seed: u64,
) -> CoastlineNetwork {
    let width = heightmap.width;
    let height = heightmap.height;

    // Step 1: Extract raw coastline pixels
    let coastline_pixels = extract_coastline(heightmap, 0.0);

    if coastline_pixels.is_empty() {
        return CoastlineNetwork {
            segments: Vec::new(),
            bays: Vec::new(),
            peninsulas: Vec::new(),
            params: params.clone(),
        };
    }

    // Step 2: Trace contours to get ordered polylines
    let contours = trace_coastline_contour(&coastline_pixels, width, height);

    // Step 3: Simplify and subdivide each contour
    let mut segments = Vec::new();
    for (id, contour) in contours.into_iter().enumerate() {
        // Simplify first
        let simplified = simplify_polyline(&contour, params.simplify_epsilon);

        // Apply midpoint subdivision
        let subdivided = apply_midpoint_subdivision(&simplified, params, seed);

        segments.push(CoastlineSegment::new(subdivided, id));
    }

    // Step 4: Detect bays and peninsulas
    let (bays, peninsulas) = detect_features(&segments, params);

    CoastlineNetwork {
        segments,
        bays,
        peninsulas,
        params: params.clone(),
    }
}

/// Apply coastline back to heightmap with smooth blending
pub fn apply_coastline_to_heightmap(
    coastline: &CoastlineNetwork,
    heightmap: &mut Tilemap<f32>,
    blend_width: f32,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Create distance field from coastline segments
    let mut distance_field = Tilemap::new_with(width, height, f32::MAX);

    for segment in &coastline.segments {
        for point in &segment.points {
            let px = point.x.round() as i32;
            let py = point.y.round() as i32;

            // Update distance for nearby cells
            let radius = (blend_width * 2.0) as i32;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let nx = (px + dx).rem_euclid(width as i32) as usize;
                    let ny = (py + dy).clamp(0, height as i32 - 1) as usize;

                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let current = *distance_field.get(nx, ny);
                    if dist < current {
                        distance_field.set(nx, ny, dist);
                    }
                }
            }
        }
    }

    // Apply blending based on distance
    for y in 0..height {
        for x in 0..width {
            let dist = *distance_field.get(x, y);
            if dist > blend_width {
                continue;
            }

            let h = *heightmap.get(x, y);
            let blend_factor = dist / blend_width;

            // Slightly modify elevation near coast for smooth transition
            if h > 0.0 && h < 20.0 {
                // Land near coast - add slight variation
                let adjustment = (1.0 - blend_factor) * 2.0 * (0.5 - (x + y) as f32 * 0.01 % 1.0);
                heightmap.set(x, y, (h + adjustment).max(0.1));
            } else if h < 0.0 && h > -20.0 {
                // Shallow water - add variation
                let adjustment = (1.0 - blend_factor) * 2.0 * ((x + y) as f32 * 0.01 % 1.0 - 0.5);
                heightmap.set(x, y, (h + adjustment).min(-0.1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coastline_point_creation() {
        let pt = CoastlinePoint::new(10.0, 20.0);
        assert_eq!(pt.x, 10.0);
        assert_eq!(pt.y, 20.0);
        assert_eq!(pt.roughness, 0.5);
    }

    #[test]
    fn test_simplify_polyline() {
        let points = vec![
            CoastlinePoint::new(0.0, 0.0),
            CoastlinePoint::new(1.0, 0.1),
            CoastlinePoint::new(2.0, 0.0),
            CoastlinePoint::new(3.0, 0.0),
        ];

        let simplified = simplify_polyline(&points, 0.2);
        assert!(simplified.len() <= points.len());
    }

    #[test]
    fn test_segment_length() {
        let points = vec![
            CoastlinePoint::new(0.0, 0.0),
            CoastlinePoint::new(3.0, 4.0),
        ];
        let segment = CoastlineSegment::new(points, 0);
        assert!((segment.length() - 5.0).abs() < 0.001);
    }
}
