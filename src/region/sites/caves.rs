//! Caves as systems (ROADMAP Stage 5): cave graphs planned per world tile in absolute region
//! cells and metres above the sea (`plan::CaveGraph`), from the karst's sinks and resurgences,
//! the volcanoes' flanks, the hard coasts and the cliffs: karst conduits down the hydraulic
//! gradient to a resurgence (vadose canyons above the water table, phreatic tubes below,
//! chambers at junctions and breakdown, shafts on joints, old dry levels), lava tubes with
//! skylights, sea caves, rock shelters, talus and ice caves, mine adits. The embark carves them
//! (`local::caves`); the region draws their mouths; neighbouring embarks share one graph.
//!
//! Deterministic per world tile: reads only the tile and its neighbours, keys every choice on
//! absolute position. The stream's way underground follows the steepest descent on the raw
//! heights (a pure function of position), not the window's routing, whose receivers near the
//! margin depend on where the window ends; it stops at a pit, the sea, or where it leaves the
//! carbonate rock, within two tiles of the sink.
//!
//! A karst cave, from its walk-in mouth: the spring (`Resurgence`) at the valley floor, the
//! `Mouth` a few dozen metres up the slope, a dry upper gallery (an old level the stream left
//! when the valley cut down) running upstream to a breakdown `Chamber`, a `Shaft` from the
//! chamber's floor to the active stream level (`Vadose`: tall, narrow, the stream in the floor,
//! walkable with wet feet) that climbs on to the doline (`Sink`) where the stream goes under,
//! and a flooded `Phreatic` sump from under the chamber down to the spring. Where limestone lies
//! at the surface the chamber's shaft goes on down to the first cavern layer (`local::caverns`).

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::{is_carbonate, CaveGraph, CaveKind, CaveNode, CaveNodeKind, CavePassage, Landform, PassageKind, Place, PlaceKind};

use super::{Core, SitePlan};

/// Levels of the tile's rock stack within which limestone makes the tile karst-prone (30 m).
const KARST_STACK_LEVELS: i32 = 15;
/// Metres a sink must stand above its tile's lowest land for a cave to have a gradient to work on.
const SINK_RELIEF_M: f32 = 15.0;
/// The stream's way underground is followed at most this many tiles from the sink.
const CHAIN_TILES: usize = 2;
/// Sinks of one tile keep this far apart (cells).
const SINK_SPACING: i64 = 20;
/// Metres of ground kept over every passage.
const ROOF_M: f32 = 8.0;
/// The first cavern layer's middle below the smooth surface (`local::caverns::DEPTH_M[0]`).
const FIRST_CAVERN_M: f32 = 22.0;
/// Carbonate tiles with a cave: one in this many.
const KARST_ONE_IN: u64 = 3;
/// A karst cave's stream runs at least this far under its mouth.
const MIN_DROP_M: f32 = 10.0;
/// Coast tiles with a sea cave: one in this many.
const SEA_CAVE_ONE_IN: u64 = 5;

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn unit(a: i64, b: i64, salt: u64) -> f32 { (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0 }

/// Whether a world tile's rock stack holds limestone near enough the surface for water to work it.
fn karst_tile(f: &RegionFields, tile: (usize, usize)) -> bool {
    let Some(hs) = f.world.handshakes.as_ref() else { return false };
    let mut top = 0i32;
    for l in &hs.get(tile.0, tile.1).tile.rock_stack {
        if l.rock_type == RockType::Limestone && top <= KARST_STACK_LEVELS { return true; }
        top += l.thickness as i32;
        if top > KARST_STACK_LEVELS { break; }
    }
    false
}

/// What the planner knows of one window: the fields, the labels, and per-tile answers it has
/// already worked out.
struct Ctx<'a> {
    f: &'a RegionFields<'a>,
    land: &'a [Landform],
    rock: &'a [RockType],
    karst: Vec<((usize, usize), bool)>,
}

