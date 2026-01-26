use noise::{NoiseFn, Perlin, Seedable};

use crate::plates::{Plate, PlateId, PlateType};
use crate::scale::{MapScale, scale_distance, scale_frequency, scale_elevation};
use crate::tilemap::Tilemap;

/// Normalize a seed value to a small floating point range suitable for noise coordinates.
/// This prevents crashes from using very large u64 seeds directly as 3D noise coordinates.
#[inline]
fn seed_to_z(seed: u64, offset: f64) -> f64 {
    // Hash the seed down to a small range [0, 1000) and add offset
    let hash = ((seed.wrapping_mul(0x517cc1b727220a95)) >> 48) as f64 / 65536.0 * 1000.0;
    hash + offset
}

// =============================================================================
// TERRAIN PARAMETERS
// =============================================================================

/// Parameters for terrain generation
pub struct TerrainParams {
    /// Base frequency for noise (lower = larger features)
    pub base_frequency: f64,
    /// Number of noise octaves
    pub octaves: u32,
    /// Amplitude decay per octave (0.0-1.0)
    pub persistence: f64,
    /// Frequency multiplier per octave
    pub lacunarity: f64,
    /// Domain warping strength
    pub warp_strength: f64,
    /// Ridge noise power (higher = sharper ridges)
    pub ridge_power: f64,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            base_frequency: 0.008,
            octaves: 6,
            persistence: 0.5,
            lacunarity: 2.0,
            warp_strength: 0.15,  // Reduced from 0.4 to reduce swirly appearance
            ridge_power: 2.0,
        }
    }
}

// =============================================================================
// LAYERED NOISE ARCHITECTURE (Phase 3b)
// =============================================================================

/// Blend mode for combining noise layers
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendMode {
    /// Add layer values together
    Add,
    /// Multiply layers together
    Multiply,
    /// Take maximum of existing and layer value
    Max,
    /// Take minimum of existing and layer value
    Min,
    /// Linear interpolation with specified weight
    Lerp(f32),
}

/// Mask type for selective layer application
#[derive(Clone, Debug)]
pub enum LayerMask {
    /// Apply based on current elevation range
    Elevation { min: f32, max: f32 },
    /// Apply based on moisture range (requires moisture map)
    Moisture { min: f32, max: f32 },
    /// Apply everywhere (no masking)
    None,
}

/// A single noise layer in the terrain stack
#[derive(Clone, Debug)]
pub struct NoiseLayer {
    /// Descriptive name for debugging
    pub name: &'static str,
    /// Seed offset for this layer
    pub seed_offset: u64,
    /// Base frequency of the noise
    pub frequency: f64,
    /// Amplitude (max contribution in meters)
    pub amplitude: f32,
    /// Number of octaves for fBm
    pub octaves: u32,
    /// Amplitude decay per octave
    pub persistence: f64,
    /// Frequency multiplier per octave
    pub lacunarity: f64,
    /// How to combine with existing terrain
    pub blend_mode: BlendMode,
    /// Optional mask for selective application
    pub mask: LayerMask,
}

impl Default for NoiseLayer {
    fn default() -> Self {
        Self {
            name: "default",
            seed_offset: 0,
            frequency: 0.01,
            amplitude: 50.0,
            octaves: 4,
            persistence: 0.5,
            lacunarity: 2.0,
            blend_mode: BlendMode::Add,
            mask: LayerMask::None,
        }
    }
}

impl NoiseLayer {
    /// Create a new noise layer with the given name
    pub fn new(name: &'static str) -> Self {
        Self { name, ..Default::default() }
    }

    /// Set frequency
    pub fn with_frequency(mut self, freq: f64) -> Self {
        self.frequency = freq;
        self
    }

    /// Set amplitude
    pub fn with_amplitude(mut self, amp: f32) -> Self {
        self.amplitude = amp;
        self
    }

    /// Set octaves
    pub fn with_octaves(mut self, octaves: u32) -> Self {
        self.octaves = octaves;
        self
    }

    /// Set persistence
    pub fn with_persistence(mut self, persistence: f64) -> Self {
        self.persistence = persistence;
        self
    }

    /// Set lacunarity
    pub fn with_lacunarity(mut self, lacunarity: f64) -> Self {
        self.lacunarity = lacunarity;
        self
    }

    /// Set blend mode
    pub fn with_blend_mode(mut self, mode: BlendMode) -> Self {
        self.blend_mode = mode;
        self
    }

    /// Set elevation mask
    pub fn with_elevation_mask(mut self, min: f32, max: f32) -> Self {
        self.mask = LayerMask::Elevation { min, max };
        self
    }

    /// Set seed offset
    pub fn with_seed_offset(mut self, offset: u64) -> Self {
        self.seed_offset = offset;
        self
    }
}

/// A stack of noise layers for terrain generation
#[derive(Clone, Debug)]
pub struct TerrainNoiseStack {
    /// Ordered layers (applied in sequence)
    pub layers: Vec<NoiseLayer>,
}

/// A compiled noise layer with pre-created Perlin noise generator
pub struct CompiledNoiseLayer {
    /// The noise generator (pre-created for performance)
    pub noise: Perlin,
    /// Reference to the original layer configuration
    pub frequency: f64,
    pub amplitude: f32,
    pub octaves: u32,
    pub persistence: f64,
    pub lacunarity: f64,
    pub blend_mode: BlendMode,
    pub mask: LayerMask,
}

/// A compiled noise stack with pre-created noise generators for efficient evaluation.
/// This avoids creating Perlin noise objects on every evaluate() call.
pub struct CompiledNoiseStack {
    /// Compiled layers with pre-created noise generators
    pub layers: Vec<CompiledNoiseLayer>,
}

impl CompiledNoiseStack {
    /// Evaluate all layers at a position
    pub fn evaluate(
        &self,
        x: f64,
        y: f64,
        current_elevation: f32,
        moisture: Option<f32>,
    ) -> f32 {
        let mut result = current_elevation;

        for layer in &self.layers {
            // Check mask
            let mask_weight = match &layer.mask {
                LayerMask::None => 1.0,
                LayerMask::Elevation { min, max } => {
                    if result >= *min && result <= *max {
                        // Smooth falloff at edges
                        let range = max - min;
                        let center = (min + max) / 2.0;
                        let dist = (result - center).abs() / (range / 2.0);
                        1.0 - dist.powi(2)
                    } else {
                        0.0
                    }
                }
                LayerMask::Moisture { min, max } => {
                    if let Some(m) = moisture {
                        if m >= *min && m <= *max {
                            let range = max - min;
                            let center = (min + max) / 2.0;
                            let dist = (m - center).abs() / (range / 2.0);
                            1.0 - dist.powi(2)
                        } else {
                            0.0
                        }
                    } else {
                        1.0 // No moisture data, apply fully
                    }
                }
            };

            if mask_weight <= 0.0 {
                continue;
            }

            // Sample noise using pre-created generator
            let nx = x * layer.frequency;
            let ny = y * layer.frequency;
            let noise_val = fbm(
                &layer.noise,
                nx, ny,
                layer.octaves,
                layer.persistence,
                layer.lacunarity,
            ) as f32;

            // Scale by amplitude and mask
            let layer_value = noise_val * layer.amplitude * mask_weight;

            // Apply blend mode
            result = match layer.blend_mode {
                BlendMode::Add => result + layer_value,
                BlendMode::Multiply => result * (1.0 + layer_value / layer.amplitude),
                BlendMode::Max => result.max(result + layer_value),
                BlendMode::Min => result.min(result + layer_value),
                BlendMode::Lerp(weight) => {
                    result * (1.0 - weight) + (result + layer_value) * weight
                }
            };
        }

        result
    }
}

impl TerrainNoiseStack {
    /// Create an empty noise stack
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a layer to the stack
    pub fn add_layer(&mut self, layer: NoiseLayer) -> &mut Self {
        self.layers.push(layer);
        self
    }

    /// Compile the noise stack for efficient evaluation.
    /// Pre-creates all Perlin noise generators so they don't need to be
    /// created on every evaluate() call.
    pub fn compile(&self, base_seed: u64) -> CompiledNoiseStack {
        let layers = self.layers.iter().map(|layer| {
            CompiledNoiseLayer {
                noise: Perlin::new(1).set_seed((base_seed + layer.seed_offset) as u32),
                frequency: layer.frequency,
                amplitude: layer.amplitude,
                octaves: layer.octaves,
                persistence: layer.persistence,
                lacunarity: layer.lacunarity,
                blend_mode: layer.blend_mode,
                mask: layer.mask.clone(),
            }
        }).collect();

        CompiledNoiseStack { layers }
    }

    /// Evaluate all layers at a position (legacy method - creates noise on each call)
    /// Prefer using compile() + CompiledNoiseStack::evaluate() for better performance.
    pub fn evaluate(
        &self,
        x: f64,
        y: f64,
        base_seed: u64,
        current_elevation: f32,
        moisture: Option<f32>,
    ) -> f32 {
        let mut result = current_elevation;

        for layer in &self.layers {
            // Check mask
            let mask_weight = match &layer.mask {
                LayerMask::None => 1.0,
                LayerMask::Elevation { min, max } => {
                    if result >= *min && result <= *max {
                        // Smooth falloff at edges
                        let range = max - min;
                        let center = (min + max) / 2.0;
                        let dist = (result - center).abs() / (range / 2.0);
                        1.0 - dist.powi(2)
                    } else {
                        0.0
                    }
                }
                LayerMask::Moisture { min, max } => {
                    if let Some(m) = moisture {
                        if m >= *min && m <= *max {
                            let range = max - min;
                            let center = (min + max) / 2.0;
                            let dist = (m - center).abs() / (range / 2.0);
                            1.0 - dist.powi(2)
                        } else {
                            0.0
                        }
                    } else {
                        1.0 // No moisture data, apply fully
                    }
                }
            };

            if mask_weight <= 0.0 {
                continue;
            }

            // Create noise for this layer (inefficient - use compile() instead)
            let noise = Perlin::new(1).set_seed((base_seed + layer.seed_offset) as u32);

            // Sample noise
            let nx = x * layer.frequency;
            let ny = y * layer.frequency;
            let noise_val = fbm(
                &noise,
                nx, ny,
                layer.octaves,
                layer.persistence,
                layer.lacunarity,
            ) as f32;

            // Scale by amplitude and mask
            let layer_value = noise_val * layer.amplitude * mask_weight;

            // Apply blend mode
            result = match layer.blend_mode {
                BlendMode::Add => result + layer_value,
                BlendMode::Multiply => result * (1.0 + layer_value / layer.amplitude),
                BlendMode::Max => result.max(result + layer_value),
                BlendMode::Min => result.min(result + layer_value),
                BlendMode::Lerp(weight) => {
                    result * (1.0 - weight) + (result + layer_value) * weight
                }
            };
        }

        result
    }
}

/// Predefined layer stacks for different terrain types
pub mod layer_presets {
    use super::*;

    /// Forest terrain: rolling hills with fine detail
    pub fn forest_layers() -> TerrainNoiseStack {
        let mut stack = TerrainNoiseStack::new();
        stack.add_layer(
            NoiseLayer::new("forest_hills")
                .with_frequency(0.02)
                .with_amplitude(50.0)
                .with_octaves(4)
                .with_persistence(0.5)
                .with_blend_mode(BlendMode::Add)
                .with_seed_offset(100)
        );
        stack.add_layer(
            NoiseLayer::new("forest_detail")
                .with_frequency(0.08)
                .with_amplitude(10.0)
                .with_octaves(3)
                .with_persistence(0.6)
                .with_blend_mode(BlendMode::Add)
                .with_seed_offset(101)
        );
        stack
    }

    /// Floodplain terrain: very flat with subtle variation
    pub fn floodplain_layers() -> TerrainNoiseStack {
        let mut stack = TerrainNoiseStack::new();
        stack.add_layer(
            NoiseLayer::new("floodplain_base")
                .with_frequency(0.005)
                .with_amplitude(5.0)
                .with_octaves(2)
                .with_persistence(0.3)
                .with_blend_mode(BlendMode::Lerp(0.8))
                .with_seed_offset(200)
        );
        stack
    }

    /// Mountain terrain: dramatic peaks with ridge noise
    pub fn mountain_layers() -> TerrainNoiseStack {
        let mut stack = TerrainNoiseStack::new();
        stack.add_layer(
            NoiseLayer::new("mountain_base")
                .with_frequency(0.03)
                .with_amplitude(200.0)
                .with_octaves(5)
                .with_persistence(0.55)
                .with_blend_mode(BlendMode::Add)
                .with_seed_offset(300)
        );
        stack.add_layer(
            NoiseLayer::new("mountain_ridges")
                .with_frequency(0.06)
                .with_amplitude(80.0)
                .with_octaves(3)
                .with_persistence(0.45)
                .with_blend_mode(BlendMode::Max)
                .with_elevation_mask(500.0, 3000.0)
                .with_seed_offset(301)
        );
        stack
    }

    /// Ocean terrain: gentle swells with current patterns
    pub fn ocean_layers() -> TerrainNoiseStack {
        let mut stack = TerrainNoiseStack::new();
        stack.add_layer(
            NoiseLayer::new("ocean_swells")
                .with_frequency(0.01)
                .with_amplitude(20.0)
                .with_octaves(3)
                .with_persistence(0.4)
                .with_blend_mode(BlendMode::Add)
                .with_elevation_mask(-6000.0, 0.0)
                .with_seed_offset(400)
        );
        stack
    }

