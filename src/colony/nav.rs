//! Walking on the surface of a playable area.
//!
//! Settlers walk on the ground's surface, one node per column (at its floor z-level). They can
//! step to any of the 8 neighbours whose floor is at most one level higher or lower (the map
//! puts ramps where the ground steps up one level), wade water one level deep at a cost, and
//! can't cross deeper water or the walls of buildings. A* with an octile heuristic.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use crate::local::{LocalMap, Material, Plant, Shape};

pub type Pos = (u16, u16);

/// The open list: pops the least (f, k), as a `BinaryHeap<Reverse<(f, k)>>` would, but kept as
/// one small heap of k per value of f (estimates are small integers and most pops take from the
/// lowest bucket), so pushes and pops touch a few entries instead of the whole heap.
#[derive(Default)]
struct Open {
    buckets: Vec<BinaryHeap<Reverse<u32>>>,
    /// Buckets that may hold entries (to empty them for the next search).
    used: Vec<u32>,
    /// No bucket below this holds an entry.
    low: usize,
    /// Entries in the buckets.
    len: usize,
    /// Estimates of `OPEN_BUCKETS` or more (never on a playable map): an ordinary heap, popped
    /// after every bucket.
    high: BinaryHeap<Reverse<(u32, u32)>>,
}

const OPEN_BUCKETS: u32 = 1 << 16;

impl Open {
    fn clear(&mut self) {
        for &f in &self.used { self.buckets[f as usize].clear(); }
        self.used.clear();
        self.low = 0;
        self.len = 0;
        self.high.clear();
    }
    #[inline]
    fn push(&mut self, f: u32, k: u32) {
        if f >= OPEN_BUCKETS { self.high.push(Reverse((f, k))); return; }
        let fi = f as usize;
        if fi >= self.buckets.len() { self.buckets.resize_with(fi + 1, BinaryHeap::new); }
        let b = &mut self.buckets[fi];
        if b.is_empty() { self.used.push(f); }
        b.push(Reverse(k));
        if fi < self.low { self.low = fi; }
        self.len += 1;
    }
    #[inline]
    fn pop(&mut self) -> Option<(u32, u32)> {
        if self.len == 0 { return self.high.pop().map(|Reverse(e)| e); }
        while self.buckets[self.low].is_empty() { self.low += 1; }
        let Reverse(k) = self.buckets[self.low].pop()?;
        self.len -= 1;
        Some((self.low as u32, k))
    }
}

/// One search's per-cell costs and links, kept between searches (a 192x192 map's two arrays had
/// been allocated and cleared for every search, short ones too): a cell's entries count only
/// when its `mark` is this search's `gen`.
#[derive(Default)]
struct Scratch {
    g: Vec<u32>,
    came: Vec<u32>,
    mark: Vec<u32>,
    gen: u32,
    open: Open,
    /// Cells off the surface (`path3`), numbered from `n` as met, with their costs and links.
    extra: crate::history::det::FastMap<P3, u32>,
    extra_list: Vec<P3>,
    gx: Vec<u32>,
    cx: Vec<u32>,
    nb: Vec<(P3, u32)>,
    /// `Memo`'s arrays (path3 only): surface cells' standable and cost, marked by `gen`.
    smark: Vec<u32>,
    st: Vec<u8>,
    cmark: Vec<u32>,
    cost: Vec<u32>,
    off: Vec<Off>,
}

impl Scratch {
    /// Ready for a new search over `n` surface cells.
    fn begin(&mut self, n: usize) {
        if self.mark.len() != n {
            self.g = vec![u32::MAX; n];
            self.came = vec![u32::MAX; n];
            self.mark = vec![0; n];
            // (`gen` runs on: entries kept from searches before are older than it.)
            self.smark.clear();
            self.cmark.clear();
            self.off.clear();
        }
        if self.gen == u32::MAX {
            self.mark.iter_mut().for_each(|m| *m = 0);
            self.smark.iter_mut().for_each(|m| *m = 0);
            self.cmark.iter_mut().for_each(|m| *m = 0);
            self.off.iter_mut().for_each(|e| e.gen = 0);
            self.gen = 0;
        }
        self.gen += 1;
        self.open.clear();
        self.extra.clear();
        self.extra_list.clear();
        self.gx.clear();
        self.cx.clear();
    }
    #[inline]
    fn g(&self, k: usize) -> u32 {
        let n = self.mark.len();
        if k < n { if self.mark[k] == self.gen { self.g[k] } else { u32::MAX } } else { self.gx[k - n] }
    }
    #[inline]
    fn came(&self, k: usize) -> u32 {
        let n = self.mark.len();
        if k < n { if self.mark[k] == self.gen { self.came[k] } else { u32::MAX } } else { self.cx[k - n] }
    }
    #[inline]
    fn set(&mut self, k: usize, g: u32, came: u32) {
        let n = self.mark.len();
        if k < n { self.mark[k] = self.gen; self.g[k] = g; self.came[k] = came; } else { self.gx[k - n] = g; self.cx[k - n] = came; }
    }
}

