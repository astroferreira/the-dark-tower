//! Per-season simulation step.
//!
//! Each step processes one season of world history: population growth,
//! diplomacy, wars, creature activity, artifact creation, etc.

use rand::Rng;
use crate::world::WorldData;
use crate::history::*;
use crate::history::data::GameData;
use crate::history::time::Date;
use crate::history::events::types::{Event, EventType, Consequence};
use crate::history::world_state::WorldHistory;
use crate::history::entities::traits::{DeathCause, Personality, Skill};
use crate::history::entities::figures::Figure;
use crate::history::civilizations::diplomacy::DiplomaticStance;
use crate::history::civilizations::military::{War, WarCause};
use crate::history::objects::artifacts::{Artifact, ArtifactType, ArtifactQuality, AcquisitionMethod};
use crate::history::objects::monuments::{Monument, MonumentType, MonumentPurpose};
use crate::history::civilizations::economy::{TradeRoute, ResourceType};
use crate::history::civilizations::settlement::{Settlement, SettlementType};
use crate::history::naming::styles::NamingStyle;
use crate::history::naming::generator::NameGenerator;
use crate::history::religion::worship::{Religion, Doctrine};

/// Per-phase timing of the history simulation (`--history-profile`): off unless enabled, so the
/// simulation pays only an atomic load per phase.
pub mod profile {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    pub static ON: AtomicBool = AtomicBool::new(false);
    static TOTALS: Mutex<Vec<(&'static str, f64, u64)>> = Mutex::new(Vec::new());

    pub fn enabled() -> bool { ON.load(Ordering::Relaxed) }
    pub fn add(name: &'static str, secs: f64) {
        let mut t = TOTALS.lock().unwrap();
        match t.iter_mut().find(|e| e.0 == name) {
            Some(e) => { e.1 += secs; e.2 += 1; }
            None => t.push((name, secs, 1)),
        }
    }
    /// Phases by total time, slowest first: (name, seconds, calls).
    pub fn take() -> Vec<(&'static str, f64, u64)> {
        let mut t = std::mem::take(&mut *TOTALS.lock().unwrap());
        t.sort_by(|a, b| b.1.total_cmp(&a.1));
        t
    }
}

#[inline]
fn timed<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
    if !profile::enabled() { return f(); }
    let t0 = std::time::Instant::now();
    let r = f();
    profile::add(name, t0.elapsed().as_secs_f64());
    r
}

/// Run one season of simulation.
pub fn simulate_step(
    history: &mut WorldHistory,
    world: &WorldData,
    game_data: &GameData,
    rng: &mut impl Rng,
) {
    let date = history.current_date;

    // 1. Population growth
    timed("population_growth", || step_population_growth(history));

    // 2. Settlement upgrades
    timed("settlement_upgrades", || step_settlement_upgrades(history));

    // 2.5 Territory expansion
    timed("territory_expansion", || step_territory_expansion(history, world, rng));

    // 2.6 Colonization (new villages) and abandonment of dying settlements
    timed("colonization", || step_colonization(history, world, game_data, rng));


    // 3. Opinion friction (border disputes, rivalries)
    timed("opinion_friction", || step_opinion_friction(history, rng));

    // 4. Peaceful diplomacy (treaties, alliances)
    timed("diplomacy_peaceful", || step_diplomacy_peaceful(history, rng));

    // 5. War declarations
    timed("diplomacy", || step_diplomacy(history, rng));

    // 5. Alliance obligations and treaty enforcement
    timed("alliance_obligations", || step_alliance_obligations(history, rng));

    // 5a. Active wars: battles
    timed("wars", || step_wars(history, world, rng));

    // 5.5. Active sieges: attrition, resolution
    timed("sieges", || step_sieges(history, rng));

    // 5.6. The Shadow spreads and strikes (no RNG draws)
    timed("shadow", || crate::history::shadow::step(history));

    // 6. Creature activity
    timed("creatures", || step_creatures(history, rng));
    if date.season == crate::seasons::Season::Spring {
        timed("broods", || step_broods(history, rng));
        timed("revivals", || step_revivals(history, game_data, rng));
    }

    // 7. Figure lifecycle (births, deaths, succession)
    timed("figures", || step_figures(history, game_data, rng));

    // 8. Artifact and monument creation
    timed("artifacts", || step_artifacts(history, rng));

    // 9. Trade route establishment
    timed("trade", || step_trade(history, world, rng));

    // 10. Religious events (conversion, schisms, sacrifice)
    timed("religion", || step_religion(history, rng));

    // 11. Natural events
    timed("natural_events", || step_natural_events(history, rng));

    // 12. Hero quests
    timed("quests", || step_quests(history, rng));

    // 13. Assassination & Intrigue
    timed("assassination", || step_assassination(history, game_data, rng));

    // 13. Artifact lifecycle (inheritance, loss, hoarding, destruction)
    timed("artifact_lifecycle", || step_artifact_lifecycle(history, rng));

    // 13. Wealth tick (income, trade revenue, war costs)
    timed("wealth_tick", || step_wealth_tick(history, rng));

    // 14. Ecology, once a year: land cover, wildlife and what people do to them.
    if date.season == crate::seasons::Season::Spring {
        timed("ecology", || crate::history::ecology::step_year(history, world));
    }

    // 15. Keep the state consistent: no war or siege outlives a people, every living people has a
    //     living ruler, and diplomatic stances match the wars actually being fought.
    timed("integrity", || step_integrity(history, game_data, rng));
    timed("people", || crate::history::people::step(history, game_data, rng));

    // 16. Advance date
    history.current_date = date.next();
}

/// End-of-step consistency pass. Every system kills figures, dissolves peoples and ends wars in
/// its own way; this is the one place that reconciles what follows from those:
/// - a war or siege with a dissolved people on either side ends;
/// - an active people whose ruler is dead (assassinated, executed, slain on a quest...) gets a
///   successor (only natural deaths used to trigger one);
/// - each pair's `War` stance matches the active wars between them, so a pair can't open a
///   second war while one runs, and allies called into an ended war are released.
fn step_integrity(history: &mut WorldHistory, game_data: &GameData, rng: &mut impl Rng) {
    let date = history.current_date;
    let active = |h: &WorldHistory, f: FactionId| h.factions.get(&f).map_or(false, |x| x.is_active());

    // Wars with a vanished side.
    let doomed: Vec<WarId> = history.wars.values()
        .filter(|w| w.is_active())
        .filter(|w| !w.aggressors.iter().any(|&f| active(history, f)) || !w.defenders.iter().any(|&f| active(history, f)))
        .map(|w| w.id)
        .collect();
    for wid in doomed {
        if let Some(w) = history.wars.get_mut(&wid) {
            let agg_alive = w.aggressors.iter().copied().find(|&f| history.factions.get(&f).map_or(false, |x| x.is_active()));
            let def_alive = w.defenders.iter().copied().find(|&f| history.factions.get(&f).map_or(false, |x| x.is_active()));
            w.end(date, agg_alive.or(def_alive));
        }
    }
    // Drop dissolved peoples from wars still running between other parties.
    let gone: Vec<FactionId> = history.factions.values().filter(|f| !f.is_active()).map(|f| f.id).collect();
    for w in history.wars.values_mut().filter(|w| w.is_active()) {
        w.aggressors.retain(|f| !gone.contains(f));
        w.defenders.retain(|f| !gone.contains(f));
    }

    // Sieges with a vanished side (sieges otherwise outlive their war on purpose).
    let stale: Vec<crate::history::SiegeId> = history.sieges.values()
        .filter(|s| s.is_active())
        .filter(|s| !active(history, s.attacker) || !active(history, s.defender))
        .map(|s| s.id)
        .collect();
    for sid in stale {
        if let Some(s) = history.sieges.get_mut(&sid) { s.end(date, false); }
    }

    // Rulerless peoples.
    let rulerless: Vec<(FactionId, FigureId)> = history.factions.values()
        .filter(|f| f.is_active())
        .filter_map(|f| {
            let lid = f.current_leader?;
            let alive = history.figures.get(&lid).map_or(false, |fig| fig.is_alive());
            (!alive).then_some((f.id, lid))
        })
        .collect();
    for (fid, dead) in rulerless {
        succeed(history, dead, fid, game_data, rng);
    }

    // Stances follow the wars.
    let mut warring: crate::history::det::HashMap<(FactionId, FactionId), WarId> = Default::default();
    for w in history.wars.values().filter(|w| w.is_active()) {
        for &a in &w.aggressors {
            for &d in &w.defenders {
                warring.insert((a, d), w.id);
                warring.insert((d, a), w.id);
            }
        }
    }
    let new_year = date.season == crate::seasons::Season::Spring;
    for f in history.factions.values_mut() {
        let me = f.id;
        for (&other, rel) in f.relations.iter_mut() {
            match warring.get(&(me, other)) {
                Some(&wid) if !rel.stance.is_at_war() => rel.declare_war(wid),
                None if rel.stance.is_at_war() => rel.make_peace(),
                // Grudges fade at peace, a point a year, unless new quarrels feed them.
                None if new_year => {
                    let baseline = ((rel.cultural_similarity - 0.5) * 50.0) as i32;
                    rel.adjust_opinion((baseline - rel.opinion).signum());
                }
                _ => {}
            }
        }
    }
}

fn step_population_growth(history: &mut WorldHistory) {
    let settlement_ids: Vec<SettlementId> = history.settlements.keys().copied().collect();
    for sid in settlement_ids {
        if let Some(settlement) = history.settlements.get_mut(&sid) {
            if !settlement.is_destroyed() {
                let old_pop = settlement.population;
                settlement.grow_population();
                let new_pop = settlement.population;

                // Update faction total
                let faction_id = settlement.faction;
                if let Some(faction) = history.factions.get_mut(&faction_id) {
                    faction.total_population = faction.total_population
                        .saturating_sub(old_pop)
                        .saturating_add(new_pop);
                }
            }
        }
    }
}

fn step_settlement_upgrades(history: &mut WorldHistory) {
    let settlement_ids: Vec<SettlementId> = history.settlements.keys().copied().collect();
    let date = history.current_date;
    let mut events_to_record = Vec::new();

    for sid in settlement_ids {
        if let Some(settlement) = history.settlements.get_mut(&sid) {
            let old_type = settlement.settlement_type;
            settlement.check_upgrade();
            if settlement.settlement_type != old_type {
                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::SettlementGrew,
                    date,
                    format!("{} grows into a {}", settlement.name, plain_words(&format!("{:?}", settlement.settlement_type))),
                    format!("{} has grown from a {} into a {}.",
                        settlement.name, plain_words(&format!("{:?}", old_type)), plain_words(&format!("{:?}", settlement.settlement_type))),
                )
                .at_location(settlement.location.0, settlement.location.1)
                .with_faction(settlement.faction)
                .with_participant(EntityId::Settlement(sid));
                events_to_record.push(event);
            }
        }
    }

    for event in events_to_record {
        history.chronicle.record(event);
    }
}

/// Generate opinion friction between neighboring factions.
/// This is the mechanism that creates rivalries and eventually wars.
fn step_opinion_friction(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    if faction_ids.len() < 2 { return; }

    // Sample faction pairs per step
    let pairs_to_check = (faction_ids.len() * 3).min(600);

    for _ in 0..pairs_to_check {
        let idx_a = rng.gen_range(0..faction_ids.len());
        let mut idx_b = rng.gen_range(0..faction_ids.len());
        if idx_a == idx_b { idx_b = (idx_a + 1) % faction_ids.len(); }
        let fid_a = faction_ids[idx_a];
        let fid_b = faction_ids[idx_b];

        // Check geographic proximity (settlements within ~45 tiles)
        let close = factions_are_neighbors(history, fid_a, fid_b, 45);
        if !close { continue; }

        // Cultural distance drives friction
        let cultural_sim = get_cultural_similarity(history, fid_a, fid_b);
        let xenophobia_a = get_faction_xenophobia(history, fid_a);
        let xenophobia_b = get_faction_xenophobia(history, fid_b);
        let avg_xenophobia = (xenophobia_a + xenophobia_b) / 2.0;

        // Friction = cultural distance * xenophobia
        let cultural_distance = 1.0 - cultural_sim;
        let friction = cultural_distance * avg_xenophobia;

        // Different-religion friction (stronger if either faction has HolyWar doctrine)
        let religion_friction = if !factions_share_religion(history, fid_a, fid_b) {
            let hw_a = faction_has_holy_war_doctrine(history, fid_a);
            let hw_b = faction_has_holy_war_doctrine(history, fid_b);
            if hw_a || hw_b { 0.6 } else { 0.3 }
        } else {
            0.0
        };

        // Resource envy: a neighbour holds metals, salt or farmland we lack.
        let (envy, envied) = resource_envy(history, fid_a, fid_b);
        let (envy_rev, envied_rev) = resource_envy(history, fid_b, fid_a);
        let (envy_best, envied_good) = if envy >= envy_rev { (envy, envied) } else { (envy_rev, envied_rev) };
        let envy_friction = 0.35 * (envy_best / 20.0).min(1.0);

        // Total opinion delta: negative (friction) or slightly positive (cultural affinity)
        let total_friction = friction + religion_friction + envy_friction;

        // Apply: 20% chance per checked pair per step to generate a friction event
        if rng.gen::<f32>() < 0.20 && total_friction > 0.12 {
            let delta = -(1.0 + total_friction * 4.0) as i32; // -1 to -5 per event

            if let Some(faction_a) = history.factions.get_mut(&fid_a) {
                let rel = faction_a.get_relation_mut(fid_b, cultural_sim);
                rel.adjust_opinion(delta);
            }
            if let Some(faction_b) = history.factions.get_mut(&fid_b) {
                let rel = faction_b.get_relation_mut(fid_a, cultural_sim);
                rel.adjust_opinion(delta);
            }
        }

        // Major incident: border clash, diplomatic insult, trade dispute (2% chance)
        if rng.gen::<f32>() < 0.02 && total_friction > 0.12 {
            let delta = -(rng.gen_range(12..30));

            if let Some(faction_a) = history.factions.get_mut(&fid_a) {
                let rel = faction_a.get_relation_mut(fid_b, cultural_sim);
                rel.adjust_opinion(delta);
            }
            if let Some(faction_b) = history.factions.get_mut(&fid_b) {
                let rel = faction_b.get_relation_mut(fid_a, cultural_sim);
                rel.adjust_opinion(delta);
            }

            let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
            let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();
            let incident_type: String = match (envied_good, rng.gen_range(0..4)) {
                (Some(r), 0 | 1) if envy_friction > 0.1 => format!("dispute over {}", crate::lore::resource_name(r)),
                (_, 0) => "border clash".to_string(),
                (_, 1) => "diplomatic insult".to_string(),
                (_, 2) => "trade dispute".to_string(),
                _ => "territorial encroachment".to_string(),
            };
            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::Raid,
                date,
                format!("{} between {} and {}", incident_type, name_a, name_b),
                format!("A {} has soured relations between {} and {}.", incident_type, name_a, name_b),
            )
            .with_faction(fid_a)
            .with_faction(fid_b)
            .with_consequence(Consequence::RelationChange(fid_a, fid_b, delta));
            history.chronicle.record(event);
        }
    }
}

/// Years after a war ends during which the same two peoples won't fight again.
const TRUCE_YEARS: u32 = 12;
/// Wars fought in this many recent years make a people weary of war.
const WEARINESS_YEARS: u32 = 20;

/// What peoples remember of their recent wars, read off `history.wars` once per step (nothing new
/// is stored, so saves don't change): a truce between a pair after their war ends, and weariness
/// that halves a people's appetite for war with each war it fought in the last 20 years. Without
/// it, a people at peace declared war again at once, and wars ran back to back until most peoples
/// were gone.
struct WarMemory {
    year: u32,
    last_ended: crate::history::det::HashMap<(FactionId, FactionId), u32>,
    recent: crate::history::det::HashMap<FactionId, u32>,
}

impl WarMemory {
    fn of(history: &WorldHistory) -> Self {
        let year = history.current_date.year;
        let mut last_ended: crate::history::det::HashMap<(FactionId, FactionId), u32> = Default::default();
        let mut recent: crate::history::det::HashMap<FactionId, u32> = Default::default();
        for w in history.wars.values() {
            let ended = w.ended.map(|d| d.year).unwrap_or(year);
            for &a in &w.aggressors {
                for &d in &w.defenders {
                    let key = if a < d { (a, d) } else { (d, a) };
                    let e = last_ended.entry(key).or_insert(0);
                    *e = (*e).max(ended);
                }
            }
            if ended + WEARINESS_YEARS >= year {
                for &f in w.aggressors.iter().chain(&w.defenders) { *recent.entry(f).or_default() += 1; }
            }
        }
        WarMemory { year, last_ended, recent }
    }

    /// Whether the two are still bound by the truce that followed their last war.
    fn truce(&self, a: FactionId, b: FactionId) -> bool {
        let key = if a < b { (a, b) } else { (b, a) };
        self.last_ended.get(&key).map_or(false, |&y| y + TRUCE_YEARS > self.year)
    }

    /// Appetite for war of a people: 1.0 rested, halved per recent war.
    fn appetite(&self, f: FactionId) -> f32 {
        0.5f32.powi(self.recent.get(&f).copied().unwrap_or(0).min(8) as i32)
    }

    /// Multiplier on the chance that `a` declares war on `b`.
    fn pace(&self, a: FactionId, b: FactionId) -> f32 {
        if self.truce(a, b) { 0.0 } else { self.appetite(a) }
    }
}

fn step_diplomacy(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let memory = WarMemory::of(history);
    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    if faction_ids.len() < 2 {
        return;
    }

    // Maximum active wars per faction to prevent war spam
    let max_wars_per_faction = 2;
    let war_chance = 0.004 * history.config.war_frequency;

    // Instead of O(n^2), sample random pairs AND check factions with existing hostile relations
    let pairs_to_check = (faction_ids.len() * 2).min(300);

    for _ in 0..pairs_to_check {
        let idx_a = rng.gen_range(0..faction_ids.len());
        let mut idx_b = rng.gen_range(0..faction_ids.len());
        if idx_a == idx_b { idx_b = (idx_a + 1) % faction_ids.len(); }
        let fid_a = faction_ids[idx_a];
        let fid_b = faction_ids[idx_b];

        // Skip if either faction at war limit
        let a_active_wars = history.factions.get(&fid_a)
            .map_or(0, |f| f.active_war_count());
        let b_active_wars = history.factions.get(&fid_b)
            .map_or(0, |f| f.active_war_count());
        if a_active_wars >= max_wars_per_faction || b_active_wars >= max_wars_per_faction {
            continue;
        }

        let already_at_war = history.factions.get(&fid_a)
            .map_or(false, |f| f.is_at_war_with(fid_b));
        if already_at_war { continue; }

        let opinion = history.factions.get(&fid_a)
            .and_then(|f| f.relations.get(&fid_b))
            .map(|r| r.opinion)
            .unwrap_or(0);

        // War threshold: base -30, but warlike leaders can go at -15
        let leader_war_incl = leader_personality(history, fid_a)
            .map(|p| p.war_inclination()).unwrap_or(0.5);
        let war_threshold = if leader_war_incl > 0.7 { -15 } else { -30 };

        if opinion >= war_threshold { continue; }

        // Require geographic proximity for war
        if !factions_are_neighbors(history, fid_a, fid_b, 60) { continue; }

        // Personality multiplier: 0.1x (pacifist) to 4.0x (warmonger)
        let personality_mult = Personality::score_to_multiplier(leader_war_incl, 0.1, 4.0);

        // Religion modifier: HolyWar +50%, Pacifism -70%
        let religion_war_mult = faction_religion_war_modifier(history, fid_a);

        // Same-religion factions are less likely to fight (-60%)
        let same_religion_mult = if factions_share_religion(history, fid_a, fid_b) {
            0.4
        } else {
            1.0
        };

        // Stronger opinion = higher war chance (opinion < -30 gives boost)
        let opinion_mult = 1.0 + ((-opinion as f32 - 30.0).max(0.0) / 50.0);

        let effective_war_chance = war_chance * personality_mult * religion_war_mult
            * same_religion_mult * opinion_mult * memory.pace(fid_a, fid_b);

        if rng.gen::<f32>() >= effective_war_chance {
            continue;
        }

        // Declare war
        let war_id = history.id_generators.next_war();
        let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
        let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();

        let leader_id_a = history.factions.get(&fid_a).and_then(|f| f.current_leader);
        let leader_p = leader_id_a
            .and_then(|lid| history.figures.get(&lid))
            .map(|fig| &fig.personality);
        let envy_here = resource_envy(history, fid_a, fid_b).0;
        let cause = if envy_here > 10.0 && rng.gen::<f32>() < 0.6 { WarCause::Resource } else { pick_war_cause(leader_p, rng) };
        let envied = resource_envy(history, fid_a, fid_b).1;

        let event_id = history.id_generators.next_event();
        let war_name = war_name(history, cause, fid_a, fid_b, envied);
        let mut war = War::new(war_id, war_name.clone(), fid_a, fid_b, date, cause);
        war.declaration_event = Some(event_id);
        history.wars.insert(war_id, war);

        if let Some(faction) = history.factions.get_mut(&fid_a) {
            faction.wars.push(war_id);
            let rel = faction.get_relation_mut(fid_b, 0.0);
            rel.declare_war(war_id);
        }
        if let Some(faction) = history.factions.get_mut(&fid_b) {
            faction.wars.push(war_id);
            let rel = faction.get_relation_mut(fid_a, 0.0);
            rel.declare_war(war_id);
        }

        // Use HolyWarDeclared if declaring faction has HolyWar and target has different religion
        let is_holy_war = faction_has_holy_war_doctrine(history, fid_a)
            && !factions_share_religion(history, fid_a, fid_b);
        let war_event_type = if is_holy_war {
            EventType::HolyWarDeclared
        } else {
            EventType::WarDeclared
        };
        let desc = if is_holy_war {
            format!("{} declared a holy war on {}.", name_a, name_b)
        } else {
            format!("{} declared war on {}.", name_a, name_b)
        };

        let mut event = Event::new(event_id, war_event_type, date, war_name, desc)
            .with_faction(fid_a)
            .with_faction(fid_b);
        if let Some(lid) = leader_id_a {
            event = event.with_participant(EntityId::Figure(lid));
        }
        history.chronicle.record(event);
    }

    // Also scan factions with existing hostile relations for war declarations
    for &fid_a in &faction_ids {
        let a_active_wars = history.factions.get(&fid_a)
            .map_or(0, |f| f.active_war_count());
        if a_active_wars >= max_wars_per_faction { continue; }

        // Find hostile or very negative relations
        let hostile_targets: Vec<(FactionId, i32)> = history.factions.get(&fid_a)
            .map(|f| {
                f.relations.iter()
                    .filter(|(_, r)| r.opinion < -30 && !r.stance.is_at_war())
                    .map(|(&fid, r)| (fid, r.opinion))
                    .collect()
            })
            .unwrap_or_default();

        for (fid_b, opinion) in hostile_targets {
            if !history.factions.get(&fid_b).map_or(false, |f| f.is_active()) { continue; }
            let b_active_wars = history.factions.get(&fid_b)
                .map_or(0, |f| f.active_war_count());
            if b_active_wars >= max_wars_per_faction { continue; }
            if history.factions.get(&fid_a).map_or(false, |f| f.is_at_war_with(fid_b)) { continue; }

            let leader_war_incl = leader_personality(history, fid_a)
                .map(|p| p.war_inclination()).unwrap_or(0.5);
            let personality_mult = Personality::score_to_multiplier(leader_war_incl, 0.1, 4.0);
            let religion_war_mult = faction_religion_war_modifier(history, fid_a);
            let opinion_mult = 1.0 + ((-opinion as f32 - 30.0).max(0.0) / 50.0);

            // Hostile-relation path uses higher base chance (these factions already hate each other)
            let hostile_war_chance = 0.018 * history.config.war_frequency;
            let effective = hostile_war_chance * personality_mult * religion_war_mult * opinion_mult
                * memory.pace(fid_a, fid_b);
            if rng.gen::<f32>() >= effective { continue; }

            let war_id = history.id_generators.next_war();
            let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
            let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();

            let leader_id_a = history.factions.get(&fid_a).and_then(|f| f.current_leader);
            let leader_p = leader_id_a
                .and_then(|lid| history.figures.get(&lid))
                .map(|fig| &fig.personality);
            let cause = pick_war_cause(leader_p, rng);

            let event_id = history.id_generators.next_event();
            let war_name = war_name(history, cause, fid_a, fid_b, None);
            let mut war = War::new(war_id, war_name.clone(), fid_a, fid_b, date, cause);
            war.declaration_event = Some(event_id);
            history.wars.insert(war_id, war);

            if let Some(faction) = history.factions.get_mut(&fid_a) {
                faction.wars.push(war_id);
                let rel = faction.get_relation_mut(fid_b, 0.0);
                rel.declare_war(war_id);
            }
            if let Some(faction) = history.factions.get_mut(&fid_b) {
                faction.wars.push(war_id);
                let rel = faction.get_relation_mut(fid_a, 0.0);
                rel.declare_war(war_id);
            }

            let is_holy_war = faction_has_holy_war_doctrine(history, fid_a)
                && !factions_share_religion(history, fid_a, fid_b);
            let war_event_type = if is_holy_war {
                EventType::HolyWarDeclared
            } else {
                EventType::WarDeclared
            };
            let desc = if is_holy_war {
                format!("{} declared a holy war on {}.", name_a, name_b)
            } else {
                format!("{} declared war on {}.", name_a, name_b)
            };

            let mut event = Event::new(event_id, war_event_type, date, war_name, desc)
                .with_faction(fid_a)
                .with_faction(fid_b);
            if let Some(lid) = leader_id_a {
                event = event.with_participant(EntityId::Figure(lid));
            }
            history.chronicle.record(event);
        }
    }

    // Warmongering: highly aggressive leaders may start unprovoked wars on neighbors
    for &fid_a in &faction_ids {
        let war_incl = leader_personality(history, fid_a)
            .map(|p| p.war_inclination()).unwrap_or(0.5);
        // Only leaders with war_inclination > 0.6 can warmonger
        if war_incl < 0.6 { continue; }

        let a_active_wars = history.factions.get(&fid_a)
            .map_or(0, |f| f.active_war_count());
        if a_active_wars >= max_wars_per_faction { continue; }

        // Chance scales with excess war_inclination: (incl - 0.5) * 0.004
        let warmonger_chance = (war_incl - 0.5) * 0.004 * history.config.war_frequency;
        // Religion modifier
        let rel_mult = faction_religion_war_modifier(history, fid_a);
        if rng.gen::<f32>() >= warmonger_chance * rel_mult * memory.appetite(fid_a) { continue; }

        // Pick a random neighbor to attack (even without deep hatred)
        let neighbor_idx = rng.gen_range(0..faction_ids.len());
        let fid_b = faction_ids[neighbor_idx];
        if fid_a == fid_b { continue; }
        if !factions_are_neighbors(history, fid_a, fid_b, 45) { continue; }
        if history.factions.get(&fid_a).map_or(false, |f| f.is_at_war_with(fid_b)) { continue; }
        if memory.truce(fid_a, fid_b) { continue; }
        let b_active_wars = history.factions.get(&fid_b)
            .map_or(0, |f| f.active_war_count());
        if b_active_wars >= max_wars_per_faction { continue; }

        // Declare unprovoked war
        let event_id = history.id_generators.next_event();
        let war_id = history.id_generators.next_war();
        let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
        let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();

        let leader_id_a = history.factions.get(&fid_a).and_then(|f| f.current_leader);
        let leader_p = leader_id_a
            .and_then(|lid| history.figures.get(&lid))
            .map(|fig| &fig.personality);
        let cause = pick_war_cause(leader_p, rng);

        let war_name = war_name(history, cause, fid_a, fid_b, None);
        let mut war = War::new(war_id, war_name.clone(), fid_a, fid_b, date, cause);
        war.declaration_event = Some(event_id);
        history.wars.insert(war_id, war);

        if let Some(faction) = history.factions.get_mut(&fid_a) {
            faction.wars.push(war_id);
            let rel = faction.get_relation_mut(fid_b, 0.0);
            rel.declare_war(war_id);
        }
        if let Some(faction) = history.factions.get_mut(&fid_b) {
            faction.wars.push(war_id);
            let rel = faction.get_relation_mut(fid_a, 0.0);
            rel.declare_war(war_id);
        }

        let is_holy_war = faction_has_holy_war_doctrine(history, fid_a)
            && !factions_share_religion(history, fid_a, fid_b);
        let war_event_type = if is_holy_war {
            EventType::HolyWarDeclared
        } else {
            EventType::WarDeclared
        };
        let desc = if is_holy_war {
            format!("{} launched a holy crusade against {}.", name_a, name_b)
        } else {
            format!("{} launched an unprovoked attack on {}.", name_a, name_b)
        };

        let mut event = Event::new(event_id, war_event_type, date, war_name, desc)
            .with_faction(fid_a)
            .with_faction(fid_b);
        if let Some(lid) = leader_id_a {
            event = event.with_participant(EntityId::Figure(lid));
        }
        history.chronicle.record(event);
    }

    // Holy Crusade pathway: HolyWar factions specifically target different-religion neighbors
    // This is independent of leader personality — it's a doctrinal compulsion
    for &fid_a in &faction_ids {
        if !faction_has_holy_war_doctrine(history, fid_a) { continue; }

        let a_active_wars = history.factions.get(&fid_a)
            .map_or(0, |f| f.active_war_count());
        if a_active_wars >= max_wars_per_faction { continue; }

        // 0.002 per step — doctrine-driven, not personality-driven
        let crusade_chance = 0.002 * history.config.war_frequency;
        if rng.gen::<f32>() >= crusade_chance * memory.appetite(fid_a) { continue; }

        // Find a different-religion neighbor to crusade against
        let neighbor_idx = rng.gen_range(0..faction_ids.len());
        let fid_b = faction_ids[neighbor_idx];
        if fid_a == fid_b { continue; }
        if factions_share_religion(history, fid_a, fid_b) { continue; }
        if !factions_are_neighbors(history, fid_a, fid_b, 50) { continue; }
        if history.factions.get(&fid_a).map_or(false, |f| f.is_at_war_with(fid_b)) { continue; }
        if memory.truce(fid_a, fid_b) { continue; }
        let b_active_wars = history.factions.get(&fid_b)
            .map_or(0, |f| f.active_war_count());
        if b_active_wars >= max_wars_per_faction { continue; }

        let event_id = history.id_generators.next_event();
        let war_id = history.id_generators.next_war();
        let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
        let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();
        let leader_id_a = history.factions.get(&fid_a).and_then(|f| f.current_leader);

        let war_name = war_name(history, WarCause::HolyWar, fid_a, fid_b, None);
        let mut war = War::new(war_id, war_name.clone(), fid_a, fid_b, date, WarCause::HolyWar);
        war.declaration_event = Some(event_id);
        history.wars.insert(war_id, war);

        if let Some(faction) = history.factions.get_mut(&fid_a) {
            faction.wars.push(war_id);
            let rel = faction.get_relation_mut(fid_b, 0.0);
            rel.declare_war(war_id);
        }
        if let Some(faction) = history.factions.get_mut(&fid_b) {
            faction.wars.push(war_id);
            let rel = faction.get_relation_mut(fid_a, 0.0);
            rel.declare_war(war_id);
        }

        let mut event = Event::new(
            event_id,
            EventType::HolyWarDeclared,
            date,
            war_name,
            format!("{} launched a holy crusade against the infidels of {}.", name_a, name_b),
        )
        .with_faction(fid_a)
        .with_faction(fid_b);
        if let Some(lid) = leader_id_a {
            event = event.with_participant(EntityId::Figure(lid));
        }
        history.chronicle.record(event);
    }
}

