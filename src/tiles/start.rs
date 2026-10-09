//! The start screen: a parchment card where the generation options are tailored before a world
//! is made (size, seed, world style, tectonics, fantasy, history, peoples, the Shadow, whether
//! to watch the history being written). Shown when the game runs with no arguments, or with
//! `--start` (other flags then pre-fill it).
//!
//! Keys: Up/Down choose a row, Left/Right change it, digits type a seed (Backspace deletes),
//! R rolls a new seed, Enter begins, Esc quits. Everything can also be clicked.

use std::error::Error;

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

use crate::plates::WorldStyle;

use super::ui::*;

/// The options a new world is made with.
#[derive(Clone, Debug)]
pub struct StartConfig {
    pub width: usize,
    pub height: usize,
    pub seed: u64,
    pub style: WorldStyle,
    /// None: the generator picks 6-15.
    pub plates: Option<usize>,
    pub tectonic_myr: f32,
    pub fantasy: f32,
    /// 0: no history.
    pub history_years: u32,
    pub civilizations: u32,
    pub shadow: bool,
    pub watch: bool,
}

const SIZES: [(usize, usize, &str, &str); 4] = [
    (96, 48, "Dev", "A tiny world for trying things out: generated with its history in about a second."),
    (256, 128, "Small", "A small world: a handful of lands and seas, quick to generate."),
    (512, 256, "Standard", "The standard world: continents, island chains and long rivers."),
    (1024, 512, "Large", "A large world with room for many peoples. Slow: allow some time."),
];
const MYR: [f32; 3] = [100.0, 200.0, 400.0];
const FANTASY: [(f32, &str); 7] = [
    (0.0, "None"), (0.1, "Faint"), (0.2, "A touch"), (0.35, "Noticeable"), (0.5, "Strong"), (0.75, "Wild"), (1.0, "High fantasy"),
];
const YEARS: [u32; 5] = [0, 100, 250, 500, 1000];
const PEOPLES: [u32; 9] = [4, 6, 8, 12, 20, 30, 45, 60, 100];

#[derive(Clone, Copy, PartialEq)]
enum Row { Size, Seed, Style, Plates, Myr, Fantasy, Years, Peoples, Shadow, Watch, Begin }

const ROWS: [Row; 11] = [Row::Size, Row::Seed, Row::Style, Row::Plates, Row::Myr, Row::Fantasy, Row::Years, Row::Peoples, Row::Shadow, Row::Watch, Row::Begin];

fn nearest<T: Copy>(list: &[T], key: impl Fn(T) -> f32, target: f32) -> usize {
    (0..list.len()).min_by(|&a, &b| (key(list[a]) - target).abs().partial_cmp(&(key(list[b]) - target).abs()).unwrap()).unwrap_or(0)
}

fn step_index(i: usize, len: usize, d: i32) -> usize {
    (i as i32 + d).rem_euclid(len as i32) as usize
}

/// Rough generation time, from measurements on an M4 Pro (world and 250 years of history
/// together: Dev ~1 s, Small 3.4 s, Standard 11 s, Large 66 s).
fn estimate(cfg: &StartConfig) -> String {
    let r = (cfg.width * cfg.height) as f32 / (512.0 * 256.0);
    let world = 4.0 * r.powf(1.2) * cfg.tectonic_myr / 200.0;
    let peoples = (cfg.civilizations as f32 / suggested_peoples(cfg.width) as f32).max(0.2).sqrt();
    let history = 7.0 * r.powf(1.2) * cfg.history_years as f32 / 250.0 * peoples;
    let fmt = |s: f32| if s < 5.0 { "a few seconds".to_string() } else if s < 90.0 { format!("about {:.0} s", s) } else { format!("about {:.0} min", s / 60.0) };
    if cfg.history_years == 0 { format!("World: {}. No history.", fmt(world)) } else { format!("World: {}. History: {}.", fmt(world), fmt(history)) }
}

fn suggested_peoples(width: usize) -> u32 {
    if width <= 128 { 8 } else if width <= 256 { 20 } else { 60 }
}

