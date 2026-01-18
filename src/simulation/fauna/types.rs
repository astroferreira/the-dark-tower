//! Fauna types and core data structures
//!
//! Defines passive and neutral animals that populate the world,
//! providing hunting resources and ambient life.

use serde::{Deserialize, Serialize};

use crate::biomes::ExtendedBiome;
use crate::simulation::types::{GlobalLocalCoord, TileCoord};

/// Unique identifier for a fauna creature
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaunaId(pub u32);

/// A fauna entity in the simulation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fauna {
    pub id: FaunaId,
    pub species: FaunaSpecies,
    pub location: TileCoord,
    /// Position in global local coordinates (for local map rendering)
    pub local_position: GlobalLocalCoord,
    pub health: f32,
    pub max_health: f32,
    /// Home range center
    pub home_range_center: TileCoord,
    pub home_range_radius: usize,
    pub state: FaunaState,
    /// Age in ticks
    pub age: u32,
    /// Hunger level (0.0 = full, 1.0 = starving)
    pub hunger: f32,
    /// Whether this is a male (for breeding)
    pub is_male: bool,
    /// Tick when last bred
    pub last_breed_tick: u64,
    /// Current activity for display
    pub current_activity: FaunaActivity,
    pub last_action_tick: u64,
}

impl Fauna {
    pub fn new(
        id: FaunaId,
        species: FaunaSpecies,
        location: TileCoord,
        is_male: bool,
        current_tick: u64,
    ) -> Self {
        let stats = species.stats();
        // Scatter fauna within the tile
        let base_pos = GlobalLocalCoord::from_world_tile(location);
        let scatter_x = ((id.0 as i32 * 13) % 50) - 25;
        let scatter_y = ((id.0 as i32 * 23) % 50) - 25;
        let local_position = GlobalLocalCoord::new(
            (base_pos.x as i32 + scatter_x).max(0) as u32,
            (base_pos.y as i32 + scatter_y).max(0) as u32,
        );

        Fauna {
            id,
            species,
            location,
            local_position,
            health: stats.health,
            max_health: stats.health,
            home_range_center: location,
            home_range_radius: stats.home_range,
            state: FaunaState::Idle,
            age: 0,
            hunger: 0.3,
            is_male,
            last_breed_tick: 0,
            current_activity: FaunaActivity::Resting,
            last_action_tick: current_tick,
        }
    }

    /// Check if fauna is dead
    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    /// Take damage, return true if died
    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.health = (self.health - damage).max(0.0);
        self.is_dead()
    }

    /// Heal the fauna
    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    /// Get distance to a coordinate
    pub fn distance_to(&self, coord: &TileCoord, map_width: usize) -> usize {
        self.location.distance_wrapped(coord, map_width)
    }

    /// Check if coordinate is within home range
    pub fn in_home_range(&self, coord: &TileCoord, map_width: usize) -> bool {
        self.home_range_center.distance_wrapped(coord, map_width) <= self.home_range_radius
    }

    /// Check if can breed (adult, not recently bred, not hungry)
    pub fn can_breed(&self, current_tick: u64) -> bool {
        let stats = self.species.stats();
        self.age >= stats.maturity_age
            && current_tick - self.last_breed_tick >= stats.breed_cooldown
            && self.hunger < 0.5
            && self.health > self.max_health * 0.7
    }
}

/// Different fauna species
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FaunaSpecies {
    // Forest fauna
    Deer,
    Rabbit,
    Squirrel,
    Boar,
    Fox,

    // Plains fauna
    Bison,
    Horse,
    Elk,
    PrairieDog,

    // Mountain fauna
    MountainGoat,
    Eagle,
    Marmot,

    // Arctic fauna
    ArcticHare,
    Caribou,
    Seal,
    Penguin,

    // Desert fauna
    Camel,
    Lizard,
    Vulture,

    // Swamp fauna
    Frog,
    Heron,
    Alligator,

    // Tropical fauna
    Monkey,
    Parrot,
    Tapir,

    // Aquatic fauna (coastal/lakes)
    Fish,
    Salmon,
    Crab,
}

/// Stats for a fauna species
#[derive(Clone, Debug)]
pub struct FaunaStats {
    pub health: f32,
    pub speed: f32,
    pub home_range: usize,
    pub herd_size_min: u32,
    pub herd_size_max: u32,
    /// Alertness (0-1, higher = harder to hunt)
    pub alertness: f32,
    /// Food value when hunted
    pub food_value: f32,
    /// Hide/material value when hunted
    pub material_value: f32,
    /// Age at maturity (in ticks)
    pub maturity_age: u32,
    /// Ticks between breeding
    pub breed_cooldown: u64,
    /// Offspring per breeding
    pub offspring_count: u32,
    /// Diet type
    pub diet: FaunaDiet,
}

