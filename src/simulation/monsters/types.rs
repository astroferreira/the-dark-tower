//! Monster types and core data structures

use serde::{Deserialize, Serialize};

use crate::simulation::types::{TileCoord, TribeId, GlobalLocalCoord};
use crate::simulation::interaction::SpeciesDisposition;
use crate::biomes::ExtendedBiome;

/// Unique identifier for a monster
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonsterId(pub u32);

/// A monster entity in the simulation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monster {
    pub id: MonsterId,
    pub species: MonsterSpecies,
    pub location: TileCoord,
    /// Position in global local coordinates (for local map rendering)
    pub local_position: GlobalLocalCoord,
    pub health: f32,
    pub max_health: f32,
    pub strength: f32,
    pub territory_center: TileCoord,
    pub territory_radius: usize,
    pub state: MonsterState,
    pub kills: u32,
    pub last_action_tick: u64,
}

impl Monster {
    pub fn new(
        id: MonsterId,
        species: MonsterSpecies,
        location: TileCoord,
        current_tick: u64,
    ) -> Self {
        let stats = species.stats();
        Monster {
            id,
            species,
            location,
            local_position: GlobalLocalCoord::from_world_tile(location),
            health: stats.health,
            max_health: stats.health,
            strength: stats.strength,
            territory_center: location,
            territory_radius: stats.territory_radius,
            state: MonsterState::Idle,
            kills: 0,
            last_action_tick: current_tick,
        }
    }

    /// Check if monster is dead
    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    /// Take damage, return true if died
    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.health = (self.health - damage).max(0.0);
        self.is_dead()
    }

    /// Heal the monster
    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    /// Check if monster should flee (low health)
    pub fn should_flee(&self) -> bool {
        self.health < self.max_health * 0.25
    }

    /// Get distance to a coordinate
    pub fn distance_to(&self, coord: &TileCoord, map_width: usize) -> usize {
        self.location.distance_wrapped(coord, map_width)
    }

    /// Check if coordinate is within territory
    pub fn in_territory(&self, coord: &TileCoord, map_width: usize) -> bool {
        self.territory_center.distance_wrapped(coord, map_width) <= self.territory_radius
    }
}

/// Different monster species
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MonsterSpecies {
    // Forest monsters
    Wolf,
    Bear,
    GiantSpider,

    // Mountain monsters
    Troll,
    Griffin,
    Dragon,

    // Swamp monsters
    Hydra,
    BogWight,

    // Desert monsters
    Sandworm,
    Scorpion,

    // Arctic monsters
    IceWolf,
    Yeti,

    // Rare/special monsters
    Basilisk,
    Phoenix,
}

/// Stats for a monster species
pub struct MonsterStats {
    pub health: f32,
    pub strength: f32,
    pub territory_radius: usize,
    pub pack_size_min: u32,
    pub pack_size_max: u32,
    pub aggression: f32,
}

impl MonsterSpecies {
    /// Get the ASCII character for this monster
    pub fn map_char(&self) -> char {
        match self {
            MonsterSpecies::Wolf => 'w',
            MonsterSpecies::Bear => 'B',
            MonsterSpecies::GiantSpider => 'x',
            MonsterSpecies::Troll => 'T',
            MonsterSpecies::Griffin => 'G',
            MonsterSpecies::Dragon => 'D',
            MonsterSpecies::Hydra => 'H',
            MonsterSpecies::BogWight => 'b',
            MonsterSpecies::Sandworm => 'W',
            MonsterSpecies::Scorpion => 's',
            MonsterSpecies::IceWolf => 'i',
            MonsterSpecies::Yeti => 'Y',
            MonsterSpecies::Basilisk => 'Z',
            MonsterSpecies::Phoenix => 'P',
        }
    }

