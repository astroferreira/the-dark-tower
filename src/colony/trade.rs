//! Caravans: the world comes to trade, and brings news.
//!
//! The idea is Dwarf Fortress's seasonal caravans: traders of the settlers' own people come from
//! a real town of the history, buy what the camp has made and bring what it lacks. Here the
//! partner is the nearest living town of the first settler's people (`partner`, read once from
//! the history at founding), its goods from what that people holds (iron among them), and its
//! news the latest great events of the chronicle near it. A caravan sets out each season; three
//! traders walk in from the town's side of the map, trade at the fire, and walk home. They take
//! the camp's works (`craft.rs`) other than its masterworks, paying in meals and, once, in iron
//! tools; with nothing to buy they share the news and move on.

use super::*;
use super::creatures::{Creature, CreatureKind};
use crate::history::{EventId, FactionId};

/// The town that trades with the camp.
#[derive(Clone, Debug)]
pub struct Partner {
    pub town: String,
    pub people: String,
    pub faction: FactionId,
    /// Its world tile (the caravan comes from that side).
    pub from: (usize, usize),
    /// Days' walk.
    pub days: u64,
    /// It can sell iron tools.
    pub iron: bool,
    /// News it carries (the last 30 years' great events near the town): (a line, the event).
    pub news: Vec<(String, EventId)>,
    /// For each item of news: the towns and peoples it touches, and whether it is good (+1) or
    /// bad (-1) for them.
    pub touches: Vec<(Vec<crate::history::SettlementId>, Vec<FactionId>, i8)>,
}

/// The camp's trading partner: the nearest living town of `people`, else (their people gone) the
/// nearest living town of anyone.
pub fn partner(h: &crate::history::world_state::WorldHistory, tile: (usize, usize), people: Option<FactionId>) -> Option<Partner> {
    use crate::history::civilizations::economy::ResourceType as R;
    let w = h.tile_history.width.max(1);
    let dist = |a: (usize, usize)| { let dx = a.0.abs_diff(tile.0); dx.min(w.saturating_sub(dx)) + a.1.abs_diff(tile.1) };
    let living = |f: FactionId| h.factions.get(&f).map_or(false, |x| x.is_active());
    let town = people.filter(|f| living(*f)).and_then(|f| h.settlements.values().filter(|s| s.faction == f && !s.is_destroyed()).min_by_key(|s| (dist(s.location), s.id)))
        .or_else(|| h.settlements.values().filter(|s| !s.is_destroyed() && living(s.faction)).min_by_key(|s| (dist(s.location), s.id)))?;
    let f = town.faction;
    let fac = h.factions.get(&f)?;
    let km = dist(town.location) as f32 * 40_075.0 / w as f32;
    let days = (km / 25.0).ceil().max(1.0) as u64;
    let iron = fac.resources.get(&R::Iron).copied().unwrap_or(0) > 0 || fac.resources.get(&R::Copper).copied().unwrap_or(0) > 0;
    // The latest great events within 8 tiles of the town, newest first.
    // Distinct headlines (the same title in another year is old news).
    let mut seen: Vec<String> = Vec::new();
    let news: Vec<(String, EventId)> = h.chronicle.events.iter().rev()
        .filter(|e| e.date.year + 30 >= h.current_date.year)
        .filter(|e| e.is_major && e.location.map_or(false, |l| { let dx = l.0.abs_diff(town.location.0); dx.min(w.saturating_sub(dx)) + l.1.abs_diff(town.location.1) <= 8 }))
        .filter(|e| if seen.contains(&e.title) { false } else { seen.push(e.title.clone()); true })
        .take(6).map(|e| (format!("{} ({})", e.title.trim_end_matches('.'), e.date.year), e.id)).collect();
    use crate::history::events::types::EventType as E;
    let touches = news.iter().filter_map(|(_, id)| h.chronicle.get(*id)).map(|e| {
        let towns = e.primary_participants.iter().filter_map(|p| if let crate::history::EntityId::Settlement(s) = p { Some(*s) } else { None }).collect();
        let good: i8 = match e.event_type {
            E::SettlementDestroyed | E::ShadowConquest | E::SiegeBegun | E::Massacre | E::Plague | E::WarDeclared | E::Raid | E::MonsterRaid | E::HeroDied | E::FactionDestroyed => -1,
            E::SettlementFounded | E::ShadowLiberated | E::ShadowRepelled | E::TreatySigned | E::WarEnded | E::CreatureSlain | E::FactionFounded | E::MonumentBuilt | E::RulerCrowned => 1,
            _ => if e.title.contains("rises again") { 1 } else { 0 },
        };
        (towns, e.factions_involved.clone(), good)
    }).collect();
    Some(Partner { town: town.name.clone(), people: fac.name.clone(), faction: f, from: town.location, days, iron, news, touches })
}