/// The defender's town nearest the attacker: where a war's battles are fought.
fn battle_town(history: &WorldHistory, agg: FactionId, def: FactionId) -> Option<SettlementId> {
    let w = history.tile_history.width as i64;
    let alive = |f: FactionId| history.settlements.values().filter(move |s| s.faction == f && !s.is_destroyed()).map(|s| (s.id, s.location));
    let mut best: Option<(i64, SettlementId)> = None;
    for (did, d) in alive(def) {
        for (_, a) in alive(agg) {
            let mut dx = (d.0 as i64 - a.0 as i64).abs();
            dx = dx.min(w - dx);
            let dy = d.1 as i64 - a.1 as i64;
            let dist = dx * dx + dy * dy;
            if best.map_or(true, |b| (dist, did) < (b.0, b.1)) { best = Some((dist, did)); }
        }
    }
    best.map(|b| b.1)
}

/// What the ground at a battle gives the defender, and what the battle is called after.
#[derive(Clone, Copy, PartialEq)]
enum Ground { Walls, Ford, Pass, Wood, Field }

fn battle_ground(history: &WorldHistory, world: &WorldData, town: SettlementId) -> Ground {
    use crate::history::civilizations::settlement::WallLevel;
    let Some(t) = history.settlements.get(&town) else { return Ground::Field };
    let (x, y) = t.location;
    if matches!(t.walls, WallLevel::StoneWall | WallLevel::Fortified | WallLevel::Citadel) { return Ground::Walls; }
    let (w, h) = (world.width, world.height);
    let near = |dx: i64, dy: i64| (((x as i64 + dx).rem_euclid(w as i64)) as usize, (y as i64 + dy).clamp(0, h as i64 - 1) as usize);
    let around: Vec<(usize, usize)> = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| near(dx, dy)).collect();
    if world.river_network.as_ref().map_or(false, |rn| around.iter().any(|&(i, j)| rn.has_significant_flow(i, j))) { return Ground::Ford; }
    let hs: Vec<f32> = around.iter().map(|&(i, j)| *world.heightmap.get(i, j)).collect();
    let relief = hs.iter().cloned().fold(f32::MIN, f32::max) - hs.iter().cloned().fold(f32::MAX, f32::min);
    if *world.heightmap.get(x, y) > 1200.0 || relief > 900.0 { return Ground::Pass; }
    let b = format!("{:?}", world.biomes.get(x, y));
    if b.contains("Forest") || b.contains("Jungle") || b.contains("Taiga") { return Ground::Wood; }
    Ground::Field
}

impl Ground {
    /// The defender's multiplier.
    fn hold(self) -> f32 { match self { Ground::Walls => 1.6, Ground::Pass => 1.4, Ground::Ford => 1.25, Ground::Wood => 1.1, Ground::Field => 1.0 } }
    fn battle_name(self, town: &str, n: u64) -> String {
        match self {
            Ground::Walls => format!("the Battle before the Walls of {}", town),
            Ground::Ford => format!("the Battle of {} Ford", town),
            Ground::Pass => format!("the Battle of the {} Pass", town),
            Ground::Wood => if n % 2 == 0 { format!("the Battle of {} Wood", town) } else { format!("the Battle in the Woods of {}", town) },
            Ground::Field => match n % 3 { 0 => format!("the Battle of {}", town), 1 => format!("the Battle of the Fields of {}", town), _ => format!("the Battle of {} Field", town) },
        }
    }
}

/// A war's name from its cause and where it is fought ("The Salt War", "The Conquest of
/// Galost", "The Second War of the Mirrorwish Succession"); repeated names get an ordinal.
fn war_name(history: &WorldHistory, cause: WarCause, a: FactionId, b: FactionId, envied: Option<ResourceType>) -> String {
    let town = |s: Option<SettlementId>| s.and_then(|s| history.settlements.get(&s)).map(|t| t.name.clone());
    let place = town(battle_town(history, a, b)).or_else(|| town(history.factions.get(&b).and_then(|f| f.capital))).unwrap_or_else(|| "the Marches".into());
    let seat = |f: FactionId| town(history.factions.get(&f).and_then(|x| x.capital)).unwrap_or_else(|| place.clone());
    let stem = match cause {
        WarCause::Resource => match envied {
            Some(r) => format!("{} War", resource_word(r)),
            None => format!("War for the Wealth of {}", place),
        },
        WarCause::Territorial => format!("War for {}", place),
        WarCause::Conquest => format!("Conquest of {}", place),
        WarCause::Succession => format!("War of the {} Succession", seat(b)),
        WarCause::Religious => format!("War of the Altars at {}", place),
        WarCause::HolyWar => format!("Holy War on {}", seat(b)),
        WarCause::Revenge => format!("Vengeance of {}", seat(a)),
        WarCause::Independence => format!("War of {}'s Independence", place),
        WarCause::DefensivePact => format!("{} War", place),
    };
    let before = history.wars.values().filter(|w| w.name.ends_with(&stem)).count();
    const ORD: [&str; 9] = ["Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth", "Ninth", "Tenth"];
    match before {
        0 => format!("The {}", stem),
        n if n <= ORD.len() => format!("The {} {}", ORD[n - 1], stem),
        n => format!("The {}th {}", n + 1, stem),
    }
}

fn resource_word(r: ResourceType) -> &'static str {
    match r {
        ResourceType::Food => "Grain", ResourceType::Wood => "Timber", ResourceType::Stone => "Quarry",
        ResourceType::Iron => "Iron", ResourceType::Copper => "Copper", ResourceType::Gold => "Gold",
        ResourceType::Silver => "Silver", ResourceType::Mithril => "Mithril", ResourceType::Adamantine => "Adamant",
        ResourceType::Gems => "Jewel", ResourceType::Diamonds => "Diamond", ResourceType::Rubies => "Ruby",
        ResourceType::Emeralds => "Emerald", ResourceType::Spices => "Spice", ResourceType::Silk => "Silk",
        ResourceType::Wine => "Wine", ResourceType::Salt => "Salt", ResourceType::Herbs => "Herb",
        ResourceType::MagicalComponents => "Spellstone", ResourceType::AncientRelics => "Relic",
        ResourceType::DragonScale => "Dragonscale", ResourceType::MonsterBones => "Bone", ResourceType::Ichor => "Ichor",
        ResourceType::Coal => "Coal", ResourceType::Tin => "Tin", ResourceType::Fish => "Fishing",
    }
}

/// An enum's name as running text: "MagicalComponents" -> "magical components".
pub(crate) fn plain_words(debug: &str) -> String {
    let mut out = String::new();
    for (i, c) in debug.chars().enumerate() {
        if c.is_uppercase() && i > 0 { out.push(' '); }
        out.extend(c.to_lowercase());
    }
    out
}

/// A people whose seat its enemy now holds.
fn lost_seat(history: &WorldHistory, f: FactionId, to: FactionId) -> bool {
    history.factions.get(&f).and_then(|x| x.capital).and_then(|c| history.settlements.get(&c))
        .map_or(false, |t| !t.is_destroyed() && t.faction == to)
}

/// Who leads a people's host: the captain of its town nearest the field, else its ruler.
fn commander(history: &WorldHistory, f: FactionId, at: (usize, usize)) -> Option<FigureId> {
    use crate::history::people::Role;
    let people = history.people.as_ref();
    let mut best: Option<(usize, FigureId)> = None;
    if let Some(p) = people {
        for (fig, town) in &p.home {
            if p.role.get(fig) != Some(&Role::Captain) { continue; }
            let Some(x) = history.figures.get(fig) else { continue };
            if !x.is_alive() || x.faction != Some(f) { continue; }
            let Some(t) = history.settlements.get(town) else { continue };
            let d = t.location.0.abs_diff(at.0) + t.location.1.abs_diff(at.1);
            if best.map_or(true, |b| (d, *fig) < b) { best = Some((d, *fig)); }
        }
    }
    best.map(|b| b.1).or_else(|| history.factions.get(&f).and_then(|x| x.current_leader).filter(|l| history.figures.get(l).map_or(false, |x| x.is_alive())))
}

/// A commander's gift for war, fixed for life (0.8-1.25; 1 for a host without one).
fn talent(c: Option<FigureId>) -> f32 {
    c.map_or(1.0, |f| {
        let mut x = f.0 as u64 ^ 0x9E37_79B9_7F4A_7C15;
        x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 29;
        0.8 + 0.45 * (x % 1000) as f32 / 1000.0
    })
}

/// A people's field army: about one in twenty-five souls, more for a martial people.
fn army_size(history: &WorldHistory, f: FactionId) -> u32 {
    history.factions.get(&f).map_or(50, |x| ((x.total_population / 25) as f32 * (0.6 + (x.military_strength as f32 / 400.0).min(1.4))) as u32).max(40)
}

fn step_wars(history: &mut WorldHistory, world: &WorldData, rng: &mut impl Rng) {
    let date = history.current_date;
    let active_war_ids: Vec<WarId> = history.wars.keys()
        .copied()
        .filter(|id| history.wars.get(id).map_or(false, |w| w.is_active()))
        .collect();

    for war_id in active_war_ids {
        let (agg, def) = {
            let Some(war) = history.wars.get(&war_id) else { continue };
            (*war.aggressors.first().unwrap_or(&FactionId(0)), *war.defenders.first().unwrap_or(&FactionId(0)))
        };
        // Set when a battle decides the war outright (a ruler slain in the field).
        let mut decided: Option<FactionId> = None;
        // Battle chance per season
        if rng.gen::<f32>() < 0.15 {
            let town = battle_town(history, agg, def);
            let ground = town.map_or(Ground::Field, |t| battle_ground(history, world, t));
            let site = town.and_then(|t| history.settlements.get(&t)).map(|t| t.location);
            let town_name = town.and_then(|t| history.settlements.get(&t)).map(|t| t.name.clone()).unwrap_or_else(|| "the Marches".into());
            let at = site.unwrap_or((0, 0));
            let (cmd_a, cmd_d) = (commander(history, agg, at), commander(history, def, at));
            let (army_a, army_d) = (army_size(history, agg), army_size(history, def));
            // Strength: numbers (damped: a host ten times the size is about twice as strong, so
            // small peoples win some), who leads it, a roll for the day, the ground for the defender.
            let force_a = (army_a as f32).powf(0.35) * talent(cmd_a) * rng.gen_range(0.5..1.5);
            let force_d = (army_d as f32).powf(0.35) * talent(cmd_d) * rng.gen_range(0.5..1.5) * ground.hold();
            let agg_won = force_a > force_d;
            let margin = if agg_won { force_a / force_d.max(1.0) } else { force_d / force_a.max(1.0) };
            let (win_army, lose_army) = if agg_won { (army_a, army_d) } else { (army_d, army_a) };
            let lose_losses = ((lose_army as f32) * rng.gen_range(0.05..0.15) * margin.min(2.0)).max(5.0) as u32;
            let win_losses = ((win_army as f32) * rng.gen_range(0.02..0.06)).max(2.0) as u32;
            let (agg_losses, def_losses) = if agg_won { (win_losses, lose_losses) } else { (lose_losses, win_losses) };

            if let Some(war) = history.wars.get_mut(&war_id) {
                war.casualties.aggressor_losses += agg_losses;
                war.casualties.defender_losses += def_losses;
            }
            if let Some(faction) = history.factions.get_mut(&agg) {
                faction.total_population = faction.total_population.saturating_sub(agg_losses);
            }
            if let Some(faction) = history.factions.get_mut(&def) {
                faction.total_population = faction.total_population.saturating_sub(def_losses);
            }

            let agg_name = history.factions.get(&agg).map(|f| f.name.clone()).unwrap_or_default();
            let def_name = history.factions.get(&def).map(|f| f.name.clone()).unwrap_or_default();
            let fig_name = |c: Option<FigureId>| c.and_then(|c| history.figures.get(&c)).map(|x| x.full_name());
            let (win_fac, lose_fac) = if agg_won { (agg, def) } else { (def, agg) };
            let (win_cmd, lose_cmd) = if agg_won { (cmd_a, cmd_d) } else { (cmd_d, cmd_a) };
            let (win_name, lose_name) = if agg_won { (agg_name.clone(), def_name.clone()) } else { (def_name.clone(), agg_name.clone()) };
            let battle = ground.battle_name(&town_name, war_id.0 as u64 + history.wars.get(&war_id).map_or(0, |w| w.battles.len() as u64));
            let mut title = battle.clone();
            if let Some(c) = title.get_mut(0..1) { c.make_ascii_uppercase(); }

            // The commanders' fates: the beaten one may fall, the victor of a rout earns a name.
            let lose_cmd_dies = lose_cmd.is_some() && rng.gen::<f32>() < 0.12 + 0.12 * (margin - 1.0).min(1.0);
            let ruler_slain = lose_cmd_dies && history.factions.get(&lose_fac).and_then(|f| f.current_leader) == lose_cmd;
            let mut text = match (fig_name(win_cmd), fig_name(lose_cmd)) {
                (Some(w), Some(l)) => format!("{} of {} beat {} of {} in {}", w, win_name, l, lose_name, battle),
                (Some(w), None) => format!("{} led {} to victory over {} in {}", w, win_name, lose_name, battle),
                (None, Some(l)) => format!("{} beat {}'s host under {} in {}", win_name, lose_name, l, battle),
                (None, None) => format!("{} beat {} in {}", win_name, lose_name, battle),
            };
            text.push_str(&format!(" ({} of {} fell, {} of {}).", lose_losses, lose_army, win_losses, win_army));
            if ground != Ground::Field && !agg_won {
                text.push_str(match ground { Ground::Walls => " The walls held.", Ground::Ford => " The ford was held.", Ground::Pass => " The pass was held.", _ => " The woods hid the defenders." });
            }
            if lose_cmd_dies {
                if let Some(l) = fig_name(lose_cmd) { text.push_str(&format!(" {} died on the field.", l)); }
            } else if let Some(l) = fig_name(lose_cmd) {
                if margin > 1.3 { text.push_str(&format!(" {} escaped and swore to return.", l)); }
            }
            let win_epithet = win_cmd.filter(|c| margin > 1.6 && history.figures.get(c).map_or(false, |x| x.epithet.is_none()));
            // Defenders who hold their town are its shield; attackers who take the field its
            // victor, hammer or scourge; a name already borne goes to the next.
            let epithet = win_epithet.and_then(|_| {
                let forms: &[&str] = if agg_won { &["the Victor of", "the Hammer of", "the Scourge of", "the Bane of"] } else { &["the Shield of", "the Wall of", "the Warden of", "the Defender of"] };
                forms.iter().map(|f| format!("{} {}", f, town_name))
                    .find(|e| !history.figures.values().any(|x| x.epithet.as_deref() == Some(e.as_str())))
            });
            if let (Some(ep), Some(w)) = (&epithet, fig_name(win_cmd)) {
                text.push_str(&format!(" {} was called {} after.", w, ep));
            }

            let event_id = history.id_generators.next_event();
            let declaration_evt = history.wars.get(&war_id).and_then(|w| w.declaration_event);
            let mut event = Event::new(event_id, EventType::BattleFought, date, title, text)
                .with_faction(agg)
                .with_faction(def);
            for c in [win_cmd, lose_cmd].into_iter().flatten() { event = event.with_participant(EntityId::Figure(c)); }
            if let Some(t) = town { event = event.with_participant(EntityId::Settlement(t)); }
            if let Some(decl_id) = declaration_evt {
                event = event.caused_by(decl_id);
            }
            if let Some((x, y)) = site {
                event = event.at_location(x, y);
                history.tile_history.record_event(x, y, event_id);
            }
            if let Some(war) = history.wars.get_mut(&war_id) {
                war.battles.push(event_id);
            }
            history.chronicle.record(event);

            if lose_cmd_dies {
                if let Some(c) = lose_cmd.and_then(|c| history.figures.get_mut(&c)) { c.kill(date, crate::history::entities::traits::DeathCause::Battle); }
                if let (Some(w), Some(l)) = (win_cmd, lose_cmd) {
                    if let Some(x) = history.figures.get_mut(&w) { x.kills.push(EntityId::Figure(l)); }
                }
            } else if lose_cmd.is_some() && margin > 1.3 {
                // A beaten commander who lived carries the grudge home.
                if let Some(f) = history.factions.get_mut(&lose_fac) { f.get_relation_mut(win_fac, 0.0).adjust_opinion(-12); }
            }
            if let (Some(w), Some(ep)) = (win_epithet, epithet) {
                if let Some(x) = history.figures.get_mut(&w) { x.epithet = Some(ep); }
            }
            if ruler_slain { decided = Some(win_fac); }
        }

        // A war ends when a ruler falls in battle, when a side's seat is lost, or by exhaustion
        // (which builds each year: most wars end in 3-10 years, none outlast 20).
        if decided.is_none() {
            if lost_seat(history, def, agg) { decided = Some(agg); }
            else if lost_seat(history, agg, def) { decided = Some(def); }
        }
        let should_end = decided.is_some() || {
            let war = match history.wars.get(&war_id) {
                Some(w) => w,
                None => continue,
            };
            let duration = date.year.saturating_sub(war.started.year);
            duration >= 20 || (duration >= 1 && rng.gen::<f32>() < 0.010 + 0.005 * duration as f32)
        };

        if should_end {
            let (agg, def) = {
                let war = history.wars.get(&war_id).unwrap();
                let agg = *war.aggressors.first().unwrap_or(&FactionId(0));
                let def = *war.defenders.first().unwrap_or(&FactionId(0));
                (agg, def)
            };

            // Determine victor based on total casualties (fewer losses = winner)
            let agg_losses = history.wars.get(&war_id)
                .map(|w| w.casualties.aggressor_losses).unwrap_or(0);
            let def_losses = history.wars.get(&war_id)
                .map(|w| w.casualties.defender_losses).unwrap_or(0);
            let victor = decided.or_else(|| {
                let (ra, rd) = (agg_losses as f32 / army_size(history, agg) as f32, def_losses as f32 / army_size(history, def) as f32);
                Some(if ra <= rd { agg } else { def })
            });
            let loser = if victor == Some(agg) { def } else { agg };

            if let Some(war) = history.wars.get_mut(&war_id) {
                war.end(date, victor);
            }

            // Normalize relations
            if let Some(faction) = history.factions.get_mut(&agg) {
                let rel = faction.get_relation_mut(def, 0.0);
                rel.make_peace();
            }
            if let Some(faction) = history.factions.get_mut(&def) {
                let rel = faction.get_relation_mut(agg, 0.0);
                rel.make_peace();
            }

            let agg_name = history.factions.get(&agg).map(|f| f.name.clone()).unwrap_or_default();
            let def_name = history.factions.get(&def).map(|f| f.name.clone()).unwrap_or_default();
            let victor_name = victor.and_then(|v| history.factions.get(&v).map(|f| f.name.clone()))
                .unwrap_or_else(|| "none".to_string());
            let war_title = history.wars.get(&war_id).map(|w| w.name.clone()).unwrap_or_default();
            let years = history.wars.get(&war_id).map_or(0, |w| date.year.saturating_sub(w.started.year));
            let why = match decided {
                Some(v) if lost_seat(history, loser, v) => format!(", having taken the seat of {}", history.factions.get(&loser).map(|f| f.name.clone()).unwrap_or_default()),
                Some(_) => ", its foe's ruler slain in the field".to_string(),
                None => String::new(),
            };

            // War conquest: victor initiates sieges instead of instant transfer
            // Settlement transfers now happen through the siege system
            if let Some(victor_id) = victor {
                let total_casualties = agg_losses + def_losses;
                let loser_casualty_ratio = if victor == Some(agg) {
                    def_losses as f32 / total_casualties.max(1) as f32
                } else {
                    agg_losses as f32 / total_casualties.max(1) as f32
                };
                // Battles are lopsided (the beaten side loses several times more), so the loser
                // usually carries 0.6-0.8 of the dead; only a rout takes two towns.
                let conquest_chance = 0.55 + loser_casualty_ratio * 0.3;
                let settlements_to_take = if loser_casualty_ratio > 0.85 { 2 } else { 1 };

                if rng.gen::<f32>() < conquest_chance {
                    for _ in 0..settlements_to_take {
                        let loser_settlements: Vec<SettlementId> = history.factions.get(&loser)
                            .map(|f| f.settlements.clone())
                            .unwrap_or_default();

                        // Pick a settlement not already under siege
                        let already_sieged: Vec<SettlementId> = history.sieges.values()
                            .filter(|s| s.is_active())
                            .map(|s| s.target)
                            .collect();

                        if let Some(&sid_to_siege) = loser_settlements.iter()
                            .find(|&&sid| {
                                !already_sieged.contains(&sid)
                                && history.factions.get(&loser).and_then(|f| f.capital) != Some(sid)
                            })
                            .or_else(|| loser_settlements.iter()
                                .find(|&&sid| !already_sieged.contains(&sid)))
                        {
                            let attacker_str = history.factions.get(&victor_id)
                                .map(|f| f.military_strength).unwrap_or(100);
                            let defender_str = history.settlements.get(&sid_to_siege)
                                .map(|s| s.defense_strength()).unwrap_or(100);

                            let siege_id = history.id_generators.next_siege();
                            let siege = crate::history::civilizations::military::Siege::new(
                                siege_id, war_id, victor_id, loser,
                                sid_to_siege, date,
                                attacker_str, defender_str,
                            );
                            history.sieges.insert(siege_id, siege);

                            let target_name = history.settlements.get(&sid_to_siege)
                                .map(|s| s.name.clone()).unwrap_or_default();
                            let att_name = history.factions.get(&victor_id)
                                .map(|f| f.name.clone()).unwrap_or_default();

                            let event_id = history.id_generators.next_event();
                            let event = Event::new(
                                event_id,
                                EventType::SiegeBegun,
                                date,
                                format!("Siege of {}", target_name),
                                format!("{} laid siege to {}.", att_name, target_name),
                            )
                            .with_faction(victor_id)
                            .with_faction(loser)
                            .with_participant(EntityId::Settlement(sid_to_siege));
                            // The siege is the war's doing.
                            let event = match history.wars.get(&war_id).and_then(|w| w.declaration_event) {
                                Some(decl) => event.caused_by(decl),
                                None => event,
                            };
                            if let Some(war) = history.wars.get_mut(&war_id) {
                                war.sieges.push(event_id);
                            }
                            if let Some(siege) = history.sieges.get_mut(&siege_id) {
                                siege.begin_event = Some(event_id);
                            }
                            history.chronicle.record(event);
                        }
                    }
                }
            }

            // Dissolve loser if they lost all settlements
            let loser_settlement_count = history.factions.get(&loser)
                .map(|f| f.settlements.len()).unwrap_or(0);
            let loser_active = history.factions.get(&loser).map_or(false, |f| f.is_active());
            if loser_settlement_count == 0 && loser_active {
                if let Some(loser_f) = history.factions.get_mut(&loser) {
                    loser_f.dissolve(date);
                }
                let loser_name = history.factions.get(&loser)
                    .map(|f| f.name.clone()).unwrap_or_default();
                let event_id = history.id_generators.next_event();
                let declaration_evt = history.wars.get(&war_id)
                    .and_then(|w| w.declaration_event);
                let mut event = Event::new(
                    event_id,
                    EventType::FactionDestroyed,
                    date,
                    format!("{} destroyed", loser_name),
                    format!("{} has been destroyed after losing the war.", loser_name),
                )
                .with_faction(loser);
                if let Some(decl_id) = declaration_evt {
                    event = event.caused_by(decl_id);
                }
                history.chronicle.record(event);
            }

            let event_id = history.id_generators.next_event();
            let declaration_evt = history.wars.get(&war_id)
                .and_then(|w| w.declaration_event);
            let mut event = Event::new(
                event_id,
                EventType::WarEnded,
                date,
                format!("End of {}", war_title),
                format!("{} ended after {} between {} and {}: {} prevailed{}.", war_title,
                    match years { 0 => "less than a year".to_string(), 1 => "a year".to_string(), n => format!("{} years", n) },
                    agg_name, def_name, victor_name, why),
            )
            .with_faction(agg)
            .with_faction(def);
            if let Some(decl_id) = declaration_evt {
                event = event.caused_by(decl_id);
            }
            history.chronicle.record(event);
        }
    }
}

