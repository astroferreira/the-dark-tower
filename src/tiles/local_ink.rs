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

fn hash(x: i64, y: i64, salt: u64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}
fn unit(x: i64, y: i64, salt: u64) -> f32 { (hash(x, y, salt) >> 40) as f32 / (1u64 << 24) as f32 }

/// Smooth value noise in cell units (for watercolour mottling).
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

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }
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

/// What stands on a column, seen from above.
#[derive(Clone, Copy, PartialEq)]
enum Top { Ground, Water(f32), Wall(Material, i32) }

/// A tree's crown, precomputed for its column.
#[derive(Clone, Copy)]
struct Crown { ox: f32, oy: f32, r: f32, col: Rgb, conifer: bool, h: u64 }

/// Every column read once per frame, so pixels only index arrays (a pixel looks at up to 49
/// columns for overlapping crowns).
struct View<'a> {
    map: &'a LocalMap,
    top: Vec<Top>,
    wash: Vec<Rgb>,
    crowns: Vec<Option<Crown>>,
}

impl<'a> View<'a> {
    fn new(map: &'a LocalMap) -> Self {
        let (w, h) = (map.width, map.height);
        let top: Vec<Top> = (0..w * h).into_par_iter().map(|k| column_top(map, k % w, k / w)).collect();
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
        let tall: Vec<i32> = (0..w * h).into_par_iter().map(|k| {
            let (x, y, z) = (k % w, k / w, map.surface_z[k]);
            (1..=10).rev().find(|&dz| {
                let zz = z + dz;
                zz >= 0 && (zz as usize) < map.depth && {
                    let c = map.cell(x, y, zz as usize);
                    c.shape != Shape::Empty && matches!(c.material, Material::Wood | Material::Block(_) | Material::Clay)
                }
            }).unwrap_or(0)
        }).collect();
        let s = (SHADOW.0 * SHADOW.0 + SHADOW.1 * SHADOW.1).sqrt();
        let shaded = |k: usize| {
            let (x, y) = ((k % w) as f32, (k / w) as f32);
            tall[k] == 0 && (1..=4).any(|d| {
                let (qx, qy) = ((x - SHADOW.0 / s * d as f32).round(), (y - SHADOW.1 / s * d as f32).round());
                qx >= 0.0 && qy >= 0.0 && (qx as usize) < w && (qy as usize) < h && tall[qy as usize * w + qx as usize] as f32 * 0.6 >= d as f32
            })
        };
        let wash = (0..w * h).into_par_iter().map(|k| match top[k] {
            Top::Water(d) => water_wash(d),
            _ => {
                let c = wash(floor_of(k));
                let d = sunk(k);
                let c = if d >= 2 { mix(c, [70.0, 60.0, 50.0], (0.18 * d as f32).min(0.5)) } else { c };
                if shaded(k) { mix(c, [70.0, 60.0, 50.0], 0.22) } else { c }
            }
        }).collect();
        let crowns = (0..w * h).into_par_iter().map(|k| {
            if !matches!(top[k], Top::Ground) { return None; }
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
        }).collect();
        View { map, top, wash, crowns }
    }
    fn inside(&self, x: i64, y: i64) -> bool { x >= 0 && y >= 0 && (x as usize) < self.map.width && (y as usize) < self.map.height }
    fn idx(&self, x: i64, y: i64) -> usize {
        (y.clamp(0, self.map.height as i64 - 1) as usize) * self.map.width + x.clamp(0, self.map.width as i64 - 1) as usize
    }
    fn sz(&self, x: i64, y: i64) -> i32 { self.map.surface_z[self.idx(x, y)] }
    fn floor(&self, x: i64, y: i64) -> &crate::local::Cell {
        let k = self.idx(x, y);
        self.map.cell(k % self.map.width, k / self.map.width, self.map.surface_z[k].clamp(0, self.map.depth as i32 - 1) as usize)
    }
    fn top(&self, x: i64, y: i64) -> Top { self.top[self.idx(x, y)] }
    fn ground_wash(&self, x: i64, y: i64) -> Rgb { self.wash[self.idx(x, y)] }
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
    let v = View::new(map);
    let t = cam.tile_px;
    // Ink lines are about a pixel wide whatever the zoom: their width in cells.
    let line = (1.1 / t).max(0.03);
    buf.par_chunks_mut(w).enumerate().for_each(|(sy, row)| {
        let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
        for (sx, out) in row.iter_mut().enumerate() {
            let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
            if fx < 0.0 || fy < 0.0 || fx >= map.width as f32 || fy >= map.height as f32 {
                *out = OFF_MAP;
                continue;
            }
            *out = pack(pixel(&v, fx, fy, sx as i64, sy as i64, line, t));
        }
    });
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
    let higher = |dx: i64, dy: i64| v.inside(cx + dx, cy + dy) && v.sz(cx + dx, cy + dy) > sz && !matches!(v.top(cx + dx, cy + dy), Top::Water(_));
    let d = edge_distance(u, w, higher(-1, 0), higher(1, 0), higher(0, -1), higher(0, 1));
    if d < line { c = mix(c, INK, 0.7); }
    else if d < 0.32 {
        // Short strokes perpendicular to the edge, every few pixels along it.
        let along = if higher(0, -1) || higher(0, 1) { sx } else { sy };
        if along.rem_euclid((t * 0.25).max(3.0) as i64) == 0 { c = mix(c, INK, 0.45 * (1.0 - d / 0.32)); }
    }

