//! The inspector: click anything on the map and read its story.
//!
//! A parchment panel on the right of the viewer. Click a town, a ruin, a beast's lair or a realm's
//! land and it says what it is, who holds it, and its last events with their years. Every event
//! line opens the event's own page, which walks its causes back ("because") and lists what it
//! led to, so from a ruin the war that razed it is two clicks away. Pages stack: Backspace (or a
//! right click) goes back, Esc closes. The same panel will show settlers once the colony exists.

use crate::history::*;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::world::WorldData;
use super::text::{draw_ink, text_width};
use super::ui::{self, Rect, INK, INK_FADED, RUBRIC, SEA};

/// What a page is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Subject {
    Tile(usize, usize),
    Settlement(SettlementId),
    Faction(FactionId),
    Beast(LegendaryCreatureId),
    Event(EventId),
}

/// One line on a page; `link` makes it clickable.
#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub color: u32,
    pub link: Option<Subject>,
    /// A blank gap before this line (section break).
    pub gap: bool,
}

impl Line {
    fn plain(text: impl Into<String>) -> Self { Line { text: text.into(), color: INK, link: None, gap: false } }
    fn faded(text: impl Into<String>) -> Self { Line { text: text.into(), color: INK_FADED, link: None, gap: false } }
    fn section(text: impl Into<String>) -> Self { Line { text: text.into(), color: RUBRIC, link: None, gap: true } }
    fn link(text: impl Into<String>, to: Subject) -> Self { Line { text: text.into(), color: SEA, link: Some(to), gap: false } }
}

#[derive(Clone, Debug)]
pub struct Page {
    pub title: String,
    pub lines: Vec<Line>,
}

/// Events to list per page.
const RECENT: usize = 6;
/// How far back a "because" chain is walked.
const CHAIN: usize = 5;

fn faction_name(h: &WorldHistory, f: FactionId) -> String {
    h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_else(|| "a people now gone".into())
}

/// An event as one line: "213  The Kingdom of X stormed Y..." (+ "why?" when it has a cause).
fn event_line(e: &Event) -> Line {
    let why = if e.causes.is_empty() { "" } else { "  why?" };
    Line::link(format!("{}  {}{}", e.date.year, e.description.trim_end_matches('.'), why), Subject::Event(e.id))
}

fn recent<'a>(h: &'a WorldHistory, pred: impl Fn(&Event) -> bool) -> Vec<&'a Event> {
    let mut v: Vec<&Event> = h.chronicle.events.iter().rev()
        .filter(|e| !matches!(e.event_type, EventType::Raid | EventType::TradeRouteEstablished | EventType::BattleFought))
        .filter(|e| pred(e))
        .take(RECENT)
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.date));
    v
}

