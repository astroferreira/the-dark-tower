//! Zoomed regional terrain: re-simulates a window of world tiles at high resolution.
//!
//! Each world tile becomes `cells_per_tile` x `cells_per_tile` cells. The pipeline keeps the
//! world as the boundary condition and lets physics fill in the detail:
//!
//! 1. **Base terrain**: bicubic upsampling of the world heightmap, plus fractal relief scaled
//!    to the local world relief (ridged in mountains, gentle on plains, damped at coasts).
//! 2. **World rivers as constraints**: the world's Bezier rivers carve valleys into the base,
//!    and rivers entering the window from outside inject their real upstream drainage area.
//! 3. **Landscape evolution**: implicit stream-power incision (Braun & Willett 2013) plus
//!    hillslope diffusion on a priority-flood drainage tree, so every cell drains to the sea or
//!    out of the window and dendritic valleys grow out of the noise.
//! 4. **Hydrology**: lakes from depression filling, rivers from contributing area (fewer in
//!    dry climates), widths from hydraulic geometry.
//!
//! Seamlessness: terrain is built from fixed world-tile chunks (each simulated with a one-tile
//! border) blended with tent weights, so a cell's height depends only on its world position.
//! Regions generated separately, in any order, join exactly. Rivers and lakes are computed on
//! the blended terrain with a two-tile margin so streams see their catchment across edges.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;

use image::{ImageBuffer, Luma, Rgb, RgbImage};
use noise::{NoiseFn, Perlin};
use rayon::prelude::*;

use crate::map_export::{bilinear_biome_color, sample_lut};
use crate::tilemap::Tilemap;
use crate::world::WorldData;

const EARTH_CIRCUMFERENCE_KM: f32 = 40_075.0;
const NONE: u32 = u32::MAX;
/// Dimensionless stream-power coefficient per iteration (F = K * sqrt(A_cells) / distance).
const STREAM_POWER_K: f32 = 0.015;
/// Dimensionless hillslope diffusion per iteration.
const HILLSLOPE_D: f32 = 0.08;
/// Fraction of one world tile's area a catchment needs before it shows as a river (humid).
const RIVER_AREA_FRACTION: f32 = 0.02;
const LAKE_MIN_DEPTH_M: f32 = 0.5;
/// Radius (world tiles) of the smoothing applied to world temperature/moisture before zooming.
const CLIMATE_SMOOTH_TILES: usize = 1;
/// Share of the world biome colour mixed over the local climate colour table when rendering.
/// Zero by default: the world biome map is tile-sized, and any visible share of it paints the
/// world grid onto the zoom; the climate table already follows the zoomed terrain.
const BIOME_COLOR_WEIGHT: f32 = 0.0;
/// Closed hollows shallower than this (m) are filled before erosion; deeper ones stay lakes.
const SHALLOW_HOLLOW_M: f32 = 25.0;
/// Slope (m per cell) given to filled hollows so they drain toward their outlet.
const PREFILL_GRADIENT_M: f32 = 0.02;
/// Amplitude (m) of the smooth perturbation added to filled flats.
const FLAT_PERTURBATION_M: f32 = 0.4;

#[derive(Clone, Debug)]
pub struct ZoomParams {
    /// World tile at the centre of the window.
    pub center_x: usize,
    pub center_y: usize,
    /// Window size in world tiles.
    pub tiles: usize,
    /// Cells per world tile edge.
    pub cells_per_tile: usize,
    /// Landscape-evolution iterations.
    pub erosion_iterations: usize,
    pub seed: u64,
}

impl Default for ZoomParams {
    fn default() -> Self {
        Self { center_x: 0, center_y: 0, tiles: 8, cells_per_tile: 128, erosion_iterations: 40, seed: 0 }
    }
}

/// A generated high-resolution region (margin already cropped).
pub struct ZoomRegion {
    pub width: usize,
    pub height: usize,
    /// Approximate size of one cell in metres.
    pub cell_m: f32,
    /// World tile at the window's top-left corner.
    pub world_x0: i64,
    pub world_y0: i64,
    pub params: ZoomParams,
    pub elevation_m: Vec<f32>,
    /// Contributing drainage area in cells (including inflow from outside the window).
    pub drainage_cells: Vec<f32>,
    /// Index of each cell's downstream neighbour in this region (`u32::MAX` if it drains to
    /// the sea, out of the window, or is an outlet).
    pub receiver: Vec<u32>,
    /// River channel width in metres (0 = no river).
    pub river_width_m: Vec<f32>,
    pub lake_depth_m: Vec<f32>,
    pub temperature_c: Vec<f32>,
    pub moisture: Vec<f32>,
    pub biome_color: Vec<(u8, u8, u8)>,
    /// How much of `biome_color` to mix over the climate colour table (0 = climate only).
    pub biome_weight: Vec<f32>,
}

