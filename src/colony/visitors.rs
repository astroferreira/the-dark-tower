//! Visitors: living figures of the history who come to the camp and stay a while.
//!
//! The idea is Dwarf Fortress's visitors (monster hunters, performers, scholars at the tavern)
//! who may petition to stay. Here they are real figures of the world (`plan`, at founding):
//! a hero whose quest is to slay a beast that will threaten the camp (the chronicle's "X hunts
//! Y") comes the day after the rumour of that beast, keeps the watch, drills with the militia
//! and stands first in the fight (`fight.rs` puts guests who hunt first among the defenders);
//! loremasters and silver tongues of peoples not at war with the settlers' come as bards from
//! day 40, one every 45 days or so, and each evening perform their people's works by the fire,
//! teaching one, once, from the second night to the camp's most art-inclined listener (35+) (`Past::arts`, which
//! `evening_arts` then performs "as X taught them"). A guest is a settler with `guest_until`:
//! they eat, sleep and work but hold no office, role or mood, and are not chosen to speak. When
//! their stay ends, one the camp likes (mean opinion 3+) and who likes company or is of the
//! settlers' people asks to stay, and the speaker (or the camp) agrees: a moment, and they are a
//! settler; else they walk on ("moved on" in the annals).

use super::*;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId};
use crate::persona::Facet;

#[derive(Clone, Debug, PartialEq)]
pub enum VisitKind {
    /// Hunts the named beast.
    Hunter { beast: String },
    Bard,
    /// Seeks the relic lost near the camp (`relic.rs`).
    Seeker { relic: String },
    /// Fights for pay (`tavern.rs`).
    Sellsword,
}

#[derive(Clone, Debug)]
pub struct Visitor {
    pub name: String,
    pub kind: VisitKind,
    /// "a monster hunter of The Kingdom of Titankeep", "a loremaster of The Git Clans".
    pub calling: String,
    /// The line of their past that brings them ("set out to slay Baelfang Storm-Caller in 445").
    pub why: String,
    pub cause: Option<EventId>,
    pub past: crate::history::settlers::Past,
    /// Fighting hand (a hero's combat skill).
    pub hand: f32,
    pub came: bool,
    /// Taught a work to the camp on this visit.
    pub taught: bool,
    /// A seeker who left before the relic was found (comes back when it is).
    pub left_once: bool,
}

fn at_war_with(h: &WorldHistory, a: Option<crate::history::FactionId>, b: Option<crate::history::FactionId>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => h.factions.get(&a).and_then(|x| x.relations.get(&b)).map_or(false, |r| r.stance.is_at_war()),
        _ => false,
    }
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> usize {
    let dx = a.0.abs_diff(b.0);
    dx.min(w.saturating_sub(dx)) + a.1.abs_diff(b.1)
}

