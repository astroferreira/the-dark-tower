//! The camp's works in ink: each thing the settlers build has its own look, so the camp can be
//! read from the map. A real campfire (a ring of stones, crossed logs, flames that move), the
//! palisade as a row of pointed stakes standing up (stone courses where it was closed in stone),
//! the woodpile as stacked log ends that run down as the fire eats them, a windbreak of dry
//! stone, the smokehouse with its smoke, a drying rack hung with hides and fish, a still's copper
//! pot, rail fences round the store, the pen and the field, a well with its windlass and bucket,
//! a jetty out over the water, cage traps (with what they caught), and on every roofed building
//! what it is for: a chimney and its smoke, a temple's bell-cote, a lord's banners, and a
//! signboard by the door with the house's sign (a sack, a tankard, a book, a hammer, a pot...).
//!
//! Drawn with the ink kit in cell units (a `Pen` whose unit is one cell), over the map and under
//! the people, from `local_ink::draw_colony`.

use super::ink::{mix, Finish, Pen, Rgb, INK};
use super::render::LocalCamera;
use crate::colony::projects::ProjectKind;
use crate::colony::Colony;

const WOOD: Rgb = [164.0, 120.0, 78.0];
const DARK_WOOD: Rgb = [122.0, 86.0, 54.0];
const STONE: Rgb = [178.0, 172.0, 160.0];
const THATCH: Rgb = [196.0, 168.0, 108.0];
const SMOKE: Rgb = [214.0, 210.0, 204.0];
/// Trodden earth, painted over the wall blocks a work is stamped as before its sprite is drawn.
const EARTH: Rgb = [158.0, 148.0, 110.0];

/// Paint a cell rectangle as trodden earth with a ragged speckle (no rim).
fn earth(pen: &mut Pen, u0: f32, v0: f32, u1: f32, v1: f32) {
    pen.rect_f(u0, v0, u1, v1, EARTH, Finish::Paint);
    for k in 0..((u1 - u0) * (v1 - v0) * 6.0) as i32 {
        let (a, b) = ((k * 37 % 97) as f32 / 97.0, (k * 61 % 89) as f32 / 89.0);
        pen.dot(u0 + a * (u1 - u0), v0 + b * (v1 - v0), mix(EARTH, INK, 0.35));
    }
}

/// A pen whose unit is one cell, with the cell (0, 0) of the map at... the map's origin.
pub(crate) struct Cells { pub x0: f32, pub y0: f32, pub t: f32 }

impl Cells {
    pub fn new(cam: &LocalCamera, w: usize, h: usize) -> Cells {
        let t = cam.tile_px;
        Cells { x0: w as f32 / 2.0 - cam.cx * t, y0: h as f32 / 2.0 - cam.cy * t, t }
    }
    /// A pen with its origin at map cell (x, y) (the cell's top-left corner); units are cells.
    pub fn pen<'a>(&self, put: &'a mut dyn FnMut(i64, i64, Rgb, f32), x: f32, y: f32) -> Pen<'a> {
        Pen::new(put, self.x0 + x * self.t, self.y0 + y * self.t, 2.0 * self.t)
    }
    pub fn visible(&self, x: f32, y: f32, w: usize, h: usize, margin: f32) -> bool {
        let (sx, sy) = (self.x0 + x * self.t, self.y0 + y * self.t);
        sx > -margin * self.t && sy > -margin * self.t && sx < w as f32 + margin * self.t && sy < h as f32 + margin * self.t
    }
}

/// Smoke rising from (u, v) in a pen's units: puffs drifting east and fading, moving with time.
fn smoke(pen: &mut Pen, u: f32, v: f32, tick: u64, thick: f32) {
    for k in 0..4 {
        let phase = ((tick as f32 / 6.0) + k as f32 * 0.25).fract();
        let rise = phase * 1.6 + k as f32 * 0.0;
        let (pu, pv) = (u + rise * 0.45 + (phase * 9.0).sin() * 0.06, v - rise);
        let r = 0.12 + phase * 0.22;
        pen.glow(pu, pv, r * thick.max(0.6) * 1.6, SMOKE, 0.8 * (1.0 - phase) * thick.min(1.0));
        if phase < 0.6 { pen.glow(pu, pv, r * 0.7, [190.0, 186.0, 180.0], 0.35 * (1.0 - phase)); }
    }
}

/// A chimney stack standing on a roof at (u, v), smoking.
fn chimney(pen: &mut Pen, u: f32, v: f32, tick: u64, thick: f32) {
    pen.rect(u - 0.18, v - 0.3, u + 0.18, v + 0.1, STONE);
    pen.line((u - 0.18, v - 0.12), (u + 0.18, v - 0.12), mix(STONE, INK, 0.5), 1.0);
    pen.rect_f(u - 0.1, v - 0.32, u + 0.1, v - 0.26, [60.0, 52.0, 48.0], Finish::Plain);
    smoke(pen, u, v - 0.45, tick, thick);
}

/// A signboard hanging from a post by a door: the post at (u, v) on the ground, the board's
/// emblem drawn by `emblem` in the board's own units.
fn signboard(pen: &mut Pen, u: f32, v: f32, emblem: &dyn Fn(&mut Pen, f32, f32)) {
    pen.bone(&[(u, v), (u, v - 1.25)], DARK_WOOD, (pen.half * 0.08).max(1.5));
    pen.bone(&[(u, v - 1.2), (u + 0.55, v - 1.2)], DARK_WOOD, (pen.half * 0.06).max(1.0));
    pen.line((u + 0.12, v - 1.2), (u + 0.12, v - 1.1), INK, 1.0);
    pen.line((u + 0.48, v - 1.2), (u + 0.48, v - 1.1), INK, 1.0);
    pen.rect(u - 0.02, v - 1.1, u + 0.62, v - 0.62, [214.0, 196.0, 156.0]);
    emblem(pen, u + 0.3, v - 0.86);
}

/// A flag on a pole: the pole's foot at (u, v), in `field` with a `charge` stripe.
fn banner(pen: &mut Pen, u: f32, v: f32, field: Rgb, charge: Rgb, tick: u64) {
    pen.bone(&[(u, v), (u, v - 1.6)], DARK_WOOD, (pen.half * 0.06).max(1.0));
    let flap = ((tick as f32 / 3.0).sin()) * 0.05;
    let pts = [(u, v - 1.58), (u + 0.62, v - 1.5 + flap), (u + 0.5, v - 1.32 + flap), (u + 0.62, v - 1.14 + flap), (u, v - 1.1)];
    pen.poly(&pts, field);
    pen.shape(charge, Finish::Paint, [u, v - 1.6, u + 0.62, v - 1.1], &move |a, b| ((b - (v - 1.34)).abs() < 0.05) && a > u + 0.04 && a < u + 0.5);
}

