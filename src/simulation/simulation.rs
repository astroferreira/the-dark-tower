//! Main simulation state and tick loop

use std::collections::HashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::world::WorldData;
use crate::simulation::types::{
    TribeId, TileCoord, SimTick, RelationLevel, TribeEvent, TribeEventType, Treaty,
};
use crate::simulation::params::SimulationParams;
use crate::simulation::tribe::{Tribe, TribeCulture, Settlement};
use crate::simulation::tribe::culture::generate_tribe_name;
use crate::simulation::resources::extract_resources;
use crate::simulation::technology::BuildingType;
use crate::simulation::interaction::{DiplomacyState, process_diplomacy_tick, process_trade_tick, process_conflict_tick};
use crate::simulation::territory::process_expansion_tick;
use crate::simulation::structures::{StructureManager, StructureType};
use crate::simulation::roads::{RoadNetwork, RoadType};
use crate::simulation::monsters::{MonsterManager, MonsterId, Monster};

/// Main simulation state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationState {
    /// Current simulation tick
    pub current_tick: SimTick,
    /// All tribes in the simulation
    pub tribes: HashMap<TribeId, Tribe>,
    /// Diplomatic relations and treaties
    pub diplomacy: DiplomacyState,
    /// Territory ownership map (tile -> tribe)
    pub territory_map: HashMap<TileCoord, TribeId>,
    /// Next tribe ID to assign
    pub next_tribe_id: u32,
    /// Simulation statistics
    pub stats: SimulationStats,
    /// Random seed used
    pub seed: u64,
    /// Structures manager
    pub structures: StructureManager,
    /// Road network
    pub road_network: RoadNetwork,
    /// Monster manager
    pub monsters: MonsterManager,
}

/// Statistics tracked during simulation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimulationStats {
    pub total_tribes_created: u32,
    pub total_tribes_extinct: u32,
    pub total_battles: u32,
    pub total_raids: u32,
    pub total_trades: u32,
    pub total_treaties: u32,
    pub total_age_advances: u32,
    pub peak_population: u32,
    pub current_population: u32,
    pub total_monsters_spawned: u32,
    pub total_monsters_killed: u32,
    pub total_monster_attacks: u32,
    pub current_monster_count: u32,
}

impl SimulationState {
    /// Create a new simulation state
    pub fn new(seed: u64) -> Self {
        SimulationState {
            current_tick: SimTick(0),
            tribes: HashMap::new(),
            diplomacy: DiplomacyState::new(),
            territory_map: HashMap::new(),
            next_tribe_id: 0,
            stats: SimulationStats::default(),
            seed,
            structures: StructureManager::new(),
            road_network: RoadNetwork::new(),
            monsters: MonsterManager::new(),
        }
    }

    /// Initialize the simulation with tribes placed on the world
    pub fn initialize<R: Rng>(
        &mut self,
        world: &WorldData,
        params: &SimulationParams,
        rng: &mut R,
    ) {
        // Find suitable spawn locations
        let spawn_locations = self.find_spawn_locations(world, params, rng);

        // Create tribes at each location
        for location in spawn_locations {
            let biome = *world.biomes.get(location.x, location.y);
            let culture = TribeCulture::from_biome(biome, rng);
            let name = generate_tribe_name(&culture, biome, rng);

            let tribe_id = self.create_tribe(name, location, params.initial_tribe_population, culture);

            // Claim initial territory
            self.claim_initial_territory(tribe_id, location, params.initial_territory_radius, world);

            // Place initial town center structure at capital
            self.structures.create_structure(
                StructureType::TownCenter,
                location,
                Some(tribe_id),
                self.current_tick.0,
            );

            // Record founding event
            if let Some(t) = self.tribes.get_mut(&tribe_id) {
                t.record_event(self.current_tick, TribeEventType::Founded { location });
            }
        }

        // Initialize diplomacy between all tribes
        let tribe_ids: Vec<TribeId> = self.tribes.keys().copied().collect();
        for (i, &tribe_a) in tribe_ids.iter().enumerate() {
            for &tribe_b in tribe_ids.iter().skip(i + 1) {
                self.diplomacy.set_relation(tribe_a, tribe_b, RelationLevel::NEUTRAL);
            }
        }

        self.update_stats();
    }

