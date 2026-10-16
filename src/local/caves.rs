//! Caves carved from the site layer (ROADMAP Stage 5): every cave graph the region planned
//! (`region::sites::caves`; `plan::CaveGraph`: nodes and passages in absolute region cells and
//! metres above the sea) is cut into each embark it crosses, in absolute levels, so two
//! neighbouring embarks carve one cave and it runs on under their shared edge. A karst cave is
//! its walk-in mouth on the hillside, the dry gallery, the breakdown chamber, the shaft down to
//! the stream's canyon (a stair, the stream in its floor), the flooded sump to the spring, the
//! shaft on down to the first cavern where the limestone goes deep; a lava tube has its
//! skylights; a sea cave its hollow at the shore; a rock shelter its overhang. Each is an
//! `UnderPlace` (`PlaceKind::Cave`) with its mouth, so the colony finds and explores it and the
//! level view keeps the unfound dark blank (`local_ink::draw_delve`).
//!
//! A cave is kilometres long and an embark 384 m across, so most embarks hold a piece: the
//! mouth and the gallery running off under the edge, or a stretch of the canyon with no way in
//! but from the next embark. The piece's cells are the place's; the far end is the last cut.

use super::*;
use crate::region::plan::{CaveKind, CaveNodeKind, PassageKind};

/// Levels of rock kept over a passage under the ground.
const ROOF: i32 = 2;
/// Columns from the mouth within which the way in may cut the ground open (the entrance cut).
const ENTRANCE_COLS: f64 = 4.0;
/// Water in a vadose canyon's floor cell (of `WATER_FULL`): a stream to wade.
const STREAM_WATER: u8 = 2;

/// A node on this embark: fractional columns (which may lie off the map) and its level.
#[derive(Clone, Copy)]
struct At { x: f64, y: f64, z: i32 }