/// Every emblem a signboard can carry, in a board ~0.6 x 0.45 cells round (cu, cv).
fn emblem(kind: ProjectKind) -> Option<fn(&mut Pen, f32, f32)> {
    Some(match kind {
        ProjectKind::Storehouse => |p: &mut Pen, u, v| { p.ellipse(u, v + 0.04, 0.14, 0.15, [196.0, 168.0, 116.0]); p.rect(u - 0.05, v - 0.16, u + 0.05, v - 0.08, [196.0, 168.0, 116.0]); },
        ProjectKind::Tavern => |p: &mut Pen, u, v| { p.rect(u - 0.1, v - 0.13, u + 0.08, v + 0.15, [176.0, 124.0, 70.0]); p.rect_f(u - 0.1, v - 0.16, u + 0.08, v - 0.09, [240.0, 232.0, 210.0], Finish::Plain); p.line((u + 0.08, v - 0.06), (u + 0.16, v + 0.06), INK, 1.5); },
        ProjectKind::Library => |p: &mut Pen, u, v| { p.rect(u - 0.15, v - 0.12, u + 0.15, v + 0.12, [130.0, 58.0, 46.0]); p.line((u, v - 0.12), (u, v + 0.12), INK, 1.0); },
        ProjectKind::Workshop => |p: &mut Pen, u, v| { p.bone(&[(u - 0.12, v + 0.14), (u + 0.08, v - 0.06)], DARK_WOOD, 1.5); p.rect(u + 0.0, v - 0.16, u + 0.18, v - 0.04, [150.0, 150.0, 158.0]); },
        ProjectKind::Kitchen => |p: &mut Pen, u, v| { p.ellipse(u, v + 0.04, 0.16, 0.11, [70.0, 64.0, 62.0]); p.line((u - 0.18, v - 0.04), (u + 0.18, v - 0.04), INK, 1.5); p.glow(u, v - 0.12, 0.12, [240.0, 236.0, 230.0], 0.6); },
        ProjectKind::Temple => |p: &mut Pen, u, v| { p.ellipse(u, v, 0.12, 0.12, [214.0, 176.0, 70.0]); for k in 0..8 { let a = k as f32 * 0.785; p.line((u + a.cos() * 0.14, v + a.sin() * 0.14), (u + a.cos() * 0.2, v + a.sin() * 0.2), [190.0, 150.0, 50.0], 1.0); } },
        ProjectKind::GuildHall => |p: &mut Pen, u, v| { p.bone(&[(u - 0.14, v + 0.14), (u + 0.14, v - 0.14)], [150.0, 150.0, 158.0], 1.5); p.bone(&[(u + 0.14, v + 0.14), (u - 0.14, v - 0.14)], DARK_WOOD, 1.5); },
        ProjectKind::LordsHall => |p: &mut Pen, u, v| { p.poly(&[(u - 0.17, v + 0.1), (u - 0.17, v - 0.12), (u - 0.08, v - 0.02), (u, v - 0.15), (u + 0.08, v - 0.02), (u + 0.17, v - 0.12), (u + 0.17, v + 0.1)], [214.0, 176.0, 70.0]); },
        ProjectKind::Smokehouse => |p: &mut Pen, u, v| { p.ellipse(u, v, 0.16, 0.08, [150.0, 176.0, 188.0]); p.poly(&[(u - 0.14, v), (u - 0.22, v - 0.07), (u - 0.22, v + 0.07)], [150.0, 176.0, 188.0]); },
        _ => return None,
    })
}

/// The signboard emblem of a work (for the moment cards' vignettes).
pub fn emblem_pub(kind: ProjectKind) -> Option<fn(&mut Pen, f32, f32)> { emblem(kind) }

/// A roof seen from above, as the ink renderer draws houses (`local_ink`): a ridge along x, two
/// slopes in courses, the south one hatched, ink eaves.
fn roof_top(pen: &mut Pen, u0: f32, v0: f32, u1: f32, v1: f32, c: Rgb) {
    let mid = (v0 + v1) / 2.0;
    pen.rect(u0, v0, u1, mid, c);
    pen.rect(u0, mid, u1, v1, mix(c, INK, 0.12));
    let n = ((v1 - v0) * 4.0) as i32;
    for k in 1..n { let v = v0 + (v1 - v0) * k as f32 / n as f32; if (v - mid).abs() > 0.05 { pen.line_a((u0, v), (u1, v), INK, 1.0, 0.35); } }
    pen.line((u0, mid), (u1, mid), INK, 1.5);
}

/// The campfire: a ring of stones, crossed logs and flames (embers when the woodpile is empty and
/// no windbreak keeps it).
pub fn draw_fire(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    let c = Cells::new(cam, w, h);
    let (fx, fy) = (colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 + 0.5);
    if !c.visible(fx, fy, w, h, 2.0) { return; }
    let lit = colony.fire_kept() || colony.clock.day() <= 2;
    let tick = colony.clock.tick;
    // A little larger than life, so the camp's heart reads at any zoom.
    let mut pen = Pen::new(put, c.x0 + fx * c.t, c.y0 + fy * c.t, 2.0 * c.t * 1.6);
    pen.ground_shadow(0.0, 0.08, 0.5, 0.32);
    pen.ellipse_f(0.0, 0.05, 0.36, 0.26, [64.0, 54.0, 48.0], Finish::Plain);
    for k in 0..9 {
        let a = k as f32 / 9.0 * std::f32::consts::TAU;
        let s = 0.07 + 0.02 * ((k * 5 % 3) as f32);
        pen.ellipse(a.cos() * 0.42, 0.05 + a.sin() * 0.3, s * 1.3, s, mix(STONE, INK, 0.05 * (k % 3) as f32));
    }
    let log = if lit { DARK_WOOD } else { [70.0, 58.0, 50.0] };
    pen.limb((-0.28, 0.16), 0.06, (0.24, -0.08), 0.06, log);
    pen.limb((-0.24, -0.06), 0.06, (0.28, 0.14), 0.06, log);
    if lit {
        pen.glow(0.0, -0.05, 0.75, [250.0, 170.0, 70.0], 0.45);
        for k in 0..3 {
            let flick = ((tick as f32 * 1.7 + k as f32 * 2.1).sin() * 0.5 + 0.5) * 0.18;
            let u = -0.14 + k as f32 * 0.14;
            let top = -0.42 - flick - if k == 1 { 0.14 } else { 0.0 };
            pen.poly(&[(u - 0.1, 0.04), (u - 0.06, -0.12), (u + 0.01, top), (u + 0.06, -0.1), (u + 0.1, 0.04)], [226.0, 120.0, 46.0]);
            pen.poly_f(&[(u - 0.05, 0.03), (u, top * 0.55), (u + 0.05, 0.03)], [250.0, 214.0, 120.0], Finish::Paint);
        }
        smoke(&mut pen, 0.05, -0.75, tick, 0.7);
    } else {
        pen.glow(0.0, 0.04, 0.3, [200.0, 70.0, 40.0], 0.5);
        for (u, v) in [(-0.08, 0.06), (0.06, 0.02), (0.0, 0.1)] { pen.dot(u, v, [230.0, 110.0, 50.0]); }
    }
}

