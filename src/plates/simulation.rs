//! Time-stepped tectonic simulation on a sphere.
//!
//! Every plate is a rigid body rotating about its own Euler pole. Crust properties
//! (thickness, age, volcanic edifices) live in each plate's *body frame*, so moving a plate
//! never resamples (and therefore never blurs) its crust. Each step the map cells ask which
//! plates currently cover them:
//!
//! * **overlap**  -> convergence. Oceanic crust subducts (older / denser goes under) and builds
//!   a volcanic arc on the overriding plate; continent-continent contact collides, removing the
//!   loser's crust and thickening the winner (orogeny).
//! * **gap**      -> divergence. New oceanic crust (age 0) forms, so ridges and continental
//!   rifts emerge from plate motion rather than from noise.
//!
//! Plate speeds respond to slab pull (faster) and continental collision (locking), mantle
//! hotspots are fixed in the global frame so drifting plates leave volcanic chains, and
//! mountains slowly relax by erosion.

use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

use noise::{NoiseFn, Perlin};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use smallvec::SmallVec;

use crate::tilemap::Tilemap;

use super::stress::smooth_stress;
use super::types::{Plate, PlateId, PlateType, Vec2};

type Vec3 = [f64; 3];
type Mat3 = [[f64; 3]; 3];

pub const EARTH_RADIUS_KM: f64 = 6371.0;
/// Normal oceanic crust thickness (km).
pub const OCEANIC_CRUST_KM: f32 = 7.0;
/// Equilibrium continental crust thickness (km); mountain roots relax toward this.
pub const CONTINENTAL_CRUST_KM: f32 = 34.0;
/// Crust thicker than this behaves as buoyant continental crust.
pub const CONTINENTAL_THRESHOLD_KM: f32 = 20.0;
const MAX_CRUST_KM: f32 = 75.0;
const MAX_VOLCANIC_M: f32 = 6500.0;

const NONE_U8: u8 = 255;
/// Closing distance (in grid cells per step) below which an overlap is treated as sampling
/// noise rather than real convergence.
const REAL_CLOSING_CELLS: f64 = 0.6;
/// Per-step decay of boundary stress/trench memory. The memory lives in fixed map
/// coordinates while boundaries move, so it must be short or every past boundary position
/// leaves its own parallel trench line.
const STRESS_DECAY: f32 = 0.5;
const STRESS_OUTPUT_GAIN: f32 = 0.45;

// Process rates
const ARC_THICKNESS_PER_HIT_KM: f32 = 3.0;
const ARC_VOLCANIC_PER_HIT_M: f32 = 350.0;
const COLLISION_TRANSFER: f32 = 0.6;
const OROGEN_RELAX_MYR: f32 = 140.0;
const VOLCANIC_DECAY_MYR: f32 = 25.0;
/// Fraction of the continental-to-oceanic thickness gap removed per step on rift flanks.
const RIFT_THINNING: f32 = 0.06;
const HOTSPOT_VOLCANIC_M_PER_MYR: f32 = 10_000.0;

#[derive(Clone, Debug)]
pub struct TectonicParams {
    /// Total simulated time in millions of years.
    pub total_myr: f32,
    /// Number of time steps (each plate moves ~1-3 cells per step at default settings).
    pub steps: usize,
    /// Number of fixed mantle hotspots.
    pub hotspots: usize,
    /// Desired emerged land fraction. Continental shelves are widened at the start of the run
    /// until the crust could support it; lower targets are met by flooding instead.
    pub target_land_fraction: f32,
}

impl Default for TectonicParams {
    fn default() -> Self {
        Self { total_myr: 200.0, steps: 60, hotspots: 8, target_land_fraction: 0.35 }
    }
}

// ---------------------------------------------------------------------------------------------
// Small vector helpers
// ---------------------------------------------------------------------------------------------

#[inline]
fn dot(a: Vec3, b: Vec3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
#[inline]
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline]
fn scale3(a: Vec3, s: f64) -> Vec3 { [a[0] * s, a[1] * s, a[2] * s] }
#[inline]
fn sub3(a: Vec3, b: Vec3) -> Vec3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
#[inline]
fn norm3(a: Vec3) -> f64 { dot(a, a).sqrt() }
#[inline]
fn mul(m: &Mat3, v: Vec3) -> Vec3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}
fn transpose(m: &Mat3) -> Mat3 {
    [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]]
}
/// Rodrigues rotation matrix about a unit axis.
fn rotation(axis: Vec3, angle: f64) -> Mat3 {
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    let [x, y, z] = axis;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

/// Distance (in units of one grid row) from every cell to the nearest seed, measured along
/// the sphere's surface so it stays round at high latitudes.
fn sphere_distance(grid: &Grid, seeds: &[bool]) -> Vec<f32> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let cell = PI / grid.h as f64;
    let mut dist = vec![f32::MAX; seeds.len()];
    let mut heap = BinaryHeap::new();
    for (i, &s) in seeds.iter().enumerate() {
        if s { dist[i] = 0.0; heap.push(Reverse((0u32, i))); }
    }
    while let Some(Reverse((dk, i))) = heap.pop() {
        let d = dk as f32 / 64.0;
        if d > dist[i] + 0.02 { continue; }
        for j in grid.neighbors8(i) {
            let step = (norm3(sub3(grid.dirs[i], grid.dirs[j])) / cell) as f32;
            let nd = d + step;
            if nd < dist[j] {
                dist[j] = nd;
                heap.push(Reverse(((nd * 64.0) as u32, j)));
            }
        }
    }
    dist
}