// ---------------------------------------------------------------------------------------------
// Sampling helpers
// ---------------------------------------------------------------------------------------------

fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

/// Bicubic (Catmull-Rom) sample of a world field at fractional tile coordinates (cell centres
/// at integers; x wraps, y clamps).
fn bicubic(map: &Tilemap<f32>, u: f32, v: f32) -> f32 {
    let (w, h) = (map.width as i64, map.height as i64);
    let (x0, y0) = (u.floor() as i64, v.floor() as i64);
    let (tx, ty) = (u - x0 as f32, v - y0 as f32);
    let mut rows = [0.0f32; 4];
    for (k, row) in rows.iter_mut().enumerate() {
        let yy = (y0 + k as i64 - 1).clamp(0, h - 1) as usize;
        let s = |m: i64| *map.get((x0 + m).rem_euclid(w) as usize, yy);
        *row = catmull(s(-1), s(0), s(1), s(2), tx);
    }
    catmull(rows[0], rows[1], rows[2], rows[3], ty)
}

/// Separable box blur of radius `r` cells, applied `passes` times (x wraps, y clamps).
fn box_blur(map: &Tilemap<f32>, r: usize, passes: usize) -> Tilemap<f32> {
    let (w, h) = (map.width, map.height);
    let mut cur = map.clone();
    let ri = r as i64;
    let norm = (2 * r + 1) as f32;
    for _ in 0..passes {
        let mut tmp = cur.clone();
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                for d in -ri..=ri {
                    sum += *cur.get((x as i64 + d).rem_euclid(w as i64) as usize, y);
                }
                tmp.set(x, y, sum / norm);
            }
        }
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                for d in -ri..=ri {
                    sum += *tmp.get(x, (y as i64 + d).clamp(0, h as i64 - 1) as usize);
                }
                cur.set(x, y, sum / norm);
            }
        }
    }
    cur
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm(noise: &Perlin, u: f64, v: f64, octaves: u32) -> f32 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([u * f + 0.37, v * f + 0.71]);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    (sum / norm) as f32
}

fn ridged(noise: &Perlin, u: f64, v: f64, octaves: u32) -> f32 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for _ in 0..octaves {
        let n = 1.0 - noise.get([u * f + 5.3, v * f + 2.9]).abs();
        sum += amp * n * n;
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    (sum / norm) as f32
}

/// Total ordering key for f32 heights in a min-heap.
fn height_key(h: f32) -> u32 {
    let b = h.to_bits();
    if b >> 31 == 0 { b | 0x8000_0000 } else { !b }
}

// ---------------------------------------------------------------------------------------------
// Drainage
// ---------------------------------------------------------------------------------------------

struct Drainage {
    /// Cells from outlets upward (valid topological order for accumulation).
    order: Vec<u32>,
    /// Downstream neighbour (NONE for outlets).
    recv: Vec<u32>,
    /// Depression-filled water level.
    filled: Vec<f32>,
}

/// Priority-flood from the sea and the grid border. Every cell gets a receiver that leads to an
/// outlet: steepest descent where the ground falls, the spill path across lakes and flats.
/// `fill_gradient` (m per step) makes filled depressions slope gently toward their outlet.
fn route(h: &[f32], w: usize, ht: usize, fill_gradient: f32) -> Drainage {
    let n = w * ht;
    let mut filled = h.to_vec();
    let mut recv = vec![NONE; n];
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut heap = BinaryHeap::new();
    for y in 0..ht {
        for x in 0..w {
            let i = y * w + x;
            if h[i] <= 0.0 || x == 0 || y == 0 || x == w - 1 || y == ht - 1 {
                done[i] = true;
                heap.push(Reverse((height_key(h[i]), i as u32)));
            }
        }
    }
    while let Some(Reverse((_, i))) = heap.pop() {
        let i = i as usize;
        order.push(i as u32);
        let (x, y) = (i % w, i / w);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= ht as i64 { continue; }
                let j = ny as usize * w + nx as usize;
                if done[j] { continue; }
                done[j] = true;
                filled[j] = h[j].max(filled[i] + fill_gradient);
                recv[j] = i as u32;
                heap.push(Reverse((height_key(filled[j]), j as u32)));
            }
        }
    }
    // Where the ground actually falls, prefer the steepest strictly-lower neighbour (true D8).
    // Strictly lower filled cells were popped earlier, so `order` stays topological.
    for i in 0..n {
        if recv[i] == NONE { continue; }
        let (x, y) = (i % w, i / w);
        let mut best = 0.0f32;
        let mut best_j = NONE;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= ht as i64 { continue; }
                let j = ny as usize * w + nx as usize;
                let drop = filled[i] - filled[j];
                if drop <= 0.0 { continue; }
                let slope = drop / if dx != 0 && dy != 0 { std::f32::consts::SQRT_2 } else { 1.0 };
                if slope > best { best = slope; best_j = j as u32; }
            }
        }
        if best_j != NONE { recv[i] = best_j; }
    }
    Drainage { order, recv, filled }
}

