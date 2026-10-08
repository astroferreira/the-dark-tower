//! The colony's face in the window: the clock and the patron's favour (top left), the last
//! moments of the log (bottom left; a click on a line naming a settler opens them), a chip at the
//! mouse saying what the settler under it is doing and why, the keys along the bottom (the
//! patron's verbs greyed while no favour is left), and the patron's marks on people: a gold ring
//! round the favourite and a small star over each dreamer. Lettered in IM Fell on parchment.

use super::fonts::{self, Face};
use super::render::LocalCamera;
use super::ui::{self, Rect, GOLD, INK, INK_FADED, RUBRIC};
use crate::colony::Colony;

/// What the HUD needs from the viewer besides the colony.
pub(crate) struct HudState<'a> {
    pub speed: u32,
    /// The answer to the player's last act ("The patron favours X; the others notice.").
    pub status: &'a str,
    pub mouse: (f32, f32),
}

/// A clickable log line and the settler it names.
pub(crate) struct LogHit { pub rect: Rect, pub settler: usize }

const BODY: f32 = 15.0;
const SMALL: f32 = 13.0;

fn to_screen(lcam: &LocalCamera, w: usize, h: usize, p: (u16, u16)) -> (f32, f32) {
    ((p.0 as f32 + 0.5 - lcam.cx) * lcam.tile_px + w as f32 / 2.0, (p.1 as f32 + 0.5 - lcam.cy) * lcam.tile_px + h as f32 / 2.0)
}

fn to_screen_f(lcam: &LocalCamera, w: usize, h: usize, p: (f32, f32)) -> (f32, f32) {
    ((p.0 + 0.5 - lcam.cx) * lcam.tile_px + w as f32 / 2.0, (p.1 + 0.5 - lcam.cy) * lcam.tile_px + h as f32 / 2.0)
}

/// Wrap to a pixel width in the given face.
fn wrap_px(text: &str, face: Face, px: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let trial = if line.is_empty() { word.to_string() } else { format!("{} {}", line, word) };
        if !line.is_empty() && fonts::width(&trial, face, px, 0.0) > max_w {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = trial;
        }
    }
    if !line.is_empty() { lines.push(line); }
    lines
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

fn ring(buf: &mut [u32], w: usize, h: usize, c: (f32, f32), r: f32, color: u32, thick: f32) {
    let (x0, x1) = ((c.0 - r - thick).floor() as i64, (c.0 + r + thick).ceil() as i64);
    let (y0, y1) = ((c.1 - r - thick).floor() as i64, (c.1 + r + thick).ceil() as i64);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = ((x as f32 + 0.5 - c.0).hypot(y as f32 + 0.5 - c.1) - r).abs();
            if d < thick { ui::blend_px(buf, w, h, x, y, color, (thick - d).min(1.0)); }
        }
    }
}

fn star(buf: &mut [u32], w: usize, h: usize, c: (f32, f32), r: f32, color: u32) {
    // Four inked rays and a dot: a small spark over a dreamer's head.
    for k in 0..4 {
        let a = k as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4 * (k % 2) as f32 * 0.0;
        let (dx, dy) = (a.cos(), a.sin());
        let mut t = 0.0;
        while t < r {
            ui::blend_px(buf, w, h, (c.0 + dx * t) as i64, (c.1 + dy * t) as i64, color, 1.0 - t / r * 0.6);
            t += 0.5;
        }
    }
    ring(buf, w, h, c, 1.2, color, 1.2);
}