struct Menu {
    cfg: StartConfig,
    size: usize,
    style: usize,
    myr: usize,
    fantasy: usize,
    years: usize,
    peoples: usize,
    seed_text: String,
}

impl Menu {
    fn new(cfg: StartConfig) -> Self {
        let size = SIZES.iter().position(|s| s.0 == cfg.width && s.1 == cfg.height).unwrap_or(2);
        let style = WorldStyle::all().iter().position(|s| *s == cfg.style).unwrap_or(0);
        Menu {
            size,
            style,
            myr: nearest(&MYR, |v| v, cfg.tectonic_myr),
            fantasy: nearest(&FANTASY, |v| v.0, cfg.fantasy),
            years: nearest(&YEARS, |v| v as f32, cfg.history_years as f32),
            peoples: nearest(&PEOPLES, |v| v as f32, cfg.civilizations as f32),
            seed_text: cfg.seed.to_string(),
            cfg,
        }
    }

    fn change(&mut self, row: Row, d: i32) {
        match row {
            Row::Size => {
                self.size = step_index(self.size, SIZES.len(), d);
                let (w, h, _, _) = SIZES[self.size];
                self.cfg.width = w;
                self.cfg.height = h;
                self.peoples = nearest(&PEOPLES, |v| v as f32, suggested_peoples(w) as f32);
            }
            Row::Seed => self.reroll(),
            Row::Style => self.style = step_index(self.style, WorldStyle::all().len(), d),
            Row::Plates => {
                // None, then 6..=15.
                let opts: Vec<Option<usize>> = std::iter::once(None).chain((6..=15).map(Some)).collect();
                let i = opts.iter().position(|o| *o == self.cfg.plates).unwrap_or(0);
                self.cfg.plates = opts[step_index(i, opts.len(), d)];
            }
            Row::Myr => self.myr = step_index(self.myr, MYR.len(), d),
            Row::Fantasy => self.fantasy = step_index(self.fantasy, FANTASY.len(), d),
            Row::Years => self.years = step_index(self.years, YEARS.len(), d),
            Row::Peoples => self.peoples = step_index(self.peoples, PEOPLES.len(), d),
            Row::Shadow => self.cfg.shadow = !self.cfg.shadow,
            Row::Watch => self.cfg.watch = !self.cfg.watch,
            Row::Begin => {}
        }
        self.sync();
    }

    fn reroll(&mut self) {
        self.cfg.seed = rand::random::<u64>() % 1_000_000_000;
        self.seed_text = self.cfg.seed.to_string();
    }

    fn sync(&mut self) {
        self.cfg.style = WorldStyle::all()[self.style];
        self.cfg.tectonic_myr = MYR[self.myr];
        self.cfg.fantasy = FANTASY[self.fantasy].0;
        self.cfg.history_years = YEARS[self.years];
        self.cfg.civilizations = PEOPLES[self.peoples];
        if let Ok(s) = self.seed_text.parse::<u64>() { self.cfg.seed = s; }
    }