impl<'a> Ctx<'a> {
    fn karst_at(&mut self, i: usize, j: usize) -> bool {
        let t = self.f.tile_of(i, j);
        if let Some(v) = self.karst.iter().find(|v| v.0 == t) { return v.1; }
        let v = karst_tile(self.f, t);
        self.karst.push((t, v));
        v
    }
    /// Carbonate rock at the surface, or limestone close under it.
    fn carbonate(&mut self, i: usize, j: usize) -> bool {
        is_carbonate(self.rock[self.f.idx(i, j)]) || self.karst_at(i, j)
    }
    fn channel(&self, k: usize, j: usize) -> bool {
        let s2 = (self.f.s * self.f.s) as f32;
        self.f.acc[k] > 0.02 * s2 / (0.25 + self.f.moist[k]) && self.f.h[k] > 0.0 && self.f.lake_depth(k) == 0.0 && self.f.lat_cos(j) > 0.0
    }
    /// Ground height (m) at a fractional cell position (bilinear between cells; cell k lies at
    /// k, the frame every place uses).
    fn ground(&self, x: f64, y: f64) -> f32 {
        let (gw, gh) = (self.f.gw, self.f.gh);
        let (fx, fy) = (x.clamp(0.0, (gw - 1) as f64), y.clamp(0.0, (gh - 1) as f64));
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(gw - 1), (y0 + 1).min(gh - 1));
        let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
        let h = |i: usize, j: usize| self.f.h[j * gw + i];
        (h(x0, y0) * (1.0 - tx) + h(x1, y0) * tx) * (1.0 - ty) + (h(x0, y1) * (1.0 - tx) + h(x1, y1) * tx) * ty
    }
    /// The steepest-descent neighbour of cell (i, j) on the raw heights, if any lies lower.
    fn descend(&self, i: usize, j: usize) -> Option<(usize, usize)> {
        let (gw, gh) = (self.f.gw, self.f.gh);
        let here = self.f.h[self.f.idx(i, j)];
        let mut best: Option<((usize, usize), f32)> = None;
        for dj in -1i64..=1 {
            for di in -1i64..=1 {
                if di == 0 && dj == 0 { continue; }
                let (x, y) = (i as i64 + di, j as i64 + dj);
                if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 { continue; }
                let (x, y) = (x as usize, y as usize);
                let dist = if di != 0 && dj != 0 { std::f32::consts::SQRT_2 } else { 1.0 };
                let drop = (here - self.f.h[self.f.idx(x, y)]) / dist;
                if drop > 0.0 && best.map_or(true, |b| drop > b.1) { best = Some(((x, y), drop)); }
            }
        }
        best.map(|b| b.0)
    }
}

/// One cave's nodes and passages being laid out, in region cells (grid-local) and metres.
struct Build {
    nodes: Vec<CaveNode>,
    passages: Vec<CavePassage>,
}

impl Build {
    fn node(&mut self, f: &RegionFields, i: f64, j: f64, z_m: f32, kind: CaveNodeKind, width_m: f32, height_m: f32) -> u32 {
        // Grid-local fractional cells to absolute cells.
        let (ax, ay) = ((f.gx0 * f.s as i64) as f64 + i, (f.gy0 * f.s as i64) as f64 + j);
        self.nodes.push(CaveNode { x: ax, y: ay, z_m, kind, width_m, height_m });
        (self.nodes.len() - 1) as u32
    }
    fn link(&mut self, a: u32, b: u32, kind: PassageKind, width_m: f32, height_m: f32, stream: bool) {
        self.passages.push(CavePassage { a, b, kind, width_m, height_m, stream });
    }
}

/// Plan the caves of the window's core tiles. Runs after `natural::plan`, so a natural cave mouth
/// within two cells of one of ours is moved onto our mouth and linked.
pub fn plan(f: &RegionFields, land: &[Landform], rock: &[RockType], core: Core, out: &mut SitePlan) {
    let mut ctx = Ctx { f, land, rock, karst: Vec::new() };
    let first = out.caves.len();
    for (tx, ty) in core.tiles() {
        let (i0, j0) = (((tx - f.gx0) * f.s as i64) as usize, ((ty - f.gy0) * f.s as i64) as usize);
        if i0 + f.s > f.gw || j0 + f.s > f.gh { continue; }
        plan_karst(&mut ctx, i0, j0, out);
        plan_lava_tubes(&mut ctx, i0, j0, out);
        plan_sea_caves(&mut ctx, i0, j0, out);
        plan_rock_shelters(&mut ctx, i0, j0, out);
    }
    // Our mouths and the natural wonders' cave mouths are one thing.
    let natural: Vec<usize> = out.places.iter().enumerate().filter(|(_, p)| p.cave.is_none() && matches!(p.kind, PlaceKind::CaveMouth | PlaceKind::RockShelter)).map(|(k, _)| k).collect();
    let mut mine: Vec<Place> = Vec::new();
    for (g, cave) in out.caves.iter().enumerate().skip(first) {
        let Some(m) = cave.nodes.iter().find(|n| n.kind == CaveNodeKind::Mouth) else { continue };
        let kind = if cave.kind == CaveKind::RockShelter { PlaceKind::RockShelter } else { PlaceKind::CaveMouth };
        let near = natural.iter().copied().find(|&k| {
            let p = &out.places[k];
            p.kind == kind && p.cave.is_none() && (p.x - m.x).abs() <= 2.0 && (p.y - m.y).abs() <= 2.0
        });
        let deepest = cave.nodes.iter().map(|n| n.z_m).fold(f32::MAX, f32::min);
        if let Some(k) = near {
            let p = &mut out.places[k];
            p.x = m.x;
            p.y = m.y;
            p.cave = Some(g as u32);
            p.z_m = Some(deepest);
            if p.cause.is_empty() { p.cause = cave_cause(cave); }
            if p.contents.is_empty() { p.contents = cave_contents(cave); }
        } else {
            let mut p = Place::point(kind, m.x, m.y, cave.tile);
            p.cause = cave_cause(cave);
            p.description = cave_description(cave);
            p.contents = cave_contents(cave);
            p.z_m = Some(deepest);
            p.cave = Some(g as u32);
            p.rank = match cave.kind { CaveKind::Karst => 140, CaveKind::LavaTube => 160, _ => 190 };
            mine.push(p);
        }
    }
    out.places.extend(mine);
}

