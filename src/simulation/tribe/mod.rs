//! Tribe module - core civilization unit

pub mod population;
pub mod needs;
pub mod culture;

use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::simulation::types::{TribeId, TileCoord, TribeEvent, TribeEventType, SimTick};
use crate::simulation::resources::Stockpile;
use crate::simulation::technology::TechnologyState;

pub use population::Population;
pub use needs::TribeNeeds;
pub use culture::TribeCulture;

/// A settlement within tribe territory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settlement {
    pub location: TileCoord,
    pub population: u32,
    pub buildings: Vec<String>,
    pub is_capital: bool,
}

impl Settlement {
    pub fn new(location: TileCoord, population: u32, is_capital: bool) -> Self {
        Settlement {
            location,
            population,
            buildings: Vec::new(),
            is_capital,
        }
    }

    pub fn add_building(&mut self, building: String) {
        self.buildings.push(building);
    }

    pub fn has_building(&self, building: &str) -> bool {
        self.buildings.iter().any(|b| b == building)
    }
}

/// Main tribe structure representing a civilization unit
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tribe {
    pub id: TribeId,
    pub name: String,
    pub population: Population,
    pub territory: HashSet<TileCoord>,
    pub capital: TileCoord,
    pub settlements: Vec<Settlement>,
    pub culture: TribeCulture,
    pub stockpile: Stockpile,
    pub tech_state: TechnologyState,
    pub needs: TribeNeeds,
    pub events: Vec<TribeEvent>,
    pub is_alive: bool,
}

impl Tribe {
    /// Create a new tribe at the specified location
    pub fn new(
        id: TribeId,
        name: String,
        capital: TileCoord,
        initial_population: u32,
        culture: TribeCulture,
    ) -> Self {
        let mut territory = HashSet::new();
        territory.insert(capital);

        let capital_settlement = Settlement::new(capital, initial_population, true);

        let stockpile = Stockpile::with_initial(
            initial_population as f32 * 2.0, // 2 ticks of food
            initial_population as f32 * 1.0, // 1 tick of water
            50.0,  // Some starting wood
            30.0,  // Some starting stone
        );

        Tribe {
            id,
            name,
            population: Population::new(initial_population),
            territory,
            capital,
            settlements: vec![capital_settlement],
            culture,
            stockpile,
            tech_state: TechnologyState::new(),
            needs: TribeNeeds::default(),
            events: Vec::new(),
            is_alive: true,
        }
    }

    /// Record an event in tribe history
    pub fn record_event(&mut self, tick: SimTick, event_type: TribeEventType) {
        self.events.push(TribeEvent::new(tick, event_type));
    }

    /// Check if this tribe controls a tile
    pub fn owns_tile(&self, coord: &TileCoord) -> bool {
        self.territory.contains(coord)
    }

    /// Add a tile to territory
    pub fn claim_tile(&mut self, coord: TileCoord) {
        self.territory.insert(coord);
    }

    /// Remove a tile from territory
    pub fn lose_tile(&mut self, coord: &TileCoord) {
        self.territory.remove(coord);
    }

    /// Get total population across all settlements
    pub fn total_population(&self) -> u32 {
        self.population.total()
    }

    /// Get number of workers (non-warrior population)
    pub fn workers(&self) -> u32 {
        self.population.workers()
    }

    /// Get number of warriors
    pub fn warriors(&self) -> u32 {
        self.population.warriors()
    }

    /// Get the settlement at a location
    pub fn settlement_at(&self, coord: &TileCoord) -> Option<&Settlement> {
        self.settlements.iter().find(|s| &s.location == coord)
    }

    /// Get mutable settlement at a location
    pub fn settlement_at_mut(&mut self, coord: &TileCoord) -> Option<&mut Settlement> {
        self.settlements.iter_mut().find(|s| &s.location == coord)
    }

    /// Check if tribe has a specific building anywhere
    pub fn has_building(&self, building: &str) -> bool {
        self.settlements.iter().any(|s| s.has_building(building))
    }

    /// Count total buildings of a type
    pub fn count_buildings(&self, building: &str) -> usize {
        self.settlements
            .iter()
            .flat_map(|s| s.buildings.iter())
            .filter(|b| b.as_str() == building)
            .count()
    }

    /// Calculate military strength
    pub fn military_strength(&self) -> f32 {
        let base_warrior_strength = self.warriors() as f32;
        let tech_mult = self.tech_state.military_multiplier();
        let morale_mult = self.needs.military_modifier;
        let equipment_mult = if self.stockpile.get(crate::simulation::types::ResourceType::Weapons) > 0.0 {
            1.3
        } else {
            1.0
        };

        base_warrior_strength * tech_mult * morale_mult * equipment_mult
    }

    /// Check if tribe should be marked as extinct
    pub fn check_extinction(&mut self) -> bool {
        if self.population.total() == 0 {
            self.is_alive = false;
            true
        } else {
            false
        }
    }

    /// Get a summary string for the tribe
    pub fn summary(&self) -> String {
        format!(
            "{} ({}): Pop {} | Territory {} | Age {:?}",
            self.name,
            self.id,
            self.population.total(),
            self.territory.len(),
            self.tech_state.current_age()
        )
    }
}
