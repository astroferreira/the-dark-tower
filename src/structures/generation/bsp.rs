//! Binary Space Partitioning (BSP) for room generation
//!
//! Generates building layouts, dungeon rooms, and city blocks using
//! recursive binary space partitioning with varied room shapes.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use crate::zlevel::{ZTile, Tilemap3D};
use crate::structures::types::{BspNode, Room, RoomShape};
use super::shapes::{
    filled_circle, circle_outline, circle_interior,
    rounded_rectangle, extract_outline, extract_interior,
    l_shape, irregular_circle, organic_blob,
};

/// Minimum room size for BSP generation
const MIN_ROOM_SIZE: usize = 4;

/// Ratio to split (how uneven splits can be)
const SPLIT_RATIO_MIN: f32 = 0.35;
const SPLIT_RATIO_MAX: f32 = 0.65;

/// Generate a BSP tree for a given area
pub fn generate_bsp_tree(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    min_size: usize,
    rng: &mut ChaCha8Rng,
) -> BspNode {
    let mut root = BspNode::new(x, y, width, height);
    split_node(&mut root, min_size, rng);
    root
}

/// Recursively split a BSP node
fn split_node(node: &mut BspNode, min_size: usize, rng: &mut ChaCha8Rng) {
    // Don't split if already too small
    if node.width < min_size * 2 && node.height < min_size * 2 {
        return;
    }

    // Decide split direction (prefer splitting the longer dimension)
    let width_f = node.width as f32;
    let height_f = node.height as f32;
    let split_h = if width_f >= height_f * 1.25 {
        false // Split vertically (make width smaller)
    } else if height_f >= width_f * 1.25 {
        true // Split horizontally (make height smaller)
    } else {
        rng.gen_bool(0.5) // Random
    };

    // Check if we can split in the chosen direction
    if split_h && node.height < min_size * 2 {
        return;
    }
    if !split_h && node.width < min_size * 2 {
        return;
    }

    // Calculate split position
    let ratio = rng.gen_range(SPLIT_RATIO_MIN..SPLIT_RATIO_MAX);

    if split_h {
        // Horizontal split
        let split_y = node.y + (node.height as f32 * ratio) as usize;
        let split_y = split_y.max(node.y + min_size).min(node.y + node.height - min_size);

        let mut left = BspNode::new(node.x, node.y, node.width, split_y - node.y);
        let mut right = BspNode::new(node.x, split_y, node.width, node.y + node.height - split_y);

        split_node(&mut left, min_size, rng);
        split_node(&mut right, min_size, rng);

        node.left = Some(Box::new(left));
        node.right = Some(Box::new(right));
    } else {
        // Vertical split
        let split_x = node.x + (node.width as f32 * ratio) as usize;
        let split_x = split_x.max(node.x + min_size).min(node.x + node.width - min_size);

        let mut left = BspNode::new(node.x, node.y, split_x - node.x, node.height);
        let mut right = BspNode::new(split_x, node.y, node.x + node.width - split_x, node.height);

        split_node(&mut left, min_size, rng);
        split_node(&mut right, min_size, rng);

        node.left = Some(Box::new(left));
        node.right = Some(Box::new(right));
    }
}

/// Create rooms in BSP leaf nodes with varied shapes
pub fn create_rooms_in_bsp(node: &mut BspNode, rng: &mut ChaCha8Rng) {
    if node.is_leaf() {
        // Create a room that fits within this leaf with some padding
        let padding = 1;
        let room_width = rng.gen_range(
            (node.width / 2).max(MIN_ROOM_SIZE)..=(node.width - padding * 2).max(MIN_ROOM_SIZE)
        );
        let room_height = rng.gen_range(
            (node.height / 2).max(MIN_ROOM_SIZE)..=(node.height - padding * 2).max(MIN_ROOM_SIZE)
        );

        let room_x = node.x + rng.gen_range(padding..=(node.width - room_width - padding).max(padding));
        let room_y = node.y + rng.gen_range(padding..=(node.height - room_height - padding).max(padding));

        // Choose room shape based on size and randomness
        let shape = choose_room_shape(room_width, room_height, rng);
        let corner = rng.gen_range(0..4); // For L-shaped rooms

        let mut room = Room::with_shape(room_x, room_y, room_width, room_height, shape);
        room.corner = corner;
        node.room = Some(room);
    } else {
        if let Some(ref mut left) = node.left {
            create_rooms_in_bsp(left, rng);
        }
        if let Some(ref mut right) = node.right {
            create_rooms_in_bsp(right, rng);
        }
    }
}