/// Keep only the largest connected region of every plate; smaller detached pieces are
/// reassigned to the plate that most borders them. Repeats until every plate is contiguous.
fn remove_plate_enclaves(grid: &Grid, labels: &mut [PlateId]) {
    let n = labels.len();
    for _ in 0..8 {
        let mut comp = vec![usize::MAX; n];
        let mut comps: Vec<(PlateId, Vec<usize>)> = Vec::new();
        for start in 0..n {
            if comp[start] != usize::MAX { continue; }
            let id = labels[start];
            let c = comps.len();
            let mut cells = vec![start];
            comp[start] = c;
            let mut k = 0;
            while k < cells.len() {
                let i = cells[k];
                k += 1;
                for j in grid.neighbors8(i) {
                    if comp[j] == usize::MAX && labels[j] == id {
                        comp[j] = c;
                        cells.push(j);
                    }
                }
            }
            comps.push((id, cells));
        }
        // Largest component per plate id.
        let mut largest: std::collections::HashMap<PlateId, usize> = std::collections::HashMap::new();
        for (c, (id, cells)) in comps.iter().enumerate() {
            let e = largest.entry(*id).or_insert(c);
            if comps[*e].1.len() < cells.len() { *e = c; }
        }
        let mut changed = false;
        for (c, (id, cells)) in comps.iter().enumerate() {
            if largest[id] == c { continue; }
            let mut votes: std::collections::HashMap<PlateId, usize> = std::collections::HashMap::new();
            for &i in cells {
                for j in grid.neighbors8(i) {
                    if comp[j] != c { *votes.entry(labels[j]).or_insert(0) += 1; }
                }
            }
            if let Some((&best, _)) = votes.iter().max_by_key(|(id, v)| (**v, id.0)) {
                for &i in cells { labels[i] = best; }
                changed = true;
            }
        }
        if !changed { break; }
    }
}

/// Label (plate id) of the nearest seed for every cell; `labels[i] == NONE_U8` marks non-seeds.
fn sphere_nearest_label(grid: &Grid, labels: &[u8]) -> Vec<u8> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let cell = PI / grid.h as f64;
    let mut dist = vec![f32::MAX; labels.len()];
    let mut out = labels.to_vec();
    let mut heap = BinaryHeap::new();
    for (i, &l) in labels.iter().enumerate() {
        if l != NONE_U8 { dist[i] = 0.0; heap.push(Reverse((0u32, i))); }
    }
    while let Some(Reverse((dk, i))) = heap.pop() {
        let d = dk as f32 / 64.0;
        if d > dist[i] + 0.02 { continue; }
        for j in grid.neighbors8(i) {
            let nd = d + (norm3(sub3(grid.dirs[i], grid.dirs[j])) / cell) as f32;
            if nd < dist[j] {
                dist[j] = nd;
                out[j] = out[i];
                heap.push(Reverse(((nd * 64.0) as u32, j)));
            }
        }
    }
    out
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm3(noise: &Perlin, v: Vec3, freq: f64, octaves: u32) -> f64 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, freq, 0.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([v[0] * f + 11.3, v[1] * f + 7.1, v[2] * f + 3.7]);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

