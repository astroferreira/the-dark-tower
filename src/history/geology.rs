//! The ground in the chronicle (ROADMAP Stage 7; only with upheaval, `crate::upheaval`): the
//! history's earthquakes, eruptions, landslides and tsunamis strike where the ground has them,
//! instead of at a town drawn by lot (`step_natural_events`, which leaves its quakes and
//! eruptions to this on an upheaval world).
//!
//! Once a year (in spring), for every living town, hashed from the seed, the year and the town
//! (no draws from the history's RNG, as `history::sky`):
//! - an **earthquake** with a chance of `QUAKE_RATE` x the fault zone's intensity squared a year
//!   (a town on a strong boundary is shaken every century or two): 2-15% of its people die, more
//!   on a strong fault; in high country (relief over 800 m round the tile) the shaking brings the
//!   hillside down on it one time in two (a **landslide**, caused by the quake), and where the
//!   fault is a thrust under the sea a **tsunami** follows one time in two and strikes every
//!   coastal town within `TSUNAMI_TILES`;
//! - an **eruption** of an active volcano within `ERUPTION_TILES` (a stratovolcano's
//!   `STRATO_RATE` a year, a shield's lava `SHIELD_RATE`, a caldera's great eruption
//!   `CALDERA_RATE`): the nearer, the more die; lava takes a town on the vent's own tile.
//! A town left with under half its people, or under sixty, may be given up (`hard_times`).

use crate::heightmap::VolcanoType;
use crate::history::events::types::{Consequence, Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, SettlementId};
use crate::upheaval::{unit3, FaultKind};
use crate::world::WorldData;

const QUAKE_RATE: f32 = 0.07;
const ERUPTION_TILES: f32 = 3.0;
const STRATO_RATE: f32 = 0.012;
const SHIELD_RATE: f32 = 0.02;
const CALDERA_RATE: f32 = 0.0015;
const TSUNAMI_TILES: i64 = 4;

/// A quake, slide, tsunami or eruption of the chronicle, where and when, for the region's
/// places (`region::sites::violent`).
#[derive(Clone, Debug)]
pub struct GroundEvent {
    pub tile: (usize, usize),
    pub year: u32,
    pub kind: EventType,
    pub title: String,
    pub id: EventId,
    /// A stone that fell (`EventType::Skyfall` with a crater): its crater across (m); 0 else.
    pub crater_m: f32,
    /// An airburst: the forest flattened and burned round the point.
    pub airburst: bool,
}

/// The chronicle's ground events (quakes, slides, tsunamis, eruptions, what fell from the sky)
/// with a place. `seed` is the world's (the size of a fallen stone's crater is hashed from it).
pub fn ground_events(h: &WorldHistory, seed: u64) -> Vec<GroundEvent> {
    h.chronicle.events.iter()
        .filter(|e| matches!(e.event_type, EventType::Earthquake | EventType::Landslide | EventType::Tsunami | EventType::VolcanoErupted | EventType::Skyfall))
        .filter_map(|e| e.location.map(|tile| {
            let fell = e.event_type == EventType::Skyfall && e.title.starts_with("A stone falls") && e.title != "A stone falls into the sea";
            GroundEvent { tile, year: e.date.year, kind: e.event_type.clone(), title: e.title.clone(), id: e.id,
                crater_m: if fell { crate::history::space::fall_size_m(seed, e.date.year) } else { 0.0 },
                airburst: e.event_type == EventType::Skyfall && e.title.starts_with("The sky bursts") }
        }))
        .collect()
}

fn roll(seed: u64, year: u32, town: u64, salt: u64) -> f32 { unit3(seed ^ (year as u64) << 20, town, salt) }

fn record(h: &mut WorldHistory, kind: EventType, title: String, text: String, sid: SettlementId, dead: u32, cause: Option<EventId>, major: bool) -> EventId {
    let id = h.id_generators.next_event();
    let mut e = Event::new(id, kind, h.current_date, title, text);
    e.is_major = major;
    if let Some(s) = h.settlements.get(&sid) {
        let (loc, f) = (s.location, s.faction);
        e = e.at_location(loc.0, loc.1).with_faction(f).with_participant(EntityId::Settlement(sid))
            .with_consequence(Consequence::PopulationChange(f, -(dead as i32)));
        h.tile_history.record_event(loc.0, loc.1, id);
    }
    if let Some(c) = cause { e = e.caused_by(c); }
    h.chronicle.record(e);
    id
}

/// Take `share` of a town's people; returns the dead and the count before.
fn kill(h: &mut WorldHistory, sid: SettlementId, share: f32) -> (u32, u32) {
    let Some(s) = h.settlements.get_mut(&sid) else { return (0, 0) };
    let before = s.population;
    let dead = ((before as f32) * share.clamp(0.0, 0.95)) as u32;
    s.population = s.population.saturating_sub(dead);
    let f = s.faction;
    if let Some(fa) = h.factions.get_mut(&f) { fa.total_population = fa.total_population.saturating_sub(dead); }
    (dead, before)
}

fn relief(world: &WorldData, x: usize, y: usize) -> f32 {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for (nx, ny) in world.heightmap.neighbors_8(x, y).into_iter().chain(std::iter::once((x, y))) {
        let e = *world.heightmap.get(nx, ny);
        lo = lo.min(e);
        hi = hi.max(e);
    }
    hi - lo
}

fn coastal(world: &WorldData, x: usize, y: usize) -> bool {
    world.heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| *world.heightmap.get(nx, ny) <= 0.0)
}