impl Colony {
    /// Migrants arrive (Dwarf Fortress's waves): one to three of those who heard of the camp from
    /// the traders, while there is food for them.
    pub(crate) fn migrants_arrive(&mut self) {
        let Some(d) = self.migrant_day else { return };
        if self.clock.day() < d || self.alive() == 0 || self.departed.is_some() { return; }
        self.migrant_day = None;
        let alive = self.alive() as u32;
        if self.food_stored() < 3 * alive || self.migrants.is_empty() {
            self.note("Travellers come asking after the camp, see how little food is stored, and go back.".into());
            return;
        }
        let n = (1 + (self.works.iter().filter(|w| w.traded).count() / 15)).min(3).min(self.migrants.len());
        let town = self.trade.as_ref().map(|p| p.town.clone()).unwrap_or_else(|| "the road".into());
        let mut names = Vec::new();
        let mut came = Vec::new();
        for _ in 0..n {
            let (name, past) = self.migrants.remove(0);
            names.push(format!("{} ({})", name, past.calling));
            self.add_settler(name, Some(past));
            came.push(self.settlers.len() - 1);
        }
        // Under the Shadow, one may not be what they seem (`night.rs`).
        self.maybe_vampire(&came);
        let line = format!("Migrants arrive from {}: {}. Word of the camp's works reached them with the traders.", town, crate::persona::list(&names));
        self.note(line.clone());
        let at = self.camp;
        self.moment("Migrants".into(), line, format!("because the caravan from {} carried word of what the camp makes", town), at);
        for j in 0..self.settlers.len() {
            if self.settlers[j].alive && self.settlers[j].persona.facet(crate::persona::Facet::Gregariousness) >= 60 {
                self.feel(j, mind::Feel::Friend { with: "the newcomers".into() });
            }
        }
    }

