//! Perimeter-Based Biome Feathering (Phase 2a)
//!
//! Replaces hard-edge biome transitions with smooth Gaussian-distributed borders
//! using weighted spline interpolation for natural-looking boundaries.

use std::collections::VecDeque;
use noise::{NoiseFn, Perlin, Seedable};

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Configuration for biome feathering
#[derive(Clone, Debug)]
pub struct FeatherConfig {
    /// Gaussian sigma for border depth variance (0.5-2.0)
    pub gaussian_sigma: f32,
    /// Minimum border width in tiles
    pub min_depth: usize,
    /// Maximum border width in tiles
    pub max_depth: usize,
    /// Catmull-Rom spline weights for interpolation
    pub spline_weights: [f32; 4],
    /// Noise frequency for border perturbation
    pub noise_frequency: f64,
    /// Noise amplitude for border perturbation
    pub noise_amplitude: f32,
}

impl Default for FeatherConfig {
    fn default() -> Self {
        Self {
            gaussian_sigma: 1.0,
            min_depth: 2,
            max_depth: 8,
            spline_weights: [0.5, 0.5, 0.5, 0.5], // Catmull-Rom defaults
            noise_frequency: 0.1,
            noise_amplitude: 0.3,
        }
    }
}

// =============================================================================
// FEATHER MAP
// =============================================================================

/// Precomputed feathering data for efficient runtime lookup
#[derive(Clone)]
pub struct BiomeFeatherMap {
    /// Distance to nearest biome boundary (0 at edge, positive inland)
    pub depth_map: Tilemap<f32>,
    /// Gradient direction pointing toward nearest boundary (normalized)
    pub gradient_map: Tilemap<(f32, f32)>,
    /// Per-tile blend weights for each neighboring biome
    /// Format: Vec of (biome, weight) pairs sorted by weight descending
    pub blend_weights: Tilemap<Vec<(ExtendedBiome, f32)>>,
    /// The configuration used to generate this map
    pub config: FeatherConfig,
}

impl BiomeFeatherMap {
    /// Get the normalized depth at a position (0.0 = edge, 1.0 = center)
    pub fn get_normalized_depth(&self, x: usize, y: usize) -> f32 {
        let depth = *self.depth_map.get(x, y);
        // Normalize to 0-1 range based on config
        (depth / self.config.max_depth as f32).clamp(0.0, 1.0)
    }

    /// Get blend factor for edge blending (1.0 at center, 0.0 at edge)
    pub fn get_edge_blend_factor(&self, x: usize, y: usize) -> f32 {
        let norm_depth = self.get_normalized_depth(x, y);
        // Smooth step for nicer blending
        smooth_step(0.0, 1.0, norm_depth)
    }

    /// Get the primary biome and its weight at a position
    pub fn get_primary_biome(&self, x: usize, y: usize) -> Option<(ExtendedBiome, f32)> {
        let weights = self.blend_weights.get(x, y);
        weights.first().cloned()
    }

    /// Get all biomes and their weights at a position
    pub fn get_biome_weights(&self, x: usize, y: usize) -> &[(ExtendedBiome, f32)] {
        self.blend_weights.get(x, y)
    }

    /// Check if this tile is in a transition zone
    pub fn is_transition_zone(&self, x: usize, y: usize) -> bool {
        let weights = self.blend_weights.get(x, y);
        weights.len() > 1 && weights.get(1).map(|(_, w)| *w > 0.1).unwrap_or(false)
    }
}

// =============================================================================
// FEATHERING COMPUTATION
// =============================================================================

/// Compute the biome feather map from a biome tilemap
pub fn compute_biome_feathering(
    biomes: &Tilemap<ExtendedBiome>,
    config: &FeatherConfig,
    seed: u64,
) -> BiomeFeatherMap {
    let width = biomes.width;
    let height = biomes.height;
    let noise = Perlin::new(1).set_seed(seed as u32);

    // Step 1: Compute distance field from biome boundaries
    let (depth_map, gradient_map) = compute_distance_field(biomes, config, &noise);

    // Step 2: Compute blend weights using Catmull-Rom interpolation
    let blend_weights = compute_blend_weights(biomes, &depth_map, config, &noise);

    BiomeFeatherMap {
        depth_map,
        gradient_map,
        blend_weights,
        config: config.clone(),
    }
}

