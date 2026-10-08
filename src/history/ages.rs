//! The world names its own ages.
//!
//! The idea is Dwarf Fortress's: an age is named for the state of the world's powers, not cut
//! from the calendar. Each decade of the history is read for what held power in it: beasts that
//! outnumber the peoples (an age of myth, named for its two most murderous monsters), the
//! Shadow taking towns (its age), one people holding most of the towns (its age), wars on every
//! side (named for the longest), beasts falling to heroes (named for the great slayer), or none
//! of these (a long peace, named for the greatest people of it). Decades of one kind and one
//! name run together into an age; ages shorter than 20 years fold into a neighbour. The names
//! are the world's own (`Era`, read by the journal and the present day). No RNG.

use crate::history::*;
use crate::history::events::types::{Event, EventType};
use crate::history::time::{Date, Era};
use crate::history::world_state::WorldHistory;
use crate::seasons::Season;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Myth, Shadow, Empire, Strife, Heroes, Peace }

const STEP: u32 = 10;
const MIN_AGE: u32 = 30;

fn ended_after(d: Option<Date>, y: u32) -> bool { d.map_or(true, |d| d.year > y) }

fn window<'a>(by_year: &'a [(u32, &'a Event)], y0: u32, y1: u32) -> Vec<&'a Event> {
    by_year.iter().filter(|(y, _)| *y >= y0 && *y <= y1).map(|(_, e)| *e).collect()
}

/// Who held each living town at year `y` (the tile's ownership record, else its people): counts
/// by people, and the total.
fn holdings(h: &WorldHistory, y: u32) -> (std::collections::BTreeMap<u64, usize>, usize) {
    let mut held: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    for t in h.settlements.values().filter(|s| s.founded.year <= y && ended_after(s.destroyed, y)) {
        let rec = h.tile_history.get(t.location.0, t.location.1).ownership.iter()
            .filter(|r| r.gained.year <= y && ended_after(r.lost, y)).last().map(|r| r.faction).unwrap_or(t.faction);
        *held.entry(rec.0).or_default() += 1;
    }
    let total = held.values().sum();
    (held, total)
}

/// What held power in the decade from `y`: what its events were mostly about.
fn kind_of(h: &WorldHistory, y: u32, by_year: &[(u32, &Event)]) -> Kind {
    let ev = window(by_year, y, y + STEP - 1);
    let count = |t: EventType| ev.iter().filter(|e| e.event_type == t).count();
    let (raids, battles, sieges, shadow, slain) = (count(EventType::MonsterRaid), count(EventType::BattleFought), count(EventType::SiegeBegun), count(EventType::ShadowConquest), count(EventType::CreatureSlain));
    let war = battles + sieges;
    if shadow >= 2 && h.shadow.is_some() { return Kind::Shadow; }
    let (held, total) = holdings(h, y + STEP / 2);
    if total >= 6 && held.values().max().map_or(false, |&n| n * 10 >= total * 4) { return Kind::Empire; }
    if raids >= 5 && raids >= 2 * war { return Kind::Myth; }
    if slain >= 2 && slain * 3 >= war { return Kind::Heroes; }
    if war >= 4 { return Kind::Strife; }
    Kind::Peace
}

/// The name of an age of `kind` over years `y0..=y1`, from the powers of the whole span.
fn name_of(h: &WorldHistory, kind: Kind, y0: u32, y1: u32, by_year: &[(u32, &Event)]) -> String {
    let ev = window(by_year, y0, y1);
    let fname = |id: u64| h.factions.get(&FactionId(id)).map(|f| f.name.replacen("The ", "the ", 1)).unwrap_or_default();
    match kind {
        Kind::Myth => {
            // The two beasts that raided most in it.
            let mut score: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
            for e in ev.iter().filter(|e| e.event_type == EventType::MonsterRaid) {
                for p in &e.primary_participants { if let EntityId::LegendaryCreature(c) = p { *score.entry(c.0).or_default() += 1; } }
            }
            let mut v: Vec<(u64, usize)> = score.into_iter().collect();
            v.sort_by_key(|(id, n)| (std::cmp::Reverse(*n), *id));
            let names: Vec<String> = v.iter().take(2).filter_map(|(id, _)| h.legendary_creatures.get(&LegendaryCreatureId(*id)).map(|c| c.name.clone())).collect();
            if names.is_empty() { "The Age of Myth".into() } else { format!("The Age of {}", names.join(" and ")) }
        }
        Kind::Shadow => match h.shadow.as_ref() {
            Some(sh) => format!("The Age of {}", sh.name.replacen("The ", "the ", 1)).replacen("The Age of Shadow", "The Age of the Shadow", 1),
            None => "The Dark Age".into(),
        },
        Kind::Empire => {
            let (held, _) = holdings(h, (y0 + y1) / 2);
            held.iter().max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id))).map(|(id, _)| format!("The Age of {}", fname(*id))).unwrap_or_else(|| "The Age of Empire".into())
        }
        Kind::Strife => {
            // The war with most battles in the span.
            let w = h.wars.values().filter(|w| w.started.year <= y1 && ended_after(w.ended, y0))
                .max_by_key(|w| (w.battles.iter().filter(|b| ev.iter().any(|e| e.id == **b)).count(), std::cmp::Reverse(w.id)));
            match w { Some(w) => format!("The Age of {}", w.name.replacen("The ", "the ", 1)), None => "The Age of Strife".into() }
        }
        Kind::Heroes => {
            let mut count: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
            for e in ev.iter().filter(|e| e.event_type == EventType::CreatureSlain) {
                for p in &e.primary_participants { if let EntityId::Figure(f) = p { *count.entry(f.0).or_default() += 1; } }
            }
            count.iter().max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id))).and_then(|(id, _)| h.figures.get(&FigureId(*id)))
                .map(|f| format!("The Age of {}", f.full_name())).unwrap_or_else(|| "The Age of Heroes".into())
        }
        Kind::Peace => {
            let (held, _) = holdings(h, (y0 + y1) / 2);
            held.iter().max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id))).map(|(id, _)| format!("The Peace of {}", fname(*id))).unwrap_or_else(|| "The Quiet Age".into())
        }
    }
}