fn step_creatures(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    // Legendary creature raids
    let creature_ids: Vec<LegendaryCreatureId> = history.legendary_creatures.keys()
        .copied()
        .filter(|id| history.legendary_creatures.get(id).map_or(false, |c| c.is_alive()))
        .collect();

    let settlement_locations: Vec<(SettlementId, (usize, usize), FactionId)> = history.settlements.values()
        .filter(|s| !s.is_destroyed())
        .map(|s| (s.id, s.location, s.faction))
        .collect();

    for cid in creature_ids {
        let raid_chance = 0.01 * history.config.monster_activity;
        if rng.gen::<f32>() >= raid_chance {
            continue;
        }

        let creature_loc = history.legendary_creatures.get(&cid)
            .and_then(|c| c.lair_location);
        let creature_name = history.legendary_creatures.get(&cid)
            .map(|c| c.full_name())
            .unwrap_or_default();

        if let Some((cx, cy)) = creature_loc {
            // Find nearest settlement within range
            let mut closest: Option<(SettlementId, FactionId, i64)> = None;
            for &(sid, (sx, sy), fid) in &settlement_locations {
                let dx = cx as i64 - sx as i64;
                let dy = cy as i64 - sy as i64;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < 400 { // Within ~20 tiles
                    match closest {
                        None => closest = Some((sid, fid, dist_sq)),
                        Some((_, _, d)) if dist_sq < d => closest = Some((sid, fid, dist_sq)),
                        _ => {}
                    }
                }
            }

            if let Some((sid, fid, _)) = closest {
                let settlement_name = history.settlements.get(&sid)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();

                // Damage settlement
                let losses = scaled_loss(history, sid, rng.gen_range(10..200), rng);
                if let Some(settlement) = history.settlements.get_mut(&sid) {
                    settlement.population = settlement.population.saturating_sub(losses);
                }
                if let Some(faction) = history.factions.get_mut(&fid) {
                    faction.total_population = faction.total_population.saturating_sub(losses);
                }

                let (sx, sy) = history.settlements.get(&sid)
                    .map(|s| s.location)
                    .unwrap_or((0, 0));

                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::MonsterRaid,
                    date,
                    format!("{} raids {}", creature_name, settlement_name),
                    format!("{} attacked {}, killing {} people.",
                        creature_name, settlement_name, losses),
                )
                .at_location(sx, sy)
                .with_faction(fid)
                .with_participant(EntityId::LegendaryCreature(cid))
                .with_participant(EntityId::Settlement(sid))
                .with_consequence(Consequence::PopulationChange(fid, -(losses as i32)));
                history.chronicle.record(event);
                history.tile_history.record_event(sx, sy, event_id);
            }
        }
    }
}

/// A fallen people may rise again this many years after its fall at the earliest...
const REVIVAL_AFTER: u32 = 15;
/// ...and no later than this (by then its memory is gone).
const REVIVAL_WITHIN: u32 = 150;
/// Yearly chance that a fallen people's old seat rises against a foreign ruler.
const REVIVAL_CHANCE: f32 = 0.03;

/// Conquered peoples don't simply vanish. While a fallen people's founding seat still stands under
/// a ruler of another race, the town may rise and declare for it again: the people is restored
/// there (caused by its fall), a new ruler is crowned and a war of independence opens against the
/// holder. Without it every war of conquest was final, and over 250 years the map thinned to a
/// couple of peoples with nobody left to fight or trade with at the present day.
fn step_revivals(history: &mut WorldHistory, game_data: &GameData, rng: &mut impl Rng) {
    let date = history.current_date;
    let mut fallen: Vec<FactionId> = history.factions.values()
        .filter(|f| f.dissolved.map_or(false, |d| d.year + REVIVAL_AFTER <= date.year && d.year + REVIVAL_WITHIN >= date.year))
        .map(|f| f.id)
        .collect();
    fallen.sort();
    for fid in fallen {
        // The founding seat, if it stands under a living foreign people.
        let Some(seat_loc) = history.chronicle.events.iter()
            .find(|e| e.event_type == EventType::FactionFounded && e.factions_involved.first() == Some(&fid))
            .and_then(|e| e.location)
        else { continue };
        let Some(seat) = history.settlements.values()
            .filter(|t| !t.is_destroyed() && t.location == seat_loc)
            .map(|t| t.id).min()
        else { continue };
        let holder = history.settlements.get(&seat).map(|t| t.faction).unwrap_or(fid);
        let (race, holder_race) = (history.factions.get(&fid).map(|f| f.race_id), history.factions.get(&holder).map(|f| f.race_id));
        if holder == fid || race == holder_race { continue; }
        if !history.factions.get(&holder).map_or(false, |f| f.is_active()) { continue; }
        // A people's last town doesn't rebel away from it.
        if history.factions.get(&holder).map_or(true, |f| f.settlements.len() < 2) { continue; }
        if rng.gen::<f32>() >= REVIVAL_CHANCE { continue; }

        let fall = history.chronicle.events.iter().rev()
            .find(|e| e.event_type == EventType::FactionDestroyed && e.factions_involved.first() == Some(&fid))
            .map(|e| (e.id, e.date.year));
        let (name, holder_name) = (
            history.factions.get(&fid).map(|f| f.name.clone()).unwrap_or_default(),
            history.factions.get(&holder).map(|f| f.name.clone()).unwrap_or_default(),
        );
        let (seat_name, pop) = history.settlements.get(&seat).map(|t| (t.name.clone(), t.population)).unwrap_or_default();

        // The town changes hands; the people lives again.
        if let Some(h) = history.factions.get_mut(&holder) { h.remove_settlement(seat); }
        if let Some(f) = history.factions.get_mut(&fid) {
            f.dissolved = None;
            f.add_settlement(seat);
            f.capital = Some(seat);
            f.total_population = pop;
        }
        if let Some(t) = history.settlements.get_mut(&seat) { t.faction = fid; }
        history.tile_history.set_owner(seat_loc.0, seat_loc.1, fid, date);

        let event_id = history.id_generators.next_event();
        let mut event = Event::new(
            event_id,
            EventType::FactionFounded,
            date,
            format!("{} rises again", name),
            format!("{} threw off the rule of {} and declared for {} again{}.", seat_name, holder_name, name,
                fall.map(|(_, y)| format!(", {} years after its fall", date.year.saturating_sub(y))).unwrap_or_default()),
        )
        .at_location(seat_loc.0, seat_loc.1)
        .with_faction(fid)
        .with_faction(holder)
        .with_participant(EntityId::Settlement(seat));
        if let Some((cause, _)) = fall { event = event.caused_by(cause); }
        history.tile_history.record_event(seat_loc.0, seat_loc.1, event_id);
        history.chronicle.record(event);

        // A ruler for the restored people: the old one if they outlived the fall (elves do).
        if let Some(old) = history.factions.get(&fid).and_then(|f| f.current_leader) {
            if !history.figures.get(&old).map_or(false, |l| l.is_alive()) {
                succeed(history, old, fid, game_data, rng);
            }
        }

        // The holder fights to take it back.
        let war_id = history.id_generators.next_war();
        let war_event = history.id_generators.next_event();
        let war_name = format!("The War of {}'s Independence", seat_name);
        let mut war = War::new(war_id, war_name.clone(), holder, fid, date, WarCause::Independence);
        war.declaration_event = Some(war_event);
        history.wars.insert(war_id, war);
        for (a, b) in [(holder, fid), (fid, holder)] {
            if let Some(f) = history.factions.get_mut(&a) {
                f.wars.push(war_id);
                f.get_relation_mut(b, 0.0).declare_war(war_id);
            }
        }
        let event = Event::new(
            war_event,
            EventType::WarDeclared,
            date,
            war_name,
            format!("{} marched to take back {}.", holder_name, seat_name),
        )
        .with_faction(holder)
        .with_faction(fid)
        .caused_by(event_id);
        history.chronicle.record(event);
    }
}

/// Below this share of the beasts the world began with, slain beasts' broods start to rise.
const BROOD_FLOOR: f32 = 0.2;
/// Yearly chance that a brood rises while the world is below the floor.
const BROOD_CHANCE: f32 = 0.6;
/// Years a new brood grows before heroes go after it.
const BROOD_HIDING_YEARS: u32 = 20;

/// Heroes slay legendary beasts and nothing used to replace them, so by the present day the
/// world's beasts were gone (the dev world ended with none). When fewer than a fifth remain, the
/// brood of a slain beast may rise in its old lair: a new beast of the same kind, chronicled as
/// caused by the slaying, so it has a past ("the brood of Golrok the Profane, slain in 394").
fn step_broods(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let living = history.legendary_creatures.values().filter(|c| c.is_alive()).count();
    let floor = ((history.config.initial_legendary_creatures as f32 * BROOD_FLOOR).ceil() as usize).max(3);
    if living >= floor || rng.gen::<f32>() >= BROOD_CHANCE { return; }
    // Far below the floor, several broods come of age in one year.
    let n = 1 + (floor - living) / 4;
    for _ in 0..n { rise_brood(history, rng); }
}

/// One brood rises from the most recent slaying that has none yet.
fn rise_brood(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    // The most recent slaying whose beast has no brood yet.
    let has_brood: Vec<EventId> = history.chronicle.events.iter()
        .filter(|e| e.event_type == EventType::CreatureAppeared)
        .flat_map(|e| e.causes.iter().copied())
        .collect();
    let Some((slain_event, parent)) = history.chronicle.events.iter().rev()
        .filter(|e| e.event_type == EventType::CreatureSlain && !has_brood.contains(&e.id))
        .find_map(|e| e.primary_participants.iter().find_map(|p| match p {
            EntityId::LegendaryCreature(c) => Some((e.id, *c)),
            _ => None,
        }))
    else { return };
    let Some((species, lair, parent_name, slain_year)) = history.legendary_creatures.get(&parent)
        .and_then(|c| c.lair_location.map(|l| (c.species_id, l, c.full_name(), c.death_date.map(|d| d.year).unwrap_or(date.year))))
    else { return };

    let id = history.id_generators.next_legendary_creature();
    let (name, epithet) = crate::history::creatures::legendary::generate_legendary_name(rng);
    let mut beast = crate::history::creatures::legendary::LegendaryCreature::new(id, species, name, epithet, Some(date));
    beast.generate_unique_abilities(rng);
    beast.generate_size_multiplier(rng);
    beast.lair_location = Some(lair);
    beast.territory.push(lair);
    let full = beast.full_name();
    history.legendary_creatures.insert(id, beast);

    let event_id = history.id_generators.next_event();
    let event = Event::new(
        event_id,
        EventType::CreatureAppeared,
        date,
        format!("{} rises", full),
        format!("In the old lair of {}, slain in {}, its brood has grown: {} hunts again.", parent_name, slain_year, full),
    )
    .at_location(lair.0, lair.1)
    .with_participant(EntityId::LegendaryCreature(id))
    .with_participant(EntityId::LegendaryCreature(parent))
    .caused_by(slain_event);
    history.tile_history.record_event(lair.0, lair.1, event_id);
    history.chronicle.record(event);
}

/// Install a successor after a faction's leader has died (succession law, dynasties, crises).
pub(crate) fn succeed(history: &mut WorldHistory, dead_leader_id: FigureId, faction_id: FactionId, game_data: &GameData, rng: &mut impl Rng) {
    // A people that has ended crowns no one.
    if !history.factions.get(&faction_id).map_or(false, |f| f.is_active()) { return; }
    let date = history.current_date;
    // The crowning (or the crisis) follows the old ruler's death.
    let death = history.chronicle.last_of(EntityId::Figure(dead_leader_id));
    let after_death = |e: Event| match death { Some(d) => e.caused_by(d), None => e };
    let dead_name = history.figures.get(&dead_leader_id)
        .map(|f| f.full_name())
        .unwrap_or_default();

    let race_id = history.factions.get(&faction_id)
        .map(|f| f.race_id)
        .unwrap_or(RaceId(0));
    let succession_law = history.factions.get(&faction_id)
        .map(|f| f.succession_law)
        .unwrap_or(crate::history::civilizations::government::SuccessionLaw::Primogeniture);
    let dynasty_id = history.factions.get(&faction_id)
        .and_then(|f| f.ruling_dynasty);

    // Determine if a succession crisis occurs
    // Crisis-prone laws (OpenSuccession, Tanistry, ElectiveMonarchy) have higher chance
    let crisis_chance = if succession_law.crisis_prone() { 0.35 } else { 0.08 };
    let is_crisis = rng.gen::<f32>() < crisis_chance;

    // Under a dynastic law the dead ruler's eldest living child takes the throne (the heir
    // `people.rs` raised); only with none is a new ruler found.
    let naming_style = naming_style_for_race(history, race_id, game_data);
    let heir = if succession_law.requires_dynasty() {
        history.figures.get(&dead_leader_id).map(|d| d.children.clone()).unwrap_or_default().into_iter()
            .filter_map(|c| history.figures.get(&c).filter(|c| c.is_alive() && c.faction == Some(faction_id) && c.age_at(&date) >= 14))
            .min_by_key(|c| (c.birth_date.year, c.id)).map(|c| (c.id, c.name.clone()))
    } else { None };
    let inherited = heir.is_some();
    let (new_leader_id, new_leader_name, mut new_leader) = match heir {
        Some((id, name)) => (id, name, history.figures.remove(&id).unwrap()),
        None => {
            let id = history.id_generators.next_figure();
            let name = NameGenerator::personal_name(&naming_style, rng);
            let personality = Personality::random(rng);
            let mut f = Figure::new(id, name.clone(), race_id,
                Date::new(date.year.saturating_sub(rng.gen_range(20..50)), crate::seasons::Season::Spring), personality);
            f.faction = Some(faction_id);
            (id, name, f)
        }
    };

    // Wire dynasty link based on succession law (a born heir is already the ruler's child)
    if succession_law.requires_dynasty() && !inherited {
        new_leader.parents.0 = Some(dead_leader_id);
        if let Some(dead_leader) = history.figures.get_mut(&dead_leader_id) {
            dead_leader.add_child(new_leader_id);
        }
    }

    // Update faction
    let faction_name = if let Some(faction) = history.factions.get_mut(&faction_id) {
        faction.current_leader = Some(new_leader_id);
        faction.notable_figures.push(new_leader_id);
        faction.name.clone()
    } else {
        String::new()
    };

    // Update dynasty
    if let Some(did) = dynasty_id {
        if let Some(dynasty) = history.dynasties.get_mut(&did) {
            dynasty.add_member(new_leader_id);
            dynasty.current_head = Some(new_leader_id);
            dynasty.generations += 1;
        }
        new_leader.dynasty = Some(did);
    }

    history.figures.insert(new_leader_id, new_leader);

    if is_crisis {
        // Succession crisis: rival claimant challenges the new ruler
        let rival_id = history.id_generators.next_figure();
        let rival_name = NameGenerator::personal_name(&naming_style, rng);
        let rival_personality = Personality::random(rng);
        let mut rival = Figure::new(
            rival_id, rival_name.clone(),
            race_id,
            Date::new(date.year.saturating_sub(rng.gen_range(25..55)), crate::seasons::Season::Spring),
            rival_personality,
        );
        rival.faction = Some(faction_id);
        rival.enemies.push(new_leader_id);

        // New leader considers rival an enemy too
        if let Some(new_leader) = history.figures.get_mut(&new_leader_id) {
            new_leader.enemies.push(rival_id);
        }

        if let Some(faction) = history.factions.get_mut(&faction_id) {
            faction.notable_figures.push(rival_id);
        }

        history.figures.insert(rival_id, rival);

        // Record succession crisis event
        let crisis_event_id = history.id_generators.next_event();
        let crisis_event = Event::new(
            crisis_event_id,
            EventType::SuccessionCrisis,
            date,
            format!("Succession crisis in {}", faction_name),
            format!("Upon the death of {}, {} and {} both claim the throne of {}.",
                dead_name, new_leader_name, rival_name, faction_name),
        )
        .with_faction(faction_id)
        .with_participant(EntityId::Figure(new_leader_id))
        .with_participant(EntityId::Figure(rival_id));
        history.chronicle.record(after_death(crisis_event));

        // Determine crisis outcome: coup (30%) or civil unrest (70%)
        if rng.gen::<f32>() < 0.30 {
            // Coup: rival seizes power
            if let Some(faction) = history.factions.get_mut(&faction_id) {
                faction.current_leader = Some(rival_id);
            }
            if let Some(new_leader) = history.figures.get_mut(&new_leader_id) {
                new_leader.kill(date, DeathCause::Execution);
            }

            // Dynasty scandal
            if let Some(did) = dynasty_id {
                if let Some(dynasty) = history.dynasties.get_mut(&did) {
                    dynasty.scandals.push(crisis_event_id);
                    dynasty.prestige = dynasty.prestige.saturating_sub(10);
                }
            }

            let coup_event_id = history.id_generators.next_event();
            let coup_event = Event::new(
                coup_event_id,
                EventType::Coup,
                date,
                format!("{} seizes power in {}", rival_name, faction_name),
                format!("{} overthrew {} and seized the throne of {}. {} was executed.",
                    rival_name, new_leader_name, faction_name, new_leader_name),
            )
            .with_faction(faction_id)
            .with_participant(EntityId::Figure(rival_id))
            .with_participant(EntityId::Figure(new_leader_id))
            .with_consequence(Consequence::FigureDeath(new_leader_id, DeathCause::Execution))
            .caused_by(crisis_event_id);
            history.chronicle.record(coup_event);
        } else {
            // Civil unrest: population loss, rival becomes enemy, dynasty loses prestige
            let losses = rng.gen_range(50..200);
            if let Some(faction) = history.factions.get_mut(&faction_id) {
                faction.total_population = faction.total_population.saturating_sub(losses);
            }
            if let Some(did) = dynasty_id {
                if let Some(dynasty) = history.dynasties.get_mut(&did) {
                    dynasty.scandals.push(crisis_event_id);
                    dynasty.prestige = dynasty.prestige.saturating_sub(5);
                }
            }

            let deposed_event_id = history.id_generators.next_event();
            let deposed_event = Event::new(
                deposed_event_id,
                EventType::RulerDeposed,
                date,
                format!("Unrest in {} over succession", faction_name),
                format!("The succession of {} in {} was contested by {}. {} perished in the fighting.",
                    new_leader_name, faction_name, rival_name, losses),
            )
            .with_faction(faction_id)
            .with_participant(EntityId::Figure(new_leader_id))
            .with_participant(EntityId::Figure(rival_id))
            .with_consequence(Consequence::PopulationChange(faction_id, -(losses as i32)))
            .caused_by(crisis_event_id);
            history.chronicle.record(deposed_event);
        }
    } else {
        // Normal succession
        let (title, desc) = game_data.backstory.succession_description(
            &new_leader_name, &dead_name, &faction_name, rng,
        );
        if let Some(did) = dynasty_id {
            if let Some(dynasty) = history.dynasties.get_mut(&did) {
                dynasty.prestige += 3;
            }
        }

        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::RulerCrowned,
            date,
            title,
            desc,
        )
        .with_faction(faction_id)
        .with_participant(EntityId::Figure(new_leader_id));
        history.chronicle.record(after_death(event));
    }
}

