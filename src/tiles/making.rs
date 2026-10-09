//! The world being made: a window kept open between the start screen and the history (it had
//! closed on Begin, and generation spoke only to the terminal). A parchment card lists the
//! pipeline's steps, ticked as they finish, the one under way in rubric, beside a small inked
//! map of the land as it stands after each stage (`terrain::generate_terrain`'s `on_stage`).
//! `PLANET_MAKING_FRAMES=PREFIX` also saves each frame as `<prefix>_NN.png` (headless too).

use minifb::{Window, WindowOptions};

use super::ui::{self, Rect, INK, INK_FADED, PAPER, RUBRIC};
use crate::tilemap::Tilemap;

/// The steps as the player reads them, and the pipeline stages (or notes) that finish each.
const STEPS: [(&str, &[&str]); 9] = [
    ("The plates drift and collide", &["tectonic"]),
    ("The winds and rains are reckoned", &["climate"]),
    ("Rivers wear the land down", &["landscape", "erosion"]),
    ("Coasts and fjords are cut", &["coastline", "fjords", "noise"]),
    ("Fire mountains rise", &["volcanoes", "islands"]),
    ("The shores are finished", &["beaches", "final"]),
    ("Lakes and rivers are found", &["water"]),
    ("Woods, deserts and ice are laid", &["biomes"]),
    ("The ages are written", &["history"]),
];

pub struct Making {
    window: Option<Window>,
    frames: Option<String>,
    frame_no: usize,
    size: (usize, usize),
    buf: Vec<u32>,
    title: String,
    done: Vec<&'static str>,
    current: String,
    preview: Option<(Vec<u32>, usize, usize)>,
    t0: std::time::Instant,
}

impl Making {
    /// A window when `show` (the game was begun from the start screen); frames when
    /// `PLANET_MAKING_FRAMES` is set. With neither, every call does nothing.
    pub fn new(width: usize, height: usize, seed: u64, style: &str, show: bool) -> Making {
        let frames = std::env::var("PLANET_MAKING_FRAMES").ok();
        let window = if show { Window::new("The Dark Tower - the world is being made", 1100, 760, WindowOptions { resize: true, ..WindowOptions::default() }).ok() } else { None };
        let mut m = Making { window, frames, frame_no: 0, size: (1100, 760), buf: vec![0; 1100 * 760], title: format!("A {}x{} {} world, seed {}", width, height, style, seed), done: Vec::new(), current: "The plates drift and collide".into(), preview: None, t0: std::time::Instant::now() };
        m.draw();
        // A new window shows nothing until the system has had a few events (the first stage runs
        // seconds without a redraw): present the first frame a few times.
        if let Some(win) = m.window.as_mut() {
            for _ in 0..6 { std::thread::sleep(std::time::Duration::from_millis(16)); let _ = win.update_with_buffer(&m.buf, m.size.0, m.size.1); }
        }
        m
    }

    fn active(&self) -> bool { self.window.is_some() || self.frames.is_some() }

    /// Close the window (the watcher or the viewer opens its own).
    pub fn close(&mut self) { self.window = None; self.frames = None; }

    /// A pipeline stage finished: tick it, and ink the land as it now stands.
    pub fn stage(&mut self, name: &'static str, heightmap: &Tilemap<f32>) {
        if !self.active() { return; }
        self.done.push(name);
        self.preview = Some(preview(heightmap, 520));
        self.advance();
        self.draw();
    }

    /// A later step (water, biomes, history) begins or ends.
    pub fn note(&mut self, finished: &'static str) {
        if !self.active() { return; }
        self.done.push(finished);
        self.advance();
        self.draw();
    }

    fn advance(&mut self) {
        self.current = STEPS.iter().find(|(_, keys)| !keys.iter().all(|k| self.done.contains(k))).map(|s| s.0.to_string()).unwrap_or_else(|| "The world is made".into());
    }

