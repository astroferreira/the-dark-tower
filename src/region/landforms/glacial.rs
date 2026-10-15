//! What the ice left where the world's ice reached, and where it still lies (ROADMAP Stage 2).
//!
//! `glaciation` reads the world's climate per tile: the ice age reached ground that is cold
//! today (annual mean under 6 C) past 40 degrees of latitude, and every mountain colder than
//! -2 C wherever it stands; ice lies today where the mean at height is under -2 C. Sampled
//! smoothly between tiles, the weight `once` is 1 under the old ice and fades to 0 across its
//! front, so the front is a line the terminal moraine can follow.
//!
//! Once glaciated: the trunk valleys are widened to U cross-sections (`shape_after`), cirque
//! bowls are cut at valley heads high in the relief (a tarn where one holds water), aretes are
//! the sharp ridges between, drumlins (streamlined hills along the ice flow) and the terminal
//! moraine (a ridge along the front, across the valley floors) are raised before the rivers
//! wear them (`shape_before`), outwash plains lie just beyond the front with kettle holes,
//! erratics stand scattered on the low ground. Ice now: glaciers in the valleys and on the
//! caps, snowfields above the snow line, patterned ground on the cold flats past the tree
//! line. Amplitudes are in metres; distances in cells are scaled by `rock::scale` so the dev
//! world's 3 km cells read like the 610 m cells of a 512-wide world.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::tilemap::Tilemap;
use crate::world::WorldData;

use super::rock::{chunk_slopes, lattice, max_filter, river_threshold, sample_tile, scale, slopes, smoothstep, unit};
use super::{ChunkGrid, Drainage, RegionFields};

/// Annual mean (C) under which the ice age reached the ground past `ICE_AGE_LAT`.
const ICE_AGE_TEMP_C: (f32, f32) = (8.0, 2.0);
const ICE_AGE_LAT: (f32, f32) = (35.0, 45.0);
/// Annual mean under which a mountain was glaciated wherever it stands.
const ALPINE_TEMP_C: (f32, f32) = (2.0, -4.0);
/// Mean at height under which ice lies today, and snow stays the year.
const GLACIER_TEMP_C: f32 = -2.0;
const SNOW_TEMP_C: f32 = 0.0;
/// Patterned ground: cold flats between these means.
const PATTERNED_TEMP_C: (f32, f32) = (-1.0, 2.0);
/// A trunk valley holds this share of a tile's cells of catchment.
const TRUNK_SHARE: f32 = 0.005;
/// U-valley half-width (cells) at 610 m cells: base plus a term in the catchment.
const U_WIDTH: (f32, f32) = (2.0, 8.0);
/// Cirques: valley heads above this height with this relief within 4 cells; the bowl's radius
/// (cells) and depth (m).
const CIRQUE_MIN_H: f32 = 1500.0;
const CIRQUE_RELIEF_M: f32 = 150.0;
const CIRQUE_RADIUS: f32 = 3.0;
const CIRQUE_DEPTH_M: f32 = 45.0;
/// An arete: a convex ridge this far above its neighbours' mean, in glaciated high ground.
const ARETE_CONVEX_M: f32 = 20.0;
/// The terminal moraine: a ridge where `once` crosses `MORAINE_FRONT`, `MORAINE_WIDTH` wide in
/// the weight, `MORAINE_HEIGHT_M` high on flat valley floors and lowlands.
const MORAINE_FRONT: f32 = 0.5;
const MORAINE_WIDTH: f32 = 0.06;
const MORAINE_HEIGHT_M: f32 = 22.0;
/// Drumlins: lattice spacing (cells), semi-axes (cells, along and across the flow), height (m).
const DRUMLIN_SPACING: i64 = 6;
const DRUMLIN_AXES: (f32, f32) = (3.0, 1.2);
const DRUMLIN_HEIGHT_M: (f32, f32) = (10.0, 25.0);
const LOWLAND_H: f32 = 600.0;
/// Outwash lies beyond the front where `once` is between these; kettles on a lattice in it.
const OUTWASH_ONCE: (f32, f32) = (0.15, 0.45);
const KETTLE_SPACING: i64 = 7;
const KETTLE_DEPTH_M: (f32, f32) = (3.0, 8.0);
/// One cell in this many of low glaciated ground carries an erratic.
const ERRATIC_ONE_IN: f32 = 250.0;

