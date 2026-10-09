//! The land between the places, walked cell by cell (DF's adventure mode walks the region map
//! the world was made of; here every world tile is a chunk of `CH` x `CH` cells). A chunk is
//! built from the world's own fields keyed on world coordinates (land and sea, elevation, biome,
//! temperature, the history's forest cover, farmland, roads and the Shadow's corruption), so a
//! coast, a ridge, a river, a road or a wood crosses a tile's border as one thing: rivers and
//! roads run between the tiles' anchors along wiggles that both tiles draw the same way.
//!
//! The places of the history stand in the land: a town is laid out in its tile (its walls,
//! gates where its roads leave, streets, square, houses and people); a ruin, a temple, a keep, a
//! tomb, a war camp or the Shadow's fortress stands in the open with its first floor lent to the
//! land, and a cave, a mine, a lair, a labyrinth or a dwarven hall opens as a mouth in the
//! ground (`Feature::Entrance`). Each tile has its perils by land, hour and history (`perils`)
//! and its finds (a camp by the road, a fallen traveller, the old stones, a hermit, a farm, a
//! battlefield's bones).

use super::actor::{Monster, Npc, Role};
use super::data::data;
use super::item::Item;
use super::map::{Feature, Floor, Ground, Tile, Wall, DIRS4, DIRS8};
use super::site::{Builder, Place, SiteKind, SiteSpec};
use super::world::WorldInfo;
use crate::biomes::ExtendedBiome;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

/// Cells a side of a world tile.
pub const CH: i32 = 96;

fn mix64(mut x: u64) -> u64 { x ^= x >> 33; x = x.wrapping_mul(0xff51_afd7_ed55_8ccd); x ^= x >> 33; x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53); x ^ (x >> 33) }
pub fn hash(seed: u64, x: i64, y: i64, salt: u64) -> u64 { mix64(seed ^ mix64((x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9))) }
fn unit(h: u64) -> f32 { (h >> 40) as f32 / (1u64 << 24) as f32 }

/// What a stretch of land is, for its ground, its growth and its perils.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Plains, Forest, Jungle, Taiga, Desert, Snow, Swamp, Mountain, Savanna, Waste, Lake, Sea }

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Plains => "open grassland", Kind::Forest => "woods", Kind::Jungle => "jungle", Kind::Taiga => "pine forest", Kind::Desert => "desert",
            Kind::Snow => "snowfields", Kind::Swamp => "marsh", Kind::Mountain => "mountains", Kind::Savanna => "dry grassland", Kind::Waste => "blasted waste",
            Kind::Lake => "lake country", Kind::Sea => "coast",
        }
    }
    /// The monsters' habitat words for this land.
    pub fn habitat(self) -> &'static str {
        match self {
            Kind::Plains | Kind::Lake | Kind::Sea => "plains", Kind::Forest => "forest", Kind::Jungle => "jungle", Kind::Taiga => "forest", Kind::Desert => "desert",
            Kind::Snow => "snow", Kind::Swamp => "swamp", Kind::Mountain => "mountain", Kind::Savanna => "savanna", Kind::Waste => "waste",
        }
    }
    fn trees(self) -> f32 {
        match self {
            Kind::Plains => 0.02, Kind::Forest => 0.2, Kind::Jungle => 0.28, Kind::Taiga => 0.18, Kind::Desert => 0.004, Kind::Snow => 0.008,
            Kind::Swamp => 0.1, Kind::Mountain => 0.05, Kind::Savanna => 0.03, Kind::Waste => 0.012, Kind::Lake => 0.06, Kind::Sea => 0.02,
        }
    }
}

pub fn kind_of(b: ExtendedBiome, elev: f32) -> Kind {
    use ExtendedBiome::*;
    match b {
        DeepOcean | Ocean | CoastalWater | Lagoon | AbyssalVents | Sargasso | KelpTowers | InkSea | PhosphorShallows | CoralPlateau | SunkenCity => Kind::Sea,
        HighlandLake | CraterLake | FrozenLake | AcidLake | LavaLake | BioluminescentWater | MirrorLake | BrinePools | SinkholeLakes | PrismaticPools | HotSprings => Kind::Lake,
        Ice | Tundra | AlpineTundra | SnowyPeaks | AuroraWastes => Kind::Snow,
        BorealForest | SubalpineForest => Kind::Taiga,
        TropicalForest | TropicalRainforest | CloudForest | MontaneForest => Kind::Jungle,
        TemperateForest | TemperateRainforest | DeadForest | CrystalForest | BioluminescentForest | MushroomForest | PetrifiedForest | AncientGrove | SiliconGrove | OvergrownCitadel | FungalBloom => Kind::Forest,
        Desert | SaltFlats | SingingDunes | GlassDesert | Oasis | PaintedHills => Kind::Desert,
        Swamp | Marsh | Bog | MangroveSaltmarsh | Shadowfen | CarnivorousBog | SpiritMarsh | TarPits => Kind::Swamp,
        VolcanicWasteland | Ashlands | ObsidianFields | BasaltColumns | SulfurVents | VoidScar | CrystalWasteland | Geysers | SporeWastes | BleedingStone | BoneFields | StarfallCrater => Kind::Waste,
        Savanna | Paramo => Kind::Savanna,
        RazorPeaks => Kind::Mountain,
        _ => if elev > 2400.0 { Kind::Mountain } else { Kind::Plains },
    }
}

/// A town's place on its tile: the half extents of its walls by size.
pub fn town_extent(size: u8) -> (i32, i32) { [(19, 16), (24, 19), (30, 25), (40, 35)][size.min(3) as usize] }

/// The side of a town a road toward `d` leaves by (0 north, 1 east, 2 south, 3 west).
pub fn gate_side(d: (i32, i32)) -> usize { if d.1 < 0 { 0 } else if d.1 > 0 { 2 } else if d.0 > 0 { 1 } else { 3 } }

/// Chunk cell just outside a town's gate on `side`.
pub fn gate_cell(size: u8, side: usize) -> (i32, i32) {
    let (hw, hh) = town_extent(size);
    let c = CH / 2;
    match side { 0 => (c, c - hh - 2), 1 => (c + hw + 2, c), 2 => (c, c + hh + 2), _ => (c - hw - 2, c) }
}

/// What the land knows of the places in it (built once from the sites).
#[derive(Clone, Debug, Default)]
pub struct Atlas {
    /// Town tiles: (size, road directions as DIRS8 bits).
    pub towns: HashMap<usize, (u8, u8)>,
    pub seed: u64,
    /// Whether the history gave forest cover (else the biome alone decides).
    pub cover: bool,
}

impl Atlas {
    pub fn new(info: &WorldInfo, sites: &[SiteSpec], seed: u64) -> Atlas {
        let mut towns = HashMap::new();
        for s in sites.iter().filter(|s| s.kind == SiteKind::Town) {
            let (size, roads) = s.town.as_ref().map_or((1, 0), |t| (t.size, t.roads));
            towns.insert(s.tile.1 * info.w + s.tile.0, (size, roads));
        }
        Atlas { towns, seed: seed ^ 0x1A2D_5EED, cover: info.forest.iter().any(|&v| v > 0) }
    }
}

/// The land's fields at any cell.
pub struct Land<'a> { pub info: &'a WorldInfo, pub atlas: &'a Atlas }

