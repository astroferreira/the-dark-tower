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

/// Convert 2D map coordinates (normalized nx in [0, 1], ny in [0, 1]) to 3D cylindrical coordinates
/// to guarantee seamless periodic wrapping across longitude (X) without grid-plane resonance.
#[inline]
fn cylindrical_coords(nx: f64, ny: f64, radius: f64) -> [f64; 3] {
    let angle = nx * std::f64::consts::TAU;
    let cx = angle.cos() * radius;
    let cz = angle.sin() * radius;
    let cy = (ny - 0.5) * radius * 2.0;

    // Rotate slightly so equator and parallels never align with Cartesian integer lattice planes
    let cos_a = 0.93969; // cos(20 deg)
    let sin_a = 0.34202; // sin(20 deg)
    let rx = cx;
    let ry = cy * cos_a - cz * sin_a + 17.382;
    let rz = cy * sin_a + cz * cos_a + 31.914;
    [rx + 11.234, ry, rz]
}

/// Fractional Brownian Motion in 3D
fn fbm_3d(
    noise: &Perlin,
    p: [f64; 3],
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += amplitude * noise.get([p[0] * frequency, p[1] * frequency, p[2] * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

/// Multi-octave Ridged Multifractal noise in 3D (sharp alpine crests, arêtes, and cordilleras)
fn ridged_fbm_3d(
    noise: &Perlin,
    p: [f64; 3],
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
    power: f64,
) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_val = 0.0;

    for i in 0..octaves {
        let n = noise.get([
            p[0] * frequency,
            p[1] * frequency,
            p[2] * frequency + i as f64 * 30.0,
        ]);
        let ridge = (1.0 - n.abs()).max(0.0).powf(power);
        total += amplitude * ridge;
        max_val += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    (total / max_val) as f32
}

/// Domain warping in 3D
fn domain_warp_cylindrical(
    p: [f64; 3],
    warp_noise: &Perlin,
    warp_strength: f64,
    warp_frequency: f64,
) -> [f64; 3] {
    let wx = warp_noise.get([p[0] * warp_frequency, p[1] * warp_frequency, p[2] * warp_frequency]);
    let wy = warp_noise.get([
        p[0] * warp_frequency + 100.0,
        p[1] * warp_frequency + 100.0,
        p[2] * warp_frequency + 100.0,
    ]);
    let wz = warp_noise.get([
        p[0] * warp_frequency + 200.0,
        p[1] * warp_frequency + 200.0,
        p[2] * warp_frequency + 200.0,
    ]);

    [
        p[0] + wx * warp_strength,
        p[1] + wy * warp_strength,
        p[2] + wz * warp_strength,
    ]
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
const COASTAL_HEIGHT: f32 = 8.0;         // Beach level
const SHELF_DEPTH: f32 = -150.0;         // Continental shelf

// Coastal beach shaping parameters
const BEACH_WIDTH_KM: f32 = 8.0;         // Target beach/shore strip width in km
const BEACH_MAX_RISE: f32 = 120.0;       // Max elevation at inner beach edge (m)
const BEACH_CAP_EXPONENT: f32 = 1.5;     // Curve for beach rise inland
const BEACH_CLIFF_STRESS_MIN: f32 = 0.25; // Start preserving cliffs at this stress
const BEACH_CLIFF_STRESS_MAX: f32 = 0.45; // Full cliff preservation at this stress
const BEACH_WATER_WIDTH_KM: f32 = 12.0;  // Nearshore shallow water width in km
const BEACH_WATER_SHORE_DEPTH: f32 = -3.0; // Depth right at shore (m)
const BEACH_WATER_OUTER_DEPTH: f32 = -80.0; // Depth at outer edge of nearshore shelf (m)

// Inland relief uplift parameters
const INLAND_UPLIFT_DISTANCE_KM: f32 = 300.0; // Distance inland where uplift reaches full strength
const INLAND_UPLIFT_BASE: f32 = 300.0;        // Base uplift at interior (m)
const INLAND_UPLIFT_STRESS: f32 = 700.0;      // Stress-weighted uplift at interior (m)
const INLAND_UPLIFT_EXPONENT: f32 = 1.2;      // Curve for inland uplift ramp

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
    let coast_noise = Perlin::new(1).set_seed(seed as u32 + 4444);
    
    // Pre-compute continental distance field
    let continental_distance = compute_continental_distance(plate_map, plates);
    
    // Pre-compute distance from coast for gradient
    let coast_distance = compute_coast_distance(plate_map, plates);
    
    let mut heightmap = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let plate_id = *plate_map.get(x, y);
            if plate_id.is_none() || (plate_id.0 as usize) >= plates.len() {
                heightmap.set(x, y, OCEAN_FLOOR);
                continue;
            }
            
            let plate = &plates[plate_id.0 as usize];
            let stress = *stress_map.get(x, y);
            let cont_dist = *continental_distance.get(x, y);
            let raw_coast_dist = *coast_distance.get(x, y);
            
            let nx = x as f64 / width as f64;
            let ny = y as f64 / height as f64;
            
            let p = cylindrical_coords(nx, ny, 1.0);
            let wp_macro = domain_warp_cylindrical(p, &warp_noise, 0.75, 0.85);
            let wp_meso = domain_warp_cylindrical(wp_macro, &detail_noise, 0.35, 2.4);

            // 1. Macro-scale continental craton morphology (deep oceanic indentations, sweeping arcs)
            let craton_macro = fbm_3d(&terrain_noise, [wp_macro[0] * 1.1, wp_macro[1] * 1.1, wp_macro[2] * 1.1], 5, 0.55, 2.0);
            
            // 2. Meso-scale peninsulas, capes, and horns
            let lobes_meso = fbm_3d(&detail_noise, [wp_meso[0] * 3.0, wp_meso[1] * 3.0, wp_meso[2] * 3.0], 4, 0.52, 2.0);

            // 3. Micro-scale coastal fractal rias, headlands, and coves
            let coast_fractal = fbm_3d(&coast_noise, [p[0] * 14.0, p[1] * 14.0, p[2] * 14.0], 4, 0.60, 2.0);

            // Base effective coast before geological modifiers
            let mut effective_coast = raw_coast_dist 
                + (craton_macro as f32 * 80.0) 
                + (lobes_meso as f32 * 42.0) 
                + (coast_fractal as f32 * 16.0);

            // 4. Inland seas, continental rifting, and Mediterranean/Baltic/Hudson Bay style embayments
            let rift_basin = fbm_3d(&ridge_noise, [wp_macro[0] * 1.8, wp_macro[1] * 1.8, wp_macro[2] * 1.8], 3, 0.5, 2.0);
            if rift_basin < -0.30 && raw_coast_dist > 15.0 {
                let basin_cut = ((-rift_basin - 0.30) / 0.50).min(1.2) * 85.0;
                effective_coast -= basin_cut as f32;
            }

            // Continental rifting: divergent stress on land pulls crust apart into flooded rift channels
            if stress < -0.04 && raw_coast_dist > -40.0 {
                let rift = ((-stress - 0.04) / 0.35).min(1.2) * 65.0;
                effective_coast -= rift;
            }

            // Orogenic uplift & peninsular extrusion on active collision margins
            if stress > 0.04 {
                let uplift = ((stress - 0.04) / 0.35).min(1.2) * 35.0;
                effective_coast += uplift;
            }

            // Polar ocean taper: ensure all continents are completely surrounded by water (polar oceans)
            // and never cut off by northern or southern map borders with organic fractal shorelines.
            let pole_noise = fbm_3d(&coast_noise, [wp_macro[0] * 4.0, wp_macro[1] * 4.0, wp_macro[2] * 4.0], 3, 0.5, 2.0) as f32 * 0.05;
            let pole_dist = ((ny.min(1.0 - ny)) as f32 + pole_noise).max(0.0);
            let polar_margin = 0.12f32;
            if pole_dist < polar_margin {
                let t = (pole_dist / polar_margin).clamp(0.0, 1.0);
                let smooth_t = t * t * (3.0 - 2.0 * t);
                effective_coast = effective_coast * smooth_t - (1.0 - smooth_t) * 55.0;
            }

            let elevation = if effective_coast > 0.0 {
                generate_continental_elevation(
                    nx, ny,
                    wp_macro,
                    effective_coast,
                    stress,
                    &terrain_noise,
                    &ridge_noise,
                    &detail_noise,
                    &params,
                    seed,
                    map_scale,
                )
            } else {
                let stress_dx = if x > 0 && x < width - 1 {
                    *stress_map.get(x + 1, y) - *stress_map.get(x - 1, y)
                } else { 0.0 };
                let stress_dy = if y > 0 && y < height - 1 {
                    *stress_map.get(x, y + 1) - *stress_map.get(x, y - 1)
                } else { 0.0 };
                let stress_gradient = (stress_dx, stress_dy);

                let is_continental_plate = plate.plate_type == PlateType::Continental;

                generate_oceanic_elevation(
                    nx, ny,
                    p,
                    is_continental_plate,
                    effective_coast,
                    cont_dist,
                    stress,
                    stress_gradient,
                    &terrain_noise,
                    &detail_noise,
                    &coast_noise,
                    &params,
                    seed,
                    map_scale,
                )
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

/// Generate elevation for continental plates, including Andean-style coastal cordilleras,
/// collision mountain belts (Himalayas), plateaus (Altiplano), ancient fold belts, and rift valleys.
fn generate_continental_elevation(
    nx: f64,
    ny: f64,
    p: [f64; 3],
    effective_coast: f32,
    stress: f32,
    terrain_noise: &Perlin,
    ridge_noise: &Perlin,
    detail_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    let coastal_grad_dist = scale_distance(100.0, map_scale);
    let distance_factor = (effective_coast / coastal_grad_dist).clamp(0.0, 1.0);
    let coastal_gradient = smooth_step(0.0, 1.0, distance_factor);

    // 1. Base Continental Lowlands, Plains, Prairies & Sedimentary Basins
    // Low-frequency, gentle rolling relief for vast flat plains and open basins
    let plains_fbm = fbm_3d(terrain_noise, [p[0] * 1.5, p[1] * 1.5, p[2] * 1.5], 4, 0.50, 2.0) as f32;
    // Flatten the lowlands using power curve to create expansive flat plains (pampas, steppes, prairies)
    let plains_norm = (plains_fbm * 0.5 + 0.5).clamp(0.0, 1.0);
    let gentle_plains = plains_norm.powf(1.6) * scale_elevation(220.0, map_scale);

    // 2. Structural Orogenic Belts & Ancient Fold Corridors
    // Mountain ranges exist along discrete tectonic corridors, leaving the rest of the continent as open plains and broad river valleys.
    let orogen_corridor = fbm_3d(ridge_noise, [p[0] * 1.3, p[1] * 1.3, p[2] * 1.3], 3, 0.5, 2.0) as f32;
    let mountain_belt_mask = smooth_step(0.08, 0.48, orogen_corridor);

    // Alpine Mountain Ridges within the mountain corridors
    let alpine_spines = ridged_fbm_3d(ridge_noise, [p[0] * 4.5, p[1] * 4.5, p[2] * 4.5], 5, 0.52, 2.0, 2.0);
    let alpine_detail = ridged_fbm_3d(detail_noise, [p[0] * 9.0, p[1] * 9.0, p[2] * 9.0], 3, 0.5, 2.0, 1.8);
    let mountain_ridge = alpine_spines * 0.70 + alpine_detail * 0.30;

    // Interior mountain ranges (only in orogenic belt corridors)
    let interior_mountains = mountain_ridge * mountain_belt_mask * scale_elevation(2200.0, map_scale) * (0.20 + coastal_gradient * 0.80);

    // Foothills / rolling hills transitioning from mountain ranges to plains
    let foothills_noise = fbm_3d(detail_noise, [p[0] * 3.5, p[1] * 3.5, p[2] * 3.5], 3, 0.5, 2.0) as f32;
    let foothills = (foothills_noise * 0.5 + 0.5) * mountain_belt_mask * scale_elevation(380.0, map_scale);

    // Secondary Ancient Highlands & Rolling Uplands (e.g. Appalachians, Brazilian Highlands, Massif Central)
    let highland_fbm = fbm_3d(terrain_noise, [p[0] * 2.2 + 37.0, p[1] * 2.2 + 19.0, p[2] * 2.2], 3, 0.5, 2.0) as f32;
    let highland_mask = smooth_step(0.12, 0.46, highland_fbm);
    let highland_hills = fbm_3d(detail_noise, [p[0] * 5.0, p[1] * 5.0, p[2] * 5.0], 3, 0.5, 2.0) as f32 * 0.5 + 0.5;
    let uplands = highland_mask * highland_hills * scale_elevation(360.0, map_scale) * coastal_gradient;

    // 3. Active Tectonic Orogeny (Andes-style Coastal Cordilleras & Collision Belts)
    let tectonic_orogeny = if stress > 0.04 {
        let stress_factor = ((stress - 0.04) / 0.22).min(1.6);
        let cordillera_base = stress_factor.powf(0.8) * scale_elevation(2800.0, map_scale);
        let cordillera_peaks = mountain_ridge.powf(1.2) * stress_factor * scale_elevation(3400.0, map_scale);
        cordillera_base * 0.35 + cordillera_peaks
    } else if stress < -0.05 {
        // Continental Rifting
        let rift_strength = (-stress - 0.05).min(0.5);
        let rift_depth = rift_strength * scale_elevation(1200.0, map_scale);
        let rift_trough = (1.0 - mountain_ridge).max(0.0);
        -rift_depth * rift_trough
    } else {
        0.0
    };

    // 4. Subtle micro-relief for plains (very small, smooth, natural)
    let fine_noise = fbm_3d(detail_noise, [p[0] * 12.0, p[1] * 12.0, p[2] * 12.0], 2, 0.5, 2.0) as f32;
    let fine_relief = fine_noise * scale_elevation(30.0, map_scale);

    let min_elevation = COASTAL_HEIGHT + CONTINENTAL_MIN * coastal_gradient;

    (min_elevation + gentle_plains + uplands + foothills + interior_mountains + tectonic_orogeny + fine_relief).max(COASTAL_HEIGHT)
}

// =============================================================================
// OCEANIC TERRAIN & ISLAND SYSTEMS
// =============================================================================

/// Generate elevation for oceanic and shelf areas, including the 4 geological island systems:
/// 1. Coastal shelf archipelagos & rias
/// 2. Barrier islands
/// 3. Subduction volcanic island arcs
/// 4. Mantle plume hotspot chains
fn generate_oceanic_elevation(
    nx: f64,
    ny: f64,
    p: [f64; 3],
    is_continental_plate: bool,
    effective_coast: f32, // negative: 0.0 at shoreline to -300+ in deep ocean
    continental_distance: f32,
    stress: f32,
    stress_gradient: (f32, f32),
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    coast_noise: &Perlin,
    params: &TerrainParams,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    let shelf_blend_dist = scale_distance(18.0, map_scale);
    let shelf_noise_height = scale_elevation(35.0, map_scale);

    // If this is on a continental plate (inland lake, flooded rift basin, or drowned valley),
    // keep it as shallow continental freshwater/fjord depth, never deep oceanic abyss or oceanic volcanoes.
    if is_continental_plate {
        let lake_depth = (effective_coast * 1.5).clamp(-60.0, -2.0);
        let detail = fbm_3d(terrain_noise, [p[0] * 4.0, p[1] * 4.0, p[2] * 4.0], 3, 0.5, 2.0) as f32;
        return lake_depth + detail * 8.0;
    }

    // Base ocean bathymetry (shelf near coast, abyssal plain + ridges + trenches in deep water)
    let base_ocean = if effective_coast >= -shelf_blend_dist {
        let shelf_blend = (-effective_coast / shelf_blend_dist).clamp(0.0, 1.0);
        let base_shelf = SHELF_DEPTH * shelf_blend;

        // Nearshore Coastal Archipelagos & drowned rias (smooth, moderate frequency)
        let arch_noise = fbm_3d(coast_noise, [p[0] * 4.2, p[1] * 4.2, p[2] * 4.2], 4, 0.55, 2.0) as f32;
        let detail_arch = fbm_3d(detail_noise, [p[0] * 8.0, p[1] * 8.0, p[2] * 8.0], 3, 0.5, 2.0) as f32;
        let island_potential = arch_noise * 0.70 + detail_arch * 0.30;

        // Drowned continental margin topography creates shelf islands with natural coastal straits
        let shelf_topography = base_shelf + (island_potential + 0.15) * scale_elevation(260.0, map_scale);

        if shelf_topography > 0.0 {
            shelf_topography
        } else {
            let shelf_detail = fbm_3d(terrain_noise, [p[0] * 4.0, p[1] * 4.0, p[2] * 4.0], 3, 0.5, 2.0) as f32;
            base_shelf + shelf_detail * shelf_noise_height
        }
    } else {
        // Deep Ocean Zone
        let ocean_variation = scale_elevation(1000.0, map_scale);
        let abyssal_hills = fbm_3d(terrain_noise, [p[0] * 5.0, p[1] * 5.0, p[2] * 5.0], 3, 0.5, 2.0) as f32 * ocean_variation;
        let ocean_floor = OCEAN_FLOOR + abyssal_hills;

        // Mid-Ocean Spreading Ridges (divergent oceanic boundaries)
        let ridge_contribution = if stress < -0.06 {
            let ridge_strength = (-stress - 0.06).min(0.5) / 0.5;
            let base_lift = (OCEAN_RIDGE - OCEAN_FLOOR) * ridge_strength;
            let axial_noise = fbm_3d(detail_noise, [p[0] * 10.0, p[1] * 10.0, p[2] * 10.0], 3, 0.5, 2.0) as f32;
            let axial_rift = if axial_noise.abs() < 0.15 { scale_elevation(250.0, map_scale) } else { 0.0 };
            base_lift - axial_rift * ridge_strength
        } else {
            0.0
        };

        // Subduction Trenches (convergent oceanic boundaries)
        let trench_contribution = if stress > 0.12 {
            let trench_strength = ((stress - 0.12) / 0.45).min(1.0);
            -trench_strength * scale_elevation(TRENCH_SCALE, map_scale)
        } else {
            0.0
        };

        let deep_ocean = ocean_floor + ridge_contribution + trench_contribution;
        // Smooth transition from continental shelf edge to deep ocean floor
        let deep_blend_dist = scale_distance(25.0, map_scale);
        let dist_beyond_shelf = -effective_coast - shelf_blend_dist;
        let t = (dist_beyond_shelf / deep_blend_dist).clamp(0.0, 1.0);
        SHELF_DEPTH * (1.0 - t) + deep_ocean * t
    };

    // Volcanic Island Arcs (subduction zones in ocean)
    let volcanic_elevation = if stress > VOLCANIC_THRESHOLD {
        generate_island_arc_3d(p, stress, stress_gradient, terrain_noise, detail_noise, seed, map_scale)
    } else {
        f32::MIN
    };

    // Hotspot Volcanic Chains (mantle plumes in deep ocean)
    let hotspot_elevation = generate_hotspot_archipelago_3d(p, continental_distance, terrain_noise, detail_noise, seed, map_scale);

    let raw_ocean = base_ocean.max(volcanic_elevation).max(hotspot_elevation);

    // Polar ocean taper: ensures no islands or land touch the extreme polar map borders
    let pole_noise = fbm_3d(detail_noise, [p[0] * 4.0, p[1] * 4.0, p[2] * 4.0], 3, 0.5, 2.0) as f32 * 0.03;
    let pole_dist = ((ny.min(1.0 - ny)) as f32 + pole_noise).max(0.0);
    let polar_margin = 0.06f32;
    if pole_dist < polar_margin && raw_ocean > 0.0 {
        let t = (pole_dist / polar_margin).clamp(0.0, 1.0);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        raw_ocean * smooth_t - (1.0 - smooth_t) * 150.0
    } else {
        raw_ocean
    }
}

/// Generate volcanic island arcs at subduction zones
fn generate_island_arc_3d(
    p: [f64; 3],
    stress: f32,
    stress_gradient: (f32, f32),
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    if stress < 0.035 {
        return f32::MIN;
    }

    let grad_mag = (stress_gradient.0 * stress_gradient.0 + stress_gradient.1 * stress_gradient.1).sqrt();
    if grad_mag < 0.001 {
        return f32::MIN;
    }

    let stress_factor = ((stress - 0.035) / 0.18).clamp(0.0, 1.0);

    // Submarine volcanic arc platform (lifts from deep ocean to shallow submarine ridge)
    let arc_platform = scale_elevation(-220.0, map_scale) + scale_elevation(160.0, map_scale) * stress_factor.powf(0.7);

    // Volcanic centers along the curving arc
    let arc_chain = fbm_3d(terrain_noise, [p[0] * 4.5, p[1] * 4.5, p[2] * 4.5], 3, 0.55, 2.0) as f32;
    let volcanic_cones = fbm_3d(detail_noise, [p[0] * 8.5, p[1] * 8.5, p[2] * 8.5], 3, 0.5, 2.0) as f32;

    let combo = arc_chain * 0.65 + volcanic_cones * 0.35;

    // Volcanic edifices rise on top of the arc platform
    let edifice_height = (combo + 0.12).max(0.0) * scale_elevation(650.0, map_scale) * (0.4 + stress_factor * 0.6);

    arc_platform + edifice_height
}

/// Generate hotspot island chains (like Hawaii) formed by stationary mantle plumes
fn generate_hotspot_archipelago_3d(
    p: [f64; 3],
    continental_distance: f32,
    terrain_noise: &Perlin,
    detail_noise: &Perlin,
    seed: u64,
    map_scale: &MapScale,
) -> f32 {
    // Only in deep ocean far from continents
    if continental_distance < scale_distance(14.0, map_scale) {
        return f32::MIN;
    }

    // Mantle plume swell regions: broad regions in deep ocean
    let plume_noise = fbm_3d(terrain_noise, [p[0] * 2.2 + 50.0, p[1] * 2.2 + 50.0, p[2] * 2.2 + 50.0], 3, 0.5, 2.0) as f32;
    if plume_noise < 0.10 {
        return f32::MIN;
    }
    let plume_strength = ((plume_noise - 0.10) / 0.70).min(1.0);

    // Directional hotspot chain / track (aligned linear volcanic trail)
    let track_noise = fbm_3d(terrain_noise, [p[0] * 4.5 + 120.0, p[1] * 2.2 + 120.0, p[2] * 4.5 + 120.0], 3, 0.5, 2.0) as f32;
    let track_dist = track_noise.abs();
    if track_dist > 0.28 {
        return f32::MIN;
    }
    let track_factor = (1.0 - (track_dist / 0.28)).powf(1.2);

    // Discrete volcanic shield centers along the track
    let shield_noise = fbm_3d(detail_noise, [p[0] * 6.5 + 200.0, p[1] * 6.5 + 200.0, p[2] * 6.5 + 200.0], 3, 0.5, 2.0) as f32;
    let cone_noise = fbm_3d(detail_noise, [p[0] * 12.0 + 300.0, p[1] * 12.0 + 300.0, p[2] * 12.0 + 300.0], 2, 0.5, 2.0) as f32;
    let shield_combo = shield_noise * 0.75 + cone_noise * 0.25;

    // Plume swell elevates deep ocean floor to a broad submarine plateau
    let swell_base = scale_elevation(-350.0, map_scale) + scale_elevation(250.0, map_scale) * plume_strength * track_factor;

    // Volcanic shield edifice rises above sea level
    let shield_height = (shield_combo + 0.10).max(0.0) * scale_elevation(750.0, map_scale) * track_factor;

    let total_elev = swell_base + shield_height;
    if total_elev > OCEAN_FLOOR + 500.0 {
        total_elev
    } else {
        f32::MIN
    }
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

/// Apply edge-preserving bilateral smoothing to reduce harsh transitions while keeping mountain peaks sharp
fn smooth_heightmap(heightmap: &Tilemap<f32>, radius: usize) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut result = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let center_h = *heightmap.get(x, y);
            // Alpine terrain: preserve crisp mountain peaks, arêtes, and deep valleys
            if center_h > 180.0 {
                result.set(x, y, center_h);
                continue;
            }

            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            
            for dy in -(radius as i32)..=(radius as i32) {
                for dx in -(radius as i32)..=(radius as i32) {
                    let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    
                    let sample_h = *heightmap.get(nx, ny);
                    let h_diff = (sample_h - center_h).abs();
                    // Don't blur across steep coastal cliffs or fjord walls
                    if h_diff > 60.0 {
                        continue;
                    }

                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist <= radius as f32 {
                        let weight = (1.0 - dist / (radius as f32 + 1.0)) / (1.0 + h_diff * 0.05);
                        sum += sample_h * weight;
                        count += weight;
                    }
                }
            }
            
            result.set(x, y, if count > 0.0 { sum / count } else { center_h });
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
    seed: u64,
) -> Tilemap<bool> {
    let width = plate_map.width;
    let height = plate_map.height;
    let dummy_stress = Tilemap::new_with(width, height, 0.0f32);
    let heightmap = generate_heightmap(plate_map, plates, &dummy_stress, seed);
    let mut land_mask = Tilemap::new_with(width, height, false);

    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) > 0.0 {
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

                // Shoreline preservation: fade noise smoothly near sea level and preserve land/ocean sign
                let coast_fade = (elevation.abs() / 20.0).min(1.0);
                let scaled_noise = noise_contribution * coast_fade;
                let new_elevation = if elevation > 0.0 {
                    (elevation + scaled_noise).max(0.1)
                } else {
                    (elevation + scaled_noise).min(-0.1)
                };

                row_contributions.push((x, y, new_elevation));
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
    _heightmap: &mut Tilemap<f32>,
    _stress_map: &Tilemap<f32>,
    _seed: u64,
) {
    // Islands are synthesized physically and continuously during initial heightmap synthesis
    // (in generate_oceanic_elevation) to maintain natural seamount bathymetry and prevent
    // artificial post-processing square water box artifacts.
}

/// Expand small islands into larger clusters (deprecated: islands are continuously generated).
pub fn expand_island_clusters(
    _heightmap: &mut Tilemap<f32>,
    _stress_map: &Tilemap<f32>,
    _seed: u64,
) {
    // Deprecated: natural multi-tile islands are synthesized organically in generate_oceanic_elevation.
}

// =============================================================================
// FJORD INCISION SYSTEM
// =============================================================================

/// Mark ocean water (water connected to the map edges).
fn compute_ocean_mask(heightmap: &Tilemap<f32>) -> Tilemap<bool> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;

    let mut is_ocean = Tilemap::new_with(width, height, false);
    let mut queue: VecDeque<(usize, usize)> = VecDeque::with_capacity(width * 2);

    let mut push_if_water = |x: usize, y: usize, queue: &mut VecDeque<(usize, usize)>| {
        if *heightmap.get(x, y) < 0.0 && !*is_ocean.get(x, y) {
            is_ocean.set(x, y, true);
            queue.push_back((x, y));
        }
    };

    // Seed from boundary water tiles
    for x in 0..width {
        push_if_water(x, 0, &mut queue);
        push_if_water(x, height - 1, &mut queue);
    }
    for y in 0..height {
        push_if_water(0, y, &mut queue);
        push_if_water(width - 1, y, &mut queue);
    }

    // If no boundary water, treat all water as ocean (closed basins)
    if queue.is_empty() {
        for y in 0..height {
            for x in 0..width {
                push_if_water(x, y, &mut queue);
            }
        }
    }

    // Flood-fill ocean water
    while let Some((x, y)) = queue.pop_front() {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                if *heightmap.get(nx, ny) < 0.0 && !*is_ocean.get(nx, ny) {
                    is_ocean.set(nx, ny, true);
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    is_ocean
}

/// Compute distance from each cell to nearest ocean water using BFS.
/// Ocean water is any water connected to the map edges.
fn compute_distance_to_ocean(heightmap: &Tilemap<f32>, max_dist: f32) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;

    let is_ocean = compute_ocean_mask(heightmap);
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize)> = VecDeque::with_capacity(width * height / 4);

    for y in 0..height {
        for x in 0..width {
            if *is_ocean.get(x, y) {
                distance.set(x, y, 0.0);
                queue.push_back((x, y));
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        let current_dist = *distance.get(x, y);
        if current_dist >= max_dist {
            continue;
        }

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
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

/// Compute distance from each cell to nearest land using BFS.
fn compute_distance_to_land(heightmap: &Tilemap<f32>, max_dist: f32) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;
    let mut distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize)> = VecDeque::with_capacity(width * height / 4);

    // Initialize: all land tiles have distance 0
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) > 0.0 {
                distance.set(x, y, 0.0);
                queue.push_back((x, y));
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        let current_dist = *distance.get(x, y);
        if current_dist >= max_dist {
            continue;
        }

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
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

/// Apply an inland uplift pass to restore macro relief away from coastlines.
/// Uses distance to ocean and tectonic stress to scale uplift.
pub fn apply_inland_uplift(
    heightmap: &mut Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    map_scale: &MapScale,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    let max_dist = scale_distance(INLAND_UPLIFT_DISTANCE_KM, map_scale).max(1.0);
    let base_uplift = scale_elevation(INLAND_UPLIFT_BASE, map_scale);
    let stress_uplift = scale_elevation(INLAND_UPLIFT_STRESS, map_scale);

    let ocean_distance = compute_distance_to_ocean(heightmap, max_dist + 2.0);

    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            if elevation <= 0.0 {
                continue;
            }

            let dist = *ocean_distance.get(x, y);
            if dist <= 0.0 {
                continue;
            }

            let t = (dist / max_dist).clamp(0.0, 1.0);
            let inland = smooth_step(0.0, 1.0, t).powf(INLAND_UPLIFT_EXPONENT);

            let stress = (*stress_map.get(x, y)).max(0.0);
            let uplift = base_uplift * inland + stress_uplift * inland * stress.powf(0.6);
            heightmap.set(x, y, elevation + uplift);
        }
    }
}

/// Apply a coastal beach/shore strip near sea level for most coastlines.
/// High-stress convergent coastlines can preserve steep cliffs.
pub fn apply_coastal_beaches(
    heightmap: &mut Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    map_scale: &MapScale,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    let beach_width = scale_distance(BEACH_WIDTH_KM, map_scale).max(1.0);
    let beach_water_width = scale_distance(BEACH_WATER_WIDTH_KM, map_scale).max(1.0);
    let beach_rise = scale_elevation(BEACH_MAX_RISE, map_scale);
    let beach_level = scale_elevation(COASTAL_HEIGHT, map_scale);
    let shore_depth = scale_elevation(BEACH_WATER_SHORE_DEPTH, map_scale);
    let outer_depth = scale_elevation(BEACH_WATER_OUTER_DEPTH, map_scale);

    let ocean_distance = compute_distance_to_ocean(heightmap, beach_width + 2.0);
    let land_distance = compute_distance_to_land(heightmap, beach_water_width + 2.0);
    let ocean_mask = compute_ocean_mask(heightmap);

    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);

            if elevation > 0.0 {
                let dist = *ocean_distance.get(x, y);
                if dist <= 0.0 || dist > beach_width {
                    continue;
                }

                let t = (dist / beach_width).clamp(0.0, 1.0);
                let cap = beach_level + beach_rise * t.powf(BEACH_CAP_EXPONENT);

                if elevation <= cap {
                    continue;
                }

                let stress = *stress_map.get(x, y);
                let cliff_factor = if stress > BEACH_CLIFF_STRESS_MIN {
                    smooth_step(BEACH_CLIFF_STRESS_MIN, BEACH_CLIFF_STRESS_MAX, stress)
                } else {
                    0.0
                };

                let adjusted = cap + (elevation - cap) * cliff_factor;
                heightmap.set(x, y, adjusted);
            } else if *ocean_mask.get(x, y) {
                let water_dist = *land_distance.get(x, y);
                if water_dist <= 0.0 || water_dist > beach_water_width {
                    continue;
                }

                let t = (water_dist / beach_water_width).clamp(0.0, 1.0);
                let target_depth = shore_depth + (outer_depth - shore_depth) * t.powf(1.2);

                if elevation < target_depth {
                    heightmap.set(x, y, target_depth);
                }
            }
        }
    }
}

/// Apply fjord-like channel incisions to coastal terrain
/// Creates features like Chilean Patagonian fjords, Norwegian fjords, Scottish sea lochs
pub fn apply_fjord_incisions(
    heightmap: &mut Tilemap<f32>,
    seed: u64,
    map_scale: &MapScale,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    let channel_noise = Perlin::new(1).set_seed((seed + 8001) as u32);
    let warp_noise = Perlin::new(1).set_seed((seed + 8002) as u32);
    let detail_noise = Perlin::new(1).set_seed((seed + 8003) as u32);

    let check_radius = 24.0f32;
    let water_distance = compute_distance_to_water(heightmap, check_radius + 1.0);

    let mut fjords_carved = 0;

    for y in 0..height {
        let ny = y as f64 / height as f64;
        // Glacial latitudes: Patagonian/Chilean south (> 38°S, ny > 0.71) or Norwegian/Alaskan north (> 38°N, ny < 0.29)
        let lat_deg = ((ny - 0.5).abs() * 180.0) as f32;
        let is_glacial_latitude = lat_deg > 36.0;

        for x in 0..width {
            let elevation = *heightmap.get(x, y);

            // Only carve land within reasonable elevation range
            if elevation <= 0.0 || elevation > scale_elevation(3000.0, map_scale) {
                continue;
            }

            let water_dist = *water_distance.get(x, y);
            if water_dist > check_radius || water_dist == 0.0 {
                continue;
            }

            // High mountains near coast also qualify for alpine glacial fjords even at mid-latitudes
            let is_coastal_mountain = elevation > scale_elevation(600.0, map_scale);
            if !is_glacial_latitude && !is_coastal_mountain {
                continue;
            }

            let nx = x as f64 / width as f64;
            let p = cylindrical_coords(nx, ny, 1.0);
            let wp = domain_warp_cylindrical(p, &warp_noise, 0.40, 3.5);

            // Fjord zone noise: selects fjord districts (like Western Norway or Chilean Patagonia)
            let fjord_zone = fbm_3d(&channel_noise, [wp[0] * 3.0, wp[1] * 3.0, wp[2] * 3.0], 3, 0.5, 2.0);
            if fjord_zone < 0.05 {
                continue;
            }
            let zone_strength = ((fjord_zone - 0.05) / 0.65).clamp(0.0, 1.0) as f32;

            // Fjord valley network: ridged multifractal troughs cutting perpendicular/oblique to coast
            let trough = ridged_fbm_3d(&channel_noise, [wp[0] * 12.0, wp[1] * 6.5, wp[2] * 12.0], 3, 0.5, 2.0, 1.6);
            let cross_trough = ridged_fbm_3d(&detail_noise, [wp[0] * 6.0, wp[1] * 12.0, wp[2] * 6.0], 2, 0.5, 2.0, 1.6);
            let fjord_network = trough.max(cross_trough * 0.7);

            // Fjord threshold: creates narrow, deep U-shaped valleys
            if fjord_network < 0.62 {
                continue;
            }

            let channel_strength = ((fjord_network - 0.62) / 0.38).powf(1.4);
            let dist_factor = (1.0 - water_dist / check_radius).powf(0.8);

            // Max incision depth: up to 650m incision
            let max_depth = scale_elevation(650.0, map_scale);
            let incision = max_depth * zone_strength * channel_strength * dist_factor;

            if incision < 15.0 {
                continue;
            }

            let new_elev = elevation - incision;
            // If the fjord gouges below sea level, it becomes a saltwater fjord channel (-15m to -120m)
            let final_elev = if new_elev < 0.0 {
                (new_elev * 0.4).clamp(-120.0, -12.0)
            } else {
                new_elev
            };

            heightmap.set(x, y, final_elev);
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
#[derive(serde::Serialize, serde::Deserialize)]
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
#[derive(serde::Serialize, serde::Deserialize)]
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
    let width = heightmap.width;
    let height = heightmap.height;
    let mut tiles_modified = 0;

    for volcano in volcanoes {
        let base_peak = volcano.peak_height;
        let radius = 3.0f32;

        for dy in -3i32..=3 {
            let ny = volcano.y as i32 + dy;
            if ny < 0 || ny >= height as i32 {
                continue;
            }
            let uy = ny as usize;

            for dx in -3i32..=3 {
                let r = ((dx * dx + dy * dy) as f32).sqrt();
                if r > radius {
                    continue;
                }

                let ux = ((volcano.x as i32 + dx).rem_euclid(width as i32)) as usize;
                let current = *heightmap.get(ux, uy);

                // Conical / Gaussian volcanic profile
                let cone_profile = (-(r / 1.5).powi(2)).exp();
                let boost = base_peak * cone_profile;

                heightmap.set(ux, uy, current + boost);
                tiles_modified += 1;
            }
        }
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
#[derive(serde::Serialize, serde::Deserialize)]
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
