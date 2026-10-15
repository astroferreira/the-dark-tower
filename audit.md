This technical and geophysical audit of the terrain generation pipeline in `src.zip` covers **pipeline causality**, **spherical metric geometry**, **geomorphological equations**, and **hidden bugs** currently limiting realism, along with drop-in mathematical and Rust implementations.

---

## 1. Pipeline Architecture & Causal Ordering (`terrain.rs`)

### 1.1 Destructive Post-Erosion Passes Overwrite Graded Fluvial Topology

* **What the code does now:** In `terrain::generate_terrain`, the physically coupled evolution (`erosion::landscape::evolve`) and particle/river/glacial erosion (`erosion::simulate_erosion`) execute at **Stages 3 and 4**. Afterward, **Stages 5 through 9** (`coastline`, `fjords`, `apply_regional_noise_stacks`, `apply_volcano_pass`, `apply_island_coasts`, `apply_coastal_beaches`) directly mutate `heightmap`.


* **Why it hurts realism:**
1. `apply_regional_noise_stacks` adds up to `150.0 * stress` meters of raw Perlin noise to mountains, `40.0` m to forests, and `20.0` m to floodplains *after* erosion has finished. Fluvial erosion obeys Playfair’s Law: valleys are carved into monotonically descending, concave-up longitudinal profiles ($S \propto A^{-\theta}$). Superimposing 40–150m of un-eroded Perlin noise onto carved valleys turns graded riverbeds into bumpy, pitted rollercoasters.


2. At the very end, `erosion::landscape::fill_pits(&mut heightmap, 4, 10.0)` only fills depressions up to **4 cells in area and 10m deep**. Any noise hump taller than 10m or wider than 4 cells permanently dams the river network or creates unnatural endorheic artifacts.


3. Volcanoes (`1000.0` to `4000.0` m tall cones over a $7 \times 7$ footprint) are pasted on top of the final terrain without any radial gullies, lahars, or ring-plain drainage deflection around their bases.




* **How to make it 100x better:**
* **Move structural & constructive passes BEFORE erosion:** Reorder `generate_terrain` so that **Tectonics $\rightarrow$ Volcanoes $\rightarrow$ Regional Noise Stacks $\rightarrow$ Island Coasts $\rightarrow$ Coastline Jitter $\rightarrow$ Climate $\rightarrow$ Landscape Evolution & Erosion $\rightarrow$ Fjords $\rightarrow$ Coastal Beaches $\rightarrow$ Final Pit Fill & Flow Routing**.


* When `apply_regional_noise_stacks` and `apply_volcano_pass` run *before* `landscape::evolve`, stream-power incision naturally carves radial barrancos (erosional gullies) down stratovolcano flanks and integrates the regional noise into realistic ridge-and-ravine drainage networks.



### 1.2 Stale `flow_accumulation` and Hardcoded `0.3` Rock Hardness

* **What the code does now:** In `terrain.rs`, when `cfg.legacy_erosion == false` (which is the **default** in `TerrainConfig::new`), the pipeline executes:
```rust
let (hardness, flow_accumulation) = if !cfg.legacy_erosion {
    println!("Legacy erosion skipped");
    let (_, acc, _) = erosion::rivers::compute_flow_with_filled_routing(&heightmap);
    (Tilemap::new_with(width, height, 0.3f32), acc)
}

```


and never updates `flow_accumulation` after Stages 5–10 modify the heightmap.


* **Why it hurts realism:**
1. **Stale River Map:** `Terrain.flow_accumulation` reflects the pre-coastline, pre-fjord, pre-volcano, pre-beach, pre-pit-fill heightmap. Rivers in `flow_accumulation` can flow straight through the summit of a newly placed 4,000m volcano or terminate mid-land where coastline jittering shifted the shore.


