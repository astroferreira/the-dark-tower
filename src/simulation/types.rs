//! Core types for the civilization simulation system

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a tribe
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TribeId(pub u32);

impl fmt::Display for TribeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tribe#{}", self.0)
    }
}

/// Simulation time unit (4 ticks = 1 year, representing seasons)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SimTick(pub u64);

impl SimTick {
    pub fn year(&self) -> u64 {
        self.0 / 4
    }

    pub fn season(&self) -> Season {
        match self.0 % 4 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    pub fn next(&self) -> SimTick {
        SimTick(self.0 + 1)
    }
}

impl fmt::Display for SimTick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Year {} {:?}", self.year(), self.season())
    }
}

/// Season of the year
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Temperature modifier for the season
    pub fn temperature_modifier(&self) -> f32 {
        match self {
            Season::Spring => 0.0,
            Season::Summer => 10.0,
            Season::Winter => -15.0,
            Season::Autumn => -5.0,
        }
    }

    /// Food production modifier
    pub fn food_modifier(&self) -> f32 {
        match self {
            Season::Spring => 1.2,  // Growing season
            Season::Summer => 1.5,  // Peak harvest
            Season::Autumn => 1.0,  // Harvest ends
            Season::Winter => 0.3,  // Scarcity
        }
    }
}

/// Relation level between tribes (-100 to +100)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationLevel(pub i8);

impl RelationLevel {
    pub const HOSTILE: RelationLevel = RelationLevel(-100);
    pub const UNFRIENDLY: RelationLevel = RelationLevel(-30);
    pub const NEUTRAL: RelationLevel = RelationLevel(0);
    pub const FRIENDLY: RelationLevel = RelationLevel(30);
    pub const ALLIED: RelationLevel = RelationLevel(80);

    pub fn new(value: i8) -> Self {
        RelationLevel(value.clamp(-100, 100))
    }

    pub fn adjust(&mut self, delta: i8) {
        self.0 = (self.0 as i16 + delta as i16).clamp(-100, 100) as i8;
    }

    pub fn status(&self) -> RelationStatus {
        match self.0 {
            -100..=-50 => RelationStatus::Hostile,
            -49..=-10 => RelationStatus::Unfriendly,
            -9..=9 => RelationStatus::Neutral,
            10..=49 => RelationStatus::Friendly,
            50..=100 => RelationStatus::Allied,
            _ => RelationStatus::Neutral,
        }
    }
}

impl Default for RelationLevel {
    fn default() -> Self {
        RelationLevel::NEUTRAL
    }
}

/// Diplomatic status derived from relation level
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationStatus {
    Hostile,
    Unfriendly,
    Neutral,
    Friendly,
    Allied,
}

/// Resource types available in the simulation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    // Basic resources
    Food,
    Water,
    Wood,
    Stone,

    // Metals (unlock with technology)
    Copper,
    Tin,
    Bronze,
    Iron,
    Gold,
    Silver,

    // Advanced materials
    Coal,
    Leather,
    Cloth,
    Clay,
    Tools,
    Weapons,

    // Luxury goods
    Gems,
    Spices,
    Salt,
    Obsidian,
}

impl ResourceType {
    /// Resources available from the start (no tech required)
    pub fn basic_resources() -> &'static [ResourceType] {
        &[
            ResourceType::Food,
            ResourceType::Water,
            ResourceType::Wood,
            ResourceType::Stone,
        ]
    }

    /// Check if this is a metal resource
    pub fn is_metal(&self) -> bool {
        matches!(
            self,
            ResourceType::Copper
                | ResourceType::Tin
                | ResourceType::Bronze
                | ResourceType::Iron
                | ResourceType::Gold
                | ResourceType::Silver
        )
    }

    /// Check if this is a luxury resource
    pub fn is_luxury(&self) -> bool {
        matches!(
            self,
            ResourceType::Gems
                | ResourceType::Spices
                | ResourceType::Salt
                | ResourceType::Obsidian
        )
    }

    /// Base decay rate per tick (0.0 = no decay, 1.0 = complete decay)
    pub fn decay_rate(&self) -> f32 {
        match self {
            ResourceType::Food => 0.05,   // Food spoils
            ResourceType::Water => 0.0,   // Water doesn't decay (represents access)
            ResourceType::Leather => 0.01,
            ResourceType::Cloth => 0.01,
            _ => 0.0,  // Most resources don't decay
        }
    }

    /// Trade value multiplier (relative to food)
    pub fn trade_value(&self) -> f32 {
        match self {
            ResourceType::Food => 1.0,
            ResourceType::Water => 0.5,
            ResourceType::Wood => 1.5,
            ResourceType::Stone => 1.5,
            ResourceType::Copper => 5.0,
            ResourceType::Tin => 6.0,
            ResourceType::Bronze => 15.0,
            ResourceType::Iron => 20.0,
            ResourceType::Gold => 50.0,
            ResourceType::Silver => 30.0,
            ResourceType::Coal => 3.0,
            ResourceType::Leather => 2.0,
            ResourceType::Cloth => 3.0,
            ResourceType::Clay => 1.0,
            ResourceType::Tools => 10.0,
            ResourceType::Weapons => 25.0,
            ResourceType::Gems => 100.0,
            ResourceType::Spices => 40.0,
            ResourceType::Salt => 8.0,
            ResourceType::Obsidian => 15.0,
        }
    }
}

