//! Tile atlas: the pixel-art tiles the graphical viewer draws the world with.
//!
//! The default atlas is drawn procedurally (16x16 pixels, 4 variants per kind). It can be
//! exported as a PNG (`--export-tileset`), edited, and loaded back (`--tileset`). A Dwarf
//! Fortress style CP437 sheet (16x16 glyph grid, magenta or transparent background) can be
//! loaded instead: each tile kind maps to a glyph tinted with that kind's colours, as in DF.

use std::error::Error;
use std::path::Path;

use image::{Rgba, RgbaImage};

/// Variants per tile kind (chosen per world tile by hash so the map doesn't repeat).
pub const VARIANTS: usize = 4;
/// Size of the generated tiles in pixels.
pub const GENERATED_TILE_PX: usize = 16;

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
}

pub const ALL_KINDS: [TileKind; 42] = [
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
];

impl TileKind {
    pub fn index(self) -> usize {
        ALL_KINDS.iter().position(|&k| k == self).unwrap()
    }

    pub fn is_sprite(self) -> bool {
        self.index() >= TileKind::Deciduous.index()
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
// Procedural pixel art
// ---------------------------------------------------------------------------------------------

struct Canvas {
    s: usize,
    px: Vec<Px>,
    rng: u64,
}

const fn rgb(r: u8, g: u8, b: u8) -> Px { [r, g, b, 255] }

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
    fn chance(&mut self, p: f32) -> bool { (self.rand() % 1000) as f32 / 1000.0 < p }
    fn set(&mut self, x: i32, y: i32, c: Px) {
        if x >= 0 && y >= 0 && (x as usize) < self.s && (y as usize) < self.s {
            self.px[y as usize * self.s + x as usize] = c;
        }
    }
    fn fill(&mut self, c: Px) { self.px.iter_mut().for_each(|p| *p = c); }
    /// Scatter single-pixel specks of `c` over roughly `density` of the tile.
    fn specks(&mut self, c: Px, density: f32) {
        let n = (self.s * self.s) as f32 * density;
        for _ in 0..n as usize {
            let (x, y) = (self.range(0, self.s as i32 - 1), self.range(0, self.s as i32 - 1));
            self.set(x, y, c);
        }
    }
    fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Px) {
        for x in x0..=x1 { self.set(x, y, c); }
    }
    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Px) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, c);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }
    /// Shaded disc: highlight toward the top-left, shadow toward the bottom-right.
    fn ball(&mut self, cx: i32, cy: i32, r: i32, dark: Px, mid: Px, light: Px) {
        for y in -r..=r {
            for x in -r..=r {
                let d2 = x * x + y * y;
                if d2 > r * r + r / 2 { continue; }
                let lit = -(x + y) as f32 / (2.0 * r as f32);
                let c = if d2 > (r - 1) * (r - 1) + 1 && lit < 0.2 { dark } else if lit > 0.25 { light } else { mid };
                self.set(cx + x, cy + y, c);
            }
        }
    }
    /// Wave dashes for water.
    fn waves(&mut self, c: Px, count: usize) {
        for _ in 0..count {
            let (x, y) = (self.range(0, self.s as i32 - 4), self.range(1, self.s as i32 - 2));
            let len = self.range(2, 4);
            self.hline(x, x + len, y, c);
            self.set(x + len + 1, y - 1, c);
        }
    }
    /// Grass tufts: little "v" shapes.
    fn tufts(&mut self, c: Px, count: usize) {
        for _ in 0..count {
            let (x, y) = (self.range(1, self.s as i32 - 2), self.range(2, self.s as i32 - 1));
            self.set(x, y, c);
            self.set(x - 1, y - 1, c);
            self.set(x + 1, y - 1, c);
        }
    }
    /// Filled mountain triangle with lit left face and shaded right face.
    fn peak(&mut self, cx: i32, top: i32, base: i32, half_w: i32, light: Px, dark: Px, outline: Px, cap: Option<(Px, f32)>) {
        let h = (base - top).max(1);
        for y in top..=base {
            let t = (y - top) as f32 / h as f32;
            let hw = (t * half_w as f32).round() as i32;
            for x in cx - hw..=cx + hw {
                let mut c = if x < cx || (x == cx && y % 2 == 0) { light } else { dark };
                if let Some((snow, frac)) = cap {
                    let edge = frac + 0.08 * ((x * 7 + y * 3) % 3) as f32;
                    if t < edge { c = if x <= cx { snow } else { [snow[0] - 40, snow[1] - 35, snow[2] - 25, 255] }; }
                }
                if x == cx - hw || x == cx + hw { c = outline; }
                self.set(x, y, c);
            }
        }
    }
}