2. **Broken Lithology/Porosity:** Because `hardness` defaults to a flat `0.3f32` everywhere when `legacy_erosion == false`, downstream systems like `underground_water::detect_aquifers` (which computes `let porosity = 1.0 - hard;`) see a uniform $70\%$ rock porosity across the entire planet—treating granite cratons, basaltic shields, and alluvial floodplains identically!




* **How to make it 100x better:**
* Derive a physical `hardness` map directly from `CrustFields` (e.g., cratonic/metamorphic shields & igneous intrusions $\approx 0.75\text{–}0.90$, fold-belt sed-metamorphic $\approx 0.50\text{–}0.70$, foreland/shelf sedimentary basins & alluvium $\approx 0.15\text{–}0.35$).


* Feed that spatially varying lithological erodibility $K(x,y) = K_0 (1 - \text{hardness}(x,y))^2$ into `landscape::evolve` so rivers preferentially excavate soft sedimentary valleys around hard crystalline massifs (creating water gaps and hogback ridges).
* Recompute `let (_, flow_accumulation, _) = erosion::rivers::compute_flow_with_filled_routing(&heightmap);` at the **`final`** stage after `fill_pits`.





### 1.3 Missing Post-Evolution Orographic & Lapse-Rate Climate Feedback

* **What the code does now:** `climate::run_climate_simulation` runs once at Stage 2. Later stages alter elevations by hundreds to thousands of meters (`landscape::evolve`, `relevel_to_volume`, `apply_volcano_pass`, `apply_fjord_incisions`), but `climate_sim` is never updated.


* **How to make it 100x better:**
* After the final heightmap is locked in, apply a fast **adiabatic lapse-rate and coastal-mask correction** to `climate_sim.mean_temperature` and `seasonal_temperatures`:

$$T_{\text{final}}(x,y) = T_{\text{stage2}}(x,y) - \Gamma \cdot \left(\max(0, h_{\text{final}}(x,y)) - \max(0, h_{\text{stage2}}(x,y))\right)$$



where $\Gamma \approx 0.0065\text{ }^\circ\text{C/m}$ ($6.5\text{ }^\circ\text{C/km}$). This ensures newly erupted stratovolcanoes and uplifted cordilleras immediately receive alpine snow caps and glacial biomes rather than retaining lowland temperatures.





---

## 2. Spherical Metric Geometry & Differential Operators (`tilemap.rs`)

### 2.1 Critical Sign-Inversion Bug in `DirectionalContext` Curvature (`is_ridge` / `is_valley`)

* **What the code does now:** In `Tilemap<f32>::analyze_directional_context_weighted` (`tilemap.rs`):
```rust
let neighbor_avg = neighbor_sum / total_weight;
let curvature = neighbor_avg - center_value;

```


And in `DirectionalContext`:
```rust
/// Curvature (Laplacian) - positive = convex/ridge, negative = concave/valley
pub curvature: f32,
...
pub fn is_ridge(&self, slope_threshold: f32) -> bool {
    self.gradient_magnitude > slope_threshold && self.curvature > 0.0
}
pub fn is_valley(&self, slope_threshold: f32) -> bool {
    self.gradient_magnitude > slope_threshold && self.curvature < 0.0
}

```



* **Why this is a critical bug:**
* At a **ridge or peak**, `center_value` is **greater** than `neighbor_avg`, which makes `neighbor_avg - center_value` **NEGATIVE** ($\nabla^2 h < 0$), not positive!


* At a **valley or depression**, `center_value` is **less** than `neighbor_avg`, which makes `neighbor_avg - center_value` **POSITIVE** ($\nabla^2 h > 0$), not negative!


* Every caller of `is_ridge()` and `is_valley()` in the codebase is currently classifying valleys as ridges and ridges as valleys.


* Furthermore, `weights.center` (`4.0`) is defined in `DirectionalWeights` but completely unused.




* **How to make it 100x better:**
* Flip the definition to `let curvature = center_value - neighbor_avg;` (so positive = convex ridge/peak, negative = concave valley/hollow).


