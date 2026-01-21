//! Castle and fortress generation with organic shapes
//!
//! Creates castles with round towers, curved walls, and multi-level structures.
//! Ensures Z-level consistency so structures look correct from all levels.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::f32::consts::PI;
use crate::zlevel::{ZTile, Tilemap3D};
use super::shapes::{filled_circle, circle_outline, irregular_circle, organic_blob, extract_outline, extract_interior};

/// Number of Z-levels for structure walls (above and below surface)
const WALL_HEIGHT_ABOVE: i32 = 2;
const WALL_HEIGHT_BELOW: i32 = 1;

/// Generate a castle with round towers and organic layout
pub fn generate_complex_castle(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    size: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    let radius = size as i32 / 2;

    // Generate main castle footprint using irregular circle
    let castle_shape = irregular_circle(
        center_x as i32,
        center_y as i32,
        radius,
        0.2, // 20% variation for organic feel
        rng.gen(),
    );

    let walls = extract_outline(&castle_shape);
    let interior = extract_interior(&castle_shape);

    // Place walls on multiple Z-levels for consistency
    for &(wx, wy) in &walls {
        place_wall_column(zlevels, wx, wy, z, map_width, map_height);
    }

    // Place floor
    for &(fx, fy) in &interior {
        place_floor_tile(zlevels, fx, fy, z, ZTile::StoneFloor, map_width, map_height);
    }

    // Add round towers at cardinal points
    let tower_positions = [
        (center_x as i32, center_y as i32 - radius + 2),  // North
        (center_x as i32, center_y as i32 + radius - 2),  // South
        (center_x as i32 - radius + 2, center_y as i32),  // West
        (center_x as i32 + radius - 2, center_y as i32),  // East
    ];

    let tower_radius = (radius / 4).max(3);
    for (tx, ty) in tower_positions {
        generate_round_tower(zlevels, tx, ty, tower_radius, z, rng, map_width, map_height);
    }

    // Generate central keep (larger round tower)
    let keep_radius = (radius / 3).max(4);
    generate_round_tower(
        zlevels,
        center_x as i32,
        center_y as i32,
        keep_radius,
        z,
        rng,
        map_width,
        map_height,
    );

    // Add gate (opening in south wall)
    let gate_y = center_y as i32 + radius - 1;
    for dx in -1..=1 {
        let gx = (center_x as i32 + dx).rem_euclid(map_width as i32) as usize;
        let gy = gate_y.clamp(0, map_height as i32 - 1) as usize;
        // Place door on all relevant Z-levels
        for dz in -WALL_HEIGHT_BELOW..=WALL_HEIGHT_ABOVE {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(gx, gy, z + dz, ZTile::Door);
            }
        }
    }

    // Add interior features
    add_castle_interior_features(zlevels, center_x, center_y, radius as usize, z, rng, map_width, map_height);
}

/// Generate a round tower
fn generate_round_tower(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: i32,
    center_y: i32,
    radius: i32,
    base_z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    let tower_shape = filled_circle(center_x, center_y, radius);
    let walls = extract_outline(&tower_shape);
    let interior = extract_interior(&tower_shape);

    // Towers are taller than regular walls
    let tower_height_above = WALL_HEIGHT_ABOVE + 1;

    // Place tower walls on multiple Z-levels
    for &(wx, wy) in &walls {
        for dz in -WALL_HEIGHT_BELOW..=tower_height_above {
            if zlevels.is_valid_z(base_z + dz) {
                let px = wx.rem_euclid(map_width as i32) as usize;
                let py = wy.clamp(0, map_height as i32 - 1) as usize;
                zlevels.set(px, py, base_z + dz, ZTile::StoneWall);
            }
        }
    }

    // Place tower floor
    for &(fx, fy) in &interior {
        place_floor_tile(zlevels, fx, fy, base_z, ZTile::StoneFloor, map_width, map_height);
    }

    // Add stairs in tower center
    let cx = center_x.rem_euclid(map_width as i32) as usize;
    let cy = center_y.clamp(0, map_height as i32 - 1) as usize;
    zlevels.set(cx, cy, base_z, ZTile::StairsUp);

    // Place stairs on upper levels too
    if zlevels.is_valid_z(base_z + 1) {
        zlevels.set(cx, cy, base_z + 1, ZTile::StairsDown);
        // Floor around stairs on upper level
        for &(fx, fy) in &interior {
            if fx != center_x || fy != center_y {
                let px = fx.rem_euclid(map_width as i32) as usize;
                let py = fy.clamp(0, map_height as i32 - 1) as usize;
                zlevels.set(px, py, base_z + 1, ZTile::StoneFloor);
            }
        }
    }
}