/// How much the ice shaped a tile, and whether it lies there still: weights in 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glaciation {
    pub once: f32,
    pub now: f32,
}

/// The ice's weights for a world tile, from the world's annual mean temperature and latitude.
pub fn glaciated(world: &WorldData, tile: (usize, usize)) -> Option<Glaciation> {
    let (x, y) = (tile.0.min(world.width - 1), tile.1.min(world.height - 1));
    let t = *world.temperature.get(x, y);
    let lat = (90.0 - (y as f32 + 0.5) / world.height as f32 * 180.0).abs();
    let ice_age = smoothstep(ICE_AGE_TEMP_C.0, ICE_AGE_TEMP_C.1, t) * smoothstep(ICE_AGE_LAT.0, ICE_AGE_LAT.1, lat);
    let alpine = smoothstep(ALPINE_TEMP_C.0, ALPINE_TEMP_C.1, t);
    let once = ice_age.max(alpine);
    let now = smoothstep(0.0, -4.0, t);
    if once <= 0.0 && now <= 0.0 { None } else { Some(Glaciation { once, now }) }
}

/// The ice's weights over the whole world, to sample smoothly between tiles.
pub struct IceMap {
    once: Tilemap<f32>,
}

impl IceMap {
    pub fn new(world: &WorldData) -> Self {
        let mut once = Tilemap::new_with(world.width, world.height, 0.0f32);
        for y in 0..world.height {
            for x in 0..world.width {
                if let Some(g) = glaciated(world, (x, y)) { once.set(x, y, g.once); }
            }
        }
        IceMap { once }
    }

    /// `once` at tile coordinates (tile centres at integers).
    pub fn once(&self, u: f32, v: f32) -> f32 { sample_tile(&self.once, u, v) }

    /// The direction the ice flowed at (u, v): down the gradient of `once`, toward the front
    /// (unit vector in tile coordinates; (1, 0) where the field is flat).
    pub fn flow(&self, u: f32, v: f32) -> (f32, f32) {
        let e = 0.5;
        let dx = self.once(u + e, v) - self.once(u - e, v);
        let dy = self.once(u, v + e) - self.once(u, v - e);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 { (1.0, 0.0) } else { (-dx / len, -dy / len) }
    }
}