fn step_figures(history: &mut WorldHistory, game_data: &GameData, rng: &mut impl Rng) {
    let date = history.current_date;

    // Natural deaths of old figures
    let figure_ids: Vec<FigureId> = history.figures.keys()
        .copied()
        .filter(|id| history.figures.get(id).map_or(false, |f| f.is_alive()))
        .collect();

    let mut dead_leaders: Vec<(FigureId, FactionId)> = Vec::new();

    for fid in figure_ids {
        let (age, race_id, faction) = match history.figures.get(&fid) {
            Some(fig) => (fig.age_at(&date), fig.race_id, fig.faction),
            None => continue,
        };

        // Get lifespan from race
        let max_age = history.races.get(&race_id)
            .map(|r| r.lifespan.1)
            .unwrap_or(100);

        // Immortal races don't die of old age
        if max_age == 0 {
            continue;
        }

        // Death probability increases with age
        if age > max_age / 2 {
            let death_chance = (age as f32 - max_age as f32 / 2.0) / (max_age as f32 / 2.0) * 0.05;
            if rng.gen::<f32>() < death_chance {
                if let Some(fig) = history.figures.get_mut(&fid) {
                    fig.kill(date, DeathCause::Natural);
                }

                // Check if this was a faction leader
                if let Some(faction_id) = faction {
                    let is_leader = history.factions.get(&faction_id)
                        .map_or(false, |f| f.current_leader == Some(fid));
                    if is_leader {
                        dead_leaders.push((fid, faction_id));
                    }
                }

                let fig_name = history.figures.get(&fid)
                    .map(|f| f.full_name())
                    .unwrap_or_default();

                let event_id = history.id_generators.next_event();
                let mut event = Event::new(
                    event_id,
                    EventType::HeroDied,
                    date,
                    format!("Death of {}", fig_name),
                    format!("{} died of old age at {}.", fig_name, age),
                )
                .with_participant(EntityId::Figure(fid));

                if let Some(fac_id) = faction {
                    event = event.with_faction(fac_id);
                }
                history.chronicle.record(event);
            }
        }
    }

    // Succession for dead leaders — consults SuccessionLaw, may trigger crises
    for (dead_leader_id, faction_id) in dead_leaders {
        succeed(history, dead_leader_id, faction_id, game_data, rng);
    }

    // Hero births (rare)
    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    for &fid in &faction_ids {
        if rng.gen::<f32>() < 0.01 {
            let race_id = history.factions.get(&fid).map(|f| f.race_id).unwrap_or(RaceId(0));
            let hero_style = naming_style_for_race(history, race_id, game_data);
            let hero_id = history.id_generators.next_figure();
            let personality = Personality::random(rng);
            let hero_name = NameGenerator::personal_name(&hero_style, rng);
            let mut hero = Figure::new(
                hero_id,
                hero_name,
                race_id,
                date,
                personality,
            );
            hero.faction = Some(fid);

            // Give some skills
            let skill = match rng.gen_range(0..5) {
                0 => Skill::Combat,
                1 => Skill::Leadership,
                2 => Skill::Diplomacy,
                3 => Skill::Crafting,
                _ => Skill::Strategy,
            };
            hero.skills.insert(skill, rng.gen_range(5..10));

            if let Some(faction) = history.factions.get_mut(&fid) {
                faction.notable_figures.push(hero_id);
            }

            let faction_name = history.factions.get(&fid)
                .map(|f| f.name.as_str())
                .unwrap_or("unknown");

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::HeroBorn,
                date,
                format!("Birth of {}", hero.name),
                format!("A notable figure was born in {}.", faction_name),
            )
            .with_faction(fid)
            .with_participant(EntityId::Figure(hero_id));
            history.chronicle.record(event);

            history.figures.insert(hero_id, hero);
        }
    }

    // Rebellion events: tyrannical leaders risk uprisings
    for &fid in &faction_ids {
        let tyranny_score = leader_personality(history, fid)
            .map(|p| p.tyranny())
            .unwrap_or(0.5);

        // Only trigger if tyranny > 0.6, scaling chance up to 1% for tyranny = 1.0
        if tyranny_score > 0.6 {
            let rebellion_chance = (tyranny_score - 0.6) * 0.025; // 0% at 0.6, 1% at 1.0
            if rng.gen::<f32>() < rebellion_chance {
                let faction_name = history.factions.get(&fid)
                    .map(|f| f.name.clone()).unwrap_or_default();
                let leader_name = history.factions.get(&fid)
                    .and_then(|f| f.current_leader)
                    .and_then(|lid| history.figures.get(&lid))
                    .map(|f| f.full_name())
                    .unwrap_or_else(|| "the ruler".to_string());

                // Population loss from rebellion
                let losses = rng.gen_range(50..300);
                if let Some(faction) = history.factions.get_mut(&fid) {
                    faction.total_population = faction.total_population.saturating_sub(losses);
                }

                // Chance the leader dies in the rebellion (20%)
                let leader_dies = rng.gen::<f32>() < 0.2;
                let leader_id = history.factions.get(&fid).and_then(|f| f.current_leader);

                if leader_dies {
                    if let Some(lid) = leader_id {
                        if let Some(fig) = history.figures.get_mut(&lid) {
                            fig.kill(date, DeathCause::Execution);
                        }
                    }
                }

                let event_id = history.id_generators.next_event();
                let desc = if leader_dies {
                    format!("The people of {} rose against the tyrannical {}. In the chaos, {} was killed. {} perished in the fighting.",
                        faction_name, leader_name, leader_name, losses)
                } else {
                    format!("A rebellion erupted in {} against the tyranny of {}. The uprising was crushed, but {} people perished.",
                        faction_name, leader_name, losses)
                };
                let event = Event::new(
                    event_id,
                    EventType::Rebellion,
                    date,
                    format!("Rebellion in {}", faction_name),
                    desc,
                )
                .with_faction(fid)
                .with_consequence(Consequence::PopulationChange(fid, -(losses as i32)));
                history.chronicle.record(event);

                // If leader died, trigger succession in next step (they're already marked dead)
            }
        }
    }
}

fn step_artifacts(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let rate = history.config.artifact_creation_rate;

    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    for &fid in &faction_ids {
        // Leader's builder_inclination modulates monument/artifact rate
        let builder_incl = leader_personality(history, fid)
            .map(|p| p.builder_inclination()).unwrap_or(0.5);
        let builder_mult = Personality::score_to_multiplier(builder_incl, 0.2, 3.5);

        // Religion modifier on monuments (MonasticTradition +50%, Asceticism -40%)
        let religion_monument_mult = faction_religion_monument_modifier(history, fid);
        let builder_mult = builder_mult * religion_monument_mult;

        // Artifact creation
        if rng.gen::<f32>() < 0.005 * rate * builder_mult {
            let art_id = history.id_generators.next_artifact();
            let art_type = match rng.gen_range(0..10) {
                0 => ArtifactType::Weapon,
                1 => ArtifactType::Armor,
                2 => ArtifactType::Crown,
                3 => ArtifactType::Ring,
                4 => ArtifactType::Amulet,
                5 => ArtifactType::Staff,
                6 => ArtifactType::Book,
                7 => ArtifactType::Goblet,
                8 => ArtifactType::Instrument,
                _ => ArtifactType::Relic,
            };
            let quality = match rng.gen_range(0u32..100) {
                0..=40 => ArtifactQuality::Fine,
                41..=70 => ArtifactQuality::Superior,
                71..=90 => ArtifactQuality::Masterwork,
                91..=98 => ArtifactQuality::Legendary,
                _ => ArtifactQuality::Divine,
            };

            // Made by a real hand (a smith of theirs, else the ruler) to remember a real deed.
            use crate::history::remembrance as mem;
            let (creator, town) = mem::maker(history, fid);
            let deed = mem::recent_deed(history, fid, 40);
            let salt = art_id.0 as u64;
            let item = mem::item_word(art_type, salt);
            let maker_name = creator.and_then(|c| history.figures.get(&c)).map(|x| x.name.clone());
            let maker_full = creator.and_then(|c| history.figures.get(&c)).map(|x| x.full_name());
            let town_name = town.and_then(|t| history.settlements.get(&t)).map(|t| t.name.clone());
            let art_name = mem::artifact_name(history, item, maker_name.as_deref(), town_name.as_deref(), deed.as_ref(), salt)
                .unwrap_or_else(|| {
                    // No maker, town or deed: the old stock name, kept unique.
                    let existing: Vec<&str> = history.artifacts.values().map(|a| a.name.as_str()).collect();
                    let mut n = generate_artifact_name(art_type, quality, rng);
                    let mut attempts = 0;
                    while existing.contains(&n.as_str()) && attempts < 10 { n = generate_artifact_name(art_type, quality, rng); attempts += 1; }
                    n
                });
            let mut artifact = Artifact::new(
                art_id, art_name.clone(), art_type, quality, date, creator,
            );
            let quality_word = plain_words(&format!("{:?}", quality));
            let made = match (&maker_full, &town_name) {
                (Some(m), Some(t)) => format!("{} {} {} made by {} at {} in the year {}", mem::article(&quality_word), quality_word, item.to_lowercase(), m, t, date.year),
                (Some(m), None) => format!("{} {} {} made by {} in the year {}", mem::article(&quality_word), quality_word, item.to_lowercase(), m, date.year),
                (None, _) => format!("{} {} {} made in the year {}", mem::article(&quality_word), quality_word, item.to_lowercase(), date.year),
            };
            artifact.description = match &deed {
                Some(d) => format!("{}, to remember {} ({}).", capitalize_first(&made), d.phrase, d.year),
                None => format!("{}.", capitalize_first(&made)),
            };
            if let Some(d) = &deed {
                let mut refers: Vec<EntityId> = d.who.clone();
                if let Some(c) = creator { refers.push(EntityId::Figure(c)); }
                artifact.inscriptions.push(crate::history::objects::artifacts::Inscription {
                    text: format!("{} made me, for {}.", maker_name.clone().unwrap_or_else(|| "A smith".into()), d.phrase),
                    translation: String::new(),
                    refers_to: refers,
                    date_inscribed: date,
                });
            }
            artifact.creation_location = town.and_then(|t| history.settlements.get(&t)).map(|t| t.location);

            // Assign to faction leader
            if let Some(leader_id) = history.factions.get(&fid).and_then(|f| f.current_leader) {
                artifact.transfer_to(
                    EntityId::Figure(leader_id), date, AcquisitionMethod::Created,
                );
                if let Some(fig) = history.figures.get_mut(&leader_id) {
                    fig.artifacts.push(art_id);
                }
            }

            let faction_name = history.factions.get(&fid).map(|f| f.name.clone()).unwrap_or_default();
            let event_id = history.id_generators.next_event();
            let mut event = Event::new(
                event_id,
                EventType::ArtifactCreated,
                date,
                format!("Creation of {}", art_name),
                match (&maker_full, &deed) {
                    (Some(m), Some(d)) => format!("{} of {} made {}, to remember {}.", m, faction_name, art_name, d.phrase),
                    (Some(m), None) => format!("{} of {} made {}.", m, faction_name, art_name),
                    (None, Some(d)) => format!("{} was made for {}, to remember {}.", art_name, faction_name, d.phrase),
                    (None, None) => format!("{} was crafted by {}.", art_name, faction_name),
                },
            )
            .with_faction(fid)
            .with_participant(EntityId::Artifact(art_id));
            if let Some(c) = creator { event = event.with_participant(EntityId::Figure(c)); }
            if let Some(d) = &deed { event = event.caused_by(d.event); }
            if let Some(l) = artifact.creation_location { event = event.at_location(l.0, l.1); }
            artifact.creation_event = Some(event_id);
            history.chronicle.record(event);

            history.artifacts.insert(art_id, artifact);
        }

        // Monument construction (also modulated by builder_inclination)
        if rng.gen::<f32>() < 0.003 * rate * builder_mult {
            let capital = history.factions.get(&fid).and_then(|f| f.capital);
            let location = capital.and_then(|sid| history.settlements.get(&sid).map(|s| s.location));

            if let Some((mx, my)) = location {
                let mon_id = history.id_generators.next_monument();
                // Bias monument type by leader personality
                let leader_p = leader_personality(history, fid);
                let mon_type = pick_monument_type(leader_p, rng);
                let purpose = pick_monument_purpose(leader_p, rng);

                // Raised for a real deed, and named after it.
                let deed = crate::history::remembrance::recent_deed(history, fid, 40);
                let faction_name = history.factions.get(&fid).map(|f| f.name.clone()).unwrap_or_default();
                let capital_name = capital.and_then(|c| history.settlements.get(&c)).map(|t| t.name.clone()).unwrap_or_default();
                let type_str = plain_words(&format!("{:?}", mon_type));
                let type_title = format!("{:?}", mon_type);
                let place_word = strip_the(&faction_name).split_whitespace().next().unwrap_or("Grand");
                let mut mon_name = match &deed {
                    Some(d) if d.subject.starts_with("the ") => format!("The {} of {}", type_title, d.subject),
                    Some(d) => format!("The {} of {}", type_title, d.subject),
                    None => format!("The {} {}", place_word, type_title),
                };
                if history.monuments.values().any(|m| m.name == mon_name) {
                    mon_name = format!("{} at {}", mon_name, capital_name);
                }
                let mut monument = Monument::new(
                    mon_id, mon_name.clone(), mon_type, (mx, my),
                    fid, date, purpose,
                );
                monument.commissioned_by = history.factions.get(&fid).and_then(|f| f.current_leader);

                let event_id = history.id_generators.next_event();
                let mut event = Event::new(
                    event_id,
                    EventType::MonumentBuilt,
                    date,
                    format!("Construction of {}", mon_name),
                    match &deed {
                        Some(d) => format!("{} raised {} {} at {} to remember {} ({}).", faction_name, crate::history::remembrance::article(&type_str), type_str, capital_name, d.phrase, d.year),
                        None => format!("{} raised {} {} at {}.", faction_name, crate::history::remembrance::article(&type_str), type_str, capital_name),
                    },
                )
                .at_location(mx, my)
                .with_faction(fid);
                if let Some(c) = capital { event = event.with_participant(EntityId::Settlement(c)); }
                if let Some(d) = &deed {
                    event = event.caused_by(d.event);
                    monument.commemorates = Some(d.event);
                    monument.honors = d.who.clone();
                    monument.inscriptions.push(crate::history::objects::artifacts::Inscription {
                        text: format!("Raised by {} in the year {}, to remember {}.", faction_name, date.year, d.phrase),
                        translation: String::new(),
                        refers_to: d.who.clone(),
                        date_inscribed: date,
                    });
                }
                monument.construction_event = Some(event_id);
                history.chronicle.record(event);
                history.tile_history.record_event(mx, my, event_id);

                if let Some(settlement) = capital.and_then(|sid| history.settlements.get_mut(&sid)) {
                    settlement.monuments.push(mon_id);
                }

                history.monuments.insert(mon_id, monument);
            }
        }
    }
}

fn step_religion(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    // Collect religion and faction data needed for processing
    let religion_ids: Vec<ReligionId> = history.religions.keys().copied().collect();
    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    // 1. Proselytizing conversion attempts (0.5% per step per proselytizing religion toward non-follower factions)
    for &rid in &religion_ids {
        let is_proselytizing = history.religions.get(&rid)
            .map_or(false, |r| r.has_doctrine(Doctrine::Proselytizing));
        if !is_proselytizing { continue; }

        // Cap conversion attempts per religion per step
        let max_conversions_per_step = 1;
        let mut conversions_this_step = 0;

        let follower_factions: Vec<FactionId> = history.religions.get(&rid)
            .map(|r| r.follower_factions.clone())
            .unwrap_or_default();

        for &fid in &faction_ids {
            if conversions_this_step >= max_conversions_per_step { break; }
            if follower_factions.contains(&fid) { continue; }

            // Base 0.5% chance, reduced by target culture's xenophobia
            let xenophobia = history.factions.get(&fid)
                .and_then(|f| history.races.get(&f.race_id))
                .and_then(|r| history.cultures.get(&r.culture_id))
                .map(|c| c.values.xenophobia)
                .unwrap_or(0.5);
            let conversion_chance = 0.005 * (1.0 - xenophobia * 0.7);

            // Missionaries come from a neighbouring people that keeps the faith; no neighbour of
            // the faith, no conversion (faith spreads by contact, not by chance across the map).
            let missionary = follower_factions.iter().copied()
                .filter(|&m| m != fid && history.factions.get(&m).map_or(false, |f| f.is_active()))
                .find(|&m| factions_are_neighbors(history, fid, m, 30));
            let Some(missionary) = missionary else { continue };

            if rng.gen::<f32>() < conversion_chance {
                let religion_name = history.religions.get(&rid)
                    .map(|r| r.name.clone()).unwrap_or_default();
                let faction_name = history.factions.get(&fid)
                    .map(|f| f.name.clone()).unwrap_or_default();

                // Convert: set new state religion
                let old_religion = history.factions.get(&fid).and_then(|f| f.state_religion);
                if let Some(faction) = history.factions.get_mut(&fid) {
                    faction.state_religion = Some(rid);
                }
                if let Some(religion) = history.religions.get_mut(&rid) {
                    religion.add_follower_faction(fid);
                    religion.follower_count += history.factions.get(&fid)
                        .map(|f| f.total_population).unwrap_or(0);
                }
                // Remove from old religion
                if let Some(old_rid) = old_religion {
                    if let Some(old_rel) = history.religions.get_mut(&old_rid) {
                        old_rel.follower_factions.retain(|&f| f != fid);
                        old_rel.follower_count = old_rel.follower_count.saturating_sub(
                            history.factions.get(&fid).map(|f| f.total_population).unwrap_or(0)
                        );
                    }
                }

                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::Miracle,
                    date,
                    format!("{} converts to {}", faction_name, religion_name),
                    format!("{} adopted {}, carried to them by missionaries of {}.", faction_name, religion_name,
                        history.factions.get(&missionary).map(|f| f.name.clone()).unwrap_or_default()),
                )
                .with_faction(fid)
                .with_faction(missionary);
                history.chronicle.record(event);

                conversions_this_step += 1;
            }
        }
    }

    // 2. SacrificeRequired: generate sacrifice events (population cost, recorded event)
    for &fid in &faction_ids {
        let has_sacrifice = history.factions.get(&fid)
            .and_then(|f| f.state_religion)
            .and_then(|rid| history.religions.get(&rid))
            .map_or(false, |r| r.has_doctrine(Doctrine::SacrificeRequired));
        if !has_sacrifice { continue; }

        // 0.5% chance per season of a sacrifice event
        if rng.gen::<f32>() < 0.005 {
            let losses = rng.gen_range(5..30);
            let faction_name = history.factions.get(&fid)
                .map(|f| f.name.clone()).unwrap_or_default();
            if let Some(faction) = history.factions.get_mut(&fid) {
                faction.total_population = faction.total_population.saturating_sub(losses);
            }

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::Miracle,
                date,
                format!("Ritual sacrifice in {}", faction_name),
                format!("{} conducted a ritual sacrifice of {} souls to appease the gods.",
                    faction_name, losses),
            )
            .with_faction(fid)
            .with_consequence(Consequence::PopulationChange(fid, -(losses as i32)));
            history.chronicle.record(event);
        }
    }

    // 3. MonasticTradition: increased temple building rate (handled via monument_modifier above)
    // Already wired into step_artifacts.

    // 4. Religious schisms (0.1% per step for religions with 2+ follower factions)
    let mut schism_events = Vec::new();
    for &rid in &religion_ids {
        let follower_count = history.religions.get(&rid)
            .map(|r| r.follower_factions.len())
            .unwrap_or(0);
        if follower_count < 2 { continue; }

        if rng.gen::<f32>() < 0.001 {
            let religion_name = history.religions.get(&rid)
                .map(|r| r.name.clone()).unwrap_or_default();

            // Pick a random follower to become the heretic faction
            let followers: Vec<FactionId> = history.religions.get(&rid)
                .map(|r| r.follower_factions.clone())
                .unwrap_or_default();
            let heretic_fid = followers[rng.gen_range(0..followers.len())];
            let heretic_faction_name = history.factions.get(&heretic_fid)
                .map(|f| f.name.clone()).unwrap_or_default();

            schism_events.push((rid, heretic_fid, religion_name, heretic_faction_name));
        }
    }

    // Apply schisms (deferred to avoid borrow issues)
    for (rid, heretic_fid, religion_name, heretic_faction_name) in schism_events {
        let new_rid = history.id_generators.next_religion();
        let heresy_name = generate_heresy_name(&religion_name, rng);

        // Copy deities from parent
        let deities = history.religions.get(&rid)
            .map(|r| r.deities.clone())
            .unwrap_or_default();

        let mut heresy = Religion::new(
            new_rid,
            heresy_name.clone(),
            deities,
            date,
            history.factions.get(&heretic_fid).and_then(|f| f.current_leader),
        );
        heresy.add_follower_faction(heretic_fid);
        heresy.follower_count = history.factions.get(&heretic_fid)
            .map(|f| f.total_population).unwrap_or(0);

        // Give the heresy a random subset of parent doctrines + 1 new one
        if let Some(parent) = history.religions.get(&rid) {
            for d in &parent.doctrines {
                if rng.gen_bool(0.5) {
                    heresy.doctrines.push(*d);
                }
            }
        }

        // Register heresy
        if let Some(parent) = history.religions.get_mut(&rid) {
            parent.add_heresy(new_rid);
            parent.hostile_religions.push(new_rid);
            parent.follower_factions.retain(|&f| f != heretic_fid);
            parent.follower_count = parent.follower_count.saturating_sub(
                history.factions.get(&heretic_fid).map(|f| f.total_population).unwrap_or(0)
            );
        }
        heresy.hostile_religions.push(rid);

        // Update faction
        if let Some(faction) = history.factions.get_mut(&heretic_fid) {
            faction.state_religion = Some(new_rid);
        }

        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::ReligionFounded,
            date,
            format!("Schism: {} breaks from {}", heresy_name, religion_name),
            format!("{} declared {} a heresy and split from {} faith.",
                heretic_faction_name, heresy_name, religion_name),
        )
        .with_faction(heretic_fid);
        let mut event = event;
        event.is_major = true;
        history.chronicle.record(event);

        history.religions.insert(new_rid, heresy);
    }
}

fn step_natural_events(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    // Rare natural disasters
    if rng.gen::<f32>() < 0.005 {
        let event_type = match rng.gen_range(0..5) {
            0 => EventType::Earthquake,
            1 => EventType::Flood,
            2 => EventType::Drought,
            3 => EventType::Plague,
            _ => EventType::VolcanoErupted,
        };

        // Pick a random settlement to affect
        let settlement_ids: Vec<SettlementId> = history.settlements.keys()
            .copied()
            .filter(|id| history.settlements.get(id).map_or(false, |s| !s.is_destroyed()))
            .collect();

        if settlement_ids.is_empty() {
            return;
        }

        let &sid = &settlement_ids[rng.gen_range(0..settlement_ids.len())];

        let (loc, faction_id, settlement_name) = {
            let s = match history.settlements.get(&sid) {
                Some(s) => s,
                None => return,
            };
            (s.location, s.faction, s.name.clone())
        };

        let losses = scaled_loss(history, sid, rng.gen_range(50..500), rng);
        if let Some(settlement) = history.settlements.get_mut(&sid) {
            settlement.population = settlement.population.saturating_sub(losses);
        }
        if let Some(faction) = history.factions.get_mut(&faction_id) {
            faction.total_population = faction.total_population.saturating_sub(losses);
        }

        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            event_type.clone(),
            date,
            format!("{} strikes {}", disaster_noun(&event_type), settlement_name),
            format!("{} devastated {}, killing {} people.",
                capitalize_first(&disaster_phrase(&event_type)), settlement_name, losses),
        )
        .at_location(loc.0, loc.1)
        .with_faction(faction_id)
        .with_participant(EntityId::Settlement(sid))
        .with_consequence(Consequence::PopulationChange(faction_id, -(losses as i32)));
        history.chronicle.record(event);
        history.tile_history.record_event(loc.0, loc.1, event_id);
    }
}

