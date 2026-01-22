//! Debug export for local map analysis
//!
//! Exports local chunk data as readable text for debugging chunk boundary issues.

use std::fs::File;
use std::io::{Write, BufWriter};
use std::path::Path;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::world::WorldData;
use crate::biomes::ExtendedBiome;
use super::local::{LocalChunk, LocalTerrain, LocalFeature, generate_local_chunk};
use super::geology::{get_corner_biomes, is_water_biome, calculate_noise_water_factor};
use super::LOCAL_SIZE;
use noise::Perlin;

/// Debug info for a single local tile
struct TileDebug {
    terrain_char: char,
    water_factor: f32,
    is_water_terrain: bool,
}

/// Export debug view of local chunks around a world position
pub fn export_debug_local_maps(
    world: &WorldData,
    center_wx: usize,
    center_wy: usize,
    output_path: &str,
) -> std::io::Result<()> {
    let file = File::create(output_path)?;
    let mut w = BufWriter::new(file);

    writeln!(w, "=== LOCAL MAP DEBUG EXPORT ===")?;
    writeln!(w, "Center world position: ({}, {})", center_wx, center_wy)?;
    writeln!(w, "World seed: {}", world.seed)?;
    writeln!(w, "Local chunk size: {}x{}", LOCAL_SIZE, LOCAL_SIZE)?;
    writeln!(w, "")?;

    // Generate 3x3 grid of chunks around center
    let mut chunks: [[Option<LocalChunk>; 3]; 3] = Default::default();

    writeln!(w, "=== WORLD TILE INFO (3x3 around center) ===")?;
    writeln!(w, "Note: Grid position [gx,gy] where [1,1] is center")?;
    for dy in 0..3i32 {
        for dx in 0..3i32 {
            let wx = (center_wx as i32 + dx - 1).max(0) as usize;
            let wy = (center_wy as i32 + dy - 1).max(0) as usize;

            if wx >= world.heightmap.width || wy >= world.heightmap.height {
                continue;
            }

            let biome = *world.biomes.get(wx, wy);
            let height = *world.heightmap.get(wx, wy);
            let water_id = *world.water_body_map.get(wx, wy);
            let corner_biomes = get_corner_biomes(world, wx, wy);
            let world_surface_z = *world.surface_z.get(wx, wy);

            writeln!(w, "")?;
            writeln!(w, "World tile ({}, {}) [grid {}, {}]:", wx, wy, dx, dy)?;
            writeln!(w, "  Biome: {:?}", biome)?;
            writeln!(w, "  Height: {:.1}m", height)?;
            writeln!(w, "  World surface_z: {}", world_surface_z)?;
            writeln!(w, "  Water body ID: {:?}", water_id)?;
            writeln!(w, "  Corner biomes:")?;
            writeln!(w, "    NW: {:?} (water={})", corner_biomes[0][0], is_water_biome(corner_biomes[0][0]))?;
            writeln!(w, "    NE: {:?} (water={})", corner_biomes[0][1], is_water_biome(corner_biomes[0][1]))?;
            writeln!(w, "    SW: {:?} (water={})", corner_biomes[1][0], is_water_biome(corner_biomes[1][0]))?;
            writeln!(w, "    SE: {:?} (water={})", corner_biomes[1][1], is_water_biome(corner_biomes[1][1]))?;

            let water_count = corner_biomes.iter().flatten()
                .filter(|&&b| is_water_biome(b)).count();
            writeln!(w, "  Water corner count: {}/4", water_count)?;

            // Generate chunk
            let chunk = generate_local_chunk(world, wx, wy);

            // Get corner heights for this chunk
            let corner_heights = super::geology::get_corner_surface_heights(world, wx, wy);
            writeln!(w, "  Chunk surface_z: {}, z_range: {}..{}", chunk.surface_z, chunk.z_min, chunk.z_max)?;
            writeln!(w, "  Corner heights: NW={}, NE={}, SW={}, SE={}",
                corner_heights[0][0], corner_heights[0][1],
                corner_heights[1][0], corner_heights[1][1])?;
            chunks[dx as usize][dy as usize] = Some(chunk);
        }
    }

    // Get surface z for the center chunk
    let center_chunk = chunks[1][1].as_ref().unwrap();
    let surface_z = center_chunk.surface_z;

    writeln!(w, "")?;
    writeln!(w, "=== TERRAIN MAP (surface z={}) ===", surface_z)?;
    writeln!(w, "Legend: . = floor, ~ = water, # = wall/solid, ' ' = air")?;
    writeln!(w, "Chunk boundaries marked with | and -")?;
    writeln!(w, "")?;

    // Export terrain map for the 3x3 chunks at surface level
    // Show every 4th tile to keep it readable (12x12 per chunk = 36x36 total)
    let step = 4;
    let tiles_per_chunk = LOCAL_SIZE / step;

    // Header with chunk indices
    write!(w, "     ")?;
    for cx in 0..3 {
        for tx in 0..tiles_per_chunk {
            if tx == 0 {
                write!(w, "|")?;
            }
            write!(w, "{}", tx % 10)?;
        }
    }
    writeln!(w, "|")?;

    // Separator
    write!(w, "     ")?;
    for _ in 0..3 {
        write!(w, "+")?;
        for _ in 0..tiles_per_chunk {
            write!(w, "-")?;
        }
    }
    writeln!(w, "+")?;

    for cy in 0..3 {
        for ty in 0..tiles_per_chunk {
            let local_y = ty * step;

            // Row label
            if ty == 0 {
                write!(w, "c{}y{:02}", cy, local_y)?;
            } else {
                write!(w, "  y{:02}", local_y)?;
            }

            for cx in 0..3 {
                write!(w, "|")?;

                if let Some(chunk) = &chunks[cx][cy] {
                    for tx in 0..tiles_per_chunk {
                        let local_x = tx * step;
                        let tile = chunk.get(local_x, local_y, surface_z);
                        let ch = terrain_to_char(tile.terrain, tile.feature);
                        write!(w, "{}", ch)?;
                    }
                } else {
                    for _ in 0..tiles_per_chunk {
                        write!(w, "?")?;
                    }
                }
            }
            writeln!(w, "|")?;
        }

        // Chunk separator
        write!(w, "     ")?;
        for _ in 0..3 {
            write!(w, "+")?;
            for _ in 0..tiles_per_chunk {
                write!(w, "-")?;
            }
        }
        writeln!(w, "+")?;
    }

    // Also show z-1 and z+1 levels
    writeln!(w, "")?;
    writeln!(w, "=== TERRAIN MAP (z={}, one BELOW surface) ===", surface_z - 1)?;
    render_terrain_map_at_z(&mut w, &chunks, surface_z - 1, step, tiles_per_chunk)?;

    writeln!(w, "")?;
    writeln!(w, "=== TERRAIN MAP (z={}, one ABOVE surface) ===", surface_z + 1)?;
    render_terrain_map_at_z(&mut w, &chunks, surface_z + 1, step, tiles_per_chunk)?;

    writeln!(w, "")?;
    writeln!(w, "=== WATER FACTOR MAP (center chunk only) ===")?;
    writeln!(w, "Values: 0=land, 9=water, shown for every 4th tile")?;
    writeln!(w, "")?;

    // Calculate water factors for center chunk
    let coastline_noise = Perlin::new((world.seed + 2) as u32);
    let corner_biomes = get_corner_biomes(world, center_wx, center_wy);

    write!(w, "    ")?;
    for x in (0..LOCAL_SIZE).step_by(step) {
        write!(w, "{:2}", x % 100)?;
    }
    writeln!(w, "")?;

    for y in (0..LOCAL_SIZE).step_by(step) {
        write!(w, "{:3} ", y)?;
        for x in (0..LOCAL_SIZE).step_by(step) {
            let wf = calculate_noise_water_factor(
                &corner_biomes, center_wx, center_wy, x, y, LOCAL_SIZE, &coastline_noise
            );
            let digit = (wf * 9.0).round() as u8;
            write!(w, "{:2}", digit)?;
        }
        writeln!(w, "")?;
    }

    writeln!(w, "")?;
    writeln!(w, "=== DETAILED TILE INFO (corners and edges of center chunk) ===")?;
    writeln!(w, "Note: Checking ACTUAL surface (highest non-air z) at each position")?;

    // Show detailed info for corners and edges
    let positions = [
        (0, 0, "NW corner"),
        (LOCAL_SIZE/2, 0, "N edge center"),
        (LOCAL_SIZE-1, 0, "NE corner"),
        (0, LOCAL_SIZE/2, "W edge center"),
        (LOCAL_SIZE/2, LOCAL_SIZE/2, "Center"),
        (LOCAL_SIZE-1, LOCAL_SIZE/2, "E edge center"),
        (0, LOCAL_SIZE-1, "SW corner"),
        (LOCAL_SIZE/2, LOCAL_SIZE-1, "S edge center"),
        (LOCAL_SIZE-1, LOCAL_SIZE-1, "SE corner"),
    ];

    for (x, y, label) in positions {
        // Find actual surface z at this position (highest non-air tile)
        let mut actual_surface_z = center_chunk.z_min;
        for z in (center_chunk.z_min..=center_chunk.z_max).rev() {
            let t = center_chunk.get(x, y, z);
            if t.terrain != LocalTerrain::Air {
                actual_surface_z = z;
                break;
            }
        }

        let tile = center_chunk.get(x, y, actual_surface_z);
        let tile_at_fixed_z = center_chunk.get(x, y, surface_z);
        let wf = calculate_noise_water_factor(
            &corner_biomes, center_wx, center_wy, x, y, LOCAL_SIZE, &coastline_noise
        );

        writeln!(w, "")?;
        writeln!(w, "{} ({}, {}):", label, x, y)?;
        writeln!(w, "  Actual surface z: {} (chunk surface_z={})", actual_surface_z, surface_z)?;
        writeln!(w, "  Terrain at actual surface: {:?}", tile.terrain)?;
        writeln!(w, "  Terrain at fixed z={}: {:?}", surface_z, tile_at_fixed_z.terrain)?;
        writeln!(w, "  Feature: {:?}", tile.feature)?;
        writeln!(w, "  Water factor: {:.3}", wf)?;
        writeln!(w, "  Is water terrain (at actual surface): {}", tile.terrain.is_water())?;
        writeln!(w, "  MISMATCH: {}",
            if wf > 0.5 && !tile.terrain.is_water() { "YES - water_factor>0.5 but terrain not water!" }
            else if wf < 0.3 && tile.terrain.is_water() { "YES - water_factor<0.3 but terrain is water!" }
            else { "no" }
        )?;
    }

    writeln!(w, "")?;
    writeln!(w, "=== CHUNK BOUNDARY COMPARISON ===")?;
    writeln!(w, "Comparing edges between adjacent chunks")?;

    // Compare center chunk's east edge with east chunk's west edge
    if let (Some(center), Some(east)) = (&chunks[1][1], &chunks[2][1]) {
        writeln!(w, "")?;
        writeln!(w, "Center chunk EAST edge vs East chunk WEST edge:")?;
        writeln!(w, "Y    Center(x=47)  East(x=0)    Match?")?;

        let mut mismatches = 0;
        for y in (0..LOCAL_SIZE).step_by(step) {
            let center_tile = center.get(LOCAL_SIZE-1, y, surface_z);
            let east_tile = east.get(0, y, surface_z);

            let center_ch = terrain_to_char(center_tile.terrain, center_tile.feature);
            let east_ch = terrain_to_char(east_tile.terrain, east_tile.feature);

            let matches = terrain_matches(center_tile.terrain, east_tile.terrain);
            if !matches {
                mismatches += 1;
            }

            writeln!(w, "{:3}  {}            {}            {}",
                y, center_ch, east_ch, if matches { "OK" } else { "MISMATCH" })?;
        }
        writeln!(w, "Total mismatches: {}/{}", mismatches, LOCAL_SIZE / step)?;
    }

    // Compare center chunk's south edge with south chunk's north edge
    if let (Some(center), Some(south)) = (&chunks[1][1], &chunks[1][2]) {
        writeln!(w, "")?;
        writeln!(w, "Center chunk SOUTH edge vs South chunk NORTH edge:")?;
        writeln!(w, "X    Center(y=47)  South(y=0)   Match?")?;

        let mut mismatches = 0;
        for x in (0..LOCAL_SIZE).step_by(step) {
            let center_tile = center.get(x, LOCAL_SIZE-1, surface_z);
            let south_tile = south.get(x, 0, surface_z);

            let center_ch = terrain_to_char(center_tile.terrain, center_tile.feature);
            let south_ch = terrain_to_char(south_tile.terrain, south_tile.feature);

            let matches = terrain_matches(center_tile.terrain, south_tile.terrain);
            if !matches {
                mismatches += 1;
            }

            writeln!(w, "{:3}  {}            {}            {}",
                x, center_ch, south_ch, if matches { "OK" } else { "MISMATCH" })?;
        }
        writeln!(w, "Total mismatches: {}/{}", mismatches, LOCAL_SIZE / step)?;
    }

    writeln!(w, "")?;
    writeln!(w, "=== END MAIN LOCATION ===")?;

    // Generate a random walk to another 3x3 area
    writeln!(w, "")?;
    writeln!(w, "============================================================")?;
    writeln!(w, "=== RANDOM WALK TO SECOND LOCATION ===")?;
    writeln!(w, "============================================================")?;

    let mut rng = ChaCha8Rng::seed_from_u64(world.seed + center_wx as u64 + center_wy as u64);

    // Pick a random offset (5-15 tiles away in each direction)
    let offset_x: i32 = rng.gen_range(-15..=15);
    let offset_y: i32 = rng.gen_range(-15..=15);

    let walk_wx = ((center_wx as i32 + offset_x).max(1) as usize).min(world.heightmap.width - 2);
    let walk_wy = ((center_wy as i32 + offset_y).max(1) as usize).min(world.heightmap.height - 2);

    writeln!(w, "")?;
    writeln!(w, "Walking from ({}, {}) to ({}, {})", center_wx, center_wy, walk_wx, walk_wy)?;
    writeln!(w, "Offset: ({}, {})", offset_x, offset_y)?;

    // Show the path (world tiles along the way)
    writeln!(w, "")?;
    writeln!(w, "=== PATH WORLD TILES ===")?;

    let steps = offset_x.abs().max(offset_y.abs()) as usize;
    if steps > 0 {
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let path_x = (center_wx as f32 + offset_x as f32 * t).round() as usize;
            let path_y = (center_wy as f32 + offset_y as f32 * t).round() as usize;

            if path_x < world.heightmap.width && path_y < world.heightmap.height {
                let biome = *world.biomes.get(path_x, path_y);
                let height = *world.heightmap.get(path_x, path_y);
                let water_id = *world.water_body_map.get(path_x, path_y);

                writeln!(w, "  Step {:2}: ({:3}, {:3}) - {:?}, h={:.0}m, water_id={:?}",
                    i, path_x, path_y, biome, height, water_id)?;
            }
        }
    }

    // Generate and show the second location's terrain map
    writeln!(w, "")?;
    writeln!(w, "=== SECOND LOCATION ({}, {}) WORLD TILE INFO ===", walk_wx, walk_wy)?;

    let walk_biome = *world.biomes.get(walk_wx, walk_wy);
    let walk_height = *world.heightmap.get(walk_wx, walk_wy);
    let walk_water_id = *world.water_body_map.get(walk_wx, walk_wy);
    let walk_corner_biomes = get_corner_biomes(world, walk_wx, walk_wy);

    writeln!(w, "  Biome: {:?}", walk_biome)?;
    writeln!(w, "  Height: {:.1}m", walk_height)?;
    writeln!(w, "  Water body ID: {:?}", walk_water_id)?;
    writeln!(w, "  Corner biomes:")?;
    writeln!(w, "    NW: {:?} (water={})", walk_corner_biomes[0][0], is_water_biome(walk_corner_biomes[0][0]))?;
    writeln!(w, "    NE: {:?} (water={})", walk_corner_biomes[0][1], is_water_biome(walk_corner_biomes[0][1]))?;
    writeln!(w, "    SW: {:?} (water={})", walk_corner_biomes[1][0], is_water_biome(walk_corner_biomes[1][0]))?;
    writeln!(w, "    SE: {:?} (water={})", walk_corner_biomes[1][1], is_water_biome(walk_corner_biomes[1][1]))?;

    let walk_water_count = walk_corner_biomes.iter().flatten()
        .filter(|&&b| is_water_biome(b)).count();
    writeln!(w, "  Water corner count: {}/4", walk_water_count)?;

    // Generate the walk chunk
    let walk_chunk = generate_local_chunk(world, walk_wx, walk_wy);
    let walk_surface_z = walk_chunk.surface_z;

    writeln!(w, "")?;
    writeln!(w, "=== SECOND LOCATION TERRAIN MAP (surface z={}) ===", walk_surface_z)?;

    // Header
    write!(w, "    ")?;
    for x in (0..LOCAL_SIZE).step_by(step) {
        write!(w, "{}", (x / step) % 10)?;
    }
    writeln!(w, "")?;

    // Terrain rows
    for y in (0..LOCAL_SIZE).step_by(step) {
        write!(w, "{:3} ", y)?;
        for x in (0..LOCAL_SIZE).step_by(step) {
            let tile = walk_chunk.get(x, y, walk_surface_z);
            let ch = terrain_to_char(tile.terrain, tile.feature);
            write!(w, "{}", ch)?;
        }
        writeln!(w, "")?;
    }

    // Water factor map for second location
    writeln!(w, "")?;
    writeln!(w, "=== SECOND LOCATION WATER FACTOR MAP ===")?;

    write!(w, "    ")?;
    for x in (0..LOCAL_SIZE).step_by(step) {
        write!(w, "{}", (x / step) % 10)?;
    }
    writeln!(w, "")?;

    for y in (0..LOCAL_SIZE).step_by(step) {
        write!(w, "{:3} ", y)?;
        for x in (0..LOCAL_SIZE).step_by(step) {
            let wf = calculate_noise_water_factor(
                &walk_corner_biomes, walk_wx, walk_wy, x, y, LOCAL_SIZE, &coastline_noise
            );
            let digit = (wf * 9.0).round() as u8;
            write!(w, "{}", digit)?;
        }
        writeln!(w, "")?;
    }

    writeln!(w, "")?;
    writeln!(w, "=== END DEBUG EXPORT ===")?;

    Ok(())
}

