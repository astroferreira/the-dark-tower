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
    /// A person of the history: life, kin, who they are, deeds.
    Figure(FigureId),
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
    /// A picture drawn by the title: a beast as its sprite, a thing as its glyph.
    pub picture: Option<Picture>,
}

/// What a page's picture shows.
#[derive(Clone, Debug)]
pub enum Picture { Beast(super::beasts::Look), Thing(super::glyphs::Glyph) }

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
                None => Page { title: if *world.heightmap.get(x, y) < 0.0 { "Open water".into() } else { "Unclaimed land".into() }, lines: Vec::new(), emblem: None, portrait: None, picture: None },
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
            let Some(s) = h.settlements.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
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
            // What word has reached it, as its people tell it (`history::knowledge`).
            if s.destroyed.is_none() { lines.extend(knowledge_lines(h, Subject::Settlement(id))); }
            Page { title: s.name.clone(), lines, emblem: None, portrait: None, picture: None }
        }
        Subject::Faction(f) => {
            let Some(fac) = h.factions.get(&f) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
            let mut lines = Vec::new();
            let race = h.races.get(&fac.race_id).map(|r| r.name.clone()).unwrap_or_default();
            lines.push(Line::plain(format!("{} {:?}, founded {}", race, fac.government, fac.founded.year)));
            match fac.dissolved {
                Some(d) => lines.push(Line::plain(format!("Gone since year {}", d.year))),
                None => {
                    match fac.current_leader.and_then(|l| h.figures.get(&l)) {
                        Some(r) => lines.push(Line::link(format!("Ruled by {}", r.full_name()), Subject::Figure(r.id))),
                        None => lines.push(Line::plain("Ruled by no one")),
                    }
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
            // Their arts (`arts.rs`): instruments, then each work and who made it.
            let arts = crate::history::arts::of_people(h, f);
            if !arts.forms.is_empty() {
                lines.push(Line::section("Arts"));
                for i in &arts.instruments { lines.push(Line::faded(capitalize(&i.describe()))); }
                for (form, text) in arts.forms.iter().zip(arts.lines(h)) {
                    let text = capitalize(&text);
                    lines.push(match form.author { Some(a) => Line::link(text, Subject::Figure(a)), None => Line::plain(text) });
                }
            }
            lines.push(Line::section("Lately"));
            lines.extend(recent(h, |e| e.factions_involved.first() == Some(&f)).into_iter().map(event_line));
            Page { title: fac.name.clone(), lines, emblem: Some(super::heraldry::arms_of(world, h, f)), portrait: None, picture: None }
        }
        Subject::Figure(id) => figure_page(h, id),
        Subject::Beast(c) => {
            let Some(b) = h.legendary_creatures.get(&c) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
            let mut lines = Vec::new();
            lines.push(Line::plain(match b.death_date {
                Some(d) => format!("Slain in year {}", d.year),
                None => "Alive".to_string(),
            }));
            if let Some((x, y)) = b.lair_location { lines.push(Line::link(format!("Lair at {},{}", x, y), Subject::Tile(x, y))); }
            // What it is (`monsters.rs`): its body and its danger, in words its making owns.
            let m = crate::monsters::of_legend(h, b);
            lines.push(Line::section("What it is"));
            lines.push(Line::plain(m.description.clone()));
            lines.push(Line::faded(format!("Its blood is {}{}.", m.blood, if m.flies { "; it flies" } else { "" })));
            lines.push(Line::section("Deeds"));
            lines.extend(recent(h, |e| e.primary_participants.contains(&EntityId::LegendaryCreature(c))).into_iter().map(event_line));
            Page { title: b.full_name(), lines, emblem: None, portrait: None, picture: Some(Picture::Beast(super::beasts::of_monster(&m))) }
        }
        Subject::Event(id) => {
            let Some(e) = h.chronicle.get(id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
            let mut lines = vec![Line::faded(format!("Year {}, {:?}", e.date.year, e.date.season))];
            // Where it belongs: its age, its war, its battle (`collections.rs`).
            for (what, opened) in crate::history::collections::context(h, e) {
                let text = format!("Part of {}", what.replacen("The ", "the ", 1));
                lines.push(match opened.filter(|o| *o != e.id) { Some(o) => Line::link(text, Subject::Event(o)), None => Line::faded(text) });
            }
            lines.push(Line::plain(e.description.clone()));
            for p in &e.primary_participants {
                match p {
                    EntityId::Settlement(s) => if let Some(t) = h.settlements.get(s) { lines.push(Line::link(format!("Place: {}", t.name), Subject::Settlement(*s))); },
                    EntityId::LegendaryCreature(c) => if let Some(b) = h.legendary_creatures.get(c) { lines.push(Line::link(format!("Beast: {}", b.full_name()), Subject::Beast(*c))); },
                    EntityId::Artifact(a) => if let Some(x) = h.artifacts.get(a) { lines.push(Line::link(format!("Treasure: {}", x.name), Subject::Artifact(*a))); },
                    EntityId::Figure(f) => if let Some(x) = h.figures.get(f) { lines.push(Line::link(format!("Person: {}", x.full_name()), Subject::Figure(*f))); },
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
            // How far word of it went, and how each people it touched tells it (`history::knowledge`).
            lines.extend(knowledge_lines(h, Subject::Event(id)));
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
            Page { title: e.title.clone(), lines, emblem: None, portrait: None, picture: None }
        }
        Subject::Artifact(id) => {
            let Some(a) = h.artifacts.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
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
            let g = super::glyphs::Glyph::of_thing(&format!("{:?} {}", a.item_type, a.name));
            Page { title: a.name.clone(), lines, emblem: None, portrait: None, picture: Some(Picture::Thing(if g == super::glyphs::Glyph::Work { super::glyphs::Glyph::Chest } else { g })) }
        }
        Subject::Settler(_) | Subject::ColonyMark(_) => Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None },
        Subject::Monument(id) => {
            let Some(m) = h.monuments.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
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
            Page { title: m.name.clone(), lines, emblem: None, portrait: None, picture: None }
        }
    }
}

thread_local! {
    /// The last knowledge section built, by history and subject: pages are rebuilt every frame
    /// and these read the whole recent chronicle against every town.
    static KNOWN: std::cell::RefCell<Option<((usize, usize), Subject, Vec<Line>)>> = std::cell::RefCell::new(None);
}

/// What a town has heard ("Word heard here"), or how far word of an event went and how each
/// people it touched tells it ("How it is told"), from `history::knowledge`.
fn knowledge_lines(h: &WorldHistory, subject: Subject) -> Vec<Line> {
    use crate::history::knowledge::{account, tell_verb, Knowledge};
    let key = (h as *const WorldHistory as usize, h.chronicle.events.len());
    if let Some(l) = KNOWN.with(|c| c.borrow().as_ref().filter(|x| x.0 == key && x.1 == subject).map(|x| x.2.clone())) { return l; }
    let mut lines = Vec::new();
    match subject {
        Subject::Settlement(id) => {
            let news = Knowledge::new(h).news_of_town(id, 30, 5);
            if !news.is_empty() {
                lines.push(Line::section("Word heard here"));
                for t in news { lines.push(Line::link(capitalize(&t.line()), Subject::Event(t.event))); }
            }
        }
        Subject::Event(id) => if let Some(e) = h.chronicle.get(id) {
            let told = account(h, e, None);
            let (n, m) = Knowledge::new(h).known_in(e);
            lines.push(Line::section("How it is told"));
            lines.push(Line::faded(format!("Known in {} of {} living towns", n, m)));
            if let Some(g) = &told.gloss { lines.push(Line::plain(format!("Plainly: {}", g))); }
            for (f, name, g) in &told.others {
                lines.push(Line::link(format!("As {} {} it: {}", name, tell_verb(name), g), Subject::Faction(*f)));
            }
        },
        _ => {}
    }
    KNOWN.with(|c| *c.borrow_mut() = Some((key, subject, lines.clone())));
    lines
}

/// A settler's page: who they are, what they are doing and why, their past (each line opens its
/// event) and how they feel.
pub fn settler_page(h: Option<&WorldHistory>, s: &crate::colony::Settler) -> Page {
    settler_page_with(h, s, None, &[])
}

/// A person of the history: born and died, people and home, kin (each a link), who they are
/// (their persona, consistent with the personality the history acted on) and their deeds.
pub fn figure_page(h: &WorldHistory, id: FigureId) -> Page {
    let Some(f) = h.figures.get(&id) else { return Page { title: "?".into(), lines: Vec::new(), emblem: None, portrait: None, picture: None } };
    let mut lines = Vec::new();
    let age = match f.death_date { Some(d) => f.age_at(&d), None => f.age_at(&h.current_date) };
    lines.push(Line::faded(match (f.death_date, &f.cause_of_death) {
        (Some(d), Some(c)) => format!("Born {}, died {} aged {} ({})", f.birth_date.year, d.year, age, format!("{:?}", c).to_lowercase()),
        (Some(d), None) => format!("Born {}, died {} aged {}", f.birth_date.year, d.year, age),
        _ => format!("Born {}, aged {}", f.birth_date.year, age),
    }));
    if !f.titles.is_empty() { lines.push(Line::plain(f.titles.join(", "))); }
    if let Some(r) = h.people.as_ref().and_then(|p| p.role.get(&id)) { lines.push(Line::plain(format!("The {}", r.word()))); }
    if let Some(fac) = f.faction { lines.push(Line::link(format!("Of {}", faction_name(h, fac)), Subject::Faction(fac))); }
    if let Some(t) = h.people.as_ref().and_then(|p| p.home.get(&id)).and_then(|t| h.settlements.get(t)) { lines.push(Line::link(format!("Lives at {}", t.name), Subject::Settlement(t.id))); }
    // Kin: never deleted, so a dead spouse is still a spouse.
    let kin: Vec<(String, FigureId)> = [f.parents.0, f.parents.1].iter().flatten().map(|p| ("Parent", *p))
        .chain(f.spouse.map(|s| ("Spouse", s)))
        .chain(f.children.iter().map(|c| ("Child", *c)))
        .chain(f.mentors.iter().map(|m| ("Mentor", *m)))
        .chain(f.enemies.iter().map(|m| ("Enemy", *m)))
        .filter_map(|(w, k)| h.figures.get(&k).map(|x| (format!("{}: {}{}", w, x.full_name(), if x.is_alive() { "" } else { " (dead)" }), k)))
        .collect();
    if !kin.is_empty() {
        lines.push(Line::section("Kin and others"));
        for (t, k) in kin.into_iter().take(10) { lines.push(Line::link(t, Subject::Figure(k))); }
    }
    lines.push(Line::section("Who"));
    for para in crate::persona::Persona::of_figure(h, f).describe(&f.name, age) { lines.push(Line::plain(para)); }
    let mut deeds: Vec<&Event> = f.events.iter().filter_map(|e| h.chronicle.get(*e)).collect();
    deeds.sort_by_key(|e| (e.date, e.id));
    if !deeds.is_empty() {
        lines.push(Line::section("Deeds"));
        lines.extend(deeds.into_iter().rev().take(8).map(event_line));
    }
    Page { title: f.full_name(), lines, emblem: None, portrait: None, picture: None }
}

/// `settler_page` with the night they were wounded, if they were.
pub fn settler_page_with(h: Option<&WorldHistory>, s: &crate::colony::Settler, wounded: Option<(String, u64)>, about: &[String]) -> Page {
    let face = super::portraits::of_settler(s, h, wounded);
    let mut lines = Vec::new();
    match &s.past {
        Some(p) => lines.push(Line::faded(format!("{}, {}", p.age, p.calling))),
        None => lines.push(Line::faded("A wanderer with no past anyone remembers")),
    }
    if let Some(k) = s.role { lines.push(Line::plain(format!("The camp's {}", crate::colony::ROLES[k]))); }
    if let Some(o) = &s.office { lines.push(Line::plain(o.clone())); }
    // What the camp knows of them besides: family, pet, arms, guest (`Colony::about`).
    for a in about { lines.push(Line::plain(a.clone())); }
    if s.drill >= 0.05 { lines.push(Line::plain(format!("Drilled with the spear ({})", if s.drill >= 0.4 { "a seasoned hand" } else if s.drill >= 0.2 { "steady" } else { "green" }))); }
    if let Some((rel, god)) = s.past.as_ref().and_then(|p| p.faith.clone()) {
        let devout = s.persona.facet(crate::persona::Facet::Piety) >= 60;
        lines.push(Line::faded(format!("{} {} ({})", if devout { "Worships" } else { "Of the faith of" }, god, rel)));
    }
    // Who they are: looks, gifts, character, values, likes (`persona.rs`).
    let age = s.past.as_ref().map_or(30, |p| p.age);
    let who = s.persona.describe(&s.name, age);
    {
        let best = (0..5).max_by(|&a, &b| s.skill[a].total_cmp(&s.skill[b])).unwrap_or(0);
        lines.push(Line::faded(format!("Best at {} ({:.0}% of a master's hand); {} loads laid", ["foraging", "fishing", "felling", "carrying", "building"][best], s.skill[best] * 100.0, s.loads_laid)));
    }
    if !s.alive && !s.mind.left { lines.push(Line::plain("Dead")); }
    else { lines.push(Line::plain(format!("Now: {} - {}", s.job.verb(), s.why))); }
    if s.alive {
        // How they feel, and why (Dwarf Fortress's thoughts): the mood, then the latest thoughts.
        lines.push(Line::section(format!("Feels {}", crate::colony::mind::mood(s.mind.stress))));
        if let Some((b, _)) = s.mind.broken { lines.push(Line::plain(format!("In the grip of {}", b.word()))); }
        for t in s.mind.thoughts.iter().rev().take(4) {
            let line = format!("Day {}: {}", t.tick / crate::colony::TICKS_PER_DAY + 1, t.text);
            lines.push(if t.weight < 0.0 { Line::plain(line) } else { Line::faded(line) });
        }
    } else if s.mind.left {
        lines.push(Line::plain("Left the camp for good"));
    }
    let _ = &s.mind;
    if !s.deeds.is_empty() {
        lines.push(Line::section("Deeds"));
        for d in &s.deeds { lines.push(Line::plain(capitalize(d))); }
    }
    if !s.made.is_empty() {
        lines.push(Line::section("Made"));
        for m in s.made.iter().rev().take(4) { lines.push(Line::faded(capitalize(m))); }
    }
    if !s.wounds.is_empty() {
        lines.push(Line::section("Wounds"));
        for w in &s.wounds { lines.push(Line::plain(format!("{} from {}", capitalize(&w.word()), w.from))); }
    }
    if !s.persona.race.is_empty() {
        lines.push(Line::section("Who"));
        for para in who { lines.push(Line::plain(para)); }
    }
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
    Page { title: s.name.clone(), lines, emblem: None, portrait: Some(face), picture: None }
}

/// A colony mark's page: what it is, the day, and the words on it.
pub fn mark_page(m: &crate::colony::ColonyMark) -> Page {
    let kind = match m.kind { crate::colony::MarkKind::Grave => "A grave", crate::colony::MarkKind::Stone => "A raised stone", crate::colony::MarkKind::Scorch => "Scorched ground", crate::colony::MarkKind::Cage => "A cage trap",
        crate::colony::MarkKind::Cairn => "A cairn", crate::colony::MarkKind::Bench => "A bench", crate::colony::MarkKind::Carving => "A carved post" };
    Page { title: m.title.clone(), lines: vec![
        Line::faded(format!("{}, day {}, at {},{}", kind, m.day, m.at.0, m.at.1)),
        Line::section("The words on it"),
        Line::plain(m.text.clone()),
    ], emblem: None, portrait: None, picture: None }
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
    } else if let Some(pic) = &page.picture {
        let (cx, cy) = ((r.x + r.w - 52) as f32, (r.y + 40) as f32);
        match pic {
            Picture::Beast(look) => {
                let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| ui::blend_px(buf, w, h, x, y, super::ink::pack(c), a);
                super::beasts::draw(&mut put, look, cx, cy + 30.0, if look.flies { 90.0 } else { 116.0 }, true, super::beasts::Pose::Stand, 1.0);
            }
            Picture::Thing(g) => super::glyphs::draw_u32(buf, w, h, *g, cx, cy, 44.0, None),
        }
        full - 96.0
    } else { full };
    for line in fonts::wrap(&page.title, Face::SmallCaps, 20.0, title_w) {
        fonts::draw(buf, w, h, x0 as f32, y as f32, &line, Face::SmallCaps, 20.0, 0.5, RUBRIC, None);
        y += 24;
    }
    if page.portrait.is_some() || page.picture.is_some() { y = y.max(r.y + 80); }
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
