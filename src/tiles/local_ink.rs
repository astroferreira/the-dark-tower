//! Embarks drawn in the world map's ink style (the surface view of a playable area).
//!
//! The world map is ink cartography: muted washes on parchment, sepia outlines, shadow sides
//! hatched rather than darkened. Embarks used to reuse the world atlas's tiles cell by cell, so
//! a 2 m tree was one cell with a tiny glyph and a forest read as a green field with specks.
//! Here everything is drawn per pixel from the map itself:
//! - ground: a wash per material, blended across cells and mottled like watercolour, with ink
//!   tufts on grass, stipple on sand and gravel, cobbles on dressed stone;
//! - trees and shrubs: crowns several metres across seen from above (broadleaf lobed, conifer
//!   star-shaped), outlined in ink, the side away from the top-left light hatched, a hatched
//!   shadow cast on the ground; southern crowns overlap northern ones;
//! - water: depth-graded washes, an ink bank where it meets land and sparse ripple strokes;
//! - steps in the ground: an ink edge along each rise with short hachures falling downhill;
//! - walls of buildings: inked and hatched, timber warm, stone grey.
//! Every mark is keyed on the absolute cell position, so it doesn't swim as the camera moves.

use rayon::prelude::*;
use crate::local::{LocalMap, Material, Plant, Shape, TreeKind, WATER_FULL};
use crate::local::wildlife::Feature;
use super::render::LocalCamera;

pub type Rgb = [f32; 3];

const INK: Rgb = [56.0, 42.0, 32.0];
const SEA_INK: Rgb = [40.0, 66.0, 82.0];
const PAPER: Rgb = [234.0, 222.0, 196.0];
const OFF_MAP: u32 = 0x002A_2420;

/// Light comes from the top left: shadows fall to the bottom right.
const SHADOW: (f32, f32) = (0.45, 0.55);
/// How far (cells) a crown can reach from its tree's cell.
const CROWN_REACH: i64 = 2;

#[inline(always)]
fn hash(x: i64, y: i64, salt: u64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}
#[inline(always)]
fn unit(x: i64, y: i64, salt: u64) -> f32 { (hash(x, y, salt) >> 40) as f32 / (1u64 << 24) as f32 }

/// Smooth value noise in cell units (for watercolour mottling).
#[inline(always)]
fn mottle(x: f32, y: f32, scale: f32, salt: u64) -> f32 {
    let (x, y) = (x / scale, y / scale);
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(tx), s(ty));
    let a = unit(x0, y0, salt) * (1.0 - sx) + unit(x0 + 1, y0, salt) * sx;
    let b = unit(x0, y0 + 1, salt) * (1.0 - sx) + unit(x0 + 1, y0 + 1, salt) * sx;
    a * (1.0 - sy) + b * sy
}

#[inline(always)]
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }
#[inline(always)]
fn pack(c: Rgb) -> u32 {
    let q = |v: f32| v.clamp(0.0, 255.0) as u32;
    (q(c[0]) << 16) | (q(c[1]) << 8) | q(c[2])
}

/// The ground wash of a surface cell.
fn wash(c: &crate::local::Cell) -> Rgb {
    let grassy = matches!(c.plant, Plant::Grass | Plant::Shrub | Plant::Tree(_));
    match c.material {
        Material::Soil | Material::Clay if grassy => [170.0, 174.0, 120.0],
        Material::Soil => [182.0, 160.0, 120.0],
        Material::Clay => [186.0, 150.0, 118.0],
        Material::Sand => [226.0, 206.0, 156.0],
        Material::Gravel => [190.0, 182.0, 160.0],
        Material::Snow => [240.0, 238.0, 230.0],
        Material::Ice => [214.0, 226.0, 228.0],
        // Each stone its own wash, so the strata read as layers in a level or the section.
        Material::Rock(r) => rock_wash(r),
        Material::Ore(_) => [178.0, 168.0, 148.0],
        Material::Block(_) => [196.0, 188.0, 170.0],
        Material::Wood => [176.0, 140.0, 100.0],
        Material::Air => [182.0, 160.0, 120.0],
        Material::Magma => [214.0, 86.0, 30.0],
    }
}

/// A stone's wash: granite a pinkish grey, basalt dark slate, sandstone ochre, limestone cream,
/// shale a blue-grey, loose sediment buff.
fn rock_wash(r: crate::erosion::materials::RockType) -> Rgb {
    use crate::erosion::materials::RockType as R;
    match r {
        R::Granite => [184.0, 164.0, 156.0],
        R::Basalt => [128.0, 128.0, 134.0],
        R::Sandstone => [204.0, 172.0, 120.0],
        R::Limestone => [214.0, 206.0, 178.0],
        R::Shale => [148.0, 154.0, 164.0],
        R::Sediment => [190.0, 170.0, 136.0],
        R::Ice => [214.0, 226.0, 228.0],
    }
}

/// Crown radius (cells, 2 m each) and wash. A broadleaf crown is ~4 m across, a fir ~3 m:
/// drawn flat like the world map's tree symbols, so neighbours touch rather than pile up.
fn crown(kind: TreeKind) -> (f32, Rgb) {
    match kind {
        TreeKind::Broadleaf => (0.95, [132.0, 150.0, 92.0]),
        TreeKind::Conifer => (0.75, [112.0, 134.0, 96.0]),
        TreeKind::Jungle => (1.1, [120.0, 146.0, 84.0]),
        TreeKind::Palm => (0.85, [128.0, 152.0, 90.0]),
        TreeKind::Acacia => (1.05, [154.0, 154.0, 94.0]),
        TreeKind::Dead => (0.55, [150.0, 132.0, 108.0]),
        // A fungus cap: wide and pale, mauve-grey.
        TreeKind::Fungus => (0.9, [156.0, 128.0, 150.0]),
    }
}

/// `(0..n).map(f).collect()`, in parallel when `par` (a view of a few hundred cells is built
/// on this thread: handing it to the pool cost more than the work, and stalled on a busy machine).
fn build<T: Send>(n: usize, par: bool, f: impl Fn(usize) -> T + Sync + Send) -> Vec<T> {
    if par { (0..n).into_par_iter().map(f).collect() } else { (0..n).map(f).collect() }
}

/// What stands on a column, seen from above.
#[derive(Clone, Copy, PartialEq)]
enum Top { Ground, Water(f32), Wall(Material, i32) }

/// A tree's crown, precomputed for its column.
#[derive(Clone, Copy)]
struct Crown { ox: f32, oy: f32, r: f32, col: Rgb, conifer: bool, h: u64 }

/// Every column read once per frame, so pixels only index arrays (a pixel looks at up to 49
/// columns for overlapping crowns).
///
/// (Only a window of the map is read: the cells on screen and a margin, `x0..x1` by `y0..y1`;
/// the whole map each frame had cost ten milliseconds.)
struct View<'a> {
    map: &'a LocalMap,
    x0: usize,
    y0: usize,
    gw: usize,
    gh: usize,
    top: Vec<Top>,
    wash: Vec<Rgb>,
    crowns: Vec<Option<Crown>>,
    /// Which neighbours stand higher (and are not water): west, east, north, south as bits 0-3.
    rises: Vec<u8>,
}

impl<'a> View<'a> {
    #[allow(dead_code)]
    fn new(map: &'a LocalMap) -> Self { Self::window(map, 0, 0, map.width, map.height) }

    /// The view of the cells `x0..x1` by `y0..y1` (clamped to the map).
    fn window(map: &'a LocalMap, x0: usize, y0: usize, x1: usize, y1: usize) -> Self {
        let (x1, y1) = (x1.min(map.width).max(x0 + 1), y1.min(map.height).max(y0 + 1));
        let (gw, gh) = (x1 - x0, y1 - y0);
        let (w, h) = (map.width, map.height);
        // Window index -> map index.
        let mk = move |i: usize| (y0 + i / gw) * w + x0 + i % gw;
        let par = gw * gh > 4000;
        let top: Vec<Top> = build(gw * gh, par, |i| { let k = mk(i); column_top(map, k % w, k / w) });
        let floor_of = |k: usize| {
            let (x, y) = (k % w, k / w);
            map.cell(x, y, map.surface_z[k].clamp(0, map.depth as i32 - 1) as usize)
        };
        // Ground cut two levels or more below its neighbours (a ditch, a trench, a pit's mouth)
        // lies in shadow.
        let sunk = |k: usize| {
            let (x, y) = (k % w, k / w);
            let z = map.surface_z[k];
            let hi = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)].iter()
                .filter(|&&(qx, qy)| qx < w && qy < h).map(|&(qx, qy)| map.surface_z[qy * w + qx]).max().unwrap_or(z);
            hi - z
        };
        // How many levels a building stands over its ground (a storey is two), for the shadow it
        // casts down-light: 0.6 cells a level, so a two-storey house throws one cell of shade, a
        // keep four.
        // (Tall buildings cast shade up to four cells: read four more round the window.)
        let tall_at = |x: usize, y: usize| -> i32 {
            let k = y * w + x;
            let z = map.surface_z[k];
            (1..=10).rev().find(|&dz| {
                let zz = z + dz;
                zz >= 0 && (zz as usize) < map.depth && {
                    let c = map.cell(x, y, zz as usize);
                    c.shape != Shape::Empty && matches!(c.material, Material::Wood | Material::Block(_) | Material::Clay)
                }
            }).unwrap_or(0)
        };
        let (tx0, ty0) = (x0.saturating_sub(4), y0.saturating_sub(4));
        let (tx1, ty1) = ((x1 + 4).min(w), (y1 + 4).min(h));
        let tw = tx1 - tx0;
        let tall: Vec<i32> = build(tw * (ty1 - ty0), par, |i| tall_at(tx0 + i % tw, ty0 + i / tw));
        let tall_of = |x: usize, y: usize| if x >= tx0 && x < tx1 && y >= ty0 && y < ty1 { tall[(y - ty0) * tw + x - tx0] } else { 0 };
        let s = (SHADOW.0 * SHADOW.0 + SHADOW.1 * SHADOW.1).sqrt();
        let shaded = |k: usize| {
            let (x, y) = ((k % w) as f32, (k / w) as f32);
            tall_of(k % w, k / w) == 0 && (1..=4).any(|d| {
                let (qx, qy) = ((x - SHADOW.0 / s * d as f32).round(), (y - SHADOW.1 / s * d as f32).round());
                qx >= 0.0 && qy >= 0.0 && (qx as usize) < w && (qy as usize) < h && tall_of(qx as usize, qy as usize) as f32 * 0.6 >= d as f32
            })
        };
        let wash = build(gw * gh, par, |i| { let k = mk(i); match top[i] {
            Top::Water(d) => water_wash(d),
            _ => {
                let c = wash(floor_of(k));
                let d = sunk(k);
                let c = if d >= 2 { mix(c, [70.0, 60.0, 50.0], (0.18 * d as f32).min(0.5)) } else { c };
                if shaded(k) { mix(c, [70.0, 60.0, 50.0], 0.22) } else { c }
            }
        } });
        let crowns = build(gw * gh, par, |i| {
            let k = mk(i);
            if !matches!(top[i], Top::Ground) { return None; }
            let (tx, ty) = ((k % w) as i64, (k / w) as i64);
            let (r, col, conifer) = match floor_of(k).plant {
                Plant::Tree(kind) => { let (r, col) = crown(kind); (r, col, kind == TreeKind::Conifer) }
                Plant::Shrub => (0.55, [140.0, 154.0, 100.0], false),
                _ => return None,
            };
            // Size and centre jitter per tree, keyed on its cell.
            Some(Crown {
                ox: tx as f32 + 0.5 + 0.3 * (unit(tx, ty, 71) - 0.5),
                oy: ty as f32 + 0.5 + 0.3 * (unit(tx, ty, 72) - 0.5),
                r: r * (0.85 + 0.3 * unit(tx, ty, 70)),
                col, conifer, h: hash(tx, ty, 73),
            })
        });
        let rises = build(gw * gh, par, |i| {
            let k = mk(i);
            let (x, y) = ((k % w) as i64, (k / w) as i64);
            let sz = map.surface_z[k];
            let mut bits = 0u8;
            for (b, (dx, dy)) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)].iter().enumerate() {
                let (qx, qy) = (x + dx, y + dy);
                if qx < 0 || qy < 0 || qx as usize >= w || qy as usize >= h { continue; }
                let q = qy as usize * w + qx as usize;
                if map.surface_z[q] > sz && !matches!(column_top(map, qx as usize, qy as usize), Top::Water(_)) { bits |= 1 << b; }
            }
            bits
        });
        View { map, x0, y0, gw, gh, top, wash, crowns, rises }
    }
    #[inline(always)]
    fn inside(&self, x: i64, y: i64) -> bool { x >= 0 && y >= 0 && (x as usize) < self.map.width && (y as usize) < self.map.height }
    /// The map index of (x, y), clamped to the map.
    #[inline(always)]
    fn idx(&self, x: i64, y: i64) -> usize {
        (y.clamp(0, self.map.height as i64 - 1) as usize) * self.map.width + x.clamp(0, self.map.width as i64 - 1) as usize
    }
    /// The window index of (x, y), clamped to the window.
    #[inline(always)]
    fn widx(&self, x: i64, y: i64) -> usize {
        let wx = (x - self.x0 as i64).clamp(0, self.gw as i64 - 1) as usize;
        let wy = (y - self.y0 as i64).clamp(0, self.gh as i64 - 1) as usize;
        wy * self.gw + wx
    }
    #[inline(always)]
    fn sz(&self, x: i64, y: i64) -> i32 { self.map.surface_z[self.idx(x, y)] }
    #[inline(always)]
    fn floor(&self, x: i64, y: i64) -> &crate::local::Cell {
        let k = self.idx(x, y);
        self.map.cell(k % self.map.width, k / self.map.width, self.map.surface_z[k].clamp(0, self.map.depth as i32 - 1) as usize)
    }
    #[inline(always)]
    fn top(&self, x: i64, y: i64) -> Top { self.top[self.widx(x, y)] }
    #[inline(always)]
    fn ground_wash(&self, x: i64, y: i64) -> Rgb { self.wash[self.widx(x, y)] }
    /// Which neighbours of (x, y) stand higher (bits: west, east, north, south).
    #[inline(always)]
    fn rise(&self, x: i64, y: i64) -> u8 { self.rises[self.widx(x, y)] }
    /// The crown standing in (x, y), none outside the window.
    #[inline(always)]
    fn crown(&self, x: i64, y: i64) -> Option<Crown> {
        let (wx, wy) = (x - self.x0 as i64, y - self.y0 as i64);
        if wx < 0 || wy < 0 || wx >= self.gw as i64 || wy >= self.gh as i64 { return None; }
        self.crowns[wy as usize * self.gw + wx as usize]
    }
}

/// What stands on a column, seen from above.
fn column_top(map: &LocalMap, x: usize, y: usize) -> Top {
    let sz = map.surface_z[y * map.width + x];
    let mut water = 0.0;
    let mut z = sz + 1;
    while (z as usize) < map.depth && map.cell(x, y, z as usize).water > 0 {
        water += map.cell(x, y, z as usize).water as f32 / WATER_FULL as f32;
        z += 1;
    }
    if water > 0.0 { return Top::Water(water); }
    let above = sz + 1;
    if (above as usize) < map.depth {
        let c = map.cell(x, y, above as usize);
        if c.shape == Shape::Wall && matches!(c.material, Material::Wood | Material::Block(_)) {
            let mut h = 1;
            while ((above + h) as usize) < map.depth && map.cell(x, y, (above + h) as usize).shape == Shape::Wall { h += 1; }
            return Top::Wall(c.material, h);
        }
    }
    Top::Ground
}

fn water_wash(levels: f32) -> Rgb {
    let shallow: Rgb = [150.0, 180.0, 176.0];
    let deep: Rgb = [112.0, 146.0, 156.0];
    mix(shallow, deep, ((levels - 0.5) / 3.0).clamp(0.0, 1.0))
}

/// Draw the playable area's surface in ink. Same camera and buffer layout as `render_local`.
pub fn render_local_ink(map: &LocalMap, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    render_ground(map, None, None, cam, buf, w, h);
}

/// The colony's ground: the ink surface with the camp's worn paths on it, kept between frames
/// like the surface (`render_local_ink`). `draw_colony_on_ground` then draws the rest.
pub fn render_colony_ground(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    let worn = (colony.steps.len() == colony.map.width * colony.map.height).then_some(colony.steps.as_slice());
    render_ground(&colony.map, worn, None, cam, buf, w, h);
}

/// `draw_colony` over ground drawn by `render_colony_ground` (the worn paths already on it).
pub fn draw_colony_on_ground(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    let mut placed = Vec::new();
    draw_colony_inner(colony, cam, buf, w, h, history, None, &mut placed, true);
}

/// How worn a column's ground is, in eight steps (0: not worn, under 15 footsteps), and whether
/// it can show wear (no roof or wall on it).
fn worn_level(map: &LocalMap, worn: Option<&[u16]>, x: usize, y: usize) -> u8 {
    let Some(st) = worn else { return 0 };
    let k = y * map.width + x;
    let n = st[k];
    if n < 15 { return 0; }
    let open = map.roofs[k] == 0 && !matches!(map.cell(x, y, (map.surface_z[k] + 1).clamp(0, map.depth as i32 - 1) as usize).shape, Shape::Wall);
    if !open { return 0; }
    1 + ((n as u32 - 15) * 7 / 105).min(7) as u8
}

/// The ground kept between frames; with `level` the slice at that level drawn over it
/// (`level_pixel`), kept likewise.
fn render_ground(map: &LocalMap, worn: Option<&[u16]>, level: Option<i32>, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    let tg0 = std::time::Instant::now();
    render_ground_inner(map, worn, level, cam, buf, w, h);
    if std::env::var("PLANET_TIME_INK").is_ok() {
        let ms = tg0.elapsed().as_secs_f64() * 1000.0;
        if ms > 15.0 { INK_STATS.with(|st| eprintln!("INK slow {:.1} ms: {}", ms, st.borrow())); }
    }
}

thread_local! {
    /// What the last ground drawn did (for PLANET_TIME_INK).
    static INK_STATS: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
}

