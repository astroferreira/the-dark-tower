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
use crate::history::naming::styles::NamingStyle;
use crate::history::naming::generator::NameGenerator;
use crate::history::religion::worship::{Religion, Doctrine};

/// Run one season of simulation.
pub fn simulate_step(
    history: &mut WorldHistory,
    world: &WorldData,
    game_data: &GameData,
    rng: &mut impl Rng,
) {
    let date = history.current_date;

    // 1. Population growth
    step_population_growth(history);

    // 2. Settlement upgrades
    step_settlement_upgrades(history);

    // 2.5 Territory expansion
    step_territory_expansion(history, world, rng);


    // 3. Opinion friction (border disputes, rivalries)
    step_opinion_friction(history, rng);

    // 4. Peaceful diplomacy (treaties, alliances)
    step_diplomacy_peaceful(history, rng);

    // 5. War declarations
    step_diplomacy(history, rng);

    // 5. Alliance obligations and treaty enforcement
    step_alliance_obligations(history, rng);

    // 5a. Active wars: battles
    step_wars(history, rng);

    // 5.5. Active sieges: attrition, resolution
    step_sieges(history, rng);

    // 6. Creature activity
    step_creatures(history, rng);

    // 7. Figure lifecycle (births, deaths, succession)
    step_figures(history, game_data, rng);

    // 8. Artifact and monument creation
    step_artifacts(history, rng);

    // 9. Trade route establishment
    step_trade(history, world, rng);

    // 10. Religious events (conversion, schisms, sacrifice)
    step_religion(history, rng);

    // 11. Natural events
    step_natural_events(history, rng);

    // 12. Hero quests
    step_quests(history, rng);

    // 13. Assassination & Intrigue
    step_assassination(history, game_data, rng);

    // 13. Artifact lifecycle (inheritance, loss, hoarding, destruction)
    step_artifact_lifecycle(history, rng);

    // 13. Wealth tick (income, trade revenue, war costs)
    step_wealth_tick(history, rng);

    // 13. Advance date
    history.current_date = date.next();
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
                    format!("{} grows to a {:?}", settlement.name, settlement.settlement_type),
                    format!("{} has grown from a {:?} to a {:?}.",
                        settlement.name, old_type, settlement.settlement_type),
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

        // Total opinion delta: negative (friction) or slightly positive (cultural affinity)
        let total_friction = friction + religion_friction;

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
            let incident_type = match rng.gen_range(0..4) {
                0 => "border clash",
                1 => "diplomatic insult",
                2 => "trade dispute",
                _ => "territorial encroachment",
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

fn step_diplomacy(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
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
            * same_religion_mult * opinion_mult;

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
        let cause = pick_war_cause(leader_p, rng);

        let event_id = history.id_generators.next_event();
        let war_name = format!("{:?} War of {} and {}", cause, name_a, name_b);
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
            let effective = hostile_war_chance * personality_mult * religion_war_mult * opinion_mult;
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
            let war_name = format!("{:?} War of {} and {}", cause, name_a, name_b);
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
        if rng.gen::<f32>() >= warmonger_chance * rel_mult { continue; }

        // Pick a random neighbor to attack (even without deep hatred)
        let neighbor_idx = rng.gen_range(0..faction_ids.len());
        let fid_b = faction_ids[neighbor_idx];
        if fid_a == fid_b { continue; }
        if !factions_are_neighbors(history, fid_a, fid_b, 45) { continue; }
        if history.factions.get(&fid_a).map_or(false, |f| f.is_at_war_with(fid_b)) { continue; }
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

        let war_name = format!("{:?} War of {} and {}", cause, name_a, name_b);
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
        if rng.gen::<f32>() >= crusade_chance { continue; }

        // Find a different-religion neighbor to crusade against
        let neighbor_idx = rng.gen_range(0..faction_ids.len());
        let fid_b = faction_ids[neighbor_idx];
        if fid_a == fid_b { continue; }
        if factions_share_religion(history, fid_a, fid_b) { continue; }
        if !factions_are_neighbors(history, fid_a, fid_b, 50) { continue; }
        if history.factions.get(&fid_a).map_or(false, |f| f.is_at_war_with(fid_b)) { continue; }
        let b_active_wars = history.factions.get(&fid_b)
            .map_or(0, |f| f.active_war_count());
        if b_active_wars >= max_wars_per_faction { continue; }

        let event_id = history.id_generators.next_event();
        let war_id = history.id_generators.next_war();
        let name_a = history.factions.get(&fid_a).map(|f| f.name.clone()).unwrap_or_default();
        let name_b = history.factions.get(&fid_b).map(|f| f.name.clone()).unwrap_or_default();
        let leader_id_a = history.factions.get(&fid_a).and_then(|f| f.current_leader);

        let war_name = format!("Holy Crusade of {} against {}", name_a, name_b);
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

fn step_wars(history: &mut WorldHistory, rng: &mut impl Rng) {
    let date = history.current_date;
    let active_war_ids: Vec<WarId> = history.wars.keys()
        .copied()
        .filter(|id| history.wars.get(id).map_or(false, |w| w.is_active()))
        .collect();

    for war_id in active_war_ids {
        // Battle chance per season
        if rng.gen::<f32>() < 0.15 {
            let (agg, def) = {
                let war = match history.wars.get(&war_id) {
                    Some(w) => w,
                    None => continue,
                };
                let agg = *war.aggressors.first().unwrap_or(&FactionId(0));
                let def = *war.defenders.first().unwrap_or(&FactionId(0));
                (agg, def)
            };

            let agg_strength = history.factions.get(&agg).map(|f| f.military_strength).unwrap_or(100);
            let def_strength = history.factions.get(&def).map(|f| f.military_strength).unwrap_or(100);

            let agg_roll: f32 = rng.gen::<f32>() * agg_strength.max(1) as f32;
            let def_roll: f32 = rng.gen::<f32>() * def_strength.max(1) as f32;

            let agg_losses = rng.gen_range(10..100);
            let def_losses = rng.gen_range(10..100);

            // Update casualties
            if let Some(war) = history.wars.get_mut(&war_id) {
                war.casualties.aggressor_losses += agg_losses;
                war.casualties.defender_losses += def_losses;
            }

            // Update populations
            let agg_name = history.factions.get(&agg).map(|f| f.name.clone()).unwrap_or_default();
            let def_name = history.factions.get(&def).map(|f| f.name.clone()).unwrap_or_default();

            if let Some(faction) = history.factions.get_mut(&agg) {
                faction.total_population = faction.total_population.saturating_sub(agg_losses);
            }
            if let Some(faction) = history.factions.get_mut(&def) {
                faction.total_population = faction.total_population.saturating_sub(def_losses);
            }

            // Record battle event (caused by the war declaration)
            let event_id = history.id_generators.next_event();
            let outcome = if agg_roll > def_roll { "attacker victory" } else { "defender victory" };
            let declaration_evt = history.wars.get(&war_id)
                .and_then(|w| w.declaration_event);
            let mut event = Event::new(
                event_id,
                EventType::BattleFought,
                date,
                format!("Battle between {} and {}", agg_name, def_name),
                format!("Battle result: {}. Losses: {} ({}) vs {} ({}).",
                    outcome, agg_losses, agg_name, def_losses, def_name),
            )
            .with_faction(agg)
            .with_faction(def);
            if let Some(decl_id) = declaration_evt {
                event = event.caused_by(decl_id);
            }

            if let Some(war) = history.wars.get_mut(&war_id) {
                war.battles.push(event_id);
            }
            history.chronicle.record(event);
        }

        // Check war exhaustion / end condition
        let should_end = {
            let war = match history.wars.get(&war_id) {
                Some(w) => w,
                None => continue,
            };
            let duration = date.year.saturating_sub(war.started.year);
            duration > 3 && rng.gen::<f32>() < 0.1 * duration as f32
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
            let victor = if agg_losses <= def_losses { Some(agg) } else { Some(def) };
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

            // War conquest: victor initiates sieges instead of instant transfer
            // Settlement transfers now happen through the siege system
            if let Some(victor_id) = victor {
                let total_casualties = agg_losses + def_losses;
                let loser_casualty_ratio = if victor == Some(agg) {
                    def_losses as f32 / total_casualties.max(1) as f32
                } else {
                    agg_losses as f32 / total_casualties.max(1) as f32
                };
                let conquest_chance = 0.65 + loser_casualty_ratio * 0.3;
                let settlements_to_take = if loser_casualty_ratio > 0.6 { 2 } else { 1 };

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
            if loser_settlement_count == 0 {
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
                format!("End of the war between {} and {}", agg_name, def_name),
                format!("The war has ended. Victor: {}.", victor_name),
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
                let losses = rng.gen_range(10..200);
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

        // Generate new leader
        let naming_style = naming_style_for_race(history, race_id, game_data);
        let new_leader_id = history.id_generators.next_figure();
        let new_leader_name = NameGenerator::personal_name(&naming_style, rng);
        let personality = Personality::random(rng);
        let mut new_leader = Figure::new(
            new_leader_id, new_leader_name.clone(),
            race_id,
            Date::new(date.year.saturating_sub(rng.gen_range(20..50)), crate::seasons::Season::Spring),
            personality,
        );
        new_leader.faction = Some(faction_id);

        // Wire dynasty link based on succession law
        if succession_law.requires_dynasty() {
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
            history.chronicle.record(crisis_event);

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
            history.chronicle.record(event);
        }
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

            let creator = history.factions.get(&fid)
                .and_then(|f| f.notable_figures.last().copied());

            // Generate unique name (retry if duplicate)
            let existing_names: Vec<&str> = history.artifacts.values()
                .map(|a| a.name.as_str()).collect();
            let mut art_name = generate_artifact_name(art_type, quality, rng);
            let mut attempts = 0;
            while existing_names.contains(&art_name.as_str()) && attempts < 10 {
                art_name = generate_artifact_name(art_type, quality, rng);
                attempts += 1;
            }
            let mut artifact = Artifact::new(
                art_id, art_name.clone(), art_type, quality, date, creator,
            );

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
            let event = Event::new(
                event_id,
                EventType::ArtifactCreated,
                date,
                format!("Creation of {}", art_name),
                format!("{} was crafted by {}.", art_name, faction_name),
            )
            .with_faction(fid);
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

                let faction_name = history.factions.get(&fid).map(|f| f.name.clone()).unwrap_or_default();
                let type_str = format!("{:?}", mon_type);
                let place_word = strip_the(&faction_name).split_whitespace().next().unwrap_or("Grand");
                let mon_name = format!("The {} {}", place_word, type_str);
                let monument = Monument::new(
                    mon_id, mon_name.clone(), mon_type, (mx, my),
                    fid, date, purpose,
                );

                let article = match mon_type {
                    MonumentType::Obelisk => "an",
                    _ => "a",
                };
                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::MonumentBuilt,
                    date,
                    format!("Construction of {}", mon_name),
                    format!("{} built {} {} at their capital.", faction_name, article, type_str.to_lowercase()),
                )
                .at_location(mx, my)
                .with_faction(fid);
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
                    format!("{} adopted {}, converting from their old faith.", faction_name, religion_name),
                )
                .with_faction(fid);
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

        let losses = rng.gen_range(50..500);
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
            format!("{:?} strikes {}", event_type, settlement_name),
            format!("A {:?} devastated {}, killing {} people.",
                event_type, settlement_name, losses),
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
        
        // Pick a random candidate
        let (sid_b, loc_b, fid_b) = candidates[rng.gen_range(0..candidates.len())];
        
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
        
        // Determine traded goods based on biomes
        let biome_a = world.biomes.get(loc_a.0, loc_a.1);
        let biome_b = world.biomes.get(loc_b.0, loc_b.1);
        let goods_a = ResourceType::from_biome(*biome_a);
        let goods_b = ResourceType::from_biome(*biome_b);
        
        // Find complementary goods
        let mut traded: Vec<ResourceType> = Vec::new();
        for g in &goods_a {
            if !goods_b.contains(g) {
                traded.push(*g);
            }
        }
        for g in &goods_b {
            if !goods_a.contains(g) && !traded.contains(g) {
                traded.push(*g);
            }
        }
        
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
        let goods_str: Vec<String> = traded.iter().map(|g| format!("{:?}", g)).collect();
        
        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::TradeRouteEstablished,
            date,
            format!("Trade route: {} ↔ {}", name_a, name_b),
            format!("A trade route was established between {} and {} for {}.",
                name_a, name_b, goods_str.join(", ")),
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

/// Find a path between two locations using A* that prefers existing roads.
/// Existing roads have much lower traversal cost, causing routes to converge.
fn find_trade_path(
    world: &WorldData,
    tile_history: &crate::history::world_state::tile_history::TileHistoryMap,
    from: (usize, usize),
    to: (usize, usize),
) -> Vec<(usize, usize)> {
    use std::collections::{BinaryHeap, HashMap};
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

    // Terrain traversal cost - roads are MUCH cheaper
    // Costs reflect difficulty of building and traversing roads
    let terrain_cost = |x: usize, y: usize| -> u32 {
        use crate::biomes::ExtendedBiome;
        
        // Check if there's already a road - very cheap to use!
        if tile_history.has_road(x, y) {
            return 1; // Roads are super cheap - natural convergence
        }
        
        let h = *world.heightmap.get(x, y);
        let biome = *world.biomes.get(x, y);
        
        // Check for rivers - they add significant cost (bridges needed)
        let has_river = world.river_network.as_ref()
            .map_or(false, |rn| rn.has_significant_flow(x, y));
        let river_penalty = if has_river { 15 } else { 0 };

        // Impassable terrain
        if h > 0.85 { return 0; } // Very high mountains
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
            ExtendedBiome::Foothills => 4,
            
            // Moderate - some vegetation
            ExtendedBiome::TemperateForest | ExtendedBiome::TropicalForest => 8,
            
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
        
        // Check for parallel roads: if we are not a road, but adjacent to one,
        // apply a huge penalty. This forces paths to either merge onto the road
        // or stay at least 1 tile away, preventing double-width roads.
        let mut parallel_penalty = 0;
        if !tile_history.has_road(x, y) {
             for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        if tile_history.has_road(nx as usize, ny as usize) {
                             parallel_penalty = 50; 
                             break;
                        }
                    }
                }
                if parallel_penalty > 0 { break; }
            }
        }

        // Add varying noise to make roads wavy instead of straight
        // Using sin/cos based on coordinates creates consistent "organic" curves
        let noise_val = ((x as f32 * 0.15).sin() + (y as f32 * 0.25).cos());
        let noise_cost = (noise_val.abs() * 4.0) as u32;
        
        // Height penalty for hills (not mountains)
        let height_penalty = if h > 0.6 { 8 } else if h > 0.5 { 4 } else { 0 };
        
        base_cost + river_penalty + height_penalty + parallel_penalty + noise_cost
    };

    let heuristic = |pos: (usize, usize)| -> u32 {
        let dx = (pos.0 as i64 - to.0 as i64).unsigned_abs() as u32;
        let dy = (pos.1 as i64 - to.1 as i64).unsigned_abs() as u32;
        dx + dy
    };

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut g_score: HashMap<(usize, usize), u32> = HashMap::new();

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
        let living_creatures: Vec<LegendaryCreatureId> = history.legendary_creatures.values()
            .filter(|c| c.is_alive())
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

            let mut desc = String::new();
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

        // Check if the war ended — auto-lift siege
        let war_active = history.wars.get(&war_id).map_or(false, |w| w.is_active());
        if !war_active {
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

        if success {
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
            if def_settlements == 0 {
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
                history.chronicle.record(event);
            }

            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::SiegeEnded,
                date,
                format!("{} falls to {}", target_name, att_name),
                format!("{} captured {} after a siege of {} seasons.",
                    att_name, target_name, duration),
            )
            .with_faction(attacker)
            .with_faction(defender)
            .with_participant(EntityId::Settlement(target));
            history.chronicle.record(event);
        } else {
            // Siege failed — attacker withdraws
            let event_id = history.id_generators.next_event();
            let event = Event::new(
                event_id,
                EventType::SiegeEnded,
                date,
                format!("Siege of {} lifted", target_name),
                format!("{} withdrew from the siege of {} after {} seasons.",
                    att_name, target_name, duration),
            )
            .with_faction(attacker)
            .with_faction(defender)
            .with_participant(EntityId::Settlement(target));
            history.chronicle.record(event);
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
    let owned_artifacts: Vec<(ArtifactId, ArtifactQuality)> = history.artifacts.values()
        .filter(|a| !a.destroyed && !a.lost)
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
        let base_income = (total_pop / 100).max(settlement_count);

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
                     if !is_water && h < 0.9 {
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
