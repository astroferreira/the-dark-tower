//! Ask anyone about anything their town knows (Tibia's keywords, DF's per-entity knowledge):
//! a name typed in talk is looked up among the history's beasts, people, towns, peoples and
//! treasures; what the teller can say is what their town has heard (`history::knowledge`; a
//! sage knows what their whole people knows), told their people's way (proud, bitter, plain).
//! "Where" is answered with a direction and a distance and marks the map. Not everyone is
//! reliable: the tavern's drunk mixes things up (the wrong direction, the dead numbers grown).

use super::actor::{Npc, Role};
use super::game::Game;
use crate::history::events::types::Event;
use crate::history::knowledge::{account, fame, Fame, Knowledge};
use crate::history::world_state::WorldHistory;
use crate::history::{ArtifactId, EntityId, FactionId, FigureId, LegendaryCreatureId, SettlementId};

/// What a name names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Subject { Beast(LegendaryCreatureId), Figure(FigureId), Town(SettlementId), People(FactionId), Treasure(ArtifactId) }

impl Subject {
    fn entity(self) -> EntityId {
        match self { Subject::Beast(c) => EntityId::LegendaryCreature(c), Subject::Figure(f) => EntityId::Figure(f), Subject::Town(s) => EntityId::Settlement(s), Subject::People(f) => EntityId::Faction(f), Subject::Treasure(a) => EntityId::Artifact(a) }
    }
}

/// The subject a name names, and its full name: an exact name first, then one it begins, then
/// one it is in; among equals the most told of.
pub fn find(h: &WorldHistory, q: &str) -> Option<(Subject, String)> {
    let q = q.trim().to_lowercase();
    if q.len() < 3 { return None; }
    let mut cands: Vec<(Subject, String)> = Vec::new();
    for (id, c) in &h.legendary_creatures { cands.push((Subject::Beast(*id), c.full_name())); }
    for (id, f) in &h.figures { cands.push((Subject::Figure(*id), f.full_name())); }
    for (id, s) in &h.settlements { cands.push((Subject::Town(*id), s.name.clone())); }
    for (id, f) in &h.factions { cands.push((Subject::People(*id), f.name.clone())); }
    for (id, a) in &h.artifacts { cands.push((Subject::Treasure(*id), a.name.clone())); }
    let rank = |n: &str| { let l = n.to_lowercase(); let l = l.trim_start_matches("the "); if l == q || n.to_lowercase() == q { 0 } else if l.starts_with(&q) { 1 } else if l.split(|c: char| !c.is_alphanumeric()).any(|w| w == q) { 2 } else if l.contains(&q) { 3 } else { 9 } };
    let told = |s: &Subject| h.chronicle.last_of(s.entity()).map_or(0, |e| e.0);
    cands.into_iter().filter(|(_, n)| rank(n) < 9).min_by_key(|(s, n)| (rank(n), std::cmp::Reverse(told(s)), n.clone()))
}

/// Where a subject is (a world tile), if it is anywhere.
pub fn whereabouts(h: &WorldHistory, s: Subject) -> Option<(usize, usize)> {
    match s {
        Subject::Beast(c) => h.legendary_creatures.get(&c).filter(|c| c.death_date.is_none()).and_then(|c| c.lair_location),
        Subject::Town(t) => h.settlements.get(&t).map(|t| t.location),
        Subject::People(f) => h.factions.get(&f).and_then(|f| f.capital).and_then(|c| h.settlements.get(&c)).map(|s| s.location),
        Subject::Figure(f) => h.people.as_ref().and_then(|p| p.home.get(&f)).and_then(|s| h.settlements.get(s)).map(|s| s.location)
            .or_else(|| h.figures.get(&f).filter(|x| x.is_alive()).and_then(|x| x.faction).and_then(|fa| h.factions.get(&fa)).and_then(|fa| fa.capital).and_then(|c| h.settlements.get(&c)).map(|s| s.location)),
        Subject::Treasure(a) => h.artifacts.get(&a).filter(|a| !a.destroyed).and_then(|a| a.current_location),
    }
}

/// The events about a subject, newest first.
fn about<'a>(h: &'a WorldHistory, s: Subject) -> Vec<&'a Event> {
    let e = s.entity();
    let at = if let Subject::Town(t) = s { h.settlements.get(&t).map(|t| t.location) } else { None };
    h.chronicle.events.iter().rev().filter(|ev| ev.primary_participants.contains(&e) || (at.is_some() && ev.location == at) || matches!(s, Subject::People(f) if ev.factions_involved.contains(&f))).take(400).collect()
}

/// Whether a teller tells things wrong now and then (the tavern's drunk).
fn unreliable(n: &Npc) -> bool { n.of == "drunk" }