/// Unit vector for the centre of grid cell (x, y). Row 0 is the north pole.
pub fn cell_dir(x: usize, y: usize, w: usize, h: usize) -> [f64; 3] {
    let lat = PI / 2.0 - (y as f64 + 0.5) / h as f64 * PI;
    let lon = (x as f64 + 0.5) / w as f64 * TAU;
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

// ---------------------------------------------------------------------------------------------
// Grid
// ---------------------------------------------------------------------------------------------

struct Grid {
    w: usize,
    h: usize,
    dirs: Vec<Vec3>,
}

impl Grid {
    fn new(w: usize, h: usize) -> Self {
        let mut dirs = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                dirs.push(cell_dir(x, y, w, h));
            }
        }
        Self { w, h, dirs }
    }

    /// Nearest grid cell to a unit vector.
    #[inline]
    fn locate(&self, v: Vec3) -> usize {
        let mut lon = v[1].atan2(v[0]);
        if lon < 0.0 { lon += TAU; }
        let lat = v[2].clamp(-1.0, 1.0).asin();
        let fx = lon / TAU * self.w as f64 - 0.5;
        let ix = (fx.round() as i64).rem_euclid(self.w as i64) as usize;
        let fy = (PI / 2.0 - lat) / PI * self.h as f64 - 0.5;
        let iy = (fy.round().max(0.0) as usize).min(self.h - 1);
        iy * self.w + ix
    }

    fn neighbors8(&self, i: usize) -> SmallVec<[usize; 8]> {
        let (x, y) = (i % self.w, i / self.w);
        let mut out = SmallVec::new();
        for dy in -1i32..=1 {
            let ny = y as i32 + dy;
            if ny < 0 || ny >= self.h as i32 { continue; }
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = (x as i32 + dx).rem_euclid(self.w as i32) as usize;
                out.push(ny as usize * self.w + nx);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Plate state
// ---------------------------------------------------------------------------------------------

/// Crust carried by one plate, indexed in the plate's own (unrotated) body frame.
struct Frame {
    owned: Vec<bool>,
    thick: Vec<f32>,
    age: Vec<f32>,
    volc: Vec<f32>,
    /// Body-frame centroid and cosine of the cap angle that bounds all owned cells.
    centroid: Vec3,
    cap_cos: f64,
    area: usize,
    /// Owned cells carrying continental-type crust; decides collision polarity.
    cont_area: usize,
}

impl Frame {
    fn empty(n: usize) -> Self {
        Self {
            owned: vec![false; n],
            thick: vec![OCEANIC_CRUST_KM; n],
            age: vec![0.0; n],
            volc: vec![0.0; n],
            centroid: [1.0, 0.0, 0.0],
            cap_cos: -1.0,
            area: 0,
            cont_area: 0,
        }
    }

    fn refresh_cap(&mut self, grid: &Grid) {
        let mut sum = [0.0; 3];
        let mut area = 0;
        let mut cont_area = 0;
        for (i, &o) in self.owned.iter().enumerate() {
            if o {
                let d = grid.dirs[i];
                sum[0] += d[0]; sum[1] += d[1]; sum[2] += d[2];
                area += 1;
                if self.thick[i] >= CONTINENTAL_THRESHOLD_KM { cont_area += 1; }
            }
        }
        self.area = area;
        self.cont_area = cont_area;
        if area == 0 { self.cap_cos = 2.0; return; }
        let len = norm3(sum);
        self.centroid = if len > 1e-9 { scale3(sum, 1.0 / len) } else { [1.0, 0.0, 0.0] };
        let mut min_dot: f64 = 1.0;
        for (i, &o) in self.owned.iter().enumerate() {
            if o { min_dot = min_dot.min(dot(grid.dirs[i], self.centroid)); }
        }
        // Slack so cells next to the boundary (splats, new crust) are never culled.
        let ang = min_dot.clamp(-1.0, 1.0).acos() + 0.05;
        self.cap_cos = if ang >= PI { -1.0 } else { ang.cos() };
    }
}

struct Kin {
    axis: Vec3,
    /// Angular rate (rad/Myr) of the unmodified plate.
    base_rate: f64,
    mult: f64,
    angle: f64,
    lock: f32,
    slab: f32,
}

impl Kin {
    fn omega(&self) -> Vec3 { scale3(self.axis, self.base_rate * self.mult) }
}

/// Fields extracted from the simulation, one value per map cell.
#[derive(Clone)]
pub struct CrustFields {
    pub thickness_km: Tilemap<f32>,
    pub age_myr: Tilemap<f32>,
    /// Un-compensated volcanic edifice height (m): hotspot chains and arc volcanoes.
    pub volcanic_m: Tilemap<f32>,
    /// Recent subduction intensity 0..1 (trench placement).
    pub trench: Tilemap<f32>,
}

pub struct TectonicResult {
    pub plate_map: Tilemap<PlateId>,
    pub plates: Vec<Plate>,
    pub stress_map: Tilemap<f32>,
    pub crust: CrustFields,
}

pub struct TectonicSim {
    grid: Grid,
    frames: Vec<Frame>,
    kin: Vec<Kin>,
    owner: Vec<u8>,
    cur_fi: Vec<u32>,
    stress_acc: Vec<f32>,
    trench_acc: Vec<f32>,
    hotspots: Vec<Vec3>,
    km_per_cell: f64,
    dt: f32,
    steps_total: usize,
    steps_done: usize,
    /// Normalised Gaussian deposit kernel (dx, dy, weight), wider on finer grids so successive
    /// steps' arc/collision deposits overlap into continuous belts.
    kernel: Vec<(i32, i32, f32)>,
}

impl TectonicSim {
    pub fn new(
        plate_map: &Tilemap<PlateId>,
        plates: &[Plate],
        rng: &mut ChaCha8Rng,
        params: &TectonicParams,
    ) -> Self {
        let (w, h) = (plate_map.width, plate_map.height);
        let n = w * h;
        let grid = Grid::new(w, h);
        let np = plates.len();
        let sc = (w as f32 / 512.0).max(0.25);

        let coast_noise = Perlin::new(rng.gen::<u32>());
        let craton_noise = Perlin::new(rng.gen::<u32>());
        let age_noise = Perlin::new(rng.gen::<u32>());
        let belt_noise = Perlin::new(rng.gen::<u32>());

        // Continents are the zero set of a noise-perturbed signed distance field: positive
        // inside the continental plates, negative outside, measured in cells on the sphere.
        // Fractal noise at continental and coastal scales turns the straight plate borders into
        // peninsulas, gulfs and ragged coasts, and a single offset (solved below) sets how much
        // area is continental crust.
        let in_cont: Vec<bool> = (0..n)
            .map(|i| {
                let id = *plate_map.get(i % w, i / w);
                !id.is_none() && (id.0 as usize) < np && plates[id.0 as usize].plate_type == PlateType::Continental
            })
            .collect();
        let outside: Vec<bool> = in_cont.iter().map(|&c| !c).collect();
        let d_in = sphere_distance(&grid, &outside);
        let d_out = sphere_distance(&grid, &in_cont);
        let big_noise = Perlin::new(rng.gen::<u32>());
        let small_noise = Perlin::new(rng.gen::<u32>());
        let signed: Vec<f32> = (0..n)
            .map(|i| {
                let s = if in_cont[i] { d_in[i].min(200.0) } else { -d_out[i].min(200.0) };
                let dir = grid.dirs[i];
                let wander = 24.0 * fbm3(&big_noise, dir, 1.7, 4) as f32 + 9.0 * fbm3(&small_noise, dir, 7.0, 4) as f32;
                // Keep continents from forming over the poles (the map edges must stay oceanic).
                let edge = ((i / w).min(h - 1 - i / w) as f32 + 0.5) / h as f32;
                let polar = 40.0 * sc * (1.0 - smoothstep(0.05, 0.16, edge));
                s + wander * sc - polar
            })
            .collect();

        // About 70-75% of continental crust ends up above sea level; the rest is drowned shelf or
        // polar crust that is flooded to keep the map edges oceanic.
        let needed = ((params.target_land_fraction / 0.72).min(0.9) * n as f32) as usize;
        let (mut lo, mut hi) = (-80.0 * sc, 80.0 * sc);
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            let count = signed.iter().filter(|&&v| v + mid >= 0.0).count();
            if count >= needed { hi = mid } else { lo = mid }
        }
        let offset = hi;
        let taper = 6.0 * sc;

        // Continental crust and its shelf belong to the nearest continental plate, so a
        // continent never straddles two plates and rifts open along coasts rather than along
        // the flood-fill's polygon borders. The plate boundary ends up offshore.
        let seed_labels: Vec<u8> = (0..n)
            .map(|i| if in_cont[i] { plate_map.get(i % w, i / w).0 } else { NONE_U8 })
            .collect();
        let nearest_cont = sphere_nearest_label(&grid, &seed_labels);
        let plate_raw: Vec<PlateId> = (0..n)
            .map(|i| {
                let orig = *plate_map.get(i % w, i / w);
                let on_continent = signed[i] + offset >= -taper;
                if on_continent && !in_cont[i] && nearest_cont[i] != NONE_U8 {
                    PlateId(nearest_cont[i])
                } else {
                    orig
                }
            })
            .collect();
        // Domain-warp the plate layout so boundaries (future rifts, sutures and trenches) are
        // wiggly at several scales instead of straight flood-fill / Voronoi edges.
        let warp_noise: [Perlin; 3] = [Perlin::new(rng.gen::<u32>()), Perlin::new(rng.gen::<u32>()), Perlin::new(rng.gen::<u32>())];
        let cell_rad = PI / h as f64;
        let plate_of: Vec<PlateId> = (0..n)
            .map(|i| {
                let d = grid.dirs[i];
                let mut q = d;
                for (a, wn) in warp_noise.iter().enumerate() {
                    let off = 5.0 * fbm3(wn, d, 2.0, 3) + 2.0 * fbm3(wn, d, 9.0, 3);
                    q[a] += off * 2.0 * cell_rad * sc as f64;
                }
                let l = norm3(q);
                plate_raw[grid.locate(scale3(q, 1.0 / l))]
            })
            .collect();
        // Warping can strand small detached pieces of one plate inside another; each would then
        // plough through its host as a tiny independent plate, leaving parallel trails of new
        // seafloor. Absorb every piece except each plate's main body into its surroundings.
        let mut plate_of = plate_of;
        remove_plate_enclaves(&grid, &mut plate_of);

        // Initial crust: continental where the field is positive, tapering to oceanic crust
        // across a passive margin; everything else is 7 km oceanic crust with a smooth random
        // age field.
        let mut frames: Vec<Frame> = (0..np).map(|_| Frame::empty(n)).collect();
        let mut owner = vec![NONE_U8; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let id = plate_of[i];
                if id.is_none() || id.0 as usize >= np { continue; }
                let k = id.0 as usize;
                let dir = grid.dirs[i];
                let f = &mut frames[k];
                f.owned[i] = true;
                owner[i] = id.0;
                let t = smoothstep(-taper, taper, signed[i] + offset);
                let craton = fbm3(&craton_noise, dir, 1.8, 3) as f32;
                // Inherited relief: shields/basins plus old fold belts along the zero
                // crossings of a noise field; both erode away over the run.
                let belt = smoothstep(0.10, 0.0, (fbm3(&belt_noise, dir, 2.2, 3) as f32).abs());
                f.thick[i] = OCEANIC_CRUST_KM
                    + (CONTINENTAL_CRUST_KM - OCEANIC_CRUST_KM + 2.0 * craton + 10.0 * belt) * t;
                let a = fbm3(&age_noise, dir, 1.3, 3) as f32;
                f.age[i] = (70.0 + 80.0 * a).clamp(3.0, 150.0);
            }
        }
        for f in frames.iter_mut() { f.refresh_cap(&grid); }

        // Euler poles. Oceanic plates move faster (slab pull); continental plates are sluggish.
        let kin: Vec<Kin> = plates
            .iter()
            .map(|p| {
                let axis = loop {
                    let v = [rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)];
                    let l = norm3(v);
                    if l > 0.1 && l <= 1.0 { break scale3(v, 1.0 / l); }
                };
                let cm_per_yr: f64 = match p.plate_type {
                    PlateType::Oceanic => rng.gen_range(2.0..7.0),
                    PlateType::Continental => rng.gen_range(0.8..3.5),
                };
                // 1 cm/yr == 10 km/Myr
                Kin { axis, base_rate: cm_per_yr * 10.0 / EARTH_RADIUS_KM, mult: 1.0, angle: 0.0, lock: 0.0, slab: 0.0 }
            })
            .collect();

        let hotspots = (0..params.hotspots)
            .map(|_| loop {
                let v = [rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)];
                let l = norm3(v);
                if l > 0.1 && l <= 1.0 { break scale3(v, 1.0 / l); }
            })
            .collect();

        // `steps` is calibrated for a 512-wide map; finer grids need proportionally more steps so
        // plates still move ~1-2 cells per step (otherwise each step's arc/collision deposit
        // lands as a separate stripe).
        let steps = (params.steps as f32 * sc.max(1.0)).round().max(1.0) as usize;
        Self {
            km_per_cell: PI * EARTH_RADIUS_KM / h as f64,
            grid,
            frames,
            kin,
            cur_fi: (0..n as u32).collect(),
            owner,
            stress_acc: vec![0.0; n],
            trench_acc: vec![0.0; n],
            hotspots,
            dt: params.total_myr / steps as f32,
            steps_total: steps,
            steps_done: 0,
            kernel: deposit_kernel(2.0 * sc.max(0.5)),
        }
    }

    pub fn steps_total(&self) -> usize { self.steps_total }
    pub fn steps_done(&self) -> usize { self.steps_done }

    pub fn run(&mut self) {
        while self.steps_done < self.steps_total {
            self.step();
        }
    }

    #[inline]
    fn score(&self, k: u8, fi: u32) -> f32 {
        let f = &self.frames[k as usize];
        let t = f.thick[fi as usize];
        if t >= CONTINENTAL_THRESHOLD_KM {
            // Continental collisions are decided at the plate scale: the plate carrying more
            // continental crust overrides, so an island arc riding on an oceanic plate is
            // accreted as a terrane instead of bulldozing a channel through the continent.
            1000.0 + f.cont_area as f32 + t * 0.001
        } else {
            // Younger, thicker oceanic crust is more buoyant and overrides older crust.
            50.0 - f.age[fi as usize].min(200.0) * 0.2 + t * 0.5
        }
    }

    /// Relative approach distance (grid cells) accumulated over one step between two plates at p.
    fn closing_cells(&self, a: usize, b: usize, p: Vec3) -> f64 {
        let dw = sub3(self.kin[a].omega(), self.kin[b].omega());
        norm3(cross(dw, p)) * EARTH_RADIUS_KM * self.dt as f64 / self.km_per_cell
    }

    pub fn step(&mut self) {
        let (w, h) = (self.grid.w, self.grid.h);
        let n = w * h;
        let np = self.frames.len();
        let dt = self.dt;

        // 1. Advance plate rotations.
        for k in self.kin.iter_mut() {
            k.angle += k.base_rate * k.mult * dt as f64;
        }
        let rot: Vec<Mat3> = self.kin.iter().map(|k| rotation(k.axis, k.angle)).collect();
        let rinv: Vec<Mat3> = rot.iter().map(transpose).collect();
        let world_centroid: Vec<Vec3> =
            (0..np).map(|k| mul(&rot[k], self.frames[k].centroid)).collect();

        // 2. Which plates cover each cell? (parallel; read-only)
        let cands: Vec<SmallVec<[(u8, u32); 3]>> = {
            let frames = &self.frames;
            let grid = &self.grid;
            (0..n)
                .into_par_iter()
                .map(|d| {
                    let p = grid.dirs[d];
                    let mut c: SmallVec<[(u8, u32); 3]> = SmallVec::new();
                    for k in 0..np {
                        let f = &frames[k];
                        if f.area == 0 || dot(p, world_centroid[k]) < f.cap_cos { continue; }
                        let fi = grid.locate(mul(&rinv[k], p));
                        if f.owned[fi] { c.push((k as u8, fi as u32)); }
                    }
                    c
                })
                .collect()
        };

        // 3. Resolve overlaps / gaps (serial).
        let mut new_owner = vec![NONE_U8; n];
        let mut new_fi = vec![0u32; n];
        let mut removals: Vec<(u8, u32)> = Vec::new();
        let mut collisions: Vec<(u8, u32, f32)> = Vec::new();
        let mut arcs: Vec<(u8, u32)> = Vec::new();
        let mut gaps: Vec<usize> = Vec::new();
        let mut stress_step = vec![0.0f32; n];
        let mut trench_step = vec![0.0f32; n];
        let mut sub_cnt = vec![0.0f32; np];
        let mut coll_cnt = vec![0.0f32; np];

        for d in 0..n {
            let c = &cands[d];
            match c.len() {
                0 => {
                    gaps.push(d);
                    stress_step[d] = -0.6;
                    continue;
                }
                1 => {
                    new_owner[d] = c[0].0;
                    new_fi[d] = c[0].1;
                    continue;
                }
                _ => {}
            }

            let p = self.grid.dirs[d];
            let mut best = 0;
            let mut best_s = f32::MIN;
            for (i, &(k, fi)) in c.iter().enumerate() {
                let s = self.score(k, fi);
                if s > best_s { best_s = s; best = i; }
            }
            let (bk, _) = c[best];
            let real = c.iter().enumerate().any(|(i, &(k, _))| {
                i != best && self.closing_cells(bk as usize, k as usize, p) >= REAL_CLOSING_CELLS
            });

            // Neutral overlap: keep whoever owned the cell last step (no crust is destroyed).
            let win = if real {
                c[best]
            } else {
                c.iter().copied().find(|&(k, _)| k == self.owner[d]).unwrap_or(c[best])
            };
            new_owner[d] = win.0;
            new_fi[d] = win.1;
            let win_thick = self.frames[win.0 as usize].thick[win.1 as usize];

            for &(k, fi) in c.iter() {
                if k == win.0 { continue; }
                let loser_thick = self.frames[k as usize].thick[fi as usize];
                let closing = self.closing_cells(win.0 as usize, k as usize, p);
                let both_cont = win_thick >= CONTINENTAL_THRESHOLD_KM && loser_thick >= CONTINENTAL_THRESHOLD_KM;
                if !real || closing < REAL_CLOSING_CELLS {
                    if both_cont {
                        // Locked continental contact: no crust moves, but it resists plate motion.
                        coll_cnt[win.0 as usize] += 0.5;
                        coll_cnt[k as usize] += 0.5;
                        stress_step[d] = stress_step[d].max(0.15);
                    }
                    continue;
                }
                let intensity = (closing / 3.0).min(1.0) as f32;
                removals.push((k, fi));
                if both_cont {
                    collisions.push((win.0, win.1, loser_thick * COLLISION_TRANSFER));
                    coll_cnt[win.0 as usize] += 1.0;
                    coll_cnt[k as usize] += 1.0;
                    stress_step[d] = stress_step[d].max(0.4 + 0.6 * intensity);
                } else {
                    arcs.push((win.0, win.1));
                    sub_cnt[k as usize] += 1.0;
                    stress_step[d] = stress_step[d].max(0.3 + 0.6 * intensity);
                    trench_step[d] = trench_step[d].max(intensity);
                }
            }
        }

        // 4. Apply crust removal, accretion and orogeny.
        for &(k, fi) in &removals {
            self.frames[k as usize].owned[fi as usize] = false;
        }
        for &(k, fi, add) in &collisions {
            splat(&mut self.frames[k as usize], &self.kernel, w, h, fi as usize, add, 0.0);
        }
        for &(k, fi) in &arcs {
            splat(&mut self.frames[k as usize], &self.kernel, w, h, fi as usize, ARC_THICKNESS_PER_HIT_KM, ARC_VOLCANIC_PER_HIT_M);
        }

        // 5. Divergence: fill gaps with fresh oceanic crust, layer by layer from the neighbours
        //    so both sides of a ridge grow symmetrically.
        let mut remaining = gaps;
        let mut filled: Vec<(usize, u8)> = Vec::new();
        while !remaining.is_empty() {
            let mut next = Vec::new();
            let mut newly: Vec<(usize, u8)> = Vec::new();
            for &d in &remaining {
                let mut votes: SmallVec<[(u8, u8); 4]> = SmallVec::new();
                for j in self.grid.neighbors8(d) {
                    let o = new_owner[j];
                    if o == NONE_U8 { continue; }
                    match votes.iter_mut().find(|v| v.0 == o) {
                        Some(v) => v.1 += 1,
                        None => votes.push((o, 1)),
                    }
                }
                match votes.iter().max_by_key(|v| v.1) {
                    Some(&(o, _)) => newly.push((d, o)),
                    None => next.push(d),
                }
            }
            if newly.is_empty() { break; }
            for &(d, o) in &newly {
                new_owner[d] = o;
                filled.push((d, o));
            }
            remaining = next;
        }
        // Rift flanks: continental crust next to freshly opened seafloor is stretched and thins,
        // so rifts widen into sloping margins instead of staying knife-cut channels.
        let mut flanks: Vec<(u8, u32)> = Vec::new();
        for &(d, _) in &filled {
            for j in self.grid.neighbors8(d) {
                let o = new_owner[j];
                if o != NONE_U8 && !filled.iter().any(|&(fd, _)| fd == j) {
                    flanks.push((o, new_fi[j]));
                }
            }
        }
        for &(k, fi) in &flanks {
            let t = &mut self.frames[k as usize].thick[fi as usize];
            if *t >= CONTINENTAL_THRESHOLD_KM {
                *t -= RIFT_THINNING * (*t - OCEANIC_CRUST_KM);
            }
        }
        for &(d, k) in &filled {
            let fi = self.grid.locate(mul(&rinv[k as usize], self.grid.dirs[d]));
            new_fi[d] = fi as u32;
            let nbrs = self.grid.neighbors8(fi);
            let f = &mut self.frames[k as usize];
            for idx in std::iter::once(fi).chain(nbrs.into_iter()) {
                if !f.owned[idx] {
                    f.owned[idx] = true;
                    f.thick[idx] = OCEANIC_CRUST_KM;
                    f.age[idx] = 0.0;
                    f.volc[idx] = 0.0;
                }
            }
        }

        // 6. Mantle hotspots (fixed in the global frame).
        for hs in &self.hotspots {
            let d = self.grid.locate(*hs);
            let k = new_owner[d];
            if k == NONE_U8 { continue; }
            let fi = new_fi[d] as usize;
            splat(&mut self.frames[k as usize], &self.kernel, w, h, fi, 0.0, HOTSPOT_VOLCANIC_M_PER_MYR * dt);
        }

        // 7. Per-plate ageing, erosion and relaxation.
        self.frames.par_iter_mut().for_each(|f| {
            let orogen = (-dt / OROGEN_RELAX_MYR).exp();
            let volc = (-dt / VOLCANIC_DECAY_MYR).exp();
            for i in 0..f.owned.len() {
                if !f.owned[i] { continue; }
                f.age[i] += dt;
                f.volc[i] = (f.volc[i] * volc).min(MAX_VOLCANIC_M);
                let t = f.thick[i];
                if t >= CONTINENTAL_THRESHOLD_KM && t > CONTINENTAL_CRUST_KM {
                    f.thick[i] = CONTINENTAL_CRUST_KM + (t - CONTINENTAL_CRUST_KM) * orogen;
                }
                f.thick[i] = f.thick[i].min(MAX_CRUST_KM);
            }
        });

        // 8. Feedback on plate speeds: slab pull accelerates, continental collision locks.
        for k in 0..np {
            let perimeter = 2.0 * (self.frames[k].area as f32).sqrt().max(1.0);
            let kin = &mut self.kin[k];
            kin.slab = 0.7 * kin.slab + 0.3 * sub_cnt[k] / perimeter;
            kin.lock = 0.7 * kin.lock + 0.3 * coll_cnt[k] / perimeter;
            let target = (1.0 + 0.8 * (4.0 * kin.slab).tanh()) * (1.0 - 0.92 * (4.0 * kin.lock).tanh());
            let target = (target as f64).clamp(0.05, 1.6);
            kin.mult += 0.3 * (target - kin.mult);
        }

        // 9. Accumulate boundary stress / trench memory and commit ownership.
        for d in 0..n {
            self.stress_acc[d] = self.stress_acc[d] * STRESS_DECAY + stress_step[d];
            self.trench_acc[d] = self.trench_acc[d] * STRESS_DECAY + trench_step[d];
        }
        self.owner = new_owner;
        self.cur_fi = new_fi;
        for f in self.frames.iter_mut() { f.refresh_cap(&self.grid); }
        self.steps_done += 1;
    }

    /// Per-cell crust properties at the current simulation time.
    pub fn crust_fields(&self) -> CrustFields {
        let (w, h) = (self.grid.w, self.grid.h);
        let n = w * h;
        let mut thick = vec![OCEANIC_CRUST_KM; n];
        let mut age = vec![100.0f32; n];
        let mut volc = vec![0.0f32; n];
        for d in 0..n {
            let k = self.owner[d];
            if k == NONE_U8 { continue; }
            let f = &self.frames[k as usize];
            let fi = self.cur_fi[d] as usize;
            thick[d] = f.thick[fi];
            // Seafloor older than ~180 Myr is recycled into the mantle on Earth.
            age[d] = f.age[fi].min(180.0);
            volc[d] = f.volc[fi];
        }
        let gain = 1.0 - STRESS_DECAY;
        let trench: Vec<f32> = self.trench_acc.iter().map(|t| (t * gain * 2.5).min(1.0)).collect();
        let trench = smooth_stress(&Tilemap::from_vec(w, h, trench), 3, 1.5);
        CrustFields {
            thickness_km: Tilemap::from_vec(w, h, thick),
            age_myr: Tilemap::from_vec(w, h, age),
            volcanic_m: Tilemap::from_vec(w, h, volc),
            trench,
        }
    }

    pub fn plate_map(&self) -> Tilemap<PlateId> {
        Tilemap::from_vec(self.grid.w, self.grid.h, self.owner.iter().map(|&o| PlateId(o)).collect())
    }

    /// Finish: compact surviving plates, classify them from their final crust and derive the
    /// legacy `stress_map` (convergent > 0, divergent < 0) used by downstream stages.
    pub fn finish(self, plates: Vec<Plate>) -> TectonicResult {
        let (w, h) = (self.grid.w, self.grid.h);
        let n = w * h;
        let crust = self.crust_fields();

        // Stress: remembered convergence/divergence plus standing orogens.
        let gain = 1.0 - STRESS_DECAY;
        let mut raw = vec![0.0f32; n];
        for d in 0..n {
            let mut s = self.stress_acc[d] * gain * 2.0;
            let t = crust.thickness_km.get(d % w, d / w);
            if *t > 42.0 {
                s += ((*t - 42.0) / 50.0).min(0.4);
            } else if *t < CONTINENTAL_THRESHOLD_KM {
                // Spreading ridges: young oceanic crust is under tension.
                s -= 0.35 * (-crust.age_myr.get(d % w, d / w) / 10.0).exp();
            }
            raw[d] = s;
        }
        // Downstream stages (volcanoes, cliffs, rock types) are tuned for stress in roughly
        // [-0.2, 0.5], with 0.15 meaning "volcanic" and 0.3 meaning "mountain building".
        let stress_map = smooth_stress(&Tilemap::from_vec(w, h, raw), 3, 1.5)
            .iter()
            .fold(Tilemap::new_with(w, h, 0.0f32), |mut m, (x, y, &v)| {
                m.set(x, y, (v * STRESS_OUTPUT_GAIN).clamp(-0.6, 0.6));
                m
            });

        // Compact plate ids and update plate type / velocity from the final state.
        let mut area = vec![0usize; plates.len()];
        let mut cont = vec![0usize; plates.len()];
        let mut centroid = vec![[0.0f64; 3]; plates.len()];
        for d in 0..n {
            let k = self.owner[d];
            if k == NONE_U8 { continue; }
            let k = k as usize;
            area[k] += 1;
            if crust.thickness_km.get(d % w, d / w) >= &CONTINENTAL_THRESHOLD_KM { cont[k] += 1; }
            for a in 0..3 { centroid[k][a] += self.grid.dirs[d][a]; }
        }
        let mut remap = vec![NONE_U8; plates.len()];
        let mut out_plates = Vec::new();
        for (k, mut p) in plates.into_iter().enumerate() {
            if area[k] == 0 { continue; }
            let id = PlateId(out_plates.len() as u8);
            remap[k] = id.0;
            p.id = id;
            p.plate_type = if cont[k] as f32 > 0.35 * area[k] as f32 { PlateType::Continental } else { PlateType::Oceanic };
            let c = centroid[k];
            let l = norm3(c);
            if l > 1e-9 {
                let c = scale3(c, 1.0 / l);
                let v = cross(self.kin[k].omega(), c);
                let (lat, lon) = (c[2].asin(), c[1].atan2(c[0]));
                let east = [-lon.sin(), lon.cos(), 0.0];
                let north = [-lat.sin() * lon.cos(), -lat.sin() * lon.sin(), lat.cos()];
                // km/Myr / 10 = cm/yr; legacy velocities are ~0.1-0.7.
                let s = EARTH_RADIUS_KM / 100.0;
                p.velocity = Vec2::new((dot(v, east) * s) as f32, (-dot(v, north) * s) as f32);
            }
            out_plates.push(p);
        }
        let plate_map = Tilemap::from_vec(
            w,
            h,
            self.owner.iter().map(|&o| if o == NONE_U8 { PlateId::NONE } else { PlateId(remap[o as usize]) }).collect(),
        );

        TectonicResult { plate_map, plates: out_plates, stress_map, crust }
    }
}

