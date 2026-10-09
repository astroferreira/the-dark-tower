//! Legends: the whole history of the world as a folder of cross-linked HTML pages
//! (`--legends DIR`), after Dwarf Fortress's legends mode.
//!
//! The chronicle is the record (DF's idea: an append-only log of typed events, with the text
//! written when it is read). Every people, site, figure, beast, treasure, monument, war, faith,
//! age and year gets a page; each page's story is its events in order, with the names in them
//! linked. Every event has an entry on its year's page with where it belongs (its age, war and
//! battle, `collections.rs`), its causes ("Because", five links back) and what it led to, like the
//! inspector. Sites and realms are pinned on one rendered ink map; realms show their arms. The
//! legends of earlier camps (`colonies/legends_<seed>.json`) get pages of their own.
//!
//! Pure and deterministic: everything is read from the history in id order, nothing is drawn
//! from an RNG, and the same seed writes byte-identical pages. The style is the journal's
//! (parchment, IM Fell, sepia, red rubrics, a dark theme). A search box on every page looks up
//! any name (`legends.js` holds the index).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use crate::colony::legend::Legend;
use crate::history::det::HashMap;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::*;
use crate::lore::{FeatureKind, Gazetteer};
use crate::world::WorldData;

/// How far back an entry's "Because" walks.
const CHAIN: usize = 5;
/// Events folded into a tally on the longer pages (peoples, sites).
const ROUTINE: [EventType; 3] = [EventType::TradeRouteEstablished, EventType::Raid, EventType::SettlementGrew];

/// What was written.
#[derive(Debug, Default)]
pub struct Report {
    pub pages: usize,
    pub peoples: usize,
    pub sites: usize,
    pub figures: usize,
    pub beasts: usize,
    pub treasures: usize,
    pub wars: usize,
    pub faiths: usize,
    pub years: usize,
    pub camps: usize,
}

impl Report {
    pub fn line(&self) -> String {
        format!("{} pages: {} peoples, {} sites, {} figures, {} beasts, {} treasures and monuments, {} wars, {} faiths, {} years, {} camps",
            self.pages, self.peoples, self.sites, self.figures, self.beasts, self.treasures, self.wars, self.faiths, self.years, self.camps)
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

fn tidy(s: &str) -> String {
    s.replace("The The ", "The ").replace("the The ", "the ")
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// "ElementalControl" -> "elemental control".
fn words(camel: &str) -> String {
    let mut s = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 { s.push(' '); }
        s.extend(c.to_lowercase());
    }
    s
}

fn plural(n: usize, one: &str, many: &str) -> String { format!("{} {}", n, if n == 1 { one } else { many }) }

fn a(url: &str, text: &str) -> String { format!("<a href=\"{}\">{}</a>", url, esc(text)) }

/// "A", "A and B", "A, B and C".
fn and_list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

fn death_words(cause: &str) -> &'static str {
    match cause {
        "Natural" => "died of age",
        "Battle" => "fell in battle",
        "Assassination" => "was assassinated",
        "Execution" => "was executed",
        "Duel" => "was killed in a duel",
        "Monster" => "was slain by a monster",
        "Disease" => "was taken by sickness",
        "Magic" => "was killed by sorcery",
        "Accident" => "died by mishap",
        "Suicide" => "died by their own hand",
        _ => "died",
    }
}

/// The kind of an event, for its margin mark (the journal's kinds).
fn kind_of(t: &EventType) -> &'static str {
    use EventType::*;
    match t {
        WarDeclared | WarEnded | BattleFought | SiegeBegun | SiegeEnded | Raid | Massacre | HolyWarDeclared => "war",
        FactionDestroyed | SettlementDestroyed | ShadowConquest => "fall",
        CreatureAppeared | CreatureSlain | MonsterRaid | LairEstablished | LairDestroyed | QuestBegun | QuestCompleted => "beast",
        FactionFounded | SettlementFounded => "found",
        Authored => "story",
        _ => "other",
    }
}

/// Everything the pages share: indexes over the chronicle, built once.
struct Book<'a> {
    h: &'a WorldHistory,
    gaz: &'a Gazetteer,
    world_name: String,
    seed: u64,
    map: Option<(u32, u32)>,
    map_tiles: (usize, usize),
    /// Events sorted by (date, id).
    events: Vec<&'a Event>,
    pos: HashMap<EventId, usize>,
    by_entity: HashMap<EntityId, Vec<usize>>,
    by_faction: HashMap<FactionId, Vec<usize>>,
    by_tile: HashMap<(usize, usize), Vec<usize>>,
    led_to: HashMap<EventId, Vec<usize>>,
    /// Settlements by tile (every one that stood there), lowest id first.
    sites_at: HashMap<(usize, usize), Vec<SettlementId>>,
    fight_war: HashMap<EventId, WarId>,
    decl_war: HashMap<EventId, WarId>,
    tale_of: HashMap<EventId, usize>,
    tales: Vec<crate::lore::sifting::Tale>,
    arms: BTreeSet<FactionId>,
    /// Names borne by one thing only (sites, figures, beasts, treasures, monuments, peoples,
    /// faiths, wars), by their first word other than "The": (url, name). Links the names an
    /// event mentions but does not list.
    by_word: HashMap<String, Vec<(String, String)>>,
}

fn sorted<K: Ord + Copy, V>(m: &HashMap<K, V>) -> Vec<(K, &V)> {
    let mut v: Vec<(K, &V)> = m.iter().map(|(k, v)| (*k, v)).collect();
    v.sort_by_key(|(k, _)| *k);
    v
}

impl<'a> Book<'a> {
    fn new(world: &WorldData, h: &'a WorldHistory, gaz: &'a Gazetteer, map: Option<(u32, u32)>) -> Self {
        let mut events: Vec<&Event> = h.chronicle.events.iter().collect();
        events.sort_by_key(|e| (e.date, e.id));
        let pos: HashMap<EventId, usize> = events.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        let mut sites_at: HashMap<(usize, usize), Vec<SettlementId>> = HashMap::default();
        for (id, s) in sorted(&h.settlements) { sites_at.entry(s.location).or_default().push(id); }
        let mut by_entity: HashMap<EntityId, Vec<usize>> = HashMap::default();
        let mut by_faction: HashMap<FactionId, Vec<usize>> = HashMap::default();
        let mut by_tile: HashMap<(usize, usize), Vec<usize>> = HashMap::default();
        let mut led_to: HashMap<EventId, Vec<usize>> = HashMap::default();
        for (i, e) in events.iter().enumerate() {
            let mut seen: Vec<EntityId> = Vec::new();
            for p in &e.primary_participants {
                if !seen.contains(p) { seen.push(p.clone()); by_entity.entry(p.clone()).or_default().push(i); }
            }
            let mut fs: Vec<FactionId> = Vec::new();
            for f in &e.factions_involved { if !fs.contains(f) { fs.push(*f); by_faction.entry(*f).or_default().push(i); } }
            if let Some(l) = e.location { by_tile.entry(l).or_default().push(i); }
            for c in &e.causes { led_to.entry(*c).or_default().push(i); }
        }
        let mut fight_war = HashMap::default();
        let mut decl_war = HashMap::default();
        for (id, w) in sorted(&h.wars) {
            for b in w.battles.iter().chain(w.sieges.iter()) { fight_war.entry(*b).or_insert(id); }
            if let Some(d) = w.declaration_event { decl_war.entry(d).or_insert(id); }
        }
        let tales = crate::lore::sifting::sift(h);
        let mut tale_of = HashMap::default();
        for (i, t) in tales.iter().enumerate() { for e in &t.events { tale_of.entry(*e).or_insert(i); } }
        let world_name = gaz.features.iter().filter(|f| f.kind == FeatureKind::Continent).max_by_key(|f| (f.size, std::cmp::Reverse(f.name.clone())))
            .map(|f| f.name.clone()).unwrap_or_else(|| "the World".into());
        let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut add = |name: String, url: String| { if name.len() >= 3 { named.entry(name).or_default().insert(url); } };
        for (id, x) in sorted(&h.settlements) { add(x.name.clone(), format!("site-{}.html", id.0)); }
        for (id, x) in sorted(&h.figures) { add(x.full_name(), format!("figure-{}.html", id.0)); add(x.name.clone(), format!("figure-{}.html", id.0)); }
        for (id, x) in sorted(&h.legendary_creatures) { add(x.full_name(), format!("beast-{}.html", id.0)); add(x.name.clone(), format!("beast-{}.html", id.0)); }
        for (id, x) in sorted(&h.artifacts) { add(x.name.clone(), format!("artifact-{}.html", id.0)); }
        for (id, x) in sorted(&h.monuments) { add(x.name.clone(), format!("monument-{}.html", id.0)); }
        for (id, x) in sorted(&h.factions) { add(tidy(&x.name), format!("people-{}.html", id.0)); }
        for (id, x) in sorted(&h.religions) { add(tidy(&x.name), format!("religion-{}.html", id.0)); }
        for (id, x) in sorted(&h.wars) { add(tidy(&x.name), format!("war-{}.html", id.0)); }
        let mut by_word: HashMap<String, Vec<(String, String)>> = HashMap::default();
        for (name, urls) in named {
            if urls.len() != 1 { continue; }
            let Some(key) = name.split_whitespace().find(|w| *w != "The") else { continue };
            by_word.entry(key.to_string()).or_default().push((urls.into_iter().next().unwrap(), name));
        }
        Book { h, gaz, world_name, seed: world.seed(), by_word, map, map_tiles: (world.width, world.height), events, pos, by_entity, by_faction, by_tile, led_to, sites_at, fight_war, decl_war, tale_of, tales, arms: BTreeSet::new() }
    }

    fn ev(&self, id: EventId) -> Option<&'a Event> { self.pos.get(&id).map(|&i| self.events[i]) }

    fn ev_url(e: &Event) -> String { format!("year-{}.html#e{}", e.date.year, e.id.0) }

    fn url(&self, id: EntityId) -> Option<String> {
        let h = self.h;
        match id {
            EntityId::Faction(f) if h.factions.contains_key(&f) => Some(format!("people-{}.html", f.0)),
            EntityId::Settlement(s) if h.settlements.contains_key(&s) => Some(format!("site-{}.html", s.0)),
            EntityId::Figure(f) if h.figures.contains_key(&f) => Some(format!("figure-{}.html", f.0)),
            EntityId::LegendaryCreature(c) if h.legendary_creatures.contains_key(&c) => Some(format!("beast-{}.html", c.0)),
            EntityId::Artifact(x) if h.artifacts.contains_key(&x) => Some(format!("artifact-{}.html", x.0)),
            EntityId::Monument(m) if h.monuments.contains_key(&m) => Some(format!("monument-{}.html", m.0)),
            EntityId::Religion(r) if h.religions.contains_key(&r) => Some(format!("religion-{}.html", r.0)),
            _ => None,
        }
    }

