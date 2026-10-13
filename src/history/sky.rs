//! The sky in the chronicle, under another star (`astro`): a restless star's great flares, which
//! burn the harvest under a sky on fire; total eclipses over a people's seat, read as omens
//! against their worst neighbour; comets out of the icy belts; a far companion's close approach,
//! when for a year the night is never dark; a close pair eclipsing each other.
//!
//! Each is drawn from a hash of the world's seed, the year and its kind, never from the history's
//! RNG, and nothing happens under the Sun: the Sun's histories are unchanged event for event.

use crate::astro::{PlanetKind, StarSystem};
use crate::history::events::types::{Consequence, Event, EventType};
use crate::history::EntityId;
use crate::history::world_state::WorldHistory;
use crate::history::SettlementId;
use crate::world::WorldData;

/// A roll in [0, 1) for (seed, year, kind).
fn roll(seed: u64, year: u32, kind: u64) -> f32 {
    let mut z = seed ^ (year as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ kind.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// Chances a year of each kind of sky event, from the system.
pub struct SkyRates {
    pub flare: f32,
    pub eclipse: f32,
    pub comet: f32,
    pub pair_eclipse: f32,
    /// The far companion's period (years) and the year it is closest, when within reach.
    pub approach: Option<(f32, f32)>,
}

pub fn rates(sys: &StarSystem, seed: u64) -> SkyRates {
    let act = sys.host().activity;
    let flare = if act > 30.0 { 0.025 } else if act > 6.0 { 0.01 } else { 0.0 };
    let sun = sys.sun_angular_deg();
    let eclipse = if sys.world.moons.iter().any(|m| m.angular_deg(6371.0) >= 0.95 * sun) { 0.02 } else { 0.0 };
    let comet = if sys.belts.iter().any(|b| b.icy) { 0.012 } else { 0.0 };
    let pair = sys.companion.as_ref().filter(|c| c.circumbinary);
    let pair_eclipse = if pair.is_some() { 0.015 } else { 0.0 };
    let approach = sys.companion.as_ref().filter(|c| !c.circumbinary).map(|c| {
        let years = c.period_days / crate::astro::orbit::EARTH_YEAR_DAYS;
        (years, roll(seed, 0, 77) * years)
    }).filter(|(years, _)| *years < 5000.0);
    SkyRates { flare, eclipse, comet, pair_eclipse, approach }
}

fn living_towns(h: &WorldHistory) -> Vec<SettlementId> {
    let mut ids: Vec<SettlementId> = h.settlements.values().filter(|s| !s.is_destroyed()).map(|s| s.id).collect();
    ids.sort();
    ids
}

fn record(h: &mut WorldHistory, kind_title: String, text: String, at: Option<SettlementId>, major: bool, consequences: Vec<Consequence>) {
    let id = h.id_generators.next_event();
    let mut e = Event::new(id, EventType::Celestial, h.current_date, kind_title, text);
    e.is_major = major;
    if let Some(sid) = at {
        if let Some(s) = h.settlements.get(&sid) {
            let (loc, f) = (s.location, s.faction);
            e = e.at_location(loc.0, loc.1).with_faction(f).with_participant(EntityId::Settlement(sid));
            h.tile_history.record_event(loc.0, loc.1, id);
        }
    }
    for c in consequences { e = e.with_consequence(c); }
    h.chronicle.record(e);
}

/// Once a year (in summer), what the sky did.
pub fn step_sky(h: &mut WorldHistory, world: &WorldData) {
    let Some(sys) = world.star_system.as_ref().filter(|s| !s.is_sol()) else { return };
    if h.current_date.season != crate::seasons::Season::Summer { return; }
    let seed = world.seed();
    let year = h.current_date.year;
    let r = rates(sys, seed);
    let towns = living_towns(h);
    if towns.is_empty() { return; }
    let star = sys.star.name.clone();
    let pick = |k: u64| towns[((roll(seed, year, k) * towns.len() as f32) as usize).min(towns.len() - 1)];

    // A great flare: the harvest burns where the star stands highest.
    if roll(seed, year, 1) < r.flare {
        let locked = sys.world.spin.locked.then(|| sys.world.spin.substellar_lon_deg);
        let mut struck: Vec<SettlementId> = towns.iter().copied().filter(|sid| {
            let s = &h.settlements[sid];
            let lat = 90.0 - (s.location.1 as f32 + 0.5) / world.height as f32 * 180.0;
            match locked {
                Some(lon0) => {
                    let lon = (s.location.0 as f32 + 0.5) / world.width as f32 * 360.0;
                    (lat.to_radians().cos() * (lon - lon0).to_radians().cos()) > 0.5
                }
                None => lat.abs() < 35.0,
            }
        }).collect();
        if struck.is_empty() { struck = vec![pick(2)]; }
        // One to three of them, by the year's roll.
        struck.sort_by(|a, b| roll(seed, year, 50 + a.0 as u64).partial_cmp(&roll(seed, year, 50 + b.0 as u64)).unwrap());
        struck.truncate(1 + (roll(seed, year, 4) * 3.0) as usize);
        let mut dead_total = 0u32;
        let mut names = Vec::new();
        let mut cons = Vec::new();
        for sid in &struck {
            let s = h.settlements.get_mut(sid).unwrap();
            let dead = (s.population as f32 * (0.01 + 0.02 * roll(seed, year, 3 + sid.0 as u64))) as u32;
            s.population = s.population.saturating_sub(dead);
            let f = s.faction;
            names.push(s.name.clone());
            dead_total += dead;
            if let Some(fa) = h.factions.get_mut(&f) { fa.total_population = fa.total_population.saturating_sub(dead); }
            cons.push(Consequence::PopulationChange(f, -(dead as i32)));
        }
        let list = match names.len() { 1 => names[0].clone(), 2 => format!("{} and {}", names[0], names[1]), _ => format!("{}, {} and {}", names[0], names[1], names[2]) };
        record(h, format!("The Burning Sky over {}", names[0]),
            format!("{} flared at noon and the sky burned: the crops withered in the fields of {} and fires ran through the dry grass{}. That night the aurora hung low over the whole land.", star, list,
                if dead_total > 0 { format!("; {} died in the hungry months after", dead_total) } else { String::new() }),
            Some(struck[0]), dead_total > 200, cons);
    }

    // A total eclipse over a seat: an omen against the people's worst neighbour.
    if roll(seed, year, 11) < r.eclipse {
        let moon = sys.world.moons.iter().filter(|m| m.angular_deg(6371.0) >= 0.95 * sys.sun_angular_deg()).map(|m| m.name.clone()).next().unwrap_or_else(|| "the moon".into());
        let mut seats: Vec<(SettlementId, crate::history::FactionId)> = h.factions.values().filter(|f| f.is_active()).filter_map(|f| f.capital.map(|c| (c, f.id))).collect();
        seats.sort();
        if !seats.is_empty() {
            let (sid, fid) = seats[((roll(seed, year, 12) * seats.len() as f32) as usize).min(seats.len() - 1)];
            let worst = h.factions.get(&fid).and_then(|f| f.relations.iter().filter(|(o, _)| h.factions.get(o).map_or(false, |x| x.is_active())).min_by_key(|(o, r)| (r.opinion, o.0)).map(|(o, _)| *o));
            let (town, people) = (h.settlements[&sid].name.clone(), h.factions[&fid].name.clone());
            let text = match worst {
                Some(enemy) => {
                    let en = h.factions[&enemy].name.clone();
                    if let Some(rel) = h.factions.get_mut(&fid).and_then(|f| f.relations.get_mut(&enemy)) { rel.adjust_opinion(-6); }
                    format!("At midday {} swallowed {} over {}, and the stars came out. The wise of {} read it as a sign against {}.", moon, star, town, people, en)
                }
                None => format!("At midday {} swallowed {} over {}, and the stars came out; the people of {} feared it for a generation.", moon, star, town, people),
            };
            record(h, format!("The Eclipse over {}", town), text, Some(sid), false, Vec::new());
        }
    }

    // A comet out of the icy belt.
    if roll(seed, year, 21) < r.comet {
        let belt = sys.belts.iter().find(|b| b.icy).map(|b| b.name.clone()).unwrap_or_default();
        let sid = pick(22);
        let town = h.settlements[&sid].name.clone();
        let omen = ["war", "a plague", "the fall of a king", "a hard winter", "a great harvest", "the end of a line of kings"][((roll(seed, year, 23) * 6.0) as usize).min(5)];
        record(h, format!("The Comet of {}", year),
            format!("A comet fell inward from {} and hung over the world for a season, its tail across half the sky; at {} they said it foretold {}.", belt, town, omen),
            Some(sid), false, Vec::new());
    }

    // A close pair: the lesser sun passes before the greater.
    if roll(seed, year, 31) < r.pair_eclipse {
        if let Some(c) = &sys.companion {
            let sid = pick(32);
            let town = h.settlements[&sid].name.clone();
            record(h, format!("The Two Suns Meet over {}", town),
                format!("{} passed before {} and the day dimmed to a bronze dusk; at {} the shadows ran single for an hour, and the old wives spoke of the end of days.", c.star.name, star, town),
                Some(sid), false, Vec::new());
        }
    }

    // The far companion at its closest.
    if let (Some((period, closest)), Some(c)) = (r.approach, sys.companion.as_ref()) {
        let phase = (year as f32 - closest).rem_euclid(period);
        if phase < 1.0 {
            let sid = pick(42);
            let town = h.settlements[&sid].name.clone();
            record(h, format!("The Year Without Night"),
                format!("{} came to its nearest, as it does every {:.0} years: for a year the night was never dark, the beasts would not sleep, and at {} they kept the old vigil.", c.star.name, period, town),
                Some(sid), true, Vec::new());
        }
    }
    let _ = PlanetKind::Home;
}
