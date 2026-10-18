//! Impact craters at region scale (ROADMAP Stage 7; `crate::upheaval`): every crater of the
//! world's record from two cells across up to the size the world map shapes itself
//! (`upheaval::WORLD_SCALE_TILES`) is cut into the chunk before the rivers wear it (they breach
//! the rims and the bowls fill to lakes), by the record's own profile: a bowl under the transition
//! diameter, a flat floor, terraced walls and a central peak above it, a ring of peaks in the
//! largest; a rim and an ejecta blanket round it. The reference level is the world's height at
//! the crater's centre, so every chunk cuts the same bowl. Smaller craters are the embark's
//! (`upheaval::small_craters`, carved by `local::sites`).

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::upheaval::{self, Crater, CraterKind};
use crate::world::WorldData;

use super::{ChunkGrid, RegionFields};

/// A crater is shaped at region scale from this many cells across.
const MIN_CELLS: f32 = 2.0;

/// The craters of the record that the region shapes and that can reach world tile coordinates
/// u in [u0, u1], v in [v0, v1], with the world's height at each centre.
pub(super) fn craters_near(world: &WorldData, cell_m: f32, u0: f32, u1: f32, v0: f32, v1: f32) -> Vec<(Crater, f32, f32)> {
    let Some(rec) = world.impacts() else { return Vec::new() };
    let km = upheaval::tile_km(world.width);
    let min_km = MIN_CELLS * cell_m / 1000.0;
    let max_km = upheaval::WORLD_SCALE_TILES * km;
    let ww = world.width as f32;
    let mut out: Vec<(Crater, f32, f32)> = Vec::new();
    let (cu, cv) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
    let tiles = (((u1 - u0).max(v1 - v0)) * 0.5).ceil() as i64 + 1;
    let tx = (cu.round() as i64).rem_euclid(world.width as i64) as usize;
    let ty = (cv.round() as i64).clamp(0, world.height as i64 - 1) as usize;
    for c in rec.around(tx, ty, tiles) {
        if c.d_km < min_km || c.d_km >= max_km { continue; }
        let href = upheaval::sample_world(&world.heightmap, c.u, c.v);
        if href <= 20.0 { continue; }
        // The copy of the centre nearest the window (x wraps).
        let u = c.u + ((cu - c.u) / ww).round() * ww;
        let reach = c.reach_km() / km + 0.5;
        if u + reach < u0 || u - reach > u1 || c.v + reach < v0 || c.v - reach > v1 { continue; }
        out.push((c.clone(), u, href));
    }
    out
}

/// Distance (km) from tile coordinates (u, v) to a crater centred at (cu, c.v), the east-west
/// step shrinking with the latitude, and the angle from the centre (east 0, south pi/2).
fn dist_km(world: &WorldData, cu: f32, cv: f32, u: f32, v: f32) -> (f32, f32) {
    let km = upheaval::tile_km(world.width);
    let lat = std::f32::consts::FRAC_PI_2 - ((v + cv) * 0.5 + 0.5) / world.height as f32 * std::f32::consts::PI;
    let dx = (u - cu) * lat.cos().abs().max(0.02);
    ((dx.powi(2) + (v - cv).powi(2)).sqrt() * km, (v - cv).atan2(dx))
}