/// Cut the region's caves into the embark made round region cell `(cx, cy)`; the places made.
pub(super) fn carve(map: &mut LocalMap, region: &ZoomRegion, cx: f64, cy: f64) -> Vec<places::UnderPlace> {
    let n = map.width as i64;
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    // Region cells per column.
    let tile_cells = TILE_M as f64 / region.cell_m as f64;
    let (x0, y0) = ((region.world_x0 * s) as f64, (region.world_y0 * s) as f64);
    let _ = (x0, y0, tile_cells);
    let col = |ax: f64, ay: f64| -> (f64, f64) { place_column(region, cx, cy, n as usize, ax, ay) };
    let z_min = map.z_min_m;
    let level = |z_m: f32| ((z_m - z_min) / Z_STEP_M).floor() as i32;
    let mut out = Vec::new();
    for (g, cave) in region.caves.iter().enumerate() {
        if cave.nodes.is_empty() { continue; }
        let at: Vec<At> = cave.nodes.iter().map(|nd| { let (x, y) = col(nd.x, nd.y); At { x, y, z: level(nd.z_m) } }).collect();
        // Only a cave whose footprint (widened by its widest chamber) crosses the map.
        let pad = cave.nodes.iter().map(|nd| nd.width_m).fold(0.0f32, f32::max) as f64 / TILE_M as f64 + 2.0;
        let (lo_x, hi_x) = (at.iter().map(|a| a.x).fold(f64::MAX, f64::min) - pad, at.iter().map(|a| a.x).fold(f64::MIN, f64::max) + pad);
        let (lo_y, hi_y) = (at.iter().map(|a| a.y).fold(f64::MAX, f64::min) - pad, at.iter().map(|a| a.y).fold(f64::MIN, f64::max) + pad);
        if hi_x < 0.0 || hi_y < 0.0 || lo_x >= n as f64 || lo_y >= n as f64 { continue; }
        let mouth_ix = cave.nodes.iter().position(|nd| nd.kind == CaveNodeKind::Mouth).unwrap_or(0);
        let mouth_at = at[mouth_ix];
        // The passages in walking order from the mouth (a breadth-first walk over the graph), so
        // the place's cells begin at its mouth and end at its far end.
        let mut order: Vec<usize> = Vec::new();
        let mut seen = vec![false; cave.nodes.len()];
        let mut queue = std::collections::VecDeque::new();
        seen[mouth_ix] = true;
        queue.push_back(mouth_ix as u32);
        while let Some(v) = queue.pop_front() {
            for (k, p) in cave.passages.iter().enumerate() {
                let other = if p.a == v { p.b } else if p.b == v { p.a } else { continue };
                if order.contains(&k) { continue; }
                order.push(k);
                if !seen[other as usize] { seen[other as usize] = true; queue.push_back(other); }
            }
        }
        let mut walk: Vec<((u16, u16), i32)> = Vec::new();
        // The mouth's own cell first: the way in is the ground there.
        if let Some(c) = inside(map, mouth_at.x, mouth_at.y) {
            let sz = map.surface_z[c.1 * map.width + c.0];
            walk.push(((c.0 as u16, c.1 as u16), sz));
        }
        for k in order {
            let p = &cave.passages[k];
            let (a, b) = (at[p.a as usize], at[p.b as usize]);
            let (na, nb) = (&cave.nodes[p.a as usize], &cave.nodes[p.b as usize]);
            match p.kind {
                PassageKind::Shaft => cut_shaft(map, a, b, na.kind, nb.kind, &mut walk),
                _ => {
                    // The passage from the node nearer the mouth in the walk (a, as queued).
                    let (from, to, nfrom, nto) = if seen_first(&cave.passages, k, p.a) { (a, b, na, nb) } else { (b, a, nb, na) };
                    let _ = (nfrom, nto);
                    let stream = p.stream;
                    let flooded = stream && p.kind == PassageKind::Phreatic;
                    // Only the mouth's own passage cuts the ground open at the entrance; a sump
                    // running under the mouth stays under it.
                    let entrance = (p.a as usize == mouth_ix || p.b as usize == mouth_ix).then_some(mouth_at);
                    cut_passage(map, from, to, p.width_m, p.height_m, entrance, flooded, stream && p.kind == PassageKind::Vadose, &mut walk);
                }
            }
        }
        // The chambers and skylights at the nodes.
        for (i, nd) in cave.nodes.iter().enumerate() {
            match nd.kind {
                CaveNodeKind::Chamber => cut_room(map, at[i], nd.width_m, nd.height_m, &mut walk),
                CaveNodeKind::Skylight => cut_skylight(map, at[i], &mut walk),
                _ => {}
            }
        }
        if walk.is_empty() { continue; }
        // Water only where it stands: a later cut through the same column may have re-opened
        // or re-filled a cell the stream was set in.
        let mut cols: Vec<(u16, u16)> = walk.iter().map(|c| c.0).collect();
        cols.sort_unstable();
        cols.dedup();
        for &(x, y) in &cols {
            let (x, y) = (x as usize, y as usize);
            for z in 1..map.depth - 1 {
                let k = map.idx(x, y, z);
                if map.cells[k].water == 0 { continue; }
                let below = map.cell(x, y, z - 1);
                let floating = below.shape == Shape::Empty && below.water == 0;
                if map.cells[k].shape != Shape::Empty || floating { map.cells[k].water = 0; }
            }
        }
        // The place: named and caused by the region's mouth place when it has one.
        let place = region.places.iter().find(|p| p.cave == Some(g as u32));
        let (cause, contents) = match place {
            Some(p) => (p.cause.clone(), p.contents.clone()),
            None => (match cave.kind {
                CaveKind::Karst => "the limestone here is hollowed by water".to_string(),
                CaveKind::LavaTube => "a lava flow ran on under its own crust and drained".to_string(),
                CaveKind::SeaCave => "the swell has cut into the rock of the shore".to_string(),
                _ => "the rock here is hollow".to_string(),
            }, vec!["dripping stone and a cold draught".to_string()]),
        };
        let mouth = inside(map, mouth_at.x, mouth_at.y).map(|c| (c.0 as u16, c.1 as u16));
        let mut cells = walk;
        cells.dedup();
        out.push(places::UnderPlace { kind: places::PlaceKind::Cave, name: cave.name.clone(), cause, contents, cells, mouth, found: false });
    }
    out
}

/// An absolute region position (cell k at k, the places' frame; `bicubic` reads cell k's own
/// value at k + 0.5) to fractional columns of the embark made round region cell `(cx, cy)`
/// (column i's centre at i).
pub(super) fn place_column(region: &ZoomRegion, cx: f64, cy: f64, n: usize, ax: f64, ay: f64) -> (f64, f64) {
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    let tile_cells = TILE_M as f64 / region.cell_m as f64;
    let (x0, y0) = ((region.world_x0 * s) as f64, (region.world_y0 * s) as f64);
    ((ax + 0.5 - x0 - cx) / tile_cells + n as f64 / 2.0 - 0.5, (ay + 0.5 - y0 - cy) / tile_cells + n as f64 / 2.0 - 0.5)
}

