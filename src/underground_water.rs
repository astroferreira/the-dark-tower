//! Underground water features: aquifers, springs, and waterfalls
//!
//! This module detects and classifies underground water features:
//! - **Aquifers**: Underground water reservoirs based on geology and precipitation
//! - **Springs**: Where underground water emerges at the surface
//! - **Waterfalls**: Where rivers drop significantly in elevation

use crate::tilemap::Tilemap;
use crate::erosion::rivers::{compute_flow_direction, compute_flow_accumulation, DX, DY, NO_FLOW};

/// Type of aquifer present at a tile
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AquiferType {
    #[default]
    None,
    /// Unconfined aquifer - water table near surface, recharged by rainfall
    Unconfined,
    /// Confined aquifer - deep, under pressure from impermeable layers
    Confined,
    /// Perched aquifer - sits above impermeable layer, above main water table
    Perched,
}

impl AquiferType {
    pub fn display_name(&self) -> &'static str {
        match self {
            AquiferType::None => "None",
            AquiferType::Unconfined => "Unconfined",
            AquiferType::Confined => "Confined",
            AquiferType::Perched => "Perched",
        }
    }

    pub fn is_present(&self) -> bool {
        *self != AquiferType::None
    }
}

/// Type of spring at a tile
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SpringType {
    #[default]
    None,
    /// Seepage spring - water slowly seeps out due to gravity
    Seepage,
    /// Artesian spring - pressurized water from confined aquifer
    Artesian,
    /// Thermal spring - heated by geothermal activity
    Thermal,
    /// Karst spring - emerges from limestone/cave systems
    Karst,
}

impl SpringType {
    pub fn display_name(&self) -> &'static str {
        match self {
            SpringType::None => "None",
            SpringType::Seepage => "Seepage",
            SpringType::Artesian => "Artesian",
            SpringType::Thermal => "Thermal",
            SpringType::Karst => "Karst",
        }
    }

    pub fn is_present(&self) -> bool {
        *self != SpringType::None
    }
}

/// Information about aquifer at a tile
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AquiferInfo {
    /// Type of aquifer
    pub aquifer_type: AquiferType,
    /// Depth to water table in meters (0 = at surface)
    pub depth: f32,
    /// Yield potential (0.0-1.0) - how much water can be extracted
    pub yield_potential: f32,
    /// Recharge rate (0.0-1.0) - how fast it refills from precipitation
    pub recharge_rate: f32,
}

