//! River edge crossing detection for region boundaries
//!
//! Identifies where rivers cross tile boundaries so adjacent regions
//! can properly connect their river systems.

use crate::erosion::RiverNetwork;
use crate::tilemap::Tilemap;
use super::handshake::RegionHandshake;

/// Edge identifiers for region boundaries
pub const EDGE_N: u8 = 0;
pub const EDGE_E: u8 = 1;
pub const EDGE_S: u8 = 2;
pub const EDGE_W: u8 = 3;

/// A river crossing at a region boundary edge
#[derive(Clone, Debug)]
pub struct RiverEdgeCrossing {
    /// Which edge (N=0, E=1, S=2, W=3)
    pub edge: u8,
    /// Position along edge (0-63 in region coordinates)
    pub position: u8,
    /// Flow volume (determines river width)
    pub flow_volume: f32,
    /// Direction of flow: true = flowing out of region, false = flowing in
    pub is_outflow: bool,
    /// River segment ID from the network
    pub segment_id: usize,
}

/// Find river crossings for a single world tile
pub fn find_river_crossings(
    river_network: &RiverNetwork,
    world_x: usize,
    world_y: usize,
    _world_width: usize,
    _world_height: usize,
) -> Vec<RiverEdgeCrossing> {
    let mut crossings = Vec::new();

    // Tile boundaries in world coordinates
    let tile_x_min = world_x as f32;
    let tile_x_max = (world_x + 1) as f32;
    let tile_y_min = world_y as f32;
    let tile_y_max = (world_y + 1) as f32;

    // Check each river segment for boundary crossings
    for segment in &river_network.segments {
        // Sample along the segment to find crossings
        let samples = 20;
        let mut prev_inside = None;

        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let pt = segment.evaluate(t);

            // Check if point is inside this tile
            let inside = pt.world_x >= tile_x_min
                && pt.world_x < tile_x_max
                && pt.world_y >= tile_y_min
                && pt.world_y < tile_y_max;

            // Detect crossing
            if let Some(was_inside) = prev_inside {
                if inside != was_inside {
                    // Found a crossing - determine which edge
                    let prev_t = (i - 1) as f32 / samples as f32;
                    let prev_pt = segment.evaluate(prev_t);

                    let (edge, position) = determine_crossing_edge(
                        prev_pt.world_x, prev_pt.world_y,
                        pt.world_x, pt.world_y,
                        tile_x_min, tile_y_min,
                        tile_x_max, tile_y_max,
                    );

                    // Flow direction: outflow if we were inside and now outside
                    let is_outflow = was_inside && !inside;

                    crossings.push(RiverEdgeCrossing {
                        edge,
                        position,
                        flow_volume: pt.flow_accumulation,
                        is_outflow,
                        segment_id: segment.id,
                    });
                }
            }

            prev_inside = Some(inside);
        }
    }

    crossings
}

/// Determine which edge a crossing occurs at and the position along that edge
fn determine_crossing_edge(
    x1: f32, y1: f32,
    x2: f32, y2: f32,
    x_min: f32, y_min: f32,
    x_max: f32, y_max: f32,
) -> (u8, u8) {
    // Check each edge for intersection
    const REGION_SIZE: f32 = 64.0;

    // North edge (y = y_min)
    if (y1 < y_min && y2 >= y_min) || (y1 >= y_min && y2 < y_min) {
        let t = (y_min - y1) / (y2 - y1);
        let x_cross = x1 + t * (x2 - x1);
        let pos = ((x_cross - x_min) / (x_max - x_min) * REGION_SIZE) as u8;
        return (EDGE_N, pos.min(63));
    }

    // South edge (y = y_max)
    if (y1 < y_max && y2 >= y_max) || (y1 >= y_max && y2 < y_max) {
        let t = (y_max - y1) / (y2 - y1);
        let x_cross = x1 + t * (x2 - x1);
        let pos = ((x_cross - x_min) / (x_max - x_min) * REGION_SIZE) as u8;
        return (EDGE_S, pos.min(63));
    }

    // West edge (x = x_min)
    if (x1 < x_min && x2 >= x_min) || (x1 >= x_min && x2 < x_min) {
        let t = (x_min - x1) / (x2 - x1);
        let y_cross = y1 + t * (y2 - y1);
        let pos = ((y_cross - y_min) / (y_max - y_min) * REGION_SIZE) as u8;
        return (EDGE_W, pos.min(63));
    }

    // East edge (x = x_max)
    if (x1 < x_max && x2 >= x_max) || (x1 >= x_max && x2 < x_max) {
        let t = (x_max - x1) / (x2 - x1);
        let y_cross = y1 + t * (y2 - y1);
        let pos = ((y_cross - y_min) / (y_max - y_min) * REGION_SIZE) as u8;
        return (EDGE_E, pos.min(63));
    }

    // Fallback: use closest edge
    let mid_x = (x1 + x2) / 2.0;
    let mid_y = (y1 + y2) / 2.0;

    let dist_n = (mid_y - y_min).abs();
    let dist_s = (mid_y - y_max).abs();
    let dist_w = (mid_x - x_min).abs();
    let dist_e = (mid_x - x_max).abs();

    let min_dist = dist_n.min(dist_s).min(dist_w).min(dist_e);

    if min_dist == dist_n {
        let pos = ((mid_x - x_min) / (x_max - x_min) * REGION_SIZE) as u8;
        (EDGE_N, pos.min(63))
    } else if min_dist == dist_s {
        let pos = ((mid_x - x_min) / (x_max - x_min) * REGION_SIZE) as u8;
        (EDGE_S, pos.min(63))
    } else if min_dist == dist_w {
        let pos = ((mid_y - y_min) / (y_max - y_min) * REGION_SIZE) as u8;
        (EDGE_W, pos.min(63))
    } else {
        let pos = ((mid_y - y_min) / (y_max - y_min) * REGION_SIZE) as u8;
        (EDGE_E, pos.min(63))
    }
}

/// Update all region handshakes with river crossing data
pub fn calculate_river_crossings(
    handshakes: &mut Tilemap<RegionHandshake>,
    river_network: &RiverNetwork,
) {
    let width = handshakes.width;
    let height = handshakes.height;

    for y in 0..height {
        for x in 0..width {
            let crossings = find_river_crossings(river_network, x, y, width, height);
            handshakes.get_mut(x, y).river_crossings = crossings;
        }
    }
}
