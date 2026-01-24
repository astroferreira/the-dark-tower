# Erosion Systems

Comprehensive documentation of the procedural world generator's terrain erosion systems.

---

## Table of Contents

1. [Overview](#overview)
2. [Erosion Pipeline](#erosion-pipeline)
3. [Hydraulic Erosion](#hydraulic-erosion)
4. [Glacial Erosion](#glacial-erosion)
5. [River Erosion](#river-erosion)
6. [Materials System](#materials-system)
7. [Parameters Reference](#parameters-reference)
8. [Geomorphometry Analysis](#geomorphometry-analysis)
9. [System Interactions](#system-interactions)
10. [Performance](#performance)
11. [Configuration Recommendations](#configuration-recommendations)

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

**Execution order**:

1. **River Erosion** - Carves major drainage channels
2. **Hydraulic Erosion** - Adds fine detail with 500k+ droplets
3. **Glacial Erosion** - U-shaped valleys in cold regions
4. **Post-Processing**:
   - Fill depressions (removes pits)
   - Carve river network with monotonic descent
   - Final depression fill

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

fn gradient_at(heightmap: &Tilemap<f32>, x: f32, y: f32) -> (f32, f32) {
    // Partial derivatives of bilinear interpolation
    let grad_x = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
    let grad_y = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;
    (grad_x, grad_y)
}
```

### Erosion Brush Pattern

Gaussian-weighted circular brush for smooth erosion/deposition:

```rust
fn create_erosion_brush(radius: usize) -> Vec<(i32, i32, f32)> {
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

Radius 2 creates ~13 affected cells per erosion/deposition event.

### High Elevation Spawning

Droplets spawn preferentially at high elevations (rain on mountains):

```rust
fn spawn_at_high_elevation(heightmap: &Tilemap<f32>, rng: &mut impl Rng) -> (f32, f32) {
    for _ in 0..10 {
        let x = rng.gen_range(0.0..width as f32);
        let y = rng.gen_range(0.0..height as f32);
        let h = height_at(heightmap, x, y);

        if h < 0.0 { continue; }  // Skip ocean

        // Probability increases with elevation (squared)
        let normalized_h = ((h - min_h) / height_range).clamp(0.0, 1.0);
        let probability = normalized_h * normalized_h;

        if rng.gen::<f32>() < probability.max(0.1) {
            return (x, y);
        }
    }
    // Fallback to random land position
}
```

### Parallelization

Batch processing for multi-core performance:

```rust
// Split 500k droplets into 10k-droplet batches
for batch in droplets.chunks(10_000) {
    // Snapshot heightmap for this batch
    let heightmap_snapshot = heightmap.clone();

    // Process batch in parallel (rayon)
    let deltas: Vec<_> = batch.par_iter()
        .map(|_| simulate_single_droplet(&heightmap_snapshot, ...))
        .collect();

    // Apply all deltas atomically
    for delta in deltas {
        apply_delta(heightmap, delta);
    }
}
```

### Key Parameters

| Parameter | Default | Effect |
|-----------|---------|--------|
| `hydraulic_iterations` | 500,000 | Number of droplets |
| `droplet_inertia` | 0.3 | Momentum (0=follow gradient, 1=straight) |
| `droplet_capacity_factor` | 10.0 | Sediment capacity multiplier |
| `droplet_erosion_rate` | 0.05 | Erosion aggressiveness |
| `droplet_deposit_rate` | 0.1 | Deposition aggressiveness |
| `droplet_evaporation` | 0.002 | Water loss per step (low=long rivers) |
| `droplet_min_volume` | 0.01 | Threshold before droplet dies |
| `droplet_max_steps` | 2000 | Max steps per droplet |
| `droplet_erosion_radius` | 2 | Gaussian brush radius |
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

### Glacial State

```rust
struct GlacialState {
    bedrock: Tilemap<f32>,           // Elevation being eroded
    ice_thickness: Tilemap<f32>,     // Current ice depth
    flux_x: Tilemap<f32>,            // Ice velocity X
    flux_y: Tilemap<f32>,            // Ice velocity Y
    sliding_velocity: Tilemap<f32>,  // Basal sliding (drives erosion)
}
```

### Mass Balance Calculation

```rust
fn calculate_mass_balance(
    state: &GlacialState,
    temperature: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
    params: &ErosionParams,
) -> Tilemap<f32> {
    let ela = estimate_equilibrium_line_altitude(temperature, heightmap);

    for (x, y) in all_cells {
        let surface = *state.bedrock.get(x, y) + *state.ice_thickness.get(x, y);
        let temp = *temperature.get(x, y);

        if temp > params.glaciation_temperature {
            // Too warm - ablation only
            mass_balance.set(x, y, -params.mass_balance_gradient * 10.0);
        } else {
            // Above ELA = accumulation, below = ablation
            let elevation_above_ela = surface - ela;
            let balance = elevation_above_ela * params.mass_balance_gradient;
            mass_balance.set(x, y, balance.clamp(-5.0, 5.0));
        }
    }
}
```

### Ice Flux Computation

```rust
fn calculate_ice_flux(state: &mut GlacialState, params: &ErosionParams) {
    let n = params.glen_exponent;      // 3.0
    let A = params.ice_deform_coeff;   // 1e-7
    let u_b = params.ice_sliding_coeff; // 5e-4
    let rho_g = 0.01;                  // Scaled gravity

    for (x, y) in all_cells {
        let h = *state.ice_thickness.get(x, y);
        if h <= 0.1 { continue; }

        let (grad_x, grad_y) = surface_gradient(state, x, y);
        let grad_mag = (grad_x * grad_x + grad_y * grad_y).sqrt();
        if grad_mag < 1e-4 { continue; }

        // Deformation term (Glen's flow law)
        let deform_coeff = (2.0 * A) / (n + 2.0);
        let deform = deform_coeff
            * rho_g.powf(n)
            * h.powf(n + 2.0)
            * grad_mag.powf(n - 1.0);

        // Sliding term
        let sliding = u_b * h;

        // Total flux (flows downslope)
        let flux_mag = -(deform + sliding);
        state.flux_x.set(x, y, flux_mag * grad_x);
        state.flux_y.set(x, y, flux_mag * grad_y);

        // Store sliding velocity for erosion
        let sliding_vel = (u_b * h * grad_mag).min(100.0);
        state.sliding_velocity.set(x, y, sliding_vel);
    }
}
```

### Ice Thickness Update

```rust
fn update_ice_thickness(
    state: &mut GlacialState,
    mass_balance: &Tilemap<f32>,
    params: &ErosionParams,
) {
    let dt = params.glacial_dt;

    for (x, y) in all_cells {
        let h = *state.ice_thickness.get(x, y);
        let m = *mass_balance.get(x, y);

        // Flux divergence: how much ice flows away
        let div_q = divergence_at_cell(&state.flux_x, &state.flux_y, x, y);

        // Update: h_new = h + dt * (accumulation - outflow)
        let dh = dt * (m - div_q);
        let h_new = (h + dh).max(0.0);

        state.ice_thickness.set(x, y, h_new);
    }
}
```

### Bedrock Erosion

```rust
fn apply_glacial_erosion(
    state: &mut GlacialState,
    hardness: &Tilemap<f32>,
    params: &ErosionParams,
) {
    let k = params.erosion_coefficient;
    let exp = params.erosion_exponent;
    let dt = params.glacial_dt;

    for (x, y) in all_cells {
        let u_b = *state.sliding_velocity.get(x, y);
        let h = *state.ice_thickness.get(x, y);

        if u_b <= 0.0 || h < 10.0 { continue; }

        // Erosion increases with ice thickness (pressure)
        let ice_factor = (h / 200.0).clamp(0.1, 1.5);
        let erosion_rate = k * u_b.powf(exp) * ice_factor;

        // Modulate by rock hardness
        let hardness_factor = 1.0 - *hardness.get(x, y);
        let erosion = (erosion_rate * hardness_factor * dt).min(5.0);

        if erosion > 0.0 {
            let current = *state.bedrock.get(x, y);
            state.bedrock.set(x, y, current - erosion);
        }
    }
}
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
| `glaciation_temperature` | -3.0 | Below this, ice forms |
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
| `river_source_min_accumulation` | 10.0 | Min flow for river source |
| `river_source_min_elevation` | 100.0 | Min elevation for source |
| `river_capacity_factor` | 20.0 | Sediment capacity multiplier |
| `river_erosion_rate` | 1.0 | Erosion aggressiveness |
| `river_deposition_rate` | 0.5 | Deposition rate |
| `river_channel_width` | 2 | Base channel half-width |

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

### Material Assignment Logic

```rust
fn assign_material(
    elevation: f32,
    plate_type: PlateType,
    stress: f32,
    noise: f32,
) -> RockType {
    match plate_type {
        PlateType::Oceanic => {
            if elevation > 0.0 { RockType::Basalt }
            else if elevation > -1000.0 { mix(Basalt, Sediment) }
            else { RockType::Basalt }
        }
        PlateType::Continental => {
            if stress > 0.6 { RockType::Granite }  // Mountain building
            else if elevation > 2000.0 { RockType::Granite }  // Highlands
            else if elevation > 500.0 { mix(Sandstone, Limestone, Shale) }
            else if elevation > 50.0 { mix(Sandstone, Shale) }
            else { RockType::Sediment }  // Coastal/delta
        }
    }
}
```

### Hardness Map Generation

```rust
fn generate_hardness_map(
    material_map: &Tilemap<RockType>,
    seed: u64,
) -> Tilemap<f32> {
    let noise = Perlin::new(seed);

    for (x, y) in all_cells {
        let base_hardness = material_map.get(x, y).hardness();

        // Add variation ±0.15
        let variation = noise.get([x as f64 * 0.1, y as f64 * 0.1]) as f32 * 0.15;
        let hardness = (base_hardness + variation).clamp(0.05, 1.0);

        hardness_map.set(x, y, hardness);
    }
}
```

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

    // Hydraulic erosion
    pub hydraulic_iterations: usize,
    pub droplet_inertia: f32,
    pub droplet_capacity_factor: f32,
    pub droplet_erosion_rate: f32,
    pub droplet_deposit_rate: f32,
    pub droplet_evaporation: f32,
    pub droplet_min_volume: f32,
    pub droplet_max_steps: usize,
    pub droplet_erosion_radius: usize,
    pub droplet_initial_water: f32,
    pub droplet_initial_velocity: f32,
    pub droplet_gravity: f32,

    // Glacial erosion
    pub glacial_timesteps: usize,
    pub glacial_dt: f32,
    pub ice_deform_coefficient: f32,
    pub ice_sliding_coefficient: f32,
    pub erosion_coefficient: f32,
    pub mass_balance_gradient: f32,
    pub glaciation_temperature: f32,
    pub glen_exponent: f32,
    pub erosion_exponent: f32,

    // River erosion
    pub river_source_min_accumulation: f32,
    pub river_source_min_elevation: f32,
    pub river_capacity_factor: f32,
    pub river_erosion_rate: f32,
    pub river_deposition_rate: f32,
    pub river_max_erosion: f32,
    pub river_channel_width: usize,

    // GPU acceleration
    pub use_gpu: bool,
}
```

### Preset Configurations

| Preset | Hydraulic | Glacial | Use Case |
|--------|-----------|---------|----------|
| `None` | 0 | 0 | Raw terrain only |
| `Minimal` | 50k | 100 | Fast preview |
| `Normal` | 500k | 500 | Default (balanced) |
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

### Advanced Metrics

| Metric | Target | Description |
|--------|--------|-------------|
| Hypsometric Integral | 0.3-0.6 | Terrain maturity |
| Moran's I | >0.8 | Spatial autocorrelation |
| Slope Skewness | >0.0 | Natural log-normal distribution |
| Plan Curvature | ~0.0 | Balanced convergence/divergence |
| Profile Curvature | <0.0 | Concave (natural) profiles |
| Knickpoint Density | <0.01 | Smooth river profiles |
| Relative Relief | >50 | Landscape-scale roughness |

### Strahler Stream Ordering

```
Order 1 = headwaters (no upstream tributaries)
Order 2 = two order-1 streams meet
Order n = two order-(n-1) streams meet
```

### Realism Score

Combines all metrics into 0-100 score:
- **Hydrological (50 points)**: Bifurcation, Hack's, Concavity, Fractal
- **Advanced (50 points)**: Hypsometric, Moran's I, Curvatures, Relief

### Example Output

```
========== GEOMORPHOMETRY ANALYSIS ==========
1. Bifurcation Ratio (Rb):    3.42 [target: 3.0-5.0] PASS
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

**Mixed Hardness**:
- Step-like profiles
- Hard layers form cliffs
- Differential erosion

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

Where: d=droplets, s=steps, t=timesteps, w=width, h=height, W=GPU workgroups

### Memory

| Component | Size (512×256) |
|-----------|----------------|
| Heightmap | 512 KB |
| Hardness map | 512 KB |
| Flow accumulation | 512 KB |
| Glacial state (5 maps) | 2.5 MB |
| **Total** | ~4-5 MB |

### Scalability

| Resolution | Approximate Time |
|------------|------------------|
| 512×256 | ~10 seconds |
| 1024×512 | ~40 seconds |
| 2048×1024 | ~160 seconds |

Linear with pixel count.

### GPU Acceleration

**File**: `src/erosion/gpu.rs`

Optional WGSL compute shader for hydraulic erosion:
- 50-100x speedup over single-threaded CPU
- Falls back to rayon parallelization if unavailable

```rust
if params.use_gpu {
    simulate_gpu_or_cpu(heightmap, hardness, params, seed)
} else {
    simulate_parallel(heightmap, hardness, params, seed)
}
```

---

## Configuration Recommendations

### Game Worlds (Fast)

```rust
ErosionParams {
    enable_rivers: true,
    enable_hydraulic: true,
    enable_glacial: false,
    hydraulic_iterations: 300_000,
    river_max_erosion: 100.0,
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

### Arid Worlds (Minimal Water)

```rust
ErosionParams {
    hydraulic_iterations: 50_000,
    droplet_evaporation: 0.01,  // Short rivers
    enable_glacial: false,
    ..Default::default()
}
```

### Ice Age Worlds

```rust
ErosionParams {
    enable_glacial: true,
    glacial_timesteps: 1000,
    ice_sliding_coefficient: 1e-3,  // More sliding
    glaciation_temperature: 5.0,     // Glaciate at higher temps
    ..Default::default()
}
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
| `src/erosion/mod.rs` | Main pipeline orchestration |
| `src/erosion/params.rs` | All configurable parameters |
| `src/erosion/hydraulic.rs` | Water droplet simulation |
| `src/erosion/glacial.rs` | Shallow Ice Approximation |
| `src/erosion/rivers.rs` | Flow accumulation, river tracing |
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
- Tributary networks
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
