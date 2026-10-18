//! More fire (ROADMAP Stage 7; only with upheaval, `crate::upheaval`):
//!
//! - **Maars**: where young volcanism meets wet ground, rising magma flashed the groundwater
//!   to steam and blew a round crater below the land's level with a low ring of ash round it;
//!   rain fills it to a deep round lake. On a hashed lattice `MAAR_LATTICE_M` apart, one point in
//!   four, on ground under 1,500 m with moisture over 0.35.
//! - **Traps**: the flood-basalt provinces (`upheaval::provinces`): flow upon flow of basalt
//!   `flow_m` thick, which the rivers cut back into steps (the word is the Swedish for stairs);
//!   the surface rock is basalt.
//! - **Fissure vents**: along a rift's normal faults near young volcanism or lava, rows of
//!   spatter cones on the cracks.
//! - **Lahars**: the valleys below a stratovolcano that carries snow or a crater lake, where
//!   mudflows of ash and meltwater have run and will run again: grey flat deposits on the floors.

use crate::erosion::materials::RockType;
use crate::heightmap::VolcanoType;
use crate::region::plan::Landform;
use crate::upheaval::{province_share, FaultKind};
use crate::world::WorldData;

use super::rock::{sample_tile, scale, smoothstep, unit};
use super::volcanic::{lava_at, vents_near, young_at, Vent};
use super::{ChunkGrid, Drainage, RegionFields};

const MAAR_LATTICE_M: f32 = 5200.0;
const MAAR_RATE: f32 = 0.25;
const MAAR_R_M: (f32, f32) = (450.0, 1100.0);
const MAAR_DEPTH_M: (f32, f32) = (40.0, 120.0);
const MAAR_RING_M: f32 = 18.0;
/// A province's steps: how much of each flow's thickness the step keeps.
const TRAP_STEP: f32 = 0.7;
/// How far below a snowy cone its lahars run (tiles).
const LAHAR_TILES: f32 = 2.5;

/// The maar that covers absolute cell (ax, ay): centre (absolute cells), radius (cells), depth.
fn maar_at(world: &WorldData, vents: &[Vent], s: usize, cell_m: f32, ax: i64, ay: i64) -> Option<(f32, f32, f32, f32)> {
    let lattice = ((MAAR_LATTICE_M / cell_m).round() as i64).max(4);
    let (bx, by) = (ax.div_euclid(lattice), ay.div_euclid(lattice));
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (px, py) = (bx + dx, by + dy);
            if unit(px, py, 0x3AA0) >= MAAR_RATE { continue; }
            let cx = (px * lattice) as f32 + unit(px, py, 0x3AA1) * lattice as f32;
            let cy = (py * lattice) as f32 + unit(px, py, 0x3AA2) * lattice as f32;
            let (u, v) = ((cx + 0.5) / s as f32 - 0.5, (cy + 0.5) / s as f32 - 0.5);
            if !young_at(world, vents, u, v) { continue; }
            if sample_tile(&world.moisture, u, v) < 0.35 { continue; }
            let e = sample_tile(&world.heightmap, u, v);
            if !(5.0..1500.0).contains(&e) { continue; }
            let r = (MAAR_R_M.0 + (MAAR_R_M.1 - MAAR_R_M.0) * unit(px, py, 0x3AA3)) / cell_m;
            let d = ((ax as f32 + 0.5 - cx).powi(2) + (ay as f32 + 0.5 - cy).powi(2)).sqrt();
            if d > 1.6 * r.max(0.8) { continue; }
            let depth = MAAR_DEPTH_M.0 + (MAAR_DEPTH_M.1 - MAAR_DEPTH_M.0) * unit(px, py, 0x3AA4);
            return Some((cx, cy, r.max(0.8), depth));
        }
    }
    None
}

/// The world's provinces' share at tile coordinates (u, v), the largest.
pub(super) fn traps_at(world: &WorldData, u: f32, v: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for p in world.provinces() {
        let sh = province_share(world.width, world.height, p, u, v);
        if sh > 0.0 && best.map_or(true, |b| sh > b.0) { best = Some((sh, p.flow_m)); }
    }
    best
}

