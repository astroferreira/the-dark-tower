//! Landscape evolution at world scale: tectonic uplift and river erosion acting together.
//!
//! Each step (Braun & Willett 2013 style, on the world grid):
//! * **Routing**: a priority flood from the sea (enclosed seas count) gives every cell a receiver and a
//!   base-to-head order. Water in a closed hollow fills it to its spill level (a lake) and leaves
//!   through the outlet, so every river reaches the sea.
//! * **Uplift** of land where plates converge (from the tectonic `stress_map`).
//! * **Stream-power incision** `dh/dt = U - K Q^m S`, solved implicitly (stable at any step).
//!   Q is the real discharge: the climate's annual precipitation summed downstream.
//! * **Hillslopes**: linear diffusion on land (sub-grid slopes and landslides), with the sea
//!   as base level. It runs before the rivers each step so they re-grade what it fills.
//! * **Sediment**: what rivers erode is carried downstream. It settles in lakes (deltas build
//!   in from the inflows until the hollow is full and drains) and at river mouths, where it
//!   builds a shelf below sea level. What the shelf cannot hold goes to the deep sea.
//! * **Flexural isostasy**: removing rock lets the lithosphere rebound (rho_crust/rho_mantle of
//!   the load, spread over the flexural wavelength); sediment loads push it down.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::tilemap::Tilemap;

/// Earth's equatorial circumference (m); one map width spans it.
const CIRCUMFERENCE_M: f32 = 40_075_000.0;
/// Rebound per metre of rock removed (rho_crust / rho_mantle) and subsidence per metre of
/// sediment laid down under water ((rho_sed - rho_water) / (rho_mantle - rho_water)).
const REBOUND: f32 = 2700.0 / 3300.0;
const SEDIMENT_SUBSIDENCE: f32 = (2400.0 - 1030.0) / (3300.0 - 1030.0);
/// Water deeper than this is not treated as a lake cell (avoids float noise at spill level).
const LAKE_EPS_M: f32 = 0.05;

/// Physical width (km) of a tile on a world map `width` tiles wide (~78 km at 512).
pub fn tile_km(width: usize) -> f32 {
    CIRCUMFERENCE_M / 1000.0 / width.max(1) as f32
}

#[derive(Clone, Debug)]
pub struct LandscapeParams {
    /// Simulated span (years) and number of implicit steps.
    pub duration_yr: f32,
    pub steps: usize,
    /// Erodibility K for `K Q^m S` with Q in m^3/yr (effective value for ~80 km cells).
    pub k_fluvial: f32,
    /// Discharge exponent m (slope exponent n = 1).
    pub m: f32,
    /// Share of precipitation that becomes runoff.
    pub runoff: f32,
    /// Peak tectonic uplift rate (m/yr) and the stress range it ramps over.
    pub uplift_m_per_yr: f32,
    pub uplift_stress: (f32, f32),
    /// Effective hillslope diffusivity (m^2/yr).
    pub diffusion_m2_per_yr: f32,
    /// Flexural length scale (km): the width over which a load change is spread.
    pub flexure_km: f32,
    /// Shelf built at river mouths: depth at the mouth (m, positive down), extra depth per cell
    /// away from it, and how many cells out it reaches.
    pub shelf_depth_m: f32,
    pub shelf_drop_m_per_cell: f32,
    pub shelf_reach_cells: usize,
}