/// Contributing area (cells) including injected inflow.
fn accumulate(d: &Drainage, inflow: &[f32]) -> Vec<f32> {
    let mut acc: Vec<f32> = inflow.iter().map(|&q| 1.0 + q).collect();
    for &i in d.order.iter().rev() {
        let r = d.recv[i as usize];
        if r != NONE {
            acc[r as usize] += acc[i as usize];
        }
    }
    acc
}

// ---------------------------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------------------------

/// Pick the window centre with the most river-bearing, high-relief land (avoiding the poles).
pub fn pick_interesting_window(world: &WorldData, tiles: usize) -> (usize, usize) {
    let (w, h) = (world.width, world.height);
    let flow = world.flow_accumulation.as_ref();
    let mut best = (w / 2, h / 2);
    let mut best_score = f32::MIN;
    // Stay within ~60 degrees of the equator.
    let y_min = (h as f32 * 0.17) as usize;
    let y_max = (h as f32 * 0.83) as usize;
    for cy in (y_min + tiles / 2..y_max.saturating_sub(tiles / 2)).step_by(2) {
        for cx in (0..w).step_by(2) {
            let (mut land, mut rivers, mut relief) = (0usize, 0usize, 0.0f32);
            for dy in 0..tiles {
                for dx in 0..tiles {
                    let x = (cx + w + dx - tiles / 2) % w;
                    let y = (cy + dy - tiles / 2).min(h - 1);
                    let e = *world.heightmap.get(x, y);
                    if e > 0.0 {
                        land += 1;
                        // Local relief (not altitude): a high flat plateau is not interesting.
                        let xr = (x + 1) % w;
                        let yr = (y + 1).min(h - 1);
                        relief += (e - *world.heightmap.get(xr, y)).abs() + (e - *world.heightmap.get(x, yr)).abs();
                        if flow.map(|f| *f.get(x, y) > 50.0).unwrap_or(false) { rivers += 1; }
                    }
                }
            }
            if land * 2 < tiles * tiles { continue; }
            let score = rivers as f32 * 5.0 + relief / 200.0 + land as f32 * 0.2;
            if score > best_score { best_score = score; best = (cx, cy); }
        }
    }
    best
}

/// World-tile chunks are simulated with this many tiles of border on each side.
const CHUNK_BORDER_TILES: i64 = 1;
/// Hydrology margin (tiles) simulated around the requested window, so streams crossing its
/// edge see their upstream catchment.
const FINAL_MARGIN_TILES: usize = 2;
/// Cached chunks kept in memory before the cache is cleared (~260 KB each at 128 cells/tile).
const CHUNK_CACHE_LIMIT: usize = 1500;

/// Everything chunk simulations share: world references, noise and derived world fields.
struct Ctx<'a> {
    world: &'a WorldData,
    s: usize,
    octaves: u32,
    iterations: usize,
    relief: Tilemap<f32>,
    smooth_temp: Tilemap<f32>,
    smooth_moist: Tilemap<f32>,
    detail_noise: Perlin,
    ridge_noise: Perlin,
    flat_noise: Perlin,
    world_cell_km: f32,
    cell_m: f32,
    /// Cheap fingerprint of the world heightmap (cache key).
    fingerprint: u64,
}