/// Compute signed distance field from biome boundaries
fn compute_distance_field(
    biomes: &Tilemap<ExtendedBiome>,
    config: &FeatherConfig,
    noise: &Perlin,
) -> (Tilemap<f32>, Tilemap<(f32, f32)>) {
    let width = biomes.width;
    let height = biomes.height;

    // Initialize with max distance
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut gradient = Tilemap::new_with(width, height, (0.0f32, 0.0f32));

    // Find all boundary cells (cells adjacent to different biome)
    let mut boundary_queue: VecDeque<(usize, usize, f32)> = VecDeque::new();

    for y in 0..height {
        for x in 0..width {
            let biome = *biomes.get(x, y);

            // Check if any neighbor has a different biome
            let is_boundary = biomes.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                *biomes.get(nx, ny) != biome
            });

            if is_boundary {
                // Add Gaussian jitter to boundary depth
                let jitter = gaussian_jitter(x, y, noise, config.gaussian_sigma, config.noise_frequency);
                let depth = (jitter * config.noise_amplitude).max(0.0);

                distance.set(x, y, depth);
                boundary_queue.push_back((x, y, depth));
            }
        }
    }

    // BFS to propagate distance from boundaries
    while let Some((x, y, dist)) = boundary_queue.pop_front() {
        for (nx, ny) in biomes.neighbors_8(x, y) {
            let current_dist = *distance.get(nx, ny);
            let new_dist = dist + 1.0;

            if new_dist < current_dist && new_dist < config.max_depth as f32 {
                distance.set(nx, ny, new_dist);

                // Compute gradient (direction toward boundary)
                let gx = x as f32 - nx as f32;
                let gy = y as f32 - ny as f32;
                let len = (gx * gx + gy * gy).sqrt();
                if len > 0.0 {
                    gradient.set(nx, ny, (gx / len, gy / len));
                }

                boundary_queue.push_back((nx, ny, new_dist));
            }
        }
    }

    // Normalize distances: cells at max distance or beyond get max depth
    for y in 0..height {
        for x in 0..width {
            let d = *distance.get(x, y);
            if d >= f32::MAX - 1.0 {
                distance.set(x, y, config.max_depth as f32);
            }
        }
    }

    (distance, gradient)
}

/// Compute blend weights for each tile based on neighboring biomes.
/// Optimized to use O(n×8) instead of O(n×r²) by leveraging the distance field
/// and only checking immediate neighbors.
fn compute_blend_weights(
    biomes: &Tilemap<ExtendedBiome>,
    depth_map: &Tilemap<f32>,
    config: &FeatherConfig,
    _noise: &Perlin,
) -> Tilemap<Vec<(ExtendedBiome, f32)>> {
    let width = biomes.width;
    let height = biomes.height;

    let mut weights_map: Tilemap<Vec<(ExtendedBiome, f32)>> = Tilemap::new_with(width, height, Vec::new());

    for y in 0..height {
        for x in 0..width {
            let depth = *depth_map.get(x, y);
            let center_biome = *biomes.get(x, y);

            // If deep in biome center, just use that biome
            if depth >= config.max_depth as f32 - 1.0 {
                weights_map.set(x, y, vec![(center_biome, 1.0)]);
                continue;
            }

            // Optimized: Use distance field directly for blend weights
            // and only check 8 immediate neighbors instead of r² area
            let mut biome_contributions: std::collections::HashMap<ExtendedBiome, f32> = std::collections::HashMap::new();

            // Center biome weight based on distance from boundary
            let center_weight = catmull_rom_weight(depth, config.max_depth as f32, &config.spline_weights);
            biome_contributions.insert(center_biome, center_weight);

            // Check only 8 immediate neighbors to find adjacent biomes
            // The distance field already tells us how far we are from boundaries
            let neighbors = biomes.neighbors_8(x, y);
            for (nx, ny) in neighbors {
                let neighbor_biome = *biomes.get(nx, ny);
                if neighbor_biome == center_biome {
                    continue;
                }

                // Weight based on our distance to boundary (from depth_map) and compatibility
                // Closer to boundary = more influence from neighbor biome
                let blend_factor = 1.0 - (depth / config.max_depth as f32).clamp(0.0, 1.0);
                let compat = biome_compatibility(center_biome, neighbor_biome);
                let weight = blend_factor * compat * (1.0 - center_weight);

                if weight > 0.01 {
                    // Accumulate weight for each unique neighbor biome
                    *biome_contributions.entry(neighbor_biome).or_insert(0.0) += weight;
                }
            }

            // Normalize weights
            let total: f32 = biome_contributions.values().sum();
            let mut weights: Vec<(ExtendedBiome, f32)> = if total > 0.0 {
                biome_contributions
                    .into_iter()
                    .map(|(b, w)| (b, w / total))
                    .filter(|(_, w)| *w > 0.01)
                    .collect()
            } else {
                vec![(center_biome, 1.0)]
            };

            // Sort by weight descending
            weights.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            weights_map.set(x, y, weights);
        }
    }

    weights_map
}

