# Erosion Systems

Comprehensive documentation of the procedural world generator's terrain erosion systems.

---

## Table of Contents

1. [Overview](#overview)
2. [Erosion Pipeline](#erosion-pipeline)
3. [High-Resolution Simulation](#high-resolution-simulation)
4. [Hydraulic Erosion](#hydraulic-erosion)
5. [Glacial Erosion](#glacial-erosion)
6. [River Erosion](#river-erosion)
7. [Materials System](#materials-system)
8. [Parameters Reference](#parameters-reference)
9. [Geomorphometry Analysis](#geomorphometry-analysis)
10. [System Interactions](#system-interactions)
11. [Performance](#performance)
12. [Configuration Recommendations](#configuration-recommendations)

---

## Overview

The erosion system implements three complementary physically-based algorithms:

| System | Algorithm | Features Created |
|--------|-----------|------------------|
| **River Erosion** | D8 flow accumulation + sediment transport | Drainage networks, deltas, floodplains |
| **Hydraulic Erosion** | Water droplet particle simulation | Fine detail, meandering, tributaries |
| **Glacial Erosion** | Shallow Ice Approximation (SIA) | U-shaped valleys, cirques, fjords |

All systems interact with rock hardness and temperature to create realistic terrain.

---

## Erosion Pipeline

**File**: `src/erosion/mod.rs`

### Order of Operations

```rust
pub fn simulate_erosion(
    heightmap: &mut Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    stress_map: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    params: &ErosionParams,
    rng: &mut ChaCha8Rng,
    seed: u64,
) -> (ErosionStats, Tilemap<f32>)
```

**Execution order** (with high-resolution simulation enabled):

1. **Upscale Heightmap** - 4x resolution with terrain roughness for meandering
2. **Pre-Erosion Blur** - Gaussian blur (radius 3) to melt sharp ridges
3. **River Erosion** - Carves major drainage channels
4. **Hydraulic Erosion** - Adds fine detail with 750k+ droplets
5. **Glacial Erosion** - U-shaped valleys in cold regions
6. **Post-Processing**:
   - Fill depressions (removes pits)
   - Carve river network with monotonic descent
   - Apply meander erosion (12 passes)
   - Final depression fill
7. **Smart Downscale** - Variance-based downsampling preserves river channels

### Statistics Tracking

```rust
pub struct ErosionStats {
    pub total_eroded: f64,         // Sum of all erosion
    pub total_deposited: f64,      // Sum of all deposition
    pub steps_taken: u64,          // Simulation steps
    pub iterations: usize,         // Droplets or timesteps
    pub max_erosion: f32,          // Deepest single erosion
    pub max_deposition: f32,       // Highest single deposition
    pub river_lengths: Vec<usize>, // Per-river traced lengths
}
```

---

## High-Resolution Simulation

**File**: `src/erosion/mod.rs`

The erosion system runs at 4x resolution by default for sharper river channels.

### Pipeline

```rust
fn simulate_erosion_hires(...) {
    // Step 1: Upscale with terrain roughness for meandering
    let mut hires_heightmap = heightmap.upscale_for_erosion(
        factor,              // 4x default
        params.hires_roughness,  // 20.0 - creates meandering paths
        params.hires_warp,       // 0.0 - disabled, use meander erosion instead
        seed,
    );

    // Step 1b: Pre-erosion blur - melts sharp ridges between parallel noise
    hires_heightmap = hires_heightmap.gaussian_blur(3);

    // Step 2-7: Run all erosion on high-res map
    // ... river, hydraulic, glacial erosion ...

    // Step 8: Meander erosion (12 passes)
    for pass in 0..12 {
        rivers::apply_meander_erosion(&mut hires_heightmap, ...);
    }

    // Step 10: Smart downscale with river preservation
    let result = hires_heightmap.downscale_preserve_rivers(factor, 15.0);
}
```

### Parameter Scaling

```rust
fn scale_params_for_resolution(params: &ErosionParams, factor: usize) -> ErosionParams {
    let mut scaled = params.clone();
    let area_scale = (factor * factor) as f32;

    // Scale flow thresholds by area (with 0.25x multiplier for dense network)
    scaled.river_source_min_accumulation *= area_scale * 0.25;

    // Scale max steps for larger map traversal
    scaled.droplet_max_steps *= factor;

    // Keep erosion radius small for sharp channels
    scaled.droplet_erosion_radius = scaled.droplet_erosion_radius.min(1);

    scaled
}
```

### Smart Downsampling

Variance-based downsampling preserves river channels:

```rust
pub fn downscale_preserve_rivers(&self, factor: usize, variance_threshold: f32) -> Self {
    for each output cell {
        let values = sample_input_block(factor);
        let variance = calculate_variance(values);

        if variance > variance_threshold {
            // High variance = river channel, use minimum (preserves carved depth)
            output = values.min();
        } else {
            // Low variance = flat terrain, use average
            output = values.mean();
        }
    }
}
```

---

## Hydraulic Erosion

**File**: `src/erosion/hydraulic.rs`

Simulates individual water droplets flowing down terrain, eroding steep slopes and depositing sediment when flow slows.

### Water Droplet Structure

```rust
struct WaterDroplet {
    x: f32, y: f32,           // Floating-point position
    dir_x: f32, dir_y: f32,   // Normalized direction (momentum)
    velocity: f32,             // Current speed
    water: f32,                // Remaining volume
    sediment: f32,             // Carried material
}
```

### Algorithm Per Droplet

```rust
for step in 0..params.droplet_max_steps {
    // 1. Calculate gradient at current position (bilinear interpolation)
    let (grad_x, grad_y) = gradient_at(heightmap, droplet.x, droplet.y);

    // 2. Update direction with inertia (maintains flow direction)
    droplet.dir_x = droplet.dir_x * inertia - grad_x * (1.0 - inertia);
    droplet.dir_y = droplet.dir_y * inertia - grad_y * (1.0 - inertia);
    normalize(&mut droplet.dir_x, &mut droplet.dir_y);

    // 3. Move droplet one cell
    let old_height = height_at(heightmap, droplet.x, droplet.y);
    droplet.x += droplet.dir_x;
    droplet.y += droplet.dir_y;
    let new_height = height_at(heightmap, droplet.x, droplet.y);
    let delta_height = new_height - old_height;

    // 4. Calculate sediment capacity
    let slope = (-delta_height).clamp(0.0, 50.0);
    let capacity = (slope * droplet.velocity * droplet.water * capacity_factor)
        .clamp(0.0, 500.0);

    // 5. Erode or deposit
    if droplet.sediment > capacity {
        // River slowing - DEPOSIT
        let deposit = (droplet.sediment - capacity) * deposit_rate;
        apply_deposit_brush(heightmap, droplet.x, droplet.y, deposit);
        droplet.sediment -= deposit;
    } else {
        // River accelerating - ERODE
        let erode = (capacity - droplet.sediment) * erosion_rate * (1.0 - hardness);
        let erode = erode.min(MAX_CHANGE_PER_STEP);  // Cap at 15 units
        apply_erosion_brush(heightmap, droplet.x, droplet.y, erode);
        droplet.sediment += erode;
    }

    // 6. Update velocity (accelerate downhill)
    droplet.velocity = (velocity * velocity + delta_height * gravity).sqrt();
    droplet.velocity = droplet.velocity.clamp(0.0, 50.0);

    // 7. Evaporate water
    droplet.water *= 1.0 - evaporation;

    // 8. Termination conditions
    if droplet.water < min_volume { break; }
    if new_height < 0.0 { /* deposit delta */ break; }
}
```

### Bilinear Interpolation

Enables smooth droplet movement between grid cells:

```rust
fn height_at(heightmap: &Tilemap<f32>, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let fx = x.fract();
    let fy = y.fract();

    let h00 = heightmap.get(x0, y0);
    let h10 = heightmap.get(x0 + 1, y0);
    let h01 = heightmap.get(x0, y0 + 1);
    let h11 = heightmap.get(x0 + 1, y0 + 1);

    // Bilinear: h = h00(1-fx)(1-fy) + h10*fx*(1-fy) + h01*(1-fx)*fy + h11*fx*fy
    h00 * (1.0 - fx) * (1.0 - fy)
        + h10 * fx * (1.0 - fy)
        + h01 * (1.0 - fx) * fy
        + h11 * fx * fy
}
```

### Erosion Brush Pattern

Gaussian-weighted circular brush for smooth erosion/deposition:

```rust
fn create_erosion_brush(radius: usize) -> Vec<(i32, i32, f32)> {
    if radius == 0 {
        return vec![(0, 0, 1.0)];  // Point erosion for sharp channels
    }

    let mut cells = Vec::new();
    let r = radius as i32;
    let r_sq = (radius * radius) as f32;

    for dy in -r..=r {
        for dx in -r..=r {
            let dist_sq = (dx * dx + dy * dy) as f32;
            if dist_sq <= r_sq {
                let weight = (1.0 - dist_sq / r_sq).max(0.0);
                cells.push((dx, dy, weight));
            }
        }
    }

    // Normalize weights to sum to 1.0
    let total: f32 = cells.iter().map(|(_, _, w)| w).sum();
    cells.iter_mut().for_each(|(_, _, w)| *w /= total);

    cells
}
```

Radius 3 creates ~29 affected cells per erosion/deposition event.

### Key Parameters ("POLISHED" Config)

The default parameters are tuned to fix the "Comb Effect" (parallel rivers) while maintaining sharp channels:

| Parameter | Default | Effect |
|-----------|---------|--------|
| `hydraulic_iterations` | 750,000 | Number of droplets |
| `droplet_inertia` | 0.3 | Low inertia - water turns easily, meanders naturally |
| `droplet_capacity_factor` | 10.0 | Sediment capacity multiplier |
| `droplet_erosion_rate` | 0.05 | Slow digging - prevents trench lock |
| `droplet_deposit_rate` | 0.2 | Moderate deposition - forces river merging |
| `droplet_evaporation` | 0.001 | Low evaporation - long-lived droplets find merges |
| `droplet_min_volume` | 0.01 | Threshold before droplet dies |
| `droplet_max_steps` | 3000 | Max steps per droplet |
| `droplet_erosion_radius` | 3 | Medium brush - sharp valleys, breaks parallel streams |
| `droplet_gravity` | 8.0 | Gravity multiplier |

---

## Glacial Erosion

**File**: `src/erosion/glacial.rs`

Implements the **Shallow Ice Approximation (SIA)** to simulate glacier flow and erosion.

### Physics Model

**Ice Flux (SIA equation)**:
```
q = -[(2A/(n+2)) × (ρg)ⁿ × hⁿ⁺² × |∇s|ⁿ⁻¹ + u_b × h] × ∇s
```

Where:
- `A` = Glen's flow coefficient (ice deformability)
- `n` = Glen's exponent (typically 3)
- `ρg` = ice density × gravity
- `h` = ice thickness
- `∇s` = surface gradient
- `u_b` = basal sliding coefficient

**Mass Balance (Continuity)**:
```
∂h/∂t = ȧ - ∇·q
```

Where:
- `ȧ` = mass balance (accumulation - ablation)
- `∇·q` = flux divergence

**Erosion Law**:
```
ė = K × |u_b|^k × (1 - hardness)
```

### Key Parameters

| Parameter | Default | Effect |
|-----------|---------|--------|
| `glacial_timesteps` | 500 | Simulation steps |
| `glacial_dt` | 100.0 | Years per step |
| `ice_deform_coefficient` | 1e-7 | Glen's A (ice plasticity) |
| `ice_sliding_coefficient` | 5e-4 | Basal sliding rate |
| `erosion_coefficient` | 1e-4 | Bedrock erodibility K |
| `mass_balance_gradient` | 0.005 | Accumulation rate per meter |
| `glaciation_temperature` | -3.0 | Below this, ice forms (enables coastal glaciation) |
| `glen_exponent` | 3.0 | Stress exponent (n) |
| `erosion_exponent` | 1.0 | Linear erosion law |

### Physical Features Created

- **U-shaped valleys** - Symmetric erosion (unlike V-shaped rivers)
- **Cirques** - Bowl-shaped headwalls from focused flow
- **Fjords** - Deep valleys where glaciers reach sea
- **Hanging valleys** - Tributaries higher than main glacier
- **Arêtes** - Sharp ridges between glaciers

---

## River Erosion

**File**: `src/erosion/rivers.rs`

Flow accumulation-based erosion with sediment transport. See [RIVERS.md](RIVERS.md) for complete details.

### Quick Summary

1. **D8 Flow Direction** - Each cell flows to steepest neighbor
2. **Flow Accumulation** - Count upstream drainage area
3. **Source Detection** - High elevation + sufficient drainage
4. **River Tracing** - Follow flow downstream with sediment transport
5. **Channel Carving** - V-shaped profile, width scales with flow
6. **Delta Formation** - Deposit sediment at river mouths

### Key Parameters

| Parameter | Default | Effect |
|-----------|---------|--------|
| `river_source_min_accumulation` | 15.0 | Baseline threshold (scaled by simulation_scale) |
| `river_source_min_elevation` | 100.0 | Min elevation for source |
| `river_capacity_factor` | 20.0 | Sediment capacity multiplier |
| `river_erosion_rate` | 0.5 | Erosion rate (reduced from 1.0) |
| `river_deposition_rate` | 0.5 | Deposition rate |
| `river_max_erosion` | 30.0 | Max erosion per cell (reduced from 150.0) |
| `river_channel_width` | 2 | Base channel half-width |

### Sea Level Constraint

All river erosion functions enforce a minimum height of 0.1m (`MIN_RIVER_HEIGHT`) to prevent rivers from eroding below sea level:

```rust
const MIN_RIVER_HEIGHT: f32 = 0.1;

// In apply_erosion(), carve_river_network(), and apply_meander_erosion():
let max_possible_erosion = (current_height - MIN_RIVER_HEIGHT).max(0.0);
let actual_erosion = erosion.min(max_possible_erosion);
```

This prevents river channels from being misclassified as ocean tiles due to digging below sea level.

---

## Materials System

**File**: `src/erosion/materials.rs`

Rock types with different erosion resistance.

### Rock Types and Hardness

| Rock Type | Hardness | Friability | Typical Environment |
|-----------|----------|------------|---------------------|
| Basalt | 0.95 | 0.05 | Oceanic crust, volcanic |
| Granite | 0.85 | 0.15 | Continental basement |
| Sandstone | 0.50 | 0.50 | Sedimentary basins |
| Limestone | 0.40 | 0.60 | Carbonate rocks, karst |
| Shale | 0.25 | 0.75 | Soft sedimentary |
| Sediment | 0.10 | 0.90 | Unconsolidated deposits |
| Ice | 0.05 | 0.95 | Glaciated regions |

### Impact on Erosion

In all erosion systems:
```rust
let erosion = base_erosion * (1.0 - hardness);
```

| Rock Type | Erosion Factor | Effect |
|-----------|----------------|--------|
| Basalt (0.95) | 0.05 | Nearly immune |
| Granite (0.85) | 0.15 | Very resistant |
| Sandstone (0.50) | 0.50 | Moderate |
| Sediment (0.10) | 0.90 | Erodes rapidly |

---

## Parameters Reference

**File**: `src/erosion/params.rs`

### ErosionParams Structure

```rust
pub struct ErosionParams {
    // Enable flags
    pub enable_rivers: bool,
    pub enable_hydraulic: bool,
    pub enable_glacial: bool,
    pub enable_analysis: bool,

    // Hydraulic erosion ("POLISHED" config)
    pub hydraulic_iterations: usize,    // 750,000
    pub droplet_inertia: f32,           // 0.3 - low for natural meandering
    pub droplet_capacity_factor: f32,   // 10.0
    pub droplet_erosion_rate: f32,      // 0.05 - slow to prevent trench lock
    pub droplet_deposit_rate: f32,      // 0.2 - moderate for river merging
    pub droplet_evaporation: f32,       // 0.001 - low for long rivers
    pub droplet_min_volume: f32,        // 0.01
    pub droplet_max_steps: usize,       // 3000
    pub droplet_erosion_radius: usize,  // 3 - medium brush
    pub droplet_initial_water: f32,     // 1.0
    pub droplet_initial_velocity: f32,  // 1.0
    pub droplet_gravity: f32,           // 8.0

    // Glacial erosion
    pub glacial_timesteps: usize,       // 500
    pub glacial_dt: f32,                // 100.0
    pub ice_deform_coefficient: f32,    // 1e-7
    pub ice_sliding_coefficient: f32,   // 5e-4
    pub erosion_coefficient: f32,       // 1e-4
    pub mass_balance_gradient: f32,     // 0.005
    pub glaciation_temperature: f32,    // -3.0
    pub glen_exponent: f32,             // 3.0
    pub erosion_exponent: f32,          // 1.0

    // River erosion (with sea level protection)
    pub river_source_min_accumulation: f32, // 15.0
    pub river_source_min_elevation: f32,    // 100.0
    pub river_capacity_factor: f32,         // 20.0
    pub river_erosion_rate: f32,            // 0.5 (reduced to prevent over-erosion)
    pub river_deposition_rate: f32,         // 0.5
    pub river_max_erosion: f32,             // 30.0 (reduced to prevent sub-sea-level digging)
    pub river_max_deposition: f32,          // 0.0
    pub river_channel_width: usize,         // 2

    // High-resolution simulation
    pub simulation_scale: usize,        // 4 (4x upscale)
    pub hires_roughness: f32,           // 20.0
    pub hires_warp: f32,                // 0.0

    // GPU acceleration
    pub use_gpu: bool,                  // true
}
```

### Preset Configurations

| Preset | Hydraulic | Glacial | Use Case |
|--------|-----------|---------|----------|
| `None` | 0 | 0 | Raw terrain only |
| `Minimal` | 50k | 100 | Fast preview |
| `Normal` | 750k | 500 | Default (balanced) |
| `Dramatic` | 750k | 750 | Deep canyons |
| `Realistic` | 1M | 1000 | Maximum quality |

### Usage

```rust
// From preset
let params = ErosionParams::from_preset(ErosionPreset::Normal);

// Custom
let params = ErosionParams {
    hydraulic_iterations: 300_000,
    enable_glacial: false,
    ..Default::default()
};

// Convenience constructors
let params = ErosionParams::fast();         // Testing
let params = ErosionParams::high_quality(); // Publication
```

---

## Geomorphometry Analysis

**File**: `src/erosion/geomorphometry.rs`

Quantitative validation that erosion produces realistic terrain.

### Hydrological Metrics

| Metric | Target | Description |
|--------|--------|-------------|
| Bifurcation Ratio (Rb) | 3.0-5.0 | Stream count ratio between orders |
| Hack's Law Exponent | 0.5-0.6 | Stream length vs drainage area |
| Concavity Index (θ) | 0.4-0.7 | Slope-area relationship |
| Fractal Dimension | ~2.0 | River network space-filling |
| Sinuosity Index | >1.5 | Meandering degree |
| Drainage Density | varies | Stream length per unit area |

### Validation Output

```
========== GEOMORPHOMETRY ANALYSIS ==========
1. Bifurcation Ratio (Rb):    3.94 [target: 3.0-5.0] PASS
2. Drainage Density (Dd):     0.0031 channels/pixel
3. Hack's Law Exponent (h):   0.54 [target: 0.5-0.6] PASS
4. Concavity Index (theta):   0.52 [target: 0.4-0.7] PASS
5. Fractal Dimension (D):     1.88 [target: ~2.0] PASS
...
Overall Realism Score: 76.3/100
```

---

## System Interactions

### Hydraulic + Glacial

**Execution order matters**:
- Hydraulic first: carves initial drainage
- Glaciers follow existing valleys
- Creates natural U-shaped overlaid on V-shaped

**In cold regions**:
- Glaciers widen and deepen river channels
- Creates fjord morphology
- Overdeepens valley heads (cirques)

### Hardness Effects

**Soft Rock (Sediment)**:
- Rapid erosion
- Wide, gentle valleys
- Quick-flowing droplets

**Hard Rock (Basalt)**:
- Slow erosion
- Narrow, steep canyons
- Droplets slow, deposit more

### Temperature Effects

**Warm Regions** (T > glaciation_temperature):
- Only hydraulic erosion
- V-shaped river valleys
- No glaciation

**Cold Regions** (T < glaciation_temperature):
- Glacial erosion adds U-shaped valleys
- Enhanced hydraulic from meltwater
- Fjords at coastlines

---

## Performance

### Time Complexity

| System | Complexity | Typical Time (512×256) |
|--------|------------|------------------------|
| River Erosion | O(r × L) | 100-500ms |
| Hydraulic (CPU) | O(d × s) | 5-30 seconds |
| Hydraulic (GPU) | O(d × s / W) | 100-500ms |
| Glacial | O(t × w × h) | 2-10 seconds |
| High-Res Pipeline | O(4² × above) | 60-120 seconds |

Where: d=droplets, s=steps, t=timesteps, w=width, h=height, W=GPU workgroups

### Memory

| Component | Size (512×256) | Size (2048×1024 hi-res) |
|-----------|----------------|-------------------------|
| Heightmap | 512 KB | 8 MB |
| Hardness map | 512 KB | 8 MB |
| Flow accumulation | 512 KB | 8 MB |
| Glacial state (5 maps) | 2.5 MB | 40 MB |
| **Total** | ~4-5 MB | ~80 MB |

---

## Configuration Recommendations

### Game Worlds (Fast)

```rust
ErosionParams {
    enable_rivers: true,
    enable_hydraulic: true,
    enable_glacial: false,
    hydraulic_iterations: 300_000,
    simulation_scale: 2,  // 2x instead of 4x
    use_gpu: true,
    ..Default::default()
}
```

### Realistic Earth-like

```rust
ErosionParams::from_preset(ErosionPreset::Realistic)
```

### Fast Iteration/Testing

```rust
ErosionParams::fast()
// or
ErosionParams::from_preset(ErosionPreset::Minimal)
```

### Deep Canyons

```rust
ErosionParams {
    hydraulic_iterations: 750_000,
    droplet_erosion_rate: 0.08,
    river_max_erosion: 200.0,
    droplet_evaporation: 0.001,  // Very long rivers
    ..Default::default()
}
```

---

## Module Reference

| File | Purpose |
|------|---------|
| `src/erosion/mod.rs` | Main pipeline orchestration, high-res simulation |
| `src/erosion/params.rs` | All configurable parameters |
| `src/erosion/hydraulic.rs` | Water droplet simulation |
| `src/erosion/glacial.rs` | Shallow Ice Approximation |
| `src/erosion/rivers.rs` | Flow accumulation, river tracing, meander erosion |
| `src/erosion/materials.rs` | Rock types and hardness |
| `src/erosion/geomorphometry.rs` | Validation metrics |
| `src/erosion/utils.rs` | Interpolation, gradients, brushes |
| `src/erosion/gpu.rs` | GPU compute shaders |
| `src/erosion/river_geometry.rs` | Bezier curve rivers |

---

## Features Created

### Hydraulic Erosion
- Meandering rivers (sinuosity 1.2-1.8)
- River deltas at mouths
- Tributary networks with proper Y-junctions
- Alluvial plains
- Canyon cutting

### Glacial Erosion
- U-shaped valleys
- Cirques (bowl headwalls)
- Hanging valleys
- Fjords
- Arêtes (sharp ridges)
- Moraines

### Combined
- Alpine lakes in cirques
- Waterfall zones at hanging valleys
- Layered terrain from differential hardness
- Natural mountain passes