fn render_ground_inner(map: &LocalMap, worn: Option<&[u16]>, level: Option<i32>, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    let t = cam.tile_px;
    let ts = std::time::Instant::now();
    // The camera on whole pixels: the world pixel at the screen's top-left. Hatching and grain
    // are keyed on world pixels, so a pan is a shift of the last frame and only what comes into
    // view is drawn.
    let ox = (cam.cx * t - w as f32 / 2.0).round() as i64;
    let oy = (cam.cy * t - h as f32 / 2.0).round() as i64;
    let win = ink_window(map, t, ox, oy, w, h);
    let (x0, y0, x1, y1) = win;
    if x1 <= x0 || y1 <= y0 { buf.iter_mut().for_each(|p| *p = OFF_MAP); return; }
    let snap = column_snaps(map, x0, y0, x1, y1, level);
    let gw0 = x1 - x0;
    let wear: Vec<u8> = (0..gw0 * (y1 - y0)).map(|i| worn_level(map, worn, x0 + i % gw0, y0 + i / gw0)).collect();
    let key = InkKey { t: t.to_bits(), w, h, map: map as *const LocalMap as usize, mw: map.width, tile: map.world_tile, level: level.unwrap_or(i32::MIN) };
    INK_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        // (Debug: PLANET_INK_NOCACHE=1 draws every frame afresh, to time and profile that.)
        let usable = std::env::var("PLANET_INK_NOCACHE").is_err() && cache.as_ref().map_or(false, |c| c.key == key && c.buf.len() == w * h && (ox - c.ox).abs() < w as i64 && (oy - c.oy).abs() < h as i64);
        if !usable {
            // A large window is first drawn coarse (a pixel computed for each 2x2 block) and
            // refined over the next frames: a zoom step at 2560x1440 had cost three frames.
            let coarse = w * h >= PREVIEW_PIXELS && std::env::var("PLANET_INK_NOCACHE").is_err() && std::env::var("PLANET_INK_CHECK").is_err();
            INK_STATS.with(|st| *st.borrow_mut() = if coarse { "coarse".into() } else { "full".into() });
            let t0 = std::time::Instant::now();
            if coarse { draw_ink_coarse(map, t, ox, oy, buf, w, h, win, &wear, level); } else { draw_ink(map, t, ox, oy, buf, w, h, win, &wear, level, &|_, _| true); }
            // How fast this machine draws the ground (pixels a millisecond), for the refining.
            let px = if coarse { w * h / 4 } else { w * h } as f64;
            INK_RATE.with(|r| r.set(px / (t0.elapsed().as_secs_f64() * 1000.0).max(0.1)));
            *cache = Some(InkCache { key, ox, oy, win, snap, wear, buf: buf.to_vec(), coarse: vec![coarse; h] });
            return;
        }
        let c = cache.as_mut().unwrap();
        let (dx, dy) = (ox - c.ox, oy - c.oy);
        // Shift the last frame by the pan.
        if dx != 0 || dy != 0 {
            let old = std::mem::take(&mut c.buf);
            let mut moved = vec![OFF_MAP; w * h];
            // (On this thread: a copy is quick, and a parallel one waits on every worker.)
            moved.chunks_mut(w).enumerate().for_each(|(sy, row)| {
                let oy2 = sy as i64 + dy;
                if oy2 < 0 || oy2 >= h as i64 { return; }
                let src = &old[oy2 as usize * w..(oy2 as usize + 1) * w];
                let (a, b) = ((-dx).max(0) as usize, (w as i64 - dx.max(0)) as usize);
                if b > a { row[a..b].copy_from_slice(&src[(a as i64 + dx) as usize..(b as i64 + dx) as usize]); }
            });
            c.buf = moved;
            // The rows still coarse move with the picture; rows come into view are drawn fully.
            let old_coarse = std::mem::take(&mut c.coarse);
            c.coarse = (0..h).map(|sy| { let o = sy as i64 + dy; o >= 0 && o < h as i64 && old_coarse[o as usize] }).collect();
        }
        // Cells changed since (in both windows), and five round each.
        let (gw, gh) = (x1 - x0, y1 - y0);
        let (ox0, oy0, ox1, oy1) = c.win;
        let ogw = ox1 - ox0;
        let mut dirty = vec![false; gw * gh];
        let mut any = false;
        let r = INK_REACH;
        for i in 0..snap.len() {
            let (x, y) = (x0 + i % gw, y0 + i / gw);
            if x < ox0 || x >= ox1 || y < oy0 || y >= oy1 { continue; }
            let j = (y - oy0) * ogw + x - ox0;
            // (Wear reaches only its neighbours: the pixels blend the four nearest cells.)
            let r = if snap[i] != c.snap[j] { snap[i].reach(&c.snap[j]) } else if wear[i] != c.wear[j] { 1 } else { continue };
            any = true;
            let (cx, cy) = ((i % gw) as i64, (i / gw) as i64);
            for ddy in -r..=r { for ddx in -r..=r {
                let (qx, qy) = (cx + ddx, cy + ddy);
                if qx >= 0 && qy >= 0 && (qx as usize) < gw && (qy as usize) < gh { dirty[qy as usize * gw + qx as usize] = true; }
            } }
        }
        let snap_ms = ts.elapsed().as_secs_f64() * 1000.0;
        let ndirty = dirty.iter().filter(|&&d| d).count();
        let td = std::time::Instant::now();
        if any || dx != 0 || dy != 0 {
            // Up to three passes, each reading the view only where it draws (and three cells
            // round): the rows that came into view, the columns that came into view, and the
            // dirty cells. (One view over the whole window for a strip a few pixels wide had made
            // a pan cost ten milliseconds at 2560x1440.)
            let cell_of = |p: i64| ((p as f32 + 0.5) / t).floor() as i64;
            let clamp_win = |a: i64, b: i64, lo: usize, hi: usize| ((a - 3).max(lo as i64).min(hi as i64) as usize, (b + 4).max(lo as i64).min(hi as i64) as usize);
            // Rows exposed (screen rows), then columns exposed (screen columns).
            let rows = if dy > 0 { (h as i64 - dy).max(0) as usize..h } else { 0..((-dy).min(h as i64)) as usize };
            let cols = if dx > 0 { (w as i64 - dx).max(0) as usize..w } else { 0..((-dx).min(w as i64)) as usize };
            if !rows.is_empty() {
                let (r0, r1) = (rows.start, rows.end);
                let (vy0, vy1) = clamp_win(cell_of(oy + r0 as i64), cell_of(oy + r1 as i64 - 1), y0, y1);
                let need = |_: i64, sy: i64| sy >= r0 as i64 && sy < r1 as i64;
                let row_need = |sy: usize| sy >= r0 && sy < r1;
                draw_ink_rows(map, t, ox, oy, &mut c.buf, w, h, win, (x0, vy0, x1, vy1), &wear, level, &need, &row_need, (r1 - r0) * w, 0..w);
            }
            if !cols.is_empty() {
                let (c0, c1) = (cols.start, cols.end);
                let (vx0, vx1) = clamp_win(cell_of(ox + c0 as i64), cell_of(ox + c1 as i64 - 1), x0, x1);
                let (r0, r1) = (rows.start, rows.end);
                let need = |sx: i64, sy: i64| sx >= c0 as i64 && sx < c1 as i64 && !(sy >= r0 as i64 && sy < r1 as i64);
                let row_need = |sy: usize| !(sy >= r0 && sy < r1);
                draw_ink_rows(map, t, ox, oy, &mut c.buf, w, h, win, (vx0, y0, vx1, y1), &wear, level, &need, &row_need, (c1 - c0) * h, c0..c1);
            }
            if any {
                let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0usize, 0usize);
                for (i, &d) in dirty.iter().enumerate() {
                    if !d { continue; }
                    let (x, y) = (x0 + i % gw, y0 + i / gw);
                    bx0 = bx0.min(x); by0 = by0.min(y); bx1 = bx1.max(x + 1); by1 = by1.max(y + 1);
                }
                let view_win = (bx0.saturating_sub(3).max(x0), by0.saturating_sub(3).max(y0), (bx1 + 3).min(x1), (by1 + 3).min(y1));
                let exposed = |sx: i64, sy: i64| { let (px, py) = (sx + dx, sy + dy); px < 0 || py < 0 || px >= w as i64 || py >= h as i64 };
                let need = |sx: i64, sy: i64| -> bool {
                    if exposed(sx, sy) { return false; }
                    let (wx, wy) = (cell_of(ox + sx) - x0 as i64, cell_of(oy + sy) - y0 as i64);
                    wx >= 0 && wy >= 0 && (wx as usize) < gw && (wy as usize) < gh && dirty[wy as usize * gw + wx as usize]
                };
                let dirty_rows: Vec<bool> = (0..gh).map(|r| dirty[r * gw..(r + 1) * gw].iter().any(|&d| d)).collect();
                let row_need = |sy: usize| -> bool {
                    let cy = cell_of(oy + sy as i64) - y0 as i64;
                    cy >= 0 && (cy as usize) < gh && dirty_rows[cy as usize]
                };
                // (Only the screen columns over the dirty cells' box.)
                let sx0 = (((bx0 as f32) * t) as i64 - ox).clamp(0, w as i64) as usize;
                let sx1 = ((((bx1 as f32) * t).ceil()) as i64 - ox + 1).clamp(0, w as i64) as usize;
                draw_ink_rows(map, t, ox, oy, &mut c.buf, w, h, win, view_win, &wear, level, &need, &row_need, ndirty * (t * t) as usize, sx0..sx1);
            }
        }
        // Refine rows still coarse: about six milliseconds' worth a frame (at the rate the coarse
        // frame was drawn), top first.
        if let Some(r0) = c.coarse.iter().position(|&b| b) {
            let n = ((INK_RATE.with(|r| r.get()) * 6.0) as usize / w).max(1);
            let r1 = (r0..h).take(n).take_while(|&r| c.coarse[r]).last().map_or(r0 + 1, |r| r + 1);
            let cell_of = |p: i64| ((p as f32 + 0.5) / t).floor() as i64;
            let vy0 = (cell_of(oy + r0 as i64) - 3).clamp(y0 as i64, y1 as i64) as usize;
            let vy1 = (cell_of(oy + r1 as i64 - 1) + 4).clamp(y0 as i64, y1 as i64) as usize;
            let need = |_: i64, sy: i64| sy >= r0 as i64 && sy < r1 as i64;
            let row_need = |sy: usize| sy >= r0 && sy < r1;
            draw_ink_rows(map, t, ox, oy, &mut c.buf, w, h, win, (x0, vy0, x1, vy1.max(vy0 + 1)), &wear, level, &need, &row_need, (r1 - r0) * w, 0..w);
            for r in r0..r1 { c.coarse[r] = false; }
        }
        INK_STATS.with(|st| *st.borrow_mut() = format!("snap {:.1} ms, dirty cells {} of {}, pan {},{}, draw {:.1} ms", snap_ms, ndirty, gw * gh, dx, dy, td.elapsed().as_secs_f64() * 1000.0));
        c.ox = ox; c.oy = oy; c.win = win; c.snap = snap; c.wear = wear;
        buf.copy_from_slice(&c.buf);
        // (Debug: PLANET_INK_CHECK=1 draws the whole frame afresh and counts pixels that differ.)
        if std::env::var("PLANET_INK_CHECK").is_ok() && !c.coarse.iter().any(|&b| b) {
            let mut fresh = vec![0u32; w * h];
            draw_ink(map, t, ox, oy, &mut fresh, w, h, win, &c.wear, level, &|_, _| true);
            let bad = fresh.iter().zip(buf.iter()).filter(|(a, b)| a != b).count();
            if bad > 0 { eprintln!("INK CHECK: {} pixels differ (pan {},{})", bad, dx, dy); }
        }
    });
}

/// The camera moved onto whole screen pixels (as the surface is drawn: `render_local_ink`), so
/// everything drawn over the ground stays put on it while the view pans.
pub fn snap_camera(cam: &LocalCamera, w: usize, h: usize) -> LocalCamera {
    let t = cam.tile_px;
    let ox = (cam.cx * t - w as f32 / 2.0).round();
    let oy = (cam.cy * t - h as f32 / 2.0).round();
    LocalCamera { cx: (ox + w as f32 / 2.0) / t, cy: (oy + h as f32 / 2.0) / t, ..*cam }
}

/// The cells under the screen whose top-left world pixel is (ox, oy), and a margin.
fn ink_window(map: &LocalMap, t: f32, ox: i64, oy: i64, w: usize, h: usize) -> (usize, usize, usize, usize) {
    let m = INK_MARGIN;
    let x0 = ((ox as f32 / t).floor() as i64 - m).clamp(0, map.width as i64) as usize;
    let y0 = ((oy as f32 / t).floor() as i64 - m).clamp(0, map.height as i64) as usize;
    let x1 = (((ox + w as i64) as f32 / t).ceil() as i64 + m).clamp(0, map.width as i64) as usize;
    let y1 = (((oy + h as i64) as f32 / t).ceil() as i64 + m).clamp(0, map.height as i64) as usize;
    (x0, y0, x1, y1)
}

/// How far (cells) round the screen the view is read, and how far a changed cell's pixels
/// reach (`render_local_ink`).
const INK_MARGIN: i64 = 7;
const INK_REACH: i64 = 5;

/// What the ink surface of a column is drawn from: its ground level, roof and mark, and the
/// cells from just below the ground to ten above it (water, walls, plants, buildings).
#[derive(Clone, PartialEq)]
struct ColumnSnap { sz: i32, roof: u32, feature: crate::local::wildlife::Feature, cells: [crate::local::Cell; 12], lv: [crate::local::Cell; 3] }

impl ColumnSnap {
    /// How far (cells) a change from `old` to this can show: a plant grown or felled (or a mark
    /// on the ground) reaches its crown and the crown's shade (two cells); anything else (a building's walls and the
    /// shade they cast, the ground's level) five (`INK_REACH`). The season's regrowth had
    /// redrawn a third of the screen.
    fn reach(&self, old: &ColumnSnap) -> i64 {
        // (A mark on the ground, a stump or a trail, is drawn inside its own cell.)
        let plants_only = self.sz == old.sz && self.roof == old.roof && self.lv == old.lv
            && self.cells.iter().zip(old.cells.iter()).all(|(a, b)| a.shape == b.shape && a.material == b.material && a.water == b.water && a.boulder == b.boulder);
        if plants_only { CROWN_REACH + 1 } else { INK_REACH }
    }
}

fn column_snaps(map: &LocalMap, x0: usize, y0: usize, x1: usize, y1: usize, level: Option<i32>) -> Vec<ColumnSnap> {
    let gw = x1 - x0;
    // (On this thread: a few thousand columns, and the pool's hand-off had stalled for twenty
    // milliseconds when the machine was busy.)
    (0..gw * (y1 - y0)).map(|i| {
        let (x, y) = (x0 + i % gw, y0 + i / gw);
        let k = y * map.width + x;
        let sz = map.surface_z[k];
        let mut cells = [*map.cell(x, y, 0); 12];
        for (j, c) in cells.iter_mut().enumerate() {
            let z = sz - 1 + j as i32;
            *c = if z >= 0 && (z as usize) < map.depth { *map.cell(x, y, z as usize) } else { *map.cell(x, y, 0) };
        }
        // (A level slice also reads the floor at its level and the two cells over it.)
        let mut lv = [*map.cell(x, y, 0); 3];
        if let Some(z) = level {
            for (j, c) in lv.iter_mut().enumerate() { let zz = z as usize + j; if zz < map.depth { *c = *map.cell(x, y, zz); } }
        }
        ColumnSnap { sz, roof: map.roofs[k], feature: map.features[k], cells, lv }
    }).collect()
}

#[derive(Clone, Copy, PartialEq)]
struct InkKey { t: u32, w: usize, h: usize, map: usize, mw: usize, tile: (usize, usize), level: i32 }

/// The last surface drawn: its zoom and map, where the screen stood (world pixels), the window
/// of columns it was drawn from and their state, its pixels.
struct InkCache { key: InkKey, ox: i64, oy: i64, win: (usize, usize, usize, usize), snap: Vec<ColumnSnap>, wear: Vec<u8>, buf: Vec<u32>,
    /// Screen rows drawn coarse, still to be refined.
    coarse: Vec<bool> }

/// A window this large (pixels) is drawn coarse first, then refined a block of rows a frame.
const PREVIEW_PIXELS: usize = 1_500_000;

thread_local! {
    /// Pixels a millisecond this machine draws the ground at (`render_ground`).
    static INK_RATE: std::cell::Cell<f64> = std::cell::Cell::new(60_000.0);
}

thread_local! {
    static INK_CACHE: std::cell::RefCell<Option<InkCache>> = std::cell::RefCell::new(None);
}

/// Draw the ink surface into `buf` for the screen whose top-left world pixel is (ox, oy): the
/// pixels `need` asks for. Hatching and grain are keyed on world pixels.
#[allow(clippy::too_many_arguments)]
fn draw_ink(map: &LocalMap, t: f32, ox: i64, oy: i64, buf: &mut [u32], w: usize, h: usize, win: (usize, usize, usize, usize), wear: &[u8], level: Option<i32>, need: &(dyn Fn(i64, i64) -> bool + Sync)) {
    draw_ink_rows(map, t, ox, oy, buf, w, h, win, win, wear, level, need, &|_| true, w * h, 0..w);
}

/// `draw_ink` at half resolution: one pixel computed for each 2x2 block (its top-left, exactly
/// as the full image has it), copied to the rest.
#[allow(clippy::too_many_arguments)]
fn draw_ink_coarse(map: &LocalMap, t: f32, ox: i64, oy: i64, buf: &mut [u32], w: usize, h: usize, win: (usize, usize, usize, usize), wear: &[u8], level: Option<i32>) {
    let mut scratch = vec![0u32; w * h];
    let need = |sx: i64, sy: i64| sx % 2 == 0 && sy % 2 == 0;
    let row_need = |sy: usize| sy % 2 == 0;
    draw_ink_rows(map, t, ox, oy, &mut scratch, w, h, win, win, wear, level, &need, &row_need, w * h / 4, 0..w);
    buf.par_chunks_mut(w).enumerate().for_each(|(sy, row)| {
        let src = &scratch[(sy & !1) * w..(sy & !1) * w + w];
        for (sx, out) in row.iter_mut().enumerate() { *out = src[sx & !1]; }
    });
}

/// `draw_ink`, told which screen rows may need anything (`row_need`) and about how many pixels
/// will be drawn: a small redraw (under 150,000) is drawn on this thread, a large one in parallel (a parallel pass over
/// every row for a few dirty cells waited on every worker, and stalled when the machine was busy).
#[allow(clippy::too_many_arguments)]
fn draw_ink_rows(map: &LocalMap, t: f32, ox: i64, oy: i64, buf: &mut [u32], w: usize, h: usize, win: (usize, usize, usize, usize), view_win: (usize, usize, usize, usize), wear: &[u8], level: Option<i32>, need: &(dyn Fn(i64, i64) -> bool + Sync), row_need: &(dyn Fn(usize) -> bool + Sync), est_pixels: usize, cols: std::ops::Range<usize>) {
    let (x0, y0, x1, y1) = win;
    // (The view is read only where pixels are drawn, and three cells round: `view_win`.)
    let v = View::window(map, view_win.0, view_win.1, view_win.2, view_win.3);
    let (gw, gh) = (x1 - x0, y1 - y0);
    let worn_any = wear.iter().any(|&l| l > 0);
    // A worn column's strength (0.55 at fifteen footsteps .. 1.0 at a hundred and twenty).
    let wval = |x: i64, y: i64| -> f32 {
        let (wx, wy) = (x - x0 as i64, y - y0 as i64);
        if wx < 0 || wy < 0 || wx as usize >= gw || wy as usize >= gh { return 0.0; }
        let l = wear[wy as usize * gw + wx as usize];
        if l == 0 { 0.0 } else { 0.55 + 0.45 * (l - 1) as f32 / 7.0 }
    };
    let open = |x: i64, y: i64| -> bool {
        let (wx, wy) = (x - x0 as i64, y - y0 as i64);
        wx >= 0 && wy >= 0 && (wx as usize) < gw && (wy as usize) < gh && {
            let k = y as usize * map.width + x as usize;
            map.roofs[k] == 0 && !matches!(map.cell(x as usize, y as usize, (map.surface_z[k] + 1).clamp(0, map.depth as i32 - 1) as usize).shape, Shape::Wall)
        }
    };
    let dust: Rgb = [164.0, 138.0, 100.0];
    // Ink lines are about a pixel wide whatever the zoom: their width in cells.
    let line = (1.1 / t).max(0.03);
    let draw_row = |(sy, row): (usize, &mut [u32])| {
        if !row_need(sy) { return; }
        let py = oy + sy as i64;
        let fy = (py as f32 + 0.5) / t;
        for (sx, out) in row.iter_mut().enumerate().skip(cols.start).take(cols.end.saturating_sub(cols.start)) {
            if !need(sx as i64, sy as i64) { continue; }
            let px = ox + sx as i64;
            let fx = (px as f32 + 0.5) / t;
            if fx < 0.0 || fy < 0.0 || fx >= map.width as f32 || fy >= map.height as f32 {
                *out = OFF_MAP;
                continue;
            }
            let mut c = pixel(&v, fx, fy, px, py, line, t);
            // The camp's worn paths: a dusty wash where feet have gone often, blended between
            // cells with a ragged edge, never on a roof or a wall (`render_colony_ground`).
            if worn_any && open(fx as i64, fy as i64) {
                let (gx, gy) = (fx - 0.5, fy - 0.5);
                let (cx, cy) = (gx.floor() as i64, gy.floor() as i64);
                let (bx, by) = (gx - cx as f32, gy - cy as f32);
                let f = (wval(cx, cy) * (1.0 - bx) + wval(cx + 1, cy) * bx) * (1.0 - by) + (wval(cx, cy + 1) * (1.0 - bx) + wval(cx + 1, cy + 1) * bx) * by;
                if f > 0.05 {
                    let f = f + 0.3 * (mottle(fx, fy, 1.4, 0x57E) - 0.5);
                    let a = ((f - 0.28) / 0.22).clamp(0.0, 1.0);
                    if a > 0.0 {
                        c = mix(c, dust, a * (0.32 + 0.22 * f.min(1.0)));
                        if unit(px, py, 0x57F) < 0.035 * a { c = mix(c, INK, 0.3); }
                    }
                }
            }
            *out = pack(c);
            if let Some(z) = level { *out = level_pixel(map, z, t, fx, fy, *out); }
        }
    };
    if est_pixels < 150_000 { buf.chunks_mut(w).enumerate().for_each(draw_row); }
    else { buf.par_chunks_mut(w).enumerate().for_each(draw_row); }
}