    /// Desert terrain: dunes with wind patterns
    pub fn desert_layers() -> TerrainNoiseStack {
        let mut stack = TerrainNoiseStack::new();
        stack.add_layer(
            NoiseLayer::new("desert_dunes")
                .with_frequency(0.04)
                .with_amplitude(30.0)
                .with_octaves(3)
                .with_persistence(0.5)
                .with_blend_mode(BlendMode::Add)
                .with_seed_offset(500)
        );
        stack.add_layer(
            NoiseLayer::new("desert_detail")
                .with_frequency(0.15)
                .with_amplitude(8.0)
                .with_octaves(2)
                .with_persistence(0.4)
                .with_blend_mode(BlendMode::Add)
                .with_seed_offset(501)
        );
        stack
    }
}

/// Apply a noise stack to enhance a heightmap
pub fn apply_noise_stack(
    heightmap: &mut Tilemap<f32>,
    stack: &TerrainNoiseStack,
    seed: u64,
    moisture: Option<&Tilemap<f32>>,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Pre-compile the noise stack for efficient evaluation
    // This creates all Perlin noise generators once instead of per-cell
    let compiled = stack.compile(seed);

    for y in 0..height {
        for x in 0..width {
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;

            let current = *heightmap.get(x, y);
            let m = moisture.map(|m| *m.get(x, y));

            let new_elevation = compiled.evaluate(nx, ny, current, m);
            heightmap.set(x, y, new_elevation);
        }
    }
}

// =============================================================================
// ELEVATION CONSTANTS
// =============================================================================

// Continental elevations (meters)
const CONTINENTAL_MIN: f32 = 50.0;       // Lowland plains
const CONTINENTAL_MAX: f32 = 600.0;      // Base highland plateaus (increased)
const COASTAL_HEIGHT: f32 = 5.0;         // Beach level
const SHELF_DEPTH: f32 = -150.0;         // Continental shelf

// Oceanic elevations
const OCEAN_FLOOR: f32 = -5000.0;        // Deep ocean baseline (was -4000)
const OCEAN_RIDGE: f32 = -2500.0;        // Mid-ocean ridges (was -2000)
const TRENCH_SCALE: f32 = 4000.0;        // Additional depth for oceanic trenches

// Ridge parameters - prominent mountain ranges
const RIDGE_HEIGHT: f32 = 2500.0;        // Procedural ridge height (increased for real mountains)
const RIDGE_FREQUENCY: f64 = 0.012;      // Ridge spacing (lower = larger features)

// Tectonic stress multiplier - dramatic boundary mountains
const TECTONIC_SCALE: f32 = 2000.0;      // Increased for proper mountain ranges

// Volcanic island parameters (oceanic convergence zones)
const VOLCANIC_THRESHOLD: f32 = 0.015;   // Lower threshold for more islands
const VOLCANIC_BASE: f32 = -400.0;       // Seamount base (shallower for more emergence)
const VOLCANIC_PEAK: f32 = 1200.0;       // Max island peak height (taller islands)
const VOLCANIC_ISLAND_FREQ: f64 = 0.10;  // Island clustering frequency (lower = larger clusters)

// Coastal fractal parameters
const COAST_FRACTAL_OCTAVES: u32 = 5;
const COAST_FRACTAL_SCALE: f64 = 0.15;

// Hotspot archipelago parameters (independent ocean island clusters)
const HOTSPOT_MIN_DISTANCE: f32 = 12.0;       // Reduced min distance for more hotspot zones
const HOTSPOT_ZONE_FREQ: f64 = 0.007;         // Higher freq for more hotspot regions (~20% coverage)
const HOTSPOT_CHAIN_FREQ: f64 = 0.020;        // Lower freq for longer island chains
const HOTSPOT_ISLAND_FREQ: f64 = 0.10;        // Slightly lower for larger individual islands
const HOTSPOT_BASE_HEIGHT: f32 = 150.0;       // Higher minimum island height
const HOTSPOT_MAX_HEIGHT: f32 = 1200.0;       // Taller maximum peak

// Continental fragmentation parameters (breaking up coastlines into islands)
const FRAG_ZONE_FREQ: f64 = 0.006;            // Lower freq for larger fragmentation zones
const FRAG_ISLAND_FREQ: f64 = 0.04;           // Lower freq for larger scattered islands
const FRAG_WATER_RANGE: f32 = -180.0;         // Extended further into water for more offshore islands
const FRAG_LAND_RANGE: f32 = 80.0;            // Extended onto land for more coastal fragmentation
const FRAG_BASE_HEIGHT: f32 = 60.0;           // Higher minimum fragmented island height
const FRAG_MAX_HEIGHT: f32 = 700.0;           // Taller maximum fragmented island height

// Fjord incision parameters (narrow channels cutting into coast)
const FJORD_ZONE_FREQ: f64 = 0.012;           // Low freq for fjord zone selection
const FJORD_CHANNEL_LONG_FREQ: f64 = 0.005;   // Very low freq along channel (long features)
const FJORD_CHANNEL_NARROW_FREQ: f64 = 0.10;  // High freq across channel (narrow features)
const FJORD_MAX_DEPTH: f32 = 250.0;           // Max channel incision depth in meters (deeper)
const FJORD_MIN_ELEVATION: f32 = 3.0;         // Min land elevation to carve
const FJORD_MAX_ELEVATION: f32 = 800.0;       // Max land elevation to carve (higher terrain)

// =============================================================================
// MAIN HEIGHTMAP GENERATION
// =============================================================================

/// Generate a heightmap using layered terrain synthesis:
/// 1. Multi-octave fBm for base terrain variation
/// 2. Domain warping for natural-looking features
/// 3. Procedural ridges for internal mountains
/// 4. Tectonic stress for plate boundary mountains
/// 5. Smooth blending with continental mask
pub fn generate_heightmap(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> Tilemap<f32> {
    generate_heightmap_scaled(plate_map, plates, stress_map, seed, &MapScale::default())
}

/// Generate heightmap with explicit scale parameter
pub fn generate_heightmap_scaled(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    seed: u64,
    map_scale: &MapScale,
) -> Tilemap<f32> {
    let width = plate_map.width;
    let height = plate_map.height;
    let params = TerrainParams::default();
    
    // Initialize noise generators with different seeds for variety
    let terrain_noise = Perlin::new(1).set_seed(seed as u32);
    let warp_noise = Perlin::new(1).set_seed(seed as u32 + 1111);
    let ridge_noise = Perlin::new(1).set_seed(seed as u32 + 2222);
    let detail_noise = Perlin::new(1).set_seed(seed as u32 + 3333);
    let coast_noise = Perlin::new(1).set_seed(seed as u32 + 4444);  // For fractal coastlines
    
    // Pre-compute continental distance field for smooth blending
    let continental_distance = compute_continental_distance(plate_map, plates);
    
    // Pre-compute distance from coast for gradient
    let coast_distance = compute_coast_distance(plate_map, plates);
    
    let mut heightmap = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() {
                heightmap.set(x, y, OCEAN_FLOOR);
                continue;
            }
            
            let plate = &plates[plate_id.0 as usize];
            let stress = *stress_map.get(x, y);
            let cont_dist = *continental_distance.get(x, y);
            let raw_coast_dist = *coast_distance.get(x, y);
            
            // Normalize coordinates for noise sampling
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;
            
            // Apply domain warping for organic shapes
            let (warped_x, warped_y) = apply_domain_warp(
                nx, ny, &warp_noise, params.warp_strength, seed
            );
            
            // Scale noise frequencies and distances based on map scale
            let coast_fractal_freq = scale_frequency(COAST_FRACTAL_SCALE * 100.0, map_scale);
            let coast_perturb_range = scale_distance(50.0, map_scale);
            let coast_perturb_mag = scale_distance(25.0, map_scale);

            // Fractal perturbation for coastline - creates jagged edges
            let coast_fractal = fbm(
                &coast_noise,
                nx * coast_fractal_freq,
                ny * coast_fractal_freq,
                COAST_FRACTAL_OCTAVES,
                0.6,
                2.2
            ) as f32;

            // Perturb coast distance - larger perturbation near coast
            let coast_perturbation = if raw_coast_dist.abs() < coast_perturb_range {
                coast_fractal * coast_perturb_mag * (1.0 - raw_coast_dist.abs() / coast_perturb_range)
            } else {
                0.0
            };
            let coast_dist = raw_coast_dist + coast_perturbation;

            // Base elevation depends on plate type
            let elevation = match plate.plate_type {
                PlateType::Continental => {
                    generate_continental_elevation(
                        warped_x, warped_y,
                        coast_dist,
                        stress,
                        &terrain_noise,
                        &ridge_noise,
                        &detail_noise,
                        &params,
                        seed,
                        map_scale,
                    )
                }
                PlateType::Oceanic => {
                    // Compute stress gradient for island arc alignment
                    // This tells us the boundary direction for curving island chains
                    let stress_dx = if x > 0 && x < width - 1 {
                        *stress_map.get(x + 1, y) - *stress_map.get(x - 1, y)
                    } else { 0.0 };
                    let stress_dy = if y > 0 && y < height - 1 {
                        *stress_map.get(x, y + 1) - *stress_map.get(x, y - 1)
                    } else { 0.0 };
                    let stress_gradient = (stress_dx, stress_dy);

                    // Use original coordinates for ocean/islands - no domain warping
                    generate_oceanic_elevation(
                        nx, ny,
                        cont_dist,
                        stress,
                        stress_gradient,
                        &terrain_noise,
                        &detail_noise,
                        &params,
                        seed,
                        map_scale,
                    )
                }
            };
            
            heightmap.set(x, y, elevation);
        }
    }
    
    // Apply smoothing pass to reduce harsh transitions
    smooth_heightmap(&heightmap, 2)
}

// =============================================================================
// CONTINENTAL TERRAIN
// =============================================================================