/// Normalised Gaussian kernel with standard deviation `sigma` cells (truncated at 2 sigma).
fn deposit_kernel(sigma: f32) -> Vec<(i32, i32, f32)> {
    let r = (2.0 * sigma).ceil() as i32;
    let mut k = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 <= (r * r) as f32 {
                k.push((dx, dy, (-d2 / (2.0 * sigma * sigma)).exp()));
            }
        }
    }
    let sum: f32 = k.iter().map(|e| e.2).sum();
    k.iter().map(|&(dx, dy, wgt)| (dx, dy, wgt / sum)).collect()
}

/// Add `thick_add` km and `volc_add` m (as conserved totals) around frame cell `fi`, spread by
/// `kernel`. Only owned cells receive material.
fn splat(f: &mut Frame, kernel: &[(i32, i32, f32)], w: usize, h: usize, fi: usize, thick_add: f32, volc_add: f32) {
    let (fx, fy) = ((fi % w) as i32, (fi / w) as i32);
    for &(dx, dy, wgt) in kernel {
        let y = fy + dy;
        if y < 0 || y >= h as i32 { continue; }
        let x = (fx + dx).rem_euclid(w as i32);
        let idx = y as usize * w + x as usize;
        if !f.owned[idx] { continue; }
        f.thick[idx] += thick_add * wgt;
        f.volc[idx] += volc_add * wgt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plates::{generate_plates, WorldStyle};
    use rand::SeedableRng;

    #[test]
    fn simulation_is_deterministic_and_covers_the_globe() {
        let run = || {
            let mut rng = ChaCha8Rng::seed_from_u64(7);
            let (pm, plates) = generate_plates(128, 64, None, WorldStyle::Earthlike, &mut rng);
            let mut sim = TectonicSim::new(&pm, &plates, &mut rng, &TectonicParams { steps: 20, ..Default::default() });
            sim.run();
            sim.finish(plates)
        };
        let a = run();
        let b = run();
        assert!(a.plate_map.iter().all(|(_, _, id)| !id.is_none()), "every cell must be owned");
        assert!(a.crust.thickness_km.iter().all(|(_, _, &t)| (1.0..=MAX_CRUST_KM + 1.0).contains(&t)));
        let same = a.plate_map.iter().zip(b.plate_map.iter()).all(|(p, q)| p.2 == q.2);
        assert!(same, "same seed must give the same plates");
        assert!(a.plates.iter().enumerate().all(|(i, p)| p.id.0 as usize == i));
    }
}