impl<'a> Land<'a> {
    fn seed(&self) -> u64 { self.atlas.seed }
    pub fn period(&self) -> i64 { self.info.w as i64 * CH as i64 }
    /// The index of tile (tx, ty), wrapping east-west; None past the poles.
    pub fn idx(&self, tx: i64, ty: i64) -> Option<usize> {
        if ty < 0 || ty >= self.info.h as i64 || self.info.w == 0 { None } else { Some(ty as usize * self.info.w + tx.rem_euclid(self.info.w as i64) as usize) }
    }
    fn rnd(&self, gx: i64, gy: i64, salt: u64) -> f32 { unit(hash(self.seed(), gx.rem_euclid(self.period().max(1)), gy, salt)) }
    /// Smooth noise 0..1 on a lattice of `cell` cells (a divisor of `CH`), wrapping east-west.
    fn noise(&self, gx: f32, gy: f32, cell: i64, salt: u64) -> f32 {
        let p = (self.period() / cell).max(1);
        let (fx, fy) = (gx / cell as f32, gy / cell as f32);
        let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let v = |x: i64, y: i64| unit(hash(self.seed(), x.rem_euclid(p), y, salt));
        let a = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * s(tx);
        let b = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * s(tx);
        a + (b - a) * s(ty)
    }
    fn fbm(&self, gx: f32, gy: f32, cell: i64, oct: u32, salt: u64) -> f32 {
        let (mut sum, mut amp, mut norm, mut c) = (0.0, 1.0, 0.0, cell);
        for o in 0..oct { sum += amp * self.noise(gx, gy, c.max(2), salt + o as u64 * 101); norm += amp; amp *= 0.5; c /= 2; }
        sum / norm
    }
    /// A tile field, bilinear between the tiles' centres.
    fn field(&self, gx: f32, gy: f32, f: impl Fn(usize) -> f32) -> f32 {
        let (u, v) = (gx / CH as f32 - 0.5, gy / CH as f32 - 0.5);
        let (u0, v0) = (u.floor() as i64, v.floor() as i64);
        let (a, b) = (u - u0 as f32, v - v0 as f32);
        let hmax = self.info.h as i64 - 1;
        let g = |tx: i64, ty: i64| self.idx(tx, ty.clamp(0, hmax)).map_or(0.0, &f);
        let top = g(u0, v0) * (1.0 - a) + g(u0 + 1, v0) * a;
        let bot = g(u0, v0 + 1) * (1.0 - a) + g(u0 + 1, v0 + 1) * a;
        top * (1.0 - b) + bot * b
    }
    /// Land (above 0.5) or sea at a cell: the tiles' land, with a wandering shore.
    pub fn landness(&self, gx: i64, gy: i64) -> f32 {
        let (x, y) = (gx as f32 + 0.5, gy as f32 + 0.5);
        self.field(x, y, |k| if self.info.land[k] { 1.0 } else { 0.0 }) + (self.fbm(x, y, 32, 4, 1) - 0.5) * 0.6
    }
    /// The tile whose land a cell is (the nearest tile's middle, the borders warped).
    pub fn tile_of(&self, gx: i64, gy: i64) -> Option<usize> {
        let (x, y) = (gx as f32 + 0.5, gy as f32 + 0.5);
        let wx = (self.fbm(x, y, 32, 3, 11) - 0.5) * CH as f32 * 0.9;
        let wy = (self.fbm(x, y, 32, 3, 12) - 0.5) * CH as f32 * 0.9;
        self.idx(((x + wx) / CH as f32).floor() as i64, (((y + wy) / CH as f32).floor() as i64).clamp(0, self.info.h as i64 - 1))
    }
    pub fn kind(&self, k: usize) -> Kind { kind_of(self.info.biome[k], self.info.elevation[k]) }
    fn town(&self, tx: i64, ty: i64) -> Option<(u8, u8)> { self.idx(tx, ty).and_then(|k| self.atlas.towns.get(&k).copied()) }
    /// Where a tile's roads and rivers meet: a town's square; elsewhere the middle, nudged.
    pub fn anchor(&self, tx: i64, ty: i64) -> (f32, f32) {
        let c = ((tx as f32 + 0.5) * CH as f32, (ty as f32 + 0.5) * CH as f32);
        if self.town(tx, ty).is_some() { return c; }
        let h = hash(self.seed(), tx.rem_euclid(self.info.w as i64), ty, 0xA2C);
        (c.0 + (unit(h) - 0.5) * CH as f32 * 0.4, c.1 + (unit(mix64(h)) - 0.5) * CH as f32 * 0.4)
    }
    /// Where a road leaves tile t toward its neighbour in direction d.
    fn road_end(&self, tx: i64, ty: i64, d: (i32, i32)) -> (f32, f32) {
        match self.town(tx, ty) {
            Some((size, _)) => { let g = gate_cell(size, gate_side(d)); ((tx * CH as i64) as f32 + g.0 as f32 + 0.5, (ty * CH as i64) as f32 + g.1 as f32 + 0.5) }
            None => self.anchor(tx, ty),
        }
    }
    /// A line from a to b that wanders (the same for both tiles that draw it: `key` orders it).
    fn wiggle(&self, a: (f32, f32), b: (f32, f32), key: u64, rough: f32) -> Vec<(f32, f32)> {
        let mut pts = vec![a, b];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let mut amp = rough * (dx * dx + dy * dy).sqrt();
        for depth in 0..5u64 {
            let mut next = Vec::with_capacity(pts.len() * 2);
            for i in 0..pts.len() - 1 {
                let (p, q) = (pts[i], pts[i + 1]);
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let l = (dx * dx + dy * dy).sqrt().max(1e-3);
                let r = unit(hash(key, depth as i64, i as i64, 0x51)) - 0.5;
                next.push(p);
                next.push(((p.0 + q.0) / 2.0 - dy / l * r * amp, (p.1 + q.1) / 2.0 + dx / l * r * amp));
            }
            next.push(*pts.last().unwrap());
            pts = next;
            amp *= 0.5;
        }
        pts
    }
}

/// A chunk as first made.
pub struct Gen {
    pub tiles: Vec<Tile>,
    pub items: Vec<((i32, i32), Item)>,
    /// Creatures of the places stamped here (uids given by the game).
    pub monsters: Vec<Monster>,
    pub npcs: Vec<Npc>,
    /// Places whose first floor stands in this chunk (realized, that floor's dwellers moved out).
    pub places: Vec<Place>,
    /// Cells where nothing wild is set (in towns and their yards).
    pub safe: Vec<bool>,
    pub kind: Kind,
}

fn idx(x: i32, y: i32) -> usize { y as usize * CH as usize + x as usize }
fn inside(x: i32, y: i32) -> bool { x >= 0 && y >= 0 && x < CH && y < CH }