/// Once a year, what the ground did.
pub fn step(h: &mut WorldHistory, world: &WorldData) {
    if world.upheaval().is_none() { return; }
    if h.current_date.season != crate::seasons::Season::Spring { return; }
    let seed = world.seed();
    let year = h.current_date.year;
    let mut towns: Vec<SettlementId> = h.settlements.values().filter(|s| !s.is_destroyed()).map(|s| s.id).collect();
    towns.sort();
    let faults = world.faults();
    for sid in towns {
        let Some(s) = h.settlements.get(&sid) else { continue };
        if s.is_destroyed() { continue; }
        let (x, y) = s.location;
        let name = s.name.clone();
        let key = sid.0 as u64;
        // Earthquakes on the faults.
        let f = faults.tile(x, y);
        if f.kind != FaultKind::None && roll(seed, year, key, 0x0E1) < QUAKE_RATE * f.intensity * f.intensity {
            let share = 0.02 + 0.13 * f.intensity * roll(seed, year, key, 0x0E2);
            let (dead, before) = kill(h, sid, share);
            let how = match f.kind { FaultKind::Normal => "the ground dropped along the rift", FaultKind::Thrust => "the mountains' roots slipped", _ => "the land slid along the great fault" };
            let quake = record(h, EventType::Earthquake, format!("Earthquake at {name}"),
                format!("The earth shook at {name}: {how}, and {dead} died under the walls."), sid, dead, None, f.intensity > 0.5);
            crate::history::hard_times::abandon_after(h, sid, before, quake, "earthquake");
            if relief(world, x, y) > 800.0 && roll(seed, year, key, 0x0E3) < 0.5 {
                let (dead, before) = kill(h, sid, 0.03 + 0.1 * roll(seed, year, key, 0x0E4));
                let river = world.river_tile_cache.as_ref().map_or(false, |c| *c.get(x, y));
                let dam = if river { " and dammed the river into a lake above the town" } else { "" };
                let slide = record(h, EventType::Landslide, format!("The hillside falls on {name}"),
                    format!("The shaking brought the slope above {name} down{dam}; {dead} were buried."), sid, dead, Some(quake), false);
                crate::history::hard_times::abandon_after(h, sid, before, slide, "landslide");
            }
            if f.kind == FaultKind::Thrust && coastal(world, x, y) && roll(seed, year, key, 0x0E5) < 0.5 {
                let mut struck: Vec<SettlementId> = h.settlements.values()
                    .filter(|o| !o.is_destroyed() && coastal(world, o.location.0, o.location.1)
                        && (o.location.0 as i64 - x as i64).abs().min(world.width as i64 - (o.location.0 as i64 - x as i64).abs()) <= TSUNAMI_TILES
                        && (o.location.1 as i64 - y as i64).abs() <= TSUNAMI_TILES)
                    .map(|o| o.id).collect();
                struck.sort();
                for t in struck {
                    let tname = h.settlements.get(&t).map(|o| o.name.clone()).unwrap_or_default();
                    let (dead, before) = kill(h, t, 0.05 + 0.2 * roll(seed, year, t.0 as u64, 0x0E6));
                    let wave = record(h, EventType::Tsunami, format!("The sea rises over {tname}"),
                        format!("After the earthquake at {name} the sea drew back from {tname} and came again as a wall of water; {dead} drowned."), t, dead, Some(quake), true);
                    crate::history::hard_times::abandon_after(h, t, before, wave, "wave");
                }
            }
        }
        // Eruptions of the volcanoes near.
        let Some(s) = h.settlements.get(&sid) else { continue };
        if s.is_destroyed() { continue; }
        for (vi, v) in world.volcanoes.iter().enumerate().filter(|(_, v)| v.is_active) {
            let mut dx = (v.x as f32 - x as f32).abs();
            dx = dx.min(world.width as f32 - dx);
            let d = dx.hypot(v.y as f32 - y as f32);
            if d > ERUPTION_TILES { continue; }
            let rate = match v.volcano_type { VolcanoType::Stratovolcano => STRATO_RATE, VolcanoType::Shield => SHIELD_RATE, VolcanoType::Caldera => CALDERA_RATE };
            // One eruption a year per volcano, felt by every town near it.
            if roll(seed, year, 0xF00 + vi as u64, 0x0E7) >= rate { continue; }
            let near = 1.0 - d / (ERUPTION_TILES + 0.5);
            let (share, what) = match v.volcano_type {
                VolcanoType::Stratovolcano => (0.04 + 0.3 * near * near, "ash and burning cloud came down the mountain"),
                VolcanoType::Shield => (0.01 + 0.06 * near, "lava ran down the valleys"),
                VolcanoType::Caldera => (0.2 + 0.6 * near, "the mountain blew itself apart and ash buried the land"),
            };
            let (dead, before) = kill(h, sid, share);
            let ev = record(h, EventType::VolcanoErupted, format!("The mountain wakes over {name}"),
                format!("The volcano {:.0} leagues from {name} erupted: {what}; {dead} died.", d * crate::upheaval::tile_km(world.width) / 4.8), sid, dead, None, v.volcano_type != VolcanoType::Shield);
            if d < 0.5 && v.volcano_type == VolcanoType::Shield {
                // Lava over the town itself.
                if let Some(s) = h.settlements.get_mut(&sid) { s.population = s.population.min(40); }
            }
            crate::history::hard_times::abandon_after(h, sid, before, ev, "eruption");
            break;
        }
    }
}
