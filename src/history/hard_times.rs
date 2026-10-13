//! Hard times: towns emptied by plague and disaster, famine where a town outgrows its land, and
//! villages that drift away to a city of their people (card 'More ruins from plague, famine and
//! depopulation'). Each ruin has its cause in the chronicle. Rolls are hashes of the seed, the
//! year and the town (no draws from the history's RNG), so the other systems draw as before.
//!
//! - After a disaster (`step_natural_events`), a village, outpost or town left with under a
//!   half of its people, or under sixty, is given up: "After the plague, the last of X left it"
//!   (`abandon_after`, caused by the disaster).
//! - Famine (`step_year`, each spring): a town at nine tenths of what its land can feed may
//!   starve one year in two hundred, one in twenty-five where game has grown scarce or the woods were
//!   cleared: a fifth to two fifths die (`EventType::Famine`, appended), caused by the town's last
//!   ecology record when there is one; left under sixty, the hungry leave it and it falls to ruin.
//! - Drift: a village under 120 souls, fifty years old, within five tiles of a city or capital of
//!   its own people, empties one year in thirty as its young go to the city.

use super::*;
use crate::history::world_state::WorldHistory;
use crate::history::civilizations::settlement::SettlementType;
use crate::history::events::types::{Event, EventType};
use crate::history::{EventId, FactionId, SettlementId};

/// On unless `PLANET_NO_HARD_TIMES=1` (to compare a world without them).
fn enabled() -> bool { std::env::var("PLANET_NO_HARD_TIMES").is_err() }

fn roll(seed: u64, year: u32, salt: u64, id: u64) -> f32 {
    let mut h = seed ^ (year as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ id.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 31; h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9); h ^= h >> 29;
    (h % 10_000) as f32 / 10_000.0
}

/// Whether a settlement this size may be given up (never a capital or a city).
fn may_empty(t: SettlementType) -> bool { !matches!(t, SettlementType::Capital | SettlementType::City) }

/// Give a settlement up to ruin with its cause: the last families leave.
fn abandon(history: &mut WorldHistory, sid: SettlementId, title: String, text: String, cause: Option<EventId>) {
    let date = history.current_date;
    let Some((fid, name)) = history.settlements.get(&sid).filter(|s| !s.is_destroyed()).map(|s| (s.faction, s.name.clone())) else { return };
    if let Some(f) = history.factions.get_mut(&fid) { f.remove_settlement(sid); }
    let loc = simulation::step::destroy_settlement(history, sid, date);
    let event_id = history.id_generators.next_event();
    let mut event = Event::new(event_id, EventType::SettlementDestroyed, date, title, text)
        .with_faction(fid)
        .with_participant(EntityId::Settlement(sid));
    if let Some(c) = cause { event = event.caused_by(c); }
    if let Some((x, y)) = loc {
        event = event.at_location(x, y);
        history.tile_history.record_event(x, y, event_id);
    }
    history.chronicle.record(event);
    let fname = history.factions.get(&fid).map(|f| f.name.clone()).unwrap_or_default();
    let _ = name;
    simulation::step::dissolve_if_landless(history, fid, &fname, date);
}

/// After a disaster struck `sid` (event `cause`, `before` its people before): a village or town
/// left with under a third of them, or under forty, is given up.
pub(crate) fn abandon_after(history: &mut WorldHistory, sid: SettlementId, before: u32, cause: EventId, what: &str) {
    if !enabled() { return; }
    let Some(s) = history.settlements.get(&sid) else { return };
    if s.is_destroyed() || !may_empty(s.settlement_type) { return; }
    if s.population * 2 >= before && s.population >= 60 { return; }
    let name = s.name.clone();
    abandon(history, sid, format!("{} abandoned", name),
        format!("After the {}, the last of {} took what they could carry and left it to ruin.", what, name), Some(cause));
}

