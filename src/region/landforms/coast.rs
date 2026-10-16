//! Coasts: cliffs on hard rock facing the open sea, with stacks off them; beaches on soft
//! shores; spits, bars and barrier islands with lagoons behind them on gentle soft coasts;
//! dunes behind the beaches where it is dry or windy; tidal flats, salt marsh and mangrove in
//! sheltered shallow water; raised beaches on rising hard coasts (ROADMAP Stage 2).
//!
//! The shape pass squares hard open shores into cliffs inside each chunk; the labels come from
//! the blended region: the shore (land within a few cells of the sea), its openness (the share
//! of sea in the 21x21 cells round a cell, from a summed-area table) and the tile's rock,
//! hardness and stress. Nothing reads beyond the region's margin.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, Drainage, RegionFields};

/// Hardness (`WorldData::hardness_map`) from which a shore stands as a cliff.
const HARD: f32 = 0.6;
/// Share of sea in the 21x21 cells round a shore cell: open past the first, sheltered below
/// the second.
const OPEN: f32 = 0.3;
const SHELTERED: f32 = 0.2;
/// A cliff's slope (m per m) at 610 m cells; the step a cliffed chunk shore is raised to.
const CLIFF_SLOPE: f32 = 0.05;
const CLIFF_MIN_M: f32 = 12.0;
/// A beach lies this low (m) and this flat.
const BEACH_MAX_M: f32 = 12.0;
const BEACH_SLOPE: f32 = 0.05;
/// Tidal flats, salt marsh and mangrove: this shallow (m) and this low on the land.
const FLAT_DEPTH_M: f32 = 2.0;
const MARSH_MAX_M: f32 = 3.0;
const MANGROVE_TEMP_C: f32 = 20.0;
const MARSH_MIN_TEMP_C: f32 = 5.0;
/// Barriers stand this far from the land (cells), in water this shallow (m), on coasts this
/// open, in one coarse block of this many cells in three.
const BARRIER_DIST: (u8, u8) = (2, 4);
const BARRIER_DEPTH_M: f32 = 3.0;
const BARRIER_OPEN: (f32, f32) = (0.25, 0.6);
const BARRIER_BLOCK: i64 = 8;
const BARRIER_RATE: f32 = 0.35;
/// Dunes behind a beach: this far inland (cells), in moisture below this, or in the westerlies.
const DUNE_DIST: (u8, u8) = (2, 3);
const DUNE_MOIST: f32 = 0.35;
/// Raised beaches: this far inland, this high (m), on hard rising coasts.
const RAISED_DIST: (u8, u8) = (2, 5);
const RAISED_M: (f32, f32) = (5.0, 20.0);
/// Stacks: one shallow sea cell in this many beside a cliff.
const STACK_RATE: f32 = 0.08;
const FAR: u8 = u8::MAX;

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn unit(a: i64, b: i64, salt: u64) -> f32 {
    (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0
}

/// Steps (8-neighbour) from every cell to the nearest cell where `source` holds, up to `max`
/// (`FAR` beyond).
fn distance_to(source: &[bool], w: usize, h: usize, max: u8) -> Vec<u8> {
    let mut d = vec![FAR; w * h];
    let mut front: Vec<usize> = (0..w * h).filter(|&k| source[k]).collect();
    for k in &front { d[*k] = 0; }
    let mut step = 0u8;
    while !front.is_empty() && step < max {
        step += 1;
        let mut next = Vec::new();
        for &k in &front {
            let (x, y) = ((k % w) as i64, (k / w) as i64);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 { continue; }
                    let j = ny as usize * w + nx as usize;
                    if d[j] == FAR {
                        d[j] = step;
                        next.push(j);
                    }
                }
            }
        }
        front = next;
    }
    d
}

/// Summed-area table of a mask, for the share of sea round a cell in O(1).
struct Sat { w: usize, h: usize, t: Vec<u32> }