fn cave_cause(c: &CaveGraph) -> String {
    match c.kind {
        CaveKind::Karst => "the limestone here is hollowed by water: a stream that sinks on the high ground comes out again at the valley floor".to_string(),
        CaveKind::LavaTube => "a lava flow ran on under its own crust and drained, leaving its tube".to_string(),
        CaveKind::SeaCave => "the swell has cut into the hard rock of the headland".to_string(),
        CaveKind::RockShelter => "the soft bed under a harder one has weathered back, leaving an overhang".to_string(),
        CaveKind::Talus => "boulders fallen from the face lean together over a hollow".to_string(),
        CaveKind::Ice => "meltwater runs under the ice".to_string(),
        CaveKind::Mine => "people cut it".to_string(),
    }
}

fn cave_description(c: &CaveGraph) -> String {
    let deepest = c.nodes.iter().map(|n| n.z_m).fold(f32::MAX, f32::min);
    let mouth = c.nodes.iter().find(|n| n.kind == CaveNodeKind::Mouth).map(|n| n.z_m).unwrap_or(deepest);
    let drop = (mouth - deepest).max(0.0);
    match c.kind {
        CaveKind::Karst => format!("A walk-in mouth above a spring. A dry gallery leads back to a chamber of fallen blocks, and a drop of {:.0} m from its floor reaches the stream, which can be followed up its canyon to the doline where it goes under.", drop),
        CaveKind::LavaTube => "A tube of glassy black rock running down the slope, open to the sky where its roof fell in.".to_string(),
        CaveKind::SeaCave => "A sea-cut hollow at the foot of the cliff, the swell breaking in its back.".to_string(),
        CaveKind::RockShelter => "A wide overhang at the foot of the face, dry at the back.".to_string(),
        _ => String::new(),
    }
}

fn cave_contents(c: &CaveGraph) -> Vec<String> {
    let h = hash(c.tile.0 as i64, c.tile.1 as i64, 0xCAFE);
    let mut out: Vec<String> = Vec::new();
    match c.kind {
        CaveKind::Karst => {
            out.push("a cold stream in the dark".into());
            out.push("bats, and their guano on the floor".into());
            if h % 3 == 0 { out.push("the bones of cave bears in the old gallery".into()); }
            if h % 5 == 1 { out.push("a sump of still black water".into()); }
        }
        CaveKind::LavaTube => { out.push("walls of glassy black rock".into()); if h % 2 == 0 { out.push("bats under the skylights".into()); } }
        CaveKind::SeaCave => { out.push("the sea breaking in the dark".into()); out.push("seals on the ledge at low water".into()); }
        CaveKind::RockShelter => out.push("a dry floor under the overhang".into()),
        _ => {}
    }
    out
}