/// Build chunk (tx, ty) (tx may be off the map's width: it wraps), with the places on its tile.
pub fn generate(land: &Land, tx: i64, ty: i64, sites: &[&SiteSpec]) -> Gen {
    let n = (CH * CH) as usize;
    let tk = land.idx(tx, ty);
    let kind = tk.map_or(Kind::Snow, |k| land.kind(k));
    let mut g = Gen { tiles: vec![Tile::floor(Ground::Grass); n], items: Vec::new(), monsters: Vec::new(), npcs: Vec::new(), places: Vec::new(), safe: vec![false; n], kind };
    // Past the poles: the ice wall at the end of the world.
    let Some(tk) = tk else { g.tiles = vec![Tile::wall(Wall::Rock, Ground::Ice); n]; return g; };
    let (ox, oy) = (tx * CH as i64, ty * CH as i64);
    let info = land.info;
    let has_hist = land.atlas.cover;
    // 1. The ground cell by cell.
    for ly in 0..CH { for lx in 0..CH {
        let (gx, gy) = (ox + lx as i64, oy + ly as i64);
        let (x, y) = (gx as f32 + 0.5, gy as f32 + 0.5);
        let lnd = land.landness(gx, gy);
        let k = land.tile_of(gx, gy).unwrap_or(tk);
        let kd = land.kind(k);
        let temp = land.field(x, y, |q| info.temperature[q]);
        let cold = temp < -3.0;
        let t = &mut g.tiles[idx(lx, ly)];
        if lnd < 0.5 {
            *t = if temp < -8.0 { Tile::floor(Ground::Ice) } else if lnd < 0.42 { Tile::floor(Ground::Water) } else { Tile::floor(Ground::Shallows) };
            continue;
        }
        let elev = land.field(x, y, |q| info.elevation[q].max(0.0)) + (land.fbm(x, y, 16, 3, 5) - 0.5) * 300.0;
        let rough = (((elev - 700.0) / 1800.0).clamp(0.0, 1.0) + if kd == Kind::Mountain { 0.35 } else { 0.0 }).min(1.0);
        let n1 = land.fbm(x, y, 16, 4, 7);
        let mut ground = match kd {
            Kind::Plains | Kind::Lake | Kind::Sea => if n1 > 0.7 { Ground::Earth } else { Ground::Grass },
            Kind::Forest => if n1 > 0.66 { Ground::Moss } else { Ground::Grass },
            Kind::Jungle => if n1 > 0.55 { Ground::Moss } else { Ground::Grass },
            Kind::Taiga => if cold { Ground::Snow } else if n1 > 0.7 { Ground::Moss } else { Ground::Grass },
            Kind::Desert => if n1 > 0.74 { Ground::Earth } else { Ground::Sand },
            Kind::Snow => if n1 > 0.78 { Ground::Ice } else { Ground::Snow },
            Kind::Swamp => if n1 > 0.72 { Ground::Water } else if n1 > 0.62 { Ground::Shallows } else if n1 > 0.4 { Ground::Mud } else { Ground::Grass },
            Kind::Mountain => if n1 > 0.6 { Ground::Rock } else if cold { Ground::Snow } else { Ground::Earth },
            Kind::Savanna => if n1 > 0.6 { Ground::Earth } else { Ground::Grass },
            Kind::Waste => if n1 > 0.7 { Ground::Rock } else { Ground::Ash },
        };
        if cold && matches!(ground, Ground::Grass | Ground::Earth | Ground::Moss) && kd != Kind::Swamp { ground = Ground::Snow; }
        // Shores: a beach where the sea meets the land.
        if lnd < 0.56 && !cold && !matches!(kd, Kind::Swamp) { ground = Ground::Sand; }
        // The Shadow's corruption: ash where it lies thick.
        let sh = land.field(x, y, |q| info.shadow[q]);
        if sh > 0.12 && land.fbm(x, y, 32, 3, 9) < sh * 1.2 - 0.05 && !matches!(ground, Ground::Water | Ground::Shallows) { ground = Ground::Ash; }
        *t = Tile::floor(ground);
        // Ridges and outcrops in high country; boulders anywhere.
        let ridge = 1.0 - (2.0 * land.fbm(x, y, 32, 3, 13) - 1.0).abs();
        // (Ridges with valleys between: the mountains are crossed by their passes.)
        if rough > 0.05 && ridge > 0.95 - rough * 0.17 { *t = Tile::wall(Wall::Rock, Ground::Rock); continue; }
        if rough > 0.3 && ridge > 0.9 - rough * 0.17 { t.ground = Ground::Rock; }
        let r = land.rnd(gx, gy, 3);
        if r < 0.0025 + rough * 0.01 && !matches!(ground, Ground::Water | Ground::Shallows) { t.wall = Wall::Rock; continue; }
        if sh > 0.55 && r > 0.996 { t.wall = Wall::Shadow; continue; }
        // Woods: clustered, thinned where the history cleared them.
        let cover = if has_hist { land.field(x, y, |q| info.forest[q] as f32 / 255.0) } else { 0.5 };
        let base = kd.trees() * if has_hist { 0.35 + 1.3 * cover } else { 1.0 };
        let cluster = land.fbm(x, y, 16, 3, 17);
        // (Thickets, never a wall of trees: woods are walked through.)
        let dens = (base * (0.25 + 1.8 * (cluster - 0.3).max(0.0))).min(0.38);
        if !matches!(ground, Ground::Water | Ground::Shallows | Ground::Ice) && land.rnd(gx, gy, 19) < dens { t.wall = Wall::Tree; }
    } }
    // 2. Lakes in lake country.
    if kind == Kind::Lake {
        let a = land.anchor(tx, ty);
        for ly in 0..CH { for lx in 0..CH {
            let (x, y) = ((ox + lx as i64) as f32 + 0.5, (oy + ly as i64) as f32 + 0.5);
            let d = ((x - a.0).powi(2) + (y - a.1).powi(2)).sqrt() / (CH as f32 * 0.3);
            let edge = 0.7 + 0.6 * land.fbm(x, y, 16, 3, 23);
            let t = &mut g.tiles[idx(lx, ly)];
            if d < edge * 0.85 { *t = Tile::floor(Ground::Water); } else if d < edge { *t = Tile::floor(Ground::Shallows); }
        } }
    }
    // 3. Farmland: fields in a grid about the towns, hedged and pathed.
    for ly in 0..CH { for lx in 0..CH {
        let (gx, gy) = (ox + lx as i64, oy + ly as i64);
        let (x, y) = (gx as f32 + 0.5, gy as f32 + 0.5);
        let farm = land.field(x, y, |q| info.farmland[q] as f32 / 255.0);
        if farm < 0.08 { continue; }
        let (fx, fy) = (gx.div_euclid(11), gy.div_euclid(9));
        if unit(hash(land.seed(), fx.rem_euclid(land.period() / 11 + 1), fy, 0xFA)) > farm * 1.6 { continue; }
        let t = &mut g.tiles[idx(lx, ly)];
        if matches!(t.ground, Ground::Water | Ground::Shallows | Ground::Ice | Ground::Sand) || t.wall == Wall::Rock || t.wall == Wall::Shadow { continue; }
        let edge = gx.rem_euclid(11) == 0 || gy.rem_euclid(9) == 0;
        // Hedged on some sides (one field's west and north edges are its own), a path on others.
        let hedge_side = if gx.rem_euclid(11) == 0 { unit(hash(land.seed(), fx, fy, 0xFB)) < 0.3 } else { unit(hash(land.seed(), fx, fy, 0xFC)) < 0.3 };
        let gap = gx.rem_euclid(11) == 5 || gy.rem_euclid(9) == 4 || (gx.rem_euclid(11) == 0 && gy.rem_euclid(9) == 0);
        *t = if edge { if hedge_side && !gap { Tile::wall(Wall::Hedge, Ground::Grass) } else { Tile::floor(Ground::Earth) } } else { Tile::floor(Ground::Field) };
    } }
    // 4. Rivers between the tiles' anchors (the full line between two tiles, clipped here).
    let span = 2i64;
    let clip = |gx: f32, gy: f32| -> Option<(i32, i32)> { let (lx, ly) = ((gx.floor() as i64 - ox) as i32, (gy.floor() as i64 - oy) as i32); if inside(lx, ly) { Some((lx, ly)) } else { None } };
    for dy in -span..=span { for dx in -span..=span {
        let (sx, sy) = (tx + dx, ty + dy);
        let Some(k) = land.idx(sx, sy) else { continue };
        if !info.river[k] || info.downhill[k] == 255 { continue; }
        let (ddx, ddy) = DIRS8[info.downhill[k] as usize];
        let (ex, ey) = (sx + ddx as i64, sy + ddy as i64);
        let Some(e) = land.idx(ex, ey) else { continue };
        // Ordered so both tiles wiggle it alike.
        let (a, b, key) = if k < e { (land.anchor(sx, sy), land.anchor(ex, ey), (k as u64) << 32 | e as u64) } else { (land.anchor(ex, ey), land.anchor(sx, sy), (e as u64) << 32 | k as u64) };
        let pts = land.wiggle(a, b, key ^ 0x7121, 0.22);
        let mut s = 0.0f32;
        for w in pts.windows(2) {
            let (p, q) = (w[0], w[1]);
            let l = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
            let steps = (l * 2.0).ceil().max(1.0) as i32;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let (cx, cy) = (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
                let arc = s + l * t;
                // A ford every so often (and the banks shallow).
                let ford = (arc / 24.0).fract() < 0.1;
                for oy2 in -2..=2 { for ox2 in -2..=2 {
                    let (px, py) = (cx + ox2 as f32, cy + oy2 as f32);
                    let Some((lx, ly)) = clip(px, py) else { continue };
                    let d = ((px.floor() + 0.5 - cx).powi(2) + (py.floor() + 0.5 - cy).powi(2)).sqrt();
                    let t = &mut g.tiles[idx(lx, ly)];
                    if t.ground == Ground::Ice { continue; }
                    if d < 0.9 && !ford { *t = Tile::floor(Ground::Water); }
                    else if d < 1.9 && t.ground != Ground::Water { *t = Tile::floor(Ground::Shallows); }
                    else if d < 2.6 && t.wall == Wall::None && matches!(t.ground, Ground::Grass | Ground::Earth) && land.rnd((lx as i64) + ox, (ly as i64) + oy, 29) < 0.3 { t.ground = Ground::Mud; }
                } }
            }
            s += l;
        }
    } }
    // 5. A town in the middle of its tile (its roads come to its gates after).
    let mut taken: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut sites: Vec<&SiteSpec> = sites.to_vec();
    sites.sort_by_key(|s| (s.kind != SiteKind::Town, s.id));
    for s in sites.iter().filter(|s| s.kind == SiteKind::Town || (s.kind == SiteKind::Ruin && s.town.is_some())) { stamp_town(land, &mut g, s, tx, ty, &mut taken); }
    // 6. Roads between road tiles (bridges over water; trees and rocks cleared).
    for dy in -span..=span { for dx in -span..=span {
        let (sx, sy) = (tx + dx, ty + dy);
        let Some(k) = land.idx(sx, sy) else { continue };
        if !info.road[k] { continue; }
        for (d, &(ddx, ddy)) in DIRS8.iter().enumerate() {
            let (ex, ey) = (sx + ddx as i64, sy + ddy as i64);
            let Some(e) = land.idx(ex, ey) else { continue };
            if !info.road[e] || e < k && d < 8 && false { continue; }
            // Each pair once (from the lower index); diagonals only where no corner joins them.
            if e < k { continue; }
            if ddx != 0 && ddy != 0 {
                let c1 = land.idx(sx + ddx as i64, sy).map_or(false, |q| info.road[q]);
                let c2 = land.idx(sx, sy + ddy as i64).map_or(false, |q| info.road[q]);
                if c1 || c2 { continue; }
            }
            let a = land.road_end(sx, sy, (ddx, ddy));
            let b = land.road_end(ex, ey, (-ddx, -ddy));
            let pts = land.wiggle(a, b, ((k as u64) << 32 | e as u64) ^ 0x20AD, 0.12);
            let near_town = land.town(sx, sy).is_some() || land.town(ex, ey).is_some();
            for w in pts.windows(2) {
                let (p, q) = (w[0], w[1]);
                let l = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
                let steps = (l * 2.0).ceil().max(1.0) as i32;
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    let (cx, cy) = (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
                    for oy2 in -1..=1 { for ox2 in -1..=1 {
                        let (px, py) = (cx + ox2 as f32, cy + oy2 as f32);
                        let Some((lx, ly)) = clip(px, py) else { continue };
                        let d = ((px.floor() + 0.5 - cx).powi(2) + (py.floor() + 0.5 - cy).powi(2)).sqrt();
                        if d > 1.05 { continue; }
                        if g.safe[idx(lx, ly)] && !matches!(g.tiles[idx(lx, ly)].ground, Ground::Water | Ground::Shallows) && g.tiles[idx(lx, ly)].wall != Wall::None { continue; }
                        if g.safe[idx(lx, ly)] && g.tiles[idx(lx, ly)].feature != Feature::None { continue; }
                        let t = &mut g.tiles[idx(lx, ly)];
                        let wet = matches!(t.ground, Ground::Water | Ground::Shallows);
                        *t = Tile::floor(if wet { Ground::Wood } else if near_town { Ground::Cobbles } else if t.ground == Ground::Snow { Ground::Snow } else { Ground::Earth });
                    } }
                }
            }
        }
    } }
    // 7. The other places on this tile, where they fit.
    for s in sites {
        match s.kind {
            SiteKind::Town => {}
            SiteKind::Ruin if s.town.is_some() => {}
            SiteKind::Ruin | SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine | SiteKind::Castle | SiteKind::Camp | SiteKind::DarkFortress => {
                if !stamp_site(land, &mut g, s, tx, ty, &mut taken) { mouth(land, &mut g, s, tx, ty, &mut taken); }
            }
            SiteKind::Wilds => {}
            SiteKind::Cellar => cellar_door(&mut g, s),
            _ => mouth(land, &mut g, s, tx, ty, &mut taken),
        }
    }
    // 8. What lies about: finds by the land, the road and the history.
    finds(land, &mut g, tx, ty, &taken);
    g
}