fn step_diplomacy_peaceful(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let diplomacy_rate = history.config.diplomacy_rate;

    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    if faction_ids.len() < 2 {
        return;
    }

    // Higher chance for peaceful diplomacy than war
    let treaty_chance = 0.05 * diplomacy_rate;
    let alliance_chance = 0.025 * diplomacy_rate;

    // Sample pairs instead of O(n^2)
    let pairs_to_check = (faction_ids.len() * 2).min(300);

    for _ in 0..pairs_to_check {
        let idx_a = rng.gen_range(0..faction_ids.len());
        let mut idx_b = rng.gen_range(0..faction_ids.len());
        if idx_a == idx_b { idx_b = (idx_a + 1) % faction_ids.len(); }
        let fid_a = faction_ids[idx_a];
        let fid_b = faction_ids[idx_b];

        // Skip factions at war
        let at_war = history.factions.get(&fid_a)
            .map_or(false, |f| f.is_at_war_with(fid_b));
        if at_war { continue; }

        let opinion = history.factions.get(&fid_a)
            .and_then(|f| f.relations.get(&fid_b))
            .map(|r| r.opinion)
            .unwrap_or(0);

        let stance = history.factions.get(&fid_a)
            .and_then(|f| f.relations.get(&fid_b))
            .map(|r| r.stance)
            .unwrap_or(DiplomaticStance::Neutral);

        // Personality-modulated diplomacy
        let dip_a = leader_personality(history, fid_a)
            .map(|p| p.diplomacy_inclination()).unwrap_or(0.5);
        let dip_b = leader_personality(history, fid_b)
            .map(|p| p.diplomacy_inclination()).unwrap_or(0.5);
        let avg_diplomacy = (dip_a + dip_b) / 2.0;
        let diplomacy_mult = Personality::score_to_multiplier(avg_diplomacy, 0.2, 3.5);

        let rel_dip_a = faction_religion_diplomacy_modifier(history, fid_a);
        let rel_dip_b = faction_religion_diplomacy_modifier(history, fid_b);
        let religion_dip_mult = (rel_dip_a + rel_dip_b) / 2.0;

        let same_religion_bonus = if factions_share_religion(history, fid_a, fid_b) { 1.5 } else { 1.0 };
        let diplomacy_mult = diplomacy_mult * religion_dip_mult * same_religion_bonus;

        // Try to form treaty if neutral and opinion >= 0
        if matches!(stance, DiplomaticStance::Neutral) && opinion >= 0 && rng.gen::<f32>() < treaty_chance * diplomacy_mult {
            let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
            let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();

            if let Some(faction) = history.factions.get_mut(&fid_a) {
                let rel = faction.get_relation_mut(fid_b, 0.0);
                rel.stance = DiplomaticStance::Friendly;
                rel.opinion = (rel.opinion + 20).min(100);
            }
            if let Some(faction) = history.factions.get_mut(&fid_b) {
                let rel = faction.get_relation_mut(fid_a, 0.0);
                rel.stance = DiplomaticStance::Friendly;
                rel.opinion = (rel.opinion + 20).min(100);
            }

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::TreatySigned,
                date,
                format!("Treaty between {} and {}", name_a, name_b),
                format!("{} and {} signed a peace treaty, improving relations.", name_a, name_b),
            )
            .with_faction(fid_a)
            .with_faction(fid_b)
            .with_consequence(Consequence::RelationChange(fid_a, fid_b, 20));
            history.chronicle.record(event);
        }
    }

    // Also process existing friendly relations for alliance upgrades
    let faction_ids2 = faction_ids.clone();
    for &fid_a in &faction_ids2 {
        let friendly_targets: Vec<(FactionId, i32)> = history.factions.get(&fid_a)
            .map(|f| {
                f.relations.iter()
                    .filter(|(_, r)| matches!(r.stance, DiplomaticStance::Friendly) && r.opinion >= 50)
                    .map(|(&fid, r)| (fid, r.opinion))
                    .collect()
            })
            .unwrap_or_default();

        for (fid_b, _opinion) in friendly_targets {
            if !history.factions.get(&fid_b).map_or(false, |f| f.is_active()) { continue; }

            let dip_a = leader_personality(history, fid_a)
                .map(|p| p.diplomacy_inclination()).unwrap_or(0.5);
            let dip_b = leader_personality(history, fid_b)
                .map(|p| p.diplomacy_inclination()).unwrap_or(0.5);
            let avg_diplomacy = (dip_a + dip_b) / 2.0;
            let diplomacy_mult = Personality::score_to_multiplier(avg_diplomacy, 0.2, 3.5);

            if rng.gen::<f32>() < alliance_chance * diplomacy_mult {
                let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
                let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();

                if let Some(faction) = history.factions.get_mut(&fid_a) {
                    let rel = faction.get_relation_mut(fid_b, 0.0);
                    rel.stance = DiplomaticStance::Allied;
                    rel.opinion = (rel.opinion + 30).min(100);
                }
                if let Some(faction) = history.factions.get_mut(&fid_b) {
                    let rel = faction.get_relation_mut(fid_a, 0.0);
                    rel.stance = DiplomaticStance::Allied;
                    rel.opinion = (rel.opinion + 30).min(100);
                }

                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::AllianceFormed,
                    date,
                    format!("Alliance of {} and {}", name_a, name_b),
                    format!("{} and {} formed a military alliance.", name_a, name_b),
                )
                .with_faction(fid_a)
                .with_faction(fid_b)
                .with_consequence(Consequence::RelationChange(fid_a, fid_b, 30));
                history.chronicle.record(event);
            }
        }
    }
}

fn step_trade(history: &mut WorldHistory, world: &WorldData, rng: &mut impl Rng) {
    let date = history.current_date;
    let trade_rate = history.config.trade_frequency;

    // Higher chance per settlement to try establishing a trade route
    let route_chance = 0.02 * trade_rate;
    
    // Gather all active settlements with their locations and factions
    let settlements: Vec<(SettlementId, (usize, usize), FactionId)> = history.settlements.values()
        .filter(|s| !s.is_destroyed())
        .map(|s| (s.id, s.location, s.faction))
        .collect();
    
    // Max new routes per step to avoid explosion
    let mut new_routes_this_step = 0;
    let max_routes_per_step = 20;
    
    // For each settlement, try to establish a trade route with a nearby settlement
    for (sid_a, loc_a, fid_a) in &settlements {
        if new_routes_this_step >= max_routes_per_step {
            break;
        }
        
        if rng.gen::<f32>() >= route_chance {
            continue;
        }
        
        // Find nearby settlements from different factions (within 80 tiles)
        let candidates: Vec<_> = settlements.iter()
            .filter(|(sid_b, loc_b, fid_b)| {
                if fid_b == fid_a { return false; } // Different faction
                if sid_b == sid_a { return false; }
                let dx = loc_a.0 as i64 - loc_b.0 as i64;
                let dy = loc_a.1 as i64 - loc_b.1 as i64;
                dx * dx + dy * dy <= 6400 // Within ~80 tiles
            })
            .collect();
        
        if candidates.is_empty() {
            continue;
        }
        
        // Pick the partner with the most to swap: sample a few and weigh goods against distance.
        let mut best: Option<(f32, usize)> = None;
        for _ in 0..8 {
            let ci = rng.gen_range(0..candidates.len());
            let (sb, lb, _) = candidates[ci];
            let (gain, _) = trade_gain(history, *sid_a, *sb);
            let d = (((loc_a.0 as f32 - lb.0 as f32).powi(2) + (loc_a.1 as f32 - lb.1 as f32).powi(2)).sqrt()).max(1.0);
            let score = gain / (1.0 + d / 30.0);
            if best.map(|b| score > b.0).unwrap_or(true) { best = Some((score, ci)); }
        }
        let (best_score, best_idx) = best.unwrap();
        // Partners with nothing to exchange only rarely trade.
        if best_score < 0.5 && rng.gen::<f32>() > 0.15 { continue; }
        let (sid_b, loc_b, fid_b) = candidates[best_idx];
        
        // Check diplomatic stance - only block if hostile or at war
        let stance = history.factions.get(fid_a)
            .and_then(|f| f.relations.get(fid_b))
            .map(|r| r.stance.clone())
            .unwrap_or(DiplomaticStance::Neutral);
        
        if matches!(stance, DiplomaticStance::Hostile) {
            continue;
        }
        
        let at_war = history.factions.get(fid_a)
            .map_or(false, |f| f.is_at_war_with(*fid_b));
        if at_war {
            continue;
        }
        
        // Check if route already exists
        let route_exists = history.trade_routes.values()
            .any(|r| r.is_active() &&
                ((r.endpoints.0 == *sid_a && r.endpoints.1 == *sid_b) ||
                 (r.endpoints.0 == *sid_b && r.endpoints.1 == *sid_a)));
        if route_exists {
            continue;
        }
        
        // Find road-aware path (prefers existing roads)
        let path = find_trade_path(world, &history.tile_history, *loc_a, *loc_b);
        if path.is_empty() {
            continue;
        }
        
        // Carve roads on all path tiles (permanent infrastructure)
        for &(rx, ry) in &path {
            history.tile_history.build_road(rx, ry);
        }
        
        // Goods flow from where they are produced to where they are missing.
        let (_, mut traded) = trade_gain(history, *sid_a, *sid_b);
        traded.truncate(5);
        
        // If no complementary goods, just trade food (everyone trades food)
        if traded.is_empty() {
            traded.push(ResourceType::Food);
        }
        
        let route_id = history.id_generators.next_trade_route();
        let mut route = TradeRoute::new(route_id, *sid_a, *sid_b, date, traded.clone());
        route.path = path;
        
        // Add to faction trade routes
        if let Some(faction) = history.factions.get_mut(fid_a) {
            faction.trade_routes.push(route_id);
        }
        if let Some(faction) = history.factions.get_mut(fid_b) {
            faction.trade_routes.push(route_id);
        }
        
        // Improve relations from trade
        if let Some(faction) = history.factions.get_mut(fid_a) {
            let rel = faction.get_relation_mut(*fid_b, 0.0);
            rel.opinion = (rel.opinion + 5).min(100);
        }
        if let Some(faction) = history.factions.get_mut(fid_b) {
            let rel = faction.get_relation_mut(*fid_a, 0.0);
            rel.opinion = (rel.opinion + 5).min(100);
        }
        
        let name_a = history.settlements.get(sid_a).map(|s| s.name.clone()).unwrap_or_default();
        let name_b = history.settlements.get(sid_b).map(|s| s.name.clone()).unwrap_or_default();
        let goods_str: Vec<String> = traded.iter().map(|g| plain_words(&format!("{:?}", g))).collect();
        
        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::TradeRouteEstablished,
            date,
            format!("Trade route: {} ↔ {}", name_a, name_b),
            format!("A trade route was established between {} and {} for {}.",
                name_a, name_b, match goods_str.split_last() {
                    Some((last, rest)) if !rest.is_empty() => format!("{} and {}", rest.join(", "), last),
                    _ => goods_str.join(""),
                }),
        )
        .at_location(loc_a.0, loc_a.1)
        .with_faction(*fid_a)
        .with_faction(*fid_b)
        .with_participant(EntityId::Settlement(*sid_a))
        .with_participant(EntityId::Settlement(*sid_b));
        history.chronicle.record(event);
        
        history.trade_routes.insert(route_id, route);
        new_routes_this_step += 1;
    }
}

/// The road-building cost of each tile that doesn't change during the history (biome, rivers,
/// height, the waviness noise), or 0 where no road can go. Computed once per world: the river
/// query scans every river segment, and the trade A* used to ask it 80,000 times per route
/// (95% of a 512x256 history).
fn static_road_costs(world: &WorldData) -> std::sync::Arc<Vec<u32>> {
    use rayon::prelude::*;
    static CACHE: std::sync::Mutex<Option<(u64, std::sync::Arc<Vec<u32>>)>> = std::sync::Mutex::new(None);
    let (width, height) = (world.width, world.height);
    let mut key = world.seed() ^ ((width as u64) << 40) ^ ((height as u64) << 20);
    for i in (0..width * height).step_by(97) { key = key.rotate_left(7) ^ world.heightmap.get(i % width, i / width).to_bits() as u64; }
    if let Some((k, c)) = CACHE.lock().unwrap().as_ref() { if *k == key { return c.clone(); } }
    let cost = |x: usize, y: usize| -> u32 {
        use crate::biomes::ExtendedBiome;
        let h = *world.heightmap.get(x, y);
        let biome = *world.biomes.get(x, y);
        
        // Check for rivers - they add significant cost (bridges needed)
        let has_river = world.river_network.as_ref()
            .map_or(false, |rn| rn.has_significant_flow(x, y));
        let river_penalty = if has_river { 15 } else { 0 };

        // Impassable terrain
        if h > 3500.0 { return 0; } // Very high mountains (heights are metres)
        if matches!(biome, ExtendedBiome::Ocean | ExtendedBiome::DeepOcean | ExtendedBiome::AbyssalPlain) {
            return 0; // Deep water - impassable
        }
        if matches!(biome, ExtendedBiome::Ice) {
            return 0; // Frozen wastelands - impassable for roads
        }

        // Base cost by biome type (higher = harder to build roads)
        let base_cost = match biome {
            // Easy terrain - open land
            ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna |
            ExtendedBiome::Foothills | ExtendedBiome::MediterraneanShrubland => 4,
            
            // Moderate - some vegetation
            ExtendedBiome::TemperateForest | ExtendedBiome::TropicalForest |
            ExtendedBiome::MonsoonForest => 8,
            
            // Dense vegetation - harder
            ExtendedBiome::TropicalRainforest | ExtendedBiome::TemperateRainforest |
            ExtendedBiome::MangroveSaltmarsh => 12,
            
            // Coniferous forests - moderate difficulty
            ExtendedBiome::BorealForest | ExtendedBiome::MontaneForest |
            ExtendedBiome::SubalpineForest | ExtendedBiome::CloudForest => 10,
            
            // Wetlands - very difficult
            ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog |
            ExtendedBiome::SpiritMarsh | ExtendedBiome::CarnivorousBog => 18,
            
            // Arid regions - difficult
            ExtendedBiome::Desert | ExtendedBiome::SaltFlats | ExtendedBiome::Ashlands |
            ExtendedBiome::SingingDunes | ExtendedBiome::GlassDesert => 14,
            
            // Highland/mountain - very difficult
            ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineTundra | 
            ExtendedBiome::AlpineMeadow | ExtendedBiome::Paramo |
            ExtendedBiome::RazorPeaks => 20,
            
            // Volcanic - extremely difficult
            ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaField |
            ExtendedBiome::Caldera | ExtendedBiome::LavaLake => 25,
            
            // Tundra - harsh conditions
            ExtendedBiome::Tundra | ExtendedBiome::Ice |
            ExtendedBiome::AuroraWastes => 16,
            
            // Water - impassable
            ExtendedBiome::CoastalWater | ExtendedBiome::Ocean |
            ExtendedBiome::DeepOcean | ExtendedBiome::KelpForest |
            ExtendedBiome::CoralReef | ExtendedBiome::AbyssalPlain |
            ExtendedBiome::HighlandLake | ExtendedBiome::CraterLake | 
            ExtendedBiome::FrozenLake | ExtendedBiome::AcidLake |
            ExtendedBiome::Cenote | ExtendedBiome::Lagoon => 0,
            
            // Default for any unmapped biomes
            _ => 10,
        };
        
        // Add varying noise to make roads wavy instead of straight
        // Using sin/cos based on coordinates creates consistent "organic" curves
        let noise_val = (x as f32 * 0.15).sin() + (y as f32 * 0.25).cos();
        let noise_cost = (noise_val.abs() * 4.0) as u32;
        
        // Height penalty for hills (not mountains)
        let height_penalty = if h > 2000.0 { 8 } else if h > 1200.0 { 4 } else { 0 };
        
        base_cost + river_penalty + height_penalty + noise_cost
    };
    let costs: Vec<u32> = (0..width * height).into_par_iter().map(|i| cost(i % width, i / width)).collect();
    let costs = std::sync::Arc::new(costs);
    *CACHE.lock().unwrap() = Some((key, costs.clone()));
    costs
}

/// Find a path between two locations using A* that prefers existing roads.
/// Existing roads have much lower traversal cost, causing routes to converge.
fn find_trade_path(
    world: &WorldData,
    tile_history: &crate::history::world_state::tile_history::TileHistoryMap,
    from: (usize, usize),
    to: (usize, usize),
) -> Vec<(usize, usize)> {
    use std::collections::BinaryHeap;
use crate::history::det::HashMap;
    use std::cmp::Ordering;

    #[derive(Clone, Eq, PartialEq)]
    struct Node {
        pos: (usize, usize),
        cost: u32,
        heuristic: u32,
    }

    impl Ord for Node {
        fn cmp(&self, other: &Self) -> Ordering {
            (other.cost + other.heuristic).cmp(&(self.cost + self.heuristic))
        }
    }
    impl PartialOrd for Node {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let width = world.width;
    let height = world.height;

    // Terrain traversal cost - roads are MUCH cheaper (static part precomputed per world).
    let fixed = static_road_costs(world);
    let terrain_cost = |x: usize, y: usize| -> u32 {
        // Check if there's already a road - very cheap to use!
        if tile_history.has_road(x, y) {
            return 1; // Roads are super cheap - natural convergence
        }
        let base = fixed[y * width + x];
        if base == 0 { return 0; }
        // Check for parallel roads: if we are not a road, but adjacent to one,
        // apply a huge penalty. This forces paths to either merge onto the road
        // or stay at least 1 tile away, preventing double-width roads.
        let mut parallel_penalty = 0;
        'n: for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 && tile_history.has_road(nx as usize, ny as usize) {
                    parallel_penalty = 50;
                    break 'n;
                }
            }
        }
        base + parallel_penalty
    };

    let heuristic = |pos: (usize, usize)| -> u32 {
        let dx = (pos.0 as i64 - to.0 as i64).unsigned_abs() as u32;
        let dy = (pos.1 as i64 - to.1 as i64).unsigned_abs() as u32;
        dx + dy
    };

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::default();
    let mut g_score: HashMap<(usize, usize), u32> = HashMap::default();

    open.push(Node { pos: from, cost: 0, heuristic: heuristic(from) });
    g_score.insert(from, 0);

    let directions: [(i32, i32); 8] = [
        (-1, 0), (1, 0), (0, -1), (0, 1),
        (-1, -1), (-1, 1), (1, -1), (1, 1),
    ];

    let max_iterations = 10000; // Higher limit for longer paths
    let mut iterations = 0;

    while let Some(current) = open.pop() {
        iterations += 1;
        if iterations > max_iterations {
            // Fall back to simple straight line if A* fails
            return bresenham_line(from, to);
        }

        if current.pos == to {
            // Reconstruct path
            let mut path = vec![to];
            let mut curr = to;
            while let Some(&prev) = came_from.get(&curr) {
                path.push(prev);
                curr = prev;
            }
            path.reverse();
            return path;
        }

        let current_g = g_score.get(&current.pos).copied().unwrap_or(u32::MAX);

        for (dx, dy) in directions {
            let nx = current.pos.0 as i32 + dx;
            let ny = current.pos.1 as i32 + dy;

            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }

            let neighbor = (nx as usize, ny as usize);
            let cost = terrain_cost(neighbor.0, neighbor.1);
            if cost == 0 {
                continue; // Impassable
            }

            let tentative_g = current_g.saturating_add(cost);
            if tentative_g < g_score.get(&neighbor).copied().unwrap_or(u32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open.push(Node {
                    pos: neighbor,
                    cost: tentative_g,
                    heuristic: heuristic(neighbor),
                });
            }
        }
    }

    // Fall back to straight line if no path found
    bresenham_line(from, to)
}

/// Simple fallback path generator using Bresenham line algorithm.
fn bresenham_line(from: (usize, usize), to: (usize, usize)) -> Vec<(usize, usize)> {
    let mut path = Vec::new();
    
    let (mut x0, mut y0) = (from.0 as i64, from.1 as i64);
    let (x1, y1) = (to.0 as i64, to.1 as i64);
    
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    
    loop {
        path.push((x0 as usize, y0 as usize));
        
        if x0 == x1 && y0 == y1 {
            break;
        }
        
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
        
        if path.len() > 500 {
            break;
        }
    }
    
    path
}

// =========================================================================
// System 7: Alliance Wars & Treaty Breaking
// =========================================================================

/// Pull allies into existing wars (defensive pacts), break treaties when
/// opinion drops, dissolve alliances.
fn step_alliance_obligations(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    // --- 1. Call allies into wars ---
    let active_wars: Vec<(WarId, Vec<FactionId>, Vec<FactionId>)> = history.wars.values()
        .filter(|w| w.is_active())
        .map(|w| (w.id, w.aggressors.clone(), w.defenders.clone()))
        .collect();

    for (war_id, aggressors, defenders) in &active_wars {
        // For each defender, check if they have allies not yet in this war
        for &def_fid in defenders {
            let allies: Vec<FactionId> = history.factions.get(&def_fid)
                .map(|f| f.relations.iter()
                    .filter(|(_, r)| matches!(r.stance, DiplomaticStance::Allied))
                    .map(|(&fid, _)| fid)
                    .filter(|&aid| {
                        !defenders.contains(&aid) && !aggressors.contains(&aid)
                        && history.factions.get(&aid).map_or(false, |f| f.is_active())
                    })
                    .collect())
                .unwrap_or_default();

            for ally_fid in allies {
                // 30% chance per season to honor defensive pact
                if rng.gen::<f32>() >= 0.30 { continue; }
                // Already fighting the aggressor in another war: no second war with them.
                let already = history.factions.get(&ally_fid)
                    .map_or(false, |f| aggressors.iter().any(|&a| f.is_at_war_with(a)));
                if already { continue; }

                // Add ally to defenders
                if let Some(war) = history.wars.get_mut(war_id) {
                    if !war.defenders.contains(&ally_fid) {
                        war.defenders.push(ally_fid);
                    }
                }

                // Set ally at war with aggressors
                for &agg_fid in aggressors {
                    if let Some(faction) = history.factions.get_mut(&ally_fid) {
                        let rel = faction.get_relation_mut(agg_fid, 0.0);
                        rel.declare_war(*war_id);
                    }
                    if let Some(faction) = history.factions.get_mut(&agg_fid) {
                        let rel = faction.get_relation_mut(ally_fid, 0.0);
                        rel.declare_war(*war_id);
                    }
                }

                if let Some(faction) = history.factions.get_mut(&ally_fid) {
                    if !faction.wars.contains(war_id) {
                        faction.wars.push(*war_id);
                    }
                }

                let ally_name = history.factions.get(&ally_fid)
                    .map(|f| f.name.clone()).unwrap_or_default();
                let def_name = history.factions.get(&def_fid)
                    .map(|f| f.name.clone()).unwrap_or_default();

                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::WarDeclared,
                    date,
                    format!("{} joins war to defend {}", ally_name, def_name),
                    format!("{} honored their alliance with {} and joined the war.",
                        ally_name, def_name),
                )
                .with_faction(ally_fid)
                .with_faction(def_fid);
                history.chronicle.record(event);
            }
        }
    }

    // --- 2. Treaty breaking when opinion drops ---
    for &fid in &faction_ids {
        let relations_snapshot: Vec<(FactionId, i32, Vec<TreatyId>)> = history.factions.get(&fid)
            .map(|f| f.relations.iter()
                .filter(|(_, r)| !r.treaties.is_empty())
                .map(|(&other_fid, r)| (other_fid, r.opinion, r.treaties.clone()))
                .collect())
            .unwrap_or_default();

        for (other_fid, opinion, treaty_ids) in relations_snapshot {
            // Treaties break when opinion drops below -20
            if opinion >= -20 { continue; }

            // 5% chance per season to break each treaty
            for tid in &treaty_ids {
                if rng.gen::<f32>() >= 0.05 { continue; }

                // Mark treaty as broken
                let treaty_type = history.factions.get(&fid)
                    .and_then(|f| f.relations.get(&other_fid))
                    .and_then(|r| r.treaties.iter()
                        .find(|&&t| t == *tid)
                        .copied())
                    .and_then(|tid| {
                        // We don't have a standalone treaty store; record via event
                        Some(tid)
                    });

                if treaty_type.is_none() { continue; }

                // Remove treaty from both sides
                if let Some(faction) = history.factions.get_mut(&fid) {
                    if let Some(rel) = faction.relations.get_mut(&other_fid) {
                        rel.treaties.retain(|t| t != tid);
                    }
                }
                if let Some(faction) = history.factions.get_mut(&other_fid) {
                    if let Some(rel) = faction.relations.get_mut(&fid) {
                        rel.treaties.retain(|t| t != tid);
                    }
                }

                let name_a = history.factions.get(&fid)
                    .map(|f| f.name.clone()).unwrap_or_default();
                let name_b = history.factions.get(&other_fid)
                    .map(|f| f.name.clone()).unwrap_or_default();

                // Worsen opinion further
                if let Some(faction) = history.factions.get_mut(&other_fid) {
                    let rel = faction.get_relation_mut(fid, 0.0);
                    rel.adjust_opinion(-20);
                }

                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::TreatyBroken,
                    date,
                    format!("{} breaks treaty with {}", name_a, name_b),
                    format!("{} broke their treaty with {}, souring relations further.",
                        name_a, name_b),
                )
                .with_faction(fid)
                .with_faction(other_fid)
                .with_consequence(Consequence::RelationChange(other_fid, fid, -20));
                history.chronicle.record(event);

                break; // One treaty break per pair per step
            }
        }
    }

    // --- 3. Alliance dissolution when opinion drops ---
    for &fid in &faction_ids {
        let allied_with: Vec<(FactionId, i32)> = history.factions.get(&fid)
            .map(|f| f.relations.iter()
                .filter(|(_, r)| matches!(r.stance, DiplomaticStance::Allied))
                .map(|(&other_fid, r)| (other_fid, r.opinion))
                .collect())
            .unwrap_or_default();

        for (other_fid, opinion) in allied_with {
            // Alliance breaks if opinion drops below 30
            if opinion >= 30 { continue; }

            // Downgrade to Friendly
            if let Some(faction) = history.factions.get_mut(&fid) {
                if let Some(rel) = faction.relations.get_mut(&other_fid) {
                    rel.stance = DiplomaticStance::Friendly;
                }
            }
            if let Some(faction) = history.factions.get_mut(&other_fid) {
                if let Some(rel) = faction.relations.get_mut(&fid) {
                    rel.stance = DiplomaticStance::Friendly;
                }
            }

            let name_a = history.factions.get(&fid)
                .map(|f| f.name.clone()).unwrap_or_default();
            let name_b = history.factions.get(&other_fid)
                .map(|f| f.name.clone()).unwrap_or_default();

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::AllianceBroken,
                date,
                format!("Alliance between {} and {} dissolves", name_a, name_b),
                format!("The alliance between {} and {} has dissolved due to deteriorating relations.",
                    name_a, name_b),
            )
            .with_faction(fid)
            .with_faction(other_fid);
            history.chronicle.record(event);
        }
    }
}

