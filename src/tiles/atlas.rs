//! Tile atlas: the pixel-art tiles the graphical viewer draws the world with.
//!
//! The default atlas is drawn procedurally in an ink-cartography style (32x32 pixels, 4
//! variants per kind). It can be
//! exported as a PNG (`--export-tileset`), edited, and loaded back (`--tileset`). A Dwarf
//! Fortress style CP437 sheet (16x16 glyph grid, magenta or transparent background) can be
//! loaded instead: each tile kind maps to a glyph tinted with that kind's colours, as in DF.

use std::error::Error;
use std::path::Path;

use image::{Rgba, RgbaImage};

/// Variants per tile kind (chosen per world tile by hash so the map doesn't repeat).
pub const VARIANTS: usize = 4;
/// Size of the generated tiles in pixels.
pub const GENERATED_TILE_PX: usize = 32;

pub type Px = [u8; 4];

/// Everything the viewer can draw. Ground kinds fill a tile; the rest are sprites drawn on
/// top of a ground tile (transparent background).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TileKind {
    DeepOcean,
    Ocean,
    Shallows,
    Lake,
    SeaIce,
    Grass,
    Steppe,
    Savanna,
    Sand,
    Salt,
    Tundra,
    Snow,
    Swamp,
    JungleFloor,
    Rock,
    Ash,
    Lava,
    Beach,
    Dirt,
    Gravel,
    StoneFloor,
    StoneWall,
    SoilWall,
    Deciduous,
    Conifer,
    Jungle,
    Palm,
    Acacia,
    DeadTree,
    Hills,
    Mountain,
    SnowPeak,
    Volcano,
    Mushroom,
    Crystal,
    Ruins,
    Shrub,
    Boulder,
    Ramp,
    BigBroadleaf,
    BigConifer,
    BigJungle,
    Village,
    Town,
    City,
    Castle,
    WoodWall,
    BlockWall,
    WoodFloor,
    Crops,
    OreWall,
    /// Cultivated land around settlements (ground): a patchwork of ploughed strips.
    Fields,
    /// Scattered skulls and bones (battlefields).
    Bones,
    /// A giant's ribcage and skull.
    TitanBones,
    /// Animal signs in playable areas.
    Burrow,
    Nest,
    Den,
}

pub const ALL_KINDS: [TileKind; 57] = [
    TileKind::DeepOcean, TileKind::Ocean, TileKind::Shallows, TileKind::Lake, TileKind::SeaIce,
    TileKind::Grass, TileKind::Steppe, TileKind::Savanna, TileKind::Sand, TileKind::Salt,
    TileKind::Tundra, TileKind::Snow, TileKind::Swamp, TileKind::JungleFloor, TileKind::Rock,
    TileKind::Ash, TileKind::Lava, TileKind::Beach,
    TileKind::Dirt, TileKind::Gravel, TileKind::StoneFloor, TileKind::StoneWall, TileKind::SoilWall,
    TileKind::Deciduous, TileKind::Conifer, TileKind::Jungle, TileKind::Palm, TileKind::Acacia,
    TileKind::DeadTree, TileKind::Hills, TileKind::Mountain, TileKind::SnowPeak, TileKind::Volcano,
    TileKind::Mushroom, TileKind::Crystal, TileKind::Ruins,
    TileKind::Shrub, TileKind::Boulder, TileKind::Ramp,
    TileKind::BigBroadleaf, TileKind::BigConifer, TileKind::BigJungle,
    TileKind::Village, TileKind::Town, TileKind::City, TileKind::Castle,
    TileKind::WoodWall, TileKind::BlockWall, TileKind::WoodFloor, TileKind::Crops,
    TileKind::OreWall, TileKind::Fields, TileKind::Bones, TileKind::TitanBones,
    TileKind::Burrow, TileKind::Nest, TileKind::Den,
];

impl TileKind {
    pub fn index(self) -> usize {
        ALL_KINDS.iter().position(|&k| k == self).unwrap()
    }

    pub fn is_sprite(self) -> bool {
        self.index() >= TileKind::Deciduous.index() && self != TileKind::Fields
    }

    pub fn is_water(self) -> bool {
        matches!(self, TileKind::DeepOcean | TileKind::Ocean | TileKind::Shallows | TileKind::Lake)
    }

    /// CP437 glyph and (foreground, background) colours used with a DF-style tileset.
    pub fn cp437(self) -> (u8, [u8; 3], [u8; 3]) {
        use TileKind::*;
        match self {
            DeepOcean => (247, [40, 70, 150], [8, 20, 55]),
            Ocean => (247, [70, 120, 200], [16, 42, 95]),
            Shallows => (126, [130, 190, 230], [35, 85, 140]),
            Lake => (247, [110, 170, 225], [30, 75, 130]),
            SeaIce => (176, [235, 245, 255], [150, 185, 210]),
            Grass => (34, [110, 175, 70], [40, 75, 30]),
            Steppe => (44, [185, 190, 100], [80, 85, 40]),
            Savanna => (34, [215, 185, 100], [100, 80, 40]),
            Sand => (126, [245, 220, 150], [170, 140, 80]),
            Salt => (250, [250, 250, 245], [180, 175, 165]),
            Tundra => (46, [165, 175, 150], [70, 78, 62]),
            Snow => (176, [250, 252, 255], [185, 195, 210]),
            Swamp => (34, [95, 140, 80], [35, 50, 30]),
            JungleFloor => (44, [60, 140, 55], [20, 50, 20]),
            Rock => (46, [170, 165, 160], [75, 72, 68]),
            Ash => (46, [120, 112, 108], [40, 37, 36]),
            Lava => (247, [255, 170, 50], [130, 30, 10]),
            Beach => (46, [250, 235, 190], [190, 170, 120]),
            Dirt => (46, [150, 110, 70], [80, 56, 34]),
            Gravel => (250, [190, 185, 175], [100, 96, 90]),
            StoneFloor => (43, [170, 168, 165], [70, 68, 66]),
            StoneWall => (219, [150, 146, 140], [40, 38, 36]),
            SoilWall => (219, [130, 96, 62], [50, 36, 22]),
            Deciduous => (5, [80, 170, 60], [0, 0, 0]),
            Conifer => (24, [45, 120, 70], [0, 0, 0]),
            Jungle => (6, [60, 180, 70], [0, 0, 0]),
            Palm => (231, [110, 190, 80], [0, 0, 0]),
            Acacia => (226, [150, 160, 70], [0, 0, 0]),
            DeadTree => (231, [140, 110, 80], [0, 0, 0]),
            Hills => (239, [160, 170, 110], [0, 0, 0]),
            Mountain => (30, [175, 170, 165], [0, 0, 0]),
            SnowPeak => (30, [250, 252, 255], [0, 0, 0]),
            Volcano => (30, [230, 80, 40], [0, 0, 0]),
            Mushroom => (6, [200, 80, 200], [0, 0, 0]),
            Crystal => (42, [140, 230, 250], [0, 0, 0]),
            Ruins => (35, [190, 185, 170], [0, 0, 0]),
            Shrub => (231, [90, 150, 70], [0, 0, 0]),
            Boulder => (7, [170, 165, 160], [0, 0, 0]),
            Ramp => (30, [235, 235, 220], [0, 0, 0]),
            BigBroadleaf => (5, [80, 170, 60], [0, 0, 0]),
            BigConifer => (24, [45, 120, 70], [0, 0, 0]),
            BigJungle => (6, [60, 180, 70], [0, 0, 0]),
            Village => (127, [200, 160, 110], [0, 0, 0]),
            Town => (127, [230, 190, 130], [0, 0, 0]),
            City => (15, [240, 220, 170], [0, 0, 0]),
            Castle => (35, [220, 210, 200], [0, 0, 0]),
            WoodWall => (219, [150, 100, 56], [60, 40, 20]),
            BlockWall => (219, [190, 186, 178], [70, 68, 64]),
            WoodFloor => (43, [170, 120, 70], [100, 68, 38]),
            Crops => (34, [210, 190, 90], [0, 0, 0]),
            OreWall => (219, [150, 146, 140], [40, 38, 36]),
            Fields => (240, [190, 170, 110], [120, 100, 60]),
            Bones => (250, [230, 222, 200], [0, 0, 0]),
            TitanBones => (21, [236, 228, 206], [0, 0, 0]),
            Burrow => (9, [120, 92, 64], [0, 0, 0]),
            Nest => (15, [150, 120, 80], [0, 0, 0]),
            Den => (79, [90, 70, 52], [0, 0, 0]),
        }
    }
}