fn overlaps(taken: &[(i32, i32, i32, i32)], r: (i32, i32, i32, i32)) -> bool {
    taken.iter().any(|o| r.0 < o.0 + o.2 && o.0 < r.0 + r.2 && r.1 < o.1 + o.3 && o.1 < r.1 + r.3)
}

/// The share of land (not water) in a chunk rectangle.
fn land_share(g: &Gen, r: (i32, i32, i32, i32)) -> f32 {
    let mut n = 0; let mut l = 0;
    for y in r.1..r.1 + r.3 { for x in r.0..r.0 + r.2 { if inside(x, y) { n += 1; if !matches!(g.tiles[idx(x, y)].ground, Ground::Water | Ground::Shallows) { l += 1; } } } }
    if n == 0 { 0.0 } else { l as f32 / n as f32 }
}

/// Lay the town's first floor (its streets) over the chunk; its people come with it.
fn stamp_town(_land: &Land, g: &mut Gen, s: &SiteSpec, tx: i64, ty: i64, taken: &mut Vec<(i32, i32, i32, i32)>) {
    let mut p = super::town::realize(s);
    let mask = super::town::footprint(s);
    let f = &p.floors[0];
    for y in 0..CH { for x in 0..CH {
        if !mask[idx(x, y)] { continue; }
        let mut t = f.at(x, y).clone();
        // Piers stand on the water that is there.
        if t.ground == Ground::Wood && t.wall == Wall::None && t.feature == Feature::None && super::town::is_pier(s, x, y) && !matches!(g.tiles[idx(x, y)].ground, Ground::Water | Ground::Shallows) { t.ground = g.tiles[idx(x, y)].ground; }
        if t.feature == Feature::Grate { t.feature = Feature::Entrance { site: s.id, z: 1 }; }
        if t.feature == Feature::Exit { t.feature = Feature::None; }
        g.tiles[idx(x, y)] = t;
        g.safe[idx(x, y)] = true;
    } }
    for (pos, items) in f.items.iter() { for it in items { g.items.push((*pos, it.clone())); } }
    let (hw, hh) = town_extent(s.town.as_ref().map_or(1, |t| t.size));
    taken.push((CH / 2 - hw - 3, CH / 2 - hh - 3, 2 * hw + 7, 2 * hh + 7));
    let razed = s.kind != SiteKind::Town;
    if razed {
        // Razed: walls broken, roofs burned, its people gone; looters and the dead in the streets.
        let mut b = Builder { rng: ChaCha8Rng::seed_from_u64(s.seed ^ 0xA5E5) };
        for y in 0..CH { for x in 0..CH {
            if !mask[idx(x, y)] { continue; }
            let t = &mut g.tiles[idx(x, y)];
            g.safe[idx(x, y)] = false;
            if matches!(t.wall, Wall::Brick | Wall::Timber | Wall::Palisade | Wall::Hedge | Wall::Rock) && b.chance(0.35) { *t = Tile::floor(Ground::Rubble); continue; }
            if t.wall == Wall::None {
                if matches!(t.ground, Ground::Wood | Ground::Carpet | Ground::Field | Ground::Grass | Ground::Moss) && b.chance(0.6) { t.ground = Ground::Ash; }
                match t.feature { Feature::Door { .. } | Feature::Counter | Feature::Table | Feature::Bed | Feature::Bookshelf | Feature::Throne | Feature::Barrel | Feature::Crate | Feature::Sign { .. } => { t.feature = if b.chance(0.2) { Feature::Bones } else { Feature::None }; } _ => {} }
                if t.feature == Feature::None && b.chance(0.012) { t.feature = Feature::Bones; }
            }
        } }
        let cells: Vec<(i32, i32)> = (0..CH * CH).map(|k| (k % CH, k / CH)).filter(|&(x, y)| mask[idx(x, y)] && g.tiles[idx(x, y)].walkable() && g.tiles[idx(x, y)].feature == Feature::None).collect();
        let table = ["bandit", "bandit", "ghoul", "zombie", "skeleton", "rat", "orc"];
        for _ in 0..(6 + s.town.as_ref().map_or(1, |t| t.size as i32) * 3) {
            let Some(&(x, y)) = b.pick(&cells).as_ref() else { break };
            g.monsters.push(Monster::new(0, table[b.range(0, table.len() as i32 - 1) as usize], x, y, 0));
        }
        if let Some(boss) = &s.boss { if let Some(&(x, y)) = b.pick(&cells).as_ref() { g.monsters.push(Monster::boss(0, &boss.def, &boss.name, boss.scale, x, y, 0)); } }
        p.npcs.clear();
    }
    let mut npcs: Vec<Npc> = p.npcs.drain(..).collect();
    for n in npcs.iter_mut() { n.home = s.id; }
    g.npcs.extend(npcs);
    let floor0: Vec<Monster> = p.monsters.iter().filter(|m| m.z == 0).cloned().collect();
    g.monsters.extend(floor0);
    p.monsters.retain(|m| m.z != 0);
    p.top = 1;
    p.origin = Some(((tx * CH as i64) as i32, (ty * CH as i64) as i32));
    p.floors[0].items.clear();
    g.places.push(p);
}