/// Answer `n` (of town `town`, a settlement of the history) asked about `q`; marks the map when
/// they say where. Returns what they say.
pub fn ask(g: &mut Game, n: &Npc, town: Option<SettlementId>, q: &str) -> String {
    let Some(h) = g.history.clone() else { return "I don't follow you.".into() };
    let Some((subject, name)) = find(&h, q) else { return format!("\"{}\"? Never heard the name.", q.trim()) };
    let k = Knowledge::new(&h);
    let teller = town.and_then(|t| h.settlements.get(&t)).map(|t| t.faction);
    // What they know: their town's news; a sage, their people's; anyone, their own town.
    let knows = |e: &Event| match (n.role, town, teller) {
        (Role::Sage, _, Some(f)) => k.people_knows(f, e),
        (_, Some(t), _) => k.town_knows(t, e),
        _ => fame(e) >= Fame::Great,
    };
    let known: Vec<&Event> = about(&h, subject).into_iter().filter(|e| knows(e)).collect();
    let mut parts: Vec<String> = Vec::new();
    // The best of what they know: the most famous, then the newest.
    let mut best = known.clone();
    best.sort_by_key(|e| (std::cmp::Reverse(fame(e)), std::cmp::Reverse((e.date, e.id))));
    let wrong = unreliable(n);
    for e in best.iter().take(if n.role == Role::Sage { 3 } else { 2 }) {
        let mut line = account(&h, e, teller).line();
        if wrong { line = garble(&line, e.id.0); }
        parts.push(line);
    }
    let lead = match subject {
        Subject::Beast(_) => format!("{}? ", name),
        Subject::Figure(_) => format!("{}? ", name),
        Subject::Town(_) => format!("{}? ", name),
        Subject::People(_) => format!("{}? ", name),
        Subject::Treasure(_) => format!("{}? ", name),
    };
    let mut said = if parts.is_empty() {
        if n.role == Role::Sage { format!("{}The name is in the old books, but nothing of late has come to us.", lead) }
        else { format!("{}I have heard the name, no more. Ask the sage.", lead) }
    } else {
        let t = if parts.len() == 1 { parts[0].clone() } else { format!("{}; and {}", parts[0], parts[1..].join("; and ")) };
        format!("{}They say: {}.", lead, t)
    };
    // Where it is, if they know (it took part in what they know, or it is near them).
    if let Some(at) = whereabouts(&h, subject) {
        let here = g.place().map(|p| p.spec.tile).unwrap_or(g.tile);
        let w = g.world.w;
        let d = super::world::dist(here, at, w);
        let near = d <= 6;
        if !known.is_empty() || near || n.role == Role::Sage {
            let dir = super::quest::direction(here, at, w);
            let dir = if wrong { ["north", "east", "south", "west", "north-east", "south-west"][(crate::persona::seed_of(&name, 3) % 6) as usize] } else { dir };
            let what = match subject { Subject::Beast(_) => "Its lair is", Subject::Town(_) | Subject::People(_) => "That is", Subject::Figure(_) => "They were last heard of", Subject::Treasure(_) => "It lies, they say," };
            said.push_str(&format!(" {} {} days' walk to the {}{}.", what, d.max(1), dir, if d == 0 { ", right here" } else { "" }));
            if !wrong {
                g.rumour(at);
                if let Some(id) = g.sites.iter().filter(|s| s.tile == at).map(|s| s.id).next() { if !g.known.contains(&id) { g.known.push(id); } }
                said.push_str(" (It is on your map now.)");
            }
        }
    }
    said
}

/// A drunk's telling: numbers grown, names muddled.
fn garble(line: &str, salt: u64) -> String {
    let mut out = String::new();
    for w in line.split(' ') {
        let digits: String = w.chars().filter(|c| c.is_ascii_digit()).collect();
        // (Numbers of the dead and the fighting grow; years, in brackets, stay.)
        if !digits.is_empty() && !w.starts_with('(') && digits.len() == w.trim_matches(|c: char| !c.is_ascii_digit()).len() && w.len() <= 6 {
            if let Ok(v) = digits.parse::<u64>() { out.push_str(&w.replace(&digits, &((v * (3 + salt % 4)).to_string()))); out.push(' '); continue; }
        }
        out.push_str(w);
        out.push(' ');
    }
    let tail = ["or so I heard, hic", "I was there, I swear it", "my cousin saw it all", "and that's the truth of it"][(salt % 4) as usize];
    let out = out.trim_end().replace(" raids ", " burned half of ").replace(" beat ", " slaughtered ").replace(" fell", " fell, every last one");
    format!("{}, {}", out, tail)
}

/// Things the teller's town knows of worth asking about: the beasts, people and treasures in
/// its news and near it (for the talk menu).
pub fn subjects(g: &Game, town: Option<SettlementId>, n: usize) -> Vec<String> {
    let Some(h) = g.history.clone() else { return Vec::new() };
    let k = Knowledge::new(&h);
    let Some(t) = town else { return Vec::new() };
    let mut names: Vec<String> = Vec::new();
    for e in h.chronicle.events.iter().rev().take(3000) {
        if names.len() >= n { break; }
        if fame(e) < Fame::Notable || !k.town_knows(t, e) { continue; }
        for p in &e.primary_participants {
            let name = match p {
                EntityId::LegendaryCreature(c) => h.legendary_creatures.get(c).filter(|c| c.death_date.is_none()).map(|c| c.name.clone()),
                EntityId::Artifact(a) => h.artifacts.get(a).map(|a| a.name.clone()),
                EntityId::Figure(f) => h.figures.get(f).filter(|f| f.is_alive() && Some(f.id.0) != g.hero_figure).map(|f| f.name.clone()),
                _ => None,
            };
            if let Some(nm) = name { if !names.contains(&nm) && names.len() < n { names.push(nm); } }
        }
    }
    names
}
