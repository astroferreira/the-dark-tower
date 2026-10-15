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
8. [Water Body Detection](#water-body-detection)
9. [Water Depth Tracking](#water-depth-tracking)
10. [Bezier River Geometry](#bezier-river-geometry)
11. [Hydraulic Erosion](#hydraulic-erosion)
12. [Glacial Erosion](#glacial-erosion)
13. [System Interactions](#system-interactions)
14. [Module Reference](#module-reference)

---

## Overview

The river system uses physically-based algorithms to create realistic drainage networks:

- **D8 Flow Direction**: Determines where water flows from each cell
- **Flow Accumulation**: Calculates drainage area (proxy for discharge)
- **Sediment Transport**: Rivers erode uplands and deposit in lowlands
- **Depression Filling**: Computes water surface level for lake detection
- **Water Level Detection**: Identifies alpine lakes using filled terrain comparison
- **Multi-scale Erosion**: Hydraulic droplets + glacial ice sheets

The result is dendritic river networks with proper tributaries, widening downstream, floodplains, deltas, and realistic alpine lakes.

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
- `source_min_elevation`: 100m
- `source_min_accumulation`: 15.0 (scaled by simulation_scale)

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

### Sea Level Constraint

Rivers must not erode below sea level. All erosion functions clamp to a minimum height:

```rust
const MIN_RIVER_HEIGHT: f32 = 0.1;  // Just above sea level

fn apply_erosion(heightmap, x, y, erosion, ...) {
    let current = *heightmap.get(x, y);

    // Clamp erosion to not dig below sea level
    let max_possible_erosion = (current - MIN_RIVER_HEIGHT).max(0.0);
    let actual_erosion = erosion.min(max_possible_erosion);

    heightmap.set(x, y, current - actual_erosion);
}
```

This constraint is applied in:
- `apply_erosion()` - V-shaped channel carving
- `apply_meander_erosion()` - Outer bank erosion
- `carve_river_network()` - Channel depth calculation
- `enforce_monotonic_descent()` - Downstream lowering

### Monotonic Descent Enforcement

Critical for river connectivity:

```rust
const MIN_RIVER_HEIGHT: f32 = 0.1;
let min_drop = 0.05;

if next_height >= current_height - min_drop {
    // Force downstream cell to be strictly lower, but not below sea level
    let new_height = (current_height - min_drop).max(MIN_RIVER_HEIGHT);
    heightmap.set(nx, ny, new_height);
}
```

This prevents pits and ensures continuous flow to the ocean while keeping rivers above sea level.

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

## Key River Parameters

**File**: `src/erosion/params.rs`

| Parameter | Default | Effect |
|-----------|---------|--------|
| `river_source_min_accumulation` | 15.0 | Flow threshold for river sources |
| `river_source_min_elevation` | 100m | Minimum source elevation |
| `river_capacity_factor` | 20.0 | Sediment carrying capacity |
| `river_erosion_rate` | 0.5 | Erosion speed (reduced from 1.0) |
| `river_deposition_rate` | 0.5 | Deposition speed |
| `river_max_erosion` | 30.0 | Maximum erosion per cell (reduced from 150.0) |
| `river_max_deposition` | 0.0 | Maximum deposition per cell |
| `river_channel_width` | 2 | Base half-width in cells |

**Sea Level Protection**: All erosion functions enforce `MIN_RIVER_HEIGHT = 0.1m` to prevent rivers from digging below sea level. This prevents river channels from being misclassified as ocean.

---

## Depression Filling

**File**: `src/erosion/rivers.rs`

The **Planchon-Darboux algorithm** computes water surface levels by filling depressions. This is critical for detecting alpine lakes.

### Algorithm

```rust
pub fn fill_depressions(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
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

### Water Level vs Terrain

The key insight: comparing `water_level` to `terrain_height` reveals submerged areas:

```
water_level > terrain_height  →  Submerged (lake or ocean)
water_level == terrain_height →  Dry land
```

This correctly identifies:
- **Ocean floor**: Below sea level + connected to map edges
- **Alpine lakes**: Above sea level but trapped in depressions
- **Inland seas**: Below sea level but not connected to ocean

---

## Water Body Detection

**File**: `src/water_bodies.rs`

The water body detection system uses the filled terrain (water_level) to identify all water bodies, including alpine lakes at high elevations.

### Algorithm Overview

```rust
pub fn detect_water_bodies(
    heightmap: &Tilemap<f32>,
) -> (Tilemap<WaterBodyId>, Vec<WaterBody>, Tilemap<f32>) {
    // Step 1: Compute water surface level (fills depressions)
    let water_level = compute_water_level(heightmap);  // Planchon-Darboux

    // Step 2: Compute flow for river detection
    let flow_dir = compute_flow_direction(heightmap);
    let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

    // Step 3: Detect all water bodies
    detect_water_bodies_full(heightmap, &water_level, &flow_acc)
}
```

### Step-by-Step Detection

**Step 1: Identify Water Candidates**

A tile is a water candidate if:
- **Submerged**: `water_level > terrain_height` (alpine lakes)
- **Below sea level**: `terrain_height <= 0.0` (ocean/coastal)

```rust
fn is_submerged(terrain_h: f32, water_h: f32) -> bool {
    water_h > terrain_h + WATER_EPSILON  // 1e-4
}

for (x, y) in all_cells {
    let terrain_h = *heightmap.get(x, y);
    let water_h = *water_level.get(x, y);

    if is_submerged(terrain_h, water_h) || is_below_sea_level(terrain_h) {
        is_water_candidate.set(x, y, true);
    }
}
```

**Step 2: Ocean Detection (BFS from edges)**

Ocean = below-sea-level tiles connected to map edges:

```rust
// Seed from top/bottom edges (polar regions)
for x in 0..width {
    if is_below_sea_level(heightmap.get(x, 0)) {
        queue.push_back((x, 0));
    }
    if is_below_sea_level(heightmap.get(x, height-1)) {
        queue.push_back((x, height-1));
    }
}

// BFS flood-fill (only follows below-sea-level tiles)
while let Some((x, y)) = queue.pop_front() {
    water_map.set(x, y, WaterBodyId::OCEAN);

    for (nx, ny) in neighbors(x, y) {
        if is_below_sea_level(heightmap.get(nx, ny)) && !visited.get(nx, ny) {
            queue.push_back((nx, ny));
        }
    }
}
```

**Step 3: Lake Detection (remaining water candidates)**

Lakes = water candidates NOT connected to ocean:

```rust
for (x, y) in unvisited_water_candidates {
    let lake_id = WaterBodyId(next_id);
    next_id += 1;

    // BFS to find all tiles in this lake
    let mut lake = WaterBody::new(lake_id, WaterBodyType::Lake);
    flood_fill_lake(&mut lake, x, y);

    // Reclassify as ocean if touches edge AND below sea level
    if (lake.touches_north_edge || lake.touches_south_edge)
        && lake.min_elevation <= SEA_LEVEL {
        reclassify_as_ocean(lake);
    } else {
        water_bodies.push(lake);
    }
}
```

**Step 4: River Detection (dry land with high flow)**

Rivers are NOT submerged - they flow on the surface:

```rust
const RIVER_FLOW_THRESHOLD: f32 = 50.0;

for (x, y) in all_cells {
    // Only mark as river if:
    // 1. Not already water (lake/ocean)
    // 2. High flow accumulation
    if water_map.get(x, y).is_none() {
        let flow = *flow_acc.get(x, y);
        if flow >= RIVER_FLOW_THRESHOLD {
            water_map.set(x, y, river_id);
        }
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
    pub touches_north_edge: bool,
    pub touches_south_edge: bool,
    pub bounds: (usize, usize, usize, usize),  // min_x, min_y, max_x, max_y
}
```

### Alpine Lake Example

A mountain valley at 3000m elevation with a depression:
- Terrain: 3000m at edges, 2900m at center
- Water level (filled): 3000m everywhere
- Center is submerged: water_level (3000m) > terrain (2900m)
- Result: Alpine lake at 2900-3000m elevation with 100m depth

---

## Water Depth Tracking

**File**: `src/water_bodies.rs`, `src/world.rs`

Water depth is calculated as the difference between water surface and terrain:

```rust
// Calculate water depth everywhere
for (x, y) in all_cells {
    let terrain_h = *heightmap.get(x, y);
    let water_h = *water_level.get(x, y);
    let depth = (water_h - terrain_h).max(0.0);  // Positive = submerged
    water_depth.set(x, y, depth);
}
```

### TileInfo Structure

```rust
pub struct TileInfo {
    pub elevation: f32,      // Terrain height relative to sea level
    pub water_depth: f32,    // Water depth above terrain (0 = dry)
    pub water_body_type: WaterBodyType,
    // ... other fields
}
```

### Explorer Display

The explorer shows both elevation and water depth:

```rust
// Always show terrain elevation relative to sea level
let elev_str = format!("  Elevation: {:.0}m", tile.elevation);
lines.push((elev_str, Style::default().fg(Color::White)));

// Show water depth if tile is submerged
if tile.water_depth > 0.0 {
    let depth_str = format!("  Water Depth: {:.0}m", tile.water_depth);
    lines.push((depth_str, Style::default().fg(Color::Cyan)));
}
```

### Interpretation

| Tile Type | Elevation | Water Depth |
|-----------|-----------|-------------|
| Mountain Peak | 4000m | 0m |
| Alpine Lake Bed | 2900m | 100m |
| Ocean Floor | -5000m | 5000m |
| Coastal Beach | 5m | 0m |
| River Bed | 200m | 0m (rivers are surface flow) |

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

Particle-based water droplet simulation for fine detail. See [EROSION.md](EROSION.md) for complete details.

### Key Parameters ("POLISHED" Config)

| Parameter | Default | Effect |
|-----------|---------|--------|
| `hydraulic_iterations` | 750,000 | Number of droplets |
| `droplet_inertia` | 0.3 | Low inertia - meanders naturally |
| `droplet_deposit_rate` | 0.2 | Moderate - forces river merging |
| `droplet_erosion_radius` | 3 | Medium brush - breaks parallel streams |
| `droplet_evaporation` | 0.001 | Low - long-lived droplets |

---

## Glacial Erosion

**File**: `src/erosion/glacial.rs`

Uses the **Shallow Ice Approximation (SIA)** to simulate glacier flow and U-shaped valley carving. See [EROSION.md](EROSION.md) for complete details.

---

## System Interactions

### Erosion → Water Bodies

1. Erosion shapes the topography
2. Depression filling computes water surface levels
3. Water detection compares water_level vs terrain
4. Result: lakes positioned realistically at any elevation

### River → Lake Interaction

- Rivers carve channels into terrain
- Depression filling identifies where water pools
- Alpine lakes form in high-elevation depressions
- Rivers drain from lakes, not into them (monotonic descent)

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

---

## Module Reference

| File | Purpose |
|------|---------|
| `src/erosion/rivers.rs` | D8 routing, flow accumulation, river tracing, sediment transport, meander erosion |
| `src/erosion/river_geometry.rs` | Bezier curves, meandering, confluences |
| `src/erosion/hydraulic.rs` | Water droplet particle simulation |
| `src/erosion/glacial.rs` | Shallow Ice Approximation, U-valleys |
| `src/erosion/materials.rs` | Rock hardness values |
| `src/erosion/params.rs` | All tunable parameters |
| `src/erosion/geomorphometry.rs` | Validation metrics (Horton's law, etc.) |
| `src/water_bodies.rs` | Water level detection, ocean/lake/river classification, water depth |

---

## Erosion Pipeline Order

From `src/erosion/mod.rs`:

1. **Upscale heightmap** - 4x resolution with terrain roughness
2. **Pre-erosion blur** - Gaussian blur to melt sharp ridges
3. **River erosion** - Creates main drainage channels
4. **Hydraulic erosion** - Adds detailed erosion patterns
5. **Glacial erosion** - U-shaped valleys and fjords
6. **Depression filling** - Ensures connectivity
7. **River carving** - Enforces monotonic descent
8. **Meander erosion** - 12 passes for natural curves
9. **Final depression fill** - Connectivity check
10. **Smart downscale** - Variance-based river preservation

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

---

## Example: Alpine Lake Formation

1. **Terrain**: Mountain range at 3500m with glacial cirque
2. **Depression**: Bowl-shaped valley at 3200m surrounded by 3500m ridges
3. **Erosion**: Glacial erosion deepens the cirque
4. **Water Level**: Depression filling computes water surface at 3500m (rim height)
5. **Detection**: `water_level (3500m) > terrain (3200m)` = submerged
6. **Result**: Alpine lake at 3200m elevation with 300m maximum depth

The lake is correctly detected even though it's 3000+ meters above sea level.
