//! Landforms at region resolution (ROADMAP Stage 2): the region's terrain gets the rock and the
//! processes the world's climate implies. Two kinds of pass, one file per process:
//!
//! - `shape_*` run inside a chunk's simulation (`zoom::Ctx::simulate_chunk`) and move heights:
//!   before the landscape evolution, what the rivers then wear (cones, lava, moraines, dunes);
//!   after it, what the final land shows (dolines, benches, terraces, cliffs cut square). They
//!   see the chunk's bordered grid and must be pure functions of world position (position-keyed
//!   noise through `ChunkGrid::to_world`), so the overlap of two chunks agrees and the blend
//!   stays seamless. Bump `SHAPE_VERSION` when a shape pass changes what it does: it keys the
//!   chunk cache on disk.
//! - `classify` runs on the blended region (with its margin) and labels every cell with a
//!   `Landform` and the `RockType` at the surface, from the fields and the same position-keyed
//!   functions, never from anything beyond the margin, so every window labels a cell the same
//!   way and the embark reads the same label from any region.
//!
//! `mod.rs` fixes only the order. Each process keeps its numbers and its notes in its own file.

pub mod rock;
pub mod karst;
pub mod glacial;
pub mod volcanic;
pub mod aeolian;
pub mod fluvial;
pub mod coast;
pub mod wet;

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::world::WorldData;

/// Keys the chunk cache: bump when any `shape_*` pass changes the heights it makes.
pub const SHAPE_VERSION: u32 = 1;

/// A chunk's bordered grid, as the shape passes see it: `gw` x `gw` cells, cell (0, 0) at the
/// top-left of world tile (`gx0`, `gy0`), `s` cells per tile.
pub struct ChunkGrid<'a> {
    pub world: &'a WorldData,
    pub gw: usize,
    pub s: usize,
    pub gx0: i64,
    pub gy0: i64,
    pub cell_m: f32,
    pub seed: u64,
}

impl<'a> ChunkGrid<'a> {
    /// World tile coordinate (fractional, tile centres at integers) of cell (i, j).
    pub fn to_world(&self, i: usize, j: usize) -> (f32, f32) {
        let s = self.s as f32;
        (self.gx0 as f32 + (i as f32 + 0.5) / s - 0.5, self.gy0 as f32 + (j as f32 + 0.5) / s - 0.5)
    }
    /// Absolute region cell of cell (i, j).
    pub fn abs_cell(&self, i: usize, j: usize) -> (i64, i64) {
        (self.gx0 * self.s as i64 + i as i64, self.gy0 * self.s as i64 + j as i64)
    }
    /// The world tile cell (i, j) lies in (x wrapped, y clamped).
    pub fn tile_of(&self, i: usize, j: usize) -> (usize, usize) {
        let (ax, ay) = self.abs_cell(i, j);
        let s = self.s as i64;
        (ax.div_euclid(s).rem_euclid(self.world.width as i64) as usize, ay.div_euclid(s).clamp(0, self.world.height as i64 - 1) as usize)
    }
    pub fn idx(&self, i: usize, j: usize) -> usize { j * self.gw + i }
}

/// Heights before the landscape evolution: build what the rivers then wear.
pub fn shape_before_erosion(h: &mut [f32], g: &ChunkGrid) {
    volcanic::shape_before(h, g);
    glacial::shape_before(h, g);
    aeolian::shape_before(h, g);
    rock::shape_before(h, g);
}

/// Heights after the landscape evolution: what the final land shows.
pub fn shape_after_erosion(h: &mut [f32], g: &ChunkGrid) {
    rock::shape_after(h, g);
    karst::shape_after(h, g);
    glacial::shape_after(h, g);
    fluvial::shape_after(h, g);
    coast::shape_after(h, g);
}

