//! The complete WorldHistory database.
//!
//! This is the master struct that holds all entities, events, and state
//! produced by the history simulation. Everything is stored in HashMaps
//! keyed by their respective ID types.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use crate::history::{
    FactionId, SettlementId, FigureId, DynastyId, RaceId, CultureId,
    CreatureSpeciesId, LegendaryCreatureId, PopulationId,
    DeityId, ReligionId, CultId,
    ArtifactId, MonumentId,
    WarId, ArmyId, SiegeId, TradeRouteId,
    IdGenerators,
};
use crate::history::civilizations::economy::TradeRoute;
use crate::history::config::HistoryConfig;
use crate::history::time::{Date, Timeline};
use crate::history::events::chronicle::Chronicle;
use crate::history::entities::races::Race;
use crate::history::entities::culture::Culture;
use crate::history::entities::figures::Figure;
use crate::history::entities::lineage::Dynasty;
use crate::history::civilizations::faction::Faction;
use crate::history::civilizations::settlement::Settlement;
use crate::history::civilizations::military::{Army, War, Siege};
use crate::history::creatures::generator::CreatureSpecies;
use crate::history::creatures::legendary::LegendaryCreature;
use crate::history::creatures::populations::CreaturePopulation;
use crate::history::religion::deity::Deity;
use crate::history::religion::worship::Religion;
use crate::history::religion::monster_cults::MonsterCult;
use crate::history::objects::artifacts::Artifact;
use crate::history::objects::monuments::Monument;
use super::tile_history::TileHistoryMap;

/// The complete world history database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldHistory {
    // Configuration
    pub config: HistoryConfig,

    // Current simulation date
    pub current_date: Date,

    // Timeline (eras)
    pub timeline: Timeline,

    // Chronicle (all events)
    pub chronicle: Chronicle,

    // Tile-level history
    pub tile_history: TileHistoryMap,

    // === Entity stores ===

    // Races and cultures
    pub races: HashMap<RaceId, Race>,
    pub cultures: HashMap<CultureId, Culture>,

    // Civilizations
    pub factions: HashMap<FactionId, Faction>,
    pub settlements: HashMap<SettlementId, Settlement>,

    // People
    pub figures: HashMap<FigureId, Figure>,
    pub dynasties: HashMap<DynastyId, Dynasty>,

    // Military
    pub armies: HashMap<ArmyId, Army>,
    pub wars: HashMap<WarId, War>,
    pub sieges: HashMap<SiegeId, Siege>,

    // Creatures
    pub creature_species: HashMap<CreatureSpeciesId, CreatureSpecies>,
    pub legendary_creatures: HashMap<LegendaryCreatureId, LegendaryCreature>,
    pub populations: HashMap<PopulationId, CreaturePopulation>,

    // Religion
    pub deities: HashMap<DeityId, Deity>,
    pub religions: HashMap<ReligionId, Religion>,
    pub cults: HashMap<CultId, MonsterCult>,

    // Objects
    pub artifacts: HashMap<ArtifactId, Artifact>,
    pub monuments: HashMap<MonumentId, Monument>,

    // Economy
    pub trade_routes: HashMap<TradeRouteId, TradeRoute>,

    // ID generators (not serialized - rebuilt from max IDs on load)
    #[serde(skip)]
    pub id_generators: IdGenerators,
}

impl WorldHistory {
    /// Create a new empty world history for a given map size.
    pub fn new(config: HistoryConfig, map_width: usize, map_height: usize, start_date: Date) -> Self {
        Self {
            config,
            current_date: start_date,
            timeline: Timeline::new(),
            chronicle: Chronicle::new(),
            tile_history: TileHistoryMap::new(map_width, map_height),
            races: HashMap::new(),
            cultures: HashMap::new(),
            factions: HashMap::new(),
            settlements: HashMap::new(),
            figures: HashMap::new(),
            dynasties: HashMap::new(),
            armies: HashMap::new(),
            wars: HashMap::new(),
            sieges: HashMap::new(),
            creature_species: HashMap::new(),
            legendary_creatures: HashMap::new(),
            populations: HashMap::new(),
            deities: HashMap::new(),
            religions: HashMap::new(),
            cults: HashMap::new(),
            artifacts: HashMap::new(),
            monuments: HashMap::new(),
            trade_routes: HashMap::new(),
            id_generators: IdGenerators::new(),
        }
    }

    // === Convenience accessors ===