    fn label(row: Row) -> &'static str {
        match row {
            Row::Size => "World size",
            Row::Seed => "Seed",
            Row::Style => "Shape of the lands",
            Row::Plates => "Tectonic plates",
            Row::Myr => "Age of the crust",
            Row::Fantasy => "Fantasy",
            Row::Years => "Written history",
            Row::Peoples => "Founding peoples",
            Row::Shadow => "The Shadow",
            Row::Watch => "Watch it unfold",
            Row::Begin => "Begin",
        }
    }

    fn value(&self, row: Row) -> String {
        let c = &self.cfg;
        match row {
            Row::Size => format!("{}  {}x{}", SIZES[self.size].2, c.width, c.height),
            Row::Seed => if self.seed_text.is_empty() { "_".into() } else { self.seed_text.clone() },
            Row::Style => {
                let name = c.style.to_string();
                let mut chars = name.chars();
                chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
            }
            Row::Plates => c.plates.map_or("Random (6-15)".into(), |p| p.to_string()),
            Row::Myr => format!("{:.0} million years", c.tectonic_myr),
            Row::Fantasy => FANTASY[self.fantasy].1.to_string(),
            Row::Years => if c.history_years == 0 { "None".into() } else { format!("{} years", c.history_years) },
            Row::Peoples => c.civilizations.to_string(),
            Row::Shadow => if c.shadow { "Rises".into() } else { "None (sandbox)".into() },
            Row::Watch => if c.watch { "Yes".into() } else { "No".into() },
            Row::Begin => String::new(),
        }
    }

    fn help(&self, row: Row) -> String {
        let c = &self.cfg;
        match row {
            Row::Size => SIZES[self.size].3.to_string(),
            Row::Seed => "The same seed and options always make the same world and the same history. Type digits to enter one, or press R for a new one.".into(),
            Row::Style => format!("{}.", c.style.description()),
            Row::Plates => "How many tectonic plates the crust starts with. More plates make more, smaller continents and more mountain belts.".into(),
            Row::Myr => "How long the plates drift before the present. Older worlds have seen more collisions and rifting: more ranges, more scattered lands.".into(),
            Row::Fantasy => "How much of the world is strange: crystal woods, glowing marshes, ashlands. None is a natural world.".into(),
            Row::Years => "Years of history simulated before the present day: peoples found towns, build roads, wage wars, and the land remembers. None makes an empty world.".into(),
            Row::Peoples => "The peoples who found the first realms. Fewer means each one matters more.".into(),
            Row::Shadow => "A dark power rises at the dawn of history and spreads across the land, taking town after town. Turn it off for a world without a central enemy.".into(),
            Row::Watch => "Open a window that plays the history back as it is written: towns, roads and borders spreading, wars and falls in the chronicle.".into(),
            Row::Begin => "Make this world.".into(),
        }
    }
}

struct Hit { row: usize, rect: Rect, delta: i32 }

/// Draw a small ink compass rose centred at (cx, cy).
fn compass(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32) {
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::PI / 8.0;
        let len = if k % 4 == 0 { r } else if k % 2 == 0 { r * 0.6 } else { r * 0.35 };
        let steps = (len * 2.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps.max(1) as f32 * len;
            blend_px(buf, w, h, (cx + a.sin() * t) as i64, (cy - a.cos() * t) as i64, if k == 0 { RUBRIC } else { INK }, if k % 4 == 0 { 1.0 } else { 0.7 });
        }
    }
    let ri = r * 0.22;
    for k in 0..64 {
        let a = k as f32 / 64.0 * std::f32::consts::TAU;
        blend_px(buf, w, h, (cx + a.cos() * ri) as i64, (cy + a.sin() * ri) as i64, INK, 0.8);
    }
    super::fonts::draw(buf, w, h, cx - 5.0, cy - r - 18.0, "N", super::fonts::Face::SmallCaps, 15.0, 0.0, RUBRIC, None);
}