/// Generate elevation for continental plates
fn generate_continental_elevation(
    x: f64,
    y: f64,
    coast_distance: f32,
    stress: f32,
    terrain_noise: &Perlin,
    ridge_noise: &Perlin,
    detail_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Scale distance and elevation thresholds
    let shelf_blend_dist = scale_distance(50.0, map_scale);
    let coastal_grad_dist = scale_distance(150.0, map_scale);
    let ridge_height = scale_elevation(RIDGE_HEIGHT, map_scale);
    let detail_height = scale_elevation(50.0, map_scale);
    let tectonic_scale = scale_elevation(TECTONIC_SCALE, map_scale);
    let detail_freq = scale_frequency(25.0, map_scale);

    // Underwater continental shelf - with island generation checks
    if coast_distance < 0.0 {
        // Check for continental fragmentation islands first (archipelago-like scatter)
        let frag_island = generate_continental_fragmentation(
            x, y, coast_distance, terrain_noise, detail_noise, seed, map_scale
        );

        if frag_island > 0.0 {
            // Fragmented archipelago island rises above sea level
            return frag_island;
        }

        // Check for barrier islands (they rise above sea level)
        let barrier_island = generate_barrier_islands(
            x, y, coast_distance, terrain_noise, detail_noise, seed, map_scale
        );

        if barrier_island > 0.0 {
            // Barrier island rises above sea level
            return barrier_island;
        }

        // Normal shelf depth
        let shelf_blend = (-coast_distance / shelf_blend_dist).min(1.0);
        let shelf_noise = fbm(terrain_noise, x * 2.0, y * 2.0, 3, 0.5, 2.0) as f32;
        return SHELF_DEPTH * shelf_blend + shelf_noise * scale_elevation(20.0, map_scale);
    }

    // Distance-based gradient (still use for blending, but less restrictive)
    let distance_factor = (coast_distance / coastal_grad_dist).min(1.0);
    let coastal_gradient = smooth_step(0.0, 1.0, distance_factor);

    // Scale base frequency for terrain
    let base_freq = scale_frequency(params.base_frequency * 80.0, map_scale);

    // Multi-octave fBm for base terrain - always present, not just inland
    let base_fbm = fbm(
        terrain_noise,
        x * base_freq,
        y * base_freq,
        params.octaves,
        params.persistence,
        params.lacunarity,
    ) as f32;

    // Normalize fBm to 0-1 range
    let base_terrain = (base_fbm + 1.0) * 0.5;

    // PURE ISOLATED PEAKS - no ridged noise at all
    // Completely eliminates "wormy" continuous ridge patterns
    // Mountains are formed ONLY from isolated peak clusters

    // Isolated peak noise - ONLY mountain source
    let isolated_peaks = generate_isolated_peaks(x, y, detail_noise, map_scale);

    // Add some additional peak variation at different scale for variety
    let peaks_fine = generate_isolated_peaks(x * 1.7, y * 1.7, ridge_noise, map_scale);
    let peaks_coarse = generate_isolated_peaks(x * 0.6, y * 0.6, terrain_noise, map_scale);

    // Blend peak layers for multi-scale mountains
    let combined_peaks = (isolated_peaks * 0.5 + peaks_fine * 0.3 + peaks_coarse * 0.3).min(1.0);

    // Sharp peaks
    let ridge_squared = combined_peaks * combined_peaks;

    // Ridges are present everywhere but slightly higher inland
    let ridge_contribution = ridge_squared * ridge_height * (0.5 + coastal_gradient * 0.5);

    // Fine detail noise for texture
    let detail = fbm(detail_noise, x * detail_freq, y * detail_freq, 4, 0.6, 2.0) as f32;
    let detail_contribution = detail * detail_height;

    // High-frequency mountain roughness - adds jagged crags to break smooth ridges
    // This layer kicks in proportionally to ridge height
    let roughness_freq = scale_frequency(100.0, map_scale);  // Higher frequency for finer crags
    let roughness_raw = fbm(detail_noise, x * roughness_freq, y * roughness_freq, 6, 0.6, 2.0) as f32;
    // Roughness amplitude scales with ridge contribution (more rough = more jagged peaks)
    let roughness_amplitude = scale_elevation(400.0, map_scale);  // Up to 400m of roughness
    // Apply roughness to all elevated terrain, not just ridges
    let elevation_factor = (ridge_squared + coastal_gradient * 0.3).min(1.0);
    let roughness_contribution = roughness_raw * roughness_amplitude * elevation_factor;
    
    // Scale frequencies for tectonic noise
    let peak_freq = scale_frequency(150.0, map_scale);
    let chain_freq = scale_frequency(40.0, map_scale);
    let rift_freq = scale_frequency(60.0, map_scale);

    // Tectonic stress contribution (mountains at plate boundaries)
    // Uses isolated peaks instead of ridged noise to avoid "wormy" appearance
    let tectonic = if stress > 0.05 {
        // Isolated peaks for tectonic mountains - creates distinct peaks, not ridges
        let tectonic_peaks = generate_isolated_peaks(x * 1.3, y * 1.3, detail_noise, map_scale);

        // High-frequency detail for individual peak variation
        let peak_variation = detail_noise.get([x * peak_freq, y * peak_freq, 0.5]) as f32;
        let peak_factor = 0.6 + peak_variation * 0.4; // 0.2 to 1.0 range

        // Combine: stress provides envelope, peaks create variation
        let base_height = stress.sqrt() * tectonic_scale;
        let peak_modulation = 0.4 + tectonic_peaks * 0.6; // 0.4 to 1.0
        let organic_height = base_height * peak_modulation * peak_factor;

        // Add some extra height for strong peak areas
        let dramatic_peaks = if tectonic_peaks > 0.6 {
            base_height * 0.25 * (tectonic_peaks - 0.6) / 0.4
        } else {
            0.0
        };

        organic_height + dramatic_peaks
    } else if stress < -0.05 {
        // Enhanced rift valleys at divergent continental boundaries
        // Creates deep, linear depressions like the East African Rift
        let rift_strength = (-stress - 0.05).min(0.5);

        // Low-frequency noise for linear rift coherence (elongated pattern)
        let rift_linear = terrain_noise.get([x * 0.03, y * 0.03, 2.0]) as f32;

        // High-frequency detail for rift floor variation
        let rift_detail = detail_noise.get([x * rift_freq, y * rift_freq, 2.5]) as f32;

        // Deeper rifts (0.6 scale vs old 0.2) with linear pattern
        let rift_depth = rift_strength * tectonic_scale * 0.6;
        let rift_floor = 0.7 + rift_linear * 0.3; // 70-100% of full depth

        // Final rift elevation (negative = depression)
        -rift_depth * rift_floor * (0.8 + rift_detail * 0.2)
    } else {
        0.0
    };
    
    // Combine all layers:
    // - Base elevation provides underlying terrain variation (always present)
    // - Coastal gradient mainly affects minimum elevation
    let min_elevation = COASTAL_HEIGHT + CONTINENTAL_MIN * coastal_gradient;
    let base_variation = base_terrain * CONTINENTAL_MAX * (0.3 + coastal_gradient * 0.7);

    // roughness_contribution adds jagged detail to mountain ridges
    min_elevation + base_variation + ridge_contribution + detail_contribution + roughness_contribution + tectonic
}

/// Generate small coastal islands near continental edges
fn generate_coastal_islands(
    x: f64,
    y: f64,
    coast_distance: f32,
    coast_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
) -> f32 {
    // Island probability increases closer to coast, peaks around -25 distance
    // Extended range for more offshore islands
    let distance_factor = (-coast_distance - 5.0) / 80.0; // Extended range
    let proximity_factor = if coast_distance > -40.0 {
        // Peak probability near coast (extended)
        1.0 - ((-coast_distance - 20.0).abs() / 20.0).min(1.0)
    } else {
        // Decreasing further out
        1.0 - distance_factor.min(1.0)
    };

    // Multi-scale noise for island clusters - lower frequencies for larger clusters
    let large_cluster = coast_noise.get([
        x * 90.0,
        y * 90.0,
        seed_to_z(seed, 2.1),
    ]);

    let medium_cluster = coast_noise.get([
        x * 200.0 + 5.2,
        y * 200.0 + 3.1,
        seed_to_z(seed, 2.2),
    ]);

    let small_peaks = detail_noise.get([
        x * 400.0,
        y * 400.0,
        seed_to_z(seed, 2.3),
    ]);

    // Combine scales - larger features guide smaller ones
    let combined = (large_cluster * 0.4 + medium_cluster * 0.35 + small_peaks * 0.25 + 0.5) as f32;

    // Lower threshold for island formation - more islands
    let base_threshold = 0.55;
    let threshold = base_threshold - proximity_factor * 0.18;

    if combined < threshold {
        return f32::MIN; // No island - return very low so it doesn't override ocean
    }

    // Island height - taller islands possible
    let peak_factor = ((combined - threshold) / (1.0 - threshold)).min(1.0);
    let max_height = 250.0 * proximity_factor; // Taller islands possible

    // Some islands are just rocks, some are proper islands
    let height = 8.0 + peak_factor * max_height;

    height
}

// =============================================================================
// OCEANIC TERRAIN
// =============================================================================

/// Generate elevation for oceanic plates
fn generate_oceanic_elevation(
    x: f64,
    y: f64,
    continental_distance: f32,
    stress: f32,
    stress_gradient: (f32, f32),  // Gradient for island arc alignment
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Scale parameters
    let ocean_freq = scale_frequency(params.base_frequency * 50.0, map_scale);
    let shelf_blend_dist = scale_distance(15.0, map_scale);  // Reduced from 100 - shelf transition is narrow
    let ocean_variation = scale_elevation(1500.0, map_scale);  // Increased from 500 for more depth variety
    let shelf_noise_height = scale_elevation(50.0, map_scale);

    // Base ocean floor with variation
    let base_fbm = fbm(
        terrain_noise,
        x * ocean_freq,
        y * ocean_freq,
        4,
        0.5,
        2.0,
    ) as f32;

    let variation = base_fbm * ocean_variation;
    let base = OCEAN_FLOOR + variation;

    // Mid-ocean ridges with visible linear structure (spreading centers)
    // Real ridges have: elevated terrain, parallel ridge peaks, and central axial valley
    let ridge_contribution = if stress < -0.1 {
        let ridge_strength = (-stress - 0.1).min(1.0);
        let base_lift = (OCEAN_RIDGE - OCEAN_FLOOR) * ridge_strength;

        // Linear ridge texture - creates parallel peaks perpendicular to spreading
        let ridge_texture = terrain_noise.get([x * 0.08, y * 0.08, 5.0]) as f32;
        let ridge_peaks = (ridge_texture * std::f32::consts::PI).sin().abs();

        // Central axial rift valley along ridge axis (characteristic of mid-ocean ridges)
        let axial_noise = detail_noise.get([x * 0.15, y * 0.15, 6.0]) as f32;
        let axial_valley = if axial_noise.abs() < 0.15 { 200.0 } else { 0.0 };

        // Combine: base elevation lift + ridge peaks - central valley
        base_lift + ridge_peaks * 300.0 * ridge_strength - axial_valley * ridge_strength
    } else {
        0.0
    };

    // Oceanic trenches at convergent boundaries (subduction zones)
    // High positive stress in ocean = deep trenches (like Mariana, Puerto Rico)
    let trench_contribution = if stress > 0.25 {
        let trench_strength = ((stress - 0.25) / 0.5).min(1.0);
        -trench_strength * TRENCH_SCALE  // Negative = deeper
    } else {
        0.0
    };

    // Calculate base ocean elevation
    let ocean_elevation = base + ridge_contribution + trench_contribution;

    // Island arcs at convergent boundaries (subduction zones)
    // Creates curving volcanic chains parallel to trenches (like Japan, Aleutians, Caribbean)
    let volcanic_elevation = if stress > VOLCANIC_THRESHOLD {
        let v = generate_island_arc(
            x, y, stress, stress_gradient, terrain_noise, detail_noise, seed, map_scale
        );
        v
    } else {
        f32::MIN
    };

    // Hotspot archipelagos - independent of plate stress
    // Creates Hawaii-like or Faroe-like island chains in open ocean
    let hotspot_elevation = generate_hotspot_archipelago(
        x, y, continental_distance, terrain_noise, detail_noise, seed, map_scale
    );

    // Use the higher of ocean floor, volcanic island, or hotspot island
    let final_ocean = ocean_elevation.max(volcanic_elevation).max(hotspot_elevation);

    // Transition zone near continental shelf
    let shelf_blend = if continental_distance < shelf_blend_dist {
        let t = continental_distance / shelf_blend_dist;
        smooth_step(0.0, 1.0, t)
    } else {
        1.0
    };

    // Blend from shelf depth to ocean floor
    let shelf_elevation = SHELF_DEPTH + base_fbm * shelf_noise_height;

    shelf_elevation * (1.0 - shelf_blend) + final_ocean * shelf_blend
}

/// Generate island arc chains parallel to subduction trenches
/// Creates curving volcanic chains like Japan, Aleutians, Caribbean island arcs
fn generate_island_arc(
    x: f64,
    y: f64,
    stress: f32,
    stress_gradient: (f32, f32),
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Only generate in convergent zones with significant stress
    if stress < 0.08 { return f32::MIN; }

    let stress_factor = (stress / 0.3).min(1.0);

    // Calculate boundary tangent (perpendicular to stress gradient)
    // This gives us the direction along which the island arc curves
    let grad_mag = (stress_gradient.0 * stress_gradient.0 + stress_gradient.1 * stress_gradient.1).sqrt();

    // If gradient is too weak, fall back to scattered generation
    if grad_mag < 0.01 {
        return generate_volcanic_islands_scaled(x, y, stress, detail_noise, seed, map_scale);
    }

    // Boundary tangent (perpendicular to gradient = along the boundary)
    let tangent = (-stress_gradient.1 / grad_mag, stress_gradient.0 / grad_mag);

    // Create arc-aligned coordinate system
    // u = distance along the arc, v = distance from the arc center
    let u = x * tangent.0 as f64 + y * tangent.1 as f64;
    let v = x * (-tangent.1) as f64 + y * tangent.0 as f64;

    // Scale frequencies for map scale
    let arc_freq = scale_frequency(0.15, map_scale);
    let spacing_freq = scale_frequency(0.4, map_scale);
    let detail_freq = scale_frequency(200.0, map_scale);

    // Island placement along the arc (creates chain pattern)
    // Use sine wave along tangent direction for regular spacing
    let arc_position = terrain_noise.get([u * arc_freq, v * 0.02, seed_to_z(seed, 0.4)]) as f32;

    // Island spacing along the arc (~50-100km apart in chain)
    let chain_pattern = (u * spacing_freq + arc_position as f64 * 0.5).sin() as f32;
    let is_in_chain = chain_pattern > 0.3;  // Creates discrete island spots along arc

    // Width of the island arc band (narrower = more linear chain)
    let arc_width_noise = detail_noise.get([x * 0.08, y * 0.08, seed_to_z(seed, 0.5)]) as f32;
    let arc_band = 0.15 + arc_width_noise * 0.05;  // Narrow band for arc

    // Check if we're in the arc band
    let distance_from_center = (terrain_noise.get([v * 0.1, u * 0.02, seed_to_z(seed, 0.6)]) as f32).abs();
    let in_arc_band = distance_from_center < arc_band;

    if !is_in_chain || !in_arc_band {
        return f32::MIN;
    }

    // High-frequency detail for island peaks
    let peak_noise = detail_noise.get([x * detail_freq, y * detail_freq, seed_to_z(seed, 0.7)]) as f32;
    let is_peak = peak_noise > 0.2;

    if !is_peak {
        return f32::MIN;
    }

    // Scale island heights
    let volcanic_base = scale_elevation(120.0, map_scale);
    let volcanic_extra = scale_elevation(400.0, map_scale);
    let stress_bonus = scale_elevation(80.0, map_scale);

    // Island height based on peak quality and stress
    let peak_factor = ((peak_noise - 0.2) / 0.8).min(1.0);
    let base_height = volcanic_base + peak_factor * volcanic_extra;
    let height = base_height + stress_factor * stress_bonus;

    height
}