* Better yet, decompose curvature into **Planform (Contour) Curvature** $\kappa_c$ (which measures convergence/divergence of flow lines across hillslopes—identifying ridges vs. swales even on steep slopes) and **Profile Curvature** $\kappa_p$ (which measures acceleration/deceleration downslope—identifying cliff breaks vs. footslope concavities) using the $3\times 3$ finite-difference second derivatives ($z_{xx}, z_{yy}, z_{xy}$).



### 2.2 Equirectangular Latitude Distortion ($\cos\phi$ Metric Missing)

* **What the code does now:** `Tilemap` uses an equirectangular projection that wraps horizontally. However, `analyze_directional_context_weighted`, `compute_flow_direction_at`, `get_local_gradient`, and `gaussian_blur` treat $\Delta x$ and $\Delta y$ as equal Euclidean distances at all latitudes.


* **Why it hurts realism:** On a sphere, the physical east-west spacing of a cell at row $y$ is $\Delta x(\phi) = \Delta y \cdot \cos\phi$, where latitude $\phi = \frac{\pi}{2} - \frac{(y + 0.5)\pi}{H}$. At $\phi = 60^\circ$, $\Delta x = 0.5\,\Delta y$; at $\phi = 75^\circ$, $\Delta x \approx 0.26\,\Delta y$.
* Because `gx` is not divided by $\cos\phi$, east-west slopes at high latitudes are underestimated by $2\times\text{ to }4\times$, biasing D8 river routing (`compute_flow_direction_at`) to flow North-South instead of East-West!


* In `gaussian_blur`, the horizontal convolution pass blurs across $4\times$ more physical kilometers near the poles than at the equator.




* **How to make it 100x better:**
* Scale horizontal differences by $\sec\phi = 1 / \cos(\phi).\max(0.15)$:
```rust
let lat = std::f32::consts::FRAC_PI_2 - (y as f32 + 0.5) / self.height as f32 * std::f32::consts::PI;
let inv_cos_lat = 1.0 / lat.cos().max(0.15);
let gradient = ((gx / weight_sum) * inv_cos_lat, gy / weight_sum);

```


* In `compute_flow_direction_at`, compute physical step distance as:

$$\text{dist}(dx, dy, \phi) = \sqrt{(dx \cdot \cos\phi)^2 + dy^2}$$





### 2.3 Longitudinal Seam in `upscale_with_detail` & `upscale_for_erosion`

* **What the code does now:** Both upscalers in `tilemap.rs` sample 2D Perlin noise using planar coordinates `[new_x as f64 * ..., new_y as f64 * ...]`.


* **Why it hurts realism:** At the map wrap boundary (`new_x = 0` vs `new_x = new_width - 1`), the 2D noise coordinates jump discontinuously, creating a visible vertical seam in upscaled terrain and valley meanders.


* **How to make it 100x better:** Sample 3D Perlin noise on the unit sphere $p = [\cos\phi\cos\lambda, \cos\phi\sin\lambda, \sin\phi]$ just as `apply_island_coasts` does.



---

## 3. Tectonic Geomorphology, Bathymetry & Landforms (`heightmap.rs`)

### 3.1 Spherical vs. Cylindrical 3D Noise Coordinates (`cylindrical_coords`)

* **What the code does now:** `cylindrical_coords(nx, ny, radius)` maps longitude to a circle `(cos(angle), sin(angle))` of *constant* radius $R$ from pole to pole.


* **Why it hurts realism:** While a cylinder eliminates the longitudinal seam at `nx = 0 / 1`, it does not converge at the poles ($\cos\phi \to 0$). Mapping a cylinder onto a sphere pinches all high-latitude noise features into narrow longitudinal "spokes" radiating from the poles.
* **How to make it 100x better:** Replace `cylindrical_coords` with **true rotated spherical coordinates** (which you already wrote in `apply_island_coasts`!):


