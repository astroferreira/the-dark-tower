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

use super::text::{draw_ink, text_width};
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
    draw_ink(buf, w, h, cx as i64 - 3, (cy - r - 12.0) as i64, "N", RUBRIC, 1, true);
}

/// Show the start screen. Returns the chosen options, or None if the window was closed or Esc
/// pressed.
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

    while window.is_open() {
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
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) || window.is_key_pressed(Key::NumPadEnter, KeyRepeat::No) {
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
        for (i, p) in buf.iter_mut().enumerate() {
            let grain = ((hash(i % w / 2, i / w / 9) & 0x1F) as f32 / 31.0) * 0.08;
            *p = mix(DESK, 0x0040_3428, grain);
        }
        let cw = 900.min(w - 32);
        let ch = 600.min(h - 32);
        let c = Rect { x: (w - cw) / 2, y: (h - ch) / 2, w: cw, h: ch };
        card(&mut buf, w, c);
        compass(&mut buf, w, h, (c.x + c.w - 70) as f32, (c.y + 74) as f32, 34.0);

        let x = c.x + 36;
        let mut y = c.y as i64 + 30;
        draw_ink(&mut buf, w, h, x as i64, y, "THE DARK TOWER", RUBRIC, 3, true);
        y += 32;
        draw_ink(&mut buf, w, h, x as i64, y, "A new world is drawn", INK_FADED, 2, false);
        y += 28;
        hline(&mut buf, w, x, c.x + c.w - 140, y as usize, INK);
        hline(&mut buf, w, x, c.x + c.w - 140, y as usize + 2, INK_FADED);
        y += 18;

        hits.clear();
        let label_w = 290;
        let value_x = x + label_w;
        let value_w = 300;
        let help_x = value_x + value_w + 30;
        let help_w = (c.x + c.w).saturating_sub(help_x + 30);
        for (i, &row) in ROWS.iter().enumerate() {
            if row == Row::Years {
                y += 6;
                heading(&mut buf, w, h, x, y, label_w + value_w, "HISTORY");
                y += 18;
            }
            if row == Row::Begin { y += 14; }
            let line_h = 30;
            let band = Rect { x: x - 12, y: y as usize - 6, w: label_w + value_w + 12, h: line_h - 4 };
            let selected = i == sel;
            if row == Row::Begin {
                let bw = 200;
                let b = Rect { x, y: y as usize - 8, w: bw, h: 40 };
                fill(&mut buf, w, b, if selected { RUBRIC } else { mix(RUBRIC, PAPER, 0.15) });
                outline(&mut buf, w, b, INK);
                let t = "BEGIN";
                draw_ink(&mut buf, w, h, (b.x + (bw - text_width(t, 2)) / 2) as i64, b.y as i64 + 12, t, PAPER, 2, true);
                hits.push(Hit { row: i, rect: b, delta: 0 });
                let est = estimate(&menu.cfg);
                draw_ink(&mut buf, w, h, (x + bw + 20) as i64, y + 4, &est, INK_FADED, 1, false);
                y += 40;
                continue;
            }
            if selected { fill(&mut buf, w, band, PAPER_SHADE); }
            if selected { draw_ink(&mut buf, w, h, (x - 10) as i64, y, ">", RUBRIC, 2, true); }
            draw_ink(&mut buf, w, h, (x + 8) as i64, y, Menu::label(row), INK, 2, selected);
            // Value with arrows.
            let v = ascii(&menu.value(row));
            let lx = value_x;
            draw_ink(&mut buf, w, h, lx as i64, y, "<", if selected { RUBRIC } else { INK_FADED }, 2, true);
            draw_ink(&mut buf, w, h, (lx + 22) as i64, y, &truncate(&v, (value_w - 50) / 14), INK, 2, false);
            draw_ink(&mut buf, w, h, (lx + value_w - 16) as i64, y, ">", if selected { RUBRIC } else { INK_FADED }, 2, true);
            hits.push(Hit { row: i, rect: Rect { x: lx - 4, y: band.y, w: 24, h: band.h }, delta: -1 });
            hits.push(Hit { row: i, rect: Rect { x: lx + value_w - 20, y: band.y, w: 24, h: band.h }, delta: 1 });
            hits.push(Hit { row: i, rect: band, delta: 0 });
            y += line_h as i64;
        }

        // Help for the selected row.
        let hy = c.y + 160;
        if help_w > 120 {
            let r = Rect { x: help_x - 14, y: hy - 12, w: help_w + 14, h: 230 };
            outline(&mut buf, w, r, INK_FADED);
            let row = ROWS[sel];
            draw_ink(&mut buf, w, h, help_x as i64, hy as i64, &Menu::label(row).to_uppercase(), RUBRIC, 1, true);
            let mut ty = hy as i64 + 18;
            for line in wrap(&menu.help(row), help_w / 7) {
                draw_ink(&mut buf, w, h, help_x as i64, ty, &line, INK, 1, false);
                ty += 13;
            }
        }
        let keys = "UP/DOWN choose   LEFT/RIGHT change   type digits for a seed   R new seed   ENTER begin   ESC quit";
        let kx = c.x + (c.w.saturating_sub(text_width(keys, 1))) / 2;
        draw_ink(&mut buf, w, h, kx as i64, (c.y + c.h - 26) as i64, keys, INK_FADED, 1, false);

        window.update_with_buffer(&buf, w, h)?;
    }
    Ok(None)
}
