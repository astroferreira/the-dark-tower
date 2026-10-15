//! Rock at the surface, and what hardness makes of it (ROADMAP Stage 2).
//!
//! The strata: each world tile's rock stack (`region::handshake`, surface layer first,
//! thicknesses in 2 m levels) is laid as horizontal beds with a gentle dip hashed per tile,
//! anchored `TOP_M` above the tile's mean height, so a hillside cuts down through the layers and
//! the valleys show the beds under the plateau's. The tile a cell takes its beds from is its own,
//! but the border between two tiles' beds is bent by a position-keyed warp so no rock boundary
//! follows the tile grid. `rock_at` is a pure function of absolute position and height, so the
//! embark can read the same beds at any level (Stage 6; today it drapes the stack under its
//! surface, which agrees with this on the tops and not in the valleys).
//!
//! Landforms: cliffs where the slope passes a hardness-scaled threshold (hard rock holds a
//! face, soft rock slumps), scree at their feet, outcrops on convex crests of hard rock, tors on
//! old worn granite, mesas, slot canyons and hoodoos where a hard bed caps a soft one in dry
//! country, badlands on shale in the dry. Slopes are compared against thresholds scaled by
//! `sqrt(610 m / cell)` (fractal ground is gentler the coarser it is sampled): the same rule
//! reads on a 512-wide world (610 m cells) and the dev world (3,261 m).
//!
//! The helpers at the end (hashes, smoothstep, tile sampling, the min filter) serve the karst
//! and glacial passes too.

use noise::{NoiseFn, Perlin};

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::tilemap::Tilemap;
use crate::world::WorldData;

use super::{ChunkGrid, Drainage, RegionFields};

/// Height of a stack level (the embark's `Z_STEP_M`).
const LEVEL_M: f32 = 2.0;
/// The beds' top stands this far above the tile's mean height: summits are in the top bed.
const TOP_M: f32 = 60.0;
/// Dip across half a tile, metres: the beds shift by this from a tile's middle to its edge.
const DIP_MIN_M: f32 = 40.0;
const DIP_MAX_M: f32 = 300.0;
/// Bend of the border between two tiles' beds, in tiles.
const BORDER_WARP: f32 = 0.18;
/// A surface sediment bed no thicker than this (levels) is regolith, and the bed under it is the
/// surface rock (the embark lays its own soil over the stack).
const REGOLITH_LEVELS: u8 = 8;
/// World hardness under which low ground is alluvium: `Sediment` at the surface whatever the beds.
const ALLUVIUM_HARDNESS: f32 = 0.25;

/// Slope (m/m at 610 m cells) past which hard rock stands as a cliff.
const CLIFF_SLOPE: f32 = 0.38;
/// Cells of scree under a cliff.
const SCREE_CELLS: usize = 3;
/// Convexity (m above the neighbours' mean, at 610 m cells) that bares a crest of hard rock.
const OUTCROP_CONVEX_M: f32 = 10.0;
/// A mesa's top stands at least this far above the ground within `MESA_RADIUS` cells.
const MESA_RELIEF_M: f32 = 40.0;
const MESA_RADIUS: usize = 6;
/// Dry country for mesas, hoodoos and badlands.
const DRY_MOIST: f32 = 0.3;

/// The rock at the surface of every cell: the beds at the cell's own height, the regolith
/// skipped, alluvium where the world's hardness says so on low ground.
pub fn surface_rock(f: &RegionFields) -> Vec<RockType> {
    let n = f.gw * f.gh;
    let beds = Beds::new(f.world, f.s, f.seed);
    let mut out = Vec::with_capacity(n);
    for j in 0..f.gh {
        for i in 0..f.gw {
            let (ax, ay) = f.abs_cell(i, j);
            out.push(beds.surface_rock(ax as f64 + 0.5, ay as f64 + 0.5, f.h[f.idx(i, j)]));
        }
    }
    out
}

/// The beds of a world: which tile a point reads (bent borders), each tile's dip, and the rock
/// at a height. A pure function of absolute position.
pub struct Beds<'a> {
    world: &'a WorldData,
    s: usize,
    warp: Perlin,
}

impl<'a> Beds<'a> {
    pub fn new(world: &'a WorldData, s: usize, seed: u64) -> Self {
        Beds { world, s, warp: Perlin::new((seed as u32).wrapping_add(0x5EDA)) }
    }

