//! The first arc: the world reaches the colony.
//!
//! One complete story through every layer, from the history to the camp: on day 3 a trader brings
//! a rumour of the nearest real threat (a living beast whose lair is near, the Shadow's raiders, or
//! the war band of the people who took the nearest fallen town), caused by that threat's last deed
//! in the chronicle; on day 6 two survivors of a real fallen town reach the camp and stay; for the
//! nights between, one settler keeps watch (or no one does, if they are starving); on the night of
//! day 14 the raid comes, and how ready the camp is (the watch kept, the hut standing, the numbers)
//! decides whether someone dies or is saved. Each step is an `ArcEvent` caused by the one before,
//! shown as a banner, written to the log, and told afterwards as a tale (`tale`). Everything is
//! hashed from the seed: the same world code tells the same arc.

use crate::history::*;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;
use super::{Colony, ColonyMark, MarkKind, TICKS_PER_DAY};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatKind { Beast, Shadow, Warband, Outlaws }

#[derive(Clone, Debug)]
pub struct Threat {
    pub kind: ThreatKind,
    /// "Morfang the Terrible", "raiders of the Shadow of Skullfang", "a war band of The Git Clans".
    pub name: String,
    /// Why it is near, in the chronicle's words.
    pub why: String,
    pub cause: Option<EventId>,
}

#[derive(Clone, Debug)]
pub struct ArcEvent {
    pub day: u64,
    pub title: String,
    pub text: String,
    /// Why it happened: the step before, or the world event behind the first.
    pub because: String,
    pub world_cause: Option<EventId>,
}

#[derive(Clone, Debug)]
pub struct Arc {
    pub threat: Threat,
    /// The fallen town the refugees come from: (name, the fall, year).
    pub fallen: Option<(String, EventId, u32)>,
    refugees: Vec<(String, crate::history::settlers::Past)>,
    pub events: Vec<ArcEvent>,
    stage: u8,
    /// Nights someone kept watch before the raid.
    pub watches: u32,
    last_watch_day: u64,
    seed: u64,
}

const RUMOUR_DAY: u64 = 3;
const REFUGEE_DAY: u64 = 6;
const RAID_DAY: u64 = 14;

fn hash(seed: u64, salt: u64) -> u64 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> usize {
    let dx = a.0.abs_diff(b.0);
    dx.min(w.saturating_sub(dx)) + a.1.abs_diff(b.1)
}

