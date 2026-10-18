//! Volcanic ground near the world's volcanoes (`WorldData::volcanoes`: tile, kind, peak, whether
//! active; `lava_map`: the tiles its lava reached): a crater at a stratovolcano's summit and a
//! caldera at a shield's or a caldera's, cinder cones on the flanks, lava fields with pressure
//! ridges and tumuli, columnar basalt on old basalt cliffs, ash plains downwind of active
//! cones, and fumaroles and thermal ground on faults over young volcanism: the caused spots for
//! the hot springs, geysers and mud pots the site planner places (ROADMAP Stage 2).
//!
//! "Young" volcanism: an active volcano (`is_active`) within `YOUNG_TILES` world tiles, or lava
//! (any `LavaState` but `None`) on the cell's tile. Every shape is a pure function of world
//! position: the vents are fixed tiles, the cones sit on a hashed lattice in absolute cells.

use noise::{NoiseFn, Perlin};

use crate::erosion::materials::RockType;
use crate::heightmap::{LavaState, VolcanoType};
use crate::region::plan::Landform;
use crate::world::WorldData;

use super::{ChunkGrid, Drainage, RegionFields};

/// World tiles from an active vent within which volcanism counts as young.
const YOUNG_TILES: f32 = 2.0;
/// Reach of an edifice in world tiles: a shield is broad, a cone steep.
fn reach_tiles(kind: VolcanoType) -> f32 {
    match kind { VolcanoType::Shield => 4.0, VolcanoType::Stratovolcano => 2.5, VolcanoType::Caldera => 3.0 }
}
/// Summit crater: radius (m) and depth (m) by kind.
fn crater(kind: VolcanoType, tile_m: f32) -> (f32, f32) {
    match kind {
        VolcanoType::Stratovolcano => (600.0, 80.0),
        VolcanoType::Shield => (2500.0, 60.0),
        VolcanoType::Caldera => ((0.3 * tile_m).min(12_000.0), 150.0),
    }
}
/// Cinder cones: lattice spacing (m), radius (m) and height (m) ranges, and the share of
/// lattice points that carry one on an active and on an old volcano's flanks.
const CONE_LATTICE_M: f32 = 2500.0;
const CONE_RADIUS_M: (f32, f32) = (350.0, 900.0);
const CONE_HEIGHT_M: (f32, f32) = (25.0, 80.0);
const CONE_FLANK_TILES: (f32, f32) = (0.3, 1.8);
const CONE_RATE: (f32, f32) = (0.35, 0.18);
/// Lava fields: ridged relief (m) over the lava's share of the ground.
const LAVA_RELIEF_M: f32 = 12.0;
/// Stress (`stress_map`) from which the ground is faulted enough for thermal springs.
const FAULT_STRESS: f32 = 0.15;
/// Thermal ground lies within this many tiles of young volcanism, on ground this gentle.
const THERMAL_TILES: f32 = 3.0;
const THERMAL_SLOPE: f32 = 0.08;
/// Fumaroles lie within this many tiles of an active vent, on ground this steep.
const FUMAROLE_TILES: f32 = 0.6;

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn unit(a: i64, b: i64, salt: u64) -> f32 {
    (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0
}

fn fbm(noise: &Perlin, x: f64, y: f64, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([x * freq, y * freq]);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm) as f32
}

fn ridged(noise: &Perlin, x: f64, y: f64, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..octaves {
        sum += amp * (1.0 - noise.get([x * freq, y * freq]).abs());
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm) as f32
}

/// A vent as the passes see it: its tile centre in unwrapped world coordinates near the grid.
#[derive(Clone, Copy)]
pub(super) struct Vent {
    pub u: f32,
    pub v: f32,
    pub kind: VolcanoType,
    pub active: bool,
}

/// The vents whose edifice can reach world tile coordinates u in [u0, u1], v in [v0, v1]
/// (x wrapped: a vent appears at every unwrapped copy that is near).
pub(super) fn vents_near(world: &WorldData, u0: f32, u1: f32, v0: f32, v1: f32) -> Vec<Vent> {
    let ww = world.width as f32;
    let mut out = Vec::new();
    for vol in &world.volcanoes {
        let reach = reach_tiles(vol.volcano_type).max(THERMAL_TILES);
        if (vol.y as f32) < v0 - reach || (vol.y as f32) > v1 + reach { continue; }
        for copy in -1..=1 {
            let u = vol.x as f32 + copy as f32 * ww;
            if u < u0 - reach || u > u1 + reach { continue; }
            out.push(Vent { u, v: vol.y as f32, kind: vol.volcano_type, active: vol.is_active });
        }
    }
    out
}

