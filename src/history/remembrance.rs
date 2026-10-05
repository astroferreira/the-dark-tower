//! What a people remembers, for the things it makes: artifacts and monuments are made for a real
//! deed (a battle, a crowning, a beast slain, a town founded) by a real person, and say so.
//!
//! The stock path named artifacts from a list (seed 42 had five called Ashborne), left their
//! descriptions empty, and every monument "built a memorial at their capital", remembering
//! nothing. Here an object's name comes from its maker, its town or its deed, its description
//! cites them, and a monument's `commemorates` is the event it was raised for.

use crate::history::*;
use crate::history::events::types::{Event, EventType};
use crate::history::objects::artifacts::ArtifactType;
use crate::history::world_state::WorldHistory;

/// A deed worth remembering.
#[derive(Clone, Debug)]
pub struct Memory {
    pub event: EventId,
    pub year: u32,
    /// Running text: "the Battle of Ripu Ford", "the crowning of Sku the Butcher".
    pub phrase: String,
    /// What it is named after: "Ripu Ford", "Sku the Butcher".
    pub subject: String,
    /// Who and what took part (for links and `honors`).
    pub who: Vec<EntityId>,
}

fn figure_name(h: &WorldHistory, e: &Event) -> Option<String> {
    e.primary_participants.iter().find_map(|p| match p { EntityId::Figure(f) => h.figures.get(f).map(|x| x.full_name()), _ => None })
}
fn town_name(h: &WorldHistory, e: &Event) -> Option<String> {
    e.primary_participants.iter().find_map(|p| match p { EntityId::Settlement(s) => h.settlements.get(s).map(|x| x.name.clone()), _ => None })
}
fn beast_name(h: &WorldHistory, e: &Event) -> Option<String> {
    e.primary_participants.iter().find_map(|p| match p { EntityId::LegendaryCreature(c) => h.legendary_creatures.get(c).map(|x| x.full_name()), _ => None })
}

/// "The Battle of Ripu Ford" -> "the Battle of Ripu Ford"; other titles unchanged.
fn running(title: &str) -> String {
    match title.strip_prefix("The ") { Some(rest) => format!("the {}", rest), None => title.to_string() }
}

/// How an event is remembered, if it is one a people would make something for.
pub fn memory_of(h: &WorldHistory, e: &Event, fid: FactionId) -> Option<Memory> {
    let (phrase, subject) = match e.event_type {
        EventType::BattleFought => {
            let place = e.title.rsplit(" of ").next().unwrap_or(&e.title).trim_start_matches("the ").to_string();
            (running(&e.title), place)
        }
        EventType::WarEnded => {
            let war = e.title.strip_prefix("End of ").unwrap_or(&e.title);
            if e.description.contains(&format!("{} prevailed", h.factions.get(&fid)?.name)) {
                (format!("the victory in {}", running(war)), running(war).trim_start_matches("the ").to_string())
            } else { return None }
        }
        EventType::CreatureSlain => { let b = beast_name(h, e)?; (format!("the slaying of {}", b), b) }
        EventType::SettlementFounded => { let t = town_name(h, e)?; (format!("the founding of {}", t), t) }
        EventType::RulerCrowned => { let f = figure_name(h, e)?; (format!("the crowning of {}", f), f) }
        EventType::HeroDied => { let f = figure_name(h, e)?; (format!("the death of {}", f), f) }
        EventType::ShadowRepelled => { let t = town_name(h, e)?; (format!("the day {} held against the Shadow", t), t) }
        EventType::ShadowLiberated => { let t = town_name(h, e)?; (format!("the freeing of {}", t), t) }
        EventType::ShadowAlliance => ("the Last Alliance".to_string(), "the Last Alliance".to_string()),
        EventType::SiegeEnded => { let t = town_name(h, e)?; (format!("the siege of {}", t), t) }
        EventType::QuestCompleted => { let f = figure_name(h, e)?; (format!("the quest of {}", f), f) }
        EventType::TreatySigned | EventType::AllianceFormed => {
            let other = e.factions_involved.iter().find(|&&f| f != fid).and_then(|f| h.factions.get(f))?;
            let word = crate::history::simulation::setup::people_word(&other.name);
            (format!("the {} with {}", if e.event_type == EventType::TreatySigned { "treaty" } else { "alliance" }, other.name), word)
        }
        _ => return None,
    };
    Some(Memory { event: e.id, year: e.date.year, phrase, subject, who: e.primary_participants.clone() })
}

