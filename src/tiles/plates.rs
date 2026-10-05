//! Plates: screenshots that explain themselves.
//!
//! P in the viewer used to save the raw frame to `view_<seed>.png`, overwriting the last. A plate
//! is the map without the interface, inside a ruled border, with a cartouche (the world's name,
//! the year and season, the seed), a caption (the world's one sentence, `lore::claims`) and a
//! legend of the realms in view, numbered in `plates/`. The PNG's text chunks carry the command
//! line that generates the world and the view, so `--plate FILE` opens that world at that view.

use std::sync::OnceLock;

use super::text::{draw_ink, text_width};
use super::ui::{self, Rect, INK, INK_FADED, PAPER, RUBRIC};

/// The arguments that generate this world (set by `main`), written into every plate.
static WORLD_ARGS: OnceLock<String> = OnceLock::new();

pub fn set_world_args(args: String) { let _ = WORLD_ARGS.set(args); }

/// The world part of the world code ("SEED.WxH.STYLE.PEOPLES.YEARS"); the site is added per
/// embark ("@X,Y").
static WORLD_CODE: OnceLock<String> = OnceLock::new();

pub fn set_world_code(code: String) { let _ = WORLD_CODE.set(code); }

/// The full world code for an embark at `site`.
pub fn world_code(site: (usize, usize)) -> Option<String> { WORLD_CODE.get().map(|c| format!("{}@{},{}", c, site.0, site.1)) }

/// What a plate says about itself.
pub struct PlateInfo {
    pub world_name: String,
    pub year: Option<u32>,
    pub season: String,
    pub seed: u64,
    pub caption: String,
    /// Realms in view, most land first: (name, arms).
    pub realms: Vec<(String, super::heraldry::Arms)>,
}

const MARGIN: usize = 18;

/// Frame `buf` (the map, already drawn) as a plate: a parchment margin with a double rule, the
/// cartouche top left, the caption along the bottom, the realm legend bottom right.
pub fn decorate(buf: &mut [u32], w: usize, h: usize, info: &PlateInfo) {
    if w < 300 || h < 200 { return; }
    // The margin: parchment, then a heavy and a fine rule.
    for y in 0..h {
        for x in 0..w {
            if x < MARGIN || y < MARGIN || x >= w - MARGIN || y >= h - MARGIN {
                let n = (ui::hash(x / 3, y / 3) & 0xFF) as f32 / 255.0;
                buf[y * w + x] = ui::mix(PAPER, 0x00D8_C8A0, 0.4 * n);
            }
        }
    }
    let inner = Rect { x: MARGIN, y: MARGIN, w: w - 2 * MARGIN, h: h - 2 * MARGIN };
    ui::outline(buf, w, inner, INK);
    ui::outline(buf, w, Rect { x: inner.x - 1, y: inner.y - 1, w: inner.w + 2, h: inner.h + 2 }, INK);
    ui::outline(buf, w, Rect { x: inner.x - 6, y: inner.y - 6, w: inner.w + 12, h: inner.h + 12 }, INK_FADED);

    // The cartouche.
    let title = format!("The Lands of {}", info.world_name);
    let when = match info.year { Some(y) => format!("{}, year {}", info.season, y), None => info.season.clone() };
    let code = format!("seed {}", info.seed);
    let cw = [text_width(&title, 2), text_width(&when, 1), text_width(&code, 1)].into_iter().max().unwrap_or(0) + 28;
    let card = Rect { x: inner.x + 14, y: inner.y + 14, w: cw.min(inner.w - 28), h: 62 };
    ui::card(buf, w, card);
    draw_ink(buf, w, h, (card.x + 14) as i64, (card.y + 10) as i64, &title, RUBRIC, 2, true);
    draw_ink(buf, w, h, (card.x + 14) as i64, (card.y + 32) as i64, &when, INK, 1, false);
    draw_ink(buf, w, h, (card.x + 14) as i64, (card.y + 45) as i64, &code, INK_FADED, 1, false);

    // The caption, wrapped, on a band along the bottom.
    let max_chars = (inner.w - 40) / 7;
    let lines = ui::wrap(&ui::ascii(&info.caption), max_chars.max(20));
    let lines: Vec<String> = lines.into_iter().take(3).collect();
    if !lines.is_empty() {
        let bh = 12 * lines.len() + 16;
        let band = Rect { x: inner.x + 14, y: inner.y + inner.h - bh - 14, w: inner.w - 28 - legend_width(info), h: bh };
        if band.w > 120 {
            ui::card(buf, w, band);
            let lines = ui::wrap(&ui::ascii(&info.caption), (band.w - 24) / 7);
            for (k, l) in lines.iter().take(3).enumerate() {
                draw_ink(buf, w, h, (band.x + 12) as i64, (band.y + 8 + 12 * k) as i64, l, INK, 1, false);
            }
        }
    }

    // The legend of realms in view.
    if !info.realms.is_empty() {
        let lw = legend_width(info) - 10;
        let lh = 22 + 16 * info.realms.len();
        let r = Rect { x: inner.x + inner.w - lw - 14, y: inner.y + inner.h - lh - 14, w: lw, h: lh };
        ui::card(buf, w, r);
        draw_ink(buf, w, h, (r.x + 10) as i64, (r.y + 7) as i64, "Realms", RUBRIC, 1, true);
        for (k, (name, arms)) in info.realms.iter().enumerate() {
            let y = r.y + 20 + 16 * k;
            super::heraldry::draw(buf, w, h, (r.x + 9) as i64, y as i64 - 1, 14, arms);
            draw_ink(buf, w, h, (r.x + 26) as i64, y as i64 + 2, &ui::truncate(&ui::ascii(name), (lw - 36) / 7), INK, 1, false);
        }
    }
}

