//! Natural terrain generation at local scale.
//!
//! Provides additional terrain features beyond the basic geology-driven generation.
//! Used for adding biome-specific details and special features.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;

use super::local::{LocalChunk, LocalTile, LocalTerrain, LocalFeature, Material};
use super::LOCAL_SIZE;
use super::geology::GeologyParams;

/// Add biome-specific terrain features to a local chunk
pub fn add_biome_terrain_features(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    rng: &mut ChaCha8Rng,
) {
    match geology.biome {
        // Volcanic areas - add lava patches
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaLake => {
            add_lava_patches(chunk, geology.surface_z, rng);
        }

        // Oasis - add water and vegetation
        ExtendedBiome::Oasis => {
            add_oasis(chunk, geology.surface_z, rng);
        }

        // Haunted areas - add eerie features
        ExtendedBiome::DeadForest | ExtendedBiome::Shadowfen => {
            add_haunted_features(chunk, geology.surface_z, rng);
        }

        // Crystal biomes - add crystal formations
        ExtendedBiome::CrystalWasteland | ExtendedBiome::CrystalForest => {
            add_crystal_formations(chunk, geology.surface_z, rng);
        }

        _ => {}
    }
}

/// Add lava patches to volcanic terrain
fn add_lava_patches(chunk: &mut LocalChunk, surface_z: i16, rng: &mut ChaCha8Rng) {
    let num_patches = rng.gen_range(1..4);
    let noise = Perlin::new(rng.gen());

    for _ in 0..num_patches {
        let cx = rng.gen_range(5..LOCAL_SIZE - 5);
        let cy = rng.gen_range(5..LOCAL_SIZE - 5);
        let radius = rng.gen_range(3..8);

        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                let x = (cx as i32 + dx).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                let y = (cy as i32 + dy).clamp(0, LOCAL_SIZE as i32 - 1) as usize;

                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                let noise_val = noise.get([dx as f64 * 0.5, dy as f64 * 0.5]);

                if dist + noise_val as f32 * 2.0 < radius as f32 {
                    chunk.set(x, y, surface_z, LocalTile::new(LocalTerrain::Magma, Material::Magma));
                }
            }
        }
    }
}

/// Add oasis features (pool and palms)
fn add_oasis(chunk: &mut LocalChunk, surface_z: i16, rng: &mut ChaCha8Rng) {
    let cx = LOCAL_SIZE / 2;
    let cy = LOCAL_SIZE / 2;
    let radius = rng.gen_range(8..15);

    // Water pool in center
    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            let x = (cx as i32 + dx).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
            let y = (cy as i32 + dy).clamp(0, LOCAL_SIZE as i32 - 1) as usize;

            let dist = ((dx * dx + dy * dy) as f32).sqrt();

            if dist < (radius / 2) as f32 {
                chunk.set(x, y, surface_z, LocalTile::new(LocalTerrain::ShallowWater, Material::Water));
            } else if dist < radius as f32 {
                let mut tile = LocalTile::new(LocalTerrain::Grass, Material::Grass);
                if rng.gen_bool(0.1) {
                    tile.feature = LocalFeature::Tree { height: rng.gen_range(4..7) }; // Palm tree
                }
                chunk.set(x, y, surface_z, tile);
            }
        }
    }
}

/// Add haunted/eerie features
fn add_haunted_features(chunk: &mut LocalChunk, surface_z: i16, rng: &mut ChaCha8Rng) {
    // Add gravestones (using Pillar as placeholder)
    for _ in 0..rng.gen_range(3..8) {
        let x = rng.gen_range(2..LOCAL_SIZE - 2);
        let y = rng.gen_range(2..LOCAL_SIZE - 2);

        if chunk.get(x, y, surface_z).terrain.is_passable() && chunk.get(x, y, surface_z).feature == LocalFeature::None {
            chunk.get_mut(x, y, surface_z).feature = LocalFeature::Pillar; // Gravestone
        }
    }

    // Add dead trees
    for _ in 0..rng.gen_range(2..5) {
        let x = rng.gen_range(2..LOCAL_SIZE - 2);
        let y = rng.gen_range(2..LOCAL_SIZE - 2);

        if chunk.get(x, y, surface_z).terrain.is_passable() && chunk.get(x, y, surface_z).feature == LocalFeature::None {
            chunk.get_mut(x, y, surface_z).feature = LocalFeature::Tree { height: rng.gen_range(2..4) }; // Dead tree
        }
    }
}

/// Add crystal formations
fn add_crystal_formations(chunk: &mut LocalChunk, surface_z: i16, rng: &mut ChaCha8Rng) {
    let noise = Perlin::new(rng.gen());

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            let nx = x as f64 / LOCAL_SIZE as f64 * 4.0;
            let ny = y as f64 / LOCAL_SIZE as f64 * 4.0;
            let noise_val = noise.get([nx, ny]);

            if noise_val > 0.7 && chunk.get(x, y, surface_z).feature == LocalFeature::None {
                if rng.gen_bool(0.3) {
                    chunk.get_mut(x, y, surface_z).feature = LocalFeature::Crystal;
                }
            }
        }
    }
}

/// Generate a cave room at a specific z-level
pub fn generate_cave_room(
    chunk: &mut LocalChunk,
    z_level: i16,
    rng: &mut ChaCha8Rng,
) {
    let seed = rng.gen::<u32>();
    let noise = Perlin::new(seed);
    let detail = Perlin::new(seed.wrapping_add(1));

    // Use cellular automata-style generation
    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            let nx = x as f64 / LOCAL_SIZE as f64 * 4.0;
            let ny = y as f64 / LOCAL_SIZE as f64 * 4.0;
            let noise_val = noise.get([nx, ny]);
            let detail_val = detail.get([nx * 2.0, ny * 2.0]) * 0.3;

            // Edge walls
            let edge_dist = (x.min(LOCAL_SIZE - 1 - x).min(y).min(LOCAL_SIZE - 1 - y)) as f64;
            let edge_factor = (edge_dist / 5.0).min(1.0);

            let terrain = if edge_factor < 0.5 || noise_val + detail_val < -0.2 {
                LocalTerrain::Stone { stone_type: super::local::StoneType::Granite }
            } else {
                LocalTerrain::CaveFloor
            };

            chunk.set(x, y, z_level, LocalTile::new(terrain, Material::Stone));
        }
    }

    // Add cave features
    for y in 2..LOCAL_SIZE - 2 {
        for x in 2..LOCAL_SIZE - 2 {
            if chunk.get(x, y, z_level).terrain == LocalTerrain::CaveFloor {
                let nx = x as f64 * 0.2;
                let ny = y as f64 * 0.2;
                let feature_noise = noise.get([nx, ny]);

                if feature_noise > 0.6 && rng.gen_bool(0.1) {
                    chunk.get_mut(x, y, z_level).feature = LocalFeature::Stalagmite;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_cave_room_generation() {
        let mut chunk = super::super::local::LocalChunk::new(0, 0, 0);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        generate_cave_room(&mut chunk, -5, &mut rng);

        // Check that we have both floor and walls
        let mut has_floor = false;
        let mut has_wall = false;
        for y in 0..LOCAL_SIZE {
            for x in 0..LOCAL_SIZE {
                match chunk.get(x, y, -5).terrain {
                    LocalTerrain::CaveFloor => has_floor = true,
                    LocalTerrain::Stone { .. } => has_wall = true,
                    _ => {}
                }
            }
        }
        assert!(has_floor);
        assert!(has_wall);
    }
}
