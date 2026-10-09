//! The world moves while one plays (DF's adventure mode runs the world's history on as one
//! walks): the history goes on a season every `SEASON` ticks of the adventure's clock (fifteen
//! days: the world's years pass faster than the hero's, so wars, sieges and the Shadow's march
//! are seen in a play), and what it does comes into the adventure: towns razed (their streets
//! burned in the land), towns founded, new lords, each town's news of this season, word of great
//! events reaching the adventurer.
//!
//! The adventurer is in the history too: arriving makes them a figure of it, and their deeds
//! (beasts and bosses slain, relics found, quests done, falls, the Shadow broken) are recorded in
//! its chronicle as events (`EventType::AdventurerDeed`, `CreatureSlain`, `ArtifactFound`), so
//! word of them spreads by the knowledge layer and bards sing them (`Game::songs`).
//!
//! The history is not saved with the adventure: the host (the window, the bot) keeps a `Living`
//! made from the world's history and brings it up to the adventure's seasons (`sync`). On load
//! it is rebuilt by replay: each season stepped with its own seed, the adventurer's deeds put in
//! at the season they were done (`Game::hero_events`), so it is the same history again.

use super::game::{Game, Tone};
use super::site::{SiteKind, SiteSpec};
use super::surface::CH;
use crate::history::events::types::{Event, EventType as E};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, FigureId};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Ticks a season of the history takes in the adventure (fifteen days).
pub const SEASON: u64 = 15 * super::land::DAY;

/// What the adventurer did, for the chronicle.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum DeedKind {
    /// Set out from home: the adventurer becomes a figure of the history.
    Arrived,
    /// Slew a beast of the history (its id).
    BeastSlain(u64),
    /// Slew a named enemy that is no beast of the history.
    BossSlain,
    /// Found an artifact of the history (its id).
    RelicFound(u64),
    QuestDone,
    Fell,
    ShadowBroken,
}

/// A deed for the chronicle, with the season it was done in (for the replay).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HeroEvent {
    pub season: u32,
    pub kind: DeedKind,
    pub title: String,
    pub text: String,
    pub tile: (usize, usize),
}

/// The history as it goes on beside the adventure (kept by the host).
pub struct Living {
    /// Shared with the game (`Game::history`) for talk; copied on write when a season steps.
    pub history: std::sync::Arc<WorldHistory>,
    pub data: crate::history::data::GameData,
    /// Seasons stepped since the adventure began.
    pub ran: u32,
    /// Hero events put into the history so far (an index into `Game::hero_events`).
    applied: usize,
}

impl Living {
    /// The history as it stood when the adventure began.
    pub fn new(history: &WorldHistory) -> Living {
        let data = if std::path::Path::new("data").is_dir() { crate::history::data::GameData::load_from(std::path::Path::new("data")) } else { crate::history::data::GameData::defaults() };
        Living { history: std::sync::Arc::new(history.clone()), data, ran: 0, applied: 0 }
    }

