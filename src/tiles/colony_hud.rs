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
    /// Pixels taken on the right by the open ledger (`colony_ui`): the log stays clear of it.
    pub right: usize,
    /// The settler chosen in the ledger: an ink ring round them.
    pub selected: Option<usize>,
    /// No chip at the mouse (it is over the interface).
    pub hide_chip: bool,
    /// Draw the old key bar (the window, until `colony_ui`'s button bar is wired in).
    pub bar: bool,
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
        if let Some(i) = st.selected.filter(|&i| colony.settlers.get(i).map_or(false, |s| s.alive) && !colony.below(i)) {
            let c = to_screen_f(lcam, w, h, colony.draw_pos(i));
            ring(buf, w, h, c, (lcam.tile_px * 1.1).max(9.0), RUBRIC, 1.2);
        }
        for &(i, _, until) in &colony.patron.dreams {
            if until <= colony.clock.tick || !colony.settlers[i].alive { continue; }
            let c = to_screen_f(lcam, w, h, colony.draw_pos(i));
            star(buf, w, h, (c.0, c.1 - (lcam.tile_px * 1.1).max(9.0)), 4.0, GOLD);
        }
    }

    // Top left: name, clock and speed, favour, and the answer to the last act.
    let name = colony.name.clone().unwrap_or_else(|| format!("The {} at {},{}", colony.stage_word(), colony.map.world_tile.0, colony.map.world_tile.1));
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
    if st.bar { ui::card(buf, w, bar); }
    let spent = colony.patron.favour == 0;
    let keys: [(&str, bool); 17] = [
        ("F bless", true), ("X forbid", true), ("G favour", true), ("R dream", true), ("B bell", true),
        ("H/J/K stones", false), ("N name", false), ("Space pause", false), ("1/2/3 speed", false), ("4 skip", false), ("M stops", false), ("Y plan", false), ("U section", false), ("</> levels", false), ("[/] delve", false), ("click to read", false), ("Esc leave", false),
    ];
    let mut x = 22.0;
    for (text, costs) in keys.into_iter().filter(|_| st.bar) {
        let color = if costs && spent { ui::mix(INK_FADED, ui::PAPER, 0.5) } else if costs { INK } else { INK_FADED };
        let tw = fonts::width(text, Face::Roman, SMALL, 0.0);
        if x + tw > (bar.x + bar.w) as f32 - 12.0 { break; }
        fonts::draw(buf, w, h, x, (bar.y + 6) as f32, text, Face::Roman, SMALL, 0.0, color, None);
        x += tw + 18.0;
    }

    // Bottom left: the last six moments, newest darkest.
    let log_w = 560usize.min(w.saturating_sub(40 + super::inspector::panel_rect(w, h).w.min(w / 3).max(st.right)));
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

    // A level below: a key to what is cut in it, under the level's caption.
    if !lcam.surface_view { draw_level_key(colony, lcam, buf, w, h, 10 + card_h + 8); }
    // At the mouse on a level below: what the rock there is.
    if !lcam.surface_view && !st.hide_chip {
        let (hx, hy) = (lcam.cx + (st.mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (st.mouse.1 - h as f32 / 2.0) / lcam.tile_px);
        let under = colony.settler_at(hx, hy, (0.8f32).max(8.0 / lcam.tile_px), Some(lcam.z));
        if under.is_none() && hx >= 0.0 && hy >= 0.0 {
            if let Some(words) = ground_words(colony, hx as usize, hy as usize, Some(lcam.z)) { word_chip(&words, st, buf, w, h); }
        }
    }
    // At the mouse: who this is, what they are doing and why.
    if lcam.surface_view && !st.hide_chip {
        let (hx, hy) = (lcam.cx + (st.mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (st.mouse.1 - h as f32 / 2.0) / lcam.tile_px);
        let reach = (0.8f32).max(8.0 / lcam.tile_px);
        let building = if hx >= 0.0 && hy >= 0.0 { colony.building_at((hx as u16, hy as u16)) } else { None };
        let under = colony.settler_at(hx, hy, reach, if lcam.surface_view { None } else { Some(lcam.z) });
        // A thing on the ground or a heap of the store: what it is and how many.
        let cell = (hx.floor() as i64, hy.floor() as i64);
        let thing = if under.is_none() && cell.0 >= 0 && cell.1 >= 0 {
            super::local_ink::store_heaps(colony).into_iter().find(|hp| hp.cell.0 as i64 == cell.0 && hp.cell.1 as i64 == cell.1)
                .map(|hp| format!("The store: {} {}", hp.count, hp.stuff.plural()))
                .or_else(|| super::local_ink::loose_piles(colony).into_iter().find(|p| p.0 .0 as i64 == cell.0 && p.0 .1 as i64 == cell.1)
                    .map(|(_, s, n)| if n == 1 { format!("{}, lying here to be carried in", super::colony_ui::cap_pub(s.one())) } else { format!("{} {}, lying here to be carried in", n, s.plural()) }))
        } else { None };
        let building = thing.or(building);
        // Else a creature (what it is doing), else the ground itself.
        let building = building.or_else(|| if under.is_none() { creature_words(colony, hx, hy, reach) } else { None })
            .or_else(|| if under.is_none() { mouth_words(colony, hx, hy) } else { None })
            .or_else(|| if under.is_none() && hx >= 0.0 && hy >= 0.0 { ground_words(colony, hx as usize, hy as usize, None) } else { None });
        if let (Some(b), None) = (&building, under) {
            let chip_w = (fonts::width(b, Face::Italic, SMALL, 0.0) + 26.0).min(320.0).min(w as f32 - 20.0);
            let lines = wrap_px(b, Face::Italic, SMALL, chip_w - 24.0);
            let chip_h = 16 + lines.len().min(5) * 16;
            let x = chip_x(st, w, chip_w);
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
            let x = chip_x(st, w, chip_w);
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

/// A short chip at the mouse with `text` in it.
fn word_chip(text: &str, st: &HudState, buf: &mut [u32], w: usize, h: usize) {
    let chip_w = (fonts::width(text, Face::Italic, SMALL, 0.0) + 26.0).min(320.0).min(w as f32 - 20.0);
    let lines = wrap_px(text, Face::Italic, SMALL, chip_w - 24.0);
    let chip_h = 16 + lines.len().min(5) * 16;
    let x = chip_x(st, w, chip_w);
    let y = (st.mouse.1 + 18.0).min((h - chip_h - 10) as f32).max(10.0) as usize;
    ui::card(buf, w, Rect { x, y, w: chip_w as usize, h: chip_h });
    for (k, l) in lines.iter().take(5).enumerate() {
        fonts::draw(buf, w, h, (x + 12) as f32, (y + 8 + k * 16) as f32, l, Face::Italic, SMALL, 0.0, INK, None);
    }
}

/// A creature near (hx, hy), in words: what it is and what it is doing.
/// A way into a place in the hills under the mouse: unknown until someone has gone in.
pub(crate) fn mouth_words(colony: &Colony, hx: f32, hy: f32) -> Option<String> {
    let (k, pl) = colony.map.places.iter().enumerate().find(|(_, p)| p.mouth.map_or(false, |m| (m.0 as f32 + 0.5 - hx).abs() < 1.2 && (m.1 as f32 + 0.5 - hy).abs() < 1.2))?;
    if colony.places_found.contains(&k) { return Some(format!("{}: {}", pl.name, pl.cause)); }
    if let Some(l) = colony.lost.iter().find(|l| l.place == k) { return Some(format!("A way in: {} went in on day {} and did not come back", colony.settlers[l.who].name, l.day)); }
    Some(if colony.mouths_seen.contains(&k) { "A way in: no one has gone in, and no one knows what lies inside".to_string() } else { "A way into the hill: no one has noticed it yet".to_string() })
}

pub(crate) fn creature_words(colony: &Colony, hx: f32, hy: f32, reach: f32) -> Option<String> {
    use crate::colony::creatures::CreatureKind as K;
    let c = colony.creatures.iter().filter(|c| c.z.is_none() && !colony.creature_below(c))
        .find(|c| (c.pos.0 as f32 + 0.5 - hx).abs() < reach && (c.pos.1 as f32 + 0.5 - hy).abs() < reach)?;
    let name = super::colony_ui::cap_pub(&c.name);
    Some(match c.kind {
        K::Game => format!("{}, grazing", name),
        K::Pet => match colony.pets.iter().find(|p| p.alive && p.id == c.id) {
            Some(p) => format!("{}, {}, kept by {}", p.name, p.kind, colony.settlers[p.keeper].name),
            None => format!("{}, tame", name),
        },
        K::Prowler | K::Scout => {
            let doing = if c.leaving { "going off" } else if c.path.is_empty() { "watching the camp" } else { "coming nearer" };
            // The pack's cause (`colony::bestiary`).
            match colony.bestiary().into_iter().find(|k| c.kind == K::Prowler && (c.name.contains(k.name.trim_end_matches('s')) || c.name.contains(&k.name.replace("ves", "f")))) {
                Some(k) => format!("{}, {}: here because {}", name, doing, k.cause),
                None => format!("{}, {}", name, doing),
            }
        }
        K::Wolf if c.name == "a wolf" => format!("{}, hunting in the dark: wants meat, fears fire", name),
        K::Wolf => format!("{}, hunting in the dark", name),
        K::Raider | K::Beast => {
            let doing = if c.leaving { "drawing off" } else { "coming for the camp" };
            match colony.bestiary().into_iter().find(|k| k.name == c.name) {
                Some(k) => format!("{}, {}: wants {}; here because {}", name, doing, k.want, k.cause),
                None => format!("{}, {}", name, doing),
            }
        }
        K::Besieger => format!("{}, at the siege fires", name),
        K::Trader => format!("{}, a trader with a laden mule", name),
        K::CaveHunter | K::CaveLife => name,
    })
}

/// The ground at column (x, y) in words: the surface (`z` None) or a level below (`z`). A place
/// not yet found reads as the rock round it, as it is drawn.
pub(crate) fn ground_words(colony: &Colony, x: usize, y: usize, z: Option<i32>) -> Option<String> {
    use crate::local::{Material, Plant, Shape};
    let map = &colony.map;
    if x >= map.width || y >= map.height { return None; }
    let sz = map.surface_z[y * map.width + x];
    let z = z.unwrap_or(sz);
    if z < 0 || z as usize >= map.depth { return None; }
    let unfound = hidden_place(colony, x, y, z);
    let c = *map.cell(x, y, z as usize);
    let stuff = |m: Material| -> String {
        match m {
            Material::Rock(r) => format!("{:?}", r).to_lowercase(),
            Material::Block(r) => format!("dressed {:?}", r).to_lowercase(),
            Material::Ore(o) => format!("{:?} ore", o).to_lowercase(),
            Material::Wood => "timber".into(),
            Material::Magma => "magma".into(),
            m => format!("{:?}", m).to_lowercase(),
        }
    };
    if unfound {
        let rock = (0..map.depth).rev().map(|k| map.cell(x, y, k).material).find(|m| matches!(m, Material::Rock(_))).unwrap_or(Material::Rock(crate::erosion::materials::RockType::Granite));
        return Some(format!("Solid {}", stuff(rock)));
    }
    let wet = map.is_aquifer(x, y, z as usize) && c.shape == Shape::Wall;
    let mut words = match c.shape {
        Shape::Wall if c.material == Material::Magma => "The magma sea".to_string(),
        Shape::Wall => format!("{}{}", super::colony_ui::cap_pub(&stuff(c.material)), if wet { ", wet: the aquifer" } else { "" }),
        Shape::Floor | Shape::Ramp => {
            let plant = match c.plant {
                Plant::Tree(t) => format!(", a {} tree", format!("{:?}", t).to_lowercase()),
                Plant::Shrub => ", a berry bush".to_string(),
                Plant::Grass => ", grass".to_string(),
                Plant::Crop(_) => ", a crop row".to_string(),
                Plant::None => String::new(),
            };
            format!("{} {}{}{}", super::colony_ui::cap_pub(&stuff(c.material)), if c.shape == Shape::Ramp { "slope" } else { "ground" }, plant, if c.boulder { ", a boulder" } else { "" })
        }
        Shape::Stair => format!("A stair cut in the {}", stuff(c.material)),
        Shape::Empty if c.water > 0 && c.material == Material::Magma => "Magma".to_string(),
        Shape::Empty if c.water > 0 => format!("Water, {} deep", if c.water >= 6 { "over a head" } else if c.water >= 3 { "waist" } else { "ankle" }),
        Shape::Empty if z > sz => return None,
        Shape::Empty => "Open: dug out".to_string(),
    };
    let sign = map.features[y * map.width + x];
    if z == sz && sign != crate::local::wildlife::Feature::None { words.push_str(&format!("; {}", sign.name())); }
    if z < sz { words.push_str(&format!(" ({} below the ground here)", sz - z)); }
    Some(words)
}

/// Whether (x, y, z) is in a place the camp has not found: drawn as the rock round it, so it is
/// told as rock too (as `local_ink::draw_delve` covers it).
fn hidden_place(colony: &Colony, x: usize, y: usize, z: i32) -> bool {
    if std::env::var("PLANET_REVEAL").is_ok() { return false; }
    colony.map.places.iter().enumerate().any(|(k, p)| p.mouth.is_some() && !colony.places_found.contains(&k)
        && p.cells.iter().any(|&((cx, cy), f)| cx as usize == x && cy as usize == y && (f..=f + 2).contains(&z) && !colony.revealed.contains(&((cx, cy), f))))
}

/// A key to a level below: what is cut or lies in the rock in view, each with its colour.
fn draw_level_key(colony: &Colony, lcam: &LocalCamera, buf: &mut [u32], w: usize, h: usize, top: usize) {
    use crate::local::{Material, Shape};
    let map = &colony.map;
    let z = lcam.z;
    if z < 0 || z as usize >= map.depth { return; }
    let half = (w as f32 / 2.0 / lcam.tile_px, h as f32 / 2.0 / lcam.tile_px);
    let (x0, x1) = ((lcam.cx - half.0).max(0.0) as usize, ((lcam.cx + half.0) as usize).min(map.width - 1));
    let (y0, y1) = ((lcam.cy - half.1).max(0.0) as usize, ((lcam.cy + half.1) as usize).min(map.height - 1));
    let mut kinds: Vec<(String, u32)> = Vec::new();
    let mut add = |name: String, col: u32| { if !kinds.iter().any(|k| k.0 == name) { kinds.push((name, col)); } };
    let px = |c: [u8; 3]| ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
    for y in (y0..=y1).step_by(2) {
        for x in (x0..=x1).step_by(2) {
            let sz = map.surface_z[y * map.width + x];
            if z > sz { continue; }
            let c = map.cell(x, y, z as usize);
            if hidden_place(colony, x, y, z) { continue; }
            match (c.shape, c.material) {
                (Shape::Wall, Material::Ore(o)) => add(format!("{:?} ore", o).to_lowercase(), px(crate::lore::resource_color(o))),
                (Shape::Wall, Material::Magma) | (Shape::Empty, Material::Magma) => add("magma".into(), 0x00C8_4A1E),
                (Shape::Wall, _) if map.is_aquifer(x, y, z as usize) => add("wet rock: the aquifer".into(), 0x0070_90A8),
                (Shape::Wall, Material::Rock(r)) => add(format!("{:?}", r).to_lowercase(), px(super::local_ink::rock_wash(r).map(|v| v as u8))),
                (Shape::Empty, _) if c.water > 0 => add("water".into(), 0x0060_80A0),
                (Shape::Floor | Shape::Ramp | Shape::Empty, _) if z < sz => add("dug or open".into(), 0x00D8_C8A8),
                _ => {}
            }
        }
    }
    if kinds.is_empty() { return; }
    kinds.truncate(7);
    let kw = 230usize.min(w.saturating_sub(20));
    let kh = 34 + kinds.len() * 18;
    // (Below the camp's card at the top left, however tall it is.)
    let (x, y) = (10usize, top.min(h.saturating_sub(kh + 10)));
    ui::card(buf, w, Rect { x, y, w: kw, h: kh });
    let sz = map.surface_z[(lcam.cy as usize).min(map.height - 1) * map.width + (lcam.cx as usize).min(map.width - 1)];
    fonts::draw(buf, w, h, (x + 12) as f32, (y + 8) as f32, &format!("Level {}, {} down", z, (sz - z).max(0)), Face::SmallCaps, SMALL, 0.3, RUBRIC, None);
    for (k, (name, col)) in kinds.iter().enumerate() {
        let (sx, sy) = (x + 14, y + 30 + k * 18);
        for dy in 0..10 { for dx in 0..14 { ui::blend_px(buf, w, h, (sx + dx) as i64, (sy + dy) as i64, *col, 1.0); } }
        fonts::draw(buf, w, h, (sx + 22) as f32, (sy - 3) as f32, &super::colony_ui::cap_pub(name), Face::Roman, SMALL, 0.0, INK, None);
    }
}

/// Where a hover chip starts: right of the mouse, else left of it, never under the open panel
/// on the right (`st.right`).
fn chip_x(st: &HudState, w: usize, chip_w: f32) -> usize {
    let edge = w as f32 - st.right as f32 - 10.0;
    let x = if st.mouse.0 + 18.0 + chip_w <= edge { st.mouse.0 + 18.0 } else { st.mouse.0 - 18.0 - chip_w };
    x.min(edge - chip_w).max(10.0) as usize
}

/// A great moment's card, across the top middle: its title, what happened and why, until Space.
pub(crate) fn draw_moment(m: &crate::colony::Moment, colony: Option<&crate::colony::Colony>, buf: &mut [u32], w: usize, h: usize) -> Option<Rect> {
    // A roundel at the left with a picture of the moment (`vignette.rs`), when there is room.
    let pic = w >= 700 && !m.because.is_empty();
    let card_w = if pic { 640usize } else { 520 }.min(w.saturating_sub(40));
    if card_w < 200 || h < 200 { return None; }
    let pic_w = if pic { 120.0 } else { 0.0 };
    let inner = card_w as f32 - 40.0 - pic_w;
    let text = wrap_px(&m.text, Face::Roman, 17.0, inner);
    let because = wrap_px(&m.because, Face::Italic, BODY, inner);
    let card_h = (60 + text.len().min(6) * 22 + 8 + because.len().min(4) * 19 + 34).max(if pic { 170 } else { 0 });
    let r = Rect { x: (w - card_w) / 2, y: 70.min(h.saturating_sub(card_h + 10)), w: card_w, h: card_h };
    ui::card(buf, w, r);
    if pic {
        let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| ui::blend_px(buf, w, h, x, y, super::ink::pack(c), a);
        super::vignette::draw(&mut put, colony, m, (r.x + 72) as f32, (r.y + 20) as f32 + (card_h as f32 - 20.0) / 2.0, 52.0);
    }
    let x = (r.x + 20) as f32 + pic_w;
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
    if m.because.is_empty() { return Some(r); }
    let hint = if m.choice { "or Y / N" } else { "Space to go on   M: stop for moments on/off" };
    let hw = fonts::width(hint, Face::Italic, SMALL, 0.0);
    fonts::draw(buf, w, h, (r.x + card_w) as f32 - 20.0 - hw, (r.y + card_h) as f32 - 26.0, hint, Face::Italic, SMALL, 0.0, GOLD, None);
    Some(r)
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
