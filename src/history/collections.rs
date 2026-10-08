//! Where an event belongs: its age, its war, its battle.
//!
//! Dwarf Fortress files every event under the innermost open collection (a death under its
//! battle, the battle under its war). Here the same nesting is read back from what the
//! chronicle already records, so nothing new is saved: a war holds its declaration, its battles
//! and sieges (`War::battles`, `War::sieges`) and anything caused by them within its years; a
//! battle holds the deaths and flights its causes point back to. The world's ages (`ages.rs`)
//! wrap them all.

use crate::history::*;
use crate::history::events::types::Event;
use crate::history::world_state::WorldHistory;

/// The collections an event belongs to, outermost first: (what, the event that opens it, if
/// any). An age, then a war, then a battle or siege.
pub fn context(h: &WorldHistory, e: &Event) -> Vec<(String, Option<EventId>)> {
    let mut out: Vec<(String, Option<EventId>)> = Vec::new();
    if let Some(age) = h.timeline.eras.iter().find(|a| a.contains(&e.date)) { out.push((age.name.clone(), age.defining_events.first().copied())); }
    // The battle or siege it happened in: itself, or what its causes point to (two links back).
    let is_fight = |id: EventId| h.wars.values().any(|w| w.battles.contains(&id) || w.sieges.contains(&id));
    let mut fight: Option<EventId> = None;
    let mut frontier: Vec<EventId> = vec![e.id];
    for _ in 0..3 {
        if let Some(f) = frontier.iter().copied().find(|id| is_fight(*id)) { fight = Some(f); break; }
        frontier = frontier.iter().filter_map(|id| h.chronicle.get(*id)).flat_map(|x| x.causes.iter().copied()).collect();
        if frontier.is_empty() { break; }
    }
    // The war: holds the fight, or the event itself (its declaration), or the event's causes
    // reach the declaration within its years.
    let war = h.wars.values().filter(|w| e.date.year >= w.started.year && w.ended.map_or(true, |d| e.date.year <= d.year)).find(|w| {
        fight.map_or(false, |f| w.battles.contains(&f) || w.sieges.contains(&f)) || w.declaration_event == Some(e.id)
            || w.battles.contains(&e.id) || w.sieges.contains(&e.id)
            || w.declaration_event.map_or(false, |d| e.causes.contains(&d))
    });
    if let Some(w) = war { out.push((w.name.clone(), w.declaration_event)); }
    if let Some(f) = fight.filter(|f| *f != e.id) {
        if let Some(fe) = h.chronicle.get(f) { out.push((fe.title.clone(), Some(f))); }
    }
    out
}

/// The events a war holds, in order: its declaration, its battles and sieges, the deaths and
/// flights they caused, and its end.
pub fn war_events(h: &WorldHistory, w: &crate::history::civilizations::military::War) -> Vec<EventId> {
    let mut v: Vec<EventId> = w.declaration_event.into_iter().chain(w.battles.iter().copied()).chain(w.sieges.iter().copied()).collect();
    let fights: Vec<EventId> = v.clone();
    for e in &h.chronicle.events {
        if e.date.year < w.started.year || w.ended.map_or(false, |d| e.date.year > d.year) { continue; }
        if e.causes.iter().any(|c| fights.contains(c)) && !v.contains(&e.id) { v.push(e.id); }
    }
    v.sort_by_key(|id| h.chronicle.get(*id).map(|e| (e.date.year, e.id.0)).unwrap_or((u32::MAX, id.0)));
    v
}