/// Generate volcanic islands at oceanic convergence zones (island arcs)
/// Creates scattered archipelago-like clusters of small islands, NOT continuous ridges
fn generate_volcanic_islands(
    x: f64,
    y: f64,
    stress: f32,
    noise: &Perlin,
    seed: u64,
) -> f32 {
    // Scale stress to 0-1 range for probability
    let stress_factor = (stress / 0.2).min(1.0);
    
    // High-frequency noise for isolated island spots
    let spot1 = noise.get([x * 500.0, y * 500.0, seed_to_z(seed, 0.1)]);
    let spot2 = noise.get([x * 450.0 + 77.0, y * 450.0 + 33.0, seed_to_z(seed, 0.2)]);

    // Cluster zones - medium frequency
    let cluster = noise.get([x * 80.0, y * 80.0, seed_to_z(seed, 0.3)]);
    let in_cluster = cluster > -0.6; // ~80% of stressed areas can have islands (increased)

    if !in_cluster {
        return f32::MIN;
    }

    // Take max of spots for isolated peaks (not average - creates dots not lines)
    let best_spot = spot1.max(spot2) as f32;

    // Higher stress = lower threshold = more islands
    // Lower base threshold for more islands overall
    let threshold = 0.22 - stress_factor * 0.20;
    
    if best_spot < threshold {
        return f32::MIN;
    }
    
    // Island height - scale with how much we exceeded threshold
    let peak_factor = ((best_spot - threshold) / (1.0 - threshold)).min(1.0);
    
    // Chance for larger volcanic islands - the highest peaks become volcanoes
    let is_volcanic = peak_factor > 0.7;
    let base_height = if is_volcanic {
        // Volcanic peaks: 150-400m
        150.0 + (peak_factor - 0.7) / 0.3 * 250.0
    } else {
        // Small islands: 30-150m
        30.0 + peak_factor * 120.0
    };
    
    // Stress bonus for all islands
    let height = base_height + stress_factor * 50.0;

    height
}

/// Generate volcanic islands with explicit scale parameter
fn generate_volcanic_islands_scaled(
    x: f64,
    y: f64,
    stress: f32,
    noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Scale frequencies
    let spot_freq1 = scale_frequency(500.0, map_scale);
    let spot_freq2 = scale_frequency(450.0, map_scale);
    let cluster_freq = scale_frequency(80.0, map_scale);

    // Scale stress to 0-1 range for probability
    let stress_factor = (stress / 0.2).min(1.0);

    // High-frequency noise for isolated island spots
    let spot1 = noise.get([x * spot_freq1, y * spot_freq1, seed_to_z(seed, 1.1)]);
    let spot2 = noise.get([x * spot_freq2 + 77.0, y * spot_freq2 + 33.0, seed_to_z(seed, 1.2)]);

    // Cluster zones - medium frequency
    let cluster = noise.get([x * cluster_freq, y * cluster_freq, seed_to_z(seed, 1.3)]);
    let in_cluster = cluster > -0.6; // ~80% of stressed areas can have islands (increased)

    if !in_cluster {
        return f32::MIN;
    }

    // Take max of spots for isolated peaks (not average - creates dots not lines)
    let best_spot = spot1.max(spot2) as f32;

    // Higher stress = lower threshold = more islands
    // Lower base threshold for more islands overall
    let threshold = 0.22 - stress_factor * 0.20;

    if best_spot < threshold {
        return f32::MIN;
    }

    // Island height - scale with how much we exceeded threshold
    let peak_factor = ((best_spot - threshold) / (1.0 - threshold)).min(1.0);

    // Scale island heights
    let volcanic_base = scale_elevation(150.0, map_scale);
    let volcanic_extra = scale_elevation(250.0, map_scale);
    let small_base = scale_elevation(30.0, map_scale);
    let small_extra = scale_elevation(120.0, map_scale);
    let stress_bonus = scale_elevation(50.0, map_scale);

    // Chance for larger volcanic islands - the highest peaks become volcanoes
    let is_volcanic = peak_factor > 0.7;
    let base_height = if is_volcanic {
        // Volcanic peaks
        volcanic_base + (peak_factor - 0.7) / 0.3 * volcanic_extra
    } else {
        // Small islands
        small_base + peak_factor * small_extra
    };

    // Stress bonus for all islands
    let height = base_height + stress_factor * stress_bonus;

    height
}

// =============================================================================
// HOTSPOT ARCHIPELAGOS (Independent Ocean Island Clusters)
// =============================================================================

/// Generate hotspot archipelago islands in open ocean
/// Creates Hawaii-like or Faroe-like island chains independent of plate boundaries
/// These represent mantle plume hotspots that create island chains as plates move over them
fn generate_hotspot_archipelago(
    x: f64,
    y: f64,
    continental_distance: f32,
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Only in deep ocean, far from continents
    let min_dist = scale_distance(HOTSPOT_MIN_DISTANCE, map_scale);
    if continental_distance < min_dist {
        return f32::MIN;
    }

    // Scale frequencies for map scale
    let zone_freq = scale_frequency(HOTSPOT_ZONE_FREQ * 100.0, map_scale);
    let chain_freq = scale_frequency(HOTSPOT_CHAIN_FREQ * 100.0, map_scale);
    let island_freq = scale_frequency(HOTSPOT_ISLAND_FREQ * 100.0, map_scale);

    // LOW-FREQUENCY hotspot zone placement - creates hotspot regions
    // ~20-25% of deep ocean gets hotspot activity (increased for more archipelagos)
    let hotspot_zone = terrain_noise.get([
        x * zone_freq,
        y * zone_freq,
        seed_to_z(seed, 70.0),
    ]) as f32;

    // Lower threshold for more hotspot zones
    if hotspot_zone < 0.15 {
        return f32::MIN;
    }

    let zone_strength = ((hotspot_zone - 0.15) / 0.85).min(1.0);

    // MEDIUM-FREQUENCY chain pattern - creates linear island chains within zones
    // Hotspots create chains as the plate moves over them (like Hawaiian chain)
    // Slightly elongated pattern for chain effect
    let chain_x = terrain_noise.get([
        x * chain_freq * 1.3,
        y * chain_freq,
        seed_to_z(seed, 71.0),
    ]) as f32;
    let chain_y = terrain_noise.get([
        x * chain_freq,
        y * chain_freq * 1.3,
        seed_to_z(seed, 72.0),
    ]) as f32;
    let chain_pattern = (chain_x + chain_y) * 0.5;

    // Chain modulation - affects island density along the chain
    let chain_factor = (chain_pattern * 0.5 + 0.5).clamp(0.3, 1.0);

    // HIGH-FREQUENCY individual islands using multiplicative rotated noise
    // This creates truly isolated peaks (same technique as generate_isolated_peaks)

    // Layer 1: Original orientation
    let n1 = detail_noise.get([x * island_freq, y * island_freq, seed_to_z(seed, 73.0)]);
    let p1 = (n1 * 1.6 + 0.2).max(0.0).min(1.0) as f32;

    // Layer 2: Rotated 60 degrees
    let cos60: f64 = 0.5;
    let sin60: f64 = 0.866;
    let x2 = x * cos60 - y * sin60;
    let y2 = x * sin60 + y * cos60;
    let n2 = detail_noise.get([x2 * island_freq, y2 * island_freq, seed_to_z(seed, 74.0)]);
    let p2 = (n2 * 1.6 + 0.2).max(0.0).min(1.0) as f32;

    // Layer 3: Rotated 120 degrees
    let cos120: f64 = -0.5;
    let sin120: f64 = 0.866;
    let x3 = x * cos120 - y * sin120;
    let y3 = x * sin120 + y * cos120;
    let n3 = detail_noise.get([x3 * island_freq, y3 * island_freq, seed_to_z(seed, 75.0)]);
    let p3 = (n3 * 1.6 + 0.2).max(0.0).min(1.0) as f32;

    // Multiply layers - islands only where ALL layers positive
    let isolation = (p1 * p2 * p3).sqrt();

    // Combined probability
    let combined = zone_strength * chain_factor * isolation;

    // Lower threshold for more island formation
    if combined < 0.05 {
        return f32::MIN;
    }

    // Island height based on combined strength
    let peak_factor = ((combined - 0.05) / 0.95).min(1.0);
    let base = scale_elevation(HOTSPOT_BASE_HEIGHT, map_scale);
    let extra = scale_elevation(HOTSPOT_MAX_HEIGHT - HOTSPOT_BASE_HEIGHT, map_scale);

    // Larger islands for stronger combined values
    let height = base + peak_factor.powf(0.7) * extra;

    // Add some height variation for volcanic peaks
    let peak_detail = detail_noise.get([
        x * island_freq * 2.0,
        y * island_freq * 2.0,
        seed_to_z(seed, 76.0),
    ]) as f32;
    let detail_bonus = scale_elevation(100.0, map_scale) * peak_detail.abs() * peak_factor;

    height + detail_bonus
}

// =============================================================================
// CONTINENTAL FRAGMENTATION (Breaking Coastlines into Islands)
// =============================================================================

/// Generate continental fragmentation - scattered islands at continental edges
/// Creates archipelago-like patterns near coasts (like Scotland's western islands, Norway's coast)
fn generate_continental_fragmentation(
    x: f64,
    y: f64,
    coast_distance: f32,
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Only in the fragmentation zone near coastlines
    let water_range = scale_distance(FRAG_WATER_RANGE, map_scale);
    let land_range = scale_distance(FRAG_LAND_RANGE, map_scale);

    if coast_distance < water_range || coast_distance > land_range {
        return f32::MIN;
    }

    // Only create islands in water (coast_distance < 0)
    // Land fragmentation is handled by fjord incisions
    if coast_distance >= 0.0 {
        return f32::MIN;
    }

    // Scale frequencies
    let zone_freq = scale_frequency(FRAG_ZONE_FREQ * 100.0, map_scale);
    let island_freq = scale_frequency(FRAG_ISLAND_FREQ * 100.0, map_scale);

    // ZONE-BASED fragmentation - not all coastlines fragment
    // ~35-40% of coastline gets archipelago-like fragmentation
    let frag_zone = terrain_noise.get([
        x * zone_freq,
        y * zone_freq,
        seed_to_z(seed, 80.0),
    ]) as f32;

    if frag_zone < 0.15 {
        return f32::MIN;  // No fragmentation in this coastal section
    }

    let zone_strength = ((frag_zone - 0.15) / 0.85).min(1.0);

    // Distance factor - more islands closer to coast, fewer further out
    let normalized_dist = -coast_distance / -water_range;  // 0 at coast, 1 at max water range
    // Peak probability at ~30% of the way out, taper at edges
    let dist_factor = if normalized_dist < 0.35 {
        normalized_dist / 0.35
    } else {
        1.0 - (normalized_dist - 0.35) / 0.65
    };

    if dist_factor < 0.1 {
        return f32::MIN;
    }

    // ISLAND SCATTER using multiplicative isolation
    // Primary layer
    let n1 = detail_noise.get([
        x * island_freq,
        y * island_freq,
        seed_to_z(seed, 81.0),
    ]) as f32;
    let p1 = (n1 * 1.5 + 0.35).max(0.0).min(1.0);

    // Rotated 45 degrees for second layer
    let cos45: f64 = 0.707;
    let sin45: f64 = 0.707;
    let x2 = x * cos45 - y * sin45;
    let y2 = x * sin45 + y * cos45;
    let n2 = detail_noise.get([
        x2 * island_freq * 0.9,
        y2 * island_freq * 0.9,
        seed_to_z(seed, 82.0),
    ]) as f32;
    let p2 = (n2 * 1.5 + 0.35).max(0.0).min(1.0);

    // Third layer at 90 degrees
    let n3 = detail_noise.get([
        -y * island_freq * 1.1,
        x * island_freq * 1.1,
        seed_to_z(seed, 83.0),
    ]) as f32;
    let p3 = (n3 * 1.5 + 0.3).max(0.0).min(1.0);

    // Multiplicative isolation
    let isolation = (p1 * p2 * p3).powf(0.6);

    // Combined probability
    let combined = zone_strength * dist_factor * isolation;

    // Lower threshold for more fragmented islands
    if combined < 0.08 {
        return f32::MIN;
    }

    // Island height
    let peak_factor = ((combined - 0.08) / 0.92).min(1.0);
    let base = scale_elevation(FRAG_BASE_HEIGHT, map_scale);
    let extra = scale_elevation(FRAG_MAX_HEIGHT - FRAG_BASE_HEIGHT, map_scale);

    // Height with some variation
    let height = base + peak_factor.powf(0.8) * extra;

    // Detail variation for natural look
    let height_detail = terrain_noise.get([
        x * island_freq * 1.5,
        y * island_freq * 1.5,
        seed_to_z(seed, 84.0),
    ]) as f32;
    let variation = scale_elevation(50.0, map_scale) * height_detail.abs() * peak_factor;

    height + variation
}

// =============================================================================
// BARRIER ISLANDS
// =============================================================================

