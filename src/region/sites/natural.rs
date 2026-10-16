//! Natural wonders from the landforms: cave mouths (resurgences, potholes, cliff caves, sea
//! caves), gorges, arches and stacks, erratics and boulder fields, hot springs, geysers and mud
//! pots, salt springs, tar seeps, petrified forests, fossil beds, amber shores, rare meteorite
//! craters, oxbows, tarns, tors, pavements, blowholes, giant trees, crater lakes (ROADMAP Stage
//! 4). Springs and waterfalls are `water.rs`'s.
//!
//! Deterministic per world tile: reads only the tile and its neighbours (never past the margin),
//! keys every choice on the wrapped absolute cell, so any window plans a tile's places the same
//! way. Caused, never rolled: a rule says where a kind *can* be; the hash picks which of the
//! possible spots, and a per-kind rarity (`one_in`, `lattice`) how many tiles get one. Every
//! place carries its cause; `names.rs` gives the words.
//!
//! This file also holds the terrain scan the prehistory shares (`Scan`): per-cell slope, water
//! masks, local minima and maxima at two radii (van Herk running extrema, O(n)), summed-area
//! tables for "how much sea / lake / bog within r", and the facts of each world tile (climate,
//! soil, fertility, ore, volcanoes, rock). The landform classes are preferred where the Stage 2
//! passes set them and every rule also works from the raw fields, so places exist before they do.

use crate::biomes::ExtendedBiome;
use crate::erosion::materials::RockType;
use crate::heightmap::VolcanoType;
use crate::history::civilizations::economy::ResourceType;
use crate::region::landforms::RegionFields;
use crate::region::plan::{Landform, Place, PlaceKind};
use crate::soils::SoilKind;

use super::{Core, SitePlan};

// --- Rarity: how many tiles of those that can hold a kind get one -----------------------------
// `one_in` N: a hash of the tile passes one tile in N. `lattice` N: one tile in every N x N
// block (rare things stay rare and spaced). "Max" is per tile.
// A place is something to walk to: on seed 42 an 8-tile window of a volcanic massif held 92
// gorges and 58 tarns with the first numbers (every mountain stream and every cirque lake), so
// a kind takes one tile in several and only its best spot.
const GORGE_MAX: usize = 1;
const GORGE_ONE_IN: u64 = 5;
const GORGE_DEPTH_M: f32 = 100.0;
const ARCH_LATTICE: i64 = 6;
const STACK_ONE_IN: u64 = 4;
const ERRATIC_ONE_IN: u64 = 10;
const BOULDERS_ONE_IN: u64 = 6;
const HOT_SPRING_MAX: usize = 1;
const GEYSER_ONE_IN: u64 = 4;
const MUD_POT_ONE_IN: u64 = 4;
const SALT_SPRING_ONE_IN: u64 = 3;
const TAR_ONE_IN: u64 = 5;
const PETRIFIED_ONE_IN: u64 = 6;
const FOSSIL_ONE_IN: u64 = 4;
const AMBER_ONE_IN: u64 = 8;
const METEOR_ONE_IN: u64 = 4000;
const OXBOW_ONE_IN: u64 = 8;
const TARN_MAX: usize = 1;
const TARN_ONE_IN: u64 = 5;
const TARN_WALLS_M: f32 = 150.0;
const TOR_ONE_IN: u64 = 6;
const PAVEMENT_ONE_IN: u64 = 8;
const BLOWHOLE_ONE_IN: u64 = 5;
const CLIFF_CAVE_MAX: usize = 1;
const CLIFF_CAVE_ONE_IN: u64 = 8;
const SINK_CAVE_MAX: usize = 1;
const SEA_CAVE_ONE_IN: u64 = 8;
/// Cells between any two places of a tile, and between two of one kind.
pub(super) const MIN_APART: i64 = 3;
pub(super) const MIN_APART_SAME: i64 = 8;
/// Candidates are read on this lattice of cells (a place needs no finer a spot).
pub(super) const STRIDE: usize = 2;

// --- Hashing (the same shape as `local::wildlife`) ---------------------------------------------

pub(super) fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

