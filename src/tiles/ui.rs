//! Shared drawing for the tile viewer's own screens (the watcher, the start screen): the ink
//! map's palette on a dark desk, parchment cards with a double ink rule, ink lettering, word
//! wrap. All drawing is into a `0x00RRGGBB` pixel buffer.

use super::text::{draw_ink, text_width};

// Palette: the ink map on a dark desk.
pub(crate) const DESK: u32 = 0x0026_201B;
pub(crate) const PAPER: u32 = 0x00EA_DEC4;
pub(crate) const PAPER_SHADE: u32 = 0x00DC_CCA8;
pub(crate) const INK: u32 = 0x0038_2A20;
pub(crate) const INK_FADED: u32 = 0x0080_6A52;
pub(crate) const RUBRIC: u32 = 0x009A_2A1E;
pub(crate) const GOLD: u32 = 0x00A8_7A26;
pub(crate) const SEA: u32 = 0x0030_5670;
pub(crate) const MOSS: u32 = 0x004E_6A30;
pub(crate) const VIOLET: u32 = 0x0064_3A6E;

#[derive(Clone, Copy, Default)]
pub(crate) struct Rect { pub x: usize, pub y: usize, pub w: usize, pub h: usize }

impl Rect {
    pub(crate) fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x as f32 && py >= self.y as f32 && px < (self.x + self.w) as f32 && py < (self.y + self.h) as f32
    }
}

pub(crate) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

pub(crate) fn hash(x: usize, y: usize) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

pub(crate) fn fill(buf: &mut [u32], w: usize, r: Rect, color: u32) {
    for y in r.y..r.y + r.h {
        buf[y * w + r.x..y * w + r.x + r.w].fill(color);
    }
}

pub(crate) fn blend_px(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64, color: u32, a: f32) {
    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
        let k = y as usize * w + x as usize;
        buf[k] = mix(buf[k], color, a);
    }
}

pub(crate) fn hline(buf: &mut [u32], w: usize, x0: usize, x1: usize, y: usize, color: u32) {
    buf[y * w + x0..y * w + x1].fill(color);
}

pub(crate) fn outline(buf: &mut [u32], w: usize, r: Rect, color: u32) {
    hline(buf, w, r.x, r.x + r.w, r.y, color);
    hline(buf, w, r.x, r.x + r.w, r.y + r.h - 1, color);
    for y in r.y..r.y + r.h {
        buf[y * w + r.x] = color;
        buf[y * w + r.x + r.w - 1] = color;
    }
}

/// A parchment card with a mottled wash and a double ink rule, like the map's own frame.
pub(crate) fn card(buf: &mut [u32], w: usize, r: Rect) {
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            let n = (hash(x / 3, y / 3) & 0xFF) as f32 / 255.0;
            let edge = ((x - r.x).min(r.x + r.w - 1 - x).min(y - r.y).min(r.y + r.h - 1 - y)) as f32;
            let foxing = (1.0 - edge / 18.0).max(0.0) * 0.18;
            buf[y * w + x] = mix(mix(PAPER, PAPER_SHADE, 0.35 * n), 0x00B8_9A6A, foxing);
        }
    }
    outline(buf, w, r, INK);
    outline(buf, w, Rect { x: r.x + 3, y: r.y + 3, w: r.w - 6, h: r.h - 6 }, INK_FADED);
}

/// Small-caps style heading with a rule under it.
pub(crate) fn heading(buf: &mut [u32], w: usize, h: usize, x: usize, y: i64, width: usize, text: &str) {
    super::text::draw_fell(buf, w, h, x as i64, y, text, RUBRIC, 1, true);
    let tx = x + super::text::fell_width(text, 1, true) + 6;
    if tx < x + width {
        let ry = (y + 4) as usize;
        if ry < h { hline(buf, w, tx, x + width, ry, INK_FADED); }
    }
}

/// Greedy word wrap to `max_chars` per line.
pub(crate) fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max_chars {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() { line.push(' '); }
        line.push_str(word);
    }
    if !line.is_empty() { lines.push(line); }
    lines
}

/// Fold accented letters to ASCII for the 8x8 font.
pub(crate) fn ascii(s: &str) -> String {
    s.chars().map(|c| match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'É' | 'È' => 'E',
        'Ó' | 'Ö' => 'O',
        '’' | '‘' => '\'',
        '—' | '–' => '-',
        c => c,
    }).collect()
}

pub(crate) fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars { s.to_string() } else { format!("{}.", s.chars().take(max_chars.saturating_sub(1)).collect::<String>()) }
}