/// Place a wall column spanning multiple Z-levels
fn place_wall_column(
    zlevels: &mut Tilemap3D<ZTile>,
    x: i32,
    y: i32,
    base_z: i32,
    map_width: usize,
    map_height: usize,
) {
    let px = x.rem_euclid(map_width as i32) as usize;
    let py = y.clamp(0, map_height as i32 - 1) as usize;

    for dz in -WALL_HEIGHT_BELOW..=WALL_HEIGHT_ABOVE {
        if zlevels.is_valid_z(base_z + dz) {
            zlevels.set(px, py, base_z + dz, ZTile::StoneWall);
        }
    }
}

/// Place a floor tile with proper ceiling above
fn place_floor_tile(
    zlevels: &mut Tilemap3D<ZTile>,
    x: i32,
    y: i32,
    base_z: i32,
    floor_tile: ZTile,
    map_width: usize,
    map_height: usize,
) {
    let px = x.rem_euclid(map_width as i32) as usize;
    let py = y.clamp(0, map_height as i32 - 1) as usize;

    // Place floor at base level
    zlevels.set(px, py, base_z, floor_tile);

    // Place solid (ceiling) above the floor if there's a level above
    if zlevels.is_valid_z(base_z + 1) {
        let above = *zlevels.get(px, py, base_z + 1);
        // Only place ceiling if not already a structure tile
        if !above.is_structure() && above != ZTile::Air {
            // Keep it as solid for ceiling effect
        }
    }

    // Foundation below
    for dz in 1..=WALL_HEIGHT_BELOW {
        if zlevels.is_valid_z(base_z - dz) {
            let below = *zlevels.get(px, py, base_z - dz);
            if below == ZTile::Solid || below == ZTile::Surface {
                // Keep as solid - this is the foundation
            }
        }
    }
}

/// Add interior features to a castle
fn add_castle_interior_features(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    radius: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    // Add columns in a ring pattern
    let column_radius = radius / 2;
    let num_columns = 6;
    for i in 0..num_columns {
        let angle = (i as f32 / num_columns as f32) * 2.0 * PI;
        let cx = center_x as f32 + angle.cos() * column_radius as f32;
        let cy = center_y as f32 + angle.sin() * column_radius as f32;

        let px = (cx as i32).rem_euclid(map_width as i32) as usize;
        let py = (cy as i32).clamp(0, map_height as i32 - 1) as usize;

        // Columns span multiple Z-levels
        for dz in 0..=WALL_HEIGHT_ABOVE {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(px, py, z + dz, ZTile::Column);
            }
        }
    }

    // Add chests
    let num_chests = rng.gen_range(2..=4);
    for _ in 0..num_chests {
        let angle = rng.gen_range(0.0..2.0 * PI);
        let dist = rng.gen_range(radius as f32 * 0.2..radius as f32 * 0.6);
        let cx = center_x as f32 + angle.cos() * dist;
        let cy = center_y as f32 + angle.sin() * dist;

        let px = (cx as i32).rem_euclid(map_width as i32) as usize;
        let py = (cy as i32).clamp(0, map_height as i32 - 1) as usize;

        let current = *zlevels.get(px, py, z);
        if current == ZTile::StoneFloor {
            zlevels.set(px, py, z, ZTile::Chest);
        }
    }

    // Add altar in keep center
    zlevels.set(center_x, center_y, z, ZTile::Altar);

    // Add stairs down to dungeon
    let dungeon_x = center_x.saturating_sub(2);
    let dungeon_y = center_y.saturating_sub(2);
    if dungeon_x < map_width && dungeon_y < map_height {
        let current = *zlevels.get(dungeon_x, dungeon_y, z);
        if current == ZTile::StoneFloor {
            zlevels.set(dungeon_x, dungeon_y, z, ZTile::StairsDown);

            // Create corresponding stairs up on level below
            if zlevels.is_valid_z(z - 1) {
                zlevels.set(dungeon_x, dungeon_y, z - 1, ZTile::StairsUp);
            }
        }
    }
}

/// Generate castle walls using curved segments
pub fn generate_castle_walls(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    radius: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) -> Vec<(usize, usize)> {
    let mut wall_points = Vec::new();

    // Use irregular circle for organic walls
    let wall_shape = irregular_circle(
        center_x as i32,
        center_y as i32,
        radius as i32,
        0.15,
        rng.gen(),
    );

    let walls = extract_outline(&wall_shape);

    for (wx, wy) in walls {
        let px = wx.rem_euclid(map_width as i32) as usize;
        let py = wy.clamp(0, map_height as i32 - 1) as usize;

        // Place wall on multiple Z-levels
        for dz in -WALL_HEIGHT_BELOW..=WALL_HEIGHT_ABOVE {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(px, py, z + dz, ZTile::StoneWall);
            }
        }

        wall_points.push((px, py));
    }

    // Fill interior with floor
    let interior = extract_interior(&wall_shape);
    for (fx, fy) in interior {
        place_floor_tile(zlevels, fx, fy, z, ZTile::StoneFloor, map_width, map_height);
    }

    wall_points
}

