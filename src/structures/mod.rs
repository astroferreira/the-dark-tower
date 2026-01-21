//! Human-made structures generation system
//!
//! This module generates human-made structures throughout the world:
//! - Abandoned castles on hilltops and strategic locations
//! - Ruins of cities in flat fertile areas
//! - Villages along roads and near resources
//! - Cave dwellings inside existing caves
//! - Roads connecting major structures
//!
//! Uses a combination of:
//! - Prefab library for small buildings
//! - BSP (Binary Space Partitioning) for room layouts
//! - L-systems for castle walls
//! - Cellular automata for decay/ruins effects
//! - Dijkstra's algorithm for road network and intelligent placement

pub mod generation;
pub mod placement;
pub mod prefabs;
pub mod types;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::water_bodies::WaterBodyId;
use crate::zlevel::{Tilemap3D, ZTile, MIN_Z};

use generation::{
    bsp::{generate_city_block, generate_dungeon_level},
    decay::{apply_decay, apply_overgrowth},
    lsystem::generate_complex_castle,
    roads::{generate_road_network, generate_village_paths},
};
use placement::{
    compute_castle_desirability, compute_city_desirability, compute_village_desirability,
    place_structures_from_desirability,
};
use prefabs::prefabs_by_tag;
use types::{PlacedStructure, StructureType};

/// Main entry point for structure generation
///
/// This function orchestrates the entire structure generation process:
/// 1. Compute desirability maps for each structure type
/// 2. Place major structures (castles, cities) at optimal locations
/// 3. Generate road network connecting structures
/// 4. Place villages along roads
/// 5. Generate structure interiors (BSP, L-systems)
/// 6. Place cave dwellings in existing caves
/// 7. Apply decay/ruins effect
pub fn generate_structures(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    stress_map: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    seed: u64,
) -> Vec<PlacedStructure> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0x5701C7));

    let width = heightmap.width;
    let height = heightmap.height;

    let mut all_structures: Vec<PlacedStructure> = Vec::new();

    // Scale structure counts based on map size (base is 512x256)
    let map_area = width * height;
    let base_area = 512 * 256;
    let scale_factor = ((map_area as f32 / base_area as f32).sqrt()).max(0.5).min(2.0);

    println!("  Computing desirability maps... (scale factor: {:.2})", scale_factor);

    // Phase 1: Place castles (very rare - ancient ruins on hilltops)
    let castle_count = if rng.gen_bool(0.5 * scale_factor as f64) { 1 } else { 0 };
    let mut castle_desirability = compute_castle_desirability(
        heightmap,
        stress_map,
        water_bodies,
        biomes,
        &all_structures,
    );

    let castles = place_structures_from_desirability(
        &mut castle_desirability,
        StructureType::Castle,
        castle_count,
        surface_z,
    );

    println!("  Placed {} castles", castles.len());
    all_structures.extend(castles.clone());

    // Phase 2: Place cities (rare - ruined settlements near water)
    let city_count = if rng.gen_bool(0.4 * scale_factor as f64) { 1 } else { 0 };
    let mut city_desirability = compute_city_desirability(
        heightmap,
        moisture,
        temperature,
        water_bodies,
        biomes,
        &all_structures,
    );

    let cities = place_structures_from_desirability(
        &mut city_desirability,
        StructureType::City,
        city_count,
        surface_z,
    );

    println!("  Placed {} cities", cities.len());
    all_structures.extend(cities.clone());

    // Phase 3: Generate road network
    println!("  Generating road network...");
    let _road_segments = generate_road_network(
        &all_structures,
        heightmap,
        water_bodies,
        zlevels,
        surface_z,
        &mut rng,
    );

    // Phase 4: Place villages (occasional small settlements)
    let village_count = ((rng.gen_range(0..=2) as f32 * scale_factor) as usize);
    let mut village_desirability = compute_village_desirability(
        heightmap,
        moisture,
        water_bodies,
        biomes,
        &all_structures,
    );

    let villages = place_structures_from_desirability(
        &mut village_desirability,
        StructureType::Village,
        village_count,
        surface_z,
    );

    println!("  Placed {} villages", villages.len());
    all_structures.extend(villages.clone());

    // Phase 5: Generate structure interiors
    println!("  Generating structure interiors...");

    // Generate castle interiors
    for castle in &castles {
        let (size_min, size_max) = StructureType::Castle.size_range();
        let size = rng.gen_range(size_min..=size_max);

        generate_complex_castle(
            zlevels,
            castle.x + castle.width / 2,
            castle.y + castle.height / 2,
            size,
            castle.z,
            &mut rng,
            width,
            height,
        );

        // Generate dungeon below castle
        if castle.z > MIN_Z + 2 {
            let dungeon_z = castle.z - 1;
            let _rooms = generate_dungeon_level(
                zlevels,
                castle.x,
                castle.y,
                castle.width,
                castle.height,
                dungeon_z,
                &mut rng,
                width,
                height,
            );
        }
    }

    // Generate city interiors
    for city in &cities {
        let (size_min, size_max) = StructureType::City.size_range();
        let size = rng.gen_range(size_min..=size_max);

        generate_city_block(
            zlevels,
            city.x,
            city.y,
            size,
            size,
            city.z,
            &mut rng,
            width,
            height,
        );
    }

    // Generate village interiors using prefabs
    let village_prefabs = prefabs_by_tag("village");
    for village in &villages {
        generate_village(
            zlevels,
            surface_z,
            village,
            &village_prefabs,
            &mut rng,
            width,
            height,
        );

        // Add village paths
        generate_village_paths(
            zlevels,
            surface_z,
            village.x + village.width / 2,
            village.y + village.height / 2,
            village.width / 2,
            &mut rng,
        );
    }

    // Phase 6: Place cave dwellings
    println!("  Placing cave dwellings...");
    let cave_dwellings = place_cave_dwellings(
        zlevels,
        surface_z,
        &mut rng,
        width,
        height,
    );
    all_structures.extend(cave_dwellings);

    // Phase 7: Generate mines and underground fortresses
    println!("  Generating mines...");
    let mines = generation::generate_mines(
        zlevels,
        surface_z,
        heightmap,
        stress_map,
        &mut rng,
        scale_factor,
    );
    println!("  Placed {} mines", mines.len());
    all_structures.extend(mines);

    // Generate standalone underground fortresses (very rare)
    let fortresses = generation::generate_standalone_fortress(
        zlevels,
        surface_z,
        heightmap,
        &mut rng,
        scale_factor,
        width,
        height,
    );
    if !fortresses.is_empty() {
        println!("  Placed {} underground fortresses", fortresses.len());
        all_structures.extend(fortresses);
    }

    // Phase 8: Apply decay
    println!("  Applying decay to structures...");
    for structure in &all_structures {
        let decay_pct = structure.structure_type.decay_percentage();
        let iterations = match structure.structure_type {
            StructureType::Castle => 3,
            StructureType::City => 4,
            StructureType::Village => 2,
            StructureType::CaveDwelling => 2,
            StructureType::Dungeon => 2,
        };

        apply_decay(
            zlevels,
            surface_z,
            structure.x,
            structure.y,
            structure.width,
            structure.height,
            structure.z,
            decay_pct,
            iterations,
            &mut rng,
        );

        // Apply overgrowth in moist areas
        apply_overgrowth(
            zlevels,
            surface_z,
            moisture,
            structure.x,
            structure.y,
            structure.width,
            structure.height,
            structure.z,
            &mut rng,
        );
    }

    println!("  Total structures placed: {}", all_structures.len());

    all_structures
}

