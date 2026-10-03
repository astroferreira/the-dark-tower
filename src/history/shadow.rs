//! The Shadow: a dark power that rises at the start of history and spreads over the world.
//!
//! One realm is chosen at the dawn of history (a dark-leaning people with many neighbours in
//! reach) and its capital becomes the Shadow's seat. From there a corruption field spreads
//! every season: fast along roads and rivers, slowly over mountains, never across the sea,
//! held back by living towns. Its reach grows as the Shadow grows stronger. Every so often
//! the Shadow strikes the town deepest in its shadow: the town falls (captured, or burned) or
//! holds. Each deed is a chronicle event caused by the previous one, so the chain of falls
//! leads back to the rise. Land the Shadow holds is its dominion; long-shadowed land is
//! blighted (ash, dead woods) when drawn.
//!
//! The Shadow draws no numbers from the history's RNG (its rolls hash the seed, the season and
//! the target), so adding it doesn't reshuffle anything else.

use serde::{Deserialize, Serialize};

use crate::history::entities::races::RaceType;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, FactionId, SettlementId};
use crate::world::WorldData;

/// Corruption at or above which a tile is in the Shadow's reach (drawn as a grey wash).
pub const REACH: f32 = 0.15;
/// Corruption at or above which the land is blighted (ash and dead woods).
pub const BLIGHT: f32 = 0.6;
/// A town is a target once the Shadow's corruption at it reaches this.
const TARGET_REACH: f32 = 0.2;
/// Unowned land under this much corruption is claimed as dominion.
const CLAIM_REACH: f32 = 0.55;
const MAX_STRENGTH: f32 = 3.0;
/// Seasons the Shadow leaves a town alone after it held (it turns elsewhere).
const HELD_RESPITE: u32 = 40;
/// Seasons after its breaking before the Shadow returns in a new seat.
const RETURN_AFTER: u32 = 80;

/// What kind of dark power it is (names and flavour).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Archetype {
    /// Iron and fire: armies, burned towns.
    Warlord,
    /// The deathless: the dead rise, the woods die.
    Necromancer,
    /// A crown that devours: order bought with fear.
    Tyrant,
}

