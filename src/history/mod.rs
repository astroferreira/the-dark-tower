//! History simulation module
//!
//! Simulates world history on top of generated terrain: civilizations,
//! creatures, notable figures, events, artifacts, and more.
//! Inspired by Dwarf Fortress legends mode.

pub mod civilizations;
pub mod config;
pub mod creatures;
pub mod data;
pub mod entities;
pub mod events;
pub mod legends;
pub mod naming;
pub mod objects;
pub mod persistence;
pub mod religion;
pub mod simulation;
pub mod time;
pub mod world_state;

use std::fmt;
use serde::{Serialize, Deserialize};

// =============================================================================
// ID TYPES
// =============================================================================

/// Macro to generate newtype ID wrappers with common derives and Display.
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

// Entity IDs
define_id!(FactionId);
define_id!(SettlementId);
define_id!(FigureId);
define_id!(DynastyId);
define_id!(RaceId);
define_id!(CultureId);
define_id!(NamingStyleId);

// Creature IDs
define_id!(CreatureSpeciesId);
define_id!(LegendaryCreatureId);
define_id!(PopulationId);

// Religion IDs
define_id!(DeityId);
define_id!(ReligionId);
define_id!(CultId);
define_id!(TempleId);

// Object IDs
define_id!(ArtifactId);
define_id!(MonumentId);

// Event IDs
define_id!(EventId);
define_id!(EraId);

// Military / Diplomacy IDs
define_id!(ArmyId);
define_id!(WarId);
define_id!(TreatyId);
define_id!(SiegeId);
define_id!(TradeRouteId);

// Other IDs
define_id!(LanguageId);
define_id!(LairId);

// =============================================================================
// ID GENERATOR
// =============================================================================

/// Thread-safe monotonic ID generator for a specific ID type.
#[derive(Clone, Debug)]
pub struct IdGenerator {
    next: u64,
}

impl IdGenerator {
    pub fn new() -> Self {
        Self { next: 0 }
    }

    /// Start from a specific value (useful when loading saves).
    pub fn starting_at(start: u64) -> Self {
        Self { next: start }
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of ID generators for all entity types.
#[derive(Clone, Debug, Default)]
pub struct IdGenerators {
    pub faction: IdGenerator,
    pub settlement: IdGenerator,
    pub figure: IdGenerator,
    pub dynasty: IdGenerator,
    pub race: IdGenerator,
    pub culture: IdGenerator,
    pub naming_style: IdGenerator,
    pub creature_species: IdGenerator,
    pub legendary_creature: IdGenerator,
    pub population: IdGenerator,
    pub deity: IdGenerator,
    pub religion: IdGenerator,
    pub cult: IdGenerator,
    pub temple: IdGenerator,
    pub artifact: IdGenerator,
    pub monument: IdGenerator,
    pub event: IdGenerator,
    pub era: IdGenerator,
    pub army: IdGenerator,
    pub war: IdGenerator,
    pub treaty: IdGenerator,
    pub siege: IdGenerator,
    pub trade_route: IdGenerator,
    pub language: IdGenerator,
    pub lair: IdGenerator,
}

impl IdGenerators {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_faction(&mut self) -> FactionId { FactionId(self.faction.next_id()) }
    pub fn next_settlement(&mut self) -> SettlementId { SettlementId(self.settlement.next_id()) }
    pub fn next_figure(&mut self) -> FigureId { FigureId(self.figure.next_id()) }
    pub fn next_dynasty(&mut self) -> DynastyId { DynastyId(self.dynasty.next_id()) }
    pub fn next_race(&mut self) -> RaceId { RaceId(self.race.next_id()) }
    pub fn next_culture(&mut self) -> CultureId { CultureId(self.culture.next_id()) }
    pub fn next_naming_style(&mut self) -> NamingStyleId { NamingStyleId(self.naming_style.next_id()) }
    pub fn next_creature_species(&mut self) -> CreatureSpeciesId { CreatureSpeciesId(self.creature_species.next_id()) }
    pub fn next_legendary_creature(&mut self) -> LegendaryCreatureId { LegendaryCreatureId(self.legendary_creature.next_id()) }
    pub fn next_population(&mut self) -> PopulationId { PopulationId(self.population.next_id()) }
    pub fn next_deity(&mut self) -> DeityId { DeityId(self.deity.next_id()) }
    pub fn next_religion(&mut self) -> ReligionId { ReligionId(self.religion.next_id()) }
    pub fn next_cult(&mut self) -> CultId { CultId(self.cult.next_id()) }
    pub fn next_temple(&mut self) -> TempleId { TempleId(self.temple.next_id()) }
    pub fn next_artifact(&mut self) -> ArtifactId { ArtifactId(self.artifact.next_id()) }
    pub fn next_monument(&mut self) -> MonumentId { MonumentId(self.monument.next_id()) }
    pub fn next_event(&mut self) -> EventId { EventId(self.event.next_id()) }
    pub fn next_era(&mut self) -> EraId { EraId(self.era.next_id()) }
    pub fn next_army(&mut self) -> ArmyId { ArmyId(self.army.next_id()) }
    pub fn next_war(&mut self) -> WarId { WarId(self.war.next_id()) }
    pub fn next_treaty(&mut self) -> TreatyId { TreatyId(self.treaty.next_id()) }
    pub fn next_siege(&mut self) -> SiegeId { SiegeId(self.siege.next_id()) }
    pub fn next_trade_route(&mut self) -> TradeRouteId { TradeRouteId(self.trade_route.next_id()) }
    pub fn next_language(&mut self) -> LanguageId { LanguageId(self.language.next_id()) }
    pub fn next_lair(&mut self) -> LairId { LairId(self.lair.next_id()) }
}

// =============================================================================
// ENTITY REFERENCE
// =============================================================================

/// A reference to any entity in the history (for events, inscriptions, etc.)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityId {
    Faction(FactionId),
    Settlement(SettlementId),
    Figure(FigureId),
    Dynasty(DynastyId),
    LegendaryCreature(LegendaryCreatureId),
    CreaturePopulation(PopulationId),
    Deity(DeityId),
    Religion(ReligionId),
    Artifact(ArtifactId),
    Monument(MonumentId),
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityId::Faction(id) => write!(f, "{}", id),
            EntityId::Settlement(id) => write!(f, "{}", id),
            EntityId::Figure(id) => write!(f, "{}", id),
            EntityId::Dynasty(id) => write!(f, "{}", id),
            EntityId::LegendaryCreature(id) => write!(f, "{}", id),
            EntityId::CreaturePopulation(id) => write!(f, "{}", id),
            EntityId::Deity(id) => write!(f, "{}", id),
            EntityId::Religion(id) => write!(f, "{}", id),
            EntityId::Artifact(id) => write!(f, "{}", id),
            EntityId::Monument(id) => write!(f, "{}", id),
        }
    }
}