    /// The names an entity goes by in text, longest first.
    fn names(&self, id: EntityId) -> Vec<String> {
        let h = self.h;
        match id {
            EntityId::Faction(f) => h.factions.get(&f).map(|x| vec![tidy(&x.name)]).unwrap_or_default(),
            EntityId::Settlement(s) => h.settlements.get(&s).map(|x| vec![x.name.clone()]).unwrap_or_default(),
            EntityId::Figure(f) => h.figures.get(&f).map(|x| { let mut v = vec![x.full_name()]; if x.epithet.is_some() { v.push(x.name.clone()); } v }).unwrap_or_default(),
            EntityId::LegendaryCreature(c) => h.legendary_creatures.get(&c).map(|x| vec![x.full_name(), x.name.clone()]).unwrap_or_default(),
            EntityId::Artifact(x) => h.artifacts.get(&x).map(|x| vec![x.name.clone()]).unwrap_or_default(),
            EntityId::Monument(m) => h.monuments.get(&m).map(|x| vec![x.name.clone()]).unwrap_or_default(),
            EntityId::Religion(r) => h.religions.get(&r).map(|x| vec![tidy(&x.name)]).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The display name of an entity.
    fn name(&self, id: EntityId) -> String { self.names(id).into_iter().next().unwrap_or_else(|| "someone now forgotten".into()) }

    /// A link to an entity, or its name.
    fn link(&self, id: EntityId) -> String {
        let n = self.name(id.clone());
        match self.url(id) { Some(u) => a(&u, &n), None => esc(&n) }
    }

    fn people(&self, f: FactionId) -> String { self.link(EntityId::Faction(f)) }

    /// `text` escaped, with the names of the event's participants (and the town on its tile, and
    /// the wars it names) linked at their first mention. A short name never matches inside a
    /// longer one: longer names are swapped for placeholders first.
    fn linked(&self, e: &Event, text: &str) -> String {
        let mut cands: Vec<(String, String)> = Vec::new();
        let push = |id: EntityId, me: &Self, c: &mut Vec<(String, String)>| {
            if let Some(u) = me.url(id.clone()) { for n in me.names(id) { c.push((u.clone(), n)); } }
        };
        for p in &e.primary_participants { push(p.clone(), self, &mut cands); }
        for f in &e.factions_involved { push(EntityId::Faction(*f), self, &mut cands); }
        if let Some(l) = e.location { for s in self.sites_at.get(&l).into_iter().flatten() { push(EntityId::Settlement(*s), self, &mut cands); } }
        for f in &e.factions_involved {
            if let Some(fac) = self.h.factions.get(f) {
                for w in &fac.wars {
                    if let Some(war) = self.h.wars.get(w) {
                        let n = tidy(&war.name);
                        if text.contains(&n) { cands.push((format!("war-{}.html", w.0), n)); }
                    }
                }
            }
        }
        cands.extend(self.named_in(text));
        for f in &e.factions_involved {
            if let Some(r) = self.h.factions.get(f).and_then(|x| x.state_religion).filter(|r| self.h.religions.contains_key(r)) {
                let n = tidy(&self.h.religions[&r].name);
                if text.contains(&n) { cands.push((format!("religion-{}.html", r.0), n)); }
            }
        }
        self.link_names(&tidy(text), cands)
    }

    /// The things `text` names that only one thing bears (`by_word`).
    fn named_in(&self, text: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for w in text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-')) {
            let w = w.trim_end_matches("'s");
            if !w.starts_with(|c: char| c.is_uppercase()) { continue; }
            for (u, n) in self.by_word.get(w).into_iter().flatten() {
                if text.contains(n.as_str()) { out.push((u.clone(), n.clone())); }
            }
        }
        out
    }

    fn link_names(&self, text: &str, mut names: Vec<(String, String)>) -> String {
        names.sort_by(|x, y| y.1.len().cmp(&x.1.len()).then(x.0.cmp(&y.0)));
        let mut out = text.to_string();
        let mut subs: Vec<String> = Vec::new();
        let mut done: Vec<String> = Vec::new();
        for (url, name) in names {
            if name.len() < 3 || done.contains(&url) { continue; }
            if let Some(i) = out.find(&name) {
                let key = format!("\u{1}{}\u{2}", subs.len());
                out.replace_range(i..i + name.len(), &key);
                subs.push(a(&url, &name));
                done.push(url);
            }
        }
        let mut html = esc(&out);
        for (k, s) in subs.iter().enumerate() { html = html.replace(&format!("\u{1}{}\u{2}", k), s); }
        html
    }

    /// One event in a story: its year (linked to its entry), its text with names linked, and a
    /// "why?" when it has a cause.
    fn line(&self, e: &Event) -> String {
        let why = if e.causes.is_empty() { String::new() } else { format!(" <a class=\"why\" href=\"{}\">why?</a>", Self::ev_url(e)) };
        format!("<li><a class=\"yr\" href=\"{}\">{}</a> {}{}</li>", Self::ev_url(e), e.date.year, self.linked(e, &e.description), why)
    }

    /// A story: the events in order, routine ones (`ROUTINE`) folded into a tally when `fold`.
    fn story(&self, idx: &[usize], fold: bool) -> String {
        let mut o = String::from("<ol class=\"story\">");
        let mut tally: BTreeMap<(&'static str, &'static str), usize> = BTreeMap::new();
        let mut any = false;
        for &i in idx {
            let e = self.events[i];
            if fold && ROUTINE.contains(&e.event_type) {
                let k = match e.event_type { EventType::TradeRouteEstablished => ("trade route opened", "trade routes opened"), EventType::Raid => ("raid", "raids"), _ => ("year of growth", "years of growth") };
                *tally.entry(k).or_default() += 1;
                continue;
            }
            o.push_str(&self.line(e));
            any = true;
        }
        o.push_str("</ol>");
        if !any { o = "<p class=\"meta\">Nothing is recorded.</p>".into(); }
        if !tally.is_empty() {
            let t: Vec<String> = tally.iter().map(|((one, many), n)| plural(*n, one, many)).collect();
            let _ = write!(o, "<p class=\"meta\">Also in the record: {}.</p>", and_list(&t));
        }
        o
    }

    /// A people's arts (`arts.rs`): its instruments, then each work with its maker and town linked.
    fn arts_html(&self, fid: FactionId) -> String {
        let arts = crate::history::arts::of_people(self.h, fid);
        if arts.forms.is_empty() && arts.instruments.is_empty() { return String::new(); }
        let mut o = String::from("<ul class=\"plain\">");
        for i in &arts.instruments { let _ = write!(o, "<li class=\"meta\">{}</li>", esc(&capital(&i.describe()))); }
        for (form, text) in arts.forms.iter().zip(arts.lines(self.h)) {
            let mut names = Vec::new();
            if let Some(x) = form.author.and_then(|x| self.h.figures.get(&x)) { names.push((format!("figure-{}.html", x.id.0), x.full_name())); }
            if let Some(t) = form.town.and_then(|t| self.h.settlements.get(&t)) { names.push((format!("site-{}.html", t.id.0), t.name.clone())); }
            let _ = write!(o, "<li>{}</li>", self.link_names(&capital(&text), names));
        }
        o.push_str("</ul>");
        o
    }

    /// The age containing `d`, as (index, era).
    fn age_at(&self, d: &crate::history::time::Date) -> Option<(usize, &'a crate::history::time::Era)> {
        self.h.timeline.eras.iter().enumerate().find(|(_, a)| a.contains(d))
    }

    /// Where an event belongs, outermost first (as `collections::context`, from indexes): its
    /// war, its battle or siege (its age heads the year's page).
    fn context(&self, e: &Event) -> Vec<String> {
        let mut out = Vec::new();
        let mut fight: Option<EventId> = None;
        let mut frontier = vec![e.id];
        for _ in 0..3 {
            if let Some(f) = frontier.iter().copied().find(|id| self.fight_war.contains_key(id)) { fight = Some(f); break; }
            frontier = frontier.iter().filter_map(|id| self.ev(*id)).flat_map(|x| x.causes.iter().copied()).collect();
            if frontier.is_empty() { break; }
        }
        let within = |w: &crate::history::civilizations::military::War| e.date.year >= w.started.year && w.ended.map_or(true, |d| e.date.year <= d.year);
        let war = fight.and_then(|f| self.fight_war.get(&f)).or_else(|| self.decl_war.get(&e.id))
            .or_else(|| e.causes.iter().find_map(|c| self.decl_war.get(c)))
            .copied().filter(|w| self.h.wars.get(w).map_or(false, within));
        if let Some(w) = war.and_then(|w| self.h.wars.get(&w)) { out.push(a(&format!("war-{}.html", w.id.0), &tidy(&w.name).replacen("The ", "the ", 1))); }
        if let Some(fe) = fight.filter(|f| *f != e.id).and_then(|f| self.ev(f)) { out.push(a(&Self::ev_url(fe), &fe.title)); }
        out
    }

    /// An event's entry on its year's page.
    fn entry(&self, e: &Event) -> String {
        let mut o = format!("<article class=\"ev k-{}\" id=\"e{}\"><h3><span class=\"season\">{:?}</span> {}</h3><p>{}</p>",
            kind_of(&e.event_type), e.id.0, e.date.season, esc(&capital(&tidy(&e.title))), self.linked(e, &e.description));
        let ctx = self.context(e);
        if !ctx.is_empty() { let _ = write!(o, "<p class=\"meta\">Part of {}.</p>", ctx.join(", in ")); }
        let mut chain: Vec<&Event> = Vec::new();
        let mut cur = e;
        while let Some(c) = cur.causes.first() {
            let Some(ce) = self.ev(*c) else { break };
            if chain.iter().any(|x| x.id == ce.id) { break; }
            chain.push(ce);
            if chain.len() >= CHAIN { break; }
            cur = ce;
        }
        if !chain.is_empty() {
            o.push_str("<div class=\"chain\"><p class=\"label\">Because</p><ol>");
            for ce in &chain { let _ = write!(o, "<li><span class=\"yr\">{}</span> {}</li>", ce.date.year, a(&Self::ev_url(ce), &capital(&tidy(&ce.title)))); }
            o.push_str("</ol></div>");
        }
        if let Some(led) = self.led_to.get(&e.id) {
            o.push_str("<div class=\"chain\"><p class=\"label\">It led to</p><ol>");
            for &i in led.iter().take(12) { let x = self.events[i]; let _ = write!(o, "<li><span class=\"yr\">{}</span> {}</li>", x.date.year, a(&Self::ev_url(x), &capital(&tidy(&x.title)))); }
            if led.len() > 12 { let _ = write!(o, "<li class=\"meta\">and {} more</li>", led.len() - 12); }
            o.push_str("</ol></div>");
        }
        if let Some(t) = self.tale_of.get(&e.id).map(|&i| &self.tales[i]) {
            let _ = write!(o, "<aside class=\"tale\"><p class=\"label\">A tale worth telling: {}</p><p>{}</p></aside>", esc(&t.kind.label().to_lowercase()), self.link_names(&t.text, self.named_in(&t.text)));
        }
        o.push_str("</article>");
        o
    }

    /// The country round a place, cut from the same map (CSS: the map scaled up behind a 360x240
    /// window centred on the tile, clamped at the map's edges), the place pinned.
    fn site_crop(&self, loc: (usize, usize), caption: &str) -> String {
        let Some(_) = self.map else { return String::new() };
        let (bw, bh) = (360.0f32, 240.0f32);
        let tile = bw / 14.0;
        let (sw, sh) = (self.map_tiles.0 as f32 * tile, self.map_tiles.1 as f32 * tile);
        let (cx, cy) = ((loc.0 as f32 + 0.5) * tile, (loc.1 as f32 + 0.5) * tile);
        let ox = (bw / 2.0 - cx).clamp(-(sw - bw).max(0.0), 0.0);
        let oy = (bh / 2.0 - cy).clamp(-(sh - bh).max(0.0), 0.0);
        format!("<figure class=\"map crop\"><div class=\"crop-box\" style=\"background-size:{:.0}px {:.0}px;background-position:{:.0}px {:.0}px\" role=\"img\" aria-label=\"{}\"><span class=\"pin big\" style=\"left:{:.0}px;top:{:.0}px\"></span></div><figcaption>{}</figcaption></figure>",
            sw, sh, ox, oy, esc(caption), cx + ox, cy + oy, esc(caption))
    }

    /// The world map with pins, if a map was rendered.
    fn map_fig(&self, pins: &[((usize, usize), bool, String)], caption: &str) -> String {
        let Some((w, hgt)) = self.map else { return String::new() };
        if pins.is_empty() { return String::new(); }
        let mut o = format!("<figure class=\"map\"><div class=\"map-box\"><img src=\"map.png\" width=\"{}\" height=\"{}\" alt=\"{}\">", w, hgt, esc(caption));
        for ((x, y), big, label) in pins {
            let (px, py) = ((*x as f32 + 0.5) / self.map_tiles.0 as f32 * 100.0, (*y as f32 + 0.5) / self.map_tiles.1 as f32 * 100.0);
            let _ = write!(o, "<span class=\"pin{}\" style=\"left:{:.2}%;top:{:.2}%\" title=\"{}\"></span>", if *big { " big" } else { "" }, px, py, esc(label));
        }
        let _ = write!(o, "</div><figcaption>{}</figcaption></figure>", esc(caption));
        o
    }

    fn arms_img(&self, f: FactionId) -> String {
        if self.arms.contains(&f) { format!("<img class=\"arms\" src=\"arms-{}.png\" width=\"41\" height=\"48\" alt=\"\">", f.0) } else { String::new() }
    }

    fn where_is(&self, (x, y): (usize, usize)) -> String {
        let d = self.gaz.describe(x, y);
        if d.is_empty() { format!("at {},{}", x, y) } else { format!("in {} ({},{})", d, x, y) }
    }

    /// A whole page: the bar with its search, the body, the footer.
    fn frame(&self, title: &str, body: &str) -> String {
        format!("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{}The Legends of {}</title>{}<link rel=\"stylesheet\" href=\"legends.css\"></head>\n<body><header class=\"bar\"><a class=\"home\" href=\"index.html\">The Legends of {}</a>\
<nav aria-label=\"The record\">{}</nav><div class=\"find\"><input id=\"q\" type=\"search\" placeholder=\"Find a name or a year\" aria-label=\"Find a name or a year\" autocomplete=\"off\"><ol id=\"hits\"></ol></div></header>\n\
<main>{}</main>\n<footer>Written from the chronicle of seed {}: {} events in {} years.</footer><script src=\"legends.js\"></script></body></html>\n",
            if title.is_empty() { String::new() } else { format!("{} · ", esc(title)) }, esc(&self.world_name), FONTS, esc(&self.world_name), NAV, body, self.seed, self.events.len(), self.h.current_date.year)
    }
}

const FONTS: &str = "<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\"><link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin><link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?family=IM+Fell+English:ital@0;1&family=IM+Fell+English+SC&family=Alegreya:ital,wght@0,400;0,600;1,400&family=Alegreya+Sans+SC:wght@500&display=swap\">";

const NAV: &str = "<a href=\"ages.html\">Ages</a><a href=\"peoples.html\">Peoples</a><a href=\"sites.html\">Sites</a><a href=\"figures.html\">Figures</a><a href=\"beasts.html\">Beasts</a><a href=\"treasures.html\">Treasures</a><a href=\"wars.html\">Wars</a><a href=\"faiths.html\">Faiths</a><a href=\"arts.html\">Arts</a><a href=\"years.html\">Years</a><a href=\"camps.html\">Camps</a>";

/// Arms as a small PNG (transparent outside the shield).
fn arms_png(world: &WorldData, h: &WorldHistory, f: FactionId, path: &Path) -> bool {
    const BLANK: u32 = 0x0100_0000;
    let (w, ht) = (41usize, 48usize);
    let mut buf = vec![BLANK; w * ht];
    let arms = crate::tiles::heraldry::arms_of(world, h, f);
    crate::tiles::heraldry::draw(&mut buf, w, ht, 0, 0, ht, &arms);
    let img = image::RgbaImage::from_fn(w as u32, ht as u32, |x, y| {
        let p = buf[y as usize * w + x as usize];
        if p == BLANK { image::Rgba([0, 0, 0, 0]) } else { image::Rgba([(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]) }
    });
    img.save(path).is_ok()
}

/// Write the legends of the world into `dir`: `index.html` and one page for every people, site,
/// figure, beast, treasure, monument, war, faith, age, year with events and camp. `map` (the
/// rendered world, `tiles::viewer::map_image`) pins sites and realms when given; `camps` are the
/// legends of earlier colonies (`colony::legend::load`).
pub fn write_legends(world: &WorldData, h: &WorldHistory, gaz: &Gazetteer, camps: &[Legend], map: Option<&image::RgbImage>, dir: &Path) -> std::io::Result<Report> {
    std::fs::create_dir_all(dir)?;
    let map_size = match map {
        Some(img) => { img.save(dir.join("map.png")).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?; Some((img.width(), img.height())) }
        None => None,
    };
    let mut b = Book::new(world, h, gaz, map_size);
    for (id, _) in sorted(&h.factions) {
        if arms_png(world, h, id, &dir.join(format!("arms-{}.png", id.0))) { b.arms.insert(id); }
    }
    let mut files: Vec<(String, String)> = Vec::new();
    let mut search: Vec<(String, &'static str, String)> = Vec::new();
    let mut rep = Report::default();

    // --- Ages and years ---------------------------------------------------------------------
    let mut years: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, e) in b.events.iter().enumerate() { years.entry(e.date.year).or_default().push(i); }
    let year_list: Vec<u32> = years.keys().copied().collect();
    for (k, (&y, idx)) in years.iter().enumerate() {
        let mut o = String::new();
        let prev = if k > 0 { a(&format!("year-{}.html", year_list[k - 1]), &format!("‹ {}", year_list[k - 1])) } else { String::new() };
        let next = year_list.get(k + 1).map(|n| a(&format!("year-{}.html", n), &format!("{} ›", n))).unwrap_or_default();
        let age = b.age_at(&crate::history::time::Date::new(y, crate::seasons::Season::Summer)).map(|(i, a_)| a(&format!("age-{}.html", i + 1), &a_.name)).unwrap_or_default();
        let _ = write!(o, "<header class=\"title\"><p class=\"eyebrow\">{}</p><h1>The Year {}</h1><p class=\"pager\">{}<span>{}</span>{}</p></header>",
            age, y, prev, plural(idx.len(), "event", "events"), next);
        for &i in idx { o.push_str(&b.entry(b.events[i])); }
        files.push((format!("year-{}.html", y), b.frame(&format!("The Year {}", y), &o)));
        search.push((format!("Year {}", y), "year", format!("year-{}.html", y)));
        rep.years += 1;
    }
    {
        let mut o = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Years</h1><p class=\"lede\">Every year in which something was written down, age by age.</p></header>");
        for (i, age) in h.timeline.eras.iter().enumerate() {
            let in_age: Vec<String> = year_list.iter().filter(|y| age.contains(&crate::history::time::Date::new(**y, crate::seasons::Season::Summer)))
                .map(|y| format!("<a href=\"year-{0}.html\">{0}</a>", y)).collect();
            if in_age.is_empty() { continue; }
            let _ = write!(o, "<section class=\"part\"><h2>{}</h2><p class=\"years\">{}</p></section>", a(&format!("age-{}.html", i + 1), &age.name), in_age.join(" "));
        }
        let outside: Vec<String> = year_list.iter().filter(|y| b.age_at(&crate::history::time::Date::new(**y, crate::seasons::Season::Summer)).is_none())
            .map(|y| format!("<a href=\"year-{0}.html\">{0}</a>", y)).collect();
        if !outside.is_empty() { let _ = write!(o, "<section class=\"part\"><h2>Before the Ages</h2><p class=\"years\">{}</p></section>", outside.join(" ")); }
        files.push(("years.html".into(), b.frame("The Years", &o)));
    }
    let mut ages_list = String::from("<ol class=\"index\">");
    for (i, age) in h.timeline.eras.iter().enumerate() {
        let end = age.end.map(|d| d.year.to_string()).unwrap_or_else(|| "now".into());
        let idx: Vec<usize> = b.events.iter().enumerate().filter(|(_, e)| age.contains(&e.date)).map(|(i, _)| i).collect();
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">The {} age</p><h1>{}</h1><p class=\"lede\">From the year {} to {}: {}.</p></header>",
            ordinal(i + 1), esc(&age.name), age.start.year, end, plural(idx.len(), "event", "events"));
        let count = |t: EventType| idx.iter().filter(|&&j| b.events[j].event_type == t).count();
        let tally = [(count(EventType::BattleFought), "battle", "battles"), (count(EventType::SiegeBegun), "siege", "sieges"), (count(EventType::SettlementDestroyed) + count(EventType::ShadowConquest), "town fallen", "towns fallen"),
            (count(EventType::SettlementFounded), "town founded", "towns founded"), (count(EventType::MonsterRaid), "beast's raid", "beasts' raids"), (count(EventType::CreatureSlain), "beast slain", "beasts slain"), (count(EventType::RulerCrowned), "crowning", "crownings")];
        let t: Vec<String> = tally.iter().filter(|x| x.0 > 0).map(|x| plural(x.0, x.1, x.2)).collect();
        if !t.is_empty() { let _ = write!(o, "<p class=\"age-sum\">{}.</p>", capital(&and_list(&t))); }
        let wars: Vec<String> = sorted(&h.wars).into_iter().filter(|(_, w)| age.contains(&w.started))
            .map(|(id, w)| format!("{} <span class=\"yr\">{}</span>", a(&format!("war-{}.html", id.0), &tidy(&w.name)), w.started.year)).collect();
        if !wars.is_empty() { let _ = write!(o, "<h2>Wars begun in it</h2><p>{}</p>", wars.join(" · ")); }
        let great: Vec<usize> = idx.iter().copied().filter(|&j| { let e = b.events[j]; e.is_major || matches!(e.event_type, EventType::SettlementDestroyed | EventType::Authored | EventType::ShadowRepelled | EventType::ShadowBane | EventType::ArtifactCreated) }).collect();
        let _ = write!(o, "<h2>Its great events</h2>{}", b.story(&great[..great.len().min(300)], false));
        let yrs: Vec<String> = year_list.iter().filter(|y| age.contains(&crate::history::time::Date::new(**y, crate::seasons::Season::Summer))).map(|y| format!("<a href=\"year-{0}.html\">{0}</a>", y)).collect();
        let _ = write!(o, "<h2>Year by year</h2><p class=\"years\">{}</p>", yrs.join(" "));
        files.push((format!("age-{}.html", i + 1), b.frame(&age.name, &o)));
        search.push((age.name.clone(), "age", format!("age-{}.html", i + 1)));
        let _ = write!(ages_list, "<li>{} <span class=\"yr\">{}–{}</span> <span class=\"meta\">{}</span></li>", a(&format!("age-{}.html", i + 1), &age.name), age.start.year, end, plural(idx.len(), "event", "events"));
    }
    ages_list.push_str("</ol>");
    files.push(("ages.html".into(), b.frame("The Ages", &format!("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Ages</h1><p class=\"lede\">The world names its ages for what held power in them.</p></header>{}", ages_list))));