```rust
#[inline]
fn spherical_coords(nx: f64, ny: f64, radius: f64) -> [f64; 3] {
    let lon = nx * std::f64::consts::TAU;
    let lat = std::f64::consts::FRAC_PI_2 - ny * std::f64::consts::PI;
    let cos_lat = lat.cos();
    let sx = cos_lat * lon.cos() * radius;
    let sz = cos_lat * lon.sin() * radius;
    let sy = lat.sin() * radius;
    // Keep your 20-degree axis tilt to avoid Perlin integer lattice alignment!
    let (cos_a, sin_a) = (0.93969, 0.34202);
    [sx + 11.234, sy * cos_a - sz * sin_a + 17.382, sy * sin_a + sz * cos_a + 31.914]
}

```


This guarantees isotropic, distortion-free continental lobes, mountain ridges, and archipelagos at every latitude from the equator to the poles.



### 3.2 Chebyshev ($L_\infty$) Square-Box Artifacts in Distance Fields

* **What the code does now:** In `compute_continental_distance` and `compute_coast_distance`, the BFS loops over 8 neighbors (`plate_map.neighbors_8(x, y)`) and increments distance by `let new_dist = dist + 1.0;` for both cardinal and diagonal neighbors.


* **Why it hurts realism:** Setting diagonal step cost equal to cardinal step cost ($1.0$ instead of $\sqrt{2}$) computes the **Chebyshev ($L_\infty$) norm** $\max(\vert{}\Delta x\vert{}, \vert{}\Delta y\vert{})$ instead of Euclidean distance $\sqrt{\Delta x^2 + \Delta y^2}$. Distance waves propagate $41.4\%$ faster along $45^\circ$ diagonals, imprinting square/octagonal coastal gradients and shelf contours onto `raw_coast_dist`.


* **How to make it 100x better:** Replace the unweighted BFS with an **exact $O(N)$ Euclidean Distance Transform (Felzenszwalb & Huttenlocher 2012)** or a **2-pass Chamfer / Dijkstra priority queue** with spherical step weights $\Delta s = \sqrt{(\Delta x \cos\phi)^2 + \Delta y^2}$.

### 3.3 Gradient/Derivative-Weighted Ridged Multifractal (`ridged_fbm_3d`)

* **What the code does now:** `ridged_fbm_3d` sums `ridge = (1.0 - n.abs()).max(0.0).powf(power)` with constant octave weights `amplitude *= persistence`.


* **Why it hurts realism:** Standard unweighted ridged fBm produces uniform high-frequency "wrinkles" everywhere—even in the middle of flat intermontane valleys—and lacks the self-similar branching of real alpine arêtes.
* **How to make it 100x better:** Implement **Musgrave's Heterogeneous Signal-Weighted Ridged Multifractal** combined with **Analytic Derivative Damping**:

$$R_i(\mathbf{p}) = \left(1 - \vert{}N(\mathbf{p} \cdot f_i)\vert{}\right)^p \cdot w_{i-1}, \qquad w_i = \text{clamp}(R_i \cdot G, 0, 1)$$



where each octave's contribution is multiplied by the previous octave's height $w_{i-1}$ (with gain $G \approx 1.8$). This ensures fine craggy arêtes only emerge high up on major ridge spines, while valley bottoms between ridges remain smooth and U/V-shaped.

### 3.4 Orogenic Asymmetry, Foreland Basins & Rain-Shadow Plateaus (`generate_continental_elevation`)

* **What the code does now:** In `generate_continental_elevation`, interior mountain belts are placed via an arbitrary noise mask (`orogen_corridor = fbm_3d(ridge_noise, ...)`) and active tectonic uplift is a symmetric function of `stress > 0.04`.


