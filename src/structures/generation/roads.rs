//! Road network generation using Dijkstra's algorithm
//!
//! Creates roads connecting major structures using pathfinding with
//! terrain-aware cost functions.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use crate::tilemap::Tilemap;
use crate::water_bodies::WaterBodyId;
use crate::zlevel::{ZTile, Tilemap3D};
use crate::structures::types::{PlacedStructure, RoadSegment, RoadType};

/// Node for Dijkstra's priority queue
#[derive(Clone, Copy)]
struct PathNode {
    x: usize,
    y: usize,
    cost: f32,
}

impl PartialEq for PathNode {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for PathNode {}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

/// Generate the road network connecting structures
pub fn generate_road_network(
    structures: &[PlacedStructure],
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    rng: &mut ChaCha8Rng,
) -> Vec<RoadSegment> {
    let mut road_segments = Vec::new();

    if structures.len() < 2 {
        return road_segments;
    }

    // Build minimum spanning tree of structures using Prim's algorithm
    let mst_edges = compute_structure_mst(structures);

    // Generate roads for each MST edge
    for (i, j) in mst_edges {
        let (ax, ay) = structures[i].center();
        let (bx, by) = structures[j].center();

        // Determine road type based on structure types
        let road_type = determine_road_type(&structures[i], &structures[j]);

        // Find path using Dijkstra
        if let Some(path) = find_road_path(
            ax, ay, bx, by,
            heightmap,
            water_bodies,
            zlevels,
            surface_z,
        ) {
            // Render the road
            render_road(
                zlevels,
                surface_z,
                &path,
                road_type,
            );

            road_segments.push(RoadSegment {
                start: (ax, ay),
                end: (bx, by),
                road_type,
                path,
            });
        }
    }

    // Add some extra connections (shortcuts) for major structures
    let extra_count = (structures.len() / 4).max(1).min(3);
    for _ in 0..extra_count {
        if structures.len() < 2 {
            break;
        }

        let i = rng.gen_range(0..structures.len());
        let mut j = rng.gen_range(0..structures.len());
        while j == i {
            j = rng.gen_range(0..structures.len());
        }

        let (ax, ay) = structures[i].center();
        let (bx, by) = structures[j].center();

        // Only add if structures are somewhat close
        let dist = ((ax as f32 - bx as f32).powi(2) + (ay as f32 - by as f32).powi(2)).sqrt();
        if dist > 150.0 {
            continue;
        }

        let road_type = RoadType::Secondary;

        if let Some(path) = find_road_path(
            ax, ay, bx, by,
            heightmap,
            water_bodies,
            zlevels,
            surface_z,
        ) {
            render_road(zlevels, surface_z, &path, road_type);

            road_segments.push(RoadSegment {
                start: (ax, ay),
                end: (bx, by),
                road_type,
                path,
            });
        }
    }

    road_segments
}

/// Compute minimum spanning tree of structures
fn compute_structure_mst(structures: &[PlacedStructure]) -> Vec<(usize, usize)> {
    if structures.is_empty() {
        return Vec::new();
    }

    let n = structures.len();
    let mut in_mst = vec![false; n];
    let mut mst_edges = Vec::new();

    // Priority queue: (negative distance, from_idx, to_idx)
    let mut pq: BinaryHeap<(i64, usize, usize)> = BinaryHeap::new();

    // Start from first structure
    in_mst[0] = true;

    // Add all edges from node 0
    for j in 1..n {
        let dist = structures[0].distance_to(&structures[j]);
        pq.push((-(dist as i64), 0, j));
    }

    while mst_edges.len() < n - 1 && !pq.is_empty() {
        let (_, from, to) = pq.pop().unwrap();

        if in_mst[to] {
            continue;
        }

        in_mst[to] = true;
        mst_edges.push((from, to));

        // Add edges from new node
        for j in 0..n {
            if !in_mst[j] {
                let dist = structures[to].distance_to(&structures[j]);
                pq.push((-(dist as i64), to, j));
            }
        }
    }

    mst_edges
}

/// Determine the type of road based on connected structures
fn determine_road_type(a: &PlacedStructure, b: &PlacedStructure) -> RoadType {
    use crate::structures::types::StructureType::*;

    match (&a.structure_type, &b.structure_type) {
        (Castle, Castle) | (Castle, City) | (City, City) | (City, Castle) => RoadType::Main,
        (Castle, Village) | (City, Village) | (Village, Castle) | (Village, City) => RoadType::Secondary,
        _ => RoadType::Path,
    }
}

/// Find a road path using Dijkstra's algorithm
fn find_road_path(
    start_x: usize,
    start_y: usize,
    end_x: usize,
    end_y: usize,
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    zlevels: &Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
) -> Option<Vec<(usize, usize)>> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Cost map: infinity initially
    let mut cost_map: HashMap<(usize, usize), f32> = HashMap::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

    let mut pq = BinaryHeap::new();

    cost_map.insert((start_x, start_y), 0.0);
    pq.push(PathNode { x: start_x, y: start_y, cost: 0.0 });