// =========================================================================
// System 6: Hero Quest System
// =========================================================================

/// Heroes with skills can embark on quests: slay creatures, recover artifacts,
/// or explore ruins. Wires QuestBegun/QuestCompleted events, Figure.skills,
/// Figure.kills. Successful heroes earn epithets and dynasty prestige.
fn step_quests(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let rate = history.config.quest_rate;

    // --- 1. Launch new quests for idle heroes ---
    let heroes: Vec<(FigureId, FactionId, u8)> = history.figures.values()
        .filter(|f| f.is_alive() && f.active_quest.is_none() && f.faction.is_some())
        .filter(|f| {
            // Must have at least one skill >= 5
            f.skills.values().any(|&v| v >= 5)
        })
        .map(|f| {
            let best_skill = f.skills.values().max().copied().unwrap_or(0);
            (f.id, f.faction.unwrap(), best_skill)
        })
        .collect();

    for (hero_id, faction_id, skill_level) in &heroes {
        // Base 2% chance per season, scaled by skill and config rate
        let quest_chance = 0.02 * rate * (*skill_level as f32 / 10.0);
        if rng.gen::<f32>() >= quest_chance { continue; }

        let hero_name = history.figures.get(hero_id)
            .map(|f| f.full_name()).unwrap_or_default();
        let faction_name = history.factions.get(faction_id)
            .map(|f| f.name.clone()).unwrap_or_default();

        // Determine quest type based on world state
        // A young brood grows in hiding: heroes hear of a beast only once it is grown.
        let living_creatures: Vec<LegendaryCreatureId> = history.legendary_creatures.values()
            .filter(|c| c.is_alive())
            .filter(|c| c.birth_date.map_or(true, |b| b.year + BROOD_HIDING_YEARS <= date.year))
            .map(|c| c.id)
            .collect();
        let lost_artifacts: Vec<ArtifactId> = history.artifacts.values()
            .filter(|a| a.lost && !a.destroyed)
            .map(|a| a.id)
            .collect();

        #[derive(Clone, Copy)]
        enum QuestType { SlayCreature(LegendaryCreatureId), RecoverArtifact(ArtifactId), ExploreRuins }

        let quest = if !living_creatures.is_empty() && rng.gen::<f32>() < 0.4 {
            QuestType::SlayCreature(living_creatures[rng.gen_range(0..living_creatures.len())])
        } else if !lost_artifacts.is_empty() && rng.gen::<f32>() < 0.5 {
            QuestType::RecoverArtifact(lost_artifacts[rng.gen_range(0..lost_artifacts.len())])
        } else {
            QuestType::ExploreRuins
        };

        let (quest_desc, quest_title) = match quest {
            QuestType::SlayCreature(cid) => {
                let cname = history.legendary_creatures.get(&cid)
                    .map(|c| c.full_name()).unwrap_or_else(|| "a beast".to_string());
                (format!("{} of {} set out to slay {}.", hero_name, faction_name, cname),
                 format!("{} hunts {}", hero_name, cname))
            }
            QuestType::RecoverArtifact(aid) => {
                let aname = history.artifacts.get(&aid)
                    .map(|a| a.name.clone()).unwrap_or_else(|| "a lost artifact".to_string());
                (format!("{} of {} departed to recover the lost {}.", hero_name, faction_name, strip_the(&aname)),
                 format!("{} seeks {}", hero_name, aname))
            }
            QuestType::ExploreRuins => {
                (format!("{} of {} ventured into unknown ruins seeking glory.", hero_name, faction_name),
                 format!("{} explores ancient ruins", hero_name))
            }
        };

        let quest_event_id = history.id_generators.next_event();
        let event = Event::new(
            quest_event_id,
            EventType::QuestBegun,
            date,
            quest_title,
            quest_desc,
        )
        .with_faction(*faction_id)
        .with_participant(EntityId::Figure(*hero_id));
        // The quest's target takes part too: its own story (a beast's raids, a treasure's loss)
        // is why the hero set out.
        let event = match quest {
            QuestType::SlayCreature(cid) => event.with_participant(EntityId::LegendaryCreature(cid)),
            QuestType::RecoverArtifact(aid) => event.with_participant(EntityId::Artifact(aid)),
            QuestType::ExploreRuins => event,
        };
        history.chronicle.record(event);

        if let Some(fig) = history.figures.get_mut(hero_id) {
            fig.active_quest = Some(quest_event_id);
        }
    }

    // --- 2. Resolve active quests (1-4 seasons after start) ---
    let active_questers: Vec<(FigureId, EventId, Option<FactionId>, Option<DynastyId>)> = history.figures.values()
        .filter(|f| f.is_alive() && f.active_quest.is_some())
        .map(|f| (f.id, f.active_quest.unwrap(), f.faction, f.dynasty))
        .collect();

    for (hero_id, quest_event_id, faction, dynasty) in active_questers {
        // Check quest duration (resolve after 1-4 seasons, ~25% chance each step)
        if rng.gen::<f32>() >= 0.25 { continue; }

        let hero_name = history.figures.get(&hero_id)
            .map(|f| f.full_name()).unwrap_or_default();
        let best_skill = history.figures.get(&hero_id)
            .map(|f| f.skills.values().max().copied().unwrap_or(1) as f32)
            .unwrap_or(1.0);
        let bravery = history.figures.get(&hero_id)
            .map(|f| f.personality.bravery).unwrap_or(0.5);

        // Success chance scales with best skill and bravery
        let success_chance = (best_skill * 0.08 + bravery * 0.2).clamp(0.2, 0.75);
        let success = rng.gen::<f32>() < success_chance;

        // Clear the quest
        if let Some(fig) = history.figures.get_mut(&hero_id) {
            fig.active_quest = None;
        }

        if success {
            // Determine reward based on quest type
            // Try to find what this quest was about from the original event
            let quest_involved_creature = history.chronicle.events.get(quest_event_id.0 as usize)
                .map(|e| e.title.contains("hunts"))
                .unwrap_or(false);
            let quest_involved_artifact = history.chronicle.events.get(quest_event_id.0 as usize)
                .map(|e| e.title.contains("seeks"))
                .unwrap_or(false);

            let desc;
            let mut title = format!("{} completes quest", hero_name);

            if quest_involved_creature {
                // Find a living creature to kill
                let victim = history.legendary_creatures.values()
                    .filter(|c| c.is_alive())
                    .map(|c| c.id)
                    .next();
                if let Some(cid) = victim {
                    let cname = history.legendary_creatures.get(&cid)
                        .map(|c| c.full_name()).unwrap_or_default();
                    if let Some(creature) = history.legendary_creatures.get_mut(&cid) {
                        creature.kill(date);
                    }
                    if let Some(fig) = history.figures.get_mut(&hero_id) {
                        fig.kills.push(EntityId::LegendaryCreature(cid));
                    }
                    title = format!("{} slays {}", hero_name, cname);
                    desc = format!("{} slew the legendary {} and returned victorious.", hero_name, cname);

                    // Record creature slain event
                    let slay_event_id = history.id_generators.next_event();
                    let mut slay_event = Event::new(
                        slay_event_id,
                        EventType::CreatureSlain,
                        date,
                        format!("{} slain by {}", cname, hero_name),
                        format!("The legendary {} was slain by {}.", cname, hero_name),
                    )
                    .with_participant(EntityId::Figure(hero_id))
                    .with_participant(EntityId::LegendaryCreature(cid))
                    .caused_by(quest_event_id);
                    slay_event.is_major = true;
                    if let Some(fid) = faction {
                        let slay_event = slay_event.with_faction(fid);
                        history.chronicle.record(slay_event);
                    } else {
                        history.chronicle.record(slay_event);
                    }
                } else {
                    desc = format!("{} returned from the hunt with tales of glory.", hero_name);
                }
            } else if quest_involved_artifact {
                // Find a lost artifact to recover
                let found = history.artifacts.values()
                    .filter(|a| a.lost && !a.destroyed)
                    .map(|a| a.id)
                    .next();
                if let Some(aid) = found {
                    let aname = history.artifacts.get(&aid)
                        .map(|a| a.name.clone()).unwrap_or_default();
                    if let Some(artifact) = history.artifacts.get_mut(&aid) {
                        artifact.transfer_to(EntityId::Figure(hero_id), date, AcquisitionMethod::Found);
                        artifact.historical_importance += 3;
                    }
                    if let Some(fig) = history.figures.get_mut(&hero_id) {
                        fig.artifacts.push(aid);
                    }
                    title = format!("{} recovers {}", hero_name, aname);
                    desc = format!("{} recovered the lost {} and returned in triumph.", hero_name, strip_the(&aname));

                    let find_event_id = history.id_generators.next_event();
                    let mut find_event = Event::new(
                        find_event_id,
                        EventType::ArtifactFound,
                        date,
                        format!("{} recovered", aname),
                        format!("{} found and recovered the lost {}.", hero_name, strip_the(&aname)),
                    )
                    .with_participant(EntityId::Figure(hero_id))
                    .with_participant(EntityId::Artifact(aid))
                    .caused_by(quest_event_id);
                    find_event.is_major = true;
                    if let Some(fid) = faction {
                        let find_event = find_event.with_faction(fid);
                        history.chronicle.record(find_event);
                    } else {
                        history.chronicle.record(find_event);
                    }
                } else {
                    desc = format!("{} returned from the search with ancient knowledge.", hero_name);
                }
            } else {
                desc = format!("{} returned from exploring ancient ruins with valuable secrets.", hero_name);
            }

            // Grant epithet if hero doesn't have one yet
            if history.figures.get(&hero_id).map_or(false, |f| f.epithet.is_none()) {
                let epithets = [
                    "the Bold", "the Brave", "the Seeker", "the Valiant",
                    "the Fearless", "the Wanderer", "the Slayer", "the Unyielding",
                    "Dragon-Bane", "the Relentless", "the Undaunted", "the Proven",
                ];
                let epithet = epithets[rng.gen_range(0..epithets.len())];
                if let Some(fig) = history.figures.get_mut(&hero_id) {
                    fig.epithet = Some(epithet.to_string());
                }
            }

            // Boost skills
            if let Some(fig) = history.figures.get_mut(&hero_id) {
                let skill = *fig.skills.keys().next().unwrap_or(&Skill::Combat);
                let current = fig.skills.get(&skill).copied().unwrap_or(0);
                fig.skills.insert(skill, (current + 1).min(10));
            }

            // Dynasty prestige
            if let Some(did) = dynasty {
                if let Some(dynasty) = history.dynasties.get_mut(&did) {
                    dynasty.prestige += 5;
                }
            }

            let comp_event_id = history.id_generators.next_event();
            let mut comp_event = Event::new(
                comp_event_id,
                EventType::QuestCompleted,
                date,
                title,
                desc,
            )
            .with_participant(EntityId::Figure(hero_id))
            .caused_by(quest_event_id);
            if let Some(fid) = faction {
                comp_event = comp_event.with_faction(fid);
            }
            history.chronicle.record(comp_event);
        } else {
            // Quest failed — hero may die (15%)
            let hero_dies = rng.gen::<f32>() < 0.15;
            if hero_dies {
                if let Some(fig) = history.figures.get_mut(&hero_id) {
                    fig.kill(date, DeathCause::Monster);
                }

                let event_id = history.id_generators.next_event();
                let mut event = Event::new(
                    event_id,
                    EventType::HeroDied,
                    date,
                    format!("{} perishes on quest", hero_name),
                    format!("{} died during a perilous quest and was never seen again.", hero_name),
                )
                .with_participant(EntityId::Figure(hero_id))
                .caused_by(quest_event_id);
                event.is_major = true;
                if let Some(fid) = faction {
                    event = event.with_faction(fid);
                }
                history.chronicle.record(event);
            } else {
                let event_id = history.id_generators.next_event();
                let mut event = Event::new(
                    event_id,
                    EventType::QuestCompleted,
                    date,
                    format!("{} returns empty-handed", hero_name),
                    format!("{} returned from the quest having failed in the endeavor.", hero_name),
                )
                .with_participant(EntityId::Figure(hero_id))
                .caused_by(quest_event_id);
                if let Some(fid) = faction {
                    event = event.with_faction(fid);
                }
                history.chronicle.record(event);
            }
        }
    }
}

// =========================================================================
// System 5: Assassination & Intrigue
// =========================================================================

/// Cunning leaders may attempt to assassinate enemy faction leaders.
/// Uses Skill::Stealth, Personality.cunning/paranoia. Wires Assassination
/// event, DeathCause::Assassination, Figure.enemies, Figure.kills.
fn step_assassination(history: &mut WorldHistory, game_data: &GameData, rng: &mut impl Rng) {
    let date = history.current_date;
    let rate = history.config.assassination_rate;

    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    for &fid in &faction_ids {
        // Only cunning leaders attempt assassinations
        let cunning = leader_personality(history, fid)
            .map(|p| p.cunning)
            .unwrap_or(0.0);
        if cunning < 0.5 { continue; }

        // Base 0.3% per season, scaled by cunning and config rate
        let attempt_chance = 0.003 * rate * Personality::score_to_multiplier(cunning, 0.5, 3.0);
        if rng.gen::<f32>() >= attempt_chance { continue; }

        // Pick an enemy faction (at war or hostile)
        let enemies: Vec<FactionId> = history.factions.get(&fid)
            .map(|f| f.relations.iter()
                .filter(|(_, r)| r.stance.is_at_war() || matches!(r.stance, DiplomaticStance::Hostile))
                .filter(|(eid, _)| history.factions.get(eid).map_or(false, |ef| ef.is_active() && ef.current_leader.is_some()))
                .map(|(&eid, _)| eid)
                .collect())
            .unwrap_or_default();

        if enemies.is_empty() { continue; }
        let target_faction = enemies[rng.gen_range(0..enemies.len())];

        let target_leader_id = match history.factions.get(&target_faction)
            .and_then(|f| f.current_leader) {
            Some(id) => id,
            None => continue,
        };

        // Find an assassin: notable figure with Stealth skill, or the leader themselves
        let assassin_id = history.factions.get(&fid)
            .map(|f| f.notable_figures.iter()
                .filter(|&&nfid| history.figures.get(&nfid).map_or(false, |fig| {
                    fig.is_alive() && fig.skills.get(&Skill::Stealth).copied().unwrap_or(0) >= 3
                }))
                .copied()
                .next()
                .unwrap_or_else(|| f.current_leader.unwrap_or(FigureId(0))))
            .unwrap_or(FigureId(0));

        // Success chance: assassin cunning vs target paranoia
        let assassin_cunning = history.figures.get(&assassin_id)
            .map(|f| f.personality.cunning).unwrap_or(0.5);
        let assassin_stealth = history.figures.get(&assassin_id)
            .and_then(|f| f.skills.get(&Skill::Stealth))
            .copied().unwrap_or(0) as f32;
        let target_paranoia = history.figures.get(&target_leader_id)
            .map(|f| f.personality.paranoia).unwrap_or(0.5);

        let success_chance = (assassin_cunning * 0.3 + assassin_stealth * 0.05)
            / (1.0 + target_paranoia);
        let success = rng.gen::<f32>() < success_chance.clamp(0.05, 0.6);

        let assassin_name = history.figures.get(&assassin_id)
            .map(|f| f.full_name()).unwrap_or_default();
        let target_name = history.figures.get(&target_leader_id)
            .map(|f| f.full_name()).unwrap_or_default();
        let att_faction_name = history.factions.get(&fid)
            .map(|f| f.name.clone()).unwrap_or_default();
        let def_faction_name = history.factions.get(&target_faction)
            .map(|f| f.name.clone()).unwrap_or_default();

        if success {
            // Kill the target
            if let Some(fig) = history.figures.get_mut(&target_leader_id) {
                fig.kill(date, DeathCause::Assassination);
            }
            // Record kill on assassin
            if let Some(fig) = history.figures.get_mut(&assassin_id) {
                fig.kills.push(EntityId::Figure(target_leader_id));
            }
            // Make them enemies
            if let Some(fig) = history.figures.get_mut(&assassin_id) {
                if !fig.enemies.contains(&target_leader_id) {
                    fig.enemies.push(target_leader_id);
                }
            }

            // Worsen relations
            if let Some(faction) = history.factions.get_mut(&target_faction) {
                let rel = faction.get_relation_mut(fid, 0.0);
                rel.adjust_opinion(-30);
            }

            let event_id = history.id_generators.next_event();
            let mut event = Event::new(
                event_id,
                EventType::Assassination,
                date,
                format!("Assassination of {}", target_name),
                format!("{} of {} was assassinated by an agent of {}.",
                    target_name, def_faction_name, att_faction_name),
            )
            .with_faction(fid)
            .with_faction(target_faction)
            .with_participant(EntityId::Figure(target_leader_id))
            .with_participant(EntityId::Figure(assassin_id))
            .with_consequence(Consequence::FigureDeath(target_leader_id, DeathCause::Assassination))
            .with_consequence(Consequence::RelationChange(target_faction, fid, -30));
            event.is_major = true;
            history.chronicle.record(event);
        } else {
            // Failed attempt: assassin may die (40%), relations worsen
            let assassin_caught = rng.gen::<f32>() < 0.4;
            if assassin_caught {
                if let Some(fig) = history.figures.get_mut(&assassin_id) {
                    fig.kill(date, DeathCause::Execution);
                }
            }

            if let Some(faction) = history.factions.get_mut(&target_faction) {
                let rel = faction.get_relation_mut(fid, 0.0);
                rel.adjust_opinion(-15);
            }

            let event_id = history.id_generators.next_event();
            let desc = if assassin_caught {
                format!("An assassination attempt on {} by {} was foiled. The assassin {} was caught and executed.",
                    target_name, att_faction_name, assassin_name)
            } else {
                format!("An assassination attempt on {} by {} was foiled. The assassin escaped.",
                    target_name, att_faction_name)
            };
            let event = Event::new(
                event_id,
                EventType::Assassination,
                date,
                format!("Failed assassination of {}", target_name),
                desc,
            )
            .with_faction(fid)
            .with_faction(target_faction)
            .with_participant(EntityId::Figure(target_leader_id))
            .with_consequence(Consequence::RelationChange(target_faction, fid, -15));
            history.chronicle.record(event);
        }
    }
}

// =========================================================================
// System 3: Siege Warfare
// =========================================================================

/// Process active sieges: attrition each season, resolve when defender breaks
/// or attacker gives up. Successful sieges transfer settlements and may
/// destroy monuments.
fn step_sieges(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let siege_duration_mult = history.config.siege_duration;

    let active_siege_ids: Vec<SiegeId> = history.sieges.keys()
        .copied()
        .filter(|id| history.sieges.get(id).map_or(false, |s| s.is_active()))
        .collect();

    for siege_id in active_siege_ids {
        // Get siege data
        let (attacker, defender, target, att_str, def_str, war_id, duration) = {
            let siege = match history.sieges.get(&siege_id) {
                Some(s) => s,
                None => continue,
            };
            (siege.attacker, siege.defender, siege.target,
             siege.attacker_strength, siege.defender_strength,
             siege.war_id, siege.duration_seasons(&date))
        };

        // Sieges are the victor's conquests, launched as the war ends, so they carry on after
        // it. They are lifted only if the war ended in the defender's favour (or nobody's).
        let war_open = history.wars.get(&war_id).map_or(false, |w| w.is_active());
        let victor_is_attacker = history.wars.get(&war_id).map_or(false, |w| w.victor == Some(attacker));
        let war_active = war_open || victor_is_attacker;
        let sides_alive = [attacker, defender].iter()
            .all(|f| history.factions.get(f).map_or(false, |x| x.is_active()));
        if !war_active || !sides_alive {
            if let Some(siege) = history.sieges.get_mut(&siege_id) {
                siege.end(date, false);
            }
            continue;
        }

        // Attrition: each season both sides take losses
        let att_attrition = rng.gen_range(5..25);
        let def_attrition = rng.gen_range(2..15);

        if let Some(faction) = history.factions.get_mut(&attacker) {
            faction.total_population = faction.total_population.saturating_sub(att_attrition);
        }
        if let Some(settlement) = history.settlements.get_mut(&target) {
            settlement.population = settlement.population.saturating_sub(def_attrition);
        }
        if let Some(faction) = history.factions.get_mut(&defender) {
            faction.total_population = faction.total_population.saturating_sub(def_attrition);
        }
        if let Some(siege) = history.sieges.get_mut(&siege_id) {
            siege.attrition_days += 1;
        }

        // Resolve chance increases with duration
        // Base: 10% per season, +5% per additional season, scaled by siege_duration config
        let base_resolve = 0.10 + (duration as f32 * 0.05);
        let resolve_chance = base_resolve / siege_duration_mult;

        if rng.gen::<f32>() >= resolve_chance {
            continue;
        }

        // Determine outcome: attacker wins if their strength > defender's defense
        let att_effective = att_str as f32 * rng.gen_range(0.6..1.4);
        let def_effective = def_str as f32 * rng.gen_range(0.6..1.2);
        let success = att_effective > def_effective;

        if let Some(siege) = history.sieges.get_mut(&siege_id) {
            siege.end(date, success);
        }

        let target_name = history.settlements.get(&target)
            .map(|s| s.name.clone()).unwrap_or_default();
        let att_name = history.factions.get(&attacker)
            .map(|f| f.name.clone()).unwrap_or_default();
        let def_name = history.factions.get(&defender)
            .map(|f| f.name.clone()).unwrap_or_default();

        // Its ending (taken, razed or lifted) is caused by the siege's beginning.
        let siege_began = history.sieges.get(&siege_id).and_then(|s| s.begin_event);
        let because = |e: Event| match siege_began { Some(b) => e.caused_by(b), None => e };

        // Some conquerors raze what they take; capitals are kept as prizes.
        let is_capital = history.settlements.get(&target)
            .map_or(false, |s| s.settlement_type == SettlementType::Capital);
        let razed = success && !is_capital && rng.gen::<f32>() < RAZE_CHANCE;

        if razed {
            if let Some(loser_f) = history.factions.get_mut(&defender) {
                loser_f.remove_settlement(target);
            }
            let loc = destroy_settlement(history, target, date);
            let event_id = history.id_generators.next_event();
            let mut event = Event::new(
                event_id,
                EventType::SettlementDestroyed,
                date,
                format!("{} razed by {}", target_name, att_name),
                format!("{} stormed {} after a siege of {} and burned it to the ground.",
                    att_name, target_name, seasons_text(duration)),
            )
            .with_faction(attacker)
            .with_faction(defender)
            .with_participant(EntityId::Settlement(target));
            event = because(event);
            if let Some((x, y)) = loc {
                event = event.at_location(x, y);
                history.tile_history.record_event(x, y, event_id);
            }
            history.chronicle.record(event);
        }

        if success && !razed {
            // Transfer settlement to attacker
            if let Some(loser_f) = history.factions.get_mut(&defender) {
                loser_f.remove_settlement(target);
            }
            if let Some(victor_f) = history.factions.get_mut(&attacker) {
                victor_f.add_settlement(target);
            }
            if let Some(settlement) = history.settlements.get_mut(&target) {
                settlement.faction = attacker;
            }

            // Monument destruction during siege (30% chance per monument)
            let monument_ids: Vec<MonumentId> = history.settlements.get(&target)
                .map(|s| s.monuments.clone())
                .unwrap_or_default();
            for mon_id in &monument_ids {
                if rng.gen::<f32>() < 0.3 {
                    if let Some(monument) = history.monuments.get_mut(mon_id) {
                        if monument.intact {
                            monument.intact = false;
                            monument.destruction_date = Some(date);

                            let mon_name = monument.name.clone();
                            let event_id = history.id_generators.next_event();
                            let event = Event::new(
                                event_id,
                                EventType::MonumentDestroyed,
                                date,
                                format!("{} destroyed in siege", mon_name),
                                format!("{} was destroyed during the siege of {}.",
                                    mon_name, target_name),
                            )
                            .at_location(monument.location.0, monument.location.1)
                            .with_faction(attacker)
                            .with_faction(defender);
                            if let Some(monument) = history.monuments.get_mut(mon_id) {
                                monument.destruction_event = Some(event_id);
                            }
                            history.chronicle.record(event);
                        }
                    }
                }
            }

            // Dissolve defender if they lost all settlements
            let def_settlements = history.factions.get(&defender)
                .map(|f| f.settlements.len()).unwrap_or(0);
            let def_active = history.factions.get(&defender).map_or(false, |f| f.is_active());
            if def_settlements == 0 && def_active {
                if let Some(def_f) = history.factions.get_mut(&defender) {
                    def_f.dissolve(date);
                }
                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::FactionDestroyed,
                    date,
                    format!("{} destroyed", def_name),
                    format!("{} has been destroyed after losing their last settlement.", def_name),
                )
                .with_faction(defender);
                // The siege that took their last town.
                let event = match history.sieges.get(&siege_id).and_then(|s| s.begin_event) { Some(b) => event.caused_by(b), None => event };
                history.chronicle.record(event);
            }

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::SiegeEnded,
                date,
                format!("{} falls to {}", target_name, att_name),
                format!("{} captured {} after a siege of {}.",
                    att_name, target_name, seasons_text(duration)),
            )
            .with_faction(attacker)
            .with_faction(defender)
            .with_participant(EntityId::Settlement(target));
            history.chronicle.record(because(event));
        } else if razed {
            dissolve_if_landless(history, defender, &def_name, date);
        } else {
            // Siege failed — attacker withdraws
            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::SiegeEnded,
                date,
                format!("Siege of {} lifted", target_name),
                format!("{} withdrew from the siege of {} after {}.",
                    att_name, target_name, seasons_text(duration)),
            )
            .with_faction(attacker)
            .with_faction(defender)
            .with_participant(EntityId::Settlement(target));
            history.chronicle.record(because(event));
        }
    }
}