/// Draw the HUD over the colony frame. Returns the clickable log lines.
pub(crate) fn draw(colony: &Colony, lcam: &LocalCamera, st: &HudState, buf: &mut [u32], w: usize, h: usize) -> Vec<LogHit> {
    let mut hits = Vec::new();
    if w < 320 || h < 240 { return hits; }

    // The patron's marks on people.
    if lcam.surface_view {
        if let Some(i) = colony.patron.favourite.filter(|&i| colony.settlers[i].alive) {
            let p = colony.draw_pos(i);
            let c = to_screen_f(lcam, w, h, p);
            ring(buf, w, h, c, (lcam.tile_px * 0.9).max(7.0), GOLD, 1.6);
        }
        for &(i, _, until) in &colony.patron.dreams {
            if until <= colony.clock.tick || !colony.settlers[i].alive { continue; }
            let c = to_screen_f(lcam, w, h, colony.draw_pos(i));
            star(buf, w, h, (c.0, c.1 - (lcam.tile_px * 1.1).max(9.0)), 4.0, GOLD);
        }
    }

    // Top left: name, clock and speed, favour, and the answer to the last act.
    let name = colony.name.clone().unwrap_or_else(|| format!("The camp at {},{}", colony.map.world_tile.0, colony.map.world_tile.1));
    let clock = format!("{}   {}", colony.clock.stamp(), if st.speed == 0 { "paused".to_string() } else { format!("{}x", st.speed) });
    let alive = colony.alive();
    let people = format!("{} of {} alive", alive, colony.company());
    let card_w = 300usize.min(w - 20);
    // The camp's plan, else the answer to the last act.
    let said = if !st.status.is_empty() { st.status } else { colony.plan_line.as_str() };
    let status = if said.is_empty() { Vec::new() } else { wrap_px(said, Face::Italic, SMALL, card_w as f32 - 28.0) };
    // How the camp stands: speaker and mandate, temple, mood, the moon, the stocks.
    let standing: Vec<String> = colony.standing().into_iter().take(4).map(|l| fit(&l, Face::Roman, SMALL, card_w as f32 - 28.0)).collect();
    let card_h = 92 + status.len().min(3) * 16 + if standing.is_empty() { 0 } else { 8 + standing.len() * 16 };
    let r = Rect { x: 10, y: 10, w: card_w, h: card_h };
    ui::card(buf, w, r);
    fonts::draw(buf, w, h, 22.0, 18.0, &fit(&name, Face::SmallCaps, 18.0, card_w as f32 - 30.0), Face::SmallCaps, 18.0, 0.5, RUBRIC, None);
    fonts::draw(buf, w, h, 22.0, 42.0, &clock, Face::Roman, BODY, 0.0, INK, None);
    fonts::draw(buf, w, h, 22.0, 62.0, &people, Face::Italic, SMALL, 0.0, INK_FADED, None);
    // Favour: three pips, gold while unspent.
    let label_w = fonts::width("favour", Face::Italic, SMALL, 0.0);
    let fx = card_w as f32 - 22.0 - label_w - 3.0 * 16.0;
    fonts::draw(buf, w, h, fx, 62.0, "favour", Face::Italic, SMALL, 0.0, INK_FADED, None);
    for k in 0..crate::colony::FAVOUR_MAX {
        let c = (fx + label_w + 12.0 + k as f32 * 15.0, 70.0);
        if k < colony.patron.favour {
            for y in -5i64..=5 { for x in -5i64..=5 {
                let d = ((x * x + y * y) as f32).sqrt();
                if d <= 5.0 { ui::blend_px(buf, w, h, c.0 as i64 + x, c.1 as i64 + y, GOLD, (5.5 - d).min(1.0)); }
            } }
        }
        ring(buf, w, h, c, 5.0, INK, 0.9);
    }
    for (k, line) in status.iter().take(3).enumerate() {
        fonts::draw(buf, w, h, 22.0, 84.0 + k as f32 * 16.0, line, Face::Italic, SMALL, 0.0, INK, None);
    }
    let sy = 92.0 + status.len().min(3) as f32 * 16.0;
    for (k, line) in standing.iter().enumerate() {
        // The first line is the most pressing: in rubric when it is a mood, the moon or a fit.
        let urgent = k == 0 && (line.contains("mood") || line.contains("moon is up") || line.contains("fit"));
        fonts::draw(buf, w, h, 22.0, sy + k as f32 * 16.0, line, Face::Roman, SMALL, 0.0, if urgent { RUBRIC } else { INK_FADED }, None);
    }

    // Bottom: the keys. The patron's verbs fade while there is no favour.
    let bar_h = 26usize;
    let bar = Rect { x: 10, y: h - bar_h - 8, w: w - 20, h: bar_h };
    ui::card(buf, w, bar);
    let spent = colony.patron.favour == 0;
    let keys: [(&str, bool); 16] = [
        ("F bless", true), ("X forbid", true), ("G favour", true), ("R dream", true), ("B bell", true),
        ("H/J/K stones", false), ("N name", false), ("Space pause", false), ("1/2/3 speed", false), ("4 skip", false), ("M stops", false), ("U section", false), ("</> levels", false), ("[/] delve", false), ("click to read", false), ("Esc leave", false),
    ];
    let mut x = 22.0;
    for (text, costs) in keys {
        let color = if costs && spent { ui::mix(INK_FADED, ui::PAPER, 0.5) } else if costs { INK } else { INK_FADED };
        let tw = fonts::width(text, Face::Roman, SMALL, 0.0);
        if x + tw > (bar.x + bar.w) as f32 - 12.0 { break; }
        fonts::draw(buf, w, h, x, (bar.y + 6) as f32, text, Face::Roman, SMALL, 0.0, color, None);
        x += tw + 18.0;
    }

    // Bottom left: the last six moments, newest darkest.
    let log_w = 560usize.min(w.saturating_sub(40 + super::inspector::panel_rect(w, h).w.min(w / 3)));
    let n = colony.log.len().min(6);
    if n > 0 && log_w > 200 {
        let line_h = 18usize;
        let lr = Rect { x: 10, y: bar.y - 8 - (n * line_h + 20), w: log_w, h: n * line_h + 20 };
        ui::card(buf, w, lr);
        for (k, line) in colony.log[colony.log.len() - n..].iter().enumerate() {
            let age = (n - 1 - k) as f32 / 6.0;
            let color = ui::mix(INK, INK_FADED, age * 1.4);
            let y = lr.y + 10 + k * line_h;
            // "Day 3, 10:00  text": the time small, the text in the body face.
            let (when, what) = line.split_once("  ").unwrap_or(("", line.as_str()));
            let when = when.trim_start_matches("Day ").to_string();
            let ww = fonts::width(&when, Face::Italic, SMALL, 0.0);
            fonts::draw(buf, w, h, (lr.x + 12) as f32, y as f32 + 1.0, &when, Face::Italic, SMALL, 0.0, RUBRIC, None);
            let tx = (lr.x + 22) as f32 + ww;
            fonts::draw(buf, w, h, tx, y as f32, &fit(what, Face::Roman, BODY, (lr.x + lr.w) as f32 - 12.0 - tx), Face::Roman, BODY, 0.0, color, None);
            if let Some(i) = colony.settlers.iter().position(|s| what.contains(&s.name)) {
                hits.push(LogHit { rect: Rect { x: lr.x, y, w: lr.w, h: line_h }, settler: i });
            }
        }
    }

    // At the mouse: who this is, what they are doing and why.
    if lcam.surface_view {
        let (hx, hy) = (lcam.cx + (st.mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (st.mouse.1 - h as f32 / 2.0) / lcam.tile_px);
        let reach = (0.8f32).max(8.0 / lcam.tile_px);
        let building = if hx >= 0.0 && hy >= 0.0 { colony.building_at((hx as u16, hy as u16)) } else { None };
        let under = colony.settler_at(hx, hy, reach, if lcam.surface_view { None } else { Some(lcam.z) });
        if let (Some(b), None) = (&building, under) {
            let chip_w = 320.0f32.min(w as f32 - 20.0);
            let lines = wrap_px(b, Face::Italic, SMALL, chip_w - 24.0);
            let chip_h = 16 + lines.len().min(5) * 16;
            let x = (st.mouse.0 + 18.0).min(w as f32 - chip_w - 10.0).max(10.0) as usize;
            let y = (st.mouse.1 + 18.0).min((h - chip_h - 10) as f32).max(10.0) as usize;
            ui::card(buf, w, Rect { x, y, w: chip_w as usize, h: chip_h });
            for (k, l) in lines.iter().take(5).enumerate() {
                fonts::draw(buf, w, h, (x + 12) as f32, (y + 8 + k * 16) as f32, l, Face::Italic, SMALL, 0.0, INK, None);
            }
        }
        if let Some(s) = under.map(|i| &colony.settlers[i]) {
            let chip_w = 300.0f32.min(w as f32 - 20.0);
            let why = wrap_px(&s.why, Face::Italic, SMALL, chip_w - 24.0);
            let mut head = s.name.clone();
            if let Some(o) = &s.office { head.push_str(&format!(", {}", o.to_lowercase())); }
            if colony.patron.favourite.map_or(false, |f| colony.settlers[f].name == s.name) { head.push_str(", the patron's favourite"); }
            let feels = match s.mind.broken { Some((b, _)) => format!("{}  -  {}{}", s.job.verb(), if b.word().starts_with("wander") { "" } else { "in " }, b.word()), None => format!("{}  -  {}", s.job.verb(), crate::colony::mind::mood(s.mind.stress)) };
            let chip_h = 48 + why.len().min(4) * 16;
            let x = (st.mouse.0 + 18.0).min(w as f32 - chip_w - 10.0).max(10.0) as usize;
            let y = (st.mouse.1 + 18.0).min((h - chip_h - 10) as f32).max(10.0) as usize;
            let r = Rect { x, y, w: chip_w as usize, h: chip_h };
            ui::card(buf, w, r);
            fonts::draw(buf, w, h, (x + 12) as f32, (y + 8) as f32, &fit(&head, Face::SmallCaps, BODY, chip_w - 24.0), Face::SmallCaps, BODY, 0.3, RUBRIC, None);
            fonts::draw(buf, w, h, (x + 12) as f32, (y + 27) as f32, &fit(&feels, Face::Roman, SMALL, chip_w - 24.0), Face::Roman, SMALL, 0.0, INK, None);
            for (k, line) in why.iter().take(4).enumerate() {
                fonts::draw(buf, w, h, (x + 12) as f32, (y + 44 + k * 16) as f32, line, Face::Italic, SMALL, 0.0, INK_FADED, None);
            }
        }
    }
    hits
}

/// A great moment's card, across the top middle: its title, what happened and why, until Space.
pub(crate) fn draw_moment(m: &crate::colony::Moment, buf: &mut [u32], w: usize, h: usize) {
    let card_w = 520usize.min(w.saturating_sub(40));
    if card_w < 200 || h < 200 { return; }
    let inner = card_w as f32 - 40.0;
    let text = wrap_px(&m.text, Face::Roman, 17.0, inner);
    let because = wrap_px(&m.because, Face::Italic, BODY, inner);
    let card_h = 60 + text.len().min(6) * 22 + 8 + because.len().min(4) * 19 + 34;
    let r = Rect { x: (w - card_w) / 2, y: 70.min(h.saturating_sub(card_h + 10)), w: card_w, h: card_h };
    ui::card(buf, w, r);
    let x = (r.x + 20) as f32;
    let tw = fonts::width(&m.title, Face::SmallCaps, 24.0, 1.0);
    fonts::draw(buf, w, h, (r.x as f32 + (card_w as f32 - tw) / 2.0).max(x), (r.y + 16) as f32, &m.title, Face::SmallCaps, 24.0, 1.0, RUBRIC, None);
    let mut y = (r.y + 56) as f32;
    for line in text.iter().take(6) {
        fonts::draw(buf, w, h, x, y, line, Face::Roman, 17.0, 0.0, INK, None);
        y += 22.0;
    }
    y += 8.0;
    for line in because.iter().take(4) {
        fonts::draw(buf, w, h, x, y, line, Face::Italic, BODY, 0.0, INK_FADED, None);
        y += 19.0;
    }
    if m.because.is_empty() { return; }
    let hint = if m.choice { "Y: take them in     N: turn them away" } else { "Space to go on   M: stop for moments on/off" };
    let hw = fonts::width(hint, Face::Italic, SMALL, 0.0);
    fonts::draw(buf, w, h, (r.x + card_w) as f32 - 20.0 - hw, (r.y + card_h) as f32 - 26.0, hint, Face::Italic, SMALL, 0.0, GOLD, None);
}

/// One line of guidance in a parchment chip at the bottom middle of the screen.
pub(crate) fn draw_hint(text: &str, buf: &mut [u32], w: usize, h: usize) {
    let tw = fonts::width(text, Face::Italic, BODY, 0.0);
    let cw = (tw + 36.0) as usize;
    if cw + 20 > w || h < 120 { return; }
    let r = Rect { x: (w - cw) / 2, y: h - 46, w: cw, h: 30 };
    ui::card(buf, w, r);
    fonts::draw(buf, w, h, (r.x + 18) as f32, (r.y + 6) as f32, text, Face::Italic, BODY, 0.0, INK, None);
}

/// First letter up ("the camp at 45,12" -> "The camp at 45,12").
pub(crate) fn capitalize_pub(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
