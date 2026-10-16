//! The world's springs and waterfalls (`underground_water.rs`, kept on
//! `WorldData::underground_water`) brought down to the region and placed on its channels and
//! slopes: the first caused places, and the first `views_agree` cases (ROADMAP Stage 1).
//!
//! Per core tile: the world's waterfall becomes a `Waterfall` at the lip on the tile's main stem
//! with the greatest fall (the pool's height is `z_m`, the rank follows the world's drops); a
//! seepage, artesian or karst spring becomes up to two `Spring`s at the spots its kind fits: an
//! artesian or karst spring, or a seepage spring under a scarp, at a slope foot; a headwater
//! seepage spring at the channel head with the largest catchment. Thermal springs are the
//! natural wonders' (`natural.rs`, `HotSpring`).
//!
//! Deterministic per tile and the same from any window: every choice is a first-best over the
//! tile's own cells (ties to the lower absolute cell), reading at most three receivers beyond
//! the tile, all inside the region's margin.

use crate::erosion::materials::RockType;
use crate::erosion::rivers::{DX, DY};
use crate::region::landforms::RegionFields;
use crate::region::plan::{Landform, Place, PlaceKind};
use crate::region::zoom::{LAKE_MIN_DEPTH_M, RIVER_AREA_FRACTION};
use crate::underground_water::{SpringType, UndergroundWater, UndergroundWaterParams};

use super::{Core, SitePlan};

/// A strong spring (flow 0..1) earns a second spot.
const SECOND_SPRING_FLOW: f32 = 0.6;
/// A caprock ledge: this much harder at the lip than one tile downstream (the contrast
/// `detect_waterfalls` lowers its threshold for).
const LEDGE_HARDNESS_STEP: f32 = 0.15;
/// The falls stand on the tile's main stem: channel cells with at least this share of the
/// largest catchment among the tile's channel cells.
const MAIN_STEM_SHARE: f32 = 0.1;
/// A fall is measured from the lip to the lowest cell within this many receivers, and counts
/// from this many metres (`survey::check` holds the same rule).
const FALL_STEPS: usize = 3;
const MIN_FALL_M: f32 = 2.0;
/// A slope foot: a cell under `FOOT_FLAT` (m per m) beside one over `FOOT_STEEP`.
const FOOT_FLAT: f32 = 0.04;
const FOOT_STEEP: f32 = 0.08;
/// A channel head carries at most this many times its threshold area; more is a river let in at
/// the region's edge.
const HEAD_MAX_THRESHOLDS: f32 = 2.0;
/// Two springs of one tile stand at least this many cells apart (either axis).
const SPRING_SPACING_CELLS: i64 = 4;

/// The catchment (cells) a channel needs at cell `k`: the region's own rule (`zoom`), more on
/// drier ground.
fn threshold(f: &RegionFields, k: usize) -> f32 {
    RIVER_AREA_FRACTION * (f.s * f.s) as f32 / (0.25 + f.moist[k])
}

/// A channel cell, as the cropped region marks it (`river_width_m > 0`).
fn channel(f: &RegionFields, k: usize) -> bool {
    f.h[k] > 0.0 && f.lake_depth(k) < LAKE_MIN_DEPTH_M && f.acc[k] > threshold(f, k)
}

fn rock_word(r: RockType) -> &'static str {
    match r {
        RockType::Basalt => "basalt", RockType::Granite => "granite", RockType::Sandstone => "sandstone",
        RockType::Limestone => "limestone", RockType::Shale => "shale", RockType::Sediment => "sediment", RockType::Ice => "ice",
    }
}

/// The rock the water works in at cell `k`: the surface rock, or under loose sediment the first
/// bedrock of the tile's rock stack.
fn bedrock(f: &RegionFields, rock: &[RockType], k: usize, tile: (usize, usize)) -> RockType {
    if rock[k] != RockType::Sediment { return rock[k]; }
    f.world.handshakes.as_ref()
        .and_then(|hs| hs.get(tile.0, tile.1).tile.rock_stack.iter().map(|l| l.rock_type).find(|&r| r != RockType::Sediment))
        .unwrap_or(RockType::Sediment)
}

/// The tile's cells as grid indices in row-major order, when the whole tile is in the grid.
fn tile_cells(f: &RegionFields, tx: i64, ty: i64) -> Option<Vec<usize>> {
    let s = f.s as i64;
    let (i0, j0) = ((tx - f.gx0) * s, (ty - f.gy0) * s);
    if i0 < 0 || j0 < 0 || i0 + s > f.gw as i64 || j0 + s > f.gh as i64 { return None; }
    let (i0, j0, s) = (i0 as usize, j0 as usize, s as usize);
    Some((j0..j0 + s).flat_map(|j| (i0..i0 + s).map(move |i| f.idx(i, j))).collect())
}

/// The fall at `k`: the greatest drop from `k` to a cell within `FALL_STEPS` receivers, and that
/// cell (the pool).
fn fall(f: &RegionFields, k: usize) -> Option<(f32, usize)> {
    let (mut c, mut best) = (k, None::<(f32, usize)>);
    for _ in 0..FALL_STEPS {
        let rc = f.recv[c];
        if rc == u32::MAX { break; }
        c = rc as usize;
        let d = f.h[k] - f.h[c];
        if best.map(|(b, _)| d > b).unwrap_or(true) { best = Some((d, c)); }
    }
    best
}