* **Why it hurts realism:**
1. Real mountain ranges do not form in random Perlin blotches; they form along current and suture-scar plate boundaries.
2. Real collisional/subduction orogens have a distinct **cross-sectional asymmetry**:
* **Trench / Suture $\rightarrow$ Steep Pro-Wedge:** Rapid elevation rise close to the plate boundary.
* **Orogenic Plateau (Tibet / Altiplano):** Where convergence is wide and intense, crustal thickening reaches an isostatic ceiling ($\sim 4500\text{–}5500\text{ m}$) forming a high, flat-ish internal plateau ringed by peaks, rather than infinitely spiky cones.
* **Flexural Foreland Basin (Indo-Gangetic Plain / Llanos / Po Valley):** The immense weight of the thrust belt bends the adjacent continental lithosphere downward via elastic flexure:

$$w(x) = w_0 e^{-x/\alpha}\cos(x/\alpha)$$



where $\alpha = \left(\frac{4D}{\Delta\rho g}\right)^{1/4} \approx 80\text{–}150\text{ km}$ is the flexural parameter. This creates a broad, low-elevation alluvial trough immediately inland of major mountain ranges that captures major trunk rivers (like the Ganges, Po, or Paraná)!





### 3.5 Hypsometric Curve & Parsons–Sclater Ocean Bathymetry (`generate_oceanic_elevation`)

* **What the code does now:** `generate_oceanic_elevation` linearly blends from `SHELF_DEPTH` (`-150.0` m) to `OCEAN_FLOOR` (`-5000.0` m) over `25.0` units, and adds mid-ocean ridges only right where `stress < -0.06`.


* **How to make it 100x better:**
* **Sigmoidal Shelf–Slope–Rise Profile:** Replace the linear blend `(1.0 - t)` with a realistic continental margin profile:
1. **Continental Shelf** ($0 \le d < d_{\text{shelf}}$): Very gentle slope from $0\text{ m}$ to $-130\text{ m}$ ($t^{0.6}$).
2. **Shelf Break & Continental Slope** ($d_{\text{shelf}} \le d < d_{\text{slope}}$): Steep sigmoid drop from $-130\text{ m}$ to $-3200\text{ m}$.
3. **Continental Rise** ($d_{\text{slope}} \le d < d_{\text{rise}}$): Exponentially decaying turbidite apron $\exp(-k(d - d_{\text{slope}}))$ grading smoothly into the abyssal plain at $-4800\text{ m}$.


* **Fracture Zones:** Offset mid-ocean ridge axes with perpendicular transform fault troughs using domain-warped step functions along the spreading vector.



### 3.6 Drainage-Guided Fjord Carving with Terminal Sills (`apply_fjord_incisions`)

* **What the code does now:** `apply_fjord_incisions` carves wherever a 3D ridged noise function `fjord_network >= 0.62` within 24 tiles of water at latitudes $> 36^\circ$.


* **Why it hurts realism:** Noise-based carving slices across ridges, creates isolated inland saltwater holes disconnected from the ocean, and lacks the classic **U-shaped glacial profile** and **terminal moraine sill**.
* **How to make it 100x better:**
* Instead of evaluating random Perlin noise per pixel, **trace upstream from coastal river mouths** (cells at `ocean_distance <= 1.5` with high `flow_accumulation` at latitudes $\vert{}\phi\vert{} > 38^\circ$ or coastal mountains $> 600\text{ m}$).


* Along each glaciated coastal valley trunk (and its main tributaries), carve a parabolic U-shaped cross-section of half-width $W \propto Q_{\text{ice}}^{0.35}$:

$$\Delta h(r, s) = -D_{\max} \cdot \underbrace{\left(1 - \left(\frac{r}{W}\right)^2\right)}_{\text{Parabolic U-trough}} \cdot \underbrace{\left(0.25 + 0.75\sin\left(\pi \frac{s}{L}\right)\right)}_{\text{Overdeepened inner basin + shallow mouth sill}}$$



where $r$ is cross-valley distance and $s \in [0, L]$ is distance inland from the fjord mouth. This guarantees every fjord is a continuous, drowned glacial valley connected to the sea, shallow at the entrance sill and deeply overdeepened inland!