    // --- Peoples ----------------------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Peoples</h1><p class=\"lede\">Every people that rose, in the order of their founding.</p></header><div class=\"cards\">");
    let mut facs: Vec<_> = sorted(&h.factions);
    facs.sort_by_key(|(id, f)| (f.founded, *id));
    for (fid, f) in &facs {
        let race = h.races.get(&f.race_id).map(|r| r.name.clone()).unwrap_or_default();
        let gov = words(&format!("{:?}", f.government));
        let span = match f.dissolved { Some(d) => format!("{}–{}", f.founded.year, d.year), None => format!("since {}", f.founded.year) };
        let _ = write!(list, "<article class=\"card\">{}<h3>{}</h3><p class=\"meta\">{} · {} · {}</p></article>", b.arms_img(*fid), a(&format!("people-{}.html", fid.0), &tidy(&f.name)), esc(&race), esc(&gov), span);
        let mut o = format!("<header class=\"title\">{}<p class=\"eyebrow\">{} · {}</p><h1>{}</h1>", b.arms_img(*fid), esc(&race), esc(&gov), esc(&tidy(&f.name)));
        let founding = b.by_faction.get(fid).and_then(|v| v.iter().map(|&i| b.events[i]).find(|e| e.event_type == EventType::FactionFounded));
        let mut lede = match founding { Some(e) => b.linked(e, &e.description), None => format!("Founded in the year {}.", f.founded.year) };
        match f.dissolved {
            Some(d) => { let _ = write!(lede, " It came to an end in the year {}.", d.year); }
            None => {
                let ruler = f.current_leader.filter(|l| h.figures.contains_key(l)).map(|l| format!(", ruled by {}", b.link(EntityId::Figure(l)))).unwrap_or_default();
                let seat = f.capital.filter(|c| h.settlements.contains_key(c)).map(|c| format!(" from {}", b.link(EntityId::Settlement(c)))).unwrap_or_default();
                let _ = write!(lede, " It endures with {} and {} souls{}{}.", plural(f.settlements.len(), "town", "towns"), f.total_population, ruler, seat);
            }
        }
        if let Some(r) = f.state_religion.filter(|r| h.religions.contains_key(r)) { let _ = write!(lede, " Its faith is {}.", b.link(EntityId::Religion(r))); }
        let _ = write!(o, "<p class=\"lede\">{}</p></header>", lede);
        // Its towns now (pinned), and the places it once held.
        let mut towns: Vec<_> = sorted(&h.settlements).into_iter().filter(|(_, s)| s.faction == *fid).collect();
        towns.sort_by_key(|(id, s)| (s.destroyed.is_some(), Some(*id) != f.capital, s.founded, *id));
        let pins: Vec<((usize, usize), bool, String)> = towns.iter().filter(|(_, s)| s.destroyed.is_none()).map(|(id, s)| (s.location, Some(*id) == f.capital, s.name.clone())).collect();
        o.push_str(&b.map_fig(&pins, &format!("The towns of {}", tidy(&f.name))));
        if !towns.is_empty() {
            let t: Vec<String> = towns.iter().map(|(id, s)| format!("{}{}", b.link(EntityId::Settlement(*id)), match s.destroyed { Some(d) => format!(" <span class=\"yr\">fell {}</span>", d.year), None => String::new() })).collect();
            let _ = write!(o, "<h2>Its towns</h2><p>{}</p>", t.join(" · "));
        }
        // Rulers, in order of their crowning.
        let rulers: Vec<String> = b.by_faction.get(fid).into_iter().flatten().map(|&i| b.events[i]).filter(|e| e.event_type == EventType::RulerCrowned)
            .filter_map(|e| e.primary_participants.iter().find_map(|p| if let EntityId::Figure(x) = p { Some((*x, e)) } else { None }))
            .map(|(x, e)| format!("{} <a class=\"yr\" href=\"{}\">{}</a>", b.link(EntityId::Figure(x)), Book::ev_url(e), e.date.year)).collect();
        if !rulers.is_empty() { let _ = write!(o, "<h2>Its rulers</h2><p>{}</p>", rulers.join(" · ")); }
        let wars: Vec<String> = f.wars.iter().filter_map(|w| h.wars.get(w)).map(|w| {
            let outcome = match (w.victor, w.ended) { (Some(v), _) if v == *fid => "won", (Some(_), _) => "lost", (None, Some(_)) => "drawn", (None, None) => "still fought" };
            format!("{} <span class=\"yr\">{}, {}</span>", a(&format!("war-{}.html", w.id.0), &tidy(&w.name)), w.started.year, outcome)
        }).collect();
        if !wars.is_empty() { let _ = write!(o, "<h2>Its wars</h2><p>{}</p>", wars.join(" · ")); }
        let arts = b.arts_html(*fid);
        if !arts.is_empty() { let _ = write!(o, "<h2>Its arts</h2>{}", arts); }
        let mut people: Vec<_> = sorted(&h.figures).into_iter().filter(|(_, x)| x.faction == Some(*fid)).collect();
        people.sort_by_key(|(id, x)| (x.birth_date, *id));
        if !people.is_empty() {
            let p: Vec<String> = people.iter().map(|(id, x)| format!("{}{}", b.link(EntityId::Figure(*id)), if x.is_alive() { "" } else { "<span class=\"dead\">†</span>" })).collect();
            let _ = write!(o, "<h2>Its people of note</h2><p class=\"names\">{}</p>", p.join(" · "));
        }
        let _ = write!(o, "<h2>Its story</h2>{}", b.story(b.by_faction.get(fid).map(|v| v.as_slice()).unwrap_or(&[]), true));
        files.push((format!("people-{}.html", fid.0), b.frame(&tidy(&f.name), &o)));
        search.push((tidy(&f.name), "people", format!("people-{}.html", fid.0)));
        rep.peoples += 1;
    }
    list.push_str("</div>");
    files.push(("peoples.html".into(), b.frame("The Peoples", &list)));