/// Karst: sinks on the carbonate high ground of the tile, each with its underground stream to
/// a resurgence, as one cave graph per sink.
fn plan_karst(ctx: &mut Ctx, i0: usize, j0: usize, out: &mut SitePlan) {
    let f = ctx.f;
    let s = f.s;
    let tile = f.tile_of(i0, j0);
    // The tile's lowest land, for the relief a sink needs.
    let mut carb_cells = 0usize;
    let mut low = f32::MAX;
    for j in j0..j0 + s { for i in i0..i0 + s {
        let k = f.idx(i, j);
        if f.h[k] <= 0.0 { continue; }
        low = low.min(f.h[k]);
        if ctx.carbonate(i, j) { carb_cells += 1; }
    } }
    if carb_cells < s * s / 12 { return; }
    // One cave a tile, on one carbonate tile in three (the natural wonders' sinks and cliff
    // mouths are the other openings).
    if hash(tile.0 as i64, tile.1 as i64, 0x5157) % KARST_ONE_IN != 0 { return; }
    // Candidate sinks: flat carbonate ground off the channels, above the valley floor, ranked by
    // hash; kept apart.
    let mut cands: Vec<(u64, usize, usize)> = Vec::new();
    for j in (j0 + 2..j0 + s - 2).step_by(2) { for i in (i0 + 2..i0 + s - 2).step_by(2) {
        let k = f.idx(i, j);
        if f.h[k] <= 0.0 || f.h[k] - low < SINK_RELIEF_M || f.lake_depth(k) > 0.0 || ctx.channel(k, j) || !ctx.carbonate(i, j) { continue; }
        if f.slope(i, j) > 0.06 { continue; }
        let (ax, ay) = f.abs_cell(i, j);
        cands.push((hash(ax, ay, 0x51C0), i, j));
    } }
    cands.sort_unstable();
    let want = 1;
    let mut sinks: Vec<(usize, usize)> = Vec::new();
    for &(_, i, j) in &cands {
        if sinks.len() >= want { break; }
        if sinks.iter().any(|&(x, y)| (x as i64 - i as i64).abs() < SINK_SPACING && (y as i64 - j as i64).abs() < SINK_SPACING) { continue; }
        // The stream's way under: steepest descent until it leaves the carbonate rock, meets a
        // channel or a lake well below the sink, reaches the sea, or sits in a pit.
        let sink_h = f.h[f.idx(i, j)];
        let mut chain: Vec<(usize, usize)> = vec![(i, j)];
        let (mut x, mut y) = (i, j);
        let mut ok = false;
        for _ in 0..CHAIN_TILES * s {
            let Some((nx, ny)) = ctx.descend(x, y) else { ok = chain.len() >= 3; break };
            x = nx; y = ny;
            let k = f.idx(x, y);
            if f.h[k] <= 0.0 { ok = chain.len() >= 3; break; }
            chain.push((x, y));
            let deep = sink_h - f.h[k] >= 12.0;
            if deep && (!ctx.carbonate(x, y) || ctx.channel(k, y) || f.lake_depth(k) > 0.0) { ok = true; break; }
        }
        if !ok || chain.len() < 3 { continue; }
        sinks.push((i, j));
        build_karst(ctx, &chain, tile, out);
    }
}

