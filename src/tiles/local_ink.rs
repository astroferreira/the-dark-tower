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
        let wash = (0..w * h).into_par_iter().map(|k| match top[k] {
            Top::Water(d) => water_wash(d),
            _ => { let c = wash(floor_of(k)); let d = sunk(k); if d >= 2 { mix(c, [70.0, 60.0, 50.0], (0.18 * d as f32).min(0.5)) } else { c } }
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
        let same = |dx: i64, dy: i64| v.inside(cx + dx, cy + dy) && v.map.roofs[((cy + dy) as usize) * v.map.width + (cx + dx) as usize] == roof;
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
            // A cut stump: a ring of bark round pale wood.
            let r = ((u - 0.5).powi(2) + (w - 0.5).powi(2)).sqrt();
            if r < 0.1 { c = mix(c, [196.0, 168.0, 120.0], 0.8); }
            if (r - 0.1).abs() < line * 0.8 { c = mix(c, INK, 0.5); }
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
    for sy in 0..h {
        let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
        for sx in 0..w {
            let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
            if fx < 0.0 || fy < 0.0 || fx >= map.width as f32 || fy >= map.height as f32 { continue; }
            let (x, y) = (fx as usize, fy as usize);
            let sz = map.surface_z[y * map.width + x];
            let k = sy * w + sx;
            let p = buf[k];
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
                    let mut c = mix(paper, wash(floor), 0.5);
                    if ((fx * 4.0).fract() < 0.08) || ((fy * 4.0).fract() < 0.08) { c = mix(c, INK, 0.25); }
                    if body.shape == Shape::Stair || floor.shape == Shape::Stair { if (v * 5.0).fract() < 0.28 && u > 0.12 && u < 0.88 { c = mix(c, INK, 0.5); } }
                    if open_near(2) { INK } else { c }
                } else if kd == 0 {
                    let base = mix(wash(body), INK, 0.3);
                    if ((fx + fy) * 5.0).fract() < 0.3 { mix(base, INK, 0.3) } else { base }
                } else if floor.water > 0 { mix(water_wash(floor.water as f32), paper, 0.3) } else { mix(here, paper, 0.72) }
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
                    // The magma sea: glowing orange, darker crust-veins across it.
                    let vein = unit((fx * 3.0) as i64, (fy * 3.0) as i64, 0x3A6) < 0.25;
                    if vein { [120.0, 40.0, 20.0] } else { mix([236.0, 120.0, 30.0], [200.0, 50.0, 20.0], mottle(fx, fy, 3.0, 0x3A7)) }
                } else if body.water > 0 && kd != 0 {
                    water_wash(body.water as f32)
                } else if kd == 0 {
                    // Rock or soil at standing height, hatched in its colour.
                    let base = mix(paper, wash(body), 0.5);
                    let (gx, gy) = (fx * 6.0, fy * 6.0);
                    let hatch = ((gx + gy) as i64).rem_euclid(3) == 0;
                    let mut c = if hatch { mix(base, INK, 0.18) } else { base };
                    if let crate::local::Material::Ore(r) = body.material {
                        let col = crate::lore::resource_color(r);
                        if unit((fx * 5.0) as i64, (fy * 5.0) as i64, 0x0E) < 0.35 { c = mix(c, [col[0] as f32, col[1] as f32, col[2] as f32], 0.85); }
                    }
                    if crate::local::gem_in(map, x, y, zu + 1).is_some() && ((u - 0.5).abs() + (v - 0.5).abs()) < 0.16 { c = mix(c, [150.0, 60.0, 120.0], 0.6); }
                    if map.is_aquifer(x, y, zu + 1) && unit((fx * 7.0) as i64, (fy * 7.0) as i64, 0xA9) < 0.12 { c = mix(c, [70.0, 110.0, 160.0], 0.7); }
                    if edge_to(1) || edge_to(2) { c = INK; }
                    c
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
                        let (cu, cv) = (u - 0.5, v - 0.5);
                        let on = |a: f32| (a - cu.abs() * 1.2).abs() < 0.07;
                        if (up && cv < 0.0 && on(-cv - 0.05)) || (down && cv > 0.0 && on(cv - 0.05)) { c = [150.0, 40.0, 30.0]; }
                    }
                    if edge_to(0) { c = INK; }
                    c
                } else {
                    // Open dark: a cavern's air, a shaft, the space over a lower floor.
                    let c = mix(paper, [62.0, 54.0, 66.0], 0.75);
                    if edge_to(0) { INK } else { c }
                }
            };
            buf[k] = pack(c);
        }
    }
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