impl Archetype {
    pub fn lord_title(self) -> &'static str {
        match self {
            Archetype::Warlord => "the Dark Lord",
            Archetype::Necromancer => "the Deathless",
            Archetype::Tyrant => "the Black King",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shadow {
    pub faction: FactionId,
    pub archetype: Archetype,
    /// "the Shadow of <seat>".
    pub name: String,
    pub seat: (usize, usize),
    pub seat_settlement: SettlementId,
    pub width: usize,
    pub height: usize,
    /// Corruption per tile, 0..1.
    pub corruption: Vec<f32>,
    /// How far each tile carries corruption to its neighbours (terrain only; roads are added
    /// per step as they are built).
    conduct: Vec<f32>,
    /// Grows with time and conquest, shrinks with defeats; drives reach and strike rate.
    pub strength: f32,
    pub risen: EventId,
    /// The Shadow's most recent deed (each deed is caused by the one before).
    pub last_deed: EventId,
    pub broken: Option<EventId>,
    broken_season: u32,
    /// Seasons since the rise, and the season of the next strike.
    pub seasons: u32,
    next_strike: u32,
    /// Towns that fell to it (captured or burned), in order.
    pub fallen: Vec<SettlementId>,
    pub repelled: u32,
    /// Towns that threw it back, and the season they did: it turns elsewhere for a while.
    held: Vec<(SettlementId, u32)>,
    seed: u64,
}

impl Shadow {
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.corruption[y * self.width + x]
    }

    pub fn is_broken(&self) -> bool {
        self.broken.is_some()
    }

    /// Tiles in reach, and of those, blighted.
    pub fn extent(&self) -> (usize, usize) {
        let reach = self.corruption.iter().filter(|&&c| c >= REACH).count();
        let blight = self.corruption.iter().filter(|&&c| c >= BLIGHT).count();
        (reach, blight)
    }

    /// The Dark Lord's name and title, from the realm's current ruler.
    pub fn lord(&self, history: &WorldHistory) -> String {
        let ruler = history.factions.get(&self.faction)
            .and_then(|f| f.current_leader)
            .and_then(|id| history.figures.get(&id))
            .map(|f| f.name.clone());
        match ruler {
            Some(n) => format!("{}, {}", n, self.archetype.lord_title()),
            None => self.archetype.lord_title().to_string(),
        }
    }
}

/// Deterministic roll in [0, 1) from the world seed, the season and a salt.
fn roll(seed: u64, season: u32, salt: u64) -> f32 {
    let mut h = seed ^ (season as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn wrap_dx(a: usize, b: usize, w: usize) -> i64 {
    let d = (a as i64 - b as i64).rem_euclid(w as i64);
    d.min(w as i64 - d)
}

/// The realm a Shadow rises in: the one with the most foreign towns within striking distance
/// of its capital (dark-leaning peoples weigh more). None if no realm qualifies.
fn pick_realm(history: &WorldHistory, w: usize) -> Option<(FactionId, SettlementId)> {
    let radius = (w as i64 / 6).max(10);
    let mut ids: Vec<FactionId> = history.factions.keys().copied().collect();
    ids.sort();
    let mut best: Option<(f32, FactionId, SettlementId)> = None;
    for fid in ids {
        let f = &history.factions[&fid];
        if !f.is_active() { continue; }
        let Some(cap) = f.capital.filter(|c| history.settlements.get(c).map_or(false, |s| !s.is_destroyed() && s.faction == fid)) else { continue };
        let (cx, cy) = history.settlements[&cap].location;
        let targets = history.settlements.values()
            .filter(|s| !s.is_destroyed() && s.faction != fid)
            .filter(|s| {
                let dx = wrap_dx(s.location.0, cx, w);
                let dy = s.location.1 as i64 - cy as i64;
                dx * dx + dy * dy <= radius * radius
            })
            .count() as f32;
        if targets < 2.0 { continue; }
        let dark = history.races.get(&f.race_id).map_or(false, |r| matches!(r.base_type, RaceType::Orc | RaceType::Goblin | RaceType::Undead));
        let score = targets * if dark { 1.6 } else { 1.0 };
        if best.as_ref().map_or(true, |b| score > b.0) {
            best = Some((score, fid, cap));
        }
    }
    best.map(|(_, f, c)| (f, c))
}

fn archetype_of(history: &WorldHistory, fid: FactionId) -> Archetype {
    match history.factions.get(&fid).and_then(|f| history.races.get(&f.race_id)).map(|r| r.base_type.clone()) {
        Some(RaceType::Undead) => Archetype::Necromancer,
        Some(RaceType::Orc | RaceType::Goblin | RaceType::Giant) => Archetype::Warlord,
        _ => Archetype::Tyrant,
    }
}

/// Seat the Shadow in a realm and record its rising (a first rise, or a return caused by its
/// earlier breaking).
fn rise(shadow: &mut Shadow, history: &mut WorldHistory, fid: FactionId, cap: SettlementId, returning: bool) {
    let (seat, seat_name) = {
        let s = &history.settlements[&cap];
        (s.location, s.name.clone())
    };
    let faction_name = history.factions[&fid].name.clone();
    shadow.faction = fid;
    shadow.archetype = archetype_of(history, fid);
    shadow.name = format!("the Shadow of {}", seat_name);
    shadow.seat = seat;
    shadow.seat_settlement = cap;
    shadow.strength = 1.0;
    shadow.broken = None;
    shadow.held.clear();
    shadow.next_strike = shadow.seasons + 6;
    let i = seat.1 * shadow.width + seat.0;
    shadow.corruption[i] = 1.0;
    let lord = shadow.lord(history);
    let event_id = history.id_generators.next_event();
    let (title, text) = if returning {
        (format!("The Shadow returns at {}", seat_name),
         format!("The darkness that was broken has found a new seat: {} of {} takes up the old dominion at {}.", lord, faction_name, seat_name))
    } else {
        (format!("A shadow rises over {}", seat_name),
         format!("{} of {} claims dominion over all peoples from {}, and a darkness begins to spread from it.", lord, faction_name, seat_name))
    };
    let mut event = Event::new(event_id, EventType::ShadowRose, history.current_date, title, text)
        .at_location(seat.0, seat.1)
        .with_faction(fid)
        .with_participant(EntityId::Settlement(cap));
    if returning { event = event.caused_by(shadow.last_deed); }
    history.tile_history.record_event(seat.0, seat.1, event_id);
    history.chronicle.record(event);
    if !returning { shadow.risen = event_id; }
    shadow.last_deed = event_id;
}

/// Raise the Shadow at the dawn of history. Returns false if no realm qualifies (fewer than
/// two peoples, no capitals).
pub fn seed(history: &mut WorldHistory, world: &WorldData) -> bool {
    let (w, h) = (world.width, world.height);
    let Some((fid, cap)) = pick_realm(history, w) else { return false };

    // Terrain conductance: how much corruption a tile passes on. A narrow map has bigger
    // tiles, so each tile carries less.
    let k = (512.0 / w as f32).clamp(1.0, 6.0);
    let threshold = crate::water_bodies::river_flow_threshold(w);
    let mut conduct = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let e = *world.heightmap.get(x, y);
            let water = e < 0.0 || world.water_body_map.get(x, y).is_lake();
            let river = world.flow_accumulation.as_ref().map_or(false, |f| *f.get(x, y) >= threshold);
            let base: f32 = if water { 0.0 } else if river { 0.955 } else if e > 2200.0 { 0.72 } else if e > 1100.0 { 0.87 } else { 0.93 };
            conduct[y * w + x] = if base > 0.0 { base.powf(k) } else { 0.0 };
        }
    }

    let mut shadow = Shadow {
        faction: fid,
        archetype: Archetype::Tyrant,
        name: String::new(),
        seat: (0, 0),
        seat_settlement: cap,
        width: w,
        height: h,
        corruption: vec![0.0; w * h],
        conduct,
        strength: 1.0,
        risen: EventId(0),
        last_deed: EventId(0),
        broken: None,
        broken_season: 0,
        seasons: 0,
        next_strike: 6,
        fallen: Vec::new(),
        repelled: 0,
        held: Vec::new(),
        seed: world.seed() ^ 0x5AD0_0000_0000_0000,
    };
    rise(&mut shadow, history, fid, cap, false);
    history.shadow = Some(shadow);
    true
}

/// One season of the Shadow: spread, claim, strike; when broken, fade, and in time return.
pub fn step(history: &mut WorldHistory) {
    let Some(mut shadow) = history.shadow.take() else { return };
    shadow.seasons += 1;
    let alive = history.factions.get(&shadow.faction).map_or(false, |f| f.is_active())
        && history.settlements.get(&shadow.seat_settlement).map_or(false, |s| !s.is_destroyed() && s.faction == shadow.faction);
    if !alive && shadow.broken.is_none() {
        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::ShadowBroken,
            history.current_date,
            format!("{} is broken", capitalize(&shadow.name)),
            format!("With the fall of its seat, {} loses its hold on the land, and the darkness begins to lift.", shadow.name),
        )
        .at_location(shadow.seat.0, shadow.seat.1)
        .caused_by(shadow.last_deed);
        history.chronicle.record(event);
        shadow.broken = Some(event_id);
        shadow.broken_season = shadow.seasons;
        shadow.last_deed = event_id;
    }
    // A broken Shadow is not destroyed: after a generation it finds a new seat.
    if shadow.broken.is_some() && shadow.seasons >= shadow.broken_season + RETURN_AFTER {
        if let Some((fid, cap)) = pick_realm(history, shadow.width) {
            rise(&mut shadow, history, fid, cap, true);
        }
    }
    let alive = shadow.broken.is_none();
    spread(&mut shadow, history, alive);
    if alive {
        shadow.strength = (shadow.strength + 0.0015).min(MAX_STRENGTH);
        claim(&shadow, history);
        if shadow.seasons >= shadow.next_strike {
            strike(&mut shadow, history);
            let interval = (24.0 / shadow.strength).clamp(6.0, 24.0) as u32;
            shadow.next_strike = shadow.seasons + interval;
        }
    }
    history.shadow = Some(shadow);
}