/// The share of lava among the four tiles round (u, v) (bilinear), 0..1.
pub(super) fn lava_at(world: &WorldData, u: f32, v: f32) -> f32 {
    let Some(lava) = world.lava_map.as_ref() else { return 0.0 };
    let (ww, wh) = (world.width as i64, world.height as i64);
    let (fu, fv) = (u.floor(), v.floor());
    let (tu, tv) = (u - fu, v - fv);
    let at = |dx: i64, dy: i64| -> f32 {
        let (x, y) = ((fu as i64 + dx).rem_euclid(ww) as usize, (fv as i64 + dy).clamp(0, wh - 1) as usize);
        (*lava.get(x, y) != LavaState::None) as u8 as f32
    };
    at(0, 0) * (1.0 - tu) * (1.0 - tv) + at(1, 0) * tu * (1.0 - tv) + at(0, 1) * (1.0 - tu) * tv + at(1, 1) * tu * tv
}

/// Whether volcanism is young at (u, v): an active vent within reach, or lava on the tile.
pub(super) fn young_at(world: &WorldData, vents: &[Vent], u: f32, v: f32) -> bool {
    if vents.iter().any(|x| x.active && (x.u - u).hypot(x.v - v) <= YOUNG_TILES) { return true; }
    lava_at(world, u, v) > 0.0
}

/// The cinder cone that covers absolute cell (ax, ay), if any: its centre (absolute cells),
/// radius (cells) and height (m). Cones sit on a hashed lattice of `CONE_LATTICE_M` and only on
/// a vent's flanks.
fn cone_at(world: &WorldData, vents: &[Vent], s: usize, cell_m: f32, ax: i64, ay: i64, u: f32, v: f32) -> Option<(f32, f32, f32, f32)> {
    let lattice = ((CONE_LATTICE_M / cell_m).round() as i64).max(3);
    let (bx, by) = (ax.div_euclid(lattice), ay.div_euclid(lattice));
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (bx + dx, by + dy);
            let px = cx as f32 * lattice as f32 + lattice as f32 * unit(cx, cy, 0xf01);
            let py = cy as f32 * lattice as f32 + lattice as f32 * unit(cx, cy, 0xf02);
            // The cone's own place on the edifice decides whether it exists.
            let (pu, pv) = (u + (px - ax as f32) / s as f32, v + (py - ay as f32) / s as f32);
            let Some(vent) = vents.iter().min_by(|a, b| (a.u - pu).hypot(a.v - pv).partial_cmp(&(b.u - pu).hypot(b.v - pv)).unwrap()) else { continue };
            let d = (vent.u - pu).hypot(vent.v - pv);
            if d < CONE_FLANK_TILES.0 || d > CONE_FLANK_TILES.1 * reach_tiles(vent.kind) / 2.5 { continue; }
            let rate = if vent.active { CONE_RATE.0 } else { CONE_RATE.1 };
            if unit(cx, cy, 0xf03) >= rate { continue; }
            let r = (CONE_RADIUS_M.0 + (CONE_RADIUS_M.1 - CONE_RADIUS_M.0) * unit(cx, cy, 0xf04)) / cell_m;
            let r = r.max(1.2);
            let dist = (px - ax as f32).hypot(py - ay as f32);
            if dist <= r {
                let hgt = CONE_HEIGHT_M.0 + (CONE_HEIGHT_M.1 - CONE_HEIGHT_M.0) * unit(cx, cy, 0xf05);
                return Some((px, py, r, hgt));
            }
        }
    }
    let _ = world;
    None
}

