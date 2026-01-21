//! Shared types for the history system
//!
//! Contains common IDs, enums, and traits used across the history module.

use std::fmt;

/// Unique identifier for a faction
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FactionId(pub u32);

impl fmt::Display for FactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Faction#{}", self.0)
    }
}

/// Unique identifier for a settlement
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct SettlementId(pub u32);

impl fmt::Display for SettlementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Settlement#{}", self.0)
    }
}

/// Unique identifier for a historical event
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct EventId(pub u32);

/// Unique identifier for a monster lair
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct LairId(pub u32);

/// Unique identifier for a trade route
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TradeRouteId(pub u32);

/// Unique identifier for a hero/notable figure
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct HeroId(pub u32);

impl fmt::Display for HeroId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hero#{}", self.0)
    }
}

/// Unique identifier for an artifact
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ArtifactId(pub u32);

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Artifact#{}", self.0)
    }
}

/// Unique identifier for a dungeon
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct DungeonId(pub u32);

impl fmt::Display for DungeonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dungeon#{}", self.0)
    }
}

/// Species of intelligent beings that can form civilizations
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Species {
    Human,
    Dwarf,
    Elf,
    Orc,
    Goblin,
    Giant,
    DragonKin,
    Undead,
    Elemental,
}

impl Species {
    /// Get all species variants
    pub fn all() -> &'static [Species] {
        &[
            Species::Human,
            Species::Dwarf,
            Species::Elf,
            Species::Orc,
            Species::Goblin,
            Species::Giant,
            Species::DragonKin,
            Species::Undead,
            Species::Elemental,
        ]
    }

    /// Get the display name for this species
    pub fn name(&self) -> &'static str {
        match self {
            Species::Human => "Human",
            Species::Dwarf => "Dwarf",
            Species::Elf => "Elf",
            Species::Orc => "Orc",
            Species::Goblin => "Goblin",
            Species::Giant => "Giant",
            Species::DragonKin => "Dragon-kin",
            Species::Undead => "Undead",
            Species::Elemental => "Elemental",
        }
    }

    /// Get the plural name for this species
    pub fn plural(&self) -> &'static str {
        match self {
            Species::Human => "Humans",
            Species::Dwarf => "Dwarves",
            Species::Elf => "Elves",
            Species::Orc => "Orcs",
            Species::Goblin => "Goblins",
            Species::Giant => "Giants",
            Species::DragonKin => "Dragon-kin",
            Species::Undead => "Undead",
            Species::Elemental => "Elementals",
        }
    }

    /// Get preferred terrain (biome categories)
    pub fn preferred_terrain(&self) -> &'static [TerrainPreference] {
        match self {
            Species::Human => &[TerrainPreference::Temperate, TerrainPreference::Coastal, TerrainPreference::Plains],
            Species::Dwarf => &[TerrainPreference::Mountain, TerrainPreference::Underground, TerrainPreference::Hills],
            Species::Elf => &[TerrainPreference::Forest, TerrainPreference::Temperate],
            Species::Orc => &[TerrainPreference::Wasteland, TerrainPreference::Hills, TerrainPreference::Mountain],
            Species::Goblin => &[TerrainPreference::Underground, TerrainPreference::Swamp, TerrainPreference::Wasteland],
            Species::Giant => &[TerrainPreference::Mountain, TerrainPreference::Tundra, TerrainPreference::Hills],
            Species::DragonKin => &[TerrainPreference::Mountain, TerrainPreference::Volcanic, TerrainPreference::Desert],
            Species::Undead => &[TerrainPreference::Wasteland, TerrainPreference::Swamp, TerrainPreference::Underground],
            Species::Elemental => &[TerrainPreference::Volcanic, TerrainPreference::Desert, TerrainPreference::Tundra],
        }
    }

    /// How aggressive this species is (affects war likelihood)
    pub fn aggression(&self) -> f32 {
        match self {
            Species::Human => 0.5,
            Species::Dwarf => 0.4,
            Species::Elf => 0.2,
            Species::Orc => 0.9,
            Species::Goblin => 0.7,
            Species::Giant => 0.6,
            Species::DragonKin => 0.8,
            Species::Undead => 0.95,
            Species::Elemental => 0.5,
        }
    }
}

/// Terrain preference categories for faction placement
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TerrainPreference {
    Mountain,
    Forest,
    Plains,
    Desert,
    Swamp,
    Tundra,
    Coastal,
    Underground,
    Volcanic,
    Hills,
    Temperate,
    Wasteland,
}

/// Cultural characteristics of a faction
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CultureType {
    Militaristic,
    Mercantile,
    Scholarly,
    Religious,
    Nomadic,
    Industrial,
    Isolationist,
    Expansionist,
}