fn build_karst(ctx: &Ctx, chain: &[(usize, usize)], tile: (usize, usize), out: &mut SitePlan) {
    let f = ctx.f;
    let m = chain.len() - 1;
    let (ax0, ay0) = f.abs_cell(chain[0].0, chain[0].1);
    let salt = hash(ax0, ay0, 0x6A57);
    let u = |k: u64| (hash(ax0, ay0, salt ^ k) % 1000) as f32 / 1000.0;
    // Positions along the chain, with a little lateral wander so passages don't run cell to cell.
    let pos = |t: usize| -> (f64, f64) {
        let (i, j) = chain[t.min(m)];
        let (ai, aj) = f.abs_cell(i, j);
        let wx = (unit(ai, aj, salt) - 0.5) as f64 * 0.6;
        let wy = (unit(ai, aj, salt ^ 1) - 0.5) as f64 * 0.6;
        let (wx, wy) = if t == 0 || t == m { (0.0, 0.0) } else { (wx, wy) };
        (i as f64 + wx, j as f64 + wy)
    };
    let cell_m = f.cell_m as f32;
    let ground = |t: usize| { let (x, y) = pos(t); ctx.ground(x, y) };
    // The resurgence at the chain's end, the mouth a few dozen metres up the slope toward it.
    let (rx, ry) = pos(m);
    let (px, py) = pos(m - 1);
    let dir = ((px - rx), (py - ry));
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1e-6);
    let mouth_off = ((40.0 + 30.0 * u(1)) / cell_m) as f64;
    let (mx, my) = (rx + dir.0 / len * mouth_off, ry + dir.1 / len * mouth_off);
    // The mouth stands on land: where the spring rises in a lake or the sea, the mouth moves
    // back to the chain's last land cell.
    let on_land = |x: f64, y: f64| {
        let (ci, cj) = ((x.round() as i64).clamp(0, f.gw as i64 - 1) as usize, (y.round() as i64).clamp(0, f.gh as i64 - 1) as usize);
        let k = f.idx(ci, cj);
        f.h[k] > 0.0 && f.lake_depth(k) == 0.0
    };
    let (mx, my) = if on_land(mx, my) { (mx, my) } else {
        (1..=m).rev().map(|t| pos(t - 1)).find(|&(x, y)| on_land(x, y)).unwrap_or((mx, my))
    };
    let z_res = ctx.ground(rx, ry);
    let z_mouth = ctx.ground(mx, my).max(z_res + 1.0);
    let mut b = Build { nodes: Vec::new(), passages: Vec::new() };
    let res = b.node(f, rx, ry, z_res, CaveNodeKind::Resurgence, 3.0, 2.0);
    let mouth = b.node(f, mx, my, z_mouth, CaveNodeKind::Mouth, 5.0 + 3.0 * u(2), 3.0 + 2.0 * u(3));
    // The dry upper gallery from the mouth upstream to the chain's middle, rising a little
    // (the old stream's grade), never nearer the ground than the roof allows.
    let mid = (m / 2).max(1);
    let mut upper: Vec<u32> = vec![mouth];
    let mut z_u = z_mouth;
    let mut d_along = 0.0f32;
    for t in (mid..m).rev() {
        let (x, y) = pos(t);
        let (x1, y1) = pos(t + 1);
        d_along += (((x - x1).powi(2) + (y - y1).powi(2)).sqrt() as f32) * cell_m;
        z_u = (z_mouth + 0.01 * d_along).min(ground(t) - ROOF_M);
        let w = 3.0 + 3.0 * unit(t as i64, ax0, salt ^ 7);
        let n = b.node(f, x, y, z_u, CaveNodeKind::Passage, w, 3.0 + 2.0 * unit(t as i64, ay0, salt ^ 8));
        upper.push(n);
    }
    for w in upper.windows(2) { b.link(w[0], w[1], PassageKind::Phreatic, 4.0, 4.0, false); }
    // The breakdown chamber at the gallery's end, and the shaft from its floor to the stream.
    let (cx, cy) = pos(mid);
    let z_ch = z_u;
    let chamber = b.node(f, cx, cy, z_ch, CaveNodeKind::Chamber, 14.0 + 26.0 * u(4), 6.0 + 6.0 * u(5));
    b.link(*upper.last().unwrap(), chamber, PassageKind::Breakdown, 6.0, 5.0, false);
    let drop = 10.0 + 20.0 * u(6);
    // The stream level: `drop` under the chamber and at least `MIN_DROP_M` under the mouth
    // (on flat ground the water loops under the spring's level before rising to it).
    let z_active_mid = (z_ch - drop).min(z_mouth - MIN_DROP_M);
    let shaft_foot = b.node(f, cx, cy, z_active_mid, CaveNodeKind::Shaft, 2.5, 2.0);
    b.link(chamber, shaft_foot, PassageKind::Shaft, 2.5, drop, false);
    // The active stream level: a vadose canyon climbing from under the chamber to the sink's
    // shaft, the stream in its floor.
    let z_sink_floor = (ground(0) - (16.0 + 14.0 * u(7))).max(z_active_mid + 4.0);
    let mut active: Vec<u32> = vec![shaft_foot];
    for t in (1..mid).rev() {
        let (x, y) = pos(t);
        let frac = (mid - t) as f32 / mid as f32;
        let z = (z_active_mid + (z_sink_floor - z_active_mid) * frac).min(ground(t) - ROOF_M);
        let n = b.node(f, x, y, z, CaveNodeKind::Passage, 1.5 + 2.5 * unit(t as i64, ax0, salt ^ 9), 8.0 + 12.0 * unit(t as i64, ay0, salt ^ 10));
        active.push(n);
    }
    let (sx, sy) = pos(0);
    let z_sf = z_sink_floor.min(ground(0) - ROOF_M);
    let sink_foot = b.node(f, sx, sy, z_sf, CaveNodeKind::Shaft, 2.0, 2.0);
    active.push(sink_foot);
    for w in active.windows(2) { b.link(w[0], w[1], PassageKind::Vadose, 2.5, 10.0, true); }
    let sink = b.node(f, sx, sy, ground(0), CaveNodeKind::Sink, 6.0, 2.0);
    b.link(sink_foot, sink, PassageKind::Shaft, 2.0, ground(0) - z_sf, false);
    // The sump: flooded, from under the chamber down to the spring.
    let mut sump: Vec<u32> = vec![shaft_foot];
    let steps = (m - mid).max(1);
    for (k, t) in (mid + 1..=m).enumerate() {
        let frac = (k + 1) as f32 / steps as f32;
        let z = z_active_mid + (z_res - 2.0 - z_active_mid) * frac;
        let (x, y) = if t == m { (rx, ry) } else { pos(t) };
        let n = b.node(f, x, y, z.min(ground(t) - 4.0), CaveNodeKind::Passage, 3.0, 3.0);
        sump.push(n);
    }
    sump.push(res);
    for w in sump.windows(2) { b.link(w[0], w[1], PassageKind::Phreatic, 3.0, 3.0, true); }
    // Where the limestone lies at the surface, the shaft goes on down to the first cavern layer.
    if is_carbonate(ctx.rock[f.idx(chain[mid].0, chain[mid].1)]) {
        let z_cav = ground(mid) - FIRST_CAVERN_M;
        if z_cav < z_active_mid - 6.0 {
            let deep = b.node(f, cx, cy, z_cav, CaveNodeKind::Shaft, 2.0, 2.0);
            b.link(shaft_foot, deep, PassageKind::Shaft, 2.0, z_active_mid - z_cav, false);
        }
    }
    let rock = format!("{:?}", ctx.rock[f.idx(chain[mid].0, chain[mid].1)]).to_lowercase();
    let rock = if rock == "limestone" { rock } else { "limestone".to_string() };
    out.caves.push(CaveGraph { kind: CaveKind::Karst, name: format!("A cave in the {rock}"), nodes: b.nodes, passages: b.passages, tile });
}