    /// Number of active (non-dissolved) factions.
    pub fn active_faction_count(&self) -> usize {
        self.factions.values().filter(|f| f.is_active()).count()
    }

    /// Number of living figures.
    pub fn living_figure_count(&self) -> usize {
        self.figures.values().filter(|f| f.is_alive()).count()
    }

    /// Number of living legendary creatures.
    pub fn living_legendary_count(&self) -> usize {
        self.legendary_creatures.values().filter(|c| c.is_alive()).count()
    }

    /// Total world population across all settlements.
    pub fn total_population(&self) -> u64 {
        self.settlements.values()
            .filter(|s| !s.is_destroyed())
            .map(|s| s.population as u64)
            .sum()
    }

    /// Get a faction by ID.
    pub fn faction(&self, id: FactionId) -> Option<&Faction> {
        self.factions.get(&id)
    }

    /// Get a mutable faction by ID.
    pub fn faction_mut(&mut self, id: FactionId) -> Option<&mut Faction> {
        self.factions.get_mut(&id)
    }

    /// Get a settlement by ID.
    pub fn settlement(&self, id: SettlementId) -> Option<&Settlement> {
        self.settlements.get(&id)
    }

    /// Get a figure by ID.
    pub fn figure(&self, id: FigureId) -> Option<&Figure> {
        self.figures.get(&id)
    }

    /// Get a legendary creature by ID.
    pub fn legendary_creature(&self, id: LegendaryCreatureId) -> Option<&LegendaryCreature> {
        self.legendary_creatures.get(&id)
    }

    /// Summary statistics for display.
    pub fn summary(&self) -> HistorySummary {
        HistorySummary {
            years_simulated: self.current_date.year,
            total_events: self.chronicle.len(),
            major_events: self.chronicle.major_events().len(),
            total_factions: self.factions.len(),
            active_factions: self.active_faction_count(),
            total_settlements: self.settlements.len(),
            total_figures: self.figures.len(),
            living_figures: self.living_figure_count(),
            total_dynasties: self.dynasties.len(),
            legendary_creatures: self.legendary_creatures.len(),
            living_legendary: self.living_legendary_count(),
            artifacts: self.artifacts.len(),
            monuments: self.monuments.len(),
            religions: self.religions.len(),
            wars: self.wars.len(),
            total_population: self.total_population(),
        }
    }
}

/// Summary statistics for display.
#[derive(Clone, Debug)]
pub struct HistorySummary {
    pub years_simulated: u32,
    pub total_events: usize,
    pub major_events: usize,
    pub total_factions: usize,
    pub active_factions: usize,
    pub total_settlements: usize,
    pub total_figures: usize,
    pub living_figures: usize,
    pub total_dynasties: usize,
    pub legendary_creatures: usize,
    pub living_legendary: usize,
    pub artifacts: usize,
    pub monuments: usize,
    pub religions: usize,
    pub wars: usize,
    pub total_population: u64,
}

impl std::fmt::Display for HistorySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== World History: {} years ==="  , self.years_simulated)?;
        writeln!(f, "Events: {} ({} major)", self.total_events, self.major_events)?;
        writeln!(f, "Factions: {} ({} active)", self.total_factions, self.active_factions)?;
        writeln!(f, "Settlements: {}", self.total_settlements)?;
        writeln!(f, "Figures: {} ({} living)", self.total_figures, self.living_figures)?;
        writeln!(f, "Dynasties: {}", self.total_dynasties)?;
        writeln!(f, "Legendary creatures: {} ({} alive)", self.legendary_creatures, self.living_legendary)?;
        writeln!(f, "Artifacts: {}, Monuments: {}", self.artifacts, self.monuments)?;
        writeln!(f, "Religions: {}, Wars: {}", self.religions, self.wars)?;
        writeln!(f, "World population: {}", self.total_population)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_world_history_creation() {
        let config = HistoryConfig::default();
        let history = WorldHistory::new(config, 100, 50, Date::new(1, Season::Spring));

        assert_eq!(history.active_faction_count(), 0);
        assert_eq!(history.living_figure_count(), 0);
        assert_eq!(history.total_population(), 0);
        assert!(history.chronicle.is_empty());
    }

    #[test]
    fn test_world_history_summary() {
        let config = HistoryConfig::default();
        let history = WorldHistory::new(config, 100, 50, Date::new(1, Season::Spring));
        let summary = history.summary();
        assert_eq!(summary.total_events, 0);
        assert_eq!(summary.active_factions, 0);
    }
}
