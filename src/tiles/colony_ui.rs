//! The colony's ledger and the patron's hand: everything about the camp, a click away.
//!
//! A parchment panel on the right with five leaves, opened by their tabs (top right, always
//! shown) or keys: Settlers (C), Stocks (I), Works (O), Annals (L) and the Camp (T). A click on a
//! settler's row selects them and brings the camera to them; a second click (or a click on them
//! on the map) opens their sheet: body and mind in words, skills, needs, thoughts, kin and
//! friends, wounds, arms, deeds, dreams and vows, and their past (each line opens its event in
//! the inspector). Along the bottom the patron's verbs and the clock are buttons as well as keys:
//! bless or forbid ground, favour a settler, send a dream, ring the bell, set the founding
//! stones, name the camp or a place. Every one goes through the same `Colony` method as its key,
//! so it is recorded in `interventions` and replays. Lettered in IM Fell, drawn in the map's
//! ink; things are shown with the map's own glyphs (`glyphs.rs`).

use super::fonts::{self, Face};
use super::glyphs::{self, Glyph};
use super::ui::{self, Rect, GOLD, INK, INK_FADED, MOSS, PAPER, RUBRIC, SEA};
use crate::colony::{Colony, Dream, Stuff};

/// Faded ink dark enough to read on parchment (about 5:1).
const SOFT: u32 = 0x005A_4634;
const BODY: f32 = 15.0;
const SMALL: f32 = 13.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab { Settlers, Stocks, Works, Annals, Camp }

impl Tab {
    pub(crate) const ALL: [Tab; 5] = [Tab::Settlers, Tab::Stocks, Tab::Works, Tab::Annals, Tab::Camp];
    fn word(self) -> &'static str { match self { Tab::Settlers => "Settlers", Tab::Stocks => "Stocks", Tab::Works => "Works", Tab::Annals => "Annals", Tab::Camp => "Camp" } }
    fn key(self) -> &'static str { match self { Tab::Settlers => "C", Tab::Stocks => "I", Tab::Works => "O", Tab::Annals => "L", Tab::Camp => "T" } }
    fn index(self) -> usize { self as usize }
}

/// A verb that waits for a click on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool { Bless, Forbid, Hall, Grove, Shrine, Favour, Dream, NamePlace }

impl Tool {
    /// What the player is asked to do while it is held.
    pub(crate) fn prompt(self) -> &'static str {
        match self {
            Tool::Bless => "Click the ground to bless (six cells round; again on blessed ground lifts it). Esc: put it down.",
            Tool::Forbid => "Click the ground to forbid (six cells round; again on it lifts the ban). Esc: put it down.",
            Tool::Hall => "Click where the hall stone goes: the hut is raised beside it. Esc: put it down.",
            Tool::Grove => "Click a grove to keep: its trees are spared the axe. Esc: put it down.",
            Tool::Shrine => "Click where the shrine stands. Esc: put it down.",
            Tool::Favour => "Click a settler on the map to favour them. Esc: put it down.",
            Tool::Dream => "Click a settler on the map to send them a dream. Esc: put it down.",
            Tool::NamePlace => "Click the place to name. Esc: put it down.",
        }
    }
}

/// What a click on the interface asks for (the viewer carries it out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Action {
    Tab(Tab),
    Close,
    /// Select a settler and bring the camera to them (a second click opens their sheet).
    Select(usize),
    Sheet(usize),
    Back,
    Arm(Tool),
    Bell,
    NameCamp,
    Favour(usize),
    DreamFor(usize),
    SendDream(usize, Dream),
    NoDream,
    Speed(u32),
    Pause,
    Skip,
    /// The annals' filter: 0 moments, 1 the whole log, 2 the patron's own acts.
    Filter(u8),
    Event(crate::history::EventId),
    Refugees(bool),
    /// Bring the camera to a place on the surface.
    Look(u16, u16),
    /// Scroll the open leaf to its top.
    Top,
}

/// The interface's own state, kept by the viewer between frames.
pub(crate) struct UiState {
    pub open: Option<Tab>,
    pub sheet: Option<usize>,
    pub selected: Option<usize>,
    /// Scroll of each leaf (the five tabs, then the sheet), and the height of what was drawn.
    pub scroll: [f32; 6],
    pub content: [f32; 6],
    pub annals: u8,
    pub tool: Option<Tool>,
    /// The clock was stopped by opening a sheet: closing it starts it again.
    pub paused_by_sheet: bool,
}

impl Default for UiState {
    fn default() -> Self { UiState { open: None, sheet: None, selected: None, scroll: [0.0; 6], content: [0.0; 6], annals: 0, tool: None, paused_by_sheet: false } }
}

impl UiState {
    fn leaf(&self) -> usize { if self.sheet.is_some() { 5 } else { self.open.map_or(0, |t| t.index()) } }
    /// Scroll the open leaf by `dy` pixels (kept within what it holds).
    pub(crate) fn scroll_by(&mut self, dy: f32, view_h: f32) {
        let k = self.leaf();
        self.scroll[k] = (self.scroll[k] + dy).clamp(0.0, (self.content[k] - view_h + 24.0).max(0.0));
    }
}

/// The key bar's place (the bottom of the window).
pub(crate) fn bar_rect(w: usize, h: usize) -> Rect { Rect { x: 10, y: h.saturating_sub(34), w: w.saturating_sub(20), h: 26 } }

/// The panel's width for a window `w` wide.
fn panel_w(w: usize) -> usize { ((w as f32 * 0.4) as usize).clamp(400, 600).min(w.saturating_sub(20)) }

/// The tabs (top right): always shown.
pub(crate) fn tabs_rect(w: usize, _h: usize) -> Rect {
    let pw = panel_w(w);
    Rect { x: w.saturating_sub(pw + 10), y: 10, w: pw, h: 28 }
}

/// The panel's body, below its tabs and above the key bar.
pub(crate) fn panel_rect(w: usize, h: usize) -> Rect {
    let t = tabs_rect(w, h);
    Rect { x: t.x, y: t.y + t.h, w: t.w, h: h.saturating_sub(t.y + t.h + 44) }
}

/// Pixels the open panel takes on the right of the window (0 when closed).
pub(crate) fn reserve_right(ui: &UiState, w: usize) -> usize { if ui.open.is_some() { panel_w(w) + 20 } else { 0 } }

/// The scrolling part of the panel.
pub(crate) fn body_rect(w: usize, h: usize) -> Rect {
    let p = panel_rect(w, h);
    Rect { x: p.x + 6, y: p.y + 6, w: p.w.saturating_sub(12), h: p.h.saturating_sub(12) }
}

/// Whether a point is on the interface (the tabs, the open panel, the key bar): clicks and
/// drags there do not reach the map.
pub(crate) fn over_ui(ui: &UiState, w: usize, h: usize, p: (f32, f32)) -> bool {
    let tabs = tabs_rect(w, h);
    let tab_strip = Rect { x: tabs.x + tabs.w.saturating_sub(tab_strip_w()), y: tabs.y, w: tab_strip_w().min(tabs.w), h: tabs.h };
    bar_rect(w, h).contains(p.0, p.1) || (if ui.open.is_some() { tabs.contains(p.0, p.1) || panel_rect(w, h).contains(p.0, p.1) } else { tab_strip.contains(p.0, p.1) })
}

fn tab_strip_w() -> usize { Tab::ALL.iter().map(|t| tab_w(*t) as usize + 4).sum::<usize>() + 4 }
fn tab_w(t: Tab) -> f32 { fonts::width(t.word(), Face::SmallCaps, 15.0, 0.5) + fonts::width(t.key(), Face::Italic, 12.0, 0.0) + 26.0 }

/// A hit area and what it asks for.
pub(crate) struct Hit { pub rect: Rect, pub action: Action }

/// A small inked button: the key in red italic, then the word. Returns its width.
#[allow(clippy::too_many_arguments)]
fn button(buf: &mut [u32], w: usize, h: usize, x: f32, y: f32, key: &str, label: &str, enabled: bool, active: bool, hover: bool) -> (f32, Rect) {
    let kw = if key.is_empty() { 0.0 } else { fonts::width(key, Face::Italic, SMALL, 0.0) + 5.0 };
    let bw = (kw + fonts::width(label, Face::Roman, SMALL, 0.0) + 14.0).ceil();
    let r = Rect { x: x.max(0.0) as usize, y: y.max(0.0) as usize, w: bw as usize, h: 21 };
    if r.x + r.w >= w || r.y + r.h >= h { return (bw, r); }
    if active || hover {
        let wash = if active { ui::mix(PAPER, GOLD, 0.35) } else { ui::mix(PAPER, GOLD, 0.15) };
        for yy in r.y + 1..r.y + r.h - 1 { for xx in r.x + 1..r.x + r.w - 1 { let k = yy * w + xx; buf[k] = ui::mix(buf[k], wash, 0.8); } }
    }
    ui::outline(buf, w, r, if active { INK } else { ui::mix(INK_FADED, PAPER, 0.25) });
    let ink = if enabled { INK } else { ui::mix(INK_FADED, PAPER, 0.45) };
    if kw > 0.0 { fonts::draw(buf, w, h, x + 7.0, y + 3.0, key, Face::Italic, SMALL, 0.0, if enabled { RUBRIC } else { ink }, None); }
    fonts::draw(buf, w, h, x + 7.0 + kw, y + 3.0, label, Face::Roman, SMALL, 0.0, ink, None);
    (bw, r)
}

// ----------------------------------------------------------------------------------------------
// A scrolling leaf, drawn into its own buffer (so nothing spills over the panel's edges).

struct Pane {
    buf: Vec<u32>,
    w: usize,
    h: usize,
    y: f32,
    scroll: f32,
    left: f32,
    right: f32,
    hits: Vec<(f32, f32, f32, f32, Action)>,
    mouse: Option<(f32, f32)>,
    /// The next ledger line draws no glyph (an animal's sprite is drawn instead).
    no_glyph: bool,
}

impl Pane {
    fn on(&self, y: f32, hgt: f32) -> bool { let s = y - self.scroll; s + hgt >= -2.0 && s <= self.h as f32 + 2.0 }
    fn hover(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse.map_or(false, |(mx, my)| mx >= x && mx < x + w && my >= y - self.scroll && my < y - self.scroll + h)
    }
    fn write(&mut self, x: f32, y: f32, text: &str, face: Face, px: f32, color: u32) {
        if self.on(y, px * 1.5) { fonts::draw(&mut self.buf, self.w, self.h, x, y - self.scroll, text, face, px, 0.0, color, None); }
    }
    fn write_tracked(&mut self, x: f32, y: f32, text: &str, face: Face, px: f32, tracking: f32, color: u32) {
        if self.on(y, px * 1.5) { fonts::draw(&mut self.buf, self.w, self.h, x, y - self.scroll, text, face, px, tracking, color, None); }
    }
    fn hit(&mut self, x: f32, y: f32, w: f32, h: f32, a: Action) { if self.on(y, h) { self.hits.push((x, y - self.scroll, w, h, a)); } }
    fn wash(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32, a: f32) {
        let (x0, x1) = (x.max(0.0) as usize, ((x + w) as usize).min(self.w));
        let (y0, y1) = ((y - self.scroll).max(0.0) as usize, ((y - self.scroll + h).max(0.0) as usize).min(self.h));
        for yy in y0..y1 { for xx in x0..x1 { let k = yy * self.w + xx; self.buf[k] = ui::mix(self.buf[k], color, a); } }
    }
    fn rule(&mut self, x0: f32, x1: f32, y: f32, color: u32) {
        let sy = y - self.scroll;
        if sy >= 0.0 && (sy as usize) < self.h { let (a, b) = (x0.max(0.0) as usize, (x1 as usize).min(self.w)); if b > a { ui::hline(&mut self.buf, self.w, a, b, sy as usize, color); } }
    }
    fn width(&self) -> f32 { self.right - self.left }