### 3.7 Volcano Geometry & Two Bugs in Lava Flow (`mark_volcano_tiles` & `generate_lava_map`)

* **What the code does now:**
1. `mark_volcano_tiles` applies the exact same Gaussian `(-(r / 1.5).powi(2)).exp()` to all volcanoes, ignoring `volcano.volcano_type` (`Shield`, `Stratovolcano`, `Caldera`).


2. In `generate_lava_map`, `get_neighbors_8` wraps `ny` vertically around the poles:
```rust
let ny = (y as isize + dy).rem_euclid(height as isize) as usize;

```


and allows lava to flow **uphill by 50m per step** (`if neighbor_height <= current_height + 50.0`) using a `Vec::pop()` (depth-first stack instead of breadth-first queue)!




* **How to make it 100x better:**
1. **Differentiate Volcano Profiles in `mark_volcano_tiles`:**
```rust
let (radius, profile) = match volcano.volcano_type {
    VolcanoType::Shield => {
        let r_max = 4.0f32;
        (r_max, (-(r / 2.4).powi(2)).exp()) // Broad, gentle dome
    }
    VolcanoType::Stratovolcano => {
        let r_max = 2.5f32;
        (r_max, (1.0 - (r / r_max).min(1.0)).powf(1.8)) // Steep concave cone
    }
    VolcanoType::Caldera => {
        let r_max = 3.5f32;
        // Ring wall peak at r ~ 1.0, collapsed central crater at r == 0
        let ring = (-( (r - 1.0) / 1.1 ).powi(2)).exp();
        let collapse = if r < 0.8 { -0.35 * (1.0 - (r / 0.8).powi(2)) } else { 0.0 };
        (r_max, (ring + collapse).max(-0.2))
    }
};

```


2. **Fix `generate_lava_map`:**
* Replace `get_neighbors_8` with `heightmap.neighbors_8(x, y)` so `ny` clamps at the poles instead of teleporting across the planet.


* Replace `Vec` with `std::collections::VecDeque` (`pop_front()`) so BFS processes distance rings in order ($d = 1$, then $d = 2$, then $d = 3$).


* Change the uphill condition `neighbor_height <= current_height + 50.0` to strictly downhill `neighbor_height < current_height` (lava cannot climb 50m uphill!).







---

## 4. Hydrogeology, Aquifers & Waterfalls (`underground_water.rs`)

### 4.1 Replace Elevation-Proportional Water Tables with Topographic Wetness Index (TWI)

* **What the code does now:**
```rust
fn calculate_aquifer_depth(elevation: f32, porosity: f32, stress: f32, params: &UndergroundWaterParams) -> f32 {
    let base_depth = elevation * params.depth_scale * 0.1;
    ...
}
fn determine_aquifer_type(depth: f32, elevation: f32, stress: f32, porosity: f32) -> AquiferType {
    if depth < 10.0 && porosity > 0.4 { return AquiferType::Unconfined; }
    if depth > 30.0 && stress > 0.2 { return AquiferType::Confined; }
    ...
}

```



* **Why it hurts realism:**
* Because `base_depth = elevation * 0.05`, a high mountain peak at $2000\text{ m}$ gets `depth > 30.0`, and because mountains have `stress > 0.2`, **every mountain range is classified as a `Confined` aquifer**, whereas low-lying sedimentary basins (`elevation < 200m`) can never reach `depth > 30.0` and never become confined aquifers!


* Furthermore, `detect_aquifers` does not receive `flow_acc`, so a major river floodplain at $400\text{ m}$ elevation has the exact same water table depth as a dry ridge crest at $400\text{ m}$ elevation.




* **How to make it 100x better:**
* Pass `flow_acc` into `detect_aquifers` and compute the **Beven–Kirkby Topographic Wetness Index (TWI)**:

