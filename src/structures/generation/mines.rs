//! Mine generation system
//!
//! Generates mines, mine shafts, tunnels, and underground fortresses.
//! Mines follow ore veins and consist of:
//! - Mine entrances at the surface (near mountains/hills)
//! - Vertical shafts connecting Z-levels
//! - Horizontal tunnels following ore veins
//! - Mine chambers for larger excavations
//! - Underground fortresses built by miners

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use noise::{NoiseFn, Perlin};
use std::collections::{HashSet, VecDeque};

use crate::tilemap::Tilemap;
use crate::zlevel::{ZTile, Tilemap3D, MIN_Z};
use crate::structures::types::{PlacedStructure, StructureType};
use super::shapes::{filled_circle, circle_outline, extract_outline, extract_interior};

/// Configuration for mine generation
pub struct MineConfig {
    /// Minimum depth of mines (negative Z)
    pub min_depth: i32,
    /// Maximum depth of mines (negative Z)
    pub max_depth: i32,
    /// Probability of placing ore veins along tunnels
    pub ore_probability: f32,
    /// Probability of rich ore vs regular ore
    pub rich_ore_probability: f32,
    /// Whether to add mine rails
    pub add_rails: bool,
    /// Whether to add support beams
    pub add_supports: bool,
}

impl Default for MineConfig {
    fn default() -> Self {
        Self {
            min_depth: -3,
            max_depth: -10,
            ore_probability: 0.15,
            rich_ore_probability: 0.2,
            add_rails: true,
            add_supports: true,
        }
    }
}

/// Generate mines throughout the world
pub fn generate_mines(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
    scale_factor: f32,
) -> Vec<PlacedStructure> {
    let width = heightmap.width;
    let height = heightmap.height;
    let config = MineConfig::default();

    let mut mines = Vec::new();

    // Find suitable mine entrance locations (hills/mountains with high stress = ore)
    // Mines are common - abandoned mining operations are everywhere
    let mine_count = ((rng.gen_range(3..=6) as f32 * scale_factor) as usize).max(2);
    let candidates = find_mine_candidates(heightmap, stress_map, surface_z, width, height);

    if candidates.is_empty() {
        return mines;
    }

    // Place mines at best candidates
    let mut placed_positions: HashSet<(usize, usize)> = HashSet::new();

    for _ in 0..mine_count {
        // Find a candidate not too close to existing mines
        let mut best_candidate = None;
        for &(x, y, score) in &candidates {
            let too_close = placed_positions.iter().any(|&(px, py)| {
                let dx = x as i32 - px as i32;
                let dy = y as i32 - py as i32;
                (dx * dx + dy * dy) < 900 // Minimum 30 tiles apart (reduced from 50)
            });

            if !too_close {
                best_candidate = Some((x, y, score));
                break;
            }
        }

        if let Some((x, y, _score)) = best_candidate {
            placed_positions.insert((x, y));

            let z = *surface_z.get(x, y);
            let depth = rng.gen_range(config.max_depth..=config.min_depth);

            // Generate the mine
            let mine_size = generate_mine(
                zlevels,
                surface_z,
                x, y, z,
                depth,
                &config,
                rng,
                width, height,
            );

            mines.push(PlacedStructure::new(
                x.saturating_sub(mine_size / 2),
                y.saturating_sub(mine_size / 2),
                z,
                mine_size,
                mine_size,
                StructureType::CaveDwelling, // Reusing this type for mines
            ));
        }
    }

    mines
}

/// Find candidate locations for mine entrances
fn find_mine_candidates(
    heightmap: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    surface_z: &Tilemap<i32>,
    width: usize,
    height: usize,
) -> Vec<(usize, usize, f32)> {
    let mut candidates = Vec::new();

    // Sample every 8 tiles for efficiency
    for y in (10..height - 10).step_by(8) {
        for x in (10..width - 10).step_by(8) {
            let h = *heightmap.get(x, y);
            let stress = *stress_map.get(x, y);
            let z = *surface_z.get(x, y);

            // Mines prefer: elevated areas with tectonic stress (ore deposits)
            // Must be above sea level (z >= 0) and not too high (not glaciers)
            // Lowered height threshold from 0.3 to 0.2 to allow more candidates
            if h > 0.2 && h < 0.9 && z >= 0 {
                let elevation_score = (h - 0.2) * 1.5; // Normalized elevation bonus
                let stress_score = stress.abs() * 2.0; // Any stress indicates geology

                let score = elevation_score + stress_score;
                if score > 0.1 { // Minimum score threshold
                    candidates.push((x, y, score));
                }
            }
        }
    }

    // Sort by score descending
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Keep top 50 candidates for mine placement
    candidates.truncate(50);
    candidates
}