fn pixel(v: &View, fx: f32, fy: f32, sx: i64, sy: i64, line: f32, t: f32) -> Rgb {
    let (cx, cy) = (fx.floor() as i64, fy.floor() as i64);
    let (u, w) = (fx - cx as f32, fy - cy as f32);
    // Diagonal hatching in screen pixels, so it stays crisp at any zoom.
    let hatch = |n: i64| (sx - sy).rem_euclid(n) == 0;

    // 1. Ground wash, blended between cell centres (bilinear over the four nearest).
    let (gx, gy) = (fx - 0.5, fy - 0.5);
    let (x0, y0) = (gx.floor() as i64, gy.floor() as i64);
    let (bx, by) = (gx - x0 as f32, gy - y0 as f32);
    let blend = |a: Rgb, b: Rgb, s: f32| mix(a, b, s);
    let top_row = blend(v.ground_wash(x0, y0), v.ground_wash(x0 + 1, y0), bx);
    let bot_row = blend(v.ground_wash(x0, y0 + 1), v.ground_wash(x0 + 1, y0 + 1), bx);
    let mut c = blend(top_row, bot_row, by);
    // Watercolour: slow mottling plus paper grain.
    let m = mottle(fx, fy, 9.0, 11) * 0.06 + mottle(fx, fy, 2.3, 12) * 0.04 - 0.05;
    c = [c[0] * (1.0 + m), c[1] * (1.0 + m), c[2] * (1.0 + m)];
    c = mix(c, PAPER, 0.08 + 0.04 * unit(sx, sy, 13));

    let mut top = v.top(cx, cy);
    let floor = *v.floor(cx, cy);
    let sz = v.sz(cx, cy);

    // Shoreline: wet or dry per pixel from a blended wet field (1 in water cells, 0 on land),
    // slightly warped, so banks curve instead of following the cell grid.
    let wet_at = |x: i64, y: i64| if matches!(v.top(x, y), Top::Water(_)) { 1.0f32 } else { 0.0 };
    let (wx, wy) = (fx + 0.35 * (mottle(fx, fy, 3.0, 90) - 0.5), fy + 0.35 * (mottle(fx, fy, 3.0, 91) - 0.5));
    let wet_field = |x: f32, y: f32| {
        let (gx, gy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (gx.floor() as i64, gy.floor() as i64);
        let (bx, by) = (gx - x0 as f32, gy - y0 as f32);
        let a = wet_at(x0, y0) * (1.0 - bx) + wet_at(x0 + 1, y0) * bx;
        let b = wet_at(x0, y0 + 1) * (1.0 - bx) + wet_at(x0 + 1, y0 + 1) * bx;
        a * (1.0 - by) + b * by
    };
    let f = wet_field(wx, wy);
    let mixed = (f > 0.0 && f < 1.0) || matches!(top, Top::Water(_)) != (f >= 0.5);
    if mixed {
        let wet = f >= 0.5;
        if wet && !matches!(top, Top::Water(_)) {
            // The neighbouring water's depth.
            let mut lv = 1.0f32;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] { if let Top::Water(l) = v.top(cx + dx, cy + dy) { lv = l; } }
            top = Top::Water(lv);
        } else if !wet && matches!(top, Top::Water(_)) {
            top = Top::Ground;
        }
        // Ink bank on the 0.5 contour (distance from it in cells, via the field's gradient).
        let e = 0.05;
        let gx = (wet_field(wx + e, wy) - wet_field(wx - e, wy)) / (2.0 * e);
        let gy = (wet_field(wx, wy + e) - wet_field(wx, wy - e)) / (2.0 * e);
        let grad = (gx * gx + gy * gy).sqrt().max(1e-3);
        let d = (f - 0.5).abs() / grad;
        if d < line { return mix(mix(c, water_wash(1.0), 0.5), SEA_INK, 0.75); }
        if wet && d < 0.35 { c = mix(c, PAPER, 0.25 * (1.0 - d / 0.35)); }
    }

    // Roofs: a pitched roof over each standing house, ridge along its long axis.
    let roof = v.map.roofs[(cy as usize) * v.map.width + cx as usize];
    if roof > 0 && !matches!(top, Top::Water(_)) {
        let r = v.map.houses[(roof - 1) as usize];
        let same = |dx: i64, dy: i64| v.inside(cx + dx, cy + dy) && v.map.roofs[((cy + dy) as usize) * v.map.width + (cx + dx) as usize] == roof;
        if r.flat {
            // A keep's or tower's walkable roof: dressed flags, the parapet's merlons (every other
            // cell of its rim) standing up pale with their shadow side hatched, the stair's head
            // as steps, ink round it all.
            let mut k = mix([192.0, 184.0, 168.0], PAPER, 0.08 * (mottle(fx, fy, 1.5, 96) - 0.5));
            if fx.fract() < line || (fy + if (fx.floor() as i64) % 2 == 0 { 0.0 } else { 0.5 }).fract() < line { k = mix(k, INK, 0.2); }
            let (west, east, north, south) = (!same(-1, 0), !same(1, 0), !same(0, -1), !same(0, 1));
            if (west || east || north || south) && (cx + cy) % 2 == 0 {
                k = [214.0, 206.0, 190.0];
                if (u > 0.6 || w > 0.6) && hatch(3) { k = mix(k, INK, 0.5); }
                if edge_distance(u, w, true, true, true, true) < line * 1.2 { k = mix(k, INK, 0.8); }
            }
            let up = v.sz(cx, cy) + 1;
            if (up as usize) < v.map.depth && v.map.cell(cx as usize, cy as usize, up as usize).shape == Shape::Stair && (w * 4.0).fract() < 0.3 && u > 0.15 && u < 0.85 { k = mix(k, INK, 0.5); }
            if edge_distance(u, w, west, east, north, south) < line * 1.4 { k = mix(k, INK, 0.9); }
            return k;
        }
        let (px, py) = (fx - r.cx, fy - r.cy);
        let across = px * -r.axis.1 + py * r.axis.0;
        let tiles: Rgb = if r.stone { [172.0, 98.0, 72.0] } else { [190.0, 164.0, 104.0] };
        let mut k = mix(tiles, PAPER, 0.05 * (mottle(fx, fy, 1.5, 95) - 0.5));
        // Courses parallel to the ridge (tiles or thatch bundles).
        let course = if r.stone { 0.5 } else { 0.33 };
        if (across.abs() / course).fract() < line / course * 0.9 { k = mix(k, INK, 0.3); }
        // The slope facing away from the light (bottom right) is hatched.
        let normal = (-r.axis.1, r.axis.0);
        let away = (normal.0 * SHADOW.0 + normal.1 * SHADOW.1) * across > 0.0;
        if away && hatch(3) { k = mix(k, INK, 0.5); }
        // Ridge line, then the eaves where the roof ends.
        if across.abs() < line * 0.8 { k = mix(k, INK, 0.85); }
        let d = edge_distance(u, w, !same(-1, 0), !same(1, 0), !same(0, -1), !same(0, 1));
        if d < line * 1.4 { k = mix(k, INK, 0.9); }
        return k;
    }

    match top {
        Top::Water(levels) => {
            c = mix(c, water_wash(levels), 0.6);
            if mixed {
                let d = (f - 0.5).abs();
                if d < 0.15 { c = mix(c, PAPER, 0.2); }
            }
            // Ripples: short strokes at hashed spots.
            let (rx, ry) = (cx.div_euclid(3), cy.div_euclid(2));
            let px = rx as f32 * 3.0 + 0.5 + unit(rx, ry, 21) * 2.0;
            let py = ry as f32 * 2.0 + 0.5 + unit(rx, ry, 22);
            let (dx, dy) = (fx - px, fy - py);
            if unit(rx, ry, 23) < 0.45 && dx.abs() < 0.7 && (dy - 0.18 * (dx * 2.2).sin()).abs() < line * 0.7 {
                return mix(c, SEA_INK, 0.45);
            }
            return c;
        }
        Top::Wall(material, _) => {
            let timber = matches!(material, Material::Wood);
            let fill: Rgb = if timber { [168.0, 120.0, 82.0] } else { [200.0, 192.0, 176.0] };
            c = fill;
            // Shadow side of the wall hatched; outline where the wall ends.
            let open = |dx: i64, dy: i64| !matches!(v.top(cx + dx, cy + dy), Top::Wall(..));
            let d = edge_distance(u, w, open(-1, 0), open(1, 0), open(0, -1), open(0, 1));
            if d < line * 1.3 { return mix(c, INK, 0.9); }
            if (open(1, 0) && u > 0.55 || open(0, 1) && w > 0.55) && hatch(3) { c = mix(c, INK, 0.55); }
            if timber && ((fy * 3.0).fract() < line * 3.0) { c = mix(c, INK, 0.25); }
            return c;
        }
        Top::Ground => {}
    }

    // 2. Ground texture by material.
    match floor.material {
        Material::Block(_) if floor.shape != Shape::Wall => {
            // Cobbles: a staggered grid of rounded stones.
            let (qx, qy) = (fx * 2.0 + if (fy * 2.0).floor() as i64 % 2 == 0 { 0.0 } else { 0.5 }, fy * 2.0);
            let (ex, ey) = (qx.fract() - 0.5, qy.fract() - 0.5);
            if (ex.abs() - 0.42).max(ey.abs() - 0.38) > -line * 1.5 { c = mix(c, INK, 0.35); }
        }
        Material::Sand | Material::Gravel => {
            if unit(sx, sy, 31) < if floor.material == Material::Gravel { 0.06 } else { 0.03 } { c = mix(c, INK, 0.3); }
        }
        _ => {}
    }
    if matches!(floor.plant, Plant::Grass) {
        // Grass tufts: a few small ink "v" marks per cell.
        for k in 0..2u64 {
            let (px, py) = (cx as f32 + 0.2 + 0.6 * unit(cx, cy, 40 + k), cy as f32 + 0.25 + 0.6 * unit(cx, cy, 50 + k));
            if unit(cx, cy, 60 + k) < 0.55 {
                let (dx, dy) = (fx - px, py - fy);
                if dy > 0.0 && dy < 0.22 && (dx.abs() - dy * 0.7).abs() < line * 0.8 { c = mix(c, INK, 0.4); }
            }
        }
    }
    if let Plant::Crop(k) = floor.plant {
        // Furrows across the field.
        let rows = if k % 2 == 0 { fy } else { fx };
        if (rows * 2.0).fract() < line * 2.0 { c = mix(c, INK, 0.3); }
        c = mix(c, [204.0, 190.0, 128.0], 0.4);
    }
    match v.map.features[(cy as usize) * v.map.width + cx as usize] {
        Feature::Trail => c = mix(c, [190.0, 166.0, 126.0], 0.55),
        Feature::Burrow | Feature::Den => {
            let (dx, dy) = (u - 0.5, w - 0.5);
            let r = (dx * dx + dy * dy * 2.2).sqrt();
            if r < 0.22 { c = mix(c, INK, 0.75); } else if r < 0.22 + line { c = mix(c, INK, 0.5); }
        }
        Feature::Nest => {
            let r = ((u - 0.5).powi(2) + (w - 0.5).powi(2)).sqrt();
            if (r - 0.2).abs() < line { c = mix(c, INK, 0.6); }
        }
        Feature::Bones => {
            if ((u - w).abs() < line || (u + w - 1.0).abs() < line) && (u - 0.5).abs() < 0.3 { c = mix(c, PAPER, 0.85); }
        }
        Feature::Grave => {
            // A low mound with a cross.
            let r = ((u - 0.5) / 0.32).powi(2) + ((w - 0.6) / 0.2).powi(2);
            if r < 1.0 { c = mix(c, [150.0, 128.0, 96.0], 0.5); }
            if ((u - 0.5).abs() < line && w > 0.15 && w < 0.6) || ((w - 0.3).abs() < line && (u - 0.5).abs() < 0.14) { c = mix(c, INK, 0.85); }
        }
        Feature::Stone => {
            // A tall stone, its shadow side hatched.
            let r = ((u - 0.5) / 0.2).powi(2) + ((w - 0.5) / 0.42).powi(2);
            if r < 1.0 { c = [168.0, 162.0, 150.0]; if u > 0.5 && hatch(3) { c = mix(c, INK, 0.5); } }
            if (r - 1.0).abs() < 0.25 { c = mix(c, INK, 0.8); }
        }
        Feature::Stump => {
            // A cut stump: a ring of bark round pale wood, kept faint (a cleared camp has
            // hundreds; bright, they pocked it with pink dots). Off-centre by its cell, so they
            // do not stand in a grid.
            let (ou, ow) = (0.5 + 0.3 * (unit(cx, cy, 0x5D1) - 0.5), 0.5 + 0.3 * (unit(cx, cy, 0x5D2) - 0.5));
            let r = ((u - ou).powi(2) + (w - ow).powi(2)).sqrt();
            let rs = 0.12;
            if t < 10.0 { if r < rs { c = mix(c, INK, 0.18); } }
            else {
                if r < rs { c = mix(c, [188.0, 164.0, 124.0], 0.45); }
                if (r - rs).abs() < line * 0.8 { c = mix(c, INK, 0.35); }
            }
        }
        Feature::Spring => {
            let r = ((u - 0.5).powi(2) + (w - 0.5).powi(2)).sqrt();
            if (r - 0.18).abs() < line || (r - 0.32).abs() < line * 0.7 { c = mix(c, [60.0, 92.0, 120.0], 0.7); }
        }
        Feature::None => {}
    }
    if floor.boulder {
        let (dx, dy) = ((u - 0.5) / 0.38, (w - 0.55) / 0.3);
        let r = (dx * dx + dy * dy).sqrt();
        if r < 1.0 {
            c = [182.0, 174.0, 160.0];
            if dx + dy > 0.3 && hatch(3) { c = mix(c, INK, 0.5); }
            if r > 1.0 - line * 3.0 { c = mix(c, INK, 0.85); }
        }
    }

    // 3. Steps in the ground: an ink edge along each rise, hachures falling downhill.
    let rise = v.rise(cx, cy);
    let higher = |dx: i64, dy: i64| rise & match (dx, dy) { (-1, 0) => 1, (1, 0) => 2, (0, -1) => 4, _ => 8 } != 0;
    let d = edge_distance(u, w, rise & 1 != 0, rise & 2 != 0, rise & 4 != 0, rise & 8 != 0);
    if d < line { c = mix(c, INK, 0.7); }
    else if d < 0.32 {
        // Short strokes perpendicular to the edge, every few pixels along it.
        let along = if higher(0, -1) || higher(0, 1) { sx } else { sy };
        if along.rem_euclid((t * 0.25).max(3.0) as i64) == 0 { c = mix(c, INK, 0.45 * (1.0 - d / 0.32)); }
    }

    // 4. Shadows cast by crowns, then the crowns themselves (southern ones drawn over northern).
    let mut crown_hit: Option<(f32, Rgb, f32, f32, f32, bool)> = None; // (tree y, colour, dx, dy, radius, conifer)
    // (The hit crown's radius and hash, when its outline was not reckoned.)
    let mut lazy: Option<(f32, u64)> = None;
    let mut shaded = false;
    for ty in cy - CROWN_REACH..=cy + CROWN_REACH {
        for tx in cx - CROWN_REACH..=cx + CROWN_REACH {
            if !v.inside(tx, ty) { continue; }
            let Some(Crown { ox, oy, r, col, conifer, h }) = v.crown(tx, ty) else { continue };
            let (dx, dy) = (fx - ox, fy - oy);
            // Outlines stay within 0.78r..1.1r of the centre: decide by distance where possible.
            let d2 = dx * dx + dy * dy;
            if d2 <= (r * 1.1) * (r * 1.1) && crown_hit.map_or(true, |hit| oy > hit.0) {
                // (Deep inside, the outline's wave cannot matter: it is reckoned only near it.)
                let inner = d2 < (r * 0.75) * (r * 0.75);
                let edge = if inner { -1.0 } else { crown_radius(r, dx, dy, conifer, h) };
                if inner || d2 <= edge * edge {
                    crown_hit = Some((oy, col, dx, dy, if inner { -r - 1.0 - h as f32 * 0.0 } else { edge }, conifer));
                    lazy = if inner { Some((r, h)) } else { None };
                    continue;
                }
            }
            if !shaded && crown_hit.is_none() {
                let (sx2, sy2) = (dx - SHADOW.0 * r * 0.4, dy - SHADOW.1 * r * 0.4);
                if sx2 * sx2 + sy2 * sy2 <= (r * 0.85) * (r * 0.85) { shaded = true; }
            }
        }
    }
    if let Some((_, col, dx, dy, edge, conifer)) = crown_hit {
        let dist = (dx * dx + dy * dy).sqrt();
        // The outline, reckoned now only where it could tell: near the rim (it is never under
        // 0.78 of the radius) or on a fir's needle line.
        let edge = match lazy {
            Some((r, h)) if dist > 0.78 * r - line * 1.3 || (conifer && dx.abs() < line * 0.5 && dist >= 0.5 * r) => crown_radius(r, dx, dy, conifer, h),
            Some((r, _)) => r,
            None => edge,
        };
        // Flat, like a map symbol: a wash, a few leaf flecks (needle ticks on firs), an outline.
        let mut k = mix(col, PAPER, 0.06 * (unit(sx / 3, sy / 3, 80) - 0.5));
        if unit(sx / 2, sy / 2, 81) < 0.07 { k = mix(k, INK, 0.25); }
        if conifer && (dx.abs() < line * 0.5) && dist < edge * 0.7 { k = mix(k, INK, 0.3); }
        if dist > edge - line * 1.2 { k = mix(k, INK, 0.85); }
        return k;
    }
    if shaded { c = mix(c, INK, 0.08); }
    c
}

/// Crown outline at angle of (dx, dy): broadleaf lobed, conifer star-pointed.
fn crown_radius(r: f32, dx: f32, dy: f32, conifer: bool, h: u64) -> f32 {
    let a = dy.atan2(dx) + (h % 628) as f32 / 100.0;
    if conifer {
        // Eight soft points, like a fir seen from above.
        r * (0.78 + 0.22 * (a * 4.0).cos().abs().powf(0.5))
    } else {
        // Scalloped like an inked tree: seven to nine bumps, a slightly uneven outline.
        let bumps = 7.0 + (h % 3) as f32;
        r * (0.9 + 0.1 * (a * bumps / 2.0).sin().abs() + 0.04 * (a * 2.0 + 0.7).sin())
    }
}

/// Distance (cells) from (u, w) inside a cell to the nearest of its flagged sides
/// (west, east, north, south); infinity if none is flagged.
#[inline(always)]
fn edge_distance(u: f32, w: f32, west: bool, east: bool, north: bool, south: bool) -> f32 {
    let mut d = f32::INFINITY;
    if west { d = d.min(u); }
    if east { d = d.min(1.0 - u); }
    if north { d = d.min(w); }
    if south { d = d.min(1.0 - w); }
    d
}

/// Draw a colony's people and things over its surface: the camp's fire and stockpile, logs and
/// food lying about, and each settler as a small inked figure (a dot of colour in an ink ring,
/// what they carry beside them, "z" when asleep), with names when zoomed in.
/// Each settler's (skin, hair, dress) on the map, from their portrait; a dress that would make
/// two living settlers look alike is shifted to the next unused colour.
/// A level slice in ink, so the levels read as the same land as the surface view: the surface is
/// drawn first; where the ground lies at this level it shows as it is; above the ground (open
/// air) the surface below shows as a faint ghost. Below the ground the view is a cut at standing
/// height (DF's level): the rock and soil one would walk into are parchment hatched in their
/// colour (ore flecked in its metal, gem clusters glinting, wet rock of the aquifer stippled
/// blue), each layer its own wash; floors one can stand on (halls, rooms, passages, cavern
/// floors) are a pale wash of their stone, inked where they meet the rock; stairs are drawn as
/// steps with a mark for the way they go (up, down or both); open dark (a cavern's air, a shaft)
/// is dusk-grey; water is blue.
/// Rock or soil cut at standing height (cell `body` at column (x, y), level `zb`), at map point
/// (fx, fy): parchment washed in its stone's colour and hatched, ore flecked in its metal, a gem
/// cluster as a small mark, the aquifer's wet rock tinted blue with a few short ripples (it had
/// been a blue stipple that read as noise over whole levels). `line` is an ink line's width in
/// cells. Shared by the level renderer and the cover over unfound places, so they match.
fn cut_rock(map: &LocalMap, body: &crate::local::Cell, x: usize, y: usize, zb: usize, fx: f32, fy: f32, line: f32) -> Rgb {
    let base = mix(PAPER, wash(body), 0.5);
    let wet = map.is_aquifer(x, y, zb);
    let base = if wet { mix(base, [132.0, 160.0, 176.0], 0.3) } else { base };
    let hatch = ((fx * 6.0 + fy * 6.0) as i64).rem_euclid(3) == 0;
    let mut c = if hatch { mix(base, INK, 0.18) } else { base };
    let (u, v) = (fx.fract(), fy.fract());
    if let Material::Ore(r) = body.material {
        let col = crate::lore::resource_color(r);
        if unit((fx * 5.0) as i64, (fy * 5.0) as i64, 0x0E) < 0.35 { c = mix(c, [col[0] as f32, col[1] as f32, col[2] as f32], 0.85); }
    }
    if crate::local::gem_in(map, x, y, zb).is_some() {
        let d = (u - 0.5).abs() + (v - 0.5).abs();
        if d < 0.13 { c = mix(c, [150.0, 72.0, 116.0], 0.55); } else if d < 0.13 + line { c = mix(c, INK, 0.5); }
    }
    if wet {
        // A short ripple in some cells, keyed on the cell: water standing in the stone.
        let (cx, cy) = (fx.floor() as i64, fy.floor() as i64);
        if unit(cx, cy, 0xA9) < 0.14 {
            let (u0, v0) = (0.2 + 0.4 * unit(cx, cy, 0xAA), 0.25 + 0.5 * unit(cx, cy, 0xAB));
            let du = u - u0;
            if du > 0.0 && du < 0.36 && (v - v0 - 0.05 * (du * 17.0).sin()).abs() < line * 0.7 { c = mix(c, SEA_INK, 0.45); }
        }
    }
    c
}

/// The red mark on a stair cell for the way it goes: up a caret (^), down a vee (v), both an X.
/// (u, v) is the point within the cell. (The single marks had been drawn the wrong way round.)
fn stair_mark(u: f32, v: f32, up: bool, down: bool) -> bool {
    let (cu, cv) = (u - 0.5, v - 0.5);
    let a = cu.abs() * 1.2;
    if a > 0.4 { return false; }
    let near = |target: f32| (cv - target).abs() < 0.07;
    if up && down { (cv < 0.0 && near(-0.05 - a)) || (cv > 0.0 && near(0.05 + a)) }
    else if up { near(-0.38 + a) }
    else if down { near(0.38 - a) }
    else { false }
}

pub fn render_level_ink(map: &LocalMap, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    // (Kept between frames with the ground under it: `render_ground`.)
    let z = cam.z.clamp(0, map.depth as i32 - 2);
    let surface = LocalCamera { surface_view: true, ..*cam };
    render_ground(map, None, Some(z), &surface, buf, w, h);
}