/// Tile coordinate on the world map
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: usize,
    pub y: usize,
}

impl TileCoord {
    pub fn new(x: usize, y: usize) -> Self {
        TileCoord { x, y }
    }

    /// Manhattan distance to another coordinate
    pub fn distance_to(&self, other: &TileCoord) -> usize {
        let dx = (self.x as i32 - other.x as i32).unsigned_abs() as usize;
        let dy = (self.y as i32 - other.y as i32).unsigned_abs() as usize;
        dx + dy
    }

    /// Distance with horizontal wrapping
    pub fn distance_wrapped(&self, other: &TileCoord, map_width: usize) -> usize {
        let dx1 = (self.x as i32 - other.x as i32).unsigned_abs() as usize;
        let dx2 = map_width - dx1;
        let dx = dx1.min(dx2);
        let dy = (self.y as i32 - other.y as i32).unsigned_abs() as usize;
        dx + dy
    }
}

impl fmt::Display for TileCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// Types of treaties between tribes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TreatyType {
    NonAggression,
    TradeAgreement,
    DefensiveAlliance,
    MilitaryAlliance,
    Vassalage,
}

impl TreatyType {
    /// Minimum relation level required to propose this treaty
    pub fn required_relation(&self) -> RelationLevel {
        match self {
            TreatyType::NonAggression => RelationLevel(-10),
            TreatyType::TradeAgreement => RelationLevel(10),
            TreatyType::DefensiveAlliance => RelationLevel(40),
            TreatyType::MilitaryAlliance => RelationLevel(60),
            TreatyType::Vassalage => RelationLevel(-100), // Can be forced
        }
    }

    /// Default duration in ticks
    pub fn default_duration(&self) -> u64 {
        match self {
            TreatyType::NonAggression => 40,      // 10 years
            TreatyType::TradeAgreement => 20,     // 5 years
            TreatyType::DefensiveAlliance => 40,  // 10 years
            TreatyType::MilitaryAlliance => 20,   // 5 years
            TreatyType::Vassalage => u64::MAX,    // Permanent until broken
        }
    }
}

/// An active treaty between two tribes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Treaty {
    pub treaty_type: TreatyType,
    pub tribe_a: TribeId,
    pub tribe_b: TribeId,
    pub started_tick: SimTick,
    pub expires_tick: Option<SimTick>,
}

impl Treaty {
    pub fn new(
        treaty_type: TreatyType,
        tribe_a: TribeId,
        tribe_b: TribeId,
        current_tick: SimTick,
    ) -> Self {
        let duration = treaty_type.default_duration();
        let expires = if duration == u64::MAX {
            None
        } else {
            Some(SimTick(current_tick.0 + duration))
        };

        Treaty {
            treaty_type,
            tribe_a,
            tribe_b,
            started_tick: current_tick,
            expires_tick: expires,
        }
    }

    pub fn is_expired(&self, current_tick: SimTick) -> bool {
        self.expires_tick.map(|e| current_tick >= e).unwrap_or(false)
    }

    pub fn involves(&self, tribe: TribeId) -> bool {
        self.tribe_a == tribe || self.tribe_b == tribe
    }
}

/// Event types that can happen to a tribe
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TribeEventType {
    // Foundation
    Founded { location: TileCoord },

    // Population
    PopulationGrowth { amount: u32 },
    PopulationDecline { amount: u32, cause: String },
    TribeSplit { new_tribe: TribeId },

    // Territory
    TerritoryExpanded { tile: TileCoord },
    TerritoryLost { tile: TileCoord, to: Option<TribeId> },
    SettlementFounded { location: TileCoord },

    // Technology
    AgeAdvanced { new_age: String },
    TechUnlocked { tech: String },
    BuildingConstructed { building: String, location: TileCoord },

    // Diplomacy
    TreatyFormed { with: TribeId, treaty_type: TreatyType },
    TreatyBroken { with: TribeId, treaty_type: TreatyType },
    WarDeclared { against: TribeId },
    PeaceMade { with: TribeId },

    // Conflict
    RaidLaunched { target: TribeId, success: bool },
    RaidDefended { attacker: TribeId, success: bool },
    BattleWon { against: TribeId },
    BattleLost { against: TribeId },

    // Trade
    TradeCompleted { with: TribeId, gave: Vec<(ResourceType, f32)>, received: Vec<(ResourceType, f32)> },

    // Crisis
    Famine { severity: f32 },
    Plague { deaths: u32 },
    NaturalDisaster { disaster_type: String },

    // Monster-related
    MonsterAttack { monster_type: String, casualties: u32 },
    MonsterSlain { monster_type: String, slayer_tribe: Option<TribeId> },
}

/// A recorded event in tribe history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TribeEvent {
    pub tick: SimTick,
    pub event_type: TribeEventType,
}

impl TribeEvent {
    pub fn new(tick: SimTick, event_type: TribeEventType) -> Self {
        TribeEvent { tick, event_type }
    }
}