    /// While a war band, outlaws or the Shadow's raiders are foretold, one caravan in two (never
    /// the first) is taken on the road: no trade this season, what the liaison was asked for is lost, the
    /// settlers of the traders' people grieve, and the raiders' coming is the surer for it.
    fn caravan_ambushed(&mut self, p: &Partner) -> bool {
        let Some(a) = self.arc.as_ref() else { return false };
        if !matches!(a.stage, 1 | 2 | 5) || !matches!(a.threat.kind, arc::ThreatKind::Warband | arc::ThreatKind::Outlaws | arc::ThreatKind::Shadow) { return false; }
        // Not by the traders' own people, and never the first caravan (word of the camp must
        // reach the town once).
        if a.threat.faction == Some(p.faction) || self.caravans == 0 { return false; }
        if crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0xA4B5) % 2 != 0 { return false; }
        let who = a.threat.name.split(", led by ").next().unwrap_or("").to_string();
        let day = self.clock.day();
        let dead = 1 + crate::history::settlers::hash_pub(self.seed ^ day, 0xA4B6) % 3;
        let line = format!("The caravan from {} does not come. Toward noon a mule walks in alone, its packs slashed: {} fell on the traders on the road, and {} of them did not live.", p.town, who, dead);
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("The caravan from {} is taken", p.town), line, format!("because {} were on the roads, as was foretold", who), at);
        if let Some(w) = self.request.take() { self.note(format!("The {} the camp had asked of {} lies scattered on the road.", w.word().trim_start_matches("a ").trim_start_matches("sacks of ").trim_start_matches("cask of "), p.town)); }
        for j in 0..self.settlers.len() {
            if self.settlers[j].alive && self.settlers[j].past.as_ref().and_then(|x| x.people) == Some(p.faction) {
                self.feel(j, mind::Feel::News { what: format!("the caravan from {} was taken on the road", p.town), good: false });
            }
        }
        true
    }

    /// Each season a caravan: sent at 08:00 on its day, from the town's side.
    pub(crate) fn caravan_tick(&mut self) {
        let Some(p) = self.trade.clone() else { return };
        let day = self.clock.day();
        // The first comes when word has reached the town (at most a season's wait).
        if self.next_caravan == 0 { self.next_caravan = 15 + p.days.min(SEASON_DAYS); }
        if day < self.next_caravan || self.clock.hour() != 8 || self.clock.minute() != 0 { return; }
        self.next_caravan = day + SEASON_DAYS;
        if self.alive() == 0 || self.attackers_out() { return; }
        // A people sworn to vengeance sends no caravans (`regard.rs`).
        if self.regards.iter().any(|r| r.faction == p.faction && r.acted == Some(false)) {
            self.note(format!("No caravan comes from {} this season: {} have sworn vengeance on the camp.", p.town, p.people));
            return;
        }
        // Raiders on the roads may fall on it first (DF: goblins ambush caravans).
        if self.caravan_ambushed(&p) { return; }
        // From the town's side of the map, like a raid but in daylight and slow.
        let (tx, ty) = self.map.world_tile;
        let d = (p.from.0 as f32 - tx as f32, p.from.1 as f32 - ty as f32);
        let n = (d.0 * d.0 + d.1 * d.1).sqrt().max(0.01);
        let (dx, dy) = if n < 0.5 { (1.0, 0.0) } else { (d.0 / n, d.1 / n) };
        let c = (self.camp.0 as f32, self.camp.1 as f32);
        let mut spawned = 0;
        for k in 0..3 {
            let off = (k as f32 - 1.0) * 2.0;
            let at = ((c.0 + dx * 80.0 - dy * off) as i32, (c.1 + dy * 80.0 + dx * off) as i32);
            let Some(at) = self.passable_near_pub(at) else { continue };
            let path = nav::path(&self.map, at, self.camp, PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            let id = self.new_creature_id();
            self.creatures.push(Creature { kind: CreatureKind::Trader, name: format!("traders of {}", p.town), pos: at, path, stride: 0, leaving: false, size: 1.0, home: at, spawned: self.clock.tick, id });
            spawned += 1;
        }
        if spawned > 0 {
            self.caravans += 1;
            let n = self.caravans;
            self.regard(Some(p.faction), &p.people, Some(p.from), "trade", 2, format!("traded with their caravans {} time{}", n, if n == 1 { "" } else { "s" }));
            self.note(format!("A caravan from {} ({}) comes up the road: three traders with laden mules.", p.town, p.people));
        }
    }

    /// Traders at the fire trade, then go home.
    pub(crate) fn caravan_arrives(&mut self) {
        let camp = self.camp;
        let here = self.creatures.iter().any(|c| c.kind == CreatureKind::Trader && !c.leaving && ((c.pos.0 as i32 - camp.0 as i32).abs().max((c.pos.1 as i32 - camp.1 as i32).abs()) <= 3 || c.path.is_empty()));
        if !here { return; }
        let Some(p) = self.trade.clone() else { return };
        // What they buy: the works, but not the masterworks (the camp keeps those).
        let mut value = 0u32;
        let mut sold: Vec<String> = Vec::new();
        // (With a library the camp keeps its books.)
        let library = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Library);
        for w in self.works.iter_mut().filter(|w| !w.traded && w.quality < 5 && !(library && w.kind == "book")) {
            w.traded = true;
            value += (1 + w.quality as u32).pow(2) * 2 + if w.kind.contains(" set with ") { 6 } else { 0 } + if w.material.ends_with("-bone") { 8 } else { 0 };
            let d = w.describe();
            if sold.len() < 2 && !sold.contains(&d) { sold.push(d); }
        }
        let count = self.works.iter().filter(|w| w.traded).count() as u32 - self.traded_before;
        self.traded_before += count;
        // Each item of news is told once, oldest-told last (the newest first).
        let news = p.news.get((self.caravans as usize).saturating_sub(1)).cloned();
        let news_line = news.as_ref().map(|(n, _)| format!(" They bring news: {}.", n)).unwrap_or_default();
        if value == 0 {
            self.note(format!("The traders of {} find nothing they want; they share the fire and move on.{}", p.town, news_line));
        } else {
            let meals = (value * 2).min(48);
            for _ in 0..meals { self.items.push(Item { kind: ItemKind::Food, at: camp, stored: true, reserved: false }); }
            let tools = p.iron && !self.tools_bought && value >= 12;
            if tools { self.tools_bought = true; }
            let what = if count > 2 { format!("{} and {} more", sold.join(", "), count - 2) } else { sold.join(" and ") };
            self.note(format!("The traders of {} buy {} for {} meals{}.{}", p.town, what, meals, if tools { " and a set of iron tools" } else { "" }, news_line));
            // Word of the camp goes home with them: migrants follow in a few weeks.
            if self.migrant_day.is_none() && !self.migrants.is_empty() && value >= 8 {
                self.migrant_day = Some(self.clock.day() + 12 + crate::history::settlers::hash_pub(self.seed, self.caravans as u64) % 9);
            }
            let at = self.camp;
            if tools { self.moment("Iron from the caravan".into(), format!("The traders of {} leave a set of iron tools for the camp's works.", p.town), format!("because the camp had made {} things worth buying", count), at); }
        }
        // What was asked comes; the liaison asks again (`liaison.rs`).
        self.liaison(&p, value > 0);
        // A lost thing near here: the traders know the tale (`relic.rs`).
        self.relic_told(&p.town);
        // News that touches someone (their people, a town of their past) moves them.
        let k = (self.caravans as usize).saturating_sub(1);
        if let (Some((line, _)), Some((towns, peoples, good))) = (p.news.get(k), p.touches.get(k)) {
            if *good != 0 {
                let mut moved = Vec::new();
                for j in 0..self.settlers.len() {
                    if !self.settlers[j].alive { continue; }
                    let hit = self.settlers[j].past.as_ref().map_or(false, |x| x.towns.iter().any(|t| towns.contains(t)) || x.people.map_or(false, |f| peoples.contains(&f)));
                    if hit { let what = line.clone(); self.feel(j, mind::Feel::News { what, good: *good > 0 }); moved.push(self.settlers[j].name.clone()); }
                }
                if !moved.is_empty() {
                    let n = moved.len();
                    let who = if n > 3 { format!("{} and {} others", moved[..2].join(", "), n - 2) } else { crate::persona::list(&moved) };
                    self.note(format!("{} {} at the news: it is their people's, or their own town's.", who, if *good > 0 { "take heart" } else { "grieve" }));
                }
            }
        }
        // How it sits with each: the greedy and the trade-minded are glad; those who hate the
        // traders' people are not.
        for j in 0..self.settlers.len() {
            if !self.settlers[j].alive { continue; }
            let hates = self.settlers[j].past.as_ref().and_then(|x| x.feeling.as_ref()).map_or(false, |f| (f.0.starts_with("hates") || f.0.starts_with("has not forgiven")) && f.1 == crate::history::EntityId::Faction(p.faction));
            let town = p.town.clone();
            self.feel(j, mind::Feel::Caravan { town, hated: hates, sold: value > 0 });
        }
        for k in 0..self.creatures.len() {
            if self.creatures[k].kind != CreatureKind::Trader { continue; }
            let (from, home) = (self.creatures[k].pos, self.creatures[k].home);
            self.creatures[k].path = nav::path(&self.map, from, home, PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            self.creatures[k].leaving = true;
        }
    }
}