/// Generate a village using organic shapes (mixed round and rounded rectangle buildings)
fn generate_village(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    village: &PlacedStructure,
    prefabs: &[types::Prefab],
    rng: &mut ChaCha8Rng,
    map_width: usize,
    map_height: usize,
) {
    // Use organic village generation for varied, rounded shapes
    generation::generate_organic_village(
        zlevels,
        village.x + village.width / 2,
        village.y + village.height / 2,
        village.width.min(village.height) / 2,
        village.z,
        rng,
        map_width,
        map_height,
    );

    // Optionally place a few prefabs for variety (like wells, market stalls)
    if !prefabs.is_empty() && rng.gen_bool(0.5) {
        let special_prefabs: Vec<_> = prefabs.iter()
            .filter(|p| p.has_tag("well") || p.has_tag("market"))
            .collect();

        if !special_prefabs.is_empty() {
            let prefab = special_prefabs[rng.gen_range(0..special_prefabs.len())];
            let world_x = village.x + village.width / 2;
            let world_y = village.y + village.height / 2;

            place_prefab(
                zlevels,
                surface_z,
                prefab,
                world_x,
                world_y,
                map_width,
                map_height,
            );
        }
    }
}

/// Place a single prefab at a location
fn place_prefab(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    prefab: &types::Prefab,
    x: usize,
    y: usize,
    map_width: usize,
    map_height: usize,
) {
    for py in 0..prefab.height {
        for px in 0..prefab.width {
            let world_x = (x + px) % map_width;
            let world_y = (y + py).min(map_height - 1);

            if let Some(tile) = prefab.get(px, py) {
                let z = *surface_z.get(world_x, world_y);
                zlevels.set(world_x, world_y, z, tile);
            }
        }
    }
}