/// Generate barrier islands parallel to coastlines
/// Creates long, thin sandy islands that run parallel to the coast (like the Outer Banks, Texas coast)
/// These form in shallow water and create protected lagoons behind them
fn generate_barrier_islands(
    x: f64,
    y: f64,
    coast_distance: f32,  // Negative = water, positive = land
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Barrier islands form offshore, in the shallow water zone
    // coast_distance is negative for water, so -5 to -35 range
    let min_offshore = scale_distance(-5.0, map_scale);   // Not too close to shore
    let max_offshore = scale_distance(-40.0, map_scale);  // Not too far out

    // Only in the right distance range
    if coast_distance > min_offshore || coast_distance < max_offshore {
        return f32::MIN;
    }

    // Optimal formation zone is 15-25 units offshore
    let optimal_dist = scale_distance(-20.0, map_scale);
    let dist_from_optimal = (coast_distance - optimal_dist).abs();
    let dist_factor = 1.0 - (dist_from_optimal / scale_distance(18.0, map_scale)).min(1.0);

    if dist_factor < 0.2 {
        return f32::MIN;
    }

    // Elongated pattern: low frequency parallel to coast (long axis), high freq perpendicular (narrow)
    // Using different frequency scales for the two axes creates elongated shapes
    let parallel_freq = scale_frequency(0.015, map_scale);  // Long axis - low freq = long features
    let perp_freq = scale_frequency(0.12, map_scale);       // Short axis - high freq = narrow

    // Sample noise at both frequencies
    let parallel_noise = terrain_noise.get([x * parallel_freq, y * parallel_freq, seed_to_z(seed, 7.0)]) as f32;
    let perp_noise = detail_noise.get([x * perp_freq, y * perp_freq, seed_to_z(seed, 8.0)]) as f32;

    // Combine: weight heavily toward parallel (elongated) pattern
    // The perpendicular noise creates breaks in the chain (inlets)
    let island_pattern = parallel_noise * 0.8 + perp_noise * 0.2;

    // Threshold for island formation
    if island_pattern < 0.25 {
        return f32::MIN;
    }

    // Island height: barrier islands are low and sandy (3-12m above sea level)
    let pattern_strength = (island_pattern - 0.25) / 0.75;  // 0-1 normalized
    let base_height = scale_elevation(3.0, map_scale);
    let max_extra = scale_elevation(9.0, map_scale);

    let height = base_height + pattern_strength * max_extra * dist_factor;

    // Add small-scale dune detail
    let dune_freq = scale_frequency(0.5, map_scale);
    let dune_noise = detail_noise.get([x * dune_freq, y * dune_freq, seed_to_z(seed, 9.0)]) as f32;
    let dune_height = scale_elevation(2.0, map_scale) * dune_noise.abs();

    height + dune_height
}

// =============================================================================
// KARST TERRAIN GENERATION
// =============================================================================

/// Calculate karst potential based on conditions
/// Returns 0.0-1.0 indicating likelihood of karst formation
/// Karst forms in wet areas with limestone bedrock (simulated via noise)
pub fn calculate_karst_potential(
    x: f64,
    y: f64,
    elevation: f32,
    moisture: f32,
    temperature: f32,
    limestone_noise: &Perlin,
    map_scale: &MapScale,
) -> f32 {
    // Must be on land
    if elevation <= 0.0 {
        return 0.0;
    }

    // Limestone presence (noise-based "geology")
    let limestone_freq = scale_frequency(0.03, map_scale);
    let limestone = limestone_noise.get([x * limestone_freq, y * limestone_freq, 3.14]) as f32;
    let has_limestone = limestone > 0.1;  // ~45% of land can have limestone

    if !has_limestone {
        return 0.0;
    }

    // Moisture factor - karst needs water for dissolution
    let moisture_factor = if moisture > 0.3 {
        ((moisture - 0.3) / 0.5).min(1.0)
    } else {
        0.0
    };

    // Temperature factor - dissolution works better in warm climates
    let temp_factor = if temperature > 5.0 {
        ((temperature - 5.0) / 20.0).min(1.0)
    } else {
        0.2  // Some karst even in cold climates
    };

    // Elevation factor - karst most common at low-moderate elevations
    let elev_factor = if elevation < 800.0 {
        1.0 - (elevation / 1200.0)
    } else {
        0.2
    };

    // Combine factors
    let limestone_strength = (limestone - 0.1) / 0.9;  // 0-1 for limestone presence
    limestone_strength * moisture_factor * temp_factor * elev_factor
}

/// Generate sinkhole/doline features - circular depressions
/// Returns negative value for depression depth
pub fn generate_sinkhole_terrain(
    x: f64,
    y: f64,
    karst_potential: f32,
    sinkhole_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    if karst_potential < 0.2 {
        return 0.0;
    }

    // High-frequency noise for sinkhole placement
    let spot_freq = scale_frequency(0.4, map_scale);
    let spot_noise = sinkhole_noise.get([x * spot_freq, y * spot_freq, seed_to_z(seed, 20.0)]) as f32;

    // Only create sinkholes at local maxima of noise (isolated spots)
    let threshold = 0.6 - karst_potential * 0.2;  // Higher karst = more sinkholes
    if spot_noise < threshold {
        return 0.0;
    }

    // Sinkhole depth based on how much it exceeds threshold
    let strength = (spot_noise - threshold) / (1.0 - threshold);
    let base_depth = scale_elevation(15.0, map_scale);  // 15m base depth
    let max_extra = scale_elevation(35.0, map_scale);   // Up to 50m total

    // Add variation
    let detail = detail_noise.get([x * spot_freq * 3.0, y * spot_freq * 3.0, seed_to_z(seed, 21.0)]) as f32;

    // Return negative value (depression)
    -(base_depth + strength * max_extra) * (0.7 + detail.abs() * 0.3) * karst_potential
}

/// Generate tower karst terrain - tall limestone pillars
/// Returns positive value for tower height
pub fn generate_tower_karst_terrain(
    x: f64,
    y: f64,
    karst_potential: f32,
    temperature: f32,
    tower_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Tower karst only in tropical climates with high karst potential
    if karst_potential < 0.4 || temperature < 18.0 {
        return 0.0;
    }

    let tropical_factor = ((temperature - 18.0) / 12.0).min(1.0);

    // Tower placement - creates isolated pillars
    let tower_freq = scale_frequency(0.25, map_scale);
    let tower_base = tower_noise.get([x * tower_freq, y * tower_freq, seed_to_z(seed, 30.0)]) as f32;

    // Secondary frequency for grouping towers
    let group_freq = scale_frequency(0.08, map_scale);
    let group_noise = tower_noise.get([x * group_freq, y * group_freq, seed_to_z(seed, 31.0)]) as f32;
    let in_tower_zone = group_noise > 0.0;

    if !in_tower_zone {
        return 0.0;
    }

    // Create isolated tower peaks
    let threshold = 0.55;
    if tower_base < threshold {
        return 0.0;
    }

    let strength = (tower_base - threshold) / (1.0 - threshold);

    // Tower heights - dramatic pillars
    let base_height = scale_elevation(50.0, map_scale);   // 50m base
    let max_extra = scale_elevation(150.0, map_scale);    // Up to 200m

    // Add detail for varied tower shapes
    let detail_freq = scale_frequency(0.8, map_scale);
    let detail = detail_noise.get([x * detail_freq, y * detail_freq, seed_to_z(seed, 32.0)]) as f32;

    (base_height + strength * max_extra) * karst_potential * tropical_factor * (0.8 + detail.abs() * 0.2)
}

/// Generate karst surface roughness - small-scale dissolution features
pub fn generate_karst_surface(
    x: f64,
    y: f64,
    karst_potential: f32,
    surface_noise: &Perlin,
    map_scale: &MapScale,
) -> f32 {
    if karst_potential < 0.1 {
        return 0.0;
    }

    // High-frequency roughness (karren, rillenkarren)
    let rough_freq = scale_frequency(1.5, map_scale);
    let roughness = surface_noise.get([x * rough_freq, y * rough_freq, 40.0]) as f32;

    // Scale roughness by karst potential
    let amplitude = scale_elevation(5.0, map_scale);  // Up to 5m surface variation
    roughness * amplitude * karst_potential * 0.5
}

// =============================================================================
// NOISE FUNCTIONS
// =============================================================================