pub fn draw_colony(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    use crate::colony::{ItemKind, Job};
    let t = cam.tile_px;
    let to_screen = |x: f32, y: f32| ((x - cam.cx) * t + w as f32 / 2.0, (y - cam.cy) * t + h as f32 / 2.0);
    // Winter: in a hard winter the land pales toward snow, more in a deep freeze (the embark's
    // season, which the colony lives through; the map itself keeps its annual colours).
    if colony.hard_winter() {
        let snow: Rgb = [236.0, 234.0, 226.0];
        let a = if colony.frozen() { 0.42 } else { 0.28 };
        for p in buf.iter_mut() {
            let old = [((*p >> 16) & 0xFF) as f32, ((*p >> 8) & 0xFF) as f32, (*p & 0xFF) as f32];
            // Darker ink (lines, buildings' walls) keeps more of itself than the open ground.
            let lum = (old[0] + old[1] + old[2]) / 765.0;
            *p = pack(mix(old, snow, a * lum.powf(0.7)));
        }
    }
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
                let p = buf[k];
                let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
                let c = mix(old, night, dark * (1.0 - glow));
                buf[k] = pack(mix(c, warm, 0.18 * dark * glow));
            }
        }
    }
    let mut put = |x: i64, y: i64, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
        let k = y as usize * w + x as usize;
        let p = buf[k];
        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
        buf[k] = pack(mix(old, c, a));
    };
    let disc = |put: &mut dyn FnMut(i64, i64, Rgb, f32), cx: f32, cy: f32, r: f32, fill: Rgb, ring: Rgb| {
        let rr = r.ceil() as i64 + 1;
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                let d = ((dx as f32 + 0.5 - (cx.fract())).powi(2) + (dy as f32 + 0.5 - (cy.fract())).powi(2)).sqrt();
                let (x, y) = (cx.floor() as i64 + dx, cy.floor() as i64 + dy);
                if d <= r - 1.0 { put(x, y, fill, 1.0); } else if d <= r + 0.3 { put(x, y, ring, 0.95); }
            }
        }
    };
    // The camp: a fire ring.
    let (fx, fy) = to_screen(colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 + 0.5);
    disc(&mut put, fx, fy, (t * 0.45).max(3.0), [214.0, 120.0, 60.0], INK);
    disc(&mut put, fx, fy, (t * 0.18).max(1.5), [240.0, 200.0, 110.0], [214.0, 120.0, 60.0]);
    // Things lying about (stored ones are drawn as a heap at the camp).
    let (mut logs, mut food) = (0, 0);
    for it in &colony.items {
        if it.stored { match it.kind { ItemKind::Log | ItemKind::Stone => logs += 1, ItemKind::Food => food += 1 }; continue; }
        let (x, y) = to_screen(it.at.0 as f32 + 0.5, it.at.1 as f32 + 0.5);
        let col = match it.kind { ItemKind::Log => [150.0, 104.0, 64.0], ItemKind::Stone => [168.0, 166.0, 158.0], ItemKind::Food => [196.0, 64.0, 70.0] };
        disc(&mut put, x, y, (t * 0.18).max(1.5), col, INK);
    }
    for k in 0..logs.min(12) {
        let (x, y) = to_screen(colony.camp.0 as f32 + 1.6 + (k % 4) as f32 * 0.22, colony.camp.1 as f32 + 0.2 + (k / 4) as f32 * 0.22);
        disc(&mut put, x, y, (t * 0.12).max(1.2), [150.0, 104.0, 64.0], INK);
    }
    for k in 0..food.min(16) {
        let (x, y) = to_screen(colony.camp.0 as f32 - 0.8 + (k % 4) as f32 * 0.2, colony.camp.1 as f32 + 0.2 + (k / 4) as f32 * 0.2);
        disc(&mut put, x, y, (t * 0.1).max(1.0), [196.0, 64.0, 70.0], INK);
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
            crate::colony::StoneKind::Hall => {
                let r = (t * 0.35).max(3.0) as i64;
                for dy in -r..=r { for dx in -r..=r {
                    let edge = dx.abs() == r || dy.abs() == r;
                    put(cx as i64 + dx, cy as i64 + dy, if edge { INK } else { [176.0, 168.0, 150.0] }, 0.95);
                } }
            }
            crate::colony::StoneKind::Grove => {
                let r = (crate::colony::GROVE_RADIUS as f32 + 0.5) * t;
                let steps = (r * 6.3) as i64 + 8;
                for k in 0..steps {
                    if k % 6 >= 4 { continue; }
                    let a = k as f32 / steps as f32 * std::f32::consts::TAU;
                    for o in [0.0, 1.0, 2.0] { put((cx + (r - o) * a.cos()) as i64, (cy + (r - o) * a.sin()) as i64, INK, 0.8); }
                }
                disc(&mut put, cx, cy, (t * 0.25).max(2.0), [176.0, 168.0, 150.0], INK);
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
        let (cx, cy) = to_screen(m.at.0 as f32 + 0.5, m.at.1 as f32 + 0.5);
        let r = (t * 0.7).max(5.0);
        match m.kind {
            crate::colony::MarkKind::Grave => {
                let ri = r as i64;
                for dy in -ri / 2..=ri / 2 { for dx in -ri..=ri {
                    if (dx * dx) as f32 / (r * r) + (dy * dy * 4) as f32 / (r * r) <= 1.0 { put(cx as i64 + dx, cy as i64 + dy + ri / 3, [150.0, 128.0, 96.0], 0.9); }
                } }
                for k in -ri..=ri / 3 { put(cx as i64, cy as i64 + k, INK, 0.95); }
                for k in -ri / 2..=ri / 2 { put(cx as i64 + k, cy as i64 - ri / 2, INK, 0.95); }
            }
            crate::colony::MarkKind::Cage => {
                // A barred square; dark within when something sits caged.
                let ri = (r * 0.8) as i64;
                let full = m.title.starts_with("The cage of");
                for dy in -ri..=ri { for dx in -ri..=ri {
                    let edge = dx.abs() == ri || dy.abs() == ri;
                    let bar = dx % 2 == 0;
                    if edge || bar { put(cx as i64 + dx, cy as i64 + dy, INK, 0.9); }
                    else if full { put(cx as i64 + dx, cy as i64 + dy, [70.0, 50.0, 46.0], 0.85); }
                } }
            }
            crate::colony::MarkKind::Scorch => {
                let ri = (r * 1.6) as i64;
                for dy in -ri..=ri { for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / ri as f32;
                    let n = ((dx * 7 + dy * 13).rem_euclid(5)) as f32 / 5.0;
                    if d < 1.0 { put(cx as i64 + dx, cy as i64 + dy, [70.0, 56.0, 46.0], (0.75 - d * 0.5) * (0.6 + 0.4 * n)); }
                } }
            }
            crate::colony::MarkKind::Stone => {
                let (rw, rh) = ((r * 0.5) as i64, r as i64);
                for dy in -rh..=rh { for dx in -rw..=rw {
                    let edge = dx.abs() == rw || dy.abs() == rh;
                    put(cx as i64 + dx, cy as i64 + dy, if edge { INK } else { [176.0, 168.0, 150.0] }, 0.95);
                } }
            }
        }
    }
    // Engravings on the hall's walls: a carved panel on the face toward the floor, a little
    // figure scratched in it (`colony::engrave`).
    for e in &colony.engravings {
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
    // Worn ground: a track where feet have gone often (15+ steps), a lane where most (120+).
    if colony.steps.len() == colony.map.width * colony.map.height {
        let (x0, y0) = ((cam.cx - w as f32 / 2.0 / t).max(0.0) as usize, (cam.cy - h as f32 / 2.0 / t).max(0.0) as usize);
        let (x1, y1) = (((cam.cx + w as f32 / 2.0 / t) as usize + 1).min(colony.map.width), ((cam.cy + h as f32 / 2.0 / t) as usize + 1).min(colony.map.height));
        for cy in y0..y1 { for cx in x0..x1 {
            let n = colony.steps[cy * colony.map.width + cx];
            if n < 15 { continue; }
            let a = if n >= 120 { 0.6 } else { 0.3 + 0.3 * (n as f32 - 15.0) / 105.0 };
            let (px, py) = to_screen(cx as f32 + 0.5, cy as f32 + 0.5);
            let r = (t * if n >= 120 { 0.6 } else { 0.45 }).max(2.0);
            let ri = r.ceil() as i64;
            for dy in -ri..=ri { for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
                if d < 1.0 { put(px as i64 + dx, py as i64 + dy, [132.0, 104.0, 72.0], (a + 0.15) * (1.0 - d * d)); }
            } }
        } }
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
        let (skin, hair, dress) = looks[i];
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
        if s.role.is_some() { disc(&mut put, x - sw * 0.55, base - sh * 0.55, (1.8 * scale).max(1.5), [200.0, 160.0, 60.0], INK); }
        // What they are doing, beside the head.
        let watch = colony.watcher == Some(i) && colony.clock.is_night();
        let (gx, gy) = (x + sw + 2.0, hy - hr);
        let g = (5.0 * scale).max(3.0);
        let line = |put: &mut dyn FnMut(i64, i64, Rgb, f32), x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb| {
            let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0) as i32 + 1;
            for k in 0..=n { let f = k as f32 / n as f32; put((x0 + (x1 - x0) * f) as i64, (y0 + (y1 - y0) * f) as i64, c, 0.95); }
        };
        match (s.job, s.carrying) {
            _ if watch => { line(&mut put, gx + g * 0.5, gy - g * 0.6, gx + g * 0.5, gy + g * 1.6, INK); line(&mut put, gx + g * 0.2, gy - g * 0.2, gx + g * 0.5, gy - g * 0.8, INK); line(&mut put, gx + g * 0.8, gy - g * 0.2, gx + g * 0.5, gy - g * 0.8, INK); }
            (_, Some(kind)) => {
                let col = match kind { ItemKind::Log => [150.0, 104.0, 64.0], ItemKind::Stone => [168.0, 166.0, 158.0], ItemKind::Food => [196.0, 64.0, 70.0] };
                disc(&mut put, gx + g * 0.5, gy + g * 0.9, g * 0.55, col, INK);
            }
            (Job::Fell(_), _) => { line(&mut put, gx, gy + g * 1.4, gx + g, gy, [120.0, 84.0, 50.0]); line(&mut put, gx + g * 0.6, gy - g * 0.2, gx + g * 1.2, gy + g * 0.4, INK); line(&mut put, gx + g * 0.7, gy - g * 0.1, gx + g * 1.1, gy + g * 0.3, INK); }
            (Job::Quarry(_), _) => { line(&mut put, gx + g * 0.5, gy + g * 1.4, gx + g * 0.5, gy, [120.0, 84.0, 50.0]); line(&mut put, gx - g * 0.1, gy + g * 0.3, gx + g * 1.1, gy + g * 0.3, INK); }
            (Job::Build, _) => { line(&mut put, gx, gy + g * 1.4, gx + g * 0.8, gy + g * 0.2, [120.0, 84.0, 50.0]); line(&mut put, gx + g * 0.4, gy - g * 0.1, gx + g * 1.2, gy + g * 0.5, INK); line(&mut put, gx + g * 0.5, gy - g * 0.2, gx + g * 1.3, gy + g * 0.4, INK); }
            (Job::Forage(_), _) => {
                for k in 0..=12 { let a = std::f32::consts::PI * k as f32 / 12.0; put((gx + g * 0.6 + g * 0.6 * a.cos()) as i64, (gy + g * 0.6 + g * 0.6 * a.sin()) as i64, INK, 0.95); }
                line(&mut put, gx, gy + g * 0.6, gx + g * 1.2, gy + g * 0.6, INK);
                put((gx + g * 0.5) as i64, (gy + g * 0.4) as i64, [196.0, 64.0, 70.0], 1.0);
            }
            (Job::Fish(_), _) => { line(&mut put, gx, gy + g * 1.4, gx + g * 1.1, gy - g * 0.3, [120.0, 84.0, 50.0]); line(&mut put, gx + g * 1.1, gy - g * 0.3, gx + g * 1.1, gy + g * 1.2, [60.0, 80.0, 110.0]); }
            (Job::Sleep, _) => {
                // A small z.
                line(&mut put, gx, gy, gx + g, gy, INK);
                line(&mut put, gx + g, gy, gx, gy + g, INK);
                line(&mut put, gx, gy + g, gx + g, gy + g, INK);
            }
            _ => {}
        }
        if t >= 6.0 { labels.push((x, hy - hr - 2.0, s.name.clone())); }
    }
    // Creatures: the raid's attackers and the night's wolves.
    for c in &colony.creatures {
        let (x, y) = to_screen(c.pos.0 as f32 + 0.5, c.pos.1 as f32 + 0.5);
        match c.kind {
            crate::colony::creatures::CreatureKind::Beast => {
                // A dark bulk at its size, inked, with two red eyes.
                let (rx, ry) = ((t * 0.9 * c.size).max(12.0), (t * 0.6 * c.size).max(8.0));
                let (ix, iy) = (rx.ceil() as i64 + 1, ry.ceil() as i64 + 1);
                for dy in -iy..=iy { for dx in -ix..=ix {
                    let e = (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2);
                    if e <= 1.0 { put(x as i64 + dx, y as i64 + dy, if e > 0.8 { INK } else { [52.0, 40.0, 40.0] }, 0.97); }
                } }
                for ex in [-0.35f32, 0.35] { put((x + rx * ex) as i64, (y - ry * 0.3) as i64, [210.0, 40.0, 30.0], 1.0); put((x + rx * ex) as i64 + 1, (y - ry * 0.3) as i64, [210.0, 40.0, 30.0], 1.0); }
            }
            crate::colony::creatures::CreatureKind::Raider => {
                let r = (5.0 * scale).max(3.0);
                disc(&mut put, x, y + r * 0.6, r, [110.0, 32.0, 30.0], INK);
                disc(&mut put, x, y - r * 0.9, r * 0.6, [150.0, 150.0, 150.0], INK);
            }
            crate::colony::creatures::CreatureKind::Besieger => {
                // A raider at the besiegers' fires: the figure, and the fire's glow at its feet.
                let r = (5.0 * scale).max(3.0);
                disc(&mut put, x + r * 1.2, y + r * 1.3, r * 0.55, [226.0, 128.0, 40.0], [150.0, 60.0, 20.0]);
                disc(&mut put, x, y + r * 0.6, r, [110.0, 32.0, 30.0], INK);
                disc(&mut put, x, y - r * 0.9, r * 0.6, [150.0, 150.0, 150.0], INK);
            }
            crate::colony::creatures::CreatureKind::Game => {
                // A grazer: a small brown body with a head.
                let (rx, ry) = ((t * 0.4).max(4.0), (t * 0.22).max(2.5));
                let (ix, iy) = (rx.ceil() as i64 + 1, ry.ceil() as i64 + 1);
                for dy in -iy..=iy { for dx in -ix..=ix {
                    let e = (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2);
                    if e <= 1.0 { put(x as i64 + dx, y as i64 + dy, if e > 0.7 { INK } else { [150.0, 112.0, 72.0] }, 0.95); }
                } }
                disc(&mut put, x + rx, y - ry, (t * 0.12).max(1.6), [150.0, 112.0, 72.0], INK);
            }
            crate::colony::creatures::CreatureKind::Pet => {
                // A kept animal: smaller and paler than the herd, with a red collar.
                let (rx, ry) = ((t * 0.32).max(3.5), (t * 0.18).max(2.2));
                let (ix, iy) = (rx.ceil() as i64 + 1, ry.ceil() as i64 + 1);
                for dy in -iy..=iy { for dx in -ix..=ix {
                    let e = (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2);
                    if e <= 1.0 { put(x as i64 + dx, y as i64 + dy, if e > 0.7 { INK } else { [190.0, 150.0, 100.0] }, 0.95); }
                } }
                disc(&mut put, x + rx, y - ry, (t * 0.11).max(1.5), [190.0, 150.0, 100.0], INK);
                put((x + rx * 0.7) as i64, (y - ry * 0.6) as i64, [180.0, 40.0, 30.0], 1.0);
                put((x + rx * 0.7) as i64 + 1, (y - ry * 0.6) as i64, [180.0, 40.0, 30.0], 1.0);
            }
            crate::colony::creatures::CreatureKind::Trader => {
                // A trader with a pack: a figure in brown with an ochre bundle.
                let r = (5.0 * scale).max(3.0);
                disc(&mut put, x, y + r * 0.6, r, [128.0, 96.0, 60.0], INK);
                disc(&mut put, x + r * 0.9, y + r * 0.2, r * 0.6, [196.0, 160.0, 80.0], INK);
                disc(&mut put, x, y - r * 0.9, r * 0.6, [214.0, 180.0, 150.0], INK);
            }
            crate::colony::creatures::CreatureKind::Wolf => {
                let (rx, ry) = ((t * 0.45).max(5.0), (t * 0.22).max(3.0));
                let (ix, iy) = (rx.ceil() as i64 + 1, ry.ceil() as i64 + 1);
                for dy in -iy..=iy { for dx in -ix..=ix {
                    let e = (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2);
                    if e <= 1.0 { put(x as i64 + dx, y as i64 + dy, if e > 0.75 { INK } else { [120.0, 118.0, 112.0] }, 0.95); }
                } }
            }
        }
    }
    // The attackers are named on the map (one label for a band).
    if let Some(c) = colony.creatures.iter().find(|c| !matches!(c.kind, crate::colony::creatures::CreatureKind::Wolf | crate::colony::creatures::CreatureKind::Game | crate::colony::creatures::CreatureKind::Pet) && !c.leaving) {
        let (x, y) = to_screen(c.pos.0 as f32 + 0.5, c.pos.1 as f32 + 0.5);
        let tw = super::fonts::width(&c.name, super::fonts::Face::Italic, 14.0, 0.0);
        super::fonts::draw(buf, w, h, x - tw / 2.0, y - (t * 0.9 * c.size).max(12.0) - 18.0, &c.name, super::fonts::Face::Italic, 14.0, 0.0, 0x009A_2A1E, Some(0x00EE_E4CC));
    }
    // Under a roof: how many are inside, lettered on it.
    for (roof, n) in under_roof {
        let (x, y) = to_screen(roof.0 as f32 + 0.5, roof.1 as f32 + 0.5);
        let txt = format!("{} within", n);
        let tw = super::fonts::width(&txt, super::fonts::Face::Italic, 13.0, 0.0);
        super::fonts::draw(buf, w, h, x - tw / 2.0, y - 8.0, &txt, super::fonts::Face::Italic, 13.0, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
    }
    // Below: how many are down in the delve, lettered by its mouth.
    if let (Some(m), true) = (colony.delve_mouth, below > 0) {
        let (x, y) = to_screen(m.0 as f32 + 0.5, m.1 as f32 + 1.6);
        let txt = format!("{} below", below);
        let tw = super::fonts::width(&txt, super::fonts::Face::Italic, 13.0, 0.0);
        super::fonts::draw(buf, w, h, x - tw / 2.0, y, &txt, super::fonts::Face::Italic, 13.0, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
    }
    // Names in the map's hand, stepping aside from each other (above, below, right, left), or
    // left out when there is no room (hover shows them).
    let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new();
    for (x, y, label) in labels {
        let px = 12.0;
        let lw = super::fonts::width(&label, super::fonts::Face::Italic, px, 0.0);
        let lh = 13.0;
        let spots = [(x - lw / 2.0, y - lh), (x - lw / 2.0, y + 14.0 * scale + 6.0), (x + 9.0 * scale + 6.0, y), (x - 9.0 * scale - 6.0 - lw, y)];
        let free = spots.iter().copied().find(|&(lx, ly)| !placed.iter().any(|&(qx, qy, qw, qh)| lx < qx + qw + 3.0 && qx < lx + lw + 3.0 && ly < qy + qh && qy < ly + lh));
        let Some((lx, ly)) = free else { continue };
        placed.push((lx, ly, lw, lh));
        super::fonts::draw(buf, w, h, lx, ly, &label, super::fonts::Face::Italic, px, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
    }
    // The patron's names: the settlement at its camp, named places where they lie.
    let mut names: Vec<(f32, f32, String, f32)> = colony.place_names.iter().map(|(p, n)| (p.0 as f32 + 0.5, p.1 as f32 + 0.5, n.clone(), 15.0)).collect();
    if let Some(n) = &colony.name { names.push((colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 - 2.5, n.clone(), 22.0)); }
    for (x, y, n, px) in names {
        let (sx, sy) = to_screen(x, y);
        let face = if px > 18.0 { super::fonts::Face::SmallCaps } else { super::fonts::Face::Italic };
        let tw = super::fonts::width(&n, face, px, 1.0);
        super::fonts::draw(buf, w, h, sx - tw / 2.0, sy - px, &n, face, px, 1.0, 0x0030_1E14, Some(0x00EE_E4CC));
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
            let c: Rgb = if z < 0 || z as usize >= map.depth { sky } else {
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
                    // A stair: steps climbing across the cell.
                    let (u, v) = (fx.fract(), fz.fract());
                    let step = ((u + v) * 3.0).fract() < 0.3;
                    if step { mix(paper, INK, 0.6) } else { mix(paper, [205.0, 190.0, 160.0], 0.6) }
                } else if cell.material == Material::Magma {
                    mix([236.0, 120.0, 30.0], [200.0, 50.0, 20.0], unit(sx as i64 / 4, sy as i64 / 4, 0x3A6))
                } else if cell.water > 0 {
                    water_wash(cell.water as f32)
                } else if map.cavern_at(x, row, z).is_some() {
                    // A cavern: dark air, a fungus stalk rising from the floor here and there.
                    let floor = z >= 1 && map.cell(x, row, (z - 1) as usize).shape == Shape::Floor && map.cell(x, row, (z - 1) as usize).plant == crate::local::Plant::Tree(crate::local::TreeKind::Fungus);
                    let (u, _) = (fx.fract(), fz.fract());
                    if floor && (u - 0.5).abs() < 0.18 { mix(paper, [150.0, 110.0, 140.0], 0.8) } else { mix(paper, [62.0, 54.0, 66.0], 0.82) }
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
    // Settlers on the row stand at their level.
    for (i, s) in colony.settlers.iter().enumerate().filter(|(_, s)| s.alive && (s.pos.1 as i32 - row as i32).abs() <= 1) {
        let x = s.pos.0 as usize;
        if x < x0 || x >= x1 { continue; }
        let z = colony.here3(i).2 + 1;
        let (cx, base) = (margin + (x - x0) as f32 * cw + cw / 2.0, top + (ztop - z + 1) as f32 * ch);
        let fig_h = ch * 0.9;
        for yy in (base - fig_h) as i64..base as i64 {
            let head = (yy as f32) < base - fig_h * 0.7;
            let half = if head { fig_h * 0.14 } else { fig_h * 0.2 };
            for xx in (cx - half) as i64..(cx + half) as i64 {
                if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h { buf[yy as usize * w + xx as usize] = pack(if head { [200.0, 160.0, 120.0] } else { [150.0, 60.0, 40.0] }); }
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
    if near_ground { draw_colony(colony, cam, buf, w, h, history); }
    draw_delve_inner(colony, cam, buf, w, h, history, near_ground);
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
    draw_delve_inner(colony, cam, buf, w, h, history, false);
}

fn draw_delve_inner(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, history: Option<&crate::history::world_state::WorldHistory>, surface_drawn: bool) {
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
    for r in colony.rooms.iter().filter(|r| r.z == cam.z) {
        match r.kind {
            RoomKind::Bedroom => {
                // The door: a plank in the doorway (the room's first cell).
                if let Some(&d) = r.cells.first() {
                    let (x, y) = to_screen(d.0 as f32, d.1 as f32);
                    rect(&mut put, x + t * 0.2, y + t * 0.2, x + t * 0.8, y + t * 0.8, [150.0, 110.0, 70.0]);
                }
                if let Some(b) = r.bed {
                    let (x, y) = to_screen(b.0 as f32, b.1 as f32);
                    // A bed (once made at the workshop): frame, blanket, pillow; else a pallet of straw.
                    if r.furnished.is_some() {
                        rect(&mut put, x + t * 0.15, y + t * 0.1, x + t * 0.85, y + t * 0.9, [176.0, 120.0, 90.0]);
                        rect(&mut put, x + t * 0.25, y + t * 0.15, x + t * 0.75, y + t * 0.35, [236.0, 228.0, 210.0]);
                    } else {
                        for yy in (y + t * 0.3) as i64..(y + t * 0.8) as i64 { for xx in (x + t * 0.2) as i64..(x + t * 0.8) as i64 { if (xx + yy) % 3 == 0 { put(xx, yy, [196.0, 170.0, 100.0], 0.7); } } }
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
                    // The long table and its two benches.
                    let (x, y) = to_screen(c.0 as f32 - 2.0, c.1 as f32);
                    rect(&mut put, x + t * 0.1, y + t * 0.2, x + t * 4.9, y + t * 0.8, [160.0, 116.0, 74.0]);
                    rect(&mut put, x + t * 0.3, y - t * 0.25, x + t * 4.7, y - t * 0.05, [140.0, 100.0, 64.0]);
                    rect(&mut put, x + t * 0.3, y + t * 1.05, x + t * 4.7, y + t * 1.25, [140.0, 100.0, 64.0]);
                    let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 1.6);
                    labels.push((lx, ly, "the great hall".into(), false));
                }
            }
            RoomKind::Hall | RoomKind::Cellar => {
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
                    if let Some(o) = r.owner {
                        rect(&mut put, x + t * 0.2, y + t * 0.12, x + t * 0.8, y + t * 0.88, [120.0, 112.0, 104.0]);
                        for k in 0..(t * 0.5) as i64 { put((x + t * 0.5) as i64, (y + t * 0.25) as i64 + k, INK, 0.9); }
                        for k in 0..(t * 0.3) as i64 { put((x + t * 0.35) as i64 + k, (y + t * 0.4) as i64, INK, 0.9); }
                        if t >= 9.0 { labels.push((x + t * 0.5, y - 2.0, colony.settlers[o].name.clone(), false)); }
                    } else {
                        for xx in (x + t * 0.2) as i64..(x + t * 0.8) as i64 { put(xx, (y + t * 0.5) as i64, INK, 0.4); }
                    }
                }
            }
            RoomKind::Workshop => {
                // The benches: two heavy tables with tools on them, and the room named.
                if let Some(c) = r.bed {
                    for (ox, oy) in [(-1.0f32, -1.0f32), (1.0, 1.0)] {
                        let (x, y) = to_screen(c.0 as f32 + ox, c.1 as f32 + oy);
                        rect(&mut put, x + t * 0.05, y + t * 0.25, x + t * 1.9, y + t * 0.75, [150.0, 112.0, 76.0]);
                        for k in 0..(t * 0.4) as i64 { put((x + t * 0.6) as i64 + k, (y + t * 0.45) as i64, INK, 0.8); }
                    }
                    let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 1.8);
                    labels.push((lx, ly, "the workshops".into(), false));
                }
            }
            RoomKind::Farm => {
                // Plots in rows: pale caps of what grows in the dark.
                for &c in &r.cells {
                    let (x, y) = to_screen(c.0 as f32, c.1 as f32);
                    for k in 0..3 {
                        let (cx, cy) = (x + t * (0.25 + 0.25 * k as f32), y + t * 0.5);
                        let rr = (t * 0.1).max(1.0);
                        for yy in (cy - rr) as i64..=(cy + rr) as i64 { for xx in (cx - rr) as i64..=(cx + rr) as i64 { put(xx, yy, [176.0, 140.0, 170.0], 0.85); } }
                    }
                }
                if let Some(c) = r.bed { let (lx, ly) = to_screen(c.0 as f32 + 0.5, c.1 as f32 - 2.6); labels.push((lx, ly, "the farm under the rock".into(), false)); }
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
            PlaceKind::Tomb => {
                rect(&mut put, x + t * 0.1, y + t * 0.25, x + t * 0.9, y + t * 0.75, [150.0, 142.0, 130.0]);
                for k in 0..(t * 0.6) as i64 { put((x + t * 0.2) as i64 + k, (y + t * 0.5) as i64, INK, 0.7); }
            }
            PlaceKind::Lair => {
                for k in 0..7 { let (bx, by) = (x + t * (0.2 + 0.1 * k as f32), y + t * (0.3 + 0.07 * ((k * 5) % 7) as f32)); for d in 0..(t * 0.25) as i64 { put(bx as i64 + d, by as i64, [228.0, 220.0, 196.0], 0.9); } }
                put((x + t * 0.6) as i64, (y + t * 0.6) as i64, [210.0, 170.0, 60.0], 1.0);
            }
            PlaceKind::OldMine => { rect(&mut put, x + t * 0.25, y + t * 0.35, x + t * 0.75, y + t * 0.7, [110.0, 90.0, 70.0]); }
            _ => {}
        }
        if t >= 9.0 { labels.push((x + t * 0.5, y - 2.0, pl.name.clone(), false)); }
    }
    // Settlers on this level, and faintly those a level off.
    let looks = settler_looks(colony, history);
    let scale = (t / 16.0).clamp(0.55, 1.4);
    for (i, s) in colony.settlers.iter().enumerate().filter(|(_, s)| s.alive) {
        // (Those on the surface are drawn by `draw_colony` when it ran.)
        if surface_drawn && !colony.below(i) { continue; }
        let z = colony.here3(i).2;
        let off = (z - cam.z).abs();
        if off > 1 { continue; }
        let a = if off == 0 { 0.97 } else { 0.3 };
        let dp = colony.draw_pos(i);
        let (x, y) = to_screen(dp.0 + 0.5, dp.1 + 0.5);
        let (skin, hair, dress) = looks[i];
        let (sw, sh) = (7.0 * scale, 6.0 * scale);
        let base = y + 5.0 * scale;
        for dy in -(sh as i64)..=0 { for dx in -(sw as i64 + 1)..=(sw as i64 + 1) {
            let e = (dx as f32 / sw).powi(2) + (dy as f32 / sh).powi(2);
            if e <= 1.0 { put(x as i64 + dx, base as i64 + dy, if e > 0.72 { INK } else { dress }, a); }
        } }
        let hr = 4.2 * scale;
        let (hx, hy) = (x, base - sh - hr * 0.7);
        let rr = hr.ceil() as i64 + 1;
        for dy in -rr..=rr { for dx in -rr..=rr {
            let d = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt();
            if d > hr + 0.4 { continue; }
            let c = if d > hr - 1.0 { INK } else if (dy as f32) < -hr * 0.25 { hair } else { skin };
            put(hx as i64 + dx, hy as i64 + dy, c, a);
        } }
        if off == 0 && t >= 6.0 { labels.push((x, hy - hr - 2.0, s.name.clone(), false)); }
    }
    // The level's caption: how deep, and what is dug on it.
    {
        let zc = colony.map.surface_z[colony.camp.1 as usize * colony.map.width + colony.camp.0 as usize];
        let d = cam.z - zc;
        let mut kinds: Vec<String> = Vec::new();
        for (r, dug) in colony.rooms.iter().map(|r| (r, true)).chain(colony.dig_rooms.iter().map(|r| (r, false))).filter(|(r, _)| r.z == cam.z) {
            let k = match r.kind { RoomKind::Bedroom => "bedrooms", RoomKind::GreatHall => "the great hall", RoomKind::Hall => "the hall in the hill", RoomKind::Cellar => "the cellar", RoomKind::Tomb => "the tombs", RoomKind::Workshop => "the workshops", RoomKind::Farm => "the farm", RoomKind::Corridor => continue };
            let k = if dug { k.to_string() } else { format!("{} (being dug)", k) };
            if !kinds.iter().any(|x| *x == k) { kinds.push(k); }
        }
        if let Some(c) = colony.map.caverns.iter().find(|c| (0..colony.map.width).step_by(16).any(|x| colony.map.cavern_at(x, colony.camp.1 as usize, cam.z + 1).map_or(false, |l| l == c.layer as usize))) { kinds.push(c.name.clone()); }
        let depth = if d == 0 { "the camp's ground".to_string() } else if d < 0 { format!("{} below the camp's ground", -d) } else { format!("{} above the camp's ground", d) };
        let cap = format!("Level {} - {}{}", cam.z, depth, if kinds.is_empty() { String::new() } else { format!(": {}", kinds.join(", ")) });
        let tw = super::fonts::width(&cap, super::fonts::Face::Italic, 16.0, 0.0);
        super::fonts::draw(buf, w, h, w as f32 / 2.0 - tw / 2.0, 14.0, &cap, super::fonts::Face::Italic, 16.0, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
    }
    // Names step aside from each other (above, below, right, left) or are left out.
    let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new();
    for (x, y, text, faint) in labels {
        let px = 12.0;
        let tw = super::fonts::width(&text, super::fonts::Face::Italic, px, 0.0);
        for (ox, oy) in [(0.0, 0.0), (0.0, px + 16.0), (tw / 2.0 + 10.0, px * 0.6), (-tw / 2.0 - 10.0, px * 0.6)] {
            let r = (x - tw / 2.0 + ox, y - px + oy, tw, px + 2.0);
            if placed.iter().any(|q| r.0 < q.0 + q.2 && q.0 < r.0 + r.2 && r.1 < q.1 + q.3 && q.1 < r.1 + r.3) { continue; }
            placed.push(r);
            super::fonts::draw(buf, w, h, r.0, r.1, &text, super::fonts::Face::Italic, px, 0.0, if faint { 0x0080_7060 } else { 0x0030_1E14 }, Some(0x00EE_E4CC));
            break;
        }
    }
}
