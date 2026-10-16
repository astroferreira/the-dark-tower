//! Rivers at region scale: floodplains with meander belts, oxbows, levees and backswamps on
//! low gradients; braided reaches below the mountains; terraces (the valley's incision history);
//! waterfalls at knickpoints, rapids and gorges on the channel; alluvial fans at range fronts;
//! deltas (arcuate on open coasts, bird's-foot in sheltered water) and estuaries at the mouths
//! (ROADMAP Stage 2).
//!
//! Everything here is a label on the blended region (`classify`): the channel and its drainage
//! are only known there, so the chunk passes shape nothing; the embark carves the levee, the
//! oxbow's hollow and the terrace's step from the labels (Stage 6). Every rule reads the cell,
//! its drainage chain down to the nearest channel and a bounded neighbourhood, nothing beyond
//! the region's margin.

use std::collections::HashMap;

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, Drainage, RegionFields};

const NONE: u32 = u32::MAX;
/// The region's own river rule (`zoom::RIVER_AREA_FRACTION`): a cell is a channel where its
/// catchment passes this share of a world tile's area, more in dry country.
const RIVER_AREA_FRACTION: f32 = 0.02;
/// Catchment (in world tiles) of a river big enough to build a floodplain and a delta.
const BIG_TILES: f32 = 0.3;
/// Catchment (in world tiles) of a great river: levees, oxbows.
const GREAT_TILES: f32 = 2.0;
/// Channel gradient (m per m) below which a river meanders and builds a floodplain.
const MEANDER_GRADIENT: f32 = 0.004;
/// Height above the channel (m) that counts as its floodplain.
const FLOODPLAIN_HAND_M: f32 = 5.0;
/// Meander belt half-width: this many metres per sqrt(km2) of catchment (~15 channel widths).
const BELT_M_PER_SQRT_KM2: f32 = 12.0;
/// Terrace benches stand this far above the channel (m).
const TERRACE_HAND_M: (f32, f32) = (8.0, 40.0);
/// A knickpoint drops this much (m) over one cell at 610 m cells (scaled with the cell).
const FALLS_DROP_M: f32 = 8.0;
const RAPIDS_DROP_M: f32 = 3.0;
/// A gorge's walls rise this much (m) within three cells of the channel on both sides.
const GORGE_WALL_M: f32 = 60.0;
/// Relief over the world tile's 3x3 neighbourhood (m) that says "below the mountains".
const MOUNTAIN_RELIEF_M: f32 = 1500.0;
const FAN_RELIEF_M: f32 = 800.0;
/// Sea cells within this radius of a mouth are counted for its fetch (open or sheltered).
const FETCH_R: i64 = 12;
/// Oxbows per floodplain cell of a great river (hashed).
const OXBOW_RATE: f32 = 0.004;

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn unit(a: i64, b: i64, salt: u64) -> f32 {
    (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0
}

/// Which cells are channels, by the region's own rule (`river_width_m` is set after the
/// landforms, from the same rule).
pub(super) fn channel_mask(f: &RegionFields) -> Vec<bool> {
    let tile_cells = (f.s * f.s) as f32;
    (0..f.gw * f.gh)
        .map(|k| f.h[k] > 0.0 && f.lake_depth(k) == 0.0 && f.acc[k] > RIVER_AREA_FRACTION * tile_cells / (0.25 + f.moist[k]))
        .collect()
}

/// Every cell's nearest channel downstream (`NONE` where the chain reaches the sea, a lake's
/// floor or the grid's edge first), its height above it (HAND) and the steps down to it.
pub(super) struct Channels {
    pub mask: Vec<bool>,
    pub nearest: Vec<u32>,
    pub hand: Vec<f32>,
    pub steps: Vec<u16>,
}

pub(super) fn channels(f: &RegionFields) -> Channels {
    let n = f.gw * f.gh;
    let mask = channel_mask(f);
    let mut nearest = vec![NONE; n];
    let mut steps = vec![u16::MAX; n];
    let mut resolved = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if resolved[start] { continue; }
        // Walk the chain down to something already known; path-compress on the way back.
        let mut c = start;
        loop {
            if resolved[c] { break; }
            if mask[c] {
                nearest[c] = c as u32;
                steps[c] = 0;
                resolved[c] = true;
                break;
            }
            if f.h[c] <= 0.0 || f.recv[c] == NONE {
                resolved[c] = true;
                break;
            }
            stack.push(c);
            c = f.recv[c] as usize;
        }
        let (base_n, mut d) = (nearest[c], steps[c]);
        while let Some(u) = stack.pop() {
            d = d.saturating_add(1);
            nearest[u] = base_n;
            steps[u] = if base_n == NONE { u16::MAX } else { d };
            resolved[u] = true;
        }
    }
    let hand = (0..n).map(|k| if nearest[k] == NONE { f32::INFINITY } else { f.h[k] - f.h[nearest[k] as usize] }).collect();
    Channels { mask, nearest, hand, steps }
}