/// A pixel of the level slice at `z` (clamped by the caller) at map point (fx, fy), over the
/// ground pixel `p` drawn there.
#[inline]
fn level_pixel(map: &LocalMap, z: i32, t: f32, fx: f32, fy: f32, p: u32) -> u32 {
    use crate::local::Shape;
    let zu = z as usize;
    let paper: Rgb = [234.0, 222.0, 196.0];
    // What a cell of the slice is: 0 rock at standing height, 1 a floor to stand on, 2 open dark.
    let kind = |x: i64, y: i64| -> Option<u8> {
        if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height { return None; }
        let (x, y) = (x as usize, y as usize);
        if crate::colony::nav::standable(map, x, y, z) { return Some(1); }
        let body = map.cell(x, y, zu + 1);
        Some(if matches!(body.shape, Shape::Empty | Shape::Stair) { 2 } else { 0 })
    };
    let (x, y) = (fx as usize, fy as usize);
    let sz = map.surface_z[y * map.width + x];
    let here: Rgb = [((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32];
    let floor = map.cell(x, y, zu);
    let body = map.cell(x, y, zu + 1);
    let (u, v) = (fx.fract(), fy.fract());
    let e = 1.2 / t;
    let c: Rgb = if z == sz && floor.shape != Shape::Stair {
        // The ground at this level: the surface as drawn.
        here
    } else if z > sz {
        // Above the ground: what was built up to this level (a tower's platform, the walls
        // of a hut or a tower at standing height), else open air with the land faint below.
        let kd = kind(x as i64, y as i64).unwrap_or(2);
        let open_near = |want: u8| (u < e && kind(x as i64 - 1, y as i64) == Some(want)) || (u > 1.0 - e && kind(x as i64 + 1, y as i64) == Some(want))
            || (v < e && kind(x as i64, y as i64 - 1) == Some(want)) || (v > 1.0 - e && kind(x as i64, y as i64 + 1) == Some(want));
        if kd == 1 {
            // A platform built up to this level (a tower's top): planks or flags, a
            // parapet of merlons along its open edges, its stair as steps with the way
            // marked, as on the levels below. (It had been a bare square.)
            let mut c = mix(paper, wash(floor), 0.55);
            let lw = 1.1 / t;
            if matches!(floor.material, Material::Wood) { if (fy * 4.0).fract() < lw * 4.0 { c = mix(c, INK, 0.3); } }
            else if (fx * 2.0).fract() < lw * 2.0 || (fy * 2.0 + if (fx * 2.0) as i64 % 2 == 0 { 0.0 } else { 0.5 }).fract() < lw * 2.0 { c = mix(c, INK, 0.25); }
            let side = |dx: i64, dy: i64| kind(x as i64 + dx, y as i64 + dy) == Some(2);
            let d = edge_distance(u, v, side(-1, 0), side(1, 0), side(0, -1), side(0, 1));
            if d < 0.18 && ((fx + fy) * 3.0).fract() < 0.5 { c = mix(c, INK, 0.45); }
            let (up, down) = (body.shape == Shape::Stair, floor.shape == Shape::Stair);
            if up || down {
                if (v * 5.0).fract() < 0.28 && u > 0.12 && u < 0.88 { c = mix(c, INK, 0.5); }
                if stair_mark(u, v, up, down) { c = [150.0, 40.0, 30.0]; }
            }
            if open_near(2) { INK } else { c }
        } else if kd == 0 {
            let base = mix(wash(body), INK, 0.3);
            if ((fx + fy) * 5.0).fract() < 0.3 { mix(base, INK, 0.3) } else { base }
        } else if floor.water > 0 { mix(water_wash(floor.water as f32), paper, 0.3) }
        else {
            // Ground below this level, seen through the open air: the surface as drawn,
            // paling with the depth (a level down nearly as it is, so the camp's ground
            // and a ditch or a lower bank still read as the same land; from five levels
            // up a faint ghost). Trees reach up through the levels over the ground (DF),
            // and fade with it. (It had been 72% pale at once, with each tree's cell and
            // its neighbours kept darker: square halos round every crown.)
            let d = (z - sz) as f32;
            mix(here, paper, (0.1 + 0.14 * (d - 1.0)).clamp(0.1, 0.72))
        }
    } else if sz - z <= 2 && floor.shape == Shape::Wall && !crate::colony::nav::standable(map, x, y, z) && map.cavern_at(x, y, z + 1).is_none() {
        // Ground a level or two above this one (a slope, the bank of a rise): the surface
        // as drawn, shaded with hachures, inked where it meets this level's ground. (Cut
        // rock hatching here made a gentle slope look like a quarry face.)
        let mut c = mix(here, INK, 0.12 + 0.06 * (sz - z) as f32);
        if ((fx - fy) * 5.0).fract() < 0.18 { c = mix(c, INK, 0.25); }
        let lower = |dx: i64, dy: i64| { let (qx, qy) = (x as i64 + dx, y as i64 + dy); qx >= 0 && qy >= 0 && (qx as usize) < map.width && (qy as usize) < map.height && map.surface_z[qy as usize * map.width + qx as usize] <= z };
        if (u < e && lower(-1, 0)) || (u > 1.0 - e && lower(1, 0)) || (v < e && lower(0, -1)) || (v > 1.0 - e && lower(0, 1)) { c = INK; }
        c
    } else {
        let kd = kind(x as i64, y as i64).unwrap_or(0);
        let edge_to = |want: u8| (u < e && kind(x as i64 - 1, y as i64) == Some(want)) || (u > 1.0 - e && kind(x as i64 + 1, y as i64) == Some(want))
            || (v < e && kind(x as i64, y as i64 - 1) == Some(want)) || (v > 1.0 - e && kind(x as i64, y as i64 + 1) == Some(want));
        if body.material == Material::Magma {
            // The magma sea: a slow orange glow, darker crust in thin veins across it
            // (the section's look; the veins had been blocky dark squares).
            let vein = (mottle(fx * 1.6, fy * 1.6, 1.0, 0x3A8) - 0.5).abs() < 0.03;
            if vein { [140.0, 52.0, 24.0] } else { mix([238.0, 132.0, 36.0], [196.0, 56.0, 22.0], mottle(fx, fy, 1.6, 0x3A7)) }
        } else if body.water > 0 && kd != 0 {
            water_wash(body.water as f32)
        } else if kd == 0 {
            // Rock or soil at standing height, hatched in its colour.
            let c = cut_rock(map, body, x, y, zu + 1, fx, fy, 1.1 / t);
            if edge_to(1) || edge_to(2) { INK } else { c }
        } else if kd == 1 {
            // A floor to stand on: a pale wash of its stone (a cavern's darker), a stair's
            // steps across it.
            let cavern = map.cavern_at(x, y, z + 1).is_some();
            let mut c = if cavern { mix(paper, [96.0, 88.0, 100.0], 0.55) } else { mix(paper, wash(floor), 0.22) };
            if floor.plant == crate::local::Plant::Tree(TreeKind::Fungus) {
                let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
                if d < 0.38 { c = if d > 0.3 { INK } else { [150.0, 110.0, 140.0] }; }
            }
            let up = body.shape == Shape::Stair;
            let down = floor.shape == Shape::Stair;
            if up || down {
                // Steps: bars across the cell, darker toward the way down.
                let bar = ((v * 5.0) as i32).clamp(0, 4);
                let in_bar = (v * 5.0).fract() < 0.28 && u > 0.12 && u < 0.88;
                if in_bar { c = mix(c, INK, 0.45 + 0.1 * bar as f32); }
                // The way: a chevron up (^), down (v), or both (X).
                if stair_mark(u, v, up, down) { c = [150.0, 40.0, 30.0]; }
            }
            if edge_to(0) { c = INK; }
            c
        } else {
            // Open dark: a cavern's air, a shaft, the space over a lower floor.
            let c = mix(paper, [62.0, 54.0, 66.0], 0.75);
            if edge_to(0) { INK } else { c }
        }
    };
    pack(c)
}

pub fn settler_looks(colony: &crate::colony::Colony, history: Option<&crate::history::world_state::WorldHistory>) -> Vec<(Rgb, Rgb, Rgb)> {
    const SPARE: [Rgb; 6] = [[60.0, 120.0, 120.0], [180.0, 90.0, 40.0], [80.0, 80.0, 150.0], [140.0, 140.0, 60.0], [150.0, 70.0, 110.0], [70.0, 70.0, 70.0]];
    let mut out: Vec<(Rgb, Rgb, Rgb)> = Vec::new();
    for s in &colony.settlers {
        let p = super::portraits::of_settler(s, history, None);
        let mut look = (p.skin, p.hair, p.dress);
        let mut k = 0;
        while out.iter().zip(&colony.settlers).any(|(o, t)| t.alive && *o == look) && k < SPARE.len() {
            look.2 = SPARE[k];
            k += 1;
        }
        out.push(look);
    }
    out
}

/// A label's box on screen (x, y, width, height), for keeping labels apart.
type LabelBox = (f32, f32, f32, f32);

/// Whether a box overlaps any placed one (with a few pixels of air between).
fn crowded(placed: &[LabelBox], r: LabelBox) -> bool {
    const AIR: f32 = 3.0;
    placed.iter().any(|q| r.0 < q.0 + q.2 + AIR && q.0 < r.0 + r.2 + AIR && r.1 < q.1 + q.3 + 1.0 && q.1 < r.1 + r.3 + 1.0)
}

/// Letter `text` centred on `x` with its top at `y`, in the map's italic on a parchment halo, and
/// note its box (labels that must show: a roof's count, a place's name).
fn letter(buf: &mut [u32], w: usize, h: usize, placed: &mut Vec<LabelBox>, x: f32, y: f32, text: &str, face: super::fonts::Face, px: f32, spacing: f32, ink: u32) {
    let tw = super::fonts::width(text, face, px, spacing);
    placed.push((x - tw / 2.0, y, tw, px + 1.0));
    super::fonts::draw(buf, w, h, x - tw / 2.0, y, text, face, px, spacing, ink, Some(0x00EE_E4CC));
}

/// Whether a level slice at `z` shows the surface at column (x, y) (the ground at this level or
/// below it, or a bank a level or two above), as `render_level_ink` draws it; elsewhere it shows
/// the cut through rock, where the camp's ground marks, night and winter do not reach.
fn level_shows_surface(map: &LocalMap, x: usize, y: usize, z: i32) -> bool {
    let sz = map.surface_z[y * map.width + x];
    let zu = z.clamp(0, map.depth as i32 - 2) as usize;
    if z >= sz { return !(z == sz && map.cell(x, y, zu).shape == Shape::Stair); }
    sz - z <= 2 && map.cell(x, y, zu).shape == Shape::Wall && !crate::colony::nav::standable(map, x, y, z) && map.cavern_at(x, y, z + 1).is_none()
}

/// A disc of `fill` in a ring of `ring`, centred at (cx, cy) on screen.
fn ink_disc(put: &mut dyn FnMut(i64, i64, Rgb, f32), cx: f32, cy: f32, r: f32, fill: Rgb, ring: Rgb) {
    let rr = r.ceil() as i64 + 1;
    for dy in -rr..=rr {
        for dx in -rr..=rr {
            let d = ((dx as f32 + 0.5 - (cx.fract())).powi(2) + (dy as f32 + 0.5 - (cy.fract())).powi(2)).sqrt();
            let (x, y) = (cx.floor() as i64 + dx, cy.floor() as i64 + dy);
            if d <= r - 1.0 { put(x, y, fill, 1.0); } else if d <= r + 0.3 { put(x, y, ring, 0.95); }
        }
    }
}

/// Settler `i` as an inked head-and-shoulders figure standing at (x, y) on screen, in their
/// portrait's colours (`look`: skin, hair, dress; greyed when ill), a gold mark on the shoulder
/// for a camp role, and a pictogram beside the head for what they are doing (axe, pick, hammer,
/// basket, rod, a spear for the night's watch or the hunt, a z for sleep, the item carried).
/// The surface view and the level view draw the same figure. Returns the top of the head.
fn draw_figure(put: &mut dyn FnMut(i64, i64, Rgb, f32), colony: &crate::colony::Colony, i: usize, x: f32, y: f32, scale: f32, look: (Rgb, Rgb, Rgb)) -> f32 {
    use crate::colony::{ItemKind, Job};
    let s = &colony.settlers[i];
    // Children are smaller (an infant about half a grown figure).
    let age = s.past.as_ref().map_or(30, |p| p.age);
    let scale = scale * match age { 0..=2 => 0.5, 3..=7 => 0.65, 8..=11 => 0.78, 12..=15 => 0.9, _ => 1.0 };
    let (skin, hair, dress) = look;
    let ill = s.ill_until > colony.clock.tick;
    let dress = if ill { mix(dress, [150.0, 150.0, 140.0], 0.5) } else { dress };
    // Clothes wear out (`clothes.rs`): faded and patched from 120 days, rags from 180.
    let worn = if s.guest_until == 0 { colony.clothes_worn(i) } else { 0 };
    let dress = if worn >= 120 { mix(dress, [168.0, 160.0, 146.0], 0.3) } else { dress };
    // Shoulders: a half ellipse; the head above, hair on its crown.
    let (sw, sh) = (7.0 * scale, 6.0 * scale);
    let base = y + 5.0 * scale;
    for dy in -(sh as i64)..=0 {
        for dx in -(sw as i64 + 1)..=(sw as i64 + 1) {
            let e = (dx as f32 / sw).powi(2) + (dy as f32 / sh).powi(2);
            if e <= 1.0 { put(x as i64 + dx, base as i64 + dy, if e > 0.72 { INK } else { dress }, 0.97); }
        }
    }
    if worn >= 120 {
        // Patches of another cloth, and a ragged hem when they are rags.
        let patch = mix(dress, [120.0, 96.0, 70.0], 0.5);
        for (px, py) in [(-0.4f32, -0.35f32), (0.35, -0.55)] {
            let (cx, cy) = (x + sw * px, base + sh * py);
            let r = (1.4 * scale).max(1.0) as i64;
            for dy in -r..=r { for dx in -r..=r { put(cx as i64 + dx, cy as i64 + dy, if dx.abs() == r || dy.abs() == r { mix(patch, INK, 0.4) } else { patch }, 0.9); } }
        }
        if worn >= 180 {
            for k in 0..5 {
                let u = -0.8 + k as f32 * 0.4;
                let (tx, ty) = (x + sw * u, base);
                for d in 0..(2.0 * scale).max(1.0) as i64 { put(tx as i64 + d / 2, ty as i64 + 1 + d, INK, 0.8); }
            }
        }
    }
    // Armour worn: mail rings or a leather coat's seams over the dress.
    if let Some(a) = colony.armour.iter().find(|a| a.holder == Some(i)) {
        let leather = a.material.contains("leather") || a.material.contains("fur") || a.material.contains("hide");
        let c = if leather { [128.0, 92.0, 60.0] } else { super::glyphs::metal_colour(&a.material) };
        for dy in -(sh as i64) + 1..=0 {
            for dx in -(sw as i64)..=(sw as i64) {
                let e = (dx as f32 / sw).powi(2) + (dy as f32 / sh).powi(2);
                if e > 0.62 { continue; }
                let ring = if leather { dy % 3 == 0 } else { (dx + dy * 2).rem_euclid(3) == 0 };
                put(x as i64 + dx, base as i64 + dy, if ring { mix(c, INK, 0.45) } else { c }, 0.75);
            }
        }
    }
    let hr = 4.2 * scale;
    let (hx, hy) = (x, base - sh - hr * 0.7);
    let rr = hr.ceil() as i64 + 1;
    for dy in -rr..=rr {
        for dx in -rr..=rr {
            let d = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt();
            if d > hr + 0.4 { continue; }
            let c = if d > hr - 1.0 { INK } else if (dy as f32) < -hr * 0.25 { hair } else { skin };
            put(hx as i64 + dx, hy as i64 + dy, c, 0.97);
        }
    }
    // The camp's builder, forager, ...: a small gold mark on the shoulder.
    // (Its role's glyph: berries, a fish, an axe, a pack, a hammer, on a gold roundel.)
    if let Some(r) = s.role {
        let (bx, by) = (x - sw * 0.55, base - sh * 0.55);
        ink_disc(put, bx, by, (2.6 * scale).max(2.0), [214.0, 186.0, 110.0], INK);
        let g = [super::glyphs::Glyph::Berries, super::glyphs::Glyph::Fish, super::glyphs::Glyph::Axe, super::glyphs::Glyph::Provisions, super::glyphs::Glyph::Mace][r.min(4)];
        if scale >= 0.8 { super::glyphs::draw(put, g, bx, by, (4.6 * scale).max(5.0), None); }
    }
    // A bandage, an office's headgear, a visitor's hat; and a bubble with what they are going
    // through (`status_ink`).
    super::status_ink::figure_marks(put, colony, i, hx, hy, hr);
    if colony.stocks.map_or(false, |(who, until)| who == i && until > colony.clock.tick) { super::fx_ink::draw_stocks(put, x, base - sh * 0.9, scale); }
    if let Some(e) = super::status_ink::emblem_of(colony, i) {
        super::status_ink::draw_bubble(put, e, x - sw - 2.0 * scale, hy - hr - 1.0, scale * 0.85);
    }
    // What they are doing, beside the head.
    let watch = colony.watcher == Some(i) && colony.clock.is_night();
    let (gx, gy) = (x + sw + 2.0, hy - hr);
    let g = (5.0 * scale).max(3.0);
    let line = |put: &mut dyn FnMut(i64, i64, Rgb, f32), x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb| {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0) as i32 + 1;
        for k in 0..=n { let f = k as f32 / n as f32; put((x0 + (x1 - x0) * f) as i64, (y0 + (y1 - y0) * f) as i64, c, 0.95); }
    };
    let haft: Rgb = [120.0, 84.0, 50.0];
    let glyph = |put: &mut dyn FnMut(i64, i64, Rgb, f32), gl: super::glyphs::Glyph| super::glyphs::draw(put, gl, gx + g * 0.6, gy + g * 0.6, (g * 1.7).max(7.0), None);
    use super::glyphs::Glyph as Gl;
    match (s.job, s.carrying) {
        _ if watch || matches!((s.job, s.carrying), (Job::Hunt(_), None)) => glyph(put, Gl::Spear),
        (job, Some(kind)) => {
            // What they carry, as its glyph (the load's own kind: berries, a fish, a log...).
            let stuff = match job { Job::Haul(k) => colony.items.get(k).map(|it| it.what), _ => None }.unwrap_or(crate::colony::Stuff::of(kind));
            let _: ItemKind = kind;
            super::glyphs::draw(put, super::glyphs::Glyph::of_stuff(stuff), gx + g * 0.6, gy + g * 0.9, (g * 1.5).max(6.0), None);
        }
        (Job::Fell(_), _) => glyph(put, Gl::Axe),
        // A pick: for the quarry and the dig below.
        (Job::Quarry(_) | Job::Dig(..), _) => glyph(put, Gl::Tool),
        // A hammer: for the builder and the crafter.
        (Job::Build | Job::Craft, _) => glyph(put, Gl::Mace),
        (Job::Forage(_), _) => glyph(put, Gl::Berries),
        (Job::Fish(_), _) => { line(put, gx, gy + g * 1.4, gx + g * 1.1, gy - g * 0.3, haft); line(put, gx + g * 1.1, gy - g * 0.3, gx + g * 1.1, gy + g * 1.2, [60.0, 80.0, 110.0]); super::glyphs::draw(put, Gl::Fish, gx + g * 1.1, gy + g * 1.4, (g * 1.1).max(5.0), None); }
        (Job::Sleep, _) => {
            // A small z.
            line(put, gx, gy, gx + g, gy, INK);
            line(put, gx + g, gy, gx, gy + g, INK);
            line(put, gx, gy + g, gx + g, gy + g, INK);
        }
        // Armed, with the attackers out or on the watch: their spear at hand.
        _ if colony.attackers_out() && colony.arms.iter().any(|a| a.holder == Some(i)) => glyph(put, Gl::Spear),
        _ => {}
    }
    // (Headgear rises above the head: the name goes above it, not over it.)
    let hat = s.office.is_some() || (s.visitor.is_some() && s.guest_until > colony.clock.tick) || colony.stocks.map_or(false, |st| st.0 == i);
    if hat { return hy - hr * 2.4; }
    hy - hr
}

/// One of the store's heaps by the fire: where it lies (in cells), what, and how many.
pub(crate) struct Heap { pub cell: (f32, f32), pub stuff: crate::colony::Stuff, pub count: usize }

/// The store's heaps: one a kind of thing, each kind in its own place round the fire (timber and
/// stone east of it, the kinds of food west), so a heap does not wander as others come and go.
pub(crate) fn store_heaps(colony: &crate::colony::Colony) -> Vec<Heap> {
    use crate::colony::Stuff;
    let slot = |s: Stuff| -> (f32, f32) {
        match s {
            Stuff::Timber => (2.0, 0.0), Stuff::Stone => (2.0, 1.0), Stuff::Berries => (-2.0, 0.0), Stuff::Fish => (-2.0, 1.0),
            Stuff::Meat => (-3.0, 0.0), Stuff::Grain => (-3.0, 1.0), Stuff::Fungus => (-1.0, 1.0), Stuff::Provisions => (-1.0, -1.0),
        }
    };
    // A slot under a roof or in a wall moves to the nearest open cell round the fire not taken
    // by another heap (the store must not lie on a hut's roof).
    let map = &colony.map;
    let open = |x: i32, y: i32| {
        if x < 1 || y < 1 || x as usize >= map.width - 1 || y as usize >= map.height - 1 { return false; }
        let k = y as usize * map.width + x as usize;
        let sz = map.surface_z[k];
        map.roofs[k] == 0 && ((x - colony.camp.0 as i32).abs() > 1 || (y - colony.camp.1 as i32).abs() > 1)
            && (sz + 1 >= map.depth as i32 || map.cell(x as usize, y as usize, (sz + 1) as usize).shape != Shape::Wall)
    };
    let mut taken: Vec<(i32, i32)> = Vec::new();
    let mut out = Vec::new();
    for s in Stuff::ALL {
        let n = colony.items.iter().filter(|it| it.stored && it.what == s).count();
        if n == 0 { continue; }
        let (dx, dy) = slot(s);
        let want = (colony.camp.0 as i32 + dx as i32, colony.camp.1 as i32 + dy as i32);
        let mut best = want;
        if !open(want.0, want.1) || taken.contains(&want) {
            let mut found = None;
            'ring: for r in 1..6i32 {
                let mut ring: Vec<(i32, i32)> = (-r..=r).flat_map(|a| (-r..=r).map(move |b| (a, b))).filter(|&(a, b)| a.abs().max(b.abs()) == r).collect();
                ring.sort_by_key(|&(a, b)| ((want.0 + a - colony.camp.0 as i32).abs() + (want.1 + b - colony.camp.1 as i32).abs(), b, a));
                for (a, b) in ring { let q = (want.0 + a, want.1 + b); if open(q.0, q.1) && !taken.contains(&q) { found = Some(q); break 'ring; } }
            }
            best = found.unwrap_or(want);
        }
        taken.push(best);
        out.push(Heap { cell: (best.0 as f32, best.1 as f32), stuff: s, count: n });
    }
    out
}

/// Things lying about (not in the store, not in someone's arms): (cell, what, how many), one
/// entry per cell and kind.
pub(crate) fn loose_piles(colony: &crate::colony::Colony) -> Vec<((u16, u16), crate::colony::Stuff, usize)> {
    let carried: Vec<usize> = colony.settlers.iter().filter(|s| s.alive && s.carrying.is_some()).filter_map(|s| if let crate::colony::Job::Haul(k) = s.job { Some(k) } else { None }).collect();
    let mut out: Vec<((u16, u16), crate::colony::Stuff, usize)> = Vec::new();
    for (k, it) in colony.items.iter().enumerate() {
        // Stored, carried, or riding in a basket on its way home.
        if it.stored || carried.contains(&k) || (it.reserved && it.at == colony.camp) { continue; }
        match out.iter_mut().find(|e| e.0 == it.at && e.1 == it.what) { Some(e) => e.2 += 1, None => out.push((it.at, it.what, 1)) }
    }
    out
}

pub fn draw_colony(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    let mut placed = Vec::new();
    draw_colony_inner(colony, cam, buf, w, h, history, None, &mut placed, false);
}

/// `draw_colony`, with a mask of the pixels that show the surface (a level view's: the camp's
/// marks, its night and winter are drawn only there) and the labels placed so far.
#[allow(clippy::too_many_arguments)]
fn draw_colony_inner(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>, mask: Option<&[bool]>, placed: &mut Vec<LabelBox>, worn_drawn: bool) {
    use crate::colony::ItemKind;
    let t = cam.tile_px;
    let to_screen = |x: f32, y: f32| ((x - cam.cx) * t + w as f32 / 2.0, (y - cam.cy) * t + h as f32 / 2.0);
    let shows = |k: usize| mask.map_or(true, |m| m[k]);
    // Worn ground first, under everything: a dusty wash where feet have gone often (15+ steps),
    // stronger on the lanes (120+), blended between cells with a ragged edge, so paths read as
    // paths rather than a dot on every cell; never on a roof or a wall.
    let tm0 = std::time::Instant::now();
    if !worn_drawn && colony.steps.len() == colony.map.width * colony.map.height {
        let map = &colony.map;
        let (x0, y0) = ((cam.cx - w as f32 / 2.0 / t - 1.0).max(0.0) as usize, (cam.cy - h as f32 / 2.0 / t - 1.0).max(0.0) as usize);
        let (x1, y1) = (((cam.cx + w as f32 / 2.0 / t) as usize + 2).min(map.width), ((cam.cy + h as f32 / 2.0 / t) as usize + 2).min(map.height));
        if x1 > x0 && y1 > y0 {
            let gw = x1 - x0;
            let bare = |x: usize, y: usize| { let k = y * map.width + x; map.roofs[k] == 0 && !matches!(map.cell(x, y, (map.surface_z[k] + 1).clamp(0, map.depth as i32 - 1) as usize).shape, Shape::Wall) };
            let open: Vec<bool> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| bare(x, y)).collect();
            let val: Vec<f32> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).zip(&open).map(|((x, y), &o)| {
                let n = colony.steps[y * map.width + x];
                if n < 15 || !o { 0.0 } else { 0.55 + 0.45 * ((n as f32 - 15.0) / 105.0).min(1.0) }
            }).collect();
            let inside = |x: i64, y: i64| x >= x0 as i64 && y >= y0 as i64 && x < x1 as i64 && y < y1 as i64;
            let at = |x: i64, y: i64| if inside(x, y) { val[(y as usize - y0) * gw + (x as usize - x0)] } else { 0.0 };
            let is_open = |x: i64, y: i64| inside(x, y) && open[(y as usize - y0) * gw + (x as usize - x0)];
            if val.iter().any(|&v| v > 0.0) {
                let dust: Rgb = [164.0, 138.0, 100.0];
                // The cells with worn ground in or beside them (a pixel blends its four nearest
                // cells): only their pixels are looked at, and rows with none are skipped.
                let gh = y1 - y0;
                let mut touch = vec![false; gw * gh];
                for i in 0..gw * gh {
                    if val[i] <= 0.0 { continue; }
                    let (cx, cy) = ((i % gw) as i64, (i / gw) as i64);
                    for dy in -1..=1i64 { for dx in -1..=1i64 {
                        let (qx, qy) = (cx + dx, cy + dy);
                        if qx >= 0 && qy >= 0 && (qx as usize) < gw && (qy as usize) < gh { touch[qy as usize * gw + qx as usize] = true; }
                    } }
                }
                let row_touched: Vec<bool> = (0..gh).map(|r| touch[r * gw..(r + 1) * gw].iter().any(|&b| b)).collect();
                buf.par_chunks_mut(w).enumerate().for_each(|(sy, row)| {
                    let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
                    if fy < 0.0 || fy >= map.height as f32 { return; }
                    let ry = fy as i64 - y0 as i64;
                    if ry < 0 || ry as usize >= gh || !row_touched[ry as usize] { return; }
                    for sx in 0..w {
                        let k = sy * w + sx;
                        let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
                        if fx < 0.0 || fx >= map.width as f32 { continue; }
                        let rx = fx as i64 - x0 as i64;
                        if rx < 0 || rx as usize >= gw || !touch[ry as usize * gw + rx as usize] { continue; }
                        if !shows(k) { continue; }
                        if !is_open(fx as i64, fy as i64) { continue; }
                        let (gx, gy) = (fx - 0.5, fy - 0.5);
                        let (cx, cy) = (gx.floor() as i64, gy.floor() as i64);
                        let (bx, by) = (gx - cx as f32, gy - cy as f32);
                        let f = (at(cx, cy) * (1.0 - bx) + at(cx + 1, cy) * bx) * (1.0 - by) + (at(cx, cy + 1) * (1.0 - bx) + at(cx + 1, cy + 1) * bx) * by;
                        if f <= 0.05 { continue; }
                        let f = f + 0.3 * (mottle(fx, fy, 1.4, 0x57E) - 0.5);
                        let a = ((f - 0.28) / 0.22).clamp(0.0, 1.0);
                        if a <= 0.0 { continue; }
                        let p = row[sx];
                        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
                        let mut c = mix(old, dust, a * (0.32 + 0.22 * f.min(1.0)));
                        // Grit: a few ink specks on the trodden earth.
                        if unit(sx as i64, sy as i64, 0x57F) < 0.035 * a { c = mix(c, INK, 0.3); }
                        row[sx] = pack(c);
                    }
                });
            }
        }
    }
    let tm1 = std::time::Instant::now();
    // Winter: in a hard winter the land pales toward snow, more in a deep freeze (the embark's
    // season, which the colony lives through; the map itself keeps its annual colours).
    // Night: the map washes toward sea-ink blue, but for a warm glow round the fire and the watch.
    // (One pass for both, row by row in parallel, the glow reckoned only near a light: two
    // serial passes over every pixel had cost up to thirty milliseconds a frame at night.)
    let winter = if colony.hard_winter() { Some(if colony.frozen() { 0.42f32 } else { 0.28 }) } else { None };
    let dark = colony.darkness();
    // (A closure so evil weather and falling snow (`fx_ink`) can go between the snow and the
    // night, as they did when the two were separate passes.)
    let wash = |buf: &mut [u32], winter: Option<f32>, dark: f32| {
        if winter.is_none() && dark <= 0.0 { return; }
        let mut lights: Vec<(f32, f32, f32)> = Vec::new();
        if dark > 0.0 {
            lights.push({ let (x, y) = to_screen(colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 + 0.5); (x, y, 7.0 * t) });
            if let Some(i) = colony.watcher {
                let p = colony.draw_pos(i);
                let (x, y) = to_screen(p.0 + 0.5, p.1 + 0.5);
                lights.push((x, y, 3.0 * t));
            }
        }
        let snow: Rgb = [236.0, 234.0, 226.0];
        let night: Rgb = [34.0, 48.0, 74.0];
        let warm: Rgb = [250.0, 196.0, 120.0];
        // (Whole numbers away from the lights: the snow's strength by brightness from a table,
        // the night's blend fixed. 256ths throughout.)
        let snow_by_lum: Vec<u32> = (0..=765u32).map(|l| (256.0 * winter.unwrap_or(0.0) * (l as f32 / 765.0).powf(0.7)).round() as u32).collect();
        let night_a = (256.0 * dark).round() as u32;
        let (sr, sg, sb) = (snow[0] as u32, snow[1] as u32, snow[2] as u32);
        let (nr, ng, nb) = (night[0] as u32, night[1] as u32, night[2] as u32);
        buf.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let near: Vec<(f32, f32, f32)> = lights.iter().copied().filter(|&(_, ly, r)| (y as f32 - ly).abs() < r).collect();
            for x in 0..w {
                let k = y * w + x;
                if !shows(k) { continue; }
                let p = row[x];
                let lit = near.iter().any(|&(lx, _, r)| (x as f32 - lx).abs() < r);
                if !lit {
                    let (mut r, mut g, mut b) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
                    if winter.is_some() {
                        let a = snow_by_lum[(r + g + b) as usize];
                        r = (r * (256 - a) + sr * a + 128) >> 8; g = (g * (256 - a) + sg * a + 128) >> 8; b = (b * (256 - a) + sb * a + 128) >> 8;
                    }
                    if night_a > 0 {
                        let a = night_a;
                        r = (r * (256 - a) + nr * a + 128) >> 8; g = (g * (256 - a) + ng * a + 128) >> 8; b = (b * (256 - a) + nb * a + 128) >> 8;
                    }
                    row[x] = (p & 0xFF00_0000) | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
                    continue;
                }
                let mut c = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
                if let Some(a) = winter {
                    // Darker ink (lines, buildings' walls) keeps more of itself than the open ground.
                    let lum = (c[0] + c[1] + c[2]) / 765.0;
                    c = mix(c, snow, a * lum.powf(0.7));
                }
                if dark > 0.0 {
                    let mut glow = 0.0f32;
                    for &(lx, ly, r) in &near {
                        let dx = x as f32 - lx;
                        if dx.abs() >= r { continue; }
                        let d = (dx * dx + (y as f32 - ly).powi(2)).sqrt() / r;
                        if d < 1.0 { glow = glow.max((1.0 - d) * (1.0 - d) * (3.0 - 2.0 * (1.0 - d)).min(1.0)); }
                    }
                    c = mix(c, night, dark * (1.0 - glow));
                    if glow > 0.0 { c = mix(c, warm, 0.18 * dark * glow); }
                }
                row[x] = pack(c);
            }
        });
    };
    if colony.evil_weather_over().is_some() || colony.frozen() {
        wash(buf, winter, 0.0);
        super::fx_ink::draw_weather(colony, buf, w, h, mask);
        wash(buf, None, dark);
    } else {
        wash(buf, winter, dark);
    }
    let tm2 = std::time::Instant::now();
    if std::env::var("PLANET_TIME_DRAW").is_ok() { eprintln!("DRAW worn {:.2} washes {:.2}", (tm1 - tm0).as_secs_f64() * 1e3, (tm2 - tm1).as_secs_f64() * 1e3); }
    let masking = std::cell::Cell::new(true);
    let mut put = |x: i64, y: i64, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
        let k = y as usize * w + x as usize;
        if masking.get() && !shows(k) { return; }
        let p = buf[k];
        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
        buf[k] = pack(mix(old, c, a));
    };
    let disc = ink_disc;
    // The camp's fire (`camp_ink`).
    super::camp_ink::draw_fire(colony, cam, &mut put, w, h);
    // Things lying about, each as its own ink glyph (a log, a stone, berries, a fish...), piled
    // where several lie on one cell; the store's heaps by the fire, one heap a kind.
    let mut count_at: Vec<(f32, f32, String)> = Vec::new();
    for (cell, stuff, n) in loose_piles(colony) {
        let (x, y) = to_screen(cell.0 as f32 + 0.5, cell.1 as f32 + 0.5);
        if x < -t || y < -t || x > w as f32 + t || y > h as f32 + t { continue; }
        let g = super::glyphs::Glyph::of_stuff(stuff);
        if n == 1 { super::glyphs::draw(&mut put, g, x, y, (t * 0.7).clamp(6.0, 18.0), None); }
        else { super::glyphs::heap(&mut put, g, n.min(3), x, y, t.max(9.0), None); }
        if n > 3 && t >= 12.0 { count_at.push((x + t * 0.4, y - t * 0.55, n.to_string())); }
    }
    for heap in store_heaps(colony) {
        let (x, y) = to_screen(heap.cell.0 + 0.5, heap.cell.1 + 0.5);
        super::glyphs::heap(&mut put, super::glyphs::Glyph::of_stuff(heap.stuff), heap.count, x, y, t.max(9.0), None);
        if heap.count > 1 && t >= 10.0 { count_at.push((x + t * 0.42, y - t * 0.6, heap.count.to_string())); }
    }
    // The patron's marks: a dashed ring, gold for blessed ground, red and hatched for forbidden.
    for m in &colony.patron.marks {
        let (cx, cy) = to_screen(m.at.0 as f32 + 0.5, m.at.1 as f32 + 0.5);
        let r = (m.radius as f32 + 0.5) * t;
        let col: Rgb = if m.forbidden { [160.0, 40.0, 30.0] } else { [200.0, 160.0, 60.0] };
        let steps = (r * 6.3) as i64 + 8;
        for k in 0..steps {
            if k % 8 >= 5 { continue; }
            let a = k as f32 / steps as f32 * std::f32::consts::TAU;
            for o in [0.0, 1.0] { put((cx + (r - o) * a.cos()) as i64, (cy + (r - o) * a.sin()) as i64, col, 0.95); }
        }
        if m.forbidden {
            let ri = r as i64;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if ((dx + dy).rem_euclid(9)) == 0 && ((dx * dx + dy * dy) as f32) < r * r { put(cx as i64 + dx, cy as i64 + dy, col, 0.35); }
                }
            }
        }
    }
    // Founding stones: a hall stone (a square block), a grove stone (a green dashed ring round
    // the kept trees); the shrine is drawn as the map's standing stone.
    for (kind, at) in &colony.stones {
        let (cx, cy) = to_screen(at.0 as f32 + 0.5, at.1 as f32 + 0.5);
        match kind {
            crate::colony::StoneKind::Hall => super::camp_ink::draw_hall_stone(&mut put, cam, w, h, *at),
            crate::colony::StoneKind::Grove => {
                let r = (crate::colony::GROVE_RADIUS as f32 + 0.5) * t;
                let steps = (r * 6.3) as i64 + 8;
                for k in 0..steps {
                    if k % 6 >= 4 { continue; }
                    let a = k as f32 / steps as f32 * std::f32::consts::TAU;
                    for o in [0.0, 1.0, 2.0] { put((cx + (r - o) * a.cos()) as i64, (cy + (r - o) * a.sin()) as i64, INK, 0.8); }
                }
                super::camp_ink::draw_standing_stone(&mut put, cam, w, h, *at, 0.9);
            }
            crate::colony::StoneKind::Shrine => {
                // A ring round the standing stone.
                for k in 0..64 {
                    let a = k as f32 / 64.0 * std::f32::consts::TAU;
                    let r = (t * 1.2).max(6.0);
                    for o in [0.0, 1.0] { put((cx + (r - o) * a.cos()) as i64, (cy + (r - o) * a.sin()) as i64, INK, 0.85); }
                }
            }
        }
    }
    // The colony's marks: graves (a mound and a cross) and raised stones.
    for m in &colony.marks {
        // A mark a later building stands over is inside it (the hover still names it).
        if colony.map.roofs[m.at.1 as usize * colony.map.width + m.at.0 as usize] != 0 { continue; }
        let (cx, cy) = to_screen(m.at.0 as f32 + 0.5, m.at.1 as f32 + 0.5);
        let r = (t * 0.7).max(5.0);
        match m.kind {
            crate::colony::MarkKind::Grave => super::camp_ink::draw_grave(&mut put, cam, w, h, m.at, m.title.starts_with("An old"), colony.burned.contains(&m.at)),
            crate::colony::MarkKind::Cage => {
                // A cage standing at the gate, with what it caught inside (`camp_ink::draw_cage`).
                let held = m.title.strip_prefix("The cage of ");
                super::camp_ink::draw_cage(&mut put, cam, w, h, m.at, held, 1.0, colony);
            }
            crate::colony::MarkKind::Scorch => super::camp_ink::draw_scorch(&mut put, cam, w, h, m.at),
            crate::colony::MarkKind::Stone => super::camp_ink::draw_stone_mark(&mut put, cam, w, h, m.at, &m.title),
            crate::colony::MarkKind::Cairn | crate::colony::MarkKind::Bench | crate::colony::MarkKind::Carving => super::camp_ink::draw_haunt(&mut put, cam, w, h, m.at, m.kind),
        }
    }
    // Engravings on the hall's walls: a carved panel on the face toward the floor, a little
    // figure scratched in it (`colony::engrave`).
    for e in colony.engravings.iter().filter(|e| e.z == colony.hall_z && colony.hall_cells.contains(&e.from)) {
        super::camp_ink::draw_engraving(&mut put, cam, w, h, e);
    }
    // Dug rooms under rock, cut away: the rock above hatched dark, the room's edge inked.
    {
        let (x0, y0) = ((cam.cx - w as f32 / 2.0 / t).max(0.0) as usize, (cam.cy - h as f32 / 2.0 / t).max(0.0) as usize);
        let (x1, y1) = (((cam.cx + w as f32 / 2.0 / t) as usize + 1).min(colony.map.width), ((cam.cy + h as f32 / 2.0 / t) as usize + 1).min(colony.map.height));
        for cy in y0..y1 { for cx in x0..x1 {
            let p = (cx as u16, cy as u16);
            if !colony.hall_cells.contains(&p) { continue; }
            let (px, py) = to_screen(cx as f32, cy as f32);
            let open = |dx: i32, dy: i32| { let q = ((cx as i32 + dx) as u16, (cy as i32 + dy) as u16); !colony.hall_cells.contains(&q) };
            for yy in py as i64..(py + t).ceil() as i64 { for xx in px as i64..(px + t).ceil() as i64 {
                let (u, v) = ((xx as f32 - px) / t, (yy as f32 - py) / t);
                let edge = (u < 0.12 && open(-1, 0)) || (u > 0.88 && open(1, 0)) || (v < 0.12 && open(0, -1)) || (v > 0.88 && open(0, 1));
                let hatch = (xx + yy).rem_euclid(5) == 0;
                put(xx, yy, if edge { INK } else if hatch { [70.0, 60.0, 52.0] } else { [120.0, 108.0, 94.0] }, if edge { 0.95 } else { 0.6 });
            } }
        } }
    }
    // The delve's mouth: a stair going down into the dark, framed in ink.
    if let Some(m) = colony.delve_mouth.filter(|_| colony.spine.is_some()) {
        let sp = colony.spine.unwrap();
        let at = if colony.map.cell(sp.at.0 as usize, sp.at.1 as usize, colony.map.surface_z[sp.at.1 as usize * colony.map.width + sp.at.0 as usize].max(0) as usize).shape == crate::local::Shape::Stair { sp.at } else { m };
        let (px, py) = to_screen(at.0 as f32, at.1 as f32);
        for yy in py as i64..(py + t).ceil() as i64 { for xx in px as i64..(px + t).ceil() as i64 {
            let (u, v) = ((xx as f32 - px) / t, (yy as f32 - py) / t);
            let edge = u < 0.1 || u > 0.9 || v < 0.1 || v > 0.9;
            let step = (v * 4.0).fract() < 0.3;
            put(xx, yy, if edge { INK } else if step { mix([60.0, 52.0, 48.0], INK, v) } else { mix([120.0, 108.0, 94.0], [40.0, 34.0, 32.0], v) }, 0.95);
        } }
    }
    if let Some(m) = colony.delve_mouth.filter(|_| colony.spine.is_some() && colony.projects.iter().any(|p| matches!(p.kind, crate::colony::projects::ProjectKind::Mine | crate::colony::projects::ProjectKind::DeepShaft))) {
        let sp = colony.spine.unwrap();
        let at = if colony.map.cell(sp.at.0 as usize, sp.at.1 as usize, colony.map.surface_z[sp.at.1 as usize * colony.map.width + sp.at.0 as usize].max(0) as usize).shape == crate::local::Shape::Stair { sp.at } else { m };
        super::camp_ink::draw_headframe(&mut put, cam, w, h, at);
    }
    // Drawbridges over the ditch: let down, a timber deck with its planks across the way and an
    // inked rail along each side; raised, the leaf stands hatched at the camp's side of the ditch
    // and the crossing is the ditch's dark.
    if !colony.bridges.is_empty() {
        let is_bridge = |q: (i32, i32)| colony.bridges.iter().any(|b| b.0 .0 as i32 == q.0 && b.0 .1 as i32 == q.1);
        let timber: Rgb = [172.0, 128.0, 84.0];
        for &(p, _) in &colony.bridges {
            // Across the ditch east-west, or north-south.
            let ew = (p.0 as i32 - colony.camp.0 as i32).abs() > (p.1 as i32 - colony.camp.1 as i32).abs();
            let (px, py) = to_screen(p.0 as f32, p.1 as f32);
            if colony.bridges_up {
                // The raised leaf: a narrow hatched board on the inner bank.
                let inward = if ew { (colony.camp.0 as f32 - p.0 as f32).signum() } else { (colony.camp.1 as f32 - p.1 as f32).signum() };
                for yy in py as i64..(py + t).ceil() as i64 { for xx in px as i64..(px + t).ceil() as i64 {
                    let (u, v) = ((xx as f32 - px) / t, (yy as f32 - py) / t);
                    let a = if ew { if inward > 0.0 { u } else { 1.0 - u } } else if inward > 0.0 { v } else { 1.0 - v };
                    if a < 0.72 { continue; }
                    let edge = a < 0.76 || a > 0.95;
                    let hatch = (xx - yy).rem_euclid(3) == 0;
                    put(xx, yy, if edge { INK } else if hatch { mix(timber, INK, 0.5) } else { timber }, 0.95);
                } }
                continue;
            }
            for yy in py as i64..(py + t).ceil() as i64 { for xx in px as i64..(px + t).ceil() as i64 {
                let (u, v) = ((xx as f32 - px) / t, (yy as f32 - py) / t);
                // Along the way (a) and across it (b).
                let (a, b) = if ew { (u, v) } else { (v, u) };
                let side_lo = !is_bridge(if ew { (p.0 as i32, p.1 as i32 - 1) } else { (p.0 as i32 - 1, p.1 as i32) });
                let side_hi = !is_bridge(if ew { (p.0 as i32, p.1 as i32 + 1) } else { (p.0 as i32 + 1, p.1 as i32) });
                let rail = (side_lo && b < 0.14) || (side_hi && b > 0.86);
                let plank = (a * 4.0).fract() < 0.14;
                let c = if rail { INK } else if plank { mix(timber, INK, 0.45) } else { mix(timber, PAPER, 0.08 * unit(xx / 3, yy / 3, 0xB71)) };
                put(xx, yy, c, if rail { 0.9 } else { 0.92 });
            } }
        }
    }
    // The works as what they are (`camp_ink`): the palisade's stakes, the woodpile, the well,
    // racks and fences, chimneys, bell-cote, banners and signboards.
    super::camp_ink::draw_palisade(colony, cam, &mut put, w, h);
    super::camp_ink::draw_works(colony, cam, &mut put, w, h);
    super::fx_ink::draw_siege(colony, cam, &mut put, w, h);
    super::camp_ink::draw_relic(colony, cam, &mut put, w, h, None);
    super::camp_ink::draw_mandate(colony, cam, &mut put, w, h);
    super::fx_ink::draw_clash(colony, cam, &mut put, w, h);
    // Buildings going up, drawn by the share of loads laid: pegs and a line (a quarter), a
    // timber frame (to three fifths), then walls rising round the ring.
    for (at, bw, bh, share) in colony.rising() {
        let stone = colony.projects.iter().find(|p| p.at == at && !p.done).map_or(colony.hut_material == ItemKind::Stone, |p| p.material == ItemKind::Stone);
        super::camp_ink::draw_rising(&mut put, cam, w, h, at, bw, bh, share, stone);
    }
    // Settlers: inked head-and-shoulders figures in their portrait's colours, with a pictogram
    // for what they are doing. Those asleep under a roof are a count on it.
    let scale = (t / 16.0).clamp(0.55, 1.4);
    let mut under_roof: Vec<((u16, u16), usize)> = Vec::new();
    let mut labels = Vec::new();
    let looks = settler_looks(colony, history);
    let mut below = 0;
    for (i, s) in colony.settlers.iter().enumerate() {
        if !s.alive { continue; }
        // Down in the delve: counted at its mouth, not drawn on the ground above them.
        if colony.below(i) { below += 1; continue; }
        if s.path.is_empty() {
            if let Some(roof) = colony.roof_over(s.pos) {
                match under_roof.iter_mut().find(|r| r.0 == roof) { Some(r) => r.1 += 1, None => under_roof.push((roof, 1)) }
                continue;
            }
        }
        let dp = colony.draw_pos(i);
        let (x, y) = to_screen(dp.0 + 0.5, dp.1 + 0.5);
        masking.set(false);
        let top = draw_figure(&mut put, colony, i, x, y, scale, looks[i]);
        masking.set(true);
        if t >= 6.0 { labels.push((x, top - 2.0, s.name.clone())); }
    }
    // Creatures: the raid's attackers and the night's wolves (those below the ground are drawn
    // on their level, `draw_delve`).
    for c in colony.creatures.iter().filter(|c| !colony.creature_below(c)) {
        let (x, y) = to_screen(c.pos.0 as f32 + 0.5, c.pos.1 as f32 + 0.5);
        draw_creature(&mut put, colony, history, c, x, y, scale, 1.0);
    }
    // Blows where attackers meet settlers, the restless dead walking, the patron's bell.
    masking.set(false);
    super::fx_ink::draw_fights(colony, cam, &mut put, w, h);
    let mut ghost_names = Vec::new();
    super::fx_ink::draw_ghosts(colony, cam, &mut put, w, h, scale, &mut ghost_names);
    super::fx_ink::draw_prisoner(colony, cam, &mut put, w, h, scale, &mut ghost_names);
    super::fx_ink::draw_away_sign(colony, cam, &mut put, w, h, &mut ghost_names);
    super::fx_ink::draw_bell(colony, cam, &mut put, w, h);
    masking.set(true);
    // Labels that must show come first (the attackers' band, the patron's names, a roof's count,
    // the delve's); settlers' names then step aside from them and from each other.
    use super::fonts::Face;
    if let Some(c) = colony.creatures.iter().find(|c| !matches!(c.kind, crate::colony::creatures::CreatureKind::Wolf | crate::colony::creatures::CreatureKind::Game | crate::colony::creatures::CreatureKind::Pet | crate::colony::creatures::CreatureKind::CaveHunter | crate::colony::creatures::CreatureKind::CaveLife) && !c.leaving && !colony.creature_below(c)) {
        let (x, y) = to_screen(c.pos.0 as f32 + 0.5, c.pos.1 as f32 + 0.5);
        // Above its sprite: a beast's full height (flying ones higher), a raider's head.
        let top = if c.kind == crate::colony::creatures::CreatureKind::Beast {
            let look = creature_look(colony, c);
            let px = super::beasts::px_for(&look, scale);
            y + 7.0 * scale - px * if look.flies { 1.25 } else { 0.95 }
        } else if colony.snatched.iter().any(|sn| sn.coming && sn.home.is_none()) { y - 40.0 * scale } else { y - 16.0 * scale };
        letter(buf, w, h, placed, x, top - 16.0, &c.name, Face::Italic, 14.0, 0.0, 0x009A_2A1E);
    }
    for (x, y, n) in ghost_names { letter(buf, w, h, placed, x, y - 12.0, &n, Face::Italic, 12.0, 0.0, 0x0060_7068); }
    // The patron's names: the settlement at its camp, named places where they lie.
    let mut names: Vec<(f32, f32, String, f32)> = colony.place_names.iter().map(|(p, n)| (p.0 as f32 + 0.5, p.1 as f32 + 0.5, n.clone(), 15.0)).collect();
    if let Some(n) = &colony.name { names.push((colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 - 2.5, n.clone(), 22.0)); }
    for (x, y, n, px) in names {
        let (sx, sy) = to_screen(x, y);
        let face = if px > 18.0 { Face::SmallCaps } else { Face::Italic };
        letter(buf, w, h, placed, sx, sy - px, &n, face, px, 1.0, 0x0030_1E14);
    }
    // Under a roof: how many are inside, lettered on it.
    for (roof, n) in under_roof {
        let (x, y) = to_screen(roof.0 as f32 + 0.5, roof.1 as f32 + 0.5);
        letter(buf, w, h, placed, x, y - 8.0, &format!("{} within", n), Face::Italic, 13.0, 0.0, 0x0030_1E14);
    }
    // Below: how many are down in the delve, lettered by its mouth.
    if let (Some(m), true) = (colony.delve_mouth, below > 0) {
        let (x, y) = to_screen(m.0 as f32 + 0.5, m.1 as f32 + 1.6);
        letter(buf, w, h, placed, x, y, &format!("{} below", below), Face::Italic, 13.0, 0.0, 0x0030_1E14);
    }
    // How many lie in each heap: a small number at its shoulder.
    for (x, y, n) in count_at {
        let tw = super::fonts::width(&n, Face::Roman, 11.0, 0.0);
        placed.push((x - 1.0, y, tw + 2.0, 12.0));
        super::fonts::draw(buf, w, h, x, y, &n, Face::Roman, 11.0, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
    }
    // Names in the map's hand, stepping aside (above, below, right, left), or left out when
    // there is no room (hover shows them).
    for (x, y, label) in labels {
        let px = 12.0;
        let lw = super::fonts::width(&label, Face::Italic, px, 0.0);
        let lh = 13.0;
        let spots = [(x - lw / 2.0, y - lh), (x - lw / 2.0, y + 14.0 * scale + 6.0), (x + 9.0 * scale + 6.0, y), (x - 9.0 * scale - 6.0 - lw, y)];
        let Some((lx, ly)) = spots.iter().copied().find(|&(lx, ly)| !crowded(placed, (lx, ly, lw, lh))) else { continue };
        placed.push((lx, ly, lw, lh));
        super::fonts::draw(buf, w, h, lx, ly, &label, Face::Italic, px, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
    }
    // A banner for a moment: four real seconds from when it is first drawn, fading over the last
    // (it had lasted 720 game minutes: a second at 10x, for ever while paused), within three
    // game days of the moment. A parchment card with a double rule, as the other cards.
    if let Some((text, at)) = &colony.banner {
        let alpha = if colony.clock.tick < at + 3 * crate::colony::TICKS_PER_DAY { banner_alpha(text, *at) } else { 0.0 };
        if alpha > 0.0 {
            let px = 26.0;
            let tw = super::fonts::width(text, super::fonts::Face::SmallCaps, px, 2.0);
            let (bx, by, bw, bh) = (w as f32 / 2.0 - tw / 2.0 - 24.0, 18.0f32, tw + 48.0, px * 1.7);
            for y in by as usize..((by + bh) as usize).min(h) {
                for x in bx.max(0.0) as usize..((bx + bw) as usize).min(w) {
                    let k = y * w + x;
                    let edge = y == by as usize || y + 1 == (by + bh) as usize || x == bx.max(0.0) as usize || x + 1 == (bx + bw) as usize;
                    let inner = y == by as usize + 3 || y + 4 == (by + bh) as usize || x == bx.max(0.0) as usize + 3 || x + 4 == (bx + bw) as usize;
                    let c = if edge { 0x0038_2A20 } else if inner { 0x0098_8468 } else { 0x00EA_DEC4 };
                    buf[k] = blend_px(buf[k], c, alpha);
                }
            }
            if alpha >= 0.999 {
                super::fonts::draw(buf, w, h, bx + 24.0, by + 4.0, text, super::fonts::Face::SmallCaps, px, 2.0, 0x009A_2A1E, None);
            } else {
                super::fonts::draw(buf, w, h, bx + 24.0, by + 4.0, text, super::fonts::Face::SmallCaps, px, 2.0, blend_px(0x00EA_DEC4, 0x009A_2A1E, alpha), None);
            }
        }
    }
}

/// How opaque the moment's banner is now: 1 for three real seconds after it is first drawn,
/// fading to 0 over the fourth. Headless renders draw it once, so they show it whole.
fn banner_alpha(text: &str, at: u64) -> f32 {
    use std::sync::Mutex;
    static SEEN: Mutex<Option<(String, u64, std::time::Instant)>> = Mutex::new(None);
    let mut g = match SEEN.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let fresh = !matches!(g.as_ref(), Some((t, a, _)) if t == text && *a == at);
    if fresh { *g = Some((text.to_string(), at, std::time::Instant::now())); }
    let secs = g.as_ref().map(|(_, _, i)| i.elapsed().as_secs_f32()).unwrap_or(0.0);
    (4.0 - secs).clamp(0.0, 1.0)
}

fn blend_px(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| ((a >> s) & 255) as f32 * (1.0 - t) + ((b >> s) & 255) as f32 * t;
    ((ch(16) as u32) << 16) | ((ch(8) as u32) << 8) | ch(0) as u32
}

/// The camp cut open from the side, in ink: the row `row` of the map from `x0` to `x1`, levels
/// from 9 above to 9 below the camp's ground. Rock and soil are parchment hatched in their
/// colour (ore flecked in its metal), open air pale, dug rooms a pale wash with inked walls,
/// water blue; settlers on the row (or a cell beside it) stand at their level as small figures;
/// a depth scale on the left counts levels from the camp's ground.
pub fn render_section_ink(colony: &crate::colony::Colony, row: usize, x0: usize, x1: usize, buf: &mut [u32], w: usize, h: usize) {
    section_ink(colony, row, x0, x1, buf, w, h, None)
}

/// The section down to level `floor` at least (the embark's whole column for
/// `--local-snapshot`'s `_section.png`, which had been the old flat-pixel cross-section).
pub fn render_section_ink_to(colony: &crate::colony::Colony, row: usize, floor: i32, buf: &mut [u32], w: usize, h: usize) {
    section_ink(colony, row, 0, colony.map.width, buf, w, h, Some(floor))
}

fn section_ink(colony: &crate::colony::Colony, row: usize, x0: usize, x1: usize, buf: &mut [u32], w: usize, h: usize, floor: Option<i32>) {
    use crate::local::{Material, Shape};
    let map = &colony.map;
    let paper: Rgb = [234.0, 222.0, 196.0];
    let sky: Rgb = [242.0, 236.0, 220.0];
    let zc = map.surface_z[colony.camp.1 as usize * map.width + colony.camp.0 as usize];
    // Deep enough for what was dug in this row (a mine) and the cavern it broke into.
    let row_c = row.min(map.height - 1);
    let mut deepest = zc;
    for r in colony.rooms.iter().filter(|r| r.cells.iter().any(|c| c.1 as usize == row_c && (c.0 as usize) >= x0 && (c.0 as usize) < x1)) { deepest = deepest.min(r.z); }
    if let Some(sp) = colony.spine.filter(|s| s.at.1 as usize == row_c) { deepest = deepest.min(sp.bottom); }
    for x in x0..x1.min(map.width) {
        if colony.under_rock((x as u16, row_c as u16)) { deepest = deepest.min(map.surface_z[row_c * map.width + x]); }
        for &l in &colony.breached { if let Some((f, _)) = map.cavern_z.get(row_c * map.width + x).map(|c| c[l as usize]) { if f >= 0 { deepest = deepest.min(f as i32); } } }
    }
    // Down to the magma sea when the stair comes within reach of it.
    if let (Some(m), Some(sp)) = (map.magma_top, colony.spine) { if sp.bottom <= m + 3 { deepest = deepest.min(1); } }
    if let Some(f) = floor { deepest = deepest.min(f.max(0)); }
    let (ztop, levels) = (zc + 9, (zc + 9 - deepest + 3).clamp(19, if floor.is_some() { 400 } else { 75 }));
    let margin = 56.0f32;
    let cw = (w as f32 - margin - 12.0) / (x1 - x0).max(1) as f32;
    let ch = (h as f32 - 60.0) / levels as f32;
    let top = 40.0f32;
    for k in buf.iter_mut() { *k = pack(paper); }
    let row = row.min(map.height - 1);
    let open = |x: usize, z: i32| -> bool {
        if z < 0 || z as usize >= map.depth { return z >= 0; }
        !matches!(map.cell(x, row, z as usize).shape, Shape::Wall | Shape::Floor | Shape::Ramp)
    };
    for sy in 0..h {
        let fz = (sy as f32 - top) / ch;
        if fz < 0.0 || fz >= levels as f32 { continue; }
        let z = ztop - fz as i32;
        for sx in 0..w {
            let fx = (sx as f32 - margin) / cw;
            if fx < 0.0 || fx >= (x1 - x0) as f32 { continue; }
            let x = x0 + fx as usize;
            if x >= map.width { continue; }
            let sz = map.surface_z[row * map.width + x];
            let c: Rgb = if z < 0 {
                // Under the embark's last level: the deep rock, dark and hatched (it had been the
                // sky's colour, a pale strip under the magma).
                let base = mix(paper, [96.0, 84.0, 80.0], 0.6);
                if ((sx as i64 + sy as i64) / 2).rem_euclid(4) == 0 { mix(base, INK, 0.3) } else { base }
            } else if z as usize >= map.depth { sky } else {
                let cell = map.cell(x, row, z as usize);
                if matches!(cell.shape, Shape::Wall | Shape::Floor | Shape::Ramp) {
                    let base = mix(paper, wash(cell), if cell.shape == Shape::Wall { 0.5 } else { 0.7 });
                    let hatch = cell.shape == Shape::Wall && ((sx as i64 + sy as i64) / 2).rem_euclid(4) == 0;
                    let mut c = if hatch { mix(base, INK, 0.22) } else { base };
                    if let Material::Ore(r) = cell.material {
                        let col = crate::lore::resource_color(r);
                        if unit(sx as i64 / 3, sy as i64 / 3, 0x5EC) < 0.4 { c = mix(c, [col[0] as f32, col[1] as f32, col[2] as f32], 0.85); }
                    }
                    // Ink where solid meets open.
                    let (u, v) = (fx.fract(), fz.fract());
                    let e = 1.2 / cw.min(ch);
                    if (v < e && open(x, z + 1)) || (u < e && x > 0 && open(x - 1, z)) || (u > 1.0 - e && x + 1 < map.width && open(x + 1, z)) || (v > 1.0 - e && open(x, z - 1)) { c = INK; }
                    c
                } else if cell.shape == Shape::Stair {
                    // A stair in profile: three treads climbing across the level, inked, the
                    // stone under them washed, the air over them pale. (It had been diagonal
                    // stripes, which read as a rope.)
                    let (u, v) = (fx.fract(), fz.fract());
                    let flight = if (ztop - z).rem_euclid(2) == 0 { u } else { 1.0 - u };
                    let tread = 1.0 - ((flight * 3.0).floor() + 1.0) / 3.0;
                    let lw = 1.1 / ch;
                    let riser = (flight * 3.0).fract() < 3.3 / cw && v > tread && v < tread + 1.0 / 3.0 + lw;
                    if (v - tread).abs() < lw || riser { INK }
                    else if v > tread { mix(paper, [176.0, 160.0, 132.0], 0.6) } else { mix(paper, [205.0, 190.0, 160.0], 0.35) }
                } else if cell.material == Material::Magma {
                    // The magma sea: a slow glow mottled in the rock's cells, darker crust in
                    // veins across it (the level view's look; it had been per-pixel noise).
                    let (mx, my) = (fx + x0 as f32, fz);
                    let vein = (mottle(mx * 1.6, my * 2.2, 1.0, 0x3A8) - 0.5).abs() < 0.035;
                    if vein { [140.0, 52.0, 24.0] } else { mix([238.0, 132.0, 36.0], [196.0, 56.0, 22.0], mottle(mx, my, 1.3, 0x3A7)) }
                } else if cell.water > 0 {
                    water_wash(cell.water as f32)
                } else if map.cavern_at(x, row, z).is_some() {
                    // A cavern: dark air, a fungus stalk rising from the floor here and there.
                    let floor = z >= 1 && map.cell(x, row, (z - 1) as usize).shape == Shape::Floor && map.cell(x, row, (z - 1) as usize).plant == crate::local::Plant::Tree(crate::local::TreeKind::Fungus);
                    // (A fungus tree in profile: a pale stalk under a domed cap, inked; it had
                    // been a mauve block.)
                    let (u, v) = (fx.fract(), fz.fract());
                    let dark = mix(paper, [62.0, 54.0, 66.0], 0.82);
                    let cap = ((u - 0.5) / 0.4).powi(2) + ((v - 0.42) / 0.3).powi(2);
                    let lw = 1.2 / ch;
                    if floor && v < 0.42 && cap < 1.0 { if cap > 1.0 - 4.0 * lw { INK } else { [168.0, 124.0, 156.0] } }
                    else if floor && v >= 0.42 && (u - 0.5).abs() < 0.08 { if (u - 0.5).abs() > 0.08 - 1.2 / cw { INK } else { [196.0, 180.0, 170.0] } }
                    else { dark }
                } else if z < sz || (z <= sz + 1 && colony.under_rock((x as u16, row as u16))) {
                    // Dug, under the ground: a pale wash.
                    mix(paper, [205.0, 190.0, 160.0], 0.6)
                } else { sky }
            };
            buf[sy * w + sx] = pack(c);
        }
    }
    // The depth scale: a tick a level, numbered every third from the camp's ground.
    for l in 0..=levels {
        let y = top + l as f32 * ch;
        let d = (ztop - l) - zc;
        let len = if d % 3 == 0 { 12.0 } else { 6.0 };
        for x in (margin - len) as usize..margin as usize { if (y as usize) < h { buf[y as usize * w + x] = pack(INK); } }
        if d % 3 == 0 && l < levels {
            let label = if d == 0 { "0".to_string() } else { format!("{:+}", d) };
            super::fonts::draw(buf, w, h, 6.0, y - 6.0, &label, super::fonts::Face::Roman, 12.0, 0.0, pack(INK), None);
        }
    }
    for y in top as usize..(top + levels as f32 * ch) as usize { if y < h { buf[y * w + margin as usize] = pack(INK); } }
    // Strata named down the left edge: each run of one stone three levels or more, in italic.
    {
        let xs = x0.min(map.width - 1);
        let name = |c: &crate::local::Cell| -> Option<&'static str> {
            if c.shape != Shape::Wall { return None; }
            use crate::erosion::materials::RockType as R;
            match c.material {
                Material::Rock(R::Granite) => Some("granite"), Material::Rock(R::Basalt) => Some("basalt"), Material::Rock(R::Sandstone) => Some("sandstone"),
                Material::Rock(R::Limestone) => Some("limestone"), Material::Rock(R::Shale) => Some("shale"), Material::Rock(R::Sediment) => Some("sediment"),
                Material::Soil | Material::Clay | Material::Sand | Material::Gravel => Some("soil"),
                _ => None,
            }
        };
        let mut l = 0;
        while l < levels {
            let z = ztop - l;
            let here = if z >= 0 && (z as usize) < map.depth { name(map.cell(xs, row, z as usize)) } else { None };
            let mut run = 1;
            while l + run < levels { let z2 = ztop - l - run; if z2 >= 0 && (z2 as usize) < map.depth && name(map.cell(xs, row, z2 as usize)) == here && here.is_some() { run += 1; } else { break; } }
            if let Some(n) = here.filter(|_| run >= 3 && ch * run as f32 >= 18.0) {
                let y = top + (l as f32 + run as f32 / 2.0) * ch - 7.0;
                super::fonts::draw(buf, w, h, margin + 6.0, y, n, super::fonts::Face::Italic, 13.0, 0.0, 0x0050_3C2C, Some(0x00EE_E4CC));
            }
            l += run;
        }
    }
    // Above the ground on the row: trees in profile and the houses' pitched roofs; creatures
    // on the row at their level (the section had shown only rock, rooms and settlers).
    {
        let mut put = |px: i64, py: i64, c: Rgb, a: f32| {
            if px < 0 || py < 0 || px as usize >= w || py as usize >= h { return; }
            let k = py as usize * w + px as usize;
            let p = buf[k];
            let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
            buf[k] = pack(mix(old, c, a));
        };
        let ground_y = |z: i32| top + (ztop - z) as f32 * ch;
        let mut x = x0;
        while x < x1.min(map.width) {
            let k = row * map.width + x;
            let sz = map.surface_z[k];
            let roof = map.roofs[k];
            if roof > 0 {
                // A run of this roof along the row: one pitched roof over its walls.
                let mut e = x;
                while e + 1 < x1.min(map.width) && map.roofs[row * map.width + e + 1] == roof { e += 1; }
                let (lx, rx) = (margin + (x - x0) as f32 * cw, margin + (e + 1 - x0) as f32 * cw);
                let eave = ground_y(sz + 1);
                let peak = eave - ((rx - lx) * 0.35).min(ch * 1.6);
                let stone = map.houses.get(roof as usize - 1).map_or(false, |r| r.stone);
                let mut pen = super::ink::Pen::new(&mut put, 0.0, 0.0, 2.0);
                pen.poly(&[(lx - 3.0, eave), ((lx + rx) / 2.0, peak), (rx + 3.0, eave)], if stone { [150.0, 140.0, 130.0] } else { [196.0, 168.0, 108.0] });
                x = e + 1;
                continue;
            }
            if sz >= 0 && (sz as usize) < map.depth {
                if let crate::local::Plant::Tree(kind) = map.cell(x, row, sz as usize).plant {
                    if kind != crate::local::TreeKind::Fungus {
                        let (cxp, gy) = (margin + (x - x0) as f32 * cw + cw / 2.0, ground_y(sz));
                        let hgt = (ch * 2.6).max(14.0);
                        let mut pen = super::ink::Pen::new(&mut put, 0.0, 0.0, 2.0);
                        pen.rect(cxp - (cw * 0.08).max(1.0), gy - hgt * 0.45, cxp + (cw * 0.08).max(1.0), gy, [120.0, 86.0, 54.0]);
                        let (r, c) = crown(kind);
                        let _ = r;
                        // Crowns as wide as the surface view's (a broadleaf ~4 m across, a fir
                        // ~3 m): they had been thin poplars, unlike the same trees seen from above.
                        if matches!(kind, crate::local::TreeKind::Conifer) {
                            let half = (hgt * 0.24).max(cw * 0.7);
                            pen.poly(&[(cxp - half, gy - hgt * 0.28), (cxp, gy - hgt * 1.05), (cxp + half, gy - hgt * 0.28)], c);
                        } else {
                            let r = (hgt * 0.27).max(cw * 0.9);
                            for (ox, oy, k) in [(-0.62f32, 0.18f32, 0.72f32), (0.62, 0.18, 0.72), (0.0, 0.0, 1.0)] {
                                pen.ellipse(cxp + ox * r, gy - hgt * 0.7 + oy * r, r * k, r * k * 0.9, c);
                            }
                        }
                    }
                }
            }
            x += 1;
        }
        for c in colony.creatures.iter().filter(|c| (c.pos.1 as i32 - row as i32).abs() <= 1 && (c.pos.0 as usize) >= x0 && (c.pos.0 as usize) < x1) {
            let z = colony.creature_here3(c).2 + 1;
            let (cxp, base) = (margin + (c.pos.0 as usize - x0) as f32 * cw + cw / 2.0, top + (ztop - z + 1) as f32 * ch);
            let look = creature_look(colony, c);
            let px = (ch * 1.6 * look.len.sqrt()).clamp(12.0, 60.0);
            let (left, pose) = creature_motion(c);
            super::beasts::draw(&mut put, &look, cxp, base, px, left, pose, 1.0);
        }
    }
    // Settlers on the row stand at their level: small inked figures in their portrait's colours.
    let looks = settler_looks(colony, None);
    for (i, s) in colony.settlers.iter().enumerate().filter(|(_, s)| s.alive && (s.pos.1 as i32 - row as i32).abs() <= 1) {
        let x = s.pos.0 as usize;
        if x < x0 || x >= x1 { continue; }
        let z = colony.here3(i).2 + 1;
        let (cx, base) = (margin + (x - x0) as f32 * cw + cw / 2.0, top + (ztop - z + 1) as f32 * ch);
        let fig_h = (ch * 0.95).max(9.0);
        let (skin, hair, dress) = looks[i];
        let (bw, hr) = (fig_h * 0.24, fig_h * 0.17);
        let hy = base - fig_h + hr;
        for yy in (base - fig_h - 1.0) as i64..=base as i64 {
            for xx in (cx - bw - 1.0) as i64..=(cx + bw + 1.0) as i64 {
                if xx < 0 || yy < 0 || xx as usize >= w || yy as usize >= h { continue; }
                let (dx, dy) = (xx as f32 + 0.5 - cx, yy as f32 + 0.5);
                // The head (hair on its crown), then the body, a rounded block.
                let dh = (dx * dx + (dy - hy) * (dy - hy)).sqrt();
                let c = if dh <= hr + 0.5 { Some(if dh > hr - 0.7 { INK } else if dy < hy - hr * 0.2 { hair } else { skin }) }
                    else if dy > hy + hr && dy <= base && dx.abs() <= bw { Some(if dx.abs() > bw - 1.0 || dy > base - 1.0 { INK } else { dress }) }
                    else { None };
                if let Some(c) = c { buf[yy as usize * w + xx as usize] = pack(c); }
            }
        }
    }
    // The rooms cut by the row, in profile: what stands on each floor and its name (they had
    // been bare boxes). Pixel pen: (u, v) are screen pixels.
    {
        use crate::colony::delve::RoomKind;
        let mut names: Vec<(f32, f32, String)> = Vec::new();
        {
            let mut put = |px: i64, py: i64, c: Rgb, a: f32| {
                if px < 0 || py < 0 || px as usize >= w || py as usize >= h { return; }
                let k = py as usize * w + px as usize;
                let p = buf[k];
                let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
                buf[k] = pack(mix(old, c, a));
            };
            for r in colony.rooms.iter().filter(|r| r.kind != RoomKind::Corridor) {
                let xs: Vec<usize> = r.cells.iter().filter(|c| c.1 as usize == row && (c.0 as usize) >= x0 && (c.0 as usize) < x1).map(|c| c.0 as usize).collect();
                let (Some(&a), Some(&b)) = (xs.iter().min(), xs.iter().max()) else { continue };
                if r.z > ztop || r.z < ztop - levels as i32 { continue; }
                let (l, rr) = (margin + (a - x0) as f32 * cw, margin + (b - x0 + 1) as f32 * cw);
                let floor = top + (ztop - r.z) as f32 * ch;
                let cx = (l + rr) / 2.0;
                let u = ch.min((rr - l) * 0.45).max(4.0);
                let mut pen = super::ink::Pen::new(&mut put, 0.0, 0.0, 2.0);
                let wood: Rgb = [150.0, 108.0, 70.0];
                let stone: Rgb = [196.0, 186.0, 168.0];
                match r.kind {
                    RoomKind::Bedroom => {
                        let blanket = r.owner.and_then(|o| colony.settlers.get(o)).map(|_| [150.0, 96.0, 84.0]).unwrap_or([150.0, 120.0, 96.0]);
                        pen.rect(cx - u * 0.9, floor - u * 0.35, cx + u * 0.9, floor - u * 0.05, wood);
                        pen.rect(cx - u * 0.8, floor - u * 0.5, cx + u * 0.5, floor - u * 0.35, blanket);
                        pen.ellipse(cx + u * 0.65, floor - u * 0.47, u * 0.2, u * 0.12, [232.0, 226.0, 212.0]);
                    }
                    RoomKind::GreatHall | RoomKind::Hall => {
                        pen.rect(cx - u * 1.1, floor - u * 0.55, cx + u * 1.1, floor - u * 0.45, wood);
                        for s in [-0.9f32, 0.9] { pen.rect(cx + s * u - u * 0.06, floor - u * 0.45, cx + s * u + u * 0.06, floor, wood); }
                        for s in [-1.45f32, 1.45] { pen.rect(cx + s * u - u * 0.25, floor - u * 0.25, cx + s * u + u * 0.25, floor - u * 0.17, wood); }
                    }
                    RoomKind::Cellar => {
                        for k in 0..3 { let bx = cx + (k as f32 - 1.0) * u * 0.7; pen.ellipse(bx, floor - u * 0.3, u * 0.28, u * 0.3, [146.0, 104.0, 66.0]); pen.rect_f(bx - u * 0.28, floor - u * 0.34, bx + u * 0.28, floor - u * 0.28, [80.0, 64.0, 50.0], super::ink::Finish::Plain); }
                    }
                    RoomKind::Farm => {
                        for k in 0..4 { let bx = cx + (k as f32 - 1.5) * u * 0.55; pen.rect(bx - u * 0.04, floor - u * 0.35, bx + u * 0.04, floor, [222.0, 214.0, 196.0]); pen.ellipse(bx, floor - u * 0.38, u * 0.2, u * 0.1, [150.0, 112.0, 150.0]); }
                    }
                    RoomKind::Tomb => {
                        pen.rect(cx - u * 0.9, floor - u * 0.35, cx + u * 0.9, floor, stone);
                        pen.rect(cx - u * 0.75, floor - u * 0.5, cx + u * 0.75, floor - u * 0.35, [120.0, 90.0, 62.0]);
                    }
                    RoomKind::Workshop | RoomKind::Carpenter | RoomKind::Mason => {
                        pen.rect(cx - u * 0.8, floor - u * 0.45, cx + u * 0.8, floor - u * 0.35, wood);
                        for s in [-0.65f32, 0.65] { pen.rect(cx + s * u - u * 0.05, floor - u * 0.35, cx + s * u + u * 0.05, floor, wood); }
                        if r.kind == RoomKind::Mason { pen.rect(cx - u * 0.3, floor - u * 0.75, cx + u * 0.2, floor - u * 0.45, stone); }
                        else { pen.rect(cx - u * 0.5, floor - u * 0.55, cx + u * 0.3, floor - u * 0.45, [176.0, 140.0, 96.0]); }
                    }
                    RoomKind::Smelter | RoomKind::Forge | RoomKind::Kiln => {
                        pen.glow(cx, floor - u * 0.3, u * 0.9, [240.0, 140.0, 60.0], 0.35);
                        pen.poly(&[(cx - u * 0.6, floor), (cx - u * 0.45, floor - u * 0.7), (cx + u * 0.45, floor - u * 0.7), (cx + u * 0.6, floor)], [150.0, 96.0, 72.0]);
                        pen.ellipse(cx, floor - u * 0.25, u * 0.2, u * 0.15, [236.0, 150.0, 60.0]);
                        if r.kind == RoomKind::Forge { pen.rect(cx + u * 0.75, floor - u * 0.35, cx + u * 1.15, floor - u * 0.25, [90.0, 90.0, 96.0]); }
                    }
                    RoomKind::Corridor => {}
                }
                let name = match r.kind { RoomKind::Bedroom => r.owner.and_then(|o| colony.settlers.get(o)).map(|s| s.name.clone()).unwrap_or_else(|| "a bedroom".into()), RoomKind::GreatHall => "the great hall".into(), RoomKind::Hall => "the hall".into(), RoomKind::Cellar => "the cellar".into(), RoomKind::Farm => "the farm".into(), RoomKind::Tomb => "the tombs".into(), RoomKind::Workshop => "the workshops".into(), RoomKind::Mason => "the mason's".into(), RoomKind::Carpenter => "the carpenter's".into(), RoomKind::Smelter => "the smelter".into(), RoomKind::Forge => "the forge".into(), RoomKind::Kiln => "the kiln".into(), RoomKind::Corridor => String::new() };
                // Beside the room, right of its far wall (above it, a name covered the room overhead).
                if ch >= 6.0 { names.push((rr + 6.0, floor - ch * 0.5 + 5.0, name)); }
            }
        }
        let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new();
        for (x, y, t) in names {
            let tw = super::fonts::width(&t, super::fonts::Face::Italic, 11.0, 0.0);
            let r = (x, y - 11.0, tw, 12.0);
            if crowded(&placed, r) { continue; }
            placed.push(r);
            super::fonts::draw(buf, w, h, r.0, r.1, &t, super::fonts::Face::Italic, 11.0, 0.0, pack(INK), Some(0x00EE_E4CC));
        }
    }
    let title = format!("Section through row {} ({} levels; 0 is the camp's ground)", row, levels);
    // Top centre below the HUD's banner (at the left it ran under the camp's card).
    let tw = super::fonts::width(&title, super::fonts::Face::Italic, 16.0, 0.0);
    super::fonts::draw(buf, w, h, (w as f32 - tw) / 2.0, 62.0, &title, super::fonts::Face::Italic, 16.0, 0.0, pack(INK), Some(0x00EE_E4CC));
}

