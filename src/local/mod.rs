//! Playable areas: Dwarf Fortress style 3D local maps generated from a zoomed region.
//!
//! An embark is `LOCAL_SIZE` x `LOCAL_SIZE` tiles of `TILE_M` metres (person-sized) with
//! z-levels of `Z_STEP_M` metres. Everything is derived from the region it sits in:
//!
//! * **Surface**: bicubic elevation from the zoomed region plus metre-scale relief whose
//!   amplitude follows the local slope (rough mountainsides, gentle plains).
//! * **Water**: river channels follow the region's river cells (meandering centrelines, width
//!   from the region's hydrology, carved bed, water filled to level); lakes keep their water
//!   level; the sea fills everything below 0 m.
//! * **Underground**: soil (sand / clay / gravel / loam by setting, thinner on slopes) over the
//!   world tile's rock stack from the region handshake, with gently undulating strata.
//! * **Life**: individual trees, shrubs, grass and boulders; species and density come from the
//!   local climate through the world's biome classifier.
//!
//! All noise and object placement use absolute world positions, so the same spot always
//! generates the same way (neighbouring embarks would match).

pub mod structures;
pub mod wildlife;

use noise::{NoiseFn, Perlin};

use crate::climate::biomes::Biome;
use crate::erosion::materials::RockType;
use crate::region::zoom::ZoomRegion;
use crate::world::WorldData;