    /// Find suitable spawn locations for tribes
    fn find_spawn_locations<R: Rng>(
        &self,
        world: &WorldData,
        params: &SimulationParams,
        rng: &mut R,
    ) -> Vec<TileCoord> {
        let mut locations = Vec::new();
        let mut attempts = 0;
        let max_attempts = params.initial_tribe_count * 100;

        while locations.len() < params.initial_tribe_count && attempts < max_attempts {
            attempts += 1;

            let x = rng.gen_range(0..world.heightmap.width);
            let y = rng.gen_range(0..world.heightmap.height);
            let coord = TileCoord::new(x, y);

            // Check if location is suitable
            let elevation = *world.heightmap.get(x, y);
            if elevation < 0.0 {
                continue; // No spawning in water
            }

            let biome = *world.biomes.get(x, y);
            if !is_habitable_biome(biome) {
                continue;
            }

            // Check distance from other spawn locations
            let too_close = locations.iter().any(|other: &TileCoord| {
                coord.distance_wrapped(other, world.heightmap.width) < params.min_tribe_separation
            });

            if !too_close {
                locations.push(coord);
            }
        }

        locations
    }

    /// Create a new tribe
    fn create_tribe(
        &mut self,
        name: String,
        capital: TileCoord,
        population: u32,
        culture: TribeCulture,
    ) -> TribeId {
        let id = TribeId(self.next_tribe_id);
        self.next_tribe_id += 1;

        let tribe = Tribe::new(id, name, capital, population, culture);
        self.tribes.insert(id, tribe);
        self.stats.total_tribes_created += 1;

        id
    }

