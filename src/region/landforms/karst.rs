//! Karst on carbonate rock drained through the ground (ROADMAP Stage 2): limestone pavements on
//! plateaus, dolines (closed hollows that drain underground: cut by `shape_after`, labelled by
//! `classify`), dry valleys, sinking streams and their resurgences (a channel leaving other rock
//! onto limestone goes under at a `Sink` and comes out where the limestone ends downstream, a
//! `Resurgence`), tower karst in the wet tropics, cenotes on flat low limestone in warm country,
//! and gorges where a river has cut the limestone deep. The caves pass (`sites::caves`) builds
//! its graphs from the sinks, resurgences and dolines labelled here.
//!
//! Every pass is a pure function of world position (noise keyed through `ChunkGrid::to_world` /
//! `RegionFields::to_world`) and reads nothing beyond the region's margin. Dolines, towers and
//! cenotes are centred on hashed lattices (`rock::lattice`), so the chunk that cuts them and the
//! region that labels them agree; the shape pass tests the ground with the chunk's own drainage,
//! the labelling with the blended region's, which differ only along a chunk's border.

use crate::erosion::materials::RockType;
use crate::region::plan::{is_carbonate, Landform};

use super::rock::{lattice, max_filter, min_filter, river_threshold, sample_tile, scale, slopes, unit, Beds};
use super::{ChunkGrid, Drainage, RegionFields};

/// Water enough for dissolution.
const WET_MOIST: f32 = 0.25;
/// Dolines: lattice spacing (cells), radius (cells) and depth (m) ranges.
const DOLINE_SPACING: i64 = 8;
const DOLINE_RADIUS: (f32, f32) = (1.0, 3.0);
const DOLINE_DEPTH_M: (f32, f32) = (5.0, 30.0);
/// Share of lattice points that hold a doline on wet limestone (scaled by moisture).
const DOLINE_SHARE: f32 = 0.7;
/// A pavement stands this far above the ground within `PAVEMENT_RADIUS` cells.
const PAVEMENT_RELIEF_M: f32 = 60.0;
const PAVEMENT_RADIUS: usize = 10;
/// Tower karst: the wet tropics, lattice spacing, radius (cells), height (m).
const TOWER_TEMP_C: f32 = 20.0;
const TOWER_MOIST: f32 = 0.6;
const TOWER_SPACING: i64 = 5;
const TOWER_RADIUS: (f32, f32) = (1.0, 2.2);
const TOWER_HEIGHT_M: (f32, f32) = (30.0, 100.0);
/// Cenotes: warm flat low limestone; lattice spacing, depth (m).
const CENOTE_TEMP_C: f32 = 18.0;
const CENOTE_SPACING: i64 = 12;
const CENOTE_DEPTH_M: (f32, f32) = (10.0, 20.0);
const CENOTE_MAX_H: f32 = 200.0;
/// A gorge: the ground within 3 cells rises this far over a channel.
const GORGE_RELIEF_M: f32 = 80.0;

/// A hollow or a tower from the lattice at (cx, cy): its radius (cells), its depth or height
/// (m) and whether the lattice point holds one at all.
struct Pit { cx: i64, cy: i64, r: f32, amp: f32 }

/// The dolines of an area, from the lattice alone (the ground's fitness is checked by the caller
/// at each centre).
fn dolines(ax0: i64, ay0: i64, w: usize, h: usize, moist_at: impl Fn(i64, i64) -> f32) -> Vec<Pit> {
    let mut out = Vec::new();
    lattice(ax0, ay0, w, h, DOLINE_SPACING, 0xD0, |cx, cy, accept, ur, ud| {
        let m = moist_at(cx, cy);
        if m < WET_MOIST || accept > DOLINE_SHARE * (0.4 + 0.6 * m) { return; }
        out.push(Pit { cx, cy, r: DOLINE_RADIUS.0 + (DOLINE_RADIUS.1 - DOLINE_RADIUS.0) * ur, amp: DOLINE_DEPTH_M.0 + (DOLINE_DEPTH_M.1 - DOLINE_DEPTH_M.0) * ud });
    });
    out
}

fn towers(ax0: i64, ay0: i64, w: usize, h: usize) -> Vec<Pit> {
    let mut out = Vec::new();
    lattice(ax0, ay0, w, h, TOWER_SPACING, 0x70E, |cx, cy, accept, ur, ua| {
        if accept > 0.8 { return; }
        out.push(Pit { cx, cy, r: TOWER_RADIUS.0 + (TOWER_RADIUS.1 - TOWER_RADIUS.0) * ur, amp: TOWER_HEIGHT_M.0 + (TOWER_HEIGHT_M.1 - TOWER_HEIGHT_M.0) * ua });
    });
    out
}

