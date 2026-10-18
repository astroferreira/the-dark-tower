//! Slopes that fail (ROADMAP Stage 7): landslides, rock avalanches and the fallen flanks of
//! volcanoes, run on the **blended** region (`zoom::generate_zoom`, after the chunks are
//! blended and before the water is routed), so the debris that falls across a valley dams it
//! and the hydrology makes the lake behind. Overlapping windows blend the same heights, so they
//! make the same slides: every decision is read from the heights before any slide is cut, on a
//! lattice in absolute cells.
//!
//! - A **landslide** where the slope passes its rock's limit (soft rock 0.08, hard 0.22 m/m at
//!   610 m cells, a cell's mean slope: steeper ground within it, scaled like `rock::scale`), more readily in wet climates and in a fault zone
//!   (the earthquakes shake them loose): an elliptical scar cut into the upper slope, its rock
//!   run down the steepest descent for `relief / 0.5` and laid as a hummocky tongue.
//! - A **rock avalanche** (sturzstrom) off high hard faces (relief over 350 m): the runout
//!   `relief / 0.2`, a long sheet of blocks.
//! - A **fallen flank**: one stratovolcano in three has lost a side (Mount St Helens, Socompa):
//!   a horseshoe amphitheatre at the summit opening downhill and a field of hummocks beyond it.

use crate::region::plan::Landform;
use crate::world::WorldData;
use crate::heightmap::VolcanoType;

use super::rock::{lattice, scale, unit};

/// The blended region a disturbance pass works on.
pub struct DisturbGrid<'a> {
    pub world: &'a WorldData,
    pub gw: usize,
    pub gh: usize,
    pub s: usize,
    pub gx0: i64,
    pub gy0: i64,
    pub cell_m: f32,
    pub seed: u64,
}

impl<'a> DisturbGrid<'a> {
    fn to_world(&self, i: usize, j: usize) -> (f32, f32) {
        let s = self.s as f32;
        (self.gx0 as f32 + (i as f32 + 0.5) / s - 0.5, self.gy0 as f32 + (j as f32 + 0.5) / s - 0.5)
    }
}