/// Corruption spreads one tile per season from its sources (the seat, the Shadow's towns and
/// land), fading over distance by terrain; living towns of other peoples hold it back; land
/// it no longer feeds slowly recovers.
fn spread(shadow: &mut Shadow, history: &WorldHistory, alive: bool) {
    let (w, h) = (shadow.width, shadow.height);
    let fid = shadow.faction;
    let mut source = vec![0.0f32; w * h];
    let mut resist = vec![1.0f32; w * h];
    if alive {
        for y in 0..h {
            for x in 0..w {
                let t = history.tile_history.get(x, y);
                let i = y * w + x;
                match t.current_owner {
                    Some(o) if o == fid => source[i] = 0.5,
                    Some(_) => resist[i] = 0.92,
                    None => {}
                }
            }
        }
        for s in history.settlements.values() {
            if s.is_destroyed() { continue; }
            let i = s.location.1 * w + s.location.0;
            if s.faction == fid {
                source[i] = source[i].max(0.8);
            } else {
                resist[i] = 0.75 - (s.population as f32 / 20_000.0).min(0.35);
            }
        }
        if history.settlements.get(&shadow.seat_settlement).map_or(false, |s| !s.is_destroyed() && s.faction == fid) {
            source[shadow.seat.1 * w + shadow.seat.0] = 1.0;
        }
    }
    // A stronger Shadow carries further: conductance raised to a power below one.
    let reach = 1.0 / (0.6 + 0.4 * shadow.strength);
    let old = shadow.corruption.clone();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut c = shadow.conduct[i];
            if history.tile_history.get(x, y).has_road { c = c.max(0.96f32.powf((512.0 / w as f32).clamp(1.0, 6.0))); }
            let c = if c > 0.0 { c.powf(reach) } else { 0.0 };
            let mut inflow = 0.0f32;
            for dy in -1i64..=1 {
                let ny = y as i64 + dy;
                if ny < 0 || ny >= h as i64 { continue; }
                for dx in -1i64..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                    inflow = inflow.max(old[ny as usize * w + nx]);
                }
            }
            shadow.corruption[i] = source[i].max(old[i] * 0.985).max(inflow * c * resist[i]).min(1.0);
        }
    }
}