/// The blended region's fields with their margin, for `classify` and the site planner
/// (`region::sites`). Row-major `gw` x `gh`; cell (0, 0) at the top-left of world tile (`gx0`,
/// `gy0`); the window the caller asked for starts `margin` tiles in.
pub struct RegionFields<'a> {
    pub world: &'a WorldData,
    pub gw: usize,
    pub gh: usize,
    pub s: usize,
    pub gx0: i64,
    pub gy0: i64,
    /// Tiles of margin round the requested window on every side.
    pub margin: usize,
    pub cell_m: f32,
    pub seed: u64,
    /// Heights (m; sea at or below 0).
    pub h: &'a [f32],
    /// Heights with closed hollows filled to their spill level (lakes stand between the two).
    pub filled: &'a [f32],
    /// Downstream neighbour (`u32::MAX` at the sea or an outlet).
    pub recv: &'a [u32],
    /// Contributing area in cells, world rivers' inflow included.
    pub acc: &'a [f32],
    pub temp: &'a [f32],
    pub moist: &'a [f32],
}

impl<'a> RegionFields<'a> {
    pub fn to_world(&self, i: usize, j: usize) -> (f32, f32) {
        let s = self.s as f32;
        (self.gx0 as f32 + (i as f32 + 0.5) / s - 0.5, self.gy0 as f32 + (j as f32 + 0.5) / s - 0.5)
    }
    pub fn abs_cell(&self, i: usize, j: usize) -> (i64, i64) {
        (self.gx0 * self.s as i64 + i as i64, self.gy0 * self.s as i64 + j as i64)
    }
    pub fn tile_of(&self, i: usize, j: usize) -> (usize, usize) {
        let (ax, ay) = self.abs_cell(i, j);
        let s = self.s as i64;
        (ax.div_euclid(s).rem_euclid(self.world.width as i64) as usize, ay.div_euclid(s).clamp(0, self.world.height as i64 - 1) as usize)
    }
    pub fn idx(&self, i: usize, j: usize) -> usize { j * self.gw + i }
    pub fn is_sea(&self, k: usize) -> bool { self.h[k] <= 0.0 }
    pub fn lake_depth(&self, k: usize) -> f32 { if self.h[k] > 0.0 { (self.filled[k] - self.h[k]).max(0.0) } else { 0.0 } }
    /// cos of the latitude of row `j` (a cell's true width is `cell_m` times this).
    pub fn lat_cos(&self, j: usize) -> f32 {
        let v = self.gy0 as f32 + (j as f32 + 0.5) / self.s as f32;
        (std::f32::consts::FRAC_PI_2 - (v + 0.5) / self.world.height as f32 * std::f32::consts::PI).cos().abs().max(0.05)
    }
    /// Slope in m per m at cell (i, j) (central differences; the sphere's metric east-west).
    pub fn slope(&self, i: usize, j: usize) -> f32 {
        let (gw, gh) = (self.gw, self.gh);
        let (i0, i1) = (i.saturating_sub(1), (i + 1).min(gw - 1));
        let (j0, j1) = (j.saturating_sub(1), (j + 1).min(gh - 1));
        let dx = (self.h[self.idx(i1, j)] - self.h[self.idx(i0, j)]) / ((i1 - i0).max(1) as f32 * self.cell_m * self.lat_cos(j));
        let dy = (self.h[self.idx(i, j1)] - self.h[self.idx(i, j0)]) / ((j1 - j0).max(1) as f32 * self.cell_m);
        (dx * dx + dy * dy).sqrt()
    }
}

/// Label every cell of the blended region with its landform and its surface rock.
pub fn classify(f: &RegionFields) -> (Vec<Landform>, Vec<RockType>) {
    let n = f.gw * f.gh;
    let rock = rock::surface_rock(f);
    let mut land = vec![Landform::None; n];
    rock::classify(f, &rock, &mut land);
    karst::classify(f, &rock, &mut land);
    glacial::classify(f, &rock, &mut land);
    volcanic::classify(f, &rock, &mut land);
    aeolian::classify(f, &rock, &mut land);
    fluvial::classify(f, &rock, &mut land);
    coast::classify(f, &rock, &mut land);
    wet::classify(f, &rock, &mut land);
    (land, rock)
}
