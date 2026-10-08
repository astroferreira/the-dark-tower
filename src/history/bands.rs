//! Outlaw bands: the exiled gather in the hills and live by raiding.
//!
//! The idea is Dwarf Fortress's wandering groups and bandit camps, which split off with a leader
//! and a home. Here they are read from the present day rather than simulated: every living
//! exile (`people.rs`: a notable whose town fell with nowhere of their people left to flee) is
//! placed where they were driven out; exiles within a few tiles of each other gather into one
//! band, led by the most violent and daring of them (by persona), with a hideout in the hills
//! near the town they lost. Each band names its cause, the fall that drove its leader out. The
//! colony's "outlaws" are such a band when one is near (`colony::arc`).
//!
//! Pure: nothing saved, no RNG.

use crate::history::*;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;

#[derive(Clone, Debug)]
pub struct Band {
    /// "the Exiles of Brolmdustoor".
    pub name: String,
    pub leader: FigureId,
    pub members: Vec<FigureId>,
    /// The people they were of.
    pub people: Option<FactionId>,
    /// Where they hide (a world tile near the town they lost).
    pub hideout: (usize, usize),
    /// The town they lost and the event that drove the leader out.
    pub town: String,
    pub cause: Option<EventId>,
    pub year: u32,
}

impl Band {
    /// "the Exiles of Brolmdustoor, led by Gagraarm".
    pub fn title(&self, h: &WorldHistory) -> String {
        format!("{}, led by {}", self.name, h.figures.get(&self.leader).map(|f| f.full_name()).unwrap_or_default())
    }
}

fn hash(a: u64, b: u64) -> u64 {
    let mut x = a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

/// The bands of the present day.
pub fn of(h: &WorldHistory) -> Vec<Band> {
    let Some(people) = h.people.as_ref() else { return Vec::new() };
    let w = h.tile_history.width.max(1);
    // Each living exile: where they were driven out (the fall behind their exile), and when.
    let mut exiles: Vec<(FigureId, (usize, usize), String, Option<EventId>, u32)> = Vec::new();
    let mut ids: Vec<FigureId> = people.role.iter().filter(|(_, r)| **r == crate::history::people::Role::Exile).map(|(f, _)| *f).collect();
    ids.sort();
    for f in ids {
        let Some(fig) = h.figures.get(&f).filter(|x| x.is_alive()) else { continue };
        let exile = h.chronicle.events.iter().rev().find(|e| e.event_type == EventType::FigureMoved && e.title == format!("{} goes into exile", fig.name)
            && e.primary_participants.contains(&EntityId::Figure(f)));
        let fall = exile.and_then(|e| e.causes.first()).and_then(|c| h.chronicle.get(*c));
        let town = fall.and_then(|e| e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { h.settlements.get(s) } else { None }));
        let Some(t) = town else { continue };
        exiles.push((f, t.location, t.name.clone(), exile.map(|e| e.id), exile.map_or(0, |e| e.date.year)));
    }
    // Gather those driven out within four tiles of each other.
    let mut bands: Vec<Band> = Vec::new();
    for (f, at, town, cause, year) in exiles {
        let near = |b: &Band| { let dx = b.hideout.0.abs_diff(at.0); dx.min(w.saturating_sub(dx)) + b.hideout.1.abs_diff(at.1) <= 4 };
        if let Some(b) = bands.iter_mut().find(|b| near(b)) { b.members.push(f); continue; }
        let fig = &h.figures[&f];
        // A hideout in the hills a tile or two from the lost town.
        let k = hash(f.0, 0xB4D);
        let (dx, dy) = ((k % 5) as i64 - 2, ((k >> 8) % 5) as i64 - 2);
        let hideout = ((at.0 as i64 + dx).rem_euclid(w as i64) as usize, (at.1 as i64 + dy).clamp(0, h.tile_history.height.max(1) as i64 - 1) as usize);
        bands.push(Band { name: format!("the Exiles of {}", town), leader: f, members: vec![f], people: fig.faction, hideout, town, cause, year });
    }
    // The leader: the most violent and daring of them.
    for b in bands.iter_mut() {
        let grit = |f: &FigureId| h.figures.get(f).map(|x| { let p = crate::persona::Persona::of_figure(h, x); p.facet(crate::persona::Facet::Violence) as u32 + p.facet(crate::persona::Facet::Bravery) as u32 + p.facet(crate::persona::Facet::Ambition) as u32 }).unwrap_or(0);
        if let Some(&l) = b.members.iter().max_by_key(|f| (grit(f), std::cmp::Reverse(**f))) { b.leader = l; }
    }
    bands
}

/// One line per band, for `--present`.
pub fn report(h: &WorldHistory) -> String {
    let b = of(h);
    if b.is_empty() { return "Outlaw bands: none".into(); }
    let parts: Vec<String> = b.iter().map(|x| format!("{} ({} exiles, hideout at {},{}, since {})", x.title(h), x.members.len(), x.hideout.0, x.hideout.1, x.year)).collect();
    format!("Outlaw bands: {}", parts.join("; "))
}