/// Stand a surface place (its first floor's walls and fittings) in the land; false if it does
/// not fit.
fn stamp_site(_land: &Land, g: &mut Gen, s: &SiteSpec, tx: i64, ty: i64, taken: &mut Vec<(i32, i32, i32, i32)>) -> bool {
    let mut p = super::site::realize(s);
    let f = &p.floors[0];
    // What stands: walls other than trees, made ground, fittings.
    let built = |t: &Tile| (t.wall != Wall::None && t.wall != Wall::Tree && !t.boulder()) || t.ground != s.surface || (t.feature != Feature::None && t.feature != Feature::Exit);
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in 0..f.h as i32 { for x in 0..f.w as i32 { if built(f.at(x, y)) { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y); } } }
    if x0 > x1 { return false; }
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    if bw + 8 > CH || bh + 8 > CH { return false; }
    // The middle first, then outward on a coarse grid.
    let mut spots: Vec<(i32, i32)> = Vec::new();
    for gy in (4..CH - bh - 3).step_by(4) { for gx in (4..CH - bw - 3).step_by(4) { spots.push((gx, gy)); } }
    let mid = ((CH - bw) / 2, (CH - bh) / 2);
    spots.sort_by_key(|&(x, y)| ((x - mid.0).abs() + (y - mid.1).abs(), x, y));
    let Some(&(sx, sy)) = spots.iter().find(|&&(x, y)| { let r = (x - 3, y - 3, bw + 6, bh + 6); !overlaps(taken, r) && land_share(g, (x, y, bw, bh)) > 0.92 }) else { return false };
    let (offx, offy) = (sx - x0, sy - y0);
    for y in y0 - 2..=y1 + 2 { for x in x0 - 2..=x1 + 2 {
        let (cx, cy) = (x + offx, y + offy);
        if !inside(cx, cy) { continue; }
        let t = f.at(x, y);
        let cell = &mut g.tiles[idx(cx, cy)];
        if f.inside(x, y) && built(t) {
            let mut t = t.clone();
            if matches!(t.feature, Feature::StairsDown | Feature::LadderDown | Feature::Hole | Feature::Grate) { t.feature = Feature::Entrance { site: s.id, z: 1 }; }
            if t.feature == Feature::Exit { t.feature = Feature::None; }
            *cell = t;
        } else if matches!(cell.wall, Wall::Tree | Wall::Rock) {
            // The yard is clear.
            cell.wall = Wall::None;
        }
    } }
    for (pos, items) in f.items.iter() { for it in items { let (cx, cy) = (pos.0 + offx, pos.1 + offy); if inside(cx, cy) { g.items.push(((cx, cy), it.clone())); } } }
    // Its first floor's dwellers stand in the land now (near where they were).
    // (Those within its walls: a tomb's dead do not wander the country about it.)
    for m in p.monsters.iter().filter(|m| m.z == 0 && m.x >= x0 && m.y >= y0 && m.x <= x1 && m.y <= y1) {
        let mut m = m.clone();
        let (cx, cy) = ((m.x + offx).clamp(1, CH - 2), (m.y + offy).clamp(1, CH - 2));
        if !g.tiles[idx(cx, cy)].walkable() { continue; }
        m.x = cx; m.y = cy; m.home = (cx, cy);
        g.monsters.push(m);
    }
    p.monsters.retain(|m| m.z != 0);
    p.floors[0].items.clear();
    p.top = 1;
    p.origin = Some(((tx * CH as i64) as i32 + offx, (ty * CH as i64) as i32 + offy));
    taken.push((sx - 3, sy - 3, bw + 6, bh + 6));
    g.places.push(p);
    true
}