/// The steepest slope among the 8 neighbours of a flat cell, if it is a slope foot.
fn foot(f: &RegionFields, k: usize) -> Option<f32> {
    let (i, j) = (k % f.gw, k / f.gw);
    if f.slope(i, j) >= FOOT_FLAT { return None; }
    let mut steep = 0.0f32;
    for dj in -1i64..=1 {
        for di in -1i64..=1 {
            let (x, y) = (i as i64 + di, j as i64 + dj);
            if (di, dj) == (0, 0) || x < 0 || y < 0 || x >= f.gw as i64 || y >= f.gh as i64 { continue; }
            steep = steep.max(f.slope(x as usize, y as usize));
        }
    }
    (steep > FOOT_STEEP).then_some(steep)
}

/// How many of the world's present features rank above `v` (0 the greatest).
fn world_rank(values: &[f32], v: f32) -> u8 {
    values.iter().filter(|&&x| x > v).count().min(254) as u8
}

pub fn plan(f: &RegionFields, _land: &[Landform], rock: &[RockType], core: Core, out: &mut SitePlan) {
    let Some(uw) = f.world.underground_water.as_ref() else { return };
    let world = f.world;
    let (ww, wh) = (world.width, world.height);
    let hardness = |x: usize, y: usize| world.hardness_map.as_ref().map(|m| *m.get(x, y)).unwrap_or(0.5);
    let tiles: Vec<(i64, i64, usize, usize)> = core.tiles()
        .map(|(tx, ty)| (tx, ty, tx.rem_euclid(ww as i64) as usize, ty.clamp(0, wh as i64 - 1) as usize))
        .filter(|&(_, _, wx, wy)| *world.heightmap.get(wx, wy) > 0.0)
        .collect();
    let any_fall = tiles.iter().any(|&(_, _, wx, wy)| uw.waterfalls.get(wx, wy).is_present);
    let any_spring = tiles.iter().any(|&(_, _, wx, wy)| { let sp = uw.springs.get(wx, wy); sp.is_present() && sp.spring_type != SpringType::Thermal });
    if !any_fall && !any_spring { return; }
    let (drops, flows) = world_orders(uw);
    // Channel cells fed by another channel cell (a head is a channel cell that is not).
    let fed = if any_spring {
        let mut fed = vec![false; f.gw * f.gh];
        for k in 0..f.gw * f.gh {
            if f.recv[k] != u32::MAX && channel(f, k) { fed[f.recv[k] as usize] = true; }
        }
        fed
    } else { Vec::new() };

    for (tx, ty, wx, wy) in tiles {
        let Some(cells) = tile_cells(f, tx, ty) else { continue };
        let tile = (wx, wy);
        let at = |k: usize| f.abs_cell(k % f.gw, k / f.gw);

        // The waterfall: the lip on the main stem with the greatest fall.
        let wf = *uw.waterfalls.get(wx, wy);
        if wf.is_present {
            let stem = cells.iter().filter(|&&k| channel(f, k)).map(|&k| f.acc[k]).fold(0.0f32, f32::max) * MAIN_STEM_SHARE;
            let mut best: Option<(usize, f32, usize)> = None;
            for &k in &cells {
                if !channel(f, k) || f.acc[k] < stem { continue; }
                let Some((d, pool)) = fall(f, k) else { continue };
                if d >= MIN_FALL_M && best.map(|(_, b, _)| d > b).unwrap_or(true) { best = Some((k, d, pool)); }
            }
            if let Some((k, d, pool)) = best {
                let (ax, ay) = at(k);
                let mut p = Place::point(PlaceKind::Waterfall, ax as f64, ay as f64, tile);
                // The pool's height: the region's, or the world's drop below the lip where the
                // region's erosion has smoothed the knickpoint (the embark cuts the step to it).
                p.z_m = Some(f.h[pool].min(f.h[k] - wf.drop_height));
                p.rank = world_rank(&drops, wf.drop_height);
                // Why here: hard rock at the lip over softer rock a tile downstream (Niagara's
                // dolomite over shale), else the knickpoint the world's river makes leaving the
                // upland.
                let (dx, dy) = ((wx as i64 + DX[wf.direction as usize % 8] as i64).rem_euclid(ww as i64) as usize,
                    (wy as i64 + DY[wf.direction as usize % 8] as i64).clamp(0, wh as i64 - 1) as usize);
                let (h_lip, h_down) = (hardness(wx, wy), hardness(dx, dy));
                let rk = rock_word(bedrock(f, rock, k, tile));
                p.cause = if h_lip - h_down > LEDGE_HARDNESS_STEP {
                    format!("a caprock ledge: hard {rk} over softer rock downstream (hardness {h_lip:.2} over {h_down:.2})")
                } else if h_lip > 0.7 {
                    format!("a knickpoint held up by hard {rk} (hardness {h_lip:.2})")
                } else {
                    format!("a knickpoint where the river leaves the upland, {:.0} m above the next valley", wf.drop_height)
                };
                let (j, width) = (k / f.gw, 0.8 * (f.acc[k] * (f.cell_m / 1000.0).powi(2)).sqrt());
                let lat_w = width * f.lat_cos(j).sqrt();
                p.description = format!("The {} falls {d:.0} m over a ledge of {rk}, {lat_w:.0} m across; the pool below lies {:.0} m above the sea.",
                    if lat_w >= 10.0 { "river" } else { "stream" }, f.h[pool]);
                p.contents = vec!["a plunge pool".to_string()];
                out.places.push(p);
            }
        }

        // The springs: the spots the spring's kind fits, the other kind of spot if none.
        let sp = *uw.springs.get(wx, wy);
        if !sp.is_present() || sp.spring_type == SpringType::Thermal { continue; }
        let heads = || {
            let mut v: Vec<(usize, f32)> = cells.iter().copied()
                .filter(|&k| channel(f, k) && !fed[k] && f.acc[k] <= HEAD_MAX_THRESHOLDS * threshold(f, k))
                .map(|k| (k, f.acc[k])).collect();
            v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            v.into_iter().map(|(k, _)| (k, true)).collect::<Vec<_>>()
        };
        let feet = || {
            let mut v: Vec<(usize, f32)> = cells.iter().copied()
                .filter(|&k| f.h[k] > 0.0 && f.lake_depth(k) <= 0.0 && !channel(f, k))
                .filter_map(|k| foot(f, k).map(|s| (k, s))).collect();
            v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            v.into_iter().map(|(k, _)| (k, false)).collect::<Vec<_>>()
        };
        // A seepage spring under a scarp (a neighbour tile higher by the world's spring
        // gradient) is a contact spring at its foot; otherwise a headwater spring.
        let scarp = {
            let e = *world.heightmap.get(wx, wy);
            let g = UndergroundWaterParams::default().spring_gradient_threshold;
            (-1i64..=1).flat_map(|dy| (-1i64..=1).map(move |dx| (dx, dy))).any(|(dx, dy)| {
                let (x, y) = ((wx as i64 + dx).rem_euclid(ww as i64) as usize, (wy as i64 + dy).clamp(0, wh as i64 - 1) as usize);
                *world.heightmap.get(x, y) - e > g
            })
        };
        let at_foot = match sp.spring_type { SpringType::Artesian | SpringType::Karst => true, _ => scarp };
        let spots: Vec<(usize, bool)> = if at_foot { feet().into_iter().chain(heads()).collect() } else { heads().into_iter().chain(feet()).collect() };
        let want = if sp.flow_rate > SECOND_SPRING_FLOW { 2 } else { 1 };
        let mut chosen: Vec<(usize, bool)> = Vec::new();
        for (k, head) in spots {
            if chosen.len() >= want { break; }
            let (ax, ay) = at(k);
            let apart = chosen.iter().all(|&(c, _)| { let (cx, cy) = at(c); (cx - ax).abs().max((cy - ay).abs()) >= SPRING_SPACING_CELLS });
            if apart { chosen.push((k, head)); }
        }
        for (n, &(k, head)) in chosen.iter().enumerate() {
            let (ax, ay) = at(k);
            let mut p = Place::point(PlaceKind::Spring, ax as f64, ay as f64, tile);
            p.rank = world_rank(&flows, sp.flow_rate).saturating_add(n as u8);
            let rk = rock_word(bedrock(f, rock, k, tile));
            let (cause, what) = match (sp.spring_type, head) {
                (SpringType::Artesian, _) => ("a confined aquifer under the head of the uplands, opened by a fault at the slope foot".to_string(),
                    "Cold water wells up from the ground, steady through the dry months"),
                (SpringType::Karst, _) => (format!("a perched aquifer in the {rk} spills at its edge"),
                    "A stream pours from a crack in the rock, full and cloudy after rain"),
                (_, false) => (format!("the water table meets the foot of the slope in the {rk}"),
                    "Water seeps from the foot of the slope and gathers into a rill"),
                (_, true) => (format!("a shallow aquifer in the {rk} feeds the head of the stream"),
                    "The stream begins here, at a pool fed from the ground"),
            };
            p.cause = cause;
            p.description = format!("{what}; {}.", if sp.flow_rate > SECOND_SPRING_FLOW { "strong enough to turn a mill wheel" } else { "enough to water a camp all year" });
            if n > 0 { p.description.push_str(" A second spring of the same water."); }
            p.contents = vec!["fresh water".to_string()];
            out.places.push(p);
        }
    }
}

/// The world's waterfall drops and its springs' flows (thermal ones left out), for the ranks.
fn world_orders(uw: &UndergroundWater) -> (Vec<f32>, Vec<f32>) {
    let drops = uw.waterfalls.iter().map(|(_, _, w)| w).filter(|w| w.is_present).map(|w| w.drop_height).collect();
    let flows = uw.springs.iter().map(|(_, _, s)| s).filter(|s| s.is_present() && s.spring_type != SpringType::Thermal).map(|s| s.flow_rate).collect();
    (drops, flows)
}