/// Everything over a level slice: near the camp's ground (two levels either side) the camp as
/// the surface view draws it (its fire, store, marks, crops, creatures, night and winter, those
/// on the surface), then the delve's rooms and those below at this level; deeper or higher,
/// the delve alone. So the level view and the surface view show the same camp.
pub fn draw_level(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    let zc = colony.map.surface_z[colony.camp.1 as usize * colony.map.width + colony.camp.0 as usize];
    let near_ground = (cam.z - zc).abs() <= 2;
    let mut placed = Vec::new();
    let tl0 = std::time::Instant::now();
    let mut tl1 = tl0;
    if near_ground {
        // The camp's marks, night and winter only where this level shows the surface: not on
        // the rock cut through below it, nor on the rooms dug there.
        let map = &colony.map;
        let t = cam.tile_px;
        let (x0, y0) = ((cam.cx - w as f32 / 2.0 / t).floor().max(0.0) as usize, (cam.cy - h as f32 / 2.0 / t).floor().max(0.0) as usize);
        let (x1, y1) = (((cam.cx + w as f32 / 2.0 / t) as usize + 1).min(map.width), ((cam.cy + h as f32 / 2.0 / t) as usize + 1).min(map.height));
        let gw = x1.saturating_sub(x0);
        let cells: Vec<bool> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| level_shows_surface(map, x, y, cam.z)).collect();
        // (Each screen column's cell found once: a float test per pixel had cost several
        // milliseconds a frame at 2560x1440.)
        let colx: Vec<Option<usize>> = (0..w).map(|sx| {
            let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
            (fx >= x0 as f32 && fx < x1 as f32).then(|| fx as usize - x0)
        }).collect();
        let mut mask = vec![true; w * h];
        for (sy, row) in mask.chunks_mut(w).enumerate() {
            let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
            if fy < y0 as f32 || fy >= y1 as f32 { continue; }
            let line = &cells[(fy as usize - y0) * gw..(fy as usize - y0 + 1) * gw];
            for (m, cx) in row.iter_mut().zip(&colx) { if let Some(cx) = cx { *m = line[*cx]; } }
        }
        tl1 = std::time::Instant::now();
        draw_colony_inner(colony, cam, buf, w, h, history, Some(&mask), &mut placed, false);
    }
    let tl2 = std::time::Instant::now();
    draw_delve_inner(colony, cam, buf, w, h, history, near_ground, &mut placed);
    if std::env::var("PLANET_TIME_DRAW").is_ok() { eprintln!("LEVEL mask {:.2} camp {:.2} delve {:.2}", (tl1 - tl0).as_secs_f64() * 1e3, (tl2 - tl1).as_secs_f64() * 1e3, tl2.elapsed().as_secs_f64() * 1e3); }
}