/// Generate a complete mine at a location
fn generate_mine(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    entrance_x: usize,
    entrance_y: usize,
    entrance_z: i32,
    target_depth: i32,
    config: &MineConfig,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) -> usize {
    // Place mine entrance at surface
    zlevels.set(entrance_x, entrance_y, entrance_z, ZTile::MineEntrance);

    // Generate the main shaft going down
    let shaft_depth = generate_mine_shaft(
        zlevels,
        entrance_x, entrance_y,
        entrance_z,
        target_depth,
        rng,
        width, height,
    );

    // Generate horizontal tunnel networks at various depths
    let mut total_size = 5;
    let num_levels = ((entrance_z - target_depth).abs() / 2).max(1);

    for level in 0..num_levels {
        let z = entrance_z - 1 - (level * 2);
        if z < MIN_Z + 2 {
            break;
        }

        // Generate tunnels radiating from the shaft
        let tunnel_size = generate_mine_tunnels(
            zlevels,
            entrance_x, entrance_y, z,
            config,
            rng,
            width, height,
        );

        total_size = total_size.max(tunnel_size);
    }

    // Often generate an underground fortress at the deepest level
    // Miners frequently established fortified bases deep underground
    if rng.gen_bool(0.6) && shaft_depth > 2 {
        let fortress_z = target_depth.max(MIN_Z + 3);
        generate_underground_fortress(
            zlevels,
            entrance_x, entrance_y, fortress_z,
            rng,
            width, height,
        );
    }

    total_size
}

/// Generate a vertical mine shaft
fn generate_mine_shaft(
    zlevels: &mut Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    start_z: i32,
    end_z: i32,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) -> i32 {
    let mut current_x = x as i32;
    let mut current_y = y as i32;
    let mut depth = 0;

    // Shaft can meander slightly as it descends
    for z in (end_z..start_z).rev() {
        if !zlevels.is_valid_z(z) {
            continue;
        }

        // Small random drift
        if rng.gen_bool(0.2) {
            current_x += rng.gen_range(-1..=1);
            current_y += rng.gen_range(-1..=1);
        }

        let px = current_x.rem_euclid(width as i32) as usize;
        let py = current_y.clamp(1, height as i32 - 2) as usize;

        // Carve the shaft (2x2 area)
        for dy in 0..=1 {
            for dx in 0..=1 {
                let sx = (px + dx) % width;
                let sy = (py + dy).min(height - 1);

                let current = *zlevels.get(sx, sy, z);
                if current == ZTile::Solid || current == ZTile::CaveFloor {
                    zlevels.set(sx, sy, z, ZTile::MineShaft);
                }
            }
        }

        // Add ladder on one side
        zlevels.set(px, py, z, ZTile::MineLadder);

        depth += 1;
    }

    depth
}