impl AquiferInfo {
    pub fn new(aquifer_type: AquiferType, depth: f32, yield_potential: f32, recharge_rate: f32) -> Self {
        Self {
            aquifer_type,
            depth,
            yield_potential,
            recharge_rate,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_present(&self) -> bool {
        self.aquifer_type.is_present()
    }
}

/// Information about a spring at a tile
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SpringInfo {
    /// Type of spring
    pub spring_type: SpringType,
    /// Flow rate (0.0-1.0) - how much water emerges
    pub flow_rate: f32,
    /// Temperature modifier (0.0 = ambient, positive = heated)
    pub temperature_mod: f32,
}

impl SpringInfo {
    pub fn new(spring_type: SpringType, flow_rate: f32, temperature_mod: f32) -> Self {
        Self {
            spring_type,
            flow_rate,
            temperature_mod,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_present(&self) -> bool {
        self.spring_type.is_present()
    }
}

/// Information about a waterfall at a tile
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WaterfallInfo {
    /// Whether a waterfall is present
    pub is_present: bool,
    /// Height of the drop in meters
    pub drop_height: f32,
    /// Width based on river flow (0.0-1.0 normalized)
    pub width: f32,
    /// Direction of flow (0-7, same as D8)
    pub direction: u8,
}

impl WaterfallInfo {
    pub fn new(drop_height: f32, width: f32, direction: u8) -> Self {
        Self {
            is_present: true,
            drop_height,
            width,
            direction,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }
}

/// Parameters for underground water detection
#[derive(Clone, Debug)]
pub struct UndergroundWaterParams {
    /// Minimum porosity for aquifer formation (0.0-1.0)
    pub min_porosity: f32,
    /// Minimum moisture for aquifer recharge
    pub min_moisture_for_recharge: f32,
    /// Depth scaling factor (elevation to depth conversion)
    pub depth_scale: f32,
    /// Minimum elevation gradient for spring formation
    pub spring_gradient_threshold: f32,
    /// Minimum flow accumulation for spring from aquifer
    pub spring_flow_threshold: f32,
    /// Minimum elevation drop for waterfall (meters)
    pub waterfall_min_drop: f32,
    /// Minimum flow accumulation for waterfall detection
    pub waterfall_min_flow: f32,
    /// High stress threshold for thermal springs
    pub thermal_stress_threshold: f32,
}

impl Default for UndergroundWaterParams {
    fn default() -> Self {
        Self {
            min_porosity: 0.3,
            min_moisture_for_recharge: 0.2,
            depth_scale: 0.5,
            spring_gradient_threshold: 40.0, // meters drop per tile
            spring_flow_threshold: 20.0,
            waterfall_min_drop: 20.0,
            waterfall_min_flow: 50.0,
            thermal_stress_threshold: 0.75,
        }
    }
}

/// Detect aquifers across the map
///
/// Aquifers form based on:
/// - Rock porosity (inverse of hardness) - porous rocks hold water
/// - Moisture/precipitation - water to fill the aquifer
/// - Elevation - affects depth and pressure
pub fn detect_aquifers(
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    hardness_map: Option<&Tilemap<f32>>,
    stress_map: &Tilemap<f32>,
    params: &UndergroundWaterParams,
) -> Tilemap<AquiferInfo> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut aquifers = Tilemap::new_with(width, height, AquiferInfo::none());

    // Default hardness if not provided
    let default_hardness = 0.5f32;

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);

            // Skip underwater tiles (ocean floor doesn't have aquifers in this model)
            if elev < 0.0 {
                continue;
            }

            let moist = *moisture.get(x, y);
            let hard = hardness_map.map(|h| *h.get(x, y)).unwrap_or(default_hardness);
            let stress = stress_map.get(x, y).abs();

            // Porosity is inverse of hardness (soft rocks like sandstone are porous)
            let porosity = 1.0 - hard;

            // Skip if rock is too hard (no aquifer possible)
            if porosity < params.min_porosity {
                continue;
            }

            // Calculate aquifer properties
            let depth = calculate_aquifer_depth(elev, porosity, stress, params);
            let yield_potential = calculate_yield_potential(porosity, moist, depth);
            let recharge_rate = calculate_recharge_rate(porosity, moist, params);

            // Determine aquifer type based on depth and geology
            let aquifer_type = determine_aquifer_type(depth, elev, stress, porosity);

            if aquifer_type != AquiferType::None {
                aquifers.set(x, y, AquiferInfo::new(
                    aquifer_type,
                    depth,
                    yield_potential,
                    recharge_rate,
                ));
            }
        }
    }

    aquifers
}

/// Deepest water table (m) under a dry, steep, tight ridge, and how fast wetness, porosity and
/// convergence bring it up (the c1 of d = d_max exp(-c1 moisture porosity TWI)).
const WATER_TABLE_MAX_M: f32 = 120.0;
const WATER_TABLE_C1: f32 = 0.6;
/// The regional uplands that recharge a confined aquifer: their radius (tiles) and how much
/// higher than the basin they stand on average (m).
const RECHARGE_RADIUS: usize = 8;
const RECHARGE_RISE_M: f32 = 150.0;
/// Share of the uplands' head lost on the way down the aquifer to the basin.
const HEAD_LOSS: f32 = 0.3;

/// The Beven-Kirkby topographic wetness index ln(a / tan b) of every land cell, less its
/// smallest value on land (so 0 on the driest ground): a the specific catchment area
/// (upstream cells x cell width), tan b the local slope (at least 0.001).
pub fn wetness_index(heightmap: &Tilemap<f32>, flow_acc: &Tilemap<f32>) -> Tilemap<f32> {
    let (w, h) = (heightmap.width, heightmap.height);
    let cell_m = crate::erosion::landscape::tile_km(w) * 1000.0;
    let mut twi = Tilemap::new_with(w, h, 0.0f32);
    let mut lo = f32::MAX;
    for y in 0..h {
        for x in 0..w {
            if *heightmap.get(x, y) < 0.0 { continue; }
            let slope = heightmap.analyze_directional_context(x, y).gradient_magnitude / cell_m;
            let a = flow_acc.get(x, y).max(1.0) * cell_m;
            let v = (a / slope.max(0.001)).ln();
            twi.set(x, y, v);
            lo = lo.min(v);
        }
    }
    if lo == f32::MAX { return twi; }
    for (x, y, v) in twi.iter_mut() { if *heightmap.get(x, y) >= 0.0 { *v -= lo; } }
    twi
}

/// The mean height of the land within `RECHARGE_RADIUS` tiles of each cell (the regional
/// uplands that feed its aquifers; sea counts as nothing).
fn regional_land_mean(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let (w, h) = (heightmap.width, heightmap.height);
    let r = RECHARGE_RADIUS as i32;
    let land = |x: usize, y: usize| { let e = *heightmap.get(x, y); if e >= 0.0 { (e, 1.0f32) } else { (0.0, 0.0) } };
    // Box sums, separable: rows (wrapping), then columns (clamped).
    let mut row = vec![(0.0f32, 0.0f32); w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = (0.0, 0.0);
            for dx in -r..=r {
                let (e, c) = land((x as i32 + dx).rem_euclid(w as i32) as usize, y);
                acc.0 += e; acc.1 += c;
            }
            row[y * w + x] = acc;
        }
    }
    let mut out = Tilemap::new_with(w, h, 0.0f32);
    for y in 0..h {
        for x in 0..w {
            let mut acc = (0.0, 0.0);
            for dy in -r..=r {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 { continue; }
                let v = row[ny as usize * w + x];
                acc.0 += v.0; acc.1 += v.1;
            }
            out.set(x, y, if acc.1 > 0.0 { acc.0 / acc.1 } else { 0.0 });
        }
    }
    out
}