/// The page for a subject. A tile resolves to what stands on it (a town or ruin, then a lair,
/// then the realm whose land it is) or, failing all, to its ground.
pub fn page(world: &WorldData, h: &WorldHistory, subject: Subject) -> Page {
    match subject {
        Subject::Tile(x, y) => {
            let at = |s: &&crate::history::civilizations::settlement::Settlement| s.location == (x, y);
            if let Some(s) = h.settlements.values().filter(at).min_by_key(|s| (s.is_destroyed(), s.id)) {
                return page(world, h, Subject::Settlement(s.id));
            }
            if let Some(c) = h.legendary_creatures.values().filter(|c| c.lair_location == Some((x, y))).min_by_key(|c| (!c.is_alive(), c.id)) {
                return page(world, h, Subject::Beast(c.id));
            }
            let owner = h.tile_history.get(x, y).current_owner;
            let mut p = match owner {
                Some(f) => page(world, h, Subject::Faction(f)),
                None => Page { title: "Unclaimed land".into(), lines: Vec::new() },
            };
            p.lines.insert(0, Line::faded(format!("Tile {},{}: {:?}, {:.0} m", x, y, world.biomes.get(x, y), world.heightmap.get(x, y))));
            let here = recent(h, |e| e.location == Some((x, y)));
            if !here.is_empty() {
                p.lines.push(Line::section("Here"));
                p.lines.extend(here.into_iter().map(event_line));
            }
            p
        }
        Subject::Settlement(id) => {
            let Some(s) = h.settlements.get(&id) else { return Page { title: "?".into(), lines: Vec::new() } };
            let mut lines = Vec::new();
            match s.destroyed {
                Some(d) => {
                    lines.push(Line::plain(format!("Ruins of a {:?}, fallen in year {}", s.settlement_type, d.year).to_lowercase().replacen("ruins", "Ruins", 1)));
                    lines.push(Line::link(format!("Last held by {}", faction_name(h, s.faction)), Subject::Faction(s.faction)));
                }
                None => {
                    lines.push(Line::plain(format!("A {:?} of {} people", s.settlement_type, s.population).replacen("A ", "A ", 1)));
                    lines.push(Line::link(format!("Held by {}", faction_name(h, s.faction)), Subject::Faction(s.faction)));
                }
            }
            lines.push(Line::faded(format!("Founded in year {}", s.founded.year)));
            if let Some(sh) = h.shadow.as_ref() {
                let dark = sh.at(s.location.0, s.location.1);
                if dark >= crate::history::present::FRONTIER_CORRUPTION && s.faction != sh.faction && s.destroyed.is_none() {
                    lines.push(Line::plain(format!("On the frontier of {} (darkness {:.2})", sh.name, dark)));
                }
            }
            lines.push(Line::section("Its story"));
            let ev = recent(h, |e| e.primary_participants.contains(&EntityId::Settlement(id)) || e.location == Some(s.location));
            if ev.is_empty() { lines.push(Line::faded("Nothing recorded")); }
            lines.extend(ev.into_iter().map(event_line));
            Page { title: s.name.clone(), lines }
        }
        Subject::Faction(f) => {
            let Some(fac) = h.factions.get(&f) else { return Page { title: "?".into(), lines: Vec::new() } };
            let mut lines = Vec::new();
            let race = h.races.get(&fac.race_id).map(|r| r.name.clone()).unwrap_or_default();
            lines.push(Line::plain(format!("{} {:?}, founded {}", race, fac.government, fac.founded.year)));
            match fac.dissolved {
                Some(d) => lines.push(Line::plain(format!("Gone since year {}", d.year))),
                None => {
                    let ruler = fac.current_leader.and_then(|l| h.figures.get(&l)).map(|r| r.full_name()).unwrap_or_else(|| "no one".into());
                    lines.push(Line::plain(format!("Ruled by {}", ruler)));
                    lines.push(Line::plain(format!("{} towns, {} souls", fac.settlements.len(), fac.total_population)));
                    if let Some(cap) = fac.capital.and_then(|c| h.settlements.get(&c)) {
                        lines.push(Line::link(format!("Seat: {}", cap.name), Subject::Settlement(cap.id)));
                    }
                }
            }
            let wars: Vec<_> = h.wars.values().filter(|w| w.is_active() && (w.aggressors.contains(&f) || w.defenders.contains(&f))).collect();
            if !wars.is_empty() {
                lines.push(Line::section("At war"));
                for w in wars {
                    let line = match w.declaration_event.and_then(|e| h.chronicle.get(e)) {
                        Some(e) => Line::link(format!("{} (since {})", w.name, w.started.year), Subject::Event(e.id)),
                        None => Line::plain(format!("{} (since {})", w.name, w.started.year)),
                    };
                    lines.push(line);
                }
            }
            lines.push(Line::section("Lately"));
            lines.extend(recent(h, |e| e.factions_involved.first() == Some(&f)).into_iter().map(event_line));
            Page { title: fac.name.clone(), lines }
        }
        Subject::Beast(c) => {
            let Some(b) = h.legendary_creatures.get(&c) else { return Page { title: "?".into(), lines: Vec::new() } };
            let mut lines = Vec::new();
            lines.push(Line::plain(match b.death_date {
                Some(d) => format!("Slain in year {}", d.year),
                None => "Alive".to_string(),
            }));
            if let Some((x, y)) = b.lair_location { lines.push(Line::link(format!("Lair at {},{}", x, y), Subject::Tile(x, y))); }
            lines.push(Line::section("Deeds"));
            lines.extend(recent(h, |e| e.primary_participants.contains(&EntityId::LegendaryCreature(c))).into_iter().map(event_line));
            Page { title: b.full_name(), lines }
        }
        Subject::Event(id) => {
            let Some(e) = h.chronicle.get(id) else { return Page { title: "?".into(), lines: Vec::new() } };
            let mut lines = vec![Line::faded(format!("Year {}, {:?}", e.date.year, e.date.season))];
            lines.push(Line::plain(e.description.clone()));
            for p in &e.primary_participants {
                match p {
                    EntityId::Settlement(s) => if let Some(t) = h.settlements.get(s) { lines.push(Line::link(format!("Place: {}", t.name), Subject::Settlement(*s))); },
                    EntityId::LegendaryCreature(c) => if let Some(b) = h.legendary_creatures.get(c) { lines.push(Line::link(format!("Beast: {}", b.full_name()), Subject::Beast(*c))); },
                    _ => {}
                }
            }
            for &f in &e.factions_involved {
                lines.push(Line::link(faction_name(h, f), Subject::Faction(f)));
            }
            // Why: the chain of causes, nearest first.
            let mut chain = Vec::new();
            let mut cur = e;
            while let Some(&c) = cur.causes.first() {
                let Some(ce) = h.chronicle.get(c) else { break };
                chain.push(ce);
                if chain.len() >= CHAIN { break; }
                cur = ce;
            }
            lines.push(Line::section("Because"));
            if chain.is_empty() { lines.push(Line::faded("No cause is recorded")); }
            for (k, ce) in chain.iter().enumerate() {
                let mut l = event_line(ce);
                l.text = format!("{}{}", "  ".repeat(k), l.text);
                lines.push(l);
            }
            let led: Vec<&Event> = h.chronicle.events.iter().filter(|x| x.causes.contains(&id)).take(RECENT).collect();
            if !led.is_empty() {
                lines.push(Line::section("It led to"));
                lines.extend(led.into_iter().map(event_line));
            }
            Page { title: e.title.clone(), lines }
        }
    }
}