/// Show the start screen. Returns the chosen options, or None if the window was closed or Esc
/// pressed.
/// Draw the start screen into `buf` (the window's frame, or `--start-snapshot`'s): a parchment
/// card on the desk with the compass, the title, one row a choice (its value between ink arrows,
/// the selected row washed), the history's rows under their heading, Begin with the time it will
/// take, the help for the selected row; lettered in IM Fell. Returns what can be clicked.
fn draw_start(menu: &Menu, sel: usize, mouse: (f32, f32), buf: &mut [u32], w: usize, h: usize) -> Vec<Hit> {
    use super::fonts::{self, Face};
    let _ = mouse;
    for (i, p) in buf.iter_mut().enumerate() {
        let grain = ((hash(i % w / 2, i / w / 9) & 0x1F) as f32 / 31.0) * 0.08;
        *p = mix(DESK, 0x0040_3428, grain);
    }
    let cw = 900.min(w - 32);
    let ch = 620.min(h - 32);
    let c = Rect { x: (w - cw) / 2, y: (h - ch) / 2, w: cw, h: ch };
    card(buf, w, c);
    compass(buf, w, h, (c.x + c.w - 70) as f32, (c.y + 74) as f32, 34.0);
    let x = c.x + 36;
    let mut y = c.y as f32 + 24.0;
    fonts::draw(buf, w, h, x as f32, y, "The Dark Tower", Face::SmallCaps, 36.0, 1.5, RUBRIC, None);
    y += 44.0;
    fonts::draw(buf, w, h, x as f32, y, "A new world is drawn", Face::Italic, 20.0, 0.0, INK_FADED, None);
    y += 32.0;
    hline(buf, w, x, c.x + c.w - 140, y as usize, INK);
    hline(buf, w, x, c.x + c.w - 140, y as usize + 2, INK_FADED);
    y += 16.0;
    let mut hits = Vec::new();
    let label_w = 250;
    let value_x = x + label_w;
    let value_w = 300;
    let help_x = value_x + value_w + 30;
    let help_w = (c.x + c.w).saturating_sub(help_x + 30);
    // A small inked arrow (a triangle) for the value's steppers.
    let arrow = |buf: &mut [u32], cx: f32, cy: f32, left: bool, col: u32| {
        for k in 0..9 {
            let half = if left { k } else { 8 - k } as f32 * 0.6;
            let xx = cx - 4.0 + k as f32;
            for dy in -(half as i64)..=(half as i64) { blend_px(buf, w, h, xx as i64, cy as i64 + dy, col, 0.95); }
        }
    };
    for (i, &row) in ROWS.iter().enumerate() {
        if row == Row::Years {
            y += 6.0;
            fonts::draw(buf, w, h, x as f32, y - 2.0, "History", Face::SmallCaps, 17.0, 0.6, RUBRIC, None);
            let tx = x + fonts::width("History", Face::SmallCaps, 17.0, 0.6) as usize + 8;
            hline(buf, w, tx, x + label_w + value_w, (y + 8.0) as usize, INK_FADED);
            y += 24.0;
        }
        if row == Row::Begin { y += 12.0; }
        let line_h = 30.0;
        let band = Rect { x: x - 12, y: y as usize - 5, w: label_w + value_w + 12, h: line_h as usize - 3 };
        let selected = i == sel;
        if row == Row::Begin {
            let bw = 200;
            let b = Rect { x, y: y as usize - 6, w: bw, h: 40 };
            fill(buf, w, b, if selected { RUBRIC } else { mix(RUBRIC, PAPER, 0.15) });
            outline(buf, w, b, INK);
            let tw = fonts::width("Begin", Face::SmallCaps, 24.0, 1.0);
            fonts::draw(buf, w, h, b.x as f32 + (bw as f32 - tw) / 2.0, b.y as f32 + 6.0, "Begin", Face::SmallCaps, 24.0, 1.0, PAPER, None);
            hits.push(Hit { row: i, rect: b, delta: 0 });
            let est = estimate(&menu.cfg);
            fonts::draw(buf, w, h, (x + bw + 20) as f32, y + 4.0, &est, Face::Italic, 15.0, 0.0, INK_FADED, None);
            continue;
        }
        if selected { fill(buf, w, band, PAPER_SHADE); arrow(buf, (x - 4) as f32, y + 10.0, false, RUBRIC); }
        fonts::draw(buf, w, h, (x + 10) as f32, y, Menu::label(row), if selected { Face::SmallCaps } else { Face::Roman }, 18.0, if selected { 0.4 } else { 0.0 }, if selected { RUBRIC } else { INK }, None);
        let v = menu.value(row);
        let lx = value_x;
        arrow(buf, (lx + 6) as f32, y + 10.0, true, if selected { RUBRIC } else { INK_FADED });
        let shown = { let mut t = v.clone(); while fonts::width(&t, Face::Roman, 18.0, 0.0) > (value_w - 56) as f32 && !t.is_empty() { t.pop(); } t };
        fonts::draw(buf, w, h, (lx + 22) as f32, y, &shown, Face::Roman, 18.0, 0.0, INK, None);
        arrow(buf, (lx + value_w - 10) as f32, y + 10.0, false, if selected { RUBRIC } else { INK_FADED });
        hits.push(Hit { row: i, rect: Rect { x: lx - 4, y: band.y, w: 24, h: band.h }, delta: -1 });
        hits.push(Hit { row: i, rect: Rect { x: lx + value_w - 20, y: band.y, w: 24, h: band.h }, delta: 1 });
        hits.push(Hit { row: i, rect: band, delta: 0 });
        y += line_h;
    }
    // Help for the selected row.
    let hy = c.y + 160;
    if help_w > 120 {
        let r = Rect { x: help_x - 14, y: hy - 12, w: help_w + 14, h: 250 };
        outline(buf, w, r, INK_FADED);
        let row = ROWS[sel];
        fonts::draw(buf, w, h, help_x as f32, hy as f32 - 4.0, Menu::label(row), Face::SmallCaps, 16.0, 0.5, RUBRIC, None);
        let mut ty = hy as f32 + 20.0;
        for line in fonts::wrap(&menu.help(row), Face::Roman, 15.0, help_w as f32 - 6.0) {
            fonts::draw(buf, w, h, help_x as f32, ty, &line, Face::Roman, 15.0, 0.0, INK, None);
            ty += 19.0;
        }
        // A picture of the choice, under its words.
        let (px, py) = (help_x as f32 + help_w as f32 / 2.0 - 7.0, (r.y + r.h) as f32 - 70.0);
        if py > ty + 40.0 { row_picture(buf, w, h, row, px, py, 96.0); }
    }
    let keys = "Up/Down choose    Left/Right change    type digits for a seed    R a new seed    Enter begin    Esc quit";
    let kw = fonts::width(keys, Face::Italic, 14.0, 0.0);
    fonts::draw(buf, w, h, c.x as f32 + (c.w as f32 - kw) / 2.0, (c.y + c.h - 30) as f32, keys, Face::Italic, 14.0, 0.0, INK_FADED, None);
    hits
}