impl CultureType {
    pub fn all() -> &'static [CultureType] {
        &[
            CultureType::Militaristic,
            CultureType::Mercantile,
            CultureType::Scholarly,
            CultureType::Religious,
            CultureType::Nomadic,
            CultureType::Industrial,
            CultureType::Isolationist,
            CultureType::Expansionist,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            CultureType::Militaristic => "Militaristic",
            CultureType::Mercantile => "Mercantile",
            CultureType::Scholarly => "Scholarly",
            CultureType::Religious => "Religious",
            CultureType::Nomadic => "Nomadic",
            CultureType::Industrial => "Industrial",
            CultureType::Isolationist => "Isolationist",
            CultureType::Expansionist => "Expansionist",
        }
    }

    /// How likely this culture is to build monuments
    pub fn monument_affinity(&self) -> f32 {
        match self {
            CultureType::Militaristic => 0.6,
            CultureType::Mercantile => 0.4,
            CultureType::Scholarly => 0.7,
            CultureType::Religious => 0.9,
            CultureType::Nomadic => 0.1,
            CultureType::Industrial => 0.3,
            CultureType::Isolationist => 0.5,
            CultureType::Expansionist => 0.7,
        }
    }
}

/// Architectural style of a faction's buildings
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArchitectureStyle {
    Imperial,
    Rustic,
    Dwarven,
    Elven,
    Orcish,
    Ancient,
    Gothic,
    Nomadic,
    Underground,
}

impl ArchitectureStyle {
    pub fn all() -> &'static [ArchitectureStyle] {
        &[
            ArchitectureStyle::Imperial,
            ArchitectureStyle::Rustic,
            ArchitectureStyle::Dwarven,
            ArchitectureStyle::Elven,
            ArchitectureStyle::Orcish,
            ArchitectureStyle::Ancient,
            ArchitectureStyle::Gothic,
            ArchitectureStyle::Nomadic,
            ArchitectureStyle::Underground,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ArchitectureStyle::Imperial => "Imperial",
            ArchitectureStyle::Rustic => "Rustic",
            ArchitectureStyle::Dwarven => "Dwarven",
            ArchitectureStyle::Elven => "Elven",
            ArchitectureStyle::Orcish => "Orcish",
            ArchitectureStyle::Ancient => "Ancient",
            ArchitectureStyle::Gothic => "Gothic",
            ArchitectureStyle::Nomadic => "Nomadic",
            ArchitectureStyle::Underground => "Underground",
        }
    }

    /// Default architecture style for a species
    pub fn default_for_species(species: Species) -> Self {
        match species {
            Species::Human => ArchitectureStyle::Imperial,
            Species::Dwarf => ArchitectureStyle::Dwarven,
            Species::Elf => ArchitectureStyle::Elven,
            Species::Orc => ArchitectureStyle::Orcish,
            Species::Goblin => ArchitectureStyle::Underground,
            Species::Giant => ArchitectureStyle::Ancient,
            Species::DragonKin => ArchitectureStyle::Ancient,
            Species::Undead => ArchitectureStyle::Gothic,
            Species::Elemental => ArchitectureStyle::Ancient,
        }
    }
}

/// Relationship between two factions
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactionRelation {
    /// Close allies, will defend each other
    Allied,
    /// Friendly, trade partners
    Friendly,
    /// No strong feelings either way
    Neutral,
    /// Dislike, occasional skirmishes
    Hostile,
    /// Open warfare
    AtWar,
}

impl FactionRelation {
    /// Convert a numeric value (-1.0 to 1.0) to a relation
    pub fn from_value(value: f32) -> Self {
        if value > 0.7 {
            FactionRelation::Allied
        } else if value > 0.3 {
            FactionRelation::Friendly
        } else if value > -0.3 {
            FactionRelation::Neutral
        } else if value > -0.7 {
            FactionRelation::Hostile
        } else {
            FactionRelation::AtWar
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FactionRelation::Allied => "Allied",
            FactionRelation::Friendly => "Friendly",
            FactionRelation::Neutral => "Neutral",
            FactionRelation::Hostile => "Hostile",
            FactionRelation::AtWar => "At War",
        }
    }
}

/// State of a settlement
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettlementState {
    /// Active, growing settlement
    Thriving,
    /// Declining population, reduced activity
    Declining,
    /// Recently abandoned, structures intact
    Abandoned,
    /// Old ruins, partially collapsed
    Ruined,
    /// Completely destroyed (war, disaster)
    Destroyed,
}

impl SettlementState {
    pub fn name(&self) -> &'static str {
        match self {
            SettlementState::Thriving => "Thriving",
            SettlementState::Declining => "Declining",
            SettlementState::Abandoned => "Abandoned",
            SettlementState::Ruined => "Ruined",
            SettlementState::Destroyed => "Destroyed",
        }
    }

    /// How much decay to apply (0.0 = none, 1.0 = complete)
    pub fn decay_factor(&self) -> f32 {
        match self {
            SettlementState::Thriving => 0.0,
            SettlementState::Declining => 0.2,
            SettlementState::Abandoned => 0.5,
            SettlementState::Ruined => 0.8,
            SettlementState::Destroyed => 0.95,
        }
    }
}