thread_local! {
    static SCRATCH2: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
    static SCRATCH3: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

/// Cost of crossing a column (in "straight steps"), or None if it can't be entered.
pub fn cost(map: &LocalMap, x: usize, y: usize) -> Option<u32> {
    let k = y * map.width + x;
    let sz = map.surface_z[k];
    let above = sz + 1;
    if above as usize >= map.depth { return Some(10); }
    let c = map.cell(x, y, above as usize);
    if c.shape == Shape::Wall { return None; }
    let mut water = 0u32;
    let mut z = above;
    while (z as usize) < map.depth && map.cell(x, y, z as usize).water > 0 { water += 1; z += 1; }
    if water > 1 { return None; }
    let floor = map.cell(x, y, sz.max(0) as usize);
    let mut c = 10;
    if water == 1 { c += 20; }
    if matches!(floor.plant, Plant::Tree(_)) { c += 4; }
    if floor.boulder { c += 6; }
    if matches!(floor.material, Material::Block(_) | Material::Gravel) { c -= 2; }
    Some(c)
}

pub fn passable(map: &LocalMap, p: Pos) -> bool { cost(map, p.0 as usize, p.1 as usize).is_some() }

/// Shortest walk from `from` to `to` (both included), or None if there is none within
/// `max_nodes` expansions.
pub fn path(map: &LocalMap, from: Pos, to: Pos, max_nodes: usize) -> Option<Vec<Pos>> { path_worn(map, None, from, to, max_nodes) }

/// As `path`, but cells worn by feet (`steps`, per cell) cost up to 35% less, so walkers follow
/// the tracks others made and desire paths form.
pub fn path_worn(map: &LocalMap, steps: Option<&[u16]>, from: Pos, to: Pos, max_nodes: usize) -> Option<Vec<Pos>> {
    if from == to { return Some(vec![from]); }
    let (w, h) = (map.width, map.height);
    if !passable(map, to) { return None; }
    let idx = |p: Pos| p.1 as usize * w + p.0 as usize;
    let heur = |p: Pos| {
        let (dx, dy) = ((p.0 as i32 - to.0 as i32).unsigned_abs(), (p.1 as i32 - to.1 as i32).unsigned_abs());
        8 * dx.min(dy) + 10 * dx.max(dy)
    };
    SCRATCH2.with(|s| {
    let s = &mut *s.borrow_mut();
    s.begin(w * h);
    s.set(idx(from), 0, u32::MAX);
    s.open.push(heur(from), idx(from) as u32);
    let mut expanded = 0;
    while let Some((_, k)) = s.open.pop() {
        let k = k as usize;
        let p = ((k % w) as u16, (k / w) as u16);
        if p == to {
            let mut out = vec![p];
            let mut c = k;
            while s.came(c) != u32::MAX { c = s.came(c) as usize; out.push(((c % w) as u16, (c / w) as u16)); }
            out.reverse();
            return Some(out);
        }
        expanded += 1;
        if expanded > max_nodes { return None; }
        let sz = map.surface_z[k];
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (nx, ny) = (p.0 as i32 + dx, p.1 as i32 + dy);
            if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h { continue; }
            let (nx, ny) = (nx as usize, ny as usize);
            let nk = ny * w + nx;
            if (map.surface_z[nk] - sz).abs() > 1 { continue; }
            let Some(step) = cost(map, nx, ny) else { continue };
            let step = match steps { Some(st) if st.len() == w * h => step * (100 - (st[nk] as u32 / 2).min(35)) / 100, _ => step };
            let diagonal = dx != 0 && dy != 0;
            if diagonal {
                // No cutting corners past blocked columns.
                if cost(map, p.0 as usize, ny).is_none() || cost(map, nx, p.1 as usize).is_none() { continue; }
            }
            let ng = s.g(k) + if diagonal { step * 14 / 10 } else { step };
            if ng < s.g(nk) {
                s.set(nk, ng, k as u32);
                s.open.push(ng + heur((nx as u16, ny as u16)), nk as u32);
            }
        }
    }
    None
    })
}