pub(super) fn unit(a: i64, b: i64, salt: u64) -> f32 {
    (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0
}

/// One tile in `n` (by hash) holds the kind.
pub(super) fn one_in(tile: (usize, usize), salt: u64, n: u64) -> bool {
    hash(tile.0 as i64, tile.1 as i64, salt) % n == 0
}

/// One tile in every `n` x `n` block (the block picks which of its tiles, by hash).
pub(super) fn lattice(tile: (usize, usize), salt: u64, n: i64) -> bool {
    let (tx, ty) = (tile.0 as i64, tile.1 as i64);
    let (bx, by) = (tx.div_euclid(n), ty.div_euclid(n));
    let pick = hash(bx, by, salt) % (n * n) as u64;
    (tx.rem_euclid(n) + n * ty.rem_euclid(n)) as u64 == pick
}

// --- Summed-area tables and running extrema ----------------------------------------------------

/// Counts of a mask in any rectangle in O(1).
pub(super) struct Sat {
    w: usize,
    h: usize,
    s: Vec<u32>,
}

impl Sat {
    pub fn new(mask: &[bool], w: usize, h: usize) -> Sat {
        let mut s = vec![0u32; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = 0u32;
            for x in 0..w {
                row += mask[y * w + x] as u32;
                s[(y + 1) * (w + 1) + x + 1] = s[y * (w + 1) + x + 1] + row;
            }
        }
        Sat { w, h, s }
    }
    /// Count within `r` cells of (x, y) (a square), the grid's edge clipping it.
    pub fn count(&self, x: usize, y: usize, r: usize) -> u32 {
        let x0 = x.saturating_sub(r);
        let y0 = y.saturating_sub(r);
        let x1 = (x + r + 1).min(self.w);
        let y1 = (y + r + 1).min(self.h);
        let w1 = self.w + 1;
        self.s[y1 * w1 + x1] + self.s[y0 * w1 + x0] - self.s[y0 * w1 + x1] - self.s[y1 * w1 + x0]
    }
    pub fn share(&self, x: usize, y: usize, r: usize) -> f32 {
        let x0 = x.saturating_sub(r);
        let y0 = y.saturating_sub(r);
        let x1 = (x + r + 1).min(self.w);
        let y1 = (y + r + 1).min(self.h);
        self.count(x, y, r) as f32 / ((x1 - x0) * (y1 - y0)).max(1) as f32
    }
}

/// Running minimum (or maximum) over a window of 2r + 1 along one line (van Herk / Gil-Werman).
fn running_extreme(line: &[f32], r: usize, out: &mut [f32], max: bool) {
    let n = line.len();
    let pick = |a: f32, b: f32| if max { a.max(b) } else { a.min(b) };
    let k = 2 * r + 1;
    let mut pre = vec![0.0f32; n];
    let mut suf = vec![0.0f32; n];
    for start in (0..n).step_by(k) {
        let end = (start + k).min(n);
        pre[start] = line[start];
        for i in start + 1..end { pre[i] = pick(pre[i - 1], line[i]); }
        suf[end - 1] = line[end - 1];
        for i in (start..end - 1).rev() { suf[i] = pick(suf[i + 1], line[i]); }
    }
    for i in 0..n {
        let lo = i.saturating_sub(r);
        let hi = (i + r).min(n - 1);
        // A window cut by the line's ends is shorter than a block, and the block extrema
        // would reach past it: those few are taken the slow way.
        out[i] = if lo + k - 1 == hi { pick(suf[lo], pre[hi]) } else { line[lo..=hi].iter().copied().fold(line[lo], pick) };
    }
}

/// The minimum (or maximum) within `r` cells (a square) of every cell, O(n).
pub(super) fn local_extreme(src: &[f32], w: usize, h: usize, r: usize, max: bool) -> Vec<f32> {
    let mut rows = vec![0.0f32; w * h];
    let mut tmp = vec![0.0f32; w.max(h)];
    for y in 0..h {
        running_extreme(&src[y * w..(y + 1) * w], r, &mut tmp[..w], max);
        rows[y * w..(y + 1) * w].copy_from_slice(&tmp[..w]);
    }
    let mut out = vec![0.0f32; w * h];
    let mut col = vec![0.0f32; h];
    for x in 0..w {
        for y in 0..h { col[y] = rows[y * w + x]; }
        running_extreme(&col, r, &mut tmp[..h], max);
        for y in 0..h { out[y * w + x] = tmp[y]; }
    }
    out
}

// --- The facts of a world tile ----------------------------------------------------------------

/// What a world tile is, for the rules: its climate, soil, fertility, rock, ore, volcanoes.
#[derive(Clone, Debug)]
pub(super) struct TileFacts {
    pub biome: ExtendedBiome,
    pub soil: SoilKind,
    /// `world.resources().fertility` (0-1).
    pub fertility: f32,
    pub temp: f32,
    pub moist: f32,
    pub hardness: f32,
    pub stress: f32,
    pub lat_deg: f32,
    /// The ice reached here: past 40 degrees, or cold today (the ice age was colder everywhere).
    pub once_cold: bool,
    /// Nearest volcano within 4 tiles: (distance in tiles, active, kind).
    pub volcano: Option<(f32, bool, VolcanoType)>,
    pub copper_near: bool,
    pub gold_near: bool,
    pub salt_near: bool,
    pub coal_near: bool,
    pub salt_flats_near: bool,
    pub ice: bool,
}

impl TileFacts {
    pub fn of(f: &RegionFields, tile: (usize, usize)) -> TileFacts {
        let w = f.world;
        let (tx, ty) = tile;
        let ww = w.width as i64;
        let biome = *w.biomes.get(tx, ty);
        let res = w.resources();
        let near = |kinds: &[ResourceType], r: i64| res.deposits_near(tx, ty, r, w.width).iter().any(|d| kinds.contains(&d.kind));
        let lat_deg = 90.0 - (ty as f32 + 0.5) / w.height as f32 * 180.0;
        let temp = *w.temperature.get(tx, ty);
        let volcano = w.volcanoes.iter()
            .map(|v| {
                let dx = ((v.x as i64 - tx as i64).rem_euclid(ww)).min((tx as i64 - v.x as i64).rem_euclid(ww)) as f32;
                let dy = (v.y as f32 - ty as f32).abs();
                ((dx * dx + dy * dy).sqrt(), v.is_active, v.volcano_type)
            })
            .filter(|(d, _, _)| *d <= 4.0)
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut salt_flats_near = false;
        for dy in -2i64..=2 {
            for dx in -2i64..=2 {
                let yy = ty as i64 + dy;
                if yy < 0 || yy >= w.height as i64 { continue; }
                if matches!(w.biomes.get((tx as i64 + dx).rem_euclid(ww) as usize, yy as usize), ExtendedBiome::SaltFlats) { salt_flats_near = true; }
            }
        }
        TileFacts {
            biome,
            soil: *w.soils().kind.get(tx, ty),
            fertility: *res.fertility.get(tx, ty),
            temp,
            moist: *w.moisture.get(tx, ty),
            hardness: w.hardness_map.as_ref().map(|m| *m.get(tx, ty)).unwrap_or(0.5),
            stress: *w.stress_map.get(tx, ty),
            lat_deg,
            once_cold: lat_deg.abs() >= 40.0 || temp < 6.0,
            volcano,
            copper_near: near(&[ResourceType::Copper, ResourceType::Tin, ResourceType::Gold, ResourceType::Silver], 3),
            gold_near: near(&[ResourceType::Gold, ResourceType::Silver], 3),
            salt_near: near(&[ResourceType::Salt], 2),
            coal_near: near(&[ResourceType::Coal], 2),
            salt_flats_near,
            ice: matches!(biome, ExtendedBiome::Ice),
        }
    }

    pub fn desert(&self) -> bool { self.moist < 0.25 && self.temp > 8.0 }
}

// --- The scan ----------------------------------------------------------------------------------

/// The window's terrain read for the rules: cheap per-cell features over the whole grid
/// (margin included) so a candidate's test is O(1). Built once by `natural::plan` and handed
/// to `prehistory::plan` through `hand_over` (the same thread, in `sites::plan`'s order).
pub(super) struct ScanData {
    pub window: (i64, i64, usize, usize, u64),
    pub slope: Vec<f32>,
    pub sea: Vec<bool>,
    pub lake: Vec<bool>,
    /// River channel width (m), 0 where none (the zoom's own threshold).
    pub river_w: Vec<f32>,
    pub bog: Vec<bool>,
    /// Local minima and maxima of the height at the short and the long radius.
    pub lo_s: Vec<f32>,
    pub hi_s: Vec<f32>,
    pub lo_l: Vec<f32>,
    pub hi_l: Vec<f32>,
    pub r_s: usize,
    pub r_l: usize,
    pub sat_sea: Sat,
    pub sat_lake: Sat,
    pub sat_river: Sat,
    pub sat_bog: Sat,
}

thread_local! {
    static HANDOVER: std::cell::RefCell<Option<std::rc::Rc<ScanData>>> = const { std::cell::RefCell::new(None) };
}

impl ScanData {
    fn window_of(f: &RegionFields) -> (i64, i64, usize, usize, u64) {
        // The fields' identity: where the window is, how big, and a taste of its heights.
        let mut taste = f.seed ^ (f.world.seed().rotate_left(13));
        for k in (0..f.h.len()).step_by((f.h.len() / 64).max(1)) { taste = taste.rotate_left(5) ^ f.h[k].to_bits() as u64; }
        (f.gx0, f.gy0, f.gw, f.gh, taste)
    }

    pub fn new(f: &RegionFields, land: &[Landform]) -> ScanData {
        let (gw, gh) = (f.gw, f.gh);
        let n = gw * gh;
        // Radii in cells for ~2.5 km and ~8 km, at least 1 and 2 cells (the dev world's cells
        // are 3.26 km; seed 42's 610 m).
        let r_s = ((2500.0 / f.cell_m).round() as usize).clamp(1, 12);
        let r_l = ((8000.0 / f.cell_m).round() as usize).clamp(2, 24);
        let tile_cells = (f.s * f.s) as f32;
        let mut slope = vec![0.0f32; n];
        let mut sea = vec![false; n];
        let mut lake = vec![false; n];
        let mut river_w = vec![0.0f32; n];
        let mut bog = vec![false; n];
        let cell_km2 = (f.cell_m / 1000.0).powi(2);
        for j in 0..gh {
            let lc = f.lat_cos(j);
            let (j0, j1) = (j.saturating_sub(1), (j + 1).min(gh - 1));
            for i in 0..gw {
                let k = j * gw + i;
                let (i0, i1) = (i.saturating_sub(1), (i + 1).min(gw - 1));
                let dx = (f.h[j * gw + i1] - f.h[j * gw + i0]) / ((i1 - i0).max(1) as f32 * f.cell_m * lc);
                let dy = (f.h[j1 * gw + i] - f.h[j0 * gw + i]) / ((j1 - j0).max(1) as f32 * f.cell_m);
                slope[k] = (dx * dx + dy * dy).sqrt();
                sea[k] = f.is_sea(k);
                lake[k] = f.lake_depth(k) >= 0.5;
                if !sea[k] && !lake[k] {
                    // The zoom's own rule: a permanent stream needs 2% of a tile's area, more
                    // where it is dry.
                    let threshold = 0.02 * tile_cells / (0.25 + f.moist[k]);
                    if f.acc[k] > threshold { river_w[k] = 0.8 * (f.acc[k] * cell_km2 * lc).sqrt(); }
                }
            }
        }
        for k in 0..n {
            if sea[k] || lake[k] || river_w[k] > 0.0 { continue; }
            bog[k] = matches!(land[k], Landform::Bog | Landform::Fen | Landform::Marsh)
                || (slope[k] < 0.015 && f.moist[k] > 0.6 && f.temp[k] < 12.0 && f.temp[k] > -4.0);
        }
        let lo_s = local_extreme(f.h, gw, gh, r_s, false);
        let hi_s = local_extreme(f.h, gw, gh, r_s, true);
        let lo_l = local_extreme(f.h, gw, gh, r_l, false);
        let hi_l = local_extreme(f.h, gw, gh, r_l, true);
        let river_mask: Vec<bool> = river_w.iter().map(|&w| w > 0.0).collect();
        ScanData {
            window: Self::window_of(f),
            sat_sea: Sat::new(&sea, gw, gh),
            sat_lake: Sat::new(&lake, gw, gh),
            sat_river: Sat::new(&river_mask, gw, gh),
            sat_bog: Sat::new(&bog, gw, gh),
            slope, sea, lake, river_w, bog, lo_s, hi_s, lo_l, hi_l, r_s, r_l,
        }
    }

    /// Keep the data for the next planner on this thread.
    pub fn hand_over(d: std::rc::Rc<ScanData>) { HANDOVER.with(|h| *h.borrow_mut() = Some(d)); }

    /// The data the last planner left for these fields, else built now.
    pub fn take_or_new(f: &RegionFields, land: &[Landform]) -> std::rc::Rc<ScanData> {
        let kept = HANDOVER.with(|h| h.borrow_mut().take());
        match kept {
            Some(d) if d.window == Self::window_of(f) => d,
            _ => std::rc::Rc::new(ScanData::new(f, land)),
        }
    }
}

/// The scan as the rules use it: the fields, the labels, the data and what is placed so far.
pub(super) struct Scan<'a> {
    pub f: &'a RegionFields<'a>,
    pub land: &'a [Landform],
    pub rock: &'a [RockType],
    pub core: Core,
    pub d: std::rc::Rc<ScanData>,
    /// Places already put down this plan, as (wrapped absolute cell, kind), for spacing.
    pub placed: Vec<((i64, i64), PlaceKind)>,
}

