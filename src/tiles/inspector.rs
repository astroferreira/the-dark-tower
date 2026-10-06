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
    Artifact(ArtifactId),
    Monument(MonumentId),
    /// A settler of the colony, by index (the viewer builds this page: `settler_page`).
    Settler(usize),
    /// A mark left on the colony (a grave, a raised stone), by index (`mark_page`).
    ColonyMark(usize),
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
    /// Arms drawn by the title (a realm's page).
    pub emblem: Option<super::heraldry::Arms>,
    /// A face drawn by the title (a settler's page).
    pub portrait: Option<super::portraits::Portrait>,
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
                None => Page { title: "Unclaimed land".into(), lines: Vec::new(), emblem: None, portrait: None },
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
            let Some(s) = h.settlements.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None } };
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
            // What stands here and what was made here.
            let mut mons: Vec<_> = h.monuments.values().filter(|m| m.location == s.location).collect();
            mons.sort_by_key(|m| m.id);
            if !mons.is_empty() {
                lines.push(Line::section("Monuments"));
                for m in mons {
                    let state = if m.intact { String::new() } else { ", in ruins".into() };
                    lines.push(Line::link(format!("{} ({}{})", m.name, m.built_date.year, state), Subject::Monument(m.id)));
                }
            }
            let mut made: Vec<_> = h.artifacts.values().filter(|a| a.creation_location == Some(s.location)).collect();
            made.sort_by_key(|a| a.id);
            if !made.is_empty() {
                lines.push(Line::section("Made here"));
                for a in made.into_iter().take(RECENT) { lines.push(Line::link(format!("{} ({})", a.name, a.creation_date.year), Subject::Artifact(a.id))); }
            }
            lines.push(Line::section("Its story"));
            let ev = recent(h, |e| e.primary_participants.contains(&EntityId::Settlement(id)) || e.location == Some(s.location));
            if ev.is_empty() { lines.push(Line::faded("Nothing recorded")); }
            lines.extend(ev.into_iter().map(event_line));
            Page { title: s.name.clone(), lines, emblem: None, portrait: None }
        }
        Subject::Faction(f) => {
            let Some(fac) = h.factions.get(&f) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None } };
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
            Page { title: fac.name.clone(), lines, emblem: Some(super::heraldry::arms_of(world, h, f)), portrait: None }
        }
        Subject::Beast(c) => {
            let Some(b) = h.legendary_creatures.get(&c) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None } };
            let mut lines = Vec::new();
            lines.push(Line::plain(match b.death_date {
                Some(d) => format!("Slain in year {}", d.year),
                None => "Alive".to_string(),
            }));
            if let Some((x, y)) = b.lair_location { lines.push(Line::link(format!("Lair at {},{}", x, y), Subject::Tile(x, y))); }
            lines.push(Line::section("Deeds"));
            lines.extend(recent(h, |e| e.primary_participants.contains(&EntityId::LegendaryCreature(c))).into_iter().map(event_line));
            Page { title: b.full_name(), lines, emblem: None, portrait: None }
        }
        Subject::Event(id) => {
            let Some(e) = h.chronicle.get(id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None } };
            let mut lines = vec![Line::faded(format!("Year {}, {:?}", e.date.year, e.date.season))];
            lines.push(Line::plain(e.description.clone()));
            for p in &e.primary_participants {
                match p {
                    EntityId::Settlement(s) => if let Some(t) = h.settlements.get(s) { lines.push(Line::link(format!("Place: {}", t.name), Subject::Settlement(*s))); },
                    EntityId::LegendaryCreature(c) => if let Some(b) = h.legendary_creatures.get(c) { lines.push(Line::link(format!("Beast: {}", b.full_name()), Subject::Beast(*c))); },
                    EntityId::Artifact(a) => if let Some(x) = h.artifacts.get(a) { lines.push(Line::link(format!("Treasure: {}", x.name), Subject::Artifact(*a))); },
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
            // The tale it belongs to, if a reader would retell it (lore::sifting).
            for t in crate::lore::sifting::sift(h).into_iter().filter(|t| t.events.contains(&id)).take(1) {
                lines.push(Line::section(format!("A tale worth telling ({})", t.kind.label().to_lowercase())));
                lines.push(Line::plain(t.text));
                for other in t.events.iter().filter(|x| **x != id).filter_map(|x| h.chronicle.get(*x)) { lines.push(event_line(other)); }
            }
            let led: Vec<&Event> = h.chronicle.events.iter().filter(|x| x.causes.contains(&id)).take(RECENT).collect();
            if !led.is_empty() {
                lines.push(Line::section("It led to"));
                lines.extend(led.into_iter().map(event_line));
            }
            Page { title: e.title.clone(), lines, emblem: None, portrait: None }
        }
        Subject::Artifact(id) => {
            let Some(a) = h.artifacts.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None } };
            let mut lines = vec![Line::faded(format!("{:?} {:?}, made in year {}", a.quality, a.item_type, a.creation_date.year))];
            if !a.description.is_empty() { lines.push(Line::plain(a.description.clone())); }
            for i in &a.inscriptions { lines.push(Line::plain(format!("Inscribed: \"{}\"", i.text))); }
            let now = match &a.current_owner {
                _ if a.destroyed => "Destroyed".to_string(),
                Some(EntityId::Figure(f)) => h.figures.get(f).map(|x| format!("Held by {}", x.full_name())).unwrap_or_default(),
                Some(EntityId::LegendaryCreature(c)) => h.legendary_creatures.get(c).map(|x| format!("In the hoard of {}", x.full_name())).unwrap_or_default(),
                Some(EntityId::Faction(f)) => format!("Kept by {}", faction_name(h, *f)),
                _ if a.lost => "Lost".to_string(),
                _ => String::new(),
            };
            if !now.is_empty() {
                match &a.current_owner {
                    Some(EntityId::LegendaryCreature(c)) => lines.push(Line::link(now, Subject::Beast(*c))),
                    Some(EntityId::Faction(f)) => lines.push(Line::link(now, Subject::Faction(*f))),
                    _ => lines.push(Line::plain(now)),
                }
            }
            if let Some((x, y)) = a.creation_location { lines.push(Line::link(format!("Made at {},{}", x, y), Subject::Tile(x, y))); }
            lines.push(Line::section("Its story"));
            let mut ev: Vec<&Event> = h.chronicle.events.iter().filter(|e| e.primary_participants.contains(&EntityId::Artifact(id)) || Some(e.id) == a.creation_event).collect();
            ev.sort_by_key(|e| e.date);
            if ev.is_empty() { lines.push(Line::faded("Nothing recorded")); }
            lines.extend(ev.into_iter().take(RECENT * 2).map(event_line));
            Page { title: a.name.clone(), lines, emblem: None, portrait: None }
        }
        Subject::Settler(_) | Subject::ColonyMark(_) => Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None },
        Subject::Monument(id) => {
            let Some(m) = h.monuments.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None } };
            let mut lines = vec![Line::faded(format!("{:?}, raised in year {}", m.monument_type, m.built_date.year))];
            lines.push(Line::link(format!("Raised by {}", faction_name(h, m.faction)), Subject::Faction(m.faction)));
            if !m.intact { lines.push(Line::plain(format!("In ruins since year {}", m.destruction_date.map_or(0, |d| d.year)))); }
            for i in &m.inscriptions { lines.push(Line::plain(format!("Inscribed: \"{}\"", i.text))); }
            if let Some(e) = m.commemorates.and_then(|e| h.chronicle.get(e)) {
                lines.push(Line::section("It remembers"));
                lines.push(event_line(e));
            }
            if let Some(e) = m.construction_event.and_then(|e| h.chronicle.get(e)) {
                lines.push(Line::section("Its raising"));
                lines.push(event_line(e));
            }
            Page { title: m.name.clone(), lines, emblem: None, portrait: None }
        }
    }
}