    fn draw(&mut self) {
        if !self.active() { return; }
        let (w, h) = match &self.window { Some(win) => win.get_size(), None => self.size };
        let (w, h) = (w.max(600), h.max(420));
        if (w, h) != self.size || self.buf.len() != w * h { self.size = (w, h); self.buf = vec![0; w * h]; }
        let buf = &mut self.buf;
        for y in 0..h { for x in 0..w { let n = (ui::hash(x / 3, y / 3) & 0xFF) as f32 / 255.0; buf[y * w + x] = ui::mix(PAPER, 0x00D8_C8A0, 0.35 * n); } }
        let (pw, ph) = self.preview.as_ref().map_or((520, 260), |p| (p.1, p.2));
        let cw = (380 + 40 + pw).min(w - 40);
        let card = Rect { x: (w - cw) / 2, y: 60, w: cw, h: (ph.max(STEPS.len() * 24) + 140).min(h - 80) };
        ui::card(buf, w, card);
        super::text::draw_fell(buf, w, h, (card.x + 24) as i64, (card.y + 20) as i64, "The world is being made", RUBRIC, 2, true);
        super::text::draw_fell(buf, w, h, (card.x + 24) as i64, (card.y + 50) as i64, &self.title, INK_FADED, 1, false);
        let mut y = card.y + 86;
        for (label, keys) in STEPS.iter() {
            let finished = keys.iter().all(|k| self.done.contains(k));
            let now = !finished && self.current == *label;
            // A tick in ink, a ring for what is under way, a faint dot for what is to come.
            let (cx, cy) = (card.x + 34, y + 8);
            {
                let mut put = |px: i64, py: i64, c: [f32; 3], a: f32| {
                    if px < 0 || py < 0 || px as usize >= w || py as usize >= h { return; }
                    let k = py as usize * w + px as usize;
                    let p = buf[k];
                    let old = [((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32];
                    buf[k] = super::ink::pack([old[0] + (c[0] - old[0]) * a, old[1] + (c[1] - old[1]) * a, old[2] + (c[2] - old[2]) * a]);
                };
                let mut pen = super::ink::Pen::new(&mut put, cx as f32, cy as f32, 16.0);
                if finished { pen.path(&[(-0.4, 0.0), (-0.1, 0.35), (0.45, -0.4)], [56.0, 42.0, 32.0], 2.2); }
                else if now { pen.ellipse(0.0, 0.0, 0.32, 0.32, [234.0, 200.0, 170.0]); }
                else { pen.ellipse_f(0.0, 0.0, 0.1, 0.1, [150.0, 134.0, 110.0], super::ink::Finish::Paint); }
            }
            super::text::draw_fell(buf, w, h, (card.x + 52) as i64, y as i64, label, if now { RUBRIC } else if finished { INK } else { INK_FADED }, 1, now);
            y += 24;
        }
        let secs = self.t0.elapsed().as_secs();
        super::text::draw_fell(buf, w, h, (card.x + 24) as i64, (card.y + card.h - 30) as i64, &format!("{} s", secs), INK_FADED, 1, false);
        if let Some((pix, pw, ph)) = &self.preview {
            let (ox, oy) = (card.x + cw - pw - 24, card.y + 86);
            for yy in 0..*ph { for xx in 0..*pw { if oy + yy < h && ox + xx < w { buf[(oy + yy) * w + ox + xx] = pix[yy * pw + xx]; } } }
            ui::outline(buf, w, Rect { x: ox - 1, y: oy - 1, w: pw + 2, h: ph + 2 }, INK);
            ui::outline(buf, w, Rect { x: ox - 5, y: oy - 5, w: pw + 10, h: ph + 10 }, INK_FADED);
        }
        if let Some(win) = self.window.as_mut() { let _ = win.update_with_buffer(&self.buf, w, h); }
        if let Some(prefix) = &self.frames {
            let path = format!("{}_{:02}.png", prefix, self.frame_no);
            let b = &self.buf;
            super::viewer::save_rgb_png_pub(&path, w, h, |x, y| { let q = b[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
            self.frame_no += 1;
        }
    }
}

/// The heightmap inked small: the sea in the map's washes by depth, the land by height from
/// green through ochre to pale stone, lit from the top left, an ink line at the shore.
fn preview(hm: &Tilemap<f32>, pw: usize) -> (Vec<u32>, usize, usize) {
    let (w, h) = (hm.width, hm.height);
    let ph = (pw * h / w).max(1);
    let at = |x: usize, y: usize| *hm.get(x.min(w - 1), y.min(h - 1));
    // Bilinear between tile centres, so a small world isn't drawn in blocks.
    let smooth = |fx: f32, fy: f32| -> f32 {
        let (fx, fy) = ((fx - 0.5).max(0.0), (fy - 0.5).max(0.0));
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
        (a + (b - a) * tx) * (1.0 - ty) + (c + (d - c) * tx) * ty
    };
    let mut out = vec![0u32; pw * ph];
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    for py in 0..ph {
        for px in 0..pw {
            let (fx, fy) = ((px as f32 + 0.5) * w as f32 / pw as f32, (py as f32 + 0.5) * h as f32 / ph as f32);
            let e = smooth(fx, fy);
            let d = w as f32 / pw as f32;
            let c = if e < 0.0 {
                let t = (-e / 3000.0).clamp(0.0, 1.0).sqrt();
                mix([156.0, 184.0, 178.0], [96.0, 128.0, 142.0], t)
            } else {
                let t = (e / 3500.0).clamp(0.0, 1.0);
                let base = if t < 0.35 { mix([168.0, 176.0, 128.0], [196.0, 178.0, 126.0], t / 0.35) } else { mix([196.0, 178.0, 126.0], [226.0, 220.0, 206.0], (t - 0.35) / 0.65) };
                let slope = (smooth(fx + d, fy + d) - smooth(fx - d, fy - d)) / (400.0 * d.max(0.25));
                let k = (1.0 - slope * 0.25).clamp(0.75, 1.12);
                [base[0] * k, base[1] * k, base[2] * k]
            };
            let shore = (e >= 0.0) != (smooth(fx + d, fy) >= 0.0) || (e >= 0.0) != (smooth(fx, fy + d) >= 0.0);
            let c = if shore { mix(c, [56.0, 42.0, 32.0], 0.7) } else { c };
            out[py * pw + px] = super::ink::pack(c);
        }
    }
    (out, pw, ph)
}

thread_local! {
    /// The one making window, kept on the main thread (minifb windows stay where they were made).
    static MAKING: std::cell::RefCell<Option<Making>> = const { std::cell::RefCell::new(None) };
}

/// Open the making window (or start saving frames); `show` as in `Making::new`.
pub fn begin(width: usize, height: usize, seed: u64, style: &str, show: bool) {
    let m = Making::new(width, height, seed, style, show);
    MAKING.with(|c| *c.borrow_mut() = if m.active() { Some(m) } else { None });
}

/// A pipeline stage finished (see `Making::stage`).
pub fn stage(name: &'static str, heightmap: &Tilemap<f32>) { MAKING.with(|c| if let Some(m) = c.borrow_mut().as_mut() { m.stage(name, heightmap); }); }

/// A later step finished (see `Making::note`).
pub fn note(finished: &'static str) { MAKING.with(|c| if let Some(m) = c.borrow_mut().as_mut() { m.note(finished); }); }

/// Close the window.
pub fn close() { MAKING.with(|c| *c.borrow_mut() = None); }
