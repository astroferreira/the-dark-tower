//! Tribe module - core civilization unit
//!
//! Tribes are the primary civilization units. Each tribe now includes:
//! - Society type (government form)
//! - Notable colonists (individually tracked)
//! - Population pool (aggregate tracking)
//! - Job assignments
//! - Workplaces

pub mod population;
pub mod needs;
pub mod culture;

use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::simulation::types::{TribeId, TileCoord, TribeEvent, TribeEventType, SimTick, GlobalLocalCoord};
use crate::simulation::resources::Stockpile;
use crate::simulation::technology::TechnologyState;
use crate::simulation::society::{SocietyType, SocietyState};
use crate::simulation::colonists::{NotableColonists, PopulationPool};
use crate::simulation::jobs::JobManager;
use crate::simulation::workplaces::WorkplaceManager;

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

    // Local map positions
    /// City center in global local coordinates
    pub city_center: GlobalLocalCoord,
    /// Radius of the city in local tiles (for colonist placement)
    pub city_radius: usize,

    // Colony simulation fields
    /// Government/society type and state
    pub society_state: SocietyState,
    /// Individually tracked notable colonists (~5% of population)
    #[serde(skip)]
    pub notable_colonists: NotableColonists,
    /// Aggregate population tracking (~95% of population)
    pub population_pool: PopulationPool,
    /// Job assignments and demand
    #[serde(skip)]
    pub jobs: JobManager,
    /// Workplaces in this tribe's territory
    #[serde(skip)]
    pub workplaces: WorkplaceManager,
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

        // Initialize society with default tribal council
        let society_state = SocietyState::new(
            SocietyType::TribalCouncil,
            format!("Chief of {}", name),
        );

        // Initialize population pool (95% of population)
        let population_pool = PopulationPool::new(
            (initial_population as f32 * 0.95) as u32
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
            city_center: GlobalLocalCoord::from_world_tile(capital),
            city_radius: 15,
            society_state,
            notable_colonists: NotableColonists::new(),
            population_pool,
            jobs: JobManager::new(),
            workplaces: WorkplaceManager::new(),
        }
    }

    /// Create a new tribe with a specific society type
    pub fn new_with_society<R: rand::Rng>(
        id: TribeId,
        name: String,
        capital: TileCoord,
        initial_population: u32,
        culture: TribeCulture,
        society_type: SocietyType,
        current_tick: u64,
        rng: &mut R,
    ) -> Self {
        let mut tribe = Self::new(id, name.clone(), capital, initial_population, culture);

        // Set society type
        tribe.society_state = SocietyState::new(society_type, format!("Leader of {}", name));

        // Initialize notable colonists (~5% of population, min 3)
        let notable_count = crate::simulation::colonists::target_notable_count(initial_population);
        for _ in 0..notable_count {
            use crate::simulation::colonists::{Gender, ColonistRole};

            let gender = Gender::random(rng);
            let age = 20 + rng.gen_range(0..30);
            let colonist_id = tribe.notable_colonists.create_colonist(age, gender, current_tick, capital, rng);

            // First notable becomes leader
            if tribe.notable_colonists.count() == 1 {
                if let Some(colonist) = tribe.notable_colonists.get_mut(colonist_id) {
                    colonist.role = ColonistRole::Leader;
                    tribe.society_state.leader_id = Some(colonist.id.0);
                    tribe.society_state.leader_name = colonist.name.clone();
                    tribe.society_state.leader_age = colonist.age;
                }
            }
        }

        tribe
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
            "{} ({}): Pop {} | Territory {} | Age {:?} | {}",
            self.name,
            self.id,
            self.population.total(),
            self.territory.len(),
            self.tech_state.current_age(),
            self.society_state.society_type.name()
        )
    }

    /// Get colony-specific summary
    pub fn colony_summary(&self) -> String {
        format!(
            "{}: {} ({}) | Pop: {} (Notable: {}, Pool: {}) | Workers: {} | Jobs: {}",
            self.name,
            self.society_state.society_type.name(),
            self.society_state.leader_name,
            self.population.total(),
            self.notable_colonists.count(),
            self.population_pool.total(),
            self.population_pool.workers(),
            self.jobs.total_demand()
        )
    }

    /// Get total population including pool and notables
    pub fn total_colony_population(&self) -> u32 {
        self.population_pool.total() + self.notable_colonists.count() as u32
    }

    /// Get workers available for assignment
    pub fn available_workers(&self) -> u32 {
        self.population_pool.available_workers() +
        self.notable_colonists.workers().filter(|c| c.current_job.is_none()).count() as u32
    }
}