/// A settler's page: who they are, what they are doing and why, their past (each line opens its
/// event) and how they feel.
pub fn settler_page(h: Option<&WorldHistory>, s: &crate::colony::Settler) -> Page {
    settler_page_with(h, s, None)
}

/// `settler_page` with the night they were wounded, if they were.
pub fn settler_page_with(h: Option<&WorldHistory>, s: &crate::colony::Settler, wounded: Option<(String, u64)>) -> Page {
    let face = super::portraits::of_settler(s, h, wounded);
    let mut lines = Vec::new();
    match &s.past {
        Some(p) => lines.push(Line::faded(format!("{}, {}", p.age, p.calling))),
        None => lines.push(Line::faded("A wanderer with no past anyone remembers")),
    }
    if !s.alive { lines.push(Line::plain("Dead")); }
    else { lines.push(Line::plain(format!("Now: {} - {}", s.job.verb(), s.why))); }
    if let Some(p) = &s.past {
        if !p.lines.is_empty() {
            lines.push(Line::section("Before"));
            for (text, ev) in &p.lines {
                match ev.filter(|e| h.map_or(false, |h| h.chronicle.get(*e).is_some())) {
                    Some(e) => lines.push(Line::link(text.clone(), Subject::Event(e))),
                    None => lines.push(Line::plain(text.clone())),
                }
            }
        }
        if let Some((text, whom)) = &p.feeling {
            lines.push(Line::section("Feels"));
            let mut line = Line::plain(capitalize(text));
            match whom {
                EntityId::Faction(f) => line = Line::link(line.text, Subject::Faction(*f)),
                EntityId::Settlement(t) => line = Line::link(line.text, Subject::Settlement(*t)),
                _ => {}
            }
            lines.push(line);
        }
        if let Some(f) = p.people.filter(|f| h.map_or(false, |h| h.factions.contains_key(f))) {
            lines.push(Line::link(format!("Of {}", h.map(|h| faction_name(h, f)).unwrap_or_default()), Subject::Faction(f)));
        }
    }
    if let Some((text, ev)) = &face.scar {
        let line = format!("Scar: {}{}", text, if face.eye_patch { "; it took an eye" } else { "" });
        lines.insert(1, match ev { Some(e) => Line::link(line, Subject::Event(*e)), None => Line::plain(line) });
    }
    Page { title: s.name.clone(), lines, emblem: None, portrait: Some(face) }
}