/// A way into a place under the ground: a cave's mouth in a mound of rock, a mine's timbered
/// adit, a hall's carved door, a labyrinth's ring.
fn mouth(land: &Land, g: &mut Gen, s: &SiteSpec, tx: i64, ty: i64, taken: &mut Vec<(i32, i32, i32, i32)>) {
    let h = hash(land.seed(), s.id as i64, 0, 0x30);
    // Spots on a ring about the middle, the hashed one first; land, clear of other places.
    let mut spots: Vec<(i32, i32)> = (0..48).map(|k| {
        let a = (unit(h) + k as f32 / 48.0) * std::f32::consts::TAU;
        let r = CH as f32 * if k % 2 == 0 { 0.32 } else { 0.18 };
        ((CH as f32 / 2.0 + a.cos() * r) as i32, (CH as f32 / 2.0 + a.sin() * r) as i32)
    }).collect();
    spots.push((CH / 2, CH / 2));
    // Then anywhere on the tile (a town may fill the middle).
    for y in (4..CH - 4).step_by(3) { for x in (4..CH - 4).step_by(3) { spots.push((x, y)); } }
    let ok = |g: &Gen, x: i32, y: i32, r: i32| x > r && y > r && x < CH - r - 1 && y < CH - r - 1 && !overlaps(taken, (x - r, y - r, 2 * r + 1, 2 * r + 1)) && !g.safe[idx(x, y)] && land_share(g, (x - r + 1, y - r + 1, 2 * r - 1, 2 * r - 1)) > 0.9;
    let (mx, my) = spots.iter().copied().find(|&(x, y)| ok(g, x, y, 4)).or_else(|| spots.iter().copied().find(|&(x, y)| ok(g, x, y, 2))).unwrap_or((3, 3));
    let ground = g.tiles[idx(mx, my)].ground;
    let ground = if matches!(ground, Ground::Water | Ground::Shallows | Ground::Wood | Ground::Cobbles) { Ground::Earth } else { ground };
    let set = |g: &mut Gen, x: i32, y: i32, t: Tile| { if inside(x, y) { g.tiles[idx(x, y)] = t; } };
    // Clear the ground about it.
    for y in my - 3..=my + 3 { for x in mx - 3..=mx + 3 { set(g, x, y, Tile::floor(ground)); } }
    match s.kind {
        SiteKind::Mine => {
            for y in my - 2..=my { for x in mx - 2..=mx + 2 { if (x - mx).abs() == 2 || y == my - 2 { set(g, x, y, Tile::wall(Wall::Timber, Ground::Earth)); } } }
            for y in my - 1..=my + 3 { set(g, mx, y, Tile { ground: Ground::Earth, wall: Wall::None, feature: Feature::Rail }); }
            set(g, mx + 2, my + 2, Tile { ground: Ground::Earth, wall: Wall::None, feature: Feature::Crate });
        }
        SiteKind::Halls => {
            for y in my - 2..=my { for x in mx - 3..=mx + 3 { if (x - mx).abs() >= 1 || y == my - 2 { set(g, x, y, Tile::wall(Wall::Rock, Ground::Flags)); } } }
            set(g, mx - 2, my + 1, Tile { ground: Ground::Flags, wall: Wall::None, feature: Feature::Statue });
            set(g, mx + 2, my + 1, Tile { ground: Ground::Flags, wall: Wall::None, feature: Feature::Statue });
            set(g, mx, my + 1, Tile::floor(Ground::Flags));
        }
        SiteKind::Labyrinth => {
            for y in my - 3..=my + 3 { for x in mx - 3..=mx + 3 { if (x - mx).abs() == 3 || (y - my).abs() == 3 { set(g, x, y, Tile::wall(Wall::Brick, Ground::Flags)); } else { set(g, x, y, Tile::floor(Ground::Flags)); } } }
            set(g, mx, my + 3, Tile::floor(Ground::Flags));
        }
        SiteKind::Ruin | SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine | SiteKind::Castle | SiteKind::Camp | SiteKind::DarkFortress => {
            // A place too big for what room there is: its gatehouse stands here, the rest beyond.
            let (wall, floor) = match s.kind { SiteKind::DarkFortress => (Wall::Shadow, Ground::Ash), SiteKind::Camp => (Wall::Palisade, Ground::Earth), SiteKind::Temple | SiteKind::Shrine => (Wall::Brick, Ground::Marble), _ => (Wall::Brick, Ground::Flags) };
            for y in my - 2..=my + 1 { for x in mx - 2..=mx + 2 { let edge = (x - mx).abs() == 2 || y == my - 2 || y == my + 1; set(g, x, y, if edge { Tile::wall(wall, floor) } else { Tile::floor(floor) }); } }
            set(g, mx, my + 1, Tile { ground: floor, wall: Wall::None, feature: Feature::Door { open: true, lock: 0 } });
            if s.kind == SiteKind::Ruin { set(g, mx + 2, my - 2, Tile::floor(Ground::Rubble)); }
        }
        _ => {
            // A mound of rock with the mouth in its south face.
            for y in my - 3..=my { for x in mx - 3..=mx + 3 {
                let d = ((x - mx) as f32 / 3.5).powi(2) + ((y - my) as f32 / 3.2).powi(2);
                if d < 1.0 { set(g, x, y, Tile::wall(Wall::Rock, Ground::Rock)); }
            } }
            if s.kind == SiteKind::Lair { for (dx, dy) in [(-2, 2), (2, 1), (1, 3)] { set(g, mx + dx, my + dy, Tile { ground, wall: Wall::None, feature: Feature::Bones }); } }
        }
    }
    set(g, mx, my, Tile { ground: if s.kind == SiteKind::Mine { Ground::Earth } else { ground }, wall: Wall::None, feature: Feature::Entrance { site: s.id, z: 0 } });
    taken.push((mx - 4, my - 4, 9, 9));
    let _ = (tx, ty);
}

/// A cellar under a town: its door is in a house's floor (beside a bed), else in a street.
fn cellar_door(g: &mut Gen, s: &SiteSpec) {
    let beds: Vec<(i32, i32)> = (0..CH * CH).map(|k| (k % CH, k / CH)).filter(|&(x, y)| g.tiles[idx(x, y)].feature == Feature::Bed).collect();
    let h = s.seed as usize;
    let near = |(x, y): (i32, i32)| DIRS4.iter().map(|(dx, dy)| (x + dx, y + dy)).find(|&(a, b)| inside(a, b) && g.tiles[idx(a, b)].walkable() && g.tiles[idx(a, b)].feature == Feature::None);
    let spot = if beds.is_empty() { None } else { (0..beds.len()).map(|k| beds[(h + k) % beds.len()]).find_map(near) };
    let spot = spot.or_else(|| (0..CH * CH).map(|k| (k % CH, k / CH)).find(|&(x, y)| g.safe[idx(x, y)] && g.tiles[idx(x, y)].walkable() && g.tiles[idx(x, y)].feature == Feature::None && g.tiles[idx(x, y)].ground == Ground::Cobbles));
    if let Some((x, y)) = spot { g.tiles[idx(x, y)].feature = Feature::Entrance { site: s.id, z: 0 }; }
}