/// A set of square tiles, `VARIANTS` per kind, with pre-shrunk copies for zoomed-out views.
pub struct Atlas {
    pub size: usize,
    /// levels[0] is full size; each further level halves the tile size (box filtered).
    levels: Vec<(usize, Vec<Vec<Px>>)>,
}

impl Atlas {
    fn from_tiles(size: usize, tiles: Vec<Vec<Px>>) -> Self {
        let mut levels = vec![(size, tiles)];
        while levels.last().unwrap().0 > 2 {
            let (s, prev) = levels.last().unwrap();
            let half = s / 2;
            let next = prev.iter().map(|t| downsample(t, *s, half)).collect();
            levels.push((half, next));
        }
        Self { size, levels }
    }

    /// Tile pixels at the smallest stored size that is still >= `px` (or the base size).
    pub fn tile_for(&self, kind: TileKind, variant: usize, px: usize) -> (&[Px], usize) {
        let mut best = &self.levels[0];
        for lvl in &self.levels {
            if lvl.0 >= px { best = lvl; }
        }
        (&best.1[kind.index() * VARIANTS + variant % VARIANTS], best.0)
    }

    /// Average colour of a tile (used for the minimap and very small zoom levels).
    pub fn average(&self, kind: TileKind, variant: usize) -> [u8; 3] {
        let (t, _) = self.tile_for(kind, variant, 1);
        let (mut acc, mut n) = ([0u32; 3], 0u32);
        for p in t {
            if p[3] > 0 {
                for c in 0..3 { acc[c] += p[c] as u32; }
                n += 1;
            }
        }
        let n = n.max(1);
        [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
    }

    /// The built-in pixel-art atlas.
    pub fn generated() -> Self {
        let s = GENERATED_TILE_PX;
        let mut tiles = Vec::with_capacity(ALL_KINDS.len() * VARIANTS);
        for &kind in &ALL_KINDS {
            for v in 0..VARIANTS {
                let mut c = Canvas::new(s, (kind.index() as u64 + 1) * 7919 + v as u64 * 104729);
                paint(kind, &mut c);
                tiles.push(c.px);
            }
        }
        Self::from_tiles(s, tiles)
    }

    /// Save in the native layout: one row per kind (in `ALL_KINDS` order), `VARIANTS` columns.
    pub fn save_png(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let (s, tiles) = (&self.levels[0].0, &self.levels[0].1);
        let mut img = RgbaImage::new((s * VARIANTS) as u32, (s * ALL_KINDS.len()) as u32);
        for (i, t) in tiles.iter().enumerate() {
            let (col, row) = (i % VARIANTS, i / VARIANTS);
            for y in 0..*s {
                for x in 0..*s {
                    img.put_pixel((col * s + x) as u32, (row * s + y) as u32, Rgba(t[y * s + x]));
                }
            }
        }
        img.save(path)?;
        Ok(())
    }

    /// Load either a native atlas (as written by `save_png`) or a DF-style CP437 sheet.
    pub fn load_png(path: &Path) -> Result<Self, Box<dyn Error>> {
        let img = image::open(path)?.to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        if w % VARIANTS == 0 && h == (w / VARIANTS) * ALL_KINDS.len() {
            let s = w / VARIANTS;
            let mut tiles = Vec::new();
            for row in 0..ALL_KINDS.len() {
                for col in 0..VARIANTS {
                    let mut t = vec![[0u8; 4]; s * s];
                    for y in 0..s {
                        for x in 0..s {
                            t[y * s + x] = img.get_pixel((col * s + x) as u32, (row * s + y) as u32).0;
                        }
                    }
                    tiles.push(t);
                }
            }
            return Ok(Self::from_tiles(s, tiles));
        }
        if w % 16 == 0 && h % 16 == 0 {
            return Ok(Self::from_cp437(&img));
        }
        Err(format!(
            "unrecognised tileset {}x{}: expected a native atlas ({} columns x {} rows of square tiles) or a 16x16 CP437 grid",
            w, h, VARIANTS, ALL_KINDS.len()
        ).into())
    }

    /// Build tiles from a DF CP437 sheet: glyph pixels are tinted with the kind's foreground
    /// colour (by their brightness); background pixels (magenta or transparent) become the
    /// kind's background colour, or transparent for sprites.
    fn from_cp437(img: &RgbaImage) -> Self {
        let (gw, gh) = (img.width() as usize / 16, img.height() as usize / 16);
        let s = gw.max(gh);
        let mut tiles = Vec::new();
        for &kind in &ALL_KINDS {
            let (glyph, fg, bg) = kind.cp437();
            let (gx, gy) = ((glyph as usize % 16) * gw, (glyph as usize / 16) * gh);
            let mut t = vec![[0u8; 4]; s * s];
            for y in 0..s {
                for x in 0..s {
                    let p = img.get_pixel((gx + x * gw / s) as u32, (gy + y * gh / s) as u32).0;
                    let is_bg = p[3] < 128 || (p[0] > 200 && p[1] < 60 && p[2] > 200);
                    let lum = (p[0] as f32 * 0.3 + p[1] as f32 * 0.59 + p[2] as f32 * 0.11) / 255.0;
                    t[y * s + x] = if is_bg || lum < 0.05 {
                        if kind.is_sprite() { [0, 0, 0, 0] } else { [bg[0], bg[1], bg[2], 255] }
                    } else {
                        let f = |c: u8| (c as f32 * lum).min(255.0) as u8;
                        [f(fg[0]), f(fg[1]), f(fg[2]), 255]
                    };
                }
            }
            for _ in 0..VARIANTS { tiles.push(t.clone()); }
        }
        Self::from_tiles(s, tiles)
    }
}

fn downsample(t: &[Px], s: usize, half: usize) -> Vec<Px> {
    let mut out = vec![[0u8; 4]; half * half];
    for y in 0..half {
        for x in 0..half {
            let mut acc = [0u32; 4];
            let mut opaque = 0u32;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = t[(2 * y + dy) * s + 2 * x + dx];
                acc[3] += p[3] as u32;
                if p[3] > 0 {
                    for c in 0..3 { acc[c] += p[c] as u32; }
                    opaque += 1;
                }
            }
            let o = opaque.max(1);
            out[y * half + x] = [(acc[0] / o) as u8, (acc[1] / o) as u8, (acc[2] / o) as u8, (acc[3] / 4) as u8];
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Procedural ink cartography
//
// One style for every tile: flat muted washes on a parchment ground, sepia ink linework, and
// shading done by hatching (lines running "\" on the side away from a top-left light), never
// by darker blobs. Sprites are built from shape masks so every symbol gets the same ink
// outline, fill and hatch treatment.
// ---------------------------------------------------------------------------------------------

const fn rgb(r: u8, g: u8, b: u8) -> Px { [r, g, b, 255] }

/// Sepia ink for all linework.
const INK: Px = rgb(56, 42, 32);
/// Blue-grey ink for marks on water.
const SEA_INK: Px = rgb(40, 66, 82);
/// Light parchment used for lit faces and walls.
const PAPER: Px = rgb(234, 222, 196);
const ROOF: Px = rgb(168, 94, 68);
const WOOD: Px = rgb(132, 98, 66);
const STONE: Px = rgb(208, 198, 178);

/// Hatch pattern: "\" lines every `n` pixels.
fn hatch_line(x: i32, y: i32, n: i32) -> bool { (x - y).rem_euclid(n) == 0 }

/// A set of pixels: shapes are unioned into a mask, then filled, hatched and outlined at once.
#[derive(Clone)]
struct Mask {
    s: i32,
    m: Vec<bool>,
}

impl Mask {
    fn new(s: usize) -> Self { Self { s: s as i32, m: vec![false; s * s] } }
    fn get(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.s && y < self.s && self.m[(y * self.s + x) as usize]
    }
    fn put(&mut self, x: i32, y: i32) {
        if x >= 0 && y >= 0 && x < self.s && y < self.s { self.m[(y * self.s + x) as usize] = true; }
    }
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) -> &mut Self {
        for y in 0..self.s {
            for x in 0..self.s {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 { self.put(x, y); }
            }
        }
        self
    }
    fn disc(&mut self, cx: f32, cy: f32, r: f32) -> &mut Self { self.ellipse(cx, cy, r, r) }
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) -> &mut Self {
        for y in y0..=y1 { for x in x0..=x1 { self.put(x, y); } }
        self
    }
    /// Filled polygon, sampled at pixel centres (even-odd rule).
    fn poly(&mut self, pts: &[(f32, f32)]) -> &mut Self {
        for y in 0..self.s {
            let fy = y as f32 + 0.5;
            let mut xs = Vec::new();
            for k in 0..pts.len() {
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                if (a.1 <= fy) != (b.1 <= fy) {
                    xs.push(a.0 + (fy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks(2) {
                if pair.len() < 2 { continue; }
                for x in 0..self.s {
                    let fx = x as f32 + 0.5;
                    if fx >= pair[0] && fx <= pair[1] { self.put(x, y); }
                }
            }
        }
        self
    }
    fn union(&mut self, o: &Mask) -> &mut Self {
        for (a, b) in self.m.iter_mut().zip(&o.m) { *a |= *b; }
        self
    }
    fn cells(&self) -> Vec<(i32, i32)> {
        (0..self.s * self.s).filter(|&i| self.m[i as usize]).map(|i| (i % self.s, i / self.s)).collect()
    }
}

struct Canvas {
    s: usize,
    px: Vec<Px>,
    rng: u64,
}

impl Canvas {
    fn new(s: usize, seed: u64) -> Self {
        Self { s, px: vec![[0, 0, 0, 0]; s * s], rng: seed | 1 }
    }
    fn rand(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 { lo + (self.rand() % (hi - lo + 1) as u64) as i32 }
    fn unit(&mut self) -> f32 { (self.rand() % 10_000) as f32 / 10_000.0 }
    fn chance(&mut self, p: f32) -> bool { self.unit() < p }
    fn mask(&self) -> Mask { Mask::new(self.s) }

    fn set(&mut self, x: i32, y: i32, c: Px) {
        if x >= 0 && y >= 0 && (x as usize) < self.s && (y as usize) < self.s {
            self.px[y as usize * self.s + x as usize] = c;
        }
    }
    /// Paint `c` over a pixel with opacity `a` (onto transparent pixels it sets a translucent one).
    fn blend(&mut self, x: i32, y: i32, c: Px, a: f32) {
        if x < 0 || y < 0 || x as usize >= self.s || y as usize >= self.s { return; }
        let p = &mut self.px[y as usize * self.s + x as usize];
        let pa = p[3] as f32 / 255.0;
        let oa = a + pa * (1.0 - a);
        if oa <= 0.0 { return; }
        for k in 0..3 {
            p[k] = ((c[k] as f32 * a + p[k] as f32 * pa * (1.0 - a)) / oa).round() as u8;
        }
        p[3] = (oa * 255.0).round() as u8;
    }
    fn ink(&mut self, x: i32, y: i32, a: f32) { self.blend(x, y, INK, a); }
    fn fill(&mut self, c: Px) { self.px.iter_mut().for_each(|p| *p = c); }
    /// Fine paper grain: tiny brightness jitter on every opaque pixel.
    fn grain(&mut self, amount: f32) {
        for i in 0..self.px.len() {
            let f = 1.0 + amount * (self.unit() * 2.0 - 1.0);
            let p = &mut self.px[i];
            for k in 0..3 { p[k] = (p[k] as f32 * f).clamp(0.0, 255.0) as u8; }
        }
    }
    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Px, a: f32) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.blend(x, y, c, a);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }
    fn polyline(&mut self, pts: &[(i32, i32)], c: Px, a: f32) {
        for w in pts.windows(2) { self.line(w[0].0, w[0].1, w[1].0, w[1].1, c, a); }
    }
    /// Jittered grid of mark positions (about one per `cell`), kept `margin` px inside the tile
    /// so marks are never cut by the tile edge (cut marks would reveal the grid).
    fn scatter(&mut self, cell: i32, margin: i32, p: f32) -> Vec<(i32, i32)> {
        let s = self.s as i32;
        let n = (s / cell).max(1);
        let mut out = Vec::new();
        for gy in 0..n {
            for gx in 0..n {
                if !self.chance(p) { continue; }
                let (x0, x1) = ((gx * cell).max(margin), (gx * cell + cell - 1).min(s - 1 - margin));
                let (y0, y1) = ((gy * cell).max(margin), (gy * cell + cell - 1).min(s - 1 - margin));
                if x0 > x1 || y0 > y1 { continue; }
                out.push((self.range(x0, x1), self.range(y0, y1)));
            }
        }
        out
    }
    fn dots(&mut self, c: Px, density: f32, a: f32) {
        let n = (self.s * self.s) as f32 * density;
        for _ in 0..n as usize {
            let (x, y) = (self.range(1, self.s as i32 - 2), self.range(1, self.s as i32 - 2));
            self.blend(x, y, c, a);
        }
    }

    // --- marks ------------------------------------------------------------------------------

    /// Grass tuft: three short strokes fanning up from a point.
    fn tuft(&mut self, x: i32, y: i32, a: f32) {
        for (dx, dy) in [(0, 0), (0, -1), (0, -2), (-1, -1), (-2, -2), (1, -1), (2, -2)] {
            self.ink(x + dx, y + dy, a);
        }
    }
    /// Marsh symbol: a waterline with reeds above it.
    fn marsh(&mut self, x: i32, y: i32, a: f32) {
        for dx in -3..=3 { self.ink(x + dx, y, a); }
        for (dx, h) in [(-2, 2), (0, 3), (2, 2)] {
            for k in 1..=h { self.ink(x + dx, y - k, a); }
        }
    }
    /// Wave mark: a small double crest.
    fn wave(&mut self, x: i32, y: i32, a: f32) {
        for (dx, dy) in [(0, 1), (1, 0), (2, 0), (3, 1), (4, 0), (5, 0), (6, 1)] {
            self.blend(x + dx, y + dy, SEA_INK, a);
        }
    }
    /// Shallow dune / drift arc.
    fn arc(&mut self, x: i32, y: i32, w: i32, c: Px, a: f32) {
        for i in 0..w {
            let t = i as f32 / (w - 1).max(1) as f32;
            let dy = -(t * std::f32::consts::PI).sin() * 1.6;
            self.blend(x + i, y + dy.round() as i32, c, a);
        }
    }
    /// Random crack: a short wandering polyline.
    fn crack(&mut self, c: Px, a: f32) {
        let s = self.s as i32;
        let (mut x, mut y) = (self.range(2, s - 3), self.range(2, s - 3));
        let mut pts = vec![(x, y)];
        for _ in 0..self.range(2, 4) {
            x = (x + self.range(-6, 6)).clamp(1, s - 2);
            y = (y + self.range(-6, 6)).clamp(1, s - 2);
            pts.push((x, y));
        }
        self.polyline(&pts, c, a);
    }

    // --- symbols ----------------------------------------------------------------------------

    fn fill_mask(&mut self, m: &Mask, c: Px) {
        for (x, y) in m.cells() { self.set(x, y, c); }
    }
    fn clear_mask(&mut self, m: &Mask) {
        for (x, y) in m.cells() { self.set(x, y, [0, 0, 0, 0]); }
    }
    fn hatch(&mut self, m: &Mask, a: f32, f: impl Fn(i32, i32) -> bool) {
        for (x, y) in m.cells() { if f(x, y) { self.ink(x, y, a); } }
    }
    /// Ink the mask's edge pixels. Rows below `open_below` count as inside, so a symbol can stand
    /// on the ground without a base line.
    fn outline(&mut self, m: &Mask, a: f32, open_below: i32) {
        let inside = |x: i32, y: i32| m.get(x, y) || y > open_below;
        let edge: Vec<(i32, i32)> = m.cells().into_iter()
            .filter(|&(x, y)| !inside(x - 1, y) || !inside(x + 1, y) || !inside(x, y - 1) || !inside(x, y + 1))
            .collect();
        for (x, y) in edge { self.ink(x, y, a); }
    }
    /// The standard symbol: flat fill, hatched shadow side, ink outline.
    fn symbol(&mut self, m: &Mask, fill: Px, hatch_a: f32, shadow: impl Fn(i32, i32) -> bool) {
        self.fill_mask(m, fill);
        self.hatch(m, hatch_a, shadow);
        self.outline(m, 0.92, i32::MAX);
    }
    /// Soft cast shadow on the ground under a symbol.
    fn shadow(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        let mut m = self.mask();
        m.ellipse(cx, cy, rx, ry);
        for (x, y) in m.cells() {
            if self.px[y as usize * self.s + x as usize][3] == 0 { self.blend(x, y, INK, 0.16); }
        }
    }

    /// Round-crowned tree: a cloud of discs over a short trunk.
    fn broadleaf(&mut self, cx: f32, cy: f32, r: f32, fill: Px) {
        self.shadow(cx + 1.5, cy + r + 1.0, r * 0.9, r * 0.3);
        let mut trunk = self.mask();
        trunk.rect(cx as i32 - 1, (cy + r * 0.4) as i32, cx as i32, (cy + r + 1.5) as i32);
        self.symbol(&trunk, WOOD, 0.0, |_, _| false);
        let mut m = self.mask();
        m.disc(cx, cy, r);
        let k = self.range(4, 5);
        let rot = self.unit() * std::f32::consts::TAU;
        for j in 0..k {
            let ang = rot + j as f32 / k as f32 * std::f32::consts::TAU;
            m.disc(cx + ang.cos() * r * 0.55, cy + ang.sin() * r * 0.45, r * 0.6);
        }
        self.symbol(&m, fill, 0.5, |x, y| hatch_line(x, y, 3) && (x as f32 - cx) + (y as f32 - cy) > r * 0.1);
    }
    /// Two-tier conifer standing on (x, base).
    fn conifer(&mut self, x: f32, base: f32, h: f32, w: f32, fill: Px) {
        self.shadow(x + 1.5, base, w * 0.9, 1.6);
        let mut trunk = self.mask();
        trunk.rect(x as i32, (base - 3.0) as i32, x as i32, base as i32);
        self.symbol(&trunk, WOOD, 0.0, |_, _| false);
        let mut m = self.mask();
        m.poly(&[(x + 0.5, base - h), (x + 0.5 + w * 0.68, base - h * 0.42), (x + 0.5 - w * 0.68, base - h * 0.42)]);
        m.poly(&[(x + 0.5, base - h * 0.72), (x + 0.5 + w, base - 1.5), (x + 0.5 - w, base - 1.5)]);
        self.symbol(&m, fill, 0.5, |px, py| px as f32 > x && hatch_line(px, py, 3));
    }
    /// Lumpy rounded rock.
    fn boulder(&mut self, cx: f32, cy: f32, r: f32) {
        self.shadow(cx + 1.5, cy + r * 0.8, r * 1.1, r * 0.45);
        let mut m = self.mask();
        m.ellipse(cx, cy, r, r * 0.8);
        m.disc(cx - r * 0.45, cy + r * 0.15, r * 0.6);
        self.symbol(&m, STONE, 0.5, |x, y| hatch_line(x, y, 2) && (x as f32 - cx) + (y as f32 - cy) > r * 0.2);
    }
    /// A mountain: lit left face, hatched right face divided by a ridge line, open at the base.
    fn mountain(&mut self, cx: f32, top: f32, base: f32, half_w: f32, snow: bool) {
        let h = base - top;
        let mut m = self.mask();
        let shoulder = if self.chance(0.5) { -1.0 } else { 1.0 };
        m.poly(&[
            (cx - half_w, base + 1.0),
            (cx - half_w * 0.45, top + h * 0.45),
            (cx - half_w * 0.3, top + h * 0.38 + shoulder),
            (cx, top),
            (cx + half_w * 0.3, top + h * 0.33),
            (cx + half_w * 0.45, top + h * 0.3 - shoulder),
            (cx + half_w, base + 1.0),
        ]);
        let ridge = |y: i32| cx + ((y as f32 - top) / h) * half_w * 0.18 + ((y * 5) % 3) as f32 * 0.4 - 0.4;
        let snow_line = |x: i32| top + h * 0.4 + ((x * 7) % 5) as f32 * 0.7 - 1.4;
        for (x, y) in m.cells() {
            let shaded = x as f32 + 0.5 > ridge(y);
            let capped = snow && (y as f32) < snow_line(x);
            let c = match (capped, shaded) {
                (true, false) => rgb(246, 244, 238),
                (true, true) => rgb(214, 220, 222),
                (false, false) => rgb(222, 208, 182),
                (false, true) => rgb(184, 168, 142),
            };
            self.set(x, y, c);
            if shaded && hatch_line(x, y, if capped { 4 } else { 2 }) {
                if capped { self.blend(x, y, SEA_INK, 0.3); } else { self.ink(x, y, 0.55); }
            }
        }
        // Ridge stroke from the summit partway down.
        for y in top as i32..(top + h * 0.7) as i32 {
            let x = ridge(y).floor() as i32;
            if m.get(x, y) { self.ink(x, y, 0.75); }
        }
        self.outline(&m, 0.95, base as i32);
    }
    /// A house seen from the front: walls with a pitched roof.
    fn house(&mut self, x: i32, y: i32, w: i32, wall_h: i32, roof_h: i32, roof: Px) {
        self.shadow(x as f32 + w as f32 * 0.5 + 1.5, (y + wall_h) as f32, w as f32 * 0.6, 1.4);
        let mut walls = self.mask();
        walls.rect(x, y, x + w - 1, y + wall_h - 1);
        let mut rf = self.mask();
        rf.poly(&[(x as f32 - 1.0, y as f32 + 0.6), (x as f32 + w as f32 * 0.5, (y - roof_h) as f32), (x as f32 + w as f32 + 1.0, y as f32 + 0.6)]);
        let mid = x as f32 + w as f32 * 0.5;
        self.fill_mask(&walls, PAPER);
        self.hatch(&walls, 0.4, |px, py| px as f32 >= mid + 1.0 && hatch_line(px, py, 2));
        self.fill_mask(&rf, roof);
        self.hatch(&rf, 0.45, |px, py| px as f32 >= mid && hatch_line(px, py, 2));
        let mut all = walls.clone();
        all.union(&rf);
        self.outline(&all, 0.92, i32::MAX);
        self.outline(&rf, 0.6, i32::MAX);
        let dx = x + w / 2 - 1;
        self.ink(dx, y + wall_h - 2, 0.9);
        self.ink(dx, y + wall_h - 3, 0.9);
    }
    /// A round or square tower with a pointed roof.
    fn tower(&mut self, x: i32, top: i32, base: i32, w: i32, roof: Px) {
        self.shadow(x as f32 + w as f32 * 0.5 + 1.5, base as f32 + 0.5, w as f32 * 0.7, 1.4);
        let mut body = self.mask();
        body.rect(x, top, x + w - 1, base);
        let mut cap = self.mask();
        cap.poly(&[(x as f32 - 1.0, top as f32 + 0.6), (x as f32 + w as f32 * 0.5, (top - w - 1) as f32), (x as f32 + w as f32 + 1.0, top as f32 + 0.6)]);
        let mid = x as f32 + w as f32 * 0.5;
        self.fill_mask(&body, STONE);
        self.hatch(&body, 0.45, |px, py| px as f32 >= mid && hatch_line(px, py, 2));
        self.fill_mask(&cap, roof);
        self.hatch(&cap, 0.45, |px, py| px as f32 >= mid && hatch_line(px, py, 2));
        let mut all = body.clone();
        all.union(&cap);
        self.outline(&all, 0.92, i32::MAX);
        self.outline(&cap, 0.6, i32::MAX);
        self.ink(x + w / 2, top + 3, 0.9);
    }
}

fn ground(c: &mut Canvas, base: Px) {
    c.fill(base);
    c.grain(0.025);
}

fn paint(kind: TileKind, c: &mut Canvas) {
    use TileKind::*;
    let s = c.s as i32;
    let sf = s as f32;
    match kind {
        // --- water: flat washes with sparse wave marks ---------------------------------------
        DeepOcean => {
            ground(c, rgb(84, 114, 130));
            for (x, y) in c.scatter(16, 3, 0.45) { c.wave(x - 3, y, 0.2); }
        }
        Ocean => {
            ground(c, rgb(112, 146, 156));
            for (x, y) in c.scatter(13, 3, 0.55) { c.wave(x - 3, y, 0.26); }
        }
        Shallows => {
            ground(c, rgb(150, 180, 176));
            for (x, y) in c.scatter(16, 3, 0.4) { c.wave(x - 3, y, 0.18); }
        }
        Lake => {
            ground(c, rgb(126, 160, 166));
            for (x, y) in c.scatter(16, 3, 0.35) { c.wave(x - 3, y, 0.22); }
        }
        SeaIce => {
            ground(c, rgb(222, 230, 228));
            for _ in 0..2 { c.crack(rgb(130, 154, 166), 0.55); }
            c.dots(rgb(150, 170, 180), 0.01, 0.5);
        }
        // --- land washes ----------------------------------------------------------------------
        Grass => {
            ground(c, rgb(170, 174, 120));
            for (x, y) in c.scatter(10, 3, 0.55) { c.tuft(x, y, 0.38); }
        }
        Steppe => {
            ground(c, rgb(196, 188, 132));
            for (x, y) in c.scatter(8, 3, 0.6) {
                let len = c.range(2, 4);
                c.line(x - len / 2, y, x + len / 2, y, INK, 0.3);
            }
            for (x, y) in c.scatter(16, 3, 0.4) { c.tuft(x, y, 0.3); }
        }
        Savanna => {
            ground(c, rgb(206, 180, 116));
            for (x, y) in c.scatter(11, 3, 0.5) { c.tuft(x, y, 0.36); }
            c.dots(INK, 0.012, 0.3);
        }
        Sand => {
            ground(c, rgb(226, 206, 156));
            c.dots(INK, 0.03, 0.26);
            for (x, y) in c.scatter(16, 5, 0.6) { c.arc(x - 4, y, 9, INK, 0.3); }
        }
        Salt => {
            ground(c, rgb(234, 230, 214));
            for _ in 0..2 { c.crack(INK, 0.18); }
            c.dots(INK, 0.008, 0.25);
        }
        Tundra => {
            ground(c, rgb(178, 178, 150));
            c.dots(INK, 0.014, 0.32);
            for (x, y) in c.scatter(12, 3, 0.35) { c.tuft(x, y, 0.28); }
        }
        Snow => {
            ground(c, rgb(240, 238, 230));
            c.dots(rgb(140, 160, 176), 0.008, 0.45);
            for (x, y) in c.scatter(16, 5, 0.35) { c.arc(x - 3, y, 7, rgb(150, 170, 184), 0.4); }
        }
        Swamp => {
            ground(c, rgb(154, 156, 114));
            for (x, y) in c.scatter(16, 5, 0.5) {
                let mut m = c.mask();
                m.ellipse(x as f32, y as f32, 4.0, 1.6);
                c.fill_mask(&m, rgb(132, 158, 154));
                c.outline(&m, 0.25, i32::MAX);
            }
            for (x, y) in c.scatter(10, 4, 0.55) { c.marsh(x, y, 0.45); }
        }
        JungleFloor => {
            ground(c, rgb(134, 148, 100));
            c.dots(INK, 0.045, 0.3);
        }
        Rock => {
            ground(c, rgb(178, 168, 148));
            for (x, y) in c.scatter(10, 4, 0.55) {
                for k in 0..3 { c.line(x - 1 + k * 2, y + 1, x + 1 + k * 2, y - 1, INK, 0.32); }
            }
            c.dots(INK, 0.01, 0.35);
        }
        Ash => {
            ground(c, rgb(130, 120, 110));
            c.dots(INK, 0.07, 0.35);
        }
        Lava => {
            ground(c, rgb(104, 54, 42));
            for _ in 0..3 { c.crack(rgb(214, 118, 60), 0.9); }
            c.dots(rgb(236, 170, 92), 0.012, 0.9);
        }
        Beach => {
            ground(c, rgb(230, 214, 168));
            c.dots(INK, 0.025, 0.22);
        }
        // --- local (embark) materials ---------------------------------------------------------
        Dirt => {
            ground(c, rgb(174, 144, 108));
            c.dots(INK, 0.04, 0.3);
            for (x, y) in c.scatter(10, 3, 0.4) { c.line(x - 1, y, x + 1, y, INK, 0.25); }
        }
        Gravel => {
            ground(c, rgb(186, 178, 162));
            for (x, y) in c.scatter(6, 2, 0.8) {
                let mut m = c.mask();
                m.ellipse(x as f32 + 0.5, y as f32 + 0.5, 1.8, 1.3);
                c.fill_mask(&m, rgb(206, 198, 182));
                c.outline(&m, 0.45, i32::MAX);
            }
        }
        StoneFloor => {
            // Irregular flagstones.
            ground(c, rgb(192, 186, 172));
            let rows = 4;
            let rh = s / rows;
            for r in 0..rows {
                let y = r * rh;
                c.line(0, y, s - 1, y, INK, 0.32);
                let mut x = c.range(2, 8);
                while x < s - 2 {
                    let dx = c.range(-1, 1);
                    c.line(x, y, x + dx, y + rh - 1, INK, 0.32);
                    x += c.range(7, 12);
                }
            }
            c.dots(INK, 0.01, 0.25);
        }
        StoneWall | OreWall => {
            // Solid rock in section: cross-hatching. Neutral grey: the renderer tints it per rock.
            ground(c, rgb(142, 138, 130));
            for y in 0..s {
                for x in 0..s {
                    if (x + y).rem_euclid(4) == 0 { c.ink(x, y, 0.42); }
                    if (x - y).rem_euclid(4) == 0 { c.ink(x, y, 0.22); }
                }
            }
            if kind == OreWall {
                // Bright flecks; the renderer recolours the near-white pixels by ore.
                for (x, y) in c.scatter(8, 2, 0.8) {
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        c.set(x + dx, y + dy, rgb(255, 255, 255));
                    }
                    c.ink(x - 1, y, 0.6);
                    c.ink(x + 2, y + 1, 0.6);
                }
            }
        }
        SoilWall => {
            ground(c, rgb(150, 118, 86));
            for y in 0..s {
                for x in 0..s {
                    if hatch_line(x, y, 4) { c.ink(x, y, 0.3); }
                }
            }
            c.dots(INK, 0.05, 0.35);
        }
        BlockWall => {
            // Coursed ashlar.
            ground(c, rgb(206, 198, 180));
            for y in 0..s {
                let course = y / 8;
                for x in 0..s {
                    let joint = y % 8 == 7 || (x + if course % 2 == 0 { 0 } else { 5 }) % 10 == 9;
                    if joint { c.ink(x, y, 0.65); } else if y % 8 >= 5 && hatch_line(x, y, 3) { c.ink(x, y, 0.2); }
                }
            }
        }
        WoodWall => {
            // Log courses.
            ground(c, rgb(140, 102, 68));
            for y in 0..s {
                for x in 0..s {
                    if y % 6 == 5 { c.ink(x, y, 0.7); } else if y % 6 == 4 && x % 2 == 0 { c.ink(x, y, 0.3); }
                }
            }
            for (x, y) in c.scatter(8, 2, 0.6) { c.line(x - 2, y, x + 2, y, INK, 0.22); }
        }
        WoodFloor => {
            ground(c, rgb(190, 152, 104));
            for x in (7..s).step_by(8) { c.line(x, 0, x, s - 1, INK, 0.5); }
            for k in 0..4 {
                let (x, y) = (k * 8 + c.range(1, 5), c.range(2, s - 3));
                c.line(k * 8, y, k * 8 + 6, y, INK, 0.4);
                let (y0, y1) = (c.range(1, s - 6), c.range(s - 6, s - 2));
                c.line(x, y0, x, y1, INK, 0.12);
            }
        }
        Fields => {
            // Patchwork: two or three plots of ploughed strips, each its own crop and direction,
            // with ink hedgerows between them.
            ground(c, rgb(192, 184, 128));
            let crops = [rgb(204, 190, 128), rgb(178, 180, 118), rgb(192, 178, 124), rgb(184, 184, 126)];
            let split = c.range(11, 21);
            let vertical_split = c.chance(0.5);
            for plot in 0..2 {
                let col = crops[c.range(0, 3) as usize];
                let dir = c.range(0, 2);
                for y in 0..s {
                    for x in 0..s {
                        let along = if vertical_split { x } else { y };
                        if (along < split) != (plot == 0) { continue; }
                        let k = match dir { 0 => y, 1 => x, _ => x + y };
                        let furrow = k.rem_euclid(4) == 0;
                        c.set(x, y, col);
                        if furrow { c.ink(x, y, 0.16); }
                    }
                }
            }
            c.grain(0.02);
            // Hedgerow along the split and dotted with shrubs.
            for k in 0..s {
                let (x, y) = if vertical_split { (split, k) } else { (k, split) };
                c.ink(x, y, 0.35);
                if k % 9 == 4 {
                    let mut m = c.mask();
                    m.disc(x as f32 + 0.5, y as f32 + 0.5, 1.6);
                    c.symbol(&m, rgb(128, 146, 92), 0.0, |_, _| false);
                }
            }
        }
        // --- vegetation ----------------------------------------------------------------------
        Deciduous => {
            let n = c.range(2, 3);
            let mut trees: Vec<(f32, f32)> = (0..n)
                .map(|k| {
                    let x = sf * (0.25 + 0.5 * (k as f32 / (n - 1).max(1) as f32)) + c.range(-2, 2) as f32;
                    let y = sf * if k % 2 == 0 { 0.36 } else { 0.58 } + c.range(-2, 2) as f32;
                    (x, y)
                })
                .collect();
            trees.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            for (x, y) in trees {
                let r = c.range(55, 70) as f32 / 10.0;
                c.broadleaf(x, y, r, rgb(132, 150, 92));
            }
        }
        Conifer => {
            let n = c.range(3, 4);
            let mut trees: Vec<(f32, f32)> = (0..n)
                .map(|k| {
                    let x = 5.0 + k as f32 * (sf - 10.0) / (n - 1) as f32 + c.range(-1, 1) as f32;
                    let base = sf * if k % 2 == 0 { 0.62 } else { 0.86 } + c.range(-2, 1) as f32;
                    (x, base)
                })
                .collect();
            trees.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            for (x, base) in trees {
                let h = c.range(13, 16) as f32;
                let w = c.range(45, 55) as f32 / 10.0;
                c.conifer(x, base, h, w, rgb(98, 120, 82));
            }
        }
        Jungle => {
            let mut trees: Vec<(f32, f32)> = (0..5).map(|_| (c.range(7, s - 8) as f32, c.range(7, s - 10) as f32)).collect();
            trees.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            for (x, y) in trees {
                let g = c.range(0, 14) as u8;
                let r = c.range(55, 75) as f32 / 10.0;
                c.broadleaf(x, y, r, rgb(100 + g, 132 + g, 80));
            }
        }
        Palm => {
            let (x, base) = (c.range(12, s - 14) as f32, sf - 4.0);
            c.shadow(x + 2.0, base, 6.0, 1.6);
            let (tx, ty) = (x + 4.0, base - 17.0);
            let mut trunk = c.mask();
            for k in 0..=12 {
                let t = k as f32 / 12.0;
                let px = x + (tx - x) * t * t;
                trunk.rect(px as i32, (base - 17.0 * t) as i32, px as i32 + 1, (base - 17.0 * t) as i32);
            }
            c.symbol(&trunk, WOOD, 0.0, |_, _| false);
            let mut fronds = c.mask();
            for (dx, dy) in [(-9.0, 3.0), (9.0, 3.0), (-7.0, -3.0), (7.0, -3.5), (1.0, -6.0), (-3.0, 6.0), (4.0, 6.0)] {
                let (ex, ey) = (tx + dx, ty + dy);
                let (mx, my) = ((tx + ex) * 0.5, (ty + ey) * 0.5 - 1.5);
                let len = ((ex - tx).powi(2) + (ey - ty).powi(2)).sqrt().max(1.0);
                let (nx, ny) = (-(ey - ty) / len * 1.6, (ex - tx) / len * 1.6);
                fronds.poly(&[(tx, ty), (mx + nx, my + ny), (ex, ey), (mx - nx, my - ny)]);
            }
            c.symbol(&fronds, rgb(124, 150, 86), 0.5, |px, py| px as f32 > tx && hatch_line(px, py, 3));
        }
        Acacia => {
            let (x, base) = (c.range(12, s - 13) as f32, sf - 5.0);
            c.shadow(x + 2.0, base, 9.0, 1.8);
            c.line(x as i32, base as i32, x as i32, (base - 8.0) as i32, INK, 0.9);
            c.line(x as i32, (base - 5.0) as i32, x as i32 - 4, (base - 11.0) as i32, INK, 0.9);
            c.line(x as i32, (base - 6.0) as i32, x as i32 + 4, (base - 11.0) as i32, INK, 0.9);
            let mut m = c.mask();
            m.ellipse(x, base - 13.0, 11.0, 3.4);
            m.ellipse(x - 3.0, base - 15.0, 6.0, 2.4);
            c.symbol(&m, rgb(154, 154, 94), 0.5, |px, py| (py as f32) > base - 13.5 && hatch_line(px, py, 3));
        }
        DeadTree => {
            let (x, base) = (c.range(10, s - 11), s - 4);
            c.shadow(x as f32 + 1.0, base as f32, 4.0, 1.2);
            c.line(x, base, x, base - 15, INK, 0.9);
            c.line(x + 1, base, x + 1, base - 9, INK, 0.9);
            c.polyline(&[(x, base - 7), (x - 4, base - 11), (x - 6, base - 15)], INK, 0.85);
            c.polyline(&[(x + 1, base - 10), (x + 5, base - 14), (x + 6, base - 18)], INK, 0.85);
            c.line(x - 4, base - 11, x - 2, base - 16, INK, 0.7);
            c.line(x, base - 15, x - 1, base - 19, INK, 0.75);
        }
        Shrub => {
            for _ in 0..c.range(2, 3) {
                let (x, y) = (c.range(8, s - 9) as f32, c.range(10, s - 8) as f32);
                let mut m = c.mask();
                m.disc(x, y, 3.5).disc(x - 3.0, y + 1.0, 2.6).disc(x + 3.0, y + 1.0, 2.6);
                c.shadow(x + 1.0, y + 3.0, 5.0, 1.4);
                c.symbol(&m, rgb(138, 152, 96), 0.45, |px, py| (px as f32 - x) + (py as f32 - y) > 0.5 && hatch_line(px, py, 3));
            }
        }
        BigBroadleaf => {
            let (x, y) = (sf * 0.5 + c.range(-1, 1) as f32, sf * 0.45);
            c.broadleaf(x, y, 11.0, rgb(132, 150, 92));
        }
        BigConifer => {
            let x = sf * 0.5 + c.range(-1, 0) as f32;
            c.conifer(x, sf - 2.0, sf - 3.0, 11.0, rgb(98, 120, 82));
        }
        BigJungle => {
            let (x, y) = (sf * 0.5, sf * 0.5);
            c.broadleaf(x - 4.0, y - 3.0, 8.0, rgb(108, 138, 82));
            c.broadleaf(x + 5.0, y + 1.0, 8.5, rgb(100, 132, 80));
        }
        Crops => {
            // Furrows of grain seen from above.
            for y in (5..s - 1).step_by(6) {
                c.line(1, y + 1, s - 2, y + 1, INK, 0.18);
                for x in (2..s - 1).step_by(3) {
                    let dy = c.range(0, 1);
                    c.ink(x, y - dy, 0.55);
                    c.ink(x, y - dy - 1, 0.55);
                    c.set(x, y - dy - 2, rgb(208, 176, 96));
                }
            }
        }
        Mushroom => {
            for _ in 0..c.range(2, 3) {
                let (x, y) = (c.range(8, s - 9) as f32, c.range(12, s - 6) as f32);
                c.shadow(x + 1.0, y + 3.0, 4.0, 1.2);
                let mut stem = c.mask();
                stem.rect(x as i32 - 1, y as i32 - 2, x as i32 + 1, y as i32 + 3);
                c.symbol(&stem, PAPER, 0.0, |_, _| false);
                let mut cap = c.mask();
                cap.poly(&[(x - 6.0, y - 1.0), (x - 4.5, y - 5.0), (x, y - 6.5), (x + 4.5, y - 5.0), (x + 6.0, y - 1.0)]);
                c.symbol(&cap, rgb(156, 104, 140), 0.45, |px, py| px as f32 > x && hatch_line(px, py, 2));
                c.set(x as i32 - 2, y as i32 - 4, PAPER);
                c.set(x as i32 + 1, y as i32 - 3, PAPER);
            }
        }
        Crystal => {
            for _ in 0..3 {
                let (x, base) = (c.range(6, s - 7) as f32, c.range(s - 9, s - 3) as f32);
                let h = c.range(8, 15) as f32;
                let lean = c.range(-2, 2) as f32;
                let mut m = c.mask();
                m.poly(&[(x - 2.5, base), (x - 2.5 + lean, base - h + 3.0), (x + lean, base - h), (x + 2.5 + lean, base - h + 3.0), (x + 2.5, base)]);
                c.shadow(x + 1.5, base, 3.5, 1.2);
                for (px, py) in m.cells() {
                    let right = px as f32 + 0.5 > x + lean * (base - py as f32) / h;
                    c.set(px, py, if right { rgb(132, 176, 182) } else { rgb(190, 222, 220) });
                }
                c.outline(&m, 0.92, i32::MAX);
                c.line((x + lean * 0.9) as i32, (base - h + 1.0) as i32, x as i32, base as i32 - 1, INK, 0.5);
            }
        }
        // --- scars --------------------------------------------------------------------------
        Bones => {
            // Skulls and scattered long bones.
            const BONE: Px = rgb(232, 224, 202);
            for (x, y) in c.scatter(11, 4, 0.7) {
                if c.chance(0.4) {
                    let mut skull = c.mask();
                    skull.disc(x as f32 + 0.5, y as f32, 2.4).rect(x - 1, y + 1, x + 1, y + 2);
                    c.symbol(&skull, BONE, 0.0, |_, _| false);
                    c.ink(x - 1, y, 0.9);
                    c.ink(x + 1, y, 0.9);
                } else {
                    let len = c.range(3, 5);
                    let (dx, dy) = if c.chance(0.5) { (1, 0) } else { (1, 1) };
                    let mut m = c.mask();
                    for k in 0..=len { m.rect(x + dx * k, y + dy * k, x + dx * k, y + dy * k); }
                    m.disc(x as f32 + 0.5, y as f32 + 0.5, 1.1).disc((x + dx * len) as f32 + 0.5, (y + dy * len) as f32 + 0.5, 1.1);
                    c.symbol(&m, BONE, 0.0, |_, _| false);
                }
            }
        }
        TitanBones => {
            // A colossal ribcage arching out of the ground, with the skull at one end.
            const BONE: Px = rgb(236, 228, 206);
            let base = s - 6;
            c.shadow(sf * 0.5, base as f32 + 1.0, 14.0, 2.0);
            let mut spine = c.mask();
            spine.rect(5, base - 1, s - 9, base);
            c.symbol(&spine, BONE, 0.0, |_, _| false);
            for k in 0..5 {
                let x = 7 + k * 4;
                let h = 13 - (k as i32 - 2).abs() * 2;
                let mut rib = c.mask();
                for t in 0..=h {
                    let bend = ((t as f32 / h as f32) * 2.2) as i32;
                    rib.rect(x + bend, base - t, x + bend + 1, base - t);
                }
                c.symbol(&rib, BONE, 0.45, |px, _| px % 2 == 1);
            }
            let mut skull = c.mask();
            skull.ellipse(sf - 7.0, base as f32 - 2.5, 4.5, 3.5).rect(s - 11, base - 1, s - 3, base);
            c.symbol(&skull, BONE, 0.45, |px, py| py > base - 2 && hatch_line(px, py, 2));
            c.ink(s - 8, base - 3, 0.95);
            c.ink(s - 7, base - 3, 0.95);
        }
        // --- animal signs (playable areas) -----------------------------------------------------
        Burrow => {
            // A dark hole with a crescent of thrown-out earth.
            let (x, y) = (sf * 0.5, sf * 0.55);
            let mut mound = c.mask();
            mound.ellipse(x + 1.0, y + 2.0, 9.0, 5.5);
            c.symbol(&mound, rgb(178, 146, 104), 0.35, |px, py| py as f32 > y + 2.0 && hatch_line(px, py, 2));
            let mut hole = c.mask();
            hole.ellipse(x, y, 5.0, 3.5);
            c.symbol(&hole, rgb(48, 36, 28), 0.0, |_, _| false);
        }
        Nest => {
            // A ring of twigs with three eggs.
            let (x, y) = (sf * 0.5, sf * 0.5);
            let mut ring = c.mask();
            ring.ellipse(x, y, 9.0, 7.0);
            let mut inner = c.mask();
            inner.ellipse(x, y, 5.5, 4.0);
            for (a, b) in ring.m.iter_mut().zip(&inner.m) { *a &= !*b; }
            c.symbol(&ring, rgb(150, 116, 78), 0.5, |px, py| (px + 2 * py).rem_euclid(3) == 0);
            c.fill_mask(&inner, rgb(96, 74, 52));
            for (dx, dy) in [(-2.0, -0.5), (1.5, -1.0), (0.0, 1.5)] {
                let mut egg = c.mask();
                egg.ellipse(x + dx, y + dy, 1.8, 1.4);
                c.symbol(&egg, rgb(226, 220, 200), 0.0, |_, _| false);
            }
        }
        Den => {
            // A low cave mouth under a rock, worn earth in front.
            let (x, base) = (sf * 0.5, sf * 0.7);
            let mut rock = c.mask();
            rock.ellipse(x, base - 4.0, 12.0, 8.0);
            let mut keep = c.mask();
            for (px, py) in rock.cells() { if (py as f32) < base { keep.put(px, py); } }
            c.shadow(x + 2.0, base + 1.0, 12.0, 2.5);
            c.symbol(&keep, STONE, 0.5, |px, py| px as f32 > x + 2.0 && hatch_line(px, py, 2));
            let mut mouth = c.mask();
            mouth.ellipse(x, base, 6.0, 4.5);
            let mut m2 = c.mask();
            for (px, py) in mouth.cells() { if (py as f32) < base { m2.put(px, py); } }
            c.symbol(&m2, rgb(40, 30, 24), 0.0, |_, _| false);
            let mut apron = c.mask();
            apron.ellipse(x, base + 2.5, 8.0, 2.5);
            for (px, py) in apron.cells() { if py as f32 >= base { c.blend(px, py, rgb(150, 120, 86), 0.8); } }
        }
        // --- relief --------------------------------------------------------------------------
        Hills => {
            // Ink humps with the shadow side hatched; the ground shows through.
            for (cx, base, rx, ry) in [(sf * 0.64, sf * 0.56, 9.0, 6.5), (sf * 0.38, sf * 0.84, 11.0, 8.0)] {
                let cx = cx + c.range(-2, 2) as f32;
                let mut m = c.mask();
                m.ellipse(cx, base, rx, ry);
                let mut hump = c.mask();
                for (x, y) in m.cells() { if (y as f32) < base { hump.put(x, y); } }
                c.clear_mask(&hump);
                for (x, y) in hump.cells() {
                    if (x as f32) < cx - rx * 0.1 { c.blend(x, y, PAPER, 0.3); }
                    else if hatch_line(x, y, 2) { c.ink(x, y, 0.42); }
                }
                c.outline(&hump, 0.9, base as i32 - 1);
            }
        }
        Mountain => {
            if c.chance(0.6) {
                let bx = if c.chance(0.5) { sf * 0.3 } else { sf * 0.72 };
                c.mountain(bx, sf * 0.22, sf * 0.66, 9.0, false);
            }
            let cx = sf * 0.5 + c.range(-2, 2) as f32;
            c.mountain(cx, sf * 0.12, sf - 3.0, 14.0, false);
        }
        SnowPeak => {
            if c.chance(0.6) {
                let bx = if c.chance(0.5) { sf * 0.3 } else { sf * 0.72 };
                c.mountain(bx, sf * 0.2, sf * 0.66, 9.0, true);
            }
            let cx = sf * 0.5 + c.range(-2, 2) as f32;
            c.mountain(cx, sf * 0.06, sf - 3.0, 14.5, true);
        }
        Volcano => {
            let (cx, top, base) = (sf * 0.5, sf * 0.34, sf - 3.0);
            let mut m = c.mask();
            m.poly(&[(cx - 14.0, base + 1.0), (cx - 4.5, top), (cx + 4.5, top), (cx + 14.0, base + 1.0)]);
            for (x, y) in m.cells() {
                let shaded = x as f32 + 0.5 > cx + 1.0;
                c.set(x, y, if shaded { rgb(150, 132, 116) } else { rgb(196, 180, 160) });
                if shaded && hatch_line(x, y, 2) { c.ink(x, y, 0.55); }
            }
            // Lava tongue down the lit face.
            c.polyline(&[(cx as i32 - 2, top as i32 + 1), (cx as i32 - 4, top as i32 + 6), (cx as i32 - 3, top as i32 + 11), (cx as i32 - 6, base as i32 - 2)], rgb(190, 92, 54), 0.95);
            c.outline(&m, 0.95, base as i32);
            let mut crater = c.mask();
            crater.ellipse(cx, top + 0.5, 4.5, 1.6);
            c.symbol(&crater, rgb(184, 86, 52), 0.0, |_, _| false);
            // Smoke: a column of ink curls.
            for (k, (dx, dy, r)) in [(0.5, -3.0, 1.8), (2.5, -6.5, 2.4), (1.0, -10.5, 2.9)].iter().enumerate() {
                let mut puff = c.mask();
                puff.disc(cx + dx, top + dy, *r);
                c.fill_mask(&puff, rgb(214, 206, 192));
                c.outline(&puff, 0.7 - k as f32 * 0.15, i32::MAX);
            }
        }
        Boulder => {
            let (x, y) = (sf * 0.5 + c.range(-3, 3) as f32, sf * 0.55 + c.range(-2, 2) as f32);
            let r = c.range(6, 8) as f32;
            c.boulder(x, y, r);
        }
        Ramp => {
            // An up-ramp: an arrow pointing to the higher side.
            let mut m = c.mask();
            m.poly(&[(16.0, 5.0), (27.0, 16.0), (20.5, 16.0), (20.5, 27.0), (11.5, 27.0), (11.5, 16.0), (5.0, 16.0)]);
            c.symbol(&m, PAPER, 0.45, |x, y| x >= 16 && hatch_line(x, y, 2));
        }
        // --- settlements ---------------------------------------------------------------------
        Village => {
            let spots = [(5, 13), (18, 10), (11, 23)];
            let mut v: Vec<(i32, i32)> = spots.iter().map(|&(x, y)| (x + c.range(-1, 1), y + c.range(-1, 1))).collect();
            if c.chance(0.5) { v.pop(); }
            v.sort_by_key(|p| p.1);
            for (x, y) in v { c.house(x, y, 8, 5, 4, ROOF); }
        }
        Town => {
            c.tower(14, 6, 18, 5, ROOF);
            let mut v = vec![(2, 12), (21, 11), (5, 24), (18, 24)];
            v.sort_by_key(|p| p.1);
            for (x, y) in v {
                let jx = c.range(0, 1);
                c.house(x + jx, y, 8, 5, 4, ROOF);
            }
        }
        City => {
            // Walled: a ring wall with a gate, houses and a spire inside.
            let mut ring = c.mask();
            ring.disc(sf * 0.5, sf * 0.52, 14.5);
            let mut inner = c.mask();
            inner.disc(sf * 0.5, sf * 0.52, 12.0);
            for (a, b) in ring.m.iter_mut().zip(&inner.m) { *a &= !*b; }
            c.symbol(&ring, STONE, 0.45, |x, y| x + y > s + 2 && hatch_line(x, y, 2));
            for k in 0..12 {
                let ang = k as f32 / 12.0 * std::f32::consts::TAU;
                c.ink((sf * 0.5 + ang.cos() * 13.2) as i32, (sf * 0.52 + ang.sin() * 13.2) as i32, 0.8);
            }
            c.tower(14, 7, 17, 4, ROOF);
            for (x, y) in [(6, 15), (20, 15), (9, 23), (17, 23)] { c.house(x, y, 7, 4, 3, ROOF); }
        }
        Castle => {
            // Keep between two towers, crenellated, with a gate and a pennant.
            let mut keep = c.mask();
            keep.rect(8, 12, 23, 27);
            for x in (8..24).step_by(3) { keep.rect(x, 10, x + 1, 11); }
            let mut towers = c.mask();
            towers.rect(3, 8, 9, 27).rect(22, 8, 28, 27);
            for x in [3, 6, 9, 22, 25, 28] { towers.rect(x, 6, x, 7); }
            c.shadow(18.0, 28.0, 14.0, 1.8);
            c.symbol(&keep, STONE, 0.45, |x, y| x >= 16 && hatch_line(x, y, 2));
            c.symbol(&towers, STONE, 0.5, |x, y| (x >= 7 && x <= 9 || x >= 26) && hatch_line(x, y, 2));
            let mut gate = c.mask();
            gate.rect(14, 20, 17, 27).disc(16.0, 20.0, 2.0);
            c.symbol(&gate, INK, 0.0, |_, _| false);
            c.line(16, 2, 16, 11, INK, 0.9);
            let mut flag = c.mask();
            flag.poly(&[(17.0, 2.0), (23.0, 3.5), (17.0, 5.5)]);
            c.symbol(&flag, rgb(172, 70, 58), 0.0, |_, _| false);
        }
        Ruins => {
            let mut m = c.mask();
            for (x, h) in [(4, c.range(8, 13)), (13, c.range(4, 8)), (22, c.range(10, 16))] {
                let base = s - 5;
                m.rect(x, base - h, x + 4, base);
                // Broken top.
                m.poly(&[(x as f32, (base - h) as f32 + 0.5), (x as f32 + 2.0, (base - h - 2) as f32), (x as f32 + 5.0, (base - h) as f32 + 0.5)]);
            }
            for _ in 0..4 {
                let (x, y) = (c.range(2, s - 6), c.range(s - 6, s - 3));
                m.rect(x, y, x + 2, y + 1);
            }
            c.shadow(sf * 0.5, sf - 4.0, 13.0, 1.6);
            c.symbol(&m, STONE, 0.45, |x, y| x % 9 >= 6 && hatch_line(x, y, 2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_atlas_round_trips_through_png() {
        let dir = std::env::temp_dir().join(format!("atlas_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("atlas.png");
        let a = Atlas::generated();
        a.save_png(&path).unwrap();
        let b = Atlas::load_png(&path).unwrap();
        assert_eq!(b.size, GENERATED_TILE_PX);
        for &k in &ALL_KINDS {
            for v in 0..VARIANTS {
                assert_eq!(a.tile_for(k, v, 16).0, b.tile_for(k, v, 16).0, "{k:?} variant {v}");
            }
        }
        // Sprites keep transparent backgrounds; ground tiles are fully opaque.
        assert!(a.tile_for(TileKind::Conifer, 0, 16).0.iter().any(|p| p[3] == 0));
        assert!(a.tile_for(TileKind::Grass, 0, 16).0.iter().all(|p| p[3] == 255));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cp437_sheet_is_tinted_per_kind() {
        // 16x16 grid of 8x8 glyphs: magenta background with a white square in every glyph.
        let mut img = RgbaImage::from_pixel(128, 128, Rgba([255, 0, 255, 255]));
        for gy in 0..16 {
            for gx in 0..16 {
                for y in 2..6 {
                    for x in 2..6 {
                        img.put_pixel(gx * 8 + x, gy * 8 + y, Rgba([255, 255, 255, 255]));
                    }
                }
            }
        }
        let a = Atlas::from_cp437(&img);
        assert_eq!(a.size, 8);
        let (_, fg, bg) = TileKind::Grass.cp437();
        let grass = a.tile_for(TileKind::Grass, 0, 8).0;
        assert_eq!(grass[0], [bg[0], bg[1], bg[2], 255], "background takes the kind's bg colour");
        assert_eq!(grass[3 * 8 + 3], [fg[0], fg[1], fg[2], 255], "glyph pixels take the fg colour");
        let tree = a.tile_for(TileKind::Conifer, 0, 8).0;
        assert_eq!(tree[0][3], 0, "sprite backgrounds are transparent");
    }
}