/// The start screen drawn headlessly (`--start-snapshot FILE`), with `initial` and the first
/// row selected.
pub fn save_start_snapshot(initial: StartConfig, path: &str) -> Result<(), Box<dyn Error>> {
    let (w, h) = (1100usize, 760usize);
    let mut menu = Menu::new(initial);
    menu.sync();
    let mut buf = vec![0u32; w * h];
    let _ = draw_start(&menu, 0, (-1.0, -1.0), &mut buf, w, h);
    image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(path)?;
    Ok(())
}

pub fn run_start_screen(initial: StartConfig) -> Result<Option<StartConfig>, Box<dyn Error>> {
    let mut window = Window::new("The Dark Tower", 1100, 760, WindowOptions { resize: true, ..WindowOptions::default() })?;
    window.set_target_fps(60);
    let mut menu = Menu::new(initial);
    menu.sync();
    let mut sel = 0usize;
    let mut buf: Vec<u32> = Vec::new();
    let mut size = (0, 0);
    let mut was_down = false;
    let mut hits: Vec<Hit> = Vec::new();
    // PLANET_START_BEGIN=N: press Begin on frame N (testing the start -> making -> game flow).
    let auto_begin: Option<u64> = std::env::var("PLANET_START_BEGIN").ok().and_then(|v| v.parse().ok());
    let mut frame: u64 = 0;

    while window.is_open() {
        frame += 1;
        let (w, h) = window.get_size();
        if (w, h) != size {
            size = (w, h);
            buf = vec![0; w * h];
        }
        if w < 400 || h < 400 { window.update(); continue; }

        // --- Input ---
        let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::Yes);
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) { return Ok(None); }
        if pressed(Key::Up) { sel = step_index(sel, ROWS.len(), -1); }
        if pressed(Key::Down) || window.is_key_pressed(Key::Tab, KeyRepeat::Yes) { sel = step_index(sel, ROWS.len(), 1); }
        let row = ROWS[sel];
        if pressed(Key::Left) { menu.change(row, -1); }
        if pressed(Key::Right) { menu.change(row, 1); }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) || window.is_key_pressed(Key::NumPadEnter, KeyRepeat::No) || auto_begin == Some(frame) {
            menu.sync();
            return Ok(Some(menu.cfg));
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) { menu.reroll(); }
        // Digits type the seed (from any row).
        let digits = [
            (Key::Key0, '0'), (Key::Key1, '1'), (Key::Key2, '2'), (Key::Key3, '3'), (Key::Key4, '4'),
            (Key::Key5, '5'), (Key::Key6, '6'), (Key::Key7, '7'), (Key::Key8, '8'), (Key::Key9, '9'),
            (Key::NumPad0, '0'), (Key::NumPad1, '1'), (Key::NumPad2, '2'), (Key::NumPad3, '3'), (Key::NumPad4, '4'),
            (Key::NumPad5, '5'), (Key::NumPad6, '6'), (Key::NumPad7, '7'), (Key::NumPad8, '8'), (Key::NumPad9, '9'),
        ];
        for (k, c) in digits {
            if window.is_key_pressed(k, KeyRepeat::No) {
                if ROWS[sel] != Row::Seed { sel = ROWS.iter().position(|r| *r == Row::Seed).unwrap(); menu.seed_text.clear(); }
                if menu.seed_text.len() < 19 { menu.seed_text.push(c); }
                menu.sync();
            }
        }
        if ROWS[sel] == Row::Seed && pressed(Key::Backspace) {
            menu.seed_text.pop();
            menu.sync();
        }
        let mouse = window.get_mouse_pos(MouseMode::Clamp).unwrap_or((0.0, 0.0));
        let down = window.get_mouse_down(MouseButton::Left);
        let clicked = down && !was_down;
        was_down = down;
        if clicked {
            if let Some(hit) = hits.iter().find(|h| h.rect.contains(mouse.0, mouse.1)) {
                sel = hit.row;
                if ROWS[sel] == Row::Begin {
                    menu.sync();
                    return Ok(Some(menu.cfg));
                }
                if hit.delta != 0 { menu.change(ROWS[sel], hit.delta); }
            }
        }

        // --- Draw ---
        hits = draw_start(&menu, sel, mouse, &mut buf, w, h);
        window.update_with_buffer(&buf, w, h)?;
    }
    Ok(None)
}