/// Aquifers from the wetness of the ground (`audit.md` 4.1): the water table lies at
/// d = d_max exp(-c1 moisture porosity TWI), at the surface in wet convergent valley floors and
/// deep under steep dry ridges (revision 1 put it at a fixed share of the elevation, so every
/// mountain range was a confined aquifer and no basin could be). Confined aquifers lie in the
/// concave basins (curvature below -5 m) whose recharge uplands stand 150 m above them; the
/// rest are unconfined, or perched on hills where the table is middling and the rock porous.
pub fn detect_aquifers_twi(
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    hardness_map: Option<&Tilemap<f32>>,
    flow_acc: &Tilemap<f32>,
    params: &UndergroundWaterParams,
) -> Tilemap<AquiferInfo> {
    let (width, height) = (heightmap.width, heightmap.height);
    let mut aquifers = Tilemap::new_with(width, height, AquiferInfo::none());
    let twi = wetness_index(heightmap, flow_acc);
    let uplands = regional_land_mean(heightmap);
    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            if elev < 0.0 { continue; }
            let moist = *moisture.get(x, y);
            let porosity = 1.0 - hardness_map.map(|h| *h.get(x, y)).unwrap_or(0.5);
            if porosity < params.min_porosity { continue; }
            let depth = WATER_TABLE_MAX_M * (-WATER_TABLE_C1 * moist * porosity * *twi.get(x, y)).exp();
            let curvature = heightmap.analyze_directional_context(x, y).curvature;
            let confined = curvature < -5.0 && *uplands.get(x, y) > elev + RECHARGE_RISE_M;
            let kind = if confined {
                AquiferType::Confined
            } else if depth > 5.0 && depth < 30.0 && elev > 100.0 && porosity > 0.5 {
                AquiferType::Perched
            } else {
                AquiferType::Unconfined
            };
            aquifers.set(x, y, AquiferInfo::new(kind, depth, calculate_yield_potential(porosity, moist, depth), calculate_recharge_rate(porosity, moist, params)));
        }
    }
    aquifers
}