/// Raise the terminal moraine along the ice front and the drumlins on the low glaciated
/// ground, before the rivers wear them: pure functions of position and the base heights.
pub fn shape_before(h: &mut [f32], g: &ChunkGrid) {
    let gw = g.gw;
    let ice = IceMap::new(g.world);
    // Any old ice over this chunk at all?
    let (u0, v0) = g.to_world(0, 0);
    let (u1, v1) = g.to_world(gw - 1, gw - 1);
    let peak = [ice.once(u0, v0), ice.once(u1, v0), ice.once(u0, v1), ice.once(u1, v1), ice.once((u0 + u1) / 2.0, (v0 + v1) / 2.0)]
        .into_iter().fold(0.0f32, f32::max);
    if peak <= 0.0 { return; }
    let k = scale(g.cell_m);
    let slope = chunk_slopes(h, g);
    let (ax0, ay0) = g.abs_cell(0, 0);
    // The local low line (the valley floors and every hollow's drain) is never raised: a ridge
    // across it would dam the water behind it, and a river cuts its gap through a moraine. Lakes
    // behind the front come from the ground's own hollows, not from the ridge.
    let low = super::rock::min_filter(h, gw, gw, 3);
    let low_line: Vec<bool> = (0..gw * gw).map(|c| h[c] - low[c] < 5.0).collect();
    let on_low_line = |c: usize| low_line[c];
    // The moraine: a ridge along the front, on gentle ground, lumpy along its length.
    for j in 0..gw {
        for i in 0..gw {
            let c = j * gw + i;
            if h[c] <= 0.0 || slope[c] > 0.04 * k || on_low_line(c) { continue; }
            let (u, v) = g.to_world(i, j);
            let once = ice.once(u, v);
            let bell = (-(once - MORAINE_FRONT).powi(2) / (2.0 * MORAINE_WIDTH * MORAINE_WIDTH)).exp();
            if bell < 0.05 { continue; }
            let (ax, ay) = g.abs_cell(i, j);
            let lump = 0.6 + 0.8 * unit(ax.div_euclid(3), ay.div_euclid(3), 0x3A);
            h[c] += MORAINE_HEIGHT_M * bell * lump;
        }
    }
    // Drumlins: ellipses along the ice flow on low flat glaciated ground.
    let mut ellipses = Vec::new();
    lattice(ax0, ay0, gw, gw, DRUMLIN_SPACING, 0xD4, |cx, cy, accept, _, uh| {
        if accept > 0.6 { return; }
        ellipses.push((cx, cy, DRUMLIN_HEIGHT_M.0 + (DRUMLIN_HEIGHT_M.1 - DRUMLIN_HEIGHT_M.0) * uh));
    });
    for (cx, cy, height) in ellipses {
        let (i, j) = (cx - ax0, cy - ay0);
        if i < 0 || j < 0 || i >= gw as i64 || j >= gw as i64 { continue; }
        let c = j as usize * gw + i as usize;
        if h[c] <= 0.0 || h[c] > LOWLAND_H || slope[c] > 0.03 * k { continue; }
        let (u, v) = g.to_world(i as usize, j as usize);
        if ice.once(u, v) < 0.6 { continue; }
        let (fx, fy) = ice.flow(u, v);
        let (a, b) = (DRUMLIN_AXES.0, DRUMLIN_AXES.1);
        let r = a.ceil() as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                let (x, y) = (i + dx, j + dy);
                if x < 0 || y < 0 || x >= gw as i64 || y >= gw as i64 { continue; }
                let (dx, dy) = (dx as f32, dy as f32);
                let along = dx * fx + dy * fy;
                let across = -dx * fy + dy * fx;
                let q = (along / a).powi(2) + (across / b).powi(2);
                if q >= 1.0 { continue; }
                let cc = y as usize * gw + x as usize;
                if h[cc] > 0.0 && !on_low_line(cc) { h[cc] += height * (1.0 - q); }
            }
        }
    }
}

/// Trunk cells (catchment above the trunk share) and, for every cell within the U half-width of
/// one, the distance (cells) and the trunk's height: a multi-source breadth-first walk.
fn valley_distance(h: &[f32], acc: &[f32], w: usize, ht: usize, s: usize, k: f32, land_ok: impl Fn(usize) -> bool) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = w * ht;
    let trunk_min = TRUNK_SHARE * (s * s) as f32;
    let mut dist = vec![f32::MAX; n];
    let mut floor = vec![0.0f32; n];
    let mut width = vec![0.0f32; n];
    let mut queue = std::collections::VecDeque::new();
    for c in 0..n {
        if land_ok(c) && acc[c] >= trunk_min {
            dist[c] = 0.0;
            floor[c] = h[c];
            width[c] = (U_WIDTH.0 + U_WIDTH.1 * (acc[c] / (0.05 * (s * s) as f32)).sqrt().min(1.0)) * k;
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        let (x, y) = (c % w, c / w);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= ht as i64 { continue; }
                let q = ny as usize * w + nx as usize;
                let step = if dx != 0 && dy != 0 { std::f32::consts::SQRT_2 } else { 1.0 };
                let nd = dist[c] + step;
                if nd < dist[q] && nd <= width[c] && land_ok(q) {
                    dist[q] = nd;
                    floor[q] = floor[c];
                    width[q] = width[c];
                    queue.push_back(q);
                }
            }
        }
    }
    (dist, floor, width)
}

/// Valley heads: trunk cells with no trunk cell draining into them.
fn is_valley_head(c: usize, acc: &[f32], recv: &[u32], w: usize, ht: usize, trunk_min: f32) -> bool {
    let (x, y) = (c % w, c / w);
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            if dx == 0 && dy == 0 { continue; }
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= ht as i64 { continue; }
            let q = ny as usize * w + nx as usize;
            if recv[q] == c as u32 && acc[q] >= trunk_min { return false; }
        }
    }
    true
}

