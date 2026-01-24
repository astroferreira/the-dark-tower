# Rivers, Lakes, and Water Systems

Comprehensive documentation of the procedural world generator's hydrological systems.

---

## Table of Contents

1. [Overview](#overview)
2. [Flow Direction (D8 Algorithm)](#flow-direction-d8-algorithm)
3. [Flow Accumulation](#flow-accumulation)
4. [River Erosion and Sediment Transport](#river-erosion-and-sediment-transport)
5. [Channel Geometry](#channel-geometry)
6. [Delta Formation](#delta-formation)
7. [Depression Filling](#depression-filling)
8. [Lake Detection](#lake-detection)
9. [Bezier River Geometry](#bezier-river-geometry)
10. [Hydraulic Erosion](#hydraulic-erosion)
11. [Glacial Erosion](#glacial-erosion)
12. [System Interactions](#system-interactions)
13. [Module Reference](#module-reference)

---

## Overview

The river system uses physically-based algorithms to create realistic drainage networks:

- **D8 Flow Direction**: Determines where water flows from each cell
- **Flow Accumulation**: Calculates drainage area (proxy for discharge)
- **Sediment Transport**: Rivers erode uplands and deposit in lowlands
- **Depression Filling**: Ensures all water reaches the ocean
- **Multi-scale Erosion**: Hydraulic droplets + glacial ice sheets

The result is dendritic river networks with proper tributaries, widening downstream, floodplains, and deltas.

---

## Flow Direction (D8 Algorithm)

**File**: `src/erosion/rivers.rs`

The D8 algorithm assigns each cell a flow direction toward its steepest downslope neighbor.

### Direction Encoding

```
7 0 1
6 X 2
5 4 3
```

```rust
pub const DX: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
pub const DY: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
pub const NO_FLOW: u8 = 255;  // Pit or ocean
```

### Algorithm

```rust
pub fn compute_flow_direction(heightmap: &Tilemap<f32>) -> Tilemap<u8> {
    for (x, y) in all_cells {
        let h = *heightmap.get(x, y);
        let mut best_dir = NO_FLOW;
        let mut best_slope = 0.0;

        for dir in 0..8 {
            let nx = (x as i32 + DX[dir]).rem_euclid(width);  // Horizontal wrap
            let ny = (y as i32 + DY[dir]).clamp(0, height-1); // Vertical clamp

            let nh = *heightmap.get(nx, ny);
            let drop = h - nh;
            let dist = if dir % 2 == 0 { 1.0 } else { 1.414 };  // Diagonal distance
            let slope = drop / dist;

            if slope > best_slope {
                best_slope = slope;
                best_dir = dir as u8;
            }
        }

        flow_dir.set(x, y, best_dir);
    }
}
```

**Key points**:
- Horizontal wrapping (equirectangular projection)
- Diagonal neighbors have distance √2 ≈ 1.414
- `NO_FLOW` (255) indicates pits or ocean cells

---

## Flow Accumulation

**File**: `src/erosion/rivers.rs`

Flow accumulation counts how many upstream cells drain through each point. This is a proxy for river discharge.

### Algorithm

```rust
pub fn compute_flow_accumulation(
    heightmap: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
) -> Tilemap<f32> {
    // Each cell starts with 1.0 (itself)
    let mut accumulation = Tilemap::new_with(width, height, 1.0);

    // Sort cells by elevation (highest first)
    let mut cells: Vec<(usize, usize, f32)> = all_cells_with_height();
    cells.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // Process high-to-low, passing accumulation downstream
    for (x, y, _) in cells {
        let dir = *flow_dir.get(x, y);
        if dir == NO_FLOW { continue; }

        let (nx, ny) = next_cell(x, y, dir);
        let current_acc = *accumulation.get(x, y);
        let downstream_acc = *accumulation.get(nx, ny);
        accumulation.set(nx, ny, downstream_acc + current_acc);
    }

    accumulation
}
```

**Why high-to-low order?** Processing from mountain peaks ensures each cell's full upstream area is accumulated before passing it downstream.

### Interpretation

| Flow Accumulation | Meaning |
|-------------------|---------|
| 1 | Ridge/peak (only itself) |
| 10-50 | Small stream |
| 50-200 | Minor river |
| 200-1000 | Major river |
| 1000+ | Large river system |

---

## River Erosion and Sediment Transport

**File**: `src/erosion/rivers.rs`

Rivers trace paths from sources to the ocean, eroding terrain and transporting sediment.

### River State

```rust
struct RiverState {
    x: usize, y: usize,        // Current position
    sediment: f32,              // Material being carried
    flow: f32,                  // Accumulated drainage (increases downstream)
    velocity: f32,              // Current speed (from slope)
}
```

### Source Detection

```rust
fn find_river_sources(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    params: &RiverErosionParams,
) -> Vec<(usize, usize)> {
    sources.filter(|cell| {
        elevation >= params.source_min_elevation &&  // High enough
        flow_acc >= params.source_min_accumulation &&  // Enough drainage
        flow_acc < params.source_min_accumulation * 3.0  // Not mid-river
    })
}
```

Default thresholds:
- `source_min_elevation`: 50m
- `source_min_accumulation`: 100.0

### Erosion vs Deposition

The core decision: **erode if under-capacity, deposit if over-capacity**.

```rust
// Sediment capacity depends on flow and slope
let capacity = capacity_factor * flow.sqrt() * slope * velocity;
let min_capacity = capacity_factor * flow.sqrt() * 0.01;  // Minimum even on flat terrain
let capacity = capacity.max(min_capacity);

if sediment < capacity {
    // River is "hungry" - ERODE
    let erosion = (capacity - sediment) * erosion_rate * hardness_factor;
    erosion = erosion.min(max_safe_erosion);  // Don't erode below downstream
    apply_erosion(heightmap, x, y, erosion, ...);
    sediment += erosion;
} else {
    // River is overloaded - DEPOSIT
    let deposit = (sediment - capacity) * deposition_rate;
    apply_deposition(heightmap, x, y, deposit, ...);
    sediment -= deposit;
}
```

### Rock Hardness Modifier

```rust
let rock_hardness = *hardness.get(x, y);
let hardness_factor = (1.0 - rock_hardness).max(0.1);

// Examples:
// Basalt (0.95 hardness) → factor 0.05 → resists erosion
// Sediment (0.1 hardness) → factor 0.9 → erodes easily
```

### Monotonic Descent Enforcement

Critical for river connectivity:

```rust
let min_drop = 0.05;
if next_height >= current_height - min_drop {
    // Force downstream cell to be strictly lower
    heightmap.set(nx, ny, current_height - min_drop);
}
```

This prevents pits and ensures continuous flow to the ocean.

---

## Channel Geometry

**File**: `src/erosion/rivers.rs`

### Dynamic Width

River width scales with drainage area (hydraulic geometry):

```rust
fn calculate_river_width(flow: f32, base_width: usize, source_threshold: f32) -> usize {
    // w ∝ Q^0.5 (Leopold & Maddock, 1953)
    let flow_ratio = (flow / source_threshold).max(1.0);
    let width_multiplier = flow_ratio.sqrt();

    let dynamic_width = (base_width as f32 * width_multiplier).round() as usize;
    dynamic_width.clamp(1, 8)  // 1-8 pixel half-width
}
```

### V-Shaped Channel Profile

```rust
fn apply_erosion(heightmap, x, y, amount, flow_dir, flow) {
    let half_width = calculate_river_width(flow, ...);

    // Erode perpendicular to flow direction
    for i in -(half_width as i32)..=(half_width as i32) {
        let dist = i.abs() as f32;
        let falloff = 1.0 - (dist / (half_width as f32 + 1.0));
        let local_erosion = amount * falloff * falloff;  // Squared for V-shape

        // Apply to perpendicular neighbor
        let (px, py) = perpendicular_to_flow(x, y, flow_dir, i);
        heightmap.set(px, py, current - local_erosion);
    }
}
```

### Floodplain Deposition

Sediment deposits *beside* the channel, not in it:

```rust
fn apply_deposition(heightmap, x, y, amount, flow_dir, flow) {
    let half_width = calculate_river_width(flow, ...);
    let inner_radius = half_width + 1;  // Start outside channel
    let outer_radius = half_width + 3;  // Deposit extent

    for i in -outer_radius..=outer_radius {
        if i.abs() <= inner_radius { continue; }  // Skip channel

        let dist_from_channel = (i.abs() - inner_radius) as f32;
        let falloff = 1.0 - (dist_from_channel / outer_radius as f32);
        let local_deposit = amount * falloff * 0.3;  // Subtle levees

        heightmap.set(px, py, current + local_deposit);
    }
}
```

This creates natural **levees and floodplains**.

---

## Delta Formation

**File**: `src/erosion/rivers.rs`

When rivers reach sea level, sediment fans out as a delta:

```rust
fn apply_delta_deposition(heightmap, x, y, sediment, flow_dir) {
    let fan_radius = 4i32;
    let (flow_dx, flow_dy) = direction_vector(flow_dir);

    for dy in 0..=fan_radius {
        for dx in -fan_radius..=fan_radius {
            // Only deposit downstream (forward direction)
            let forward = dx * flow_dx + dy * flow_dy;
            if forward < 0 { continue; }

            // Only within fan radius
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > fan_radius * fan_radius { continue; }

            let dist = (dist_sq as f32).sqrt();
            let falloff = 1.0 - (dist / (fan_radius as f32 + 1.0));
            let local_deposit = sediment * falloff * 0.5;

            // Only deposit in water (building delta)
            let (nx, ny) = (x + dx, y + dy);
            if *heightmap.get(nx, ny) < 0.0 {
                let new_h = (*heightmap.get(nx, ny) + local_deposit).min(5.0);
                heightmap.set(nx, ny, new_h);
            }
        }
    }
}
```

---

## Depression Filling

**File**: `src/erosion/rivers.rs`

The **Planchon-Darboux algorithm** fills pits to ensure all water reaches the ocean.

### Algorithm

```rust
fn fill_depressions(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let epsilon = 1e-4;  // Tiny drop to ensure flow
    let mut water = Tilemap::new(width, height);

    // Initialize: ocean at original height, land at infinity
    for (x, y) in all_cells {
        if *heightmap.get(x, y) < 0.0 {
            water.set(x, y, *heightmap.get(x, y));  // Ocean
        } else {
            water.set(x, y, f32::MAX);  // Land starts at infinity
        }
    }

    // Iteratively lower water surface
    loop {
        let mut changed = false;

        // Forward pass (top-left to bottom-right)
        for (x, y) in all_cells {
            let terrain = *heightmap.get(x, y);
            let current_water = *water.get(x, y);

            // Find minimum neighbor water level
            let min_neighbor = neighbors(x, y)
                .map(|(nx, ny)| *water.get(nx, ny))
                .min();

            // Water level is max(terrain, neighbor + epsilon)
            let new_water = terrain.max(min_neighbor + epsilon);

            if new_water < current_water {
                water.set(x, y, new_water);
                changed = true;
            }
        }

        // Backward pass (bottom-right to top-left)
        // ... similar logic, reverse iteration order

        if !changed { break; }
    }

    water
}
```

**Why epsilon?** The tiny drop (0.0001) ensures water can flow over flat filled areas.

---

## Lake Detection

**File**: `src/water_bodies.rs`

### Three-Step Process

**Step 1: Mark Water Tiles**
```rust
for (x, y) in all_cells {
    if *heightmap.get(x, y) <= 0.0 {
        is_water.set(x, y, true);
    }
}
```

**Step 2: Ocean Detection (BFS from edges)**
```rust
// Start from water tiles on north/south edges
let mut queue = VecDeque::new();
for x in 0..width {
    if *is_water.get(x, 0) { queue.push_back((x, 0)); }
    if *is_water.get(x, height-1) { queue.push_back((x, height-1)); }
}

// BFS flood-fill marks all connected water as ocean
while let Some((x, y)) = queue.pop_front() {
    water_map.set(x, y, WaterBodyId::OCEAN);

    for (nx, ny) in neighbors_4(x, y) {
        if is_water.get(nx, ny) && !visited.get(nx, ny) {
            visited.set(nx, ny, true);
            queue.push_back((nx, ny));
        }
    }
}
```

**Step 3: Lake Detection (remaining water)**
```rust
for (x, y) in unvisited_water_cells {
    let lake_id = WaterBodyId(next_id);
    next_id += 1;

    // BFS to find all tiles in this lake
    let mut lake = WaterBody::new(lake_id, WaterBodyType::Lake);
    flood_fill_lake(&mut lake, x, y);

    water_bodies.push(lake);
}
```

### River Detection

```rust
const RIVER_FLOW_THRESHOLD: f32 = 50.0;

for (x, y) in all_cells {
    let elevation = *heightmap.get(x, y);
    let flow = *flow_accumulation.get(x, y);

    // River: LAND tile with high flow accumulation
    if elevation > 0.0 && flow >= RIVER_FLOW_THRESHOLD {
        river_tiles.push((x, y, flow));
    }
}
```

### WaterBody Structure

```rust
pub struct WaterBody {
    pub id: WaterBodyId,
    pub body_type: WaterBodyType,  // Ocean | Lake | River
    pub tile_count: usize,
    pub min_elevation: f32,
    pub max_elevation: f32,
    pub avg_elevation: f32,
    pub bounds: (usize, usize, usize, usize),  // min_x, min_y, max_x, max_y
}
```

### Fantasy Lake Conversions

Special biomes based on conditions:

```rust
pub fn determine_lake_fantasy_biome(
    water_body: &WaterBody,
    avg_temp: f32,
    avg_stress: f32,
    rng_value: f32,
) -> Option<ExtendedBiome> {
    if avg_temp < -5.0 && rng_value < 0.7 {
        return Some(ExtendedBiome::FrozenLake);
    }
    if avg_stress > 0.4 && rng_value < 0.5 {
        return Some(ExtendedBiome::LavaLake);
    }
    if avg_stress > 0.2 && avg_temp < 10.0 && rng_value < 0.3 {
        return Some(ExtendedBiome::AcidLake);
    }
    if avg_temp > 20.0 && water_body.tile_count > 10 && rng_value < 0.2 {
        return Some(ExtendedBiome::BioluminescentWater);
    }
    None
}
```

---

## Bezier River Geometry

**File**: `src/erosion/river_geometry.rs`

Advanced system for smooth, natural-looking river curves.

### Control Points

```rust
pub struct RiverControlPoint {
    pub world_x: f32,
    pub world_y: f32,
    pub flow_accumulation: f32,
    pub width: f32,
    pub elevation: f32,
}

pub struct BezierRiverSegment {
    pub p0: RiverControlPoint,  // Start
    pub p1: RiverControlPoint,  // Control 1
    pub p2: RiverControlPoint,  // Control 2
    pub p3: RiverControlPoint,  // End
    pub tributaries: Vec<usize>,
}
```

### Cubic Bezier Evaluation

```rust
pub fn evaluate(&self, t: f32) -> RiverControlPoint {
    let mt = 1.0 - t;

    // B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃
    let w0 = mt * mt * mt;
    let w1 = 3.0 * mt * mt * t;
    let w2 = 3.0 * mt * t * t;
    let w3 = t * t * t;

    RiverControlPoint {
        world_x: w0*p0.x + w1*p1.x + w2*p2.x + w3*p3.x,
        world_y: w0*p0.y + w1*p1.y + w2*p2.y + w3*p3.y,
        // ... interpolate other fields
    }
}
```

### Meandering with Perlin Noise

```rust
fn apply_meander(points: &[RiverControlPoint], noise: &Perlin) -> Vec<RiverControlPoint> {
    for (i, pt) in points.iter().enumerate() {
        if i == 0 || i == points.len() - 1 {
            result.push(pt.clone());  // Keep endpoints fixed
            continue;
        }

        // Calculate perpendicular direction
        let tangent = (next.pos - prev.pos).normalize();
        let perpendicular = (-tangent.y, tangent.x);

        // Sample noise for offset
        let noise_val = noise.get([
            pt.world_x * meander_frequency,
            pt.world_y * meander_frequency,
        ]) as f32;

        let offset = noise_val * meander_amplitude * pt.width * 2.0;

        result.push(RiverControlPoint {
            world_x: pt.world_x + perpendicular.0 * offset,
            world_y: pt.world_y + perpendicular.1 * offset,
            ..pt.clone()
        });
    }
}
```

### Width from Hydraulic Geometry

```rust
fn calculate_river_width(flow_acc: f32, params: &RiverNetworkParams) -> f32 {
    // Leopold & Maddock (1953): w ∝ Q^0.5
    let flow_ratio = (flow_acc / params.source_threshold).max(1.0);
    let width = params.base_width * flow_ratio.powf(params.width_exponent);

    width.clamp(0.5, 12.0)
}
```

---

## Hydraulic Erosion

**File**: `src/erosion/hydraulic.rs`

Particle-based water droplet simulation for fine detail.

### Water Droplet

```rust
struct WaterDroplet {
    x: f32, y: f32,          // Floating-point position
    dir_x: f32, dir_y: f32,  // Normalized direction
    velocity: f32,
    water: f32,              // Volume
    sediment: f32,           // Carried material
}
```

### Droplet Simulation Loop

```rust
for step in 0..params.droplet_max_steps {
    // 1. Calculate terrain gradient
    let (grad_x, grad_y) = gradient_at(heightmap, droplet.x, droplet.y);

    // 2. Update direction with inertia
    droplet.dir_x = droplet.dir_x * inertia - grad_x * (1.0 - inertia);
    droplet.dir_y = droplet.dir_y * inertia - grad_y * (1.0 - inertia);
    normalize(&mut droplet.dir_x, &mut droplet.dir_y);

    // 3. Move droplet
    let old_height = height_at(droplet.x, droplet.y);
    droplet.x += droplet.dir_x;
    droplet.y += droplet.dir_y;
    let new_height = height_at(droplet.x, droplet.y);
    let delta_height = new_height - old_height;

    // 4. Calculate sediment capacity
    let slope = (-delta_height).clamp(0.0, 50.0);
    let capacity = slope * droplet.velocity * droplet.water * capacity_factor;

    // 5. Erode or deposit
    if droplet.sediment > capacity {
        let deposit = (droplet.sediment - capacity) * deposit_rate;
        apply_deposit(...);
        droplet.sediment -= deposit;
    } else {
        let erode = (capacity - droplet.sediment) * erosion_rate;
        apply_erosion(...);
        droplet.sediment += erode;
    }

    // 6. Update velocity (accelerate downhill)
    droplet.velocity = sqrt(velocity² + delta_height * gravity);

    // 7. Evaporate
    droplet.water *= (1.0 - evaporation);

    if droplet.water < min_volume { break; }
}
```

### High Elevation Spawning Preference

```rust
fn spawn_droplet(heightmap, rng) -> (f32, f32) {
    for _ in 0..10 {
        let (x, y) = random_position();
        let h = height_at(x, y);

        if h < sea_level { continue; }

        // Higher elevation = higher spawn probability
        let normalized_h = (h - min_h) / height_range;
        let probability = normalized_h * normalized_h;  // Squared

        if rng.gen::<f32>() < probability.max(0.1) {
            return (x, y);
        }
    }
}
```

---

## Glacial Erosion

**File**: `src/erosion/glacial.rs`

Uses the **Shallow Ice Approximation (SIA)** to simulate glacier flow and U-shaped valley carving.

### Glacial State

```rust
struct GlacialState {
    bedrock: Tilemap<f32>,          // Eroded by glaciers
    ice_thickness: Tilemap<f32>,
    flux_x: Tilemap<f32>,           // Ice flow (x)
    flux_y: Tilemap<f32>,           // Ice flow (y)
    sliding_velocity: Tilemap<f32>, // Basal sliding
}
```

### Mass Balance

```rust
fn calculate_mass_balance(state, temperature, params) -> Tilemap<f32> {
    let ela = estimate_equilibrium_line_altitude(temperature, heightmap);

    for (x, y) in all_cells {
        let surface = state.bedrock.get(x, y) + state.ice_thickness.get(x, y);
        let temp = *temperature.get(x, y);

        if temp > params.glaciation_temperature {
            // Too warm - melt
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

### Shallow Ice Approximation

```rust
// SIA: q = -[(2A/(n+2)) * (ρg)^n * h^(n+2) * |∇s|^(n-1) + u_b*h] * ∇s

fn calculate_ice_flux(state, params) {
    let n = params.glen_exponent;  // Typically 3.0
    let A = params.ice_deform_coeff;
    let u_b = params.ice_sliding_coeff;

    for (x, y) in all_cells {
        let h = *state.ice_thickness.get(x, y);
        if h <= 0.1 { continue; }

        let (grad_x, grad_y) = surface_gradient(state, x, y);
        let grad_mag = sqrt(grad_x² + grad_y²);

        // Deformation term (Glen's flow law)
        let deform = (2*A)/(n+2) * (rho_g)^n * h^(n+2) * grad_mag^(n-1);

        // Sliding term
        let sliding = u_b * h;

        // Total flux (downslope)
        let flux_mag = -(deform + sliding);
        state.flux_x.set(x, y, flux_mag * grad_x);
        state.flux_y.set(x, y, flux_mag * grad_y);

        // Record sliding velocity for erosion
        state.sliding_velocity.set(x, y, u_b * h * grad_mag);
    }
}
```

### Ice Thickness Evolution

```rust
// Continuity equation: ∂h/∂t = ȧ - ∇·q

fn update_ice_thickness(state, mass_balance, params) {
    for (x, y) in all_cells {
        let h = *state.ice_thickness.get(x, y);
        let m = *mass_balance.get(x, y);
        let div_q = flux_divergence(state, x, y);

        let dh = params.dt * (m - div_q);
        state.ice_thickness.set(x, y, (h + dh).max(0.0));
    }
}
```

### Glacial Erosion Law

```rust
// ė = K * |u_b|^exp * (1 - hardness)

fn apply_glacial_erosion(state, hardness, params) {
    for (x, y) in all_cells {
        let u_b = *state.sliding_velocity.get(x, y);
        let h = *state.ice_thickness.get(x, y);

        if u_b <= 0.0 || h < 10.0 { continue; }

        // Erosion rate from sliding
        let ice_factor = (h / 200.0).clamp(0.1, 1.5);
        let erosion_rate = params.erosion_coeff * u_b.powf(params.erosion_exp) * ice_factor;

        // Modulate by rock hardness
        let hardness_factor = 1.0 - *hardness.get(x, y);
        let erosion = erosion_rate * hardness_factor * params.dt;

        state.bedrock.set(x, y, *state.bedrock.get(x, y) - erosion);
    }
}
```

---

## System Interactions

### River → Hydraulic

- Rivers create **large-scale** drainage structure
- Hydraulic droplets add **fine detail** around those channels
- Together: realistic dendritic networks with varied texture

### Erosion → Water Bodies

- Erosion shapes the topography
- Water detection uses the final eroded heightmap
- Result: lakes and rivers positioned realistically

### Rock Hardness → All Erosion

```
Material       Hardness   Erosion Factor
─────────────────────────────────────────
Basalt         0.95       0.05 (resistant)
Granite        0.85       0.15
Limestone      0.50       0.50
Sandstone      0.30       0.70
Sediment       0.10       0.90 (easily eroded)
```

Different rock types create varied terrain:
- Hard granite maintains steep slopes
- Soft sedimentary lowlands get heavily carved

### Temperature → Glacial Extent

- Cold regions form glaciers
- Glaciers carve U-shaped valleys
- Creates alpine features: cirques, arêtes, fjords

---

## Module Reference

| File | Purpose |
|------|---------|
| `src/erosion/rivers.rs` | D8 routing, flow accumulation, river tracing, sediment transport |
| `src/erosion/river_geometry.rs` | Bezier curves, meandering, confluences |
| `src/erosion/hydraulic.rs` | Water droplet particle simulation |
| `src/erosion/glacial.rs` | Shallow Ice Approximation, U-valleys |
| `src/erosion/materials.rs` | Rock hardness values |
| `src/erosion/params.rs` | All tunable parameters |
| `src/erosion/geomorphometry.rs` | Validation metrics (Horton's law, etc.) |
| `src/water_bodies.rs` | Ocean/lake/river detection, fantasy biomes |

---

## Erosion Pipeline Order

From `src/erosion/mod.rs`:

1. **River erosion** - Creates main drainage channels
2. **Hydraulic erosion** - Adds detailed erosion patterns
3. **Glacial erosion** - U-shaped valleys and fjords
4. **Depression filling** - Ensures connectivity
5. **River carving** - Enforces monotonic descent
6. **Final depression fill** - Connectivity check

---

## Validation Metrics

The system validates against real-world hydrological laws:

| Metric | Target | Description |
|--------|--------|-------------|
| Horton's bifurcation ratio | 3-5 | Ratio of stream counts between orders |
| Hack's Law exponent | 0.5-0.6 | Stream length vs drainage area |
| Flint's concavity | 0.4-0.7 | Slope-area relationship |
| Drainage density | varies | Total stream length per unit area |
| Pit count | 0 | Unfilled depressions |

---

## Example: River Lifecycle

1. **Source**: Identified at 450m elevation, flow_acc=150
2. **Headwaters**: Erodes V-shaped channel, sediment=10
3. **Mid-reach**: Flow increases to 300, channel widens
4. **Confluence**: Tributary joins, combined flow=500
5. **Floodplain**: Sediment exceeds capacity → deposits levees
6. **Lowlands**: Gentle slope, net deposition
7. **Delta**: Reaches sea level, fans out sediment

Result: Realistic river with tributaries, widening downstream, and delta at mouth.