    while let Some(PathNode { x, y, cost }) = pq.pop() {
        // Check if we reached the destination
        if x == end_x && y == end_y {
            // Reconstruct path
            let mut path = Vec::new();
            let mut current = (x, y);
            path.push(current);

            while let Some(&prev) = came_from.get(&current) {
                path.push(prev);
                current = prev;
            }

            path.reverse();
            return Some(path);
        }

        // Skip if we've found a better path to this node
        if let Some(&best_cost) = cost_map.get(&(x, y)) {
            if cost > best_cost {
                continue;
            }
        }

        // Explore neighbors
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            // Compute movement cost
            let move_cost = compute_road_cost(
                x, y, nx, ny,
                heightmap,
                water_bodies,
                zlevels,
                surface_z,
            );

            if move_cost == f32::INFINITY {
                continue;
            }

            // Diagonal movement costs more
            let move_cost = if dx != 0 && dy != 0 {
                move_cost * 1.414
            } else {
                move_cost
            };

            let new_cost = cost + move_cost;

            // Only update if this is a better path
            let current_cost = cost_map.get(&(nx, ny)).copied().unwrap_or(f32::INFINITY);
            if new_cost < current_cost {
                cost_map.insert((nx, ny), new_cost);
                came_from.insert((nx, ny), (x, y));
                pq.push(PathNode { x: nx, y: ny, cost: new_cost });
            }
        }
    }

    None // No path found
}

/// Compute the cost of moving from one tile to another for road building
fn compute_road_cost(
    from_x: usize,
    from_y: usize,
    to_x: usize,
    to_y: usize,
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    zlevels: &Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
) -> f32 {
    let to_elev = *heightmap.get(to_x, to_y);
    let from_elev = *heightmap.get(from_x, from_y);
    let to_water = *water_bodies.get(to_x, to_y);
    let to_z = *surface_z.get(to_x, to_y);
    let to_tile = *zlevels.get(to_x, to_y, to_z);

    // Base movement cost
    let mut cost = 1.0;

    // Water crossing is expensive (needs bridge)
    if !to_water.is_none() {
        cost += 10.0;
    }

    // Underwater is very expensive
    if to_elev < 0.0 {
        cost += 50.0;
    }

    // Slope cost (height difference)
    let slope = (to_elev - from_elev).abs();
    cost += slope / 50.0; // 50m elevation = +1 cost

    // Prefer existing roads
    if to_tile.is_road() {
        cost *= 0.3; // Much cheaper to follow existing roads
    }

    // Avoid building over structures (but not impossible)
    if to_tile.is_structure() && !to_tile.is_road() && !to_tile.is_floor() {
        cost += 20.0;
    }

    // Very high altitude is harder
    if to_elev > 2000.0 {
        cost += (to_elev - 2000.0) / 500.0;
    }

    cost
}

/// Render a road onto the map
fn render_road(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    path: &[(usize, usize)],
    road_type: RoadType,
) {
    let road_tile = road_type.to_tile();
    let road_width = road_type.width();

    for &(x, y) in path {
        let z = *surface_z.get(x, y);
        let current = *zlevels.get(x, y, z);

        // Determine what tile to place
        let tile = if current == ZTile::Water {
            ZTile::Bridge
        } else if current.is_wall() || current.is_structure() {
            // Don't overwrite walls unless it's a door
            if current == ZTile::Door {
                continue;
            }
            road_tile
        } else {
            road_tile
        };

        // Place road tile
        zlevels.set(x, y, z, tile);

        // For wider roads, also place adjacent tiles
        if road_width > 1 {
            let width = zlevels.width;
            let height = zlevels.height;

            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                let nz = *surface_z.get(nx, ny);
                let neighbor = *zlevels.get(nx, ny, nz);

                // Only widen onto passable non-structure tiles
                if neighbor == ZTile::Surface || neighbor == ZTile::Solid {
                    zlevels.set(nx, ny, nz, road_tile);
                }
            }
        }
    }
}

/// Generate roads along village paths
pub fn generate_village_paths(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    center_x: usize,
    center_y: usize,
    radius: usize,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    // Create paths from center to edges
    let num_paths = rng.gen_range(3..=6);
    let angle_step = std::f32::consts::PI * 2.0 / num_paths as f32;

    for i in 0..num_paths {
        let angle = angle_step * i as f32 + rng.gen_range(-0.3..0.3);
        let end_x = center_x as f32 + angle.cos() * radius as f32;
        let end_y = center_y as f32 + angle.sin() * radius as f32;

        let end_x = (end_x as i32).rem_euclid(width as i32) as usize;
        let end_y = (end_y as i32).clamp(0, height as i32 - 1) as usize;

        // Simple straight line path
        let path = bresenham_line(center_x, center_y, end_x, end_y, width, height);

        // Render as dirt path
        for (px, py) in path {
            let z = *surface_z.get(px, py);
            let current = *zlevels.get(px, py, z);

            if current == ZTile::Surface || current == ZTile::Solid {
                zlevels.set(px, py, z, ZTile::DirtRoad);
            }
        }
    }
}

/// Bresenham's line algorithm
fn bresenham_line(
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    width: usize,
    height: usize,
) -> Vec<(usize, usize)> {
    let mut path = Vec::new();

    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = -(y1 as i32 - y0 as i32).abs();
    let sx = if x0 < x1 { 1i32 } else { -1 };
    let sy = if y0 < y1 { 1i32 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0 as i32;
    let mut y = y0 as i32;

    loop {
        let px = x.rem_euclid(width as i32) as usize;
        let py = y.clamp(0, height as i32 - 1) as usize;
        path.push((px, py));

        if x == x1 as i32 && y == y1 as i32 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 as i32 { break; }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 as i32 { break; }
            err += dx;
            y += sy;
        }
    }

    path
}