// ---------------------------------------------------------------------------------------------
// Walking in three dimensions (DF's map: every open cell over solid ground is somewhere to stand)
// ---------------------------------------------------------------------------------------------

/// A place to stand: column and floor level (the walker's body is in the cell above, `z + 1`).
pub type P3 = (u16, u16, i32);

/// Solid: something to stand on and nothing to walk through (rock, soil, built walls, floors).
fn solid(map: &LocalMap, x: usize, y: usize, z: i32) -> bool {
    if z < 0 { return true; }
    if z as usize >= map.depth { return false; }
    matches!(map.cell(x, y, z as usize).shape, Shape::Wall | Shape::Floor | Shape::Ramp)
}

/// Whether one can stand at `(x, y)` on level `z`: solid ground or a stair underfoot, open space
/// (air or a stair) for the body above it, and no more than one level of water.
pub fn standable(map: &LocalMap, x: usize, y: usize, z: i32) -> bool {
    if x >= map.width || y >= map.height || z < 0 || z as usize + 1 >= map.depth { return false; }
    // (One index and the levels above it by stride: this is asked for most cells a search meets.)
    let (k, wh) = (map.idx(x, y, z as usize), map.width * map.height);
    if !(matches!(map.cells[k].shape, Shape::Wall | Shape::Floor | Shape::Ramp | Shape::Stair)) { return false; }
    // The body's cell, z + 1 (below the top: checked above), must not be solid (`solid`).
    let body = &map.cells[k + wh];
    if matches!(body.shape, Shape::Wall | Shape::Floor | Shape::Ramp) { return false; }
    !(body.water > 0 && z as usize + 2 < map.depth && map.cells[k + 2 * wh].water > 0)
}

/// Cost of standing at `(x, y, z)` (as `cost`, for any level), or None.
pub fn cost3(map: &LocalMap, x: usize, y: usize, z: i32) -> Option<u32> {
    if !standable(map, x, y, z) { return None; }
    let (k, wh) = (map.idx(x, y, z as usize), map.width * map.height);
    let floor = &map.cells[k];
    let mut c = 10;
    if map.cells[k + wh].water > 0 { c += 20; }
    if matches!(floor.plant, Plant::Tree(_)) { c += 4; }
    if floor.boulder { c += 6; }
    if matches!(floor.material, Material::Block(_) | Material::Gravel) || floor.shape == Shape::Stair { c -= 2; }
    Some(c)
}

/// The levels a walker can reach from `p` in one step, with each step's cost: the 8 neighbours
/// at the same level or one up or down (with headroom to climb and room to step down into), and
/// up or down a stair.
pub fn steps3(map: &LocalMap, p: P3, out: &mut Vec<(P3, u32)>) { steps3_by(map, p, out, &mut Direct) }

/// How `steps3` asks whether a cell can be stood on and what it costs: directly, or remembered
/// for the surface cells during one search (`Memo`; the map does not change within a search).
trait Probe {
    fn standable(&mut self, map: &LocalMap, x: usize, y: usize, z: i32) -> bool;
    fn cost3(&mut self, map: &LocalMap, x: usize, y: usize, z: i32) -> Option<u32>;
}

struct Direct;
impl Probe for Direct {
    #[inline]
    fn standable(&mut self, map: &LocalMap, x: usize, y: usize, z: i32) -> bool { standable(map, x, y, z) }
    #[inline]
    fn cost3(&mut self, map: &LocalMap, x: usize, y: usize, z: i32) -> Option<u32> { cost3(map, x, y, z) }
}

/// Surface cells' answers within one search, valid where `mark` is `gen` (bit 0: standable
/// known, bit 1: cost known; `st` 0/1, `cost` u32::MAX for none).
struct Memo<'a> { gen: u32, smark: &'a mut [u32], st: &'a mut [u8], cmark: &'a mut [u32], cost: &'a mut [u32], off: &'a mut [Off] }

/// A cell off the surface in `Memo`'s small table (direct-mapped: a clash just asks again).
#[derive(Clone, Copy, Default)]
struct Off { gen: u32, key: u64, st: u8, has_cost: bool, cost: u32 }
const OFF_BITS: u32 = 15;