/// Cinder cones on the flanks and the lava's ridges and tumuli, before the rivers wear them.
pub fn shape_before(h: &mut [f32], g: &ChunkGrid) {
    let (gw, s) = (g.gw, g.s);
    let (u0, v0) = g.to_world(0, 0);
    let (u1, v1) = g.to_world(gw - 1, gw - 1);
    let vents = vents_near(g.world, u0, u1, v0, v1);
    let has_lava = g.world.lava_map.is_some();
    if vents.is_empty() && !has_lava { return; }
    let noise = Perlin::new((g.seed as u32).wrapping_add(707));
    for j in 0..gw {
        for i in 0..gw {
            let k = j * gw + i;
            if h[k] <= 0.0 { continue; }
            let (u, v) = g.to_world(i, j);
            let (ax, ay) = g.abs_cell(i, j);
            if let Some((px, py, r, hgt)) = cone_at(g.world, &vents, s, g.cell_m, ax, ay, u, v) {
                let d = (px - ax as f32).hypot(py - ay as f32) / r;
                let mut rise = hgt * (1.0 - d).max(0.0).powf(1.3);
                if d < 0.25 { rise -= 0.35 * hgt * (1.0 - d / 0.25); }
                h[k] += rise;
            }
            if has_lava {
                let lava = lava_at(g.world, u, v);
                if lava > 0.3 {
                    let wu = u.rem_euclid(g.world.width as f32) as f64;
                    let rdg = ridged(&noise, wu * 40.0, v as f64 * 40.0, 3);
                    h[k] += LAVA_RELIEF_M * lava * (0.3 + 0.7 * rdg);
                }
            }
        }
    }
}

/// The summit: a crater bowl with a rim, or a caldera's sunken floor, after the rivers.
pub fn shape_after(h: &mut [f32], g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {
    let (gw, s) = (g.gw, g.s);
    let (u0, v0) = g.to_world(0, 0);
    let (u1, v1) = g.to_world(gw - 1, gw - 1);
    let vents = vents_near(g.world, u0, u1, v0, v1);
    if vents.is_empty() { return; }
    let tile_m = g.cell_m * s as f32;
    for vent in &vents {
        let (rc_m, depth) = crater(vent.kind, tile_m);
        let rc = (rc_m / g.cell_m).max(1.2);
        // The summit's cell: the tile's centre.
        let ci = (vent.u - u0) * s as f32;
        let cj = (vent.v - v0) * s as f32;
        let reach = (1.5 * rc).ceil() as i64;
        for j in ((cj as i64) - reach).max(0)..=((cj as i64) + reach).min(gw as i64 - 1) {
            for i in ((ci as i64) - reach).max(0)..=((ci as i64) + reach).min(gw as i64 - 1) {
                let k = j as usize * gw + i as usize;
                if h[k] <= 0.0 { continue; }
                let d = (i as f32 - ci).hypot(j as f32 - cj) / rc;
                if d < 1.0 {
                    let edge = ((1.0 - d) / 0.25).clamp(0.0, 1.0);
                    let sink = match vent.kind {
                        VolcanoType::Caldera => depth * edge * edge * (3.0 - 2.0 * edge),
                        _ => depth * (1.0 - d * d),
                    };
                    h[k] = (h[k] - sink).max(0.5);
                } else if d < 1.4 {
                    h[k] += 0.15 * depth * (1.0 - (d - 1.0) / 0.4);
                }
            }
        }
    }
}

pub fn classify(f: &RegionFields, rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh, s) = (f.gw, f.gh, f.s);
    let upheaval = f.world.upheaval().is_some();
    let lava_warp = Perlin::new((f.seed as u32).wrapping_add(919));
    let (u0, v0) = f.to_world(0, 0);
    let (u1, v1) = f.to_world(gw - 1, gh - 1);
    let vents = vents_near(f.world, u0, u1, v0, v1);
    let has_lava = f.world.lava_map.is_some();
    if vents.is_empty() && !has_lava { return; }
    let tile_m = f.cell_m * s as f32;
    let noise = Perlin::new((f.seed as u32).wrapping_add(707));
    let ww = f.world.width as f32;
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if f.h[k] <= 0.0 || land[k] != Landform::None { continue; }
            let (u, v) = f.to_world(i, j);
            let (ax, ay) = f.abs_cell(i, j);
            let wu = u.rem_euclid(ww) as f64;
            // The summit.
            let mut nearest: Option<(&Vent, f32)> = None;
            for vent in &vents {
                let d = (vent.u - u).hypot(vent.v - v);
                if nearest.map_or(true, |(_, nd)| d < nd) { nearest = Some((vent, d)); }
            }
            if let Some((vent, d_tiles)) = nearest {
                let (rc_m, _) = crater(vent.kind, tile_m);
                if d_tiles * tile_m <= rc_m.max(1.2 * f.cell_m) {
                    land[k] = if vent.kind == VolcanoType::Stratovolcano { Landform::Crater } else { Landform::Caldera };
                    continue;
                }
            }
            if cone_at(f.world, &vents, s, f.cell_m, ax, ay, u, v).is_some() {
                land[k] = Landform::CinderCone;
                continue;
            }
            let slope = f.slope(i, j);
            // (On an upheaval world the lava runs down the valleys over many tiles (`upheaval::
            // run_lava_downhill`): its edge is read through a warp so the flows are not squares.)
            let (lu, lv) = if upheaval {
                let wu = u.rem_euclid(f.world.width as f32) as f64;
                (u + 0.45 * lava_warp.get([wu * 2.3, v as f64 * 2.3]) as f32, v + 0.45 * lava_warp.get([wu * 2.3 + 17.0, v as f64 * 2.3]) as f32)
            } else { (u, v) };
            if has_lava && lava_at(f.world, lu, lv) > 0.5 && slope < 0.3 {
                land[k] = Landform::LavaField;
                continue;
            }
            let young = young_at(f.world, &vents, u, v);
            let t = f.tile_of(i, j);
            let stress = *f.world.stress_map.get(t.0, t.1);
            if let Some((vent, d_tiles)) = nearest {
                if vent.active && d_tiles <= FUMAROLE_TILES && slope > THERMAL_SLOPE && fbm(&noise, wu as f64 * 10.0, v as f64 * 10.0, 3) > 0.3 {
                    land[k] = Landform::Fumarole;
                    continue;
                }
                let young_near = vents.iter().any(|x| x.active && (x.u - u).hypot(x.v - v) <= THERMAL_TILES) || lava_at(f.world, u, v) > 0.0;
                if young_near && stress > FAULT_STRESS && slope < THERMAL_SLOPE && fbm(&noise, wu as f64 * 8.0 + 3.0, v as f64 * 8.0, 3) > 0.45 {
                    land[k] = Landform::ThermalGround;
                    continue;
                }
                // Ash plains downwind of an active cone, on gentle ground.
                if vent.active && d_tiles <= 1.5 && slope < 0.03 {
                    let lat = (v + 0.5) / f.world.height as f32 * 180.0 - 90.0;
                    let (wx, wy) = wind_dir(lat);
                    let (dx, dy) = (u - vent.u, v - vent.v);
                    let along = dx * wx + dy * wy;
                    let across = (dx * wy - dy * wx).abs();
                    if along > 0.0 && across < 0.5 && fbm(&noise, wu as f64 * 6.0 + 9.0, v as f64 * 6.0, 2) > -0.2 {
                        land[k] = Landform::AshPlain;
                        continue;
                    }
                }
            }
            if rock[k] == RockType::Basalt && slope > 0.12 && !young && fbm(&noise, wu as f64 * 12.0 + 5.0, v as f64 * 12.0, 2) > 0.0 {
                land[k] = Landform::ColumnarBasalt;
            }
        }
    }
}