/// Settlers in a level view (`draw_delve` without the portraits' colours).
pub fn draw_settlers_by_level(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    draw_delve(colony, cam, buf, w, h, None);
}

/// A level of the delve: what furnishes its rooms (a bed in each bedroom with its owner's name, a
/// door at its mouth, the great hall's long table and benches), the rooms named, and the settlers
/// who stand on this level drawn as on the surface (those a level above or below faint, the rest
/// not at all), so the player sees who is down in their room and who is at the table.
pub fn draw_delve(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    draw_delve_inner(colony, cam, buf, w, h, history, false, &mut Vec::new());
}

fn draw_delve_inner(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>, surface_drawn: bool, placed: &mut Vec<LabelBox>) {
    use crate::colony::delve::RoomKind;
    let t = cam.tile_px;
    let to_screen = |x: f32, y: f32| ((x - cam.cx) * t + w as f32 / 2.0, (y - cam.cy) * t + h as f32 / 2.0);
    let mut put = |x: i64, y: i64, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
        let k = y as usize * w + x as usize;
        let p = buf[k];
        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
        buf[k] = pack(mix(old, c, a));
    };
    let rect = |put: &mut dyn FnMut(i64, i64, Rgb, f32), x0: f32, y0: f32, x1: f32, y1: f32, fill: Rgb| {
        for yy in y0 as i64..=y1 as i64 { for xx in x0 as i64..=x1 as i64 {
            let edge = xx == x0 as i64 || xx == x1 as i64 || yy == y0 as i64 || yy == y1 as i64;
            put(xx, yy, if edge { INK } else { fill }, 0.95);
        } }
    };
    // Cuts still to be made on this level (DF's designations): dashed outlines in red ink, a
    // stair's with its steps; the cell being dug now is filled.
    if let Some(plan) = colony.dig_plan.as_ref() {
        let busy: Vec<(u16, u16)> = colony.settlers.iter().filter_map(|s| if let crate::colony::Job::Dig(p, z) = s.job { (z == cam.z).then_some(p) } else { None }).collect();
        for c in plan.iter().filter(|c| c.z == cam.z && !colony.cut_done(c)) {
            let (x, y) = to_screen(c.p.0 as f32, c.p.1 as f32);
            let red: Rgb = [170.0, 50.0, 36.0];
            let now = busy.contains(&c.p);
            for yy in y as i64..(y + t).ceil() as i64 { for xx in x as i64..(x + t).ceil() as i64 {
                let (u, v) = ((xx as f32 - x) / t, (yy as f32 - y) / t);
                let edge = u < 0.08 || u > 0.92 || v < 0.08 || v > 0.92;
                let dash = ((u + v) * 6.0).fract() < 0.5;
                if edge && dash { put(xx, yy, red, 0.9); }
                else if now { put(xx, yy, red, 0.18); }
                else if c.stair && (v * 4.0).fract() < 0.2 && u > 0.2 && u < 0.8 { put(xx, yy, red, 0.5); }
            } }
        }
    }
    let mut labels: Vec<(f32, f32, String, bool)> = Vec::new();
    // The unknown is blank: places in the hills not yet found are drawn as the rock around them
    // (DF shows only what has been seen). Their mouths on the surface stay as they are.
    // (Debug: PLANET_REVEAL=1 shows them.)
    let reveal = std::env::var("PLANET_REVEAL").is_ok();
    for (k, pl) in colony.map.places.iter().enumerate() {
        if reveal || colony.places_found.contains(&k) || pl.mouth.is_none() { continue; }
        let sz_of = |c: (u16, u16)| colony.map.surface_z[c.1 as usize * colony.map.width + c.0 as usize];
        let hidden: Vec<((u16, u16), i32)> = pl.cells.iter().filter(|(c, z)| (*z == cam.z || *z + 1 == cam.z) && *z < sz_of(*c) - 1).copied().collect();
        // (And the rock beside them, whose inked edges would trace the place's shape.)
        let mut cover: Vec<((u16, u16), i32)> = hidden.clone();
        for &(c, z) in &hidden { for dy in -1i32..=1 { for dx in -1i32..=1 {
            let q = ((c.0 as i32 + dx).max(0) as u16, (c.1 as i32 + dy).max(0) as u16);
            if (q.0 as usize) < colony.map.width && (q.1 as usize) < colony.map.height && !cover.iter().any(|h| h.0 == q) && colony.map.cell(q.0 as usize, q.1 as usize, (cam.z + 1).max(0) as usize).shape == crate::local::Shape::Wall { cover.push((q, z)); }
        } } }
        let zb = (cam.z + 1).max(0) as usize;
        let in_place = |q: (u16, u16)| hidden.iter().any(|h| h.0 == q);
        for &(c, z) in &cover {
            let (x, y) = to_screen(c.0 as f32, c.1 as f32);
            // (The level renderer's own rock: its wash, and its hatching in cell units. Where the
            // place is cut, the rock is the nearest uncut rock of this level, since the strata
            // run sideways: the floor below the place can be another stone and would show its
            // outline as a ghost.)
            let body = colony.map.cell(c.0 as usize, c.1 as usize, zb);
            let rock = if body.shape == crate::local::Shape::Wall { body } else {
                (1i32..=4).find_map(|r| (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter(|&(dx, dy)| dx.abs().max(dy.abs()) == r).find_map(|(dx, dy)| {
                    let q = (c.0 as i32 + dx, c.1 as i32 + dy);
                    if q.0 < 0 || q.1 < 0 || q.0 as usize >= colony.map.width || q.1 as usize >= colony.map.height || in_place((q.0 as u16, q.1 as u16)) { return None; }
                    let k = colony.map.cell(q.0 as usize, q.1 as usize, zb);
                    (k.shape == crate::local::Shape::Wall).then_some(k)
                })).unwrap_or_else(|| colony.map.cell(c.0 as usize, c.1 as usize, z.max(0) as usize))
            };
            for yy in y as i64..(y + t).ceil() as i64 { for xx in x as i64..(x + t).ceil() as i64 {
                let (fx, fy) = (cam.cx + (xx as f32 + 0.5 - w as f32 / 2.0) / t, cam.cy + (yy as f32 + 0.5 - h as f32 / 2.0) / t);
                put(xx, yy, cut_rock(&colony.map, rock, c.0 as usize, c.1 as usize, zb, fx, fy, 1.1 / t), 1.0);
            } }
        }
    }
    let cells = super::camp_ink::Cells::new(cam, w, h);
    let looks = settler_looks(colony, history);
    // The lost relic where it lies below; the wet shaft's lining at the aquifer's levels.
    if colony.relic.as_ref().map_or(false, |r| r.below) { super::camp_ink::draw_relic(colony, cam, &mut put, w, h, Some(cam.z)); }
    if let (true, Some((lo, hi)), Some(sp)) = (colony.aquifer_lined, colony.map.aquifer, colony.spine) {
        if cam.z >= lo && cam.z <= hi + 1 { super::camp_ink::draw_lining(&mut put, cam, w, h, sp.at); }
    }
    // The hatch over the stair below the first cavern: planks bound with iron, an iron ring.
    if let Some((p, _)) = colony.hatch.filter(|h| h.1 == cam.z || h.1 == cam.z + 1) {
        super::furniture::hatch(&mut cells.pen(&mut put, p.0 as f32, p.1 as f32));
    }
    // Engravings on this level's walls: a carved panel on the face toward the floor.
    for e in colony.engravings.iter().filter(|e| e.z == cam.z) {
        super::camp_ink::draw_engraving(&mut put, cam, w, h, e);
    }
    for r in colony.rooms.iter().filter(|r| r.z == cam.z) {
        match r.kind {
            RoomKind::Bedroom => {
                // The door: a leaf of planks across the doorway (the room's first cell), at its
                // room side, with a hinge pin. (A filled square in the passage read as a crate.)
                if let Some(&d) = r.cells.first() {
                    let (x, y) = to_screen(d.0 as f32, d.1 as f32);
                    let into = r.cells.get(1).map(|n| (n.0 as i32 - d.0 as i32, n.1 as i32 - d.1 as i32)).unwrap_or((0, 1));
                    let leaf: Rgb = [150.0, 110.0, 70.0];
                    // The leaf's box in cell units: thin across the way in, at the room's side.
                    let (u0, v0, u1, v1) = match into {
                        (1, _) => (0.62, 0.08, 0.84, 0.92), (-1, _) => (0.16, 0.08, 0.38, 0.92),
                        (_, 1) => (0.08, 0.62, 0.92, 0.84), _ => (0.08, 0.16, 0.92, 0.38),
                    };
                    rect(&mut put, x + t * u0, y + t * v0, x + t * u1, y + t * v1, leaf);
                    // The planks' joints, along the leaf.
                    let along_x = u1 - u0 > v1 - v0;
                    for k in [0.33f32, 0.66] {
                        if along_x { let yy = (y + t * (v0 + (v1 - v0) * 0.5)) as i64; let xx = (x + t * (u0 + (u1 - u0) * k)) as i64; put(xx, yy, INK, 0.6); put(xx, yy - 1, INK, 0.6); }
                        else { let xx = (x + t * (u0 + (u1 - u0) * 0.5)) as i64; let yy = (y + t * (v0 + (v1 - v0) * k)) as i64; put(xx, yy, INK, 0.6); put(xx - 1, yy, INK, 0.6); }
                    }
                }
                if let Some(b) = r.bed {
                    let (x, y) = to_screen(b.0 as f32, b.1 as f32);
                    // A bed (once made at the workshop), its blanket in its owner's colour; else a
                    // pallet of straw.
                    {
                        let mut pen = cells.pen(&mut put, b.0 as f32, b.1 as f32);
                        if r.furnished.is_some() { super::furniture::bed(&mut pen, r.owner.map(|o| looks[o].2).unwrap_or([150.0, 130.0, 110.0]), r.quality >= 3); }
                        else { super::furniture::pallet(&mut pen); }
                    }
                    // A cradle made ready by the bed of one expecting (`family.rs`).
                    if let Some(o) = r.owner.filter(|o| colony.expecting.iter().any(|e| e.0 == *o)) {
                        let _ = o;
                        let mut pen = cells.pen(&mut put, b.0 as f32 - 0.6, b.1 as f32 + 0.3);
                        pen.shape([150.0, 108.0, 70.0], super::ink::Finish::Inked, [-0.3, -0.2, 0.3, 0.3], &|u, v| v > -0.15 && v < 0.25 && u.abs() < 0.28 - (v - 0.25).abs() * 0.1);
                        pen.rect_f(-0.2, -0.1, 0.2, 0.1, [236.0, 228.0, 210.0], super::ink::Finish::Plain);
                    }
                    // What the owner keeps as their own, by the bed (`Colony::kept`).
                    if let Some(o) = r.owner {
                        for (j, &(k, _)) in colony.kept.iter().filter(|kk| kk.1 == o).enumerate().take(3) {
                            if let Some(wk) = colony.works.get(k) {
                                let (gx, gy) = to_screen(b.0 as f32 + 1.2 + 0.0 * j as f32, b.1 as f32 + 0.25 + j as f32 * 0.35);
                                let tint = if wk.quality >= 4 { Some([214.0, 176.0, 70.0]) } else { None };
                                super::glyphs::draw(&mut put, super::glyphs::Glyph::of_thing(&wk.kind), gx, gy, (t * 0.4).max(6.0), tint);
                            }
                        }
                    }
                    // (Named on the bed only when its owner is not lying in it.)
                    let abed = r.owner.map_or(false, |o| colony.settlers[o].alive && colony.here3(o) == (b.0, b.1, r.z));
                    if t >= 9.0 && !abed {
                        let who = r.owner.map(|o| colony.settlers[o].name.clone()).unwrap_or_else(|| "empty".into());
                        labels.push((x + t * 0.5, y - 2.0, who, r.owner.is_none()));
                    }
                }
            }
            RoomKind::GreatHall => {
                if let Some(c) = r.bed.filter(|_| r.furnished.is_some()) {
                    // The long table, its two benches, bowls and cups.
                    super::furniture::long_table(&mut cells.pen(&mut put, c.0 as f32 - 2.0, c.1 as f32), r.quality >= 3);
                    let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 1.6);
                    labels.push((lx, ly, "the great hall".into(), false));
                }
            }
            RoomKind::Hall | RoomKind::Cellar => {
                if r.kind == RoomKind::Hall { super::camp_ink::draw_hill_hall(&mut put, cam, w, h, &r.cells); }
                if r.kind == RoomKind::Cellar {
                    // Casks, sacks and crates along it.
                    for (k, &c) in r.cells.iter().enumerate().filter(|(k, _)| k % 3 == 1) {
                        super::furniture::cellar_stores(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32), k / 3);
                    }
                }
                let n = r.cells.len().max(1) as f32;
                let (mx, my) = r.cells.iter().fold((0.0, 0.0), |a, c| (a.0 + c.0 as f32, a.1 + c.1 as f32));
                let (lx, ly) = to_screen(mx / n + 0.5, my / n + 0.5);
                labels.push((lx, ly, r.kind.word().trim_start_matches("the ").to_string(), false));
            }
            RoomKind::Tomb => {
                // A niche: a stone coffin with a cross cut in its lid, and the name of who lies
                // there; an empty niche is a bare ledge.
                if let Some(&c) = r.cells.first() {
                    let (x, y) = to_screen(c.0 as f32, c.1 as f32);
                    super::furniture::coffin(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32), r.owner.is_some());
                    if let Some(o) = r.owner {
                        if t >= 9.0 { labels.push((x + t * 0.5, y - 2.0, colony.settlers[o].name.clone(), false)); }
                    }
                }
            }
            RoomKind::Workshop => {
                // The benches: two heavy tables with tools on them, and the room named.
                if let Some(c) = r.bed {
                    for (ox, oy) in [(-1.0f32, -1.0f32), (1.0, 1.0)] {
                        super::furniture::bench(&mut cells.pen(&mut put, c.0 as f32 + ox, c.1 as f32 + oy), true);
                    }
                    let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 1.8);
                    labels.push((lx, ly, "the workshops".into(), false));
                }
            }
            RoomKind::Farm => {
                // Plots in rows: pale caps of what grows in the dark.
                for &c in &r.cells {
                    let grown = 1 + ((c.0 as usize * 7 + c.1 as usize * 3 + colony.clock.day() as usize) % 3);
                    super::furniture::fungus_bed(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32), grown);
                }
                if let Some(c) = r.bed { let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 2.6); labels.push((lx, ly, "the farm under the rock".into(), false)); }
            }
            RoomKind::Mason | RoomKind::Carpenter | RoomKind::Smelter | RoomKind::Forge | RoomKind::Kiln => {
                // The industries' shops (`colony/industry.rs`): each its bench or furnace, named.
                if let Some(c) = r.bed {
                    {
                        let mut pen = cells.pen(&mut put, c.0 as f32, c.1 as f32);
                        let metal = colony.industry.bars.first().map(|b| super::glyphs::metal_colour(&b.0)).unwrap_or([148.0, 150.0, 158.0]);
                        match r.kind {
                            RoomKind::Mason => super::furniture::mason(&mut pen),
                            RoomKind::Carpenter => super::furniture::carpenter(&mut pen),
                            RoomKind::Smelter => super::furniture::smelter(&mut pen, metal),
                            RoomKind::Kiln => super::furniture::kiln(&mut pen),
                            _ => super::furniture::forge(&mut pen, colony.magma_forge),
                        }
                    }
                    let name = match r.kind { RoomKind::Mason => "the mason's", RoomKind::Carpenter => "the carpenter's", RoomKind::Smelter => "the smelter", RoomKind::Forge => "the forge", _ => "the kiln" };
                    let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 1.3);
                    labels.push((lx, ly, name.into(), false));
                }
            }
            RoomKind::Corridor => {}
        }
    }
    // The places in the hills, once found: what lies at the far end (a sarcophagus, a beast's
    // bones and hoard, an ore cart at the seam), named.
    for (k, pl) in colony.map.places.iter().enumerate() {
        if !colony.places_found.contains(&k) || pl.mouth.is_none() { continue; }
        let Some(&(c, z)) = pl.cells.last() else { continue };
        if z != cam.z { continue; }
        let (x, y) = to_screen(c.0 as f32, c.1 as f32);
        use crate::local::places::PlaceKind;
        match pl.kind {
            PlaceKind::Tomb => super::furniture::coffin(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32), true),
            PlaceKind::Lair => super::furniture::lair(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32)),
            PlaceKind::OldMine => super::furniture::ore_cart(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32)),
            PlaceKind::Cave => super::furniture::cave_end(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32)),
            PlaceKind::Halls => super::furniture::halls_end(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32)),
            _ => {}
        }
        if t >= 9.0 { labels.push((x + t * 0.5, y - 2.0, pl.name.clone(), false)); }
    }
    // Artifacts set in the rooms: a gilded plinth with the work on it, named.
    for (title, k, c) in &colony.placed {
        if colony.rooms.get(*k).map_or(true, |r| r.z != cam.z) { continue; }
        let (x, y) = to_screen(c.0 as f32, c.1 as f32);
        super::furniture::artifact(&mut cells.pen(&mut put, c.0 as f32, c.1 as f32), title);
        if t >= 9.0 { labels.push((x + t * 0.5, y - 2.0, title.clone(), false)); }
    }
    // Creatures below the ground on this level (a cavern's life, a hunter on the stair, what the
    // deep sent), faintly those a level off; those on the surface are `draw_colony`'s.
    let scale = (t / 16.0).clamp(0.55, 1.4);
    for c in colony.creatures.iter().filter(|c| colony.creature_below(c) || (!surface_drawn && c.z.is_some())) {
        let z = colony.creature_here3(c).2;
        let off = (z - cam.z).abs();
        if off > 1 { continue; }
        let (x, y) = to_screen(c.pos.0 as f32 + 0.5, c.pos.1 as f32 + 0.5);
        draw_creature(&mut put, colony, history, c, x, y, scale, if off == 0 { 1.0 } else { 0.3 });
        if off == 0 && t >= 9.0 && matches!(c.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::CaveHunter) {
            labels.push((x, y - (t * 0.6 * c.size).max(8.0) - 4.0, c.name.clone(), false));
        }
    }
    // Settlers on this level, and faintly those a level off.
    for (i, s) in colony.settlers.iter().enumerate().filter(|(_, s)| s.alive) {
        // (Those on the surface are drawn by `draw_colony` when it ran.)
        if surface_drawn && !colony.below(i) { continue; }
        let z = colony.here3(i).2;
        let off = (z - cam.z).abs();
        if off > 1 { continue; }
        // The surface view's figure, job pictogram and all; a level off, faint.
        let a = if off == 0 { 1.0 } else { 0.3 };
        let dp = colony.draw_pos(i);
        let (x, y) = to_screen(dp.0 + 0.5, dp.1 + 0.5);
        let mut faint = |px: i64, py: i64, c: Rgb, al: f32| put(px, py, c, al * a);
        let top = draw_figure(&mut faint, colony, i, x, y, scale, looks[i]);
        if off == 0 && t >= 6.0 { labels.push((x, top - 2.0, s.name.clone(), false)); }
    }
    // The level's caption: how deep, and what is dug on it.
    {
        let zc = colony.map.surface_z[colony.camp.1 as usize * colony.map.width + colony.camp.0 as usize];
        let d = cam.z - zc;
        let mut kinds: Vec<String> = Vec::new();
        for (r, dug) in colony.rooms.iter().map(|r| (r, true)).chain(colony.dig_rooms.iter().map(|r| (r, false))).filter(|(r, _)| r.z == cam.z) {
            let k = match r.kind { RoomKind::Bedroom => "bedrooms", RoomKind::GreatHall => "the great hall", RoomKind::Hall => "the hall in the hill", RoomKind::Cellar => "the cellar", RoomKind::Tomb => "the tombs", RoomKind::Workshop => "the workshops", RoomKind::Farm => "the farm", RoomKind::Mason => "the mason's", RoomKind::Carpenter => "the carpenter's", RoomKind::Smelter => "the smelter", RoomKind::Forge => "the forge", RoomKind::Kiln => "the kiln", RoomKind::Corridor => continue };
            let k = if dug { k.to_string() } else { format!("{} (being dug)", k) };
            if !kinds.iter().any(|x| *x == k) { kinds.push(k); }
        }
        if let Some(c) = colony.map.caverns.iter().find(|c| (0..colony.map.width).step_by(16).any(|x| colony.map.cavern_at(x, colony.camp.1 as usize, cam.z + 1).map_or(false, |l| l == c.layer as usize))) { kinds.push(c.name.clone()); }
        let depth = if d == 0 { "the camp's ground".to_string() } else if d < 0 { format!("{} below the camp's ground", -d) } else { format!("{} above the camp's ground", d) };
        let cap = format!("Level {} - {}{}", cam.z, depth, if kinds.is_empty() { String::new() } else { format!(": {}", kinds.join(", ")) });
        // Below the HUD's banner at the top centre (a moment's title sits there; they had overlapped).
        letter(buf, w, h, placed, w as f32 / 2.0, 70.0, &cap, super::fonts::Face::Italic, 16.0, 0.0, 0x0030_1E14);
    }
    // Names step aside from each other and from the surface view's labels (above, below,
    // right, left) or are left out.
    for (x, y, text, faint) in labels {
        let px = 12.0;
        let tw = super::fonts::width(&text, super::fonts::Face::Italic, px, 0.0);
        for (ox, oy) in [(0.0, 0.0), (0.0, px + 16.0), (tw / 2.0 + 10.0, px * 0.6), (-tw / 2.0 - 10.0, px * 0.6)] {
            let r = (x - tw / 2.0 + ox, y - px + oy, tw, px + 1.0);
            if crowded(placed, r) { continue; }
            placed.push(r);
            super::fonts::draw(buf, w, h, r.0, r.1, &text, super::fonts::Face::Italic, px, 0.0, if faint { 0x0080_7060 } else { 0x0030_1E14 }, Some(0x00EE_E4CC));
            break;
        }
    }
}