impl Memo<'_> {
    #[inline]
    fn off(&mut self, k: usize, z: i32) -> &mut Off {
        let key = (k as u64) << 32 | z as u32 as u64;
        let slot = (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - OFF_BITS)) as usize;
        let e = &mut self.off[slot];
        if e.gen != self.gen || e.key != key { *e = Off { gen: self.gen, key, st: 0, has_cost: false, cost: 0 }; }
        e
    }
}

impl Probe for Memo<'_> {
    #[inline]
    fn standable(&mut self, map: &LocalMap, x: usize, y: usize, z: i32) -> bool {
        if x >= map.width || y >= map.height { return standable(map, x, y, z); }
        let k = y * map.width + x;
        if map.surface_z[k] != z {
            let e = self.off(k, z);
            if e.st == 0 { e.st = 1 + standable(map, x, y, z) as u8; }
            return e.st == 2;
        }
        if self.smark[k] == self.gen { return self.st[k] != 0; }
        let v = standable(map, x, y, z);
        self.smark[k] = self.gen;
        self.st[k] = v as u8;
        v
    }
    #[inline]
    fn cost3(&mut self, map: &LocalMap, x: usize, y: usize, z: i32) -> Option<u32> {
        if x >= map.width || y >= map.height { return cost3(map, x, y, z); }
        let k = y * map.width + x;
        if map.surface_z[k] != z {
            let e = self.off(k, z);
            if !e.has_cost { e.has_cost = true; e.cost = cost3(map, x, y, z).unwrap_or(u32::MAX); }
            return (e.cost != u32::MAX).then_some(e.cost);
        }
        if self.cmark[k] == self.gen { let c = self.cost[k]; return (c != u32::MAX).then_some(c); }
        let v = cost3(map, x, y, z);
        self.cmark[k] = self.gen;
        self.cost[k] = v.unwrap_or(u32::MAX);
        v
    }
}

fn steps3_by(map: &LocalMap, p: P3, out: &mut Vec<(P3, u32)>, probe: &mut impl Probe) {
    out.clear();
    let (x, y, z) = (p.0 as usize, p.1 as usize, p.2);
    let (w, h) = (map.width as i32, map.height as i32);
    // (The column's surface first: nearly every step is on it.)
    fn any_level(map: &LocalMap, probe: &mut impl Probe, ox: usize, oy: usize, z: i32) -> bool {
        let sz = map.surface_z[oy * map.width + ox];
        ((sz - z).abs() <= 1 && probe.standable(map, ox, oy, sz)) || (z - 1..=z + 1).any(|zz| zz != sz && probe.standable(map, ox, oy, zz))
    }
    // The four sides' answers, each asked at most once (every diagonal asks two of them):
    // 0 not yet asked, 1 no, 2 yes; west, east, north, south.
    let mut sides = [0u8; 4];
    let mut side = |map: &LocalMap, probe: &mut _, s: usize, ox: usize, oy: usize| {
        if sides[s] == 0 { sides[s] = 1 + any_level(map, probe, ox, oy, z) as u8; }
        sides[s] == 2
    };
    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if nx < 0 || ny < 0 || nx >= w || ny >= h { continue; }
        let (nx, ny) = (nx as usize, ny as usize);
        let diagonal = dx != 0 && dy != 0;
        if diagonal && !(side(map, probe, if dy < 0 { 2 } else { 3 }, x, ny) && side(map, probe, if dx < 0 { 0 } else { 1 }, nx, y)) { continue; }
        let sz = map.surface_z[ny * map.width + nx] - z;
        let order = if sz.abs() <= 1 { [sz, if sz == 0 { 1 } else { 0 }, if sz == -1 { 1 } else { -1 }] } else { [0, 1, -1] };
        for dz in order {
            let nz = z + dz;
            // Climbing needs headroom over where one stands; stepping down, room at one's own
            // height over the lower cell.
            if dz == 1 && solid(map, x, y, z + 2) { continue; }
            if dz == -1 && solid(map, nx, ny, z + 1) { continue; }
            let Some(c) = probe.cost3(map, nx, ny, nz) else { continue };
            out.push(((nx as u16, ny as u16, nz), if diagonal { c * 14 / 10 } else { c }));
            break;
        }
    }
    // Stairs: up through a stair cell over one's head, down through the stair underfoot.
    if (z as usize + 1) < map.depth && map.cell(x, y, z as usize + 1).shape == Shape::Stair {
        if let Some(c) = probe.cost3(map, x, y, z + 1) { out.push(((p.0, p.1, z + 1), c + 6)); }
    }
    if z >= 1 && map.cell(x, y, z as usize).shape == Shape::Stair {
        if let Some(c) = probe.cost3(map, x, y, z - 1) { out.push(((p.0, p.1, z - 1), c + 6)); }
    }
}