/// Finds by the land and the history: a camp by the road, a fallen traveller, the old stones,
/// a hermit far from towns, a farmstead, a battlefield's bones, herbs in the woods.
fn finds(land: &Land, g: &mut Gen, tx: i64, ty: i64, taken: &[(i32, i32, i32, i32)]) {
    let Some(k) = land.idx(tx, ty) else { return };
    let info = land.info;
    if !info.land[k] { return; }
    let mut b = Builder { rng: ChaCha8Rng::seed_from_u64(hash(land.seed(), tx.rem_euclid(info.w as i64), ty, 0xF1D)) };
    let danger = info.danger[k] as u32;
    let tier = (1 + danger * 3 / 255).clamp(1, 4);
    let town_here = land.atlas.towns.contains_key(&k);
    // A free spot (open land, clear of places) near a wanted one.
    let free_near = |g: &Gen, b: &mut Builder, want: Option<(i32, i32)>, r: i32| -> Option<(i32, i32)> {
        for _ in 0..60 {
            let (x, y) = match want { Some((wx, wy)) => (wx + b.range(-r, r), wy + b.range(-r, r)), None => (b.range(6, CH - 7), b.range(6, CH - 7)) };
            if x < 4 || y < 4 || x >= CH - 4 || y >= CH - 4 || overlaps(taken, (x - 3, y - 3, 7, 7)) { continue; }
            if (y - 2..=y + 2).all(|yy| (x - 2..=x + 2).all(|xx| { let t = &g.tiles[idx(xx, yy)]; t.wall == Wall::None && t.feature == Feature::None && !matches!(t.ground, Ground::Water | Ground::Shallows | Ground::Wood) && !g.safe[idx(xx, yy)] })) { return Some((x, y)); }
        }
        None
    };
    let road_cell = (0..200).find_map(|_| { let (x, y) = (b.range(8, CH - 9), b.range(8, CH - 9)); let t = &g.tiles[idx(x, y)]; if matches!(t.ground, Ground::Earth | Ground::Cobbles) && t.wall == Wall::None && info.road[k] && !g.safe[idx(x, y)] { Some((x, y)) } else { None } });
    let put = |g: &mut Gen, x: i32, y: i32, f: Feature| { if inside(x, y) { g.tiles[idx(x, y)].wall = Wall::None; g.tiles[idx(x, y)].feature = f; } };
    // A camp by the road: abandoned, or the bandits' own.
    if let Some(rc) = road_cell { if b.chance(0.45) { if let Some((x, y)) = free_near(g, &mut b, Some(rc), 7) {
        put(g, x, y, Feature::Campfire);
        put(g, x - 2, y - 1, Feature::Tent);
        if b.chance(0.5) { put(g, x + 2, y - 1, Feature::Tent); }
        put(g, x + 1, y + 2, Feature::Chest { items: super::site::treasure(&mut b, tier), opened: false, lock: 0, quest: 0 });
        let by_town = (-1..=1i64).any(|dy| (-1..=1i64).any(|dx| land.idx(tx + dx, ty + dy).map_or(false, |q| land.atlas.towns.contains_key(&q))));
        if danger > 40 && !by_town && b.chance(0.7) {
            for _ in 0..b.range(2, 3 + tier as i32) { let (mx, my) = (x + b.range(-3, 3), y + b.range(-3, 3)); if inside(mx, my) && g.tiles[idx(mx, my)].walkable() { g.monsters.push(Monster::new(0, if b.chance(0.3) && data().monster("highwayman").is_some() { "highwayman" } else { "bandit" }, mx, my, 0)); } }
        }
    } } }
    // A traveller who did not make it.
    if b.chance(0.3) { if let Some((x, y)) = free_near(g, &mut b, None, 0) {
        put(g, x, y, Feature::Bones);
        let mut loot = vec![super::site::gear(&mut b, tier)];
        if b.chance(0.5) { loot.push(Item::new("gold", b.range(3, 12) as u32 * tier)); }
        if b.chance(0.4) { loot.push(Item::new(if b.chance(0.5) { "health_potion" } else { "torch" }, 1)); }
        if b.chance(0.15) && data().item("treasure_map").is_some() { let mut m = Item::new("treasure_map", 1); m.tag = treasure_target(land, tx, ty, &mut b); loot.push(m); }
        for it in loot { g.items.push(((x + 1, y), it)); }
    } }
    // The old stones: a ring about an altar (a blessing for the weary).
    if !town_here && b.chance(0.1) { if let Some((x, y)) = free_near(g, &mut b, None, 0) {
        for (dx, dy) in [(0, -3), (2, -2), (3, 0), (2, 2), (0, 3), (-2, 2), (-3, 0), (-2, -2)] { if b.chance(0.85) { put(g, x + dx, y + dy, Feature::Statue); } }
        put(g, x, y, Feature::Altar);
    } }
    // A hermit far from the towns, who knows the land.
    if !town_here && danger > 70 && b.chance(0.12) { if let Some((x, y)) = free_near(g, &mut b, None, 0) {
        for yy in y - 2..=y + 2 { for xx in x - 3..=x + 3 { let edge = (xx - x).abs() == 3 || (yy - y).abs() == 2; if inside(xx, yy) { g.tiles[idx(xx, yy)] = if edge { Tile::wall(Wall::Timber, Ground::Wood) } else { Tile::floor(Ground::Wood) }; } } }
        put(g, x, y + 2, Feature::Door { open: false, lock: 0 });
        put(g, x - 2, y - 1, Feature::Bed);
        put(g, x + 2, y - 1, Feature::Bookshelf);
        for yy in y - 2..=y + 2 { for xx in x - 3..=x + 3 { if inside(xx, yy) { g.safe[idx(xx, yy)] = true; } } }
        let people = ["human", "elf", "dwarf", "halfling"][b.range(0, 3) as usize];
        g.npcs.push(Npc { name: super::town::person_name(people, hash(land.seed(), tx, ty, 0x4E)), role: Role::Sage, x: x + 1, y, z: 0, post: (x + 1, y), race: people.into(), female: b.chance(0.5), of: "hermit".into(), home: 0, met: Default::default() });
    } }
    // A farmstead where the land is farmed.
    if !town_here && info.farmland[k] > 90 && b.chance(0.6) { if let Some((x, y)) = free_near(g, &mut b, None, 0) {
        for yy in y - 2..=y + 1 { for xx in x - 2..=x + 2 { let edge = (xx - x).abs() == 2 || yy == y - 2 || yy == y + 1; if inside(xx, yy) { g.tiles[idx(xx, yy)] = if edge { Tile::wall(Wall::Timber, Ground::Wood) } else { Tile::floor(Ground::Wood) }; } } }
        put(g, x, y + 1, Feature::Door { open: false, lock: 0 });
        put(g, x - 1, y - 1, Feature::Bed);
        put(g, x + 1, y - 1, Feature::Barrel);
        put(g, x + 3, y + 2, Feature::Well);
        for yy in y - 2..=y + 2 { for xx in x - 2..=x + 3 { if inside(xx, yy) { g.safe[idx(xx, yy)] = true; } } }
        g.npcs.push(Npc { name: super::town::person_name("human", hash(land.seed(), tx, ty, 0xFA4)), role: Role::Townsfolk, x, y: y + 2, z: 0, post: (x, y + 2), race: "human".into(), female: b.chance(0.5), of: "farmer".into(), home: 0, met: Default::default() });
    } }
    // A battlefield of the history: bones and rusted arms in the grass.
    if info.battles[k] > 0 {
        let a = land.anchor(tx, ty);
        let (ax, ay) = ((a.0 as i64 - tx * CH as i64) as i32, (a.1 as i64 - ty * CH as i64) as i32);
        let n = 12 + 10 * info.battles[k].min(4) as i32;
        for _ in 0..n {
            let (x, y) = (ax + b.range(-22, 22), ay + b.range(-16, 16));
            if !inside(x, y) || g.safe[idx(x, y)] { continue; }
            let t = &g.tiles[idx(x, y)];
            if t.wall != Wall::None || t.feature != Feature::None || matches!(t.ground, Ground::Water | Ground::Shallows) { continue; }
            g.tiles[idx(x, y)].feature = Feature::Bones;
            if b.chance(0.18) { let id = ["sword", "spear", "axe", "mace", "chain_helmet", "round_shield"][b.range(0, 5) as usize]; g.items.push(((x, y), Item::of(id, "iron", 0))); }
        }
    }
    // Herbs in the woods and meadows.
    if matches!(g.kind, Kind::Forest | Kind::Jungle | Kind::Taiga | Kind::Plains | Kind::Swamp) && data().item("herbs").is_some() {
        for _ in 0..b.range(1, 4) { if let Some((x, y)) = free_near(g, &mut b, None, 0) { g.items.push(((x, y), Item::new("herbs", 1))); } }
    }
}