    /// A section's heading: rubric small capitals with a rule after them.
    fn heading(&mut self, text: &str) {
        self.y += 10.0;
        let tw = fonts::width(text, Face::SmallCaps, 16.0, 0.5);
        let (l, y) = (self.left, self.y);
        self.write_tracked(l, y, text, Face::SmallCaps, 16.0, 0.5, RUBRIC);
        let r = self.right;
        self.rule(l + tw + 8.0, r, y + 11.0, ui::mix(INK_FADED, PAPER, 0.3));
        self.y += 24.0;
    }

    /// Wrapped text; a link when `action` is given (sea ink, the whole block clickable).
    fn para(&mut self, text: &str, face: Face, px: f32, color: u32, indent: f32, action: Option<Action>) {
        let line_h = (px * 1.3).round();
        let lines = fonts::wrap(text, face, px, self.width() - indent - 4.0);
        let top = self.y;
        let total = lines.len() as f32 * line_h;
        let (l, wd) = (self.left, self.width());
        if action.is_some() && self.hover(l, top, wd, total) { self.wash(l - 4.0, top - 1.0, wd + 8.0, total + 2.0, GOLD, 0.12); }
        let color = if action.is_some() { SEA } else { color };
        for (k, line) in lines.iter().enumerate() {
            let x = self.left + indent + if k > 0 { 12.0 } else { 0.0 };
            let y = self.y;
            self.write(x, y, line, face, px, color);
            self.y += line_h;
        }
        if let Some(a) = action { self.hit(l - 4.0, top, wd + 8.0, total, a); }
    }

    /// A label on the left and a value on the right of one line.
    fn pair(&mut self, label: &str, value: &str, color: u32) {
        let (l, r, y) = (self.left, self.right, self.y);
        self.write(l, y, label, Face::Italic, SMALL, SOFT);
        let lw = fonts::width(label, Face::Italic, SMALL, 0.0) + 12.0;
        let room = r - l - lw;
        let v = fit(value, Face::Roman, BODY, room);
        let vw = fonts::width(&v, Face::Roman, BODY, 0.0);
        self.write(r - vw, y - 2.0, &v, Face::Roman, BODY, color);
        self.y += 19.0;
    }

    /// A line in the ledger: the thing's glyph, its name, a count on the right, a note under it.
    fn ledger(&mut self, g: Glyph, tint: Option<[f32; 3]>, label: &str, count: &str, note: &str, action: Option<Action>) {
        let (l, r, top) = (self.left, self.right, self.y);
        let note_lines = if note.is_empty() { Vec::new() } else { fonts::wrap(note, Face::Italic, SMALL, r - l - 34.0) };
        let hgt = 22.0 + note_lines.len() as f32 * 16.0;
        if action.is_some() && self.hover(l, top, r - l, hgt) { self.wash(l - 4.0, top - 2.0, r - l + 8.0, hgt, GOLD, 0.12); }
        if self.on(top, 22.0) && !self.no_glyph {
            let s = self.scroll;
            glyphs::draw_u32(&mut self.buf, self.w, self.h, g, l + 10.0, top + 9.0 - s, 17.0, tint);
        }
        let cw = fonts::width(count, Face::Roman, BODY, 0.0);
        let name = fit(label, Face::Roman, BODY, r - l - 34.0 - cw - 10.0);
        self.write(l + 26.0, top, &name, Face::Roman, BODY, if action.is_some() { SEA } else { INK });
        self.write(r - cw, top, count, Face::Roman, BODY, INK);
        self.y += 21.0;
        for line in note_lines { let y = self.y; self.write(l + 26.0, y, &line, Face::Italic, SMALL, SOFT); self.y += 16.0; }
        if let Some(a) = action { self.hit(l - 4.0, top - 2.0, r - l + 8.0, hgt, a); }
        self.y += 1.0;
    }

    /// A ledger line for a living animal: its bestiary sprite (`beasts.rs`) instead of a glyph.
    fn ledger_beast(&mut self, name: &str, label: &str, count: &str, note: &str, action: Option<Action>) {
        let (l, top) = (self.left, self.y);
        if self.on(top, 22.0) {
            let s = self.scroll;
            let look = super::beasts::of_name(name);
            let (w, h) = (self.w, self.h);
            let buf = &mut self.buf;
            let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| {
                if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
                let k = y as usize * w + x as usize;
                buf[k] = ui::mix(buf[k], super::ink::pack(c), a.clamp(0.0, 1.0));
            };
            super::beasts::draw(&mut put, &look, l + 10.0, top + 16.0 - s, 26.0, false, super::beasts::Pose::Stand, 1.0);
        }
        // The rest as any ledger line, with no glyph.
        self.no_glyph = true;
        self.ledger(Glyph::Work, None, label, count, note, action);
        self.no_glyph = false;
    }

    /// A measure: a label, an inked bar filled to `frac`, and a word after it.
    fn measure(&mut self, label: &str, frac: f32, word: &str, color: u32) {
        let (l, y) = (self.left, self.y);
        self.write(l, y + 1.0, label, Face::Italic, SMALL, SOFT);
        let bx = l + 104.0;
        let bw = (self.width() * 0.38).clamp(90.0, 200.0);
        self.bar_at(bx, y + 4.0, bw, 9.0, frac, color);
        self.write(bx + bw + 10.0, y, word, Face::Roman, SMALL + 1.0, INK);
        self.y += 18.0;
    }

    fn bar_at(&mut self, x: f32, y: f32, bw: f32, bh: f32, frac: f32, color: u32) {
        let sy = y - self.scroll;
        if sy < -bh || sy > self.h as f32 { return; }
        let r = Rect { x: x as usize, y: sy.max(0.0) as usize, w: bw as usize, h: bh as usize };
        if r.y + r.h >= self.h || r.x + r.w >= self.w { return; }
        let fill = (frac.clamp(0.0, 1.0) * (r.w as f32 - 2.0)) as usize;
        for yy in r.y + 1..r.y + r.h - 1 {
            for xx in r.x + 1..r.x + r.w - 1 {
                let k = yy * self.w + xx;
                // Filled: a wash with hatching; empty: bare paper.
                if xx - r.x - 1 < fill { self.buf[k] = if (xx + yy) % 4 == 0 { ui::mix(color, INK, 0.35) } else { ui::mix(self.buf[k], color, 0.75) }; }
            }
        }
        ui::outline(&mut self.buf, self.w, r, INK_FADED);
    }

    /// A row of buttons.
    fn buttons(&mut self, items: &[(&str, &str, Action, bool, bool)]) {
        let mut x = self.left;
        let y = self.y;
        for &(key, label, action, enabled, active) in items {
            let kw = if key.is_empty() { 0.0 } else { fonts::width(key, Face::Italic, SMALL, 0.0) + 5.0 };
            let bw = kw + fonts::width(label, Face::Roman, SMALL, 0.0) + 14.0;
            if x + bw > self.right { x = self.left; self.y += 26.0; }
            let sy = self.y - self.scroll;
            let hov = self.hover(x, self.y, bw, 21.0);
            if self.on(self.y, 21.0) && sy >= 0.0 {
                let (wd, hh) = (self.w, self.h);
                button(&mut self.buf, wd, hh, x, sy, key, label, enabled, active, hov && enabled);
            }
            if enabled { let yy = self.y; self.hit(x, yy, bw, 21.0, action); }
            x += bw + 6.0;
        }
        let _ = y;
        self.y += 28.0;
    }
}

/// Cut `text` to fit `max_w`, ending in an ellipsis.
fn fit(text: &str, face: Face, px: f32, max_w: f32) -> String {
    if fonts::width(text, face, px, 0.0) <= max_w { return text.to_string(); }
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if fonts::width(&out, face, px, 0.0) + fonts::width("...", face, px, 0.0) > max_w { out.pop(); break; }
    }
    format!("{}...", out.trim_end())
}

pub(crate) fn cap_pub(s: &str) -> String { cap(s) }

fn cap(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }

fn plural(n: usize, one: &str, many: &str) -> String { format!("{} {}", n, if n == 1 { one } else { many }) }

// ----------------------------------------------------------------------------------------------
// What a settler is, in short.

/// Where they are: on the surface, N levels down, up the tower, away.
fn whereabouts(colony: &Colony, i: usize) -> String {
    let s = &colony.settlers[i];
    if !s.alive { return if s.mind.left { "gone".into() } else { "dead".into() }; }
    if s.away_until > colony.clock.day() { return "away".into(); }
    let w = colony.map.width;
    let ground = colony.map.surface_z[colony.camp.1 as usize * w + colony.camp.0 as usize];
    let here = colony.map.surface_z[s.pos.1 as usize * w + s.pos.0 as usize];
    if !colony.below(i) { return "on the surface".into(); }
    if s.z > here { format!("{} up", s.z - here) } else { format!("level {} ({} down)", s.z, ground - s.z) }
}

fn health(colony: &Colony, i: usize) -> (String, u32) {
    let s = &colony.settlers[i];
    let now = colony.clock.tick;
    let fresh: Vec<&crate::colony::fight::Wound> = s.wounds.iter().filter(|w| w.healed_at > now).collect();
    if fresh.iter().any(|w| w.infected) { return ("a festering wound".into(), RUBRIC); }
    if let Some(w) = fresh.first() { return (if fresh.len() > 1 { format!("{} wounds", fresh.len()) } else { w.word() }, RUBRIC); }
    if s.ill_until > now { return ("ill".into(), RUBRIC); }
    if s.exposure > 0.7 { return ("chilled".into(), GOLD); }
    if s.hunger > 0.85 { return ("starving".into(), RUBRIC); }
    ("well".into(), SOFT)
}

fn mood_of(s: &crate::colony::Settler) -> (String, u32) {
    if let Some((b, _)) = s.mind.broken { return (format!("in {}", b.word()), RUBRIC); }
    let m = crate::colony::mind::mood(s.mind.stress);
    let c = if s.mind.stress < -0.2 { MOSS } else if s.mind.stress < 0.4 { INK } else if s.mind.stress < 0.8 { GOLD } else { RUBRIC };
    (m.to_string(), c)
}