/// Each spring: famine where a town outgrows its land, and villages drifting to the city.
pub(crate) fn step_year(history: &mut WorldHistory, world: &crate::world::WorldData) {
    // (Debug: PLANET_NO_HARD_TIMES=1 leaves them out, to compare.)
    if !enabled() { return; }
    let date = history.current_date;
    let seed = world.seed();
    // The last ecology record of each settlement (game scarce, forest cleared): famine's cause.
    let scarce: crate::history::det::HashMap<SettlementId, EventId> = history.chronicle.events.iter()
        .filter(|e| matches!(e.event_type, EventType::GameScarce | EventType::ForestCleared))
        .filter_map(|e| e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { Some((s.clone(), e.id)) } else { None }))
        .collect();
    // Famine.
    let mut hungry: Vec<(SettlementId, FactionId, String, u32, (usize, usize), Option<EventId>)> = history.settlements.values()
        .filter(|s| !s.is_destroyed() && s.population_cap > 0 && s.population as f32 >= 0.9 * s.population_cap as f32)
        .filter(|s| roll(seed, date.year, 0xFA41, s.id.0 as u64) < if scarce.contains_key(&s.id) { 0.06 } else { 0.01 })
        .map(|s| (s.id, s.faction, s.name.clone(), s.population, s.location, scarce.get(&s.id).copied()))
        .collect();
    hungry.sort_by_key(|h| h.0 .0);
    for (sid, fid, name, pop, loc, cause) in hungry {
        let share = 0.2 + 0.2 * roll(seed, date.year, 0xFA42, sid.0 as u64);
        let dead = (pop as f32 * share) as u32;
        if let Some(s) = history.settlements.get_mut(&sid) { s.population = s.population.saturating_sub(dead); }
        if let Some(f) = history.factions.get_mut(&fid) { f.total_population = f.total_population.saturating_sub(dead); }
        let event_id = history.id_generators.next_event();
        let why = if cause.is_some() { "the land round it gave less than it had, and" } else { "it had outgrown what its land could feed, and" };
        let mut event = Event::new(event_id, EventType::Famine, date, format!("Famine in {}", name),
            format!("Hunger came to {}: {} a bad harvest left {} dead.", name, why, dead))
            .at_location(loc.0, loc.1)
            .with_faction(fid)
            .with_participant(EntityId::Settlement(sid));
        if let Some(c) = cause { event = event.caused_by(c); }
        history.chronicle.record(event);
        history.tile_history.record_event(loc.0, loc.1, event_id);
        let small = history.settlements.get(&sid).map_or(false, |s| s.population < 80 && may_empty(s.settlement_type));
        if small {
            abandon(history, sid, format!("{} abandoned", name), format!("The hungry left {}, and it fell to ruin.", name), Some(event_id));
        }
    }
    // Drift to the city.
    let cities: Vec<(SettlementId, FactionId, (usize, usize), String, EventId)> = history.settlements.values()
        .filter(|s| !s.is_destroyed() && matches!(s.settlement_type, SettlementType::Capital | SettlementType::City))
        .filter_map(|s| {
            let founded = history.chronicle.events.iter().find(|e| e.event_type == EventType::SettlementFounded && e.primary_participants.contains(&EntityId::Settlement(s.id))).map(|e| e.id);
            Some((s.id, s.faction, s.location, s.name.clone(), founded.unwrap_or(EventId(0))))
        })
        .collect();
    let mut drifting: Vec<(SettlementId, String, String, EventId)> = Vec::new();
    for s in history.settlements.values() {
        if s.is_destroyed() || !may_empty(s.settlement_type) || s.population >= 150 || date.year.saturating_sub(s.founded.year) < 40 { continue; }
        let Some(c) = cities.iter().filter(|c| c.1 == s.faction && c.0 != s.id)
            .filter(|c| (c.2 .0 as i64 - s.location.0 as i64).abs().max((c.2 .1 as i64 - s.location.1 as i64).abs()) <= 5)
            .min_by_key(|c| ((c.2 .0 as i64 - s.location.0 as i64).abs().max((c.2 .1 as i64 - s.location.1 as i64).abs()), c.0 .0)) else { continue };
        if roll(seed, date.year, 0xD21F, s.id.0 as u64) >= 1.0 / 12.0 { continue; }
        drifting.push((s.id, s.name.clone(), c.3.clone(), c.4));
    }
    drifting.sort_by_key(|d| d.0 .0);
    for (sid, name, city, cause) in drifting {
        abandon(history, sid, format!("{} abandoned", name),
            format!("The young of {} went to {} one by one, and the last families followed; it fell to ruin.", name, city),
            if cause.0 == 0 { None } else { Some(cause) });
    }
}