    // --- Arts -------------------------------------------------------------------------------
    {
        let mut o = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Arts</h1><p class=\"lede\">Each people's instruments, and its poems, music and dances with the hands that made them.</p></header>");
        for (fid, f) in &facs {
            let arts = b.arts_html(*fid);
            if arts.is_empty() { continue; }
            let _ = write!(o, "<section class=\"part\"><h2>{}</h2>{}</section>", b.people(*fid), arts);
        }
        files.push(("arts.html".into(), b.frame("The Arts", &o)));
    }

    // --- Sites ------------------------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Sites</h1><p class=\"lede\">Every town, fort and holy place, standing or fallen, by the people that held it last.</p></header>");
    let mut by_holder: BTreeMap<(u32, u64), Vec<SettlementId>> = BTreeMap::new();
    for (id, s) in sorted(&h.settlements) {
        let founded = h.factions.get(&s.faction).map_or(u32::MAX, |f| f.founded.year);
        by_holder.entry((founded, s.faction.0)).or_default().push(id);
    }
    for ((_, fid), ids) in &by_holder {
        let _ = write!(list, "<section class=\"part\"><h2>{}</h2><div class=\"table-wrap\"><table><thead><tr><th>Site</th><th>Kind</th><th class=\"num\">Founded</th><th class=\"num\">Fell</th><th class=\"num\">Souls</th></tr></thead><tbody>", b.people(FactionId(*fid)));
        for id in ids {
            let s = &h.settlements[id];
            let _ = write!(list, "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>", b.link(EntityId::Settlement(*id)), words(&format!("{:?}", s.settlement_type)),
                s.founded.year, s.destroyed.map(|d| d.year.to_string()).unwrap_or_default(), if s.destroyed.is_some() { String::new() } else { s.population.to_string() });
        }
        list.push_str("</tbody></table></div></section>");
    }
    files.push(("sites.html".into(), b.frame("The Sites", &list)));
    for (id, s) in sorted(&h.settlements) {
        let kind = words(&format!("{:?}", s.settlement_type));
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">{} · {}</p><h1>{}</h1>", esc(&capital(&kind)), esc(&b.where_is(s.location)), esc(&s.name));
        let lede = match s.destroyed {
            Some(d) => format!("Founded in the year {}, it fell in {}. It was last held by {}.", s.founded.year, d.year, b.people(s.faction)),
            None => format!("Founded in the year {}, it is held by {} and home to {} souls.", s.founded.year, b.people(s.faction), s.population),
        };
        let _ = write!(o, "<p class=\"lede\">{}</p></header>", lede);
        o.push_str(&b.site_crop(s.location, &format!("The country round {}", s.name)));
        o.push_str(&b.map_fig(&[(s.location, true, s.name.clone())], &format!("{} on the map of the world", s.name)));
        // Who held it, by the tile's record.
        let holders: Vec<String> = h.tile_history.get(s.location.0, s.location.1).ownership.iter()
            .filter(|r| r.lost.map_or(true, |l| l.year >= s.founded.year) && s.destroyed.map_or(true, |d| r.gained.year <= d.year))
            .map(|r| format!("{} <span class=\"yr\">{}–{}</span>", b.people(r.faction), r.gained.year.max(s.founded.year), r.lost.map(|l| l.year.to_string()).unwrap_or_else(|| if s.destroyed.is_some() { String::new() } else { "now".into() }))).collect();
        if holders.len() > 1 { let _ = write!(o, "<h2>Its holders</h2><p>{}</p>", holders.join(" · ")); }
        if let Some(p) = h.people.as_ref() {
            let n: Vec<String> = p.notables_of(h, id).into_iter().map(|(f, r)| format!("{}, its {}", b.link(EntityId::Figure(f)), r.word())).collect();
            if !n.is_empty() { let _ = write!(o, "<h2>Who lives here</h2><p>{}</p>", n.join(" · ")); }
        }
        let mons: Vec<String> = sorted(&h.monuments).into_iter().filter(|(_, m)| m.location == s.location).map(|(m, x)| format!("{} <span class=\"yr\">{}</span>", b.link(EntityId::Monument(m)), x.built_date.year)).collect();
        if !mons.is_empty() { let _ = write!(o, "<h2>Its monuments</h2><p>{}</p>", mons.join(" · ")); }
        let made: Vec<String> = sorted(&h.artifacts).into_iter().filter(|(_, x)| x.creation_location == Some(s.location)).map(|(x, t)| format!("{} <span class=\"yr\">{}</span>", b.link(EntityId::Artifact(x)), t.creation_date.year)).collect();
        if !made.is_empty() { let _ = write!(o, "<h2>Made here</h2><p>{}</p>", made.join(" · ")); }
        let mut idx: Vec<usize> = b.by_entity.get(&EntityId::Settlement(id)).cloned().unwrap_or_default();
        idx.extend(b.by_tile.get(&s.location).into_iter().flatten().copied().filter(|&i| { let y = b.events[i].date.year; y >= s.founded.year && s.destroyed.map_or(true, |d| y <= d.year + 200) }));
        idx.sort();
        idx.dedup();
        let _ = write!(o, "<h2>Its story</h2>{}", b.story(&idx, true));
        files.push((format!("site-{}.html", id.0), b.frame(&s.name, &o)));
        search.push((s.name.clone(), "site", format!("site-{}.html", id.0)));
        rep.sites += 1;
    }

    // --- Figures ----------------------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Figures</h1><p class=\"lede\">Everyone the chronicle remembers, by people and in the order of their birth. A cross marks the dead.</p></header>");
    let mut groups: BTreeMap<(u32, u64), Vec<FigureId>> = BTreeMap::new();
    for (id, f) in sorted(&h.figures) {
        let key = f.faction.and_then(|x| h.factions.get(&x).map(|fa| (fa.founded.year, x.0))).unwrap_or((u32::MAX, u64::MAX));
        groups.entry(key).or_default().push(id);
    }
    for ((_, fid), ids) in &groups {
        let head = if *fid == u64::MAX { "Of no people".to_string() } else { b.people(FactionId(*fid)) };
        let mut ids = ids.clone();
        ids.sort_by_key(|i| (h.figures[i].birth_date, *i));
        let names: Vec<String> = ids.iter().map(|i| { let x = &h.figures[i]; format!("{} <span class=\"yr\">{}</span>{}", b.link(EntityId::Figure(*i)), x.birth_date.year, if x.is_alive() { "" } else { "<span class=\"dead\">†</span>" }) }).collect();
        let _ = write!(list, "<section class=\"part\"><h2>{}</h2><p class=\"names\">{}</p></section>", head, names.join(" · "));
    }
    files.push(("figures.html".into(), b.frame("The Figures", &list)));
    for (id, f) in sorted(&h.figures) {
        let race = h.races.get(&f.race_id).map(|r| r.name.clone()).unwrap_or_default();
        let age = match f.death_date { Some(d) => f.age_at(&d), None => f.age_at(&h.current_date) };
        let role = h.people.as_ref().and_then(|p| p.role.get(&id)).map(|r| r.word());
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">{}{}</p><h1>{}</h1>", esc(&race), role.map(|r| format!(" · {}", r)).unwrap_or_default(), esc(&f.full_name()));
        let mut lede = format!("{} was born in the year {}", esc(&f.name), f.birth_date.year);
        if let Some(fa) = f.faction { let _ = write!(lede, " of {}", b.people(fa)); }
        match (f.death_date, &f.cause_of_death) {
            (Some(d), Some(c)) => { let _ = write!(lede, " and {} in {}, aged {}.", death_words(&format!("{:?}", c)), d.year, age); }
            (Some(d), None) => { let _ = write!(lede, " and died in {}, aged {}.", d.year, age); }
            _ => {
                let home = h.people.as_ref().and_then(|p| p.home.get(&id)).filter(|t| h.settlements.contains_key(t)).map(|t| format!(", and lives at {}", b.link(EntityId::Settlement(*t)))).unwrap_or_default();
                let _ = write!(lede, "; {} years old in {}{}.", age, h.current_date.year, home);
            }
        }
        if !f.titles.is_empty() { let _ = write!(lede, " Titles: {}.", esc(&f.titles.join(", "))); }
        if let Some(d) = f.dynasty.and_then(|d| h.dynasties.get(&d)) { let _ = write!(lede, " Of {}.", esc(&tidy(&d.name))); }
        let _ = write!(o, "<p class=\"lede\">{}</p></header>", lede);
        let kin: Vec<String> = [f.parents.0, f.parents.1].iter().flatten().map(|p| ("parent", *p))
            .chain(f.spouse.map(|s| ("spouse", s)))
            .chain(f.children.iter().map(|c| ("child", *c)))
            .chain(f.mentors.iter().map(|m| ("mentor", *m)))
            .chain(f.enemies.iter().map(|m| ("enemy", *m)))
            .filter(|(_, k)| h.figures.contains_key(k))
            .map(|(w, k)| format!("{} <span class=\"meta\">({}{})</span>", b.link(EntityId::Figure(k)), w, if h.figures[&k].is_alive() { "" } else { ", dead" })).collect();
        if !kin.is_empty() { let _ = write!(o, "<h2>Kin and others</h2><p>{}</p>", kin.join(" · ")); }
        o.push_str("<h2>Who they were</h2>");
        for para in crate::persona::Persona::of_figure(h, f).describe(&f.name, age) { let _ = write!(o, "<p>{}</p>", esc(&para)); }
        let held: Vec<String> = sorted(&h.artifacts).into_iter().filter(|(_, x)| x.creator == Some(id) || x.owner_history.iter().any(|o| o.0 == EntityId::Figure(id)))
            .map(|(x, t)| format!("{}{}", b.link(EntityId::Artifact(x)), if t.creator == Some(id) { " <span class=\"meta\">(made)</span>" } else { "" })).collect();
        if !held.is_empty() { let _ = write!(o, "<h2>Treasures</h2><p>{}</p>", held.join(" · ")); }
        let slew: Vec<String> = f.kills.iter().filter(|k| matches!(k, EntityId::LegendaryCreature(_))).map(|k| b.link(k.clone())).collect();
        if !slew.is_empty() { let _ = write!(o, "<h2>Slew</h2><p>{}</p>", slew.join(" · ")); }
        let mut idx: Vec<usize> = b.by_entity.get(&EntityId::Figure(id)).cloned().unwrap_or_default();
        idx.extend(f.events.iter().filter_map(|e| b.pos.get(e).copied()));
        idx.sort();
        idx.dedup();
        let _ = write!(o, "<h2>Their life</h2>{}", b.story(&idx, false));
        files.push((format!("figure-{}.html", id.0), b.frame(&f.full_name(), &o)));
        search.push((f.full_name(), "figure", format!("figure-{}.html", id.0)));
        rep.figures += 1;
    }

    // --- Beasts -----------------------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Beasts of Legend</h1><p class=\"lede\">The great beasts, living first, then the slain by the year of their death.</p></header><div class=\"table-wrap\"><table><thead><tr><th>Beast</th><th>Kind</th><th>Lair</th><th class=\"num\">Slain</th></tr></thead><tbody>");
    let mut beasts: Vec<_> = sorted(&h.legendary_creatures);
    beasts.sort_by_key(|(id, c)| (c.death_date.is_some(), c.death_date, *id));
    for (id, c) in &beasts {
        let species = h.creature_species.get(&c.species_id);
        let kind = species.map(|s| s.name.clone()).unwrap_or_default();
        let lair = c.lair_location.map(|l| b.where_is(l)).unwrap_or_default();
        let _ = write!(list, "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>", b.link(EntityId::LegendaryCreature(*id)), esc(&kind), esc(&lair), c.death_date.map(|d| d.year.to_string()).unwrap_or_default());
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">{}</p><h1>{}</h1>", esc(&capital(&kind)), esc(&c.full_name()));
        let m = crate::monsters::of_legend(h, c);
        let state = match c.death_date { Some(d) => format!("It was slain in the year {}.", d.year), None => "It lives still.".into() };
        let _ = write!(o, "<p class=\"lede\">{} {}</p></header>", esc(&m.description), state);
        let mut traits: Vec<String> = Vec::new();
        if m.blood != "blood" { traits.push(format!("its blood is {}", m.blood)); }
        if m.flies { traits.push("it flies".into()); }
        if !c.unique_abilities.is_empty() { traits.push(format!("its powers are {}", and_list(&c.unique_abilities.iter().map(|x| words(&format!("{:?}", x))).collect::<Vec<_>>()))); }
        if !traits.is_empty() { let _ = write!(o, "<p class=\"meta\">{}.</p>", esc(&capital(&traits.join("; ")))); }
        if let Some(l) = c.lair_location {
            let _ = write!(o, "<p>Its lair is {}.</p>", esc(&b.where_is(l)));
            o.push_str(&b.map_fig(&[(l, true, format!("The lair of {}", c.full_name()))], &format!("The lair of {}", c.full_name())));
        }
        let hoard: Vec<String> = sorted(&h.artifacts).into_iter().filter(|(x, t)| c.artifacts_owned.contains(x) || t.current_owner == Some(EntityId::LegendaryCreature(*id))).map(|(x, _)| b.link(EntityId::Artifact(x))).collect();
        if !hoard.is_empty() { let _ = write!(o, "<h2>Its hoard</h2><p>{}</p>", hoard.join(" · ")); }
        if let Some(cult) = sorted(&h.cults).into_iter().find(|(_, x)| x.worshipped_creature == *id) {
            let _ = write!(o, "<p>It is worshipped by {}, founded in {}.</p>", esc(&tidy(&cult.1.name)), cult.1.founded.year);
        }
        let idx = b.by_entity.get(&EntityId::LegendaryCreature(*id)).cloned().unwrap_or_default();
        let _ = write!(o, "<h2>Its deeds</h2>{}", b.story(&idx, false));
        files.push((format!("beast-{}.html", id.0), b.frame(&c.full_name(), &o)));
        search.push((c.full_name(), "beast", format!("beast-{}.html", id.0)));
        rep.beasts += 1;
    }
    list.push_str("</tbody></table></div>");
    files.push(("beasts.html".into(), b.frame("The Beasts of Legend", &list)));

    // --- Treasures and monuments ------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>Treasures and Monuments</h1><p class=\"lede\">What hands made to remember deeds, in the order of their making.</p></header><h2>Treasures</h2><ul class=\"plain\">");
    let mut arts: Vec<_> = sorted(&h.artifacts);
    arts.sort_by_key(|(id, x)| (x.creation_date, *id));
    for (id, x) in &arts {
        let now = artifact_now(&b, x);
        let _ = write!(list, "<li><span class=\"yr\">{}</span> {} <span class=\"meta\">{}</span></li>", x.creation_date.year, b.link(EntityId::Artifact(*id)), now);
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">{} {}</p><h1>{}</h1>", esc(&capital(&words(&format!("{:?}", x.quality)))), esc(&words(&format!("{:?}", x.item_type))), esc(&x.name));
        let maker = x.creator.filter(|c| h.figures.contains_key(c)).map(|c| format!(" by {}", b.link(EntityId::Figure(c)))).unwrap_or_default();
        let at = x.creation_location.and_then(|l| b.sites_at.get(&l).and_then(|v| v.first().copied())).map(|s| format!(" at {}", b.link(EntityId::Settlement(s)))).unwrap_or_default();
        let _ = write!(o, "<p class=\"lede\">Made{}{} in the year {}. {}.</p></header>", maker, at, x.creation_date.year, capital(&now));
        if !x.description.is_empty() { let _ = write!(o, "<p class=\"desc\">{}</p>", esc(&x.description)); }
        for i in &x.inscriptions { let _ = write!(o, "<blockquote>{}</blockquote>", esc(&i.text)); }
        if !x.owner_history.is_empty() {
            o.push_str("<h2>Its keepers</h2><ol class=\"story\">");
            for (who, from, to, how) in &x.owner_history {
                let _ = write!(o, "<li><span class=\"yr\">{}{}</span> {} <span class=\"meta\">({})</span></li>", from.year, to.map(|t| format!("–{}", t.year)).unwrap_or_default(), b.link(who.clone()), how_kept(how));
            }
            o.push_str("</ol>");
        }
        let mut idx: Vec<usize> = b.by_entity.get(&EntityId::Artifact(*id)).cloned().unwrap_or_default();
        idx.extend(x.creation_event.iter().chain(x.involved_in.iter()).filter_map(|e| b.pos.get(e).copied()));
        idx.sort();
        idx.dedup();
        let _ = write!(o, "<h2>Its story</h2>{}", b.story(&idx, false));
        files.push((format!("artifact-{}.html", id.0), b.frame(&x.name, &o)));
        search.push((x.name.clone(), "treasure", format!("artifact-{}.html", id.0)));
        rep.treasures += 1;
    }
    list.push_str("</ul><h2>Monuments</h2><ul class=\"plain\">");
    let mut mons: Vec<_> = sorted(&h.monuments);
    mons.sort_by_key(|(id, m)| (m.built_date, *id));
    for (id, m) in &mons {
        let _ = write!(list, "<li><span class=\"yr\">{}</span> {}{}</li>", m.built_date.year, b.link(EntityId::Monument(*id)), if m.intact { "" } else { " <span class=\"meta\">in ruins</span>" });
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">{} · {}</p><h1>{}</h1>", esc(&capital(&words(&format!("{:?}", m.monument_type)))), esc(&b.where_is(m.location)), esc(&m.name));
        let at = b.sites_at.get(&m.location).and_then(|v| v.first().copied()).map(|s| format!(" at {}", b.link(EntityId::Settlement(s)))).unwrap_or_default();
        let ruin = if m.intact { String::new() } else { format!(" It has lain in ruins since {}.", m.destruction_date.map_or(0, |d| d.year)) };
        let _ = write!(o, "<p class=\"lede\">Raised by {}{} in the year {}.{}</p></header>", b.people(m.faction), at, m.built_date.year, ruin);
        for i in &m.inscriptions { let _ = write!(o, "<blockquote>{}</blockquote>", esc(&i.text)); }
        o.push_str(&b.map_fig(&[(m.location, true, m.name.clone())], &format!("{} on the map of the world", m.name)));
        if let Some(e) = m.commemorates.and_then(|e| b.ev(e)) { let _ = write!(o, "<h2>It remembers</h2><ol class=\"story\">{}</ol>", b.line(e)); }
        let honors: Vec<String> = m.honors.iter().filter(|x| b.url((*x).clone()).is_some()).map(|x| b.link(x.clone())).collect();
        if !honors.is_empty() { let _ = write!(o, "<p>It honours {}.</p>", and_list(&honors)); }
        let mut idx: Vec<usize> = b.by_entity.get(&EntityId::Monument(*id)).cloned().unwrap_or_default();
        idx.extend(m.construction_event.iter().chain(m.destruction_event.iter()).filter_map(|e| b.pos.get(e).copied()));
        idx.sort();
        idx.dedup();
        let _ = write!(o, "<h2>Its story</h2>{}", b.story(&idx, false));
        files.push((format!("monument-{}.html", id.0), b.frame(&m.name, &o)));
        search.push((m.name.clone(), "monument", format!("monument-{}.html", id.0)));
        rep.treasures += 1;
    }
    list.push_str("</ul>");
    files.push(("treasures.html".into(), b.frame("Treasures and Monuments", &list)));

    // --- Wars -------------------------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Wars</h1><p class=\"lede\">Every war, in the order of its declaration.</p></header><div class=\"table-wrap\"><table><thead><tr><th>War</th><th>Years</th><th>Sides</th><th class=\"num\">Battles</th><th>Outcome</th></tr></thead><tbody>");
    let mut wars: Vec<_> = sorted(&h.wars);
    wars.sort_by_key(|(id, w)| (w.started, *id));
    for (id, w) in &wars {
        let side = |v: &[FactionId]| v.iter().map(|f| b.people(*f)).collect::<Vec<_>>().join(", ");
        let sides = format!("{} <span class=\"vs\">against</span> {}", side(&w.aggressors), side(&w.defenders));
        let outcome = match (w.victor, w.ended) { (Some(v), _) => format!("{} prevailed", b.people(v)), (None, Some(_)) => "No victor".into(), (None, None) => "Still fought".into() };
        let years = format!("{}–{}", w.started.year, w.ended.map(|d| d.year.to_string()).unwrap_or_default());
        let _ = write!(list, "<tr><td>{}</td><td class=\"yrs\">{}</td><td>{}</td><td class=\"num\">{}</td><td>{}</td></tr>", a(&format!("war-{}.html", id.0), &tidy(&w.name)), years, sides, w.battles.len(), outcome);
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">A war of {} · {}</p><h1>{}</h1>", esc(&words(&format!("{:?}", w.cause))), years, esc(&tidy(&w.name)));
        let fallen = w.casualties.aggressor_losses + w.casualties.defender_losses + w.casualties.civilian_losses;
        let _ = write!(o, "<p class=\"lede\">{}. {}. It saw {}, {} and {} fallen{}.</p></header>", capital(&sides), outcome, plural(w.battles.len(), "battle", "battles"), plural(w.sieges.len(), "siege", "sieges"), fallen,
            if w.casualties.settlements_destroyed > 0 { format!(", and {} destroyed", plural(w.casualties.settlements_destroyed as usize, "town", "towns")) } else { String::new() });
        let pins: Vec<((usize, usize), bool, String)> = w.battles.iter().chain(w.sieges.iter()).filter_map(|e| b.ev(*e)).filter_map(|e| e.location.map(|l| (l, false, e.title.clone()))).collect();
        o.push_str(&b.map_fig(&pins, &format!("The fields of {}", tidy(&w.name))));
        let idx: Vec<usize> = crate::history::collections::war_events(h, w).into_iter().filter_map(|e| b.pos.get(&e).copied()).collect();
        let _ = write!(o, "<h2>Its course</h2>{}", b.story(&idx, false));
        files.push((format!("war-{}.html", id.0), b.frame(&tidy(&w.name), &o)));
        search.push((tidy(&w.name), "war", format!("war-{}.html", id.0)));
        rep.wars += 1;
    }
    list.push_str("</tbody></table></div>");
    files.push(("wars.html".into(), b.frame("The Wars", &list)));

    // --- Faiths -----------------------------------------------------------------------------
    let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Faiths</h1><p class=\"lede\">The religions of the peoples, and the cults that worship beasts.</p></header><ul class=\"plain\">");
    for (id, r) in sorted(&h.religions) {
        let followers: Vec<String> = sorted(&h.factions).into_iter().filter(|(fid, f)| f.state_religion == Some(id) || r.follower_factions.contains(fid)).map(|(fid, _)| b.people(fid)).collect();
        let _ = write!(list, "<li><span class=\"yr\">{}</span> {} <span class=\"meta\">{}</span></li>", r.origin_date.year, b.link(EntityId::Religion(id)), if followers.is_empty() { "kept by no people now".to_string() } else { format!("kept by {}", followers.len()) });
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">{} · since {}</p><h1>{}</h1>", if r.is_monotheistic() { "One god" } else { "Many gods" }, r.origin_date.year, esc(&tidy(&r.name)));
        let founder = r.founder.filter(|f| h.figures.contains_key(f)).map(|f| format!(", founded by {}", b.link(EntityId::Figure(f)))).unwrap_or_default();
        let kept = if followers.is_empty() { "No people keeps it now.".to_string() } else { format!("It is kept by {}.", and_list(&followers)) };
        let _ = write!(o, "<p class=\"lede\">A faith of the year {}{}. {}</p></header>", r.origin_date.year, founder, kept);
        let gods: Vec<String> = r.deities.iter().filter_map(|d| h.deities.get(d)).map(|d| {
            let ep = d.epithets.first().map(|e| format!(" {}", e)).unwrap_or_default();
            let doms: Vec<String> = d.domains.iter().map(|x| words(&format!("{:?}", x))).collect();
            format!("<li><b>{}{}</b> <span class=\"meta\">{}, {}{}</span></li>", esc(&d.name), esc(&ep), words(&format!("{:?}", d.deity_type)), words(&format!("{:?}", d.alignment)), if doms.is_empty() { String::new() } else { format!("; of {}", esc(&and_list(&doms))) })
        }).collect();
        if !gods.is_empty() { let _ = write!(o, "<h2>Its gods</h2><ul class=\"plain\">{}</ul>", gods.join("")); }
        if !r.doctrines.is_empty() { let _ = write!(o, "<h2>Its teachings</h2><p>{}.</p>", esc(&capital(&and_list(&r.doctrines.iter().map(|d| words(&format!("{:?}", d))).collect::<Vec<_>>())))); }
        let pins: Vec<((usize, usize), bool, String)> = r.holy_sites.iter().map(|l| (*l, true, format!("A holy site of {}", tidy(&r.name)))).collect();
        o.push_str(&b.map_fig(&pins, &format!("The holy sites of {}", tidy(&r.name))));
        let name = tidy(&r.name);
        let mut idx = b.by_entity.get(&EntityId::Religion(id)).cloned().unwrap_or_default();
        idx.extend(b.events.iter().enumerate().filter(|(_, e)| e.description.contains(&name) || e.title.contains(&name)).map(|(i, _)| i));
        idx.sort();
        idx.dedup();
        let _ = write!(o, "<h2>Its story</h2>{}", b.story(&idx, true));
        files.push((format!("religion-{}.html", id.0), b.frame(&tidy(&r.name), &o)));
        search.push((tidy(&r.name), "faith", format!("religion-{}.html", id.0)));
        rep.faiths += 1;
    }
    list.push_str("</ul>");
    let cults: Vec<String> = sorted(&h.cults).into_iter().map(|(_, c)| format!("<li><span class=\"yr\">{}</span> {}, worshipping {}</li>", c.founded.year, esc(&tidy(&c.name)), b.link(EntityId::LegendaryCreature(c.worshipped_creature)))).collect();
    if !cults.is_empty() { let _ = write!(list, "<h2>Cults of the beasts</h2><ul class=\"plain\">{}</ul>", cults.join("")); }
    files.push(("faiths.html".into(), b.frame("The Faiths", &list)));

    // --- Camps ------------------------------------------------------------------------------
    {
        let mut camps: Vec<&Legend> = camps.iter().collect();
        camps.sort_by(|x, y| x.code.cmp(&y.code));
        let mut list = String::from("<header class=\"title\"><p class=\"eyebrow\">The record</p><h1>The Camps</h1><p class=\"lede\">Colonies founded in this world, as their legends were kept.</p></header>");
        if camps.is_empty() { list.push_str("<p class=\"meta\">No camp has been founded here yet.</p>"); }
        list.push_str("<ul class=\"plain\">");
        for (k, l) in camps.iter().enumerate() {
            let url = format!("camp-{}.html", k + 1);
            let name = capital(&l.name);
            let _ = write!(list, "<li>{} <span class=\"meta\">{}, day {}</span></li>", a(&url, &name), esc(&l.fate), l.day);
            let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">A camp · {}</p><h1>{}</h1>", esc(&b.where_is(l.tile)), esc(&name));
            let _ = write!(o, "<p class=\"lede\">{} came; at the last telling, on day {}, {} lived. Its fate: {}.</p></header>", l.came, l.day, l.alive, esc(&l.fate));
            o.push_str(&b.map_fig(&[(l.tile, true, name.clone())], &format!("{} on the map of the world", name)));
            if !l.deeds.is_empty() {
                o.push_str("<h2>Its great moments</h2><ol class=\"story\">");
                for d in &l.deeds {
                    let (day, text) = d.split_once(": ").unwrap_or(("", d));
                    let _ = write!(o, "<li><span class=\"yr\">{}</span> {}</li>", esc(day), b.link_names(text, b.named_in(text)));
                }
                o.push_str("</ol>");
            }
            if !l.slain.is_empty() {
                let s: Vec<String> = l.slain.iter().map(|n| b.link_names(n, b.named_in(n))).collect();
                let _ = write!(o, "<h2>Beasts slain</h2><p>{}</p>", s.join(" · "));
            }
            if let Some(r) = &l.relic {
                let _ = write!(o, "<h2>A thing of the old world</h2><p>{}.</p>", capital(&b.link_names(r, b.named_in(r))));
            }
            if !l.regards.is_empty() {
                o.push_str("<h2>What the peoples thought of it</h2><ul class=\"plain\">");
                for (f, people, total, cause) in &l.regards {
                    let who = if h.factions.contains_key(f) { b.people(*f) } else { esc(people) };
                    let _ = write!(o, "<li>{} <span class=\"meta\">({:+})</span>: it {}</li>", who, total, esc(cause));
                }
                o.push_str("</ul>");
            }
            let _ = write!(o, "<p class=\"meta\">Its code: <code>{}</code></p>", esc(&l.code));
            files.push((url.clone(), b.frame(&name, &o)));
            search.push((name, "camp", url));
            rep.camps += 1;
        }
        list.push_str("</ul>");
        files.push(("camps.html".into(), b.frame("The Camps", &list)));
    }

    // --- The index --------------------------------------------------------------------------
    {
        let present = crate::history::ages::current(h).map(|e| e.name.clone()).unwrap_or_default();
        let mut o = format!("<header class=\"title\"><p class=\"eyebrow\">Legends</p><h1>The Legends of {}</h1><p class=\"lede\">{} years written down in {} events: {}, {}, {}, {}, {}, {} and {}.{}</p></header>",
            esc(&b.world_name), h.current_date.year, b.events.len(),
            plural(rep.peoples, "people", "peoples"), plural(rep.sites, "site", "sites"), plural(rep.figures, "figure", "figures"), plural(rep.beasts, "beast of legend", "beasts of legend"),
            plural(h.artifacts.len(), "treasure", "treasures"), plural(rep.wars, "war", "wars"), plural(rep.faiths, "faith", "faiths"),
            if present.is_empty() { String::new() } else { format!(" The present age is {}.", a(&format!("age-{}.html", h.timeline.eras.len()), &present.replacen("The ", "the ", 1))) });
        if map_size.is_some() { let _ = write!(o, "<figure class=\"map\"><div class=\"map-box\"><img src=\"map.png\" alt=\"The world of {0}\"></div><figcaption>The world as it stood in the year {1}</figcaption></figure>", esc(&b.world_name), h.current_date.year); }
        let _ = write!(o, "<section class=\"part\"><h2>The Ages</h2>{}</section>", ages_list);
        let record = [("peoples.html", "The Peoples", rep.peoples), ("sites.html", "The Sites", rep.sites), ("figures.html", "The Figures", rep.figures), ("beasts.html", "The Beasts of Legend", rep.beasts),
            ("treasures.html", "Treasures and Monuments", rep.treasures), ("wars.html", "The Wars", rep.wars), ("faiths.html", "The Faiths", rep.faiths), ("arts.html", "The Arts", facs.len()), ("years.html", "The Years", rep.years), ("camps.html", "The Camps", rep.camps)];
        o.push_str("<section class=\"part\"><h2>The Record</h2><ul class=\"record\">");
        for (u, t, n) in record { let _ = write!(o, "<li><a href=\"{}\">{}</a><span>{}</span></li>", u, t, n); }
        o.push_str("</ul></section>");
        if !b.tales.is_empty() {
            o.push_str("<section class=\"part\"><h2>Tales Worth Telling</h2><div class=\"cards\">");
            for t in b.tales.iter().take(12) {
                let evs: Vec<String> = t.events.iter().filter_map(|e| b.ev(*e)).map(|e| format!("<a href=\"{}\">{}</a>", Book::ev_url(e), e.date.year)).collect();
                let _ = write!(o, "<article class=\"card\"><h3>{}</h3><p>{}</p><p class=\"meta\">{} · {}</p></article>", esc(&t.title), b.link_names(&t.text, b.named_in(&t.text)), esc(t.kind.label()), evs.join(", "));
            }
            o.push_str("</div></section>");
        }
        files.push(("index.html".into(), b.frame("", &o)));
    }

    // --- Shared files -----------------------------------------------------------------------
    let mut js = String::from("var LEGENDS=[");
    for (i, (n, k, u)) in search.iter().enumerate() {
        if i > 0 { js.push(','); }
        let _ = write!(js, "[{},\"{}\",\"{}\"]", serde_json::to_string(n).unwrap_or_default(), k, u);
    }
    js.push_str("];\n");
    js.push_str(JS);
    std::fs::write(dir.join("legends.js"), js)?;
    std::fs::write(dir.join("legends.css"), CSS)?;
    rep.pages = files.len();
    for (name, body) in &files { std::fs::write(dir.join(name), body)?; }
    Ok(rep)
}