    /// The world tile whose beds the point (absolute region cells, fractional) lies in: its own
    /// tile, the border bent by `BORDER_WARP` tiles of smooth noise.
    pub fn tile(&self, ax: f64, ay: f64) -> (usize, usize) {
        let s = self.s as f64;
        let (u, v) = (ax / s, ay / s);
        let un = u.rem_euclid(self.world.width as f64);
        let wx = u + BORDER_WARP as f64 * self.warp.get([un * 1.7 + 11.3, v * 1.7 + 7.1]);
        let wy = v + BORDER_WARP as f64 * self.warp.get([un * 1.7 + 3.3, v * 1.7 + 19.7]);
        (
            (wx.floor() as i64).rem_euclid(self.world.width as i64) as usize,
            (wy.floor() as i64).clamp(0, self.world.height as i64 - 1) as usize,
        )
    }

    /// Height (m) of the top of tile (tx, ty)'s beds at the point: the tile's mean height plus
    /// `TOP_M`, tilted by the tile's dip.
    fn top_m(&self, tile: (usize, usize), ax: f64, ay: f64) -> f32 {
        let (tx, ty) = tile;
        let datum = *self.world.heightmap.get(tx, ty);
        let theta = std::f32::consts::TAU * unit(tx as i64, ty as i64, 0xD1);
        let u = unit(tx as i64, ty as i64, 0xD2);
        let dip_m = DIP_MIN_M + (DIP_MAX_M - DIP_MIN_M) * u * u;
        let s = self.s as f64;
        // Offsets from the tile's middle in tiles; x the short way round the world.
        let mut fx = (ax / s - (tx as f64 + 0.5)) as f32;
        let w = self.world.width as f32;
        if fx > w / 2.0 { fx -= w; } else if fx < -w / 2.0 { fx += w; }
        let fy = (ay / s - (ty as f64 + 0.5)) as f32;
        datum + TOP_M + dip_m * 2.0 * (fx * theta.cos() + fy * theta.sin())
    }

    /// The bed at height `z_m` under the point, and whether it is the stack's surface bed.
    fn bed_at(&self, tile: (usize, usize), ax: f64, ay: f64, z_m: f32) -> (RockType, bool) {
        let (tx, ty) = tile;
        let stack = match self.world.handshakes.as_ref() {
            Some(hs) => &hs.get(tx, ty).tile.rock_stack,
            None => return (RockType::Sandstone, true),
        };
        let mut depth = self.top_m(tile, ax, ay) - z_m;
        let mut rock = stack.first().map(|l| l.rock_type).unwrap_or_default();
        for (k, l) in stack.iter().enumerate() {
            rock = l.rock_type;
            if depth <= l.thickness as f32 * LEVEL_M { return (rock, k == 0); }
            depth -= l.thickness as f32 * LEVEL_M;
        }
        (rock, false)
    }

    /// The rock at any height under a point: the embark's reading at a level (Stage 6).
    pub fn rock_at(&self, ax: f64, ay: f64, z_m: f32) -> RockType {
        let tile = self.tile(ax, ay);
        self.bed_at(tile, ax, ay, z_m).0
    }

    /// The rock at the surface of a point whose ground is at `z_m`: thin surface sediment is
    /// regolith (the bed under it shows), alluvium stays on soft low ground.
    pub fn surface_rock(&self, ax: f64, ay: f64, z_m: f32) -> RockType {
        let tile = self.tile(ax, ay);
        let (rock, top) = self.bed_at(tile, ax, ay, z_m);
        if rock != RockType::Sediment || !top { return rock; }
        let hardness = self.world.hardness_map.as_ref().map(|h| *h.get(tile.0, tile.1)).unwrap_or(0.5);
        if hardness < ALLUVIUM_HARDNESS && z_m < *self.world.heightmap.get(tile.0, tile.1) + TOP_M { return RockType::Sediment; }
        let Some(hs) = self.world.handshakes.as_ref() else { return rock };
        let stack = &hs.get(tile.0, tile.1).tile.rock_stack;
        match stack.first() {
            Some(l) if l.thickness <= REGOLITH_LEVELS => stack.get(1).map(|l| l.rock_type).unwrap_or(rock),
            _ => rock,
        }
    }

    /// The bed under the surface bed at the point (what a cap rock rests on), if any.
    pub fn bed_below(&self, ax: f64, ay: f64, z_m: f32) -> Option<RockType> {
        let tile = self.tile(ax, ay);
        let (rock, _) = self.bed_at(tile, ax, ay, z_m);
        let stack = &self.world.handshakes.as_ref()?.get(tile.0, tile.1).tile.rock_stack;
        let k = stack.iter().position(|l| l.rock_type == rock)?;
        stack.get(k + 1).map(|l| l.rock_type)
    }

