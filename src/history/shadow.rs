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
const MAX_STRENGTH: f32 = 2.0;
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
    /// Whether the free peoples' check (the Last Alliance) has come. Not saved: a loaded history
    /// is finished, and the chronicle records it (`ShadowAlliance`).
    #[serde(skip)]
    checked: bool,
}

impl Shadow {
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.corruption[y * self.width + x]
    }

    /// Share of the land in the Shadow's reach (corruption at or above `REACH`).
    pub fn land_share(&self) -> f32 {
        let land = self.conduct.iter().filter(|&&c| c > 0.0).count().max(1);
        let dark = self.corruption.iter().zip(&self.conduct).filter(|(&k, &c)| c > 0.0 && k >= REACH).count();
        dark as f32 / land as f32
    }

    /// Share of the land the Shadow's people own (its dominion and towns).
    pub fn held_share(&self, history: &WorldHistory) -> f32 {
        let (mut land, mut held) = (0usize, 0usize);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.conduct[y * self.width + x] <= 0.0 { continue; }
                land += 1;
                if history.tile_history.get(x, y).current_owner == Some(self.faction) { held += 1; }
            }
        }
        held as f32 / land.max(1) as f32
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
        checked: false,
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
        if history.current_date.season == crate::seasons::Season::Spring {
            liberate(&mut shadow, history);
        }
        let mut checked_now = false;
        if !shadow.checked && history.current_date.season == crate::seasons::Season::Spring {
            let start = history.config.prehistory_depth + 1;
            let years = history.config.simulation_years as f32;
            let at = start + (years * (CHECK_AT + CHECK_SPREAD * roll(shadow.seed, 0, 0xC4EC))) as u32;
            if history.current_date.year >= at { check(&mut shadow, history); checked_now = true; }
        }
        // The season of the check it does not strike: its seat has just fallen.
        if !checked_now && shadow.seasons >= shadow.next_strike {
            strike(&mut shadow, history);
            let interval = (24.0 / shadow.strength).clamp(12.0, 24.0) as u32;
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
                    // Dominion feeds the darkness weakly: its reach follows the towns it holds
                    // (at 0.5 the land fed itself and crept over half the map).
                    Some(o) if o == fid => source[i] = 0.35,
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

/// When the free peoples' check comes, as a share of the simulated years (plus up to the spread):
/// late enough that the Shadow has risen and taken its first great town, early enough that it
/// returns (`RETURN_AFTER`) and has a frontier again by the present day.
const CHECK_AT: f32 = 0.55;
const CHECK_SPREAD: f32 = 0.1;
const BANE_FORMS: [&str; 5] = ["Spear", "Blade", "Bow", "Hammer", "Lance"];

/// The free peoples' check, the third act of the Shadow's story: the peoples nearest its seat
/// unite under the ruler of the greatest of them and storm the seat. The champion strikes down
/// the Dark Lord and falls in the same hour; the champion's weapon is lost in the fall of the
/// seat (the Shadow's bane: what wounded it once can wound it again, a quest the present day
/// inherits); the seat goes to the champion's people, so the Shadow is broken, to return about
/// twenty years later in a new seat. Nothing happens if the seat or a champion is missing.
fn check(shadow: &mut Shadow, history: &mut WorldHistory) {
    use crate::history::entities::traits::DeathCause;
    use crate::history::objects::artifacts::{Artifact, ArtifactQuality, ArtifactType};
    shadow.checked = true;
    let date = history.current_date;
    let fid = shadow.faction;
    let seat = shadow.seat_settlement;
    let Some((seat_name, seat_loc)) = history.settlements.get(&seat)
        .filter(|s| !s.is_destroyed() && s.faction == fid).map(|s| (s.name.clone(), s.location)) else { return };
    let living = |f: Option<crate::history::FigureId>| f.filter(|id| history.figures.get(id).map_or(false, |x| x.is_alive()));
    let Some(lord) = living(history.factions.get(&fid).and_then(|f| f.current_leader)) else { return };
    let mut free: Vec<(i64, FactionId)> = history.factions.values()
        .filter(|f| f.is_active() && f.id != fid)
        .filter_map(|f| f.capital.and_then(|c| history.settlements.get(&c)).map(|t| {
            let dx = wrap_dx(t.location.0, seat_loc.0, shadow.width);
            let dy = t.location.1 as i64 - seat_loc.1 as i64;
            (dx * dx + dy * dy, f.id)
        }))
        .collect();
    free.sort();
    free.truncate(4);
    let Some(leader) = free.iter().map(|x| x.1).max_by_key(|f| (history.factions[f].total_population, std::cmp::Reverse(*f))) else { return };
    let Some(champion) = living(history.factions[&leader].current_leader) else { return };

    let name_of = |f: crate::history::FigureId| history.figures.get(&f).map(|x| x.full_name()).unwrap_or_default();
    let (champ_full, champ_name) = (name_of(champion), history.figures[&champion].name.clone());
    let lord_full = format!("{}, {}", history.figures[&lord].name, shadow.archetype.lord_title());
    let leader_name = history.factions[&leader].name.clone();
    let mut members: Vec<String> = free.iter().map(|x| history.factions[&x.1].name.clone()).collect();
    let last = members.pop().unwrap_or_default();
    let hosts = if members.is_empty() { last } else { format!("{} and {}", members.join(", "), last) };

    // The shape of the check, from the state of the world (not a roll): how many the free
    // peoples are against the Shadow's realm, how they feel about each other, how strong it is,
    // and how the champion's people feel about the Shadow's.
    let pop = |f: FactionId| history.factions.get(&f).map_or(0, |x| x.total_population) as f32;
    let (shadow_pop, allied_pop) = (pop(fid), free.iter().map(|x| pop(x.1)).sum::<f32>());
    let opinion = |a: FactionId, b: FactionId| history.factions.get(&a).and_then(|f| f.relations.get(&b)).map_or(0, |r| r.opinion);
    let discord = free.iter().flat_map(|a| free.iter().filter(move |b| b.1 != a.1).map(move |b| (a.1, b.1))).map(|(a, b)| opinion(a, b)).min().unwrap_or(0);
    // Thresholds from dev seeds (median discord -78, allies 2-40x the Shadow's realm).
    let shape = if allied_pop < shadow_pop * 1.2 { CheckShape::ShadowWins }
        else if opinion(leader, fid) >= 0 { CheckShape::ChampionTurns }
        else if discord <= -95 { CheckShape::AllianceBreaks }
        else if shadow.strength >= 1.5 && allied_pop < shadow_pop * 3.0 { CheckShape::Tribute }
        else { CheckShape::Victory };
    if std::env::var("PLANET_DEBUG_CHECK").is_ok() {
        eprintln!("CHECK shadow_pop {:.0} allied {:.0} ratio {:.2} discord {} strength {:.2} leader->shadow {} => {:?}", shadow_pop, allied_pop, allied_pop / shadow_pop.max(1.0), discord, shadow.strength, opinion(leader, fid), shape);
    }
    if shape != CheckShape::Victory {
        other_shape(shadow, history, shape, (fid, seat, seat_name, seat_loc, lord, leader, champion), &hosts, &free);
        return;
    }
    let form = BANE_FORMS[(roll(shadow.seed, shadow.seasons, 0xBA4E) * BANE_FORMS.len() as f32) as usize % BANE_FORMS.len()];
    let bane_name = format!("the {} of {}", form, champ_name);

    // The deaths, the seat taken.
    for f in [lord, champion] {
        if let Some(x) = history.figures.get_mut(&f) { x.kill(date, DeathCause::Battle); }
    }
    if let Some(x) = history.figures.get_mut(&champion) { x.kills.push(EntityId::Figure(lord)); }
    if let Some(f) = history.factions.get_mut(&fid) { f.remove_settlement(seat); }
    if let Some(f) = history.factions.get_mut(&leader) { f.add_settlement(seat); }
    if let Some(s) = history.settlements.get_mut(&seat) { s.faction = leader; }
    history.tile_history.set_owner(seat_loc.0, seat_loc.1, leader, date);
    shadow.fallen.retain(|t| *t != seat);

    let alliance = history.id_generators.next_event();
    let mut event = Event::new(alliance, EventType::ShadowAlliance, date,
        format!("The Last Alliance storms {}", seat_name),
        format!("{} gathered under {} of {} and marched on {}. At the gates of {}, {} struck down {} and fell in the same hour, and {} was taken.",
            hosts, champ_full, leader_name, shadow.name, seat_name, champ_name, lord_full, seat_name))
        .at_location(seat_loc.0, seat_loc.1)
        .with_faction(leader)
        .with_participant(EntityId::Figure(champion))
        .with_participant(EntityId::Figure(lord))
        .with_participant(EntityId::Settlement(seat))
        .caused_by(shadow.last_deed);
    for x in &free { if x.1 != leader { event = event.with_faction(x.1); } }
    event = event.with_faction(fid);
    history.tile_history.record_event(seat_loc.0, seat_loc.1, alliance);
    history.chronicle.record(event);

    let id = history.id_generators.next_artifact();
    let mut art = Artifact::new(id, capitalize(&bane_name), ArtifactType::Weapon, ArtifactQuality::Legendary, date, Some(champion));
    art.description = format!("The {} {} carried against {}. With it {} struck down {} at the gates of {} in the year {}; it was lost in the fall of the seat, and it is said that what wounded the Shadow once can wound it again.",
        form.to_lowercase(), champ_full, shadow.name, champ_name, lord_full, seat_name, date.year);
    art.creation_event = Some(alliance);
    art.creation_location = Some(seat_loc);
    art.current_location = Some(seat_loc);
    art.current_owner = None;
    art.lost = true;
    art.historical_importance = 1000;
    let bane_text = art.description.clone();
    history.artifacts.insert(id, art);
    let bane = history.id_generators.next_event();
    let event = Event::new(bane, EventType::ShadowBane, date, format!("{} is lost at {}", capitalize(&bane_name), seat_name), bane_text)
        .at_location(seat_loc.0, seat_loc.1)
        .with_participant(EntityId::Artifact(id))
        .with_participant(EntityId::Settlement(seat))
        .caused_by(alliance);
    history.chronicle.record(event);
    shadow.last_deed = bane;
}

/// Yearly chance that a town the Shadow took rises and is freed.
const LIBERATION_CHANCE: f32 = 0.05;
/// A free people within this many tiles (at 512 wide; scaled with the map) can free a town
/// whose old people is gone.
const LIBERATOR_RANGE: f32 = 40.0;

/// Towns don't stay taken forever: each year a town the Shadow conquered may rise and be freed,
/// by its old people if they still stand, else by the nearest free people. Without it the
/// Shadow's holdings only grew, and by the present day it darkened half the land or more.
fn liberate(shadow: &mut Shadow, history: &mut WorldHistory) {
    let date = history.current_date;
    let fid = shadow.faction;
    let mut held: Vec<SettlementId> = shadow.fallen.iter().copied()
        .filter(|t| *t != shadow.seat_settlement)
        .filter(|t| history.settlements.get(t).map_or(false, |s| !s.is_destroyed() && s.faction == fid))
        .collect();
    held.sort();
    held.dedup();
    let range = LIBERATOR_RANGE * shadow.width as f32 / 512.0;
    for town in held {
        if roll(shadow.seed, shadow.seasons, town.0 as u64 ^ 0x11B) >= LIBERATION_CHANCE { continue; }
        let Some((name, loc)) = history.settlements.get(&town).map(|s| (s.name.clone(), s.location)) else { continue };
        // Its old people, from the conquest that took it.
        let conquest = history.chronicle.events.iter().rev()
            .find(|e| e.event_type == EventType::ShadowConquest && e.primary_participants.contains(&EntityId::Settlement(town)));
        let old_people = conquest.and_then(|e| e.factions_involved.iter().copied().find(|&f| f != fid))
            .filter(|f| history.factions.get(f).map_or(false, |x| x.is_active()));
        let cause = conquest.map(|e| e.id);
        let liberator = old_people.or_else(|| {
            history.settlements.values()
                .filter(|s| !s.is_destroyed() && s.faction != fid)
                .filter(|s| history.factions.get(&s.faction).map_or(false, |f| f.is_active()))
                .map(|s| {
                    let dx = wrap_dx(s.location.0, loc.0, shadow.width) as f32;
                    let dy = s.location.1 as f32 - loc.1 as f32;
                    ((dx * dx + dy * dy).sqrt(), s.faction)
                })
                .filter(|(d, _)| *d <= range)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                .map(|(_, f)| f)
        });
        let Some(liberator) = liberator else { continue };

        if let Some(f) = history.factions.get_mut(&fid) { f.remove_settlement(town); }
        if let Some(f) = history.factions.get_mut(&liberator) { f.add_settlement(town); }
        if let Some(s) = history.settlements.get_mut(&town) { s.faction = liberator; }
        history.tile_history.set_owner(loc.0, loc.1, liberator, date);
        shadow.fallen.retain(|t| *t != town);
        shadow.strength = (shadow.strength - 0.05).max(0.6);

        let lib_name = history.factions.get(&liberator).map(|f| f.name.clone()).unwrap_or_default();
        let returned = Some(liberator) == old_people;
        let event_id = history.id_generators.next_event();
        let mut event = Event::new(
            event_id,
            EventType::ShadowLiberated,
            date,
            format!("{} freed from {}", name, shadow.name),
            if returned {
                format!("The people of {} rose against {} and threw open the gates to {}; the town is theirs again.", name, shadow.name, lib_name)
            } else {
                format!("{} drove the hosts of {} out of {}, and the town is free under its banner.", lib_name, shadow.name, name)
            },
        )
        .at_location(loc.0, loc.1)
        .with_faction(liberator)
        .with_faction(fid)
        .with_participant(EntityId::Settlement(town));
        if let Some(c) = cause { event = event.caused_by(c); }
        history.tile_history.record_event(loc.0, loc.1, event_id);
        history.chronicle.record(event);
    }
}

/// Strikes the free peoples remember when they decide how hard to resist.
const MEMORY_STRIKES: usize = 12;

/// How hard the free peoples resist, from the Shadow's recent strikes: a run of falls makes them
/// rally (+0.25 per fall), a run of victories makes them complacent (-0.5 per town held). It
/// balances at about two falls per town held whatever the map size: on a 512x256 world with
/// hundreds of villages in reach the Shadow used to win every strike (112 falls, none held) and on
/// the dev world it lost nine in ten, because a global count of falls was the only check on it.
fn rally(history: &WorldHistory) -> f32 {
    let (mut falls, mut holds) = (0, 0);
    for e in history.chronicle.events.iter().rev() {
        match e.event_type {
            EventType::ShadowConquest => falls += 1,
            EventType::ShadowRepelled => holds += 1,
            EventType::ShadowRose => break,
            _ => continue,
        }
        if falls + holds >= MEMORY_STRIKES { break; }
    }
    (0.25 * falls as f32 - 0.5 * holds as f32).clamp(-0.8, 1.5)
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
    // Nothing within reach: it gathers strength, and its darkness carries further (otherwise it
    // could stall a hair short of every free town, with no frontier left at all).
    let Some(&(_, target)) = targets.first() else {
        shadow.strength = (shadow.strength + 0.05).min(MAX_STRENGTH);
        return;
    };

    let (name, loc, pop, defender, capital) = {
        let s = &history.settlements[&target];
        (s.name.clone(), s.location, s.population, s.faction,
         s.settlement_type == crate::history::civilizations::settlement::SettlementType::Capital)
    };
    let def_name = history.factions.get(&defender).map(|f| f.name.clone()).unwrap_or_default();
    let attack = shadow.strength * (0.7 + 0.6 * roll(shadow.seed, shadow.seasons, target.0));
    let rally = rally(history);
    let defense = 0.7 + (pop as f32 / 2500.0).min(2.0) + if capital { 0.6 } else { 0.0 } + rally;
    let event_id = history.id_generators.next_event();
    let event = if attack > defense {
        let burn = !capital && roll(shadow.seed, shadow.seasons, target.0 ^ 0xB0) < 0.5;
        if let Some(f) = history.factions.get_mut(&defender) { f.remove_settlement(target); }
        shadow.fallen.push(target);
        shadow.strength = (shadow.strength + 0.04).min(MAX_STRENGTH);
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
        shadow.strength = (shadow.strength - 0.1).max(0.6);
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

/// The shapes the free peoples' check can take (only a victory leaves a bane).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckShape { Victory, AllianceBreaks, Tribute, ChampionTurns, ShadowWins }

impl CheckShape {
    /// The shape's name, as `--present` gives it.
    pub fn word(self) -> &'static str {
        match self {
            CheckShape::Victory => "the Last Alliance won", CheckShape::AllianceBreaks => "the alliance broke before the gate",
            CheckShape::Tribute => "peace was bought with tribute", CheckShape::ChampionTurns => "the champion took the dark crown",
            CheckShape::ShadowWins => "the Shadow won",
        }
    }
    /// The shape of a recorded check, read from its title.
    pub fn of_title(t: &str) -> CheckShape {
        if t.contains(" breaks before ") { CheckShape::AllianceBreaks }
        else if t.contains(" buys peace ") { CheckShape::Tribute }
        else if t.contains(" takes the crown ") { CheckShape::ChampionTurns }
        else if t.contains(" is destroyed before ") { CheckShape::ShadowWins }
        else { CheckShape::Victory }
    }
}

type CheckCast = (FactionId, SettlementId, String, (usize, usize), crate::history::FigureId, FactionId, crate::history::FigureId);

/// The check's other shapes: recorded as the same chronicle kind (`ShadowAlliance`), each with
/// its consequence for the present the game inherits.
fn other_shape(shadow: &mut Shadow, history: &mut WorldHistory, shape: CheckShape, cast: CheckCast, hosts: &str, free: &[(i64, FactionId)]) {
    use crate::history::entities::traits::DeathCause;
    let (fid, seat, seat_name, seat_loc, lord, leader, champion) = cast;
    let date = history.current_date;
    let name_of = |f: crate::history::FigureId| history.figures.get(&f).map(|x| x.full_name()).unwrap_or_default();
    let (champ_full, lord_full) = (name_of(champion), format!("{}, {}", history.figures[&lord].name, shadow.archetype.lord_title()));
    let leader_name = history.factions[&leader].name.clone();
    let (title, text) = match shape {
        CheckShape::ShadowWins => {
            if let Some(x) = history.figures.get_mut(&champion) { x.kill(date, DeathCause::Battle); }
            shadow.strength = (shadow.strength + 0.3).min(MAX_STRENGTH);
            (format!("The Last Alliance is destroyed before {}", seat_name),
             format!("{} marched on {} under {}, too few against the hosts of {}; at the gates of {} the alliance was destroyed and {} fell. The Shadow's grip on the land tightened.", hosts, shadow.name, champ_full, lord_full, seat_name, champ_full))
        }
        CheckShape::AllianceBreaks => {
            // Old hatreds: the allies turn on each other and the grudge deepens.
            for a in free { for b in free {
                if a.1 == b.1 { continue; }
                if let Some(r) = history.factions.get_mut(&a.1).and_then(|f| f.relations.get_mut(&b.1)) { r.opinion -= 20; }
            } }
            shadow.strength = (shadow.strength + 0.15).min(MAX_STRENGTH);
            (format!("The Last Alliance breaks before {}", seat_name),
             format!("{} gathered against {}, but old hatreds broke the alliance before the gates of {}; the hosts went home quarrelling, each blaming the others, and {} laughed.", hosts, shadow.name, seat_name, lord_full))
        }
        CheckShape::Tribute => {
            shadow.strength = (shadow.strength + 0.1).min(MAX_STRENGTH);
            (format!("{} buys peace from {}", leader_name, shadow.name),
             format!("No alliance would march. {} sent tribute to {} at {} and bought a peace that holds by fear; the other peoples call it shame.", leader_name, lord_full, seat_name))
        }
        CheckShape::ChampionTurns => {
            // The champion strikes down the lord and puts on the dark crown: the Shadow does not
            // break, it changes hands.
            if let Some(x) = history.figures.get_mut(&lord) { x.kill(date, DeathCause::Battle); }
            if let Some(f) = history.factions.get_mut(&fid) { f.remove_settlement(seat); }
            if let Some(f) = history.factions.get_mut(&leader) { f.add_settlement(seat); f.current_leader = Some(champion); }
            if let Some(t) = history.settlements.get_mut(&seat) { t.faction = leader; }
            history.tile_history.set_owner(seat_loc.0, seat_loc.1, leader, date);
            shadow.faction = leader;
            (format!("{} takes the crown of {}", champ_full, shadow.name),
             format!("{} led {} against {} and struck down {} at the gates of {}; but in the throne room the champion put on the dark crown, and {} serves a new lord.", champ_full, hosts, shadow.name, lord_full, seat_name, shadow.name))
        }
        CheckShape::Victory => unreachable!(),
    };
    let id = history.id_generators.next_event();
    let mut event = Event::new(id, EventType::ShadowAlliance, date, title, text)
        .at_location(seat_loc.0, seat_loc.1)
        .with_faction(leader)
        .with_participant(EntityId::Figure(champion))
        .with_participant(EntityId::Figure(lord))
        .with_participant(EntityId::Settlement(seat))
        .caused_by(shadow.last_deed);
    for x in free { if x.1 != leader { event = event.with_faction(x.1); } }
    event = event.with_faction(fid);
    history.tile_history.record_event(seat_loc.0, seat_loc.1, id);
    history.chronicle.record(event);
    shadow.last_deed = id;
}