/// A small inked picture for a start-screen row (in the help box).
fn row_picture(buf: &mut [u32], w: usize, h: usize, row: Row, cx: f32, cy: f32, size: f32) {
    use super::ink::{Finish, Pen, INK as K};
    let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| blend_px(buf, w, h, x, y, super::ink::pack(c), a);
    let mut pen = Pen::new(&mut put, cx, cy, size);
    let sea = [130.0, 160.0, 176.0];
    let land = [170.0, 176.0, 120.0];
    let stone = [178.0, 172.0, 160.0];
    let lw = (size * 0.03).max(1.0);
    match row {
        Row::Size => {
            pen.ellipse(0.0, 0.0, 0.8, 0.8, sea);
            pen.ellipse_f(-0.25, -0.2, 0.3, 0.22, land, Finish::Plain);
            pen.ellipse_f(0.3, 0.25, 0.25, 0.3, land, Finish::Plain);
            for k in [-0.4f32, 0.0, 0.4] { pen.line_a((-(0.62 - k * k).max(0.0).sqrt(), k), ((0.62 - k * k).max(0.0).sqrt(), k), K, 1.0, 0.35); }
            pen.line_a((0.0, -0.8), (0.0, 0.8), K, 1.0, 0.35);
        }
        Row::Seed => {
            pen.poly(&[(-0.5, -0.3), (0.0, -0.6), (0.5, -0.3), (0.5, 0.35), (0.0, 0.65), (-0.5, 0.35)], [236.0, 228.0, 210.0]);
            pen.line((-0.5, -0.3), (0.0, 0.0), K, lw); pen.line((0.5, -0.3), (0.0, 0.0), K, lw); pen.line((0.0, 0.0), (0.0, 0.65), K, lw);
            for (u, v) in [(0.0, -0.3), (-0.3, 0.1), (-0.2, 0.35), (0.25, 0.1), (0.35, 0.35), (0.3, 0.22)] { pen.ellipse_f(u, v, 0.05, 0.05, K, Finish::Paint); }
        }
        Row::Style => {
            pen.rect_f(-0.9, -0.6, 0.9, 0.6, sea, Finish::Plain);
            pen.poly(&[(-0.7, -0.3), (-0.2, -0.45), (0.1, -0.1), (-0.1, 0.35), (-0.6, 0.3)], land);
            pen.poly(&[(0.3, 0.0), (0.7, -0.2), (0.75, 0.3), (0.4, 0.45)], land);
        }
        Row::Plates => {
            pen.rect(-0.85, -0.55, 0.85, 0.55, [196.0, 170.0, 130.0]);
            pen.path(&[(-0.85, 0.1), (-0.3, -0.05), (0.1, 0.2), (0.85, 0.0)], [160.0, 50.0, 40.0], lw * 1.5);
            pen.path(&[(-0.1, -0.55), (0.0, -0.1), (0.1, 0.2), (0.0, 0.55)], [160.0, 50.0, 40.0], lw * 1.5);
            pen.path(&[(-0.6, -0.3), (-0.45, -0.3)], K, lw); pen.path(&[(0.5, 0.3), (0.65, 0.3)], K, lw);
        }
        Row::Myr => {
            for (u, hh) in [(-0.45f32, 0.7f32), (0.1, 0.95), (0.55, 0.6)] {
                pen.poly(&[(u - 0.4, 0.5), (u, 0.5 - hh), (u + 0.4, 0.5)], stone);
                pen.poly_f(&[(u - 0.1, 0.5 - hh * 0.75), (u, 0.5 - hh), (u + 0.1, 0.5 - hh * 0.75)], [240.0, 238.0, 232.0], Finish::Paint);
            }
        }
        Row::Fantasy => {
            pen.poly(&[(0.0, -0.75), (0.25, -0.1), (0.1, 0.6), (-0.15, 0.6), (-0.25, -0.1)], [170.0, 150.0, 200.0]);
            pen.poly(&[(0.35, -0.3), (0.5, 0.1), (0.42, 0.6), (0.25, 0.6), (0.2, 0.1)], [150.0, 190.0, 200.0]);
            pen.glow(0.0, -0.1, 0.7, [220.0, 200.0, 250.0], 0.4);
        }
        Row::Years => {
            pen.rect(-0.6, -0.45, 0.6, 0.45, [236.0, 226.0, 200.0]);
            pen.ellipse(-0.6, 0.0, 0.12, 0.45, [214.0, 200.0, 170.0]);
            pen.ellipse(0.6, 0.0, 0.12, 0.45, [214.0, 200.0, 170.0]);
            for k in 0..5 { let v = -0.28 + k as f32 * 0.14; pen.line((-0.4, v), (0.4, v), K, 1.0); }
        }
        Row::Peoples => {
            for (k, col) in [[150.0, 52.0, 44.0], [62.0, 84.0, 128.0], [70.0, 110.0, 70.0]].iter().enumerate() {
                let u = -0.5 + k as f32 * 0.5;
                pen.bone(&[(u, 0.7), (u, -0.7)], [122.0, 86.0, 54.0], lw * 1.3);
                pen.poly(&[(u, -0.7), (u + 0.38, -0.6), (u + 0.3, -0.4), (u + 0.38, -0.2), (u, -0.25)], *col);
            }
        }
        Row::Shadow => {
            pen.ellipse_f(0.0, 0.0, 0.85, 0.6, [52.0, 46.0, 50.0], Finish::Plain);
            pen.ellipse(0.0, 0.0, 0.55, 0.28, [214.0, 170.0, 60.0]);
            pen.ellipse_f(0.0, 0.0, 0.08, 0.26, K, Finish::Paint);
            pen.glow(0.0, 0.0, 0.7, [230.0, 100.0, 40.0], 0.35);
        }
        Row::Watch => {
            pen.rect(-0.45, -0.75, 0.45, -0.65, [122.0, 86.0, 54.0]);
            pen.rect(-0.45, 0.65, 0.45, 0.75, [122.0, 86.0, 54.0]);
            pen.poly(&[(-0.38, -0.65), (0.38, -0.65), (0.05, 0.0), (0.38, 0.65), (-0.38, 0.65), (-0.05, 0.0)], [236.0, 232.0, 220.0]);
            pen.poly_f(&[(-0.25, 0.62), (0.25, 0.62), (0.0, 0.3)], [206.0, 176.0, 110.0], Finish::Paint);
        }
        Row::Begin => {}
    }
}