fn paint(kind: TileKind, c: &mut Canvas) {
    use TileKind::*;
    let s = c.s as i32;
    match kind {
        DeepOcean => { c.fill(rgb(16, 36, 82)); c.waves(rgb(28, 58, 112), 3); }
        Ocean => { c.fill(rgb(28, 70, 140)); c.waves(rgb(60, 110, 180), 4); }
        Shallows => { c.fill(rgb(52, 122, 172)); c.waves(rgb(115, 175, 215), 5); }
        Lake => { c.fill(rgb(44, 102, 160)); c.waves(rgb(90, 150, 205), 3); }
        SeaIce => {
            c.fill(rgb(205, 228, 238));
            c.specks(rgb(235, 248, 255), 0.08);
            let (x0, y0) = (c.range(0, s - 1), c.range(0, 4));
            let (x1, y1) = (c.range(0, s - 1), c.range(s - 5, s - 1));
            c.line(x0, y0, x1, y1, rgb(150, 188, 208));
        }
        Grass => {
            c.fill(rgb(84, 140, 60));
            c.specks(rgb(66, 118, 48), 0.12);
            c.specks(rgb(112, 166, 78), 0.06);
            c.tufts(rgb(58, 104, 42), 3);
        }
        Steppe => {
            c.fill(rgb(152, 160, 88));
            c.specks(rgb(132, 140, 70), 0.12);
            c.specks(rgb(176, 180, 110), 0.05);
            c.tufts(rgb(118, 126, 60), 3);
        }
        Savanna => {
            c.fill(rgb(192, 166, 98));
            c.specks(rgb(170, 146, 80), 0.12);
            c.tufts(rgb(150, 128, 62), 4);
        }
        Sand => {
            c.fill(rgb(222, 196, 132));
            c.specks(rgb(206, 180, 118), 0.08);
            for _ in 0..2 {
                let (x, y) = (c.range(0, s - 8), c.range(3, s - 3));
                for i in 0..7 {
                    let dy = if i == 0 || i == 6 { 1 } else { 0 };
                    c.set(x + i, y + dy, rgb(242, 220, 160));
                    c.set(x + i, y + dy + 1, rgb(196, 166, 104));
                }
            }
        }
        Salt => {
            c.fill(rgb(232, 230, 222));
            for _ in 0..2 {
                let (x0, y0) = (c.range(0, s - 1), c.range(0, s - 1));
                let (x1, y1) = (c.range(0, s - 1), c.range(0, s - 1));
                c.line(x0, y0, x1, y1, rgb(196, 190, 178));
            }
        }
        Tundra => {
            c.fill(rgb(128, 138, 114));
            c.specks(rgb(104, 112, 92), 0.12);
            c.specks(rgb(160, 164, 146), 0.08);
            c.specks(rgb(150, 120, 90), 0.02);
        }
        Snow => {
            c.fill(rgb(234, 240, 248));
            c.specks(rgb(210, 220, 234), 0.10);
            c.specks(rgb(255, 255, 255), 0.04);
        }
        Swamp => {
            c.fill(rgb(70, 88, 50));
            c.specks(rgb(56, 72, 40), 0.12);
            for _ in 0..2 {
                let (x, y) = (c.range(2, s - 5), c.range(2, s - 4));
                for dx in 0..4 { c.set(x + dx, y, rgb(52, 92, 92)); c.set(x + dx, y + 1, rgb(46, 82, 84)); }
            }
            for _ in 0..3 {
                let (x, y) = (c.range(1, s - 2), c.range(4, s - 1));
                c.line(x, y, x, y - 3, rgb(96, 110, 52));
            }
        }
        JungleFloor => {
            c.fill(rgb(40, 92, 40));
            c.specks(rgb(30, 74, 30), 0.15);
            c.specks(rgb(60, 116, 52), 0.06);
        }
        Rock => {
            c.fill(rgb(124, 119, 114));
            c.specks(rgb(150, 146, 140), 0.08);
            for _ in 0..2 {
                let (x, y) = (c.range(0, s - 4), c.range(0, s - 4));
                let (dx, dy) = (c.range(1, 3), c.range(1, 3));
                c.line(x, y, x + dx, y + dy, rgb(88, 84, 80));
            }
        }
        Ash => {
            c.fill(rgb(70, 66, 64));
            c.specks(rgb(48, 45, 44), 0.12);
            c.specks(rgb(98, 92, 88), 0.06);
        }
        Lava => {
            c.fill(rgb(92, 26, 12));
            for _ in 0..3 {
                let (x0, y0) = (c.range(0, s - 1), c.range(0, s - 1));
                let (x1, y1) = (c.range(0, s - 1), c.range(0, s - 1));
                c.line(x0, y0, x1, y1, rgb(255, 140, 30));
            }
            c.specks(rgb(255, 214, 90), 0.03);
        }
        Beach => {
            c.fill(rgb(232, 212, 160));
            c.specks(rgb(214, 192, 140), 0.10);
            c.specks(rgb(246, 232, 196), 0.05);
        }
        Dirt => {
            c.fill(rgb(128, 92, 58));
            c.specks(rgb(108, 76, 46), 0.14);
            c.specks(rgb(150, 112, 74), 0.06);
            c.specks(rgb(96, 88, 80), 0.02);
        }
        Gravel => {
            c.fill(rgb(150, 146, 138));
            for _ in 0..14 {
                let (x, y) = (c.range(0, s - 2), c.range(0, s - 2));
                let col = if c.chance(0.5) { rgb(180, 176, 168) } else { rgb(112, 108, 102) };
                c.set(x, y, col);
                c.set(x + 1, y, col);
            }
        }
        StoneFloor => {
            c.fill(rgb(150, 148, 144));
            c.specks(rgb(132, 130, 126), 0.10);
            c.specks(rgb(166, 164, 160), 0.05);
        }
        StoneWall => {
            // Rough rock face seen from above: blocky facets with dark cracks.
            c.fill(rgb(120, 116, 110));
            for y in 0..s {
                for x in 0..s {
                    let facet = ((x / 4 + y / 5 * 3) % 3) as u8;
                    let v = [128u8, 112, 98][facet as usize];
                    c.set(x, y, rgb(v, v - 3, v - 8));
                }
            }
            for _ in 0..3 {
                let (x0, y0) = (c.range(0, s - 1), c.range(0, s - 1));
                let (x1, y1) = (c.range(0, s - 1), c.range(0, s - 1));
                c.line(x0, y0, x1, y1, rgb(64, 60, 56));
            }
            c.hline(0, s - 1, 0, rgb(150, 146, 140));
        }
        SoilWall => {
            c.fill(rgb(104, 76, 48));
            c.specks(rgb(86, 62, 38), 0.18);
            c.specks(rgb(124, 92, 60), 0.08);
            c.hline(0, s - 1, 0, rgb(130, 98, 64));
        }
        Deciduous => {
            let n = c.range(1, 2);
            for _ in 0..n {
                let (x, y) = (c.range(4, s - 5), c.range(4, s - 7));
                c.line(x, y + 3, x, y + 6, rgb(92, 62, 36));
                let r = c.range(3, 4);
                c.ball(x, y, r, rgb(34, 80, 30), rgb(58, 122, 44), rgb(96, 164, 64));
            }
        }
        Conifer => {
            let n = c.range(2, 3);
            for i in 0..n {
                let x = 3 + i * (s - 6) / n.max(1) + c.range(0, 2);
                let base = c.range(s - 4, s - 2);
                let top = base - c.range(8, 11);
                c.peak(x, top, base - 1, 3, rgb(52, 108, 60), rgb(26, 70, 40), rgb(18, 50, 30), None);
                c.set(x, base, rgb(80, 56, 34));
            }
        }
        Jungle => {
            for _ in 0..3 {
                let (x, y) = (c.range(3, s - 4), c.range(3, s - 4));
                let r = c.range(3, 5);
                c.ball(x, y, r, rgb(22, 78, 30), rgb(40, 128, 46), rgb(80, 172, 62));
            }
        }
        Palm => {
            let (x, base) = (c.range(6, s - 7), s - 2);
            c.line(x, base, x + 2, base - 9, rgb(134, 98, 56));
            let (tx, ty) = (x + 2, base - 9);
            for (dx, dy) in [(-5, 1), (5, 1), (-4, -2), (4, -2), (0, -4)] {
                c.line(tx, ty, tx + dx, ty + dy, rgb(64, 150, 62));
            }
        }
        Acacia => {
            let (x, base) = (c.range(5, s - 6), s - 3);
            c.line(x, base, x, base - 6, rgb(110, 80, 50));
            for dy in 0..3 {
                let hw = 5 - dy;
                c.hline(x - hw, x + hw, base - 7 - dy, if dy == 0 { rgb(84, 104, 40) } else { rgb(116, 138, 54) });
            }
        }
        DeadTree => {
            let (x, base) = (c.range(5, s - 6), s - 2);
            let col = rgb(104, 84, 64);
            c.line(x, base, x, base - 9, col);
            c.line(x, base - 5, x - 3, base - 8, col);
            c.line(x, base - 6, x + 3, base - 10, col);
        }
        Hills => {
            // Two overlapping shaded domes: lit from the top-left, dark outline.
            let back = (c.range(9, 11), s - 6, 5, 4);
            let front = (c.range(5, 7), s - 2, 6, 6);
            for (cx, base, rx, ry) in [back, front] {
                for dy in 0..=ry {
                    let t = dy as f32 / ry as f32;
                    let hw = (rx as f32 * (1.0 - (1.0 - t) * (1.0 - t)).sqrt()).round() as i32;
                    let y = base - ry + dy;
                    for x in cx - hw..=cx + hw {
                        let lit = (x - cx) as f32 / rx.max(1) as f32 + (1.0 - t) * 0.6;
                        let mut col = if lit < -0.1 { rgb(150, 166, 96) } else if lit < 0.5 { rgb(118, 134, 74) } else { rgb(88, 100, 56) };
                        if dy == 0 || x == cx - hw || x == cx + hw { col = rgb(62, 72, 40); }
                        c.set(x, y, col);
                    }
                }
            }
        }
        Mountain => {
            let cx = c.range(6, s - 7);
            c.peak(cx, 2, s - 2, 7, rgb(172, 166, 160), rgb(110, 104, 99), rgb(62, 58, 55), None);
        }
        SnowPeak => {
            let cx = c.range(6, s - 7);
            c.peak(cx, 1, s - 2, 7, rgb(160, 156, 152), rgb(104, 100, 96), rgb(60, 58, 56), Some((rgb(244, 248, 252), 0.45)));
        }
        Volcano => {
            let cx = s / 2;
            c.peak(cx, 4, s - 2, 7, rgb(96, 84, 78), rgb(58, 50, 46), rgb(36, 30, 28), None);
            c.hline(cx - 2, cx + 2, 4, rgb(255, 120, 30));
            c.hline(cx - 1, cx + 1, 5, rgb(255, 200, 80));
            c.line(cx + 1, 6, cx + 3, s - 4, rgb(230, 80, 20));
            c.set(cx - 1, 2, rgb(150, 150, 150));
            c.set(cx, 1, rgb(170, 170, 170));
        }
        Mushroom => {
            for _ in 0..2 {
                let (x, y) = (c.range(4, s - 5), c.range(6, s - 5));
                c.line(x, y, x, y + 4, rgb(224, 214, 196));
                for dx in -3..=3 {
                    let h = 2 - (dx as i32).abs() / 2;
                    for dy in 0..=h { c.set(x + dx, y - dy, rgb(176, 52, 164)); }
                }
                c.set(x - 1, y - 1, rgb(240, 200, 240));
                c.set(x + 2, y, rgb(240, 200, 240));
            }
        }
        Crystal => {
            for _ in 0..3 {
                let (x, base) = (c.range(3, s - 4), c.range(s - 4, s - 2));
                let h = c.range(4, 8);
                c.line(x, base, x, base - h, rgb(120, 220, 245));
                c.line(x + 1, base, x + 1, base - h + 1, rgb(70, 160, 210));
                c.set(x, base - h, rgb(240, 255, 255));
            }
        }
        Shrub => {
            let (x, y) = (c.range(5, s - 6), c.range(6, s - 5));
            c.ball(x, y, 3, rgb(40, 86, 34), rgb(66, 124, 48), rgb(98, 160, 66));
            c.ball(x + 3, y + 1, 2, rgb(40, 86, 34), rgb(66, 124, 48), rgb(98, 160, 66));
        }
        Boulder => {
            let (x, y) = (c.range(5, s - 6), c.range(6, s - 5));
            c.ball(x, y, 4, rgb(84, 80, 76), rgb(132, 128, 122), rgb(176, 172, 166));
        }
        Ramp => {
            // An up-ramp: a light chevron pointing to the higher side.
            let col = rgb(236, 232, 214);
            let shadow = rgb(70, 66, 60);
            for i in 0..5 {
                c.hline(s / 2 - i, s / 2 + i, 4 + i, col);
                c.hline(s / 2 - i, s / 2 + i, 5 + i, shadow);
            }
            for i in 0..5 {
                c.hline(s / 2 - i, s / 2 + i, 4 + i, col);
            }
        }
        BigBroadleaf => {
            // One tree filling the tile: a round crown with a bit of trunk.
            let (x, y) = (s / 2 + c.range(-1, 0), s / 2 - 1);
            c.line(x, y + 5, x, s - 1, rgb(92, 62, 36));
            c.ball(x, y, 7, rgb(32, 76, 28), rgb(56, 118, 42), rgb(94, 160, 62));
        }
        BigConifer => {
            let x = s / 2 + c.range(-1, 0);
            c.peak(x, 0, s - 2, 7, rgb(54, 110, 62), rgb(26, 70, 40), rgb(16, 46, 28), None);
            c.line(x, s - 2, x, s - 1, rgb(80, 56, 34));
        }
        BigJungle => {
            let (x, y) = (s / 2 + c.range(-1, 1), s / 2 + c.range(-1, 1));
            c.ball(x - 2, y + 1, 5, rgb(22, 78, 30), rgb(40, 128, 46), rgb(80, 172, 62));
            c.ball(x + 2, y - 1, 5, rgb(22, 78, 30), rgb(44, 134, 50), rgb(88, 180, 66));
        }
        Ruins => {
            let stone = rgb(150, 144, 134);
            let dark = rgb(98, 94, 88);
            for i in 0..3 {
                let x = 2 + i * 5;
                let h = c.range(3, 9);
                for y in 0..h {
                    c.set(x, s - 3 - y, stone);
                    c.set(x + 1, s - 3 - y, dark);
                }
            }
            c.hline(1, s - 3, s - 2, dark);
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