/// Generate horizontal mine tunnels at a Z-level
fn generate_mine_tunnels(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    z: i32,
    config: &MineConfig,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) -> usize {
    let mut max_extent = 0;

    // Generate 2-4 tunnels radiating outward
    let num_tunnels = rng.gen_range(2..=4);
    let ore_noise = Perlin::new(rng.gen());

    for i in 0..num_tunnels {
        // Direction for this tunnel
        let base_angle = (i as f32 / num_tunnels as f32) * std::f32::consts::TAU;
        let angle = base_angle + rng.gen_range(-0.3..0.3);

        let dx = angle.cos();
        let dy = angle.sin();

        // Tunnel length
        let tunnel_length = rng.gen_range(15..=35);

        let mut x = center_x as f32;
        let mut y = center_y as f32;

        for step in 0..tunnel_length {
            // Add some waviness to the tunnel
            let wave = (step as f32 * 0.2).sin() * 1.5;
            let perp_dx = -dy;
            let perp_dy = dx;

            let tx = (x + perp_dx * wave) as i32;
            let ty = (y + perp_dy * wave) as i32;

            let px = tx.rem_euclid(width as i32) as usize;
            let py = ty.clamp(1, height as i32 - 2) as usize;

            // Carve the tunnel (1-2 tiles wide)
            let tunnel_width = if step % 5 == 0 { 2 } else { 1 };

            for w in 0..tunnel_width {
                let wp = ((px as i32 + (perp_dx * w as f32) as i32).rem_euclid(width as i32)) as usize;
                let current = *zlevels.get(wp, py, z);

                if current == ZTile::Solid {
                    zlevels.set(wp, py, z, ZTile::MinedTunnel);

                    // Check for ore veins using noise
                    let noise_val = ore_noise.get([px as f64 * 0.1, py as f64 * 0.1, z as f64 * 0.5]);
                    if noise_val > 0.5 && rng.gen_bool(config.ore_probability as f64) {
                        // Place ore in adjacent wall
                        for (ox, oy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                            let ore_x = (px as i32 + ox).rem_euclid(width as i32) as usize;
                            let ore_y = (py as i32 + oy).clamp(0, height as i32 - 1) as usize;
                            let adj = *zlevels.get(ore_x, ore_y, z);

                            if adj == ZTile::Solid {
                                let ore_tile = if rng.gen_bool(config.rich_ore_probability as f64) {
                                    ZTile::RichOreVein
                                } else {
                                    ZTile::OreVein
                                };
                                zlevels.set(ore_x, ore_y, z, ore_tile);
                                break;
                            }
                        }
                    }

                    // Add mine rails on main tunnels
                    if config.add_rails && w == 0 && step % 2 == 0 {
                        zlevels.set(wp, py, z, ZTile::MineRails);
                    }
                }
            }

            // Add support beams periodically
            if config.add_supports && step % 6 == 0 {
                // Check if there's solid above
                if zlevels.is_valid_z(z + 1) {
                    let above = *zlevels.get(px, py, z + 1);
                    if above == ZTile::Solid {
                        zlevels.set(px, py, z + 1, ZTile::MineSupport);
                    }
                }
            }

            // Track extent
            let dist = ((px as i32 - center_x as i32).pow(2) + (py as i32 - center_y as i32).pow(2)) as f32;
            max_extent = max_extent.max(dist.sqrt() as usize);

            x += dx;
            y += dy;
        }

        // Possibly add a mine chamber at the end
        if rng.gen_bool(0.5) {
            let chamber_x = (x as i32).rem_euclid(width as i32) as usize;
            let chamber_y = (y as i32).clamp(2, height as i32 - 3) as usize;

            generate_mine_chamber(
                zlevels,
                chamber_x, chamber_y, z,
                rng,
                width, height,
            );
        }
    }

    max_extent * 2
}

/// Generate a mine chamber (larger excavated area)
fn generate_mine_chamber(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) {
    let radius = rng.gen_range(3..=5);
    let chamber = filled_circle(center_x as i32, center_y as i32, radius);

    for (cx, cy) in chamber {
        let px = cx.rem_euclid(width as i32) as usize;
        let py = cy.clamp(0, height as i32 - 1) as usize;

        let current = *zlevels.get(px, py, z);
        if current == ZTile::Solid || current == ZTile::MinedTunnel {
            zlevels.set(px, py, z, ZTile::MinedRoom);
        }
    }

    // Add support pillars
    let pillars = [
        (center_x.saturating_sub(2), center_y.saturating_sub(2)),
        (center_x + 2, center_y.saturating_sub(2)),
        (center_x.saturating_sub(2), center_y + 2),
        (center_x + 2, center_y + 2),
    ];

    for (px, py) in pillars {
        let wx = px % width;
        let wy = py.min(height - 1);
        if *zlevels.get(wx, wy, z) == ZTile::MinedRoom {
            zlevels.set(wx, wy, z, ZTile::MineSupport);
            // Support extends up
            if zlevels.is_valid_z(z + 1) {
                zlevels.set(wx, wy, z + 1, ZTile::MineSupport);
            }
        }
    }

    // Add a torch
    let torch_x = (center_x + 1) % width;
    let torch_y = center_y.min(height - 1);
    zlevels.set(torch_x, torch_y, z, ZTile::Torch);

    // Possibly add a chest
    if rng.gen_bool(0.3) {
        let chest_x = center_x % width;
        let chest_y = (center_y + 1).min(height - 1);
        if *zlevels.get(chest_x, chest_y, z) == ZTile::MinedRoom {
            zlevels.set(chest_x, chest_y, z, ZTile::Chest);
        }
    }
}