/// The confined aquifer's piezometric head at a cell: the recharge uplands' mean height less
/// what friction takes on the way.
pub fn piezometric_head(regional_mean: f32, elev: f32) -> f32 {
    regional_mean - HEAD_LOSS * (regional_mean - elev).max(0.0)
}

/// Calculate depth to water table
fn calculate_aquifer_depth(elevation: f32, porosity: f32, stress: f32, params: &UndergroundWaterParams) -> f32 {
    // Base depth scales with elevation (higher = deeper water table)
    let base_depth = elevation * params.depth_scale * 0.1;

    // High porosity means shallower water table
    let porosity_factor = 1.0 - porosity * 0.5;

    // Tectonic stress can fracture rock, creating paths for water
    let stress_factor = 1.0 - stress * 0.3;

    (base_depth * porosity_factor * stress_factor).max(0.0)
}

/// Calculate yield potential of aquifer
fn calculate_yield_potential(porosity: f32, moisture: f32, depth: f32) -> f32 {
    // High porosity and moisture = better yield
    let base_yield = porosity * moisture;

    // Deep aquifers have lower yield (harder to extract)
    let depth_penalty = 1.0 / (1.0 + depth * 0.01);

    (base_yield * depth_penalty).clamp(0.0, 1.0)
}

/// Calculate recharge rate from precipitation
fn calculate_recharge_rate(porosity: f32, moisture: f32, params: &UndergroundWaterParams) -> f32 {
    if moisture < params.min_moisture_for_recharge {
        return 0.0;
    }

    // Recharge depends on precipitation and rock permeability
    (porosity * moisture * 0.5).clamp(0.0, 1.0)
}

/// Determine aquifer type based on conditions
fn determine_aquifer_type(depth: f32, elevation: f32, stress: f32, porosity: f32) -> AquiferType {
    // Very shallow water table = unconfined
    if depth < 10.0 && porosity > 0.4 {
        return AquiferType::Unconfined;
    }

    // Deep aquifers under high stress (compressed layers) = confined
    if depth > 30.0 && stress > 0.2 {
        return AquiferType::Confined;
    }

    // Moderate depth in hilly terrain = perched
    if depth > 5.0 && depth < 30.0 && elevation > 100.0 && porosity > 0.5 {
        return AquiferType::Perched;
    }

    // Default: unconfined if there's any aquifer potential
    if porosity > 0.3 {
        return AquiferType::Unconfined;
    }

    AquiferType::None
}

/// Detect springs across the map
///
/// Springs form where:
/// - Aquifer meets surface (elevation gradient)
/// - Base of hills/mountains
/// - Near fault lines (stress)
/// - At karst/limestone features
pub fn detect_springs(
    heightmap: &Tilemap<f32>,
    aquifers: &Tilemap<AquiferInfo>,
    stress_map: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    params: &UndergroundWaterParams,
) -> Tilemap<SpringInfo> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut springs = Tilemap::new_with(width, height, SpringInfo::none());
    // Revision 2: artesian water rises where a confined aquifer's head stands above the ground
    // and a fault opens the way (`audit.md` 4.1).
    let uplands = if crate::terrain_rev::v1() { None } else { Some(regional_land_mean(heightmap)) };

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);

            // Skip underwater tiles
            if elev < 0.0 {
                continue;
            }

            let aquifer = aquifers.get(x, y);
            let stress = stress_map.get(x, y).abs();
            let flow = *flow_acc.get(x, y);

            if let Some(up) = &uplands {
                let head = piezometric_head(*up.get(x, y), elev);
                if aquifer.aquifer_type == AquiferType::Confined && head > elev && stress > 0.15 && aquifer.yield_potential > 0.2 {
                    let flow_rate = (aquifer.yield_potential * (0.5 + (head - elev) / 500.0)).min(1.0);
                    springs.set(x, y, SpringInfo::new(SpringType::Artesian, flow_rate, 0.0));
                    continue;
                }
            }

            // Check if this could be a spring location
            if let Some(spring_info) = check_spring_conditions(
                x, y, elev, aquifer, stress, flow, heightmap, aquifers, params
            ) {
                springs.set(x, y, spring_info);
            }
        }
    }

    springs
}

