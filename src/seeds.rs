//! Seed management for world generation
//!
//! Provides separate seeds for each generation system, allowing fine-grained control
//! over which aspects of world generation to vary or keep constant.

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// Seeds for all world generation systems.
///
/// Each system gets its own seed, derived from a master seed by default.
/// Individual seeds can be overridden for experimentation.
#[derive(Clone, Debug)]
pub struct WorldSeeds {
    /// Master seed (used for display/reference)
    pub master: u64,
    /// Tectonic plate generation (plate count, positions, types, velocities)
    pub tectonics: u64,
    /// Heightmap generation (base terrain shape, mountains, valleys)
    pub heightmap: u64,
    /// Erosion simulation (hydraulic erosion, sediment transport)
    pub erosion: u64,
    /// Climate patterns (temperature distribution, moisture flow)
    pub climate: u64,
    /// Biome generation and special biome placement
    pub biomes: u64,
    /// Coastline jittering and shoreline detail
    pub coastline: u64,
    /// River network generation (paths, confluences, widths)
    pub rivers: u64,
    /// Rock materials and hardness distribution
    pub materials: u64,
}

impl WorldSeeds {
    /// Create seeds from a master seed, deriving all sub-seeds deterministically.
    pub fn from_master(master: u64) -> Self {
        Self {
            master,
            tectonics: derive_seed(master, "tectonics"),
            heightmap: derive_seed(master, "heightmap"),
            erosion: derive_seed(master, "erosion"),
            climate: derive_seed(master, "climate"),
            biomes: derive_seed(master, "biomes"),
            coastline: derive_seed(master, "coastline"),
            rivers: derive_seed(master, "rivers"),
            materials: derive_seed(master, "materials"),
        }
    }

    /// Create with explicit seeds for each system.
    pub fn explicit(
        tectonics: u64,
        heightmap: u64,
        erosion: u64,
        climate: u64,
        biomes: u64,
        coastline: u64,
        rivers: u64,
        materials: u64,
    ) -> Self {
        // Use tectonics as the "master" for display purposes
        Self {
            master: tectonics,
            tectonics,
            heightmap,
            erosion,
            climate,
            biomes,
            coastline,
            rivers,
            materials,
        }
    }

    /// Create a builder for customizing individual seeds
    pub fn builder(master: u64) -> WorldSeedsBuilder {
        WorldSeedsBuilder::new(master)
    }
}

impl Default for WorldSeeds {
    fn default() -> Self {
        Self::from_master(rand::random())
    }
}

/// Builder for customizing individual seeds while deriving others from master
pub struct WorldSeedsBuilder {
    seeds: WorldSeeds,
}

impl WorldSeedsBuilder {
    pub fn new(master: u64) -> Self {
        Self {
            seeds: WorldSeeds::from_master(master),
        }
    }

    /// Override the tectonics seed
    pub fn tectonics(mut self, seed: u64) -> Self {
        self.seeds.tectonics = seed;
        self
    }

    /// Override the heightmap seed
    pub fn heightmap(mut self, seed: u64) -> Self {
        self.seeds.heightmap = seed;
        self
    }

    /// Override the erosion seed
    pub fn erosion(mut self, seed: u64) -> Self {
        self.seeds.erosion = seed;
        self
    }

    /// Override the climate seed
    pub fn climate(mut self, seed: u64) -> Self {
        self.seeds.climate = seed;
        self
    }

    /// Override the biomes seed
    pub fn biomes(mut self, seed: u64) -> Self {
        self.seeds.biomes = seed;
        self
    }

    /// Override the coastline seed
    pub fn coastline(mut self, seed: u64) -> Self {
        self.seeds.coastline = seed;
        self
    }

    /// Override the rivers seed
    pub fn rivers(mut self, seed: u64) -> Self {
        self.seeds.rivers = seed;
        self
    }

    /// Override the materials seed
    pub fn materials(mut self, seed: u64) -> Self {
        self.seeds.materials = seed;
        self
    }

    /// Build the final WorldSeeds
    pub fn build(self) -> WorldSeeds {
        self.seeds
    }
}

/// Derive a sub-seed from a master seed and a system name.
/// Uses hashing to ensure different systems get different but deterministic seeds.
fn derive_seed(master: u64, system: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    master.hash(&mut hasher);
    system.hash(&mut hasher);
    hasher.finish()
}

/// Display format for seeds (useful for sharing world configurations)
impl std::fmt::Display for WorldSeeds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WorldSeeds {{ master: {}, tectonics: {}, heightmap: {}, erosion: {}, \
             climate: {}, biomes: {}, coastline: {}, rivers: {}, materials: {} }}",
            self.master,
            self.tectonics,
            self.heightmap,
            self.erosion,
            self.climate,
            self.biomes,
            self.coastline,
            self.rivers,
            self.materials,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_derivation() {
        let seeds1 = WorldSeeds::from_master(12345);
        let seeds2 = WorldSeeds::from_master(12345);

        assert_eq!(seeds1.tectonics, seeds2.tectonics);
        assert_eq!(seeds1.heightmap, seeds2.heightmap);
        assert_eq!(seeds1.erosion, seeds2.erosion);
    }

    #[test]
    fn test_different_systems_get_different_seeds() {
        let seeds = WorldSeeds::from_master(12345);

        // Each system should get a unique seed
        assert_ne!(seeds.tectonics, seeds.heightmap);
        assert_ne!(seeds.heightmap, seeds.erosion);
        assert_ne!(seeds.erosion, seeds.climate);
    }

    #[test]
    fn test_builder_override() {
        let seeds = WorldSeeds::builder(12345)
            .erosion(99999)
            .build();

        // Erosion should be overridden
        assert_eq!(seeds.erosion, 99999);

        // Others should be derived from master
        let default_seeds = WorldSeeds::from_master(12345);
        assert_eq!(seeds.tectonics, default_seeds.tectonics);
        assert_eq!(seeds.heightmap, default_seeds.heightmap);
    }
}
