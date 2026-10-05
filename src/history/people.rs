//! Where people live, and who matters in each town.
//!
//! Figures used to belong only to a people: no figure lived anywhere, and a town had no one in it
//! but numbers. Here every living figure has a home town; towns of some size have notables (a
//! captain from 300 souls, a priest from 800 where the people keeps a faith, a smith from 1,500 or
//! where there is metal to work), filled again when one dies; and when a town falls, its people
//! of note flee to another town of their own (or go into exile, or die defending it), each move
//! a chronicle event caused by the fall. Saved as the world file's `people` field (version 6).

use rand::Rng;
use serde::{Deserialize, Serialize};
use crate::history::*;
use crate::history::det::HashMap;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::entities::figures::Figure;
use crate::history::entities::traits::{DeathCause, Personality};

/// What a figure is to their town.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role { Ruler, Captain, Priest, Smith, Exile }

impl Role {
    pub fn word(self) -> &'static str {
        match self { Role::Ruler => "ruler", Role::Captain => "captain", Role::Priest => "priest", Role::Smith => "smith", Role::Exile => "exile" }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct People {
    /// The town each figure lives in (None: an exile, or the dead).
    pub home: HashMap<FigureId, SettlementId>,
    pub role: HashMap<FigureId, Role>,
}

impl People {
    /// The living notables of a town, by role.
    pub fn notables_of(&self, h: &WorldHistory, town: SettlementId) -> Vec<(FigureId, Role)> {
        let mut v: Vec<(FigureId, Role)> = self.home.iter()
            .filter(|(f, s)| **s == town && h.figures.get(f).map_or(false, |x| x.is_alive()))
            .map(|(f, _)| (*f, self.role.get(f).copied().unwrap_or(Role::Exile)))
            .filter(|(_, r)| *r != Role::Exile)
            .collect();
        v.sort_by_key(|(f, r)| (*r as u8, *f));
        v
    }
}

fn town_name(h: &WorldHistory, s: SettlementId) -> String { h.settlements.get(&s).map(|t| t.name.clone()).unwrap_or_default() }

/// Once a season: homes for the homeless, flight from fallen towns. Once a year: notables.
pub fn step(h: &mut WorldHistory, game_data: &crate::history::data::GameData, rng: &mut impl Rng) {
    let mut people = h.people.take().unwrap_or_default();
    let date = h.current_date;

    // Flight: a notable whose town burned or fell to another people leaves it.
    let mut fleeing: Vec<(FigureId, SettlementId)> = people.home.iter()
        .filter(|(f, _)| h.figures.get(f).map_or(false, |x| x.is_alive()))
        .filter(|(f, s)| {
            let fig_people = h.figures.get(f).and_then(|x| x.faction);
            h.settlements.get(s).map_or(true, |t| t.is_destroyed() || Some(t.faction) != fig_people)
        })
        .map(|(f, s)| (*f, *s))
        .collect();
    fleeing.sort();
    for (f, old) in fleeing {
        let fall = h.chronicle.last_of(EntityId::Settlement(old));
        let razed = h.settlements.get(&old).map_or(true, |t| t.is_destroyed());
        let role = people.role.get(&f).copied().unwrap_or(Role::Exile);
        let name = h.figures.get(&f).map(|x| x.full_name()).unwrap_or_default();
        let old_name = town_name(h, old);
        let fac = h.figures.get(&f).and_then(|x| x.faction);
        // A captain may die defending the walls.
        if role == Role::Captain && rng.gen::<f32>() < 0.4 {
            if let Some(x) = h.figures.get_mut(&f) { x.kill(date, DeathCause::Battle); }
            people.home.remove(&f);
            record(h, EventType::HeroDied, format!("{} falls at {}", name, old_name),
                format!("{}, captain of {}, died on the walls when the town {}.", name, old_name, if razed { "burned" } else { "fell" }),
                f, fac, Some(old), fall);
            continue;
        }
        // Refuge: the nearest living town of their own people.
        let loc = h.settlements.get(&old).map(|t| t.location).unwrap_or((0, 0));
        let refuge = fac.and_then(|p| h.factions.get(&p)).filter(|p| p.is_active())
            .and_then(|p| p.settlements.iter().filter_map(|s| h.settlements.get(s)).filter(|t| !t.is_destroyed() && t.id != old)
                .min_by_key(|t| (t.location.0.abs_diff(loc.0) + t.location.1.abs_diff(loc.1), t.id)).map(|t| t.id));
        match refuge {
            Some(to) => {
                people.home.insert(f, to);
                if role == Role::Ruler { continue; }
                let to_name = town_name(h, to);
                record(h, EventType::FigureMoved, format!("{} flees to {}", name, to_name),
                    format!("{}, {} of {}, fled {} to {}.", name, role.word(), old_name, if razed { "the burning" } else { "the conquerors" }, to_name),
                    f, fac, Some(to), fall);
            }
            None => {
                people.home.remove(&f);
                people.role.insert(f, Role::Exile);
                record(h, EventType::FigureMoved, format!("{} goes into exile", name),
                    format!("{}, {} of {}, had no town of their own left to flee to and went into exile.", name, role.word(), old_name),
                    f, fac, None, fall);
            }
        }
    }

    // Homes: rulers at their seat; anyone else of a people without a home, its seat.
    let mut figs: Vec<FigureId> = h.figures.values().filter(|x| x.is_alive() && x.faction.is_some()).map(|x| x.id).collect();
    figs.sort();
    for f in figs {
        let Some(fac) = h.figures.get(&f).and_then(|x| x.faction).and_then(|p| h.factions.get(&p)) else { continue };
        if !fac.is_active() { continue; }
        let Some(seat) = fac.capital.or_else(|| fac.settlements.first().copied()) else { continue };
        if fac.current_leader == Some(f) {
            people.home.insert(f, seat);
            people.role.insert(f, Role::Ruler);
        } else if !people.home.contains_key(&f) && people.role.get(&f) != Some(&Role::Exile) {
            people.home.insert(f, seat);
        }
    }

    // Notables, once a year: each town of some size has its captain, priest and smith.
    if date.season == crate::seasons::Season::Spring {
        let mut towns: Vec<SettlementId> = h.settlements.values().filter(|t| !t.is_destroyed()).map(|t| t.id).collect();
        towns.sort();
        for town in towns {
            let (pop, fid, metal) = {
                let t = &h.settlements[&town];
                let metal = t.local_resources.iter().any(|r| matches!(r, crate::history::civilizations::economy::ResourceType::Iron | crate::history::civilizations::economy::ResourceType::Copper));
                (t.population, t.faction, metal)
            };
            let Some(fac) = h.factions.get(&fid).filter(|f| f.is_active()) else { continue };
            let faith = fac.state_religion.is_some();
            let race = fac.race_id;
            let have: Vec<Role> = people.notables_of(h, town).into_iter().map(|(_, r)| r).collect();
            let mut wanted = Vec::new();
            if pop >= 300 && !have.contains(&Role::Captain) { wanted.push(Role::Captain); }
            if pop >= 800 && faith && !have.contains(&Role::Priest) { wanted.push(Role::Priest); }
            if (pop >= 1500 || (metal && pop >= 400)) && !have.contains(&Role::Smith) { wanted.push(Role::Smith); }
            for role in wanted {
                let id = h.id_generators.next_figure();
                let style = crate::history::simulation::step::naming_style_for(h, race, game_data);
                let name = crate::history::naming::generator::NameGenerator::personal_name(&style, rng);
                let born = crate::history::time::Date::new(date.year.saturating_sub(rng.gen_range(20..45)), crate::seasons::Season::Spring);
                let mut fig = Figure::new(id, name, race, born, Personality::random(rng));
                fig.faction = Some(fid);
                fig.titles.push(format!("{} of {}", capital(role.word()), town_name(h, town)));
                h.figures.insert(id, fig);
                if let Some(f) = h.factions.get_mut(&fid) { f.notable_figures.push(id); }
                people.home.insert(id, town);
                people.role.insert(id, role);
            }
        }
    }
    h.people = Some(people);
}

fn record(h: &mut WorldHistory, kind: EventType, title: String, text: String, who: FigureId, fac: Option<FactionId>, place: Option<SettlementId>, cause: Option<EventId>) {
    let id = h.id_generators.next_event();
    let mut e = Event::new(id, kind, h.current_date, title, text).with_participant(EntityId::Figure(who));
    if let Some(f) = fac { e = e.with_faction(f); }
    if let Some(p) = place {
        e = e.with_participant(EntityId::Settlement(p));
        if let Some(t) = h.settlements.get(&p) { e = e.at_location(t.location.0, t.location.1); }
    }
    if let Some(c) = cause { e = e.caused_by(c); }
    h.chronicle.record(e);
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Link every event to the figures who took part in it (`Figure::events`), so a life lists its
/// deeds. Run when a history is finished.
pub fn link_lives(h: &mut WorldHistory) {
    let mut by_fig: HashMap<FigureId, Vec<EventId>> = HashMap::default();
    for e in &h.chronicle.events {
        for p in &e.primary_participants {
            if let EntityId::Figure(f) = p { by_fig.entry(*f).or_default().push(e.id); }
        }
    }
    for (f, evs) in by_fig {
        if let Some(fig) = h.figures.get_mut(&f) {
            for e in evs { if !fig.events.contains(&e) { fig.events.push(e); } }
        }
    }
}

/// One line for the end-of-history report: homes, notables, and how long the lives of note are.
pub fn report(h: &WorldHistory) -> String {
    let empty = People::default();
    let p = h.people.as_ref().unwrap_or(&empty);
    let living: Vec<&Figure> = h.figures.values().filter(|f| f.is_alive() && f.faction.map_or(false, |x| h.factions.get(&x).map_or(false, |y| y.is_active()))).collect();
    let housed = living.iter().filter(|f| p.home.contains_key(&f.id)).count();
    let exiles = p.role.values().filter(|r| **r == Role::Exile).count();
    let towns: Vec<_> = h.settlements.values().filter(|t| !t.is_destroyed()).collect();
    let with_notables = towns.iter().filter(|t| !p.notables_of(h, t.id).is_empty()).count();
    // The journal's lives of note: rulers, slayers and titled figures.
    let ruled: crate::history::det::HashSet<FigureId> = h.chronicle.events.iter()
        .filter(|e| e.event_type == EventType::RulerCrowned)
        .flat_map(|e| e.primary_participants.iter().filter_map(|x| if let EntityId::Figure(f) = x { Some(*f) } else { None }))
        .collect();
    let mut lives: Vec<&Figure> = h.figures.values().filter(|f| ruled.contains(&f.id) || !f.kills.is_empty() || !f.titles.is_empty()).collect();
    lives.sort_by_key(|f| (std::cmp::Reverse(f.events.len() + 3 * f.kills.len() + if ruled.contains(&f.id) { 5 } else { 0 }), f.id));
    lives.truncate(220);
    let mean = lives.iter().map(|f| f.events.len()).sum::<usize>() as f32 / lives.len().max(1) as f32;
    let moved = h.chronicle.events.iter().filter(|e| e.event_type == EventType::FigureMoved).count();
    format!("People: {} of {} living figures have a home ({} exiles); {} of {} towns have notables; {} lives of note average {:.1} events; {} flights from fallen towns",
        housed, living.len(), exiles, with_notables, towns.len(), lives.len(), mean, moved)
}