$$\text{TWI}(x,y) = \ln\left(\frac{a(x,y)}{\max(\tan\beta(x,y), 0.001)}\right)$$



where $a$ is specific catchment area (`flow_acc`) and $\tan\beta$ is local slope (`DirectionalContext.gradient_magnitude`).


* **Water Table Depth ($d_{\text{wt}}$):**

$$d_{\text{wt}}(x,y) = d_{\max} \cdot \exp\left(-c_1 \cdot \text{moisture} \cdot \text{porosity} \cdot \text{TWI}(x,y)\right)$$



Now, water tables naturally intersect the surface ($d_{\text{wt}} \to 0$) in wet, convergent valley bottoms (producing **Seepage Springs** and **Wetlands**), while sitting deep below steep, dry ridges!
* **Confined Artesian Aquifers & Piezometric Head:** Define confined aquifers in **synclinal / foreland basins** (`curvature < -5.0` where surrounding regional uplands within radius $R \approx 6\text{–}10$ tiles have higher mean elevation $\bar{h}_{\text{recharge}} > h(x,y) + 150\text{ m}$). An **Artesian Spring** erupts where the **piezometric head** $H_p = \bar{h}_{\text{recharge}} - \Delta h_{\text{head\_loss}}$ exceeds the local surface elevation $h(x,y)$ along a fault (`stress > 0.15`)!



### 4.2 Waterfall Drop Inflation & Lithological Knickpoints (`detect_waterfalls`)

* **What the code does now:**
```rust
let adjusted_drop = drop * (1.0 + hardness * 0.5);

```



* **Why it hurts realism:** Multiplying `drop` by `(1.0 + hardness * 0.5)` means a river dropping $20\text{ m}$ between two tiles reports a $30\text{ m}$ waterfall—violating conservation of elevation ($h_1 - h_2 = 20\text{ m}$).