fn terrain_to_char(terrain: LocalTerrain, feature: LocalFeature) -> char {
    // Check features first
    match feature {
        LocalFeature::Tree { .. } => return 'T',
        LocalFeature::Bush => return '*',
        LocalFeature::RampUp => return '^',
        LocalFeature::RampDown => return 'v',
        LocalFeature::StairsUp => return '<',
        LocalFeature::StairsDown => return '>',
        _ => {}
    }

    match terrain {
        LocalTerrain::Air => ' ',
        LocalTerrain::Grass => ',',
        LocalTerrain::Sand => '.',
        LocalTerrain::ShallowWater => '~',
        LocalTerrain::DeepWater => '≈',
        LocalTerrain::FlowingWater => '~',
        LocalTerrain::StoneFloor | LocalTerrain::DirtFloor | LocalTerrain::CaveFloor => '.',
        LocalTerrain::StoneWall | LocalTerrain::BrickWall | LocalTerrain::WoodWall | LocalTerrain::CaveWall => '#',
        LocalTerrain::Stone { .. } | LocalTerrain::Soil { .. } => '#',
        LocalTerrain::Mud => '~',
        LocalTerrain::Ice | LocalTerrain::Snow => '*',
        LocalTerrain::Lava | LocalTerrain::Magma => '!',
        _ => '?',
    }
}