fn in_footprints(colony: &Colony, x: u16, y: u16) -> bool {
    let inside = |at: (u16, u16), w: u16, h: u16| x >= at.0 && x < at.0 + w && y >= at.1 && y < at.1 + h;
    colony.projects.iter().any(|p| Colony::footprint(p.kind).map_or(false, |(w, h)| inside(p.at, w, h)))
        || colony.hut.as_ref().map_or(false, |h| inside(h.at, crate::colony::HUT_W as u16, crate::colony::HUT_H as u16))
}

/// The palisade: each wall cell of the ring as three pointed stakes standing up from it, lashed;
/// in stone, courses of blocks. Drawn row by row, north first, so a nearer stake hides a farther.
pub fn draw_palisade(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    if !colony.projects.iter().any(|p| p.kind == ProjectKind::Palisade && p.used > 0) { return; }
    let c = Cells::new(cam, w, h);
    let map = &colony.map;
    let r = colony.wall_r() + 1;
    let (cx, cy) = (colony.camp.0 as i32, colony.camp.1 as i32);
    for y in (cy - r).max(0)..=(cy + r).min(map.height as i32 - 1) {
        for x in (cx - r).max(0)..=(cx + r).min(map.width as i32 - 1) {
            let d = (((x - cx).pow(2) + (y - cy).pow(2)) as f32).sqrt();
            if d < (r - 2) as f32 || d > r as f32 + 0.6 { continue; }
            let (xu, yu) = (x as usize, y as usize);
            let k = yu * map.width + xu;
            if map.roofs[k] != 0 { continue; }
            let sz = map.surface_z[k];
            if sz + 1 >= map.depth as i32 { continue; }
            let cell = map.cell(xu, yu, (sz + 1) as usize);
            if cell.shape != crate::local::Shape::Wall || in_footprints(colony, x as u16, y as u16) { continue; }
            if !c.visible(x as f32, y as f32, w, h, 2.0) { continue; }
            let mut pen = c.pen(put, x as f32, y as f32);
            if matches!(cell.material, crate::local::Material::Wood) {
                for s in 0..3 {
                    let u = 0.17 + s as f32 * 0.33;
                    let tone = mix(WOOD, DARK_WOOD, (((x * 7 + y * 3 + s) % 4) as f32) / 5.0);
                    pen.poly(&[(u - 0.17, 1.0), (u - 0.17, -0.2), (u, -0.48), (u + 0.17, -0.2), (u + 0.17, 1.0)], tone);
                }
                pen.line_a((0.0, 0.15), (1.0, 0.15), INK, 1.0, 0.7);
                pen.line_a((0.0, 0.62), (1.0, 0.62), INK, 1.0, 0.5);
            } else {
                pen.rect(0.0, -0.3, 1.0, 1.0, STONE);
                for row in 0..4 {
                    let v = -0.3 + row as f32 * 0.325;
                    pen.line_a((0.0, v), (1.0, v), INK, 1.0, 0.6);
                    let off = if row % 2 == 0 { 0.0 } else { 0.25 };
                    for b in 0..3 { let u = off + b as f32 * 0.5; if u > 0.02 && u < 0.98 { pen.line_a((u, v), (u, v + 0.325), INK, 1.0, 0.5); } }
                }
            }
        }
    }
}