/// A colony mark's page: what it is, the day, and the words on it.
pub fn mark_page(m: &crate::colony::ColonyMark) -> Page {
    let kind = match m.kind { crate::colony::MarkKind::Grave => "A grave", crate::colony::MarkKind::Stone => "A raised stone", crate::colony::MarkKind::Scorch => "Scorched ground" };
    Page { title: m.title.clone(), lines: vec![
        Line::faded(format!("{}, day {}, at {},{}", kind, m.day, m.at.0, m.at.1)),
        Line::section("The words on it"),
        Line::plain(m.text.clone()),
    ], emblem: None, portrait: None }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
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
    use super::fonts::{self, Face};
    // Lettered in the map's hand (IM Fell): small-caps title, roman body, rubric sections.
    const BODY: f32 = 15.0;
    const LINE: usize = 19;
    // Faded ink dark enough to read on parchment (about 5:1).
    const SOFT: u32 = 0x005A_4634;
    let r = panel_rect(w, h);
    if r.w < 120 || r.h < 80 { return Vec::new(); }
    ui::card(buf, w, r);
    let (x0, mut y) = (r.x + 16, r.y + 14);
    let full = (r.w - 32) as f32;
    // A realm's arms or a settler's face by the title; the title wraps short of them.
    let title_w = if let Some(a) = &page.emblem {
        super::heraldry::draw(buf, w, h, (r.x + r.w - 56) as i64, (r.y + 12) as i64, 48, a);
        full - 56.0
    } else if let Some(p) = &page.portrait {
        super::portraits::draw(buf, w, h, (r.x + r.w - 70) as i64, (r.y + 8) as i64, 60, p);
        full - 70.0
    } else { full };
    for line in fonts::wrap(&page.title, Face::SmallCaps, 20.0, title_w) {
        fonts::draw(buf, w, h, x0 as f32, y as f32, &line, Face::SmallCaps, 20.0, 0.5, RUBRIC, None);
        y += 24;
    }
    if page.portrait.is_some() { y = y.max(r.y + 74); }
    y += 2;
    ui::hline(buf, w, x0, r.x + r.w - 16, y, INK_FADED);
    y += 8;
    let mut hits = Vec::new();
    let bottom = r.y + r.h - 28;
    for line in &page.lines {
        if line.gap { y += 8; }
        let (face, color) = if line.gap && line.color == RUBRIC { (Face::SmallCaps, RUBRIC) }
            else if line.color == INK_FADED { (Face::Italic, SOFT) } else { (Face::Roman, line.color) };
        let wrapped = fonts::wrap(&line.text, face, BODY, full - 14.0);
        let top = y;
        for (k, part) in wrapped.iter().enumerate() {
            if y + LINE > bottom { break; }
            let indent = if k > 0 { 14.0 } else { 0.0 };
            // "why?" in red at the end of a line that has a cause.
            let (body, why) = match part.strip_suffix("why?") {
                Some(b) if k + 1 == wrapped.len() => (b, true),
                _ => (part.as_str(), false),
            };
            fonts::draw(buf, w, h, x0 as f32 + indent, y as f32, body, face, BODY, 0.0, color, None);
            if why {
                let wx = x0 as f32 + indent + fonts::width(body, face, BODY, 0.0);
                fonts::draw(buf, w, h, wx, y as f32, "why?", Face::Italic, BODY, 0.0, RUBRIC, None);
            }
            y += LINE;
        }
        if let Some(to) = line.link {
            if y > top { hits.push(Hit { rect: Rect { x: x0, y: top, w: r.w - 32, h: y - top }, to }); }
        }
        if y + LINE > bottom { break; }
    }
    let foot = if depth > 1 { "click a line  \u{b7}  Backspace: back  \u{b7}  Esc: close" } else { "click a line to follow it  \u{b7}  Esc: close" };
    fonts::draw(buf, w, h, x0 as f32, (r.y + r.h - 24) as f32, foot, Face::Italic, 13.0, 0.0, SOFT, None);
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