/// Plan the arc for a colony at world `tile`: the threat, and the refugees' town.
pub fn plan(h: &WorldHistory, tile: (usize, usize), seed: u64) -> Arc {
    use crate::history::naming::styles::NamingStyle;
    use crate::history::naming::generator::NameGenerator;
    use rand::SeedableRng;
    let w = h.tile_history.width.max(1);
    let now = h.current_date.year;
    let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();
    // The nearest fall in living memory.
    let fall = h.chronicle.events.iter()
        .filter(|e| e.date.year + 60 >= now && matches!(e.event_type, EventType::SiegeEnded | EventType::ShadowConquest | EventType::SettlementDestroyed))
        .filter_map(|e| e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { h.settlements.get(s) } else { None }).map(|t| (dist(t.location, tile, w), e, t)))
        .min_by_key(|(d, e, _)| (*d, e.id));
    // The threat: a beast laired near, else the Shadow if its reach is near, else the takers of
    // the nearest fallen town.
    let beast = h.legendary_creatures.values().filter(|c| c.is_alive())
        .filter_map(|c| c.lair_location.map(|l| (dist(l, tile, w), c)))
        .filter(|(d, _)| *d <= 4)
        .min_by_key(|(d, c)| (*d, c.id));
    let shadow_near = h.shadow.as_ref().filter(|s| !s.is_broken())
        .filter(|s| (0..=6i64).any(|r| { let (x, y) = (tile.0 as i64, tile.1 as i64); [(r, 0), (-r, 0), (0, r), (0, -r)].iter().any(|&(dx, dy)| {
            let yy = (y + dy).clamp(0, s.height as i64 - 1) as usize;
            let xx = (x + dx).rem_euclid(s.width as i64) as usize;
            s.at(xx, yy) >= crate::history::shadow::REACH }) }));
    let threat = if let Some((d, c)) = beast {
        let last = h.chronicle.last_of(EntityId::LegendaryCreature(c.id));
        let km = d as f32 * 40_075.0 / w as f32;
        let ev = last.and_then(|e| h.chronicle.get(e)).map(|e| format!("; its last deed: {} ({})", e.title.replacen("The ", "the ", 1), e.date.year)).unwrap_or_default();
        Threat { kind: ThreatKind::Beast, name: c.full_name(), why: format!("its lair is {} days' walk from here{}", (km / 25.0).ceil().max(1.0), ev), cause: last }
    } else if let Some(s) = shadow_near {
        Threat { kind: ThreatKind::Shadow, name: format!("raiders of {}", s.name), why: format!("{} reaches this far", s.name), cause: Some(s.last_deed) }
    } else if let Some((_, e, t)) = fall.filter(|(_, e, _)| e.factions_involved.len() >= 2) {
        let taker = e.factions_involved[0];
        Threat { kind: ThreatKind::Warband, name: format!("a war band of {}", fname(taker)), why: format!("they took {} in {}", t.name, e.date.year), cause: Some(e.id) }
    } else {
        Threat { kind: ThreatKind::Outlaws, name: "a band of outlaws".into(), why: "the roads are lawless".into(), cause: None }
    };
    // Two refugees from the fallen town, in its people's tongue.
    let mut refugees = Vec::new();
    let fallen = fall.map(|(_, e, t)| (t.name.clone(), e.id, e.date.year));
    if let Some((_, e, t)) = fall {
        let people = e.factions_involved.get(1).copied().or(Some(t.faction));
        let arche = people.and_then(|f| h.factions.get(&f)).and_then(|f| h.races.get(&f.race_id))
            .map(|r| r.base_type.default_naming_archetype()).unwrap_or(crate::history::naming::styles::NamingArchetype::Compound);
        let style = NamingStyle::from_archetype(NamingStyleId(0), arche);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xAE7E);
        for k in 0..2u64 {
            let name = NameGenerator::personal_name(&style, &mut rng);
            let age = 16 + (hash(seed, 40 + k) % 30) as u32;
            let past = crate::history::settlers::Past {
                age, people, calling: format!("a refugee from {}", t.name),
                lines: vec![(format!("Fled {} when it {} in {}, and has walked the roads since.", t.name,
                    if e.title.contains("burned") || e.event_type == EventType::SettlementDestroyed { "burned" } else { "fell" }, e.date.year), Some(e.id))],
                feeling: e.factions_involved.first().copied().filter(|f| Some(*f) != people).map(|f| (format!("hates {}", fname(f)), EntityId::Faction(f))),
            };
            refugees.push((name, past));
        }
    }
    Arc { threat, fallen, refugees, events: Vec::new(), stage: 0, watches: 0, last_watch_day: 0, seed }
}

impl Colony {
    fn arc_event(&mut self, title: String, text: String, world_cause: Option<EventId>) {
        let day = self.clock.day();
        let Some(arc) = self.arc.as_mut() else { return };
        let because = match (arc.events.last(), arc.events.len()) {
            (None, _) => format!("because {}", arc.threat.why),
            (Some(_), 1) => match &arc.fallen { Some((t, _, y)) => format!("because {} fell in {}, and its survivors are still on the roads", t, y), None => "because the danger the trader spoke of is real".into() },
            (Some(_), _) => format!("because {} {} near, as the trader said, and the refugees had been followed", arc.threat.name,
                if arc.threat.kind == ThreatKind::Beast { "was" } else { "were" }),
        };
        arc.events.push(ArcEvent { day, title: title.clone(), text: text.clone(), because, world_cause });
        self.banner = Some((title.clone(), self.clock.tick));
        self.note(format!("{}: {}", title, text));
    }