/// The works that stand (and the hut): each kind its own look.
pub fn draw_works(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    let c = Cells::new(cam, w, h);
    let tick = colony.clock.tick;
    let map = &colony.map;
    let night = colony.clock.is_night();
    // The hut: its chimney smokes.
    if let Some(hut) = colony.hut.as_ref().filter(|h| h.done) {
        let (x, y) = (hut.at.0 as f32, hut.at.1 as f32);
        if c.visible(x, y, w, h, 8.0) { let mut pen = c.pen(put, x, y); chimney(&mut pen, crate::colony::HUT_W as f32 - 1.3, 1.6, tick, 0.8); }
    }
    let mut order: Vec<&crate::colony::projects::Project> = colony.projects.iter().filter(|p| p.done || matches!(p.kind, ProjectKind::Woodpile) && p.used > 0).collect();
    order.sort_by_key(|p| p.at.1);
    for p in order {
        let Some((fw, fh)) = Colony::footprint(p.kind).or(match p.kind { ProjectKind::Lookout => Some((2, 2)), _ => None }) else {
            if p.kind == ProjectKind::Jetty { }
            continue;
        };
        let (x, y) = (p.at.0 as f32, p.at.1 as f32);
        if !c.visible(x, y, w, h, 10.0) { continue; }
        let (fw, fh) = (fw as f32, fh as f32);
        let stone = p.material == crate::colony::ItemKind::Stone;
        let mut pen = c.pen(put, x, y);
        let door = (fw / 2.0).floor() + 0.5;
        match p.kind {
            ProjectKind::Woodpile => {
                earth(&mut pen, 0.0, 0.0, fw, fh);
                // Stacked log ends between two stakes; the stack runs down as the fire eats it.
                let full = (p.used as f32 / p.needed.max(1) as f32).clamp(0.0, 1.0);
                let rows = (full * 3.0).ceil() as i32;
                pen.ellipse_f(1.5, 0.95, 1.6, 0.18, [70.0, 60.0, 52.0], Finish::Paint);
                for row in 0..rows {
                    let n = 7 - row;
                    for k in 0..n {
                        let u = 0.28 + k as f32 * 0.41 + row as f32 * 0.2;
                        let v = 0.78 - row as f32 * 0.3;
                        pen.ellipse(u, v, 0.17, 0.15, [214.0, 182.0, 128.0]);
                        pen.ellipse_f(u, v, 0.06, 0.05, [176.0, 140.0, 90.0], Finish::Paint);
                    }
                }
                pen.bone(&[(0.05, 1.0), (0.05, -0.1)], DARK_WOOD, (pen.half * 0.07).max(1.5));
                pen.bone(&[(2.95, 1.0), (2.95, -0.1)], DARK_WOOD, (pen.half * 0.07).max(1.5));
            }
            ProjectKind::Windbreak => {
                earth(&mut pen, 0.0, 0.0, fw, fh);
                // A dry-stone wall: rounded stones in two courses.
                for row in 0..2 {
                    for k in 0..(fw as i32 * 2 + 1) {
                        let u = 0.05 + k as f32 * 0.5 + if row == 1 { 0.25 } else { 0.0 };
                        if u > fw - 0.05 { continue; }
                        let v = 0.72 - row as f32 * 0.32;
                        pen.ellipse(u, v, 0.27, 0.2, mix(STONE, INK, 0.06 * ((k * 3 + row) % 4) as f32));
                    }
                }
            }
            ProjectKind::Smokehouse => {
                // A small shed, its roof vented and smoking, fish hung under the eaves.
                pen.rect(0.0, 0.0, fw, fh, DARK_WOOD);
                roof_top(&mut pen, -0.1, -0.1, fw + 0.1, fh - 0.4, if stone { STONE } else { THATCH });
                pen.rect(0.0, fh - 0.4, fw, fh, mix(DARK_WOOD, INK, 0.2));
                for k in 0..4 { let u = 0.4 + k as f32 * 0.7; pen.line((u, fh - 0.4), (u, fh - 0.2), INK, 1.0); pen.ellipse_rot(u, fh - 0.08, 0.06, 0.13, 0.0, [150.0, 176.0, 188.0]); }
                smoke(&mut pen, fw / 2.0, (fh - 0.4) / 2.0 - 0.3, tick, 1.0);
                if let Some(e) = emblem(p.kind) { signboard(&mut pen, fw + 0.3, fh, &e); }
            }
            ProjectKind::DryingRack => {
                earth(&mut pen, 0.0, 0.0, fw, fh);
                // Two A-frames and a pole across, hung with a hide and fish.
                for u in [0.2, 1.8] { pen.bone(&[(u - 0.2, 1.9), (u, 0.4), (u + 0.2, 1.9)], DARK_WOOD, (pen.half * 0.06).max(1.0)); }
                pen.bone(&[(0.1, 0.45), (1.9, 0.45)], WOOD, (pen.half * 0.06).max(1.0));
                pen.poly(&[(0.4, 0.5), (1.05, 0.5), (1.0, 1.15), (0.75, 1.3), (0.45, 1.15)], [176.0, 136.0, 96.0]);
                for k in 0..3 { let u = 1.25 + k as f32 * 0.2; pen.line((u, 0.45), (u, 0.62), INK, 1.0); pen.ellipse(u, 0.85, 0.07, 0.22, [150.0, 176.0, 188.0]); }
            }
            ProjectKind::Still => {
                earth(&mut pen, 0.0, 0.0, fw, fh);
                // A copper pot over a little fire, its coil running to a cask.
                pen.glow(0.7, 1.55, 0.5, [250.0, 160.0, 70.0], 0.35);
                pen.ellipse(0.7, 1.2, 0.48, 0.42, [184.0, 112.0, 70.0]);
                pen.ellipse(0.7, 0.75, 0.2, 0.16, [184.0, 112.0, 70.0]);
                pen.bone(&[(0.85, 0.7), (1.3, 0.6), (1.45, 0.85), (1.3, 1.0), (1.5, 1.1)], [190.0, 120.0, 76.0], (pen.half * 0.05).max(1.0));
                pen.shape([150.0, 104.0, 64.0], Finish::Inked, [1.25, 1.0, 1.85, 1.8], &|u, v| ((u - 1.55) / (0.26 + 0.05 * (1.0 - ((v - 1.4) / 0.4).powi(2)))).abs() <= 1.0 && (v - 1.4).abs() <= 0.4);
                pen.line((1.3, 1.2), (1.8, 1.2), INK, 1.0);
                pen.line((1.3, 1.6), (1.8, 1.6), INK, 1.0);
            }
            ProjectKind::Fence | ProjectKind::Pen | ProjectKind::Field => {
                // The corner posts were stamped as blocks: earth, then a stout post on each.
                for (cu, cv) in [(0.0, 0.0), (fw - 1.0, 0.0), (0.0, fh - 1.0), (fw - 1.0, fh - 1.0)] {
                    earth(&mut pen, cu, cv, cu + 1.0, cv + 1.0);
                }
                if p.kind == ProjectKind::Field { crops(&mut pen, colony, p.at); }
                fence(&mut pen, fw, fh, p.kind != ProjectKind::Field);
                for (cu, cv) in [(0.1, 0.1), (fw - 0.1, 0.1), (0.1, fh - 0.1), (fw - 0.1, fh - 0.1)] {
                    pen.ellipse(cu, cv, 0.2, 0.2, DARK_WOOD);
                    pen.ellipse_f(cu, cv, 0.09, 0.09, [176.0, 140.0, 90.0], Finish::Paint);
                }
                drop(pen);
                if p.kind == ProjectKind::Pen {
                    // The beasts kept in it, ambling about.
                    if let Some((kind, n)) = colony.pen.as_ref() {
                        let look = super::beasts::of_name(kind);
                        let scale = (c.t / 16.0).clamp(0.55, 1.4);
                        let px = super::beasts::px_for(&look, scale) * 0.85;
                        for k in 0..(*n).min(6) {
                            let ph = tick as f32 / 90.0 + k as f32 * 1.9;
                            let u = 1.2 + (k % 3) as f32 * 1.4 + ph.sin() * 0.35;
                            let v = 1.6 + (k / 3) as f32 * 1.6 + (ph * 0.7).cos() * 0.25;
                            let (sx, sy) = (c.x0 + (x + u) * c.t, c.y0 + (y + v) * c.t);
                            let walking = (tick / 30 + k as u64) % 4 == 0;
                            let pose = if walking { super::beasts::Pose::Walk((tick / 4) % 2 == 0) } else if k % 2 == 0 { super::beasts::Pose::Graze } else { super::beasts::Pose::Stand };
                            super::beasts::draw(put, &look, sx, sy, px, ph.cos() < 0.0, pose, 1.0);
                        }
                    }
                }
            }
            ProjectKind::Well => {
                // A ring of stones round dark water, a little roof on two posts, a windlass, a bucket.
                pen.ellipse(0.5, 0.55, 0.48, 0.4, STONE);
                pen.ellipse_f(0.5, 0.55, 0.3, 0.24, [52.0, 62.0, 74.0], Finish::Plain);
                pen.ellipse_f(0.42, 0.5, 0.08, 0.04, [150.0, 170.0, 190.0], Finish::Paint);
                for k in 0..8 { let a = k as f32 * 0.785; pen.line((0.5 + a.cos() * 0.31, 0.55 + a.sin() * 0.25), (0.5 + a.cos() * 0.47, 0.55 + a.sin() * 0.39), INK, 1.0); }
                for u in [0.05, 0.95] { pen.bone(&[(u, 0.6), (u, -0.35)], DARK_WOOD, (pen.half * 0.06).max(1.0)); }
                pen.bone(&[(0.05, 0.05), (0.95, 0.05)], WOOD, (pen.half * 0.09).max(1.5));
                pen.poly(&[(-0.1, -0.3), (0.5, -0.62), (1.1, -0.3), (1.1, -0.2), (-0.1, -0.2)], THATCH);
                pen.line((0.5, 0.05), (0.5, 0.35), INK, 1.0);
                pen.rect(0.4, 0.35, 0.6, 0.52, [150.0, 104.0, 64.0]);
            }
            ProjectKind::Lookout => {
                // A pennant over the tower's top.
                banner(&mut pen, 1.8, 0.2, [150.0, 52.0, 44.0], [220.0, 186.0, 90.0], tick);
            }
            _ => {
                // Roofed buildings: chimneys, a bell-cote, banners, a signboard by the door.
                let ridge = fh / 2.0;
                match p.kind {
                    ProjectKind::SecondHut => chimney(&mut pen, fw - 1.3, ridge - 0.9, tick, 0.8),
                    ProjectKind::Kitchen => { chimney(&mut pen, fw - 0.8, ridge - 0.9, tick, 1.3); chimney(&mut pen, 0.8, ridge - 0.9, tick, 1.0); }
                    ProjectKind::Tavern => { chimney(&mut pen, fw - 1.0, ridge - 0.9, tick, if night { 1.2 } else { 0.7 }); }
                    ProjectKind::Temple => {
                        // A bell-cote astride the ridge, a gold disc over it.
                        let u = fw / 2.0;
                        pen.rect(u - 0.35, ridge - 1.3, u + 0.35, ridge, if stone { STONE } else { WOOD });
                        pen.rect_f(u - 0.18, ridge - 1.1, u + 0.18, ridge - 0.55, [60.0, 52.0, 48.0], Finish::Plain);
                        pen.ellipse(u, ridge - 0.72, 0.12, 0.14, [200.0, 160.0, 70.0]);
                        pen.poly(&[(u - 0.48, ridge - 1.25), (u, ridge - 1.95), (u + 0.48, ridge - 1.25)], if stone { mix(STONE, INK, 0.15) } else { THATCH });
                        pen.ellipse(u, ridge - 2.1, 0.12, 0.12, [214.0, 176.0, 70.0]);
                    }
                    ProjectKind::LordsHall => {
                        let (f, ch) = colony.lord.as_ref().map(|l| super::folk::band_colours(&l.people)).unwrap_or(([96.0, 64.0, 110.0], [220.0, 186.0, 90.0]));
                        banner(&mut pen, 0.15, ridge, f, ch, tick);
                        banner(&mut pen, fw - 0.15, ridge, f, ch, tick + 7);
                        chimney(&mut pen, fw / 2.0, ridge - 0.9, tick, 0.7);
                    }
                    ProjectKind::GuildHall => {
                        let (f, ch) = super::folk::band_colours(colony.guilds.first().map(|g| g.name.as_str()).unwrap_or("guild"));
                        banner(&mut pen, fw - 0.2, ridge, f, ch, tick);
                    }
                    ProjectKind::Storehouse => {
                        // Casks and sacks by the door.
                        super::glyphs::draw(&mut |px, py, col, a| pen.pixel(px, py, col, a), super::glyphs::Glyph::Barrel, c.x0 + (x + door - 1.1) * c.t, c.y0 + (y + fh + 0.35) * c.t, (c.t * 0.75).max(6.0), None);
                        super::glyphs::draw(&mut |px, py, col, a| pen.pixel(px, py, col, a), super::glyphs::Glyph::Provisions, c.x0 + (x + door - 1.8) * c.t, c.y0 + (y + fh + 0.4) * c.t, (c.t * 0.7).max(6.0), None);
                    }
                    ProjectKind::Workshop => {
                        // An anvil on its block, a bench, in the open side.
                        // In the yard before its open side.
                        let v = fh + 0.55;
                        pen.rect(0.8, v - 0.1, 1.2, v + 0.3, DARK_WOOD);
                        pen.poly(&[(0.55, v - 0.28), (1.5, v - 0.28), (1.28, v - 0.1), (0.82, v - 0.1)], [120.0, 122.0, 130.0]);
                        pen.rect(fw - 1.6, v - 0.25, fw - 0.2, v + 0.05, WOOD);
                        pen.line((fw - 1.5, v + 0.05), (fw - 1.5, v + 0.35), INK, 1.0);
                        pen.line((fw - 0.3, v + 0.05), (fw - 0.3, v + 0.35), INK, 1.0);
                    }
                    _ => {}
                }
                if let Some(e) = emblem(p.kind) { if p.kind != ProjectKind::Smokehouse { signboard(&mut pen, door + 0.65, fh, &e); } }
            }
        }
    }
    // The jetty: planks out from the bank over the water, on posts.
    if let Some(j) = colony.jetty {
        let (jx, jy) = (j.0 as i32, j.1 as i32);
        let wet = |x: i32, y: i32| x >= 0 && y >= 0 && (x as usize) < map.width && (y as usize) < map.height && {
            let k = y as usize * map.width + x as usize;
            let sz = map.surface_z[k].max(0) as usize;
            map.cell(x as usize, y as usize, (sz + 1).min(map.depth - 1)).water > 0 || map.cell(x as usize, y as usize, sz).water > 0
        };
        let dir = [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().max_by_key(|&(dx, dy)| (1..=3).filter(|&k| wet(jx + dx * k, jy + dy * k)).count()).unwrap_or((0, 1));
        if c.visible(jx as f32, jy as f32, w, h, 5.0) {
            let mut pen = c.pen(put, jx as f32, jy as f32);
            let (dx, dy) = (dir.0 as f32, dir.1 as f32);
            let len = 3.0;
            let (u0, v0, u1, v1) = if dx != 0.0 { (0.5 + dx.min(0.0) * len, 0.25, 0.5 + dx.max(0.0) * len, 0.75) } else { (0.25, 0.5 + dy.min(0.0) * len, 0.75, 0.5 + dy.max(0.0) * len) };
            for (pu, pv) in [(u0, v0), (u1, v0), (u0, v1), (u1, v1)] { pen.ellipse(pu, pv, 0.08, 0.08, DARK_WOOD); }
            pen.rect(u0, v0, u1, v1, WOOD);
            let n = (len * 4.0) as i32;
            for k in 1..n {
                let f = k as f32 / n as f32;
                if dx != 0.0 { let u = u0 + (u1 - u0) * f; pen.line_a((u, v0), (u, v1), INK, 1.0, 0.45); } else { let v = v0 + (v1 - v0) * f; pen.line_a((u0, v), (u1, v), INK, 1.0, 0.45); }
            }
        }
    }
}

/// A rail fence round a w x h lot: a post at every cell corner on its edge, two rails between;
/// a gate gap in the south side's middle.
fn fence(pen: &mut Pen, fw: f32, fh: f32, gate: bool) {
    let gap = (fw / 2.0).floor();
    let rail = |pen: &mut Pen, a: (f32, f32), b: (f32, f32)| { pen.bone(&[a, b], WOOD, (pen.half * 0.04).max(1.0)); };
    for k in 0..fw as i32 {
        let u = k as f32;
        rail(pen, (u + 0.1, 0.1), (u + 1.0 - 0.1, 0.1));
        if !(gate && (k as f32 == gap)) { rail(pen, (u + 0.1, fh - 0.1), (u + 1.0 - 0.1, fh - 0.1)); }
    }
    for k in 0..fh as i32 {
        let v = k as f32;
        rail(pen, (0.1, v + 0.1), (0.1, v + 1.0 - 0.1));
        rail(pen, (fw - 0.1, v + 0.1), (fw - 0.1, v + 1.0 - 0.1));
    }
    for k in 0..=fw as i32 { for v in [0.1, fh - 0.1] { pen.rect(k as f32 - 0.08 + if k == 0 { 0.1 } else if k as f32 == fw { -0.1 } else { 0.0 }, v - 0.08, k as f32 + 0.08 + if k == 0 { 0.1 } else if k as f32 == fw { -0.1 } else { 0.0 }, v + 0.08, DARK_WOOD); } }
    for k in 1..fh as i32 { for u in [0.1, fw - 0.1] { pen.rect(u - 0.08, k as f32 - 0.08, u + 0.08, k as f32 + 0.08, DARK_WOOD); } }
}

/// A cage trap at a gate (a mark), standing up: a timber frame with bars, the trip-stone; what it
/// holds drawn inside it.
pub fn draw_cage(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16), held: Option<&str>, scale: f32, colony: &Colony) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, 3.0) { return; }
    if let Some(name) = held {
        // The beast as it was (if it came against the camp), else by its name.
        let look = colony.foes_seen.iter().find(|(n, _)| name.starts_with(n.as_str()) || n.starts_with(name)).map(|(_, m)| super::beasts::of_monster(m)).unwrap_or_else(|| super::beasts::of_name(name));
        let px = (c.t * 1.5).max(14.0);
        super::beasts::draw(put, &look, c.x0 + (at.0 as f32 + 0.5) * c.t, c.y0 + (at.1 as f32 + 0.95) * c.t, px, false, super::beasts::Pose::Stand, 1.0);
    }
    let _ = scale;
    let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
    pen.rect_f(0.0, 0.85, 1.0, 1.0, DARK_WOOD, Finish::Plain);
    pen.rect_f(0.0, -0.25, 1.0, -0.1, DARK_WOOD, Finish::Plain);
    for k in 0..6 { let u = k as f32 * 0.2; pen.line((u, -0.15), (u, 0.9), INK, (pen.half * 0.04).max(1.0)); }
    if held.is_none() { pen.ellipse(0.5, 0.78, 0.14, 0.08, STONE); }
}