/// A disc of `fill` ringed in `ring`, at opacity `a` (as `draw_colony`'s disc).
fn disc_a(put: &mut dyn FnMut(i64, i64, Rgb, f32), a: f32, cx: f32, cy: f32, r: f32, fill: Rgb, ring: Rgb) {
    let rr = r.ceil() as i64 + 1;
    for dy in -rr..=rr {
        for dx in -rr..=rr {
            let d = ((dx as f32 + 0.5 - (cx.fract())).powi(2) + (dy as f32 + 0.5 - (cy.fract())).powi(2)).sqrt();
            let (x, y) = (cx.floor() as i64 + dx, cy.floor() as i64 + dy);
            if d <= r - 1.0 { put(x, y, fill, a); } else if d <= r + 0.3 { put(x, y, ring, 0.95 * a); }
        }
    }
}

/// The sprite of a creature (`beasts.rs`): a beast of the threat or of the deep as its monster
/// is described, game, herds, pets and cave life by their names, the dead and the cursed.
pub(crate) fn creature_look(colony: &crate::colony::Colony, c: &crate::colony::creatures::Creature) -> super::beasts::Look {
    use crate::colony::creatures::CreatureKind;
    if c.kind == CreatureKind::Beast {
        let monster = colony.arc.as_ref().filter(|a| a.threat.name == c.name).and_then(|a| a.threat.monster.as_ref())
            .or_else(|| colony.map.caverns.iter().filter_map(|cv| cv.beast.as_ref()).find(|(n, _)| *n == c.name).map(|(_, m)| m))
            .or_else(|| colony.expedition.as_ref().and_then(|e| e.monster.as_ref()));
        if let Some(m) = monster { return super::beasts::of_monster(m); }
        let mut l = super::beasts::of_name(&c.name);
        l.len = (1.2 + c.size * 0.55).clamp(1.2, 4.0);
        if l.glow.is_none() { l.glow = Some([230.0, 70.0, 40.0]); }
        return l;
    }
    let mut l = super::beasts::of_name(&c.name);
    if c.kind == CreatureKind::Wolf && !c.name.contains("risen") && !c.name.contains("dead") && !c.name.contains(crate::colony::curse::MOON) && !c.name.contains(crate::colony::curse::CHANGED) {
        l = super::beasts::of_name("wolf");
    }
    l
}