/// Whether node `v` of passage `k` was reached before the passage's other end in the walk:
/// true when `v` is the passage's `a` end and no earlier passage reached `b` first.
fn seen_first(passages: &[crate::region::plan::CavePassage], k: usize, v: u32) -> bool {
    let p = &passages[k];
    let other = if p.a == v { p.b } else { p.a };
    // Earlier passages (in index order, as the walk visits them) that touch `other` but not `v`
    // mean `other` was reached first.
    !passages[..k].iter().any(|q| (q.a == other || q.b == other) && q.a != v && q.b != v)
}

/// The column under a fractional position, if on the map.
fn inside(map: &LocalMap, x: f64, y: f64) -> Option<(usize, usize)> {
    let (cx, cy) = (x.round() as i64, y.round() as i64);
    (cx >= 0 && cy >= 0 && cx < map.width as i64 && cy < map.height as i64).then(|| (cx as usize, cy as usize))
}

/// Open the cells over a floor at level `z` in column `(x, y)`: `h` levels of room, the floor
/// made walkable; under the ground the roof is kept, at the entrance the ground is cut open.
/// False where water lies in the way (the cut is left out).
fn open_column(map: &mut LocalMap, x: usize, y: usize, z: i32, h: i32, entrance: bool, water: u8) -> bool {
    let n = map.width;
    let sz = map.surface_z[y * n + x];
    if z < 1 || (z + h + 1) as usize >= map.depth { return false; }
    let top = if entrance { (z + h).min(sz.max(z)) } else { (z + h).min(sz - 1) };
    if top < z { return false; }
    for zz in z..=top {
        if map.cell(x, y, zz as usize).water > 0 { return false; }
    }
    for zz in z + 1..=top {
        let k = map.idx(x, y, zz as usize);
        if matches!(map.cells[k].shape, Shape::Stair) { continue; }
        map.cells[k] = Cell::AIR;
    }
    let f = map.idx(x, y, z as usize);
    if matches!(map.cells[f].shape, Shape::Wall | Shape::Ramp) { map.cells[f].shape = Shape::Floor; }
    if map.cells[f].shape == Shape::Empty { map.cells[f].shape = Shape::Floor; map.cells[f].material = Material::Gravel; }
    map.cells[f].plant = Plant::None;
    map.cells[f].boulder = false;
    if water > 0 {
        let k = map.idx(x, y, (z + 1) as usize);
        map.cells[k].water = water;
    }
    // The ground cut down to the floor at the entrance.
    if entrance && top >= sz && z < sz { map.surface_z[y * n + x] = z; }
    true
}

/// The lowest the floor may lie in column `(x, y)` with `h` levels of room under the roof.
fn roofed(map: &LocalMap, x: usize, y: usize, h: i32) -> i32 { map.surface_z[y * map.width + x] - ROOF - h }

/// A passage from `a` to `b`: a column's step at a time, the floor following the nodes'
/// levels a level at a time at most (so it can be walked), kept under the roof except at the
/// entrance, with `width_m` of room either side and `height_m` of headroom.
fn cut_passage(map: &mut LocalMap, a: At, b: At, width_m: f32, height_m: f32, mouth: Option<At>, flooded: bool, stream: bool, walk: &mut Vec<((u16, u16), i32)>) {
    let h = ((height_m / Z_STEP_M).ceil() as i32).clamp(2, 8);
    let r = ((width_m / 2.0 / TILE_M).round() as i64).clamp(0, 12);
    let steps = ((b.x - a.x).abs().max((b.y - a.y).abs()).ceil() as usize).max(1);
    let water = if flooded { WATER_FULL } else if stream { STREAM_WATER } else { 0 };
    let mut prev_z: Option<i32> = None;
    for t in 0..=steps {
        let f = t as f64 / steps as f64;
        let (x, y) = (a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f);
        let want = (a.z as f64 + (b.z - a.z) as f64 * f).round() as i32;
        let (cx, cy) = (x.round() as i64, y.round() as i64);
        let entrance = mouth.map_or(false, |m| (x - m.x).abs().max((y - m.y).abs()) <= ENTRANCE_COLS);
        // The floor of this step: under the roof of every column of its width, a level from the
        // last step at most.
        let mut z = want;
        if !entrance {
            for dy in -r..=r { for dx in -r..=r {
                let (px, py) = (cx + dx, cy + dy);
                if px < 0 || py < 0 || px >= map.width as i64 || py >= map.height as i64 { continue; }
                z = z.min(roofed(map, px as usize, py as usize, h));
            } }
        }
        if let Some(pz) = prev_z { z = z.clamp(pz - 1, pz + 1); }
        prev_z = Some(z);
        for dy in -r..=r { for dx in -r..=r {
            if dx * dx + dy * dy > r * r + r { continue; }
            let (px, py) = (cx + dx, cy + dy);
            if px < 0 || py < 0 || px >= map.width as i64 || py >= map.height as i64 { continue; }
            if open_column(map, px as usize, py as usize, z, h, entrance, water) { walk.push(((px as u16, py as u16), z)); }
        } }
    }
}