impl<'a> Ctx<'a> {
    fn new(world: &'a WorldData, params: &ZoomParams) -> Self {
        let s = params.cells_per_tile.max(8) & !1; // even, so chunk centres fall between cells
        let (ww, wh) = (world.width, world.height);
        let mut relief = Tilemap::new_with(ww, wh, 0.0f32);
        for y in 0..wh {
            for x in 0..ww {
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let yy = (y as i64 + dy).clamp(0, wh as i64 - 1) as usize;
                        let xx = (x as i64 + dx).rem_euclid(ww as i64) as usize;
                        let e = *world.heightmap.get(xx, yy);
                        lo = lo.min(e);
                        hi = hi.max(e);
                    }
                }
                relief.set(x, y, hi - lo);
            }
        }
        let mut fingerprint = (ww as u64) << 32 ^ wh as u64 ^ world.seed().rotate_left(17);
        for (_, _, &e) in world.heightmap.iter() {
            fingerprint = fingerprint.rotate_left(5) ^ e.to_bits() as u64;
        }
        let seed = params.seed as u32;
        let world_cell_km = EARTH_CIRCUMFERENCE_KM / ww as f32;
        Self {
            world,
            s,
            octaves: ((s as f32).log2() as u32).saturating_sub(1).max(3),
            iterations: params.erosion_iterations,
            relief,
            // Climate fields are smoothed over a couple of world tiles before sampling: the world
            // climate can be blocky at tile scale, and bicubic upsampling would turn each step
            // into a visible soft-edged rectangle in the zoom.
            smooth_temp: box_blur(&world.temperature, CLIMATE_SMOOTH_TILES, 3),
            smooth_moist: box_blur(&world.moisture, CLIMATE_SMOOTH_TILES, 3),
            detail_noise: Perlin::new(seed.wrapping_add(101)),
            ridge_noise: Perlin::new(seed.wrapping_add(202)),
            flat_noise: Perlin::new(seed.wrapping_add(303)),
            world_cell_km,
            cell_m: world_cell_km * 1000.0 / s as f32,
            fingerprint: fingerprint ^ params.seed.rotate_left(29),
        }
    }

    /// World tile coordinate of cell (i, j) in a grid whose cell 0 starts at tile (gx0, gy0).
    fn to_world(&self, gx0: i64, gy0: i64, i: usize, j: usize) -> (f32, f32) {
        let s = self.s as f32;
        (gx0 as f32 + (i as f32 + 0.5) / s - 0.5, gy0 as f32 + (j as f32 + 0.5) / s - 0.5)
    }

    /// Upsampled world height plus relief-scaled fractal detail (a pure function of position).
    fn base_height(&self, u: f32, v: f32) -> f32 {
        let h0 = bicubic(&self.world.heightmap, u, v);
        let r = bicubic(&self.relief, u, v).max(0.0);
        let mut amp = if h0 > 0.0 { 15.0 + 0.30 * r } else { 5.0 + 0.05 * r };
        amp = amp.min(0.6 * h0.abs() + 25.0); // keep coastlines recognisable
        let mw = smoothstep(600.0, 2500.0, h0);
        let d = fbm(&self.detail_noise, u as f64, v as f64, self.octaves);
        let rdg = (ridged(&self.ridge_noise, u as f64, v as f64, self.octaves) - 0.5) * 2.0;
        h0 + amp * ((1.0 - mw) * d * 2.0 + mw * rdg)
    }

    /// World rivers over a grid: optionally carve their valleys, and inject their upstream
    /// drainage area where they enter the grid.
    fn world_rivers(&self, gx0: i64, gy0: i64, gw: usize, gh: usize, mut carve: Option<&mut Vec<f32>>, inflow: &mut [f32]) {
        let Some(net) = &self.world.river_network else { return };
        let (s, ww, wh) = (self.s, self.world.width as f32, self.world.height);
        let lat_cos = |v: f32| (std::f32::consts::FRAC_PI_2 - (v + 0.5) / wh as f32 * std::f32::consts::PI).cos().abs().max(0.05);
        let (tw, th) = (gw as f32 / s as f32, gh as f32 / s as f32);
        let to_grid = |wx: f32, wy: f32| -> Option<(i64, i64, f32, f32)> {
            let du = (wx - gx0 as f32).rem_euclid(ww);
            let fi = (du + 0.5) * s as f32 - 0.5;
            let fj = (wy - gy0 as f32 + 0.5) * s as f32 - 0.5;
            let (ci, cj) = (fi.round() as i64, fj.round() as i64);
            if ci >= 0 && cj >= 0 && ci < gw as i64 && cj < gh as i64 { Some((ci, cj, fi, fj)) } else { None }
        };
        let near = |wx: f32, wy: f32| {
            let du = (wx - gx0 as f32 + 2.0).rem_euclid(ww);
            let dv = wy - gy0 as f32;
            du <= tw + 4.0 && dv >= -2.0 && dv <= th + 2.0
        };
        for seg in &net.segments {
            if ![&seg.p0, &seg.p1, &seg.p2, &seg.p3].iter().any(|p| near(p.world_x, p.world_y)) {
                continue;
            }
            let len = seg.approximate_length(8) * s as f32;
            let samples = (len * 2.0).ceil() as usize + 2;
            let mut was_inside = to_grid(seg.p0.world_x, seg.p0.world_y).is_some();
            for k in 0..=samples {
                let pt = seg.evaluate(k as f32 / samples as f32);
                let Some((ci, cj, fi, fj)) = to_grid(pt.world_x, pt.world_y) else {
                    was_inside = false;
                    continue;
                };
                if !was_inside {
                    inflow[cj as usize * gw + ci as usize] += pt.flow_accumulation * (s * s) as f32;
                    was_inside = true;
                }
                let Some(carve) = carve.as_deref_mut() else { continue };
                let area_km2 = pt.flow_accumulation * self.world_cell_km * self.world_cell_km * lat_cos(pt.world_y);
                let width_m = 0.8 * area_km2.sqrt();
                let depth = 8.0 * (1.0 + area_km2 / 100.0).ln();
                let rv = (4.0 * width_m / self.cell_m).max(3.0);
                let ri = rv.ceil() as i64;
                for dj in -ri..=ri {
                    for di in -ri..=ri {
                        let (x, y) = (ci + di, cj + dj);
                        if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { continue; }
                        let d = ((x as f32 - fi).powi(2) + (y as f32 - fj).powi(2)).sqrt() / rv;
                        if d >= 1.0 { continue; }
                        let c = depth * (1.0 - d * d);
                        let k = y as usize * gw + x as usize;
                        if c > carve[k] { carve[k] = c; }
                    }
                }
            }
        }
    }

    /// Simulate one chunk (world tile `cx, cy`) with a border of `CHUNK_BORDER_TILES` and
    /// return the eroded heights of its central 2x2-tile region (row-major, 2s x 2s), which is
    /// exactly where its blend weight is non-zero. Depends only on the world and `cx, cy`.
    fn simulate_chunk(&self, cx: i64, cy: i64) -> Vec<f32> {
        let s = self.s;
        let (gx0, gy0) = (cx - CHUNK_BORDER_TILES, cy - CHUNK_BORDER_TILES);
        let gw = (1 + 2 * CHUNK_BORDER_TILES as usize) * s;
        let n = gw * gw;
        let mut h: Vec<f32> = (0..n)
            .map(|k| {
                let (u, v) = self.to_world(gx0, gy0, k % gw, k / gw);
                self.base_height(u, v)
            })
            .collect();

        let mut carve = vec![0.0f32; n];
        let mut inflow = vec![0.0f32; n];
        self.world_rivers(gx0, gy0, gw, gw, Some(&mut carve), &mut inflow);
        for k in 0..n {
            if h[k] > 0.0 { h[k] = (h[k] - carve[k]).max(0.5); }
        }

        // Drainage integration: fill shallow closed hollows (with a slight slope toward their
        // outlet and a tiny smooth perturbation so flow doesn't run in straight lines).
        let d = route(&h, gw, gw, PREFILL_GRADIENT_M);
        for k in 0..n {
            let fill = d.filled[k] - h[k];
            if h[k] > 0.0 && fill > 0.0 && fill < SHALLOW_HOLLOW_M {
                let (u, v) = self.to_world(gx0, gy0, k % gw, k / gw);
                h[k] = d.filled[k] + FLAT_PERTURBATION_M * fbm(&self.flat_noise, u as f64 * 24.0, v as f64 * 24.0, 3);
            }
        }

        // Landscape evolution: implicit stream power + hillslope diffusion.
        let land0: Vec<bool> = h.iter().map(|&e| e > 0.0).collect();
        for _ in 0..self.iterations {
            let d = route(&h, gw, gw, 0.0);
            let acc = accumulate(&d, &inflow);
            for &i in &d.order {
                let i = i as usize;
                let r = d.recv[i];
                if r == NONE || !land0[i] { continue; }
                let r = r as usize;
                let dist = if (i % gw != r % gw) && (i / gw != r / gw) { std::f32::consts::SQRT_2 } else { 1.0 };
                let f = STREAM_POWER_K * acc[i].sqrt() / dist;
                if h[i] > h[r] {
                    h[i] = (h[i] + f * h[r]) / (1.0 + f);
                }
            }
            let prev = h.clone();
            for y in 1..gw - 1 {
                for x in 1..gw - 1 {
                    let k = y * gw + x;
                    if !land0[k] { continue; }
                    let lap = prev[k - 1] + prev[k + 1] + prev[k - gw] + prev[k + gw] - 4.0 * prev[k];
                    h[k] = (prev[k] + HILLSLOPE_D * lap).max(0.5);
                }
            }
        }

        // Central 2s x 2s region: global cells [(cx - 0.5) s, (cx + 1.5) s).
        let off = s / 2;
        let mut out = Vec::with_capacity(4 * s * s);
        for j in off..off + 2 * s {
            out.extend_from_slice(&h[j * gw + off..j * gw + off + 2 * s]);
        }
        out
    }
}

