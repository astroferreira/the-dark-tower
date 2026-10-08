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
    let mut g = vec![u32::MAX; w * h];
    let mut came = vec![u32::MAX; w * h];
    let mut open = BinaryHeap::new();
    g[idx(from)] = 0;
    open.push(Reverse((heur(from), idx(from) as u32)));
    let mut expanded = 0;
    while let Some(Reverse((_, k))) = open.pop() {
        let k = k as usize;
        let p = ((k % w) as u16, (k / w) as u16);
        if p == to {
            let mut out = vec![p];
            let mut c = k;
            while came[c] != u32::MAX { c = came[c] as usize; out.push(((c % w) as u16, (c / w) as u16)); }
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
            let ng = g[k] + if diagonal { step * 14 / 10 } else { step };
            if ng < g[nk] {
                g[nk] = ng;
                came[nk] = k as u32;
                open.push(Reverse((ng + heur((nx as u16, ny as u16)), nk as u32)));
            }
        }
    }
    None
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
    let under = map.cell(x, y, z as usize);
    if !(matches!(under.shape, Shape::Wall | Shape::Floor | Shape::Ramp | Shape::Stair)) { return false; }
    let body = map.cell(x, y, z as usize + 1);
    if solid(map, x, y, z + 1) { return false; }
    !(body.water > 0 && z as usize + 2 < map.depth && map.cell(x, y, z as usize + 2).water > 0)
}

/// Cost of standing at `(x, y, z)` (as `cost`, for any level), or None.
pub fn cost3(map: &LocalMap, x: usize, y: usize, z: i32) -> Option<u32> {
    if !standable(map, x, y, z) { return None; }
    let floor = map.cell(x, y, z as usize);
    let mut c = 10;
    if map.cell(x, y, z as usize + 1).water > 0 { c += 20; }
    if matches!(floor.plant, Plant::Tree(_)) { c += 4; }
    if floor.boulder { c += 6; }
    if matches!(floor.material, Material::Block(_) | Material::Gravel) || floor.shape == Shape::Stair { c -= 2; }
    Some(c)
}

/// The levels a walker can reach from `p` in one step, with each step's cost: the 8 neighbours
/// at the same level or one up or down (with headroom to climb and room to step down into), and
/// up or down a stair.
pub fn steps3(map: &LocalMap, p: P3, out: &mut Vec<(P3, u32)>) {
    out.clear();
    let (x, y, z) = (p.0 as usize, p.1 as usize, p.2);
    let (w, h) = (map.width as i32, map.height as i32);
    // (The column's surface first: nearly every step is on it.)
    let any_level = |ox: usize, oy: usize| {
        let sz = map.surface_z[oy * map.width + ox];
        ((sz - z).abs() <= 1 && standable(map, ox, oy, sz)) || (z - 1..=z + 1).any(|zz| zz != sz && standable(map, ox, oy, zz))
    };
    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if nx < 0 || ny < 0 || nx >= w || ny >= h { continue; }
        let (nx, ny) = (nx as usize, ny as usize);
        let diagonal = dx != 0 && dy != 0;
        if diagonal && !(any_level(x, ny) && any_level(nx, y)) { continue; }
        let sz = map.surface_z[ny * map.width + nx] - z;
        let order = if sz.abs() <= 1 { [sz, if sz == 0 { 1 } else { 0 }, if sz == -1 { 1 } else { -1 }] } else { [0, 1, -1] };
        for dz in order {
            let nz = z + dz;
            // Climbing needs headroom over where one stands; stepping down, room at one's own
            // height over the lower cell.
            if dz == 1 && solid(map, x, y, z + 2) { continue; }
            if dz == -1 && solid(map, nx, ny, z + 1) { continue; }
            let Some(c) = cost3(map, nx, ny, nz) else { continue };
            out.push(((nx as u16, ny as u16, nz), if diagonal { c * 14 / 10 } else { c }));
            break;
        }
    }
    // Stairs: up through a stair cell over one's head, down through the stair underfoot.
    if (z as usize + 1) < map.depth && map.cell(x, y, z as usize + 1).shape == Shape::Stair {
        if let Some(c) = cost3(map, x, y, z + 1) { out.push(((p.0, p.1, z + 1), c + 6)); }
    }
    if z >= 1 && map.cell(x, y, z as usize).shape == Shape::Stair {
        if let Some(c) = cost3(map, x, y, z - 1) { out.push(((p.0, p.1, z - 1), c + 6)); }
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
    let mut extra: crate::history::det::HashMap<P3, u32> = Default::default();
    let mut extra_list: Vec<P3> = Vec::new();
    let mut g = vec![u32::MAX; n];
    let mut came = vec![u32::MAX; n];
    let id = |p: P3, extra: &mut crate::history::det::HashMap<P3, u32>, extra_list: &mut Vec<P3>, g: &mut Vec<u32>, came: &mut Vec<u32>| -> usize {
        let k = p.1 as usize * w + p.0 as usize;
        if map.surface_z[k] == p.2 { return k; }
        *extra.entry(p).or_insert_with(|| { extra_list.push(p); g.push(u32::MAX); came.push(u32::MAX); (n + extra_list.len() - 1) as u32 }) as usize
    };
    let heur = |p: P3| {
        let (dx, dy) = ((p.0 as i32 - to.0 as i32).unsigned_abs(), (p.1 as i32 - to.1 as i32).unsigned_abs());
        (8 * dx.min(dy) + 10 * dx.max(dy)).max(10 * (p.2 - to.2).unsigned_abs())
    };
    let at = |k: usize, extra_list: &Vec<P3>| -> P3 { if k < n { ((k % w) as u16, (k / w) as u16, map.surface_z[k]) } else { extra_list[k - n] } };
    let start = id(from, &mut extra, &mut extra_list, &mut g, &mut came);
    g[start] = 0;
    let mut open = BinaryHeap::new();
    open.push(Reverse((heur(from), start as u32)));
    let mut expanded = 0;
    let mut nb = Vec::with_capacity(12);
    while let Some(Reverse((f, k))) = open.pop() {
        let k = k as usize;
        let p = at(k, &extra_list);
        if f > g[k].saturating_add(heur(p)) { continue; }
        if p == to {
            let mut out = vec![p];
            let mut c = k;
            while came[c] != u32::MAX { c = came[c] as usize; out.push(at(c, &extra_list)); }
            out.reverse();
            return Some(out);
        }
        expanded += 1;
        if expanded > max_nodes { return None; }
        steps3(map, p, &mut nb);
        for &(q, c) in nb.iter() {
            let c = match steps {
                Some(st) if st.len() == n && map.surface_z[q.1 as usize * w + q.0 as usize] == q.2 => c * (100 - (st[q.1 as usize * w + q.0 as usize] as u32 / 2).min(35)) / 100,
                _ => c,
            };
            let nk = id(q, &mut extra, &mut extra_list, &mut g, &mut came);
            let ng = g[k] + c;
            if ng < g[nk] {
                g[nk] = ng;
                came[nk] = k as u32;
                open.push(Reverse((ng + heur(q), nk as u32)));
            }
        }
    }
    None
}

/// The level to stand at in column `p`: its surface, or the nearest standable level to it.
pub fn surface3(map: &LocalMap, p: Pos) -> P3 {
    let (x, y) = (p.0 as usize, p.1 as usize);
    let sz = map.surface_z[y * map.width + x];
    (p.0, p.1, sz)
}
