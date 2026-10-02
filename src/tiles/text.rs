//! Bitmap text for map labels (8x8 public-domain font), with a dark outline for legibility
//! over any terrain.

use font8x8::legacy::BASIC_LEGACY;

/// Pixel width of `text` at `scale`.
pub fn text_width(text: &str, scale: usize) -> usize {
    text.chars().count() * 7 * scale + scale
}

fn glyph(c: char) -> [u8; 8] {
    let i = c as usize;
    if i < 128 { BASIC_LEGACY[i] } else { BASIC_LEGACY[b'?' as usize] }
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
}

/// Greedy, collision-free label placement: highest rank first, skipping any label that would
/// overlap one already placed. `to_screen` maps world tile coordinates to screen pixels.
pub fn place_labels(labels: &[Label], tile_px: f32, w: usize, h: usize, buf: &mut [u32], to_screen: impl Fn(f32, f32) -> (f32, f32)) {
    let mut placed: Vec<(i64, i64, i64, i64)> = Vec::new();
    for l in labels {
        if tile_px < l.min_tile_px { continue; }
        let (sx, sy) = to_screen(l.x, l.y);
        let scale = if l.rank >= 95 && tile_px < 3.0 { 2 } else { 1 };
        let tw = text_width(&l.text, scale) as i64;
        let th = (8 * scale) as i64;
        let (x0, y0) = (sx as i64 - tw / 2, sy as i64 - th / 2);
        if x0 + tw < 0 || y0 + th < 0 || x0 >= w as i64 || y0 >= h as i64 { continue; }
        let pad = 4;
        let hit = placed.iter().any(|&(a, b, c, d)| x0 - pad < c && x0 + tw + pad > a && y0 - pad < d && y0 + th + pad > b);
        if hit { continue; }
        draw_text(buf, w, h, x0, y0, &l.text, l.color, 0x0010_1010, scale);
        placed.push((x0, y0, x0 + tw, y0 + th));
    }
}