/// A slope that failed, for the site planner (a `Landslide` or `CollapsedFlank` place).
#[derive(Clone, Debug)]
pub struct Slide {
    /// The scar's centre and the debris' far end, absolute region cells.
    pub scar: (f64, f64),
    pub toe: (f64, f64),
    pub kind: SlideKind,
    /// The drop from the scar's head to the toe (m).
    pub relief_m: f32,
    /// Cells of debris.
    pub debris_cells: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideKind { Landslide, RockAvalanche, FallenFlank }

/// Candidate spacing (m): one slope in this many metres may fail.
const LATTICE_M: f32 = 2400.0;
/// Runout ratios (fall over run).
const SLIDE_HL: f32 = 0.5;
const AVALANCHE_HL: f32 = 0.2;
/// A fallen flank's amphitheatre radius (m), and how far its debris runs (m).
const FLANK_R_M: f32 = 2200.0;
const FLANK_RUN_M: f32 = 18_000.0;

fn sample(map: &crate::tilemap::Tilemap<f32>, u: f32, v: f32) -> f32 { crate::upheaval::sample_world(map, u, v) }

/// Cut the slides into `h` (row-major `gw` x `gh`); returns each cell's label (`None` where no
/// slide reached) and the slides.
pub fn disturb(h: &mut [f32], g: &DisturbGrid) -> (Vec<Landform>, Vec<Slide>) {
    let (gw, gh) = (g.gw, g.gh);
    let n = gw * gh;
    let mut tags = vec![Landform::None; n];
    let mut slides: Vec<Slide> = Vec::new();
    if g.world.upheaval().is_none() { return (tags, slides); }
    let h0: Vec<f32> = h.to_vec();
    let k = scale(g.cell_m);
    let at = |i: i64, j: i64| h0[(j.clamp(0, gh as i64 - 1) as usize) * gw + i.clamp(0, gw as i64 - 1) as usize];
    let slope = |i: i64, j: i64| -> (f32, f32, f32) {
        let gx = (at(i + 1, j) - at(i - 1, j)) / (2.0 * g.cell_m);
        let gy = (at(i, j + 1) - at(i, j - 1)) / (2.0 * g.cell_m);
        ((gx * gx + gy * gy).sqrt(), -gx, -gy)
    };
    let faults = g.world.faults();
    let (ax0, ay0) = (g.gx0 * g.s as i64, g.gy0 * g.s as i64);
    let spacing = ((LATTICE_M / g.cell_m).round() as i64).max(3);
    let margin = 6i64;
    // Decide every slide from the untouched heights first.
    struct Plan { i: i64, j: i64, dir: (f32, f32), a: f32, b: f32, depth: f32, run: f32, kind: SlideKind, relief: f32 }
    let mut plans: Vec<Plan> = Vec::new();
    lattice(ax0, ay0, gw, gh, spacing, g.seed ^ 0x5115E, |cx, cy, accept, ru, strength| {
        let (i, j) = (cx - ax0, cy - ay0);
        if i < margin || j < margin || i >= gw as i64 - margin || j >= gh as i64 - margin { return; }
        let e = at(i, j);
        if e <= 2.0 { return; }
        // Ice holds its ground (the glaciers' own falls are crevasses and seracs).
        if sample(&g.world.temperature, g.to_world(i as usize, j as usize).0, g.to_world(i as usize, j as usize).1) < -8.0 { return; }
        let (s, dx, dy) = slope(i, j);
        let (u, v) = g.to_world(i as usize, j as usize);
        let hard = g.world.hardness_map.as_ref().map_or(0.5, |m| sample(m, u, v));
        let moist = sample(&g.world.moisture, u, v);
        let seis = faults.at(u, v).intensity;
        let crit = (0.08 + 0.14 * hard) * k;
        let factor = s / crit * (0.7 + 0.6 * moist) * (1.0 + 1.2 * seis);
        let p = ((factor - 1.0) * 0.6).clamp(0.0, 0.75);
        if accept >= p { return; }
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for dj in -4..=4 { for di in -4..=4 { let z = at(i + di, j + dj); lo = lo.min(z); hi = hi.max(z); } }
        let relief = hi - lo;
        if relief < 40.0 { return; }
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let dir = (dx / len, dy / len);
        let rock = hard > 0.6 && relief > 350.0 && s > 1.3 * crit && strength > 0.55;
        let a = (1.0 + 2.0 * ru) * (610.0 / g.cell_m).max(0.35).min(3.0);
        let (kind, hl) = if rock { (SlideKind::RockAvalanche, AVALANCHE_HL) } else { (SlideKind::Landslide, SLIDE_HL) };
        plans.push(Plan { i, j, dir, a: a.max(1.0), b: (0.65 * a).max(0.8), depth: (0.25 * relief).min(15.0 + 60.0 * strength), run: (relief / hl) / g.cell_m, kind, relief });
    });
    // Volcanoes that lost a flank.
    for vol in &g.world.volcanoes {
        if vol.volcano_type != VolcanoType::Stratovolcano { continue; }
        let key = (vol.y * g.world.width + vol.x) as i64;
        if !crate::upheaval::flank_fell(g.world.width, vol.x, vol.y, g.seed) { continue; }
        let ww = g.world.width as i64;
        for copy in -1..=1i64 {
            let ci = (vol.x as i64 + copy * ww) * g.s as i64 + g.s as i64 / 2 - ax0;
            let cj = vol.y as i64 * g.s as i64 + g.s as i64 / 2 - ay0;
            if ci < margin || cj < margin || ci >= gw as i64 - margin || cj >= gh as i64 - margin { continue; }
            if at(ci, cj) <= 50.0 { continue; }
            // Open toward the steepest side, turned by a hash.
            let (_, dx, dy) = slope(ci, cj);
            let ang = dy.atan2(dx) + (unit(key, 1, g.seed ^ 0xF1A5) - 0.5) * 1.2;
            let a = (FLANK_R_M / g.cell_m).max(1.5);
            let relief = at(ci, cj) - at(ci + (ang.cos() * 4.0 * a) as i64, cj + (ang.sin() * 4.0 * a) as i64);
            plans.push(Plan { i: ci, j: cj, dir: (ang.cos(), ang.sin()), a, b: a * 0.9, depth: (0.3 * relief.max(200.0)).min(500.0), run: FLANK_RUN_M / g.cell_m, kind: SlideKind::FallenFlank, relief: relief.max(200.0) });
        }
    }
    // `PLANET_DEBUG_SLIDES=1` prints how many slopes failed in a window.
    if std::env::var_os("PLANET_DEBUG_SLIDES").is_some() { eprintln!("slides planned: {}", plans.len()); }
    for p in &plans {
        // The scar: a bowl cut into the slope, its head upslope of the lattice point.
        let (ci, cj) = (p.i as f32 - p.dir.0 * p.a * 0.3, p.j as f32 - p.dir.1 * p.a * 0.3);
        let r = (p.a.max(p.b) + 1.0).ceil() as i64;
        let mut volume = 0.0f32;
        for dj in -r..=r {
            for di in -r..=r {
                let (x, y) = (ci as i64 + di, cj as i64 + dj);
                if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { continue; }
                let (fx, fy) = (x as f32 - ci, y as f32 - cj);
                let (al, ac) = (fx * p.dir.0 + fy * p.dir.1, -fx * p.dir.1 + fy * p.dir.0);
                // A horseshoe: open on the downslope side.
                let rho = ((al / p.a).powi(2) + (ac / p.b).powi(2)).sqrt();
                if rho >= 1.0 { continue; }
                let c = y as usize * gw + x as usize;
                if h0[c] <= 0.0 { continue; }
                let cut = p.depth * (1.0 - rho * rho) * if al > 0.3 * p.a { 0.6 } else { 1.0 };
                h[c] -= cut;
                h[c] = h[c].max(0.5);
                volume += cut;
                tags[c] = if p.kind == SlideKind::FallenFlank { Landform::CollapseScar } else { Landform::LandslideScar };
            }
        }
        // The debris: down the steepest descent from the scar's foot for the runout, laid over
        // the last part of the path, widening.
        let (mut x, mut y) = ((ci + p.dir.0 * p.a) as i64, (cj + p.dir.1 * p.a) as i64);
        let mut path: Vec<(i64, i64)> = Vec::new();
        let steps = p.run.ceil().max(2.0) as usize;
        for _ in 0..steps {
            if x < 1 || y < 1 || x >= gw as i64 - 1 || y >= gh as i64 - 1 { break; }
            if h0[y as usize * gw + x as usize] <= 0.0 { break; }
            path.push((x, y));
            let here = at(x, y);
            let mut best = (x + p.dir.0.round() as i64, y + p.dir.1.round() as i64, f32::MAX);
            for dj in -1..=1 { for di in -1..=1 {
                if di == 0 && dj == 0 { continue; }
                let z = at(x + di, y + dj);
                // Debris keeps its momentum: it prefers the way it was going.
                let bias = -(di as f32 * p.dir.0 + dj as f32 * p.dir.1) * 2.0;
                if z + bias < best.2 && z <= here + 5.0 { best = (x + di, y + dj, z + bias); }
            } }
            x = best.0;
            y = best.1;
        }
        if path.is_empty() { continue; }
        let start = path.len() * 2 / 5;
        let tongue = &path[start..];
        let wide = if p.kind == SlideKind::Landslide { 1.0 } else { 1.8 };
        let mut cells: Vec<(usize, f32)> = Vec::new();
        for (t, &(px, py)) in tongue.iter().enumerate() {
            let w = wide * (1.0 + 2.5 * t as f32 / tongue.len().max(1) as f32) * p.b.max(1.0) * 0.6;
            let wi = w.ceil() as i64;
            for dj in -wi..=wi { for di in -wi..=wi {
                let (qx, qy) = (px + di, py + dj);
                if qx < 0 || qy < 0 || qx >= gw as i64 || qy >= gh as i64 { continue; }
                let d = ((di * di + dj * dj) as f32).sqrt() / w.max(0.5);
                if d >= 1.0 { continue; }
                let c = qy as usize * gw + qx as usize;
                if h0[c] <= 0.0 { continue; }
                if let Some(e) = cells.iter_mut().find(|e| e.0 == c) { e.1 = e.1.max(1.0 - d); } else { cells.push((c, 1.0 - d)); }
            } }
        }
        let wsum: f32 = cells.iter().map(|e| e.1).sum::<f32>().max(1e-3);
        let (tx, ty) = *tongue.last().unwrap();
        for &(c, wgt) in &cells {
            let (ax, ay) = ((c % gw) as i64 + ax0, (c / gw) as i64 + ay0);
            let hummock = 0.55 + 0.9 * unit(ax, ay, g.seed ^ 0xD3B);
            h[c] += volume / wsum * wgt * hummock;
            if tags[c] == Landform::None {
                tags[c] = match p.kind { SlideKind::RockAvalanche => Landform::RockAvalanche, _ => Landform::LandslideDebris };
            }
        }
        slides.push(Slide {
            scar: ((ci as i64 + ax0) as f64 + 0.5, (cj as i64 + ay0) as f64 + 0.5),
            toe: ((tx + ax0) as f64 + 0.5, (ty + ay0) as f64 + 0.5),
            kind: p.kind,
            relief_m: p.relief,
            debris_cells: cells.len(),
        });
    }
    (tags, slides)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upheaval::Upheaval;

    #[test]
    fn steep_slopes_fail_the_same_way_in_two_windows() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        world.set_upheaval(Some(Upheaval::earth()));
        let s = 32usize;
        // A steep valley side in absolute cells: 1,500 m falling to 200 m east-west, repeating.
        let land = |ax: i64, ay: i64| { let x = (ax.rem_euclid(48)) as f32 / 48.0; 200.0 + 1300.0 * (1.0 - x).powi(2) + 3.0 * unit(ax, ay, 5) };
        let make = |gx0: i64| {
            let g = DisturbGrid { world: &world, gw: 160, gh: 96, s, gx0, gy0: 10, cell_m: 610.0, seed: 4 };
            let mut h: Vec<f32> = (0..160 * 96).map(|c| land((c % 160) as i64 + gx0 * s as i64, (c / 160) as i64 + 320)).collect();
            let (t, sl) = disturb(&mut h, &g);
            (t, sl, h)
        };
        let (ta, sa, ha) = make(10);
        let (tb, _, hb) = make(11);
        assert!(!sa.is_empty(), "a 1,300 m valley side holds no slide");
        assert!(ta.iter().any(|&t| t == Landform::LandslideScar));
        assert!(ta.iter().any(|&t| matches!(t, Landform::LandslideDebris | Landform::RockAvalanche)));
        // Window b starts 32 cells east: where both see a cell far from their edges, it agrees.
        let mut compared = 0;
        for j in 30..66 {
            for i in 32 + 40..160 - 40 {
                let (ka, kb) = (j * 160 + i, j * 160 + i - 32);
                assert_eq!(ta[ka], tb[kb], "label differs at {i},{j}");
                assert_eq!(ha[ka].to_bits(), hb[kb].to_bits(), "height differs at {i},{j}");
                compared += 1;
            }
        }
        assert!(compared > 0);
    }
}