    /// Get the color for this monster (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            MonsterSpecies::Wolf => (150, 150, 150),      // Gray
            MonsterSpecies::Bear => (139, 90, 43),        // Brown
            MonsterSpecies::GiantSpider => (50, 50, 50),  // Dark gray
            MonsterSpecies::Troll => (0, 128, 0),         // Green
            MonsterSpecies::Griffin => (218, 165, 32),    // Gold
            MonsterSpecies::Dragon => (255, 0, 0),        // Red
            MonsterSpecies::Hydra => (0, 100, 0),         // Dark green
            MonsterSpecies::BogWight => (128, 128, 0),    // Olive
            MonsterSpecies::Sandworm => (210, 180, 140),  // Tan
            MonsterSpecies::Scorpion => (255, 69, 0),     // Orange-red
            MonsterSpecies::IceWolf => (173, 216, 230),   // Light blue
            MonsterSpecies::Yeti => (255, 255, 255),      // White
            MonsterSpecies::Basilisk => (148, 0, 211),    // Purple
            MonsterSpecies::Phoenix => (255, 165, 0),     // Orange
        }
    }

    /// Get stats for this species
    pub fn stats(&self) -> MonsterStats {
        match self {
            MonsterSpecies::Wolf => MonsterStats {
                health: 30.0,
                strength: 5.0,
                territory_radius: 8,
                pack_size_min: 3,
                pack_size_max: 6,
                aggression: 0.4,
            },
            MonsterSpecies::Bear => MonsterStats {
                health: 80.0,
                strength: 15.0,
                territory_radius: 6,
                pack_size_min: 1,
                pack_size_max: 2,
                aggression: 0.3,
            },
            MonsterSpecies::GiantSpider => MonsterStats {
                health: 25.0,
                strength: 8.0,
                territory_radius: 4,
                pack_size_min: 1,
                pack_size_max: 3,
                aggression: 0.5,
            },
            MonsterSpecies::Troll => MonsterStats {
                health: 150.0,
                strength: 25.0,
                territory_radius: 4,
                pack_size_min: 1,
                pack_size_max: 2,
                aggression: 0.6,
            },
            MonsterSpecies::Griffin => MonsterStats {
                health: 100.0,
                strength: 30.0,
                territory_radius: 15,
                pack_size_min: 1,
                pack_size_max: 2,
                aggression: 0.35,
            },
            MonsterSpecies::Dragon => MonsterStats {
                health: 500.0,
                strength: 100.0,
                territory_radius: 20,
                pack_size_min: 1,
                pack_size_max: 1,
                aggression: 0.3,
            },
            MonsterSpecies::Hydra => MonsterStats {
                health: 200.0,
                strength: 35.0,
                territory_radius: 6,
                pack_size_min: 1,
                pack_size_max: 1,
                aggression: 0.5,
            },
            MonsterSpecies::BogWight => MonsterStats {
                health: 40.0,
                strength: 10.0,
                territory_radius: 5,
                pack_size_min: 2,
                pack_size_max: 4,
                aggression: 0.7,
            },
            MonsterSpecies::Sandworm => MonsterStats {
                health: 200.0,
                strength: 40.0,
                territory_radius: 15,
                pack_size_min: 1,
                pack_size_max: 1,
                aggression: 0.5,
            },
            MonsterSpecies::Scorpion => MonsterStats {
                health: 35.0,
                strength: 12.0,
                territory_radius: 5,
                pack_size_min: 1,
                pack_size_max: 3,
                aggression: 0.4,
            },
            MonsterSpecies::IceWolf => MonsterStats {
                health: 35.0,
                strength: 7.0,
                territory_radius: 10,
                pack_size_min: 4,
                pack_size_max: 8,
                aggression: 0.45,
            },
            MonsterSpecies::Yeti => MonsterStats {
                health: 120.0,
                strength: 30.0,
                territory_radius: 8,
                pack_size_min: 1,
                pack_size_max: 2,
                aggression: 0.35,
            },
            MonsterSpecies::Basilisk => MonsterStats {
                health: 80.0,
                strength: 50.0,
                territory_radius: 6,
                pack_size_min: 1,
                pack_size_max: 1,
                aggression: 0.6,
            },
            MonsterSpecies::Phoenix => MonsterStats {
                health: 150.0,
                strength: 45.0,
                territory_radius: 12,
                pack_size_min: 1,
                pack_size_max: 1,
                aggression: 0.2,
            },
        }
    }

    /// Get the biomes this species can spawn in
    pub fn spawn_biomes(&self) -> Vec<ExtendedBiome> {
        match self {
            MonsterSpecies::Wolf => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::Foothills,
                ExtendedBiome::Tundra,
            ],
            MonsterSpecies::Bear => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateRainforest,
            ],
            MonsterSpecies::GiantSpider => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TropicalRainforest,
                ExtendedBiome::Swamp,
            ],
            MonsterSpecies::Troll => vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::Foothills,
                ExtendedBiome::SnowyPeaks,
            ],
            MonsterSpecies::Griffin => vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::Foothills,
            ],
            MonsterSpecies::Dragon => vec![
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::RazorPeaks,
            ],
            MonsterSpecies::Hydra => vec![
                ExtendedBiome::Swamp,
                ExtendedBiome::Marsh,
            ],
            MonsterSpecies::BogWight => vec![
                ExtendedBiome::Swamp,
                ExtendedBiome::Marsh,
                ExtendedBiome::Bog,
            ],
            MonsterSpecies::Sandworm => vec![
                ExtendedBiome::Desert,
                ExtendedBiome::SaltFlats,
            ],
            MonsterSpecies::Scorpion => vec![
                ExtendedBiome::Desert,
                ExtendedBiome::SaltFlats,
                ExtendedBiome::Savanna,
            ],
            MonsterSpecies::IceWolf => vec![
                ExtendedBiome::Tundra,
                ExtendedBiome::Ice,
                ExtendedBiome::FrozenLake,
            ],
            MonsterSpecies::Yeti => vec![
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::Ice,
                ExtendedBiome::Tundra,
            ],
            MonsterSpecies::Basilisk => vec![
                ExtendedBiome::Desert,
                ExtendedBiome::SaltFlats,
                ExtendedBiome::VolcanicWasteland,
            ],
            MonsterSpecies::Phoenix => vec![
                ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::Desert,
            ],
        }
    }

    /// Check if this species can spawn in a given biome
    pub fn can_spawn_in(&self, biome: ExtendedBiome) -> bool {
        self.spawn_biomes().contains(&biome)
    }

    /// Get spawn rarity (lower = more common)
    pub fn rarity(&self) -> u32 {
        match self {
            MonsterSpecies::Wolf => 10,
            MonsterSpecies::Bear => 20,
            MonsterSpecies::GiantSpider => 15,
            MonsterSpecies::Troll => 30,
            MonsterSpecies::Griffin => 50,
            MonsterSpecies::Dragon => 200,
            MonsterSpecies::Hydra => 40,
            MonsterSpecies::BogWight => 25,
            MonsterSpecies::Sandworm => 60,
            MonsterSpecies::Scorpion => 20,
            MonsterSpecies::IceWolf => 15,
            MonsterSpecies::Yeti => 40,
            MonsterSpecies::Basilisk => 100,
            MonsterSpecies::Phoenix => 150,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            MonsterSpecies::Wolf => "Wolf",
            MonsterSpecies::Bear => "Bear",
            MonsterSpecies::GiantSpider => "Giant Spider",
            MonsterSpecies::Troll => "Troll",
            MonsterSpecies::Griffin => "Griffin",
            MonsterSpecies::Dragon => "Dragon",
            MonsterSpecies::Hydra => "Hydra",
            MonsterSpecies::BogWight => "Bog Wight",
            MonsterSpecies::Sandworm => "Sandworm",
            MonsterSpecies::Scorpion => "Giant Scorpion",
            MonsterSpecies::IceWolf => "Ice Wolf",
            MonsterSpecies::Yeti => "Yeti",
            MonsterSpecies::Basilisk => "Basilisk",
            MonsterSpecies::Phoenix => "Phoenix",
        }
    }

    /// Get the disposition category for this species
    /// Determines baseline reputation and max reputation with tribes
    pub fn disposition(&self) -> SpeciesDisposition {
        match self {
            // Always hostile - powerful apex predators
            MonsterSpecies::Dragon
            | MonsterSpecies::Hydra
            | MonsterSpecies::Sandworm
            | MonsterSpecies::Basilisk => SpeciesDisposition::AlwaysHostile,

            // Territorial - defend territory but can coexist
            MonsterSpecies::Troll
            | MonsterSpecies::Bear
            | MonsterSpecies::Yeti
            | MonsterSpecies::Griffin => SpeciesDisposition::Territorial,

            // Neutral - pack animals, can become allies or enemies
            MonsterSpecies::Wolf
            | MonsterSpecies::IceWolf
            | MonsterSpecies::Scorpion
            | MonsterSpecies::GiantSpider => SpeciesDisposition::Neutral,

            // Mythical - rare, potentially beneficial
            MonsterSpecies::Phoenix => SpeciesDisposition::Mythical,

            // Undead - always hostile, minimal negotiation
            MonsterSpecies::BogWight => SpeciesDisposition::Undead,
        }
    }

    /// Check if this is a significant monster (for reputation purposes)
    pub fn is_significant(&self) -> bool {
        matches!(
            self.disposition(),
            SpeciesDisposition::AlwaysHostile | SpeciesDisposition::Mythical
        )
    }
}

/// Current state of a monster
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterState {
    Idle,
    Roaming,
    Hunting,
    Attacking(AttackTarget),
    Fleeing,
    Dead,
}

/// What a monster is attacking
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackTarget {
    Tribe(TribeId),
    Monster(MonsterId),
}

/// All monster species for iteration
pub const ALL_SPECIES: &[MonsterSpecies] = &[
    MonsterSpecies::Wolf,
    MonsterSpecies::Bear,
    MonsterSpecies::GiantSpider,
    MonsterSpecies::Troll,
    MonsterSpecies::Griffin,
    MonsterSpecies::Dragon,
    MonsterSpecies::Hydra,
    MonsterSpecies::BogWight,
    MonsterSpecies::Sandworm,
    MonsterSpecies::Scorpion,
    MonsterSpecies::IceWolf,
    MonsterSpecies::Yeti,
    MonsterSpecies::Basilisk,
    MonsterSpecies::Phoenix,
];