/// Read the history decade by decade and replace its eras with named ages.
pub fn name_ages(h: &mut WorldHistory) {
    let last = h.current_date.year.max(1);
    let first = h.chronicle.events.iter().map(|e| e.date.year).min().unwrap_or(1).max(1);
    let mut by_year: Vec<(u32, &Event)> = h.chronicle.events.iter().map(|e| (e.date.year, e)).collect();
    by_year.sort_by_key(|(y, e)| (*y, e.id));
    // (kind, start, end)
    let mut ages: Vec<(Kind, u32, u32)> = Vec::new();
    let mut y = first;
    while y <= last {
        let k = kind_of(h, y, &by_year);
        let end = (y + STEP - 1).min(last);
        match ages.last_mut() {
            Some(a) if a.0 == k => a.2 = end,
            _ => ages.push((k, y, end)),
        }
        y += STEP;
    }
    // Short ages fold into the longer neighbour; neighbours of one kind then join.
    loop {
        if ages.len() <= 1 { break; }
        let Some(i) = (0..ages.len()).filter(|&i| ages[i].2 + 1 - ages[i].1 < MIN_AGE).min_by_key(|&i| (ages[i].2 + 1 - ages[i].1, i)) else { break };
        let into = if i == 0 { 1 } else if i + 1 == ages.len() { i - 1 } else if ages[i - 1].2 - ages[i - 1].1 >= ages[i + 1].2 - ages[i + 1].1 { i - 1 } else { i + 1 };
        let gone = ages.remove(i);
        let into = if into > i { into - 1 } else { into };
        ages[into].1 = ages[into].1.min(gone.1);
        ages[into].2 = ages[into].2.max(gone.2);
        let mut k = 1;
        while k < ages.len() {
            if ages[k].0 == ages[k - 1].0 { let g = ages.remove(k); ages[k - 1].2 = g.2; } else { k += 1; }
        }
    }
    // Names from each age's whole span; a name the world used before is its second coming.
    let named: Vec<(String, u32, u32, Vec<EventId>)> = ages.iter().map(|&(k, a, b)| {
        let ev = window(&by_year, a, b).into_iter().filter(|e| e.is_major).map(|e| e.id).take(12).collect();
        (name_of(h, k, a, b, &by_year), a, b, ev)
    }).collect();
    let mut seen: Vec<String> = Vec::new();
    h.timeline.eras.clear();
    for (name, start, end, ev) in named {
        let n = seen.iter().filter(|s| **s == name).count();
        seen.push(name.clone());
        let ord = ["Second", "Third", "Fourth", "Fifth"][n.saturating_sub(1).min(3)];
        let name = if n == 0 { name } else { name.replacen("The Age of", &format!("The {} Age of", ord), 1).replacen("The Peace of", &format!("The {} Peace of", ord), 1) };
        let mut era = Era::new(h.id_generators.next_era(), name, Date::new(start, Season::Spring));
        era.defining_events = ev;
        // The last age is the present one: still open.
        if end < last { era.close(Date::new(end, Season::Winter)); }
        h.timeline.eras.push(era);
    }
}

/// The age the present day belongs to.
pub fn current(h: &WorldHistory) -> Option<&Era> { h.timeline.eras.last() }