/// Widen the trunk valleys to U sections, cut the cirque bowls and the kettle holes, on the
/// chunk's drainage from the evolution's last step.
pub fn shape_after(h: &mut [f32], g: &ChunkGrid, d: &Drainage, acc: &[f32]) {
    let gw = g.gw;
    let ice = IceMap::new(g.world);
    let (u0, v0) = g.to_world(0, 0);
    let (u1, v1) = g.to_world(gw - 1, gw - 1);
    let peak = [ice.once(u0, v0), ice.once(u1, v0), ice.once(u0, v1), ice.once(u1, v1), ice.once((u0 + u1) / 2.0, (v0 + v1) / 2.0)]
        .into_iter().fold(0.0f32, f32::max);
    if peak <= 0.3 { return; }
    let k = scale(g.cell_m);
    let land_ok = |c: usize| h[c] > 0.0 && d.filled[c] - h[c] < 0.5;
    let (dist, floor, width) = valley_distance(h, acc, gw, gw, g.s, k, land_ok);
    let high4 = max_filter(h, gw, gw, 4);
    let trunk_min = TRUNK_SHARE * (g.s * g.s) as f32;
    let (ax0, ay0) = g.abs_cell(0, 0);
    let mut out = h.to_vec();
    for j in 0..gw {
        for i in 0..gw {
            let c = j * gw + i;
            if h[c] <= 0.0 { continue; }
            let (u, v) = g.to_world(i, j);
            let once = ice.once(u, v);
            if once < 0.3 { continue; }
            // The U: the flank within the half-width drops toward the floor on a parabola.
            if dist[c] > 0.0 && dist[c] < f32::MAX && h[c] > floor[c] {
                let t = (dist[c] / width[c]).min(1.0);
                let target = floor[c] + (h[c] - floor[c]) * t * t;
                out[c] = h[c] - once * (h[c] - target);
            }
        }
    }
    // Cirques at the valley heads high in the relief: a bowl with a flat floor.
    let mut bowls = Vec::new();
    for c in 0..gw * gw {
        if !land_ok(c) || acc[c] < trunk_min * 0.5 || h[c] < CIRQUE_MIN_H { continue; }
        if high4[c] - h[c] < CIRQUE_RELIEF_M || !is_valley_head(c, acc, &d.recv, gw, gw, trunk_min * 0.5) { continue; }
        let (u, v) = g.to_world(c % gw, c / gw);
        let once = ice.once(u, v);
        if once < 0.3 { continue; }
        bowls.push((c, once));
    }
    for (c, once) in bowls {
        let (i, j) = ((c % gw) as i64, (c / gw) as i64);
        let r = (CIRQUE_RADIUS * k).max(1.5);
        let ri = r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (x, y) = (i + dx, j + dy);
                if x < 0 || y < 0 || x >= gw as i64 || y >= gw as i64 { continue; }
                let dd = ((dx * dx + dy * dy) as f32).sqrt() / r;
                if dd >= 1.0 { continue; }
                let cc = y as usize * gw + x as usize;
                if h[cc] <= 0.0 { continue; }
                let cut = CIRQUE_DEPTH_M * once * smoothstep(1.0, 0.45, dd);
                out[cc] = (out[cc] - cut).max(0.5);
            }
        }
    }
    // Kettle holes in the outwash beyond the front.
    lattice(ax0, ay0, gw, gw, KETTLE_SPACING, 0xE7, |cx, cy, accept, _, ud| {
        if accept > 0.5 { return; }
        let (i, j) = (cx - ax0, cy - ay0);
        if i < 0 || j < 0 || i >= gw as i64 || j >= gw as i64 { return; }
        let c = j as usize * gw + i as usize;
        if h[c] <= 0.0 || h[c] > LOWLAND_H { return; }
        let (u, v) = g.to_world(i as usize, j as usize);
        let once = ice.once(u, v);
        if once < OUTWASH_ONCE.0 || once > OUTWASH_ONCE.1 { return; }
        let depth = KETTLE_DEPTH_M.0 + (KETTLE_DEPTH_M.1 - KETTLE_DEPTH_M.0) * ud;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (x, y) = (i + dx, j + dy);
                if x < 0 || y < 0 || x >= gw as i64 || y >= gw as i64 { continue; }
                let cc = y as usize * gw + x as usize;
                let dd = ((dx * dx + dy * dy) as f32).sqrt() / 1.6;
                if dd < 1.0 && h[cc] > 0.0 { out[cc] = (out[cc] - depth * (1.0 - dd * dd)).max(0.5); }
            }
        }
    });
    h.copy_from_slice(&out);
}

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let n = gw * gh;
    let ice = IceMap::new(f.world);
    let k = scale(f.cell_m);
    // The ice's weight at every cell, and whether any of the region knew it.
    let mut once = vec![0.0f32; n];
    let mut any = false;
    for j in 0..gh {
        for i in 0..gw {
            let (u, v) = f.to_world(i, j);
            let o = ice.once(u, v);
            once[j * gw + i] = o;
            any |= o > 0.0 || f.temp[j * gw + i] < PATTERNED_TEMP_C.1;
        }
    }
    if !any { return; }
    let slope = slopes(f);
    let land_ok = |c: usize| !f.is_sea(c) && f.lake_depth(c) == 0.0;
    let thr = |c: usize| river_threshold(f.s) / (0.25 + f.moist[c]);
    let trunk_min = TRUNK_SHARE * (f.s * f.s) as f32;
    let (dist, _floor, width) = valley_distance(f.h, f.acc, gw, gh, f.s, k, |c| f.h[c] > 0.0);
    let high4 = max_filter(f.h, gw, gh, 4);
    let (ax0, ay0) = f.abs_cell(0, 0);
    let cell_of = |cx: i64, cy: i64| -> Option<usize> {
        let (i, j) = (cx - ax0, cy - ay0);
        if i < 0 || j < 0 || i >= gw as i64 || j >= gh as i64 { None } else { Some(j as usize * gw + i as usize) }
    };

    // Ice today: glaciers in the valleys and on the caps, snowfields above the snow line.
    for c in 0..n {
        if f.is_sea(c) || land[c] != Landform::None { continue; }
        let t = f.temp[c];
        if t < GLACIER_TEMP_C {
            let valley = f.acc[c] >= thr(c) * 0.25 || dist[c] < f32::MAX;
            let cap = slope[c] < 0.04 * k;
            land[c] = if valley || cap { Landform::Glacier } else { Landform::Snowfield };
        } else if t < SNOW_TEMP_C && f.lake_depth(c) == 0.0 {
            land[c] = Landform::Snowfield;
        }
    }
    // Cirques and tarns at the heads, aretes between, the U-valleys' flanks.
    for c in 0..n {
        if !land_ok(c) || once[c] < 0.3 || f.acc[c] < trunk_min * 0.5 || f.h[c] < CIRQUE_MIN_H { continue; }
        if high4[c] - f.h[c] < CIRQUE_RELIEF_M || !is_valley_head(c, f.acc, f.recv, gw, gh, trunk_min * 0.5) { continue; }
        let (i, j) = ((c % gw) as i64, (c / gw) as i64);
        let r = (CIRQUE_RADIUS * k).max(1.5);
        let ri = r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if ((dx * dx + dy * dy) as f32).sqrt() >= r { continue; }
                if let Some(cc) = cell_of(ax0 + i + dx, ay0 + j + dy) {
                    if f.is_sea(cc) { continue; }
                    land[cc] = if f.lake_depth(cc) > 0.0 { Landform::Tarn } else if land[cc] == Landform::None { Landform::Cirque } else { land[cc] };
                }
            }
        }
    }
    for j in 1..gh - 1 {
        for i in 1..gw - 1 {
            let c = j * gw + i;
            if !land_ok(c) || land[c] != Landform::None || once[c] < 0.3 { continue; }
            let h = f.h[c];
            let mean8 = (f.h[c - 1] + f.h[c + 1] + f.h[c - gw] + f.h[c + gw] + f.h[c - gw - 1] + f.h[c - gw + 1] + f.h[c + gw - 1] + f.h[c + gw + 1]) / 8.0;
            if h > CIRQUE_MIN_H && h - mean8 > ARETE_CONVEX_M * k && slope[c] > 0.1 * k {
                land[c] = Landform::Arete;
            } else if dist[c] > 0.0 && dist[c] < width[c] && f.acc[c] < thr(c) {
                land[c] = Landform::UValley;
            }
        }
    }
    // The front and the ground beyond it: moraine, outwash, kettles; the low ground's drumlins
    // and erratics.
    let mut ellipses = Vec::new();
    lattice(ax0, ay0, gw, gh, DRUMLIN_SPACING, 0xD4, |cx, cy, accept, _, _| { if accept <= 0.6 { ellipses.push((cx, cy)); } });
    let mut kettles = Vec::new();
    lattice(ax0, ay0, gw, gh, KETTLE_SPACING, 0xE7, |cx, cy, accept, _, _| { if accept <= 0.5 { kettles.push((cx, cy)); } });
    for c in 0..n {
        if !land_ok(c) || land[c] != Landform::None { continue; }
        let o = once[c];
        let (i, j) = (c % gw, c / gw);
        let bell = (-(o - MORAINE_FRONT).powi(2) / (2.0 * MORAINE_WIDTH * MORAINE_WIDTH)).exp();
        if bell > 0.4 && slope[c] < 0.06 * k && f.acc[c] < thr(c) {
            land[c] = Landform::Moraine;
        } else if o >= OUTWASH_ONCE.0 && o <= OUTWASH_ONCE.1 && slope[c] < 0.02 * k && f.h[c] < LOWLAND_H && f.acc[c] < thr(c) {
            land[c] = Landform::Outwash;
        } else if o > 0.5 && slope[c] < 0.05 * k && f.h[c] < LOWLAND_H {
            let (ax, ay) = f.abs_cell(i, j);
            if unit(ax, ay, 0xE8) < 1.0 / ERRATIC_ONE_IN { land[c] = Landform::Erratic; }
        }
        if land[c] == Landform::None && f.temp[c] >= PATTERNED_TEMP_C.0 && f.temp[c] < PATTERNED_TEMP_C.1 && slope[c] < 0.03 * k && f.acc[c] < thr(c) {
            land[c] = Landform::PatternedGround;
        }
    }
    for (cx, cy) in ellipses {
        let Some(c) = cell_of(cx, cy) else { continue };
        if !land_ok(c) || f.h[c] > LOWLAND_H || slope[c] > 0.03 * k || once[c] < 0.6 { continue; }
        let (u, v) = f.to_world(c % gw, c / gw);
        let (fx, fy) = ice.flow(u, v);
        let (a, b) = DRUMLIN_AXES;
        let r = a.ceil() as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                let (dxf, dyf) = (dx as f32, dy as f32);
                let along = dxf * fx + dyf * fy;
                let across = -dxf * fy + dyf * fx;
                if (along / a).powi(2) + (across / b).powi(2) >= 1.0 { continue; }
                if let Some(cc) = cell_of(cx + dx, cy + dy) {
                    if land_ok(cc) && matches!(land[cc], Landform::None | Landform::Erratic | Landform::PatternedGround) { land[cc] = Landform::Drumlin; }
                }
            }
        }
    }
    for (cx, cy) in kettles {
        let Some(c) = cell_of(cx, cy) else { continue };
        if f.is_sea(c) || f.h[c] > LOWLAND_H || once[c] < OUTWASH_ONCE.0 || once[c] > OUTWASH_ONCE.1 { continue; }
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if ((dx * dx + dy * dy) as f32).sqrt() >= 1.6 { continue; }
                if let Some(cc) = cell_of(cx + dx, cy + dy) {
                    if !f.is_sea(cc) && matches!(land[cc], Landform::None | Landform::Outwash) { land[cc] = Landform::Kettle; }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::zoom::{accumulate, route};

    /// The ice's rule: cold high latitudes were glaciated, the warm never, cold mountains
    /// anywhere; the map samples smoothly.
    #[test]
    fn ice_follows_the_climate() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let ice = IceMap::new(&world);
        for y in 0..world.height {
            for x in 0..world.width {
                let t = *world.temperature.get(x, y);
                let g = glaciated(&world, (x, y));
                if t > 8.0 { assert!(g.map(|g| g.once).unwrap_or(0.0) == 0.0, "warm ground glaciated at {x},{y}"); }
                if t < -4.0 { assert!(g.map(|g| g.once).unwrap_or(0.0) >= 0.99, "cold ground not glaciated at {x},{y}"); }
                let o = ice.once(x as f32, y as f32);
                assert!((o - g.map(|g| g.once).unwrap_or(0.0)).abs() < 1e-4);
            }
        }
    }

    /// Shaped twice in chunks that overlap, the same world cells get the same ground.
    #[test]
    fn chunks_shape_the_same_ice() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let s = 32usize;
        let gw = 3 * s;
        // The coldest tile row of the world: the ice must have been there.
        let (mut cold_y, mut cold_t) = (0usize, f32::MAX);
        for y in 2..world.height - 2 {
            let t: f32 = (0..world.width).map(|x| *world.temperature.get(x, y)).sum::<f32>() / world.width as f32;
            if t < cold_t { cold_t = t; cold_y = y; }
        }
        let make = |gx0: i64, gy0: i64| {
            let g = ChunkGrid { world: &world, gw, s, gx0, gy0, cell_m: 610.0, seed: 1 };
            let mut h: Vec<f32> = (0..gw * gw).map(|c| { let (ax, ay) = g.abs_cell(c % gw, c / gw); 300.0 + unit(ax, ay, 9) * 3.0 + 0.2 * (ay as f32) }).collect();
            shape_before(&mut h, &g);
            (g, h)
        };
        let (ga, ha) = make(10, cold_y as i64 - 1);
        let (gb, hb) = make(11, cold_y as i64 - 1);
        // The chunks' border cells see one-sided slopes; the central region a chunk gives out
        // lies half a tile in, so compare the overlap inside that.
        let mut compared = 0;
        for j in 8..gw - 8 {
            for i in s + 8..gw - 8 {
                let (ax, ay) = ga.abs_cell(i, j);
                assert_eq!((ax, ay), gb.abs_cell(i - s, j));
                assert_eq!(ha[j * gw + i].to_bits(), hb[j * gw + i - s].to_bits(), "differs at {ax},{ay}");
                compared += 1;
            }
        }
        assert!(compared > 0);
    }

    /// Nothing glacial is labelled on the sea, and glaciers only where it is cold.
    #[test]
    fn labels_stay_on_cold_land() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let (gw, gh, s) = (64usize, 64usize, 32usize);
        let h: Vec<f32> = (0..gw * gh).map(|c| if c / gw < 8 { -10.0 } else { 100.0 + ((c % gw) as f32) * 3.0 }).collect();
        let d = route(&h, gw, gh, 0.0);
        let acc = accumulate(&d, &vec![0.0; gw * gh]);
        let temp: Vec<f32> = (0..gw * gh).map(|c| if c % gw > 48 { -6.0 } else { 10.0 }).collect();
        let moist = vec![0.5f32; gw * gh];
        let f = RegionFields { world: &world, gw, gh, s, gx0: 10, gy0: 10, margin: 0, cell_m: 610.0, seed: 1, h: &h, filled: &d.filled, recv: &d.recv, acc: &acc, temp: &temp, moist: &moist };
        let rock = vec![RockType::Granite; gw * gh];
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        for c in 0..gw * gh {
            if h[c] <= 0.0 { assert_eq!(land[c], Landform::None); }
            if matches!(land[c], Landform::Glacier | Landform::Snowfield) { assert!(temp[c] < SNOW_TEMP_C); }
        }
        assert!(land.iter().any(|&l| matches!(l, Landform::Glacier | Landform::Snowfield)), "the cold side carries ice");
    }
}
