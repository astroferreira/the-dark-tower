//! Places carved into the embark (ROADMAP Stage 6): every region place (`region::plan::Place`)
//! that falls on the embark is made at its column, so the region and the embark show the
//! same thing at the same cell (`views_agree`). The barrow is a mound with its chamber, the
//! circle stands, the henge and the enclosure have their bank and ditch, the fort its
//! ramparts and hut rings, the cairns their heaps, the midden its shell mound, the kill site
//! its bones, the flint mine its shafts (an old working the colony can go down), the quarry
//! its cut, the crannog its made island, the tower and the mill their ruined walls, the
//! drowned village its walls under the water, the ridgeway and the causeway their track, the
//! ford its gravel; the natural wonders get their stone, boulders, bones or spring, and a cave
//! mouth without a planned graph a short cave of its own. The waterfall's step is made where
//! the ground is laid (`generate_local`: `fall_adjust`), not here.
//!
//! Everything is keyed on the place's cell and the column's absolute position, so two
//! neighbouring embarks carve one place the same way.

use super::*;
use super::caves::place_column;
use super::places::{self, Cut, UnderPlace};
use crate::region::plan::{Place, PlaceKind};

/// A column under a fractional position, if on the map.
fn inside(map: &LocalMap, x: f64, y: f64) -> Option<(usize, usize)> {
    let (cx, cy) = (x.round() as i64, y.round() as i64);
    (cx >= 0 && cy >= 0 && cx < map.width as i64 && cy < map.height as i64).then(|| (cx as usize, cy as usize))
}

fn hash64(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}
fn unit(a: i64, b: i64, salt: u64) -> f64 { (hash64(a, b, salt) % 1_000_000) as f64 / 1_000_000.0 }

/// Dry ground: a floor with no water over it, no building.
fn dry(map: &LocalMap, x: usize, y: usize) -> bool {
    let sz = map.surface_z[y * map.width + x];
    if sz < 1 || (sz + 2) as usize >= map.depth { return false; }
    let c = map.cell(x, y, sz as usize);
    let above = map.cell(x, y, sz as usize + 1);
    c.shape == Shape::Floor && above.water == 0 && !matches!(c.material, Material::Block(_) | Material::Wood | Material::Ice)
}

/// Raise a column's ground by `levels` of `material` (a mound, a bank).
fn raise(map: &mut LocalMap, x: usize, y: usize, levels: i32, material: Material) {
    if levels <= 0 || !dry(map, x, y) { return; }
    let n = map.width;
    let sz = map.surface_z[y * n + x];
    if (sz + levels + 2) as usize >= map.depth { return; }
    let old = *map.cell(x, y, sz as usize);
    for z in sz..sz + levels {
        let k = map.idx(x, y, z as usize);
        map.cells[k] = Cell { shape: Shape::Wall, material, water: 0, plant: Plant::None, boulder: false };
    }
    let top = map.idx(x, y, (sz + levels) as usize);
    map.cells[top] = Cell { shape: Shape::Floor, material, water: 0, plant: if matches!(old.plant, Plant::Grass | Plant::Shrub) { Plant::Grass } else { Plant::None }, boulder: false };
    map.surface_z[y * n + x] = sz + levels;
}

/// Cut a column's ground down by `levels` (a ditch, a pit), the new floor of `material`.
fn lower(map: &mut LocalMap, x: usize, y: usize, levels: i32, material: Option<Material>) {
    if levels <= 0 || !dry(map, x, y) { return; }
    let n = map.width;
    let sz = map.surface_z[y * n + x];
    if sz - levels < 2 { return; }
    for z in sz - levels + 1..=sz {
        let k = map.idx(x, y, z as usize);
        map.cells[k] = Cell::AIR;
    }
    let f = map.idx(x, y, (sz - levels) as usize);
    let m = material.unwrap_or(map.cells[f].material);
    map.cells[f] = Cell { shape: Shape::Floor, material: m, water: 0, plant: Plant::None, boulder: false };
    map.surface_z[y * n + x] = sz - levels;
}

fn feature(map: &mut LocalMap, x: usize, y: usize, f: wildlife::Feature) {
    if !dry(map, x, y) { return; }
    let n = map.width;
    let sz = map.surface_z[y * n + x];
    let k = map.idx(x, y, sz as usize);
    map.cells[k].plant = Plant::None;
    map.cells[k].boulder = false;
    map.features[y * n + x] = f;
}

