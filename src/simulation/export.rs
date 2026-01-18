//! Export simulation state to JSON

use std::fs::File;
use std::io::Write;
use serde::{Deserialize, Serialize};

use crate::simulation::simulation::SimulationState;
use crate::simulation::types::{TribeId, SimTick};

/// Exported simulation data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationExport {
    pub seed: u64,
    pub final_tick: SimTick,
    pub stats: SimulationStatsExport,
    pub tribes: Vec<TribeExport>,
    pub diplomacy: DiplomacyExport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationStatsExport {
    pub total_tribes_created: u32,
    pub total_tribes_extinct: u32,
    pub living_tribes: usize,
    pub total_battles: u32,
    pub total_raids: u32,
    pub total_trades: u32,
    pub total_treaties: u32,
    pub total_age_advances: u32,
    pub peak_population: u32,
    pub final_population: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TribeExport {
    pub id: u32,
    pub name: String,
    pub is_alive: bool,
    pub population: u32,
    pub warriors: u32,
    pub territory_size: usize,
    pub capital: (usize, usize),
    pub age: String,
    pub culture: String,
    pub food_satisfaction: f32,
    pub morale: f32,
    pub military_strength: f32,
    pub significant_events: Vec<EventExport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventExport {
    pub tick: u64,
    pub year: u64,
    pub event_type: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiplomacyExport {
    pub relations: Vec<RelationExport>,
    pub treaties: Vec<TreatyExport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelationExport {
    pub tribe_a: u32,
    pub tribe_b: u32,
    pub level: i8,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreatyExport {
    pub treaty_type: String,
    pub tribe_a: u32,
    pub tribe_b: u32,
    pub started_year: u64,
}

/// Export simulation state to JSON file
pub fn export_simulation(state: &SimulationState, path: &str) -> std::io::Result<()> {
    let export = create_export(state);
    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;

    Ok(())
}

/// Create export structure from simulation state
fn create_export(state: &SimulationState) -> SimulationExport {
    let living_count = state.tribes.values().filter(|t| t.is_alive).count();

    let stats = SimulationStatsExport {
        total_tribes_created: state.stats.total_tribes_created,
        total_tribes_extinct: state.stats.total_tribes_extinct,
        living_tribes: living_count,
        total_battles: state.stats.total_battles,
        total_raids: state.stats.total_raids,
        total_trades: state.stats.total_trades,
        total_treaties: state.stats.total_treaties,
        total_age_advances: state.stats.total_age_advances,
        peak_population: state.stats.peak_population,
        final_population: state.stats.current_population,
    };

    let tribes: Vec<TribeExport> = state.tribes.values().map(|tribe| {
        let significant_events: Vec<EventExport> = tribe.events
            .iter()
            .filter(|e| is_significant_event(&e.event_type))
            .take(20) // Limit to 20 most important events
            .map(|e| EventExport {
                tick: e.tick.0,
                year: e.tick.year(),
                event_type: event_type_name(&e.event_type),
                description: event_description(&e.event_type),
            })
            .collect();

        TribeExport {
            id: tribe.id.0,
            name: tribe.name.clone(),
            is_alive: tribe.is_alive,
            population: tribe.population.total(),
            warriors: tribe.population.warriors(),
            territory_size: tribe.territory.len(),
            capital: (tribe.capital.x, tribe.capital.y),
            age: format!("{:?}", tribe.tech_state.current_age()),
            culture: tribe.culture.name().to_string(),
            food_satisfaction: tribe.needs.food.satisfaction,
            morale: tribe.needs.morale.satisfaction,
            military_strength: tribe.military_strength(),
            significant_events,
        }
    }).collect();

    let relations: Vec<RelationExport> = state.diplomacy
        .get_related_tribes(TribeId(0)) // Hack: iterate all
        .iter()
        .map(|(other_id, level)| RelationExport {
            tribe_a: 0,
            tribe_b: other_id.0,
            level: level.0,
            status: format!("{:?}", level.status()),
        })
        .collect();

    // Get all unique relation pairs
    let mut all_relations = Vec::new();
    let tribe_ids: Vec<TribeId> = state.tribes.keys().copied().collect();
    for (i, &tribe_a) in tribe_ids.iter().enumerate() {
        for &tribe_b in tribe_ids.iter().skip(i + 1) {
            let level = state.diplomacy.get_relation(tribe_a, tribe_b);
            all_relations.push(RelationExport {
                tribe_a: tribe_a.0,
                tribe_b: tribe_b.0,
                level: level.0,
                status: format!("{:?}", level.status()),
            });
        }
    }

    let treaties: Vec<TreatyExport> = state.diplomacy
        .get_treaties(TribeId(0)) // All treaties
        .iter()
        .map(|t| TreatyExport {
            treaty_type: format!("{:?}", t.treaty_type),
            tribe_a: t.tribe_a.0,
            tribe_b: t.tribe_b.0,
            started_year: t.started_tick.year(),
        })
        .collect();

    let diplomacy = DiplomacyExport {
        relations: all_relations,
        treaties,
    };

    SimulationExport {
        seed: state.seed,
        final_tick: state.current_tick,
        stats,
        tribes,
        diplomacy,
    }
}

/// Check if an event is significant enough to export
fn is_significant_event(event: &crate::simulation::types::TribeEventType) -> bool {
    use crate::simulation::types::TribeEventType::*;

    matches!(
        event,
        Founded { .. }
            | TribeSplit { .. }
            | AgeAdvanced { .. }
            | TreatyFormed { .. }
            | WarDeclared { .. }
            | BattleWon { .. }
            | BattleLost { .. }
            | Famine { .. }
    )
}

/// Get event type name for export
fn event_type_name(event: &crate::simulation::types::TribeEventType) -> String {
    use crate::simulation::types::TribeEventType::*;

    match event {
        Founded { .. } => "Founded".to_string(),
        PopulationGrowth { .. } => "PopulationGrowth".to_string(),
        PopulationDecline { .. } => "PopulationDecline".to_string(),
        TribeSplit { .. } => "TribeSplit".to_string(),
        TerritoryExpanded { .. } => "TerritoryExpanded".to_string(),
        TerritoryLost { .. } => "TerritoryLost".to_string(),
        SettlementFounded { .. } => "SettlementFounded".to_string(),
        AgeAdvanced { .. } => "AgeAdvanced".to_string(),
        TechUnlocked { .. } => "TechUnlocked".to_string(),
        BuildingConstructed { .. } => "BuildingConstructed".to_string(),
        TreatyFormed { .. } => "TreatyFormed".to_string(),
        TreatyBroken { .. } => "TreatyBroken".to_string(),
        WarDeclared { .. } => "WarDeclared".to_string(),
        PeaceMade { .. } => "PeaceMade".to_string(),
        RaidLaunched { .. } => "RaidLaunched".to_string(),
        RaidDefended { .. } => "RaidDefended".to_string(),
        BattleWon { .. } => "BattleWon".to_string(),
        BattleLost { .. } => "BattleLost".to_string(),
        TradeCompleted { .. } => "TradeCompleted".to_string(),
        Famine { .. } => "Famine".to_string(),
        Plague { .. } => "Plague".to_string(),
        NaturalDisaster { .. } => "NaturalDisaster".to_string(),
        MonsterAttack { .. } => "MonsterAttack".to_string(),
        MonsterSlain { .. } => "MonsterSlain".to_string(),
    }
}

/// Generate human-readable event description
fn event_description(event: &crate::simulation::types::TribeEventType) -> String {
    use crate::simulation::types::TribeEventType::*;

    match event {
        Founded { location } => format!("Tribe founded at ({}, {})", location.x, location.y),
        PopulationGrowth { amount } => format!("Population grew by {}", amount),
        PopulationDecline { amount, cause } => format!("Population declined by {} due to {}", amount, cause),
        TribeSplit { new_tribe } => format!("Tribe split, new tribe {} formed", new_tribe.0),
        TerritoryExpanded { tile } => format!("Expanded to ({}, {})", tile.x, tile.y),
        TerritoryLost { tile, to } => {
            if let Some(conqueror) = to {
                format!("Lost ({}, {}) to tribe {}", tile.x, tile.y, conqueror.0)
            } else {
                format!("Abandoned ({}, {})", tile.x, tile.y)
            }
        }
        SettlementFounded { location } => format!("Settlement founded at ({}, {})", location.x, location.y),
        AgeAdvanced { new_age } => format!("Advanced to {} Age", new_age),
        TechUnlocked { tech } => format!("Unlocked {}", tech),
        BuildingConstructed { building, .. } => format!("Constructed {}", building),
        TreatyFormed { with, treaty_type } => format!("{:?} treaty with tribe {}", treaty_type, with.0),
        TreatyBroken { with, treaty_type } => format!("Broke {:?} treaty with tribe {}", treaty_type, with.0),
        WarDeclared { against } => format!("Declared war on tribe {}", against.0),
        PeaceMade { with } => format!("Made peace with tribe {}", with.0),
        RaidLaunched { target, success } => {
            if *success {
                format!("Successful raid against tribe {}", target.0)
            } else {
                format!("Failed raid against tribe {}", target.0)
            }
        }
        RaidDefended { attacker, success } => {
            if *success {
                format!("Successfully defended raid from tribe {}", attacker.0)
            } else {
                format!("Failed to defend raid from tribe {}", attacker.0)
            }
        }
        BattleWon { against } => format!("Won battle against tribe {}", against.0),
        BattleLost { against } => format!("Lost battle against tribe {}", against.0),
        TradeCompleted { with, .. } => format!("Trade completed with tribe {}", with.0),
        Famine { severity } => format!("Famine struck (severity: {:.0}%)", severity * 100.0),
        Plague { deaths } => format!("Plague killed {} people", deaths),
        NaturalDisaster { disaster_type } => format!("{} struck", disaster_type),
        MonsterAttack { monster_type, casualties } => format!("{} attack killed {} people", monster_type, casualties),
        MonsterSlain { monster_type, slayer_tribe } => {
            if let Some(tribe_id) = slayer_tribe {
                format!("{} slain by tribe {}", monster_type, tribe_id.0)
            } else {
                format!("{} slain", monster_type)
            }
        }
    }
}

/// Export combat logs to JSON file
pub fn export_combat_logs(state: &SimulationState, path: &str) -> std::io::Result<()> {
    use crate::simulation::combat::CombatLogStats;

    #[derive(Serialize)]
    struct CombatLogExport {
        stats: CombatLogStats,
        encounters: Vec<crate::simulation::combat::CombatEncounterLog>,
    }

    let export = CombatLogExport {
        stats: state.combat_log.stats(),
        encounters: state.combat_log.all_encounters().to_vec(),
    };

    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;

    Ok(())
}

/// Generate a narrative combat report
pub fn generate_combat_narrative(state: &SimulationState) -> String {
    let mut narrative = String::new();

    narrative.push_str("=== Combat Chronicle ===\n\n");

    let stats = state.combat_log.stats();
    narrative.push_str(&format!(
        "Total Encounters: {}\n",
        stats.total_encounters
    ));
    narrative.push_str(&format!(
        "Total Attacks: {} | Kills: {} | Wounds: {}\n\n",
        stats.total_attacks, stats.total_kills, stats.total_wounds
    ));

    // Show recent encounters
    let recent = state.combat_log.recent_encounters(10);
    for encounter in recent {
        narrative.push_str(&encounter.full_narrative());
        narrative.push_str("\n---\n\n");
    }

    narrative
}

/// Generate a summary text report of the simulation
pub fn generate_summary(state: &SimulationState) -> String {
    let mut summary = String::new();

    summary.push_str(&format!(
        "=== Simulation Summary (Seed: {}) ===\n",
        state.seed
    ));
    summary.push_str(&format!(
        "Duration: {} years ({} ticks)\n\n",
        state.current_tick.year(),
        state.current_tick.0
    ));

    summary.push_str("--- Statistics ---\n");
    summary.push_str(&format!(
        "Tribes: {} created, {} extinct, {} surviving\n",
        state.stats.total_tribes_created,
        state.stats.total_tribes_extinct,
        state.stats.total_tribes_created - state.stats.total_tribes_extinct
    ));
    summary.push_str(&format!(
        "Population: {} current, {} peak\n",
        state.stats.current_population, state.stats.peak_population
    ));
    summary.push_str(&format!(
        "Conflicts: {} battles, {} raids\n",
        state.stats.total_battles, state.stats.total_raids
    ));
    summary.push_str(&format!(
        "Diplomacy: {} trades, {} treaties\n",
        state.stats.total_trades, state.stats.total_treaties
    ));
    summary.push_str(&format!(
        "Progress: {} age advances\n\n",
        state.stats.total_age_advances
    ));

    summary.push_str("--- Living Tribes ---\n");
    let mut living: Vec<_> = state.tribes.values().filter(|t| t.is_alive).collect();
    living.sort_by(|a, b| b.population.total().cmp(&a.population.total()));

    for tribe in living.iter().take(10) {
        summary.push_str(&format!(
            "  {} ({}): Pop {} | Territory {} | {:?} Age\n",
            tribe.name,
            tribe.id.0,
            tribe.population.total(),
            tribe.territory.len(),
            tribe.tech_state.current_age()
        ));
    }

    if living.len() > 10 {
        summary.push_str(&format!("  ... and {} more tribes\n", living.len() - 10));
    }

    summary
}