/// Lava tubes down a volcano's own tile: from the vent along the steepest descent, a tube just
/// under the surface with skylights where the roof fell in, its mouth at the lower end.
fn plan_lava_tubes(ctx: &mut Ctx, i0: usize, j0: usize, out: &mut SitePlan) {
    let f = ctx.f;
    let s = f.s;
    let tile = f.tile_of(i0, j0);
    let Some(v) = f.world.volcanoes.iter().find(|v| v.x == tile.0 && v.y == tile.1) else { return };
    // The vent: the tile's highest land cell.
    let mut vent: Option<((usize, usize), f32)> = None;
    for j in j0 + 1..j0 + s - 1 { for i in i0 + 1..i0 + s - 1 {
        let h = f.h[f.idx(i, j)];
        if h > 0.0 && vent.map_or(true, |b| h > b.1) { vent = Some(((i, j), h)); }
    } }
    let Some(((vi, vj), _)) = vent else { return };
    let (ax, ay) = f.abs_cell(vi, vj);
    let tubes = 1 + (hash(ax, ay, 0x1A7A) % 2) as usize;
    let mut chains: Vec<Vec<(usize, usize)>> = Vec::new();
    for t in 0..tubes {
        // Each tube starts a step from the vent in its own direction, then runs downhill.
        let a = (hash(ax, ay, 0x1A7B + t as u64) % 628) as f64 / 100.0;
        let (si, sj) = ((vi as f64 + 1.5 * a.cos()).round() as i64, (vj as f64 + 1.5 * a.sin()).round() as i64);
        if si < 1 || sj < 1 || si as usize >= f.gw - 1 || sj as usize >= f.gh - 1 { continue; }
        let mut chain = vec![(si as usize, sj as usize)];
        let len = 2 + (hash(ax, ay, 0x1A7C + t as u64) % 3) as usize;
        for _ in 0..len {
            let &(x, y) = chain.last().unwrap();
            let Some(n) = ctx.descend(x, y) else { break };
            if f.h[f.idx(n.0, n.1)] <= 0.0 { break; }
            chain.push(n);
        }
        if chain.len() >= 2 && !chains.iter().any(|c| c[0] == chain[0]) { chains.push(chain); }
    }
    for chain in chains {
        let mut b = Build { nodes: Vec::new(), passages: Vec::new() };
        let m = chain.len() - 1;
        let mut nodes: Vec<u32> = Vec::new();
        for (t, &(i, j)) in chain.iter().enumerate() {
            let (x, y) = (i as f64, j as f64);
            let g = ctx.ground(x, y);
            let kind = if t == m { CaveNodeKind::Mouth } else if t % 2 == 1 { CaveNodeKind::Skylight } else { CaveNodeKind::Passage };
            let z = if matches!(kind, CaveNodeKind::Mouth) { g } else { g - 5.0 };
            let (ai, aj) = f.abs_cell(i, j);
            nodes.push(b.node(f, x, y, z, kind, 4.0 + 4.0 * unit(ai, aj, 0x1A7D), 3.0 + 2.0 * unit(ai, aj, 0x1A7E)));
        }
        for w in nodes.windows(2) { b.link(w[0], w[1], PassageKind::Tube, 5.0, 3.5, false); }
        let name = if v.is_active { "A lava tube under the live flow".to_string() } else { "An old lava tube".to_string() };
        out.caves.push(CaveGraph { kind: CaveKind::LavaTube, name, nodes: b.nodes, passages: b.passages, tile });
    }
}