/// Choose a room shape based on dimensions and randomness
fn choose_room_shape(width: usize, height: usize, rng: &mut ChaCha8Rng) -> RoomShape {
    let min_dim = width.min(height);
    let aspect_ratio = width as f32 / height as f32;

    // Circular rooms work best when roughly square
    let can_be_circular = aspect_ratio > 0.7 && aspect_ratio < 1.4 && min_dim >= 5;

    // L-shapes need enough space
    let can_be_l_shape = min_dim >= 6;

    // Organic shapes need moderate size
    let can_be_organic = min_dim >= 5;

    let roll = rng.gen_range(0..100);

    match roll {
        0..=25 => RoomShape::Rectangular,  // 26% rectangular
        26..=45 => RoomShape::Rounded,     // 20% rounded corners
        46..=65 if can_be_circular => RoomShape::Circular,  // 20% circular (if suitable)
        66..=80 if can_be_l_shape => RoomShape::LShape,     // 15% L-shaped (if suitable)
        81..=100 if can_be_organic => RoomShape::Organic,   // 19% organic (if suitable)
        _ => RoomShape::Rounded,  // Fallback to rounded
    }
}

/// Get all rooms from a BSP tree
pub fn collect_rooms(node: &BspNode) -> Vec<Room> {
    let mut rooms = Vec::new();
    collect_rooms_recursive(node, &mut rooms);
    rooms
}

fn collect_rooms_recursive(node: &BspNode, rooms: &mut Vec<Room>) {
    if let Some(ref room) = node.room {
        rooms.push(room.clone());
    }

    if let Some(ref left) = node.left {
        collect_rooms_recursive(left, rooms);
    }
    if let Some(ref right) = node.right {
        collect_rooms_recursive(right, rooms);
    }
}

/// Connect rooms in a BSP tree with corridors
pub fn connect_rooms_bsp(node: &BspNode, corridors: &mut Vec<((usize, usize), (usize, usize))>) {
    if let (Some(ref left), Some(ref right)) = (&node.left, &node.right) {
        // Find rooms to connect from left and right subtrees
        let left_room = find_closest_room(left);
        let right_room = find_closest_room(right);

        if let (Some(lr), Some(rr)) = (left_room, right_room) {
            corridors.push((lr.center(), rr.center()));
        }

        // Recursively connect within subtrees
        connect_rooms_bsp(left, corridors);
        connect_rooms_bsp(right, corridors);
    }
}

/// Find the closest room in a BSP subtree (to its center)
fn find_closest_room(node: &BspNode) -> Option<Room> {
    if let Some(ref room) = node.room {
        return Some(room.clone());
    }

    // Return room from left or right child
    if let Some(ref left) = node.left {
        if let Some(room) = find_closest_room(left) {
            return Some(room);
        }
    }
    if let Some(ref right) = node.right {
        if let Some(room) = find_closest_room(right) {
            return Some(room);
        }
    }

    None
}

/// Render a BSP building to the tilemap
pub fn render_bsp_building(
    zlevels: &mut Tilemap3D<ZTile>,
    rooms: &[Room],
    corridors: &[((usize, usize), (usize, usize))],
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    // Render rooms
    for room in rooms {
        render_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);
    }

    // Render corridors
    for (start, end) in corridors {
        render_corridor(zlevels, *start, *end, z, floor_tile, map_width, map_height);
    }
}

/// Render a single room based on its shape
fn render_room(
    zlevels: &mut Tilemap3D<ZTile>,
    room: &Room,
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    match room.shape {
        RoomShape::Rectangular => {
            render_rectangular_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);
        }
        RoomShape::Rounded => {
            render_rounded_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);
        }
        RoomShape::Circular => {
            render_circular_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);
        }
        RoomShape::LShape => {
            render_l_shaped_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);
        }
        RoomShape::Organic => {
            render_organic_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);
        }
    }
}