type ChunkKey = (u64, usize, usize, i64, i64);

fn chunk_cache() -> &'static std::sync::Mutex<std::collections::HashMap<ChunkKey, std::sync::Arc<Vec<f32>>>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<ChunkKey, std::sync::Arc<Vec<f32>>>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Generate a high-resolution region. Terrain is assembled from fixed world-tile chunks
/// blended with tent weights (a partition of unity), so every cell's height depends only on its
/// world position: regions generated separately (in any order) join seamlessly.
pub fn generate_zoom(world: &WorldData, params: &ZoomParams) -> ZoomRegion {
    let ctx = Ctx::new(world, params);
    let s = ctx.s;
    let tiles = params.tiles.max(1);
    let (ww, wh) = (world.width, world.height);
    let core_x0 = params.center_x as i64 - (tiles / 2) as i64;
    let core_y0 = params.center_y as i64 - (tiles / 2) as i64;
    let mf = FINAL_MARGIN_TILES;
    let (gx0, gy0) = (core_x0 - mf as i64, core_y0 - mf as i64);
    let gt = tiles + 2 * mf;
    let gw = gt * s;
    let n = gw * gw;

    // 1. Chunks: fetch from the cache, simulate the missing ones in parallel.
    let keys: Vec<(i64, i64)> = (gy0 - 1..=gy0 + gt as i64)
        .flat_map(|cy| (gx0 - 1..=gx0 + gt as i64).map(move |cx| (cx, cy)))
        .collect();
    let key_of = |(cx, cy): (i64, i64)| -> ChunkKey { (ctx.fingerprint, s, ctx.iterations, cx, cy) };
    let mut chunks: std::collections::HashMap<(i64, i64), std::sync::Arc<Vec<f32>>> = {
        let cache = chunk_cache().lock().unwrap();
        keys.iter().filter_map(|&k| cache.get(&key_of(k)).map(|c| (k, c.clone()))).collect()
    };
    let missing: Vec<(i64, i64)> = keys.iter().copied().filter(|k| !chunks.contains_key(k)).collect();
    let fresh: Vec<((i64, i64), std::sync::Arc<Vec<f32>>)> = missing
        .par_iter()
        .map(|&(cx, cy)| ((cx, cy), std::sync::Arc::new(ctx.simulate_chunk(cx, cy))))
        .collect();
    {
        let mut cache = chunk_cache().lock().unwrap();
        if cache.len() + fresh.len() > CHUNK_CACHE_LIMIT { cache.clear(); }
        for (k, c) in &fresh {
            cache.insert(key_of(*k), c.clone());
        }
    }
    chunks.extend(fresh);

    // 2. Blend: each cell mixes the two nearest chunk centres in x and in y (tent weights).
    let mut h = vec![0.0f32; n];
    for j in 0..gw {
        let gy = gy0 * s as i64 + j as i64;
        let fy = (gy as f32 + 0.5) / s as f32 - 0.5;
        let c0y = fy.floor() as i64;
        let ty = fy - c0y as f32;
        for i in 0..gw {
            let gx = gx0 * s as i64 + i as i64;
            let fx = (gx as f32 + 0.5) / s as f32 - 0.5;
            let c0x = fx.floor() as i64;
            let tx = fx - c0x as f32;
            let mut v = 0.0;
            for (by, wy) in [(0i64, 1.0 - ty), (1, ty)] {
                for (bx, wx) in [(0i64, 1.0 - tx), (1, tx)] {
                    let (cx, cy) = (c0x + bx, c0y + by);
                    let lx = (gx - cx * s as i64 + (s / 2) as i64) as usize;
                    let ly = (gy - cy * s as i64 + (s / 2) as i64) as usize;
                    v += wx * wy * chunks[&(cx, cy)][ly * 2 * s + lx];
                }
            }
            h[j * gw + i] = v;
        }
    }

    // 3. Climate (a pure function of position and local height).
    let (mut temp, mut moist) = (vec![0.0f32; n], vec![0.0f32; n]);
    temp.par_iter_mut().zip(moist.par_iter_mut()).enumerate().for_each(|(k, (t, m))| {
        let (u, v) = ctx.to_world(gx0, gy0, k % gw, k / gw);
        let h0 = bicubic(&world.heightmap, u, v);
        // Warp the climate lookup slightly so its gradients follow organic shapes.
        let cu = u + 0.6 * fbm(&ctx.detail_noise, u as f64 * 0.9 + 41.0, v as f64 * 0.9, 3);
        let cv = v + 0.6 * fbm(&ctx.detail_noise, u as f64 * 0.9, v as f64 * 0.9 + 53.0, 3);
        *t = bicubic(&ctx.smooth_temp, cu, cv) - 6.5 * (h[k] - h0) / 1000.0;
        *m = bicubic(&ctx.smooth_moist, cu, cv).clamp(0.0, 1.0);
    });

    // 4. Hydrology on the blended terrain (with the margin), inflow from world rivers.
    let mut inflow = vec![0.0f32; n];
    ctx.world_rivers(gx0, gy0, gw, gw, None, &mut inflow);
    let d = route(&h, gw, gw, 0.0);
    let acc = accumulate(&d, &inflow);
    let cell_km2 = (ctx.cell_m / 1000.0).powi(2);
    let tile_cells = (s * s) as f32;

    // 5. Crop to the requested window.
    let (w, ht) = (tiles * s, tiles * s);
    let off = mf * s;
    let mut out = ZoomRegion {
        width: w,
        height: ht,
        cell_m: ctx.cell_m,
        world_x0: core_x0,
        world_y0: core_y0,
        params: params.clone(),
        elevation_m: Vec::with_capacity(w * ht),
        drainage_cells: Vec::with_capacity(w * ht),
        receiver: Vec::with_capacity(w * ht),
        river_width_m: Vec::with_capacity(w * ht),
        lake_depth_m: Vec::with_capacity(w * ht),
        temperature_c: Vec::with_capacity(w * ht),
        moisture: Vec::with_capacity(w * ht),
        biome_color: Vec::with_capacity(w * ht),
        biome_weight: Vec::with_capacity(w * ht),
    };
    for j in off..off + ht {
        for i in off..off + w {
            let k = j * gw + i;
            let e = h[k];
            let lake = if e > 0.0 { (d.filled[k] - e).max(0.0) } else { 0.0 };
            let lake = if lake >= LAKE_MIN_DEPTH_M { lake } else { 0.0 };
            // Drier ground needs a larger catchment before a permanent stream appears.
            let threshold = RIVER_AREA_FRACTION * tile_cells / (0.25 + moist[k]);
            let river = if e > 0.0 && lake == 0.0 && acc[k] > threshold {
                0.8 * (acc[k] * cell_km2).sqrt() // hydraulic geometry: width (m) ~ 0.8 sqrt(A km2)
            } else {
                0.0
            };
            let (u, v) = ctx.to_world(gx0, gy0, i, j);
            out.elevation_m.push(e);
            out.drainage_cells.push(acc[k]);
            let r = d.recv[k];
            out.receiver.push(if r == NONE {
                u32::MAX
            } else {
                let (rx, ry) = (r as usize % gw, r as usize / gw);
                if rx >= off && ry >= off && rx < off + w && ry < off + ht {
                    ((ry - off) * w + (rx - off)) as u32
                } else {
                    u32::MAX
                }
            });
            out.river_width_m.push(river);
            out.lake_depth_m.push(lake);
            out.temperature_c.push(temp[k]);
            out.moisture.push(moist[k]);
            // Biome colour from the world, with the lookup warped by noise so biome borders
            // follow organic shapes instead of the world-tile grid.
            let wu = u + 0.45 * fbm(&ctx.detail_noise, u as f64 * 1.3 + 17.0, v as f64 * 1.3, 4);
            let wv = v + 0.45 * fbm(&ctx.detail_noise, u as f64 * 1.3, v as f64 * 1.3 + 29.0, 4);
            let world_land = bicubic(&world.heightmap, wu, wv) > 0.0;
            out.biome_color.push(bilinear_biome_color(&world.biomes, wu.rem_euclid(ww as f32), wv.clamp(0.0, wh as f32 - 1.0)));
            out.biome_weight.push(if world_land { BIOME_COLOR_WEIGHT } else { 0.0 });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------------------------

fn blend(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

impl ZoomRegion {
    /// Number of river cells and lake cells.
    pub fn stats(&self) -> (usize, usize) {
        (
            self.river_width_m.iter().filter(|&&r| r > 0.0).count(),
            self.lake_depth_m.iter().filter(|&&l| l > 0.0).count(),
        )
    }

    /// Shaded relief colours (row-major RGB): climate colours, lakes, anti-aliased rivers.
    /// Shared by the PNG export and the explorer's zoom view.
    pub fn render_rgb(&self) -> Vec<[u8; 3]> {
        let (w, h) = (self.width, self.height);
        let at = |x: usize, y: usize| self.elevation_m[y.min(h - 1) * w + x.min(w - 1)];

        // River coverage: a disc per river cell, radius from channel width (>= ~1 px wide).
        let mut cov = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let width = self.river_width_m[y * w + x];
                if width <= 0.0 { continue; }
                let r = (width / self.cell_m * 0.5).max(0.55);
                let ri = r.ceil() as i64 + 1;
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        let (px, py) = (x as i64 + dx, y as i64 + dy);
                        if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 { continue; }
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        let c = (r + 0.5 - dist).clamp(0.0, 1.0);
                        let k = py as usize * w + px as usize;
                        if c > cov[k] { cov[k] = c; }
                    }
                }
            }
        }

        let light = {
            let l: (f32, f32, f32) = (-0.6, -0.6, 0.55);
            let n = (l.0 * l.0 + l.1 * l.1 + l.2 * l.2).sqrt();
            (l.0 / n, l.1 / n, l.2 / n)
        };
        let mut rgb = vec![[0u8; 3]; w * h];
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                let e = self.elevation_m[k];
                let color = if e <= 0.0 {
                    let t = (-e / 2000.0).clamp(0.0, 1.0).sqrt();
                    blend((70, 140, 200), (12, 35, 95), t)
                } else {
                    let dzdx = (at(x + 1, y) - at(x.saturating_sub(1), y)) / (2.0 * self.cell_m) * 2.0;
                    let dzdy = (at(x, y + 1) - at(x, y.saturating_sub(1))) / (2.0 * self.cell_m) * 2.0;
                    let nlen = (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
                    let ndl = (-dzdx * light.0 - dzdy * light.1 + light.2) / nlen;
                    let shade = (0.3 + 0.85 * (ndl * 0.5 + 0.5).powi(2)).clamp(0.25, 1.15);
                    let lut = sample_lut(self.temperature_c[k], self.moisture[k], e);
                    let base = blend(lut, self.biome_color[k], self.biome_weight[k]);
                    let mut c = (
                        (base.0 as f32 * shade).min(255.0) as u8,
                        (base.1 as f32 * shade).min(255.0) as u8,
                        (base.2 as f32 * shade).min(255.0) as u8,
                    );
                    if self.lake_depth_m[k] > 0.0 {
                        c = blend(c, (55, 105, 165), 0.75);
                    } else if cov[k] > 0.0 {
                        c = blend(c, (45, 105, 175), 0.85 * cov[k]);
                    }
                    c
                };
                rgb[k] = [color.0, color.1, color.2];
            }
        }
        rgb
    }

    /// Save the shaded relief render as a PNG.
    pub fn save_png(&self, path: &std::path::Path) -> Result<(), Box<dyn Error>> {
        let rgb = self.render_rgb();
        let img: RgbImage = ImageBuffer::from_fn(self.width as u32, self.height as u32, |x, y| {
            Rgb(rgb[y as usize * self.width + x as usize])
        });
        img.save(path)?;
        Ok(())
    }

    /// 16-bit greyscale heightmap, normalised to the region's min..max (printed by the caller).
    pub fn save_heightmap16(&self, path: &std::path::Path) -> Result<(f32, f32), Box<dyn Error>> {
        let lo = self.elevation_m.iter().cloned().fold(f32::MAX, f32::min);
        let hi = self.elevation_m.iter().cloned().fold(f32::MIN, f32::max);
        let span = (hi - lo).max(1.0);
        let img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::from_fn(self.width as u32, self.height as u32, |x, y| {
            let e = self.elevation_m[y as usize * self.width + x as usize];
            Luma([(((e - lo) / span) * 65535.0) as u16])
        });
        img.save(path)?;
        Ok((lo, hi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two overlapping regions generated separately must agree exactly on terrain where they
    /// overlap (chunks are a pure function of world position), and nearly always on rivers.
    #[test]
    fn overlapping_regions_are_seamless() {
        let world = crate::world::generate_world_with_style(128, 64, 11, crate::plates::WorldStyle::Earthlike);
        let (cx, cy) = pick_interesting_window(&world, 4);
        let base = ZoomParams { center_x: cx, center_y: cy, tiles: 4, cells_per_tile: 32, erosion_iterations: 10, seed: 3 };
        let a = generate_zoom(&world, &base);
        let b = generate_zoom(&world, &ZoomParams { center_x: cx + 2, ..base.clone() });
        let s = 32;
        // A covers tiles [cx-2, cx+2), B covers [cx, cx+4): overlap is A columns [2s, 4s), B [0, 2s).
        let (mut same_river, mut river_cells) = (0usize, 0usize);
        for y in 0..a.height {
            for x in 0..2 * s {
                let ka = y * a.width + x + 2 * s;
                let kb = y * b.width + x;
                assert_eq!(a.elevation_m[ka].to_bits(), b.elevation_m[kb].to_bits(), "terrain differs at ({x},{y})");
                let (ra, rb) = (a.river_width_m[ka] > 0.0, b.river_width_m[kb] > 0.0);
                if ra || rb {
                    river_cells += 1;
                    if ra == rb { same_river += 1; }
                }
            }
        }
        let agree = same_river as f32 / river_cells.max(1) as f32;
        println!("river agreement in overlap: {:.1}% of {} river cells", agree * 100.0, river_cells);
        assert!(agree > 0.9, "rivers should mostly agree across separately generated regions");
    }

    /// The drainage tree must reach every cell, contain no cycles, and conserve area.
    #[test]
    fn routing_drains_every_cell_to_an_outlet() {
        let (w, h) = (64, 48);
        let noise = Perlin::new(7);
        // Bumpy terrain with closed basins and a small sea in one corner.
        let hm: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64, (i / w) as f64);
                let e = 200.0 * noise.get([x * 0.15, y * 0.15]) as f32 + 0.5 * (x + y) as f32;
                if x < 6.0 && y < 6.0 { -50.0 } else { e.max(1.0) }
            })
            .collect();
        let d = route(&hm, w, h, 0.0);
        assert_eq!(d.order.len(), w * h, "every cell is visited exactly once");
        let acc = accumulate(&d, &vec![0.0; w * h]);
        let outlet_total: f32 = (0..w * h).filter(|&i| d.recv[i] == NONE).map(|i| acc[i]).sum();
        assert!((outlet_total - (w * h) as f32).abs() < 0.5, "all area reaches an outlet");
        // Following receivers always terminates (no cycles).
        for start in 0..w * h {
            let (mut i, mut steps) = (start, 0);
            while d.recv[i] != NONE {
                i = d.recv[i] as usize;
                steps += 1;
                assert!(steps <= w * h, "receiver cycle from {start}");
            }
        }
    }
}