/// The visitors this camp may see: hunters of the beasts its arc names, and up to three bards.
pub fn plan(h: &WorldHistory, tile: (usize, usize), beasts: &[String], relic: Option<(crate::history::ArtifactId, Option<crate::history::FigureId>)>, home: Option<crate::history::FactionId>, seed: u64) -> Vec<Visitor> {
    use crate::history::entities::traits::Skill;
    let w = h.tile_history.width.max(1);
    let now = h.current_date.year;
    let mut out = Vec::new();
    let past_of = |f: &crate::history::entities::figures::Figure, calling: String, lines: Vec<(String, Option<EventId>)>| {
        let mut p = crate::history::settlers::Past { age: now.saturating_sub(f.birth_date.year).max(16) as u32, people: f.faction, calling, lines, ..Default::default() };
        p.persona = Some(crate::persona::Persona::of_figure(h, f));
        p.arts = crate::history::settlers::arts_of(h, f.faction);
        crate::history::settlers::fill_craft(h, &mut p);
        p
    };
    let people = |f: &crate::history::entities::figures::Figure| f.faction.and_then(|x| h.factions.get(&x)).map(|x| x.name.clone()).unwrap_or_else(|| "no people".into());
    // Hunters: the latest living hero whose quest is one of these beasts.
    for beast in beasts {
        let Some(c) = h.legendary_creatures.values().filter(|c| c.is_alive()).find(|c| &c.full_name() == beast) else { continue };
        let quest = h.chronicle.events.iter().rev().filter(|e| e.event_type == EventType::QuestBegun && e.primary_participants.contains(&EntityId::LegendaryCreature(c.id)))
            .find_map(|e| e.primary_participants.iter().find_map(|p| match p { EntityId::Figure(f) => h.figures.get(f).filter(|f| f.is_alive()), _ => None }).map(|f| (e, f)));
        let Some((e, f)) = quest else { continue };
        if out.iter().any(|v: &Visitor| v.name == f.full_name()) { continue; }
        let combat = f.skills.get(&Skill::Combat).copied().unwrap_or(3) as f32;
        let why = format!("set out to slay {} in {}", beast, e.date.year);
        let calling = format!("a monster hunter of {}", people(f));
        let past = past_of(f, calling.clone(), vec![(format!("Set out to slay {} in {}.", beast, e.date.year), Some(e.id))]);
        out.push(Visitor { name: f.full_name(), kind: VisitKind::Hunter { beast: beast.clone() }, calling, why, cause: Some(e.id), past, hand: (0.2 + 0.05 * combat).min(0.6), came: false, taught: false, left_once: false });
    }
    // The seeker of the lost thing near here: the latest living hero whose quest names it.
    // Else an heir: the last holder's living child or spouse (as Dwarf Fortress's heirs claim).
    if let Some((aid, holder)) = relic {
        let quest = h.chronicle.events.iter().rev().filter(|e| e.event_type == EventType::QuestBegun && e.primary_participants.contains(&EntityId::Artifact(aid)))
            .find_map(|e| e.primary_participants.iter().find_map(|p| match p { EntityId::Figure(f) => h.figures.get(f).filter(|f| f.is_alive()), _ => None }).map(|f| (e, f)));
        let lost = h.chronicle.events.iter().rev().find(|e| e.event_type == EventType::ArtifactLost && e.primary_participants.contains(&EntityId::Artifact(aid)));
        if let Some(a) = h.artifacts.get(&aid) {
            if let Some((e, f)) = quest {
                let why = format!("has sought {} since {}", a.name, e.date.year);
                let calling = format!("a seeker of lost things, of {}", people(f));
                let past = past_of(f, calling.clone(), vec![(format!("Set out to recover {} in {}.", a.name, e.date.year), Some(e.id))]);
                out.push(Visitor { name: f.full_name(), kind: VisitKind::Seeker { relic: a.name.clone() }, calling, why, cause: Some(e.id), past, hand: 0.25, came: false, taught: false, left_once: false });
            } else if let Some(hf) = holder.and_then(|x| h.figures.get(&x)) {
                let mut heirs: Vec<&crate::history::entities::figures::Figure> = hf.children.iter().chain(hf.spouse.iter()).filter_map(|c| h.figures.get(c)).filter(|c| c.is_alive()).collect();
                heirs.sort_by_key(|c| (c.birth_date, c.id));
                if let Some(f) = heirs.first() {
                    let female = crate::persona::Persona::of_figure(h, hf).female;
                    let rel = if hf.spouse == Some(f.id) { if female { "wife" } else { "husband" } } else if female { "mother" } else { "father" };
                    let year = lost.map(|e| e.date.year).unwrap_or(now);
                    let why = format!("has sought {}'s {} since {} died in {}", rel, a.name, hf.name, year);
                    let why = why.replacen(&format!("{}'s ", rel), &format!("their {}'s ", rel), 1).replacen("'s The ", "'s ", 1);
                    let calling = format!("an heir seeking what was lost, of {}", people(f));
                    let past = past_of(f, calling.clone(), lost.map(|e| vec![(format!("Lost {} when {} died ({}).", a.name, hf.name, e.date.year), Some(e.id))]).unwrap_or_default());
                    out.push(Visitor { name: f.full_name(), kind: VisitKind::Seeker { relic: a.name.clone() }, calling, why, cause: lost.map(|e| e.id), past, hand: 0.15, came: false, taught: false, left_once: false });
                }
            }
        }
    }
    // A sellsword: a living warrior (kills, no title) of a people at peace with the settlers',
    // within ten tiles of their towns, who is no beast's hunter and leads no band.
    {
        let outlaws: Vec<crate::history::FigureId> = crate::history::bands::of(h).iter().map(|b| b.leader).collect();
        let mut swords: Vec<(usize, &crate::history::entities::figures::Figure)> = h.figures.values()
            .filter(|f| f.is_alive() && f.faction.is_some() && f.faction != home && !at_war_with(h, f.faction, home) && f.kills.len() >= 2 && f.titles.is_empty() && !outlaws.contains(&f.id))
            .filter(|f| !out.iter().any(|v: &Visitor| v.name == f.full_name()))
            .filter_map(|f| {
                let d = h.settlements.values().filter(|t| Some(t.faction) == f.faction && !t.is_destroyed()).map(|t| dist(t.location, tile, w)).min()?;
                (d <= 10).then_some((d, f))
            }).collect();
        swords.sort_by_key(|(d, f)| (*d, std::cmp::Reverse(f.kills.len()), f.id));
        if let Some((_, f)) = swords.first() {
            let calling = format!("a sellsword of {}", people(f));
            let past = past_of(f, calling.clone(), vec![]);
            out.push(Visitor { name: f.full_name(), kind: VisitKind::Sellsword, calling, why: format!("has killed {} and fights for pay", f.kills.len()), cause: None, past, hand: 0.45, came: false, taught: false, left_once: false });
        }
    }
    // Bards: loremasters and silver tongues within ten tiles' reach of their people's towns,
    // of peoples not at war with the settlers'.
    let at_war = |f: Option<crate::history::FactionId>| match (f, home) {
        (Some(a), Some(b)) => h.factions.get(&a).and_then(|x| x.relations.get(&b)).map_or(false, |r| r.stance.is_at_war()),
        _ => false,
    };
    let mut bards: Vec<(usize, &crate::history::entities::figures::Figure)> = h.figures.values().filter(|f| f.is_alive() && f.faction.is_some() && !at_war(f.faction))
        .filter(|f| f.titles.is_empty())
        .filter_map(|f| {
            let d = h.settlements.values().filter(|t| Some(t.faction) == f.faction && !t.is_destroyed()).map(|t| dist(t.location, tile, w)).min()?;
            (d <= 10).then_some((d, f))
        }).collect();
    // Given to art (as their persona says) and with something to tell; no outlaw's leader.
    let outlaws: Vec<crate::history::FigureId> = crate::history::bands::of(h).iter().map(|b| b.leader).collect();
    bards.retain(|(_, f)| crate::persona::Persona::of_figure(h, f).facet(Facet::ArtInclined) >= 75 && !f.events.is_empty() && !outlaws.contains(&f.id));
    bards.sort_by_key(|(d, f)| (*d, crate::history::settlers::hash_pub(seed, f.id.0 as u64)));
    for (_, f) in bards.into_iter().take(3) {
        let lore = crate::persona::Persona::of_figure(h, f).attr(crate::persona::Attr::Memory) >= 1200.0;
        let calling = format!("{} of {}", if lore { "a loremaster" } else { "a teller of tales" }, people(f));
        let last = f.events.iter().rev().filter_map(|e| h.chronicle.get(*e)).next();
        let lines = last.map(|e| vec![(format!("{} ({}).", e.title, e.date.year), Some(e.id))]).unwrap_or_default();
        let past = past_of(f, calling.clone(), lines);
        if past.arts.is_empty() { continue; }
        out.push(Visitor { name: f.full_name(), kind: VisitKind::Bard, calling, why: "travels the roads with the songs of their people".into(), cause: last.map(|e| e.id), past, hand: 0.1, came: false, taught: false, left_once: false });
    }
    out
}