/// Fractional Brownian Motion - multi-octave noise
fn fbm(
    noise: &Perlin,
    x: f64,
    y: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        total += amplitude * noise.get([x * frequency, y * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    
    total / max_value
}

/// Domain warping - distort coordinates for organic shapes
fn apply_domain_warp(
    x: f64,
    y: f64,
    noise: &Perlin,
    strength: f64,
    seed: u64,
) -> (f64, f64) {
    let warp_scale = 4.0;
    
    // First warp layer
    let warp_x1 = noise.get([x * warp_scale, y * warp_scale]);
    let warp_y1 = noise.get([x * warp_scale + 5.2, y * warp_scale + 1.3]);
    
    // Second warp layer (warp the warp for more organic feel)
    let x2 = x + warp_x1 * strength;
    let y2 = y + warp_y1 * strength;
    
    let warp_x2 = noise.get([x2 * warp_scale * 2.0, y2 * warp_scale * 2.0]);
    let warp_y2 = noise.get([x2 * warp_scale * 2.0 + 3.7, y2 * warp_scale * 2.0 + 8.1]);
    
    (
        x + (warp_x1 + warp_x2 * 0.5) * strength,
        y + (warp_y1 + warp_y2 * 0.5) * strength,
    )
}

/// Generate procedural ridges using ridged noise
fn generate_ridges(x: f64, y: f64, noise: &Perlin, power: f64) -> f64 {
    let freq = RIDGE_FREQUENCY * 100.0;

    // Multi-octave ridged noise
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_val = 0.0;

    for i in 0..4 {
        let n = noise.get([
            x * freq * frequency,
            y * freq * frequency,
            i as f64 * 0.5,
        ]);

        // Ridge function: 1 - |noise| creates ridges at zero crossings
        let ridge = 1.0 - n.abs();
        // Sharpen with power function
        let ridge = ridge.powf(power);

        total += amplitude * ridge;
        max_val += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    (total / max_val).max(0.0)
}

/// Generate procedural ridges with explicit scale parameter
fn generate_ridges_scaled(x: f64, y: f64, noise: &Perlin, power: f64, map_scale: &MapScale) -> f64 {
    let freq = scale_frequency(RIDGE_FREQUENCY * 100.0, map_scale);

    // VERY AGGRESSIVE DOMAIN WARPING - shatters continuous ridge lines into fragments
    // This is the key fix for the "brain coral" / "wormy" mountain appearance
    // Multiple scales of warping create chaotic, non-continuous terrain

    // Large-scale warp (shatters major ridge continuity into separate clusters)
    let warp1_freq = freq * 0.2;  // Very low frequency = continent-scale distortion
    let warp1_strength = 1.2;     // Very strong displacement
    let warp1_x = noise.get([x * warp1_freq, y * warp1_freq, 100.0]) * warp1_strength;
    let warp1_y = noise.get([x * warp1_freq + 5.2, y * warp1_freq + 1.3, 200.0]) * warp1_strength;

    // Medium-scale warp (breaks ridge paths into segments)
    let warp2_freq = freq * 0.5;
    let warp2_strength = 0.5;
    let warp2_x = noise.get([x * warp2_freq + 3.7, y * warp2_freq + 8.1, 150.0]) * warp2_strength;
    let warp2_y = noise.get([x * warp2_freq + 9.2, y * warp2_freq + 2.8, 250.0]) * warp2_strength;

    // Fine-scale warp (adds jagged irregularity to individual peaks)
    let warp3_freq = freq * 1.5;
    let warp3_strength = 0.15;
    let warp3_x = noise.get([x * warp3_freq + 7.1, y * warp3_freq + 4.4, 175.0]) * warp3_strength;
    let warp3_y = noise.get([x * warp3_freq + 2.9, y * warp3_freq + 6.7, 275.0]) * warp3_strength;

    // Combined warped coordinates - total warp up to ~1.85
    let warped_x = x + (warp1_x + warp2_x + warp3_x) / freq;
    let warped_y = y + (warp1_y + warp2_y + warp3_y) / freq;

    // PEAK ISOLATION MASK - creates distinct mountain clusters instead of continuous ridges
    // This uses low-frequency noise to create "mountain zones" vs "valley zones"
    let isolation_freq = freq * 0.15;
    let isolation_noise = noise.get([x * isolation_freq + 50.0, y * isolation_freq + 50.0, 600.0]);
    // Only ~40% of area gets significant mountains, rest are lowlands/foothills
    let isolation_mask = ((isolation_noise + 0.3) * 1.5).clamp(0.0, 1.0);

    // Multi-octave ridged noise - 6 octaves for detail
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_val = 0.0;

    for i in 0..6 {
        let n = noise.get([
            warped_x * freq * frequency,
            warped_y * freq * frequency,
            i as f64 * 0.5,
        ]);

        // Ridge function: 1 - |noise| creates ridges at zero crossings
        let ridge = 1.0 - n.abs();

        // Add "ridge breaking" - randomly suppress ridge height to create gaps/saddles
        // Combined with isolation mask, this creates truly isolated peak clusters
        let break_noise = noise.get([
            x * freq * frequency * 0.3,
            y * freq * frequency * 0.3,
            500.0 + i as f64,
        ]);

        // Aggressive breaking: creates ~40% gaps when combined with isolation
        let break_factor = if break_noise < -0.1 {
            (0.15 + (break_noise + 0.1) * 0.6 / 0.9).max(0.05)
        } else if break_noise < 0.2 {
            // Partial height for transitional zones
            0.6 + (break_noise + 0.1) * 0.4 / 0.3
        } else {
            1.0
        };

        // Sharpen with power function, then apply isolation mask and breaking
        let ridge = ridge.powf(power) * break_factor;

        total += amplitude * ridge;
        max_val += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    // Apply isolation mask - mountains only appear in "mountain zones"
    // This fundamentally breaks the continuous ridge pattern
    let base_result = (total / max_val).max(0.0);
    base_result * isolation_mask.powf(0.7)
}

/// Generate truly isolated mountain peaks using multiplicative rotated noise
/// Peaks only form where MULTIPLE rotated noise layers are all positive
/// This creates genuine isolation - no continuous ridges possible
fn generate_isolated_peaks(x: f64, y: f64, noise: &Perlin, map_scale: &MapScale) -> f32 {
    let freq = scale_frequency(3.5, map_scale);

    // MULTIPLICATIVE ROTATED NOISE
    // Sample noise at 3 different rotations and multiply the positive parts
    // Peaks only exist where ALL three layers happen to be positive
    // This mathematically guarantees isolation (no continuous patterns)

    // Layer 1: Original orientation
    let n1 = noise.get([x * freq, y * freq, 700.0]);
    let p1 = (n1 * 1.5).max(0.0).min(1.0);  // Expanded positive region

    // Layer 2: Rotated 60 degrees
    let cos60: f64 = 0.5;
    let sin60: f64 = 0.866;
    let x2 = x * cos60 - y * sin60;
    let y2 = x * sin60 + y * cos60;
    let n2 = noise.get([x2 * freq, y2 * freq, 750.0]);
    let p2 = (n2 * 1.5).max(0.0).min(1.0);

    // Layer 3: Rotated 120 degrees
    let cos120: f64 = -0.5;
    let sin120: f64 = 0.866;
    let x3 = x * cos120 - y * sin120;
    let y3 = x * sin120 + y * cos120;
    let n3 = noise.get([x3 * freq, y3 * freq, 800.0]);
    let p3 = (n3 * 1.5).max(0.0).min(1.0);

    // Multiply layers - only strong where ALL are positive
    let combined = p1 * p2 * p3;

    // Add medium-scale variation for varied peak sizes
    let med_freq = freq * 0.6;
    let n_med = noise.get([x * med_freq + 50.0, y * med_freq + 50.0, 850.0]);
    let p_med = (n_med * 1.3).max(0.0).min(1.0);

    // Layer 4 at 45 degrees for medium scale
    let cos45: f64 = 0.707;
    let sin45: f64 = 0.707;
    let x4 = x * cos45 - y * sin45;
    let y4 = x * sin45 + y * cos45;
    let n4 = noise.get([x4 * med_freq, y4 * med_freq, 900.0]);
    let p4 = (n4 * 1.3).max(0.0).min(1.0);

    let med_combined = p_med * p4;

    // Final: blend fine and medium isolated peaks
    let peak_value = (combined * 0.6 + med_combined * 0.5).min(1.0);

    // Sharpen to create more distinct peak boundaries
    (peak_value as f32).powf(0.6).min(1.0)
}

/// Smooth step interpolation
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// =============================================================================
// DISTANCE FIELDS
// =============================================================================

/// Compute distance from each cell to nearest continental plate
fn compute_continental_distance(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
) -> Tilemap<f32> {
    use std::collections::VecDeque;
    
    let width = plate_map.width;
    let height = plate_map.height;
    
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    // Initialize with continental plate boundaries
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() {
                continue;
            }
            
            let plate = &plates[plate_id.0 as usize];
            if plate.plate_type == PlateType::Continental {
                // Find cells that border oceanic plates
                let borders_ocean = plate_map.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                    let n_id = *plate_map.get(nx, ny);
                    !n_id.is_none() && plates[n_id.0 as usize].plate_type == PlateType::Oceanic
                });
                
                if borders_ocean {
                    distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                }
            }
        }
    }
    
    // BFS to fill distance field
    while let Some((x, y, dist)) = queue.pop_front() {
        for (nx, ny) in plate_map.neighbors_8(x, y) {
            let new_dist = dist + 1.0;
            if new_dist < *distance.get(nx, ny) {
                distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    distance
}

/// Compute signed distance from coast (positive = land, negative = water)
fn compute_coast_distance(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
) -> Tilemap<f32> {
    use std::collections::VecDeque;
    
    let width = plate_map.width;
    let height = plate_map.height;
    
    // First, identify all continental cells
    let mut is_continental = Tilemap::new_with(width, height, false);
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if !plate_id.is_none() && plates[plate_id.0 as usize].plate_type == PlateType::Continental {
                is_continental.set(x, y, true);
            }
        }
    }
    
    // Find coastal cells: continental cells that border oceanic cells
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    for y in 0..height {
        for x in 0..width {
            if *is_continental.get(x, y) {
                // Check if any neighbor is oceanic (not continental)
                let borders_ocean = plate_map.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                    let n_id = *plate_map.get(nx, ny);
                    // Borders ocean if neighbor is oceanic plate (not continental, not none)
                    !n_id.is_none() && plates[n_id.0 as usize].plate_type == PlateType::Oceanic
                });
                
                if borders_ocean {
                    distance.set(x, y, 0.0);
                    queue.push_back((x, y, 0.0));
                }
            }
        }
    }
    
    // BFS for land cells only - propagate distance from coast
    while let Some((x, y, dist)) = queue.pop_front() {
        for (nx, ny) in plate_map.neighbors_8(x, y) {
            if !*is_continental.get(nx, ny) {
                continue; // Only propagate within continental
            }
            let new_dist = dist + 1.0;
            if new_dist < *distance.get(nx, ny) {
                distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    // Now compute negative distances for water cells
    let mut water_distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    // Start from same coastal cells but propagate into water
    for y in 0..height {
        for x in 0..width {
            if *is_continental.get(x, y) && *distance.get(x, y) == 0.0 {
                water_distance.set(x, y, 0.0);
                queue.push_back((x, y, 0.0));
            }
        }
    }
    
    while let Some((x, y, dist)) = queue.pop_front() {
        for (nx, ny) in plate_map.neighbors_8(x, y) {
            if *is_continental.get(nx, ny) {
                continue; // Only propagate into water
            }
            let new_dist = dist + 1.0;
            if new_dist < *water_distance.get(nx, ny) {
                water_distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    // Combine: positive for land, negative for water
    // Note: f32::MAX means cell was not reached by BFS from coast
    // For continental cells, this means very far inland (or isolated from ocean)
    // For water cells, this means very far from any continent
    let mut signed_distance = Tilemap::new_with(width, height, 0.0f32);
    for y in 0..height {
        for x in 0..width {
            if *is_continental.get(x, y) {
                let d = *distance.get(x, y);
                // Unreachable continental = very far inland
                signed_distance.set(x, y, if d == f32::MAX { 200.0 } else { d });
            } else {
                let d = *water_distance.get(x, y);
                signed_distance.set(x, y, if d == f32::MAX { -1000.0 } else { -d });
            }
        }
    }
    
    signed_distance
}

// =============================================================================
// POST-PROCESSING
// =============================================================================

/// Apply smoothing to reduce harsh transitions
fn smooth_heightmap(heightmap: &Tilemap<f32>, radius: usize) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut result = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            
            for dy in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist <= radius as f32 {
                        let weight = 1.0 - dist / (radius as f32 + 1.0);
                        sum += *heightmap.get(nx, ny) * weight;
                        count += weight;
                    }
                }
            }
            
            result.set(x, y, sum / count);
        }
    }
    
    result
}

/// Normalize heightmap values to 0.0-1.0 range.
pub fn normalize_heightmap(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;

    for (_, _, &val) in heightmap.iter() {
        if val < min_val {
            min_val = val;
        }
        if val > max_val {
            max_val = val;
        }
    }

    let range = max_val - min_val;
    if range < 0.0001 {
        return heightmap.clone();
    }

    let mut normalized = Tilemap::new_with(heightmap.width, heightmap.height, 0.0);
    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let val = *heightmap.get(x, y);
            normalized.set(x, y, (val - min_val) / range);
        }
    }

    normalized
}

/// Generate land mask for continental plates (for compatibility with existing code)
pub fn generate_land_mask(
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    _seed: u64,
) -> Tilemap<bool> {
    let width = plate_map.width;
    let height = plate_map.height;
    let mut land_mask = Tilemap::new_with(width, height, false);

    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if !plate_id.is_none() && plates[plate_id.0 as usize].plate_type == PlateType::Continental {
                land_mask.set(x, y, true);
            }
        }
    }

    land_mask
}

/// Print a histogram of height values for debugging.
/// Shows distribution across bins and key statistics.
pub fn print_height_histogram(heightmap: &Tilemap<f32>, num_bins: usize) {
    let num_bins = num_bins.max(5).min(50);

    // Collect all heights and compute statistics
    let mut heights: Vec<f32> = Vec::with_capacity(heightmap.width * heightmap.height);
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    let mut sum = 0.0f64;

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let h = *heightmap.get(x, y);
            heights.push(h);
            min_h = min_h.min(h);
            max_h = max_h.max(h);
            sum += h as f64;
        }
    }

    let count = heights.len();
    let mean = sum / count as f64;

    // Compute standard deviation
    let variance: f64 = heights.iter()
        .map(|h| {
            let diff = *h as f64 - mean;
            diff * diff
        })
        .sum::<f64>() / count as f64;
    let std_dev = variance.sqrt();

    // Compute median
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if count % 2 == 0 {
        (heights[count / 2 - 1] + heights[count / 2]) / 2.0
    } else {
        heights[count / 2]
    };

    // Count above/below sea level
    let above_sea = heights.iter().filter(|h| **h >= 0.0).count();
    let below_sea = count - above_sea;

    // Create bins
    let range = max_h - min_h;
    let bin_width = range / num_bins as f32;
    let mut bins = vec![0usize; num_bins];

    for h in &heights {
        let bin_idx = ((*h - min_h) / bin_width) as usize;
        let bin_idx = bin_idx.min(num_bins - 1);
        bins[bin_idx] += 1;
    }

    // Find max bin for scaling
    let max_bin = *bins.iter().max().unwrap_or(&1);
    let bar_max_width = 50;

    // Print header
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                     HEIGHT DISTRIBUTION HISTOGRAM                     ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");

    // Print statistics
    println!("║ Statistics:                                                          ║");
    println!("║   Min: {:>10.2}m    Max: {:>10.2}m    Range: {:>10.2}m           ║", min_h, max_h, range);
    println!("║   Mean: {:>9.2}m    Median: {:>8.2}m    Std Dev: {:>8.2}m          ║", mean, median, std_dev);
    println!("║   Above sea level: {:>6} ({:>5.1}%)    Below: {:>6} ({:>5.1}%)       ║",
        above_sea, 100.0 * above_sea as f64 / count as f64,
        below_sea, 100.0 * below_sea as f64 / count as f64);
    println!("╠══════════════════════════════════════════════════════════════════════╣");

    // Print histogram
    for (i, &bin_count) in bins.iter().enumerate() {
        let bin_start = min_h + i as f32 * bin_width;
        let bin_end = bin_start + bin_width;
        let bar_len = (bin_count as f64 / max_bin as f64 * bar_max_width as f64) as usize;
        let bar = "█".repeat(bar_len);
        let pct = 100.0 * bin_count as f64 / count as f64;

        // Mark sea level bin
        let marker = if bin_start <= 0.0 && bin_end > 0.0 { "◄SEA" } else { "    " };

        println!("║ {:>7.0} - {:>6.0}m │{:<50}│{:>5.1}% {} ║",
            bin_start, bin_end, bar, pct, marker);
    }

    println!("╚══════════════════════════════════════════════════════════════════════╝\n");
}

// =============================================================================
// REGIONAL NOISE APPLICATION (Phase 2 Integration)
// =============================================================================

/// Apply region-based noise layers to enhance terrain variety.
/// Different terrain types get different noise stacks:
/// - Mountains (high stress): More rugged, dramatic peaks
/// - Oceans (elevation < 0): Subtle swells and ridges
/// - Floodplains (negative stress): Smooth, flat terrain
/// - Default (forest/grassland): Gentle rolling hills
pub fn apply_regional_noise_stacks(
    heightmap: &mut Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) {
    use noise::{Perlin, Seedable};
    use rayon::prelude::*;

    let width = heightmap.width;
    let height = heightmap.height;

    // Create noise generators for each terrain type
    let mountain_noise = Perlin::new(1).set_seed(seed as u32);
    let ocean_noise = Perlin::new(1).set_seed((seed + 1) as u32);
    let floodplain_noise = Perlin::new(1).set_seed((seed + 2) as u32);
    let forest_noise = Perlin::new(1).set_seed((seed + 3) as u32);

    // Define noise parameters per region type
    // (frequency, amplitude, octaves)
    let mountain_params = (0.08, 150.0f32, 4usize);
    let ocean_params = (0.02, 50.0f32, 3usize);
    let floodplain_params = (0.03, 20.0f32, 2usize);
    let forest_params = (0.05, 40.0f32, 3usize);

    // Compute noise contributions in parallel
    let contributions: Vec<(usize, usize, f32)> = (0..height)
        .into_par_iter()
        .flat_map(|y| {
            let mut row_contributions = Vec::with_capacity(width);
            for x in 0..width {
                let elevation = *heightmap.get(x, y);
                let stress = *stress_map.get(x, y);

                let nx = x as f64 / width as f64;
                let ny = y as f64 / height as f64;

                // Select noise based on region type
                let noise_contribution = if stress > 0.1 {
                    // Mountain regions - more dramatic variation
                    let (freq, amp, octaves) = mountain_params;
                    fbm_simple(&mountain_noise, nx * freq * 100.0, ny * freq * 100.0, octaves) as f32 * amp * stress
                } else if elevation < 0.0 {
                    // Ocean regions - subtle variation
                    let (freq, amp, octaves) = ocean_params;
                    fbm_simple(&ocean_noise, nx * freq * 100.0, ny * freq * 100.0, octaves) as f32 * amp
                } else if stress < -0.05 {
                    // Floodplain/rift regions - very smooth
                    let (freq, amp, octaves) = floodplain_params;
                    fbm_simple(&floodplain_noise, nx * freq * 100.0, ny * freq * 100.0, octaves) as f32 * amp
                } else {
                    // Default forest/grassland - gentle rolling
                    let (freq, amp, octaves) = forest_params;
                    fbm_simple(&forest_noise, nx * freq * 100.0, ny * freq * 100.0, octaves) as f32 * amp
                };

                row_contributions.push((x, y, elevation + noise_contribution));
            }
            row_contributions
        })
        .collect();

    // Apply contributions to heightmap
    for (x, y, new_elevation) in contributions {
        heightmap.set(x, y, new_elevation);
    }
}