    /// Advance the arc (called every tick while one is planned).
    pub(crate) fn arc_tick(&mut self) {
        let Some(arc) = self.arc.as_ref() else { return };
        let (day, hour, stage) = (self.clock.day(), self.clock.hour(), arc.stage);
        match stage {
            0 if day >= RUMOUR_DAY && hour >= 10 => {
                let t = arc.threat.clone();
                let what = match t.kind {
                    ThreatKind::Beast => format!("{} is stirring; {}.", t.name, t.why),
                    ThreatKind::Shadow => format!("{}; its raiders have been seen on the roads.", t.why),
                    ThreatKind::Warband => format!("{} is roaming the hills: {}.", t.name, t.why),
                    ThreatKind::Outlaws => "outlaws are robbing travellers on the roads.".to_string(),
                };
                self.arc.as_mut().unwrap().stage = 1;
                self.arc_event("A rumour".into(), format!("A trader passing the camp says {}", what), t.cause);
            }
            1 if day >= REFUGEE_DAY && hour >= 16 => {
                let a = self.arc.as_mut().unwrap();
                a.stage = 2;
                let refugees = std::mem::take(&mut a.refugees);
                let fallen = a.fallen.clone();
                match fallen {
                    Some((town, fall, year)) if !refugees.is_empty() => {
                        let names: Vec<String> = refugees.iter().map(|r| r.0.clone()).collect();
                        for (name, past) in refugees { self.add_settler(name, Some(past)); }
                        self.arc_event("Refugees".into(), format!("{} and {}, survivors of {} (fallen in {}), stumble into camp and are taken in. They say the same danger is coming.",
                            names[0], names[1], town, year), Some(fall));
                    }
                    _ => self.arc_event("Refugees".into(), "No one comes; the roads are empty, which frightens them more.".into(), None),
                }
            }
            2 => {
                // The watch: each night until the raid, one settler stays up (if anyone can).
                if hour == 21 && self.clock.minute() == 0 && day > arc.last_watch_day && day < RAID_DAY {
                    let fit: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].hunger < 0.8).collect();
                    let a = self.arc.as_mut().unwrap();
                    a.last_watch_day = day;
                    if let Some(&i) = fit.get((day as usize) % fit.len().max(1)) {
                        a.watches += 1;
                        let name = self.settlers[i].name.clone();
                        let threat = self.arc.as_ref().unwrap().threat.name.clone();
                        self.watcher = Some(i);
                        self.note(format!("{} keeps watch tonight, for fear of {}.", name, threat));
                    }
                }
                if hour == 6 { self.watcher = None; }
                if day >= RAID_DAY && hour >= 2 && hour < 6 { self.raid(); }
            }
            _ => {}
        }
    }

    fn raid(&mut self) {
        let Some(arc) = self.arc.as_mut() else { return };
        arc.stage = 3;
        let (threat, watches, seed) = (arc.threat.clone(), arc.watches, arc.seed);
        self.watcher = None;
        let hut = self.hut.as_ref().map_or(false, |h| h.done);
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive).collect();
        // How ready they are: the watch kept, a roof to hold, hands to fight.
        let ready = 0.05 * watches as f32 + if hut { 0.25 } else { 0.0 } + 0.03 * alive.len() as f32;
        let readiness = format!(" ({} night{} of watch kept, {}, {} to fight)", watches, if watches == 1 { "" } else { "s" },
            if hut { "the hut standing" } else { "no roof to hold" }, alive.len());
        // How strong the threat is, and the night's luck.
        let strength = match threat.kind { ThreatKind::Beast => 0.8, ThreatKind::Shadow => 0.7, ThreatKind::Warband => 0.6, ThreatKind::Outlaws => 0.4 };
        let danger = strength + 0.6 * (hash(seed, 0xBA1D) % 1000) as f32 / 1000.0;
        let who = match threat.kind {
            ThreatKind::Beast => threat.name.clone(),
            _ => capital(&threat.name),
        };
        let at = self.spot_from_camp(4, -5);
        self.marks.push(ColonyMark { at, kind: MarkKind::Scorch, title: "Scorched ground".into(),
            text: format!("Burned in the raid of day {}, when {} came in the night.", self.clock.day(), who), day: self.clock.day() });
        if alive.is_empty() { return; }
        let victim = alive[(hash(seed, 0x51C7) as usize) % alive.len()];
        let vname = self.settlers[victim].name.clone();
        if danger > ready + 0.3 {
            self.arc_event("The raid".into(), format!("{} came in the night. {} was killed before the others could reach them{}.", who, vname, readiness), None);
            self.bury(victim, &format!("in the raid of {}", who));
        } else if danger > ready {
            let saviour = alive.iter().copied().find(|&i| i != victim).unwrap_or(victim);
            let sname = self.settlers[saviour].name.clone();
            self.arc_event("The raid".into(), format!("{} came in the night. {} was struck down, but {} dragged them back to the fire, and they lived{}.", who, vname, sname, readiness), None);
            let stone_at = self.spot_from_camp(-4, -5);
            self.marks.push(ColonyMark { at: stone_at, kind: MarkKind::Stone, title: format!("The stone of {}", sname),
                text: format!("Raised for {}, who saved {} on the night of the raid, day {}.", sname, vname, self.clock.day()), day: self.clock.day() });
        } else {
            self.arc_event("The raid".into(), format!("{} came in the night, found the watch awake and the camp ready, and went away with nothing{}.", who, readiness), None);
        }
    }

    /// A newcomer joins at the camp.
    pub fn add_settler(&mut self, name: String, past: Option<crate::history::settlers::Past>) {
        let mut s = self.settlers[0].clone();
        s.name = name;
        s.pos = self.camp;
        s.path.clear();
        s.hunger = 0.6;
        s.fatigue = 0.6;
        s.job = super::Job::Idle;
        s.carrying = None;
        s.why = "Just arrived, footsore".into();
        s.alive = true;
        s.stuck = 0;
        s.starving = 0;
        s.past = past;
        self.settlers.push(s);
    }

    /// The arc told afterwards, as a journal entry (HTML).
    pub fn tale(&self, history: Option<&WorldHistory>) -> String {
        let Some(arc) = &self.arc else { return String::new() };
        let name = self.name.clone().unwrap_or_else(|| "the camp".into());
        let mut out = format!("<!doctype html><meta charset=\"utf-8\"><title>The First Trouble of {0}</title>\
<style>body{{background:#efe6d0;color:#382a20;font:18px/1.6 'IM Fell English',Georgia,serif;max-width:44rem;margin:3rem auto;padding:0 1rem}}\
h2{{color:#9a2a1e;font-variant:small-caps;letter-spacing:.05em}}b{{color:#9a2a1e}}.cause{{color:#806a52;font-size:.9em}}</style>\
<article class=\"tale\"><h2>The First Trouble of {0}</h2>", esc(&name));
        for e in &arc.events {
            let cause = e.world_cause.and_then(|c| history.and_then(|h| h.chronicle.get(c)))
                .map(|c| format!(" <span class=\"cause\">(In the chronicle: {}, year {}.)</span>", esc(&c.title), c.date.year)).unwrap_or_default();
            out.push_str(&format!("<p><b>Day {}. {}.</b> {} <i>It happened {}.</i>{}</p>", e.day, esc(&e.title), esc(&e.text), esc(&e.because), cause));
        }
        out.push_str("</article>");
        out
    }
}

fn lower(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default() }
fn capital(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;") }

/// The arc's last step, for a test or a plate: did the raid end in a death, a rescue or a rout?
pub fn ending(arc: &Arc) -> Option<&ArcEvent> { arc.events.iter().rev().find(|e| e.title == "The raid") }

#[allow(dead_code)]
const _DAY: u64 = TICKS_PER_DAY;