// =========================================================================
// System 2: Artifact Lifecycle
// =========================================================================

/// Handles artifact inheritance on leader death, loss in battle, creature
/// hoarding, and destruction. Wires ArtifactLost/Found/Destroyed events,
/// ArtifactTransfer consequences, LegendaryCreature.artifacts_owned,
/// Dynasty.heirlooms, and Artifact.historical_importance.
fn step_artifact_lifecycle(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    // --- 1. Inherit artifacts from dead leaders to their successors ---
    // Find figures who just died (death_date == current date) and had artifacts
    let recently_dead: Vec<(FigureId, Vec<ArtifactId>, Option<FactionId>, Option<DynastyId>)> =
        history.figures.values()
            .filter(|f| f.death_date == Some(date) && !f.artifacts.is_empty())
            .map(|f| (f.id, f.artifacts.clone(), f.faction, f.dynasty))
            .collect();

    for (dead_fig_id, artifact_ids, faction, dynasty) in recently_dead {
        let dead_name = history.figures.get(&dead_fig_id)
            .map(|f| f.full_name()).unwrap_or_default();

        for art_id in &artifact_ids {
            let art_available = history.artifacts.get(art_id)
                .map_or(false, |a| a.is_available());
            if !art_available { continue; }

            // Try to find the new faction leader as inheritor
            let new_owner = faction.and_then(|fid| {
                history.factions.get(&fid)
                    .and_then(|f| f.current_leader)
                    .filter(|&lid| lid != dead_fig_id && history.figures.get(&lid).map_or(false, |f| f.is_alive()))
            });

            if let Some(heir_id) = new_owner {
                // Transfer artifact to new leader
                if let Some(artifact) = history.artifacts.get_mut(art_id) {
                    artifact.transfer_to(EntityId::Figure(heir_id), date, AcquisitionMethod::Inherited);
                    artifact.historical_importance += 1;
                }
                if let Some(heir) = history.figures.get_mut(&heir_id) {
                    if !heir.artifacts.contains(art_id) {
                        heir.artifacts.push(*art_id);
                    }
                }

                // Add to dynasty heirlooms if applicable
                if let Some(did) = dynasty {
                    if let Some(dynasty) = history.dynasties.get_mut(&did) {
                        if !dynasty.heirlooms.contains(art_id) {
                            dynasty.heirlooms.push(*art_id);
                        }
                        dynasty.prestige += 2;
                    }
                }

                let heir_name = history.figures.get(&heir_id)
                    .map(|f| f.full_name()).unwrap_or_default();
                let art_name = history.artifacts.get(art_id)
                    .map(|a| a.name.clone()).unwrap_or_default();

                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::ArtifactFound,
                    date,
                    format!("{} inherits {}", heir_name, art_name),
                    format!("{} inherited {} after the death of {}.",
                        heir_name, art_name, dead_name),
                )
                .with_participant(EntityId::Figure(heir_id))
                .with_participant(EntityId::Figure(dead_fig_id))
                .with_participant(EntityId::Artifact(*art_id))
                .with_consequence(Consequence::ArtifactTransfer(
                    *art_id, EntityId::Figure(dead_fig_id), EntityId::Figure(heir_id),
                ));
                if let Some(fid) = faction {
                    let event = event.with_faction(fid);
                    history.chronicle.record(event);
                } else {
                    history.chronicle.record(event);
                }
            } else {
                // No heir found — artifact is lost
                if let Some(artifact) = history.artifacts.get_mut(art_id) {
                    artifact.lose(date);
                }

                let art_name = history.artifacts.get(art_id)
                    .map(|a| a.name.clone()).unwrap_or_default();

                let event_id = history.id_generators.next_event();
                let mut event = Event::new(
                    event_id,
                    EventType::ArtifactLost,
                    date,
                    format!("{} is lost", art_name),
                    format!("{} was lost after the death of {}.", art_name, dead_name),
                )
                .with_participant(EntityId::Figure(dead_fig_id))
                .with_participant(EntityId::Artifact(*art_id));
                if let Some(fid) = faction {
                    event = event.with_faction(fid);
                }
                history.chronicle.record(event);
            }
        }

        // Remove artifacts from the dead figure's list
        if let Some(fig) = history.figures.get_mut(&dead_fig_id) {
            fig.artifacts.clear();
        }
    }

    // --- 2. Legendary creatures hoard artifacts ---
    // Living creatures near lost artifacts pick them up (1% chance per step)
    let lost_artifacts: Vec<(ArtifactId, Option<(usize, usize)>)> = history.artifacts.values()
        .filter(|a| a.lost && !a.destroyed)
        .map(|a| (a.id, a.current_location))
        .collect();

    let creatures: Vec<(LegendaryCreatureId, Option<(usize, usize)>)> = history.legendary_creatures.values()
        .filter(|c| c.is_alive())
        .map(|c| (c.id, c.lair_location))
        .collect();

    for (art_id, art_loc) in &lost_artifacts {
        if rng.gen::<f32>() >= 0.01 { continue; }

        // Find a creature near the artifact (or any creature if location unknown)
        let finder = if let Some((ax, ay)) = art_loc {
            creatures.iter()
                .filter(|(_, loc)| {
                    if let Some((cx, cy)) = loc {
                        let dx = *ax as i64 - *cx as i64;
                        let dy = *ay as i64 - *cy as i64;
                        dx * dx + dy * dy < 900 // Within ~30 tiles
                    } else {
                        false
                    }
                })
                .map(|(cid, _)| *cid)
                .next()
        } else if !creatures.is_empty() {
            Some(creatures[rng.gen_range(0..creatures.len())].0)
        } else {
            None
        };

        if let Some(cid) = finder {
            if let Some(artifact) = history.artifacts.get_mut(art_id) {
                artifact.transfer_to(
                    EntityId::LegendaryCreature(cid), date, AcquisitionMethod::Found,
                );
                artifact.historical_importance += 2;
            }
            if let Some(creature) = history.legendary_creatures.get_mut(&cid) {
                if !creature.artifacts_owned.contains(art_id) {
                    creature.artifacts_owned.push(*art_id);
                }
            }

            let creature_name = history.legendary_creatures.get(&cid)
                .map(|c| c.full_name()).unwrap_or_default();
            let art_name = history.artifacts.get(art_id)
                .map(|a| a.name.clone()).unwrap_or_default();

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::ArtifactFound,
                date,
                format!("{} claims {}", creature_name, art_name),
                format!("{} added {} to its hoard.", creature_name, art_name),
            )
            .with_participant(EntityId::LegendaryCreature(cid))
            .with_participant(EntityId::Artifact(*art_id));
            history.chronicle.record(event);
        }
    }

    // --- 3. Artifact destruction (very rare, 0.05% per artifact per step) ---
    // The Shadow's bane (importance 1000) cannot be destroyed: what wounded it once must remain.
    let owned_artifacts: Vec<(ArtifactId, ArtifactQuality)> = history.artifacts.values()
        .filter(|a| !a.destroyed && !a.lost && a.historical_importance < 1000)
        .map(|a| (a.id, a.quality))
        .collect();

    for (art_id, quality) in owned_artifacts {
        // Higher quality artifacts are more durable
        let destroy_chance = match quality {
            ArtifactQuality::Fine => 0.001,
            ArtifactQuality::Superior => 0.0005,
            ArtifactQuality::Masterwork => 0.0002,
            ArtifactQuality::Legendary => 0.0001,
            ArtifactQuality::Divine => 0.00005,
        };

        if rng.gen::<f32>() < destroy_chance {
            let art_name = history.artifacts.get(&art_id)
                .map(|a| a.name.clone()).unwrap_or_default();

            if let Some(artifact) = history.artifacts.get_mut(&art_id) {
                artifact.destroy(date);
            }

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::ArtifactDestroyed,
                date,
                format!("{} destroyed", art_name),
                format!("{} was destroyed, lost to history forever.", art_name),
            )
            .with_participant(EntityId::Artifact(art_id));
            history.chronicle.record(event);
        }
    }
}

// =========================================================================
// System 1: Trade & Wealth
// =========================================================================

/// Per-season wealth tick: settlement income, trade revenue, war costs.
/// Wealth boosts military strength and artifact creation rates.
/// Wars reduce trade route safety; unsafe routes dissolve.
fn step_wealth_tick(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;

    let faction_ids: Vec<FactionId> = history.factions.keys()
        .copied()
        .filter(|id| history.factions.get(id).map_or(false, |f| f.is_active()))
        .collect();

    for &fid in &faction_ids {
        let settlement_count = history.factions.get(&fid)
            .map(|f| f.settlements.len() as u32)
            .unwrap_or(0);
        let total_pop = history.factions.get(&fid)
            .map(|f| f.total_population)
            .unwrap_or(0);

        // --- Base income from settlements ---
        // Each settlement generates wealth proportional to population
        // Extraction: what the land yields (ore, grain, timber, fish) times the people to work it.
        let extraction: f32 = history.factions.get(&fid)
            .map(|f| f.settlements.iter()
                .filter_map(|sid| history.settlements.get(sid))
                .filter(|st| !st.is_destroyed())
                .map(|st| st.production.iter().map(|(k, q)| *q * k.base_value() as f32).sum::<f32>() * (st.population as f32 / 400.0).sqrt())
                .sum())
            .unwrap_or(0.0);
        let base_income = (total_pop / 100).max(settlement_count) + (extraction * 0.6) as u32;

        // Wealth drive personality bonus: greedy/ambitious leaders extract more wealth
        let wealth_mult = leader_personality(history, fid)
            .map(|p| Personality::score_to_multiplier(p.wealth_drive(), 0.5, 2.0))
            .unwrap_or(1.0);
        let income = (base_income as f32 * wealth_mult) as u32;

        // --- Trade route revenue ---
        let trade_route_ids: Vec<TradeRouteId> = history.factions.get(&fid)
            .map(|f| f.trade_routes.clone())
            .unwrap_or_default();

        let mut trade_revenue: u32 = 0;
        for &trid in &trade_route_ids {
            if let Some(route) = history.trade_routes.get(&trid) {
                if route.is_active() {
                    // Revenue = route value * safety
                    trade_revenue += (route.value as f32 * route.safety) as u32;
                }
            }
        }

        // --- War costs ---
        let active_wars = history.factions.get(&fid)
            .map(|f| f.active_war_count() as u32)
            .unwrap_or(0);
        // Each war costs wealth proportional to military strength
        let mil_strength = history.factions.get(&fid)
            .map(|f| f.military_strength)
            .unwrap_or(0);
        let war_cost = active_wars * (mil_strength / 10 + 20);

        // --- Apply wealth changes ---
        if let Some(faction) = history.factions.get_mut(&fid) {
            faction.wealth = faction.wealth
                .saturating_add(income)
                .saturating_add(trade_revenue)
                .saturating_sub(war_cost);

            // Wealth boosts military strength (can afford larger armies)
            // Military = population/5 + wealth/50
            let pop_mil = faction.total_population / 5;
            let wealth_mil = faction.wealth / 50;
            faction.military_strength = pop_mil + wealth_mil;
        }

        // --- War reduces trade route safety ---
        if active_wars > 0 {
            for &trid in &trade_route_ids {
                if let Some(route) = history.trade_routes.get_mut(&trid) {
                    if route.is_active() {
                        // Each war reduces safety by 5-15%
                        let safety_loss = active_wars as f32 * rng.gen_range(0.05..0.15);
                        route.safety = (route.safety - safety_loss).max(0.0);

                        // Dissolve unsafe routes (safety < 0.15)
                        if route.safety < 0.15 {
                            route.dissolve(date);
                        }
                    }
                }
            }
        } else {
            // Peace slowly restores safety
            for &trid in &trade_route_ids {
                if let Some(route) = history.trade_routes.get_mut(&trid) {
                    if route.is_active() && route.safety < 1.0 {
                        route.safety = (route.safety + 0.02).min(1.0);
                    }
                }
            }
        }
    }

    // Clean up dissolved trade routes from faction lists
    for &fid in &faction_ids {
        let dissolved: Vec<TradeRouteId> = history.factions.get(&fid)
            .map(|f| f.trade_routes.iter()
                .filter(|trid| history.trade_routes.get(trid).map_or(true, |r| !r.is_active()))
                .copied()
                .collect())
            .unwrap_or_default();

        if !dissolved.is_empty() {
            if let Some(faction) = history.factions.get_mut(&fid) {
                faction.trade_routes.retain(|trid| !dissolved.contains(trid));
            }
        }
    }
}

/// Get a naming style for a race by looking up its base type's naming archetype.
/// If game_data has a matching archetype, builds a NamingStyle from the data;
/// otherwise falls back to the hardcoded archetype.
/// The naming style of a race (for other modules creating figures).
pub(crate) fn naming_style_for(history: &WorldHistory, race_id: RaceId, game_data: &GameData) -> NamingStyle {
    naming_style_for_race(history, race_id, game_data)
}

fn naming_style_for_race(history: &WorldHistory, race_id: RaceId, game_data: &GameData) -> NamingStyle {
    let race = history.races.get(&race_id);
    let tag = race.map(|r| r.base_type.tag()).unwrap_or("human");

    // Try to get archetype name from game data
    let archetype_name = game_data.race(tag)
        .map(|r| r.naming_archetype.as_str())
        .unwrap_or_else(|| {
            race.map(|r| match r.base_type.default_naming_archetype() {
                crate::history::naming::styles::NamingArchetype::Harsh => "Harsh",
                crate::history::naming::styles::NamingArchetype::Flowing => "Flowing",
                crate::history::naming::styles::NamingArchetype::Compound => "Compound",
                crate::history::naming::styles::NamingArchetype::Guttural => "Guttural",
                crate::history::naming::styles::NamingArchetype::Mystical => "Mystical",
                crate::history::naming::styles::NamingArchetype::Sibilant => "Sibilant",
                crate::history::naming::styles::NamingArchetype::Ancient => "Ancient",
            }).unwrap_or("Compound")
        });

    // Build NamingStyle from game data template if available, else fall back
    if let Some(template) = game_data.naming_style(archetype_name) {
        NamingStyle {
            id: NamingStyleId(0),
            onset_consonants: template.onset_consonants.clone(),
            coda_consonants: template.coda_consonants.clone(),
            vowels: template.vowels.clone(),
            syllable_range: (template.syllable_range[0], template.syllable_range[1]),
            uses_apostrophes: template.uses_apostrophes,
            uses_hyphens: template.uses_hyphens,
            place_prefixes: template.place_prefixes.clone(),
            place_suffixes: template.place_suffixes.clone(),
            epithet_patterns: template.epithet_patterns.clone(),
        }
    } else {
        let archetype = race.map(|r| r.base_type.default_naming_archetype())
            .unwrap_or(crate::history::naming::styles::NamingArchetype::Compound);
        NamingStyle::from_archetype(NamingStyleId(0), archetype)
    }
}

/// Generate a varied heresy name from the parent religion name.
fn generate_heresy_name(parent_name: &str, rng: &mut impl Rng) -> String {
    // Strip leading "The " from parent name to avoid "The Reformed The ..."
    let base = parent_name.strip_prefix("The ").unwrap_or(parent_name);
    let adjectives = [
        "Reformed", "True", "Purified", "Orthodox", "Awakened",
        "Reborn", "New", "Hidden", "Radical", "Illuminated",
        "Ascendant", "Exalted",
    ];
    // Also strip any existing heresy adjective to avoid "True True X"
    let mut base = base;
    for adj in &adjectives {
        if let Some(rest) = base.strip_prefix(adj) {
            base = rest.trim_start();
            break;
        }
    }
    let adj = adjectives[rng.gen_range(0..adjectives.len())];
    format!("The {} {}", adj, base)
}

/// Generate a unique legendary artifact name combining a proper name, material/type hint,
/// and optional epithet. Produces names like "Dawnbreaker", "The Scepter of Azureth",
/// "Frostbane, the Glacier's Wrath", etc.
fn generate_artifact_name(art_type: ArtifactType, quality: ArtifactQuality, rng: &mut impl Rng) -> String {
    // One-word legendary names
    let legendary_names = [
        "Dawnbreaker", "Nightfall", "Stormcaller", "Frostbane", "Soulreaver",
        "Sunforge", "Moonblade", "Starweave", "Flameheart", "Ironwill",
        "Thunderclap", "Shadowmend", "Voidrender", "Lightkeeper", "Ashborne",
        "Grimshard", "Evergleam", "Deathwhisper", "Lifebloom", "Windshear",
        "Bloodthorn", "Silentedge", "Crystalsong", "Emberveil", "Duskmantle",
        "Oathbinder", "Runesplitter", "Wargrowl", "Peacebringer", "Doomhammer",
        "Wraithclaw", "Hopespark", "Dreamsunder", "Gloryhilt", "Abyssgaze",
        "Bonechill", "Spiritforge", "Wyrmtooth", "Tidecaller", "Earthshaker",
    ];

    // Two-part "The X of Y" names
    let prefixes = match art_type {
        ArtifactType::Weapon => &["Blade", "Sword", "Axe", "Spear", "Mace", "Hammer", "Glaive", "Scythe"][..],
        ArtifactType::Armor => &["Shield", "Aegis", "Bulwark", "Cuirass", "Mantle", "Ward"][..],
        ArtifactType::Crown => &["Crown", "Diadem", "Circlet", "Tiara", "Coronet"][..],
        ArtifactType::Ring => &["Ring", "Band", "Signet", "Loop", "Circle"][..],
        ArtifactType::Amulet => &["Amulet", "Talisman", "Pendant", "Charm", "Necklace"][..],
        ArtifactType::Staff => &["Staff", "Rod", "Scepter", "Wand", "Crozier"][..],
        ArtifactType::Book => &["Tome", "Codex", "Grimoire", "Chronicle", "Scroll"][..],
        ArtifactType::Goblet => &["Goblet", "Chalice", "Grail", "Cup", "Vessel"][..],
        ArtifactType::Instrument => &["Harp", "Horn", "Lute", "Drum", "Bell"][..],
        ArtifactType::Relic => &["Orb", "Eye", "Heart", "Fang", "Skull", "Shard"][..],
    };

    let name_roots = [
        "Azureth", "Kalindra", "Morghul", "Thandris", "Veloran",
        "Xareth", "Ildris", "Norath", "Sylvain", "Darkoth",
        "Valoris", "Pyranthos", "Cerulean", "Obsidian", "Adamant",
        "Mithral", "Eclipse", "Zenith", "Nadir", "Solstice",
        "Equinox", "Tempest", "Eternity", "Entropy", "Genesis",
        "Ruin", "Glory", "Sorrow", "Fury", "Silence",
    ];

    let epithets = [
        "the Undying", "the Cursed", "the Blessed", "the Forgotten",
        "the Eternal", "the Burning", "the Frozen", "the Shattered",
        "the Ancient", "the Radiant", "the Corrupted", "the Hallowed",
        "the Boundless", "the Forsaken", "the Awakened", "the Dreaming",
    ];

    match quality {
        ArtifactQuality::Divine | ArtifactQuality::Legendary => {
            // Top-tier: single legendary name + optional epithet
            let name = legendary_names[rng.gen_range(0..legendary_names.len())];
            if rng.gen_bool(0.5) {
                let epithet = epithets[rng.gen_range(0..epithets.len())];
                format!("{}, {}", name, epithet)
            } else {
                name.to_string()
            }
        }
        ArtifactQuality::Masterwork => {
            // "The X of Y" or single name
            if rng.gen_bool(0.5) {
                let prefix = prefixes[rng.gen_range(0..prefixes.len())];
                let root = name_roots[rng.gen_range(0..name_roots.len())];
                format!("The {} of {}", prefix, root)
            } else {
                legendary_names[rng.gen_range(0..legendary_names.len())].to_string()
            }
        }
        ArtifactQuality::Superior => {
            // "The X of Y"
            let prefix = prefixes[rng.gen_range(0..prefixes.len())];
            let root = name_roots[rng.gen_range(0..name_roots.len())];
            format!("The {} of {}", prefix, root)
        }
        ArtifactQuality::Fine => {
            // Simpler: "Type-Root" or "The Root Type"
            let prefix = prefixes[rng.gen_range(0..prefixes.len())];
            let root = name_roots[rng.gen_range(0..name_roots.len())];
            if rng.gen_bool(0.5) {
                format!("{} of {}", prefix, root)
            } else {
                format!("The {} {}", root, prefix)
            }
        }
    }
}

/// Pick a war cause biased by the aggressor leader's personality.
/// Ambitious → Conquest, Greedy → Resource, Pious → Religious, Paranoid → Territorial.
fn pick_war_cause(personality: Option<&Personality>, rng: &mut impl Rng) -> WarCause {
    if let Some(p) = personality {
        // Build weighted distribution from personality
        let weights = [
            (WarCause::Territorial, 1.0 + p.paranoia * 2.0),
            (WarCause::Resource, 1.0 + p.greed * 2.0),
            (WarCause::Conquest, 1.0 + p.ambition * 2.0),
            (WarCause::Religious, 1.0 + p.piety * 2.0),
            (WarCause::Revenge, 1.0 + p.cruelty * 1.5),
        ];
        let total: f32 = weights.iter().map(|(_, w)| w).sum();
        let mut roll = rng.gen::<f32>() * total;
        for (cause, w) in &weights {
            roll -= w;
            if roll <= 0.0 {
                return *cause;
            }
        }
        WarCause::Conquest // fallback
    } else {
        match rng.gen_range(0..5) {
            0 => WarCause::Territorial,
            1 => WarCause::Resource,
            2 => WarCause::Conquest,
            3 => WarCause::Religious,
            _ => WarCause::Revenge,
        }
    }
}

/// Pick a monument type biased by leader personality.
/// Pious leaders build temples; ambitious ones build towers and statues.
fn pick_monument_type(personality: Option<&Personality>, rng: &mut impl Rng) -> MonumentType {
    if let Some(p) = personality {
        let weights = [
            (MonumentType::Statue, 1.0 + p.ambition * 1.5),
            (MonumentType::Obelisk, 1.0 + p.paranoia),
            (MonumentType::Temple, 1.0 + p.piety * 2.5),
            (MonumentType::Tower, 1.0 + p.ambition * 2.0),
            (MonumentType::Memorial, 1.0 + p.honor * 1.5),
            (MonumentType::Fountain, 1.0 + p.charisma),
        ];
        let total: f32 = weights.iter().map(|(_, w)| w).sum();
        let mut roll = rng.gen::<f32>() * total;
        for (mt, w) in &weights {
            roll -= w;
            if roll <= 0.0 {
                return *mt;
            }
        }
        MonumentType::Statue
    } else {
        match rng.gen_range(0..6) {
            0 => MonumentType::Statue,
            1 => MonumentType::Obelisk,
            2 => MonumentType::Temple,
            3 => MonumentType::Tower,
            4 => MonumentType::Memorial,
            _ => MonumentType::Fountain,
        }
    }
}