/// Generate an underground fortress (dwarven-style)
pub fn generate_underground_fortress(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    z: i32,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) {
    let fortress_radius = rng.gen_range(12..=20);

    // Create the main hall (circular)
    let main_hall = filled_circle(center_x as i32, center_y as i32, fortress_radius / 2);
    let walls = extract_outline(&main_hall);
    let interior = extract_interior(&main_hall);

    // Place fortress walls (multiple Z-levels for height)
    for &(wx, wy) in &walls {
        let px = wx.rem_euclid(width as i32) as usize;
        let py = wy.clamp(0, height as i32 - 1) as usize;

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(px, py, z + dz, ZTile::FortressWall);
            }
        }
    }

    // Place fortress floor
    for &(fx, fy) in &interior {
        let px = fx.rem_euclid(width as i32) as usize;
        let py = fy.clamp(0, height as i32 - 1) as usize;
        zlevels.set(px, py, z, ZTile::FortressFloor);
    }

    // Add fortress gate (entrance)
    if !walls.is_empty() {
        let gate_idx = rng.gen_range(0..walls.len());
        let (gx, gy) = walls[gate_idx];
        let px = gx.rem_euclid(width as i32) as usize;
        let py = gy.clamp(0, height as i32 - 1) as usize;

        for dz in 0..=1 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(px, py, z + dz, ZTile::FortressGate);
            }
        }
    }

    // Add rooms around the main hall
    let room_angles = [0.0f32, 1.57, 3.14, 4.71]; // N, E, S, W

    for (i, &angle) in room_angles.iter().enumerate() {
        let room_dist = fortress_radius as f32 * 0.7;
        let room_x = center_x as f32 + angle.cos() * room_dist;
        let room_y = center_y as f32 + angle.sin() * room_dist;

        let rx = (room_x as i32).rem_euclid(width as i32) as usize;
        let ry = (room_y as i32).clamp(2, height as i32 - 3) as usize;

        // Different room types
        let room_type = match i {
            0 => RoomType::Barracks,
            1 => RoomType::Forge,
            2 => RoomType::Vault,
            _ => RoomType::Cistern,
        };

        generate_fortress_room(
            zlevels,
            rx, ry, z,
            room_type,
            rng,
            width, height,
        );

        // Connect room to main hall with corridor
        carve_corridor(
            zlevels,
            center_x, center_y,
            rx, ry,
            z,
            width, height,
        );
    }

    // Add pillars in main hall
    let pillar_positions = [
        (center_x.saturating_sub(3), center_y.saturating_sub(3)),
        (center_x + 3, center_y.saturating_sub(3)),
        (center_x.saturating_sub(3), center_y + 3),
        (center_x + 3, center_y + 3),
    ];

    for (px, py) in pillar_positions {
        let wx = px % width;
        let wy = py.min(height - 1);

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) && *zlevels.get(wx, wy, z + dz) != ZTile::FortressWall {
                zlevels.set(wx, wy, z + dz, ZTile::Column);
            }
        }
    }

    // Add torches
    for &(px, py) in &pillar_positions {
        let tx = (px + 1) % width;
        let ty = py.min(height - 1);
        if *zlevels.get(tx, ty, z) == ZTile::FortressFloor {
            zlevels.set(tx, ty, z, ZTile::Torch);
        }
    }
}

#[derive(Clone, Copy)]
enum RoomType {
    Barracks,
    Forge,
    Vault,
    Cistern,
}