/// Render a traditional rectangular room
fn render_rectangular_room(
    zlevels: &mut Tilemap3D<ZTile>,
    room: &Room,
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    for dy in 0..room.height {
        for dx in 0..room.width {
            let x = (room.x + dx) % map_width;
            let y = (room.y + dy).min(map_height - 1);

            let is_wall = dx == 0 || dx == room.width - 1 || dy == 0 || dy == room.height - 1;
            let tile = if is_wall { wall_tile } else { floor_tile };

            // Place wall on multiple Z-levels for consistency
            if is_wall {
                for dz in 0..=2 {
                    if zlevels.is_valid_z(z + dz) {
                        zlevels.set(x, y, z + dz, tile);
                    }
                }
            } else {
                zlevels.set(x, y, z, tile);
            }
        }
    }
}

/// Render a room with rounded corners
fn render_rounded_room(
    zlevels: &mut Tilemap3D<ZTile>,
    room: &Room,
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    let corner_radius = (room.width.min(room.height) / 4).max(1) as i32;
    let shape = rounded_rectangle(
        room.x as i32,
        room.y as i32,
        room.width as i32,
        room.height as i32,
        corner_radius,
    );

    let walls = extract_outline(&shape);
    let interior = extract_interior(&shape);

    // Render walls on multiple Z-levels
    for (wx, wy) in walls {
        let x = wx.rem_euclid(map_width as i32) as usize;
        let y = wy.clamp(0, map_height as i32 - 1) as usize;

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(x, y, z + dz, wall_tile);
            }
        }
    }

    // Render floor
    for (fx, fy) in interior {
        let x = fx.rem_euclid(map_width as i32) as usize;
        let y = fy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(x, y, z, floor_tile);
    }
}

/// Render a circular room
fn render_circular_room(
    zlevels: &mut Tilemap3D<ZTile>,
    room: &Room,
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    let center_x = (room.x + room.width / 2) as i32;
    let center_y = (room.y + room.height / 2) as i32;
    let radius = (room.width.min(room.height) / 2) as i32;

    // Get walls and floor
    let walls = circle_outline(center_x, center_y, radius, 1);
    let interior = circle_interior(center_x, center_y, radius, 1);

    // Render walls on multiple Z-levels
    for (wx, wy) in walls {
        let x = wx.rem_euclid(map_width as i32) as usize;
        let y = wy.clamp(0, map_height as i32 - 1) as usize;

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(x, y, z + dz, wall_tile);
            }
        }
    }

    // Render floor
    for (fx, fy) in interior {
        let x = fx.rem_euclid(map_width as i32) as usize;
        let y = fy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(x, y, z, floor_tile);
    }
}

/// Render an L-shaped room
fn render_l_shaped_room(
    zlevels: &mut Tilemap3D<ZTile>,
    room: &Room,
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    // L-shape with specified corner
    let w1 = room.width as i32;
    let h1 = (room.height / 2).max(2) as i32;
    let w2 = (room.width / 2).max(2) as i32;
    let h2 = room.height as i32;

    let shape = l_shape(
        room.x as i32,
        room.y as i32,
        w1, h1, w2, h2,
        room.corner,
    );

    let walls = extract_outline(&shape);
    let interior = extract_interior(&shape);

    // Render walls on multiple Z-levels
    for (wx, wy) in walls {
        let x = wx.rem_euclid(map_width as i32) as usize;
        let y = wy.clamp(0, map_height as i32 - 1) as usize;

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(x, y, z + dz, wall_tile);
            }
        }
    }

    // Render floor
    for (fx, fy) in interior {
        let x = fx.rem_euclid(map_width as i32) as usize;
        let y = fy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(x, y, z, floor_tile);
    }
}

/// Render an organic blob-shaped room
fn render_organic_room(
    zlevels: &mut Tilemap3D<ZTile>,
    room: &Room,
    z: i32,
    wall_tile: ZTile,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    let center_x = (room.x + room.width / 2) as i32;
    let center_y = (room.y + room.height / 2) as i32;
    let radius = (room.width.min(room.height) / 2) as i32;

    // Use irregular circle for organic look
    let seed = ((center_x as u32).wrapping_mul(31337)).wrapping_add(center_y as u32);
    let shape = irregular_circle(center_x, center_y, radius, 0.3, seed);

    let walls = extract_outline(&shape);
    let interior = extract_interior(&shape);

    // Render walls on multiple Z-levels
    for (wx, wy) in walls {
        let x = wx.rem_euclid(map_width as i32) as usize;
        let y = wy.clamp(0, map_height as i32 - 1) as usize;

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(x, y, z + dz, wall_tile);
            }
        }
    }

    // Render floor
    for (fx, fy) in interior {
        let x = fx.rem_euclid(map_width as i32) as usize;
        let y = fy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(x, y, z, floor_tile);
    }
}