impl Sat {
    fn new(mask: &[bool], w: usize, h: usize) -> Sat {
        let mut t = vec![0u32; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = 0u32;
            for x in 0..w {
                row += mask[y * w + x] as u32;
                t[(y + 1) * (w + 1) + x + 1] = t[y * (w + 1) + x + 1] + row;
            }
        }
        Sat { w, h, t }
    }
    /// Share of the mask in the (2r+1)^2 window round (i, j), clipped to the grid.
    fn share(&self, i: usize, j: usize, r: usize) -> f32 {
        let (x0, y0) = (i.saturating_sub(r), j.saturating_sub(r));
        let (x1, y1) = ((i + r + 1).min(self.w), (j + r + 1).min(self.h));
        let at = |x: usize, y: usize| self.t[y * (self.w + 1) + x];
        let sum = at(x1, y1) + at(x0, y0) - at(x1, y0) - at(x0, y1);
        sum as f32 / ((x1 - x0) * (y1 - y0)) as f32
    }
}

fn hardness(f: &RegionFields, t: (usize, usize)) -> f32 {
    f.world.hardness_map.as_ref().map(|m| *m.get(t.0, t.1)).unwrap_or(0.5)
}

fn tile_hardness(g: &ChunkGrid, i: usize, j: usize) -> f32 {
    let t = g.tile_of(i, j);
    g.world.hardness_map.as_ref().map(|m| *m.get(t.0, t.1)).unwrap_or(0.5)
}

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

