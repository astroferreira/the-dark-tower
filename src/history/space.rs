//! Disasters from space in the chronicle (ROADMAP Stage 7; only with upheaval, `crate::upheaval`,
//! so the Sun's histories are unchanged). The impact record (`upheaval::Impacts`) is the deep
//! past; this is what falls and burns within the chronicle's years, each event hashed from the
//! seed, the year and its kind (no draws from the history's RNG, as `history::sky`):
//!
//! - **Airbursts** (Tunguska, 1908: a stone of tens of metres explodes in the air and flattens
//!   and burns the forest for 20-40 km round): `AIRBURST_RATE` x the impact flux a year. Over a
//!   town's tile the blast and the fire kill a fifth to a half; the region burns the woods round
//!   the point (`landforms::hazard`) and marks it (`sites::violent`).
//! - **A stone that reaches the ground** and leaves a crater 50-600 m across (Barringer, Wabar):
//!   `FALL_RATE` x the flux. A town under it is destroyed; the region and the embark make the
//!   crater (a `MeteorCrater` place "of the year N", carved by `local::sites`).
//! - **A great stone in the sea**: `OCEAN_RATE` x the flux; a tsunami on every coastal town within
//!   `OCEAN_TSUNAMI_TILES` of where it fell.
//! - **A great storm of the sun** (a coronal mass ejection): auroras far from the poles and a night
//!   read as an omen, `STORM_RATE` / the world's magnetic field a year.
//! - **A new star** (a supernova in the galaxy, seen by day for weeks: 1006, 1054, 1572):
//!   `NEW_STAR_RATE` a year, an omen.

use crate::history::events::types::{Consequence, Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, SettlementId};
use crate::upheaval::unit3;
use crate::world::WorldData;

const AIRBURST_RATE: f32 = 0.006;
const FALL_RATE: f32 = 0.002;
const OCEAN_RATE: f32 = 0.0004;
const STORM_RATE: f32 = 0.015;
const NEW_STAR_RATE: f32 = 0.008;
const OCEAN_TSUNAMI_TILES: f32 = 6.0;
/// An airburst's blast reach (km): towns within it lose people.
const BLAST_KM: f32 = 35.0;

fn roll(seed: u64, year: u32, kind: u64) -> f32 { unit3(seed ^ 0x5BACE, (year as u64) << 8 | kind, 0x5BAC) }

fn record(h: &mut WorldHistory, kind: EventType, title: String, text: String, at: Option<(usize, usize)>, sid: Option<SettlementId>, dead: u32, cause: Option<EventId>, major: bool) -> EventId {
    let id = h.id_generators.next_event();
    let mut e = Event::new(id, kind, h.current_date, title, text);
    e.is_major = major;
    if let Some((x, y)) = at {
        e = e.at_location(x, y);
        h.tile_history.record_event(x, y, id);
    }
    if let Some(sid) = sid {
        if let Some(s) = h.settlements.get(&sid) {
            let f = s.faction;
            e = e.with_faction(f).with_participant(EntityId::Settlement(sid));
            if dead > 0 { e = e.with_consequence(Consequence::PopulationChange(f, -(dead as i32))); }
        }
    }
    if let Some(c) = cause { e = e.caused_by(c); }
    h.chronicle.record(e);
    id
}

fn kill(h: &mut WorldHistory, sid: SettlementId, share: f32) -> (u32, u32) {
    let Some(s) = h.settlements.get_mut(&sid) else { return (0, 0) };
    let before = s.population;
    let dead = ((before as f32) * share.clamp(0.0, 1.0)) as u32;
    s.population = s.population.saturating_sub(dead);
    let f = s.faction;
    if let Some(fa) = h.factions.get_mut(&f) { fa.total_population = fa.total_population.saturating_sub(dead); }
    (dead, before)
}

/// A point on the sphere for this year's event of `kind`: a tile.
fn spot(world: &WorldData, seed: u64, year: u32, kind: u64) -> (usize, usize) {
    let lon = roll(seed, year, kind * 16 + 1);
    let lat = (2.0 * roll(seed, year, kind * 16 + 2) - 1.0).asin();
    let x = ((lon * world.width as f32) as usize).min(world.width - 1);
    let y = (((0.5 - lat / std::f32::consts::PI) * world.height as f32) as usize).min(world.height - 1);
    (x, y)
}