    // 4. Shadows cast by crowns, then the crowns themselves (southern ones drawn over northern).
    let mut crown_hit: Option<(f32, Rgb, f32, f32, f32, bool)> = None; // (tree y, colour, dx, dy, radius, conifer)
    let mut shaded = false;
    for ty in cy - CROWN_REACH..=cy + CROWN_REACH {
        for tx in cx - CROWN_REACH..=cx + CROWN_REACH {
            if !v.inside(tx, ty) { continue; }
            let Some(Crown { ox, oy, r, col, conifer, h }) = v.crowns[v.idx(tx, ty)] else { continue };
            let (dx, dy) = (fx - ox, fy - oy);
            // Outlines stay within 0.78r..1.1r of the centre: decide by distance where possible.
            let d2 = dx * dx + dy * dy;
            if d2 <= (r * 1.1) * (r * 1.1) && crown_hit.map_or(true, |hit| oy > hit.0) {
                let edge = if d2 < (r * 0.75) * (r * 0.75) { r } else { crown_radius(r, dx, dy, conifer, h) };
                if d2 <= edge * edge {
                    let edge = if d2 < (r * 0.75) * (r * 0.75) { crown_radius(r, dx, dy, conifer, h) } else { edge };
                    crown_hit = Some((oy, col, dx, dy, edge, conifer));
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
    use crate::local::Shape;
    let surface = LocalCamera { surface_view: true, ..*cam };
    render_local_ink(map, &surface, buf, w, h);
    let t = cam.tile_px;
    let z = cam.z.clamp(0, map.depth as i32 - 2);
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
    buf.par_chunks_mut(w).enumerate().for_each(|(sy, row)| {
        let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
        for sx in 0..w {
            let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
            if fx < 0.0 || fy < 0.0 || fx >= map.width as f32 || fy >= map.height as f32 { continue; }
            let (x, y) = (fx as usize, fy as usize);
            let sz = map.surface_z[y * map.width + x];
            let k = sx;
            let p = row[k];
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
            row[k] = pack(c);
        }
    });
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
    // Shoulders: a half ellipse; the head above, hair on its crown.
    let (sw, sh) = (7.0 * scale, 6.0 * scale);
    let base = y + 5.0 * scale;
    for dy in -(sh as i64)..=0 {
        for dx in -(sw as i64 + 1)..=(sw as i64 + 1) {
            let e = (dx as f32 / sw).powi(2) + (dy as f32 / sh).powi(2);
            if e <= 1.0 { put(x as i64 + dx, base as i64 + dy, if e > 0.72 { INK } else { dress }, 0.97); }
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
    if s.role.is_some() { ink_disc(put, x - sw * 0.55, base - sh * 0.55, (1.8 * scale).max(1.5), [200.0, 160.0, 60.0], INK); }
    // A bandage, an office's headgear, a visitor's hat; and a bubble with what they are going
    // through (`status_ink`).
    super::status_ink::figure_marks(put, colony, i, hx, hy, hr);
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
    match (s.job, s.carrying) {
        _ if watch || matches!((s.job, s.carrying), (Job::Hunt(_), None)) => { line(put, gx + g * 0.5, gy - g * 0.6, gx + g * 0.5, gy + g * 1.6, INK); line(put, gx + g * 0.2, gy - g * 0.2, gx + g * 0.5, gy - g * 0.8, INK); line(put, gx + g * 0.8, gy - g * 0.2, gx + g * 0.5, gy - g * 0.8, INK); }
        (job, Some(kind)) => {
            // What they carry, as its glyph (the load's own kind: berries, a fish, a log...).
            let stuff = match job { Job::Haul(k) => colony.items.get(k).map(|it| it.what), _ => None }.unwrap_or(crate::colony::Stuff::of(kind));
            let _: ItemKind = kind;
            super::glyphs::draw(put, super::glyphs::Glyph::of_stuff(stuff), gx + g * 0.6, gy + g * 0.9, (g * 1.5).max(6.0), None);
        }
        (Job::Fell(_), _) => { line(put, gx, gy + g * 1.4, gx + g, gy, haft); line(put, gx + g * 0.6, gy - g * 0.2, gx + g * 1.2, gy + g * 0.4, INK); line(put, gx + g * 0.7, gy - g * 0.1, gx + g * 1.1, gy + g * 0.3, INK); }
        // A pick: for the quarry and the dig below.
        (Job::Quarry(_) | Job::Dig(..), _) => { line(put, gx + g * 0.5, gy + g * 1.4, gx + g * 0.5, gy, haft); line(put, gx - g * 0.1, gy + g * 0.3, gx + g * 1.1, gy + g * 0.3, INK); }
        // A hammer: for the builder and the crafter.
        (Job::Build | Job::Craft, _) => { line(put, gx, gy + g * 1.4, gx + g * 0.8, gy + g * 0.2, haft); line(put, gx + g * 0.4, gy - g * 0.1, gx + g * 1.2, gy + g * 0.5, INK); line(put, gx + g * 0.5, gy - g * 0.2, gx + g * 1.3, gy + g * 0.4, INK); }
        (Job::Forage(_), _) => {
            for k in 0..=12 { let a = std::f32::consts::PI * k as f32 / 12.0; put((gx + g * 0.6 + g * 0.6 * a.cos()) as i64, (gy + g * 0.6 + g * 0.6 * a.sin()) as i64, INK, 0.95); }
            line(put, gx, gy + g * 0.6, gx + g * 1.2, gy + g * 0.6, INK);
            put((gx + g * 0.5) as i64, (gy + g * 0.4) as i64, [196.0, 64.0, 70.0], 1.0);
        }
        (Job::Fish(_), _) => { line(put, gx, gy + g * 1.4, gx + g * 1.1, gy - g * 0.3, haft); line(put, gx + g * 1.1, gy - g * 0.3, gx + g * 1.1, gy + g * 1.2, [60.0, 80.0, 110.0]); }
        (Job::Sleep, _) => {
            // A small z.
            line(put, gx, gy, gx + g, gy, INK);
            line(put, gx + g, gy, gx, gy + g, INK);
            line(put, gx, gy + g, gx + g, gy + g, INK);
        }
        _ => {}
    }
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
    draw_colony_inner(colony, cam, buf, w, h, history, None, &mut placed);
}

/// `draw_colony`, with a mask of the pixels that show the surface (a level view's: the camp's
/// marks, its night and winter are drawn only there) and the labels placed so far.
fn draw_colony_inner(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>, mask: Option<&[bool]>, placed: &mut Vec<LabelBox>) {
    use crate::colony::ItemKind;
    let t = cam.tile_px;
    let to_screen = |x: f32, y: f32| ((x - cam.cx) * t + w as f32 / 2.0, (y - cam.cy) * t + h as f32 / 2.0);
    let shows = |k: usize| mask.map_or(true, |m| m[k]);
    // Worn ground first, under everything: a dusty wash where feet have gone often (15+ steps),
    // stronger on the lanes (120+), blended between cells with a ragged edge, so paths read as
    // paths rather than a dot on every cell; never on a roof or a wall.
    if colony.steps.len() == colony.map.width * colony.map.height {
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
                buf.par_chunks_mut(w).enumerate().for_each(|(sy, row)| {
                    let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
                    if fy < 0.0 || fy >= map.height as f32 { return; }
                    for sx in 0..w {
                        let k = sy * w + sx;
                        if !shows(k) { continue; }
                        let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
                        if fx < 0.0 || fx >= map.width as f32 { continue; }
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
    // Winter: in a hard winter the land pales toward snow, more in a deep freeze (the embark's
    // season, which the colony lives through; the map itself keeps its annual colours).
    if colony.hard_winter() {
        let snow: Rgb = [236.0, 234.0, 226.0];
        let a = if colony.frozen() { 0.42 } else { 0.28 };
        for (k, p) in buf.iter_mut().enumerate() {
            if !shows(k) { continue; }
            let old = [((*p >> 16) & 0xFF) as f32, ((*p >> 8) & 0xFF) as f32, (*p & 0xFF) as f32];
            // Darker ink (lines, buildings' walls) keeps more of itself than the open ground.
            let lum = (old[0] + old[1] + old[2]) / 765.0;
            *p = pack(mix(old, snow, a * lum.powf(0.7)));
        }
    }
    // Evil weather over the camp, snow in a deep freeze (`fx_ink`).
    super::fx_ink::draw_weather(colony, buf, w, h, mask);
    // Night: the map washes toward sea-ink blue, but for a warm glow round the fire and the watch.
    let dark = colony.darkness();
    if dark > 0.0 {
        let mut lights: Vec<(f32, f32, f32)> = vec![{ let (x, y) = to_screen(colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 + 0.5); (x, y, 7.0 * t) }];
        if let Some(i) = colony.watcher {
            let p = colony.draw_pos(i);
            let (x, y) = to_screen(p.0 + 0.5, p.1 + 0.5);
            lights.push((x, y, 3.0 * t));
        }
        let night: Rgb = [34.0, 48.0, 74.0];
        let warm: Rgb = [250.0, 196.0, 120.0];
        for y in 0..h {
            for x in 0..w {
                let mut glow = 0.0f32;
                for &(lx, ly, r) in &lights {
                    let d = ((x as f32 - lx).powi(2) + (y as f32 - ly).powi(2)).sqrt() / r;
                    if d < 1.0 { glow = glow.max((1.0 - d) * (1.0 - d) * (3.0 - 2.0 * (1.0 - d)).min(1.0)); }
                }
                let k = y * w + x;
                if !shows(k) { continue; }
                let p = buf[k];
                let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
                let c = mix(old, night, dark * (1.0 - glow));
                buf[k] = pack(mix(c, warm, 0.18 * dark * glow));
            }
        }
    }
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
                super::camp_ink::draw_cage(&mut put, cam, w, h, m.at, held, 1.0);
            }
            crate::colony::MarkKind::Scorch => {
                let ri = (r * 1.6) as i64;
                for dy in -ri..=ri { for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / ri as f32;
                    let n = ((dx * 7 + dy * 13).rem_euclid(5)) as f32 / 5.0;
                    if d < 1.0 { put(cx as i64 + dx, cy as i64 + dy, [70.0, 56.0, 46.0], (0.75 - d * 0.5) * (0.6 + 0.4 * n)); }
                } }
            }
            crate::colony::MarkKind::Stone => super::camp_ink::draw_stone_mark(&mut put, cam, w, h, m.at, &m.title),
        }
    }
    // Engravings on the hall's walls: a carved panel on the face toward the floor, a little
    // figure scratched in it (`colony::engrave`).
    for e in colony.engravings.iter().filter(|e| e.z == colony.hall_z && colony.hall_cells.contains(&e.from)) {
        let (wx, wy) = to_screen(e.wall.0 as f32 + 0.5, e.wall.1 as f32 + 0.5);
        let (dx, dy) = (e.from.0 as f32 - e.wall.0 as f32, e.from.1 as f32 - e.wall.1 as f32);
        let (cx, cy) = (wx + dx * t * 0.3, wy + dy * t * 0.3);
        let (hw, hh) = if dx != 0.0 { ((t * 0.12).max(1.5), (t * 0.36).max(3.0)) } else { ((t * 0.36).max(3.0), (t * 0.12).max(1.5)) };
        let (hwi, hhi) = (hw as i64, hh as i64);
        for yy in -hhi..=hhi { for xx in -hwi..=hwi {
            let edge = xx.abs() == hwi || yy.abs() == hhi;
            put(cx as i64 + xx, cy as i64 + yy, if edge { INK } else { [196.0, 178.0, 140.0] }, if edge { 0.9 } else { 0.8 });
        } }
        // The figure: a dot and a stroke, darker for a finer hand.
        let a = 0.5 + 0.08 * e.quality as f32;
        put(cx as i64, cy as i64 - 1, INK, a);
        put(cx as i64, cy as i64, INK, a);
        put(cx as i64, cy as i64 + 1, INK, a);
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
    super::fx_ink::draw_clash(colony, cam, &mut put, w, h);
    // Buildings going up, drawn by the share of loads laid: pegs and a line (a quarter), a
    // timber frame (to three fifths), then walls rising round the ring.
    for (at, bw, bh, share) in colony.rising() {
        let ring: Vec<(u16, u16)> = (0..bw).map(|dx| (dx, 0)).chain((1..bh).map(|dy| (bw - 1, dy))).chain((0..bw.saturating_sub(1)).rev().map(|dx| (dx, bh - 1))).chain((1..bh.saturating_sub(1)).rev().map(|dy| (0, dy))).collect();
        let (x0, y0) = to_screen(at.0 as f32, at.1 as f32);
        let (x1, y1) = to_screen((at.0 + bw) as f32, (at.1 + bh) as f32);
        // The pegged line.
        for x in x0 as i64..x1 as i64 { if (x / 3) % 2 == 0 { put(x, y0 as i64, INK, 0.6); put(x, y1 as i64 - 1, INK, 0.6); } }
        for y in y0 as i64..y1 as i64 { if (y / 3) % 2 == 0 { put(x0 as i64, y, INK, 0.6); put(x1 as i64 - 1, y, INK, 0.6); } }
        for (cx, cy) in [(x0, y0), (x1 - 1.0, y0), (x0, y1 - 1.0), (x1 - 1.0, y1 - 1.0)] { disc(&mut put, cx, cy, (t * 0.12).max(1.5), [120.0, 84.0, 50.0], INK); }
        if share >= 0.25 {
            // The frame: posts every other cell of the ring.
            for (k, &(dx, dy)) in ring.iter().enumerate() {
                if k % 2 != 0 { continue; }
                let (px, py) = to_screen(at.0 as f32 + dx as f32 + 0.5, at.1 as f32 + dy as f32 + 0.5);
                disc(&mut put, px, py, (t * 0.16).max(1.5), [150.0, 108.0, 66.0], INK);
            }
        }
        if share >= 0.6 {
            // Walls: the ring filled as far as the loads go.
            let n = ((share - 0.6) / 0.4 * ring.len() as f32).ceil() as usize;
            for &(dx, dy) in ring.iter().take(n) {
                let (px, py) = to_screen(at.0 as f32 + dx as f32, at.1 as f32 + dy as f32);
                for yy in py as i64..(py + t) as i64 { for xx in px as i64..(px + t) as i64 {
                    let edge = xx == px as i64 || yy == py as i64 || xx == (px + t) as i64 - 1 || yy == (py + t) as i64 - 1;
                    put(xx, yy, if edge { INK } else { [176.0, 140.0, 100.0] }, 0.9);
                } }
            }
        }
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
        } else { y - 16.0 * scale };
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
    // A banner for a great moment, for half a game day.
    if let Some((text, at)) = &colony.banner {
        if colony.clock.tick < at + 720 {
            let px = 26.0;
            let tw = super::fonts::width(text, super::fonts::Face::SmallCaps, px, 2.0);
            let (bx, by) = (w as f32 / 2.0 - tw / 2.0 - 24.0, 18.0);
            for y in by as usize..(by + px * 1.7) as usize {
                for x in bx.max(0.0) as usize..((bx + tw + 48.0) as usize).min(w) {
                    if y < h { let k = y * w + x; buf[k] = 0x00EA_DEC4; }
                }
            }
            super::fonts::draw(buf, w, h, bx + 24.0, by + 4.0, text, super::fonts::Face::SmallCaps, px, 2.0, 0x009A_2A1E, None);
        }
    }
}

/// The camp cut open from the side, in ink: the row `row` of the map from `x0` to `x1`, levels
/// from 9 above to 9 below the camp's ground. Rock and soil are parchment hatched in their
/// colour (ore flecked in its metal), open air pale, dug rooms a pale wash with inked walls,
/// water blue; settlers on the row (or a cell beside it) stand at their level as small figures;
/// a depth scale on the left counts levels from the camp's ground.
pub fn render_section_ink(colony: &crate::colony::Colony, row: usize, x0: usize, x1: usize, buf: &mut [u32], w: usize, h: usize) {
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
    let (ztop, levels) = (zc + 9, (zc + 9 - deepest + 3).clamp(19, 75));
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
    let title = format!("Section through row {} ({} levels; 0 is the camp's ground)", row, levels);
    super::fonts::draw(buf, w, h, margin, 10.0, &title, super::fonts::Face::Italic, 16.0, 0.0, pack(INK), None);
}

/// Everything over a level slice: near the camp's ground (two levels either side) the camp as
/// the surface view draws it (its fire, store, marks, crops, creatures, night and winter, those
/// on the surface), then the delve's rooms and those below at this level; deeper or higher,
/// the delve alone. So the level view and the surface view show the same camp.
pub fn draw_level(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    let zc = colony.map.surface_z[colony.camp.1 as usize * colony.map.width + colony.camp.0 as usize];
    let near_ground = (cam.z - zc).abs() <= 2;
    let mut placed = Vec::new();
    if near_ground {
        // The camp's marks, night and winter only where this level shows the surface: not on
        // the rock cut through below it, nor on the rooms dug there.
        let map = &colony.map;
        let t = cam.tile_px;
        let (x0, y0) = ((cam.cx - w as f32 / 2.0 / t).floor().max(0.0) as usize, (cam.cy - h as f32 / 2.0 / t).floor().max(0.0) as usize);
        let (x1, y1) = (((cam.cx + w as f32 / 2.0 / t) as usize + 1).min(map.width), ((cam.cy + h as f32 / 2.0 / t) as usize + 1).min(map.height));
        let gw = x1.saturating_sub(x0);
        let cells: Vec<bool> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| level_shows_surface(map, x, y, cam.z)).collect();
        let mut mask = vec![true; w * h];
        for sy in 0..h {
            let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
            for sx in 0..w {
                let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
                if fx < x0 as f32 || fy < y0 as f32 || fx >= x1 as f32 || fy >= y1 as f32 { continue; }
                mask[sy * w + sx] = cells[(fy as usize - y0) * gw + (fx as usize - x0)];
            }
        }
        draw_colony_inner(colony, cam, buf, w, h, history, Some(&mask), &mut placed);
    }
    draw_delve_inner(colony, cam, buf, w, h, history, near_ground, &mut placed);
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
    // The hatch over the stair below the first cavern: planks bound with iron, an iron ring.
    if let Some((p, _)) = colony.hatch.filter(|h| h.1 == cam.z || h.1 == cam.z + 1) {
        super::furniture::hatch(&mut cells.pen(&mut put, p.0 as f32, p.1 as f32));
    }
    // Engravings on this level's walls: a carved panel on the face toward the floor.
    for e in colony.engravings.iter().filter(|e| e.z == cam.z) {
        let (wx, wy) = to_screen(e.wall.0 as f32 + 0.5, e.wall.1 as f32 + 0.5);
        let (dx, dy) = (e.from.0 as f32 - e.wall.0 as f32, e.from.1 as f32 - e.wall.1 as f32);
        let (cx, cy) = (wx + dx * t * 0.42, wy + dy * t * 0.42);
        let (hw, hh) = if dx != 0.0 { ((t * 0.08).max(1.5), (t * 0.34).max(3.0)) } else { ((t * 0.34).max(3.0), (t * 0.08).max(1.5)) };
        let fine = if e.quality >= 3 { [196.0, 160.0, 80.0] } else { [170.0, 150.0, 120.0] };
        for yy in (cy - hh) as i64..=(cy + hh) as i64 { for xx in (cx - hw) as i64..=(cx + hw) as i64 {
            let edge = (xx as f32 - (cx - hw)).abs() < 1.0 || (xx as f32 - (cx + hw)).abs() < 1.0 || (yy as f32 - (cy - hh)).abs() < 1.0 || (yy as f32 - (cy + hh)).abs() < 1.0;
            put(xx, yy, if edge { INK } else { fine }, 0.95);
        } }
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
        letter(buf, w, h, placed, w as f32 / 2.0, 14.0, &cap, super::fonts::Face::Italic, 16.0, 0.0, 0x0030_1E14);
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
            super::folk::draw(put, &f, x, y, scale * 1.2, left, strike, a);
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
                let mut pen = super::ink::Pen::new(put, x, y + 7.0 * scale - px * 0.4, px).facing_left(left).faint(a);
                pen.ellipse_f(0.32, -0.08, 0.05, 0.05, [180.0, 40.0, 30.0], super::ink::Finish::Plain);
            }
        }
    }
}