/// A grave: an earthen mound with a wooden cross at its head (an old one: a leaning headstone);
/// charred where the restless dead were burned.
pub fn draw_grave(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16), old: bool, burned: bool) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, 2.0) { return; }
    let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
    let soil = if burned { [74.0, 62.0, 54.0] } else { [150.0, 124.0, 90.0] };
    pen.ellipse(0.5, 0.62, 0.42, 0.24, soil);
    pen.line_a((0.2, 0.62), (0.8, 0.62), INK, 1.0, 0.25);
    if old {
        pen.shape(mix(STONE, INK, 0.1), Finish::Inked, [0.28, -0.25, 0.72, 0.5], &|u, v| { let (a, b) = (u - 0.5 - (0.5 - v) * 0.12, v); a.abs() <= 0.18 && b <= 0.5 && (b >= -0.05 || (a / 0.18).powi(2) + ((b + 0.05) / 0.18).powi(2) <= 1.0) });
        pen.line_a((0.42, 0.1), (0.56, 0.1), INK, 1.0, 0.5);
    } else {
        let wood = if burned { [60.0, 50.0, 44.0] } else { DARK_WOOD };
        pen.bone(&[(0.5, 0.55), (0.5, -0.3)], wood, (pen.half * 0.07).max(1.5));
        pen.bone(&[(0.3, -0.05), (0.7, -0.05)], wood, (pen.half * 0.07).max(1.5));
    }
}