/// Wild land deep in the shadow becomes dominion.
fn claim(shadow: &Shadow, history: &mut WorldHistory) {
    let date = history.current_date;
    for y in 0..shadow.height {
        for x in 0..shadow.width {
            if shadow.at(x, y) < CLAIM_REACH || shadow.conduct[y * shadow.width + x] == 0.0 { continue; }
            if history.tile_history.get(x, y).current_owner.is_none() {
                history.tile_history.set_owner(x, y, shadow.faction, date);
            }
        }
    }
}

/// Strike the town deepest in the shadow: it falls (captured, or burned) or holds.
fn strike(shadow: &mut Shadow, history: &mut WorldHistory) {
    let date = history.current_date;
    let fid = shadow.faction;
    let mut targets: Vec<(f32, SettlementId)> = history.settlements.values()
        .filter(|s| !s.is_destroyed() && s.faction != fid)
        .filter(|s| !shadow.held.iter().any(|&(id, when)| id == s.id && shadow.seasons < when + HELD_RESPITE))
        .filter_map(|s| {
            let c = shadow.at(s.location.0, s.location.1);
            (c >= TARGET_REACH).then(|| (c - s.population as f32 / 40_000.0, s.id))
        })
        .collect();
    targets.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    let Some(&(_, target)) = targets.first() else { return };

    let (name, loc, pop, defender, capital) = {
        let s = &history.settlements[&target];
        (s.name.clone(), s.location, s.population, s.faction,
         s.settlement_type == crate::history::civilizations::settlement::SettlementType::Capital)
    };
    let def_name = history.factions.get(&defender).map(|f| f.name.clone()).unwrap_or_default();
    let attack = shadow.strength * (0.7 + 0.6 * roll(shadow.seed, shadow.seasons, target.0));
    // The free peoples rally as town after town falls.
    let rally = (0.07 * shadow.fallen.len() as f32).min(1.0);
    let defense = 0.7 + (pop as f32 / 2500.0).min(2.0) + if capital { 0.6 } else { 0.0 } + rally;
    let event_id = history.id_generators.next_event();
    let event = if attack > defense {
        let burn = !capital && roll(shadow.seed, shadow.seasons, target.0 ^ 0xB0) < 0.5;
        if let Some(f) = history.factions.get_mut(&defender) { f.remove_settlement(target); }
        shadow.fallen.push(target);
        shadow.strength = (shadow.strength + 0.08).min(MAX_STRENGTH);
        if burn {
            crate::history::simulation::step::destroy_settlement(history, target, date);
            Event::new(
                event_id,
                EventType::ShadowConquest,
                date,
                format!("{} burned by {}", name, shadow.name),
                format!("The hosts of {} fell upon {} of {} and left it in ashes; its survivors fled into exile.", shadow.name, name, def_name),
            )
        } else {
            if let Some(f) = history.factions.get_mut(&fid) { f.add_settlement(target); }
            if let Some(s) = history.settlements.get_mut(&target) { s.faction = fid; }
            // The land around the town goes with it.
            let r = 2i64;
            for dy in -r..=r {
                for dx in -r..=r {
                    let y = loc.1 as i64 + dy;
                    if y < 0 || y >= shadow.height as i64 { continue; }
                    let x = (loc.0 as i64 + dx).rem_euclid(shadow.width as i64) as usize;
                    let owner = history.tile_history.get(x, y as usize).current_owner;
                    if owner.is_none() || owner == Some(defender) {
                        history.tile_history.set_owner(x, y as usize, fid, date);
                    }
                }
            }
            Event::new(
                event_id,
                EventType::ShadowConquest,
                date,
                format!("{} falls to {}", name, shadow.name),
                format!("{} of {} was taken by {}; its people now labour under the darkness.", name, def_name, shadow.name),
            )
        }
    } else {
        shadow.repelled += 1;
        shadow.held.retain(|&(id, _)| id != target);
        shadow.held.push((target, shadow.seasons));
        shadow.strength = (shadow.strength - 0.25).max(0.6);
        Event::new(
            event_id,
            EventType::ShadowRepelled,
            date,
            format!("{} holds against {}", name, shadow.name),
            format!("The defenders of {} threw back the hosts of {}.", name, shadow.name),
        )
    };
    let event = event
        .at_location(loc.0, loc.1)
        .with_faction(fid)
        .with_faction(defender)
        .with_participant(EntityId::Settlement(target))
        .caused_by(shadow.last_deed);
    history.tile_history.record_event(loc.0, loc.1, event_id);
    history.chronicle.record(event);
    shadow.last_deed = event_id;
    crate::history::simulation::step::dissolve_if_landless(history, defender, &def_name, date);
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