    pub fn hardness(&self, ax: f64, ay: f64) -> f32 {
        let (tx, ty) = self.tile(ax, ay);
        self.world.hardness_map.as_ref().map(|h| *h.get(tx, ty)).unwrap_or(0.5)
    }

    pub fn stress(&self, ax: f64, ay: f64) -> f32 {
        let (tx, ty) = self.tile(ax, ay);
        *self.world.stress_map.get(tx, ty)
    }
}

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {}

pub fn classify(f: &RegionFields, rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let n = gw * gh;
    let k = scale(f.cell_m);
    let beds = Beds::new(f.world, f.s, f.seed);
    let slope = slopes(f);
    let low = min_filter(f.h, gw, gh, MESA_RADIUS);
    let thr = river_threshold(f.s);

    // Cliffs: the slope passes the rock's threshold, and so does a neighbour's (a face, not a
    // single steep cell).
    let cliff_thr = |r: RockType| CLIFF_SLOPE * k * (1.6 - 0.8 * r.hardness());
    let mut cliff = vec![false; n];
    for j in 1..gh - 1 {
        for i in 1..gw - 1 {
            let c = f.idx(i, j);
            if f.is_sea(c) || f.lake_depth(c) > 0.0 { continue; }
            let t = cliff_thr(rock[c]);
            if slope[c] < t { continue; }
            let steep_beside = [c - 1, c + 1, c - gw, c + gw, c - gw - 1, c - gw + 1, c + gw - 1, c + gw + 1]
                .iter().any(|&q| slope[q] >= t && !f.is_sea(q));
            cliff[c] = steep_beside;
        }
    }
    for c in 0..n {
        if cliff[c] && land[c] == Landform::None { land[c] = Landform::Cliff; }
    }
    // Scree: down the receiver chain from a cliff for a few cells of gentler ground.
    for c in 0..n {
        if !cliff[c] { continue; }
        let mut q = c;
        for _ in 0..SCREE_CELLS {
            let r = f.recv[q];
            if r == u32::MAX { break; }
            q = r as usize;
            if cliff[q] || f.is_sea(q) || f.lake_depth(q) > 0.0 { break; }
            if land[q] == Landform::None { land[q] = Landform::Scree; }
        }
    }

    for j in 1..gh - 1 {
        for i in 1..gw - 1 {
            let c = f.idx(i, j);
            if land[c] != Landform::None || f.is_sea(c) || f.lake_depth(c) > 0.0 { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            let (px, py) = (ax as f64 + 0.5, ay as f64 + 0.5);
            let r = rock[c];
            let hard = r.hardness();
            let h = f.h[c];
            let mean8 = (f.h[c - 1] + f.h[c + 1] + f.h[c - gw] + f.h[c + gw] + f.h[c - gw - 1] + f.h[c - gw + 1] + f.h[c + gw - 1] + f.h[c + gw + 1]) / 8.0;
            let convex = h - mean8;
            let dry = f.moist[c] < DRY_MOIST;
            let channel = f.acc[c] >= thr;

            // Tors: old worn granite uplands, at their local summits, one in two.
            if r == RockType::Granite && beds.hardness(px, py) > 0.8 && beds.stress(px, py).abs() < 0.1 && h > 300.0
                && convex > 0.0 && is_summit(f, i, j) && unit(ax, ay, 0x70) < 0.5
            {
                land[c] = Landform::Tor;
                continue;
            }
            // Cap rock over a softer bed, in the dry: mesas (flat tops over a drop), slot canyons
            // (a channel cut into the cap), hoodoos (the soft bed under the rim, in pillars).
            if dry {
                let cap = beds.bed_below(px, py, h).map(|b| b.hardness() < hard - 0.15).unwrap_or(false);
                if cap && slope[c] < 0.03 * k && h - low[c] > MESA_RELIEF_M {
                    land[c] = Landform::Mesa;
                    continue;
                }
                if cap && channel && local_relief(f, i, j, 3) > MESA_RELIEF_M {
                    land[c] = Landform::SlotCanyon;
                    continue;
                }
                if r == RockType::Shale && slope[c] > 0.1 * k {
                    land[c] = Landform::Badland;
                    continue;
                }
            }
            // Outcrops: convex crests of hard rock with little soil.
            if hard >= 0.6 && convex > OUTCROP_CONVEX_M * k && slope[c] > 0.04 * k {
                land[c] = Landform::Outcrop;
            }
        }
    }
    // Hoodoos: the soft bed just under a mesa's rim, in pillars (one cell in three).
    for j in 1..gh - 1 {
        for i in 1..gw - 1 {
            let c = f.idx(i, j);
            if land[c] != Landform::None || f.is_sea(c) { continue; }
            let r = rock[c];
            if !matches!(r, RockType::Shale | RockType::Sandstone | RockType::Sediment) || slope[c] < 0.08 * k { continue; }
            let rim = (-2i64..=2).any(|dy| (-2i64..=2).any(|dx| {
                let (x, y) = (i as i64 + dx, j as i64 + dy);
                x >= 0 && y >= 0 && (x as usize) < gw && (y as usize) < gh && land[f.idx(x as usize, y as usize)] == Landform::Mesa
            }));
            let (ax, ay) = f.abs_cell(i, j);
            if rim && unit(ax, ay, 0x71) < 0.34 { land[c] = Landform::Hoodoo; }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers shared by the rock, karst and glacial passes
// ---------------------------------------------------------------------------------------------

/// Slope thresholds are set at 610 m cells (a 512-wide world); coarser cells see gentler ground.
pub(super) fn scale(cell_m: f32) -> f32 { (610.0 / cell_m).sqrt().clamp(0.2, 2.0) }

/// Contributing area (cells) a permanent stream needs, humid (`zoom::RIVER_AREA_FRACTION`);
/// the region divides it by `0.25 + moisture`.
pub(super) fn river_threshold(s: usize) -> f32 { 0.02 * (s * s) as f32 }

pub(super) fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// A number in [0, 1) fixed by two integers and a salt.
pub(super) fn unit(a: i64, b: i64, salt: u64) -> f32 { (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0 }

pub(super) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A world field sampled bilinearly at tile coordinates (tile centres at integers, x wrapped,
/// y clamped): the coarse climate under a region cell.
pub(super) fn sample_tile(map: &Tilemap<f32>, u: f32, v: f32) -> f32 {
    let (w, h) = (map.width as i64, map.height as i64);
    let (x0, y0) = (u.floor(), v.floor());
    let (tx, ty) = (u - x0, v - y0);
    let at = |x: i64, y: i64| *map.get(x.rem_euclid(w) as usize, y.clamp(0, h - 1) as usize);
    let (x0, y0) = (x0 as i64, y0 as i64);
    (at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx) * (1.0 - ty) + (at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx) * ty
}

/// Slope (m/m) of every cell of the blended region.
pub(super) fn slopes(f: &RegionFields) -> Vec<f32> {
    let mut out = vec![0.0f32; f.gw * f.gh];
    for j in 0..f.gh {
        for i in 0..f.gw {
            out[f.idx(i, j)] = f.slope(i, j);
        }
    }
    out
}

/// Slope (m/m) of every cell of a chunk grid (the sphere's metric east-west).
pub(super) fn chunk_slopes(h: &[f32], g: &ChunkGrid) -> Vec<f32> {
    let gw = g.gw;
    let mut out = vec![0.0f32; h.len()];
    for j in 0..gw {
        let (_, v) = g.to_world(0, j);
        let lat_cos = (std::f32::consts::FRAC_PI_2 - (v + 0.5) / g.world.height as f32 * std::f32::consts::PI).cos().abs().max(0.05);
        for i in 0..gw {
            let (i0, i1) = (i.saturating_sub(1), (i + 1).min(gw - 1));
            let (j0, j1) = (j.saturating_sub(1), (j + 1).min(gw - 1));
            let dx = (h[j * gw + i1] - h[j * gw + i0]) / ((i1 - i0).max(1) as f32 * g.cell_m * lat_cos);
            let dy = (h[j1 * gw + i] - h[j0 * gw + i]) / ((j1 - j0).max(1) as f32 * g.cell_m);
            out[j * gw + i] = (dx * dx + dy * dy).sqrt();
        }
    }
    out
}

/// The lowest height within `r` cells of each cell (a separable square window).
pub(super) fn min_filter(h: &[f32], w: usize, ht: usize, r: usize) -> Vec<f32> {
    let mut row = vec![0.0f32; w * ht];
    for y in 0..ht {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            row[y * w + x] = h[y * w + a..=y * w + b].iter().cloned().fold(f32::MAX, f32::min);
        }
    }
    let mut out = vec![0.0f32; w * ht];
    for y in 0..ht {
        let (a, b) = (y.saturating_sub(r), (y + r).min(ht - 1));
        for x in 0..w {
            out[y * w + x] = (a..=b).map(|yy| row[yy * w + x]).fold(f32::MAX, f32::min);
        }
    }
    out
}

/// The highest height within `r` cells of each cell.
pub(super) fn max_filter(h: &[f32], w: usize, ht: usize, r: usize) -> Vec<f32> {
    let neg: Vec<f32> = h.iter().map(|v| -v).collect();
    min_filter(&neg, w, ht, r).into_iter().map(|v| -v).collect()
}

/// How far the ground within `r` cells rises above cell (i, j).
pub(super) fn local_relief(f: &RegionFields, i: usize, j: usize, r: usize) -> f32 {
    let mut hi = f32::MIN;
    for y in j.saturating_sub(r)..=(j + r).min(f.gh - 1) {
        for x in i.saturating_sub(r)..=(i + r).min(f.gw - 1) {
            hi = hi.max(f.h[f.idx(x, y)]);
        }
    }
    hi - f.h[f.idx(i, j)]
}

/// Cell (i, j) stands at or above all eight neighbours.
pub(super) fn is_summit(f: &RegionFields, i: usize, j: usize) -> bool {
    let h = f.h[f.idx(i, j)];
    (-1i64..=1).all(|dy| (-1i64..=1).all(|dx| {
        let (x, y) = (i as i64 + dx, j as i64 + dy);
        x < 0 || y < 0 || x as usize >= f.gw || y as usize >= f.gh || f.h[f.idx(x as usize, y as usize)] <= h
    }))
}

/// Poisson-like centres: a lattice `spacing` cells apart, each point jittered by its hash, with
/// a radius and a strength of its own. Pure in absolute position; `visit` gets the centre's
/// absolute cell, the lattice point's hash unit (for acceptance), the radius and the strength.
pub(super) fn lattice(ax0: i64, ay0: i64, w: usize, h: usize, spacing: i64, salt: u64, mut visit: impl FnMut(i64, i64, f32, f32, f32)) {
    let (px0, py0) = (ax0.div_euclid(spacing) - 1, ay0.div_euclid(spacing) - 1);
    let (px1, py1) = ((ax0 + w as i64).div_euclid(spacing) + 1, (ay0 + h as i64).div_euclid(spacing) + 1);
    for py in py0..=py1 {
        for px in px0..=px1 {
            let jx = (unit(px, py, salt ^ 0x11) * spacing as f32) as i64;
            let jy = (unit(px, py, salt ^ 0x22) * spacing as f32) as i64;
            visit(px * spacing + jx, py * spacing + jy, unit(px, py, salt ^ 0x33), unit(px, py, salt ^ 0x44), unit(px, py, salt ^ 0x55));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beds_are_a_function_of_position() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let beds = Beds::new(&world, 32, 3);
        let a = beds.rock_at(1000.5, 500.5, 120.0);
        let b = beds.rock_at(1000.5, 500.5, 120.0);
        assert_eq!(a, b);
        // Across the date line the same point reads the same beds.
        let c = beds.tile(1000.5 + 64.0 * 32.0, 500.5);
        assert_eq!(beds.tile(1000.5, 500.5), c);
        // Deeper is never a younger bed: walking down the beds only moves forward in the stack.
        let t = beds.tile(1000.5, 500.5);
        let stack = &world.handshakes.as_ref().unwrap().get(t.0, t.1).tile.rock_stack;
        let order = |r: RockType| stack.iter().position(|l| l.rock_type == r).unwrap_or(usize::MAX);
        let mut last = 0;
        for z in (-400..400).rev().step_by(10) {
            let k = order(beds.rock_at(1000.5, 500.5, z as f32));
            assert!(k >= last, "bed order broken at {z} m");
            last = k;
        }
    }

    #[test]
    fn helpers_behave() {
        assert!((0.0..1.0).contains(&unit(3, 4, 5)));
        assert_eq!(unit(3, 4, 5), unit(3, 4, 5));
        assert_ne!(unit(3, 4, 5), unit(4, 3, 5));
        let h: Vec<f32> = (0..25).map(|k| k as f32).collect();
        let lo = min_filter(&h, 5, 5, 1);
        assert_eq!(lo[12], 6.0);
        assert_eq!(lo[0], 0.0);
        let hi = max_filter(&h, 5, 5, 1);
        assert_eq!(hi[12], 18.0);
        let mut seen = 0;
        // Lattice points a cell beyond the area on each side, each jittered by up to a spacing.
        lattice(0, 0, 64, 64, 8, 9, |x, y, _, _, _| { assert!((-8..80).contains(&x) && (-8..80).contains(&y)); seen += 1; });
        assert!(seen >= 64);
    }
}