/// Diet type for fauna
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaunaDiet {
    Herbivore,
    Carnivore,
    Omnivore,
    Piscivore,
}

impl FaunaSpecies {
    /// Get the ASCII character for this fauna
    pub fn map_char(&self) -> char {
        match self {
            FaunaSpecies::Deer => 'd',
            FaunaSpecies::Rabbit => 'r',
            FaunaSpecies::Squirrel => 'q',
            FaunaSpecies::Boar => 'b',
            FaunaSpecies::Fox => 'f',
            FaunaSpecies::Bison => 'B',
            FaunaSpecies::Horse => 'h',
            FaunaSpecies::Elk => 'E',
            FaunaSpecies::PrairieDog => 'p',
            FaunaSpecies::MountainGoat => 'g',
            FaunaSpecies::Eagle => 'e',
            FaunaSpecies::Marmot => 'm',
            FaunaSpecies::ArcticHare => 'a',
            FaunaSpecies::Caribou => 'C',
            FaunaSpecies::Seal => 'S',
            FaunaSpecies::Penguin => 'P',
            FaunaSpecies::Camel => 'c',
            FaunaSpecies::Lizard => 'l',
            FaunaSpecies::Vulture => 'v',
            FaunaSpecies::Frog => 'F',
            FaunaSpecies::Heron => 'H',
            FaunaSpecies::Alligator => 'A',
            FaunaSpecies::Monkey => 'M',
            FaunaSpecies::Parrot => 't',
            FaunaSpecies::Tapir => 'T',
            FaunaSpecies::Fish => '~',
            FaunaSpecies::Salmon => 's',
            FaunaSpecies::Crab => 'x',
        }
    }