fn render_terrain_map_at_z<W: Write>(
    w: &mut W,
    chunks: &[[Option<LocalChunk>; 3]; 3],
    z: i16,
    step: usize,
    tiles_per_chunk: usize,
) -> std::io::Result<()> {
    // Header with chunk indices
    write!(w, "     ")?;
    for _cx in 0..3 {
        for tx in 0..tiles_per_chunk {
            if tx == 0 {
                write!(w, "|")?;
            }
            write!(w, "{}", tx % 10)?;
        }
    }
    writeln!(w, "|")?;

    // Separator
    write!(w, "     ")?;
    for _ in 0..3 {
        write!(w, "+")?;
        for _ in 0..tiles_per_chunk {
            write!(w, "-")?;
        }
    }
    writeln!(w, "+")?;

    for cy in 0..3 {
        for ty in 0..tiles_per_chunk {
            let local_y = ty * step;

            // Row label
            if ty == 0 {
                write!(w, "c{}y{:02}", cy, local_y)?;
            } else {
                write!(w, "  y{:02}", local_y)?;
            }

            for cx in 0..3 {
                write!(w, "|")?;

                if let Some(chunk) = &chunks[cx][cy] {
                    for tx in 0..tiles_per_chunk {
                        let local_x = tx * step;
                        let tile = chunk.get(local_x, local_y, z);
                        let ch = terrain_to_char(tile.terrain, tile.feature);
                        write!(w, "{}", ch)?;
                    }
                } else {
                    for _ in 0..tiles_per_chunk {
                        write!(w, "?")?;
                    }
                }
            }
            writeln!(w, "|")?;
        }

        // Chunk separator
        write!(w, "     ")?;
        for _ in 0..3 {
            write!(w, "+")?;
            for _ in 0..tiles_per_chunk {
                write!(w, "-")?;
            }
        }
        writeln!(w, "+")?;
    }
    Ok(())
}

fn terrain_matches(a: LocalTerrain, b: LocalTerrain) -> bool {
    // Consider water types as matching each other
    let a_water = matches!(a, LocalTerrain::ShallowWater | LocalTerrain::DeepWater | LocalTerrain::FlowingWater);
    let b_water = matches!(b, LocalTerrain::ShallowWater | LocalTerrain::DeepWater | LocalTerrain::FlowingWater);

    if a_water && b_water {
        return true;
    }

    // Consider land types as matching each other
    let a_land = matches!(a, LocalTerrain::Grass | LocalTerrain::Sand | LocalTerrain::DirtFloor | LocalTerrain::StoneFloor);
    let b_land = matches!(b, LocalTerrain::Grass | LocalTerrain::Sand | LocalTerrain::DirtFloor | LocalTerrain::StoneFloor);

    if a_land && b_land {
        return true;
    }

    // Water vs land is a mismatch
    if (a_water && b_land) || (a_land && b_water) {
        return false;
    }

    // For other terrain, compare directly
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}