/// A chamber at a node: a round room of one floor level, as low as the roof over all of it
/// allows.
fn cut_room(map: &mut LocalMap, at: At, width_m: f32, height_m: f32, walk: &mut Vec<((u16, u16), i32)>) {
    let h = ((height_m / Z_STEP_M).ceil() as i32).clamp(2, 8);
    let r = ((width_m / 2.0 / TILE_M).round() as i64).clamp(1, 16);
    let (cx, cy) = (at.x.round() as i64, at.y.round() as i64);
    let mut z = at.z;
    let mut any = false;
    for dy in -r..=r { for dx in -r..=r {
        if dx * dx + dy * dy > r * r + r { continue; }
        let (px, py) = (cx + dx, cy + dy);
        if px < 0 || py < 0 || px >= map.width as i64 || py >= map.height as i64 { continue; }
        z = z.min(roofed(map, px as usize, py as usize, h));
        any = true;
    } }
    if !any { return; }
    for dy in -r..=r { for dx in -r..=r {
        if dx * dx + dy * dy > r * r + r { continue; }
        let (px, py) = (cx + dx, cy + dy);
        if px < 0 || py < 0 || px >= map.width as i64 || py >= map.height as i64 { continue; }
        if open_column(map, px as usize, py as usize, z, h, false, 0) { walk.push(((px as u16, py as u16), z)); }
    } }
}

/// A shaft between two nodes of one column: a stair from the upper level to the lower; a node
/// at the ground (a sink, a mouth, a skylight) starts it at the surface.
fn cut_shaft(map: &mut LocalMap, a: At, b: At, ka: CaveNodeKind, kb: CaveNodeKind, walk: &mut Vec<((u16, u16), i32)>) {
    let Some((x, y)) = inside(map, (a.x + b.x) / 2.0, (a.y + b.y) / 2.0) else { return };
    let sz = map.surface_z[y * map.width + x];
    let at_ground = |k: CaveNodeKind| matches!(k, CaveNodeKind::Sink | CaveNodeKind::Mouth | CaveNodeKind::Skylight | CaveNodeKind::Resurgence);
    let (mut top, mut bottom) = (a.z.max(b.z), a.z.min(b.z));
    if (a.z >= b.z && at_ground(ka)) || (b.z > a.z && at_ground(kb)) { top = sz; }
    top = top.min(sz);
    bottom = bottom.max(1);
    if top <= bottom || (top + 1) as usize >= map.depth { return; }
    // No stair through water.
    for z in bottom..=top { if map.cell(x, y, z as usize).water > 0 { return; } }
    for z in bottom + 1..=top {
        let k = map.idx(x, y, z as usize);
        map.cells[k].shape = Shape::Stair;
        map.cells[k].plant = Plant::None;
        map.cells[k].boulder = false;
        walk.push(((x as u16, y as u16), z));
    }
    let f = map.idx(x, y, bottom as usize);
    if matches!(map.cells[f].shape, Shape::Wall | Shape::Ramp | Shape::Empty) { map.cells[f].shape = Shape::Floor; }
    if map.cells[f].material == Material::Air { map.cells[f].material = Material::Gravel; }
    walk.push(((x as u16, y as u16), bottom));
}

/// A skylight: the tube's roof fallen in, a hole from the ground down to the tube's floor.
fn cut_skylight(map: &mut LocalMap, at: At, walk: &mut Vec<((u16, u16), i32)>) {
    let Some((x, y)) = inside(map, at.x, at.y) else { return };
    let n = map.width;
    let sz = map.surface_z[y * n + x];
    if at.z < 1 || at.z >= sz { return; }
    for z in at.z..=sz { if map.cell(x, y, z as usize).water > 0 { return; } }
    for z in at.z + 1..=sz { let k = map.idx(x, y, z as usize); map.cells[k] = Cell::AIR; }
    let f = map.idx(x, y, at.z as usize);
    if map.cells[f].shape != Shape::Stair { map.cells[f].shape = Shape::Floor; }
    map.surface_z[y * n + x] = at.z;
    walk.push(((x as u16, y as u16), at.z));
}