// Pre-computed fBm constants (avoid recalculating in hot loops)
const FBM_AMPLITUDES: [f64; 6] = [1.0, 0.5, 0.25, 0.125, 0.0625, 0.03125];
const FBM_FREQUENCIES: [f64; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
const FBM_MAX_VALS: [f64; 6] = [1.0, 1.5, 1.75, 1.875, 1.9375, 1.96875];

/// Simple fBm helper for regional noise (optimized with precomputed constants)
#[inline]
fn fbm_simple(noise: &noise::Perlin, x: f64, y: f64, octaves: usize) -> f64 {
    use noise::NoiseFn;
    let octaves = octaves.min(6);
    let mut total = 0.0;

    for i in 0..octaves {
        total += FBM_AMPLITUDES[i] * noise.get([x * FBM_FREQUENCIES[i], y * FBM_FREQUENCIES[i]]);
    }

    total / FBM_MAX_VALS[octaves - 1]
}

// =============================================================================
// ARCHIPELAGO PASS - SMALL SCATTERED ISLANDS
// =============================================================================

/// Apply archipelago pass to create small scattered islands in shallow ocean areas.
///
/// This uses high-frequency noise to "sprinkle" small islands across shallow ocean
/// zones (continental shelves), creating archipelago-like clusters of tiny islands.
///
/// Islands form where:
/// - Water is shallow (-500m to -10m depth)
/// - High-frequency noise exceeds a threshold
/// - Multiple noise octaves align (creating clustered patterns)
/// Apply archipelago pass - creates islands guided by tectonic stress patterns.
/// Islands form preferentially near plate boundaries and stressed zones.
pub fn apply_archipelago_pass(
    heightmap: &mut Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) {
    use noise::{NoiseFn, Perlin, Seedable};
    use rayon::prelude::*;

    let width = heightmap.width;
    let height = heightmap.height;

    // Create noise generators with unique seed offsets
    let cluster_noise = Perlin::new(1).set_seed((seed + 5555) as u32);
    let shape_noise = Perlin::new(1).set_seed((seed + 6666) as u32);

    // Ocean depth thresholds
    const OCEAN_MIN: f32 = -2000.0;  // Not too deep
    const OCEAN_MAX: f32 = -10.0;    // Must be underwater

    // Lower frequencies = larger island clusters (not single tiles)
    const CLUSTER_FREQ: f64 = 0.03;   // Very low - creates large island groups
    const SHAPE_FREQ: f64 = 0.08;     // Low - creates smooth island shapes

    // Compute island modifications in parallel
    let islands: Vec<(usize, usize, f32)> = (0..height)
        .into_par_iter()
        .flat_map(|y| {
            let mut row_islands = Vec::new();
            for x in 0..width {
                let elevation = *heightmap.get(x, y);

                // Only affect ocean areas
                if elevation < OCEAN_MIN || elevation > OCEAN_MAX {
                    continue;
                }

                // Get tectonic stress at this location
                let stress = stress_map.get(x, y).abs();

                // Islands much more likely in stressed areas (near plate boundaries)
                // Minimum stress threshold - no islands in completely calm ocean
                if stress < 0.02 {
                    continue;
                }

                let stress_factor = (stress / 0.3).min(1.0); // Normalize stress

                let nx = x as f64 / width as f64;
                let ny = y as f64 / height as f64;

                // Large-scale cluster pattern - determines island group locations
                let cluster = cluster_noise.get([
                    nx * CLUSTER_FREQ * 100.0,
                    ny * CLUSTER_FREQ * 100.0,
                    seed_to_z(seed, 50.0)
                ]) as f32;

                // Shape pattern - determines island boundaries within clusters
                let shape = shape_noise.get([
                    nx * SHAPE_FREQ * 100.0,
                    ny * SHAPE_FREQ * 100.0,
                    seed_to_z(seed, 51.0)
                ]) as f32;

                // Combined pattern favoring larger connected features
                let combined = cluster * 0.6 + shape * 0.4;

                // Threshold depends on stress - higher stress = more islands
                // Base threshold is high, stress lowers it significantly
                let threshold = 0.35 - stress_factor * 0.30;

                if combined < threshold {
                    continue;
                }

                // Depth factor: shallower water = easier to form islands
                let depth_factor = 1.0 - (elevation.abs() / 2000.0);
                let depth_factor = depth_factor.max(0.2).min(1.0);

                // Calculate island height
                let strength = ((combined - threshold) / (1.0 - threshold)).min(1.0);

                // Height based on strength and stress (volcanic = taller)
                let base_height = 30.0 + strength * 200.0 * depth_factor;
                let stress_bonus = stress_factor * 150.0; // Volcanic islands are taller

                let island_height = base_height + stress_bonus;

                row_islands.push((x, y, island_height));
            }
            row_islands
        })
        .collect();

    // Apply islands to heightmap
    let islands_created = islands.len();
    for (x, y, new_height) in islands {
        heightmap.set(x, y, new_height);
    }

    if islands_created > 0 {
        println!("  Created {} archipelago island tiles", islands_created);
    }
}

/// Expand small islands into larger clusters (minimum 3 tiles).
/// Uses stress patterns to guide expansion direction.
pub fn expand_island_clusters(
    heightmap: &mut Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) {
    use noise::{NoiseFn, Perlin, Seedable};

    let width = heightmap.width;
    let height = heightmap.height;

    let shape_noise = Perlin::new(1).set_seed((seed + 3333) as u32);

    // Find all small islands (land tiles surrounded mostly by water)
    let mut island_seeds: Vec<(usize, usize, f32)> = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);

            // Only consider land tiles
            if elevation <= 0.0 {
                continue;
            }

            // Count water neighbors
            let mut water_neighbors = 0;
            let mut land_neighbors = 0;

            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    if *heightmap.get(nx, ny) <= 0.0 {
                        water_neighbors += 1;
                    } else {
                        land_neighbors += 1;
                    }
                }
            }

            // Island seed: land tile with mostly water neighbors (isolated)
            // This identifies small islands that need expansion
            if water_neighbors >= 5 && land_neighbors <= 3 {
                island_seeds.push((x, y, elevation));
            }
        }
    }

    // Expand each island seed into a larger cluster
    let mut expansions: Vec<(usize, usize, f32)> = Vec::new();

    for (ix, iy, base_elevation) in &island_seeds {
        let stress = stress_map.get(*ix, *iy).abs();
        let stress_factor = (stress / 0.2).min(1.0);

        // Expansion radius based on stress (higher stress = bigger volcanic islands)
        let base_radius = 2;
        let stress_radius = (stress_factor * 3.0) as i32;
        let radius = base_radius + stress_radius;

        let nx_base = *ix as f64 / width as f64;
        let ny_base = *iy as f64 / height as f64;

        // Expand in a radius around the island seed
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let dist_sq = dx * dx + dy * dy;
                let max_dist_sq = radius * radius;

                if dist_sq > max_dist_sq {
                    continue;
                }

                let nx = ((*ix as i32) + dx).rem_euclid(width as i32) as usize;
                let ny = ((*iy as i32) + dy).clamp(0, height as i32 - 1) as usize;

                // Only expand into water
                if *heightmap.get(nx, ny) > 0.0 {
                    continue;
                }

                // Use noise to create natural irregular shapes
                let sample_x = nx as f64 / width as f64;
                let sample_y = ny as f64 / height as f64;

                let shape = shape_noise.get([
                    sample_x * 15.0 * 100.0,
                    sample_y * 15.0 * 100.0,
                    seed_to_z(seed, 60.0)
                ]) as f32;

                // Distance falloff - closer to center = more likely to be land
                let dist = (dist_sq as f32).sqrt();
                let dist_factor = 1.0 - (dist / radius as f32);

                // Combined probability for this cell to become land
                let prob = dist_factor * 0.7 + (shape * 0.5 + 0.5) * 0.3;

                // Higher stress = fill in more of the island shape
                let threshold = 0.4 - stress_factor * 0.2;

                if prob > threshold {
                    // Height decreases toward edges
                    let edge_factor = dist_factor.powf(0.5);
                    let new_height = base_elevation * edge_factor * 0.8 + 20.0;

                    expansions.push((nx, ny, new_height.max(15.0)));
                }
            }
        }
    }

    // Apply expansions
    let expanded_count = expansions.len();
    for (x, y, new_height) in expansions {
        // Only expand if still water (don't overwrite other expansions with lower values)
        if *heightmap.get(x, y) <= 0.0 {
            heightmap.set(x, y, new_height);
        }
    }

    if expanded_count > 0 {
        println!("  Expanded {} island tiles from {} seeds", expanded_count, island_seeds.len());
    }
}

// =============================================================================
// FJORD INCISION SYSTEM
// =============================================================================

/// Compute distance from each cell to nearest water using BFS
/// Returns a tilemap with distance values (0.0 for water, increasing for land)
/// This is O(n) instead of O(n * r^2) for repeated neighbor searches
fn compute_distance_to_water(heightmap: &Tilemap<f32>, max_dist: f32) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize)> = VecDeque::with_capacity(width * height / 4);

    // Initialize: all water tiles have distance 0
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) < 0.0 {
                distance.set(x, y, 0.0);
                queue.push_back((x, y));
            }
        }
    }

    // BFS propagation - process cells in order of distance
    while let Some((x, y)) = queue.pop_front() {
        let current_dist = *distance.get(x, y);
        if current_dist >= max_dist {
            continue;
        }

        // Check 8 neighbors
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                // Diagonal distance is sqrt(2) ≈ 1.414
                let step = if dx != 0 && dy != 0 { 1.414 } else { 1.0 };
                let new_dist = current_dist + step;

                if new_dist < *distance.get(nx, ny) {
                    distance.set(nx, ny, new_dist);
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    distance
}

/// Apply fjord-like channel incisions to coastal terrain
/// This is a post-processing pass that carves narrow inlets into existing coastlines
/// Creates features like Norwegian fjords, Scottish sea lochs, Faroe Island sounds
pub fn apply_fjord_incisions(
    heightmap: &mut Tilemap<f32>,
    seed: u64,
    map_scale: &MapScale,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    let channel_noise = Perlin::new(1).set_seed((seed + 8001) as u32);
    let direction_noise = Perlin::new(1).set_seed((seed + 8002) as u32);
    let detail_noise = Perlin::new(1).set_seed((seed + 8003) as u32);

    // Scale parameters
    let zone_freq = scale_frequency(FJORD_ZONE_FREQ * 100.0, map_scale);
    let channel_long = scale_frequency(FJORD_CHANNEL_LONG_FREQ * 100.0, map_scale);
    let channel_narrow = scale_frequency(FJORD_CHANNEL_NARROW_FREQ * 100.0, map_scale);
    let max_depth = scale_elevation(FJORD_MAX_DEPTH, map_scale);
    let min_elev = scale_elevation(FJORD_MIN_ELEVATION, map_scale);
    let max_elev = scale_elevation(FJORD_MAX_ELEVATION, map_scale);

    let check_radius = 12.0f32;

    // Pre-compute distance to water using BFS - O(n) instead of O(n * r^2)
    let water_distance = compute_distance_to_water(heightmap, check_radius + 1.0);

    let mut fjords_carved = 0;

    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);

            // Only affect land in the right elevation range
            if elevation < min_elev || elevation > max_elev {
                continue;
            }

            // O(1) lookup instead of O(625) neighbor search
            let min_water_dist = *water_distance.get(x, y);
            if min_water_dist > check_radius || min_water_dist == 0.0 {
                continue;  // Too far from water or is water
            }

            // Normalized coordinates for noise sampling
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;

            // FJORD ZONE: low-frequency noise determines which coastal stretches get fjords
            let fjord_zone = channel_noise.get([
                nx * zone_freq,
                ny * zone_freq,
                seed_to_z(seed, 90.0),
            ]) as f32;

            if fjord_zone < 0.25 {
                continue;
            }

            let zone_strength = ((fjord_zone - 0.25) / 0.75).min(1.0);

            // DIRECTION: varies smoothly across the map for natural fjord orientations
            let dir_angle = direction_noise.get([
                nx * zone_freq * 0.5,
                ny * zone_freq * 0.5,
                seed_to_z(seed, 91.0),
            ]) as f32 * std::f32::consts::PI;

            let cos_d = dir_angle.cos() as f64;
            let sin_d = dir_angle.sin() as f64;

            // Rotated coordinates for elongated channel pattern
            let u = nx * cos_d + ny * sin_d;
            let v = -nx * sin_d + ny * cos_d;

            // CHANNEL PATTERN: anisotropic noise creates elongated channels
            let channel_pattern = channel_noise.get([
                u * channel_long * 100.0,
                v * channel_narrow * 100.0,
                seed_to_z(seed, 92.0),
            ]) as f32;

            if channel_pattern < 0.40 {
                continue;
            }

            let channel_strength = (channel_pattern - 0.40) / 0.60;

            // Distance factor - channels more likely/deeper closer to existing water
            let dist_factor = 1.0 - (min_water_dist / check_radius).min(1.0);

            // Elevation factor - deeper channels at lower elevations
            let elev_normalized = (elevation - min_elev) / (max_elev - min_elev);
            let elev_factor = 1.0 - elev_normalized * 0.6;

            // Combined incision depth
            let incision = max_depth * zone_strength * channel_strength * dist_factor * elev_factor;

            if incision < 5.0 {
                continue;
            }

            // Add detail noise for irregular channel floor
            let detail = detail_noise.get([
                nx * channel_narrow * 50.0,
                ny * channel_narrow * 50.0,
                seed_to_z(seed, 93.0),
            ]) as f32;
            let varied_incision = incision * (0.75 + detail.abs() * 0.25);

            // Carve the channel
            let new_elev = (elevation - varied_incision).max(-150.0);
            heightmap.set(x, y, new_elev);
            fjords_carved += 1;
        }
    }

    if fjords_carved > 0 {
        println!("  Carved {} fjord channel tiles", fjords_carved);
    }
}