fn how_kept(m: &crate::history::objects::artifacts::AcquisitionMethod) -> &'static str {
    use crate::history::objects::artifacts::AcquisitionMethod::*;
    match m { Created => "from its making", Inherited => "inherited", Gifted => "a gift", Stolen => "stolen", Looted => "looted", Found => "found", Purchased => "bought", Won => "won" }
}

fn ordinal(n: usize) -> &'static str {
    ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"].get(n.saturating_sub(1)).copied().unwrap_or("next")
}

/// Where a treasure is now, in a few words (escaped HTML with a link).
fn artifact_now(b: &Book, x: &crate::history::objects::artifacts::Artifact) -> String {
    if x.destroyed { return "destroyed".into(); }
    match &x.current_owner {
        Some(EntityId::Figure(f)) if b.h.figures.contains_key(f) => format!("held by {}", b.link(EntityId::Figure(*f))),
        Some(EntityId::LegendaryCreature(c)) if b.h.legendary_creatures.contains_key(c) => format!("in the hoard of {}", b.link(EntityId::LegendaryCreature(*c))),
        Some(EntityId::Faction(f)) if b.h.factions.contains_key(f) => format!("kept by {}", b.people(*f)),
        _ if x.lost => "lost".into(),
        _ => "whereabouts unknown".into(),
    }
}

