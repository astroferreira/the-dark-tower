//! Playable areas: Dwarf Fortress style 3D local maps generated from a zoomed region.
//!
//! An embark is `LOCAL_SIZE` x `LOCAL_SIZE` tiles of `TILE_M` metres (person-sized) with
//! z-levels of `Z_STEP_M` metres. Everything is derived from the region it sits in:
//!
//! * **Surface**: bicubic elevation from the zoomed region plus metre-scale relief whose
//!   amplitude follows the local slope (rough mountainsides, gentle plains).
//! * **Water**: river channels follow the region's river cells (meandering centrelines, width
//!   from the region's hydrology, carved bed, water filled to level); lakes keep their water
//!   level; the sea fills everything below 0 m.
//! * **Underground**: soil (sand / clay / gravel / loam by setting, thinner on slopes) over the
//!   world tile's rock stack from the region handshake, with gently undulating strata.
//! * **Life**: individual trees, shrubs, grass and boulders; species and density come from the
//!   local climate through the world's biome classifier.
//!
//! All noise and object placement use absolute world positions, so the same spot always
//! generates the same way (neighbouring embarks would match).

pub mod structures;
pub mod wildlife;
pub mod site;
pub mod places;
pub mod caverns;

use noise::{NoiseFn, Perlin};

use crate::climate::biomes::Biome;
use crate::erosion::materials::RockType;
use crate::region::zoom::ZoomRegion;
use crate::world::WorldData;