/// Generate a village with organic layout
pub fn generate_organic_village(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    size: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    // Place 4-7 buildings in organic cluster
    let num_buildings = rng.gen_range(4..=7);

    for i in 0..num_buildings {
        // Scatter buildings around center
        let angle = rng.gen_range(0.0..2.0 * PI);
        let dist = rng.gen_range(size as f32 * 0.1..size as f32 * 0.4);

        let bx = center_x as f32 + angle.cos() * dist;
        let by = center_y as f32 + angle.sin() * dist;

        // Choose building shape
        let building_radius = rng.gen_range(2..=4);

        if rng.gen_bool(0.4) {
            // Round building
            generate_round_building(
                zlevels,
                bx as i32,
                by as i32,
                building_radius,
                z,
                rng,
                map_width,
                map_height,
            );
        } else {
            // Rounded rectangle building
            generate_rounded_building(
                zlevels,
                bx as i32,
                by as i32,
                building_radius * 2,
                building_radius + rng.gen_range(1..3),
                z,
                rng,
                map_width,
                map_height,
            );
        }
    }

    // Add a central well/gathering area
    let well_shape = filled_circle(center_x as i32, center_y as i32, 2);
    for (wx, wy) in well_shape {
        let px = wx.rem_euclid(map_width as i32) as usize;
        let py = wy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(px, py, z, ZTile::CobblestoneFloor);
    }

    // Well center
    zlevels.set(center_x, center_y, z, ZTile::StoneWall);
}

/// Generate a round building (hut-like)
fn generate_round_building(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: i32,
    center_y: i32,
    radius: i32,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    let shape = filled_circle(center_x, center_y, radius);
    let walls = extract_outline(&shape);
    let interior = extract_interior(&shape);

    // Place walls
    for &(wx, wy) in &walls {
        let px = wx.rem_euclid(map_width as i32) as usize;
        let py = wy.clamp(0, map_height as i32 - 1) as usize;

        for dz in 0..=1 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(px, py, z + dz, ZTile::WoodWall);
            }
        }
    }

    // Place floor
    for (fx, fy) in interior {
        let px = fx.rem_euclid(map_width as i32) as usize;
        let py = fy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(px, py, z, ZTile::WoodFloor);
    }

    // Add door on random side
    if !walls.is_empty() {
        let (dx, dy) = walls[rng.gen_range(0..walls.len())];
        let px = dx.rem_euclid(map_width as i32) as usize;
        let py = dy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(px, py, z, ZTile::Door);
        if zlevels.is_valid_z(z + 1) {
            zlevels.set(px, py, z + 1, ZTile::Door);
        }
    }
}

/// Generate a rounded rectangle building
fn generate_rounded_building(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: i32,
    center_y: i32,
    width: i32,
    height: i32,
    z: i32,
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    let corner_radius = (width.min(height) / 3).max(1);
    let shape = super::shapes::rounded_rectangle(
        center_x - width / 2,
        center_y - height / 2,
        width,
        height,
        corner_radius,
    );

    let walls = extract_outline(&shape);
    let interior = extract_interior(&shape);

    // Place walls
    for &(wx, wy) in &walls {
        let px = wx.rem_euclid(map_width as i32) as usize;
        let py = wy.clamp(0, map_height as i32 - 1) as usize;

        for dz in 0..=1 {
            if zlevels.is_valid_z(z + dz) {
                let wall_type = if rng.gen_bool(0.7) {
                    ZTile::WoodWall
                } else {
                    ZTile::StoneWall
                };
                zlevels.set(px, py, z + dz, wall_type);
            }
        }
    }

    // Place floor
    for (fx, fy) in interior {
        let px = fx.rem_euclid(map_width as i32) as usize;
        let py = fy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(px, py, z, ZTile::WoodFloor);
    }

    // Add door
    if let Some(&(dx, dy)) = walls.get(rng.gen_range(0..walls.len().max(1))) {
        let px = dx.rem_euclid(map_width as i32) as usize;
        let py = dy.clamp(0, map_height as i32 - 1) as usize;
        zlevels.set(px, py, z, ZTile::Door);
        if zlevels.is_valid_z(z + 1) {
            zlevels.set(px, py, z + 1, ZTile::Door);
        }
    }
}