/// Check if a tile meets conditions for a spring
fn check_spring_conditions(
    x: usize,
    y: usize,
    elev: f32,
    aquifer: &AquiferInfo,
    stress: f32,
    flow: f32,
    heightmap: &Tilemap<f32>,
    aquifers: &Tilemap<AquiferInfo>,
    params: &UndergroundWaterParams,
) -> Option<SpringInfo> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Calculate local gradient (how much higher are neighbors?)
    let mut max_uphill = 0.0f32;
    let mut uphill_aquifer_yield = 0.0f32;

    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let neighbor_elev = *heightmap.get(nx, ny);
            let elev_diff = neighbor_elev - elev;

            if elev_diff > max_uphill {
                max_uphill = elev_diff;
                uphill_aquifer_yield = aquifers.get(nx, ny).yield_potential;
            }
        }
    }

    // Condition 1: Thermal spring - high tectonic stress hotspot
    if stress > params.thermal_stress_threshold && aquifer.is_present() && aquifer.yield_potential > 0.3 {
        let flow_rate = aquifer.yield_potential * (0.5 + stress);
        let temp_mod = stress * 30.0; // Hotter with more stress
        return Some(SpringInfo::new(SpringType::Thermal, flow_rate.min(1.0), temp_mod));
    }

    // Condition 2: Artesian spring - confined aquifer meets surface at low point
    if aquifer.aquifer_type == AquiferType::Confined && aquifer.depth < 2.0 && aquifer.yield_potential > 0.4 {
        let flow_rate = aquifer.yield_potential * 0.8;
        return Some(SpringInfo::new(SpringType::Artesian, flow_rate, 0.0));
    }

    // Condition 3: Seepage spring - steep elevation break with uphill aquifer
    if max_uphill > params.spring_gradient_threshold && uphill_aquifer_yield > 0.4 {
        let flow_rate = uphill_aquifer_yield * 0.5;
        return Some(SpringInfo::new(SpringType::Seepage, flow_rate, 0.0));
    }

    // Condition 4: Headwater source spring - emerging stream source from shallow aquifer
    if flow >= 5.0 && flow <= params.spring_flow_threshold && aquifer.is_present() && aquifer.depth < 5.0 && aquifer.yield_potential > 0.4 {
        let flow_rate = aquifer.yield_potential * 0.6;
        return Some(SpringInfo::new(SpringType::Seepage, flow_rate, 0.0));
    }

    // Condition 5: Karst spring - perched aquifer with high recharge
    if aquifer.aquifer_type == AquiferType::Perched && aquifer.recharge_rate > 0.5 && aquifer.yield_potential > 0.4 {
        let flow_rate = aquifer.yield_potential * 0.6;
        return Some(SpringInfo::new(SpringType::Karst, flow_rate, 0.0));
    }

    None
}