/// Render a corridor between two points using L-shaped path
fn render_corridor(
    zlevels: &mut Tilemap3D<ZTile>,
    start: (usize, usize),
    end: (usize, usize),
    z: i32,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    let (mut x, mut y) = start;
    let (ex, ey) = end;

    // Horizontal then vertical
    while x != ex {
        let wrapped_x = x % map_width;
        let wrapped_y = y.min(map_height - 1);

        // Only set floor if not already a structure tile
        let current = *zlevels.get(wrapped_x, wrapped_y, z);
        if !current.is_structure() || current == ZTile::Solid || current == ZTile::Surface {
            zlevels.set(wrapped_x, wrapped_y, z, floor_tile);
        }

        if x < ex { x += 1; } else { x -= 1; }
    }

    while y != ey {
        let wrapped_x = x % map_width;
        let wrapped_y = y.min(map_height - 1);

        let current = *zlevels.get(wrapped_x, wrapped_y, z);
        if !current.is_structure() || current == ZTile::Solid || current == ZTile::Surface {
            zlevels.set(wrapped_x, wrapped_y, z, floor_tile);
        }

        if y < ey { y += 1; } else if y > 0 { y -= 1; } else { break; }
    }
}

/// Generate a dungeon using BSP
pub fn generate_dungeon_level(
    zlevels: &mut Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) -> Vec<Room> {
    let mut root = generate_bsp_tree(x, y, width, height, MIN_ROOM_SIZE + 2, rng);
    create_rooms_in_bsp(&mut root, rng);

    let rooms = collect_rooms(&root);
    let mut corridors = Vec::new();
    connect_rooms_bsp(&root, &mut corridors);

    render_bsp_building(
        zlevels,
        &rooms,
        &corridors,
        z,
        ZTile::StoneWall,
        ZTile::StoneFloor,
        map_width,
        map_height,
    );

    rooms
}

/// Generate a city block using BSP (for city ruins)
pub fn generate_city_block(
    zlevels: &mut Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) -> Vec<Room> {
    let mut root = generate_bsp_tree(x, y, width, height, 6, rng);
    create_rooms_in_bsp(&mut root, rng);

    let rooms = collect_rooms(&root);

    // Render buildings (no corridors between city buildings)
    for room in &rooms {
        // Choose wall type based on random
        let wall_tile = if rng.gen_bool(0.6) {
            ZTile::BrickWall
        } else {
            ZTile::StoneWall
        };

        let floor_tile = if rng.gen_bool(0.7) {
            ZTile::WoodFloor
        } else {
            ZTile::StoneFloor
        };

        render_room(zlevels, room, z, wall_tile, floor_tile, map_width, map_height);

        // Add a door on a random side
        let door_side = rng.gen_range(0..4);
        let (door_x, door_y) = match door_side {
            0 => (room.x + room.width / 2, room.y), // Top
            1 => (room.x + room.width / 2, room.y + room.height - 1), // Bottom
            2 => (room.x, room.y + room.height / 2), // Left
            _ => (room.x + room.width - 1, room.y + room.height / 2), // Right
        };

        let door_x = door_x % map_width;
        let door_y = door_y.min(map_height - 1);
        zlevels.set(door_x, door_y, z, ZTile::Door);
    }

    // Add cobblestone streets around buildings
    for dy in 0..height {
        for dx in 0..width {
            let px = (x + dx) % map_width;
            let py = (y + dy).min(map_height - 1);

            let current = *zlevels.get(px, py, z);
            if current == ZTile::Surface || current == ZTile::Solid {
                // Check if adjacent to a building
                let mut adjacent_to_building = false;
                for (nx, ny) in [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)] {
                    let check_x = (px as i32 + nx).rem_euclid(map_width as i32) as usize;
                    let check_y = (py as i32 + ny).clamp(0, map_height as i32 - 1) as usize;
                    let neighbor = *zlevels.get(check_x, check_y, z);
                    if matches!(neighbor, ZTile::StoneWall | ZTile::BrickWall | ZTile::WoodWall | ZTile::Door) {
                        adjacent_to_building = true;
                        break;
                    }
                }

                if adjacent_to_building {
                    zlevels.set(px, py, z, ZTile::CobblestoneFloor);
                }
            }
        }
    }

    rooms
}
