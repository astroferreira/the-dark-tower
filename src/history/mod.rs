//! Historical world enrichment system
//!
//! This module generates evidence of history throughout the world:
//! - Factions (civilizations) with species, culture, and architecture
//! - Historical timeline with eras and events
//! - Territories and settlements with lifecycle states
//! - Monster ecology and lairs
//! - Trade routes and resource sites
//! - Physical evidence (battlefields, monuments, graveyards)
//! - Notable heroes with philosophies and beliefs
//! - Artifacts as lore carriers that move through history
//! - Dungeons and cave systems with historical significance
//!
//! The goal is to make the procedurally generated world feel rich with past history,
//! creating locations that appear to have been used by characters, monsters, and factions
//! over centuries. Dwarf Fortress-style depth without real-time simulation.

pub mod types;
pub mod naming;
pub mod factions;
pub mod timeline;
pub mod territories;
pub mod monsters;
pub mod trade;
pub mod heroes;
pub mod artifacts;
pub mod dungeons;
pub mod evidence;
pub mod integration;

pub use types::*;
pub use factions::{Faction, FactionRegistry, generate_factions};
pub use naming::NameGenerator;
pub use timeline::{HistoricalEvent, EventType, Era, Timeline, generate_timeline};
pub use territories::{Territory, Settlement, generate_territories};
pub use monsters::{MonsterLair, MonsterSpecies, generate_monster_lairs};
pub use trade::{TradeRoute, ResourceSite, generate_trade_network};
pub use heroes::{Hero, HeroRegistry, HeroRole, generate_heroes};
pub use artifacts::{Artifact, ArtifactRegistry, ArtifactLore, ArtifactLocation, generate_artifacts};
pub use dungeons::{Dungeon, DungeonRegistry, DungeonOrigin, generate_dungeons};
pub use evidence::generate_historical_evidence;
pub use integration::{WorldHistory, generate_world_history};