fn legend_width(info: &PlateInfo) -> usize {
    if info.realms.is_empty() { return 0; }
    info.realms.iter().map(|(n, _)| text_width(&ui::ascii(n), 1)).max().unwrap_or(0).min(220) + 48
}

/// The next free numbered path in `plates/`: `plates/plate_<seed>_<n>.png`.
pub fn next_path(seed: u64) -> String {
    let _ = std::fs::create_dir_all("plates");
    (1..).map(|n| format!("plates/plate_{}_{:03}.png", seed, n)).find(|p| !std::path::Path::new(p).exists()).unwrap()
}

/// Save a plate with its metadata in PNG text chunks: `world-args` (the command line that makes
/// the world), `view` ("x,y,tile_px"), `seed`, `caption`.
pub fn save(path: &str, buf: &[u32], w: usize, h: usize, view: (f32, f32, f32), info: &PlateInfo) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.add_text_chunk("world-args".into(), WORLD_ARGS.get().cloned().unwrap_or_default())?;
    enc.add_text_chunk("view".into(), format!("{:.2},{:.2},{:.2}", view.0, view.1, view.2))?;
    enc.add_text_chunk("seed".into(), info.seed.to_string())?;
    enc.add_text_chunk("caption".into(), info.caption.clone())?;
    let mut writer = enc.write_header()?;
    let data: Vec<u8> = buf.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    writer.write_image_data(&data)?;
    Ok(())
}

/// A plate's metadata: (world args, view x, view y, tile px).
pub fn read(path: &str) -> Result<(String, f32, f32, f32), Box<dyn std::error::Error>> {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let reader = dec.read_info()?;
    let info = reader.info();
    let text = |key: &str| info.uncompressed_latin1_text.iter().find(|t| t.keyword == key).map(|t| t.text.clone());
    let args = text("world-args").ok_or("not a plate (no world-args)")?;
    let view = text("view").ok_or("not a plate (no view)")?;
    let v: Vec<f32> = view.split(',').filter_map(|x| x.parse().ok()).collect();
    if v.len() != 3 { return Err("bad view in plate".into()); }
    Ok((args, v[0], v[1], v[2]))
}