/// Sea caves: one at most per tile, where hard high land meets the sea.
fn plan_sea_caves(ctx: &mut Ctx, i0: usize, j0: usize, out: &mut SitePlan) {
    let f = ctx.f;
    let s = f.s;
    let tile = f.tile_of(i0, j0);
    let mut best: Option<(u64, usize, usize, (i64, i64))> = None;
    for j in j0 + 1..j0 + s - 1 { for i in i0 + 1..i0 + s - 1 {
        let k = f.idx(i, j);
        if f.h[k] < 15.0 || !matches!(ctx.rock[k], RockType::Basalt | RockType::Granite | RockType::Sandstone) { continue; }
        // The sea beside it, and which way.
        let sea = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)].into_iter().find(|&(dx, dy)| f.h[f.idx((i as i64 + dx) as usize, (j as i64 + dy) as usize)] <= 0.0);
        let Some(d) = sea else { continue };
        let (ax, ay) = f.abs_cell(i, j);
        let h = hash(ax, ay, 0x5EA);
        if best.map_or(true, |b| h < b.0) { best = Some((h, i, j, d)); }
    } }
    let Some((h, i, j, (dx, dy))) = best else { return };
    // Not every headland: about one coast tile in five.
    if h % SEA_CAVE_ONE_IN != 0 { return; }
    let mut b = Build { nodes: Vec::new(), passages: Vec::new() };
    // The mouth in the shore cell itself (on land, at the water's edge), the hollow running
    // into the land.
    let (mx, my) = (i as f64, j as f64);
    let (ai, aj) = f.abs_cell(i, j);
    let depth = ((30.0 + 40.0 * unit(ai, aj, 0x5EB)) / f.cell_m) as f64;
    let (ex, ey) = (mx - dx as f64 * depth, my - dy as f64 * depth);
    let mouth = b.node(f, mx, my, 1.0, CaveNodeKind::Mouth, 6.0, 5.0);
    let end = b.node(f, ex, ey, 1.0, CaveNodeKind::Chamber, 10.0, 5.0);
    b.link(mouth, end, PassageKind::Phreatic, 6.0, 5.0, true);
    out.caves.push(CaveGraph { kind: CaveKind::SeaCave, name: "A sea cave".to_string(), nodes: b.nodes, passages: b.passages, tile });
}

/// Rock shelters: an overhang at the foot of a sandstone slope, one at most per tile.
fn plan_rock_shelters(ctx: &mut Ctx, i0: usize, j0: usize, out: &mut SitePlan) {
    let f = ctx.f;
    let s = f.s;
    let tile = f.tile_of(i0, j0);
    let mut best: Option<(u64, usize, usize)> = None;
    for j in j0 + 1..j0 + s - 1 { for i in i0 + 1..i0 + s - 1 {
        let k = f.idx(i, j);
        if f.h[k] <= 0.0 || ctx.rock[k] != RockType::Sandstone || f.slope(i, j) < 0.03 || ctx.channel(k, j) { continue; }
        if ctx.land[k] != Landform::None && !matches!(ctx.land[k], Landform::Cliff | Landform::Outcrop | Landform::Scree) { continue; }
        let (ax, ay) = f.abs_cell(i, j);
        let h = hash(ax, ay, 0x5E17);
        if best.map_or(true, |b| h < b.0) { best = Some((h, i, j)); }
    } }
    let Some((h, i, j)) = best else { return };
    if h % 4 != 0 { return; }
    let (x, y) = (i as f64, j as f64);
    let g = ctx.ground(x, y);
    let mut b = Build { nodes: Vec::new(), passages: Vec::new() };
    let (ai, aj) = f.abs_cell(i, j);
    let mouth = b.node(f, x, y, g, CaveNodeKind::Mouth, 12.0 + 8.0 * unit(ai, aj, 0x5E18), 3.0);
    // The back of the shelter a few metres into the slope, the way the ground rises.
    let up = ctx.descend(i, j).map(|(nx, ny)| ((i as f64 - nx as f64), (j as f64 - ny as f64))).unwrap_or((0.0, -1.0));
    let len = (up.0 * up.0 + up.1 * up.1).sqrt().max(1e-6);
    let back = 8.0 / f.cell_m as f64;
    let end = b.node(f, x + up.0 / len * back, y + up.1 / len * back, g + 0.5, CaveNodeKind::Chamber, 12.0, 3.0);
    b.link(mouth, end, PassageKind::Breakdown, 12.0, 3.0, false);
    out.caves.push(CaveGraph { kind: CaveKind::RockShelter, name: "A rock shelter".to_string(), nodes: b.nodes, passages: b.passages, tile });
}