/// "41, the camp's builder" / "12, a survivor of Ripu".
fn standing_of(colony: &Colony, i: usize) -> String {
    let s = &colony.settlers[i];
    let age = s.past.as_ref().map(|p| p.age.to_string());
    let what = match (s.role, &s.office) {
        (_, Some(o)) => o.to_lowercase(),
        (Some(k), None) => format!("the camp's {}", crate::colony::ROLES[k]),
        (None, None) => s.visitor.clone().or_else(|| s.past.as_ref().map(|p| p.calling.clone())).unwrap_or_else(|| "a wanderer".into()),
    };
    match age { Some(a) => format!("{}, {}", a, what), None => what }
}

/// An opinion in words.
fn regard_word(o: i32) -> Option<&'static str> {
    Some(match o {
        o if o >= 20 => "a close friend",
        o if o >= 10 => "a friend",
        o if o >= 4 => "on good terms",
        o if o <= -8 => "an enemy",
        o if o <= -3 => "at odds",
        _ => return None,
    })
}

// ----------------------------------------------------------------------------------------------
// The leaves.

fn settlers_leaf(p: &mut Pane, colony: &Colony, ui: &UiState, history: Option<&crate::history::world_state::WorldHistory>) {
    let day = colony.clock.day();
    let living: Vec<usize> = (0..colony.settlers.len()).filter(|&i| colony.settlers[i].alive && colony.settlers[i].guest_until == 0 && colony.settlers[i].away_until <= day).collect();
    let guests: Vec<usize> = (0..colony.settlers.len()).filter(|&i| colony.settlers[i].alive && colony.settlers[i].guest_until > 0).collect();
    let away: Vec<usize> = (0..colony.settlers.len()).filter(|&i| { let s = &colony.settlers[i]; s.away_until > day && s.guest_until == 0 && !s.mind.left }).collect();
    let gone: Vec<usize> = (0..colony.settlers.len()).filter(|&i| { let s = &colony.settlers[i]; !s.alive && s.away_until <= day && !(s.mind.left && s.guest_until > 0) }).collect();
    p.heading(&format!("The company: {} in the camp", living.len()));
    p.para("Click a row to find them on the map; click again to read them.", Face::Italic, SMALL, SOFT, 0.0, None);
    p.y += 2.0;
    for i in living { settler_row(p, colony, ui, history, i); }
    if !away.is_empty() {
        p.heading("Away");
        for i in away { settler_row(p, colony, ui, history, i); }
    }
    if !guests.is_empty() {
        p.heading("Guests");
        for i in guests { settler_row(p, colony, ui, history, i); }
    }
    if !gone.is_empty() {
        p.heading(&format!("The dead and the gone: {}", gone.len()));
        for i in gone {
            let s = &colony.settlers[i];
            let fate = if s.mind.left { "left the camp for good".to_string() } else {
                colony.marks.iter().find(|m| m.title.contains(&s.name) && m.day > 0).map(|m| format!("died on day {}; {}", m.day, if matches!(m.kind, crate::colony::MarkKind::Grave) { "buried at the camp's edge" } else { "remembered by a stone" }))
                    .unwrap_or_else(|| "dead; laid in the tombs below or lost".into())
            };
            let (l, top) = (p.left, p.y);
            p.write(l, top, &s.name, Face::SmallCaps, BODY, SOFT);
            let nw = fonts::width(&s.name, Face::SmallCaps, BODY, 0.0);
            let rest = fit(&format!("  {}", fate), Face::Italic, SMALL, p.width() - nw - 6.0);
            p.write(l + nw, top + 1.0, &rest, Face::Italic, SMALL, SOFT);
            p.hit(l - 4.0, top - 2.0, p.width() + 8.0, 20.0, Action::Sheet(i));
            p.y += 20.0;
        }
    }
}

fn settler_row(p: &mut Pane, colony: &Colony, ui: &UiState, history: Option<&crate::history::world_state::WorldHistory>, i: usize) {
    let s = &colony.settlers[i];
    let (l, r, top) = (p.left, p.right, p.y);
    let row_h = 46.0;
    let sel = ui.selected == Some(i);
    if sel { p.wash(l - 4.0, top - 2.0, r - l + 8.0, row_h, GOLD, 0.22); }
    else if p.hover(l, top, r - l, row_h) { p.wash(l - 4.0, top - 2.0, r - l + 8.0, row_h, GOLD, 0.1); }
    if p.on(top, row_h) {
        let face = super::portraits::of_settler(s, history, super::viewer::wounded_in(colony, &s.name));
        let sc = p.scroll;
        super::portraits::draw(&mut p.buf, p.w, p.h, l as i64, (top - sc) as i64, 38, &face);
    }
    let x = l + 46.0;
    // The patron's marks on them: favourite, a dream sent.
    let mut name = s.name.clone();
    if colony.patron.favourite == Some(i) { name.push_str(" *"); }
    p.write(x, top, &name, Face::SmallCaps, BODY + 1.0, if sel { RUBRIC } else { INK });
    let nw = fonts::width(&name, Face::SmallCaps, BODY + 1.0, 0.0);
    let (mood, mc) = mood_of(s);
    let mw = fonts::width(&mood, Face::Italic, SMALL, 0.0);
    let stand = fit(&format!("  {}", standing_of(colony, i)), Face::Italic, SMALL, r - x - nw - mw - 70.0);
    p.write(x + nw, top + 2.0, &stand, Face::Italic, SMALL, SOFT);
    // Their spirits: a short bar and the word.
    let frac = 1.0 - ((s.mind.stress + 1.0) / 2.4).clamp(0.0, 1.0);
    p.bar_at(r - mw - 52.0, top + 5.0, 44.0, 8.0, frac, if mc == RUBRIC { RUBRIC } else if mc == GOLD { GOLD } else { MOSS });
    p.write(r - mw, top + 1.0, &mood, Face::Italic, SMALL, mc);
    // What they are doing and why; where; how they are.
    let (hw, hc) = health(colony, i);
    let place = whereabouts(colony, i);
    let tail = format!("{}  \u{b7}  {}", place, hw);
    let tw = fonts::width(&tail, Face::Italic, SMALL, 0.0);
    let doing = if s.alive { format!("{} - {}", cap(s.job.verb()), s.why) } else { "dead".into() };
    let doing = fit(&doing, Face::Roman, SMALL + 1.0, r - x - tw - 12.0);
    p.write(x, top + 21.0, &doing, Face::Roman, SMALL + 1.0, INK);
    p.write(r - tw, top + 22.0, &place, Face::Italic, SMALL, SOFT);
    let pw = fonts::width(&format!("{}  \u{b7}  ", place), Face::Italic, SMALL, 0.0);
    p.write(r - tw + pw, top + 22.0, &hw, Face::Italic, SMALL, hc);
    p.rule(x, r, top + row_h - 4.0, ui::mix(PAPER, INK_FADED, 0.35));
    p.hit(l - 4.0, top - 2.0, r - l + 8.0, row_h, if sel { Action::Sheet(i) } else { Action::Select(i) });
    p.y += row_h + 2.0;
}