/// Type of settlement
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettlementType {
    /// Capital city of a faction
    Capital,
    /// Major city
    City,
    /// Medium-sized town
    Town,
    /// Small village
    Village,
    /// Fortified position
    Fortress,
    /// Religious center
    Temple,
    /// Mining outpost
    Mine,
    /// Trading post
    Outpost,
}

impl SettlementType {
    pub fn name(&self) -> &'static str {
        match self {
            SettlementType::Capital => "Capital",
            SettlementType::City => "City",
            SettlementType::Town => "Town",
            SettlementType::Village => "Village",
            SettlementType::Fortress => "Fortress",
            SettlementType::Temple => "Temple",
            SettlementType::Mine => "Mine",
            SettlementType::Outpost => "Outpost",
        }
    }

    /// Base population range
    pub fn population_range(&self) -> (u32, u32) {
        match self {
            SettlementType::Capital => (5000, 50000),
            SettlementType::City => (1000, 10000),
            SettlementType::Town => (200, 2000),
            SettlementType::Village => (20, 200),
            SettlementType::Fortress => (50, 500),
            SettlementType::Temple => (20, 200),
            SettlementType::Mine => (30, 300),
            SettlementType::Outpost => (10, 100),
        }
    }

    /// Size range in tiles
    pub fn size_range(&self) -> (usize, usize) {
        match self {
            SettlementType::Capital => (60, 100),
            SettlementType::City => (40, 70),
            SettlementType::Town => (25, 45),
            SettlementType::Village => (15, 30),
            SettlementType::Fortress => (30, 50),
            SettlementType::Temple => (20, 40),
            SettlementType::Mine => (15, 30),
            SettlementType::Outpost => (10, 20),
        }
    }
}

/// Historical era type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EraType {
    /// Dawn of civilization
    Primordial,
    /// Golden age of expansion
    GoldenAge,
    /// Major conflict period
    GreatWar,
    /// Decline and chaos
    DarkAge,
    /// Recovery and rebuilding
    Renaissance,
    /// Current era
    Modern,
}

impl EraType {
    pub fn name(&self) -> &'static str {
        match self {
            EraType::Primordial => "Primordial Age",
            EraType::GoldenAge => "Golden Age",
            EraType::GreatWar => "Age of War",
            EraType::DarkAge => "Dark Age",
            EraType::Renaissance => "Age of Rebirth",
            EraType::Modern => "Current Age",
        }
    }

    /// What types of events are common in this era
    pub fn common_events(&self) -> &'static [&'static str] {
        match self {
            EraType::Primordial => &["settlement_founded", "tribe_formed", "discovery"],
            EraType::GoldenAge => &["city_founded", "monument_built", "alliance", "trade_route"],
            EraType::GreatWar => &["battle", "siege", "conquest", "massacre", "hero_death"],
            EraType::DarkAge => &["plague", "famine", "abandonment", "monster_attack", "collapse"],
            EraType::Renaissance => &["rebuilding", "rediscovery", "new_settlement", "treaty"],
            EraType::Modern => &["stability", "trade", "minor_conflict"],
        }
    }
}

/// Reason for settlement abandonment
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AbandonmentReason {
    /// Conquered by another faction
    Conquest,
    /// Destroyed in battle
    War,
    /// Disease wiped out population
    Plague,
    /// Resources depleted
    ResourceDepletion,
    /// Monster infestation
    MonsterAttack,
    /// Natural disaster (earthquake, volcano, flood)
    NaturalDisaster,
    /// Faction collapsed entirely
    FactionCollapse,
    /// Unknown/gradual decline
    Unknown,
}

impl AbandonmentReason {
    pub fn name(&self) -> &'static str {
        match self {
            AbandonmentReason::Conquest => "conquered",
            AbandonmentReason::War => "destroyed in war",
            AbandonmentReason::Plague => "struck by plague",
            AbandonmentReason::ResourceDepletion => "resources depleted",
            AbandonmentReason::MonsterAttack => "overrun by monsters",
            AbandonmentReason::NaturalDisaster => "destroyed by disaster",
            AbandonmentReason::FactionCollapse => "faction collapsed",
            AbandonmentReason::Unknown => "abandoned",
        }
    }
}

/// A point in history (year)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Year(pub i32);

impl Year {
    /// Create a year in the past (negative years before present)
    pub fn years_ago(years: i32) -> Self {
        Year(-years)
    }

    /// Get the number of years ago this was
    pub fn age(&self) -> i32 {
        -self.0
    }
}

impl fmt::Display for Year {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 >= 0 {
            write!(f, "Year {}", self.0)
        } else {
            write!(f, "{} years ago", -self.0)
        }
    }
}
