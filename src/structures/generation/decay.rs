//! Cellular automata decay system for creating ruins
//!
//! Applies decay effects to structures, transforming them into ruins
//! using cellular automata rules.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use crate::zlevel::{ZTile, Tilemap3D};
use crate::tilemap::Tilemap;

/// Apply decay to a structure region using cellular automata
///
/// The decay process:
/// 1. Walls with few neighbors have high chance to become rubble or air
/// 2. Floors near destroyed walls accumulate rubble
/// 3. Multiple iterations create natural-looking ruins
pub fn apply_decay(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    decay_percentage: f32,
    iterations: usize,
    rng: &mut ChaCha8Rng,
) {
    let map_width = zlevels.width;
    let map_height = zlevels.height;

    // Run cellular automata iterations
    for _ in 0..iterations {
        // Collect changes to apply
        let mut changes: Vec<(usize, usize, ZTile)> = Vec::new();

        for dy in 0..height {
            for dx in 0..width {
                let px = (x + dx) % map_width;
                let py = (y + dy).min(map_height - 1);

                let tile = *zlevels.get(px, py, z);

                // Only decay structure tiles
                if !tile.is_structure() {
                    continue;
                }

                // Count neighboring walls
                let wall_count = count_wall_neighbors(zlevels, px, py, z, map_width, map_height);

                // Decay rules for walls
                if tile.is_wall() {
                    // Walls with few supporting neighbors collapse
                    if wall_count < 3 {
                        // High chance to collapse
                        if rng.gen::<f32>() < decay_percentage * 0.8 {
                            if rng.gen_bool(0.6) {
                                changes.push((px, py, ZTile::Rubble));
                            } else {
                                changes.push((px, py, ZTile::Air));
                            }
                        }
                    } else if wall_count < 5 {
                        // Moderate chance to become ruined
                        if rng.gen::<f32>() < decay_percentage * 0.4 {
                            changes.push((px, py, ZTile::RuinedWall));
                        }
                    }
                }

                // Decay rules for floors
                if tile.is_floor() {
                    // Floors near missing walls accumulate rubble
                    let missing_walls = count_missing_wall_neighbors(zlevels, px, py, z, map_width, map_height);
                    if missing_walls > 2 {
                        if rng.gen::<f32>() < decay_percentage * 0.3 {
                            changes.push((px, py, ZTile::Rubble));
                        }
                    }
                }

                // Special feature decay
                match tile {
                    ZTile::Door | ZTile::Window => {
                        // Doors/windows can collapse
                        if rng.gen::<f32>() < decay_percentage * 0.5 {
                            changes.push((px, py, ZTile::Air));
                        }
                    }
                    ZTile::Column => {
                        // Columns can topple
                        if rng.gen::<f32>() < decay_percentage * 0.3 {
                            changes.push((px, py, ZTile::Rubble));
                        }
                    }
                    ZTile::Chest => {
                        // Some chests remain, some are looted (empty)
                        if rng.gen::<f32>() < decay_percentage * 0.6 {
                            changes.push((px, py, ZTile::StoneFloor));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Apply changes
        for (cx, cy, tile) in changes {
            zlevels.set(cx, cy, z, tile);
        }
    }

    // Final pass: clean up isolated structures
    cleanup_isolated_structures(zlevels, x, y, width, height, z, map_width, map_height, rng);
}

/// Count wall neighbors around a tile
fn count_wall_neighbors(
    zlevels: &Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    z: i32,
    map_width: usize,
    map_height: usize,
) -> usize {
    let mut count = 0;

    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let nx = (x as i32 + dx).rem_euclid(map_width as i32) as usize;
        let ny = (y as i32 + dy).clamp(0, map_height as i32 - 1) as usize;

        let neighbor = *zlevels.get(nx, ny, z);
        if neighbor.is_wall() {
            count += 1;
        }
    }

    count
}

/// Count neighbors that should be walls but aren't (for rubble accumulation)
fn count_missing_wall_neighbors(
    zlevels: &Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    z: i32,
    map_width: usize,
    map_height: usize,
) -> usize {
    let mut count = 0;

    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
        let nx = (x as i32 + dx).rem_euclid(map_width as i32) as usize;
        let ny = (y as i32 + dy).clamp(0, map_height as i32 - 1) as usize;

        let neighbor = *zlevels.get(nx, ny, z);
        if matches!(neighbor, ZTile::Air | ZTile::Rubble) {
            count += 1;
        }
    }

    count
}

/// Remove isolated single-tile structures
fn cleanup_isolated_structures(
    zlevels: &mut Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    map_width: usize,
    map_height: usize,
    rng: &mut ChaCha8Rng,
) {
    for dy in 0..height {
        for dx in 0..width {
            let px = (x + dx) % map_width;
            let py = (y + dy).min(map_height - 1);

            let tile = *zlevels.get(px, py, z);

            if tile.is_wall() {
                let wall_count = count_wall_neighbors(zlevels, px, py, z, map_width, map_height);

                // Isolated walls should collapse
                if wall_count == 0 {
                    if rng.gen_bool(0.8) {
                        zlevels.set(px, py, z, ZTile::Rubble);
                    }
                }
            }
        }
    }
}

/// Apply light decay (for villages - less ruined)
pub fn apply_light_decay(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
) {
    apply_decay(zlevels, surface_z, x, y, width, height, z, 0.3, 2, rng);
}

/// Apply heavy decay (for ancient cities)
pub fn apply_heavy_decay(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
) {
    apply_decay(zlevels, surface_z, x, y, width, height, z, 0.8, 4, rng);
}

/// Apply decay to all structures in the map
pub fn apply_global_decay(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    decay_percentage: f32,
    seed: u64,
) {
    use rand::SeedableRng;
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xDECA1));

    let width = zlevels.width;
    let height = zlevels.height;

    // Process each surface z-level
    for y in 0..height {
        for x in 0..width {
            let z = *surface_z.get(x, y);

            let tile = *zlevels.get(x, y, z);

            // Only decay structure tiles
            if !tile.is_structure() {
                continue;
            }

            // Count neighboring walls
            let wall_count = count_wall_neighbors(zlevels, x, y, z, width, height);

            // Apply decay based on isolation
            if tile.is_wall() {
                if wall_count < 3 {
                    if rng.gen::<f32>() < decay_percentage * 0.6 {
                        if rng.gen_bool(0.5) {
                            zlevels.set(x, y, z, ZTile::Rubble);
                        } else {
                            zlevels.set(x, y, z, ZTile::RuinedWall);
                        }
                    }
                } else if wall_count < 5 {
                    if rng.gen::<f32>() < decay_percentage * 0.3 {
                        zlevels.set(x, y, z, ZTile::RuinedWall);
                    }
                }
            }

            // Accumulate rubble on floors near damage
            if tile.is_floor() {
                let damage_nearby = count_missing_wall_neighbors(zlevels, x, y, z, width, height);
                if damage_nearby > 1 && rng.gen::<f32>() < decay_percentage * 0.2 {
                    zlevels.set(x, y, z, ZTile::Rubble);
                }
            }
        }
    }
}

/// Spread vegetation into ruins (overgrown effect)
pub fn apply_overgrowth(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    moisture: &Tilemap<f32>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
) {
    let map_width = zlevels.width;
    let map_height = zlevels.height;

    // In moist areas, ruins get overgrown
    for dy in 0..height {
        for dx in 0..width {
            let px = (x + dx) % map_width;
            let py = (y + dy).min(map_height - 1);

            let tile = *zlevels.get(px, py, z);
            let moist = *moisture.get(px, py);

            // Only process floor/rubble tiles
            if !matches!(tile, ZTile::StoneFloor | ZTile::WoodFloor | ZTile::DirtFloor | ZTile::Rubble) {
                continue;
            }

            // Higher moisture = more vegetation
            let veg_chance = moist * 0.3;

            if rng.gen::<f32>() < veg_chance {
                // Convert to cave moss (represents surface vegetation)
                zlevels.set(px, py, z, ZTile::CaveMoss);
            }
        }
    }
}