impl<'a> Scan<'a> {
    pub fn new(f: &'a RegionFields<'a>, land: &'a [Landform], rock: &'a [RockType], core: Core, d: std::rc::Rc<ScanData>, out: &SitePlan) -> Scan<'a> {
        let s = f.s as i64;
        let ww = f.world.width as i64;
        // What earlier planners put down, keyed like `key`.
        let placed = out.places.iter().map(|p| {
            let (ax, ay) = (p.x.round() as i64, p.y.round() as i64);
            let tx = ax.div_euclid(s).rem_euclid(ww);
            ((tx * s + ax.rem_euclid(s), ay), p.kind)
        }).collect();
        Scan { f, land, rock, core, d, placed }
    }

    pub fn idx(&self, i: usize, j: usize) -> usize { j * self.f.gw + i }
    pub fn h(&self, k: usize) -> f32 { self.f.h[k] }
    pub fn land_cell(&self, k: usize) -> bool { !self.d.sea[k] && !self.d.lake[k] }
    pub fn dry(&self, k: usize) -> bool { self.land_cell(k) && self.d.river_w[k] == 0.0 }
    /// Height above the lowest ground within the short / long radius.
    pub fn rel_s(&self, k: usize) -> f32 { self.f.h[k] - self.d.lo_s[k] }
    pub fn rel_l(&self, k: usize) -> f32 { self.f.h[k] - self.d.lo_l[k] }
    /// How far the highest ground within the short / long radius stands above this cell.
    pub fn prom_s(&self, k: usize) -> f32 { self.d.hi_s[k] - self.f.h[k] }
    pub fn prom_l(&self, k: usize) -> f32 { self.d.hi_l[k] - self.f.h[k] }
    pub fn slope(&self, k: usize) -> f32 { self.d.slope[k] }
    pub fn river_w(&self, k: usize) -> f32 { self.d.river_w[k] }
    pub fn lake(&self, k: usize) -> bool { self.d.lake[k] }
    pub fn sea(&self, k: usize) -> bool { self.d.sea[k] }
    pub fn bog(&self, k: usize) -> bool { self.d.bog[k] }
    pub fn hi_s(&self, k: usize) -> f32 { self.d.hi_s[k] }
    pub fn r_s(&self) -> usize { self.d.r_s }
    pub fn sat_sea(&self) -> &Sat { &self.d.sat_sea }
    pub fn sat_lake(&self) -> &Sat { &self.d.sat_lake }
    pub fn sat_river(&self) -> &Sat { &self.d.sat_river }
    pub fn sat_bog(&self) -> &Sat { &self.d.sat_bog }
    /// Cells for a distance in metres, at least 1.
    pub fn cells(&self, m: f32) -> usize { ((m / self.f.cell_m).round() as usize).max(1) }
    pub fn hard(&self, k: usize) -> bool { matches!(self.rock[k], RockType::Granite | RockType::Basalt | RockType::Sandstone | RockType::Limestone) }
    pub fn sedimentary(&self, k: usize) -> bool { matches!(self.rock[k], RockType::Sandstone | RockType::Limestone | RockType::Shale | RockType::Sediment) }

    /// The wrapped absolute cell of (i, j): the hash key every choice uses.
    pub fn key(&self, i: usize, j: usize) -> (i64, i64) {
        let (tx, ty) = self.f.tile_of(i, j);
        let s = self.f.s as i64;
        let (ax, ay) = self.f.abs_cell(i, j);
        (tx as i64 * s + ax.rem_euclid(s), ty as i64 * s + ay.rem_euclid(s))
    }

    /// The range of grid cells of core tile (x, y) (x unwrapped).
    pub fn tile_cells(&self, tile: (i64, i64)) -> (usize, usize, usize, usize) {
        let s = self.f.s;
        let i0 = ((tile.0 - self.f.gx0) as usize) * s;
        let j0 = ((tile.1 - self.f.gy0) as usize) * s;
        (i0, j0, i0 + s, j0 + s)
    }

    /// The wrapped world tile of a core tile.
    pub fn wrap_tile(&self, tile: (i64, i64)) -> (usize, usize) {
        (tile.0.rem_euclid(self.f.world.width as i64) as usize, tile.1.clamp(0, self.f.world.height as i64 - 1) as usize)
    }

    /// Curvature: the cell less the mean of its four neighbours (positive on a crest).
    pub fn convexity(&self, i: usize, j: usize) -> f32 {
        let (gw, gh) = (self.f.gw, self.f.gh);
        if i == 0 || j == 0 || i + 1 >= gw || j + 1 >= gh { return 0.0; }
        let k = self.idx(i, j);
        let h = self.f.h;
        h[k] - 0.25 * (h[k - 1] + h[k + 1] + h[k - gw] + h[k + gw])
    }

    /// In how many of the eight directions the ground falls by `drop` m within `r` cells.
    pub fn falls_around(&self, i: usize, j: usize, r: usize, drop: f32) -> usize {
        let (gw, gh) = (self.f.gw as i64, self.f.gh as i64);
        let h0 = self.f.h[self.idx(i, j)];
        let mut n = 0;
        for (dx, dy) in [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)] {
            for t in 1..=r as i64 {
                let (x, y) = (i as i64 + dx * t, j as i64 + dy * t);
                if x < 0 || y < 0 || x >= gw || y >= gh { break; }
                if h0 - self.f.h[(y * gw + x) as usize] >= drop { n += 1; break; }
            }
        }
        n
    }

    /// A lake cell touches (i, j).
    pub fn lake_beside(&self, i: usize, j: usize) -> bool { self.d.sat_lake.count(i, j, 1) > 0 && !self.d.lake[self.idx(i, j)] }
    pub fn sea_beside(&self, i: usize, j: usize) -> bool { self.d.sat_sea.count(i, j, 1) > 0 && !self.d.sea[self.idx(i, j)] }

    /// The direction of flow at k as a unit step, if it drains to a neighbour.
    pub fn flow_dir(&self, k: usize) -> Option<(i64, i64)> {
        let r = self.f.recv[k];
        if r == u32::MAX { return None; }
        let gw = self.f.gw as i64;
        let (x0, y0) = ((k % self.f.gw) as i64, (k / self.f.gw) as i64);
        let (x1, y1) = ((r as i64) % gw, (r as i64) / gw);
        Some(((x1 - x0).signum(), (y1 - y0).signum()))
    }

    /// Nothing placed within `apart` cells (and nothing of the same kind within `same`).
    pub fn free(&self, key: (i64, i64), kind: PlaceKind) -> bool {
        self.placed.iter().all(|(p, k)| {
            let d = (p.0 - key.0).abs().max((p.1 - key.1).abs());
            d >= MIN_APART && (*k != kind || d >= MIN_APART_SAME)
        })
    }

    /// Put a place down at grid cell (i, j) with its cause; `None` if the spot is taken.
    pub fn put(&mut self, out: &mut SitePlan, kind: PlaceKind, i: usize, j: usize, cause: String) -> Option<usize> {
        let key = self.key(i, j);
        if !self.free(key, kind) { return None; }
        let (ax, ay) = self.f.abs_cell(i, j);
        let tile = self.f.tile_of(i, j);
        let mut p = Place::point(kind, ax as f64, ay as f64, tile);
        p.cause = cause;
        p.z_m = Some(self.f.h[self.idx(i, j)]);
        self.placed.push((key, kind));
        out.places.push(p);
        Some(out.places.len() - 1)
    }

    /// The best candidate of a tile by score: the lattice cells of the tile that pass `test`
    /// (returning a score), the highest score winning, ties broken by the cell's hash.
    pub fn best(&self, tile: (i64, i64), test: impl Fn(&Scan, usize, usize, usize) -> Option<f32>) -> Option<(usize, usize, f32)> {
        let (i0, j0, i1, j1) = self.tile_cells(tile);
        let mut best: Option<(usize, usize, f32, u64)> = None;
        for j in (j0..j1).step_by(STRIDE) {
            for i in (i0..i1).step_by(STRIDE) {
                let k = self.idx(i, j);
                if let Some(score) = test(self, i, j, k) {
                    let key = self.key(i, j);
                    let tie = hash(key.0, key.1, 7);
                    if best.map_or(true, |b| score > b.2 || (score == b.2 && tie > b.3)) {
                        best = Some((i, j, score, tie));
                    }
                }
            }
        }
        best.map(|(i, j, s, _)| (i, j, s))
    }

    /// Up to `max` candidates of a tile, best first, each at least `MIN_APART_SAME` from the
    /// others.
    pub fn several(&self, tile: (i64, i64), max: usize, test: impl Fn(&Scan, usize, usize, usize) -> Option<f32>) -> Vec<(usize, usize, f32)> {
        let (i0, j0, i1, j1) = self.tile_cells(tile);
        let mut all: Vec<(usize, usize, f32, u64)> = Vec::new();
        for j in (j0..j1).step_by(STRIDE) {
            for i in (i0..i1).step_by(STRIDE) {
                let k = self.idx(i, j);
                if let Some(score) = test(self, i, j, k) {
                    let key = self.key(i, j);
                    all.push((i, j, score, hash(key.0, key.1, 7)));
                }
            }
        }
        all.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap().then(b.3.cmp(&a.3)));
        let mut out: Vec<(usize, usize, f32)> = Vec::new();
        for (i, j, s, _) in all {
            if out.len() >= max { break; }
            if out.iter().all(|&(x, y, _)| (x as i64 - i as i64).abs().max((y as i64 - j as i64).abs()) >= MIN_APART_SAME) {
                out.push((i, j, s));
            }
        }
        out
    }
}