fn boulder(map: &mut LocalMap, x: usize, y: usize) {
    if !dry(map, x, y) { return; }
    let sz = map.surface_z[y * map.width + x];
    let k = map.idx(x, y, sz as usize);
    map.cells[k].plant = Plant::None;
    map.cells[k].boulder = true;
}

/// Every column within an ellipse (`rx`, `ry` columns, turned by `angle`) round `(cx, cy)`,
/// with its distance as a share of the radius.
fn ellipse(map: &LocalMap, cx: f64, cy: f64, rx: f64, ry: f64, angle: f64) -> Vec<(usize, usize, f64)> {
    let (ca, sa) = (angle.cos(), angle.sin());
    let r = rx.max(ry).ceil() as i64 + 1;
    let mut out = Vec::new();
    for dy in -r..=r { for dx in -r..=r {
        let (x, y) = (cx.round() as i64 + dx, cy.round() as i64 + dy);
        if x < 0 || y < 0 || x >= map.width as i64 || y >= map.height as i64 { continue; }
        let (fx, fy) = (x as f64 - cx, y as f64 - cy);
        let (u, v) = (fx * ca + fy * sa, -fx * sa + fy * ca);
        let d = ((u / rx).powi(2) + (v / ry).powi(2)).sqrt();
        if d <= 1.0 { out.push((x as usize, y as usize, d)); }
    } }
    out
}

/// A mound: `levels` high over its inner part, a level at the rim.
fn mound(map: &mut LocalMap, cx: f64, cy: f64, rx: f64, ry: f64, angle: f64, levels: i32, material: Material) {
    for (x, y, d) in ellipse(map, cx, cy, rx, ry, angle) {
        let h = if levels <= 1 { 1 } else if d < 0.55 { levels } else if d < 0.85 { (levels - 1).max(1) } else { 1 };
        raise(map, x, y, h, material);
    }
}

/// A ring round `(cx, cy)` of radius `r` and `width` columns, raised (`levels` > 0) or cut
/// (`levels` < 0), with gaps centred on the given angles (radians), each `gap` columns wide.
fn ring(map: &mut LocalMap, cx: f64, cy: f64, r: f64, width: f64, levels: i32, gaps: &[f64], gap: f64, material: Material) {
    let rr = (r + width).ceil() as i64 + 1;
    for dy in -rr..=rr { for dx in -rr..=rr {
        let (x, y) = (cx.round() as i64 + dx, cy.round() as i64 + dy);
        if x < 0 || y < 0 || x >= map.width as i64 || y >= map.height as i64 { continue; }
        let (fx, fy) = (x as f64 - cx, y as f64 - cy);
        let d = (fx * fx + fy * fy).sqrt();
        if (d - r).abs() > width / 2.0 { continue; }
        let a = fy.atan2(fx);
        let in_gap = gaps.iter().any(|&g| { let da = (a - g + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI; (da * r).abs() < gap / 2.0 });
        if in_gap { continue; }
        if levels > 0 { raise(map, x as usize, y as usize, levels, material); } else { lower(map, x as usize, y as usize, -levels, None); }
    } }
}

/// Ruined walls of dressed stone on a rectangle `w` x `h` columns round `(cx, cy)`: whole
/// stretches stand `levels` high, the rest a course or nothing (`ruin` the share fallen), a
/// doorway in the south wall.
fn ruin_walls(map: &mut LocalMap, cx: f64, cy: f64, w: i64, h: i64, levels: i32, rock: RockType, ruin: f64, salt: u64) {
    let (x0, y0) = (cx.round() as i64 - w / 2, cy.round() as i64 - h / 2);
    for y in y0..=y0 + h { for x in x0..=x0 + w {
        let on_edge = x == x0 || x == x0 + w || y == y0 || y == y0 + h;
        if !on_edge || x < 0 || y < 0 || x >= map.width as i64 || y >= map.height as i64 { continue; }
        if y == y0 + h && x == x0 + w / 2 { continue; } // the doorway
        let (ux, uy) = (x as usize, y as usize);
        if !dry(map, ux, uy) { continue; }
        let u = unit(x, y, salt);
        let hgt = if u < ruin * 0.5 { 0 } else if u < ruin { 1 } else { levels };
        if hgt == 0 { boulder(map, ux, uy); continue; }
        let sz = map.surface_z[uy * map.width + ux];
        if (sz + hgt + 1) as usize >= map.depth { continue; }
        for z in sz + 1..=sz + hgt {
            let k = map.idx(ux, uy, z as usize);
            map.cells[k] = Cell { shape: Shape::Wall, material: Material::Block(rock), water: 0, plant: Plant::None, boulder: false };
        }
        let f = map.idx(ux, uy, sz as usize);
        map.cells[f].plant = Plant::None;
    } }
}

/// A line of columns from `a` to `b` (fractional columns), one column a step.
fn line(a: (f64, f64), b: (f64, f64)) -> Vec<(i64, i64)> {
    let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1);
    let mut out = Vec::with_capacity(steps + 1);
    for t in 0..=steps {
        let f = t as f64 / steps as f64;
        out.push(((a.0 + (b.0 - a.0) * f).round() as i64, (a.1 + (b.1 - a.1) * f).round() as i64));
    }
    out.dedup();
    out
}