/// Compute biome compatibility for blending (0.0 = incompatible, 1.0 = very compatible)
fn biome_compatibility(a: ExtendedBiome, b: ExtendedBiome) -> f32 {
    use ExtendedBiome::*;

    // Same biome = perfect compatibility
    if a == b {
        return 1.0;
    }

    // Group biomes by category for compatibility scoring
    let is_water = |b: ExtendedBiome| matches!(b,
        DeepOcean | Ocean | CoastalWater | Lagoon | AcidLake | LavaLake |
        FrozenLake | BioluminescentWater | AbyssalVents | Sargasso
    );

    let is_forest = |b: ExtendedBiome| matches!(b,
        BorealForest | TemperateForest | TemperateRainforest | TropicalForest |
        TropicalRainforest | DeadForest | CrystalForest | BioluminescentForest |
        MushroomForest | PetrifiedForest | AncientGrove
    );

    let is_cold = |b: ExtendedBiome| matches!(b,
        Ice | Tundra | AlpineTundra | SnowyPeaks | FrozenLake | AuroraWastes |
        AlpineMeadow | SubalpineForest
    );

    let is_hot = |b: ExtendedBiome| matches!(b,
        Desert | VolcanicWasteland | LavaLake | Ashlands | SaltFlats |
        SingingDunes | GlassDesert | SulfurVents
    );

    let is_wetland = |b: ExtendedBiome| matches!(b,
        Swamp | Marsh | Bog | MangroveSaltmarsh | SpiritMarsh | Shadowfen
    );

    // Mountain biomes (altitudinal zones) - smooth vertical transitions
    let is_mountain = |b: ExtendedBiome| matches!(b,
        MontaneForest | CloudForest | Paramo | SubalpineForest |
        AlpineMeadow | AlpineTundra | SnowyPeaks | HighlandLake | CraterLake | Foothills
    );

    // Water-to-water transitions are smooth
    if is_water(a) && is_water(b) {
        return 0.9;
    }

    // Forest-to-forest transitions are smooth
    if is_forest(a) && is_forest(b) {
        return 0.85;
    }

    // Mountain-to-mountain transitions (altitudinal bands)
    if is_mountain(a) && is_mountain(b) {
        return 0.9; // Very smooth - these are natural elevation gradients
    }

    // Cold-to-cold transitions
    if is_cold(a) && is_cold(b) {
        return 0.8;
    }

    // Hot-to-hot transitions
    if is_hot(a) && is_hot(b) {
        return 0.75;
    }

    // Wetland transitions
    if is_wetland(a) && is_wetland(b) {
        return 0.85;
    }

    // Water to land is sharp
    if is_water(a) != is_water(b) {
        return 0.2;
    }

    // Hot to cold is sharp
    if is_hot(a) && is_cold(b) || is_cold(a) && is_hot(b) {
        return 0.1;
    }

    // Default moderate compatibility
    0.5
}

