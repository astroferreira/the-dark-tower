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
pub fn path(map: &LocalMap, from: Pos, to: Pos, max_nodes: usize) -> Option<Vec<Pos>> {
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