/// Relief (m) over the 3x3 world tiles round a tile, cached per tile.
fn tile_relief(f: &RegionFields, cache: &mut HashMap<(usize, usize), f32>, t: (usize, usize)) -> f32 {
    *cache.entry(t).or_insert_with(|| {
        let (ww, wh) = (f.world.width as i64, f.world.height as i64);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y) = ((t.0 as i64 + dx).rem_euclid(ww) as usize, (t.1 as i64 + dy).clamp(0, wh - 1) as usize);
                let e = *f.world.heightmap.get(x, y);
                lo = lo.min(e);
                hi = hi.max(e);
            }
        }
        hi - lo
    })
}

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {}

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let n = gw * gh;
    let ch = channels(f);
    let s2 = (f.s * f.s) as f32;
    let km2_per_cell = (f.cell_m / 1000.0).powi(2);
    let scale = (f.cell_m / 610.0).clamp(1.0, 6.0);
    let (falls_drop, rapids_drop) = (FALLS_DROP_M * scale, RAPIDS_DROP_M * scale);
    let (big, great) = (BIG_TILES * s2, GREAT_TILES * s2);
    let mut relief_cache = HashMap::new();

    // The highest donor of every cell: a knickpoint drops more than the reaches above and
    // below it; a fan's apex has a steep tributary coming in and gentle ground going out.
    let mut donor_h = vec![f32::MIN; n];
    for k in 0..n {
        let r = f.recv[k];
        if r != NONE && f.h[k] > donor_h[r as usize] { donor_h[r as usize] = f.h[k]; }
    }
    let drop_of = |k: usize| -> f32 { let r = f.recv[k]; if r == NONE { 0.0 } else { (f.h[k] - f.h[r as usize]).max(0.0) } };

    let mut mouths: Vec<usize> = Vec::new();
    let mut apexes: Vec<usize> = Vec::new();
    let mut oxbows: Vec<(usize, usize)> = Vec::new();

    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if f.h[k] <= 0.0 { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            if ch.mask[k] {
                let r = f.recv[k];
                if r == NONE { continue; }
                let r = r as usize;
                if f.h[r] <= 0.0 {
                    mouths.push(k);
                    continue;
                }
                let drop = f.h[k] - f.h[r];
                let grad = drop / f.cell_m;
                let (rx, ry) = ((r % gw) as i64 - i as i64, (r / gw) as i64 - j as i64);
                // Gorge: both banks rise above the channel within three cells across the flow.
                let (px, py) = (-ry, rx);
                let wall = |sign: i64| -> f32 {
                    let mut top = f32::MIN;
                    for t in 1..=3 {
                        let (x, y) = (i as i64 + sign * px * t, j as i64 + sign * py * t);
                        if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { break; }
                        top = top.max(f.h[y as usize * gw + x as usize]);
                    }
                    top - f.h[k]
                };
                if (px != 0 || py != 0) && wall(1) > GORGE_WALL_M && wall(-1) > GORGE_WALL_M {
                    land[k] = Landform::Gorge;
                    for sign in [1i64, -1] {
                        for t in 1..=2 {
                            let (x, y) = (i as i64 + sign * px * t, j as i64 + sign * py * t);
                            if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { break; }
                            let w = y as usize * gw + x as usize;
                            if land[w] == Landform::None && f.h[w] - f.h[k] > GORGE_WALL_M * 0.5 { land[w] = Landform::Gorge; }
                        }
                    }
                    continue;
                }
                let above = (donor_h[k] - f.h[k]).max(0.0);
                let below = drop_of(r);
                if drop > falls_drop && drop > 2.5 * above.max(below) {
                    land[k] = Landform::Waterfall;
                    continue;
                }
                if drop > rapids_drop && grad > 0.005 {
                    land[k] = Landform::Rapids;
                    continue;
                }
                let relief = tile_relief(f, &mut relief_cache, f.tile_of(i, j));
                if f.acc[k] > 0.2 * s2 && (0.003..0.02).contains(&grad) && relief > MOUNTAIN_RELIEF_M {
                    land[k] = Landform::Braided;
                    continue;
                }
                // A fan's apex: a steep tributary meets gentle ground below the mountains.
                if f.acc[k] > 0.01 * s2 && f.acc[k] < 0.5 * s2 && grad < 0.006 && above / f.cell_m > 0.03 && relief > FAN_RELIEF_M {
                    apexes.push(k);
                }
                continue;
            }
            let near = ch.nearest[k];
            if near == NONE { continue; }
            let c = near as usize;
            let acc_c = f.acc[c];
            if acc_c < big { continue; }
            let grad_c = drop_of(c) / f.cell_m;
            if grad_c >= MEANDER_GRADIENT { continue; }
            let dist_m = ch.steps[k] as f32 * f.cell_m;
            let hand = ch.hand[k];
            let belt = (BELT_M_PER_SQRT_KM2 * (acc_c * km2_per_cell * f.lat_cos(j)).sqrt()).max(f.cell_m);
            if hand < FLOODPLAIN_HAND_M && dist_m <= belt {
                land[k] = if ch.steps[k] <= 1 && acc_c > great {
                    Landform::Levee
                } else if dist_m > 0.5 * belt && hand < 2.0 {
                    Landform::Backswamp
                } else {
                    Landform::Floodplain
                };
                if acc_c > great && dist_m > 0.2 * belt && dist_m < 0.7 * belt && unit(ax, ay, 0x0b0) < OXBOW_RATE {
                    oxbows.push((i, j));
                }
            } else if hand >= TERRACE_HAND_M.0 && hand <= TERRACE_HAND_M.1 && dist_m <= 3.0 * belt && f.slope(i, j) < 0.03 {
                land[k] = Landform::Terrace;
            }
        }
    }

    // Oxbows: a cut-off loop beside the river, an arc of cells (one cell where cells are km).
    let arc_r = (1200.0 / f.cell_m).round().max(1.0) as i64;
    for (i, j) in oxbows {
        let (ax, ay) = f.abs_cell(i, j);
        let a0 = unit(ax, ay, 0x0b1) * std::f32::consts::TAU;
        let steps = if arc_r <= 1 { 1 } else { 7 };
        for t in 0..steps {
            let a = a0 + (t as f32 - 3.0) * 0.5;
            let (x, y) = (i as i64 + (arc_r as f32 * a.cos()).round() as i64, j as i64 + (arc_r as f32 * a.sin()).round() as i64);
            if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { continue; }
            let w = y as usize * gw + x as usize;
            if matches!(land[w], Landform::None | Landform::Floodplain | Landform::Backswamp) && f.h[w] > 0.0 && !ch.mask[w] {
                land[w] = Landform::Oxbow;
            }
        }
    }

    // Fans: a cone of ground below each apex, on the side the water goes.
    let fan_r = (2000.0 / f.cell_m).round().max(2.0) as i64;
    for k in apexes {
        let (i, j) = ((k % gw) as i64, (k / gw) as i64);
        let r = f.recv[k] as usize;
        let (dx, dy) = ((r % gw) as i64 - i, (r / gw) as i64 - j);
        for y in (j - fan_r).max(0)..=(j + fan_r).min(gh as i64 - 1) {
            for x in (i - fan_r).max(0)..=(i + fan_r).min(gw as i64 - 1) {
                let (ox, oy) = (x - i, y - j);
                if ox * ox + oy * oy > fan_r * fan_r || ox * dx + oy * dy < -fan_r / 2 { continue; }
                let w = y as usize * gw + x as usize;
                if land[w] != Landform::None || ch.mask[w] || f.h[w] <= 0.0 { continue; }
                if f.h[w] >= f.h[k] - 2.0 && f.h[w] <= f.h[k] + 30.0 { land[w] = Landform::Fan; }
            }
        }
    }

    // Mouths: a delta where a big river meets the sea, an estuary where a smaller one does.
    let sea_in_cone = |m: usize, dx: i64, dy: i64| -> usize {
        let (i, j) = ((m % gw) as i64, (m / gw) as i64);
        let mut count = 0;
        for y in (j - FETCH_R).max(0)..=(j + FETCH_R).min(gh as i64 - 1) {
            for x in (i - FETCH_R).max(0)..=(i + FETCH_R).min(gw as i64 - 1) {
                let (ox, oy) = (x - i, y - j);
                if ox * ox + oy * oy > FETCH_R * FETCH_R || ox * dx + oy * dy <= 0 { continue; }
                if f.h[y as usize * gw + x as usize] <= 0.0 { count += 1; }
            }
        }
        count
    };
    let half_disc = (std::f32::consts::PI * (FETCH_R * FETCH_R) as f32 / 2.0) as usize;
    for m in mouths {
        let acc = f.acc[m];
        let (i, j) = ((m % gw) as i64, (m / gw) as i64);
        let r = f.recv[m] as usize;
        let (dx, dy) = ((r % gw) as i64 - i, (r / gw) as i64 - j);
        if acc > BIG_TILES * s2 {
            let open = sea_in_cone(m, dx, dy) > half_disc / 2;
            let rad = (3.0 * (acc / s2).sqrt()).clamp(2.0, 12.0);
            let rad_i = rad.ceil() as i64;
            // Lobes: three for a bird's-foot delta in sheltered water, the whole fan when open.
            let (ax, ay) = f.abs_cell(i as usize, j as usize);
            let base = (dy as f32).atan2(dx as f32) + (unit(ax, ay, 0x0d1) - 0.5) * 0.4;
            for y in (j - rad_i).max(0)..=(j + rad_i).min(gh as i64 - 1) {
                for x in (i - rad_i).max(0)..=(i + rad_i).min(gw as i64 - 1) {
                    let w = y as usize * gw + x as usize;
                    if f.h[w] <= 0.0 || land[w] != Landform::None { continue; }
                    let (ox, oy) = ((x - i) as f32, (y - j) as f32);
                    let d = (ox * ox + oy * oy).sqrt();
                    if d > rad { continue; }
                    let inside = if open || d <= 0.5 * rad {
                        true
                    } else {
                        let ang = oy.atan2(ox);
                        [-0.6f32, 0.0, 0.6].iter().any(|&da| {
                            let mut diff = (ang - (base + da)).abs();
                            if diff > std::f32::consts::PI { diff = std::f32::consts::TAU - diff; }
                            diff < 0.22
                        })
                    };
                    if inside { land[w] = Landform::Delta; }
                }
            }
        } else if acc > 0.05 * s2 {
            let rad = (6.0f32).min(3.0 + (acc / s2).sqrt() * 6.0) as i64;
            for y in (j - rad).max(0)..=(j + rad).min(gh as i64 - 1) {
                for x in (i - rad).max(0)..=(i + rad).min(gw as i64 - 1) {
                    let w = y as usize * gw + x as usize;
                    if f.h[w] <= 0.0 || f.h[w] > 4.0 || land[w] != Landform::None || ch.mask[w] { continue; }
                    land[w] = Landform::Estuary;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::zoom::{accumulate, route};

    /// A flat plain sloping to a sea along the bottom, with a trench down the middle that
    /// gathers a great river, and a mountain block in the top-left corner.
    fn plain(gw: usize, gh: usize) -> Vec<f32> {
        (0..gw * gh)
            .map(|k| {
                let (x, y) = ((k % gw) as f32, (k / gw) as f32);
                if y >= gh as f32 - 6.0 { return -5.0; }
                let mut h = 2.0 + (gh as f32 - 6.0 - y) * 0.6;
                let dx = (x - gw as f32 / 2.0).abs();
                h -= 1.5 * (1.0 - dx / 3.0).max(0.0);
                if x < 20.0 && y < 20.0 { h += 2000.0 * ((20.0 - x) / 20.0) * ((20.0 - y) / 20.0); }
                h
            })
            .collect()
    }

    #[test]
    fn a_great_river_builds_a_floodplain_and_a_delta() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let (s, gw, gh) = (32usize, 96usize, 96usize);
        let h = plain(gw, gh);
        let d = route(&h, gw, gh, 0.0);
        let mut inflow = vec![0.0f32; gw * gh];
        inflow[gw / 2] = 3.0 * (s * s) as f32; // a great river enters at the top
        let acc = accumulate(&d, &inflow);
        let temp = vec![15.0f32; gw * gh];
        let moist = vec![0.7f32; gw * gh];
        let f = RegionFields { world: &world, gw, gh, s, gx0: 3, gy0: 3, margin: 1, cell_m: 610.0, seed: 1, h: &h, filled: &d.filled, recv: &d.recv, acc: &acc, temp: &temp, moist: &moist };
        let rock = vec![RockType::Sandstone; gw * gh];
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        let count = |l: Landform| land.iter().filter(|&&x| x == l).count();
        assert!(count(Landform::Floodplain) + count(Landform::Levee) + count(Landform::Backswamp) > 40, "floodplain along the trench: {}", count(Landform::Floodplain));
        assert!(count(Landform::Delta) > 3, "a delta at the mouth: {}", count(Landform::Delta));
        for k in 0..gw * gh {
            assert!(!(h[k] <= 0.0 && land[k] != Landform::None), "no river label on the sea at {k}");
        }
    }

    #[test]
    fn channels_resolve_every_cell_downstream() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let (s, gw, gh) = (32usize, 96usize, 96usize);
        let h = plain(gw, gh);
        let d = route(&h, gw, gh, 0.0);
        let mut inflow = vec![0.0f32; gw * gh];
        inflow[gw / 2] = 3.0 * (s * s) as f32;
        let acc = accumulate(&d, &inflow);
        let temp = vec![15.0f32; gw * gh];
        let moist = vec![0.7f32; gw * gh];
        let f = RegionFields { world: &world, gw, gh, s, gx0: 0, gy0: 0, margin: 1, cell_m: 610.0, seed: 1, h: &h, filled: &d.filled, recv: &d.recv, acc: &acc, temp: &temp, moist: &moist };
        let ch = channels(&f);
        let channel_cells = ch.mask.iter().filter(|&&m| m).count();
        assert!(channel_cells > 50, "the trench is a channel: {channel_cells}");
        for k in 0..gw * gh {
            if ch.nearest[k] != NONE {
                assert!(ch.mask[ch.nearest[k] as usize]);
                assert!(ch.hand[k] >= -1e-3, "HAND is a height above the channel");
                assert!(ch.steps[k] < u16::MAX);
            }
        }
    }
}