* **How to make it 100x better:**
* Keep `drop_height = drop` physically exact (so the waterfall isn't taller than the cliff it falls over!), and instead use **lithological hardness contrast** ($\Delta H_{\text{rock}} = \text{hardness}_{\text{upstream}} - \text{hardness}_{\text{downstream}} > 0.15$) to lower the `waterfall_min_drop` threshold at caprock ledges (like Niagara Falls, where hard Lockport Dolomite overlies soft Rochester Shale) and hanging glacial tributary valleys.



---

## 5. Biome Feathering & Seasonal Climatology (`biome_feathering.rs` & `seasons.rs`)

### 5.1 Critical 1-Ring Bug in `biome_feathering::compute_blend_weights`

* **What the code does now:** `compute_distance_field` computes a BFS `depth_map` up to `config.max_depth` (default `8` tiles) from biome boundaries. Then `compute_blend_weights` loops over every tile with `depth < max_depth - 1.0` and looks for foreign biomes to blend with by checking **only its immediate 1-ring `neighbors_8**`:


```rust
let neighbors = biomes.neighbors_8(x, y);
for (nx, ny) in neighbors {
    let neighbor_biome = *biomes.get(nx, ny);
    if neighbor_biome == center_biome {
        continue;
    }
    ...
}

```



* **Why this is a critical bug:**
* If a tile is at `depth = 2, 3, 4, 5, 6, or 7` tiles away from a biome boundary, **all 8 of its immediate 1-ring neighbors have the exact same `center_biome` as the tile itself**!


* Consequently, `if neighbor_biome == center_biome { continue; }` skips all 8 neighbors, `biome_contributions` contains *only* `center_biome`, and after normalization (`w / total`), `center_biome` receives a weight of **`1.0`**!


* Despite computing an 8-tile `depth_map`, **`compute_blend_weights` only ever feathers a 1-tile-wide strip right on the border (`depth == 0`)**, leaving `depth = 1..8` completely unblended!




* **How to make it 100x better (in $O(N)$ time):**
* During the BFS in `compute_distance_field`, track and propagate `nearest_foreign_biome: Tilemap<ExtendedBiome>` (and optionally second-nearest) alongside `distance`:


```rust
// At boundary initialization (depth == 0):
// Set nearest_foreign_biome(x, y) to the most frequent foreign neighbor in neighbors_8(x, y).
// During BFS propagation from (x, y) -> (nx, ny):
let foreign = *nearest_foreign_biome.get(x, y);
nearest_foreign_biome.set(nx, ny, foreign);

```


* Then in `compute_blend_weights`, every tile at `depth = 1..max_depth` immediately knows its `nearest_foreign_biome` in $O(1)$ time without an $O(r^2)$ neighborhood scan, and smoothly blends `center_weight` against `1.0 - center_weight` across the entire 8-tile Catmull-Rom feathering zone!





### 5.2 Spherical Sunrise Hour Angle & Maritime Thermal Lag (`seasons.rs`)

* **What the code does now:**
1. `calculate_day_length` uses a linear ramp `0.5 + latitude_normalized * (adjusted_tilt - 0.5)`, which means 24-hour daylight (midnight sun) and 24-hour darkness (polar night) only happen at the single pixel row $y = 0$ / $y = H-1$ ($\pm 90^\circ$), rather than across the entire Polar Circle ($\vert{}\phi\vert{} \ge 66.56^\circ$).


2. `get_temperature` uses the same seasonal phase (`0.0` in Summer, `PI` in Winter) for both oceans and continental interiors.




* **How to make it 100x better:**
1. **Exact Sunrise Hour Angle Formula in `calculate_day_length`:**
```rust
let lat_rad = latitude_normalized * std::f32::consts::FRAC_PI_2 * if is_northern { 1.0 } else { -1.0 };
let axial_tilt_rad = 23.44_f32.to_radians();
let decl = match season {
    Season::Summer => axial_tilt_rad,
    Season::Winter => -axial_tilt_rad,
    Season::Spring | Season::Autumn => 0.0,
} * if is_northern { 1.0 } else { -1.0 };
let cos_omega = (-lat_rad.tan() * decl.tan()).clamp(-1.0, 1.0);
let day_length = cos_omega.acos() / std::f32::consts::PI; // 0.0 = polar night, 0.5 = 12h, 1.0 = midnight sun

```


2. **Maritime Thermal Lag:** Add a phase lag $\Delta\theta \in [0, \pi/6]$ to `get_temperature` for `ClimateSeasonType::Oceanic` and coastal tiles (since ocean heat capacity delays peak summer/winter temperatures by $\sim 1$ month compared to continental interiors).





---

## Summary Priority Checklist for Implementation

1. **Bug Fixes (Immediate):**
* Fix the inverted sign of `curvature` in `Tilemap::analyze_directional_context_weighted` (`center_value - neighbor_avg`).


* Fix the 1-ring neighbor lookup in `biome_feathering::compute_blend_weights` by propagating `nearest_foreign_biome` during the distance BFS.


* Fix the polar Y-wrap (`rem_euclid(height)`) and uphill lava climbing (`+ 50.0`) in `heightmap::generate_lava_map`.


* Recompute `flow_accumulation` at the `final` stage of `terrain::generate_terrain` after `fill_pits`.




2. **Geometric & Physical Upgrades (High Impact):**
* Reorder `terrain::generate_terrain` so regional noise stacks and volcanoes are placed *before* `landscape::evolve` / erosion.


* Replace `cylindrical_coords` with true rotated `spherical_coords` and add $1/\cos\phi$ latitude metric scaling to `tilemap.rs` gradients and distance fields.


* Upgrade `detect_aquifers` in `underground_water.rs` to use the **Topographic Wetness Index ($\text{TWI}$)** and synclinal basin piezometric head instead of `elevation * 0.05`.


* Trace `apply_fjord_incisions` along high-latitude coastal `flow_accumulation` valleys with parabolic U-troughs and terminal moraine sills.