/// Tiles per side of a playable area (DF's 4x4 embark of 48x48).
pub const LOCAL_SIZE: usize = 192;
/// Horizontal size of a tile (m).
pub const TILE_M: f32 = 2.0;
/// Vertical size of a z-level (m).
pub const Z_STEP_M: f32 = 2.0;
/// Solid z-levels kept below the lowest surface point.
const DEPTH_BELOW: i32 = 30;
/// Empty z-levels kept above the highest surface point.
const AIR_ABOVE: i32 = 8;
/// Share of a world tile's area a catchment needs to carry a creek at person scale (a quarter
/// of the region's river threshold).
const CREEK_AREA_FRACTION: f32 = 0.005;
/// Mean annual temperature (C) below which lakes and rivers freeze over.
const FREEZE_TEMP_C: f32 = -3.0;
/// Water depth units per full z-level (DF uses 1-7).
pub const WATER_FULL: u8 = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Material {
    Air,
    Soil,
    Sand,
    Clay,
    Gravel,
    Snow,
    Ice,
    Rock(RockType),
    /// Constructed: timber.
    Wood,
    /// Constructed: dressed stone blocks.
    Block(RockType),
    /// Ore vein in the rock.
    Ore(crate::history::civilizations::economy::ResourceType),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Open space (air or water).
    Empty,
    /// Walkable top of the ground.
    Floor,
    /// A floor next to ground one level higher: leads up.
    Ramp,
    /// Solid ground.
    Wall,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeKind {
    Broadleaf,
    Conifer,
    Jungle,
    Palm,
    Acacia,
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plant {
    None,
    Grass,
    Shrub,
    Tree(TreeKind),
    /// Cultivated crop row (variety 0-2).
    Crop(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub shape: Shape,
    pub material: Material,
    /// Water depth in the cell, 0..=WATER_FULL.
    pub water: u8,
    pub plant: Plant,
    pub boulder: bool,
}

impl Cell {
    const AIR: Cell = Cell { shape: Shape::Empty, material: Material::Air, water: 0, plant: Plant::None, boulder: false };
}

pub struct LocalMap {
    pub width: usize,
    pub height: usize,
    /// Number of z-levels; z = 0 is the deepest.
    pub depth: usize,
    /// Elevation (m) of the bottom of z-level 0.
    pub z_min_m: f32,
    /// [z][y][x]
    pub cells: Vec<Cell>,
    /// Floor z-level of every column.
    pub surface_z: Vec<i32>,
    /// Surface elevation (m) of every column, for shading.
    pub surface_m: Vec<f32>,
    /// World tile this area lies in.
    pub world_tile: (usize, usize),
    /// Biome at the centre (for display).
    pub biome: Biome,
    /// Signs of animal life on each column's surface (trails, burrows, nests, dens, bones).
    pub features: Vec<wildlife::Feature>,
}

impl LocalMap {
    #[inline]
    pub fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.height + y) * self.width + x
    }
    pub fn cell(&self, x: usize, y: usize, z: usize) -> &Cell {
        &self.cells[self.idx(x, y, z)]
    }
    /// Elevation (m) of the bottom of z-level `z`.
    pub fn z_elevation(&self, z: i32) -> f32 {
        self.z_min_m + z as f32 * Z_STEP_M
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm(noise: &Perlin, x: f64, y: f64, octaves: u32) -> f32 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([x * f + 0.31, y * f + 0.77]);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    (sum / norm) as f32
}

fn hash(x: i64, y: i64, salt: u64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

/// Bicubic sample of a region field at fractional cell coordinates (cell centres at x + 0.5).
fn bicubic(f: &[f32], w: usize, h: usize, x: f64, y: f64) -> f32 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
    let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
    let at = |dx: i64, dy: i64| {
        let xx = (x0 + dx).clamp(0, w as i64 - 1) as usize;
        let yy = (y0 + dy).clamp(0, h as i64 - 1) as usize;
        f[yy * w + xx]
    };
    let row = |dy: i64| catmull(at(-1, dy), at(0, dy), at(1, dy), at(2, dy), tx);
    catmull(row(-1), row(0), row(1), row(2), ty)
}

fn bilinear(f: &[f32], w: usize, h: usize, x: f64, y: f64) -> f32 {
    let (fx, fy) = ((x - 0.5).clamp(0.0, w as f64 - 1.0), (y - 0.5).clamp(0.0, h as f64 - 1.0));
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
    let top = f[y0 * w + x0] * (1.0 - tx) + f[y0 * w + x1] * tx;
    let bot = f[y1 * w + x0] * (1.0 - tx) + f[y1 * w + x1] * tx;
    top * (1.0 - ty) + bot * ty
}

/// Distance from a point to a segment, and the position along it (0 at `a`, 1 at `b`).
fn seg_dist(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a.0 + dx * t - px, a.1 + dy * t - py);
    ((qx * qx + qy * qy).sqrt(), t)
}

/// Trees per floor tile and the species mix for a biome.
fn vegetation(biome: Biome) -> (f32, f32, &'static [TreeKind]) {
    use Biome::*;
    use TreeKind::*;
    // (tree density, shrub density, species)
    match biome {
        TropicalRainforest => (0.50, 0.10, &[Jungle, Jungle, Broadleaf]),
        TropicalForest => (0.35, 0.10, &[Broadleaf, Jungle]),
        Savanna => (0.03, 0.06, &[Acacia]),
        TemperateRainforest => (0.45, 0.08, &[Conifer, Conifer, Broadleaf]),
        TemperateForest => (0.35, 0.07, &[Broadleaf, Broadleaf, Broadleaf, Conifer]),
        BorealForest | SubalpineForest => (0.35, 0.05, &[Conifer]),
        MontaneForest | CloudForest => (0.38, 0.08, &[Broadleaf, Conifer]),
        TemperateGrassland => (0.008, 0.03, &[Broadleaf]),
        AlpineMeadow | Paramo => (0.003, 0.06, &[Conifer]),
        Tundra | AlpineTundra => (0.0, 0.05, &[Conifer]),
        Desert => (0.002, 0.01, &[Dead]),
        _ => (0.0, 0.0, &[Broadleaf]),
    }
}

/// A river piece from cell centre `a` to its downstream neighbour `b`, with the channel width
/// (m) at each end.
struct RiverSeg {
    a: (f64, f64),
    b: (f64, f64),
    wa: f32,
    wb: f32,
}

/// River segments (in region cell coordinates) near a point: each river cell links to its
/// steepest-descent wet neighbour (river, lake or sea). Sources (no river flowing in) start at
/// zero width so streams taper in instead of appearing at full width.
fn river_segments(region: &ZoomRegion, cx: f64, cy: f64, radius: i64) -> Vec<RiverSeg> {
    let (w, h) = (region.width as i64, region.height as i64);
    // At person scale, rivers continue upstream as creeks: any cell with a quarter of the
    // region's river catchment carries water, with width from hydraulic geometry.
    let s = (region.params.cells_per_tile.max(8) & !1) as f32;
    let cell_km2 = (region.cell_m / 1000.0).powi(2);
    let channel_width = |k: usize| -> f32 {
        let threshold = CREEK_AREA_FRACTION * s * s / (0.25 + region.moisture[k]);
        if region.elevation_m[k] <= 0.0 || region.lake_depth_m[k] > 0.0 || region.drainage_cells[k] < threshold {
            0.0
        } else {
            region.river_width_m[k].max(0.8 * (region.drainage_cells[k] * cell_km2).sqrt())
        }
    };
    let wet = |x: i64, y: i64| {
        let k = (y * w + x) as usize;
        channel_width(k) > 0.0 || region.lake_depth_m[k] > 0.0 || region.elevation_m[k] <= 0.0
    };
    // Follow the region's own drainage links: across flat, lake-filled reaches its routing
    // differs from steepest descent, and recomputing it would break streams apart.
    let receiver = |x: i64, y: i64| -> Option<(i64, i64)> {
        let r = region.receiver[(y * w + x) as usize];
        if r == u32::MAX { None } else { Some(((r as usize % region.width) as i64, (r as usize / region.width) as i64)) }
    };
    let r = radius + 1;
    let (x0, x1) = ((cx as i64 - r).max(0), (cx as i64 + r).min(w - 1));
    let (y0, y1) = ((cy as i64 - r).max(0), (cy as i64 + r).min(h - 1));
    // Which cells have a river flowing into them (within a slightly larger window).
    let mut fed = std::collections::HashSet::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if channel_width((y * w + x) as usize) > 0.0 {
                if let Some(rc) = receiver(x, y) { fed.insert(rc); }
            }
        }
    }
    let mut segs = Vec::new();
    for y in (cy as i64 - radius).max(0)..=(cy as i64 + radius).min(h - 1) {
        for x in (cx as i64 - radius).max(0)..=(cx as i64 + radius).min(w - 1) {
            let width = channel_width((y * w + x) as usize);
            if width <= 0.0 { continue; }
            let Some((nx, ny)) = receiver(x, y) else { continue };
            let down = channel_width((ny * w + nx) as usize);
            segs.push(RiverSeg {
                a: (x as f64 + 0.5, y as f64 + 0.5),
                b: (nx as f64 + 0.5, ny as f64 + 0.5),
                // Headwater creeks start narrow.
                wa: if fed.contains(&(x, y)) { width } else { width * 0.4 },
                wb: if down > 0.0 { down } else { width },
            });
        }
    }
    segs
}