/// A standing stone raised for someone or something: a tall stone, carved with lines.
pub fn draw_standing_stone(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16), tall: f32) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, 3.0) { return; }
    let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
    pen.ground_shadow(0.5, 0.9, 0.4, 0.12);
    let top = 0.85 - tall;
    pen.poly(&[(0.28, 0.9), (0.24, top + 0.3), (0.36, top + 0.05), (0.56, top), (0.7, top + 0.2), (0.74, 0.9)], STONE);
    for k in 0..3 { let v = top + 0.3 + k as f32 * (tall - 0.4) / 3.0; pen.line_a((0.36, v), (0.62, v + 0.04), INK, 1.0, 0.55); }
    pen.line_a((0.49, top + 0.25), (0.49, 0.8), INK, 1.0, 0.35);
}

/// The hall stone: a squared block, its face carved with the camp's mark.
pub fn draw_hall_stone(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16)) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, 3.0) { return; }
    let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
    pen.ground_shadow(0.5, 0.92, 0.5, 0.12);
    pen.rect(0.12, 0.25, 0.88, 0.92, STONE);
    pen.poly(&[(0.12, 0.25), (0.3, 0.05), (1.04, 0.05), (0.88, 0.25)], mix(STONE, [250.0, 244.0, 230.0], 0.25));
    pen.poly(&[(0.88, 0.25), (1.04, 0.05), (1.04, 0.72), (0.88, 0.92)], mix(STONE, INK, 0.25));
    pen.ellipse_f(0.5, 0.58, 0.16, 0.16, mix(STONE, INK, 0.45), Finish::Plain);
    pen.line((0.5, 0.42), (0.5, 0.74), INK, 1.0);
}

/// A mark of the "stone" kind, drawn as what its title says it is: the refugees' cold fire and the
/// old camp (a ring of stones round ash), a slain beast's bones (a skull and ribs), a memorial
/// slab or post, the builders' and the tribute stone (a squared block), else a standing stone.
pub fn draw_stone_mark(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16), title: &str) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, 3.0) { return; }
    if title.contains("fire") || title.contains("old camp") {
        let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
        pen.ellipse_f(0.5, 0.55, 0.36, 0.26, [120.0, 112.0, 104.0], Finish::Plain);
        for k in 0..8 { let a = k as f32 / 8.0 * std::f32::consts::TAU; pen.ellipse(0.5 + a.cos() * 0.42, 0.55 + a.sin() * 0.3, 0.09, 0.07, STONE); }
        pen.limb((0.32, 0.6), 0.04, (0.62, 0.5), 0.04, [70.0, 60.0, 54.0]);
        return;
    }
    if title.starts_with("The bones of") {
        let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
        let bone = [232.0, 224.0, 204.0];
        for k in 0..4 { let u = 0.35 + k as f32 * 0.16; pen.bone(&[(u, 0.4), (u + 0.05, 0.62), (u - 0.02, 0.85)], bone, (pen.half * 0.05).max(1.0)); }
        pen.bone(&[(0.3, 0.42), (0.9, 0.42)], bone, (pen.half * 0.06).max(1.0));
        pen.ellipse(0.12, 0.5, 0.26, 0.2, bone);
        pen.ellipse_f(0.06, 0.46, 0.06, 0.05, INK, Finish::Paint);
        pen.poly(&[(0.0, 0.62), (-0.18, 0.72), (0.04, 0.7)], bone);
        pen.bone(&[(0.18, 0.34), (0.3, 0.05), (0.18, -0.1)], bone, (pen.half * 0.06).max(1.0));
        return;
    }
    if title.starts_with("The post of") {
        let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
        pen.ground_shadow(0.5, 0.9, 0.3, 0.1);
        pen.rect(0.36, -0.4, 0.64, 0.9, DARK_WOOD);
        for k in 0..3 { let v = -0.2 + k as f32 * 0.3; pen.line_a((0.4, v), (0.6, v + 0.06), INK, 1.0, 0.6); }
        pen.poly(&[(0.36, -0.4), (0.5, -0.55), (0.64, -0.4)], DARK_WOOD);
        return;
    }
    if title.starts_with("The slab of") {
        let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
        pen.rect(0.1, 0.2, 0.9, 0.85, STONE);
        pen.rect_f(0.1, 0.2, 0.9, 0.3, mix(STONE, [250.0, 244.0, 230.0], 0.3), Finish::Plain);
        for k in 0..3 { let v = 0.42 + k as f32 * 0.13; pen.line_a((0.25, v), (0.75, v), INK, 1.0, 0.5); }
        return;
    }
    if title.contains("builders") || title.contains("tribute") { draw_hall_stone(put, cam, w, h, at); return; }
    draw_standing_stone(put, cam, w, h, at, 1.3);
}