    /// Claim initial territory around a capital
    fn claim_initial_territory(
        &mut self,
        tribe_id: TribeId,
        center: TileCoord,
        radius: usize,
        world: &WorldData,
    ) {
        let tribe = match self.tribes.get_mut(&tribe_id) {
            Some(t) => t,
            None => return,
        };

        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                let nx = (center.x as i32 + dx).rem_euclid(world.heightmap.width as i32) as usize;
                let ny = (center.y as i32 + dy).clamp(0, world.heightmap.height as i32 - 1) as usize;

                let dist = (dx.abs() + dy.abs()) as usize;
                if dist <= radius {
                    let coord = TileCoord::new(nx, ny);
                    let elevation = *world.heightmap.get(nx, ny);

                    // Only claim land tiles
                    if elevation >= 0.0 && !self.territory_map.contains_key(&coord) {
                        tribe.claim_tile(coord);
                        self.territory_map.insert(coord, tribe_id);
                    }
                }
            }
        }
    }

    /// Run a single simulation tick
    pub fn tick<R: Rng>(&mut self, world: &WorldData, params: &SimulationParams, rng: &mut R) {
        // 1. Resource Production
        self.process_resource_production(world, params);

        // 2. Resource Consumption
        self.process_resource_consumption(params);

        // 3. Needs Calculation
        self.process_needs_calculation(params);

        // 4. Population Dynamics
        self.process_population_dynamics(params);

        // 5. Technology Progress
        self.process_technology_progress(params);

        // 6. Territory Management
        process_expansion_tick(self, world, params, rng);

        // 6.5 Monster Processing
        self.process_monsters(world, params, rng);

        // 6.6 Structure and Road Updates
        self.process_structures_and_roads(world, params, rng);

        // 7. Diplomacy Updates
        process_diplomacy_tick(self, params, rng);

        // 8. Trade
        process_trade_tick(self, params, rng);

        // 9. Conflict
        process_conflict_tick(self, params, rng);

        // 10. Cleanup
        self.cleanup_extinct_tribes();
        self.monsters.cleanup_dead();

        // Advance tick
        self.current_tick = self.current_tick.next();
        self.update_stats();
    }

    /// Process resource extraction for all tribes
    fn process_resource_production(&mut self, world: &WorldData, params: &SimulationParams) {
        let season = self.current_tick.season();

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            let workers = tribe.workers();
            let tech_mult = tribe.tech_state.production_multiplier();
            let needs_mult = tribe.needs.production_modifier;
            let efficiency = params.extraction_efficiency * tech_mult * needs_mult;

            // Extract resources from each territory tile
            for &coord in &tribe.territory {
                let biome = *world.biomes.get(coord.x, coord.y);
                let resources = extract_resources(biome, season, efficiency);

                // Scale by workers (diminishing returns)
                let worker_mult = (workers as f32 / tribe.territory.len() as f32).sqrt().min(2.0);

                for (resource, base_amount) in resources {
                    let amount = base_amount * worker_mult;
                    tribe.stockpile.add(resource, amount);
                }
            }
        }
    }

    /// Process resource consumption
    fn process_resource_consumption(&mut self, params: &SimulationParams) {
        use crate::simulation::types::ResourceType;

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            let pop = tribe.population.total() as f32;

            // Food consumption
            let food_needed = pop * params.food_per_pop_per_tick;
            tribe.stockpile.remove(ResourceType::Food, food_needed);

            // Water consumption
            let water_needed = pop * params.water_per_pop_per_tick;
            tribe.stockpile.remove(ResourceType::Water, water_needed);

            // Apply decay
            tribe.stockpile.apply_decay();
        }
    }

    /// Calculate needs for all tribes
    fn process_needs_calculation(&mut self, params: &SimulationParams) {
        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Calculate shelter capacity from buildings
            let shelter_capacity: u32 = tribe
                .settlements
                .iter()
                .flat_map(|s| s.buildings.iter())
                .map(|b| {
                    // Map building names to BuildingType
                    match b.as_str() {
                        "Hut" => BuildingType::Hut.shelter_capacity(),
                        "Wooden House" | "WoodenHouse" => BuildingType::WoodenHouse.shelter_capacity(),
                        "Barracks" => BuildingType::Barracks.shelter_capacity(),
                        "Castle" => BuildingType::Castle.shelter_capacity(),
                        "Hospital" => BuildingType::Hospital.shelter_capacity(),
                        _ => 0,
                    }
                })
                .sum();

            // Calculate building bonuses
            let health_bonus: f32 = tribe
                .settlements
                .iter()
                .flat_map(|s| s.buildings.iter())
                .map(|b| match b.as_str() {
                    "Well" => BuildingType::Well.health_bonus(),
                    "Aqueduct" => BuildingType::Aqueduct.health_bonus(),
                    "Bathhouse" => BuildingType::Bathhouse.health_bonus(),
                    "Hospital" => BuildingType::Hospital.health_bonus(),
                    _ => 0.0,
                })
                .sum();

            let morale_bonus: f32 = tribe
                .settlements
                .iter()
                .flat_map(|s| s.buildings.iter())
                .map(|b| match b.as_str() {
                    "Campfire" => BuildingType::Campfire.morale_bonus(),
                    "Shrine" => BuildingType::Shrine.morale_bonus(),
                    "Temple" => BuildingType::Temple.morale_bonus(),
                    "Arena" => BuildingType::Arena.morale_bonus(),
                    "Cathedral" => BuildingType::Cathedral.morale_bonus(),
                    "Theatre House" | "TheatreHouse" => BuildingType::TheatreHouse.morale_bonus(),
                    _ => 0.0,
                })
                .sum();

            tribe.needs.calculate(
                tribe.population.total(),
                &tribe.stockpile,
                shelter_capacity,
                health_bonus,
                morale_bonus,
                tribe.population.warriors(),
                params,
            );
        }
    }

    /// Process population changes
    fn process_population_dynamics(&mut self, params: &SimulationParams) {
        let current_tick = self.current_tick;

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            let change = tribe.population.tick(
                &tribe.needs,
                tribe.territory.len(),
                params,
            );

            // Record significant events
            if change.is_starving && change.deaths > 10 {
                tribe.record_event(
                    current_tick,
                    TribeEventType::Famine {
                        severity: 1.0 - tribe.needs.food.satisfaction,
                    },
                );
            }

            if change.deaths > change.births * 2 && change.deaths > 20 {
                tribe.record_event(
                    current_tick,
                    TribeEventType::PopulationDecline {
                        amount: change.deaths - change.births,
                        cause: if change.is_starving {
                            "famine".to_string()
                        } else {
                            "hardship".to_string()
                        },
                    },
                );
            }

            if change.births > change.deaths && change.births > 10 {
                tribe.record_event(
                    current_tick,
                    TribeEventType::PopulationGrowth {
                        amount: change.births - change.deaths,
                    },
                );
            }

            // Update stockpile capacity based on new population
            tribe.stockpile.update_capacity_for_population(
                tribe.population.total(),
                params.max_stockpile_per_pop,
            );
        }
    }

    /// Process technology and research
    fn process_technology_progress(&mut self, params: &SimulationParams) {
        let current_tick = self.current_tick;

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Generate research points from workers
            let workers = tribe.workers();
            let research_bonus: f32 = tribe
                .settlements
                .iter()
                .flat_map(|s| s.buildings.iter())
                .map(|b| match b.as_str() {
                    "Shrine" => BuildingType::Shrine.research_bonus(),
                    "Temple" => BuildingType::Temple.research_bonus(),
                    "Library" => BuildingType::Library.research_bonus(),
                    "University" => BuildingType::University.research_bonus(),
                    "Observatory" => BuildingType::Observatory.research_bonus(),
                    "Printing Press" | "PrintingPress" => BuildingType::PrintingPress.research_bonus(),
                    _ => 0.0,
                })
                .sum();

            let research = workers as f32 * params.research_per_worker + research_bonus;
            tribe.tech_state.add_research(research);

            // Check for age advancement
            let can_advance = tribe.tech_state.can_advance(
                tribe.population.total(),
                |building| tribe.has_building(building),
                |resource, amount| tribe.stockpile.has(resource, amount),
            );

            if can_advance {
                if let Some(new_age) = tribe.tech_state.advance_age() {
                    tribe.record_event(
                        current_tick,
                        TribeEventType::AgeAdvanced {
                            new_age: format!("{:?}", new_age),
                        },
                    );
                    self.stats.total_age_advances += 1;

                    // Grant initial buildings for new age
                    let capital = tribe.capital;
                    if let Some(capital_settlement) = tribe.settlement_at_mut(&capital) {
                        // Add a basic building from the new age
                        let new_buildings = crate::simulation::technology::TechUnlock::buildings_for_age(new_age);
                        if let Some(first_building) = new_buildings.first() {
                            capital_settlement.add_building(first_building.name().to_string());
                        }
                    }
                }
            }
        }
    }

    /// Remove extinct tribes
    fn cleanup_extinct_tribes(&mut self) {
        let extinct: Vec<TribeId> = self
            .tribes
            .iter()
            .filter(|(_, t)| !t.is_alive || t.population.total() == 0)
            .map(|(id, _)| *id)
            .collect();

        for id in extinct {
            // Release territory
            let tiles_to_release: Vec<TileCoord> = self
                .territory_map
                .iter()
                .filter(|(_, &tid)| tid == id)
                .map(|(coord, _)| *coord)
                .collect();

            for coord in tiles_to_release {
                self.territory_map.remove(&coord);
            }

            // Remove from diplomacy
            self.diplomacy.remove_tribe(id);

            // Mark as extinct (keep for history)
            if let Some(tribe) = self.tribes.get_mut(&id) {
                tribe.is_alive = false;
            }

            self.stats.total_tribes_extinct += 1;
        }
    }

    /// Update statistics
    fn update_stats(&mut self) {
        let current_pop: u32 = self
            .tribes
            .values()
            .filter(|t| t.is_alive)
            .map(|t| t.population.total())
            .sum();

        self.stats.current_population = current_pop;
        self.stats.peak_population = self.stats.peak_population.max(current_pop);
    }

    /// Get living tribes
    pub fn living_tribes(&self) -> Vec<&Tribe> {
        self.tribes.values().filter(|t| t.is_alive).collect()
    }

    /// Get a tribe by ID
    pub fn get_tribe(&self, id: TribeId) -> Option<&Tribe> {
        self.tribes.get(&id)
    }

    /// Get mutable tribe by ID
    pub fn get_tribe_mut(&mut self, id: TribeId) -> Option<&mut Tribe> {
        self.tribes.get_mut(&id)
    }

    /// Get tribe that owns a tile
    pub fn tile_owner(&self, coord: &TileCoord) -> Option<TribeId> {
        self.territory_map.get(coord).copied()
    }

    /// Get neighboring tribes of a tribe
    pub fn neighboring_tribes(&self, tribe_id: TribeId) -> Vec<TribeId> {
        let tribe = match self.tribes.get(&tribe_id) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut neighbors = std::collections::HashSet::new();

        for coord in &tribe.territory {
            // Check adjacent tiles for other tribes
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = TileCoord::new(
                        (coord.x as i32 + dx).rem_euclid(512) as usize, // TODO: Use actual width
                        (coord.y as i32 + dy).clamp(0, 255) as usize,   // TODO: Use actual height
                    );

                    if let Some(&owner) = self.territory_map.get(&nx) {
                        if owner != tribe_id {
                            neighbors.insert(owner);
                        }
                    }
                }
            }
        }

        neighbors.into_iter().collect()
    }

    /// Process monster spawning, behavior, and combat
    fn process_monsters<R: Rng>(&mut self, world: &WorldData, _params: &SimulationParams, rng: &mut R) {
        let current_tick = self.current_tick.0;

        // Only spawn monsters every 4 ticks (once per year)
        if current_tick % self.monsters.spawn_params.spawn_interval == 0 {
            let old_count = self.monsters.living_count();
            if let Some(_monster_id) = self.monsters.try_spawn(world, &self.territory_map, current_tick, rng) {
                self.stats.total_monsters_spawned += 1;
            }
        }

        // Process monster behavior (movement, state changes)
        self.monsters.process_behavior(
            &self.tribes,
            &self.territory_map,
            world,
            current_tick,
            rng,
        );

        // Process combat
        let combat_events = self.monsters.process_combat(
            &mut self.tribes,
            &self.territory_map,
            world,
            self.current_tick,
            rng,
        );

        // Update stats based on combat events
        for event in &combat_events {
            self.stats.total_monster_attacks += 1;
            match event {
                crate::simulation::monsters::CombatEvent::MonsterVsTribe { monster_killed, .. } => {
                    if *monster_killed {
                        self.stats.total_monsters_killed += 1;
                    }
                }
                crate::simulation::monsters::CombatEvent::MonsterVsMonster { attacker_killed, defender_killed, .. } => {
                    if *attacker_killed {
                        self.stats.total_monsters_killed += 1;
                    }
                    if *defender_killed {
                        self.stats.total_monsters_killed += 1;
                    }
                }
            }
        }

        // Update current monster count
        self.stats.current_monster_count = self.monsters.living_count() as u32;
    }

    /// Process structures and roads
    fn process_structures_and_roads<R: Rng>(&mut self, world: &WorldData, _params: &SimulationParams, rng: &mut R) {
        // Decay roads over time
        if self.current_tick.0 % 10 == 0 {
            self.road_network.decay_roads();
        }

        // Check if tribes should build roads or structures
        let tribe_data: Vec<(TribeId, TileCoord, usize, crate::simulation::technology::Age)> = self.tribes
            .iter()
            .filter(|(_, t)| t.is_alive)
            .map(|(id, t)| (*id, t.capital, t.population.total() as usize, t.tech_state.current_age()))
            .collect();

        for (tribe_id, capital, population, age) in tribe_data {
            // Build roads between settlements when population is high enough
            if population > 150 && rng.gen::<f32>() < 0.02 {
                // Find another tribe to connect to
                let neighbors = self.neighboring_tribes(tribe_id);
                if let Some(&neighbor_id) = neighbors.first() {
                    if let Some(neighbor) = self.tribes.get(&neighbor_id) {
                        let road_type = match age {
                            crate::simulation::technology::Age::Stone => RoadType::Trail,
                            crate::simulation::technology::Age::Copper => RoadType::Trail,
                            crate::simulation::technology::Age::Bronze => RoadType::Road,
                            _ => RoadType::PavedRoad,
                        };
                        self.road_network.build_road(
                            capital,
                            neighbor.capital,
                            road_type,
                            Some(tribe_id),
                            world,
                        );
                    }
                }
            }

            // Add structures for buildings
            if let Some(tribe) = self.tribes.get(&tribe_id) {
                for settlement in &tribe.settlements {
                    for building in &settlement.buildings {
                        // Check if we should add a structure for this building
                        if !self.structures.has_structure_at(&settlement.location) {
                            if let Some(structure_type) = StructureType::from_building_name(building) {
                                self.structures.create_structure(
                                    structure_type,
                                    settlement.location,
                                    Some(tribe_id),
                                    self.current_tick.0,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get structure at a coordinate
    pub fn get_structure_at(&self, coord: &TileCoord) -> Option<&crate::simulation::structures::Structure> {
        self.structures.get_at(coord)
    }

    /// Get monster at a coordinate
    pub fn get_monster_at(&self, coord: &TileCoord) -> Option<&Monster> {
        self.monsters.get_at(coord)
    }
}

/// Check if a biome is suitable for habitation
fn is_habitable_biome(biome: crate::biomes::ExtendedBiome) -> bool {
    use crate::biomes::ExtendedBiome;

    !matches!(
        biome,
        ExtendedBiome::DeepOcean
            | ExtendedBiome::Ocean
            | ExtendedBiome::CoastalWater
            | ExtendedBiome::Ice
            | ExtendedBiome::LavaLake
            | ExtendedBiome::AcidLake
            | ExtendedBiome::VolcanicWasteland
            | ExtendedBiome::VoidScar
            | ExtendedBiome::VoidMaw
    )
}

/// Run the full simulation
pub fn run_simulation<R: Rng>(
    world: &WorldData,
    params: &SimulationParams,
    num_ticks: u64,
    rng: &mut R,
) -> SimulationState {
    let mut state = SimulationState::new(world.seed);
    state.initialize(world, params, rng);

    println!("Simulation initialized with {} tribes", state.tribes.len());

    for tick in 0..num_ticks {
        state.tick(world, params, rng);

        // Progress reporting every 10 ticks
        if tick > 0 && tick % 10 == 0 {
            let living = state.living_tribes().len();
            let pop = state.stats.current_population;
            println!(
                "Tick {}: {} living tribes, {} total population",
                tick, living, pop
            );
        }
    }

    state
}