/// Detect waterfalls along river paths
///
/// Waterfalls occur where:
/// - Rivers flow over steep elevation drops
/// - Cliff edges along river courses
/// - Resistant rock layers create steps
pub fn detect_waterfalls(
    heightmap: &Tilemap<f32>,
    flow_acc: &Tilemap<f32>,
    flow_dir: &Tilemap<u8>,
    hardness_map: Option<&Tilemap<f32>>,
    params: &UndergroundWaterParams,
) -> Tilemap<WaterfallInfo> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut waterfalls = Tilemap::new_with(width, height, WaterfallInfo::none());

    for y in 0..height {
        for x in 0..width {
            let flow = *flow_acc.get(x, y);
            let dir = *flow_dir.get(x, y);

            // Skip if not enough flow to be a river
            if flow < params.waterfall_min_flow || dir == NO_FLOW {
                continue;
            }

            let elev = *heightmap.get(x, y);

            // Skip underwater
            if elev < 0.0 {
                continue;
            }

            // Get downstream cell
            let nx = (x as i32 + DX[dir as usize]).rem_euclid(width as i32) as usize;
            let ny = y as i32 + DY[dir as usize];

            if ny < 0 || ny >= height as i32 {
                continue;
            }
            let ny = ny as usize;

            let next_elev = *heightmap.get(nx, ny);
            let drop = elev - next_elev;

            if !crate::terrain_rev::v1() {
                // Revision 2 (`audit.md` 4.2): the fall is exactly the drop, and a caprock ledge
                // (hard rock over soft downstream, a contrast above 0.15: Niagara's dolomite
                // over shale) makes one where a smaller drop would not.
                let hard = |x: usize, y: usize| hardness_map.map(|h| *h.get(x, y)).unwrap_or(0.5);
                let contrast = hard(x, y) - hard(nx, ny);
                let min_drop = if contrast > 0.15 { params.waterfall_min_drop * 0.5 } else { params.waterfall_min_drop };
                if drop >= min_drop {
                    let width_factor = (flow.ln() / 10.0).clamp(0.1, 1.0);
                    waterfalls.set(x, y, WaterfallInfo::new(drop, width_factor, dir));
                }
                continue;
            }

            // Check if drop is significant enough for a waterfall
            if drop >= params.waterfall_min_drop {
                // Width scales with flow (log scale)
                let width_factor = (flow.ln() / 10.0).clamp(0.1, 1.0);

                // Hard rock can create taller, narrower falls
                let hardness = hardness_map.map(|h| *h.get(x, y)).unwrap_or(0.5);
                let adjusted_drop = drop * (1.0 + hardness * 0.5);

                waterfalls.set(x, y, WaterfallInfo::new(adjusted_drop, width_factor, dir));
            }
        }
    }

    waterfalls
}

/// All underground water features for a world
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UndergroundWater {
    pub aquifers: Tilemap<AquiferInfo>,
    pub springs: Tilemap<SpringInfo>,
    pub waterfalls: Tilemap<WaterfallInfo>,
}

impl UndergroundWater {
    /// Generate all underground water features
    pub fn generate(
        heightmap: &Tilemap<f32>,
        moisture: &Tilemap<f32>,
        stress_map: &Tilemap<f32>,
        hardness_map: Option<&Tilemap<f32>>,
        params: &UndergroundWaterParams,
    ) -> Self {
        // Compute flow data
        let flow_dir = compute_flow_direction(heightmap);
        let flow_acc = compute_flow_accumulation(heightmap, &flow_dir);

        // Detect features (revision 2: the water table from the wetness index, `audit.md` 4.1)
        let aquifers = if crate::terrain_rev::v1() {
            detect_aquifers(heightmap, moisture, hardness_map, stress_map, params)
        } else {
            detect_aquifers_twi(heightmap, moisture, hardness_map, &flow_acc, params)
        };
        let springs = detect_springs(heightmap, &aquifers, stress_map, &flow_acc, params);
        let waterfalls = detect_waterfalls(heightmap, &flow_acc, &flow_dir, hardness_map, params);

        Self {
            aquifers,
            springs,
            waterfalls,
        }
    }

    /// Get feature info at a specific tile
    pub fn get_tile_features(&self, x: usize, y: usize) -> TileWaterFeatures {
        TileWaterFeatures {
            aquifer: *self.aquifers.get(x, y),
            spring: *self.springs.get(x, y),
            waterfall: *self.waterfalls.get(x, y),
        }
    }
}