/// The field's crop on its cells (the inner 6 x 4): sprouts in spring, green stalks in summer,
/// gold heads in autumn until it is reaped; nothing where the field lies bare.
fn crops(pen: &mut Pen, colony: &Colony, at: (u16, u16)) {
    use crate::seasons::Season;
    let map = &colony.map;
    let season = colony.season();
    for dy in 1..5u16 { for dx in 1..7u16 {
        let (x, y) = ((at.0 + dx) as usize, (at.1 + dy) as usize);
        if x >= map.width || y >= map.height { continue; }
        let sz = map.surface_z[y * map.width + x].max(0) as usize;
        if !matches!(map.cell(x, y, sz).plant, crate::local::Plant::Crop(_)) { continue; }
        for row in 0..2 {
            for k in 0..3 {
                let (u, v) = (dx as f32 + 0.2 + k as f32 * 0.3, dy as f32 + 0.45 + row as f32 * 0.45);
                match season {
                    Season::Spring => { pen.line((u, v), (u - 0.04, v - 0.12), [110.0, 150.0, 70.0], 1.0); pen.line((u, v), (u + 0.05, v - 0.1), [110.0, 150.0, 70.0], 1.0); }
                    Season::Summer => { pen.line((u, v), (u, v - 0.3), [96.0, 136.0, 62.0], 1.0); pen.ellipse_f(u, v - 0.3, 0.03, 0.06, [120.0, 160.0, 76.0], Finish::Paint); }
                    Season::Autumn => { pen.line((u, v), (u + 0.02, v - 0.32), [170.0, 140.0, 70.0], 1.0); pen.ellipse_f(u + 0.03, v - 0.34, 0.035, 0.08, [214.0, 176.0, 80.0], Finish::Plain); }
                    Season::Winter => { pen.line((u, v), (u, v - 0.06), [150.0, 130.0, 96.0], 1.0); }
                }
            }
        }
    } }
}

/// A building going up, by the share of loads laid: pegs and a line, then a timber frame (posts
/// every other cell of its ring with a sill between them), then walls of log courses (or stone)
/// rising round the ring as far as the loads go.
pub fn draw_rising(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16), bw: u16, bh: u16, share: f32, stone: bool) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, (bw.max(bh) + 2) as f32) { return; }
    let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
    let (fw, fh) = (bw as f32, bh as f32);
    // The pegged line.
    for k in 0..(2.0 * (fw + fh)) as i32 * 2 {
        if k % 2 == 1 { continue; }
        let d = k as f32 * 0.25;
        let (a, b) = if d < fw { ((d, 0.05), (d + 0.25, 0.05)) } else if d < fw + fh { ((fw - 0.05, d - fw), (fw - 0.05, d - fw + 0.25)) }
            else if d < 2.0 * fw + fh { ((fw - (d - fw - fh), fh - 0.05), (fw - (d - fw - fh) - 0.25, fh - 0.05)) } else { ((0.05, fh - (d - 2.0 * fw - fh)), (0.05, fh - (d - 2.0 * fw - fh) - 0.25)) };
        pen.line_a(a, b, INK, 1.0, 0.6);
    }
    for (u, v) in [(0.05, 0.05), (fw - 0.05, 0.05), (0.05, fh - 0.05), (fw - 0.05, fh - 0.05)] { pen.ellipse(u, v, 0.1, 0.1, DARK_WOOD); }
    let ring: Vec<(f32, f32)> = (0..bw).map(|dx| (dx as f32, 0.0)).chain((1..bh).map(|dy| (fw - 1.0, dy as f32))).chain((0..bw.saturating_sub(1)).rev().map(|dx| (dx as f32, fh - 1.0))).chain((1..bh.saturating_sub(1)).rev().map(|dy| (0.0, dy as f32))).collect();
    if share >= 0.25 {
        // The frame: a sill all round and posts.
        pen.rect_f(0.35, 0.35, fw - 0.35, 0.65, WOOD, Finish::Plain);
        pen.rect_f(0.35, fh - 0.65, fw - 0.35, fh - 0.35, WOOD, Finish::Plain);
        pen.rect_f(0.35, 0.35, 0.65, fh - 0.35, WOOD, Finish::Plain);
        pen.rect_f(fw - 0.65, 0.35, fw - 0.35, fh - 0.35, WOOD, Finish::Plain);
        for (k, &(u, v)) in ring.iter().enumerate() { if k % 2 == 0 { pen.ellipse(u + 0.5, v + 0.5, 0.17, 0.17, DARK_WOOD); } }
    }
    if share >= 0.6 {
        let n = ((share - 0.6) / 0.4 * ring.len() as f32).ceil() as usize;
        for &(u, v) in ring.iter().take(n) {
            if stone {
                pen.rect(u, v, u + 1.0, v + 1.0, STONE);
                pen.line_a((u, v + 0.5), (u + 1.0, v + 0.5), INK, 1.0, 0.5);
                pen.line_a((u + 0.5, v), (u + 0.5, v + 0.5), INK, 1.0, 0.5);
            } else {
                pen.rect(u, v, u + 1.0, v + 1.0, WOOD);
                for k in 1..4 { let y = v + k as f32 * 0.25; pen.line_a((u, y), (u + 1.0, y), INK, 1.0, 0.45); }
                pen.ellipse_f(u + 0.15, v + 0.37, 0.08, 0.08, [214.0, 182.0, 128.0], Finish::Plain);
                pen.ellipse_f(u + 0.85, v + 0.62, 0.08, 0.08, [214.0, 182.0, 128.0], Finish::Plain);
            }
        }
    }
}