/// A treasure map's mark: a tile some way off (packed as y * 65536 + x) where a cache lies.
pub fn treasure_target(land: &Land, tx: i64, ty: i64, b: &mut Builder) -> u32 {
    let w = land.info.w as i64;
    for _ in 0..40 {
        let (x, y) = (tx + b.range(-8, 8) as i64, ty + b.range(-6, 6) as i64);
        if let Some(k) = land.idx(x, y) { if land.info.land[k] && !land.atlas.towns.contains_key(&k) { return (y as u32) << 16 | x.rem_euclid(w) as u32; } }
    }
    (ty as u32) << 16 | tx.rem_euclid(w) as u32
}

/// Where on its tile a treasure map's cache is buried (chunk cell).
pub fn cache_cell(land: &Land, tile: (usize, usize)) -> (i32, i32) {
    let h = hash(land.seed(), tile.0 as i64, tile.1 as i64, 0xCAC4E);
    (12 + (h % (CH as u64 - 24)) as i32, 12 + ((h >> 20) % (CH as u64 - 24)) as i32)
}

/// The perils of a tile now: monster kinds (weighted) and how many bands roam it.
pub fn perils(land: &Land, k: usize, night: bool, full_moon: bool) -> (Vec<&'static str>, usize, u32) {
    let info = land.info;
    let kd = land.kind(k);
    let danger = info.danger[k] as u32;
    let sh = info.shadow[k];
    let mut tier = (1 + danger * 3 / 255).clamp(1, 5);
    if sh > 0.4 && danger > 60 { tier = (tier + 1).min(5); }
    // About a town the watch keeps the country: small game and vermin, few of them.
    let by_town = (-1..=1).any(|dy: i64| (-1..=1).any(|dx: i64| land.idx((k % info.w) as i64 + dx, (k / info.w) as i64 + dy).map_or(false, |q| land.atlas.towns.contains_key(&q))));
    if by_town { tier = tier.min(if sh > 0.5 { 2 } else { 1 }); }
    let mut habs: Vec<String> = vec![kd.habitat().into()];
    if info.road[k] { habs.push("road".into()); }
    if info.battles[k] > 0 && night { habs.push("battlefield".into()); }
    if sh > 0.25 { habs.push("shadowland".into()); }
    if night { habs.push("night".into()); habs.push(format!("{}_night", kd.habitat())); if full_moon { habs.push("full_moon".into()); } }
    let mut v: Vec<&'static str> = Vec::new();
    for hab in &habs {
        for m in data().living_in(hab, tier) {
            let wgt = match tier.saturating_sub(m.tier) { 0 => 3, 1 => 3, 2 => 2, _ => 1 };
            for _ in 0..wgt { v.push(m.id.as_str()); }
        }
    }
    let bands = if by_town { 2 } else { (2 + danger as usize / 40 + if night { 2 } else { 0 }).min(10) };
    (v, bands, tier)
}

/// A floor for the land: `w` x `h` cells (outdoor).
pub fn land_floor(w: usize, h: usize) -> Floor { Floor::new(w, h, Tile::floor(Ground::Grass), "the land", true) }

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> WorldInfo {
        let (w, h) = (8usize, 6usize);
        let mut i = WorldInfo::default();
        i.w = w; i.h = h;
        i.land = (0..w * h).map(|k| { let (x, y) = (k % w, k / w); !(x == 0 || y == 0 || y == h - 1) }).collect();
        i.ground = vec![Ground::Grass; w * h];
        i.danger = (0..w * h).map(|k| (k * 9 % 200) as u8).collect();
        i.elevation = (0..w * h).map(|k| if i.land[k] { 200.0 + (k % 5) as f32 * 400.0 } else { -500.0 }).collect();
        i.biome = (0..w * h).map(|k| if !i.land[k] { ExtendedBiome::Ocean } else if k % 3 == 0 { ExtendedBiome::TemperateForest } else { ExtendedBiome::TemperateGrassland }).collect();
        i.temperature = vec![12.0; w * h];
        i.moisture = vec![0.5; w * h];
        i.river = (0..w * h).map(|k| k % w == 3 && i.land[k]).collect();
        i.downhill = (0..w * h).map(|k| if k % w == 3 { 4 } else { 255 }).collect();
        i.road = (0..w * h).map(|k| k / w == 2 && i.land[k]).collect();
        i.forest = vec![0; w * h];
        i.farmland = vec![0; w * h];
        i.shadow = vec![0.0; w * h];
        i.battles = vec![0; w * h];
        i
    }

    /// Across a tile's border the land goes on: the cells on both sides of every border agree
    /// with what each chunk made (the same cell is the same in either chunk's reckoning), and
    /// rivers and roads cross it.
    #[test]
    fn the_land_runs_on_across_borders() {
        let i = info();
        let atlas = Atlas::new(&i, &[], 7);
        let land = Land { info: &i, atlas: &atlas };
        let a = generate(&land, 3, 2, &[]);
        let b = generate(&land, 4, 2, &[]);
        let c = generate(&land, 3, 3, &[]);
        let d = generate(&land, 3, 2, &[]);
        assert!(a.tiles.iter().zip(d.tiles.iter()).all(|(p, q)| p == q), "a chunk is the same twice");
        // The road along row 2 leaves the east edge of (3,2) and enters the west edge of (4,2).
        let road = |t: &Tile| matches!(t.ground, Ground::Earth | Ground::Cobbles | Ground::Wood);
        let east: Vec<i32> = (0..CH).filter(|&y| road(&a.tiles[idx(CH - 1, y)])).collect();
        let west: Vec<i32> = (0..CH).filter(|&y| road(&b.tiles[idx(0, y)])).collect();
        assert!(!east.is_empty() && !west.is_empty(), "a road crosses the border");
        assert!(east.iter().any(|y| west.iter().any(|w| (y - w).abs() <= 2)), "the road meets itself: {:?} vs {:?}", east, west);
        // The river in column 3 runs south across the border of (3,2) and (3,3).
        let wet = |t: &Tile| matches!(t.ground, Ground::Water | Ground::Shallows);
        let south: Vec<i32> = (0..CH).filter(|&x| wet(&a.tiles[idx(x, CH - 1)])).collect();
        let north: Vec<i32> = (0..CH).filter(|&x| wet(&c.tiles[idx(x, 0)])).collect();
        assert!(south.iter().any(|x| north.iter().any(|n| (x - n).abs() <= 2)), "the river runs on: {:?} vs {:?}", south, north);
    }
}