/// Tiles per side of a playable area (DF's 4x4 embark of 48x48).
pub const LOCAL_SIZE: usize = 192;
/// Horizontal size of a tile (m).
pub const TILE_M: f32 = 2.0;
/// Vertical size of a z-level (m).
pub const Z_STEP_M: f32 = 2.0;
/// Solid z-levels kept below the lowest surface point.
const DEPTH_BELOW: i32 = 52;
/// Empty z-levels kept above the highest surface point.
const AIR_ABOVE: i32 = 8;
/// Share of a world tile's area a catchment needs to carry a creek at person scale (a quarter
/// of the region's river threshold).
const CREEK_AREA_FRACTION: f32 = 0.005;
/// Mean annual temperature (C) below which lakes and rivers freeze over.
const FREEZE_TEMP_C: f32 = -3.0;
/// Water depth units per full z-level (DF uses 1-7).
pub const WATER_FULL: u8 = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Material {
    Air,
    Soil,
    Sand,
    Clay,
    Gravel,
    Snow,
    Ice,
    Rock(RockType),
    /// Constructed: timber.
    Wood,
    /// Constructed: dressed stone blocks.
    Block(RockType),
    /// Ore vein in the rock.
    Ore(crate::history::civilizations::economy::ResourceType),
    /// The magma sea at the bottom of the world, under volcanic ground (a liquid: `water` 7).
    Magma,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Open space (air or water).
    Empty,
    /// Walkable top of the ground.
    Floor,
    /// A floor next to ground one level higher: leads up.
    Ramp,
    /// Solid ground.
    Wall,
    /// A stair cut in the rock (DF's up/down stair): stood on like a floor, and open to the
    /// stair above and below it, so walkers climb from level to level (`colony::nav::path3`).
    Stair,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeKind {
    Broadleaf,
    Conifer,
    Jungle,
    Palm,
    Acacia,
    Dead,
    /// A fungus tree of the caverns (a tower of cap and stalk).
    Fungus,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plant {
    None,
    Grass,
    Shrub,
    Tree(TreeKind),
    /// Cultivated crop row (variety 0-2).
    Crop(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub shape: Shape,
    pub material: Material,
    /// Water depth in the cell, 0..=WATER_FULL.
    pub water: u8,
    pub plant: Plant,
    pub boulder: bool,
}

impl Cell {
    const AIR: Cell = Cell { shape: Shape::Empty, material: Material::Air, water: 0, plant: Plant::None, boulder: false };
}

#[derive(Clone)]
pub struct LocalMap {
    pub width: usize,
    pub height: usize,
    /// Number of z-levels; z = 0 is the deepest.
    pub depth: usize,
    /// Elevation (m) of the bottom of z-level 0.
    pub z_min_m: f32,
    /// [z][y][x]
    pub cells: Vec<Cell>,
    /// Floor z-level of every column.
    pub surface_z: Vec<i32>,
    /// Surface elevation (m) of every column, for shading.
    pub surface_m: Vec<f32>,
    /// World tile this area lies in.
    pub world_tile: (usize, usize),
    /// Biome at the centre (for display).
    pub biome: Biome,
    /// Signs of animal life on each column's surface (trails, burrows, nests, dens, bones).
    pub features: Vec<wildlife::Feature>,
    /// Who lies in each grave (`site::furnish`): (x, y, the words on it).
    pub graves: Vec<(usize, usize, String)>,
    /// Which roofed house covers each column (index + 1 into `houses`; 0 = open sky).
    pub roofs: Vec<u32>,
    /// The standing houses' roofs, for drawing the surface from above.
    pub houses: Vec<RoofPlan>,
    /// The world tile's mean temperature (°C) in spring, summer, autumn and winter.
    pub season_temps: [f32; 4],
    /// What the site held within reach before it was furnished: water, trees, stone, bushes.
    pub found: [usize; 4],
    /// What furnishing added so the site is livable ("a spring", "a grove", ...).
    pub furnished: Vec<String>,
    /// Game that grazes here, from the tile's ecology: (species, head).
    pub game: Vec<(String, u32)>,
    /// Places in the rock (lairs, tombs, old mines, caves, a deep cavern), each with its cause.
    pub places: Vec<places::UnderPlace>,
    /// Each column's cavern layers (`caverns.rs`): (floor z, top open z), (-1, -1) where none.
    pub cavern_z: Vec<[(i16, i16); caverns::LAYERS]>,
    /// The cavern layers under this embark, with what lives there.
    pub caverns: Vec<caverns::Cavern>,
    /// The top level of the magma sea at the bottom of the map.
    pub magma_top: Option<i32>,
    /// Where a pipe of magma rises from the sea toward the surface, under volcanic ground.
    pub magma_pipe: Option<(u16, u16)>,
    /// Levels (absolute z, inclusive) of wet permeable rock: an aquifer (`is_aquifer`).
    pub aquifer: Option<(i32, i32)>,
    /// Width (m) of the widest river channel that runs through the embark (0: none).
    pub river_m: f32,
    /// Levels the map was made deeper than usual to hold its caverns whole (0 mostly); ore seams,
    /// gems and the magma sea count their levels from the usual bottom (`z - deepened`).
    pub deepened: i32,
}

/// A house's roof seen from above, in cell units: a pitched roof whose ridge runs along the
/// house's long axis through its centre.
#[derive(Clone, Copy, Debug)]
pub struct RoofPlan {
    pub cx: f32,
    pub cy: f32,
    /// Unit vector along the ridge.
    pub axis: (f32, f32),
    /// Half the house's width across the ridge (cells).
    pub half_width: f32,
    /// Stone houses have tiled roofs; timber and earth ones thatch.
    pub stone: bool,
}

impl LocalMap {
    /// A cell of the aquifer: within its levels, of rock or ground that lets water through
    /// (sandstone, limestone, loose sediment, sand, gravel, soil).
    pub fn is_aquifer(&self, x: usize, y: usize, z: usize) -> bool {
        let Some((lo, hi)) = self.aquifer else { return false };
        if (z as i32) < lo || z as i32 > hi || x >= self.width || y >= self.height || z >= self.depth { return false; }
        use crate::erosion::materials::RockType as R;
        matches!(self.cell(x, y, z).material, Material::Rock(R::Sandstone | R::Limestone | R::Sediment) | Material::Sand | Material::Gravel | Material::Soil)
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.height + y) * self.width + x
    }
    pub fn cell(&self, x: usize, y: usize, z: usize) -> &Cell {
        &self.cells[self.idx(x, y, z)]
    }
    /// Elevation (m) of the bottom of z-level `z`.
    pub fn z_elevation(&self, z: i32) -> f32 {
        self.z_min_m + z as f32 * Z_STEP_M
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm(noise: &Perlin, x: f64, y: f64, octaves: u32) -> f32 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([x * f + 0.31, y * f + 0.77]);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    (sum / norm) as f32
}

fn hash(x: i64, y: i64, salt: u64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

/// Bicubic sample of a region field at fractional cell coordinates (cell centres at x + 0.5).
fn bicubic(f: &[f32], w: usize, h: usize, x: f64, y: f64) -> f32 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
    let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
    let at = |dx: i64, dy: i64| {
        let xx = (x0 + dx).clamp(0, w as i64 - 1) as usize;
        let yy = (y0 + dy).clamp(0, h as i64 - 1) as usize;
        f[yy * w + xx]
    };
    let row = |dy: i64| catmull(at(-1, dy), at(0, dy), at(1, dy), at(2, dy), tx);
    catmull(row(-1), row(0), row(1), row(2), ty)
}

fn bilinear(f: &[f32], w: usize, h: usize, x: f64, y: f64) -> f32 {
    let (fx, fy) = ((x - 0.5).clamp(0.0, w as f64 - 1.0), (y - 0.5).clamp(0.0, h as f64 - 1.0));
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
    let top = f[y0 * w + x0] * (1.0 - tx) + f[y0 * w + x1] * tx;
    let bot = f[y1 * w + x0] * (1.0 - tx) + f[y1 * w + x1] * tx;
    top * (1.0 - ty) + bot * ty
}

/// Distance from a point to a segment, and the position along it (0 at `a`, 1 at `b`).
fn seg_dist(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a.0 + dx * t - px, a.1 + dy * t - py);
    ((qx * qx + qy * qy).sqrt(), t)
}

/// Trees per floor tile and the species mix for a biome.
/// Vegetation the world's biome dictates (None: let the local climate decide).
fn world_vegetation(b: crate::biomes::ExtendedBiome) -> Option<(f32, f32, &'static [TreeKind])> {
    use crate::biomes::ExtendedBiome::*;
    use TreeKind::*;
    Some(match b {
        DeadForest => (0.25, 0.01, &[Dead]),
        PetrifiedForest => (0.15, 0.0, &[Dead]),
        Ashlands | VolcanicWasteland => (0.01, 0.0, &[Dead]),
        SaltFlats | CrystalWasteland => (0.0, 0.0, &[Dead]),
        BoneFields => (0.004, 0.02, &[Dead]),
        AncientGrove => (0.6, 0.05, &[Broadleaf, Broadleaf, Conifer]),
        TitanBones => (0.01, 0.03, &[Acacia]),
        MonsoonForest => (0.3, 0.08, &[Broadleaf, Jungle]),
        MediterraneanShrubland => (0.04, 0.15, &[Broadleaf]),
        Foothills => (0.08, 0.05, &[Conifer, Broadleaf]),
        Swamp | MangroveSaltmarsh => (0.15, 0.08, &[Broadleaf, Jungle]),
        Marsh => (0.01, 0.06, &[Broadleaf]),
        Bog => (0.03, 0.05, &[Conifer]),
        MushroomForest | BioluminescentForest | CrystalForest => (0.3, 0.06, &[Broadleaf, Conifer]),
        // Water tiles: the local climate decides the shore.
        DeepOcean | Ocean | CoastalWater | HighlandLake | CraterLake | Lagoon | AcidLake | LavaLake | FrozenLake | BioluminescentWater | CoralPlateau => return None,
        other => vegetation(other.parent_biome()),
    })
}

fn vegetation(biome: Biome) -> (f32, f32, &'static [TreeKind]) {
    use Biome::*;
    use TreeKind::*;
    // (tree density, shrub density, species)
    match biome {
        TropicalRainforest => (0.50, 0.10, &[Jungle, Jungle, Broadleaf]),
        TropicalForest => (0.35, 0.10, &[Broadleaf, Jungle]),
        Savanna => (0.03, 0.06, &[Acacia]),
        TemperateRainforest => (0.45, 0.08, &[Conifer, Conifer, Broadleaf]),
        TemperateForest => (0.35, 0.07, &[Broadleaf, Broadleaf, Broadleaf, Conifer]),
        BorealForest | SubalpineForest => (0.35, 0.05, &[Conifer]),
        MontaneForest | CloudForest => (0.38, 0.08, &[Broadleaf, Conifer]),
        TemperateGrassland => (0.008, 0.03, &[Broadleaf]),
        AlpineMeadow | Paramo => (0.003, 0.06, &[Conifer]),
        Tundra | AlpineTundra => (0.0, 0.05, &[Conifer]),
        Desert => (0.002, 0.01, &[Dead]),
        _ => (0.0, 0.0, &[Broadleaf]),
    }
}

/// A river piece from cell centre `a` to its downstream neighbour `b`, with the channel width
/// (m) at each end.
struct RiverSeg {
    a: (f64, f64),
    b: (f64, f64),
    wa: f32,
    wb: f32,
}

/// River segments (in region cell coordinates) near a point: each river cell links to its
/// steepest-descent wet neighbour (river, lake or sea). Sources (no river flowing in) start at
/// zero width so streams taper in instead of appearing at full width.
/// The width (m) of the channel an embark draws through region cell `k`, 0 where none: at
/// person scale rivers continue upstream as creeks, so any cell with a quarter of the region's
/// river catchment carries water, its width from hydraulic geometry.
pub fn channel_width(region: &ZoomRegion, k: usize) -> f32 {
    let s = (region.params.cells_per_tile.max(8) & !1) as f32;
    let cell_km2 = (region.cell_m / 1000.0).powi(2);
    let threshold = CREEK_AREA_FRACTION * s * s / (0.25 + region.moisture[k]);
    if region.elevation_m[k] <= 0.0 || region.lake_depth_m[k] > 0.0 || region.drainage_cells[k] < threshold {
        0.0
    } else {
        // The region's own rule (0.8 sqrt(A km2), A shrinking toward the poles): the world's
        // discharge carried down to the embark.
        0.8 * (region.drainage_cells[k] * cell_km2 * region.lat_cos(k / region.width)).sqrt()
    }
}

/// The world tiles a point (absolute metres) takes coarse values from, with weights: its own tile,
/// and within `blend` metres of a border the tile across it too (half and half at the border).
/// A function of the point alone, so a value blended this way is the same at a place whichever
/// embark holds it; deep inside a tile it is the tile's own value exactly.
fn tile_weights(mx: f64, my: f64, tile_m: f64, blend: f64, world_w: usize, world_h: usize) -> [((usize, usize), f32); 4] {
    let axis = |v: f64| -> [(i64, f32); 2] {
        let u = v / tile_m;
        let i = u.floor();
        let (dl, dr) = ((u - i) * tile_m, (1.0 - (u - i)) * tile_m);
        let i = i as i64;
        if dl < blend { let w = (0.5 + 0.5 * dl / blend) as f32; [(i, w), (i - 1, 1.0 - w)] }
        else if dr < blend { let w = (0.5 + 0.5 * dr / blend) as f32; [(i, w), (i + 1, 1.0 - w)] }
        else { [(i, 1.0), (i, 0.0)] }
    };
    let (xs, ys) = (axis(mx), axis(my));
    let mut out = [((0, 0), 0.0); 4];
    for (a, &(tx, wx)) in xs.iter().enumerate() {
        for (b, &(ty, wy)) in ys.iter().enumerate() {
            out[a * 2 + b] = ((tx.rem_euclid(world_w as i64) as usize, ty.clamp(0, world_h as i64 - 1) as usize), wx * wy);
        }
    }
    out
}

/// The width (m) of the world's river on tile (x, y) from the world's discharge (its flow
/// accumulation, as the zoomed region carves it: 0.8 sqrt(A km2)); 0 where the world map has no
/// river on the tile.
pub fn world_river_width(world: &WorldData, x: usize, y: usize) -> f32 {
    if !world.water_body_map.get(x, y).is_river() { return 0.0; }
    let Some(flow) = world.flow_accumulation.as_ref() else { return 0.0 };
    let cell_km = 40_075.0 / world.width as f32;
    let lat = (std::f32::consts::FRAC_PI_2 - (y as f32 + 0.5) / world.height as f32 * std::f32::consts::PI).cos().abs().max(0.05);
    0.8 * (*flow.get(x, y) * cell_km * cell_km * lat).sqrt()
}

/// A point on a river's bank near the region point (x, y): candidates on a 25 m grid within 0.6
/// region cells are tested the way `generate_local` draws channels (meander warp, then the
/// distance to each segment less half its width), and the one 20-60 m from the water's edge
/// nearest (x, y) wins, so long as the land goes on behind it (of eight points 150 m out, five
/// are dry: not a spit or an island in a great river). None where no channel runs near.
pub fn bank_near(region: &ZoomRegion, x: f64, y: f64) -> Option<(f64, f64)> {
    let segs = river_segments(region, x, y, 2);
    if segs.is_empty() { return None; }
    let seed = region.params.seed as u32;
    let meander = Perlin::new(seed.wrapping_add(703));
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    let cell_m = region.cell_m as f64;
    let (ox_m, oy_m) = ((region.world_x0 * s) as f64 * cell_m, (region.world_y0 * s) as f64 * cell_m);
    // Distance (m) from a point to the nearest channel's edge (negative inside), and whether it
    // lies in a lake or the sea.
    let edge = |zx: f64, zy: f64| -> (f64, bool) {
        let (mx, my) = (ox_m + zx * cell_m, oy_m + zy * cell_m);
        let wx = zx + 0.30 * fbm(&meander, mx / 700.0, my / 700.0, 3) as f64;
        let wy = zy + 0.30 * fbm(&meander, mx / 700.0 + 19.0, my / 700.0, 3) as f64;
        let mut edge = f64::MAX;
        for seg in &segs {
            let (d, t) = seg_dist(wx, wy, seg.a, seg.b);
            let width = (seg.wa + (seg.wb - seg.wa) * t as f32) as f64;
            if width < TILE_M as f64 { continue; }
            edge = edge.min(d * cell_m - width * 0.5);
        }
        let k = (wy as i64).clamp(0, region.height as i64 - 1) as usize * region.width + (wx as i64).clamp(0, region.width as i64 - 1) as usize;
        (edge, region.lake_depth_m[k] > 0.0 || region.elevation_m[k] <= 0.0)
    };
    let step = 25.0 / cell_m;
    let n = (0.6 / step).ceil() as i64;
    let behind = 150.0 / cell_m;
    let mut cands: Vec<(f64, (f64, f64))> = Vec::new();
    for j in -n..=n {
        for i in -n..=n {
            let (zx, zy) = (x + i as f64 * step, y + j as f64 * step);
            let (e, wet) = edge(zx, zy);
            if wet || !(20.0..=60.0).contains(&e) { continue; }
            cands.push(((zx - x).hypot(zy - y), (zx, zy)));
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    cands.into_iter().map(|c| c.1).find(|&(zx, zy)| {
        (0..8).filter(|&q| {
            let a = q as f64 * std::f64::consts::FRAC_PI_4;
            let (e, wet) = edge(zx + behind * a.cos(), zy + behind * a.sin());
            e > 0.0 && !wet
        }).count() >= 5
    })
}

fn river_segments(region: &ZoomRegion, cx: f64, cy: f64, radius: i64) -> Vec<RiverSeg> {
    let (w, h) = (region.width as i64, region.height as i64);
    let channel_width = |k: usize| -> f32 { channel_width(region, k) };
    let wet = |x: i64, y: i64| {
        let k = (y * w + x) as usize;
        channel_width(k) > 0.0 || region.lake_depth_m[k] > 0.0 || region.elevation_m[k] <= 0.0
    };
    // Follow the region's own drainage links: across flat, lake-filled reaches its routing
    // differs from steepest descent, and recomputing it would break streams apart.
    let receiver = |x: i64, y: i64| -> Option<(i64, i64)> {
        let r = region.receiver[(y * w + x) as usize];
        if r == u32::MAX { None } else { Some(((r as usize % region.width) as i64, (r as usize / region.width) as i64)) }
    };
    let r = radius + 1;
    let (x0, x1) = ((cx as i64 - r).max(0), (cx as i64 + r).min(w - 1));
    let (y0, y1) = ((cy as i64 - r).max(0), (cy as i64 + r).min(h - 1));
    // Which cells have a river flowing into them (within a slightly larger window).
    let mut fed = std::collections::HashSet::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if channel_width((y * w + x) as usize) > 0.0 {
                if let Some(rc) = receiver(x, y) { fed.insert(rc); }
            }
        }
    }
    let mut segs = Vec::new();
    for y in (cy as i64 - radius).max(0)..=(cy as i64 + radius).min(h - 1) {
        for x in (cx as i64 - radius).max(0)..=(cx as i64 + radius).min(w - 1) {
            let width = channel_width((y * w + x) as usize);
            if width <= 0.0 { continue; }
            let Some((nx, ny)) = receiver(x, y) else { continue };
            let down = channel_width((ny * w + nx) as usize);
            segs.push(RiverSeg {
                a: (x as f64 + 0.5, y as f64 + 0.5),
                b: (nx as f64 + 0.5, ny as f64 + 0.5),
                // Headwater creeks start narrow.
                wa: if fed.contains(&(x, y)) { width } else { width * 0.4 },
                wb: if down > 0.0 { down } else { width },
            });
        }
    }
    segs
}

/// An ore that can occur around an embark: strength 0..1 falls with distance and grows with
/// richness.
struct VeinSource {
    kind: crate::history::civilizations::economy::ResourceType,
    strength: f32,
    noise: Perlin,
}

fn vein_sources(world: &WorldData, tile: (usize, usize), params: &crate::region::zoom::ZoomParams) -> Vec<VeinSource> {
    let res = world.resources();
    let mut best: Vec<(crate::history::civilizations::economy::ResourceType, f32)> = Vec::new();
    for d in res.deposits_near(tile.0, tile.1, 3, world.width) {
        let dx = (d.x as i64 - tile.0 as i64).abs();
        let dx = dx.min(world.width as i64 - dx) as f32;
        let dist = (dx * dx + (d.y as f32 - tile.1 as f32).powi(2)).sqrt();
        let strength = d.richness as f32 / (1.0 + dist * 1.2);
        match best.iter_mut().find(|b| b.0 == d.kind) {
            Some(b) => b.1 = b.1.max(strength),
            None => best.push((d.kind, strength)),
        }
    }
    best.into_iter()
        .map(|(kind, strength)| VeinSource { kind, strength, noise: Perlin::new((params.seed as u32).wrapping_add(900 + kind as u32 * 7)) })
        .collect()
}

/// Which ore, if any, fills this rock cell: thin, mostly horizontal seams (like real veins and
/// beds) where the noise field rises above a threshold that falls with source strength.
fn ore_vein(veins: &[VeinSource], rock: RockType, mx: f64, my: f64, z: i32) -> Option<crate::history::civilizations::economy::ResourceType> {
    for v in veins {
        let hosts = crate::lore::resources::host_rocks(v.kind);
        if !hosts.is_empty() && !hosts.contains(&rock) { continue; }
        let n = v.noise.get([mx / 38.0, my / 38.0, z as f64 * 0.55]) as f32;
        let thr = 0.74 - 0.14 * v.strength.min(2.0);
        if n > thr { return Some(v.kind); }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------------------------

/// Generate the playable area centred on region cell coordinates (`cx`, `cy`).
pub fn generate_local(world: &WorldData, region: &ZoomRegion, lore: Option<&crate::lore::RegionLore>, cx: f64, cy: f64) -> LocalMap {
    let (n, rw, rh) = (LOCAL_SIZE, region.width, region.height);
    let cell_m = region.cell_m as f64;
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    let tile_cells = TILE_M as f64 / cell_m;
    // Absolute position of region cell (0,0) in metres, for position-based noise and hashing.
    let (ox_m, oy_m) = ((region.world_x0 * s) as f64 * cell_m, (region.world_y0 * s) as f64 * cell_m);
    let world_tile = (
        ((region.world_x0 * s + cx as i64).div_euclid(s)).rem_euclid(world.width as i64) as usize,
        ((region.world_y0 * s + cy as i64).div_euclid(s)).clamp(0, world.height as i64 - 1) as usize,
    );
    let hs = world.handshakes.as_ref().map(|h| h.get(world_tile.0, world_tile.1).tile.clone()).unwrap_or_default();
    // The world tile's soil (`world.soils()`): its kind picks the soil material, its depth the
    // number of soil levels before rock.
    let (soil_kind, soil_depth_m) = {
        let s = world.soils();
        (*s.kind.get(world_tile.0, world_tile.1), *s.depth_m.get(world_tile.0, world_tile.1))
    };

    let seed = region.params.seed as u32;
    let relief_noise = Perlin::new(seed.wrapping_add(701));
    let ridge_noise = Perlin::new(seed.wrapping_add(702));
    let meander = Perlin::new(seed.wrapping_add(703));
    let patch_noise = Perlin::new(seed.wrapping_add(704));
    let strata_noise = Perlin::new(seed.wrapping_add(705));

    let segments = river_segments(region, cx, cy, 3);

    // Mountains at human scale: on a high or rugged tile, terraced relief (ledges and faces
    // several levels high, a slope across the embark), its height from the tile's ruggedness.
    // Each world tile's terrace height; a column takes its own tile's, blended with the tile
    // across a border within 2 km of it (`tile_weights`), so the hillsides of two embarks meet.
    let amp_of = |(tx, ty): (usize, usize)| -> f32 {
        let mut lo = f32::MAX; let mut hi = f32::MIN;
        for dy in -1i64..=1 { for dx in -1i64..=1 {
            let (x, y) = ((tx as i64 + dx).rem_euclid(world.width as i64) as usize, (ty as i64 + dy).clamp(0, world.height as i64 - 1) as usize);
            let e = *world.heightmap.get(x, y);
            lo = lo.min(e); hi = hi.max(e);
        } }
        let here = *world.heightmap.get(tx, ty);
        let rugged = (hi - lo).max(0.0);
        if here > 600.0 || rugged > 500.0 { (rugged / 40.0 + (here - 600.0).max(0.0) / 120.0).clamp(10.0, 60.0) } else if here > 150.0 && rugged > 150.0 { (rugged / 25.0).clamp(6.0, 20.0) } else { 0.0 }
    };
    let tile_m = s as f64 * cell_m;
    let blend = (tile_m / 4.0).min(2000.0);
    let mountain_at = |mx: f64, my: f64| -> f32 {
        tile_weights(mx, my, tile_m, blend, world.width, world.height).iter().filter(|t| t.1 > 0.0).map(|&(t, w)| w * amp_of(t)).sum()
    };
    let mountain_noise = Perlin::new(seed.wrapping_add(706));
    let terrace = |m: f32, mx: f64, my: f64| -> f32 {
        if m <= 0.0 { return 0.0; }
        // A broad hillside (large-scale noise plus a tilt), cut into terraces: most of each step
        // a ledge, its last fifth a face.
        let f = (0.5 + 0.5 * fbm(&mountain_noise, mx / 420.0, my / 420.0, 4) as f32 + 0.25 * ((mx / 384.0) as f32).sin()).clamp(0.0, 1.0);
        let k = 7.0;
        let step = (f * k).floor();
        let frac = f * k - step;
        (step + smoothstep(0.78, 1.0, frac)) / k * m
    };

    // Ore deposits within a couple of world tiles feed veins in the rock below.
    let veins = vein_sources(world, world_tile, &region.params);

    // Per-column surface, water level and climate.
    struct Col { e: f32, base: f32, water_level: Option<f32>, temp: f32, moist: f32, slope: f32, river_d: f32, river_hw: f32, river_w: f32, sea: bool }
    let mut cols: Vec<Col> = Vec::with_capacity(n * n);
    let mut mountains = 0.0f32;
    for j in 0..n {
        for i in 0..n {
            let zx = cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells;
            let zy = cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells;
            let (mx, my) = (ox_m + zx * cell_m, oy_m + zy * cell_m);
            let base = bicubic(&region.elevation_m, rw, rh, zx, zy);
            let gx = (bicubic(&region.elevation_m, rw, rh, zx + 0.5, zy) - bicubic(&region.elevation_m, rw, rh, zx - 0.5, zy)) / cell_m as f32;
            let gy = (bicubic(&region.elevation_m, rw, rh, zx, zy + 0.5) - bicubic(&region.elevation_m, rw, rh, zx, zy - 0.5)) / cell_m as f32;
            let slope = (gx * gx + gy * gy).sqrt();
            let temp = bilinear(&region.temperature_c, rw, rh, zx, zy);
            let moist = bilinear(&region.moisture, rw, rh, zx, zy);

            // Distance to the nearest river centreline, with a meander warp of the query point.
            let wx = zx + 0.30 * fbm(&meander, mx / 700.0, my / 700.0, 3) as f64;
            let wy = zy + 0.30 * fbm(&meander, mx / 700.0 + 19.0, my / 700.0, 3) as f64;
            // Nearest edge of any channel (width varies along each segment).
            let (mut river_d, mut river_w, mut edge) = (f32::MAX, 0.0f32, f32::MAX);
            for seg in &segments {
                let (d, t) = seg_dist(wx, wy, seg.a, seg.b);
                let width = seg.wa + (seg.wb - seg.wa) * t as f32;
                let d = (d * cell_m) as f32;
                if d - width * 0.5 < edge { edge = d - width * 0.5; river_d = d; river_w = width; }
            }
            let hw = (river_w * 0.5).max(TILE_M * 0.6);
            if river_w < TILE_M * 0.5 { river_w = 0.0; } // too thin to be a channel yet

            // Metre-scale relief, stronger on slopes, calm around rivers.
            let mut amp = (0.8 + 35.0 * slope).min(40.0);
            if river_w > 0.0 { amp *= smoothstep(hw, hw + 30.0, river_d); }
            let rough = fbm(&relief_noise, mx / 90.0, my / 90.0, 5) * 2.0;
            let ridges = 1.0 - (fbm(&ridge_noise, mx / 140.0, my / 140.0, 4) * 2.0).abs();
            let steep = smoothstep(0.15, 0.6, slope);
            let mut e = base + amp * ((1.0 - steep) * rough + steep * (ridges - 0.5) * 2.0);
            // The mountain's own relief (calm by rivers).
            let calm = if river_w > 0.0 { smoothstep(hw, hw + 40.0, river_d) } else { 1.0 };
            let m_amp = mountain_at(mx, my);
            mountains = mountains.max(m_amp);
            e += terrace(m_amp, mx, my) * calm;

            let mut water_level = None;
            let bank = (hw * 0.8).max(2.0);
            if river_w > 0.0 && river_d < hw + bank {
                // Water sits a metre below the smooth valley floor; the bed is a parabola.
                let wl = base - 1.0;
                let depth = (0.3 * river_w.powf(0.6)).clamp(0.8, 16.0);
                if river_d < hw {
                    e = wl - depth * (1.0 - (river_d / hw).powi(2));
                    water_level = Some(wl);
                } else {
                    let t = smoothstep(hw, hw + bank, river_d);
                    e = (wl + 0.5) * (1.0 - t) + e.max(wl + 0.5) * t;
                }
            }
            // Lakes: the nearest region cell decides (warped), keeping its water level.
            let (lx, ly) = ((wx as i64).clamp(0, rw as i64 - 1) as usize, (wy as i64).clamp(0, rh as i64 - 1) as usize);
            let lk = ly * rw + lx;
            if region.lake_depth_m[lk] > 0.0 {
                let level = region.elevation_m[lk] + region.lake_depth_m[lk];
                let bed = level - bilinear(&region.lake_depth_m, rw, rh, zx, zy).max(1.0);
                e = e.min(bed);
                water_level = Some(level);
            }
            let sea = base <= 0.0 || e < 0.0 && water_level.is_none() && base < 2.0;
            if sea {
                water_level = Some(0.0);
            }
            cols.push(Col { e, base, water_level, temp, moist, slope, river_d, river_hw: if river_w > 0.0 { hw } else { 0.0 }, river_w, sea });
        }
    }

    let lo = cols.iter().map(|c| c.e).fold(f32::MAX, f32::min);
    let hi = cols.iter().map(|c| c.e.max(c.water_level.unwrap_or(f32::MIN))).fold(f32::MIN, f32::max);
    // The caverns are planned first, in absolute levels, from coarse fields and absolute-position
    // noise (`caverns::plan`), so they meet across embarks.
    let abs = |i: usize, j: usize| (ox_m + (cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m, oy_m + (cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m);
    let fields: Vec<([f32; caverns::LAYERS], f32)> = {
        // The coarse values the caverns take from the world tiles (the rock's openness, the water
        // table), blended over the last 2 km before a tile border so they meet across it.
        let mut by_tile: Vec<((usize, usize), ([f32; caverns::LAYERS], f32))> = Vec::new();
        let mut tile_field = |t: (usize, usize)| -> ([f32; caverns::LAYERS], f32) {
            if let Some(v) = by_tile.iter().find(|v| v.0 == t) { return v.1; }
            let v = if t == world_tile { caverns::tile_fields(&hs) } else {
                caverns::tile_fields(&world.handshakes.as_ref().map(|h| h.get(t.0, t.1).tile.clone()).unwrap_or_default())
            };
            by_tile.push((t, v));
            v
        };
        (0..n * n).map(|c| {
            let (mx, my) = abs(c % n, c / n);
            let (mut open, mut wet) = ([0.0f32; caverns::LAYERS], 0.0f32);
            for (t, w) in tile_weights(mx, my, tile_m, blend, world.width, world.height) {
                if w <= 0.0 { continue; }
                let (o, wt) = tile_field(t);
                for k in 0..caverns::LAYERS { open[k] += w * o[k]; }
                wet += w * wt;
            }
            (open, wet)
        }).collect()
    };
    let runs = {
        let refs: Vec<f32> = cols.iter().map(|c| c.base).collect();
        // Each column's ground level (absolute), or the ice over frozen water: the caverns keep
        // their roof under it.
        let ground: Vec<i32> = cols.iter().map(|c| {
            let sz = (c.e / Z_STEP_M).floor() as i32;
            match c.water_level {
                Some(level) if c.temp < FREEZE_TEMP_C => sz.max((level / Z_STEP_M).ceil() as i32 - 1),
                _ => sz,
            }
        }).collect();
        caverns::plan(&refs, &ground, &fields, &abs, n, seed)
    };
    // Deep enough to hold every cavern whole (its floor above the map's bottom level), so the
    // map's floor never clips a layer where a neighbouring embark's would not.
    let lowest_floor = runs.iter().flat_map(|r| r.iter().flatten().map(|r| r.floor)).min().unwrap_or(i32::MAX);
    let z_ground = ((lo / Z_STEP_M).floor() - DEPTH_BELOW as f32) * Z_STEP_M;
    let z_min_m = z_ground.min((lowest_floor - 1) as f32 * Z_STEP_M);
    // Levels added under the usual bottom for the caverns: what is keyed on the level's number
    // (ore seams, gems, the magma sea) counts from the usual bottom, so it does not move.
    let deepened = ((z_ground - z_min_m) / Z_STEP_M).round() as i32;
    let depth = (((hi - z_min_m) / Z_STEP_M).ceil() as i32 + AIR_ABOVE) as usize;
    let mut map = LocalMap {
        width: n,
        height: n,
        depth,
        z_min_m,
        cells: vec![Cell::AIR; n * n * depth],
        surface_z: vec![0; n * n],
        surface_m: cols.iter().map(|c| c.e).collect(),
        world_tile,
        biome: Biome::classify(cols[n * n / 2 + n / 2].e, cols[n * n / 2 + n / 2].temp, cols[n * n / 2 + n / 2].moist),
        features: vec![wildlife::Feature::None; n * n],
        graves: Vec::new(),
        roofs: vec![0; n * n],
        houses: Vec::new(),
        season_temps: {
            let (tx, ty) = (world_tile.0.min(world.width - 1), world_tile.1.min(world.height - 1));
            let base = *world.temperature.get(tx, ty);
            let north = ty < world.height / 2;
            use crate::seasons::Season;
            let at = |s: Season, k: usize| world.seasonal_climate.as_ref().map(|c| c.get_temperature(tx, ty, s, north)).unwrap_or(base + [0.0, 8.0, 0.0, -8.0][k]);
            [at(Season::Spring, 0), at(Season::Summer, 1), at(Season::Autumn, 2), at(Season::Winter, 3)]
        },
        found: [0; 4],
        furnished: Vec::new(),
        game: Vec::new(),
        places: Vec::new(),
        cavern_z: Vec::new(),
        caverns: Vec::new(),
        aquifer: None,
        magma_top: None,
        magma_pipe: None,
        deepened,
        river_m: cols.iter().filter(|c| c.river_hw > 0.0 && c.river_d < c.river_hw && c.water_level.is_some()).map(|c| c.river_w).fold(0.0, f32::max),
    };

    for j in 0..n {
        for i in 0..n {
            let c = &cols[j * n + i];
            let (mx, my) = (ox_m + (cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m,
                            oy_m + (cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m);
            let (gtx, gty) = ((mx / TILE_M as f64).floor() as i64, (my / TILE_M as f64).floor() as i64);
            let sz = (((c.e - z_min_m) / Z_STEP_M).floor() as i32).clamp(1, depth as i32 - 2);
            map.surface_z[j * n + i] = sz;
            let biome = Biome::classify(c.e, c.temp, c.moist);
            let underwater = c.water_level.map(|w| w > c.e).unwrap_or(false);
            // The bare strip of sand and gravel along a river: wider by a wider river, but a great
            // river's banks are wooded a dozen metres from the water.
            let near_river = c.river_hw > 0.0 && c.river_d < c.river_hw + (c.river_hw * 0.6).clamp(1.5, 12.0);

            // Soil: the world tile's soil depth, thinner on slopes, none on cliffs; deeper by the
            // river where floods lay silt.
            let soil_levels = if c.slope > 0.9 {
                0
            } else {
                let levels = soil_depth_m / Z_STEP_M + if near_river { 1.0 } else { 0.0 };
                (levels.min(8.0) * (1.0 - c.slope * 1.1).clamp(0.15, 1.0)).round().max(1.0) as i32
            };
            use crate::soils::SoilKind as SK;
            let soil = if near_river || c.sea && c.e > -3.0 {
                if hash(gtx, gty, 1) < 0.5 { Material::Sand } else { Material::Gravel }
            } else if matches!(biome, Biome::Desert) || c.moist < 0.08 || matches!(soil_kind, SK::DesertSoil | SK::Podzol) {
                Material::Sand
            } else if matches!(soil_kind, SK::Rocky) {
                Material::Gravel
            } else if matches!(soil_kind, SK::Laterite | SK::TerraRossa) || (c.moist > 0.6 && c.slope < 0.05) {
                Material::Clay
            } else {
                Material::Soil
            };
            // Strata boundaries undulate gently instead of being perfectly flat.
            let wobble = (fbm(&strata_noise, mx / 160.0, my / 160.0, 3) * 4.0).round() as i32;
            let glacier = matches!(biome, Biome::Ice) || c.temp < -18.0;

            for z in 0..sz {
                let below = sz - z; // 1 = directly under the floor
                let material = if glacier && below <= 4 {
                    Material::Ice
                } else if below <= soil_levels {
                    soil
                } else {
                    let mut d = below - soil_levels + wobble;
                    let mut rock = RockType::Granite;
                    for layer in &hs.rock_stack {
                        rock = layer.rock_type;
                        if d <= layer.thickness as i32 { break; }
                        d -= layer.thickness as i32;
                    }
                    Material::Rock(rock)
                };
                // Ore veins in host rock, thicker the closer and richer the nearest deposit.
                let material = match material {
                    Material::Rock(rock) => ore_vein(&veins, rock, mx, my, z - deepened).map(Material::Ore).unwrap_or(material),
                    other => other,
                };
                let k = map.idx(i, j, z as usize);
                map.cells[k] = Cell { shape: Shape::Wall, material, water: 0, plant: Plant::None, boulder: false };
            }

            // The floor: snow in the cold, bare rock where there is no soil. Under water it is
            // the river or lake bed.
            let floor_material = if underwater {
                if c.sea || near_river { soil } else { Material::Clay }
            } else if glacier || c.temp < -2.0 {
                // Glaciers are snow-covered at the surface (ice below); ice floors mean water.
                Material::Snow
            } else if soil_levels == 0 {
                map.cell(i, j, (sz - 1).max(0) as usize).material
            } else {
                soil
            };
            // What grows follows the world's biome for the tile this column lies on (looked up
            // through a jitter so tile borders blend), so a desert embark is a desert and the
            // caused landscapes (dead woods, ashlands, bone fields...) are drawn as what they are;
            // the local climate decides only where the world gives no rule.
            let jx = (hash(i as i64 / 6, j as i64 / 6, 31) - 0.5) as f64 * 24.0;
            let jy = (hash(i as i64 / 6, j as i64 / 6, 37) - 0.5) as f64 * 24.0;
            let rx = region.world_x0 * s + (cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells + jx) as i64;
            let ry = region.world_y0 * s + (cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells + jy) as i64;
            let wb = *world.biomes.get(rx.div_euclid(s).rem_euclid(world.width as i64) as usize, ry.div_euclid(s).clamp(0, world.height as i64 - 1) as usize);
            let (tree_density, shrub_density, species) = world_vegetation(wb).unwrap_or_else(|| vegetation(biome));
            // Forests have clearings and groves rather than uniform density.
            let grove = 0.35 + 0.95 * smoothstep(-0.3, 0.3, fbm(&patch_noise, mx / 150.0, my / 150.0, 3));
            let r = hash(gtx, gty, 7);
            let plant = if underwater || near_river || matches!(floor_material, Material::Ice | Material::Rock(_)) {
                Plant::None
            } else if r < tree_density * grove {
                Plant::Tree(species[(hash(gtx, gty, 9) * species.len() as f32) as usize % species.len()])
            } else if r < tree_density * grove + shrub_density {
                Plant::Shrub
            } else if matches!(floor_material, Material::Soil | Material::Clay) && c.moist > 0.12 && c.temp > -3.0 {
                Plant::Grass
            } else {
                Plant::None
            };
            let boulder = !underwater && plant == Plant::None && hash(gtx, gty, 11) < 0.004 + 0.06 * smoothstep(0.2, 0.8, c.slope);
            let k = map.idx(i, j, sz as usize);
            map.cells[k] = Cell { shape: Shape::Floor, material: floor_material, water: 0, plant, boulder };

            // Water above the floor, filled to the water level; its top level freezes in the cold. Depth in the first level is
            // measured from the actual ground, so water shallower than a z-level still shows.
            if let Some(level) = c.water_level {
                for z in sz + 1..depth as i32 {
                    let bottom = if z == sz + 1 { c.e } else { map.z_elevation(z) };
                    if bottom >= level { break; }
                    let amount = (((level - bottom) / Z_STEP_M).min(1.0) * WATER_FULL as f32).round() as u8;
                    let k = map.idx(i, j, z as usize);
                    map.cells[k].water = amount.max(1);
                }
                // Freeze over unless the water's surface sits within the floor's own level (the
                // ice would replace the ground; a frozen puddle reads better as snow anyway).
                if c.temp < FREEZE_TEMP_C {
                    let top = ((level - z_min_m) / Z_STEP_M).ceil() as i32 - 1;
                    if top > sz && (top as usize) < depth {
                        let k = map.idx(i, j, top as usize);
                        map.cells[k] = Cell { shape: Shape::Floor, material: Material::Ice, water: 0, plant: Plant::None, boulder: false };
                        map.surface_z[j * n + i] = top;
                    } else {
                        // Water shallower than one level freezes solid onto the bed.
                        let k = map.idx(i, j, sz as usize);
                        map.cells[k].material = Material::Ice;
                        map.cells[k].plant = Plant::None;
                        for z in sz + 1..depth as i32 {
                            let k = map.idx(i, j, z as usize);
                            map.cells[k].water = 0;
                        }
                    }
                }
            }
        }
    }

    // The caverns, as planned.
    {
        map.caverns = caverns::carve(&mut map, &runs, &fields, &hs, &abs, seed, world_tile);
        // The magma sea (DF): the bottom three levels of every embark, wherever the caverns
        // leave rock; under ground a volcano stands near (within 8 tiles; `PLANET_FORCE_MAGMA=1`)
        // a pipe of magma rises from it to three levels under the surface.
        if map.depth > 12 {
            // (Under a map deepened for its caverns, the sea fills the levels added too.)
            let top = 3 + map.deepened;
            let molten = |map: &mut LocalMap, x: usize, y: usize, z: i32| {
                let k = map.idx(x, y, z as usize);
                map.cells[k] = Cell { shape: Shape::Empty, material: Material::Magma, water: WATER_FULL, plant: Plant::None, boulder: false };
            };
            for y in 0..map.height { for x in 0..map.width {
                for z in 1..=top {
                    if map.cavern_at(x, y, z).is_some() || z >= map.surface_z[y * map.width + x] - 2 { continue; }
                    molten(&mut map, x, y, z);
                }
            } }
            map.magma_top = Some(top);
            let volcanic = world.volcanoes.iter().any(|v| (v.x as i64 - world_tile.0 as i64).abs().max((v.y as i64 - world_tile.1 as i64).abs()) <= 8)
                || std::env::var("PLANET_FORCE_MAGMA").is_ok();
            if volcanic {
                // The pipe: 40-70 cells from the middle, in a direction hashed by the tile.
                let h = (world_tile.0 as u64).wrapping_mul(0x9E37_79B9) ^ (world_tile.1 as u64).wrapping_mul(0x85EB_CA6B) ^ 0x7A6A;
                let a = (h % 628) as f32 / 100.0;
                let r = 40.0 + (h / 628 % 30) as f32;
                let (px, py) = ((map.width as f32 / 2.0 + a.cos() * r) as i32, (map.height as f32 / 2.0 + a.sin() * r) as i32);
                for dy in -2i32..=2 { for dx in -2i32..=2 {
                    if dx * dx + dy * dy > 5 { continue; }
                    let (x, y) = (px + dx, py + dy);
                    if x < 2 || y < 2 || x as usize + 2 >= map.width || y as usize + 2 >= map.height { continue; }
                    let (x, y) = (x as usize, y as usize);
                    let sz = map.surface_z[y * map.width + x];
                    for z in top + 1..sz - 2 { if map.cavern_at(x, y, z).is_none() { molten(&mut map, x, y, z); } }
                } }
                map.magma_pipe = Some((px.max(0) as u16, py.max(0) as u16));
            }
        }
        // An aquifer (DF): where the land holds water (the tile's water table 0.45+), a band of
        // wet permeable rock five levels thick, four levels under the usual ground.
        if hs.water_table >= 0.45 {
            let mut zs: Vec<i32> = map.surface_z.clone();
            zs.sort_unstable();
            let med = zs[zs.len() / 2];
            if med > 12 { map.aquifer = Some((med - 8, med - 4)); }
        }
    }

    // What history left here: settlements, ruins, fields and roads.
    if let Some(lore) = lore {
        structures::apply(&mut map, region, lore, cx, cy);
        // Animals: trails, burrows, nests, dens (keyed on absolute 2 m tile position).
        let origin = (
            ((ox_m + (cx - n as f64 / 2.0 * tile_cells) * cell_m) / TILE_M as f64).round() as i64,
            ((oy_m + (cy - n as f64 / 2.0 * tile_cells) * cell_m) / TILE_M as f64).round() as i64,
        );
        wildlife::apply(&mut map, world, lore, origin);
    }
    // What a first camp needs within reach: water, wood, stone, berries, something to look at.
    let battles = lore.and_then(|l| l.battles.get(&map.world_tile).cloned()).unwrap_or_default();
    // Rock faces show bare rock (no snow or soil holds on them), and scree gathers at their foot.
    if mountains > 0.0 {
        let rock = (0..map.depth).rev().find_map(|z| match map.cell(n / 2, n / 2, z).material { Material::Rock(r) => Some(r), _ => None }).unwrap_or(RockType::Granite);
        let szs = map.surface_z.clone();
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let sz = szs[j * n + i];
                let nb = [szs[j * n + i - 1], szs[j * n + i + 1], szs[(j - 1) * n + i], szs[(j + 1) * n + i]];
                let drop = nb.iter().map(|&o| sz - o).max().unwrap_or(0);
                let rise = nb.iter().map(|&o| o - sz).max().unwrap_or(0);
                let k = map.idx(i, j, sz.max(0) as usize);
                // Ice over a frozen lake stays ice.
                if map.cells[k].shape != Shape::Floor || map.cells[k].material == Material::Ice { continue; }
                if drop >= 1 {
                    map.cells[k].material = Material::Rock(rock);
                    map.cells[k].plant = Plant::None;
                } else if rise >= 1 && hash(i as i64, j as i64, 0x5C2EE) < 0.3 {
                    map.cells[k].boulder = true;
                    map.cells[k].plant = Plant::None;
                }
            }
        }
    }
    site::furnish(&mut map, &battles);
    // Something down there: lairs, tombs, old mines and caves, each with its cause.
    {
        let (x0, y0) = (region.world_x0, region.world_y0);
        let tile_of = |x: f64, y: f64| (((x0 * s + x as i64).div_euclid(s)).rem_euclid(world.width as i64) as usize, ((y0 * s + y as i64).div_euclid(s)).clamp(0, world.height as i64 - 1) as usize);
        map.places = places::place(&mut map, lore, world, world_tile, tile_of, region.params.seed);
    }

    // Ramps: a floor next to ground exactly one level higher leads up (DF style).
    for j in 0..n {
        for i in 0..n {
            let sz = map.surface_z[j * n + i];
            let up = [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)].iter().any(|&(dx, dy)| {
                let (x, y) = (i as i64 + dx, j as i64 + dy);
                x >= 0 && y >= 0 && x < n as i64 && y < n as i64 && map.surface_z[y as usize * n + x as usize] == sz + 1
            });
            let k = map.idx(i, j, sz as usize);
            let built_over = (sz as usize + 1) < map.depth && map.cells[map.idx(i, j, sz as usize + 1)].shape == Shape::Wall;
            let paved = matches!(map.cells[k].material, Material::Wood | Material::Block(_));
            if up && map.cells[k].plant == Plant::None && map.cells[k].water == 0 && !built_over && !paved {
                map.cells[k].shape = Shape::Ramp;
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A playable area is solid below its surface, open above it, ramps connect single steps,
    /// and water only sits in open cells.
    #[test]
    fn local_map_is_consistent() {
        let world = crate::world::generate_world_with_style(128, 64, 5, crate::plates::WorldStyle::Earthlike);
        let (cx, cy) = crate::region::zoom::pick_interesting_window(&world, 4);
        let params = crate::region::zoom::ZoomParams { center_x: cx, center_y: cy, tiles: 4, cells_per_tile: 32, erosion_iterations: 10, seed: 3 };
        let region = crate::region::zoom::generate_zoom(&world, &params);
        let map = generate_local(&world, &region, None, region.width as f64 / 2.0, region.height as f64 / 2.0);
        assert_eq!(map.cells.len(), LOCAL_SIZE * LOCAL_SIZE * map.depth);
        // Places carved under the ground (caves, lairs, tombs: `places.rs`) are open below it.
        let carved: std::collections::HashSet<(u16, u16)> = map.places.iter().flat_map(|p| p.cells.iter().map(|c| c.0)).collect();
        assert!(!map.caverns.is_empty(), "no cavern under the test embark");
        for y in 0..map.height {
            for x in 0..map.width {
                if carved.contains(&(x as u16, y as u16)) { continue; }
                let sz = map.surface_z[y * map.width + x] as usize;
                for z in 0..map.depth {
                    let c = map.cell(x, y, z);
                    // Below the surface: rock, or under a frozen-over lake its water and its bed (the
                    // ice may sit directly on the bed when the water froze through).
                    let above = if z + 1 < map.depth { Some(map.cell(x, y, z + 1)) } else { None };
                    let bed = c.shape == Shape::Floor && above.map(|a| a.water > 0 || a.material == Material::Ice).unwrap_or(false);
                    if z < sz && map.cavern_at(x, y, z as i32).is_some() { continue; }
                    if z < sz { assert!(c.shape == Shape::Wall || c.water > 0 || bed, "solid (or frozen-over water) below the surface at {},{},{}", x, y, z); }
                    if z == sz { assert!(matches!(c.shape, Shape::Floor | Shape::Ramp)); }
                    if z > sz && c.shape != Shape::Empty {
                        assert!(matches!(c.material, Material::Wood | Material::Block(_) | Material::Clay | Material::Ice), "only buildings or ice above the surface");
                    }
                    if c.water > 0 { assert_eq!(c.shape, Shape::Empty, "water only in open cells"); }
                }
            }
        }
        let trees = map.cells.iter().filter(|c| matches!(c.plant, Plant::Tree(_))).count();
        println!("local map: {} z-levels, {} trees, biome {:?}", map.depth, trees, map.biome);
    }

    /// Two embarks that overlap by half their width agree on the overlap: the ground, the
    /// surface water and the cavern layers with their pools (DF's feature layers line up across
    /// embark tiles), inside one world tile and across world-tile borders where the rock (and so
    /// the caverns' openness) or the water table changes.
    #[test]
    fn neighbouring_embarks_line_up() {
        let world = crate::world::generate_world_with_style(128, 64, 5, crate::plates::WorldStyle::Earthlike);
        let (cx, cy) = crate::region::zoom::pick_interesting_window(&world, 4);
        let params = crate::region::zoom::ZoomParams { center_x: cx, center_y: cy, tiles: 4, cells_per_tile: 32, erosion_iterations: 10, seed: 3 };
        let region = crate::region::zoom::generate_zoom(&world, &params);
        let s = 32usize;
        let tc = TILE_M as f64 / region.cell_m as f64;
        let half = (LOCAL_SIZE / 2) as f64 * tc;
        let n = LOCAL_SIZE;
        let hs = |x: i64, y: i64| world.handshakes.as_ref().unwrap().get((region.world_x0 + x).rem_euclid(world.width as i64) as usize, (region.world_y0 + y) as usize).tile.clone();
        // Embark A's centre (B's = A + half an embark in x): one inside a tile, then up to three
        // on land across a vertical tile border where the rock makes the two tiles' caverns differ.
        let mut cases: Vec<(f64, f64)> = vec![(region.width as f64 / 2.0 - 15.3, region.height as f64 / 2.0 + 9.7)];
        'find: for bx in 1..4i64 {
            for ty in 0..4i64 {
                let (l, r) = (caverns::tile_fields(&hs(bx - 1, ty)), caverns::tile_fields(&hs(bx, ty)));
                if l.0 == r.0 { continue; }
                let (ax, ay) = ((bx as usize * s) as f64 - half / 2.0, (ty as usize * s) as f64 + s as f64 * 0.5 + 0.37);
                if region.elevation_m[ay as usize * region.width + bx as usize * s] <= 0.0 { continue; }
                cases.push((ax, ay));
                if cases.len() == 4 { break 'find; }
            }
        }
        assert!(cases.len() >= 2, "no tile border with different caverns over land");
        let (mut ground, mut water, mut cav, mut pools) = ([0usize; 2], [0usize; 2], [0usize; 2], [0usize; 2]);
        for (c, &(ax, ay)) in cases.iter().enumerate() {
            let a = generate_local(&world, &region, None, ax, ay);
            let b = generate_local(&world, &region, None, ax + half, ay);
            assert!(c == 0 || a.world_tile != b.world_tile, "case {c} should straddle a tile border");
            let (oa, ob) = ((a.z_min_m / Z_STEP_M).round() as i32, (b.z_min_m / Z_STEP_M).round() as i32);
            let before = (ground, water, cav, pools);
            for j in 0..n {
                for i in n / 2..n {
                    let (ka, kb) = (j * n + i, j * n + i - n / 2);
                    let tally = |t: &mut [usize; 2], same: bool| { t[1] += 1; if same { t[0] += 1; } };
                    tally(&mut ground, a.surface_z[ka] + oa == b.surface_z[kb] + ob);
                    let wet = |m: &LocalMap, k: usize| { let z = m.surface_z[k] as usize + 1; z < m.depth && m.cells[m.idx(k % n, k / n, z)].water > 0 };
                    tally(&mut water, wet(&a, ka) == wet(&b, kb));
                    for l in 0..caverns::LAYERS {
                        let (fa, fb) = (a.cavern_z[ka][l], b.cavern_z[kb][l]);
                        if fa.0 < 0 && fb.0 < 0 { continue; }
                        let abs = |c: (i16, i16), o: i32| if c.0 < 0 { (-1, -1) } else { (c.0 as i32 + o, c.1 as i32 + o) };
                        tally(&mut cav, abs(fa, oa) == abs(fb, ob));
                        if fa.0 >= 0 && fb.0 >= 0 {
                            let pool = |m: &LocalMap, k: usize, f: (i16, i16)| (f.0..=f.1).filter(|&z| m.cells[m.idx(k % n, k / n, z as usize)].water > 0).count();
                            tally(&mut pools, pool(&a, ka, fa) == pool(&b, kb, fb));
                        }
                    }
                }
            }
            let pct = |t: [usize; 2], b: [usize; 2]| 100.0 * (t[0] - b[0]) as f32 / (t[1] - b[1]).max(1) as f32;
            println!("tiles {:?} / {:?}: ground {:.1}%, surface water {:.1}%, cavern runs {:.1}% of {}, cavern pools {:.1}%", a.world_tile, b.world_tile,
                pct(ground, before.0), pct(water, before.1), pct(cav, before.2), cav[1] - before.2[1], pct(pools, before.3));
        }
        let pct = |t: [usize; 2]| 100.0 * t[0] as f32 / t[1].max(1) as f32;
        println!("overall: ground {:.1}%, surface water {:.1}%, cavern runs {:.1}%, cavern pools {:.1}%", pct(ground), pct(water), pct(cav), pct(pools));
        // The ground differs only where a mountain tile's terraces meet a neighbour's (each embark
        // takes its terraces' height from its own tile); the caverns are keyed on place alone.
        assert!(pct(cav) > 99.9 && pct(pools) > 99.9, "cavern layers do not line up across embarks");
    }
}

/// A cluster of gems in a rock cell (Dwarf Fortress's small clusters, by host rock): one cell in
/// sixty, by a hash of its place in the world (so the same cell holds the same stone on every
/// visit). Granite holds rock crystal or garnets, basalt obsidian or agate, limestone marble,
/// shale jet, sandstone and loose sediment amber.
pub fn gem_in(map: &LocalMap, x: usize, y: usize, z: usize) -> Option<&'static str> {
    use crate::erosion::materials::RockType as R;
    let Material::Rock(r) = map.cell(x, y, z).material else { return None };
    let (tx, ty) = (map.world_tile.0 as u64, map.world_tile.1 as u64);
    let mut h = (tx << 40) ^ (ty << 28) ^ ((x as u64) << 18) ^ ((y as u64) << 8) ^ (z as i64 - map.deepened as i64) as u64;
    h = h.wrapping_add(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    if h % 60 != 0 { return None; }
    let pick = (h >> 8) % 2 == 0;
    Some(match r {
        R::Granite => if pick { "rock crystal" } else { "garnets" },
        R::Basalt => if pick { "obsidian" } else { "agate" },
        R::Limestone => "marble",
        R::Shale => "jet",
        R::Sandstone | R::Sediment => "amber",
        R::Ice => return None,
    })
}