/// The deed a people would remember now: its most recent memorable event of the last `years`.
pub fn recent_deed(h: &WorldHistory, fid: FactionId, years: u32) -> Option<Memory> {
    let since = h.current_date.year.saturating_sub(years);
    h.chronicle.events.iter().rev()
        .take_while(|e| e.date.year >= since || e.date.year > h.current_date.year)
        .filter(|e| e.date.year >= since && e.factions_involved.contains(&fid))
        .find_map(|e| memory_of(h, e, fid))
}

/// Who makes a people's treasures: a smith of theirs (the one at the seat first), else the ruler;
/// and the town it is made in.
pub fn maker(h: &WorldHistory, fid: FactionId) -> (Option<FigureId>, Option<SettlementId>) {
    use crate::history::people::Role;
    let capital = h.factions.get(&fid).and_then(|f| f.capital);
    if let Some(p) = h.people.as_ref() {
        let mut smiths: Vec<(bool, FigureId, SettlementId)> = p.home.iter()
            .filter(|(f, _)| p.role.get(f) == Some(&Role::Smith))
            .filter(|(f, _)| h.figures.get(f).map_or(false, |x| x.is_alive() && x.faction == Some(fid)))
            .map(|(f, t)| (Some(*t) != capital, *f, *t))
            .collect();
        smiths.sort();
        if let Some(&(_, f, t)) = smiths.first() { return (Some(f), Some(t)); }
    }
    let ruler = h.factions.get(&fid).and_then(|f| f.current_leader).filter(|l| h.figures.get(l).map_or(false, |x| x.is_alive()));
    (ruler, capital)
}

/// The word for a kind of treasure.
pub fn item_word(t: ArtifactType, salt: u64) -> &'static str {
    let pick = |v: &[&'static str]| v[(salt as usize) % v.len()];
    match t {
        ArtifactType::Weapon => pick(&["Sword", "Axe", "Spear", "Bow"]),
        ArtifactType::Armor => pick(&["Mail", "Shield", "Helm"]),
        ArtifactType::Crown => "Crown",
        ArtifactType::Ring => "Ring",
        ArtifactType::Amulet => pick(&["Amulet", "Torc"]),
        ArtifactType::Staff => "Staff",
        ArtifactType::Book => pick(&["Book", "Codex"]),
        ArtifactType::Goblet => pick(&["Cup", "Chalice"]),
        ArtifactType::Instrument => pick(&["Horn", "Harp", "Drum"]),
        ArtifactType::Relic => pick(&["Reliquary", "Stele"]),
    }
}

/// A treasure's name from its maker, its town or its deed (the first not already borne):
/// "Hroagd's Spear", "the Crown of Ripu", "the Ripu Ford Horn".
pub fn artifact_name(h: &WorldHistory, item: &str, maker: Option<&str>, town: Option<&str>, deed: Option<&Memory>, salt: u64) -> Option<String> {
    let mut forms = Vec::new();
    if let Some(d) = deed {
        // "The Ripu Ford Horn", but "The Book of Khiaelae the Relentless".
        if d.subject.contains(" the ") || d.subject.starts_with("the ") { forms.push(format!("The {} of {}", item, d.subject)); }
        else { forms.push(format!("The {} {}", d.subject, item)); }
    }
    if let Some(m) = maker { forms.push(format!("{}'s {}", m, item)); }
    if let Some(t) = town { forms.push(format!("The {} of {}", item, t)); }
    if forms.is_empty() { return None; }
    let n = forms.len();
    forms.rotate_left(salt as usize % n);
    forms.into_iter().find(|f| !h.artifacts.values().any(|a| a.name == *f))
}

/// "a" or "an".
pub fn article(word: &str) -> &'static str {
    if word.chars().next().map_or(false, |c| "aeiouAEIOU".contains(c)) { "an" } else { "a" }
}