/// The nearest dry column to `(x, y)` within `r`, by a hashed order among the nearest.
fn dry_near(map: &LocalMap, x: f64, y: f64, r: i64) -> Option<(usize, usize)> {
    let (cx, cy) = (x.round() as i64, y.round() as i64);
    let mut best: Option<((usize, usize), f64)> = None;
    for dy in -r..=r { for dx in -r..=r {
        let (px, py) = (cx + dx, cy + dy);
        if px < 4 || py < 4 || px >= map.width as i64 - 4 || py >= map.height as i64 - 4 { continue; }
        if !dry(map, px as usize, py as usize) { continue; }
        let d = ((dx * dx + dy * dy) as f64).sqrt();
        if best.map_or(true, |b| d < b.1) { best = Some(((px as usize, py as usize), d)); }
    } }
    best.map(|b| b.0)
}

/// A place laid out on levels (`places::layout`) at the nearest dry ground to `(x, y)`, as an
/// `UnderPlace` of `kind` named for the region place.
fn laid_out(map: &mut LocalMap, p: &Place, kind: places::PlaceKind, x: f64, y: f64, salt: u64) -> Option<UnderPlace> {
    let (mx, my) = dry_near(map, x, y, 6)?;
    let (cuts, walk) = places::layout(map, kind, mx as i32, my as i32, salt)?;
    if !cuts.iter().all(|&c| places::cuttable(map, c)) { return None; }
    for &c in &cuts { places::apply(map, c); }
    Some(UnderPlace { kind, name: p.title(), cause: p.cause.clone(), contents: p.contents.clone(), cells: walk, mouth: Some((mx as u16, my as u16)), found: false })
}