/// Generate a specific type of fortress room
fn generate_fortress_room(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    z: i32,
    room_type: RoomType,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) {
    let radius = rng.gen_range(4..=6);
    let room = filled_circle(center_x as i32, center_y as i32, radius);
    let walls = extract_outline(&room);
    let interior = extract_interior(&room);

    // Place walls
    for &(wx, wy) in &walls {
        let px = wx.rem_euclid(width as i32) as usize;
        let py = wy.clamp(0, height as i32 - 1) as usize;

        for dz in 0..=2 {
            if zlevels.is_valid_z(z + dz) {
                zlevels.set(px, py, z + dz, ZTile::FortressWall);
            }
        }
    }

    // Place floor based on room type
    let floor_tile = match room_type {
        RoomType::Barracks => ZTile::BarracksFloor,
        RoomType::Forge => ZTile::ForgeFloor,
        RoomType::Vault => ZTile::Vault,
        RoomType::Cistern => ZTile::Cistern,
    };

    for (fx, fy) in interior {
        let px = fx.rem_euclid(width as i32) as usize;
        let py = fy.clamp(0, height as i32 - 1) as usize;
        zlevels.set(px, py, z, floor_tile);
    }

    // Add room-specific features
    match room_type {
        RoomType::Vault => {
            // Add chest in center
            zlevels.set(center_x, center_y, z, ZTile::Chest);
        }
        RoomType::Forge => {
            // Add torch for light
            let tx = (center_x + 1) % width;
            zlevels.set(tx, center_y, z, ZTile::Torch);
        }
        RoomType::Barracks => {
            // Add torch
            zlevels.set(center_x, center_y, z, ZTile::Torch);
        }
        RoomType::Cistern => {
            // Water in center
            zlevels.set(center_x, center_y, z, ZTile::Water);
        }
    }
}

/// Carve a corridor between two points
fn carve_corridor(
    zlevels: &mut Tilemap3D<ZTile>,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    z: i32,
    width: usize,
    height: usize,
) {
    let mut x = x1 as i32;
    let mut y = y1 as i32;
    let tx = x2 as i32;
    let ty = y2 as i32;

    // L-shaped corridor
    while x != tx {
        let px = x.rem_euclid(width as i32) as usize;
        let py = y.clamp(0, height as i32 - 1) as usize;

        if *zlevels.get(px, py, z) == ZTile::Solid {
            zlevels.set(px, py, z, ZTile::FortressFloor);
        }

        if x < tx { x += 1; } else { x -= 1; }
    }

    while y != ty {
        let px = x.rem_euclid(width as i32) as usize;
        let py = y.clamp(0, height as i32 - 1) as usize;

        if *zlevels.get(px, py, z) == ZTile::Solid {
            zlevels.set(px, py, z, ZTile::FortressFloor);
        }

        if y < ty { y += 1; } else if y > 0 { y -= 1; } else { break; }
    }
}

/// Generate standalone underground fortress (not connected to mine)
pub fn generate_standalone_fortress(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
    scale_factor: f32,
    width: usize,
    height: usize,
) -> Vec<PlacedStructure> {
    let mut fortresses = Vec::new();

    // Standalone fortresses - ancient dwarven halls hidden deep underground
    let fortress_count = ((rng.gen_range(1..=2) as f32 * scale_factor) as usize).min(2);

    if fortress_count == 0 {
        return fortresses;
    }

    // Find a suitable deep underground location
    for _ in 0..10 {
        let x = rng.gen_range(20..width - 20);
        let y = rng.gen_range(20..height - 20);
        let surf_z = *surface_z.get(x, y);
        let h = *heightmap.get(x, y);

        // Prefer areas under mountains
        if h > 0.4 && surf_z > 0 {
            let fortress_z = rng.gen_range(MIN_Z + 5..MIN_Z + 10);

            // Check if area is mostly solid
            let mut solid_count = 0;
            for dy in -5..=5 {
                for dx in -5..=5 {
                    let cx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    let cy = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    if *zlevels.get(cx, cy, fortress_z) == ZTile::Solid {
                        solid_count += 1;
                    }
                }
            }

            if solid_count > 80 {
                generate_underground_fortress(
                    zlevels,
                    x, y, fortress_z,
                    rng,
                    width, height,
                );

                fortresses.push(PlacedStructure::new(
                    x.saturating_sub(20),
                    y.saturating_sub(20),
                    fortress_z,
                    40,
                    40,
                    StructureType::Dungeon,
                ));

                break;
            }
        }
    }

    fortresses
}