    /// Bring the history up to the adventure: its deeds put in, the seasons its clock has passed
    /// stepped; what changed comes into the adventure. Returns whether anything did.
    pub fn sync(&mut self, g: &mut Game, world: &crate::world::WorldData) -> bool {
        let target = ((g.turn / SEASON) as u32).max(g.seasons);
        // A loaded adventure: replay what was played, quietly (the save holds its results).
        let replay = self.ran < g.seasons;
        let queued = !g.deed_queue.is_empty();
        if self.ran >= target && !queued && self.applied >= g.hero_events.len() { return false; }
        // (The game lets go of its handle while the history changes, so it is not copied.)
        g.history = None;
        let before = self.history.chronicle.events.len();
        let mut stepped = false;
        loop {
            // Deeds of this season (replayed ones first, then new ones).
            while self.applied < g.hero_events.len() && g.hero_events[self.applied].season <= self.ran {
                let e = g.hero_events[self.applied].clone();
                self.apply(g, &e);
                self.applied += 1;
            }
            if self.ran >= g.seasons {
                for mut e in std::mem::take(&mut g.deed_queue) {
                    e.season = self.ran;
                    self.apply(g, &e);
                    g.hero_events.push(e);
                    self.applied += 1;
                }
            }
            if self.ran >= target { break; }
            let mut rng = ChaCha8Rng::seed_from_u64(g.seed ^ 0x11F3_5EA5 ^ (self.ran as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            crate::history::simulation::step::simulate_step(std::sync::Arc::make_mut(&mut self.history), world, &self.data, &mut rng);
            self.ran += 1;
            stepped = true;
        }
        g.seasons = self.ran;
        if stepped && !replay { g.on_history(&self.history, before); }
        g.refresh_talk(&self.history);
        g.history = Some(self.history.clone());
        true
    }

    /// Put a deed into the history.
    fn apply(&mut self, g: &mut Game, e: &HeroEvent) {
        let h = std::sync::Arc::make_mut(&mut self.history);
        let date = h.current_date;
        // The town nearest the deed: its people are those who tell it first.
        let near = h.settlements.values().filter(|s| !s.is_destroyed()).min_by_key(|s| (super::world::dist(s.location, e.tile, h.tile_history.width.max(1)), s.id.0)).map(|s| (s.id, s.faction));
        if e.kind == DeedKind::Arrived && g.hero_figure.is_none() {
            let id = h.id_generators.next_figure();
            let faction = g.site(g.hero.temple).and_then(|s| s.settlement).and_then(|sid| h.settlements.get(&crate::history::SettlementId(sid))).map(|s| s.faction).or(near.map(|n| n.1));
            let race = faction.and_then(|f| h.factions.get(&f)).map(|f| f.race_id).or_else(|| h.races.keys().min().copied());
            if let Some(race) = race {
                let mut f = crate::history::entities::figures::Figure::new(id, g.hero.name.clone(), race, crate::history::time::Date::new(date.year.saturating_sub(19), date.season), Default::default());
                f.faction = faction;
                f.epithet = Some("the Wanderer".into());
                f.titles.push("adventurer".into());
                h.figures.insert(id, f);
                g.hero_figure = Some(id.0);
            }
        }
        let fig = g.hero_figure.map(FigureId);
        let (kind, mut parts) = match &e.kind {
            DeedKind::BeastSlain(c) => {
                let cid = crate::history::LegendaryCreatureId(*c);
                if let Some(b) = h.legendary_creatures.get_mut(&cid) { b.death_date = Some(date); }
                (E::CreatureSlain, vec![EntityId::LegendaryCreature(cid)])
            }
            DeedKind::RelicFound(a) => (E::ArtifactFound, vec![EntityId::Artifact(crate::history::ArtifactId(*a))]),
            DeedKind::QuestDone => (E::QuestCompleted, Vec::new()),
            _ => (E::AdventurerDeed, Vec::new()),
        };
        if let Some(f) = fig { parts.insert(0, EntityId::Figure(f)); }
        let id = h.id_generators.next_event();
        let mut ev = Event::new(id, kind, date, e.title.clone(), e.text.clone());
        ev.location = Some(e.tile);
        ev.primary_participants = parts;
        if let Some((_, fac)) = near { ev.factions_involved = vec![fac]; }
        if let Some(f) = fig.and_then(|f| h.figures.get_mut(&f)) {
            f.events.push(id);
            if let DeedKind::BeastSlain(c) = e.kind { f.kills.push(EntityId::LegendaryCreature(crate::history::LegendaryCreatureId(c))); }
        }
        h.chronicle.record(ev);
    }
}

impl Game {
    /// Record a deed for the history (put in by the host's `Living` at its next `sync`).
    pub fn chronicle(&mut self, kind: DeedKind, title: String, text: String) {
        let tile = self.place().filter(|_| !self.on_land()).map(|p| p.spec.tile).unwrap_or(self.tile);
        self.deed_queue.push(HeroEvent { season: self.seasons, kind, title, text, tile });
    }

    /// What the history did since `before` (an index into its chronicle) comes into the adventure:
    /// towns razed and founded, new lords, word of great events.
    pub fn on_history(&mut self, h: &WorldHistory, before: usize) {
        let (w, hh) = (self.world.w, self.world.h);
        let mut changed_tiles: Vec<(usize, usize)> = Vec::new();
        let knowledge = crate::history::knowledge::Knowledge::new(h);
        let year = h.current_date.year;
        // Towns razed: their streets burned and broken, their people gone.
        let razed: Vec<(u32, u64)> = self.sites.iter().filter(|s| s.kind == SiteKind::Town).filter_map(|s| s.settlement.map(|x| (s.id, x))).filter(|(_, x)| h.settlements.get(&crate::history::SettlementId(*x)).map_or(false, |t| t.is_destroyed())).collect();
        for (id, sid) in razed {
            let by = h.chronicle.events[before.min(h.chronicle.events.len())..].iter().rev().find(|e| e.location == h.settlements.get(&crate::history::SettlementId(sid)).map(|t| t.location) && matches!(e.event_type, E::SettlementDestroyed | E::ShadowConquest | E::SiegeEnded | E::Raid | E::Massacre)).map(|e| e.title.clone());
            let Some(s) = self.sites.iter_mut().find(|s| s.id == id) else { continue };
            let name = s.name.clone();
            s.kind = SiteKind::Ruin;
            s.name = format!("the ruins of {}", name);
            s.cause = format!("{} was razed in {}{}; its streets are ash and its people gone.", name, year, by.map(|b| format!(" ({})", b)).unwrap_or_default());
            s.tier = 2;
            s.boss = Some(super::site::BossSpec { def: "bandit".into(), name: format!("the Looter-King of {}", name), scale: 1.5, legend: None, hoard: vec![super::item::Item::new("gold", 90)], story: "He picks the bones of the town.".into() });
            if let Some(t) = s.town.as_mut() { t.razed = Some(year); }
            s.news.clear();
            changed_tiles.push(s.tile);
            let line = format!("Terrible news: {} has fallen and burns.", name);
            self.say(Tone::Danger, line);
            let turn = self.turn;
            self.deeds.push((turn, format!("heard that {} fell", name)));
            if !self.known.contains(&id) { self.known.push(id); }
        }
        // The adventurer's home gone: the nearest living town's temple takes them in.
        if self.site(self.hero.temple).map_or(true, |s| s.kind != SiteKind::Town) {
            let home = self.site(self.hero.temple).map(|s| s.tile).unwrap_or(self.tile);
            if let Some(t) = self.sites.iter().filter(|s| s.kind == SiteKind::Town).min_by_key(|s| (super::world::dist(s.tile, home, w), s.id)).map(|s| s.id) { self.hero.temple = t; }
        }
        // Towns founded.
        let mut setts: Vec<&crate::history::civilizations::settlement::Settlement> = h.settlements.values().filter(|s| !s.is_destroyed() && !self.sites.iter().any(|q| q.settlement == Some(s.id.0))).collect();
        setts.sort_by_key(|s| s.id.0);
        for s in setts {
            let (x, y) = s.location;
            if x >= w || y >= hh || !self.world.land[y * w + x] { continue; }
            let id = self.sites.iter().map(|q| q.id).filter(|q| *q < 900_000).max().unwrap_or(0) + 1;
            let (people, god) = super::world::town_people(h, s);
            let shape = super::world::town_shape(h, s, &self.world.land, &self.world.road, w, hh);
            let spec = SiteSpec { id, kind: SiteKind::Town, name: s.name.clone(), tile: (x, y), seed: super::world::site_seed(self.seed, id, (x, y)), tier: 1, cause: format!("{} was founded in {}.", s.name, s.founded.year), boss: None, treasures: Vec::new(),
                surface: self.world.ground[y * w + x], rock: "granite".into(), floors: 3, people, god, news: super::world::town_news(&knowledge, s), lord: super::world::town_lord(h, s), town: Some(shape), settlement: Some(s.id.0), creature: None, notes: Vec::new() };
            self.sites.push(spec);
            self.rumour((x, y));
            if !self.known.contains(&id) { self.known.push(id); }
            changed_tiles.push((x, y));
            self.say(Tone::Quest, format!("Word comes of a new town, {}, founded {} of here.", s.name, super::quest::direction(self.tile, (x, y), w)));
        }
        // New lords in the capitals (and in the hall's chair).
        let lords: Vec<(u32, Option<(String, String)>)> = self.sites.iter().filter(|s| s.kind == SiteKind::Town).filter_map(|s| s.settlement.and_then(|x| h.settlements.get(&crate::history::SettlementId(x))).map(|t| (s.id, super::world::town_lord(h, t)))).collect();
        for (id, lord) in lords {
            let Some(s) = self.sites.iter_mut().find(|s| s.id == id) else { continue };
            if s.lord == lord || lord.is_none() { continue; }
            let town = s.name.clone();
            s.lord = lord.clone();
            let (name, title) = lord.unwrap();
            self.rename_lord(id, &name, &title);
            if super::world::dist(self.site(id).map(|s| s.tile).unwrap_or(self.tile), self.tile, w) <= 12 { self.say(Tone::Quest, format!("{} rules {} now, as {}.", name, town, title)); }
        }
        // Word of great events reaching the town nearest the adventurer.
        let near = self.sites.iter().filter(|s| s.kind == SiteKind::Town).filter_map(|s| s.settlement.map(|x| (super::world::dist(s.tile, self.tile, w), crate::history::SettlementId(x)))).min_by_key(|t| (t.0, t.1 .0)).map(|t| t.1);
        let mut told = 0;
        for e in h.chronicle.events[before.min(h.chronicle.events.len())..].iter().rev() {
            if told >= 2 { break; }
            if e.event_type == E::AdventurerDeed || crate::history::knowledge::fame(e) < crate::history::knowledge::Fame::Great { continue; }
            if near.map_or(false, |s| knowledge.town_knows(s, e)) { self.say(Tone::Quest, format!("News reaches you: {}.", crate::history::knowledge::account(h, e, None).line())); told += 1; }
        }
        // Each town's news, as of this season.
        let news: Vec<(u32, Vec<String>)> = self.sites.iter().filter(|s| s.kind == SiteKind::Town).filter_map(|s| s.settlement.and_then(|x| h.settlements.get(&crate::history::SettlementId(x))).map(|t| (s.id, super::world::town_news(&knowledge, t)))).collect();
        for (id, n) in news { if let Some(s) = self.sites.iter_mut().find(|s| s.id == id) { s.news = n; } }
        // The land where places changed is made anew.
        if !changed_tiles.is_empty() {
            self.set_atlas();
            self.remake_tiles(&changed_tiles);
        }
        let _ = CH;
    }

    /// The songs and news each town has of the adventurer and the world, for talk.
    pub fn refresh_talk(&mut self, h: &WorldHistory) {
        let Some(fig) = self.hero_figure.map(FigureId) else { return };
        let knowledge = crate::history::knowledge::Knowledge::new(h);
        let deeds: Vec<&Event> = h.figures.get(&fig).map(|f| f.events.iter().filter_map(|e| h.chronicle.get(*e)).collect()).unwrap_or_default();
        self.songs.clear();
        for s in self.sites.iter().filter(|s| s.kind == SiteKind::Town) {
            let Some(t) = s.settlement.and_then(|x| h.settlements.get(&crate::history::SettlementId(x))) else { continue };
            let v: Vec<String> = deeds.iter().filter(|e| e.event_type != E::AdventurerDeed || !e.title.contains("sets out")).filter(|e| knowledge.town_knows(t.id, e)).map(|e| crate::history::knowledge::account(h, e, Some(t.faction)).line()).collect();
            if !v.is_empty() { self.songs.insert(s.id, v); }
        }
    }

    /// A new lord in town `id`'s hall: the person in the chair takes the name.
    fn rename_lord(&mut self, id: u32, name: &str, title: &str) {
        for ch in self.chunks.values_mut() { for n in ch.npcs.iter_mut().filter(|n| n.home == id && n.role == super::actor::Role::Lord) { n.name = name.to_string(); n.of = title.to_string(); } }
        if let Some(p) = self.land.as_mut() { for n in p.npcs.iter_mut().filter(|n| n.home == id && n.role == super::actor::Role::Lord) { n.name = name.to_string(); n.of = title.to_string(); } }
    }

    /// Make the land of these tiles anew (their places changed): what was kept there is let go.
    pub fn remake_tiles(&mut self, tiles: &[(usize, usize)]) {
        let on = self.on_land();
        let at = if on { Some(self.global(self.x, self.y)) } else { None };
        if on { self.store_land(); }
        for t in tiles {
            let k = (t.0 as u32, t.1 as u32);
            self.chunks.remove(&k);
            self.pristine.remove(&k);
        }
        if let (true, Some(g)) = (on, at) {
            self.land = None;
            self.build_land();
            if let Some((x, y)) = self.local(g) {
                let (x, y) = if self.floor().map_or(false, |f| f.walkable(x, y)) { (x, y) } else { self.open_near(x, y) };
                self.x = x; self.y = y;
            }
            self.look();
        }
    }
}