fn cenotes(ax0: i64, ay0: i64, w: usize, h: usize) -> Vec<Pit> {
    let mut out = Vec::new();
    lattice(ax0, ay0, w, h, CENOTE_SPACING, 0xCE0, |cx, cy, accept, _, ud| {
        if accept > 0.5 { return; }
        out.push(Pit { cx, cy, r: 1.0, amp: CENOTE_DEPTH_M.0 + (CENOTE_DEPTH_M.1 - CENOTE_DEPTH_M.0) * ud });
    });
    out
}

/// The climate under a cell of a chunk, from the world's tiles.
fn chunk_climate(g: &ChunkGrid, i: usize, j: usize) -> (f32, f32) {
    let (u, v) = g.to_world(i, j);
    (sample_tile(&g.world.temperature, u, v), sample_tile(&g.world.moisture, u, v))
}

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

/// Cut the dolines and cenotes, raise the towers: on carbonate ground that is dry of channels
/// (the chunk's drainage from the evolution's last step), as pure functions of position.
pub fn shape_after(h: &mut [f32], g: &ChunkGrid, _d: &Drainage, acc: &[f32]) {
    let gw = g.gw;
    let k = scale(g.cell_m);
    let beds = Beds::new(g.world, g.s, g.seed);
    let (ax0, ay0) = g.abs_cell(0, 0);
    let carbonate_cell = |cx: i64, cy: i64| -> Option<usize> {
        let (i, j) = (cx - ax0, cy - ay0);
        if i < 0 || j < 0 || i >= gw as i64 || j >= gw as i64 { return None; }
        let c = j as usize * gw + i as usize;
        if h[c] <= 0.0 { return None; }
        is_carbonate(beds.surface_rock(cx as f64 + 0.5, cy as f64 + 0.5, h[c])).then_some(c)
    };
    // Is there any carbonate ground at all? (Most chunks have none: skip the slopes.)
    let any = (0..gw * gw).step_by(7).any(|c| h[c] > 0.0 && is_carbonate(beds.surface_rock((ax0 + (c % gw) as i64) as f64 + 0.5, (ay0 + (c / gw) as i64) as f64 + 0.5, h[c])));
    if !any { return; }
    let slope = super::rock::chunk_slopes(h, g);
    let moist_at = |cx: i64, cy: i64| {
        let (i, j) = ((cx - ax0).clamp(0, gw as i64 - 1) as usize, (cy - ay0).clamp(0, gw as i64 - 1) as usize);
        chunk_climate(g, i, j).1
    };
    let mut cuts: Vec<(Pit, f32)> = Vec::new(); // (pit, sign: -1 cut, +1 raise)
    for p in dolines(ax0, ay0, gw, gw, moist_at) {
        let Some(c) = carbonate_cell(p.cx, p.cy) else { continue };
        let thr = river_threshold(g.s) / (0.25 + moist_at(p.cx, p.cy));
        if acc[c] >= thr || slope[c] > 0.1 * k { continue; }
        cuts.push((p, -1.0));
    }
    for p in towers(ax0, ay0, gw, gw) {
        let Some(c) = carbonate_cell(p.cx, p.cy) else { continue };
        let (t, m) = chunk_climate(g, (p.cx - ax0) as usize, (p.cy - ay0) as usize);
        if t < TOWER_TEMP_C || m < TOWER_MOIST || slope[c] > 0.06 * k || h[c] < 5.0 { continue; }
        cuts.push((p, 1.0));
    }
    for p in cenotes(ax0, ay0, gw, gw) {
        let Some(c) = carbonate_cell(p.cx, p.cy) else { continue };
        let (t, _) = chunk_climate(g, (p.cx - ax0) as usize, (p.cy - ay0) as usize);
        if t < CENOTE_TEMP_C || slope[c] > 0.02 * k || h[c] > CENOTE_MAX_H || h[c] < 2.0 { continue; }
        cuts.push((p, -1.0));
    }
    for (p, sign) in cuts {
        let ri = p.r.ceil() as i64;
        let thr = river_threshold(g.s) / (0.25 + moist_at(p.cx, p.cy));
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (x, y) = (p.cx + dx - ax0, p.cy + dy - ay0);
                if x < 0 || y < 0 || x >= gw as i64 || y >= gw as i64 { continue; }
                let c = y as usize * gw + x as usize;
                // Never into a channel: a hollow on a stream would trap it.
                if h[c] <= 0.0 || acc[c] >= thr { continue; }
                let dd = ((dx * dx + dy * dy) as f32).sqrt() / p.r;
                if dd >= 1.0 { continue; }
                // Hollows are bowls; towers stand up steep-sided.
                let profile = if sign < 0.0 { 1.0 - dd * dd } else { 1.0 - dd.powi(4) };
                h[c] = (h[c] + sign * p.amp * profile).max(0.5);
            }
        }
    }
}

