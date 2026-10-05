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

type Rgb = [f32; 3];

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
        Material::Rock(_) | Material::Ore(_) => [178.0, 168.0, 148.0],
        Material::Block(_) => [196.0, 188.0, 170.0],
        Material::Wood => [176.0, 140.0, 100.0],
        Material::Air => [182.0, 160.0, 120.0],
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
        let wash = (0..w * h).into_par_iter().map(|k| match top[k] {
            Top::Water(d) => water_wash(d),
            _ => wash(floor_of(k)),
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
pub fn draw_colony(colony: &crate::colony::Colony, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    use crate::colony::{ItemKind, Job};
    let t = cam.tile_px;
    let to_screen = |x: f32, y: f32| ((x - cam.cx) * t + w as f32 / 2.0, (y - cam.cy) * t + h as f32 / 2.0);
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
        if it.stored { if it.kind == ItemKind::Log { logs += 1 } else { food += 1 }; continue; }
        let (x, y) = to_screen(it.at.0 as f32 + 0.5, it.at.1 as f32 + 0.5);
        let col = if it.kind == ItemKind::Log { [150.0, 104.0, 64.0] } else { [196.0, 64.0, 70.0] };
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
    // Settlers.
    const COATS: [Rgb; 7] = [[52.0, 86.0, 120.0], [150.0, 60.0, 48.0], [70.0, 110.0, 70.0], [170.0, 130.0, 50.0], [110.0, 70.0, 120.0], [60.0, 120.0, 120.0], [130.0, 90.0, 60.0]];
    let mut labels = Vec::new();
    for (i, s) in colony.settlers.iter().enumerate() {
        let (x, y) = to_screen(s.pos.0 as f32 + 0.5, s.pos.1 as f32 + 0.5);
        let coat = if s.alive { COATS[i % COATS.len()] } else { [120.0, 112.0, 100.0] };
        disc(&mut put, x, y, (t * 0.32).max(2.5), coat, INK);
        if let Some(kind) = s.carrying {
            let col = if kind == ItemKind::Log { [150.0, 104.0, 64.0] } else { [196.0, 64.0, 70.0] };
            disc(&mut put, x + t * 0.35, y - t * 0.2, (t * 0.12).max(1.2), col, INK);
        }
        if t >= 10.0 {
            let label = if s.job == Job::Sleep && s.path.is_empty() { format!("{} z", s.name) } else { s.name.clone() };
            labels.push((x, y, label));
        }
    }
    // Names that would overlap one already drawn are left out (hover shows them).
    let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new();
    for (x, y, label) in labels {
        let lw = super::text::text_width(&label, 1) as f32;
        let (lx, ly) = (x - lw / 2.0, y - t * 0.4 - 11.0);
        if placed.iter().any(|&(px, py, pw, ph)| lx < px + pw + 4.0 && px < lx + lw + 4.0 && ly < py + ph && py < ly + 10.0) { continue; }
        placed.push((lx, ly, lw, 10.0));
        super::text::draw_text(buf, w, h, lx as i64, ly as i64, &label, 0x0038_2A20, 0x00EE_E4CC, 1);
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