/// Water features for a single tile
#[derive(Clone, Copy, Debug, Default)]
pub struct TileWaterFeatures {
    pub aquifer: AquiferInfo,
    pub spring: SpringInfo,
    pub waterfall: WaterfallInfo,
}

impl TileWaterFeatures {
    /// Check if tile has any water features
    pub fn has_any(&self) -> bool {
        self.aquifer.is_present() || self.spring.is_present() || self.waterfall.is_present
    }

    /// Format as display string
    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();

        if self.aquifer.is_present() {
            parts.push(format!("Aquifer: {}", self.aquifer.aquifer_type.display_name()));
        }
        if self.spring.is_present() {
            parts.push(format!("Spring: {}", self.spring.spring_type.display_name()));
        }
        if self.waterfall.is_present {
            parts.push(format!("Waterfall: {:.0}m", self.waterfall.drop_height));
        }

        if parts.is_empty() {
            "None".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Statistics about underground water features
#[derive(Clone, Debug, Default)]
pub struct UndergroundWaterStats {
    pub aquifer_tiles: usize,
    pub unconfined_aquifers: usize,
    pub confined_aquifers: usize,
    pub perched_aquifers: usize,
    pub spring_count: usize,
    pub seepage_springs: usize,
    pub artesian_springs: usize,
    pub thermal_springs: usize,
    pub karst_springs: usize,
    pub waterfall_count: usize,
    pub max_waterfall_height: f32,
}

impl UndergroundWater {
    /// Calculate statistics about underground water features
    pub fn stats(&self) -> UndergroundWaterStats {
        let mut stats = UndergroundWaterStats::default();

        for y in 0..self.aquifers.height {
            for x in 0..self.aquifers.width {
                let aq = self.aquifers.get(x, y);
                match aq.aquifer_type {
                    AquiferType::None => {}
                    AquiferType::Unconfined => {
                        stats.aquifer_tiles += 1;
                        stats.unconfined_aquifers += 1;
                    }
                    AquiferType::Confined => {
                        stats.aquifer_tiles += 1;
                        stats.confined_aquifers += 1;
                    }
                    AquiferType::Perched => {
                        stats.aquifer_tiles += 1;
                        stats.perched_aquifers += 1;
                    }
                }

                let sp = self.springs.get(x, y);
                match sp.spring_type {
                    SpringType::None => {}
                    SpringType::Seepage => {
                        stats.spring_count += 1;
                        stats.seepage_springs += 1;
                    }
                    SpringType::Artesian => {
                        stats.spring_count += 1;
                        stats.artesian_springs += 1;
                    }
                    SpringType::Thermal => {
                        stats.spring_count += 1;
                        stats.thermal_springs += 1;
                    }
                    SpringType::Karst => {
                        stats.spring_count += 1;
                        stats.karst_springs += 1;
                    }
                }

                let wf = self.waterfalls.get(x, y);
                if wf.is_present {
                    stats.waterfall_count += 1;
                    stats.max_waterfall_height = stats.max_waterfall_height.max(wf.drop_height);
                }
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aquifer_detection() {
        // Create a simple heightmap
        let mut heightmap = Tilemap::new_with(10, 10, 50.0);
        let moisture = Tilemap::new_with(10, 10, 0.5);
        let stress = Tilemap::new_with(10, 10, 0.1);

        // Make some tiles underwater (no aquifer expected)
        heightmap.set(0, 0, -10.0);

        let params = UndergroundWaterParams::default();
        let aquifers = detect_aquifers(&heightmap, &moisture, None, &stress, &params);

        // Underwater tile should have no aquifer
        assert!(!aquifers.get(0, 0).is_present());

        // Land tiles with default moisture/porosity should have aquifers
        assert!(aquifers.get(5, 5).is_present());
    }

    /// The water table is near the surface in a wet valley floor and deep under a dry ridge;
    /// a basin below its uplands holds a confined aquifer, a ridge never does.
    #[test]
    fn water_table_follows_wetness() {
        let (w, h) = (48, 24);
        let mut hm = Tilemap::new_with(w, h, 0.0f32);
        // A valley along x = 24 between ridges; uplands 600 m, floor 50 m.
        for y in 0..h { for x in 0..w {
            let d = (x as f32 - 24.0).abs();
            hm.set(x, y, 50.0 + (d * 60.0).min(600.0) + y as f32 * 2.0);
        } }
        let moisture = Tilemap::new_with(w, h, 0.7f32);
        let flow_dir = compute_flow_direction(&hm);
        let flow_acc = compute_flow_accumulation(&hm, &flow_dir);
        let params = UndergroundWaterParams::default();
        let a = detect_aquifers_twi(&hm, &moisture, None, &flow_acc, &params);
        let floor = a.get(24, 4).depth;
        let ridge = a.get(4, 12).depth;
        assert!(floor < ridge, "valley floor {floor} m vs ridge {ridge} m");
        assert_eq!(a.get(24, 12).aquifer_type, AquiferType::Confined, "the basin below its uplands");
        assert_ne!(a.get(4, 12).aquifer_type, AquiferType::Confined, "a high flank is not confined");
    }

    /// A waterfall is as tall as the drop it falls over (`audit.md` 4.2).
    #[test]
    fn waterfall_height_is_the_drop() {
        let mut heightmap = Tilemap::new_with(10, 10, 100.0);
        for x in 0..10 { heightmap.set(x, 5, 50.0); heightmap.set(x, 6, 45.0); }
        let flow_dir = compute_flow_direction(&heightmap);
        let flow_acc = compute_flow_accumulation(&heightmap, &flow_dir);
        let params = UndergroundWaterParams { waterfall_min_flow: 1.0, waterfall_min_drop: 30.0, ..Default::default() };
        let hard = Tilemap::new_with(10, 10, 0.9f32);
        let wf = detect_waterfalls(&heightmap, &flow_acc, &flow_dir, Some(&hard), &params);
        assert!(wf.get(5, 4).is_present && (wf.get(5, 4).drop_height - 50.0).abs() < 1e-3, "drop {}", wf.get(5, 4).drop_height);
        // A 20 m step over a caprock (hard over soft) falls; on uniform rock it does not.
        let mut step = Tilemap::new_with(10, 10, 100.0);
        for x in 0..10 { for y in 5..10 { step.set(x, y, 80.0 - y as f32 * 0.1); } }
        let fd = compute_flow_direction(&step);
        let fa = compute_flow_accumulation(&step, &fd);
        let mut cap = Tilemap::new_with(10, 10, 0.9f32);
        for x in 0..10 { for y in 5..10 { cap.set(x, y, 0.3); } }
        assert!(detect_waterfalls(&step, &fa, &fd, Some(&cap), &params).get(5, 4).is_present, "caprock ledge");
        assert!(!detect_waterfalls(&step, &fa, &fd, Some(&hard), &params).get(5, 4).is_present, "uniform rock");
    }

    #[test]
    fn test_waterfall_detection() {
        // Create terrain with a steep drop
        let mut heightmap = Tilemap::new_with(10, 10, 100.0);

        // Create a cliff: row 5 drops suddenly
        for x in 0..10 {
            heightmap.set(x, 4, 100.0);
            heightmap.set(x, 5, 50.0); // 50m drop
            heightmap.set(x, 6, 45.0);
        }

        let flow_dir = compute_flow_direction(&heightmap);
        let flow_acc = compute_flow_accumulation(&heightmap, &flow_dir);

        let params = UndergroundWaterParams {
            waterfall_min_flow: 1.0, // Low threshold for test
            waterfall_min_drop: 30.0,
            ..Default::default()
        };

        let waterfalls = detect_waterfalls(&heightmap, &flow_acc, &flow_dir, None, &params);

        // Should detect waterfall at the cliff edge
        let wf = waterfalls.get(5, 4);
        assert!(wf.is_present);
        assert!(wf.drop_height >= 30.0);
    }
}
