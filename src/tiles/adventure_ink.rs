//! Adventure places drawn in the map's ink: each floor is an old plan of the place (washes on
//! parchment, rock and masonry hatched, ink rims where walls meet floor, a soft shadow down-light
//! of every wall), its furniture and fittings drawn with the sprite kit (`ink.rs`), creatures by
//! their bodies (`beasts.rs`), people by `folk.rs` (the adventurer in what they wear), things by
//! their glyphs (`glyphs.rs`).
//!
//! The floor's plan is kept between frames, a cell redrawn only when what is on it changed
//! (a door opened, a chest emptied): a frame is a copy of the kept plan, the light and the fog,
//! then the living things.

use super::glyphs::{self, Glyph};
use super::ink::{mix, pack, Finish, Pen, Rgb, INK};
use crate::adventure::map::{Feature, Floor, Ground, Tile, Wall};
use crate::adventure::site::SiteKind;

pub const PARCH: Rgb = [234.0, 222.0, 196.0];
const DARK: Rgb = [38.0, 32.0, 27.0];

fn h(a: i64, b: i64, s: u64) -> u64 { super::ink::hash(a, b, s) }
fn u(a: i64, b: i64, s: u64) -> f32 { (h(a, b, s) % 10_000) as f32 / 10_000.0 }
fn unpack(p: u32) -> Rgb { [((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32] }

/// A glyph by its name in the data ("Sword", "Coin").
pub fn glyph(name: &str) -> Glyph { Glyph::ALL.iter().copied().find(|g| format!("{:?}", g) == name).unwrap_or(Glyph::Work) }

pub fn ground_wash(g: Ground) -> Rgb {
    match g {
        Ground::Rock => [176.0, 166.0, 150.0], Ground::Flags => [200.0, 190.0, 170.0], Ground::Earth => [184.0, 160.0, 124.0],
        Ground::Grass => [172.0, 182.0, 128.0], Ground::Sand => [222.0, 204.0, 158.0], Ground::Snow => [236.0, 238.0, 236.0],
        Ground::Wood => [186.0, 146.0, 100.0], Ground::Shallows => [150.0, 180.0, 176.0], Ground::Water => [112.0, 146.0, 156.0],
        Ground::Lava => [230.0, 110.0, 40.0], Ground::Rubble => [168.0, 156.0, 140.0], Ground::Mud => [140.0, 120.0, 90.0],
        Ground::Marble => [222.0, 218.0, 210.0], Ground::Cobbles => [188.0, 178.0, 160.0], Ground::Moss => [140.0, 160.0, 110.0],
        Ground::Ash => [120.0, 112.0, 108.0], Ground::Carpet => [150.0, 60.0, 50.0], Ground::Void => DARK,
        Ground::Field => [190.0, 170.0, 104.0], Ground::Ice => [214.0, 226.0, 230.0],
    }
}

fn wall_wash(w: Wall, kind: SiteKind) -> Rgb {
    match w {
        Wall::Rock => if kind == SiteKind::Mine { [128.0, 112.0, 96.0] } else { [118.0, 110.0, 100.0] },
        Wall::Brick => [150.0, 132.0, 112.0], Wall::Timber => [132.0, 96.0, 62.0], Wall::Tree => [96.0, 122.0, 76.0],
        Wall::Bars => [90.0, 90.0, 96.0], Wall::Palisade => [128.0, 92.0, 58.0], Wall::Hedge => [90.0, 116.0, 70.0],
        Wall::Shadow => [52.0, 44.0, 50.0], Wall::None => PARCH,
    }
}

fn solid(t: &Tile) -> bool { t.wall != Wall::None && t.wall != Wall::Tree && t.wall != Wall::Palisade && !t.boulder() }

/// One pixel of a floor's plan at (fx, fy) in cells (px, py its world pixel, cs pixels a cell;
/// `go` the floor's cell (0, 0) in the world, so what is hashed is the world's cell).
fn plan_pixel(f: &Floor, kind: SiteKind, fx: f32, fy: f32, px: i64, py: i64, cs: f32, go: (i64, i64)) -> Rgb {
    let (cx, cy) = (fx.floor() as i32, fy.floor() as i32);
    let (wcx, wcy) = (cx as i64 + go.0, cy as i64 + go.1);
    let (u0, v0) = (fx - cx as f32, fy - cy as f32);
    let t = f.at(cx, cy);
    let line = (1.1 / cs).max(0.025);
    // Grain and mottling, keyed on world pixels.
    let grain = 0.07 * (u(px / 3, py / 3, 0x6A1) - 0.5) + 0.05 * (u(px / 11, py / 11, 0x6A2) - 0.5);
    let gr = |c: Rgb| [c[0] * (1.0 + grain), c[1] * (1.0 + grain), c[2] * (1.0 + grain)];
    if solid(t) {
        let mut c = wall_wash(t.wall, kind);
        // Rock hatched; masonry in courses; timber in planks.
        match t.wall {
            Wall::Rock | Wall::Shadow => { if (px - py).rem_euclid(5) == 0 { c = mix(c, INK, 0.35); } if (px + 2 * py).rem_euclid(13) == 0 { c = mix(c, INK, 0.15); } }
            Wall::Brick => {
                let row = (v0 * 3.0) as i32;
                let off = if row % 2 == 0 { 0.0 } else { 0.5 };
                if (v0 * 3.0).fract() < line * 3.0 || ((u0 * 2.0 + off).fract() < line * 2.0) { c = mix(c, INK, 0.45); }
            }
            Wall::Timber => { if (u0 * 4.0).fract() < line * 4.0 { c = mix(c, INK, 0.4); } }
            Wall::Bars => { if (u0 * 4.0).fract() < 0.25 { c = mix(c, INK, 0.7); } else { c = mix(ground_wash(t.ground), PARCH, 0.2); } }
            _ => {}
        }
        if t.wall == Wall::Shadow && u(px / 2, py / 2, 0x5AD) < 0.02 { c = [150.0, 40.0, 30.0]; }
        // The rim: ink where the wall meets open ground.
        let open = |dx: i32, dy: i32| !solid(f.at(cx + dx, cy + dy));
        let e = line * 1.6;
        if (u0 < e && open(-1, 0)) || (u0 > 1.0 - e && open(1, 0)) || (v0 < e && open(0, -1)) || (v0 > 1.0 - e && open(0, 1)) { return INK; }
        return gr(c);
    }
    // Ground, blended a little toward its neighbours' (so grounds meet softly).
    let mut c = ground_wash(t.ground);
    let gx = if u0 < 0.25 { -1 } else if u0 > 0.75 { 1 } else { 0 };
    let gy = if v0 < 0.25 { -1 } else if v0 > 0.75 { 1 } else { 0 };
    if gx != 0 || gy != 0 {
        let n = f.at(cx + gx, cy + gy);
        if !solid(n) && n.ground != t.ground {
            let d = (if gx != 0 { (u0 - 0.5).abs() - 0.25 } else { 0.0 }).max(if gy != 0 { (v0 - 0.5).abs() - 0.25 } else { 0.0 }) * 2.0;
            c = mix(c, ground_wash(n.ground), d * 0.5);
        }
    }
    c = mix(c, PARCH, 0.12);
    match t.ground {
        Ground::Flags | Ground::Marble => {
            let row = (fy * 2.0) as i64;
            let off = if row % 2 == 0 { 0.0 } else { 0.5 };
            if (fy * 2.0).fract() < line * 2.0 || (fx * 2.0 + off).fract() < line * 2.0 { c = mix(c, INK, if t.ground == Ground::Marble { 0.12 } else { 0.22 }); }
            if t.ground == Ground::Marble && (px + py).rem_euclid(17) == 0 { c = mix(c, [120.0, 110.0, 120.0], 0.3); }
        }
        Ground::Cobbles => {
            let (sx, sy) = ((fx * 3.0).floor() as i64, (fy * 3.0).floor() as i64);
            let (lx, ly) = ((fx * 3.0).fract() - 0.5 - 0.2 * (u(sx, sy, 7) - 0.5), (fy * 3.0).fract() - 0.5);
            let d = (lx * lx + ly * ly).sqrt();
            if d > 0.42 { c = mix(c, INK, 0.25); } else if d < 0.25 && lx + ly < 0.0 { c = mix(c, PARCH, 0.2); }
        }
        Ground::Wood => { if (fy * 4.0).fract() < line * 4.0 { c = mix(c, INK, 0.35); } if (fx * 1.0 + if (fy * 4.0) as i64 % 2 == 0 { 0.0 } else { 0.5 }).fract() < line { c = mix(c, INK, 0.25); } }
        Ground::Grass | Ground::Moss => { if h(px / 2, py / 3, 0x9A5) % 37 == 0 { c = mix(c, [80.0, 104.0, 60.0], 0.7); } }
        Ground::Sand | Ground::Snow => { if h(px, py, 0x5A) % 61 == 0 { c = mix(c, INK, 0.25); } }
        Ground::Shallows | Ground::Water => {
            let r = ((fx * 2.3 + (fy * 1.7).sin() * 0.4) * 3.0).fract();
            if r < 0.08 && h(px / 6, py / 2, 0x77) % 3 == 0 { c = mix(c, [60.0, 90.0, 110.0], 0.45); }
        }
        Ground::Lava => { let n = u(px / 4, py / 4, 0x1A); c = mix([238.0, 132.0, 36.0], [196.0, 56.0, 22.0], n); }
        Ground::Rubble => { if h(px / 3, py / 3, 0x2B) % 9 == 0 { c = mix(c, INK, 0.35); } }
        Ground::Carpet => { if u0 < 0.08 || u0 > 0.92 { c = mix(c, [200.0, 160.0, 70.0], 0.6); } }
        Ground::Field => { if (fy * 3.0).fract() < 0.22 { c = mix(c, [120.0, 104.0, 60.0], 0.5); } else if h(px / 2, py, 0xF1) % 7 == 0 { c = mix(c, [140.0, 150.0, 70.0], 0.5); } }
        Ground::Ice => { if (px + 2 * py).rem_euclid(23) == 0 { c = mix(c, [130.0, 154.0, 166.0], 0.5); } }
        _ => {}
    }
    // Shadow down-light (south and east of walls), a soft fall-off.
    let wall_n = solid(f.at(cx, cy - 1));
    let wall_w = solid(f.at(cx - 1, cy));
    let wall_nw = solid(f.at(cx - 1, cy - 1));
    let mut sh: f32 = 0.0;
    if wall_n { sh = sh.max((1.0 - v0 / 0.45).max(0.0)); }
    if wall_w { sh = sh.max((1.0 - u0 / 0.45).max(0.0)); }
    if wall_nw && !wall_n && !wall_w { sh = sh.max((1.0 - (u0 * u0 + v0 * v0).sqrt() / 0.45).max(0.0)); }
    if f.at(cx, cy - 1).wall == Wall::Tree { sh = sh.max((1.0 - v0 / 0.35).max(0.0) * 0.6); }
    c = mix(c, [70.0, 58.0, 48.0], 0.32 * sh);
    // Trees, boulders and palisades stand on the ground.
    if t.boulder() {
        let (ox, oy) = (0.5 + 0.12 * (u(wcx, wcy, 4) - 0.5), 0.55);
        let (dx, dy) = ((u0 - ox) / 0.42, (v0 - oy) / 0.34);
        let d = (dx * dx + dy * dy).sqrt() * (1.0 + 0.08 * ((dy.atan2(dx) * 5.0 + cx as f32).sin()));
        if d < 1.0 {
            let mut bc: Rgb = [150.0, 144.0, 134.0];
            if dx + dy < -0.5 { bc = mix(bc, PARCH, 0.25); } else if dx + dy > 0.3 && (px - py).rem_euclid(3) == 0 { bc = mix(bc, INK, 0.45); }
            if d > 1.0 - line * 3.0 { bc = INK; }
            return gr(bc);
        }
        if ((u0 - ox - 0.08).powi(2) / 0.2 + (v0 - oy - 0.1).powi(2) / 0.14) < 1.0 { c = mix(c, [70.0, 58.0, 48.0], 0.25); }
    }
    match t.wall {
        Wall::Tree => {
            let (ox, oy) = (0.5 + 0.18 * (u(wcx, wcy, 1) - 0.5), 0.5 + 0.18 * (u(wcx, wcy, 2) - 0.5));
            let r = 0.46 + 0.08 * u(wcx, wcy, 3);
            let (dx, dy) = (u0 - ox, v0 - oy);
            let d = (dx * dx + dy * dy).sqrt();
            let wob = r * (1.0 + 0.07 * ((dy.atan2(dx) * 6.0 + cx as f32).sin()));
            if d < wob {
                let mut tc: Rgb = match t.ground { Ground::Ash => [96.0, 88.0, 84.0], Ground::Snow => [120.0, 140.0, 120.0], Ground::Sand => [150.0, 150.0, 90.0], Ground::Mud => [96.0, 118.0, 80.0], _ => [110.0, 138.0, 84.0] };
                if t.ground == Ground::Snow && dx + dy < -0.1 { tc = mix(tc, [244.0, 246.0, 248.0], 0.6); }
                if dx + dy < -0.15 { tc = mix(tc, PARCH, 0.2); } else if dx + dy > 0.15 && (px - py).rem_euclid(3) == 0 { tc = mix(tc, INK, 0.4); }
                if d > wob - line * 1.4 { tc = INK; }
                return gr(tc);
            }
            if ((dx - 0.08).powi(2) + (dy - 0.1).powi(2)).sqrt() < r { c = mix(c, [70.0, 58.0, 48.0], 0.2); }
        }
        Wall::Palisade => {
            let k = (u0 * 4.0) as i32;
            let lu = (u0 * 4.0).fract();
            let mut pc = wall_wash(Wall::Palisade, kind);
            if lu < 0.15 || lu > 0.85 { pc = INK; } else if lu < 0.4 { pc = mix(pc, PARCH, 0.2); }
            let _ = k;
            return gr(pc);
        }
        _ => {}
    }
    gr(c)
}

/// What a cell's drawing depends on (to know when to draw it again).
pub fn cell_sig(f: &Floor, x: i32, y: i32) -> u64 {
    let t = f.at(x, y);
    let feat: u64 = match &t.feature {
        Feature::Door { open, lock } => 1000 + *open as u64 * 2 + (*lock > 0) as u64,
        Feature::Gate { open, .. } => 2000 + *open as u64,
        Feature::Lever { pulled, .. } => 3000 + *pulled as u64,
        Feature::Chest { opened, .. } => 4000 + *opened as u64,
        Feature::QuestChest { taken, .. } => 5000 + *taken as u64,
        Feature::Sarcophagus { opened, .. } => 6000 + *opened as u64,
        Feature::Plinth { item } => 7000 + item.is_some() as u64,
        Feature::Trap { armed, .. } => 8000 + *armed as u64,
        Feature::LevelDoor { level } => 8500 + *level as u64,
        Feature::Entrance { site, z } => 8600 + *site as u64 * 7 + *z as u64,
        Feature::RiddleDoor { open, .. } => 8700 + *open as u64,
        Feature::Plate { safe } => 8710 + *safe as u64,
        other => 9000 + std::mem::discriminant(other).hash_u64(),
    };
    (t.ground as u64) | (t.wall as u64) << 8 | feat << 16
}

trait HashU64 { fn hash_u64(&self) -> u64; }
impl<T: std::hash::Hash> HashU64 for T { fn hash_u64(&self) -> u64 { use std::hash::Hasher; let mut s = std::collections::hash_map::DefaultHasher::new(); self.hash(&mut s); s.finish() % 997 } }

/// Draw a feature centred in its cell at (x, y) on screen, `cs` pixels a cell.
pub fn draw_feature(put: &mut dyn FnMut(i64, i64, Rgb, f32), f: &Floor, cx: i32, cy: i32, x: f32, y: f32, cs: f32) {
    let t = f.at(cx, cy);
    let wood: Rgb = [150.0, 106.0, 64.0];
    let stone: Rgb = [176.0, 168.0, 156.0];
    let iron: Rgb = [110.0, 110.0, 118.0];
    let gold: Rgb = [214.0, 172.0, 70.0];
    let mut pen = Pen::new(put, x, y, cs);
    let horizontal = solid(f.at(cx - 1, cy)) || solid(f.at(cx + 1, cy));
    match &t.feature {
        Feature::None => {}
        Feature::Door { open, lock } => {
            if *open {
                if horizontal { pen.rect(-0.95, -0.95, -0.7, 0.2, wood); } else { pen.rect(-0.95, -0.95, 0.2, -0.7, wood); }
            } else {
                if horizontal { pen.rect(-1.0, -0.22, 1.0, 0.22, wood); for k in [-0.5f32, 0.0, 0.5] { pen.line((k, -0.22), (k, 0.22), INK, 1.0); } }
                else { pen.rect(-0.22, -1.0, 0.22, 1.0, wood); for k in [-0.5f32, 0.0, 0.5] { pen.line((-0.22, k), (0.22, k), INK, 1.0); } }
                if *lock > 0 { pen.ellipse(0.0, 0.0, 0.16, 0.16, gold); pen.rect_f(-0.03, -0.02, 0.03, 0.1, INK, Finish::Paint); }
            }
        }
        Feature::Gate { open, .. } => {
            if *open { for k in 0..5 { let a = -0.8 + k as f32 * 0.4; pen.line((a, -0.95), (a, -0.6), iron, 2.0); } }
            else if horizontal { for k in 0..6 { let a = -0.9 + k as f32 * 0.36; pen.line((a, -0.3), (a, 0.3), iron, 2.0); } pen.line((-1.0, -0.3), (1.0, -0.3), INK, 1.5); pen.line((-1.0, 0.3), (1.0, 0.3), INK, 1.5); }
            else { for k in 0..6 { let a = -0.9 + k as f32 * 0.36; pen.line((-0.3, a), (0.3, a), iron, 2.0); } pen.line((-0.3, -1.0), (-0.3, 1.0), INK, 1.5); pen.line((0.3, -1.0), (0.3, 1.0), INK, 1.5); }
        }
        Feature::Lever { pulled, .. } => {
            pen.rect(-0.35, 0.2, 0.35, 0.5, iron);
            let tip = if *pulled { (0.45, -0.5) } else { (-0.45, -0.5) };
            pen.bone(&[(0.0, 0.3), tip], wood, (cs / 16.0).max(1.5));
            pen.ellipse(tip.0, tip.1, 0.13, 0.13, [170.0, 50.0, 40.0]);
        }
        Feature::StairsDown | Feature::StairsUp => {
            let down = matches!(t.feature, Feature::StairsDown);
            for k in 0..5 {
                let v0 = -0.85 + k as f32 * 0.34;
                let shade = if down { k as f32 * 0.16 } else { (4 - k) as f32 * 0.16 };
                pen.rect(-0.8, v0, 0.8, v0 + 0.3, mix(stone, INK, shade));
            }
            let red: Rgb = [160.0, 40.0, 30.0];
            if down { pen.path(&[(-0.35, -0.15), (0.0, 0.25), (0.35, -0.15)], red, (cs / 14.0).max(1.5)); } else { pen.path(&[(-0.35, 0.2), (0.0, -0.2), (0.35, 0.2)], red, (cs / 14.0).max(1.5)); }
        }
        Feature::LadderUp | Feature::LadderDown => {
            if matches!(t.feature, Feature::LadderDown) { pen.ellipse_f(0.0, 0.0, 0.75, 0.75, DARK, Finish::Plain); }
            for s in [-0.35f32, 0.35] { pen.bone(&[(s, -0.85), (s, 0.85)], wood, (cs / 18.0).max(1.2)); }
            for k in 0..5 { let v = -0.65 + k as f32 * 0.33; pen.line((-0.35, v), (0.35, v), wood, (cs / 20.0).max(1.0)); }
        }
        Feature::Hole => { pen.ellipse(0.0, 0.05, 0.7, 0.55, DARK); pen.dot(-0.6, 0.5, stone); pen.dot(0.55, -0.45, stone); }
        Feature::RopeSpot => {
            pen.ellipse_f(0.0, 0.0, 0.62, 0.5, [226.0, 218.0, 196.0], Finish::Plain);
            pen.glow(0.0, 0.0, 0.7, [255.0, 250.0, 230.0], 0.4);
            pen.path(&[(0.0, -0.9), (0.05, -0.3), (-0.08, 0.2), (0.0, 0.6)], [190.0, 160.0, 110.0], (cs / 16.0).max(1.5));
        }
        Feature::Exit => {
            pen.glow(0.0, 0.0, 0.9, [255.0, 240.0, 200.0], 0.35);
            pen.path(&[(-0.4, 0.15), (0.0, -0.35), (0.4, 0.15)], [160.0, 40.0, 30.0], (cs / 12.0).max(1.5));
            pen.path(&[(-0.4, 0.5), (0.0, 0.0), (0.4, 0.5)], [160.0, 40.0, 30.0], (cs / 12.0).max(1.5));
        }
        Feature::Chest { opened, .. } => {
            if *opened { pen.rect(-0.6, -0.15, 0.6, 0.45, wood); pen.rect_f(-0.5, -0.05, 0.5, 0.35, DARK, Finish::Plain); pen.rect(-0.6, -0.6, 0.6, -0.2, mix(wood, PARCH, 0.15)); }
            else { glyphs::draw(pen_put(&mut pen), Glyph::Chest, x, y, cs * 0.85, None); }
        }
        Feature::QuestChest { taken, .. } => {
            pen.rect(-0.7, -0.4, 0.7, 0.5, [120.0, 80.0, 50.0]);
            for k in [-0.45f32, 0.0, 0.45] { pen.rect_f(k - 0.07, -0.4, k + 0.07, 0.5, gold, Finish::Plain); }
            pen.rect(-0.7, -0.62, 0.7, -0.35, [134.0, 92.0, 56.0]);
            if !*taken { pen.glow(0.0, -0.2, 1.1, [255.0, 220.0, 120.0], 0.35); pen.ellipse(0.0, 0.0, 0.12, 0.12, gold); }
        }
        Feature::Sarcophagus { opened, .. } => {
            let (a, b) = if horizontal { (0.9, 0.5) } else { (0.5, 0.9) };
            pen.rect(-a, -b, a, b, stone);
            if *opened { pen.rect_f(-a + 0.1, -b + 0.1, a - 0.1, b - 0.1, DARK, Finish::Plain); pen.poly(&[(-a, -b + 0.3), (a - 0.2, -b - 0.1), (a + 0.05, -b + 0.15), (-a + 0.2, -b + 0.55)], mix(stone, PARCH, 0.2)); }
            else { pen.line((-a * 0.6, 0.0), (a * 0.6, 0.0), INK, 1.0); pen.line((0.0, -b * 0.6), (0.0, b * 0.6), INK, 1.0); }
        }
        Feature::Altar => {
            pen.rect(-0.8, -0.45, 0.8, 0.45, mix(stone, PARCH, 0.25));
            pen.rect_f(-0.2, -0.45, 0.2, 0.45, [150.0, 40.0, 40.0], Finish::Plain);
            for s in [-0.6f32, 0.6] { pen.ellipse(s, -0.2, 0.07, 0.07, PARCH); pen.glow(s, -0.3, 0.3, [255.0, 200.0, 100.0], 0.6); }
        }
        Feature::Fountain | Feature::Well => {
            pen.ellipse(0.0, 0.0, 0.8, 0.8, stone);
            pen.ellipse_f(0.0, 0.0, 0.55, 0.55, [112.0, 146.0, 156.0], Finish::Plain);
            if matches!(t.feature, Feature::Well) { pen.line((-0.85, -0.85), (0.85, -0.85), wood, 2.0); for s in [-0.85f32, 0.85] { pen.line((s, -0.85), (s, 0.3), wood, 2.0); } }
            else { pen.ellipse(0.0, 0.0, 0.12, 0.12, mix(stone, PARCH, 0.3)); }
        }
        Feature::Brazier | Feature::Campfire | Feature::Sconce => {
            if matches!(t.feature, Feature::Campfire) { for (a, b) in [((-0.5, 0.4), (0.5, -0.2)), ((-0.5, -0.2), (0.5, 0.4))] { pen.bone(&[a, b], wood, (cs / 14.0).max(1.5)); } }
            else if matches!(t.feature, Feature::Brazier) { pen.ellipse(0.0, 0.15, 0.45, 0.32, iron); }
            else { pen.rect(-0.12, -0.2, 0.12, 0.4, wood); }
            pen.glow(0.0, -0.15, 1.3, [255.0, 190.0, 90.0], 0.55);
            pen.ellipse_f(0.0, -0.15, 0.2, 0.28, [250.0, 200.0, 100.0], Finish::Plain);
            pen.ellipse_f(0.0, -0.08, 0.1, 0.14, [255.0, 240.0, 190.0], Finish::Paint);
        }
        Feature::Statue => { pen.rect(-0.6, 0.35, 0.6, 0.8, stone); pen.ellipse(0.0, -0.5, 0.22, 0.22, mix(stone, PARCH, 0.2)); pen.limb((0.0, -0.3), 0.28, (0.0, 0.35), 0.38, mix(stone, PARCH, 0.15)); }
        Feature::Pillar => { pen.ellipse(0.06, 0.08, 0.62, 0.62, mix(stone, INK, 0.3)); pen.ellipse(0.0, 0.0, 0.6, 0.6, stone); pen.ellipse_f(0.0, 0.0, 0.42, 0.42, mix(stone, PARCH, 0.25), Finish::Paint); }
        Feature::Bones => { let bone: Rgb = [226.0, 218.0, 196.0]; pen.bone(&[(-0.5, 0.3), (0.3, 0.0)], bone, (cs / 18.0).max(1.0)); pen.bone(&[(-0.1, 0.5), (0.4, 0.5)], bone, (cs / 18.0).max(1.0)); pen.ellipse(0.3, -0.35, 0.22, 0.2, bone); pen.dot(0.25, -0.35, INK); pen.dot(0.37, -0.35, INK); }
        Feature::Web => { let c: Rgb = [230.0, 230.0, 226.0]; for k in 0..6 { let a = k as f32 * 1.047; pen.line_a((-1.0, -1.0), (-1.0 + 1.9 * a.cos().abs(), -1.0 + 1.9 * a.sin().abs()), c, 1.0, 0.7); } for r in [0.6f32, 1.1, 1.6] { let pts: Vec<(f32, f32)> = (0..=6).map(|k| { let a = k as f32 * 0.26; (-1.0 + r * a.cos(), -1.0 + r * a.sin()) }).collect(); for w in pts.windows(2) { pen.line_a(w[0], w[1], c, 1.0, 0.6); } } }
        Feature::Table | Feature::Counter => { let (a, b) = if matches!(t.feature, Feature::Counter) { (1.0, 0.45) } else { (0.75, 0.55) }; pen.rect(-a, -b, a, b, wood); pen.line((-a, 0.0), (a, 0.0), mix(wood, INK, 0.5), 1.0); }
        Feature::Bed => { super::furniture::bed(&mut pen, [150.0, 60.0, 50.0], false); }
        Feature::Barrel => { super::furniture::barrel(&mut pen, 0.0, 0.0, 0.6); }
        Feature::Crate => { pen.rect(-0.6, -0.6, 0.6, 0.6, [176.0, 136.0, 90.0]); pen.line((-0.6, -0.6), (0.6, 0.6), INK, 1.0); pen.line((-0.6, 0.6), (0.6, -0.6), INK, 1.0); }
        Feature::Bookshelf => { pen.rect(-0.95, -0.45, 0.95, 0.45, wood); for k in 0..7 { let a = -0.85 + k as f32 * 0.25; let c = [[140.0, 50.0, 40.0], [60.0, 80.0, 120.0], [80.0, 110.0, 60.0], [170.0, 140.0, 60.0]][(h(cx as i64, cy as i64 + k, 3) % 4) as usize]; pen.rect_f(a, -0.35, a + 0.18, 0.35, c, Finish::Plain); } }
        Feature::Throne => { pen.rect(-0.55, -0.8, 0.55, -0.3, gold); pen.rect(-0.55, -0.35, 0.55, 0.6, [140.0, 40.0, 40.0]); pen.ellipse(0.0, -0.65, 0.12, 0.12, [180.0, 40.0, 50.0]); }
        Feature::Anvil => { pen.poly(&[(-0.75, -0.3), (0.5, -0.3), (0.8, -0.1), (0.4, 0.05), (0.2, 0.05), (0.3, 0.5), (-0.4, 0.5), (-0.3, 0.05), (-0.75, 0.0)], iron); }
        Feature::Grave => { pen.ellipse(0.0, 0.25, 0.45, 0.6, [150.0, 130.0, 100.0]); pen.rect(-0.3, -0.85, 0.3, -0.25, stone); pen.line((0.0, -0.75), (0.0, -0.4), INK, 1.0); pen.line((-0.15, -0.62), (0.15, -0.62), INK, 1.0); }
        Feature::Tent => { pen.poly(&[(-0.95, 0.75), (0.0, -0.85), (0.95, 0.75)], [176.0, 140.0, 96.0]); pen.poly(&[(-0.2, 0.75), (0.0, 0.2), (0.2, 0.75)], DARK); }
        Feature::Plinth { item } => { pen.rect(-0.6, -0.6, 0.6, 0.6, mix(stone, PARCH, 0.2)); if let Some(it) = item { pen.glow(0.0, 0.0, 1.0, [255.0, 220.0, 130.0], 0.4); glyphs::draw(pen_put(&mut pen), glyph(&it.def().glyph), x, y, cs * 0.6, None); } }
        Feature::Grate => { pen.rect(-0.7, -0.7, 0.7, 0.7, DARK); for k in 0..5 { let a = -0.56 + k as f32 * 0.28; pen.line((a, -0.7), (a, 0.7), iron, 1.5); pen.line((-0.7, a), (0.7, a), iron, 1.5); } }
        Feature::Sign { .. } => { pen.line((0.0, 0.2), (0.0, 0.9), wood, 2.0); pen.rect(-0.6, -0.5, 0.6, 0.25, mix(wood, PARCH, 0.3)); for k in [-0.25f32, -0.05] { pen.line((-0.4, k), (0.4, k), INK, 1.0); } }
        Feature::Trap { armed, .. } => { if *armed { pen.rect_f(-0.45, -0.45, 0.45, 0.45, mix(stone, INK, 0.12), Finish::Plain); } }
        Feature::SecretDoor => {}
        Feature::Lore { look: 0, .. } => {
            // A carved panel: lines of writing in the stone.
            pen.rect(-0.62, -0.62, 0.62, 0.62, [168.0, 158.0, 144.0]);
            pen.line((-0.62, -0.62), (0.62, -0.62), INK, 1.0); pen.line((-0.62, 0.62), (0.62, 0.62), INK, 1.0);
            for k in 0..4 { let y = -0.4 + k as f32 * 0.26; pen.line((-0.45, y), (0.45 - (k % 2) as f32 * 0.25, y), [80.0, 70.0, 64.0], 1.0); }
        }
        Feature::Lore { look: 1, .. } => {
            // Remains: a skull and scattered bones.
            pen.ellipse(-0.1, -0.1, 0.24, 0.2, [226.0, 218.0, 196.0]);
            pen.dot(-0.18, -0.12, INK); pen.dot(-0.02, -0.12, INK);
            pen.line((-0.5, 0.35), (0.3, 0.15), [214.0, 206.0, 186.0], 2.0); pen.line((0.1, 0.45), (0.55, 0.05), [214.0, 206.0, 186.0], 2.0);
        }
        Feature::Lore { .. } => {
            pen.rect(-0.45, -0.35, 0.45, 0.35, [176.0, 130.0, 60.0]);
            for k in 0..3 { let y = -0.18 + k as f32 * 0.18; pen.line((-0.3, y), (0.3, y), [96.0, 64.0, 30.0], 1.0); }
        }
        Feature::Plate { safe } => {
            pen.rect(-0.8, -0.8, 0.8, 0.8, [150.0, 142.0, 130.0]);
            pen.line((-0.8, -0.8), (0.8, -0.8), [90.0, 84.0, 78.0], 1.0); pen.line((-0.8, 0.8), (0.8, 0.8), [90.0, 84.0, 78.0], 1.0);
            pen.line((-0.8, -0.8), (-0.8, 0.8), [90.0, 84.0, 78.0], 1.0); pen.line((0.8, -0.8), (0.8, 0.8), [90.0, 84.0, 78.0], 1.0);
            if *safe { pen.ellipse(0.0, -0.05, 0.22, 0.2, [214.0, 206.0, 190.0]); pen.dot(-0.08, -0.07, INK); pen.dot(0.08, -0.07, INK); }
        }
        Feature::RiddleDoor { open, .. } => {
            if *open { pen.rect_f(-0.6, -0.8, 0.6, 0.8, DARK, Finish::Plain); }
            else {
                pen.rect(-0.7, -0.85, 0.7, 0.85, [150.0, 140.0, 126.0]);
                pen.ellipse(0.0, -0.1, 0.42, 0.48, [176.0, 166.0, 150.0]);
                pen.dot(-0.15, -0.22, INK); pen.dot(0.15, -0.22, INK);
                pen.path(&[(-0.18, 0.15), (0.0, 0.22), (0.18, 0.15)], INK, 1.5);
            }
        }
        Feature::Entrance { .. } if matches!(t.ground, Ground::Cobbles | Ground::Flags | Ground::Marble) && t.wall == Wall::None && !matches!(f.at(cx, cy - 1).wall, Wall::Rock | Wall::Timber) => {
            // A town's grate into its sewers.
            pen.rect(-0.7, -0.7, 0.7, 0.7, DARK); for k in 0..5 { let a = -0.56 + k as f32 * 0.28; pen.line((a, -0.7), (a, 0.7), iron, 1.5); pen.line((-0.7, a), (0.7, a), iron, 1.5); }
        }
        Feature::Entrance { .. } => {
            // A dark mouth with a step down and a lantern's glow.
            pen.ellipse(0.0, 0.1, 0.72, 0.55, [70.0, 62.0, 58.0]);
            pen.ellipse_f(0.0, 0.18, 0.5, 0.38, DARK, Finish::Plain);
            for k in 0..3 { let v = 0.0 + k as f32 * 0.14; pen.line((-0.3 + k as f32 * 0.06, v), (0.3 - k as f32 * 0.06, v), [120.0, 110.0, 100.0], 1.0); }
            pen.path(&[(-0.25, -0.55), (0.0, -0.8), (0.25, -0.55)], [160.0, 40.0, 30.0], (cs / 14.0).max(1.5));
        }
        Feature::LevelDoor { .. } => {
            if horizontal { pen.rect(-1.0, -0.25, 1.0, 0.25, [120.0, 110.0, 120.0]); } else { pen.rect(-0.25, -1.0, 0.25, 1.0, [120.0, 110.0, 120.0]); }
            pen.glow(0.0, 0.0, 0.6, [150.0, 120.0, 230.0], 0.6);
            pen.path(&[(-0.15, 0.15), (0.0, -0.18), (0.15, 0.15), (-0.15, 0.15)], [200.0, 180.0, 255.0], 1.5);
        }
        Feature::Rail => { let c: Rgb = [110.0, 104.0, 98.0]; for s in [-0.3f32, 0.3] { pen.line((s, -1.0), (s, 1.0), c, 1.5); } for k in [-0.6f32, 0.0, 0.6] { pen.line((-0.45, k), (0.45, k), wood, 2.0); } }
    }
}

/// The pen's put, to hand to a drawer that wants one.
fn pen_put<'a, 'b>(pen: &'a mut Pen<'b>) -> &'a mut dyn FnMut(i64, i64, Rgb, f32) { pen.put_fn() }

/// A floor's plan kept between frames: a region of it (all of a small floor; on the land the
/// part about the camera, carried along as one walks) at `cs` pixels a cell, cells drawn the
/// first time they come into view and again when what is on them changes. Keyed on world cells
/// (`go` is the floor's cell (0, 0) in the world), so the land looks the same when its floor is
/// put together anew about another tile.
pub struct Plan { pub key: (u32, usize, u32), pub gx0: i64, pub gy0: i64, pub cw: usize, pub ch: usize, pub w: usize, pub h: usize, pub buf: Vec<u32>, sig: Vec<u64> }

impl Plan {
    /// A plan of the world cells (gx0.., gy0..), cw x ch of them.
    pub fn new(key: (u32, usize, u32), gx0: i64, gy0: i64, cw: usize, ch: usize, cs: f32) -> Plan {
        let (w, h) = ((cw as f32 * cs) as usize, (ch as f32 * cs) as usize);
        Plan { key, gx0, gy0, cw, ch, w, h, buf: vec![pack(PARCH); w * h], sig: vec![u64::MAX; cw * ch] }
    }
    /// Whether the world cells (x0..x1, y0..y1) are in the plan.
    pub fn covers(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> bool { x0 >= self.gx0 && y0 >= self.gy0 && x1 <= self.gx0 + self.cw as i64 && y1 <= self.gy0 + self.ch as i64 }
    /// Move the plan to start at world cell (gx0, gy0), keeping what was drawn where the old and
    /// the new overlap.
    pub fn rebase(&mut self, gx0: i64, gy0: i64, cs: f32) {
        let csi = cs as usize;
        let mut buf = vec![pack(PARCH); self.w * self.h];
        let mut sig = vec![u64::MAX; self.cw * self.ch];
        for ry in 0..self.ch as i64 { for rx in 0..self.cw as i64 {
            let (ox, oy) = (gx0 + rx - self.gx0, gy0 + ry - self.gy0);
            if ox < 0 || oy < 0 || ox >= self.cw as i64 || oy >= self.ch as i64 { continue; }
            sig[ry as usize * self.cw + rx as usize] = self.sig[oy as usize * self.cw + ox as usize];
            for py in 0..csi {
                let from = (oy as usize * csi + py) * self.w + ox as usize * csi;
                let to = (ry as usize * csi + py) * self.w + rx as usize * csi;
                buf[to..to + csi].copy_from_slice(&self.buf[from..from + csi]);
            }
        } }
        // (A cell's fittings may reach into its neighbours: the border is drawn again.)
        for ry in 0..self.ch { for rx in 0..self.cw { if rx == 0 || ry == 0 || rx + 1 == self.cw || ry + 1 == self.ch { sig[ry * self.cw + rx] = u64::MAX; } } }
        self.buf = buf;
        self.sig = sig;
        self.gx0 = gx0;
        self.gy0 = gy0;
    }
    /// The plan's pixel at world pixel (wx, wy), if it has it.
    pub fn pixel(&self, wx: i64, wy: i64, cs: f32) -> Option<u32> {
        let (px, py) = (wx - (self.gx0 as f32 * cs) as i64, wy - (self.gy0 as f32 * cs) as i64);
        if px < 0 || py < 0 || px as usize >= self.w || py as usize >= self.h { None } else { Some(self.buf[py as usize * self.w + px as usize]) }
    }
    /// Bring the floor's cells (x0..x1, y0..y1) up to date (floor cells; `go` the floor's cell
    /// (0, 0) in the world).
    pub fn refresh(&mut self, f: &Floor, kind: SiteKind, cs: f32, go: (i64, i64), x0: i32, y0: i32, x1: i32, y1: i32) {
        use rayon::prelude::*;
        // The floor's cells in the plan.
        let (lx0, ly0) = ((self.gx0 - go.0) as i32, (self.gy0 - go.1) as i32);
        let (x0, y0) = (x0.max(0).max(lx0), y0.max(0).max(ly0));
        let (x1, y1) = (x1.min(f.w as i32).min(lx0 + self.cw as i32), y1.min(f.h as i32).min(ly0 + self.ch as i32));
        let inside = |x: i32, y: i32| f.inside(x, y) && x >= lx0 && y >= ly0 && x < lx0 + self.cw as i32 && y < ly0 + self.ch as i32;
        let ri = |x: i32, y: i32| (y - ly0) as usize * self.cw + (x - lx0) as usize;
        // Cells whose drawing changed, and their neighbours (rims and shadows reach a cell).
        let mut stale: Vec<(i32, i32)> = Vec::new();
        let mut mark = vec![false; self.cw * self.ch];
        for y in y0..y1 { for x in x0..x1 {
            let s = cell_sig(f, x, y);
            if self.sig[ri(x, y)] != s {
                self.sig[ri(x, y)] = s;
                for dy in -1..=1 { for dx in -1..=1 { let (nx, ny) = (x + dx, y + dy); if inside(nx, ny) && !mark[ri(nx, ny)] { mark[ri(nx, ny)] = true; stale.push((nx, ny)); } } }
            }
        } }
        if stale.is_empty() { return; }
        let csi = cs as usize;
        let w = self.w;
        // Ground and walls, cell by cell (in parallel when many).
        let draw_cell = |(x, y): &(i32, i32)| -> (i32, i32, Vec<u32>) {
            let mut out = vec![0u32; csi * csi];
            for py in 0..csi { for px in 0..csi {
                let (wx, wy) = ((*x as i64 + go.0) * csi as i64 + px as i64, (*y as i64 + go.1) * csi as i64 + py as i64);
                let (fx, fy) = (*x as f32 + (px as f32 + 0.5) / cs, *y as f32 + (py as f32 + 0.5) / cs);
                out[py * csi + px] = pack(plan_pixel(f, kind, fx, fy, wx, wy, cs, go));
            } }
            (*x, *y, out)
        };
        let cells: Vec<(i32, i32, Vec<u32>)> = if stale.len() > 40 { stale.par_iter().map(draw_cell).collect() } else { stale.iter().map(draw_cell).collect() };
        for (x, y, out) in cells {
            let (rx, ry) = ((x - lx0) as usize, (y - ly0) as usize);
            for py in 0..csi { let row = (ry * csi + py) * w + rx * csi; if row + csi <= self.buf.len() { self.buf[row..row + csi].copy_from_slice(&out[py * csi..py * csi + csi]); } }
        }
        // Then the fittings over them (they may reach a little beyond their cell).
        let (pw, ph) = (self.w, self.h);
        let buf = &mut self.buf;
        let mut put = |x: i64, y: i64, c: Rgb, a: f32| {
            if x < 0 || y < 0 || x as usize >= pw || y as usize >= ph { return; }
            let k = y as usize * pw + x as usize;
            buf[k] = pack(mix(unpack(buf[k]), c, a.clamp(0.0, 1.0)));
        };
        for &(x, y) in &stale {
            if f.at(x, y).feature != Feature::None {
                draw_feature(&mut put, f, x, y, ((x - lx0) as f32 + 0.5) * cs, ((y - ly0) as f32 + 0.5) * cs, cs);
            }
        }
    }
}