const CSS: &str = r#"
:root {
  --page: #efe5cf; --page-deep: #e4d6b8; --ink: #382a20; --ink-soft: #6b5846; --rule: #c8b48f;
  --rubric: #9c3324; --sea: #2c4a5c; --wash: #f6efdf;
  --display: "IM Fell English", "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif;
  --display-sc: "IM Fell English SC", "IM Fell English", Georgia, serif;
  --body: "Alegreya", "Iowan Old Style", Georgia, serif;
  --label: "Alegreya Sans SC", "Gill Sans", "Trebuchet MS", sans-serif;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --page: #1d1813; --page-deep: #262019; --ink: #e8dcc2; --ink-soft: #b3a183; --rule: #4a3f31;
    --rubric: #e0826a; --sea: #9cc0d2; --wash: #2a231b; color-scheme: dark;
  }
}
:root[data-theme="dark"] {
  --page: #1d1813; --page-deep: #262019; --ink: #e8dcc2; --ink-soft: #b3a183; --rule: #4a3f31;
  --rubric: #e0826a; --sea: #9cc0d2; --wash: #2a231b; color-scheme: dark;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--page); color: var(--ink); font-family: var(--body); font-size: 1.0625rem; line-height: 1.6; }
a { color: var(--sea); text-decoration-color: color-mix(in srgb, var(--sea) 40%, transparent); text-underline-offset: 2px; }
a:hover { text-decoration-color: currentColor; }
:focus-visible { outline: 2px solid var(--rubric); outline-offset: 2px; }
.bar { position: sticky; top: 0; z-index: 2; display: flex; flex-wrap: wrap; align-items: center; gap: 0.4rem 1.2rem; padding: 0.55rem 1rem; background: var(--page-deep); border-bottom: 1px solid var(--rule); }
.home { font-family: var(--display-sc); font-size: 1.1rem; color: var(--rubric); text-decoration: none; }
.bar nav { display: flex; flex-wrap: wrap; gap: 0.2rem 0.8rem; font-family: var(--label); font-size: 0.85rem; letter-spacing: 0.05em; }
.bar nav a { color: var(--ink); text-decoration: none; }
.bar nav a:hover { color: var(--rubric); }
.find { position: relative; margin-left: auto; min-width: min(100%, 15rem); }
.find input { width: 100%; font: inherit; font-size: 0.95rem; padding: 0.3rem 0.55rem; border: 1px solid var(--rule); border-radius: 3px; background: var(--wash); color: var(--ink); }
#hits { position: absolute; right: 0; left: 0; top: 100%; margin: 0.2rem 0 0; padding: 0; list-style: none; background: var(--wash); border: 1px solid var(--rule); max-height: 60vh; overflow-y: auto; }
#hits:empty { display: none; }
#hits li a { display: flex; justify-content: space-between; gap: 0.6rem; padding: 0.25rem 0.55rem; text-decoration: none; color: var(--ink); }
#hits li a span { font-family: var(--label); font-size: 0.75rem; color: var(--ink-soft); }
#hits li a:hover, #hits li a.on { background: var(--page-deep); color: var(--rubric); }
main { max-width: 48rem; margin: 0 auto; padding: 2rem 1rem 4rem; min-width: 0; }
h1, h2, h3 { font-family: var(--display); font-weight: 400; text-wrap: balance; line-height: 1.15; }
h1 { font-size: clamp(2.1rem, 6vw, 3.2rem); margin: 0 0 0.8rem; }
h2 { font-size: clamp(1.45rem, 4vw, 1.9rem); margin: 2rem 0 0.5rem; }
h3 { font-size: 1.25rem; margin: 0 0 0.25rem; }
.title { padding-bottom: 1.5rem; border-bottom: 1px solid var(--rule); }
.title::after { content: ""; display: table; clear: both; }
.eyebrow { font-family: var(--label); letter-spacing: 0.1em; color: var(--rubric); font-size: 0.85rem; margin: 0 0 0.3rem; }
.lede { font-size: 1.15rem; line-height: 1.55; margin: 0; }
.arms { float: right; margin: 0 0 0.5rem 1rem; }
.card .arms { margin: 0 0 0.3rem 0.6rem; }
.meta { color: var(--ink-soft); font-size: 0.92rem; }
.label { font-family: var(--label); font-size: 0.8rem; letter-spacing: 0.07em; color: var(--ink-soft); margin: 0.4rem 0 0.1rem; }
.yr { font-family: var(--label); font-size: 0.82rem; color: var(--ink-soft); font-variant-numeric: tabular-nums; }
a.yr { color: var(--rubric); text-decoration: none; }
a.yr:hover { text-decoration: underline; }
.why { font-family: var(--label); font-size: 0.78rem; color: var(--rubric); }
.dead { color: var(--ink-soft); font-size: 0.8em; margin-left: 0.1em; }
.desc { font-style: italic; }
blockquote { margin: 0.6rem 0; padding: 0.4rem 0.9rem; border-left: 2px solid var(--rubric); background: var(--wash); font-style: italic; }
.story, .plain { list-style: none; padding: 0; margin: 0.4rem 0; display: grid; gap: 0.3rem; }
.story li { padding-left: 3.4rem; text-indent: -3.4rem; }
.story li > .yr:first-child { display: inline-block; width: 3rem; text-indent: 0; text-align: right; margin-right: 0.4rem; }
.names, .years { line-height: 1.9; }
.years a { display: inline-block; min-width: 2.6rem; font-variant-numeric: tabular-nums; }
.age-sum { margin: 1rem 0 0; padding: 0.8rem 1rem; background: var(--wash); border-left: 2px solid var(--rubric); }
.index { padding-left: 1.4rem; display: grid; gap: 0.3rem; }
.record { list-style: none; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr)); gap: 0.3rem 1.5rem; }
.record li { display: flex; justify-content: space-between; gap: 0.6rem; border-bottom: 1px dotted var(--rule); padding: 0.25rem 0; }
.record li span { color: var(--ink-soft); font-variant-numeric: tabular-nums; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 19rem), 1fr)); gap: 0.6rem 1.5rem; }
.card { padding: 0.8rem 0 0.9rem; border-top: 1px solid var(--rule); min-width: 0; }
.card p { margin: 0.25rem 0; }
.part { padding-top: 0.5rem; }
.pager { display: flex; justify-content: space-between; gap: 1rem; font-family: var(--label); margin: 0.4rem 0 0; }
.pager span { color: var(--ink-soft); }
.ev { position: relative; padding: 1rem 0 1rem 1.1rem; border-bottom: 1px dotted var(--rule); }
.ev::before { content: ""; position: absolute; left: 0; top: 1.45rem; width: 0.45rem; height: 0.45rem; border-radius: 50%; background: var(--rule); }
.ev.k-war::before, .ev.k-fall::before { background: var(--rubric); }
.ev.k-found::before, .ev.k-beast::before { background: var(--sea); }
.ev:target { background: var(--wash); }
.ev p { margin: 0.3rem 0; }
.season { font-family: var(--label); font-size: 0.8rem; letter-spacing: 0.08em; color: var(--rubric); margin-right: 0.3rem; }
.chain ol { list-style: none; margin: 0; padding: 0 0 0 0.9rem; border-left: 1px solid var(--rule); font-size: 0.95rem; }
.tale { margin-top: 0.6rem; padding: 0.5rem 0.8rem; background: var(--wash); border-left: 2px solid var(--rubric); font-size: 0.95rem; }
.tale p { margin: 0.15rem 0; }
.map { margin: 1.2rem 0; }
.map-box { position: relative; line-height: 0; border: 1px solid var(--rule); }
.map-box img { width: 100%; height: auto; display: block; }
.pin { position: absolute; width: 9px; height: 9px; margin: -4.5px 0 0 -4.5px; border-radius: 50%; background: var(--rubric); border: 1.5px solid #f6efdf; box-shadow: 0 0 0 1px #382a20; }
.pin.big { width: 14px; height: 14px; margin: -7px 0 0 -7px; }
.crop-box { position: relative; width: 360px; height: 240px; max-width: 100%; background-image: url(map.png); background-repeat: no-repeat; border: 1px solid #382a20; box-shadow: 0 0 0 4px #efe5cf, 0 0 0 5px #8a7458; }
.map figcaption { font-family: var(--label); font-size: 0.8rem; letter-spacing: 0.05em; color: var(--ink-soft); margin-top: 0.3rem; }
.table-wrap { overflow-x: auto; }
table { border-collapse: collapse; width: 100%; font-size: 0.95rem; }
th { font-family: var(--label); font-weight: 500; letter-spacing: 0.06em; color: var(--ink-soft); text-align: left; border-bottom: 1px solid var(--ink); padding: 0.4rem 0.5rem; }
td { border-bottom: 1px dotted var(--rule); padding: 0.45rem 0.5rem; vertical-align: top; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.yrs { white-space: nowrap; font-variant-numeric: tabular-nums; }
.vs { color: var(--ink-soft); font-style: italic; }
code { font-size: 0.85rem; }
footer { max-width: 48rem; margin: 0 auto; padding: 1rem 1rem 3rem; border-top: 1px solid var(--rule); color: var(--ink-soft); font-size: 0.9rem; }
@media (max-width: 640px) { .find { margin-left: 0; flex: 1 1 100%; } .story li { padding-left: 0; text-indent: 0; } .story li > .yr:first-child { width: auto; } }
"#;

const JS: &str = r#"
(function () {
  var q = document.getElementById('q'), hits = document.getElementById('hits');
  if (!q || !hits) return;
  var sel = -1;
  function show() {
    var t = q.value.trim().toLowerCase();
    hits.innerHTML = ''; sel = -1;
    if (t.length < 2) return;
    var first = [], rest = [];
    for (var i = 0; i < LEGENDS.length; i++) {
      var n = LEGENDS[i][0].toLowerCase(), k = n.indexOf(t);
      if (k < 0) continue;
      (k === 0 || n.charAt(k - 1) === ' ' ? first : rest).push(LEGENDS[i]);
      if (first.length >= 20) break;
    }
    first.concat(rest).slice(0, 20).forEach(function (r) {
      var li = document.createElement('li'), a = document.createElement('a'), s = document.createElement('span');
      a.href = r[2]; a.textContent = r[0]; s.textContent = r[1]; a.appendChild(s); li.appendChild(a); hits.appendChild(li);
    });
  }
  function mark() { var as = hits.querySelectorAll('a'); for (var i = 0; i < as.length; i++) as[i].classList.toggle('on', i === sel); }
  q.addEventListener('input', show);
  q.addEventListener('keydown', function (e) {
    var as = hits.querySelectorAll('a');
    if (e.key === 'ArrowDown') { sel = Math.min(sel + 1, as.length - 1); mark(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { sel = Math.max(sel - 1, 0); mark(); e.preventDefault(); }
    else if (e.key === 'Enter' && as.length) { location.href = as[Math.max(sel, 0)].getAttribute('href'); }
    else if (e.key === 'Escape') { q.value = ''; show(); }
  });
})();
"#;