fn sheet_leaf(p: &mut Pane, colony: &Colony, i: usize, history: Option<&crate::history::world_state::WorldHistory>) {
    let s = &colony.settlers[i];
    let (l, r) = (p.left, p.right);
    p.buttons(&[("Esc", "back to the company", Action::Back, true, false), ("", "find on the map", Action::Select(i), s.alive, false)]);
    // The face, the name and what they are.
    let top = p.y;
    if p.on(top, 84.0) {
        let face = super::portraits::of_settler(s, history, super::viewer::wounded_in(colony, &s.name));
        let sc = p.scroll;
        super::portraits::draw(&mut p.buf, p.w, p.h, (r - 80.0) as i64, (top - sc) as i64, 80, &face);
    }
    let save_r = p.right;
    p.right = r - 92.0;
    p.write_tracked(l, top, &s.name, Face::SmallCaps, 24.0, 0.8, RUBRIC);
    p.y += 32.0;
    let age = s.past.as_ref().map_or(30, |pp| pp.age);
    let race = crate::persona::template(&s.persona.race).word.clone();
    let kind = format!("{} {}, aged {}{}", if race.starts_with(['a', 'e', 'i', 'o', 'u']) { "An" } else { "A" }, race, age, s.past.as_ref().map(|pp| format!("; {}", pp.calling)).unwrap_or_default());
    p.para(&kind, Face::Italic, SMALL + 1.0, SOFT, 0.0, None);
    let mut titles = Vec::new();
    if let Some(k) = s.role { titles.push(format!("The camp's {}", crate::colony::ROLES[k])); }
    if let Some(o) = &s.office { titles.push(o.clone()); }
    if colony.patron.favourite == Some(i) { titles.push("The patron's favourite".into()); }
    if colony.healer == Some(i) { titles.push("Tends the wounded".into()); }
    if !titles.is_empty() { p.para(&titles.join("  \u{b7}  "), Face::Roman, BODY, INK, 0.0, None); }
    if s.alive { p.para(&format!("Now {}: {}", s.job.verb(), s.why), Face::Roman, BODY, INK, 0.0, None); }
    p.y = p.y.max(top + 88.0);
    p.right = save_r;
    // The patron's hand on them.
    if s.alive {
        let spent = colony.patron.favour == 0;
        let dreaming = colony.patron.dreams.iter().any(|d| d.0 == i && d.2 > colony.clock.tick);
        p.buttons(&[("G", "favour them", Action::Favour(i), !spent && colony.patron.favourite != Some(i), colony.patron.favourite == Some(i)),
                    ("R", "send a dream", Action::DreamFor(i), !spent, dreaming)]);
    }
    if s.alive {
        p.heading("Needs");
        let (mood, mc) = mood_of(s);
        p.measure("fed", 1.0 - s.hunger, if s.hunger > 0.85 { "starving" } else if s.hunger > 0.6 { "hungry" } else { "fed" }, if s.hunger > 0.6 { RUBRIC } else { MOSS });
        p.measure("rested", 1.0 - s.fatigue, if s.fatigue > 0.8 { "exhausted" } else if s.fatigue > 0.5 { "tired" } else { "rested" }, if s.fatigue > 0.5 { GOLD } else { MOSS });
        p.measure("warm", 1.0 - s.exposure, if s.exposure > 0.7 { "chilled through" } else if s.exposure > 0.4 { "cold" } else { "warm" }, if s.exposure > 0.4 { SEA } else { MOSS });
        p.measure("spirits", 1.0 - ((s.mind.stress + 1.0) / 2.4).clamp(0.0, 1.0), &mood, if mc == INK { MOSS } else { mc });
        let drink = if s.last_drink > 0 { format!("last cup on day {}", s.last_drink) } else { "has not had a cup here".into() };
        p.para(&format!("{}; {}.", cap(&drink), if s.rationed { "on rations" } else { "eats from the store" }), Face::Italic, SMALL, SOFT, 0.0, None);
    }
    p.heading("Health");
    let now = colony.clock.tick;
    let mut any = false;
    if s.ill_until > now { p.para(&format!("Ill, abed until day {}", s.ill_until / crate::colony::TICKS_PER_DAY + 1), Face::Roman, BODY, RUBRIC, 0.0, None); any = true; }
    for w in &s.wounds {
        let state = if w.healed_at <= now { "healed".to_string() } else if w.infected { "festering".into() } else { format!("healing, until day {}", w.healed_at / crate::colony::TICKS_PER_DAY + 1) };
        p.para(&format!("{} from {}: {}", cap(&w.word()), w.from, state), Face::Roman, BODY, if w.healed_at > now { RUBRIC } else { INK }, 0.0, None);
        any = true;
    }
    if !any { p.para(if s.alive { "Whole and well." } else { "Dead." }, Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
    p.heading("Hands");
    let words = ["foraging", "fishing", "felling and quarrying", "carrying", "building"];
    for k in 0..5 {
        let v = s.skill[k];
        let word = if v >= 0.8 { "a master" } else if v >= 0.55 { "skilled" } else if v >= 0.3 { "able" } else if v >= 0.12 { "a novice" } else { "green" };
        p.measure(words[k], v, &format!("{}{}", word, if s.role == Some(k) { ", the camp's" } else { "" }), if s.role == Some(k) { GOLD } else { SEA });
    }
    if s.drill >= 0.05 { p.measure("the spear", s.drill / 0.6, if s.drill >= 0.4 { "a seasoned hand" } else if s.drill >= 0.2 { "steady" } else { "green" }, RUBRIC); }
    if let Some(&v) = colony.industry.smith.get(&i) { if v > 0.05 { p.measure("the forge", v, if v >= 0.8 { "a master smith" } else if v >= 0.4 { "a smith" } else { "learning" }, GOLD); } }
    p.para(&format!("{} laid in the camp's works.", plural(s.loads_laid as usize, "load", "loads")), Face::Italic, SMALL, SOFT, 0.0, None);
    if !s.persona.race.is_empty() {
        p.heading("Body and mind");
        if let Some(g) = s.persona.gifts_text() { p.para(&g, Face::Roman, BODY, INK, 0.0, None); }
        if let Some(c) = s.persona.character_text() { p.para(&c, Face::Roman, BODY, INK, 0.0, None); }
        if let Some(v) = s.persona.values_text() { p.para(&v, Face::Roman, BODY, INK, 0.0, None); }
        p.para(&s.persona.likes_text(), Face::Italic, BODY, SOFT, 0.0, None);
        p.para(&s.persona.looks_text(&s.name, age), Face::Italic, BODY, SOFT, 0.0, None);
    }
    if !s.mind.thoughts.is_empty() || !s.mind.scars.is_empty() {
        p.heading("Thoughts");
        for t in s.mind.thoughts.iter().rev().take(7) {
            p.para(&format!("Day {}: {}", t.tick / crate::colony::TICKS_PER_DAY + 1, t.text), Face::Roman, SMALL + 1.0, if t.weight < 0.0 { INK } else { SOFT }, 0.0, None);
        }
        for t in s.mind.scars.iter().rev().take(2) {
            p.para(&format!("Still weighs on them (day {}): {}", t.tick / crate::colony::TICKS_PER_DAY + 1, t.text), Face::Italic, SMALL + 1.0, RUBRIC, 0.0, None);
        }
    }
    p.heading("Kin and company");
    let mut said = false;
    if let Some(f) = colony.family_of(i) { p.para(&cap(&f), Face::Roman, BODY, INK, 0.0, None); said = true; }
    if let Some(pet) = colony.pet_of(i) { p.para(&cap(&pet), Face::Roman, BODY, INK, 0.0, None); said = true; }
    if let Some(g) = colony.guild_of(i) { p.para(&g, Face::Roman, BODY, INK, 0.0, None); said = true; }
    let mut ties: Vec<(i32, usize)> = (0..colony.settlers.len()).filter(|&j| j != i && colony.settlers[j].alive).map(|j| (colony.opinion(i, j), j)).filter(|(o, _)| regard_word(*o).is_some()).collect();
    ties.sort_by_key(|&(o, j)| (std::cmp::Reverse(o), j));
    let mut shown: Vec<(i32, usize)> = ties.iter().copied().filter(|t| t.0 > 0).take(4).collect();
    shown.extend(ties.iter().rev().copied().filter(|t| t.0 < 0).take(3));
    for (o, j) in shown {
        let line = format!("{}: {}", colony.settlers[j].name, regard_word(o).unwrap_or(""));
        p.para(&line, Face::Roman, BODY, if o < 0 { RUBRIC } else { INK }, 0.0, Some(Action::Select(j)));
        said = true;
    }
    if !said { p.para("No kin here, and no one close yet.", Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
    p.heading("Arms and dress");
    let arm = colony.arm_of(i).map(|a| format!("Bears {}", a.kind));
    let armour = colony.armour_of(i).map(|a| format!("Wears {}", a.kind));
    let clothes = colony.clothes.get(&i).map(|&d| format!("Clothes made on day {}", d));
    let lines: Vec<String> = [arm, armour, clothes].into_iter().flatten().collect();
    if lines.is_empty() { p.para("Unarmed; in the clothes they came in.", Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
    for line in lines { p.para(&line, Face::Roman, BODY, INK, 0.0, None); }
    let made: Vec<String> = colony.works.iter().filter(|w| w.maker == i).map(|w| format!("{} (day {}{})", cap(&w.describe()), w.day, if w.traded { ", sold" } else { "" })).collect();
    if !s.deeds.is_empty() || !made.is_empty() || !s.made.is_empty() {
        p.heading("Deeds and works");
        for d in &s.deeds { p.para(&cap(d), Face::Roman, BODY, INK, 0.0, None); }
        for m in made.iter().rev().take(5) { p.para(m, Face::Italic, BODY, SOFT, 0.0, None); }
        if made.is_empty() { for m in s.made.iter().rev().take(4) { p.para(&cap(m), Face::Italic, BODY, SOFT, 0.0, None); } }
    }
    let dream = colony.dream_line(i);
    let vow = colony.vow_of(i);
    let sent = colony.patron.dreams.iter().find(|d| d.0 == i && d.2 > colony.clock.tick).map(|d| format!("Tonight they dream of {} (the patron's gift)", d.1.word()));
    if dream.is_some() || vow.is_some() || sent.is_some() {
        p.heading("Dreams and vows");
        for line in [sent, dream, vow].into_iter().flatten() { p.para(&line, Face::Roman, BODY, INK, 0.0, None); }
    }
    if let Some(pp) = &s.past {
        if !pp.lines.is_empty() || pp.feeling.is_some() {
            p.heading("Before the camp");
            for (text, ev) in &pp.lines {
                let link = ev.filter(|e| history.map_or(false, |h| h.chronicle.get(*e).is_some())).map(Action::Event);
                p.para(text, Face::Roman, BODY, INK, 0.0, link);
            }
            if let Some((text, _)) = &pp.feeling { p.para(&cap(text), Face::Italic, BODY, SOFT, 0.0, None); }
            if let Some((rel, god)) = &pp.faith { p.para(&format!("Of the faith of {} ({})", god, rel), Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
        }
    }
    if let Some(v) = &s.visitor { p.para(&format!("Came as {}{}", v, if s.guest_until == 0 { ", and stayed" } else { "; a guest" }), Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
}

fn stocks_leaf(p: &mut Pane, colony: &Colony) {
    let stored = |s: Stuff| colony.items.iter().filter(|it| it.stored && it.what == s).count();
    let loose = super::local_ink::loose_piles(colony);
    let loose_of = |pred: &dyn Fn(Stuff) -> bool| -> (usize, Vec<(u16, u16)>) {
        let mut n = 0;
        let mut at: Vec<(u16, u16)> = Vec::new();
        for (c, s, k) in &loose { if pred(*s) { n += k; if !at.contains(c) { at.push(*c); } } }
        (n, at)
    };
    let places = |at: &[(u16, u16)]| -> String {
        if at.is_empty() { return String::new(); }
        let shown: Vec<String> = at.iter().take(3).map(|c| format!("{},{}", c.0, c.1)).collect();
        format!("at {}{}", shown.join("; "), if at.len() > 3 { format!(" and {} more places", at.len() - 3) } else { String::new() })
    };
    let food = colony.food_stored() as usize;
    p.heading(&format!("Food and drink: {} meals", food));
    p.para(&format!("About {:.0} days of food for {} mouths; the store keeps food {} days{}.", colony.days_of_food(), colony.alive(), colony.keeps_days(),
        if colony.rations { "; on half rations" } else { "" }), Face::Italic, SMALL + 1.0, SOFT, 0.0, None);
    for s in [Stuff::Berries, Stuff::Fish, Stuff::Meat, Stuff::Grain, Stuff::Fungus, Stuff::Provisions] {
        let n = stored(s);
        if n == 0 { continue; }
        let note = match s { Stuff::Provisions => "brought, given or paid in", Stuff::Grain => "reaped from the field", Stuff::Fungus => "grown under the rock", Stuff::Meat => "from the hunt and the pen", Stuff::Fish => "caught and carried home", Stuff::Berries => "gathered from the bushes", _ => "" };
        p.ledger(Glyph::of_stuff(s), None, &cap(s.plural()), &n.to_string(), &format!("in the store by the fire; {}", note), Some(Action::Look(colony.camp.0, colony.camp.1)));
    }
    let (n, at) = loose_of(&|s| !matches!(s, Stuff::Timber | Stuff::Stone));
    if n > 0 { p.ledger(Glyph::Berries, None, "Food lying where it was got", &n.to_string(), &format!("to be carried in; {}", places(&at)), at.first().map(|c| Action::Look(c.0, c.1))); }
    if colony.drink > 0 { p.ledger(Glyph::Drink, None, "Berry wine", &colony.drink.to_string(), "cups in the store", None); }
    if colony.herbs > 0 { p.ledger(Glyph::Herbs, None, "Herbs", &colony.herbs.to_string(), "for the wounded, from the caravan", None); }
    if let Some((d, dish, fine)) = &colony.supper { if *d == colony.clock.day() { p.para(&format!("Tonight's supper: {}{}.", dish, if *fine { ", a fine one" } else { "" }), Face::Italic, SMALL + 1.0, SOFT, 0.0, None); } }

    p.heading("Timber and stone");
    let logs = stored(Stuff::Timber);
    let stone = stored(Stuff::Stone);
    p.ledger(Glyph::Log, None, "Logs", &logs.to_string(), "in the store by the fire", Some(Action::Look(colony.camp.0, colony.camp.1)));
    if stone > 0 { p.ledger(Glyph::Stone, None, "Stone", &stone.to_string(), "in the store by the fire", Some(Action::Look(colony.camp.0, colony.camp.1))); }
    let (nl, at) = loose_of(&|s| s == Stuff::Timber);
    if nl > 0 { p.ledger(Glyph::Log, None, "Logs lying where trees fell", &nl.to_string(), &places(&at), at.first().map(|c| Action::Look(c.0, c.1))); }
    let (ns, at) = loose_of(&|s| s == Stuff::Stone);
    if ns > 0 { p.ledger(Glyph::Stone, None, "Stone lying where it was broken", &ns.to_string(), &places(&at), at.first().map(|c| Action::Look(c.0, c.1))); }
    if let Some(wp) = colony.projects.iter().find(|q| q.done && q.kind == crate::colony::projects::ProjectKind::Woodpile) {
        p.ledger(Glyph::Log, None, "The woodpile", &format!("{} of {}", wp.used, wp.needed), "burnt each night by the fire", Some(Action::Look(wp.at.0, wp.at.1)));
    }

    let ind = &colony.industry;
    let any_ind = !ind.ore.is_empty() || !ind.bars.is_empty() || ind.charcoal + ind.blocks + ind.barrels + ind.clay + ind.sand > 0 || !colony.gems.is_empty() || colony.cloth > 0 || !colony.remains.is_empty();
    if any_ind {
        p.heading("Worked materials");
        for (m, n) in &ind.ore { if *n > 0 { p.ledger(Glyph::Ore, Some(glyphs::metal_colour(m)), &format!("{} ore", cap(m)), &n.to_string(), "loads, for the smelter", None); } }
        for (m, n) in &ind.bars { if *n > 0 { p.ledger(Glyph::Bars, Some(glyphs::metal_colour(m)), &format!("{} bars", cap(m)), &n.to_string(), "for the forge", None); } }
        if ind.charcoal > 0 { p.ledger(Glyph::Charcoal, None, "Charcoal", &ind.charcoal.to_string(), "fuel for the smelter and the forge", None); }
        if ind.blocks > 0 { p.ledger(Glyph::Block, None, "Dressed blocks", &ind.blocks.to_string(), "from the mason's", None); }
        if ind.barrels > 0 { p.ledger(Glyph::Barrel, None, "Barrels", &ind.barrels.to_string(), "from the carpenter's", None); }
        if ind.clay > 0 { p.ledger(Glyph::Clay, None, "Clay", &ind.clay.to_string(), "for the kiln", None); }
        if ind.sand > 0 { p.ledger(Glyph::Clay, Some([214.0, 196.0, 150.0]), "Sand", &ind.sand.to_string(), "for the kiln", None); }
        for (g, n) in &colony.gems { if *n > 0 { p.ledger(Glyph::Gem, Some(glyphs::gem_colour(g)), &cap(g), &n.to_string(), "prised from the rock", None); } }
        let cloth = colony.cloth.saturating_sub(colony.cloth_used);
        if cloth > 0 { p.ledger(Glyph::Cloth, None, "Cloth", &cloth.to_string(), "bolts from the caravans", None); }
        for (name, _, bones, hide, skin) in &colony.remains {
            if *bones > 0 { p.ledger(Glyph::Bone, None, &format!("Bones of {}", name), &bones.to_string(), "for the carvers", None); }
            if *hide > 0 { p.ledger(Glyph::Hide, None, &format!("The {} of {}", skin, name), &hide.to_string(), "pieces, for armour", None); }
        }
    }

    let kept: Vec<&crate::colony::craft::Work> = colony.works.iter().filter(|w| !w.traded).collect();
    let sold = colony.works.len() - kept.len();
    if !kept.is_empty() || !colony.treasures.is_empty() || !colony.placed.is_empty() || colony.relic.as_ref().map_or(false, |r| r.found.is_some()) {
        p.heading(&format!("Works and treasures: {}", kept.len() + colony.treasures.len()));
        for t in &colony.treasures { p.ledger(Glyph::of_thing(t), None, &fit(&cap(t), Face::Roman, BODY, p.width() - 60.0), "", "", None); }
        if let Some(r) = colony.relic.as_ref().filter(|r| r.found.is_some()) {
            p.ledger(Glyph::of_thing(&format!("{} {}", r.what, r.name)), None, &format!("{}, {}", r.name, r.what), "", &format!("found by {} on day {}; {}", r.found.as_ref().unwrap().0, r.found.as_ref().unwrap().1, r.tale), None);
        }
        for (title, room, _) in &colony.placed {
            let where_ = colony.rooms.get(*room).map(|r| r.kind.word()).unwrap_or("a room below");
            p.ledger(Glyph::of_thing(title), None, &cap(title), "", &format!("set in {}", where_), None);
        }
        for w in kept.iter().rev() {
            let maker = colony.settlers.get(w.maker).map(|s| s.name.clone()).unwrap_or_default();
            let tint = if w.quality >= 4 { Some([214.0, 176.0, 70.0]) } else { None };
            let ev = w.image.as_ref().and_then(|(_, e)| *e).map(Action::Event);
            // A settler's own (`Colony::kept`): said, and their sheet a click away.
            let keeper = colony.works.iter().position(|x| std::ptr::eq(x, &**w)).and_then(|k| colony.kept.iter().find(|kk| kk.0 == k)).map(|kk| kk.1);
            let kept_by = keeper.and_then(|o| colony.settlers.get(o)).map(|s| format!("; kept by {} as their own", s.name)).unwrap_or_default();
            p.ledger(Glyph::of_thing(&w.kind), tint, &cap(&w.describe()), "", &format!("made by {} on day {}{}", maker, w.day, kept_by), ev.or(Some(Action::Sheet(keeper.unwrap_or(w.maker)))));
        }
        if sold > 0 { p.para(&format!("{} sold to the caravans.", plural(sold, "work", "works")), Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
        for (t, _, d) in &colony.stolen { p.para(&format!("{}: stolen in the night of day {}.", cap(t), d), Face::Italic, SMALL + 1.0, RUBRIC, 0.0, None); }
    }

    if !colony.arms.is_empty() || !colony.armour.is_empty() {
        p.heading(&format!("Arms and armour: {}", colony.arms.len() + colony.armour.len()));
        let holder = |h: Option<usize>| h.and_then(|k| colony.settlers.get(k)).map(|s| if s.alive { format!("borne by {}", s.name) } else { format!("was {}'s", s.name) }).unwrap_or_else(|| "in the store".into());
        for a in &colony.arms {
            let maker = colony.settlers.get(a.maker).map(|s| s.name.clone()).unwrap_or_default();
            p.ledger(Glyph::of_thing(&a.kind).max_spear(), Some(glyphs::metal_colour(&a.material)), &cap(&a.kind), "", &format!("{}; made by {} on day {}", holder(a.holder), maker, a.day), a.holder.map(Action::Sheet));
        }
        for a in &colony.armour {
            let tint = if a.material.contains("leather") || a.material.contains("fur") || a.material.contains("hide") { None } else { Some(glyphs::metal_colour(&a.material)) };
            p.ledger(match Glyph::of_thing(&a.kind) { Glyph::Work | Glyph::Block => Glyph::Armour, g => g }, tint, &cap(&a.kind), "", &holder(a.holder), a.holder.map(Action::Sheet));
        }
    }
    let pets: Vec<&crate::colony::pets::Pet> = colony.pets.iter().filter(|q| q.alive).collect();
    if colony.pen.is_some() || !pets.is_empty() || !colony.caged.is_empty() {
        p.heading("Beasts");
        if let Some((kind, n)) = &colony.pen { p.ledger_beast(kind, &format!("{} in the pen", cap(kind)), &n.to_string(), "kept for meat", None); }
        for q in pets { p.ledger_beast(&q.kind, &format!("{}, a {}", q.name, q.kind), "", &format!("kept by {}", colony.settlers.get(q.keeper).map(|s| s.name.as_str()).unwrap_or("no one")), Some(Action::Sheet(q.keeper))); }
        for c in &colony.caged { p.ledger_beast(c, &cap(c), "", "in a cage trap", None); }
    }
}

fn works_leaf(p: &mut Pane, colony: &Colony) {
    use crate::colony::projects::{is_dig, ProjectKind};
    let active = colony.projects.iter().position(|q| !q.done && !colony.marked_at(q.at, true));
    p.heading("Under way");
    let mut any = false;
    if let Some(hut) = colony.hut.as_ref().filter(|h| !h.done) {
        p.measure("the hut", hut.logs_used as f32 / crate::colony::HUT_LOGS as f32, &format!("{} of {} loads", hut.logs_used, crate::colony::HUT_LOGS), GOLD);
        any = true;
    }
    for (k, q) in colony.projects.iter().enumerate().filter(|(_, q)| !q.done) {
        let dig = is_dig(q.kind);
        let (frac, word) = if dig {
            let left = colony.dig_plan.as_ref().map_or(0, |d| d.len());
            (if q.needed > 0 { q.used as f32 / q.needed as f32 } else { 0.0 }, if left > 0 { format!("{} cuts to make", left) } else { "being dug".into() })
        } else { (q.used as f32 / q.needed.max(1) as f32, format!("{} of {} loads", q.used, q.needed)) };
        let forbidden = colony.marked_at(q.at, true);
        let status = if forbidden { "waits: its ground is forbidden".to_string() } else if Some(k) == active { word } else { format!("waiting; {}", word) };
        p.para(&cap(q.kind.word()), Face::SmallCaps, BODY + 1.0, INK, 0.0, Some(Action::Look(q.at.0, q.at.1)));
        p.measure(if dig { "dug" } else { "raised" }, frac, &status, if forbidden { RUBRIC } else { GOLD });
        p.para(&q.why, Face::Italic, SMALL + 1.0, SOFT, 12.0, None);
        any = true;
    }
    if !colony.dig_rooms.is_empty() {
        let rooms: Vec<String> = colony.dig_rooms.iter().map(|r| r.kind.word().to_string()).collect();
        p.para(&format!("The dig will make {}.", crate::persona::list(&rooms)), Face::Italic, SMALL + 1.0, SOFT, 0.0, None);
    }
    if !any { p.para("Nothing is being built.", Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
    if !colony.plan_line.is_empty() { p.para(&colony.plan_line, Face::Italic, SMALL + 1.0, INK, 0.0, None); }

    if !colony.rooms.is_empty() {
        let w = colony.map.width;
        let ground = colony.map.surface_z[colony.camp.1 as usize * w + colony.camp.0 as usize];
        let mut levels: Vec<i32> = colony.rooms.iter().map(|r| r.z).collect();
        levels.sort_unstable_by(|a, b| b.cmp(a));
        levels.dedup();
        p.heading(&format!("Rooms under the rock: {}", colony.rooms.len()));
        for z in levels {
            let depth = ground - z;
            p.para(&format!("Level {}{}", z, if depth > 0 { format!(", {} below the camp's ground", depth) } else if depth < 0 { format!(", {} above it", -depth) } else { ", the camp's ground".into() }), Face::SmallCaps, SMALL + 1.0, RUBRIC, 0.0, None);
            for r in colony.rooms.iter().filter(|r| r.z == z) {
                let carved = colony.engravings.iter().filter(|e| e.z == r.z && r.cells.contains(&e.from)).count();
                let value = r.cells.len() as u32 * 2 + r.quality as u32 * 15 + if r.furnished.is_some() { 10 } else { 0 } + carved as u32 * 12
                    + colony.placed.iter().filter(|a| colony.rooms.get(a.1).map_or(false, |x| std::ptr::eq(x, r))).count() as u32 * 60;
                let tier = match value { 0..=19 => "meagre", 20..=39 => "modest", 40..=69 => "decent", 70..=109 => "fine", _ => "grand" };
                let owner = r.owner.and_then(|o| colony.settlers.get(o)).map(|s| format!(", {}'s", s.name)).unwrap_or_default();
                let mut notes = vec![format!("{} cells", r.cells.len())];
                if let Some(m) = &r.furnished { notes.push(format!("{}furnished in {}", crate::colony::craft::QUALITY[r.quality.min(5) as usize], m)); }
                if carved > 0 { notes.push(plural(carved, "wall carved", "walls carved")); }
                if r.day > 0 { notes.push(format!("dug on day {}", r.day)); }
                let at = r.cells.first().copied().unwrap_or(colony.camp);
                let g = match r.kind { crate::colony::delve::RoomKind::Bedroom => Glyph::Provisions, crate::colony::delve::RoomKind::Tomb => Glyph::Bone, crate::colony::delve::RoomKind::Farm => Glyph::Fungus,
                    crate::colony::delve::RoomKind::Smelter | crate::colony::delve::RoomKind::Forge => Glyph::Bars, crate::colony::delve::RoomKind::Mason => Glyph::Block, crate::colony::delve::RoomKind::Carpenter => Glyph::Barrel,
                    crate::colony::delve::RoomKind::Kiln => Glyph::Clay, crate::colony::delve::RoomKind::Cellar => Glyph::Barrel, _ => Glyph::Stone };
                p.ledger(g, None, &format!("{}{}", cap(r.kind.word()), owner), &format!("{} ({})", value, tier), &notes.join("; "), Some(Action::Look(at.0, at.1)));
            }
        }
    }

    let shops: Vec<&crate::colony::projects::Project> = colony.projects.iter().filter(|q| q.done && matches!(q.kind, ProjectKind::Workshop | ProjectKind::Workshops | ProjectKind::MasonShop | ProjectKind::CarpenterShop | ProjectKind::Smelter | ProjectKind::Forge | ProjectKind::Kiln | ProjectKind::Still | ProjectKind::Kitchen | ProjectKind::Library)).collect();
    let ind = &colony.industry;
    if !shops.is_empty() || ind.tools.is_some() || !colony.guilds.is_empty() {
        p.heading("Workshops and industries");
        for q in &shops { p.para(&format!("{} (day {})", cap(q.kind.word()), q.day), Face::Roman, BODY, INK, 0.0, Some(Action::Look(q.at.0, q.at.1))); }
        let mut tally = Vec::new();
        if ind.smelted > 0 { tally.push(format!("{} loads of ore smelted into {} bars", ind.smelted, ind.bars_made)); }
        if ind.burned > 0 { tally.push(format!("{} charcoal burnt", ind.burned)); }
        if ind.dressed > 0 { tally.push(format!("{} stones dressed", ind.dressed)); }
        if ind.barrels_made > 0 { tally.push(format!("{} barrels made", ind.barrels_made)); }
        if ind.fired > 0 { tally.push(format!("{} pots fired", ind.fired)); }
        if ind.forged > 0 { tally.push(format!("{} arms and mail forged", ind.forged)); }
        if !tally.is_empty() { p.para(&format!("So far: {}.", crate::persona::list(&tally)), Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
        match &ind.tools { Some((m, d)) => p.para(&format!("Tools of {}, forged on day {}.", m, d), Face::Roman, BODY, INK, 0.0, None), None if colony.tools_bought => p.para("Iron tools bought from the caravan.", Face::Roman, BODY, INK, 0.0, None), None => {} }
        for g in &colony.guilds {
            let members: Vec<String> = g.members.iter().filter_map(|&m| colony.settlers.get(m)).map(|s| s.name.clone()).collect();
            p.para(&format!("{} ({}): {}", g.name, crate::colony::ROLES[g.trade], crate::persona::list(&members)), Face::Roman, BODY, INK, 0.0, None);
        }
    }

    let done: Vec<&crate::colony::projects::Project> = colony.projects.iter().filter(|q| q.done).collect();
    if colony.hut.as_ref().map_or(false, |h| h.done) || !done.is_empty() {
        p.heading(&format!("Built and dug: {}", done.len() + colony.hut.as_ref().map_or(0, |h| h.done as usize)));
        if let Some(h) = colony.hut.as_ref().filter(|h| h.done) { p.para(&format!("The hut, of {}", if colony.hut_material == crate::colony::ItemKind::Stone { "stone" } else { "timber" }), Face::Roman, BODY, INK, 0.0, Some(Action::Look(h.at.0 + 2, h.at.1 + 2))); }
        for q in done.iter().rev() {
            p.para(&format!("{}, day {}", cap(q.kind.word()), q.day), Face::Roman, BODY, INK, 0.0, Some(Action::Look(q.at.0, q.at.1)));
            p.para(&q.why, Face::Italic, SMALL, SOFT, 12.0, None);
        }
    }
}

fn annals_leaf(p: &mut Pane, colony: &Colony, ui: &UiState) {
    p.buttons(&[("", "great moments", Action::Filter(0), true, ui.annals == 0), ("", "the whole log", Action::Filter(1), true, ui.annals == 1), ("", "the patron's acts", Action::Filter(2), true, ui.annals == 2)]);
    let named = |text: &str| colony.settlers.iter().position(|s| text.contains(&s.name));
    match ui.annals {
        0 => {
            p.heading(&format!("Moments: {}", colony.moments.len()));
            for m in colony.moments.iter().rev().take(150) {
                let day = m.tick / crate::colony::TICKS_PER_DAY + 1;
                let (l, y) = (p.left, p.y);
                let dw = fonts::width(&format!("Day {}", day), Face::Italic, SMALL, 0.0);
                p.write(l, y + 1.0, &format!("Day {}", day), Face::Italic, SMALL, RUBRIC);
                let title = fit(&m.title, Face::SmallCaps, BODY + 1.0, p.width() - dw - 10.0);
                p.write(l + dw + 8.0, y, &title, Face::SmallCaps, BODY + 1.0, if m.major() { INK } else { SOFT });
                p.hit(l - 4.0, y, p.width() + 8.0, 18.0, Action::Look(m.at.0, m.at.1));
                p.y += 20.0;
                p.para(&m.text, Face::Roman, SMALL + 1.0, INK, 0.0, None);
                if !m.because.is_empty() { p.para(&m.because, Face::Italic, SMALL, SOFT, 12.0, None); }
                p.y += 4.0;
            }
        }
        k => {
            let lines: Vec<&String> = colony.log.iter().rev().filter(|l| k == 1 || l.contains("(your doing)")).take(400).collect();
            p.heading(&format!("{}: {} lines", if k == 1 { "The log" } else { "The patron's acts" }, lines.len()));
            for line in lines {
                let (when, what) = line.split_once("  ").unwrap_or(("", line.as_str()));
                let when = when.trim_start_matches("Day ").to_string();
                let (l, y) = (p.left, p.y);
                p.write(l, y + 1.0, &when, Face::Italic, SMALL, RUBRIC);
                let save = p.left;
                p.left = l + 66.0;
                p.para(what, Face::Roman, SMALL + 1.0, INK, 0.0, named(what).map(Action::Select));
                p.left = save;
                p.y += 2.0;
            }
        }
    }
}

fn camp_leaf(p: &mut Pane, colony: &Colony) {
    let name = colony.name.clone().unwrap_or_else(|| format!("The camp at {},{}", colony.map.world_tile.0, colony.map.world_tile.1));
    let (l, y) = (p.left, p.y);
    p.write_tracked(l, y, &name, Face::SmallCaps, 22.0, 0.8, RUBRIC);
    p.y += 30.0;
    let season = colony.season();
    let winter = if colony.hard_winter() { if colony.frozen() { "a hard winter; the water is frozen".to_string() } else { "a hard winter".into() } }
        else { colony.days_to_winter().map(|d| format!("winter in {} days", d)).unwrap_or_default() };
    p.para(&format!("{}: {} ({:.0} \u{b0}C){}{}", colony.clock.stamp(), season.name(), colony.temperature(), if winter.is_empty() { "" } else { "; " }, winter), Face::Roman, BODY, INK, 0.0, None);
    p.para(&format!("{} of {} alive; {} meals in the store, about {:.0} days of food (the camp aims for {}).", colony.alive(), colony.company(), colony.food_stored(), colony.days_of_food(), colony.food_goal()), Face::Roman, BODY, INK, 0.0, None);
    let standing = colony.standing();
    if !standing.is_empty() {
        p.heading("How the camp stands");
        for s in standing { p.para(&s, Face::Roman, BODY, INK, 0.0, None); }
    }
    p.heading("Against a raid");
    let (ready, tally, _) = colony.readiness();
    p.measure("readiness", ready, &format!("{:.2}", ready), if ready >= 0.7 { MOSS } else if ready >= 0.5 { GOLD } else { RUBRIC });
    p.para(&cap(&tally), Face::Italic, SMALL + 1.0, SOFT, 0.0, None);
    if let Some(a) = &colony.arc {
        let day = colony.clock.day();
        if matches!(a.stage, 1 | 2 | 5) {
            p.para(&format!("Foretold: {}, {}. Looked for in {} days.", a.threat.name, a.threat.why, a.raid_day.saturating_sub(day)), Face::Roman, BODY, RUBRIC, 0.0, None);
        } else if a.quiet_until.map_or(false, |q| q > day) {
            p.para("Nothing is foretold; the roads are quiet for now.", Face::Italic, SMALL + 1.0, SOFT, 0.0, None);
        }
        if a.refugee_day > 0 && colony.refugees_waiting() { p.buttons(&[("Y", "take the refugees in", Action::Refugees(true), true, false), ("N", "turn them away", Action::Refugees(false), true, false)]); }
    }
    if let Some(s) = &colony.siege { if day_ok(colony, s.since) { p.para(&format!("Besieged by {}.", s.who), Face::Roman, BODY, RUBRIC, 0.0, None); } }
    if !colony.regards.is_empty() {
        p.heading("The peoples' regard");
        for r in &colony.regards {
            let latest = r.causes.iter().max_by_key(|c| c.day).map(|c| c.text.clone()).unwrap_or_default();
            let c = match r.acted { Some(false) => RUBRIC, Some(true) => MOSS, None if r.total() <= -10 => RUBRIC, None if r.total() >= 10 => MOSS, _ => INK };
            p.para(&format!("{} {} ({:+})", r.people, r.word(), r.total()), Face::Roman, BODY, c, 0.0, None);
            if !latest.is_empty() { p.para(&format!("lately: {}", latest), Face::Italic, SMALL, SOFT, 12.0, None); }
        }
    }
    p.heading("Who leads");
    let mut any = false;
    if let Some(sp) = colony.speaker.filter(|&s| colony.settlers[s].alive) {
        p.para(&format!("{} speaks for the camp{}", colony.settlers[sp].name, colony.mandate.map(|m| format!(": {}", m.short())).unwrap_or_default()), Face::Roman, BODY, INK, 0.0, Some(Action::Sheet(sp)));
        any = true;
    }
    if let Some(lord) = &colony.lord {
        let came = lord.came.map(|d| format!("came on day {}", d)).unwrap_or_else(|| "is to be sent".into());
        p.para(&format!("The lord: {}, kin of {}; {}", lord.name, lord.ruler, came), Face::Roman, BODY, INK, 0.0, None);
        if let Some((what, since, met)) = &lord.demand { p.para(&format!("Demands a fine work of {} (day {}){}", what, since, if *met { "; met" } else { "" }), Face::Italic, SMALL + 1.0, if *met { SOFT } else { RUBRIC }, 12.0, None); }
        any = true;
    }
    if let Some((_, god)) = colony.temple() { p.para(&format!("A temple to {}", god), Face::Roman, BODY, INK, 0.0, None); any = true; }
    if let Some(hl) = colony.healer.filter(|&k| colony.settlers.get(k).map_or(false, |s| s.alive)) { p.para(&format!("{} tends the wounded", colony.settlers[hl].name), Face::Roman, BODY, INK, 0.0, Some(Action::Sheet(hl))); any = true; }
    if !any { p.para("No one yet: the camp decides by the fire.", Face::Italic, SMALL + 1.0, SOFT, 0.0, None); }
    if let Some(t) = &colony.trade {
        p.heading("Trade");
        p.para(&format!("Caravans from {} of {}, {} days' walk; {} came so far.", t.town, t.people, t.days, colony.caravans), Face::Roman, BODY, INK, 0.0, None);
    }
    p.heading("The patron");
    let pat = &colony.patron;
    p.measure("favour", pat.favour as f32 / crate::colony::FAVOUR_MAX as f32, &format!("{} of {} (one more each dawn)", pat.favour, crate::colony::FAVOUR_MAX), GOLD);
    if let Some(f) = pat.favourite { p.para(&format!("Favours {}", colony.settlers[f].name), Face::Roman, BODY, INK, 0.0, Some(Action::Sheet(f))); }
    for &(i, d, until) in &pat.dreams { if until > colony.clock.tick { p.para(&format!("{} dreams of {}", colony.settlers[i].name, d.word()), Face::Roman, BODY, INK, 0.0, Some(Action::Sheet(i))); } }
    for m in &pat.marks { p.para(&format!("{} ground about {},{} ({} cells round)", if m.forbidden { "Forbidden" } else { "Blessed" }, m.at.0, m.at.1, m.radius), Face::Roman, BODY, if m.forbidden { RUBRIC } else { INK }, 0.0, Some(Action::Look(m.at.0, m.at.1))); }
    for (k, at) in &colony.stones { p.para(&format!("A {} at {},{}", k.word(), at.0, at.1), Face::Roman, BODY, INK, 0.0, Some(Action::Look(at.0, at.1))); }
    if colony.bell_rung() { p.para("The bell has rung: all keep under a roof until dawn.", Face::Italic, SMALL + 1.0, RUBRIC, 0.0, None); }
    p.para(&format!("{} acts recorded; they replay with the world's code.", colony.interventions.len()), Face::Italic, SMALL, SOFT, 0.0, None);
    p.heading("Keys");
    for line in [
        "C settlers  \u{b7}  I stocks  \u{b7}  O works  \u{b7}  L annals  \u{b7}  T the camp  \u{b7}  wheel scrolls the panel",
        "F bless  \u{b7}  X forbid  \u{b7}  G favour  \u{b7}  R dream  \u{b7}  B the bell  \u{b7}  H J K the stones  \u{b7}  N name the camp",
        "Space pause  \u{b7}  1 2 3 speed  \u{b7}  4 or Tab skip  \u{b7}  M stop for moments  \u{b7}  P the saga",
        "< > levels  \u{b7}  [ ] the delve  \u{b7}  V level view  \u{b7}  U section  \u{b7}  Esc close, then leave",
    ] { p.para(line, Face::Roman, SMALL + 1.0, SOFT, 0.0, None); }
}

fn day_ok(colony: &Colony, since: u64) -> bool { colony.clock.day() >= since }

// ----------------------------------------------------------------------------------------------

/// Draw the tabs, the open leaf and the key bar; returns what can be clicked.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(colony: &Colony, ui: &mut UiState, history: Option<&crate::history::world_state::WorldHistory>, speed: u32, mouse: (f32, f32), buf: &mut [u32], w: usize, h: usize) -> Vec<Hit> {
    let mut hits = Vec::new();
    if w < 480 || h < 300 { return hits; }
    // The tabs: a strip at the top right; the open one joins the panel below it.
    let tabs = tabs_rect(w, h);
    let mut x = (tabs.x + tabs.w) as f32 - tab_strip_w() as f32 + 4.0;
    if ui.open.is_some() {
        let p = panel_rect(w, h);
        ui::card(buf, w, p);
        x = tabs.x as f32;
    }
    for t in Tab::ALL {
        let tw = tab_w(t);
        let open = ui.open == Some(t);
        let r = Rect { x: x as usize, y: tabs.y + if open { 0 } else { 3 }, w: tw as usize, h: tabs.h + if open { 2 } else { -3i32 as usize }.wrapping_add(0) };
        let r = Rect { h: if open { tabs.h + 2 } else { tabs.h - 3 }, ..r };
        ui::card(buf, w, r);
        if r.contains(mouse.0, mouse.1) && !open { for yy in r.y + 4..r.y + r.h - 3 { for xx in r.x + 4..r.x + r.w - 4 { let k = yy * w + xx; buf[k] = ui::mix(buf[k], GOLD, 0.12); } } }
        if open {
            // Joined to the panel: no rule between them.
            for yy in r.y + r.h - 4..r.y + r.h + 1 { for xx in r.x + 4..r.x + r.w - 4 { let k = yy * w + xx; buf[k] = ui::mix(PAPER, ui::PAPER_SHADE, 0.2); } }
        }
        let ty = r.y as f32 + 5.0;
        fonts::draw(buf, w, h, x + 10.0, ty, t.word(), Face::SmallCaps, 15.0, 0.5, if open { RUBRIC } else { INK }, None);
        let kx = x + 10.0 + fonts::width(t.word(), Face::SmallCaps, 15.0, 0.5) + 5.0;
        fonts::draw(buf, w, h, kx, ty + 2.0, t.key(), Face::Italic, 12.0, 0.0, SOFT, None);
        hits.push(Hit { rect: r, action: Action::Tab(t) });
        x += tw + 4.0;
    }
    if let Some(tab) = ui.open {
        let p = panel_rect(w, h);
        // The close mark.
        let cx = Rect { x: p.x + p.w - 30, y: p.y + 8, w: 20, h: 20 };
        fonts::draw(buf, w, h, cx.x as f32 + 5.0, cx.y as f32, "x", Face::Roman, 17.0, 0.0, if cx.contains(mouse.0, mouse.1) { RUBRIC } else { SOFT }, None);
        hits.push(Hit { rect: cx, action: Action::Close });
        let body = body_rect(w, h);
        let leaf = ui.leaf();
        let mut pane = Pane {
            buf: vec![0; body.w * body.h], w: body.w, h: body.h, y: 6.0, scroll: ui.scroll[leaf], left: 12.0, right: body.w as f32 - 34.0, hits: Vec::new(), no_glyph: false,
            mouse: body.contains(mouse.0, mouse.1).then(|| (mouse.0 - body.x as f32, mouse.1 - body.y as f32)),
        };
        for yy in 0..body.h { pane.buf[yy * body.w..(yy + 1) * body.w].copy_from_slice(&buf[(body.y + yy) * w + body.x..(body.y + yy) * w + body.x + body.w]); }
        match (ui.sheet, tab) {
            (Some(i), _) if i < colony.settlers.len() => sheet_leaf(&mut pane, colony, i, history),
            (_, Tab::Settlers) => settlers_leaf(&mut pane, colony, ui, history),
            (_, Tab::Stocks) => stocks_leaf(&mut pane, colony),
            (_, Tab::Works) => works_leaf(&mut pane, colony),
            (_, Tab::Annals) => annals_leaf(&mut pane, colony, ui),
            (_, Tab::Camp) => camp_leaf(&mut pane, colony),
        }
        ui.content[leaf] = pane.y + 10.0;
        ui.scroll[leaf] = ui.scroll[leaf].clamp(0.0, (ui.content[leaf] - body.h as f32 + 24.0).max(0.0));
        for yy in 0..body.h { buf[(body.y + yy) * w + body.x..(body.y + yy) * w + body.x + body.w].copy_from_slice(&pane.buf[yy * body.w..(yy + 1) * body.w]); }
        // A scroll mark on the right edge: where the view is in the leaf.
        if ui.content[leaf] > body.h as f32 {
            let track = body.h as f32 - 8.0;
            let len = (track * body.h as f32 / ui.content[leaf]).max(20.0);
            let at = (track - len) * ui.scroll[leaf] / (ui.content[leaf] - body.h as f32 + 24.0).max(1.0);
            let sx = body.x + body.w - 8;
            for yy in (body.y as f32 + 4.0 + at) as usize..((body.y as f32 + 4.0 + at + len) as usize).min(body.y + body.h) { buf[yy * w + sx] = INK_FADED; buf[yy * w + sx + 1] = INK_FADED; }
        }
        for (x0, y0, ww, hh, a) in pane.hits {
            let (x0, y0) = (x0.max(0.0), y0.max(0.0));
            let (x1, y1) = ((x0 + ww).min(body.w as f32), (y0 + hh).min(body.h as f32));
            if x1 <= x0 || y1 <= y0 { continue; }
            hits.push(Hit { rect: Rect { x: body.x + x0 as usize, y: body.y + y0 as usize, w: (x1 - x0) as usize, h: (y1 - y0) as usize }, action: a });
        }
    }
    hits.extend(draw_bar(colony, ui, speed, mouse, buf, w, h));
    hits
}

/// The key bar: the patron's verbs, the stones and names, the clock; then the other keys as
/// far as they fit. Every entry is a button.
fn draw_bar(colony: &Colony, ui: &UiState, speed: u32, mouse: (f32, f32), buf: &mut [u32], w: usize, h: usize) -> Vec<Hit> {
    let mut hits = Vec::new();
    let bar = bar_rect(w, h);
    ui::card(buf, w, bar);
    let spent = colony.patron.favour == 0;
    let sel = ui.selected.filter(|&i| colony.settlers.get(i).map_or(false, |s| s.alive));
    let tool = ui.tool;
    let entries: Vec<(&str, &str, Action, bool, bool)> = vec![
        ("F", "bless", Action::Arm(Tool::Bless), !spent || colony.patron.marks.iter().any(|m| !m.forbidden), tool == Some(Tool::Bless)),
        ("X", "forbid", Action::Arm(Tool::Forbid), !spent || colony.patron.marks.iter().any(|m| m.forbidden), tool == Some(Tool::Forbid)),
        ("G", "favour", match sel { Some(i) => Action::Favour(i), None => Action::Arm(Tool::Favour) }, !spent, tool == Some(Tool::Favour)),
        ("R", "dream", match sel { Some(i) => Action::DreamFor(i), None => Action::Arm(Tool::Dream) }, !spent, tool == Some(Tool::Dream)),
        ("B", "bell", Action::Bell, !spent && !colony.bell_rung(), colony.bell_rung()),
        ("|", "", Action::Top, false, false),
        ("H", "hall", Action::Arm(Tool::Hall), colony.stones.len() < 5, tool == Some(Tool::Hall)),
        ("J", "grove", Action::Arm(Tool::Grove), colony.stones.len() < 5, tool == Some(Tool::Grove)),
        ("K", "shrine", Action::Arm(Tool::Shrine), colony.stones.len() < 5, tool == Some(Tool::Shrine)),
        ("N", "name", Action::NameCamp, !colony.refugees_waiting(), false),
        ("", "name a place", Action::Arm(Tool::NamePlace), true, tool == Some(Tool::NamePlace)),
        ("|", "", Action::Top, false, false),
        ("Space", if speed == 0 { "go on" } else { "pause" }, Action::Pause, true, speed == 0),
        ("1", "1x", Action::Speed(1), true, speed == 1),
        ("2", "3x", Action::Speed(3), true, speed == 3),
        ("3", "10x", Action::Speed(10), true, speed == 10),
        ("4", "skip", Action::Skip, true, false),
    ];
    let mut x = (bar.x + 8) as f32;
    let y = (bar.y + 3) as f32;
    let right = (bar.x + bar.w) as f32 - 8.0;
    for (key, label, action, enabled, active) in entries {
        if key == "|" { x += 8.0; for yy in bar.y + 6..bar.y + bar.h - 6 { buf[yy * w + x as usize] = ui::mix(INK_FADED, PAPER, 0.3); } x += 9.0; continue; }
        let kw = if key.is_empty() { 0.0 } else { fonts::width(key, Face::Italic, SMALL, 0.0) + 5.0 };
        let bw = kw + fonts::width(label, Face::Roman, SMALL, 0.0) + 14.0;
        if x + bw > right { break; }
        let hov = Rect { x: x as usize, y: y as usize, w: bw as usize, h: 21 }.contains(mouse.0, mouse.1);
        let (bw, r) = button(buf, w, h, x, y, key, label, enabled, active, hov && enabled);
        if enabled { hits.push(Hit { rect: r, action }); }
        x += bw + 4.0;
    }
    // The other keys, as far as they fit.
    x += 10.0;
    for text in ["M stops", "U section", "</> levels", "[/] delve", "Esc leave"] {
        let tw = fonts::width(text, Face::Roman, SMALL, 0.0);
        if x + tw > right { break; }
        fonts::draw(buf, w, h, x, y + 3.0, text, Face::Roman, SMALL, 0.0, SOFT, None);
        x += tw + 14.0;
    }
    hits
}

/// The dream card's choices (a settler chosen, favour in hand), as buttons under the card.
pub(crate) fn dream_buttons(i: usize, buf: &mut [u32], w: usize, h: usize, mouse: (f32, f32), right: usize) -> Vec<Hit> {
    let items = [("1", "the hut finished", Action::SendDream(i, Dream::Hut)), ("2", "plenty", Action::SendDream(i, Dream::Plenty)), ("3", "rest", Action::SendDream(i, Dream::Rest)), ("4", "the watch", Action::SendDream(i, Dream::Watch)), ("Esc", "none", Action::NoDream)];
    let widths: Vec<f32> = items.iter().map(|(k, l, _)| fonts::width(k, Face::Italic, SMALL, 0.0) + 5.0 + fonts::width(l, Face::Roman, SMALL, 0.0) + 14.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0 * (items.len() - 1) as f32;
    let area = w.saturating_sub(right) as f32;
    let mut x = ((area - total) / 2.0).max(10.0);
    let y = 70.0 + 130.0;
    let mut hits = Vec::new();
    let card = Rect { x: (x - 12.0).max(0.0) as usize, y: (y - 10.0) as usize, w: (total + 24.0) as usize, h: 41 };
    if card.x + card.w < w && card.y + card.h < h { ui::card(buf, w, card); }
    for ((k, l, a), bw) in items.into_iter().zip(widths) {
        let hov = Rect { x: x as usize, y: y as usize, w: bw as usize, h: 21 }.contains(mouse.0, mouse.1);
        let (_, r) = button(buf, w, h, x, y, k, l, true, false, hov);
        hits.push(Hit { rect: r, action: a });
        x += bw + 6.0;
    }
    hits
}

/// Buttons on a moment's card that asks (the refugees): take them in, turn them away.
pub(crate) fn choice_buttons(card: Rect, buf: &mut [u32], w: usize, h: usize, mouse: (f32, f32)) -> Vec<Hit> {
    let mut hits = Vec::new();
    let mut x = (card.x + 20) as f32;
    let y = (card.y + card.h) as f32 - 30.0;
    for (k, l, a) in [("Y", "take them in", Action::Refugees(true)), ("N", "turn them away", Action::Refugees(false))] {
        let bw = fonts::width(k, Face::Italic, SMALL, 0.0) + 5.0 + fonts::width(l, Face::Roman, SMALL, 0.0) + 14.0;
        let hov = Rect { x: x as usize, y: y as usize, w: bw as usize, h: 21 }.contains(mouse.0, mouse.1);
        let (_, r) = button(buf, w, h, x, y, k, l, true, false, hov);
        hits.push(Hit { rect: r, action: a });
        x += bw + 8.0;
    }
    hits
}

/// Headless renders of every leaf and a settler's sheet over the camp, as the window shows them
/// (`--sim-snapshot` writes `<prefix>_ui_settlers.png`, `_ui_stocks.png`, `_ui_works.png`,
/// `_ui_annals.png`, `_ui_camp.png`, `_ui_sheet.png` and `_ui_store.png`, the store's heaps close
/// up). Returns the files written.
pub(crate) fn save_ui_snapshots(colony: &Colony, history: Option<&crate::history::world_state::WorldHistory>, atlas: &super::Atlas, prefix: &str) -> Vec<String> {
    use super::render::{render_local, LocalCamera};
    let (w, h) = (1280usize, 800usize);
    let mut written = Vec::new();
    let pick = colony.settlers.iter().enumerate().filter(|(_, s)| s.alive).max_by_key(|(_, s)| s.mind.thoughts.len() + s.wounds.len() * 3 + s.deeds.len() * 3).map(|(i, _)| i).unwrap_or(0);
    let save = |name: &str, buf: &[u32]| -> String {
        let path = format!("{prefix}_{name}.png");
        let img = image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) });
        let _ = img.save(&path);
        path
    };
    let views: [(&str, Option<Tab>, Option<usize>); 6] = [("ui_settlers", Some(Tab::Settlers), None), ("ui_stocks", Some(Tab::Stocks), None), ("ui_works", Some(Tab::Works), None), ("ui_annals", Some(Tab::Annals), None), ("ui_camp", Some(Tab::Camp), None), ("ui_sheet", Some(Tab::Settlers), Some(pick))];
    for (name, tab, sheet) in views {
        let mut ui = UiState { open: tab, sheet, selected: Some(pick), ..UiState::default() };
        let cam = LocalCamera { cx: colony.camp.0 as f32 - 6.0, cy: colony.camp.1 as f32 + 1.0, tile_px: 16.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        super::local_ink::draw_colony(colony, &cam, &mut buf, w, h, history);
        let status = String::new();
        super::colony_hud::draw(colony, &cam, &super::colony_hud::HudState { speed: if sheet.is_some() { 0 } else { 1 }, status: &status, mouse: (-100.0, -100.0), right: reserve_right(&ui, w), selected: ui.selected, hide_chip: true, bar: false }, &mut buf, w, h);
        let hits = draw(colony, &mut ui, history, 1, (-100.0, -100.0), &mut buf, w, h);
        println!("UI {}: {} things to click, the leaf {:.0} px long", name, hits.len(), ui.content[ui.leaf()]);
        written.push(save(name, &buf));
    }
    // The store's heaps close up, with the chip naming the heap under the mouse.
    {
        let cam = LocalCamera { cx: colony.camp.0 as f32 + 0.5, cy: colony.camp.1 as f32 + 0.5, tile_px: 40.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        super::local_ink::draw_colony(colony, &cam, &mut buf, w, h, history);
        let heap = super::local_ink::store_heaps(colony).into_iter().next();
        let mouse = heap.map(|hp| ((hp.cell.0 + 0.5 - cam.cx) * cam.tile_px + w as f32 / 2.0, (hp.cell.1 + 0.5 - cam.cy) * cam.tile_px + h as f32 / 2.0)).unwrap_or((-100.0, -100.0));
        let status = String::new();
        super::colony_hud::draw(colony, &cam, &super::colony_hud::HudState { speed: 1, status: &status, mouse, right: 0, selected: None, hide_chip: false, bar: false }, &mut buf, w, h);
        let mut ui = UiState::default();
        draw(colony, &mut ui, history, 1, mouse, &mut buf, w, h);
        // Every glyph in a row along the top, for checking them side by side.
        let all = [Glyph::Log, Glyph::Stone, Glyph::Berries, Glyph::Fish, Glyph::Meat, Glyph::Grain, Glyph::Fungus, Glyph::Provisions, Glyph::Bars, Glyph::Ore, Glyph::Gem, Glyph::Block,
            Glyph::Work, Glyph::Spear, Glyph::Armour, Glyph::Drink, Glyph::Cloth, Glyph::Hide, Glyph::Bone, Glyph::Charcoal, Glyph::Barrel, Glyph::Book, Glyph::Herbs, Glyph::Clay];
        let strip = Rect { x: 330, y: 50, w: all.len() * 36 + 20, h: 76 };
        ui::card(&mut buf, w, strip);
        for (k, g) in all.iter().enumerate() {
            glyphs::draw_u32(&mut buf, w, h, *g, (strip.x + 26 + k * 36) as f32, (strip.y + 24) as f32, 24.0, None);
            glyphs::draw_u32(&mut buf, w, h, *g, (strip.x + 26 + k * 36) as f32, (strip.y + 56) as f32, 11.0, None);
        }
        written.push(save("ui_store", &buf));
    }
    written
}