pub fn shape_after(h: &mut [f32], g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {
    if g.world.upheaval().is_none() { return; }
    let gw = g.gw;
    let (u0, v0) = g.to_world(0, 0);
    let (u1, v1) = g.to_world(gw - 1, gw - 1);
    let vents = vents_near(g.world, u0, u1, v0, v1);
    let provinces = !g.world.provinces().is_empty();
    for j in 0..gw {
        for i in 0..gw {
            let c = j * gw + i;
            if h[c] <= 0.0 { continue; }
            let (ax, ay) = g.abs_cell(i, j);
            if !vents.is_empty() {
                if let Some((cx, cy, r, depth)) = maar_at(g.world, &vents, g.s, g.cell_m, ax, ay) {
                    let d = ((ax as f32 + 0.5 - cx).powi(2) + (ay as f32 + 0.5 - cy).powi(2)).sqrt() / r;
                    if d < 1.0 { h[c] = (h[c] - depth * (1.0 - d.powi(4))).max(0.5); }
                    else if d < 1.6 { h[c] += MAAR_RING_M * (1.0 - (d - 1.0) / 0.6); }
                }
            }
            if provinces {
                let (u, v) = g.to_world(i, j);
                if let Some((sh, flow)) = traps_at(g.world, u, v) {
                    if sh > 0.3 {
                        let q = h[c] / flow;
                        let fl = q.floor();
                        let stepped = (fl + smoothstep(0.7, 1.0, q - fl)) * flow;
                        let w = TRAP_STEP * smoothstep(0.3, 0.6, sh);
                        h[c] = (h[c] * (1.0 - w) + stepped * w).max(0.5);
                    }
                }
            }
        }
    }
}

/// The provinces' basalt at the surface.
pub fn rock_over(f: &RegionFields, rock: &mut [RockType]) {
    if f.world.upheaval().is_none() || f.world.provinces().is_empty() { return; }
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) { continue; }
            let (u, v) = f.to_world(i, j);
            if traps_at(f.world, u, v).map_or(false, |t| t.0 > 0.35) { rock[k] = RockType::Basalt; }
        }
    }
}

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    if f.world.upheaval().is_none() { return; }
    let (u0, v0) = f.to_world(0, 0);
    let (u1, v1) = f.to_world(f.gw - 1, f.gh - 1);
    let vents = vents_near(f.world, u0, u1, v0, v1);
    let provinces = !f.world.provinces().is_empty();
    let faults = super::tectonic::FaultField::new(f.world, f.s, f.cell_m, f.seed);
    let k_scale = scale(f.cell_m);
    // Cones that carry snow or a lake: their valleys take the lahars.
    let lahar_vents: Vec<&Vent> = vents.iter().filter(|v| {
        v.kind == VolcanoType::Stratovolcano && sample_tile(&f.world.temperature, v.u, v.v) < 4.0
    }).collect();
    let chans = if lahar_vents.is_empty() { None } else { Some(super::fluvial::channels(f)) };
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            let (u, v) = f.to_world(i, j);
            if !vents.is_empty() {
                if let Some((cx, cy, r, _)) = maar_at(f.world, &vents, f.s, f.cell_m, ax, ay) {
                    let d = ((ax as f32 + 0.5 - cx).powi(2) + (ay as f32 + 0.5 - cy).powi(2)).sqrt() / r;
                    if d < 1.5 { land[k] = Landform::Maar; continue; }
                }
            }
            if let Some(ff) = &faults {
                let hit = ff.at(ax as f64, ay as f64);
                if hit.kind == FaultKind::Normal && hit.face_m > 0.0 && (young_at(f.world, &vents, u, v) || lava_at(f.world, u, v) > 0.0) && unit(ax, ay, 0x515F) < 0.6 {
                    land[k] = Landform::FissureVents;
                    continue;
                }
            }
            if let Some(ch) = &chans {
                let near = lahar_vents.iter().any(|vt| (vt.u - u).hypot(vt.v - v) < LAHAR_TILES);
                if near && ch.hand[k] < 6.0 && ch.steps[k] < 4 && f.slope(i, j) < 0.08 * k_scale
                    && matches!(land[k], Landform::None | Landform::Floodplain | Landform::Braided | Landform::Fan | Landform::Terrace | Landform::Outwash) {
                    land[k] = Landform::Lahar;
                    continue;
                }
            }
            if provinces && land[k] == Landform::None {
                if traps_at(f.world, u, v).map_or(false, |t| t.0 > 0.4) { land[k] = Landform::Traps; }
            }
        }
    }
}