impl Default for LandscapeParams {
    fn default() -> Self {
        Self {
            duration_yr: 10.0e6,
            steps: 40,
            k_fluvial: 5.0e-7,
            m: 0.5,
            runoff: 0.5,
            uplift_m_per_yr: 2.5e-4,
            uplift_stress: (0.15, 0.6),
            diffusion_m2_per_yr: 2.0e3,
            flexure_km: 150.0,
            shelf_depth_m: 40.0,
            shelf_drop_m_per_cell: 60.0,
            shelf_reach_cells: 10,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LandscapeReport {
    /// Volumes in km^3.
    pub uplifted_km3: f64,
    pub eroded_km3: f64,
    pub into_lakes_km3: f64,
    pub onto_shelves_km3: f64,
    pub to_deep_sea_km3: f64,
    /// Land cells still holding a lake (below their spill level) at the end.
    pub lake_cells: usize,
}

/// Drainage of one landscape state: base-to-head order, receivers and water levels.
struct Routing {
    /// Cell indices, every receiver before its donors (sea first).
    stack: Vec<usize>,
    /// Receiver of each cell (ocean cells are their own receiver).
    receiver: Vec<usize>,
    /// Water surface: spill level of the hollow a cell sits in (= its height when it drains).
    level: Vec<f32>,
    ocean: Vec<bool>,
}

struct Grid {
    w: usize,
    h: usize,
    dx: f32,
}

impl Grid {
    fn neighbors(&self, i: usize, out: &mut Vec<(usize, f32)>) {
        out.clear();
        let (x, y) = ((i % self.w) as i64, (i / self.w) as i64);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let ny = y + dy;
                if ny < 0 || ny >= self.h as i64 { continue; }
                let nx = (x + dx).rem_euclid(self.w as i64);
                let d = if dx != 0 && dy != 0 { self.dx * std::f32::consts::SQRT_2 } else { self.dx };
                out.push((ny as usize * self.w + nx as usize, d));
            }
        }
    }
}

fn key(e: f32) -> Reverse<u64> {
    Reverse(((e as f64 + 20_000.0) * 1000.0) as u64)
}

/// Sea cells: connected bodies of cells at or below sea level that are big enough to be seas
/// (smaller ones are hollows on land). The map edges need not be reached.
fn open_ocean(g: &Grid, h: &[f32]) -> Vec<bool> {
    let min_cells = ((g.w * g.h) as f32 / 2600.0).ceil() as usize; // 50 cells at 512x256
    let mut ocean = vec![false; h.len()];
    let mut seen = vec![false; h.len()];
    let mut nb = Vec::with_capacity(8);
    let mut comp = Vec::new();
    for s in 0..h.len() {
        if seen[s] || h[s] > 0.0 { continue; }
        comp.clear();
        seen[s] = true;
        comp.push(s);
        let mut k = 0;
        while k < comp.len() {
            g.neighbors(comp[k], &mut nb);
            for &(n, _) in &nb {
                if !seen[n] && h[n] <= 0.0 {
                    seen[n] = true;
                    comp.push(n);
                }
            }
            k += 1;
        }
        if comp.len() >= min_cells {
            for &i in &comp { ocean[i] = true; }
        }
    }
    ocean
}

fn route(g: &Grid, h: &[f32]) -> Routing {
    let n = h.len();
    let ocean = open_ocean(g, h);
    let mut level = h.to_vec();
    let mut parent = vec![usize::MAX; n];
    let mut done = vec![false; n];
    let mut stack = Vec::with_capacity(n);
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if ocean[i] {
            done[i] = true;
            parent[i] = i;
            level[i] = 0.0;
            heap.push((key(0.0), i));
        }
    }
    let mut nb = Vec::with_capacity(8);
    while let Some((_, i)) = heap.pop() {
        stack.push(i);
        g.neighbors(i, &mut nb);
        for &(j, _) in &nb {
            if done[j] { continue; }
            done[j] = true;
            parent[j] = i;
            level[j] = h[j].max(level[i]);
            heap.push((key(level[j]), j));
        }
    }
    // Cells cut off from the ocean entirely (only if there is no open ocean): drain to themselves.
    for i in 0..n {
        if !done[i] {
            parent[i] = i;
            stack.push(i);
        }
    }

    // Draining cells flow down the steepest water-surface slope (any strictly lower neighbour
    // was popped earlier, so the order stays valid); lake and flat cells keep their flood parent.
    let mut receiver = parent;
    for i in 0..n {
        if ocean[i] || level[i] > h[i] + LAKE_EPS_M { continue; }
        g.neighbors(i, &mut nb);
        let mut best = 0.0f32;
        for &(j, d) in &nb {
            let s = (level[i] - level[j]) / d;
            if s > best {
                best = s;
                receiver[i] = j;
            }
        }
    }
    Routing { stack, receiver, level, ocean }
}