// --- The rules ---------------------------------------------------------------------------------

pub(super) fn rock_word(r: RockType) -> &'static str {
    match r {
        RockType::Basalt => "basalt", RockType::Granite => "granite", RockType::Sandstone => "sandstone",
        RockType::Limestone => "limestone", RockType::Shale => "shale", RockType::Sediment => "soft ground", RockType::Ice => "ice",
    }
}

pub fn plan(f: &RegionFields, land: &[Landform], rock: &[RockType], core: Core, out: &mut SitePlan) {
    let d = ScanData::take_or_new(f, land);
    let mut sc = Scan::new(f, land, rock, core, d.clone(), out);
    for tile in core.tiles() {
        let wt = sc.wrap_tile(tile);
        let facts = TileFacts::of(f, wt);
        natural_tile(&mut sc, &facts, tile, wt, out);
    }
    // The prehistory reads the same scan (`prehistory::plan` takes it, or builds its own).
    ScanData::hand_over(d);
}

fn natural_tile(sc: &mut Scan, t: &TileFacts, tile: (i64, i64), wt: (usize, usize), out: &mut SitePlan) {
    let cell_m = sc.f.cell_m;
    let s = sc.f.s;

    // Gorges: a channel with both banks rising 100 m+ within ~3 cells across the flow, the
    // deepest of a tile, on one tile in three.
    let r3 = sc.cells(1800.0).min(4);
    let gorges = if one_in(wt, 10, GORGE_ONE_IN) { GORGE_MAX } else { 0 };
    for (i, j, _) in sc.several(tile, gorges, |sc, i, j, k| {
        if sc.river_w(k) <= 0.0 { return None; }
        if matches!(sc.land[k], Landform::Gorge | Landform::KarstGorge) && sc.prom_s(k) >= GORGE_DEPTH_M { return Some(500.0 + sc.prom_s(k)); }
        let (dx, dy) = sc.flow_dir(k)?;
        let (px, py) = (-dy, dx);
        let (gw, gh) = (sc.f.gw as i64, sc.f.gh as i64);
        let h0 = sc.h(k);
        let mut rise = [0.0f32; 2];
        for (side, sign) in [(0usize, 1i64), (1, -1)] {
            for t in 1..=r3 as i64 {
                let (x, y) = (i as i64 + px * sign * t, j as i64 + py * sign * t);
                if x < 0 || y < 0 || x >= gw || y >= gh { break; }
                let r = sc.h((y * gw + x) as usize) - h0;
                if r > rise[side] { rise[side] = r; }
            }
        }
        let depth = rise[0].min(rise[1]);
        if depth >= GORGE_DEPTH_M { Some(depth) } else { None }
    }) {
        let k = sc.idx(i, j);
        let depth = (sc.prom_s(k)).round();
        let cause = format!("the river has cut {} m into {} here", depth.max(60.0), rock_word(sc.rock[k]));
        sc.put(out, PlaceKind::Gorge, i, j, cause);
    }

    // Natural arches: rare, in hard rock at a bend of a river or on a hard coast.
    if lattice(wt, 11, ARCH_LATTICE) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.land_cell(k) || !matches!(sc.rock[k], RockType::Sandstone | RockType::Limestone | RockType::Basalt) { return None; }
            if sc.slope(k) < 0.15 { return None; }
            let bend = sc.sat_river().count(i, j, 2) >= 3;
            let coast = sc.sea_beside(i, j) && sc.sat_sea().share(i, j, 4) > 0.3;
            if bend || coast { Some(sc.slope(k) + if coast { 0.2 } else { 0.0 }) } else { None }
        }) {
            let k = sc.idx(i, j);
            let cause = if sc.sea_beside(i, j) { format!("the sea has eaten through a headland of {}", rock_word(sc.rock[k])) } else { format!("a river bend has cut through a fin of {}", rock_word(sc.rock[k])) };
            sc.put(out, PlaceKind::NaturalArch, i, j, cause);
        }
    }

    // Sea stacks and blowholes: hard, exposed coast.
    let hard_coast = |sc: &Scan, i: usize, j: usize, k: usize| -> Option<f32> {
        if !sc.land_cell(k) || !sc.hard(k) || sc.slope(k) < 0.2 || !sc.sea_beside(i, j) { return None; }
        if matches!(sc.land[k], Landform::SeaCliff | Landform::Stack) { return Some(400.0); }
        let exposure = sc.sat_sea().share(i, j, 6);
        if exposure > 0.45 { Some(exposure + sc.slope(k)) } else { None }
    };
    if one_in(wt, 12, STACK_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, hard_coast) {
            let k = sc.idx(i, j);
            let cause = format!("the swell has cut a {} headland back and left a pillar standing", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::SeaStack, i, j, cause);
        }
    }
    if one_in(wt, 13, BLOWHOLE_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, hard_coast) {
            let k = sc.idx(i, j);
            let cause = format!("a sea cave in the {} cliff has broken through to the cliff top", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::Blowhole, i, j, cause);
        }
    }

    // Erratics and boulder fields where the ice reached.
    if t.once_cold && !t.ice {
        if one_in(wt, 14, ERRATIC_ONE_IN) {
            if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
                if matches!(sc.land[k], Landform::Erratic) { return Some(300.0); }
                if !sc.dry(k) || sc.bog(k) || sc.slope(k) > 0.05 || sc.rel_s(k) > 15.0 { return None; }
                Some(1.0 - sc.slope(k))
            }) {
                sc.put(out, PlaceKind::Erratic, i, j, "a boulder the ice carried here and dropped when it melted".into());
            }
        }
        if one_in(wt, 15, BOULDERS_ONE_IN) {
            if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
                if matches!(sc.land[k], Landform::Scree | Landform::Outwash) { return Some(300.0); }
                if !sc.dry(k) || !sc.hard(k) || sc.slope(k) < 0.15 || sc.slope(k) > 0.5 || sc.prom_s(k) < 40.0 { return None; }
                Some(sc.prom_s(k))
            }) {
                let k = sc.idx(i, j);
                let cause = format!("frost has broken the {} of the slope above into blocks", rock_word(sc.rock[k]));
                sc.put(out, PlaceKind::BoulderField, i, j, cause);
            }
        }
    }

    // Hot springs, geysers, mud pots: heat under the ground on a fault.
    let thermal = t.volcano.map_or(false, |(d, _, _)| d <= 3.0) && t.stress > 0.15
        || (sc.tile_cells(tile).0..sc.tile_cells(tile).2).step_by(16).any(|i| (sc.tile_cells(tile).1..sc.tile_cells(tile).3).step_by(16).any(|j| matches!(sc.land[sc.idx(i, j)], Landform::ThermalGround | Landform::Fumarole)));
    if thermal {
        // On thermal ground where the landforms mark it, else low flat dry ground; never in water.
        let low_wet = |sc: &Scan, _i: usize, _j: usize, k: usize| -> Option<f32> {
            if !sc.land_cell(k) { return None; }
            if matches!(sc.land[k], Landform::ThermalGround | Landform::Fumarole) { return Some(300.0 - sc.rel_s(k)); }
            if !sc.dry(k) || sc.rel_s(k) > 12.0 || sc.slope(k) > 0.08 { return None; }
            Some(100.0 - sc.rel_s(k))
        };
        let vol = t.volcano.map(|(d, _, _)| d).unwrap_or(9.0);
        // A volcanic massif is thermal on every tile: a spring on one tile in two, two where the
        // landforms found thermal ground.
        let marked = (sc.tile_cells(tile).0..sc.tile_cells(tile).2).step_by(8).any(|i| (sc.tile_cells(tile).1..sc.tile_cells(tile).3).step_by(8).any(|j| matches!(sc.land[sc.idx(i, j)], Landform::ThermalGround | Landform::Fumarole)));
        let want = if marked { HOT_SPRING_MAX } else if one_in(wt, 29, 2) { 1 } else { 0 };
        for (i, j, _) in sc.several(tile, want, low_wet) {
            let cause = format!("water heated by the fire under the ground rises along a fault, the volcano {:.0} tiles off", vol);
            sc.put(out, PlaceKind::HotSpring, i, j, cause);
        }
        if t.volcano.map_or(false, |(d, active, _)| active && d <= 2.0) && one_in(wt, 16, GEYSER_ONE_IN) {
            if let Some((i, j, _)) = sc.best(tile, low_wet) {
                sc.put(out, PlaceKind::Geyser, i, j, "a flooded fissure over hot rock boils over at intervals".into());
            }
        }
        if one_in(wt, 17, MUD_POT_ONE_IN) {
            if let Some((i, j, _)) = sc.best(tile, low_wet) {
                sc.put(out, PlaceKind::MudPot, i, j, "sour steam has rotted the rock to clay that boils".into());
            }
        }
    }

    // Salt springs in dry basins with salt in the ground.
    if t.moist < 0.3 && (t.salt_near || t.salt_flats_near) && one_in(wt, 18, SALT_SPRING_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if !sc.dry(k) || sc.rel_l(k) > 30.0 || sc.slope(k) > 0.05 { return None; }
            Some(50.0 - sc.rel_l(k))
        }) {
            sc.put(out, PlaceKind::SaltSpring, i, j, "ground water through old salt beds rises in a dry basin; the game paths meet here".into());
        }
    }

    // Tar seeps: warm sedimentary lowlands over coal measures.
    if t.coal_near && t.temp > 10.0 && one_in(wt, 19, TAR_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if !sc.dry(k) || !sc.sedimentary(k) || sc.rel_s(k) > 10.0 || sc.slope(k) > 0.04 { return None; }
            Some(20.0 - sc.rel_s(k))
        }) {
            sc.put(out, PlaceKind::TarSeep, i, j, "pitch from the coal measures below wells up through the clay".into());
        }
    }

    // Petrified forests: old ash over a wood that stood in a forest climate.
    if t.volcano.map_or(false, |(d, active, _)| !active && d <= 4.0) && t.moist > 0.45 && t.temp > 5.0 && t.temp < 25.0 && one_in(wt, 20, PETRIFIED_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if matches!(sc.land[k], Landform::AshPlain | Landform::LavaField) { return Some(300.0); }
            if !sc.dry(k) || sc.slope(k) < 0.08 || sc.slope(k) > 0.4 { return None; }
            Some(sc.slope(k))
        }) {
            sc.put(out, PlaceKind::PetrifiedForest, i, j, "a wood buried in ash by the old volcano and turned to stone, now washed out of the hillside".into());
        }
    }

    // Fossil beds in the faces of sedimentary rock.
    if one_in(wt, 21, FOSSIL_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if !sc.land_cell(k) || !matches!(sc.rock[k], RockType::Sandstone | RockType::Shale | RockType::Limestone) { return None; }
            let cliff = matches!(sc.land[k], Landform::Cliff | Landform::SeaCliff | Landform::Badland);
            if !cliff && sc.slope(k) < 0.3 { return None; }
            Some(sc.slope(k) + if cliff { 1.0 } else { 0.0 })
        }) {
            let k = sc.idx(i, j);
            let cause = format!("a face of {} laid down under an old sea, full of shells", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::FossilBed, i, j, cause);
        }
    }

    // Amber shores: a cold wooded coast.
    if t.temp > 0.0 && t.temp < 10.0 && t.moist > 0.4 && one_in(wt, 22, AMBER_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || !sc.sea_beside(i, j) || sc.slope(k) > 0.05 { return None; }
            Some(1.0 - sc.slope(k))
        }) {
            sc.put(out, PlaceKind::AmberShore, i, j, "the resin of a drowned forest, washed up after storms".into());
        }
    }

    // A meteorite crater: very rare, on flattish ground.
    if one_in(wt, 23, METEOR_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if !sc.dry(k) || sc.slope(k) > 0.06 { return None; }
            Some(1.0 - sc.slope(k))
        }) {
            let radius = 300.0 + 1200.0 * unit(wt.0 as i64, wt.1 as i64, 24);
            if let Some(ix) = sc.put(out, PlaceKind::MeteorCrater, i, j, "a stone from the sky struck here and left a ring of broken ground".into()) {
                out.places[ix].radius_m = radius;
                out.places[ix].rank = 0;
            }
        }
    }

    // Oxbows beside big, slow rivers.
    if one_in(wt, 25, OXBOW_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if matches!(sc.land[k], Landform::Oxbow) { return Some(300.0); }
            if !sc.dry(k) || sc.rel_s(k) > 6.0 || sc.slope(k) > 0.01 { return None; }
            let r = sc.cells(1500.0).min(3);
            let big = sc.sat_river().count(i, j, r) > 0;
            if !big { return None; }
            // A big river: the widest channel within r.
            let (x0, y0) = (i.saturating_sub(r), j.saturating_sub(r));
            let (x1, y1) = ((i + r + 1).min(sc.f.gw), (j + r + 1).min(sc.f.gh));
            let mut w = 0.0f32;
            for y in y0..y1 { for x in x0..x1 { w = w.max(sc.river_w(y * sc.f.gw + x)); } }
            if w < 40.0 { return None; }
            Some(w)
        }) {
            sc.put(out, PlaceKind::Oxbow, i, j, "a bend the river cut off and left as a curved lake on its floodplain".into());
        }
    }

    // Tarns: small high lakes in bowls with walls, the best of a tile, on one tile in three.
    let tarn_want = if one_in(wt, 30, TARN_ONE_IN) { TARN_MAX } else { 0 };
    let tarns = sc.several(tile, tarn_want, |sc, i, j, k| {
        if !sc.lake(k) { return None; }
        let surface = sc.f.filled[k];
        let walls = sc.hi_s(k) - surface;
        if matches!(sc.land[k], Landform::Tarn | Landform::Cirque) && walls >= TARN_WALLS_M * 0.5 { return Some(1000.0 + walls); }
        if surface < 1500.0 { return None; }
        let r = sc.cells(3000.0).min(6);
        if sc.sat_lake().count(i, j, r) > 40 { return None; }
        if walls < TARN_WALLS_M { return None; }
        Some(walls)
    });
    for (i, j, _) in tarns {
        let k = sc.idx(i, j);
        let cause = format!("a hollow the ice scooped at {} m, filled by the snow", sc.f.filled[k].round());
        sc.put(out, PlaceKind::Tarn, i, j, cause);
    }

    // Tors on granite summits.
    if one_in(wt, 26, TOR_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if matches!(sc.land[k], Landform::Tor) { return Some(300.0); }
            if !sc.dry(k) || sc.rock[k] != RockType::Granite || sc.prom_s(k) > 0.5 { return None; }
            let r = sc.rel_s(k);
            if r < 20.0 || r > 200.0 { return None; }
            Some(r)
        }) {
            sc.put(out, PlaceKind::Tor, i, j, "granite weathered along its joints into a stack of rounded blocks on the summit".into());
        }
    }

    // Limestone pavements on flat high ground.
    if one_in(wt, 27, PAVEMENT_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if matches!(sc.land[k], Landform::Pavement) { return Some(300.0); }
            if !sc.dry(k) || sc.rock[k] != RockType::Limestone || sc.slope(k) > 0.02 || sc.rel_l(k) < 50.0 || sc.prom_s(k) > 15.0 { return None; }
            Some(sc.rel_l(k))
        }) {
            sc.put(out, PlaceKind::Pavement, i, j, "bare limestone the ice scraped clean, split by rain into clints and grikes".into());
        }
    }

    // Giant trees and crater lakes: the world's own patches (a grove spans tiles: one tree in
    // three of them is the one worth the walk).
    if matches!(t.biome, ExtendedBiome::AncientGrove) && one_in(wt, 34, 3) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if !sc.dry(k) || sc.slope(k) > 0.1 { return None; }
            Some(sc.f.moist[k] - sc.slope(k))
        }) {
            sc.put(out, PlaceKind::GiantTree, i, j, "the oldest tree of an ancient grove the axe never reached".into());
        }
    }
    if matches!(t.biome, ExtendedBiome::CraterLake | ExtendedBiome::Caldera) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if sc.lake(k) { return Some(1000.0 - sc.f.filled[k].abs() * 0.0 + sc.sat_lake().count(i, j, 2) as f32); }
            if !sc.dry(k) { return None; }
            Some(-sc.rel_l(k))
        }) {
            sc.put(out, PlaceKind::CraterLake, i, j, "a lake in the bowl of a crater".into());
        }
    }

    // Cave mouths: cliff feet in limestone, sinks and resurgences, sea caves in hard cliffs.
    let cliff_foot = |sc: &Scan, i: usize, j: usize, k: usize| -> Option<f32> {
        if !sc.dry(k) || sc.rock[k] != RockType::Limestone { return None; }
        // A face above: the ground within 2 cells stands 25 m+ higher and this cell is gentle.
        if sc.slope(k) > 0.12 || sc.prom_s(k) < 25.0 { return None; }
        let face = sc.falls_around(i, j, 1, 0.0) < 8 && sc.hi_s(k) - sc.h(k) >= 25.0;
        if !face { return None; }
        let steep_above = (0..8).any(|d| {
            let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)][d];
            let (x, y) = (i as i64 + dx, j as i64 + dy);
            x >= 0 && y >= 0 && (x as usize) < sc.f.gw && (y as usize) < sc.f.gh && sc.slope(sc.idx(x as usize, y as usize)) > 0.35
        });
        if !steep_above { return None; }
        Some(sc.prom_s(k))
    };
    let cliff_caves = if one_in(wt, 31, CLIFF_CAVE_ONE_IN) { CLIFF_CAVE_MAX } else { 0 };
    for (i, j, _) in sc.several(tile, cliff_caves, cliff_foot) {
        sc.put(out, PlaceKind::CaveMouth, i, j, "an opening at the foot of a limestone face where a stream once came out".into());
    }
    // Sinks and resurgences are the walk-in mouths of the karst (one tile in six has one worth
    // the name); a doline or a cenote opens into a cave on one limestone tile in eight.
    let sinks = one_in(wt, 35, 6);
    let pits = one_in(wt, 33, 8);
    for (i, j, _) in sc.several(tile, SINK_CAVE_MAX, |sc, _i, _j, k| {
        match sc.land[k] {
            Landform::Sink if sinks => Some(200.0),
            Landform::Resurgence if sinks => Some(180.0),
            Landform::Doline if pits => Some(100.0),
            Landform::Cenote if pits => Some(90.0),
            _ => None,
        }
    }) {
        let k = sc.idx(i, j);
        let cause = match sc.land[k] {
            Landform::Sink => "a stream sinks into the limestone here",
            Landform::Resurgence => "the water the plateau swallowed comes out here",
            Landform::Doline => "the roof of a cave fell in and left a hollow with a hole in its floor",
            _ => "a flooded shaft into the limestone",
        };
        sc.put(out, PlaceKind::CaveMouth, i, j, cause.into());
    }
    if one_in(wt, 28, SEA_CAVE_ONE_IN) {
        if let Some((i, j, _)) = sc.best(tile, hard_coast) {
            let k = sc.idx(i, j);
            let cause = format!("the swell has hollowed a cave along a joint in the {} cliff", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::CaveMouth, i, j, cause);
        }
    }
    let _ = (cell_m, s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_extremes_match_the_slow_way() {
        let line: Vec<f32> = (0..37).map(|i| ((i * 7919) % 101) as f32).collect();
        for r in [1usize, 2, 5] {
            let mut fast = vec![0.0; line.len()];
            running_extreme(&line, r, &mut fast, true);
            for i in 0..line.len() {
                let lo = i.saturating_sub(r);
                let hi = (i + r).min(line.len() - 1);
                let slow = line[lo..=hi].iter().cloned().fold(f32::MIN, f32::max);
                assert_eq!(fast[i], slow, "r {r} i {i}");
            }
        }
        let w = 9;
        let grid: Vec<f32> = (0..w * 7).map(|i| ((i * 31) % 17) as f32).collect();
        let lo = local_extreme(&grid, w, 7, 2, false);
        for y in 0..7usize {
            for x in 0..w {
                let mut m = f32::MAX;
                for yy in y.saturating_sub(2)..(y + 3).min(7) { for xx in x.saturating_sub(2)..(x + 3).min(w) { m = m.min(grid[yy * w + xx]); } }
                assert_eq!(lo[y * w + x], m);
            }
        }
    }

    #[test]
    fn summed_area_counts() {
        let w = 6;
        let mask: Vec<bool> = (0..w * 5).map(|i| i % 3 == 0).collect();
        let sat = Sat::new(&mask, w, 5);
        for y in 0..5usize {
            for x in 0..w {
                let mut n = 0;
                for yy in y.saturating_sub(1)..(y + 2).min(5) { for xx in x.saturating_sub(1)..(x + 2).min(w) { n += mask[yy * w + xx] as u32; } }
                assert_eq!(sat.count(x, y, 1), n);
            }
        }
    }

    #[test]
    fn a_lattice_picks_one_tile_a_block() {
        for bx in 0..5i64 {
            for by in 0..5i64 {
                let n: usize = (0..4).flat_map(|x| (0..4).map(move |y| (x, y)))
                    .filter(|&(x, y)| lattice(((bx * 4 + x) as usize, (by * 4 + y) as usize), 11, 4)).count();
                assert_eq!(n, 1);
            }
        }
    }
}