/// Pick a monument purpose biased by leader personality.
fn pick_monument_purpose(personality: Option<&Personality>, rng: &mut impl Rng) -> MonumentPurpose {
    if let Some(p) = personality {
        let weights = [
            (MonumentPurpose::CommemorateVictory, 1.0 + p.bravery * 1.5),
            (MonumentPurpose::ReligiousWorship, 1.0 + p.piety * 2.5),
            (MonumentPurpose::ArtisticExpression, 1.0 + p.charisma * 1.5),
            (MonumentPurpose::MarkTerritory, 1.0 + p.paranoia * 1.5),
        ];
        let total: f32 = weights.iter().map(|(_, w)| w).sum();
        let mut roll = rng.gen::<f32>() * total;
        for (mp, w) in &weights {
            roll -= w;
            if roll <= 0.0 {
                return *mp;
            }
        }
        MonumentPurpose::CommemorateVictory
    } else {
        match rng.gen_range(0..4) {
            0 => MonumentPurpose::CommemorateVictory,
            1 => MonumentPurpose::ReligiousWorship,
            2 => MonumentPurpose::ArtisticExpression,
            _ => MonumentPurpose::MarkTerritory,
        }
    }
}

/// Get the leader personality of a faction (returns None if no leader or figure not found).
fn leader_personality<'a>(history: &'a WorldHistory, faction_id: FactionId) -> Option<&'a Personality> {
    history.factions.get(&faction_id)
        .and_then(|f| f.current_leader)
        .and_then(|lid| history.figures.get(&lid))
        .map(|fig| &fig.personality)
}

/// Get the war modifier from a faction's state religion.
fn faction_religion_war_modifier(history: &WorldHistory, faction_id: FactionId) -> f32 {
    history.factions.get(&faction_id)
        .and_then(|f| f.state_religion)
        .and_then(|rid| history.religions.get(&rid))
        .map(|r| r.war_modifier())
        .unwrap_or(1.0)
}

/// Get the diplomacy modifier from a faction's state religion.
fn faction_religion_diplomacy_modifier(history: &WorldHistory, faction_id: FactionId) -> f32 {
    history.factions.get(&faction_id)
        .and_then(|f| f.state_religion)
        .and_then(|rid| history.religions.get(&rid))
        .map(|r| r.diplomacy_modifier())
        .unwrap_or(1.0)
}

/// Get the monument modifier from a faction's state religion.
fn faction_religion_monument_modifier(history: &WorldHistory, faction_id: FactionId) -> f32 {
    history.factions.get(&faction_id)
        .and_then(|f| f.state_religion)
        .and_then(|rid| history.religions.get(&rid))
        .map(|r| r.monument_modifier())
        .unwrap_or(1.0)
}

/// Strip leading "The " from a name to avoid doubling ("The The X", "by the The X").
/// Faction names always start with "The " so use this when embedding them
/// after an article already present in the sentence.
fn strip_the(name: &str) -> &str {
    name.strip_prefix("The ").unwrap_or(name)
}

/// Check if two factions are geographic neighbors (any settlement within distance tiles).
fn factions_are_neighbors(history: &WorldHistory, a: FactionId, b: FactionId, max_dist: usize) -> bool {
    let settlements_a: Vec<(usize, usize)> = history.factions.get(&a)
        .map(|f| f.settlements.iter()
            .filter_map(|sid| history.settlements.get(sid).map(|s| s.location))
            .collect())
        .unwrap_or_default();
    let settlements_b: Vec<(usize, usize)> = history.factions.get(&b)
        .map(|f| f.settlements.iter()
            .filter_map(|sid| history.settlements.get(sid).map(|s| s.location))
            .collect())
        .unwrap_or_default();

    let max_dist_sq = max_dist * max_dist;
    for &(ax, ay) in &settlements_a {
        for &(bx, by) in &settlements_b {
            let dx = ax.abs_diff(bx);
            let dy = ay.abs_diff(by);
            if dx * dx + dy * dy <= max_dist_sq {
                return true;
            }
        }
    }
    false
}

/// Get cultural similarity between two factions.
fn get_cultural_similarity(history: &WorldHistory, a: FactionId, b: FactionId) -> f32 {
    let values_a = history.factions.get(&a)
        .and_then(|f| history.races.get(&f.race_id))
        .and_then(|r| history.cultures.get(&r.culture_id))
        .map(|c| &c.values);
    let values_b = history.factions.get(&b)
        .and_then(|f| history.races.get(&f.race_id))
        .and_then(|r| history.cultures.get(&r.culture_id))
        .map(|c| &c.values);
    match (values_a, values_b) {
        (Some(a), Some(b)) => a.similarity(b),
        _ => 0.5,
    }
}

/// Get a faction's xenophobia value.
fn get_faction_xenophobia(history: &WorldHistory, faction_id: FactionId) -> f32 {
    history.factions.get(&faction_id)
        .and_then(|f| history.races.get(&f.race_id))
        .and_then(|r| history.cultures.get(&r.culture_id))
        .map(|c| c.values.xenophobia)
        .unwrap_or(0.5)
}

/// Check if two factions share the same state religion.
fn faction_has_holy_war_doctrine(history: &WorldHistory, faction_id: FactionId) -> bool {
    history.factions.get(&faction_id)
        .and_then(|f| f.state_religion)
        .and_then(|rid| history.religions.get(&rid))
        .map_or(false, |r| r.has_doctrine(crate::history::religion::worship::Doctrine::HolyWar))
}

fn factions_share_religion(history: &WorldHistory, a: FactionId, b: FactionId) -> bool {
    let rel_a = history.factions.get(&a).and_then(|f| f.state_religion);
    let rel_b = history.factions.get(&b).and_then(|f| f.state_religion);
    match (rel_a, rel_b) {
        (Some(ra), Some(rb)) => ra == rb,
        _ => false,
    }
}

/// Expand faction territory based on settlement influence
fn step_territory_expansion(
    history: &mut WorldHistory,
    world: &WorldData,
    rng: &mut impl Rng,
) {
    let mut claims = Vec::new();
    let width = history.tile_history.width;
    let height = history.tile_history.height;
    
    // Check expansions for each settlement
    for settlement in history.settlements.values() {
        if settlement.is_destroyed() { continue; }
        
        // Influence radius grows with population
        // Pop 500 => ~5 tiles. Pop 5000 => ~17 tiles.
        // Cap max radius to avoid map domination
        let radius = ((settlement.population as f32).sqrt() * 0.25).clamp(2.0, 15.0) as i32;
        let (sx, sy) = settlement.location;
        let faction_id = settlement.faction;
        
        // Try to claim N tiles per turn where N is related to population
        let expansion_attempts = (radius as usize / 2).max(1);
        
        for _ in 0..expansion_attempts {
            // Pick a random tile in influence radius
            let dx = rng.gen_range(-radius..=radius);
            let dy = rng.gen_range(-radius..=radius);
            
            if dx*dx + dy*dy > radius*radius { continue; }
            
            let tx = sx as i32 + dx;
            let ty = sy as i32 + dy;
            
            if tx >= 0 && tx < width as i32 && ty >= 0 && ty < height as i32 {
                let x = tx as usize;
                let y = ty as usize;
                
                // Only claim if unowned
                if history.tile_history.get(x, y).current_owner.is_none() {
                     let h = *world.heightmap.get(x, y);
                     let is_water = *world.water_depth.get(x, y) > 0.0 || h < 0.0;
                     
                     // Claim land tiles (including rivers, but not oceans/lakes if significant)
                     if !is_water && h < 3000.0 {
                         claims.push((x, y, faction_id));
                     }
                }
            }
        }
    }
    
    // Apply claims
    let date = history.current_date;
    for (x, y, faction_id) in claims {
         history.tile_history.set_owner(x, y, faction_id, date);
    }
}

/// A disaster takes at most 10-50% of a settlement, so hamlets survive what would wipe out
/// nothing in a city.
fn scaled_loss(history: &WorldHistory, sid: SettlementId, base: u32, rng: &mut impl Rng) -> u32 {
    let pop = history.settlements.get(&sid).map(|s| s.population).unwrap_or(0);
    base.min((pop as f32 * rng.gen_range(0.1..0.5)) as u32)
}

// =========================================================================
// Resources in diplomacy and trade
// =========================================================================

/// Resources worth fighting or trading over (basics everyone has don't count).
fn is_strategic(r: ResourceType) -> bool {
    matches!(r,
        ResourceType::Iron | ResourceType::Copper | ResourceType::Tin | ResourceType::Gold |
        ResourceType::Silver | ResourceType::Gems | ResourceType::Coal | ResourceType::Salt |
        ResourceType::Mithril | ResourceType::Adamantine)
}

/// What `a` could gain by trading with `b`: the value of strategic and staple goods each side
/// has that the other lacks, and the goods that would move.
fn trade_gain(history: &WorldHistory, a: SettlementId, b: SettlementId) -> (f32, Vec<ResourceType>) {
    let (Some(sa), Some(sb)) = (history.settlements.get(&a), history.settlements.get(&b)) else { return (0.0, Vec::new()) };
    let surplus = |from: &Settlement, to: &Settlement| -> Vec<ResourceType> {
        from.production.iter()
            .filter(|(k, q)| **q >= 0.5 && !to.local_resources.contains(k) && **k != ResourceType::Food)
            .map(|(k, _)| *k)
            .collect()
    };
    let mut goods = surplus(sa, sb);
    for g in surplus(sb, sa) { if !goods.contains(&g) { goods.push(g); } }
    goods.sort_by_key(|g| std::cmp::Reverse(g.base_value()));
    let gain = goods.iter().map(|g| g.base_value() as f32).sum::<f32>();
    (gain, goods)
}

/// How much `a` covets what `b` holds: value of strategic resources in `b`'s settlements that
/// none of `a`'s have (0 if `a` is not short of them), and the most valuable such resource.
fn resource_envy(history: &WorldHistory, a: FactionId, b: FactionId) -> (f32, Option<ResourceType>) {
    let held = |f: FactionId| -> Vec<ResourceType> {
        let mut v: Vec<ResourceType> = Vec::new();
        if let Some(fa) = history.factions.get(&f) {
            for sid in &fa.settlements {
                if let Some(s) = history.settlements.get(sid) {
                    if s.is_destroyed() { continue; }
                    for r in &s.local_resources { if is_strategic(*r) && !v.contains(r) { v.push(*r); } }
                }
            }
        }
        v
    };
    let (mine, theirs) = (held(a), held(b));
    let mut best: Option<ResourceType> = None;
    let mut total = 0.0;
    for r in theirs.iter().filter(|r| !mine.contains(r)) {
        total += r.base_value() as f32;
        if best.map(|x| r.base_value() > x.base_value()).unwrap_or(true) { best = Some(*r); }
    }
    (total, best)
}

// =========================================================================
// Settlement lifecycle: colonization, abandonment, razing
// =========================================================================

/// Chance that a successfully besieged (non-capital) settlement is razed rather than taken.
const RAZE_CHANCE: f32 = 0.3;
/// Per-season chance that a faction with a crowded settlement sends out colonists.
const COLONIZE_CHANCE: f32 = 0.06;
/// People who leave to found a new village.
const COLONIST_POPULATION: u32 = 90;
/// A settlement this small for decades may be abandoned (per-season chance).
const ABANDON_BELOW: u32 = 12;
const ABANDON_CHANCE: f32 = 0.05;
/// Minimum distance (tiles) between living settlements.
const SETTLEMENT_SPACING: i64 = 4;

/// A disaster's name for titles ("Earthquake strikes Bonegore").
fn disaster_noun(t: &EventType) -> &'static str {
    match t {
        EventType::VolcanoErupted => "Eruption",
        EventType::Earthquake => "Earthquake",
        EventType::Flood => "Flood",
        EventType::Drought => "Drought",
        EventType::Plague => "Plague",
        EventType::MagicalCatastrophe => "Catastrophe",
        _ => "Disaster",
    }
}

/// A disaster as the subject of a sentence ("an earthquake devastated Bonegore").
fn disaster_phrase(t: &EventType) -> String {
    match t {
        EventType::VolcanoErupted => "the eruption of a volcano".into(),
        EventType::MagicalCatastrophe => "a magical catastrophe".into(),
        EventType::Earthquake => "an earthquake".into(),
        other => format!("a {}", disaster_noun(other).to_lowercase()),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// "1 season", "4 seasons"; a siege that fell at once took "less than a season".
fn seasons_text(n: u32) -> String {
    match n {
        0 => "less than a season".to_string(),
        1 => "1 season".to_string(),
        n => format!("{} seasons", n),
    }
}

/// Mark a settlement destroyed and clear it from the map. Returns its location.
pub(crate) fn destroy_settlement(history: &mut WorldHistory, id: SettlementId, date: Date) -> Option<(usize, usize)> {
    let loc = {
        let s = history.settlements.get_mut(&id)?;
        if s.destroyed.is_some() { return None; }
        s.destroyed = Some(date);
        s.population = 0;
        s.location
    };
    let t = history.tile_history.get_mut(loc.0, loc.1);
    if t.settlement == Some(id) { t.settlement = None; }
    if !t.former_settlements.contains(&id) { t.former_settlements.push(id); }
    Some(loc)
}

/// Dissolve a faction that has lost its last settlement.
pub(crate) fn dissolve_if_landless(history: &mut WorldHistory, faction: FactionId, name: &str, date: Date) {
    let left = history.factions.get(&faction).map(|f| f.settlements.len()).unwrap_or(1);
    let active = history.factions.get(&faction).map_or(false, |f| f.is_active());
    if left > 0 || !active { return; }
    if let Some(f) = history.factions.get_mut(&faction) { f.dissolve(date); }
    let event_id = history.id_generators.next_event();
    let event = Event::new(
        event_id,
        EventType::FactionDestroyed,
        date,
        format!("{} destroyed", name),
        format!("{} has been destroyed after losing their last settlement.", name),
    )
    .with_faction(faction);
    // Whatever last befell the people (their last town's fall, as a rule).
    let event = match history.chronicle.last_of(EntityId::Faction(faction)) { Some(c) => event.caused_by(c), None => event };
    history.chronicle.record(event);
}

/// Crowded settlements send colonists to found villages on good nearby land (rivers, coasts,
/// flat ground); tiny outlying settlements that never took hold are abandoned.
fn step_colonization(history: &mut WorldHistory, world: &WorldData, game_data: &GameData, rng: &mut impl Rng) {
    let date = history.current_date;
    let (w, h) = (world.width, world.height);
    let living: Vec<(SettlementId, (usize, usize))> = history.settlements.values()
        .filter(|s| !s.is_destroyed())
        .map(|s| (s.id, s.location))
        .collect();
    let too_close = |x: usize, y: usize, extra: &[(usize, usize)]| {
        living.iter().map(|(_, l)| *l).chain(extra.iter().copied()).any(|(sx, sy)| {
            let dx = (x as i64 - sx as i64).abs();
            let dx = dx.min(w as i64 - dx);
            dx.max((y as i64 - sy as i64).abs()) < SETTLEMENT_SPACING
        })
    };
    let site_score = |x: usize, y: usize| -> f32 {
        let e = *world.heightmap.get(x, y);
        if e <= 0.0 || e > 2500.0 || world.water_body_map.get(x, y).is_lake() { return f32::MIN; }
        let biome = *world.biomes.get(x, y);
        if matches!(biome, crate::biomes::ExtendedBiome::Ice | crate::biomes::ExtendedBiome::SnowyPeaks) { return f32::MIN; }
        let mut score = 1.0 - e / 2500.0;
        let res = world.resources();
        score += res.fertility_near(x, y, 3, w) * 2.0;
        score += (res.wealth_near(x, y, 4, w) / 25.0).min(1.5);
        let flow = world.flow_accumulation.as_ref().map(|f| *f.get(x, y)).unwrap_or(0.0);
        if flow > 50.0 { score += 1.5; } else if flow > 15.0 { score += 0.6; }
        let coast = (-1i64..=1).any(|dy| (-1i64..=1).any(|dx| {
            let ny = y as i64 + dy;
            ny >= 0 && ny < h as i64 && *world.heightmap.get((x as i64 + dx).rem_euclid(w as i64) as usize, ny as usize) <= 0.0
        }));
        if coast { score += 0.8; }
        let t = *world.temperature.get(x, y);
        if t < -5.0 { score -= 1.0; }
        score
    };

    let faction_ids: Vec<FactionId> = history.factions.values().filter(|f| f.is_active()).map(|f| f.id).collect();
    let mut founded: Vec<(usize, usize)> = Vec::new();
    for fid in faction_ids {
        if rng.gen::<f32>() >= COLONIZE_CHANCE { continue; }
        let (race_id, faction_name, parents) = {
            let f = &history.factions[&fid];
            (f.race_id, f.name.clone(), f.settlements.clone())
        };
        // The most crowded settlement sends the colonists.
        let parent = parents.iter()
            .filter_map(|id| history.settlements.get(id))
            .filter(|s| !s.is_destroyed() && s.population >= 400)
            .max_by_key(|s| s.population)
            .map(|s| (s.id, s.location, s.name.clone()));
        let Some((parent_id, (px, py), parent_name)) = parent else { continue };
        if parents.len() >= 12 { continue; }

        // Best of a handful of candidate sites 4-10 tiles away, on own or unclaimed land.
        let mut best: Option<(usize, usize, f32)> = None;
        for _ in 0..24 {
            let r = rng.gen_range(SETTLEMENT_SPACING as f32..10.0);
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            let ny = py as i64 + (a.sin() * r).round() as i64;
            if ny < 0 || ny >= h as i64 { continue; }
            let (nx, ny) = ((px as i64 + (a.cos() * r).round() as i64).rem_euclid(w as i64) as usize, ny as usize);
            let owner = history.tile_history.get(nx, ny).current_owner;
            if owner.is_some() && owner != Some(fid) { continue; }
            if too_close(nx, ny, &founded) { continue; }
            let sc = site_score(nx, ny) + rng.gen_range(0.0..0.3);
            if sc > best.map(|b| b.2).unwrap_or(0.2) { best = Some((nx, ny, sc)); }
        }
        let Some((sx, sy, _)) = best else { continue };

        let style = naming_style_for_race(history, race_id, game_data);
        let name = NameGenerator::place_name(&style, rng);
        let sid = history.id_generators.next_settlement();
        let biome = *world.biomes.get(sx, sy);
        let mut settlement = Settlement::new(sid, name.clone(), SettlementType::Village, (sx, sy), fid, date, ResourceType::from_biome(biome));
        settlement.population = COLONIST_POPULATION;
        super::setup::apply_local_economy(&mut settlement, world);
        history.settlements.insert(sid, settlement);
        if let Some(p) = history.settlements.get_mut(&parent_id) { p.population = p.population.saturating_sub(COLONIST_POPULATION); }
        if let Some(f) = history.factions.get_mut(&fid) { f.add_settlement(sid); }
        history.tile_history.set_owner(sx, sy, fid, date);
        history.tile_history.get_mut(sx, sy).settlement = Some(sid);
        founded.push((sx, sy));

        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::SettlementFounded,
            date,
            format!("{} founded", name),
            format!("Settlers from {} of {} founded the village of {}.", parent_name, faction_name, name),
        )
        .at_location(sx, sy)
        .with_faction(fid)
        .with_participant(EntityId::Settlement(sid))
        .with_participant(EntityId::Settlement(parent_id));
        history.tile_history.record_event(sx, sy, event_id);
        history.chronicle.record(event);
    }

    // Abandonment: villages and outposts that dwindled away.
    let dying: Vec<(SettlementId, FactionId, String)> = history.settlements.values()
        .filter(|s| !s.is_destroyed()
            && s.population < ABANDON_BELOW
            && !matches!(s.settlement_type, SettlementType::Capital | SettlementType::City)
            && date.year.saturating_sub(s.founded.year) > 30)
        .map(|s| (s.id, s.faction, s.name.clone()))
        .collect();
    for (sid, fid, name) in dying {
        if rng.gen::<f32>() > ABANDON_CHANCE { continue; }
        if let Some(f) = history.factions.get_mut(&fid) { f.remove_settlement(sid); }
        let loc = destroy_settlement(history, sid, date);
        let event_id = history.id_generators.next_event();
        let mut event = Event::new(
            event_id,
            EventType::SettlementDestroyed,
            date,
            format!("{} abandoned", name),
            format!("The last families left {}, and it fell to ruin.", name),
        )
        .with_faction(fid)
        .with_participant(EntityId::Settlement(sid));
        if let Some((x, y)) = loc {
            event = event.at_location(x, y);
            history.tile_history.record_event(x, y, event_id);
        }
        history.chronicle.record(event);
        let fname = history.factions.get(&fid).map(|f| f.name.clone()).unwrap_or_default();
        dissolve_if_landless(history, fid, &fname, date);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::simulation::setup::initialize_world;
    use crate::history::config::HistoryConfig;
    use crate::biomes::ExtendedBiome;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use crate::tilemap::Tilemap;
    use crate::seeds::WorldSeeds;
    use crate::scale::MapScale;
    use crate::plates::PlateId;
    use crate::water_bodies::WaterBodyId;
    use crate::seasons::Season;

    fn make_test_world() -> WorldData {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.3);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);

        for x in 0..width {
            *biomes.get_mut(x, 0) = ExtendedBiome::Ocean;
            *heightmap.get_mut(x, 0) = -0.1;
        }

        let seeds = WorldSeeds::from_master(42);
        let scale = MapScale::new(1.0);
        let temperature = Tilemap::new_with(width, height, 15.0);
        let moisture = Tilemap::new_with(width, height, 0.5);
        let stress_map = Tilemap::new_with(width, height, 0.0);
        let plate_map = Tilemap::new_with(width, height, PlateId(0));
        let water_body_map = Tilemap::new_with(width, height, WaterBodyId::NONE);
        let water_depth = Tilemap::new_with(width, height, 0.0);

        WorldData::new(
            seeds, scale, heightmap, temperature, moisture,
            biomes, stress_map, plate_map, Vec::new(),
            None, water_body_map, Vec::new(), water_depth,
            None, None,
        )
    }

    #[test]
    fn test_simulate_one_step() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            initial_civilizations: 3,
            initial_legendary_creatures: 3,
            simulation_years: 10,
            prehistory_depth: 0,
            ..HistoryConfig::default()
        };
        let mut history = initialize_world(&world, config, &game_data, &mut rng);

        let initial_events = history.chronicle.len();
        simulate_step(&mut history, &world, &game_data, &mut rng);

        assert!(history.chronicle.len() >= initial_events);
        assert_eq!(history.current_date, Date::new(1, Season::Summer));
    }

    #[test]
    fn test_simulate_multiple_steps() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            initial_civilizations: 4,
            initial_legendary_creatures: 5,
            simulation_years: 10,
            prehistory_depth: 0,
            ..HistoryConfig::default()
        };
        let mut history = initialize_world(&world, config, &game_data, &mut rng);

        // Simulate 40 seasons (10 years)
        for _ in 0..40 {
            simulate_step(&mut history, &world, &game_data, &mut rng);
        }

        let summary = history.summary();
        eprintln!("{}", summary);

        assert!(summary.total_events > 0);
        assert!(summary.years_simulated >= 10);
        assert!(summary.total_population > 0);
    }

    #[test]
    fn dump_history_names() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            initial_civilizations: 20,
            initial_legendary_creatures: 10,
            simulation_years: 100,
            prehistory_depth: 0,
            ..HistoryConfig::default()
        };
        let mut history = initialize_world(&world, config, &game_data, &mut rng);
        for _ in 0..400 {
            simulate_step(&mut history, &world, &game_data, &mut rng);
        }
        let mut out = String::new();
        out.push_str("=== ARTIFACTS ===\n");
        for a in history.artifacts.values() {
            out.push_str(&format!("  {} ({:?}, {:?}) destroyed={} lost={}\n",
                a.name, a.item_type, a.quality, a.destroyed, a.lost));
        }
        out.push_str("\n=== RELIGIONS ===\n");
        for r in history.religions.values() {
            out.push_str(&format!("  {} (followers: {})\n", r.name, r.follower_count));
        }
        out.push_str("\n=== RECENT EVENTS (last 100) ===\n");
        let events = &history.chronicle.events;
        let start = events.len().saturating_sub(100);
        for e in &events[start..] {
            out.push_str(&format!("  [{:?}] {}: {}\n", e.event_type, e.title, e.description));
        }
        out.push_str(&format!("\n=== SUMMARY ===\n{}\n", history.summary()));
        std::fs::write("/tmp/history_dump.txt", &out).unwrap();
        eprintln!("{}", out);
    }
}