/// Carve the region's places that fall on the embark made round region cell `(cx, cy)`; the
/// under-places made (chambers, shafts, caves).
pub(super) fn carve(map: &mut LocalMap, region: &ZoomRegion, cx: f64, cy: f64, seed: u64) -> Vec<UnderPlace> {
    let n = map.width;
    let col = |ax: f64, ay: f64| place_column(region, cx, cy, n, ax, ay);
    let mut out: Vec<UnderPlace> = Vec::new();
    let reach = 48.0;
    for p in &region.places {
        let (px, py) = col(p.x, p.y);
        let key = (p.x.round() as i64, p.y.round() as i64);
        let salt = seed ^ hash64(key.0, key.1, 0x51E5);
        let angle = unit(key.0, key.1, 0x51E6) * std::f64::consts::PI;
        let on_map = px > -reach && py > -reach && px < n as f64 + reach && py < n as f64 + reach;
        // Tracks run between their cells whatever lies on the map.
        let track_cells: Vec<(f64, f64)> = p.cells.iter().map(|&(x, y)| col(x as f64, y as f64)).collect();
        use PlaceKind::*;
        match p.kind {
            Ridgeway | Causeway if track_cells.len() >= 2 => {
                for w in track_cells.windows(2) {
                    for (x, y) in line(w[0], w[1]) {
                        if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 { continue; }
                        let (ux, uy) = (x as usize, y as usize);
                        if p.kind == Ridgeway { feature(map, ux, uy, wildlife::Feature::Trail); } else {
                            // Split logs laid over the wet ground, a level up where the bog's pools lie.
                            let sz = map.surface_z[uy * n + ux];
                            let above = map.cell(ux, uy, sz as usize + 1).water;
                            if above > 0 {
                                let k = map.idx(ux, uy, sz as usize + 1);
                                map.cells[k] = Cell { shape: Shape::Floor, material: Material::Wood, water: 0, plant: Plant::None, boulder: false };
                                map.surface_z[uy * n + ux] = sz + 1;
                            } else if dry(map, ux, uy) {
                                let k = map.idx(ux, uy, sz as usize);
                                map.cells[k].material = Material::Wood; map.cells[k].plant = Plant::None; map.cells[k].boulder = false;
                            }
                        }
                    }
                }
                continue;
            }
            _ => {}
        }
        if !on_map { continue; }
        match p.kind {
            LongBarrow => {
                mound(map, px, py, 15.0, 5.0, angle, 2, Material::Soil);
                // The chamber: a passage in from the east end at the old ground, a room at its end,
                // the way in a notch in the mound.
                let (dx, dy) = (angle.cos(), angle.sin());
                let mut cells: Vec<((u16, u16), i32)> = Vec::new();
                let mut mouth = None;
                for t in 0..9 {
                    let (x, y) = (px + dx * (14.0 - t as f64), py + dy * (14.0 - t as f64));
                    let Some((ux, uy)) = inside(map, x, y) else { continue };
                    let sz = map.surface_z[uy * n + ux];
                    let base = sz - 2;
                    if base < 2 { continue; }
                    let room = t >= 6;
                    let span: Vec<(i64, i64)> = if room { vec![(0, 0), (-dy.round() as i64, dx.round() as i64), (dy.round() as i64, -dx.round() as i64)] } else { vec![(0, 0)] };
                    for (sx, sy) in span {
                        let (qx, qy) = (ux as i64 + sx, uy as i64 + sy);
                        if qx < 0 || qy < 0 || qx >= n as i64 || qy >= n as i64 { continue; }
                        let (qx, qy) = (qx as usize, qy as usize);
                        if map.surface_z[qy * n + qx] < base + 2 { continue; }
                        let k = map.idx(qx, qy, (base + 1) as usize);
                        map.cells[k] = Cell::AIR;
                        let f = map.idx(qx, qy, base as usize);
                        map.cells[f] = Cell { shape: Shape::Floor, material: Material::Block(RockType::Sandstone), water: 0, plant: Plant::None, boulder: false };
                        if t == 0 {
                            // The notch: the mound cut down to the passage at its end.
                            let top = map.idx(qx, qy, (base + 2) as usize);
                            map.cells[top] = Cell::AIR;
                            map.surface_z[qy * n + qx] = base;
                            mouth = Some((qx as u16, qy as u16));
                        }
                        cells.push(((qx as u16, qy as u16), base));
                    }
                }
                if let Some(m) = mouth {
                    out.push(UnderPlace { kind: places::PlaceKind::Tomb, name: p.title(), cause: p.cause.clone(), contents: p.contents.clone(), cells, mouth: Some(m), found: false });
                }
            }
            RoundBarrows => {
                let spots: Vec<(f64, f64)> = if track_cells.len() >= 2 { track_cells.clone() } else {
                    (0..4).map(|k| { let a = k as f64 * 1.3 + angle; (px + 14.0 * a.cos(), py + 14.0 * a.sin()) }).collect()
                };
                for (k, (x, y)) in spots.iter().enumerate().take(6) {
                    let r = 4.0 + 2.0 * unit(key.0, key.1 + k as i64, 0x51E7);
                    mound(map, *x, *y, r, r, 0.0, 2, Material::Soil);
                }
            }
            ShellMidden => mound(map, px, py, 6.0, 4.0, angle, 1, Material::Gravel),
            StoneCircle => {
                let r = 8.0 + 6.0 * unit(key.0, key.1, 0x51E8);
                let stones = 10 + (hash64(key.0, key.1, 0x51E9) % 10) as usize;
                for k in 0..stones {
                    let a = k as f64 / stones as f64 * std::f64::consts::TAU;
                    if let Some((x, y)) = inside(map, px + r * a.cos(), py + r * a.sin()) { feature(map, x, y, wildlife::Feature::Stone); }
                }
                if unit(key.0, key.1, 0x51EA) < 0.4 { if let Some((x, y)) = inside(map, px, py) { feature(map, x, y, wildlife::Feature::Stone); } }
            }
            StandingStone => { if let Some((x, y)) = inside(map, px, py) { feature(map, x, y, wildlife::Feature::Stone); } }
            Dolmen => {
                for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                    if let Some((x, y)) = inside(map, px + dx, py + dy) { feature(map, x, y, wildlife::Feature::Stone); }
                }
            }
            RockArt => { for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] { if let Some((x, y)) = inside(map, px + dx, py + dy) { boulder(map, x, y); } } }
            Henge => {
                let r = 16.0 + 8.0 * unit(key.0, key.1, 0x51EB);
                ring(map, px, py, r, 2.0, 1, &[angle, angle + std::f64::consts::PI], 4.0, Material::Soil);
                ring(map, px, py, r - 2.5, 2.0, -1, &[angle, angle + std::f64::consts::PI], 4.0, Material::Soil);
            }
            Enclosure => {
                let r = 20.0 + 10.0 * unit(key.0, key.1, 0x51EC);
                let gaps: Vec<f64> = (0..5).map(|k| angle + k as f64 * 1.26).collect();
                ring(map, px, py, r, 2.0, 1, &gaps, 3.0, Material::Soil);
                ring(map, px, py, r + 2.5, 2.0, -1, &gaps, 3.0, Material::Soil);
            }
            HillFort => {
                let r = 26.0 + 10.0 * unit(key.0, key.1, 0x51ED);
                ring(map, px, py, r, 3.0, 1, &[angle], 5.0, Material::Soil);
                ring(map, px, py, r + 3.0, 2.0, -1, &[angle], 5.0, Material::Soil);
                // Hut rings inside: low circles of stone.
                let huts = 3 + (hash64(key.0, key.1, 0x51EE) % 3) as usize;
                for k in 0..huts {
                    let a = k as f64 * 2.1 + angle + 0.7;
                    let d = r * (0.3 + 0.35 * unit(key.0 + k as i64, key.1, 0x51EF));
                    let (hx, hy) = (px + d * a.cos(), py + d * a.sin());
                    for s in 0..8 {
                        let b = s as f64 / 8.0 * std::f64::consts::TAU;
                        if let Some((x, y)) = inside(map, hx + 3.0 * b.cos(), hy + 3.0 * b.sin()) { boulder(map, x, y); }
                    }
                }
            }
            ClearanceCairns => {
                let heaps = 6 + (hash64(key.0, key.1, 0x51F0) % 6) as usize;
                for k in 0..heaps {
                    let (hx, hy) = (px + (unit(key.0 + k as i64, key.1, 0x51F1) - 0.5) * 50.0, py + (unit(key.0, key.1 + k as i64, 0x51F2) - 0.5) * 50.0);
                    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                        if let Some((x, y)) = inside(map, hx + dx, hy + dy) { boulder(map, x, y); }
                    }
                }
            }
            KillSite | BoneBed | FossilBed => {
                let count = if p.kind == FossilBed { 8 } else { 14 };
                for k in 0..count {
                    let (bx, by) = (px + (unit(key.0 + k as i64, key.1, 0x51F3) - 0.5) * 20.0, py + (unit(key.0, key.1 + k as i64, 0x51F4) - 0.5) * 20.0);
                    if let Some((x, y)) = inside(map, bx, by) { feature(map, x, y, wildlife::Feature::Bones); }
                }
            }
            FlintMine => {
                let shafts = 2 + (hash64(key.0, key.1, 0x51F5) % 3) as usize;
                for k in 0..shafts {
                    let (sx, sy) = (px + (unit(key.0 + k as i64, key.1, 0x51F6) - 0.5) * 30.0, py + (unit(key.0, key.1 + k as i64, 0x51F7) - 0.5) * 30.0);
                    if let Some(u) = laid_out(map, p, places::PlaceKind::OldMine, sx, sy, salt ^ k as u64) { out.push(u); }
                }
            }
            CopperWorking => { if let Some(u) = laid_out(map, p, places::PlaceKind::OldMine, px, py, salt) { out.push(u); } }
            Quarry => {
                // An open cut two levels into the rock, spoil heaped round its rim.
                for (x, y, d) in ellipse(map, px, py, 7.0, 4.0, angle) {
                    if d < 0.8 { lower(map, x, y, 2, None); } else { boulder(map, x, y); }
                }
            }
            Crannog => {
                // A made island in the shallows: the bed raised to the water's level, timber and
                // stone, and a causeway to the nearest dry shore.
                let Some((ix, iy)) = inside(map, px, py) else { continue };
                let shore = dry_near(map, px, py, 30);
                let mut cols: Vec<(usize, usize)> = ellipse(map, px, py, 5.0, 5.0, 0.0).into_iter().map(|(x, y, _)| (x, y)).collect();
                if let Some((sx, sy)) = shore {
                    for (x, y) in line((ix as f64, iy as f64), (sx as f64, sy as f64)) { if x >= 0 && y >= 0 && x < n as i64 && y < n as i64 { cols.push((x as usize, y as usize)); } }
                }
                for (x, y) in cols {
                    let sz = map.surface_z[y * n + x];
                    // The water over the column, up to its top.
                    let mut top = sz;
                    while (top + 1) < map.depth as i32 - 1 && map.cell(x, y, top as usize + 1).water > 0 { top += 1; }
                    if top == sz { continue; }
                    for z in sz + 1..=top {
                        let k = map.idx(x, y, z as usize);
                        map.cells[k] = Cell { shape: Shape::Wall, material: Material::Gravel, water: 0, plant: Plant::None, boulder: false };
                    }
                    let t = map.idx(x, y, top as usize);
                    map.cells[t] = Cell { shape: Shape::Floor, material: Material::Wood, water: 0, plant: Plant::None, boulder: false };
                    map.surface_z[y * n + x] = top;
                }
            }
            Watchtower => ruin_walls(map, px, py, 4, 4, 2, RockType::Granite, 0.4, salt),
            MillRuin => {
                ruin_walls(map, px, py, 5, 3, 1, RockType::Sandstone, 0.3, salt);
                if let Some((x, y)) = inside(map, px + 4.0, py) { boulder(map, x, y); }
            }
            DrownedVillage => {
                for k in 0..4 {
                    let (hx, hy) = (px + (unit(key.0 + k, key.1, 0x51F8) - 0.5) * 30.0, py + (unit(key.0, key.1 + k, 0x51F9) - 0.5) * 30.0);
                    let (x0, y0) = (hx.round() as i64 - 2, hy.round() as i64 - 1);
                    for y in y0..=y0 + 3 { for x in x0..=x0 + 4 {
                        let on_edge = x == x0 || x == x0 + 4 || y == y0 || y == y0 + 3;
                        if !on_edge || x < 0 || y < 0 || x >= n as i64 || y >= n as i64 { continue; }
                        let (ux, uy) = (x as usize, y as usize);
                        let sz = map.surface_z[uy * n + ux];
                        if map.cell(ux, uy, sz as usize + 1).water == 0 || (sz + 2) as usize >= map.depth { continue; }
                        let k = map.idx(ux, uy, sz as usize + 1);
                        map.cells[k] = Cell { shape: Shape::Wall, material: Material::Block(RockType::Sandstone), water: 0, plant: Plant::None, boulder: false };
                    } }
                }
            }
            Ford => {
                // A gravel bar across the channel: the bed raised to a wade's depth.
                let (dx, dy) = (angle.cos(), angle.sin());
                for t in -12..=12 {
                    for s in -1..=1 {
                        let (x, y) = (px + dx * t as f64 - dy * s as f64, py + dy * t as f64 + dx * s as f64);
                        let Some((ux, uy)) = inside(map, x, y) else { continue };
                        let sz = map.surface_z[uy * n + ux];
                        let mut top = sz;
                        while (top + 1) < map.depth as i32 - 1 && map.cell(ux, uy, top as usize + 1).water > 0 { top += 1; }
                        if top <= sz { continue; }
                        for z in sz + 1..top {
                            let k = map.idx(ux, uy, z as usize);
                            map.cells[k] = Cell { shape: Shape::Wall, material: Material::Gravel, water: 0, plant: Plant::None, boulder: false };
                        }
                        let f = map.idx(ux, uy, (top - 1).max(sz) as usize);
                        map.cells[f] = Cell { shape: Shape::Floor, material: Material::Gravel, water: 0, plant: Plant::None, boulder: false };
                        let w = map.idx(ux, uy, top as usize);
                        map.cells[w].water = 2;
                        map.surface_z[uy * n + ux] = (top - 1).max(sz);
                    }
                }
            }
            Spring | HotSpring | Geyser | MudPot | SaltSpring | TarSeep => { if let Some((x, y)) = dry_near(map, px, py, 3) { feature(map, x, y, wildlife::Feature::Spring); } }
            Erratic => { if let Some((x, y)) = inside(map, px, py) { boulder(map, x, y); boulder(map, (x + 1).min(n - 1), y); } }
            BoulderField | PetrifiedForest => {
                for (x, y, _) in ellipse(map, px, py, 14.0, 10.0, angle) {
                    if unit(x as i64, y as i64, 0x51FA) < if p.kind == BoulderField { 0.3 } else { 0.08 } { boulder(map, x, y); }
                }
            }
            Tor => {
                for (x, y, d) in ellipse(map, px, py, 3.0, 2.5, angle) { if d < 0.7 { raise(map, x, y, 2, Material::Rock(RockType::Granite)); } else { boulder(map, x, y); } }
            }
            GiantTree => {
                if let Some((x, y)) = dry_near(map, px, py, 2) {
                    let sz = map.surface_z[y * n + x];
                    let k = map.idx(x, y, sz as usize);
                    map.cells[k].plant = Plant::Tree(TreeKind::Broadleaf);
                    map.cells[k].boulder = false;
                    for (dx, dy) in [(1i64, 0i64), (0, 1), (-1, 0), (0, -1)] {
                        let (qx, qy) = (x as i64 + dx, y as i64 + dy);
                        if qx >= 0 && qy >= 0 && (qx as usize) < n && (qy as usize) < n && dry(map, qx as usize, qy as usize) {
                            let q = map.idx(qx as usize, qy as usize, map.surface_z[qy as usize * n + qx as usize] as usize);
                            map.cells[q].plant = Plant::None;
                        }
                    }
                }
            }
            CaveMouth | PaintedCave | RockShelter if p.cave.is_none() => {
                // A mouth the planner gave no graph: a short cave of its own.
                if let Some(u) = laid_out(map, p, places::PlaceKind::Cave, px, py, salt) { out.push(u); }
            }
            _ => {}
        }
    }
    out
}