/// Catmull-Rom spline weight function
fn catmull_rom_weight(distance: f32, max_distance: f32, weights: &[f32; 4]) -> f32 {
    let t = (distance / max_distance).clamp(0.0, 1.0);
    let t2 = t * t;
    let t3 = t2 * t;

    // Catmull-Rom basis functions
    let w0 = weights[0] * (-0.5 * t3 + t2 - 0.5 * t);
    let w1 = weights[1] * (1.5 * t3 - 2.5 * t2 + 1.0);
    let w2 = weights[2] * (-1.5 * t3 + 2.0 * t2 + 0.5 * t);
    let w3 = weights[3] * (0.5 * t3 - 0.5 * t2);

    // Return blend factor (1.0 at center, decreasing toward edges)
    (w0 + w1 + w2 + w3).clamp(0.0, 1.0)
}

/// Generate Gaussian jitter for border depth variation
fn gaussian_jitter(x: usize, y: usize, noise: &Perlin, sigma: f32, frequency: f64) -> f32 {
    let nx = x as f64 * frequency;
    let ny = y as f64 * frequency;

    // Use multiple noise octaves for Gaussian-like distribution
    let n1 = noise.get([nx, ny, 0.0]) as f32;
    let n2 = noise.get([nx * 2.0, ny * 2.0, 1.0]) as f32 * 0.5;
    let n3 = noise.get([nx * 4.0, ny * 4.0, 2.0]) as f32 * 0.25;

    // Combine and scale by sigma
    (n1 + n2 + n3) * sigma
}

/// Smooth step interpolation (Hermite smoothstep)
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// =============================================================================
// INTEGRATION HELPERS
// =============================================================================

/// Get edge blend factor using the feather map (replacement for old edge_blend_factor)
pub fn get_feathered_blend_factor(
    feather_map: &BiomeFeatherMap,
    x: usize,
    y: usize,
) -> f32 {
    feather_map.get_edge_blend_factor(x, y)
}

/// Blend between biome configs based on feather map weights
pub fn blend_biome_values<T: Clone + Default>(
    feather_map: &BiomeFeatherMap,
    x: usize,
    y: usize,
    get_value: impl Fn(ExtendedBiome) -> T,
    blend: impl Fn(&T, &T, f32) -> T,
) -> T {
    let weights = feather_map.get_biome_weights(x, y);

    if weights.is_empty() {
        return T::default();
    }

    if weights.len() == 1 {
        return get_value(weights[0].0);
    }

    // Blend between top 2 biomes for efficiency
    let primary = get_value(weights[0].0);
    let secondary = get_value(weights[1].0);
    let blend_factor = weights[0].1; // Primary weight

    blend(&secondary, &primary, blend_factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feather_config_default() {
        let config = FeatherConfig::default();
        assert!(config.gaussian_sigma > 0.0);
        assert!(config.max_depth > config.min_depth);
    }

    #[test]
    fn test_biome_compatibility() {
        use ExtendedBiome::*;

        // Same biome = perfect
        assert_eq!(biome_compatibility(TemperateForest, TemperateForest), 1.0);

        // Forest to forest = high
        assert!(biome_compatibility(TemperateForest, TropicalForest) > 0.7);

        // Water to land = low
        assert!(biome_compatibility(Ocean, Desert) < 0.3);
    }

    #[test]
    fn test_smooth_step() {
        assert!((smooth_step(0.0, 1.0, 0.0) - 0.0).abs() < 0.001);
        assert!((smooth_step(0.0, 1.0, 1.0) - 1.0).abs() < 0.001);
        assert!((smooth_step(0.0, 1.0, 0.5) - 0.5).abs() < 0.001);
    }
}
