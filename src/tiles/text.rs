//! Bitmap text for map labels (8x8 public-domain font): ink lettering with a parchment halo
//! for legibility over any terrain.

use font8x8::legacy::BASIC_LEGACY;

/// Pixel width of `text` at `scale`.
pub fn text_width(text: &str, scale: usize) -> usize {
    text.chars().count() * 7 * scale + scale
}

fn glyph(c: char) -> [u8; 8] {
    let i = c as usize;
    if i < 128 { BASIC_LEGACY[i] } else { BASIC_LEGACY[b'?' as usize] }
}

/// Draw `text` in plain ink (no halo) with its top-left at (x, y); `bold` doubles each stroke
/// one pixel to the right. Same 7 px advance as `draw_text`.
pub fn draw_ink(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64, text: &str, color: u32, scale: usize, bold: bool) {
    let s = scale as i64;
    for (k, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        let ox = x + k as i64 * 7 * s;
        for (row, bits) in g.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) == 0 { continue; }
                for sy in 0..s {
                    for sx in 0..s + bold as i64 {
                        let (px, py) = (ox + col as i64 * s + sx, y + row as i64 * s + sy);
                        if px >= 0 && py >= 0 && px < w as i64 && py < h as i64 {
                            buf[py as usize * w + px as usize] = color;
                        }
                    }
                }
            }
        }
    }
}

/// Draw `text` with its top-left at (x, y): an outline in `shadow`, then the glyphs in `color`.
/// Glyphs are 7 px apart (tighter than the 8 px cell) for map-label density.
pub fn draw_text(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64, text: &str, color: u32, shadow: u32, scale: usize) {
    let s = scale as i64;
    let mut put = |px: i64, py: i64, c: u32| {
        if px >= 0 && py >= 0 && px < w as i64 && py < h as i64 {
            buf[py as usize * w + px as usize] = c;
        }
    };
    for pass in 0..2 {
        for (k, ch) in text.chars().enumerate() {
            let g = glyph(ch);
            let ox = x + k as i64 * 7 * s;
            for (row, bits) in g.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) == 0 { continue; }
                    for sy in 0..s {
                        for sx in 0..s {
                            let (px, py) = (ox + col as i64 * s + sx, y + row as i64 * s + sy);
                            if pass == 0 {
                                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (1, -1), (-1, 1)] {
                                    put(px + dx, py + dy, shadow);
                                }
                            } else {
                                put(px, py, color);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Parchment halo behind the ink lettering.
const LABEL_HALO: u32 = 0x00EE_E4CC;

/// How a label is set: the map's hierarchy of lettering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LabelStyle {
    /// Wide-spaced italic capitals.
    Ocean,
    /// Seas and gulfs: smaller spaced italic capitals.
    Sea,
    /// Continents and islands: spaced small capitals.
    Land,
    /// Mountain ranges: spaced small capitals in brown.
    Range,
    /// Forests, deserts, plains, tundra, ice fields, marshes: italic.
    Region,
    /// Rivers and lakes: italic, in the water's ink.
    Water,
    /// Peaks, landmarks.
    Feature,
    Capital,
    City,
    #[default]
    Town,
    Ruin,
}

impl LabelStyle {
    /// Face, size in px, tracking in px and whether to set in capitals, at `tile_px` zoom.
    fn set(self, tile_px: f32) -> (super::fonts::Face, f32, f32, bool) {
        use super::fonts::Face::*;
        let far = tile_px < 3.0;
        match self {
            LabelStyle::Ocean => (Italic, if far { 17.0 } else { 20.0 }, if far { 3.0 } else { 5.0 }, true),
            LabelStyle::Sea => (Italic, 14.0, 2.5, true),
            LabelStyle::Land => (SmallCaps, if far { 18.0 } else { 20.0 }, 3.0, false),
            LabelStyle::Range => (SmallCaps, 14.0, 2.0, false),
            LabelStyle::Region => (Italic, 14.0, 1.0, false),
            LabelStyle::Water => (Italic, 13.0, 0.5, false),
            LabelStyle::Feature => (Roman, 12.0, 0.0, false),
            LabelStyle::Capital => (SmallCaps, 15.0, 0.8, false),
            LabelStyle::City => (Roman, 14.0, 0.3, false),
            LabelStyle::Town => (Roman, 12.5, 0.0, false),
            LabelStyle::Ruin => (Italic, 12.0, 0.0, false),
        }
    }
}

/// A label to place on the map, in world-tile coordinates.
#[derive(Clone, Debug)]
pub struct Label {
    pub x: f32,
    pub y: f32,
    pub text: String,
    /// Higher = placed first and shown at lower zoom.
    pub rank: u32,
    /// Minimum tile size (px) at which the label appears.
    pub min_tile_px: f32,
    pub color: u32,
    pub style: LabelStyle,
}

/// Greedy, collision-free label placement: highest rank first, skipping any label that would
/// overlap one already placed, run off the edge, cover an `avoid` rectangle (the minimap) or
/// repeat a name already on screen. `to_screen` maps world tile coordinates to screen pixels.
pub fn place_labels(labels: &[Label], tile_px: f32, w: usize, h: usize, buf: &mut [u32], avoid: &[(i64, i64, i64, i64)], to_screen: impl Fn(f32, f32) -> (f32, f32)) {
    place_labels_scaled(labels, tile_px, 1.0, w, h, buf, avoid, to_screen);
}

/// `place_labels` with the lettering `font_scale` times larger (posters).
pub fn place_labels_scaled(labels: &[Label], tile_px: f32, font_scale: f32, w: usize, h: usize, buf: &mut [u32], avoid: &[(i64, i64, i64, i64)], to_screen: impl Fn(f32, f32) -> (f32, f32)) {
    let mut placed: Vec<(i64, i64, i64, i64)> = Vec::new();
    let mut names: Vec<&str> = Vec::new();
    for l in labels {
        if tile_px < l.min_tile_px { continue; }
        if names.contains(&l.text.as_str()) { continue; }
        let (sx, sy) = to_screen(l.x, l.y);
        let (face, px, tracking, caps) = l.style.set(tile_px / font_scale);
        let (px, tracking) = (px * font_scale, tracking * font_scale);
        let text = if caps { l.text.to_uppercase() } else { l.text.clone() };
        let tw = super::fonts::width(&text, face, px, tracking).ceil() as i64;
        let (asc, desc) = super::fonts::line_metrics(face, px);
        let th = (asc + desc).ceil() as i64;
        let (x0, y0) = (sx as i64 - tw / 2, sy as i64 - th / 2);
        // Whole or not at all: a label cut by the window edge reads as a mistake.
        let edge = 3;
        if x0 < edge || y0 < edge || x0 + tw > w as i64 - edge || y0 + th > h as i64 - edge { continue; }
        let pad = (5.0 * font_scale) as i64;
        let hits = |r: &(i64, i64, i64, i64)| x0 - pad < r.2 && x0 + tw + pad > r.0 && y0 - pad < r.3 && y0 + th + pad > r.1;
        if placed.iter().any(hits) || avoid.iter().any(hits) { continue; }
        // Names on the water get a pale sea halo, so they read on dark water without a parchment smear.
        let halo = if matches!(l.style, LabelStyle::Ocean | LabelStyle::Sea) { 0x00C8_D8DC } else { LABEL_HALO };
        super::fonts::draw(buf, w, h, x0 as f32, y0 as f32, &text, face, px, tracking, l.color, Some(halo));
        placed.push((x0, y0, x0 + tw, y0 + th));
        names.push(&l.text);
    }
}