/// An ore that can occur around an embark: strength 0..1 falls with distance and grows with
/// richness.
struct VeinSource {
    kind: crate::history::civilizations::economy::ResourceType,
    strength: f32,
    noise: Perlin,
}

fn vein_sources(world: &WorldData, tile: (usize, usize), params: &crate::region::zoom::ZoomParams) -> Vec<VeinSource> {
    let res = world.resources();
    let mut best: Vec<(crate::history::civilizations::economy::ResourceType, f32)> = Vec::new();
    for d in res.deposits_near(tile.0, tile.1, 3, world.width) {
        let dx = (d.x as i64 - tile.0 as i64).abs();
        let dx = dx.min(world.width as i64 - dx) as f32;
        let dist = (dx * dx + (d.y as f32 - tile.1 as f32).powi(2)).sqrt();
        let strength = d.richness as f32 / (1.0 + dist * 1.2);
        match best.iter_mut().find(|b| b.0 == d.kind) {
            Some(b) => b.1 = b.1.max(strength),
            None => best.push((d.kind, strength)),
        }
    }
    best.into_iter()
        .map(|(kind, strength)| VeinSource { kind, strength, noise: Perlin::new((params.seed as u32).wrapping_add(900 + kind as u32 * 7)) })
        .collect()
}

/// Which ore, if any, fills this rock cell: thin, mostly horizontal seams (like real veins and
/// beds) where the noise field rises above a threshold that falls with source strength.
fn ore_vein(veins: &[VeinSource], rock: RockType, mx: f64, my: f64, z: usize) -> Option<crate::history::civilizations::economy::ResourceType> {
    for v in veins {
        let hosts = crate::lore::resources::host_rocks(v.kind);
        if !hosts.is_empty() && !hosts.contains(&rock) { continue; }
        let n = v.noise.get([mx / 38.0, my / 38.0, z as f64 * 0.55]) as f32;
        let thr = 0.74 - 0.14 * v.strength.min(2.0);
        if n > thr { return Some(v.kind); }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------------------------

/// Generate the playable area centred on region cell coordinates (`cx`, `cy`).
pub fn generate_local(world: &WorldData, region: &ZoomRegion, lore: Option<&crate::lore::RegionLore>, cx: f64, cy: f64) -> LocalMap {
    let (n, rw, rh) = (LOCAL_SIZE, region.width, region.height);
    let cell_m = region.cell_m as f64;
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    let tile_cells = TILE_M as f64 / cell_m;
    // Absolute position of region cell (0,0) in metres, for position-based noise and hashing.
    let (ox_m, oy_m) = ((region.world_x0 * s) as f64 * cell_m, (region.world_y0 * s) as f64 * cell_m);
    let world_tile = (
        ((region.world_x0 * s + cx as i64).div_euclid(s)).rem_euclid(world.width as i64) as usize,
        ((region.world_y0 * s + cy as i64).div_euclid(s)).clamp(0, world.height as i64 - 1) as usize,
    );
    let hs = world.handshakes.as_ref().map(|h| h.get(world_tile.0, world_tile.1).tile.clone()).unwrap_or_default();

    let seed = region.params.seed as u32;
    let relief_noise = Perlin::new(seed.wrapping_add(701));
    let ridge_noise = Perlin::new(seed.wrapping_add(702));
    let meander = Perlin::new(seed.wrapping_add(703));
    let patch_noise = Perlin::new(seed.wrapping_add(704));
    let strata_noise = Perlin::new(seed.wrapping_add(705));

    let segments = river_segments(region, cx, cy, 3);

    // Ore deposits within a couple of world tiles feed veins in the rock below.
    let veins = vein_sources(world, world_tile, &region.params);

    // Per-column surface, water level and climate.
    struct Col { e: f32, water_level: Option<f32>, temp: f32, moist: f32, slope: f32, river_d: f32, river_hw: f32, sea: bool }
    let mut cols: Vec<Col> = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let zx = cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells;
            let zy = cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells;
            let (mx, my) = (ox_m + zx * cell_m, oy_m + zy * cell_m);
            let base = bicubic(&region.elevation_m, rw, rh, zx, zy);
            let gx = (bicubic(&region.elevation_m, rw, rh, zx + 0.5, zy) - bicubic(&region.elevation_m, rw, rh, zx - 0.5, zy)) / cell_m as f32;
            let gy = (bicubic(&region.elevation_m, rw, rh, zx, zy + 0.5) - bicubic(&region.elevation_m, rw, rh, zx, zy - 0.5)) / cell_m as f32;
            let slope = (gx * gx + gy * gy).sqrt();
            let temp = bilinear(&region.temperature_c, rw, rh, zx, zy);
            let moist = bilinear(&region.moisture, rw, rh, zx, zy);

            // Distance to the nearest river centreline, with a meander warp of the query point.
            let wx = zx + 0.30 * fbm(&meander, mx / 700.0, my / 700.0, 3) as f64;
            let wy = zy + 0.30 * fbm(&meander, mx / 700.0 + 19.0, my / 700.0, 3) as f64;
            // Nearest edge of any channel (width varies along each segment).
            let (mut river_d, mut river_w, mut edge) = (f32::MAX, 0.0f32, f32::MAX);
            for seg in &segments {
                let (d, t) = seg_dist(wx, wy, seg.a, seg.b);
                let width = seg.wa + (seg.wb - seg.wa) * t as f32;
                let d = (d * cell_m) as f32;
                if d - width * 0.5 < edge { edge = d - width * 0.5; river_d = d; river_w = width; }
            }
            let hw = (river_w * 0.5).max(TILE_M * 0.6);
            if river_w < TILE_M * 0.5 { river_w = 0.0; } // too thin to be a channel yet

            // Metre-scale relief, stronger on slopes, calm around rivers.
            let mut amp = (0.8 + 35.0 * slope).min(40.0);
            if river_w > 0.0 { amp *= smoothstep(hw, hw + 30.0, river_d); }
            let rough = fbm(&relief_noise, mx / 90.0, my / 90.0, 5) * 2.0;
            let ridges = 1.0 - (fbm(&ridge_noise, mx / 140.0, my / 140.0, 4) * 2.0).abs();
            let steep = smoothstep(0.15, 0.6, slope);
            let mut e = base + amp * ((1.0 - steep) * rough + steep * (ridges - 0.5) * 2.0);

            let mut water_level = None;
            let bank = (hw * 0.8).max(2.0);
            if river_w > 0.0 && river_d < hw + bank {
                // Water sits a metre below the smooth valley floor; the bed is a parabola.
                let wl = base - 1.0;
                let depth = (0.3 * river_w.powf(0.6)).clamp(0.8, 8.0);
                if river_d < hw {
                    e = wl - depth * (1.0 - (river_d / hw).powi(2));
                    water_level = Some(wl);
                } else {
                    let t = smoothstep(hw, hw + bank, river_d);
                    e = (wl + 0.5) * (1.0 - t) + e.max(wl + 0.5) * t;
                }
            }
            // Lakes: the nearest region cell decides (warped), keeping its water level.
            let (lx, ly) = ((wx as i64).clamp(0, rw as i64 - 1) as usize, (wy as i64).clamp(0, rh as i64 - 1) as usize);
            let lk = ly * rw + lx;
            if region.lake_depth_m[lk] > 0.0 {
                let level = region.elevation_m[lk] + region.lake_depth_m[lk];
                let bed = level - bilinear(&region.lake_depth_m, rw, rh, zx, zy).max(1.0);
                e = e.min(bed);
                water_level = Some(level);
            }
            let sea = base <= 0.0 || e < 0.0 && water_level.is_none() && base < 2.0;
            if sea {
                water_level = Some(0.0);
            }
            cols.push(Col { e, water_level, temp, moist, slope, river_d, river_hw: if river_w > 0.0 { hw } else { 0.0 }, sea });
        }
    }

    let lo = cols.iter().map(|c| c.e).fold(f32::MAX, f32::min);
    let hi = cols.iter().map(|c| c.e.max(c.water_level.unwrap_or(f32::MIN))).fold(f32::MIN, f32::max);
    let z_min_m = ((lo / Z_STEP_M).floor() - DEPTH_BELOW as f32) * Z_STEP_M;
    let depth = (((hi - z_min_m) / Z_STEP_M).ceil() as i32 + AIR_ABOVE) as usize;
    let mut map = LocalMap {
        width: n,
        height: n,
        depth,
        z_min_m,
        cells: vec![Cell::AIR; n * n * depth],
        surface_z: vec![0; n * n],
        surface_m: cols.iter().map(|c| c.e).collect(),
        world_tile,
        biome: Biome::classify(cols[n * n / 2 + n / 2].e, cols[n * n / 2 + n / 2].temp, cols[n * n / 2 + n / 2].moist),
        features: vec![wildlife::Feature::None; n * n],
    };

    for j in 0..n {
        for i in 0..n {
            let c = &cols[j * n + i];
            let (mx, my) = (ox_m + (cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m,
                            oy_m + (cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m);
            let (gtx, gty) = ((mx / TILE_M as f64).floor() as i64, (my / TILE_M as f64).floor() as i64);
            let sz = (((c.e - z_min_m) / Z_STEP_M).floor() as i32).clamp(1, depth as i32 - 2);
            map.surface_z[j * n + i] = sz;
            let biome = Biome::classify(c.e, c.temp, c.moist);
            let underwater = c.water_level.map(|w| w > c.e).unwrap_or(false);
            let near_river = c.river_hw > 0.0 && c.river_d < c.river_hw + (c.river_hw * 0.6).max(1.5);

            // Soil: deeper on gentle, wet ground; none on cliffs.
            let soil_levels = if c.slope > 0.9 {
                0
            } else {
                ((hs.sediment_depth as f32).min(8.0) * (1.0 - c.slope * 1.1).clamp(0.15, 1.0)).round().max(1.0) as i32
            };
            let soil = if near_river || c.sea && c.e > -3.0 {
                if hash(gtx, gty, 1) < 0.5 { Material::Sand } else { Material::Gravel }
            } else if matches!(biome, Biome::Desert) || c.moist < 0.08 {
                Material::Sand
            } else if c.moist > 0.6 && c.slope < 0.05 {
                Material::Clay
            } else {
                Material::Soil
            };
            // Strata boundaries undulate gently instead of being perfectly flat.
            let wobble = (fbm(&strata_noise, mx / 160.0, my / 160.0, 3) * 4.0).round() as i32;
            let glacier = matches!(biome, Biome::Ice) || c.temp < -18.0;

            for z in 0..sz {
                let below = sz - z; // 1 = directly under the floor
                let material = if glacier && below <= 4 {
                    Material::Ice
                } else if below <= soil_levels {
                    soil
                } else {
                    let mut d = below - soil_levels + wobble;
                    let mut rock = RockType::Granite;
                    for layer in &hs.rock_stack {
                        rock = layer.rock_type;
                        if d <= layer.thickness as i32 { break; }
                        d -= layer.thickness as i32;
                    }
                    Material::Rock(rock)
                };
                // Ore veins in host rock, thicker the closer and richer the nearest deposit.
                let material = match material {
                    Material::Rock(rock) => ore_vein(&veins, rock, mx, my, z as usize).map(Material::Ore).unwrap_or(material),
                    other => other,
                };
                let k = map.idx(i, j, z as usize);
                map.cells[k] = Cell { shape: Shape::Wall, material, water: 0, plant: Plant::None, boulder: false };
            }

            // The floor: snow in the cold, bare rock where there is no soil. Under water it is
            // the river or lake bed.
            let floor_material = if underwater {
                if c.sea || near_river { soil } else { Material::Clay }
            } else if glacier || c.temp < -2.0 {
                // Glaciers are snow-covered at the surface (ice below); ice floors mean water.
                Material::Snow
            } else if soil_levels == 0 {
                map.cell(i, j, (sz - 1).max(0) as usize).material
            } else {
                soil
            };
            let (tree_density, shrub_density, species) = vegetation(biome);
            // Forests have clearings and groves rather than uniform density.
            let grove = 0.35 + 0.95 * smoothstep(-0.3, 0.3, fbm(&patch_noise, mx / 150.0, my / 150.0, 3));
            let r = hash(gtx, gty, 7);
            let plant = if underwater || near_river || matches!(floor_material, Material::Ice | Material::Rock(_)) {
                Plant::None
            } else if r < tree_density * grove {
                Plant::Tree(species[(hash(gtx, gty, 9) * species.len() as f32) as usize % species.len()])
            } else if r < tree_density * grove + shrub_density {
                Plant::Shrub
            } else if matches!(floor_material, Material::Soil | Material::Clay) && c.moist > 0.12 && c.temp > -3.0 {
                Plant::Grass
            } else {
                Plant::None
            };
            let boulder = !underwater && plant == Plant::None && hash(gtx, gty, 11) < 0.004 + 0.06 * smoothstep(0.2, 0.8, c.slope);
            let k = map.idx(i, j, sz as usize);
            map.cells[k] = Cell { shape: Shape::Floor, material: floor_material, water: 0, plant, boulder };

            // Water above the floor, filled to the water level; its top level freezes in the cold. Depth in the first level is
            // measured from the actual ground, so water shallower than a z-level still shows.
            if let Some(level) = c.water_level {
                for z in sz + 1..depth as i32 {
                    let bottom = if z == sz + 1 { c.e } else { map.z_elevation(z) };
                    if bottom >= level { break; }
                    let amount = (((level - bottom) / Z_STEP_M).min(1.0) * WATER_FULL as f32).round() as u8;
                    let k = map.idx(i, j, z as usize);
                    map.cells[k].water = amount.max(1);
                }
                // Freeze over unless the water's surface sits within the floor's own level (the
                // ice would replace the ground; a frozen puddle reads better as snow anyway).
                if c.temp < FREEZE_TEMP_C {
                    let top = ((level - z_min_m) / Z_STEP_M).ceil() as i32 - 1;
                    if top > sz && (top as usize) < depth {
                        let k = map.idx(i, j, top as usize);
                        map.cells[k] = Cell { shape: Shape::Floor, material: Material::Ice, water: 0, plant: Plant::None, boulder: false };
                        map.surface_z[j * n + i] = top;
                    } else {
                        // Water shallower than one level freezes solid onto the bed.
                        let k = map.idx(i, j, sz as usize);
                        map.cells[k].material = Material::Ice;
                        map.cells[k].plant = Plant::None;
                        for z in sz + 1..depth as i32 {
                            let k = map.idx(i, j, z as usize);
                            map.cells[k].water = 0;
                        }
                    }
                }
            }
        }
    }

    // What history left here: settlements, ruins, fields and roads.
    if let Some(lore) = lore {
        structures::apply(&mut map, region, lore, cx, cy);
        // Animals: trails, burrows, nests, dens (keyed on absolute 2 m tile position).
        let origin = (
            ((ox_m + (cx - n as f64 / 2.0 * tile_cells) * cell_m) / TILE_M as f64).round() as i64,
            ((oy_m + (cy - n as f64 / 2.0 * tile_cells) * cell_m) / TILE_M as f64).round() as i64,
        );
        wildlife::apply(&mut map, world, lore, origin);
    }

    // Ramps: a floor next to ground exactly one level higher leads up (DF style).
    for j in 0..n {
        for i in 0..n {
            let sz = map.surface_z[j * n + i];
            let up = [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)].iter().any(|&(dx, dy)| {
                let (x, y) = (i as i64 + dx, j as i64 + dy);
                x >= 0 && y >= 0 && x < n as i64 && y < n as i64 && map.surface_z[y as usize * n + x as usize] == sz + 1
            });
            let k = map.idx(i, j, sz as usize);
            let built_over = (sz as usize + 1) < map.depth && map.cells[map.idx(i, j, sz as usize + 1)].shape == Shape::Wall;
            let paved = matches!(map.cells[k].material, Material::Wood | Material::Block(_));
            if up && map.cells[k].plant == Plant::None && map.cells[k].water == 0 && !built_over && !paved {
                map.cells[k].shape = Shape::Ramp;
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A playable area is solid below its surface, open above it, ramps connect single steps,
    /// and water only sits in open cells.
    #[test]
    fn local_map_is_consistent() {
        let world = crate::world::generate_world_with_style(128, 64, 5, crate::plates::WorldStyle::Earthlike);
        let (cx, cy) = crate::region::zoom::pick_interesting_window(&world, 4);
        let params = crate::region::zoom::ZoomParams { center_x: cx, center_y: cy, tiles: 4, cells_per_tile: 32, erosion_iterations: 10, seed: 3 };
        let region = crate::region::zoom::generate_zoom(&world, &params);
        let map = generate_local(&world, &region, None, region.width as f64 / 2.0, region.height as f64 / 2.0);
        assert_eq!(map.cells.len(), LOCAL_SIZE * LOCAL_SIZE * map.depth);
        for y in 0..map.height {
            for x in 0..map.width {
                let sz = map.surface_z[y * map.width + x] as usize;
                for z in 0..map.depth {
                    let c = map.cell(x, y, z);
                    if z < sz { assert!(c.shape == Shape::Wall || c.water > 0, "solid (or frozen-over water) below the surface"); }
                    if z == sz { assert!(matches!(c.shape, Shape::Floor | Shape::Ramp)); }
                    if z > sz && c.shape != Shape::Empty {
                        assert!(matches!(c.material, Material::Wood | Material::Block(_) | Material::Clay | Material::Ice), "only buildings or ice above the surface");
                    }
                    if c.water > 0 { assert_eq!(c.shape, Shape::Empty, "water only in open cells"); }
                }
            }
        }
        let trees = map.cells.iter().filter(|c| matches!(c.plant, Plant::Tree(_))).count();
        println!("local map: {} z-levels, {} trees, biome {:?}", map.depth, trees, map.biome);
    }
}