/// A seeker made up for trying the flow (`PLANET_FORCE_SEEKER=<name>`): of the relic's owners'
/// people, greedy, with no history behind them.
pub fn forced_seeker(name: &str, relic: &str, owners: Option<crate::history::FactionId>, seed: u64) -> Visitor {
    let mut persona = crate::persona::Persona::roll("human", None, crate::persona::seed_of(name, seed));
    persona.facets[Facet::Greed as usize] = 80;
    let calling = "a seeker of lost things".to_string();
    let past = crate::history::settlers::Past { age: 40, people: owners, calling: calling.clone(), persona: Some(persona), ..Default::default() };
    Visitor { name: name.to_string(), kind: VisitKind::Seeker { relic: relic.to_string() }, calling, why: format!("has sought {} for years", relic), cause: None, past, hand: 0.2, came: false, taught: false, left_once: false }
}

impl Colony {
    /// 15:00: a hunter follows the rumour of their beast; a bard comes now and then.
    pub(crate) fn visitors_arrive(&mut self) {
        let day = self.clock.day();
        if self.alive() == 0 || self.departed.is_some() { return; }
        let foretold = self.trouble_foretold();
        let pick = self.visitors.iter().position(|v| !v.came && match &v.kind {
            VisitKind::Hunter { beast } => foretold.as_deref() == Some(beast.as_str()),
            VisitKind::Bard => day >= 40 && day >= self.last_visit + if self.tavern().is_some() { 22 } else { 45 } && crate::history::settlers::hash_pub(self.seed ^ day, 0xBA2D) % 4 == 0,
            // The seeker hears of it: three days after it is found, or from day 25 while it lies lost.
            // (One who left before it was found comes back only once it is.)
            VisitKind::Seeker { .. } => self.relic.as_ref().map_or(false, |r| r.known && r.fate.is_none() && match &r.found { Some((_, d)) => day >= d + 3, None => day >= 25 && !v.left_once }),
            VisitKind::Sellsword => false,
        });
        let Some(k) = pick else { return };
        self.visitors[k].came = true;
        self.last_visit = day;
        let v = self.visitors[k].clone();
        // A hunter stays until a day after the raid; a bard four nights.
        let until = match &v.kind {
            VisitKind::Hunter { .. } => self.arc.as_ref().map_or(day + 10, |a| a.raid_day.max(day) + 2),
            VisitKind::Bard => day + if self.tavern().is_some() { 8 } else { 4 },
            VisitKind::Sellsword => day + 5,
            VisitKind::Seeker { .. } => day + 20,
        } * TICKS_PER_DAY + 9 * 60;
        // A seeker coming back is the same one who left.
        let i = match self.settlers.iter().position(|s| s.name == v.name && s.mind.left) {
            Some(i) => {
                let s = &mut self.settlers[i];
                s.alive = true;
                s.mind.left = false;
                s.pos = self.camp;
                s.path.clear();
                s.job = Job::Idle;
                s.hunger = s.hunger.min(0.6);
                i
            }
            None => { self.add_settler(v.name.clone(), Some(v.past.clone())); self.settlers.len() - 1 }
        };
        self.settlers[i].guest_until = until;
        // One who is here does not lead a band against the camp (`arc::plan`'s leaders).
        let led = format!(", led by {}", v.name);
        if let Some(a) = self.arc.as_mut() {
            for t in a.later.iter_mut().chain(a.reserve.iter_mut()) { if t.name.ends_with(&led) { t.name = t.name.trim_end_matches(&led).to_string(); } }
        }
        self.settlers[i].visitor = Some(v.calling.clone());
        self.settlers[i].drill = v.hand;
        let line = match &v.kind {
            VisitKind::Hunter { beast } => format!("A stranger walks into the camp: {}, {}, who {}. They have heard {} is near, and ask to wait for it here.", v.name, v.calling, v.why, beast),
            VisitKind::Bard => format!("A traveller comes to the {}: {}, {}, who {}, and asks to stay a few nights.", if self.tavern().is_some() { "tavern" } else { "fire" }, v.name, v.calling, v.why),
            VisitKind::Sellsword => String::new(),
            VisitKind::Seeker { relic } if v.left_once => format!("{} comes back to the camp: word has reached {} that {} was found.", v.name, if v.past.persona.as_ref().map_or(false, |p| p.female) { "her" } else { "him" }, relic),
            VisitKind::Seeker { relic } => format!("A stranger comes asking after {}: {}, {}, who {}.", relic, v.name, v.calling, v.why),
        };
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("{} comes", v.name), line, match &v.kind {
            VisitKind::Hunter { beast } => format!("because {} {}, and word of {} near the camp reached them", v.name, v.why, beast),
            VisitKind::Bard => "because word of the camp has spread along the roads".to_string(),
            VisitKind::Sellsword => String::new(),
            VisitKind::Seeker { relic } => format!("because {} {}, and word of it near the camp reached them", v.name, v.why.replacen(&format!(" {}", relic), " it", 1)),
        }, at);
        for j in 0..self.settlers.len() {
            if j != i && self.settlers[j].alive && self.settlers[j].persona.facet(Facet::Gregariousness) >= 60 { self.like(i, j, 2); }
        }
    }

    /// 20:30: a visiting bard performs; on the last night they teach a work.
    pub(crate) fn guests_perform(&mut self) {
        let day = self.clock.day();
        let camp = self.camp;
        let near = |p: Pos| (p.0 as i32 - camp.0 as i32).abs().max((p.1 as i32 - camp.1 as i32).abs()) <= 8;
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.guest_until == 0 || !s.visitor.as_deref().map_or(false, |c| c.starts_with("a loremaster") || c.starts_with("a teller")) { continue; }
            let arts = s.past.as_ref().map(|p| p.arts.clone()).unwrap_or_default();
            if arts.is_empty() { continue; }
            let (form, what, kind) = arts[(crate::history::settlers::hash_pub(day, 0xBA4D + i as u64) % arts.len() as u64) as usize].clone();
            let listeners: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive && self.settlers[j].guest_until == 0 && near(self.settlers[j].pos)).collect();
            if listeners.is_empty() { continue; }
            let name = s.name.clone();
            let verb = match kind { "poem" => "recites", "dance" => "shows them a dance,", _ => "plays" };
            self.note(format!("{} {} {} {}, {}; {} {}.", if self.tavern().is_some() { "In the tavern" } else { "By the fire" }, name, verb, form, what.split(';').next().unwrap_or(&what), listeners.len(), if kind == "dance" { "try it" } else { "listen" }));
            for &j in &listeners { self.feel(j, mind::Feel::Heard { what: form.clone(), own: false }); self.warm(i, j); }
            // From the second night, once: the work is taught to the most art-inclined who does
            // not know it.
            let vk = self.visitors.iter().position(|v| v.name == name);
            let second = self.visitors.get(vk.unwrap_or(usize::MAX)).map_or(false, |v| !v.taught) && day >= self.last_visit + 1;
            if second {
                if let Some(&l) = listeners.iter().filter(|&&j| self.settlers[j].past.as_ref().map_or(false, |p| !p.arts.iter().any(|a| a.0 == form)))
                    .max_by_key(|&&j| (self.settlers[j].persona.facet(Facet::ArtInclined), std::cmp::Reverse(j))) {
                    if self.settlers[l].persona.facet(Facet::ArtInclined) >= 35 {
                        let lname = self.settlers[l].name.clone();
                        if let Some(p) = self.settlers[l].past.as_mut() { p.arts.push((form.clone(), format!("taught by {}", name), kind)); }
                        if let Some(k) = vk { self.visitors[k].taught = true; }
                        self.note(format!("{} teaches {} {}.", name, lname, form));
                    }
                }
                // With a library, a loremaster leaves a written copy of the work (`books.rs`).
                let library = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Library);
                let title = format!("The {} of {}", if kind == "poem" { "Words" } else if kind == "dance" { "Steps" } else { "Music" }, form.trim_start_matches("the "));
                let gave = self.works.iter().any(|w| w.maker == i && w.kind == "book");
                if library && !gave && self.settlers[i].visitor.as_deref().map_or(false, |c| c.starts_with("a loremaster")) && !self.works.iter().any(|w| w.called.as_deref() == Some(title.as_str())) {
                    self.works.push(craft::Work { maker: i, kind: "book".into(), material: "hide".into(), quality: 3, image: None, day, called: Some(title.clone()), traded: false });
                    self.note(format!("{} writes out {} and leaves it in the library.", name, title));
                }
            }
        }
    }

    /// 09:00: guests whose stay is over ask to stay, or walk on.
    pub(crate) fn guests_leave(&mut self) {
        let now = self.clock.tick;
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.guest_until == 0 || s.guest_until > now { continue; }
            // A seeker whose relic has been found stays until they have asked for it.
            let seeker = s.visitor.as_deref().map_or(false, |v| v.starts_with("a seeker") || v.starts_with("an heir"));
            if seeker && self.relic.as_ref().map_or(false, |r| r.found.is_some() && !r.asked && r.fate.is_none()) { continue; }
            let others: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive && self.settlers[j].guest_until == 0).collect();
            let liked = if others.is_empty() { 0.0 } else { others.iter().map(|&j| self.opinion(i, j) as f32).sum::<f32>() / others.len() as f32 };
            let kin = s.past.as_ref().and_then(|p| p.people).is_some()
                && others.iter().any(|&j| self.settlers[j].past.as_ref().and_then(|p| p.people) == s.past.as_ref().and_then(|p| p.people));
            // A hunter whose beast still lives goes after it.
            let hunting = self.visitors.iter().find(|v| v.name == s.name).and_then(|v| match &v.kind { VisitKind::Hunter { beast } if !self.slain.contains(beast) && !self.caged.contains(beast) => Some(beast.clone()), _ => None });
            let seeker = s.visitor.as_deref().map_or(false, |v| v.starts_with("a seeker") || v.starts_with("an heir") || v.starts_with("a sellsword"));
            let wants = hunting.is_none() && !seeker && liked >= 3.0 && (kin || s.persona.facet(Facet::Gregariousness) >= 60) && self.food_stored() >= 3 * self.alive() as u32;
            let name = s.name.clone();
            let calling = s.visitor.clone().unwrap_or_default();
            if wants {
                let by = self.speaker.filter(|&k| self.settlers[k].alive).map(|k| self.settlers[k].name.clone());
                let line = format!("{}, {}, asks to stay, and {} {}.", name, calling, by.clone().unwrap_or_else(|| "the camp".into()), "agrees");
                self.settlers[i].guest_until = 0;
                self.note(line.clone());
                let at = self.settlers[i].pos;
                self.moment(format!("{} stays", name), line, format!("because the camp took to {} and {}", name, if kin { "they are of the same people" } else { "they like company" }), at);
            } else {
                match hunting {
                    Some(b) => self.note(format!("{} takes leave of the camp to follow {}'s trail.", name, b)),
                    None => self.note(format!("{}, {}, takes leave of the camp and walks on.", name, calling)),
                }
                // A seeker who leaves before the relic is found will come back when it is.
                if calling.starts_with("a seeker") || calling.starts_with("an heir") {
                    if self.relic.as_ref().map_or(false, |r| r.found.is_none() && r.fate.is_none()) {
                        if let Some(v) = self.visitors.iter_mut().find(|v| v.name == name) { v.came = false; v.left_once = true; }
                    }
                }
                self.release(i);
                let s = &mut self.settlers[i];
                s.alive = false;
                s.mind.left = true;
            }
        }
    }
}