/// Hard shores stand as cliffs: a shore cell under hard rock is raised toward the ground three
/// and four cells inland, so the sea meets a step instead of a ramp. A function of the chunk's
/// own heights (which neighbouring chunks share up to the blend), modest and only where the
/// land behind stands well above the sea.
pub fn shape_after(h: &mut [f32], g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {
    let (gw, n) = (g.gw, g.gw * g.gw);
    let sea: Vec<bool> = h.iter().map(|&e| e <= 0.0).collect();
    let d = distance_to(&sea, gw, gw, 4);
    let before = h.to_vec();
    for j in 4..gw - 4 {
        for i in 4..gw - 4 {
            let k = j * gw + i;
            if sea[k] || d[k] > 2 || tile_hardness(g, i, j) < HARD { continue; }
            // The ground inland: the highest cell among those three to four cells away.
            let mut inland = 0.0f32;
            for dy in -4i64..=4 {
                for dx in -4i64..=4 {
                    let r2 = dx * dx + dy * dy;
                    if r2 < 9 || r2 > 16 { continue; }
                    let w = (j as i64 + dy) as usize * gw + (i as i64 + dx) as usize;
                    if d[w] >= 3 { inland = inland.max(before[w]); }
                }
            }
            if inland < CLIFF_MIN_M * 1.5 { continue; }
            let target = if d[k] == 1 { 0.6 * inland } else { 0.8 * inland };
            if target > h[k] { h[k] = target; }
        }
    }
    let _ = n;
}

pub fn classify(f: &RegionFields, rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let n = gw * gh;
    let sea: Vec<bool> = (0..n).map(|k| f.h[k] <= 0.0).collect();
    let is_land: Vec<bool> = sea.iter().map(|&s| !s).collect();
    let to_sea = distance_to(&sea, gw, gh, 6);
    let to_land = distance_to(&is_land, gw, gh, 6);
    let sat = Sat::new(&sea, gw, gh);
    let channel = super::fluvial::channel_mask(f);
    let cliff_slope = CLIFF_SLOPE * (610.0 / f.cell_m).clamp(0.4, 1.0);

    // The shore's land: cliffs, beaches, marsh and mangrove; the sea's edge: flats.
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if land[k] != Landform::None { continue; }
            let open = sat.share(i, j, 10);
            if sea[k] {
                if to_land[k] <= 3 && f.h[k] > -FLAT_DEPTH_M && open < SHELTERED {
                    land[k] = Landform::TidalFlat;
                }
                continue;
            }
            if to_sea[k] > 2 || channel[k] { continue; }
            let t = f.tile_of(i, j);
            let hard = hardness(f, t) > HARD || matches!(rock[k], RockType::Basalt | RockType::Granite);
            let slope = f.slope(i, j);
            if to_sea[k] == 1 && hard && open > OPEN && slope > cliff_slope && f.h[k] >= CLIFF_MIN_M {
                land[k] = Landform::SeaCliff;
            } else if to_sea[k] <= 2 && f.h[k] < MARSH_MAX_M && slope < 0.02 && open < SHELTERED + 0.02 {
                let temp = f.temp[k];
                if temp > MANGROVE_TEMP_C {
                    land[k] = Landform::Mangrove;
                } else if temp > MARSH_MIN_TEMP_C {
                    land[k] = Landform::SaltMarsh;
                }
            } else if to_sea[k] == 1 && slope < BEACH_SLOPE && f.h[k] < BEACH_MAX_M && !(hard && slope > cliff_slope * 0.5) {
                land[k] = Landform::Beach;
            }
        }
    }

    // Stacks: shallow sea cells beside a cliff, one in twelve.
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if !sea[k] || land[k] != Landform::None || f.h[k] < -20.0 || to_land[k] != 1 { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            if unit(ax, ay, 0xc01) >= STACK_RATE { continue; }
            let beside_cliff = (-1i64..=1).any(|dy| (-1i64..=1).any(|dx| {
                let (x, y) = (i as i64 + dx, j as i64 + dy);
                x >= 0 && y >= 0 && x < gw as i64 && y < gh as i64 && land[y as usize * gw + x as usize] == Landform::SeaCliff
            }));
            if beside_cliff { land[k] = Landform::Stack; }
        }
    }

    // Barriers off gentle soft coasts, in hashed stretches; then what they shelter.
    let mut barrier = vec![false; n];
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if !sea[k] || land[k] != Landform::None || f.h[k] <= -BARRIER_DEPTH_M { continue; }
            if to_land[k] < BARRIER_DIST.0 || to_land[k] > BARRIER_DIST.1 { continue; }
            let open = sat.share(i, j, 10);
            if open < BARRIER_OPEN.0 || open > BARRIER_OPEN.1 { continue; }
            let t = f.tile_of(i, j);
            if hardness(f, t) > HARD { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            if unit(ax.div_euclid(BARRIER_BLOCK), ay.div_euclid(BARRIER_BLOCK), 0xc02) < BARRIER_RATE { barrier[k] = true; }
        }
    }
    // Connected stretches: short ones are bars, the rest spits (hashed) or barrier islands.
    let mut seen = vec![false; n];
    for start in 0..n {
        if !barrier[start] || seen[start] { continue; }
        let mut comp = vec![start];
        seen[start] = true;
        let mut q = 0;
        while q < comp.len() {
            let k = comp[q];
            q += 1;
            let (x, y) = ((k % gw) as i64, (k / gw) as i64);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= gw as i64 || ny >= gh as i64 { continue; }
                    let w = ny as usize * gw + nx as usize;
                    if barrier[w] && !seen[w] {
                        seen[w] = true;
                        comp.push(w);
                    }
                }
            }
        }
        let (ax, ay) = f.abs_cell(start % gw, start / gw);
        let kind = if comp.len() < 3 { Landform::Bar } else if unit(ax, ay, 0xc03) < 0.4 { Landform::Spit } else { Landform::BarrierIsland };
        for &k in &comp { land[k] = kind; }
    }
    let near_barrier = distance_to(&barrier, gw, gh, 4);
    for k in 0..n {
        if sea[k] && land[k] == Landform::None && f.h[k] > -4.0 && to_land[k] <= 3 && near_barrier[k] <= 4 && !barrier[k] {
            land[k] = Landform::Lagoon;
        }
    }

    // Behind the beaches: dunes where it is dry or windy; raised beaches on rising hard coasts.
    let beach: Vec<bool> = land.iter().map(|&l| l == Landform::Beach).collect();
    let near_beach = distance_to(&beach, gw, gh, 3);
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if land[k] != Landform::None || sea[k] || channel[k] { continue; }
            let slope = f.slope(i, j);
            let lat = ((f.gy0 as f32 + (j as f32 + 0.5) / f.s as f32 + 0.5) / f.world.height as f32 * 180.0 - 90.0).abs();
            let windy = (35.0..60.0).contains(&lat);
            if to_sea[k] >= DUNE_DIST.0 && to_sea[k] <= DUNE_DIST.1 && near_beach[k] <= 2 && slope < BEACH_SLOPE && f.h[k] < 25.0 && (f.moist[k] < DUNE_MOIST || windy) {
                land[k] = Landform::CoastalDune;
                continue;
            }
            if to_sea[k] >= RAISED_DIST.0 && to_sea[k] <= RAISED_DIST.1 && f.h[k] >= RAISED_M.0 && f.h[k] <= RAISED_M.1 && slope < 0.03 {
                let t = f.tile_of(i, j);
                if hardness(f, t) > HARD && *f.world.stress_map.get(t.0, t.1) > 0.0 {
                    let (ax, ay) = f.abs_cell(i, j);
                    if unit(ax.div_euclid(4), ay.div_euclid(4), 0xc04) < 0.5 { land[k] = Landform::RaisedBeach; }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::zoom::{accumulate, route};

    fn world() -> crate::world::WorldData {
        crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike)
    }

    /// A low flat shore on the left half, a high steep one on the right, the sea below. A little
    /// hashed relief on the land, else every column drains straight down the plane and each
    /// shore cell is a channel of its own.
    fn coast(gw: usize, gh: usize) -> Vec<f32> {
        (0..gw * gh)
            .map(|k| {
                let (x, y) = ((k % gw) as f32, (k / gw) as f32);
                let shore = gh as f32 - 20.0;
                if y >= shore { return -1.0 - (y - shore) * 2.0; }
                let up = shore - y;
                let bump = unit((k % gw) as i64, (k / gw) as i64, 7) * if x < gw as f32 / 2.0 { 0.4 } else { 25.0 };
                if x < gw as f32 / 2.0 { 1.0 + up * 0.3 + bump } else { 5.0 + up * 60.0 + bump }
            })
            .collect()
    }

    #[test]
    fn soft_shores_are_beaches_and_steep_hard_ones_cliffs() {
        let w = world();
        let (s, gw, gh) = (32usize, 96usize, 96usize);
        let h = coast(gw, gh);
        let d = route(&h, gw, gh, 0.0);
        let acc = accumulate(&d, &vec![0.0f32; gw * gh]);
        let temp = vec![15.0f32; gw * gh];
        let moist = vec![0.3f32; gw * gh];
        let f = RegionFields { world: &w, gw, gh, s, gx0: 0, gy0: 0, margin: 1, cell_m: 610.0, seed: 1, h: &h, filled: &d.filled, recv: &d.recv, acc: &acc, temp: &temp, moist: &moist };
        let rock: Vec<RockType> = (0..gw * gh).map(|k| if k % gw < gw / 2 { RockType::Sediment } else { RockType::Granite }).collect();
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        let count = |l: Landform| land.iter().filter(|&&x| x == l).count();
        assert!(count(Landform::Beach) > 20, "beaches on the soft shore: {}", count(Landform::Beach));
        assert!(count(Landform::SeaCliff) > 20, "cliffs on the hard steep shore: {}", count(Landform::SeaCliff));
        for k in 0..gw * gh {
            if h[k] <= 0.0 {
                assert!(matches!(land[k], Landform::None | Landform::TidalFlat | Landform::Lagoon | Landform::Stack | Landform::Bar | Landform::Spit | Landform::BarrierIsland), "a sea cell carries only the sea's labels");
            }
        }
    }

    /// The cliff pass is a function of the chunk's heights alone: two chunks a tile apart
    /// shape the ground they share the same way, away from their edges.
    #[test]
    fn cliff_shaping_agrees_between_overlapping_chunks() {
        let w = world();
        let s = 16usize;
        let gw = 3 * s;
        let base = |g: &ChunkGrid| -> Vec<f32> {
            (0..gw * gw).map(|k| {
                let (ax, ay) = g.abs_cell(k % gw, k / gw);
                // A shore along a diagonal with high ground behind it.
                let d = (ax + ay) as f32 - 60.0;
                if d < 0.0 { -2.0 } else { 3.0 + d * 8.0 }
            }).collect()
        };
        let g1 = ChunkGrid { world: &w, gw, s, gx0: 1, gy0: 1, cell_m: 610.0, seed: 1 };
        let g2 = ChunkGrid { world: &w, gw, s, gx0: 2, gy0: 1, cell_m: 610.0, seed: 1 };
        let (mut h1, mut h2) = (base(&g1), base(&g2));
        let (d1, d2) = (route(&h1, gw, gw, 0.0), route(&h2, gw, gw, 0.0));
        let (a1, a2) = (accumulate(&d1, &vec![0.0; gw * gw]), accumulate(&d2, &vec![0.0; gw * gw]));
        shape_after(&mut h1, &g1, &d1, &a1);
        shape_after(&mut h2, &g2, &d2, &a2);
        for j in 6..gw - 6 {
            for i in s + 6..gw - 6 {
                assert_eq!(h1[j * gw + i].to_bits(), h2[j * gw + i - s].to_bits(), "chunks differ at ({i},{j})");
            }
        }
    }
}