// =============================================================================
// VOLCANO GENERATION SYSTEM
// =============================================================================
//
// At world map scale (several km per tile), a volcano fits within a single tile.
// We mark volcano tiles and calculate lava flow to adjacent tiles based on terrain.
// The detailed volcano structure is generated at region map scale.

/// Represents a volcano location with properties for region map generation.
#[derive(Clone, Debug)]
pub struct VolcanoLocation {
    /// Tile x coordinate
    pub x: usize,
    /// Tile y coordinate
    pub y: usize,
    /// Volcano peak height above surrounding terrain (in meters)
    pub peak_height: f32,
    /// Whether this is an active volcano (has lava)
    pub is_active: bool,
    /// Volcano type affects region map generation
    pub volcano_type: VolcanoType,
}

/// Type of volcano - affects region map generation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolcanoType {
    /// Shield volcano - broad, gentle slopes (like Hawaii)
    Shield,
    /// Stratovolcano - steep cone with crater (like Mt. Fuji)
    Stratovolcano,
    /// Caldera - collapsed crater, often with lake
    Caldera,
}

/// Find suitable locations for volcanoes based on tectonic stress.
///
/// Volcanoes form primarily at:
/// - Convergent plate boundaries (subduction zones) - high positive stress
/// - Oceanic hotspots (simulated with noise)
/// - Divergent boundaries (mid-ocean ridges) - high negative stress
///
/// Returns a list of volcano locations (single tiles).
pub fn find_volcano_locations(
    heightmap: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> Vec<VolcanoLocation> {
    use noise::{NoiseFn, Perlin, Seedable};

    let width = heightmap.width;
    let height = heightmap.height;

    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    // Noise for variation and hotspot simulation
    let hotspot_noise = Perlin::new(1).set_seed((seed + 8888) as u32);
    let variation_noise = Perlin::new(1).set_seed((seed + 9999) as u32);

    // Minimum stress threshold for volcano formation
    const MIN_STRESS: f32 = 0.15;

    // Sample grid - don't check every tile, use a coarse grid
    let sample_step = 8;

    for y in (0..height).step_by(sample_step) {
        for x in (0..width).step_by(sample_step) {
            let stress = *stress_map.get(x, y);
            let elevation = *heightmap.get(x, y);

            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;

            // Hotspot noise can create volcanoes in lower stress areas
            let hotspot = hotspot_noise.get([nx * 15.0, ny * 15.0, seed_to_z(seed, 60.0)]) as f32;
            let hotspot_boost = if hotspot > 0.6 { (hotspot - 0.6) * 1.5 } else { 0.0 };

            let effective_stress = stress.abs() + hotspot_boost;

            // Skip if stress too low
            if effective_stress < MIN_STRESS {
                continue;
            }

            // Oceanic volcanoes (underwater or island arcs)
            let is_oceanic = elevation < 500.0;

            // Continental volcanoes prefer higher elevations (mountain building zones)
            let is_mountain_zone = elevation > 500.0 && stress > 0.2;

            if !is_oceanic && !is_mountain_zone && hotspot < 0.7 {
                continue;
            }

            // Score based on stress magnitude and variation
            let variation = variation_noise.get([nx * 30.0, ny * 30.0, seed_to_z(seed, 61.0)]) as f32;
            let score = effective_stress * (0.8 + variation * 0.4);

            if score > 0.25 {
                candidates.push((x, y, score));
            }
        }
    }

    // Sort by score (highest first)
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Filter to avoid volcanoes too close together
    // At world scale, volcanoes should be at least 5-10 tiles apart
    let min_distance = 8.0;
    let mut selected: Vec<(usize, usize, f32)> = Vec::new();
    let min_distance_sq = min_distance * min_distance;  // Compare squared distances (avoid sqrt)

    for (x, y, score) in candidates {
        let too_close = selected.iter().any(|(sx, sy, _)| {
            let dx = (*sx as f32 - x as f32).abs();
            let dy = (*sy as f32 - y as f32).abs();
            let dx = dx.min(width as f32 - dx);  // Handle wraparound
            dx * dx + dy * dy < min_distance_sq  // No sqrt needed
        });

        if !too_close {
            selected.push((x, y, score));
        }
    }

    // Convert to VolcanoLocation structs
    let mut rng_seed = seed;
    selected.into_iter().map(|(x, y, score)| {
        // Pseudo-random variation per volcano
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rand1 = (rng_seed >> 33) as f32 / u32::MAX as f32;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rand2 = (rng_seed >> 33) as f32 / u32::MAX as f32;

        // Peak height based on score (1000-4000m above surrounding terrain)
        let peak_height = 1000.0 + score * 3000.0 * (0.8 + rand1 * 0.4);

        // Active volcanoes based on stress and randomness
        let stress = *stress_map.get(x, y);
        let is_active = stress.abs() > 0.25 && rand2 > 0.3;

        // Volcano type based on characteristics
        let volcano_type = if rand1 < 0.2 {
            VolcanoType::Caldera
        } else if rand1 < 0.5 || stress < 0.0 {
            // Shield volcanoes more common at hotspots and divergent boundaries
            VolcanoType::Shield
        } else {
            VolcanoType::Stratovolcano
        };

        VolcanoLocation {
            x,
            y,
            peak_height,
            is_active,
            volcano_type,
        }
    }).collect()
}

/// Mark volcano tiles on the heightmap.
///
/// At world map scale (several km per tile), a volcano fits in a single tile.
/// This function applies a height boost to the volcano tile to represent
/// the volcanic mountain. The detailed volcano structure is generated
/// at region map scale.
///
/// Returns the number of volcano tiles marked.
pub fn mark_volcano_tiles(
    heightmap: &mut Tilemap<f32>,
    volcanoes: &[VolcanoLocation],
) -> usize {
    let mut tiles_modified = 0;

    for volcano in volcanoes {
        let current = *heightmap.get(volcano.x, volcano.y);

        // Add volcano peak height to existing elevation
        // This represents the volcanic mountain rising above the terrain
        let new_elevation = current + volcano.peak_height;
        heightmap.set(volcano.x, volcano.y, new_elevation);
        tiles_modified += 1;
    }

    tiles_modified
}

/// Apply volcano generation pass to the heightmap.
///
/// At world map scale, volcanoes are marked as single tiles with a height boost.
/// The detailed structure is generated at region map scale.
///
/// Returns the list of volcano locations for further processing.
pub fn apply_volcano_pass(
    heightmap: &mut Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> Vec<VolcanoLocation> {
    // Find volcano locations
    let volcanoes = find_volcano_locations(heightmap, stress_map, seed);

    if volcanoes.is_empty() {
        println!("  No suitable volcano locations found");
        return volcanoes;
    }

    let active_count = volcanoes.iter().filter(|v| v.is_active).count();
    println!("  Found {} volcanoes ({} active)", volcanoes.len(), active_count);

    // Mark volcano tiles with height boost
    let tiles_modified = mark_volcano_tiles(heightmap, &volcanoes);
    println!("  Marked {} volcano tiles", tiles_modified);

    volcanoes
}

// =============================================================================
// LAVA SYSTEM - WORLD SCALE LAVA FLOW
// =============================================================================
//
// At world map scale (several km per tile), lava flows are calculated based on:
// - Volcano tile = molten lava source
// - Adjacent downhill tiles = flowing lava (up to ~3 tiles, representing 10-30km flows)
// - Tiles at flow edge = cooled basalt

/// Lava tile state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LavaState {
    /// No lava present
    #[default]
    None,
    /// Molten lava at volcano vent
    Molten,
    /// Actively flowing lava (still hot)
    Flowing,
    /// Cooled lava (solidified basalt)
    Cooled,
}

/// Generate lava map for active volcanoes at world scale.
///
/// At world scale:
/// - Volcano tile is marked as molten lava
/// - Lava flows downhill to adjacent tiles (max 3 tiles = ~15-30km)
/// - Flow probability based on slope and noise
///
/// Returns a tilemap marking lava presence and state.
pub fn generate_lava_map(
    heightmap: &Tilemap<f32>,
    volcanoes: &[VolcanoLocation],
    seed: u64,
) -> Tilemap<LavaState> {
    use noise::{NoiseFn, Perlin, Seedable};

    let width = heightmap.width;
    let height = heightmap.height;
    let mut lava_map = Tilemap::new_with(width, height, LavaState::None);

    // Noise for variation in lava flow patterns
    let flow_noise = Perlin::new(1).set_seed((seed + 10101) as u32);

    let active_volcanoes: Vec<_> = volcanoes.iter().filter(|v| v.is_active).collect();

    if active_volcanoes.is_empty() {
        return lava_map;
    }

    // Maximum lava flow distance in tiles (represents ~15-30km of lava flow)
    const MAX_FLOW_DISTANCE: usize = 3;

    for volcano in &active_volcanoes {
        // Volcano tile is the molten source
        lava_map.set(volcano.x, volcano.y, LavaState::Molten);

        // Flow lava to adjacent tiles using BFS
        let mut flow_frontier: Vec<(usize, usize, usize)> = Vec::new();
        let volcano_height = *heightmap.get(volcano.x, volcano.y);

        // Start from volcano tile's neighbors
        let start_neighbors = get_neighbors_8(volcano.x, volcano.y, width, height);
        for (nx, ny) in start_neighbors {
            let neighbor_height = *heightmap.get(nx, ny);
            // Only flow downhill
            if neighbor_height < volcano_height {
                flow_frontier.push((nx, ny, 1));
            }
        }

        // Process flow frontier
        while let Some((x, y, dist)) = flow_frontier.pop() {
            // Skip if already has lava
            if *lava_map.get(x, y) != LavaState::None {
                continue;
            }

            // Skip if too far
            if dist > MAX_FLOW_DISTANCE {
                continue;
            }

            let current_height = *heightmap.get(x, y);

            // Skip water (ocean)
            if current_height < 0.0 {
                // Mark as cooled if it reaches water (lava hitting ocean)
                lava_map.set(x, y, LavaState::Cooled);
                continue;
            }

            // Noise check for flow variation
            let nx_f = x as f64 / width as f64;
            let ny_f = y as f64 / height as f64;
            let noise_val = flow_noise.get([nx_f * 30.0, ny_f * 30.0, seed_to_z(seed, 70.0)]) as f32;

            // Flow probability decreases with distance
            let flow_prob = 1.0 - (dist as f32 / (MAX_FLOW_DISTANCE as f32 + 1.0));

            // Skip some tiles based on noise and distance for natural variation
            if noise_val > flow_prob * 1.5 - 0.5 {
                continue;
            }

            // Mark as flowing lava (near volcano) or cooled (at edge)
            let state = if dist <= 2 {
                LavaState::Flowing
            } else {
                LavaState::Cooled
            };
            lava_map.set(x, y, state);

            // Continue flowing to neighbors if not at max distance
            if dist < MAX_FLOW_DISTANCE {
                let neighbors = get_neighbors_8(x, y, width, height);
                for (nx, ny) in neighbors {
                    let neighbor_height = *heightmap.get(nx, ny);
                    // Only flow downhill or same level
                    if neighbor_height <= current_height + 50.0 {
                        flow_frontier.push((nx, ny, dist + 1));
                    }
                }
            }
        }
    }

    // Count lava tiles (single pass instead of 3 separate iterations)
    let (mut molten, mut flowing, mut cooled) = (0usize, 0usize, 0usize);
    for (_, _, state) in lava_map.iter() {
        match state {
            LavaState::Molten => molten += 1,
            LavaState::Flowing => flowing += 1,
            LavaState::Cooled => cooled += 1,
            LavaState::None => {}
        }
    }

    if molten + flowing + cooled > 0 {
        println!("  Lava: {} molten, {} flowing, {} cooled tiles", molten, flowing, cooled);
    }

    lava_map
}

/// Get 8-connected neighbors (including diagonals)
fn get_neighbors_8(x: usize, y: usize, width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut neighbors = Vec::with_capacity(8);
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = (x as isize + dx).rem_euclid(width as isize) as usize;
            let ny = (y as isize + dy).rem_euclid(height as isize) as usize;
            neighbors.push((nx, ny));
        }
    }
    neighbors
}