/// The panel's place on screen.
pub(crate) fn panel_rect(w: usize, h: usize) -> Rect {
    let pw = 380.min(w.saturating_sub(20));
    Rect { x: w.saturating_sub(pw + 10), y: 10, w: pw, h: h.saturating_sub(20) }
}

/// A drawn line's clickable area and where it leads.
pub(crate) struct Hit { pub rect: Rect, pub to: Subject }

/// Draw a page; returns the clickable lines.
pub(crate) fn draw(page: &Page, buf: &mut [u32], w: usize, h: usize, depth: usize) -> Vec<Hit> {
    let r = panel_rect(w, h);
    if r.w < 120 || r.h < 80 { return Vec::new(); }
    ui::card(buf, w, r);
    let (x0, mut y) = (r.x + 14, r.y + 14);
    let max_chars = (r.w - 28) / 7;
    for (k, line) in ui::wrap(&ui::ascii(&page.title), max_chars).iter().enumerate() {
        draw_ink(buf, w, h, x0 as i64, y as i64, line, RUBRIC, 1, k == 0);
        y += 12;
    }
    y += 4;
    ui::hline(buf, w, x0, r.x + r.w - 14, y, INK_FADED);
    y += 8;
    let mut hits = Vec::new();
    let bottom = r.y + r.h - 26;
    for line in &page.lines {
        if line.gap { y += 6; }
        let wrapped = ui::wrap(&ui::ascii(&line.text), max_chars);
        let top = y;
        for (k, part) in wrapped.iter().enumerate() {
            if y + 10 > bottom { break; }
            let indent = if k > 0 { 14 } else { 0 };
            // "why?" in red at the end of a line that has a cause.
            let (body, why) = match part.strip_suffix("why?") {
                Some(b) if k + 1 == wrapped.len() => (b, true),
                _ => (part.as_str(), false),
            };
            draw_ink(buf, w, h, (x0 + indent) as i64, y as i64, body, line.color, 1, line.gap);
            if why {
                let wx = x0 + indent + text_width(body, 1);
                draw_ink(buf, w, h, wx as i64, y as i64, "why?", RUBRIC, 1, false);
            }
            y += 11;
        }
        if let Some(to) = line.link {
            if y > top { hits.push(Hit { rect: Rect { x: x0, y: top, w: r.w - 28, h: y - top }, to }); }
        }
        if y + 10 > bottom { break; }
    }
    let foot = if depth > 1 { "click a line | Backspace: back | Esc: close" } else { "click a line to follow it | Esc: close" };
    draw_ink(buf, w, h, x0 as i64, (r.y + r.h - 20) as i64, &ui::truncate(foot, max_chars), INK_FADED, 1, false);
    hits
}

/// Headless check of the inspector: open the page for a tile, follow the given line numbers
/// (0 = the first clickable line), and save each page drawn over a parchment backdrop.
pub fn save_snapshots(world: &WorldData, h: &WorldHistory, tile: (usize, usize), follow: &[usize], prefix: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let (w, hh) = (1280usize, 800usize);
    let mut subject = Subject::Tile(tile.0, tile.1);
    let mut written = Vec::new();
    for step in 0..=follow.len() {
        let p = page(world, h, subject);
        let mut buf = vec![ui::DESK; w * hh];
        let hits = draw(&p, &mut buf, w, hh, step + 1);
        let path = format!("{prefix}_{step}.png");
        image::RgbImage::from_fn(w as u32, hh as u32, |x, y| {
            let px = buf[y as usize * w + x as usize];
            image::Rgb([(px >> 16) as u8, (px >> 8) as u8, px as u8])
        }).save(&path)?;
        println!("Inspector page {}: {} ({} links)", step, p.title, hits.len());
        written.push(path);
        let Some(&k) = follow.get(step) else { break };
        let Some(hit) = hits.get(k) else { break };
        subject = hit.to;
    }
    Ok(written)
}