/// An engraving: a dressed panel on the wall's face toward the floor it was carved from, with a
/// small scene cut in it from what it shows (a siege's tower, a slain beast's skull, crossed
/// blades for a battle or raid, a house for a work, a cradle for a birth, a wisp for the dead,
/// else a figure); finer hands' panels are gilded.
pub fn draw_engraving(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, e: &crate::colony::engrave::Engraving) {
    let c = Cells::new(cam, w, h);
    if !c.visible(e.wall.0 as f32, e.wall.1 as f32, w, h, 2.0) { return; }
    let (dx, dy) = (e.from.0 as f32 - e.wall.0 as f32, e.from.1 as f32 - e.wall.1 as f32);
    // The panel's centre: on the wall's edge toward the floor, standing a little into it.
    let (px, py) = (e.wall.0 as f32 + 0.5 + dx * 0.55, e.wall.1 as f32 + 0.5 + dy * 0.55);
    let mut pen = c.pen(put, px, py);
    let (hw, hh) = if dx != 0.0 { (0.2, 0.42) } else { (0.42, 0.24) };
    let stone = if e.quality >= 3 { [214.0, 190.0, 130.0] } else { [196.0, 186.0, 166.0] };
    pen.rect(-hw, -hh, hw, hh, stone);
    if e.quality >= 3 { pen.rect_f(-hw + 0.04, -hh + 0.04, hw - 0.04, hh - 0.04, mix(stone, [214.0, 170.0, 60.0], 0.35), Finish::Paint); }
    let t = e.image.to_lowercase();
    let k = hw.min(hh) * 0.8;
    let cut = mix(INK, stone, 0.25);
    let lw = (pen.half * 0.04).max(1.0);
    if t.contains("siege") || t.contains("fall") || t.contains("razing") {
        pen.rect_f(-k * 0.4, -k * 0.6, k * 0.4, k * 0.8, cut, Finish::Paint);
        for j in 0..3 { let u = -k * 0.4 + j as f32 * k * 0.4; pen.rect_f(u - k * 0.1, -k * 0.85, u + k * 0.1, -k * 0.6, cut, Finish::Paint); }
    } else if t.contains("death of") || t.contains("slain") || t.contains("beast") || t.contains("bones") {
        pen.ellipse_f(0.0, -k * 0.15, k * 0.6, k * 0.5, cut, Finish::Paint);
        pen.ellipse_f(-k * 0.22, -k * 0.2, k * 0.14, k * 0.14, stone, Finish::Paint);
        pen.ellipse_f(k * 0.22, -k * 0.2, k * 0.14, k * 0.14, stone, Finish::Paint);
        pen.rect_f(-k * 0.3, k * 0.3, k * 0.3, k * 0.6, cut, Finish::Paint);
    } else if t.contains("battle") || t.contains("raid") || t.contains("war") || t.contains("fought") {
        pen.line((-k * 0.7, k * 0.7), (k * 0.7, -k * 0.7), cut, lw);
        pen.line((k * 0.7, k * 0.7), (-k * 0.7, -k * 0.7), cut, lw);
    } else if t.contains("born") || t.contains("birth") {
        pen.ellipse_f(0.0, k * 0.2, k * 0.6, k * 0.35, cut, Finish::Paint);
        pen.ellipse_f(-k * 0.35, -k * 0.05, k * 0.2, k * 0.2, cut, Finish::Paint);
    } else if t.contains("ghost") || t.contains("dead") || t.contains("grave") {
        pen.path(&[(-k * 0.3, k * 0.7), (-k * 0.2, -k * 0.2), (0.0, -k * 0.6), (k * 0.2, -k * 0.2), (k * 0.35, k * 0.7)], cut, lw);
    } else if t.contains("finish") || t.contains("hut") || t.contains("hall") || t.contains("built") || t.contains("found") {
        pen.poly_f(&[(-k * 0.7, -k * 0.05), (0.0, -k * 0.75), (k * 0.7, -k * 0.05)], cut, Finish::Paint);
        pen.rect_f(-k * 0.5, -k * 0.05, k * 0.5, k * 0.7, cut, Finish::Paint);
    } else {
        pen.ellipse_f(0.0, -k * 0.5, k * 0.22, k * 0.22, cut, Finish::Paint);
        pen.line((0.0, -k * 0.25), (0.0, k * 0.35), cut, lw);
        pen.line((-k * 0.4, -k * 0.05), (k * 0.4, -k * 0.05), cut, lw);
        pen.line((0.0, k * 0.35), (-k * 0.3, k * 0.8), cut, lw);
        pen.line((0.0, k * 0.35), (k * 0.3, k * 0.8), cut, lw);
    }
}

/// A settler's own place (`colony/haunts.rs`): a cairn of stones heaped smaller to the top, a
/// bench (a plank on two legs, seen a little from the front), a post carved with little figures.
pub fn draw_haunt(put: &mut dyn FnMut(i64, i64, Rgb, f32), cam: &LocalCamera, w: usize, h: usize, at: (u16, u16), kind: crate::colony::MarkKind) {
    let c = Cells::new(cam, w, h);
    if !c.visible(at.0 as f32, at.1 as f32, w, h, 2.0) { return; }
    let mut pen = c.pen(put, at.0 as f32, at.1 as f32);
    match kind {
        crate::colony::MarkKind::Cairn => {
            pen.ground_shadow(0.5, 0.88, 0.42, 0.1);
            for (k, (r, v)) in [(0.32f32, 0.72f32), (0.25, 0.46), (0.18, 0.26)].iter().enumerate() {
                pen.ellipse(0.5 + if k == 1 { 0.03 } else { 0.0 }, *v, *r, r * 0.8, mix(STONE, INK, 0.05 * k as f32));
            }
        }
        crate::colony::MarkKind::Bench => {
            pen.ground_shadow(0.5, 0.82, 0.48, 0.08);
            for u in [0.18, 0.82] { pen.rect(u - 0.05, 0.45, u + 0.05, 0.8, DARK_WOOD); }
            pen.rect(0.05, 0.36, 0.95, 0.5, WOOD);
            pen.line_a((0.05, 0.43), (0.95, 0.43), INK, 1.0, 0.35);
        }
        _ => {
            pen.ground_shadow(0.5, 0.9, 0.25, 0.08);
            pen.rect(0.38, -0.3, 0.62, 0.9, [176.0, 136.0, 90.0]);
            for k in 0..3 {
                let v = -0.12 + k as f32 * 0.3;
                pen.ellipse_f(0.5, v, 0.05, 0.05, INK, Finish::Paint);
                pen.line((0.5, v + 0.05), (0.5, v + 0.16), INK, 1.0);
                pen.line((0.43, v + 0.09), (0.57, v + 0.09), INK, 1.0);
            }
            pen.poly(&[(0.38, -0.3), (0.5, -0.45), (0.62, -0.3)], [176.0, 136.0, 90.0]);
        }
    }
}