/// Place cave dwellings inside existing caves
fn place_cave_dwellings(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) -> Vec<PlacedStructure> {
    let mut dwellings = Vec::new();

    // Find suitable cave chambers (large cave floor areas)
    let mut candidates: Vec<(usize, usize, i32)> = Vec::new();

    for z in MIN_Z..0 {
        for y in 5..(height - 5) {
            for x in 5..(width - 5) {
                let tile = *zlevels.get(x, y, z);

                if tile == ZTile::CaveFloor {
                    // Check if this is a large enough chamber
                    let chamber_size = count_connected_cave_floor(zlevels, x, y, z, width, height);

                    if chamber_size >= 20 {
                        candidates.push((x, y, z));
                    }
                }
            }
        }
    }

    // Place dwellings in some of the candidates (sparse - only a few cave settlements)
    let num_dwellings = rng.gen_range(1..=3).min(candidates.len());

    // Shuffle and take first N
    for _ in 0..num_dwellings.min(candidates.len()) {
        if candidates.is_empty() {
            break;
        }

        let idx = rng.gen_range(0..candidates.len());
        let (x, y, z) = candidates.remove(idx);

        // Generate a small dwelling
        let dwelling_size = rng.gen_range(5..=10);

        generate_cave_dwelling(
            zlevels,
            x,
            y,
            z,
            dwelling_size,
            rng,
            width,
            height,
        );

        dwellings.push(PlacedStructure::new(
            x.saturating_sub(dwelling_size / 2),
            y.saturating_sub(dwelling_size / 2),
            z,
            dwelling_size,
            dwelling_size,
            StructureType::CaveDwelling,
        ));

        // Remove nearby candidates to avoid overlap
        candidates.retain(|(cx, cy, cz)| {
            let dx = (*cx as i32 - x as i32).abs();
            let dy = (*cy as i32 - y as i32).abs();
            let dz = (*cz - z).abs();
            dx > 20 || dy > 20 || dz > 2
        });
    }

    dwellings
}

/// Count connected cave floor tiles (flood fill)
fn count_connected_cave_floor(
    zlevels: &Tilemap3D<ZTile>,
    start_x: usize,
    start_y: usize,
    z: i32,
    width: usize,
    height: usize,
) -> usize {
    use std::collections::HashSet;

    let mut visited = HashSet::new();
    let mut stack = vec![(start_x, start_y)];
    let mut count = 0;
    let max_count = 100; // Limit search

    while let Some((x, y)) = stack.pop() {
        if count >= max_count {
            break;
        }

        if visited.contains(&(x, y)) {
            continue;
        }
        visited.insert((x, y));

        let tile = *zlevels.get(x, y, z);
        if tile != ZTile::CaveFloor {
            continue;
        }

        count += 1;

        // Add neighbors
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            if !visited.contains(&(nx, ny)) {
                stack.push((nx, ny));
            }
        }
    }

    count
}

/// Generate a cave dwelling
fn generate_cave_dwelling(
    zlevels: &mut Tilemap3D<ZTile>,
    center_x: usize,
    center_y: usize,
    z: i32,
    size: usize,
    rng: &mut ChaCha8Rng,
    width: usize,
    height: usize,
) {
    let half = size / 2;

    // Add mine supports around the edges
    for i in 0..size {
        // Top and bottom edges
        for (dx, dy) in [(i, 0), (i, size - 1)] {
            let px = (center_x + dx).wrapping_sub(half) % width;
            let py = (center_y + dy).saturating_sub(half).min(height - 1);

            let current = *zlevels.get(px, py, z);
            if current == ZTile::CaveFloor {
                if rng.gen_bool(0.4) {
                    zlevels.set(px, py, z, ZTile::MineSupport);
                }
            }
        }

        // Left and right edges
        for (dx, dy) in [(0, i), (size - 1, i)] {
            let px = (center_x + dx).wrapping_sub(half) % width;
            let py = (center_y + dy).saturating_sub(half).min(height - 1);

            let current = *zlevels.get(px, py, z);
            if current == ZTile::CaveFloor {
                if rng.gen_bool(0.4) {
                    zlevels.set(px, py, z, ZTile::MineSupport);
                }
            }
        }
    }

    // Convert some cave floor to mined room
    for dy in 1..(size - 1) {
        for dx in 1..(size - 1) {
            let px = (center_x + dx).wrapping_sub(half) % width;
            let py = (center_y + dy).saturating_sub(half).min(height - 1);

            let current = *zlevels.get(px, py, z);
            if current == ZTile::CaveFloor {
                zlevels.set(px, py, z, ZTile::MinedRoom);
            }
        }
    }

    // Add some torches
    let num_torches = rng.gen_range(2..=4);
    for _ in 0..num_torches {
        let tx = center_x.wrapping_sub(half) + rng.gen_range(1..size - 1);
        let ty = center_y.saturating_sub(half) + rng.gen_range(1..size - 1);
        let px = tx % width;
        let py = ty.min(height - 1);

        let current = *zlevels.get(px, py, z);
        if current == ZTile::MinedRoom {
            zlevels.set(px, py, z, ZTile::Torch);
        }
    }

    // Add a chest
    if rng.gen_bool(0.6) {
        let cx = center_x % width;
        let cy = center_y.min(height - 1);

        let current = *zlevels.get(cx, cy, z);
        if current == ZTile::MinedRoom || current == ZTile::CaveFloor {
            zlevels.set(cx, cy, z, ZTile::Chest);
        }
    }
}
