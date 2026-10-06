//! A map hand: IM Fell English (roman, italic, small capitals) and, for the player's own notes,
//! Indie Flower (all SIL Open Font License, in `assets/fonts/`) rasterised with `fontdue`, for the map's labels. Glyphs are cached per face,
//! size and character, so placing a few hundred labels a frame costs little.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face { Roman, Italic, SmallCaps, Hand }

fn fonts() -> &'static [fontdue::Font; 4] {
    static FONTS: OnceLock<[fontdue::Font; 4]> = OnceLock::new();
    FONTS.get_or_init(|| {
        let load = |b: &[u8]| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).expect("embedded font");
        [
            load(include_bytes!("../../assets/fonts/IMFeENrm28P.ttf")),
            load(include_bytes!("../../assets/fonts/IMFeENit28P.ttf")),
            load(include_bytes!("../../assets/fonts/IMFeENsc28P.ttf")),
            // The player's own hand (marginalia): Indie Flower, OFL.
            load(include_bytes!("../../assets/fonts/IndieFlower-Regular.ttf")),
        ]
    })
}

type Glyph = (fontdue::Metrics, Vec<u8>);

fn glyph(face: Face, px: f32, ch: char) -> std::sync::Arc<Glyph> {
    static CACHE: OnceLock<Mutex<HashMap<(Face, u32, char), std::sync::Arc<Glyph>>>> = OnceLock::new();
    let key = (face, (px * 4.0) as u32, ch);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(g) = cache.lock().unwrap().get(&key) { return g.clone(); }
    let g = std::sync::Arc::new(fonts()[face as usize].rasterize(ch, px));
    cache.lock().unwrap().insert(key, g.clone());
    g
}

/// Width in pixels of `text` at `px`, with `tracking` extra pixels after each letter.
pub fn width(text: &str, face: Face, px: f32, tracking: f32) -> f32 {
    let n = text.chars().count();
    text.chars().map(|c| glyph(face, px, c).0.advance_width).sum::<f32>() + tracking * n.saturating_sub(1) as f32
}

/// Height of a line (ascent above the baseline, descent below) at `px`.
pub fn line_metrics(face: Face, px: f32) -> (f32, f32) {
    match fonts()[face as usize].horizontal_line_metrics(px) {
        Some(m) => (m.ascent, -m.descent),
        None => (px * 0.8, px * 0.2),
    }
}

fn blend(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64, c: u32, a: f32) {
    if a <= 0.0 || x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { return; }
    let i = y as usize * w + x as usize;
    let p = buf[i];
    let ch = |s: u32| { let (q, r) = ((p >> s & 255) as f32, (c >> s & 255) as f32); ((q + (r - q) * a.min(1.0)) as u32) << s };
    buf[i] = ch(16) | ch(8) | ch(0);
}

/// Draw `text` with its top-left at (x, y): a soft parchment halo first, then the ink.
pub fn draw(buf: &mut [u32], w: usize, h: usize, x: f32, y: f32, text: &str, face: Face, px: f32, tracking: f32, color: u32, halo: Option<u32>) {
    let (ascent, _) = line_metrics(face, px);
    let base = y + ascent;
    let glyphs: Vec<(f32, std::sync::Arc<Glyph>)> = {
        let mut pen = x;
        text.chars().map(|c| { let g = glyph(face, px, c); let at = pen; pen += g.0.advance_width + tracking; (at, g) }).collect()
    };
    for pass in 0..2 {
        if pass == 0 && halo.is_none() { continue; }
        for (at, g) in &glyphs {
            let (m, bm) = (&g.0, &g.1);
            let gx = (at + m.xmin as f32).round() as i64;
            let gy = (base - m.height as f32 - m.ymin as f32).round() as i64;
            for row in 0..m.height {
                for col in 0..m.width {
                    let cov = bm[row * m.width + col] as f32 / 255.0;
                    if cov <= 0.02 { continue; }
                    let (px_, py_) = (gx + col as i64, gy + row as i64);
                    if pass == 0 {
                        let hc = halo.unwrap();
                        for (dx, dy, k) in [(-1, 0, 0.7), (1, 0, 0.7), (0, -1, 0.7), (0, 1, 0.7), (-2, 0, 0.35), (2, 0, 0.35), (0, -2, 0.35), (0, 2, 0.35), (-1, -1, 0.5), (1, 1, 0.5), (-1, 1, 0.5), (1, -1, 0.5)] {
                            blend(buf, w, h, px_ + dx, py_ + dy, hc, cov * k);
                        }
                    } else {
                        blend(buf, w, h, px_, py_, color, cov);
                    }
                }
            }
        }
    }
}

/// Greedy word wrap to a pixel width in `face` at `px`.
pub fn wrap(text: &str, face: Face, px: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let trial = if line.is_empty() { word.to_string() } else { format!("{} {}", line, word) };
        if !line.is_empty() && width(&trial, face, px, 0.0) > max_w {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = trial;
        }
    }
    if !line.is_empty() { lines.push(line); }
    lines
}