/// The prevailing wind's direction (x east, y south) by latitude (degrees, signed): trades from
/// the east toward the equator to 30 degrees, westerlies poleward to 60, polar easterlies
/// beyond; blended over 4 degrees at the belt edges. The same rule `aeolian.rs` uses (the
/// climate's wind field is not kept on `WorldData`).
pub(crate) fn wind_dir(lat: f32) -> (f32, f32) {
    let a = lat.abs();
    let poleward = if lat >= 0.0 { -1.0f32 } else { 1.0 };
    let belt = |a: f32| -> (f32, f32) {
        if a < 30.0 { (-1.0, -0.4 * poleward) } else if a < 60.0 { (1.0, 0.4 * poleward) } else { (-1.0, -0.3 * poleward) }
    };
    let mix = |x: f32, y: f32, t: f32| x + (y - x) * t;
    let (bx, by) = belt(a);
    let (ox, oy) = if (26.0..34.0).contains(&a) {
        let t = (a - 26.0) / 8.0;
        let (nx, ny) = belt(if a < 30.0 { 34.0 } else { 26.0 });
        if a < 30.0 { (mix(bx, nx, t), mix(by, ny, t)) } else { (mix(nx, bx, t), mix(ny, by, t)) }
    } else if (56.0..64.0).contains(&a) {
        let t = (a - 56.0) / 8.0;
        let (nx, ny) = belt(if a < 60.0 { 64.0 } else { 56.0 });
        if a < 60.0 { (mix(bx, nx, t), mix(by, ny, t)) } else { (mix(nx, bx, t), mix(ny, by, t)) }
    } else {
        (bx, by)
    };
    let len = ox.hypot(oy).max(1e-3);
    (ox / len, oy / len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightmap::VolcanoLocation;
    use crate::region::zoom::{accumulate, route};

    fn world_with_a_volcano() -> WorldData {
        let mut w = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        w.volcanoes.push(VolcanoLocation { x: 20, y: 12, peak_height: 2000.0, is_active: true, volcano_type: VolcanoType::Stratovolcano });
        w.volcanoes.push(VolcanoLocation { x: 40, y: 16, peak_height: 1200.0, is_active: false, volcano_type: VolcanoType::Shield });
        w
    }

    /// Two chunks a tile apart shape the ground they share identically (cones on an absolute
    /// lattice, craters at fixed tiles).
    #[test]
    fn volcanic_shaping_agrees_between_overlapping_chunks() {
        let w = world_with_a_volcano();
        let s = 16usize;
        let gw = 3 * s;
        let base = |_g: &ChunkGrid| -> Vec<f32> { vec![500.0; gw * gw] };
        let g1 = ChunkGrid { world: &w, gw, s, gx0: 18, gy0: 11, cell_m: 610.0, seed: 1 };
        let g2 = ChunkGrid { world: &w, gw, s, gx0: 19, gy0: 11, cell_m: 610.0, seed: 1 };
        let (mut h1, mut h2) = (base(&g1), base(&g2));
        shape_before(&mut h1, &g1);
        let d1 = route(&h1, gw, gw, 0.0);
        shape_after(&mut h1, &g1, &d1, &accumulate(&d1, &vec![0.0; gw * gw]));
        shape_before(&mut h2, &g2);
        let d2 = route(&h2, gw, gw, 0.0);
        shape_after(&mut h2, &g2, &d2, &accumulate(&d2, &vec![0.0; gw * gw]));
        let mut changed = 0;
        for j in 0..gw {
            for i in s..gw {
                assert_eq!(h1[j * gw + i].to_bits(), h2[j * gw + i - s].to_bits(), "chunks differ at ({i},{j})");
                if h1[j * gw + i] != 500.0 { changed += 1; }
            }
        }
        assert!(changed > 20, "the volcano shaped some ground in the overlap: {changed}");
    }

    /// The summit of a stratovolcano is a crater; cones stand on its flanks; nothing volcanic
    /// is labelled far from any vent.
    #[test]
    fn a_crater_at_the_summit_and_cones_on_the_flanks() {
        let w = world_with_a_volcano();
        let (s, gw, gh) = (16usize, 96usize, 96usize);
        let h = vec![800.0f32; gw * gh];
        let filled = h.clone();
        let recv = vec![u32::MAX; gw * gh];
        let acc = vec![1.0f32; gw * gh];
        let temp = vec![15.0f32; gw * gh];
        let moist = vec![0.4f32; gw * gh];
        let f = RegionFields { world: &w, gw, gh, s, gx0: 18, gy0: 10, margin: 1, cell_m: 610.0, seed: 1, h: &h, filled: &filled, recv: &recv, acc: &acc, temp: &temp, moist: &moist };
        let rock = vec![RockType::Basalt; gw * gh];
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        let count = |l: Landform| land.iter().filter(|&&x| x == l).count();
        assert!(count(Landform::Crater) >= 3, "a crater at the summit: {}", count(Landform::Crater));
        assert!(count(Landform::CinderCone) > 10, "cones on the flanks: {}", count(Landform::CinderCone));
        // The vent is at tile (20, 12): cells far from it (more than 4 tiles) carry nothing.
        for j in 0..gh {
            for i in 0..gw {
                let (u, v) = f.to_world(i, j);
                if (u - 20.0).hypot(v - 12.0) > 4.5 && (u - 40.0).hypot(v - 16.0) > 4.5 {
                    assert!(!matches!(land[j * gw + i], Landform::Crater | Landform::CinderCone | Landform::Fumarole | Landform::ThermalGround | Landform::AshPlain), "volcanic label far from the vents at ({i},{j})");
                }
            }
        }
    }

    #[test]
    fn winds_follow_the_belts() {
        assert!(wind_dir(10.0).0 < 0.0, "trades blow westward");
        assert!(wind_dir(45.0).0 > 0.0, "westerlies blow eastward");
        assert!(wind_dir(-45.0).0 > 0.0);
        assert!(wind_dir(75.0).0 < 0.0, "polar easterlies blow westward");
        assert!(wind_dir(10.0).1 > 0.0, "northern trades blow toward the equator (south)");
        assert!(wind_dir(-10.0).1 < 0.0, "southern trades blow toward the equator (north)");
    }
}
