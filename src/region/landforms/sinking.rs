//! Ground that gives way (ROADMAP Stage 7; only with upheaval): beyond the limestone's dolines
//! (`karst`),
//!
//! - **Salt sinkholes**: where a dry lake's salt and gypsum lie under the crust, water from the
//!   margins dissolves them and the ground falls in, along the playa's edge (within
//!   `SALT_EDGE_CELLS` of it), in clusters.
//! - **Thaw lakes** (thermokarst): ice-rich frozen ground on cold wet flats thaws under a pond,
//!   the pond deepens the thaw, and the ground sinks into shallow lakes a kilometre or two long,
//!   drawn out across the wind. On a lattice `THAW_LATTICE_M` apart, half the points, where the
//!   mean is -12 to -2 C, the moisture over 0.3, the ground flat and under 600 m.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::rock::{sample_tile, scale, unit};
use super::{ChunkGrid, Drainage, RegionFields};

const SALT_EDGE_CELLS: i64 = 3;
const THAW_LATTICE_M: f32 = 4800.0;
const THAW_RATE: f32 = 0.5;
const THAW_R_M: (f32, f32) = (700.0, 1800.0);
const THAW_DEPTH_M: (f32, f32) = (3.0, 9.0);

/// The thaw lake that covers absolute cell (ax, ay): centre, half-axes (cells, along and across
/// the wind), depth.
fn thaw_at(f_world: &crate::world::WorldData, s: usize, cell_m: f32, ax: i64, ay: i64) -> Option<(f32, f32, f32, f32, f32, (f32, f32))> {
    let lattice = ((THAW_LATTICE_M / cell_m).round() as i64).max(4);
    let (bx, by) = (ax.div_euclid(lattice), ay.div_euclid(lattice));
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (px, py) = (bx + dx, by + dy);
            if unit(px, py, 0x7A40) >= THAW_RATE { continue; }
            let cx = (px * lattice) as f32 + unit(px, py, 0x7A41) * lattice as f32;
            let cy = (py * lattice) as f32 + unit(px, py, 0x7A42) * lattice as f32;
            let (u, v) = ((cx + 0.5) / s as f32 - 0.5, (cy + 0.5) / s as f32 - 0.5);
            let t = sample_tile(&f_world.temperature, u, v);
            if !(-12.0..-2.0).contains(&t) { continue; }
            if sample_tile(&f_world.moisture, u, v) < 0.3 { continue; }
            let e = sample_tile(&f_world.heightmap, u, v);
            if !(2.0..600.0).contains(&e) { continue; }
            let lat = 90.0 - (v + 0.5) / f_world.height as f32 * 180.0;
            let wind = super::volcanic::wind_dir(lat);
            let a = (THAW_R_M.0 + (THAW_R_M.1 - THAW_R_M.0) * unit(px, py, 0x7A43)) / cell_m;
            let b = a * 0.55;
            let (fx, fy) = (ax as f32 + 0.5 - cx, ay as f32 + 0.5 - cy);
            // Drawn out across the wind.
            let (across, along) = (fx * -wind.1 + fy * wind.0, fx * wind.0 + fy * wind.1);
            let rho = ((across / a.max(0.7)).powi(2) + (along / b.max(0.6)).powi(2)).sqrt();
            if rho > 1.0 { continue; }
            let depth = THAW_DEPTH_M.0 + (THAW_DEPTH_M.1 - THAW_DEPTH_M.0) * unit(px, py, 0x7A44);
            return Some((cx, cy, a, b, depth * (1.0 - rho * rho).max(0.0) + 0.0 * rho, wind));
        }
    }
    None
}

pub fn shape_after(h: &mut [f32], g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {
    if g.world.upheaval().is_none() { return; }
    let gw = g.gw;
    // Only chunks with cold ground at all.
    let (u0, v0) = g.to_world(gw / 2, gw / 2);
    let t0 = sample_tile(&g.world.temperature, u0, v0);
    if t0 > 4.0 { return; }
    let slopes = super::rock::chunk_slopes(h, g);
    let k = scale(g.cell_m);
    for j in 0..gw {
        for i in 0..gw {
            let c = j * gw + i;
            if h[c] <= 1.0 || slopes[c] > 0.02 * k { continue; }
            let (ax, ay) = g.abs_cell(i, j);
            if let Some((_, _, _, _, sink, _)) = thaw_at(g.world, g.s, g.cell_m, ax, ay) {
                h[c] = (h[c] - sink).max(0.5);
            }
        }
    }
}

pub fn classify(f: &crate::region::landforms::RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    if f.world.upheaval().is_none() { return; }
    let (gw, gh) = (f.gw, f.gh);
    let src = land.to_vec();
    for j in 0..gh {
        for i in 0..gw {
            let k = f.idx(i, j);
            if f.is_sea(k) { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            if src[k] == Landform::Playa {
                // Near the playa's edge, in clusters.
                let mut edge = false;
                'n: for dy in -SALT_EDGE_CELLS..=SALT_EDGE_CELLS {
                    for dx in -SALT_EDGE_CELLS..=SALT_EDGE_CELLS {
                        let (x, y) = (i as i64 + dx, j as i64 + dy);
                        if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { continue; }
                        if src[f.idx(x as usize, y as usize)] != Landform::Playa { edge = true; break 'n; }
                    }
                }
                if edge && unit(ax.div_euclid(3), ay.div_euclid(3), 0x5A17) < 0.35 { land[k] = Landform::SaltSink; }
                continue;
            }
            if matches!(src[k], Landform::None | Landform::PatternedGround | Landform::Bog | Landform::Fen | Landform::Marsh) {
                if thaw_at(f.world, f.s, f.cell_m, ax, ay).is_some() && f.slope(i, j) < 0.03 * scale(f.cell_m) { land[k] = Landform::ThawLake; }
            }
        }
    }
    let _ = RegionFields::idx;
}