/// Which way a creature faces (towards its next step; else as it last faced, by its id) and
/// whether it is mid-stride.
fn creature_motion(c: &crate::colony::creatures::Creature) -> (bool, super::beasts::Pose) {
    let next = c.path.first().copied().or_else(|| c.path3.first().map(|p| (p.0, p.1)));
    let left = match next { Some(n) if n.0 != c.pos.0 => n.0 < c.pos.0, _ => (c.id + c.home.0 as u32) % 2 == 0 };
    let pose = if next.is_some() { super::beasts::Pose::Walk((c.pos.0 as u32 + c.pos.1 as u32) % 2 == 0) } else { super::beasts::Pose::Stand };
    (left, pose)
}

/// One creature at screen `(x, y)` (the middle of its cell; cell size `t`), at opacity `a` (faint
/// a level off): attackers as armed figures of their band (`folk.rs`), traders with a pack mule,
/// everything else as its beast (`beasts.rs`), facing the way it walks.
fn draw_creature(put: &mut dyn FnMut(i64, i64, Rgb, f32), colony: &crate::colony::Colony, history: Option<&crate::history::world_state::WorldHistory>, c: &crate::colony::creatures::Creature, x: f32, y: f32, scale: f32, a: f32) {
    use crate::colony::creatures::CreatureKind;
    let (left, pose) = creature_motion(c);
    let left = if c.leaving { !left || c.path.is_empty() } else { left };
    match c.kind {
        CreatureKind::Raider | CreatureKind::Besieger => {
            let threat = colony.arc.as_ref().map(|a| &a.threat).filter(|t| t.name == c.name);
            let f = super::folk::raider(threat, &c.name, c.id, history);
            // Striking when a settler is within reach.
            let near = colony.settlers.iter().any(|s| s.alive && (s.pos.0 as i32 - c.pos.0 as i32).abs() <= 1 && (s.pos.1 as i32 - c.pos.1 as i32).abs() <= 1);
            let strike = near && (colony.clock.tick / 3 + c.id as u64) % 2 == 0;
            let head_top = super::folk::draw(put, &f, x, y, scale * 1.2, left, strike, a);
            // A child snatched by these raiders and coming back among them (`snatch.rs`) rides on
            // the shoulders of the first of them, small, in their own colours, legs either side of
            // the raider's head and hands on it.
            let first_raider = colony.creatures.iter().filter(|k| k.kind == CreatureKind::Raider).map(|k| k.id).min() == Some(c.id);
            if first_raider {
                if let Some(sn) = colony.snatched.iter().find(|sn| sn.coming && sn.home.is_none()) {
                    if let Some(look) = settler_looks(colony, history).get(sn.who).copied() {
                        let back = if left { 1.0 } else { -1.0 } * 1.5 * scale;
                        let size = 19.0 * scale;
                        let mut pen = super::ink::Pen::new(put, x + back, head_top - 0.32 * size, size).faint(a);
                        pen.ellipse(0.0, 0.45, 0.5, 0.35, look.2);
                        pen.ellipse(0.0, -0.2, 0.36, 0.36, look.0);
                        pen.shape(look.1, super::ink::Finish::Paint, [-0.36, -0.56, 0.36, -0.2], &|u, v| u * u + (v + 0.2).powi(2) < 0.1 && v < -0.32);
                        for s2 in [-1.0f32, 1.0] { pen.limb((s2 * 0.35, 0.35), 0.1, (s2 * 0.6, 0.7), 0.08, look.0); }
                    }
                }
            }
        }
        CreatureKind::Trader => {
            // A pack mule a step behind the trader, laden.
            let mule = super::beasts::of_name("mule");
            let back = if left { 1.0 } else { -1.0 } * 20.0 * scale;
            let mpx = super::beasts::px_for(&mule, scale) * 0.85;
            super::beasts::draw(put, &mule, x + back, y + 7.0 * scale, mpx, left, pose, a);
            let mut pen = super::ink::Pen::new(put, x + back, y + 7.0 * scale - mpx * 0.42, mpx).facing_left(left).faint(a);
            pen.rect(-0.38, -0.08, -0.02, 0.12, [196.0, 160.0, 96.0]);
            pen.rect(-0.02, -0.12, 0.26, 0.1, [170.0, 130.0, 80.0]);
            let f = super::folk::trader(&c.name, c.id);
            super::folk::draw(put, &f, x, y, scale, left, false, a);
        }
        _ => {
            let look = creature_look(colony, c);
            let px = super::beasts::px_for(&look, scale);
            let pose = if c.kind == CreatureKind::Game && pose == super::beasts::Pose::Stand && (colony.clock.tick / 240 + c.id as u64) % 3 == 0 { super::beasts::Pose::Graze } else { pose };
            super::beasts::draw(put, &look, x, y + 7.0 * scale, px, left, pose, a);
            if c.kind == CreatureKind::Pet {
                // A kept animal: a red collar knot at the neck.
                let mut pen = super::ink::Pen::new(put, x, y + 7.0 * scale - 0.8 * px / 2.0, px).facing_left(left).faint(a);
                let (u, v) = super::beasts::neck_point(&look);
                pen.ellipse_f(u, v, 0.05, 0.06, [180.0, 40.0, 30.0], super::ink::Finish::Plain);
            }
        }
    }
}