/// Views agree over a region: for every place the region lists (away from its edge) whose kind
/// the embark carves, whether the embark made round the place's cell shows it. Returns the
/// places shown, the places judged and the titles of those not shown ("X at ax,ay").
pub fn views_agree(world: &crate::world::WorldData, zoom: &crate::region::zoom::ZoomRegion) -> (usize, usize, Vec<String>) {
    let s = (zoom.params.cells_per_tile.max(8) & !1) as i64;
    let (tx, ty) = ((zoom.world_x0 * s) as f64, (zoom.world_y0 * s) as f64);
    let (mut agree, mut judged) = (0usize, 0usize);
    let mut failed: Vec<String> = Vec::new();
    for p in &zoom.places {
        let (px, py) = (p.x - tx + 0.5, p.y - ty + 0.5);
        if px < 2.0 || py < 2.0 || px > zoom.width as f64 - 2.0 || py > zoom.height as f64 - 2.0 { continue; }
        let map = generate_local(world, zoom, None, px, py);
        let Some(ok) = shows(&map, p.kind) else { continue };
        judged += 1;
        if ok { agree += 1; } else { failed.push(format!("{} at {:.0},{:.0}", p.title(), p.x, p.y)); }
    }
    (agree, judged, failed)
}

/// Whether the embark made round a place of `kind` shows it at its middle (`views_agree`): the
/// kind's own sign within reach of the centre. None for a kind the embark does not carve.
pub fn shows(map: &LocalMap, kind: PlaceKind) -> Option<bool> {
    use PlaceKind::*;
    let n = map.width;
    let (cx, cy) = (n as i64 / 2, n as i64 / 2);
    let within = |r: i64| -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for dy in -r..=r { for dx in -r..=r {
            if dx * dx + dy * dy > r * r { continue; }
            let (x, y) = (cx + dx, cy + dy);
            if x >= 0 && y >= 0 && x < n as i64 && y < n as i64 { v.push((x as usize, y as usize)); }
        } }
        v
    };
    let sz = |x: usize, y: usize| map.surface_z[y * n + x];
    let floor = |x: usize, y: usize| map.cell(x, y, sz(x, y) as usize);
    // The ground's typical level in a ring round the centre.
    let ring_median = |r0: i64, r1: i64| -> i32 {
        let mut v: Vec<i32> = within(r1).into_iter().filter(|&(x, y)| { let (dx, dy) = (x as i64 - cx, y as i64 - cy); dx * dx + dy * dy >= r0 * r0 }).map(|(x, y)| sz(x, y)).collect();
        v.sort_unstable();
        v.get(v.len() / 2).copied().unwrap_or(0)
    };
    let features = |r: i64, f: wildlife::Feature| within(r).into_iter().filter(|&(x, y)| map.features[y * n + x] == f).count();
    let boulders = |r: i64| within(r).into_iter().filter(|&(x, y)| floor(x, y).boulder).count();
    let under = |r: i64, kinds: &[places::PlaceKind]| map.places.iter().any(|p| kinds.contains(&p.kind) && p.mouth.map_or(false, |m| (m.0 as i64 - cx).abs() <= r && (m.1 as i64 - cy).abs() <= r));
    let raised = |r: i64, by: i32| { let med = ring_median(r + 8, r + 16); within(r).into_iter().any(|(x, y)| sz(x, y) >= med + by) };
    let cut = |r: i64, by: i32| { let med = ring_median(r + 8, r + 16); within(r).into_iter().any(|(x, y)| sz(x, y) <= med - by) };
    let material = |r: i64, test: &dyn Fn(Material) -> bool| within(r).into_iter().any(|(x, y)| (0..3).any(|up| { let z = sz(x, y) + up; (z as usize) < map.depth && test(map.cell(x, y, z as usize).material) }));
    Some(match kind {
        LongBarrow | RoundBarrows | ShellMidden | Tor => raised(18, 1),
        StoneCircle => features(24, wildlife::Feature::Stone) >= 8,
        StandingStone => features(6, wildlife::Feature::Stone) >= 1,
        Dolmen => features(6, wildlife::Feature::Stone) >= 3,
        Henge | Enclosure | HillFort => raised(40, 1) && cut(44, 1),
        ClearanceCairns => boulders(32) >= 10,
        BoulderField => boulders(16) >= 30,
        PetrifiedForest => boulders(16) >= 5,
        RockArt => boulders(4) >= 2,
        Erratic => boulders(3) >= 1,
        KillSite | BoneBed | FossilBed => features(16, wildlife::Feature::Bones) >= 4,
        FlintMine | CopperWorking => under(24, &[places::PlaceKind::OldMine]),
        CaveMouth | PaintedCave | RockShelter => under(24, &[places::PlaceKind::Cave]),
        Quarry => cut(10, 2),
        Crannog | Causeway => material(12, &|m| m == Material::Wood),
        Watchtower | MillRuin => material(8, &|m| matches!(m, Material::Block(_))),
        DrownedVillage => material(26, &|m| matches!(m, Material::Block(_))),
        Ford => within(16).into_iter().any(|(x, y)| { let z = sz(x, y) + 1; (z as usize) < map.depth && map.cell(x, y, z as usize).water == 2 && floor(x, y).material == Material::Gravel }),
        Ridgeway => features(12, wildlife::Feature::Trail) >= 4,
        Spring | HotSpring | Geyser | MudPot | SaltSpring | TarSeep => features(6, wildlife::Feature::Spring) >= 1,
        GiantTree => within(3).into_iter().any(|(x, y)| matches!(floor(x, y).plant, Plant::Tree(_))),
        Waterfall => {
            // A step of two levels or more among the water-covered columns near the middle.
            let wet: Vec<i32> = within(20).into_iter().filter(|&(x, y)| { let z = sz(x, y) + 1; (z as usize) < map.depth && map.cell(x, y, z as usize).water > 0 }).map(|(x, y)| sz(x, y)).collect();
            match (wet.iter().min(), wet.iter().max()) { (Some(lo), Some(hi)) => hi - lo >= 2, _ => false }
        }
        _ => return Option::None,
    })
}