fn nearest_town(h: &WorldHistory, world: &WorldData, x: usize, y: usize) -> Option<(SettlementId, f32)> {
    h.settlements.values().filter(|s| !s.is_destroyed())
        .map(|s| (s.id, crate::upheaval::distance_km(world.width, world.height, x as f32, y as f32, s.location.0 as f32, s.location.1 as f32)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)))
}

fn land(world: &WorldData, x: usize, y: usize) -> bool { *world.heightmap.get(x, y) > 0.0 }

fn region_word(world: &WorldData, h: &WorldHistory, x: usize, y: usize) -> String {
    match nearest_town(h, world, x, y) {
        Some((sid, km)) if km < 400.0 => format!("{} km from {}", ((km / 10.0).round() * 10.0).max(10.0), h.settlements[&sid].name),
        _ => "in the empty country".into(),
    }
}

/// The crater (m across) a stone that fell in `year` left: a pure function of the seed and the
/// year, so the region reads it from the chronicle's event (`geology::ground_events`).
pub fn fall_size_m(seed: u64, year: u32) -> f32 { 50.0 + 550.0 * roll(seed, year, 21).powi(2) }

/// The reach (km) of an airburst's flattened and burned forest.
pub const AIRBURST_BURN_KM: f32 = 25.0;

/// Once a year (in summer), what fell from the sky.
pub fn step(h: &mut WorldHistory, world: &WorldData) {
    let Some(up) = world.upheaval() else { return };
    if h.current_date.season != crate::seasons::Season::Summer { return; }
    let seed = world.seed();
    let year = h.current_date.year;
    let flux = up.impact_flux;

    // An airburst over the land: the forest flattened and burned.
    if roll(seed, year, 1) < AIRBURST_RATE * flux {
        let (x, y) = spot(world, seed, year, 1);
        if land(world, x, y) {
            let near = nearest_town(h, world, x, y);
            let mut dead_total = 0;
            let mut struck = None;
            if let Some((sid, km)) = near {
                if km < BLAST_KM {
                    let (dead, before) = kill(h, sid, 0.2 + 0.3 * roll(seed, year, 3));
                    dead_total = dead;
                    struck = Some((sid, before));
                }
            }
            let where_ = region_word(world, h, x, y);
            let text = match struck {
                Some((sid, _)) => format!("A light brighter than {} crossed the morning sky and burst over {}: the blast threw down the houses and the fire that followed burned the woods for a day's walk round; {} died.",
                    world.star_system.as_ref().map_or("the sun".to_string(), |s| s.star.name.clone()), h.settlements[&sid].name, dead_total),
                None => format!("A light brighter than the sun crossed the morning sky and burst {}: the thunder was heard a week's walk away, and the forest there lay flattened and burned, every trunk pointing away from the place.", where_),
            };
            let id = record(h, EventType::Skyfall, format!("The sky bursts {}", where_), text, Some((x, y)), struck.map(|s| s.0), dead_total, None, struck.is_some());
            if let Some((sid, before)) = struck { crate::history::hard_times::abandon_after(h, sid, before, id, "fire from the sky"); }
        }
    }

    // A stone that reaches the ground and leaves its crater.
    if roll(seed, year, 2) < FALL_RATE * flux {
        let (x, y) = spot(world, seed, year, 2);
        if land(world, x, y) {
            let d_m = fall_size_m(seed, year);
            let near = nearest_town(h, world, x, y);
            let where_ = region_word(world, h, x, y);
            let town_hit = near.filter(|(_, km)| *km < 8.0 + d_m / 100.0);
            let (text, sid, dead) = match town_hit {
                Some((sid, _)) => {
                    let (dead, before) = kill(h, sid, 0.6 + 0.4 * roll(seed, year, 22));
                    let name = h.settlements[&sid].name.clone();
                    (format!("A stone fell out of a clear sky on {}: where the town stood there is a pit {:.0} paces across, ringed with broken rock; {} died.", name, d_m * 1.3, dead), Some((sid, before)), dead)
                }
                None => (format!("A stone fell out of the sky {} and left a pit {:.0} paces across, ringed with broken rock and black iron; the shepherds who saw it fall would not go near it for a year.", where_, d_m * 1.3), None, 0),
            };
            let id = record(h, EventType::Skyfall, format!("A stone falls {}", where_), text, Some((x, y)), sid.map(|s| s.0), dead, None, sid.is_some());
            if let Some((sid, before)) = sid { crate::history::hard_times::abandon_after(h, sid, before, id, "stone from the sky"); }
        }
    }

    // A great stone in the sea: the wave on the coasts near.
    if roll(seed, year, 3) < OCEAN_RATE * flux {
        let (x, y) = spot(world, seed, year, 3);
        if !land(world, x, y) {
            let fall = record(h, EventType::Skyfall, "A stone falls into the sea".into(),
                "A great stone fell into the sea; the sky flashed white far over the horizon and the sea drew back.".into(), Some((x, y)), None, 0, None, true);
            let mut coast: Vec<SettlementId> = h.settlements.values()
                .filter(|s| !s.is_destroyed() && world.heightmap.neighbors_8(s.location.0, s.location.1).into_iter().any(|(nx, ny)| *world.heightmap.get(nx, ny) <= 0.0))
                .filter(|s| {
                    let mut dx = (s.location.0 as f32 - x as f32).abs();
                    dx = dx.min(world.width as f32 - dx);
                    dx.hypot(s.location.1 as f32 - y as f32) <= OCEAN_TSUNAMI_TILES
                })
                .map(|s| s.id).collect();
            coast.sort();
            for sid in coast {
                let (dead, before) = kill(h, sid, 0.1 + 0.3 * roll(seed, year, 30 + sid.0 as u64 % 64));
                let name = h.settlements[&sid].name.clone();
                let loc = h.settlements[&sid].location;
                let wave = record(h, EventType::Tsunami, format!("The sea rises over {}", name),
                    format!("Hours after the flash over the sea, the water drew back from {} and came again as a wall; {} drowned.", name, dead), Some(loc), Some(sid), dead, Some(fall), true);
                crate::history::hard_times::abandon_after(h, sid, before, wave, "wave");
            }
        }
    }

    // A great storm of the sun: curtains of light far from the poles.
    let field = world.star_system.as_ref().map_or(1.0, |s| s.world.magnetic_field).max(0.15);
    if roll(seed, year, 4) < (STORM_RATE / field).min(0.03) {
        // How far from the poles the light reached this time: the storm's strength, by the roll;
        // a town at about that latitude saw it, one by hash among those there.
        let reach_deg = 60.0 - 40.0 * roll(seed, year, 41);
        let mut towns: Vec<(SettlementId, f32)> = h.settlements.values().filter(|s| !s.is_destroyed()).map(|s| {
            let lat = 90.0 - (s.location.1 as f32 + 0.5) / world.height as f32 * 180.0;
            (s.id, (lat.abs() - reach_deg).abs())
        }).collect();
        towns.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));
        towns.truncate(6);
        if !towns.is_empty() {
            let sid = towns[((roll(seed, year, 42) * towns.len() as f32) as usize).min(towns.len() - 1)].0;
            let name = h.settlements[&sid].name.clone();
            let loc = h.settlements[&sid].location;
            record(h, EventType::Celestial, format!("The sky burns red over {}", name),
                format!("For three nights curtains of red and green light hung over {}, where no one had seen them before; the beasts would not settle, and the priests read it as a judgement.", name),
                Some(loc), Some(sid), 0, None, false);
        }
    }

    // A new star.
    if roll(seed, year, 5) < NEW_STAR_RATE {
        let towns: Vec<SettlementId> = { let mut v: Vec<SettlementId> = h.factions.values().filter(|f| f.is_active()).filter_map(|f| f.capital).collect(); v.sort(); v };
        if !towns.is_empty() {
            let sid = towns[((roll(seed, year, 51) * towns.len() as f32) as usize).min(towns.len() - 1)];
            let name = h.settlements[&sid].name.clone();
            let loc = h.settlements[&sid].location;
            record(h, EventType::Celestial, "A new star".into(),
                format!("A new star stood in the sky, bright enough to be seen by day for three weeks and to cast shadows at night; the astrologers of {} wrote that it foretold the fall of a great house.", name),
                Some(loc), Some(sid), 0, None, false);
        }
    }
}