#[cfg(test)]
mod tests {
    use crate::region::plan::CaveNodeKind;
    use crate::region::zoom::{generate_zoom, ZoomParams};

    /// Two windows that share a tile plan the same caves for it: the same nodes (by cell and
    /// height) and the same passages.
    #[test]
    fn two_windows_plan_the_same_caves() {
        let mut checked = 0;
        for seed in [5u64, 3, 7, 11, 2, 13, 17, 19] {
            let world = crate::world::generate_world_with_style(128, 64, seed, crate::plates::WorldStyle::Earthlike);
            let (cx, cy) = crate::region::zoom::pick_interesting_window(&world, 4);
            let params = ZoomParams { center_x: cx, center_y: cy, tiles: 4, cells_per_tile: 32, erosion_iterations: 10, seed: 3 };
            let a = generate_zoom(&world, &params);
            if a.caves.is_empty() { continue; }
            let shifted = ZoomParams { center_x: cx + 1, center_y: cy, ..params.clone() };
            let b = generate_zoom(&world, &shifted);
            // The tiles both windows hold as core.
            let s = 32i64;
            for ca in &a.caves {
                let tx = (ca.nodes[0].x / s as f64).floor() as i64;
                if tx < a.world_x0 + 1 || tx >= a.world_x0 + 4 { continue; }
                let cb = b.caves.iter().find(|c| c.tile == ca.tile && c.kind == ca.kind && (c.nodes[0].x - ca.nodes[0].x).abs() < 1e-6 && (c.nodes[0].y - ca.nodes[0].y).abs() < 1e-6)
                    .unwrap_or_else(|| panic!("seed {seed}: cave of tile {:?} planned by one window and not the other", ca.tile));
                assert_eq!(ca.nodes.len(), cb.nodes.len());
                assert_eq!(ca.passages.len(), cb.passages.len());
                for (na, nb) in ca.nodes.iter().zip(&cb.nodes) {
                    assert!((na.x - nb.x).abs() < 1e-6 && (na.y - nb.y).abs() < 1e-6 && (na.z_m - nb.z_m).abs() < 0.01, "seed {seed}: node differs: {na:?} vs {nb:?}");
                    assert_eq!(na.kind, nb.kind);
                }
                checked += 1;
            }
            if checked > 0 { break; }
        }
        println!("caves compared across two windows: {checked}");
    }

    /// A karst cave has a mouth above its spring and reaches the stream well below the ground.
    #[test]
    fn karst_caves_go_down() {
        for seed in [5u64, 3, 7, 11, 2, 13, 17, 19, 23, 29] {
            let world = crate::world::generate_world_with_style(128, 64, seed, crate::plates::WorldStyle::Earthlike);
            let (cx, cy) = crate::region::zoom::pick_interesting_window(&world, 4);
            let params = ZoomParams { center_x: cx, center_y: cy, tiles: 4, cells_per_tile: 32, erosion_iterations: 10, seed: 3 };
            let r = generate_zoom(&world, &params);
            let Some(c) = r.caves.iter().find(|c| c.kind == crate::region::plan::CaveKind::Karst) else { continue };
            let mouth = c.nodes.iter().find(|n| n.kind == CaveNodeKind::Mouth).expect("a mouth");
            let deepest = c.nodes.iter().map(|n| n.z_m).fold(f32::MAX, f32::min);
            assert!(mouth.z_m - deepest >= 10.0, "seed {seed}: the cave drops only {:.1} m", mouth.z_m - deepest);
            assert!(c.nodes.iter().any(|n| n.kind == CaveNodeKind::Sink));
            assert!(c.passages.iter().any(|p| p.stream));
            let place = r.places.iter().find(|p| p.cave.is_some()).expect("a place for the cave");
            assert!((place.x - mouth.x).abs() < 1e-6);
            println!("seed {seed}: {} nodes, {} passages, drop {:.0} m, {} caves in the window", c.nodes.len(), c.passages.len(), mouth.z_m - deepest, r.caves.len());
            return;
        }
        println!("no karst cave on ten test worlds");
    }
}