    /// Get the color for this fauna (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            FaunaSpecies::Deer => (160, 120, 80),       // Brown
            FaunaSpecies::Rabbit => (180, 160, 140),   // Light brown
            FaunaSpecies::Squirrel => (139, 90, 43),   // Reddish brown
            FaunaSpecies::Boar => (100, 80, 60),       // Dark brown
            FaunaSpecies::Fox => (255, 140, 0),        // Orange
            FaunaSpecies::Bison => (80, 60, 40),       // Dark brown
            FaunaSpecies::Horse => (160, 140, 120),    // Tan
            FaunaSpecies::Elk => (140, 100, 60),       // Brown
            FaunaSpecies::PrairieDog => (200, 180, 140), // Sandy
            FaunaSpecies::MountainGoat => (220, 220, 220), // Light gray
            FaunaSpecies::Eagle => (100, 80, 60),      // Brown
            FaunaSpecies::Marmot => (160, 140, 100),   // Tan
            FaunaSpecies::ArcticHare => (250, 250, 250), // White
            FaunaSpecies::Caribou => (120, 100, 80),   // Brown
            FaunaSpecies::Seal => (100, 100, 110),     // Gray
            FaunaSpecies::Penguin => (40, 40, 40),     // Black
            FaunaSpecies::Camel => (210, 180, 140),    // Tan
            FaunaSpecies::Lizard => (120, 140, 80),    // Green-brown
            FaunaSpecies::Vulture => (60, 50, 40),     // Dark brown
            FaunaSpecies::Frog => (50, 150, 50),       // Green
            FaunaSpecies::Heron => (150, 150, 160),    // Gray-blue
            FaunaSpecies::Alligator => (60, 80, 50),   // Dark green
            FaunaSpecies::Monkey => (140, 100, 70),    // Brown
            FaunaSpecies::Parrot => (50, 200, 50),     // Green
            FaunaSpecies::Tapir => (80, 70, 60),       // Dark gray
            FaunaSpecies::Fish => (100, 150, 200),     // Blue-gray
            FaunaSpecies::Salmon => (250, 130, 110),   // Pink
            FaunaSpecies::Crab => (220, 100, 50),      // Orange-red
        }
    }

    /// Get stats for this species
    pub fn stats(&self) -> FaunaStats {
        match self {
            FaunaSpecies::Deer => FaunaStats {
                health: 40.0,
                speed: 1.5,
                home_range: 10,
                herd_size_min: 3,
                herd_size_max: 8,
                alertness: 0.7,
                food_value: 30.0,
                material_value: 15.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Rabbit => FaunaStats {
                health: 10.0,
                speed: 2.0,
                home_range: 4,
                herd_size_min: 2,
                herd_size_max: 6,
                alertness: 0.8,
                food_value: 5.0,
                material_value: 2.0,
                maturity_age: 4,
                breed_cooldown: 4,
                offspring_count: 4,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Squirrel => FaunaStats {
                health: 5.0,
                speed: 2.5,
                home_range: 3,
                herd_size_min: 1,
                herd_size_max: 3,
                alertness: 0.9,
                food_value: 2.0,
                material_value: 1.0,
                maturity_age: 4,
                breed_cooldown: 8,
                offspring_count: 3,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Boar => FaunaStats {
                health: 50.0,
                speed: 1.2,
                home_range: 8,
                herd_size_min: 2,
                herd_size_max: 6,
                alertness: 0.5,
                food_value: 35.0,
                material_value: 10.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 4,
                diet: FaunaDiet::Omnivore,
            },
            FaunaSpecies::Fox => FaunaStats {
                health: 20.0,
                speed: 1.8,
                home_range: 12,
                herd_size_min: 1,
                herd_size_max: 2,
                alertness: 0.8,
                food_value: 8.0,
                material_value: 8.0,
                maturity_age: 4,
                breed_cooldown: 16,
                offspring_count: 4,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Bison => FaunaStats {
                health: 100.0,
                speed: 1.0,
                home_range: 15,
                herd_size_min: 10,
                herd_size_max: 30,
                alertness: 0.4,
                food_value: 60.0,
                material_value: 30.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Horse => FaunaStats {
                health: 60.0,
                speed: 2.0,
                home_range: 20,
                herd_size_min: 5,
                herd_size_max: 15,
                alertness: 0.6,
                food_value: 40.0,
                material_value: 20.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Elk => FaunaStats {
                health: 70.0,
                speed: 1.4,
                home_range: 12,
                herd_size_min: 4,
                herd_size_max: 12,
                alertness: 0.6,
                food_value: 45.0,
                material_value: 25.0,
                maturity_age: 12,
                breed_cooldown: 16,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::PrairieDog => FaunaStats {
                health: 5.0,
                speed: 1.5,
                home_range: 2,
                herd_size_min: 5,
                herd_size_max: 20,
                alertness: 0.9,
                food_value: 2.0,
                material_value: 1.0,
                maturity_age: 4,
                breed_cooldown: 8,
                offspring_count: 5,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::MountainGoat => FaunaStats {
                health: 45.0,
                speed: 1.3,
                home_range: 8,
                herd_size_min: 3,
                herd_size_max: 10,
                alertness: 0.7,
                food_value: 25.0,
                material_value: 15.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Eagle => FaunaStats {
                health: 15.0,
                speed: 3.0,
                home_range: 25,
                herd_size_min: 1,
                herd_size_max: 2,
                alertness: 0.95,
                food_value: 5.0,
                material_value: 10.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 2,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Marmot => FaunaStats {
                health: 15.0,
                speed: 1.2,
                home_range: 3,
                herd_size_min: 2,
                herd_size_max: 8,
                alertness: 0.8,
                food_value: 8.0,
                material_value: 4.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 4,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::ArcticHare => FaunaStats {
                health: 8.0,
                speed: 2.2,
                home_range: 5,
                herd_size_min: 1,
                herd_size_max: 4,
                alertness: 0.85,
                food_value: 4.0,
                material_value: 3.0,
                maturity_age: 4,
                breed_cooldown: 8,
                offspring_count: 5,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Caribou => FaunaStats {
                health: 55.0,
                speed: 1.6,
                home_range: 30,
                herd_size_min: 20,
                herd_size_max: 100,
                alertness: 0.5,
                food_value: 40.0,
                material_value: 25.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Seal => FaunaStats {
                health: 35.0,
                speed: 0.8,
                home_range: 10,
                herd_size_min: 5,
                herd_size_max: 30,
                alertness: 0.6,
                food_value: 30.0,
                material_value: 20.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 1,
                diet: FaunaDiet::Piscivore,
            },
            FaunaSpecies::Penguin => FaunaStats {
                health: 12.0,
                speed: 0.6,
                home_range: 5,
                herd_size_min: 10,
                herd_size_max: 50,
                alertness: 0.4,
                food_value: 8.0,
                material_value: 3.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 1,
                diet: FaunaDiet::Piscivore,
            },
            FaunaSpecies::Camel => FaunaStats {
                health: 70.0,
                speed: 1.2,
                home_range: 25,
                herd_size_min: 3,
                herd_size_max: 12,
                alertness: 0.5,
                food_value: 45.0,
                material_value: 25.0,
                maturity_age: 16,
                breed_cooldown: 24,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Lizard => FaunaStats {
                health: 3.0,
                speed: 1.8,
                home_range: 2,
                herd_size_min: 1,
                herd_size_max: 3,
                alertness: 0.7,
                food_value: 1.0,
                material_value: 1.0,
                maturity_age: 4,
                breed_cooldown: 8,
                offspring_count: 6,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Vulture => FaunaStats {
                health: 12.0,
                speed: 2.5,
                home_range: 30,
                herd_size_min: 1,
                herd_size_max: 6,
                alertness: 0.9,
                food_value: 3.0,
                material_value: 5.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 2,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Frog => FaunaStats {
                health: 2.0,
                speed: 1.0,
                home_range: 1,
                herd_size_min: 3,
                herd_size_max: 15,
                alertness: 0.6,
                food_value: 1.0,
                material_value: 0.0,
                maturity_age: 2,
                breed_cooldown: 4,
                offspring_count: 20,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Heron => FaunaStats {
                health: 10.0,
                speed: 1.5,
                home_range: 8,
                herd_size_min: 1,
                herd_size_max: 4,
                alertness: 0.8,
                food_value: 5.0,
                material_value: 5.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 3,
                diet: FaunaDiet::Piscivore,
            },
            FaunaSpecies::Alligator => FaunaStats {
                health: 80.0,
                speed: 0.6,
                home_range: 6,
                herd_size_min: 1,
                herd_size_max: 3,
                alertness: 0.4,
                food_value: 30.0,
                material_value: 35.0,
                maturity_age: 16,
                breed_cooldown: 20,
                offspring_count: 15,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Monkey => FaunaStats {
                health: 15.0,
                speed: 2.0,
                home_range: 6,
                herd_size_min: 5,
                herd_size_max: 20,
                alertness: 0.9,
                food_value: 8.0,
                material_value: 3.0,
                maturity_age: 8,
                breed_cooldown: 12,
                offspring_count: 1,
                diet: FaunaDiet::Omnivore,
            },
            FaunaSpecies::Parrot => FaunaStats {
                health: 5.0,
                speed: 2.5,
                home_range: 8,
                herd_size_min: 2,
                herd_size_max: 10,
                alertness: 0.9,
                food_value: 2.0,
                material_value: 5.0,
                maturity_age: 8,
                breed_cooldown: 16,
                offspring_count: 3,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Tapir => FaunaStats {
                health: 50.0,
                speed: 1.0,
                home_range: 8,
                herd_size_min: 1,
                herd_size_max: 3,
                alertness: 0.5,
                food_value: 35.0,
                material_value: 15.0,
                maturity_age: 12,
                breed_cooldown: 20,
                offspring_count: 1,
                diet: FaunaDiet::Herbivore,
            },
            FaunaSpecies::Fish => FaunaStats {
                health: 2.0,
                speed: 1.5,
                home_range: 5,
                herd_size_min: 10,
                herd_size_max: 50,
                alertness: 0.5,
                food_value: 3.0,
                material_value: 0.0,
                maturity_age: 2,
                breed_cooldown: 4,
                offspring_count: 100,
                diet: FaunaDiet::Omnivore,
            },
            FaunaSpecies::Salmon => FaunaStats {
                health: 5.0,
                speed: 2.0,
                home_range: 15,
                herd_size_min: 5,
                herd_size_max: 30,
                alertness: 0.6,
                food_value: 8.0,
                material_value: 0.0,
                maturity_age: 4,
                breed_cooldown: 8,
                offspring_count: 500,
                diet: FaunaDiet::Carnivore,
            },
            FaunaSpecies::Crab => FaunaStats {
                health: 5.0,
                speed: 0.5,
                home_range: 2,
                herd_size_min: 3,
                herd_size_max: 10,
                alertness: 0.4,
                food_value: 4.0,
                material_value: 2.0,
                maturity_age: 4,
                breed_cooldown: 8,
                offspring_count: 100,
                diet: FaunaDiet::Omnivore,
            },
        }
    }

    /// Get the biomes this species can spawn in
    pub fn spawn_biomes(&self) -> Vec<ExtendedBiome> {
        match self {
            FaunaSpecies::Deer => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::Foothills,
                ExtendedBiome::TemperateGrassland,
            ],
            FaunaSpecies::Rabbit => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Foothills,
                ExtendedBiome::Savanna,
            ],
            FaunaSpecies::Squirrel => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::TropicalForest,
            ],
            FaunaSpecies::Boar => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::Foothills,
            ],
            FaunaSpecies::Fox => vec![
                ExtendedBiome::TemperateForest,
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Tundra,
            ],
            FaunaSpecies::Bison => vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
            ],
            FaunaSpecies::Horse => vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
                ExtendedBiome::Foothills,
            ],
            FaunaSpecies::Elk => vec![
                ExtendedBiome::BorealForest,
                ExtendedBiome::TemperateForest,
                ExtendedBiome::AlpineTundra,
            ],
            FaunaSpecies::PrairieDog => vec![
                ExtendedBiome::TemperateGrassland,
                ExtendedBiome::Savanna,
                ExtendedBiome::Desert,
            ],
            FaunaSpecies::MountainGoat => vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::Foothills,
            ],
            FaunaSpecies::Eagle => vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
                ExtendedBiome::Foothills,
                ExtendedBiome::TemperateForest,
            ],
            FaunaSpecies::Marmot => vec![
                ExtendedBiome::AlpineTundra,
                ExtendedBiome::SnowyPeaks,
            ],
            FaunaSpecies::ArcticHare => vec![
                ExtendedBiome::Tundra,
                ExtendedBiome::Ice,
            ],
            FaunaSpecies::Caribou => vec![
                ExtendedBiome::Tundra,
                ExtendedBiome::BorealForest,
            ],
            FaunaSpecies::Seal => vec![
                ExtendedBiome::Ice,
                ExtendedBiome::CoastalWater,
                ExtendedBiome::FrozenLake,
            ],
            FaunaSpecies::Penguin => vec![
                ExtendedBiome::Ice,
                ExtendedBiome::Tundra,
            ],
            FaunaSpecies::Camel => vec![
                ExtendedBiome::Desert,
                ExtendedBiome::SaltFlats,
            ],
            FaunaSpecies::Lizard => vec![
                ExtendedBiome::Desert,
                ExtendedBiome::Savanna,
                ExtendedBiome::SaltFlats,
            ],
            FaunaSpecies::Vulture => vec![
                ExtendedBiome::Desert,
                ExtendedBiome::Savanna,
                ExtendedBiome::SaltFlats,
            ],
            FaunaSpecies::Frog => vec![
                ExtendedBiome::Swamp,
                ExtendedBiome::Marsh,
                ExtendedBiome::TropicalRainforest,
            ],
            FaunaSpecies::Heron => vec![
                ExtendedBiome::Swamp,
                ExtendedBiome::Marsh,
                ExtendedBiome::MirrorLake,
            ],
            FaunaSpecies::Alligator => vec![
                ExtendedBiome::Swamp,
                ExtendedBiome::Marsh,
            ],
            FaunaSpecies::Monkey => vec![
                ExtendedBiome::TropicalRainforest,
                ExtendedBiome::TropicalForest,
            ],
            FaunaSpecies::Parrot => vec![
                ExtendedBiome::TropicalRainforest,
                ExtendedBiome::TropicalForest,
            ],
            FaunaSpecies::Tapir => vec![
                ExtendedBiome::TropicalRainforest,
                ExtendedBiome::TropicalForest,
            ],
            FaunaSpecies::Fish => vec![
                ExtendedBiome::CoastalWater,
                ExtendedBiome::MirrorLake,
                ExtendedBiome::Lagoon,
            ],
            FaunaSpecies::Salmon => vec![
                ExtendedBiome::CoastalWater,
                ExtendedBiome::MirrorLake,
            ],
            FaunaSpecies::Crab => vec![
                ExtendedBiome::CoastalWater,
                ExtendedBiome::VolcanicBeach,
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
            FaunaSpecies::Rabbit | FaunaSpecies::Squirrel | FaunaSpecies::Fish => 5,
            FaunaSpecies::Deer | FaunaSpecies::Frog | FaunaSpecies::Lizard => 8,
            FaunaSpecies::Boar | FaunaSpecies::Fox | FaunaSpecies::PrairieDog => 10,
            FaunaSpecies::Elk | FaunaSpecies::MountainGoat | FaunaSpecies::Marmot => 15,
            FaunaSpecies::Bison | FaunaSpecies::Horse | FaunaSpecies::Caribou => 20,
            FaunaSpecies::Eagle | FaunaSpecies::Heron | FaunaSpecies::Vulture => 25,
            FaunaSpecies::ArcticHare | FaunaSpecies::Seal | FaunaSpecies::Penguin => 15,
            FaunaSpecies::Camel | FaunaSpecies::Alligator | FaunaSpecies::Tapir => 30,
            FaunaSpecies::Monkey | FaunaSpecies::Parrot | FaunaSpecies::Crab | FaunaSpecies::Salmon => 12,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            FaunaSpecies::Deer => "Deer",
            FaunaSpecies::Rabbit => "Rabbit",
            FaunaSpecies::Squirrel => "Squirrel",
            FaunaSpecies::Boar => "Wild Boar",
            FaunaSpecies::Fox => "Fox",
            FaunaSpecies::Bison => "Bison",
            FaunaSpecies::Horse => "Wild Horse",
            FaunaSpecies::Elk => "Elk",
            FaunaSpecies::PrairieDog => "Prairie Dog",
            FaunaSpecies::MountainGoat => "Mountain Goat",
            FaunaSpecies::Eagle => "Eagle",
            FaunaSpecies::Marmot => "Marmot",
            FaunaSpecies::ArcticHare => "Arctic Hare",
            FaunaSpecies::Caribou => "Caribou",
            FaunaSpecies::Seal => "Seal",
            FaunaSpecies::Penguin => "Penguin",
            FaunaSpecies::Camel => "Camel",
            FaunaSpecies::Lizard => "Lizard",
            FaunaSpecies::Vulture => "Vulture",
            FaunaSpecies::Frog => "Frog",
            FaunaSpecies::Heron => "Heron",
            FaunaSpecies::Alligator => "Alligator",
            FaunaSpecies::Monkey => "Monkey",
            FaunaSpecies::Parrot => "Parrot",
            FaunaSpecies::Tapir => "Tapir",
            FaunaSpecies::Fish => "Fish",
            FaunaSpecies::Salmon => "Salmon",
            FaunaSpecies::Crab => "Crab",
        }
    }
}

/// Current state of a fauna creature
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaunaState {
    Idle,
    Grazing,
    Roaming,
    Fleeing,
    Breeding,
    Migrating,
    Hunting,   // For predators
    Dead,
}

/// Current activity for display purposes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaunaActivity {
    Resting,
    Eating,
    Drinking,
    Wandering,
    Running,
    Playing,
    Grooming,
    Sleeping,
    Nesting,
    Swimming,
    Flying,
    Hunting,
    Fighting,
}

impl FaunaActivity {
    pub fn description(&self) -> &'static str {
        match self {
            FaunaActivity::Resting => "resting",
            FaunaActivity::Eating => "eating",
            FaunaActivity::Drinking => "drinking",
            FaunaActivity::Wandering => "wandering",
            FaunaActivity::Running => "running",
            FaunaActivity::Playing => "playing",
            FaunaActivity::Grooming => "grooming",
            FaunaActivity::Sleeping => "sleeping",
            FaunaActivity::Nesting => "nesting",
            FaunaActivity::Swimming => "swimming",
            FaunaActivity::Flying => "flying",
            FaunaActivity::Hunting => "hunting",
            FaunaActivity::Fighting => "fighting",
        }
    }
}

/// All fauna species for iteration
pub const ALL_FAUNA_SPECIES: &[FaunaSpecies] = &[
    FaunaSpecies::Deer,
    FaunaSpecies::Rabbit,
    FaunaSpecies::Squirrel,
    FaunaSpecies::Boar,
    FaunaSpecies::Fox,
    FaunaSpecies::Bison,
    FaunaSpecies::Horse,
    FaunaSpecies::Elk,
    FaunaSpecies::PrairieDog,
    FaunaSpecies::MountainGoat,
    FaunaSpecies::Eagle,
    FaunaSpecies::Marmot,
    FaunaSpecies::ArcticHare,
    FaunaSpecies::Caribou,
    FaunaSpecies::Seal,
    FaunaSpecies::Penguin,
    FaunaSpecies::Camel,
    FaunaSpecies::Lizard,
    FaunaSpecies::Vulture,
    FaunaSpecies::Frog,
    FaunaSpecies::Heron,
    FaunaSpecies::Alligator,
    FaunaSpecies::Monkey,
    FaunaSpecies::Parrot,
    FaunaSpecies::Tapir,
    FaunaSpecies::Fish,
    FaunaSpecies::Salmon,
    FaunaSpecies::Crab,
];