/// Shortest walk from `from` to `to` in three dimensions (both included), or None within
/// `max_nodes` expansions. Surface cells use dense arrays; cells off the surface (halls, stairs,
/// caverns, upper floors) are numbered as they are met. Worn ground (`steps`) is cheaper on the
/// surface, as in `path_worn`.
pub fn path3(map: &LocalMap, steps: Option<&[u16]>, from: P3, to: P3, max_nodes: usize) -> Option<Vec<P3>> {
    if from == to { return Some(vec![from]); }
    if !standable(map, to.0 as usize, to.1 as usize, to.2) { return None; }
    let (w, h) = (map.width, map.height);
    let n = w * h;
    // Off-surface cells are numbered n, n + 1, ... in the order they are met.
    let id = |p: P3, s: &mut Scratch| -> usize {
        let k = p.1 as usize * w + p.0 as usize;
        if map.surface_z[k] == p.2 { return k; }
        let Scratch { extra, extra_list, gx, cx, .. } = s;
        *extra.entry(p).or_insert_with(|| { extra_list.push(p); gx.push(u32::MAX); cx.push(u32::MAX); (n + extra_list.len() - 1) as u32 }) as usize
    };
    let heur = |p: P3| {
        let (dx, dy) = ((p.0 as i32 - to.0 as i32).unsigned_abs(), (p.1 as i32 - to.1 as i32).unsigned_abs());
        (8 * dx.min(dy) + 10 * dx.max(dy)).max(10 * (p.2 - to.2).unsigned_abs())
    };
    let at = |k: usize, extra_list: &Vec<P3>| -> P3 { if k < n { ((k % w) as u16, (k / w) as u16, map.surface_z[k]) } else { extra_list[k - n] } };
    SCRATCH3.with(|s| {
    let s = &mut *s.borrow_mut();
    s.begin(n);
    if s.smark.len() != n { s.smark = vec![0; n]; s.st = vec![0; n]; s.cmark = vec![0; n]; s.cost = vec![0; n]; }
    if s.off.is_empty() { s.off = vec![Off::default(); 1 << OFF_BITS]; }
    let start = id(from, s);
    s.set(start, 0, u32::MAX);
    s.open.push(heur(from), start as u32);
    let mut expanded = 0;
    let mut nb = std::mem::take(&mut s.nb);
    let found = loop {
        let Some((f, k)) = s.open.pop() else { break None };
        let k = k as usize;
        let p = at(k, &s.extra_list);
        if f > s.g(k).saturating_add(heur(p)) { continue; }
        if p == to {
            let mut out = vec![p];
            let mut c = k;
            while s.came(c) != u32::MAX { c = s.came(c) as usize; out.push(at(c, &s.extra_list)); }
            out.reverse();
            break Some(out);
        }
        expanded += 1;
        if expanded > max_nodes { break None; }
        steps3_by(map, p, &mut nb, &mut Memo { gen: s.gen, smark: &mut s.smark, st: &mut s.st, cmark: &mut s.cmark, cost: &mut s.cost, off: &mut s.off });
        for &(q, c) in nb.iter() {
            let c = match steps {
                Some(st) if st.len() == n && map.surface_z[q.1 as usize * w + q.0 as usize] == q.2 => c * (100 - (st[q.1 as usize * w + q.0 as usize] as u32 / 2).min(35)) / 100,
                _ => c,
            };
            let nk = id(q, s);
            let ng = s.g(k) + c;
            if ng < s.g(nk) {
                s.set(nk, ng, k as u32);
                s.open.push(ng + heur(q), nk as u32);
            }
        }
    };
    s.nb = nb;
    found
    })
}

/// The level to stand at in column `p`: its surface, or the nearest standable level to it.
pub fn surface3(map: &LocalMap, p: Pos) -> P3 {
    let (x, y) = (p.0 as usize, p.1 as usize);
    let sz = map.surface_z[y * map.width + x];
    (p.0, p.1, sz)
}