pub fn shape_before(h: &mut [f32], g: &ChunkGrid) {
    let gw = g.gw;
    let (u0, v0) = g.to_world(0, 0);
    let (u1, v1) = g.to_world(gw - 1, gw - 1);
    let near = craters_near(g.world, g.cell_m, u0, u1, v0, v1);
    if near.is_empty() { return; }
    for j in 0..gw {
        for i in 0..gw {
            let (u, v) = g.to_world(i, j);
            let c = j * gw + i;
            for (cr, cu, href) in &near {
                let (r, theta) = dist_km(g.world, *cu, cr.v, u, v);
                if r > cr.reach_km() { continue; }
                let (t, w) = cr.profile_at(r, theta, true);
                // Inside the walls the crater replaces the ground (its floor at the reference
                // level less its depth, half the land's own texture kept); beyond the rim its
                // ejecta are laid on the ground.
                let detail = h[c] - upheaval::sample_world(&g.world.heightmap, u, v);
                let inside = href + 0.5 * detail;
                let ground = h[c] + (inside - h[c]) * w;
                h[c] = (ground + t).max(if h[c] > 0.0 { 0.5 } else { h[c] });
            }
        }
    }
}

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    let (u0, v0) = f.to_world(0, 0);
    let (u1, v1) = f.to_world(f.gw - 1, f.gh - 1);
    let near = craters_near(f.world, f.cell_m, u0, u1, v0, v1);
    if near.is_empty() { return; }
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) { continue; }
            let (u, v) = f.to_world(i, j);
            for (cr, cu, _) in &near {
                let (r, theta) = dist_km(f.world, *cu, cr.v, u, v);
                if r > cr.reach_km() { continue; }
                let (x, lobe) = cr.x_at(r, theta);
                let l = if x < 0.13 && cr.kind == CraterKind::Complex && cr.wear < 0.85 { Landform::CentralPeak }
                    else if (x - 0.42).abs() < 0.07 && cr.kind == CraterKind::PeakRing && cr.wear < 0.85 { Landform::CentralPeak }
                    else if x < 0.85 { Landform::ImpactCrater }
                    else if x < 1.12 { Landform::ImpactRim }
                    else if x < 1.0 + 0.9 * lobe && cr.wear < 0.6 { Landform::Ejecta }
                    else { continue };
                land[k] = l;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::zoom::{accumulate, route};

    #[test]
    fn chunks_cut_the_same_craters() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        world.set_upheaval(Some(crate::upheaval::Upheaval { impact_flux: 25.0, ..crate::upheaval::Upheaval::earth() }));
        let Some(rec) = world.impacts() else { panic!("upheaval is on") };
        // A crater on land the region shapes, and two chunks over it a tile apart.
        let km = crate::upheaval::tile_km(64);
        let s = 32usize;
        let cell_m = km * 1000.0 / s as f32;
        let Some(c) = rec.craters.iter().find(|c| c.d_km >= 2.0 * cell_m / 1000.0 && c.d_km < 1.5 * km && crate::upheaval::sample_world(&world.heightmap, c.u, c.v) > 50.0) else { panic!("no crater on land to test") };
        let (tx, ty) = rec.tile_of(c);
        let gw = 3 * s;
        let make = |gx0: i64, gy0: i64| {
            let g = ChunkGrid { world: &world, gw, s, gx0, gy0, cell_m, seed: 1 };
            let mut h: Vec<f32> = (0..gw * gw).map(|k| { let (u, v) = g.to_world(k % gw, k / gw); crate::upheaval::sample_world(&world.heightmap, u, v) }).collect();
            let before = h.clone();
            shape_before(&mut h, &g);
            (g, h, before)
        };
        let (ga, ha, ba) = make(tx as i64 - 1, ty as i64 - 1);
        let (gb, hb, _) = make(tx as i64, ty as i64 - 1);
        let mut compared = 0;
        let mut moved = 0;
        for j in 0..gw {
            for i in s..gw {
                let (ax, ay) = ga.abs_cell(i, j);
                assert_eq!((ax, ay), gb.abs_cell(i - s, j));
                assert_eq!(ha[j * gw + i].to_bits(), hb[j * gw + i - s].to_bits(), "differs at {ax},{ay}");
                compared += 1;
                if (ha[j * gw + i] - ba[j * gw + i]).abs() > 1.0 { moved += 1; }
            }
        }
        assert!(compared > 0 && moved > 0, "the crater moved {moved} cells");
        let _ = (route, accumulate);
    }
}