pub fn classify(f: &RegionFields, rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let n = gw * gh;
    let k = scale(f.cell_m);
    let any = rock.iter().any(|&r| is_carbonate(r));
    if !any { return; }
    let slope = slopes(f);
    let low = min_filter(f.h, gw, gh, PAVEMENT_RADIUS);
    let high3 = max_filter(f.h, gw, gh, 3);
    let thr = |c: usize| river_threshold(f.s) / (0.25 + f.moist[c]);
    let land_ok = |c: usize| !f.is_sea(c) && f.lake_depth(c) == 0.0;
    let (ax0, ay0) = f.abs_cell(0, 0);
    let cell_of = |cx: i64, cy: i64| -> Option<usize> {
        let (i, j) = (cx - ax0, cy - ay0);
        if i < 0 || j < 0 || i >= gw as i64 || j >= gh as i64 { None } else { Some(j as usize * gw + i as usize) }
    };
    // A pit's footprint never takes a channel cell: the stream runs past the hollow's rim.
    let stamp = |land: &mut [Landform], p: &Pit, what: Landform| {
        let ri = p.r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if ((dx * dx + dy * dy) as f32).sqrt() >= p.r { continue; }
                if let Some(c) = cell_of(p.cx + dx, p.cy + dy) {
                    if land_ok(c) && is_carbonate(rock[c]) && f.acc[c] < thr(c) { land[c] = what; }
                }
            }
        }
    };

    // Sinks and resurgences along the channels, then the pits and towers, then the rest.
    for c in 0..n {
        if !land_ok(c) || f.acc[c] < thr(c) { continue; }
        let r = f.recv[c];
        if r == u32::MAX { continue; }
        let r = r as usize;
        if !land_ok(r) { continue; }
        // Both marks sit on the limestone: the sink on its first cell, the resurgence on its
        // last (the spring breaks out at the contact, from rock that can hold a cave).
        match (is_carbonate(rock[c]), is_carbonate(rock[r])) {
            (false, true) if f.moist[c] >= WET_MOIST => land[r] = Landform::Sink,
            (true, false) if f.moist[c] >= WET_MOIST => land[c] = Landform::Resurgence,
            _ => {}
        }
    }
    // A cut doline holds the region's water until the crop drains it (`generate_zoom`): the
    // centre may be a pond, but one enclosed by dry ground just outside the footprint, so a
    // real lake a lattice point happens to fall in is never taken for a doline.
    let enclosed = |p: &Pit| -> bool {
        let ro = (p.r + 1.5).ceil() as i64;
        for dy in -ro..=ro {
            for dx in -ro..=ro {
                let dd = ((dx * dx + dy * dy) as f32).sqrt();
                if dd < p.r || dd > p.r + 1.5 { continue; }
                match cell_of(p.cx + dx, p.cy + dy) {
                    Some(c) if !f.is_sea(c) && f.lake_depth(c) == 0.0 => {}
                    _ => return false,
                }
            }
        }
        true
    };
    for p in dolines(ax0, ay0, gw, gh, |cx, cy| cell_of(cx, cy).map(|c| f.moist[c]).unwrap_or(0.0)) {
        let Some(c) = cell_of(p.cx, p.cy) else { continue };
        if f.is_sea(c) || f.lake_depth(c) > DOLINE_DEPTH_M.1 || !is_carbonate(rock[c]) || f.acc[c] >= thr(c) || !enclosed(&p) { continue; }
        if f.lake_depth(c) == 0.0 && slope[c] > 0.1 * k { continue; }
        // The whole bowl is the doline (its centre's rock decided it), so the crop drains all of it.
        let ri = p.r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if ((dx * dx + dy * dy) as f32).sqrt() >= p.r { continue; }
                if let Some(cc) = cell_of(p.cx + dx, p.cy + dy) {
                    if !f.is_sea(cc) && f.acc[cc] < thr(cc) { land[cc] = Landform::Doline; }
                }
            }
        }
    }
    for p in towers(ax0, ay0, gw, gh) {
        let Some(c) = cell_of(p.cx, p.cy) else { continue };
        if !land_ok(c) || !is_carbonate(rock[c]) || f.temp[c] < TOWER_TEMP_C || f.moist[c] < TOWER_MOIST || f.h[c] < 5.0 { continue; }
        stamp(land, &p, Landform::TowerKarst);
    }
    for p in cenotes(ax0, ay0, gw, gh) {
        let Some(c) = cell_of(p.cx, p.cy) else { continue };
        if !land_ok(c) || !is_carbonate(rock[c]) || f.temp[c] < CENOTE_TEMP_C || f.h[c] > CENOTE_MAX_H + CENOTE_DEPTH_M.1 { continue; }
        stamp(land, &p, Landform::Cenote);
    }
    for c in 0..n {
        if land[c] != Landform::None || !land_ok(c) || !is_carbonate(rock[c]) || f.moist[c] < WET_MOIST { continue; }
        let t = thr(c);
        if f.acc[c] >= t {
            if high3[c] - f.h[c] > GORGE_RELIEF_M { land[c] = Landform::KarstGorge; }
        } else if f.acc[c] >= 0.5 * t {
            land[c] = Landform::DryValley;
        } else if slope[c] < 0.05 * k && f.h[c] - low[c] > PAVEMENT_RELIEF_M {
            land[c] = Landform::Pavement;
        }
    }
    let _ = unit; // the lattice hashes every centre; nothing else here rolls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::zoom::{accumulate, route};

    /// A limestone plateau with a river across it: dolines stay off the channel, the channel
    /// sinks where it enters the limestone and resurges where it leaves, nothing on the sea.
    #[test]
    fn limestone_plateau_gets_its_karst() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let (gw, gh, s) = (96usize, 96usize, 32usize);
        // A plateau 200 m up with a valley running west to east down its middle, the sea at the
        // bottom rows.
        let mut h = vec![0.0f32; gw * gh];
        for j in 0..gh {
            for i in 0..gw {
                let valley = (j as f32 - 48.0).abs();
                let e = 200.0 - 2.0 * (i as f32) - 60.0 * (1.0 - (valley / 6.0).min(1.0));
                h[j * gw + i] = if j >= 90 { -20.0 } else { e.max(1.0) };
            }
        }
        let d = route(&h, gw, gh, 0.0);
        let acc = accumulate(&d, &vec![0.0; gw * gh]);
        let temp = vec![12.0f32; gw * gh];
        let moist = vec![0.6f32; gw * gh];
        let f = RegionFields { world: &world, gw, gh, s, gx0: 10, gy0: 10, margin: 0, cell_m: 610.0, seed: 1, h: &h, filled: &d.filled, recv: &d.recv, acc: &acc, temp: &temp, moist: &moist };
        // Limestone on the east half of the plateau, sandstone west.
        let rock: Vec<RockType> = (0..gw * gh).map(|c| if c % gw >= 40 && c % gw < 80 { RockType::Limestone } else { RockType::Sandstone }).collect();
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        let count = |l: Landform| land.iter().filter(|&&x| x == l).count();
        assert!(count(Landform::Doline) > 0, "dolines on wet limestone");
        for c in 0..gw * gh {
            if land[c] == Landform::Doline { assert!(acc[c] < river_threshold(s) / 0.85, "a doline on a channel at {c}"); }
            if h[c] <= 0.0 { assert_eq!(land[c], Landform::None, "a label on the sea at {c}"); }
            if land[c] != Landform::None { assert!(is_carbonate(rock[c]), "karst off the limestone at {c}"); }
        }
        assert!(count(Landform::Sink) >= 1, "the river sinks where it meets the limestone");
        assert!(count(Landform::Resurgence) >= 1, "and resurges where it leaves");
    }

    /// The same world cells shaped in two chunks that overlap get the same pits.
    #[test]
    fn chunks_cut_the_same_dolines() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let s = 32usize;
        let gw = 3 * s;
        let make = |gx0: i64, gy0: i64| {
            let g = ChunkGrid { world: &world, gw, s, gx0, gy0, cell_m: 610.0, seed: 1 };
            let mut h: Vec<f32> = (0..gw * gw).map(|c| { let (ax, ay) = g.abs_cell(c % gw, c / gw); 100.0 + unit(ax, ay, 9) * 3.0 }).collect();
            let d = route(&h, gw, gw, 0.0);
            let acc = accumulate(&d, &vec![0.0; gw * gw]);
            shape_after(&mut h, &g, &d, &acc);
            (g, h)
        };
        let (ga, ha) = make(10, 8);
        let (gb, hb) = make(11, 8);
        let mut compared = 0;
        for j in 8..gw - 8 {
            for i in s + 8..gw - 8 {
                let (ax, ay) = ga.abs_cell(i, j);
                let (bx, by) = gb.abs_cell(i - s, j);
                assert_eq!((ax, ay), (bx, by));
                assert_eq!(ha[j * gw + i].to_bits(), hb[j * gw + i - s].to_bits(), "differs at {ax},{ay}");
                compared += 1;
            }
        }
        assert!(compared > 0);
    }
}