/// Separable Gaussian blur (wrapping in x, clamped in y) with standard deviation `sigma` cells.
fn gaussian(g: &Grid, v: &[f32], sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil().max(1.0) as i64;
    let k: Vec<f32> = (-r..=r).map(|d| (-(d * d) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let norm: f32 = k.iter().sum();
    let mut tmp = vec![0.0f32; v.len()];
    for y in 0..g.h {
        for x in 0..g.w {
            let mut s = 0.0;
            for (o, kw) in (-r..=r).zip(&k) {
                let nx = (x as i64 + o).rem_euclid(g.w as i64) as usize;
                s += kw * v[y * g.w + nx];
            }
            tmp[y * g.w + x] = s / norm;
        }
    }
    let mut out = vec![0.0f32; v.len()];
    for y in 0..g.h {
        for x in 0..g.w {
            let mut s = 0.0;
            for (o, kw) in (-r..=r).zip(&k) {
                let ny = (y as i64 + o).clamp(0, g.h as i64 - 1) as usize;
                s += kw * tmp[ny * g.w + x];
            }
            out[y * g.w + x] = s / norm;
        }
    }
    out
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Evolve `heightmap` (m, sea level 0) under uplift, river erosion, hillslope creep, sediment
/// deposition and flexural isostasy. `precipitation_mm` is annual precipitation (mm/yr).
pub fn evolve(
    heightmap: &mut Tilemap<f32>,
    precipitation_mm: &Tilemap<f32>,
    stress: &Tilemap<f32>,
    p: &LandscapeParams,
) -> LandscapeReport {
    let (w, hgt) = (heightmap.width, heightmap.height);
    let g = Grid { w, h: hgt, dx: CIRCUMFERENCE_M / w as f32 };
    let area = g.dx * g.dx;
    let n = w * hgt;
    let mut h: Vec<f32> = heightmap.iter().map(|(_, _, &e)| e).collect();
    let runoff: Vec<f32> = precipitation_mm.iter().map(|(_, _, &mm)| mm.max(0.0) * 1e-3 * p.runoff * area).collect();
    let uplift: Vec<f32> = stress
        .iter()
        .map(|(_, _, &s)| p.uplift_m_per_yr * smoothstep(p.uplift_stress.0, p.uplift_stress.1, s))
        .collect();
    let dt = p.duration_yr / p.steps.max(1) as f32;
    let sigma = (p.flexure_km * 1000.0 / g.dx).max(0.5);
    let km3 = |m: f64| m * area as f64 * 1e-9;

    let mut rep = LandscapeReport::default();
    let mut nb = Vec::with_capacity(8);
    let mut q = vec![0.0f32; n];
    let mut sed = vec![0.0f32; n];
    let mut eroded = vec![0.0f32; n];
    let mut deposited = vec![0.0f32; n];
    let mut load = vec![0.0f32; n];
    for _ in 0..p.steps {
        if load.iter().any(|&l| l != 0.0) {
            let response = gaussian(&g, &load, sigma);
            for i in 0..n { h[i] += response[i]; }
        }
        // Hillslope creep (explicit, sub-stepped for stability). The sea is the base level: land
        // creeps into it (that material is lost offshore) but the sea floor is left alone. Creep
        // runs before the rivers so they re-grade whatever it fills in.
        let kd = p.diffusion_m2_per_yr * dt / (g.dx * g.dx);
        let subs = (kd / 0.1).ceil().max(1.0) as usize;
        let kd = kd / subs as f32;
        let ocean_now = open_ocean(&g, &h);
        for _ in 0..subs {
            let prev = h.clone();
            for i in 0..n {
                if ocean_now[i] { continue; }
                g.neighbors(i, &mut nb);
                let mut flux = 0.0;
                for &(j, d) in &nb {
                    let wgt = if d > g.dx { 0.5 } else { 1.0 };
                    let hj = if ocean_now[j] { prev[j].max(0.0).min(prev[i]) } else { prev[j] };
                    flux += wgt * (hj - prev[i]);
                }
                h[i] += kd * flux / 6.0;
            }
        }

        let r = route(&g, &h);
        let lake: Vec<bool> = (0..n).map(|i| !r.ocean[i] && r.level[i] > h[i] + LAKE_EPS_M).collect();

        // Discharge, head to base.
        q.copy_from_slice(&runoff);
        for &i in r.stack.iter().rev() {
            let rc = r.receiver[i];
            if rc != i { q[rc] += q[i]; }
        }

        // Uplift, then implicit stream-power incision, base to head. A river entering a lake
        // is graded to the lake surface, not the lake floor.
        eroded.fill(0.0);
        for &i in &r.stack {
            if r.ocean[i] { continue; }
            let up = uplift[i] * dt;
            h[i] += up;
            rep.uplifted_km3 += km3(up as f64);
            if lake[i] { continue; }
            let rc = r.receiver[i];
            if rc == i { continue; }
            let base = if r.ocean[rc] { 0.0 } else { h[rc].max(r.level[rc]) };
            if h[i] <= base { continue; }
            let d = {
                let (dxc, dyc) = ((i % w) as i64 - (rc % w) as i64, (i / w) as i64 - (rc / w) as i64);
                if dxc != 0 && dyc != 0 { g.dx * std::f32::consts::SQRT_2 } else { g.dx }
            };
            let f = p.k_fluvial * dt * q[i].powf(p.m) / d;
            let new = (h[i] + f * base) / (1.0 + f);
            eroded[i] = h[i] - new;
            h[i] = new;
        }

        // Sediment: carried down the receivers, settling in lakes (up to the spill level) and,
        // where a river meets the open ocean, on a shelf around its mouth.
        deposited.fill(0.0);
        for i in 0..n { sed[i] = eroded[i]; }
        rep.eroded_km3 += km3(eroded.iter().map(|&e| e as f64).sum());
        let mut mouths: Vec<(usize, f32)> = Vec::new();
        for &i in r.stack.iter().rev() {
            if r.ocean[i] { continue; }
            let room = r.level[i] - LAKE_EPS_M - h[i];
            if lake[i] && room > 0.0 && sed[i] > 0.0 {
                let d = sed[i].min(room);
                h[i] += d;
                deposited[i] += d;
                sed[i] -= d;
                rep.into_lakes_km3 += km3(d as f64);
            }
            let rc = r.receiver[i];
            if rc == i { continue; }
            if r.ocean[rc] {
                if sed[i] > 0.0 { mouths.push((rc, sed[i])); }
            } else {
                sed[rc] += sed[i];
            }
        }
        mouths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let mut seen = vec![u32::MAX; n];
        for (k, &(mouth, load)) in mouths.iter().enumerate() {
            let mut left = load;
            let mut bfs = VecDeque::new();
            bfs.push_back((mouth, 0usize));
            seen[mouth] = k as u32;
            while let Some((i, dist)) = bfs.pop_front() {
                if left <= 0.0 { break; }
                let target = -p.shelf_depth_m - p.shelf_drop_m_per_cell * dist as f32;
                if h[i] < target {
                    let d = (target - h[i]).min(left);
                    h[i] += d;
                    deposited[i] += d;
                    left -= d;
                    rep.onto_shelves_km3 += km3(d as f64);
                }
                if dist + 1 > p.shelf_reach_cells { continue; }
                g.neighbors(i, &mut nb);
                for &(j, _) in &nb {
                    if r.ocean[j] && seen[j] != k as u32 {
                        seen[j] = k as u32;
                        bfs.push_back((j, dist + 1));
                    }
                }
            }
            rep.to_deep_sea_km3 += km3(left.max(0.0) as f64);
        }

        // Flexural isostasy: rebound under erosion, subsidence under sediment. It takes effect at
        // the start of the next step, so the rivers grade to it and the final surface is theirs.
        for i in 0..n {
            load[i] = REBOUND * eroded[i] - SEDIMENT_SUBSIDENCE * deposited[i];
        }
    }
    let r = route(&g, &h);
    rep.lake_cells = (0..n).filter(|&i| !r.ocean[i] && r.level[i] > h[i] + 1.0).count();

    for (i, (_, _, e)) in heightmap.iter_mut().enumerate() {
        *e = h[i];
    }
    rep
}

/// Depth of standing water each land cell would hold if every closed hollow filled to its
/// spill level (0 where the cell drains), using the same sea definition as `evolve`.
pub fn lake_depth(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    let (w, h) = (heightmap.width, heightmap.height);
    let g = Grid { w, h, dx: CIRCUMFERENCE_M / w as f32 };
    let e: Vec<f32> = heightmap.iter().map(|(_, _, &v)| v).collect();
    let r = route(&g, &e);
    let d = (0..e.len()).map(|i| if r.ocean[i] { 0.0 } else { (r.level[i] - e[i]).max(0.0) }).collect();
    Tilemap::from_vec(w, h, d)
}

/// Make the land drain: fill the closed hollows that would read as pits rather than lakes
/// (fewer than `min_lake_cells` cells, or shallower than `min_lake_depth_m` at their deepest)
/// and tilt flats, so every cell outside a lake has a strictly lower neighbour on its way to the
/// sea (a fall of `FILL_GRADIENT_M` per cell). Larger hollows stay as lake basins: water fills
/// them to the spill level and the river flows on. Returns the number of cells raised.
///
/// Detail passes that run after the landscape evolution (fjords, regional noise, volcanoes)
/// pock the land with small pits, and depression filling leaves flats; both end rivers.
pub fn fill_pits(heightmap: &mut Tilemap<f32>, min_lake_cells: usize, min_lake_depth_m: f32) -> usize {
    const FILL_GRADIENT_M: f32 = 0.05;
    let (w, hgt) = (heightmap.width, heightmap.height);
    let g = Grid { w, h: hgt, dx: CIRCUMFERENCE_M / w as f32 };
    let e: Vec<f32> = heightmap.iter().map(|(_, _, &v)| v).collect();
    let n = e.len();
    let r = route(&g, &e);
    let wet: Vec<bool> = (0..n).map(|i| !r.ocean[i] && r.level[i] > e[i] + LAKE_EPS_M).collect();

    // Level with a slight gradient: flood again from the sea, stepping up FILL_GRADIENT_M per cell.
    let mut graded = e.clone();
    let mut done = r.ocean.clone();
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if r.ocean[i] { graded[i] = 0.0; heap.push((key(0.0), i)); }
    }
    let mut nb = Vec::with_capacity(8);
    while let Some((_, i)) = heap.pop() {
        g.neighbors(i, &mut nb);
        for &(j, _) in &nb {
            if done[j] { continue; }
            done[j] = true;
            graded[j] = e[j].max(graded[i] + FILL_GRADIENT_M);
            heap.push((key(graded[j]), j));
        }
    }

    // Hollows = connected wet cells; the pits among them are filled, the rest kept as lakes.
    let mut keep = vec![false; n];
    let mut seen = vec![false; n];
    let mut comp = Vec::new();
    let mut raised = 0;
    for s in 0..n {
        if !wet[s] || seen[s] { continue; }
        comp.clear();
        seen[s] = true;
        comp.push(s);
        let mut k = 0;
        while k < comp.len() {
            g.neighbors(comp[k], &mut nb);
            for &(j, _) in &nb {
                if wet[j] && !seen[j] {
                    seen[j] = true;
                    comp.push(j);
                }
            }
            k += 1;
        }
        let depth = comp.iter().map(|&i| r.level[i] - e[i]).fold(0.0f32, f32::max);
        if comp.len() >= min_lake_cells && depth >= min_lake_depth_m {
            for &i in &comp { keep[i] = true; }
        }
    }
    for i in 0..n {
        if r.ocean[i] || keep[i] || graded[i] <= e[i] { continue; }
        heightmap.set(i % w, i / w, graded[i]);
        raised += 1;
    }
    raised
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A round island (peak ~2 km) in a 64x32 ocean, with a 1-cell pit and a deep 4x4 hollow.
    fn island() -> Tilemap<f32> {
        let (w, h) = (64, 32);
        let mut m = Tilemap::new_with(w, h, -3000.0f32);
        for y in 0..h {
            for x in 0..w {
                let d = (((x as f32 - 32.0) / 18.0).powi(2) + ((y as f32 - 16.0) / 10.0).powi(2)).sqrt();
                if d < 1.0 { m.set(x, y, 2000.0 * (1.0 - d) + 50.0); }
            }
        }
        let pit = *m.get(24, 16) - 300.0;
        m.set(24, 16, pit);
        for y in 14..18 {
            for x in 38..42 {
                let v = *m.get(x, y) - 400.0;
                m.set(x, y, v);
            }
        }
        m
    }

    #[test]
    fn fill_pits_keeps_lakes_and_drains_the_rest() {
        let mut m = island();
        let hollow = |d: &Tilemap<f32>| (38..42).flat_map(|x| (14..18).map(move |y| (x, y))).map(|(x, y)| *d.get(x, y)).fold(0.0f32, f32::max);
        let before = lake_depth(&m);
        assert!(*before.get(24, 16) > 10.0 && hollow(&before) > 100.0);
        fill_pits(&mut m, 4, 10.0);
        let depth = lake_depth(&m);
        assert!(*depth.get(24, 16) < 0.01, "1-cell pit should be filled");
        assert!(hollow(&depth) > 100.0, "deep 4x4 hollow should stay a lake");
        // Every land cell outside the lake has a strictly lower neighbour.
        for (x, y, &e) in m.iter() {
            if e <= 0.0 || *depth.get(x, y) > 0.0 { continue; }
            assert!(m.neighbors_8(x, y).iter().any(|&(nx, ny)| *m.get(nx, ny) < e), "flat or pit at ({x},{y})");
        }
    }

    #[test]
    fn evolve_conserves_sediment() {
        let mut m = island();
        let precip = Tilemap::new_with(64, 32, 1000.0f32);
        let mut stress = Tilemap::new_with(64, 32, 0.0f32);
        for y in 12..20 { for x in 28..36 { stress.set(x, y, 0.8); } }
        let r = evolve(&mut m, &precip, &stress, &LandscapeParams { steps: 10, ..Default::default() });
        assert!(r.eroded_km3 > 0.0 && r.uplifted_km3 > 0.0);
        let settled = r.into_lakes_km3 + r.onto_shelves_km3 + r.to_deep_sea_km3;
        assert!((settled - r.eroded_km3).abs() < 1e-3 * r.eroded_km3, "eroded {} vs settled {}", r.eroded_km3, settled);
        assert!(m.iter().all(|(_, _, e)| e.is_finite()));
    }
}
